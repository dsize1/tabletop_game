//! B16 技能门禁（J4：8 将 / 14 技能；本文件随批次推进增长）。

use serde_json::json;
use sanguosha3::kernel::resolve::{self, Ask, Stage, Step};
use sanguosha3::kernel::{flow, skills, Card, Game, Phase, Seat, Suit, Zone};

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
    assert!(
        p.has_option(ans),
        "选项 {ans} 不在 {:?}",
        p.options().iter().map(|o| o.id.clone()).collect::<Vec<_>>()
    );
    resolve::submit(g, actor, ans).expect("submit");
}

fn mk(g: &mut Game, id: &str, def: &str, suit: Suit, zone: Zone) {
    g.cards.insert(id.to_string(), zone);
    g.card_defs.insert(
        id.to_string(),
        Card { id: id.to_string(), def: def.to_string(), suit, rank: 7 },
    );
}

// ---------------------------------------------------------------------------
// 祸首
// ---------------------------------------------------------------------------

#[test]
fn huoshou_blocks_nanman_and_replaces_damage_source() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["huoshou".into()];
    assert!(
        sanguosha3::kernel::skills::b16::aoe_excluded(&g, 1, 0, "nanman_ruqin"),
        "祸首持有者不能成为南蛮目标"
    );
    assert!(!sanguosha3::kernel::skills::b16::aoe_excluded(&g, 1, 0, "wanjian_qifa"));
    assert_eq!(
        sanguosha3::kernel::skills::b16::aoe_damage_source(&g, 1, "nanman_ruqin"),
        Some(0),
        "其他角色使用的南蛮 → 来源改为祸首持有者"
    );
    // 反例：自己使用不替换；非南蛮不替换。
    assert_eq!(
        sanguosha3::kernel::skills::b16::aoe_damage_source(&g, 0, "nanman_ruqin"),
        None
    );
    assert_eq!(
        sanguosha3::kernel::skills::b16::aoe_damage_source(&g, 1, "wanjian_qifa"),
        None
    );
}

// ---------------------------------------------------------------------------
// 断粮
// ---------------------------------------------------------------------------

#[test]
fn duanliang_converts_black_basic_or_equip_and_no_distance_before_damage() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["duanliang".into()];
    mk(&mut g, "b1", "sha", Suit::Spade, Zone::Hand(0));
    mk(&mut g, "b2", "qinggang_jian", Suit::Club, Zone::Hand(0));
    mk(&mut g, "r1", "sha", Suit::Heart, Zone::Hand(0));
    mk(&mut g, "b3", "shunshou_qianyang", Suit::Spade, Zone::Hand(0));
    assert_eq!(
        sanguosha3::kernel::skills::b16::use_alias(&g, 0, "b1"),
        Some("bingliang_cunduan")
    );
    assert_eq!(
        sanguosha3::kernel::skills::b16::use_alias(&g, 0, "b2"),
        Some("bingliang_cunduan")
    );
    assert_eq!(sanguosha3::kernel::skills::b16::use_alias(&g, 0, "r1"), None);
    assert_eq!(sanguosha3::kernel::skills::b16::use_alias(&g, 0, "b3"), None);
    assert!(sanguosha3::kernel::skills::b16::trick_no_distance(&g, 0, "bingliang_cunduan"));
    g.bump("turn.0.damage_dealt");
    assert!(!sanguosha3::kernel::skills::b16::trick_no_distance(&g, 0, "bingliang_cunduan"), "已造成伤害 → 有距离");
}

// ---------------------------------------------------------------------------
// 再起
// ---------------------------------------------------------------------------

