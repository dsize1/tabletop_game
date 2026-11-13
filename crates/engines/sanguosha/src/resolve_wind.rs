//! P4.4.1 风包技能帧：狂骨 / 奇谋 / 据守 / 解围 / 奋激 / 雷击 / 神速 / 天香。
//!
//! 与 `resolve.rs` 共用结算栈与 pending；本文件只放技能专属状态机。
//! 口径（`docs/games/sanguosha/skills.md`）：可发动技能默认 `choice` 询问，
//! 锁定技自动；需要逐张选牌的技能复用 `choose_many`（可带 `data.allowed` 过滤）。

use crate::resolve::{pop, push, replace_top, top, DamageType, Frame, MultiSelectFrame};
use crate::skills;
use crate::state::*;
use crate::turn;
use crate::{illegal, internal};
use game_engine_core::EngineError;
use serde_json::json;

/// 摸牌并执行「持有上限」检查（超出部分压入弃牌帧）。
fn draw_capped(st: &mut SgsState, seat: usize, n: u32) -> usize {
    let mut events = Vec::new();
    let drawn = turn::draw_cards(st, seat, n, &mut events);
    let over = st.seats[seat]
        .hand
        .len()
        .saturating_sub(st.config.max_hand_cards as usize);
    if over > 0 {
        crate::resolve::push_discard(st, seat, over, "hand_cap");
    }
    drawn
}

fn pass_option(label: &str) -> PendingOption {
    PendingOption {
        id: "pass".into(),
        label: label.into(),
    }
}

// ---------------------------------------------------------------------------
// 狂骨（魏延）：对距离 1 以内角色造成伤害后，回复 1 点体力或摸 1 张
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KuangguFrame {
    pub actor: usize,
}

pub(crate) fn step_kuanggu(st: &mut SgsState, f: KuangguFrame) -> Result<bool, EngineError> {
    let mut options = Vec::new();
    if st.seats[f.actor].hp < st.seats[f.actor].max_hp {
        options.push(PendingOption {
            id: "choose:heal".into(),
            label: "狂骨：回复 1 点体力".into(),
        });
    }
    options.push(PendingOption {
        id: "choose:draw".into(),
        label: "狂骨：摸 1 张牌".into(),
    });
    options.push(pass_option("不发动狂骨"));
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("kuanggu".into()),
        options,
        data: json!({ "skill": "kuanggu" }),
    });
    replace_top(st, Frame::Kuanggu(f));
    Ok(true)
}

pub(crate) fn act_kuanggu(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Kuanggu(f)) => f,
        _ => return Err(internal("栈顶不是狂骨帧")),
    };
    let pid = st.seats[f.actor].player_id.clone();
    pop(st);
    match action {
        "choose:heal" => {
            st.seats[f.actor].hp += 1;
            let hp = st.seats[f.actor].hp;
            st.push_log(Some(&pid), format!("狂骨：回复 1 点体力（{hp}）"));
            Ok("狂骨：回复 1 点体力".to_string())
        }
        "choose:draw" => {
            let drawn = draw_capped(st, f.actor, 1);
            st.push_log(Some(&pid), format!("狂骨：摸 {drawn} 张"));
            Ok("狂骨：摸 1 张".to_string())
        }
        "pass" => Ok("不发动狂骨".to_string()),
        _ => Err(illegal("未知狂骨选项")),
    }
}

// ---------------------------------------------------------------------------
// 奇谋（魏延，限定技）：失去任意点体力，本回合距离 -X、杀次数 +X
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QimouFrame {
    pub actor: usize,
}

pub(crate) fn step_qimou(st: &mut SgsState, f: QimouFrame) -> Result<bool, EngineError> {
    let hp = st.seats[f.actor].hp.max(0) as u32;
    let mut options: Vec<PendingOption> = (1..=hp)
        .map(|n| PendingOption {
            id: format!("choose:qimou:{n}"),
            label: format!("失去 {n} 点体力（距离 -{n}，杀次数 +{n}）"),
        })
        .collect();
    options.push(pass_option("不发动奇谋"));
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("qimou".into()),
        options,
        data: json!({ "skill": "qimou" }),
    });
    replace_top(st, Frame::Qimou(f));
    Ok(true)
}

pub(crate) fn act_qimou(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Qimou(f)) => f,
        _ => return Err(internal("栈顶不是奇谋帧")),
    };
    let pid = st.seats[f.actor].player_id.clone();
    if action == "pass" {
        pop(st);
        return Ok("不发动奇谋".to_string());
    }
    let n: i32 = action
        .strip_prefix("choose:qimou:")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| illegal("未知奇谋选项"))?;
    if n <= 0 || n > st.seats[f.actor].hp {
        return Err(illegal("奇谋失去体力数不合法"));
    }
    pop(st);
    st.seats[f.actor].flags.insert("qimou_used".into(), true);
    st.set_counter("qimou", n as u32);
    st.seats[f.actor].hp -= n;
    let hp = st.seats[f.actor].hp;
    // 界奇谋（界魏延）：额外摸 X 张牌。
    if crate::skills::is_jiexian(st, f.actor) {
        let mut ev = Vec::new();
        let drawn = crate::turn::draw_cards(st, f.actor, n as u32, &mut ev);
        st.push_log(
            Some(&pid),
            format!("奇谋：失去 {n} 点体力（体力 {hp}），摸 {drawn} 张，距离 -{n}、杀次数 +{n}"),
        );
    } else {
        st.push_log(
            Some(&pid),
            format!("奇谋：失去 {n} 点体力（体力 {hp}），距离 -{n}、杀次数 +{n}"),
        );
    }
    if hp <= 0 {
        crate::resolve::push_dying(st, f.actor, None);
    }
    Ok(format!("奇谋：失去 {n} 点体力"))
}

// ---------------------------------------------------------------------------
// 据守（曹仁）：结束阶段翻面摸 4，弃 1 张手牌；装备牌改为使用之
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JushouFrame {
    pub actor: usize,
}

pub(crate) fn step_jushou(st: &mut SgsState, f: JushouFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("jushou".into()),
        options: vec![
            PendingOption {
                id: "choose:jushou".into(),
                label: "据守：翻面并摸 4 张，然后弃置 1 张手牌（装备牌改为使用）".into(),
            },
            pass_option("不发动据守"),
        ],
        data: json!({ "skill": "jushou" }),
    });
    replace_top(st, Frame::Jushou(f));
    Ok(true)
}

pub(crate) fn act_jushou(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Jushou(f)) => f,
        _ => return Err(internal("栈顶不是据守帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动据守".to_string());
    }
    if action != "choose:jushou" {
        return Err(illegal("未知据守选项"));
    }
    let pid = st.seats[f.actor].player_id.clone();
    st.seats[f.actor].flipped = true;
    crate::resolve_jiexian3::shebian_on_flip(st, f.actor);
    let mut ev = Vec::new();
    let drawn = turn::draw_cards(st, f.actor, 4, &mut ev);
    st.push_log(Some(&pid), format!("据守：翻面并摸 {drawn} 张"));
    if st.seats[f.actor].hand.is_empty() {
        pop(st);
        return Ok("据守：没有手牌可弃置".to_string());
    }
    replace_top(
        st,
        Frame::MultiSelect(MultiSelectFrame {
            actor: f.actor,
            purpose: "jushou".into(),
            selected: Vec::new(),
            min: 1,
            max: 1,
            data: json!({}),
        }),
    );
    Ok("据守：请选择弃置（或使用）1 张手牌".to_string())
}

