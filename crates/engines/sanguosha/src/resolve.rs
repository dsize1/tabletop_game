//! 结算栈：锦囊 / 无懈链 / 响应 / 濒死 / 弃牌 / 判定（P2.3）。
//!
//! 设计（`docs/games/sanguosha/rules-rulings.md` §4、`state-machine.md` §2.2）：
//! - `resolution: Vec<Frame>` 是唯一结算栈；`pending` 是栈顶等待输入的投影。
//! - 无懈链是栈上的可反复嵌套窗口：询问**所有存活玩家**（不跳过无手牌者）；
//!   每次有人打出无懈 → 层数 +1 并从原使用者左手侧重新开始；层数奇偶决定效果是否被抵消。
//! - AOE 逐目标结算，每个目标各自开一次无懈窗口。
//! - 子帧结束后通过 `state.outcome` 把结果交回父帧（同一时刻只有一个子帧完成）。

use crate::data;
use crate::state::*;
use crate::turn;
use crate::{illegal, internal};
use game_engine_core::{EngineError, EngineEvent};
use serde::{Deserialize, Serialize};
use serde_json::json;

// ---------------------------------------------------------------------------
// 帧定义
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Frame {
    Trick(TrickFrame),
    Sha(ShaFrame),
    Wuxie(WuxieFrame),
    PickCard(PickCardFrame),
    Respond(RespondFrame),
    Dying(DyingFrame),
    Discard(DiscardFrame),
    Judge(JudgeFrame),
    /// 奸雄：受伤后选择是否获得造成伤害的牌。
    Jianxiong(JianxiongFrame),
    /// 通用多选（制衡等技能）。
    MultiSelect(MultiSelectFrame),
    Requisition(crate::resolve_more::RequisitionFrame),
    Rende(crate::resolve_more::RendeFrame),
    Lijian(crate::resolve_more::LijianFrame),
    TuxiOffer(crate::resolve_more::TuxiOfferFrame),
    Tuxi(crate::resolve_more::TuxiFrame),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JianxiongFrame {
    pub actor: usize,
    pub card: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiSelectFrame {
    pub actor: usize,
    /// 技能/用途标识（当前：zhiheng）。
    pub purpose: String,
    pub selected: Vec<String>,
    pub min: usize,
    pub max: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrickStage {
    Target,
    Wuxie,
    WaitWuxie,
    Effect,
    WaitChild,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrickFrame {
    pub card: String,
    pub code: String,
    pub user: usize,
    pub targets: Vec<usize>,
    pub idx: usize,
    pub stage: TrickStage,
    /// 借刀杀人：第一目标（有武器者）/ 第二目标（杀目标）。
    #[serde(default)]
    pub first: Option<usize>,
    #[serde(default)]
    pub second: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShaStage {
    Target,
    WaitRespond,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShaFrame {
    pub card: String,
    pub user: usize,
    #[serde(default)]
    pub target: Option<usize>,
    pub stage: ShaStage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "root", rename_all = "snake_case")]
pub enum WuxieRoot {
    Trick {
        card: String,
        user: usize,
        target: usize,
    },
    Judge {
        delayed: String,
        owner: usize,
    },
}

impl WuxieRoot {
    fn summary(&self, st: &SgsState) -> String {
        match self {
            WuxieRoot::Trick { card, target, .. } => format!(
                "【{}】→ {}",
                turn::card_label(st, card),
                st.seats[*target].general_name
            ),
            WuxieRoot::Judge { delayed, owner } => format!(
                "{} 判定【{}】",
                st.seats[*owner].general_name,
                turn::card_label(st, delayed)
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WuxieFrame {
    pub root: WuxieRoot,
    /// 已打出的无懈层数（奇偶决定是否抵消）。
    pub count: u32,
    /// 询问顺序（openid，全部存活玩家；从原使用者左手侧开始，原使用者最后）。
    pub order: Vec<String>,
    pub cursor: usize,
    /// 自上次无懈以来的连续 pass 数。
    pub passed: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PickMode {
    Shunshou,
    Guohe,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PickCardFrame {
    pub card: String,
    pub user: usize,
    pub target: usize,
    pub mode: PickMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RespondKind {
    /// 需要【闪】（杀 / 万箭 / 借刀中的杀）。
    Shan,
    /// 南蛮：需要【杀】。
    Nanman,
    /// 决斗：需要【杀】，双方交替。
    Duel,
    /// 借刀：A 需对 B 使用【杀】。
    Jiedao,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DamageType {
    Normal,
    Thunder,
    Fire,
}

impl DamageType {
    fn label(self) -> &'static str {
        match self {
            DamageType::Normal => "普通",
            DamageType::Thunder => "雷电",
            DamageType::Fire => "火焰",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RespondFrame {
    pub kind: RespondKind,
    /// 需要响应的玩家。
    pub actor: usize,
    /// 正在结算的牌（杀/AOE/决斗/借刀；借刀被使用的杀也记在这里）。
    pub card: String,
    /// 伤害来源。
    pub source: usize,
    /// 对手/杀目标（决斗的另一方、借刀的 B、杀的目标）。
    #[serde(default)]
    pub target: Option<usize>,
    pub damage: i32,
    pub damage_type: DamageType,
    /// 本帧是否负责把 `card` 放入弃牌堆（杀/AOE 由外层帧负责；借刀里被使用的杀由本帧负责）。
    #[serde(default)]
    pub consume_card: bool,
    /// 青釭剑：无视目标防具（八卦阵不可用、仁王盾无效）。
    #[serde(default)]
    pub ignore_armor: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DyingFrame {
    pub victim: usize,
    /// 伤害来源（死亡奖惩）。
    #[serde(default)]
    pub source: Option<usize>,
    /// 询问顺序（全部存活玩家，openid）。
    pub order: Vec<String>,
    pub cursor: usize,
    /// 已明确放弃的玩家（本次濒死内）。
    pub passed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscardFrame {
    pub actor: usize,
    pub required: usize,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JudgeStage {
    Wuxie,
    WaitWuxie,
    Reveal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JudgeFrame {
    pub delayed: String,
    pub owner: usize,
    pub stage: JudgeStage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Outcome {
    Wuxie {
        cancelled: bool,
    },
    Respond {
        played: Option<String>,
        #[serde(default)]
        played_by: Option<usize>,
    },
    Picked {
        card: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// 工具
// ---------------------------------------------------------------------------

fn card_name(st: &SgsState, id: &str) -> String {
    st.cards
        .get(id)
        .and_then(|c| data::card_def(&c.def))
        .map(|d| d.name.to_string())
        .unwrap_or_default()
}

fn card_code(st: &SgsState, id: &str) -> String {
    st.cards.get(id).map(|c| c.def.clone()).unwrap_or_default()
}

fn to_discard(st: &mut SgsState, id: &str) {
    st.deck.discard.push(id.to_string());
}

/// 牌仍在处理区时才弃置（被奸雄等拿走的不再重复弃置）。
pub(crate) fn discard_from_processing(st: &mut SgsState, id: &str) {
    if st.deck.processing.iter().any(|c| c == id) {
        leave_processing(st, id);
        to_discard(st, id);
    }
}

pub(crate) fn leave_processing(st: &mut SgsState, id: &str) {
    if let Some(pos) = st.deck.processing.iter().position(|c| c == id) {
        st.deck.processing.remove(pos);
    }
}

fn wuxie_order(st: &SgsState, reference: usize) -> Vec<String> {
    let mut order: Vec<String> = st
        .seats_from_left_of(reference)
        .into_iter()
        .filter(|i| st.seats[*i].alive)
        .map(|i| st.seats[i].player_id.clone())
        .collect();
    if st.seats.get(reference).is_some_and(|s| s.alive) {
        order.push(st.seats[reference].player_id.clone());
    }
    order
}

fn alive_of_order(st: &SgsState, pid: &str) -> bool {
    st.seat(pid).is_some_and(|s| s.alive)
}

fn pending_pass() -> PendingOption {
    PendingOption {
        id: "pass".into(),
        label: "不出".into(),
    }
}

pub(crate) fn replace_top(st: &mut SgsState, frame: Frame) {
    if let Some(top) = st.resolution.last_mut() {
        *top = frame;
    }
}

/// 更新「子帧之下的父帧」（子帧已在栈顶时使用）。
fn update_parent(st: &mut SgsState, frame: Frame) {
    let n = st.resolution.len();
    if n >= 2 {
        st.resolution[n - 2] = frame;
    } else if n == 1 {
        st.resolution[0] = frame;
    }
}

pub(crate) fn push(st: &mut SgsState, frame: Frame) {
    st.resolution.push(frame);
}

pub(crate) fn pop(st: &mut SgsState) -> Option<Frame> {
    st.resolution.pop()
}

pub(crate) fn top(st: &SgsState) -> Option<&Frame> {
    st.resolution.last()
}

pub fn push_discard(st: &mut SgsState, actor: usize, required: usize, reason: &str) {
    if required > 0 {
        push(
            st,
            Frame::Discard(DiscardFrame {
                actor,
                required,
                reason: reason.to_string(),
            }),
        );
    }
}

pub fn push_judge(st: &mut SgsState, delayed: String, owner: usize) {
    push(
        st,
        Frame::Judge(JudgeFrame {
            delayed,
            owner,
            stage: JudgeStage::Wuxie,
        }),
    );
}

// ---------------------------------------------------------------------------
// 主循环
// ---------------------------------------------------------------------------

/// 推进结算栈 + 阶段机，直到需要玩家输入或终局。
pub fn advance(st: &mut SgsState, events: &mut Vec<EngineEvent>) -> Result<(), EngineError> {
    let mut guard = 0;
    loop {
        guard += 1;
        if guard > 4000 {
            return Err(internal("结算栈推进次数异常（疑似死循环）"));
        }
        if st.phase == Phase::Finished {
            st.pending = None;
            st.resolution.clear();
            return Ok(());
        }
        if st.pending.is_some() {
            return Ok(());
        }
        if top(st).is_some() {
            let frame = top(st).cloned().expect("frame");
            let waiting = step(st, frame, events)?;
            if waiting {
                return Ok(());
            }
            continue;
        }
        if !turn::phase_step(st, events)? {
            return Ok(());
        }
    }
}

fn step(
    st: &mut SgsState,
    frame: Frame,
    events: &mut Vec<EngineEvent>,
) -> Result<bool, EngineError> {
    match frame {
        Frame::Wuxie(f) => step_wuxie(st, f),
        Frame::Trick(f) => step_trick(st, f, events),
        Frame::Sha(f) => step_sha(st, f),
        Frame::PickCard(f) => step_pick(st, f),
        Frame::Respond(f) => step_respond(st, f),
        Frame::Dying(f) => step_dying(st, f, events),
        Frame::Discard(f) => step_discard(st, f),
        Frame::Judge(f) => step_judge(st, f, events),
        Frame::Jianxiong(f) => crate::resolve_skill::step_jianxiong(st, f),
        Frame::MultiSelect(f) => crate::resolve_skill::step_multiselect(st, f),
        Frame::Requisition(f) => crate::resolve_more::step_requisition(st, f),
        Frame::Rende(f) => crate::resolve_more::step_rende(st, f),
        Frame::Lijian(f) => crate::resolve_more::step_lijian(st, f),
        Frame::TuxiOffer(f) => crate::resolve_more::step_tuxi_offer(st, f),
        Frame::Tuxi(f) => crate::resolve_more::step_tuxi(st, f),
    }
}

// --- 无懈 -----------------------------------------------------------------

fn step_wuxie(st: &mut SgsState, mut f: WuxieFrame) -> Result<bool, EngineError> {
    if f.passed as usize >= f.order.len() {
        let cancelled = f.count % 2 == 1;
        st.push_log(
            None,
            if cancelled {
                "无懈可击：效果被抵消"
            } else {
                "无懈可击：效果生效"
            },
        );
        st.outcome = Some(Outcome::Wuxie { cancelled });
        pop(st);
        return Ok(false);
    }
    let pid = f.order[f.cursor % f.order.len()].clone();
    if !alive_of_order(st, &pid) {
        f.cursor += 1;
        f.passed += 1;
        replace_top(st, Frame::Wuxie(f));
        return Ok(false);
    }
    let seat = st
        .seat_index(&pid)
        .ok_or_else(|| internal("无懈询问对象不在座"))?;
    let mut options: Vec<PendingOption> = turn::hand_cards_with_def(st, seat, "wuxie_keji")
        .iter()
        .map(|id| PendingOption {
            id: format!("respond:{id}"),
            label: format!("打出【{}】", turn::card_label(st, id)),
        })
        .collect();
    options.push(pending_pass());
    st.pending = Some(Pending {
        kind: "wuxie".into(),
        actor: pid,
        skill: None,
        options,
        data: json!({
            "count": f.count,
            "layers": f.count,
            "root": f.root.summary(st),
        }),
    });
    replace_top(st, Frame::Wuxie(f));
    Ok(true)
}

// --- 杀（基本牌）----------------------------------------------------------

fn step_sha(st: &mut SgsState, f: ShaFrame) -> Result<bool, EngineError> {
    match f.stage {
        ShaStage::Target => {
            let targets = turn::sha_targets(st, f.user);
            let options: Vec<PendingOption> = targets
                .iter()
                .map(|t| PendingOption {
                    id: format!("target:{}", st.seats[*t].player_id),
                    label: format!("对 {} 使用【杀】", st.seats[*t].general_name),
                })
                .collect();
            if options.is_empty() {
                discard_from_processing(st, &f.card);
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.user].player_id.clone(),
                skill: None,
                options,
                data: json!({ "card": f.card, "source": f.user, "type": "sha" }),
            });
            replace_top(st, Frame::Sha(f));
            Ok(true)
        }
        ShaStage::WaitRespond => match st.outcome.take() {
            Some(Outcome::Respond { .. }) => {
                discard_from_processing(st, &f.card);
                if let Some(target) = f.target {
                    st.push_log(
                        None,
                        format!("【杀】对 {} 结算完毕", st.seats[target].general_name),
                    );
                }
                pop(st);
                Ok(false)
            }
            _ => Err(internal("缺少【闪】结算结果")),
        },
    }
}

pub fn start_sha(st: &mut SgsState, user: usize, card: &str) -> Result<(), EngineError> {
    st.deck.processing.push(card.to_string());
    st.bump_counter("sha_used");
    push(
        st,
        Frame::Sha(ShaFrame {
            card: card.to_string(),
            user,
            target: None,
            stage: ShaStage::Target,
        }),
    );
    Ok(())
}

// --- 锦囊 -----------------------------------------------------------------

fn needs_wuxie(code: &str) -> bool {
    !matches!(code, "lebu_sishu" | "shandian")
}

fn trick_targets(st: &SgsState, code: &str, user: usize) -> Vec<usize> {
    match code {
        "nanman_ruqin" | "wanjian_qifa" => st
            .seats_from_left_of(user)
            .into_iter()
            .filter(|i| st.seats[*i].alive)
            .collect(),
        "taoyuan_jieyi" => {
            let mut v = vec![user];
            v.extend(
                st.seats_from_left_of(user)
                    .into_iter()
                    .filter(|i| st.seats[*i].alive && *i != user),
            );
            v
        }
        _ => Vec::new(),
    }
}

/// 开始一张锦囊（调用前牌已从手牌移入 `processing`）。
pub fn start_trick(st: &mut SgsState, user: usize, card: &str) -> Result<(), EngineError> {
    start_trick_code(st, user, card, None)
}

/// 转化技（如奇袭）：用原牌但按指定锦囊结算。
pub fn start_virtual_trick(
    st: &mut SgsState,
    user: usize,
    card: &str,
    code: &str,
) -> Result<(), EngineError> {
    start_trick_code(st, user, card, Some(code))
}

fn start_trick_code(
    st: &mut SgsState,
    user: usize,
    card: &str,
    override_code: Option<&str>,
) -> Result<(), EngineError> {
    let code = override_code
        .map(str::to_string)
        .unwrap_or_else(|| card_code(st, card));
    let mut targets = trick_targets(st, &code, user);
    if code == "wuzhong_shengyou" {
        targets = vec![user];
    }
    let stage = if targets.is_empty() {
        TrickStage::Target
    } else if needs_wuxie(&code) {
        TrickStage::Wuxie
    } else {
        TrickStage::Effect
    };
    push(
        st,
        Frame::Trick(TrickFrame {
            card: card.to_string(),
            code,
            user,
            targets,
            idx: 0,
            stage,
            first: None,
            second: None,
        }),
    );
    Ok(())
}

fn trick_done(st: &mut SgsState, f: &TrickFrame) {
    if !matches!(f.code.as_str(), "lebu_sishu" | "shandian") {
        discard_from_processing(st, &f.card);
    }
    st.push_log(None, format!("【{}】结算完毕", card_name(st, &f.card)));
    pop(st);
}

fn advance_trick_target(st: &mut SgsState, mut f: TrickFrame) -> Result<(), EngineError> {
    f.idx += 1;
    f.stage = TrickStage::Wuxie;
    if f.idx >= f.targets.len() {
        trick_done(st, &f);
        return Ok(());
    }
    replace_top(st, Frame::Trick(f));
    Ok(())
}

fn step_trick(
    st: &mut SgsState,
    mut f: TrickFrame,
    events: &mut Vec<EngineEvent>,
) -> Result<bool, EngineError> {
    match f.stage {
        TrickStage::Target => {
            let user_pid = st.seats[f.user].player_id.clone();
            let options: Vec<PendingOption> = if f.code == "jiedao_sharen" {
                if f.first.is_none() {
                    st.alive_indices()
                        .into_iter()
                        .filter(|i| *i != f.user && st.equip_def(*i, EquipSlot::Weapon).is_some())
                        .map(|i| PendingOption {
                            id: format!("target:{}", st.seats[i].player_id),
                            label: format!("借 {} 的刀", st.seats[i].general_name),
                        })
                        .collect()
                } else if f.second.is_none() {
                    let first = f.first.expect("first");
                    st.alive_indices()
                        .into_iter()
                        .filter(|i| *i != first && st.in_attack_range(first, *i))
                        .map(|i| PendingOption {
                            id: format!("target:{}", st.seats[i].player_id),
                            label: format!("令其杀 {}", st.seats[i].general_name),
                        })
                        .collect()
                } else {
                    vec![
                        PendingOption {
                            id: "confirm".into(),
                            label: "确认（开始结算）".into(),
                        },
                        PendingOption {
                            id: "cancel".into(),
                            label: "取消选择".into(),
                        },
                    ]
                }
            } else {
                let candidates: Vec<usize> = match f.code.as_str() {
                    "shunshou_qianyang" => st
                        .alive_indices()
                        .into_iter()
                        .filter(|i| *i != f.user && turn::trick_distance_ok(st, f.user, *i))
                        .collect(),
                    "lebu_sishu" => st
                        .alive_indices()
                        .into_iter()
                        .filter(|i| *i != f.user && !turn::judge_zone_has(st, *i, "lebu_sishu"))
                        .collect(),
                    "wuzhong_shengyou" => vec![f.user],
                    _ => st
                        .alive_indices()
                        .into_iter()
                        .filter(|i| *i != f.user)
                        .collect(),
                };
                candidates
                    .into_iter()
                    .map(|i| PendingOption {
                        id: format!("target:{}", st.seats[i].player_id),
                        label: format!("选择 {}", st.seats[i].general_name),
                    })
                    .collect()
            };
            if options.is_empty() {
                discard_from_processing(st, &f.card);
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: user_pid,
                skill: None,
                options,
                data: json!({
                    "card": f.card,
                    "code": f.code,
                    "stage": if f.code == "jiedao_sharen" {
                        if f.first.is_none() {
                            "borrow_first"
                        } else if f.second.is_none() {
                            "borrow_second"
                        } else {
                            "confirm"
                        }
                    } else {
                        "single"
                    },
                }),
            });
            replace_top(st, Frame::Trick(f));
            Ok(true)
        }
        TrickStage::Wuxie => {
            if f.targets.is_empty() {
                f.stage = TrickStage::Effect;
                replace_top(st, Frame::Trick(f));
                return Ok(false);
            }
            let target = f.targets[f.idx];
            if !st.seats[target].alive {
                advance_trick_target(st, f)?;
                return Ok(false);
            }
            // 桃园结义：满血角色直接跳过（不询问无懈）
            if f.code == "taoyuan_jieyi" && st.seats[target].hp >= st.seats[target].max_hp {
                advance_trick_target(st, f)?;
                return Ok(false);
            }
            if !needs_wuxie(&f.code) {
                f.stage = TrickStage::Effect;
                replace_top(st, Frame::Trick(f));
                return Ok(false);
            }
            let order = wuxie_order(st, f.user);
            f.stage = TrickStage::WaitWuxie;
            replace_top(st, Frame::Trick(f.clone()));
            push(
                st,
                Frame::Wuxie(WuxieFrame {
                    root: WuxieRoot::Trick {
                        card: f.card.clone(),
                        user: f.user,
                        target,
                    },
                    count: 0,
                    order,
                    cursor: 0,
                    passed: 0,
                }),
            );
            Ok(false)
        }
        TrickStage::WaitWuxie => match st.outcome.take() {
            Some(Outcome::Wuxie { cancelled }) => {
                if cancelled {
                    st.push_log(
                        None,
                        format!("【{}】被无懈可击抵消", card_name(st, &f.card)),
                    );
                    advance_trick_target(st, f)?;
                } else {
                    f.stage = TrickStage::Effect;
                    replace_top(st, Frame::Trick(f));
                }
                Ok(false)
            }
            _ => Err(internal("缺少无懈结算结果")),
        },
        TrickStage::Effect => {
            let target = if f.code == "jiedao_sharen" {
                f.first.unwrap_or(f.user)
            } else if f.targets.is_empty() {
                f.user
            } else {
                f.targets[f.idx]
            };
            match f.code.as_str() {
                "nanman_ruqin" => {
                    push(
                        st,
                        Frame::Respond(RespondFrame {
                            kind: RespondKind::Nanman,
                            actor: target,
                            card: f.card.clone(),
                            source: f.user,
                            target: Some(target),
                            damage: 1,
                            damage_type: DamageType::Normal,
                            consume_card: false,
                            ignore_armor: false,
                        }),
                    );
                    f.stage = TrickStage::WaitChild;
                    update_parent(st, Frame::Trick(f));
                }
                "wanjian_qifa" => {
                    push(
                        st,
                        Frame::Respond(RespondFrame {
                            kind: RespondKind::Shan,
                            actor: target,
                            card: f.card.clone(),
                            source: f.user,
                            target: Some(target),
                            damage: 1,
                            damage_type: DamageType::Normal,
                            consume_card: false,
                            ignore_armor: false,
                        }),
                    );
                    f.stage = TrickStage::WaitChild;
                    update_parent(st, Frame::Trick(f));
                }
                "taoyuan_jieyi" => {
                    if st.seats[target].hp < st.seats[target].max_hp {
                        st.seats[target].hp += 1;
                        st.push_log(
                            None,
                            format!("{} 回复 1 点体力", st.seats[target].general_name),
                        );
                    }
                    advance_trick_target(st, f)?;
                }
                "wuzhong_shengyou" => {
                    let drawn = turn::draw_cards(st, target, 2, events);
                    st.push_log(
                        None,
                        format!("{} 摸 {drawn} 张", st.seats[target].general_name),
                    );
                    advance_trick_target(st, f)?;
                }
                "shunshou_qianyang" | "guohe_chaiqiao" => {
                    push(
                        st,
                        Frame::PickCard(PickCardFrame {
                            card: f.card.clone(),
                            user: f.user,
                            target,
                            mode: if f.code == "shunshou_qianyang" {
                                PickMode::Shunshou
                            } else {
                                PickMode::Guohe
                            },
                        }),
                    );
                    f.stage = TrickStage::WaitChild;
                    update_parent(st, Frame::Trick(f));
                }
                "juedou" => {
                    push(
                        st,
                        Frame::Respond(RespondFrame {
                            kind: RespondKind::Duel,
                            actor: target,
                            card: f.card.clone(),
                            source: f.user,
                            target: Some(f.user),
                            damage: 1,
                            damage_type: DamageType::Normal,
                            consume_card: false,
                            ignore_armor: false,
                        }),
                    );
                    f.stage = TrickStage::WaitChild;
                    update_parent(st, Frame::Trick(f));
                }
                "jiedao_sharen" => {
                    let a = f.first.unwrap_or(target);
                    let b = f.second.unwrap_or(f.user);
                    push(
                        st,
                        Frame::Respond(RespondFrame {
                            kind: RespondKind::Jiedao,
                            actor: a,
                            card: f.card.clone(),
                            source: f.user,
                            target: Some(b),
                            damage: 1,
                            damage_type: DamageType::Normal,
                            consume_card: false,
                            ignore_armor: false,
                        }),
                    );
                    f.stage = TrickStage::WaitChild;
                    update_parent(st, Frame::Trick(f));
                }
                "lebu_sishu" => {
                    st.seats[target].judge_zone.push(f.card.clone());
                    leave_processing(st, &f.card);
                    st.push_log(
                        None,
                        format!("【乐不思蜀】置入 {} 判定区", st.seats[target].general_name),
                    );
                    advance_trick_target(st, f)?;
                }
                "shandian" => {
                    st.seats[target].judge_zone.push(f.card.clone());
                    leave_processing(st, &f.card);
                    st.push_log(None, "【闪电】置入自己的判定区");
                    advance_trick_target(st, f)?;
                }
                other => return Err(internal(format!("未实现的锦囊效果：{other}"))),
            }
            Ok(false)
        }
        TrickStage::WaitChild => {
            if st.outcome.is_none() {
                return Err(internal("缺少子结算结果"));
            }
            st.outcome = None;
            advance_trick_target(st, f)?;
            Ok(false)
        }
        TrickStage::Done => {
            pop(st);
            Ok(false)
        }
    }
}

// --- 拿牌选择 -------------------------------------------------------------

fn step_pick(st: &mut SgsState, f: PickCardFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = Vec::new();
    for slot in EquipSlot::ALL {
        if let Some(Some(id)) = st.seats[f.target].equips.get(&slot) {
            options.push(PendingOption {
                id: format!("pick:equip:{}", slot_code(slot)),
                label: format!("装备区：{}", turn::card_label(st, id)),
            });
        }
    }
    if !st.seats[f.target].hand.is_empty() {
        options.push(PendingOption {
            id: "pick:hand".into(),
            label: format!("手牌：随机 1 张（共 {} 张）", st.seats[f.target].hand.len()),
        });
    }
    let judge: Vec<String> = st.seats[f.target].judge_zone.clone();
    for id in judge.iter().rev() {
        options.push(PendingOption {
            id: format!("pick:judge:{id}"),
            label: format!("判定区：{}", turn::card_label(st, id)),
        });
    }
    if options.is_empty() {
        st.outcome = Some(Outcome::Picked { card: None });
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "pick_card".into(),
        actor: st.seats[f.user].player_id.clone(),
        skill: None,
        options,
        data: json!({ "card": f.card, "target": st.seats[f.target].player_id }),
    });
    replace_top(st, Frame::PickCard(f));
    Ok(true)
}

fn slot_code(slot: EquipSlot) -> &'static str {
    match slot {
        EquipSlot::Weapon => "weapon",
        EquipSlot::Armor => "armor",
        EquipSlot::HorseOffense => "horse_offense",
        EquipSlot::HorseDefense => "horse_defense",
        EquipSlot::Treasure => "treasure",
    }
}

fn slot_from_code(code: &str) -> Option<EquipSlot> {
    Some(match code {
        "weapon" => EquipSlot::Weapon,
        "armor" => EquipSlot::Armor,
        "horse_offense" => EquipSlot::HorseOffense,
        "horse_defense" => EquipSlot::HorseDefense,
        "treasure" => EquipSlot::Treasure,
        _ => return None,
    })
}

// --- 响应（闪 / 杀 / 决斗 / 借刀）------------------------------------------

fn respond_options(st: &SgsState, f: &RespondFrame) -> Vec<PendingOption> {
    let code = match f.kind {
        RespondKind::Shan => "shan",
        RespondKind::Nanman | RespondKind::Duel | RespondKind::Jiedao => "sha",
    };
    let mut options: Vec<PendingOption> = turn::hand_cards_with_def(st, f.actor, code)
        .iter()
        .map(|id| PendingOption {
            id: format!("respond:{id}"),
            label: format!("打出【{}】", turn::card_label(st, id)),
        })
        .collect();
    options.push(pending_pass());
    options
}

fn step_respond(st: &mut SgsState, f: RespondFrame) -> Result<bool, EngineError> {
    // 主公技（激将/护驾）由同势力角色代替响应：结果通过 outcome 回传
    if let Some(Outcome::Respond { played, played_by }) = st.outcome.take() {
        match played {
            Some(card) if !card.is_empty() => {
                let by = played_by.unwrap_or(f.actor);
                finish_respond_play(st, &f, &card, by)?;
            }
            _ => {
                pop(st);
            }
        }
        return Ok(false);
    }
    let mut options = respond_options(st, &f);
    let needed = match f.kind {
        RespondKind::Shan => "shan",
        RespondKind::Nanman | RespondKind::Duel | RespondKind::Jiedao => "sha",
    };
    // 八卦阵：需要【闪】时可判定，红色视为打出【闪】（青釭剑无视）
    if needed == "shan"
        && !f.ignore_armor
        && st
            .equip_def(f.actor, EquipSlot::Armor)
            .is_some_and(|d| d.code == "bagua_zhen")
    {
        options.insert(
            options.len().saturating_sub(1),
            PendingOption {
                id: "skill:bagua".into(),
                label: "八卦阵：判定，红色视为出【闪】".into(),
            },
        );
    }
    if crate::skills::requisition_available(st, f.actor, needed) {
        let (action, label) = if needed == "sha" {
            ("skill:jijiang", "发动【激将】（蜀势力代出杀）")
        } else {
            ("skill:hujia", "发动【护驾】（魏势力代出闪）")
        };
        options.insert(
            options.len().saturating_sub(1),
            PendingOption {
                id: action.into(),
                label: label.into(),
            },
        );
    }
    st.pending = Some(Pending {
        kind: "respond".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: None,
        options,
        data: json!({
            "kind": match f.kind {
                RespondKind::Shan => "shan",
                RespondKind::Nanman => "nanman",
                RespondKind::Duel => "duel",
                RespondKind::Jiedao => "jiedao",
            },
            "card": f.card,
            "source": f.source,
            "target": f.target,
            "damage": f.damage,
        }),
    });
    replace_top(st, Frame::Respond(f));
    Ok(true)
}

// --- 濒死 -----------------------------------------------------------------

fn step_dying(
    st: &mut SgsState,
    mut f: DyingFrame,
    events: &mut Vec<EngineEvent>,
) -> Result<bool, EngineError> {
    while f.cursor < f.order.len() {
        let pid = &f.order[f.cursor];
        if alive_of_order(st, pid) && !f.passed.contains(pid) {
            break;
        }
        f.cursor += 1;
    }
    if f.cursor >= f.order.len() {
        let victim = f.victim;
        let source = f.source;
        pop(st);
        death(st, victim, source, events)?;
        return Ok(false);
    }
    let pid = f.order[f.cursor].clone();
    let seat = st
        .seat_index(&pid)
        .ok_or_else(|| internal("濒死询问对象不在座"))?;
    let mut options: Vec<PendingOption> = turn::hand_cards_with_def(st, seat, "tao")
        .iter()
        .map(|id| PendingOption {
            id: format!("respond:{id}"),
            label: format!("使用【{}】救援", turn::card_label(st, id)),
        })
        .collect();
    options.push(pending_pass());
    st.pending = Some(Pending {
        kind: "dying".into(),
        actor: pid,
        skill: None,
        options,
        data: json!({ "victim": st.seats[f.victim].player_id, "hp": st.seats[f.victim].hp }),
    });
    replace_top(st, Frame::Dying(f));
    Ok(true)
}

// --- 弃牌 -----------------------------------------------------------------

fn step_discard(st: &mut SgsState, f: DiscardFrame) -> Result<bool, EngineError> {
    let hand = turn::hand_ids(st, f.actor);
    if f.required == 0 || hand.len() < f.required {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "discard".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: None,
        options: hand
            .iter()
            .map(|id| PendingOption {
                id: format!("discard:{id}"),
                label: turn::card_label(st, id),
            })
            .collect(),
        data: json!({ "required": f.required, "reason": f.reason }),
    });
    replace_top(st, Frame::Discard(f));
    Ok(true)
}

// --- 判定 -----------------------------------------------------------------

fn step_judge(
    st: &mut SgsState,
    mut f: JudgeFrame,
    events: &mut Vec<EngineEvent>,
) -> Result<bool, EngineError> {
    match f.stage {
        JudgeStage::Wuxie => {
            let order = wuxie_order(st, f.owner);
            f.stage = JudgeStage::WaitWuxie;
            replace_top(st, Frame::Judge(f.clone()));
            push(
                st,
                Frame::Wuxie(WuxieFrame {
                    root: WuxieRoot::Judge {
                        delayed: f.delayed.clone(),
                        owner: f.owner,
                    },
                    count: 0,
                    order,
                    cursor: 0,
                    passed: 0,
                }),
            );
            Ok(false)
        }
        JudgeStage::WaitWuxie => match st.outcome.take() {
            Some(Outcome::Wuxie { cancelled }) => {
                if cancelled {
                    if card_code(st, &f.delayed) == "shandian" {
                        leave_processing(st, &f.delayed);
                        match turn::next_judge_lightning_target(st, f.owner) {
                            Some(next) => {
                                st.seats[next].judge_zone.push(f.delayed.clone());
                                st.push_log(
                                    None,
                                    format!(
                                        "【闪电】被无懈，流转到 {} 判定区",
                                        st.seats[next].general_name
                                    ),
                                );
                            }
                            None => {
                                to_discard(st, &f.delayed);
                                st.push_log(None, "【闪电】被无懈，无可流转目标，置入弃牌堆");
                            }
                        }
                    } else {
                        leave_processing(st, &f.delayed);
                        to_discard(st, &f.delayed);
                        st.push_log(
                            None,
                            format!(
                                "【{}】被无懈，未判定直接置入弃牌堆",
                                card_name(st, &f.delayed)
                            ),
                        );
                    }
                    pop(st);
                } else {
                    f.stage = JudgeStage::Reveal;
                    replace_top(st, Frame::Judge(f));
                }
                Ok(false)
            }
            _ => Err(internal("缺少判定无懈结果")),
        },
        JudgeStage::Reveal => {
            // 先出栈，再结算（判定可能导致濒死压栈）
            let delayed = f.delayed.clone();
            let owner = f.owner;
            pop(st);
            turn::resolve_judge_reveal(st, delayed, owner, events)?;
            Ok(false)
        }
    }
}

// ---------------------------------------------------------------------------
// 输入处理
// ---------------------------------------------------------------------------

/// 处理栈顶 pending 的动作（由 `turn::execute` 调用）。
pub fn execute_pending(
    st: &mut SgsState,
    player_id: &str,
    action: &str,
) -> Result<String, EngineError> {
    let pending = st.pending.clone().ok_or_else(|| illegal("当前没有待办"))?;
    if pending.actor != player_id {
        return Err(EngineError::PermissionDenied("当前等待其他玩家响应".into()));
    }
    let msg = match pending.kind.as_str() {
        "wuxie" => act_wuxie(st, &pending, action)?,
        "target" => act_target(st, &pending, action)?,
        "respond" => act_respond(st, &pending, action)?,
        "pick_card" => act_pick(st, &pending, action)?,
        "dying" => act_dying(st, &pending, action)?,
        "discard" => act_discard(st, &pending, action)?,
        "choice" => match pending.skill.as_deref() {
            Some("requisition") => crate::resolve_more::act_requisition(st, &pending, action)?,
            Some("tuxi") => crate::resolve_more::act_tuxi_offer(st, action)?,
            _ => crate::resolve_skill::act_jianxiong(st, action)?,
        },
        "choose_many" => match top(st) {
            Some(Frame::MultiSelect(_)) => crate::resolve_skill::act_multiselect(st, action)?,
            Some(Frame::Rende(_)) | Some(Frame::Lijian(_)) => {
                crate::resolve_more::act_select_skill(st, action)?
            }
            _ => return Err(internal("栈顶不是多选帧")),
        },
        other => return Err(internal(format!("未知 pending 类型：{other}"))),
    };
    st.pending = None;
    Ok(msg)
}

fn act_wuxie(st: &mut SgsState, pending: &Pending, action: &str) -> Result<String, EngineError> {
    let mut f = match top(st).cloned() {
        Some(Frame::Wuxie(f)) => f,
        _ => return Err(internal("栈顶不是无懈帧")),
    };
    let pid = pending.actor.clone();
    if action == "pass" {
        f.cursor += 1;
        f.passed += 1;
        replace_top(st, Frame::Wuxie(f));
        return Ok("不出无懈可击".to_string());
    }
    let card_id = action
        .strip_prefix("respond:")
        .ok_or_else(|| illegal("需要打出无懈可击或选择不出"))?;
    let seat = st.seat_index(&pid).ok_or_else(|| internal("玩家不在座"))?;
    if turn::card_def(st, card_id).map(|d| d.code) != Some("wuxie_keji") {
        return Err(illegal("只能打出【无懈可击】"));
    }
    turn::take_card(st, seat, card_id)?;
    turn::to_discard_card(st, card_id);
    f.count += 1;
    f.cursor = 0;
    f.passed = 0;
    st.push_log(
        Some(&pid),
        format!(
            "打出【{}】（无懈层数 {}）",
            turn::card_label(st, card_id),
            f.count
        ),
    );
    let count = f.count;
    replace_top(st, Frame::Wuxie(f));
    // 集智：使用非延时锦囊（无懈）也摸 1
    let mut ev = Vec::new();
    crate::skills::on_non_delayed_trick_used(st, seat, &mut ev);
    Ok(format!("已打出无懈可击（第 {count} 层）"))
}

fn act_target(st: &mut SgsState, pending: &Pending, action: &str) -> Result<String, EngineError> {
    // 杀：选目标后进入【闪】响应
    if let Some(Frame::Sha(mut f)) = top(st).cloned() {
        let target_pid = action
            .strip_prefix("target:")
            .ok_or_else(|| illegal("需要选择目标"))?;
        let target = st
            .seat_index(target_pid)
            .ok_or_else(|| illegal("目标不在座"))?;
        if !pending
            .options
            .iter()
            .any(|o| o.id == format!("target:{target_pid}"))
        {
            return Err(illegal("目标不合法"));
        }
        f.target = Some(target);
        f.stage = ShaStage::WaitRespond;
        replace_top(st, Frame::Sha(f.clone()));
        push(
            st,
            Frame::Respond(RespondFrame {
                kind: RespondKind::Shan,
                actor: target,
                card: f.card.clone(),
                source: f.user,
                target: Some(target),
                damage: 1,
                damage_type: DamageType::Normal,
                consume_card: false,
                ignore_armor: st.has_equip(f.user, "qinggang_jian"),
            }),
        );
        return Ok(format!("已选择目标：{}", st.seats[target].general_name));
    }

    if let Some(msg) = crate::resolve_more::target_action(st, pending, action)? {
        return Ok(msg);
    }
    let mut f = match top(st).cloned() {
        Some(Frame::Trick(f)) => f,
        _ => return Err(internal("栈顶不是可指定目标的帧")),
    };
    let stage = pending.data["stage"].as_str().unwrap_or("single");
    match action {
        "cancel" if stage != "single" => {
            f.first = None;
            f.second = None;
            f.targets.clear();
            f.stage = TrickStage::Target;
            replace_top(st, Frame::Trick(f));
            return Ok("已取消选择".to_string());
        }
        "confirm" if stage == "confirm" => {
            let (Some(first), Some(_)) = (f.first, f.second) else {
                return Err(illegal("尚未选全目标"));
            };
            f.targets = vec![first];
            f.stage = TrickStage::Wuxie;
            replace_top(st, Frame::Trick(f));
            return Ok("目标确认".to_string());
        }
        _ => {}
    }
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择目标"))?;
    let target = st
        .seat_index(target_pid)
        .ok_or_else(|| illegal("目标不在座"))?;
    if !pending
        .options
        .iter()
        .any(|o| o.id == format!("target:{target_pid}"))
    {
        return Err(illegal("目标不合法"));
    }
    match stage {
        "borrow_first" => {
            f.first = Some(target);
            f.stage = TrickStage::Target;
            replace_top(st, Frame::Trick(f));
            Ok(format!("已选择借刀对象：{}", st.seats[target].general_name))
        }
        "borrow_second" => {
            f.second = Some(target);
            f.stage = TrickStage::Target;
            replace_top(st, Frame::Trick(f));
            Ok(format!("已选择杀目标：{}", st.seats[target].general_name))
        }
        _ => {
            f.targets = vec![target];
            f.stage = if needs_wuxie(&f.code) {
                TrickStage::Wuxie
            } else {
                TrickStage::Effect
            };
            replace_top(st, Frame::Trick(f));
            Ok(format!("已选择目标：{}", st.seats[target].general_name))
        }
    }
}

fn act_respond(st: &mut SgsState, _pending: &Pending, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Respond(f)) => f,
        _ => return Err(internal("栈顶不是响应帧")),
    };
    let actor = f.actor;
    if action == "skill:bagua" {
        if f.kind != RespondKind::Shan {
            return Err(illegal("八卦阵只能用于需要【闪】时"));
        }
        if f.ignore_armor
            || !st
                .equip_def(actor, EquipSlot::Armor)
                .is_some_and(|d| d.code == "bagua_zhen")
        {
            return Err(illegal("没有可用的【八卦阵】"));
        }
        let mut ev = Vec::new();
        let judge_id = turn::draw_one(st, &mut ev).ok_or_else(|| internal("牌堆不可用"))?;
        let suit = st.cards.get(&judge_id).map(|c| c.suit);
        turn::to_discard_card(st, &judge_id);
        let red = matches!(suit, Some(Suit::Heart) | Some(Suit::Diamond));
        st.push_log(
            Some(&st.seats[actor].player_id.clone()),
            format!(
                "八卦阵判定：{}，{}",
                turn::card_label(st, &judge_id),
                if red {
                    "视为出【闪】"
                } else {
                    "未生效"
                }
            ),
        );
        if red {
            finish_respond_dodge(st, &f);
            return Ok("八卦阵生效：视为出【闪】".to_string());
        }
        return Ok("八卦阵判定失败，可继续响应".to_string());
    }
    if action == "skill:jijiang" || action == "skill:hujia" {
        let needed = if action == "skill:jijiang" {
            "sha"
        } else {
            "shan"
        };
        if !crate::skills::requisition_available(st, actor, needed) {
            return Err(illegal("无法发动该主公技"));
        }
        let order = crate::skills::requisition_order(st, actor, needed);
        push(
            st,
            Frame::Requisition(crate::resolve_more::RequisitionFrame {
                lord: actor,
                needed: needed.into(),
                order,
                cursor: 0,
            }),
        );
        return Ok("发动主公技，等待同势力角色响应".to_string());
    }
    if action == "pass" {
        // 先出栈，再造成伤害（伤害可能压入濒死帧）
        pop(st);
        st.outcome = Some(Outcome::Respond {
            played: None,
            played_by: None,
        });
        if f.consume_card {
            discard_from_processing(st, &f.card);
        }
        match f.kind {
            RespondKind::Shan => {
                let card = f.card.clone();
                apply_damage(
                    st,
                    actor,
                    f.damage,
                    Some(f.source),
                    f.damage_type,
                    Some(&card),
                );
            }
            RespondKind::Nanman => {
                let card = f.card.clone();
                apply_damage(
                    st,
                    actor,
                    f.damage,
                    Some(f.source),
                    f.damage_type,
                    Some(&card),
                );
            }
            RespondKind::Duel => {
                let opponent = f.target.unwrap_or(f.source);
                let card = f.card.clone();
                apply_damage(
                    st,
                    actor,
                    f.damage,
                    Some(opponent),
                    f.damage_type,
                    Some(&card),
                );
            }
            RespondKind::Jiedao => {
                let weapon = st.seats[actor]
                    .equips
                    .get(&EquipSlot::Weapon)
                    .cloned()
                    .flatten();
                if let Some(w) = weapon {
                    st.seats[actor].equips.insert(EquipSlot::Weapon, None);
                    st.seats[f.source].hand.push(w.clone());
                    st.push_log(
                        None,
                        format!(
                            "{} 未出杀，武器【{}】交给 {}",
                            st.seats[actor].general_name,
                            card_name(st, &w),
                            st.seats[f.source].general_name
                        ),
                    );
                }
            }
        }
        return Ok("已放弃响应".to_string());
    }

    let card_id = action
        .strip_prefix("respond:")
        .ok_or_else(|| illegal("需要打出牌或 pass"))?;
    finish_respond_play(st, &f, card_id, actor)
}

/// 八卦阵等"视为出闪"：不消耗手牌，通知父帧本次响应成功。
fn finish_respond_dodge(st: &mut SgsState, f: &RespondFrame) {
    if f.consume_card {
        discard_from_processing(st, &f.card);
    }
    st.outcome = Some(Outcome::Respond {
        played: Some("bagua".into()),
        played_by: Some(f.actor),
    });
    pop(st);
}

/// 响应牌已确定（可能是本人或主公技代替者）：统一结算并出栈。
fn finish_respond_play(
    st: &mut SgsState,
    f: &RespondFrame,
    card_id: &str,
    played_by: usize,
) -> Result<String, EngineError> {
    let expected = match f.kind {
        RespondKind::Shan => "shan",
        RespondKind::Nanman | RespondKind::Duel | RespondKind::Jiedao => "sha",
    };
    if turn::card_def(st, card_id).map(|d| d.code) != Some(expected) {
        return Err(illegal("打出的牌不符合要求"));
    }
    turn::take_card(st, played_by, card_id)?;
    st.push_log(
        Some(&st.seats[played_by].player_id.clone()),
        format!("打出【{}】", turn::card_label(st, card_id)),
    );
    match f.kind {
        RespondKind::Shan => {
            turn::to_discard_card(st, card_id);
            if f.consume_card {
                discard_from_processing(st, &f.card);
            }
            st.outcome = Some(Outcome::Respond {
                played: Some(card_id.to_string()),
                played_by: Some(played_by),
            });
            pop(st);
            Ok("【闪】抵消了结算".to_string())
        }
        RespondKind::Nanman => {
            turn::to_discard_card(st, card_id);
            st.outcome = Some(Outcome::Respond {
                played: Some(card_id.to_string()),
                played_by: Some(played_by),
            });
            pop(st);
            Ok("已打出【杀】响应南蛮入侵".to_string())
        }
        RespondKind::Duel => {
            turn::to_discard_card(st, card_id);
            let opponent = f.target.unwrap_or(f.source);
            replace_top(
                st,
                Frame::Respond(RespondFrame {
                    kind: RespondKind::Duel,
                    actor: opponent,
                    card: f.card.clone(),
                    source: played_by,
                    target: Some(played_by),
                    damage: f.damage,
                    damage_type: f.damage_type,
                    consume_card: false,
                    ignore_armor: false,
                }),
            );
            Ok("已打出【杀】，轮到对方".to_string())
        }
        RespondKind::Jiedao => {
            st.deck.processing.push(card_id.to_string());
            let b = f.target.unwrap_or(f.source);
            replace_top(
                st,
                Frame::Respond(RespondFrame {
                    kind: RespondKind::Shan,
                    actor: b,
                    card: card_id.to_string(),
                    source: played_by,
                    target: Some(b),
                    damage: 1,
                    damage_type: DamageType::Normal,
                    consume_card: true,
                    ignore_armor: st.has_equip(played_by, "qinggang_jian"),
                }),
            );
            Ok("A 已出杀，等待目标响应".to_string())
        }
    }
}

fn act_pick(st: &mut SgsState, pending: &Pending, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::PickCard(f)) => f,
        _ => return Err(internal("栈顶不是拿牌帧")),
    };
    let target = f.target;
    let picked = if let Some(rest) = action.strip_prefix("pick:equip:") {
        let slot = slot_from_code(rest).ok_or_else(|| illegal("非法装备槽"))?;
        let card = st.seats[target]
            .equips
            .get(&slot)
            .cloned()
            .flatten()
            .ok_or_else(|| illegal("该槽位没有装备"))?;
        st.seats[target].equips.insert(slot, None);
        card
    } else if action == "pick:hand" {
        if st.seats[target].hand.is_empty() {
            return Err(illegal("目标没有手牌"));
        }
        let n = st.seats[target].hand.len();
        let idx = st.next_index(n);
        st.seats[target].hand.remove(idx)
    } else if let Some(card) = action.strip_prefix("pick:judge:") {
        let pos = st.seats[target]
            .judge_zone
            .iter()
            .position(|c| c == card)
            .ok_or_else(|| illegal("判定区没有该牌"))?;
        st.seats[target].judge_zone.remove(pos)
    } else {
        return Err(illegal("未知的拿牌选项"));
    };

    let picked_name = turn::card_label(st, &picked);
    if f.mode == PickMode::Shunshou {
        st.seats[f.user].hand.push(picked.clone());
        st.push_log(
            Some(&st.seats[f.user].player_id.clone()),
            format!("顺手牵羊：获得【{picked_name}】"),
        );
    } else {
        to_discard(st, &picked);
        st.push_log(
            Some(&st.seats[f.user].player_id.clone()),
            format!("过河拆桥：弃置【{picked_name}】"),
        );
    }
    pop(st);
    st.outcome = Some(Outcome::Picked {
        card: Some(picked.clone()),
    });
    if f.mode == PickMode::Shunshou {
        let user = f.user;
        let over = st.seats[user]
            .hand
            .len()
            .saturating_sub(st.config.max_hand_cards as usize);
        push_discard(st, user, over, "hand_cap");
    }
    let _ = pending;
    Ok(format!("已选择【{picked_name}】"))
}

fn act_dying(st: &mut SgsState, pending: &Pending, action: &str) -> Result<String, EngineError> {
    let mut f = match top(st).cloned() {
        Some(Frame::Dying(f)) => f,
        _ => return Err(internal("栈顶不是濒死帧")),
    };
    let seat = st
        .seat_index(&pending.actor)
        .ok_or_else(|| internal("玩家不在座"))?;
    if action == "pass" {
        f.passed.push(pending.actor.clone());
        f.cursor += 1;
        replace_top(st, Frame::Dying(f));
        return Ok("放弃救援".to_string());
    }
    let card_id = action
        .strip_prefix("respond:")
        .ok_or_else(|| illegal("需要使用桃或 pass"))?;
    if turn::card_def(st, card_id).map(|d| d.code) != Some("tao") {
        return Err(illegal("只能用【桃】救援"));
    }
    turn::take_card(st, seat, card_id)?;
    turn::to_discard_card(st, card_id);
    let victim = f.victim;
    let bonus = crate::skills::tao_bonus(st, seat, victim);
    st.seats[victim].hp += 1 + bonus;
    let hp = st.seats[victim].hp;
    st.push_log(
        Some(&pending.actor.clone()),
        format!(
            "使用【{}】救援 {}（体力 {hp}{}）",
            turn::card_label(st, card_id),
            st.seats[victim].general_name,
            if bonus > 0 { "，救援 +1" } else { "" }
        ),
    );
    if hp > 0 {
        st.phase = Phase::Turn;
        pop(st);
        return Ok(format!("{} 脱离濒死", st.seats[victim].general_name));
    }
    f.passed.clear();
    replace_top(st, Frame::Dying(f));
    Ok(format!(
        "{} 仍濒死（体力 {hp}）",
        st.seats[victim].general_name
    ))
}

fn act_discard(st: &mut SgsState, pending: &Pending, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Discard(f)) => f,
        _ => return Err(internal("栈顶不是弃牌帧")),
    };
    let required = f.required;
    let csv = action
        .strip_prefix("discard:")
        .ok_or_else(|| illegal("需要弃牌"))?;
    let ids: Vec<String> = csv
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    if ids.len() != required && !(required > 1 && ids.len() == 1) {
        return Err(illegal(format!("需弃 {required} 张牌")));
    }
    let mut seen = std::collections::BTreeSet::new();
    for id in &ids {
        if !seen.insert(id.clone()) {
            return Err(illegal("弃牌列表重复"));
        }
        turn::take_card(st, f.actor, id)?;
        turn::to_discard_card(st, id);
    }
    let remaining = required - ids.len();
    let text = if remaining == 0 {
        if f.reason == "hand_cap" {
            format!("超过持有上限，弃置 {} 张", ids.len())
        } else {
            format!("弃牌阶段：弃置 {} 张", ids.len())
        }
    } else {
        format!("还需弃 {remaining} 张")
    };
    st.push_log(Some(&pending.actor.clone()), text.clone());
    if remaining == 0 {
        pop(st);
    } else {
        replace_top(
            st,
            Frame::Discard(DiscardFrame {
                required: remaining,
                ..f
            }),
        );
    }
    Ok(text)
}

// ---------------------------------------------------------------------------
// 伤害 / 死亡 / 胜负
// ---------------------------------------------------------------------------

pub fn apply_damage(
    st: &mut SgsState,
    victim: usize,
    amount: i32,
    source: Option<usize>,
    dtype: DamageType,
    card: Option<&str>,
) {
    st.seats[victim].hp -= amount;
    let pid = st.seats[victim].player_id.clone();
    st.push_log(
        None,
        format!(
            "{pid} 受到 {amount} 点{}伤害（体力 {}）",
            dtype.label(),
            st.seats[victim].hp
        ),
    );
    if st.seats[victim].hp <= 0 {
        push_dying(st, victim, source);
        return;
    }
    // 奸雄：受伤后（存活时）可获得造成伤害的牌
    if let Some(card_id) = card {
        if crate::skills::jianxiong_enabled(st, victim)
            && st.deck.processing.iter().any(|c| c == card_id)
        {
            push(
                st,
                Frame::Jianxiong(JianxiongFrame {
                    actor: victim,
                    card: card_id.to_string(),
                }),
            );
        }
    }
}

pub fn push_dying(st: &mut SgsState, victim: usize, source: Option<usize>) {
    let order = wuxie_order(st, st.turn);
    st.phase = Phase::Dying;
    push(
        st,
        Frame::Dying(DyingFrame {
            victim,
            source,
            order,
            cursor: 0,
            passed: Vec::new(),
        }),
    );
}

fn death(
    st: &mut SgsState,
    victim: usize,
    source: Option<usize>,
    events: &mut Vec<EngineEvent>,
) -> Result<(), EngineError> {
    let pid = st.seats[victim].player_id.clone();
    let identity = st.seats[victim].identity;
    st.seats[victim].alive = false;
    st.seats[victim].hp = 0;
    let hand = std::mem::take(&mut st.seats[victim].hand);
    let judge = std::mem::take(&mut st.seats[victim].judge_zone);
    let equips: Vec<String> = st.seats[victim]
        .equips
        .values_mut()
        .filter_map(Option::take)
        .collect();
    for c in hand.into_iter().chain(judge).chain(equips) {
        to_discard(st, &c);
    }
    st.push_log(None, format!("{pid} 阵亡（身份公开）"));
    events.push(EngineEvent::new(
        "player_died",
        json!({ "player": pid, "identity": identity.code() }),
    ));
    st.phase = Phase::Turn;
    st.pending = None;

    // 死亡奖惩（rules-rulings §3）
    if let Some(killer) = source {
        if killer != victim && st.seats[killer].alive {
            if identity == Identity::Rebel {
                let drawn = turn::draw_cards(st, killer, 3, events);
                st.push_log(
                    None,
                    format!("{} 杀死反贼，摸 {drawn} 张", st.seats[killer].general_name),
                );
                let over = st.seats[killer]
                    .hand
                    .len()
                    .saturating_sub(st.config.max_hand_cards as usize);
                push_discard(st, killer, over, "hand_cap");
            } else if identity == Identity::Loyalist && st.seats[killer].identity == Identity::Lord
            {
                let hand = std::mem::take(&mut st.seats[killer].hand);
                let equips: Vec<String> = st.seats[killer]
                    .equips
                    .values_mut()
                    .filter_map(Option::take)
                    .collect();
                for c in hand.into_iter().chain(equips) {
                    to_discard(st, &c);
                }
                st.push_log(None, "主公杀死忠臣：弃置所有手牌与装备".to_string());
            }
        }
    }

    if let Some(winner) = check_winner(st) {
        st.winner = Some(winner);
        st.phase = Phase::Finished;
        st.resolution.clear();
        st.pending = None;
        st.push_log(None, "对局结束");
    }
    Ok(())
}

fn check_winner(st: &SgsState) -> Option<Winner> {
    let lord_alive = st.lord_index().is_some_and(|i| st.seats[i].alive);
    let alive = st.alive_indices();
    if !lord_alive {
        if alive.len() == 1 && st.seats[alive[0]].identity == Identity::Spy {
            return Some(Winner {
                team: "spy".into(),
                players: vec![st.seats[alive[0]].player_id.clone()],
            });
        }
        let rebels: Vec<String> = st
            .seats
            .iter()
            .filter(|s| s.identity == Identity::Rebel)
            .map(|s| s.player_id.clone())
            .collect();
        return Some(Winner {
            team: "rebel".into(),
            players: rebels,
        });
    }
    if alive
        .iter()
        .all(|i| matches!(st.seats[*i].identity, Identity::Lord | Identity::Loyalist))
    {
        let winners: Vec<String> = st
            .seats
            .iter()
            .filter(|s| matches!(s.identity, Identity::Lord | Identity::Loyalist))
            .map(|s| s.player_id.clone())
            .collect();
        return Some(Winner {
            team: "lord".into(),
            players: winners,
        });
    }
    None
}
