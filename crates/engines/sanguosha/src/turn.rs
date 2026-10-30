//! 回合阶段机与出牌动作（P2.2/P2.3）。
//!
//! - 阶段机只负责 `prepare → judge → draw → play → discard → end`；
//! - 锦囊/无懈/响应/濒死/弃牌/判定全部在 `resolve.rs` 的结算栈里完成；
//! - 本文件向 `resolve` 暴露共用小工具（牌名、手牌操作、摸牌、距离）。
//! - P4.0 军争牌已入牌堆定义（`data.rs`），但酒/属性杀/火攻/铁索/兵粮寸断与军争装备效果
//!   的结算在 P4.2–P4.3 完成；在此之前这些牌**不可主动使用**（宁可不出，也不得静默按普通牌结算）。

use crate::data;
use crate::resolve;
use crate::state::*;
use crate::{illegal, internal};
use game_engine_core::{Action, EngineError, EngineEvent};
use serde_json::json;

/// P4.0 已入牌堆定义、结算待后续阶段的军争牌（`legal_actions` 与 `play_card` 双重拦截）。
/// P4.3 军争牌已全部接入结算（保留空表以便后续新牌扩展）。
pub(crate) const PENDING_JUNZHENG_CARDS: &[&str] = &[];

// ---------------------------------------------------------------------------
// 共用工具（resolve 使用）
// ---------------------------------------------------------------------------

pub(crate) fn card_def(st: &SgsState, card_id: &str) -> Option<&'static data::CardDef> {
    let c = st.cards.get(card_id)?;
    data::card_def(&c.def)
}

pub(crate) fn card_label(st: &SgsState, card_id: &str) -> String {
    let Some(c) = st.cards.get(card_id) else {
        return card_id.to_string();
    };
    let name = card_def(st, card_id)
        .map(|d| d.name)
        .unwrap_or(c.def.as_str());
    let suit = match c.suit {
        Suit::Spade => "♠",
        Suit::Heart => "♥",
        Suit::Club => "♣",
        Suit::Diamond => "♦",
    };
    let rank = match c.rank {
        1 => "A".to_string(),
        11 => "J".to_string(),
        12 => "Q".to_string(),
        13 => "K".to_string(),
        n => n.to_string(),
    };
    format!("{name}{suit}{rank}")
}

pub(crate) fn hand_ids(st: &SgsState, idx: usize) -> Vec<String> {
    st.seats
        .get(idx)
        .map(|s| s.hand.clone())
        .unwrap_or_default()
}

pub(crate) fn hand_cards_with_def(st: &SgsState, idx: usize, code: &str) -> Vec<String> {
    hand_ids(st, idx)
        .into_iter()
        .filter(|id| card_def(st, id).is_some_and(|d| d.code == code))
        .collect()
}

pub(crate) fn take_card(st: &mut SgsState, idx: usize, card_id: &str) -> Result<(), EngineError> {
    let hand = &mut st.seats[idx].hand;
    let pos = hand.iter().position(|c| c == card_id).ok_or_else(|| {
        eprintln!(
            "BT:
{}",
            std::backtrace::Backtrace::force_capture()
        );
        illegal(format!(
            "手牌中没有该牌 DEBUG idx={idx} hand={hand:?} card={card_id}"
        ))
    })?;
    hand.remove(pos);
    // 连营：失去最后的手牌后摸 1 张（自动）。
    crate::skills::lianying_on_lose_last(st, idx);
    // 屯田：回合外失去牌后判定。
    if st.turn != idx {
        crate::skills::tuotian_on_lose(st, idx);
    }
    Ok(())
}

pub(crate) fn to_discard_card(st: &mut SgsState, card_id: &str) {
    st.deck.discard.push(card_id.to_string());
}

/// 是否有「顺手/过河」可拿的牌（手牌 / 装备 / 判定区至少其一）。
pub(crate) fn has_cards_to_take(st: &SgsState, idx: usize) -> bool {
    let Some(seat) = st.seats.get(idx) else {
        return false;
    };
    !seat.hand.is_empty()
        || !seat.judge_zone.is_empty()
        || seat.equips.values().any(|v| v.is_some())
}

pub(crate) fn judge_zone_has(st: &SgsState, idx: usize, def_code: &str) -> bool {
    st.seats
        .get(idx)
        .map(|s| {
            s.judge_zone
                .iter()
                .any(|id| st.effective_def_code(id) == Some(def_code))
        })
        .unwrap_or(false)
}

/// 出牌阶段【杀】次数上限（咆哮 / 诸葛连弩 → 无限）。
pub(crate) fn sha_limit(st: &SgsState, idx: usize) -> u32 {
    // 天义（没赢）：本回合不能使用【杀】。
    if crate::skills::sha_blocked(st, idx) {
        return 0;
    }
    if crate::skills::has_equipment_skill(st, idx, "zhuge_liannu")
        || crate::skills::has(st, idx, "paoxiao")
    {
        u32::MAX
    } else {
        // 奇谋/天义：本回合【杀】限制次数 +X。
        1 + st.counter("qimou") + st.counter("tianyi")
    }
}

pub(crate) fn sha_targets(st: &SgsState, idx: usize) -> Vec<usize> {
    st.alive_indices()
        .into_iter()
        .filter(|t| {
            *t != idx
                && st.in_attack_range(idx, *t)
                // 空城（锁定）：无手牌时不能成为【杀】目标。
                && !crate::skills::kongcheng_blocks(st, *t)
        })
        .collect()
}

/// 计算与其他角色的有效距离（含马术/奇谋/对方防御马；用于技能判定）。
pub(crate) fn effective_distance(st: &SgsState, from: usize, to: usize) -> i32 {
    let d = st.seat_distance(from, to);
    if d == usize::MAX {
        return i32::MAX;
    }
    d as i32 + st.defense_bonus(to)
        - i32::from(st.equip_def(from, EquipSlot::HorseOffense).is_some())
        - st.mashu_bonus(from)
        - crate::skills::qimou_bonus(st, from)
        - crate::skills::tuotian_bonus(st, from)
}

/// 顺手牵羊等「不含武器范围」的距离：`seat_distance + 防御马 - 进攻马 ≤ 1`。
pub(crate) fn trick_distance_ok(st: &SgsState, from: usize, to: usize) -> bool {
    if crate::skills::trick_distance_unlimited(st, from) {
        return true;
    }
    effective_distance(st, from, to) <= 1
}

pub(crate) fn refill_deck(st: &mut SgsState, events: &mut Vec<EngineEvent>) {
    if !st.deck.draw.is_empty() || st.deck.discard.is_empty() {
        return;
    }
    let mut discard = std::mem::take(&mut st.deck.discard);
    st.shuffle(&mut discard);
    st.deck.draw = discard;
    events.push(EngineEvent::new(
        "deck_refilled",
        json!({ "size": st.deck.draw.len() }),
    ));
}

pub(crate) fn draw_one(st: &mut SgsState, events: &mut Vec<EngineEvent>) -> Option<String> {
    refill_deck(st, events);
    st.deck.draw.pop()
}

pub(crate) fn draw_cards(
    st: &mut SgsState,
    idx: usize,
    n: u32,
    events: &mut Vec<EngineEvent>,
) -> usize {
    let mut drawn = 0;
    for _ in 0..n {
        let Some(card) = draw_one(st, events) else {
            break;
        };
        st.seats[idx].hand.push(card);
        drawn += 1;
    }
    drawn
}

pub(crate) fn next_judge_lightning_target(st: &SgsState, owner: usize) -> Option<usize> {
    st.seats_from_left_of(owner)
        .into_iter()
        .find(|i| st.seats[*i].alive && !judge_zone_has(st, *i, "shandian"))
}

// ---------------------------------------------------------------------------
// 阶段机
// ---------------------------------------------------------------------------

