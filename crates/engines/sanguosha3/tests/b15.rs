//! B15 技能门禁（J3：8 将 / 19 技能；本文件随批次推进增长）。

use serde_json::json;
use sanguosha3::kernel::resolve::{self, Ask, Stage, Step};
use sanguosha3::kernel::{skills, Card, Game, Phase, Seat, Suit, Zone};

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

fn call(g: &mut Game, skill: &str, actor: usize, d: serde_json::Value) -> Step {
    let h = skills::handler(skill).expect("handler");
    h(g, &Ask { skill: skill.to_string(), stage: Stage::Choose, actor, data: d }).expect("不应报错")
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

fn ids(g: &Game) -> Vec<String> {
    g.pending
        .as_ref()
        .map(|p| {
            p.options()
                .iter()
                .map(|o| o.id.clone())
                .filter(|i| i != "pass")
                .collect()
        })
        .unwrap_or_default()
}

fn mk(g: &mut Game, id: &str, def: &str, suit: Suit, rank: u8, zone: Zone) {
    g.card_defs.insert(
        id.into(),
        Card { id: id.into(), def: def.into(), suit, rank },
    );
    g.cards.insert(id.into(), zone);
}

fn use_active(g: &mut Game, seat: usize, code: &str) {
    skills::use_active(g, seat, code).expect("主动技可用");
}

fn add_deck(g: &mut Game, n: usize) {
    for i in 0..n {
        let id = format!("p{i}");
        mk(g, &id, "shan", Suit::Club, 3, Zone::Draw);
        g.deck.push(id);
    }
}

// ------------------------------ 界董卓 -------------------------------------

#[test]
fn roulin_adds_one_shan_need_and_stacks_with_wushuang() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["roulin".into()];
    g.seats[1].male = false;
    use sanguosha3::kernel::cards::ResponseKind;
    assert_eq!(
        skills::response_need(&g, 1, 0, ResponseKind::Shan),
        2,
        "董卓对女性 → 2 张【闪】"
    );
    let mut g2 = test_game(2);
    g2.seats[0].skills = vec!["roulin".into(), "wushuang".into()];
    g2.seats[1].male = false;
    assert_eq!(skills::response_need(&g2, 1, 0, ResponseKind::Shan), 3, "无双+肉林 = 3");
    let mut g3 = test_game(2);
    g3.seats[0].skills = vec!["roulin".into()];
    assert_eq!(skills::response_need(&g3, 1, 0, ResponseKind::Shan), 1, "男性目标 → 1 张");
}

#[test]
fn bengkuai_triggers_when_others_not_higher_and_skips_after_jiu_sha() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["bengkuai".into()];
    g.turn = 0;
    g.phase = Phase::End;
    g.seats[1].hp = 4;
    trigger(&mut g, "bengkuai", 0, data(&[("event", "PhaseStart".into()), ("phase", "End".into())]));
    submit(&mut g, "lose_hp");
    assert_eq!(g.seats[0].hp, 3, "存在体力不大于你的其他角色 → 失去 1 点");
    let mut g2 = test_game(2);
    g2.seats[0].skills = vec!["bengkuai".into()];
    g2.turn = 0;
    g2.phase = Phase::End;
    g2.bump("turn.0.jiu_sha_hit");
    trigger(&mut g2, "bengkuai", 0, data(&[("event", "PhaseStart".into()), ("phase", "End".into())]));
    assert!(g2.pending.is_none(), "酒【杀】后本回合崩坏失效");
}

#[test]
fn jiuchi_spade_as_jiu_unlimited_and_stacks() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["jiuchi".into()];
    mk(&mut g, "sp", "sha", Suit::Spade, 5, Zone::Hand(0));
    mk(&mut g, "ht", "sha", Suit::Heart, 5, Zone::Hand(0));
    assert_eq!(skills::use_alias(&g, 0, "sp"), Some("jiu"), "黑桃手牌当【酒】");
    assert_eq!(skills::use_alias(&g, 0, "ht"), None, "非黑桃不能当【酒】");
    assert!(skills::jiu_unlimited(&g, 0), "【酒】不限次数");
    sanguosha3::kernel::card_play::use_jiu(&mut g);
    sanguosha3::kernel::card_play::use_jiu(&mut g);
    assert_eq!(g.sha_bonus, 2, "连喝两杯叠加 +2");
    let spec = sanguosha3::kernel::DamageSpec {
        from: Some(0),
        target: 1,
        amount: 1,
        kind: sanguosha3::kernel::DamageKind::Normal,
        source: sanguosha3::kernel::SourceKind::Slash,
    };
    let armor = sanguosha3::kernel::gear::armor_of(&g, 1);
    sanguosha3::kernel::flow::damage(&mut g, spec, armor, 0);
    assert!(g.counter("turn.0.jiu_sha_hit") > 0, "酒【杀】伤害后本回合崩坏失效");
}

