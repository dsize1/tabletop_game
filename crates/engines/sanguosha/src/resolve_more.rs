//! P2.4 第二批技能帧：仁德 / 离间 / 突袭 / 奇袭（含激将/护驾的接力帧）。
//!
//! 与 `resolve.rs` 共用结算栈与 pending；本文件只放技能专属状态机。

use crate::resolve::{pop, push, replace_top, top, Frame, Outcome, RespondFrame, RespondKind};
use crate::skills;
use crate::state::*;
use crate::turn;
use crate::{illegal, internal};
use game_engine_core::EngineError;
use serde_json::json;

// ---------------------------------------------------------------------------
// 激将 / 护驾（同势力接力）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RequisitionFrame {
    pub lord: usize,
    /// 需要的牌：`sha`（激将）或 `shan`（护驾）。
    pub needed: String,
    pub order: Vec<String>,
    pub cursor: usize,
}

pub(crate) fn step_requisition(
    st: &mut SgsState,
    mut f: RequisitionFrame,
) -> Result<bool, EngineError> {
    while f.cursor < f.order.len() {
        let pid = f.order[f.cursor].clone();
        // 无手牌的同势力角色不可能代出【杀】/【闪】，自动跳过（公共信息）。
        let usable = st.seat(&pid).is_some_and(|s| s.alive && !s.hand.is_empty());
        if usable {
            break;
        }
        f.cursor += 1;
    }
    if f.cursor >= f.order.len() {
        // 无人代替响应：回到主公本人的响应
        pop(st);
        return Ok(false);
    }
    let pid = f.order[f.cursor].clone();
    let seat = st
        .seat_index(&pid)
        .ok_or_else(|| internal("接力对象不在座"))?;
    let code = if f.needed == "sha" { "sha" } else { "shan" };
    let mut options: Vec<PendingOption> = turn::hand_cards_with_def(st, seat, code)
        .iter()
        .map(|id| PendingOption {
            id: format!("respond:{id}"),
            label: format!("代替打出【{}】", turn::card_label(st, id)),
        })
        .collect();
    options.push(PendingOption {
        id: "pass".into(),
        label: "不出".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid,
        skill: Some("requisition".into()),
        options,
        data: json!({ "needed": f.needed }),
    });
    replace_top(st, Frame::Requisition(f));
    Ok(true)
}

pub(crate) fn act_requisition(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let mut f = match top(st).cloned() {
        Some(Frame::Requisition(f)) => f,
        _ => return Err(internal("栈顶不是接力帧")),
    };
    if action == "pass" {
        f.cursor += 1;
        replace_top(st, Frame::Requisition(f));
        return Ok("放弃代替响应".to_string());
    }
    let card_id = action
        .strip_prefix("respond:")
        .ok_or_else(|| illegal("需要打出牌或 pass"))?;
    let expected = if f.needed == "sha" { "sha" } else { "shan" };
    if turn::card_def(st, card_id).map(|d| d.code) != Some(expected) {
        return Err(illegal("打出的牌不符合要求"));
    }
    let by = st
        .seat_index(&pending.actor)
        .ok_or_else(|| internal("玩家不在座"))?;
    st.outcome = Some(Outcome::Respond {
        played: Some(card_id.to_string()),
        played_by: Some(by),
    });
    pop(st);
    Ok("已代替打出".to_string())
}

// ---------------------------------------------------------------------------
// 仁德
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RendeStage {
    Select,
    Target,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RendeFrame {
    pub actor: usize,
    pub stage: RendeStage,
    pub selected: Vec<String>,
}

pub(crate) fn step_rende(st: &mut SgsState, f: RendeFrame) -> Result<bool, EngineError> {
    match f.stage {
        RendeStage::Select => {
            let mut options: Vec<PendingOption> = Vec::new();
            for id in turn::hand_ids(st, f.actor) {
                let mark = if f.selected.contains(&id) { "✔ " } else { "" };
                options.push(PendingOption {
                    id: format!("choose:{id}"),
                    label: format!("{mark}{}", turn::card_label(st, &id)),
                });
            }
            if !f.selected.is_empty() {
                options.push(PendingOption {
                    id: "confirm".into(),
                    label: format!("确认（已选 {} 张）", f.selected.len()),
                });
            }
            options.push(PendingOption {
                id: "cancel".into(),
                label: "取消".into(),
            });
            st.pending = Some(Pending {
                kind: "choose_many".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("rende".into()),
                options,
                data: json!({ "purpose": "rende", "selected": f.selected, "min": 1 }),
            });
            replace_top(st, Frame::Rende(f));
            Ok(true)
        }
        RendeStage::Target => {
            let options: Vec<PendingOption> = st
                .alive_indices()
                .into_iter()
                .filter(|i| *i != f.actor)
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("交给 {}", st.seats[i].general_name),
                })
                .collect();
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("rende".into()),
                options,
                data: json!({ "purpose": "rende", "selected": f.selected }),
            });
            replace_top(st, Frame::Rende(f));
            Ok(true)
        }
    }
}

