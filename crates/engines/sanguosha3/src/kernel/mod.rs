//! 内核骨架：牌区 / 阶段 / 座位 / 对局状态。
//!
//! 本模块只定义**类型与不变量**，不含任何牌结算逻辑（那是 `flow` / `card_play` 的职责）。

pub mod event;
pub mod pending;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub type CardId = String;

/// 终局轮次阈值（房规，2026-11-11 用户裁定，沿用 v2）：`round > ENDGAME_ROUND` 起
/// 【桃】/【桃园结义】回血无效，濒死救援不受限。单一判定入口 `Game::endgame_no_heal`。
pub const ENDGAME_ROUND: u32 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

/// 装备槽位（`Treasure` = 木牛流马预留）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Slot {
    Weapon,
    Armor,
    HorseOffense,
    HorseDefense,
    Treasure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Phase {
    #[default]
    Prepare,
    Judge,
    Draw,
    Play,
    Discard,
    End,
}

/// 9 类牌区。**不变量：每张牌任意时刻恰好属于 1 个区**（`zone-ownership`）。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Zone {
    /// 牌堆。
    Draw,
    /// 弃牌堆。
    Discard,
    /// 移出游戏（不屈「创」等）。
    Void,
    /// 处理区（结算中，短暂停留）。
    Processing,
    /// 手牌（座位）。
    Hand(usize),
    /// 装备区（座位 + 槽位）。
    Equip(usize, Slot),
    /// 判定区（座位）。
    Judge(usize),
    /// 武将牌上（座位 + 标记名，持牌 mark：田/权/箜声等）。
    Mark(usize, String),
    /// 结算帧暂存（帧序号）。**必须同一次推进内落盘**，栈空仍有帧牌即报错。
    Frame(u64),
}

impl Zone {
    pub fn is_frame(&self) -> bool {
        matches!(self, Zone::Frame(_))
    }
    /// 座位（帧/公共区为 `None`）。
    pub fn seat(&self) -> Option<usize> {
        match self {
            Zone::Hand(s) | Zone::Equip(s, _) | Zone::Judge(s) | Zone::Mark(s, _) => Some(*s),
            _ => None,
        }
    }
}

/// 牌实例：定义 code + 花色 + 点数；**区域是 `Game.cards[id]` 的唯一归属**，不在牌自身上。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    pub id: CardId,
    /// 牌定义 code（生成物 `cards_generated` 提供；内核零牌名）。
    pub def: String,
    pub suit: Suit,
    pub rank: u8,
}

/// 座位（存活/死亡是显式状态，见 constitution I2）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Seat {
    pub id: usize,
    pub hp: i32,
    pub max_hp: i32,
    /// 显式存活状态：只有 `flow::die` 置 false；濒死（hp≤0）仍是存活。
    pub alive: bool,
    /// 授予技（一等公民的 code；`skills::active` 单一查询 = 自带 ∪ granted ∪ 临时）。
    #[serde(default)]
    pub granted: Vec<String>,
}

/// 对局状态：类型与不变量的容器，**不含结算逻辑**。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Game {
    pub seats: Vec<Seat>,
    /// 牌实例 id → 唯一归属区。
    pub cards: HashMap<CardId, Zone>,
    /// 牌定义 id → 牌实例（花色/点数/def）。牌实例数据与区域归属分离，避免「牌在区里但数据漂移」。
    pub card_defs: HashMap<CardId, Card>,
    pub turn: usize,
    pub phase: Phase,
    pub round: u32,
    /// 当前交互（唯一）；多段由 pending 序列推进，同一时刻只有一个 active pending。
    pub pending: Option<pending::Pending>,
    /// 延迟队列：结算中产生的「待栈空处理」请求，**全内核只此一条**（禁 per-skill 队列）。
    pub deferred: Vec<crate::kernel::event::Event>,
}

impl Game {
    /// 牌区唯一归属（`zone-ownership`）：每张牌恰好在一个区，且区总数 == 牌实例总数。
    pub fn check_conservation(&self) -> Result<(), String> {
        // 每张牌实例必须有且仅有唯一区域归属。
        for id in self.card_defs.keys() {
            if !self.cards.contains_key(id) {
                return Err(format!("牌 {id} 没有区域归属"));
            }
        }
        for id in self.cards.keys() {
            if !self.card_defs.contains_key(id) {
                return Err(format!("牌 {id} 有区域归属但没有牌实例定义"));
            }
        }
        Ok(())
    }

    /// 栈空时不得有帧暂存牌（`frames 落盘`）。
    pub fn check_no_orphan_frames(&self) -> Result<(), String> {
        let orphans: Vec<&CardId> = self
            .cards
            .iter()
            .filter(|(_, z)| z.is_frame())
            .map(|(id, _)| id)
            .collect();
        if orphans.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "栈空但仍有帧暂存牌：{}",
                orphans.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(",")
            ))
        }
    }

    /// 终局房规唯一判定入口（`endgame_no_heal`）。
    pub fn endgame_no_heal(&self) -> bool {
        self.round > ENDGAME_ROUND
    }
}
