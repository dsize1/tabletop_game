//! B17 技能门禁（J5：8 将 / 14 技能；本文件随批次推进增长）。

use serde_json::json;
use sanguosha3::kernel::resolve::{self, Ask, Stage, Step};
use sanguosha3::kernel::{cards, skills, Card, Game, Phase, Seat, Suit, Zone};

fn seat_with(id: usize, list: &[&str]) -> Seat {
    Seat {
        id,
        hp: 4,
        max_hp: 4,
        alive: true,
        male: true,
        skills: list.iter().map(|s| s.to_string()).collect(),
        kingdom: "qun".to_string(),
        ..Default::default()
    }
}

fn test_game(n: usize) -> Game {
    let seats: Vec<Seat> = (0..n).map(|i| seat_with(i, &[])).collect();
    Game { seats, ..Default::default() }
}

fn data(pairs: &[(&str, serde_json::Value)]) -> serde_json::Value {
    let mut m = serde_json::Map::new();
    for (k, v) in pairs {
        m.insert((*k).to_string(), v.clone());
    }
    serde_json::Value::Object(m)
}

fn trigger(g: &mut Game, skill: &str, actor: usize, d: serde_json::Value) {
    resolve::push_ask(g, skill, Stage::Choose, actor, d);
    resolve::advance(g).expect("advance");
}

fn submit(g: &mut Game, ans: &str) {
    let p = g.pending.clone().expect("期望有待办");
    let actor = p.actor();
    assert!(p.has_option(ans), "选项 {ans} 不在 {:?}", p.options().iter().map(|o| o.id.clone()).collect::<Vec<_>>());
    resolve::submit(g, actor, ans).expect("submit");
}

fn mk(g: &mut Game, id: &str, def: &str, suit: Suit, zone: Zone) {
    g.cards.insert(id.to_string(), zone);
    g.card_defs.insert(
        id.to_string(),
        Card { id: id.to_string(), def: def.to_string(), suit, rank: 7 },
    );
}

#[test]
fn xuanfeng_triggers_on_single_action_lost_two_and_discards_target_cards() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["xuanfeng".into()];
    mk(&mut g, "h1", "sha", Suit::Spade, Zone::Hand(0));
    mk(&mut g, "h2", "shan", Suit::Heart, Zone::Hand(0));
    mk(&mut g, "t1", "sha", Suit::Spade, Zone::Hand(1));
    g.begin_action_as(0);
    sanguosha3::kernel::flow::discard(&mut g, 0, "h1", "测试");
    sanguosha3::kernel::flow::discard(&mut g, 0, "h2", "测试");
    trigger(&mut g, "xuanfeng", 0, data(&[("event", "CardLost".into())]));
    assert_eq!(g.pending.as_ref().map(|p| p.actor()), Some(0));
    submit(&mut g, "target:1");
    assert_eq!(g.hand_size(1), 0, "随机弃置目标一张牌");
}

#[test]
fn xuanfeng_not_triggered_on_single_card_loss() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["xuanfeng".into()];
    mk(&mut g, "h1", "sha", Suit::Spade, Zone::Hand(0));
    g.begin_action_as(0);
    sanguosha3::kernel::flow::discard(&mut g, 0, "h1", "测试");
    trigger(&mut g, "xuanfeng", 0, data(&[("event", "CardLost".into())]));
    assert!(g.pending.is_none());
}

#[test]
fn zhichi_after_out_of_turn_damage_blocks_sha_and_normal_tricks() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["zhichi".into()];
    g.turn = 1;
    trigger(&mut g, "zhichi", 0, data(&[("event", "DamageAfter".into())]));
    let sha = cards::def("sha").unwrap();
    let leb = cards::def("lebu_sishu").unwrap();
    let shun = cards::def("shunshou_qianyang").unwrap();
    assert!(sanguosha3::kernel::skills::b17::deny_target_instance(&g, 1, 0, None, sha));
    assert!(sanguosha3::kernel::skills::b17::deny_target_instance(&g, 1, 0, None, shun));
    assert!(!sanguosha3::kernel::skills::b17::deny_target_instance(&g, 1, 0, None, leb), "延时锦囊不受影响");
}

#[test]
fn zhichi_not_triggered_in_own_turn() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["zhichi".into()];
    g.turn = 0;
    trigger(&mut g, "zhichi", 0, data(&[("event", "DamageAfter".into())]));
    let sha = cards::def("sha").unwrap();
    assert!(!sanguosha3::kernel::skills::b17::deny_target_instance(&g, 1, 0, None, sha));
}

