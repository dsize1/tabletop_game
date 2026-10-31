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
    /// P4.4.1 风包技能帧。
    Kuanggu(crate::resolve_wind::KuangguFrame),
    Qimou(crate::resolve_wind::QimouFrame),
    Jushou(crate::resolve_wind::JushouFrame),
    Jiewei(crate::resolve_wind::JieweiFrame),
    Fenji(crate::resolve_wind::FenjiFrame),
    Leiji(crate::resolve_wind::LeijiFrame),
    Shensu(crate::resolve_wind::ShensuFrame),
    Tianxiang(crate::resolve_wind::TianxiangFrame),
    Guhuo(crate::resolve_wind::GuhuoFrame),
    /// P4.4.1 火包帧。
    Pindian(crate::resolve_fire::PindianFrame),
    Qiangxi(crate::resolve_fire::QiangxiFrame),
    Quhu(crate::resolve_fire::QuhuFrame),
    Jieming(crate::resolve_fire::JiemingFrame),
    Tianyi(crate::resolve_fire::TianyiFrame),
    Jiandu(crate::resolve_fire::JianduFrame),
    Shuangxiong(crate::resolve_fire::ShuangxiongFrame),
    /// P4.4.1 林包帧。
    Xingshang(crate::resolve_forest::XingshangFrame),
    Fangzhu(crate::resolve_forest::FangzhuFrame),
    Yinghun(crate::resolve_forest::YinghunFrame),
    Bengkuai(crate::resolve_forest::BengkuaiFrame),
    Liewen(crate::resolve_forest::LiewenFrame),
    Zaizhi(crate::resolve_forest::ZaizhiFrame),
    Haoshi(crate::resolve_forest::HaoshiFrame),
    Dimeng(crate::resolve_forest::DimengFrame),
    Luanwu(crate::resolve_forest::LuanwuFrame),
    HaoshiOffer(crate::resolve_forest::HaoshiOfferFrame),
    /// P4.4.1 山包帧。
    Awaken(crate::resolve_mountain::AwakenFrame),
    Qiaobian(crate::resolve_mountain::QiaobianFrame),
    Tiaoxin(crate::resolve_mountain::TiaoxinFrame),
    Xiangle(crate::resolve_mountain::XiangleFrame),
    Fangquan(crate::resolve_mountain::FangquanFrame),
    Zhiba(crate::resolve_mountain::ZhibaFrame),
    Zhijian(crate::resolve_mountain::ZhijianFrame),
    Guzheng(crate::resolve_mountain::GuzhengFrame),
    Huashen(crate::resolve_mountain::HuashenFrame),
    Beige(crate::resolve_mountain::BeigeFrame),
    /// P4.4.1 SP-1 帧。
    Qizhi(crate::resolve_sp::QizhiFrame),
    Jinzou(crate::resolve_sp::JinzouFrame),
    Juzhan(crate::resolve_sp::JuzhanFrame),
    Feijun(crate::resolve_sp::FeijunFrame),
    Jianxiang(crate::resolve_sp::JianxiangFrame),
    Shenshi(crate::resolve_sp::ShenshiFrame),
    Chenglue(crate::resolve_sp::ChenglueFrame),
    Yili(crate::resolve_sp::YiliFrame),
    Zhenglun(crate::resolve_sp::ZhenglunFrame),
    Kuizhu(crate::resolve_sp::KuizhuFrame),
    Chezheng(crate::resolve_sp::ChezhengFrame),
    Lijun(crate::resolve_sp::LijunFrame),
    /// P4.4.1 SP-2 帧。
    MingRen(crate::resolve_sp2::MingRenFrame),
    ZhenLiang(crate::resolve_sp2::ZhenLiangFrame),
    ZhenLiangYin(crate::resolve_sp2::ZhenLiangYinFrame),
    ZhenGu(crate::resolve_sp2::ZhenGuFrame),
    ZuiLun(crate::resolve_sp2::ZuiLunFrame),
    LiangYin(crate::resolve_sp2::LiangYinFrame),
    KongSheng(crate::resolve_sp2::KongShengFrame),
    ZhengRong(crate::resolve_sp2::ZhengRongFrame),
    JueYan(crate::resolve_sp2::JueYanFrame),
    XiongLuan(crate::resolve_sp2::XiongLuanFrame),
    CongJian(crate::resolve_sp2::CongJianFrame),
    /// P4.4.2 一将成名帧（批次 1）。
    LuoYing(crate::resolve_yijiang::LuoYingFrame),
    JiuShi(crate::resolve_yijiang::JiuShiFrame),
    XianZhen(crate::resolve_yijiang::XianZhenFrame),
    MingCe(crate::resolve_yijiang::MingCeFrame),
    XuanFeng(crate::resolve_yijiang::XuanFengFrame),
    SanYao(crate::resolve_yijiang::SanYaoFrame),
    ZhiMan(crate::resolve_yijiang::ZhiManFrame),
    GanLu(crate::resolve_yijiang::GanLuFrame),
    PoJun(crate::resolve_yijiang::PoJunFrame),
    ZhenJun(crate::resolve_yijiang::ZhenJunFrame),
    /// P4.4.2 界标·蜀帧（批次 2）。
    YiJue(crate::resolve_jiexian::YiJueFrame),
    RendeBasic(crate::resolve_jiexian::RendeBasicFrame),
    YaJiao(crate::resolve_jiexian::YaJiaoFrame),
    TieQi(crate::resolve_jiexian::TieQiFrame),
    /// P4.4.2 界标·吴帧（批次 3）。
    FenWei(crate::resolve_jiexian::FenWeiFrame),
    JieFanJian(crate::resolve_jiexian::JieFanJianFrame),
    GuoSe(crate::resolve_jiexian::GuoSeFrame),
    QinXue(crate::resolve_jiexian::QinXueFrame),
    GongXin(crate::resolve_jiexian::GongXinFrame),
    LianYing(crate::resolve_jiexian::LianYingFrame),
    /// P4.4.2 界标·魏帧（批次 4）。
    QingJian(crate::resolve_jiexian::QingJianFrame),
    XunXun(crate::resolve_jiexian::XunXunFrame),
    WangXi(crate::resolve_jiexian::WangXiFrame),
    /// P4.4.2 界标·群帧（批次 5）。
    ChuLi(crate::resolve_jiexian::ChuLiFrame),
    LiYu(crate::resolve_jiexian::LiYuFrame),
    /// P4.4.2 一将成名（批次 2）。
    JiangChi(crate::resolve_yijiang2::JiangChiFrame),
    Qice(crate::resolve_yijiang2::QiceFrame),
    QianXi(crate::resolve_yijiang2::QianXiFrame),
    AnXu(crate::resolve_yijiang2::AnXuFrame),
    ZhuiYi(crate::resolve_yijiang2::ZhuiYiFrame),
    LiHuo(crate::resolve_yijiang2::LiHuoFrame),
    ChunLao(crate::resolve_yijiang2::ChunLaoFrame),
    GongQi(crate::resolve_yijiang2::GongQiFrame),
    JieFan(crate::resolve_yijiang2::JieFanFrame),
    /// P4.4.2 授予技能（清侧）。
    QingCe(crate::resolve_sp2::QingCeFrame),
    /// P4.4.2 一将成名（批次 Y1）。
    ChengXiang(crate::resolve_yijiang3::ChengXiangFrame),
    RenXin(crate::resolve_yijiang3::RenXinFrame),
    ZiLi(crate::resolve_yijiang3::ZiLiFrame),
    PaiYi(crate::resolve_yijiang3::PaiYiFrame),
    ZiShou(crate::resolve_yijiang3::ZiShouFrame),
    JunXing(crate::resolve_yijiang3::JunXingFrame),
    YuCe(crate::resolve_yijiang3::YuCeFrame),
    LongYin(crate::resolve_yijiang3::LongYinFrame),
    QiaoShuo(crate::resolve_yijiang3::QiaoShuoFrame),
    /// P4.4.2 一将成名（批次 Y2）。
    XianSi(crate::resolve_yijiang4::XianSiFrame),
    DuoDao(crate::resolve_yijiang4::DuoDaoFrame),
    ZongXuan(crate::resolve_yijiang4::ZongXuanFrame),
    ZhiYan(crate::resolve_yijiang4::ZhiYanFrame),
    DanShou(crate::resolve_yijiang4::DanShouFrame),
    ZhuiKong(crate::resolve_yijiang4::ZhuiKongFrame),
    QiuYuan(crate::resolve_yijiang4::QiuYuanFrame),
    MieJi(crate::resolve_yijiang4::MieJiFrame),
    JueCe(crate::resolve_yijiang4::JueCeFrame),
    FenCheng(crate::resolve_yijiang4::FenChengFrame),
    EnYuan(crate::resolve_yijiang4::EnYuanFrame),
    XuanHuo(crate::resolve_yijiang4::XuanHuoFrame),
    JuJian(crate::resolve_yijiang4::JuJianFrame),
    /// P4.4.2 一将成名（批次 Y3）。
    ZhenLie(crate::resolve_yijiang5::ZhenLieFrame),
    MiJi(crate::resolve_yijiang5::MiJiFrame),
    SiDi(crate::resolve_yijiang5::SiDiFrame),
    ShanDuan(crate::resolve_yijiang5::ShanDuanFrame),
    YongLue(crate::resolve_yijiang5::YongLueFrame),
    PinDi(crate::resolve_yijiang5::PinDiFrame),
    BenXi(crate::resolve_yijiang5::BenXiFrame),
    ZhongYong(crate::resolve_yijiang5::ZhongYongFrame),
    QiangShi(crate::resolve_yijiang5::QiangShiFrame),
    XianTu(crate::resolve_yijiang5::XianTuFrame),
    /// P4.4.2 一将成名（批次 Y4）。
    ZanHui(crate::resolve_yijiang6::ZanHuiFrame),
    JiaoJin(crate::resolve_yijiang6::JiaoJinFrame),
    FenLi(crate::resolve_yijiang6::FenLiFrame),
    PingKou(crate::resolve_yijiang6::PingKouFrame),
    ShenXing(crate::resolve_yijiang6::ShenXingFrame),
    BingYi(crate::resolve_yijiang6::BingYiFrame),
    QieTing(crate::resolve_yijiang6::QieTingFrame),
    XianZhou(crate::resolve_yijiang6::XianZhouFrame),
    HuiTuo(crate::resolve_yijiang6::HuiTuoFrame),
    MingJian(crate::resolve_yijiang6::MingJianFrame),
    QingXi(crate::resolve_yijiang6::QingXiFrame),
    ZuoDing(crate::resolve_yijiang6::ZuoDingFrame),
    /// P4.4.2 一将成名（批次 Y5）。
    QiaoShi(crate::resolve_yijiang7::QiaoShiFrame),
    YanYu(crate::resolve_yijiang7::YanYuFrame),
    WuRong(crate::resolve_yijiang7::WuRongFrame),
    YanZhu(crate::resolve_yijiang7::YanZhuFrame),
    XingXue(crate::resolve_yijiang7::XingXueFrame),
    ZhaoFu(crate::resolve_yijiang7::ZhaoFuFrame),
    AnGuo(crate::resolve_yijiang7::AnGuoFrame),
    YaoMing(crate::resolve_yijiang7::YaoMingFrame),
    HuaiYi(crate::resolve_yijiang7::HuaiYiFrame),
    JiGong(crate::resolve_yijiang7::JiGongFrame),
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
    /// 往烈（陈到）：不能被【无懈可击】响应。
    #[serde(default)]
    pub no_wuxie: bool,
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
    /// 神速：视为使用的【杀】无距离限制。
    #[serde(default)]
    pub no_distance: bool,
    /// 视为使用的虚拟【杀】（无物理牌；`card` 为空串）。
    #[serde(default)]
    pub virtual_card: bool,
    /// 天义（赢）：本【杀】无距离限制且可多指定一个目标。
    #[serde(default)]
    pub tianyi: bool,
    /// 往烈（陈到）：此【杀】不能被响应（目标不能使用【闪】）。
    #[serde(default)]
    pub no_respond: bool,
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
    /// 挑衅：弃置目标一张牌。
    Tiaoxin,
    /// 奇制：弃置目标一张牌（然后目标摸 1）。
    Qizhi,
    /// 拒战（阴）：获得目标一张牌。
    Juzhan,
    /// 掣政：弃置目标一张牌。
    Chezheng,
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
    /// 拼点结果（`winner = None` 表示平手，发起者未赢）。
    Pindian {
        winner: Option<usize>,
        initiator: usize,
        target: usize,
        #[serde(default)]
        initiator_card: String,
        #[serde(default)]
        target_card: String,
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
    crate::skills::note_discard_suit(st, id);
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
        // 利驭（界吕布）：栈空后询问（避免覆盖父帧 outcome）。
        if top(st).is_none() && !st.pending_liyu.is_empty() {
            let (actor, target) = st.pending_liyu.remove(0);
            push(
                st,
                Frame::LiYu(crate::resolve_jiexian::LiYuFrame {
                    actor,
                    target,
                    stage: crate::resolve_jiexian::LiYuStage::Ask,
                }),
            );
            continue;
        }
        // 界连营：失去所有手牌后，栈空时询问令一名角色摸 1。
        if top(st).is_none() {
            if let Some(seat) = crate::resolve_jiexian::lianying_ready(st) {
                st.set_counter(&format!("lianying_pending:{seat}"), 0);
                push(
                    st,
                    Frame::LianYing(crate::resolve_jiexian::LianYingFrame { actor: seat }),
                );
                continue;
            }
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
        Frame::Kuanggu(f) => crate::resolve_wind::step_kuanggu(st, f),
        Frame::Qimou(f) => crate::resolve_wind::step_qimou(st, f),
        Frame::Jushou(f) => crate::resolve_wind::step_jushou(st, f),
        Frame::Jiewei(f) => crate::resolve_wind::step_jiewei(st, f),
        Frame::Fenji(f) => crate::resolve_wind::step_fenji(st, f),
        Frame::Leiji(f) => crate::resolve_wind::step_leiji(st, f),
        Frame::Shensu(f) => crate::resolve_wind::step_shensu(st, f),
        Frame::Tianxiang(f) => crate::resolve_wind::step_tianxiang(st, f),
        Frame::Guhuo(f) => crate::resolve_wind::step_guhuo(st, f),
        Frame::Pindian(f) => crate::resolve_fire::step_pindian(st, f),
        Frame::Qiangxi(f) => crate::resolve_fire::step_qiangxi(st, f),
        Frame::Quhu(f) => crate::resolve_fire::step_quhu(st, f),
        Frame::Jieming(f) => crate::resolve_fire::step_jieming(st, f),
        Frame::Tianyi(f) => crate::resolve_fire::step_tianyi(st, f),
        Frame::Jiandu(f) => crate::resolve_fire::step_jiandu(st, f),
        Frame::Shuangxiong(f) => crate::resolve_fire::step_shuangxiong(st, f),
        Frame::Xingshang(f) => crate::resolve_forest::step_xingshang(st, f),
        Frame::Fangzhu(f) => crate::resolve_forest::step_fangzhu(st, f),
        Frame::Yinghun(f) => crate::resolve_forest::step_yinghun(st, f),
        Frame::Bengkuai(f) => crate::resolve_forest::step_bengkuai(st, f),
        Frame::Liewen(f) => crate::resolve_forest::step_liewen(st, f),
        Frame::Zaizhi(f) => crate::resolve_forest::step_zaizhi(st, f),
        Frame::Haoshi(f) => crate::resolve_forest::step_haoshi(st, f),
        Frame::Dimeng(f) => crate::resolve_forest::step_dimeng(st, f),
        Frame::Luanwu(f) => crate::resolve_forest::step_luanwu(st, f),
        Frame::HaoshiOffer(f) => crate::resolve_forest::step_haoshi_offer(st, f),
        Frame::Awaken(f) => crate::resolve_mountain::step_awaken(st, f),
        Frame::Qiaobian(f) => crate::resolve_mountain::step_qiaobian(st, f),
        Frame::Tiaoxin(f) => crate::resolve_mountain::step_tiaoxin(st, f),
        Frame::Xiangle(f) => crate::resolve_mountain::step_xiangle(st, f),
        Frame::Fangquan(f) => crate::resolve_mountain::step_fangquan(st, f),
        Frame::Zhiba(f) => crate::resolve_mountain::step_zhiba(st, f),
        Frame::Zhijian(f) => crate::resolve_mountain::step_zhijian(st, f),
        Frame::Guzheng(f) => crate::resolve_mountain::step_guzheng(st, f),
        Frame::Huashen(f) => crate::resolve_mountain::step_huashen(st, f),
        Frame::Beige(f) => crate::resolve_mountain::step_beige(st, f),
        Frame::Qizhi(f) => crate::resolve_sp::step_qizhi(st, f),
        Frame::Jinzou(f) => crate::resolve_sp::step_jinzou(st, f),
        Frame::Juzhan(f) => crate::resolve_sp::step_juzhan(st, f),
        Frame::Feijun(f) => crate::resolve_sp::step_feijun(st, f),
        Frame::Jianxiang(f) => crate::resolve_sp::step_jianxiang(st, f),
        Frame::Shenshi(f) => crate::resolve_sp::step_shenshi(st, f),
        Frame::Chenglue(f) => crate::resolve_sp::step_chenglue(st, f),
        Frame::Yili(f) => crate::resolve_sp::step_yili(st, f),
        Frame::Zhenglun(f) => crate::resolve_sp::step_zhenglun(st, f),
        Frame::Kuizhu(f) => crate::resolve_sp::step_kuizhu(st, f),
        Frame::Chezheng(f) => crate::resolve_sp::step_chezheng(st, f),
        Frame::Lijun(f) => crate::resolve_sp::step_lijun(st, f),
        Frame::MingRen(f) => crate::resolve_sp2::step_mingren(st, f),
        Frame::ZhenLiang(f) => crate::resolve_sp2::step_zhenliang(st, f),
        Frame::ZhenLiangYin(f) => crate::resolve_sp2::step_zhenliang_yin(st, f),
        Frame::ZhenGu(f) => crate::resolve_sp2::step_zhengu(st, f),
        Frame::ZuiLun(f) => crate::resolve_sp2::step_zuilun(st, f),
        Frame::LiangYin(f) => crate::resolve_sp2::step_liangyin(st, f),
        Frame::KongSheng(f) => crate::resolve_sp2::step_kongsheng(st, f),
        Frame::ZhengRong(f) => crate::resolve_sp2::step_zhengrong(st, f),
        Frame::JueYan(f) => crate::resolve_sp2::step_jueyan(st, f),
        Frame::XiongLuan(f) => crate::resolve_sp2::step_xiongluan(st, f),
        Frame::CongJian(f) => crate::resolve_sp2::step_congjian(st, f),
        Frame::LuoYing(f) => crate::resolve_yijiang::step_luoying(st, f),
        Frame::JiuShi(f) => crate::resolve_yijiang::step_jiushi(st, f),
        Frame::XianZhen(f) => crate::resolve_yijiang::step_xianzhen(st, f),
        Frame::MingCe(f) => crate::resolve_yijiang::step_mingce(st, f),
        Frame::XuanFeng(f) => crate::resolve_yijiang::step_xuanfeng(st, f),
        Frame::SanYao(f) => crate::resolve_yijiang::step_sanyao(st, f),
        Frame::ZhiMan(f) => crate::resolve_yijiang::step_zhiman(st, f),
        Frame::GanLu(f) => crate::resolve_yijiang::step_ganlu(st, f),
        Frame::PoJun(f) => crate::resolve_yijiang::step_pojun(st, f),
        Frame::ZhenJun(f) => crate::resolve_yijiang::step_zhenjun(st, f),
        Frame::YiJue(f) => crate::resolve_jiexian::step_yijue(st, f),
        Frame::RendeBasic(f) => crate::resolve_jiexian::step_rende_basic(st, f),
        Frame::YaJiao(f) => crate::resolve_jiexian::step_yajiao(st, f),
        Frame::TieQi(f) => crate::resolve_jiexian::step_tieqi(st, f),
        Frame::FenWei(f) => crate::resolve_jiexian::step_fenwei(st, f),
        Frame::JieFanJian(f) => crate::resolve_jiexian::step_jie_fanjian(st, f),
        Frame::GuoSe(f) => crate::resolve_jiexian::step_guose(st, f),
        Frame::QinXue(f) => crate::resolve_jiexian::step_qinxue(st, f),
        Frame::GongXin(f) => crate::resolve_jiexian::step_gongxin(st, f),
        Frame::LianYing(f) => crate::resolve_jiexian::step_lianying(st, f),
        Frame::QingJian(f) => crate::resolve_jiexian::step_qingjian(st, f),
        Frame::XunXun(f) => crate::resolve_jiexian::step_xunxun(st, f),
        Frame::WangXi(f) => crate::resolve_jiexian::step_wangxi(st, f),
        Frame::ChuLi(f) => crate::resolve_jiexian::step_chuli(st, f),
        Frame::LiYu(f) => crate::resolve_jiexian::step_liyu(st, f),
        Frame::JiangChi(f) => crate::resolve_yijiang2::step_jiangchi(st, f),
        Frame::Qice(f) => crate::resolve_yijiang2::step_qice(st, f),
        Frame::QianXi(f) => crate::resolve_yijiang2::step_qianxi(st, f),
        Frame::AnXu(f) => crate::resolve_yijiang2::step_anxu(st, f),
        Frame::ZhuiYi(f) => crate::resolve_yijiang2::step_zhuiyi(st, f),
        Frame::LiHuo(f) => crate::resolve_yijiang2::step_lihuo(st, f),
        Frame::ChunLao(f) => crate::resolve_yijiang2::step_chunlao(st, f),
        Frame::GongQi(f) => crate::resolve_yijiang2::step_gongqi(st, f),
        Frame::JieFan(f) => crate::resolve_yijiang2::step_jiefan(st, f),
        Frame::QingCe(f) => crate::resolve_sp2::step_qingce(st, f),
        Frame::ChengXiang(f) => crate::resolve_yijiang3::step_chengxiang(st, f),
        Frame::RenXin(f) => crate::resolve_yijiang3::step_renxin(st, f),
        Frame::ZiLi(f) => crate::resolve_yijiang3::step_zili(st, f),
        Frame::PaiYi(f) => crate::resolve_yijiang3::step_paiyi(st, f),
        Frame::ZiShou(f) => crate::resolve_yijiang3::step_zishou(st, f),
        Frame::JunXing(f) => crate::resolve_yijiang3::step_junxing(st, f),
        Frame::YuCe(f) => crate::resolve_yijiang3::step_yuce(st, f),
        Frame::LongYin(f) => crate::resolve_yijiang3::step_longyin(st, f),
        Frame::QiaoShuo(f) => crate::resolve_yijiang3::step_qiaoshuo(st, f),
        Frame::XianSi(f) => crate::resolve_yijiang4::step_xiansi(st, f),
        Frame::DuoDao(f) => crate::resolve_yijiang4::step_duodao(st, f),
        Frame::ZongXuan(f) => crate::resolve_yijiang4::step_zongxuan(st, f),
        Frame::ZhiYan(f) => crate::resolve_yijiang4::step_zhiyan(st, f),
        Frame::DanShou(f) => crate::resolve_yijiang4::step_danshou(st, f),
        Frame::ZhuiKong(f) => crate::resolve_yijiang4::step_zhuikong(st, f),
        Frame::QiuYuan(f) => crate::resolve_yijiang4::step_qiuyuan(st, f),
        Frame::MieJi(f) => crate::resolve_yijiang4::step_mieji(st, f),
        Frame::JueCe(f) => crate::resolve_yijiang4::step_juece(st, f),
        Frame::FenCheng(f) => crate::resolve_yijiang4::step_fencheng(st, f),
        Frame::EnYuan(f) => crate::resolve_yijiang4::step_enyuan(st, f),
        Frame::XuanHuo(f) => crate::resolve_yijiang4::step_xuanhuo(st, f),
        Frame::JuJian(f) => crate::resolve_yijiang4::step_jujian(st, f),
        Frame::ZhenLie(f) => crate::resolve_yijiang5::step_zhenlie(st, f),
        Frame::MiJi(f) => crate::resolve_yijiang5::step_miji(st, f),
        Frame::SiDi(f) => crate::resolve_yijiang5::step_sidi(st, f),
        Frame::ShanDuan(f) => crate::resolve_yijiang5::step_shenduan(st, f),
        Frame::YongLue(f) => crate::resolve_yijiang5::step_yonglue(st, f),
        Frame::PinDi(f) => crate::resolve_yijiang5::step_pindi(st, f),
        Frame::BenXi(f) => crate::resolve_yijiang5::step_benxi(st, f),
        Frame::ZhongYong(f) => crate::resolve_yijiang5::step_zhongyong(st, f),
        Frame::QiangShi(f) => crate::resolve_yijiang5::step_qiangshi(st, f),
        Frame::XianTu(f) => crate::resolve_yijiang5::step_xiantu(st, f),
        Frame::ZanHui(f) => crate::resolve_yijiang6::step_zanhui(st, f),
        Frame::JiaoJin(f) => crate::resolve_yijiang6::step_jiaojin(st, f),
        Frame::FenLi(f) => crate::resolve_yijiang6::step_fenli(st, f),
        Frame::PingKou(f) => crate::resolve_yijiang6::step_pingkou(st, f),
        Frame::ShenXing(f) => crate::resolve_yijiang6::step_shenxing(st, f),
        Frame::BingYi(f) => crate::resolve_yijiang6::step_bingyi(st, f),
        Frame::QieTing(f) => crate::resolve_yijiang6::step_qieting(st, f),
        Frame::XianZhou(f) => crate::resolve_yijiang6::step_xianzhou(st, f),
        Frame::HuiTuo(f) => crate::resolve_yijiang6::step_huituo(st, f),
        Frame::MingJian(f) => crate::resolve_yijiang6::step_mingjian(st, f),
        Frame::QingXi(f) => crate::resolve_yijiang6::step_qingxi(st, f),
        Frame::ZuoDing(f) => crate::resolve_yijiang6::step_zuoding(st, f),
        Frame::QiaoShi(f) => crate::resolve_yijiang7::step_qiaoshi(st, f),
        Frame::YanYu(f) => crate::resolve_yijiang7::step_yanyu(st, f),
        Frame::WuRong(f) => crate::resolve_yijiang7::step_wurong(st, f),
        Frame::YanZhu(f) => crate::resolve_yijiang7::step_yanzhu(st, f),
        Frame::XingXue(f) => crate::resolve_yijiang7::step_xingxue(st, f),
        Frame::ZhaoFu(f) => crate::resolve_yijiang7::step_zhaofu(st, f),
        Frame::AnGuo(f) => crate::resolve_yijiang7::step_anguo(st, f),
        Frame::YaoMing(f) => crate::resolve_yijiang7::step_yaoming(st, f),
        Frame::HuaiYi(f) => crate::resolve_yijiang7::step_huaiyi(st, f),
        Frame::JiGong(f) => crate::resolve_yijiang7::step_jigong(st, f),
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
    // 解围：装备区的牌可当【无懈可击】使用（即使没有手牌也要询问）。
    let jiewei_equips: Vec<String> = if crate::skills::has(st, seat, "jiewei") {
        st.seats[seat]
            .equips
            .values()
            .filter_map(|c| c.clone())
            .collect()
    } else {
        Vec::new()
    };
    let chanyuan_locked = crate::skills::chanyuan_locked(st, seat);
    // 无手牌（或缠怨锁手牌）且无解围装备 = 公共信息，无法打出任何响应牌，无需假犹豫。
    if !alive_of_order(st, &pid)
        || ((st.seats[seat].hand.is_empty() || chanyuan_locked) && jiewei_equips.is_empty())
    {
        f.cursor += 1;
        f.passed += 1;
        replace_top(st, Frame::Wuxie(f));
        return Ok(false);
    }
    let mut options: Vec<PendingOption> = if chanyuan_locked {
        Vec::new()
    } else {
        turn::hand_cards_with_def(st, seat, "wuxie_keji")
            .iter()
            .map(|id| PendingOption {
                id: format!("respond:{id}"),
                label: format!("打出【{}】", turn::card_label(st, id)),
            })
            .collect()
    };
    for id in &jiewei_equips {
        options.push(PendingOption {
            id: format!("skill:jiewei:{id}"),
            label: format!("解围：将装备【{}】当【无懈可击】", turn::card_label(st, id)),
        });
    }
    // 看破：黑色手牌当【无懈可击】。
    if !chanyuan_locked {
        for id in &st.seats[seat].hand {
            if let Some((_, name)) = crate::skills::conversion_for(st, seat, id, "wuxie_keji") {
                options.push(PendingOption {
                    id: format!("respond:{id}"),
                    label: format!(
                        "{name}：将【{}】当【无懈可击】打出",
                        turn::card_label(st, id)
                    ),
                });
            }
        }
    }
    // 蛊惑：任意手牌当【无懈可击】（每回合限一次）。
    if !chanyuan_locked && crate::resolve_wind::guhuo_available(st, seat) {
        for id in &st.seats[seat].hand {
            options.push(PendingOption {
                id: format!("skill:guhuo:{id}"),
                label: format!("蛊惑：将【{}】当【无懈可击】", turn::card_label(st, id)),
            });
        }
    }
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
            // 神速：视为使用的【杀】无距离限制（仍受空城等目标锁定技限制）。
            let free = crate::skills::chenglue_free(st, f.user, &f.card);
            let targets = if f.no_distance || free {
                st.alive_indices()
                    .into_iter()
                    .filter(|t| {
                        *t != f.user
                            && !crate::skills::kongcheng_blocks(st, *t)
                            && !crate::skills::deny_target(st, f.user, *t)
                    })
                    .collect()
            } else {
                turn::sha_targets(st, f.user)
            };
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
            // 天义（赢）：可多指定一个目标（恰好 2 个）。
            if f.tianyi && targets.len() >= 2 {
                for combo in fangtian_combos(&targets, 2) {
                    if combo.len() != 2 {
                        continue;
                    }
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
                        label: format!("天义：同时指定 {}", names.join("、")),
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
            Some(Outcome::Respond { played, .. }) => {
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
                crate::resolve_yijiang5::yonglue_on_sha_finished(st, f.target);
                crate::resolve_yijiang5::zhongyong_on_sha_finished(
                    st,
                    f.user,
                    f.target,
                    &f.card,
                    played.clone(),
                );
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
    start_sha_flags(st, user, card, fangtian, damage_type, false, false)
}

/// 把当前结算栈上最近的【杀】标记为无距离（界武圣·方片杀）。
pub(crate) fn mark_sha_no_distance(st: &mut SgsState) {
    for f in st.resolution.iter_mut().rev() {
        if let Frame::Sha(s) = f {
            s.no_distance = true;
            return;
        }
    }
}

/// 把当前结算栈上最近的【杀】标记为「不可响应」（诈降·红杀）。
pub(crate) fn mark_sha_no_respond(st: &mut SgsState) {
    for f in st.resolution.iter_mut().rev() {
        if let Frame::Sha(s) = f {
            s.no_respond = true;
            return;
        }
    }
}

/// 带「无距离/天义多目标」标记的【杀】。
pub fn start_sha_flags(
    st: &mut SgsState,
    user: usize,
    card: &str,
    fangtian: bool,
    damage_type: DamageType,
    no_distance: bool,
    tianyi: bool,
) -> Result<(), EngineError> {
    st.deck.processing.push(card.to_string());
    st.bump_counter("sha_used");
    if st.turn == user {
        st.bump_counter("play_cards_used");
    }
    // 恃才（许攸）：本回合首次使用基本牌后置于牌堆顶并摸 1。
    crate::skills::shicai_trigger(st, user, "basic", card);
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
            no_distance,
            virtual_card: false,
            tianyi,
            no_respond: false,
        }),
    );
    // 龙吟（关平）：使用【杀】时（压在用牌帧之上先询问）。
    if st.turn == user && st.turn_phase == TurnPhase::Play {
        crate::skills::longyin_trigger(st, user, card);
    }
    // 征荣（毌丘俭）：使用【杀】时（压在用牌帧之上先询问）。
    crate::resolve_sp2::zhengrong_trigger(st, user, card);
    // 贞良（阴，卢植）：回合外使用的牌进入弃牌堆（按使用时机结算）。
    crate::resolve_sp2::push_zhenliang_yin(st, user, card);
    // 奇制（王基）：回合内使用非装备牌后。
    crate::resolve_sp::qizhi_trigger(st, user);
    Ok(())
}

// --- 锦囊 -----------------------------------------------------------------

fn needs_wuxie(code: &str) -> bool {
    !matches!(code, "lebu_sishu" | "shandian" | "bingliang_cunduan")
}

fn trick_targets(st: &SgsState, code: &str, user: usize, card: &str) -> Vec<usize> {
    match code {
        "nanman_ruqin" => st
            .seats_from_left_of(user)
            .into_iter()
            .filter(|i| {
                st.seats[*i].alive
                    && !crate::skills::nanman_immune(st, *i)
                    && !crate::skills::weimu_blocks(st, *i, card)
                    && !crate::skills::deny_target(st, user, *i)
            })
            .collect(),
        "wanjian_qifa" => st
            .seats_from_left_of(user)
            .into_iter()
            .filter(|i| {
                st.seats[*i].alive
                    && !crate::skills::weimu_blocks(st, *i, card)
                    && !crate::skills::deny_target(st, user, *i)
            })
            .collect(),
        "taoyuan_jieyi" => {
            let mut v = vec![user];
            v.extend(st.seats_from_left_of(user).into_iter().filter(|i| {
                st.seats[*i].alive
                    && *i != user
                    && !crate::skills::weimu_blocks(st, *i, card)
                    && !crate::skills::deny_target(st, user, *i)
            }));
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
    let mut targets = trick_targets(st, &code, user, card);
    if code == "wuzhong_shengyou" || code == "shandian" {
        // 无中生有与闪电都只作用于自己（闪电置入自己判定区，下回合最先判定）。
        targets = vec![user];
        targets.retain(|i| !crate::skills::weimu_blocks(st, *i, card));
    }
    // 转化后的延时锦囊：记录生效定义（判定区/判定结算按声明牌名）。
    if matches!(
        code.as_str(),
        "lebu_sishu" | "bingliang_cunduan" | "shandian"
    ) {
        st.virtual_defs.insert(card.to_string(), code.clone());
    }
    let stage = if targets.is_empty() {
        TrickStage::Target
    } else if needs_wuxie(&code) {
        TrickStage::Wuxie
    } else {
        TrickStage::Effect
    };

    let delayed = matches!(
        code.as_str(),
        "lebu_sishu" | "bingliang_cunduan" | "shandian"
    );
    // 恃才（许攸）：本回合首次使用锦囊后置于牌堆顶并摸 1（延时锦囊不进牌堆顶）。
    if !delayed {
        crate::skills::shicai_trigger(st, user, "trick", card);
    }
    let fenwei_targets = targets.clone();
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
            no_wuxie: false,
        }),
    );
    // 奋威（界甘宁，限定技）：多目标锦囊指定目标后，可令任意个目标无效。
    crate::resolve_jiexian::maybe_fenwei(st, &fenwei_targets);
    if st.turn == user {
        st.bump_counter("play_cards_used");
    }
    // 贞良（阴，卢植）：回合外使用的锦囊。
    crate::resolve_sp2::push_zhenliang_yin(st, user, card);
    // 征荣（毌丘俭）：使用伤害锦囊时（压在用牌帧之上先询问）。
    if matches!(
        st.effective_def_code(card),
        Some("juedou" | "nanman_ruqin" | "wanjian_qifa" | "huo_gong")
    ) {
        crate::resolve_sp2::zhengrong_trigger(st, user, card);
    }
    // 奇制（王基）：回合内使用非装备牌后。
    if !delayed {
        crate::resolve_sp::qizhi_trigger(st, user);
    }
    Ok(())
}

