//! B13 技能门禁（J1：8 将 / 16 技能；本文件随批次推进增长）。

use sanguosha3::kernel::resolve::{self, Ask, Stage, Step};
use sanguosha3::kernel::{generals, skills, Card, Game, Phase, Seat, Suit, Zone};

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

fn call(g: &mut sanguosha3::kernel::Game, skill: &str, actor: usize, d: serde_json::Value) -> Step {
    let h = skills::handler(skill).expect("handler");
    h(g, &Ask { skill: skill.to_string(), stage: Stage::Choose, actor, data: d }).expect("不应报错")
}

fn submit(g: &mut sanguosha3::kernel::Game, ans: &str) {
    let p = g.pending.clone().expect("期望有待办");
    let actor = p.actor();
    assert!(
        p.has_option(ans),
        "选项 {ans} 不在 {:?}",
        p.options().iter().map(|o| o.id.clone()).collect::<Vec<_>>()
    );
    resolve::submit(g, actor, ans).expect("submit");
}

fn ids(g: &sanguosha3::kernel::Game) -> Vec<String> {
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

/// 触发一次技能（走真实栈：压帧 + 推进）。
fn trigger(g: &mut sanguosha3::kernel::Game, skill: &str, actor: usize, d: serde_json::Value) {
    resolve::push_ask(g, skill, Stage::Choose, actor, d);
    resolve::advance(g).expect("advance");
}

// ---------------------------------------------------------------------------
// 界左慈 · 化身
// ---------------------------------------------------------------------------

#[test]
fn huashen_swap_display_grants_a_skill() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["huashen".into()];
    g.seats[0].general = "sgs_hero_447".into();
    g.rng = 424242;
    g.init_huashen(0, 3);
    let pool = g.huashen.get(&0).unwrap().pool.clone();
    let cur = g.huashen.get(&0).unwrap().current.clone().unwrap();
    let other = pool.iter().find(|c| **c != cur).cloned().expect("至少两张化身牌");
    trigger(&mut g, "huashen", 0, data(&[("event", "TurnStart".into())]));
    submit(&mut g, "swap");
    let swap_opts = ids(&g);
    assert_eq!(swap_opts.len(), 2, "非展示化身牌 = 2 张：{swap_opts:?}");
    submit(&mut g, &format!("c:{other}"));
    assert_eq!(g.huashen_general(0), Some(other.as_str()), "展示已切换");
    // 多技能武将必须再选一个技能（单技能则直接授予）。
    if g.pending.is_some() {
        let skill_opts = ids(&g);
        let pick = skill_opts
            .iter()
            .find(|i| i.starts_with("s:"))
            .cloned()
            .expect("多技能化身必须给出技能候选");
        submit(&mut g, &pick);
    }
    let h = g.huashen.get(&0).unwrap();
    let sk = h.active_skill.clone().expect("取得技能");
    assert!(skills::active(&g, 0, &sk), "授予技生效（grant 入口）");
    let shown = g.huashen_general(0).unwrap().to_string();
    let kd = generals::def(&shown).unwrap().kingdom;
    assert_eq!(g.seats[0].kingdom, kd, "势力跟随展示化身");
}

#[test]
fn huashen_trade_two_for_one_new_card() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["huashen".into()];
    g.seats[0].general = "sgs_hero_447".into();
    g.rng = 77;
    g.init_huashen(0, 3);
    let pool = g.huashen.get(&0).unwrap().pool.clone();
    trigger(&mut g, "huashen", 0, data(&[("event", "TurnStart".into())]));
    submit(&mut g, "trade");
    submit(&mut g, &format!("x:{}", pool[1]));
    submit(&mut g, &format!("x:{}", pool[2]));
    submit(&mut g, "confirm");
    let after = g.huashen.get(&0).unwrap().pool.clone();
    assert_eq!(after.len(), 2, "3 - 2 + 1 = 2");
    assert_eq!(
        after.iter().collect::<std::collections::BTreeSet<_>>().len(),
        after.len(),
        "化身牌不重复"
    );
    assert!(!after.contains(&pool[1]) && !after.contains(&pool[2]), "移去的牌已离手");
}

