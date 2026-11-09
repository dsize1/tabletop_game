//! 内核：牌区 / 状态 / **唯一移动出口** / 守恒 / 单一延迟队列。
//!
//! 设计约束（spike 要验证的核心）：`zone: BTreeMap<CardId, Zone>` 是**唯一**归属表。
//! 一张牌任意时刻恰好属于一个区 → 结构上不可能「丢牌/重复计牌」，只可能「卡在帧里」，
//! 而「卡在帧里」有独立断言（`check_no_orphan_frames`）。
//!
//! 唯一的移动出口是 `Game::move_card`；它只做记账、不发事件。语义事件由 helpers
//! （`to_discard` / `to_void` / `from_void` / `gain` / `to_mark` …）上报，
//! 因此「移出/移入、获得、弃置」不会在各条路径上硬编码。

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

pub type CardId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Suit {
    Spade,
    Heart,
    Club,
    Diamond,
}

impl Suit {
    pub fn is_red(self) -> bool {
        matches!(self, Suit::Heart | Suit::Diamond)
    }
    pub fn label(self) -> &'static str {
        match self {
            Suit::Spade => "黑桃",
            Suit::Heart => "红桃",
            Suit::Club => "梅花",
            Suit::Diamond => "方块",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Slot {
    Weapon,
    Armor,
    HorseOffense,
    HorseDefense,
    Treasure,
}

impl Slot {
    pub fn label(self) -> &'static str {
        match self {
            Slot::Weapon => "武器",
            Slot::Armor => "防具",
            Slot::HorseOffense => "进攻马",
            Slot::HorseDefense => "防御马",
            Slot::Treasure => "宝物",
        }
    }
}

/// 9 类牌区（不变量：每张牌恰好属于 1 个）。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Zone {
    Draw,
    Discard,
    Void,
    Processing,
    Hand(usize),
    Equip(usize, Slot),
    Judge(usize),
    Mark(usize, String),
    /// 结算帧暂存：必须同一次 advance 内落盘（否则 `check_no_orphan_frames` 报错）。
    Frame(u64),
}

impl Zone {
    pub fn is_frame(&self) -> bool {
        matches!(self, Zone::Frame(_))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    pub id: CardId,
    pub name: String,
    pub suit: Suit,
    pub rank: u8,
    /// 装备牌才有槽位。
    pub slot: Option<Slot>,
    /// 目标模式（Chosen/AllAlive/SelfOnly），由卡牌定义表给出。
    pub target_mode: crate::cards::TargetMode,
}

impl Card {
    pub fn new(id: impl Into<String>, name: &str, suit: Suit, rank: u8) -> Self {
        Self {
            id: id.into(),
            name: name.to_string(),
            suit,
            rank,
            slot: None,
            target_mode: crate::cards::target_mode(name),
        }
    }

    pub fn equip(id: impl Into<String>, name: &str, suit: Suit, rank: u8, slot: Slot) -> Self {
        Self {
            id: id.into(),
            name: name.to_string(),
            suit,
            rank,
            slot: Some(slot),
            target_mode: crate::cards::TargetMode::SelfOnly,
        }
    }
}

/// 化身状态（左慈）：化身牌（武将 code）与当前展示的化身牌。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HuashenState {
    pub cards: Vec<String>,
    pub active: Option<String>,
    /// 原势力 / 原性别（切换化身时覆盖，移去全部化身后恢复）。
    pub orig_kingdom: String,
    pub orig_male: bool,
    /// 当前由化身授予的技能 code（切换时撤销）。
    #[serde(default)]
    pub active_skill: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Seat {
    pub name: String,
    pub general: String,
    /// 身份（lord / rebel / loyalist / traitor）。驱动器按此判定胜负。
    #[serde(default = "default_identity")]
    pub identity: String,
    /// 势力（wei/shu/wu/qun），主公技判定用。
    pub kingdom: String,
    /// 性别（骄矜等技能的判定用；默认男）。
    #[serde(default = "default_true")]
    pub male: bool,
    /// 武将牌是否背面朝上。
    pub flipped: bool,
    pub hp: i32,
    pub max_hp: i32,
    pub skills: Vec<String>,
    /// 授予技：`skills::active` 的唯一查询入口会合并它。
    pub granted: Vec<String>,
    pub flags: BTreeSet<String>,
    /// 纯计数标记（不持牌）。
    pub counters: BTreeMap<String, u32>,
    /// 手牌冻结（本回合不能主动使用/打出；回合结束自动清空）。
    #[serde(default)]
    pub frozen: BTreeSet<CardId>,
    /// 阶段级手牌冻结（本阶段不能主动使用/打出；阶段切换自动清空；司敌）。
    #[serde(default)]
    pub frozen_phase: BTreeSet<CardId>,
    /// 连环状态（铁索连环；进入时发 `ChainedEnter`）。
    #[serde(default)]
    pub chained: bool,
    /// 已废除的装备栏（决堰/破势/雄乱）。
    #[serde(default)]
    pub abolished_slots: BTreeSet<Slot>,
    /// 判定区是否废除（雄乱）。
    #[serde(default)]
    pub judge_abolished: bool,
}

/// serde 默认值：老存档没有性别字段时按男处理。
fn default_true() -> bool {
    true
}

/// serde 默认值：没有身份字段时按反贼（单测夹具不判胜负）。
fn default_identity() -> String {
    "rebel".to_string()
}

impl Seat {
    pub fn is_male(&self) -> bool {
        self.male
    }