/// 栈为空时推进一个阶段；返回 `true` 表示可以继续循环，`false` 表示等待玩家输入。
pub(crate) fn phase_step(
    st: &mut SgsState,
    events: &mut Vec<EngineEvent>,
) -> Result<bool, EngineError> {
    if st.phase != Phase::Turn {
        return Ok(false);
    }
    if !st.seats.get(st.turn).is_some_and(|s| s.alive) {
        st.turn_phase = TurnPhase::End;
    }
    match st.turn_phase {
        TurnPhase::Prepare => {
            let idx = st.turn;
            // 翻面：跳过本回合，随后翻回正面（解围可在此发动）。
            if st.seats[idx].flipped {
                st.seats[idx].flipped = false;
                st.set_counter("skip_turn", 1);
                let pid = st.seats[idx].player_id.clone();
                st.push_log(Some(&pid), "翻面：跳过本回合");
                events.push(EngineEvent::new(
                    "turn_skipped",
                    json!({ "seat": idx, "player": pid }),
                ));
                if crate::skills::has(st, idx, "jiewei") {
                    resolve::push(
                        st,
                        resolve::Frame::Jiewei(crate::resolve_wind::JieweiFrame {
                            actor: idx,
                            stage: crate::resolve_wind::JieweiStage::Ask,
                        }),
                    );
                    return Ok(true);
                }
                end_turn(st, events);
                return Ok(true);
            }
            if st.counter("skip_turn") > 0 {
                st.set_counter("skip_turn", 0);
                end_turn(st, events);
                return Ok(true);
            }
            let pid = st.seats[idx].player_id.clone();
            st.push_log(Some(&pid), "准备阶段");
            events.push(EngineEvent::new(
                "phase_start",
                json!({ "phase": "prepare", "seat": st.turn, "player": pid }),
            ));
            // 观星（P2.5）：准备阶段自动进入特殊区 `guanxing`（不可取消，见 special-zones §5）。
            if crate::skills::has(st, st.turn, "guanxing") && st.counter("guanxing_done") == 0 {
                st.set_counter("guanxing_done", 1);
                resolve::push(st, crate::resolve_zone::start_guanxing(st.turn));
            }
            // 洛神（P4.1-C2）：准备阶段可反复判定黑色获得。
            if crate::skills::has(st, st.turn, "luoshen") && st.counter("luoshen_done") == 0 {
                st.set_counter("luoshen_done", 1);
                resolve::push(
                    st,
                    resolve::Frame::Luoshen(crate::resolve_more::LuoshenFrame { actor: st.turn }),
                );
            }
            // 神速：准备阶段可选（至多三项），每项视为使用一张无距离限制的【杀】。
            if crate::skills::has(st, st.turn, "shensu") && st.counter("shensu_done") == 0 {
                st.set_counter("shensu_done", 1);
                resolve::push(
                    st,
                    resolve::Frame::Shensu(crate::resolve_wind::ShensuFrame {
                        actor: st.turn,
                        stage: crate::resolve_wind::ShensuStage::Ask,
                        used: 0,
                    }),
                );
            }
            // 觉醒（凿险/志继/魂姿/若愚）。
            crate::resolve_mountain::awaken_triggers(st);
            // 巧变：准备阶段选择跳过的阶段（每回合一次）。
            if crate::skills::has(st, st.turn, "qiaobian") && st.counter("qiaobian_offered") == 0 {
                st.set_counter("qiaobian_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::Qiaobian(crate::resolve_mountain::QiaobianFrame {
                        actor: st.turn,
                        remaining: 0,
                    }),
                );
            }
            // 化身：回合开始时替换展示的化身牌。
            if crate::skills::has(st, st.turn, "huashen") && st.counter("huashen_prepare") == 0 {
                st.set_counter("huashen_prepare", 1);
                resolve::push(
                    st,
                    resolve::Frame::Huashen(crate::resolve_mountain::HuashenFrame {
                        actor: st.turn,
                    }),
                );
            }
            // 英魂：准备阶段若已受伤，可选一名其他角色二选一。
            if crate::skills::has(st, st.turn, "yinghun")
                && st.seats[st.turn].hp < st.seats[st.turn].max_hp
            {
                resolve::push(
                    st,
                    resolve::Frame::Yinghun(crate::resolve_forest::YinghunFrame {
                        actor: st.turn,
                        target: None,
                        stage: crate::resolve_forest::YinghunStage::Target,
                    }),
                );
            }
            st.turn_phase = TurnPhase::Judge;
            Ok(true)
        }
        TurnPhase::Judge => {
            // 巧变：跳过判定阶段。
            if st.counter("qiaobian_skip_judge") > 0 {
                st.set_counter("qiaobian_skip_judge", 0);
                let pid = st.seats[st.turn].player_id.clone();
                st.push_log(Some(&pid), "巧变：跳过判定阶段");
                st.turn_phase = TurnPhase::Draw;
                return Ok(true);
            }
            // 神速①：跳过判定阶段（判定区牌保留）。
            if st.counter("shensu_skip_judge") > 0 {
                let pid = st.seats[st.turn].player_id.clone();
                st.push_log(Some(&pid), "神速：跳过判定阶段");
                st.turn_phase = TurnPhase::Draw;
                return Ok(true);
            }
            // 判定区的栈顶在 `Judge` 帧里结算（含判定前的无懈窗口）。
            let top = st.seats[st.turn].judge_zone.last().cloned();
            match top {
                Some(delayed) => {
                    let pos = st.seats[st.turn]
                        .judge_zone
                        .iter()
                        .rposition(|c| c == &delayed)
                        .expect("judge card");
                    st.seats[st.turn].judge_zone.remove(pos);
                    // 判定中的牌计入处理区，保证牌数守恒
                    st.deck.processing.push(delayed.clone());
                    resolve::push_judge(st, delayed, st.turn);
                    Ok(true)
                }
                None => {
                    st.turn_phase = TurnPhase::Draw;
                    Ok(true)
                }
            }
        }
        TurnPhase::Draw => {
            let idx = st.turn;
            // 突袭：放弃摸牌，改为获得至多两名角色各一张手牌
            if crate::skills::has(st, idx, "tuxi") && st.counter("tuxi_offered") == 0 {
                st.set_counter("tuxi_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::TuxiOffer(crate::resolve_more::TuxiOfferFrame { actor: idx }),
                );
                return Ok(true);
            }
            if st.counter("skip_draw") > 0 {
                st.set_counter("skip_draw", 0);
                let reason = if crate::skills::has(st, idx, "tuxi") {
                    "突袭：跳过摸牌"
                } else {
                    "跳过摸牌阶段"
                };
                st.push_log(Some(&st.seats[idx].player_id.clone()), reason);
                crate::skills::jiezi_on_skip_draw(st, idx);
                st.turn_phase = TurnPhase::Play;
                return Ok(true);
            }
            // 再起（可选）：已受伤时改为亮 X 张、红桃回血并获得亮出牌。
            if crate::skills::has(st, idx, "zaizhi")
                && st.seats[idx].hp < st.seats[idx].max_hp
                && st.counter("zaizhi_offered") == 0
            {
                st.set_counter("zaizhi_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::Zaizhi(crate::resolve_forest::ZaizhiFrame { actor: idx }),
                );
                return Ok(true);
            }
            // 好施（可选）：摸牌阶段多摸 2 张（之后若手牌>5 需交一半）。
            if crate::skills::has(st, idx, "haoshi") && st.counter("haoshi_offered") == 0 {
                st.set_counter("haoshi_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::HaoshiOffer(crate::resolve_forest::HaoshiOfferFrame {
                        actor: idx,
                    }),
                );
                return Ok(true);
            }
            // 双雄（可选）：改为判定并获得判定牌（本回合异色手牌当【决斗】）。
            if crate::skills::has(st, idx, "shuangxiong") && st.counter("shuangxiong_offered") == 0
            {
                st.set_counter("shuangxiong_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::Shuangxiong(crate::resolve_fire::ShuangxiongFrame {
                        actor: idx,
                    }),
                );
                return Ok(true);
            }
            // 裸衣（可选）：摸牌阶段先询问是否少摸 1 张换本回合杀/决斗伤害 +1。
            if crate::skills::has(st, idx, "luoyi") && st.counter("luoyi_offered") == 0 {
                st.set_counter("luoyi_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::Luoyi(crate::resolve_more::LuoyiFrame { actor: idx }),
                );
                return Ok(true);
            }
            // 英姿：摸牌阶段多摸 1 张（锁定）；裸衣发动时少摸 1 张。
            let mut draw_n = DRAW_PER_TURN;
            if crate::skills::has(st, idx, "yingzi") {
                draw_n += 1;
            }
            if st.counter("luoyi") > 0 {
                draw_n = draw_n.saturating_sub(1);
            }
            if st.counter("haoshi") > 0 {
                draw_n += 2;
            }
            let drawn = draw_cards(st, idx, draw_n, events);
            let pid = st.seats[idx].player_id.clone();
            st.push_log(Some(&pid), format!("摸牌阶段：摸 {drawn} 张"));
            events.push(EngineEvent::new(
                "phase_start",
                json!({ "phase": "draw", "seat": idx, "player": pid, "drawn": drawn }),
            ));
            st.turn_phase = TurnPhase::Play;
            let over = st.seats[idx]
                .hand
                .len()
                .saturating_sub(st.config.max_hand_cards as usize);
            if over > 0 {
                resolve::push_discard(st, idx, over, "hand_cap");
            }
            // 好施：手牌 >5 时须将一半交给手牌最少的一名其他角色。
            if st.counter("haoshi") > 0 && st.seats[idx].hand.len() > 5 {
                resolve::push(
                    st,
                    resolve::Frame::Haoshi(crate::resolve_forest::HaoshiFrame {
                        actor: idx,
                        stage: crate::resolve_forest::HaoshiStage::Select,
                        selected: Vec::new(),
                    }),
                );
            }
            Ok(true)
        }
        TurnPhase::Play => {
            if st.counter("skip_play") > 0 {
                let pid = st.seats[st.turn].player_id.clone();
                st.set_counter("skip_play", 0);
                st.phase_discarded.clear();
                st.push_log(Some(&pid), "出牌阶段被跳过");
                events.push(EngineEvent::new(
                    "phase_skipped",
                    json!({ "phase": "play", "player": pid }),
                ));
                st.turn_phase = TurnPhase::Discard;
                return Ok(true);
            }
            let pid = st.seats[st.turn].player_id.clone();
            events.push(EngineEvent::new(
                "phase_start",
                json!({ "phase": "play", "seat": st.turn, "player": pid }),
            ));
            // 等待玩家操作
            Ok(false)
        }
        TurnPhase::Discard => {
            let idx = st.turn;
            // 巧变：跳过弃牌阶段。
            if st.counter("qiaobian_skip_discard") > 0 {
                st.set_counter("qiaobian_skip_discard", 0);
                let pid = st.seats[idx].player_id.clone();
                st.push_log(Some(&pid), "巧变：跳过弃牌阶段");
                st.turn_phase = TurnPhase::End;
                return Ok(true);
            }
            // 神速③：跳过弃牌阶段。
            if st.counter("shensu_skip_discard") > 0 {
                st.set_counter("shensu_skip_discard", 0);
                let pid = st.seats[idx].player_id.clone();
                st.push_log(Some(&pid), "神速：跳过弃牌阶段");
                st.turn_phase = TurnPhase::End;
                return Ok(true);
            }
            // 克己：出牌阶段未使用杀 → 跳过弃牌
            let limit = crate::skills::hand_limit(st, idx);
            let over = st.seats[idx].hand.len().saturating_sub(limit);
            if over > 0 {
                resolve::push_discard(st, idx, over, "discard_phase");
                // 帧结算完后会再次进入本阶段
                return Ok(true);
            }
            // 固政：其他角色弃牌阶段结束时，可令其获得一张弃牌并取得剩余。
            if st.counter("guzheng_done") == 0 {
                if let Some(owner) = crate::resolve_mountain::guzheng_owner(st, idx) {
                    if !st.phase_discarded.is_empty() {
                        st.set_counter("guzheng_done", 1);
                        let cards = st.phase_discarded.clone();
                        resolve::push(
                            st,
                            resolve::Frame::Guzheng(crate::resolve_mountain::GuzhengFrame {
                                owner,
                                discarder: idx,
                                cards,
                            }),
                        );
                        return Ok(true);
                    }
                }
            }
            st.phase_discarded.clear();
            st.turn_phase = TurnPhase::End;
            Ok(true)
        }
        TurnPhase::End => {
            let pid = st.seats[st.turn].player_id.clone();
            events.push(EngineEvent::new(
                "phase_start",
                json!({ "phase": "end", "seat": st.turn, "player": pid }),
            ));
            // 化身：回合结束时替换展示的化身牌。
            if crate::skills::has(st, st.turn, "huashen") && st.counter("huashen_end") == 0 {
                st.set_counter("huashen_end", 1);
                resolve::push(
                    st,
                    resolve::Frame::Huashen(crate::resolve_mountain::HuashenFrame {
                        actor: st.turn,
                    }),
                );
                return Ok(true);
            }
            // 放权：本回合结束时，令一名其他角色执行额外回合。
            if st.seats[st.turn].flags.contains_key("fangquan_pending")
                && st.counter("fangquan_done") == 0
            {
                st.set_counter("fangquan_done", 1);
                resolve::push(
                    st,
                    resolve::Frame::Fangquan(crate::resolve_mountain::FangquanFrame {
                        actor: st.turn,
                        stage: crate::resolve_mountain::FangquanStage::Target,
                    }),
                );
                return Ok(true);
            }
            // 崩坏：结束阶段若你不是体力值最小的角色，失 1 体力或减 1 体力上限。
            if crate::skills::has(st, st.turn, "bengkuai")
                && st.counter("bengkuai_done") == 0
                && st
                    .alive_indices()
                    .into_iter()
                    .any(|i| st.seats[i].hp < st.seats[st.turn].hp)
            {
                st.set_counter("bengkuai_done", 1);
                resolve::push(
                    st,
                    resolve::Frame::Bengkuai(crate::resolve_forest::BengkuaiFrame {
                        actor: st.turn,
                    }),
                );
                return Ok(true);
            }
            // 据守：结束阶段可选择翻面并摸 4 张，然后弃置 1 张手牌。
            if crate::skills::has(st, st.turn, "jushou") && st.counter("jushou_done") == 0 {
                st.set_counter("jushou_done", 1);
                resolve::push(
                    st,
                    resolve::Frame::Jushou(crate::resolve_wind::JushouFrame { actor: st.turn }),
                );
                return Ok(true);
            }
            crate::skills::on_phase_start(st, TurnPhase::End, events);
            end_turn(st, events);
            Ok(true)
        }
    }
}

