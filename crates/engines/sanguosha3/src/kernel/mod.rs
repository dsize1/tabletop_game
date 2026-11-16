//! 内核骨架：牌区 / 阶段 / 座位 / 对局状态。
//!
//! 本模块只定义**类型与不变量**，不含任何牌结算逻辑（那是 `flow` / `card_play` 的职责）。

pub mod card_play;
pub mod cards;
pub mod deferred;
pub mod event;
pub mod flow;
pub mod gear;
pub mod generals;
pub mod guhuo;
pub mod identity;
pub mod judge;
pub mod pending;
pub mod pindian;
pub mod quick_judge;
pub mod relay;
pub mod resolve;
pub mod skills;
pub mod turn;
pub mod view;

use std::collections::{BTreeMap, BTreeSet, HashMap};

use event::{Event, EventKind};

use serde::{Deserialize, Serialize};

pub type CardId = String;

/// 终局轮次阈值（房规，2026-11-11 用户裁定，沿用 v2）：`round > ENDGAME_ROUND` 起
/// 【桃】/【桃园结义】回血无效，濒死救援不受限。单一判定入口 `Game::endgame_no_heal`。
pub const ENDGAME_ROUND: u32 = 100;

/// 历史日志上限（超出丢弃最旧的；`Game.log` 不再是无限 append-only）。
pub const LOG_MAX: usize = 400;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Suit {
    Spade,
    Heart,
    Club,
    Diamond,
    /// **无花色**（虚拟牌：0 张承载或多张承载）。颜色/花色判定恒 false，点数按 0。
    None,
}

impl Suit {
    /// 是否红色（无花色 → false）。
    pub fn is_red(self) -> bool {
        matches!(self, Suit::Heart | Suit::Diamond)
    }
    /// 是否黑色（无花色 → false；**不要写 `!is_red()`**，那会把无花色当成黑色）。
    pub fn is_black(self) -> bool {
        matches!(self, Suit::Spade | Suit::Club)
    }
    pub fn label(self) -> &'static str {
        match self {
            Suit::Spade => "黑桃",
            Suit::Heart => "红桃",
            Suit::Club => "梅花",
            Suit::Diamond => "方块",
            Suit::None => "无花色",
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

fn default_true() -> bool {
    true
}

/// 座位（存活/死亡是显式状态，见 constitution I2）。
///
/// `Default` 手写：性别默认「男」（`derive(Default)` 会把 `male` 变 false，与「未选将=男」口径不符）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Seat {
    pub id: usize,
    /// 玩家 id（引擎侧 `EnginePlugin` 的 player_id；内核结算不读它，只用于视图/权限映射）。
    #[serde(default)]
    pub name: String,
    /// 机器人座位（引擎/AI 推进用）。
    #[serde(default)]
    pub bot: bool,
    /// 武将 code（选将结果；内核结算不读它，技能/视图读）。
    #[serde(default)]
    pub general: String,
    pub hp: i32,
    pub max_hp: i32,
    /// 显式存活状态：只有 `flow::die` 置 false；濒死（hp≤0）仍是存活。
    pub alive: bool,
    /// 授予技（一等公民的 code；`skills::active` 单一查询 = 自带 ∪ granted ∪ 临时）。
    #[serde(default)]
    pub granted: Vec<String>,
    /// 自带技能（武将自带 skill code；`skills::active` 单一查询的第一个来源）。
    #[serde(default)]
    pub skills: Vec<String>,
    /// 身份（None = 未分配/未选将）。
    #[serde(default)]
    pub identity: Option<Identity>,
    /// 性别（选将时由 `generals::apply_to_seat` 从生成物落地；默认男 = 与旧夹具兼容）。
    #[serde(default = "default_true")]
    pub male: bool,
    /// 翻面（背面朝上）。
    #[serde(default)]
    pub flipped: bool,
    /// 横置（铁索连环；属性伤害在横置角色间传导）。
    #[serde(default)]
    pub chained: bool,
    /// 势力（`wei`/`shu`/`wu`/`qun`；空串 = 未定）。庸肆 X / 立军「吴势力」等按此判定。
    #[serde(default)]
    pub kingdom: String,
    /// **计数标记**（「橘」等）：不是牌，不可被拆/顺/弃（`mark-vs-counter`）。
    #[serde(default)]
    pub counters: HashMap<String, i32>,
    /// 转换技当前面（skill code → face 索引；默认 0。声明 `transform = true` 的技能才读）。
    #[serde(default)]
    pub faces: HashMap<String, u8>,
    /// **护甲**（mechanics `armor-points`）：独立于体力的防护值，受到伤害时先扣护甲。
    /// 只经 `flow::damage`（前置扣甲）与 `Game::add_armor` 改；不是牌、不是体力上限。
    #[serde(default)]
    pub armor: i32,
    /// **手牌冻结（本回合到期）**：冻结的手牌不能在窗口内被主动使用/打出；
    /// 到期由 `turn` 在回合结束时 `Game::unfreeze_turn` 清空（mechanics `hand-freeze`）。
    #[serde(default)]
    pub frozen: BTreeSet<CardId>,
    /// **手牌冻结（本阶段到期）**：阶段切换时由 `Game::unfreeze_phase` 清空。
    #[serde(default)]
    pub frozen_phase: BTreeSet<CardId>,
    /// **永久失去所有技能**（断肠：杀死者失去所有技能，含锁定技/主公技；本局永久）。
    #[serde(default)]
    pub skills_lost: bool,
    /// **已废除的装备栏**（mechanics `hand-limit` 之外的区域状态；决堰逐槽 / 破势全槽 / 雄乱装备区）：
    /// 永久废除，不可再装备。
    #[serde(default)]
    pub abolished_slots: BTreeSet<Slot>,
    /// **判定区被废除**（雄乱）：延时锦囊不可进入，流转时跳过。
    #[serde(default)]
    pub judge_abolished: bool,
}

impl Default for Seat {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::new(),
            bot: false,
            general: String::new(),
            hp: 0,
            max_hp: 0,
            alive: false,
            granted: vec![],
            skills: vec![],
            identity: None,
            male: true,
            flipped: false,
            chained: false,
            kingdom: String::new(),
            counters: HashMap::new(),
            faces: HashMap::new(),
            armor: 0,
            frozen: BTreeSet::new(),
            frozen_phase: BTreeSet::new(),
            skills_lost: false,
            abolished_slots: BTreeSet::new(),
            judge_abolished: false,
        }
    }
}

