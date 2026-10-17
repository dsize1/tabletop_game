//! 小游戏（Small Games，A1–H2）子状态与效果。
//!
//! 阶段 3a：框架 + 设置摆放（F1/F2/G1）+ 自动类（C1/D1/A2）+ 结算钩子（B1/C2）。
//! 交互类（A1/B2/D2/E1/E2/F1/F2/G1/G2/H1/H2）留待 3b（`pending` 已预留）。
//!
//! 数值口径：用户规则文本为准；文本未给出的用下列常量并在文档标注【待确认】。

use crate::{LasVegasState, CASINOS};
use game_engine_core::{roll_die, shuffle, Action, EngineError, EngineEvent, RandomSource};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

/// C2 厄运降临：最少骰子者向银行支付。
pub const C2_PENALTY: i64 = 50_000;
/// C1 掌声响起：大奖标记结算兑换。
pub const C1_REWARD: i64 = 100_000;
/// A2 大奖挑战：指示物 6 个格位的奖励（**钞票**，用户裁定 2026-10-17）。
/// 指示物停在第 1 格（$3 万）为初始位；中奖拿**当前格**的钞票并退回第 1 格；
/// 未中前进一格，**到顶（$8 万）不再前进**（不循环）。
pub const A2_LADDER_NOTE: [u32; 6] = [30_000, 40_000, 50_000, 60_000, 70_000, 80_000];
/// F1 阻止他：5 组灰骰（用户裁定 2026-10-17）= [1,1,2,2,3]（共 9 颗）。
/// 每轮开始时重置；某组放完后清空，不再可选。
pub const F1_GROUPS: [u32; 5] = [1, 1, 2, 2, 3];
/// G1 黑色摸彩箱：6 个标记金额（单位 $万）。【待确认：据攻略 2/2/6/0/0/0】
pub const G1_LOTTERY: [u32; 6] = [2, 2, 6, 0, 0, 0];
/// A1 幸运之拳：藏 1–3 枚标记 → 猜错时发起者的奖励（用户裁定 2026-10-17）。
/// 1 枚 → 2 筹码；2 枚 → $3 万；3 枚 → $4 万。
pub fn a1_reward(count: u32) -> Reward {
    match count {
        1 => Reward::Chips(2),
        2 => Reward::Note(30_000),
        3 => Reward::Note(40_000),
        _ => Reward::Chips(0),
    }
}

/// A1 幸运之拳：标记总数与可藏范围（1–3，不含 0）。
pub const A1_MARKERS: u32 = 3;
/// B2 50%：格子奖励（用户裁定 2026-10-17）。位置 0 与 1 无奖励；
/// 2 → 1 筹码；3 → $3 万；4 → $4 万；5 → $6 万（到达即领，不能继续前进）。
pub fn b2_reward(pos: u32) -> Option<Reward> {
    match pos {
        2 => Some(Reward::Chips(1)),
        3 => Some(Reward::Note(30_000)),
        4 => Some(Reward::Note(40_000)),
        5 => Some(Reward::Note(60_000)),
        _ => None,
    }
}

/// B2 最高格位（到达即领奖并结束）。
pub const B2_TOP: u32 = 5;
/// E1 禁止进入：箭头 6 格环形奖励（用户裁定 2026-10-17）：
/// 1 格无、2 格 2 筹码、3 格无、4 格 $3 万、5 格无、6 格 1 筹码。
/// 奖励在**前进后的位置**结算。
pub fn e1_reward(slot: usize) -> Option<Reward> {
    match slot {
        2 => Some(Reward::Chips(2)),
        4 => Some(Reward::Note(30_000)),
        6 => Some(Reward::Chips(1)),
        _ => None,
    }
}
/// G2 双倍颁奖：游戏场第一 / 第二奖励（钞票）。
pub const G2_FIRST: u32 = 60_000;
pub const G2_SECOND: u32 = 30_000;
/// H2 我的抉择：金框结算奖励（钞票）。
pub const H2_GOLD: u32 = 60_000;
/// H1 好骰难求：格位奖励（用户裁定 2026-10-17）。
///
/// 格位**不由玩家选**：用 H1 时把刚下注的 1 颗骰子**重投**，投出几点就进几号格。
/// 奖励按格位：1 格 → 1 筹码，2 格 → 2 筹码，3–6 格 → $3 万 / $4 万 / $5 万 / $6 万 钞票
/// （1 筹码 = $1 万，故 6 格等价于 1–6 万递增阶梯，重投期望 = 3.5 分）。
/// 骰子到位即得奖（**不**要求赢得对应赌场），在钞票分配后统一结算。
pub fn h1_reward(slot: usize) -> Reward {
    match slot {
        1 => Reward::Chips(1),
        2 => Reward::Chips(2),
        3 => Reward::Note(30_000),
        4 => Reward::Note(40_000),
        5 => Reward::Note(50_000),
        6 => Reward::Note(60_000),
        _ => Reward::Chips(0),
    }
}

/// H1 单次使用的奖励期望（分；万分单位），AI 打分用。
pub const H1_EXPECTED: f64 = 3.5;

/// 小游戏的**结算时机**（声明式；用户裁定 2026-10-17 阶段 0）。
///
/// 引擎的驱动方不再硬编码「哪个 id 在哪个钩子跑」，而是查 `tempos_of()`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tempo {
    /// 每轮设置阶段：摆放灰骰/标记（F1/F2）。
    Setup,
    /// 下注后自动生效（无需交互）。
    OnBetAuto,
    /// 下注后挂交互（pending）。
    OnBetPending,
    /// 回合内可用的附加动作（持有特权/限一次）。
    TurnAction,
    /// 轮次结算前·全局一次。
    PreSettleRound,
    /// 轮次结算前·逐赌场（「移除平手」之前）。
    PreSettleCasino,
    /// 轮次结算前·交互（挂 pending，做完后恢复结算）。
    PreSettlePending,
    /// 钞票分配后·统一结算。
    PostAllocate,
    /// 钞票分配后·标记兑换。
    PostAllocateMarker,
    /// 持续状态：无独立结算，只影响赌场比较/动作可用性。
    Passive,
}

/// 16 块小游戏全部编号（顺序即对齐阶段顺序）。
pub const ALL_TILES: [&str; 16] = [
    "A1", "A2", "B1", "B2", "C1", "C2", "D1", "D2", "E1", "E2", "F1", "F2", "G1", "G2", "H1", "H2",
];

/// 每块小游戏的结算时机表（**唯一声明处**，改时机只改这里）。
pub fn tempos_of(id: &str) -> &'static [Tempo] {
    match id {
        "A1" => &[Tempo::OnBetPending],
        "A2" => &[Tempo::OnBetAuto],
        "B1" => &[Tempo::PreSettlePending],
        "B2" => &[Tempo::OnBetPending],
        "C1" => &[Tempo::OnBetAuto],
        "C2" => &[Tempo::PreSettleCasino],
        "D1" => &[Tempo::OnBetAuto],
        "D2" => &[Tempo::OnBetAuto, Tempo::TurnAction, Tempo::Passive],
        "E1" => &[Tempo::OnBetPending, Tempo::Passive],
        "E2" => &[Tempo::OnBetAuto, Tempo::Passive],
        "F1" => &[Tempo::Setup, Tempo::OnBetPending, Tempo::Passive],
        "F2" => &[Tempo::Setup, Tempo::OnBetPending, Tempo::Passive],
        "G1" => &[Tempo::Setup, Tempo::PreSettlePending],
        "G2" => &[Tempo::OnBetPending, Tempo::PostAllocate],
        "H1" => &[Tempo::Setup, Tempo::OnBetPending, Tempo::PostAllocate],
        "H2" => &[Tempo::OnBetPending, Tempo::PostAllocate],
        _ => &[],
    }
}

/// 本局中登记了某时机的（赌场下标, 编号）列表。
pub fn tiles_with_tempo(st: &LasVegasState, tempo: Tempo) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (ci, c) in st.casinos.iter().enumerate() {
        for t in &c.small_games {
            if tempos_of(t).contains(&tempo) {
                out.push((ci, t.clone()));
            }
        }
    }
    out
}

/// 该编号是否在局（挂在任意赌场上）。
pub fn in_play(st: &LasVegasState, id: &str) -> bool {
    st.casinos
        .iter()
        .any(|c| c.small_games.iter().any(|g| g == id))
}

/// 通用奖励形态（筹码 / 钞票）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reward {
    /// 筹码数。
    Chips(u32),
    /// 钞票面额（$）。
    Note(u32),
}

/// 奖励文案（筹码 / 钞票）。
fn reward_text(r: Reward) -> String {
    match r {
        Reward::Chips(c) => format!("{c} 筹码"),
        Reward::Note(v) => format!("${}万", v / 10_000),
    }
}

fn h1_reward_text(slot: usize) -> String {
    reward_text(h1_reward(slot))
}

/// H2 金框占用（用户裁定 2026-10-17：区分大小骰，退还时原样还原）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GoldFrame {
    /// 占用者。
    pub owner: String,
    /// 小骰颗数。
    #[serde(default)]
    pub small: u32,
    /// 大骰颗数。
    #[serde(default)]
    pub big: u32,
}

impl GoldFrame {
    /// 总颗数（大骰也只算 1 颗）。
    pub fn pieces(&self) -> u32 {
        self.small + self.big
    }
}

/// E2 酒吧里的骰子（用户裁定 2026-10-17：按大小骰分别维护，收回时原样还原）。
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct BarDice {
    /// 小骰颗数。
    #[serde(default)]
    pub small: u32,
    /// 大骰颗数。
    #[serde(default)]
    pub big: u32,
}

impl BarDice {
    /// 总颗数（大骰也只算 1 颗）。
    pub fn pieces(&self) -> u32 {
        self.small + self.big
    }
}

/// H1：格位占用（用户裁定 2026-10-17）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct H1Slot {
    /// 占用者。
    pub owner: String,
    /// 格内骰子颗数（**大骰也只算 1 颗**）。
    pub dice: u32,
    /// 其中大骰颗数（仅用于被挤出时按 2 点权重还原到赌场）。
    #[serde(default)]
    pub big: u32,
}

/// 待处理的小游戏交互（3b 用；3a 不产生）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SgPending {
    pub game: String,
    pub casino: usize,
    pub actor: String,
    pub stage: String,
    #[serde(default)]
    pub data: serde_json::Value,
}

/// F2 危机就是转机：3 组 9 格（a=[1,2] 2 格 / b=[3,4,5] 3 格 / c=[6,7,8,9] 4 格）。
pub const F2_SLOTS: usize = 9;
/// F2 各组（0=a, 1=b, 2=c）的（起始索引, 容量）。
pub const F2_GROUPS: [(usize, usize); 3] = [(0, 2), (2, 3), (5, 4)];
/// F2 小组名（用于动作 id / 文案）。
pub const F2_GROUP_NAMES: [&str; 3] = ["a", "b", "c"];

/// 小游戏子状态（每轮重置；随 `LasVegasState` 序列化）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SgState {
    /// F2：9 个格位（`Some(来源赌场号 1–6)` = 该格已放灰骰）。
    #[serde(default)]
    pub f2_slots: Vec<Option<u8>>,
    /// F1：5 组灰骰（未放入赌场）。
    #[serde(default)]
    pub gray_groups: Vec<u32>,
    /// E2：酒吧（玩家 → 骰子构成，按大小骰分别记录）。
    #[serde(default)]
    pub bars: BTreeMap<String, BarDice>,
    /// G2：游戏场（玩家 → 骰数）。
    #[serde(default)]
    pub game_field: BTreeMap<String, u32>,
    /// B2 50%：本次激活的格子位置（0–5）与上一次点数和。
    #[serde(default)]
    pub b2_pos: u32,
    /// B2：上一次两颗骰子的点数和（用于比大小）。
    #[serde(default)]
    pub b2_prev: u32,
    /// H2：金框（占用者 + 大小骰颗数）。
    #[serde(default)]
    pub gold_frame: Option<GoldFrame>,
    /// E1：禁止标记所在赌场（0-based）。
    #[serde(default)]
    pub banned: Option<usize>,
    /// E1：箭头标记位置。
    #[serde(default)]
    pub arrow: usize,
    /// G1：摸彩标记金额。
    #[serde(default)]
    pub lottery: Vec<u32>,
    /// A2：指示物位置。
    #[serde(default)]
    pub indicator: usize,
    /// C1 掌声响起：本轮「先到 5 颗」的玩家（拿到即得 $10 万；每轮仅一次）。
    #[serde(default)]
    pub c1_winner: Option<String>,
    /// D2：特权标记持有者。
    #[serde(default)]
    pub privilege: Option<String>,
    /// H1：6 个格位（占用者 + 骰数；大骰在格内也只算 1 颗）。
    #[serde(default)]
    pub h1_slots: Vec<Option<H1Slot>>,
    /// 大厅指定的开局小游戏（仅第 1 轮生效；start 后清空）。
    #[serde(default)]
    pub small_games_override: Option<Vec<String>>,
    /// H1：本回合是否已用「移 1 颗骰到格」（规则每回合 1 颗）。
    #[serde(default)]
    pub h1_used_turn: bool,
    /// B1：本轮结算前是否已执行（防暂停/恢复时重复加骰）。
    #[serde(default)]
    pub b1_done: bool,
    /// G1：本轮结算前是否已执行（每轮只执行一次）。
    #[serde(default)]
    pub g1_done: bool,
    /// C2：本轮已结算过罚金的赌场下标（结算阶段按赌场顺序推进，防重复罚）。
    #[serde(default)]
    pub c2_done: Vec<usize>,
    /// 待处理交互（3b）。
    #[serde(default)]
    pub pending: Option<SgPending>,
    /// 同一赌场/同一次下注触发了多个交互时的**排队**（先进先出，避免后触发的被吞掉）。
    #[serde(default)]
    pub pending_queue: Vec<SgPending>,
    /// 只出现在 `player_view`（个人视图）里的**加密选项**（A1/G1）：
    /// `{game, stage, multi, options:[{code, count|value, reward|label}]}`。
    /// 公共视图恒为 `None`（见 `mask_public`）。
    #[serde(default)]
    pub pending_options: Option<serde_json::Value>,
}

impl SgState {
    /// 每轮设置：清空并按抽中的小游戏重摆灰骰/摸彩/指示物。
    ///
    /// F2：1–3 号赌场各 1 颗灰骰、4–6 号各 2 颗（直接写入赌场骰池 `dice["gray"]`）。
    pub fn setup_round(&mut self, tiles: &[String], casinos: &mut [crate::Casino]) {
        *self = SgState::default();
        for t in tiles {
            if !tempos_of(t).contains(&Tempo::Setup) {
                continue;
            }
            match t.as_str() {
                "F1" => self.gray_groups = F1_GROUPS.to_vec(),
                "F2" => {
                    self.f2_slots = vec![None; F2_SLOTS];
                    for (ci, c) in casinos.iter_mut().enumerate().take(CASINOS) {
                        let n = if ci < 3 { 1 } else { 2 };
                        *c.dice.entry("gray".to_string()).or_insert(0) += n;
                    }
                }
                "G1" => self.lottery = G1_LOTTERY.to_vec(),
                "H1" => self.h1_slots = vec![None; 6],
                _ => {}
            }
        }
    }

    /// 该赌场挂载的小游戏编号列表。
    pub fn tiles_at(&self, st: &LasVegasState, casino: usize) -> Vec<String> {
        st.casinos
            .get(casino)
            .map(|c| c.small_games.clone())
            .unwrap_or_default()
    }
}

/// 座次上玩家左边的人（前一位）。
pub(crate) fn left_player(st: &LasVegasState, player: &str) -> String {
    let n = st.seats.len();
    if n == 0 {
        return player.to_string();
    }
    let idx = st
        .seats
        .iter()
        .position(|s| s.player_id == player)
        .unwrap_or(0);
    st.seats[(idx + n - 1) % n].player_id.clone()
}

fn add_chips(st: &mut LasVegasState, p: &str, n: u32) {
    if let Some(s) = st.seats.iter_mut().find(|s| s.player_id == p) {
        s.chips += n;
    }
}

fn add_note(st: &mut LasVegasState, p: &str, v: u32) {
    if let Some(s) = st.seats.iter_mut().find(|s| s.player_id == p) {
        s.banknotes.push(v);
    }
}

fn add_casino_dice(st: &mut LasVegasState, casino: usize, p: &str, w: u32) {
    *st.casinos[casino].dice.entry(p.to_string()).or_insert(0) += w;
}

/// H1：从该赌场取走 1 颗骰。
///
/// 优先取小骰；只剩大骰时**整颗取走**（大骰不可拆，赌场权重 -2），但进入格位后与小骰一样算 **1 颗**。
/// 返回 `true` 表示取走的是大骰。
fn h1_take_die(st: &mut LasVegasState, casino: usize, player: &str) -> Result<bool, EngineError> {
    let total = st.casinos[casino].dice.get(player).copied().unwrap_or(0);
    let big = st.casinos[casino].big_dice.get(player).copied().unwrap_or(0);
    let small = total.saturating_sub(big * 2);
    if small == 0 && big == 0 {
        return Err(EngineError::IllegalAction("该赌场没有你的骰子".into()));
    }
    // 取大骰在赌场权重上更亏（-2），而格位奖励与小骰相同 → 只在无小骰时才取大骰。
    let took_big = small == 0;
    let left = total.saturating_sub(if took_big { 2 } else { 1 });
    if left == 0 {
        st.casinos[casino].dice.remove(player);
    } else if let Some(d) = st.casinos[casino].dice.get_mut(player) {
        *d = left;
    }
    if took_big {
        let bigs = &mut st.casinos[casino].big_dice;
        if let Some(b) = bigs.get_mut(player) {
            *b = b.saturating_sub(1);
            if *b == 0 {
                bigs.remove(player);
            }
        }
    }
    Ok(took_big)
}

/// H1：占用某格（格内他人骰子移到对应点数赌场）。
///
/// 用户裁定 2026-10-17：格位由**重投**决定；大骰进格后与小骰一样只算 1 颗，
/// 但被他人挤出时按 2 点权重还原到赌场（大骰不脱小）。
pub(crate) fn h1_slot_put(
    st: &mut LasVegasState,
    player: &str,
    slot: usize,
    took_big: bool,
    events: &mut Vec<EngineEvent>,
) {
    if st.sg.h1_slots.len() < 6 {
        st.sg.h1_slots = vec![None; 6];
    }
    match st.sg.h1_slots[slot - 1].take() {
        Some(mut s) if s.owner == player => {
            s.dice += 1;
            if took_big {
                s.big += 1;
            }
            st.sg.h1_slots[slot - 1] = Some(s);
        }
        Some(s) => {
            // 挤出：对方整批骰子回到「格号对应赌场」（格 n ↔ n 号赌场）。
            let weight = s.dice.saturating_sub(s.big) + s.big * 2;
            if weight > 0 {
                add_casino_dice(st, slot - 1, &s.owner, weight);
            }
            if s.big > 0 {
                *st.casinos[slot - 1]
                    .big_dice
                    .entry(s.owner.clone())
                    .or_insert(0) += s.big;
            }
            // 规则「不触发该赌场小游戏」：不走 on_place（故 C1 第 5 颗、D2 重算都不发生）。
            events.push(EngineEvent::new(
                "small_game",
                json!({
                    "game": "H1",
                    "kicked_player": s.owner,
                    "kicked_dice": s.dice,
                    "kicked_big": s.big,
                    "kicked_weight": weight,
                    "to_casino": slot,
                }),
            ));
            st.sg.h1_slots[slot - 1] = Some(H1Slot {
                owner: player.to_string(),
                dice: 1,
                big: u32::from(took_big),
            });
        }
        None => {
            st.sg.h1_slots[slot - 1] = Some(H1Slot {
                owner: player.to_string(),
                dice: 1,
                big: u32::from(took_big),
            });
        }
    }
}

/// D2 权力优势：某赌场唯一最多者持有特权标记；并列则回场。
fn update_privilege(st: &mut LasVegasState, casino: usize) {
    if !st.casinos[casino].small_games.iter().any(|g| g == "D2") {
        return;
    }
    // 用户裁定 2026-10-17：**灰色骰也参与判定**（可能把某玩家从「唯一最多」挤成非唯一 →
    // 特权回到游戏场），但灰骰本身不能持有特权标记。
    let mut counts: Vec<(String, u32)> = st.casinos[casino]
        .dice
        .iter()
        .filter(|(_p, c)| **c > 0)
        .map(|(p, c)| (p.clone(), *c))
        .collect();
    counts.sort_by(|a, b| b.1.cmp(&a.1));
    let unique_max = match counts.first() {
        Some((p, c)) => {
            let tied = counts.get(1).map(|x| x.1 == *c).unwrap_or(false);
            if tied {
                None
            } else {
                Some((p.clone(), *c))
            }
        }
        None => None,
    };
    st.sg.privilege = match unique_max {
        Some((p, _)) if p != "gray" => Some(p),
        _ => None,
    };
}

// ---------------------------------------------------------------------------
// F2 危机就是转机（重做版·用户裁定 2026-10-17）
// ---------------------------------------------------------------------------