    pub fn new(name: &str, general: &str, skills: &[&str], hp: i32) -> Self {
        Self {
            name: name.to_string(),
            general: general.to_string(),
            identity: default_identity(),
            kingdom: "qun".to_string(),
            male: true,
            flipped: false,
            hp,
            max_hp: hp,
            skills: skills.iter().map(|s| s.to_string()).collect(),
            granted: Vec::new(),
            flags: BTreeSet::new(),
            counters: BTreeMap::new(),
            frozen: BTreeSet::new(),
            frozen_phase: BTreeSet::new(),
            chained: false,
            abolished_slots: BTreeSet::new(),
            judge_abolished: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Prepare,
    Judge,
    Draw,
    Play,
    Discard,
    End,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogEntry {
    pub seq: u32,
    pub text: String,
}

pub mod deferred;
pub mod event;
pub mod card_play;
pub mod pending;
pub mod pindian;
pub mod resolve;
pub mod flow;

use self::deferred::Deferred;
use self::event::{Event, Sub};
use self::pending::Pending;
use self::resolve::Frame;
use crate::presentation::Presentation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub seats: Vec<Seat>,
    pub cards: BTreeMap<CardId, Card>,
    /// 唯一归属表。
    zone: BTreeMap<CardId, Zone>,
    /// 稳定顺序（手牌/牌堆顺序按此过滤）。
    order: Vec<CardId>,
    pub turn: usize,
    pub phase: Phase,
    pub round: u32,
    pub stack: Vec<Frame>,
    pub pending: Option<Pending>,
    /// 当前 pending 的回答（handler 下一次被调用时读取）。
    pub answer: Option<String>,
    pub deferred: Vec<Deferred>,
    /// 待路由事件（栈空后按窗口派发）。
    pub events: Vec<Event>,
    pub log: Vec<LogEntry>,
    pub presentation: Vec<Presentation>,
    frame_seq: u64,
    /// 本阶段进入弃牌堆的牌数（溃诛等「本阶段弃牌数」判定）。
    #[serde(default)]
    pub phase_discarded: u32,
    /// 一次性计数器（格式 `turn.*` / `phase.*`，切换时按前缀清空）。
    pub counters: BTreeMap<String, u32>,
    /// 转换技状态：true = 阳，false = 阴。
    pub transform: BTreeMap<String, bool>,
    /// 延迟结算：`(owner, target)` —— target 的回合结束时结算一次（镇骨）。
    #[serde(default)]
    pub scheduled: Vec<(usize, usize)>,
    /// 已声明「不能被响应」的牌（往烈）。
    #[serde(default)]
    pub unresponsive: BTreeSet<CardId>,
    /// 已声明「不计入本阶段使用次数」的牌（龙吟）。
    #[serde(default)]
    pub uncounted: BTreeSet<CardId>,
    /// 本回合已使用的牌（`(seat, card)`；`sha_used` 由它派生，不另存计数）。
    #[serde(default)]
    pub used_this_turn: Vec<(usize, CardId)>,
    /// 虚拟牌：承载牌 → 生效牌名（奇策等）。颜色/点数仍读物理牌。
    #[serde(default)]
    pub virtual_defs: BTreeMap<CardId, String>,
    /// 虚拟牌的 origin cards（承载牌 → 参与本次使用的全部物理牌，可追溯）。
    #[serde(default)]
    pub origins: BTreeMap<CardId, Vec<CardId>>,
    /// 移出游戏的牌归属标记（`card → (seat, code)`），用于「逆」等置牌类技能的取回。
    #[serde(default)]
    pub void_tags: BTreeMap<CardId, (usize, String)>,
    /// 兴衰：濒死脱离后再结算的支援者（`(victim, helpers)`）。
    #[serde(default)]
    pub after_dying_damage: Vec<(usize, Vec<usize>)>,
    /// 胜方（lord / rebel）；驱动器判定后写入。
    #[serde(default)]
    pub winner: Option<String>,
    /// 「造成伤害时」钩子的自抑制标记（技能改完数值后继续走剩余钩子，避免自我递归）。
    #[serde(default)]
    pub hook_suppressed: BTreeSet<String>,
    /// 当前公开可见（展示中）的牌（mechanics `reveal`）。
    #[serde(default)]
    pub revealed: BTreeSet<CardId>,
    /// 私密展示：`viewer → cards`（只对该玩家可见，不进公共区）。
    #[serde(default)]
    pub private_reveals: BTreeMap<usize, BTreeSet<CardId>>,
    /// 当前响应场景（无懈链 / 万箭 / 南蛮…）：由响应引擎设置，技能按「响应牌」提供选项。
    #[serde(default)]
    pub response_context: Option<crate::cards::ResponseKind>,
    /// 子流程结果通道：无懈链结算完成后写 `Some(negated)`，父流程（牌结算）读取后清空。
    /// 只允许内核流程使用（技能仍走事件/返回 Step）。
    #[serde(default)]
    pub wuxie_result: Option<bool>,
    /// 判定区入区顺序（LIFO 判定：后置先判）。`to_judge` 追加，离开判定区时移除。
    #[serde(default)]
    pub judge_order: Vec<(usize, CardId)>,
    /// 拼点子流程结果通道（内核 `pindian` 写、发起技能读后清空）。
    #[serde(default)]
    pub pindian_result: Option<crate::kernel::pindian::PindianResult>,
    /// 「【杀】指定目标后」钩子的结果通道（技能写、牌结算读后清空）。
    #[serde(default)]
    pub sha_intercept: Option<crate::kernel::card_play::ShaIntercept>,
    /// 回合序号（每进入一个新回合 +1；跨回合记忆类技能用）。
    #[serde(default)]
    pub turn_seq: u32,
    /// 技能随机源状态（splitmix64；确定性、可序列化，回放一致）。
    #[serde(default)]
    pub rand_state: u64,
    /// 主公技接力结果：代出的牌与代出者（响应流程消费后清空）。
    #[serde(default)]
    pub relay_played: Option<(CardId, usize)>,
    /// 主公技接力：所有同势力角色都放弃（响应流程消费后清空）。
    #[serde(default)]
    pub relay_none: bool,
    /// 额外回合（博图）：本回合结束后由该角色再执行一个回合。
    #[serde(default)]
    pub extra_turn: Option<usize>,
    /// 当前回合是否为「额外回合」（额外回合内不再触发博图）。
    #[serde(default)]
    pub extra_turn_active: bool,
    /// 奋威：本张牌被令无效的目标（牌结算逐目标跳过）。
    #[serde(default)]
    pub fenwei_void: BTreeSet<usize>,
    /// 本动作内各座位「离开手牌的张数」（连营的 X）。
    #[serde(default)]
    pub hand_loss_batch: BTreeMap<usize, u32>,
    /// 本动作内各座位「因弃置进入弃牌堆的牌」（界固政：单次弃置 ≥2 张）。
    #[serde(default)]
    pub discard_batch: BTreeMap<usize, Vec<CardId>>,
    /// 连营待询问：`(座位, X)`（栈空后消费）。
    #[serde(default)]
    pub lianying_pending: Option<(usize, usize)>,
    /// 谦逊：移出游戏的手牌 `(座位, 牌, 到期回合序号)`，回合切换时归还。
    #[serde(default)]
    pub void_return: Vec<(usize, CardId, u32)>,
    /// 改判结果通道（鬼才）。
    #[serde(default)]
    pub judge_rewrite: Option<CardId>,
    /// 判定子流程结果（生效的判定牌）。
    #[serde(default)]
    pub judge_result: Option<CardId>,
    /// 化身（左慈）：座位 → 化身牌与展示状态。
    #[serde(default)]
    pub huashen: BTreeMap<usize, HuashenState>,
    /// 蛊惑的响应结果（延后通道）：`Some(Some(承载牌))` = 视为已响应；`Some(None)` = 假牌作废。
    #[serde(default)]
    pub guhuo_result: Option<Option<CardId>>,
    /// 技能把当前流程延后一次（如蛊惑：质疑链结束后重入）。
    #[serde(default)]
    pub skill_deferred: bool,
    /// 本阶段弃置的牌（瑰藻等「本阶段弃置」判定用）。
    #[serde(default)]
    pub phase_discard_cards: Vec<CardId>,
    /// 摸牌获得的牌不计入「本回合获得的牌」（腹鳞）。
    #[serde(skip)]
    pub suppress_gain_count: bool,
    pub subs: Vec<Sub>,
    pub finished: bool,
}

impl Game {
    pub fn new(seats: Vec<Seat>, cards: Vec<Card>) -> Self {
        let mut g = Self {
            seats,
            cards: cards.iter().map(|c| (c.id.clone(), c.clone())).collect(),
            zone: BTreeMap::new(),
            order: Vec::new(),
            turn: 0,
            phase: Phase::Prepare,
            round: 1,
            stack: Vec::new(),
            pending: None,
            answer: None,
            deferred: Vec::new(),
            events: Vec::new(),
            log: Vec::new(),
            presentation: Vec::new(),
            frame_seq: 0,
            phase_discarded: 0,
            counters: BTreeMap::new(),
            transform: BTreeMap::new(),
            scheduled: Vec::new(),
            unresponsive: BTreeSet::new(),
            uncounted: BTreeSet::new(),
            used_this_turn: Vec::new(),
            virtual_defs: BTreeMap::new(),
            origins: BTreeMap::new(),
            void_tags: BTreeMap::new(),
            after_dying_damage: Vec::new(),
            winner: None,
            hook_suppressed: BTreeSet::new(),
            revealed: BTreeSet::new(),
            private_reveals: BTreeMap::new(),
            response_context: None,
            wuxie_result: None,
            judge_order: Vec::new(),
            pindian_result: None,
            sha_intercept: None,
            turn_seq: 0,
            rand_state: 0x5EED_2026_1109_0001,
            relay_played: None,
            relay_none: false,
            extra_turn: None,
            extra_turn_active: false,
            fenwei_void: BTreeSet::new(),
            hand_loss_batch: BTreeMap::new(),
            discard_batch: BTreeMap::new(),
            lianying_pending: None,
            void_return: Vec::new(),
            judge_rewrite: None,
            judge_result: None,
            huashen: BTreeMap::new(),
            guhuo_result: None,
            skill_deferred: false,
            phase_discard_cards: Vec::new(),
            suppress_gain_count: false,
            subs: Vec::new(),
            finished: false,
        };
        for c in &cards {
            g.zone.insert(c.id.clone(), Zone::Draw);
            g.order.push(c.id.clone());
        }
        crate::skills::rebuild_subs(&mut g);
        crate::skills::on_game_start(&mut g);
        g
    }

    // -----------------------------------------------------------------
    // 查询（派生，不存第二份）
    // -----------------------------------------------------------------

    pub fn zone_of(&self, id: &CardId) -> Option<&Zone> {
        self.zone.get(id)
    }

    pub fn hand(&self, seat: usize) -> Vec<CardId> {
        self.ids_in(|z| *z == Zone::Hand(seat))
    }

    pub fn mark_cards(&self, seat: usize, code: &str) -> Vec<CardId> {
        self.ids_in(|z| matches!(z, Zone::Mark(s, c) if *s == seat && c == code))
    }

    pub fn judge_zone(&self, seat: usize) -> Vec<CardId> {
        self.ids_in(|z| *z == Zone::Judge(seat))
    }

    pub fn discard_pile(&self) -> Vec<CardId> {
        self.ids_in(|z| *z == Zone::Discard)
    }

    /// 全部牌 id（稳定顺序；只读，供视图/审计使用）。
    pub fn all_ids(&self) -> &[CardId] {
        &self.order
    }

    pub fn ids_in(&self, pred: impl Fn(&Zone) -> bool) -> Vec<CardId> {
        self.order
            .iter()
            .filter(|id| self.zone.get(*id).is_some_and(&pred))
            .cloned()
            .collect()
    }

    pub fn card_label(&self, id: &CardId) -> String {
        match self.cards.get(id) {
            Some(c) => format!("{}{}{}", c.suit.label(), c.rank, self.name_of(id)),
            None => id.clone(),
        }
    }

    /// 生效牌名（虚拟牌读 `virtual_defs`，否则读物理牌名）。
    pub fn name_of(&self, id: &CardId) -> String {
        if let Some(name) = self.virtual_defs.get(id) {
            return name.clone();
        }
        self.cards
            .get(id)
            .map(|c| c.name.clone())
            .unwrap_or_default()
    }

    /// 该牌定义（虚拟牌按生效牌名解析）。
    pub fn def_of(&self, id: &CardId) -> crate::cards::CardDef {
        crate::cards::def(&self.name_of(id))
    }

    /// 是否为红色（颜色/点数读物理牌，符合 `card-attributes` 的转化牌口径）。
    pub fn is_red(&self, id: &CardId) -> bool {
        self.cards.get(id).map(|c| c.suit.is_red()).unwrap_or(false)
    }

    /// 设为虚拟牌：承载牌 → 生效牌名 + origin cards。
    pub fn set_virtual(&mut self, base: &CardId, name: &str, origins: Vec<CardId>) {
        self.virtual_defs.insert(base.clone(), name.to_string());
        self.origins.insert(base.clone(), origins);
    }

    /// 清除虚拟牌（结算完成后调用；origin 已落盘）。
    pub fn clear_virtual(&mut self, base: &CardId) {
        self.virtual_defs.remove(base);
        self.origins.remove(base);
    }

    pub fn is_equip(&self, id: &CardId) -> bool {
        self.cards.get(id).map(|c| c.slot.is_some()).unwrap_or(false)
    }

    /// 装备区某槽位的牌（无则 `None`）。
    pub fn equip_in(&self, seat: usize, slot: Slot) -> Option<CardId> {
        self.ids_in(|z| *z == Zone::Equip(seat, slot)).into_iter().next()
    }

    /// 防具类型（读装备区牌的 `def.armor`，不比较牌名）。
    pub fn armor(&self, seat: usize) -> crate::cards::ArmorKind {
        match self.equip_in(seat, Slot::Armor) {
            Some(id) => self.def_of(&id).armor,
            // 虚拟装备（八阵）：装备区无防具时读技能提供的虚拟防具。
            None => crate::skills::virtual_armor(self, seat),
        }
    }

    /// 武器类型（读装备区牌的 `def.weapon`，不比较牌名）。
    pub fn weapon(&self, seat: usize) -> crate::cards::WeaponKind {
        self.equip_in(seat, Slot::Weapon)
            .map(|id| self.def_of(&id).weapon)
            .unwrap_or_default()
    }

    /// 坐骑距离修正合计（进攻马/防御马）。
    pub fn horse_bonus_of(&self, seat: usize, slot: Slot) -> usize {
        self.ids_in(|z| *z == Zone::Equip(seat, slot))
            .iter()
            .map(|id| self.def_of(id).horse.unwrap_or(0))
            .sum()
    }

    /// 弃置某槽位的装备（白银狮子：失去装备时回复 1 点体力）。
    pub fn discard_equip_slot(
        &mut self,
        seat: usize,
        slot: Slot,
        cause: &str,
    ) -> Result<Vec<CardId>, String> {
        let cards = self.ids_in(|z| *z == Zone::Equip(seat, slot));
        for id in &cards {
            self.to_discard(id, cause)?;
            if self.def_of(id).armor == crate::cards::ArmorKind::Baiyin {
                crate::kernel::flow::heal(self, seat, 1, "baiyin_lost");
            }
        }
        Ok(cards)
    }

    /// 真结算入口（M1）：使用一张手牌并结算（`resolve::advance` 驱动）。
    ///
    /// 与 `use_card`（只发事件，供既有技能打桩/用例）并存，保证 R1 兼容；
    /// M2 起技能改走本入口。
    pub fn resolve_use(
        &mut self,
        seat: usize,
        card: &CardId,
        targets: Vec<usize>,
    ) -> Result<(), String> {
        crate::kernel::card_play::resolve_use(self, seat, card, targets)
    }

    /// 能否使用该牌（legal 与 execute 同源：`resolve_use` 内部也调它）。
    pub fn can_resolve_use(
        &self,
        seat: usize,
        card: &CardId,
        targets: &[usize],
    ) -> Result<(), String> {
        crate::kernel::card_play::can_use(self, seat, card, targets)
    }

    pub fn draw_top(&self) -> Option<CardId> {
        self.ids_in(|z| *z == Zone::Draw).first().cloned()
    }

    // -----------------------------------------------------------------
    // 唯一移动出口
    // -----------------------------------------------------------------

    /// **唯一**改归属的地方。不发事件、不做语义。
    fn move_card(&mut self, id: &CardId, to: Zone) -> Result<Zone, String> {
        let to_probe = to.clone();
        let from = self
            .zone
            .insert(id.clone(), to)
            .ok_or_else(|| format!("牌 {id} 不存在于任何区域"))?;
        if let Zone::Hand(s) = from {
            if let Zone::Hand(s2) = to_probe {
                if s == s2 {
                    return Ok(from);
                }
            }
            *self.hand_loss_batch.entry(s).or_insert(0) += 1;
            if self.hand(s).is_empty() {
                let x = self.hand_loss_batch.get(&s).copied().unwrap_or(1) as usize;
                self.lianying_pending = Some((s, x.max(1)));
            }
        } else if let Zone::Equip(s, _) = from {
            // 枭姬：失去装备区的牌（每张一次）。
            if self.seats.get(s).is_some_and(|x| x.hp > 0) {
                crate::skills::on_equip_lost(self, s);
            }
        }
        // 失去牌（手牌/装备区）→ `CardLost`（屯田等「失去牌后」技能的单一事实源）。
        // 判定区移出不算失去（`to_discard` 对判定区另发 `JudgeZoneLeft`）。
        if matches!(from, Zone::Hand(_) | Zone::Equip(_, _)) {
            let owner = match from {
                Zone::Hand(s) | Zone::Equip(s, _) => s,
                _ => 0,
            };
            self.events.push(Event::card(
                event::EventKind::CardLost,
                Some(owner),
                id.clone(),
                Some(format!("to:{}", zone_kind_label(&to_probe))),
            ));
        }
        Ok(from)
    }

    /// 动作开始：重置「本动作内失去手牌数」的批次计数。
    pub fn begin_action(&mut self) {
        self.hand_loss_batch.clear();
        self.discard_batch.clear();
    }

    /// 弃置：进入弃牌堆 + `CardDiscarded`（原因透传）。
    ///
    /// **判定区的牌不算弃置**（用户 2026-11-06 裁定）：从判定区移出只发 `JudgeZoneLeft`，
    /// 不产生 `CardDiscarded`、不计入本阶段弃牌数、不触发「你的牌因弃置」类技能（秉壹等）。
    pub fn to_discard(&mut self, id: &CardId, cause: &str) -> Result<(), String> {
        if let Some(Zone::Judge(seat)) = self.zone.get(id).cloned() {
            self.move_card(id, Zone::Discard)?;
            self.judge_order.retain(|(_, c)| c != id);
            // 离开判定区后不再按虚拟牌名生效（慎断的【兵粮寸断】）。
            self.clear_virtual(id);
            self.events.push(Event::card(
                event::EventKind::JudgeZoneLeft,
                Some(seat),
                id.clone(),
                Some(cause.to_string()),
            ));
            return Ok(());
        }
        let seat = match self.zone.get(id) {
            Some(Zone::Hand(s)) | Some(Zone::Equip(s, _)) => Some(*s),
            _ => None,
        };
        self.move_card(id, Zone::Discard)?;
        if self.phase == Phase::Discard {
            self.phase_discarded += 1;
            self.phase_discard_cards.push(id.clone());
        }
        if let Some(seat) = seat {
            self.bump(&format!("turn.discard_by:{seat}"));
            self.discard_batch
                .entry(seat)
                .or_default()
                .push(id.clone());
        }
        // 再起（界孟获）：「本回合红色牌进入弃牌堆」计数（任何进入弃牌堆的方式）。
        if self.cards.get(id).map(|c| c.suit.is_red()).unwrap_or(false) {
            self.bump("turn.red_discarded");
        }
        self.events.push(Event::card(
            event::EventKind::CardDiscarded,
            seat,
            id.clone(),
            Some(cause.to_string()),
        ));
        Ok(())
    }

    /// 移出游戏（**唯一**「移出/移入游戏」判定点）。
    pub fn to_void(&mut self, id: &CardId) -> Result<(), String> {
        self.move_card(id, Zone::Void)?;
        self.events.push(Event::card(
            event::EventKind::CardOutOfGame,
            None,
            id.clone(),
            None,
        ));
        Ok(())
    }

    /// 移入游戏（从 void 到指定区）。
    pub fn from_void(&mut self, id: &CardId, to: Zone) -> Result<(), String> {
        if self.zone.get(id) != Some(&Zone::Void) {
            return Err(format!("牌 {id} 不在移出游戏区"));
        }
        self.move_card(id, to)?;
        self.events.push(Event::card(
            event::EventKind::CardIntoGame,
            None,
            id.clone(),
            None,
        ));
        Ok(())
    }

    /// 获得：进入某座位手牌 + `CardGained`。
    pub fn gain(&mut self, id: &CardId, seat: usize) -> Result<(), String> {
        let from_judge = match self.zone.get(id).cloned() {
            Some(Zone::Judge(s)) => Some(s),
            _ => None,
        };
        self.move_card(id, Zone::Hand(seat))?;
        if let Some(s) = from_judge {
            // 判定区移出：不算弃置，只发 `JudgeZoneLeft`。
            self.judge_order.retain(|(_, c)| c != id);
            self.clear_virtual(id);
            self.events.push(Event::card(
                event::EventKind::JudgeZoneLeft,
                Some(s),
                id.clone(),
                Some("gained".to_string()),
            ));
        }
        // 「本回合获得的牌」（腹鳞）：摸牌阶段的摸牌不算。
        if !self.suppress_gain_count {
            self.bump(&format!("turn.gained:{seat}"));
        }
        self.events.push(Event::card(
            event::EventKind::CardGained,
            Some(seat),
            id.clone(),
            None,
        ));
        Ok(())
    }

    pub fn draw(&mut self, seat: usize, n: usize) -> Vec<CardId> {
        self.draw_with_cause(seat, n, None)
    }

    /// 带原因的摸牌：传入 `Some("draw_phase")` 时奋激不触发（用户裁定）。
    pub fn draw_with_cause(&mut self, seat: usize, n: usize, cause: Option<&str>) -> Vec<CardId> {
        let mut out = Vec::new();
        for _ in 0..n {
            let Some(id) = (if crate::skills::draw_from_bottom(self, seat) {
                self.ids_in(|z| *z == Zone::Draw).last().cloned()
            } else {
                self.draw_top()
            }) else { break };
            self.suppress_gain_count = true;
            let gained = self.gain(&id, seat).is_ok();
            self.suppress_gain_count = false;
            if !gained {
                continue;
            }
            if let Some(c) = cause {
                if let Some(ev) = self.events.last_mut() {
                    ev.cause = Some(c.to_string());
                }
            }
            out.push(id);
        }
        out
    }

    pub fn to_mark(&mut self, id: &CardId, seat: usize, code: &str) -> Result<(), String> {
        self.move_card(id, Zone::Mark(seat, code.to_string()))?;
        Ok(())
    }

    pub fn to_hand(&mut self, id: &CardId, seat: usize) -> Result<(), String> {
        self.move_card(id, Zone::Hand(seat))?;
        Ok(())
    }

    pub fn to_equip(&mut self, id: &CardId, seat: usize, slot: Slot) -> Result<(), String> {
        // 消费点：已废除的装备栏不能再装备（决堰/破势/雄乱）
        if self.is_slot_abolished(seat, slot) {
            return Err(format!("{} 栏已废除，不能装备", slot.label()));
        }
        self.move_card(id, Zone::Equip(seat, slot))?;
        Ok(())
    }

    pub fn to_processing(&mut self, id: &CardId) -> Result<(), String> {
        self.move_card(id, Zone::Processing)?;
        Ok(())
    }

    pub fn to_judge(&mut self, id: &CardId, seat: usize) -> Result<(), String> {
        // 消费点：判定区已废除（雄乱）→ 延时锦囊不能进入
        if self.seats[seat].judge_abolished {
            return Err("判定区已废除".to_string());
        }
        self.move_card(id, Zone::Judge(seat))?;
        self.judge_order.retain(|(_, c)| c != id);
        self.judge_order.push((seat, id.clone()));
        self.events.push(Event::card(
            event::EventKind::JudgeZoneEntered,
            Some(seat),
            id.clone(),
            None,
        ));
        Ok(())
    }

    /// 判定区内的牌移到另一名角色的判定区（闪电的「移交下家」）。
    /// 离开旧判定区 → `JudgeZoneLeft`；进入新判定区 → `JudgeZoneEntered`。
    pub fn move_judge_to(&mut self, id: &CardId, seat: usize) -> Result<(), String> {
        let from = match self.zone.get(id).cloned() {
            Some(Zone::Judge(s)) => s,
            _ => return Err(format!("牌 {id} 不在判定区")),
        };
        if self.seats[seat].judge_abolished {
            return Err("判定区已废除".to_string());
        }
        self.judge_order.retain(|(_, c)| c != id);
        self.judge_order.push((seat, id.clone()));
        self.move_card(id, Zone::Judge(seat))?;
        self.events.push(Event::card(
            event::EventKind::JudgeZoneLeft,
            Some(from),
            id.clone(),
            Some("lightning_move".to_string()),
        ));
        self.events.push(Event::card(
            event::EventKind::JudgeZoneEntered,
            Some(seat),
            id.clone(),
            Some("lightning_move".to_string()),
        ));
        Ok(())
    }

    /// 判定区 LIFO 顺序（后置先判），只返回该座位的牌。
    pub fn judge_zone_lifo(&self, seat: usize) -> Vec<CardId> {
        self.judge_order
            .iter()
            .filter(|(s, _)| *s == seat)
            .map(|(_, c)| c.clone())
            .rev()
            .collect()
    }

    /// 帧暂存：牌不会消失，只会被打上 Frame 标记（释放时由 Land 延迟项落盘）。
    pub fn to_frame(&mut self, id: &CardId) -> Result<(), String> {
        let seq = self.next_frame_seq();
        self.move_card(id, Zone::Frame(seq))?;
        Ok(())
    }

    /// 摸牌阶段：基础 2 张 + 技能加成（庸肆/怀橘…），原因记 "draw_phase"（奋激不触发）。
    pub fn draw_phase(&mut self, seat: usize) -> Vec<CardId> {
        if self.counter("turn.skip_draw") > 0 {
            // 截辎（界徐晃）：一名角色跳过摸牌阶段后。
            crate::skills::on_skip_draw(self, seat);
            return Vec::new();
        }
        let n = 2 + crate::skills::draw_bonus(self, seat);
        self.draw_with_cause(seat, n, Some("draw_phase"))
    }

    /// 确定性随机数（skill 随机：绝响随机技能 / 怠宴随机取牌 / 清弦随机装备）。
    pub fn rand_below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        let mut x = self.rand_state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rand_state = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) % n as u64) as usize
    }

