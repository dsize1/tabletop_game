//! 回合阶段机与出牌动作（P2.2/P2.3）。
//!
//! - 阶段机只负责 `prepare → judge → draw → play → discard → end`；
//! - 锦囊/无懈/响应/濒死/弃牌/判定全部在 `resolve.rs` 的结算栈里完成；
//! - 本文件向 `resolve` 暴露共用小工具（牌名、手牌操作、摸牌、距离）。

use crate::data;
use crate::resolve;
use crate::state::*;
use crate::{illegal, internal};
use game_engine_core::{Action, EngineError, EngineEvent};
use serde_json::json;

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
    let pos = hand
        .iter()
        .position(|c| c == card_id)
        .ok_or_else(|| illegal("手牌中没有该牌"))?;
    hand.remove(pos);
    Ok(())
}

pub(crate) fn to_discard_card(st: &mut SgsState, card_id: &str) {
    st.deck.discard.push(card_id.to_string());
}

pub(crate) fn judge_zone_has(st: &SgsState, idx: usize, def_code: &str) -> bool {
    st.seats
        .get(idx)
        .map(|s| {
            s.judge_zone
                .iter()
                .any(|id| card_def(st, id).is_some_and(|d| d.code == def_code))
        })
        .unwrap_or(false)
}

pub(crate) fn sha_targets(st: &SgsState, idx: usize) -> Vec<usize> {
    st.alive_indices()
        .into_iter()
        .filter(|t| *t != idx && st.in_attack_range(idx, *t))
        .collect()
}