impl Seat {
    /// 计数标记数量（唯一读法）。
    pub fn counter(&self, key: &str) -> i32 {
        self.counters.get(key).copied().unwrap_or(0)
    }

    /// 计数标记增减（唯一写法，钳到 ≥ 0 并清理零值）。
    pub fn add_counter(&mut self, key: &str, delta: i32) -> i32 {
        let v = (self.counter(key) + delta).max(0);
        if v == 0 {
            self.counters.remove(key);
        } else {
            self.counters.insert(key.to_string(), v);
        }
        v
    }
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
    /// **虚拟牌实例集合**（`v:*`）：比较语义的唯一判据（见 `cards::attrs`）。
    /// 0 张承载或多张承载 → 无花色/点数；恰好 1 张承载 → 继承承载牌。
    #[serde(default)]
    pub virtual_ids: BTreeSet<CardId>,
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
    /// 待路由事件队列（结算产生 → 栈空后按订阅路由给技能）。
    #[serde(default)]
    pub events: Vec<event::Event>,
    /// **事件历史日志**（append-only；调试/真机验收/事件生产门禁读它，不被路由消费）。
    #[serde(default)]
    pub log: Vec<event::Event>,
    /// 回合/阶段/临时计数器（**唯一入口** `counter`/`bump`/`set_counter`）：
    /// 键自带命名空间（`turn.*` / `phase.*` / `deny.*` / `binglue.*`），回合切换清 `turn.*`。
    #[serde(default)]
    pub counters: HashMap<String, i32>,
    /// 文本标记（跨阶段校验用，如审时阴面记录「交给其的牌」）：键自带命名空间。
    /// 与 `counters` 区别：这里存**字符串事实**，不参与数值运算。
    #[serde(default)]
    pub marks: HashMap<String, String>,
    /// 额外回合队列（P1：`extra_turn` 由技能/牌插入，下一个回合轮到时优先消费）。
    #[serde(default)]
    pub extra_turns: Vec<usize>,
    /// 本回合待跳过的阶段（P1：`skip_phase` 插入；回合切换时清空）。
    #[serde(default)]
    pub skipped: Vec<Phase>,
    /// 已开局（`turn::begin_game` 置 true；引擎入口用它拒绝非法写动作）。
    #[serde(default)]
    pub started: bool,
    /// 房主（`lobby` 阶段权限；开局后保留给 `#中断` 判定）。
    #[serde(default)]
    pub host: Option<String>,
    /// 第几个回合（从 1 起；视图 `turn_index`/历史用）。
    #[serde(default)]
    pub turn_seq: u64,
    /// 中断原因（`turn::abort` 之外由引擎写入；`Some` 表示本局废弃不结算）。
    #[serde(default)]
    pub abort_reason: Option<String>,
    /// 房规手牌持有上限（0 = 用体力值；`hand_limit` 唯一读点）。
    #[serde(default)]
    pub hand_cap: usize,
    /// 内核随机数状态（牌堆耗尽重洗；由引擎用注入的 `RandomSource` 播种，可序列化回放）。
    #[serde(default)]
    pub rng: u64,
    /// 事件序号（单调；`Event.seq` 的来源）。
    #[serde(default)]
    pub log_seq: u64,
    /// **本次动作造成伤害的牌（K19）**：`card_play` / 技能在造成伤害前设置；
    /// `flow::damage_one` 写入 `DamageAfter.card` 与 `turn/phase.damage_card.<card>`；动作开始清空。
    #[serde(default)]
    pub damage_card: Option<CardId>,
    /// **牌级标记（K18）**：手牌上的标记（酒诗「落英牌」等）；离开手牌即清除。
    /// 与 `Seat.counters`（座位级）区分：这里按牌实例 id 记录。
    #[serde(default)]
    pub card_marks: HashMap<CardId, String>,
    /// **暂离游戏、回合末归还（K18；破军）**：`(牌, 原所有者, due_turn_seq)`。
    #[serde(default)]
    pub void_return: Vec<(CardId, usize, u64)>,
    /// **当先额外 Play 阶段**（K15）：`Some(seat)` = 当前正在执行该座位的额外出牌阶段；
    /// 结束时不进入正常 Discard，而是回到判定阶段（`turn::finish_phase` 读取）。
    #[serde(default)]
    pub extra_play: Option<usize>,
    /// **动作序号**（K13）：每次玩家提交动作 +1；批次字段按它清空（`begin_action_as`）。
    #[serde(default)]
    pub action_seq: u64,
    /// 当前动作的施动者（`begin_action_as` 打点）。
    #[serde(default)]
    pub action_actor: Option<usize>,
    /// 本动作内「失去牌」批次（owner → 牌）。旋风：单次失去 ≥2 张。
    #[serde(default)]
    pub lost_batch: BTreeMap<usize, Vec<CardId>>,
    /// 本动作内「失去装备区牌」批次（owner, 牌）。
    #[serde(default)]
    pub equip_lost_batch: Vec<(usize, CardId)>,
    /// 本动作内「获得牌」批次（获得者, 来源, 牌）。恩怨：单次获得他人 ≥2 张。
    #[serde(default)]
    pub gain_batch: Vec<(usize, usize, CardId)>,
    /// 本动作内「弃置牌」批次（弃置者 → 牌）。固政：单次弃置 ≥2 张。
    #[serde(default)]
    pub discard_batch: BTreeMap<usize, Vec<CardId>>,
    /// 本动作内技能去重标记（`begin_action` 清空）。
    #[serde(default)]
    pub action_seen: BTreeSet<String>,
    /// 「两张手牌当一张牌」选牌中间态（父魂/丈八共用；`(seat, 已选第一张)`）。
    /// 阶段/回合切换即清空；不使用 `marks` 做状态机（宪法硬约束 A）。
    #[serde(default)]
    pub pair_pick: Option<(usize, Option<CardId>)>,
    /// **私密可见集合**（观看手牌）：viewer → 牌；不进入公共可见集合，`clear_private_reveals` 收回。
    #[serde(default)]
    pub private_reveals: HashMap<usize, BTreeSet<CardId>>,
    /// **公共可见集合**（mechanics `reveal`）：`show` 后进入、`conceal` 收回；
    /// 依赖牌面的技能必须读这里（只写日志不算展示）。
    #[serde(default)]
    pub revealed: BTreeSet<CardId>,
    /// **本次“使用牌”的可选事实**（`no-wuxie` / `target-extra` / `range-override` /
    /// `multi-response` / `sha-not-counted` / `sha-intercept`）：单一来源，`finish_card` 清除。
    #[serde(default)]
    pub use_opts: HashMap<CardId, UseOpts>,
    /// **判定效果反转**（mechanics `judge-reversal`）：座位 → 延时锦囊 code（如乐不思蜀）。
    /// 只改判定分支，不改牌面花色；同一效果再次指定时覆盖旧目标。
    #[serde(default)]
    pub judge_reverse: HashMap<usize, String>,
    /// **判定结果（唯一结果通道）**：`quick_judge` 完成时写入；发起方读后 `take`
    /// （mechanics `judge-rewrite`；等价 v2 的 `Game.judge_result`）。
    #[serde(default)]
    pub judge_result: Option<crate::kernel::judge::JudgeResult>,
    /// **鬼才改判单通道**：技能写「代替判定牌的牌实例」，判定流程读后清空。
    #[serde(default)]
    pub judge_rewrite: Option<CardId>,
    /// **濒死结算后的延后结算**（mechanics `dying-aftermath`）：脱困后才逐个执行（兴衰）。
    #[serde(default)]
    pub after_dying: Vec<AfterDying>,
    /// **化身状态**（mechanics `general-pool`；池只含“至少一个已注册技能”的武将）。
    #[serde(default)]
    pub huashen: HashMap<usize, Huashen>,
    /// 拼点结果（唯一结果通道：内核写、发起技能读后 `take_pindian` 清空）。
    #[serde(default)]
    pub pindian_result: Option<crate::kernel::pindian::PindianResult>,
    /// 扣置声明结果（mechanics `hidden-declaration`；内核写、技能读后清空）。
    #[serde(default)]
    pub guhuo_result: Option<crate::kernel::guhuo::GuhuoResult>,
    /// 虚拟牌/转化牌的承载牌（origin）：`v:*` → 物理牌 id 列表（`finish_card` 一并落盘）。
    #[serde(default)]
    pub virtual_origins: HashMap<CardId, Vec<CardId>>,
    /// 每次使用牌的**目标列表**（读取型事实；从谏等需要「此牌的其他目标」）。
    #[serde(default)]
    pub use_targets: HashMap<CardId, Vec<usize>>,
}