    /// 洗牌（伏诛「然后洗牌」）：把 Draw 区的 `order` 顺序确定性重排。
    pub fn shuffle_draw(&mut self) {
        let ids: Vec<CardId> = self.ids_in(|z| *z == Zone::Draw);
        let n = ids.len();
        if n < 2 {
            return;
        }
        let mut pool = ids;
        let mut shuffled = Vec::with_capacity(n);
        while !pool.is_empty() {
            let idx = self.rand_below(pool.len());
            shuffled.push(pool.remove(idx));
        }
        // 重排 order：非牌堆牌保持相对位置，牌堆牌按洗后顺序插回原来占用的槽位。
        let mut it = shuffled.into_iter();
        for id in self.order.iter_mut() {
            if *self.zone.get(id).unwrap_or(&Zone::Void) == Zone::Draw {
                if let Some(next) = it.next() {
                    *id = next;
                }
            }
        }
    }

    /// 置于牌堆底：移到 Draw 区并把 order 移到最末（= 最后才被摸到）。
    pub fn put_on_bottom(&mut self, id: &CardId) -> Result<(), String> {
        self.move_card(id, Zone::Draw)?;
        self.order.retain(|x| x != id);
        self.order.push(id.clone());
        Ok(())
    }

    /// 从牌堆底摸 `n` 张（问卦：置于牌堆顶时双方从另一端摸）。
    pub fn draw_bottom(&mut self, seat: usize, n: usize) -> Vec<CardId> {
        let mut out = Vec::new();
        for _ in 0..n {
            let Some(id) = self.ids_in(|z| *z == Zone::Draw).last().cloned() else {
                break;
            };
            if self.to_hand(&id, seat).is_ok() {
                out.push(id);
            }
        }
        out
    }

