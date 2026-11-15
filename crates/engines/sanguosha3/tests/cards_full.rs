//! P4 完整卡牌结算门禁：基本牌 + 普通锦囊 + 延时锦囊（判定区 LIFO + 无懈链）+ 濒死救援。
//!
//! 每张牌都必须结算到「弃牌堆 / 装备区 / 判定区」之一（守恒），并产生事件。

mod common;

use common::{
    add, add_suit, begin, clear_hand, end_turn, game, has_option, pending_actor, start_play,
    submit, use_card,
};
use sanguosha3::kernel::card_play;
use sanguosha3::kernel::event::EventKind;
use sanguosha3::kernel::turn;
use sanguosha3::kernel::{Card, Phase, Suit, Zone};

// ---------------------------------------------------------------------------
// 基本牌
// ---------------------------------------------------------------------------

#[test]
fn tao_heals_and_jiu_boosts_next_slash() {
    let mut g = game(3);
    start_play(&mut g);
    g.seats[0].hp = 2;
    add(&mut g, "tao1", "tao", Zone::Hand(0));
    use_card(&mut g, 0, "tao1").unwrap();
    assert_eq!(g.seats[0].hp, 3);
    assert_eq!(g.cards.get("tao1"), Some(&Zone::Discard));

    // 酒：本回合下一张杀 +1（消费即清空）。
    add(&mut g, "jiu1", "jiu", Zone::Hand(0));
    add(&mut g, "sha1", "sha", Zone::Hand(0));
    g.seats[1].hp = 4;
    use_card(&mut g, 0, "jiu1").unwrap();
    assert_eq!(g.sha_bonus, 1);
    use_card(&mut g, 0, "sha1").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    assert_eq!(g.seats[1].hp, 2, "酒杀 2 点");
    assert_eq!(g.sha_bonus, 0, "酒加成已消费");
}

#[test]
fn dying_asks_for_tao_and_survives() {
    let mut g = game(3);
    start_play(&mut g);
    g.seats[1].hp = 1;
    add(&mut g, "s0", "sha", Zone::Hand(0));
    add(&mut g, "tao1", "tao", Zone::Hand(1));
    use_card(&mut g, 0, "s0").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    // 濒死：先问濒死者本人（有桃）→ 可自救。
    assert_eq!(g.pending.as_ref().unwrap().kind(), "dying");
    assert!(has_option(&g, "card:tao1"));
    submit(&mut g, 1, "card:tao1").unwrap();
    assert!(g.seats[1].alive, "被桃救回");
    assert_eq!(g.seats[1].hp, 1, "桃回到 1");
    assert!(g.winner.is_none());
}

#[test]
fn dying_without_tao_settles_death_and_winner() {
    let mut g = game(2);
    start_play(&mut g);
    g.seats[1].hp = 1;
    clear_hand(&mut g, 0);
    clear_hand(&mut g, 1);
    add(&mut g, "s0", "sha", Zone::Hand(0));
    use_card(&mut g, 0, "s0").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    // 两人都没有桃 → 逐个 pass → 死亡结算 → 主忠胜（反贼全灭）。
    while g
        .pending
        .as_ref()
        .map(|p| p.kind() == "dying")
        .unwrap_or(false)
    {
        let seat = pending_actor(&g).unwrap();
        submit(&mut g, seat, "pass").unwrap();
    }
    assert!(!g.seats[1].alive);
    assert_eq!(g.winner, Some(sanguosha3::kernel::Winner::Lord));
    assert!(g.log.iter().any(|e| e.kind == EventKind::Death));
}

#[test]
fn jiu_can_self_rescue_when_dying() {
    let mut g = game(3);
    start_play(&mut g);
    g.seats[1].hp = 1;
    add(&mut g, "s0", "sha", Zone::Hand(0));
    add(&mut g, "jiu1", "jiu", Zone::Hand(1));
    use_card(&mut g, 0, "s0").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    assert!(has_option(&g, "card:jiu1"), "濒死者可用酒自救");
    submit(&mut g, 1, "card:jiu1").unwrap();
    assert_eq!(g.seats[1].hp, 1, "酒自救回到 1");
}

