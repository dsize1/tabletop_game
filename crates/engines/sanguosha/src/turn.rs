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
    // 伤逝（张春华）：手牌数低于已损失体力值时摸至该值。
    crate::skills::shangshi_draw(st, idx);
    Ok(())
}

pub(crate) fn to_discard_card(st: &mut SgsState, card_id: &str) {
    st.deck.discard.push(card_id.to_string());
    crate::skills::note_discard_suit(st, card_id);
    // 界激昂（界孙策）：每回合首次【决斗】/红色【杀】因弃置进入弃牌堆。
    crate::resolve_jiexian3::jiang_jie_on_discard(st, card_id);
}

/// 弃置并触发「进入弃牌堆」钩子（落英等）。
pub(crate) fn discard_by(st: &mut SgsState, seat: Option<usize>, card_id: &str) {
    st.deck.discard.push(card_id.to_string());
    crate::skills::note_discard_suit(st, card_id);
    crate::resolve_yijiang::on_card_discarded(st, seat, card_id);
    // 界激昂（界孙策）。
    crate::resolve_jiexian3::jiang_jie_on_discard(st, card_id);
    // 界固政（界张昭张纮）：统计该角色本阶段弃置的牌（每阶段限一次）。
    if let Some(seat) = seat {
        st.bump_counter(&format!("discarded_by:{seat}"));
        if st.alive_indices().into_iter().any(|i| {
            i != seat
                && crate::skills::has(st, i, "guzheng")
                && crate::skills::is_jiexian(st, i)
        }) {
            st.seats[seat]
                .flags
                .insert(format!("gz_jie:{seat}:{card_id}"), true);
        }
        crate::resolve_jiexian5::guzheng_jie_on_discard(st, seat);
    }
    // 界旋风（界凌统）：一次性失去至少两张牌后（近似：同一阶段累计弃置 ≥2 张）。
    if let Some(seat) = seat {
        let n = st.counter(&format!("lost_by:{seat}")) + 1;
        st.set_counter(&format!("lost_by:{seat}"), n);
        if n == 2
            && crate::skills::has(st, seat, "xuanfeng")
            && crate::skills::is_jiexian(st, seat)
        {
            crate::resolve_yijiang::push_xuanfeng(st);
        }
    }
    // 界醇醪（界程普）：自己或相邻角色的【杀】因弃置进入弃牌堆 → 置于「醇」（至多 9）。
    if st
        .cards
        .get(card_id)
        .is_some_and(|c| matches!(c.def.as_str(), "sha" | "huo_sha" | "lei_sha"))
    {
        if let Some(seat) = seat {
            let owners: Vec<usize> = st
                .alive_indices()
                .into_iter()
                .filter(|i| {
                    crate::skills::has(st, *i, "chunlao")
                        && crate::skills::is_jiexian(st, *i)
                        && (*i == seat
                            || crate::skills::kingdom_is(st, *i, "wu")
                                && crate::resolve_jiexian6::adjacent(st, *i, seat))
                })
                .collect();
            for owner in owners {
                crate::resolve_jiexian6::chunlao_place(st, owner, card_id);
            }
        }
    }
    // 界屯田（界邓艾）：回合内弃置【杀】后也可判定。
    if let Some(seat) = seat {
        let own_turn = st.turn == seat;
        let is_sha = st
            .cards
            .get(card_id)
            .is_some_and(|c| matches!(c.def.as_str(), "sha" | "huo_sha" | "lei_sha"));
        if own_turn
            && is_sha
            && crate::skills::has(st, seat, "tuotian")
            && crate::skills::is_jiexian(st, seat)
        {
            crate::skills::tuotian_on_lose(st, seat);
        }
    }
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
        || st.counter(&format!("xl_seat:{idx}")) > 0
    {
        u32::MAX
    } else {
        // 奇谋/天义/立军/将驰：本回合【杀】限制次数 +X。
        1 + st.counter("qimou")
            + st.counter("tianyi")
            + st.counter("lijun_sha_bonus")
            + st.counter("jueyan_sha_bonus")
            + st.counter("jiangchi_bonus")
            + st.counter("jiandu_sha_bonus")
            - st.counter("sha_limit_penalty")
            + u32::from(
                st.seats
                    .get(idx)
                    .is_some_and(|s| s.flags.contains_key("mingjian")),
            )
    }
}

pub(crate) fn sha_targets(st: &SgsState, idx: usize) -> Vec<usize> {
    sha_targets_with(st, idx, false)
}

/// 指定牌面（界烈弓按点数判定距离）。
pub(crate) fn sha_targets_card(st: &SgsState, idx: usize, card: &str) -> Vec<usize> {
    sha_targets_with_card(st, idx, Some(card), false)
}

/// 【杀】目标列表；`unlimited` = 无距离限制（天义 / 成略同花色）。
pub(crate) fn sha_targets_with(st: &SgsState, idx: usize, unlimited: bool) -> Vec<usize> {
    sha_targets_with_card(st, idx, None, unlimited)
}