// ---------------------------------------------------------------------------
// 解围（曹仁）：装备区牌当无懈（在 resolve::step_wuxie 内处理）；
// 从背面翻至正面时，可弃 1 张牌并移动场上一张牌。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JieweiStage {
    Ask,
    Discard,
    /// 解围专用：选择移动来源玩家（手牌/装备/判定区之一非空）。
    MoveSrc,
    /// 选择要移动的牌：解围=来源玩家的装备/判定区/随机手牌；巧变=全场的装备/判定区。
    Move,
    /// 选择移动的目的角色。
    MoveDest,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JieweiFrame {
    pub actor: usize,
    pub stage: JieweiStage,
    /// Move → MoveDest 之间选定的牌（随机手牌时为空）。
    #[serde(default)]
    pub card: Option<String>,
    /// 解围：已选定的移动来源玩家（巧变为 None = 全场场牌）。
    #[serde(default)]
    pub source: Option<usize>,
    /// 解围：本次移动的是来源玩家的一张随机手牌。
    #[serde(default)]
    pub hand: bool,
}

fn field_cards(st: &SgsState) -> Vec<(usize, EquipSlot, String)> {
    let mut out = Vec::new();
    for (idx, seat) in st.seats.iter().enumerate() {
        for (slot, card) in &seat.equips {
            if let Some(id) = card {
                out.push((idx, *slot, id.clone()));
            }
        }
    }
    out
}

fn field_judge_cards(st: &SgsState) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (idx, seat) in st.seats.iter().enumerate() {
        for id in &seat.judge_zone {
            out.push((idx, id.clone()));
        }
    }
    out
}

pub(crate) fn step_jiewei(st: &mut SgsState, f: JieweiFrame) -> Result<bool, EngineError> {
    match f.stage {
        JieweiStage::Ask => {
            if st.seats[f.actor].hand.is_empty()
                && st.seats[f.actor].equips.values().all(|c| c.is_none())
            {
                pop(st);
                clear_jiewei_pending(st, f.actor);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("jiewei".into()),
                options: vec![
                    PendingOption {
                        id: "choose:jiewei".into(),
                        label: "解围：弃置 1 张牌，然后移动场上一张牌".into(),
                    },
                    pass_option("不发动解围"),
                ],
                data: json!({ "skill": "jiewei" }),
            });
            replace_top(
                st,
                Frame::Jiewei(JieweiFrame {
                    actor: f.actor,
                    stage: JieweiStage::Ask,
                    card: None,
                            source: None,
                            hand: false,
                }),
            );
            Ok(true)
        }
        JieweiStage::Discard => {
            let mut options: Vec<PendingOption> = turn::hand_ids(st, f.actor)
                .iter()
                .map(|id| PendingOption {
                    id: format!("choose:{id}"),
                    label: format!("弃置【{}】", turn::card_label(st, id)),
                })
                .collect();
            for (slot, card) in &st.seats[f.actor].equips {
                if let Some(id) = card {
                    options.push(PendingOption {
                        id: format!("choose:equip:{slot:?}"),
                        label: format!("弃置装备【{}】", turn::card_label(st, id)),
                    });
                }
            }
            if options.is_empty() {
                pop(st);
                clear_jiewei_pending(st, f.actor);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("jiewei".into()),
                options,
                data: json!({ "skill": "jiewei", "stage": "discard" }),
            });
            replace_top(st, Frame::Jiewei(f));
            Ok(true)
        }
        JieweiStage::MoveSrc => {
            // 解围：先选「移动来源玩家」——手牌/装备区/判定区任一非空即可（含自己）。
            let hand_sizes: Vec<usize> = st.seats.iter().map(|s| s.hand.len()).collect();
            let equip_counts: Vec<usize> = st
                .seats
                .iter()
                .map(|s| s.equips.values().filter(|c| c.is_some()).count())
                .collect();
            let judge_counts: Vec<usize> = st.seats.iter().map(|s| s.judge_zone.len()).collect();
            let mut options: Vec<PendingOption> = Vec::new();
            for idx in st.alive_indices() {
                let (h, e, j) = (hand_sizes[idx], equip_counts[idx], judge_counts[idx]);
                if h + e + j == 0 {
                    continue;
                }
                options.push(PendingOption {
                    id: format!("msrc:{}", st.seats[idx].player_id),
                    label: format!(
                        "{}（手牌 {h} / 装备 {e} / 判定 {j}）",
                        st.seats[idx].general_name
                    ),
                });
            }
            if options.is_empty() {
                let pid = st.seats[f.actor].player_id.clone();
                st.push_log(Some(&pid), "解围：没有可移动的牌，交互结束");
                pop(st);
                clear_jiewei_pending(st, f.actor);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("jiewei".into()),
                options,
                data: json!({ "stage": "jiewei_src" }),
            });
            replace_top(
                st,
                Frame::Jiewei(JieweiFrame {
                    actor: f.actor,
                    stage: JieweiStage::MoveSrc,
                    card: None,
                    source: None,
                    hand: false,
                }),
            );
            Ok(true)
        }
        JieweiStage::Move => {
            let mut options: Vec<PendingOption> = Vec::new();
            match f.source {
                // 解围：只列来源玩家的装备/判定区 + 一张随机手牌。
                Some(src) => {
                    for (slot, card) in &st.seats[src].equips {
                        if let Some(id) = card {
                            options.push(PendingOption {
                                id: format!("move:{id}"),
                                label: format!(
                                    "移动装备【{}】（{}区）",
                                    turn::card_label(st, id),
                                    match slot {
                                        EquipSlot::Weapon => "武器",
                                        EquipSlot::Armor => "防具",
                                        EquipSlot::HorseOffense | EquipSlot::HorseDefense => "坐骑",
                                        EquipSlot::Treasure => "宝物",
                                    }
                                ),
                            });
                        }
                    }
                    for id in st.seats[src].judge_zone.clone() {
                        options.push(PendingOption {
                            id: format!("move:{id}"),
                            label: format!(
                                "移动判定区【{}】",
                                turn::card_label(st, &id)
                            ),
                        });
                    }
                    if !st.seats[src].hand.is_empty() {
                        options.push(PendingOption {
                            id: "move:hand".into(),
                            label: format!(
                                "随机移动 {} 的一张手牌（共 {} 张）",
                                st.seats[src].general_name,
                                st.seats[src].hand.len()
                            ),
                        });
                    }
                }
                // 巧变：全场（含自己）的装备/判定区场牌，不含手牌。
                None => {
                    for (idx, _slot, id) in field_cards(st) {
                        options.push(PendingOption {
                            id: format!("move:{id}"),
                            label: format!(
                                "移动 {} 的装备【{}】",
                                st.seats[idx].general_name,
                                turn::card_label(st, &id)
                            ),
                        });
                    }
                    for (idx, id) in field_judge_cards(st) {
                        options.push(PendingOption {
                            id: format!("move:{id}"),
                            label: format!(
                                "移动 {} 判定区的【{}】",
                                st.seats[idx].general_name,
                                turn::card_label(st, &id)
                            ),
                        });
                    }
                }
            }
            if options.is_empty() {
                let pid = st.seats[f.actor].player_id.clone();
                st.push_log(Some(&pid), "解围/巧变：没有可移动的牌，交互结束");
                pop(st);
                clear_jiewei_pending(st, f.actor);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("jiewei".into()),
                options,
                data: json!({ "stage": "jiewei" }),
            });
            replace_top(
                st,
                Frame::Jiewei(JieweiFrame {
                    actor: f.actor,
                    stage: JieweiStage::Move,
                    card: None,
                    source: f.source,
                    hand: false,
                }),
            );
            Ok(true)
        }
        JieweiStage::MoveDest => {
            // 目的地候选：解围的随机手牌无限制；装备牌排除同槽位已装备者；
            // 判定牌排除判定区已有同类型（同延时锦囊）者；源玩家一律排除。
            let src = f.source.or_else(|| {
                f.card
                    .as_deref()
                    .and_then(|c| locate_field_card(st, c).map(|(i, _, _)| i))
            });
            let mut options: Vec<PendingOption> = Vec::new();
            if let Some(src) = src {
                let (card_slot, card_def, is_judge) = if f.hand {
                    (None, None, false)
                } else {
                    let Some(card_id) = f.card.as_deref() else {
                        pop(st);
                        clear_jiewei_pending(st, f.actor);
                        return Ok(false);
                    };
                    let Some((_i, slot, judge)) = locate_field_card(st, card_id) else {
                        pop(st);
                        clear_jiewei_pending(st, f.actor);
                        return Ok(false);
                    };
                    let def = if judge {
                        st.effective_def_code(card_id)
                    } else {
                        None
                    };
                    (Some(slot), def, judge)
                };
                for idx in st.alive_indices() {
                    if idx == src {
                        continue;
                    }
                    let dest_name = st.seats[idx].general_name.clone();
                    if f.hand {
                        options.push(PendingOption {
                            id: format!("to:{}", st.seats[idx].player_id),
                            label: format!("将其一张手牌交给 {dest_name}"),
                        });
                        continue;
                    }
                    if is_judge {
                        let conflict = card_def.is_some_and(|d| {
                            st.seats[idx]
                                .judge_zone
                                .iter()
                                .any(|c| st.effective_def_code(c) == Some(d))
                        });
                        if conflict {
                            continue;
                        }
                        options.push(PendingOption {
                            id: format!("to:{}", st.seats[idx].player_id),
                            label: format!("移动到 {dest_name} 的判定区"),
                        });
                    } else if let Some(slot) = card_slot {
                        if st.seats[idx].equips.get(&slot).cloned().flatten().is_some() {
                            continue;
                        }
                        options.push(PendingOption {
                            id: format!("to:{}", st.seats[idx].player_id),
                            label: format!("移动到 {dest_name} 的{}区", slot_label(slot)),
                        });
                    }
                }
            }
            if options.is_empty() {
                let pid = st.seats[f.actor].player_id.clone();
                st.push_log(Some(&pid), "解围/巧变：没有可移动的目的角色，交互结束");
                pop(st);
                clear_jiewei_pending(st, f.actor);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("jiewei".into()),
                options,
                data: json!({
                    "stage": "jiewei_dest",
                    "card": if f.hand { serde_json::Value::Null } else { json!(f.card) },
                    "hand": f.hand,
                }),
            });
            replace_top(st, Frame::Jiewei(f));
            Ok(true)
        }
    }
}

