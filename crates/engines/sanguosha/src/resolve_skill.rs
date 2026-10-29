//! 技能帧的步进与输入处理（奸雄 / 制衡）。
//!
//! 与 `resolve.rs` 的结算栈共用 `Frame` / `pending`；这里只放技能专属逻辑，
//! 避免主结算文件继续膨胀。

use crate::resolve::{
    pop, push_discard, replace_top, top, Frame, JianxiongFrame, MultiSelectFrame,
};
use crate::state::*;
use crate::turn;
use crate::{illegal, internal};
use game_engine_core::EngineError;
use serde_json::json;

// ---------------------------------------------------------------------------
// 麒麟弓（P3 追加）：杀造成伤害后，攻击方可弃置目标一张坐骑（进攻/防御马）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QilinFrame {
    pub attacker: usize,
    pub target: usize,
}

/// 目标当前可被弃置的坐骑槽（进攻马在前、防御马在后）。
pub(crate) fn qilin_horses(st: &SgsState, target: usize) -> Vec<EquipSlot> {
    [EquipSlot::HorseOffense, EquipSlot::HorseDefense]
        .into_iter()
        .filter(|slot| {
            st.seats[target]
                .equips
                .get(slot)
                .is_some_and(|v| v.is_some())
        })
        .collect()
}

fn horse_slot_code(slot: EquipSlot) -> &'static str {
    match slot {
        EquipSlot::HorseOffense => "horse_offense",
        EquipSlot::HorseDefense => "horse_defense",
        _ => "horse_offense",
    }
}

fn horse_slot_from_code(code: &str) -> Option<EquipSlot> {
    match code {
        "horse_offense" => Some(EquipSlot::HorseOffense),
        "horse_defense" => Some(EquipSlot::HorseDefense),
        _ => None,
    }
}

pub(crate) fn step_qilin(st: &mut SgsState, f: QilinFrame) -> Result<bool, EngineError> {
    let horses = qilin_horses(st, f.target);
    if horses.is_empty() || !st.seats[f.target].alive || !st.seats[f.attacker].alive {
        pop(st);
        return Ok(false);
    }
    let target_name = st.seats[f.target].general_name.clone();
    let mut options: Vec<PendingOption> = horses
        .iter()
        .map(|slot| {
            let id = st.seats[f.target].equips.get(slot).and_then(|v| v.clone());
            let label = match (&id, slot) {
                (Some(card), EquipSlot::HorseOffense) => {
                    format!(
                        "弃置{}的【{}】（进攻马）",
                        target_name,
                        turn::card_label(st, card)
                    )
                }
                (Some(card), _) => {
                    format!(
                        "弃置{}的【{}】（防御马）",
                        target_name,
                        turn::card_label(st, card)
                    )
                }
                (None, _) => String::new(),
            };
            PendingOption {
                id: format!("skill:qilin:{}", horse_slot_code(*slot)),
                label,
            }
        })
        .collect();
    options.push(PendingOption {
        id: "pass".into(),
        label: "不弃置".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.attacker].player_id.clone(),
        skill: Some("qilin".into()),
        options,
        data: json!({ "skill": "qilin", "target": st.seats[f.target].player_id }),
    });
    replace_top(st, Frame::Qilin(f));
    Ok(true)
}

pub(crate) fn act_qilin(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Qilin(f)) => f,
        _ => return Err(internal("栈顶不是麒麟弓帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不弃置坐骑".to_string());
    }
    let code = action
        .strip_prefix("skill:qilin:")
        .ok_or_else(|| illegal("需要选择要弃置的坐骑或选择不弃置"))?;
    let slot = horse_slot_from_code(code).ok_or_else(|| illegal("未知的坐骑槽"))?;
    let card = st.seats[f.target]
        .equips
        .get(&slot)
        .cloned()
        .flatten()
        .ok_or_else(|| illegal("该槽位没有坐骑"))?;
    st.seats[f.target].equips.insert(slot, None);
    turn::to_discard_card(st, &card);
    let label = turn::card_label(st, &card);
    st.push_log(
        Some(&pending.actor),
        format!(
            "麒麟弓：弃置{}的【{}】",
            st.seats[f.target].general_name, label
        ),
    );
    pop(st);
    Ok(format!("麒麟弓：已弃置【{label}】"))
}

// ---------------------------------------------------------------------------
// 奸雄
// ---------------------------------------------------------------------------