/// 手牌冻结的到期时机（mechanics `hand-freeze`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FreezeUntil {
    /// 本回合结束（**默认**；往烈“本阶段不能再使用牌”按回合结束口径冻结）。
    Turn,
    /// 本阶段结束（阶段切换时解冻；司敌“其本阶段不能使用/打出该颜色”）。
    Phase,
}

/// 【杀】结算前介入结果（mechanics `sha-intercept`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShaIntercept {
    /// 继续正常结算。
    Continue,
    /// 此【杀】对你无效（不询问【闪】，不产生伤害）。
    Immune,
    /// 规则禁止响应（合法集为空：不询问【闪】）。
    Unresponsive,
    /// 改目标（目标锁定技；替换目标后重新结算）。
    Replace(usize),
}

/// **本次“使用牌”的可选事实**（mechanics 单一来源）。
///
/// 不变量：只作用于**本次使用**（`finish_card` / `cancel_use` 清除）；不得跨牌残留。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UseOpts {
    /// 不能被【无懈可击】抵消（只关无懈窗口，不碰闪/杀响应）。
    #[serde(default)]
    pub no_wuxie: bool,
    /// 不能被响应（闪/杀响应窗口整段跳过）。
    #[serde(default)]
    pub no_response: bool,
    /// 不计入本阶段【杀】使用次数（龙吟；已经计过的回退）。
    #[serde(default)]
    pub sha_not_counted: bool,
    /// 本次无距离限制（往烈首张 / 陷嗣 / 慎断）。
    #[serde(default)]
    pub no_distance: bool,
    /// 目标数 +N（必须同时是合法目标；legal ≡ execute 同源）。
    #[serde(default)]
    pub extra_targets: i32,
    /// 群体锦囊少选 N 个目标（在无懈/响应之前完成）。
    #[serde(default)]
    pub trim_targets: i32,
    /// 距离口径覆盖（烈弓：距离 ≤ 此牌点数即视为可达；只影响本次、逐目标）。
    #[serde(default)]
    pub range_override: Option<i32>,
    /// 响应计数 N（无双：需连续 N 张同类响应；0/1 = 单张）。
    #[serde(default)]
    pub multi_response: u32,
    /// 【杀】结算前介入结果（sha-intercept）；`None` = 未介入。
    #[serde(default)]
    pub intercept: Option<ShaIntercept>,
    /// 介入已询问过（防重复询问）。
    #[serde(default)]
    pub intercept_checked: bool,
    /// 本次无视防具（奔袭）。
    #[serde(default)]
    pub ignore_armor: bool,
    /// 本次对指定目标无视防具（陷阵：只对拼点目标）。
    #[serde(default)]
    pub ignore_armor_for: Vec<usize>,
    /// 本次对指定目标无效（贞烈：失去体力令此牌无效）。
    #[serde(default)]
    pub target_ineffective: Vec<usize>,
    /// 结算前追加目标（求援）：目标列表末尾。
    #[serde(default)]
    pub mid_targets: Vec<usize>,
    /// 本张【杀】的【闪】响应颜色限制（父魂：同色手牌；true=红）。
    #[serde(default)]
    pub response_color_lock: Option<bool>,
    /// 本次造成伤害后摸一张（奔袭）。
    #[serde(default)]
    pub draw_on_damage: bool,
    /// 逐目标「不能使用/打出响应牌」（铁骑：不弃同花色则不能【闪】）。
    #[serde(default)]
    pub unresponsive: Vec<usize>,
}

