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
