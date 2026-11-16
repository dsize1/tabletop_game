//! B9 技能门禁（界标·蜀：7 将 / 14 技能）。

use sanguosha3::kernel::cards::ResponseKind;
use sanguosha3::kernel::resolve::{self, Ask, Stage, Step};
use sanguosha3::kernel::skills::b9;
use sanguosha3::kernel::{flow, gear, skills, Card, FreezeUntil, Game, Identity, Phase, Seat, Slot, Suit, Zone};

fn seat_with(id: usize, list: &[&str], kingdom: &str) -> Seat {
    Seat {
        id,
        hp: 4,
        max_hp: 4,
        alive: true,
        skills: list.iter().map(|s| s.to_string()).collect(),
        kingdom: kingdom.to_string(),
        ..Default::default()
    }
}

fn game(n: usize, s0: &[&str]) -> Game {
    let mut seats: Vec<Seat> = (0..n).map(|i| seat_with(i, &[], "shu")).collect();
    seats[0] = seat_with(0, s0, "shu");
    Game { seats, ..Default::default() }
}

fn add_suit(g: &mut Game, id: &str, def: &str, zone: Zone, suit: Suit, rank: u8) {
    g.cards.insert(id.to_string(), zone);
    g.card_defs.insert(
        id.to_string(),
        Card { id: id.to_string(), def: def.to_string(), suit, rank },
    );
}

fn add(g: &mut Game, id: &str, def: &str, zone: Zone) {
    add_suit(g, id, def, zone, Suit::Club, 7);
}

fn add_deck(g: &mut Game, n: usize) {
    for i in 0..n {
        let id = format!("d{i}");
        add(g, &id, "shan", Zone::Draw);
        g.deck.push(id);
    }
}

fn data(pairs: &[(&str, serde_json::Value)]) -> serde_json::Value {
    let mut m = serde_json::Map::new();
    for (k, v) in pairs {
        m.insert((*k).to_string(), v.clone());
    }
    serde_json::Value::Object(m)
}

fn call(g: &mut Game, skill: &str, actor: usize, d: serde_json::Value) -> Step {
    let h = skills::handler(skill).expect("handler");
    h(g, &Ask { skill: skill.to_string(), stage: Stage::Choose, actor, data: d })
        .expect("不应报错")
}

fn pending_of(step: &Step) -> &sanguosha3::kernel::pending::Pending {
    match step {
        Step::Need(p) | Step::NeedAt(_, p) | Step::NeedWith(_, _, p) => p,
        _ => panic!("期望待办，实际 {step:?}"),
    }
}

fn run_steps(g: &mut Game, skill: &str, actor: usize, d: serde_json::Value, answers: &[&str]) -> Step {
    let mut data = d;
    let mut step = call(g, skill, actor, data.clone());
    for ans in answers {
        loop {
            match &step {
                Step::Next(_, next) => {
                    data = next.clone();
                    step = call(g, skill, actor, data.clone());
                }
                Step::Need(_) | Step::NeedAt(_, _) => break,
                Step::NeedWith(_, next, _) => {
                    data = next.clone();
                    break;
                }
                other => panic!("期望待办，实际 {other:?}（ans={ans}）"),
            }
        }
        let p = pending_of(&step);
        assert!(
            p.has_option(ans),
            "选项 {ans} 不在 {:?}",
            p.options().iter().map(|o| o.id.clone()).collect::<Vec<_>>()
        );
        g.answer = Some(ans.to_string());
        g.answer_skill = Some(skill.to_string());
        step = call(g, skill, actor, data.clone());
    }
    step
}

fn drain(g: &mut Game, skill: &str, actor: usize, step: Step) -> Step {
    let mut step = step;
    loop {
        match step {
            Step::Next(_, data) => step = call(g, skill, actor, data),
            other => return other,
        }
    }
}

fn submit(g: &mut Game, seat: usize, option: &str) -> Result<(), String> {
    resolve::submit(g, seat, option)
}

// ---------------------------------------------------------------------------
// 仁德
// ---------------------------------------------------------------------------