pub(crate) fn rende_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Rende(f)) => f,
        _ => return Err(internal("栈顶不是仁德帧")),
    };
    let pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择目标"))?;
    let target = st.seat_index(pid).ok_or_else(|| illegal("目标不在座"))?;
    if target == f.actor {
        return Err(illegal("不能交给自己"));
    }
    let cards = f.selected.clone();
    let n = cards.len();
    for id in &cards {
        turn::take_card(st, f.actor, id)?;
        st.seats[target].hand.push(id.clone());
    }
    let given = st.counter("rende_given") + n as u32;
    st.set_counter("rende_given", given);
    let mut healed = false;
    if given >= 2
        && st.counter("rende_healed") == 0
        && st.seats[f.actor].hp < st.seats[f.actor].max_hp
    {
        st.seats[f.actor].hp += 1;
        st.set_counter("rende_healed", 1);
        healed = true;
    }
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        format!(
            "仁德：交给 {} {n} 张{}",
            st.seats[target].general_name,
            if healed { "，回复 1 点体力" } else { "" }
        ),
    );
    pop(st);
    let _ = pending;
    Ok(format!("已交给 {} {n} 张", st.seats[target].general_name))
}

// ---------------------------------------------------------------------------
// 离间
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LijianStage {
    Discard,
    Attacker,
    Defender,
    Wait,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LijianFrame {
    pub actor: usize,
    pub stage: LijianStage,
    pub selected: Vec<String>,
    pub attacker: Option<usize>,
}

pub(crate) fn step_lijian(st: &mut SgsState, f: LijianFrame) -> Result<bool, EngineError> {
    match f.stage {
        LijianStage::Discard => {
            let mut options: Vec<PendingOption> = Vec::new();
            for id in turn::hand_ids(st, f.actor) {
                let mark = if f.selected.contains(&id) { "✔ " } else { "" };
                options.push(PendingOption {
                    id: format!("choose:{id}"),
                    label: format!("{mark}{}", turn::card_label(st, &id)),
                });
            }
            if f.selected.len() == 1 {
                options.push(PendingOption {
                    id: "confirm".into(),
                    label: "确认弃置".into(),
                });
            }
            options.push(PendingOption {
                id: "cancel".into(),
                label: "取消".into(),
            });
            st.pending = Some(Pending {
                kind: "choose_many".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("lijian".into()),
                options,
                data: json!({ "purpose": "lijian", "min": 1, "max": 1 }),
            });
            replace_top(st, Frame::Lijian(f));
            Ok(true)
        }
        LijianStage::Attacker => {
            let options = male_targets(st, f.actor, None);
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("lijian".into()),
                options,
                data: json!({ "purpose": "lijian", "stage": "attacker" }),
            });
            replace_top(st, Frame::Lijian(f));
            Ok(true)
        }
        LijianStage::Defender => {
            let options = male_targets(st, f.actor, f.attacker);
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("lijian".into()),
                options,
                data: json!({ "purpose": "lijian", "stage": "defender" }),
            });
            replace_top(st, Frame::Lijian(f));
            Ok(true)
        }
        LijianStage::Wait => {
            // 决斗结束后出栈
            st.outcome = None;
            pop(st);
            Ok(false)
        }
    }
}

fn male_targets(st: &SgsState, actor: usize, exclude: Option<usize>) -> Vec<PendingOption> {
    st.alive_indices()
        .into_iter()
        .filter(|i| *i != actor && Some(*i) != exclude && skills::is_male(st, *i))
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: st.seats[i].general_name.clone(),
        })
        .collect()
}