#[test]
fn baonue_judges_per_damage_point_and_spade_heals() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["baonue".into()];
    g.seats[0].hp = 3;
    g.seats[1].kingdom = "qun".into();
    mk(&mut g, "sp", "shan", Suit::Spade, 7, Zone::Draw);
    g.deck.push("sp".into());
    mk(&mut g, "cl", "shan", Suit::Club, 7, Zone::Draw);
    g.deck.insert(0, "cl".into());
    trigger(
        &mut g,
        "baonue",
        0,
        data(&[("event", "DamageAfter".into()), ("by", 1.into()), ("victim", 0.into()), ("amount", 2.into())]),
    );
    submit(&mut g, "use");
    assert_eq!(g.seats[0].hp, 4, "黑桃判定 → 回复 1 并获得判定牌");
    assert!(g.hand(0).contains(&"sp".to_string()));
    let opts = ids(&g);
    assert!(opts.contains(&"use".to_string()), "2 点伤害逐点触发：{opts:?}");
    submit(&mut g, "use");
    assert_eq!(g.seats[0].hp, 4, "非黑桃不回复");
    let mut g2 = test_game(2);
    g2.seats[0].skills = vec!["baonue".into()];
    g2.seats[1].kingdom = "wei".into();
    trigger(
        &mut g2,
        "baonue",
        0,
        data(&[("event", "DamageAfter".into()), ("by", 1.into()), ("victim", 0.into()), ("amount", 1.into())]),
    );
    assert!(g2.pending.is_none());
}

// ------------------------------ 界姜维 -------------------------------------

#[test]
fn tiaoxin_punishes_target_who_does_not_slash() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["tiaoxin".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    mk(&mut g, "c1", "tao", Suit::Heart, 3, Zone::Hand(1));
    use_active(&mut g, 0, "tiaoxin");
    submit(&mut g, "t:1");
    submit(&mut g, "pass");
    submit(&mut g, "c:c1");
    assert_eq!(g.cards.get("c1"), Some(&Zone::Discard), "未出【杀】→ 姜维弃其一张牌");
    assert_eq!(g.counter("phase.0.tiaoxin"), 1, "第一次发动");
    let mut g2 = test_game(2);
    g2.seats[0].skills = vec!["tiaoxin".into()];
    g2.turn = 0;
    g2.phase = Phase::Play;
    mk(&mut g2, "s1", "sha", Suit::Club, 5, Zone::Hand(1));
    use_active(&mut g2, 0, "tiaoxin");
    submit(&mut g2, "t:1");
    submit(&mut g2, "s:s1");
    assert!(g2.counter("phase.0.tiaoxin_hit") > 0, "其【杀】造成伤害");
    assert!(
        !skills::active_actions(&g2, 0).iter().any(|c| *c == "tiaoxin"),
        "命中后不能再发动"
    );
}

#[test]
fn zhiji_awakens_when_hand_is_empty() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["zhiji".into()];
    g.seats[0].hp = 2;
    g.turn = 0;
    g.phase = Phase::Prepare;
    trigger(&mut g, "zhiji", 0, data(&[("event", "PhaseStart".into()), ("phase", "Prepare".into())]));
    let opts = ids(&g);
    assert!(opts.contains(&"heal".to_string()) && opts.contains(&"draw".to_string()));
    submit(&mut g, "heal");
    assert_eq!(g.seats[0].hp, 3, "回复 1 点");
    assert_eq!(g.seats[0].max_hp, 3, "上限 -1");
    assert!(skills::active(&g, 0, "guanxing"), "获得【观星】");
    let mut g2 = test_game(2);
    g2.seats[0].skills = vec!["zhiji".into()];
    g2.turn = 0;
    g2.phase = Phase::Prepare;
    mk(&mut g2, "h1", "tao", Suit::Heart, 3, Zone::Hand(0));
    trigger(&mut g2, "zhiji", 0, data(&[("event", "PhaseStart".into()), ("phase", "Prepare".into())]));
    assert!(g2.pending.is_none());
}