#[test]
fn huashen_skill_pick_requires_choice_for_multi_skill_general() {
    // 界吕蒙（克己/博图 两个可授予技能）→ 必须由左慈选一个。
    let mut g = test_game(3);
    g.seats[0].skills = vec!["huashen".into()];
    g.seats[0].general = "sgs_hero_447".into();
    g.huashen.insert(
        0,
        sanguosha3::kernel::Huashen {
            pool: vec!["sgs_hero_447".into(), "sgs_hero_306".into()],
            current: Some("sgs_hero_447".into()),
            active_skill: None,
        },
    );
    trigger(&mut g, "huashen", 0, data(&[("event", "TurnStart".into())]));
    submit(&mut g, "swap");
    submit(&mut g, "c:sgs_hero_306");
    let ids = ids(&g);
    assert_eq!(ids.len(), 2, "两个可选技能：{ids:?}");
    submit(&mut g, "s:keji");
    assert_eq!(
        g.huashen.get(&0).and_then(|h| h.active_skill.clone()),
        Some("keji".to_string())
    );
}

#[test]
fn huashen_pool_skips_generals_whose_skills_are_all_owned_on_field() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["huashen".into()];
    g.seats[0].general = "sgs_hero_447".into();
    // 座位 1 已拥有「奸雄」→ 界曹操（唯一可授予技能=奸雄）不可入池。
    g.seats[1].skills = vec!["jianxiong".into()];
    let pool = skills::huashen_generals(&g, 0);
    assert!(!pool.iter().any(|c| c == "sgs_hero_311"), "技能全被同场拥有 → 不入池");
    g.seats[1].skills.clear();
    assert!(
        skills::huashen_generals(&g, 0).iter().any(|c| c == "sgs_hero_311"),
        "解除拥有后重新入池"
    );
}

#[test]
fn huashen_trade_never_returns_the_two_removed_cards_but_reoffers_later() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["huashen".into()];
    g.seats[0].general = "sgs_hero_447".into();
    g.rng = 2026;
    g.init_huashen(0, 3);
    let pool = g.huashen.get(&0).unwrap().pool.clone();
    let (a, b) = (pool[1].clone(), pool[2].clone());
    trigger(&mut g, "huashen", 0, data(&[("event", "TurnStart".into())]));
    submit(&mut g, "trade");
    submit(&mut g, &format!("x:{a}"));
    submit(&mut g, &format!("x:{b}"));
    submit(&mut g, "confirm");
    let after = g.huashen.get(&0).unwrap().pool.clone();
    assert!(!after.contains(&a) && !after.contains(&b), "移去的两张已离手");
    assert_eq!(after.len(), 2, "3 - 2 + 1");
    // Q9：本次换入的新牌一定不是刚移去的两张；但**下一次**换取可以再随机到它们。
    let fresh = skills::huashen_generals(&g, 0);
    assert!(fresh.contains(&a) && fresh.contains(&b), "旧两张回到可用池（下次可换到）");
}

// ---------------------------------------------------------------------------
// 界左慈 · 新生
// ---------------------------------------------------------------------------

#[test]
fn xinsheng_adds_one_card_per_damage_point() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["xinsheng".into()];
    g.seats[0].general = "sgs_hero_447".into();
    g.rng = 99;
    g.init_huashen(0, 1);
    let before = g.huashen.get(&0).unwrap().pool.len();
    trigger(
        &mut g,
        "xinsheng",
        0,
        data(&[("event", "DamageAfter".into()), ("victim", 0.into()), ("by", 1.into()), ("amount", 2.into())]),
    );
    submit(&mut g, "use");
    submit(&mut g, "use");
    let after = g.huashen.get(&0).unwrap().pool.len();
    assert_eq!(after, before + 2, "2 点伤害各得 1 张");
}

#[test]
fn xinsheng_declines_and_pool_grows_once() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["xinsheng".into()];
    g.seats[0].general = "sgs_hero_447".into();
    g.rng = 5;
    g.init_huashen(0, 1);
    let before = g.huashen.get(&0).unwrap().pool.len();
    trigger(
        &mut g,
        "xinsheng",
        0,
        data(&[("event", "DamageAfter".into()), ("victim", 0.into()), ("by", 1.into()), ("amount", 3.into())]),
    );
    submit(&mut g, "use");
    submit(&mut g, "pass");
    let after = g.huashen.get(&0).unwrap().pool.len();
    assert_eq!(after, before + 1, "放弃剩余点数");
}

// ---------------------------------------------------------------------------
// 界张角 · 鬼道 / 雷击
// ---------------------------------------------------------------------------