fn trick_done(st: &mut SgsState, f: &TrickFrame) {
    // 巨象：其他角色使用的【南蛮入侵】结算后，获得该牌。
    if f.code == "nanman_ruqin" {
        if let Some(owner) = crate::skills::juxiang_owner(st, f.user) {
            if st.deck.processing.iter().any(|c| c == &f.card) {
                leave_processing(st, &f.card);
                st.seats[owner].hand.push(f.card.clone());
                let over = st.seats[owner]
                    .hand
                    .len()
                    .saturating_sub(st.config.max_hand_cards as usize);
                if over > 0 {
                    push_discard(st, owner, over, "hand_cap");
                }
                st.push_log(
                    Some(&st.seats[owner].player_id.clone()),
                    "巨象：获得【南蛮入侵】",
                );
            }
        } else {
            discard_from_processing(st, &f.card);
        }
    } else if !matches!(f.code.as_str(), "lebu_sishu" | "shandian") {
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
                        .filter(|i| {
                            *i != f.user
                                && st.equip_def(*i, EquipSlot::Weapon).is_some()
                                && !crate::skills::weimu_blocks(st, *i, &f.card)
                        })
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
                                && !crate::skills::weimu_blocks(st, *i, &f.card)
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
                                && !crate::skills::deny_target(st, f.user, *i)
                                && (turn::trick_distance_ok(st, f.user, *i)
                                    || crate::skills::chenglue_free(st, f.user, &f.card))
                                && turn::has_cards_to_take(st, *i)
                        })
                        .collect(),
                    "guohe_chaiqiao" => st
                        .alive_indices()
                        .into_iter()
                        .filter(|i| {
                            *i != f.user
                                && !crate::skills::qianxun_blocks(st, *i)
                                && !crate::skills::deny_target(st, f.user, *i)
                                && turn::has_cards_to_take(st, *i)
                        })
                        .collect(),
                    "lebu_sishu" => st
                        .alive_indices()
                        .into_iter()
                        .filter(|i| {
                            *i != f.user
                                && !turn::judge_zone_has(st, *i, "lebu_sishu")
                                && !st.seats[*i].judge_abolished
                                && !crate::skills::qianjie_blocks_target(st, *i)
                        })
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
                                && (turn::trick_distance_ok(st, f.user, *i)
                                    || crate::skills::has(st, f.user, "shenduan")
                                    || (crate::skills::has(st, f.user, "duanliang")
                                        && st.seats[*i].hand.len() >= st.seats[f.user].hand.len()))
                                && !turn::judge_zone_has(st, *i, "bingliang_cunduan")
                                && !st.seats[*i].judge_abolished
                                && !crate::skills::qianjie_blocks_target(st, *i)
                        })
                        .collect(),
                    "wuzhong_shengyou" => vec![f.user],
                    // 空城（锁定）：无手牌者不能成为【决斗】目标。
                    "juedou" => st
                        .alive_indices()
                        .into_iter()
                        .filter(|i| {
                            *i != f.user
                                && !crate::skills::kongcheng_blocks(st, *i)
                                && !crate::skills::deny_target(st, f.user, *i)
                        })
                        .collect(),
                    _ => st
                        .alive_indices()
                        .into_iter()
                        .filter(|i| *i != f.user && !crate::skills::deny_target(st, f.user, *i))
                        .collect(),
                };
                let candidates: Vec<usize> = candidates
                    .into_iter()
                    .filter(|i| !crate::skills::weimu_blocks(st, *i, &f.card))
                    .collect();
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
            // 往烈（陈到）：此锦囊不能被响应 → 跳过无懈窗口。
            if f.no_wuxie {
                f.stage = TrickStage::Effect;
                replace_top(st, Frame::Trick(f));
                return Ok(false);
            }
            // 从谏（张绣）：成为多目标锦囊目标后。
            if f.targets.len() > 1 && st.counter(&format!("cj:{}", f.card)) == 0 {
                let before = st.resolution.len();
                crate::resolve_sp2::congjian_trigger(st, &f.card, &f.targets);
                if st.resolution.len() > before {
                    // 从谏帧压在用牌帧之上，先结算从谏。
                    return Ok(false);
                }
            }
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
            // 智迟（陈宫，锁定）：本回合普通锦囊对其无效。
            if matches!(
                f.code.as_str(),
                "nanman_ruqin"
                    | "wanjian_qifa"
                    | "juedou"
                    | "huo_gong"
                    | "shunshou_qianyang"
                    | "guohe_chaiqiao"
                    | "tiesuo_lianhuan"
                    | "wugu_fengdeng"
                    | "taoyuan_jieyi"
                    | "wuzhong_shengyou"
            ) {
                let t = if f.code == "jiedao_sharen" {
                    f.first.unwrap_or(f.user)
                } else if f.targets.is_empty() {
                    f.user
                } else {
                    f.targets[f.idx]
                };
                if crate::resolve_yijiang::zhichi_active(st, t) {
                    st.push_log(
                        None,
                        format!("智迟：锦囊对 {} 无效", st.seats[t].general_name),
                    );
                    advance_trick_target(st, f)?;
                    return Ok(false);
                }
            }
            let target = if f.code == "jiedao_sharen" {
                f.first.unwrap_or(f.user)
            } else if f.targets.is_empty() {
                f.user
            } else {
                f.targets[f.idx]
            };
            if target != f.user {
                st.bump_counter("targeted_others");
            }
            // 奋威：已被宣告无效的目标跳过结算。
            if crate::resolve_jiexian::fenwei_void(st, target) {
                st.push_log(
                    None,
                    format!("奋威：此锦囊对 {} 无效", st.seats[target].general_name),
                );
                advance_trick_target(st, f)?;
                return Ok(false);
            }
            // 贞烈（王异）：已宣告无效的目标跳过结算。
            if crate::skills::zhenlie_void(st, &f.card, target) {
                st.push_log(
                    None,
                    format!("贞烈：此锦囊对 {} 无效", st.seats[target].general_name),
                );
                advance_trick_target(st, f)?;
                return Ok(false);
            }
            // 贞烈（王异）：成为普通锦囊目标后，可失去 1 点体力令此牌无效。
            if target != f.user
                && crate::skills::has(st, target, "zhenlie")
                && !crate::skills::zhenlie_skip(st, &f.card, target)
                && !matches!(f.code.as_str(), "wuzhong_shengyou" | "shandian")
            {
                push(
                    st,
                    Frame::ZhenLie(crate::resolve_yijiang5::ZhenLieFrame {
                        owner: target,
                        user: f.user,
                        card: f.card.clone(),
                        kind: crate::resolve_yijiang5::ZhenLieKind::Trick,
                        need: 0,
                        no_shan: false,
                        ignore_armor: false,
                        damage_type: DamageType::Normal,
                    }),
                );
                return Ok(false);
            }
            // 界谦逊：其他角色使用的锦囊仅指定你时，可将所有手牌移出游戏直到回合结束。
            if target != f.user
                && f.targets.len() <= 1
                && crate::skills::has(st, target, "qianxun")
                && crate::skills::is_jiexian(st, target)
                && !st.seats[target].hand.is_empty()
            {
                let cards = std::mem::take(&mut st.seats[target].hand);
                let n = cards.len();
                for c in cards {
                    st.deck.void.push(c.clone());
                    st.void_return.push(VoidReturn {
                        owner: target,
                        card: c,
                        due_seat: st.turn,
                    });
                }
                st.push_log(
                    None,
                    format!(
                        "谦逊：{} 将 {n} 张手牌移出游戏直到回合结束",
                        st.seats[target].general_name
                    ),
                );
            }
            match f.code.as_str() {
                "nanman_ruqin" => {
                    let source = crate::skills::huoshou_owner(st, f.user).unwrap_or(f.user);
                    push(
                        st,
                        Frame::Respond(RespondFrame {
                            kind: RespondKind::Nanman,
                            actor: target,
                            card: f.card.clone(),
                            source,
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
                    // 父荫（诸葛瞻，锁定）：每回合首次成为【决斗】目标且对方手牌 ≥ 你 → 无效。
                    if crate::resolve_sp2::fuyin_take(st, target, f.user) {
                        advance_trick_target(st, f)?;
                        return Ok(false);
                    }
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
                    if crate::skills::qianjie_blocks_target(st, target) {
                        st.push_log(
                            None,
                            format!("谦节：{} 不能进入连环状态", st.seats[target].general_name),
                        );
                        advance_trick_target(st, f)?;
                        return Ok(false);
                    }
                    st.seats[target].chained = !st.seats[target].chained;
                    let chained = st.seats[target].chained;
                    if chained {
                        crate::skills::faen_trigger(st, target);
                    }
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

/// 该拿牌模式是否属于「弃置」（界奇才只防止弃置，不防止获得）。
fn pick_mode_discards(mode: PickMode) -> bool {
    matches!(
        mode,
        PickMode::Guohe | PickMode::Tiaoxin | PickMode::Qizhi | PickMode::Chezheng
    )
}

fn step_pick(st: &mut SgsState, f: PickCardFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = Vec::new();
    let discarding = pick_mode_discards(f.mode);
    for slot in EquipSlot::ALL {
        if discarding && crate::skills::qicai_blocks_discard(st, f.target, f.user, slot) {
            continue;
        }
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
    // 缠怨：体力值为 1 时不能打出手牌（八卦阵/主公技由 step_respond 追加）。
    // 雄乱：目标本回合不能使用/打出手牌（八卦阵/主公技仍可用）。
    if crate::skills::chanyuan_locked(st, f.actor) || crate::skills::hand_sealed(st, f.actor) {
        return vec![pending_pass()];
    }
    let code = match f.kind {
        RespondKind::Shan => "shan",
        RespondKind::Nanman | RespondKind::Duel | RespondKind::Jiedao => "sha",
    };
    // 将驰（曹彰）：本回合不能打出【杀】。
    if code == "sha" && crate::skills::sha_play_banned(st, f.actor) {
        return vec![pending_pass()];
    }
    let mut options: Vec<PendingOption> = turn::hand_cards_with_def(st, f.actor, code)
        .iter()
        // 潜袭（马岱）：不能打出被封锁颜色的手牌。
        .filter(|id| !crate::skills::qianxi_banned(st, f.actor, id))
        .filter(|id| !crate::skills::sidiqu_banned(st, f.actor, id))
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
    // 父魂（关兴张苞）：两张手牌当【杀】打出。
    if code == "sha" {
        options.extend(crate::resolve_yijiang5::fuhen_respond_options(st, f.actor));
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
    // 蛊惑：任意手牌当所需牌打出（每回合限一次）。
    if crate::resolve_wind::guhuo_available(st, f.actor) {
        for id in &st.seats[f.actor].hand {
            options.push(PendingOption {
                id: format!("skill:guhuo:{id}"),
                label: format!(
                    "蛊惑：将【{}】当【{}】打出",
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
        && (st
            .equip_def(f.actor, EquipSlot::Armor)
            .is_some_and(|d| d.code == "bagua_zhen")
            || crate::skills::virtual_equip(st, f.actor).is_some_and(|v| v == "bagua_zhen"))
    {
        options.insert(
            options.len().saturating_sub(1),
            PendingOption {
                id: "skill:bagua".into(),
                label: "八卦阵：判定，红色视为出【闪】".into(),
            },
        );
    }
    // 饰非（郭图逢纪）：需要【闪】时可用（令当前回合角色摸 1，非唯一最多则弃最多者一张）。
    if needed == "shan" && crate::skills::has(st, f.actor, "shifei") {
        options.insert(
            options.len().saturating_sub(1),
            PendingOption {
                id: "skill:shifei".into(),
                label: "饰非：令当前回合角色摸一张，视为出【闪】".into(),
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
    // 酒池：黑桃手牌当【酒】自救。
    if f.victim == seat {
        for id in &st.seats[seat].hand {
            if crate::skills::conversion_for(st, seat, id, "jiu").is_some() {
                options.push(PendingOption {
                    id: format!("respond:{id}"),
                    label: format!("酒池：将【{}】当【酒】自救", turn::card_label(st, id)),
                });
            }
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
    // 蛊惑：任意手牌当【桃】（每回合限一次）。
    if f.victim == seat && crate::resolve_wind::guhuo_available(st, seat) {
        for id in &st.seats[seat].hand {
            options.push(PendingOption {
                id: format!("skill:guhuo:{id}"),
                label: format!("蛊惑：将【{}】当【桃】使用", turn::card_label(st, id)),
            });
        }
    }
    // 伏枥（限定，廖化）：濒死时可回复至全场势力数并翻面。
    if f.victim == seat
        && crate::skills::has(st, seat, "fuli")
        && !st.seats[seat].flags.contains_key("fuli_used")
    {
        options.push(PendingOption {
            id: "skill:fuli".into(),
            label: "伏枥：回复至全场势力数体力并翻面（限定技）".into(),
        });
    }
    // 涅槃（限定）：濒死时可发动（仅濒死者本人）。
    if f.victim == seat
        && crate::skills::has(st, seat, "niepan")
        && !st.seats[seat].flags.contains_key("niepan_used")
    {
        options.push(PendingOption {
            id: "skill:niepan".into(),
            label: "涅槃：弃置区域内所有牌，复原武将牌，摸 3 张并回复至 3 点体力".into(),
        });
    }
    // 完杀：贾诩回合内，仅濒死者本人可使用【桃】。
    if crate::skills::wansha_blocks(st, seat, f.victim) {
        options = vec![pending_pass()];
    } else if crate::skills::chanyuan_locked(st, seat) || crate::skills::hand_sealed(st, seat) {
        // 缠怨/义绝：不能使用或打出手牌。
        options = vec![pending_pass()];
    } else {
        options.push(pending_pass());
    }
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
    let suit = crate::skills::effective_suit(st, f.target, &f.revealed);
    let label = turn::card_label(st, &f.revealed);
    let revealed_name = turn::card_label(st, &f.revealed);
    let target_name = st.seats[f.target].general_name.clone();
    st.push_log(None, format!("火攻：{target_name} 亮出【{revealed_name}】"));
    let mut options: Vec<PendingOption> = st.seats[f.user]
        .hand
        .iter()
        .filter(|id| crate::skills::effective_suit(st, f.user, id) == suit)
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
                skill: Some(rewrite_skill(st, seat).into()),
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
            // 洛神/双雄：判定牌由技能获得；屯田非红桃：留在处理区待置为「田」；其余弃置。
            let tuotian_keep = f.reason == "tuotian" && suit != Some(Suit::Heart);
            if (f.reason == "luoshen" && black) || f.reason == "shuangxiong" {
                leave_processing(st, &judge_id);
                st.seats[f.owner].hand.push(judge_id.clone());
                if crate::skills::is_jiexian(st, f.owner) && f.reason == "luoshen" {
                    // 界洛神：获得的牌本回合不计入手牌上限（不立即弃牌）。
                    st.bump_counter("luoshen_free");
                } else {
                    let over = st.seats[f.owner]
                        .hand
                        .len()
                        .saturating_sub(st.config.max_hand_cards as usize);
                    if over > 0 {
                        push_discard(st, f.owner, over, "hand_cap");
                    }
                }
            } else if tuotian_keep {
                leave_processing(st, &judge_id);
            } else {
                discard_from_processing(st, &judge_id);
            }
            let pid = st.seats[f.owner].player_id.clone();
            crate::skills::songwei_on_judge(st, f.owner, &judge_id);
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
                    if crate::skills::is_jiexian(st, user) {
                        // 界铁骑：目标须弃同花色牌，否则不能使用【闪】。
                        let suit = suit.unwrap_or(Suit::Spade);
                        crate::resolve_jiexian::push_tieqi(
                            st,
                            user,
                            target,
                            &card,
                            need,
                            ignore_armor,
                            suit,
                        );
                    } else {
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
                "shuangxiong" => {
                    let color = if black { 2 } else { 1 };
                    st.set_counter("shuangxiong_color", color);
                    st.push_log(
                        Some(&pid),
                        format!(
                            "双雄：判定【{label}】{}，获得该牌（本回合异色手牌当【决斗】）",
                            if black { "黑色" } else { "红色" }
                        ),
                    );
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
                "huituo" => {
                    let target = f.data["target"].as_u64().unwrap_or(f.owner as u64) as usize;
                    let amount = f.data["amount"].as_u64().unwrap_or(1) as u32;
                    let red = matches!(suit, Some(Suit::Heart) | Some(Suit::Diamond));
                    if red {
                        if st.seats[target].hp < st.seats[target].max_hp {
                            st.seats[target].hp += 1;
                        }
                        let hp = st.seats[target].hp;
                        st.push_log(
                            Some(&pid),
                            format!(
                                "恢拓：判定【{label}】红色，{} 回复 1 点体力（{hp}）",
                                st.seats[target].general_name
                            ),
                        );
                    } else {
                        let mut ev = Vec::new();
                        let drawn = turn::draw_cards(st, target, amount, &mut ev);
                        st.push_log(
                            Some(&pid),
                            format!(
                                "恢拓：判定【{label}】黑色，{} 摸 {drawn} 张",
                                st.seats[target].general_name
                            ),
                        );
                    }
                }
                "ganglie" => {
                    let victim = f.data["victim"].as_u64().unwrap_or(f.owner as u64) as usize;
                    let source = f.data["source"].as_u64().unwrap_or(0) as usize;
                    if crate::skills::is_jiexian(st, victim) {
                        // 界刚烈：红色 → 对来源造成 1 点伤害；黑色 → 弃来源一张牌。
                        let red = matches!(suit, Some(Suit::Heart) | Some(Suit::Diamond));
                        if red {
                            st.push_log(
                                Some(&pid),
                                format!("刚烈：判定【{label}】红色，对伤害来源造成 1 点伤害"),
                            );
                            apply_damage(st, source, 1, Some(victim), DamageType::Normal, None);
                        } else {
                            let card = crate::resolve_jiexian::discard_one_from(st, source);
                            let what = card
                                .map(|c| turn::card_label(st, &c))
                                .unwrap_or_else(|| "（无牌可弃）".to_string());
                            st.push_log(
                                Some(&pid),
                                format!("刚烈：判定【{label}】黑色，弃置来源的{what}"),
                            );
                        }
                    } else if suit == Some(Suit::Heart) {
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
                "leiji" => {
                    let user = f.data["user"].as_u64().unwrap_or(f.owner as u64) as usize;
                    let target = f.data["target"].as_u64().unwrap_or(0) as usize;
                    if suit == Some(Suit::Spade) {
                        st.push_log(
                            Some(&pid),
                            format!("雷击：判定【{label}】黑桃，造成 2 点雷电伤害"),
                        );
                        apply_damage(st, target, 2, Some(user), DamageType::Thunder, None);
                    } else if suit == Some(Suit::Club) {
                        st.seats[user].hp += 1;
                        let hp = st.seats[user].hp;
                        st.push_log(
                            Some(&pid),
                            format!("雷击：判定【{label}】梅花，回复 1 点体力（{hp}）并造成 1 点雷电伤害"),
                        );
                        apply_damage(st, target, 1, Some(user), DamageType::Thunder, None);
                    } else {
                        st.push_log(
                            Some(&pid),
                            format!("雷击：判定【{label}】非黑桃/梅花，未生效"),
                        );
                    }
                }
                "tuotian" => {
                    if suit != Some(Suit::Heart) {
                        st.seats[f.owner]
                            .marks
                            .entry("tian".into())
                            .or_default()
                            .push(judge_id.clone());
                        st.push_log(
                            Some(&pid),
                            format!("屯田：判定【{label}】非红桃，置于武将牌上为「田」"),
                        );
                    } else {
                        st.push_log(Some(&pid), format!("屯田：判定【{label}】红桃，未生效"));
                    }
                }
                "beige" => {
                    let target = f.data["target"].as_u64().unwrap_or(0) as usize;
                    let source = f.data["source"].as_u64().map(|v| v as usize);
                    match suit {
                        Some(Suit::Heart) => {
                            st.seats[target].hp =
                                (st.seats[target].hp + 1).min(st.seats[target].max_hp);
                            st.push_log(
                                Some(&pid),
                                format!(
                                    "悲歌：判定【{label}】红桃，{} 回复 1 点体力",
                                    st.seats[target].general_name
                                ),
                            );
                        }
                        Some(Suit::Diamond) => {
                            let mut ev = Vec::new();
                            let drawn = turn::draw_cards(st, target, 2, &mut ev);
                            st.push_log(
                                Some(&pid),
                                format!(
                                    "悲歌：判定【{label}】方片，{} 摸 {drawn} 张",
                                    st.seats[target].general_name
                                ),
                            );
                        }
                        Some(Suit::Club) => {
                            if let Some(src) = source {
                                push_discard(st, src, 2, "beige");
                                st.push_log(
                                    Some(&pid),
                                    format!(
                                        "悲歌：判定【{label}】梅花，{} 弃 2 张",
                                        st.seats[src].general_name
                                    ),
                                );
                            }
                        }
                        Some(Suit::Spade) => {
                            if let Some(src) = source {
                                st.seats[src].flipped = !st.seats[src].flipped;
                                st.push_log(
                                    Some(&pid),
                                    format!(
                                        "悲歌：判定【{label}】黑桃，{} 翻面",
                                        st.seats[src].general_name
                                    ),
                                );
                            }
                        }
                        None => {}
                    }
                }
                "baonue" => {
                    let lord = f.data["lord"].as_u64().unwrap_or(f.owner as u64) as usize;
                    if suit == Some(Suit::Spade) {
                        st.seats[lord].hp += 1;
                        st.push_log(
                            Some(&pid),
                            format!("暴虐：判定【{label}】黑桃，回复 1 点体力"),
                        );
                    } else {
                        st.push_log(Some(&pid), format!("暴虐：判定【{label}】非黑桃，未生效"));
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
                skill: Some(rewrite_skill(st, seat).into()),
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
            let has_cards = !st.seats[*i].hand.is_empty()
                || (crate::skills::is_jiexian(st, *i)
                    && crate::skills::has(st, *i, "guicai")
                    && st.seats[*i].equips.values().any(|c| c.is_some()));
            st.seats.get(*i).is_some_and(|s| s.alive)
                && has_cards
                && (crate::skills::has(st, *i, "guicai") || crate::skills::has(st, *i, "guidao"))
        })
        .collect()
}

/// 改判持有者的技能 code（鬼才 / 鬼道）。
fn rewrite_skill(st: &SgsState, seat: usize) -> &'static str {
    if crate::skills::has(st, seat, "guicai") {
        "guicai"
    } else {
        "guidao"
    }
}

/// 改判可选牌（鬼才：任意一张手牌；鬼道：黑色手牌）+ `pass`。
fn rewrite_options(st: &SgsState, seat: usize) -> Vec<PendingOption> {
    let black_only =
        !crate::skills::has(st, seat, "guicai") && crate::skills::has(st, seat, "guidao");
    let mut ids: Vec<String> = st.seats[seat]
        .hand
        .iter()
        .filter(|id| {
            !black_only
                || st
                    .cards
                    .get(*id)
                    .is_some_and(|c| crate::skills::is_black(c.suit))
        })
        .cloned()
        .collect();
    // 界鬼才：装备区的牌也可用于改判。
    if crate::skills::is_jiexian(st, seat) && crate::skills::has(st, seat, "guicai") {
        for slot in EquipSlot::ALL {
            if let Some(id) = st.seats[seat].equips.get(&slot).cloned().flatten() {
                if !black_only
                    || st
                        .cards
                        .get(&id)
                        .is_some_and(|c| crate::skills::is_black(c.suit))
                {
                    ids.push(id);
                }
            }
        }
    }
    let mut options: Vec<PendingOption> = ids
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
    if st.seats[seat].hand.iter().any(|c| c == card_id) {
        turn::take_card(st, seat, card_id)?;
    } else {
        // 界鬼才：从装备区取牌改判。
        let mut used = false;
        for slot in EquipSlot::ALL {
            if st.seats[seat]
                .equips
                .get(&slot)
                .cloned()
                .flatten()
                .as_deref()
                == Some(card_id)
            {
                st.seats[seat].equips.insert(slot, None);
                crate::skills::xiaoji_on_lose_equip(st, seat);
                used = true;
                break;
            }
        }
        if !used {
            return Err(illegal("手牌/装备区中没有该牌"));
        }
    }
    st.deck.processing.push(card_id.to_string());
    let before = judge_card
        .as_deref()
        .map(|id| turn::card_label(st, id))
        .unwrap_or_default();
    let skill_name = if crate::skills::has(st, seat, "guicai") {
        "鬼才"
    } else {
        "鬼道"
    };
    st.push_log(
        Some(&pending.actor),
        format!(
            "{skill_name}：将判定牌 {before} 改为 {}",
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
            Some("kuanggu") => crate::resolve_wind::act_kuanggu(st, action)?,
            Some("qimou") => crate::resolve_wind::act_qimou(st, action)?,
            Some("jushou") => crate::resolve_wind::act_jushou(st, action)?,
            Some("jiewei") => {
                if pending.data["stage"] == "discard" {
                    crate::resolve_wind::act_jiewei_discard(st, action)?
                } else {
                    crate::resolve_wind::act_jiewei(st, action)?
                }
            }
            Some("fenji") => crate::resolve_wind::act_fenji(st, action)?,
            Some("shensu") => crate::resolve_wind::act_shensu(st, action)?,
            Some("tianxiang") => crate::resolve_wind::act_tianxiang(st, action)?,
            Some("guhuo") => crate::resolve_wind::act_guhuo(st, action)?,
            Some("pindian") => crate::resolve_fire::act_pindian(st, action)?,
            Some("qiangxi") => crate::resolve_fire::act_qiangxi(st, action)?,
            Some("jiandu") => crate::resolve_fire::act_jiandu(st, action)?,
            Some("shuangxiong") => crate::resolve_fire::act_shuangxiong(st, action)?,
            Some("xingshang") => crate::resolve_forest::act_xingshang(st, action)?,
            Some("yinghun") => crate::resolve_forest::act_yinghun(st, action)?,
            Some("bengkuai") => crate::resolve_forest::act_bengkuai(st, action)?,
            Some("liewen") => crate::resolve_forest::act_liewen(st, action)?,
            Some("zaizhi") => crate::resolve_forest::act_zaizhi(st, action)?,
            Some("luanwu") => crate::resolve_forest::act_luanwu(st, action)?,
            Some("haoshi_offer") => crate::resolve_forest::act_haoshi_offer(st, action)?,
            Some("awaken") => crate::resolve_mountain::act_awaken(st, action)?,
            Some("qiaobian") => crate::resolve_mountain::act_qiaobian(st, action)?,
            Some("tiaoxin") => crate::resolve_mountain::act_tiaoxin(st, action)?,
            Some("xiangle") => crate::resolve_mountain::act_xiangle(st, action)?,
            Some("zhiba") => crate::resolve_mountain::act_zhiba(st, action)?,
            Some("guzheng") => crate::resolve_mountain::act_guzheng(st, action)?,
            Some("huashen") => crate::resolve_mountain::act_huashen(st, action)?,
            Some("beige") => crate::resolve_mountain::act_beige(st, action)?,
            Some("qizhi") => crate::resolve_sp::act_qizhi(st, action)?,
            Some("jinzou") => crate::resolve_sp::act_jinzou(st, action)?,
            Some("juzhan") => crate::resolve_sp::act_juzhan(st, action)?,
            Some("feijun") => crate::resolve_sp::act_feijun(st, action)?,
            Some("jianxiang") => crate::resolve_sp::act_jianxiang(st, action)?,
            Some("shenshi") => crate::resolve_sp::act_shenshi(st, action)?,
            Some("chenglue") => crate::resolve_sp::act_chenglue(st, action)?,
            Some("yili") => crate::resolve_sp::act_yili(st, action)?,
            Some("zhenglun") => crate::resolve_sp::act_zhenglun(st, action)?,
            Some("kuizhu") => crate::resolve_sp::act_kuizhu(st, action)?,
            Some("lijun") => crate::resolve_sp::act_lijun(st, action)?,
            Some("mingren") => crate::resolve_sp2::act_mingren(st, action)?,
            Some("zhenliang") => {
                let top_frame = matches!(top(st), Some(Frame::ZhenLiangYin(_)));
                if top_frame {
                    crate::resolve_sp2::act_zhenliang_yin(st, action)?
                } else {
                    crate::resolve_sp2::act_zhenliang(st, action)?
                }
            }
            Some("zuilun") => crate::resolve_sp2::act_zuilun(st, action)?,
            Some("liangyin") => crate::resolve_sp2::act_liangyin(st, action)?,
            Some("jueyan") => crate::resolve_sp2::act_jueyan(st, action)?,
            Some("congjian") => crate::resolve_sp2::act_congjian(st, action)?,
            Some("luoying") => crate::resolve_yijiang::act_luoying(st, action)?,
            Some("jiushi") => crate::resolve_yijiang::act_jiushi(st, action)?,
            Some("mingce") => crate::resolve_yijiang::act_mingce(st, action)?,
            Some("sanyao") => crate::resolve_yijiang::act_sanyao(st, action)?,
            Some("zhiman") => crate::resolve_yijiang::act_zhiman(st, action)?,
            Some("pojun") => crate::resolve_yijiang::act_pojun(st, action)?,
            Some("zhenjun") => crate::resolve_yijiang::act_zhenjun(st, action)?,
            Some("yijue") => crate::resolve_jiexian::act_yijue(st, action)?,
            Some("rende_basic") => crate::resolve_jiexian::act_rende_basic(st, action)?,
            Some("yajiao") => crate::resolve_jiexian::act_yajiao(st, action)?,
            Some("tieqi") => crate::resolve_jiexian::act_tieqi(st, action)?,
            Some("fenwei") => crate::resolve_jiexian::act_fenwei(st, action)?,
            Some("fanjian_jie") => crate::resolve_jiexian::act_jie_fanjian(st, action)?,
            Some("guose") => crate::resolve_jiexian::act_guose(st, action)?,
            Some("qinxue") => crate::resolve_jiexian::act_qinxue(st, action)?,
            Some("gongxin") => crate::resolve_jiexian::act_gongxin(st, action)?,
            Some("lianying") => crate::resolve_jiexian::act_lianying(st, action)?,
            Some("qingjian") => crate::resolve_jiexian::act_qingjian(st, action)?,
            Some("xunxun") => crate::resolve_jiexian::act_xunxun(st, action)?,
            Some("wangxi") => crate::resolve_jiexian::act_wangxi(st, action)?,
            Some("chuli") => crate::resolve_jiexian::act_chuli(st, action)?,
            Some("liyu") => crate::resolve_jiexian::act_liyu(st, action)?,
            Some("jiangchi") => crate::resolve_yijiang2::act_jiangchi(st, action)?,
            Some("qice") => crate::resolve_yijiang2::act_qice(st, action)?,
            Some("qianxi") => crate::resolve_yijiang2::act_qianxi(st, action)?,
            Some("anxu") => crate::resolve_yijiang2::act_anxu(st, action)?,
            Some("zhuiyi") => crate::resolve_yijiang2::act_zhuiyi(st, action)?,
            Some("lihuo") => crate::resolve_yijiang2::act_lihuo(st, action)?,
            Some("chunlao") => crate::resolve_yijiang2::act_chunlao(st, action)?,
            Some("gongqi") => crate::resolve_yijiang2::act_gongqi(st, action)?,
            Some("jiefan") => crate::resolve_yijiang2::act_jiefan(st, action)?,
            Some("qingce") => crate::resolve_sp2::act_qingce(st, action)?,
            Some("chengxiang") => crate::resolve_yijiang3::act_chengxiang(st, action)?,
            Some("renxin") => crate::resolve_yijiang3::act_renxin(st, action)?,
            Some("zili") => crate::resolve_yijiang3::act_zili(st, action)?,
            Some("paiyi") => crate::resolve_yijiang3::act_paiyi(st, action)?,
            Some("zishou") => crate::resolve_yijiang3::act_zishou(st, action)?,
            Some("junxing") => crate::resolve_yijiang3::act_junxing(st, action)?,
            Some("yuce") => crate::resolve_yijiang3::act_yuce(st, action)?,
            Some("longyin") => crate::resolve_yijiang3::act_longyin(st, action)?,
            Some("qiaoshuo") => crate::resolve_yijiang3::act_qiaoshuo(st, action)?,
            Some("xiansi") => crate::resolve_yijiang4::act_xiansi(st, action)?,
            Some("duodao") => crate::resolve_yijiang4::act_duodao(st, action)?,
            Some("zongxuan") => crate::resolve_yijiang4::act_zongxuan(st, action)?,
            Some("zhiyan") => crate::resolve_yijiang4::act_zhiyan(st, action)?,
            Some("danshou") => crate::resolve_yijiang4::act_danshou(st, action)?,
            Some("zhuikong") => crate::resolve_yijiang4::act_zhuikong(st, action)?,
            Some("qiuyuan") => crate::resolve_yijiang4::act_qiuyuan(st, action)?,
            Some("mieji") => crate::resolve_yijiang4::act_mieji(st, action)?,
            Some("juece") => crate::resolve_yijiang4::act_juece(st, action)?,
            Some("fencheng") => crate::resolve_yijiang4::act_fencheng(st, action)?,
            Some("enyuan") => crate::resolve_yijiang4::act_enyuan(st, action)?,
            Some("xuanhuo") => crate::resolve_yijiang4::act_xuanhuo(st, action)?,
            Some("jujian") => crate::resolve_yijiang4::act_jujian(st, action)?,
            Some("zhenlie") => crate::resolve_yijiang5::act_zhenlie(st, action)?,
            Some("miji") => crate::resolve_yijiang5::act_miji(st, action)?,
            Some("sidi") => crate::resolve_yijiang5::act_sidi(st, action)?,
            Some("shenduan") => crate::resolve_yijiang5::act_shenduan(st, action)?,
            Some("yonglue") => crate::resolve_yijiang5::act_yonglue(st, action)?,
            Some("pindi") => crate::resolve_yijiang5::act_pindi(st, action)?,
            Some("benxi") => crate::resolve_yijiang5::act_benxi(st, action)?,
            Some("zhongyong") => crate::resolve_yijiang5::act_zhongyong(st, action)?,
            Some("qiangshi") => crate::resolve_yijiang5::act_qiangshi(st, action)?,
            Some("xiantu") => crate::resolve_yijiang5::act_xiantu(st, action)?,
            Some("zanhui") => crate::resolve_yijiang6::act_zanhui(st, action)?,
            Some("jiaojin") => crate::resolve_yijiang6::act_jiaojin(st, action)?,
            Some("fenli") => crate::resolve_yijiang6::act_fenli(st, action)?,
            Some("pingkou") => crate::resolve_yijiang6::act_pingkou(st, action)?,
            Some("shenxing") => crate::resolve_yijiang6::act_shenxing(st, action)?,
            Some("bingyi") => crate::resolve_yijiang6::act_bingyi(st, action)?,
            Some("qieting") => crate::resolve_yijiang6::act_qieting(st, action)?,
            Some("xianzhou") => crate::resolve_yijiang6::act_xianzhou(st, action)?,
            Some("huituo") => crate::resolve_yijiang6::act_huituo(st, action)?,
            Some("mingjian") => crate::resolve_yijiang6::act_mingjian(st, action)?,
            Some("qingxi") => crate::resolve_yijiang6::act_qingxi(st, action)?,
            Some("zuoding") => crate::resolve_yijiang6::act_zuoding(st, action)?,
            Some("qiaoshi") => crate::resolve_yijiang7::act_qiaoshi(st, action)?,
            Some("yanyu") => crate::resolve_yijiang7::act_yanyu(st, action)?,
            Some("wurong") => crate::resolve_yijiang7::act_wurong(st, action)?,
            Some("yanzhu") => crate::resolve_yijiang7::act_yanzhu(st, action)?,
            Some("xingxue") => crate::resolve_yijiang7::act_xingxue(st, action)?,
            Some("zhaofu") => crate::resolve_yijiang7::act_zhaofu(st, action)?,
            Some("anguo") => crate::resolve_yijiang7::act_anguo(st, action)?,
            Some("yaoming") => crate::resolve_yijiang7::act_yaoming(st, action)?,
            Some("huaiyi") => crate::resolve_yijiang7::act_huaiyi(st, action)?,
            Some("jigong") => crate::resolve_yijiang7::act_jigong(st, action)?,
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
    let seat = st.seat_index(&pid).ok_or_else(|| internal("玩家不在座"))?;
    // 蛊惑：任意手牌当【无懈可击】。
    if let Some(card_id) = action.strip_prefix("skill:guhuo:") {
        if !crate::resolve_wind::guhuo_available(st, seat) {
            return Err(illegal("蛊惑本回合已使用过"));
        }
        if !st.seats[seat].hand.iter().any(|c| c == card_id) {
            return Err(illegal("手牌中没有该牌"));
        }
        crate::resolve_wind::start_guhuo_response(st, seat, "wuxie", "wuxie_keji", card_id, None)?;
        return Ok("蛊惑：请等待其他角色质疑".to_string());
    }
    // 解围：装备区的牌当【无懈可击】。
    if let Some(card_id) = action.strip_prefix("skill:jiewei:") {
        if !crate::skills::has(st, seat, "jiewei") {
            return Err(illegal("没有该技能"));
        }
        let mut used = false;
        for card in st.seats[seat].equips.values_mut() {
            if card.as_deref() == Some(card_id) {
                *card = None;
                used = true;
                break;
            }
        }
        if !used {
            return Err(illegal("装备区没有该牌"));
        }
        turn::to_discard_card(st, card_id);
        f.count += 1;
        f.cursor = 0;
        f.passed = 0;
        st.push_log(
            Some(&pid),
            format!(
                "解围：将装备【{}】当【无懈可击】（层数 {}）",
                turn::card_label(st, card_id),
                f.count
            ),
        );
        let count = f.count;
        replace_top(st, Frame::Wuxie(f));
        return Ok(format!("已用装备当无懈可击（第 {count} 层）"));
    }
    let card_id = action
        .strip_prefix("respond:")
        .ok_or_else(|| illegal("需要打出无懈可击或选择不出"))?;
    let actual = turn::card_def(st, card_id).map(|d| d.code);
    let conversion = if actual == Some("wuxie_keji") {
        None
    } else {
        Some(
            crate::skills::conversion_for(st, seat, card_id, "wuxie_keji")
                .ok_or_else(|| illegal("只能打出【无懈可击】"))?,
        )
    };
    turn::take_card(st, seat, card_id)?;
    turn::to_discard_card(st, card_id);
    f.count += 1;
    f.cursor = 0;
    f.passed = 0;
    let label = match conversion {
        Some((_, name)) => format!(
            "{name}：将【{}】当【无懈可击】（层数 {}）",
            turn::card_label(st, card_id),
            f.count
        ),
        None => format!(
            "打出【{}】（无懈层数 {}）",
            turn::card_label(st, card_id),
            f.count
        ),
    };
    st.push_log(Some(&pid), label);
    let count = f.count;
    replace_top(st, Frame::Wuxie(f));
    // 涯角（界赵云）：回合外打出【无懈可击】后展示牌堆顶。
    crate::resolve_jiexian::yajiao_trigger(st, seat, card_id);
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
        // 烈弓：目标手牌数 ≥ 你的体力值或 ≤ 你的攻击范围 → 不能使用【闪】（自动发动）。
        let mut no_shan = crate::skills::liegong_blocks(st, f.user, target);
        if f.no_respond {
            no_shan = true;
        }
        // 陷阵（拼点赢）：无视被拼点者防具。
        let xianzhen_target = crate::resolve_yijiang::xianzhen_win_target(st) == Some(target)
            && crate::skills::has(st, f.user, "xianzhen");
        let benxi_armor = st.counter("benxi_armor") > 0;
        if benxi_armor {
            st.set_counter("benxi_armor", 0);
        }
        let ignore_armor = crate::skills::has_equipment_skill(st, f.user, "qinggang_jian")
            || xianzhen_target
            || benxi_armor;
        if no_shan && crate::skills::has(st, f.user, "liegong") {
            let user_pid = st.seats[f.user].player_id.clone();
            st.push_log(Some(&user_pid), "烈弓：目标不能使用【闪】");
        }
        // 智迟（陈宫，锁定）：回合外受伤后，本回合【杀】对其无效。
        if crate::resolve_yijiang::zhichi_active(st, target) {
            crate::resolve::discard_from_processing(st, &f.card);
            pop(st);
            st.push_log(
                None,
                format!("智迟：【杀】对 {} 无效", st.seats[target].general_name),
            );
            return Ok("智迟：此【杀】无效".to_string());
        }
        // 父荫（诸葛瞻，锁定）：每回合首次成为【杀】目标且对方手牌 ≥ 你 → 此牌无效。
        if crate::resolve_sp2::fuyin_take(st, target, f.user) {
            crate::resolve::discard_from_processing(st, &f.card);
            pop(st);
            return Ok("父荫：此【杀】对目标无效".to_string());
        }
        // 无双：吕布使用【杀】时，目标需连续打出两张【闪】。
        let mut need = if crate::skills::has(st, f.user, "wushuang") {
            2
        } else {
            1
        };
        if crate::skills::roulin_need(st, f.user, target) {
            need = need.max(2);
        }
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
        // 享乐：成为【杀】目标后，使用者需弃基本牌，否则杀无效。
        if crate::skills::has(st, target, "xiangle") {
            push(
                st,
                Frame::Xiangle(crate::resolve_mountain::XiangleFrame {
                    user: f.user,
                    target,
                    card: f.card.clone(),
                    need,
                    no_shan,
                    ignore_armor,
                    damage_type: f.damage_type,
                }),
            );
            return Ok("享乐：请使用者选择是否弃置基本牌".to_string());
        }
        // 激昂：使用/成为红色【杀】目标后摸 1。
        if let Some(inst) = st.cards.get(&f.card) {
            if crate::skills::is_red(inst.suit) {
                crate::skills::jiang_trigger(st, f.user);
                crate::skills::jiang_trigger(st, target);
            }
        }
        // 鞬出：使用【杀】指定目标后，可弃其一张牌（弃装备→不能闪；非装备→其获得此杀）。
        if crate::skills::has(st, f.user, "jiandu")
            && !f.card.is_empty()
            && crate::resolve_fire::target_has_cards(st, target)
        {
            push(
                st,
                Frame::Jiandu(crate::resolve_fire::JianduFrame {
                    user: f.user,
                    target,
                    card: f.card.clone(),
                    need,
                    no_shan,
                    ignore_armor: crate::skills::has_equipment_skill(st, f.user, "qinggang_jian"),
                    damage_type: f.damage_type,
                }),
            );
            return Ok(format!(
                "鞬出：请选择 {} 的一张牌",
                st.seats[target].general_name
            ));
        }
        // 谮毁（孙鲁班）：使用【杀】指定唯一目标时，可令另一名角色代用或成为额外目标。
        if crate::skills::has(st, f.user, "zanhui")
            && st.counter("zanhui_used") == 0
            && st
                .alive_indices()
                .into_iter()
                .any(|i| i != f.user && i != target)
        {
            push(
                st,
                Frame::ZanHui(crate::resolve_yijiang6::ZanHuiFrame {
                    owner: f.user,
                    user: f.user,
                    target,
                    card: f.card.clone(),
                    need,
                    no_shan,
                    ignore_armor,
                    damage_type: f.damage_type,
                    stage: crate::resolve_yijiang6::ZanHuiStage::Helper,
                    helper: None,
                }),
            );
            return Ok("谮毁：请选择协助角色".to_string());
        }
        if target != f.user {
            st.bump_counter("targeted_others");
        }
        // 佐定（钟繇）：其他角色出牌阶段使用黑桃牌指定目标后。
        if !f.card.is_empty() {
            crate::resolve_yijiang6::zuoding_trigger(st, f.user, &f.card, target);
        }
        // 贞烈（王异）：成为【杀】目标后，可失去 1 点体力令此【杀】对其无效。
        if crate::skills::has(st, target, "zhenlie")
            && !crate::skills::zhenlie_skip(st, &f.card, target)
        {
            push(
                st,
                Frame::ZhenLie(crate::resolve_yijiang5::ZhenLieFrame {
                    owner: target,
                    user: f.user,
                    card: f.card.clone(),
                    kind: crate::resolve_yijiang5::ZhenLieKind::Sha,
                    need,
                    no_shan,
                    ignore_armor,
                    damage_type: f.damage_type,
                }),
            );
            return Ok("贞烈：请选择是否令此【杀】无效".to_string());
        }
        // 求援（伏皇后）：成为【杀】目标时，可令另一名角色交【闪】或成为额外目标。
        if crate::skills::has(st, target, "qiuyuan")
            && st
                .alive_indices()
                .into_iter()
                .any(|v| v != target && v != f.user)
        {
            push(
                st,
                Frame::QiuYuan(crate::resolve_yijiang4::QiuYuanFrame {
                    user: f.user,
                    target,
                    card: f.card.clone(),
                    need,
                    no_shan,
                    ignore_armor,
                    damage_type: f.damage_type,
                    stage: crate::resolve_yijiang4::QiuYuanStage::Helper,
                    helper: None,
                }),
            );
            return Ok("求援：请选择协助角色".to_string());
        }
        crate::resolve_skill::push_sha_target_response(
            st,
            f.user,
            target,
            &f.card,
            need,
            no_shan,
            ignore_armor,
            f.damage_type,
        );
        // 拒战（严颜，转换技）：阳面（成为杀目标后）/ 阴面（使用杀指定目标后），
        // 压在用【闪】响应之上，先结算拒战再进入响应。
        crate::resolve_sp::push_juzhan_yang(st, target, f.user);
        crate::resolve_sp::push_juzhan_yin(st, f.user, target);
        // 荐降（蒯越蒯良）：成为其他角色使用牌的目标后，可令手牌最少者摸 1。
        if crate::skills::has(st, target, "jianxiang") && target != f.user {
            push(
                st,
                Frame::Jianxiang(crate::resolve_sp::JianxiangFrame {
                    actor: target,
                    source: f.user,
                }),
            );
        }
        // 破军（徐盛）：使用【杀】指定目标后，可将目标至多 X 张牌移出游戏直到回合结束。
        crate::resolve_yijiang::push_pojun(st, f.user, target, &f.card);
        // 立军（孙亮主公技）：其他吴势力使用【杀】后可交给孙亮。
        crate::resolve_sp::lijun_trigger(st, f.user, &f.card);
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
            if f.code == "juedou" {
                crate::skills::jiang_trigger(st, f.user);
                crate::skills::jiang_trigger(st, target);
            }
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
    if let Some(card_id) = action.strip_prefix("skill:guhuo:") {
        if !crate::resolve_wind::guhuo_available(st, actor) {
            return Err(illegal("蛊惑本回合已使用过"));
        }
        if !st.seats[actor].hand.iter().any(|c| c == card_id) {
            return Err(illegal("手牌中没有该牌"));
        }
        let needed = match f.kind {
            RespondKind::Shan => "shan",
            RespondKind::Nanman | RespondKind::Duel | RespondKind::Jiedao => "sha",
        };
        crate::resolve_wind::start_guhuo_response(
            st,
            actor,
            "respond",
            needed,
            card_id,
            Some(f.clone()),
        )?;
        return Ok("蛊惑：请等待其他角色质疑".to_string());
    }
    if action == "skill:bagua" {
        if f.kind != RespondKind::Shan {
            return Err(illegal("八卦阵只能用于需要【闪】时"));
        }
        if f.bagua_used {
            return Err(illegal("本次响应已经使用过【八卦阵】"));
        }
        if f.ignore_armor
            || !(st
                .equip_def(actor, EquipSlot::Armor)
                .is_some_and(|d| d.code == "bagua_zhen")
                || crate::skills::virtual_equip(st, actor).is_some_and(|v| v == "bagua_zhen"))
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
    if action == "skill:shifei" {
        if !pending.options.iter().any(|o| o.id == "skill:shifei") {
            return Err(illegal("饰非不可用"));
        }
        let cur = st.turn;
        let mut ev = Vec::new();
        let drawn = turn::draw_cards(st, cur, 1, &mut ev);
        let max_hand = st
            .alive_indices()
            .into_iter()
            .map(|i| st.seats[i].hand.len())
            .max()
            .unwrap_or(0);
        let max_seats: Vec<usize> = st
            .alive_indices()
            .into_iter()
            .filter(|i| st.seats[*i].hand.len() == max_hand)
            .collect();
        let unique_cur = max_seats.len() == 1 && max_seats[0] == cur;
        let mut extra = String::new();
        if !unique_cur && max_hand > 0 {
            let victim = max_seats[0];
            let n = st.seats[victim].hand.len();
            let idx = st.next_index(n);
            let card = st.seats[victim].hand.remove(idx);
            let label = turn::card_label(st, &card);
            turn::discard_by(st, Some(victim), &card);
            extra = format!("，弃置 {} 的【{label}】", st.seats[victim].general_name);
        }
        st.push_log(
            Some(&st.seats[f.actor].player_id.clone()),
            format!("饰非：当前回合角色摸 {drawn} 张{extra}，视为出【闪】"),
        );
        finish_respond_dodge(st, &f);
        return Ok("饰非：视为打出【闪】".to_string());
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
            return Err(illegal("两张手牌当【杀】只能用于打出【杀】"));
        }
        let zhangba = crate::skills::has_equipment_skill(st, actor, "zhangba_shemao");
        let fuhen = crate::skills::has(st, actor, "fuhen");
        if !zhangba && !fuhen {
            return Err(illegal("没有可将两张手牌当【杀】的技能/装备"));
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
        let (skill, name) = if zhangba {
            ("zhangba", "丈八蛇矛")
        } else {
            ("fuhen", "父魂")
        };
        return finish_respond_play(st, &f, c1, actor, Some((skill, name)));
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
    if let Some(src) = source {
        // 界裸衣：直到下回合开始，来源的【杀】/【决斗】伤害 +1。
        if crate::skills::has(st, src, "luoyi") && st.seats[src].flags.contains_key("luoyi_bonus") {
            return 1;
        }
    }
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
            // 界咆哮：被抵消后本回合下一次【杀】伤害 +1。
            if f.is_sha
                && crate::skills::has(st, f.source, "paoxiao")
                && st.counter("paoxiao_bonus") > 0
            {
                amount += 1;
                st.set_counter("paoxiao_bonus", 0);
            }
            // 义绝（界关羽）：本回合对其使用的红桃【杀】伤害 +1。
            if f.is_sha
                && crate::skills::has(st, f.source, "yijue")
                && st.counter(&format!("yijue_heart:{actor}")) > 0
                && st.cards.get(&card).is_some_and(|c| c.suit == Suit::Heart)
                && !card.is_empty()
            {
                amount += 1;
            }
            // 古锭刀：目标无手牌时【杀】伤害 +1（物理【杀】）。
            if f.is_sha
                && crate::skills::has_equipment_skill(st, f.source, "guding_dao")
                && st.seats[actor].hand.is_empty()
            {
                amount += 1;
            }
            // 暗箭（潘璋马忠，锁定）：你不在其攻击范围内 → 伤害 +1。
            if f.is_sha {
                amount += crate::skills::anjian_bonus(st, f.source, actor);
            }
            // 酒：本回合下一张【杀】伤害 +1（用后清零）。
            if f.is_sha && st.counter("jiu") > 0 {
                amount += 1;
                st.set_counter("jiu", 0);
                // 恃勇（将华雄）：标记此伤害为「酒【杀】」。
                st.set_counter(&format!("jiu_boost:{actor}"), 1);
            }
            // 白银狮子：受到的伤害改为 1。
            if crate::skills::has_equipment_skill(st, actor, "baiyin_shizi") && amount > 1 {
                amount = 1;
            }
            // 倾袭（曹休）：有武器时令目标二选一（弃牌并弃武器 / 伤害 +1）。
            if let Some(range) = crate::resolve_yijiang6::qingxi_available(st, f.source) {
                if f.source != actor {
                    push(
                        st,
                        Frame::QingXi(crate::resolve_yijiang6::QingXiFrame {
                            user: f.source,
                            target: actor,
                            card: card.clone(),
                            damage: amount,
                            damage_type: f.damage_type,
                            range,
                        }),
                    );
                    return;
                }
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
    let actor = f.actor;
    pop(st);
    maybe_leiji(st, actor);
}

/// 雷击：使用/打出【闪】时，张角可令一名其他角色判定。
fn maybe_leiji(st: &mut SgsState, actor: usize) {
    if st.seats.get(actor).is_some_and(|s| s.alive)
        && crate::skills::has(st, actor, "leiji")
        && st.alive_indices().len() > 1
    {
        push(st, Frame::Leiji(crate::resolve_wind::LeijiFrame { actor }));
    }
}

/// 响应牌已确定（可能是本人或主公技代替者）：统一结算并出栈。
pub(crate) fn finish_respond_play(
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
    // 潜袭/将驰：响应打出同样受限。
    if crate::skills::qianxi_banned(st, played_by, card_id) {
        return Err(illegal("潜袭：本回合不能打出该颜色的手牌"));
    }
    if expected == "sha" && crate::skills::sha_play_banned(st, played_by) {
        return Err(illegal("将驰：本回合不能打出【杀】"));
    }
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
    // 贞良（阴，卢植）：回合外打出的牌进入弃牌堆。
    crate::resolve_sp2::push_zhenliang_yin(st, played_by, card_id);
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
                maybe_leiji(st, played_by);
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
            maybe_leiji(st, played_by);
            // 界咆哮：本回合下一次【杀】伤害 +1（被【闪】抵消后）。
            if f.is_sha
                && crate::skills::has(st, f.source, "paoxiao")
                && crate::skills::is_jiexian(st, f.source)
            {
                st.set_counter("paoxiao_bonus", 1);
                st.push_log(
                    Some(&st.seats[f.source].player_id.clone()),
                    "咆哮：【杀】被抵消，本回合下一次【杀】伤害 +1",
                );
            }
            // 涯角（界赵云）：回合外使用/打出手牌后展示牌堆顶。
            crate::resolve_jiexian::yajiao_trigger(st, played_by, card_id);
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
            crate::resolve_jiexian::yajiao_trigger(st, played_by, card_id);
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
            crate::resolve_jiexian::yajiao_trigger(st, played_by, card_id);
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
            crate::resolve_jiexian::yajiao_trigger(st, played_by, card_id);
            Ok("A 已出杀，等待目标响应".to_string())
        }
    }
}

/// 奋激：手牌被弃置/获得后，周泰可失去 1 点体力令其摸 2 张。
fn maybe_fenji(st: &mut SgsState, lost_by: usize) {
    if let Some(owner) = crate::skills::fenji_owner(st, lost_by) {
        push(
            st,
            Frame::Fenji(crate::resolve_wind::FenjiFrame { owner, lost_by }),
        );
    }
}

fn act_pick(st: &mut SgsState, pending: &Pending, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::PickCard(f)) => f,
        _ => return Err(internal("栈顶不是拿牌帧")),
    };
    let target = f.target;
    let mut from_hand = false;
    let picked = if let Some(rest) = action.strip_prefix("pick:equip:") {
        let slot = slot_from_code(rest).ok_or_else(|| illegal("非法装备槽"))?;
        if pick_mode_discards(f.mode)
            && crate::skills::qicai_blocks_discard(st, target, f.user, slot)
        {
            return Err(illegal("奇才：不能弃置其防具/宝物牌"));
        }
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
        from_hand = true;
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
        PickMode::Tiaoxin => {
            to_discard(st, &picked);
            st.push_log(
                Some(&st.seats[f.user].player_id.clone()),
                format!("挑衅：弃置【{picked_name}】"),
            );
        }
        PickMode::Qizhi => {
            to_discard(st, &picked);
            st.push_log(
                Some(&st.seats[f.user].player_id.clone()),
                format!("奇制：弃置【{picked_name}】"),
            );
        }
        PickMode::Juzhan => {
            st.seats[f.user].hand.push(picked.clone());
            st.push_log(
                Some(&st.seats[f.user].player_id.clone()),
                format!("拒战（阴）：获得【{picked_name}】"),
            );
        }
        PickMode::Chezheng => {
            to_discard(st, &picked);
            st.push_log(
                Some(&st.seats[f.user].player_id.clone()),
                format!("掣政：弃置【{picked_name}】"),
            );
        }
    }
    pop(st);
    // 清俭（界夏侯惇）：摸牌阶段外获得牌后触发（须在 pop 之后压帧）。
    if matches!(
        f.mode,
        PickMode::Shunshou | PickMode::Fankui | PickMode::Juzhan
    ) {
        crate::resolve_jiexian::qingjian_trigger(st, f.user);
    }
    if f.mode == PickMode::Qizhi {
        crate::resolve_sp::qizhi_done(st, f.user, target);
    }
    if f.mode == PickMode::Juzhan {
        crate::resolve_sp::juzhan_yin_done(st, f.user, target);
    }
    // 反馈压在其他结算帧之上：不能覆盖父帧需要的 outcome（如【闪】响应结果）。
    if !matches!(
        f.mode,
        PickMode::Fankui
            | PickMode::Tiaoxin
            | PickMode::Qizhi
            | PickMode::Juzhan
            | PickMode::Chezheng
    ) {
        st.outcome = Some(Outcome::Picked {
            card: Some(picked.clone()),
        });
    }
    if !matches!(
        f.mode,
        PickMode::Guohe | PickMode::Tiaoxin | PickMode::Qizhi | PickMode::Chezheng
    ) {
        let user = f.user;
        let over = st.seats[user]
            .hand
            .len()
            .saturating_sub(st.config.max_hand_cards as usize);
        push_discard(st, user, over, "hand_cap");
    }
    if from_hand {
        maybe_fenji(st, target);
    }
    if target != st.turn {
        crate::skills::tuotian_on_lose(st, target);
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
    if action == "skill:fuli" {
        if f.victim != seat {
            return Err(illegal("伏枥只能自救"));
        }
        if !crate::skills::has(st, seat, "fuli") {
            return Err(illegal("没有该技能"));
        }
        if st.seats[seat].flags.contains_key("fuli_used") {
            return Err(illegal("伏枥（限定技）已使用过"));
        }
        st.seats[seat].flags.insert("fuli_used".into(), true);
        let x = crate::skills::kingdom_count(st).max(1) as i32;
        st.seats[seat].hp = x.min(st.seats[seat].max_hp);
        let flipped = !st.seats[seat].flipped;
        st.seats[seat].flipped = flipped;
        pop(st);
        st.phase = Phase::Turn;
        st.push_log(
            Some(&st.seats[seat].player_id.clone()),
            format!(
                "伏枥：回复至 {x} 点体力并翻面（体力 {}）",
                st.seats[seat].hp
            ),
        );
        return Ok(format!("伏枥：{} 脱离濒死", st.seats[seat].general_name));
    }
    if action == "skill:niepan" {
        if f.victim != seat {
            return Err(illegal("涅槃只能自救"));
        }
        if !crate::skills::has(st, seat, "niepan") {
            return Err(illegal("没有该技能"));
        }
        if st.seats[seat].flags.contains_key("niepan_used") {
            return Err(illegal("涅槃（限定技）本局已发动过"));
        }
        st.seats[seat].flags.insert("niepan_used".into(), true);
        let hand = std::mem::take(&mut st.seats[seat].hand);
        let equips: Vec<String> = st.seats[seat]
            .equips
            .values_mut()
            .filter_map(Option::take)
            .collect();
        let judge = std::mem::take(&mut st.seats[seat].judge_zone);
        let marks: Vec<String> = st.seats[seat]
            .marks
            .values_mut()
            .flat_map(|v| v.drain(..))
            .collect();
        for c in hand.into_iter().chain(equips).chain(judge).chain(marks) {
            turn::to_discard_card(st, &c);
        }
        st.seats[seat].flipped = false;
        crate::skills::faen_trigger(st, seat);
        st.seats[seat].hp = st.seats[seat].max_hp.min(3);
        st.phase = Phase::Turn;
        pop(st);
        let mut ev = Vec::new();
        let drawn = turn::draw_cards(st, seat, 3, &mut ev);
        let over = st.seats[seat]
            .hand
            .len()
            .saturating_sub(st.config.max_hand_cards as usize);
        if over > 0 {
            push_discard(st, seat, over, "hand_cap");
        }
        st.push_log(
            Some(&pending.actor.clone()),
            format!(
                "涅槃：弃置区域内所有牌，复原武将牌，摸 {drawn} 张并回复至 {} 点体力",
                st.seats[seat].hp
            ),
        );
        return Ok("涅槃：已回复至 3 点体力".to_string());
    }
    if let Some(card_id) = action.strip_prefix("skill:guhuo:") {
        if f.victim != seat {
            return Err(illegal("蛊惑当【桃】只能用于自救"));
        }
        if !crate::resolve_wind::guhuo_available(st, seat) {
            return Err(illegal("蛊惑本回合已使用过"));
        }
        if !st.seats[seat].hand.iter().any(|c| c == card_id) {
            return Err(illegal("手牌中没有该牌"));
        }
        crate::resolve_wind::start_guhuo_response(st, seat, "dying", "tao", card_id, None)?;
        return Ok("蛊惑：请等待其他角色质疑".to_string());
    }
    let card_id = action
        .strip_prefix("respond:")
        .ok_or_else(|| illegal("需要使用桃或 pass"))?;
    let actual = turn::card_def(st, card_id).map(|d| d.code);
    // 酒自救：只允许对自己、且不享受救援加成（含酒池转化）。
    let jiu_conversion = if actual == Some("jiu") {
        None
    } else {
        crate::skills::conversion_for(st, seat, card_id, "jiu")
    };
    if actual == Some("jiu") || jiu_conversion.is_some() {
        if f.victim != seat {
            return Err(illegal("【酒】只能在濒死时对自己使用"));
        }
        let used = match jiu_conversion {
            Some((_, name)) => format!(
                "{name}：将【{}】当【酒】自救",
                turn::card_label(st, card_id)
            ),
            None => format!("使用【{}】自救", turn::card_label(st, card_id)),
        };
        turn::take_card(st, seat, card_id)?;
        turn::to_discard_card(st, card_id);
        let victim = f.victim;
        st.seats[victim].hp += 1;
        let hp = st.seats[victim].hp;
        st.push_log(Some(&pending.actor.clone()), format!("{used}（体力 {hp}）"));
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
    // 界救援（主公技）：吴势力角色于其回合内自救时，可改为令主公回复 1 并摸 1。
    if seat == victim && crate::skills::jie_jiuyuan_redirect(st, seat) {
        pop(st);
        return Ok("救援：改为令主公回复 1 点体力".to_string());
    }
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
    if f.reason == "discard_phase" {
        st.phase_discarded.extend(ids.iter().cloned());
    }
    if f.actor == st.turn {
        st.bump_counter("discard_done");
    }
    maybe_fenji(st, f.actor);
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
    // 弃牌堆钩子（落英等）必须在 pop/replace_top 之后压帧，避免被覆盖。
    for id in &ids {
        crate::resolve_yijiang::on_card_discarded(st, Some(f.actor), id);
    }
    Ok(text)
}

// ---------------------------------------------------------------------------
// 伤害 / 死亡 / 胜负
// ---------------------------------------------------------------------------

/// 造成伤害（入口）：先开「受到伤害时」窗口（天香），再进入实际结算。
pub fn apply_damage(
    st: &mut SgsState,
    victim: usize,
    amount: i32,
    source: Option<usize>,
    dtype: DamageType,
    card: Option<&str>,
) {
    apply_damage_inner(st, victim, amount, source, dtype, card, true);
}

/// 绕过天香窗口直接结算伤害（天香内部、以及已确认要承受伤害时使用）。
pub(crate) fn apply_damage_now(
    st: &mut SgsState,
    victim: usize,
    amount: i32,
    source: Option<usize>,
    dtype: DamageType,
    card: Option<&str>,
) {
    apply_damage_inner(st, victim, amount, source, dtype, card, false);
}

fn apply_damage_inner(
    st: &mut SgsState,
    victim: usize,
    mut amount: i32,
    source: Option<usize>,
    dtype: DamageType,
    card: Option<&str>,
    allow_tianxiang: bool,
) {
    // 绝情（张春华，锁定）：你即将造成的伤害视为失去体力。
    if amount > 0 {
        if let Some(src) = source {
            if src != victim && crate::skills::has(st, src, "jueqing") {
                st.seats[victim].hp -= amount;
                let hp = st.seats[victim].hp;
                st.push_log(
                    Some(&st.seats[src].player_id.clone()),
                    format!(
                        "绝情：{} 失去 {amount} 点体力（体力 {hp}）",
                        st.seats[victim].general_name
                    ),
                );
                for _ in 0..amount {
                    crate::skills::zhaxiang_on_lose_hp(st, victim);
                }
                if hp <= 0 {
                    push_dying(st, victim, source);
                }
                return;
            }
        }
    }
    // 怀橘（陆绩）：有「橘」的角色受到伤害时，防止此伤害并移除 1 枚「橘」。
    if amount > 0 && crate::resolve_sp::huaiju_prevent(st, victim) {
        return;
    }
    // 掣政（孙亮，锁定）：出牌阶段内，目标攻击范围不含你的伤害被防止。
    if amount > 0 {
        if let Some(src) = source {
            if src != victim && crate::resolve_sp::chezheng_blocks(st, src, victim) {
                let pid = st.seats[src].player_id.clone();
                st.push_log(
                    Some(&pid),
                    format!(
                        "掣政：防止对 {} 造成的伤害（其攻击范围不含孙亮）",
                        st.seats[victim].general_name
                    ),
                );
                return;
            }
        }
    }
    // 自守（刘表）：本回合你防止对其他角色造成的伤害。
    if amount > 0 {
        if let Some(src) = source {
            if crate::skills::zishou_blocks_damage(st, src, victim) {
                st.push_log(
                    Some(&st.seats[src].player_id.clone()),
                    format!("自守：防止对 {} 造成的伤害", st.seats[victim].general_name),
                );
                return;
            }
        }
    }
    // 无言（徐庶，锁定）：防止你使用和受到的锦囊牌造成的伤害。
    if amount > 0 && crate::skills::wuyan_blocks(st, source.unwrap_or(victim), victim, card) {
        st.push_log(
            Some(&st.seats[victim].player_id.clone()),
            "无言：锦囊牌造成的伤害被防止".to_string(),
        );
        return;
    }
    // 制蛮（马谡）：对其他角色造成伤害时，可防止此伤害并获得其一张牌。
    if amount > 0 && allow_tianxiang {
        if let Some(src) = source {
            if src != victim
                && crate::resolve_yijiang::zhiman_trigger(st, src, victim, amount, dtype, card)
            {
                return;
            }
        }
    }
    // 天香：受到伤害时，可弃红桃手牌防止伤害并转移。
    if allow_tianxiang
        && amount > 0
        && st.seats.get(victim).is_some_and(|s| s.alive)
        && crate::skills::tianxiang_available(st, victim)
    {
        push(
            st,
            Frame::Tianxiang(crate::resolve_wind::TianxiangFrame {
                victim,
                amount,
                source,
                damage_type: dtype,
                card: card.map(str::to_string),
                stage: crate::resolve_wind::TianxiangStage::Ask,
                target: None,
                discarded: None,
            }),
        );
        return;
    }
    // 仁心（曹冲）：体力值为 1 的其他角色受到伤害时，可弃一张装备牌并翻面防止之。
    if allow_tianxiang && amount > 0 && st.seats.get(victim).is_some_and(|s| s.alive && s.hp == 1) {
        if let Some(owner) = crate::resolve_yijiang3::renxin_owner(st, victim) {
            push(
                st,
                Frame::RenXin(crate::resolve_yijiang3::RenXinFrame {
                    owner,
                    victim,
                    amount,
                    source,
                    damage_type: dtype,
                    card: card.map(str::to_string),
                }),
            );
            return;
        }
    }
    // 骄矜（孙鲁班）：男性角色对你造成伤害时，可弃一张装备牌令伤害 -1。
    if allow_tianxiang
        && amount > 0
        && source.is_some_and(|src| src != victim && crate::skills::is_male(st, src))
        && crate::skills::has(st, victim, "jiaojin")
        && crate::resolve_yijiang6::jiaojin_available(st, victim)
    {
        push(
            st,
            Frame::JiaoJin(crate::resolve_yijiang6::JiaoJinFrame {
                owner: victim,
                amount,
                source,
                damage_type: dtype,
                card: card.map(str::to_string),
            }),
        );
        return;
    }
    // 陷阵（拼点赢）：无视被拼点目标的防具（藤甲/白银狮子等不生效）。
    let armor_ignored = source.is_some_and(|src| {
        crate::resolve_yijiang::xianzhen_ignore_armor(st, victim) && src != victim
    });
    // 藤甲：火焰伤害 +1；普通【杀】/南蛮/万箭伤害无效。
    if !armor_ignored && crate::skills::has_equipment_skill(st, victim, "tengjia") {
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
    // 矢北（沮授，锁定）：本回合首次受伤回复 1；再受伤失去 1 点体力。
    if amount > 0 {
        crate::resolve_yijiang6::shibei_on_damage(st, victim);
    }
    // 诈降（界黄盖，锁定）：失去 1 点体力后摸 3 张（按伤害点数逐点触发）。
    if amount > 0 {
        for _ in 0..amount {
            crate::skills::zhaxiang_on_lose_hp(st, victim);
        }
    }
    // 忘隙（界李典）：造成/受到 1 点伤害后摸 2 并交给对方一张。
    if amount == 1 {
        if let Some(src) = source {
            if src != victim {
                crate::resolve_jiexian::wangxi_trigger(st, src, victim);
                crate::resolve_jiexian::wangxi_trigger(st, victim, src);
            }
        }
    }
    if source == Some(st.turn) {
        st.bump_counter("damage_dealt");
    }
    st.bump_counter("phase_damage");
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
    // 趫猛（界公孙瓒）/ 利驭（界吕布）：使用【杀】造成伤害后。
    if let (Some(card_id), Some(src)) = (card, source) {
        let is_sha = st
            .cards
            .get(card_id)
            .is_some_and(|c| matches!(c.def.as_str(), "sha" | "huo_sha" | "lei_sha"));
        if is_sha && src != victim && st.seats[src].alive && st.seats[victim].hp > 0 {
            // 趫猛：弃置其区域一张牌；若为坐骑牌则获得之（自动）。
            if crate::skills::has(st, src, "qiaomeng")
                && crate::skills::is_jiexian(st, src)
                && crate::turn::has_cards_to_take(st, victim)
            {
                if let Some(taken) = crate::resolve_jiexian::take_one_from_region(st, victim) {
                    let label = turn::card_label(st, &taken);
                    let mount = st
                        .cards
                        .get(&taken)
                        .and_then(|c| crate::data::card_def(&c.def))
                        .and_then(|d| d.equip_slot)
                        .is_some_and(|sl| {
                            matches!(sl, EquipSlot::HorseOffense | EquipSlot::HorseDefense)
                        });
                    if mount {
                        st.seats[src].hand.push(taken.clone());
                        st.push_log(
                            Some(&st.seats[src].player_id.clone()),
                            format!("趫猛：弃置并获得了坐骑牌【{label}】"),
                        );
                    } else {
                        turn::discard_by(st, Some(victim), &taken);
                        st.push_log(
                            Some(&st.seats[src].player_id.clone()),
                            format!("趫猛：弃置 {} 的【{label}】", st.seats[victim].general_name),
                        );
                    }
                }
            }
            // 利驭：可获得其区域一张牌（装备牌则引发决斗）。
            crate::resolve_jiexian::liyu_trigger(st, src, victim);
        }
    }
    // 夺刀（潘璋马忠）：受到其他角色【杀】的伤害后，可弃一张牌获得其武器。
    if amount > 0 && st.seats[victim].hp > 0 && crate::skills::has(st, victim, "duodao") {
        if let Some(src) = source {
            let has_weapon = st.seats[src]
                .equips
                .get(&EquipSlot::Weapon)
                .cloned()
                .flatten()
                .is_some();
            if src != victim
                && has_weapon
                && !st.seats[victim].hand.is_empty()
                && card.is_some_and(|id| {
                    st.cards
                        .get(id)
                        .is_some_and(|c| matches!(c.def.as_str(), "sha" | "huo_sha" | "lei_sha"))
                })
            {
                push(
                    st,
                    Frame::DuoDao(crate::resolve_yijiang4::DuoDaoFrame {
                        owner: victim,
                        source: src,
                    }),
                );
            }
        }
    }
    // 恩怨（法正）：每受到 1 点伤害后，来源交一张手牌或失去 1 点体力。
    if amount > 0 && st.seats[victim].hp > 0 && crate::skills::has(st, victim, "enyuan") {
        if let Some(src) = source {
            if src != victim {
                for _ in 0..amount {
                    push(
                        st,
                        Frame::EnYuan(crate::resolve_yijiang4::EnYuanFrame {
                            owner: victim,
                            source: src,
                        }),
                    );
                }
            }
        }
    }
    // 父魂（关兴张苞）：以此法造成伤害后获得武圣/咆哮（本回合）。
    if let (Some(src), Some(card_id)) = (source, card) {
        if src != victim {
            crate::resolve_yijiang5::fuhen_on_damage(st, src, card_id);
        }
    }
    // 奔袭（吴懿）：选择「造成伤害后摸一张」时摸 1。
    if amount > 0 {
        if let Some(src) = source {
            if crate::skills::has(st, src, "benxi") && st.counter("benxi_draw") > 0 {
                st.set_counter("benxi_draw", 0);
                let mut ev = Vec::new();
                let drawn = turn::draw_cards(st, src, 1, &mut ev);
                st.push_log(
                    Some(&st.seats[src].player_id.clone()),
                    format!("奔袭：造成伤害后摸 {drawn} 张"),
                );
            }
        }
    }
    // 智愚（荀攸）：受到伤害后摸 1；若手牌颜色均相同，来源弃置一张手牌。
    if st.seats[victim].hp > 0 && crate::skills::has(st, victim, "zhiyu") && amount > 0 {
        let mut ev = Vec::new();
        turn::draw_cards(st, victim, 1, &mut ev);
        let colors: Vec<bool> = st.seats[victim]
            .hand
            .iter()
            .filter_map(|id| st.cards.get(id).map(|c| crate::skills::is_red(c.suit)))
            .collect();
        let same = !colors.is_empty() && colors.iter().all(|c| *c == colors[0]);
        st.push_log(
            Some(&st.seats[victim].player_id.clone()),
            format!(
                "智愚：摸 1 张并展示手牌（{}）",
                if same {
                    "颜色均相同"
                } else {
                    "颜色不同"
                }
            ),
        );
        if same {
            if let Some(src) = source {
                if src != victim && !st.seats[src].hand.is_empty() {
                    let n = st.seats[src].hand.len();
                    let idx = st.next_index(n);
                    let card = st.seats[src].hand.remove(idx);
                    crate::skills::lianying_on_lose_last(st, src);
                    let label = turn::card_label(st, &card);
                    turn::discard_by(st, Some(src), &card);
                    st.push_log(
                        Some(&st.seats[src].player_id.clone()),
                        format!("智愚：颜色均相同，弃置一张手牌【{label}】"),
                    );
                }
            }
        }
    }
    // 恃勇（将华雄，锁定）：受到红色【杀】或【酒】【杀】造成的伤害后，减 1 点体力上限。
    if amount > 0 && st.seats[victim].hp > 0 && crate::skills::has(st, victim, "shiyong") {
        let red_sha = card.is_some_and(|id| {
            st.cards.get(id).is_some_and(|c| {
                matches!(c.def.as_str(), "sha" | "huo_sha" | "lei_sha")
                    && crate::skills::is_red(c.suit)
            })
        });
        let jiu_key = format!("jiu_boost:{victim}");
        let jiu = st.counter(&jiu_key) > 0;
        if jiu {
            st.set_counter(&jiu_key, 0);
        }
        if red_sha || jiu {
            st.seats[victim].max_hp = (st.seats[victim].max_hp - 1).max(1);
            st.seats[victim].hp = st.seats[victim].hp.min(st.seats[victim].max_hp);
            st.push_log(
                Some(&st.seats[victim].player_id.clone()),
                format!(
                    "恃勇：{}",
                    if red_sha {
                        "受到红色【杀】伤害，减 1 点体力上限"
                    } else {
                        "受到【酒】【杀】伤害，减 1 点体力上限"
                    }
                ),
            );
        }
    }
    // 战绝（刘谌）：因该【决斗】受伤后，刘谌与受伤者各摸一张。
    if let Some(card_id) = card {
        crate::resolve_yijiang7::zhanjue_on_damage(st, source, victim, card_id);
    }
    // 邀名（全琮）：造成/受到伤害后，每回合限一次。
    if amount > 0 {
        if let Some(src) = source {
            if src != victim {
                crate::resolve_yijiang7::yaoming_trigger(st, src);
            }
        }
        crate::resolve_yijiang7::yaoming_trigger(st, victim);
    }
    // 恢拓（曹叡）：受到伤害后，可令一名角色判定。
    if amount > 0 && st.seats[victim].hp > 0 && crate::skills::has(st, victim, "huituo") {
        push(
            st,
            Frame::HuiTuo(crate::resolve_yijiang6::HuiTuoFrame {
                owner: victim,
                amount,
            }),
        );
    }
    // 权计（钟会）：每受到 1 点伤害后，摸 1 张并将一张手牌置于武将牌上为「权」。
    if amount > 0 && st.seats[victim].hp > 0 && crate::skills::has(st, victim, "quanji") {
        for _ in 0..amount {
            let mut ev = Vec::new();
            turn::draw_cards(st, victim, 1, &mut ev);
            let n = st.seats[victim].hand.len();
            if n > 0 {
                let idx = st.next_index(n);
                let card = st.seats[victim].hand.remove(idx);
                let label = turn::card_label(st, &card);
                crate::skills::add_mark(st, victim, "quan", &card);
                st.push_log(
                    Some(&st.seats[victim].player_id.clone()),
                    format!("权计：摸 1 张并将【{label}】置于武将牌上为「权」"),
                );
            }
        }
    }
    // 称象（曹冲）：受到伤害后亮出牌堆顶 4 张，获得点数和 ≤13 的任意张。
    if amount > 0 && st.seats[victim].hp > 0 && crate::skills::has(st, victim, "chengxiang") {
        crate::resolve_yijiang3::start_chengxiang(st, victim);
    }
    // 御策（满宠）：受到伤害后展示一张手牌，令伤害来源二选一。
    if amount > 0 && st.seats[victim].hp > 0 && crate::skills::has(st, victim, "yuce") {
        if let Some(src) = source {
            if src != victim && !st.seats[victim].hand.is_empty() {
                if st.seats[src].hand.is_empty() {
                    if st.seats[victim].hp < st.seats[victim].max_hp {
                        st.seats[victim].hp += 1;
                    }
                    st.push_log(
                        Some(&st.seats[victim].player_id.clone()),
                        format!(
                            "御策：来源无手牌可弃，回复 1 点体力（体力 {}）",
                            st.seats[victim].hp
                        ),
                    );
                } else {
                    push(
                        st,
                        Frame::YuCe(crate::resolve_yijiang3::YuCeFrame {
                            actor: victim,
                            source: src,
                            stage: crate::resolve_yijiang3::YuCeStage::Reveal,
                            card: None,
                        }),
                    );
                }
            }
        }
    }
    // 疠火（程普）：视为火【杀】造成伤害后，失去 1 点体力。
    if amount > 0 {
        if let Some(src) = source {
            if let Some(card_id) = card {
                let key = format!("lihuo:{card_id}");
                if crate::skills::has(st, src, "lihuo") && st.counter(&key) > 0 {
                    st.set_counter(&key, 0);
                    st.seats[src].hp -= 1;
                    let hp = st.seats[src].hp;
                    st.push_log(
                        Some(&st.seats[src].player_id.clone()),
                        format!("疠火：失去 1 点体力（体力 {hp}）"),
                    );
                    crate::skills::zhaxiang_on_lose_hp(st, src);
                    if hp <= 0 {
                        push_dying(st, src, None);
                    }
                }
            }
        }
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
    // 节命：受到伤害后，可令一名角色摸至体力上限（至多 5）。
    if amount > 0 && st.seats[victim].hp > 0 && crate::skills::has(st, victim, "jieming") {
        push(
            st,
            Frame::Jieming(crate::resolve_fire::JiemingFrame { actor: victim }),
        );
    }
    // 放逐：受到伤害后，可令一名其他角色翻面并摸 X 张。
    if amount > 0 && st.seats[victim].hp > 0 && crate::skills::has(st, victim, "fangzhu") {
        push(
            st,
            Frame::Fangzhu(crate::resolve_forest::FangzhuFrame { actor: victim }),
        );
    }
    // 新生：受到伤害后获得一张新的化身牌。
    if amount > 0 && st.seats[victim].hp > 0 {
        crate::resolve_mountain::xinsheng_gain(st, victim);
    }
    // 悲歌：角色受到【杀】的伤害后，可弃一张牌令其判定。
    if amount > 0 {
        if let Some(card_id) = card {
            if st.cards.get(card_id).map(|c| c.def.as_str()) == Some("sha") {
                if let Some(owner) = crate::resolve_mountain::beige_owner(st) {
                    push(
                        st,
                        Frame::Beige(crate::resolve_mountain::BeigeFrame {
                            owner,
                            target: victim,
                            source,
                        }),
                    );
                }
            }
        }
    }
    // 烈刃：使用【杀】造成伤害后，可与目标拼点；赢则获得其一张牌。
    if let (Some(card_id), Some(src)) = (card, source) {
        if st.seats[victim].hp > 0
            && src != victim
            && st.cards.get(card_id).map(|c| c.def.as_str()) == Some("sha")
            && crate::skills::has(st, src, "liewen")
        {
            push(
                st,
                Frame::Liewen(crate::resolve_forest::LiewenFrame {
                    actor: src,
                    target: victim,
                    stage: crate::resolve_forest::LiewenStage::Start,
                    saved_outcome: None,
                }),
            );
        }
    }
    // 暴虐：其他群势力角色造成伤害后，可令主公董卓判定（黑桃回复 1）。
    if let Some(src) = source {
        let qun = st.seats[src]
            .general
            .as_deref()
            .and_then(crate::data::general)
            .is_some_and(|g| g.kingdom == "qun");
        if src != victim && qun {
            if let Some(lord) = st.alive_indices().into_iter().find(|i| {
                *i != src && crate::skills::has(st, *i, "baonue") && crate::skills::is_lord(st, *i)
            }) {
                push(
                    st,
                    Frame::QuickJudge(QuickJudgeFrame {
                        owner: lord,
                        reason: "baonue".into(),
                        stage: QuickJudgeStage::Reveal,
                        judge_card: None,
                        rewrite_order: Vec::new(),
                        rewrite_cursor: 0,
                        data: json!({ "lord": lord, "source": src }),
                        saved_outcome: None,
                    }),
                );
            }
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
    // 智迟（陈宫，锁定）：回合外受到伤害后 → 本回合【杀】与普通锦囊对其无效。
    crate::resolve_yijiang::zhichi_on_damage(st, victim);
    // 酒诗（曹植）：受伤后若背面朝上，可翻回正面。
    crate::resolve_yijiang::jiushi_heal_flip(st, victim);
    // 审时（阴）：其他角色对你造成伤害后，可观看其手牌并交给其一张牌。
    if let Some(src) = source {
        if src != victim && st.seats[victim].hp > 0 && amount > 0 {
            crate::resolve_sp::push_shenshi_yin(st, victim, src);
        }
    }
    // 狂骨：对距离 1 以内角色造成伤害后，可回复 1 点体力或摸 1 张。
    if let Some(src) = source {
        if src != victim
            && st.seats[src].alive
            && st.seats[victim].hp > 0
            && crate::skills::has(st, src, "kuanggu")
            && turn::effective_distance(st, src, victim) <= 1
        {
            push(
                st,
                Frame::Kuanggu(crate::resolve_wind::KuangguFrame { actor: src }),
            );
        }
    }
}

pub fn push_dying(st: &mut SgsState, victim: usize, source: Option<usize>) {
    // 醇醪（程普）：有「醇」的角色可令濒死者视为使用一张【酒】（回复 1），自动移去一张。
    if st.seats.get(victim).is_some_and(|s| s.hp <= 0) {
        if let Some(owner) = st
            .alive_indices()
            .into_iter()
            .find(|i| crate::skills::chun_count(st, *i) > 0)
        {
            let card = crate::skills::mark_cards(st, owner, "chun")
                .into_iter()
                .next()
                .unwrap_or_default();
            if !card.is_empty() {
                crate::skills::remove_mark(st, owner, "chun", &card);
                turn::discard_by(st, Some(owner), &card);
                st.seats[victim].hp += 1;
                let hp = st.seats[victim].hp;
                st.push_log(
                    Some(&st.seats[owner].player_id.clone()),
                    format!(
                        "醇醪：移去一张「醇」，{} 视为使用【酒】（体力 {hp}）",
                        st.seats[victim].general_name
                    ),
                );
                if hp > 0 {
                    return;
                }
            }
        }
    }
    // 兴衰（曹叡，主公技 / 限定）：濒死时其他魏势力角色令其回复体力。
    if crate::resolve_yijiang6::xingshuai_on_dying(st, victim) {
        if st.seats[victim].hp > 0 {
            return;
        }
    }
    // 补益（吴国太）：角色进入濒死时，可展示其一张手牌，非基本牌则弃置并回复 1。
    if st.seats.get(victim).is_some_and(|s| s.alive)
        && crate::resolve_yijiang::buyi_on_dying(st, victim)
    {
        return;
    }
    // 不屈（锁定）：濒死时翻「创」，点数不重复则回复至 1 点体力。
    if st.seats.get(victim).is_some_and(|s| s.alive)
        && crate::skills::has(st, victim, "buqu")
        && crate::skills::buqu_save(st, victim)
    {
        return;
    }
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
    if source == Some(st.turn) {
        st.bump_counter("phase_kills");
    }
    st.seats[victim].alive = false;
    st.seats[victim].hp = 0;
    // 断肠：杀死蔡文姬的角色失去所有技能。
    if crate::skills::has(st, victim, "duanchang") {
        if let Some(src) = source {
            if src != victim {
                st.seats[src].flags.insert("skills_lost".into(), true);
                st.push_log(
                    Some(&st.seats[src].player_id.clone()),
                    "断肠：杀死蔡文姬，失去所有技能",
                );
            }
        }
    }
    // 行殇：其他角色死亡时，可在清理前获得其所有牌（终局时不触发，避免帧被清空丢牌）。
    let game_over = check_winner(st).is_some();
    let xingshang_owner = if game_over {
        None
    } else {
        st.alive_indices()
            .into_iter()
            .find(|i| *i != victim && crate::skills::has(st, *i, "xingshang"))
    };
    // 武将牌上的标记不属于「其所有牌」，始终弃置。
    let marks: Vec<String> = st.seats[victim]
        .marks
        .values_mut()
        .flat_map(|v| v.drain(..))
        .collect();
    for c in marks {
        to_discard(st, &c);
    }
    if xingshang_owner.is_none() {
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
    }
    st.push_log(None, format!("{pid} 阵亡（身份公开）"));
    events.push(EngineEvent::new(
        "player_died",
        json!({ "player": pid, "identity": identity.code() }),
    ));
    st.phase = Phase::Turn;
    st.pending = None;
    if let Some(owner) = xingshang_owner {
        push(
            st,
            Frame::Xingshang(crate::resolve_forest::XingshangFrame { victim, owner }),
        );
    }
    // 追忆（步练师）：死亡时可令一名其他角色（杀死你的角色除外）摸 3 并回复 1。
    if crate::skills::has(st, victim, "zhuiyi") && !game_over {
        push(
            st,
            Frame::ZhuiYi(crate::resolve_yijiang2::ZhuiYiFrame {
                dead: victim,
                killer: source,
            }),
        );
    }

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