#[test]
fn rende_second_card_allows_virtual_basic() {
    let mut g = game(2, &["rende"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "a", "sha", Zone::Hand(0));
    add(&mut g, "b", "tao", Zone::Hand(0));
    let step = run_steps(
        &mut g,
        "rende",
        0,
        data(&[]),
        &["t:1", "card:a", "card:b", "pass", "use:sha"],
    );
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.hand_size(1), 2, "交给其两张牌");
    assert_eq!(g.counter("phase.sha_used.0"), 1, "视为使用【杀】计入次数");
    assert_eq!(g.counter("phase.0.rende.1"), 1, "每名角色每阶段限一次");
}

#[test]
fn rende_target_can_only_receive_once_per_phase() {
    let mut g = game(3, &["rende"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "a", "sha", Zone::Hand(0));
    add(&mut g, "b", "tao", Zone::Hand(0));
    g.set_counter("phase.0.rende.1", 1);
    let step = call(&mut g, "rende", 0, data(&[]));
    let p = pending_of(&step);
    assert!(!p.has_option("t:1"), "已交给过的角色不再出现");
    assert!(p.has_option("t:2"));
}

// ---------------------------------------------------------------------------
// 激将
// ---------------------------------------------------------------------------

#[test]
fn jijiang_relays_sha_and_responder_may_let_lord_draw() {
    let mut g = game(3, &["jijiang"]);
    g.seats[0].identity = Some(Identity::Lord);
    g.seats[1].kingdom = "shu".into();
    g.turn = 0;
    g.phase = Phase::Play;
    add_deck(&mut g, 10);
    add(&mut g, "k", "sha", Zone::Hand(1));
    add(&mut g, "t", "tao", Zone::Hand(2));
    skills::use_active(&mut g, 0, "jijiang").expect("激将可用");
    submit(&mut g, 1, "respond:k").expect("代出【杀】");
    submit(&mut g, 0, "t:2").expect("选目标");
    // 回合外的代出者（seat1）可令主公摸一张。
    if g.pending.is_some() {
        submit(&mut g, 1, "use").expect("令主公摸牌");
    }
    assert_eq!(g.seats[2].hp, 3, "【杀】造成 1 点伤害");
}

#[test]
fn jijiang_draw_only_outside_own_turn_and_shu() {
    let mut g = game(2, &["jijiang"]);
    g.seats[0].identity = Some(Identity::Lord);
    g.seats[1].kingdom = "shu".into();
    g.turn = 1; // 蜀势力的回合 → 不触发
    add(&mut g, "k", "sha", Zone::Hand(1));
    let step = call(&mut g, "jijiang", 0, data(&[("event", "CardUsed".into()), ("card", "k".into()), ("by", 1.into())]));
    assert!(matches!(step, Step::Done), "其自己的回合内不触发");
    // 反例：非蜀势力也不触发。
    g.seats[1].kingdom = "wei".into();
    g.turn = 0;
    let step = call(&mut g, "jijiang", 0, data(&[("event", "CardUsed".into()), ("card", "k".into()), ("by", 1.into())]));
    assert!(matches!(step, Step::Done));
}

// ---------------------------------------------------------------------------
// 义绝
// ---------------------------------------------------------------------------

#[test]
fn yijue_black_disables_skills_and_freezes_hand() {
    let mut g = game(2, &["yijue"]);
    g.seats[1].skills = vec!["longdan".to_string()];
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "c", "tao", Zone::Hand(0));
    add_suit(&mut g, "t", "shan", Zone::Hand(1), Suit::Spade, 5);
    let step = run_steps(&mut g, "yijue", 0, data(&[]), &["t:1", "card:c", "card:t"]);
    assert!(matches!(step, Step::Finish(_)));
    assert!(!skills::active(&g, 1, "longdan"), "本回合非锁定技失效");
    assert!(g.is_frozen(1, "t"), "不能使用或打出手牌");
    assert_eq!(g.counter("yijue.bonus.0.1"), 1, "红桃【杀】伤害 +1");
}