// ---------------------------------------------------------------------------
// 普通锦囊
// ---------------------------------------------------------------------------

#[test]
fn exnihilo_draws_two_from_deck() {
    let mut g = game(3);
    start_play(&mut g);
    add(&mut g, "wz", "wuzhong_shengyou", Zone::Hand(0));
    let before = g.hand_size(0);
    use_card(&mut g, 0, "wz").unwrap();
    assert_eq!(g.hand_size(0), before - 1 + 2);
}

#[test]
fn snatch_gains_target_card() {
    let mut g = game(3);
    start_play(&mut g);
    add(&mut g, "ss", "shunshou_qianyang", Zone::Hand(0));
    add(&mut g, "h1", "tao", Zone::Hand(1));
    let hand1 = g.hand_size(1);
    use_card(&mut g, 0, "ss").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    submit(&mut g, 0, "c:h1").unwrap();
    assert_eq!(g.cards.get("h1"), Some(&Zone::Hand(0)), "顺手牵羊：获得该牌");
    assert_eq!(g.hand_size(1), hand1 - 1);
    assert!(g.log.iter().any(|e| e.kind == EventKind::CardGained));
}

#[test]
fn duel_alternates_slash_until_someone_fails() {
    let mut g = game(3);
    start_play(&mut g);
    clear_hand(&mut g, 0);
    clear_hand(&mut g, 1);
    add(&mut g, "jd", "juedou", Zone::Hand(0));
    add(&mut g, "sha1", "sha", Zone::Hand(1));
    // 座位 0 无杀 → 座位 1 出杀后，座位 0 不出 → 座位 0 受伤（来源 = 座位 1）。
    use_card(&mut g, 0, "jd").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    assert_eq!(pending_actor(&g), Some(1));
    assert!(has_option(&g, "s:sha1"));
    submit(&mut g, 1, "s:sha1").unwrap();
    // 座位 0 手里没有【杀】→ 合法集为空 → 不询问，直接判负受伤。
    assert_eq!(g.seats[0].hp, 3, "决斗失败者（座位 0）受伤");
    let dmg = g
        .log
        .iter()
        .find(|e| e.kind == EventKind::DamageAfter)
        .expect("伤害事件");
    assert_eq!(dmg.by, Some(1), "来源 = 对方");
    assert_eq!(dmg.damage.unwrap().source, sanguosha3::kernel::SourceKind::Duel);
}

#[test]
fn nanman_requires_slash_and_hits_others() {
    let mut g = game(3);
    start_play(&mut g);
    clear_hand(&mut g, 1);
    clear_hand(&mut g, 2);
    add(&mut g, "nm", "nanman_ruqin", Zone::Hand(0));
    add(&mut g, "sha1", "sha", Zone::Hand(1));
    use_card(&mut g, 0, "nm").unwrap();
    // 目标按座位序从使用者下家开始：座位 1 先问。
    assert_eq!(pending_actor(&g), Some(1));
    submit(&mut g, 1, "p:sha1").unwrap();
    // 座位 2 没有杀 → 直接受伤。
    assert_eq!(g.seats[1].hp, 4);
    assert_eq!(g.seats[2].hp, 3);
    let dmg = g.log.iter().find(|e| e.kind == EventKind::DamageAfter).unwrap();
    assert_eq!(dmg.damage.unwrap().source, sanguosha3::kernel::SourceKind::Nanman);
}

