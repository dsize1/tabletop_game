//! 武将名录（**最小字段**：势力/体力/性别/主公）——选将/大厅切片前的唯一查询入口。
//!
//! 为什么内核需要：庸肆 X（存活不同势力数）、立军（吴势力）、伪帝（当前主公的主公技）等
//! 技能判定读「势力」，不能靠各技能内联字符串。生成物 `generals_generated.rs` 由
//! `npm run gen:sgs-engine-data` 产出，勿手改。
//!
//! 技能 code 映射（武将 → 技能）**不在本切片**：generated 里只有中文技能名，
//! 中文名 → code 的第二份映射表留到选将/大厅切片（避免两处事实源漂移）。

pub use crate::generals_generated::{by_gid, def, GeneralDef, GENERALS};

/// 势力 code（`wei`/`shu`/`wu`/`qun`；未知武将 = `None`）。
pub fn kingdom(code: &str) -> Option<&'static str> {
    def(code).map(|g| g.kingdom)
}

/// 把武将的**最小字段**写进座位：势力 + 体力上限（主公 +1，与 rules-rulings §5.1 一致）。
///
/// 身份由 `turn::begin_game` 分配；本函数只读 `Seat.identity` 判断主公加成，不做身份分配。
/// 返回 `Err` = 未知武将 code（调用方负责报非法）。
pub fn apply_to_seat(g: &mut super::Game, seat: usize, code: &str) -> Result<(), String> {
    let Some(def) = def(code) else {
        return Err(format!("未知武将：{code}"));
    };
    let Some(s) = g.seats.get_mut(seat) else {
        return Err(format!("座位 {seat} 不存在"));
    };
    let lord = s.identity == Some(super::Identity::Lord);
    s.kingdom = def.kingdom.to_string();
    s.max_hp = def.max_hp + if lord { 1 } else { 0 };
    s.hp = s.max_hp;
    Ok(())
}
