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
    // 界激将/界护驾：其他同势力角色于其回合外代出【杀】/【闪】后，主公摸 1（每回合限一次）。
    if f.needed == "sha" {
        crate::resolve_jiexian::jijiang_lord_draw(st, by);
    } else {
        crate::skills::hujia_lord_draw(st, by);
    }
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
    let jie = crate::skills::is_jiexian(st, f.actor);
    // 界仁德：每名角色每阶段限一次。
    if jie && st.counter(&format!("rende_target:{target}")) > 0 {
        return Err(illegal("界仁德本阶段已对该角色发动过"));
    }
    let cards = f.selected.clone();
    let n = cards.len();
    for id in &cards {
        turn::take_card(st, f.actor, id)?;
        st.seats[target].hand.push(id.clone());
    }
    let before = st.counter("rende_given");
    let given = before + n as u32;
    st.set_counter("rende_given", given);
    if jie {
        st.set_counter(&format!("rende_target:{target}"), 1);
    }
    let mut healed = false;
    if !jie
        && given >= 2
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
    // 界仁德：本阶段给出第二张牌时，可视为使用一张基本牌。
    let mut basic = false;
    if jie && before < 2 && given >= 2 && st.counter("rende_basic_offered") == 0 {
        st.set_counter("rende_basic_offered", 1);
        basic = true;
    }
    pop(st);
    if basic {
        crate::resolve::push(
            st,
            crate::resolve::Frame::RendeBasic(crate::resolve_jiexian::RendeBasicFrame {
                actor: f.actor,
            }),
        );
        return Ok("仁德：可视为使用一张基本牌".to_string());
    }
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
            // 空城（锁定）：无手牌者不能成为【决斗】目标（离间视为决斗）。
            let options: Vec<PendingOption> = male_targets(st, f.actor, f.attacker)
                .into_iter()
                .filter(|o| {
                    o.id.strip_prefix("target:")
                        .and_then(|pid| st.seat_index(pid))
                        .is_some_and(|i| !skills::kongcheng_blocks(st, i))
                })
                .collect();
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
    if !pending
        .options
        .iter()
        .any(|o| o.id == format!("target:{pid}"))
    {
        return Err(illegal("目标不合法"));
    }
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
                    need: 1,
                    played: 0,
                    no_shan: false,
                    duel_initiator: None,
                    is_sha: false,
                }),
            );
            Ok("离间已生效：目标需出【杀】".to_string())
        }
        _ => Err(illegal("未知离间阶段")),
    }
}

// ---------------------------------------------------------------------------
// 流离（P4.1：成为【杀】目标时弃一张手牌转移）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiuliStage {
    /// 选择弃置哪张手牌（或放弃）。
    Choice,
    /// 选择转移目标的受害者。
    Aim,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LiuliFrame {
    pub user: usize,
    pub original: usize,
    pub card: String,
    /// 原【杀】的响应牌数（无双）。
    pub need: u32,
    /// 原【杀】的铁骑结果（转移后对新目标不生效）。
    pub no_shan: bool,
    pub stage: LiuliStage,
}

fn liuli_victims(st: &SgsState, f: &LiuliFrame) -> Vec<usize> {
    st.alive_indices()
        .into_iter()
        .filter(|v| {
            *v != f.original
                && *v != f.user
                && st.in_attack_range(f.original, *v)
                && !crate::skills::kongcheng_blocks(st, *v)
        })
        .collect()
}

pub(crate) fn step_liuli(st: &mut SgsState, f: LiuliFrame) -> Result<bool, EngineError> {
    match f.stage {
        LiuliStage::Choice => {
            let mut options: Vec<PendingOption> = st.seats[f.original]
                .hand
                .iter()
                .map(|id| PendingOption {
                    id: format!("choose:{id}"),
                    label: format!("弃置【{}】并转移【杀】", turn::card_label(st, id)),
                })
                .collect();
            options.push(PendingOption {
                id: "pass".into(),
                label: "不发动流离".into(),
            });
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: st.seats[f.original].player_id.clone(),
                skill: Some("liuli".into()),
                options,
                data: json!({ "skill": "liuli", "card": f.card }),
            });
            replace_top(st, Frame::Liuli(f));
            Ok(true)
        }
        LiuliStage::Aim => {
            let options: Vec<PendingOption> = liuli_victims(st, &f)
                .into_iter()
                .map(|v| PendingOption {
                    id: format!("target:{}", st.seats[v].player_id),
                    label: format!("转移给 {}", st.seats[v].general_name),
                })
                .collect();
            if options.is_empty() {
                // 没有可转移目标：按原目标继续响应。
                let original = f.original;
                let no_shan = f.no_shan;
                let need = f.need;
                pop(st);
                push_liuli_respond(st, f.user, original, &f.card, need, no_shan);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.original].player_id.clone(),
                skill: Some("liuli".into()),
                options,
                data: json!({ "purpose": "liuli" }),
            });
            replace_top(st, Frame::Liuli(f));
            Ok(true)
        }
    }
}

/// 给最终受害者压入【闪】响应帧。
fn push_liuli_respond(
    st: &mut SgsState,
    user: usize,
    victim: usize,
    card: &str,
    need: u32,
    no_shan: bool,
) {
    crate::resolve_skill::push_sha_target_response(
        st,
        user,
        victim,
        card,
        need,
        no_shan,
        crate::skills::has_equipment_skill(st, user, "qinggang_jian"),
        crate::resolve::DamageType::Normal,
    );
    // 更新【杀】帧的目标（日志/结算展示）。
    for frame in st.resolution.iter_mut().rev() {
        if let Frame::Sha(sf) = frame {
            sf.target = Some(victim);
            break;
        }
    }
}