/// 装备槽中文名。
fn slot_label(slot: EquipSlot) -> &'static str {
    match slot {
        EquipSlot::Weapon => "武器",
        EquipSlot::Armor => "防具",
        EquipSlot::HorseOffense | EquipSlot::HorseDefense => "坐骑",
        EquipSlot::Treasure => "宝物",
    }
}

/// 定位场上牌：返回 (所属座位, 槽位, 是否判定区)。
fn locate_field_card(st: &SgsState, card_id: &str) -> Option<(usize, EquipSlot, bool)> {
    for (idx, seat) in st.seats.iter().enumerate() {
        for (slot, card) in &seat.equips {
            if card.as_deref() == Some(card_id) {
                return Some((idx, *slot, false));
            }
        }
        if seat.judge_zone.iter().any(|c| c == card_id) {
            return Some((idx, EquipSlot::Weapon, true));
        }
    }
    None
}

/// 解围②：当你翻回正面时，可弃置 1 张牌，然后移动场上 1 张牌。
///
/// 任何「背面 → 正面」来源（放逐 / 酒诗 / 举荐复原 / 回合开始跳过）都应调用本函数；
/// 返回 true 表示已压入解围帧（由后续 advance 推进 Ask → Discard → Move）。
pub(crate) fn jiewei_on_face_up(st: &mut SgsState, seat: usize) -> bool {
    if !crate::skills::has(st, seat, "jiewei") {
        return false;
    }
    let has_card = !st.seats[seat].hand.is_empty()
        || st.seats[seat].equips.values().any(|c| c.is_some());
    if !has_card {
        return false;
    }
    if st.seats[seat].flags.contains_key("jiewei_pending") {
        return false;
    }
    st.seats[seat].flags.insert("jiewei_pending".into(), true);
    crate::resolve::push(
        st,
        crate::resolve::Frame::Jiewei(JieweiFrame {
            actor: seat,
            stage: JieweiStage::Ask,
            card: None,
                            source: None,
                            hand: false,
        }),
    );
    true
}

/// 解围帧结束：清理“本次翻回已询问”标记，避免下次翻回被永久拦截。
fn clear_jiewei_pending(st: &mut SgsState, seat: usize) {
    st.seats[seat].flags.remove("jiewei_pending");
}

pub(crate) fn act_jiewei(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Jiewei(f)) => f,
        _ => return Err(internal("栈顶不是解围帧")),
    };
    if action == "pass" {
        pop(st);
        clear_jiewei_pending(st, f.actor);
        return Ok("不发动解围".to_string());
    }
    if action != "choose:jiewei" {
        return Err(illegal("未知解围选项"));
    }
    if st.seats[f.actor].hand.is_empty() && st.seats[f.actor].equips.values().all(|c| c.is_none()) {
        return Err(illegal("没有可弃置的牌"));
    }
    replace_top(
        st,
        Frame::Jiewei(JieweiFrame {
            actor: f.actor,
            stage: JieweiStage::Discard,
            card: f.card.clone(),
            source: f.source,
            hand: f.hand,
        }),
    );
    Ok("解围：请选择弃置的牌".to_string())
}

/// 解围弃置阶段的选择（`pending.kind = choice`）。
pub(crate) fn act_jiewei_discard(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Jiewei(f)) => f,
        _ => return Err(internal("栈顶不是解围帧")),
    };
    if let Some(card_id) = action.strip_prefix("choose:equip:") {
        let slot = match card_id {
            "Weapon" => EquipSlot::Weapon,
            "Armor" => EquipSlot::Armor,
            "HorseOffense" => EquipSlot::HorseOffense,
            "HorseDefense" => EquipSlot::HorseDefense,
            "Treasure" => EquipSlot::Treasure,
            _ => return Err(illegal("未知装备槽")),
        };
        let Some(Some(id)) = st.seats[f.actor].equips.insert(slot, None) else {
            return Err(illegal("该装备槽没有牌"));
        };
        turn::to_discard_card(st, &id);
        let pid = st.seats[f.actor].player_id.clone();
        st.push_log(
            Some(&pid),
            format!("解围：弃置装备【{}】", turn::card_label(st, &id)),
        );
    } else {
        let card_id = action
            .strip_prefix("choose:")
            .ok_or_else(|| illegal("需要选择弃置的牌"))?;
        if !st.seats[f.actor].hand.iter().any(|c| c == card_id) {
            return Err(illegal("手牌中没有该牌"));
        }
        turn::take_card(st, f.actor, card_id)?;
        turn::to_discard_card(st, card_id);
        let pid = st.seats[f.actor].player_id.clone();
        st.push_log(
            Some(&pid),
            format!("解围：弃置【{}】", turn::card_label(st, card_id)),
        );
    }
    replace_top(
        st,
        Frame::Jiewei(JieweiFrame {
            actor: f.actor,
            stage: JieweiStage::MoveSrc,
            card: None,
            source: None,
            hand: false,
        }),
    );
    Ok("解围：请选择移动来源玩家".to_string())
}