#[test]
fn guidao_only_offers_black_cards_and_draws_on_spade_2_to_9() {
    use sanguosha3::kernel::{Card, Suit, Zone};
    let mut g = test_game(2);
    g.seats[0].skills = vec!["guidao".into()];
    g.card_defs.insert(
        "red".into(),
        Card { id: "red".into(), def: "sha".into(), suit: Suit::Heart, rank: 5 },
    );
    g.cards.insert("red".into(), Zone::Hand(0));
    g.card_defs.insert(
        "sp9".into(),
        Card { id: "sp9".into(), def: "sha".into(), suit: Suit::Spade, rank: 9 },
    );
    g.cards.insert("sp9".into(), Zone::Hand(0));
    // 牌堆 1 张备摸。
    g.card_defs.insert(
        "d1".into(),
        Card { id: "d1".into(), def: "shan".into(), suit: Suit::Club, rank: 3 },
    );
    g.cards.insert("d1".into(), Zone::Draw);
    g.deck.push("d1".into());
    sanguosha3::kernel::quick_judge::start(&mut g, 0, "测试判定", None);
    resolve::advance(&mut g).unwrap();
    let opts = ids(&g);
    assert_eq!(opts, vec!["card:sp9".to_string()], "只有黑色牌可改判：{opts:?}");
    submit(&mut g, "card:sp9");
    assert_eq!(g.hand_size(0), 1, "黑桃 9 → 改判后摸一张");
    let r = g.judge_result.clone().expect("判定结果");
    assert_eq!(r.card, "sp9");
}

#[test]
fn guidao_on_own_judge_then_fires_leiji_after_effective() {
    use sanguosha3::kernel::{Card, Suit, Zone};
    let mut g = test_game(2);
    g.seats[0].skills = vec!["guidao".into(), "leiji".into()];
    g.seats[0].general = "sgs_hero_448".into();
    g.card_defs.insert(
        "top".into(),
        Card { id: "top".into(), def: "sha".into(), suit: Suit::Heart, rank: 5 },
    );
    g.cards.insert("top".into(), Zone::Draw);
    g.deck.push("top".into());
    g.card_defs.insert(
        "sp7".into(),
        Card { id: "sp7".into(), def: "sha".into(), suit: Suit::Spade, rank: 7 },
    );
    g.cards.insert("sp7".into(), Zone::Hand(0));
    g.card_defs.insert(
        "d1".into(),
        Card { id: "d1".into(), def: "shan".into(), suit: Suit::Club, rank: 2 },
    );
    g.cards.insert("d1".into(), Zone::Draw);
    g.deck.insert(0, "d1".into());
    sanguosha3::kernel::quick_judge::start(&mut g, 0, "测试判定", None);
    resolve::advance(&mut g).unwrap();
    submit(&mut g, "card:sp7");
    // 鬼道改判自己的判定 → 生效后询问雷击（判定后效果）。
    let opts = ids(&g);
    assert!(opts.contains(&"use".to_string()), "雷击应被询问：{opts:?}");
    submit(&mut g, "use");
    let opts = ids(&g);
    assert!(
        opts.iter().any(|i| i.starts_with("t:")),
        "黑桃 → 可选目标造成 2 点雷伤：{opts:?}"
    );
    let hp = g.seats[1].hp;
    submit(&mut g, "t:1");
    assert_eq!(g.seats[1].hp, hp - 2, "黑桃 → 2 点雷电伤害");
}

#[test]
fn leiji_club_heals_then_deals_one_thunder() {
    use sanguosha3::kernel::{Card, Suit, Zone};
    let mut g = test_game(2);
    g.seats[0].skills = vec!["leiji".into()];
    g.seats[0].hp = 3;
    g.card_defs.insert(
        "top".into(),
        Card { id: "top".into(), def: "sha".into(), suit: Suit::Club, rank: 8 },
    );
    g.cards.insert("top".into(), Zone::Draw);
    g.deck.push("top".into());
    trigger(
        &mut g,
        "leiji",
        0,
        data(&[("event", "CardPlayed".into()), ("card", "shan".into())]),
    );
    submit(&mut g, "use");
    assert_eq!(g.seats[0].hp, 4, "梅花 → 先回复 1 点");
    let hp = g.seats[1].hp;
    submit(&mut g, "t:1");
    assert_eq!(g.seats[1].hp, hp - 1, "梅花 → 1 点雷电伤害");
}