pub(crate) fn act_liuli(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Liuli(f)) => f,
        _ => return Err(internal("栈顶不是流离帧")),
    };
    if action == "pass" {
        pop(st);
        push_liuli_respond(st, f.user, f.original, &f.card, f.need, f.no_shan);
        return Ok("不发动流离".to_string());
    }
    let card_id = action
        .strip_prefix("choose:")
        .ok_or_else(|| illegal("需要选择弃置的牌或放弃"))?;
    if !st.seats[f.original].hand.iter().any(|c| c == card_id) {
        return Err(illegal("手牌中没有该牌"));
    }
    turn::take_card(st, f.original, card_id)?;
    turn::to_discard_card(st, card_id);
    let label = turn::card_label(st, card_id);
    let pid = st.seats[f.original].player_id.clone();
    st.push_log(Some(&pid), format!("流离：弃置【{label}】，选择转移目标"));
    replace_top(
        st,
        Frame::Liuli(LiuliFrame {
            stage: LiuliStage::Aim,
            ..f
        }),
    );
    Ok("请选择转移目标".to_string())
}

pub(crate) fn liuli_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Liuli(f)) => f,
        _ => return Err(internal("栈顶不是流离帧")),
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
    let victim = st.seat_index(pid).ok_or_else(|| illegal("目标不在座"))?;
    pop(st);
    push_liuli_respond(st, f.user, victim, &f.card, f.need, false);
    let pid = st.seats[f.original].player_id.clone();
    let victim_name = st.seats[victim].general_name.clone();
    st.push_log(Some(&pid), format!("流离：将【杀】转移给 {victim_name}"));
    Ok(format!("已将【杀】转移给 {victim_name}"))
}

// ---------------------------------------------------------------------------
// 刚烈 / 洛神 / 反间（P4.1-C2-2：判定与选择类）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GanglieFrame {
    pub victim: usize,
    pub source: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LuoshenFrame {
    pub actor: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FanjianStage {
    Target,
    Suit,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FanjianFrame {
    pub actor: usize,
    pub stage: FanjianStage,
    pub target: Option<usize>,
}

pub(crate) fn step_ganglie(st: &mut SgsState, f: GanglieFrame) -> Result<bool, EngineError> {
    let mut options = Vec::new();
    if st.seats[f.source].hand.len() >= 2 {
        options.push(PendingOption {
            id: "choose:discard".into(),
            label: "弃置两张手牌".into(),
        });
    }
    options.push(PendingOption {
        id: "choose:damage".into(),
        label: "受到 1 点伤害".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.source].player_id.clone(),
        skill: Some("ganglie".into()),
        options,
        data: json!({ "skill": "ganglie", "victim": st.seats[f.victim].player_id }),
    });
    replace_top(st, Frame::Ganglie(f));
    Ok(true)
}

pub(crate) fn act_ganglie(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Ganglie(f)) => f,
        _ => return Err(internal("栈顶不是刚烈帧")),
    };
    if action == "choose:discard" {
        if st.seats[f.source].hand.len() < 2 {
            return Err(illegal("手牌不足 2 张"));
        }
        replace_top(
            st,
            Frame::MultiSelect(crate::resolve::MultiSelectFrame {
                actor: f.source,
                purpose: "ganglie".into(),
                selected: Vec::new(),
                min: 2,
                max: 2,
                data: json!({}),
            }),
        );
        return Ok("请选择弃置的 2 张手牌".to_string());
    }
    if action == "choose:damage" {
        pop(st);
        let victim = f.victim;
        let source = f.source;
        crate::resolve::apply_damage(
            st,
            source,
            1,
            Some(victim),
            crate::resolve::DamageType::Normal,
            None,
        );
        let pid = st.seats[victim].player_id.clone();
        st.push_log(Some(&pid), "刚烈：伤害来源选择受到 1 点伤害");
        return Ok("刚烈：来源受到 1 点伤害".to_string());
    }
    Err(illegal("未知刚烈选项"))
}

pub(crate) fn step_luoshen(st: &mut SgsState, f: LuoshenFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("luoshen".into()),
        options: vec![
            PendingOption {
                id: "choose:luoshen".into(),
                label: "洛神：判定（黑色则获得并继续）".into(),
            },
            PendingOption {
                id: "pass".into(),
                label: "结束洛神".into(),
            },
        ],
        data: json!({ "skill": "luoshen" }),
    });
    replace_top(st, Frame::Luoshen(f));
    Ok(true)
}

pub(crate) fn act_luoshen(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Luoshen(f)) => f,
        _ => return Err(internal("栈顶不是洛神帧")),
    };
    if action != "choose:luoshen" {
        pop(st);
        return Ok("结束洛神".to_string());
    }
    // 洛神判定走通用 judge_before（可被鬼才改判）。
    pop(st);
    crate::resolve::push(
        st,
        crate::resolve::Frame::QuickJudge(crate::resolve::QuickJudgeFrame {
            owner: f.actor,
            reason: "luoshen".into(),
            stage: crate::resolve::QuickJudgeStage::Reveal,
            judge_card: None,
            rewrite_order: Vec::new(),
            rewrite_cursor: 0,
            data: json!({}),
            saved_outcome: None,
        }),
    );
    Ok("洛神：判定中".to_string())
}

pub(crate) fn step_fanjian(st: &mut SgsState, f: FanjianFrame) -> Result<bool, EngineError> {
    match f.stage {
        FanjianStage::Target => {
            let options: Vec<PendingOption> = st
                .alive_indices()
                .into_iter()
                .filter(|i| *i != f.actor)
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("令 {} 选择花色", st.seats[i].general_name),
                })
                .collect();
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("fanjian".into()),
                options,
                data: json!({ "purpose": "fanjian", "stage": "target" }),
            });
            replace_top(st, Frame::Fanjian(f));
            Ok(true)
        }
        FanjianStage::Suit => {
            let Some(target) = f.target else {
                return Err(internal("反间缺少目标"));
            };
            let suits = [
                ("spade", "黑桃"),
                ("heart", "红桃"),
                ("club", "梅花"),
                ("diamond", "方块"),
            ];
            let options = suits
                .iter()
                .map(|(code, name)| PendingOption {
                    id: format!("choose:suit:{code}"),
                    label: format!("选择{name}"),
                })
                .collect();
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: st.seats[target].player_id.clone(),
                skill: Some("fanjian".into()),
                options,
                data: json!({ "skill": "fanjian", "stage": "suit", "target": st.seats[target].player_id }),
            });
            replace_top(st, Frame::Fanjian(f));
            Ok(true)
        }
    }
}

