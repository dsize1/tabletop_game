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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Seat {
    pub name: String,
    pub general: String,
    /// 势力（wei/shu/wu/qun），主公技判定用。
    pub kingdom: String,
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
    /// 已废除的装备栏（决堰/破势/雄乱）。
    #[serde(default)]
    pub abolished_slots: BTreeSet<Slot>,
    /// 判定区是否废除（雄乱）。
    #[serde(default)]
    pub judge_abolished: bool,
}

impl Seat {
    pub fn new(name: &str, general: &str, skills: &[&str], hp: i32) -> Self {
        Self {
            name: name.to_string(),
            general: general.to_string(),
            kingdom: "qun".to_string(),
            flipped: false,
            hp,
            max_hp: hp,
            skills: skills.iter().map(|s| s.to_string()).collect(),
            granted: Vec::new(),
            flags: BTreeSet::new(),
            counters: BTreeMap::new(),
            frozen: BTreeSet::new(),
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
pub mod pending;
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

    pub fn ids_in(&self, pred: impl Fn(&Zone) -> bool) -> Vec<CardId> {
        self.order
            .iter()
            .filter(|id| self.zone.get(*id).is_some_and(&pred))
            .cloned()
            .collect()
    }

    pub fn card_label(&self, id: &CardId) -> String {
        match self.cards.get(id) {
            Some(c) => format!("{}{}{}", c.suit.label(), c.rank, c.name),
            None => id.clone(),
        }
    }

    pub fn is_equip(&self, id: &CardId) -> bool {
        self.cards.get(id).map(|c| c.slot.is_some()).unwrap_or(false)
    }

    pub fn draw_top(&self) -> Option<CardId> {
        self.ids_in(|z| *z == Zone::Draw).first().cloned()
    }

    // -----------------------------------------------------------------
    // 唯一移动出口
    // -----------------------------------------------------------------

    /// **唯一**改归属的地方。不发事件、不做语义。
    fn move_card(&mut self, id: &CardId, to: Zone) -> Result<Zone, String> {
        let from = self
            .zone
            .insert(id.clone(), to)
            .ok_or_else(|| format!("牌 {id} 不存在于任何区域"))?;
        Ok(from)
    }

    /// 弃置：进入弃牌堆 + `CardDiscarded`（原因透传）。
    pub fn to_discard(&mut self, id: &CardId, cause: &str) -> Result<(), String> {
        let seat = match self.zone.get(id) {
            Some(Zone::Hand(s)) | Some(Zone::Equip(s, _)) | Some(Zone::Judge(s)) => Some(*s),
            _ => None,
        };
        self.move_card(id, Zone::Discard)?;
        if self.phase == Phase::Discard {
            self.phase_discarded += 1;
        }
        if let Some(seat) = seat {
            self.bump(&format!("turn.discard_by:{seat}"));
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
        self.move_card(id, Zone::Hand(seat))?;
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
            if !self.gain(&id, seat).is_ok() {
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
        Ok(())
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
            return Vec::new();
        }
        let n = 2 + crate::skills::draw_bonus(self, seat);
        self.draw_with_cause(seat, n, Some("draw_phase"))
    }

    /// 置于牌堆顶（恃才）：移到 Draw 区并把 order 提到最前（= 下一个被摸到）。
    pub fn put_on_top(&mut self, id: &CardId) -> Result<(), String> {
        self.move_card(id, Zone::Draw)?;
        self.order.retain(|x| x != id);
        self.order.insert(0, id.clone());
        Ok(())
    }

    /// 使用一张牌并指定目标：唯一的事件出口（奇制/荐降等据此判定）。
    pub fn use_card(&mut self, seat: usize, card: &CardId, targets: Vec<usize>) {
        // 消费点①：手牌冻结（往烈）
        if !self.can_use_card(seat, card) {
            self.bump("turn.blocked_use");
            return;
        }
        // 消费点②：目标合法性（谦节等 target_blocker）
        let targets: Vec<usize> = targets
            .into_iter()
            .filter(|t| crate::skills::can_target(self, seat, card, *t))
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
        let n = self.seats.len();
        if n < 2 || a == b {
            return 0;
        }
        let d = (a + n - b) % n;
        let mut base = d.min(n - d);
        let horse = |seat: usize, slot: Slot| -> usize {
            self.ids_in(|z| *z == Zone::Equip(seat, slot))
                .iter()
                .filter_map(|id| self.cards.get(id))
                .filter_map(|c| crate::cards::horse_bonus(&c.name))
                .sum()
        };
        base = base.saturating_sub(horse(a, Slot::HorseOffense));
        base = base.saturating_sub(horse(b, Slot::HorseDefense));
        if crate::skills::active(self, a, "mashu") {
            base = base.saturating_sub(1);
        }
        base.max(1)
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
        let mut n = 1usize;
        if crate::skills::active(self, seat, "paoxiao") {
            return usize::MAX;
        }
        n += self.counter("turn.jueyan_sha_bonus") as usize;
        n += self.counter("turn.lijun_sha_bonus") as usize;
        n
    }

    /// 手牌上限（体力值 + 决堰防具栏…）。
    pub fn hand_limit(&self, seat: usize) -> usize {
        let hp = self.seats[seat].hp.max(0) as usize;
        hp + self.counter("turn.jueyan_hand_limit") as usize
    }

    /// 声明某张牌「不能被响应」。
    pub fn mark_unresponsive(&mut self, id: &CardId) {
        self.unresponsive.insert(id.clone());
    }

    pub fn is_unresponsive(&self, id: &CardId) -> bool {
        self.unresponsive.contains(id)
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
            event::EventKind::TurnEnd,
            Some(seat),
            Some("hand_frozen"),
        ));
    }

    /// 解冻（回合结束自动调用）。
    pub fn unfreeze_hand(&mut self, seat: usize) {
        self.seats[seat].frozen.clear();
    }

    /// 该牌当前能否被主动使用/打出（冻结机制的唯一判定入口）。
    pub fn can_use_card(&self, seat: usize, id: &CardId) -> bool {
        !self.seats[seat].frozen.contains(id)
    }

    pub fn flip(&mut self, seat: usize) {
        let now = !self.seats[seat].flipped;
        self.seats[seat].flipped = now;
        if !now {
            crate::skills::on_flip_to_face_up(self, seat);
        }
    }
}