#[test]
fn leiji_heart_has_no_effect_and_declining_judge_is_silent() {
    use sanguosha3::kernel::{Card, Suit, Zone};
    let mut g = test_game(2);
    g.seats[0].skills = vec!["leiji".into()];
    g.card_defs.insert(
        "top".into(),
        Card { id: "top".into(), def: "sha".into(), suit: Suit::Heart, rank: 8 },
    );
    g.cards.insert("top".into(), Zone::Draw);
    g.deck.push("top".into());
    trigger(
        &mut g,
        "leiji",
        0,
        data(&[("event", "CardPlayed".into()), ("card", "shan".into())]),
    );
    submit(&mut g, "use");
    assert!(g.pending.is_none(), "红桃/方块无事发生");
    let mut g2 = test_game(2);
    g2.seats[0].skills = vec!["leiji".into()];
    trigger(
        &mut g2,
        "leiji",
        0,
        data(&[("event", "CardPlayed".into()), ("card", "shan".into())]),
    );
    submit(&mut g2, "pass");
    assert!(g2.pending.is_none(), "放弃判定不结算");
}

// ---------------------------------------------------------------------------
// 界黄忠 · 烈弓
// ---------------------------------------------------------------------------

fn mk(g: &mut Game, id: &str, def: &str, suit: Suit, rank: u8, zone: Zone) {
    g.card_defs.insert(
        id.into(),
        Card { id: id.into(), def: def.into(), suit, rank },
    );
    g.cards.insert(id.into(), zone);
}

#[test]
fn liegong_blocks_shan_or_adds_damage() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["liegong".into()];
    mk(&mut g, "sha1", "sha", Suit::Spade, 9, Zone::Discard);
    mk(&mut g, "h1", "tao", Suit::Heart, 3, Zone::Hand(1));
    // 目标手牌 1 ≤ 自己 0? 否 → 不封闪；体力：自己4 ≤ 目标4 → 伤害 +1。
    let d = data(&[("step", "sha_intercept".into()), ("by", 1.into()), ("target", 0.into()), ("card", "sha1".into())]);
    let mut step = call(&mut g, "liegong", 0, d.clone());
    assert!(matches!(step, Step::Need(_)));
    g.answer = Some("use".into());
    g.answer_skill = Some("liegong".into());
    step = call(&mut g, "liegong", 0, d);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.counter("turn.0.liegong.t.1"), 1, "体力≤目标 → 此【杀】对其 +1");
    let spec = sanguosha3::kernel::DamageSpec {
        from: Some(0),
        target: 1,
        amount: 1,
        kind: sanguosha3::kernel::DamageKind::Normal,
        source: sanguosha3::kernel::SourceKind::Slash,
    };
    assert_eq!(skills::damage_bonus(&g, &spec), 1);
    // 反例：不发动 → 无拦截、无加成。
    let mut g2 = test_game(2);
    g2.seats[0].skills = vec!["liegong".into()];
    mk(&mut g2, "sha1", "sha", Suit::Spade, 9, Zone::Discard);
    let d2 = data(&[("step", "sha_intercept".into()), ("by", 1.into()), ("target", 0.into()), ("card", "sha1".into())]);
    call(&mut g2, "liegong", 0, d2.clone());
    g2.answer = Some("pass".into());
    g2.answer_skill = Some("liegong".into());
    call(&mut g2, "liegong", 0, d2);
    assert_eq!(g2.counter("turn.0.liegong.t.1"), 0);
}

#[test]
fn liegong_reach_uses_sha_rank() {
    let mut g = test_game(4);
    g.seats[0].skills = vec!["liegong".into()];
    mk(&mut g, "sha9", "sha", Suit::Spade, 9, Zone::Hand(0));
    skills::on_use_start(&mut g, 0, "sha9");
    assert_eq!(g.use_opts("sha9").range_override, Some(9), "距离 ≤ 点数即视为可达");
}

// ---------------------------------------------------------------------------
// 界徐庶 · 诛害 / 潜心 / 荐言
// ---------------------------------------------------------------------------

#[test]
fn zhuhai_uses_slash_on_damaging_other() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["zhuhai".into()];
    g.turn = 1;
    g.phase = Phase::End;
    g.set_counter("turn.1.damage_by", 1);
    mk(&mut g, "sha1", "sha", Suit::Club, 5, Zone::Hand(0));
    mk(&mut g, "sh1", "shan", Suit::Heart, 2, Zone::Hand(1));
    trigger(&mut g, "zhuhai", 0, data(&[("event", "PhaseStart".into()), ("phase", "End".into())]));
    submit(&mut g, "use");
    // 唯一【杀】→ 直接使用（无距离），进入目标/结算流程。
    assert!(g.pending.is_some(), "进入【杀】结算");
    // 反例：目标本回合未造成伤害 → 不询问。
    let mut g2 = test_game(2);
    g2.seats[0].skills = vec!["zhuhai".into()];
    g2.turn = 1;
    g2.phase = Phase::End;
    mk(&mut g2, "sha1", "sha", Suit::Club, 5, Zone::Hand(0));
    trigger(&mut g2, "zhuhai", 0, data(&[("event", "PhaseStart".into()), ("phase", "End".into())]));
    assert!(g2.pending.is_none());
}

