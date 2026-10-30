//! P4.5：装备技能注册表（SkillOwner/Trigger 阶段一）。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, edit, manager, state_of, take_card};
use crate::skills::{triggered, Trigger, EQUIPMENT_SKILLS};
use crate::state::*;

fn equip(st: &mut SgsState, seat: usize, code: &str) {
    let id = super::find_card(st, code);
    take_card(st, &id);
    let slot = if code == "bagua_zhen" || code == "renwang_dun" {
        EquipSlot::Armor
    } else {
        EquipSlot::Weapon
    };
    st.seats[seat].equips.insert(slot, Some(id));
}

#[test]
fn equipment_registry_covers_standard_set() {
    for code in [
        "zhuge_liannu",
        "cixiong_shuanggujian",
        "qinggang_jian",
        "qinglong_yanyue",
        "zhangba_shemao",
        "guanshi_fu",
        "fangtian_huaji",
        "qilin_gong",
        "hanbing_jian",
        "bagua_zhen",
        "renwang_dun",
    ] {
        assert!(
            EQUIPMENT_SKILLS.iter().any(|d| d.code == code),
            "装备技能注册表缺少 {code}"
        );
    }
    assert!(
        EQUIPMENT_SKILLS.len() >= 17,
        "至少覆盖 9 武器 + 2 防具 + 6 坐骑"
    );
}

#[test]
fn equipped_skill_only_active_when_equipped() {
    let mgr = manager(2200);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_2");
    });
    let st = state_of(&mgr, &room);
    assert!(
        !crate::skills::has_equipment_skill(&st, cur, "bagua_zhen"),
        "未装备时装备技能不生效"
    );
    edit(&mgr, &room, |st| {
        equip(st, cur, "bagua_zhen");
    });
    let st = state_of(&mgr, &room);
    assert!(
        crate::skills::has_equipment_skill(&st, cur, "bagua_zhen"),
        "装备后装备技能生效"
    );
    assert!(
        crate::skills::has(&st, cur, "bagua_zhen") == false,
        "装备技能不混入武将技能"
    );
}

#[test]
fn triggered_after_dodge_is_deterministic() {
    let mgr = manager(2201);
    let room = start_and_pick(&mgr, 5);
    let mut a = 0;
    let mut b = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        a = st.turn;
        b = st.seats_from_left_of(a)[0];
        equip(st, a, "qinglong_yanyue");
        equip(st, b, "guanshi_fu");
        equip(st, b, "bagua_zhen");
    });
    let st = state_of(&mgr, &room);
    let holders = triggered(&st, Trigger::AfterDodge);
    let seats: Vec<usize> = holders.iter().map(|(s, _)| *s).collect();
    assert_eq!(seats, vec![a, b], "座位升序返回");
    assert!(holders
        .iter()
        .all(|(_, d)| d.trigger == Trigger::AfterDodge));
    let judge: Vec<usize> = triggered(&st, Trigger::JudgeBefore)
        .iter()
        .map(|(s, _)| *s)
        .collect();
    assert_eq!(judge, vec![b], "八卦阵属于 JudgeBefore");
}
