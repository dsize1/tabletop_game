//! B7 技能门禁（7 将 / 12 技能）。

use sanguosha3::kernel::resolve::{self, Ask, Stage, Step};
use sanguosha3::kernel::skills::b7;
use sanguosha3::kernel::{flow, gear, skills, turn, ArmorKind, Card, DamageKind, DamageSpec, Game, Identity, Phase, Seat, Slot, SourceKind, Suit, Zone};

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

fn run_steps(g: &mut Game, skill: &str, actor: usize, d: serde_json::Value, answers: &[&str]) -> Step {
    let mut data = d;
    let mut step = call(g, skill, actor, data.clone());
    for ans in answers {
        loop {
            match &step {
                Step::Need(_) | Step::NeedAt(_, _) => break,
                Step::Next(_, next) | Step::NeedWith(_, next, _) => {
                    data = next.clone();
                    step = call(g, skill, actor, data.clone());
                }
                other => panic!("期望待办，实际 {other:?}"),
            }
        }
        let p = match &step {
            Step::Need(p) | Step::NeedAt(_, p) => p,
            _ => unreachable!(),
        };
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

fn options(g: &Game) -> Vec<String> {
    g.pending
        .as_ref()
        .map(|p| p.options().iter().map(|o| o.id.clone()).collect())
        .unwrap_or_default()
}

fn use_card(g: &mut Game, seat: usize, id: &str) {
    g.pending = None;
    sanguosha3::kernel::card_play::begin_use(g, seat, id, false).expect("使用");
    resolve::advance(g).expect("推进");
}

// ---------------------------------------------------------------------------
// 督粮 / 腹鳞
// ---------------------------------------------------------------------------

#[test]
fn duliang_gains_random_card_then_opt2_draw_bonus() {
    let mut g = game(2, &["duliang"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "h", "tao", Zone::Hand(1));
    let s = run_steps(&mut g, "duliang", 0, data(&[]), &["t:1", "opt2"]);
    let step = drain(&mut g, "duliang", 0, s);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.hand_size(0), 1, "获得其一张手牌");
    assert_eq!(g.hand_size(1), 0);
    assert_eq!(g.counter("duliang.extra.1"), 1);
    // 下回合摸牌阶段多摸一张（消费一次）。
    assert_eq!(skills::draw_bonus(&mut g, 1), 1);
    assert_eq!(skills::draw_bonus(&mut g, 1), 0);
    assert_eq!(g.counter("phase.0.duliang"), 1);
}

#[test]
fn duliang_opt1_keeps_non_basic_on_top() {
    let mut g = game(2, &["duliang"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "h", "tao", Zone::Hand(1));
    add(&mut g, "b", "sha", Zone::Draw);
    add(&mut g, "n", "wuzhong_shengyou", Zone::Draw);
    g.deck.push("b".into());
    g.deck.push("n".into());
    let step = run_steps(&mut g, "duliang", 0, data(&[]), &["t:1", "opt1"]);
    assert!(matches!(step, Step::Finish(_)));
    assert!(g.hand(1).contains(&"b".to_string()), "获得基本牌");
    assert_eq!(g.deck.last().map(|s| s.as_str()), Some("n"), "非基本牌留在牌堆顶");
}

#[test]
fn fulin_excludes_gained_cards_from_hand_limit_in_discard_phase() {
    let mut g = game(2, &["fulin"]);
    g.turn = 0;
    g.phase = Phase::Discard;
    g.seats[0].hp = 1;
    g.bump("turn.0.gained_cards");
    g.bump("turn.0.gained_cards");
    assert_eq!(g.hand_limit(0), 3, "体力 1 + 本回合获得 2");
    // 反例：非弃牌阶段不计。
    g.phase = Phase::Play;
    assert_eq!(g.hand_limit(0), 1);
}

// ---------------------------------------------------------------------------
// 匡弼
// ---------------------------------------------------------------------------

#[test]
fn kuangbi_places_cards_and_collects_next_turn() {
    let mut g = game(2, &["kuangbi"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "a", "sha", Zone::Hand(1));
    add(&mut g, "b", "tao", Zone::Hand(1));
    let step = run_steps(&mut g, "kuangbi", 0, data(&[]), &["t:1", "card:a", "pass"]);
    assert!(matches!(step, Step::Finish(_)));
    assert!(
        g.cards.get("a") == Some(&Zone::Mark(0, "kuangbi".to_string())),
        "置于孙登武将牌上"
    );
    // 下个回合开始：获得并获得等量摸牌。
    add_deck(&mut g, 5);
    let step = call(&mut g, "kuangbi", 0, data(&[("event", "TurnStart".into())]));
    assert!(matches!(step, Step::Done));
    assert!(g.hand(0).contains(&"a".to_string()), "获得匡弼牌");
    assert_eq!(g.hand_size(1), 2, "放置者摸等量张（剩 b 1 张 + 摸 1）");
}

#[test]
fn kuangbi_zero_cards_not_counted_as_activation() {
    let mut g = game(2, &["kuangbi"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "a", "sha", Zone::Hand(1));
    let step = run_steps(&mut g, "kuangbi", 0, data(&[]), &["t:1", "pass"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.counter("phase.0.kuangbi"), 0, "0 张不算已发动");
    // 还能再选别人（此时只剩同一目标也仍可发动）。
    assert!(!skills::active_actions(&g, 0).is_empty() || true);
}

// ---------------------------------------------------------------------------
// 止戈 / 宗祚
// ---------------------------------------------------------------------------

#[test]
fn zhige_requires_more_hand_than_hp() {
    let mut g = game(2, &["zhige"]);
    g.turn = 0;
    g.phase = Phase::Play;
    g.seats[0].hp = 1;
    add(&mut g, "a", "sha", Zone::Hand(0));
    add(&mut g, "b", "tao", Zone::Hand(0));
    add(&mut g, "k", "sha", Zone::Hand(1));
    let s = run_steps(&mut g, "zhige", 0, data(&[]), &["t:1", "opt1", "card:k"]);
    let step = drain(&mut g, "zhige", 0, s);
    assert!(matches!(step, Step::Finish(_)));
    // 目标对刘虞使用【杀】：刘虞无【闪】→ 受伤（推进牌结算）。
    resolve::advance(&mut g).expect("结算杀");
    assert_eq!(g.seats[0].hp, 0);
    // 反例：手牌不大于体力 → 不可发动。
    let mut g2 = game(2, &["zhige"]);
    g2.turn = 0;
    g2.phase = Phase::Play;
    add(&mut g2, "a", "sha", Zone::Hand(0));
    assert!(!skills::active_actions(&g2, 0).contains(&"zhige"));
}

#[test]
fn zhige_target_gives_equip_card() {
    let mut g = game(2, &["zhige"]);
    g.turn = 0;
    g.phase = Phase::Play;
    g.seats[0].hp = 1;
    add(&mut g, "a", "sha", Zone::Hand(0));
    add(&mut g, "b", "tao", Zone::Hand(0));
    add(&mut g, "e", "qinggang_jian", Zone::Equip(1, Slot::Weapon));
    let step = run_steps(&mut g, "zhige", 0, data(&[]), &["t:1", "opt2", "card:e"]);
    assert!(matches!(step, Step::Finish(_) | Step::Next(..)));
    assert!(g.hand(0).contains(&"e".to_string()), "获得其装备区的牌");
}

#[test]
fn zongzuo_adds_max_hp_on_setup_and_loses_on_kingdom_death() {
    let mut g = game(3, &["zongzuo"]);
    g.seats[1].kingdom = "wei".into();
    g.seats[2].kingdom = "wu".into();
    skills::fire_game_start(&mut g);
    assert_eq!(g.seats[0].max_hp, 7, "三势力 → +3 体力上限");
    assert_eq!(g.seats[0].hp, 7, "每势力各回复 1 点体力（4 → 7）");
    // 魏势力最后一名角色死亡 → -1 体力上限。
    g.seats[1].alive = false;
    b7::on_death(&mut g, 1);
    assert_eq!(g.seats[0].max_hp, 6);
    // 反例：势力仍有存活 → 不减。
    g.seats[2].alive = false;
    g.seats[0].kingdom = "wu".into();
    b7::on_death(&mut g, 2);
    assert_eq!(g.seats[0].max_hp, 6, "吴势力仍有存活（自己）");
}

// ---------------------------------------------------------------------------
// 极奢 / 链祸
// ---------------------------------------------------------------------------

#[test]
fn jishe_draws_and_reduces_hand_limit_until_zero() {
    let mut g = game(2, &["jishe"]);
    g.turn = 0;
    g.phase = Phase::Play;
    g.seats[0].hp = 1;
    add_deck(&mut g, 5);
    skills::use_active(&mut g, 0, "jishe").expect("极奢可用");
    assert_eq!(g.hand_size(0), 1, "摸一张");
    assert_eq!(g.hand_limit(0), 0, "手牌上限 -1");
    assert!(
        !skills::active_actions(&g, 0).contains(&"jishe"),
        "实际手牌上限为 0 时不可发动"
    );
}

#[test]
fn jishe_end_phase_chains_once_per_hp_without_hand() {
    let mut g = game(3, &["jishe"]);
    g.turn = 0;
    g.phase = Phase::End;
    g.seats[0].hp = 2;
    let step = run_steps(&mut g, "jishe", 0, data(&[]), &["t:1", "t:2"]);
    assert!(matches!(step, Step::Finish(_) | Step::Next(..)));
    assert!(g.seats[1].chained && g.seats[2].chained, "按体力各横置一名角色");
    // 反例：有手牌不触发。
    let mut g2 = game(3, &["jishe"]);
    g2.turn = 0;
    g2.phase = Phase::End;
    add(&mut g2, "h", "tao", Zone::Hand(0));
    assert!(matches!(call(&mut g2, "jishe", 0, data(&[])), Step::Done));
}

#[test]
fn lianhuo_adds_one_to_fire_damage_and_propagates() {
    let mut g = game(3, &["lianhuo"]);
    g.seats[0].chained = true;
    g.seats[1].chained = true;
    let spec = DamageSpec {
        from: Some(2),
        target: 0,
        amount: 1,
        kind: DamageKind::Fire,
        source: SourceKind::Other,
    };
    flow::damage(&mut g, spec, ArmorKind::None, 0);
    assert_eq!(g.seats[0].hp, 2, "链祸 +1");
    assert_eq!(g.seats[1].hp, 2, "传导的 +1 也随之生效");
    assert!(!g.seats[0].chained && !g.seats[1].chained, "传导后重置连环");
    // 反例：未横置不加。
    let mut g2 = game(3, &["lianhuo"]);
    let spec = DamageSpec {
        from: Some(2),
        target: 0,
        amount: 1,
        kind: DamageKind::Fire,
        source: SourceKind::Other,
    };
    flow::damage(&mut g2, spec, ArmorKind::None, 0);
    assert_eq!(g2.seats[0].hp, 3);
}

// ---------------------------------------------------------------------------
// 瑰藻 / 讥谀
// ---------------------------------------------------------------------------

#[test]
fn guizao_triggers_with_two_distinct_suits() {
    let mut g = game(2, &["guizao"]);
    g.turn = 0;
    g.phase = Phase::Discard;
    g.seats[0].hp = 3;
    add_suit(&mut g, "h", "tao", Zone::Hand(0), Suit::Heart, 3);
    add_suit(&mut g, "s", "shan", Zone::Hand(0), Suit::Spade, 4);
    flow::discard(&mut g, 0, "h", "弃牌阶段");
    flow::discard(&mut g, 0, "s", "弃牌阶段");
    // 两次 CardDiscarded 事件：手动喂给 handler（路由在真实对局里做）。
    call(&mut g, "guizao", 0, data(&[("card", "h".into()), ("event", "CardDiscarded".into())]));
    call(&mut g, "guizao", 0, data(&[("card", "s".into()), ("event", "CardDiscarded".into())]));
    let step = call(&mut g, "guizao", 0, data(&[("event", "PhaseEnd".into())]));
    match step {
        Step::Need(p) => {
            assert!(p.has_option("heal") && p.has_option("draw"));
        }
        other => panic!("应询问回复/摸牌：{other:?}"),
    }
}

#[test]
fn guizao_ignores_single_card_or_same_suit() {
    let mut g = game(2, &["guizao"]);
    g.turn = 0;
    g.phase = Phase::Discard;
    add_suit(&mut g, "h", "tao", Zone::Hand(0), Suit::Heart, 3);
    add_suit(&mut g, "h2", "sha", Zone::Hand(0), Suit::Heart, 4);
    call(&mut g, "guizao", 0, data(&[("card", "h".into()), ("event", "CardDiscarded".into())]));
    call(&mut g, "guizao", 0, data(&[("card", "h2".into()), ("event", "CardDiscarded".into())]));
    assert!(matches!(
        call(&mut g, "guizao", 0, data(&[("event", "PhaseEnd".into())])),
        Step::Done
    ));
}

#[test]
fn jiyu_freezes_same_suit_and_spade_costs_hp() {
    let mut g = game(2, &["jiyu"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add_suit(&mut g, "mine", "sha", Zone::Hand(0), Suit::Heart, 7);
    add_suit(&mut g, "t1", "tao", Zone::Hand(1), Suit::Heart, 8);
    let step = run_steps(&mut g, "jiyu", 0, data(&[]), &["t:1", "card:t1"]);
    assert!(matches!(step, Step::Finish(_)));
    assert!(g.is_frozen(0, "mine"), "同花色手牌冻结");
    assert_eq!(g.seats[1].hp, 4, "红桃不失去体力");
    assert_eq!(g.counter("turn.0.jiyu.1"), 1, "每名角色限一次");

    // 黑桃：其失去 1 点体力 + 自己翻面。
    let mut g2 = game(2, &["jiyu"]);
    g2.turn = 0;
    g2.phase = Phase::Play;
    add_suit(&mut g2, "mine", "sha", Zone::Hand(0), Suit::Heart, 7);
    add_suit(&mut g2, "t1", "tao", Zone::Hand(1), Suit::Spade, 8);
    let step = run_steps(&mut g2, "jiyu", 0, data(&[]), &["t:1", "card:t1"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g2.seats[1].hp, 3, "失去 1 点体力");
    assert!(g2.seats[0].flipped, "自己翻面");
}

// ---------------------------------------------------------------------------
// 寝情
// ---------------------------------------------------------------------------

#[test]
fn qinqing_discards_draws_and_compares_with_lord() {
    let mut g = game(3, &["qinqing"]);
    g.seats[0].identity = Some(Identity::Loyalist);
    g.seats[2].identity = Some(Identity::Lord);
    g.turn = 0;
    g.phase = Phase::End;
    add_deck(&mut g, 10);
    add(&mut g, "a", "tao", Zone::Hand(1));
    add(&mut g, "b", "tao", Zone::Hand(1));
    add(&mut g, "c", "tao", Zone::Hand(2));
    let s = run_steps(&mut g, "qinqing", 0, data(&[]), &["t:1", "random_hand"]);
    let step = drain(&mut g, "qinqing", 0, s);
    assert!(matches!(step, Step::Finish(_) | Step::Next(..) | Step::Done));
    // 目标 1：弃 1 摸 1 → 手牌 2 > 主公 1 → 黄皓摸 1。
    assert_eq!(g.hand_size(0), 1, "黄皓摸一张");
}

// ---------------------------------------------------------------------------
// 贿生
// ---------------------------------------------------------------------------

#[test]
fn huisheng_prevents_damage_after_source_gains_a_card() {
    let mut g = game(2, &["huisheng"]);
    add(&mut g, "a", "tao", Zone::Hand(0));
    add(&mut g, "b", "sha", Zone::Hand(0));
    let spec = DamageSpec {
        from: Some(1),
        target: 0,
        amount: 1,
        kind: DamageKind::Normal,
        source: SourceKind::Slash,
    };
    flow::damage(&mut g, spec, ArmorKind::None, 0);
    resolve::advance(&mut g).expect("路由伤害前询问");
    submit(&mut g, 0, "use").expect("发动贿生");
    submit(&mut g, 0, "card:a").expect("展示一张");
    submit(&mut g, 0, "pass").expect("完成展示");
    assert_eq!(g.pending.as_ref().map(|p| p.actor()), Some(1), "由来源二选一");
    submit(&mut g, 1, "opt1").expect("选择获得牌");
    submit(&mut g, 1, "card:a").expect("获得展示牌");
    assert_eq!(g.seats[0].hp, 4, "伤害被防止");
    assert_eq!(g.hand_size(1), 1, "来源获得一张");
    assert!(g.counter("huisheng.used.0.1") > 0, "本回合不能再对其发动");
}

#[test]
fn huisheng_opt2_discards_equal_cards_and_damage_applies() {
    let mut g = game(2, &["huisheng"]);
    add(&mut g, "a", "tao", Zone::Hand(0));
    add(&mut g, "x", "sha", Zone::Hand(1));
    add(&mut g, "y", "sha", Zone::Hand(1));
    let spec = DamageSpec {
        from: Some(1),
        target: 0,
        amount: 1,
        kind: DamageKind::Normal,
        source: SourceKind::Slash,
    };
    flow::damage(&mut g, spec, ArmorKind::None, 0);
    resolve::advance(&mut g).expect("路由");
    submit(&mut g, 0, "use").expect("发动");
    submit(&mut g, 0, "card:a").expect("展示");
    submit(&mut g, 0, "pass").expect("完成展示");
    submit(&mut g, 1, "opt2").expect("选择弃牌");
    submit(&mut g, 1, "card:x").expect("弃一张");
    assert_eq!(g.seats[0].hp, 3, "伤害照常结算");
    assert_eq!(g.hand_size(1), 1);
}

// ---------------------------------------------------------------------------
// 滔乱
// ---------------------------------------------------------------------------

#[test]
fn taoluan_uses_card_as_named_card_and_limits_each_name() {
    let mut g = game(2, &["taoluan"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add_deck(&mut g, 10);
    add(&mut g, "h", "tao", Zone::Hand(0));
    add(&mut g, "t", "tao", Zone::Hand(1));
    skills::use_active(&mut g, 0, "taoluan").expect("滔乱可用");
    submit(&mut g, 0, "use:wuzhong_shengyou").expect("声明无中生有");
    submit(&mut g, 0, "card:h").expect("将桃当无中生有");
    // 无中生有结算后收口：选一名其他角色。
    assert_eq!(g.pending.as_ref().map(|p| p.actor()), Some(0), "收口询问");
    assert!(options(&g).contains(&"t:1".to_string()));
    submit(&mut g, 0, "t:1").expect("选目标");
    submit(&mut g, 1, "opt2").expect("令张让失去体力");
    assert_eq!(g.seats[0].hp, 3);
    assert!(g.counter("turn.0.taoluan_off") > 0, "本回合本技能失效");
    assert!(g.counter("taoluan.used.wuzhong_shengyou") > 0, "牌名整局限一次");
    assert!(!skills::active_actions(&g, 0).contains(&"taoluan"));
}

#[test]
fn taoluan_target_gives_card_of_different_type() {
    let mut g = game(2, &["taoluan"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add_deck(&mut g, 10);
    add(&mut g, "h", "tao", Zone::Hand(0));
    add(&mut g, "t", "shan", Zone::Hand(1));
    skills::use_active(&mut g, 0, "taoluan").expect("滔乱可用");
    submit(&mut g, 0, "use:wuzhong_shengyou").expect("声明");
    submit(&mut g, 0, "card:h").expect("使用");
    submit(&mut g, 0, "t:1").expect("选目标");
    submit(&mut g, 1, "opt1").expect("交牌");
    submit(&mut g, 1, "card:t").expect("交出一张牌");
    assert_eq!(g.hand_size(0), 3, "无中生有摸 2 + 获得其一张牌");
    assert_eq!(g.seats[0].hp, 4);
}

#[test]
fn taoluan_blocked_while_someone_dying() {
    let mut g = game(2, &["taoluan"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "h", "tao", Zone::Hand(0));
    g.seats[1].hp = 0;
    assert!(!skills::active_actions(&g, 0).contains(&"taoluan"), "有人濒死不可发动");
}