#[test]
fn zaizhi_repeats_per_red_card_to_discard_and_target_chooses() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["zaizhi".into()];
    g.turn = 0;
    g.phase = Phase::End;
    g.seats[0].hp = 3;
    mk(&mut g, "d1", "sha", Suit::Spade, Zone::Draw);
    g.deck.push("d1".into());
    g.bump("turn.0.red_to_discard");
    g.bump("turn.0.red_to_discard");
    trigger(&mut g, "zaizhi", 0, json!({}));
    assert_eq!(g.pending.as_ref().map(|p| p.actor()), Some(0));
    submit(&mut g, "target:1");
    assert_eq!(g.pending.as_ref().map(|p| p.actor()), Some(1), "由被选中的角色二选一");
    let h = g.hand_size(1);
    submit(&mut g, "opt1");
    assert_eq!(g.hand_size(1), h + 1, "摸一张");
    // 第二次：选自己，令孟获回复 1。
    submit(&mut g, "target:0");
    submit(&mut g, "opt2");
    assert_eq!(g.seats[0].hp, 4, "回复 1 点");
    assert!(g.pending.is_none(), "两次结算完毕");
}

#[test]
fn zaizhi_not_triggered_without_red_cards() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["zaizhi".into()];
    g.turn = 0;
    g.phase = Phase::End;
    trigger(&mut g, "zaizhi", 0, json!({}));
    assert!(g.pending.is_none(), "没有红色牌进弃牌堆 → 不询问");
}

// ---------------------------------------------------------------------------
// 截辎
// ---------------------------------------------------------------------------

#[test]
fn jiezi_skip_draw_grants_zai_or_draws_and_extra_draw_consumes_it() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["jiezi".into()];
    // 座位 1 手牌最少（0），座位 2 有 1 张。
    mk(&mut g, "h2", "sha", Suit::Spade, Zone::Hand(2));
    sanguosha3::kernel::skills::b16::on_skip_draw(&mut g, 1);
    resolve::advance(&mut g).unwrap();
    assert_eq!(g.pending.as_ref().map(|p| p.actor()), Some(0));
    submit(&mut g, "target:1");
    assert_eq!(g.seats[1].counters.get("zai").copied(), Some(1), "手牌最少 → 获得辎");
    assert!(
        sanguosha3::kernel::skills::b16::take_extra_draw(&mut g, 1),
        "摸牌阶段结束 → 额外摸牌阶段"
    );
    assert_eq!(g.seats[1].counters.get("zai").copied(), None);
    assert!(!sanguosha3::kernel::skills::b16::take_extra_draw(&mut g, 1), "只执行一次");

    // 反例：目标不是手牌最少 → 改为摸 1。
    let mut g2 = test_game(3);
    g2.seats[0].skills = vec!["jiezi".into()];
    mk(&mut g2, "h2", "sha", Suit::Spade, Zone::Hand(2));
    sanguosha3::kernel::skills::b16::on_skip_draw(&mut g2, 1);
    resolve::advance(&mut g2).unwrap();
    submit(&mut g2, "target:2");
    assert_eq!(g2.seats[2].counters.get("zai").copied(), None, "非最少 → 不给辎");
    assert_eq!(g2.hand_size(2), 1, "改为摸 1");
}

#[test]
fn jiezi_not_offered_without_holder() {
    let mut g = test_game(3);
    sanguosha3::kernel::skills::b16::on_skip_draw(&mut g, 1);
    resolve::advance(&mut g).unwrap();
    assert!(g.pending.is_none());
}

// 让 `Step`/`skills` 导入不被未使用警告（后续技能组会用到）。
#[allow(dead_code)]
fn _keep(_s: Step) {
    let _ = skills::enabled("huoshou");
}

// ---------------------------------------------------------------------------
// 断肠 / 节命 / 驱虎
// ---------------------------------------------------------------------------

#[test]
fn duanchang_revokes_killer_skills_before_death_cleanup() {
    let mut g = test_game(3);
    g.seats[2].skills = vec!["duanchang".into()];
    g.seats[1].skills = vec!["jushou".into()];
    sanguosha3::kernel::turn::settle_death(&mut g, 2, Some(1));
    resolve::advance(&mut g).expect("advance");
    assert!(g.seats[1].skills_lost, "杀死者永久失去所有技能");
    assert!(!skills::active(&g, 1, "jushou"));
    // 反例：无伤害来源不触发。
    let mut g2 = test_game(3);
    g2.seats[2].skills = vec!["duanchang".into()];
    g2.seats[1].skills = vec!["jushou".into()];
    sanguosha3::kernel::turn::settle_death(&mut g2, 2, None);
    resolve::advance(&mut g2).expect("advance");
    assert!(!g2.seats[1].skills_lost);
}