/// 濒死结算后的延后结算项（mechanics `dying-aftermath`）。
///
/// 兴衰：支援者先救人，**脱困后**才逐个受到 1 点伤害（仍是伤害事件，来源可为无）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AfterDying {
    pub victim: usize,
    pub seat: usize,
    pub amount: i32,
    pub cause: String,
}

/// 化身状态（mechanics `general-pool`）：池 + 当前展示的化身武将 code。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Huashen {
    /// 持有的化身牌（武将 code；不重复）。
    pub pool: Vec<String>,
    /// 当前展示的化身牌（`None` = 未展示/已移去）。
    #[serde(default)]
    pub current: Option<String>,
    /// 当前从展示化身牌上取得的技能 code（授予技形式：`skills::grant` 同一入口）。
    #[serde(default)]
    pub active_skill: Option<String>,
}

impl Game {
    /// 事件唯一入队口（P2）：同时进「待路由队列」与「历史日志」。
    ///
    /// 任何结算路径都必须走这里（直接 `events.push` 会让事件历史漏记，门禁会查）。
    pub fn push_event(&mut self, mut ev: event::Event) {
        self.log_seq += 1;
        ev.seq = self.log_seq;
        self.events.push(ev.clone());
        self.log.push(ev);
        // 历史日志封顶：快照每次动作都会整份序列化，日志无上限会让长局 O(n²)。
        if self.log.len() > LOG_MAX {
            let drop = self.log.len() - LOG_MAX;
            self.log.drain(0..drop);
        }
    }