/// 解围移动阶段（`pending.kind = target`）。
pub(crate) fn jiewei_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Jiewei(f)) => f,
        _ => return Err(internal("栈顶不是解围帧")),
    };
    // 解围 1/3：选择移动来源玩家。
    if let Some(src_pid) = action.strip_prefix("msrc:") {
        if !pending
            .options
            .iter()
            .any(|o| o.id == format!("msrc:{src_pid}"))
        {
            return Err(illegal("该角色不是可用的移动来源"));
        }
        let Some(src) = st.seat_index(src_pid) else {
            return Err(illegal("来源角色不在座"));
        };
        replace_top(
            st,
            Frame::Jiewei(JieweiFrame {
                actor: f.actor,
                stage: JieweiStage::Move,
                card: None,
                source: Some(src),
                hand: false,
            }),
        );
        return Ok("解围：请选择要移动的牌".to_string());
    }
    // 解围/巧变 2/3：选择要移动的牌（解围支持「随机一张手牌」）。
    if let Some(token) = action.strip_prefix("move:") {
        if !pending
            .options
            .iter()
            .any(|o| o.id == format!("move:{token}"))
        {
            return Err(illegal("该牌不可移动"));
        }
        let hand = token == "hand";
        replace_top(
            st,
            Frame::Jiewei(JieweiFrame {
                actor: f.actor,
                stage: JieweiStage::MoveDest,
                card: if hand { None } else { Some(token.to_string()) },
                source: f.source,
                hand,
            }),
        );
        return Ok("解围：请选择移动到哪里".to_string());
    }
    // 3/3：选择目的角色并结算。
    let dest_pid = action
        .strip_prefix("to:")
        .ok_or_else(|| illegal("需要选择要移动的牌 / 目的角色"))?;
    if !pending
        .options
        .iter()
        .any(|o| o.id == format!("to:{dest_pid}"))
    {
        return Err(illegal("目的角色不合法"));
    }
    let Some(dest) = st.seat_index(dest_pid) else {
        return Err(illegal("目的角色不在座"));
    };
    let pid = st.seats[f.actor].player_id.clone();
    if f.hand {
        // 手牌：从来源玩家随机一张 → 目的玩家手牌（无区域限制）。
        let Some(src) = f.source else {
            return Err(illegal("缺少来源角色"));
        };
        if st.seats[src].hand.is_empty() {
            return Err(illegal("来源角色没有手牌"));
        }
        let n = st.seats[src].hand.len();
        let idx = st.next_index(n);
        let card = st.seats[src].hand.remove(idx);
        let label = turn::card_label(st, &card);
        let src_name = st.seats[src].general_name.clone();
        let dest_name = st.seats[dest].general_name.clone();
        st.seats[dest].hand.push(card.clone());
        st.push_log(
            Some(&pid),
            format!("解围：将 {src_name} 的一张随机手牌【{label}】交给 {dest_name}"),
        );
        // 先弹出解围帧再返回；奋激由引擎在动作结束后按「回合外 + 手牌弃置/增加」差分统一判定，
        // 不再在这里按技能硬编码（避免帧顺序问题：曾导致移动后重复询问）。
        pop(st);
        clear_jiewei_pending(st, f.actor);
        return Ok(format!("解围：已移动【{label}】"));
    }
    let Some(card_id) = f.card.clone() else {
        return Err(illegal("缺少要移动的牌"));
    };
    let Some((src, slot, is_judge)) = locate_field_card(st, &card_id) else {
        return Err(illegal("场上没有该牌"));
    };
    if src == dest {
        return Err(illegal("不能移动回原角色"));
    }
    // 从原位置移除
    for seat in st.seats.iter_mut() {
        for card in seat.equips.values_mut() {
            if card.as_deref() == Some(card_id.as_str()) {
                *card = None;
            }
        }
        if let Some(pos) = seat.judge_zone.iter().position(|c| c == &card_id) {
            seat.judge_zone.remove(pos);
        }
    }
    let label = turn::card_label(st, &card_id);
    let dest_name = st.seats[dest].general_name.clone();
    if is_judge {
        st.seats[dest].judge_zone.push(card_id.clone());
    } else {
        let old = st.seats[dest].equips.insert(slot, Some(card_id.clone()));
        if let Some(Some(old_id)) = old {
            turn::to_discard_card(st, &old_id);
            skills::xiaoji_on_lose_equip(st, dest);
        }
    }
    st.push_log(
        Some(&pid),
        format!(
            "解围/巧变：将【{label}】移动到 {dest_name} 的{}",
            if is_judge { "判定区" } else { "装备区" }
        ),
    );
    pop(st);
    clear_jiewei_pending(st, f.actor);
    Ok(format!("解围：已移动【{label}】"))
}

// ---------------------------------------------------------------------------
// 奋激（周泰）：一名角色的手牌被弃置/获得后，可失去 1 点体力令其摸 2 张
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FenjiFrame {
    pub owner: usize,
    pub lost_by: usize,
}

pub(crate) fn step_fenji(st: &mut SgsState, f: FenjiFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.owner].player_id.clone(),
        skill: Some("fenji".into()),
        options: vec![
            PendingOption {
                id: "choose:fenji".into(),
                label: format!(
                    "奋激：失去 1 点体力，令 {} 摸 2 张",
                    st.seats[f.lost_by].general_name
                ),
            },
            pass_option("不发动奋激"),
        ],
        data: json!({ "skill": "fenji", "lost_by": st.seats[f.lost_by].player_id }),
    });
    replace_top(st, Frame::Fenji(f));
    Ok(true)
}

pub(crate) fn act_fenji(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Fenji(f)) => f,
        _ => return Err(internal("栈顶不是奋激帧")),
    };
    let pid = st.seats[f.owner].player_id.clone();
    if action == "pass" {
        pop(st);
        return Ok("不发动奋激".to_string());
    }
    if action != "choose:fenji" {
        return Err(illegal("未知奋激选项"));
    }
    pop(st);
    // 奋激摸牌本身属于「回合外手牌增加」，但不应再次触发奋激（自查递归）。
    st.fenji_suppress.push(f.lost_by);
    st.seats[f.owner].hp -= 1;
    let drawn = draw_capped(st, f.lost_by, 2);
    let hp = st.seats[f.owner].hp;
    st.push_log(
        Some(&pid),
        format!(
            "奋激：失去 1 点体力（{hp}），令 {} 摸 {drawn} 张",
            st.seats[f.lost_by].general_name
        ),
    );
    if hp <= 0 {
        crate::resolve::push_dying(st, f.owner, None);
    }
    Ok(format!(
        "奋激：令 {} 摸 {drawn} 张",
        st.seats[f.lost_by].general_name
    ))
}

// ---------------------------------------------------------------------------
// 雷击（张角）：使用/打出【闪】时，可令一名其他角色判定
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LeijiFrame {
    pub actor: usize,
}

pub(crate) fn step_leiji(st: &mut SgsState, f: LeijiFrame) -> Result<bool, EngineError> {
    // 界雷击：先判定，判定后按结果选择目标（黑桃 2 伤 / 梅花回复 1 + 可选 1 伤）。
    if crate::skills::is_jiexian(st, f.actor) {
        pop(st);
        st.push_log(
            Some(&st.seats[f.actor].player_id.clone()),
            "雷击：判定中",
        );
        push(
            st,
            Frame::QuickJudge(crate::resolve::QuickJudgeFrame {
                owner: f.actor,
                reason: "leiji".into(),
                stage: crate::resolve::QuickJudgeStage::Reveal,
                judge_card: None,
                rewrite_order: Vec::new(),
                rewrite_cursor: 0,
                data: json!({ "user": f.actor, "jie": true }),
                saved_outcome: None,
            }),
        );
        return Ok(false);
    }
    let mut options: Vec<PendingOption> = Vec::new();
    for t in st.alive_indices() {
        if t == f.actor {
            continue;
        }
        options.push(PendingOption {
            id: format!("target:{}", st.seats[t].player_id),
            label: format!("雷击：令 {} 判定", st.seats[t].general_name),
        });
    }
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    options.push(pass_option("不发动雷击"));
    st.pending = Some(Pending {
        kind: "target".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("leiji".into()),
        options,
        data: json!({ "stage": "leiji" }),
    });
    replace_top(st, Frame::Leiji(f));
    Ok(true)
}