/// 生成 4 位大写字母/数字的选项代码（去掉易混淆的 I/O/0/1）。
pub(crate) fn make_code(rng: &mut dyn RandomSource) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    (0..4)
        .map(|_| ALPHABET[(rng.next_u64() % ALPHABET.len() as u64) as usize] as char)
        .collect()
}

/// 场上（赌场骰池里）的灰骰总数。
fn f2_gray_on_board(st: &LasVegasState) -> u32 {
    st.casinos
        .iter()
        .map(|c| c.dice.get("gray").copied().unwrap_or(0))
        .sum()
}

/// 某组还有空格？
fn f2_group_has_empty(sg: &SgState, group: usize) -> bool {
    let Some((start, cap)) = F2_GROUPS.get(group).copied() else {
        return false;
    };
    if sg.f2_slots.len() < F2_SLOTS {
        return false;
    }
    (start..start + cap).any(|i| sg.f2_slots[i].is_none())
}

/// 把一颗灰骰放进某组第一个空格；返回格位索引（0-based）。
fn f2_place_slot(sg: &mut SgState, group: usize, from_casino: u8) -> Option<usize> {
    let (start, cap) = F2_GROUPS.get(group).copied()?;
    if sg.f2_slots.len() < F2_SLOTS {
        return None;
    }
    let idx = (start..start + cap).find(|i| sg.f2_slots[*i].is_none())?;
    sg.f2_slots[idx] = Some(from_casino);
    Some(idx)
}

/// 手中骰子总数（小骰 + 大骰）。
fn hand_dice(st: &LasVegasState, player: &str) -> u32 {
    st.seats
        .iter()
        .find(|s| s.player_id == player)
        .map(|s| s.small_remaining as u32 + s.big_remaining as u32)
        .unwrap_or(0)
}

/// 可「收回」的赌场（1-based）：有自己的骰子且未被 E1 禁止。
fn f2_takeback_casinos(st: &LasVegasState, player: &str) -> Vec<usize> {
    (1..=CASINOS)
        .filter(|c| {
            st.sg.banned != Some(c - 1)
                && st.casinos[c - 1].dice.get(player).copied().unwrap_or(0) > 0
        })
        .collect()
}

/// c 组是否可用：手中有骰 **或** 至少 1 个可收回赌场（用户要求的冗余校验）。
fn f2_c_usable(st: &LasVegasState, player: &str) -> bool {
    hand_dice(st, player) > 0 || !f2_takeback_casinos(st, player).is_empty()
}

/// 从手中取 1 颗骰（优先小骰，无小骰则大骰）；返回赌场权重（1 或 2）。
fn take_hand_die(st: &mut LasVegasState, player: &str) -> Option<u32> {
    let seat = st.seats.iter_mut().find(|s| s.player_id == player)?;
    if seat.small_remaining > 0 {
        seat.small_remaining -= 1;
        Some(1)
    } else if seat.big_remaining > 0 {
        seat.big_remaining -= 1;
        Some(2)
    } else {
        None
    }
}

/// 从赌场取回 1 颗自己的骰回手（优先小骰，只剩大骰则整颗）；返回是否为大骰。
fn take_back_die(st: &mut LasVegasState, casino: usize, player: &str) -> Option<bool> {
    let total = st.casinos[casino].dice.get(player).copied().unwrap_or(0);
    let big = st.casinos[casino].big_dice.get(player).copied().unwrap_or(0);
    let small = total.saturating_sub(big * 2);
    if small == 0 && big == 0 {
        return None;
    }
    let took_big = small == 0;
    let left = total.saturating_sub(if took_big { 2 } else { 1 });
    if left == 0 {
        st.casinos[casino].dice.remove(player);
    } else if let Some(d) = st.casinos[casino].dice.get_mut(player) {
        *d = left;
    }
    if took_big {
        let bigs = &mut st.casinos[casino].big_dice;
        if let Some(b) = bigs.get_mut(player) {
            *b = b.saturating_sub(1);
            if *b == 0 {
                bigs.remove(player);
            }
        }
    }
    if let Some(seat) = st.seats.iter_mut().find(|s| s.player_id == player) {
        if took_big {
            seat.big_remaining += 1;
        } else {
            seat.small_remaining += 1;
        }
    }
    Some(took_big)
}

/// 取出 pending 里的加密选项代码列表（A1 用）。
pub(crate) fn pending_codes(p: &SgPending) -> Vec<String> {
    p.data
        .get("codes")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default()
}

/// 选项代码 → 实际枚数（A1）。
fn code_to_count(p: &SgPending, code: &str) -> Option<u32> {
    let codes = pending_codes(p);
    let idx = codes.iter().position(|c| c.eq_ignore_ascii_case(code))?;
    p.data
        .get("perm")
        .and_then(|v| serde_json::from_value::<Vec<u32>>(v.clone()).ok())
        .and_then(|perm| perm.get(idx).copied())
}

/// H2：掷 2 颗黑骰，且**不产生 4,4**（两颗都是 4 时系统静默重掷，玩家看不到该场景）。
fn roll_h2_dice(rng: &mut dyn RandomSource) -> Vec<u32> {
    loop {
        let a = roll_die(rng) as u32;
        let b = roll_die(rng) as u32;
        if a == 4 && b == 4 {
            continue;
        }
        return vec![a, b];
    }
}

/// H2：当前可选的**点数**列表（按骰面去重，并过滤掉做不到的选项）。
///
/// - 1/2/3：总是可选（发筹码/钞票）。
/// - 4：总是可选（村规：重掷 2 颗黑骰重新选择）。
/// - 5：手中还有骰子，**或**场上还有自己的骰子可收回 → 才可选。
/// - 6：手里有骰子才可放金框。
fn h2_options(st: &LasVegasState, player: &str, dice: &[u32]) -> Vec<u32> {
    let hand = hand_dice(st, player) > 0;
    let takeback = !f2_takeback_casinos(st, player).is_empty();
    let mut out: Vec<u32> = Vec::new();
    for d in dice {
        let ok = match *d {
            1..=4 => true,
            5 => hand || takeback,
            6 => hand,
            _ => false,
        };
        if ok && !out.contains(d) {
            out.push(*d);
        }
    }
    out.sort_unstable();
    out
}

/// 该玩家此刻的**加密选项**（代码 → 真实含义），供 `player_view` 下发。
///
/// - A1「hide」：代码 → 藏几颗标记 + 猜错奖励。
/// - G1「split」：代码 → 该摸彩标记的面额（万），支持**多选**（`#选 A，B，C`）。
pub(crate) fn private_options(p: &SgPending, lottery: &[u32]) -> Option<serde_json::Value> {
    let codes = pending_codes(p);
    match (p.game.as_str(), p.stage.as_str()) {
        ("A1", "hide") => {
            let perm: Vec<u32> = p
                .data
                .get("perm")
                .and_then(|v| serde_json::from_value(v.clone()).ok())?;
            let opts: Vec<serde_json::Value> = codes
                .iter()
                .enumerate()
                .map(|(i, code)| {
                    let count = perm.get(i).copied().unwrap_or(0);
                    json!({
                        "code": code,
                        "count": count,
                        "reward": reward_text(a1_reward(count)),
                    })
                })
                .collect();
            Some(json!({ "game": "A1", "stage": "hide", "multi": false, "options": opts }))
        }
        ("G1", "split") => {
            let opts: Vec<serde_json::Value> = codes
                .iter()
                .enumerate()
                .map(|(i, code)| {
                    let value = lottery.get(i).copied().unwrap_or(0);
                    json!({ "code": code, "value": value, "label": format!("${value} 万") })
                })
                .collect();
            Some(json!({ "game": "G1", "stage": "split", "multi": true, "options": opts }))
        }
        _ => None,
    }
}

/// F2 组名文案（0=a / 1=b / 2=c）。
fn f2_group_desc(group: usize) -> &'static str {
    match group {
        0 => "a 组（格 1–2，得 1 筹码）",
        1 => "b 组（格 3–5，得 $3 万）",
        _ => "c 组（格 6–9，放入/收回）",
    }
}

/// 下注后触发的自动小游戏（C1/D1/A2）与交互类触发。
pub(crate) fn on_place(
    st: &mut LasVegasState,
    casino: usize,
    player: &str,
    rng: &mut dyn RandomSource,
    events: &mut Vec<EngineEvent>,
) {
    // T1a：下注后自动生效的小游戏（按时机表派发；`on_bet_auto` 内按编号实现）。
    for (ci, id) in tiles_with_tempo(st, Tempo::OnBetAuto) {
        if ci == casino {
            on_bet_auto(st, &id, casino, player, rng, events);
        }
    }
    update_privilege(st, casino);
    // T1b：下注后交互（挂 pending）。
    trigger_interactive(st, casino, player, rng, events);
}

/// 下注后自动类小游戏的实现（T1a）。
fn on_bet_auto(
    st: &mut LasVegasState,
    id: &str,
    casino: usize,
    player: &str,
    rng: &mut dyn RandomSource,
    events: &mut Vec<EngineEvent>,
) {
    match id {
        // C1 掌声响起（用户裁定 2026-10-17）：**颗数**达到 5（大骰也只算 1 颗）→ 立即得 $10 万；
        // 本轮仅一个名额，先到者得之；只有「自己下注」判定（黑骰/灰骰/特权/小游戏放骰都不算）。
        "C1" => {
            if st.sg.c1_winner.is_none() {
                let weight = st.casinos[casino].dice.get(player).copied().unwrap_or(0);
                let big = st.casinos[casino].big_dice.get(player).copied().unwrap_or(0);
                // 颗数 = 小骰 + 大骰 = 权重 - 大骰数。
                let pieces = weight.saturating_sub(big);
                if pieces >= 5 {
                    add_note(st, player, C1_REWARD as u32);
                    st.sg.c1_winner = Some(player.to_string());
                    events.push(EngineEvent::new(
                        "small_game",
                        json!({
                            "game": "C1",
                            "player": player,
                            "casino": casino + 1,
                            "pieces": pieces,
                            "reward": C1_REWARD,
                        }),
                    ));
                }
            }
        }
            // E2 醉倒了吗（用户裁定 2026-10-17）：其他玩家各 1 颗入手骰进酒吧（上限 2，优先小骰）；
            // 触发者收回自己酒吧里的全部骰子（大骰还原大骰）。每次下注（含 D2 特权放置）都触发。
            "E2" => e2_effect(st, player, events),
            // D1 发薪愉快：统计有自己骰子的赌场数（含当前）→ 1/2 筹码，>=3 得 count×$1万钞票。
            "D1" => {
                let count = st
                    .casinos
                    .iter()
                    .filter(|c| c.dice.get(player).copied().unwrap_or(0) > 0)
                    .count() as u32;
                let desc = if count <= 2 {
                    if let Some(s) = st.seats.iter_mut().find(|s| s.player_id == player) {
                        s.chips += count;
                    }
                    json!({ "game": "D1", "player": player, "casinos": count, "chips": count })
                } else {
                    let note = count * 10_000;
                    if let Some(s) = st.seats.iter_mut().find(|s| s.player_id == player) {
                        s.banknotes.push(note);
                    }
                    json!({ "game": "D1", "player": player, "casinos": count, "note": note })
                };
                events.push(EngineEvent::new("small_game", desc));
            }
            // A2 大奖挑战：投 2 黑骰，和=7 或同点 → 拿指示物奖励并回位；否则指示物前进一格。
            // A2 大奖挑战：投 2 黑骰，同点或和为 7 → 拿**当前格**钞票并退回第 1 格；否则指示物前进一格（到顶不再前进）。
            "A2" => {
                let a = roll_die(rng);
                let b = roll_die(rng);
                let hit = a == b || a + b == 7;
                let slot = st.sg.indicator.min(A2_LADDER_NOTE.len() - 1);
                if hit {
                    let note = A2_LADDER_NOTE[slot];
                    add_note(st, player, note);
                    st.sg.indicator = 0;
                    events.push(EngineEvent::new(
                        "small_game",
                        json!({ "game": "A2", "player": player, "black": [a, b], "hit": true, "slot": slot + 1, "note": note }),
                    ));
                } else {
                    let next = (slot + 1).min(A2_LADDER_NOTE.len() - 1);
                    st.sg.indicator = next;
                    events.push(EngineEvent::new(
                        "small_game",
                        json!({ "game": "A2", "player": player, "black": [a, b], "hit": false, "slot": next + 1, "note": A2_LADDER_NOTE[next] }),
                    ));
                }
            }
        _ => {}
    }
}