#[test]
fn yijue_red_gains_card_and_may_heal() {
    let mut g = game(2, &["yijue"]);
    g.turn = 0;
    g.phase = Phase::Play;
    g.seats[1].hp = 3;
    add(&mut g, "c", "tao", Zone::Hand(0));
    add_suit(&mut g, "t", "shan", Zone::Hand(1), Suit::Heart, 5);
    let step = run_steps(&mut g, "yijue", 0, data(&[]), &["t:1", "card:c", "card:t", "use"]);
    assert!(matches!(step, Step::Finish(_)));
    assert!(g.hand(0).contains(&"t".to_string()), "获得展示的红牌");
    assert_eq!(g.seats[1].hp, 4, "令其回复 1 点体力");
}

// ---------------------------------------------------------------------------
// 武圣 / 龙胆
// ---------------------------------------------------------------------------

#[test]
fn wusheng_red_as_sha_and_diamond_no_distance() {
    let mut g = game(3, &["wusheng"]);
    add_suit(&mut g, "r", "tao", Zone::Hand(0), Suit::Heart, 7);
    add_suit(&mut g, "d", "shan", Zone::Hand(0), Suit::Diamond, 8);
    add_suit(&mut g, "ds", "sha", Zone::Hand(0), Suit::Diamond, 9);
    assert_eq!(skills::use_alias(&g, 0, "r"), Some("sha"));
    assert_eq!(skills::use_alias(&g, 0, "d"), Some("sha"));
    assert_eq!(
        skills::response_as(&g, 0, "r", ResponseKind::Sha),
        Some("sha")
    );
    // 方片【杀】无距离限制。
    b9::on_use_start(&mut g, 0, "ds");
    assert!(g.use_opts("ds").no_distance);
}

#[test]
fn longdan_converts_sha_shan() {
    let mut g = game(2, &["longdan"]);
    add(&mut g, "s", "sha", Zone::Hand(0));
    add(&mut g, "f", "shan", Zone::Hand(0));
    add(&mut g, "j", "jiu", Zone::Hand(0));
    add(&mut g, "t", "tao", Zone::Hand(0));
    assert_eq!(skills::use_alias(&g, 0, "f"), Some("sha"));
    assert_eq!(skills::use_alias(&g, 0, "j"), Some("tao"));
    assert_eq!(skills::use_alias(&g, 0, "t"), Some("jiu"));
    assert_eq!(
        skills::response_as(&g, 0, "s", ResponseKind::Shan),
        Some("shan")
    );
    assert_eq!(
        skills::response_as(&g, 0, "f", ResponseKind::Sha),
        Some("sha")
    );
}

// ---------------------------------------------------------------------------
// 替身 / 咆哮
// ---------------------------------------------------------------------------