#[test]
fn jieming_draws_to_x_then_discards_to_x() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["jieming".into()];
    g.seats[1].max_hp = 2;
    g.seats[1].hp = 2;
    mk(&mut g, "h1", "sha", Suit::Spade, Zone::Hand(1));
    mk(&mut g, "h2", "sha", Suit::Spade, Zone::Hand(1));
    for i in 0..3 {
        let id = format!("d{i}");
        mk(&mut g, &id, "sha", Suit::Spade, Zone::Draw);
        g.deck.push(id);
    }
    trigger(&mut g, "jieming", 0, data(&[("event", "DamageAfter".into())]));
    submit(&mut g, "target:1");
    assert_eq!(g.hand_size(1), 4, "摸至 2（原 2 + 摸 2）");
    assert_eq!(g.pending.as_ref().map(|p| p.actor()), Some(1));
    submit(&mut g, "pick:h1");
    submit(&mut g, "pick:h2");
    assert_eq!(g.hand_size(1), 2, "弃至 X=2");
}

#[test]
fn jieming_triggers_on_death() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["jieming".into()];
    g.seats[0].hp = 0;
    g.seats[0].alive = false;
    for i in 0..4 {
        let id = format!("d{i}");
        mk(&mut g, &id, "sha", Suit::Spade, Zone::Draw);
        g.deck.push(id);
    }
    trigger(&mut g, "jieming", 0, data(&[("event", "Death".into())]));
    submit(&mut g, "target:1");
    assert_eq!(g.hand_size(1), 4, "死亡时仍可发动（X=4）");
}

#[test]
fn quhu_lost_takes_damage_from_target_and_win_deals_damage() {
    use sanguosha3::kernel::pindian::PindianResult;
    let mut g = test_game(3);
    g.seats[0].skills = vec!["quhu".into()];
    g.seats[0].hp = 4;
    g.seats[1].hp = 5;
    g.pindian_result = Some(PindianResult {
        initiator: 0,
        target: 1,
        a: "a".into(),
        b: "b".into(),
        rank_a: 3,
        rank_b: 9,
        winner: Some(1),
    });
    trigger(&mut g, "quhu", 0, data(&[("stage", "wait".into()), ("target", json!(1))]));
    assert_eq!(g.seats[0].hp, 3, "没赢 → 目标对荀彧造成 1 点伤害");

    let mut g2 = test_game(3);
    g2.seats[0].skills = vec!["quhu".into()];
    g2.pindian_result = Some(PindianResult {
        initiator: 0,
        target: 1,
        a: "a".into(),
        b: "b".into(),
        rank_a: 9,
        rank_b: 3,
        winner: Some(0),
    });
    trigger(&mut g2, "quhu", 0, data(&[("stage", "wait".into()), ("target", json!(1))]));
    assert_eq!(g2.pending.as_ref().map(|p| p.actor()), Some(0));
    submit(&mut g2, "damage:2");
    assert_eq!(g2.seats[2].hp, 3, "赢 → 令目标对攻击范围内角色造成 1 点伤害");
}

// ---------------------------------------------------------------------------
// 强袭 / 狞恶 / 直谏 / 固政
// ---------------------------------------------------------------------------