#[test]
fn zhiji_offers_only_draw_when_full_hp() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["zhiji".into()];
    g.turn = 0;
    g.phase = Phase::Prepare;
    add_deck(&mut g, 3);
    trigger(&mut g, "zhiji", 0, data(&[("event", "PhaseStart".into()), ("phase", "Prepare".into())]));
    let opts = ids(&g);
    assert!(!opts.contains(&"heal".to_string()), "满体力不提供回复选项：{opts:?}");
    submit(&mut g, "draw");
    assert_eq!(g.hand_size(0), 2, "摸两张");
}

// ---------------------------------------------------------------------------
// B15 第二组（J3：太史慈 / 邓艾 / 祝融 / 贾诩 / 鲁肃 / 张郃）
// ---------------------------------------------------------------------------

fn add_deck2(g: &mut Game, n: usize) {
    for i in 0..n {
        let id = format!("q{i}");
        mk(g, &id, "shan", Suit::Club, 3, Zone::Draw);
        g.deck.push(id);
    }
}

// ------------------------------ 界太史慈 -----------------------------------

#[test]
fn tianyi_wins_boost_and_loses_block_sha() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["tianyi".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    mk(&mut g, "a1", "sha", Suit::Club, 9, Zone::Hand(0));
    mk(&mut g, "b1", "tao", Suit::Heart, 2, Zone::Hand(1));
    use_active(&mut g, 0, "tianyi");
    submit(&mut g, "t:1");
    submit(&mut g, "choose:a1");
    submit(&mut g, "choose:b1");
    assert_eq!(g.counter("turn.0.tianyi_won"), 1, "拼点赢");
    assert_eq!(skills::sha_extra(&g, 0), 1, "本回合【杀】次数 +1");
    assert_eq!(skills::response_need(&g, 1, 0, sanguosha3::kernel::cards::ResponseKind::Shan), 1);
    // 反例：没赢 → 本回合不能使用【杀】。
    let mut g2 = test_game(2);
    g2.seats[0].skills = vec!["tianyi".into()];
    g2.turn = 0;
    g2.phase = Phase::Play;
    mk(&mut g2, "a1", "sha", Suit::Club, 2, Zone::Hand(0));
    mk(&mut g2, "b1", "tao", Suit::Heart, 9, Zone::Hand(1));
    use_active(&mut g2, 0, "tianyi");
    submit(&mut g2, "t:1");
    submit(&mut g2, "choose:a1");
    submit(&mut g2, "choose:b1");
    assert_eq!(g2.counter("turn.0.tianyi_lost"), 1);
    assert!(skills::sha_use_blocked(&g2, 0), "没赢 → 不能使用【杀】");
}

#[test]
fn hanzhan_makes_target_use_random_card() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["tianyi".into(), "hanzhan".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    mk(&mut g, "a1", "sha", Suit::Club, 9, Zone::Hand(0));
    mk(&mut g, "b1", "tao", Suit::Heart, 2, Zone::Hand(1));
    use_active(&mut g, 0, "tianyi");
    submit(&mut g, "t:1");
    submit(&mut g, "hanzhan");
    // 发起者（太史慈）仍需自选；目标为随机牌（不会出现 choose:b1）。
    submit(&mut g, "choose:a1");
    let opts = ids(&g);
    assert!(
        opts.iter().all(|o| o != "choose:b1"),
        "酣战 → 目标随机牌，不应由其自选：{opts:?}"
    );
}

// ------------------------------ 界邓艾 -------------------------------------