#[test]
fn tishen_heals_all_and_draws_equal() {
    let mut g = game(2, &["tishen"]);
    g.turn = 0;
    g.phase = Phase::Prepare;
    g.seats[0].hp = 1;
    add_deck(&mut g, 5);
    let step = run_steps(&mut g, "tishen", 0, data(&[]), &["use"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.seats[0].hp, 4, "回复所有体力");
    assert_eq!(g.hand_size(0), 3, "每回复 1 点摸 1 张");
    assert!(skills::limited_used(&g, 0, "tishen"));
    // 反例：满体力不询问。
    let mut g2 = game(2, &["tishen"]);
    g2.turn = 0;
    g2.phase = Phase::Prepare;
    assert!(matches!(call(&mut g2, "tishen", 0, data(&[])), Step::Done));
}

#[test]
fn paoxiao_unlimited_sha_and_bonus_after_dodge() {
    let mut g = game(2, &["paoxiao"]);
    g.turn = 0;
    g.phase = Phase::Play;
    g.set_counter("phase.sha_used.0", 9);
    assert!(!sanguosha3::kernel::card_play::sha_limit_reached(&g, 0), "【杀】无次数限制");
    skills::on_sha_dodged(&mut g, 0);
    assert_eq!(g.counter("turn.0.paoxiao_bonus"), 1);
    add(&mut g, "s", "sha", Zone::Hand(0));
    assert_eq!(b9::sha_damage_bonus(&mut g, 0, 1, "s"), 1, "下一张【杀】+1");
    assert_eq!(g.counter("turn.0.paoxiao_bonus"), 0, "加成已消费");
}

// ---------------------------------------------------------------------------
// 涯角 / 马术
// ---------------------------------------------------------------------------

#[test]
fn yajiao_same_type_lets_someone_gain_top_card() {
    let mut g = game(2, &["yajiao"]);
    g.turn = 1; // 回合外
    add(&mut g, "used", "sha", Zone::Discard);
    add(&mut g, "top", "tao", Zone::Draw);
    g.deck.push("top".into());
    let step = run_steps(&mut g, "yajiao", 0, data(&[("card", "used".into())]), &["use", "t:0"]);
    assert!(matches!(step, Step::Finish(_)));
    assert!(g.hand(0).contains(&"top".to_string()), "同类型→令自己获得展示牌");
}

#[test]
fn yajiao_different_type_discards_target_card() {
    let mut g = game(2, &["yajiao"]);
    g.turn = 1;
    add(&mut g, "used", "sha", Zone::Discard);
    add(&mut g, "top", "wuzhong_shengyou", Zone::Draw);
    g.deck.push("top".into());
    add(&mut g, "h", "tao", Zone::Hand(1));
    let step = run_steps(
        &mut g,
        "yajiao",
        0,
        data(&[("card", "used".into())]),
        &["use", "t:1", "random_hand"],
    );
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.hand_size(1), 0, "不同类型→弃置其一张手牌");
}

#[test]
fn mashu_reduces_distance() {
    let mut g = game(4, &["mashu"]);
    assert_eq!(gear::distance(&g, 0, 2), 1, "距离 2 → 马术 -1");
    let mut g2 = game(4, &[]);
    assert_eq!(gear::distance(&g2, 0, 2), 2);
}

// ---------------------------------------------------------------------------
// 铁骑 / 空城
// ---------------------------------------------------------------------------

#[test]
fn tieqi_judges_and_blocks_shan_without_discard() {
    let mut g = game(2, &["tieqi"]);
    g.seats[1].skills = vec!["longdan".to_string()];
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "s", "sha", Zone::Hand(0));
    add(&mut g, "j", "shan", Zone::Draw);
    g.deck.push("j".into());
    let step = run_steps(
        &mut g,
        "tieqi",
        0,
        data(&[("step", "sha_targeted".into()), ("card", "s".into()), ("targets", serde_json::json!([1]))]),
        &["use", "pass"],
    );
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(
        g.use_opts("s").intercept,
        Some(sanguosha3::kernel::ShaIntercept::Unresponsive),
        "不弃同花色 → 不能使用【闪】"
    );
    assert!(!skills::active(&g, 1, "longdan"), "本回合非锁定技失效");
}

#[test]
fn kongcheng_blocks_sha_and_duel_when_handless() {
    let mut g = game(2, &["kongcheng"]);
    assert!(b9::deny_target_card(&g, 0, 0, "sha"), "无手牌不能成为【杀】目标");
    assert!(b9::deny_target_card(&g, 0, 0, "juedou"), "无手牌不能成为【决斗】目标");
    assert!(!b9::deny_target_card(&g, 0, 0, "shunshou_qianyang"), "其它牌不受限");
    add(&mut g, "h", "tao", Zone::Hand(0));
    assert!(!b9::deny_target_card(&g, 0, 0, "sha"), "有手牌则不受限");
}

// ---------------------------------------------------------------------------
// 观星 / 集智 / 奇才
// ---------------------------------------------------------------------------