#[test]
fn qiangxi_costs_hp_then_damages_once_per_target_and_twice_per_phase() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["qiangxi".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    trigger(&mut g, "qiangxi", 0, json!({}));
    submit(&mut g, "hp");
    assert_eq!(g.seats[0].hp, 3, "代价：失 1 体力");
    submit(&mut g, "target:1");
    assert_eq!(g.seats[1].hp, 3, "造成 1 点伤害");
    // 第二次：同一目标不可再选，选 2。
    trigger(&mut g, "qiangxi", 0, json!({}));
    submit(&mut g, "hp");
    submit(&mut g, "target:2");
    assert_eq!(g.seats[2].hp, 3);
    // 反例：第三次不可用。
    assert!(!sanguosha3::kernel::skills::b16::availability("qiangxi").unwrap()(&g, 0));
}

#[test]
fn ninge_draws_and_discards_on_second_damage_of_turn() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["ninge".into()];
    mk(&mut g, "d1", "sha", Suit::Spade, Zone::Draw);
    g.deck.push("d1".into());
    mk(&mut g, "h1", "sha", Suit::Heart, Zone::Hand(1));
    // 第一次受伤：不触发。
    trigger(&mut g, "ninge", 0, data(&[("event", "DamageAfter".into()), ("victim", json!(1)), ("by", json!(0))]));
    assert!(g.pending.is_none());
    // 第二次受伤：摸 1 并弃其一张。
    trigger(&mut g, "ninge", 0, data(&[("event", "DamageAfter".into()), ("victim", json!(1)), ("by", json!(0))]));
    assert_eq!(g.hand_size(0), 1, "摸一张");
    assert_eq!(g.pending.as_ref().map(|p| p.actor()), Some(0));
    submit(&mut g, "hand");
    assert_eq!(g.hand_size(1), 0, "弃其一张手牌");
}

#[test]
fn zhijian_places_hand_equip_into_other_and_draws() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["zhijian".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    mk(&mut g, "w1", "qinggang_jian", Suit::Spade, Zone::Hand(0));
    mk(&mut g, "d1", "sha", Suit::Spade, Zone::Draw);
    g.deck.push("d1".into());
    trigger(&mut g, "zhijian", 0, json!({}));
    submit(&mut g, "pick:w1");
    submit(&mut g, "target:1");
    assert_eq!(g.cards.get("w1"), Some(&Zone::Equip(1, sanguosha3::kernel::Slot::Weapon)));
    assert_eq!(g.hand_size(0), 1, "然后摸一张");
}

#[test]
fn guzheng_gives_one_and_takes_rest_from_single_discard_batch() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["guzheng".into()];
    mk(&mut g, "h1", "sha", Suit::Spade, Zone::Hand(1));
    mk(&mut g, "h2", "shan", Suit::Heart, Zone::Hand(1));
    g.begin_action_as(1);
    flow::discard(&mut g, 1, "h1", "测试");
    flow::discard(&mut g, 1, "h2", "测试");
    trigger(&mut g, "guzheng", 0, data(&[("event", "CardDiscarded".into()), ("victim", json!(1))]));
    submit(&mut g, "give:h1");
    assert_eq!(g.cards.get("h1"), Some(&Zone::Hand(1)), "弃置者收回一张");
    assert_eq!(g.cards.get("h2"), Some(&Zone::Hand(0)), "固政者获得其余");
}

#[test]
fn guzheng_not_triggered_on_single_discard() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["guzheng".into()];
    mk(&mut g, "h1", "sha", Suit::Spade, Zone::Hand(1));
    g.begin_action_as(1);
    flow::discard(&mut g, 1, "h1", "测试");
    trigger(&mut g, "guzheng", 0, data(&[("event", "CardDiscarded".into()), ("victim", json!(1))]));
    assert!(g.pending.is_none());
}

// ---------------------------------------------------------------------------
// 双雄 / 破军
// ---------------------------------------------------------------------------