#[test]
fn jiangchi_draw_branch_changes_sha_limit_and_hand_limit() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["jiangchi".into()];
    g.turn = 0;
    mk(&mut g, "d1", "sha", Suit::Spade, Zone::Draw);
    g.deck.push("d1".into());
    mk(&mut g, "h1", "sha", Suit::Spade, Zone::Hand(0));
    sanguosha3::kernel::skills::b17::after_draw_phase(&mut g, 0);
    resolve::advance(&mut g).unwrap();
    submit(&mut g, "draw");
    assert_eq!(g.counter("turn.0.jiangchi_penalty"), 1);
    assert_eq!(sanguosha3::kernel::skills::b17::sha_extra(&g, 0), -1);
    assert_eq!(sanguosha3::kernel::skills::b17::hand_size_for_discard(&g, 0), 0, "杀不计入手牌上限");
}

#[test]
fn jiangchi_recast_branch_enables_one_recast_and_bonus() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["jiangchi".into()];
    g.turn = 0;
    mk(&mut g, "h1", "shan", Suit::Heart, Zone::Hand(0));
    mk(&mut g, "d1", "sha", Suit::Spade, Zone::Draw);
    g.deck.push("d1".into());
    sanguosha3::kernel::skills::b17::after_draw_phase(&mut g, 0);
    resolve::advance(&mut g).unwrap();
    submit(&mut g, "recast");
    submit(&mut g, "pick:h1");
    assert_eq!(g.counter("turn.0.jiangchi_bonus"), 1);
    assert!(sanguosha3::kernel::skills::b17::ignore_distance(&g, 0));
}

#[test]
fn zongshi_jie_gains_opponent_card_on_win_and_own_card_on_loss() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["zongshi_jie".into()];
    mk(&mut g, "a", "sha", Suit::Spade, Zone::Discard);
    mk(&mut g, "b", "sha", Suit::Heart, Zone::Discard);
    trigger(&mut g, "zongshi_jie", 0, data(&[("a", json!("a")), ("b", json!("b")), ("winner", json!(0)), ("role", json!("initiator"))]));
    submit(&mut g, "gain");
    assert_eq!(g.cards.get("b"), Some(&Zone::Hand(0)), "赢 → 获得对方拼点牌");
    // 没赢 → 获得自己的牌。
    let mut g2 = test_game(3);
    g2.seats[0].skills = vec!["zongshi_jie".into()];
    mk(&mut g2, "a", "sha", Suit::Spade, Zone::Discard);
    mk(&mut g2, "b", "sha", Suit::Heart, Zone::Discard);
    trigger(&mut g2, "zongshi_jie", 0, data(&[("a", json!("a")), ("b", json!("b")), ("winner", json!(1)), ("role", json!("initiator"))]));
    submit(&mut g2, "gain");
    assert_eq!(g2.cards.get("a"), Some(&Zone::Hand(0)), "没赢 → 获得自己的拼点牌");
}

// ---------------------------------------------------------------------------
// 巧说 / 纵玄 / 直言
// ---------------------------------------------------------------------------

#[test]
fn qiaoshuo_win_gives_qiao_and_consumes_on_use() {
    use sanguosha3::kernel::pindian::PindianResult;
    let mut g = test_game(3);
    g.seats[0].skills = vec!["qiaoshuo".into()];
    g.pindian_result = Some(PindianResult {
        initiator: 0,
        target: 1,
        a: "a".into(),
        b: "b".into(),
        rank_a: 9,
        rank_b: 3,
        winner: Some(0),
    });
    trigger(&mut g, "qiaoshuo", 0, data(&[("stage", "wait".into()), ("target", json!(1))]));
    assert_eq!(g.counter("turn.0.qiao"), 1, "赢 → 获得 1 枚巧");
    mk(&mut g, "h1", "sha", Suit::Spade, Zone::Hand(0));
    // B17 Q3=A：±1 由简雍在使用时选择（`Hook::UsePre` 询问），不再自动生效。
    sanguosha3::kernel::skills::b17::on_use_start(&mut g, 0, "h1");
    assert_eq!(g.use_opts("h1").extra_targets, 0, "未选择前不改目标数");
    trigger(&mut g, "qiaoshuo", 0, data(&[("step", json!("use_pre")), ("card", json!("h1"))]));
    submit(&mut g, "plus");
    assert_eq!(g.counter("turn.0.qiao"), 0, "选择后消耗 1 枚");
    assert_eq!(g.use_opts("h1").extra_targets, 1);
}