fn end_turn(st: &mut SgsState, events: &mut Vec<EngineEvent>) {
    let cur = st.turn;
    let pid = st.seats[cur].player_id.clone();
    st.push_log(Some(&pid), "结束阶段");
    events.push(EngineEvent::new(
        "turn_end",
        json!({ "seat": cur, "player": pid }),
    ));
    st.turn_counters.clear();
    st.phase_discarded.clear();
    st.special_zones.remove("judge");
    // 特殊区（观星/五谷）必须在回合内结算完毕；收尾清理防止残留投影。
    st.special_zones.remove(crate::resolve_zone::GUANXING_ZONE);
    st.special_zones.remove(crate::resolve_zone::WUGU_ZONE);

    let alive = st.alive_indices();
    if alive.len() <= 1 {
        st.phase = Phase::Finished;
        return;
    }
    let mut next = alive
        .iter()
        .copied()
        .find(|i| *i > cur)
        .unwrap_or_else(|| alive[0]);
    if let Some(extra) = st.extra_turn.take() {
        if extra != cur && st.seats.get(extra).is_some_and(|s| s.alive) {
            next = extra;
        }
    }
    if Some(next) == st.lord_index() {
        st.round += 1;
    }
    st.turn = next;
    st.turn_index += 1;
    st.turn_phase = TurnPhase::Prepare;
    st.pending = None;
    st.pending_queue.clear();
    let next_pid = st.seats[next].player_id.clone();
    events.push(EngineEvent::new(
        "turn_started",
        json!({ "round": st.round, "seat": next, "player": next_pid }),
    ));
}

/// 公开判定区（特殊区 `judge`）：亮牌即公开；改判/结算后更新 `judge_card` 与 `effect`。
pub(crate) fn publish_judge_zone(
    st: &mut SgsState,
    delayed: &str,
    owner: usize,
    judge_id: &str,
    effect: &str,
) {
    let owner_pid = st.seats[owner].player_id.clone();
    let mut payload = serde_json::Map::new();
    payload.insert("owner".into(), json!(owner_pid));
    payload.insert("delayed".into(), crate::view::card_view(st, delayed));
    payload.insert("judge_card".into(), crate::view::card_view(st, judge_id));
    payload.insert("effect".into(), json!(effect));
    st.special_zones.insert(
        "judge".into(),
        SpecialZone {
            owner: None,
            visibility: Visibility::Public,
            payload,
        },
    );
}

/// 亮出判定牌：抽一张进处理区（牌数守恒）并公开；改判窗口见 `resolve::JudgeStage`。
pub(crate) fn reveal_judge_card(
    st: &mut SgsState,
    delayed: &str,
    owner: usize,
    events: &mut Vec<EngineEvent>,
) -> Result<String, EngineError> {
    let judge_id = draw_one(st, events).ok_or_else(|| internal("牌堆不可用，无法判定"))?;
    st.deck.processing.push(judge_id.clone());
    publish_judge_zone(st, delayed, owner, &judge_id, "亮牌（待改判）");
    Ok(judge_id)
}

/// 判定生效：按最终判定牌（可能已被改判）结算延时锦囊。
pub(crate) fn apply_judge_effect(
    st: &mut SgsState,
    delayed: String,
    owner: usize,
    judge_id: String,
    events: &mut Vec<EngineEvent>,
) -> Result<(), EngineError> {
    // 生效定义（国色等转化牌优先），用于结算延时锦囊效果。
    let delayed_code = st
        .effective_def_code(&delayed)
        .map(str::to_string)
        .ok_or_else(|| internal("判定区牌缺少定义"))?;
    let delayed_def = delayed_code.as_str();
    let delayed_name = data::card_def(delayed_def)
        .map(|d| d.name)
        .unwrap_or("延时锦囊");

    let judge_card = st
        .cards
        .get(&judge_id)
        .cloned()
        .ok_or_else(|| internal("判定牌缺少实例"))?;
    let owner_pid = st.seats[owner].player_id.clone();
    let owner_name = st.seats[owner].general_name.clone();

    let drop_processing = |st: &mut SgsState| {
        st.deck.processing.retain(|c| c != &delayed);
    };
    let effect: &str = match delayed_def {
        "lebu_sishu" => {
            if judge_card.suit != Suit::Heart {
                st.set_counter("skip_play", 1);
                "非红桃，跳过出牌阶段"
            } else {
                "红桃，未生效"
            }
        }
        "bingliang_cunduan" => {
            if judge_card.suit != Suit::Spade {
                st.set_counter("skip_draw", 1);
                "非黑桃，跳过摸牌阶段"
            } else {
                "黑桃，未生效"
            }
        }
        "shandian" => {
            let hit = judge_card.suit == Suit::Spade && (2..=9).contains(&judge_card.rank);
            if hit {
                "黑桃 2–9，受到 3 点雷电伤害"
            } else if next_judge_lightning_target(st, owner).is_some() {
                "未命中，移动到下家判定区"
            } else {
                "未命中，无可用下家，置入弃牌堆"
            }
        }
        _ => "未实现的延时锦囊，直接弃置",
    };

    // 判定牌公开（特殊区 judge；保留到回合结束）
    publish_judge_zone(st, &delayed, owner, &judge_id, effect);
    // 天妒：判定牌生效后获得之（自动；拿走则不再进弃牌堆）。
    let judge_gained = crate::skills::tiandu_gain(st, owner, &judge_id);
    // 颂威：其他魏势力角色的黑色判定牌生效后，主公曹丕摸 1。
    crate::skills::songwei_on_judge(st, owner, &judge_id);

    if delayed_def == "shandian" {
        let hit = judge_card.suit == Suit::Spade && (2..=9).contains(&judge_card.rank);
        if hit {
            drop_processing(st);
            to_discard_card(st, &delayed);
            if !judge_gained {
                resolve::discard_from_processing(st, &judge_id);
            }
            st.push_log(
                Some(&owner_pid),
                format!(
                    "判定【{delayed_name}】：{}，{effect}",
                    card_label(st, &judge_id)
                ),
            );
            events.push(EngineEvent::new(
                "judge",
                json!({
                    "owner": owner_pid,
                    "delayed": delayed_def,
                    "card": judge_id,
                    "suit": judge_card.suit,
                    "rank": judge_card.rank,
                    "effect": effect,
                }),
            ));
            resolve::apply_damage(st, owner, 3, None, resolve::DamageType::Thunder, None);
            return Ok(());
        }
        drop_processing(st);
        if let Some(next) = next_judge_lightning_target(st, owner) {
            st.seats[next].judge_zone.push(delayed);
        } else {
            to_discard_card(st, &delayed);
        }
    } else {
        drop_processing(st);
        to_discard_card(st, &delayed);
    }
    if !judge_gained {
        resolve::discard_from_processing(st, &judge_id);
    }

    st.push_log(
        Some(&owner_pid),
        format!(
            "判定【{delayed_name}】（{owner_name}）：{}，{effect}",
            card_label(st, &judge_id)
        ),
    );
    events.push(EngineEvent::new(
        "judge",
        json!({
            "owner": owner_pid,
            "delayed": delayed_def,
            "card": judge_id,
            "suit": judge_card.suit,
            "rank": judge_card.rank,
            "effect": effect,
        }),
    ));
    Ok(())
}

// ---------------------------------------------------------------------------
// 合法动作
// ---------------------------------------------------------------------------