pub(crate) fn leiji_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Leiji(f)) => f,
        _ => return Err(internal("栈顶不是雷击帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动雷击".to_string());
    }
    let pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择雷击目标"))?;
    if !pending
        .options
        .iter()
        .any(|o| o.id == format!("target:{pid}"))
    {
        return Err(illegal("雷击目标不合法"));
    }
    let target = st.seat_index(pid).ok_or_else(|| illegal("目标不在座"))?;
    pop(st);
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        format!("雷击：令 {} 判定", st.seats[target].general_name),
    );
    push(
        st,
        Frame::QuickJudge(crate::resolve::QuickJudgeFrame {
            owner: f.actor,
            reason: "leiji".into(),
            stage: crate::resolve::QuickJudgeStage::Reveal,
            judge_card: None,
            rewrite_order: Vec::new(),
            rewrite_cursor: 0,
            data: json!({ "user": f.actor, "target": target }),
            saved_outcome: None,
        }),
    );
    Ok("雷击：判定中".to_string())
}

// ---------------------------------------------------------------------------
// 神速（夏侯渊）：准备阶段选择至多三项，每项视为使用无距离限制【杀】
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShensuStage {
    Ask,
    PickEquip,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ShensuFrame {
    pub actor: usize,
    pub stage: ShensuStage,
    pub used: u32,
}

fn shensu_sha(st: &mut SgsState, actor: usize) {
    push(
        st,
        Frame::Sha(crate::resolve::ShaFrame {
            card: String::new(),
            user: actor,
            target: None,
            stage: crate::resolve::ShaStage::Target,
            pending_targets: Vec::new(),
            fangtian: false,
            damage_type: DamageType::Normal,
            no_distance: true,
            virtual_card: true,
            tianyi: false,
            no_respond: false,
        }),
    );
}

pub(crate) fn step_shensu(st: &mut SgsState, f: ShensuFrame) -> Result<bool, EngineError> {
    match f.stage {
        ShensuStage::Ask => {
            let mut options: Vec<PendingOption> = Vec::new();
            if st.counter("shensu_skip_judge") == 0 {
                options.push(PendingOption {
                    id: "choose:shensu:1".into(),
                    label: "神速①：跳过判定与摸牌阶段，视为使用【杀】".into(),
                });
            }
            if st.counter("shensu_skip_play") == 0
                && st.seats[f.actor]
                    .hand
                    .iter()
                    .any(|id| turn::card_def(st, id).is_some_and(|d| d.equip_slot.is_some()))
            {
                options.push(PendingOption {
                    id: "choose:shensu:2".into(),
                    label: "神速②：弃置一张装备牌并跳过出牌阶段，视为使用【杀】".into(),
                });
            }
            if st.counter("shensu_skip_discard") == 0 {
                options.push(PendingOption {
                    id: "choose:shensu:3".into(),
                    label: "神速③：翻面并跳过弃牌阶段，视为使用【杀】".into(),
                });
            }
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            options.push(pass_option("不发动神速"));
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("shensu".into()),
                options,
                data: json!({ "skill": "shensu" }),
            });
            replace_top(
                st,
                Frame::Shensu(ShensuFrame {
                    actor: f.actor,
                    stage: ShensuStage::Ask,
                    used: f.used,
                }),
            );
            Ok(true)
        }
        ShensuStage::PickEquip => {
            let mut options: Vec<PendingOption> = st.seats[f.actor]
                .hand
                .iter()
                .filter(|id| turn::card_def(st, id).is_some_and(|d| d.equip_slot.is_some()))
                .map(|id| PendingOption {
                    id: format!("choose:{id}"),
                    label: format!("弃置装备【{}】", turn::card_label(st, id)),
                })
                .collect();
            if options.is_empty() {
                // 装备牌已不在手上：本项作废，回到选择。
                replace_top(
                    st,
                    Frame::Shensu(ShensuFrame {
                        actor: f.actor,
                        stage: ShensuStage::Ask,
                        used: f.used,
                    }),
                );
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("shensu".into()),
                options: std::mem::take(&mut options),
                data: json!({ "skill": "shensu", "stage": "pick_equip" }),
            });
            replace_top(
                st,
                Frame::Shensu(ShensuFrame {
                    actor: f.actor,
                    stage: ShensuStage::PickEquip,
                    used: f.used,
                }),
            );
            Ok(true)
        }
    }
}

pub(crate) fn act_shensu(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Shensu(f)) => f,
        _ => return Err(internal("栈顶不是神速帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动神速".to_string());
    }
    match f.stage {
        ShensuStage::Ask => {
            let n: u32 = action
                .strip_prefix("choose:shensu:")
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| illegal("未知神速选项"))?;
            let pid = st.seats[f.actor].player_id.clone();
            match n {
                1 => {
                    st.set_counter("shensu_skip_judge", 1);
                    st.set_counter("skip_draw", 1);
                    st.push_log(Some(&pid), "神速①：跳过判定与摸牌阶段");
                }
                2 => {
                    replace_top(
                        st,
                        Frame::Shensu(ShensuFrame {
                            actor: f.actor,
                            stage: ShensuStage::PickEquip,
                            used: f.used,
                        }),
                    );
                    return Ok("神速②：请选择弃置的装备牌".to_string());
                }
                3 => {
                    st.set_counter("shensu_skip_discard", 1);
                    st.seats[f.actor].flipped = true;
                    crate::resolve_jiexian3::shebian_on_flip(st, f.actor);
                    st.push_log(Some(&pid), "神速③：翻面并跳过弃牌阶段");
                }
                _ => return Err(illegal("未知神速选项")),
            }
            replace_top(
                st,
                Frame::Shensu(ShensuFrame {
                    actor: f.actor,
                    stage: ShensuStage::Ask,
                    used: f.used + 1,
                }),
            );
            shensu_sha(st, f.actor);
            Ok("神速：视为使用【杀】".to_string())
        }
        ShensuStage::PickEquip => {
            let card_id = action
                .strip_prefix("choose:")
                .ok_or_else(|| illegal("需要选择弃置的装备牌"))?;
            if !st.seats[f.actor].hand.iter().any(|c| c == card_id) {
                return Err(illegal("手牌中没有该牌"));
            }
            if turn::card_def(st, card_id).is_none_or(|d| d.equip_slot.is_none()) {
                return Err(illegal("必须弃置装备牌"));
            }
            turn::take_card(st, f.actor, card_id)?;
            turn::to_discard_card(st, card_id);
            st.set_counter("shensu_skip_play", 1);
            st.set_counter("skip_play", 1);
            let pid = st.seats[f.actor].player_id.clone();
            st.push_log(
                Some(&pid),
                format!(
                    "神速②：弃置装备【{}】并跳过出牌阶段",
                    turn::card_label(st, card_id)
                ),
            );
            replace_top(
                st,
                Frame::Shensu(ShensuFrame {
                    actor: f.actor,
                    stage: ShensuStage::Ask,
                    used: f.used + 1,
                }),
            );
            shensu_sha(st, f.actor);
            Ok("神速：视为使用【杀】".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// 天香（小乔）：受到伤害时，弃红桃手牌防止伤害并转移
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TianxiangStage {
    Ask,
    Target,
    Mode,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TianxiangFrame {
    pub victim: usize,
    pub amount: i32,
    pub source: Option<usize>,
    pub damage_type: DamageType,
    pub card: Option<String>,
    pub stage: TianxiangStage,
    pub target: Option<usize>,
    pub discarded: Option<String>,
}

pub(crate) fn step_tianxiang(st: &mut SgsState, f: TianxiangFrame) -> Result<bool, EngineError> {
    match f.stage {
        TianxiangStage::Ask => {
            if !skills::tianxiang_available(st, f.victim) {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: st.seats[f.victim].player_id.clone(),
                skill: Some("tianxiang".into()),
                options: vec![
                    PendingOption {
                        id: "choose:tianxiang".into(),
                        label: "天香：弃置一张红桃手牌，防止此伤害并转移".into(),
                    },
                    pass_option("承受伤害"),
                ],
                data: json!({ "skill": "tianxiang" }),
            });
            replace_top(st, Frame::Tianxiang(f));
            Ok(true)
        }
        TianxiangStage::Target => {
            let mut options: Vec<PendingOption> = Vec::new();
            for t in st.alive_indices() {
                if t == f.victim {
                    continue;
                }
                options.push(PendingOption {
                    id: format!("target:{}", st.seats[t].player_id),
                    label: format!("天香：转移给 {}", st.seats[t].general_name),
                });
            }
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.victim].player_id.clone(),
                skill: Some("tianxiang".into()),
                options,
                data: json!({ "stage": "tianxiang" }),
            });
            replace_top(st, Frame::Tianxiang(f));
            Ok(true)
        }
        TianxiangStage::Mode => {
            let target_name = f
                .target
                .map(|t| st.seats[t].general_name.clone())
                .unwrap_or_default();
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: st.seats[f.victim].player_id.clone(),
                skill: Some("tianxiang".into()),
                options: vec![
                    PendingOption {
                        id: "choose:tianxiang:damage".into(),
                        label: format!("令 {target_name} 受到伤害来源的 1 点伤害并摸 X 张"),
                    },
                    PendingOption {
                        id: "choose:tianxiang:lose".into(),
                        label: format!("令 {target_name} 失去 1 点体力并获得弃置的牌"),
                    },
                ],
                data: json!({ "skill": "tianxiang" }),
            });
            replace_top(st, Frame::Tianxiang(f));
            Ok(true)
        }
    }
}