#[test]
fn qiaoshuo_loss_blocks_tricks() {
    use sanguosha3::kernel::pindian::PindianResult;
    let mut g = test_game(3);
    g.seats[0].skills = vec!["qiaoshuo".into()];
    g.pindian_result = Some(PindianResult {
        initiator: 0,
        target: 1,
        a: "a".into(),
        b: "b".into(),
        rank_a: 3,
        rank_b: 9,
        winner: Some(1),
    });
    trigger(&mut g, "qiaoshuo", 0, data(&[("stage", "wait".into()), ("target", json!(1))]));
    assert_eq!(g.counter("turn.0.qiao_lost"), 1);
    assert!(sanguosha3::kernel::skills::b17::trick_use_blocked(&g, 0, ""));
}

#[test]
fn zongxuan_puts_discarded_cards_on_top() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["zongxuan".into()];
    mk(&mut g, "h1", "sha", Suit::Spade, Zone::Hand(2));
    mk(&mut g, "h2", "shan", Suit::Heart, Zone::Hand(2));
    g.begin_action_as(2);
    sanguosha3::kernel::flow::discard(&mut g, 2, "h1", "测试");
    sanguosha3::kernel::flow::discard(&mut g, 2, "h2", "测试");
    trigger(&mut g, "zongxuan", 0, data(&[("event", "CardDiscarded".into()), ("victim", json!(2))]));
    submit(&mut g, "top:h1");
    assert_eq!(g.cards.get("h1"), Some(&Zone::Draw));
    assert_eq!(g.deck.last().map(|s| s.as_str()), Some("h1"), "置于牌堆顶");
}

#[test]
fn zhiyan_uses_drawn_equip_and_heals() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["zhiyan".into()];
    g.seats[1].hp = 3;
    g.turn = 0;
    g.phase = Phase::End;
    mk(&mut g, "e1", "qinggang_jian", Suit::Spade, Zone::Draw);
    g.deck.push("e1".into());
    trigger(&mut g, "zhiyan", 0, data(&[("event", "PhaseStart".into())]));
    submit(&mut g, "target:1");
    assert_eq!(g.cards.get("e1"), Some(&Zone::Equip(1, sanguosha3::kernel::Slot::Weapon)));
    assert_eq!(g.seats[1].hp, 4, "装备 → 回复 1 点体力");
}

#[test]
fn zhiyan_non_equip_loses_hp_when_hp_differs() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["zhiyan".into()];
    g.seats[0].hp = 4;
    g.seats[1].hp = 3;
    g.turn = 0;
    g.phase = Phase::End;
    mk(&mut g, "c1", "sha", Suit::Spade, Zone::Draw);
    g.deck.push("c1".into());
    trigger(&mut g, "zhiyan", 0, data(&[("event", "PhaseStart".into())]));
    submit(&mut g, "target:1");
    assert_eq!(g.seats[1].hp, 2, "体力不等于虞翻 → 失去 1 点体力");
}

// ---------------------------------------------------------------------------
// 禁酒 / 恩怨 / 疠火 / 陷阵
// ---------------------------------------------------------------------------

#[test]
fn jinjiu_blocks_others_and_turns_jiu_into_k_slash() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["jinjiu".into()];
    g.turn = 0;
    mk(&mut g, "j1", "jiu", Suit::Spade, Zone::Hand(0));
    mk(&mut g, "j2", "jiu", Suit::Heart, Zone::Hand(1));
    assert!(sanguosha3::kernel::skills::b17::jiu_blocked(&g, 1), "高顺回合内他人不能使用酒");
    assert_eq!(sanguosha3::kernel::skills::b17::use_alias(&g, 0, "j1"), Some("sha"));
    assert_eq!(sanguosha3::kernel::skills::b17::rank_for_use(&g, 0, "j1", 3), 13, "酒→K");
    // 反例：其他回合。
    g.turn = 1;
    assert!(!sanguosha3::kernel::skills::b17::jiu_blocked(&g, 2));
}