#[test]
fn qianxin_awakens_when_wounded_and_grants_jianyan() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["qianxin".into()];
    g.seats[0].hp = 2;
    call(
        &mut g,
        "qianxin",
        0,
        data(&[("event", "DamageAfter".into()), ("victim", 1.into()), ("by", 0.into()), ("amount", 1.into())]),
    );
    assert_eq!(g.seats[0].max_hp, 3, "体力上限 -1");
    assert!(skills::active(&g, 0, "jianyan"), "获得授予技【荐言】");
    // 反例：未受伤不触发。
    let mut g2 = test_game(2);
    g2.seats[0].skills = vec!["qianxin".into()];
    call(
        &mut g2,
        "qianxin",
        0,
        data(&[("event", "DamageAfter".into()), ("victim", 1.into()), ("by", 0.into()), ("amount", 1.into())]),
    );
    assert_eq!(g2.seats[0].max_hp, 4);
    assert!(!skills::active(&g2, 0, "jianyan"));
}

#[test]
fn jianyan_declares_and_gives_first_matching_card_to_male() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["jianyan".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    g.seats[1].male = true;
    mk(&mut g, "t1", "tao", Suit::Heart, 3, Zone::Draw);
    g.deck.push("t1".into());
    mk(&mut g, "x1", "qinggang_jian", Suit::Spade, 6, Zone::Draw);
    g.deck.insert(0, "x1".into());
    skills::use_active(&mut g, 0, "jianyan").expect("可发动");
    submit(&mut g, "basic");
    submit(&mut g, "t:1");
    assert_eq!(g.cards.get("t1"), Some(&Zone::Hand(1)), "第一张基本牌交给男性");
    // 反例：无男性角色时不可发动。
    let mut g2 = test_game(2);
    g2.seats[0].skills = vec!["jianyan".into()];
    g2.turn = 0;
    g2.phase = Phase::Play;
    g2.seats[1].male = false;
    g2.seats[0].male = false;
    assert!(!skills::active_actions(&g2, 0).iter().any(|c| *c == "jianyan"));
}

// ---------------------------------------------------------------------------
// 界华雄 · 耀武 / 势斩
// ---------------------------------------------------------------------------

#[test]
fn yaowu_draws_for_source_on_red_or_self_otherwise() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["yaowu".into()];
    mk(&mut g, "red", "sha", Suit::Heart, 5, Zone::Discard);
    mk(&mut g, "d1", "shan", Suit::Club, 2, Zone::Draw);
    g.deck.push("d1".into());
    call(
        &mut g,
        "yaowu",
        0,
        data(&[("event", "DamageAfter".into()), ("victim", 0.into()), ("by", 1.into()), ("card", "red".into()), ("amount", 1.into())]),
    );
    assert_eq!(g.hand_size(1), 1, "红色伤害牌 → 来源摸 1");
    // 非红/虚拟牌 → 自己摸 1。
    let mut g2 = test_game(2);
    g2.seats[0].skills = vec!["yaowu".into()];
    mk(&mut g2, "d1", "shan", Suit::Club, 2, Zone::Draw);
    g2.deck.push("d1".into());
    call(
        &mut g2,
        "yaowu",
        0,
        data(&[("event", "DamageAfter".into()), ("victim", 0.into()), ("by", 1.into()), ("amount", 1.into())]),
    );
    assert_eq!(g2.hand_size(0), 1);
    // 红牌但来源为自己 → 不摸。
    let mut g3 = test_game(2);
    g3.seats[0].skills = vec!["yaowu".into()];
    mk(&mut g3, "red", "sha", Suit::Heart, 5, Zone::Discard);
    call(
        &mut g3,
        "yaowu",
        0,
        data(&[("event", "DamageAfter".into()), ("victim", 0.into()), ("by", 0.into()), ("card", "red".into()), ("amount", 1.into())]),
    );
    assert_eq!(g3.hand_size(0), 0);
}

