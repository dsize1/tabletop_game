//! P5 装备结算门禁：武器 12 特效 + 防具 4 + 坐骑距离（特效 = 特殊技能注入）。
//!
//! 口径：所有特效都只读 `gear::weapon_of` / `gear::armor_of`（内核零牌名）。

mod common;

use common::{
    add, add_suit, clear_hand, game, has_option, pending_actor, refresh, start_play, submit, use_card,
    use_card_fire,
};
use sanguosha3::kernel::event::EventKind;
use sanguosha3::kernel::{gear, DamageKind, Slot, Suit, WeaponKind, Zone};

fn equip(g: &mut sanguosha3::kernel::Game, seat: usize, id: &str, def: &str) {
    let code = sanguosha3::kernel::cards::def(def)
        .map(|d| d.slot)
        .flatten()
        .unwrap_or(Slot::Weapon);
    add(g, id, def, Zone::Equip(seat, code));
}

fn clear_all(g: &mut sanguosha3::kernel::Game) {
    for s in 0..g.seats.len() {
        clear_hand(g, s);
    }
}

// ---------------------------------------------------------------------------
// 武器
// ---------------------------------------------------------------------------

#[test]
fn weapon_kinds_are_read_from_data_not_names() {
    let mut g = game(2);
    start_play(&mut g);
    equip(&mut g, 0, "w1", "qinggang_jian");
    assert_eq!(gear::weapon_of(&g, 0), WeaponKind::Qinggang);
    assert_eq!(gear::attack_range(&g, 0), 3, "1 + 武器范围 2");
    assert!(gear::ignores_armor(&g, 0));
    equip(&mut g, 1, "a1", "bagua_zhen");
    assert_eq!(gear::armor_of(&g, 1), sanguosha3::kernel::ArmorKind::Bagua);
}

#[test]
fn qinggang_ignores_tengjia() {
    let mut g = game(3);
    start_play(&mut g);
    clear_all(&mut g);
    equip(&mut g, 1, "tj", "tengjia");
    add(&mut g, "s1", "sha", Zone::Hand(0));
    // 无青釭：普通杀被藤甲免疫。
    use_card(&mut g, 0, "s1").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    assert_eq!(g.seats[1].hp, 4, "藤甲免疫普通杀");
    // 有青釭：无视防具，造成 1 点伤害（新出牌阶段重置【杀】次数）。
    g.set_counter("phase.sha_used.0", 0);
    equip(&mut g, 0, "qg", "qinggang_jian");
    add(&mut g, "s2", "sha", Zone::Hand(0));
    use_card(&mut g, 0, "s2").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    assert_eq!(g.seats[1].hp, 3, "青釭剑无视藤甲");
}

#[test]
fn zhugeliannu_lifts_slash_limit() {
    let mut g = game(3);
    start_play(&mut g);
    clear_all(&mut g);
    add(&mut g, "s1", "sha", Zone::Hand(0));
    add(&mut g, "s2", "sha", Zone::Hand(0));
    use_card(&mut g, 0, "s1").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    // 第二张杀：无连弩 → 拒绝。
    let err = use_card(&mut g, 0, "s2").unwrap_err();
    assert!(err.contains("次数已用尽"), "{err}");
    // 装备连弩 → 无限杀。
    equip(&mut g, 0, "zg", "zhuge_liannu");
    use_card(&mut g, 0, "s2").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    assert_eq!(g.seats[1].hp, 2, "连弩：两张杀都命中");
}

#[test]
fn zhangba_virtual_slash_from_two_cards() {
    let mut g = game(3);
    start_play(&mut g);
    clear_all(&mut g);
    equip(&mut g, 0, "zb", "zhangba_shemao");
    add(&mut g, "c1", "tao", Zone::Hand(0));
    add(&mut g, "c2", "shan", Zone::Hand(0));
    g.pending = None;
    sanguosha3::kernel::card_play::begin_use_zhangba(&mut g, 0, "c1", "c2").unwrap();
    sanguosha3::kernel::resolve::advance(&mut g).unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    assert_eq!(g.seats[1].hp, 3, "丈八：两张手牌当【杀】");
    assert_eq!(g.cards.get("c1"), Some(&Zone::Discard));
    assert_eq!(g.cards.get("c2"), Some(&Zone::Discard));
    // 虚拟牌落到 Void（守恒：实例与区域同时存在）。
    let virtuals: Vec<_> = g
        .cards
        .iter()
        .filter(|(id, _)| id.starts_with("v:"))
        .collect();
    assert!(!virtuals.is_empty(), "虚拟杀实例存在");
    for (id, zone) in virtuals {
        assert_eq!(zone, &Zone::Void, "虚拟牌结算后进 Void");
        assert!(g.card_defs.contains_key(id), "虚拟牌定义仍在（守恒）");
    }
    assert_eq!(g.check_conservation(), Ok(()));
}

