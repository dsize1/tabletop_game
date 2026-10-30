//! P4.4.1 火包技能帧：拼点 / 强袭 / 驱虎 / 节命 / 天义 / 鞬出 / 双雄。
//!
//! 拼点（`PindianFrame`）是通用机制：双方各选一张手牌 → 同时亮出 → 比点数（A=1…K=13，
//! 同点视为发起者未赢）→ 结果经 `Outcome::Pindian` 回传父帧，拼点牌进弃牌堆。

use crate::resolve::{pop, push, replace_top, top, DamageType, Frame, Outcome};
use crate::skills;
use crate::state::*;
use crate::turn;
use crate::{illegal, internal};
use game_engine_core::EngineError;
use serde_json::json;

// ---------------------------------------------------------------------------
// 拼点（通用）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PindianStage {
    Initiator,
    Target,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PindianFrame {
    pub initiator: usize,
    pub target: usize,
    pub stage: PindianStage,
    pub initiator_card: Option<String>,
    pub target_card: Option<String>,
    pub data: serde_json::Value,
}

pub(crate) fn start_pindian(
    st: &mut SgsState,
    initiator: usize,
    target: usize,
    data: serde_json::Value,
) {
    push(
        st,
        Frame::Pindian(PindianFrame {
            initiator,
            target,
            stage: PindianStage::Initiator,
            initiator_card: None,
            target_card: None,
            data,
        }),
    );
}

pub(crate) fn step_pindian(st: &mut SgsState, f: PindianFrame) -> Result<bool, EngineError> {
    let (seat, stage_name) = match f.stage {
        PindianStage::Initiator => (f.initiator, "initiator"),
        PindianStage::Target => (f.target, "target"),
    };
    let options: Vec<PendingOption> = st.seats[seat]
        .hand
        .iter()
        .map(|id| PendingOption {
            id: format!("choose:{id}"),
            label: format!("拼点：{}", turn::card_label(st, id)),
        })
        .collect();
    if options.is_empty() {
        // 无牌可拼：发起者没赢（简化处理，正常入口会过滤）。
        finish_pindian(st, f, None, None)?;
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[seat].player_id.clone(),
        skill: Some("pindian".into()),
        options,
        data: json!({ "skill": "pindian", "stage": stage_name }),
    });
    replace_top(st, Frame::Pindian(f));
    Ok(true)
}

pub(crate) fn act_pindian(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let mut f = match top(st).cloned() {
        Some(Frame::Pindian(f)) => f,
        _ => return Err(internal("栈顶不是拼点帧")),
    };
    let card = action
        .strip_prefix("choose:")
        .ok_or_else(|| illegal("拼点需要选择一张手牌"))?;
    let seat = match f.stage {
        PindianStage::Initiator => f.initiator,
        PindianStage::Target => f.target,
    };
    if !st.seats[seat].hand.iter().any(|c| c == card) {
        return Err(illegal("手牌中没有该牌"));
    }
    match f.stage {
        PindianStage::Initiator => {
            f.initiator_card = Some(card.to_string());
            f.stage = PindianStage::Target;
            replace_top(st, Frame::Pindian(f));
            Ok("拼点：请目标选择手牌".to_string())
        }
        PindianStage::Target => {
            f.target_card = Some(card.to_string());
            finish_pindian(st, f, None, None)
        }
    }
}

fn finish_pindian(
    st: &mut SgsState,
    f: PindianFrame,
    forced_a: Option<String>,
    forced_b: Option<String>,
) -> Result<String, EngineError> {
    let a = forced_a.or(f.initiator_card.clone());
    let b = forced_b.or(f.target_card.clone());
    let (Some(a), Some(b)) = (a, b) else {
        pop(st);
        return Ok("拼点取消".to_string());
    };
    let ra = st.cards.get(&a).map(|c| c.rank).unwrap_or(0);
    let rb = st.cards.get(&b).map(|c| c.rank).unwrap_or(0);
    let winner = if ra > rb {
        Some(f.initiator)
    } else if rb > ra {
        Some(f.target)
    } else {
        None
    };
    let initiator_name = st.seats[f.initiator].general_name.clone();
    let target_name = st.seats[f.target].general_name.clone();
    st.push_log(
        None,
        format!(
            "拼点：{initiator_name}【{}】({ra}) vs {target_name}【{}】({rb}) → {}",
            turn::card_label(st, &a),
            turn::card_label(st, &b),
            match winner {
                Some(w) if w == f.initiator => "发起者赢",
                Some(_) => "目标赢",
                None => "平手（发起者未赢）",
            }
        ),
    );
    for (seat, card) in [(f.initiator, a.clone()), (f.target, b.clone())] {
        turn::take_card(st, seat, &card)?;
        turn::to_discard_card(st, &card);
    }
    st.outcome = Some(Outcome::Pindian {
        winner,
        initiator: f.initiator,
        target: f.target,
    });
    pop(st);
    Ok("拼点结算完毕".to_string())
}