pub(crate) fn fanjian_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Fanjian(f)) => f,
        _ => return Err(internal("栈顶不是反间帧")),
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
        Frame::Fanjian(FanjianFrame {
            stage: FanjianStage::Suit,
            target: Some(target),
            ..f
        }),
    );
    Ok(format!("{} 请选择花色", st.seats[target].general_name))
}

pub(crate) fn act_fanjian(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Fanjian(f)) => f,
        _ => return Err(internal("栈顶不是反间帧")),
    };
    let (target, suit_code) = match f.stage {
        FanjianStage::Suit => {
            let target = f.target.ok_or_else(|| internal("反间缺少目标"))?;
            let code = action
                .strip_prefix("choose:suit:")
                .ok_or_else(|| illegal("需要选择花色"))?;
            (target, code.to_string())
        }
        FanjianStage::Target => return Err(illegal("尚未选择反间目标")),
    };
    let suit_name = match suit_code.as_str() {
        "spade" => "黑桃",
        "heart" => "红桃",
        "club" => "梅花",
        "diamond" => "方块",
        _ => return Err(illegal("非法花色")),
    };
    if st.seats[f.actor].hand.is_empty() {
        return Err(internal("反间者没有手牌"));
    }
    let n = st.seats[f.actor].hand.len();
    let idx = st.next_index(n);
    let card = st.seats[f.actor].hand.remove(idx);
    let inst = st.cards.get(&card).cloned();
    let Some(inst) = inst else {
        return Err(internal("反间牌缺少实例"));
    };
    let label = turn::card_label(st, &card);
    st.seats[target].hand.push(card.clone());
    let over = st.seats[target]
        .hand
        .len()
        .saturating_sub(st.config.max_hand_cards as usize);
    if over > 0 {
        crate::resolve::push_discard(st, target, over, "hand_cap");
    }
    let actual_name = match inst.suit {
        Suit::Spade => "黑桃",
        Suit::Heart => "红桃",
        Suit::Club => "梅花",
        Suit::Diamond => "方块",
    };
    let match_suit = actual_name == suit_name;
    st.set_counter("fanjian_used", 1);
    let actor_pid = st.seats[f.actor].player_id.clone();
    let target_name = st.seats[target].general_name.clone();
    pop(st);
    if match_suit {
        st.push_log(
            Some(&actor_pid),
            format!("反间：{target_name} 选择{suit_name}并获得【{label}】，花色相同，未造成伤害"),
        );
        Ok("反间：花色相同，未造成伤害".to_string())
    } else {
        st.push_log(
            Some(&actor_pid),
            format!(
                "反间：{target_name} 选择{suit_name}并获得【{label}】，花色不同，受到 1 点伤害"
            ),
        );
        crate::resolve::apply_damage(
            st,
            target,
            1,
            Some(f.actor),
            crate::resolve::DamageType::Normal,
            None,
        );
        Ok("反间：目标受到 1 点伤害".to_string())
    }
}

// ---------------------------------------------------------------------------
// 青囊 / 结姻 / 遗计（P4.1-C2：主动技与伤害后分配）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QingnangFrame {
    pub actor: usize,
    pub selected: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JieyinFrame {
    pub actor: usize,
    pub selected: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct YijiFrame {
    pub actor: usize,
    /// 待分配的牌（仍在 actor 手牌中，分配时移出）。
    pub cards: Vec<String>,
}

pub(crate) fn step_qingnang(st: &mut SgsState, f: QingnangFrame) -> Result<bool, EngineError> {
    let options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| st.seats[*i].hp < st.seats[*i].max_hp)
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("令 {} 回复 1 点体力", st.seats[i].general_name),
        })
        .collect();
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "target".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("qingnang".into()),
        options,
        data: json!({ "purpose": "qingnang", "selected": f.selected }),
    });
    replace_top(st, Frame::Qingnang(f));
    Ok(true)
}

pub(crate) fn qingnang_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Qingnang(f)) => f,
        _ => return Err(internal("栈顶不是青囊帧")),
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
    let card = f
        .selected
        .first()
        .cloned()
        .ok_or_else(|| internal("青囊缺少弃置牌"))?;
    turn::take_card(st, f.actor, &card)?;
    turn::to_discard_card(st, &card);
    st.seats[target].hp += 1;
    st.set_counter("qingnang_used", 1);
    let label = turn::card_label(st, &card);
    let hp = st.seats[target].hp;
    let target_name = st.seats[target].general_name.clone();
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        format!("青囊：弃置【{label}】，令 {target_name} 回复 1 点体力（{hp}）"),
    );
    pop(st);
    Ok(format!("{target_name} 回复 1 点体力"))
}

pub(crate) fn step_jieyin(st: &mut SgsState, f: JieyinFrame) -> Result<bool, EngineError> {
    let jie = crate::skills::is_jiexian(st, f.actor);
    let options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| {
            *i != f.actor
                && crate::skills::is_male(st, *i)
                && (jie || st.seats[*i].hp < st.seats[*i].max_hp)
        })
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("与 {} 结姻", st.seats[i].general_name),
        })
        .collect();
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "target".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("jieyin".into()),
        options,
        data: json!({ "purpose": "jieyin", "selected": f.selected }),
    });
    replace_top(st, Frame::Jieyin(f));
    Ok(true)
}