pub(crate) fn act_tianxiang(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Tianxiang(f)) => f,
        _ => return Err(internal("栈顶不是天香帧")),
    };
    match f.stage {
        TianxiangStage::Ask => {
            if action == "pass" {
                pop(st);
                let dtype = f.damage_type;
                crate::resolve::apply_damage_now(
                    st,
                    f.victim,
                    f.amount,
                    f.source,
                    dtype,
                    f.card.as_deref(),
                );
                return Ok("承受伤害".to_string());
            }
            if action != "choose:tianxiang" {
                return Err(illegal("未知天香选项"));
            }
            let mut heart_cards: Vec<String> = st.seats[f.victim]
                .hand
                .iter()
                .filter(|id| skills::effective_suit(st, f.victim, id) == Some(Suit::Heart))
                .cloned()
                .collect();
            // 界天香（界小乔）：装备区的红桃牌也可弃置。
            if skills::is_jiexian(st, f.victim) {
                for id in crate::resolve_jiexian::equip_card_ids(st, f.victim) {
                    if skills::effective_suit(st, f.victim, &id) == Some(Suit::Heart) {
                        heart_cards.push(id);
                    }
                }
            }
            replace_top(
                st,
                Frame::MultiSelect(MultiSelectFrame {
                    actor: f.victim,
                    purpose: "tianxiang".into(),
                    selected: Vec::new(),
                    min: 1,
                    max: 1,
                    data: json!({
                        "allowed": heart_cards,
                        "victim": f.victim,
                        "amount": f.amount,
                        "source": f.source,
                        "damage_type": f.damage_type,
                        "card": f.card,
                    }),
                }),
            );
            Ok("天香：请选择弃置的红桃手牌".to_string())
        }
        TianxiangStage::Target => Err(illegal("天香：请选择转移目标")),
        TianxiangStage::Mode => {
            let target = f.target.ok_or_else(|| internal("天香缺少目标"))?;
            let discarded = f.discarded.ok_or_else(|| internal("天香缺少弃置牌"))?;
            pop(st);
            let pid = st.seats[f.victim].player_id.clone();
            let target_name = st.seats[target].general_name.clone();
            match action {
                "choose:tianxiang:damage" => {
                    crate::resolve::apply_damage_now(
                        st,
                        target,
                        1,
                        f.source,
                        DamageType::Normal,
                        None,
                    );
                    let hp = st.seats[target].hp;
                    let lost = (st.seats[target].max_hp - hp).max(0).min(5) as u32;
                    let drawn = if hp > 0 {
                        draw_capped(st, target, lost)
                    } else {
                        0
                    };
                    st.push_log(
                        Some(&pid),
                        format!("天香：{target_name} 受到 1 点伤害并摸 {drawn} 张"),
                    );
                    Ok(format!("天香：{target_name} 受到 1 点伤害并摸 {drawn} 张"))
                }
                "choose:tianxiang:lose" => {
                    st.seats[target].hp -= 1;
                    st.deck.discard.retain(|c| c != &discarded);
                    st.seats[target].hand.push(discarded.clone());
                    let hp = st.seats[target].hp;
                    let over = st.seats[target]
                        .hand
                        .len()
                        .saturating_sub(st.config.max_hand_cards as usize);
                    if over > 0 {
                        crate::resolve::push_discard(st, target, over, "hand_cap");
                    }
                    st.push_log(
                        Some(&pid),
                        format!(
                            "天香：{target_name} 失去 1 点体力并获得【{}】（体力 {hp}）",
                            turn::card_label(st, &discarded)
                        ),
                    );
                    if hp <= 0 {
                        crate::resolve::push_dying(st, target, None);
                    }
                    Ok(format!("天香：{target_name} 失去 1 点体力"))
                }
                _ => Err(illegal("未知天香选项")),
            }
        }
    }
}

/// 天香目标选择（`pending.kind = target`）。
pub(crate) fn tianxiang_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Tianxiang(f)) => f,
        _ => return Err(internal("栈顶不是天香帧")),
    };
    let pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择天香目标"))?;
    if !pending
        .options
        .iter()
        .any(|o| o.id == format!("target:{pid}"))
    {
        return Err(illegal("天香目标不合法"));
    }
    let target = st.seat_index(pid).ok_or_else(|| illegal("目标不在座"))?;
    let discarded = f.discarded.clone();
    replace_top(
        st,
        Frame::Tianxiang(TianxiangFrame {
            victim: f.victim,
            amount: f.amount,
            source: f.source,
            damage_type: f.damage_type,
            card: f.card,
            stage: TianxiangStage::Mode,
            target: Some(target),
            discarded,
        }),
    );
    Ok(format!("天香：目标 {}", st.seats[target].general_name))
}