// ---------------------------------------------------------------------------
// 强袭（典韦）：出牌阶段限两次，失 1 体力或弃武器 → 对未指定过的角色 1 伤
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QiangxiStage {
    Cost,
    Target,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QiangxiFrame {
    pub actor: usize,
    pub stage: QiangxiStage,
}

fn qiangxi_mask(st: &SgsState) -> u32 {
    st.counter("qiangxi_targets")
}

pub(crate) fn qiangxi_targets(st: &SgsState, actor: usize) -> Vec<usize> {
    let mask = qiangxi_mask(st);
    st.alive_indices()
        .into_iter()
        .filter(|t| *t != actor && mask & (1u32 << *t) == 0)
        .collect()
}

pub(crate) fn step_qiangxi(st: &mut SgsState, f: QiangxiFrame) -> Result<bool, EngineError> {
    match f.stage {
        QiangxiStage::Cost => {
            let mut options = Vec::new();
            if st.seats[f.actor].hp > 0 {
                options.push(PendingOption {
                    id: "choose:qiangxi:hp".into(),
                    label: "强袭：失去 1 点体力".into(),
                });
            }
            if st.equip_def(f.actor, EquipSlot::Weapon).is_some() {
                options.push(PendingOption {
                    id: "choose:qiangxi:weapon".into(),
                    label: "强袭：弃置武器牌".into(),
                });
            }
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            options.push(PendingOption {
                id: "pass".into(),
                label: "取消强袭".into(),
            });
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("qiangxi".into()),
                options,
                data: json!({ "skill": "qiangxi", "stage": "cost" }),
            });
            replace_top(st, Frame::Qiangxi(f));
            Ok(true)
        }
        QiangxiStage::Target => {
            let targets = qiangxi_targets(st, f.actor);
            if targets.is_empty() {
                pop(st);
                return Ok(false);
            }
            let options: Vec<PendingOption> = targets
                .iter()
                .map(|t| PendingOption {
                    id: format!("target:{}", st.seats[*t].player_id),
                    label: format!("强袭：对 {} 造成 1 点伤害", st.seats[*t].general_name),
                })
                .collect();
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("qiangxi".into()),
                options,
                data: json!({ "stage": "qiangxi" }),
            });
            replace_top(st, Frame::Qiangxi(f));
            Ok(true)
        }
    }
}

pub(crate) fn act_qiangxi(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Qiangxi(f)) => f,
        _ => return Err(internal("栈顶不是强袭帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("取消强袭".to_string());
    }
    match action {
        "choose:qiangxi:hp" => {
            st.seats[f.actor].hp -= 1;
            let hp = st.seats[f.actor].hp;
            let pid = st.seats[f.actor].player_id.clone();
            st.push_log(Some(&pid), format!("强袭：失去 1 点体力（{hp}）"));
            if hp <= 0 {
                pop(st);
                crate::resolve::push_dying(st, f.actor, None);
                return Ok("强袭：体力不足，进入濒死".to_string());
            }
            replace_top(
                st,
                Frame::Qiangxi(QiangxiFrame {
                    actor: f.actor,
                    stage: QiangxiStage::Target,
                }),
            );
            Ok("强袭：请选择目标".to_string())
        }
        "choose:qiangxi:weapon" => {
            let Some(Some(weapon)) = st.seats[f.actor].equips.insert(EquipSlot::Weapon, None)
            else {
                return Err(illegal("没有武器牌"));
            };
            turn::to_discard_card(st, &weapon);
            skills::xiaoji_on_lose_equip(st, f.actor);
            let pid = st.seats[f.actor].player_id.clone();
            st.push_log(
                Some(&pid),
                format!("强袭：弃置武器【{}】", turn::card_label(st, &weapon)),
            );
            replace_top(
                st,
                Frame::Qiangxi(QiangxiFrame {
                    actor: f.actor,
                    stage: QiangxiStage::Target,
                }),
            );
            Ok("强袭：请选择目标".to_string())
        }
        _ => Err(illegal("未知强袭选项")),
    }
}