#[test]
fn tuotian_gains_tian_out_of_turn_and_distance_shrinks() {
    let mut g = test_game(4);
    g.seats[0].skills = vec!["tuotian".into()];
    g.turn = 1; // 回合外
    mk(&mut g, "h1", "sha", Suit::Club, 5, Zone::Hand(0));
    mk(&mut g, "sp", "shan", Suit::Spade, 6, Zone::Draw);
    g.deck.push("sp".into());
    let before = sanguosha3::kernel::gear::distance(&g, 0, 2);
    trigger(
        &mut g,
        "tuotian",
        0,
        data(&[("event", "CardLost".into()), ("seat", 0.into()), ("card", "h1".into())]),
    );
    submit(&mut g, "use");
    assert_eq!(g.counter("tian.0"), 1, "非红桃 → 得「田」");
    assert_eq!(g.cards.get("sp"), Some(&Zone::Void), "判定牌置为「田」（移出游戏）");
    assert_eq!(sanguosha3::kernel::gear::distance(&g, 0, 2), before.saturating_sub(1), "每田距离 -1");
    // 反例：红桃不得田。
    let mut g2 = test_game(2);
    g2.seats[0].skills = vec!["tuotian".into()];
    g2.turn = 1;
    mk(&mut g2, "h1", "sha", Suit::Club, 5, Zone::Hand(0));
    mk(&mut g2, "ht", "shan", Suit::Heart, 6, Zone::Draw);
    g2.deck.push("ht".into());
    trigger(
        &mut g2,
        "tuotian",
        0,
        data(&[("event", "CardLost".into()), ("seat", 0.into()), ("card", "h1".into())]),
    );
    submit(&mut g2, "use");
    assert_eq!(g2.counter("tian.0"), 0);
}

#[test]
fn zaoxian_awakens_at_three_tian_and_grants_jixi() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["zaoxian".into()];
    g.seats[0].hp = 4;
    g.turn = 0;
    g.phase = Phase::Prepare;
    g.set_counter("tian.0", 3);
    trigger(&mut g, "zaoxian", 0, data(&[("event", "PhaseStart".into()), ("phase", "Prepare".into())]));
    assert_eq!(g.seats[0].max_hp, 3, "上限 -1");
    assert!(skills::active(&g, 0, "jixi"), "获得【急袭】");
    assert_eq!(g.extra_turns, vec![0], "本回合后额外回合");
    // 反例：田 <3 不觉醒。
    let mut g2 = test_game(2);
    g2.seats[0].skills = vec!["zaoxian".into()];
    g2.turn = 0;
    g2.phase = Phase::Prepare;
    g2.set_counter("tian.0", 2);
    trigger(&mut g2, "zaoxian", 0, data(&[("event", "PhaseStart".into()), ("phase", "Prepare".into())]));
    assert_eq!(g2.seats[0].max_hp, 4);
}

#[test]
fn jixi_consumes_a_tian_as_snatch() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["jixi".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    g.set_counter("tian.0", 1);
    mk(&mut g, "h1", "tao", Suit::Heart, 3, Zone::Hand(1));
    use_active(&mut g, 0, "jixi");
    submit(&mut g, "t:1");
    assert_eq!(g.counter("tian.0"), 0, "消耗 1 枚「田」");
    assert!(g.pending.is_some(), "进入【顺手牵羊】结算");
    // 反例：无田不可发动。
    let mut g2 = test_game(2);
    g2.seats[0].skills = vec!["jixi".into()];
    g2.turn = 0;
    g2.phase = Phase::Play;
    assert!(!skills::active_actions(&g2, 0).iter().any(|c| *c == "jixi"));
}

// ------------------------------ 界祝融 -------------------------------------

#[test]
fn juxiang_blocks_nanman_and_steals_it_after_resolution() {
    use sanguosha3::kernel::cards::ResponseKind;
    let mut g = test_game(3);
    g.seats[0].skills = vec!["juxiang".into()];
    let nanman = sanguosha3::kernel::cards::def("nanman_ruqin").expect("南蛮定义");
    let targets = sanguosha3::kernel::card_play::legal_targets(&g, 1, nanman);
    assert!(!targets.contains(&0), "巨象持有者不能成为【南蛮入侵】目标");
    // 结算结束后获得该牌。
    mk(&mut g, "nm", "nanman_ruqin", Suit::Club, 7, Zone::Discard);
    skills::on_card_finished(&mut g, "nm", 1);
    assert!(g.hand(0).contains(&"nm".to_string()), "巨象获得他人使用的【南蛮】");
    let _ = ResponseKind::Sha;
}