/// 顺手牵羊等「不含武器范围」的距离：`seat_distance + 防御马 - 进攻马 ≤ 1`。
pub(crate) fn trick_distance_ok(st: &SgsState, from: usize, to: usize) -> bool {
    if crate::skills::trick_distance_unlimited(st, from) {
        return true;
    }
    let d = st.seat_distance(from, to);
    if d == usize::MAX {
        return false;
    }
    let effective = d as i32 + st.defense_bonus(to)
        - i32::from(st.equip_def(from, EquipSlot::HorseOffense).is_some());
    effective <= 1
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
            let pid = st.seats[st.turn].player_id.clone();
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
            st.turn_phase = TurnPhase::Judge;
            Ok(true)
        }
        TurnPhase::Judge => {
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
                st.push_log(Some(&st.seats[idx].player_id.clone()), "突袭：跳过摸牌");
                st.turn_phase = TurnPhase::Play;
                return Ok(true);
            }
            let drawn = draw_cards(st, idx, DRAW_PER_TURN, events);
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
            Ok(true)
        }
        TurnPhase::Play => {
            if st.counter("skip_play") > 0 {
                let pid = st.seats[st.turn].player_id.clone();
                st.set_counter("skip_play", 0);
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
            // 克己：出牌阶段未使用杀 → 跳过弃牌
            let limit = crate::skills::hand_limit(st, idx);
            let over = st.seats[idx].hand.len().saturating_sub(limit);
            if over > 0 {
                resolve::push_discard(st, idx, over, "discard_phase");
                // 帧结算完后会再次进入本阶段
                return Ok(true);
            }
            st.turn_phase = TurnPhase::End;
            Ok(true)
        }
        TurnPhase::End => {
            let pid = st.seats[st.turn].player_id.clone();
            events.push(EngineEvent::new(
                "phase_start",
                json!({ "phase": "end", "seat": st.turn, "player": pid }),
            ));
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
    st.special_zones.remove("judge");
    // 特殊区（观星/五谷）必须在回合内结算完毕；收尾清理防止残留投影。
    st.special_zones.remove(crate::resolve_zone::GUANXING_ZONE);
    st.special_zones.remove(crate::resolve_zone::WUGU_ZONE);

    let alive = st.alive_indices();
    if alive.len() <= 1 {
        st.phase = Phase::Finished;
        return;
    }
    let next = alive
        .iter()
        .copied()
        .find(|i| *i > cur)
        .unwrap_or_else(|| alive[0]);
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

/// 判定阶段：亮判定牌并按延时锦囊结算（判定前的无懈窗口已由 `Judge` 帧处理）。
pub(crate) fn resolve_judge_reveal(
    st: &mut SgsState,
    delayed: String,
    owner: usize,
    events: &mut Vec<EngineEvent>,
) -> Result<(), EngineError> {
    let delayed_def = card_def(st, &delayed)
        .map(|d| d.code)
        .ok_or_else(|| internal("判定区牌缺少定义"))?;
    let delayed_name = card_def(st, &delayed).map(|d| d.name).unwrap_or("延时锦囊");

    let judge_id = draw_one(st, events).ok_or_else(|| internal("牌堆不可用，无法判定"))?;
    let judge_card = st
        .cards
        .get(&judge_id)
        .cloned()
        .ok_or_else(|| internal("判定牌缺少实例"))?;
    let owner_pid = st.seats[owner].player_id.clone();

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
    let mut payload = serde_json::Map::new();
    payload.insert("owner".into(), json!(owner_pid));
    payload.insert("delayed".into(), crate::view::card_view(st, &delayed));
    payload.insert("judge_card".into(), crate::view::card_view(st, &judge_id));
    payload.insert("effect".into(), json!(effect));
    st.special_zones.insert(
        "judge".into(),
        SpecialZone {
            owner: None,
            visibility: Visibility::Public,
            payload,
        },
    );

    if delayed_def == "shandian" {
        let hit = judge_card.suit == Suit::Spade && (2..=9).contains(&judge_card.rank);
        if hit {
            drop_processing(st);
            to_discard_card(st, &delayed);
            to_discard_card(st, &judge_id);
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
    to_discard_card(st, &judge_id);

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
    let sha_limit = if st.has_equip(idx, "zhuge_liannu") {
        u32::MAX
    } else {
        1
    };
    let targets = sha_targets(st, idx);
    for card_id in hand_ids(st, idx) {
        let Some(def) = card_def(st, &card_id) else {
            continue;
        };
        let playable = match def.code {
            "sha" => st.counter("sha_used") < sha_limit && !targets.is_empty(),
            "tao" => st.seats[idx].hp < st.seats[idx].max_hp,
            "shan" | "wuxie_keji" => false,
            // 五谷丰登（P2.5）：亮牌 → 释放者先选 → 座位顺序
            "wugu_fengdeng" => true,
            "wuzhong_shengyou" => true,
            "shunshou_qianyang" => st
                .alive_indices()
                .into_iter()
                .any(|t| t != idx && trick_distance_ok(st, idx, t)),
            "lebu_sishu" => st
                .alive_indices()
                .into_iter()
                .any(|t| t != idx && !judge_zone_has(st, t, "lebu_sishu")),
            "shandian" => !judge_zone_has(st, idx, "shandian"),
            "jiedao_sharen" => st.alive_indices().into_iter().any(|a| {
                a != idx
                    && st.equip_def(a, EquipSlot::Weapon).is_some()
                    && st
                        .alive_indices()
                        .into_iter()
                        .any(|b| b != a && st.in_attack_range(a, b))
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
    out.push(Action {
        id: "end_play".into(),
        description: "结束出牌阶段".into(),
    });
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
            }),
        );
        st.push_log(Some(player_id), "发动制衡");
        "请选择要弃置的牌".to_string()
    } else if let Some(card_id) = action.strip_prefix("play:") {
        play_card(st, idx, card_id, &mut events)?
    } else if action == "end_play" {
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
    let actor = st.seats[idx].player_id.clone();
    let label = card_label(st, card_id);

    match def.code {
        "sha" => {
            let limit = if st.has_equip(idx, "zhuge_liannu") {
                u32::MAX
            } else {
                1
            };
            if st.counter("sha_used") >= limit {
                return Err(illegal("本出牌阶段已使用过【杀】"));
            }
            if sha_targets(st, idx).is_empty() {
                return Err(illegal("攻击范围内没有目标"));
            }
            take_card(st, idx, card_id)?;
            resolve::start_sha(st, idx, card_id)?;
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
        "shan" => Err(illegal("【闪】只能在响应时使用")),
        "wuxie_keji" => Err(illegal("【无懈可击】只能在响应时使用")),
        "lebu_sishu" | "shandian" => {
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
            }
            st.push_log(Some(&actor), format!("装备【{label}】"));
            Ok(format!("已装备【{label}】"))
        }
        _ => Err(illegal(format!("【{}】不能主动使用", def.name))),
    }
}