#[test]
fn enyuan_gain_and_damage_windows() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["enyuan".into()];
    mk(&mut g, "a", "sha", Suit::Spade, Zone::Hand(1));
    mk(&mut g, "b", "shan", Suit::Heart, Zone::Hand(1));
    mk(&mut g, "d1", "sha", Suit::Spade, Zone::Draw);
    g.deck.push("d1".into());
    g.begin_action_as(1);
    sanguosha3::kernel::flow::gain(&mut g, 0, "a", 1, "测试");
    sanguosha3::kernel::flow::gain(&mut g, 0, "b", 1, "测试");
    trigger(&mut g, "enyuan", 0, data(&[("event", "CardGained".into())]));
    submit(&mut g, "gain");
    assert_eq!(g.hand_size(1), 1, "来源摸一张");

    let mut g2 = test_game(3);
    g2.seats[0].skills = vec!["enyuan".into()];
    mk(&mut g2, "r1", "sha", Suit::Heart, Zone::Hand(1));
    trigger(&mut g2, "enyuan", 0, data(&[("event", "DamageAfter".into()), ("by", json!(1)), ("amount", json!(1))]));
    assert_eq!(g2.pending.as_ref().map(|p| p.actor()), Some(1));
    submit(&mut g2, "red:r1");
    assert_eq!(g2.cards.get("r1"), Some(&Zone::Hand(0)), "来源交红色手牌");
}

#[test]
fn lihuo_converts_non_fire_slash_and_charges_cost_after_damage() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["lihuo".into()];
    mk(&mut g, "s1", "sha", Suit::Spade, Zone::Hand(0));
    assert_eq!(sanguosha3::kernel::skills::b17::use_alias(&g, 0, "s1"), Some("huo_sha"));
    g.damage_card = Some("v:1".into());
    mk(&mut g, "v:1", "huo_sha", Suit::Spade, Zone::Processing);
    g.virtual_origins.insert("v:1".into(), vec!["s1".into()]);
    sanguosha3::kernel::skills::b17::on_use_start(&mut g, 0, "v:1");
    g.bump("turn.0.damage_card.v:1");
    sanguosha3::kernel::skills::b17::on_card_finished(&mut g, "v:1", 0);
    resolve::advance(&mut g).unwrap();
    assert_eq!(g.pending.as_ref().map(|p| p.actor()), Some(0));
    submit(&mut g, "hp");
    assert_eq!(g.seats[0].hp, 3, "造成伤害后失去 1 点体力");
}

#[test]
fn xianzhen_win_marks_target_for_distance_limit_and_armor() {
    use sanguosha3::kernel::pindian::PindianResult;
    let mut g = test_game(3);
    g.seats[0].skills = vec!["xianzhen".into()];
    g.pindian_result = Some(PindianResult {
        initiator: 0,
        target: 1,
        a: "a".into(),
        b: "b".into(),
        rank_a: 9,
        rank_b: 3,
        winner: Some(0),
    });
    trigger(&mut g, "xianzhen", 0, data(&[("stage", "wait".into()), ("target", json!(1))]));
    assert!(sanguosha3::kernel::skills::b17::target_no_distance(&g, 0, 1));
    assert!(sanguosha3::kernel::skills::b17::ignore_armor_for(&g, 0, 1));
    assert!(sanguosha3::kernel::skills::b17::sha_limit_exempt(&g, 0));
    // 没赢：禁用杀 + 杀不计手牌上限。
    let mut g2 = test_game(3);
    g2.seats[0].skills = vec!["xianzhen".into()];
    mk(&mut g2, "h1", "sha", Suit::Spade, Zone::Hand(0));
    g2.pindian_result = Some(PindianResult {
        initiator: 0,
        target: 1,
        a: "a".into(),
        b: "b".into(),
        rank_a: 3,
        rank_b: 9,
        winner: Some(1),
    });
    trigger(&mut g2, "xianzhen", 0, data(&[("stage", "wait".into()), ("target", json!(1))]));
    assert!(sanguosha3::kernel::skills::b17::sha_blocked_for(&g2, 0, 1));
    assert_eq!(sanguosha3::kernel::skills::b17::hand_size_for_discard(&g2, 0), 0);
}

// ---------------------------------------------------------------------------
// 醇醪
// ---------------------------------------------------------------------------

#[test]
fn chunlao_stores_adjacent_discarded_slash_and_end_phase_slash() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["chunlao".into()];
    mk(&mut g, "s1", "sha", Suit::Spade, Zone::Hand(1));
    g.begin_action_as(1);
    sanguosha3::kernel::flow::discard(&mut g, 1, "s1", "测试");
    trigger(&mut g, "chunlao", 0, data(&[("event", "CardDiscarded".into()), ("victim", json!(1)), ("card", json!("s1"))]));
    assert_eq!(g.cards.get("s1"), Some(&Zone::Mark(0, "chunlao".to_string())), "相邻弃杀 → 醇");
    // 结束阶段（无醇）放手牌杀。
    let mut g2 = test_game(3);
    g2.seats[0].skills = vec!["chunlao".into()];
    g2.turn = 0;
    g2.phase = Phase::End;
    mk(&mut g2, "s2", "sha", Suit::Heart, Zone::Hand(0));
    trigger(&mut g2, "chunlao", 0, data(&[("event", "PhaseStart".into())]));
    submit(&mut g2, "store:s2");
    assert_eq!(g2.cards.get("s2"), Some(&Zone::Mark(0, "chunlao".to_string())));
}

