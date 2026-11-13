//! 内核骨架：牌区 / 阶段 / 座位 / 对局状态。
//!
//! 本模块只定义**类型与不变量**，不含任何牌结算逻辑（那是 `flow` / `card_play` 的职责）。

pub mod card_play;
pub mod cards;
pub mod deferred;
pub mod event;
pub mod flow;
pub mod gear;
pub mod identity;
pub mod pending;
pub mod resolve;
pub mod turn;

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

/// 身份（标准身份局；主公公开，其余隐藏，死亡后公开）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Identity {
    Lord,
    Loyalist,
    Rebel,
    Spy,
}

/// 胜负结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Winner {
    /// 主忠胜。
    Lord,
    /// 反贼胜。
    Rebel,
    /// 内奸胜。
    Spy,
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
    /// 身份（None = 未分配/未选将）。
    #[serde(default)]
    pub identity: Option<Identity>,
}

/// 伤害属性（mechanics `damage-attributes`）：普通/火/雷。
/// 转化/虚拟牌按**生效定义**定属性（朱雀羽扇普杀→火杀=Fire；武圣红牌→普通杀=Normal）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum DamageKind {
    #[default]
    Normal,
    Fire,
    Thunder,
}

/// 伤害来源类别（mechanics `damage-attributes` 的**第二维**，2026-11-13 藤甲裁定引出）：
/// 藤甲要区分「普通【杀】（无效）/【南蛮】/【万箭】（无效）/【决斗】/技能伤害/体力流失」。
/// `HpLoss` 不是伤害（走 `flow::lose_hp`，不产生伤害事件），这里仅为来源归类的完整性。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum SourceKind {
    #[default]
    /// 普通伤害（决斗/技能伤害等）。
    Other,
    Slash,
    Nanman,
    Wanjian,
    Duel,
    Skill,
    HpLoss,
}

/// 伤害规格（二维：属性 × 来源）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DamageSpec {
    pub from: Option<usize>,
    pub target: usize,
    pub amount: i32,
    pub kind: DamageKind,
    pub source: SourceKind,
}

impl DamageSpec {
    /// 是否「普通【杀】」（藤甲免疫：slash + normal，含转化普杀）。
    pub fn is_normal_slash(&self) -> bool {
        self.source == SourceKind::Slash && self.kind == DamageKind::Normal
    }
}

/// 伤害结算结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamageOutcome {
    /// 被防具/技能免疫（藤甲对普通杀/南蛮/万箭）。
    Blocked,
    /// 实际造成点数（已过数值链）。
    Dealt(i32),
}

/// 防具（伤害链用；八卦/仁王盾是响应/拦截，M2.3 接入）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ArmorKind {
    #[default]
    None,
    /// 八卦阵：需【闪】时判定，红色视为【闪】。
    Bagua,
    /// 仁王盾：黑色【杀】无效。
    Renwang,
    /// 藤甲：火 +1；普通杀/南蛮/万箭无效。
    Tengjia,
    /// 白银狮子：伤害恒 1 + 离开装备区回 1 体力。
    Baiyin,
}

/// 武器（效果注入用；范围由 `CardDef.weapon_range` 数据给出，特效由本枚举声明）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum WeaponKind {
    #[default]
    None,
    /// 青釭剑：使用【杀】无视目标防具。
    Qinggang,
    Cixiong,
    Hanbing,
    Qinglong,
    Guanshi,
    Fangtian,
    Qilin,
    /// 诸葛连弩：出牌阶段【杀】无次数限制。
    Zhugeliannu,
    Gudingdao,
    /// 丈八蛇矛：两张手牌当【杀】。
    Zhangba,
    /// 朱雀羽扇：普通【杀】可当【火杀】。
    Zhuque,
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
    pub deferred: Vec<deferred::Deferred>,
    /// 结算栈：帧（技能帧 + 流程帧），只由 `resolve::advance` 改。
    pub stack: Vec<resolve::Frame>,
    /// 当前待办的回答（上一动作提交的 action id）。
    #[serde(default)]
    pub answer: Option<String>,
    /// 回答归属：发起当前待办的技能 code（只有它能消费回答）。
    #[serde(default)]
    pub answer_skill: Option<String>,
    /// 下一帧 id（唯一，发起帧归属用）。
    #[serde(default)]
    pub next_frame_id: u64,
    /// 【酒】加成：本回合下一张【杀】伤害 +1（回合切換清空）。
    #[serde(default)]
    pub sha_bonus: i32,
    /// 牌堆（抽牌顺序：末尾=顶）。
    #[serde(default)]
    pub deck: Vec<CardId>,
    /// 判定区 LIFO 栈（每座位；后置先判）。
    #[serde(default)]
    pub judge_stacks: HashMap<usize, Vec<CardId>>,
    /// 胜负（None = 对局未结束）。
    #[serde(default)]
    pub winner: Option<Winner>,
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