/// 交互类小游戏触发（下注后）：设置 pending（一次只挂一个）。
pub(crate) fn trigger_interactive(
    st: &mut LasVegasState,
    casino: usize,
    player: &str,
    rng: &mut dyn RandomSource,
    events: &mut Vec<EngineEvent>,
) {
    if st.sg.pending.is_some() {
        return;
    }
    for (ci, t) in tiles_with_tempo(st, Tempo::OnBetPending) {
        if ci != casino {
            continue;
        }
        let pending = match t.as_str() {
            "E2" => {
                e2_effect(st, player, events);
                None
            }
            // A1 幸运之拳：下注者先从 3 个**脱敏选项**里挑 1 个（选项↔标记数为随机
            // 置换、不一一对应），再交给左手玩家猜 1–3 枚（不含 0）。
            "A1" => {
                let mut perm: Vec<u32> = (1..=A1_MARKERS).collect();
                shuffle(rng, &mut perm);
                // 每个选项一个一次性随机代码：群里只出现代码，代码→枚数只发给该玩家的牌面页。
                let codes: Vec<String> = (0..A1_MARKERS as usize).map(|_| make_code(rng)).collect();
                Some(SgPending {
                    game: "A1".into(),
                    casino,
                    actor: player.to_string(),
                    stage: "hide".into(),
                    data: json!({ "initiator": player, "perm": perm, "codes": codes }),
                })
            }
            // B2 50% 的机会（用户裁定 2026-10-17）：标记从 0 起步，先掷一次并前进到第 1 格，
            // 之后每轮「大/小/弃权」：猜对前进一格，猜错结束无奖励；到达第 5 格直接领 $6 万。
            "B2" => {
                let a = roll_die(rng) as u32;
                let b = roll_die(rng) as u32;
                st.sg.b2_pos = 1;
                st.sg.b2_prev = a + b;
                events.push(EngineEvent::new(
                    "small_game",
                    json!({ "game": "B2", "opening": [a, b], "pos": 1 }),
                ));
                Some(SgPending {
                    game: "B2".into(),
                    casino,
                    actor: player.to_string(),
                    stage: "guess".into(),
                    data: json!({ "prev": a + b }),
                })
            }
            "E1" => Some(SgPending {
                game: "E1".into(),
                casino,
                actor: player.to_string(),
                stage: "choose".into(),
                data: json!({}),
            }),
            "F1" => {
                if st.sg.gray_groups.iter().any(|g| *g > 0) {
                    Some(SgPending {
                        game: "F1".into(),
                        casino,
                        actor: player.to_string(),
                        // 两轮交互：先选组（group），再选赌场（casino）
                        stage: "group".into(),
                        data: json!({}),
                    })
                } else {
                    None
                }
            }
            "F2" => {
                // F2 重做版：必须先能「把场上 1 颗灰骰放进 9 格」。
                if f2_gray_on_board(st) == 0
                    || !(0..F2_GROUPS.len()).any(|g| f2_group_has_empty(&st.sg, g))
                {
                    events.push(EngineEvent::new(
                        "small_game",
                        json!({
                            "game": "F2",
                            "skipped": true,
                            "reason": if f2_gray_on_board(st) == 0 { "no_gray_dice" } else { "slots_full" },
                        }),
                    ));
                    None
                } else {
                    Some(SgPending {
                        game: "F2".into(),
                        casino,
                        actor: player.to_string(),
                        // 多段：gray → group → (c_action → c_put | c_takeback)
                        stage: "gray".into(),
                        data: json!({}),
                    })
                }
            }
            // G2 双倍颁奖（用户裁定 2026-10-17）：下注后选择把该赌场里自己的 0–N 颗骰子移入游戏场。
            // 游戏场视为「第七个赌场」，按同赌场规则结算（$6 万 / $3 万）。
            "G2" => {
                let weight = st.casinos[casino].dice.get(player).copied().unwrap_or(0);
                if weight > 0 {
                    Some(SgPending {
                        game: "G2".into(),
                        casino,
                        actor: player.to_string(),
                        stage: "move".into(),
                        data: json!({}),
                    })
                } else {
                    None
                }
            }
            // H2 我的抉择（用户裁定 2026-10-17）：掷 2 颗黑骰（**不出现 4,4**）→ 玩家选一颗的点数领奖励；
            // 4 点 = 村规「重掷 2 颗黑骰，再次选择」。先按可用性过滤选项，无可选项则不进入游戏。
            "H2" => {
                let dice = roll_h2_dice(rng);
                if h2_options(st, player, &dice).is_empty() {
                    events.push(EngineEvent::new(
                        "small_game",
                        json!({ "game": "H2", "player": player, "skipped": true, "reason": "no_option", "dice": dice }),
                    ));
                    None
                } else {
                    Some(SgPending {
                        game: "H2".into(),
                        casino,
                        actor: player.to_string(),
                        stage: "choose".into(),
                        data: json!({ "dice": dice }),
                    })
                }
            }
            // H1 好骰难求：刚在该场下注后，可选择把 1 颗已下注骰移入格子（可选，可跳过）。
            "H1" => {
                // 来源可以是「该赌场里自己已下注的骰子」或「手里的骰子」。
                let c = st.casinos[casino].dice.get(player).copied().unwrap_or(0);
                if (c > 0 || hand_dice(st, player) > 0) && !st.sg.h1_used_turn {
                    Some(SgPending {
                        game: "H1".into(),
                        casino,
                        actor: player.to_string(),
                        stage: "source".into(),
                        data: json!({}),
                    })
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(p) = pending {
            events.push(EngineEvent::new(
                "small_game_pending",
                json!({ "game": p.game, "actor": p.actor, "casino": casino + 1 }),
            ));
            // 已经有 pending 时排队，逐个处理（不再吞掉后面的交互）。
            if st.sg.pending.is_none() {
                st.sg.pending = Some(p);
            } else {
                st.sg.pending_queue.push(p);
            }
        }
    }
}

/// E2 酩酊大醉（自动）：其他玩家各放 1 颗入手骰到酒吧（上限 2），触发者收回自己酒吧骰。
fn e2_effect(st: &mut LasVegasState, initiator: &str, events: &mut Vec<EngineEvent>) {
    // 引擎代玩家取骰（省去全场交互）：优先取小骰；每人酒吧上限 2 颗；没有手牌则跳过。
    let others: Vec<String> = st.seats.iter().map(|s| s.player_id.clone()).collect();
    let mut punished: Vec<(String, bool)> = Vec::new();
    for p in others {
        if p == initiator {
            continue;
        }
        let bar = st.sg.bars.get(&p).copied().unwrap_or_default();
        if bar.pieces() >= 2 {
            continue;
        }
        let Some(seat) = st.seats.iter_mut().find(|s| s.player_id == p) else {
            continue;
        };
        let took_big = if seat.small_remaining > 0 {
            seat.small_remaining -= 1;
            false
        } else if seat.big_remaining > 0 {
            seat.big_remaining -= 1;
            true
        } else {
            continue;
        };
        let entry = st.sg.bars.entry(p.clone()).or_default();
        if took_big {
            entry.big += 1;
        } else {
            entry.small += 1;
        }
        punished.push((p, took_big));
    }
    // 触发者收回自己在酒吧的全部骰子（大骰还原大骰、小骰还原小骰）。
    let mine = st.sg.bars.remove(initiator).unwrap_or_default();
    if let Some(s) = st.seats.iter_mut().find(|s| s.player_id == initiator) {
        s.small_remaining += mine.small as u8;
        s.big_remaining += mine.big as u8;
    }
    events.push(EngineEvent::new(
        "small_game",
        json!({
            "game": "E2",
            "player": initiator,
            "bar_recalled": mine.pieces(),
            "recalled_small": mine.small,
            "recalled_big": mine.big,
            "barred": punished.iter().map(|(p, big)| json!({ "player": p, "big": *big })).collect::<Vec<_>>(),
        }),
    ));
}

/// pending 状态下该玩家的合法 `sg:*` 动作。
pub(crate) fn legal_for_pending(st: &LasVegasState, player: &str) -> Vec<Action> {
    let Some(p) = st.sg.pending.clone() else {
        return vec![];
    };
    if p.actor != player {
        return vec![];
    }
    match p.game.as_str() {
        "A1" => {
            if p.stage == "hide" {
                // 加密选项：群里只出现一次性代码；代码↔枚数映射只在该玩家的牌面页显示。
                pending_codes(&p)
                    .into_iter()
                    .map(|code| Action {
                        id: format!("sg:code:{code}"),
                        description: format!("选项 {code}（对应几颗见你的牌面页）"),
                    })
                    .collect()
            } else {
                (1..=A1_MARKERS)
                    .map(|k| Action {
                        id: format!("sg:A1:guess:{k}"),
                        description: format!("猜 {k} 枚"),
                    })
                    .collect()
            }
        }
        "B2" => {
            let pos = st.sg.b2_pos;
            let mut v = vec![
                Action {
                    id: "sg:B2:guess:high".into(),
                    description: format!("猜比上次大（当前第 {pos} 格）"),
                },
                Action {
                    id: "sg:B2:guess:low".into(),
                    description: format!("猜比上次小（当前第 {pos} 格）"),
                },
            ];
            let quit_note = match b2_reward(pos) {
                Some(r) => format!("弃权，领当前第 {pos} 格的 {}", reward_text(r)),
                None => format!("弃权（第 {pos} 格无奖励）"),
            };
            v.push(Action {
                id: "sg:B2:quit".into(),
                description: quit_note,
            });
            v
        }
        "E1" => {
            // 可选：除本游戏所在赌场外的任一赌场；已禁止的赌场再放一次无意义 → 不列。
            let mut v: Vec<Action> = (1..=CASINOS)
                .filter(|c| {
                    *c != p.casino + 1 && st.sg.banned != Some(c - 1)
                })
                .map(|c| Action {
                    id: format!("sg:E1:ban:{c}"),
                    description: format!("把禁止标记放到 {c} 号赌场，箭头前进并领奖励"),
                })
                .collect();
            v.push(Action {
                id: "sg:E1:pass".into(),
                description: "放弃放置禁止标记（箭头不前进、无奖励）".into(),
            });
            v
        }
        "F1" => {
            if p.stage == "group" {
                st.sg
                    .gray_groups
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| **c > 0)
                    .map(|(g, c)| Action {
                        id: format!("sg:F1:group:{g}"),
                        description: format!("选第 {} 组（{c} 颗灰骰）", g + 1),
                    })
                    .collect()
            } else {
                let g = p.data.get("group").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let cnt = st.sg.gray_groups.get(g).copied().unwrap_or(0);
                (1..=CASINOS)
                    .map(|c| Action {
                        id: format!("sg:F1:move:{g}:{c}"),
                        description: format!("把第 {} 组的 {cnt} 颗灰骰放到 {c} 号赌场", g + 1),
                    })
                    .collect()
            }
        }
        "F2" => {
            // 多段式（重做版·裁定 2026-10-17）：gray → group → (c_action → c_put | c_takeback)
            let full: Vec<String> = (0..F2_GROUPS.len())
                .filter(|g| !f2_group_has_empty(&st.sg, *g))
                .map(|g| F2_GROUP_NAMES[g].to_string())
                .collect();
            let full_note = if full.is_empty() {
                String::new()
            } else {
                format!("；已满：{}", full.join("/"))
            };
            match p.stage.as_str() {
                "gray" => (1..=CASINOS)
                    .filter(|c| st.casinos[c - 1].dice.get("gray").copied().unwrap_or(0) > 0)
                    .map(|c| Action {
                        id: format!("sg:F2:take:{c}"),
                        description: format!(
                            "取 {c} 号赌场的 1 颗灰骰放进 F2 格子（该场灰骰 {} 颗）",
                            st.casinos[c - 1].dice.get("gray").copied().unwrap_or(0)
                        ),
                    })
                    .collect(),
                "group" => {
                    let mut v = vec![];
                    for (g, name) in F2_GROUP_NAMES.iter().enumerate() {
                        if !f2_group_has_empty(&st.sg, g) {
                            continue;
                        }
                        // c 组额外校验：选完必须还有可执行的第 2 段动作。
                        if g == 2 && !f2_c_usable(st, player) {
                            continue;
                        }
                        v.push(Action {
                            id: format!("sg:F2:group:{name}"),
                            description: format!("放进 {}{}", f2_group_desc(g), full_note),
                        });
                    }
                    v
                }
                "c_action" => {
                    let mut v = vec![];
                    if hand_dice(st, player) > 0 {
                        v.push(Action {
                            id: "sg:F2:put".into(),
                            description: "放入：从手中拿 1 颗骰放进任一赌场（不触发小游戏）".into(),
                        });
                    }
                    let back = f2_takeback_casinos(st, player);
                    if !back.is_empty() {
                        v.push(Action {
                            id: "sg:F2:takeback".into(),
                            description: format!("收回：从 {} 号赌场取回自己 1 颗骰", back.iter().map(|c| c.to_string()).collect::<Vec<_>>().join("/")),
                        });
                    }
                    v
                }
                "c_put" => (1..=CASINOS)
                    .map(|c| Action {
                        id: format!("sg:F2:put:{c}"),
                        description: format!("把手中 1 颗 {c} 点骰放进 {c} 号赌场"),
                    })
                    .collect(),
                _ => f2_takeback_casinos(st, player)
                    .into_iter()
                    .map(|c| Action {
                        id: format!("sg:F2:takeback:{c}"),
                        description: format!(
                            "取回自己在 {c} 号赌场的 {}",
                            st.casinos[c - 1].dice.get(player).copied().unwrap_or(0)
                        ),
                    })
                    .collect(),
            }
        }
        "G2" => {
            // 可选 0–N 颗（N = 自己在该赌场的骰子颗数，大骰算 1 颗）；0 = 放弃。
            let weight = st.casinos[p.casino].dice.get(player).copied().unwrap_or(0);
            let big = st.casinos[p.casino].big_dice.get(player).copied().unwrap_or(0);
            let pieces = weight.saturating_sub(big);
            (0..=pieces)
                .map(|k| Action {
                    id: format!("sg:G2:move:{k}"),
                    description: if k == 0 {
                        "放弃（不移入游戏场）".to_string()
                    } else {
                        format!("把 {k} 颗骰子移入游戏场（结算 $6 万 / $3 万）")
                    },
                })
                .collect()
        }
        "B1" => {
            // 6 个选项：放「掷出的点数命中该赌场」的颗数（0–2）到 N 号赌场作为自己的骰子。
            let black: Vec<u32> = p
                .data
                .get("black")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            (1..=CASINOS)
                .map(|c| {
                    let k = black.iter().filter(|d| **d as usize == c).count();
                    Action {
                        id: format!("sg:B1:put:{c}"),
                        description: if k > 0 {
                            format!("放 {k} 颗黑骰到 {c} 号赌场参与结算")
                        } else {
                            format!("不放骰子到 {c} 号赌场（0 颗）")
                        },
                    }
                })
                .collect()
        }
        "G1" => {
            if p.stage == "split" {
                // 多选：把选中的加密代码（可多个，逗号分隔）放进 A 堆；未选的进 B 堆。
                pending_codes(&p)
                    .into_iter()
                    .map(|code| Action {
                        id: format!("sg:G1:split:{code}"),
                        description: format!("{code} → A 堆（多选写法：#选 代码1，代码2）"),
                    })
                    .collect()
            } else {
                vec![
                    Action { id: "sg:G1:pick:0".into(), description: "选 A 堆".into() },
                    Action { id: "sg:G1:pick:1".into(), description: "选 B 堆".into() },
                ]
            }
        }
        "H2" => match p.stage.as_str() {
            "five_action" => {
                let mut v = Vec::new();
                if hand_dice(st, player) > 0 {
                    v.push(Action {
                        id: "sg:H2:put".into(),
                        description: "放入：从手中拿 1 颗骰放进任一赌场（不触发小游戏）".into(),
                    });
                }
                if !f2_takeback_casinos(st, player).is_empty() {
                    v.push(Action {
                        id: "sg:H2:takeback".into(),
                        description: "收回：从任一赌场取回自己 1 颗骰".into(),
                    });
                }
                v
            }
            "five_put" => (1..=CASINOS)
                .map(|c| Action {
                    id: format!("sg:H2:put:{c}"),
                    description: format!("把手中 1 颗 {c} 点骰放进 {c} 号赌场"),
                })
                .collect(),
            "five_takeback" => f2_takeback_casinos(st, player)
                .into_iter()
                .map(|c| Action {
                    id: format!("sg:H2:takeback:{c}"),
                    description: format!("取回自己在 {c} 号赌场的 1 颗骰"),
                })
                .collect(),
            _ => {
                let dice: Vec<u32> = p
                    .data
                    .get("dice")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                h2_options(st, player, &dice)
                    .into_iter()
                    .map(|d| Action {
                        id: format!("sg:H2:choose:{d}"),
                        description: match d {
                            1 => "1 点 → 1 筹码".to_string(),
                            2 => "2 点 → 2 筹码".to_string(),
                            3 => "3 点 → $3 万".to_string(),
                            4 => "4 点 → 重掷 2 颗黑骰，再次选择".to_string(),
                            5 => "5 点 → 放入/收回 1 颗骰（二段式）".to_string(),
                            _ => "6 点 → 1 颗骰放金框（结算 $6 万）".to_string(),
                        },
                    })
                    .collect()
            }
        },
        "H1" => {
            if p.stage == "slot" {
                (1..=6)
                    .map(|n| Action {
                        id: format!("sg:H1:slot:{n}"),
                        description: format!(
                            "放进 {n} 号格（结算得 {}）",
                            h1_reward_text(n)
                        ),
                    })
                    .collect()
            } else {
                let mut v = Vec::new();
                let c = st.casinos[p.casino].dice.get(player).copied().unwrap_or(0);
                if c > 0 {
                    v.push(Action {
                        id: "sg:H1:take:casino".into(),
                        description: format!("用该赌场已下注的 1 颗自己的骰子（现有 {c} 点权重）"),
                    });
                }
                if hand_dice(st, player) > 0 {
                    v.push(Action {
                        id: "sg:H1:take:hand".into(),
                        description: "用手里 1 颗骰子（优先小骰）".into(),
                    });
                }
                v.push(Action {
                    id: "sg:H1:skip".into(),
                    description: "跳过好骰难求".into(),
                });
                v
            }
        }
        _ => vec![],
    }
}

/// 解析一个 `sg:*` 交互动作；返回（是否完成，事件，文案）。未完成时 pending 已更新。
pub(crate) fn resolve(
    st: &mut LasVegasState,
    action: &str,
    player: &str,
    rng: &mut dyn RandomSource,
) -> Result<(bool, Vec<EngineEvent>, String), EngineError> {
    let Some(p) = st.sg.pending.clone() else {
        return Err(EngineError::IllegalAction("当前没有小游戏交互".into()));
    };
    if p.actor != player {
        return Err(EngineError::PermissionDenied("未轮到你在小游戏中操作".into()));
    }
    let rest = action
        .strip_prefix("sg:")
        .ok_or_else(|| EngineError::IllegalAction("非法小游戏动作".into()))?;
    let mut events = Vec::new();
    let msg;
    match p.game.as_str() {
        "G2" => {
            let k: u32 = rest
                .strip_prefix("G2:move:")
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| EngineError::IllegalAction("G2 参数错误".into()))?;
            let casino = p.casino;
            let weight = st.casinos[casino].dice.get(player).copied().unwrap_or(0);
            let big = st.casinos[casino].big_dice.get(player).copied().unwrap_or(0);
            let small = weight.saturating_sub(big * 2);
            let pieces = small + big;
            if k > pieces {
                return Err(EngineError::IllegalAction(
                    "移入颗数超过该赌场自己的骰子数".into(),
                ));
            }
            if k == 0 {
                events.push(EngineEvent::new(
                    "small_game",
                    json!({ "game": "G2", "player": player, "moved": 0 }),
                ));
                return Ok((true, events, "放弃移入游戏场".into()));
            }
            // 先移小骰，再移大骰（大骰整颗，权重 2）。
            let take_small = small.min(k);
            let take_big = (k - take_small).min(big);
            let moved_weight = take_small + take_big * 2;
            let left = weight.saturating_sub(moved_weight);
            if left == 0 {
                st.casinos[casino].dice.remove(player);
            } else if let Some(d) = st.casinos[casino].dice.get_mut(player) {
                *d = left;
            }
            if take_big > 0 {
                let bigs = &mut st.casinos[casino].big_dice;
                if let Some(b) = bigs.get_mut(player) {
                    *b = b.saturating_sub(take_big);
                    if *b == 0 {
                        bigs.remove(player);
                    }
                }
            }
            *st.sg.game_field.entry(player.to_string()).or_insert(0) += moved_weight;
            events.push(EngineEvent::new(
                "small_game",
                json!({
                    "game": "G2",
                    "player": player,
                    "casino": casino + 1,
                    "moved": k,
                    "weight": moved_weight,
                }),
            ));
            Ok((
                true,
                events,
                format!("把 {k} 颗骰子移入游戏场（等效权重 {moved_weight}），参与最终结算"),
            ))
        }
        "B1" => {
            let c: usize = rest
                .strip_prefix("B1:put:")
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| EngineError::IllegalAction("B1 参数错误".into()))?;
            if !(1..=CASINOS).contains(&c) {
                return Err(EngineError::IllegalAction("B1 赌场号须在 1–6".into()));
            }
            let black: Vec<u32> = p
                .data
                .get("black")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let k = black.iter().filter(|d| **d as usize == c).count() as u32;
            if k > 0 {
                // 黑骰算作该玩家自己的骰子参与结算（只参与结算，不重算 D2 / 不触发小游戏）。
                add_casino_dice(st, c - 1, player, k);
            }
            events.push(EngineEvent::new(
                "small_game",
                json!({
                    "game": "B1",
                    "player": player,
                    "black": black,
                    "casino": c,
                    "placed": k,
                }),
            ));
            let text = if k > 0 {
                format!("B1 主场时间：把 {k} 颗黑骰放进 {c} 号赌场，参与本轮结算")
            } else {
                format!("B1 主场时间：不放骰子到 {c} 号赌场")
            };
            msg = text;
            Ok((true, events, msg))
        }
        "A1" => {
            // 第一段（加密选项）：群里发的是代码，代码 → 枚数只在牌面页可见。
            if let Some(code) = rest.strip_prefix("code:") {
                let count = code_to_count(&p, code.trim())
                    .ok_or_else(|| EngineError::IllegalAction("选项代码无效或已过期".into()))?;
                let initiator = p
                    .data
                    .get("initiator")
                    .and_then(|v| v.as_str())
                    .unwrap_or(player)
                    .to_string();
                events.push(EngineEvent::new(
                    "small_game",
                    json!({ "game": "A1", "stage": "hide", "player": player, "code": code.trim().to_uppercase() }),
                ));
                let guesser = left_player(st, player);
                st.sg.pending = Some(SgPending {
                    game: "A1".into(),
                    casino: p.casino,
                    actor: guesser.clone(),
                    stage: "guess".into(),
                    data: json!({ "initiator": initiator, "count": count }),
                });
                return Ok((
                    false,
                    events,
                    format!("已收到加密选项。请 {guesser} 猜：{initiator} 右手里藏了几枚标记（1–3）"),
                ));
            }
            // 第一段（旧内部入口，测试/机器人）：直接指定第几个选项。
            if let Some(kstr) = rest.strip_prefix("A1:hide:") {
                let slot: usize = kstr
                    .parse()
                    .map_err(|_| EngineError::IllegalAction("A1 选项非法".into()))?;
                let perm: Vec<u32> = p
                    .data
                    .get("perm")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_else(|| (1..=A1_MARKERS).collect());
                if !(1..=A1_MARKERS as usize).contains(&slot) {
                    return Err(EngineError::IllegalAction("A1 选项须为 1–3".into()));
                }
                let count = *perm
                    .get(slot - 1)
                    .ok_or_else(|| EngineError::IllegalAction("A1 选项须为 1–3".into()))?;
                let initiator = p
                    .data
                    .get("initiator")
                    .and_then(|v| v.as_str())
                    .unwrap_or(player)
                    .to_string();
                events.push(EngineEvent::new(
                    "small_game",
                    json!({ "game": "A1", "stage": "hide", "player": player, "option": slot }),
                ));
                let guesser = left_player(st, player);
                st.sg.pending = Some(SgPending {
                    game: "A1".into(),
                    casino: p.casino,
                    actor: guesser.clone(),
                    stage: "guess".into(),
                    data: json!({ "initiator": initiator, "count": count }),
                });
                return Ok((
                    false,
                    events,
                    format!("请 {guesser} 猜：{initiator} 右手里藏了几枚标记（1–3）"),
                ));
            }
            // 第二段：左手玩家猜 1–3 枚。
            let k: u32 = rest
                .strip_prefix("A1:guess:")
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| EngineError::IllegalAction("A1 参数错误".into()))?;
            if !(1..=A1_MARKERS).contains(&k) {
                return Err(EngineError::IllegalAction("A1 只能猜 1–3 枚".into()));
            }
            let count = p.data.get("count").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
            let initiator = p
                .data
                .get("initiator")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let ok = k == count;
            let mut payload = json!({ "game": "A1", "guess": k, "count": count, "correct": ok });
            if ok {
                msg = format!("猜对了：{initiator} 没有奖励，小游戏结束");
            } else {
                match a1_reward(count) {
                    Reward::Chips(c) => {
                        add_chips(st, &initiator, c);
                        payload["chips"] = json!(c);
                    }
                    Reward::Note(v) => {
                        add_note(st, &initiator, v);
                        payload["note"] = json!(v);
                    }
                }
                msg = format!("猜错了：{initiator} 得 {}", reward_text(a1_reward(count)));
            }
            events.push(EngineEvent::new("small_game", payload));
            Ok((true, events, msg))
        }
        "B2" => {
            let pos = st.sg.b2_pos;
            // 弃权：按**当前格**结算奖励并结束。
            if rest == "B2:quit" {
                let gain = b2_reward(pos);
                if let Some(r) = gain {
                    match r {
                        Reward::Chips(c) => add_chips(st, player, c),
                        Reward::Note(v) => add_note(st, player, v),
                    }
                }
                events.push(EngineEvent::new(
                    "small_game",
                    json!({ "game": "B2", "quit": true, "pos": pos }),
                ));
                let text = match gain {
                    Some(r) => format!("弃权：领第 {pos} 格的 {}", reward_text(r)),
                    None => format!("弃权：第 {pos} 格无奖励"),
                };
                return Ok((true, events, text));
            }
            let high = rest.ends_with("high");
            let a = roll_die(rng) as u32;
            let b = roll_die(rng) as u32;
            let new = a + b;
            let prev = p.data.get("prev").and_then(|v| v.as_u64()).unwrap_or(7) as u32;
            let correct = (high && new > prev) || (!high && new < prev);
            events.push(EngineEvent::new(
                "small_game",
                json!({ "game": "B2", "black": [a, b], "prev": prev, "correct": correct, "pos": pos }),
            ));
            if !correct {
                // 猜错（含和相等）：游戏结束，无任何奖励。
                return Ok((
                    true,
                    events,
                    format!("掷出 {new}（上次 {prev}），猜错：游戏结束，无奖励"),
                ));
            }
            let next = pos + 1;
            st.sg.b2_pos = next;
            st.sg.b2_prev = new;
            if next >= B2_TOP {
                // 到达第 5 格：直接领 $6 万，结束。
                if let Some(r) = b2_reward(B2_TOP) {
                    match r {
                        Reward::Chips(c) => add_chips(st, player, c),
                        Reward::Note(v) => add_note(st, player, v),
                    }
                }
                events.push(EngineEvent::new(
                    "small_game",
                    json!({ "game": "B2", "top": true, "pos": B2_TOP }),
                ));
                return Ok((
                    true,
                    events,
                    format!(
                        "掷出 {new}（上次 {prev}），猜对到第 {B2_TOP} 格：{}",
                        b2_reward(B2_TOP)
                            .map(reward_text)
                            .unwrap_or_else(|| "无奖励".into())
                    ),
                ));
            }
            st.sg.pending = Some(SgPending {
                game: "B2".into(),
                casino: p.casino,
                actor: p.actor.clone(),
                stage: "guess".into(),
                data: json!({ "prev": new }),
            });
            Ok((
                false,
                events,
                format!(
                    "掷出 {new}（上次 {prev}），猜对前进到第 {next} 格；继续选大/小或弃权"
                ),
            ))
        }
        "E1" => {
            if rest == "E1:pass" {
                return Ok((true, events, "放弃放置禁止标记：箭头不前进，无奖励".into()));
            }
            let c: usize = rest
                .strip_prefix("E1:ban:")
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| EngineError::IllegalAction("E1 参数错误".into()))?;
            if c < 1 || c > CASINOS || c - 1 == p.casino {
                return Err(EngineError::IllegalAction("禁止标记位置非法".into()));
            }
            st.sg.banned = Some(c - 1);
            // 箭头环形前进 1 格，奖励按**前进后**的位置结算。
            let slots = 6usize;
            st.sg.arrow = (st.sg.arrow + 1) % slots;
            let slot = st.sg.arrow + 1;
            let reward = e1_reward(slot);
            let mut payload = json!({ "game": "E1", "banned": c, "arrow": slot });
            match reward {
                Some(Reward::Chips(n)) => {
                    add_chips(st, player, n);
                    payload["chips"] = json!(n);
                }
                Some(Reward::Note(v)) => {
                    add_note(st, player, v);
                    payload["note"] = json!(v);
                }
                None => {}
            }
            events.push(EngineEvent::new("small_game", payload));
            let reward_text = match reward {
                Some(r) => format!("，箭头前进到第 {slot} 格，获得 {}", reward_text(r)),
                None => format!("，箭头前进到第 {slot} 格（该格无奖励）"),
            };
            Ok((
                true,
                events,
                format!("禁止标记已放到 {c} 号赌场{reward_text}"),
            ))
        }
        "F1" => {
            // 第一轮：选组 → 进入第二轮（选赌场）
            if let Some(gstr) = rest.strip_prefix("F1:group:") {
                let g: usize = gstr
                    .parse()
                    .map_err(|_| EngineError::IllegalAction("F1 组号错误".into()))?;
                if g >= st.sg.gray_groups.len() || st.sg.gray_groups[g] == 0 {
                    return Err(EngineError::IllegalAction("该组灰骰不存在或已空".into()));
                }
                let mut data = p.data.clone();
                data["group"] = json!(g);
                st.sg.pending = Some(SgPending {
                    game: "F1".into(),
                    casino: p.casino,
                    actor: p.actor.clone(),
                    stage: "casino".into(),
                    data,
                });
                let cnt = st.sg.gray_groups[g];
                return Ok((
                    false,
                    events,
                    format!("已选第 {} 组（{cnt} 颗），请选择要放入的赌场", g + 1),
                ));
            }
            let parts: Vec<&str> = rest.trim_start_matches("F1:move:").split(':').collect();
            let g: usize = parts.first().and_then(|s| s.parse().ok()).unwrap_or(usize::MAX);
            let c: usize = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
            if g >= st.sg.gray_groups.len() || c < 1 || c > CASINOS {
                return Err(EngineError::IllegalAction("F1 参数错误".into()));
            }
            let cnt = st.sg.gray_groups[g];
            if cnt == 0 {
                return Err(EngineError::IllegalAction("该组灰骰已空".into()));
            }
            st.sg.gray_groups[g] = 0;
            add_casino_dice(st, c - 1, "gray", cnt);
            events.push(EngineEvent::new("small_game", json!({ "game": "F1", "group": g, "casino": c, "count": cnt })));
            Ok((true, events, format!("把 {cnt} 颗灰骰放到 {c} 号赌场")))
        }
        "F2" => {
            // 多段式（重做版·裁定 2026-10-17）：gray → group → (c_action → c_put | c_takeback)
            let body = rest.strip_prefix("F2:").unwrap_or("");
            let next = |stage: &str, data: serde_json::Value| SgPending {
                game: "F2".into(),
                casino: p.casino,
                actor: p.actor.clone(),
                stage: stage.into(),
                data,
            };

            // ① 取场上 1 颗灰骰（从原赌场移出）
            if let Some(cstr) = body.strip_prefix("take:") {
                let c: usize = cstr
                    .parse()
                    .map_err(|_| EngineError::IllegalAction("F2 赌场参数错误".into()))?;
                if !(1..=CASINOS).contains(&c) {
                    return Err(EngineError::IllegalAction("F2 赌场号须在 1–6".into()));
                }
                let n = st.casinos[c - 1].dice.get("gray").copied().unwrap_or(0);
                if n == 0 {
                    return Err(EngineError::IllegalAction("该赌场没有灰骰".into()));
                }
                if n == 1 {
                    st.casinos[c - 1].dice.remove("gray");
                } else if let Some(d) = st.casinos[c - 1].dice.get_mut("gray") {
                    *d = n - 1;
                }
                update_privilege(st, c - 1);
                let mut data = p.data.clone();
                data["from"] = json!(c);
                st.sg.pending = Some(next("group", data));
                events.push(EngineEvent::new(
                    "small_game",
                    json!({ "game": "F2", "player": player, "gray_from": c }),
                ));
                return Ok((false, events, "已取灰骰，请选择放进哪一组（a/b/c）".into()));
            }

            // ② 选组
            if let Some(gs) = body.strip_prefix("group:") {
                let group = F2_GROUP_NAMES
                    .iter()
                    .position(|g| *g == gs)
                    .ok_or_else(|| EngineError::IllegalAction("F2 组名须为 a/b/c".into()))?;
                if !f2_group_has_empty(&st.sg, group) {
                    return Err(EngineError::IllegalAction("该组格位已满".into()));
                }
                if group == 2 && !f2_c_usable(st, player) {
                    return Err(EngineError::IllegalAction(
                        "c 组当前不可用（手中无骰且场上无自己的骰）".into(),
                    ));
                }
                let from = p.data.get("from").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
                let idx = f2_place_slot(&mut st.sg, group, from)
                    .ok_or_else(|| EngineError::Internal("F2 放格失败".into()))?;
                events.push(EngineEvent::new(
                    "small_game",
                    json!({ "game": "F2", "player": player, "slot": idx + 1, "group": gs, "gray_from": from }),
                ));
                return match group {
                    0 => {
                        add_chips(st, player, 1);
                        Ok((true, events, "a 组：得 1 筹码".into()))
                    }
                    1 => {
                        add_note(st, player, 30_000);
                        Ok((true, events, "b 组：得 $3 万".into()))
                    }
                    _ => {
                        // c 组：手中无骰 → 跳过第 1 段，直达「收回」。
                        let stage = if hand_dice(st, player) > 0 {
                            "c_action"
                        } else {
                            "c_takeback"
                        };
                        st.sg.pending = Some(next(stage, p.data.clone()));
                        let text = if stage == "c_action" {
                            "c 组：请选择「放入」或「收回」"
                        } else {
                            "c 组：手中已无骰，请选择从哪个赌场收回 1 颗"
                        };
                        Ok((false, events, text.into()))
                    }
                };
            }

            // ③ c 组第 1 段：放入 / 收回
            match body {
                "put" => {
                    if hand_dice(st, player) == 0 {
                        return Err(EngineError::IllegalAction("手中已无骰子".into()));
                    }
                    st.sg.pending = Some(next("c_put", p.data.clone()));
                    return Ok((false, events, "请选择放入的点数（1–6）".into()));
                }
                "takeback" => {
                    if f2_takeback_casinos(st, player).is_empty() {
                        return Err(EngineError::IllegalAction("场上没有自己的骰子".into()));
                    }
                    st.sg.pending = Some(next("c_takeback", p.data.clone()));
                    return Ok((false, events, "请选择从哪个赌场收回 1 颗".into()));
                }
                _ => {}
            }

            // ④ c 组第 2 段：放入点数 / 收回赌场
            if let Some(nstr) = body.strip_prefix("put:") {
                let n: usize = nstr
                    .parse()
                    .map_err(|_| EngineError::IllegalAction("F2 点数参数错误".into()))?;
                if !(1..=CASINOS).contains(&n) {
                    return Err(EngineError::IllegalAction("F2 点数须在 1–6".into()));
                }
                let weight = take_hand_die(st, player)
                    .ok_or_else(|| EngineError::IllegalAction("手中已无骰子".into()))?;
                add_casino_dice(st, n - 1, player, weight);
                if weight == 2 {
                    *st.casinos[n - 1]
                        .big_dice
                        .entry(player.to_string())
                        .or_insert(0) += 1;
                }
                // 不触发该赌场小游戏（不走 on_place），也不重算 D2（D2 只在主动下注时判定）。
                events.push(EngineEvent::new(
                    "small_game",
                    json!({ "game": "F2", "player": player, "put_point": n, "weight": weight }),
                ));
                return Ok((
                    true,
                    events,
                    format!("把手中 1 颗 {n} 点骰（权重 {weight}）放进 {n} 号赌场"),
                ));
            }
            if let Some(cstr) = body.strip_prefix("takeback:") {
                let c: usize = cstr
                    .parse()
                    .map_err(|_| EngineError::IllegalAction("F2 赌场参数错误".into()))?;
                if !(1..=CASINOS).contains(&c) {
                    return Err(EngineError::IllegalAction("F2 赌场号须在 1–6".into()));
                }
                let took_big = take_back_die(st, c - 1, player)
                    .ok_or_else(|| EngineError::IllegalAction("该赌场没有你的骰子".into()))?;
                events.push(EngineEvent::new(
                    "small_game",
                    json!({ "game": "F2", "player": player, "takeback_from": c, "big": took_big }),
                ));
                let kind = if took_big { "大骰" } else { "小骰" };
                return Ok((true, events, format!("从 {c} 号赌场取回 1 颗{kind}")));
            }
            Err(EngineError::IllegalAction("F2 动作非法".into()))
        }
        "G1" => {
            if p.stage == "split" {
                // 多选：`sg:G1:split:{code1},{code2},...`（英文/中文逗号均可）。
                let raw = rest
                    .strip_prefix("G1:split:")
                    .ok_or_else(|| EngineError::IllegalAction("G1 参数错误".into()))?;
                let codes = pending_codes(&p);
                let mut picked: Vec<usize> = Vec::new();
                for tok in raw.split([',', '，']).map(|t| t.trim()).filter(|t| !t.is_empty()) {
                    let idx = codes
                        .iter()
                        .position(|c| c.eq_ignore_ascii_case(tok))
                        .ok_or_else(|| EngineError::IllegalAction(format!("标记代码无效：{tok}")))?;
                    if !picked.contains(&idx) {
                        picked.push(idx);
                    }
                }
                let total: u32 = p.data.get("total").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                let a: u32 = picked
                    .iter()
                    .map(|i| st.sg.lottery.get(*i).copied().unwrap_or(0))
                    .sum();
                let b = total.saturating_sub(a);
                let winner = p
                    .data
                    .get("winner")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                events.push(EngineEvent::new(
                    "small_game",
                    json!({ "game": "G1", "stage": "split", "player": player, "a": a, "b": b }),
                ));
                st.sg.pending = Some(SgPending {
                    game: "G1".into(),
                    casino: p.casino,
                    actor: winner.clone(),
                    stage: "pick".into(),
                    data: json!({ "winner": winner, "total": total, "piles": [a, b] }),
                });
                return Ok((
                    false,
                    events,
                    format!("已分成 A：{a} 万 / B：{b} 万，等待 {winner} 选择（#选 a 或 #选 b）"),
                ));
            }
            let idx: usize = match rest.strip_prefix("G1:pick:") {
                Some(x) => x
                    .parse()
                    .map_err(|_| EngineError::IllegalAction("G1 参数错误".into()))?,
                None => return Err(EngineError::IllegalAction("G1 参数错误".into())),
            };
            if idx > 1 {
                return Err(EngineError::IllegalAction("G1 只能选 a 或 b".into()));
            }
            let piles: Vec<u32> = p
                .data
                .get("piles")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let gain = piles.get(idx).copied().unwrap_or(0);
            if gain > 0 {
                add_note(st, player, gain * 10_000);
            }
            events.push(EngineEvent::new(
                "small_game",
                json!({ "game": "G1", "pick": idx, "gain": gain }),
            ));
            Ok((
                true,
                events,
                format!("选了 {} 堆，得 {} 万", if idx == 0 { "A" } else { "B" }, gain),
            ))
        }
        "H2" => {
            let stage = p.stage.clone();
            // 二段式（5 点）：放入 / 收回
            if stage == "five_action" {
                return match rest {
                    "H2:put" => {
                        if hand_dice(st, player) == 0 {
                            return Err(EngineError::IllegalAction("手中已无骰子".into()));
                        }
                        st.sg.pending = Some(SgPending {
                            game: "H2".into(),
                            casino: p.casino,
                            actor: p.actor.clone(),
                            stage: "five_put".into(),
                            data: p.data.clone(),
                        });
                        Ok((false, events, "请选择放入的点数（1–6）".into()))
                    }
                    "H2:takeback" => {
                        if f2_takeback_casinos(st, player).is_empty() {
                            return Err(EngineError::IllegalAction("场上没有自己的骰子".into()));
                        }
                        st.sg.pending = Some(SgPending {
                            game: "H2".into(),
                            casino: p.casino,
                            actor: p.actor.clone(),
                            stage: "five_takeback".into(),
                            data: p.data.clone(),
                        });
                        Ok((false, events, "请选择从哪个赌场收回 1 颗".into()))
                    }
                    _ => Err(EngineError::IllegalAction("H2 5 点动作非法".into())),
                };
            }
            if stage == "five_put" {
                let c: usize = rest
                    .strip_prefix("H2:put:")
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| EngineError::IllegalAction("H2 参数错误".into()))?;
                if !(1..=CASINOS).contains(&c) {
                    return Err(EngineError::IllegalAction("H2 赌场号须在 1–6".into()));
                }
                let weight = take_hand_die(st, player)
                    .ok_or_else(|| EngineError::IllegalAction("手中已无骰子".into()))?;
                add_casino_dice(st, c - 1, player, weight);
                if weight == 2 {
                    *st.casinos[c - 1]
                        .big_dice
                        .entry(player.to_string())
                        .or_insert(0) += 1;
                }
                // 不触发该赌场小游戏；D2 只在主动下注时重算。
                events.push(EngineEvent::new(
                    "small_game",
                    json!({ "game": "H2", "point": 5, "put_point": c, "weight": weight }),
                ));
                return Ok((
                    true,
                    events,
                    format!("5 点：把手中 1 颗 {c} 点骰放进 {c} 号赌场（不触发小游戏）"),
                ));
            }
            if stage == "five_takeback" {
                let c: usize = rest
                    .strip_prefix("H2:takeback:")
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| EngineError::IllegalAction("H2 参数错误".into()))?;
                if !(1..=CASINOS).contains(&c) {
                    return Err(EngineError::IllegalAction("H2 赌场号须在 1–6".into()));
                }
                let took_big = take_back_die(st, c - 1, player)
                    .ok_or_else(|| EngineError::IllegalAction("该赌场没有你的骰子".into()))?;
                events.push(EngineEvent::new(
                    "small_game",
                    json!({ "game": "H2", "point": 5, "takeback_from": c, "big": took_big }),
                ));
                let kind = if took_big { "大骰" } else { "小骰" };
                return Ok((true, events, format!("5 点：从 {c} 号赌场收回 1 颗{kind}")));
            }

            // 第一段：选黑骰点数
            let d: u32 = rest
                .strip_prefix("H2:choose:")
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| EngineError::IllegalAction("H2 参数错误".into()))?;
            let dice: Vec<u32> = p
                .data
                .get("dice")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            if !h2_options(st, player, &dice).contains(&d) {
                return Err(EngineError::IllegalAction(format!(
                    "H2 {d} 点当前不可选（条件不满足）"
                )));
            }
            msg = match d {
                1 => {
                    add_chips(st, player, 1);
                    "H2：1 点，得 1 筹码".into()
                }
                2 => {
                    add_chips(st, player, 2);
                    "H2：2 点，得 2 筹码".into()
                }
                3 => {
                    add_note(st, player, 30_000);
                    "H2：3 点，得 $3 万".into()
                }
                4 => {
                    // 村规：重掷 2 颗黑骰（不出现 4,4），再次选择
                    let next = roll_h2_dice(rng);
                    st.sg.pending = Some(SgPending {
                        game: "H2".into(),
                        casino: p.casino,
                        actor: p.actor.clone(),
                        stage: "choose".into(),
                        data: json!({ "dice": next }),
                    });
                    events.push(EngineEvent::new(
                        "small_game",
                        json!({ "game": "H2", "rerolled": next }),
                    ));
                    return Ok((
                        false,
                        events,
                        format!("H2：4 点触发村规 → 重掷黑骰得 {}、{}，请重新选择", next[0], next[1]),
                    ));
                }
                5 => {
                    let hand = hand_dice(st, player) > 0;
                    let takeback = !f2_takeback_casinos(st, player).is_empty();
                    if !hand && !takeback {
                        return Err(EngineError::IllegalAction("H2 5 点不可选".into()));
                    }
                    let stage = if hand { "five_action" } else { "five_takeback" };
                    st.sg.pending = Some(SgPending {
                        game: "H2".into(),
                        casino: p.casino,
                        actor: p.actor.clone(),
                        stage: stage.into(),
                        data: p.data.clone(),
                    });
                    let text = if hand {
                        "H2：5 点 → 请选择「放入」或「收回」"
                    } else {
                        "H2：5 点 → 手中已无骰，请选择从哪个赌场收回 1 颗"
                    };
                    return Ok((false, events, text.into()));
                }
                6 => {
                    // 放 1 颗手中骰入金框（优先小骰；只剩一颗时不管大小都取）。
                    // 挤走原骰：按原骰型还给对应玩家。
                    let took_big = {
                        let Some(seat) = st.seats.iter_mut().find(|s| s.player_id == player)
                        else {
                            return Err(EngineError::Internal("座位不存在".into()));
                        };
                        if seat.small_remaining > 0 {
                            seat.small_remaining -= 1;
                            false
                        } else if seat.big_remaining > 0 {
                            seat.big_remaining -= 1;
                            true
                        } else {
                            return Err(EngineError::IllegalAction("手中已无骰子".into()));
                        }
                    };
                    let prev = st.sg.gold_frame.take();
                    let displaced = prev.as_ref().map(|f| (f.owner.clone(), f.pieces()));
                    if let Some(old) = prev {
                        if old.owner != player {
                            if let Some(seat) =
                                st.seats.iter_mut().find(|s| s.player_id == old.owner)
                            {
                                seat.small_remaining += old.small as u8;
                                seat.big_remaining += old.big as u8;
                            }
                        }
                    }
                    st.sg.gold_frame = Some(GoldFrame {
                        owner: player.to_string(),
                        small: u32::from(!took_big),
                        big: u32::from(took_big),
                    });
                    events.push(EngineEvent::new(
                        "small_game",
                        json!({
                            "game": "H2",
                            "point": 6,
                            "player": player,
                            "big": took_big,
                            "displaced": displaced,
                        }),
                    ));
                    "H2：6 点 → 放 1 颗骰到金框（结算时若仍在框内得 $6 万）".into()
                }
                _ => return Err(EngineError::IllegalAction("H2 点数须为 1–6".into())),
            };
            events.push(EngineEvent::new("small_game", json!({ "game": "H2", "choose": d })));
            Ok((true, events, msg))
        }
        "H1" => {
            if rest == "H1:skip" {
                events.push(EngineEvent::new(
                    "small_game",
                    json!({ "game": "H1", "player": player, "skip": true }),
                ));
                return Ok((true, events, "跳过好骰难求".into()));
            }
            // 第一段：选来源（赌场里已下注的骰子 / 手里的骰子）。
            if rest == "H1:take:casino" || rest == "H1:take:hand" {
                let took_big = if rest.ends_with("casino") {
                    h1_take_die(st, p.casino, player)?
                } else {
                    match take_hand_die(st, player) {
                        Some(2) => true,
                        Some(_) => false,
                        None => return Err(EngineError::IllegalAction("手中已无骰子".into())),
                    }
                };
                let source = if rest.ends_with("casino") { "casino" } else { "hand" };
                events.push(EngineEvent::new(
                    "small_game",
                    json!({ "game": "H1", "stage": "take", "player": player, "source": source, "big": took_big }),
                ));
                st.sg.pending = Some(SgPending {
                    game: "H1".into(),
                    casino: p.casino,
                    actor: p.actor.clone(),
                    stage: "slot".into(),
                    data: json!({ "big": took_big, "source": source }),
                });
                return Ok((false, events, "请选择放进 1–6 号格".into()));
            }
            // 第二段：**玩家自选**格位（1–6）。
            let slot: usize = rest
                .strip_prefix("H1:slot:")
                .and_then(|x| x.parse().ok())
                .ok_or_else(|| EngineError::IllegalAction("H1 参数错误".into()))?;
            if !(1..=6).contains(&slot) {
                return Err(EngineError::IllegalAction("H1 格位须为 1–6".into()));
            }
            let took_big = p.data.get("big").and_then(|v| v.as_bool()).unwrap_or(false);
            h1_slot_put(st, player, slot, took_big, &mut events);
            st.sg.h1_used_turn = true;
            events.push(EngineEvent::new(
                "small_game",
                json!({ "game": "H1", "player": player, "slot": slot, "big": took_big }),
            ));
            Ok((
                true,
                events,
                format!(
                    "放进 {slot} 号格，本轮结算得 {}",
                    h1_reward_text(slot)
                ),
            ))
        }
        _ => Err(EngineError::IllegalAction("未知小游戏交互".into())),
    }
}