#[test]
fn fangtian_allows_three_targets_with_last_hand_card() {
    let mut g = game(4);
    start_play(&mut g);
    clear_all(&mut g);
    equip(&mut g, 0, "ft", "fangtian_huaji");
    add(&mut g, "s1", "sha", Zone::Hand(0));
    use_card(&mut g, 0, "s1").unwrap();
    // 最后一张手牌 → 最多 3 个目标
    submit(&mut g, 0, "t:1").unwrap();
    submit(&mut g, 0, "t:2").unwrap();
    submit(&mut g, 0, "t:3").unwrap();
    for s in 1..4 {
        assert_eq!(g.seats[s].hp, 3, "方天画戟：多目标各受 1 伤");
    }
}

#[test]
fn gudingdao_bonus_on_empty_hand_target() {
    let mut g = game(3);
    start_play(&mut g);
    clear_all(&mut g);
    equip(&mut g, 0, "gd", "guding_dao");
    add(&mut g, "s1", "sha", Zone::Hand(0));
    use_card(&mut g, 0, "s1").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    assert_eq!(g.seats[1].hp, 2, "古锭刀：目标无手牌 +1");
}

#[test]
fn zhuque_converts_normal_slash_to_fire() {
    let mut g = game(3);
    start_play(&mut g);
    clear_all(&mut g);
    equip(&mut g, 0, "zq", "zhuque_yushan");
    equip(&mut g, 1, "tj", "tengjia");
    add(&mut g, "s1", "sha", Zone::Hand(0));
    use_card_fire(&mut g, 0, "s1").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    // 火杀打藤甲：1 + 藤甲 +1 = 2。
    assert_eq!(g.seats[1].hp, 2, "朱雀羽扇：普杀转火杀（藤甲 +1）");
    let dmg = g.log.iter().find(|e| e.kind == EventKind::DamageAfter).unwrap();
    assert_eq!(dmg.damage.unwrap().kind, DamageKind::Fire);
}

#[test]
fn qilin_discards_horse_after_damage() {
    let mut g = game(3);
    start_play(&mut g);
    clear_all(&mut g);
    equip(&mut g, 0, "ql", "qilin_gong");
    equip(&mut g, 1, "horse", "chitu");
    add(&mut g, "s1", "sha", Zone::Hand(0));
    use_card(&mut g, 0, "s1").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    assert_eq!(g.seats[1].hp, 3);
    assert_eq!(g.pending.as_ref().unwrap().kind(), "qilin_opt");
    assert!(has_option(&g, "q:horse"));
    submit(&mut g, 0, "q:horse").unwrap();
    assert_eq!(g.cards.get("horse"), Some(&Zone::Discard), "麒麟弓弃坐骑");
}

#[test]
fn hanbing_prevents_damage_and_discards_two() {
    let mut g = game(3);
    start_play(&mut g);
    clear_all(&mut g);
    equip(&mut g, 0, "hb", "hanbing_jian");
    add(&mut g, "s1", "sha", Zone::Hand(0));
    add(&mut g, "h1", "tao", Zone::Hand(1));
    add(&mut g, "h2", "tao", Zone::Hand(1));
    use_card(&mut g, 0, "s1").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    assert_eq!(g.pending.as_ref().unwrap().kind(), "hanbing_opt");
    assert!(has_option(&g, "hanbing"));
    submit(&mut g, 0, "hanbing").unwrap();
    let first = g.pending.as_ref().unwrap();
    assert_eq!(first.kind(), "hanbing_pick");
    submit(&mut g, 0, "c:h1").unwrap();
    submit(&mut g, 0, "c:h2").unwrap();
    assert_eq!(g.seats[1].hp, 4, "寒冰剑：防止伤害");
    assert_eq!(g.cards.get("h1"), Some(&Zone::Discard));
    assert_eq!(g.cards.get("h2"), Some(&Zone::Discard));
}