// ---------------------------------------------------------------------------
// 蛊惑（于吉）：扣置一张手牌当任意基本牌/普通锦囊使用或打出；其他角色可质疑
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuhuoStage {
    Declare,
    Question,
    Resolve,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GuhuoFrame {
    pub user: usize,
    /// `play` / `respond` / `dying` / `wuxie`
    pub purpose: String,
    pub stage: GuhuoStage,
    /// 响应时需要的牌（`shan` / `sha` / `tao` / `wuxie_keji`）。
    pub needed: String,
    pub card: Option<String>,
    pub declared: Option<String>,
    pub order: Vec<String>,
    pub cursor: usize,
    pub questioner: Option<usize>,
    /// 响应用途保存原响应帧（蛊惑结算后继续）。
    pub respond: Option<crate::resolve::RespondFrame>,
}

fn guhuo_used_key(seat: usize) -> String {
    format!("guhuo_used_{seat}")
}

pub(crate) fn guhuo_available(st: &SgsState, seat: usize) -> bool {
    skills::has(st, seat, "guhuo")
        && !skills::chanyuan_locked(st, seat)
        && st.counter(&guhuo_used_key(seat)) == 0
        && !st.seats[seat].hand.is_empty()
}

fn guhuo_order(st: &SgsState, user: usize) -> Vec<String> {
    st.seats_from_left_of(user)
        .into_iter()
        .filter(|i| {
            st.seats[*i].alive && !st.seats[*i].flags.contains_key("chanyuan") && *i != user
        })
        .map(|i| st.seats[i].player_id.clone())
        .collect()
}

fn guhuo_declare_options(st: &SgsState, user: usize) -> Vec<PendingOption> {
    let mut out = Vec::new();
    let others: Vec<usize> = st
        .alive_indices()
        .into_iter()
        .filter(|i| *i != user)
        .collect();
    let sha_ok = st.counter("sha_used") < turn::sha_limit(st, user)
        && !turn::sha_targets(st, user).is_empty();
    if sha_ok {
        for (code, name) in [("sha", "杀"), ("huo_sha", "火杀"), ("lei_sha", "雷杀")] {
            out.push(PendingOption {
                id: format!("declare:{code}"),
                label: format!("蛊惑为【{name}】"),
            });
        }
    }
    if st.seats[user].hp < st.seats[user].max_hp {
        out.push(PendingOption {
            id: "declare:tao".into(),
            label: "蛊惑为【桃】".into(),
        });
    }
    if st.counter("jiu_used") == 0 {
        out.push(PendingOption {
            id: "declare:jiu".into(),
            label: "蛊惑为【酒】".into(),
        });
    }
    out.push(PendingOption {
        id: "declare:wuzhong_shengyou".into(),
        label: "蛊惑为【无中生有】".into(),
    });
    if !others.is_empty() {
        out.push(PendingOption {
            id: "declare:nanman_ruqin".into(),
            label: "蛊惑为【南蛮入侵】".into(),
        });
        out.push(PendingOption {
            id: "declare:wanjian_qifa".into(),
            label: "蛊惑为【万箭齐发】".into(),
        });
        out.push(PendingOption {
            id: "declare:taoyuan_jieyi".into(),
            label: "蛊惑为【桃园结义】".into(),
        });
        out.push(PendingOption {
            id: "declare:tiesuo_lianhuan".into(),
            label: "蛊惑为【铁索连环】".into(),
        });
    }
    if others.iter().any(|t| !skills::kongcheng_blocks(st, *t)) {
        out.push(PendingOption {
            id: "declare:juedou".into(),
            label: "蛊惑为【决斗】".into(),
        });
    }
    if others
        .iter()
        .any(|t| !skills::qianxun_blocks(st, *t) && turn::has_cards_to_take(st, *t))
    {
        out.push(PendingOption {
            id: "declare:guohe_chaiqiao".into(),
            label: "蛊惑为【过河拆桥】".into(),
        });
    }
    if others.iter().any(|t| {
        !skills::qianxun_blocks(st, *t)
            && turn::trick_distance_ok(st, user, *t)
            && turn::has_cards_to_take(st, *t)
    }) {
        out.push(PendingOption {
            id: "declare:shunshou_qianyang".into(),
            label: "蛊惑为【顺手牵羊】".into(),
        });
    }
    if others.iter().any(|t| !st.seats[*t].hand.is_empty()) {
        out.push(PendingOption {
            id: "declare:huo_gong".into(),
            label: "蛊惑为【火攻】".into(),
        });
    }
    let jiedao_ok = others.iter().any(|a| {
        st.equip_def(*a, EquipSlot::Weapon).is_some()
            && others
                .iter()
                .any(|b| b != a && st.in_attack_range(*a, *b) && !skills::kongcheng_blocks(st, *b))
    });
    if jiedao_ok {
        out.push(PendingOption {
            id: "declare:jiedao_sharen".into(),
            label: "蛊惑为【借刀杀人】".into(),
        });
    }
    out
}

/// 出牌阶段发动蛊惑：先选一张手牌（扣置），再声明牌名。
pub(crate) fn begin_guhuo_play(st: &mut SgsState, user: usize) -> Result<(), EngineError> {
    if !guhuo_available(st, user) {
        return Err(illegal("蛊惑本回合已使用过"));
    }
    push(
        st,
        Frame::MultiSelect(MultiSelectFrame {
            actor: user,
            purpose: "guhuo".into(),
            selected: Vec::new(),
            min: 1,
            max: 1,
            data: json!({}),
        }),
    );
    Ok(())
}

/// 响应/濒死/无懈时发动蛊惑（`needed` 为需要的牌；声明牌名固定为该牌）。
pub(crate) fn start_guhuo_response(
    st: &mut SgsState,
    user: usize,
    purpose: &str,
    needed: &str,
    card: &str,
    respond: Option<crate::resolve::RespondFrame>,
) -> Result<(), EngineError> {
    if !guhuo_available(st, user) {
        return Err(illegal("蛊惑本回合已使用过"));
    }
    let order = guhuo_order(st, user);
    st.set_counter(&guhuo_used_key(user), 1);
    push(
        st,
        Frame::Guhuo(GuhuoFrame {
            user,
            purpose: purpose.into(),
            stage: GuhuoStage::Question,
            needed: needed.into(),
            card: Some(card.to_string()),
            declared: Some(needed.to_string()),
            order,
            cursor: 0,
            questioner: None,
            respond,
        }),
    );
    Ok(())
}

pub(crate) fn step_guhuo(st: &mut SgsState, f: GuhuoFrame) -> Result<bool, EngineError> {
    match f.stage {
        GuhuoStage::Declare => {
            let mut options = guhuo_declare_options(st, f.user);
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            options.push(pass_option("取消蛊惑"));
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: st.seats[f.user].player_id.clone(),
                skill: Some("guhuo".into()),
                options,
                data: json!({ "skill": "guhuo", "stage": "declare" }),
            });
            replace_top(st, Frame::Guhuo(f));
            Ok(true)
        }
        GuhuoStage::Question => {
            let Some(pid) = f.order.get(f.cursor).cloned() else {
                let mut nf = f;
                nf.stage = GuhuoStage::Resolve;
                replace_top(st, Frame::Guhuo(nf));
                return Ok(false);
            };
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: pid,
                skill: Some("guhuo".into()),
                options: vec![
                    PendingOption {
                        id: "choose:question".into(),
                        label: "质疑：展示此牌".into(),
                    },
                    pass_option("不质疑"),
                ],
                data: json!({ "skill": "guhuo", "stage": "question", "user": st.seats[f.user].player_id }),
            });
            replace_top(st, Frame::Guhuo(f));
            Ok(true)
        }
        GuhuoStage::Resolve => {
            let card = f.card.clone().ok_or_else(|| internal("蛊惑缺少牌"))?;
            let declared = f.declared.clone().ok_or_else(|| internal("蛊惑缺少声明"))?;
            let actual = st
                .cards
                .get(&card)
                .map(|c| c.def.as_str())
                .unwrap_or_default();
            let real = actual == declared;
            let pid = st.seats[f.user].player_id.clone();
            if let Some(q) = f.questioner {
                let jie = crate::skills::is_jiexian(st, f.user);
                if real {
                    st.seats[q].flags.insert("chanyuan".into(), true);
                    if jie {
                        st.seats[q].flags.insert("chanyuan_jie".into(), true);
                        // 界蛊惑：质疑者弃置一张牌或失去 1 点体力（无牌时失体力）。
                        if !st.seats[q].hand.is_empty() {
                            let draw_card = st.seats[q].hand[0].clone();
                            turn::take_card(st, q, &draw_card)?;
                            turn::to_discard_card(st, &draw_card);
                            st.push_log(
                                Some(&pid),
                                format!(
                                    "蛊惑：{} 质疑，展示【{}】为真，其获得「缠怨」并弃置【{}】",
                                    st.seats[q].general_name,
                                    turn::card_label(st, &card),
                                    turn::card_label(st, &draw_card)
                                ),
                            );
                        } else {
                            st.seats[q].hp -= 1;
                            let hp = st.seats[q].hp;
                            st.push_log(
                                Some(&pid),
                                format!(
                                    "蛊惑：{} 质疑，展示【{}】为真，其获得「缠怨」并失去 1 点体力（{hp}）",
                                    st.seats[q].general_name,
                                    turn::card_label(st, &card)
                                ),
                            );
                            if hp <= 0 {
                                crate::resolve::push_dying(st, q, Some(f.user));
                            }
                        }
                    } else {
                        st.push_log(
                            Some(&pid),
                            format!(
                                "蛊惑：{} 质疑，展示【{}】为真，质疑者获得「缠怨」",
                                st.seats[q].general_name,
                                turn::card_label(st, &card)
                            ),
                        );
                    }
                } else {
                    // 假：此牌作废（弃置），不产生效果；响应帧保留以便继续响应。
                    turn::take_card(st, f.user, &card)?;
                    turn::to_discard_card(st, &card);
                    pop(st);
                    // 界蛊惑：牌为假时，质疑者摸一张牌。
                    if jie {
                        let drawn = crate::turn::draw_cards(st, q, 1, &mut Vec::new());
                        if drawn > 0 {
                            st.push_log(Some(&pid), "蛊惑：牌为假，质疑者摸 1 张");
                        }
                    }
                    st.push_log(
                        Some(&pid),
                        format!(
                            "蛊惑：{} 质疑，展示【{}】为假，此牌作废",
                            st.seats[q].general_name,
                            turn::card_label(st, &card)
                        ),
                    );
                    return Ok(false);
                }
            }
            pop(st);
            match f.purpose.as_str() {
                "play" => {
                    resolve_guhuo_play(st, f.user, &card, &declared)?;
                    Ok(false)
                }
                "respond" => {
                    let respond = f.respond.ok_or_else(|| internal("蛊惑缺少响应帧"))?;
                    crate::resolve::finish_respond_play(
                        st,
                        &respond,
                        &card,
                        f.user,
                        Some(("guhuo", "蛊惑")),
                    )?;
                    Ok(false)
                }
                "dying" => {
                    turn::take_card(st, f.user, &card)?;
                    turn::to_discard_card(st, &card);
                    st.seats[f.user].hp += 1;
                    let hp = st.seats[f.user].hp;
                    st.push_log(
                        Some(&pid),
                        format!(
                            "蛊惑：将【{}】当【桃】自救（体力 {hp}）",
                            turn::card_label(st, &card)
                        ),
                    );
                    if hp > 0 {
                        st.phase = Phase::Turn;
                        pop(st);
                    } else if let Some(Frame::Dying(mut df)) = top(st).cloned() {
                        df.passed.clear();
                        replace_top(st, Frame::Dying(df));
                    }
                    Ok(false)
                }
                "wuxie" => {
                    turn::take_card(st, f.user, &card)?;
                    turn::to_discard_card(st, &card);
                    if let Some(Frame::Wuxie(mut wf)) = top(st).cloned() {
                        wf.count += 1;
                        wf.cursor = 0;
                        wf.passed = 0;
                        let count = wf.count;
                        replace_top(st, Frame::Wuxie(wf));
                        st.push_log(
                            Some(&pid),
                            format!(
                                "蛊惑：将【{}】当【无懈可击】（层数 {count}）",
                                turn::card_label(st, &card)
                            ),
                        );
                    }
                    Ok(false)
                }
                _ => Err(internal("未知蛊惑用途")),
            }
        }
    }
}