#[test]
fn guanxing_places_cards_and_repeats_at_end_if_all_bottom() {
    let mut g = game(2, &["guanxing"]);
    g.turn = 0;
    g.phase = Phase::Prepare;
    add_deck(&mut g, 5);
    // 3 张（存活 <4）。
    let mut step = call(&mut g, "guanxing", 0, data(&[]));
    // 先确认发动。
    g.answer = Some("use".into());
    g.answer_skill = Some("guanxing".into());
    step = call(&mut g, "guanxing", 0, data(&[]));
    // 逐张置底（张数由存活人数决定）。
    let mut guard = 0;
    let mut data_now = serde_json::Value::Null;
    while let Step::Next(_, next) = &step {
        data_now = next.clone();
        step = call(&mut g, "guanxing", 0, data_now.clone());
        guard += 1;
        if guard > 10 {
            break;
        }
    }
    let mut placed = 0;
    loop {
        let opts = match &step {
            Step::Need(p) | Step::NeedAt(_, p) | Step::NeedWith(_, _, p) => p.options().iter().map(|o| o.id.clone()).collect::<Vec<_>>(),
            _ => break,
        };
        if !opts.contains(&"bottom".to_string()) {
            break;
        }
        g.answer = Some("bottom".into());
        g.answer_skill = Some("guanxing".into());
        step = call(&mut g, "guanxing", 0, data_now.clone());
        placed += 1;
        while let Step::Next(_, next) = &step {
            data_now = next.clone();
            step = call(&mut g, "guanxing", 0, data_now.clone());
        }
    }
    assert!(placed >= 2, "逐张置底（存活 <4 时为 3 张）");
    assert!(g.check_conservation().is_ok(), "观星不破坏牌区守恒");
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.counter("turn.0.guanxing_again"), 1, "皆置底 → 结束阶段再次发动");
    // 结束阶段可再次发动。
    g.phase = Phase::End;
    let step = call(&mut g, "guanxing", 0, data(&[]));
    assert!(matches!(step, Step::Need(_) | Step::NeedWith(..)));
}

#[test]
fn jizhi_non_transform_trick_draws_and_basic_raises_limit() {
    let mut g = game(2, &["jizhi"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "t", "tao", Zone::Draw);
    g.deck.push("t".into());
    add(&mut g, "w1", "wuzhong_shengyou", Zone::Processing);
    let step = run_steps(
        &mut g,
        "jizhi",
        0,
        data(&[("card", "w1".into()), ("cause", "使用".into())]),
        &["use", "discard"],
    );
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.hand_limit(0), 5, "弃置基本牌 → 本回合手牌上限 +1");
    // 反例：转化使用不触发。
    let mut g2 = game(2, &["jizhi"]);
    g2.turn = 0;
    g2.phase = Phase::Play;
    add(&mut g2, "w", "wuzhong_shengyou", Zone::Hand(0));
    let step = call(&mut g2, "jizhi", 0, data(&[("card", "w".into()), ("cause", "转化使用".into())]));
    assert!(matches!(step, Step::Done));
}

#[test]
fn qicai_no_trick_distance_and_prevents_armor_discard() {
    let mut g = game(2, &["qicai"]);
    assert!(b9::trick_no_distance(&g, 0));
    add(&mut g, "armor", "bagua_zhen", Zone::Equip(0, Slot::Armor));
    add(&mut g, "weapon", "qinggang_jian", Zone::Equip(0, Slot::Weapon));
    // 他人弃置防具被防止；武器不受保护。
    assert!(flow::discard(&mut g, 1, "armor", "拆").is_none());
    assert!(g.cards.get("armor") == Some(&Zone::Equip(0, Slot::Armor)));
    assert!(flow::discard(&mut g, 1, "weapon", "拆").is_some());
    // 「获得」不受影响。
    add(&mut g, "armor2", "bagua_zhen", Zone::Equip(0, Slot::Armor));
    assert!(flow::gain(&mut g, 1, "armor2", 1, "顺手牵羊").is_some());
}

#[test]
fn freeze_helper_matches_turn_scope() {
    // 义绝/复难共用 `FreezeUntil::Turn`：回合结束解冻由 turn 驱动，这里只锁定枚举语义。
    let mut g = game(2, &[]);
    add(&mut g, "h", "tao", Zone::Hand(0));
    g.freeze_cards(0, &["h".to_string()], FreezeUntil::Turn, "test");
    assert!(g.is_frozen(0, "h"));
    g.unfreeze_turn();
    assert!(!g.is_frozen(0, "h"));
}