    /// 置于牌堆顶（恃才）：移到 Draw 区并把 order 提到最前（= 下一个被摸到）。
    pub fn put_on_top(&mut self, id: &CardId) -> Result<(), String> {
        self.move_card(id, Zone::Draw)?;
        self.order.retain(|x| x != id);
        self.order.insert(0, id.clone());
        Ok(())
    }

    /// **仅事件模拟（兼容/测试）**：只发 `CardUsed` / `TargetChosen`，**不结算**。
    ///
    /// M2 起技能一律走 `resolve_use` / `use_virtual` / `use_as`；本入口只保留给既有用例与
    /// 「模拟某个事实已发生」的场景（例如只验证奇制/荐降的触发窗口）。
    pub fn use_card(&mut self, seat: usize, card: &CardId, targets: Vec<usize>) {
        // 消费点①：手牌冻结（往烈）
        if !self.can_use_card(seat, card) {
            self.bump("turn.blocked_use");
            return;
        }
        // 消费点②：目标合法性（谦节等 target_blocker）
        let targets: Vec<usize> = targets
            .into_iter()
            .filter(|t| crate::skills::can_target(self, seat, &self.name_of(card), *t, Some(card)))
            .collect();
        self.bump("turn.cards_used");
        // 消费点：雄乱生效期间，张绣的牌不能被响应
        if self.seats[seat].flags.contains("xiongluan_active") {
            self.mark_unresponsive(card);
        }
        self.events.push(Event::card(
            event::EventKind::CardUsed,
            Some(seat),
            card.clone(),
            None,
        ));
        self.used_this_turn.push((seat, card.clone()));
        if !targets.is_empty() {
            self.events.push(Event::card(
                event::EventKind::TargetChosen,
                Some(seat),
                card.clone(),
                Some(targets.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(",")),
            ));
        }
    }

