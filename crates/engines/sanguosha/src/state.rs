//! 三国杀引擎状态结构（`GameState.data` 的私有 schema，`schema_version = 1`）。
//!
//! 规则文档：`docs/games/sanguosha/rules.md`、`state-machine.md`、`data-model.md` §4。
//! 本文件只定义**引擎内部完整状态**；对外投影见 `crate::view`（三分区）。

use crate::data;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// 私有状态 schema 版本（v2：P2.3 新增结算栈 `resolution`；v1 = P2.1 骨架）。
pub const SCHEMA_VERSION: u32 = 2;

/// 人数范围（标准身份局）。
pub const MIN_PLAYERS: usize = 5;
pub const MAX_PLAYERS: usize = 8;

/// 默认配置（数值口径见 `state-machine.md` §1）。
pub const DEFAULT_MAX_HAND_CARDS: u32 = 20;
pub const DEFAULT_INITIAL_HAND: u32 = 4;
pub const DEFAULT_LORD_CANDIDATE_COUNT: u32 = 5;
pub const DEFAULT_OTHER_CANDIDATE_COUNT: u32 = 5;
pub const DEFAULT_ATTACK_RANGE: u32 = 1;
pub const DEFAULT_DEFENSE_RANGE: u32 = 1;
/// 候选张数配置上限（超过武将池时按池子实际数量下发）。
pub const MAX_CANDIDATE_COUNT: u32 = 12;
/// 手牌持有上限配置范围。
pub const MAX_HAND_CARDS_LIMIT: u32 = 50;
/// 体力上限（与中台 `max_hp <= 15` 一致）。
pub const MAX_HP_LIMIT: i32 = 15;
/// 摸牌阶段默认摸牌数。
pub const DRAW_PER_TURN: u32 = 2;
/// 出牌历史保留条数。
pub const HISTORY_LIMIT: usize = 800;

/// 全局阶段（`state-machine.md` §2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Lobby,
    Setup,
    Turn,
    Dying,
    Finished,
}

impl Phase {
    pub fn is_over(self) -> bool {
        matches!(self, Phase::Finished)
    }
}

/// 回合内阶段（P2.2 实现流程；P2.1 只进入 `prepare`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnPhase {
    Prepare,
    Judge,
    Draw,
    Play,
    Discard,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Identity {
    Lord,
    Loyalist,
    Rebel,
    Spy,
}

