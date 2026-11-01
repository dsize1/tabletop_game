//! 牌型判定与基准价值（AI 内部用；与 `crate::data::CARD_DEFS` 同源）。
//!
//! 这里的数值只影响 AI 的**取舍**（弃哪张、拿哪张、值不值得用），不参与任何规则判定；
//! 调参只需改本文件，回归口径见 `docs/games/sanguosha/ai.md`。

use crate::data;

/// 【杀】族（含属性杀）。
pub fn is_sha(code: &str) -> bool {
    matches!(code, "sha" | "huo_sha" | "lei_sha")
}

/// 【闪】。
pub fn is_dodge(code: &str) -> bool {
    code == "shan"
}

/// 【桃】。
pub fn is_peach(code: &str) -> bool {
    code == "tao"
}

/// 【无懈可击】。
pub fn is_wuxie(code: &str) -> bool {
    code == "wuxie_keji"
}

/// AOE 锦囊（万箭/南蛮/桃园/五谷）。
pub fn is_aoe(code: &str) -> bool {
    matches!(
        code,
        "wanjian_qifa" | "nanman_ruqin" | "taoyuan_jieyi" | "wugu_fengdeng"
    )
}

/// 延时锦囊。
pub fn is_delayed(code: &str) -> bool {
    data::card_def(code).map(|d| d.delayed).unwrap_or(false)
}

/// 牌名（找不到定义时回退 code）。
pub fn name(code: &str) -> &str {
    data::card_def(code).map(|d| d.name).unwrap_or(code)
}

/// 牌在 AI 眼里的基准价值（0–10；弃牌/选牌/换装权衡用）。
///
/// 口径：桃/无懈这类「救命牌」最高，其次摸牌/拆牌，再是杀闪，装备与延时锦囊居中，
/// 闪电（自伤风险）/铁索最低。
pub fn base_value(code: &str) -> f64 {
    match code {
        "tao" => 10.0,
        "wuxie_keji" => 9.0,
        "wuzhong_shengyou" => 8.0,
        "shunshou_qianyang" => 8.0,
        "shan" => 7.5,
        "guohe_chaiqiao" => 7.0,
        c if is_sha(c) => 6.5,
        "juedou" => 6.0,
        "bagua_zhen" | "renwang_dun" | "baiyin_shizi" => 6.0,
        "lebu_sishu" => 6.0,
        "jiu" => 5.5,
        "bingliang_cunduan" => 5.5,
        "wugu_fengdeng" => 5.5,
        "huo_gong" => 5.5,
        "nanman_ruqin" | "wanjian_qifa" | "taoyuan_jieyi" => 5.0,
        "tiesuo_lianhuan" => 3.0,
        "shandian" => 2.0,
        other => match data::card_def(other) {
            Some(d) if d.equip_slot.is_some() => 4.5,
            _ => 3.5,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_ordering_sane() {
        assert!(base_value("tao") > base_value("sha"));
        assert!(base_value("sha") > base_value("shandian"));
        assert!(base_value("bagua_zhen") > 0.0);
        assert_eq!(name("sha"), "杀");
    }

    #[test]
    fn classifications() {
        assert!(is_sha("huo_sha") && is_sha("lei_sha") && is_sha("sha"));
        assert!(!is_sha("shan"));
        assert!(is_aoe("nanman_ruqin"));
        assert!(is_delayed("lebu_sishu") && is_delayed("shandian"));
        assert!(!is_delayed("sha"));
    }
}