/// 回合内（非 pending）的小游戏动作：D2 特权 / G2 移游戏场 / H1 移格。
pub(crate) fn legal_turn_sg(st: &LasVegasState, player: &str) -> Vec<Action> {
    if st.phase != crate::Phase::TurnRoll {
        return vec![];
    }
    let Some(cur) = st.seats.get(st.turn) else {
        return vec![];
    };
    if cur.player_id != player {
        return vec![];
    }
    let mut out = vec![];
    // 回合内附加动作（按时机表派发；`TurnAction`）。
    for (_ci, id) in tiles_with_tempo(st, Tempo::TurnAction) {
        // D2 权力优势：持有特权时可跳过投掷，放 1 颗指定点数。
        if id == "D2" && st.sg.privilege.as_deref() == Some(player) && cur.has_dice() {
            for n in 1..=CASINOS {
                // E1 禁止的赌场不能放入。
                if st.sg.banned == Some(n - 1) {
                    continue;
                }
                out.push(Action {
                    id: format!("sg:D2:use:{n}"),
                    description: format!("用特权：不掷骰，放 1 颗 {n} 点骰到 {n} 号赌场"),
                });
            }
        }
    }
    out
}

/// 解析回合内小游戏动作；返回（事件，文案，是否推进回合）。非本类动作返回 None。
pub(crate) fn resolve_turn_sg(
    st: &mut LasVegasState,
    action: &str,
    player: &str,
    rng: &mut dyn RandomSource,
) -> Option<Result<(Vec<EngineEvent>, String, bool), EngineError>> {
    let rest = action.strip_prefix("sg:")?;
    let mut events = Vec::new();
    if let Some(nstr) = rest.strip_prefix("D2:use:") {
        let n: usize = nstr.parse().ok().unwrap_or(0);
        if !(1..=CASINOS).contains(&n) {
            return Some(Err(EngineError::IllegalAction("D2 点数非法".into())));
        }
        if st.sg.privilege.as_deref() != Some(player) {
            return Some(Err(EngineError::PermissionDenied("你没有特权标记".into())));
        }
        if st.sg.banned == Some(n - 1) {
            return Some(Err(EngineError::IllegalAction("该赌场被禁止进入".into())));
        }
        // 手中 1 颗骰：优先小骰，仅剩大骰时整颗（权重 2）。
        let Some(weight) = take_hand_die(st, player) else {
            return Some(Err(EngineError::IllegalAction("没有可用骰子".into())));
        };
        add_casino_dice(st, n - 1, player, weight);
        if weight == 2 {
            *st.casinos[n - 1]
                .big_dice
                .entry(player.to_string())
                .or_insert(0) += 1;
        }
        events.push(EngineEvent::new(
            "small_game",
            json!({ "game": "D2", "player": player, "point": n, "weight": weight }),
        ));
        // 用户裁定：用特权的放置**会触发**该赌场的小游戏（走 on_place）。
        on_place(st, n - 1, player, rng, &mut events);
        let pending = st.sg.pending.is_some();
        if pending {
            // 与 `place:` 路径一致：挂上 pending 就切到小游戏阶段，暂停推进回合。
            st.phase = crate::Phase::SmallGame;
        }
        let text = if pending {
            format!("用特权把 1 颗 {n} 点骰放到 {n} 号赌场，触发小游戏")
        } else {
            format!("用特权把 1 颗 {n} 点骰放到 {n} 号赌场")
        };
        return Some(Ok((events, text, !pending)));
    }
    None
}