#[test]
fn shuangxiong_discard_enables_opposite_color_duel_and_recovers_damage_cards() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["shuangxiong".into()];
    g.turn = 0;
    mk(&mut g, "r1", "sha", Suit::Heart, Zone::Hand(0));
    mk(&mut g, "b1", "sha", Suit::Spade, Zone::Hand(0));
    mk(&mut g, "c1", "sha", Suit::Spade, Zone::Discard);
    g.bump("turn.0.damage_card_taken.c1");
    sanguosha3::kernel::skills::b16::after_draw_phase(&mut g, 0);
    resolve::advance(&mut g).unwrap();
    submit(&mut g, "pick:r1");
    assert_eq!(
        sanguosha3::kernel::skills::b16::use_alias(&g, 0, "b1"),
        Some("juedou"),
        "弃红牌后黑牌可当决斗"
    );
    assert_eq!(sanguosha3::kernel::skills::b16::use_alias(&g, 0, "r1"), None);
    // 结束阶段取回本回合伤你的牌。
    g.phase = Phase::End;
    trigger(&mut g, "shuangxiong", 0, data(&[("event", "PhaseStart".into())]));
    submit(&mut g, "gain");
    assert_eq!(g.cards.get("c1"), Some(&Zone::Hand(0)));
}

#[test]
fn pojun_exiles_cards_until_turn_end_and_damage_bonus_compares_counts() {
    use sanguosha3::kernel::{DamageKind, DamageSpec, SourceKind, Slot};
    let mut g = test_game(3);
    g.seats[0].skills = vec!["pojun".into()];
    g.seats[1].hp = 2;
    mk(&mut g, "h1", "sha", Suit::Spade, Zone::Hand(1));
    mk(&mut g, "e1", "qinggang_jian", Suit::Club, Zone::Equip(1, Slot::Weapon));
    assert!(sanguosha3::kernel::skills::b16::fire_sha_targeted(&mut g, 0, &[1], "c1"));
    resolve::advance(&mut g).unwrap();
    submit(&mut g, "use");
    submit(&mut g, "hand");
    submit(&mut g, "equip:e1");
    assert!(
        matches!(g.cards.get("h1"), Some(Zone::Void)) || matches!(g.cards.get("e1"), Some(Zone::Void)),
        "移出游戏"
    );
    assert!(!g.void_return.is_empty());

    let spec = DamageSpec {
        from: Some(0),
        target: 1,
        amount: 1,
        kind: DamageKind::Normal,
        source: SourceKind::Slash,
    };
    assert_eq!(sanguosha3::kernel::skills::b16::damage_bonus(&g, &spec), 1, "牌数不多于你 → +1");
    // 反例：目标牌更多。
    let mut g2 = test_game(3);
    g2.seats[0].skills = vec!["pojun".into()];
    for i in 0..3 {
        mk(&mut g2, &format!("x{i}"), "sha", Suit::Spade, Zone::Hand(1));
    }
    assert_eq!(sanguosha3::kernel::skills::b16::damage_bonus(&g2, &spec), 0);
}

// ---------------------------------------------------------------------------
// 悲歌
// ---------------------------------------------------------------------------

#[test]
fn beige_discards_then_judges_and_heart_heals() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["beige".into()];
    g.seats[1].hp = 3;
    mk(&mut g, "cost", "sha", Suit::Spade, Zone::Hand(0));
    mk(&mut g, "jdg", "tao", Suit::Heart, Zone::Draw);
    g.deck.push("jdg".into());
    trigger(
        &mut g,
        "beige",
        0,
        data(&[("event", "DamageAfter".into()), ("victim", json!(1)), ("by", json!(2)), ("damage_source", json!("Slash"))]),
    );
    submit(&mut g, "pick:cost");
    assert_eq!(g.seats[1].hp, 4, "红桃 → 回复 1 点体力");
    assert_eq!(g.cards.get("cost"), Some(&Zone::Hand(0)), "同点数 → 获得弃置牌（界版追加）");
}

#[test]
fn beige_not_triggered_by_non_slash_damage() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["beige".into()];
    mk(&mut g, "cost", "sha", Suit::Spade, Zone::Hand(0));
    trigger(
        &mut g,
        "beige",
        0,
        data(&[("event", "DamageAfter".into()), ("victim", json!(1)), ("by", json!(2)), ("damage_source", json!("Other"))]),
    );
    assert!(g.pending.is_none());
}