pub(crate) fn qiangxi_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Qiangxi(f)) => f,
        _ => return Err(internal("栈顶不是强袭帧")),
    };
    let pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择目标"))?;
    if !pending
        .options
        .iter()
        .any(|o| o.id == format!("target:{pid}"))
    {
        return Err(illegal("目标不合法"));
    }
    let target = st.seat_index(pid).ok_or_else(|| illegal("目标不在座"))?;
    pop(st);
    let mask = qiangxi_mask(st) | (1u32 << target);
    st.set_counter("qiangxi_targets", mask);
    st.bump_counter("qiangxi_used");
    let pid_actor = st.seats[f.actor].player_id.clone();
    st.push_log(
        Some(&pid_actor),
        format!("强袭：对 {} 造成 1 点伤害", st.seats[target].general_name),
    );
    crate::resolve::apply_damage(st, target, 1, Some(f.actor), DamageType::Normal, None);
    Ok("强袭：已造成 1 点伤害".to_string())
}

// ---------------------------------------------------------------------------
// 驱虎（荀彧）：拼点；赢则目标攻击范围内你选的角色受伤，没赢则你受伤
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuhuStage {
    Target,
    WaitPindian,
    PickVictim,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QuhuFrame {
    pub actor: usize,
    pub target: Option<usize>,
    pub stage: QuhuStage,
}

pub(crate) fn step_quhu(st: &mut SgsState, f: QuhuFrame) -> Result<bool, EngineError> {
    match f.stage {
        QuhuStage::Target => {
            let options: Vec<PendingOption> = st
                .alive_indices()
                .into_iter()
                .filter(|t| {
                    *t != f.actor
                        && st.seats[*t].hp > st.seats[f.actor].hp
                        && !st.seats[*t].hand.is_empty()
                })
                .map(|t| PendingOption {
                    id: format!("target:{}", st.seats[t].player_id),
                    label: format!("驱虎：与 {} 拼点", st.seats[t].general_name),
                })
                .collect();
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("quhu".into()),
                options,
                data: json!({ "stage": "quhu" }),
            });
            replace_top(st, Frame::Quhu(f));
            Ok(true)
        }
        QuhuStage::WaitPindian => {
            let Some(target) = f.target else {
                pop(st);
                return Ok(false);
            };
            match st.outcome.take() {
                Some(Outcome::Pindian { winner, .. }) => {
                    if winner == Some(f.actor) {
                        replace_top(
                            st,
                            Frame::Quhu(QuhuFrame {
                                actor: f.actor,
                                target: f.target,
                                stage: QuhuStage::PickVictim,
                            }),
                        );
                        Ok(false)
                    } else {
                        pop(st);
                        st.push_log(
                            Some(&st.seats[f.actor].player_id.clone()),
                            format!(
                                "驱虎：拼点未赢，{} 对荀彧造成 1 点伤害",
                                st.seats[target].general_name
                            ),
                        );
                        crate::resolve::apply_damage(
                            st,
                            f.actor,
                            1,
                            Some(target),
                            DamageType::Normal,
                            None,
                        );
                        Ok(false)
                    }
                }
                _ => Err(internal("驱虎缺少拼点结果")),
            }
        }
        QuhuStage::PickVictim => {
            let Some(target) = f.target else {
                pop(st);
                return Ok(false);
            };
            let options: Vec<PendingOption> = st
                .alive_indices()
                .into_iter()
                .filter(|i| *i != target && st.in_attack_range(target, *i))
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!(
                        "驱虎：令 {} 对 {} 造成 1 点伤害",
                        st.seats[target].general_name, st.seats[i].general_name
                    ),
                })
                .collect();
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("quhu".into()),
                options,
                data: json!({ "stage": "quhu_victim" }),
            });
            replace_top(st, Frame::Quhu(f));
            Ok(true)
        }
    }
}