    /// 结束当前阶段（发 PhaseEnd，供溃诛/掣政等「阶段结束时」技能）。
    pub fn end_phase(&mut self, cause: &str) -> Result<(), String> {
        self.events.push(Event::simple(
            event::EventKind::PhaseEnd,
            Some(self.turn),
            Some(cause),
        ));
        crate::kernel::resolve::advance(self)
    }

    pub fn next_frame_seq(&mut self) -> u64 {
        self.frame_seq += 1;
        self.frame_seq
    }

    /// 框架层落盘（只允许 `resolve::land` 调用）。
    pub(crate) fn land_card(&mut self, id: &CardId, to: Zone) -> Result<(), String> {
        let from = self.move_card(id, to.clone())?;
        if !from.is_frame() {
            return Err(format!("落盘失败：{id} 不在帧区（实际 {from:?}）"));
        }
        Ok(())
    }

    // -----------------------------------------------------------------
    // 不变量
    // -----------------------------------------------------------------

    pub fn check_conservation(&self) -> Result<(), String> {
        if self.zone.len() != self.cards.len() {
            let missing: Vec<_> = self
                .cards
                .keys()
                .filter(|id| !self.zone.contains_key(*id))
                .cloned()
                .collect();
            return Err(format!(
                "牌区归属数 {} ≠ 牌实例数 {}（缺失：{:?}）",
                self.zone.len(),
                self.cards.len(),
                missing
            ));
        }
        Ok(())
    }