#[test]
fn archery_requires_shan() {
    let mut g = game(3);
    start_play(&mut g);
    clear_hand(&mut g, 1);
    clear_hand(&mut g, 2);
    add(&mut g, "wj", "wanjian_qifa", Zone::Hand(0));
    add(&mut g, "sh1", "shan", Zone::Hand(1));
    use_card(&mut g, 0, "wj").unwrap();
    assert_eq!(pending_actor(&g), Some(1));
    submit(&mut g, 1, "p:sh1").unwrap();
    assert_eq!(g.seats[1].hp, 4);
    assert_eq!(g.seats[2].hp, 3, "座位 2 无闪受伤");
    let dmg = g.log.iter().find(|e| e.kind == EventKind::DamageAfter).unwrap();
    assert_eq!(dmg.damage.unwrap().source, sanguosha3::kernel::SourceKind::Wanjian);
}

#[test]
fn peach_garden_heals_everyone() {
    let mut g = game(3);
    start_play(&mut g);
    for s in 0..3 {
        g.seats[s].hp = 2;
    }
    add(&mut g, "ty", "taoyuan_jieyi", Zone::Hand(0));
    use_card(&mut g, 0, "ty").unwrap();
    for s in 0..3 {
        assert_eq!(g.seats[s].hp, 3, "座位 {s} 回 1 点");
    }
}

#[test]
fn harvest_each_player_picks_one() {
    let mut g = game(2);
    start_play(&mut g);
    add(&mut g, "wg", "wugu_fengdeng", Zone::Hand(0));
    // 亮出 2 张（存活人数）。
    add_suit(&mut g, "r1", "tao", Zone::Draw, Suit::Heart, 3);
    add_suit(&mut g, "r2", "shan", Zone::Draw, Suit::Diamond, 4);
    g.deck.push("r1".into());
    g.deck.push("r2".into());
    let (h0, h1) = (g.hand_size(0), g.hand_size(1));
    use_card(&mut g, 0, "wg").unwrap();
    assert_eq!(g.pending.as_ref().unwrap().kind(), "sgs3_harvest");
    assert_eq!(pending_actor(&g), Some(0), "从使用者开始选");
    submit(&mut g, 0, "c:r2").unwrap();
    submit(&mut g, 1, "c:r1").unwrap();
    assert_eq!(g.hand_size(0), h0 - 1 + 1);
    assert_eq!(g.hand_size(1), h1 + 1);
}

#[test]
fn fire_attack_reveal_and_discard_same_suit() {
    let mut g = game(3);
    start_play(&mut g);
    add(&mut g, "hg", "huo_gong", Zone::Hand(0));
    add_suit(&mut g, "h1", "tao", Zone::Hand(1), Suit::Heart, 3);
    add_suit(&mut g, "hd", "tao", Zone::Hand(0), Suit::Heart, 9);
    use_card(&mut g, 0, "hg").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    // 目标展示一张手牌
    assert_eq!(pending_actor(&g), Some(1));
    submit(&mut g, 1, "r:h1").unwrap();
    // 使用者弃同花色手牌 → 1 点火焰伤害
    assert!(has_option(&g, "s:hd"));
    submit(&mut g, 0, "s:hd").unwrap();
    assert_eq!(g.seats[1].hp, 3);
    let dmg = g.log.iter().find(|e| e.kind == EventKind::DamageAfter).unwrap();
    assert_eq!(dmg.damage.unwrap().kind, sanguosha3::kernel::DamageKind::Fire);
}

#[test]
fn collateral_forces_slash_or_weapon_handover() {
    let mut g = game(3);
    start_play(&mut g);
    add(&mut g, "jd", "jiedao_sharen", Zone::Hand(0));
    add(&mut g, "w1", "qinggang_jian", Zone::Equip(1, sanguosha3::kernel::Slot::Weapon));
    add(&mut g, "sha1", "sha", Zone::Hand(1));
    use_card(&mut g, 0, "jd").unwrap();
    // 两个目标：先选持武器者（座位 1），再选被杀目标（座位 2）。
    assert!(!has_option(&g, "t:2"), "第一个目标必须是持武器者");
    assert!(has_option(&g, "t:1"));
    submit(&mut g, 0, "t:1").unwrap();
    submit(&mut g, 0, "t:2").unwrap();
    assert_eq!(pending_actor(&g), Some(1), "问座位 1 是否出杀");
    submit(&mut g, 1, "s:sha1").unwrap();
    // 座位 2 无闪 → 受伤（来源 = 座位 1）
    assert_eq!(g.seats[2].hp, 3);
}

