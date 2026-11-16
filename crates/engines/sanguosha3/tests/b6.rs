//! B6 技能门禁（7 将 / 12 技能）。

use sanguosha3::kernel::cards::ResponseKind;
use sanguosha3::kernel::resolve::{self, Ask, Stage, Step};
use sanguosha3::kernel::{flow, gear, skills, ArmorKind, Card, DamageKind, DamageSpec, Game, Identity, Phase, Seat, Slot, SourceKind, Suit, Zone};

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
    add_suit(g, id, def, zone, Suit::Spade, 7);
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

/// 反复「注入回答 → 重新调用 handler」，直到产生新的待办。
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

/// 沿 `Next` 链继续调用 handler 直到不再是 Next（没有新待办时用于收尾）。
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

fn use_card(g: &mut Game, seat: usize, id: &str) -> Result<(), String> {
    g.pending = None;
    sanguosha3::kernel::card_play::begin_use(g, seat, id, false)?;
    resolve::advance(g)
}

// ---------------------------------------------------------------------------
// 战绝
// ---------------------------------------------------------------------------

#[test]
fn zhanjue_all_hand_as_duel_and_both_draw_once() {
    let mut g = game(2, &["zhanjue"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add_deck(&mut g, 10);
    add(&mut g, "a", "sha", Zone::Hand(0));
    add(&mut g, "b", "tao", Zone::Hand(0));
    add(&mut g, "c", "tao", Zone::Hand(1));
    skills::use_active(&mut g, 0, "zhanjue").expect("战绝可用");
    // 决斗目标选择。
    assert_eq!(g.pending.as_ref().map(|p| p.actor()), Some(0));
    submit(&mut g, 0, "t:1").expect("选目标");
    // 目标无【杀】→ 受到 1 点伤害；刘谌与受伤者各摸 1。
    assert_eq!(g.seats[1].hp, 3);
    assert_eq!(g.hand_size(0), 1, "两张手牌当【决斗】后摸 1");
    assert_eq!(g.hand_size(1), 2, "受伤者摸 1");
    assert_eq!(g.counter("turn.0.zhanjue_uses"), 1);
    // 反例：无手牌时不可发动。
    let mut g2 = game(2, &["zhanjue"]);
    g2.turn = 0;
    g2.phase = Phase::Play;
    assert!(skills::use_active(&mut g2, 0, "zhanjue").is_err());
}

#[test]
fn zhanjue_disables_after_two_activations() {
    let mut g = game(2, &["zhanjue"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add_deck(&mut g, 10);
    add(&mut g, "a", "sha", Zone::Hand(0));
    g.set_counter("turn.0.zhanjue_uses", 2);
    assert!(
        !skills::active_actions(&g, 0).contains(&"zhanjue"),
        "发动两次后本阶段失效"
    );
}

// ---------------------------------------------------------------------------
// 勤王
// ---------------------------------------------------------------------------

#[test]
fn qinwang_relays_sha_and_responder_draws() {
    let mut g = game(3, &["qinwang"]);
    g.seats[0].identity = Some(Identity::Lord);
    g.seats[1].kingdom = "shu".into();
    g.turn = 0;
    g.phase = Phase::Play;
    add_deck(&mut g, 10);
    add(&mut g, "a", "shan", Zone::Hand(0));
    add(&mut g, "k", "sha", Zone::Hand(1));
    add(&mut g, "t", "tao", Zone::Hand(2));
    let before1 = g.hand_size(1);
    skills::use_active(&mut g, 0, "qinwang").expect("勤王可用");
    submit(&mut g, 0, "card:a").expect("弃一张发动激将");
    submit(&mut g, 1, "respond:k").expect("代出杀");
    // 视为刘谌使用【杀】。
    submit(&mut g, 0, "t:2").expect("选目标");
    assert_eq!(g.seats[2].hp, 3, "目标无【闪】受到 1 点伤害");
    assert_eq!(g.hand_size(1), before1, "代出【杀】+ 响应摸 1 = 持平");
    // 反例：非主公不可发动。
    let mut g2 = game(3, &["qinwang"]);
    g2.turn = 0;
    g2.phase = Phase::Play;
    add(&mut g2, "a", "shan", Zone::Hand(0));
    assert!(!skills::active_actions(&g2, 0).contains(&"qinwang"));
}

// ---------------------------------------------------------------------------
// 怃戎
// ---------------------------------------------------------------------------

#[test]
fn wurong_sha_vs_non_shan_deals_damage() {
    let mut g = game(2, &["wurong"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "a", "sha", Zone::Hand(0));
    add(&mut g, "b", "tao", Zone::Hand(1));
    let step = run_steps(&mut g, "wurong", 0, data(&[]), &["t:1", "card:a", "card:b"]);
    assert!(matches!(step, Step::Finish(_) | Step::Next(..)));
    assert_eq!(g.seats[1].hp, 3, "弃【杀】并造成 1 点伤害");
    assert_eq!(g.counter("phase.0.wurong"), 1);
}

#[test]
fn wurong_non_sha_vs_shan_gains_card() {
    let mut g = game(2, &["wurong"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "a", "tao", Zone::Hand(0));
    add(&mut g, "b", "shan", Zone::Hand(1));
    let step = run_steps(
        &mut g,
        "wurong",
        0,
        data(&[]),
        &["t:1", "card:a", "card:b", "card:b"],
    );
    assert!(matches!(step, Step::Finish(_) | Step::Next(..)));
    assert_eq!(g.hand_size(0), 1, "弃展示牌后获得目标一张牌");
    assert_eq!(g.hand_size(1), 0);
}

#[test]
fn wurong_other_combinations_do_nothing() {
    let mut g = game(2, &["wurong"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "a", "sha", Zone::Hand(0));
    add(&mut g, "b", "shan", Zone::Hand(1));
    let step = run_steps(&mut g, "wurong", 0, data(&[]), &["t:1", "card:a", "card:b"]);
    assert!(matches!(step, Step::Finish(_) | Step::Next(..)));
    assert_eq!(g.seats[1].hp, 4, "杀+闪 无事发生");
    assert_eq!(g.hand_size(0), 1, "展示牌不弃置");
}

// ---------------------------------------------------------------------------
// 矢志
// ---------------------------------------------------------------------------

#[test]
fn shizhi_replaces_shan_with_sha_at_one_hp() {
    let mut g = game(2, &["shizhi"]);
    g.seats[0].hp = 1;
    add(&mut g, "s", "shan", Zone::Hand(0));
    assert_eq!(skills::use_alias(&g, 0, "s"), Some("sha"));
    assert_eq!(skills::response_as(&g, 0, "s", ResponseKind::Sha), Some("sha"));
    assert_eq!(
        skills::response_as(&g, 0, "s", ResponseKind::Shan),
        None,
        "体力 1 时【闪】不能再当【闪】"
    );
    // 反例：体力 2 时仍是【闪】。
    g.seats[0].hp = 2;
    assert_eq!(skills::use_alias(&g, 0, "s"), None);
    assert_eq!(skills::response_as(&g, 0, "s", ResponseKind::Shan), Some("shan"));
    assert_eq!(skills::response_as(&g, 0, "s", ResponseKind::Sha), None);
}

// ---------------------------------------------------------------------------
// 宴诛 / 兴学
// ---------------------------------------------------------------------------

#[test]
fn yanzhu_opt1_takes_equips_and_upgrades_xingxue() {
    let mut g = game(2, &["yanzhu", "xingxue"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "e1", "qinggang_jian", Zone::Equip(1, Slot::Weapon));
    add(&mut g, "e2", "bagua_zhen", Zone::Equip(1, Slot::Armor));
    let step = run_steps(&mut g, "yanzhu", 0, data(&[]), &["t:1", "opt1"]);
    assert!(matches!(step, Step::Finish(_) | Step::Next(..)));
    assert_eq!(g.hand_size(0), 2, "获得其装备区所有牌");
    assert_eq!(g.counter("xingxue.modified.0"), 1, "修改兴学");
    assert!(!g.seats[0].skills.iter().any(|s| s == "yanzhu"), "失去宴诛");
}

#[test]
fn yanzhu_opt2_target_discards_one_card() {
    let mut g = game(2, &["yanzhu"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "h", "tao", Zone::Hand(1));
    let step = run_steps(&mut g, "yanzhu", 0, data(&[]), &["t:1", "opt2", "card:h"]);
    assert!(matches!(step, Step::Finish(_) | Step::Next(..)));
    assert_eq!(g.hand_size(1), 0);
    assert_eq!(g.counter("phase.0.yanzhu"), 1);
}

#[test]
fn xingxue_draws_then_owner_puts_card_on_top() {
    let mut g = game(2, &["xingxue"]);
    g.turn = 0;
    g.phase = Phase::End;
    g.seats[0].hp = 1;
    add(&mut g, "a", "tao", Zone::Hand(1));
    add(&mut g, "z", "sha", Zone::Draw);
    add(&mut g, "y", "sha", Zone::Draw);
    g.deck.push("z".into());
    g.deck.push("y".into());
    let step = run_steps(&mut g, "xingxue", 0, data(&[]), &["t:1", "card:a"]);
    assert!(matches!(step, Step::Finish(_) | Step::Next(..)));
    assert_eq!(g.deck.last().map(|s| s.as_str()), Some("a"), "该角色自选一张置顶");
    assert_eq!(g.hand_size(1), 1, "摸 1（手牌 2）后置顶 1 → 1");
}

#[test]
fn xingxue_modified_settles_once() {
    let mut g = game(2, &["xingxue"]);
    g.turn = 0;
    g.phase = Phase::End;
    g.seats[0].hp = 4;
    g.set_counter("xingxue.modified.0", 1);
    add(&mut g, "a", "tao", Zone::Hand(1));
    add(&mut g, "z", "sha", Zone::Draw);
    g.deck.push("z".into());
    let step = run_steps(&mut g, "xingxue", 0, data(&[]), &["t:1", "card:a"]);
    assert!(matches!(step, Step::Finish(_) | Step::Next(..)), "修改后只结算 1 次");
}

// ---------------------------------------------------------------------------
// 诏缚
// ---------------------------------------------------------------------------

#[test]
fn zhaofu_marks_seats_and_extends_attack_range() {
    let mut g = game(3, &["zhaofu"]);
    g.seats[0].identity = Some(Identity::Lord);
    g.seats[1].kingdom = "wu".into();
    g.seats[2].kingdom = "wei".into();
    g.turn = 0;
    g.phase = Phase::Play;
    let step = run_steps(&mut g, "zhaofu", 0, data(&[]), &["t:0", "t:2"]);
    assert!(matches!(step, Step::Finish(_) | Step::Next(..)));
    assert!(skills::limited_used(&g, 0, "zhaofu"), "限定技已使用");
    assert!(skills::in_attack_range(&g, 1, 2), "吴势力视为在范围内");
    assert!(!skills::in_attack_range(&g, 2, 1), "非吴势力不受益");
    assert!(gear::within_attack_range(&g, 1, 2, 1), "统一入口生效");
    // 反例：非主公不可发动。
    let mut g2 = game(3, &["zhaofu"]);
    g2.turn = 0;
    g2.phase = Phase::Play;
    assert!(!skills::active_actions(&g2, 0).contains(&"zhaofu"));
}

// ---------------------------------------------------------------------------
// 安国
// ---------------------------------------------------------------------------

#[test]
fn anguo_three_minimum_conditions() {
    let mut g = game(3, &["anguo"]);
    g.turn = 0;
    g.phase = Phase::Play;
    g.seats[1].hp = 2;
    add(&mut g, "h1", "tao", Zone::Hand(0));
    add(&mut g, "h2", "tao", Zone::Hand(0));
    add(&mut g, "h3", "tao", Zone::Hand(2));
    add(&mut g, "w", "qinggang_jian", Zone::Draw);
    g.deck.push("w".into());
    add_deck(&mut g, 5);
    let s = run_steps(&mut g, "anguo", 0, data(&[]), &["t:1"]);
    let step = drain(&mut g, "anguo", 0, s);
    assert!(matches!(step, Step::Finish(_) | Step::Next(..)));
    assert_eq!(g.hand_size(1), 1, "手牌最少 → 摸 1");
    assert_eq!(g.seats[1].hp, 3, "体力最低 → 回 1");
    assert!(gear::equip_in(&g, 1, Slot::Weapon).is_some(), "装备最少 → 使用牌堆装备");
}

// ---------------------------------------------------------------------------
// 邀名
// ---------------------------------------------------------------------------

#[test]
fn yaoming_after_damage_dealt_once_per_turn() {
    let mut g = game(2, &["yaoming"]);
    add(&mut g, "a", "tao", Zone::Hand(1));
    add(&mut g, "b", "tao", Zone::Hand(1));
    let spec = DamageSpec {
        from: Some(0),
        target: 1,
        amount: 1,
        kind: DamageKind::Normal,
        source: SourceKind::Other,
    };
    flow::damage(&mut g, spec, ArmorKind::None, 0);
    resolve::advance(&mut g).expect("路由伤害事件");
    assert_eq!(g.pending.as_ref().map(|p| p.actor()), Some(0));
    submit(&mut g, 0, "discard").expect("选择弃牌");
    submit(&mut g, 0, "t:1").expect("选目标");
    let opts = options(&g);
    assert!(opts.contains(&"card:a".to_string()), "由全琮选牌：{opts:?}");
    submit(&mut g, 0, "card:a").expect("弃置其一张手牌");
    assert_eq!(g.hand_size(1), 1);
    assert_eq!(g.counter("turn.0.yaoming_dealt"), 1);
    // 反例：同方向第二次伤害不再触发。
    flow::damage(
        &mut g,
        DamageSpec {
            from: Some(0),
            target: 1,
            amount: 1,
            kind: DamageKind::Normal,
            source: SourceKind::Other,
        },
        ArmorKind::None,
        0,
    );
    resolve::advance(&mut g).expect("路由");
    assert_eq!(g.pending.as_ref().map(|p| p.actor()), None);
}

#[test]
fn yaoming_taken_and_dealt_are_separate() {
    let mut g = game(2, &["yaoming"]);
    add(&mut g, "a", "tao", Zone::Hand(0));
    add(&mut g, "b", "tao", Zone::Hand(0));
    add(&mut g, "c", "tao", Zone::Hand(1));
    // 受到伤害（他人造成）。
    flow::damage(
        &mut g,
        DamageSpec {
            from: Some(1),
            target: 0,
            amount: 1,
            kind: DamageKind::Normal,
            source: SourceKind::Other,
        },
        ArmorKind::None,
        0,
    );
    resolve::advance(&mut g).expect("路由");
    assert_eq!(g.pending.as_ref().map(|p| p.actor()), Some(0));
    submit(&mut g, 0, "draw").expect("选择摸牌");
    submit(&mut g, 0, "t:1").expect("选目标");
    assert_eq!(g.counter("turn.0.yaoming_taken"), 1);
    assert_eq!(g.counter("turn.0.yaoming_dealt"), 0);
}

// ---------------------------------------------------------------------------
// 怀异
// ---------------------------------------------------------------------------

#[test]
fn huaiyi_discards_color_and_loses_hp_after_two_gains() {
    let mut g = game(2, &["huaiyi"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add_suit(&mut g, "r1", "sha", Zone::Hand(0), Suit::Heart, 7);
    add_suit(&mut g, "r2", "tao", Zone::Hand(0), Suit::Diamond, 8);
    add(&mut g, "b1", "tao", Zone::Hand(1));
    add(&mut g, "b2", "shan", Zone::Hand(1));
    let s = run_steps(&mut g, "huaiyi", 0, data(&[]), &["red", "t:1", "card:b1", "t:1", "card:b2"]);
    let step = drain(&mut g, "huaiyi", 0, s);
    assert!(matches!(step, Step::Finish(_) | Step::Next(..)));
    assert_eq!(g.hand_size(0), 2, "获得两张牌");
    assert_eq!(g.seats[0].hp, 3, "获得 ≥2 张 → 失去 1 点体力");
    assert_eq!(g.counter("phase.0.huaiyi"), 1);
}

#[test]
fn huaiyi_one_gain_no_hp_loss() {
    let mut g = game(2, &["huaiyi"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add_suit(&mut g, "r1", "sha", Zone::Hand(0), Suit::Heart, 7);
    add(&mut g, "b1", "tao", Zone::Hand(1));
    let s = run_steps(&mut g, "huaiyi", 0, data(&[]), &["red", "t:1", "card:b1"]);
    let step = drain(&mut g, "huaiyi", 0, s);
    assert!(matches!(step, Step::Finish(_) | Step::Next(..)));
    assert_eq!(g.seats[0].hp, 4, "只获得 1 张不失去体力");
}

// ---------------------------------------------------------------------------
// 急攻
// ---------------------------------------------------------------------------

#[test]
fn jigong_assigns_hand_limit_to_damage_dealt() {
    let mut g = game(2, &["jigong"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add_deck(&mut g, 5);
    let step = run_steps(&mut g, "jigong", 0, data(&[("step", "choose".into())]), &["opt1"]);
    assert!(matches!(step, Step::Finish(_) | Step::Next(..)));
    assert_eq!(g.hand_size(0), 2, "摸两张");
    assert_eq!(g.hand_limit(0), 0, "尚未造成伤害 → 手牌上限 0");
    g.bump("turn.0.damage_dealt");
    assert_eq!(g.hand_limit(0), 1, "赋值型覆盖");
    // 反例：不发动时按体力值。
    let mut g2 = game(2, &["jigong"]);
    g2.turn = 0;
    g2.phase = Phase::Play;
    assert_eq!(g2.hand_limit(0), 4);
}

// ---------------------------------------------------------------------------
// 饰非
// ---------------------------------------------------------------------------

#[test]
fn shifei_offers_alternative_when_needing_shan() {
    let mut g = game(2, &[]);
    g.seats[1].skills = vec!["shifei".to_string()];
    g.turn = 1;
    g.phase = Phase::Play;
    add_deck(&mut g, 5);
    add(&mut g, "s", "sha", Zone::Hand(0));
    add(&mut g, "x", "tao", Zone::Hand(1));
    add(&mut g, "y", "tao", Zone::Hand(1));
    use_card(&mut g, 0, "s").expect("使用杀");
    submit(&mut g, 0, "t:1").expect("选目标");
    let opts = options(&g);
    assert!(opts.contains(&"alt:shifei".to_string()), "提供饰非选项：{opts:?}");
    submit(&mut g, 1, "alt:shifei").expect("发动饰非");
    assert_eq!(g.seats[1].hp, 4, "视为使用【闪】，不受伤");
    assert_eq!(g.hand_size(1), 3, "当前回合角色摸 1");
}

#[test]
fn shifei_discards_from_most_hand_character() {
    let mut g = game(3, &[]);
    g.seats[0].skills = vec!["shifei".to_string()];
    g.turn = 1;
    g.phase = Phase::Play;
    add(&mut g, "a", "tao", Zone::Hand(0));
    add(&mut g, "b", "tao", Zone::Hand(0));
    add(&mut g, "c", "tao", Zone::Hand(0));
    let step = run_steps(&mut g, "shifei", 0, data(&[("step", "draw".into())]), &["card:a"]);
    assert!(matches!(step, Step::Finish(_) | Step::Next(..)));
    assert_eq!(g.hand_size(0), 2, "弃置最多者（自己）一张牌");
    assert_eq!(g.mark("response.sub.0"), Some("1"), "成功标记（视为闪）");
}
