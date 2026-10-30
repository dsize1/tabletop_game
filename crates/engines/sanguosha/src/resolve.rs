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
    /// 麒麟弓：杀造成伤害后，攻击方可弃置目标一匹坐骑。
    Qilin(crate::resolve_skill::QilinFrame),
    Requisition(crate::resolve_more::RequisitionFrame),
    Rende(crate::resolve_more::RendeFrame),
    Lijian(crate::resolve_more::LijianFrame),
    TuxiOffer(crate::resolve_more::TuxiOfferFrame),
    Tuxi(crate::resolve_more::TuxiFrame),
    /// 裸衣（P4.1：摸牌阶段可选，少摸 1 张换本回合杀/决斗伤害 +1）。
    Luoyi(crate::resolve_more::LuoyiFrame),
    /// 青囊（选择回复目标）。
    Qingnang(crate::resolve_more::QingnangFrame),
    /// 结姻（选择结姻对象）。
    Jieyin(crate::resolve_more::JieyinFrame),
    /// 遗计（逐张分配摸到的牌）。
    Yiji(crate::resolve_more::YijiFrame),
    /// 刚烈（伤害来源二选一）。
    Ganglie(crate::resolve_more::GanglieFrame),
    /// 洛神（准备阶段反复判定）。
    Luoshen(crate::resolve_more::LuoshenFrame),
    /// 反间（选择目标 → 目标选花色）。
    Fanjian(crate::resolve_more::FanjianFrame),
    /// 流离（成为【杀】目标时弃牌转移）。
    Liuli(crate::resolve_more::LiuliFrame),
    /// 青龙偃月刀 / 贯石斧：被【闪】抵消后的追加窗口。
    AfterDodge(crate::resolve_skill::AfterDodgeFrame),
    /// 寒冰剑：造成伤害时改为弃置目标两张牌。
    Hanbing(crate::resolve_skill::HanbingFrame),
    /// 雌雄双股剑：指定异性目标后二选一。
    Cixiong(crate::resolve_skill::CixiongFrame),
    /// 通用技能判定（judge_before）。
    QuickJudge(QuickJudgeFrame),
    /// 火攻（目标亮牌 → 弃同花色 → 火焰伤害）。
    HuoGong(HuoGongFrame),
    /// 五谷丰登（P2.5 特殊区 `wugu`）。
    Wugu(crate::resolve_zone::WuguFrame),
    /// 观星排序提交（P2.5 特殊区 `guanxing`，`arrange:<csv>`）。
    Arrange(crate::resolve_zone::ArrangeFrame),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JianxiongFrame {
    pub actor: usize,
    pub card: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiSelectFrame {
    pub actor: usize,
    /// 技能/用途标识（当前：zhiheng / qingnang / jieyin / ganglie / guanshi）。
    pub purpose: String,
    pub selected: Vec<String>,
    pub min: usize,
    pub max: usize,
    /// 用途附加数据（如贯石斧的 source/target/card）。
    #[serde(default)]
    pub data: serde_json::Value,
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
    /// 方天画戟：后续还需要响应的目标（顺序结算）。
    #[serde(default)]
    pub pending_targets: Vec<usize>,
    /// 方天画戟：本【杀】是否为使用者最后一张手牌（使用时判定）。
    #[serde(default)]
    pub fangtian: bool,
    /// 属性伤害（火杀/雷杀；普通杀为 Normal）。
    #[serde(default)]
    pub damage_type: DamageType,
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
    /// 反馈：获得伤害来源的一张牌（手牌随机 / 装备指定；不含判定区）。
    Fankui,
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DamageType {
    #[default]
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
    /// 八卦阵：本次响应是否已经判定过（每次需要【闪】只能判一次）。
    #[serde(default)]
    pub bagua_used: bool,
    /// 本次响应需要打出的牌数（无双【杀】=2、【决斗】=2；0 视为 1）。
    #[serde(default)]
    pub need: u32,
    /// 已打出的响应牌数。
    #[serde(default)]
    pub played: u32,
    /// 铁骑：目标不能使用【闪】（自动放弃响应）。
    #[serde(default)]
    pub no_shan: bool,
    /// 决斗发起者（无双判定用；离间为 None）。
    #[serde(default)]
    pub duel_initiator: Option<usize>,
    /// 本次响应是否是【杀】的目标响应（青龙偃月刀/贯石斧追加窗口的判定依据）。
    #[serde(default)]
    pub is_sha: bool,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HuoGongFrame {
    pub user: usize,
    pub target: usize,
    /// 目标亮出的手牌。
    pub revealed: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuickJudgeStage {
    Reveal,
    Rewrite,
    WaitRewrite,
    Effect,
}

/// 通用技能判定帧（P4.2 `judge_before`）：铁骑 / 刚烈等非延时锦囊判定，
/// 亮牌后自动进入改判窗口（鬼才），结算后按 `reason` 派发效果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickJudgeFrame {
    pub owner: usize,
    /// 判定用途（tieqi / ganglie / ...）。
    pub reason: String,
    pub stage: QuickJudgeStage,
    #[serde(default)]
    pub judge_card: Option<String>,
    #[serde(default)]
    pub rewrite_order: Vec<usize>,
    #[serde(default)]
    pub rewrite_cursor: usize,
    #[serde(default)]
    pub data: serde_json::Value,
    /// 进入判定前父帧已写入的 outcome（刚烈等在伤害结算中判定时需原样归还）。
    #[serde(default)]
    pub saved_outcome: Option<Outcome>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JudgeStage {
    Wuxie,
    WaitWuxie,
    /// 亮出判定牌（抽牌进处理区 + 公开）。
    Reveal,
    /// 改判询问（按顺序推进）。
    Rewrite,
    /// 等待改判牌/放弃。
    WaitRewrite,
    /// 判定生效。
    Effect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JudgeFrame {
    pub delayed: String,
    pub owner: usize,
    pub stage: JudgeStage,
    /// 亮出的判定牌（`deck.processing` 中；改判后为新牌）。
    #[serde(default)]
    pub judge_card: Option<String>,
    /// 改判询问顺序（座位下标；判定者左手侧起、判定者最后）。
    #[serde(default)]
    pub rewrite_order: Vec<usize>,
    /// 改判询问游标。
    #[serde(default)]
    pub rewrite_cursor: usize,
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
    /// 改判窗口结果：`card = None` 表示放弃改判。
    JudgeRewritten {
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

/// 无懈询问顺序（供 `resolve_zone` 复用）：从 `reference` 左手侧开始，`reference` 最后。
pub(crate) fn wuxie_order_of(st: &SgsState, reference: usize) -> Vec<String> {
    wuxie_order(st, reference)
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
            judge_card: None,
            rewrite_order: Vec::new(),
            rewrite_cursor: 0,
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
            // 终局时清掉可能残留的特殊区（五谷/观星必须在回合内结算完）。
            st.special_zones.remove(crate::resolve_zone::GUANXING_ZONE);
            st.special_zones.remove(crate::resolve_zone::WUGU_ZONE);
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
        Frame::Qilin(f) => crate::resolve_skill::step_qilin(st, f),
        Frame::Requisition(f) => crate::resolve_more::step_requisition(st, f),
        Frame::Rende(f) => crate::resolve_more::step_rende(st, f),
        Frame::Lijian(f) => crate::resolve_more::step_lijian(st, f),
        Frame::TuxiOffer(f) => crate::resolve_more::step_tuxi_offer(st, f),
        Frame::Tuxi(f) => crate::resolve_more::step_tuxi(st, f),
        Frame::Luoyi(f) => crate::resolve_more::step_luoyi(st, f),
        Frame::Qingnang(f) => crate::resolve_more::step_qingnang(st, f),
        Frame::Jieyin(f) => crate::resolve_more::step_jieyin(st, f),
        Frame::Yiji(f) => crate::resolve_more::step_yiji(st, f),
        Frame::Ganglie(f) => crate::resolve_more::step_ganglie(st, f),
        Frame::Luoshen(f) => crate::resolve_more::step_luoshen(st, f),
        Frame::Fanjian(f) => crate::resolve_more::step_fanjian(st, f),
        Frame::Liuli(f) => crate::resolve_more::step_liuli(st, f),
        Frame::AfterDodge(f) => crate::resolve_skill::step_after_dodge(st, f),
        Frame::Hanbing(f) => crate::resolve_skill::step_hanbing(st, f),
        Frame::Cixiong(f) => crate::resolve_skill::step_cixiong(st, f),
        Frame::QuickJudge(f) => step_quick_judge(st, f, events),
        Frame::HuoGong(f) => step_huogong(st, f),
        Frame::Wugu(f) => crate::resolve_zone::step_wugu(st, f, events),
        Frame::Arrange(f) => crate::resolve_zone::step_arrange(st, f, events),
    }
}

// --- 无懈 -----------------------------------------------------------------

fn step_wuxie(st: &mut SgsState, mut f: WuxieFrame) -> Result<bool, EngineError> {
    if f.passed as usize >= f.order.len() {
        let cancelled = f.count % 2 == 1;
        st.push_log(
            None,
            format!(
                "无懈可击：{} {}",
                f.root.summary(st),
                if cancelled {
                    "效果被抵消"
                } else {
                    "效果生效"
                }
            ),
        );
        st.outcome = Some(Outcome::Wuxie { cancelled });
        pop(st);
        return Ok(false);
    }
    let pid = f.order[f.cursor % f.order.len()].clone();
    let seat = st
        .seat_index(&pid)
        .ok_or_else(|| internal("无懈询问对象不在座"))?;
    // 无手牌 = 公共信息，无法打出任何响应牌，无需假犹豫（2026-10-28 用户裁定）。
    if !alive_of_order(st, &pid) || st.seats[seat].hand.is_empty() {
        f.cursor += 1;
        f.passed += 1;
        replace_top(st, Frame::Wuxie(f));
        return Ok(false);
    }
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

/// 方天画戟：枚举 2..=max 个目标的组合（顺序按 `targets` 原顺序）。
fn fangtian_combos(targets: &[usize], max: usize) -> Vec<Vec<usize>> {
    fn rec(pool: &[usize], start: usize, cur: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if cur.len() >= 2 {
            out.push(cur.clone());
        }
        if cur.len() >= 3 {
            return;
        }
        for i in start..pool.len() {
            cur.push(pool[i]);
            rec(pool, i + 1, cur, out);
            cur.pop();
        }
    }
    let mut out = Vec::new();
    let mut cur = Vec::new();
    rec(targets, 0, &mut cur, &mut out);
    out.retain(|c| c.len() <= max);
    out
}

fn step_sha(st: &mut SgsState, mut f: ShaFrame) -> Result<bool, EngineError> {
    match f.stage {
        ShaStage::Target => {
            let targets = turn::sha_targets(st, f.user);
            let mut options: Vec<PendingOption> = targets
                .iter()
                .map(|t| PendingOption {
                    id: format!("target:{}", st.seats[*t].player_id),
                    label: format!("对 {} 使用【杀】", st.seats[*t].general_name),
                })
                .collect();
            // 方天画戟：最后一张手牌的【杀】可额外指定至多两个目标（2~3 个目标的组合）。
            if f.fangtian && targets.len() >= 2 {
                let combos = fangtian_combos(&targets, 3);
                for combo in combos {
                    let ids: Vec<String> = combo
                        .iter()
                        .map(|t| st.seats[*t].player_id.clone())
                        .collect();
                    let names: Vec<String> = combo
                        .iter()
                        .map(|t| st.seats[*t].general_name.clone())
                        .collect();
                    options.push(PendingOption {
                        id: format!("targets:{}", ids.join(",")),
                        label: format!("方天画戟：同时指定 {}", names.join("、")),
                    });
                }
            }
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
                // 方天画戟：同一张【杀】依次对后续目标结算。
                if let Some(next) = f.pending_targets.first().copied() {
                    f.pending_targets.remove(0);
                    f.target = Some(next);
                    let need = if crate::skills::has(st, f.user, "wushuang") {
                        2
                    } else {
                        1
                    };
                    let ignore_armor =
                        crate::skills::has_equipment_skill(st, f.user, "qinggang_jian");
                    let card = f.card.clone();
                    let user = f.user;
                    let dtype = f.damage_type;
                    replace_top(st, Frame::Sha(f));
                    crate::resolve_skill::push_sha_target_response(
                        st,
                        user,
                        next,
                        &card,
                        need,
                        false,
                        ignore_armor,
                        dtype,
                    );
                    return Ok(false);
                }
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

pub fn start_sha(
    st: &mut SgsState,
    user: usize,
    card: &str,
    fangtian: bool,
) -> Result<(), EngineError> {
    start_sha_with(st, user, card, fangtian, DamageType::Normal)
}

/// 带属性伤害的【杀】（火杀/雷杀）。
pub fn start_sha_with(
    st: &mut SgsState,
    user: usize,
    card: &str,
    fangtian: bool,
    damage_type: DamageType,
) -> Result<(), EngineError> {
    st.deck.processing.push(card.to_string());
    st.bump_counter("sha_used");
    push(
        st,
        Frame::Sha(ShaFrame {
            card: card.to_string(),
            user,
            target: None,
            stage: ShaStage::Target,
            pending_targets: Vec::new(),
            fangtian,
            damage_type,
        }),
    );
    Ok(())
}

// --- 锦囊 -----------------------------------------------------------------

fn needs_wuxie(code: &str) -> bool {
    !matches!(code, "lebu_sishu" | "shandian" | "bingliang_cunduan")
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
    if code == "wuzhong_shengyou" || code == "shandian" {
        // 无中生有与闪电都只作用于自己（闪电置入自己判定区，下回合最先判定）。
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
                        .filter(|i| {
                            *i != first
                                && st.in_attack_range(first, *i)
                                // 空城（锁定）：借刀迫使的【杀】也不能指定无手牌空城角色。
                                && !crate::skills::kongcheng_blocks(st, *i)
                        })
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
                        .filter(|i| {
                            *i != f.user
                                && !crate::skills::qianxun_blocks(st, *i)
                                && turn::trick_distance_ok(st, f.user, *i)
                                && turn::has_cards_to_take(st, *i)
                        })
                        .collect(),
                    "guohe_chaiqiao" => st
                        .alive_indices()
                        .into_iter()
                        .filter(|i| {
                            *i != f.user
                                && !crate::skills::qianxun_blocks(st, *i)
                                && turn::has_cards_to_take(st, *i)
                        })
                        .collect(),
                    "lebu_sishu" => st
                        .alive_indices()
                        .into_iter()
                        .filter(|i| *i != f.user && !turn::judge_zone_has(st, *i, "lebu_sishu"))
                        .collect(),
                    "tiesuo_lianhuan" => st
                        .alive_indices()
                        .into_iter()
                        .filter(|i| *i != f.user)
                        .collect(),
                    "huo_gong" => st
                        .alive_indices()
                        .into_iter()
                        .filter(|i| *i != f.user && !st.seats[*i].hand.is_empty())
                        .collect(),
                    "bingliang_cunduan" => st
                        .alive_indices()
                        .into_iter()
                        .filter(|i| {
                            *i != f.user
                                && turn::trick_distance_ok(st, f.user, *i)
                                && !turn::judge_zone_has(st, *i, "bingliang_cunduan")
                        })
                        .collect(),
                    "wuzhong_shengyou" => vec![f.user],
                    // 空城（锁定）：无手牌者不能成为【决斗】目标。
                    "juedou" => st
                        .alive_indices()
                        .into_iter()
                        .filter(|i| *i != f.user && !crate::skills::kongcheng_blocks(st, *i))
                        .collect(),
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
                            bagua_used: false,
                            need: 1,
                            played: 0,
                            no_shan: false,
                            duel_initiator: None,
                            is_sha: false,
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
                            bagua_used: false,
                            need: 1,
                            played: 0,
                            no_shan: false,
                            duel_initiator: None,
                            is_sha: false,
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
                    // 无双：吕布使用【决斗】时，对方需连续打出两张【杀】。
                    let need = if crate::skills::has(st, f.user, "wushuang") {
                        2
                    } else {
                        1
                    };
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
                            bagua_used: false,
                            need,
                            played: 0,
                            no_shan: false,
                            duel_initiator: Some(f.user),
                            is_sha: false,
                        }),
                    );
                    f.stage = TrickStage::WaitChild;
                    update_parent(st, Frame::Trick(f));
                }
                "tiesuo_lianhuan" => {
                    st.seats[target].chained = !st.seats[target].chained;
                    let chained = st.seats[target].chained;
                    st.push_log(
                        None,
                        format!(
                            "铁索连环：{} {}",
                            st.seats[target].general_name,
                            if chained { "横置" } else { "重置" }
                        ),
                    );
                    advance_trick_target(st, f)?;
                }
                "huo_gong" => {
                    push(
                        st,
                        Frame::HuoGong(HuoGongFrame {
                            user: f.user,
                            target,
                            revealed: String::new(),
                        }),
                    );
                    // 亮牌在子帧首次步进时选择
                    if let Some(Frame::HuoGong(g)) = top(st).cloned() {
                        if g.revealed.is_empty() {
                            let n = st.seats[target].hand.len();
                            if n == 0 {
                                pop(st);
                                advance_trick_target(st, f)?;
                                return Ok(false);
                            }
                            let idx = st.next_index(n);
                            let revealed = st.seats[target].hand[idx].clone();
                            replace_top(
                                st,
                                Frame::HuoGong(HuoGongFrame {
                                    user: f.user,
                                    target,
                                    revealed,
                                }),
                            );
                        }
                    }
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
                            bagua_used: false,
                            need: 1,
                            played: 0,
                            no_shan: false,
                            duel_initiator: None,
                            is_sha: false,
                        }),
                    );
                    f.stage = TrickStage::WaitChild;
                    update_parent(st, Frame::Trick(f));
                }
                "lebu_sishu" | "bingliang_cunduan" => {
                    st.seats[target].judge_zone.push(f.card.clone());
                    leave_processing(st, &f.card);
                    let name = card_name(st, &f.card);
                    st.push_log(
                        None,
                        format!("【{name}】置入 {} 判定区", st.seats[target].general_name),
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
    if f.mode != PickMode::Fankui {
        let judge: Vec<String> = st.seats[f.target].judge_zone.clone();
        for id in judge.iter().rev() {
            options.push(PendingOption {
                id: format!("pick:judge:{id}"),
                label: format!("判定区：{}", turn::card_label(st, id)),
            });
        }
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

fn needed_name(code: &str) -> &'static str {
    match code {
        "sha" => "杀",
        "shan" => "闪",
        "tao" => "桃",
        _ => "牌",
    }
}

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
    // 丈八蛇矛（P4.2）：需要【杀】时，两张手牌当【杀】打出。
    if code == "sha" && crate::skills::has_equipment_skill(st, f.actor, "zhangba_shemao") {
        let hand: Vec<String> = st.seats[f.actor].hand.clone();
        for i in 0..hand.len() {
            for j in (i + 1)..hand.len() {
                options.push(PendingOption {
                    id: format!("respond:{},{}", hand[i], hand[j]),
                    label: format!(
                        "丈八蛇矛：将【{}】【{}】当【杀】打出",
                        turn::card_label(st, &hand[i]),
                        turn::card_label(st, &hand[j])
                    ),
                });
            }
        }
    }
    // 转化技（P4.1）：武圣（红色牌当杀）/ 龙胆（杀闪互转）/ 倾国（黑色牌当闪）。
    for id in &st.seats[f.actor].hand {
        if let Some((_, name)) = crate::skills::conversion_for(st, f.actor, id, code) {
            options.push(PendingOption {
                id: format!("respond:{id}"),
                label: format!(
                    "{name}：将【{}】当【{}】打出",
                    turn::card_label(st, id),
                    needed_name(code)
                ),
            });
        }
    }
    options.push(pending_pass());
    options
}

fn step_respond(st: &mut SgsState, f: RespondFrame) -> Result<bool, EngineError> {
    // 主公技（激将/护驾）由同势力角色代替响应：结果通过 outcome 回传
    if let Some(Outcome::Respond { played, played_by }) = st.outcome.take() {
        match played {
            Some(card) if !card.is_empty() => {
                let by = played_by.unwrap_or(f.actor);
                finish_respond_play(st, &f, &card, by, None)?;
            }
            _ => {
                pop(st);
            }
        }
        return Ok(false);
    }
    // 铁骑：目标不能使用【闪】，直接放弃响应（伤害照常结算）。
    if f.no_shan {
        respond_give_up(st, &f);
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
        && !f.bagua_used
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
    // 无手牌且无装备/技能替代选项（也无主公技接力）→ 公共信息上已无牌可打，自动放弃询问。
    // 有手牌时即使没有对应牌仍显式询问（保留犹豫博弈，不代选）；2026-10-28 用户裁定。
    if st.seats[f.actor].hand.is_empty() && options.len() == 1 {
        respond_give_up(st, &f);
        return Ok(false);
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
        let seat = st.seat_index(pid);
        // 无手牌者不可能出【桃】（酒未实现），直接跳过（公共信息，无需询问）。
        let empty_hand = seat.is_some_and(|i| st.seats[i].hand.is_empty());
        if alive_of_order(st, pid) && !f.passed.contains(pid) && !empty_hand {
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
    // 酒（P4.3）：濒死时只能对自己使用（回复 1 点体力，无救援加成）。
    if f.victim == seat {
        for id in turn::hand_cards_with_def(st, seat, "jiu") {
            options.push(PendingOption {
                id: format!("respond:{id}"),
                label: format!("使用【{}】自救", turn::card_label(st, &id)),
            });
        }
    }
    // 急救（P4.1）：回合外可将红色牌当【桃】使用。
    for id in &st.seats[seat].hand {
        if let Some((_, name)) = crate::skills::conversion_for(st, seat, id, "tao") {
            options.push(PendingOption {
                id: format!("respond:{id}"),
                label: format!("{name}：将【{}】当【桃】使用", turn::card_label(st, id)),
            });
        }
    }
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

fn step_huogong(st: &mut SgsState, f: HuoGongFrame) -> Result<bool, EngineError> {
    let suit = st.cards.get(&f.revealed).map(|c| c.suit);
    let label = turn::card_label(st, &f.revealed);
    let revealed_name = turn::card_label(st, &f.revealed);
    let target_name = st.seats[f.target].general_name.clone();
    st.push_log(None, format!("火攻：{target_name} 亮出【{revealed_name}】"));
    let mut options: Vec<PendingOption> = st.seats[f.user]
        .hand
        .iter()
        .filter(|id| st.cards.get(*id).map(|c| c.suit) == suit)
        .map(|id| PendingOption {
            id: format!("choose:{id}"),
            label: format!(
                "弃置同花色【{}】，造成 1 点火焰伤害",
                turn::card_label(st, id)
            ),
        })
        .collect();
    let _ = label;
    if options.is_empty() {
        pop(st);
        st.outcome = Some(Outcome::Picked { card: None });
        return Ok(false);
    }
    options.push(PendingOption {
        id: "pass".into(),
        label: "不弃置（不造成伤害）".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.user].player_id.clone(),
        skill: Some("huogong".into()),
        options,
        data: json!({ "skill": "huogong", "target": f.target, "revealed": f.revealed }),
    });
    replace_top(st, Frame::HuoGong(f));
    Ok(true)
}

pub(crate) fn act_huogong(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::HuoGong(f)) => f,
        _ => return Err(internal("栈顶不是火攻帧")),
    };
    if action == "pass" {
        pop(st);
        st.outcome = Some(Outcome::Picked { card: None });
        return Ok("火攻：不弃置，未造成伤害".to_string());
    }
    let card_id = action
        .strip_prefix("choose:")
        .ok_or_else(|| illegal("需要选择同花色手牌或放弃"))?;
    if !pending
        .options
        .iter()
        .any(|o| o.id == format!("choose:{card_id}"))
    {
        return Err(illegal("该牌不可用"));
    }
    turn::take_card(st, f.user, card_id)?;
    turn::to_discard_card(st, card_id);
    let label = turn::card_label(st, card_id);
    let pid = st.seats[f.user].player_id.clone();
    st.push_log(
        Some(&pid),
        format!("火攻：弃置同花色【{label}】，造成 1 点火焰伤害"),
    );
    pop(st);
    st.outcome = Some(Outcome::Picked {
        card: Some(card_id.to_string()),
    });
    apply_damage(st, f.target, 1, Some(f.user), DamageType::Fire, None);
    Ok("火攻：目标受到 1 点火焰伤害".to_string())
}

fn step_quick_judge(
    st: &mut SgsState,
    mut f: QuickJudgeFrame,
    events: &mut Vec<EngineEvent>,
) -> Result<bool, EngineError> {
    match f.stage {
        QuickJudgeStage::Reveal => {
            if f.judge_card.is_none() {
                f.saved_outcome = st.outcome.take();
                let id =
                    turn::draw_one(st, events).ok_or_else(|| internal("牌堆不可用，无法判定"))?;
                st.deck.processing.push(id.clone());
                f.judge_card = Some(id);
                f.rewrite_order = rewrite_order(st, f.owner);
                f.rewrite_cursor = 0;
            }
            f.stage = QuickJudgeStage::Rewrite;
            replace_top(st, Frame::QuickJudge(f));
            Ok(false)
        }
        QuickJudgeStage::Rewrite => {
            let Some(seat) = f.rewrite_order.get(f.rewrite_cursor).copied() else {
                f.stage = QuickJudgeStage::Effect;
                replace_top(st, Frame::QuickJudge(f));
                return Ok(false);
            };
            let options = rewrite_options(st, seat);
            if options.is_empty() {
                f.rewrite_cursor += 1;
                replace_top(st, Frame::QuickJudge(f));
                return Ok(false);
            }
            let actor = st.seats[seat].player_id.clone();
            let judge_view = f
                .judge_card
                .as_deref()
                .map(|id| crate::view::card_view(st, id))
                .unwrap_or(serde_json::Value::Null);
            st.pending = Some(Pending {
                kind: "judge_rewrite".into(),
                actor,
                skill: Some("guicai".into()),
                options,
                data: json!({
                    "judge_card": judge_view,
                    "reason": f.reason,
                    "owner": st.seats[f.owner].player_id,
                    "interaction": { "kind": "one", "commit": "respond" },
                }),
            });
            f.stage = QuickJudgeStage::WaitRewrite;
            replace_top(st, Frame::QuickJudge(f));
            Ok(true)
        }
        QuickJudgeStage::WaitRewrite => match st.outcome.take() {
            Some(Outcome::JudgeRewritten { card }) => {
                if let Some(new_card) = card {
                    if let Some(old) = f.judge_card.replace(new_card) {
                        discard_from_processing(st, &old);
                    }
                }
                f.rewrite_cursor += 1;
                f.stage = QuickJudgeStage::Rewrite;
                replace_top(st, Frame::QuickJudge(f));
                Ok(false)
            }
            _ => Err(internal("缺少改判结果")),
        },
        QuickJudgeStage::Effect => {
            // 归还进入判定前的父帧 outcome（例如【杀】的响应结果）。
            st.outcome = f.saved_outcome.clone();
            let judge_id = f.judge_card.clone().ok_or_else(|| internal("缺少判定牌"))?;
            let suit = st.cards.get(&judge_id).map(|c| c.suit);
            let label = turn::card_label(st, &judge_id);
            let black = suit.map(crate::skills::is_black).unwrap_or(false);
            pop(st);
            // 洛神：黑色判定牌由技能获得；其余判定牌正常进弃牌堆。
            if f.reason == "luoshen" && black {
                leave_processing(st, &judge_id);
                st.seats[f.owner].hand.push(judge_id.clone());
                let over = st.seats[f.owner]
                    .hand
                    .len()
                    .saturating_sub(st.config.max_hand_cards as usize);
                if over > 0 {
                    push_discard(st, f.owner, over, "hand_cap");
                }
            } else {
                discard_from_processing(st, &judge_id);
            }
            let pid = st.seats[f.owner].player_id.clone();
            match f.reason.as_str() {
                "tieqi" => {
                    let red = matches!(suit, Some(Suit::Heart) | Some(Suit::Diamond));
                    st.push_log(
                        Some(&pid),
                        format!(
                            "铁骑：判定【{label}】，{}",
                            if red {
                                "红色，目标不能使用【闪】"
                            } else {
                                "黑色，未生效"
                            }
                        ),
                    );
                    let user = f.data["user"].as_u64().unwrap_or(f.owner as u64) as usize;
                    let target = f.data["target"].as_u64().unwrap_or(0) as usize;
                    let card = f.data["card"].as_str().unwrap_or_default().to_string();
                    let need = f.data["need"].as_u64().unwrap_or(1) as u32;
                    let ignore_armor = f.data["ignore_armor"].as_bool().unwrap_or(false);
                    crate::resolve_skill::push_sha_respond(
                        st,
                        user,
                        target,
                        &card,
                        need,
                        red,
                        ignore_armor,
                    );
                }
                "luoshen" => {
                    if black {
                        st.push_log(Some(&pid), format!("洛神：判定【{label}】黑色，获得该牌"));
                        push(
                            st,
                            Frame::Luoshen(crate::resolve_more::LuoshenFrame { actor: f.owner }),
                        );
                    } else {
                        st.push_log(Some(&pid), format!("洛神：判定【{label}】红色，洛神结束"));
                    }
                }
                "bagua" => {
                    let red = matches!(suit, Some(Suit::Heart) | Some(Suit::Diamond));
                    st.push_log(
                        Some(&pid),
                        format!(
                            "八卦阵判定：【{label}】，{}",
                            if red {
                                "视为出【闪】"
                            } else {
                                "未生效"
                            }
                        ),
                    );
                    if red {
                        if let Some(Frame::Respond(rf)) = top(st).cloned() {
                            finish_respond_dodge(st, &rf);
                        }
                    }
                }
                "ganglie" => {
                    let victim = f.data["victim"].as_u64().unwrap_or(f.owner as u64) as usize;
                    let source = f.data["source"].as_u64().unwrap_or(0) as usize;
                    if suit == Some(Suit::Heart) {
                        st.push_log(Some(&pid), format!("刚烈：判定【{label}】红桃，未生效"));
                    } else {
                        st.push_log(
                            Some(&pid),
                            format!("刚烈：判定【{label}】非红桃，伤害来源需二选一"),
                        );
                        push(
                            st,
                            Frame::Ganglie(crate::resolve_more::GanglieFrame { victim, source }),
                        );
                    }
                }
                other => st.push_log(Some(&pid), format!("判定（{other}）：【{label}】")),
            }
            Ok(false)
        }
    }
}

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
                    if st.effective_def_code(&f.delayed) == Some("shandian") {
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
            if f.judge_card.is_none() {
                let judge_id = turn::reveal_judge_card(st, &f.delayed, f.owner, events)?;
                f.judge_card = Some(judge_id);
                f.rewrite_order = rewrite_order(st, f.owner);
                f.rewrite_cursor = 0;
            }
            f.stage = JudgeStage::Rewrite;
            replace_top(st, Frame::Judge(f));
            Ok(false)
        }
        JudgeStage::Rewrite => {
            let Some(seat) = f.rewrite_order.get(f.rewrite_cursor).copied() else {
                f.stage = JudgeStage::Effect;
                replace_top(st, Frame::Judge(f));
                return Ok(false);
            };
            let options = rewrite_options(st, seat);
            if options.is_empty() {
                // 手牌已空 → 没有可改判的牌，不询问。
                f.rewrite_cursor += 1;
                replace_top(st, Frame::Judge(f));
                return Ok(false);
            }
            let actor = st.seats[seat].player_id.clone();
            let judge_view = f
                .judge_card
                .as_deref()
                .map(|id| crate::view::card_view(st, id))
                .unwrap_or(serde_json::Value::Null);
            st.pending = Some(Pending {
                kind: "judge_rewrite".into(),
                actor,
                skill: Some("guicai".into()),
                options,
                data: json!({
                    "judge_card": judge_view,
                    "delayed": f.delayed,
                    "owner": st.seats[f.owner].player_id,
                    "interaction": { "kind": "one", "commit": "respond" },
                }),
            });
            f.stage = JudgeStage::WaitRewrite;
            replace_top(st, Frame::Judge(f));
            Ok(true)
        }
        JudgeStage::WaitRewrite => match st.outcome.take() {
            Some(Outcome::JudgeRewritten { card }) => {
                if let Some(new_card) = card {
                    let old = f.judge_card.replace(new_card.clone());
                    if let Some(old) = old {
                        // 被替换下的旧判定牌进弃牌堆（新牌已在处理区）。
                        discard_from_processing(st, &old);
                    }
                    turn::publish_judge_zone(
                        st,
                        &f.delayed,
                        f.owner,
                        &new_card,
                        "已改判（待结算）",
                    );
                }
                f.rewrite_cursor += 1;
                f.stage = JudgeStage::Rewrite;
                replace_top(st, Frame::Judge(f));
                Ok(false)
            }
            _ => Err(internal("缺少改判结果")),
        },
        JudgeStage::Effect => {
            // 先出栈，再结算（判定可能导致濒死压栈）
            let delayed = f.delayed.clone();
            let owner = f.owner;
            let judge_id = f.judge_card.clone().ok_or_else(|| internal("缺少判定牌"))?;
            pop(st);
            turn::apply_judge_effect(st, delayed, owner, judge_id, events)?;
            Ok(false)
        }
    }
}

// --- 改判窗口（P4.1：鬼才） -----------------------------------------------

/// 改判询问顺序：判定者左手侧起、判定者最后；只问存活且手牌非空的改判技能持有者。
fn rewrite_order(st: &SgsState, owner: usize) -> Vec<usize> {
    let mut order = st.seats_from_left_of(owner);
    order.push(owner);
    order
        .into_iter()
        .filter(|i| {
            st.seats.get(*i).is_some_and(|s| s.alive)
                && !st.seats[*i].hand.is_empty()
                && crate::skills::has(st, *i, "guicai")
        })
        .collect()
}

/// 改判可选牌（鬼才：任意一张手牌）+ `pass`。
fn rewrite_options(st: &SgsState, seat: usize) -> Vec<PendingOption> {
    let mut options: Vec<PendingOption> = st.seats[seat]
        .hand
        .iter()
        .map(|id| PendingOption {
            id: format!("respond:{id}"),
            label: format!("改判：{}", turn::card_label(st, id)),
        })
        .collect();
    options.push(PendingOption {
        id: "pass".into(),
        label: "不改判".into(),
    });
    options
}

fn act_judge_rewrite(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let judge_card = match top(st).cloned() {
        Some(Frame::Judge(f)) => f.judge_card,
        Some(Frame::QuickJudge(f)) => f.judge_card,
        _ => return Err(internal("栈顶不是判定帧")),
    };
    if action == "pass" {
        st.outcome = Some(Outcome::JudgeRewritten { card: None });
        return Ok("不改判".to_string());
    }
    let card_id = action
        .strip_prefix("respond:")
        .ok_or_else(|| illegal("需要打出改判牌或放弃"))?;
    if !pending
        .options
        .iter()
        .any(|o| o.id == format!("respond:{card_id}"))
    {
        return Err(illegal("该牌不可用于改判"));
    }
    let seat = st
        .seat_index(&pending.actor)
        .ok_or_else(|| internal("玩家不在座"))?;
    turn::take_card(st, seat, card_id)?;
    st.deck.processing.push(card_id.to_string());
    let before = judge_card
        .as_deref()
        .map(|id| turn::card_label(st, id))
        .unwrap_or_default();
    st.push_log(
        Some(&pending.actor),
        format!(
            "鬼才：将判定牌 {before} 改为 {}",
            turn::card_label(st, card_id)
        ),
    );
    st.outcome = Some(Outcome::JudgeRewritten {
        card: Some(card_id.to_string()),
    });
    Ok(format!("已改判为【{}】", turn::card_label(st, card_id)))
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
        "judge_rewrite" => act_judge_rewrite(st, &pending, action)?,
        "huogong" => act_huogong(st, &pending, action)?,
        "wugu" => crate::resolve_zone::act_wugu(st, &pending, action)?,
        "arrange" => crate::resolve_zone::act_arrange(st, action)?,
        "choice" => match pending.skill.as_deref() {
            Some("qilin") => crate::resolve_skill::act_qilin(st, &pending, action)?,
            Some("requisition") => crate::resolve_more::act_requisition(st, &pending, action)?,
            Some("tuxi") => crate::resolve_more::act_tuxi_offer(st, action)?,
            Some("luoyi") => crate::resolve_more::act_luoyi(st, action)?,
            Some("luoshen") => crate::resolve_more::act_luoshen(st, action)?,
            Some("fanjian") => crate::resolve_more::act_fanjian(st, action)?,
            Some("ganglie") => crate::resolve_more::act_ganglie(st, action)?,
            Some("liuli") => crate::resolve_more::act_liuli(st, action)?,
            Some("after_dodge") => crate::resolve_skill::act_after_dodge(st, action)?,
            Some("hanbing") => crate::resolve_skill::act_hanbing(st, action)?,
            Some("cixiong") => crate::resolve_skill::act_cixiong(st, action)?,
            Some("huogong") => act_huogong(st, &pending, action)?,
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
        // 支持方天画戟的 `targets:<p1,p2[,p3]>` 组合。
        let (target, extras) = if let Some(csv) = action.strip_prefix("targets:") {
            if !pending.options.iter().any(|o| o.id == action) {
                return Err(illegal("目标不合法"));
            }
            let seats: Vec<usize> = csv
                .split(',')
                .map(|pid| {
                    st.seat_index(pid.trim())
                        .ok_or_else(|| illegal("目标不在座"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            if seats.len() < 2 {
                return Err(illegal("方天画戟至少需要两个目标"));
            }
            (seats[0], seats[1..].to_vec())
        } else {
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
            (target, Vec::new())
        };
        f.pending_targets = extras;
        f.target = Some(target);
        f.stage = ShaStage::WaitRespond;
        replace_top(st, Frame::Sha(f.clone()));
        // 铁骑在 `push_sha_target_response` 内以 QuickJudge 处理；无双在此计算。
        let no_shan = false;
        // 无双：吕布使用【杀】时，目标需连续打出两张【闪】。
        let need = if crate::skills::has(st, f.user, "wushuang") {
            2
        } else {
            1
        };
        // 流离：成为【杀】目标时可弃一张手牌，将【杀】转移给攻击范围内的其他角色。
        let liuli = crate::skills::has(st, target, "liuli")
            && !st.seats[target].hand.is_empty()
            && st.alive_indices().into_iter().any(|v| {
                v != target
                    && v != f.user
                    && st.in_attack_range(target, v)
                    && !crate::skills::kongcheng_blocks(st, v)
            });
        if liuli {
            push(
                st,
                Frame::Liuli(crate::resolve_more::LiuliFrame {
                    user: f.user,
                    original: target,
                    card: f.card.clone(),
                    need,
                    no_shan,
                    stage: crate::resolve_more::LiuliStage::Choice,
                }),
            );
            return Ok("流离：请选择是否弃牌转移".to_string());
        }
        crate::resolve_skill::push_sha_target_response(
            st,
            f.user,
            target,
            &f.card,
            need,
            no_shan,
            crate::skills::has_equipment_skill(st, f.user, "qinggang_jian"),
            f.damage_type,
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

fn act_respond(st: &mut SgsState, pending: &Pending, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Respond(f)) => f,
        _ => return Err(internal("栈顶不是响应帧")),
    };
    let actor = f.actor;
    if action == "skill:bagua" {
        if f.kind != RespondKind::Shan {
            return Err(illegal("八卦阵只能用于需要【闪】时"));
        }
        if f.bagua_used {
            return Err(illegal("本次响应已经使用过【八卦阵】"));
        }
        if f.ignore_armor
            || !st
                .equip_def(actor, EquipSlot::Armor)
                .is_some_and(|d| d.code == "bagua_zhen")
        {
            return Err(illegal("没有可用的【八卦阵】"));
        }
        let mut used = f.clone();
        used.bagua_used = true;
        replace_top(st, Frame::Respond(used));
        // 八卦阵判定走通用 judge_before（可被鬼才改判）。
        push(
            st,
            Frame::QuickJudge(QuickJudgeFrame {
                owner: actor,
                reason: "bagua".into(),
                stage: QuickJudgeStage::Reveal,
                judge_card: None,
                rewrite_order: Vec::new(),
                rewrite_cursor: 0,
                data: json!({}),
                saved_outcome: None,
            }),
        );
        return Ok("八卦阵：判定中".to_string());
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
        respond_give_up(st, &f);
        return Ok("已放弃响应".to_string());
    }

    let card_id = action
        .strip_prefix("respond:")
        .ok_or_else(|| illegal("需要打出牌或 pass"))?;
    let needed = match f.kind {
        RespondKind::Shan => "shan",
        RespondKind::Nanman | RespondKind::Duel | RespondKind::Jiedao => "sha",
    };
    // 丈八蛇矛：respond:<c1>,<c2> 两张手牌当【杀】打出。
    if let Some((c1, c2)) = card_id.split_once(',') {
        if needed != "sha" {
            return Err(illegal("丈八蛇矛只能用于打出【杀】"));
        }
        if !crate::skills::has_equipment_skill(st, actor, "zhangba_shemao") {
            return Err(illegal("没有装备【丈八蛇矛】"));
        }
        if c1 == c2 {
            return Err(illegal("丈八蛇矛需要两张不同的手牌"));
        }
        for c in [c1, c2] {
            if !st.seats[actor].hand.iter().any(|h| h == c) {
                return Err(illegal("手牌中没有该牌"));
            }
        }
        if !pending
            .options
            .iter()
            .any(|o| o.id == format!("respond:{card_id}"))
        {
            return Err(illegal("该组合不可用"));
        }
        turn::take_card(st, actor, c2)?;
        turn::to_discard_card(st, c2);
        return finish_respond_play(st, &f, c1, actor, Some(("zhangba", "丈八蛇矛")));
    }
    let actual = turn::card_def(st, card_id).map(|d| d.code);
    let conversion = if actual == Some(needed) {
        None
    } else {
        // 转化技必须由引擎在 options 中下发，避免伪造。
        if !pending
            .options
            .iter()
            .any(|o| o.id == format!("respond:{card_id}"))
        {
            return Err(illegal("打出的牌不符合要求"));
        }
        Some(
            crate::skills::conversion_for(st, actor, card_id, needed)
                .ok_or_else(|| illegal("打出的牌不符合要求"))?,
        )
    };
    finish_respond_play(st, &f, card_id, actor, conversion)
}

/// 放弃响应（显式 `pass` 或「无手牌且无任何替代选项」的自动跳过共用）。
///
/// 先出栈再结算（伤害可能压入濒死帧）；剑/刀等「未出杀」后果也在这里处理。
/// 裸衣：本回合（摸牌阶段少摸 1 张后）使用【杀】/【决斗】造成的伤害 +1。
fn luoyi_bonus(st: &SgsState, source: Option<usize>) -> i32 {
    match source {
        Some(src)
            if st.turn == src
                && st.counter("luoyi") > 0
                && crate::skills::has(st, src, "luoyi") =>
        {
            1
        }
        _ => 0,
    }
}

fn respond_give_up(st: &mut SgsState, f: &RespondFrame) {
    let actor = f.actor;
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
            let mut amount = f.damage + luoyi_bonus(st, Some(f.source));
            // 古锭刀：目标无手牌时【杀】伤害 +1（物理【杀】）。
            if f.is_sha
                && crate::skills::has_equipment_skill(st, f.source, "guding_dao")
                && st.seats[actor].hand.is_empty()
            {
                amount += 1;
            }
            // 酒：本回合下一张【杀】伤害 +1（用后清零）。
            if f.is_sha && st.counter("jiu") > 0 {
                amount += 1;
                st.set_counter("jiu", 0);
            }
            // 白银狮子：受到的伤害改为 1。
            if crate::skills::has_equipment_skill(st, actor, "baiyin_shizi") && amount > 1 {
                amount = 1;
            }
            // 寒冰剑（damage_before）：可防止此伤害，改为弃置目标两张牌。
            if f.is_sha && crate::resolve_skill::hanbing_available_for(st, f.source, actor) {
                push(
                    st,
                    Frame::Hanbing(crate::resolve_skill::HanbingFrame {
                        user: f.source,
                        target: actor,
                        card,
                        damage: amount,
                    }),
                );
            } else {
                apply_damage(
                    st,
                    actor,
                    amount,
                    Some(f.source),
                    f.damage_type,
                    Some(&card),
                );
            }
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
            let amount = f.damage + luoyi_bonus(st, Some(opponent));
            apply_damage(
                st,
                actor,
                amount,
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
}

/// 八卦阵等"视为出闪"：不消耗手牌，通知父帧本次响应成功。
pub(crate) fn finish_respond_dodge(st: &mut SgsState, f: &RespondFrame) {
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
    conversion: Option<(&'static str, &'static str)>,
) -> Result<String, EngineError> {
    let expected = match f.kind {
        RespondKind::Shan => "shan",
        RespondKind::Nanman | RespondKind::Duel | RespondKind::Jiedao => "sha",
    };
    let actual = turn::card_def(st, card_id).map(|d| d.code);
    if actual != Some(expected) && conversion.is_none() {
        return Err(illegal("打出的牌不符合要求"));
    }
    let need = if f.need == 0 { 1 } else { f.need };
    turn::take_card(st, played_by, card_id)?;
    let label = turn::card_label(st, card_id);
    match conversion {
        Some((_, name)) => st.push_log(
            Some(&st.seats[played_by].player_id.clone()),
            format!("{name}：将【{label}】当【{}】打出", needed_name(expected)),
        ),
        None => st.push_log(
            Some(&st.seats[played_by].player_id.clone()),
            format!("打出【{label}】"),
        ),
    }
    match f.kind {
        RespondKind::Shan => {
            turn::to_discard_card(st, card_id);
            let played = f.played + 1;
            if played < need {
                // 无双：还需继续出【闪】。
                let mut nf = f.clone();
                nf.played = played;
                replace_top(st, Frame::Respond(nf));
                st.outcome = None;
                return Ok(format!("已打出【闪】，还需 {} 张", need - played));
            }
            if f.consume_card {
                discard_from_processing(st, &f.card);
            }
            st.outcome = Some(Outcome::Respond {
                played: Some(card_id.to_string()),
                played_by: Some(played_by),
            });
            pop(st);
            // 青龙偃月刀 / 贯石斧：被【闪】抵消后的追加窗口（压在原【杀】帧之上）。
            if crate::resolve_skill::push_after_dodge_if_available(st, &f) {
                return Ok("【闪】抵消，等待装备追加结算".to_string());
            }
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
            let played = f.played + 1;
            if played < need {
                // 无双：还需继续出【杀】。
                let mut nf = f.clone();
                nf.played = played;
                replace_top(st, Frame::Respond(nf));
                st.outcome = None;
                return Ok(format!("已打出【杀】，还需 {} 张", need - played));
            }
            let opponent = f.target.unwrap_or(f.source);
            // 无双：吕布发起决斗时，吕布以外的一方每次需连续两张【杀】。
            let need = match f.duel_initiator {
                Some(init) if opponent != init && crate::skills::has(st, init, "wushuang") => 2,
                _ => 1,
            };
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
                    bagua_used: false,
                    need,
                    played: 0,
                    no_shan: false,
                    duel_initiator: f.duel_initiator,
                    is_sha: false,
                }),
            );
            Ok("已打出【杀】，轮到对方".to_string())
        }
        RespondKind::Jiedao => {
            st.deck.processing.push(card_id.to_string());
            let b = f.target.unwrap_or(f.source);
            // 借刀虚拟杀不触发铁骑（简化；见 skills.md）。
            let no_shan = false;
            let need = if crate::skills::has(st, played_by, "wushuang") {
                2
            } else {
                1
            };
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
                    ignore_armor: crate::skills::has_equipment_skill(
                        st,
                        played_by,
                        "qinggang_jian",
                    ),
                    bagua_used: false,
                    need,
                    played: 0,
                    no_shan,
                    duel_initiator: None,
                    is_sha: true,
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
        // 枭姬：失去装备区的牌后摸 2 张。
        crate::skills::xiaoji_on_lose_equip(st, target);
        card
    } else if action == "pick:hand" {
        if st.seats[target].hand.is_empty() {
            return Err(illegal("目标没有手牌"));
        }
        let n = st.seats[target].hand.len();
        let idx = st.next_index(n);
        let card = st.seats[target].hand.remove(idx);
        // 连营：失去最后的手牌后摸 1 张。
        crate::skills::lianying_on_lose_last(st, target);
        card
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
    match f.mode {
        PickMode::Shunshou => {
            st.seats[f.user].hand.push(picked.clone());
            st.push_log(
                Some(&st.seats[f.user].player_id.clone()),
                format!("顺手牵羊：获得【{picked_name}】"),
            );
        }
        PickMode::Fankui => {
            st.seats[f.user].hand.push(picked.clone());
            st.push_log(
                Some(&st.seats[f.user].player_id.clone()),
                format!("反馈：获得【{picked_name}】"),
            );
        }
        PickMode::Guohe => {
            to_discard(st, &picked);
            st.push_log(
                Some(&st.seats[f.user].player_id.clone()),
                format!("过河拆桥：弃置【{picked_name}】"),
            );
        }
    }
    pop(st);
    // 反馈压在其他结算帧之上：不能覆盖父帧需要的 outcome（如【闪】响应结果）。
    if f.mode != PickMode::Fankui {
        st.outcome = Some(Outcome::Picked {
            card: Some(picked.clone()),
        });
    }
    if f.mode != PickMode::Guohe {
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
    let actual = turn::card_def(st, card_id).map(|d| d.code);
    // 酒自救：只允许对自己、且不享受救援加成。
    if actual == Some("jiu") {
        if f.victim != seat {
            return Err(illegal("【酒】只能在濒死时对自己使用"));
        }
        turn::take_card(st, seat, card_id)?;
        turn::to_discard_card(st, card_id);
        let victim = f.victim;
        st.seats[victim].hp += 1;
        let hp = st.seats[victim].hp;
        st.push_log(
            Some(&pending.actor.clone()),
            format!("使用【酒】自救（体力 {hp}）"),
        );
        if hp > 0 {
            st.phase = Phase::Turn;
            pop(st);
            return Ok(format!("{} 脱离濒死", st.seats[victim].general_name));
        }
        f.passed.clear();
        replace_top(st, Frame::Dying(f));
        return Ok(format!(
            "{} 仍濒死（体力 {hp}）",
            st.seats[victim].general_name
        ));
    }
    let conversion = if actual == Some("tao") {
        None
    } else {
        Some(
            crate::skills::conversion_for(st, seat, card_id, "tao")
                .ok_or_else(|| illegal("只能用【桃】救援"))?,
        )
    };
    turn::take_card(st, seat, card_id)?;
    turn::to_discard_card(st, card_id);
    let victim = f.victim;
    let bonus = crate::skills::tao_bonus(st, seat, victim);
    st.seats[victim].hp += 1 + bonus;
    let hp = st.seats[victim].hp;
    let label = turn::card_label(st, card_id);
    let used = match conversion {
        Some((_, name)) => format!("{name}：将【{label}】当【桃】"),
        None => format!("使用【{label}】"),
    };
    st.push_log(
        Some(&pending.actor.clone()),
        format!(
            "{used}救援 {}（体力 {hp}{}）",
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
        // 逐张弃时只提示当前动作，不写「还需弃 N 张」的中间日志（历史更干净）。
        format!("弃置 {} 张", ids.len())
    };
    if remaining == 0 {
        st.push_log(Some(&pending.actor.clone()), text.clone());
    }
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
    mut amount: i32,
    source: Option<usize>,
    dtype: DamageType,
    card: Option<&str>,
) {
    // 藤甲：火焰伤害 +1；普通【杀】/南蛮/万箭伤害无效。
    if crate::skills::has_equipment_skill(st, victim, "tengjia") {
        let code = card
            .and_then(|id| st.cards.get(id))
            .map(|c| c.def.as_str())
            .unwrap_or("");
        match dtype {
            DamageType::Fire => amount += 1,
            DamageType::Normal => {
                if matches!(code, "sha" | "nanman_ruqin" | "wanjian_qifa") {
                    let name = if code == "sha" {
                        "【杀】"
                    } else {
                        "该伤害"
                    };
                    let pid = st.seats[victim].player_id.clone();
                    st.push_log(Some(&pid), format!("藤甲：{name}的普通伤害无效"));
                    amount = 0;
                }
            }
            DamageType::Thunder => {}
        }
    }
    if amount <= 0 {
        if amount == 0 {
            return;
        }
    }
    // 铁索连环：属性伤害在同横置角色间传导（先全部重置，避免级联）。
    let chain_targets: Vec<usize> = if amount > 0
        && matches!(dtype, DamageType::Fire | DamageType::Thunder)
        && st.seats[victim].chained
    {
        let targets: Vec<usize> = st
            .alive_indices()
            .into_iter()
            .filter(|i| *i != victim && st.seats[*i].chained)
            .collect();
        for s in st.seats.iter_mut().filter(|s| s.chained) {
            s.chained = false;
        }
        targets
    } else {
        Vec::new()
    };
    if !chain_targets.is_empty() {
        let pid = st.seats[victim].player_id.clone();
        st.push_log(
            Some(&pid),
            format!(
                "铁索连环：属性伤害传导至 {} 名横置角色",
                chain_targets.len()
            ),
        );
        // 传导（已全部重置，不会级联）。
        for t in chain_targets {
            apply_damage(st, t, amount, source, dtype, None);
        }
    }
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
    // 刚烈：受到伤害后（存活时）判定，非红桃则来源二选一（弃 2 手牌 / 受 1 点伤害）。
    if let Some(src) = source {
        if src != victim && st.seats[victim].hp > 0 && crate::skills::has(st, victim, "ganglie") {
            push(
                st,
                Frame::QuickJudge(QuickJudgeFrame {
                    owner: victim,
                    reason: "ganglie".into(),
                    stage: QuickJudgeStage::Reveal,
                    judge_card: None,
                    rewrite_order: Vec::new(),
                    rewrite_cursor: 0,
                    data: json!({ "victim": victim, "source": src }),
                    saved_outcome: None,
                }),
            );
        }
    }
    // 反馈：受到伤害后获得伤害来源的一张牌（存活时；不含判定区）。
    if let Some(src) = source {
        if src != victim
            && st.seats[victim].hp > 0
            && crate::skills::has(st, victim, "fankui")
            && turn::has_cards_to_take(st, src)
        {
            push(
                st,
                Frame::PickCard(PickCardFrame {
                    card: "fankui".into(),
                    user: victim,
                    target: src,
                    mode: PickMode::Fankui,
                }),
            );
        }
    }
    // 遗计：受到伤害后（存活时）摸 2 张并逐张分配给任意角色。
    // 口径：按「每次伤害结算」触发一次（多伤事件只触发一次，待 P4.2 damage_before 统一改为按点触发）。
    if st.seats[victim].hp > 0 && crate::skills::has(st, victim, "yiji") {
        let mut ev = Vec::new();
        let drawn = turn::draw_cards(st, victim, 2, &mut ev);
        if drawn > 0 {
            let cards: Vec<String> = st.seats[victim]
                .hand
                .iter()
                .rev()
                .take(drawn)
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            let pid = st.seats[victim].player_id.clone();
            st.push_log(Some(&pid), format!("遗计：摸 {drawn} 张并分配"));
            push(
                st,
                Frame::Yiji(crate::resolve_more::YijiFrame {
                    actor: victim,
                    cards,
                }),
            );
        }
    }
    // 麒麟弓：使用【杀】造成伤害后，攻击方可弃置目标一匹坐骑（压在奸雄之上先问）。
    if let (Some(card_id), Some(src)) = (card, source) {
        let is_sha = st.cards.get(card_id).map(|c| c.def.as_str()) == Some("sha");
        if st.seats[victim].hp > 0
            && is_sha
            && src != victim
            && crate::skills::has_equipment_skill(st, src, "qilin_gong")
            && crate::resolve_skill::qilin_horses(st, victim).len() > 0
        {
            push(
                st,
                Frame::Qilin(crate::resolve_skill::QilinFrame {
                    attacker: src,
                    target: victim,
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