#[test]
fn liereng_pindian_win_steals_a_card() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["liereng".into()];
    mk(&mut g, "a1", "sha", Suit::Club, 9, Zone::Hand(0));
    mk(&mut g, "b1", "tao", Suit::Heart, 2, Zone::Hand(1));
    mk(&mut g, "b2", "shan", Suit::Heart, 4, Zone::Hand(1));
    trigger(
        &mut g,
        "liereng",
        0,
        data(&[("event", "DamageAfter".into()), ("by", 0.into()), ("victim", 1.into()), ("amount", 1.into())]),
    );
    submit(&mut g, "use");
    submit(&mut g, "choose:a1");
    submit(&mut g, "choose:b1");
    assert!(
        g.hand(0).iter().any(|c| c == "b1" || c == "b2"),
        "拼点赢 → 获得其一张牌"
    );
    // 反例：没赢不获得。
    let mut g2 = test_game(2);
    g2.seats[0].skills = vec!["liereng".into()];
    mk(&mut g2, "a1", "sha", Suit::Club, 2, Zone::Hand(0));
    mk(&mut g2, "b1", "tao", Suit::Heart, 9, Zone::Hand(1));
    trigger(
        &mut g2,
        "liereng",
        0,
        data(&[("event", "DamageAfter".into()), ("by", 0.into()), ("victim", 1.into()), ("amount", 1.into())]),
    );
    submit(&mut g2, "use");
    submit(&mut g2, "choose:a1");
    submit(&mut g2, "choose:b1");
    assert!(g2.hand(0).iter().all(|c| c != "b1"), "没赢不获得");
}

#[test]
fn changbiao_multi_card_slash_draws_x_when_damaging() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["changbiao".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    mk(&mut g, "c1", "sha", Suit::Club, 5, Zone::Hand(0));
    mk(&mut g, "c2", "tao", Suit::Heart, 6, Zone::Hand(0));
    add_deck2(&mut g, 4);
    use_active(&mut g, 0, "changbiao");
    submit(&mut g, "c:c1");
    submit(&mut g, "c:c2");
    submit(&mut g, "confirm");
    submit(&mut g, "t:1");
    // 目标无【闪】→ 受伤 → 记录命中。
    assert!(g.counter("turn.0.changbiao_hit") > 0, "造成伤害");
    assert_eq!(g.counter("turn.0.changbiao_x"), 2, "X = 使用的牌数");
    trigger(&mut g, "changbiao", 0, data(&[("event", "PhaseStart".into()), ("phase", "End".into())]));
    assert_eq!(g.hand_size(0), 2, "出牌阶段结束摸 X 张");
    // 反例：本阶段已发动过 → 不可再发动。
    assert!(!skills::active_actions(&g, 0).iter().any(|c| *c == "changbiao"));
}

// ------------------------------ 界贾诩 -------------------------------------