impl Identity {
    /// 与中台 `sgs_identities.code` 一致。
    pub fn code(self) -> &'static str {
        match self {
            Identity::Lord => "lord",
            Identity::Loyalist => "loyalist",
            Identity::Rebel => "rebel",
            Identity::Spy => "spy",
        }
    }

    /// 胜负阵营（内奸独立阵营）。
    pub fn team(self) -> &'static str {
        match self {
            Identity::Lord | Identity::Loyalist => "lord",
            Identity::Rebel => "rebel",
            Identity::Spy => "spy",
        }
    }

    pub fn is_lord(self) -> bool {
        matches!(self, Identity::Lord)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Suit {
    Spade,
    Heart,
    Club,
    Diamond,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Basic,
    Trick,
    DelayedTrick,
    Weapon,
    Armor,
    Horse,
    Treasure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EquipSlot {
    Weapon,
    Armor,
    HorseOffense,
    HorseDefense,
    Treasure,
}

impl EquipSlot {
    pub const ALL: [EquipSlot; 5] = [
        EquipSlot::Weapon,
        EquipSlot::Armor,
        EquipSlot::HorseOffense,
        EquipSlot::HorseDefense,
        EquipSlot::Treasure,
    ];

    pub fn empty_map() -> BTreeMap<EquipSlot, Option<String>> {
        EquipSlot::ALL.iter().map(|s| (*s, None)).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RangeType {
    Attack,
    Defense,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetMode {
    Single,
    Aoe,
}

/// 默认武将包组合（`Config.general_packs`）；风林火山需在 lobby 手动开启。
pub fn default_general_packs() -> Vec<String> {
    vec!["standard".to_string()]
}

/// 解析 `lobby:set_general_packs` / `Config.general_packs` 的包组合字符串。
///
/// 支持形式：`standard` / `standard+wind_fire_forest_mountain`（顺序无关、去重；未知包报错）。
/// 返回的列表按 `data::GENERAL_PACK_KEYS` 的固定顺序规范化（选将池顺序稳定）。
pub fn parse_general_packs(mode: &str) -> Result<Vec<String>, String> {
    let mut wanted: Vec<&str> = Vec::new();
    for raw in mode.split('+') {
        let code = raw.trim();
        if code.is_empty() {
            return Err("武将包组合不能为空".into());
        }
        if !crate::data::GENERAL_PACK_KEYS.contains(&code) {
            return Err(format!("未知武将包：{code}"));
        }
        if !wanted.contains(&code) {
            wanted.push(code);
        }
    }
    Ok(crate::data::GENERAL_PACK_KEYS
        .iter()
        .filter(|p| wanted.contains(p))
        .map(|p| (*p).to_string())
        .collect())
}

/// 默认牌堆包组合（标准 + 军争 = 160 张，`Config.packs`）。
pub fn default_packs() -> Vec<String> {
    vec!["standard".to_string(), "junzheng".to_string()]
}

/// 解析 `lobby:set_packs` / `Config.packs` 的包组合字符串。
///
/// 支持形式：`standard` / `standard+junzheng`（顺序无关、去重；多余/未知包报错）。
/// 返回的列表按 `data::DECK_PACKS` 的固定顺序规范化（建堆顺序稳定）。
pub fn parse_packs(mode: &str) -> Result<Vec<String>, String> {
    let mut wanted: Vec<&str> = Vec::new();
    for raw in mode.split('+') {
        let code = raw.trim();
        if code.is_empty() {
            return Err("牌堆包组合不能为空".into());
        }
        if !data::DECK_PACKS.contains(&code) {
            return Err(format!("未知牌堆包：{code}"));
        }
        if !wanted.contains(&code) {
            wanted.push(code);
        }
    }
    // 军争是标准包的扩充：不允许单独建 52 张牌的军争堆。
    if wanted.contains(&"junzheng") && !wanted.contains(&"standard") {
        return Err("军争包需与标准包同时启用".into());
    }
    Ok(data::DECK_PACKS
        .iter()
        .filter(|p| wanted.contains(p))
        .map(|p| (*p).to_string())
        .collect())
}

/// 规则配置（`state-machine.md` §1）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// 持有上限（摸牌/获得后超出立即弃至上限）。
    pub max_hand_cards: u32,
    /// 初始手牌数。
    pub initial_hand: u32,
    /// 主公候选里的非主公武将张数（N）。
    pub lord_candidate_count: u32,
    /// 其他玩家候选张数（M）。
    pub other_candidate_count: u32,
    pub base_attack_range: u32,
    pub base_defense_range: u32,
    /// 8 人局双内（1 主 1 忠 4 反 2 内）。
    #[serde(default)]
    pub double_spy: bool,
    /// 牌堆/牌池包组合（全局，一局一个；旧对局按创建时的 config 固定）。
    /// 默认 `["standard", "junzheng"]` = 160 张。
    #[serde(default = "default_packs")]
    pub packs: Vec<String>,
    /// 武将包组合（全局，一局一个；旧对局按创建时的 config 固定）。
    /// 默认 `["standard"]`；开启 `wind_fire_forest_mountain` 后选将池按包扩大。
    #[serde(default = "default_general_packs")]
    pub general_packs: Vec<String>,
}

impl Config {
    /// 规范化包组合字符串（如 `standard+junzheng`），用于动作 id 与视图展示。
    pub fn packs_mode(&self) -> String {
        self.packs.join("+")
    }

    /// 规范化武将包组合字符串（如 `standard+wind_fire_forest_mountain`）。
    pub fn general_packs_mode(&self) -> String {
        self.general_packs.join("+")
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_hand_cards: DEFAULT_MAX_HAND_CARDS,
            initial_hand: DEFAULT_INITIAL_HAND,
            lord_candidate_count: DEFAULT_LORD_CANDIDATE_COUNT,
            other_candidate_count: DEFAULT_OTHER_CANDIDATE_COUNT,
            base_attack_range: DEFAULT_ATTACK_RANGE,
            base_defense_range: DEFAULT_DEFENSE_RANGE,
            double_spy: false,
            packs: default_packs(),
            general_packs: default_general_packs(),
        }
    }
}

/// 一个座位（玩家）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seat {
    pub player_id: String,
    pub identity: Identity,
    /// 武将 code（`sgs_hero_<gid>`）；选将完成前为 `None`。
    pub general: Option<String>,
    pub general_name: String,
    pub max_hp: i32,
    /// 允许为负（濒死结算）；P2.2 实现。
    pub hp: i32,
    /// 手牌（按获得顺序；持有上限见 `Config::max_hand_cards`）。
    pub hand: Vec<String>,
    /// 装备区（5 槽，键为槽位，值为牌实例 id）。
    pub equips: BTreeMap<EquipSlot, Option<String>>,
    /// 判定区（按放入顺序；末尾 = 栈顶 = 先判，LIFO）。
    pub judge_zone: Vec<String>,
    pub alive: bool,
    /// 铁索连环：横置状态（属性伤害会在横置角色间传导）。
    #[serde(default)]
    pub chained: bool,
    pub is_bot: bool,
    /// 技能私有标记（如觉醒已触发、限定技已用）。
    pub flags: BTreeMap<String, bool>,
    /// 武将牌上的标记（牌实例 id，按获得顺序）：不屈「创」、屯田「田」、怀橘「橘」等。
    #[serde(default)]
    pub marks: BTreeMap<String, Vec<String>>,
    /// 技能获得（觉醒/化身等授予的额外技能 code）。
    #[serde(default)]
    pub extra_skills: Vec<String>,
    /// 化身牌（武将 code；左慈）。
    #[serde(default)]
    pub huashen: Vec<String>,
    /// 当前展示的化身牌（武将 code）。
    #[serde(default)]
    pub huashen_active: Option<String>,
    /// 翻面状态（背面朝上：下次回合跳过，随后翻回正面）。
    #[serde(default)]
    pub flipped: bool,
}

impl Seat {
    pub fn new(player_id: String, is_bot: bool) -> Self {
        Self {
            player_id,
            // 占位值；`lobby:start` 发身份时覆盖。
            identity: Identity::Loyalist,
            general: None,
            general_name: String::new(),
            max_hp: 0,
            hp: 0,
            hand: Vec::new(),
            equips: EquipSlot::empty_map(),
            judge_zone: Vec::new(),
            alive: true,
            chained: false,
            is_bot,
            flags: BTreeMap::new(),
            marks: BTreeMap::new(),
            extra_skills: Vec::new(),
            huashen: Vec::new(),
            huashen_active: None,
            flipped: false,
        }
    }
}

/// 牌实例（不可变部分：定义 + 花色 + 点数）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardInst {
    pub def: String,
    pub suit: Suit,
    pub rank: u8,
}

/// 牌堆与公共区域（牌实例 id 列表）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deck {
    /// 洗牌种子（可复现；仅服务端状态，事件里只记录在 `deck_shuffled`）。
    pub seed: u64,
    /// 摸牌堆（末尾 = 牌堆顶，用 `pop()` 抽取）。
    pub draw: Vec<String>,
    pub discard: Vec<String>,
    /// 处理区（结算中的牌，如观星抽出的牌、判定牌）。
    pub processing: Vec<String>,
    /// 移出游戏的牌（木牛流马等预留）。
    pub void: Vec<String>,
}

