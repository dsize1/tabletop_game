//! 主公技「他人侧」注册表（**唯一来源**，2026-11-11 用户裁定）。
//!
//! 主公技不是持有者的私有实现：同势力角色按各自模式参与。以前只有一条硬编码白名单
//! （`LORD_SKILLS_USED_BY_OTHERS = ["huangtian"]`），导致「制霸」的他人挑战一直没有入口。
//! 现在每个 `kind: Lord` 的技能都必须在下表声明模式（漏声明 → `cargo test` 红）。
//!
//! 四种模式：
//! - `OthersActive`：同势力他人**自己出牌阶段**可以主动发动（黄天 / 制霸挑战）；
//! - `OthersTrigger`：同势力他人满足条件时触发，持有者是结算对象（颂威/暴虐/立军/兴衰）——
//!   实现上仍由持有者订阅、handler 内判他人条件，这里只是声明 + 闸门；
//! - `HolderRequest`：持有者发起、同势力他人响应（激将/护驾，走 relay 通道）；
//! - `HolderOnly`：只看持有者（救援/勤王/诏缚/血裔/若愚）。
//!
//! 口径（已裁定）：
//! 1. 他人侧要求 `same_kingdom(使用者, 持有者)`；
//! 2. **伪帝借用的主公技不开放他人侧**（只认真实持有者）；
//! 3. 持有者必须存活。

use crate::kernel::Game;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LordSide {
    OthersActive,
    OthersTrigger,
    HolderRequest,
    HolderOnly,
}

impl LordSide {
    pub fn label(self) -> &'static str {
        match self {
            LordSide::OthersActive => "others-active",
            LordSide::OthersTrigger => "others-trigger",
            LordSide::HolderRequest => "holder-request",
            LordSide::HolderOnly => "holder-only",
        }
    }
}

/// 全部主公技的他人侧模式（与 `SKILLS` 双向校验，见文件尾测试）。
pub const TABLE: &[(&str, LordSide)] = &[
    // 他人主动（同势力角色在自己出牌阶段发动）
    ("huangtian", LordSide::OthersActive),
    ("zhiba", LordSide::OthersActive),
    // 持有者发起 / 他人响应
    ("jijiang", LordSide::HolderRequest),
    ("hujia", LordSide::HolderRequest),
    // 他人触发（同势力他人满足条件 → 持有者结算）
    ("songwei", LordSide::OthersTrigger),
    ("baonue", LordSide::OthersTrigger),
    ("lijun", LordSide::OthersTrigger),
    ("xingshuai", LordSide::OthersTrigger),
    // 只看持有者
    ("jiuyuan", LordSide::HolderOnly),
    ("qinwang", LordSide::HolderOnly),
    ("zhaofu", LordSide::HolderOnly),
    ("xueyi", LordSide::HolderOnly),
    ("ruoyu", LordSide::HolderOnly),
];

/// 查模式；未登记的 code 按「只有持有者」处理（覆盖校验由测试保证）。
pub fn others_side(code: &str) -> LordSide {
    TABLE
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, side)| *side)
        .unwrap_or(LordSide::HolderOnly)
}

/// 是否放行「同势力他人」的入口（合法动作 / `lord_gate_ok`）。
///
/// `HolderRequest` 的他人响应走 relay（不经过 `legal_actions`），所以不在此列。
pub fn others_may_act(code: &str) -> bool {
    matches!(
        others_side(code),
        LordSide::OthersActive | LordSide::OthersTrigger
    )
}

/// 真实持有者（**不认伪帝借用**）：存活 + 技能在自带或授予里。
pub fn holder_of(g: &Game, code: &str) -> Option<usize> {
    (0..g.seats.len()).find(|i| {
        let s = &g.seats[*i];
        s.hp > 0 && (s.skills.iter().any(|x| x == code) || s.granted.iter().any(|x| x == code))
    })
}

/// 同势力共享：存在真实持有者 `h != seat`、存活、且与 `seat` 同势力。
pub fn shared_with(g: &Game, seat: usize, code: &str) -> bool {
    let Some(h) = holder_of(g, code) else {
        return false;
    };
    h != seat
        && g.seats
            .get(seat)
            .is_some_and(|s| s.hp > 0 && s.kingdom == g.seats[h].kingdom)
}

#[cfg(test)]
mod tests {
    use super::super::{SkillKind, SKILLS};
    use super::*;

    /// 门禁：每个 `kind: Lord` 的注册技能都必须显式声明他人侧模式；表里不得有多余项。
    #[test]
    fn every_lord_skill_declares_others_side() {
        let lords: Vec<&str> = SKILLS
            .iter()
            .filter(|d| d.kind == SkillKind::Lord)
            .map(|d| d.code)
            .collect();
        assert!(!lords.is_empty(), "注册表里没有主公技？");
        for code in &lords {
            assert!(
                TABLE.iter().any(|(c, _)| c == code),
                "主公技 `{code}` 未在 skills/lord.rs 声明他人侧模式"
            );
        }
        for (code, _) in TABLE {
            let def = SKILLS.iter().find(|d| d.code == *code);
            let def = def.unwrap_or_else(|| panic!("lord.rs 表里的 `{code}` 未注册"));
            assert_eq!(
                def.kind,
                SkillKind::Lord,
                "lord.rs 表里的 `{code}` 不是主公技（kind != Lord）"
            );
        }
        assert_eq!(TABLE.len(), lords.len(), "lord.rs 表项数与主公技数不一致");
    }
}