#[test]
fn shizhan_virtual_duel_twice_per_phase() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["shizhan".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    mk(&mut g, "d1", "sha", Suit::Club, 5, Zone::Hand(0));
    skills::use_active(&mut g, 0, "shizhan").expect("可发动");
    submit(&mut g, "t:1");
    assert_eq!(
        g.pending.as_ref().map(|p| p.kind().to_string()),
        Some("duel_ask".to_string()),
        "视为【决斗】，华雄先出【杀】"
    );
    assert_eq!(g.counter("phase.0.shizhan"), 1);
    // 反例：用尽两次后不再出现。
    g.set_counter("phase.0.shizhan", 2);
    assert!(!skills::active_actions(&g, 0).iter().any(|c| *c == "shizhan"));
}

// ---------------------------------------------------------------------------
// 界张角 · 黄天
// ---------------------------------------------------------------------------

#[test]
fn huangtian_gives_shan_or_spade_to_lord() {
    let mut g = test_game(3);
    g.seats[0].identity = Some(sanguosha3::kernel::Identity::Lord);
    g.seats[0].skills = vec!["huangtian".into()];
    g.turn = 1;
    g.phase = Phase::Play;
    mk(&mut g, "shan1", "shan", Suit::Heart, 2, Zone::Hand(1));
    assert!(skills::other_side_actions(&g, 1).contains(&"huangtian"), "群势力可黄天");
    skills::start_other(&mut g, 1, "huangtian").expect("可发动");
    resolve::advance(&mut g).unwrap();
    submit(&mut g, "c:shan1");
    assert_eq!(g.cards.get("shan1"), Some(&Zone::Hand(0)), "交给主公");
    // 反例：非群势力不可用。
    let mut g2 = test_game(3);
    g2.seats[0].identity = Some(sanguosha3::kernel::Identity::Lord);
    g2.seats[0].skills = vec!["huangtian".into()];
    g2.turn = 1;
    g2.phase = Phase::Play;
    g2.seats[1].kingdom = "wei".into();
    mk(&mut g2, "shan1", "shan", Suit::Heart, 2, Zone::Hand(1));
    assert!(skills::other_side_actions(&g2, 1).is_empty());
}

// ---------------------------------------------------------------------------
// 界于吉 · 蛊惑
// ---------------------------------------------------------------------------

#[test]
fn guhuo_true_declaration_resolves_as_declared_card() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["guhuo".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    g.seats[0].hp = 3;
    mk(&mut g, "c1", "tao", Suit::Heart, 3, Zone::Hand(0));
    skills::use_active(&mut g, 0, "guhuo").expect("可发动");
    submit(&mut g, "c:c1");
    submit(&mut g, "d:tao");
    // 质疑链：座位 1 不质疑 → 翻开（真）→ 按声明牌结算（桃，仅自己）。
    submit(&mut g, "pass");
    assert_eq!(g.seats[0].hp, 4, "为真 → 按【桃】结算");
    assert_eq!(g.counter("turn.0.guhuo_used"), 1, "每回合限一次");
}

#[test]
fn guhuo_false_declaration_is_void_and_doubter_draws() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["guhuo".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    mk(&mut g, "c1", "sha", Suit::Spade, 5, Zone::Hand(0));
    mk(&mut g, "d1", "shan", Suit::Club, 2, Zone::Draw);
    g.deck.push("d1".into());
    skills::use_active(&mut g, 0, "guhuo").expect("可发动");
    submit(&mut g, "c:c1");
    submit(&mut g, "d:tao");
    submit(&mut g, "doubt");
    assert_eq!(g.cards.get("c1"), Some(&Zone::Discard), "为假 → 此牌作废");
    assert_eq!(g.hand_size(1), 1, "为假 → 质疑者摸一张");
    assert_eq!(g.counter("chanyuan.1"), 0, "为假不获得「缠怨」");
}

#[test]
fn guhuo_true_declaration_punishes_doubter_with_chanyuan() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["guhuo".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    mk(&mut g, "c1", "tao", Suit::Heart, 3, Zone::Hand(0));
    skills::use_active(&mut g, 0, "guhuo").expect("可发动");
    submit(&mut g, "c:c1");
    submit(&mut g, "d:tao");
    submit(&mut g, "doubt");
    // 为真 → 质疑者获得缠怨，并自选弃一张牌或失去 1 点体力。
    assert!(g.counter("chanyuan.1") > 0, "质疑者获得「缠怨」");
    submit(&mut g, "lose_hp");
    assert_eq!(g.seats[1].hp, 3, "质疑者失去 1 点体力");
}

