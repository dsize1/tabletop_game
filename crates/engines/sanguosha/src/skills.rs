//! 技能内核（P2.4，第一批）：技能注册表 + 触发钩子。
//!
//! 设计：
//! - 技能按**武将**绑定（`sgs_generals.code` → 技能列表），引擎用固定顺序派发
//!   （技能声明优先级 → 当前回合玩家左手侧座位顺序 → 注册顺序），不依赖 HashMap 迭代。
//! - 需要玩家决策的技能压入结算栈帧（`Jianxiong` / `MultiSelect`），与锦囊/无懈共用同一套 pending。
//! - 本批实现：奸雄、制衡、救援、闭月、集智、奇才、克己。
//!   尚未实现（P2.4 后续）：仁德、激将、护驾、离间、突袭、奇袭。

use crate::data;
use crate::state::*;
use game_engine_core::EngineEvent;
use serde_json::json;

/// 技能类型（与中台 `sgs_skills.type` 对齐）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Limited / Awaken 预留给后续武将
pub enum SkillType {
    Normal,
    Lord,
    Locked,
    Limited,
    Awaken,
}

impl SkillType {
    pub fn code(self) -> &'static str {
        match self {
            SkillType::Normal => "normal",
            SkillType::Lord => "lord",
            SkillType::Locked => "locked",
            SkillType::Limited => "limited",
            SkillType::Awaken => "awaken",
        }
    }
}

pub struct SkillDef {
    // name/kind 供视图与后续技能派发使用
    pub code: &'static str,
    pub name: &'static str,
    pub kind: SkillType,
    pub implemented: bool,
}

/// 武将 → 技能（按 `sgs_generals.code`；本批只列 P2 池内武将）。
pub fn skills_of(general: &str) -> &'static [SkillDef] {
    const LIUBEI: &[SkillDef] = &[
        SkillDef {
            code: "rende",
            name: "仁德",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "jijiang",
            name: "激将",
            kind: SkillType::Lord,
            implemented: true,
        },
    ];
    const CAOCAO: &[SkillDef] = &[
        SkillDef {
            code: "jianxiong",
            name: "奸雄",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "hujia",
            name: "护驾",
            kind: SkillType::Lord,
            implemented: true,
        },
    ];
    const SUNQUAN: &[SkillDef] = &[
        SkillDef {
            code: "zhiheng",
            name: "制衡",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "jiuyuan",
            name: "救援",
            kind: SkillType::Lord,
            implemented: true,
        },
    ];
    const HUANGYUEYING: &[SkillDef] = &[
        SkillDef {
            code: "jizhi",
            name: "集智",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "qicai",
            name: "奇才",
            kind: SkillType::Locked,
            implemented: true,
        },
    ];
    const LVMENG: &[SkillDef] = &[SkillDef {
        code: "keji",
        name: "克己",
        kind: SkillType::Normal,
        implemented: true,
    }];
    const DIAOCHAN: &[SkillDef] = &[
        SkillDef {
            code: "biyue",
            name: "闭月",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "lijian",
            name: "离间",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const ZHANGLIAO: &[SkillDef] = &[SkillDef {
        code: "tuxi",
        name: "突袭",
        kind: SkillType::Normal,
        implemented: true,
    }];
    const GANNING: &[SkillDef] = &[SkillDef {
        code: "qixi",
        name: "奇袭",
        kind: SkillType::Normal,
        implemented: true,
    }];
    // P2.5：观星（特殊区 `guanxing`）；空城留 P4。
    const ZHUGELIANG: &[SkillDef] = &[
        SkillDef {
            code: "guanxing",
            name: "观星",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "kongcheng",
            name: "空城",
            kind: SkillType::Locked,
            implemented: false,
        },
    ];
    match general {
        "sgs_hero_1" => LIUBEI,
        "sgs_hero_15" => CAOCAO,
        "sgs_hero_8" => SUNQUAN,
        "sgs_hero_7" => HUANGYUEYING,
        "sgs_hero_10" => LVMENG,
        "sgs_hero_24" => DIAOCHAN,
        "sgs_hero_18" => ZHANGLIAO,
        "sgs_hero_9" => GANNING,
        "sgs_hero_4" => ZHUGELIANG,
        _ => &[],
    }
}

/// 是否拥有某技能（只有已实现的技能会触发）。
pub fn has(st: &SgsState, seat: usize, code: &str) -> bool {
    let Some(general) = st.seats.get(seat).and_then(|s| s.general.as_deref()) else {
        return false;
    };
    skills_of(general)
        .iter()
        .any(|s| s.code == code && s.implemented)
}

/// 当前玩家（按座位顺序）中第一个拥有该技能的玩家（主公技救援等使用）。
#[allow(dead_code)]
pub fn find_owner(st: &SgsState, code: &str) -> Option<usize> {
    st.alive_indices().into_iter().find(|i| has(st, *i, code))
}

/// 摸牌并检查持有上限（超出时压入弃牌帧）。
fn draw_and_cap(st: &mut SgsState, seat: usize, n: u32, events: &mut Vec<EngineEvent>) -> usize {
    let drawn = crate::turn::draw_cards(st, seat, n, events);
    let over = st.seats[seat]
        .hand
        .len()
        .saturating_sub(st.config.max_hand_cards as usize);
    if over > 0 {
        crate::resolve::push_discard(st, seat, over, "hand_cap");
    }
    drawn
}

/// 是否主公。
pub fn is_lord(st: &SgsState, seat: usize) -> bool {
    st.seats.get(seat).is_some_and(|s| s.identity.is_lord())
}

/// 是否男性（用于离间等）。
pub fn is_male(st: &SgsState, seat: usize) -> bool {
    st.seats
        .get(seat)
        .and_then(|s| s.general.as_deref())
        .and_then(data::general)
        .map(|g| g.gender == "male")
        .unwrap_or(false)
}

fn kingdom(st: &SgsState, seat: usize) -> &'static str {
    st.seats
        .get(seat)
        .and_then(|s| s.general.as_deref())
        .and_then(data::general)
        .map(|g| g.kingdom)
        .unwrap_or("")
}