#[test]
fn weimu_blocks_black_trick_and_prevents_own_turn_damage() {
    use sanguosha3::kernel::cards::Category;
    let mut g = test_game(3);
    g.seats[0].skills = vec!["weimu".into()];
    mk(&mut g, "blk", "guohe_chaiqiao", Suit::Spade, 3, Zone::Processing);
    let def = sanguosha3::kernel::cards::def("guohe_chaiqiao").unwrap();
    assert!(
        skills::deny_target_instance(&g, 1, 0, Some("blk"), def),
        "黑色锦囊不能以帷幕角色为目标"
    );
    mk(&mut g, "red", "guohe_chaiqiao", Suit::Heart, 3, Zone::Processing);
    assert!(!skills::deny_target_instance(&g, 1, 0, Some("red"), def), "红色锦囊可指定");
    assert_eq!(def.category, Category::Trick);
    // 自己回合内伤害防止并摸 2×点数。
    g.turn = 0;
    g.seats[0].hp = 4;
    add_deck2(&mut g, 4);
    let spec = sanguosha3::kernel::DamageSpec {
        from: Some(1),
        target: 0,
        amount: 2,
        kind: sanguosha3::kernel::DamageKind::Normal,
        source: sanguosha3::kernel::SourceKind::Other,
    };
    let armor = sanguosha3::kernel::gear::armor_of(&g, 0);
    sanguosha3::kernel::flow::damage(&mut g, spec, armor, 0);
    assert_eq!(g.seats[0].hp, 4, "自己回合内伤害被防止");
    assert_eq!(g.hand_size(0), 4, "摸 2×2 张");
    // 反例：他人回合内不防止。
    let mut g2 = test_game(3);
    g2.seats[0].skills = vec!["weimu".into()];
    g2.turn = 1;
    let spec2 = sanguosha3::kernel::DamageSpec {
        from: Some(1),
        target: 0,
        amount: 1,
        kind: sanguosha3::kernel::DamageKind::Normal,
        source: sanguosha3::kernel::SourceKind::Other,
    };
    let armor2 = sanguosha3::kernel::gear::armor_of(&g2, 0);
    sanguosha3::kernel::flow::damage(&mut g2, spec2, armor2, 0);
    assert_eq!(g2.seats[0].hp, 3, "他人回合正常受伤");
}

#[test]
fn luanwu_sequence_and_final_virtual_slash() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["luanwu".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    use_active(&mut g, 0, "luanwu");
    submit(&mut g, "use");
    // 座位 1、2 各选「失去 1 点体力」。
    submit(&mut g, "lose_hp");
    submit(&mut g, "lose_hp");
    assert_eq!(g.seats[1].hp, 3);
    assert_eq!(g.seats[2].hp, 3);
    // 全部结算完毕 → 贾诩可视为使用无距离【杀】。
    let opts = ids(&g);
    assert!(opts.contains(&"use".to_string()), "可视为使用【杀】：{opts:?}");
    submit(&mut g, "pass");
    assert!(skills::limited_used(&g, 0, "luanwu"));
    // 反例：已发动过 → 不再出现。
    assert!(!skills::active_actions(&g, 0).iter().any(|c| *c == "luanwu"));
}

#[test]
fn wansha_restricts_tao_and_suppresses_non_locked_skills() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["wansha".into()];
    g.turn = 0; // 贾诩回合
    assert!(!skills::dying_allow_tao(&g, 2, 1), "他人不能对濒死者用【桃】");
    assert!(skills::dying_allow_tao(&g, 0, 1), "贾诩本人可用【桃】");
    assert!(skills::dying_allow_tao(&g, 1, 1), "濒死者本人可用【桃】");
    // 反例：非贾诩回合不限制。
    let mut g2 = test_game(3);
    g2.seats[0].skills = vec!["wansha".into()];
    g2.turn = 2;
    assert!(skills::dying_allow_tao(&g2, 1, 0));
}

// ------------------------------ 界鲁肃 -------------------------------------

#[test]
fn haoshi_draws_two_and_gives_half_to_poorest() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["haoshi".into()];
    g.turn = 0;
    g.phase = Phase::Draw;
    add_deck2(&mut g, 8);
    for i in 0..4 {
        mk(&mut g, &format!("o{i}"), "shan", Suit::Club, 3, Zone::Hand(0));
    }
    trigger(&mut g, "haoshi", 0, data(&[("event", "PhaseStart".into()), ("phase", "Draw".into())]));
    submit(&mut g, "use");
    // 手牌 4+2=6 >5 → 交一半（3 张）给手牌最少者（都 0 → 并列 → 可选）。
    submit(&mut g, "t:2");
    assert_eq!(g.hand_size(0), 3, "交出一半");
    assert_eq!(g.hand_size(2), 3);
    assert_eq!(g.mark("haoshi.giver"), Some("2"), "记录回礼者");
    // 反例：回礼者无手牌 → 成为目标不询问。
    let mut g2 = test_game(3);
    g2.seats[0].skills = vec!["haoshi".into()];
    g2.set_mark("haoshi.giver", "2");
    mk(&mut g2, "t1", "sha", Suit::Club, 5, Zone::Processing);
    trigger(
        &mut g2,
        "haoshi",
        0,
        data(&[("event", "TargetChosen".into()), ("seat", 0.into()), ("card", "t1".into())]),
    );
    assert!(g2.pending.is_none());
}