    /// xorshift64*（可序列化、可回放；只用于牌堆重洗等内核随机）。
    pub fn rand_below(&mut self, n: usize) -> usize {
        if n <= 1 {
            return 0;
        }
        let mut x = self.rng;
        if x == 0 {
            x = 0x9E37_79B9_7F4A_7C15;
        }
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) % n as u64) as usize
    }

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
                orphans
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            ))
        }
    }

    /// 终局房规唯一判定入口（`endgame_no_heal`）。
    pub fn endgame_no_heal(&self) -> bool {
        self.round > ENDGAME_ROUND
    }

    /// 手牌（按牌 id 排序 = **确定性**；HashMap 迭代序不稳定，禁止直接遍历当手牌序）。
    pub fn hand(&self, seat: usize) -> Vec<CardId> {
        let mut ids: Vec<CardId> = self
            .cards
            .iter()
            .filter(|(_, z)| matches!(z, Zone::Hand(s) if *s == seat))
            .map(|(id, _)| id.clone())
            .collect();
        ids.sort();
        ids
    }

    /// 手牌数。
    pub fn hand_size(&self, seat: usize) -> usize {
        self.cards
            .values()
            .filter(|z| matches!(z, Zone::Hand(s) if *s == seat))
            .count()
    }

    /// 计数器读（唯一读法）。
    pub fn counter(&self, key: &str) -> i32 {
        self.counters.get(key).copied().unwrap_or(0)
    }

    /// 计数器 +1（唯一写法之一）。
    pub fn bump(&mut self, key: &str) {
        let v = self.counter(key) + 1;
        self.counters.insert(key.to_string(), v);
    }

    /// 计数器赋值（0 时删除键，便于「未设置」= 0 的判定）。
    pub fn set_counter(&mut self, key: &str, v: i32) {
        if v == 0 {
            self.counters.remove(key);
        } else {
            self.counters.insert(key.to_string(), v);
        }
    }

    /// 动作开始（K13）：序号 +1、记录施动者、清空所有「单动作批次」。
    pub fn begin_action_as(&mut self, actor: usize) {
        self.action_seq += 1;
        self.action_actor = Some(actor);
        self.damage_card = None;
        self.lost_batch.clear();
        self.equip_lost_batch.clear();
        self.gain_batch.clear();
        self.discard_batch.clear();
        self.action_seen.clear();
    }

    /// 动作内去重：首次出现返回 true 并登记（`begin_action_as` 清空）。
    pub fn mark_action_seen(&mut self, key: impl Into<String>) -> bool {
        self.action_seen.insert(key.into())
    }

    /// 牌级标记（K18）：设置。
    pub fn set_card_mark(&mut self, id: &str, mark: impl Into<String>) {
        self.card_marks.insert(id.to_string(), mark.into());
    }

    /// 牌级标记（K18）：读取。
    pub fn card_mark(&self, id: &str) -> Option<&str> {
        self.card_marks.get(id).map(|s| s.as_str())
    }

    /// 牌级标记（K18）：清除（离开手牌时由 `flow::note_lost` 统一调用）。
    pub fn clear_card_mark(&mut self, id: &str) {
        self.card_marks.remove(id);
    }

    /// 私密展示（观看手牌）：仅 viewer 可见，不进入公共可见集合。
    pub fn reveal_private(&mut self, viewer: usize, ids: &[CardId], _cause: &str) {
        let set = self.private_reveals.entry(viewer).or_default();
        for id in ids {
            set.insert(id.clone());
        }
    }

    /// 清空某 viewer 的私密展示。
    pub fn clear_private_reveals(&mut self, viewer: usize) {
        self.private_reveals.remove(&viewer);
    }

    /// 清掉某命名空间的计数器（回合切换 `turn.` / 阶段切换 `phase.` 用）。
    pub fn clear_counters(&mut self, prefix: &str) {
        self.counters.retain(|k, _| !k.starts_with(prefix));
    }

    /// 清掉某命名空间的文本标记（**与 `clear_counters` 成对**：`turn.`/`phase.`/`round.`）。
    ///
    /// 教训（2026-11-16 审计）：`turn.`/`phase.` 前缀的标记此前**从不清理**，
    /// 「本回合」状态（双雄颜色/弓骑花色/潜袭牌/渐营花色/强识类别）会泄漏到后续回合。
    pub fn clear_marks(&mut self, prefix: &str) {
        self.marks.retain(|k, _| !k.starts_with(prefix));
    }

    /// 文本标记读。
    pub fn mark(&self, key: &str) -> Option<&str> {
        self.marks.get(key).map(|s| s.as_str())
    }

    /// 文本标记写。
    pub fn set_mark(&mut self, key: &str, value: impl Into<String>) {
        self.marks.insert(key.to_string(), value.into());
    }

    /// 存活角色的**不同势力数**（庸肆 X；与 v1 同口径：至少 1，同势力只算 1，空势力不计）。
    pub fn kingdom_count(&self) -> usize {
        let mut seen = std::collections::BTreeSet::new();
        for s in &self.seats {
            if s.alive && !s.kingdom.is_empty() {
                seen.insert(s.kingdom.as_str());
            }
        }
        seen.len().max(1)
    }

    /// 当前主公座位（`Identity::Lord` 且存活）。
    pub fn current_lord(&self) -> Option<usize> {
        self.seats
            .iter()
            .position(|s| s.alive && s.identity == Some(Identity::Lord))
    }

    /// 存活座位（升序；**确定性**，替代 `seats.iter().enumerate()` 各处自拼）。
    pub fn alive_seats(&self) -> Vec<usize> {
        self.seats
            .iter()
            .filter(|s| s.alive)
            .map(|s| s.id)
            .collect()
    }

    /// 是否是当前回合角色。
    pub fn is_turn(&self, seat: usize) -> bool {
        self.turn == seat
    }

    /// player_id → 座位号（`Seat.name` 唯一）。
    pub fn seat_of(&self, player_id: &str) -> Option<usize> {
        self.seats.iter().position(|s| s.name == player_id)
    }

    /// 对局是否已结束（胜负已定或已中断）。
    pub fn finished(&self) -> bool {
        self.winner.is_some() || self.abort_reason.is_some()
    }

    /// 手牌上限（弃牌阶段用；体力值 + 技能修正，下限 0）。
    pub fn hand_limit(&self, seat: usize) -> usize {
        // 技能覆盖（不屈：有「创」时 = 创数，取代体力值）。
        if let Some(n) = skills::hand_limit_override(self, seat) {
            return n;
        }
        let hp = self
            .seats
            .get(seat)
            .map(|s| s.hp.max(0) as usize)
            .unwrap_or(0);
        let limit = hp.saturating_add_signed(skills::hand_limit_bonus(self, seat) as isize);
        if self.hand_cap > 0 {
            limit.min(self.hand_cap)
        } else {
            limit
        }
    }

    // ------------------------------------------------------------------
    // 机制单一入口（mechanics：armor-points / reveal / hand-freeze /
    // use-opts / judge-reversal / dying-aftermath / general-pool）
    // ------------------------------------------------------------------

    /// 获得/失去护甲（mechanics `armor-points`；变化必须有事件 + 视图投影）。
    pub fn add_armor(&mut self, seat: usize, n: i32) {
        if n == 0 {
            return;
        }
        if let Some(s) = self.seats.get_mut(seat) {
            s.armor = (s.armor + n).max(0);
        }
        let mut ev = Event::simple(
            EventKind::ArmorChanged,
            Some(seat),
            Some(if n > 0 { "gain" } else { "lose" }),
        );
        ev.amount = Some(n);
        self.push_event(ev);
    }

    /// 展示（medcanics `reveal`）：当下进入公共可见集合 + `CardRevealed`。
    pub fn reveal(&mut self, ids: &[CardId], cause: &str) {
        for id in ids {
            if self.revealed.insert(id.clone()) {
                let ev = Event::card(
                    EventKind::CardRevealed,
                    self.cards.get(id).and_then(|z| z.seat()),
                    id.clone(),
                    Some(cause.to_string()),
                );
                self.push_event(ev);
            }
        }
    }

    /// 收回展示（结算完成必须 conceal，避免公共区残留误导）。
    pub fn conceal(&mut self, ids: &[CardId], cause: &str) {
        for id in ids {
            if self.revealed.remove(id) {
                let ev = Event::card(
                    EventKind::CardConcealed,
                    self.cards.get(id).and_then(|z| z.seat()),
                    id.clone(),
                    Some(cause.to_string()),
                );
                self.push_event(ev);
            }
        }
    }

    /// 是否处于公共可见状态。
    pub fn is_revealed(&self, id: &str) -> bool {
        self.revealed.contains(id)
    }

    /// 冻结手牌（mechanics `hand-freeze`）：绑定“牌集合 + 到期时机”。
    ///
    /// 冻结**不阻止**被弃置/被获得/被移出游戏（只挡主动使用/打出）。
    pub fn freeze_cards(&mut self, owner: usize, ids: &[CardId], until: FreezeUntil, cause: &str) {
        let Some(s) = self.seats.get_mut(owner) else {
            return;
        };
        for id in ids {
            match until {
                FreezeUntil::Turn => s.frozen.insert(id.clone()),
                FreezeUntil::Phase => s.frozen_phase.insert(id.clone()),
            };
        }
        let mut ev = Event::simple(EventKind::HandFrozen, Some(owner), Some(cause));
        ev.amount = Some(ids.len() as i32);
        self.push_event(ev);
    }

    /// 冻结整手（往烈“本阶段不能再使用牌”）。
    pub fn freeze_hand(&mut self, owner: usize, until: FreezeUntil, cause: &str) {
        let ids = self.hand(owner);
        self.freeze_cards(owner, &ids, until, cause);
    }

    /// 该手牌是否被冻结（唯一读法：`usable_hand`/响应候选/`begin_use` 共用）。
    pub fn is_frozen(&self, owner: usize, id: &str) -> bool {
        self.seats
            .get(owner)
            .map(|s| s.frozen.contains(id) || s.frozen_phase.contains(id))
            .unwrap_or(false)
    }

    /// 阶段到期解冻（由 `turn::begin_phase` 调用；跨阶段禁止残留）。
    pub fn unfreeze_phase(&mut self) {
        for seat in 0..self.seats.len() {
            if self.seats[seat].frozen_phase.is_empty() {
                continue;
            }
            self.seats[seat].frozen_phase.clear();
            let ev = Event::simple(EventKind::HandUnfrozen, Some(seat), Some("phase_end"));
            self.push_event(ev);
        }
    }

    /// 回合到期解冻（由 `turn` 在回合结束时调用）。
    pub fn unfreeze_turn(&mut self) {
        for seat in 0..self.seats.len() {
            if self.seats[seat].frozen.is_empty() {
                continue;
            }
            self.seats[seat].frozen.clear();
            let ev = Event::simple(EventKind::HandUnfrozen, Some(seat), Some("turn_end"));
            self.push_event(ev);
        }
    }

    /// 本次使用牌的可选事实（不存在 = 默认值）。
    pub fn use_opts(&self, card: &str) -> UseOpts {
        self.use_opts.get(card).cloned().unwrap_or_default()
    }

    /// 写本次使用牌的可选事实（技能声明入口）。
    pub fn use_opts_mut(&mut self, card: &str) -> &mut UseOpts {
        self.use_opts.entry(card.to_string()).or_default()
    }

    /// 清本次使用牌的可选事实（`finish_card` / `cancel_use` 必须调用）。
    pub fn clear_use_opts(&mut self, card: &str) {
        self.use_opts.remove(card);
    }

    /// 声明“本次不能被【无懈可击】抵消”（mechanics `no-wuxie`）。
    pub fn no_wuxie(&mut self, card: &str) {
        self.use_opts_mut(card).no_wuxie = true;
    }

    /// 声明“本次不能被响应”（闪/杀响应整段跳过）。
    pub fn no_response(&mut self, card: &str) {
        self.use_opts_mut(card).no_response = true;
    }

    /// 判定效果反转（mechanics `judge-reversal`）：同一延时锦囊 code 只绑定一个目标。
    pub fn reverse_judge(&mut self, seat: usize, code: &str) {
        self.judge_reverse.retain(|_, v| v != code);
        self.judge_reverse.insert(seat, code.to_string());
    }

    /// 某座位的该延时锦囊判定是否反转。
    pub fn judge_reversed(&self, seat: usize, code: &str) -> bool {
        self.judge_reverse
            .get(&seat)
            .map(|c| c == code)
            .unwrap_or(false)
    }

    /// 该延时锦囊的反转目标（视图用；没有 = None）。
    pub fn judge_reverse_target(&self, code: &str) -> Option<usize> {
        self.judge_reverse
            .iter()
            .find(|(_, c)| c.as_str() == code)
            .map(|(s, _)| *s)
    }

    /// 登记“濒死结算结束后”要执行的结算（mechanics `dying-aftermath`）。
    pub fn defer_after_dying(&mut self, entry: AfterDying) {
        self.after_dying.push(entry);
    }

    /// 濒死真正脱困/死亡后的派发点（`flow::settle_after_dying` 唯一调用）。
    pub fn drain_after_dying(&mut self) -> Vec<AfterDying> {
        std::mem::take(&mut self.after_dying)
    }

    /// 取拼点结果（唯一读法：读完清空，禁止重复消费）。
    pub fn take_pindian(&mut self) -> Option<crate::kernel::pindian::PindianResult> {
        self.pindian_result.take()
    }

    /// 化身初始化（mechanics `general-pool` / 2026-11-16 用户裁定）：
    /// 池 = `Status::Live` ∩ **至少一个可授予技能**（已注册，且非锁定/觉醒/主公/授予技）
    /// ∩ 非「同场存活武将」∩ 非自己 ∩ 非已持有化身牌。
    ///
    /// 随机取牌一律走 `rand_below`（确定性、可回放）。
    pub fn init_huashen(&mut self, seat: usize, count: usize) {
        let mut pool: Vec<String> = skills::huashen_generals(self, seat);
        pool.sort();
        let mut chosen = Vec::new();
        for _ in 0..count.min(pool.len()) {
            let idx = self.rand_below(pool.len());
            chosen.push(pool.remove(idx));
        }
        let current = chosen.first().cloned();
        self.huashen.insert(
            seat,
            Huashen {
                pool: chosen,
                current,
                active_skill: None,
            },
        );
    }

    /// 展示/切换化身牌：**改写** `Seat.kingdom` / `Seat.male` 跟随展示武将；
    /// `None` = 移去展示 → 从 `Seat.general` 重新读取原值（不另存第二份事实）。
    pub fn huashen_set_current(&mut self, seat: usize, code: Option<&str>) {
        let display: Option<String> = match code {
            Some(c) => Some(c.to_string()),
            None => self
                .seats
                .get(seat)
                .map(|s| s.general.clone())
                .filter(|c| !c.is_empty()),
        };
        let Some(display) = display else { return };
        let Some(gd) = crate::kernel::generals::def(&display) else {
            return;
        };
        if let Some(s) = self.seats.get_mut(seat) {
            s.kingdom = gd.kingdom.to_string();
            s.male = gd.male;
        }
        if let Some(h) = self.huashen.get_mut(&seat) {
            h.current = code.map(|c| c.to_string());
        }
    }

    /// 当前化身的“性别/势力”覆盖（读化身时跟随展示武将；移去后恢复原值）。
    pub fn huashen_general(&self, seat: usize) -> Option<&str> {
        self.huashen.get(&seat).and_then(|h| h.current.as_deref())
    }

    /// 废除一个装备栏（决堰/破势/雄乱；**永久**，不可再装备）。
    pub fn abolish_slot(&mut self, seat: usize, slot: Slot) {
        if let Some(s) = self.seats.get_mut(seat) {
            s.abolished_slots.insert(slot);
        }
    }

    /// 装备栏是否已废除（**唯一读法**：装备入口/候选/视图共用）。
    pub fn slot_abolished(&self, seat: usize, slot: Slot) -> bool {
        self.seats
            .get(seat)
            .map(|s| s.abolished_slots.contains(&slot))
            .unwrap_or(false)
    }

    /// 废除判定区（雄乱）：延时锦囊不可进入/流转跳过。
    pub fn abolish_judge_zone(&mut self, seat: usize) {
        if let Some(s) = self.seats.get_mut(seat) {
            s.judge_abolished = true;
        }
    }
}