impl Deck {
    pub fn empty(seed: u64) -> Self {
        Self {
            seed,
            draw: Vec::new(),
            discard: Vec::new(),
            processing: Vec::new(),
            void: Vec::new(),
        }
    }
}

/// 选将进度（P2.1 完整实现；技能/武将不在此处）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetupState {
    /// 主公 openid。
    pub lord: String,
    pub lord_picked: bool,
    /// 每人收到的候选武将 code（仅本人可见）。
    pub offers: BTreeMap<String, Vec<String>>,
    /// 每人已选武将 code。
    pub picks: BTreeMap<String, String>,
    /// 已被发放过的武将（本局不再重发）。
    pub offered: BTreeSet<String>,
    /// 武将池（未被选中的候选 + 未被发放的武将）。
    pub pool: Vec<String>,
}

/// pending 选项（加密/明码由 `vision` 决定；P2.1 仅定义结构）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingOption {
    pub id: String,
    pub label: String,
}

/// 当前交互（唯一）。多步交互由 `pending_queue` 推进。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pending {
    pub kind: String,
    /// 当前操作者 openid。
    pub actor: String,
    #[serde(default)]
    pub skill: Option<String>,
    #[serde(default)]
    pub options: Vec<PendingOption>,
    /// 交互附加数据（引擎/技能自定；视图按可见性下发）。
    #[serde(default)]
    pub data: Value,
}