pub fn legal_actions(st: &SgsState, player_id: &str) -> Vec<Action> {
    let Some(idx) = st.seat_index(player_id) else {
        return Vec::new();
    };
    if let Some(p) = &st.pending {
        if p.actor != player_id {
            return Vec::new();
        }
        return p
            .options
            .iter()
            .map(|o| Action {
                id: o.id.clone(),
                description: o.label.clone(),
            })
            .collect();
    }
    if st.phase != Phase::Turn || idx != st.turn || st.turn_phase != TurnPhase::Play {
        return Vec::new();
    }

    let mut out = Vec::new();
    let sha_limit = sha_limit(st, idx);
    let targets = sha_targets(st, idx);
    for card_id in hand_ids(st, idx) {
        let Some(def) = card_def(st, &card_id) else {
            continue;
        };
        let playable = match def.code {
            // P4.0：军争牌定义已入库，结算未实现 → 不出现在合法动作里（避免驱动脚本选中即内部错误）。
            code if PENDING_JUNZHENG_CARDS.contains(&code) => false,
            "sha" | "huo_sha" | "lei_sha" => {
                st.counter("sha_used") < sha_limit && !targets.is_empty()
            }
            // 酒：每回合限一次（本回合下一张【杀】伤害 +1）。
            "jiu" => st.counter("jiu_used") == 0,
            "tao" => st.seats[idx].hp < st.seats[idx].max_hp,
            "shan" | "wuxie_keji" => false,
            // 五谷丰登（P2.5）：亮牌 → 释放者先选 → 座位顺序
            "wugu_fengdeng" => true,
            // 铁索连环：可选其他存活角色（横置/重置）
            "tiesuo_lianhuan" => st.alive_indices().into_iter().any(|t| t != idx),
            // 火攻：目标须有手牌
            "huo_gong" => st
                .alive_indices()
                .into_iter()
                .any(|t| t != idx && !st.seats[t].hand.is_empty()),
            "wuzhong_shengyou" => true,
            "shunshou_qianyang" => st.alive_indices().into_iter().any(|t| {
                t != idx
                    && !crate::skills::qianxun_blocks(st, t)
                    && trick_distance_ok(st, idx, t)
                    && has_cards_to_take(st, t)
            }),
            "guohe_chaiqiao" => st.alive_indices().into_iter().any(|t| {
                t != idx && !crate::skills::qianxun_blocks(st, t) && has_cards_to_take(st, t)
            }),
            "juedou" => st
                .alive_indices()
                .into_iter()
                // 空城（锁定）：无手牌者不能成为【决斗】目标。
                .any(|t| t != idx && !crate::skills::kongcheng_blocks(st, t)),
            "lebu_sishu" => st
                .alive_indices()
                .into_iter()
                .any(|t| t != idx && !judge_zone_has(st, t, "lebu_sishu")),
            "shandian" => !judge_zone_has(st, idx, "shandian"),
            "bingliang_cunduan" => st.alive_indices().into_iter().any(|t| {
                t != idx
                    && trick_distance_ok(st, idx, t)
                    && !judge_zone_has(st, t, "bingliang_cunduan")
            }),
            "jiedao_sharen" => st.alive_indices().into_iter().any(|a| {
                a != idx
                    && st.equip_def(a, EquipSlot::Weapon).is_some()
                    && st
                        .alive_indices()
                        .into_iter()
                        // 空城（锁定）：借刀杀人迫使的【杀】也不能指定无手牌的空城角色。
                        .any(|b| {
                            b != a
                                && st.in_attack_range(a, b)
                                && !crate::skills::kongcheng_blocks(st, b)
                        })
            }),
            _ if def.category == Category::Trick || def.category == Category::DelayedTrick => {
                st.alive_indices().into_iter().any(|t| t != idx)
            }
            _ if def.equip_slot.is_some() => true,
            _ => false,
        };
        if playable {
            out.push(Action {
                id: format!("play:{card_id}"),
                description: format!("使用【{}】", card_label(st, &card_id)),
            });
        }
    }
    // 主动技能：制衡（每出牌阶段一次，至少 1 张手牌）
    if crate::skills::has(st, idx, "zhiheng")
        && st.counter("zhiheng_used") == 0
        && !st.seats[idx].hand.is_empty()
    {
        out.push(Action {
            id: "skill:zhiheng".into(),
            description: "制衡：弃置任意张手牌，摸等量的牌".into(),
        });
    }
    // 苦肉：失去 1 点体力，摸 2 张（可多次）
    if crate::skills::has(st, idx, "kurou") && st.seats[idx].hp > 0 {
        out.push(Action {
            id: "skill:kurou".into(),
            description: "苦肉：失去 1 点体力，摸 2 张牌".into(),
        });
    }
    // 青囊：每阶段一次，弃 1 张手牌令一名已受伤角色回复 1 点体力
    if crate::skills::has(st, idx, "qingnang")
        && st.counter("qingnang_used") == 0
        && !st.seats[idx].hand.is_empty()
        && st
            .alive_indices()
            .into_iter()
            .any(|i| st.seats[i].hp < st.seats[i].max_hp)
    {
        out.push(Action {
            id: "skill:qingnang".into(),
            description: "青囊：弃 1 张手牌，令一名已受伤角色回复 1 点体力".into(),
        });
    }
    // 结姻：每阶段一次，弃 2 张手牌，与一名已受伤男性各回复 1 点体力
    if crate::skills::has(st, idx, "jieyin")
        && st.counter("jieyin_used") == 0
        && st.seats[idx].hand.len() >= 2
        && st.alive_indices().into_iter().any(|i| {
            i != idx && crate::skills::is_male(st, i) && st.seats[i].hp < st.seats[i].max_hp
        })
    {
        out.push(Action {
            id: "skill:jieyin".into(),
            description: "结姻：弃 2 张手牌，与一名已受伤男性各回复 1 点体力".into(),
        });
    }
    // 蛊惑：每回合限一次，扣置一张手牌当任意基本牌/普通锦囊使用。
    if crate::resolve_wind::guhuo_available(st, idx) {
        out.push(Action {
            id: "skill:guhuo".into(),
            description: "蛊惑：扣置一张手牌当任意基本牌或普通锦囊使用".into(),
        });
    }
    // 强袭：出牌阶段限两次，失 1 体力或弃武器 → 对未指定过的角色 1 伤。
    if crate::skills::has(st, idx, "qiangxi")
        && st.counter("qiangxi_used") < 2
        && (st.seats[idx].hp > 0 || st.equip_def(idx, EquipSlot::Weapon).is_some())
        && !crate::resolve_fire::qiangxi_targets(st, idx).is_empty()
    {
        out.push(Action {
            id: "skill:qiangxi".into(),
            description: "强袭：失去 1 点体力或弃武器，对一名角色造成 1 点伤害".into(),
        });
    }
    // 驱虎：出牌阶段限一次，与体力大于你的角色拼点。
    if crate::skills::has(st, idx, "quhu")
        && st.counter("quhu_used") == 0
        && !st.seats[idx].hand.is_empty()
        && st
            .alive_indices()
            .into_iter()
            .any(|t| t != idx && st.seats[t].hp > st.seats[idx].hp && !st.seats[t].hand.is_empty())
    {
        out.push(Action {
            id: "skill:quhu".into(),
            description: "驱虎：与体力值大于你的角色拼点".into(),
        });
    }
    // 天义：出牌阶段限一次，与一名有手牌的角色拼点。
    if crate::skills::has(st, idx, "tianyi")
        && st.counter("tianyi_used") == 0
        && !st.seats[idx].hand.is_empty()
        && st
            .alive_indices()
            .into_iter()
            .any(|t| t != idx && !st.seats[t].hand.is_empty())
    {
        out.push(Action {
            id: "skill:tianyi".into(),
            description: "天义：拼点；赢则本回合杀次数+1/无距离/多目标".into(),
        });
    }
    // 连环：梅花手牌当【铁索连环】使用。
    if crate::skills::has(st, idx, "lianhuan") && st.alive_indices().into_iter().any(|t| t != idx) {
        for card_id in hand_ids(st, idx) {
            if st.cards.get(&card_id).is_some_and(|c| c.suit == Suit::Club) {
                out.push(Action {
                    id: format!("skill:lianhuan:{card_id}"),
                    description: format!(
                        "连环：将【{}】当【铁索连环】使用",
                        card_label(st, &card_id)
                    ),
                });
            }
        }
    }
    // 火计：红色手牌当【火攻】使用。
    if crate::skills::has(st, idx, "huoji")
        && st
            .alive_indices()
            .into_iter()
            .any(|t| t != idx && !st.seats[t].hand.is_empty())
    {
        for card_id in hand_ids(st, idx) {
            if st
                .cards
                .get(&card_id)
                .is_some_and(|c| crate::skills::is_red(c.suit))
            {
                out.push(Action {
                    id: format!("skill:huoji:{card_id}"),
                    description: format!("火计：将【{}】当【火攻】使用", card_label(st, &card_id)),
                });
            }
        }
    }
    // 双雄：本回合异色手牌当【决斗】使用。
    if crate::skills::has(st, idx, "shuangxiong")
        && st
            .alive_indices()
            .into_iter()
            .any(|t| t != idx && !crate::skills::kongcheng_blocks(st, t))
    {
        for card_id in hand_ids(st, idx) {
            if crate::resolve_fire::shuangxiong_conversion(st, idx, &card_id).is_some() {
                out.push(Action {
                    id: format!("skill:shuangxiong:{card_id}"),
                    description: format!("双雄：将【{}】当【决斗】使用", card_label(st, &card_id)),
                });
            }
        }
    }
    // 乱击：两张同花色手牌当【万箭齐发】使用。
    if crate::skills::has(st, idx, "luanji") && st.alive_indices().len() > 1 {
        let hand = hand_ids(st, idx);
        for i in 0..hand.len() {
            for j in (i + 1)..hand.len() {
                let same = st.cards.get(&hand[i]).map(|c| c.suit)
                    == st.cards.get(&hand[j]).map(|c| c.suit);
                if same {
                    out.push(Action {
                        id: format!("skill:luanji:{},{}", hand[i], hand[j]),
                        description: format!(
                            "乱击：将【{}】【{}】当【万箭齐发】使用",
                            card_label(st, &hand[i]),
                            card_label(st, &hand[j])
                        ),
                    });
                }
            }
        }
    }
    // 断粮：黑色基本牌/装备牌当【兵粮寸断】；对牌数≥你的目标无距离限制。
    if crate::skills::has(st, idx, "duanliang") {
        for card_id in hand_ids(st, idx) {
            let inst = st.cards.get(&card_id).cloned();
            let Some(inst) = inst else { continue };
            if !crate::skills::is_black(inst.suit) {
                continue;
            }
            let def = card_def(st, &card_id);
            let basic = def.is_some_and(|d| d.category == Category::Basic);
            let equip = def.is_some_and(|d| d.equip_slot.is_some());
            if !basic && !equip {
                continue;
            }
            let has_target = st.alive_indices().into_iter().any(|t| {
                t != idx
                    && !judge_zone_has(st, t, "bingliang_cunduan")
                    && (st.seats[t].hand.len() >= st.seats[idx].hand.len()
                        || trick_distance_ok(st, idx, t))
            });
            if has_target {
                out.push(Action {
                    id: format!("skill:duanliang:{card_id}"),
                    description: format!(
                        "断粮：将【{}】当【兵粮寸断】使用",
                        card_label(st, &card_id)
                    ),
                });
            }
        }
    }
    // 酒池：黑桃手牌当【酒】使用。
    if crate::skills::has(st, idx, "jiuchi") && st.counter("jiu_used") == 0 {
        for card_id in hand_ids(st, idx) {
            if st
                .cards
                .get(&card_id)
                .is_some_and(|c| c.suit == Suit::Spade)
            {
                out.push(Action {
                    id: format!("skill:jiuchi:{card_id}"),
                    description: format!("酒池：将【{}】当【酒】使用", card_label(st, &card_id)),
                });
            }
        }
    }
    // 乱武（限定技）：令所有其他角色依次杀最近或失去 1 体力。
    if crate::skills::has(st, idx, "luanwu")
        && !st.seats[idx].flags.contains_key("luanwu_used")
        && st.alive_indices().len() > 1
    {
        out.push(Action {
            id: "skill:luanwu".into(),
            description: "乱武：令所有其他角色依次杀最近或失去 1 点体力".into(),
        });
    }
    // 缔盟：出牌阶段限一次，选两名其他角色，弃 X 张后交换其手牌。
    if crate::skills::has(st, idx, "dimeng")
        && st.counter("dimeng_used") == 0
        && st.alive_indices().len() >= 3
    {
        out.push(Action {
            id: "skill:dimeng".into(),
            description: "缔盟：选择两名其他角色，弃 X 张牌后交换其手牌".into(),
        });
    }
    // 挑衅：出牌阶段限一次，令攻击范围含你的角色对你出杀，否则弃其一张牌。
    if crate::skills::has(st, idx, "tiaoxin")
        && st.counter("tiaoxin_used") == 0
        && st.alive_indices().into_iter().any(|t| {
            t != idx
                && st.in_attack_range(t, idx)
                && (!st.seats[t].hand.is_empty()
                    || !st.seats[t].judge_zone.is_empty()
                    || st.seats[t].equips.values().any(|c| c.is_some()))
        })
    {
        out.push(Action {
            id: "skill:tiaoxin".into(),
            description: "挑衅：令攻击范围含你的角色对你使用【杀】，否则弃其一张牌".into(),
        });
    }
    // 放权：跳过出牌阶段，本回合结束时可令其他角色执行额外回合。
    if crate::skills::has(st, idx, "fangquan")
        && st.counter("fangquan_used") == 0
        && st.alive_indices().len() > 1
    {
        out.push(Action {
            id: "skill:fangquan".into(),
            description: "放权：跳过出牌阶段，本回合结束时可令其他角色执行额外回合".into(),
        });
    }
    // 直谏：将一张装备手牌置入其他角色装备区，然后摸 1。
    if crate::skills::has(st, idx, "zhijian") && st.alive_indices().len() > 1 {
        for card_id in hand_ids(st, idx) {
            if card_def(st, &card_id).is_some_and(|d| d.equip_slot.is_some()) {
                out.push(Action {
                    id: format!("skill:zhijian:{card_id}"),
                    description: format!(
                        "直谏：将【{}】置入其他角色装备区并摸 1",
                        card_label(st, &card_id)
                    ),
                });
            }
        }
    }
    // 制霸（孙策主公技）：其他吴势力出牌阶段限一次，可与其拼点。
    if st.counter("zhiba_used") == 0
        && crate::skills::kingdom_is(st, idx, "wu")
        && crate::resolve_mountain::zhiba_lord(st, idx).is_some()
        && !st.seats[idx].hand.is_empty()
    {
        out.push(Action {
            id: "skill:zhiba".into(),
            description: "制霸：与孙策拼点（没赢则孙策获得两张拼点牌）".into(),
        });
    }
    // 奇谋（限定技）：失去任意点体力，本回合距离/杀次数修正。
    if crate::skills::has(st, idx, "qimou")
        && !st.seats[idx].flags.contains_key("qimou_used")
        && st.seats[idx].hp > 0
    {
        out.push(Action {
            id: "skill:qimou".into(),
            description: "奇谋：失去任意点体力，本回合距离 -X、杀次数 +X".into(),
        });
    }
    // 黄天（张角主公技）：其他群势力角色出牌阶段限一次，交给张角一张【闪】或【闪电】。
    if st.counter("huangtian_used") == 0 && crate::skills::huangtian_owner(st, idx).is_some() {
        for card_id in hand_ids(st, idx) {
            if card_def(st, &card_id).is_some_and(|d| matches!(d.code, "shan" | "shandian")) {
                out.push(Action {
                    id: format!("skill:huangtian:{card_id}"),
                    description: format!("黄天：将【{}】交给张角", card_label(st, &card_id)),
                });
            }
        }
    }
    // 重铸：铁索连环（或连环：梅花手牌）可弃置并摸 1 张。
    for card_id in hand_ids(st, idx) {
        let is_tiesuo = card_def(st, &card_id).is_some_and(|d| d.code == "tiesuo_lianhuan");
        let lianhuan_club = crate::skills::has(st, idx, "lianhuan")
            && st.cards.get(&card_id).is_some_and(|c| c.suit == Suit::Club);
        if is_tiesuo || lianhuan_club {
            out.push(Action {
                id: format!("recast:{card_id}"),
                description: format!("重铸：弃置【{}】并摸 1 张", card_label(st, &card_id)),
            });
        }
    }
    // 装备出牌阶段选项（P4.5：丈八蛇矛等，按注册表派发）。
    out.extend(crate::resolve_skill::play_phase_options(st, idx, &targets));
    // 国色：方块手牌当【乐不思蜀】
    if crate::skills::has(st, idx, "guose") {
        for card_id in hand_ids(st, idx) {
            let diamond = st
                .cards
                .get(&card_id)
                .is_some_and(|c| c.suit == Suit::Diamond);
            if !diamond {
                continue;
            }
            let has_target = st
                .alive_indices()
                .into_iter()
                .any(|t| t != idx && !judge_zone_has(st, t, "lebu_sishu"));
            if has_target {
                out.push(Action {
                    id: format!("skill:guose:{card_id}"),
                    description: format!(
                        "国色：将【{}】当【乐不思蜀】使用",
                        card_label(st, &card_id)
                    ),
                });
            }
        }
    }
    // 反间：每阶段一次，有手牌且存在其他存活角色
    if crate::skills::has(st, idx, "fanjian")
        && st.counter("fanjian_used") == 0
        && !st.seats[idx].hand.is_empty()
        && st.alive_indices().into_iter().any(|i| i != idx)
    {
        out.push(Action {
            id: "skill:fanjian".into(),
            description: "反间：令一名角色选花色，获得你一张手牌，花色不符则受 1 点伤害".into(),
        });
    }
    // 仁德：有手牌即可发动（可多次，累计 2 张回血）
    if crate::skills::has(st, idx, "rende") && !st.seats[idx].hand.is_empty() {
        out.push(Action {
            id: "skill:rende".into(),
            description: "仁德：将手牌交给其他角色".into(),
        });
    }
    // 离间：每阶段一次，需弃 1 张且至少两名男性角色
    if crate::skills::has(st, idx, "lijian")
        && st.counter("lijian_used") == 0
        && !st.seats[idx].hand.is_empty()
        && st
            .alive_indices()
            .into_iter()
            .filter(|i| *i != idx && crate::skills::is_male(st, *i))
            .count()
            >= 2
    {
        out.push(Action {
            id: "skill:lijian".into(),
            description: "离间：弃 1 张，令两名男性角色视为决斗".into(),
        });
    }
    // 奇袭：黑色手牌当过河拆桥
    if crate::skills::has(st, idx, "qixi") && st.alive_indices().len() > 1 {
        for card_id in hand_ids(st, idx) {
            if let Some(inst) = st.cards.get(&card_id) {
                if matches!(inst.suit, Suit::Spade | Suit::Club) {
                    out.push(Action {
                        id: format!("skill:qixi:{card_id}"),
                        description: format!(
                            "奇袭：将【{}】当【过河拆桥】使用",
                            card_label(st, &card_id)
                        ),
                    });
                }
            }
        }
    }
    // 武圣 / 龙胆：转化【杀】（出牌阶段主动使用；实际牌不能是【杀】本身）
    if !targets.is_empty() && st.counter("sha_used") < sha_limit {
        for card_id in hand_ids(st, idx) {
            if let Some((skill, name)) = crate::skills::conversion_for(st, idx, &card_id, "sha") {
                out.push(Action {
                    id: format!("skill:{skill}:{card_id}"),
                    description: format!("{name}：将【{}】当【杀】使用", card_label(st, &card_id)),
                });
            }
        }
    }
    out.push(Action {
        id: "end_play".into(),
        description: "结束出牌阶段".into(),
    });
    // 缠怨：体力值为 1 时不能使用或打出手牌。
    if crate::skills::chanyuan_locked(st, idx) {
        out.retain(|a| {
            !a.id.starts_with("play:")
                && !a.id.starts_with("skill:wusheng:")
                && !a.id.starts_with("skill:longdan:")
                && !a.id.starts_with("skill:guose:")
                && !a.id.starts_with("skill:qixi:")
                && !a.id.starts_with("skill:zhangba:")
                && a.id != "skill:guhuo"
        });
    }
    out
}