#[test]
fn collateral_hands_over_weapon_when_declining() {
    let mut g = game(3);
    start_play(&mut g);
    add(&mut g, "jd", "jiedao_sharen", Zone::Hand(0));
    add(&mut g, "w1", "qinggang_jian", Zone::Equip(1, sanguosha3::kernel::Slot::Weapon));
    add(&mut g, "sha1", "sha", Zone::Hand(1));
    use_card(&mut g, 0, "jd").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    submit(&mut g, 0, "t:2").unwrap();
    submit(&mut g, 1, "pass").unwrap();
    assert_eq!(g.cards.get("w1"), Some(&Zone::Hand(0)), "不出杀 → 交出武器");
}

#[test]
fn iron_chain_toggles_and_propagates_elemental_damage() {
    let mut g = game(3);
    start_play(&mut g);
    add(&mut g, "tie", "tiesuo_lianhuan", Zone::Hand(0));
    use_card(&mut g, 0, "tie").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    submit(&mut g, 0, "ok").unwrap();
    assert!(g.seats[1].chained);
    // 火杀命中横置角色 → 传导给另一个横置角色，然后全部重置。
    g.seats[2].chained = true;
    add(&mut g, "fs", "huo_sha", Zone::Hand(0));
    use_card(&mut g, 0, "fs").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    assert_eq!(g.seats[1].hp, 3);
    assert_eq!(g.seats[2].hp, 3, "火属性伤害传导");
    assert!(!g.seats[1].chained && !g.seats[2].chained, "传导后重置");
}

// ---------------------------------------------------------------------------
// 延时锦囊（判定区 LIFO + 判定阶段）
// ---------------------------------------------------------------------------

#[test]
fn supply_shortage_skips_draw_phase() {
    let mut g = game(3);
    begin(&mut g);
    add(&mut g, "bing", "bingliang_cunduan", Zone::Judge(0));
    g.judge_stacks.entry(0).or_default().push("bing".into());
    // 判定牌顶：黑桃（非梅花）→ 跳过摸牌阶段。
    add_suit(&mut g, "jt", "sha", Zone::Draw, Suit::Spade, 9);
    g.deck.push("jt".into());
    turn::start(&mut g).unwrap();
    assert_eq!(g.hand_size(0), 4, "兵粮寸断：摸牌阶段被跳过");
    assert_eq!(g.cards.get("bing"), Some(&Zone::Discard));
}

#[test]
fn lightning_deals_thunder_damage_and_moves_on_miss() {
    // 命中：黑桃 2–9 → 3 点雷电伤害。
    let mut g = game(3);
    begin(&mut g);
    add(&mut g, "sd", "shandian", Zone::Judge(0));
    g.judge_stacks.entry(0).or_default().push("sd".into());
    add_suit(&mut g, "jt", "shan", Zone::Draw, Suit::Spade, 5);
    g.deck.push("jt".into());
    turn::start(&mut g).unwrap();
    assert_eq!(g.seats[0].hp, 1, "闪电 3 点雷电伤害（满血 4）");
    assert_eq!(g.cards.get("sd"), Some(&Zone::Discard));

    // 未命中：红桃 → 移到下家判定区。
    let mut g2 = game(3);
    begin(&mut g2);
    add(&mut g2, "sd", "shandian", Zone::Judge(0));
    g2.judge_stacks.entry(0).or_default().push("sd".into());
    add_suit(&mut g2, "jt", "tao", Zone::Draw, Suit::Heart, 9);
    g2.deck.push("jt".into());
    turn::start(&mut g2).unwrap();
    assert_eq!(g2.seats[0].hp, 4);
    assert_eq!(g2.cards.get("sd"), Some(&Zone::Judge(1)), "未生效：移到下家判定区");
}