pub(crate) fn quhu_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Quhu(f)) => f,
        _ => return Err(internal("栈顶不是驱虎帧")),
    };
    let pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择目标"))?;
    if !pending
        .options
        .iter()
        .any(|o| o.id == format!("target:{pid}"))
    {
        return Err(illegal("目标不合法"));
    }
    let target = st.seat_index(pid).ok_or_else(|| illegal("目标不在座"))?;
    match f.stage {
        QuhuStage::Target => {
            replace_top(
                st,
                Frame::Quhu(QuhuFrame {
                    actor: f.actor,
                    target: Some(target),
                    stage: QuhuStage::WaitPindian,
                }),
            );
            start_pindian(st, f.actor, target, json!({ "skill": "quhu" }));
            Ok("驱虎：拼点开始".to_string())
        }
        QuhuStage::PickVictim => {
            let Some(src) = f.target else {
                return Err(internal("驱虎缺少拼点目标"));
            };
            pop(st);
            st.push_log(
                Some(&st.seats[f.actor].player_id.clone()),
                format!(
                    "驱虎：令 {} 对 {} 造成 1 点伤害",
                    st.seats[src].general_name, st.seats[target].general_name
                ),
            );
            crate::resolve::apply_damage(st, target, 1, Some(src), DamageType::Normal, None);
            Ok("驱虎：已造成 1 点伤害".to_string())
        }
        QuhuStage::WaitPindian => Err(illegal("当前不能选择目标")),
    }
}

// ---------------------------------------------------------------------------
// 节命（荀彧）：受到伤害后，令一名角色摸至体力上限（至多 5）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JiemingFrame {
    pub actor: usize,
}

pub(crate) fn step_jieming(st: &mut SgsState, f: JiemingFrame) -> Result<bool, EngineError> {
    let options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| {
            let target = (st.seats[*i].max_hp.max(0) as usize).min(5);
            st.seats[*i].hand.len() < target
        })
        .map(|i| {
            let target = (st.seats[i].max_hp.max(0) as usize).min(5);
            PendingOption {
                id: format!("target:{}", st.seats[i].player_id),
                label: format!("节命：令 {} 摸至 {} 张", st.seats[i].general_name, target),
            }
        })
        .collect();
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "target".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("jieming".into()),
        options,
        data: json!({ "stage": "jieming" }),
    });
    replace_top(st, Frame::Jieming(f));
    Ok(true)
}

pub(crate) fn jieming_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Jieming(f)) => f,
        _ => return Err(internal("栈顶不是节命帧")),
    };
    let pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择目标"))?;
    if !pending
        .options
        .iter()
        .any(|o| o.id == format!("target:{pid}"))
    {
        return Err(illegal("目标不合法"));
    }
    let target = st.seat_index(pid).ok_or_else(|| illegal("目标不在座"))?;
    pop(st);
    let want = (st.seats[target].max_hp.max(0) as usize).min(5);
    let need = want.saturating_sub(st.seats[target].hand.len()) as u32;
    let drawn = if need > 0 {
        let mut ev = Vec::new();
        turn::draw_cards(st, target, need, &mut ev)
    } else {
        0
    };
    let over = st.seats[target]
        .hand
        .len()
        .saturating_sub(st.config.max_hand_cards as usize);
    if over > 0 {
        crate::resolve::push_discard(st, target, over, "hand_cap");
    }
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        format!("节命：令 {} 摸 {drawn} 张", st.seats[target].general_name),
    );
    Ok(format!(
        "节命：{} 摸 {drawn} 张",
        st.seats[target].general_name
    ))
}

// ---------------------------------------------------------------------------
// 天义（太史慈）：拼点；赢则本回合杀次数+1/无距离/多目标，没赢则不能使用杀
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TianyiStage {
    Target,
    WaitPindian,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TianyiFrame {
    pub actor: usize,
    pub target: Option<usize>,
    pub stage: TianyiStage,
}

pub(crate) fn step_tianyi(st: &mut SgsState, f: TianyiFrame) -> Result<bool, EngineError> {
    match f.stage {
        TianyiStage::Target => {
            let options: Vec<PendingOption> = st
                .alive_indices()
                .into_iter()
                .filter(|t| *t != f.actor && !st.seats[*t].hand.is_empty())
                .map(|t| PendingOption {
                    id: format!("target:{}", st.seats[t].player_id),
                    label: format!("天义：与 {} 拼点", st.seats[t].general_name),
                })
                .collect();
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("tianyi".into()),
                options,
                data: json!({ "stage": "tianyi" }),
            });
            replace_top(st, Frame::Tianyi(f));
            Ok(true)
        }
        TianyiStage::WaitPindian => match st.outcome.take() {
            Some(Outcome::Pindian { winner, .. }) => {
                let pid = st.seats[f.actor].player_id.clone();
                if winner == Some(f.actor) {
                    st.set_counter("tianyi", 1);
                    st.push_log(
                        Some(&pid),
                        "天义：拼点赢，本回合杀次数+1/无距离/可多指定一个目标",
                    );
                } else {
                    st.set_counter("tianyi_lose", 1);
                    st.push_log(Some(&pid), "天义：拼点没赢，本回合不能使用【杀】");
                }
                pop(st);
                Ok(false)
            }
            _ => Err(internal("天义缺少拼点结果")),
        },
    }
}