/// 特殊交互区可见性（`special-zones.md` §4；`all` / `team` 预留）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Owner,
    Public,
}

/// 特殊交互区：`owner` / `visibility` 为公共字段，`payload` 为该区私有字段。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecialZone {
    #[serde(default)]
    pub owner: Option<String>,
    pub visibility: Visibility,
    #[serde(default)]
    pub payload: Map<String, Value>,
}

/// 技能事件订阅（P2.4 使用；P2.1 只保留字段，便于回放/调试）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillSubscription {
    pub event: String,
    pub skill: String,
    pub owner: String,
    #[serde(default)]
    pub priority: i32,
}

/// 出牌历史（公共可见；左侧辅助区展示）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogEntry {
    pub seq: u32,
    /// 全局回合序号（`turn_index`）。
    pub turn: u32,
    #[serde(default)]
    pub actor: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Winner {
    pub team: String,
    pub players: Vec<String>,
}

/// 引擎私有状态（序列化进 `GameState.data`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SgsState {
    pub schema_version: u32,
    pub phase: Phase,
    pub host: String,
    pub config: Config,
    pub seats: Vec<Seat>,
    /// 当前回合座位下标。
    pub turn: usize,
    pub turn_phase: TurnPhase,
    /// 已完成的回合数（0 = 尚未开始），用于回放/统计。
    pub turn_index: u32,
    /// 轮次（主公回合为一轮起点）。
    pub round: u32,
    pub cards: BTreeMap<String, CardInst>,
    /// 虚拟牌定义（P4.1 国色等）：牌实例 id → 生效定义码（如方块牌当【乐不思蜀】）。
    /// 仅用于判定区/处理区的“生效定义”查询，不改变牌实例本身的物理定义。
    #[serde(default)]
    pub virtual_defs: BTreeMap<String, String>,
    pub deck: Deck,
    pub setup: SetupState,
    #[serde(default)]
    pub pending: Option<Pending>,
    #[serde(default)]
    pub pending_queue: Vec<Pending>,
    #[serde(default)]
    pub skill_subscriptions: Vec<SkillSubscription>,
    #[serde(default)]
    pub special_zones: BTreeMap<String, SpecialZone>,
    #[serde(default)]
    pub winner: Option<Winner>,
    #[serde(default)]
    pub abort_reason: Option<String>,
    /// 本回合内的计数器（`sha_used` / `skip_play` 等），回合结束清空。
    #[serde(default)]
    pub turn_counters: BTreeMap<String, u32>,
    /// 本回合弃牌阶段进入弃牌堆的牌（固政使用）。
    #[serde(default)]
    pub phase_discarded: Vec<String>,
    /// 放权等产生的额外回合（下一个执行的座位）。
    #[serde(default)]
    pub extra_turn: Option<usize>,
    /// 结算栈（P2.3）：锦囊/无懈/响应/濒死/弃牌/判定。
    #[serde(default)]
    pub resolution: Vec<crate::resolve::Frame>,
    /// 子帧完成后交给父帧的结果（同一时刻只有一个）。
    #[serde(default)]
    pub outcome: Option<crate::resolve::Outcome>,
    /// 公共出牌历史（有上限，见 `HISTORY_LIMIT`）。
    #[serde(default)]
    pub history: Vec<LogEntry>,
    #[serde(default)]
    pub history_seq: u32,
    /// 状态内随机数状态（xorshift64*）：保证「状态快照 → 重放」确定性。
    pub rng_state: u64,
}