pub(crate) fn sha_targets_with_card(
    st: &SgsState,
    idx: usize,
    card: Option<&str>,
    unlimited: bool,
) -> Vec<usize> {
    st.alive_indices()
        .into_iter()
        .filter(|t| {
            *t != idx
                && (unlimited
                    || st.in_attack_range(idx, *t)
                    || card.is_some_and(|c| crate::skills::liegong_range_ok(st, idx, c, *t))
                    || crate::skills::xiongluan_target(st, idx) == Some(*t))
                // 空城（锁定）：无手牌时不能成为【杀】目标。
                && !crate::skills::kongcheng_blocks(st, *t)
                // 拒战（本回合不能再对其使用牌）。
                && !crate::skills::deny_target(st, idx, *t)
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
        - crate::skills::benxi_bonus(st, from)
        - crate::skills::qimou_bonus(st, from)
        - crate::skills::tuotian_bonus(st, from)
        - crate::skills::yicong_attack_bonus(st, from)
        + crate::skills::yicong_defense_bonus(st, to)
}

/// 顺手牵羊等「不含武器范围」的距离：`seat_distance + 防御马 - 进攻马 ≤ 1`。
pub(crate) fn trick_distance_ok(st: &SgsState, from: usize, to: usize) -> bool {
    if crate::skills::trick_distance_unlimited(st, from)
        || crate::skills::no_distance_turn(st, from)
        || crate::skills::xiongluan_target(st, from) == Some(to)
    {
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

/// 从牌堆底摸一张（寸目）。
pub(crate) fn draw_one_bottom_public(st: &mut SgsState) -> Option<String> {
    let mut ev = Vec::new();
    draw_one_bottom(st, &mut ev)
}

fn draw_one_bottom(st: &mut SgsState, events: &mut Vec<EngineEvent>) -> Option<String> {
    refill_deck(st, events);
    if st.deck.draw.is_empty() {
        None
    } else {
        Some(st.deck.draw.remove(0))
    }
}

/// 从牌堆顶取一张（不进手牌；供罪论等「亮牌」使用，由调用方放处理区）。
pub(crate) fn draw_top_public(st: &mut SgsState, events: &mut Vec<EngineEvent>) -> Option<String> {
    draw_one(st, events)
}

/// 牌移出游戏（进入 `void`）；触发良姻等钩子（P4.4.1 SP-2）。
///
/// 说明：当前引擎没有主动移出游戏的牌（木牛流马未实现），本函数供后续牌与测试使用。
#[allow(dead_code)]
pub(crate) fn to_void_card(st: &mut SgsState, card_id: &str) {
    st.deck.void.push(card_id.to_string());
    crate::resolve_sp2::liangyin_trigger(st);
}

/// 牌移入游戏（离开 `void`）；返回是否成功。
#[allow(dead_code)]
pub(crate) fn from_void_card(st: &mut SgsState, card_id: &str) -> bool {
    let Some(pos) = st.deck.void.iter().position(|c| c == card_id) else {
        return false;
    };
    st.deck.void.remove(pos);
    crate::resolve_sp2::liangyin_trigger(st);
    true
}

pub(crate) fn draw_cards(
    st: &mut SgsState,
    idx: usize,
    n: u32,
    events: &mut Vec<EngineEvent>,
) -> usize {
    let bottom = crate::skills::has(st, idx, "cunmu");
    let mut drawn = 0;
    for _ in 0..n {
        let card = if bottom {
            draw_one_bottom(st, events)
        } else {
            draw_one(st, events)
        };
        let Some(card) = card else {
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
            // 当先（廖化，锁定）：回合开始时先执行一个额外的出牌阶段。
            if crate::skills::has(st, idx, "dangxian") && st.counter("dangxian_done") == 0 {
                st.set_counter("dangxian_done", 1);
                st.set_counter("dangxian_extra", 1);
                st.push_log(
                    Some(&st.seats[idx].player_id.clone()),
                    "当先：回合开始时执行一个额外出牌阶段",
                );
                // 界当先（界廖化）：额外阶段开始时获得一张【杀】，未造成伤害则阶段结束自伤 1。
                crate::resolve_jiexian7::dangxian_jie_start(st, idx);
                st.turn_phase = TurnPhase::Play;
                return Ok(true);
            }
            // 界裸衣：伤害加成持续到「你的下回合开始」。
            st.seats[idx].flags.remove("luoyi_bonus");
            // 翻面：跳过本回合，随后翻回正面（解围可在此发动）。
            if st.seats[idx].flipped {
                st.seats[idx].flipped = false;
                crate::skills::faen_trigger(st, idx);
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
            // 箜声（周妃）：准备阶段可将任意张牌置于武将牌上。
            if crate::skills::has(st, st.turn, "kongsheng") && st.counter("kongsheng_offered") == 0
            {
                st.set_counter("kongsheng_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::KongSheng(crate::resolve_sp2::KongShengFrame {
                        actor: st.turn,
                        stage: crate::resolve_sp2::KongShengStage::Keep,
                        cards: Vec::new(),
                    }),
                );
                return Ok(true);
            }
            // 镇军（于禁）：准备阶段弃置一名角色 X 张牌（X=手牌-体力，至少 1）。
            if crate::skills::has(st, st.turn, "zhenjun") && st.counter("zhenjun_offered") == 0 {
                st.set_counter("zhenjun_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::ZhenJun(crate::resolve_yijiang::ZhenJunFrame {
                        actor: st.turn,
                        stage: crate::resolve_yijiang::ZhenJunStage::Target,
                        target: None,
                        non_equip: 0,
                    }),
                );
                return Ok(true);
            }
            // 鸿举（毌丘俭）/ 破势（陆抗）：准备阶段觉醒。
            crate::resolve_sp2::hongju_awaken(st, st.turn);
            crate::resolve_sp2::poshi_awaken(st, st.turn);
            // 自立（钟会，觉醒技）：准备阶段「权」≥3。
            crate::resolve_yijiang3::zili_awaken(st, st.turn);
            // 绝响（嵇康）：受保护角色在自己回合开始时解除梅花保护。
            st.seats[st.turn].flags.remove("juexiang");
            // 福绵（吴苋）：准备阶段二选一。
            if crate::skills::has(st, st.turn, "fumian") && st.counter("fumian_offered") == 0 {
                st.set_counter("fumian_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::FuMian(crate::resolve_yijiang9::FuMianFrame { actor: st.turn }),
                );
                return Ok(true);
            }
            // 匡弼（孙登）：准备阶段获得武将牌上的「匡弼」牌，原角色摸等量张。
            let kuangbi = crate::skills::mark_cards(st, st.turn, "kuangbi");
            if !kuangbi.is_empty() {
                let helper_plus = st.counter("kuangbi_helper");
                let n = kuangbi.len();
                for card in &kuangbi {
                    crate::skills::remove_mark(st, st.turn, "kuangbi", card);
                    st.seats[st.turn].hand.push(card.clone());
                }
                st.set_counter("kuangbi_helper", 0);
                st.push_log(
                    Some(&st.seats[st.turn].player_id.clone()),
                    format!("匡弼：获得 {n} 张「匡弼」牌"),
                );
                if helper_plus > 0 {
                    let helper = (helper_plus - 1) as usize;
                    if st.seats[helper].alive {
                        let drawn = draw_cards(st, helper, n as u32, events);
                        st.push_log(
                            Some(&st.seats[helper].player_id.clone()),
                            format!("匡弼：摸 {drawn} 张"),
                        );
                    }
                }
            }
            // 陷嗣（刘封）：准备阶段将至多两名角色各一张牌置于武将牌上为「逆」。
            if crate::skills::has(st, st.turn, "xiansi")
                && st.counter("xiansi_offered") == 0
                && st
                    .alive_indices()
                    .into_iter()
                    .any(|i| i != st.turn && has_cards_to_take(st, i))
            {
                st.set_counter("xiansi_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::XianSi(crate::resolve_yijiang4::XianSiFrame {
                        actor: st.turn,
                        selected: Vec::new(),
                    }),
                );
                return Ok(true);
            }
            // 惴恐（伏皇后）：其他角色回合开始时，若其已受伤可与其拼点。
            if st.counter("zhuikong_offered") == 0 {
                if let Some(owner) = crate::resolve_yijiang4::zhuikong_owner(st, st.turn) {
                    st.set_counter("zhuikong_offered", 1);
                    resolve::push(
                        st,
                        resolve::Frame::ZhuiKong(crate::resolve_yijiang4::ZhuiKongFrame {
                            owner,
                            current: st.turn,
                            stage: crate::resolve_yijiang4::ZhuiKongStage::Ask,
                        }),
                    );
                    return Ok(true);
                }
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
            // 替身（界张飞，限定技）：准备阶段回复所有体力，每回复 1 点摸一张。
            crate::resolve_jiexian::tishen_prepare(st, st.turn);
            // 潜袭（马岱）：准备阶段摸 1 弃 1，然后封锁距离 1 角色某种颜色手牌。
            if crate::skills::has(st, st.turn, "qianxi") && st.counter("qianxi_offered") == 0 {
                st.set_counter("qianxi_offered", 1);
                let drawn = draw_cards(st, st.turn, 1, events);
                st.push_log(
                    Some(&st.seats[st.turn].player_id.clone()),
                    format!("潜袭：摸 {drawn} 张"),
                );
                resolve::push(
                    st,
                    resolve::Frame::QianXi(crate::resolve_yijiang2::QianXiFrame {
                        actor: st.turn,
                        stage: crate::resolve_yijiang2::QianXiStage::Discard,
                        color: 0,
                    }),
                );
                return Ok(true);
            }
            // 勤学（界吕蒙，觉醒技）：准备阶段/结束阶段，手牌数比体力值多 2 或更多。
            if crate::resolve_jiexian::qinxue_ready(st, st.turn)
                && !st.seats[st.turn].flags.contains_key("qinxue_offered")
            {
                st.seats[st.turn]
                    .flags
                    .insert("qinxue_offered".into(), true);
                resolve::push(
                    st,
                    resolve::Frame::QinXue(crate::resolve_jiexian::QinXueFrame { actor: st.turn }),
                );
                return Ok(true);
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
            // 勇略（韩浩史涣）：判定阶段开始时，可弃置其判定牌视为使用【杀】。
            if st.counter("yonglue_offered") == 0 {
                if let Some(owner) = crate::resolve_yijiang5::yonglue_owner(st, st.turn) {
                    st.set_counter("yonglue_offered", 1);
                    resolve::push(
                        st,
                        resolve::Frame::YongLue(crate::resolve_yijiang5::YongLueFrame {
                            owner,
                            current: st.turn,
                        }),
                    );
                    return Ok(true);
                }
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
            // 恂恂（界李典）：摸牌阶段开始时观看牌堆顶 4 张，2 张置顶、其余置底。
            if crate::skills::has(st, idx, "xunxun") && st.counter("xunxun_offered") == 0 {
                st.set_counter("xunxun_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::XunXun(crate::resolve_jiexian::XunXunFrame {
                        actor: idx,
                        cards: Vec::new(),
                        selected: Vec::new(),
                    }),
                );
                return Ok(true);
            }
            // 整论（陆绩）：无「橘」时，可跳过摸牌阶段并获得 1 枚「橘」。
            if crate::skills::has(st, idx, "zhenglun")
                && crate::skills::ju_count(st, idx) == 0
                && st.counter("zhenglun_offered") == 0
            {
                st.set_counter("zhenglun_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::Zhenglun(crate::resolve_sp::ZhenglunFrame { actor: idx }),
                );
                return Ok(true);
            }
            // 突袭：放弃摸牌，改为获得至多两名角色各一张手牌
            if crate::skills::has(st, idx, "tuxi") && st.counter("tuxi_offered") == 0 {
                st.set_counter("tuxi_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::TuxiOffer(crate::resolve_more::TuxiOfferFrame { actor: idx }),
                );
                return Ok(true);
            }
            // 才识（辛宪英）：摸牌阶段开始二选一。
            if crate::skills::has(st, idx, "caishi") && st.counter("caishi_offered") == 0 {
                st.set_counter("caishi_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::CaiShi(crate::resolve_yijiang9::CaiShiFrame { actor: idx }),
                );
                return Ok(true);
            }
            // 奋励（朱桓）：手牌数为全场最多时可跳过摸牌阶段。
            if crate::skills::has(st, idx, "fenli")
                && st.counter("fenli_draw_asked") == 0
                && crate::resolve_yijiang6::fenli_condition(
                    st,
                    idx,
                    crate::resolve_yijiang6::FenLiStage::Draw,
                )
            {
                st.set_counter("fenli_draw_asked", 1);
                resolve::push(
                    st,
                    resolve::Frame::FenLi(crate::resolve_yijiang6::FenLiFrame {
                        actor: idx,
                        stage: crate::resolve_yijiang6::FenLiStage::Draw,
                    }),
                );
                return Ok(true);
            }
            // 眩惑（法正）：摸牌阶段可改为令一名其他角色摸 2 并二选一。
            if crate::skills::has(st, idx, "xuanhuo") && st.counter("xuanhuo_offered") == 0 {
                st.set_counter("xuanhuo_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::XuanHuo(crate::resolve_yijiang4::XuanHuoFrame {
                        actor: idx,
                        stage: crate::resolve_yijiang4::XuanHuoStage::Offer,
                        helper: None,
                    }),
                );
                return Ok(true);
            }
            // 自守（刘表）：摸牌阶段可多摸 X（全场势力数），本回合防止你造成的伤害。
            if crate::skills::has(st, idx, "zishou") && st.counter("zishou_offered") == 0 {
                st.set_counter("zishou_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::ZiShou(crate::resolve_yijiang3::ZiShouFrame { actor: idx }),
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
                && !crate::skills::is_jiexian(st, idx)
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
            // 截辎（界徐晃）：「辎」在摸牌阶段结束移除并执行额外摸牌阶段（简化为多摸 2 张）。
            if crate::resolve_jiexian5::jiezi_jie_extra_draw(st, idx) {
                let mut ev = Vec::new();
                let drawn = draw_cards(st, idx, 2, &mut ev);
                st.push_log(
                    Some(&st.seats[idx].player_id.clone()),
                    format!("截辎：额外摸牌阶段，摸 {drawn} 张"),
                );
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
            // 界双雄（界颜良文丑）：摸牌阶段结束时弃一张牌，本回合异色牌当【决斗】。
            if crate::skills::has(st, idx, "shuangxiong")
                && crate::skills::is_jiexian(st, idx)
                && st.counter("sx_jie_offered") == 0
                && !st.seats[idx].hand.is_empty()
            {
                st.set_counter("sx_jie_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::MultiSelect(crate::resolve::MultiSelectFrame {
                        actor: idx,
                        purpose: "shuangxiong_jie".into(),
                        selected: Vec::new(),
                        min: 0,
                        max: 1,
                        data: json!({}),
                    }),
                );
                return Ok(true);
            }
            if crate::skills::has(st, idx, "shuangxiong")
                && !crate::skills::is_jiexian(st, idx)
                && st.counter("shuangxiong_offered") == 0
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
            // 庸肆（袁术，锁定）：摸牌阶段多摸 X 张（X = 全场势力数）。
            if crate::skills::has(st, idx, "yongsi") {
                let x = crate::skills::kingdom_count(st);
                draw_n += x;
                st.push_log(
                    Some(&st.seats[idx].player_id.clone()),
                    format!("庸肆：摸牌阶段多摸 {x} 张"),
                );
            }
            // 怀橘（锁定）：有「橘」的角色摸牌阶段多摸 1 张。
            if crate::skills::ju_count(st, idx) > 0 {
                draw_n += 1;
            }
            // 自守（刘表）：本回合摸牌阶段多摸 X 张（X = 全场势力数）。
            draw_n += st.counter("zishou_draw");
            // 福绵（吴苋）：摸牌阶段多摸 X 张。
            draw_n += st.counter("fumian_draw");
            // 督粮（李严）：被指定角色下回合摸牌阶段多摸 1 张。
            if st.seats[idx].flags.remove("duliang").is_some() {
                draw_n += 1;
                st.push_log(
                    Some(&st.seats[idx].player_id.clone()),
                    "督粮：摸牌阶段多摸 1 张",
                );
            }
            let drawn = draw_cards(st, idx, draw_n, events);
            let pid = st.seats[idx].player_id.clone();
            st.push_log(Some(&pid), format!("摸牌阶段：摸 {drawn} 张"));
            events.push(EngineEvent::new(
                "phase_start",
                json!({ "phase": "draw", "seat": idx, "player": pid, "drawn": drawn }),
            ));
            st.turn_phase = TurnPhase::Play;
            // 将驰（曹彰）：摸牌阶段结束时二选一（进入出牌阶段前询问）。
            if crate::skills::has(st, idx, "jiangchi") && st.counter("jiangchi_offered") == 0 {
                st.set_counter("jiangchi_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::JiangChi(crate::resolve_yijiang2::JiangChiFrame {
                        actor: idx,
                        stage: crate::resolve_yijiang2::JiangChiStage::Choice,
                    }),
                );
                return Ok(true);
            }
            let over = st.seats[idx]
                .hand
                .len()
                .saturating_sub(st.config.max_hand_cards as usize);
            if over > 0 {
                resolve::push_discard(st, idx, over, "hand_cap");
            }
            // 通博（蔡邕）：摸牌阶段结束后可替换「书」。
            if crate::skills::has(st, idx, "tongbo")
                && st.counter("tongbo_offered") == 0
                && !crate::skills::mark_cards(st, idx, "shu").is_empty()
            {
                st.set_counter("tongbo_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::TongBo(crate::resolve_yijiang9::TongBoFrame {
                        owner: idx,
                        stage: crate::resolve_yijiang9::TongBoStage::Swap,
                        card: None,
                    }),
                );
                return Ok(true);
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
                crate::resolve_sp::chezheng_end_phase(st, st.turn);
                crate::resolve_yijiang5::xiantu_end_phase(st, st.turn);
                crate::resolve_yijiang5::sidi_end_phase(st, st.turn);
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
            // 奋励（朱桓）：体力值为全场最多时可跳过出牌阶段。
            if crate::skills::has(st, st.turn, "fenli")
                && st.counter("fenli_play_asked") == 0
                && crate::resolve_yijiang6::fenli_condition(
                    st,
                    st.turn,
                    crate::resolve_yijiang6::FenLiStage::Play,
                )
            {
                st.set_counter("fenli_play_asked", 1);
                resolve::push(
                    st,
                    resolve::Frame::FenLi(crate::resolve_yijiang6::FenLiFrame {
                        actor: st.turn,
                        stage: crate::resolve_yijiang6::FenLiStage::Play,
                    }),
                );
                return Ok(true);
            }
            // 急攻（郭图逢纪）：出牌阶段开始时，可摸 2 张，本回合手牌上限=本阶段伤害值。
            if crate::skills::has(st, st.turn, "jigong") && st.counter("jigong_offered") == 0 {
                st.set_counter("jigong_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::JiGong(crate::resolve_yijiang7::JiGongFrame { actor: st.turn }),
                );
                return Ok(true);
            }
            // 血裔（界袁绍）：出牌阶段开始时，可移去 1 枚「裔」并摸一张牌。
            if crate::skills::has(st, st.turn, "xueyi")
                && crate::skills::is_jiexian(st, st.turn)
                && st.counter("xueyi_asked") == 0
                && crate::skills::xueyi_marks(st, st.turn) > 0
            {
                resolve::push(
                    st,
                    resolve::Frame::XueYi(crate::resolve_jiexian2::XueYiFrame { actor: st.turn }),
                );
                return Ok(true);
            }
            // 思蜀（界刘禅，觉醒获得）：出牌阶段开始时，可令一名角色【乐不思蜀】判定效果反转。
            if crate::skills::has(st, st.turn, "sishu") && st.counter("sishu_asked") == 0 {
                resolve::push(
                    st,
                    resolve::Frame::SiShu(crate::resolve_jiexian2::SiShuFrame { actor: st.turn }),
                );
                return Ok(true);
            }
            // 司敌（曹真）：其他角色出牌阶段开始时，可弃同色非基本牌封锁颜色。
            if st.counter("sidi_offered") == 0 {
                if let Some(owner) = st.alive_indices().into_iter().find(|i| {
                    *i != st.turn
                        && crate::skills::has(st, *i, "sidi")
                        && !crate::resolve_yijiang5::sidi_costs(st, *i).is_empty()
                }) {
                    st.set_counter("sidi_offered", 1);
                    resolve::push(
                        st,
                        resolve::Frame::SiDi(crate::resolve_yijiang5::SiDiFrame {
                            owner,
                            current: st.turn,
                        }),
                    );
                    return Ok(true);
                }
            }
            // 献图（张松）：其他角色出牌阶段开始时，可摸 2 交 2。
            if st.counter("xiantu_offered") == 0 {
                if let Some(owner) = st
                    .alive_indices()
                    .into_iter()
                    .find(|i| *i != st.turn && crate::skills::has(st, *i, "xiantu"))
                {
                    st.set_counter("xiantu_offered", 1);
                    resolve::push(
                        st,
                        resolve::Frame::XianTu(crate::resolve_yijiang5::XianTuFrame {
                            owner,
                            target: st.turn,
                        }),
                    );
                    return Ok(true);
                }
            }
            // 强识（张松）：出牌阶段开始时展示一名其他角色一张手牌。
            if crate::skills::has(st, st.turn, "qiangshi")
                && st.counter("qiangshi_offered") == 0
                && st
                    .alive_indices()
                    .into_iter()
                    .any(|i| i != st.turn && !st.seats[i].hand.is_empty())
            {
                st.set_counter("qiangshi_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::QiangShi(crate::resolve_yijiang5::QiangShiFrame {
                        actor: st.turn,
                    }),
                );
                return Ok(true);
            }
            // 遗礼（陆绩）：出牌阶段开始时，可失去 1 点体力或移除 1 枚「橘」。
            if crate::skills::has(st, st.turn, "yili") && st.counter("yili_offered") == 0 {
                st.set_counter("yili_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::Yili(crate::resolve_sp::YiliFrame {
                        actor: st.turn,
                        stage: crate::resolve_sp::YiliStage::Cost,
                    }),
                );
                return Ok(true);
            }
            // 巧说（简雍）：出牌阶段开始时，可与一名其他角色拼点。
            if crate::skills::has(st, st.turn, "qiaoshuo")
                && !crate::skills::is_jiexian(st, st.turn)
                && st.counter("qiaoshuo_offered") == 0
                && !st.seats[st.turn].hand.is_empty()
                && st
                    .alive_indices()
                    .into_iter()
                    .any(|i| i != st.turn && !st.seats[i].hand.is_empty())
            {
                st.set_counter("qiaoshuo_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::QiaoShuo(crate::resolve_yijiang3::QiaoShuoFrame {
                        actor: st.turn,
                        stage: crate::resolve_yijiang3::QiaoShuoStage::Target,
                    }),
                );
                return Ok(true);
            }
            // 等待玩家操作
            Ok(false)
        }
        TurnPhase::Discard => {
            let idx = st.turn;
            // 放权（界刘禅）：弃牌阶段开始时结算（需弃一张手牌）。
            if st.seats[idx].flags.contains_key("fangquan_pending")
                && st.counter("fangquan_done") == 0
                && crate::skills::is_jiexian(st, idx)
            {
                st.set_counter("fangquan_done", 1);
                if st.seats[idx].hand.is_empty() {
                    st.seats[idx].flags.remove("fangquan_pending");
                    let pid = st.seats[idx].player_id.clone();
                    st.push_log(Some(&pid), "放权：没有手牌可弃，未授予额外回合");
                } else {
                    resolve::push(
                        st,
                        resolve::Frame::Fangquan(crate::resolve_mountain::FangquanFrame {
                            actor: idx,
                            stage: crate::resolve_mountain::FangquanStage::Target,
                        }),
                    );
                    return Ok(true);
                }
            }
            // 奋励（朱桓）：装备区牌数为全场最多时可跳过弃牌阶段。
            if crate::skills::has(st, idx, "fenli")
                && st.counter("fenli_discard_asked") == 0
                && crate::resolve_yijiang6::fenli_condition(
                    st,
                    idx,
                    crate::resolve_yijiang6::FenLiStage::Discard,
                )
            {
                st.set_counter("fenli_discard_asked", 1);
                resolve::push(
                    st,
                    resolve::Frame::FenLi(crate::resolve_yijiang6::FenLiFrame {
                        actor: idx,
                        stage: crate::resolve_yijiang6::FenLiStage::Discard,
                    }),
                );
                return Ok(true);
            }
            // 奋励：已选择跳过弃牌阶段。
            if st.counter("fenli_skip_discard") > 0 {
                st.set_counter("fenli_skip_discard", 0);
                st.phase_discarded.clear();
                st.turn_phase = TurnPhase::End;
                return Ok(true);
            }
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
            // 庸肆（袁术，锁定）：弃牌阶段开始时，先弃 X 张（X = 全场势力数）。
            if crate::skills::has(st, idx, "yongsi") && st.counter("yongsi_discard_done") == 0 {
                st.set_counter("yongsi_discard_done", 1);
                let x = crate::skills::kingdom_count(st) as usize;
                let n = x.min(st.seats[idx].hand.len());
                if n > 0 {
                    st.push_log(
                        Some(&st.seats[idx].player_id.clone()),
                        format!("庸肆：弃牌阶段开始时先弃 {n} 张"),
                    );
                    resolve::push_discard(st, idx, n, "yongsi");
                    return Ok(true);
                }
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
            // 旋风（凌统）：弃牌阶段弃置 ≥2 张时发动。
            if crate::skills::has(st, idx, "xuanfeng")
                && st.phase_discarded.len() >= 2
                && st.counter(&format!("xuanfeng:{}", idx)) == 0
            {
                crate::resolve_yijiang::push_xuanfeng(st);
                return Ok(true);
            }
            // 溃诛（孙亮）：弃牌阶段结束时，按本阶段弃牌数发动。
            if crate::skills::has(st, idx, "kuizhu")
                && st.counter("kuizhu_done") == 0
                && !st.phase_discarded.is_empty()
            {
                st.set_counter("kuizhu_done", 1);
                let x = st.phase_discarded.len() as u32;
                resolve::push(
                    st,
                    resolve::Frame::Kuizhu(crate::resolve_sp::KuizhuFrame {
                        actor: idx,
                        stage: crate::resolve_sp::KuizhuStage::Choice,
                        chosen: Vec::new(),
                        x,
                    }),
                );
                return Ok(true);
            }
            // 瑰藻（孙资刘放）：本阶段弃置 ≥2 张且花色均不相同。
            if crate::resolve_yijiang8::guizao_ready(st, idx) {
                st.set_counter("guizao_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::GuiZao(crate::resolve_yijiang8::GuiZaoFrame { owner: idx }),
                );
                return Ok(true);
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
            // 界直言（界虞翻）：上家的结束阶段也可发动。
            crate::resolve_jiexian6::zhiyan_upper_trigger(st, st.turn);
            // 界双雄（界颜良文丑）：结束阶段获得本回合对你造成过伤害的牌。
            crate::resolve_jiexian5::shuangxiong_jie_gain(st, st.turn);
            // 界再起（界孟获）：结束阶段按本回合红牌弃置数依次结算。
            if crate::resolve_jiexian5::zaizhi_jie_trigger(st, st.turn) {
                return Ok(true);
            }
            // 界巧变（界张郃）：结束阶段手牌数与以往均不相同 → +1 「变」。
            crate::resolve_jiexian4::qiaobian_end_check(st, st.turn);
            // 界魂姿（界孙策）：觉醒当回合结束阶段，摸两张或回复 1 点。
            if st.seats[st.turn].flags.remove("hunzi_end").is_some()
                && st.counter("hunzi_end_asked") == 0
            {
                resolve::push(
                    st,
                    resolve::Frame::HunziEnd(crate::resolve_jiexian3::HunziEndFrame {
                        actor: st.turn,
                    }),
                );
                return Ok(true);
            }
            // 飘零（界小乔）：结束阶段判定，红桃则置顶或交给一名角色。
            if crate::skills::has(st, st.turn, "piaoling") && st.counter("piaoling_asked") == 0 {
                crate::resolve_jiexian3::piaoling_trigger(st, st.turn);
                if st.counter("piaoling_asked") > 0 {
                    return Ok(true);
                }
            }
            // 武烈（界孙坚，限定技）：结束阶段失去任意点体力，令等量角色获得「烈」。
            if crate::skills::has(st, st.turn, "wulie")
                && !st.seats[st.turn].flags.contains_key("wulie_used")
                && st.counter("wulie_asked") == 0
            {
                st.set_counter("wulie_asked", 1);
                resolve::push(
                    st,
                    resolve::Frame::WuLie(crate::resolve_jiexian3::WuLieFrame {
                        actor: st.turn,
                        stage: crate::resolve_jiexian3::WuLieStage::Amount,
                    }),
                );
                return Ok(true);
            }
            // 诛害（界徐庶）：其他角色的结束阶段，若其本回合造成过伤害 → 可视为对其使用【杀】。
            if let Some(owner) = crate::resolve_jiexian2::zhuhai_available(st, st.turn) {
                resolve::push(
                    st,
                    resolve::Frame::ZhuHai(crate::resolve_jiexian2::ZhuHaiFrame {
                        owner,
                        target: st.turn,
                    }),
                );
                return Ok(true);
            }
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
            // 放权（标准刘禅）：本回合结束时，令一名其他角色执行额外回合。
            if st.seats[st.turn].flags.contains_key("fangquan_pending")
                && st.counter("fangquan_done") == 0
                && !crate::skills::is_jiexian(st, st.turn)
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
                && st.counter("bengkuai_off") == 0
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
            // 明任（卢植）：结束阶段可用手牌替换「任」。
            if crate::skills::has(st, st.turn, "mingren") && st.counter("mingren_offered") == 0 {
                st.set_counter("mingren_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::MingRen(crate::resolve_sp2::MingRenFrame { actor: st.turn }),
                );
                return Ok(true);
            }
            // 镇骨（郝昭）：结束阶段选择一名其他角色。
            if crate::skills::has(st, st.turn, "zhengu") && st.counter("zhengu_offered") == 0 {
                st.set_counter("zhengu_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::ZhenGu(crate::resolve_sp2::ZhenGuFrame { actor: st.turn }),
                );
                return Ok(true);
            }
            // 罪论（诸葛瞻）：结束阶段观看牌堆顶三张。
            if crate::resolve_sp2::start_zuilun(st) {
                return Ok(true);
            }
            // 箜声（周妃）：结束阶段获得非装备牌，并令一名角色使用剩余装备牌。
            if crate::skills::has(st, st.turn, "kongsheng") && st.counter("kongsheng_end") == 0 {
                st.set_counter("kongsheng_end", 1);
                let mut kong = crate::skills::mark_cards(st, st.turn, "kong");
                let mut equips = Vec::new();
                for card in kong.drain(..) {
                    let is_equip = card_def(st, &card).is_some_and(|d| d.equip_slot.is_some());
                    crate::skills::remove_mark(st, st.turn, "kong", &card);
                    if is_equip {
                        equips.push(card);
                    } else {
                        st.seats[st.turn].hand.push(card);
                    }
                }
                if !equips.is_empty() {
                    resolve::push(
                        st,
                        resolve::Frame::KongSheng(crate::resolve_sp2::KongShengFrame {
                            actor: st.turn,
                            stage: crate::resolve_sp2::KongShengStage::UseEquip,
                            cards: equips,
                        }),
                    );
                    return Ok(true);
                }
            }
            // 勤学（界吕蒙）：结束阶段同样可觉醒（每局只询问一次）。
            if crate::resolve_jiexian::qinxue_ready(st, st.turn)
                && !st.seats[st.turn].flags.contains_key("qinxue_offered")
            {
                st.seats[st.turn]
                    .flags
                    .insert("qinxue_offered".into(), true);
                resolve::push(
                    st,
                    resolve::Frame::QinXue(crate::resolve_jiexian::QinXueFrame { actor: st.turn }),
                );
                return Ok(true);
            }
            // 醇醪（程普）：结束阶段若没有「醇」，可将任意张【杀】置于武将牌上。
            if crate::skills::has(st, st.turn, "chunlao")
                && crate::skills::chun_count(st, st.turn) == 0
                && st.seats[st.turn].hand.iter().any(|id| {
                    st.cards
                        .get(id)
                        .is_some_and(|c| matches!(c.def.as_str(), "sha" | "huo_sha" | "lei_sha"))
                })
                && st.counter("chunlao_offered") == 0
            {
                st.set_counter("chunlao_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::ChunLao(crate::resolve_yijiang2::ChunLaoFrame {
                        actor: st.turn,
                        selected: Vec::new(),
                    }),
                );
                return Ok(true);
            }
            // 界观星：若准备阶段全部置于牌堆底，结束阶段可再次发动。
            if crate::skills::has(st, st.turn, "guanxing")
                && crate::skills::is_jiexian(st, st.turn)
                && st.counter("guanxing_bottom") > 0
                && st.counter("guanxing_end") == 0
            {
                st.set_counter("guanxing_end", 1);
                resolve::push(st, crate::resolve_zone::start_guanxing(st.turn));
                return Ok(true);
            }
            // 进趋（王基）：结束阶段可摸 2 张，然后将手牌弃至本回合奇制次数。
            if crate::skills::has(st, st.turn, "jinzou") && st.counter("jinzou_done") == 0 {
                st.set_counter("jinzou_done", 1);
                resolve::push(
                    st,
                    resolve::Frame::Jinzou(crate::resolve_sp::JinzouFrame { actor: st.turn }),
                );
                return Ok(true);
            }
            // 寝情（黄皓）：结束阶段弃置攻击范围含主公的角色各一张牌，再令其各摸一张。
            if crate::skills::has(st, st.turn, "qinqing") && st.counter("qinqing_offered") == 0 {
                st.set_counter("qinqing_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::QinQing(crate::resolve_yijiang8::QinQingFrame {
                        actor: st.turn,
                        selected: Vec::new(),
                    }),
                );
                return Ok(true);
            }
            // 极奢（岑昏）：结束阶段若没有手牌，每有 1 点体力可令一名角色进入连环状态。
            if crate::skills::has(st, st.turn, "jishe")
                && st.counter("jishe_end_offered") == 0
                && st.seats[st.turn].hand.is_empty()
                && st.seats[st.turn].hp > 0
            {
                st.set_counter("jishe_end_offered", 1);
                let remaining = st.seats[st.turn].hp as u32;
                resolve::push(
                    st,
                    resolve::Frame::JiSheEnd(crate::resolve_yijiang8::JiSheEndFrame {
                        actor: st.turn,
                        remaining,
                    }),
                );
                return Ok(true);
            }
            // 樵拾（夏侯氏）：其他角色结束阶段手牌数相等时，各摸一张。
            if st.counter("qiaoshi_checked") == 0 {
                if let Some(owner) = crate::resolve_yijiang7::qiaoshi_owner(st, st.turn) {
                    st.set_counter("qiaoshi_checked", 1);
                    resolve::push(
                        st,
                        resolve::Frame::QiaoShi(crate::resolve_yijiang7::QiaoShiFrame {
                            owner,
                            current: st.turn,
                        }),
                    );
                    return Ok(true);
                }
            }
            // 兴学（孙休）：结束阶段每有 1 点体力，可令一名角色摸一张并置于牌堆顶。
            if crate::skills::has(st, st.turn, "xingxue") && st.counter("xingxue_offered") == 0 {
                st.set_counter("xingxue_offered", 1);
                let upgraded = st.seats[st.turn].flags.contains_key("xingxue_upgraded");
                let remaining = if upgraded {
                    1
                } else {
                    st.seats[st.turn].hp.max(1) as u32
                };
                resolve::push(
                    st,
                    resolve::Frame::XingXue(crate::resolve_yijiang7::XingXueFrame {
                        actor: st.turn,
                        remaining,
                    }),
                );
                return Ok(true);
            }
            // 平寇（朱桓）：本回合每跳过一个阶段，可对一名其他角色造成 1 点伤害。
            if crate::skills::has(st, st.turn, "pingkou")
                && st.counter("pingkou_offered") == 0
                && st.counter("fenli_skipped") > 0
            {
                st.set_counter("pingkou_offered", 1);
                let remaining = st.counter("fenli_skipped");
                resolve::push(
                    st,
                    resolve::Frame::PingKou(crate::resolve_yijiang6::PingKouFrame {
                        actor: st.turn,
                        remaining,
                    }),
                );
                return Ok(true);
            }
            // 窃听（蔡夫人）：其他角色回合结束后，若其本回合未对其他角色使用过牌。
            if crate::skills::has(st, st.turn, "qieting") && st.counter("qieting_offered") == 0 {
                st.set_counter("qieting_offered", 1);
                let drawn = draw_cards(st, st.turn, 1, events);
                st.push_log(
                    Some(&st.seats[st.turn].player_id.clone()),
                    format!("窃听：自己回合结束，摸 {drawn} 张（占位）"),
                );
            }
            if st.counter("qieting_checked") == 0 {
                if let Some(owner) = crate::resolve_yijiang6::qieting_owner(st, st.turn) {
                    st.set_counter("qieting_checked", 1);
                    resolve::push(
                        st,
                        resolve::Frame::QieTing(crate::resolve_yijiang6::QieTingFrame {
                            owner,
                            current: st.turn,
                        }),
                    );
                    return Ok(true);
                }
            }
            // 怠宴（吴苋）：结束阶段令一名其他角色获得红桃基本牌。
            if crate::skills::has(st, st.turn, "daiyan") && st.counter("daiyan_offered") == 0 {
                st.set_counter("daiyan_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::DaiYan(crate::resolve_yijiang9::DaiYanFrame { actor: st.turn }),
                );
                return Ok(true);
            }
            // 惠民（曹节）：结束阶段摸 X 张并依次分配。
            if crate::skills::has(st, st.turn, "huimin") && st.counter("huimin_offered") == 0 {
                let x = crate::resolve_yijiang9::huimin_x(st, st.turn);
                if x > 0 {
                    st.set_counter("huimin_offered", 1);
                    let mut cards = Vec::new();
                    for _ in 0..x {
                        if let Some(card) = draw_one(st, events) {
                            st.seats[st.turn].hand.push(card.clone());
                            cards.push(card);
                        }
                    }
                    st.push_log(
                        Some(&st.seats[st.turn].player_id.clone()),
                        format!("惠民：摸 {} 张并分配", cards.len()),
                    );
                    resolve::push(
                        st,
                        resolve::Frame::HuiMin(crate::resolve_yijiang9::HuiMinFrame {
                            owner: st.turn,
                            cards,
                        }),
                    );
                    return Ok(true);
                }
            }
            // 诫训（薛综）：结束阶段令一名其他角色摸牌后弃牌。
            if crate::skills::has(st, st.turn, "jiexun") && st.counter("jiexun_offered") == 0 {
                st.set_counter("jiexun_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::JieXun(crate::resolve_yijiang9::JieXunFrame { owner: st.turn }),
                );
                return Ok(true);
            }
            // 伏诛（徐氏）：男性角色结束阶段，牌堆 ≤ 体力值×10 时。
            if st.counter("fuzhu_offered") == 0 {
                if let Some(owner) = crate::resolve_yijiang9::fuzhu_owner(st, st.turn) {
                    st.set_counter("fuzhu_offered", 1);
                    resolve::push(
                        st,
                        resolve::Frame::FuZhu(crate::resolve_yijiang9::FuZhuFrame {
                            owner,
                            current: st.turn,
                        }),
                    );
                    return Ok(true);
                }
            }
            // 秘计（王异）：结束阶段摸 X（已损失体力），然后可交给其他角色 X 张手牌。
            let miji_extra =
                crate::skills::is_jiexian(st, st.turn) && st.counter("miji_extra") > 0;
            if crate::skills::has(st, st.turn, "miji") && st.counter("miji_offered") == 0 {
                if miji_extra {
                    // 界贞烈（界王异）：本回合结束阶段额外发动一次秘计（一次结算内连发）。
                    st.set_counter("miji_extra", 0);
                }
                let lost = (st.seats[st.turn].max_hp - st.seats[st.turn].hp).max(0) as u32;
                if lost > 0 {
                    st.set_counter("miji_offered", 1);
                    let times = if miji_extra { 2 } else { 1 };
                    st.set_counter("miji_extra_times", times);
                    let mut drawn = 0;
                    for _ in 0..times {
                        drawn += draw_cards(st, st.turn, lost, events);
                    }
                    st.push_log(
                        Some(&st.seats[st.turn].player_id.clone()),
                        format!("秘计：摸 {drawn} 张"),
                    );
                    if !st.seats[st.turn].hand.is_empty() && st.alive_indices().len() > 1 {
                        resolve::push(
                            st,
                            resolve::Frame::MiJi(crate::resolve_yijiang5::MiJiFrame {
                                actor: st.turn,
                                stage: crate::resolve_yijiang5::MiJiStage::Select,
                                x: lost,
                                selected: Vec::new(),
                            }),
                        );
                        return Ok(true);
                    }
                }
            }
            // 直言（虞翻）：结束阶段令一名角色摸 1 展示，装备牌则使用并回复 1。
            if crate::skills::has(st, st.turn, "zhiyan") && st.counter("zhiyan_offered") == 0 {
                st.set_counter("zhiyan_offered", 1);
                resolve::push(
                    st,
                    resolve::Frame::ZhiYan(crate::resolve_yijiang4::ZhiYanFrame { actor: st.turn }),
                );
                return Ok(true);
            }
            // 绝策（李儒）：结束阶段对一名没有手牌的其他角色造成 1 点伤害。
            if crate::skills::has(st, st.turn, "juece") && st.counter("juece_offered") == 0 {
                if !crate::resolve_yijiang4::juece_targets(st, st.turn).is_empty() {
                    st.set_counter("juece_offered", 1);
                    resolve::push(
                        st,
                        resolve::Frame::JueCe(crate::resolve_yijiang4::JueCeFrame {
                            actor: st.turn,
                        }),
                    );
                    return Ok(true);
                }
            }
            // 举荐（徐庶）：结束阶段弃一张非基本牌，令一名其他角色三选一。
            if crate::skills::has(st, st.turn, "jujian") && st.counter("jujian_offered") == 0 {
                if !crate::resolve_yijiang4::jujian_cards(st, st.turn).is_empty()
                    && st.alive_indices().len() > 1
                {
                    st.set_counter("jujian_offered", 1);
                    resolve::push(
                        st,
                        resolve::Frame::JuJian(crate::resolve_yijiang4::JuJianFrame {
                            actor: st.turn,
                            stage: crate::resolve_yijiang4::JuJianStage::Card,
                            card: None,
                            target: None,
                        }),
                    );
                    return Ok(true);
                }
            }
            // 审时（阴）：当前回合结束阶段校验目标是否仍未失去该牌。
            crate::resolve_sp::shenshi_end_turn(st);
            crate::skills::on_phase_start(st, TurnPhase::End, events);
            end_turn(st, events);
            Ok(true)
        }
    }
}