pub(crate) fn lijian_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let mut f = match top(st).cloned() {
        Some(Frame::Lijian(f)) => f,
        _ => return Err(internal("栈顶不是离间帧")),
    };
    let pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择目标"))?;
    let target = st.seat_index(pid).ok_or_else(|| illegal("目标不在座"))?;
    let stage = pending.data["stage"].as_str().unwrap_or("");
    match stage {
        "attacker" => {
            f.attacker = Some(target);
            f.stage = LijianStage::Defender;
            replace_top(st, Frame::Lijian(f));
            Ok(format!(
                "离间：{} 视为使用决斗",
                st.seats[target].general_name
            ))
        }
        "defender" => {
            let attacker = f.attacker.ok_or_else(|| internal("缺少离间攻击者"))?;
            for id in &f.selected {
                turn::take_card(st, f.actor, id)?;
                turn::to_discard_card(st, id);
            }
            st.set_counter("lijian_used", 1);
            st.push_log(
                Some(&st.seats[f.actor].player_id.clone()),
                format!(
                    "离间：{} → {}（视为决斗）",
                    st.seats[attacker].general_name, st.seats[target].general_name
                ),
            );
            f.stage = LijianStage::Wait;
            replace_top(st, Frame::Lijian(f));
            push(
                st,
                Frame::Respond(RespondFrame {
                    kind: RespondKind::Duel,
                    actor: target,
                    card: "lijian".into(),
                    source: attacker,
                    target: Some(attacker),
                    damage: 1,
                    damage_type: crate::resolve::DamageType::Normal,
                    consume_card: false,
                    ignore_armor: false,
                    bagua_used: false,
                }),
            );
            Ok("离间已生效：目标需出【杀】".to_string())
        }
        _ => Err(illegal("未知离间阶段")),
    }
}

// ---------------------------------------------------------------------------
// 突袭
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TuxiOfferFrame {
    pub actor: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TuxiFrame {
    pub actor: usize,
    pub targets: Vec<usize>,
}

pub(crate) fn step_tuxi_offer(st: &mut SgsState, f: TuxiOfferFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("tuxi".into()),
        options: vec![
            PendingOption {
                id: "choose:tuxi".into(),
                label: "发动突袭：放弃摸牌，获得至多两名角色各一张手牌".into(),
            },
            PendingOption {
                id: "pass".into(),
                label: "正常摸牌".into(),
            },
        ],
        data: json!({ "skill": "tuxi" }),
    });
    replace_top(st, Frame::TuxiOffer(f));
    Ok(true)
}

pub(crate) fn act_tuxi_offer(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::TuxiOffer(f)) => f,
        _ => return Err(internal("栈顶不是突袭帧")),
    };
    if action == "choose:tuxi" {
        replace_top(
            st,
            Frame::Tuxi(TuxiFrame {
                actor: f.actor,
                targets: Vec::new(),
            }),
        );
        return Ok("请选择突袭目标".to_string());
    }
    pop(st);
    Ok("放弃突袭，正常摸牌".to_string())
}

pub(crate) fn step_tuxi(st: &mut SgsState, f: TuxiFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| {
            *i != f.actor
                && !f.targets.contains(i)
                && st.seats[*i].hand.iter().any(|c| st.cards.get(c).is_some())
        })
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("获得 {} 一张手牌", st.seats[i].general_name),
        })
        .collect();
    if !f.targets.is_empty() {
        options.push(PendingOption {
            id: "confirm".into(),
            label: format!("确认（已选 {} 人）", f.targets.len()),
        });
    }
    if options.is_empty() {
        finish_tuxi(st, f);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "target".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("tuxi".into()),
        options,
        data: json!({ "purpose": "tuxi", "targets": f.targets }),
    });
    replace_top(st, Frame::Tuxi(f));
    Ok(true)
}

pub(crate) fn tuxi_target(
    st: &mut SgsState,
    _pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let mut f = match top(st).cloned() {
        Some(Frame::Tuxi(f)) => f,
        _ => return Err(internal("栈顶不是突袭帧")),
    };
    if action == "confirm" {
        finish_tuxi(st, f);
        return Ok("突袭结束".to_string());
    }
    let pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择目标"))?;
    let target = st.seat_index(pid).ok_or_else(|| illegal("目标不在座"))?;
    if st.seats[target].hand.is_empty() {
        return Err(illegal("目标没有手牌"));
    }
    let n = st.seats[target].hand.len();
    let idx = st.next_index(n);
    let card = st.seats[target].hand.remove(idx);
    st.seats[f.actor].hand.push(card.clone());
    f.targets.push(target);
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        format!("突袭：获得 {} 一张手牌", st.seats[target].general_name),
    );
    if f.targets.len() >= 2 {
        finish_tuxi(st, f);
        Ok("突袭结束".to_string())
    } else {
        replace_top(st, Frame::Tuxi(f));
        Ok("已获得一张手牌".to_string())
    }
}

fn finish_tuxi(st: &mut SgsState, f: TuxiFrame) {
    st.set_counter("skip_draw", 1);
    let over = st.seats[f.actor]
        .hand
        .len()
        .saturating_sub(st.config.max_hand_cards as usize);
    if over > 0 {
        crate::resolve::push_discard(st, f.actor, over, "hand_cap");
    }
    pop(st);
}

// ---------------------------------------------------------------------------
// 多选/目标的分发（由 resolve 的 act_multiselect / act_target 调用）
// ---------------------------------------------------------------------------