pub(crate) fn jieyin_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Jieyin(f)) => f,
        _ => return Err(internal("栈顶不是结姻帧")),
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
    for id in &f.selected {
        turn::take_card(st, f.actor, id)?;
        turn::to_discard_card(st, id);
    }
    st.set_counter("jieyin_used", 1);
    let actor_name = st.seats[f.actor].general_name.clone();
    let target_name = st.seats[target].general_name.clone();
    let actor_pid = st.seats[f.actor].player_id.clone();
    if crate::skills::is_jiexian(st, f.actor) {
        // 界结姻：体力值较大的角色摸 1，较小的角色回复 1（相同则各摸 1）。
        let a_hp = st.seats[f.actor].hp;
        let t_hp = st.seats[target].hp;
        let (mut actor_draw, mut actor_heal, mut target_draw, mut target_heal) =
            (false, false, false, false);
        if a_hp > t_hp {
            actor_draw = true;
            target_heal = true;
        } else if t_hp > a_hp {
            target_draw = true;
            actor_heal = true;
        } else {
            actor_draw = true;
            target_draw = true;
        }
        let mut ev = Vec::new();
        if actor_draw {
            turn::draw_cards(st, f.actor, 1, &mut ev);
        }
        if target_draw {
            turn::draw_cards(st, target, 1, &mut ev);
        }
        if actor_heal && st.seats[f.actor].hp < st.seats[f.actor].max_hp {
            st.seats[f.actor].hp += 1;
        }
        if target_heal && st.seats[target].hp < st.seats[target].max_hp {
            st.seats[target].hp += 1;
        }
        st.push_log(
            Some(&actor_pid),
            format!("结姻：{actor_name} 与 {target_name}（体力高者摸 1 / 低者回复 1；相同各摸 1）"),
        );
        pop(st);
        return Ok(format!("与 {target_name} 结姻"));
    }
    st.seats[target].hp += 1;
    let mut self_heal = false;
    if st.seats[f.actor].hp < st.seats[f.actor].max_hp {
        st.seats[f.actor].hp += 1;
        self_heal = true;
    }
    st.push_log(
        Some(&actor_pid),
        format!(
            "结姻：{actor_name} 与 {target_name} 各回复 1 点体力{}",
            if self_heal { "" } else { "（自己已满）" }
        ),
    );
    pop(st);
    Ok(format!("与 {target_name} 结姻"))
}

pub(crate) fn step_yiji(st: &mut SgsState, f: YijiFrame) -> Result<bool, EngineError> {
    let Some(card) = f.cards.first().cloned() else {
        pop(st);
        return Ok(false);
    };
    let label = turn::card_label(st, &card);
    let jie = crate::skills::is_jiexian(st, f.actor);
    let options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| !jie || *i != f.actor)
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: if i == f.actor {
                format!("将【{label}】留给自己")
            } else {
                format!("将【{label}】交给 {}", st.seats[i].general_name)
            },
        })
        .collect();
    st.pending = Some(Pending {
        kind: "target".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("yiji".into()),
        options,
        data: json!({ "purpose": "yiji", "card": card, "remaining": f.cards.len() }),
    });
    replace_top(st, Frame::Yiji(f));
    Ok(true)
}

pub(crate) fn yiji_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let mut f = match top(st).cloned() {
        Some(Frame::Yiji(f)) => f,
        _ => return Err(internal("栈顶不是遗计帧")),
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
    let card = f.cards.remove(0);
    let label = turn::card_label(st, &card);
    if target != f.actor {
        turn::take_card(st, f.actor, &card)?;
        st.seats[target].hand.push(card.clone());
        let over = st.seats[target]
            .hand
            .len()
            .saturating_sub(st.config.max_hand_cards as usize);
        if over > 0 {
            crate::resolve::push_discard(st, target, over, "hand_cap");
        }
        let target_name = st.seats[target].general_name.clone();
        st.push_log(
            Some(&st.seats[f.actor].player_id.clone()),
            format!("遗计：将【{label}】交给 {target_name}"),
        );
    }
    if f.cards.is_empty() {
        // 分配结束后检查持有上限。
        let over = st.seats[f.actor]
            .hand
            .len()
            .saturating_sub(st.config.max_hand_cards as usize);
        if over > 0 {
            crate::resolve::push_discard(st, f.actor, over, "hand_cap");
        }
        pop(st);
        Ok("遗计分配完毕".to_string())
    } else {
        replace_top(st, Frame::Yiji(f));
        Ok("继续分配遗计牌".to_string())
    }
}

// ---------------------------------------------------------------------------
// 裸衣（P4.1：摸牌阶段可选）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LuoyiFrame {
    pub actor: usize,
}

pub(crate) fn step_luoyi(st: &mut SgsState, f: LuoyiFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("luoyi".into()),
        options: vec![
            PendingOption {
                id: "choose:luoyi".into(),
                label: if crate::skills::is_jiexian(st, f.actor) {
                    "发动裸衣：亮出牌堆顶 3 张，跳过摸牌并获得基本牌/武器/【决斗】，伤害 +1 直到下回合开始".into()
                } else {
                    "发动裸衣：本回合少摸 1 张牌，使用【杀】/【决斗】造成的伤害 +1".into()
                },
            },
            PendingOption {
                id: "pass".into(),
                label: "不发动裸衣".into(),
            },
        ],
        data: json!({ "skill": "luoyi" }),
    });
    replace_top(st, Frame::Luoyi(f));
    Ok(true)
}