fn end_turn(st: &mut SgsState, events: &mut Vec<EngineEvent>) {
    let cur = st.turn;
    let pid = st.seats[cur].player_id.clone();
    // 破军（徐盛）：回合结束把「移出游戏」的牌回置原主人手牌。
    if st.void_return.iter().any(|e| e.due_seat == cur) {
        let due: Vec<_> = st
            .void_return
            .iter()
            .filter(|e| e.due_seat == cur)
            .cloned()
            .collect();
        st.void_return.retain(|e| e.due_seat != cur);
        for e in due {
            if let Some(pos) = st.deck.void.iter().position(|c| c == &e.card) {
                st.deck.void.remove(pos);
                st.seats[e.owner].hand.push(e.card.clone());
            }
        }
    }
    // 延迟结算（镇骨）：本座位回合结束时触发。
    if st.scheduled.iter().any(|e| e.due_seat == cur) {
        let due: Vec<_> = st
            .scheduled
            .iter()
            .filter(|e| e.due_seat == cur)
            .cloned()
            .collect();
        st.scheduled.retain(|e| e.due_seat != cur);
        for e in due {
            if e.kind == "zhengu" {
                crate::resolve_sp2::zhengu_adjust(st, e.owner, e.target);
            }
        }
    }
    // 博图（界吕蒙）：本回合置入弃牌堆的牌含四种花色 → 执行一个额外回合（每轮至多 X=min(存活,3) 次）。
    if crate::skills::has(st, cur, "botu") && st.counter("discard_suits") == 15 {
        let key = format!("botu:{}", st.round);
        let used = st.seats[cur].mark_counts.get(&key).copied().unwrap_or(0);
        let limit = st.alive_indices().len().min(3) as u32;
        if used < limit {
            st.seats[cur].mark_counts.insert(key, used + 1);
            st.extra_turn = Some(cur);
            st.push_log(
                Some(&pid),
                format!(
                    "博图：本回合弃牌含四种花色，获得额外回合（{} / {limit}）",
                    used + 1
                ),
            );
        }
    }
    crate::resolve_yijiang8::clear_jiazhao(st, cur);
    crate::resolve_yijiang9::clear_funan(st, cur);
    st.seats[cur].flags.remove("caishi_no_self");
    st.seats[cur].flags.remove("mingjian");
    st.seats[cur].flags.remove("jigong");
    if st.seats[cur].flags.remove("fuhen_granted").is_some() {
        crate::skills::revoke_skill(st, cur, "wusheng");
        crate::skills::revoke_skill(st, cur, "paoxiao");
    }
    st.push_log(Some(&pid), "结束阶段");
    events.push(EngineEvent::new(
        "turn_end",
        json!({ "seat": cur, "player": pid }),
    ));
    st.turn_counters.clear();
    st.phase_discarded.clear();
    st.shenshi_marks.clear();
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
    // 界凿险（界邓艾）：本回合结束后执行一个额外回合。
    if st.seats[cur].flags.remove("zaoxian_extra").is_some() {
        st.extra_turn = Some(cur);
    }
    if let Some(extra) = st.extra_turn.take() {
        // 放权（他人额外回合）/ 博图（自己额外回合）都走这里。
        if st.seats.get(extra).is_some_and(|s| s.alive) {
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
            // 思蜀（界刘禅）：该角色本局【乐不思蜀】判定效果反转（红桃→跳过出牌阶段）。
            let heart = judge_card.suit == Suit::Heart;
            let skip = if crate::skills::sishu_reversed(st, owner) {
                heart
            } else {
                !heart
            };
            if skip {
                st.set_counter("skip_play", 1);
                if crate::skills::sishu_reversed(st, owner) {
                    "思蜀：红桃反转，跳过出牌阶段"
                } else {
                    "非红桃，跳过出牌阶段"
                }
            } else if crate::skills::sishu_reversed(st, owner) {
                "思蜀：非红桃反转，未生效"
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
        crate::resolve_yijiang::on_card_discarded(st, None, &judge_id);
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
                let free = crate::skills::chenglue_free(st, idx, &card_id)
                    || crate::skills::no_distance_turn(st, idx);
                (free || st.counter("sha_used") < sha_limit)
                    && !sha_targets_with_card(st, idx, Some(&card_id), free).is_empty()
            }
            // 酒：每回合限一次（本回合下一张【杀】伤害 +1）。
            "jiu" => st.counter("jiu_used") == 0 || crate::skills::jiu_unlimited(st, idx),
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
                    && (trick_distance_ok(st, idx, t) || crate::skills::has(st, idx, "shenduan"))
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
            _ if def.equip_slot.is_some() => !def
                .equip_slot
                .is_some_and(|slot| st.slot_abolished(idx, slot)),
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
    // 苦肉：标准版失去 1 点体力摸 2 张（可多次）；界版弃 1 张牌再失去 1 点体力（限一次）。
    if crate::skills::has(st, idx, "kurou") && st.seats[idx].hp > 0 {
        if crate::skills::is_jiexian(st, idx) {
            if st.counter("kurou_used") == 0 {
                for card_id in hand_ids(st, idx) {
                    out.push(Action {
                        id: format!("skill:kurou:{card_id}"),
                        description: format!(
                            "苦肉：弃置【{}】并失去 1 点体力",
                            card_label(st, &card_id)
                        ),
                    });
                }
            }
        } else {
            out.push(Action {
                id: "skill:kurou".into(),
                description: "苦肉：失去 1 点体力，摸 2 张牌".into(),
            });
        }
    }
    // 攻心（界吕蒙·勤学觉醒获得）：观看一名其他角色手牌，可弃置一张红桃牌。
    if crate::skills::has(st, idx, "gongxin")
        && st.counter("gongxin_used") == 0
        && st
            .alive_indices()
            .into_iter()
            .any(|i| i != idx && !st.seats[i].hand.is_empty())
    {
        out.push(Action {
            id: "skill:gongxin".into(),
            description: "攻心：观看一名其他角色手牌，弃置其中一张红桃牌".into(),
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
    // 结姻：标准弃 2 张手牌与已受伤男性各回复 1；界版弃 1 张手牌，体力高者摸 1、低者回复 1。
    if crate::skills::has(st, idx, "jieyin")
        && st.counter("jieyin_used") == 0
        && st.seats[idx].hand.len()
            >= if crate::skills::is_jiexian(st, idx) {
                1
            } else {
                2
            }
        && st.alive_indices().into_iter().any(|i| {
            i != idx
                && crate::skills::is_male(st, i)
                && (crate::skills::is_jiexian(st, idx) || st.seats[i].hp < st.seats[i].max_hp)
        })
    {
        out.push(Action {
            id: "skill:jieyin".into(),
            description: if crate::skills::is_jiexian(st, idx) {
                "结姻：弃 1 张手牌，与一名男性角色各按体力摸牌/回复".into()
            } else {
                "结姻：弃 2 张手牌，与一名已受伤男性各回复 1 点体力".into()
            },
        });
    }
    // 清侧（毌丘俭·鸿举觉醒）：移去一张「荣」，弃置其他角色一张装备/延时锦囊。
    if crate::skills::has(st, idx, "qingce")
        && !crate::skills::mark_cards(st, idx, "rong").is_empty()
        && st.alive_indices().into_iter().any(|i| {
            i != idx
                && (st.seats[i].equips.values().any(|c| c.is_some())
                    || !st.seats[i].judge_zone.is_empty())
        })
    {
        out.push(Action {
            id: "skill:qingce".into(),
            description: "清侧：移去一张「荣」，弃置其他角色一张装备/延时锦囊".into(),
        });
    }
    // 怀柔（陆抗·破势觉醒）：重铸装备牌（弃置并摸一张）。
    if crate::skills::has(st, idx, "huairou") {
        for card_id in hand_ids(st, idx) {
            let is_equip = crate::data::card_def(
                &st.cards
                    .get(&card_id)
                    .map(|c| c.def.as_str())
                    .unwrap_or_default(),
            )
            .is_some_and(|d| d.equip_slot.is_some());
            if is_equip {
                out.push(Action {
                    id: format!("skill:huairou:{card_id}"),
                    description: format!(
                        "怀柔：重铸【{}】（弃置并摸一张）",
                        card_label(st, &card_id)
                    ),
                });
            }
        }
    }
    // 奇策（荀攸）：所有手牌当任意普通锦囊使用。
    if crate::skills::has(st, idx, "qice")
        && st.counter("qice_used") == 0
        && !st.seats[idx].hand.is_empty()
    {
        out.push(Action {
            id: "skill:qice".into(),
            description: "奇策：将所有手牌当任意普通锦囊牌使用".into(),
        });
    }
    // 安恤（步练师）：令一名角色获得手牌更多的另一名角色一张手牌。
    if crate::skills::has(st, idx, "anxu")
        && st.counter("anxu_used") == 0
        && st.alive_indices().into_iter().any(|a| {
            a != idx
                && st
                    .alive_indices()
                    .into_iter()
                    .any(|b| b != idx && b != a && st.seats[b].hand.len() > st.seats[a].hand.len())
        })
    {
        out.push(Action {
            id: "skill:anxu".into(),
            description: "安恤：令一名角色获得另一名手牌更多的角色一张手牌".into(),
        });
    }
    // 弓骑（韩当）：弃一张牌令攻击范围无限；若为装备牌可弃置他人一张牌。
    if crate::skills::has(st, idx, "gongqi")
        && st.counter("gongqi_used") == 0
        && !st.seats[idx].hand.is_empty()
    {
        out.push(Action {
            id: "skill:gongqi".into(),
            description: "弓骑：弃一张牌，本回合攻击范围无限".into(),
        });
    }
    // 解烦（韩当，限定技）：令攻击范围包含目标角色者弃武器或令其摸 1。
    if crate::skills::has(st, idx, "jiefan")
        && !st.seats[idx].flags.contains_key("jiefan_used")
        && st.alive_indices().len() > 1
    {
        out.push(Action {
            id: "skill:jiefan".into(),
            description: "解烦：选择一名角色，攻击范围含其者弃武器或令其摸 1".into(),
        });
    }
    // 矫诏（郭皇后）：展示一张手牌，令最近角色声明基本牌。
    if crate::skills::has(st, idx, "jiazhao")
        && st.counter("jiazhao_used") == 0
        && !st.seats[idx].hand.is_empty()
    {
        out.push(Action {
            id: "skill:jiazhao".into(),
            description: "矫诏：展示一张手牌，令最近角色声明一种基本牌".into(),
        });
    }
    // 矫诏：使用展示牌当声明的基本牌。
    for card_id in hand_ids(st, idx) {
        if let Some(code) = crate::resolve_yijiang8::jiazhao_code(st, idx, &card_id) {
            let usable = match code.as_str() {
                "sha" => {
                    !sha_targets(st, idx).is_empty()
                        && st.counter("sha_used") < crate::turn::sha_limit(st, idx)
                }
                "tao" => st.seats[idx].hp < st.seats[idx].max_hp,
                "jiu" => st.counter("jiu_used") == 0 || crate::skills::jiu_unlimited(st, idx),
                _ => false,
            };
            if usable {
                let name = match code.as_str() {
                    "sha" => "杀",
                    "tao" => "桃",
                    _ => "酒",
                };
                out.push(Action {
                    id: format!("skill:jiazhao_use:{card_id}"),
                    description: format!(
                        "矫诏：将【{}】当【{name}】使用",
                        card_label(st, &card_id)
                    ),
                });
            }
        }
    }
    // 督粮（李严）：出牌阶段限一次。
    if crate::skills::has(st, idx, "duliang")
        && st.counter("duliang_used") == 0
        && st
            .alive_indices()
            .into_iter()
            .any(|i| i != idx && !st.seats[i].hand.is_empty())
    {
        out.push(Action {
            id: "skill:duliang".into(),
            description: "督粮：获得一名其他角色一张手牌，然后令其二选一".into(),
        });
    }
    // 匡弼（孙登）：出牌阶段限一次。
    if crate::skills::has(st, idx, "kuangbi")
        && st.counter("kuangbi_used") == 0
        && crate::skills::mark_cards(st, idx, "kuangbi").is_empty()
        && st
            .alive_indices()
            .into_iter()
            .any(|i| i != idx && !st.seats[i].hand.is_empty())
    {
        out.push(Action {
            id: "skill:kuangbi".into(),
            description: "匡弼：令一名其他角色将至多三张牌置于你武将牌上".into(),
        });
    }
    // 止戈（刘虞）：手牌数大于体力值时。
    if crate::skills::has(st, idx, "zhige")
        && st.counter("zhige_used") == 0
        && st.seats[idx].hand.len() > st.seats[idx].hp.max(0) as usize
        && st
            .alive_indices()
            .into_iter()
            .any(|i| i != idx && st.in_attack_range(i, idx))
    {
        out.push(Action {
            id: "skill:zhige".into(),
            description: "止戈：令攻击范围含你的一名角色使用【杀】或交装备".into(),
        });
    }
    // 极奢（岑昏）：摸一张并令本回合手牌上限 -1。
    if crate::resolve_yijiang8::jishe_available(st, idx) {
        out.push(Action {
            id: "skill:jishe".into(),
            description: "极奢：摸一张牌，本回合手牌上限 -1".into(),
        });
    }
    // 讥谀（孙资刘放）：令一名角色弃一张手牌并封锁花色。
    if crate::resolve_yijiang8::jiyu_available(st, idx) {
        out.push(Action {
            id: "skill:jiyu".into(),
            description: "讥谀：令一名角色弃置一张手牌，本回合不能使用该花色".into(),
        });
    }
    // 滔乱（张让）：一张牌当任意基本牌/普通锦囊（每种牌名限一次）。
    for (card_id, code) in crate::resolve_yijiang8::taoluan_options(st, idx) {
        let name = match code {
            "sha" => "杀",
            "tao" => "桃",
            "jiu" => "酒",
            "wuzhong_shengyou" => "无中生有",
            "guohe_chaiqiao" => "过河拆桥",
            "shunshou_qianyang" => "顺手牵羊",
            "juedou" => "决斗",
            "nanman_ruqin" => "南蛮入侵",
            _ => "万箭齐发",
        };
        out.push(Action {
            id: format!("skill:taoluan:{card_id}:{code}"),
            description: format!("滔乱：将【{}】当【{name}】使用", card_label(st, &card_id)),
        });
    }
    // 战绝（刘谌）：所有手牌当【决斗】使用。
    if crate::resolve_yijiang7::zhanjue_available(st, idx) {
        out.push(Action {
            id: "skill:zhanjue".into(),
            description: "战绝：将所有手牌当【决斗】使用".into(),
        });
    }
    // 勤王（刘谌，主公技）：弃一张牌发动“激将”。
    if crate::skills::has(st, idx, "qinwang")
        && crate::skills::is_lord(st, idx)
        && !st.seats[idx].hand.is_empty()
        && !crate::skills::requisition_order(st, idx, "sha").is_empty()
    {
        out.push(Action {
            id: "skill:qinwang".into(),
            description: "勤王：弃一张牌发动【激将】（响应者摸一张）".into(),
        });
    }
    // 燕语（夏侯氏）：重铸【杀】。
    if crate::skills::has(st, idx, "yanyu") {
        for card_id in hand_ids(st, idx) {
            if matches!(
                card_def(st, &card_id).map(|d| d.code),
                Some("sha" | "huo_sha" | "lei_sha")
            ) {
                out.push(Action {
                    id: format!("skill:yanyu:{card_id}"),
                    description: format!(
                        "燕语：重铸【{}】（弃置并摸一张）",
                        card_label(st, &card_id)
                    ),
                });
            }
        }
    }
    // 怃戎（张嶷）：出牌阶段限一次，与一名其他角色同时展示一张手牌。
    if crate::skills::has(st, idx, "wurong")
        && st.counter("wurong_used") == 0
        && !st.seats[idx].hand.is_empty()
        && st
            .alive_indices()
            .into_iter()
            .any(|i| i != idx && !st.seats[i].hand.is_empty())
    {
        out.push(Action {
            id: "skill:wurong".into(),
            description: "怃戎：与一名其他角色同时展示一张手牌".into(),
        });
    }
    // 宴诛（孙休）：出牌阶段限一次，令一名其他角色二选一。
    if crate::skills::has(st, idx, "yanzhu")
        && st.counter("yanzhu_used") == 0
        && st.alive_indices().into_iter().any(|i| i != idx)
    {
        out.push(Action {
            id: "skill:yanzhu".into(),
            description: "宴诛：令一名其他角色交出装备或弃牌".into(),
        });
    }
    // 诏缚（孙休，主公技 / 限定）：选择至多两名角色。
    if crate::skills::has(st, idx, "zhaofu")
        && crate::skills::is_lord(st, idx)
        && !st.seats[idx].flags.contains_key("zhaofu_used")
        && st.alive_indices().into_iter().any(|i| i != idx)
    {
        out.push(Action {
            id: "skill:zhaofu".into(),
            description: "诏缚：选择至多两名角色（视为在其他吴势力攻击范围内）".into(),
        });
    }
    // 安国（朱治）：出牌阶段限一次，按全场最少的三项条件结算。
    if crate::skills::has(st, idx, "anguo")
        && st.counter("anguo_used") == 0
        && st.alive_indices().into_iter().any(|i| i != idx)
    {
        out.push(Action {
            id: "skill:anguo".into(),
            description: "安国：令一名其他角色按最少项摸牌/回复/装备".into(),
        });
    }
    // 怀异（公孙渊）：出牌阶段限一次，展示手牌弃一种颜色并获得牌。
    if crate::skills::has(st, idx, "huaiyi")
        && st.counter("huaiyi_used") == 0
        && !st.seats[idx].hand.is_empty()
    {
        out.push(Action {
            id: "skill:huaiyi".into(),
            description: "怀异：展示所有手牌，弃一种颜色并获得牌".into(),
        });
    }
    // 忠鉴（辛宪英）：出牌阶段限一次（点数相同可再发动一次）。
    if crate::skills::has(st, idx, "zhongjian")
        && (st.counter("zhongjian_used") == 0
            || (st.counter("zhongjian_used") == 1 && st.counter("zhongjian_extra") > 0))
        && !st.seats[idx].hand.is_empty()
        && st
            .alive_indices()
            .into_iter()
            .any(|i| i != idx && !st.seats[i].hand.is_empty())
    {
        out.push(Action {
            id: "skill:zhongjian".into(),
            description: "忠鉴：展示一张手牌，查看一名其他角色体力值张手牌".into(),
        });
    }
    // 问卦（徐氏）：每名角色出牌阶段限一次，交一张牌给徐氏换摸牌。
    if st.counter(&format!("wengua:{idx}")) == 0 && !st.seats[idx].hand.is_empty() {
        if let Some(owner) = crate::resolve_yijiang9::wengua_owner(st, idx) {
            out.push(Action {
                id: format!("skill:wengua:{owner}"),
                description: "问卦：交给徐氏一张牌，然后双方从另一端各摸一张".into(),
            });
        }
    }
    // 慎行（顾雍）：弃两张牌摸一张（可重复）。
    if crate::skills::has(st, idx, "shenxing") && st.seats[idx].hand.len() >= 2 {
        out.push(Action {
            id: "skill:shenxing".into(),
            description: "慎行：弃两张牌，摸一张牌".into(),
        });
    }
    // 献州（蔡夫人，限定技）：交出所有装备牌。
    if crate::skills::has(st, idx, "xianzhou")
        && !st.seats[idx].flags.contains_key("xianzhou_used")
        && st.seats[idx].equips.values().any(|c| c.is_some())
        && st.alive_indices().into_iter().any(|i| i != idx)
    {
        out.push(Action {
            id: "skill:xianzhou".into(),
            description: "献州：将所有装备牌交给一名其他角色".into(),
        });
    }
    // 明鉴（曹叡）：出牌阶段限一次，将所有手牌交给一名其他角色。
    if crate::skills::has(st, idx, "mingjian")
        && st.counter("mingjian_used") == 0
        && !st.seats[idx].hand.is_empty()
        && st.alive_indices().into_iter().any(|i| i != idx)
    {
        out.push(Action {
            id: "skill:mingjian".into(),
            description: "明鉴：将所有手牌交给一名其他角色（其下回合手牌上限/【杀】次数 +1）"
                .into(),
        });
    }
    // 活墨（钟繇）：黑色非基本牌置于牌堆顶，视为使用不同牌名的基本牌。
    for (card_id, code) in crate::resolve_yijiang6::huomo_options(st, idx) {
        let name = match code {
            "sha" => "杀",
            "tao" => "桃",
            _ => "酒",
        };
        out.push(Action {
            id: format!("skill:huomo:{card_id}:{code}"),
            description: format!(
                "活墨：将【{}】置于牌堆顶，视为使用【{name}】",
                card_label(st, &card_id)
            ),
        });
    }
    // 父魂（关兴张苞）：两张手牌当【杀】使用。
    if crate::skills::has(st, idx, "fuhen")
        && st.counter("sha_used") < crate::turn::sha_limit(st, idx)
        && !sha_targets(st, idx).is_empty()
    {
        let hand = hand_ids(st, idx);
        for i in 0..hand.len() {
            for j in (i + 1)..hand.len() {
                out.push(Action {
                    id: format!("skill:fuhen:{},{}", hand[i], hand[j]),
                    description: format!(
                        "父魂：将【{}】【{}】当【杀】使用",
                        card_label(st, &hand[i]),
                        card_label(st, &hand[j])
                    ),
                });
            }
        }
    }
    // 品第（陈群）：弃本阶段未用过的类型牌，令一名角色摸/弃 X 张。
    if crate::skills::has(st, idx, "pindi")
        && !crate::resolve_yijiang5::pindi_cards(st, idx).is_empty()
        && st
            .alive_indices()
            .into_iter()
            .any(|i| i != idx && st.counter(&format!("pindi_target:{i}")) == 0)
    {
        out.push(Action {
            id: "skill:pindi".into(),
            description: "品第：弃一张本阶段未用过的类型牌，令一名角色摸/弃 X 张".into(),
        });
    }
    // 胆守（朱然）：弃 X 张牌（X=本阶段发动次数）选择攻击范围内角色。
    if crate::skills::has(st, idx, "danshou")
        && st.seats[idx].hand.len() as u32 >= crate::resolve_yijiang4::danshou_x(st)
        && st
            .alive_indices()
            .into_iter()
            .any(|i| i != idx && st.in_attack_range(idx, i))
    {
        let x = crate::resolve_yijiang4::danshou_x(st);
        out.push(Action {
            id: "skill:danshou".into(),
            description: format!("胆守：弃 {x} 张牌，对攻击范围内一名角色发动效果"),
        });
    }
    // 灭计（李儒）：出牌阶段限一次，黑色锦囊置于牌堆顶并令一名角色弃牌。
    if crate::skills::has(st, idx, "mieji")
        && st.counter("mieji_used") == 0
        && st.seats[idx].hand.iter().any(|id| {
            let Some(c) = st.cards.get(id) else {
                return false;
            };
            (crate::skills::is_jiexian(st, idx) || crate::skills::is_black(c.suit))
                && crate::data::card_def(&c.def)
                    .is_some_and(|d| matches!(d.category, Category::Trick | Category::DelayedTrick))
        })
        && st
            .alive_indices()
            .into_iter()
            .any(|i| i != idx && !st.seats[i].hand.is_empty())
    {
        out.push(Action {
            id: "skill:mieji".into(),
            description: "灭计：将一张黑色锦囊置于牌堆顶，令一名角色弃锦囊或两张非锦囊".into(),
        });
    }
    // 焚城（李儒，限定技）：其他角色依次弃更多牌或受 2 点火焰伤害。
    if crate::skills::has(st, idx, "fencheng")
        && !st.seats[idx].flags.contains_key("fencheng_used")
        && st.alive_indices().len() > 1
    {
        out.push(Action {
            id: "skill:fencheng".into(),
            description: "焚城：其他角色依次弃更多牌，否则受到 2 点火焰伤害".into(),
        });
    }
    // 陷嗣（刘封）：其他角色可移去两张「逆」视为对刘封使用【杀】。
    if st.counter("xiansi_used") == 0 {
        if let Some(owner) = crate::skills::find_owner(st, "xiansi") {
            if owner != idx && st.seats[owner].alive && crate::skills::ni_count(st, owner) >= 2 {
                out.push(Action {
                    id: "skill:xiansi".into(),
                    description: "陷嗣：移去两张「逆」，视为对刘封使用【杀】".into(),
                });
            }
        }
    }
    // 峻刑（满宠）：出牌阶段限一次，弃任意张手牌令一名其他角色二选一。
    if crate::skills::has(st, idx, "junxing")
        && st.counter("junxing_used") == 0
        && !st.seats[idx].hand.is_empty()
        && st
            .alive_indices()
            .into_iter()
            .any(|i| i != idx && !st.seats[i].hand.is_empty())
    {
        out.push(Action {
            id: "skill:junxing".into(),
            description: "峻刑：弃任意张手牌，令一名其他角色弃异类型手牌或翻面摸牌".into(),
        });
    }
    // 排异（钟会·自立觉醒）：移去一张「权」，令一名角色摸 2；其手牌数大于你则受 1 伤。
    if crate::skills::has(st, idx, "paiyi")
        && crate::skills::quan_count(st, idx) > 0
        && st.counter("paiyi_used") == 0
    {
        out.push(Action {
            id: "skill:paiyi".into(),
            description: "排异：移去一张「权」，令一名角色摸 2 张牌".into(),
        });
    }
    // 除疠（界华佗）：出牌阶段限一次，选择势力各不相同的其他角色。
    if crate::skills::has(st, idx, "chuli")
        && st.counter("chuli_used") == 0
        && st.alive_indices().into_iter().any(|i| i != idx)
    {
        out.push(Action {
            id: "skill:chuli".into(),
            description: "除疠：弃置你与势力各不相同的其他角色各一张牌".into(),
        });
    }
    // 义绝（界关羽）：出牌阶段限一次，弃一张牌令一名其他角色展示手牌。
    if crate::skills::has(st, idx, "yijue")
        && st.counter("yijue_used") == 0
        && !st.seats[idx].hand.is_empty()
        && st
            .alive_indices()
            .into_iter()
            .any(|i| i != idx && !st.seats[i].hand.is_empty())
    {
        out.push(Action {
            id: "skill:yijue".into(),
            description: "义绝：弃一张牌，令一名其他角色展示一张手牌".into(),
        });
    }
    // 龙胆（界赵云）：桃当酒使用。
    if crate::skills::has(st, idx, "longdan")
        && crate::skills::is_jiexian(st, idx)
        && st.counter("jiu_used") == 0
    {
        for card_id in hand_ids(st, idx) {
            if crate::skills::conversion_for(st, idx, &card_id, "jiu").is_some() {
                out.push(Action {
                    id: format!("skill:longdan:{card_id}"),
                    description: format!("龙胆：将【{}】当【酒】使用", card_label(st, &card_id)),
                });
            }
        }
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
    // 连环：梅花手牌当【铁索连环】使用（界版可用装备区梅花牌）。
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
        if crate::skills::is_jiexian(st, idx) {
            for card_id in crate::resolve_jiexian::equip_card_ids(st, idx) {
                if st.cards.get(&card_id).is_some_and(|c| c.suit == Suit::Club) {
                    out.push(Action {
                        id: format!("skill:lianhuan:{card_id}"),
                        description: format!(
                            "连环：将装备【{}】当【铁索连环】使用",
                            card_label(st, &card_id)
                        ),
                    });
                }
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
        // 界火计（界卧龙诸葛亮）：装备区的红色牌也可当【火攻】。
        if crate::skills::is_jiexian(st, idx) {
            for card_id in crate::resolve_jiexian::equip_card_ids(st, idx) {
                if st
                    .cards
                    .get(&card_id)
                    .is_some_and(|c| crate::skills::is_red(c.suit))
                {
                    out.push(Action {
                        id: format!("skill:huoji:{card_id}"),
                        description: format!(
                            "火计：将装备【{}】当【火攻】使用",
                            card_label(st, &card_id)
                        ),
                    });
                }
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
                        || trick_distance_ok(st, idx, t)
                        || (crate::skills::is_jiexian(st, idx)
                            && st.counter("damage_dealt") == 0))
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
    if crate::skills::has(st, idx, "jiuchi")
        && (st.counter("jiu_used") == 0 || crate::skills::jiu_unlimited(st, idx))
    {
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
    let tiaoxin_limit = if crate::skills::is_jiexian(st, idx) { 2 } else { 1 };
    if crate::skills::has(st, idx, "tiaoxin")
        && st.counter("tiaoxin_used") < tiaoxin_limit
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
    // 长标（界祝融）：出牌阶段限一次，任意张手牌当无距离限制的【杀】。
    if crate::skills::has(st, idx, "changbiao")
        && st.counter("changbiao_used") == 0
        && !st.seats[idx].hand.is_empty()
        && st.counter("sha_used") < sha_limit
        && st.alive_indices().len() > 1
    {
        out.push(Action {
            id: "skill:changbiao".into(),
            description: "长标：将任意张手牌当无距离限制的【杀】使用（造成伤害则阶段结束摸牌）".into(),
        });
    }
    // 翦灭（界张春华）：出牌阶段限一次。
    if crate::skills::has(st, idx, "jianmie")
        && crate::skills::is_jiexian(st, idx)
        && st.counter("jianmie_used") == 0
        && !st.seats[idx].hand.is_empty()
        && st.alive_indices()
            .into_iter()
            .any(|i| i != idx && !st.seats[i].hand.is_empty())
    {
        out.push(Action {
            id: "skill:jianmie".into(),
            description: "翦灭：同时选颜色各弃手牌，弃牌多者视为使用【决斗】".into(),
        });
    }
    // 界巧说（界简雍）：出牌阶段主动拼点（每阶段一次）。
    if crate::skills::has(st, idx, "qiaoshuo")
        && crate::skills::is_jiexian(st, idx)
        && st.counter("qiaoshuo_used") == 0
        && !st.seats[idx].hand.is_empty()
        && st.alive_indices()
            .into_iter()
            .any(|i| i != idx && !st.seats[i].hand.is_empty())
    {
        out.push(Action {
            id: "skill:qiaoshuo".into(),
            description: "巧说：拼点；赢则下一张牌目标 ±1，没赢则本回合不能用锦囊".into(),
        });
    }
    // 界制霸（界孙策主公技）：出牌阶段限一次，主动与另一名其他吴势力角色拼点。
    if crate::skills::has(st, idx, "zhiba")
        && crate::skills::is_jiexian(st, idx)
        && crate::skills::is_lord(st, idx)
        && st.counter("zhiba_jie_used") == 0
        && !st.seats[idx].hand.is_empty()
        && st.alive_indices().into_iter().any(|i| {
            i != idx && crate::skills::kingdom_is(st, i, "wu") && !st.seats[i].hand.is_empty()
        })
    {
        out.push(Action {
            id: "skill:zhiba_jie".into(),
            description: "制霸：与一名其他吴势力角色拼点（赢则获得两张拼点牌）".into(),
        });
    }
    // 荐言（界徐庶，觉醒获得）：出牌阶段限一次。
    if crate::skills::has(st, idx, "jianyan") && st.counter("jianyan_used") == 0 {
        out.push(Action {
            id: "skill:jianyan".into(),
            description: "荐言：声明一种类别或颜色，亮出牌堆一张牌交给男性角色".into(),
        });
    }
    // 势斩（界华雄）：出牌阶段限两次，令一名其他角色视为对你使用【决斗】。
    if crate::skills::has(st, idx, "shizhan")
        && st.counter("shizhan_used") < 2
        && st.alive_indices().len() > 1
    {
        out.push(Action {
            id: "skill:shizhan".into(),
            description: "势斩：令一名其他角色视为对你使用【决斗】".into(),
        });
    }
    // 黄天（张角主公技）：其他群势力角色出牌阶段限一次，交给张角一张【闪】或【闪电】；
    // 界版改为【闪】或黑桃手牌。
    if st.counter("huangtian_used") == 0 && crate::skills::huangtian_owner(st, idx).is_some() {
        let jie_owner = crate::skills::huangtian_owner(st, idx)
            .is_some_and(|owner| crate::skills::is_jiexian(st, owner));
        for card_id in hand_ids(st, idx) {
            let base = card_def(st, &card_id)
                .is_some_and(|d| d.code == "shan" || (!jie_owner && d.code == "shandian"));
            let spade = jie_owner
                && st
                    .cards
                    .get(&card_id)
                    .is_some_and(|c| c.suit == Suit::Spade);
            if base || spade {
                out.push(Action {
                    id: format!("skill:huangtian:{card_id}"),
                    description: format!("黄天：将【{}】交给张角", card_label(st, &card_id)),
                });
            }
        }
    }
    // 飞军（王平）：出牌阶段限一次，弃 1 张牌后二选一。
    if crate::skills::has(st, idx, "feijun")
        && st.counter("feijun_used") == 0
        && !st.seats[idx].hand.is_empty()
        && (!crate::resolve_sp::feijun_targets(st, idx, 1).is_empty()
            || !crate::resolve_sp::feijun_targets(st, idx, 2).is_empty())
    {
        out.push(Action {
            id: "skill:feijun".into(),
            description: "飞军：弃 1 张牌，令手牌最多者交给你一张牌或令装备最多者弃一张装备".into(),
        });
    }
    // 审时（阳，蒯越蒯良）：出牌阶段限一次，交给手牌最多者一张牌并造成 1 点伤害。
    if crate::skills::has(st, idx, "shenshi")
        && !crate::skills::yj_yin(st, idx, "shenshi")
        && st.counter("shenshi_used") == 0
        && !st.seats[idx].hand.is_empty()
        && !crate::resolve_sp::shenshi_targets(st, idx).is_empty()
    {
        out.push(Action {
            id: "skill:shenshi".into(),
            description: "审时（阳）：交给手牌最多的其他角色一张牌并对其造成 1 点伤害".into(),
        });
    }
    // 成略（许攸，转换技）：出牌阶段限一次，摸牌后弃牌，本阶段同花色牌无距离与次数限制。
    if crate::skills::has(st, idx, "chenglue")
        && st.counter("chenglue_used") == 0
        && !st.seats[idx].hand.is_empty()
    {
        out.push(Action {
            id: "skill:chenglue".into(),
            description: "成略：摸牌后弃牌，本阶段同花色牌无距离与次数限制".into(),
        });
    }
    // 贞良（卢植，转换阳）：出牌阶段限一次。
    if crate::resolve_sp2::zhenliang_available(st, idx) {
        out.push(Action {
            id: "skill:zhenliang".into(),
            description: "贞良（阳）：选攻击范围内角色，弃与「任」同色牌造成 1 点伤害".into(),
        });
    }
    // 决堰（陆抗）：出牌阶段限一次，废除一个装备栏并获得本回合效果。
    if crate::skills::has(st, idx, "jueyan")
        && st.counter("jueyan_used") == 0
        && !crate::resolve_sp2::jueyan_slots(st, idx).is_empty()
    {
        out.push(Action {
            id: "skill:jueyan".into(),
            description: "决堰：废除一个装备栏并获得本回合效果".into(),
        });
    }
    // 雄乱（张绣，限定）：指定一名其他角色，废判定区/装备区。
    if crate::skills::has(st, idx, "xiongluan")
        && !st.seats[idx].flags.contains_key("xiongluan_used")
        && st.alive_indices().len() > 1
    {
        out.push(Action {
            id: "skill:xiongluan".into(),
            description: "雄乱：废除判定区与装备区，本回合对该角色无距离与次数限制".into(),
        });
    }
    // 往烈（陈到）：基本牌/单目标普通锦囊不可响应（首张牌无距离）。
    if crate::resolve_sp2::wanglie_available(st, idx) && !crate::skills::no_card_use(st, idx) {
        for card_id in hand_ids(st, idx) {
            let Some(def) = card_def(st, &card_id) else {
                continue;
            };
            let is_sha = matches!(def.code, "sha" | "huo_sha" | "lei_sha");
            let single_trick = def.category == Category::Trick
                && !matches!(
                    def.code,
                    "juedou" | "nanman_ruqin" | "wanjian_qifa" | "wuzhong_shengyou"
                );
            if is_sha || single_trick {
                out.push(Action {
                    id: format!("skill:wanglie:{card_id}"),
                    description: format!(
                        "往烈：使用【{}】（不能被响应，本阶段不能再使用牌）",
                        card_label(st, &card_id)
                    ),
                });
            }
        }
    }
    // 酒诗（曹植）：翻至背面视为使用【酒】（正面朝上时）。
    if crate::skills::has(st, idx, "jiushi")
        && !st.seats[idx].flipped
        && st.counter("jiu_used") == 0
    {
        out.push(Action {
            id: "skill:jiushi".into(),
            description: "酒诗：翻至背面，视为使用【酒】".into(),
        });
    }
    // 陷阵（高顺）：出牌阶段限一次拼点。
    if crate::skills::has(st, idx, "xianzhen")
        && st.counter("xianzhen_used") == 0
        && !st.seats[idx].hand.is_empty()
        && st
            .alive_indices()
            .into_iter()
            .any(|i| i != idx && !st.seats[i].hand.is_empty())
    {
        out.push(Action {
            id: "skill:xianzhen".into(),
            description: "陷阵：拼点；赢则本回合对其无距离/次数限制并无视其防具".into(),
        });
    }
    // 明策（陈宫）：出牌阶段限一次，交【杀】/装备给其他角色。
    if crate::skills::has(st, idx, "mingce")
        && st.counter("mingce_used") == 0
        && !crate::resolve_yijiang::mingce_cards(st, idx).is_empty()
        && st.alive_indices().len() > 1
    {
        out.push(Action {
            id: "skill:mingce".into(),
            description: "明策：交给一名其他角色一张【杀】或装备牌".into(),
        });
    }
    // 散谣（马谡）：对体力/手牌最大者各限一次。
    if crate::skills::has(st, idx, "sanyao") && !st.seats[idx].hand.is_empty() {
        for (id, key, label) in [
            ("skill:sanyao:hp", "sanyao_hp_used", "体力值"),
            ("skill:sanyao:hand", "sanyao_hand_used", "手牌数"),
        ] {
            let mode = if key == "sanyao_hp_used" {
                crate::resolve_yijiang::SanYaoMode::Hp
            } else {
                crate::resolve_yijiang::SanYaoMode::Hand
            };
            if st.counter(key) == 0
                && !crate::resolve_yijiang::sanyao_targets(st, idx, mode).is_empty()
            {
                out.push(Action {
                    id: id.into(),
                    description: format!("散谣：弃一张牌，对{label}最大的角色造成 1 点伤害"),
                });
            }
        }
    }
    // 甘露（吴国太）：出牌阶段限一次，交换两名角色装备区。
    if crate::skills::has(st, idx, "ganlu")
        && st.counter("ganlu_used") == 0
        && st
            .alive_indices()
            .into_iter()
            .filter(|i| *i != idx && st.seats[*i].equips.values().any(|c| c.is_some()))
            .count()
            >= 2
    {
        out.push(Action {
            id: "skill:ganlu".into(),
            description: "甘露：交换两名角色装备区里的牌".into(),
        });
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
    // 国色：方块手牌当【乐不思蜀】（界版限一次，另可弃方片+场上乐不思蜀）
    if crate::skills::has(st, idx, "guose")
        && (!crate::skills::is_jiexian(st, idx) || st.counter("guose_used") == 0)
    {
        let mut has_diamond = false;
        for card_id in hand_ids(st, idx) {
            let diamond = st
                .cards
                .get(&card_id)
                .is_some_and(|c| c.suit == Suit::Diamond);
            if !diamond {
                continue;
            }
            has_diamond = true;
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
        if crate::skills::is_jiexian(st, idx)
            && has_diamond
            && crate::resolve_jiexian::find_lebu(st).is_some()
        {
            out.push(Action {
                id: "skill:guose:exchange".into(),
                description: "国色：弃一张方片牌与场上【乐不思蜀】，摸一张牌".into(),
            });
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
    // 离间：每阶段一次，需弃 1 张且至少两名男性角色（界版可弃装备牌）
    let lijian_card = !st.seats[idx].hand.is_empty()
        || (crate::skills::is_jiexian(st, idx)
            && st.seats[idx].equips.values().any(|c| c.is_some()));
    if crate::skills::has(st, idx, "lijian")
        && st.counter("lijian_used") == 0
        && lijian_card
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
    // 讥谀（孙资刘放）：本回合不能使用被封锁花色的手牌。
    let jiyu_banned: Vec<String> = hand_ids(st, idx)
        .into_iter()
        .filter(|id| crate::resolve_yijiang8::jiyu_banned(st, idx, id))
        .collect();
    if !jiyu_banned.is_empty() {
        out.retain(|a| {
            !jiyu_banned.iter().any(|c| a.id.ends_with(c.as_str()))
                && !jiyu_banned
                    .iter()
                    .any(|c| a.id.split(|ch| ch == ':' || ch == ',').any(|seg| seg == c))
        });
    }
    // 将驰（曹彰）：本回合不能使用/打出【杀】。
    if crate::skills::sha_play_banned(st, idx) {
        out.retain(|a| {
            if !a.id.starts_with("play:") && !a.id.starts_with("skill:") {
                return true;
            }
            let card =
                a.id.rsplit(':')
                    .next()
                    .and_then(|id| card_def(st, id))
                    .map(|d| matches!(d.code, "sha" | "huo_sha" | "lei_sha"))
                    .unwrap_or(false);
            !card
        });
    }
    // 巧说（简雍）没赢：本回合不能使用锦囊牌。
    if st.counter("qiaoshuo_fail") > 0 {
        out.retain(|a| {
            let Some(id) = a.id.strip_prefix("play:") else {
                return true;
            };
            !card_def(st, id)
                .is_some_and(|d| matches!(d.category, Category::Trick | Category::DelayedTrick))
        });
    }
    // 潜袭（马岱）：不能使用/打出指定颜色的手牌。
    let banned_cards: Vec<String> = hand_ids(st, idx)
        .into_iter()
        .filter(|id| crate::skills::qianxi_banned(st, idx, id))
        .collect();
    if !banned_cards.is_empty() {
        out.retain(|a| {
            !banned_cards.iter().any(|c| a.id.ends_with(c.as_str()))
                && !banned_cards
                    .iter()
                    .any(|c| a.id.split(|ch| ch == ':' || ch == ',').any(|seg| seg == c))
        });
    }
    out.push(Action {
        id: "end_play".into(),
        description: "结束出牌阶段".into(),
    });
    // 往烈：本阶段不能再使用牌（保留结束出牌）。
    // 雄乱：被指定的角色本回合不能使用/打出手牌。
    if crate::skills::no_card_use(st, idx) || crate::skills::hand_sealed(st, idx) {
        out.retain(|a| a.id == "end_play");
        return out;
    }
    // 缠怨：体力值为 1 时不能使用或打出手牌。
    if crate::skills::chanyuan_locked(st, idx) {
        out.retain(|a| {
            !a.id.starts_with("play:")
                && !a.id.starts_with("skill:wusheng:")
                && !a.id.starts_with("skill:longdan:")
                && !a.id.starts_with("skill:yijue")
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
    } else if action == "skill:yijue" {
        if !crate::skills::has(st, idx, "yijue") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("yijue_used") > 0 {
            return Err(illegal("义绝本阶段已使用过"));
        }
        if st.seats[idx].hand.is_empty() {
            return Err(illegal("没有手牌"));
        }
        st.set_counter("yijue_used", 1);
        resolve::push(
            st,
            resolve::Frame::YiJue(crate::resolve_jiexian::YiJueFrame {
                actor: idx,
                stage: crate::resolve_jiexian::YiJueStage::Card,
            }),
        );
        "义绝：请选择弃置的牌".to_string()
    } else if action.starts_with("skill:longdan:")
        && crate::skills::is_jiexian(st, idx)
        && st
            .cards
            .get(action.trim_start_matches("skill:longdan:"))
            .is_some_and(|c| c.def == "tao")
    {
        // 界龙胆：桃当酒（其余龙胆转化仍由下方统一处理）。
        let card_id = action.trim_start_matches("skill:longdan:");
        if !st.seats[idx].hand.iter().any(|c| c == card_id) {
            return Err(illegal("手牌中没有该牌"));
        }
        if !crate::skills::has(st, idx, "longdan") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("jiu_used") > 0 {
            return Err(illegal("本回合已使用过【酒】"));
        }
        let label = card_label(st, card_id);
        st.set_counter("jiu_used", 1);
        st.set_counter("jiu", 1);
        take_card(st, idx, card_id)?;
        to_discard_card(st, card_id);
        st.bump_counter("play_cards_used");
        st.push_log(
            Some(player_id),
            format!("龙胆：将【{label}】当【酒】（下一张杀 +1）"),
        );
        "已使用【酒】".to_string()
    } else if let Some((skill, card_id)) =
        ["wusheng", "longdan", "jixi", "jinjiu"]
            .iter()
            .find_map(|code| {
                action
                    .strip_prefix(&format!("skill:{code}:"))
                    .map(|card| (*code, card))
            })
    {
        if !crate::skills::has(st, idx, skill) {
            return Err(illegal("没有该技能"));
        }
        if !st.seats[idx].hand.iter().any(|c| c == card_id) {
            return Err(illegal("手牌中没有该牌"));
        }
        if crate::skills::qianxi_banned(st, idx, card_id) {
            return Err(illegal("潜袭：本回合不能使用该颜色的手牌"));
        }
        if crate::skills::sha_play_banned(st, idx) {
            return Err(illegal("将驰：本回合不能使用【杀】"));
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
        // 界武圣：方片牌当【杀】无距离限制。
        if skill == "wusheng"
            && crate::skills::is_jiexian(st, idx)
            && st
                .cards
                .get(card_id)
                .is_some_and(|c| c.suit == Suit::Diamond)
        {
            resolve::mark_sha_no_distance(st);
        }
        // 诈降（界黄盖）：红色牌当【杀】同样无距离/不可抵消。
        if crate::resolve_jiexian::zhaxiang_red_active(st, idx, card_id) {
            resolve::mark_sha_no_distance(st);
            resolve::mark_sha_no_respond(st);
        }
        st.push_log(
            Some(player_id),
            format!("{name}：将【{label}】当【杀】使用"),
        );
        "请选择【杀】的目标".to_string()
    } else if let Some(card_id) = action.strip_prefix("skill:kurou:") {
        if !crate::skills::has(st, idx, "kurou") {
            return Err(illegal("没有该技能"));
        }
        if !crate::skills::is_jiexian(st, idx) {
            return Err(illegal("标准苦肉不需要弃牌"));
        }
        if st.counter("kurou_used") > 0 {
            return Err(illegal("本阶段已发动过苦肉"));
        }
        if st.seats[idx].hp <= 0 {
            return Err(illegal("体力不足，不能发动苦肉"));
        }
        if !st.seats[idx].hand.iter().any(|c| c == card_id) {
            return Err(illegal("手牌中没有该牌"));
        }
        st.set_counter("kurou_used", 1);
        let label = card_label(st, card_id);
        take_card(st, idx, card_id)?;
        to_discard_card(st, card_id);
        st.seats[idx].hp -= 1;
        let mut ev = Vec::new();
        let drawn = draw_cards(st, idx, 2, &mut ev);
        crate::skills::zhaxiang_on_lose_hp(st, idx);
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
            format!("苦肉：弃置【{label}】并失去 1 点体力，摸 {drawn} 张（体力 {hp}）"),
        );
        if hp <= 0 {
            resolve::push_dying(st, idx, None);
        }
        "已发动苦肉".to_string()
    } else if action == "skill:kurou" {
        if !crate::skills::has(st, idx, "kurou") {
            return Err(illegal("没有该技能"));
        }
        if crate::skills::is_jiexian(st, idx) {
            return Err(illegal("界苦肉需弃置一张牌"));
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
        let jie = crate::skills::is_jiexian(st, idx);
        let need = if jie { 1 } else { 2 };
        if st.seats[idx].hand.len() < need {
            return Err(illegal(format!("结姻需弃置 {need} 张手牌")));
        }
        resolve::push(
            st,
            resolve::Frame::MultiSelect(resolve::MultiSelectFrame {
                actor: idx,
                purpose: "jieyin".into(),
                selected: Vec::new(),
                min: need,
                max: need,
                data: json!({}),
            }),
        );
        st.push_log(Some(player_id), "发动结姻");
        format!("请选择要弃置的 {need} 张牌")
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
        let from_equip = crate::resolve_jiexian::equip_card_ids(st, idx)
            .iter()
            .any(|c| c == card_id);
        if !st.seats[idx].hand.iter().any(|c| c == card_id) && !from_equip {
            return Err(illegal("手牌/装备区中没有该牌"));
        }
        if !st.cards.get(card_id).is_some_and(|c| c.suit == Suit::Club) {
            return Err(illegal("连环只能使用梅花牌"));
        }
        let label = card_label(st, card_id);
        if from_equip {
            crate::resolve_jiexian::take_equip_card(st, idx, card_id)?;
        } else {
            take_card(st, idx, card_id)?;
        }
        st.deck.processing.push(card_id.to_string());
        st.push_log(Some(player_id), format!("连环：将【{label}】当铁索连环"));
        // 界连环（界庞统）：可以多指定一个目标（先选目标再开结算）。
        if crate::skills::lianhuan_extra_target(st, idx) {
            resolve::push(
                st,
                resolve::Frame::LianHuanPick(crate::resolve_jiexian3::LianHuanPickFrame {
                    actor: idx,
                    card: card_id.to_string(),
                    first: None,
                }),
            );
            "连环：请选择目标（可多选一个）".to_string()
        } else {
            resolve::start_virtual_trick(st, idx, card_id, "tiesuo_lianhuan")?;
            "请选择铁索连环的目标".to_string()
        }
    } else if let Some(card_id) = action.strip_prefix("skill:huoji:") {
        if !crate::skills::has(st, idx, "huoji") {
            return Err(illegal("没有该技能"));
        }
        // 界火计（界卧龙诸葛亮）：手牌或装备区的红色牌。
        let from_equip = crate::resolve_jiexian::equip_card_ids(st, idx)
            .iter()
            .any(|c| c == card_id);
        if !st.seats[idx].hand.iter().any(|c| c == card_id) && !from_equip {
            return Err(illegal("手牌/装备区中没有该牌"));
        }
        if !st
            .cards
            .get(card_id)
            .is_some_and(|c| crate::skills::is_red(c.suit))
        {
            return Err(illegal("火计只能使用红色牌"));
        }
        let label = card_label(st, card_id);
        if from_equip {
            crate::resolve_jiexian::take_equip_card(st, idx, card_id)?;
        } else {
            take_card(st, idx, card_id)?;
        }
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
        st.push_log(
            Some(player_id),
            format!("乱击：将【{l1}】【{l2}】当万箭齐发"),
        );
        // 界乱击：使用【万箭齐发】可以少选一个目标。
        let jie_pick = crate::skills::is_jiexian(st, idx)
            && crate::resolve::trick_targets_for(st, "wanjian_qifa", idx, c1).len() >= 2;
        if jie_pick {
            resolve::push(
                st,
                resolve::Frame::LuanJiPick(crate::resolve_jiexian2::LuanJiPickFrame {
                    actor: idx,
                    card: c1.to_string(),
                }),
            );
            "乱击：请选择是否少选一个目标".to_string()
        } else {
            resolve::start_virtual_trick(st, idx, c1, "wanjian_qifa")?;
            let mut ev = Vec::new();
            crate::skills::on_non_delayed_trick_used(st, idx, &mut ev);
            "使用【万箭齐发】".to_string()
        }
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
        if st.counter("jiu_used") > 0 && !crate::skills::jiu_unlimited(st, idx) {
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
        if st.counter("tiaoxin_used") >= if crate::skills::is_jiexian(st, idx) { 2 } else { 1 } {
            return Err(illegal("本阶段挑衅已达上限"));
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
    } else if action == "skill:changbiao" {
        if !crate::skills::has(st, idx, "changbiao") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("changbiao_used") > 0 {
            return Err(illegal("本阶段已发动过长标"));
        }
        if st.seats[idx].hand.is_empty() {
            return Err(illegal("没有手牌"));
        }
        resolve::push(
            st,
            resolve::Frame::MultiSelect(crate::resolve::MultiSelectFrame {
                actor: idx,
                purpose: "changbiao".into(),
                selected: Vec::new(),
                min: 1,
                max: st.seats[idx].hand.len(),
                data: json!({}),
            }),
        );
        st.push_log(Some(player_id), "发动长标");
        "长标：请选择任意张手牌当【杀】".to_string()
    } else if action == "skill:jianmie" {
        if !crate::skills::has(st, idx, "jianmie") || !crate::skills::is_jiexian(st, idx) {
            return Err(illegal("没有该技能"));
        }
        if st.counter("jianmie_used") > 0 {
            return Err(illegal("本阶段已发动过翦灭"));
        }
        if st.seats[idx].hand.is_empty() {
            return Err(illegal("没有手牌"));
        }
        st.set_counter("jianmie_used", 1);
        resolve::push(
            st,
            resolve::Frame::JianMie(crate::resolve_jiexian7::JianMieFrame {
                actor: idx,
                target: None,
                stage: crate::resolve_jiexian7::JianMieStage::Target,
                owner_color: None,
            }),
        );
        st.push_log(Some(player_id), "翦灭：请选择一名其他角色");
        "翦灭：请选择一名其他角色".to_string()
    } else if action == "skill:qiaoshuo" {
        if !crate::skills::has(st, idx, "qiaoshuo") || !crate::skills::is_jiexian(st, idx) {
            return Err(illegal("没有该技能"));
        }
        if st.counter("qiaoshuo_used") > 0 {
            return Err(illegal("本阶段已发动过巧说"));
        }
        if st.seats[idx].hand.is_empty() {
            return Err(illegal("没有手牌可拼点"));
        }
        st.set_counter("qiaoshuo_used", 1);
        resolve::push(
            st,
            resolve::Frame::QiaoShuo(crate::resolve_yijiang3::QiaoShuoFrame {
                actor: idx,
                stage: crate::resolve_yijiang3::QiaoShuoStage::Target,
            }),
        );
        st.push_log(Some(player_id), "巧说：请选择拼点目标");
        "巧说：请选择拼点目标".to_string()
    } else if action == "skill:zhiba_jie" {
        if !crate::skills::has(st, idx, "zhiba")
            || !crate::skills::is_jiexian(st, idx)
            || !crate::skills::is_lord(st, idx)
        {
            return Err(illegal("没有该技能"));
        }
        if st.counter("zhiba_jie_used") > 0 {
            return Err(illegal("本阶段已发动过制霸"));
        }
        if st.seats[idx].hand.is_empty() {
            return Err(illegal("没有手牌可拼点"));
        }
        resolve::push(
            st,
            resolve::Frame::ZhiBaJie(crate::resolve_jiexian3::ZhiBaJieFrame {
                actor: idx,
                target: None,
                stage: crate::resolve_jiexian3::ZhiBaJieStage::Target,
            }),
        );
        st.push_log(Some(player_id), "制霸：选择拼点目标");
        "制霸：请选择其他吴势力角色".to_string()
    } else if action == "skill:jianyan" {
        if !crate::skills::has(st, idx, "jianyan") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("jianyan_used") > 0 {
            return Err(illegal("本阶段已发动过荐言"));
        }
        resolve::push(
            st,
            resolve::Frame::JianYan(crate::resolve_jiexian2::JianYanFrame {
                actor: idx,
                stage: crate::resolve_jiexian2::JianYanStage::Mode,
                card: None,
            }),
        );
        st.push_log(Some(player_id), "发动荐言");
        "荐言：请选择类别或颜色".to_string()
    } else if action == "skill:shizhan" {
        if !crate::skills::has(st, idx, "shizhan") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("shizhan_used") >= 2 {
            return Err(illegal("本阶段已发动过两次势斩"));
        }
        resolve::push(
            st,
            resolve::Frame::ShiZhan(crate::resolve_jiexian2::ShiZhanFrame { actor: idx }),
        );
        st.push_log(Some(player_id), "发动势斩");
        "势斩：请选择一名其他角色".to_string()
    } else if let Some(card_id) = action.strip_prefix("skill:huangtian:") {
        let owner = crate::skills::huangtian_owner(st, idx)
            .ok_or_else(|| illegal("没有可发动黄天的张角主公"))?;
        if st.counter("huangtian_used") > 0 {
            return Err(illegal("本阶段已发动过黄天"));
        }
        if !st.seats[idx].hand.iter().any(|c| c == card_id) {
            return Err(illegal("手牌中没有该牌"));
        }
        if !card_def(st, card_id).is_some_and(|d| matches!(d.code, "shan" | "shandian"))
            && !(crate::skills::is_jiexian(st, owner)
                && st
                    .cards
                    .get(card_id)
                    .is_some_and(|c| c.suit == Suit::Spade))
        {
            return Err(illegal("黄天只能交给【闪】或黑桃手牌"));
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
    } else if action == "skill:qingce" {
        if !crate::skills::has(st, idx, "qingce") {
            return Err(illegal("没有该技能"));
        }
        if crate::skills::mark_cards(st, idx, "rong").is_empty() {
            return Err(illegal("没有「荣」可移去"));
        }
        resolve::push(
            st,
            resolve::Frame::QingCe(crate::resolve_sp2::QingCeFrame { actor: idx }),
        );
        st.push_log(Some(player_id), "发动清侧");
        "清侧：请选择要弃置的场牌".to_string()
    } else if let Some(card_id) = action.strip_prefix("skill:huairou:") {
        if !crate::skills::has(st, idx, "huairou") {
            return Err(illegal("没有该技能"));
        }
        if !st.seats[idx].hand.iter().any(|c| c == card_id) {
            return Err(illegal("手牌中没有该牌"));
        }
        let is_equip = st
            .cards
            .get(card_id)
            .and_then(|c| crate::data::card_def(&c.def))
            .is_some_and(|d| d.equip_slot.is_some());
        if !is_equip {
            return Err(illegal("怀柔只能重铸装备牌"));
        }
        let label = card_label(st, card_id);
        take_card(st, idx, card_id)?;
        to_discard_card(st, card_id);
        let mut ev = Vec::new();
        let drawn = draw_cards(st, idx, 1, &mut ev);
        st.bump_counter("play_cards_used");
        st.push_log(
            Some(player_id),
            format!("怀柔：重铸【{label}】，摸 {drawn} 张"),
        );
        "怀柔：已重铸装备牌".to_string()
    } else if action == "skill:qice" {
        if !crate::skills::has(st, idx, "qice") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("qice_used") > 0 {
            return Err(illegal("本阶段已发动过奇策"));
        }
        if st.seats[idx].hand.is_empty() {
            return Err(illegal("没有手牌"));
        }
        resolve::push(
            st,
            resolve::Frame::Qice(crate::resolve_yijiang2::QiceFrame { actor: idx }),
        );
        st.push_log(Some(player_id), "发动奇策");
        "奇策：请选择要视为使用的锦囊".to_string()
    } else if action == "skill:anxu" {
        if !crate::skills::has(st, idx, "anxu") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("anxu_used") > 0 {
            return Err(illegal("本阶段已发动过安恤"));
        }
        resolve::push(
            st,
            resolve::Frame::AnXu(crate::resolve_yijiang2::AnXuFrame {
                actor: idx,
                stage: crate::resolve_yijiang2::AnXuStage::TargetA,
                a: None,
            }),
        );
        st.push_log(Some(player_id), "发动安恤");
        "安恤：请选择获得手牌的角色".to_string()
    } else if action == "skill:gongqi" {
        if !crate::skills::has(st, idx, "gongqi") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("gongqi_used") > 0 {
            return Err(illegal("本阶段已发动过弓骑"));
        }
        if st.seats[idx].hand.is_empty() {
            return Err(illegal("没有手牌可弃置"));
        }
        st.set_counter("gongqi_used", 1);
        resolve::push(
            st,
            resolve::Frame::GongQi(crate::resolve_yijiang2::GongQiFrame {
                actor: idx,
                stage: crate::resolve_yijiang2::GongQiStage::Discard,
            }),
        );
        st.push_log(Some(player_id), "发动弓骑");
        "弓骑：请选择弃置的牌".to_string()
    } else if action == "skill:jiefan" {
        if !crate::skills::has(st, idx, "jiefan") {
            return Err(illegal("没有该技能"));
        }
        if st.seats[idx].flags.contains_key("jiefan_used") {
            return Err(illegal("解烦（限定技）已使用过"));
        }
        resolve::push(
            st,
            resolve::Frame::JieFan(crate::resolve_yijiang2::JieFanFrame { actor: idx }),
        );
        st.push_log(Some(player_id), "发动解烦");
        "解烦：请选择一名角色".to_string()
    } else if action == "skill:chuli" {
        if !crate::skills::has(st, idx, "chuli") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("chuli_used") > 0 {
            return Err(illegal("本阶段已发动过除疠"));
        }
        if st.alive_indices().into_iter().all(|i| i == idx) {
            return Err(illegal("没有可选择的其他角色"));
        }
        resolve::push(
            st,
            resolve::Frame::ChuLi(crate::resolve_jiexian::ChuLiFrame {
                actor: idx,
                selected: Vec::new(),
            }),
        );
        st.push_log(Some(player_id), "发动除疠");
        "除疠：请选择势力各不相同的角色".to_string()
    } else if action == "skill:gongxin" {
        if !crate::skills::has(st, idx, "gongxin") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("gongxin_used") > 0 {
            return Err(illegal("本阶段已发动过攻心"));
        }
        if st
            .alive_indices()
            .into_iter()
            .all(|i| i == idx || st.seats[i].hand.is_empty())
        {
            return Err(illegal("没有可观看的目标"));
        }
        resolve::push(
            st,
            resolve::Frame::GongXin(crate::resolve_jiexian::GongXinFrame {
                actor: idx,
                stage: crate::resolve_jiexian::GongXinStage::Target,
                target: None,
            }),
        );
        st.push_log(Some(player_id), "发动攻心");
        "攻心：请选择观看目标".to_string()
    } else if action == "skill:guose:exchange" {
        if !crate::skills::has(st, idx, "guose") || !crate::skills::is_jiexian(st, idx) {
            return Err(illegal("没有该技能"));
        }
        if st.counter("guose_used") > 0 {
            return Err(illegal("本阶段已发动过国色"));
        }
        if crate::resolve_jiexian::find_lebu(st).is_none() {
            return Err(illegal("场上没有【乐不思蜀】"));
        }
        resolve::push(
            st,
            resolve::Frame::GuoSe(crate::resolve_jiexian::GuoSeFrame { actor: idx }),
        );
        "国色：请选择要弃置的方片牌".to_string()
    } else if let Some(card_id) = action.strip_prefix("skill:guose:") {
        if !crate::skills::has(st, idx, "guose") {
            return Err(illegal("没有该技能"));
        }
        if crate::skills::is_jiexian(st, idx) && st.counter("guose_used") > 0 {
            return Err(illegal("本阶段已发动过国色"));
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
        if crate::skills::is_jiexian(st, idx) {
            st.set_counter("guose_used", 1);
            let mut ev = Vec::new();
            let _ = draw_cards(st, idx, 1, &mut ev);
            st.push_log(Some(player_id), "国色：摸 1 张");
        }
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
        if crate::skills::is_jiexian(st, idx) {
            resolve::push(
                st,
                resolve::Frame::JieFanJian(crate::resolve_jiexian::JieFanJianFrame {
                    actor: idx,
                    stage: crate::resolve_jiexian::JieFanJianStage::Card,
                    card: None,
                    target: None,
                }),
            );
            st.push_log(Some(player_id), "发动反间");
            "反间：请选择要展示并交出的手牌".to_string()
        } else {
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
        }
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
    } else if action == "skill:feijun" {
        if !crate::skills::has(st, idx, "feijun") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("feijun_used") > 0 {
            return Err(illegal("本阶段已发动过飞军"));
        }
        if st.seats[idx].hand.is_empty() {
            return Err(illegal("飞军需弃置一张牌"));
        }
        resolve::push(
            st,
            resolve::Frame::Feijun(crate::resolve_sp::FeijunFrame {
                actor: idx,
                stage: crate::resolve_sp::FeijunStage::Choose,
                option: 1,
                target: None,
            }),
        );
        st.push_log(Some(player_id), "发动飞军");
        "飞军：请选择一项".to_string()
    } else if action == "skill:shenshi" {
        if !crate::skills::has(st, idx, "shenshi") {
            return Err(illegal("没有该技能"));
        }
        if crate::skills::yj_yin(st, idx, "shenshi") {
            return Err(illegal("审时当前为阴面，不能主动发动"));
        }
        if st.counter("shenshi_used") > 0 {
            return Err(illegal("本阶段已发动过审时"));
        }
        if st.seats[idx].hand.is_empty() {
            return Err(illegal("没有手牌可交出"));
        }
        if crate::resolve_sp::shenshi_targets(st, idx).is_empty() {
            return Err(illegal("没有可选目标"));
        }
        st.set_counter("shenshi_used", 1);
        resolve::push(
            st,
            resolve::Frame::Shenshi(crate::resolve_sp::ShenshiFrame {
                actor: idx,
                stage: crate::resolve_sp::ShenshiStage::YangSelect,
                source: None,
                card: None,
            }),
        );
        st.push_log(Some(player_id), "发动审时（阳）");
        "审时：请选择要交出的牌".to_string()
    } else if action == "skill:chenglue" {
        if !crate::skills::has(st, idx, "chenglue") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("chenglue_used") > 0 {
            return Err(illegal("本阶段已发动过成略"));
        }
        if st.seats[idx].hand.is_empty() {
            return Err(illegal("没有手牌"));
        }
        resolve::push(
            st,
            resolve::Frame::Chenglue(crate::resolve_sp::ChenglueFrame { actor: idx }),
        );
        st.push_log(Some(player_id), "发动成略");
        "成略：请确认要发动".to_string()
    } else if action == "skill:zhenliang" {
        if !crate::skills::has(st, idx, "zhenliang") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("zhenliang_used") > 0 {
            return Err(illegal("本阶段已发动过贞良"));
        }
        if !crate::resolve_sp2::zhenliang_available(st, idx) {
            return Err(illegal("贞良当前不可发动"));
        }
        resolve::push(
            st,
            resolve::Frame::ZhenLiang(crate::resolve_sp2::ZhenLiangFrame {
                actor: idx,
                stage: crate::resolve_sp2::ZhenLiangStage::Target,
                target: None,
            }),
        );
        st.push_log(Some(player_id), "发动贞良（阳）");
        "贞良：请选择目标".to_string()
    } else if action == "skill:jueyan" {
        if !crate::skills::has(st, idx, "jueyan") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("jueyan_used") > 0 {
            return Err(illegal("本阶段已发动过决堰"));
        }
        resolve::push(
            st,
            resolve::Frame::JueYan(crate::resolve_sp2::JueYanFrame { actor: idx }),
        );
        st.push_log(Some(player_id), "发动决堰");
        "决堰：请选择废除的装备栏".to_string()
    } else if action == "skill:xiongluan" {
        if !crate::skills::has(st, idx, "xiongluan") {
            return Err(illegal("没有该技能"));
        }
        if st.seats[idx].flags.contains_key("xiongluan_used") {
            return Err(illegal("雄乱（限定技）本局已发动过"));
        }
        st.seats[idx].flags.insert("xiongluan_used".into(), true);
        resolve::push(
            st,
            resolve::Frame::XiongLuan(crate::resolve_sp2::XiongLuanFrame { actor: idx }),
        );
        st.push_log(Some(player_id), "发动雄乱");
        "雄乱：请选择目标角色".to_string()
    } else if let Some(card_id) = action.strip_prefix("skill:wanglie:") {
        if !crate::skills::has(st, idx, "wanglie") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("wanglie_used") > 0 || crate::skills::no_card_use(st, idx) {
            return Err(illegal("本阶段已发动过往烈"));
        }
        if !st.seats[idx].hand.iter().any(|c| c == card_id) {
            return Err(illegal("手牌中没有该牌"));
        }
        let def = card_def(st, card_id).ok_or_else(|| illegal("未知牌"))?;
        let is_sha = matches!(def.code, "sha" | "huo_sha" | "lei_sha");
        let single_trick = def.category == Category::Trick
            && !matches!(
                def.code,
                "juedou" | "nanman_ruqin" | "wanjian_qifa" | "wuzhong_shengyou"
            );
        if !is_sha && !single_trick {
            return Err(illegal("往烈只能用于【杀】或单目标普通锦囊"));
        }
        let first = st.counter("play_cards_used") == 0;
        st.set_counter("wanglie_used", 1);
        let label = card_label(st, card_id);
        if is_sha {
            let dtype = match def.code {
                "huo_sha" => resolve::DamageType::Fire,
                "lei_sha" => resolve::DamageType::Thunder,
                _ => resolve::DamageType::Normal,
            };
            let free = first
                || crate::skills::chenglue_free(st, idx, card_id)
                || crate::skills::no_distance_turn(st, idx);
            take_card(st, idx, card_id)?;
            resolve::start_sha_flags(st, idx, card_id, false, dtype, free, false)?;
            // 标记为不可响应。
            if let Some(resolve::Frame::Sha(mut f)) = st.resolution.pop() {
                f.no_respond = true;
                st.resolution.push(resolve::Frame::Sha(f));
            }
            st.push_log(
                Some(player_id),
                format!("往烈：使用【{label}】（不能被响应）"),
            );
            st.set_counter("no_card_use", 1);
            "往烈：请选择【杀】的目标".to_string()
        } else {
            take_card(st, idx, card_id)?;
            st.deck.processing.push(card_id.to_string());
            resolve::start_virtual_trick(st, idx, card_id, def.code)?;
            // 标记无懈窗口关闭。
            for frame in st.resolution.iter_mut().rev() {
                if let resolve::Frame::Trick(t) = frame {
                    t.no_wuxie = true;
                    break;
                }
            }
            st.push_log(
                Some(player_id),
                format!("往烈：使用【{label}】（不能被无懈可击响应）"),
            );
            st.set_counter("no_card_use", 1);
            "往烈：已使用（不可响应）".to_string()
        }
    } else if action == "skill:jiushi" {
        if !crate::skills::has(st, idx, "jiushi") {
            return Err(illegal("没有该技能"));
        }
        if st.seats[idx].flipped {
            return Err(illegal("已背面朝上"));
        }
        resolve::push(
            st,
            resolve::Frame::JiuShi(crate::resolve_yijiang::JiuShiFrame { actor: idx }),
        );
        st.push_log(Some(player_id), "发动酒诗");
        "酒诗：请确认".to_string()
    } else if action == "skill:xianzhen" {
        if !crate::skills::has(st, idx, "xianzhen") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("xianzhen_used") > 0 {
            return Err(illegal("本阶段已发动过陷阵"));
        }
        if st.seats[idx].hand.is_empty() {
            return Err(illegal("没有手牌可拼点"));
        }
        st.set_counter("xianzhen_used", 1);
        resolve::push(
            st,
            resolve::Frame::XianZhen(crate::resolve_yijiang::XianZhenFrame {
                actor: idx,
                target: None,
                stage: crate::resolve_yijiang::XianZhenStage::Target,
            }),
        );
        st.push_log(Some(player_id), "发动陷阵");
        "陷阵：请选择拼点目标".to_string()
    } else if action == "skill:mingce" {
        if !crate::skills::has(st, idx, "mingce") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("mingce_used") > 0 {
            return Err(illegal("本阶段已发动过明策"));
        }
        if crate::resolve_yijiang::mingce_cards(st, idx).is_empty() {
            return Err(illegal("没有可交出的【杀】或装备牌"));
        }
        st.set_counter("mingce_used", 1);
        resolve::push(
            st,
            resolve::Frame::MingCe(crate::resolve_yijiang::MingCeFrame {
                actor: idx,
                card: None,
                target: None,
                stage: crate::resolve_yijiang::MingCeStage::Card,
            }),
        );
        st.push_log(Some(player_id), "发动明策");
        "明策：请选择要交出的牌".to_string()
    } else if action == "skill:sanyao:hp" || action == "skill:sanyao:hand" {
        if !crate::skills::has(st, idx, "sanyao") {
            return Err(illegal("没有该技能"));
        }
        let hp_mode = action.ends_with(":hp");
        let key = if hp_mode {
            "sanyao_hp_used"
        } else {
            "sanyao_hand_used"
        };
        if st.counter(key) > 0 {
            return Err(illegal("本阶段已发动过该散谣选项"));
        }
        if st.seats[idx].hand.is_empty() {
            return Err(illegal("没有手牌可弃置"));
        }
        let mode = if hp_mode {
            crate::resolve_yijiang::SanYaoMode::Hp
        } else {
            crate::resolve_yijiang::SanYaoMode::Hand
        };
        resolve::push(
            st,
            resolve::Frame::SanYao(crate::resolve_yijiang::SanYaoFrame {
                actor: idx,
                mode,
                stage: crate::resolve_sp::YiliStage::Cost,
            }),
        );
        st.push_log(Some(player_id), "发动散谣");
        "散谣：请选择要弃置的牌".to_string()
    } else if action == "skill:ganlu" {
        if !crate::skills::has(st, idx, "ganlu") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("ganlu_used") > 0 {
            return Err(illegal("本阶段已发动过甘露"));
        }
        resolve::push(
            st,
            resolve::Frame::GanLu(crate::resolve_yijiang::GanLuFrame {
                actor: idx,
                stage: crate::resolve_yijiang::GanLuStage::First,
                first: None,
            }),
        );
        st.push_log(Some(player_id), "发动甘露");
        "甘露：请选择第一名角色".to_string()
    } else if let Some(pair) = action.strip_prefix("skill:fuhen:") {
        let (c1, c2) = pair
            .split_once(',')
            .ok_or_else(|| illegal("父魂需要两张手牌"))?;
        crate::resolve_yijiang5::use_fuhen_sha(st, idx, c1, c2)?
    } else if action == "skill:jiazhao" {
        if !crate::skills::has(st, idx, "jiazhao") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("jiazhao_used") > 0 {
            return Err(illegal("本阶段已发动过矫诏"));
        }
        if st.seats[idx].hand.is_empty() {
            return Err(illegal("没有手牌"));
        }
        resolve::push(
            st,
            resolve::Frame::JiaoZhao(crate::resolve_yijiang8::JiaoZhaoFrame {
                actor: idx,
                stage: crate::resolve_yijiang8::JiaoZhaoStage::Card,
                card: None,
                declaimer: idx,
            }),
        );
        st.push_log(Some(player_id), "发动矫诏");
        "矫诏：请选择展示的手牌".to_string()
    } else if let Some(card_id) = action.strip_prefix("skill:jiazhao_use:") {
        crate::resolve_yijiang8::use_jiazhao(st, idx, card_id)?
    } else if action == "skill:duliang" {
        if !crate::skills::has(st, idx, "duliang") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("duliang_used") > 0 {
            return Err(illegal("本阶段已发动过督粮"));
        }
        resolve::push(
            st,
            resolve::Frame::DuLiang(crate::resolve_yijiang8::DuLiangFrame {
                actor: idx,
                stage: crate::resolve_yijiang8::DuLiangStage::Target,
                target: None,
            }),
        );
        st.push_log(Some(player_id), "发动督粮");
        "督粮：请选择目标".to_string()
    } else if action == "skill:kuangbi" {
        if !crate::skills::has(st, idx, "kuangbi") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("kuangbi_used") > 0 {
            return Err(illegal("本阶段已发动过匡弼"));
        }
        resolve::push(
            st,
            resolve::Frame::KuangBi(crate::resolve_yijiang8::KuangBiFrame {
                actor: idx,
                stage: crate::resolve_yijiang8::KuangBiStage::Target,
                helper: None,
                selected: Vec::new(),
            }),
        );
        st.push_log(Some(player_id), "发动匡弼");
        "匡弼：请选择目标".to_string()
    } else if action == "skill:zhige" {
        if !crate::skills::has(st, idx, "zhige") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("zhige_used") > 0 {
            return Err(illegal("本阶段已发动过止戈"));
        }
        resolve::push(
            st,
            resolve::Frame::ZhiGe(crate::resolve_yijiang8::ZhiGeFrame {
                actor: idx,
                stage: crate::resolve_yijiang8::ZhiGeStage::Target,
                target: None,
            }),
        );
        st.push_log(Some(player_id), "发动止戈");
        "止戈：请选择目标".to_string()
    } else if action == "skill:jishe" {
        crate::resolve_yijiang8::use_jishe(st, idx)?
    } else if action == "skill:jiyu" {
        if !crate::resolve_yijiang8::jiyu_available(st, idx) {
            return Err(illegal("讥谀不可用"));
        }
        resolve::push(
            st,
            resolve::Frame::JiYu(crate::resolve_yijiang8::JiYuFrame { actor: idx }),
        );
        st.push_log(Some(player_id), "发动讥谀");
        "讥谀：请选择目标".to_string()
    } else if let Some(rest) = action.strip_prefix("skill:taoluan:") {
        let (card, code) = rest
            .split_once(':')
            .ok_or_else(|| illegal("滔乱需要选择牌与牌名"))?;
        crate::resolve_yijiang8::use_taoluan(st, idx, card, code)?
    } else if action == "skill:zhanjue" {
        crate::resolve_yijiang7::use_zhanjue(st, idx)?
    } else if action == "skill:qinwang" {
        if !crate::skills::has(st, idx, "qinwang") || !crate::skills::is_lord(st, idx) {
            return Err(illegal("没有该技能"));
        }
        if st.seats[idx].hand.is_empty() {
            return Err(illegal("勤王需要弃置一张牌"));
        }
        let order = crate::skills::requisition_order(st, idx, "sha");
        if order.is_empty() {
            return Err(illegal("没有可响应的同势力角色"));
        }
        let cost = hand_ids(st, idx)
            .into_iter()
            .next()
            .ok_or_else(|| illegal("没有手牌"))?;
        let label = card_label(st, &cost);
        take_card(st, idx, &cost)?;
        to_discard_card(st, &cost);
        st.set_counter("qinwang", 1);
        resolve::push(
            st,
            resolve::Frame::Requisition(crate::resolve_more::RequisitionFrame {
                lord: idx,
                needed: "sha".into(),
                order,
                cursor: 0,
            }),
        );
        st.push_log(Some(player_id), format!("勤王：弃置【{label}】发动激将"));
        "勤王：等待同势力角色代出【杀】".to_string()
    } else if let Some(card_id) = action.strip_prefix("skill:yanyu:") {
        crate::resolve_yijiang7::recast_sha(st, idx, card_id)?
    } else if action == "skill:wurong" {
        if !crate::skills::has(st, idx, "wurong") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("wurong_used") > 0 {
            return Err(illegal("本阶段已发动过怃戎"));
        }
        resolve::push(
            st,
            resolve::Frame::WuRong(crate::resolve_yijiang7::WuRongFrame {
                actor: idx,
                stage: crate::resolve_yijiang7::WuRongStage::Card,
                card: None,
                target: None,
            }),
        );
        st.push_log(Some(player_id), "发动怃戎");
        "怃戎：请选择要展示的手牌".to_string()
    } else if action == "skill:yanzhu" {
        if !crate::skills::has(st, idx, "yanzhu") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("yanzhu_used") > 0 {
            return Err(illegal("本阶段已发动过宴诛"));
        }
        st.set_counter("yanzhu_used", 1);
        resolve::push(
            st,
            resolve::Frame::YanZhu(crate::resolve_yijiang7::YanZhuFrame {
                actor: idx,
                stage: crate::resolve_yijiang7::YanZhuStage::Target,
                target: None,
            }),
        );
        st.push_log(Some(player_id), "发动宴诛");
        "宴诛：请选择目标".to_string()
    } else if action == "skill:zhaofu" {
        if !crate::skills::has(st, idx, "zhaofu") || !crate::skills::is_lord(st, idx) {
            return Err(illegal("没有该技能"));
        }
        if st.seats[idx].flags.contains_key("zhaofu_used") {
            return Err(illegal("诏缚（限定技）已使用过"));
        }
        resolve::push(
            st,
            resolve::Frame::ZhaoFu(crate::resolve_yijiang7::ZhaoFuFrame {
                actor: idx,
                selected: Vec::new(),
            }),
        );
        st.push_log(Some(player_id), "发动诏缚");
        "诏缚：请选择至多两名角色".to_string()
    } else if action == "skill:anguo" {
        if !crate::skills::has(st, idx, "anguo") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("anguo_used") > 0 {
            return Err(illegal("本阶段已发动过安国"));
        }
        resolve::push(
            st,
            resolve::Frame::AnGuo(crate::resolve_yijiang7::AnGuoFrame { actor: idx }),
        );
        st.push_log(Some(player_id), "发动安国");
        "安国：请选择目标".to_string()
    } else if action == "skill:huaiyi" {
        if !crate::skills::has(st, idx, "huaiyi") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("huaiyi_used") > 0 {
            return Err(illegal("本阶段已发动过怀异"));
        }
        if st.seats[idx].hand.is_empty() {
            return Err(illegal("没有手牌"));
        }
        resolve::push(
            st,
            resolve::Frame::HuaiYi(crate::resolve_yijiang7::HuaiYiFrame {
                actor: idx,
                stage: crate::resolve_yijiang7::HuaiYiStage::Color,
                remaining: 0,
                gained: 0,
            }),
        );
        st.push_log(Some(player_id), "发动怀异");
        "怀异：请选择弃置的颜色".to_string()
    } else if action == "skill:zhongjian" {
        if !crate::skills::has(st, idx, "zhongjian") {
            return Err(illegal("没有该技能"));
        }
        resolve::push(
            st,
            resolve::Frame::ZhongJian(crate::resolve_yijiang9::ZhongJianFrame {
                actor: idx,
                stage: crate::resolve_yijiang9::ZhongJianStage::Card,
                card: None,
                target: None,
            }),
        );
        st.push_log(Some(player_id), "发动忠鉴");
        "忠鉴：请选择展示的手牌".to_string()
    } else if let Some(owner_str) = action.strip_prefix("skill:wengua:") {
        if st.counter(&format!("wengua:{idx}")) > 0 {
            return Err(illegal("本阶段已发动过问卦"));
        }
        let owner: usize = owner_str.parse().map_err(|_| illegal("问卦目标无效"))?;
        if !crate::skills::has(st, owner, "wengua") {
            return Err(illegal("该角色没有问卦"));
        }
        resolve::push(
            st,
            resolve::Frame::WenGua(crate::resolve_yijiang9::WenGuaFrame {
                actor: idx,
                owner,
                stage: crate::resolve_yijiang9::WenGuaStage::Card,
                card: None,
            }),
        );
        st.push_log(Some(player_id), "发动问卦");
        "问卦：请选择交给徐氏的牌".to_string()
    } else if action == "skill:shenxing" {
        if !crate::skills::has(st, idx, "shenxing") {
            return Err(illegal("没有该技能"));
        }
        if st.seats[idx].hand.len() < 2 {
            return Err(illegal("慎行需要至少两张手牌"));
        }
        resolve::push(
            st,
            resolve::Frame::ShenXing(crate::resolve_yijiang6::ShenXingFrame {
                actor: idx,
                selected: Vec::new(),
            }),
        );
        st.push_log(Some(player_id), "发动慎行");
        "慎行：请选择弃置的 2 张牌".to_string()
    } else if action == "skill:xianzhou" {
        if !crate::skills::has(st, idx, "xianzhou") {
            return Err(illegal("没有该技能"));
        }
        if st.seats[idx].flags.contains_key("xianzhou_used") {
            return Err(illegal("献州（限定技）已使用过"));
        }
        resolve::push(
            st,
            resolve::Frame::XianZhou(crate::resolve_yijiang6::XianZhouFrame {
                actor: idx,
                stage: crate::resolve_yijiang6::XianZhouStage::Target,
                receiver: None,
                x: 0,
            }),
        );
        st.push_log(Some(player_id), "发动献州");
        "献州：请选择接收角色".to_string()
    } else if action == "skill:mingjian" {
        if !crate::skills::has(st, idx, "mingjian") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("mingjian_used") > 0 {
            return Err(illegal("本阶段已发动过明鉴"));
        }
        if st.seats[idx].hand.is_empty() {
            return Err(illegal("没有手牌"));
        }
        resolve::push(
            st,
            resolve::Frame::MingJian(crate::resolve_yijiang6::MingJianFrame { actor: idx }),
        );
        st.push_log(Some(player_id), "发动明鉴");
        "明鉴：请选择目标".to_string()
    } else if let Some(rest) = action.strip_prefix("skill:huomo:") {
        if !crate::skills::has(st, idx, "huomo") {
            return Err(illegal("没有该技能"));
        }
        let (card, code) = rest
            .split_once(':')
            .ok_or_else(|| illegal("活墨需要选择牌与基本牌名"))?;
        crate::resolve_yijiang6::use_huomo(st, idx, card, code)?
    } else if action == "skill:pindi" {
        if !crate::skills::has(st, idx, "pindi") {
            return Err(illegal("没有该技能"));
        }
        if crate::resolve_yijiang5::pindi_cards(st, idx).is_empty() {
            return Err(illegal("没有可弃置的类型"));
        }
        resolve::push(
            st,
            resolve::Frame::PinDi(crate::resolve_yijiang5::PinDiFrame {
                actor: idx,
                stage: crate::resolve_yijiang5::PinDiStage::Card,
                x: 0,
                target: None,
            }),
        );
        st.push_log(Some(player_id), "发动品第");
        "品第：请选择要弃置的牌".to_string()
    } else if action == "skill:danshou" {
        if !crate::skills::has(st, idx, "danshou") {
            return Err(illegal("没有该技能"));
        }
        let x = crate::resolve_yijiang4::danshou_x(st);
        if (st.seats[idx].hand.len() as u32) < x {
            return Err(illegal(format!("胆守本次需弃 {x} 张牌")));
        }
        resolve::push(
            st,
            resolve::Frame::DanShou(crate::resolve_yijiang4::DanShouFrame {
                actor: idx,
                stage: crate::resolve_yijiang4::DanShouStage::Cost,
                x,
                selected: Vec::new(),
            }),
        );
        st.push_log(Some(player_id), format!("发动胆守（第 {x} 次）"));
        format!("胆守：请弃置 {x} 张牌")
    } else if action == "skill:mieji" {
        if !crate::skills::has(st, idx, "mieji") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("mieji_used") > 0 {
            return Err(illegal("本阶段已发动过灭计"));
        }
        resolve::push(
            st,
            resolve::Frame::MieJi(crate::resolve_yijiang4::MieJiFrame {
                actor: idx,
                stage: crate::resolve_yijiang4::MieJiStage::Card,
                card: None,
                target: None,
            }),
        );
        st.push_log(Some(player_id), "发动灭计");
        "灭计：请选择一张黑色锦囊牌".to_string()
    } else if action == "skill:fencheng" {
        if !crate::skills::has(st, idx, "fencheng") {
            return Err(illegal("没有该技能"));
        }
        if st.seats[idx].flags.contains_key("fencheng_used") {
            return Err(illegal("焚城（限定技）已使用过"));
        }
        st.seats[idx].flags.insert("fencheng_used".into(), true);
        resolve::push(
            st,
            resolve::Frame::FenCheng(crate::resolve_yijiang4::FenChengFrame {
                actor: idx,
                order: st.seats_from_left_of(idx),
                cursor: 0,
                last: 0,
            }),
        );
        st.push_log(Some(player_id), "发动焚城");
        "焚城：请依次选择".to_string()
    } else if action == "skill:xiansi" {
        let msg = crate::resolve_yijiang4::use_xiansi_sha(st, idx)?;
        st.push_log(Some(player_id), "发动陷嗣");
        msg
    } else if action == "skill:junxing" {
        if !crate::skills::has(st, idx, "junxing") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("junxing_used") > 0 {
            return Err(illegal("本阶段已发动过峻刑"));
        }
        if st.seats[idx].hand.is_empty() {
            return Err(illegal("没有手牌可弃置"));
        }
        st.set_counter("junxing_used", 1);
        resolve::push(
            st,
            resolve::Frame::JunXing(crate::resolve_yijiang3::JunXingFrame {
                actor: idx,
                stage: crate::resolve_yijiang3::JunXingStage::Select,
                selected: Vec::new(),
                target: None,
                types: 0,
            }),
        );
        st.push_log(Some(player_id), "发动峻刑");
        "峻刑：请选择要弃置的手牌".to_string()
    } else if action == "skill:paiyi" {
        if !crate::skills::has(st, idx, "paiyi") {
            return Err(illegal("没有该技能"));
        }
        if st.counter("paiyi_used") > 0 {
            return Err(illegal("本阶段已发动过排异"));
        }
        if crate::skills::quan_count(st, idx) == 0 {
            return Err(illegal("没有「权」"));
        }
        resolve::push(
            st,
            resolve::Frame::PaiYi(crate::resolve_yijiang3::PaiYiFrame { actor: idx }),
        );
        st.push_log(Some(player_id), "发动排异");
        "排异：请选择目标".to_string()
    } else if let Some(card_id) = action.strip_prefix("play:") {
        let msg = play_card(st, idx, card_id, &mut events)?;
        // 精策（郭淮）：记录本回合使用的花色/类型。
        crate::skills::note_card_used(st, idx, card_id);
        // 奔袭（吴懿）：本回合每使用一张牌，与其他角色距离 -1。
        if crate::skills::has(st, idx, "benxi") {
            st.bump_counter("benxi");
        }
        // 辟撰（蔡邕）：使用黑桃牌后可将牌堆顶一张置于武将牌上。
        if st.cards.get(card_id).is_some_and(|c| c.suit == Suit::Spade)
            && crate::skills::has(st, idx, "bizhuan")
        {
            crate::resolve_yijiang9::bizhuan_add(st, idx, "使用黑桃牌");
        }
        // 渐营（沮授）：与上一张牌同花色/点数则摸 1。
        crate::resolve_yijiang6::jianying_on_use(st, idx, card_id);
        // 活墨（钟繇）：记录本回合已使用过的基本牌牌名。
        if let Some(def) = card_def(st, card_id) {
            crate::resolve_yijiang6::note_basic_used(st, idx, def.code);
        }
        // 强识（张松）：使用与展示牌类型相同的牌时摸 1。
        let shown = st.counter("qiangshi_type");
        if shown > 0 && crate::resolve_yijiang5::card_type_bit_public(st, card_id) == shown {
            let drawn = draw_cards(st, idx, 1, &mut events);
            st.push_log(Some(player_id), format!("强识：使用同类牌，摸 {drawn} 张"));
        }
        msg
    } else if action == "end_play" {
        // 燕语（夏侯氏）：本阶段重铸过 ≥2 张【杀】时，可令一名男性角色摸 2。
        if crate::skills::has(st, idx, "yanyu") && st.counter("yanyu_count") >= 2 {
            resolve::push(
                st,
                resolve::Frame::YanYu(crate::resolve_yijiang7::YanYuFrame { actor: idx }),
            );
        }
        // 献图 / 司敌：出牌阶段结束时的延迟结算。
        crate::resolve_yijiang5::xiantu_end_phase(st, idx);
        crate::resolve_yijiang5::sidi_end_phase(st, idx);
        // 长标（界祝融）：本阶段长标【杀】造成过伤害 → 摸等量张。
        if crate::skills::has(st, idx, "changbiao") && st.counter("changbiao_draw") > 0 {
            let n = st.counter("changbiao_draw");
            st.set_counter("changbiao_draw", 0);
            let drawn = draw_cards(st, idx, n, &mut events);
            st.push_log(
                Some(player_id),
                format!("长标：本阶段结束时摸 {drawn} 张"),
            );
        }
        // 精策（郭淮）：出牌阶段结束时摸 X 张（X = 本回合使用过的牌的类型数）。
        if crate::skills::has(st, idx, "jingce") {
            let x = crate::skills::jingce_type_count(st);
            if x > 0 {
                let drawn = draw_cards(st, idx, x, &mut events);
                st.push_log(
                    Some(player_id),
                    format!("精策：出牌阶段结束，摸 {drawn} 张"),
                );
            }
        }
        crate::resolve_sp::chezheng_end_phase(st, idx);
        st.phase_discarded.clear();
        // 界当先（界廖化）：额外出牌阶段未造成伤害则自伤 1。
        crate::resolve_jiexian7::dangxian_jie_end(st, idx);
        // 当先（廖化）：额外出牌阶段结束后回到准备阶段继续正常回合。
        if st.counter("dangxian_extra") > 0 {
            st.set_counter("dangxian_extra", 0);
            st.turn_phase = TurnPhase::Prepare;
            st.push_log(Some(player_id), "当先：额外出牌阶段结束");
            events.push(EngineEvent::new(
                "phase_end",
                json!({ "phase": "play", "player": player_id }),
            ));
            resolve::advance(st, &mut events)?;
            return Ok(("当先：额外出牌阶段结束".to_string(), events));
        }
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
    // 界禁酒（界高顺）：其他角色不能于高顺的回合使用【酒】。
    if def.code == "jiu" && crate::resolve_jiexian6::jinjiu_blocks_jiu(st, idx) {
        return Err(illegal("界禁酒：高顺的回合其他角色不能使用【酒】"));
    }
    // 巧说（简雍）没赢：本回合不能使用锦囊牌。
    if st.counter("qiaoshuo_fail") > 0
        && matches!(def.category, Category::Trick | Category::DelayedTrick)
    {
        return Err(illegal("巧说没赢：本回合不能使用锦囊牌"));
    }
    // 潜袭（马岱）：不能使用指定颜色的手牌。
    if crate::skills::qianxi_banned(st, idx, card_id) {
        return Err(illegal("潜袭：本回合不能使用该颜色的手牌"));
    }
    // 将驰（曹彰）：本回合不能使用【杀】。
    if crate::skills::sha_play_banned(st, idx) && matches!(def.code, "sha" | "huo_sha" | "lei_sha")
    {
        return Err(illegal("将驰：本回合不能使用【杀】"));
    }
    // 复难（薛综）：本回合不能使用被其夺走的牌。
    if crate::resolve_yijiang9::funan_banned(st, idx, card_id) {
        return Err(illegal("复难：本回合不能使用此牌"));
    }
    // 才识（辛宪英）：本回合不能对自己使用牌。
    if st.seats[idx].flags.contains_key("caishi_no_self")
        && (def.code == "tao" || def.code == "wuzhong_shengyou" || def.equip_slot.is_some())
    {
        return Err(illegal("才识：本回合不能对自己使用牌"));
    }
    // 讥谀（孙资刘放）：本回合不能使用被封锁花色的牌。
    if crate::resolve_yijiang8::jiyu_banned(st, idx, card_id) {
        return Err(illegal("讥谀：本回合不能使用该花色的牌"));
    }
    // 司敌（曹真）：本阶段不能使用被封锁颜色的牌。
    if crate::skills::sidiqu_banned(st, idx, card_id) {
        return Err(illegal("司敌：本阶段不能使用该颜色的牌"));
    }
    let actor = st.seats[idx].player_id.clone();
    let label = card_label(st, card_id);

    match def.code {
        "sha" | "huo_sha" | "lei_sha" => {
            let free = crate::skills::chenglue_free(st, idx, card_id);
            let limit = sha_limit(st, idx);
            if !free && st.counter("sha_used") >= limit {
                return Err(illegal("本出牌阶段已使用过【杀】"));
            }
            if sha_targets_with_card(st, idx, Some(card_id), free).is_empty() {
                return Err(illegal("攻击范围内没有目标"));
            }
            if crate::skills::sha_blocked(st, idx) {
                return Err(illegal("天义没赢：本回合不能使用【杀】"));
            }
            let tianyi = st.counter("tianyi") > 0;
            let fangtian = crate::skills::has_equipment_skill(st, idx, "fangtian_huaji")
                && st.seats[idx].hand.len() == 1;
            // 福绵（吴苋）：红色【杀】可多指定目标（无距离）。
            let fumian_multi = if crate::skills::has(st, idx, "fumian")
                && st.counter("fumian_multi") > 0
                && st
                    .cards
                    .get(card_id)
                    .is_some_and(|c| crate::skills::is_red(c.suit))
            {
                st.set_counter("fumian_multi", 0);
                true
            } else {
                false
            };
            if fumian_multi {
                st.set_counter("fumian_sha", 1);
            }
            // 奔袭（吴懿）：单目标【杀】且距离均为 1 时选择至多两项强化（先询问）。
            if crate::skills::has(st, idx, "benxi")
                && crate::resolve_yijiang5::benxi_all_distance_one(st, idx)
            {
                resolve::push(
                    st,
                    resolve::Frame::BenXi(crate::resolve_yijiang5::BenXiFrame {
                        actor: idx,
                        card: card_id.to_string(),
                        fangtian,
                        selected: Vec::new(),
                    }),
                );
                return Ok("奔袭：请选择强化项".to_string());
            }
            // 疠火（程普）：普通【杀】可视为火【杀】（先询问）。
            if crate::skills::has(st, idx, "lihuo") && def.code == "sha" {
                resolve::push(
                    st,
                    resolve::Frame::LiHuo(crate::resolve_yijiang2::LiHuoFrame {
                        actor: idx,
                        card: card_id.to_string(),
                        fangtian,
                        no_distance: tianyi || free,
                    }),
                );
                return Ok("疠火：请选择是否视为火【杀】".to_string());
            }
            let dtype = match def.code {
                "huo_sha" => resolve::DamageType::Fire,
                "lei_sha" => resolve::DamageType::Thunder,
                _ => resolve::DamageType::Normal,
            };
            // 疠火（程普）：火【杀】可多指定一个目标。
            let qiaoshuo = st.counter("qiaoshuo_win") > 0;
            let multi = tianyi
                || (crate::skills::has(st, idx, "lihuo") && def.code == "huo_sha")
                || qiaoshuo
                || fumian_multi;
            let no_distance = tianyi || free;
            take_card(st, idx, card_id)?;
            if qiaoshuo {
                st.set_counter("qiaoshuo_win", 0);
                st.push_log(Some(&actor), "巧说：本回合下一张【杀】可多指定一个目标");
            }
            resolve::start_sha_flags(st, idx, card_id, fangtian, dtype, no_distance, multi)?;
            // 诈降（界黄盖）：出牌阶段红杀无距离且不能被抵消。
            if crate::resolve_jiexian::zhaxiang_red_active(st, idx, card_id) {
                resolve::mark_sha_no_distance(st);
                resolve::mark_sha_no_respond(st);
            }
            st.push_log(Some(&actor), format!("使用【{label}】"));
            Ok("请选择【杀】的目标".to_string())
        }
        "tao" => {
            if st.seats[idx].hp >= st.seats[idx].max_hp {
                return Err(illegal("体力已满，不能使用【桃】"));
            }
            take_card(st, idx, card_id)?;
            to_discard_card(st, card_id);
            st.bump_counter("play_cards_used");
            // 界救援（主公技）：吴势力角色于其回合内回复体力时改为令主公回复并摸 1。
            if crate::skills::jie_jiuyuan_redirect(st, idx) {
                return Ok("救援：改为令主公回复 1 点体力".to_string());
            }
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
            st.bump_counter("play_cards_used");
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
            // 藏拙（界卧龙诸葛亮）：延时锦囊同样计入「使用过锦囊牌」。
            st.bump_counter("trick_used");
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
            if st.slot_abolished(idx, slot) {
                return Err(illegal("该装备栏已被废除"));
            }
            take_card(st, idx, card_id)?;
            st.bump_counter("play_cards_used");
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