#[test]
fn qinglong_uses_another_slash_after_shan() {
    let mut g = game(3);
    start_play(&mut g);
    clear_all(&mut g);
    equip(&mut g, 0, "ql", "qinglong_yanyue");
    add(&mut g, "s1", "sha", Zone::Hand(0));
    add(&mut g, "s2", "sha", Zone::Hand(0));
    add(&mut g, "sh1", "shan", Zone::Hand(1));
    use_card(&mut g, 0, "s1").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    submit(&mut g, 1, "s:sh1").unwrap();
    // 被闪抵消 → 青龙偃月刀：可再使用一张杀
    assert_eq!(g.pending.as_ref().unwrap().kind(), "sha_blocked");
    assert!(has_option(&g, "again:s2"));
    submit(&mut g, 0, "again:s2").unwrap();
    // 目标已无闪 → 直接命中
    assert_eq!(g.seats[1].hp, 3);
    assert_eq!(g.cards.get("s2"), Some(&Zone::Discard));
}

#[test]
fn guanshi_discards_two_to_force_damage() {
    let mut g = game(3);
    start_play(&mut g);
    clear_all(&mut g);
    equip(&mut g, 0, "gs", "guanshi_fu");
    add(&mut g, "s1", "sha", Zone::Hand(0));
    add(&mut g, "c1", "tao", Zone::Hand(0));
    add(&mut g, "c2", "tao", Zone::Hand(0));
    add(&mut g, "sh1", "shan", Zone::Hand(1));
    use_card(&mut g, 0, "s1").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    submit(&mut g, 1, "s:sh1").unwrap();
    assert!(has_option(&g, "guanshi"));
    submit(&mut g, 0, "guanshi").unwrap();
    submit(&mut g, 0, "discard:c1").unwrap();
    submit(&mut g, 0, "discard:c2").unwrap();
    assert_eq!(g.seats[1].hp, 3, "贯石斧：弃两张牌令杀命中");
}

// ---------------------------------------------------------------------------
// 防具
// ---------------------------------------------------------------------------

#[test]
fn bagua_judges_red_as_shan() {
    let mut g = game(3);
    start_play(&mut g);
    clear_all(&mut g);
    equip(&mut g, 1, "bg", "bagua_zhen");
    add(&mut g, "s1", "sha", Zone::Hand(0));
    // 牌堆顶红桃 → 八卦阵视为【闪】。
    add_suit(&mut g, "jt", "tao", Zone::Draw, Suit::Heart, 9);
    g.deck.push("jt".into());
    use_card(&mut g, 0, "s1").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    assert_eq!(pending_actor(&g), Some(1), "问目标是否发动八卦阵/出闪");
    assert!(has_option(&g, "bagua"));
    submit(&mut g, 1, "bagua").unwrap();
    assert_eq!(g.seats[1].hp, 4, "八卦阵红判定 = 视为闪");
    assert!(g.log.iter().any(|e| e.kind == EventKind::JudgeRevealed));
}

#[test]
fn bagua_black_judgement_still_allows_shan() {
    let mut g = game(3);
    start_play(&mut g);
    clear_all(&mut g);
    equip(&mut g, 1, "bg", "bagua_zhen");
    add(&mut g, "s1", "sha", Zone::Hand(0));
    add(&mut g, "sh1", "shan", Zone::Hand(1));
    add_suit(&mut g, "jt", "sha", Zone::Draw, Suit::Spade, 9);
    g.deck.push("jt".into());
    use_card(&mut g, 0, "s1").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    submit(&mut g, 1, "bagua").unwrap();
    // 黑判定 → 八卦无效，但仍可打出【闪】。
    assert_eq!(g.pending.as_ref().unwrap().kind(), "shan_ask");
    assert!(has_option(&g, "s:sh1"));
    submit(&mut g, 1, "s:sh1").unwrap();
    assert_eq!(g.seats[1].hp, 4);
}

#[test]
fn renwang_voids_black_slash_only() {
    let mut g = game(3);
    start_play(&mut g);
    clear_all(&mut g);
    equip(&mut g, 1, "rw", "renwang_dun");
    add_suit(&mut g, "sb", "sha", Zone::Hand(0), Suit::Spade, 7);
    use_card(&mut g, 0, "sb").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    assert_eq!(g.seats[1].hp, 4, "仁王盾：黑杀无效");
    // 红杀照常结算（新出牌阶段重置【杀】次数）
    g.set_counter("phase.sha_used.0", 0);
    add_suit(&mut g, "sr", "sha", Zone::Hand(0), Suit::Heart, 7);
    use_card(&mut g, 0, "sr").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    assert_eq!(g.seats[1].hp, 3, "仁王盾不挡红杀");
}