pub(crate) fn step_jianxiong(st: &mut SgsState, f: JianxiongFrame) -> Result<bool, EngineError> {
    let name = turn::card_label(st, &f.card);
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("jianxiong".into()),
        options: vec![
            PendingOption {
                id: "choose:gain".into(),
                label: format!("获得【{name}】"),
            },
            PendingOption {
                id: "pass".into(),
                label: "不获得".into(),
            },
        ],
        data: json!({ "skill": "jianxiong", "card": f.card }),
    });
    replace_top(st, Frame::Jianxiong(f));
    Ok(true)
}

pub(crate) fn act_jianxiong(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Jianxiong(f)) => f,
        _ => return Err(internal("栈顶不是奸雄帧")),
    };
    let name = turn::card_label(st, &f.card);
    if action == "choose:gain" {
        if st.deck.processing.iter().any(|c| c == &f.card) {
            st.deck.processing.retain(|c| c != &f.card);
            st.seats[f.actor].hand.push(f.card.clone());
            st.push_log(
                Some(&st.seats[f.actor].player_id.clone()),
                format!("奸雄：获得【{name}】"),
            );
            let over = st.seats[f.actor]
                .hand
                .len()
                .saturating_sub(st.config.max_hand_cards as usize);
            push_discard(st, f.actor, over, "hand_cap");
        }
    }
    pop(st);
    Ok(format!("奸雄：{name}"))
}

// ---------------------------------------------------------------------------
// 多选（制衡等）
// ---------------------------------------------------------------------------

pub(crate) fn step_multiselect(
    st: &mut SgsState,
    f: MultiSelectFrame,
) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = Vec::new();
    for id in turn::hand_ids(st, f.actor) {
        let mark = if f.selected.contains(&id) { "✔ " } else { "" };
        options.push(PendingOption {
            id: format!("choose:{id}"),
            label: format!("{mark}{}", turn::card_label(st, &id)),
        });
    }
    if f.selected.len() >= f.min && !f.selected.is_empty() {
        options.push(PendingOption {
            id: "confirm".into(),
            label: format!("确认（已选 {} 张）", f.selected.len()),
        });
    }
    if f.min == 0 {
        options.push(PendingOption {
            id: "cancel".into(),
            label: "取消".into(),
        });
    }
    st.pending = Some(Pending {
        kind: "choose_many".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some(f.purpose.clone()),
        options,
        data: json!({
            "purpose": f.purpose,
            "selected": f.selected,
            "min": f.min,
            "max": f.max,
        }),
    });
    replace_top(st, Frame::MultiSelect(f));
    Ok(true)
}

pub(crate) fn act_multiselect(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let mut f = match top(st).cloned() {
        Some(Frame::MultiSelect(f)) => f,
        _ => return Err(internal("栈顶不是多选帧")),
    };
    if action == "cancel" {
        if f.min > 0 {
            return Err(illegal("至少需要选择 1 张"));
        }
        pop(st);
        return Ok("已取消".to_string());
    }
    if action == "confirm" {
        if f.selected.len() < f.min {
            return Err(illegal("选择数量不足"));
        }
        if f.selected.len() > f.max {
            return Err(illegal("选择数量过多"));
        }
        if let Some(result) = crate::resolve_more::multiselect_confirm(st, &f) {
            return result;
        }
        match f.purpose.as_str() {
            "zhiheng" => {
                let n = f.selected.len();
                for id in &f.selected {
                    turn::take_card(st, f.actor, id)?;
                    turn::to_discard_card(st, id);
                }
                st.set_counter("zhiheng_used", 1);
                let mut events = Vec::new();
                let drawn = turn::draw_cards(st, f.actor, n as u32, &mut events);
                st.push_log(
                    Some(&st.seats[f.actor].player_id.clone()),
                    format!("制衡：弃置 {n} 张，摸 {drawn} 张"),
                );
                pop(st);
                Ok(format!("制衡：弃 {n} 摸 {drawn}"))
            }
            other => Err(internal(format!("未实现的多选用途：{other}"))),
        }
    } else {
        let card_id = action
            .strip_prefix("choose:")
            .ok_or_else(|| illegal("未知选项"))?;
        if !st.seats[f.actor].hand.iter().any(|c| c == card_id) {
            return Err(illegal("手牌中没有该牌"));
        }
        if let Some(pos) = f.selected.iter().position(|c| c == card_id) {
            f.selected.remove(pos);
        } else {
            if f.selected.len() >= f.max {
                return Err(illegal("选择数量已达上限"));
            }
            f.selected.push(card_id.to_string());
        }
        replace_top(st, Frame::MultiSelect(f));
        Ok("已选择".to_string())
    }
}
