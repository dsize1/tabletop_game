//! B11 技能门禁（界标·魏：8 将 / 14 技能）+ 内核 `judge-rewrite`。

use sanguosha3::kernel::cards::ResponseKind;
use sanguosha3::kernel::resolve::{self, Ask, Stage, Step};
use sanguosha3::kernel::{flow, skills, Card, Game, Identity, Phase, Seat, Suit, Zone};

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
    let mut seats: Vec<Seat> = (0..n).map(|i| seat_with(i, &[], "wei")).collect();
    seats[0] = seat_with(0, s0, "wei");
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

/// 走真实栈：压技能帧 + 推进（需要流程帧/子帧的技能必须用它）。
fn trigger(g: &mut Game, skill: &str, actor: usize, d: serde_json::Value) {
    resolve::push_ask(g, skill, Stage::Choose, actor, d);
    resolve::advance(g).expect("advance");
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
                other => panic!("期望待办，实际 {other:?}"),
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

/// 非出口选项（`pass` 等出口不计入）。
fn pending_ids(g: &Game) -> Vec<String> {
    g.pending
        .as_ref()
        .map(|p| {
            p.options()
                .iter()
                .map(|o| o.id.clone())
                .filter(|id| id != "pass")
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// 界曹操 · 奸雄
// ---------------------------------------------------------------------------

#[test]
fn jianxiong_draws_one_and_can_gain_damage_card() {
    let mut g = game(2, &["jianxiong"]);
    add_deck(&mut g, 2);
    add(&mut g, "dmg", "sha", Zone::Processing);
    let step = call(
        &mut g,
        "jianxiong",
        0,
        data(&[("event", "DamageAfter".into()), ("victim", 0.into()), ("by", 1.into()), ("card", "dmg".into()), ("amount", 1.into())]),
    );
    // 解耦：先摸 1（无论是否获得伤害牌）。
    assert_eq!(g.hand_size(0), 1, "受到伤害即摸 1");
    assert!(matches!(step, Step::Next(..)), "进入「是否获得」阶段");
    g.answer = Some("gain".into());
    g.answer_skill = Some("jianxiong".into());
    let step = call(&mut g, "jianxiong", 0, data(&[("step", "gain".into()), ("card", "dmg".into())]));
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.hand_size(0), 2, "获得造成伤害的牌");
    assert!(g.hand(0).contains(&"dmg".to_string()));
}

#[test]
fn jianxiong_virtual_damage_draws_only() {
    let mut g = game(2, &["jianxiong"]);
    add_deck(&mut g, 2);
    let step = call(
        &mut g,
        "jianxiong",
        0,
        data(&[("event", "DamageAfter".into()), ("victim", 0.into()), ("by", 1.into()), ("amount", 1.into())]),
    );
    assert!(!matches!(step, Step::Need(_) | Step::NeedAt(_, _) | Step::NeedWith(_, _, _)), "无伤害牌 → 不询问获得");
    assert_eq!(g.hand_size(0), 1, "仍摸 1（解耦）");
}

// ---------------------------------------------------------------------------
// 界曹操 · 护驾
// ---------------------------------------------------------------------------

#[test]
fn hujia_offers_relay_for_shan_only() {
    let mut g = game(3, &["hujia"]);
    g.seats[0].identity = Some(Identity::Lord);
    add(&mut g, "s1", "shan", Zone::Hand(1));
    let alts = skills::response_alternatives(&g, 0, ResponseKind::Shan, "sha");
    assert!(
        alts.iter().any(|(c, _)| *c == "hujia"),
        "主公需要【闪】时可护驾（魏势力有牌）"
    );
    assert!(
        skills::response_alternatives(&g, 0, ResponseKind::Sha, "sha").is_empty(),
        "【杀】不触发护驾"
    );
    // 拒绝接力的反例：非魏势力无接力对象。
    g.seats[1].kingdom = "shu".to_string();
    assert!(skills::response_alternatives(&g, 0, ResponseKind::Shan, "sha").is_empty());
}

#[test]
fn hujia_relay_plays_shan_and_lord_draws_exactly_once() {
    let mut g = game(3, &["hujia"]);
    g.seats[0].identity = Some(Identity::Lord);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "s1", "shan", Zone::Hand(1));
    add_deck(&mut g, 3);
    trigger(&mut g, "hujia", 0, data(&[("step", "alt".into())]));
    // 接力询问由魏势力角色（座位 1）回答。
    assert_eq!(pending_ids(&g).len(), 1, "接力只有一张可代出的【闪】");
    submit(&mut g, "respond:s1");
    assert_eq!(g.cards.get("s1"), Some(&Zone::Discard), "代出的【闪】进弃牌堆");
    // 代出视为「打出」→ 该角色回合外打出【闪】→ 可令主公摸 1（其选择）。
    if g.pending.is_some() {
        let ids = pending_ids(&g);
        if ids.contains(&"draw".to_string()) {
            let before = g.hand_size(0);
            submit(&mut g, "draw");
            assert_eq!(g.hand_size(0), before + 1, "护驾：主公摸 1");
        }
    }
    assert!(g.counter("turn.0.hujia_draw") > 0, "每回合限一次记账");
}

// ---------------------------------------------------------------------------
// 界司马懿 · 反馈 / 鬼才
// ---------------------------------------------------------------------------

#[test]
fn fankui_triggers_per_damage_point() {
    let mut g = game(2, &["fankui"]);
    add(&mut g, "h1", "tao", Zone::Hand(1));
    add(&mut g, "h2", "sha", Zone::Hand(1));
    add(&mut g, "e1", "qinggang_jian", Zone::Equip(1, sanguosha3::kernel::Slot::Weapon));
    // 2 点伤害 → 询问 2 次。
    trigger(
        &mut g,
        "fankui",
        0,
        data(&[("event", "DamageAfter".into()), ("victim", 0.into()), ("by", 1.into()), ("amount", 2.into())]),
    );
    submit(&mut g, "use");
    submit(&mut g, "hand");
    assert_eq!(g.hand_size(0), 1, "第 1 张（手牌随机）");
    // 第二次：再问一次是否发动，然后装备区可选。
    submit(&mut g, "use");
    let ids = pending_ids(&g);
    assert!(ids.iter().any(|i| i.starts_with("equip:")), "装备区可选：{ids:?}");
    submit(&mut g, "equip:e1");
    assert_eq!(g.hand_size(0), 2, "第 2 张（装备区指定）");
    assert!(g.hand(0).contains(&"e1".to_string()));
}

#[test]
fn fankui_not_offered_without_cards() {
    let mut g = game(2, &["fankui"]);
    trigger(
        &mut g,
        "fankui",
        0,
        data(&[("event", "DamageAfter".into()), ("victim", 0.into()), ("by", 1.into()), ("amount", 1.into())]),
    );
    assert!(g.pending.is_none(), "来源没有牌 → 不询问");
}

#[test]
fn guicai_rewrites_judge_card_and_original_goes_to_discard() {
    let mut g = game(2, &["guicai"]);
    add_suit(&mut g, "top", "sha", Zone::Draw, Suit::Spade, 7);
    g.deck.push("top".into());
    add_suit(&mut g, "swap", "tao", Zone::Hand(0), Suit::Heart, 13);
    sanguosha3::kernel::quick_judge::start(&mut g, 0, "测试判定", None);
    resolve::advance(&mut g).unwrap();
    assert_eq!(pending_ids(&g), vec!["card:swap".to_string()], "只有一张可替换牌");
    submit(&mut g, "card:swap");
    let r = g.judge_result.clone().expect("判定结果");
    assert_eq!(r.card, "swap", "改判牌作为生效判定牌");
    assert_eq!(r.suit, Suit::Heart);
    assert_eq!(g.cards.get("top"), Some(&Zone::Discard), "原判定牌进弃牌堆");
    assert_eq!(g.cards.get("swap"), Some(&Zone::Discard), "生效判定牌落盘进弃牌堆");
}

#[test]
fn guicai_declined_keeps_original_judge() {
    let mut g = game(2, &["guicai"]);
    add_suit(&mut g, "top", "sha", Zone::Draw, Suit::Spade, 7);
    g.deck.push("top".into());
    add_suit(&mut g, "swap", "tao", Zone::Hand(0), Suit::Heart, 13);
    sanguosha3::kernel::quick_judge::start(&mut g, 0, "测试判定", None);
    resolve::advance(&mut g).unwrap();
    submit(&mut g, "pass");
    let r = g.judge_result.clone().expect("判定结果");
    assert_eq!(r.card, "top", "放弃改判 → 原判定牌生效");
    assert_eq!(g.hand_size(0), 1, "替代牌未消耗");
}

#[test]
fn guicai_not_asked_without_cards() {
    let mut g = game(2, &["guicai"]);
    add_suit(&mut g, "top", "sha", Zone::Draw, Suit::Spade, 7);
    g.deck.push("top".into());
    sanguosha3::kernel::quick_judge::start(&mut g, 0, "测试判定", None);
    resolve::advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "无牌可换 → 不询问（I1）");
    assert_eq!(g.judge_result.as_ref().map(|r| r.card.clone()), Some("top".to_string()));
}

// ---------------------------------------------------------------------------
// 界郭嘉 · 天妒
// ---------------------------------------------------------------------------

#[test]
fn tiandu_gains_the_effective_judge_card() {
    let mut g = game(2, &["tiandu"]);
    add_suit(&mut g, "top", "sha", Zone::Draw, Suit::Spade, 7);
    g.deck.push("top".into());
    sanguosha3::kernel::quick_judge::start(&mut g, 0, "测试判定", None);
    resolve::advance(&mut g).unwrap();
    assert_eq!(pending_ids(&g), vec!["gain".to_string()]);
    submit(&mut g, "gain");
    assert!(g.hand(0).contains(&"top".to_string()), "获得生效的判定牌");
    assert_eq!(g.cards.get("top"), Some(&Zone::Hand(0)));
}

#[test]
fn tiandu_gets_rewritten_card() {
    let mut g = game(3, &["tiandu"]);
    g.seats[1].skills = vec!["guicai".into()];
    add_suit(&mut g, "top", "sha", Zone::Draw, Suit::Spade, 7);
    g.deck.push("top".into());
    add_suit(&mut g, "swap", "tao", Zone::Hand(1), Suit::Heart, 13);
    sanguosha3::kernel::quick_judge::start(&mut g, 0, "测试判定", None);
    resolve::advance(&mut g).unwrap();
    // 改判者先询问（座位 1，判定者 0 最后）。
    submit(&mut g, "card:swap");
    // 生效后：天妒（座位 0）获得**改判后**的牌。
    submit(&mut g, "gain");
    assert!(g.hand(0).contains(&"swap".to_string()), "天妒获得改判后的判定牌");
}

// ---------------------------------------------------------------------------
// 界夏侯惇 · 刚烈 / 清俭
// ---------------------------------------------------------------------------

#[test]
fn ganglie_red_judge_damages_source() {
    let mut g = game(2, &["ganglie"]);
    add_suit(&mut g, "top", "sha", Zone::Draw, Suit::Heart, 7);
    g.deck.push("top".into());
    let hp = g.seats[1].hp;
    trigger(
        &mut g,
        "ganglie",
        0,
        data(&[("event", "DamageAfter".into()), ("victim", 0.into()), ("by", 1.into()), ("amount", 1.into())]),
    );
    submit(&mut g, "use");
    assert_eq!(g.seats[1].hp, hp - 1, "判定红色 → 对来源造成 1 点伤害");
}

#[test]
fn ganglie_black_judge_discards_source_card() {
    let mut g = game(2, &["ganglie"]);
    add_suit(&mut g, "top", "sha", Zone::Draw, Suit::Spade, 7);
    g.deck.push("top".into());
    add(&mut g, "h1", "tao", Zone::Hand(1));
    let hp = g.seats[1].hp;
    trigger(
        &mut g,
        "ganglie",
        0,
        data(&[("event", "DamageAfter".into()), ("victim", 0.into()), ("by", 1.into()), ("amount", 1.into())]),
    );
    submit(&mut g, "use");
    submit(&mut g, "hand");
    assert_eq!(g.seats[1].hp, hp, "黑色分支不造成伤害");
    assert_eq!(g.cards.get("h1"), Some(&Zone::Discard), "弃置来源一张牌");
}

#[test]
fn qingjian_gives_cards_and_bumps_current_turn_hand_limit() {
    let mut g = game(3, &["qingjian"]);
    g.turn = 1;
    add(&mut g, "b1", "sha", Zone::Hand(0));
    add(&mut g, "t1", "wuzhong_shengyou", Zone::Hand(0));
    add(&mut g, "x1", "qinggang_jian", Zone::Hand(0));
    trigger(
        &mut g,
        "qingjian",
        0,
        data(&[("event", "CardGained".into()), ("victim", 0.into()), ("card", "b1".into())]),
    );
    submit(&mut g, "use");
    submit(&mut g, "t:2");
    submit(&mut g, "card:b1");
    submit(&mut g, "card:t1");
    submit(&mut g, "pass");
    assert_eq!(g.hand_size(2), 2, "交出的两张到目标手牌");
    assert_eq!(g.counter("turn.1.qingjian_limit"), 2, "每给出一种类型 +1（当前回合角色）");
    // 反例：每回合限一次。
    trigger(
        &mut g,
        "qingjian",
        0,
        data(&[("event", "CardGained".into()), ("victim", 0.into()), ("card", "b1".into())]),
    );
    assert!(g.pending.is_none(), "每回合限一次");
}

// ---------------------------------------------------------------------------
// 界张辽 · 突袭
// ---------------------------------------------------------------------------

#[test]
fn tuxi_draws_x_and_takes_from_n_minus_x_seats() {
    let mut g = game(3, &["tuxi"]);
    g.turn = 0;
    g.phase = Phase::Draw;
    add_deck(&mut g, 3);
    add(&mut g, "h1", "tao", Zone::Hand(1));
    add(&mut g, "h2", "sha", Zone::Hand(2));
    trigger(
        &mut g,
        "tuxi",
        0,
        data(&[("event", "PhaseStart".into()), ("phase", "Draw".into())]),
    );
    submit(&mut g, "draw:1");
    assert_eq!(g.hand_size(0), 1, "摸 X=1 张");
    submit(&mut g, "t:1");
    assert_eq!(g.hand_size(0), 2, "再拿 1 名角色一张手牌");
    assert_eq!(g.cards.get("h1"), Some(&Zone::Hand(0)));
    assert!(g.counter("turn.0.skip_draw") > 0, "本阶段不再自动摸牌");
    let drawn = flow::draw_phase(&mut g, 0);
    assert!(drawn.is_empty(), "摸牌阶段已被突袭替代");
}

#[test]
fn tuxi_x_equals_n_takes_nothing() {
    let mut g = game(3, &["tuxi"]);
    g.turn = 0;
    g.phase = Phase::Draw;
    add_deck(&mut g, 3);
    add(&mut g, "h1", "tao", Zone::Hand(1));
    trigger(
        &mut g,
        "tuxi",
        0,
        data(&[("event", "PhaseStart".into()), ("phase", "Draw".into())]),
    );
    submit(&mut g, "draw:2");
    assert_eq!(g.hand_size(0), 2);
    assert_eq!(g.cards.get("h1"), Some(&Zone::Hand(1)), "不拿牌");
}

// ---------------------------------------------------------------------------
// 界许褚 · 裸衣
// ---------------------------------------------------------------------------

#[test]
fn luoyi_skip_draw_gains_basic_weapon_duel_and_boosts_damage() {
    let mut g = game(2, &["luoyi"]);
    g.turn = 0;
    g.phase = Phase::Draw;
    add_suit(&mut g, "c1", "sha", Zone::Draw, Suit::Club, 7);
    add_suit(&mut g, "c2", "qinggang_jian", Zone::Draw, Suit::Spade, 7);
    add_suit(&mut g, "c3", "juedou", Zone::Draw, Suit::Club, 7);
    g.deck.extend(["c1".to_string(), "c2".to_string(), "c3".to_string()]);
    trigger(
        &mut g,
        "luoyi",
        0,
        data(&[("event", "PhaseStart".into()), ("phase", "Draw".into())]),
    );
    submit(&mut g, "use");
    submit(&mut g, "skip");
    assert_eq!(g.hand_size(0), 3, "基本牌/武器/【决斗】全部获得");
    assert!(g.counter("turn.0.skip_draw") > 0, "跳过摸牌阶段");
    let spec = sanguosha3::kernel::DamageSpec {
        from: Some(0),
        target: 1,
        amount: 1,
        kind: sanguosha3::kernel::DamageKind::Normal,
        source: sanguosha3::kernel::SourceKind::Slash,
    };
    assert_eq!(skills::damage_bonus(&g, &spec), 1, "【杀】伤害 +1");
    let duel = sanguosha3::kernel::DamageSpec {
        source: sanguosha3::kernel::SourceKind::Duel,
        ..spec
    };
    assert_eq!(skills::damage_bonus(&g, &duel), 1, "【决斗】伤害 +1");
}

#[test]
fn luoyi_abandon_discards_shown_cards_and_draws_normally() {
    let mut g = game(2, &["luoyi"]);
    g.turn = 0;
    g.phase = Phase::Draw;
    add_deck(&mut g, 4);
    add_suit(&mut g, "c1", "sha", Zone::Draw, Suit::Club, 7);
    add_suit(&mut g, "c2", "tao", Zone::Draw, Suit::Heart, 7);
    add_suit(&mut g, "c3", "shan", Zone::Draw, Suit::Diamond, 7);
    g.deck.extend(["c1".to_string(), "c2".to_string(), "c3".to_string()]);
    trigger(
        &mut g,
        "luoyi",
        0,
        data(&[("event", "PhaseStart".into()), ("phase", "Draw".into())]),
    );
    submit(&mut g, "use");
    submit(&mut g, "pass");
    for id in ["c1", "c2", "c3"] {
        assert_eq!(g.cards.get(id), Some(&Zone::Discard), "展示牌进弃牌堆");
    }
    assert_eq!(g.counter("turn.0.skip_draw"), 0, "未跳过摸牌");
    let drawn = flow::draw_phase(&mut g, 0);
    assert_eq!(drawn.len(), 2, "正常摸 2 张");
    let spec = sanguosha3::kernel::DamageSpec {
        from: Some(0),
        target: 1,
        amount: 1,
        kind: sanguosha3::kernel::DamageKind::Normal,
        source: sanguosha3::kernel::SourceKind::Slash,
    };
    assert_eq!(skills::damage_bonus(&g, &spec), 0, "放弃后不加伤害");
}

// ---------------------------------------------------------------------------
// 界郭嘉 · 遗计
// ---------------------------------------------------------------------------

#[test]
fn yiji_draws_two_and_gives_to_at_most_two_others() {
    let mut g = game(3, &["yiji"]);
    add_deck(&mut g, 4);
    trigger(
        &mut g,
        "yiji",
        0,
        data(&[("event", "DamageAfter".into()), ("victim", 0.into()), ("by", 1.into()), ("amount", 1.into())]),
    );
    submit(&mut g, "use");
    assert_eq!(g.hand_size(0), 2, "摸两张");
    submit(&mut g, "t:1");
    assert_eq!(g.hand_size(1), 1, "交给第一名角色一张");
    submit(&mut g, "t:2");
    assert_eq!(g.hand_size(2), 1, "交给第二名角色一张");
    assert_eq!(g.hand_size(0), 0, "两张都给出");
}

#[test]
fn yiji_can_keep_cards_by_passing() {
    let mut g = game(3, &["yiji"]);
    add_deck(&mut g, 4);
    trigger(
        &mut g,
        "yiji",
        0,
        data(&[("event", "DamageAfter".into()), ("victim", 0.into()), ("by", 1.into()), ("amount", 1.into())]),
    );
    submit(&mut g, "use");
    submit(&mut g, "pass");
    submit(&mut g, "pass");
    assert_eq!(g.hand_size(0), 2, "不交出 → 自己保留");
}

// ---------------------------------------------------------------------------
// 界李典 · 恂恂 / 忘隙
// ---------------------------------------------------------------------------

#[test]
fn xunxun_puts_two_on_top_in_chosen_order_and_rest_bottom() {
    let mut g = game(2, &["xunxun"]);
    g.turn = 0;
    g.phase = Phase::Draw;
    for i in 0..4 {
        add(&mut g, &format!("k{i}"), "sha", Zone::Draw);
    }
    // 牌堆顶在末尾：k3 是顶。
    g.deck = vec!["k0".into(), "k1".into(), "k2".into(), "k3".into()];
    add_deck(&mut g, 2);
    trigger(
        &mut g,
        "xunxun",
        0,
        data(&[("event", "PhaseStart".into()), ("phase", "Draw".into())]),
    );
    // 观看牌堆顶 4 张 = k3,k2,k1,k0（以摸牌顺序给出）。
    let shown = pending_ids(&g);
    assert_eq!(shown.len(), 2, "当前牌两个选项（top/card / bottom/card）");
    // 置顶两张：先选的先摸到。
    let first_top = shown
        .iter()
        .find_map(|s| s.strip_prefix("top:").map(|x| x.to_string()))
        .expect("存在可置顶的牌");
    submit(&mut g, &format!("top:{first_top}"));
    let ids = pending_ids(&g);
    let second_top = ids
        .iter()
        .find_map(|s| s.strip_prefix("top:").map(|x| x.to_string()))
        .expect("还剩可置顶的牌");
    submit(&mut g, &format!("top:{second_top}"));
    assert_eq!(g.counter("turn.0.skip_draw"), 0, "恂恂不跳过摸牌");
    let drawn = flow::draw_phase(&mut g, 0);
    assert_eq!(drawn, vec![first_top, second_top], "两张置顶按选择顺序被摸到");
}

#[test]
fn wangxi_gives_one_of_two_drawn_to_the_other() {
    let mut g = game(2, &["wangxi"]);
    add_deck(&mut g, 4);
    trigger(
        &mut g,
        "wangxi",
        0,
        data(&[("event", "DamageAfter".into()), ("victim", 0.into()), ("by", 1.into()), ("amount", 1.into())]),
    );
    submit(&mut g, "use");
    assert_eq!(g.hand_size(0), 2, "摸两张");
    let hand = g.hand(0);
    submit(&mut g, &format!("card:{}", hand[0]));
    assert_eq!(g.hand_size(1), 1, "交给对方一张");
    assert_eq!(g.hand_size(0), 1, "自己留一张");
}

#[test]
fn wangxi_keeps_both_when_target_already_dead() {
    let mut g = game(2, &["wangxi"]);
    add_deck(&mut g, 4);
    g.seats[1].alive = false;
    trigger(
        &mut g,
        "wangxi",
        0,
        data(&[("event", "DamageAfter".into()), ("victim", 0.into()), ("by", 1.into()), ("amount", 1.into())]),
    );
    submit(&mut g, "use");
    assert_eq!(g.hand_size(0), 2, "对方已离场 → 两张都归自己");
    assert!(g.pending.is_none(), "不产生交牌询问");
}

// ---------------------------------------------------------------------------
// 界甄姬 · 倾国 / 洛神
// ---------------------------------------------------------------------------

#[test]
fn qingguo_black_card_as_shan_hand_or_equip() {
    let mut g = game(2, &["qingguo"]);
    add_suit(&mut g, "blk", "sha", Zone::Hand(0), Suit::Spade, 7);
    add_suit(&mut g, "red", "sha", Zone::Hand(0), Suit::Heart, 7);
    add_suit(&mut g, "eq", "qinggang_jian", Zone::Equip(0, sanguosha3::kernel::Slot::Weapon), Suit::Club, 7);
    assert_eq!(skills::response_as(&g, 0, "blk", ResponseKind::Shan), Some("shan"));
    assert_eq!(
        skills::response_as(&g, 0, "eq", ResponseKind::Shan),
        Some("shan"),
        "装备区黑色牌也可倾国"
    );
    assert_eq!(
        skills::response_as(&g, 0, "red", ResponseKind::Shan),
        None,
        "红色牌不能倾国"
    );
}

#[test]
fn luoshen_gains_black_and_repeats_until_red() {
    let mut g = game(2, &["luoshen"]);
    g.turn = 0;
    g.phase = Phase::Prepare;
    add_suit(&mut g, "blk", "sha", Zone::Draw, Suit::Spade, 7);
    add_suit(&mut g, "red", "shan", Zone::Draw, Suit::Heart, 7);
    g.deck = vec!["red".into(), "blk".into()];
    trigger(
        &mut g,
        "luoshen",
        0,
        data(&[("event", "PhaseStart".into()), ("phase", "Prepare".into())]),
    );
    submit(&mut g, "use");
    assert!(g.hand(0).contains(&"blk".to_string()), "黑色判定牌获得");
    assert_eq!(g.counter("turn.0.luoshen_free"), 1, "本回合不计入手牌上限");
    // 继续：下一张红色 → 结束。
    let ids = pending_ids(&g);
    if ids.contains(&"use".to_string()) {
        submit(&mut g, "use");
        assert_eq!(g.cards.get("red"), Some(&Zone::Discard), "红色判定牌进弃牌堆");
        assert_eq!(g.hand_size(0), 1, "红色不被获得");
    }
}

#[test]
fn luoshen_finishing_red_judge_ends_without_gain() {
    let mut g = game(2, &["luoshen"]);
    g.turn = 0;
    g.phase = Phase::Prepare;
    add_suit(&mut g, "red", "shan", Zone::Draw, Suit::Heart, 7);
    g.deck = vec!["red".into()];
    trigger(
        &mut g,
        "luoshen",
        0,
        data(&[("event", "PhaseStart".into()), ("phase", "Prepare".into())]),
    );
    submit(&mut g, "pass");
    assert_eq!(g.hand_size(0), 0, "放弃则不判定");
}
