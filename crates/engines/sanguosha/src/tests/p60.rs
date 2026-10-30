//! P4.5 收尾：PlayPhase 装备效果注册表（丈八蛇矛）。

#[test]
fn play_phase_registry_covers_zhangba() {
    let codes: Vec<&str> = crate::resolve_skill::PLAY_PHASE_HANDLERS
        .iter()
        .map(|h| h.code)
        .collect();
    assert_eq!(codes, vec!["zhangba_shemao", "zhuque_yushan"]);
    for code in &codes {
        let def = crate::skills::EQUIPMENT_SKILLS
            .iter()
            .find(|d| d.code == *code)
            .unwrap_or_else(|| panic!("{code} 应已登记"));
        assert_eq!(def.trigger, crate::skills::Trigger::PlayPhase);
    }
}