// ---------------------------------------------------------------------------
// 动作执行
// ---------------------------------------------------------------------------

pub fn execute(
    st: &mut SgsState,
    player_id: &str,
    action: &str,
) -> Result<(String, Vec<EngineEvent>), EngineError> {
    let mut events = Vec::new();
    let _ = st
        .seat_index(player_id)
        .ok_or_else(|| EngineError::PermissionDenied("不在座玩家".into()))?;

    if st.pending.is_some() {
        let msg = resolve::execute_pending(st, player_id, action)?;
        resolve::advance(st, &mut events)?;
        return Ok((msg, events));
    }

    if st.phase != Phase::Turn {
        return Err(illegal("当前不是出牌阶段"));
    }
    let idx = st
        .seat_index(player_id)
        .ok_or_else(|| EngineError::PermissionDenied("不在座玩家".into()))?;
    if idx != st.turn || st.turn_phase != TurnPhase::Play {
        return Err(illegal("当前不是你的出牌阶段"));
    }

    let msg = if action == "skill:rende" {
        if !crate::skills::has(st, idx, "rende") {
            return Err(illegal("没有该技能"));
        }
        if st.seats[idx].hand.is_empty() {
            return Err(illegal("没有手牌"));
        }
        resolve::push(
            st,
            resolve::Frame::Rende(crate::resolve_more::RendeFrame {
                actor: idx,
                stage: crate::resolve_more::RendeStage::Select,
                selected: Vec::new(),
            }),
        );
        st.push_log(Some(player_id), "发动仁德");
        "请选择要交出的牌".to_string()
    } else if action == "skill:lijian" {
        if !crate::skills::has(st, idx, "lijian") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("lijian_used") > 0 {
            return Err(illegal("本阶段已发动过离间"));
        }
        resolve::push(
            st,
            resolve::Frame::Lijian(crate::resolve_more::LijianFrame {
                actor: idx,
                stage: crate::resolve_more::LijianStage::Discard,
                selected: Vec::new(),
                attacker: None,
            }),
        );
        st.push_log(Some(player_id), "发动离间");
        "请选择要弃置的牌".to_string()
    } else if let Some(card_id) = action.strip_prefix("skill:qixi:") {
        if !crate::skills::has(st, idx, "qixi") {
            return Err(illegal("没有该技能"));
        }
        if !st.seats[idx].hand.iter().any(|c| c == card_id) {
            return Err(illegal("手牌中没有该牌"));
        }
        let black = st
            .cards
            .get(card_id)
            .map(|c| matches!(c.suit, Suit::Spade | Suit::Club))
            .unwrap_or(false);
        if !black {
            return Err(illegal("奇袭只能使用黑色牌"));
        }
        let label = card_label(st, card_id);
        take_card(st, idx, card_id)?;
        st.deck.processing.push(card_id.to_string());
        resolve::start_virtual_trick(st, idx, card_id, "guohe_chaiqiao")?;
        st.push_log(Some(player_id), format!("奇袭：将【{label}】当过河拆桥"));
        "请选择过河拆桥的目标".to_string()
    } else if let Some((skill, card_id)) = ["wusheng", "longdan", "jixi"].iter().find_map(|code| {
        action
            .strip_prefix(&format!("skill:{code}:"))
            .map(|card| (*code, card))
    }) {
        if !crate::skills::has(st, idx, skill) {
            return Err(illegal("没有该技能"));
        }
        if !st.seats[idx].hand.iter().any(|c| c == card_id) {
            return Err(illegal("手牌中没有该牌"));
        }
        let Some((_, name)) = crate::skills::conversion_for(st, idx, card_id, "sha") else {
            return Err(illegal("该牌不能当【杀】使用"));
        };
        let limit = sha_limit(st, idx);
        if st.counter("sha_used") >= limit {
            return Err(illegal("本出牌阶段已使用过【杀】"));
        }
        if sha_targets(st, idx).is_empty() {
            return Err(illegal("攻击范围内没有目标"));
        }
        let label = card_label(st, card_id);
        let fangtian = crate::skills::has_equipment_skill(st, idx, "fangtian_huaji")
            && st.seats[idx].hand.len() == 1;
        take_card(st, idx, card_id)?;
        resolve::start_sha(st, idx, card_id, fangtian)?;
        st.push_log(
            Some(player_id),
            format!("{name}：将【{label}】当【杀】使用"),
        );
        "请选择【杀】的目标".to_string()
    } else if action == "skill:kurou" {
        if !crate::skills::has(st, idx, "kurou") {
            return Err(illegal("没有该技能"));
        }
        if st.seats[idx].hp <= 0 {
            return Err(illegal("体力不足，不能发动苦肉"));
        }
        st.seats[idx].hp -= 1;
        let mut ev = Vec::new();
        let drawn = draw_cards(st, idx, 2, &mut ev);
        let over = st.seats[idx]
            .hand
            .len()
            .saturating_sub(st.config.max_hand_cards as usize);
        if over > 0 {
            resolve::push_discard(st, idx, over, "hand_cap");
        }
        let hp = st.seats[idx].hp;
        st.push_log(
            Some(player_id),
            format!("苦肉：失去 1 点体力，摸 {drawn} 张（体力 {hp}）"),
        );
        if hp <= 0 {
            resolve::push_dying(st, idx, None);
        }
        "已发动苦肉".to_string()
    } else if action == "skill:qingnang" {
        if !crate::skills::has(st, idx, "qingnang") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("qingnang_used") > 0 {
            return Err(illegal("本阶段已发动过青囊"));
        }
        if st.seats[idx].hand.is_empty() {
            return Err(illegal("没有手牌可弃置"));
        }
        resolve::push(
            st,
            resolve::Frame::MultiSelect(resolve::MultiSelectFrame {
                actor: idx,
                purpose: "qingnang".into(),
                selected: Vec::new(),
                min: 1,
                max: 1,
                data: json!({}),
            }),
        );
        st.push_log(Some(player_id), "发动青囊");
        "请选择要弃置的牌".to_string()
    } else if action == "skill:jieyin" {
        if !crate::skills::has(st, idx, "jieyin") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("jieyin_used") > 0 {
            return Err(illegal("本阶段已发动过结姻"));
        }
        if st.seats[idx].hand.len() < 2 {
            return Err(illegal("结姻需弃置 2 张手牌"));
        }
        resolve::push(
            st,
            resolve::Frame::MultiSelect(resolve::MultiSelectFrame {
                actor: idx,
                purpose: "jieyin".into(),
                selected: Vec::new(),
                min: 2,
                max: 2,
                data: json!({}),
            }),
        );
        st.push_log(Some(player_id), "发动结姻");
        "请选择要弃置的 2 张牌".to_string()
    } else if action == "skill:guhuo" {
        crate::resolve_wind::begin_guhuo_play(st, idx)?;
        st.push_log(Some(player_id), "发动蛊惑");
        "蛊惑：请选择要扣置的手牌".to_string()
    } else if action == "skill:qiangxi" {
        if !crate::skills::has(st, idx, "qiangxi") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("qiangxi_used") >= 2 {
            return Err(illegal("本阶段已发动过两次强袭"));
        }
        resolve::push(
            st,
            resolve::Frame::Qiangxi(crate::resolve_fire::QiangxiFrame {
                actor: idx,
                stage: crate::resolve_fire::QiangxiStage::Cost,
            }),
        );
        st.push_log(Some(player_id), "发动强袭");
        "强袭：请选择代价".to_string()
    } else if action == "skill:quhu" {
        if !crate::skills::has(st, idx, "quhu") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("quhu_used") > 0 {
            return Err(illegal("本阶段已发动过驱虎"));
        }
        if st.seats[idx].hand.is_empty() {
            return Err(illegal("没有手牌可拼点"));
        }
        st.set_counter("quhu_used", 1);
        resolve::push(
            st,
            resolve::Frame::Quhu(crate::resolve_fire::QuhuFrame {
                actor: idx,
                target: None,
                stage: crate::resolve_fire::QuhuStage::Target,
            }),
        );
        st.push_log(Some(player_id), "发动驱虎");
        "驱虎：请选择拼点目标".to_string()
    } else if action == "skill:tianyi" {
        if !crate::skills::has(st, idx, "tianyi") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("tianyi_used") > 0 {
            return Err(illegal("本阶段已发动过天义"));
        }
        if st.seats[idx].hand.is_empty() {
            return Err(illegal("没有手牌可拼点"));
        }
        st.set_counter("tianyi_used", 1);
        resolve::push(
            st,
            resolve::Frame::Tianyi(crate::resolve_fire::TianyiFrame {
                actor: idx,
                target: None,
                stage: crate::resolve_fire::TianyiStage::Target,
            }),
        );
        st.push_log(Some(player_id), "发动天义");
        "天义：请选择拼点目标".to_string()
    } else if let Some(card_id) = action.strip_prefix("skill:lianhuan:") {
        if !crate::skills::has(st, idx, "lianhuan") {
            return Err(illegal("没有该技能"));
        }
        if !st.seats[idx].hand.iter().any(|c| c == card_id) {
            return Err(illegal("手牌中没有该牌"));
        }
        if !st.cards.get(card_id).is_some_and(|c| c.suit == Suit::Club) {
            return Err(illegal("连环只能使用梅花手牌"));
        }
        let label = card_label(st, card_id);
        take_card(st, idx, card_id)?;
        st.deck.processing.push(card_id.to_string());
        resolve::start_virtual_trick(st, idx, card_id, "tiesuo_lianhuan")?;
        st.push_log(Some(player_id), format!("连环：将【{label}】当铁索连环"));
        "请选择铁索连环的目标".to_string()
    } else if let Some(card_id) = action.strip_prefix("skill:huoji:") {
        if !crate::skills::has(st, idx, "huoji") {
            return Err(illegal("没有该技能"));
        }
        if !st.seats[idx].hand.iter().any(|c| c == card_id) {
            return Err(illegal("手牌中没有该牌"));
        }
        if !st
            .cards
            .get(card_id)
            .is_some_and(|c| crate::skills::is_red(c.suit))
        {
            return Err(illegal("火计只能使用红色手牌"));
        }
        let label = card_label(st, card_id);
        take_card(st, idx, card_id)?;
        st.deck.processing.push(card_id.to_string());
        resolve::start_virtual_trick(st, idx, card_id, "huo_gong")?;
        let mut ev = Vec::new();
        crate::skills::on_non_delayed_trick_used(st, idx, &mut ev);
        st.push_log(Some(player_id), format!("火计：将【{label}】当火攻"));
        "请选择火攻的目标".to_string()
    } else if let Some(card_id) = action.strip_prefix("skill:shuangxiong:") {
        if crate::resolve_fire::shuangxiong_conversion(st, idx, card_id).is_none() {
            return Err(illegal("双雄：该牌颜色与判定牌相同"));
        }
        if !st.seats[idx].hand.iter().any(|c| c == card_id) {
            return Err(illegal("手牌中没有该牌"));
        }
        let label = card_label(st, card_id);
        take_card(st, idx, card_id)?;
        st.deck.processing.push(card_id.to_string());
        resolve::start_virtual_trick(st, idx, card_id, "juedou")?;
        let mut ev = Vec::new();
        crate::skills::on_non_delayed_trick_used(st, idx, &mut ev);
        st.push_log(Some(player_id), format!("双雄：将【{label}】当决斗"));
        "请选择决斗的目标".to_string()
    } else if let Some(pair) = action.strip_prefix("skill:luanji:") {
        if !crate::skills::has(st, idx, "luanji") {
            return Err(illegal("没有该技能"));
        }
        let (c1, c2) = pair
            .split_once(',')
            .ok_or_else(|| illegal("乱击需要两张牌"))?;
        if c1 == c2 {
            return Err(illegal("乱击需要两张不同的手牌"));
        }
        for c in [c1, c2] {
            if !st.seats[idx].hand.iter().any(|h| h == c) {
                return Err(illegal("手牌中没有该牌"));
            }
        }
        let s1 = st.cards.get(c1).map(|c| c.suit);
        let s2 = st.cards.get(c2).map(|c| c.suit);
        if s1.is_none() || s1 != s2 {
            return Err(illegal("乱击需要两张同花色手牌"));
        }
        let (l1, l2) = (card_label(st, c1), card_label(st, c2));
        take_card(st, idx, c2)?;
        to_discard_card(st, c2);
        take_card(st, idx, c1)?;
        st.deck.processing.push(c1.to_string());
        resolve::start_virtual_trick(st, idx, c1, "wanjian_qifa")?;
        let mut ev = Vec::new();
        crate::skills::on_non_delayed_trick_used(st, idx, &mut ev);
        st.push_log(
            Some(player_id),
            format!("乱击：将【{l1}】【{l2}】当万箭齐发"),
        );
        "使用【万箭齐发】".to_string()
    } else if let Some(card_id) = action.strip_prefix("skill:duanliang:") {
        if !crate::skills::has(st, idx, "duanliang") {
            return Err(illegal("没有该技能"));
        }
        if !st.seats[idx].hand.iter().any(|c| c == card_id) {
            return Err(illegal("手牌中没有该牌"));
        }
        let inst = st
            .cards
            .get(card_id)
            .cloned()
            .ok_or_else(|| illegal("未知牌"))?;
        let def = card_def(st, card_id).ok_or_else(|| illegal("未知牌定义"))?;
        let black = crate::skills::is_black(inst.suit);
        let legal_card = def.category == Category::Basic || def.equip_slot.is_some();
        if !black || !legal_card {
            return Err(illegal("断粮只能使用黑色基本牌或黑色装备牌"));
        }
        let label = card_label(st, card_id);
        take_card(st, idx, card_id)?;
        st.deck.processing.push(card_id.to_string());
        resolve::start_virtual_trick(st, idx, card_id, "bingliang_cunduan")?;
        st.push_log(Some(player_id), format!("断粮：将【{label}】当兵粮寸断"));
        "请选择兵粮寸断的目标".to_string()
    } else if let Some(card_id) = action.strip_prefix("skill:jiuchi:") {
        if !crate::skills::has(st, idx, "jiuchi") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("jiu_used") > 0 {
            return Err(illegal("本回合已使用过【酒】"));
        }
        if !st.seats[idx].hand.iter().any(|c| c == card_id) {
            return Err(illegal("手牌中没有该牌"));
        }
        if !st.cards.get(card_id).is_some_and(|c| c.suit == Suit::Spade) {
            return Err(illegal("酒池只能使用黑桃手牌"));
        }
        let label = card_label(st, card_id);
        st.set_counter("jiu_used", 1);
        st.set_counter("jiu", 1);
        take_card(st, idx, card_id)?;
        to_discard_card(st, card_id);
        st.push_log(
            Some(player_id),
            format!("酒池：将【{label}】当【酒】（下一张杀 +1）"),
        );
        "已使用【酒】".to_string()
    } else if action == "skill:luanwu" {
        if !crate::skills::has(st, idx, "luanwu") {
            return Err(illegal("没有该技能"));
        }
        if st.seats[idx].flags.contains_key("luanwu_used") {
            return Err(illegal("乱武（限定技）本局已发动过"));
        }
        st.seats[idx].flags.insert("luanwu_used".into(), true);
        let order: Vec<usize> = st
            .seats_from_left_of(idx)
            .into_iter()
            .filter(|i| st.seats[*i].alive)
            .collect();
        resolve::push(
            st,
            resolve::Frame::Luanwu(crate::resolve_forest::LuanwuFrame {
                actor: idx,
                order,
                cursor: 0,
            }),
        );
        st.push_log(Some(player_id), "乱武：所有其他角色依次选择");
        "乱武：开始结算".to_string()
    } else if action == "skill:dimeng" {
        if !crate::skills::has(st, idx, "dimeng") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("dimeng_used") > 0 {
            return Err(illegal("本阶段已发动过缔盟"));
        }
        st.set_counter("dimeng_used", 1);
        resolve::push(
            st,
            resolve::Frame::Dimeng(crate::resolve_forest::DimengFrame {
                actor: idx,
                first: None,
                stage: crate::resolve_forest::DimengStage::First,
            }),
        );
        st.push_log(Some(player_id), "发动缔盟");
        "缔盟：请选择第一名角色".to_string()
    } else if action == "skill:tiaoxin" {
        if !crate::skills::has(st, idx, "tiaoxin") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("tiaoxin_used") > 0 {
            return Err(illegal("本阶段已发动过挑衅"));
        }
        st.set_counter("tiaoxin_used", 1);
        resolve::push(
            st,
            resolve::Frame::Tiaoxin(crate::resolve_mountain::TiaoxinFrame {
                actor: idx,
                target: None,
                stage: crate::resolve_mountain::TiaoxinStage::Target,
            }),
        );
        st.push_log(Some(player_id), "发动挑衅");
        "挑衅：请选择目标".to_string()
    } else if action == "skill:fangquan" {
        if !crate::skills::has(st, idx, "fangquan") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("fangquan_used") > 0 {
            return Err(illegal("本阶段已发动过放权"));
        }
        st.set_counter("fangquan_used", 1);
        st.set_counter("skip_play", 1);
        st.seats[idx].flags.insert("fangquan_pending".into(), true);
        st.push_log(Some(player_id), "放权：跳过出牌阶段");
        "放权：已跳过出牌阶段".to_string()
    } else if let Some(card_id) = action.strip_prefix("skill:zhijian:") {
        if !crate::skills::has(st, idx, "zhijian") {
            return Err(illegal("没有该技能"));
        }
        if !st.seats[idx].hand.iter().any(|c| c == card_id) {
            return Err(illegal("手牌中没有该牌"));
        }
        if !card_def(st, card_id).is_some_and(|d| d.equip_slot.is_some()) {
            return Err(illegal("直谏只能使用装备牌"));
        }
        resolve::push(
            st,
            resolve::Frame::Zhijian(crate::resolve_mountain::ZhijianFrame {
                actor: idx,
                card: card_id.to_string(),
            }),
        );
        st.push_log(Some(player_id), "发动直谏");
        "直谏：请选择目标角色".to_string()
    } else if action == "skill:zhiba" {
        if st.counter("zhiba_used") > 0 {
            return Err(illegal("本阶段已发动过制霸"));
        }
        let Some(lord) = crate::resolve_mountain::zhiba_lord(st, idx) else {
            return Err(illegal("没有可制霸的孙策主公"));
        };
        st.set_counter("zhiba_used", 1);
        let has_hunzi = st.seats[lord].flags.contains_key("awaken_hunzi");
        if has_hunzi {
            resolve::push(
                st,
                resolve::Frame::Zhiba(crate::resolve_mountain::ZhibaFrame {
                    challenger: idx,
                    lord,
                    stage: crate::resolve_mountain::ZhibaStage::LordChoice,
                }),
            );
        } else {
            resolve::push(
                st,
                resolve::Frame::Zhiba(crate::resolve_mountain::ZhibaFrame {
                    challenger: idx,
                    lord,
                    stage: crate::resolve_mountain::ZhibaStage::WaitPindian,
                }),
            );
            crate::resolve_fire::start_pindian(
                st,
                idx,
                lord,
                serde_json::json!({ "skill": "zhiba" }),
            );
        }
        st.push_log(Some(player_id), "制霸：拼点");
        "制霸：拼点开始".to_string()
    } else if action == "skill:qimou" {
        if !crate::skills::has(st, idx, "qimou") {
            return Err(illegal("没有该技能"));
        }
        if st.seats[idx].flags.contains_key("qimou_used") {
            return Err(illegal("奇谋（限定技）本局已发动过"));
        }
        if st.seats[idx].hp <= 0 {
            return Err(illegal("体力不足，不能发动奇谋"));
        }
        resolve::push(
            st,
            resolve::Frame::Qimou(crate::resolve_wind::QimouFrame { actor: idx }),
        );
        st.push_log(Some(player_id), "发动奇谋");
        "奇谋：请选择失去的体力值".to_string()
    } else if let Some(card_id) = action.strip_prefix("skill:huangtian:") {
        let owner = crate::skills::huangtian_owner(st, idx)
            .ok_or_else(|| illegal("没有可发动黄天的张角主公"))?;
        if st.counter("huangtian_used") > 0 {
            return Err(illegal("本阶段已发动过黄天"));
        }
        if !st.seats[idx].hand.iter().any(|c| c == card_id) {
            return Err(illegal("手牌中没有该牌"));
        }
        if !card_def(st, card_id).is_some_and(|d| matches!(d.code, "shan" | "shandian")) {
            return Err(illegal("黄天只能交给【闪】或【闪电】"));
        }
        let label = card_label(st, card_id);
        take_card(st, idx, card_id)?;
        st.seats[owner].hand.push(card_id.to_string());
        st.set_counter("huangtian_used", 1);
        st.push_log(
            Some(player_id),
            format!("黄天：将【{label}】交给 {}", st.seats[owner].general_name),
        );
        format!("黄天：已将【{label}】交给张角")
    } else if let Some(card_id) = action.strip_prefix("recast:") {
        if !st.seats[idx].hand.iter().any(|c| c == card_id) {
            return Err(illegal("手牌中没有该牌"));
        }
        let is_tiesuo = card_def(st, card_id).map(|d| d.code) == Some("tiesuo_lianhuan");
        let lianhuan_club = crate::skills::has(st, idx, "lianhuan")
            && st.cards.get(card_id).is_some_and(|c| c.suit == Suit::Club);
        if !is_tiesuo && !lianhuan_club {
            return Err(illegal("只有铁索连环（或连环的梅花手牌）可以重铸"));
        }
        let label = card_label(st, card_id);
        take_card(st, idx, card_id)?;
        to_discard_card(st, card_id);
        let mut ev = Vec::new();
        let drawn = draw_cards(st, idx, 1, &mut ev);
        let over = st.seats[idx]
            .hand
            .len()
            .saturating_sub(st.config.max_hand_cards as usize);
        if over > 0 {
            resolve::push_discard(st, idx, over, "hand_cap");
        }
        st.push_log(
            Some(player_id),
            format!("重铸：弃置【{label}】，摸 {drawn} 张"),
        );
        "已重铸".to_string()
    } else if let Some(msg) = crate::resolve_skill::play_phase_act(st, idx, action)? {
        msg
    } else if let Some(card_id) = action.strip_prefix("skill:guose:") {
        if !crate::skills::has(st, idx, "guose") {
            return Err(illegal("没有该技能"));
        }
        if !st.seats[idx].hand.iter().any(|c| c == card_id) {
            return Err(illegal("手牌中没有该牌"));
        }
        let diamond = st
            .cards
            .get(card_id)
            .is_some_and(|c| c.suit == Suit::Diamond);
        if !diamond {
            return Err(illegal("国色只能使用方块牌"));
        }
        if !st
            .alive_indices()
            .into_iter()
            .any(|t| t != idx && !judge_zone_has(st, t, "lebu_sishu"))
        {
            return Err(illegal("没有可指定目标"));
        }
        let label = card_label(st, card_id);
        take_card(st, idx, card_id)?;
        st.deck.processing.push(card_id.to_string());
        // 记录虚拟定义：方块牌当【乐不思蜀】（判定按生效定义结算）。
        st.virtual_defs
            .insert(card_id.to_string(), "lebu_sishu".to_string());
        resolve::start_virtual_trick(st, idx, card_id, "lebu_sishu")?;
        st.push_log(
            Some(player_id),
            format!("国色：将【{label}】当【乐不思蜀】使用"),
        );
        "请选择国色目标".to_string()
    } else if action == "skill:fanjian" {
        if !crate::skills::has(st, idx, "fanjian") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("fanjian_used") > 0 {
            return Err(illegal("本阶段已发动过反间"));
        }
        if st.seats[idx].hand.is_empty() {
            return Err(illegal("没有手牌可反间"));
        }
        resolve::push(
            st,
            resolve::Frame::Fanjian(crate::resolve_more::FanjianFrame {
                actor: idx,
                stage: crate::resolve_more::FanjianStage::Target,
                target: None,
            }),
        );
        st.push_log(Some(player_id), "发动反间");
        "请选择反间目标".to_string()
    } else if action == "skill:zhiheng" {
        if !crate::skills::has(st, idx, "zhiheng") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("zhiheng_used") > 0 {
            return Err(illegal("本阶段已发动过制衡"));
        }
        let n = st.seats[idx].hand.len();
        if n == 0 {
            return Err(illegal("没有手牌可弃置"));
        }
        resolve::push(
            st,
            resolve::Frame::MultiSelect(resolve::MultiSelectFrame {
                actor: idx,
                purpose: "zhiheng".into(),
                selected: Vec::new(),
                min: 1,
                max: n,
                data: json!({}),
            }),
        );
        st.push_log(Some(player_id), "发动制衡");
        "请选择要弃置的牌".to_string()
    } else if let Some(card_id) = action.strip_prefix("play:") {
        play_card(st, idx, card_id, &mut events)?
    } else if action == "end_play" {
        st.phase_discarded.clear();
        st.turn_phase = TurnPhase::Discard;
        st.push_log(Some(player_id), "结束出牌阶段");
        events.push(EngineEvent::new(
            "phase_end",
            json!({ "phase": "play", "player": player_id }),
        ));
        "结束出牌阶段".to_string()
    } else {
        return Err(illegal(format!("未知动作：{action}")));
    };

    resolve::advance(st, &mut events)?;
    Ok((msg, events))
}