impl SgsState {
    /// 从 `GameState.data` 解析（失败视为内部错误）。
    pub fn parse(data: &Value) -> Result<Self, String> {
        serde_json::from_value(data.clone()).map_err(|e| format!("状态解析失败: {e}"))
    }

    /// 牌的“生效定义码”：转化牌（`virtual_defs`）优先，否则为物理牌定义。
    pub fn effective_def_code(&self, card_id: &str) -> Option<&str> {
        if let Some(code) = self.virtual_defs.get(card_id) {
            return Some(code.as_str());
        }
        self.cards.get(card_id).map(|c| c.def.as_str())
    }

    pub fn seat_index(&self, player_id: &str) -> Option<usize> {
        self.seats.iter().position(|s| s.player_id == player_id)
    }

    pub fn seat(&self, player_id: &str) -> Option<&Seat> {
        self.seats.iter().find(|s| s.player_id == player_id)
    }

    pub fn current_player(&self) -> Option<&str> {
        self.seats.get(self.turn).map(|s| s.player_id.as_str())
    }

    /// 主公座位下标。
    pub fn lord_index(&self) -> Option<usize> {
        self.seats.iter().position(|s| s.identity.is_lord())
    }

    /// 从指定座位开始、按座位顺序（左手侧）遍历其他存活玩家。
    ///
    /// 响应顺序通用原则：**从当前玩家左手侧开始**（`rules-rulings.md` §4）。
    pub fn seats_from_left_of(&self, seat: usize) -> Vec<usize> {
        let n = self.seats.len();
        (1..n).map(|offset| (seat + offset) % n).collect()
    }

    /// 下一张随机数（xorshift64*），推进 `rng_state`。
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.rng_state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng_state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// 取 `0..n` 的下标。
    pub fn next_index(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }

    /// 原地洗牌（Fisher-Yates）。
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        let n = items.len();
        if n < 2 {
            return;
        }
        for i in (1..n).rev() {
            let j = self.next_index(i + 1);
            items.swap(i, j);
        }
    }

    /// 存活座位（座位序）。
    pub fn alive_indices(&self) -> Vec<usize> {
        self.seats
            .iter()
            .enumerate()
            .filter(|(_, s)| s.alive)
            .map(|(i, _)| i)
            .collect()
    }

    /// 座位距离：只计存活角色，取顺/逆时针较小步数；自己 = 0，找不到 = `usize::MAX`。
    pub fn seat_distance(&self, from: usize, to: usize) -> usize {
        if from == to {
            return 0;
        }
        let alive = self.alive_indices();
        let n = alive.len();
        let (Some(a), Some(b)) = (
            alive.iter().position(|i| *i == from),
            alive.iter().position(|i| *i == to),
        ) else {
            return usize::MAX;
        };
        let fwd = (b + n - a) % n;
        let back = (a + n - b) % n;
        fwd.min(back)
    }

    /// 某槽位上装备牌的定义（`None` 表示空槽或未知牌）。
    pub fn equip_def(&self, idx: usize, slot: EquipSlot) -> Option<&'static data::CardDef> {
        let id = self.seats.get(idx)?.equips.get(&slot)?.as_ref()?;
        let def = &self.cards.get(id)?.def;
        data::card_def(def)
    }

    /// 是否装备了指定装备（按定义码）。
    pub fn has_equip(&self, idx: usize, code: &str) -> bool {
        EquipSlot::ALL
            .iter()
            .any(|slot| self.equip_def(idx, *slot).is_some_and(|d| d.code == code))
    }

    /// 攻击距离：武器范围（无武器用 `config.base_attack_range`）+ 进攻马 +1。
    pub fn attack_range(&self, idx: usize) -> i32 {
        let base = self
            .equip_def(idx, EquipSlot::Weapon)
            .and_then(|d| d.range_value)
            .map(i32::from)
            .unwrap_or(self.config.base_attack_range as i32);
        let horse = i32::from(self.equip_def(idx, EquipSlot::HorseOffense).is_some());
        base + horse
    }

    /// 防御距离加成：防御马 +1（别人到你更远）。
    pub fn defense_bonus(&self, idx: usize) -> i32 {
        i32::from(self.equip_def(idx, EquipSlot::HorseDefense).is_some())
    }

    /// 马术（锁定技）：计算与其他角色的距离 -1。
    pub fn mashu_bonus(&self, idx: usize) -> i32 {
        i32::from(crate::skills::has(self, idx, "mashu"))
    }

    /// `from` 能否用【杀】指定 `to`：座位距离 + 目标防御距离 ≤ 自己的攻击距离。
    pub fn in_attack_range(&self, from: usize, to: usize) -> bool {
        if from == to {
            return false;
        }
        let d = self.seat_distance(from, to);
        if d == usize::MAX {
            return false;
        }
        d as i32 - self.mashu_bonus(from) - crate::skills::qimou_bonus(self, from)
            + self.defense_bonus(to)
            <= self.attack_range(from)
    }

    /// 弃牌阶段手牌上限 = 当前体力值（最低 0）。
    pub fn hand_limit(&self, idx: usize) -> usize {
        self.seats
            .get(idx)
            .map(|s| s.hp.max(0) as usize)
            .unwrap_or(0)
    }

    pub fn counter(&self, key: &str) -> u32 {
        self.turn_counters.get(key).copied().unwrap_or(0)
    }

    pub fn set_counter(&mut self, key: &str, value: u32) {
        self.turn_counters.insert(key.to_string(), value);
    }

    pub fn bump_counter(&mut self, key: &str) {
        *self.turn_counters.entry(key.to_string()).or_insert(0) += 1;
    }

    /// 追加公共出牌历史（超上限丢弃最旧）。
    pub fn push_log(&mut self, actor: Option<&str>, text: impl Into<String>) {
        self.history_seq += 1;
        self.history.push(LogEntry {
            seq: self.history_seq,
            turn: self.turn_index,
            actor: actor.map(str::to_string),
            text: text.into(),
        });
        let over = self.history.len().saturating_sub(HISTORY_LIMIT);
        if over > 0 {
            self.history.drain(0..over);
        }
    }

    /// 牌数统计（用于守恒断言与视图）。
    pub fn card_counts(&self) -> CardCounts {
        let mut c = CardCounts::default();
        c.draw = self.deck.draw.len();
        c.discard = self.deck.discard.len();
        c.processing = self.deck.processing.len();
        c.void = self.deck.void.len();
        for s in &self.seats {
            c.hands += s.hand.len();
            c.equips += s.equips.values().flatten().count();
            c.judge += s.judge_zone.len();
            c.marks += s.marks.values().map(Vec::len).sum::<usize>();
        }
        c.instances = self.cards.len();
        c
    }

    /// 牌数守恒：所有区域的牌实例数之和 == `cards` 中的实例总数。
    pub fn check_conservation(&self) -> Result<CardCounts, String> {
        let c = self.card_counts();
        if c.total_in_zones() != c.instances {
            return Err(format!(
                "牌数不守恒：区域合计 {} ≠ 实例总数 {}（{:?}）",
                c.total_in_zones(),
                c.instances,
                c
            ));
        }
        Ok(c)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardCounts {
    pub draw: usize,
    pub discard: usize,
    pub processing: usize,
    pub void: usize,
    pub hands: usize,
    pub equips: usize,
    pub judge: usize,
    /// 武将牌上的标记（不屈「创」等）。
    pub marks: usize,
    /// 牌实例总数（`state.cards.len()`）。
    pub instances: usize,
}

impl CardCounts {
    pub fn total_in_zones(&self) -> usize {
        self.draw
            + self.discard
            + self.processing
            + self.void
            + self.hands
            + self.equips
            + self.judge
            + self.marks
    }
}