fn resolve_guhuo_play(
    st: &mut SgsState,
    user: usize,
    card: &str,
    declared: &str,
) -> Result<(), EngineError> {
    let pid = st.seats[user].player_id.clone();
    match declared {
        "sha" | "huo_sha" | "lei_sha" => {
            let dtype = match declared {
                "huo_sha" => DamageType::Fire,
                "lei_sha" => DamageType::Thunder,
                _ => DamageType::Normal,
            };
            let fangtian = skills::has_equipment_skill(st, user, "fangtian_huaji")
                && st.seats[user].hand.len() == 1;
            turn::take_card(st, user, card)?;
            crate::resolve::start_sha_with(st, user, card, fangtian, dtype)?;
        }
        "tao" => {
            turn::take_card(st, user, card)?;
            turn::to_discard_card(st, card);
            st.seats[user].hp += 1;
            let hp = st.seats[user].hp;
            st.push_log(
                Some(&pid),
                format!(
                    "蛊惑：将【{}】当【桃】使用（体力 {hp}）",
                    turn::card_label(st, card)
                ),
            );
        }
        "jiu" => {
            st.set_counter("jiu_used", 1);
            crate::skills::mark_jiu(st, user);
            turn::take_card(st, user, card)?;
            turn::to_discard_card(st, card);
            st.push_log(
                Some(&pid),
                format!("蛊惑：将【{}】当【酒】使用", turn::card_label(st, card)),
            );
        }
        "wugu_fengdeng" => {
            turn::take_card(st, user, card)?;
            st.deck.processing.push(card.to_string());
            crate::resolve_zone::start_wugu(st, user, card);
            st.push_log(
                Some(&pid),
                format!(
                    "蛊惑：将【{}】当【五谷丰登】使用",
                    turn::card_label(st, card)
                ),
            );
        }
        code => {
            turn::take_card(st, user, card)?;
            st.deck.processing.push(card.to_string());
            crate::resolve::start_virtual_trick(st, user, card, code)?;
            let mut ev = Vec::new();
            skills::on_non_delayed_trick_used(st, user, &mut ev);
            st.push_log(
                Some(&pid),
                format!(
                    "蛊惑：将【{}】当【{}】使用",
                    turn::card_label(st, card),
                    crate::data::card_def(code).map(|d| d.name).unwrap_or(code)
                ),
            );
        }
    }
    Ok(())
}

pub(crate) fn act_guhuo(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Guhuo(f)) => f,
        _ => return Err(internal("栈顶不是蛊惑帧")),
    };
    match f.stage {
        GuhuoStage::Declare => {
            if action == "pass" {
                pop(st);
                return Ok("放弃蛊惑".to_string());
            }
            let code = action
                .strip_prefix("declare:")
                .ok_or_else(|| illegal("需要声明牌名或放弃"))?;
            if !st
                .pending
                .as_ref()
                .is_some_and(|p| p.options.iter().any(|o| o.id == action))
            {
                return Err(illegal("该声明不可用"));
            }
            let order = guhuo_order(st, f.user);
            st.set_counter(&guhuo_used_key(f.user), 1);
            replace_top(
                st,
                Frame::Guhuo(GuhuoFrame {
                    user: f.user,
                    purpose: f.purpose,
                    stage: GuhuoStage::Question,
                    needed: f.needed,
                    card: f.card,
                    declared: Some(code.to_string()),
                    order,
                    cursor: 0,
                    questioner: None,
                    respond: f.respond,
                }),
            );
            Ok(format!("蛊惑：声明为【{code}】，等待质疑"))
        }
        GuhuoStage::Question => {
            let actor = st
                .pending
                .as_ref()
                .map(|p| p.actor.clone())
                .unwrap_or_default();
            let mut nf = f;
            if action == "choose:question" {
                let seat = st
                    .seat_index(&actor)
                    .ok_or_else(|| internal("质疑者不在座"))?;
                nf.questioner = Some(seat);
                nf.stage = GuhuoStage::Resolve;
            } else {
                nf.cursor += 1;
            }
            replace_top(st, Frame::Guhuo(nf));
            Ok("继续蛊惑结算".to_string())
        }
        _ => Err(illegal("蛊惑当前阶段不接受该动作")),
    }
}