    pub fn check_no_orphan_frames(&self) -> Result<(), String> {
        if !self.stack.is_empty() {
            return Ok(());
        }
        let orphans: Vec<_> = self
            .zone
            .iter()
            .filter(|(_, z)| z.is_frame())
            .map(|(id, _)| id.clone())
            .collect();
        if orphans.is_empty() {
            Ok(())
        } else {
            Err(format!("结算栈已空但仍有牌滞留在帧区：{orphans:?}"))
        }
    }

    pub fn push_log(&mut self, text: impl Into<String>) {
        let seq = self.log.len() as u32 + 1;
        self.log.push(LogEntry {
            seq,
            text: text.into(),
        });
    }

    pub fn counter(&self, key: &str) -> u32 {
        self.counters.get(key).copied().unwrap_or(0)
    }

    pub fn bump(&mut self, key: &str) {
        *self.counters.entry(key.to_string()).or_insert(0) += 1;
    }

    pub fn clear_prefix(&mut self, prefix: &str) {
        let keys: Vec<String> = self
            .counters
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        for k in keys {
            self.counters.remove(&k);
        }
    }

    pub fn seats_from_left_of(&self, from: usize) -> Vec<usize> {
        let n = self.seats.len();
        (1..=n).map(|i| (from + i) % n).collect()
    }

    /// 转换技状态查询（true = 阳）。
    pub fn yang(&self, seat: usize, code: &str) -> bool {
        *self
            .transform
            .get(&format!("{seat}:{code}"))
            .unwrap_or(&true)
    }

    pub fn toggle_yang(&mut self, seat: usize, code: &str) {
        let cur = self.yang(seat, code);
        self.transform
            .insert(format!("{seat}:{code}"), !cur);
    }

    /// 翻面（取反）；翻至正面时触发注册表里的「翻回」钩子（解围②等）。
    /// 冻结指定手牌直到**本阶段**结束（司敌）；到期由阶段切换清空。
    pub fn freeze_cards_for_phase(&mut self, seat: usize, cards: &[CardId]) {
        for id in cards {
            self.seats[seat].frozen_phase.insert(id.clone());
        }
        self.events.push(Event::simple(
            event::EventKind::HandFrozen,
            Some(seat),
            Some("until_phase_end"),
        ));
    }

    /// 解冻阶段级冻结（阶段切换时统一调用）。
    pub fn unfreeze_phase(&mut self, seat: usize) {
        if self.seats[seat].frozen_phase.is_empty() {
            return;
        }
        self.seats[seat].frozen_phase.clear();
        self.events.push(Event::simple(
            event::EventKind::HandUnfrozen,
            Some(seat),
            Some("phase_end"),
        ));
    }

    /// 该牌是否被阶段级冻结（司敌）。
    pub fn is_phase_frozen(&self, seat: usize, id: &CardId) -> bool {
        self.seats[seat].frozen_phase.contains(id)
    }

    /// 视为使用一张牌（无承载牌，如司敌/勇略/陷嗣的虚拟【杀】）。
    ///
    /// M2 起走**真实结算**（`card_play::start_virtual` → `FlowKind::Card`）：技能 handler 调用后
    /// 直接 `return Ok(Step::Done)`，由外层 `advance` 继续结算（含响应/伤害/无懈）。
    /// 牌实例与花色无关（裁定 Q18=B）。
    pub fn use_virtual(
        &mut self,
        seat: usize,
        name: &str,
        targets: Vec<usize>,
    ) -> Result<(), String> {
        crate::kernel::card_play::start_virtual(self, seat, name, targets)
    }