pub(crate) fn tianyi_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Tianyi(f)) => f,
        _ => return Err(internal("栈顶不是天义帧")),
    };
    let pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择目标"))?;
    if !pending
        .options
        .iter()
        .any(|o| o.id == format!("target:{pid}"))
    {
        return Err(illegal("目标不合法"));
    }
    let target = st.seat_index(pid).ok_or_else(|| illegal("目标不在座"))?;
    replace_top(
        st,
        Frame::Tianyi(TianyiFrame {
            actor: f.actor,
            target: Some(target),
            stage: TianyiStage::WaitPindian,
        }),
    );
    start_pindian(st, f.actor, target, json!({ "skill": "tianyi" }));
    Ok("天义：拼点开始".to_string())
}

// ---------------------------------------------------------------------------
// 鞬出（庞德）：使用【杀】指定目标后，可弃其一张牌
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JianduFrame {
    pub user: usize,
    pub target: usize,
    pub card: String,
    pub need: u32,
    pub no_shan: bool,
    pub ignore_armor: bool,
    pub damage_type: DamageType,
}

pub(crate) fn target_has_cards(st: &SgsState, target: usize) -> bool {
    let s = &st.seats[target];
    !s.hand.is_empty() || !s.judge_zone.is_empty() || s.equips.values().any(|c| c.is_some())
}

pub(crate) fn step_jiandu(st: &mut SgsState, f: JianduFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = Vec::new();
    for (slot, card) in &st.seats[f.target].equips {
        if let Some(id) = card {
            options.push(PendingOption {
                id: format!("pick:equip:{slot:?}"),
                label: format!("鞬出：弃置装备【{}】", turn::card_label(st, id)),
            });
        }
    }
    if !st.seats[f.target].hand.is_empty() {
        options.push(PendingOption {
            id: "pick:hand".into(),
            label: "鞬出：弃置一张手牌（随机）".into(),
        });
    }
    for id in &st.seats[f.target].judge_zone {
        options.push(PendingOption {
            id: format!("pick:judge:{id}"),
            label: format!("鞬出：弃置判定区【{}】", turn::card_label(st, id)),
        });
    }
    if options.is_empty() {
        pop(st);
        crate::resolve_skill::push_sha_target_response(
            st,
            f.user,
            f.target,
            &f.card,
            f.need,
            f.no_shan,
            f.ignore_armor,
            f.damage_type,
        );
        return Ok(false);
    }
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动鞬出".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.user].player_id.clone(),
        skill: Some("jiandu".into()),
        options,
        data: json!({ "skill": "jiandu", "target": st.seats[f.target].player_id }),
    });
    replace_top(st, Frame::Jiandu(f));
    Ok(true)
}