#[test]
fn guhuo_alt_in_shan_window_and_chanyuan_blocks_doubt() {
    // 闪窗口：蛊惑作为替代响应。
    let mut g = test_game(2);
    g.seats[0].skills = vec!["guhuo".into()];
    g.turn = 1;
    g.seats[0].hp = 2;
    mk(&mut g, "c1", "shan", Suit::Heart, 3, Zone::Hand(0));
    let alts = skills::response_alternatives(&g, 0, sanguosha3::kernel::cards::ResponseKind::Shan, "sha");
    assert!(alts.iter().any(|(c, _)| *c == "guhuo"), "蛊惑可当【闪】");
    // 缠怨者不能质疑：座位 1 有缠怨 → 质疑链直接跳过（不产生 pending）。
    g.bump("chanyuan.1");
    trigger(
        &mut g,
        "guhuo",
        0,
        data(&[("step", "alt".into()), ("kind", "shan".into()), ("ctx", "sha".into()), ("card", "sha1".into())]),
    );
    submit(&mut g, "c:c1");
    assert!(g.pending.is_none(), "缠怨者不参与质疑（无待办）");
    assert!(g.mark("response.sub.0").is_some(), "为真 → 响应成功标记");
}

// ---------------------------------------------------------------------------
// 界袁绍 · 乱击 / 血裔
// ---------------------------------------------------------------------------

#[test]
fn luanji_two_same_suit_as_archery_attack() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["luanji".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    mk(&mut g, "a1", "sha", Suit::Spade, 5, Zone::Hand(0));
    mk(&mut g, "a2", "tao", Suit::Spade, 6, Zone::Hand(0));
    mk(&mut g, "sh2", "shan", Suit::Heart, 2, Zone::Hand(2));
    skills::use_active(&mut g, 0, "luanji").expect("可发动");
    submit(&mut g, "c:a1");
    submit(&mut g, "c:a2");
    submit(&mut g, "trim");
    submit(&mut g, "t:1");
    assert!(g.pending.is_some(), "进入【万箭齐发】结算（目标只剩座位 2）");
    // 反例：两张不同花色不可发动。
    let mut g2 = test_game(3);
    g2.seats[0].skills = vec!["luanji".into()];
    g2.turn = 0;
    g2.phase = Phase::Play;
    mk(&mut g2, "a1", "sha", Suit::Spade, 5, Zone::Hand(0));
    mk(&mut g2, "a2", "tao", Suit::Heart, 6, Zone::Hand(0));
    assert!(!skills::active_actions(&g2, 0).iter().any(|c| *c == "luanji"));
}

#[test]
fn xueyi_grants_yi_at_start_and_boosts_hand_limit() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["xueyi".into()];
    g.seats[1].kingdom = "qun".into();
    g.seats[2].kingdom = "wei".into();
    g.turn = 0;
    g.phase = Phase::Play;
    call(&mut g, "xueyi", 0, data(&[]));
    assert_eq!(g.counter("yi.0"), 4, "群势力 2 人 × 2");
    let limit = g.hand_limit(0);
    assert_eq!(limit, 4 + 4, "手牌上限 + 裔数");
    mk(&mut g, "d1", "shan", Suit::Club, 2, Zone::Draw);
    g.deck.push("d1".into());
    trigger(&mut g, "xueyi", 0, data(&[("event", "PhaseStart".into()), ("phase", "Play".into())]));
    submit(&mut g, "use");
    assert_eq!(g.counter("yi.0"), 3, "移去 1 枚");
    assert_eq!(g.hand_size(0), 1, "摸一张");
    // 反例：裔为 0 时不再询问。
    let mut g2 = test_game(2);
    g2.seats[0].skills = vec!["xueyi".into()];
    g2.turn = 0;
    g2.phase = Phase::Play;
    g2.set_counter("yi.0", 0);
    trigger(&mut g2, "xueyi", 0, data(&[("event", "PhaseStart".into()), ("phase", "Play".into())]));
    assert!(g2.pending.is_none());
}

// ---------------------------------------------------------------------------
// 界刘禅 · 享乐 / 放权 / 若愚 + 思蜀
// ---------------------------------------------------------------------------