    /// 纯虚拟牌 + 「不能被【无懈可击】抵消」（离间：「不能被无懈可击抵消」）。
    pub fn use_virtual_no_wuxie(
        &mut self,
        seat: usize,
        name: &str,
        targets: Vec<usize>,
    ) -> Result<(), String> {
        crate::kernel::card_play::start_virtual_with(
            self,
            seat,
            name,
            targets,
            crate::kernel::card_play::UseOpts::skill().with_no_wuxie(),
        )
    }

    /// 转化牌（实体承载 + 生效牌名；如奇策/战绝/滔乱/慎断）。
    ///
    /// `strict=true` = 出牌阶段的严格校验（次数/距离）；`damage_source` 用于勤王（伤害来源视为刘谌）。
    pub fn use_as(
        &mut self,
        seat: usize,
        carrier: &CardId,
        name: &str,
        targets: Vec<usize>,
        strict: bool,
        damage_source: Option<usize>,
    ) -> Result<(), String> {
        let mut opts = if strict {
            crate::kernel::card_play::UseOpts::carrier()
        } else {
            crate::kernel::card_play::UseOpts::skill()
        };
        if let Some(src) = damage_source {
            opts = opts.with_damage_source(src);
        }
        crate::kernel::card_play::start_carrier(self, seat, carrier, name, targets, opts)
    }

    /// 冻结该角色全部手牌（本回合不能主动使用/打出）。
    /// 废除一个装备栏（栏内装备进弃牌堆）。
    pub fn abolish_slot(&mut self, seat: usize, slot: Slot) -> Result<(), String> {
        self.seats[seat].abolished_slots.insert(slot);
        let old = self.ids_in(|z| *z == Zone::Equip(seat, slot));
        for id in old {
            self.to_discard(&id, "abolish_slot")?;
        }
        Ok(())
    }

    pub fn is_slot_abolished(&self, seat: usize, slot: Slot) -> bool {
        self.seats[seat].abolished_slots.contains(&slot)
    }

    /// 座位距离（按座位环，取顺/逆较小值；马匹等修正在此扩展）。
    pub fn distance(&self, a: usize, b: usize) -> usize {
        let base = self.distance_raw(a, b) as i32;
        let delta = crate::skills::distance_delta(self, a);
        (base + delta).max(1) as usize
    }

    fn distance_raw(&self, a: usize, b: usize) -> usize {
        let n = self.seats.len();
        if n < 2 || a == b {
            return 0;
        }
        let d = (a + n - b) % n;
        let mut base = d.min(n - d) as i32;
        let horse = |seat: usize, slot: Slot| -> i32 {
            self.ids_in(|z| *z == Zone::Equip(seat, slot))
                .iter()
                .filter_map(|id| self.cards.get(id))
                .filter_map(|c| crate::cards::horse_bonus(&c.name))
                .sum::<usize>() as i32
        };
        // 与 v1 `effective_distance` 一致：进攻马缩短「我到你」的距离，防御马增加「你到你」的距离。
        base -= horse(a, Slot::HorseOffense);
        base += horse(b, Slot::HorseDefense);
        // 义从（界公孙瓒）：体力 ≤2 时其他角色算与你的距离 +1（统一距离入口）。
        if a != b {
            base += crate::skills::distance_to_delta(self, b);
        }
        if crate::skills::active(self, a, "mashu") {
            base -= 1;
        }
        // 奔袭（吴懿）第一段：回合内计算与其他角色的距离 -1（等价自带进攻马，可与进攻马叠加）。
        if a != b && self.turn == a && crate::skills::active(self, a, "benxi") {
            base -= 1;
        }
        // 千驹（曹休，锁定）：每损失 1 点体力，距离 -1（与进攻马叠加；统一距离入口）。
        if a != b && crate::skills::active(self, a, "qianju") {
            let lost = (self.seats[a].max_hp - self.seats[a].hp).max(0);
            base -= lost;
        }
        base.max(1) as usize
    }

    /// 攻击范围：装备武器时 = 武器范围；否则 1。
    pub fn attack_range(&self, seat: usize) -> usize {
        let weapon = self.ids_in(|z| matches!(z, Zone::Equip(s, _) if *s == seat));
        weapon
            .iter()
            .filter_map(|id| self.cards.get(id))
            .filter_map(|c| crate::cards::weapon_range(&c.name))
            .max()
            .unwrap_or(1)
    }

    /// 能否打到（往烈的「首张牌无距离限制」在这里生效）。
    pub fn can_reach(&self, from: usize, to: usize, card: &CardId) -> bool {
        if self.first_card_no_distance(from, card) {
            return true;
        }
        self.distance(from, to) <= self.attack_range(from)
    }

    /// 本回合【杀】可用次数（基础 1 + 决堰 + 立军 等）。
    pub fn sha_limit(&self, seat: usize) -> usize {
        if let Some(n) = crate::skills::sha_limit_override(self, seat) {
            return n;
        }
        let mut n = 1usize;
        if crate::skills::active(self, seat, "paoxiao") {
            return usize::MAX;
        }
        n += crate::skills::sha_limit_bonus(self, seat);
        n += self.counter("turn.jueyan_sha_bonus") as usize;
        n += self.counter("turn.lijun_sha_bonus") as usize;
        if self.counter(&format!("mingjian:{seat}")) >= 2 {
            n += 1;
        }
        n
    }

    /// 手牌上限（体力值 + 决堰防具栏… + 「权」）。
    pub fn hand_limit(&self, seat: usize) -> usize {
        if let Some(n) = crate::skills::hand_limit_override(self, seat) {
            return n;
        }
        let base = self.hand_limit_base(seat) as i64;
        let delta = crate::skills::hand_limit_delta(self, seat) as i64;
        base.saturating_add(delta).max(0) as usize
    }

    fn hand_limit_base(&self, seat: usize) -> usize {
        // 急攻：发动后本**出牌阶段**的手牌上限**等于**造成的伤害值（赋值，覆盖其它修正）。
        if self.seats[seat].flags.contains("jigong") {
            return self.counter("phase.damage_dealt") as usize;
        }
        // 腹鳞（李严，锁定）：本回合获得的牌不计入手牌上限（按张数加回）。
        let gained = if crate::skills::active(self, seat, "fulin") {
            self.counter(&format!("turn.gained:{seat}")) as usize
        } else {
            0
        };
        // 极奢（岑昏）：每次发动本回合手牌上限 -1。
        if crate::skills::active(self, seat, "jishe") {
            let used = self.counter("turn.jishe_used") as usize;
            let base = (self.seats[seat].hp.max(0) as usize).saturating_sub(used);
            return base + gained;
        }
        let hp = self.seats[seat].hp.max(0) as usize;
        // 明鉴：1 = 已交牌待生效，2 = 其下个回合生效中。
        let mingjian = if self.counter(&format!("mingjian:{seat}")) >= 2 { 1 } else { 0 };
        hp + self.counter("turn.jueyan_hand_limit") as usize
            + crate::skills::quan(self, seat) as usize
            + mingjian
            + gained
    }