#[test]
fn delayed_tricks_are_judged_lifo() {
    let mut g = game(3);
    begin(&mut g);
    add(&mut g, "le", "lebu_sishu", Zone::Judge(0));
    add(&mut g, "bing", "bingliang_cunduan", Zone::Judge(0));
    g.judge_stacks.entry(0).or_default().push("le".into());
    g.judge_stacks.entry(0).or_default().push("bing".into());
    // 顶：梅花 → 兵粮不生效；下一张：黑桃 → 乐生效（跳过出牌）。
    add_suit(&mut g, "j2", "sha", Zone::Draw, Suit::Spade, 9);
    g.deck.push("j2".into());
    add_suit(&mut g, "j1", "sha", Zone::Draw, Suit::Club, 3);
    g.deck.push("j1".into());
    turn::start(&mut g).unwrap();
    let revealed: Vec<_> = g
        .log
        .iter()
        .filter(|e| e.kind == EventKind::JudgeRevealed)
        .map(|e| e.card.clone().unwrap())
        .collect();
    assert_eq!(revealed, vec!["j1".to_string(), "j2".to_string()], "后置先判（LIFO）");
    // 兵粮生效（梅花→不生效），乐生效（黑桃→跳过出牌）→ 当前应是弃牌阶段。
    assert_eq!(g.pending.as_ref().unwrap().kind(), "discard_phase");
}

#[test]
fn delayed_trick_needs_wuxie_window_on_placement() {
    let mut g = game(3);
    start_play(&mut g);
    add(&mut g, "le", "lebu_sishu", Zone::Hand(0));
    add(&mut g, "wx1", "wuxie_keji", Zone::Hand(1));
    use_card(&mut g, 0, "le").unwrap();
    // 延时锦囊放置前有无懈窗口：座位 1 可无懈。
    submit(&mut g, 0, "t:1").unwrap();
    assert_eq!(pending_actor(&g), Some(1));
    assert_eq!(g.pending.as_ref().unwrap().kind(), "wuxie");
    submit(&mut g, 1, "w:wx1").unwrap();
    assert_eq!(g.cards.get("le"), Some(&Zone::Discard), "被无懈：进弃牌堆");
    assert!(g.judge_stacks.get(&1).map(|s| s.is_empty()).unwrap_or(true));
}

#[test]
fn equipment_goes_to_equip_zone_and_old_card_to_discard() {
    let mut g = game(3);
    start_play(&mut g);
    add(&mut g, "w1", "qinggang_jian", Zone::Hand(0));
    add(&mut g, "w2", "zhuge_liannu", Zone::Hand(0));
    use_card(&mut g, 0, "w1").unwrap();
    assert!(matches!(g.cards.get("w1"), Some(Zone::Equip(0, _))));
    use_card(&mut g, 0, "w2").unwrap();
    assert!(matches!(g.cards.get("w2"), Some(Zone::Equip(0, _))));
    assert_eq!(g.cards.get("w1"), Some(&Zone::Discard), "同槽旧装备进弃牌堆");
}

#[test]
fn every_card_ends_in_exactly_one_zone() {
    // 守恒抽查：连续出牌后每张牌仍有唯一归属。
    let mut g = game(4);
    start_play(&mut g);
    add(&mut g, "c1", "sha", Zone::Hand(0));
    add(&mut g, "c2", "wuzhong_shengyou", Zone::Hand(0));
    add(&mut g, "c3", "bagua_zhen", Zone::Hand(0));
    add(&mut g, "c4", "nanman_ruqin", Zone::Hand(0));
    use_card(&mut g, 0, "c2").unwrap();
    use_card(&mut g, 0, "c3").unwrap();
    use_card(&mut g, 0, "c4").unwrap();
    use_card(&mut g, 0, "c1").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    assert_eq!(g.check_conservation(), Ok(()));
    assert_eq!(g.check_no_orphan_frames(), Ok(()));
    let _ = card_play::to_discard;
    let _ = end_turn;
    let _ = Phase::Play;
    let _ = Card::clone;
}