/// 主公技接力：`needed` = sha（激将/蜀）或 shan（护驾/魏）。
pub fn requisition_available(st: &SgsState, seat: usize, needed: &str) -> bool {
    let (skill, faction) = match needed {
        "sha" => ("jijiang", "shu"),
        "shan" => ("hujia", "wei"),
        _ => return false,
    };
    is_lord(st, seat)
        && has(st, seat, skill)
        && !requisition_order(st, seat, needed).is_empty()
        && st
            .seats_from_left_of(seat)
            .into_iter()
            .any(|i| st.seats[i].alive && kingdom(st, i) == faction)
}

/// 接力询问顺序：从主公左手侧开始的同势力存活角色。
///
/// **无手牌的同势力角色不进入询问队列**（无牌可代出；手牌数是公共信息，
/// 与 `special-zones.md` §4.1 的响应询问口径一致）。
pub fn requisition_order(st: &SgsState, seat: usize, needed: &str) -> Vec<String> {
    let faction = match needed {
        "sha" => "shu",
        "shan" => "wei",
        _ => return Vec::new(),
    };
    st.seats_from_left_of(seat)
        .into_iter()
        .filter(|i| {
            st.seats[*i].alive && kingdom(st, *i) == faction && !st.seats[*i].hand.is_empty()
        })
        .map(|i| st.seats[i].player_id.clone())
        .collect()
}

// ---------------------------------------------------------------------------
// 钩子
// ---------------------------------------------------------------------------

/// 阶段开始钩子：闭月（结束阶段摸 1）。
pub fn on_phase_start(st: &mut SgsState, phase: TurnPhase, events: &mut Vec<EngineEvent>) {
    if phase != TurnPhase::End {
        return;
    }
    let idx = st.turn;
    if !has(st, idx, "biyue") {
        return;
    }
    let drawn = draw_and_cap(st, idx, 1, events);
    st.push_log(
        Some(&st.seats[idx].player_id.clone()),
        format!("闭月：摸 {drawn} 张"),
    );
}

/// 使用锦囊后的钩子：集智（使用非延时锦囊时摸 1）。
pub fn on_non_delayed_trick_used(st: &mut SgsState, seat: usize, events: &mut Vec<EngineEvent>) {
    if !has(st, seat, "jizhi") {
        return;
    }
    let drawn = draw_and_cap(st, seat, 1, events);
    st.push_log(
        Some(&st.seats[seat].player_id.clone()),
        format!("集智：摸 {drawn} 张"),
    );
}

/// 弃牌阶段上限：克己（出牌阶段未使用过杀 → 跳过弃牌）。
pub fn hand_limit(st: &SgsState, seat: usize) -> usize {
    if has(st, seat, "keji") && st.counter("sha_used") == 0 {
        usize::MAX
    } else {
        st.hand_limit(seat)
    }
}

/// 奇才：使用锦囊无距离限制。
pub fn trick_distance_unlimited(st: &SgsState, seat: usize) -> bool {
    has(st, seat, "qicai")
}

/// 救援：其他吴势力角色对拥有此技的主公使用【桃】时，回复 +1。
pub fn tao_bonus(st: &SgsState, healer: usize, target: usize) -> i32 {
    if healer == target {
        return 0;
    }
    if has(st, target, "jiuyuan") && kingdom(st, healer) == "wu" {
        1
    } else {
        0
    }
}

/// 奸雄：受伤后可以获得造成伤害的牌。
pub fn jianxiong_enabled(st: &SgsState, victim: usize) -> bool {
    has(st, victim, "jianxiong")
}

/// 技能生效事件（便于回放/调试）。
#[allow(dead_code)]
pub fn skill_event(
    st: &SgsState,
    seat: usize,
    code: &str,
    extra: serde_json::Value,
) -> EngineEvent {
    EngineEvent::new(
        "skill_triggered",
        json!({
            "player": st.seats.get(seat).map(|s| s.player_id.clone()),
            "skill": code,
            "detail": extra,
        }),
    )
}