    /// 声明某张牌「不能被响应」。
    pub fn mark_unresponsive(&mut self, id: &CardId) {
        self.unresponsive.insert(id.clone());
    }

    pub fn is_unresponsive(&self, id: &CardId) -> bool {
        self.unresponsive.contains(id)
    }

    /// 展示（公开）：**立即**进入公共可见区 + 事件 + 历史记录。
    pub fn reveal(&mut self, id: &CardId, owner: Option<usize>, cause: &str) {
        if !self.revealed.insert(id.clone()) {
            return;
        }
        let mut ev = Event::card(
            event::EventKind::CardRevealed,
            owner,
            id.clone(),
            Some(cause.to_string()),
        );
        ev.amount = None;
        self.events.push(ev);
        self.push_log(format!("展示：{}（{cause}）", self.card_label(id)));
    }

    /// 私密展示：只对 `viewer` 可见（不进公共区）。
    pub fn reveal_private(&mut self, viewer: usize, id: &CardId, cause: &str) {
        self.private_reveals
            .entry(viewer)
            .or_default()
            .insert(id.clone());
        self.events.push(Event::card(
            event::EventKind::CardRevealed,
            Some(viewer),
            id.clone(),
            Some(format!("private:{cause}")),
        ));
    }

    pub fn is_privately_revealed(&self, viewer: usize, id: &CardId) -> bool {
        self.private_reveals
            .get(&viewer)
            .is_some_and(|s| s.contains(id))
    }

    pub fn clear_private_reveals(&mut self, viewer: usize) {
        self.private_reveals.remove(&viewer);
    }

    /// 结束展示（牌离开公共可见区）。
    pub fn conceal(&mut self, id: &CardId, cause: &str) {
        if !self.revealed.remove(id) {
            return;
        }
        self.events.push(Event::card(
            event::EventKind::CardConcealed,
            None,
            id.clone(),
            Some(cause.to_string()),
        ));
    }

    pub fn is_revealed(&self, id: &CardId) -> bool {
        self.revealed.contains(id)
    }

    /// 「能否打到」的唯一判定（含诏缚：被指定角色视为在所有吴势力攻击范围内）。
    pub fn in_attack_range(&self, from: usize, to: usize) -> bool {
        if from == to {
            return false;
        }
        if self
            .seats
            .get(to)
            .is_some_and(|s| s.flags.contains("zhaofu"))
            && self.seats.get(from).is_some_and(|s| s.kingdom == "wu")
        {
            return true;
        }
        self.distance(from, to) <= self.attack_range(from)
    }

    /// 抑制某个「造成伤害时」钩子一次（技能改完数值后继续走剩余钩子）。
    pub fn suppress_hook(&mut self, key: &str) {
        self.hook_suppressed.insert(key.to_string());
    }

    pub fn hook_suppressed(&self, key: &str) -> bool {
        self.hook_suppressed.contains(key)
    }

    pub fn release_hook(&mut self, key: &str) {
        self.hook_suppressed.remove(key);
    }

    /// 标记「此牌不计入本阶段使用次数」（龙吟）：**不改**任何全局计数（用户 2026-11-06 裁定）。
    pub fn mark_uncounted(&mut self, id: &CardId) {
        self.uncounted.insert(id.clone());
    }

    pub fn is_uncounted(&self, id: &CardId) -> bool {
        self.uncounted.contains(id)
    }

    /// 本回合某角色已使用的【杀】数（属性判定；`uncounted` 的牌不计）。
    /// 纯虚拟【杀】（无承载牌）由 `turn.sha_virtual:{seat}` 计数（回合切换清空）。
    pub fn sha_used(&self, seat: usize) -> usize {
        self.used_this_turn
            .iter()
            .filter(|(s, c)| *s == seat && !self.is_uncounted(c))
            .filter(|(_, c)| self.def_of(c).tags.sha)
            .count()
            + self.counter(&format!("turn.sha_virtual:{seat}")) as usize
    }

    /// 出牌阶段首张牌是否无视距离（往烈锁定部分）——距离系统尚未落地，先提供判定入口。
    pub fn first_card_no_distance(&self, seat: usize, card: &CardId) -> bool {
        crate::skills::active(self, seat, "wanglie")
            && self.counter("turn.cards_used") <= 1
            && self.hand(seat).contains(card)
    }

    pub fn freeze_hand(&mut self, seat: usize) {
        for id in self.hand(seat) {
            self.seats[seat].frozen.insert(id);
        }
        self.events.push(Event::simple(
            event::EventKind::HandFrozen,
            Some(seat),
            Some("until_turn_end"),
        ));
    }

    /// 解冻（回合结束自动调用）。
    pub fn unfreeze_hand(&mut self, seat: usize) {
        if self.seats[seat].frozen.is_empty() {
            return;
        }
        self.seats[seat].frozen.clear();
        self.events.push(Event::simple(
            event::EventKind::HandUnfrozen,
            Some(seat),
            Some("turn_end"),
        ));
    }

    /// 该牌当前能否被主动使用/打出（冻结机制的唯一判定入口：回合级 + 阶段级）。
    pub fn can_use_card(&self, seat: usize, id: &CardId) -> bool {
        // 手牌封印（义绝·黑）：本回合不能使用或打出手牌。
        if self.seats[seat].flags.contains("hand_sealed")
            && matches!(self.zone_of(id), Some(Zone::Hand(s)) if *s == seat)
        {
            return false;
        }
        !self.seats[seat].frozen.contains(id) && !self.seats[seat].frozen_phase.contains(id)
    }

    pub fn flip(&mut self, seat: usize) {
        let now = !self.seats[seat].flipped;
        self.seats[seat].flipped = now;
        if !now {
            crate::skills::on_flip_to_face_up(self, seat);
        }
        // 统一翻面钩子（设变等：任何方向翻面都触发）。
        crate::skills::on_flip(self, seat, !now);
    }

    /// 移动一件场上装备到另一名角色的同槽位（设变）。目标槽位必须为空。
    pub fn move_equip(&mut self, id: &CardId, to: usize, slot: Slot) -> Result<(), String> {
        if self.equip_in(to, slot).is_some() {
            return Err("目标槽位已有装备".to_string());
        }
        self.move_card(id, Zone::Equip(to, slot))?;
        Ok(())
    }
}

/// 区域名（`CardLost` 的 cause：`to:<区域>`；技能据此区分弃置/使用/移出）。
fn zone_kind_label(z: &Zone) -> &'static str {
    match z {
        Zone::Draw => "Draw",
        Zone::Discard => "Discard",
        Zone::Void => "Void",
        Zone::Processing => "Processing",
        Zone::Hand(_) => "Hand",
        Zone::Equip(_, _) => "Equip",
        Zone::Judge(_) => "Judge",
        Zone::Mark(_, _) => "Mark",
        Zone::Frame(_) => "Frame",
    }
}
