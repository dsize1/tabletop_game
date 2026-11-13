//! P4.5 阶段三（续）：DamageBefore / TargetChosen 效果注册表。

#[test]
fn damage_before_registry_covers_hanbing() {
    let codes: Vec<&str> = crate::resolve_skill::DAMAGE_BEFORE_HANDLERS
        .iter()
        .map(|h| h.code)
        .collect();
    assert_eq!(codes, vec!["hanbing_jian"]);
    let def = crate::skills::EQUIPMENT_SKILLS
        .iter()
        .find(|d| d.code == "hanbing_jian")
        .expect("寒冰剑应已登记");
    assert_eq!(def.trigger, crate::skills::Trigger::DamageBefore);
}

#[test]
fn target_chosen_registry_covers_cixiong() {
    let codes: Vec<&str> = crate::resolve_skill::TARGET_CHOSEN_HANDLERS
        .iter()
        .map(|h| h.code)
        .collect();
    assert_eq!(codes, vec!["cixiong_shuanggujian"]);
    let def = crate::skills::EQUIPMENT_SKILLS
        .iter()
        .find(|d| d.code == "cixiong_shuanggujian")
        .expect("雌雄双股剑应已登记");
    assert_eq!(def.trigger, crate::skills::Trigger::TargetChosen);
}