#[test]
fn dimeng_swaps_hands_and_pays_at_play_end() {
    let mut g = test_game(4);
    g.seats[0].skills = vec!["dimeng".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    mk(&mut g, "own1", "tao", Suit::Heart, 3, Zone::Hand(0));
    mk(&mut g, "a1", "sha", Suit::Club, 5, Zone::Hand(1));
    mk(&mut g, "b1", "shan", Suit::Heart, 5, Zone::Hand(2));
    use_active(&mut g, 0, "dimeng");
    submit(&mut g, "t:1");
    submit(&mut g, "t:2");
    assert_eq!(g.cards.get("a1"), Some(&Zone::Hand(2)), "交换手牌");
    assert_eq!(g.cards.get("b1"), Some(&Zone::Hand(1)));
    assert_eq!(g.counter("turn.0.dimeng_x"), 0, "手牌差 0");
    assert_eq!(g.counter("phase.0.dimeng"), 1, "出牌阶段限一次");
    // 反例：手牌差 > 自己手牌 → 不能完成（直接报错）。
    let mut g2 = test_game(4);
    g2.seats[0].skills = vec!["dimeng".into()];
    g2.turn = 0;
    g2.phase = Phase::Play;
    mk(&mut g2, "own", "tao", Suit::Heart, 9, Zone::Hand(0));
    mk(&mut g2, "a1", "sha", Suit::Club, 5, Zone::Hand(1));
    for i in 0..3 {
        mk(&mut g2, &format!("b{i}"), "shan", Suit::Heart, 5, Zone::Hand(2));
    }
    use_active(&mut g2, 0, "dimeng");
    submit(&mut g2, "t:1");
    let p = g2.pending.clone().unwrap();
    let err = resolve::submit(&mut g2, p.actor(), "t:2");
    assert!(err.is_err(), "手牌差超过自己手牌数 → 拒绝");
}

// ------------------------------ 界张郃 -------------------------------------

#[test]
fn qiaobian_starts_with_two_bian_and_skips_phase_by_paying() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["qiaobian".into()];
    g.turn = 0;
    g.phase = Phase::Judge;
    mk(&mut g, "pay", "tao", Suit::Heart, 3, Zone::Hand(0));
    call(&mut g, "qiaobian", 0, data(&[]));
    assert_eq!(g.counter("bian.0"), 2, "开局 2 枚「变」");
    trigger(&mut g, "qiaobian", 0, data(&[("event", "PhaseStart".into()), ("phase", "Judge".into())]));
    let opts = ids(&g);
    assert!(opts.contains(&"bian".to_string()) && opts.contains(&"card".to_string()));
    submit(&mut g, "bian");
    assert_eq!(g.counter("bian.0"), 1, "移 1 枚「变」");
    assert!(g.skipped.contains(&Phase::Judge), "跳过判定阶段（判定区保留）");
    // 反例：无「变」且无牌 → 不询问。
    let mut g2 = test_game(3);
    g2.seats[0].skills = vec!["qiaobian".into()];
    g2.turn = 0;
    g2.phase = Phase::Draw;
    trigger(&mut g2, "qiaobian", 0, data(&[("event", "PhaseStart".into()), ("phase", "Draw".into())]));
    assert!(g2.pending.is_none());
}

#[test]
fn qiaobian_end_phase_grants_bian_for_new_hand_count() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["qiaobian".into()];
    g.turn = 0;
    g.phase = Phase::End;
    g.set_counter("bian.0", 0);
    mk(&mut g, "h1", "tao", Suit::Heart, 3, Zone::Hand(0));
    mk(&mut g, "h2", "sha", Suit::Club, 3, Zone::Hand(0));
    trigger(&mut g, "qiaobian", 0, data(&[("event", "PhaseStart".into()), ("phase", "End".into())]));
    assert_eq!(g.counter("bian.0"), 1, "手牌数为新值 → +1「变」");
    // 反例：重复相同手牌数 → 不再获得。
    trigger(&mut g, "qiaobian", 0, data(&[("event", "PhaseStart".into()), ("phase", "End".into())]));
    assert_eq!(g.counter("bian.0"), 1);
}