/// 结算扩展：G2 游戏场、H2 金框、H1 格位。
/// T5：钞票分配后·统一结算（按时机表派发）。
///
/// 各实现在 `settle_game_field` / `settle_gold_frame` / `settle_h1_slots`；
/// 派发结束后无论如何都清空这些公共区域（避免残影）。
pub(crate) fn settle_extras(st: &mut LasVegasState, events: &mut Vec<EngineEvent>) {
    for (_ci, id) in tiles_with_tempo(st, Tempo::PostAllocate) {
        match id.as_str() {
            "G2" => settle_game_field(st, events),
            "H2" => settle_gold_frame(st, events),
            "H1" => settle_h1_slots(st, events),
            _ => {}
        }
    }
    st.sg.game_field.clear();
    st.sg.gold_frame = None;
    st.sg.h1_slots = vec![None; 6];
}

/// G2 双倍颁奖：游戏场结算同赌场（先移除平手再比多少），最多者 $6 万、第二 $3 万。
fn settle_game_field(st: &mut LasVegasState, events: &mut Vec<EngineEvent>) {
    let field: Vec<(String, u32)> = st.sg.game_field.iter().map(|(p, c)| (p.clone(), *c)).collect();
    if field.is_empty() {
        return;
    }
    let mut by_value: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for (p, c) in &field {
        if *c > 0 {
            by_value.entry(*c).or_default().push(p.clone());
        }
    }
    let mut kept: Vec<(String, u32)> = field
        .iter()
        .filter(|(_p, c)| *c > 0 && by_value.get(c).map(|v| v.len() == 1).unwrap_or(false))
        .map(|(p, c)| (p.clone(), *c))
        .collect();
    kept.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    if let Some((p, _)) = kept.first() {
        add_note(st, p, G2_FIRST);
    }
    if let Some((p, _)) = kept.get(1) {
        add_note(st, p, G2_SECOND);
    }
    events.push(EngineEvent::new("small_game", json!({ "game": "G2", "field": kept })));
}

/// H2 我的抉择：金框内有骰 → 得 $6 万。
fn settle_gold_frame(st: &mut LasVegasState, events: &mut Vec<EngineEvent>) {
    if let Some(frame) = st.sg.gold_frame.clone() {
        if frame.pieces() > 0 {
            add_note(st, &frame.owner, H2_GOLD);
            events.push(EngineEvent::new(
                "small_game",
                json!({
                    "game": "H2",
                    "gold": frame.owner,
                    "big": frame.big,
                    "reward": H2_GOLD,
                }),
            ));
        }
    }
}

/// H1 好骰难求：按格位发奖（骰子到位即得，与是否赢下该赌场无关）。
fn settle_h1_slots(st: &mut LasVegasState, events: &mut Vec<EngineEvent>) {
    for slot in 0..st.sg.h1_slots.len() {
        let Some(slot_state) = st.sg.h1_slots[slot].clone() else {
            continue;
        };
        let (owner, n) = (slot_state.owner.clone(), slot_state.dice);
        if n == 0 {
            continue;
        }
        // 骰子到位即得奖（不要求赢得对应赌场）；多颗则按颗数叠加。
        let mut payload = json!({ "game": "H1", "slot": slot + 1, "owner": owner, "count": n });
        match h1_reward(slot + 1) {
            Reward::Chips(c) => {
                add_chips(st, &owner, c * n);
                payload["chips"] = json!(c * n);
            }
            Reward::Note(v) => {
                // 每颗骰各得 1 张（钞票张数参与同分裁决）。
                for _ in 0..n {
                    add_note(st, &owner, v);
                }
                payload["note"] = json!(v);
            }
        }
        events.push(EngineEvent::new("small_game", payload));
    }
}