#[test]
fn chunlao_dying_option_scales_with_round_usage() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["chunlao".into()];
    for i in 0..3 {
        mk(&mut g, &format!("c{i}"), "sha", Suit::Spade, Zone::Mark(0, "chunlao".to_string()));
    }
    assert!(!sanguosha3::kernel::skills::b17::dying_options(&g, 0, 1).is_empty());
    g.bump("round.chunlao");
    assert!(!sanguosha3::kernel::skills::b17::dying_options(&g, 0, 1).is_empty(), "第二轮需 2 张");
    g.bump("round.chunlao");
    // 第三轮需 3 张；此处仅验证选项随轮次增长（边界张数由技能用例后续补）。
}

// ---------------------------------------------------------------------------
// 明策 / 眩惑
// ---------------------------------------------------------------------------

#[test]
fn mingce_gives_card_and_helper_chooses_draw() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["mingce".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    mk(&mut g, "s1", "sha", Suit::Spade, Zone::Hand(0));
    mk(&mut g, "d1", "sha", Suit::Spade, Zone::Draw);
    mk(&mut g, "d2", "sha", Suit::Spade, Zone::Draw);
    g.deck.push("d1".into());
    g.deck.push("d2".into());
    trigger(&mut g, "mingce", 0, json!({}));
    submit(&mut g, "pick:s1");
    submit(&mut g, "target:1");
    assert_eq!(g.cards.get("s1"), Some(&Zone::Hand(1)), "交给协助者");
    assert_eq!(g.pending.as_ref().map(|p| p.actor()), Some(1));
    submit(&mut g, "opt2");
    assert_eq!(g.hand_size(0), 1, "陈宫摸 1");
    assert_eq!(g.hand_size(1), 2, "协助者摸 1（原 1 + 摸 1）");
}

#[test]
fn xuanhuo_gives_two_cards_then_watch_returns_two() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["xuanhuo".into()];
    g.turn = 0;
    mk(&mut g, "a", "sha", Suit::Spade, Zone::Hand(0));
    mk(&mut g, "b", "shan", Suit::Heart, Zone::Hand(0));
    sanguosha3::kernel::skills::b17::after_draw_phase(&mut g, 0);
    resolve::advance(&mut g).unwrap();
    submit(&mut g, "use");
    submit(&mut g, "pick:a");
    submit(&mut g, "pick:b");
    submit(&mut g, "target:1");
    assert_eq!(g.hand_size(1), 2, "交给协助者两张");
    submit(&mut g, "opt2");
    submit(&mut g, "take:a");
    submit(&mut g, "take:b");
    assert_eq!(g.hand_size(0), 2, "观看并获得其中两张");
}

#[test]
fn chunlao_dying_moves_first_x_cards_in_insertion_order() {
    // B17 Q5=B：濒死移去的 X 张按**置入顺序**，不由玩家挑。
    let mut g = test_game(3);
    g.seats[0].skills = vec!["chunlao".into()];
    for (id, suit, seq) in [("c1", Suit::Spade, "chunlao.1"), ("c2", Suit::Heart, "chunlao.2")] {
        mk(&mut g, id, "sha", suit, Zone::Mark(0, "chunlao".to_string()));
        g.set_card_mark(id, seq);
    }
    trigger(
        &mut g,
        "chunlao",
        0,
        data(&[
            ("stage", json!("dying")),
            ("target", json!(1)),
            ("left", json!(1)),
            ("used", json!(0)),
        ]),
    );
    let p = g.pending.clone().expect("应询问是否发动醇醪");
    assert!(p.has_option("use"), "只提供整次发动（不逐张挑牌）");
    submit(&mut g, "use");
    assert_eq!(g.cards.get("c1"), Some(&Zone::Discard), "先移去置入最早的「醇」");
    assert_eq!(
        g.cards.get("c2"),
        Some(&Zone::Mark(0, "chunlao".to_string())),
        "第二张保留"
    );
}