#[test]
fn baiyin_caps_damage_and_heals_on_leave() {
    let mut g = game(3);
    start_play(&mut g);
    clear_all(&mut g);
    equip(&mut g, 1, "by", "baiyin_shizi");
    g.seats[1].hp = 2;
    // 酒杀（2 点）→ 白银狮子把伤害压到 1。
    add(&mut g, "jiu", "jiu", Zone::Hand(0));
    add(&mut g, "sha", "sha", Zone::Hand(0));
    use_card(&mut g, 0, "jiu").unwrap();
    use_card(&mut g, 0, "sha").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    assert_eq!(g.seats[1].hp, 1, "白银狮子：>1 的伤害改为 1");
    let dmg = g.log.iter().find(|e| e.kind == EventKind::DamageAfter).unwrap();
    assert_eq!(dmg.damage.unwrap().amount, 1);
    // 离开装备区 → 回 1 点体力（被顶掉）。
    add(&mut g, "bg2", "bagua_zhen", Zone::Hand(1));
    g.pending = None;
    g.phase = sanguosha3::kernel::Phase::Play;
    use_card_direct(&mut g, 1, "bg2");
    assert!(g.seats[1].hp >= 2, "白银狮子离开装备区回 1 体力");
}

fn use_card_direct(g: &mut sanguosha3::kernel::Game, seat: usize, id: &str) {
    sanguosha3::kernel::card_play::begin_use(g, seat, id, false).unwrap();
    sanguosha3::kernel::resolve::advance(g).unwrap();
}

#[test]
fn horse_modifiers_change_distance() {
    let mut g = game(4);
    start_play(&mut g);
    assert_eq!(gear::distance(&g, 0, 2), 2);
    equip(&mut g, 0, "o", "chitu");
    assert_eq!(gear::distance(&g, 0, 2), 1, "进攻马 -1");
    equip(&mut g, 2, "d", "jueying");
    assert_eq!(gear::distance(&g, 0, 2), 2, "防御马 +1（与进攻马抵消）");
    assert_eq!(gear::distance(&g, 0, 3), 1);
}

#[test]
fn armor_change_emits_event() {
    let mut g = game(3);
    start_play(&mut g);
    clear_all(&mut g);
    add(&mut g, "a1", "bagua_zhen", Zone::Hand(0));
    use_card(&mut g, 0, "a1").unwrap();
    assert!(g.log.iter().any(|e| e.kind == EventKind::ArmorChanged));
    assert!(matches!(g.cards.get("a1"), Some(Zone::Equip(0, Slot::Armor))));
}

#[test]
fn zhangba_option_runs_through_play_phase_with_cancel_support() {
    let mut g = game(3);
    start_play(&mut g);
    clear_all(&mut g);
    equip(&mut g, 0, "zb", "zhangba_shemao");
    add(&mut g, "c1", "tao", Zone::Hand(0));
    add(&mut g, "c2", "shan", Zone::Hand(0));
    refresh(&mut g);
    assert!(has_option(&g, "zhangba"), "装备丈八时出牌阶段应给「两牌当杀」入口");
    submit(&mut g, 0, "zhangba").unwrap();
    assert_eq!(g.pending.as_ref().unwrap().kind(), "zhangba_pick");
    // 取消：清中间态并回到出牌阶段（不会卡在选牌里）。
    submit(&mut g, 0, "pass").unwrap();
    assert_eq!(g.pending.as_ref().unwrap().kind(), "play_act");
    assert!(has_option(&g, "zhangba"));
    // 再走一遍并真正使用
    submit(&mut g, 0, "zhangba").unwrap();
    submit(&mut g, 0, "zbc:c1").unwrap();
    submit(&mut g, 0, "zbc:c2").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    assert_eq!(g.seats[1].hp, 3, "丈八：两张手牌当【杀】命中");
    assert_eq!(g.check_conservation(), Ok(()));
    assert_eq!(g.check_no_orphan_frames(), Ok(()));
}

#[test]
fn zhangba_target_cancel_keeps_conservation() {
    let mut g = game(3);
    start_play(&mut g);
    clear_all(&mut g);
    equip(&mut g, 0, "zb", "zhangba_shemao");
    add(&mut g, "c1", "tao", Zone::Hand(0));
    add(&mut g, "c2", "shan", Zone::Hand(0));
    g.pending = None;
    sanguosha3::kernel::card_play::begin_use_zhangba(&mut g, 0, "c1", "c2").unwrap();
    sanguosha3::kernel::resolve::advance(&mut g).unwrap();
    submit(&mut g, 0, "cancel").unwrap();
    assert_eq!(g.seats[1].hp, 4, "取消使用：不造成伤害");
    assert_eq!(g.check_conservation(), Ok(()), "守恒：虚拟牌落 Void");
    assert_eq!(g.check_no_orphan_frames(), Ok(()));
}