pub(crate) fn act_luoyi(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let actor = match top(st).cloned() {
        Some(Frame::Luoyi(f)) => f.actor,
        _ => return Err(internal("栈顶不是裸衣帧")),
    };
    if action == "choose:luoyi" {
        if crate::skills::is_jiexian(st, actor) {
            let mut ev = Vec::new();
            let mut revealed = Vec::new();
            for _ in 0..3 {
                let Some(id) = turn::draw_one(st, &mut ev) else {
                    break;
                };
                st.deck.processing.push(id.clone());
                revealed.push(id);
            }
            let mut keep = Vec::new();
            let mut rest = Vec::new();
            for id in revealed {
                let def = st
                    .cards
                    .get(&id)
                    .and_then(|c| crate::data::card_def(&c.def));
                let keep_it = def.is_some_and(|d| {
                    d.category == Category::Basic
                        || d.code == "juedou"
                        || d.equip_slot == Some(EquipSlot::Weapon)
                });
                if keep_it {
                    keep.push(id);
                } else {
                    rest.push(id);
                }
            }
            let names: Vec<String> = keep.iter().map(|id| turn::card_label(st, id)).collect();
            for id in &keep {
                crate::resolve::leave_processing(st, id);
                st.seats[actor].hand.push(id.clone());
            }
            for id in &rest {
                crate::resolve::discard_from_processing(st, id);
            }
            st.set_counter("skip_draw", 1);
            st.seats[actor].flags.insert("luoyi_bonus".into(), true);
            st.push_log(
                Some(&st.seats[actor].player_id.clone()),
                format!(
                    "裸衣：亮出牌堆顶 3 张，获得{}；本回合跳过摸牌，直到下回合开始【杀】/【决斗】伤害 +1",
                    if names.is_empty() {
                        "（无基本牌/武器/决斗）".to_string()
                    } else {
                        names.join("、")
                    }
                ),
            );
            pop(st);
            return Ok("界裸衣：跳过摸牌，伤害 +1".to_string());
        }
        st.set_counter("luoyi", 1);
        pop(st);
        return Ok("发动裸衣：本回合【杀】/【决斗】伤害 +1".to_string());
    }
    st.set_counter("luoyi", 0);
    pop(st);
    Ok("不发动裸衣，正常摸牌".to_string())
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
                label: if crate::skills::is_jiexian(st, f.actor) {
                    "发动突袭：少摸等量张数，获得至多两名角色各一张手牌".into()
                } else {
                    "发动突袭：放弃摸牌，获得至多两名角色各一张手牌".into()
                },
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
    // 连营：失去最后的手牌后摸 1 张。
    crate::skills::lianying_on_lose_last(st, target);
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
    if crate::skills::is_jiexian(st, f.actor) {
        // 界突袭：少摸「获得张数」张，仍摸剩余的牌（随后跳过摸牌阶段）。
        let draw = 2u32.saturating_sub(f.targets.len() as u32);
        if draw > 0 {
            let mut ev = Vec::new();
            turn::draw_cards(st, f.actor, draw, &mut ev);
        }
        st.push_log(
            Some(&st.seats[f.actor].player_id.clone()),
            format!(
                "突袭：获得 {} 张，少摸 {} 张",
                f.targets.len(),
                f.targets.len()
            ),
        );
    }
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
        "cixiong" => {
            if f.selected.len() != 1 {
                return Some(Err(illegal("雌雄双股剑需弃置 1 张手牌")));
            }
            let id = f.selected[0].clone();
            if turn::take_card(st, f.actor, &id).is_err() {
                return Some(Err(illegal("手牌中没有该牌")));
            }
            turn::to_discard_card(st, &id);
            let pid = st.seats[f.actor].player_id.clone();
            let label = turn::card_label(st, &id);
            st.push_log(Some(&pid), format!("雌雄双股剑：弃置【{label}】"));
            let user = f.data["user"].as_u64().unwrap_or(0) as usize;
            let target = f.data["target"].as_u64().unwrap_or(0) as usize;
            let card = f.data["card"].as_str().unwrap_or_default().to_string();
            let need = f.data["need"].as_u64().unwrap_or(1) as u32;
            let no_shan = f.data["no_shan"].as_bool().unwrap_or(false);
            let ignore_armor = f.data["ignore_armor"].as_bool().unwrap_or(false);
            pop(st);
            crate::resolve_skill::push_sha_respond(
                st,
                user,
                target,
                &card,
                need,
                no_shan,
                ignore_armor,
            );
            Some(Ok("雌雄双股剑：目标已弃置一张手牌".to_string()))
        }
        "guanshi" => {
            if f.selected.len() != 2 {
                return Some(Err(illegal("贯石斧需弃置 2 张手牌")));
            }
            for id in &f.selected {
                if turn::take_card(st, f.actor, id).is_err() {
                    return Some(Err(illegal("手牌中没有该牌")));
                }
                turn::to_discard_card(st, id);
            }
            let target = f.data["target"].as_u64().unwrap_or(0) as usize;
            let card = f.data["card"].as_str().unwrap_or_default().to_string();
            let source = f.actor;
            let pid = st.seats[source].player_id.clone();
            st.push_log(
                Some(&pid),
                "贯石斧：弃置 2 张手牌，【杀】依然造成伤害".to_string(),
            );
            crate::resolve::apply_damage(
                st,
                target,
                1,
                Some(source),
                crate::resolve::DamageType::Normal,
                Some(&card),
            );
            pop(st);
            Some(Ok("贯石斧：目标受到 1 点伤害".to_string()))
        }
        "ganglie" => {
            if f.selected.len() != 2 {
                return Some(Err(illegal("刚烈需弃置 2 张手牌")));
            }
            for id in &f.selected {
                if turn::take_card(st, f.actor, id).is_err() {
                    return Some(Err(illegal("手牌中没有该牌")));
                }
                turn::to_discard_card(st, id);
            }
            let pid = st.seats[f.actor].player_id.clone();
            st.push_log(Some(&pid), "刚烈：弃置两张手牌".to_string());
            pop(st);
            Some(Ok("刚烈：已弃置 2 张手牌".to_string()))
        }
        "qingnang" => {
            if f.selected.len() != 1 {
                return Some(Err(illegal("青囊需弃置 1 张手牌")));
            }
            replace_top(
                st,
                Frame::Qingnang(QingnangFrame {
                    actor: f.actor,
                    selected: f.selected.clone(),
                }),
            );
            Some(Ok("请选择回复目标".to_string()))
        }
        "jieyin" => {
            let need = if crate::skills::is_jiexian(st, f.actor) {
                1
            } else {
                2
            };
            if f.selected.len() != need {
                return Some(Err(illegal(format!("结姻需弃置 {need} 张手牌"))));
            }
            replace_top(
                st,
                Frame::Jieyin(JieyinFrame {
                    actor: f.actor,
                    selected: f.selected.clone(),
                }),
            );
            Some(Ok("请选择结姻对象".to_string()))
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
        "guhuo" => {
            if f.selected.len() != 1 {
                return Some(Err(illegal("蛊惑需扣置 1 张手牌")));
            }
            let id = f.selected[0].clone();
            if !st.seats[f.actor].hand.iter().any(|c| c == &id) {
                return Some(Err(illegal("手牌中没有该牌")));
            }
            replace_top(
                st,
                Frame::Guhuo(crate::resolve_wind::GuhuoFrame {
                    user: f.actor,
                    purpose: "play".into(),
                    stage: crate::resolve_wind::GuhuoStage::Declare,
                    needed: String::new(),
                    card: Some(id),
                    declared: None,
                    order: Vec::new(),
                    cursor: 0,
                    questioner: None,
                    respond: None,
                }),
            );
            Some(Ok("蛊惑：请声明牌名".to_string()))
        }
        "jushou" => {
            if f.selected.len() != 1 {
                return Some(Err(illegal("据守需弃置 1 张手牌")));
            }
            let id = f.selected[0].clone();
            if turn::take_card(st, f.actor, &id).is_err() {
                return Some(Err(illegal("手牌中没有该牌")));
            }
            let label = turn::card_label(st, &id);
            let pid = st.seats[f.actor].player_id.clone();
            if let Some(slot) = turn::card_def(st, &id).and_then(|d| d.equip_slot) {
                let old = st.seats[f.actor].equips.insert(slot, Some(id.clone()));
                if let Some(Some(old_id)) = old {
                    turn::to_discard_card(st, &old_id);
                    skills::xiaoji_on_lose_equip(st, f.actor);
                }
                st.push_log(Some(&pid), format!("据守：使用装备【{label}】"));
                pop(st);
                let over = st.seats[f.actor]
                    .hand
                    .len()
                    .saturating_sub(st.config.max_hand_cards as usize);
                if over > 0 {
                    crate::resolve::push_discard(st, f.actor, over, "hand_cap");
                }
                return Some(Ok(format!("据守：已使用装备【{label}】")));
            }
            turn::to_discard_card(st, &id);
            st.push_log(Some(&pid), format!("据守：弃置【{label}】"));
            pop(st);
            let over = st.seats[f.actor]
                .hand
                .len()
                .saturating_sub(st.config.max_hand_cards as usize);
            if over > 0 {
                crate::resolve::push_discard(st, f.actor, over, "hand_cap");
            }
            Some(Ok(format!("据守：已弃置【{label}】")))
        }
        "qiaobian" => {
            if f.selected.len() != 1 {
                return Some(Err(illegal("巧变需弃置 1 张手牌")));
            }
            let id = f.selected[0].clone();
            if turn::take_card(st, f.actor, &id).is_err() {
                return Some(Err(illegal("手牌中没有该牌")));
            }
            turn::to_discard_card(st, &id);
            let choice = f.data["choice"].as_str().unwrap_or_default().to_string();
            match choice.as_str() {
                "judge" => {
                    st.set_counter("qiaobian_skip_judge", 1);
                    pop(st);
                    Some(Ok("巧变：跳过判定阶段".to_string()))
                }
                "draw" => {
                    st.set_counter("skip_draw", 1);
                    replace_top(
                        st,
                        Frame::Qiaobian(crate::resolve_mountain::QiaobianFrame {
                            actor: f.actor,
                            remaining: 2,
                        }),
                    );
                    Some(Ok("巧变：请选择获得手牌的角色".to_string()))
                }
                "play" => {
                    st.set_counter("skip_play", 1);
                    replace_top(
                        st,
                        Frame::Jiewei(crate::resolve_wind::JieweiFrame {
                            actor: f.actor,
                            stage: crate::resolve_wind::JieweiStage::Move,
                        }),
                    );
                    Some(Ok("巧变：请选择移动的场牌".to_string()))
                }
                "discard" => {
                    st.set_counter("qiaobian_skip_discard", 1);
                    pop(st);
                    Some(Ok("巧变：跳过弃牌阶段".to_string()))
                }
                _ => Some(Err(illegal("未知巧变选项"))),
            }
        }
        "xiangle" => {
            if f.selected.len() != 1 {
                return Some(Err(illegal("享乐需弃置 1 张基本牌")));
            }
            let id = f.selected[0].clone();
            if turn::take_card(st, f.actor, &id).is_err() {
                return Some(Err(illegal("手牌中没有该牌")));
            }
            turn::to_discard_card(st, &id);
            let user = f.data["user"].as_u64().unwrap_or(f.actor as u64) as usize;
            let target = f.data["target"].as_u64().unwrap_or(0) as usize;
            let card = f.data["card"].as_str().unwrap_or_default().to_string();
            let need = f.data["need"].as_u64().unwrap_or(1) as u32;
            let no_shan = f.data["no_shan"].as_bool().unwrap_or(false);
            let ignore_armor = f.data["ignore_armor"].as_bool().unwrap_or(false);
            let damage_type: crate::resolve::DamageType =
                serde_json::from_value(f.data["damage_type"].clone()).unwrap_or_default();
            pop(st);
            crate::resolve_skill::push_sha_target_response(
                st,
                user,
                target,
                &card,
                need,
                no_shan,
                ignore_armor,
                damage_type,
            );
            Some(Ok("享乐：已弃置基本牌，继续结算".to_string()))
        }
        "haoshi" => {
            if f.selected.is_empty() {
                return Some(Err(illegal("好施需选择要交出的牌")));
            }
            replace_top(
                st,
                Frame::Haoshi(crate::resolve_forest::HaoshiFrame {
                    actor: f.actor,
                    stage: crate::resolve_forest::HaoshiStage::Target,
                    selected: f.selected.clone(),
                }),
            );
            Some(Ok("好施：请选择手牌最少的其他角色".to_string()))
        }
        "dimeng" => {
            let first = f.data["first"].as_u64().unwrap_or(0) as usize;
            let second = f.data["second"].as_u64().unwrap_or(0) as usize;
            for id in &f.selected {
                if turn::take_card(st, f.actor, id).is_err() {
                    return Some(Err(illegal("手牌中没有该牌")));
                }
                turn::to_discard_card(st, id);
            }
            let a = std::mem::take(&mut st.seats[first].hand);
            let b = std::mem::take(&mut st.seats[second].hand);
            st.seats[first].hand = b;
            st.seats[second].hand = a;
            st.push_log(
                Some(&st.seats[f.actor].player_id.clone()),
                format!(
                    "缔盟：交换 {} 与 {} 的手牌",
                    st.seats[first].general_name, st.seats[second].general_name
                ),
            );
            pop(st);
            Some(Ok("缔盟：已交换手牌".to_string()))
        }
        "jinzou" => {
            let target = f.data["target"].as_u64().unwrap_or(0) as usize;
            let n = f.selected.len();
            for id in &f.selected {
                if turn::take_card(st, f.actor, id).is_err() {
                    return Some(Err(illegal("手牌中没有该牌")));
                }
                turn::to_discard_card(st, id);
            }
            let left = st.seats[f.actor].hand.len();
            pop(st);
            let pid = st.seats[f.actor].player_id.clone();
            st.push_log(
                Some(&pid),
                format!("进趋：弃置 {n} 张，手牌剩 {left} 张（目标 {target} 张）"),
            );
            Some(Ok(format!("进趋：已弃置 {n} 张")))
        }
        "feijun" => {
            if f.selected.len() != 1 {
                return Some(Err(illegal("飞军需弃置 1 张牌")));
            }
            let id = f.selected[0].clone();
            if turn::take_card(st, f.actor, &id).is_err() {
                return Some(Err(illegal("手牌中没有该牌")));
            }
            let label = turn::card_label(st, &id);
            turn::to_discard_card(st, &id);
            let pid = st.seats[f.actor].player_id.clone();
            st.push_log(Some(&pid), format!("飞军：弃置【{label}】"));
            let option = f.data["option"].as_u64().unwrap_or(1) as u8;
            let target = f.data["target"].as_u64().unwrap_or(0) as usize;
            pop(st);
            match crate::resolve_sp::feijun_done(st, f.actor, target, option) {
                Ok(msg) => Some(Ok(msg)),
                Err(e) => Some(Err(e)),
            }
        }
        "chenglue" => {
            let need = if f.data["yin"].as_bool().unwrap_or(false) {
                1
            } else {
                2
            };
            if f.selected.len() != need {
                return Some(Err(illegal("成略弃置张数不符")));
            }
            let mut suits = Vec::new();
            for id in &f.selected {
                if turn::take_card(st, f.actor, id).is_err() {
                    return Some(Err(illegal("手牌中没有该牌")));
                }
                if let Some(inst) = st.cards.get(id) {
                    suits.push(inst.suit);
                }
                turn::to_discard_card(st, id);
            }
            for suit in suits {
                let key = match suit {
                    Suit::Spade => "cl_spade",
                    Suit::Heart => "cl_heart",
                    Suit::Club => "cl_club",
                    Suit::Diamond => "cl_diamond",
                };
                st.set_counter(key, 1);
            }
            skills::toggle_yj(st, f.actor, "chenglue");
            let pid = st.seats[f.actor].player_id.clone();
            st.push_log(
                Some(&pid),
                format!(
                    "成略：弃置 {} 张，本阶段同花色牌无距离与次数限制",
                    f.selected.len()
                ),
            );
            pop(st);
            Some(Ok("成略：已完成".to_string()))
        }
        "mingce" => match crate::resolve_yijiang::mingce_confirm(st, f) {
            Ok(msg) => Some(Ok(msg)),
            Err(e) => Some(Err(e)),
        },
        "sanyao" => match crate::resolve_yijiang::sanyao_confirm(st, f) {
            Ok(msg) => Some(Ok(msg)),
            Err(e) => Some(Err(e)),
        },
        "mingren_init" | "mingren" => {
            if f.selected.len() != 1 {
                if f.purpose == "mingren_init" {
                    pop(st);
                    return Some(Ok("明任：未置「任」".to_string()));
                }
                return Some(Err(illegal("明任需选择 1 张手牌")));
            }
            let card = f.selected[0].clone();
            if f.purpose == "mingren" {
                return Some(crate::resolve_sp2::mingren_swap(st, f.actor, &card));
            }
            if turn::take_card(st, f.actor, &card).is_err() {
                return Some(Err(illegal("手牌中没有该牌")));
            }
            skills::add_mark(st, f.actor, "ren", &card);
            pop(st);
            Some(Ok("明任：已置「任」".to_string()))
        }
        "hongju" => {
            let cards = f.selected.clone();
            pop(st);
            Some(crate::resolve_sp2::hongju_swap(st, f.actor, &cards))
        }
        "kongsheng" => {
            for id in &f.selected {
                if turn::take_card(st, f.actor, id).is_err() {
                    return Some(Err(illegal("手牌中没有该牌")));
                }
                skills::add_mark(st, f.actor, "kong", id);
            }
            pop(st);
            Some(Ok(format!("箜声：置于武将牌上 {} 张", f.selected.len())))
        }
        "congjian" => match crate::resolve_sp2::congjian_confirm(st, f) {
            Ok(msg) => Some(Ok(msg)),
            Err(e) => Some(Err(e)),
        },
        "shenshi_yang" => match crate::resolve_sp::shenshi_yang_confirm(st, f) {
            Ok(msg) => Some(Ok(msg)),
            Err(e) => Some(Err(e)),
        },
        "shenshi_yin" => match crate::resolve_sp::shenshi_yin_confirm(st, f) {
            Ok(msg) => Some(Ok(msg)),
            Err(e) => Some(Err(e)),
        },
        "tianxiang" => {
            if f.selected.len() != 1 {
                return Some(Err(illegal("天香需弃置 1 张红桃手牌")));
            }
            let id = f.selected[0].clone();
            if skills::effective_suit(st, f.actor, &id) != Some(Suit::Heart) {
                return Some(Err(illegal("天香只能弃置红桃手牌")));
            }
            if turn::take_card(st, f.actor, &id).is_err() {
                return Some(Err(illegal("手牌中没有该牌")));
            }
            turn::to_discard_card(st, &id);
            let victim = f.data["victim"].as_u64().unwrap_or(f.actor as u64) as usize;
            let amount = f.data["amount"].as_i64().unwrap_or(1) as i32;
            let source = f.data["source"].as_u64().map(|v| v as usize);
            let damage_type: crate::resolve::DamageType =
                serde_json::from_value(f.data["damage_type"].clone()).unwrap_or_default();
            let card = f.data["card"].as_str().map(str::to_string);
            st.push_log(
                Some(&st.seats[f.actor].player_id.clone()),
                format!("天香：弃置【{}】防止伤害", turn::card_label(st, &id)),
            );
            replace_top(
                st,
                Frame::Tianxiang(crate::resolve_wind::TianxiangFrame {
                    victim,
                    amount,
                    source,
                    damage_type,
                    card,
                    stage: crate::resolve_wind::TianxiangStage::Target,
                    target: None,
                    discarded: Some(id),
                }),
            );
            Some(Ok("天香：请选择转移目标".to_string()))
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
        Some(Frame::Qingnang(_)) => qingnang_target(st, pending, action).map(Some),
        Some(Frame::Jieyin(_)) => jieyin_target(st, pending, action).map(Some),
        Some(Frame::Yiji(_)) => yiji_target(st, pending, action).map(Some),
        Some(Frame::Fanjian(_)) => fanjian_target(st, pending, action).map(Some),
        Some(Frame::Liuli(_)) => liuli_target(st, pending, action).map(Some),
        Some(Frame::Leiji(_)) => crate::resolve_wind::leiji_target(st, pending, action).map(Some),
        Some(Frame::Tianxiang(_)) => {
            crate::resolve_wind::tianxiang_target(st, pending, action).map(Some)
        }
        Some(Frame::Jiewei(_)) => crate::resolve_wind::jiewei_target(st, pending, action).map(Some),
        Some(Frame::Qiangxi(_)) => {
            crate::resolve_fire::qiangxi_target(st, pending, action).map(Some)
        }
        Some(Frame::Quhu(_)) => crate::resolve_fire::quhu_target(st, pending, action).map(Some),
        Some(Frame::Jieming(_)) => {
            crate::resolve_fire::jieming_target(st, pending, action).map(Some)
        }
        Some(Frame::Tianyi(_)) => crate::resolve_fire::tianyi_target(st, pending, action).map(Some),
        Some(Frame::Fangzhu(_)) => {
            crate::resolve_forest::fangzhu_target(st, pending, action).map(Some)
        }
        Some(Frame::Yinghun(_)) => {
            crate::resolve_forest::yinghun_target(st, pending, action).map(Some)
        }
        Some(Frame::Haoshi(_)) => {
            crate::resolve_forest::haoshi_target(st, pending, action).map(Some)
        }
        Some(Frame::Dimeng(_)) => {
            crate::resolve_forest::dimeng_target(st, pending, action).map(Some)
        }
        Some(Frame::Qiaobian(_)) => {
            crate::resolve_mountain::qiaobian_gain_target(st, pending, action).map(Some)
        }
        Some(Frame::Tiaoxin(_)) => {
            crate::resolve_mountain::tiaoxin_target(st, pending, action).map(Some)
        }
        Some(Frame::Fangquan(_)) => {
            crate::resolve_mountain::fangquan_target(st, pending, action).map(Some)
        }
        Some(Frame::Zhijian(_)) => {
            crate::resolve_mountain::zhijian_target(st, pending, action).map(Some)
        }
        Some(Frame::Qizhi(_)) => crate::resolve_sp::qizhi_target(st, pending, action).map(Some),
        Some(Frame::Feijun(_)) => crate::resolve_sp::feijun_target(st, pending, action).map(Some),
        Some(Frame::Yili(_)) => crate::resolve_sp::yili_target(st, pending, action).map(Some),
        Some(Frame::Shenshi(_)) => crate::resolve_sp::shenshi_target(st, pending, action).map(Some),
        Some(Frame::Chezheng(_)) => {
            crate::resolve_sp::chezheng_target(st, pending, action).map(Some)
        }
        Some(Frame::Kuizhu(_)) => crate::resolve_sp::kuizhu_target(st, pending, action),
        Some(Frame::ZhenLiang(_)) => {
            crate::resolve_sp2::zhenliang_target(st, pending, action).map(Some)
        }
        Some(Frame::ZhenLiangYin(_)) => {
            crate::resolve_sp2::zhenliang_yin_target(st, pending, action).map(Some)
        }
        Some(Frame::ZhenGu(_)) => crate::resolve_sp2::zhengu_target(st, pending, action).map(Some),
        Some(Frame::ZuiLun(_)) => {
            crate::resolve_sp2::zuilun_zero_target(st, pending, action).map(Some)
        }
        Some(Frame::LiangYin(_)) => {
            crate::resolve_sp2::liangyin_target(st, pending, action).map(Some)
        }
        Some(Frame::KongSheng(_)) => {
            crate::resolve_sp2::kongsheng_target(st, pending, action).map(Some)
        }
        Some(Frame::ZhengRong(_)) => {
            crate::resolve_sp2::zhengrong_target(st, pending, action).map(Some)
        }
        Some(Frame::XiongLuan(_)) => {
            crate::resolve_sp2::xiongluan_target(st, pending, action).map(Some)
        }
        Some(Frame::CongJian(_)) => {
            crate::resolve_sp2::congjian_target(st, pending, action).map(Some)
        }
        Some(Frame::XianZhen(_)) => {
            crate::resolve_yijiang::xianzhen_target(st, pending, action).map(Some)
        }
        Some(Frame::MingCe(_)) => {
            crate::resolve_yijiang::mingce_target(st, pending, action).map(Some)
        }
        Some(Frame::XuanFeng(_)) => {
            crate::resolve_yijiang::xuanfeng_target(st, pending, action).map(Some)
        }
        Some(Frame::SanYao(_)) => {
            crate::resolve_yijiang::sanyao_target(st, pending, action).map(Some)
        }
        Some(Frame::GanLu(_)) => {
            crate::resolve_yijiang::ganlu_target(st, pending, action).map(Some)
        }
        Some(Frame::ZhenJun(_)) => {
            crate::resolve_yijiang::zhenjun_target(st, pending, action).map(Some)
        }
        _ => Ok(None),
    }
}