pub(crate) fn act_jiandu(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Jiandu(f)) => f,
        _ => return Err(internal("栈顶不是鞬出帧")),
    };
    if action == "pass" {
        pop(st);
        crate::resolve_skill::push_sha_target_response(
            st,
            f.user,
            f.target,
            &f.card,
            f.need,
            f.no_shan,
            f.ignore_armor,
            f.damage_type,
        );
        return Ok("不发动鞬出".to_string());
    }
    let mut equipment = false;
    let discarded: Option<String> = if let Some(slot) = action.strip_prefix("pick:equip:") {
        let slot = match slot {
            "Weapon" => EquipSlot::Weapon,
            "Armor" => EquipSlot::Armor,
            "HorseOffense" => EquipSlot::HorseOffense,
            "HorseDefense" => EquipSlot::HorseDefense,
            "Treasure" => EquipSlot::Treasure,
            _ => return Err(illegal("未知装备槽")),
        };
        let Some(Some(id)) = st.seats[f.target].equips.insert(slot, None) else {
            return Err(illegal("该装备槽没有牌"));
        };
        equipment = true;
        Some(id)
    } else if action == "pick:hand" {
        if st.seats[f.target].hand.is_empty() {
            return Err(illegal("目标没有手牌"));
        }
        let n = st.seats[f.target].hand.len();
        let idx = st.next_index(n);
        Some(st.seats[f.target].hand.remove(idx))
    } else if let Some(id) = action.strip_prefix("pick:judge:") {
        let pos = st.seats[f.target]
            .judge_zone
            .iter()
            .position(|c| c == id)
            .ok_or_else(|| illegal("判定区没有该牌"))?;
        Some(st.seats[f.target].judge_zone.remove(pos))
    } else {
        return Err(illegal("未知鞬出选项"));
    };
    let Some(discarded) = discarded else {
        return Err(internal("鞬出缺少弃置牌"));
    };
    turn::to_discard_card(st, &discarded);
    if equipment {
        skills::xiaoji_on_lose_equip(st, f.target);
    }
    let pid = st.seats[f.user].player_id.clone();
    st.push_log(
        Some(&pid),
        format!(
            "鞬出：弃置 {} 的【{}】{}",
            st.seats[f.target].general_name,
            turn::card_label(st, &discarded),
            if equipment {
                "，该角色不能使用【闪】"
            } else {
                "，其获得此【杀】"
            }
        ),
    );
    pop(st);
    if equipment {
        crate::resolve_skill::push_sha_target_response(
            st,
            f.user,
            f.target,
            &f.card,
            f.need,
            true,
            f.ignore_armor,
            f.damage_type,
        );
        Ok("鞬出：目标不能使用【闪】".to_string())
    } else {
        // 非装备牌：目标获得此【杀】，本【杀】结束。
        if f.card.is_empty() || !st.deck.processing.iter().any(|c| c == &f.card) {
            return Err(internal("鞬出：缺少【杀】的牌实例"));
        }
        crate::resolve::leave_processing(st, &f.card);
        st.seats[f.target].hand.push(f.card.clone());
        let over = st.seats[f.target]
            .hand
            .len()
            .saturating_sub(st.config.max_hand_cards as usize);
        if over > 0 {
            crate::resolve::push_discard(st, f.target, over, "hand_cap");
        }
        // 弹出原【杀】帧（结束本次使用）。
        pop(st);
        Ok("鞬出：目标获得此【杀】".to_string())
    }
}

// ---------------------------------------------------------------------------
// 双雄（颜良文丑）：摸牌阶段改为判定并获得判定牌；本回合异色手牌当【决斗】
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ShuangxiongFrame {
    pub actor: usize,
}

pub(crate) fn step_shuangxiong(
    st: &mut SgsState,
    f: ShuangxiongFrame,
) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("shuangxiong".into()),
        options: vec![
            PendingOption {
                id: "choose:shuangxiong".into(),
                label: "双雄：判定并获得判定牌（本回合异色手牌当【决斗】）".into(),
            },
            PendingOption {
                id: "pass".into(),
                label: "正常摸牌".into(),
            },
        ],
        data: json!({ "skill": "shuangxiong" }),
    });
    replace_top(st, Frame::Shuangxiong(f));
    Ok(true)
}

pub(crate) fn act_shuangxiong(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Shuangxiong(f)) => f,
        _ => return Err(internal("栈顶不是双雄帧")),
    };
    if action == "choose:shuangxiong" {
        pop(st);
        st.set_counter("skip_draw", 1);
        push(
            st,
            Frame::QuickJudge(crate::resolve::QuickJudgeFrame {
                owner: f.actor,
                reason: "shuangxiong".into(),
                stage: crate::resolve::QuickJudgeStage::Reveal,
                judge_card: None,
                rewrite_order: Vec::new(),
                rewrite_cursor: 0,
                data: json!({ "actor": f.actor }),
                saved_outcome: None,
            }),
        );
        Ok("双雄：判定中".to_string())
    } else {
        pop(st);
        Ok("双雄：正常摸牌".to_string())
    }
}

pub(crate) fn shuangxiong_conversion(
    st: &SgsState,
    seat: usize,
    card_id: &str,
) -> Option<(&'static str, &'static str)> {
    if !skills::has(st, seat, "shuangxiong") {
        return None;
    }
    let judged = st.counter("shuangxiong_color");
    if judged == 0 {
        return None;
    }
    let suit = st.cards.get(card_id)?.suit;
    let is_red = skills::is_red(suit);
    let judged_red = judged == 1;
    if is_red == judged_red {
        return None;
    }
    Some(("shuangxiong", "双雄"))
}