#[test]
fn xiangle_forces_user_to_discard_basic_or_slash_is_void() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["xiangle".into()];
    mk(&mut g, "b1", "sha", Suit::Club, 5, Zone::Hand(1));
    let d = data(&[("step", "sha_intercept".into()), ("by", 1.into()), ("target", 0.into()), ("card", "c9".into())]);
    trigger(&mut g, "xiangle", 0, d.clone());
    assert_eq!(g.pending.as_ref().map(|p| p.actor()), Some(1), "由使用者选择");
    submit(&mut g, "c:b1");
    assert_eq!(g.cards.get("b1"), Some(&Zone::Discard));
    assert_eq!(g.use_opts("c9").intercept, Some(sanguosha3::kernel::ShaIntercept::Continue));
    // 反例：使用者没有基本牌 → 直接无效，不产生 pending。
    let mut g2 = test_game(2);
    g2.seats[0].skills = vec!["xiangle".into()];
    mk(&mut g2, "t1", "tao", Suit::Heart, 3, Zone::Hand(1));
    g2.cards.insert("t1".into(), Zone::Hand(1));
    // 只有【桃】是基本牌 → 仍有基本牌；改成只有锦囊。
    g2.cards.remove("t1");
    mk(&mut g2, "w1", "wuzhong_shengyou", Suit::Heart, 3, Zone::Hand(1));
    call(&mut g2, "xiangle", 0, d);
    assert_eq!(g2.use_opts("c9").intercept, Some(sanguosha3::kernel::ShaIntercept::Immune));
    assert!(g2.pending.is_none());
}

#[test]
fn fangquan_skips_play_and_grants_extra_turn_at_discard() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["fangquan".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    skills::use_active(&mut g, 0, "fangquan").expect("可发动");
    submit(&mut g, "use");
    assert!(g.mark("skip_play_now.0").is_some(), "立即跳过出牌阶段");
    assert_eq!(g.counter("turn.0.fangquan"), 1, "登记弃牌阶段窗口");
    // 弃牌阶段开始：弃一张手牌 + 选他人 → 额外回合。
    g.phase = Phase::Discard;
    mk(&mut g, "h1", "tao", Suit::Heart, 3, Zone::Hand(0));
    trigger(&mut g, "fangquan", 0, data(&[("event", "PhaseStart".into()), ("phase", "Discard".into())]));
    submit(&mut g, "use");
    submit(&mut g, "c:h1");
    submit(&mut g, "t:2");
    assert_eq!(g.extra_turns, vec![2], "目标获得额外回合");
    // 反例：弃牌阶段没有手牌 → 不询问。
    let mut g2 = test_game(3);
    g2.seats[0].skills = vec!["fangquan".into()];
    g2.turn = 0;
    g2.phase = Phase::Discard;
    g2.bump("turn.0.fangquan");
    trigger(&mut g2, "fangquan", 0, data(&[("event", "PhaseStart".into()), ("phase", "Discard".into())]));
    assert!(g2.pending.is_none());
}

#[test]
fn ruoyu_awakens_at_strict_minimum_hp_and_grants_sishu() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["ruoyu".into()];
    g.seats[0].identity = Some(sanguosha3::kernel::Identity::Lord);
    g.seats[0].hp = 2;
    g.seats[0].max_hp = 3;
    trigger(&mut g, "ruoyu", 0, data(&[("event", "PhaseStart".into()), ("phase", "Prepare".into())]));
    assert_eq!(g.seats[0].max_hp, 4, "体力上限 +1");
    assert_eq!(g.seats[0].hp, 3, "体力低于 3 → 设为 3");
    assert!(skills::active(&g, 0, "jijiang") && skills::active(&g, 0, "sishu"));
    // 反例：并列最小不触发。
    let mut g2 = test_game(3);
    g2.seats[0].skills = vec!["ruoyu".into()];
    g2.seats[0].identity = Some(sanguosha3::kernel::Identity::Lord);
    g2.seats[0].hp = 4;
    g2.seats[2].hp = 4;
    trigger(&mut g2, "ruoyu", 0, data(&[("event", "PhaseStart".into()), ("phase", "Prepare".into())]));
    assert_eq!(g2.seats[0].max_hp, 4);
}

#[test]
fn sishu_reverses_then_restores_indulgence_judge() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["sishu".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    skills::use_active(&mut g, 0, "sishu").expect("可发动");
    submit(&mut g, "t:1");
    assert!(g.judge_reversed(1, "lebu_sishu"), "反转");
    assert_eq!(g.counter("turn.0.sishu"), 1, "每回合一次");
    // 反例：本回合不可再发动。
    assert!(!skills::active_actions(&g, 0).iter().any(|c| *c == "sishu"));
    // 已反转 → 再次（下一回合）恢复常规。
    g.set_counter("turn.0.sishu", 0);
    skills::use_active(&mut g, 0, "sishu").expect("可再次发动");
    submit(&mut g, "t:1");
    assert!(!g.judge_reversed(1, "lebu_sishu"), "恢复常规");
}