/// 仁德/离间的手牌多选（帧类型不同，但 UI 语义与制衡一致）。
pub(crate) fn act_select_skill(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    enum Which {
        Rende,
        Lijian,
    }
    let (which, mut selected, actor) = match top(st).cloned() {
        Some(Frame::Rende(f)) => (Which::Rende, f.selected, f.actor),
        Some(Frame::Lijian(f)) => (Which::Lijian, f.selected, f.actor),
        _ => return Err(internal("栈顶不是技能多选帧")),
    };
    if action == "cancel" {
        let name = match which {
            Which::Rende => "仁德",
            Which::Lijian => "离间",
        };
        st.push_log(
            Some(&st.seats[actor].player_id.clone()),
            format!("取消{name}"),
        );
        pop(st);
        return Ok("已取消".to_string());
    }
    if action == "confirm" {
        match which {
            Which::Rende => {
                if selected.is_empty() {
                    return Err(illegal("至少选择 1 张"));
                }
                if let Some(Frame::Rende(f)) = top(st).cloned() {
                    replace_top(
                        st,
                        Frame::Rende(RendeFrame {
                            stage: RendeStage::Target,
                            selected,
                            ..f
                        }),
                    );
                }
                return Ok("请选择交给谁".to_string());
            }
            Which::Lijian => {
                if selected.len() != 1 {
                    return Err(illegal("离间需弃置 1 张牌"));
                }
                if let Some(Frame::Lijian(f)) = top(st).cloned() {
                    replace_top(
                        st,
                        Frame::Lijian(LijianFrame {
                            stage: LijianStage::Attacker,
                            selected,
                            ..f
                        }),
                    );
                }
                return Ok("请选择第一名男性角色".to_string());
            }
        }
    }
    let card_id = action
        .strip_prefix("choose:")
        .ok_or_else(|| illegal("未知选项"))?;
    match which {
        Which::Rende => {
            if let Some(Frame::Rende(f)) = top(st).cloned() {
                if !st.seats[f.actor].hand.iter().any(|c| c == card_id) {
                    return Err(illegal("手牌中没有该牌"));
                }
                if let Some(pos) = selected.iter().position(|c| c == card_id) {
                    selected.remove(pos);
                } else {
                    selected.push(card_id.to_string());
                }
                replace_top(
                    st,
                    Frame::Rende(RendeFrame {
                        stage: RendeStage::Select,
                        selected,
                        ..f
                    }),
                );
            }
        }
        Which::Lijian => {
            if let Some(Frame::Lijian(f)) = top(st).cloned() {
                if !st.seats[f.actor].hand.iter().any(|c| c == card_id) {
                    return Err(illegal("手牌中没有该牌"));
                }
                if selected.first().map(String::as_str) == Some(card_id) {
                    selected.clear();
                } else {
                    selected = vec![card_id.to_string()];
                }
                replace_top(
                    st,
                    Frame::Lijian(LijianFrame {
                        stage: LijianStage::Discard,
                        selected,
                        ..f
                    }),
                );
            }
        }
    }
    Ok("已选择".to_string())
}

/// 返回 `Some` 表示该 purpose 已由技能帧处理。
pub(crate) fn multiselect_confirm(
    st: &mut SgsState,
    f: &crate::resolve::MultiSelectFrame,
) -> Option<Result<String, EngineError>> {
    match f.purpose.as_str() {
        "rende" => {
            if f.selected.is_empty() {
                return Some(Err(illegal("至少选择 1 张")));
            }
            replace_top(
                st,
                Frame::Rende(RendeFrame {
                    actor: f.actor,
                    stage: RendeStage::Target,
                    selected: f.selected.clone(),
                }),
            );
            Some(Ok("请选择交给谁".to_string()))
        }
        "lijian" => {
            if f.selected.len() != 1 {
                return Some(Err(illegal("离间需弃置 1 张牌")));
            }
            replace_top(
                st,
                Frame::Lijian(LijianFrame {
                    actor: f.actor,
                    stage: LijianStage::Attacker,
                    selected: f.selected.clone(),
                    attacker: None,
                }),
            );
            Some(Ok("请选择第一名男性角色".to_string()))
        }
        _ => None,
    }
}

/// 返回 `Some` 表示目标 pending 已由技能帧处理。
pub(crate) fn target_action(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<Option<String>, EngineError> {
    match top(st).cloned() {
        Some(Frame::Rende(_)) => rende_target(st, pending, action).map(Some),
        Some(Frame::Lijian(_)) => lijian_target(st, pending, action).map(Some),
        Some(Frame::Tuxi(_)) => tuxi_target(st, pending, action).map(Some),
        _ => Ok(None),
    }
}