fn play_card(
    st: &mut SgsState,
    idx: usize,
    card_id: &str,
    events: &mut Vec<EngineEvent>,
) -> Result<String, EngineError> {
    if !st.seats[idx].hand.iter().any(|c| c == card_id) {
        return Err(illegal("手牌中没有该牌"));
    }
    let inst = st
        .cards
        .get(card_id)
        .cloned()
        .ok_or_else(|| illegal("未知牌实例"))?;
    let def = data::card_def(&inst.def).ok_or_else(|| illegal("未知牌定义"))?;
    if PENDING_JUNZHENG_CARDS.contains(&def.code) {
        return Err(illegal(format!(
            "【{}】的军争结算将在 P4.3 实现，当前不可使用",
            def.name
        )));
    }
    let actor = st.seats[idx].player_id.clone();
    let label = card_label(st, card_id);

    match def.code {
        "sha" | "huo_sha" | "lei_sha" => {
            let limit = sha_limit(st, idx);
            if st.counter("sha_used") >= limit {
                return Err(illegal("本出牌阶段已使用过【杀】"));
            }
            if sha_targets(st, idx).is_empty() {
                return Err(illegal("攻击范围内没有目标"));
            }
            if crate::skills::sha_blocked(st, idx) {
                return Err(illegal("天义没赢：本回合不能使用【杀】"));
            }
            let tianyi = st.counter("tianyi") > 0;
            let fangtian = crate::skills::has_equipment_skill(st, idx, "fangtian_huaji")
                && st.seats[idx].hand.len() == 1;
            let dtype = match def.code {
                "huo_sha" => resolve::DamageType::Fire,
                "lei_sha" => resolve::DamageType::Thunder,
                _ => resolve::DamageType::Normal,
            };
            take_card(st, idx, card_id)?;
            resolve::start_sha_flags(st, idx, card_id, fangtian, dtype, tianyi, tianyi)?;
            st.push_log(Some(&actor), format!("使用【{label}】"));
            Ok("请选择【杀】的目标".to_string())
        }
        "tao" => {
            if st.seats[idx].hp >= st.seats[idx].max_hp {
                return Err(illegal("体力已满，不能使用【桃】"));
            }
            take_card(st, idx, card_id)?;
            to_discard_card(st, card_id);
            st.seats[idx].hp += 1;
            let hp = st.seats[idx].hp;
            st.push_log(
                Some(&actor),
                format!("使用【{label}】回复 1 点体力（{hp}）"),
            );
            Ok(format!("回复 1 点体力（当前 {hp}）"))
        }
        "jiu" => {
            if st.counter("jiu_used") > 0 {
                return Err(illegal("本回合已使用过【酒】"));
            }
            st.set_counter("jiu_used", 1);
            st.set_counter("jiu", 1);
            take_card(st, idx, card_id)?;
            to_discard_card(st, card_id);
            st.push_log(
                Some(&actor),
                format!("使用【{label}】：本回合下一张【杀】伤害 +1"),
            );
            Ok("已使用【酒】".to_string())
        }
        "shan" => Err(illegal("【闪】只能在响应时使用")),
        "wuxie_keji" => Err(illegal("【无懈可击】只能在响应时使用")),
        "lebu_sishu" | "shandian" | "bingliang_cunduan" => {
            take_card(st, idx, card_id)?;
            st.deck.processing.push(card_id.to_string());
            resolve::start_trick(st, idx, card_id)?;
            st.push_log(Some(&actor), format!("使用【{label}】"));
            Ok("请选择目标".to_string())
        }
        "wugu_fengdeng" => {
            take_card(st, idx, card_id)?;
            st.deck.processing.push(card_id.to_string());
            crate::resolve_zone::start_wugu(st, idx, card_id);
            st.push_log(Some(&actor), format!("使用【{label}】"));
            // 集智：使用非延时锦囊摸 1
            crate::skills::on_non_delayed_trick_used(st, idx, events);
            Ok(format!("使用【{label}】，请依次选牌"))
        }
        _ if def.category == Category::Trick => {
            // 需要目标的锦囊先做一次合法性预检
            if def.code != "wuzhong_shengyou"
                && def.code != "nanman_ruqin"
                && def.code != "wanjian_qifa"
                && def.code != "taoyuan_jieyi"
                && st.alive_indices().len() < 2
            {
                return Err(illegal("没有合法目标"));
            }
            take_card(st, idx, card_id)?;
            st.deck.processing.push(card_id.to_string());
            resolve::start_trick(st, idx, card_id)?;
            st.push_log(Some(&actor), format!("使用【{label}】"));
            // 集智：使用非延时锦囊摸 1
            crate::skills::on_non_delayed_trick_used(st, idx, events);
            if matches!(
                def.code,
                "nanman_ruqin" | "wanjian_qifa" | "taoyuan_jieyi" | "wuzhong_shengyou"
            ) {
                Ok(format!("使用【{label}】"))
            } else {
                Ok("请选择目标".to_string())
            }
        }
        _ if def.equip_slot.is_some() => {
            let slot = def.equip_slot.expect("equip slot");
            take_card(st, idx, card_id)?;
            let old = st.seats[idx].equips.insert(slot, Some(card_id.to_string()));
            if let Some(Some(old_id)) = old {
                to_discard_card(st, &old_id);
                // 枭姬：失去装备区的牌后摸 2 张（自动）。
                crate::skills::xiaoji_on_lose_equip(st, idx);
            }
            st.push_log(Some(&actor), format!("装备【{label}】"));
            Ok(format!("已装备【{label}】"))
        }
        _ => Err(illegal(format!("【{}】不能主动使用", def.name))),
    }
}