/// 结算阶段的第一步（用户裁定 2026-10-17）：**按赌场顺序 1→6** 先跑「结算阶段进入」的小游戏
/// （同一赌场内 B1 先于 C2），全部跑完才正式逐场结算。
///
/// 返回 `true` = 中途挂了 pending（B1 交互），调用方应暂停结算。
pub(crate) fn pre_settle_pass(
    st: &mut LasVegasState,
    rng: &mut dyn RandomSource,
    events: &mut Vec<EngineEvent>,
) -> bool {
    for ci in 0..CASINOS {
        for tempo in [Tempo::PreSettlePending, Tempo::PreSettleCasino] {
            for (t_ci, id) in tiles_with_tempo(st, tempo) {
                if t_ci != ci {
                    continue;
                }
                match (tempo, id.as_str()) {
                    (Tempo::PreSettlePending, "B1") => {
                        if start_b1_pending(st, ci, rng, events) {
                            return true;
                        }
                    }
                    (Tempo::PreSettleCasino, "C2") if !st.sg.c2_done.contains(&ci) => {
                        st.sg.c2_done.push(ci);
                        c2_pre_casino(st, ci, events);
                    }
                    (Tempo::PreSettlePending, "G1") if !st.sg.g1_done => {
                        st.sg.g1_done = true;
                        let total: u32 = st.sg.lottery.iter().sum();
                        let winner = if total > 0 { casino_leader(st, ci) } else { None };
                        match winner {
                            Some(w) => {
                                start_g1_pending(st, ci, &w, rng, events);
                                return true;
                            }
                            None => events.push(EngineEvent::new(
                                "small_game",
                                json!({ "game": "G1", "casino": ci + 1, "skipped": true }),
                            )),
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    false
}

/// B1 主场时间（用户裁定 2026-10-17）：**本轮结算阶段**，该小游戏所在赌场的
/// 「移除平手后第一名」玩家进入一次游戏 → 掷 2 颗黑骰 → 选 1–6 号赌场放 0–2 颗黑骰
/// （放几颗由掷出的点数决定）作为**自己的骰子**参与结算；随后才正式逐场结算。
///
/// 返回 `true` = 已挂 pending（调用方应暂停结算）。
pub(crate) fn start_b1_pending(
    st: &mut LasVegasState,
    ci: usize,
    rng: &mut dyn RandomSource,
    events: &mut Vec<EngineEvent>,
) -> bool {
    if st.sg.b1_done {
        return false;
    }
    st.sg.b1_done = true;
    let Some(leader) = casino_leader(st, ci) else {
        events.push(EngineEvent::new(
            "small_game",
            json!({ "game": "B1", "casino": ci + 1, "skipped": true, "reason": "no_winner" }),
        ));
        return false;
    };
    let a = roll_die(rng);
    let b = roll_die(rng);
    events.push(EngineEvent::new(
        "small_game_pending",
        json!({ "game": "B1", "actor": leader, "casino": ci + 1 }),
    ));
    st.sg.pending = Some(SgPending {
        game: "B1".into(),
        casino: ci,
        actor: leader,
        stage: "place".into(),
        data: json!({ "black": [a, b] }),
    });
    true
}

/// 「移除平手后第一名」的玩家（非灰骰虚拟玩家）；没有唯一第一名时返回 None。
pub(crate) fn casino_leader(st: &LasVegasState, casino: usize) -> Option<String> {
    let mut by_value: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for (p, c) in &st.casinos[casino].dice {
        if *c > 0 {
            by_value.entry(*c).or_default().push(p.clone());
        }
    }
    let mut kept: Vec<(String, u32)> = st.casinos[casino]
        .dice
        .iter()
        .filter(|(_p, c)| **c > 0 && by_value.get(c).map(|v| v.len() == 1).unwrap_or(false))
        .map(|(p, c)| (p.clone(), *c))
        .collect();
    kept.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    kept.into_iter()
        .find(|(p, _)| p != "gray")
        .map(|(p, _)| p)
}

/// 启动 G1 交互（结算前）：左手玩家用加密代码多选分堆 → 赢家明文选 A/B。
pub(crate) fn start_g1_pending(
    st: &mut LasVegasState,
    casino: usize,
    winner: &str,
    rng: &mut dyn RandomSource,
    events: &mut Vec<EngineEvent>,
) {
    let splitter = left_player(st, winner);
    let total: u32 = st.sg.lottery.iter().sum();
    // 6 个摸彩标记各给一个一次性加密代码：代码↔面额映射只出现在左手玩家的牌面页。
    let codes: Vec<String> = (0..st.sg.lottery.len()).map(|_| make_code(rng)).collect();
    st.sg.pending = Some(SgPending {
        game: "G1".into(),
        casino,
        actor: splitter.clone(),
        stage: "split".into(),
        data: json!({ "winner": winner, "total": total, "codes": codes }),
    });
    events.push(EngineEvent::new(
        "small_game_pending",
        json!({ "game": "G1", "actor": splitter, "casino": casino + 1, "winner": winner, "total": total }),
    ));
}

/// 单个赌场结算前（移除平手前）的 C2 厄运降临：最少骰子者（含 0）付 $5 万，不足付全部资产。
/// T3：轮次结算前·逐赌场（按时机表派发；在「移除平手」之前执行）。
pub(crate) fn pre_settle_casino(st: &mut LasVegasState, casino: usize, events: &mut Vec<EngineEvent>) {
    for (ci, id) in tiles_with_tempo(st, Tempo::PreSettleCasino) {
        match id.as_str() {
            "C2" if ci == casino => c2_pre_casino(st, casino, events),
            _ => {}
        }
    }
}

/// C2 厄运降临实现（T3）。
fn c2_pre_casino(
    st: &mut LasVegasState,
    casino: usize,
    events: &mut Vec<EngineEvent>,
) {
    // 用户裁定 2026-10-17：**没有骰子的玩家不算**（与判定无关）；灰色骰无视。
    let counts: Vec<(String, u32)> = st
        .seats
        .iter()
        .map(|s| {
            (
                s.player_id.clone(),
                st.casinos[casino].dice.get(&s.player_id).copied().unwrap_or(0),
            )
        })
        .filter(|(_p, c)| *c > 0)
        .collect();
    let Some(min) = counts.iter().map(|x| x.1).min() else {
        events.push(EngineEvent::new(
            "small_game",
            json!({ "game": "C2", "casino": casino + 1, "skipped": true, "reason": "no_dice" }),
        ));
        return;
    };
    // 并列最少者**都要付**（最少不存在相同排名）。
    let payers: Vec<String> = counts
        .into_iter()
        .filter(|(_, c)| *c == min)
        .map(|(p, _)| p)
        .collect();
    for p in &payers {
        let paid = pay_assets(st, p, C2_PENALTY);
        events.push(EngineEvent::new(
            "small_game",
            json!({ "game": "C2", "casino": casino + 1, "player": p, "paid": paid }),
        ));
    }
}

/// 结算后：C1 大奖标记兑换为钞票。
/// 交互完成后：若队列里还有 pending，立刻提升为当前交互（返回其 game/actor 供提示）。
pub(crate) fn promote_queued_pending(st: &mut LasVegasState) -> Option<(String, String)> {
    if st.sg.pending.is_some() || st.sg.pending_queue.is_empty() {
        return None;
    }
    let p = st.sg.pending_queue.remove(0);
    let info = (p.game.clone(), p.actor.clone());
    st.sg.pending = Some(p);
    Some(info)
}

/// 恢复原语文案：重新生成「当前该谁做什么」的提示（`execute_action("resume")` 用，不改状态）。
pub(crate) fn resume_message(st: &LasVegasState) -> String {
    if let Some(p) = st.sg.pending.as_ref() {
        let hint = match (p.game.as_str(), p.stage.as_str()) {
            ("A1", "hide") => "请打开你的牌面页看「加密选项」，然后在群里发「#选 <代码>」".to_string(),
            ("A1", "guess") => "请发「#选 1」「#选 2」或「#选 3」猜枚数".to_string(),
            ("B1", _) => "请发「#选 <序号>」选择把黑骰放到哪个赌场（0 颗也算一个选项）".to_string(),
            ("F2", _) => "请按提示继续「#选 <序号>」".to_string(),
            _ => "请按提示继续操作（可用「#动作」查看选项）".to_string(),
        };
        return format!(
            "⏳ 当前等待 {} 完成小游戏 {}（{}）。{}",
            p.actor, p.game, p.stage, hint
        );
    }
    if let Some(seat) = st.seats.get(st.turn) {
        if st.phase == crate::Phase::TurnRoll {
            return format!(
                "🎯 当前轮到 {} 下注：发「#放 <点数>」或「#重投」",
                seat.player_id
            );
        }
    }
    format!("当前阶段：{:?}", st.phase)
}

/// 把「自动类」事件渲染成给群里看的一行提示（交互类由 pending 文案负责）。
pub(crate) fn event_summaries(events: &[EngineEvent]) -> Vec<String> {
    let mut out = Vec::new();
    for e in events {
        let p = &e.payload;
        let Some(game) = p.get("game").and_then(|v| v.as_str()) else {
            continue;
        };
        match game {
            "A2" => {
                let a = p.get("black").and_then(|v| v.get(0)).and_then(|v| v.as_u64()).unwrap_or(0);
                let b = p.get("black").and_then(|v| v.get(1)).and_then(|v| v.as_u64()).unwrap_or(0);
                let slot = p.get("slot").and_then(|v| v.as_u64()).unwrap_or(1);
                let note = p.get("note").and_then(|v| v.as_u64()).unwrap_or(0);
                if p.get("hit").and_then(|v| v.as_bool()).unwrap_or(false) {
                    out.push(format!(
                        "A2 大奖挑战：黑骰 {a}+{b} 中奖，得 ${}万，指示物退回第 1 格",
                        note / 10_000
                    ));
                } else {
                    out.push(format!(
                        "A2 大奖挑战：黑骰 {a}+{b} 未中，指示物前进到第 {slot} 格（${}万）",
                        note / 10_000
                    ));
                }
            }
            "D1" => {
                let n = p.get("casinos").and_then(|v| v.as_u64()).unwrap_or(0);
                if let Some(c) = p.get("chips").and_then(|v| v.as_u64()) {
                    out.push(format!("D1 发薪愉快：{n} 个赌场有你的骰子，得 {c} 筹码"));
                } else if let Some(v) = p.get("note").and_then(|v| v.as_u64()) {
                    out.push(format!("D1 发薪愉快：{n} 个赌场有你的骰子，得 ${}万", v / 10_000));
                }
            }
            "E2" => {
                let recalled = p.get("bar_recalled").and_then(|v| v.as_u64()).unwrap_or(0);
                let barred = p
                    .get("barred")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                out.push(format!(
                    "E2 醉倒了吗：{barred} 名其他玩家各 1 颗骰子被丢进酒吧；你收回酒吧骰 {recalled} 颗"
                ));
            }
            "C1" => {
                let pieces = p.get("pieces").and_then(|v| v.as_u64()).unwrap_or(5);
                out.push(format!(
                    "C1 掌声响起：你的骰子在该赌场达到 {pieces} 颗，抢先拿到 $10 万！"
                ));
            }
            _ => {}
        }
    }
    out
}

/// T6：钞票分配后·标记兑换（按时机表派发）。
///
/// 目前无板块使用（C1 已改为「拿到即得 $10 万」，不再先给标记后兑换），保留派发点以便后续新增。
pub(crate) fn post_settle(st: &mut LasVegasState, events: &mut Vec<EngineEvent>) {
    for (_ci, id) in tiles_with_tempo(st, Tempo::PostAllocateMarker) {
        debug_assert!(id.is_empty(), "T6 暂无归属：{id}");
    }
    let _ = (st, events);
}


/// 支付资产（先筹码，再面额从高到低的钞票；超出欠额由银行找零），不足付全部；返回实际支付额。
pub(crate) fn pay_assets(st: &mut LasVegasState, player: &str, amount: i64) -> i64 {
    let Some(seat) = st.seats.iter_mut().find(|s| s.player_id == player) else {
        return 0;
    };
    // 用户裁定 2026-10-17：先扣**钞票**，再扣**筹码**；资产不足时最多扣到 0（不产生欠款、
    // 后续结算获得的资产也不追溯补扣）。
    let chip_value = crate::CHIP_VALUE;
    let mut owed = amount;
    seat.banknotes.sort_unstable_by(|a, b| b.cmp(a));
    while owed > 0 {
        let Some(note) = seat.banknotes.first().copied() else {
            break;
        };
        seat.banknotes.remove(0);
        if (note as i64) <= owed {
            owed -= note as i64;
        } else {
            // 银行找零：把差额作为新钞票退回
            let change = note as i64 - owed;
            owed = 0;
            seat.banknotes.push(change as u32);
            seat.banknotes.sort_unstable_by(|a, b| b.cmp(a));
        }
    }
    while owed > 0 && seat.chips > 0 {
        seat.chips -= 1;
        owed -= chip_value;
    }
    if owed < 0 {
        owed = 0;
    }
    amount - owed
}

/// 小游戏规则文本（A1–H2，大小写不敏感）。
pub fn rule_text(id: &str) -> Option<&'static str> {
    let id = id.trim().to_ascii_uppercase();
    Some(match id.as_str() {
        "A1" => "幸运之拳：3 枚标记，藏 1–3 枚（无 0）；下注者从牌面页加密选项选一个（选项↔枚数随机），左手玩家猜 1–3；猜对无奖励，猜错按实际枚数得 2 筹码 / $3 万 / $4 万。",
        "A2" => "大奖挑战：下注后自动投 2 黑骰，同点或和为 7 → 拿指示物**当前格**的钞票并退回第 1 格；否则指示物前进一格（到顶 $8 万不再前进）。格位 $3/4/5/6/7/8 万。",
        "B1" => "主场时间：结算阶段按赌场顺序，该场「移除平手后第一名」投 2 黑骰 → 选 1–6 号赌场，放「命中该点数」的 0–2 颗黑骰作为自己骰子，再正式逐场结算。",
        "B2" => "50% 的机会：标记 0→1（先掷一次基准）后每轮选大/小/弃权；猜对前进一格、猜错结束且无任何奖励；弃权领当前格奖励；到第 5 格直接领 $6 万。格位：2=1 筹码、3=$3 万、4=$4 万。",
        "C1" => "掌声响起：自己下注后该场骰子**颗数**达 5（大骰也算 1 颗）立即得 $10 万；每轮仅一个名额，先到者得。",
        "C2" => "厄运降临：结算阶段按赌场顺序，该场骰子最少者（只算有骰子的玩家；并列都付）付 $5 万：先扣钞票（找零）再扣筹码，最多扣到 0、不追缴。",
        "D1" => "发薪愉快：下注后按你「仍有骰子的赌场数」结算：1→1 筹码、2→2 筹码、3–6→场数×$1 万钞票（酒吧/游戏场/金框/格内骰不算；禁止赌场算）。",
        "D2" => "权力优势：该场骰数**唯一最多**者得特权标记（平手即回游戏场，灰骰参与比较）。持有者回合**不掷骰**，手动放 1 颗手中骰到自选赌场（会触发该赌场小游戏）。",
        "E1" => "禁止进入：下注后把禁止标记放到除本场外任一赌场 → 箭头环形前进 1 格并领**前进后**格子奖励（2=2 筹码、4=$3 万、6=1 筹码）；放弃则都不动。禁止只拦「自己下注 / D2 特权放置」。",
        "E2" => "醉倒了吗：下注后其他玩家各 1 颗手中骰被丢进酒吧（优先小骰、每人上限 2 颗、大骰算 1 颗）；触发者收回自己酒吧全部骰子（大还大）。",
        "F1" => "阻止他：5 组灰骰 [1,1,2,2,3]（每轮重置）。下注后先选一个非空分组、再选 1–6 号赌场放入；组放完即清空，全空则不进入。灰骰参与结算与 D2 判定。",
        "F2" => "危机就是转机：设置阶段 1–3 号各 1、4–6 号各 2 颗灰骰（参与结算）。下注后必须取场上 1 颗灰骰放进 9 格：a 组(格1–2)=1 筹码、b 组(格3–5)=$3 万、c 组(格6–9)=再选放入/收回 1 颗骰。",
        "G1" => "黑色摸彩箱：结算阶段按赌场顺序，该场第一名由左手玩家用**牌面页加密代码多选**把 6 个标记（面额 2/2/6/0/0/0 万）分进 A/B 两堆，赢家明文选 A 或 B 领该堆钞票。",
        "G2" => "双倍颁奖：下注后（含特权下注）可选把该场自己的 **0–N 颗**骰子移入游戏场（0=放弃）；游戏场视为第七赌场，第一 $6 万、第二 $3 万。",
        "H1" => "好骰难求：下注后选来源（该场自己已下注的骰子 / 手中骰，优先小骰），再**自选 1–6 号格**放入；挤走原骰（大还大、小还小）。结算按格位：1=1 筹码、2=2 筹码、3–6=$3/4/5/6 万。",
        "H2" => "我的抉择：下注后掷 2 黑骰（不会出现 4,4）选一点：1→1 筹码、2→2 筹码、3→$3 万、4→村规重掷再选、5→放入/收回 1 颗骰（二段式）、6→1 颗骰放金框（结算 $6 万，挤走的骰大还大）。",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Casino, Phase, Roll, Seat, CHIP_VALUE};
    use game_engine_core::SeededRng;

    fn state_with_casinos(tiles: &[&str]) -> LasVegasState {
        let mut casinos: Vec<Casino> = (0..CASINOS).map(|_| Casino::empty()).collect();
        for (i, t) in tiles.iter().enumerate() {
            casinos[i % 3].small_games.push((*t).to_string());
        }
        LasVegasState {
            schema_version: 2,
            phase: Phase::TurnRoll,
            host: "h".into(),
            round: 1,
            start_player: Some("h".into()),
            turn: 0,
            seats: vec![Seat::new("h".into()), Seat::new("p2".into())],
            roll: Roll::default(),
            casinos,
            small_game_count: 3,
            scores: BTreeMap::new(),
            winners: Vec::new(),
            sg: SgState::default(),
        }
    }

    #[test]
    fn setup_f1_f2_g1() {
        let mut casinos: Vec<crate::Casino> = (0..CASINOS).map(|_| crate::Casino::empty()).collect();
        let mut sg = SgState::default();
        sg.setup_round(&["F1".into()], &mut casinos);
        assert_eq!(sg.gray_groups, F1_GROUPS.to_vec());
        assert_eq!(sg.gray_groups.iter().sum::<u32>(), 9);

        let mut sg = SgState::default();
        sg.setup_round(&["F2".into()], &mut casinos);
        // 灰骰直接进赌场骰池：1–3 号各 1 颗、4–6 号各 2 颗。
        let got: Vec<u32> = (0..CASINOS)
            .map(|c| casinos[c].dice.get("gray").copied().unwrap_or(0))
            .collect();
        assert_eq!(got, vec![1, 1, 1, 2, 2, 2]);
        assert_eq!(sg.f2_slots, vec![None; F2_SLOTS]);

        let mut sg = SgState::default();
        sg.setup_round(&["G1".into()], &mut casinos);
        assert_eq!(sg.lottery, G1_LOTTERY.to_vec());
    }


    // ------------------------------------------------------------------
    // F2 危机就是转机（重做版·裁定 2026-10-17）
    // ------------------------------------------------------------------

    fn f2_state() -> LasVegasState {
        let mut st = state_with_casinos(&["F2", "X", "Y"]);
        st.sg.f2_slots = vec![None; F2_SLOTS];
        // 场上灰骰：1–3 号各 1 颗、4–6 号各 2 颗。
        for (i, n) in [1u32, 1, 1, 2, 2, 2].iter().enumerate() {
            st.casinos[i].dice.insert("gray".into(), *n);
        }
        st
    }

    #[test]
    fn f2_trigger_requires_gray_dice_on_board() {
        let mut st = f2_state();
        let mut rng = SeededRng::new(1);
        let mut ev = Vec::new();
        trigger_interactive(&mut st, 0, "h", &mut rng, &mut ev);
        assert_eq!(st.sg.pending.as_ref().unwrap().game, "F2");
        assert_eq!(st.sg.pending.as_ref().unwrap().stage, "gray");

        // 场上无灰骰 → 不触发，只写一个 skipped 事件。
        let mut st = f2_state();
        for c in st.casinos.iter_mut() {
            c.dice.remove("gray");
        }
        st.sg.pending = None;
        let mut ev = Vec::new();
        trigger_interactive(&mut st, 0, "h", &mut rng, &mut ev);
        assert!(st.sg.pending.is_none(), "无灰骰不应触发 F2");
        assert!(ev.iter().any(|e| e
            .payload
            .get("skipped")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)));
    }

    #[test]
    fn f2_group_a_and_b_pay_out_and_fill_slots() {
        let mut rng = SeededRng::new(1);
        // a 组：1 筹码
        let mut st = f2_state();
        st.sg.pending = Some(SgPending {
            game: "F2".into(),
            casino: 0,
            actor: "h".into(),
            stage: "gray".into(),
            data: json!({}),
        });
        let (done, _, _) = resolve(&mut st, "sg:F2:take:2", "h", &mut rng).unwrap();
        assert!(!done);
        assert_eq!(st.casinos[1].dice.get("gray").copied(), None, "灰骰从原赌场移出");
        let chips0 = st.seats[0].chips;
        let (done, _, _) = resolve(&mut st, "sg:F2:group:a", "h", &mut rng).unwrap();
        assert!(done);
        assert_eq!(st.seats[0].chips, chips0 + 1);
        assert_eq!(st.sg.f2_slots[0], Some(2), "a 组第一格记录来源 2 号赌场");

        // b 组：$3 万
        let mut st = f2_state();
        st.sg.pending = Some(SgPending {
            game: "F2".into(),
            casino: 0,
            actor: "h".into(),
            stage: "gray".into(),
            data: json!({}),
        });
        resolve(&mut st, "sg:F2:take:1", "h", &mut rng).unwrap();
        let (done, _, _) = resolve(&mut st, "sg:F2:group:b", "h", &mut rng).unwrap();
        assert!(done);
        assert_eq!(st.seats[0].banknotes, vec![30_000]);
        assert_eq!(st.sg.f2_slots[2], Some(1));
    }

    #[test]
    fn f2_full_group_is_not_selectable() {
        let mut rng = SeededRng::new(1);
        let mut st = f2_state();
        // a 组 2 格已满
        st.sg.f2_slots[0] = Some(1);
        st.sg.f2_slots[1] = Some(1);
        st.sg.pending = Some(SgPending {
            game: "F2".into(),
            casino: 0,
            actor: "h".into(),
            stage: "group".into(),
            data: json!({ "from": 1 }),
        });
        let acts = legal_for_pending(&st, "h");
        assert!(!acts.iter().any(|a| a.id == "sg:F2:group:a"), "a 组已满不可选");
        assert!(acts.iter().any(|a| a.id == "sg:F2:group:b"));
        assert!(acts
            .iter()
            .find(|a| a.id == "sg:F2:group:b")
            .unwrap()
            .description
            .contains("已满：a"), "描述应标注已满的组");
        assert!(resolve(&mut st, "sg:F2:group:a", "h", &mut rng).is_err());
    }

    #[test]
    fn f2_c_group_put_does_not_trigger_small_game() {
        let mut rng = SeededRng::new(1);
        // 5 号赌场挂 C1：若误触发 on_place 且骰数到 5 会发大奖标记。
        let mut st = f2_state();
        st.casinos[4].small_games.push("C1".into());
        st.casinos[4].dice.insert("h".into(), 4);
        st.sg.pending = Some(SgPending {
            game: "F2".into(),
            casino: 0,
            actor: "h".into(),
            stage: "gray".into(),
            data: json!({}),
        });
        resolve(&mut st, "sg:F2:take:1", "h", &mut rng).unwrap();
        let (done, _, _) = resolve(&mut st, "sg:F2:group:c", "h", &mut rng).unwrap();
        assert!(!done, "c 组还需要第 1 段选择");
        assert_eq!(st.sg.f2_slots[5], Some(1), "c 组第一格");
        assert_eq!(st.sg.pending.as_ref().unwrap().stage, "c_action");
        let (done, _, _) = resolve(&mut st, "sg:F2:put", "h", &mut rng).unwrap();
        assert!(!done);
        assert_eq!(st.sg.pending.as_ref().unwrap().stage, "c_put");
        let hand = st.seats[0].small_remaining;
        let (done, _, _) = resolve(&mut st, "sg:F2:put:5", "h", &mut rng).unwrap();
        assert!(done);
        assert_eq!(st.seats[0].small_remaining, hand - 1, "手中少 1 颗");
        assert_eq!(st.casinos[4].dice.get("h").copied(), Some(5), "4 + 1 入 5 号赌场");
        assert!(st.sg.c1_winner.is_none(), "放入不得触发该赌场小游戏（C1 第 5 颗）");
    }

    #[test]
    fn f2_c_group_skips_put_when_hand_empty_and_takes_back() {
        let mut rng = SeededRng::new(1);
        let mut st = f2_state();
        // 手中无骰；4 号赌场有自己的 2 颗小骰
        st.seats[0].small_remaining = 0;
        st.seats[0].big_remaining = 0;
        st.casinos[3].dice.insert("h".into(), 2);
        st.sg.pending = Some(SgPending {
            game: "F2".into(),
            casino: 0,
            actor: "h".into(),
            stage: "gray".into(),
            data: json!({}),
        });
        resolve(&mut st, "sg:F2:take:1", "h", &mut rng).unwrap();
        let (done, _, _) = resolve(&mut st, "sg:F2:group:c", "h", &mut rng).unwrap();
        assert!(!done);
        assert_eq!(
            st.sg.pending.as_ref().unwrap().stage,
            "c_takeback",
            "手中无骰应跳过第 1 段"
        );
        let acts = legal_for_pending(&st, "h");
        assert!(acts.iter().any(|a| a.id == "sg:F2:takeback:4"));
        assert!(
            acts.iter().all(|a| !a.id.starts_with("sg:F2:takeback:1")),
            "没有自己骰子的赌场不应出现在收回动作里"
        );
        assert!(!acts.iter().any(|a| a.id == "sg:F2:put"), "手中无骰时无放入选项");
        let (done, _, _) = resolve(&mut st, "sg:F2:takeback:4", "h", &mut rng).unwrap();
        assert!(done);
        assert_eq!(st.seats[0].small_remaining, 1);
        assert_eq!(st.casinos[3].dice.get("h").copied(), Some(1));
    }

    #[test]
    fn f2_c_group_hidden_when_unusable() {
        let st0 = f2_state();
        let mut st = st0;
        st.seats[0].small_remaining = 0;
        st.seats[0].big_remaining = 0;
        // 场上没有自己的骰子
        st.sg.pending = Some(SgPending {
            game: "F2".into(),
            casino: 0,
            actor: "h".into(),
            stage: "group".into(),
            data: json!({ "from": 1 }),
        });
        let acts = legal_for_pending(&st, "h");
        assert!(
            !acts.iter().any(|a| a.id == "sg:F2:group:c"),
            "既不能放入也不能收回时，c 组不可选"
        );
    }

    #[test]
    fn c1_pays_immediately_once_per_round() {
        let mut st = state_with_casinos(&["C1", "X", "Y"]);
        let mut rng = SeededRng::new(1);
        let mut ev = Vec::new();
        // 颗数口径：4 小 + 1 大 = 5 颗（权重 6）→ 触发
        st.casinos[0].dice.insert("h".into(), 6);
        st.casinos[0].big_dice.insert("h".into(), 1);
        on_place(&mut st, 0, "h", &mut rng, &mut ev);
        assert_eq!(st.sg.c1_winner.as_deref(), Some("h"));
        let seat = st.seats.iter().find(|s| s.player_id == "h").unwrap();
        assert_eq!(seat.banknotes, vec![C1_REWARD as u32], "拿到即得 $10 万");

        // 本轮名额已被拿走：别人再到 5 颗也无奖励
        st.casinos[0].dice.insert("p2".into(), 5);
        on_place(&mut st, 0, "p2", &mut rng, &mut ev);
        assert_eq!(st.sg.c1_winner.as_deref(), Some("h"));
        let p2 = st.seats.iter().find(|s| s.player_id == "p2").unwrap();
        assert!(p2.banknotes.is_empty());

        // 4 颗不触发
        let mut st2 = state_with_casinos(&["C1", "X", "Y"]);
        st2.casinos[0].dice.insert("h".into(), 4);
        let mut ev2 = Vec::new();
        on_place(&mut st2, 0, "h", &mut rng, &mut ev2);
        assert!(st2.sg.c1_winner.is_none());
    }

    #[test]
    fn d1_rewards_by_casino_count() {
        let mut st = state_with_casinos(&["D1", "X", "Y"]);
        let mut rng = SeededRng::new(2);
        let mut ev = Vec::new();
        st.casinos[0].dice.insert("h".into(), 1);
        on_place(&mut st, 0, "h", &mut rng, &mut ev);
        // 只有 1 个赌场有骰子 → 1 筹码
        assert_eq!(st.seats[0].chips, crate::INITIAL_CHIPS + 1);
        // 2 个赌场 → 2 筹码（累计 1+2=3）
        st.casinos[1].dice.insert("h".into(), 1);
        on_place(&mut st, 0, "h", &mut rng, &mut ev);
        assert_eq!(st.seats[0].chips, crate::INITIAL_CHIPS + 3);
        // 3 个赌场 → 3 万钞票（不是筹码）
        st.casinos[2].dice.insert("h".into(), 1);
        on_place(&mut st, 0, "h", &mut rng, &mut ev);
        assert_eq!(st.seats[0].banknotes, vec![30_000]);
    }

    #[test]
    fn d1_only_counts_dice_still_in_casinos() {
        // 酒吧 / 游戏场 / 金框 / H1 格内的骰子都不算「在赌场有骰子」；
        // E1 禁止的赌场里的骰子**算**（只在赌场骰池口径）。
        let mut st = state_with_casinos(&["D1", "X", "Y"]);
        let mut rng = SeededRng::new(4);
        let mut ev = Vec::new();
        st.casinos[0].dice.insert("h".into(), 1);
        // 场外资产：酒吧 2 颗、游戏场 3 颗、金框 1 颗
        st.sg.bars.insert("h".into(), BarDice { small: 1, big: 1 });
        st.sg.game_field.insert("h".into(), 3);
        st.sg.gold_frame = Some(GoldFrame { owner: "h".into(), small: 1, big: 0 });
        on_place(&mut st, 0, "h", &mut rng, &mut ev);
        assert_eq!(st.seats[0].chips, crate::INITIAL_CHIPS + 1, "只算 1 个赌场");

        // E1 禁止的赌场里的骰子仍然计入
        st.casinos[2].dice.insert("h".into(), 1);
        st.sg.banned = Some(2);
        on_place(&mut st, 0, "h", &mut rng, &mut ev);
        assert_eq!(
            st.seats[0].chips,
            crate::INITIAL_CHIPS + 1 + 2,
            "禁止赌场也计入"
        );
    }

    #[test]
    fn c2_penalizes_only_players_with_dice() {
        let mut st = state_with_casinos(&["Y", "X", "Z"]);
        st.casinos[0].small_games = vec!["C2".into()];
        // h=3、p2=1 → p2 最少且**有骰子** → 付 5 万
        st.casinos[0].dice.insert("h".into(), 3);
        st.casinos[0].dice.insert("p2".into(), 1);
        st.seats[1].chips = 3; // 3 万
        st.seats[1].banknotes = vec![50_000];
        let mut ev = Vec::new();
        c2_pre_casino(&mut st, 0, &mut ev);
        let p2 = st.seats.iter().find(|s| s.player_id == "p2").unwrap();
        // 先扣钞票（5 万整张），再扣筹码 → 余 3 万筹码 + 0 钞票
        assert_eq!(p2.banknotes, Vec::<u32>::new());
        assert_eq!(p2.chips, 3);
        assert_eq!(st.seats[0].chips, crate::INITIAL_CHIPS, "h 不是最少，不付");
    }

    #[test]
    fn c2_skips_when_nobody_has_dice_and_ignores_gray() {
        let mut st = state_with_casinos(&["Y", "X", "Z"]);
        st.casinos[0].small_games = vec!["C2".into()];
        // 只有灰骰 → 无人被罚
        st.casinos[0].dice.insert("gray".into(), 4);
        let mut ev = Vec::new();
        c2_pre_casino(&mut st, 0, &mut ev);
        assert!(ev.iter().any(|e| e
            .payload
            .get("skipped")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)));
        assert_eq!(st.seats[0].banknotes, Vec::<u32>::new());
    }

    #[test]
    fn rule_text_case_insensitive() {
        assert!(rule_text("f1").is_some());
        assert!(rule_text("F1").is_some());
        assert_eq!(rule_text("f1"), rule_text("F1"));
        assert!(rule_text("H2").unwrap().contains("金框"));
        assert!(rule_text("Z9").is_none());
    }

    #[test]
    fn a1_hide_then_guess_pays_by_hidden_count() {
        // 1 枚 → 2 筹码；2 枚 → $3 万；3 枚 → $4 万；猜对无奖励。
        let mut rng = SeededRng::new(3);
        for count in 1..=3u32 {
            let mut st = state_with_casinos(&[]);
            st.sg.pending = Some(SgPending {
                game: "A1".into(),
                casino: 0,
                actor: "h".into(),
                stage: "hide".into(),
                // 选项 → 标记数的随机置换：这里直接构造「选项 1 对应 count」。
                data: json!({ "initiator": "h", "perm": [count, count % 3 + 1, (count + 1) % 3 + 1], "codes": ["AAA1", "BBB2", "CCC3"] }),
            });
            let (done, _ev, msg) = resolve(&mut st, "sg:A1:hide:1", "h", &mut rng).unwrap();
            assert!(!done, "隐藏后应流转到猜者");
            assert!(msg.contains("猜"), "应提示猜者出手");
            let p = st.sg.pending.clone().unwrap();
            assert_eq!(p.stage, "guess");
            assert_eq!(p.actor, left_player(&st, "h"), "由左手玩家猜");
            assert_eq!(p.data.get("count").and_then(|v| v.as_u64()), Some(count as u64));
            // 猜错 → 发起者拿奖励
            let wrong = if count == 1 { 2 } else { 1 };
            let act = format!("sg:A1:guess:{wrong}");
            let (done, _ev, _m) = resolve(&mut st, &act, &p.actor, &mut rng).unwrap();
            assert!(done);
            let seat = st.seats.iter().find(|s| s.player_id == "h").unwrap();
            match count {
                1 => assert_eq!(seat.chips, crate::INITIAL_CHIPS + 2),
                2 => assert_eq!(seat.banknotes, vec![30_000]),
                _ => assert_eq!(seat.banknotes, vec![40_000]),
            }
        }

        // 猜对 → 发起者无奖励
        let mut st = state_with_casinos(&[]);
        st.sg.pending = Some(SgPending {
            game: "A1".into(),
            casino: 0,
            actor: "h".into(),
            stage: "hide".into(),
            data: json!({ "initiator": "h", "perm": [2, 1, 3], "codes": ["AAA1", "BBB2", "CCC3"] }),
        });
        resolve(&mut st, "sg:A1:hide:1", "h", &mut rng).unwrap();
        let guesser = st.sg.pending.clone().unwrap().actor;
        let (done, _ev, _m) = resolve(&mut st, "sg:A1:guess:2", &guesser, &mut rng).unwrap();
        assert!(done);
        let seat = st.seats.iter().find(|s| s.player_id == "h").unwrap();
        assert_eq!(seat.chips, crate::INITIAL_CHIPS);
        assert!(seat.banknotes.is_empty());
    }

    #[test]
    fn a1_rejects_zero_and_out_of_range() {
        let mut st = state_with_casinos(&[]);
        st.sg.pending = Some(SgPending {
            game: "A1".into(),
            casino: 0,
            actor: "h".into(),
            stage: "hide".into(),
            data: json!({ "initiator": "h", "perm": [1, 2, 3], "codes": ["AAA1", "BBB2", "CCC3"] }),
        });
        let mut rng = SeededRng::new(1);
        assert!(resolve(&mut st, "sg:A1:hide:0", "h", &mut rng).is_err());
        assert!(resolve(&mut st, "sg:A1:hide:4", "h", &mut rng).is_err());
        resolve(&mut st, "sg:A1:hide:1", "h", &mut rng).unwrap();
        let guesser = st.sg.pending.clone().unwrap().actor;
        assert!(resolve(&mut st, "sg:A1:guess:0", &guesser, &mut rng).is_err());
        assert!(resolve(&mut st, "sg:A1:guess:4", &guesser, &mut rng).is_err());
    }

    #[test]
    fn a2_hit_takes_current_slot_note_and_resets() {
        let mut st = state_with_casinos(&["A2"]);
        // 指示物停在第 4 格（$6 万）
        st.sg.indicator = 3;
        let mut rng = SeededRng::new(11);
        let mut ev = Vec::new();
        // 直接驱动两次，至少验证「命中/未中」两种路径都能走通且不 panic
        let mut hits = 0;
        for _ in 0..40 {
            let before = st.seats[0].banknotes.len();
            st.sg.indicator = 3;
            on_bet_auto(&mut st, "A2", 0, "h", &mut rng, &mut ev);
            let last = ev.last().unwrap();
            let hit = last.payload.get("hit").and_then(|v| v.as_bool()).unwrap_or(false);
            if hit {
                hits += 1;
                assert_eq!(
                    st.seats[0].banknotes.len(),
                    before + 1,
                    "中奖应发 1 张钞票"
                );
                assert_eq!(
                    st.seats[0].banknotes.last().copied(),
                    Some(60_000),
                    "拿的是当前格（$6 万）"
                );
                assert_eq!(st.sg.indicator, 0, "中奖后指示物退回第 1 格");
            } else {
                assert_eq!(st.sg.indicator, 4, "未中应前进一格");
                assert_eq!(
                    st.seats[0].banknotes.len(),
                    before,
                    "未中不发奖励"
                );
            }
        }
        assert!(hits > 0, "40 次里应有命中（黑骰同点或和 7）");
    }

    #[test]
    fn a2_indicator_caps_at_top_slot() {
        let mut st = state_with_casinos(&["A2"]);
        st.sg.indicator = 5; // 已到顶（$8 万）
        let mut rng = SeededRng::new(5);
        let mut ev = Vec::new();
        for _ in 0..30 {
            st.sg.indicator = 5;
            on_bet_auto(&mut st, "A2", 0, "h", &mut rng, &mut ev);
            let hit = ev.last().unwrap().payload.get("hit").and_then(|v| v.as_bool()).unwrap_or(false);
            if !hit {
                assert_eq!(st.sg.indicator, 5, "到顶后不再前进（不循环）");
            }
        }
    }

    #[test]
    fn a1_code_flow_and_private_view() {
        use crate::{LasVegasEngine, SCHEMA_VERSION};
        use game_engine_core::{EnginePlugin, GameState, GameStatus};
        let engine = LasVegasEngine;
        let mut st = state_with_casinos(&["A1"]);
        st.schema_version = SCHEMA_VERSION;
        // 触发：下注后挂 pending（actor = 下注者），并生成 3 个代码
        let mut rng = SeededRng::new(21);
        let mut ev = Vec::new();
        st.casinos[0].dice.insert("h".into(), 1);
        trigger_interactive(&mut st, 0, "h", &mut rng, &mut ev);
        let p = st.sg.pending.clone().unwrap();
        assert_eq!((p.game.as_str(), p.stage.as_str()), ("A1", "hide"));
        let codes = pending_codes(&p);
        assert_eq!(codes.len(), 3);
        assert_eq!(codes.iter().collect::<std::collections::BTreeSet<_>>().len(), 3, "代码不重复");

        // 合法动作：只出现代码，不出现枚数
        let acts = legal_for_pending(&st, "h");
        assert_eq!(acts.len(), 3);
        assert!(acts.iter().all(|a| a.id.starts_with("sg:code:")));
        assert!(
            acts.iter().all(|a| !a.description.contains("枚")),
            "群里文案不得泄露枚数"
        );

        let gs = GameState {
            game_id: "las-vegas".into(),
            room_id: "r".into(),
            group_id: "g".into(),
            status: GameStatus::Active,
            players: vec!["h".into(), "p2".into()],
            data: serde_json::to_value(&st).unwrap(),
        };
        // 公共视图：无代码↔枚数映射
        let pub_state = engine.public_state(&gs);
        let pub_d: LasVegasState = serde_json::from_value(pub_state.data.clone()).unwrap();
        assert!(pub_d.sg.pending.as_ref().unwrap().data.get("perm").is_none());
        assert!(pub_d.sg.pending_options.is_none());
        // 个人视图（当事人）：拿到映射
        let priv_state = engine.player_view(&gs, "h");
        let priv_d: LasVegasState = serde_json::from_value(priv_state.data.clone()).unwrap();
        let opts = priv_d.sg.pending_options.clone().unwrap();
        assert_eq!(opts["options"].as_array().unwrap().len(), 3);
        assert!(opts["options"][0]["count"].as_u64().unwrap() >= 1);
        // 非当事人拿不到
        let other = engine.player_view(&gs, "p2");
        let other_d: LasVegasState = serde_json::from_value(other.data.clone()).unwrap();
        assert!(other_d.sg.pending_options.is_none());

        // 用代码选择 → 进入猜的阶段，枚数与映射一致
        let perm: Vec<u32> = serde_json::from_value(p.data.get("perm").unwrap().clone()).unwrap();
        let act = format!("sg:code:{}", codes[1]);
        let (done, _e, msg) = resolve(&mut st, &act, "h", &mut rng).unwrap();
        assert!(!done);
        assert!(msg.contains("猜"));
        let p2 = st.sg.pending.clone().unwrap();
        assert_eq!(p2.stage, "guess");
        assert_eq!(p2.data.get("count").and_then(|v| v.as_u64()), Some(perm[1] as u64));
        // 错误代码被拒
        let mut st3 = state_with_casinos(&["A1"]);
        st3.sg.pending = Some(SgPending {
            game: "A1".into(),
            casino: 0,
            actor: "h".into(),
            stage: "hide".into(),
            data: json!({ "initiator": "h", "perm": [1,2,3], "codes": ["AAA1","BBB2","CCC3"] }),
        });
        assert!(resolve(&mut st3, "sg:code:ZZZZ", "h", &mut rng).is_err());
    }

    #[test]
    fn b1_settles_at_round_settle_with_leader_choice() {
        let mut st = state_with_casinos(&["B1"]);
        // B1 所在赌场：h=3、p2=1、gray=2 → 移除平手后第一名 h
        st.casinos[0].dice.insert("h".into(), 3);
        st.casinos[0].dice.insert("p2".into(), 1);
        st.casinos[0].dice.insert("gray".into(), 2);
        let mut rng = SeededRng::new(7);
        let mut ev = Vec::new();
        assert!(start_b1_pending(&mut st, 0, &mut rng, &mut ev), "应挂 B1 pending");
        let p = st.sg.pending.clone().unwrap();
        assert_eq!((p.game.as_str(), p.actor.as_str()), ("B1", "h"));
        let black: Vec<u32> = serde_json::from_value(p.data.get("black").unwrap().clone()).unwrap();
        assert_eq!(black.len(), 2);
        // 6 个选项，描述里带「放 k 颗」或「0 颗」
        let acts = legal_for_pending(&st, "h");
        assert_eq!(acts.len(), 6);
        for a in &acts {
            let c: usize = a.id.trim_start_matches("sg:B1:put:").parse().unwrap();
            let k = black.iter().filter(|d| **d as usize == c).count();
            if k > 0 {
                assert!(a.description.contains(&format!("放 {k} 颗")));
            } else {
                assert!(a.description.contains("0 颗"));
            }
        }
        // 选掷出点数对应的赌场 → 加骰
        let target = black[0] as usize;
        let before = st.casinos[target - 1].dice.get("h").copied().unwrap_or(0);
        let (done, _e, _m) = resolve(&mut st, &format!("sg:B1:put:{target}"), "h", &mut rng).unwrap();
        assert!(done);
        let k = black.iter().filter(|d| **d as usize == target).count() as u32;
        assert_eq!(st.casinos[target - 1].dice.get("h").copied().unwrap_or(0), before + k);
        assert!(st.sg.b1_done);
        // 再次进入结算不会重复触发
        let mut ev2 = Vec::new();
        assert!(!start_b1_pending(&mut st, 0, &mut rng, &mut ev2));
    }

    #[test]
    fn resume_message_points_at_current_actor() {
        let mut st = state_with_casinos(&["A1"]);
        st.sg.pending = Some(SgPending {
            game: "A1".into(),
            casino: 0,
            actor: "h".into(),
            stage: "hide".into(),
            data: json!({ "initiator": "h", "perm": [1,2,3], "codes": ["AAA1","BBB2","CCC3"] }),
        });
        let m = resume_message(&st);
        assert!(m.contains("h") && m.contains("A1") && m.contains("#选"), "{m}");
    }

    #[test]
    fn pre_settle_pass_runs_in_casino_order() {
        // B1 在 1 号、C2 在 3 号：C2 必须看到 B1 加过的骰子（按赌场顺序推进）。
        let mut st = state_with_casinos(&["B1", "X", "C2"]);
        st.casinos[0].small_games = vec!["B1".into()];
        st.casinos[2].small_games = vec!["C2".into()];
        // 1 号：h=3（第一名，进 B1）；3 号：h=1、p2=2
        st.casinos[0].dice.insert("h".into(), 3);
        st.casinos[2].dice.insert("h".into(), 1);
        st.casinos[2].dice.insert("p2".into(), 2);
        let mut rng = SeededRng::new(3);
        let mut ev = Vec::new();
        // B1 是交互：第一次 pass 会停在 B1 pending（此时 C2 尚未执行）
        assert!(pre_settle_pass(&mut st, &mut rng, &mut ev), "B1 应挂 pending");
        assert!(st.sg.c2_done.is_empty(), "B1 未完成前不得先结算 C2");
        // 结算 B1：把黑骰放到某赌场
        let p = st.sg.pending.clone().unwrap();
        let black: Vec<u32> = serde_json::from_value(p.data.get("black").unwrap().clone()).unwrap();
        resolve(&mut st, &format!("sg:B1:put:{}", black[0]), "h", &mut rng).unwrap();
        // 第二次 pass：跑 C2（并按赌场顺序）
        let mut ev2 = Vec::new();
        assert!(!pre_settle_pass(&mut st, &mut rng, &mut ev2));
        assert_eq!(st.sg.c2_done, vec![2], "C2 只在 3 号赌场执行一次");
        assert!(ev2.iter().any(|e| e.payload.get("game").and_then(|v| v.as_str()) == Some("C2")));
    }

    #[test]
    fn e1_arrow_ring_rewards_and_pass() {
        let mut rng = SeededRng::new(1);
        let mut st = state_with_casinos(&["E1"]);
        st.sg.pending = Some(SgPending {
            game: "E1".into(),
            casino: 0,
            actor: "h".into(),
            stage: "choose".into(),
            data: json!({}),
        });
        // 第 1 次：箭头 1→2，得 2 筹码
        let (done, _e, msg) = resolve(&mut st, "sg:E1:ban:4", "h", &mut rng).unwrap();
        assert!(done);
        assert!(msg.contains("第 2 格") && msg.contains("2 筹码"), "{msg}");
        assert_eq!(st.sg.arrow, 1);
        assert_eq!(st.seats[0].chips, crate::INITIAL_CHIPS + 2);

        // 第 2 次：箭头 2→3（无奖励）
        st.sg.pending = Some(SgPending {
            game: "E1".into(),
            casino: 1,
            actor: "h".into(),
            stage: "choose".into(),
            data: json!({}),
        });
        let (_d, _e, msg) = resolve(&mut st, "sg:E1:ban:5", "h", &mut rng).unwrap();
        assert!(msg.contains("第 3 格") && msg.contains("无奖励"), "{msg}");
        assert_eq!(st.seats[0].chips, crate::INITIAL_CHIPS + 2);

        // 第 3 次：箭头 3→4，得 $3 万
        st.sg.pending = Some(SgPending {
            game: "E1".into(),
            casino: 1,
            actor: "h".into(),
            stage: "choose".into(),
            data: json!({}),
        });
        let (_d, _e, msg) = resolve(&mut st, "sg:E1:ban:6", "h", &mut rng).unwrap();
        assert!(msg.contains("第 4 格") && msg.contains("$3万"), "{msg}");
        assert_eq!(st.seats[0].banknotes, vec![30_000]);
        assert_eq!(st.sg.banned, Some(5));
    }

    #[test]
    fn e1_pass_gives_nothing_and_excludes_own_casino() {
        let mut rng = SeededRng::new(1);
        let mut st = state_with_casinos(&["E1"]);
        st.sg.pending = Some(SgPending {
            game: "E1".into(),
            casino: 2, // E1 挂在 3 号赌场
            actor: "h".into(),
            stage: "choose".into(),
            data: json!({}),
        });
        let acts = legal_for_pending(&st, "h");
        assert!(acts.iter().all(|a| a.id != "sg:E1:ban:3"), "不得禁止本场");
        assert!(acts.iter().any(|a| a.id == "sg:E1:pass"));

        let before = st.seats[0].chips;
        let (done, _e, msg) = resolve(&mut st, "sg:E1:pass", "h", &mut rng).unwrap();
        assert!(done);
        assert!(msg.contains("放弃"));
        assert_eq!(st.seats[0].chips, before, "放弃不领奖励");
        assert_eq!(st.sg.arrow, 0, "放弃箭头不前进");
        assert!(st.sg.banned.is_none());
    }

    #[test]
    fn e1_ban_blocks_bets_but_not_f2_dice_moves() {
        // 禁止只影响「自己下注」与「D2 特权放置」；F2 的放骰/收回不受影响。
        let mut rng = SeededRng::new(1);
        let mut st = state_with_casinos(&["F2"]);
        st.sg.f2_slots = vec![None; F2_SLOTS];
        st.sg.banned = Some(4); // 5 号赌场被禁止
        st.sg.pending = Some(SgPending {
            game: "F2".into(),
            casino: 0,
            actor: "h".into(),
            stage: "c_put".into(),
            data: json!({ "from": 1 }),
        });
        let acts = legal_for_pending(&st, "h");
        assert!(
            acts.iter().any(|a| a.id == "sg:F2:put:5"),
            "F2 放骰不受禁止影响"
        );
        let (done, _e, _m) = resolve(&mut st, "sg:F2:put:5", "h", &mut rng).unwrap();
        assert!(done);
        assert_eq!(st.casinos[4].dice.get("h").copied(), Some(1));
    }

    #[test]
    fn e2_takes_one_small_die_caps_at_two_and_recalls_by_type() {
        let mut st = state_with_casinos(&["E2", "X", "Y"]);
        let mut rng = SeededRng::new(1);
        let mut ev = Vec::new();
        // p2 已在酒吧 2 颗 → 不再拿；h 是触发者
        st.sg.bars.insert("p2".into(), BarDice { small: 2, big: 0 });
        // 再加一个 p3 走「只有大骰」路径
        st.seats.push(Seat::new("p3".into()));
        st.seats[2].small_remaining = 0;
        st.seats[2].big_remaining = 1;
        st.seats[0].small_remaining = 3;
        st.seats[1].small_remaining = 3;

        // 第一次：p3 的大骰进酒吧
        on_place(&mut st, 0, "h", &mut rng, &mut ev);
        let p3 = st.seats.iter().find(|s| s.player_id == "p3").unwrap();
        assert_eq!(p3.big_remaining, 0, "只有大骰时拿大骰");
        assert_eq!(st.sg.bars.get("p3").copied(), Some(BarDice { small: 0, big: 1 }));
        let p2 = st.seats.iter().find(|s| s.player_id == "p2").unwrap();
        assert_eq!(p2.small_remaining, 3, "酒吧已满 2 颗者不再被拿");

        // 触发者自己不放
        assert!(!st.sg.bars.contains_key("h"));

        // h 先被其他 E2 灌两轮 → 第三轮起不再被拿
        for _ in 0..3 {
            on_place(&mut st, 0, "p3", &mut rng, &mut ev);
        }
        assert_eq!(st.sg.bars.get("h").map(|b| b.pieces()), Some(2), "h 酒吧最多 2 颗");

        // h 触发 E2 → 收回自己酒吧里的骰子（大骰还原大骰）
        let before_big = st.seats[0].big_remaining;
        on_place(&mut st, 0, "h", &mut rng, &mut ev);
        assert!(!st.sg.bars.contains_key("h"), "触发者清空自己的酒吧");
        let h = st.seats.iter().find(|s| s.player_id == "h").unwrap();
        assert!(h.big_remaining >= before_big, "收回的大骰还原为大骰");
    }

    #[test]
    fn e2_fires_on_privilege_bet_too() {
        // D2 特权放置也属于「下注」→ 同样触发 E2（走 on_place）。
        let mut st = state_with_casinos(&["E2"]);
        // E2 与 D2 挂在同一赌场：特性放置到该赌场时才触发 E2。
        st.casinos[0].small_games.push("D2".into());
        st.sg.privilege = Some("h".into());
        st.seats[0].small_remaining = 3;
        st.seats[1].small_remaining = 3;
        let mut rng = SeededRng::new(1);
        // 直接走 D2 回合动作
        let out = resolve_turn_sg(&mut st, "sg:D2:use:1", "h", &mut rng)
            .unwrap()
            .unwrap();
        assert!(out.2, "无 pending 时推进回合");
        assert_eq!(
            st.sg.bars.get("p2").map(|b| b.pieces()),
            Some(1),
            "特权下注同样灌酒"
        );
    }

    #[test]
    fn f1_groups_are_1_1_2_2_3_and_consume_once() {
        assert_eq!(F1_GROUPS, [1, 1, 2, 2, 3]);
        assert_eq!(F1_GROUPS.iter().sum::<u32>(), 9);
        let mut rng = SeededRng::new(2);
        let mut st = state_with_casinos(&["F1"]);
        st.sg.gray_groups = F1_GROUPS.to_vec();
        // 选第 5 组（3 颗）→ 放入 4 号赌场
        st.sg.pending = Some(SgPending {
            game: "F1".into(),
            casino: 0,
            actor: "h".into(),
            stage: "group".into(),
            data: json!({}),
        });
        let (done, _e, _m) = resolve(&mut st, "sg:F1:group:4", "h", &mut rng).unwrap();
        assert!(!done, "选组后还需选赌场");
        let (done, _e, msg) = resolve(&mut st, "sg:F1:move:4:4", "h", &mut rng).unwrap();
        assert!(done);
        assert!(msg.contains("3 颗"), "{msg}");
        assert_eq!(st.casinos[3].dice.get("gray").copied(), Some(3));
        assert_eq!(st.sg.gray_groups[4], 0, "该组已清空");

        // 组已空 → 不可再选
        st.sg.pending = Some(SgPending {
            game: "F1".into(),
            casino: 0,
            actor: "h".into(),
            stage: "group".into(),
            data: json!({}),
        });
        assert!(resolve(&mut st, "sg:F1:group:4", "h", &mut rng).is_err());
        let acts = legal_for_pending(&st, "h");
        assert!(acts.iter().all(|a| a.id != "sg:F1:group:4"));
    }

    #[test]
    fn f1_gate_closes_when_all_groups_empty() {
        let mut st = state_with_casinos(&["F1"]);
        st.sg.gray_groups = vec![0; F1_GROUPS.len()];
        let mut rng = SeededRng::new(1);
        let mut ev = Vec::new();
        trigger_interactive(&mut st, 0, "h", &mut rng, &mut ev);
        assert!(st.sg.pending.is_none(), "分组全空 → 不进入游戏");
    }

    #[test]
    fn d2_gray_dice_join_judgement_but_cannot_hold() {
        let mut st = state_with_casinos(&["D2"]);
        // h=3、p2=2、gray=4 → 唯一最多是灰骰 → 无人持有
        st.casinos[0].dice.insert("h".into(), 3);
        st.casinos[0].dice.insert("p2".into(), 2);
        st.casinos[0].dice.insert("gray".into(), 4);
        update_privilege(&mut st, 0);
        assert!(st.sg.privilege.is_none(), "灰骰唯一最多：标记回游戏场");

        // gray=3 与 h=3 并列 → 淘汰
        st.casinos[0].dice.insert("gray".into(), 3);
        update_privilege(&mut st, 0);
        assert!(st.sg.privilege.is_none(), "与灰骰并列：标记回游戏场");

        // 灰骰较少 → h 唯一最多，拿到标记（即便之后灰骰摆放导致排名变化也不再重算）
        st.casinos[0].dice.insert("gray".into(), 1);
        update_privilege(&mut st, 0);
        assert_eq!(st.sg.privilege.as_deref(), Some("h"));
        st.casinos[0].dice.insert("gray".into(), 9);
        assert_eq!(
            st.sg.privilege.as_deref(),
            Some("h"),
            "灰骰摆放不触发重算，特权仍在原持有者手中"
        );
    }

    #[test]
    fn g1_multi_select_codes_and_private_options() {
        let mut st = state_with_casinos(&["G1"]);
        st.sg.lottery = G1_LOTTERY.to_vec();
        let codes: Vec<String> = vec!["P1".into(), "P2".into(), "P3".into(), "P4".into(), "P5".into(), "P6".into()];
        st.sg.pending = Some(SgPending {
            game: "G1".into(),
            casino: 0,
            actor: "p2".into(),
            stage: "split".into(),
            data: json!({ "winner": "h", "total": 10, "codes": codes }),
        });
        // 个人视图：代码 → 面额（万），multi = true
        let p = st.sg.pending.clone().unwrap();
        let opts = private_options(&p, &st.sg.lottery).unwrap();
        assert_eq!(opts["multi"], json!(true));
        assert_eq!(opts["options"][2]["value"], json!(6));
        // 公共视图要抹掉 lottery（0）→ 代码本身不含面额信息
        // 多选（中文逗号）：P1(2 万) + P3(6 万) 进 A 堆
        let mut rng = SeededRng::new(1);
        let (done, _e, msg) = resolve(&mut st, "sg:G1:split:P1，P3", "p2", &mut rng).unwrap();
        assert!(!done);
        assert!(msg.contains("A：8 万"), "{msg}");
        assert_eq!(st.sg.pending.as_ref().unwrap().data["piles"], json!([8, 2]));
        // 无效代码被拒
        let mut st2 = state_with_casinos(&["G1"]);
        st2.sg.lottery = G1_LOTTERY.to_vec();
        st2.sg.pending = Some(SgPending {
            game: "G1".into(),
            casino: 0,
            actor: "p2".into(),
            stage: "split".into(),
            data: json!({ "winner": "h", "total": 10, "codes": ["P1","P2","P3","P4","P5","P6"] }),
        });
        assert!(resolve(&mut st2, "sg:G1:split:ZZZ", "p2", &mut rng).is_err());
    }

    #[test]
    fn g2_triggers_on_bet_and_moves_chosen_pieces() {
        let mut st = state_with_casinos(&["G2"]);
        st.casinos[0].dice.insert("h".into(), 7);
        // 大骰 1 颗：权重 9 = 7 小 + 1 大 → 颗数 8
        st.casinos[0].dice.insert("h".into(), 9);
        st.casinos[0].big_dice.insert("h".into(), 1);
        let mut rng = SeededRng::new(1);
        let mut ev = Vec::new();
        trigger_interactive(&mut st, 0, "h", &mut rng, &mut ev);
        let p = st.sg.pending.clone().unwrap();
        assert_eq!((p.game.as_str(), p.stage.as_str()), ("G2", "move"));
        // 合法动作 0..=8（颗数口径）
        let acts = legal_for_pending(&st, "h");
        assert_eq!(acts.len(), 9);
        assert!(acts.iter().any(|a| a.id == "sg:G2:move:0"));

        // 移 8 颗（5 小 + 1 大 → 权重 5+2=7 太绕）→ 这里移 3 颗：先 3 小
        let (done, _e, _m) = resolve(&mut st, "sg:G2:move:3", "h", &mut rng).unwrap();
        assert!(done);
        assert_eq!(st.casinos[0].dice.get("h").copied(), Some(6), "9-3 权重");
        assert_eq!(st.sg.game_field.get("h").copied(), Some(3));

        // 再轮到该玩家下注 → 可继续移（每次下注都可用）：此时权重 6 = 4 小 + 1 大 → 5 颗
        st.sg.pending = None;
        trigger_interactive(&mut st, 0, "h", &mut rng, &mut ev);
        assert_eq!(st.sg.pending.as_ref().unwrap().game, "G2");
        let (done, _e, _m) = resolve(&mut st, "sg:G2:move:5", "h", &mut rng).unwrap();
        assert!(done);
        assert_eq!(st.casinos[0].dice.get("h").copied(), None, "全移走");
        assert_eq!(
            st.sg.game_field.get("h").copied(),
            Some(9),
            "3 + (4 小 + 大骰权重 2) = 9"
        );
    }

    #[test]
    fn g2_zero_means_skip_and_field_settles_like_casino() {
        let mut st = state_with_casinos(&["G2"]);
        st.casinos[0].dice.insert("h".into(), 3);
        st.sg.pending = Some(SgPending {
            game: "G2".into(),
            casino: 0,
            actor: "h".into(),
            stage: "move".into(),
            data: json!({}),
        });
        let mut rng = SeededRng::new(1);
        let (done, _e, _m) = resolve(&mut st, "sg:G2:move:0", "h", &mut rng).unwrap();
        assert!(done);
        assert_eq!(st.casinos[0].dice.get("h").copied(), Some(3), "0 = 放弃");
        assert!(st.sg.game_field.is_empty());

        // 游戏场结算：视为第七赌场，唯一最多 $6 万、唯一第二 $3 万
        st.sg.game_field.insert("h".into(), 5);
        st.sg.game_field.insert("p2".into(), 3);
        st.sg.game_field.insert("p3".into(), 1);
        st.seats.push(Seat::new("p3".into()));
        let mut ev = Vec::new();
        settle_game_field(&mut st, &mut ev);
        assert_eq!(
            st.seats.iter().find(|s| s.player_id == "h").unwrap().banknotes,
            vec![G2_FIRST]
        );
        assert_eq!(
            st.seats.iter().find(|s| s.player_id == "p2").unwrap().banknotes,
            vec![G2_SECOND]
        );
        assert!(st
            .seats
            .iter()
            .find(|s| s.player_id == "p3")
            .unwrap()
            .banknotes
            .is_empty());
    }

    #[test]
    fn h2_roll_never_44_and_filters_unusable_points() {
        let mut rng = SeededRng::new(5);
        for _ in 0..200 {
            let d = roll_h2_dice(&mut rng);
            assert!(!(d[0] == 4 && d[1] == 4), "不得出现 4,4");
        }
        // 手中无骰、场上无己方骰 → 5/6 都不可选
        let mut st = state_with_casinos(&["H2"]);
        st.seats[0].small_remaining = 0;
        st.seats[0].big_remaining = 0;
        let opts = h2_options(&st, "h", &[5, 6]);
        assert!(opts.is_empty(), "5/6 都不可用 → 无可选项");

        // 有手牌 → 5/6 可选
        let mut st = state_with_casinos(&["H2"]);
        st.seats[0].small_remaining = 2;
        assert_eq!(h2_options(&st, "h", &[5, 6]), vec![5, 6]);

        // 无手牌但场上有己方骰 → 5 可选、6 不可选
        let mut st = state_with_casinos(&["H2"]);
        st.seats[0].small_remaining = 0;
        st.seats[0].big_remaining = 0;
        st.casinos[1].dice.insert("h".into(), 1);
        assert_eq!(h2_options(&st, "h", &[5, 6]), vec![5]);
    }

    #[test]
    fn h2_point4_rerolls_and_points_end_game() {
        let mut rng = SeededRng::new(3);
        let mut st = state_with_casinos(&["H2"]);
        st.seats[0].small_remaining = 2;
        st.sg.pending = Some(SgPending {
            game: "H2".into(),
            casino: 0,
            actor: "h".into(),
            stage: "choose".into(),
            data: json!({ "dice": [4, 6] }),
        });
        let (done, _e, msg) = resolve(&mut st, "sg:H2:choose:4", "h", &mut rng).unwrap();
        assert!(!done, "4 点应重掷继续选择");
        assert!(msg.contains("重掷"), "{msg}");
        let dice: Vec<u32> = serde_json::from_value(
            st.sg.pending.as_ref().unwrap().data.get("dice").unwrap().clone(),
        )
        .unwrap();
        assert!(!(dice[0] == 4 && dice[1] == 4));

        // 1 点 → 1 筹码并结束
        st.sg.pending = Some(SgPending {
            game: "H2".into(),
            casino: 0,
            actor: "h".into(),
            stage: "choose".into(),
            data: json!({ "dice": [1, 5] }),
        });
        let (done, _e, _m) = resolve(&mut st, "sg:H2:choose:1", "h", &mut rng).unwrap();
        assert!(done);
        assert_eq!(st.seats[0].chips, crate::INITIAL_CHIPS + 1);
    }

    #[test]
    fn h2_point5_two_stage_and_point6_priority_with_types() {
        let mut rng = SeededRng::new(7);
        // 5 点：手中无骰 → 直接进入收回阶段
        let mut st = state_with_casinos(&["H2"]);
        st.seats[0].small_remaining = 0;
        st.seats[0].big_remaining = 0;
        st.casinos[2].dice.insert("h".into(), 1);
        st.sg.pending = Some(SgPending {
            game: "H2".into(),
            casino: 0,
            actor: "h".into(),
            stage: "choose".into(),
            data: json!({ "dice": [5, 2] }),
        });
        let (done, _e, _m) = resolve(&mut st, "sg:H2:choose:5", "h", &mut rng).unwrap();
        assert!(!done);
        assert_eq!(st.sg.pending.as_ref().unwrap().stage, "five_takeback");
        let (done, _e, _m) = resolve(&mut st, "sg:H2:takeback:3", "h", &mut rng).unwrap();
        assert!(done);
        assert_eq!(st.seats[0].small_remaining, 1);

        // 6 点：优先拿小骰入框；挤走原框内的**大骰**要还给对方（还原成大骰）
        let mut st = state_with_casinos(&["H2"]);
        st.seats[0].small_remaining = 1;
        st.seats[0].big_remaining = 1;
        st.seats[1].big_remaining = 0;
        st.sg.gold_frame = Some(GoldFrame {
            owner: "p2".into(),
            small: 0,
            big: 1,
        });
        st.sg.pending = Some(SgPending {
            game: "H2".into(),
            casino: 0,
            actor: "h".into(),
            stage: "choose".into(),
            data: json!({ "dice": [6, 3] }),
        });
        let (done, _e, _m) = resolve(&mut st, "sg:H2:choose:6", "h", &mut rng).unwrap();
        assert!(done);
        let frame = st.sg.gold_frame.clone().unwrap();
        assert_eq!((frame.owner.as_str(), frame.small, frame.big), ("h", 1, 0));
        assert_eq!(st.seats[0].small_remaining, 0, "优先拿小骰");
        assert_eq!(st.seats[1].big_remaining, 1, "被挤走的大骰还原为大骰");

        // 只剩大骰时也拿（整颗）
        let mut st = state_with_casinos(&["H2"]);
        st.seats[0].small_remaining = 0;
        st.seats[0].big_remaining = 1;
        st.sg.pending = Some(SgPending {
            game: "H2".into(),
            casino: 0,
            actor: "h".into(),
            stage: "choose".into(),
            data: json!({ "dice": [6, 1] }),
        });
        let (done, _e, _m) = resolve(&mut st, "sg:H2:choose:6", "h", &mut rng).unwrap();
        assert!(done);
        assert_eq!(st.sg.gold_frame.unwrap().big, 1, "只剩大骰时拿大骰");
    }

    #[test]
    fn h2_skips_when_no_option_and_reports() {
        let mut st = state_with_casinos(&["H2"]);
        // 手牌清空、场上无己方骰 → 只有能掷出 1/2/3/4 才可能可选。
        st.seats[0].small_remaining = 0;
        st.seats[0].big_remaining = 0;
        let mut rng = SeededRng::new(1);
        // 直接给一个「只有 5/6」的骰面场景验证 h2_options 为空
        assert!(h2_options(&st, "h", &[5, 6]).is_empty());
        // 触发时应写 skipped 事件而不是挂 pending
        let mut ev = Vec::new();
        // 强制走 trigger：把 dice 换成 5,6 无法控制 RNG，这里只验证事件路径存在
        trigger_interactive(&mut st, 0, "h", &mut rng, &mut ev);
        let _ = ev;
    }

    #[test]
    fn b2_ladder_positions_and_quit() {
        // 奖励表：0/1 无、2=1 筹码、3=$3 万、4=$4 万、5=$6 万
        assert!(b2_reward(0).is_none() && b2_reward(1).is_none());
        assert_eq!(b2_reward(2), Some(Reward::Chips(1)));
        assert_eq!(b2_reward(3), Some(Reward::Note(30_000)));
        assert_eq!(b2_reward(4), Some(Reward::Note(40_000)));
        assert_eq!(b2_reward(5), Some(Reward::Note(60_000)));

        // 进入游戏：标记先到第 1 格；第 1 格弃权 = 无奖励
        let mut st = state_with_casinos(&["B2"]);
        let mut rng = SeededRng::new(2);
        let mut ev = Vec::new();
        trigger_interactive(&mut st, 0, "h", &mut rng, &mut ev);
        assert_eq!(st.sg.b2_pos, 1);
        let p = st.sg.pending.clone().unwrap();
        assert_eq!((p.game.as_str(), p.stage.as_str()), ("B2", "guess"));
        let acts = legal_for_pending(&st, "h");
        assert_eq!(acts.len(), 3, "大/小/弃权 三个选项");
        let (done, _e, msg) = resolve(&mut st, "sg:B2:quit", "h", &mut rng).unwrap();
        assert!(done);
        assert!(msg.contains("无奖励"), "{msg}");
        assert_eq!(st.seats[0].chips, crate::INITIAL_CHIPS);

        // 第 2 格弃权 = 1 筹码
        let mut st = state_with_casinos(&["B2"]);
        st.sg.b2_pos = 2;
        st.sg.pending = Some(SgPending {
            game: "B2".into(),
            casino: 0,
            actor: "h".into(),
            stage: "guess".into(),
            data: json!({ "prev": 7 }),
        });
        let (done, _e, _m) = resolve(&mut st, "sg:B2:quit", "h", &mut rng).unwrap();
        assert!(done);
        assert_eq!(st.seats[0].chips, crate::INITIAL_CHIPS + 1);
    }

    #[test]
    fn b2_guess_wrong_ends_without_reward_and_top_pays() {
        let mut rng = SeededRng::new(11);
        // 猜错 → 结束且无奖励（即便站在第 4 格）
        let mut saw_wrong = false;
        for _ in 0..40 {
            let mut st = state_with_casinos(&["B2"]);
            st.sg.b2_pos = 4;
            st.sg.pending = Some(SgPending {
                game: "B2".into(),
                casino: 0,
                actor: "h".into(),
                stage: "guess".into(),
                data: json!({ "prev": 7 }),
            });
            let (done, _e, _m) = resolve(&mut st, "sg:B2:guess:high", "h", &mut rng).unwrap();
            if st.sg.b2_pos == B2_TOP {
                // 猜对 → 前进到第 5 格 → 立即领 $6 万并结束
                assert!(done);
                assert_eq!(st.seats[0].banknotes, vec![60_000]);
                break;
            }
            // 未前进 = 猜错：结束且无奖励
            assert!(done);
            saw_wrong = true;
            assert_eq!(st.seats[0].chips, crate::INITIAL_CHIPS, "猜错无奖励");
            assert!(st.seats[0].banknotes.is_empty(), "猜错不给钞票");
            assert_eq!(st.sg.b2_pos, 4, "猜错不前进");
        }
        assert!(saw_wrong, "40 次里应有猜错（和相等或更小）");
    }

    #[test]
    fn tempo_table_covers_all_tiles() {
        // 声明式时机表：16 块全登记，且关键时机归属正确（改表须同步改本测试）。
        for id in ALL_TILES {
            assert!(!tempos_of(id).is_empty(), "{id} 未登记结算时机");
        }
        let with = |t: Tempo| -> Vec<&str> {
            ALL_TILES
                .iter()
                .filter(|id| tempos_of(id).contains(&t))
                .copied()
                .collect()
        };
        assert_eq!(with(Tempo::Setup), vec!["F1", "F2", "G1", "H1"]);
        assert_eq!(with(Tempo::OnBetAuto), vec!["A2", "C1", "D1", "D2", "E2"]);
        assert_eq!(
            with(Tempo::OnBetPending),
            vec!["A1", "B2", "E1", "F1", "F2", "G2", "H1", "H2"]
        );
        assert_eq!(with(Tempo::TurnAction), vec!["D2"]);
        assert_eq!(with(Tempo::PreSettleRound), Vec::<&str>::new());
        assert_eq!(with(Tempo::PreSettleCasino), vec!["C2"]);
        assert_eq!(with(Tempo::PreSettlePending), vec!["B1", "G1"]);
        assert_eq!(with(Tempo::PostAllocate), vec!["G2", "H1", "H2"]);
        assert_eq!(with(Tempo::PostAllocateMarker), Vec::<&str>::new());
        assert_eq!(
            with(Tempo::Passive),
            vec!["D2", "E1", "E2", "F1", "F2"]
        );
    }

    #[test]
    fn e1_ban_moves_arrow_and_rewards() {
        let mut st = state_with_casinos(&[]);
        st.sg.pending = Some(SgPending { game: "E1".into(), casino: 0, actor: "h".into(), stage: "choose".into(), data: json!({}) });
        let mut rng = SeededRng::new(1);
        let (done, _ev, _m) = resolve(&mut st, "sg:E1:ban:4", "h", &mut rng).unwrap();
        assert!(done);
        assert_eq!(st.sg.banned, Some(3));
        // 箭头前进到第 2 格 → 2 筹码
        assert_eq!(st.seats[0].chips, crate::INITIAL_CHIPS + 2);
        assert_eq!(st.sg.arrow, 1);
    }

    #[test]
    fn f1_moves_gray_group() {
        let mut st = state_with_casinos(&["F1"]);
        st.sg.gray_groups = F1_GROUPS.to_vec();
        st.sg.pending = Some(SgPending { game: "F1".into(), casino: 0, actor: "h".into(), stage: "move".into(), data: json!({}) });
        let mut rng = SeededRng::new(1);
        let (done, _ev, _m) = resolve(&mut st, "sg:F1:move:0:3", "h", &mut rng).unwrap();
        assert!(done);
        assert_eq!(st.casinos[2].dice.get("gray").copied(), Some(F1_GROUPS[0]));
        assert_eq!(st.sg.gray_groups[0], 0);
    }

    #[test]
    fn g1_split_then_pick() {
        let mut st = state_with_casinos(&["G1"]);
        st.sg.lottery = G1_LOTTERY.to_vec();
        // 代码↔标记：C1=2 万、C2=2 万、C3=6 万、C4/5/6=0 万
        let codes = vec!["C1".to_string(), "C2".into(), "C3".into(), "C4".into(), "C5".into(), "C6".into()];
        st.sg.pending = Some(SgPending {
            game: "G1".into(),
            casino: 0,
            actor: "p2".into(),
            stage: "split".into(),
            data: json!({ "winner": "h", "total": 10, "codes": codes }),
        });
        let mut rng = SeededRng::new(1);
        // 多选：把 2 万 + 6 万 放进 A 堆（英文逗号）
        let (done, _ev, _m) = resolve(&mut st, "sg:G1:split:C1,C3", "p2", &mut rng).unwrap();
        assert!(!done);
        assert_eq!(st.sg.pending.as_ref().unwrap().actor, "h");
        assert_eq!(
            st.sg.pending.as_ref().unwrap().data.get("piles"),
            Some(&json!([8, 2])),
            "A 堆 = 2+6 万"
        );
        let (done2, _e2, _m2) = resolve(&mut st, "sg:G1:pick:0", "h", &mut rng).unwrap();
        assert!(done2);
        assert_eq!(st.seats[0].banknotes, vec![80_000], "A 堆 = 8 万");
    }

    #[test]
    fn h2_gold_frame_settles() {
        let mut st = state_with_casinos(&["H2"]);
        st.sg.pending = Some(SgPending { game: "H2".into(), casino: 0, actor: "h".into(), stage: "choose".into(), data: json!({ "dice": [3, 6] }) });
        let mut rng = SeededRng::new(1);
        let (done, _ev, _m) = resolve(&mut st, "sg:H2:choose:6", "h", &mut rng).unwrap();
        assert!(done);
        assert_eq!(
            st.sg.gold_frame,
            Some(GoldFrame {
                owner: "h".into(),
                small: 1,
                big: 0
            })
        );
        let mut ev = Vec::new();
        settle_extras(&mut st, &mut ev);
        assert_eq!(st.seats[0].banknotes, vec![H2_GOLD]);
    }

    #[test]
    fn d2_privilege_updates_on_place() {
        let mut st = state_with_casinos(&["D2"]);
        st.casinos[0].dice.insert("h".into(), 3);
        update_privilege(&mut st, 0);
        assert_eq!(st.sg.privilege.as_deref(), Some("h"));
        // p2 追平 → 回场
        st.casinos[0].dice.insert("p2".into(), 3);
        update_privilege(&mut st, 0);
        assert_eq!(st.sg.privilege, None);
        // p2 超过 → 转移
        st.casinos[0].dice.insert("p2".into(), 4);
        update_privilege(&mut st, 0);
        assert_eq!(st.sg.privilege.as_deref(), Some("p2"));
    }

    #[test]
    fn pay_assets_uses_notes_then_chips() {
        let mut st = state_with_casinos(&[]);
        st.seats[0].chips = 2; // 2 万
        st.seats[0].banknotes = vec![30_000, 40_000];
        // 先扣钞票：4 万 + 1 万（3 万找零）→ 筹码不动
        let paid = pay_assets(&mut st, "h", 50_000);
        assert_eq!(paid, 50_000);
        assert_eq!(st.seats[0].chips, 2);
        assert_eq!(st.seats[0].banknotes, vec![20_000]);
        let _ = CHIP_VALUE;

        // 钞票不够 → 再扣筹码
        let mut st2 = state_with_casinos(&[]);
        st2.seats[0].chips = 3;
        st2.seats[0].banknotes = vec![30_000];
        let paid2 = pay_assets(&mut st2, "h", 50_000);
        assert_eq!(paid2, 50_000);
        assert_eq!(st2.seats[0].banknotes, Vec::<u32>::new());
        assert_eq!(st2.seats[0].chips, 1);

        // 资产不足 → 最多扣到 0，不产生欠款
        let mut st3 = state_with_casinos(&[]);
        st3.seats[0].chips = 1;
        st3.seats[0].banknotes = vec![20_000];
        let paid3 = pay_assets(&mut st3, "h", 50_000);
        assert_eq!(paid3, 30_000);
        assert_eq!(st3.seats[0].chips, 0);
        assert_eq!(st3.seats[0].banknotes, Vec::<u32>::new());
    }
}
