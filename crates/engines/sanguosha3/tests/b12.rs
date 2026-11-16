//! B12 技能门禁（界标·群：4 将 / 8 技能）。

use sanguosha3::kernel::cards::ResponseKind;
use sanguosha3::kernel::resolve::{self, Stage};
use sanguosha3::kernel::{card_play, flow, gear, skills, Card, Game, Phase, Seat, Slot, Suit, Zone};

fn seat_with(id: usize, list: &[&str], kingdom: &str, male: bool) -> Seat {
    Seat {
        id,
        hp: 4,
        max_hp: 4,
        alive: true,
        male,
        skills: list.iter().map(|s| s.to_string()).collect(),
        kingdom: kingdom.to_string(),
        ..Default::default()
    }
}

fn game(n: usize, s0: &[&str]) -> Game {
    let mut seats: Vec<Seat> = (0..n).map(|i| seat_with(i, &[], "qun", true)).collect();
    seats[0] = seat_with(0, s0, "qun", true);
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

/// 主动技：走 `skills::use_active`（合法集 → 压帧 → 推进）。
fn use_active(g: &mut Game, seat: usize, code: &str) {
    skills::use_active(g, seat, code).expect("主动技可用");
}

// ---------------------------------------------------------------------------
// 界华佗 · 急救
// ---------------------------------------------------------------------------

#[test]
fn jijiu_rescues_out_of_turn_with_red_card() {
    let mut g = game(3, &[]);
    g.seats[1].skills = vec!["jijiu".into()];
    g.turn = 0;
    g.seats[2].hp = 0;
    add_suit(&mut g, "red", "sha", Zone::Hand(1), Suit::Heart, 7);
    add_suit(&mut g, "blk", "sha", Zone::Hand(1), Suit::Spade, 7);
    flow::enter_dying(&mut g, 2, None);
    resolve::advance(&mut g).unwrap();
    // 求桃顺序：自己（2）→ 3 → 0 → 1；先让前三位放弃。
    for _ in 0..3 {
        if g.pending.is_none() {
            break;
        }
        let ids = pending_ids(&g);
        if ids.iter().any(|i| i.starts_with("ji:")) {
            break;
        }
        submit(&mut g, "pass");
    }
    let ids = pending_ids(&g);
    assert!(ids.contains(&"ji:red".to_string()), "急救候选（红色牌）：{ids:?}");
    assert!(!ids.contains(&"ji:blk".to_string()), "黑色牌不能急救");
    submit(&mut g, "ji:red");
    assert_eq!(g.seats[2].hp, 1, "急救救回 1 点");
    assert_eq!(g.cards.get("red"), Some(&Zone::Discard), "承载牌进弃牌堆");
}

#[test]
fn jijiu_unavailable_in_own_turn() {
    let mut g = game(2, &[]);
    g.seats[0].skills = vec!["jijiu".into()];
    g.turn = 0;
    add_suit(&mut g, "red", "sha", Zone::Hand(0), Suit::Heart, 7);
    assert!(
        sanguosha3::kernel::skills::b12::jijiu_options(&g, 0).is_empty(),
        "自己回合内不能急救"
    );
    g.turn = 1;
    assert_eq!(
        sanguosha3::kernel::skills::b12::jijiu_options(&g, 0),
        vec!["red".to_string()],
        "回合外可急救（红色牌）"
    );
}

#[test]
fn jijiu_equip_area_red_card_counts() {
    let mut g = game(2, &[]);
    g.seats[0].skills = vec!["jijiu".into()];
    g.turn = 1;
    add_suit(&mut g, "eq", "qinggang_jian", Zone::Equip(0, Slot::Weapon), Suit::Diamond, 7);
    assert_eq!(
        sanguosha3::kernel::skills::b12::jijiu_options(&g, 0),
        vec!["eq".to_string()],
        "装备区红色牌也可急救"
    );
    sanguosha3::kernel::skills::b12::use_jijiu(&mut g, 0, "eq").unwrap();
    assert_eq!(g.cards.get("eq"), Some(&Zone::Discard));
}

// ---------------------------------------------------------------------------
// 界华佗 · 除疠
// ---------------------------------------------------------------------------

#[test]
fn chuli_picks_distinct_kingdoms_and_spade_draws() {
    let mut g = game(4, &["chuli"]);
    g.turn = 0;
    g.phase = Phase::Play;
    g.seats[1].kingdom = "wei".to_string();
    g.seats[2].kingdom = "wei".to_string();
    g.seats[3].kingdom = "shu".to_string();
    add(&mut g, "own", "tao", Zone::Hand(0));
    add_suit(&mut g, "s1", "sha", Zone::Hand(1), Suit::Spade, 7);
    add(&mut g, "s2", "sha", Zone::Hand(2));
    add(&mut g, "s3", "sha", Zone::Hand(3));
    add_deck(&mut g, 4);
    use_active(&mut g, 0, "chuli");
    let ids = pending_ids(&g);
    assert!(ids.contains(&"t:1".to_string()));
    assert!(!ids.contains(&"confirm".to_string()), "未选人时不能确认");
    submit(&mut g, "t:1");
    let ids = pending_ids(&g);
    assert!(ids.contains(&"confirm".to_string()), "已选 ≥1 → 可确认");
    assert!(!ids.contains(&"t:2".to_string()), "同势力第二名不出现");
    assert!(ids.contains(&"t:3".to_string()), "不同势力可选");
    submit(&mut g, "t:3");
    submit(&mut g, "confirm");
    // 参与者逐个弃牌：自己 → 座位1 → 座位3。
    submit(&mut g, "card:own");
    submit(&mut g, "card:s1");
    submit(&mut g, "card:s3");
    assert_eq!(g.cards.get("own"), Some(&Zone::Discard));
    assert_eq!(g.hand_size(1), 1, "座位1 弃黑桃后摸 1（弃 1 摸 1）");
    assert_eq!(g.hand_size(3), 0, "座位3 非黑桃不摸");
    assert!(g.counter("phase.0.chuli") > 0, "本阶段限一次");
}

#[test]
fn chuli_unavailable_without_own_cards() {
    let mut g = game(3, &["chuli"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "x", "sha", Zone::Hand(1));
    assert!(
        !skills::active_actions(&g, 0).iter().any(|c| *c == "chuli"),
        "自己无牌不能发动"
    );
}

// ---------------------------------------------------------------------------
// 界吕布 · 无双
// ---------------------------------------------------------------------------

#[test]
fn wushuang_requires_two_responses() {
    let mut g = game(3, &["wushuang"]);
    assert_eq!(skills::response_need(&g, 1, 0, ResponseKind::Shan), 2, "【杀】需两张【闪】");
    assert_eq!(skills::response_need(&g, 0, 1, ResponseKind::Shan), 1, "无【无双】只需一张");
    // 决斗：对方（非响应方）有【无双】→ 每次两张【杀】。
    assert_eq!(skills::response_need(&g, 1, 0, ResponseKind::Sha), 2);
    let mut g2 = game(3, &[]);
    assert_eq!(skills::response_need(&g2, 1, 0, ResponseKind::Sha), 1);
}

#[test]
fn wushuang_sha_actually_asks_two_shan() {
    let mut g = game(3, &["wushuang"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "sha1", "sha", Zone::Hand(0));
    add(&mut g, "s1", "shan", Zone::Hand(1));
    add(&mut g, "s2", "shan", Zone::Hand(1));
    card_play::use_on_targets(&mut g, 0, "sha1", &[1]).unwrap();
    resolve::advance(&mut g).unwrap();
    // 第一张【闪】后仍需第二张。
    submit(&mut g, "s:s1");
    assert!(g.pending.is_some(), "第一张【闪】不足两张");
    submit(&mut g, "s:s2");
    assert_eq!(g.seats[1].hp, 4, "两张【闪】抵消【杀】");
}

// ---------------------------------------------------------------------------
// 界吕布 · 利驭
// ---------------------------------------------------------------------------

#[test]
fn liyu_non_equip_lets_victim_draw() {
    let mut g = game(3, &["liyu"]);
    add(&mut g, "sha1", "sha", Zone::Discard);
    add(&mut g, "h1", "tao", Zone::Hand(1));
    add_deck(&mut g, 2);
    sanguosha3::kernel::resolve::push_ask(
        &mut g,
        "liyu",
        Stage::Choose,
        0,
        data(&[("event", "DamageAfter".into()), ("victim", 1.into()), ("by", 0.into()), ("card", "sha1".into()), ("amount", 1.into())]),
    );
    resolve::advance(&mut g).unwrap();
    submit(&mut g, "use");
    submit(&mut g, "hand");
    assert_eq!(g.hand_size(0), 1, "吕布获得其一张牌");
    assert_eq!(g.hand_size(1), 1, "非装备牌 → 受伤角色摸一张（原手牌已交出，摸 1）");
    assert!(g.pending.is_none(), "非装备牌不进入指定阶段");
}

#[test]
fn liyu_equip_triggers_virtual_duel_by_victim_choice() {
    let mut g = game(3, &["liyu"]);
    g.seats[2].hp = 4;
    add(&mut g, "sha1", "sha", Zone::Discard);
    add(&mut g, "eq1", "qinggang_jian", Zone::Equip(1, Slot::Weapon));
    add(&mut g, "h2", "sha", Zone::Hand(2));
    sanguosha3::kernel::resolve::push_ask(
        &mut g,
        "liyu",
        Stage::Choose,
        0,
        data(&[("event", "DamageAfter".into()), ("victim", 1.into()), ("by", 0.into()), ("card", "sha1".into()), ("amount", 1.into())]),
    );
    resolve::advance(&mut g).unwrap();
    submit(&mut g, "use");
    submit(&mut g, "equip:eq1");
    assert_eq!(g.hand_size(0), 1, "获得装备牌");
    // 由受伤角色（座位 1）指定另一名角色 → 吕布视为对其使用【决斗】。
    submit(&mut g, "t:2");
    assert_eq!(
        g.pending.as_ref().map(|p| p.kind().to_string()),
        Some("duel_ask".to_string()),
        "视为【决斗】走真实结算（目标先出【杀】）"
    );
}

#[test]
fn liyu_not_triggered_without_slash_damage() {
    let mut g = game(2, &["liyu"]);
    add(&mut g, "h1", "tao", Zone::Hand(1));
    add(&mut g, "sk", "wanjian_qifa", Zone::Discard);
    sanguosha3::kernel::resolve::push_ask(
        &mut g,
        "liyu",
        Stage::Choose,
        0,
        data(&[("event", "DamageAfter".into()), ("victim", 1.into()), ("by", 0.into()), ("card", "sk".into()), ("amount", 1.into())]),
    );
    resolve::advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "非【杀】类伤害不触发");
}

// ---------------------------------------------------------------------------
// 界公孙瓒 · 义从 / 趫猛
// ---------------------------------------------------------------------------

#[test]
fn yicong_modifies_distance_both_ways() {
    let mut g = game(4, &["yicong"]);
    let base = gear::distance(&g, 0, 1);
    let to = gear::distance(&g, 1, 0);
    // 我 → 他人 -1：环距 1 已在下限 1，改用远座位验证。
    let far_base = gear::distance(&g, 0, 2);
    assert_eq!(far_base, 1, "义从：0→2 环距 2 -1 = 1");
    let _ = base;
    assert_eq!(to, 1, "体力 4 时他人到我不加距离");
    g.seats[0].hp = 2;
    assert_eq!(gear::distance(&g, 1, 0), 2, "体力 ≤2 时他人到我 +1");
}

#[test]
fn yicong_distance_floor_is_one() {
    let mut g = game(3, &["yicong"]);
    assert_eq!(gear::distance(&g, 0, 1), 1, "相邻仍为 1（下限）");
    let mut g2 = game(3, &[]);
    assert_eq!(gear::distance(&g2, 0, 1), 1);
}

#[test]
fn qiaomeng_takes_horse_but_discards_other_cards() {
    let mut g = game(2, &["qiaomeng"]);
    add(&mut g, "sha1", "sha", Zone::Discard);
    add(&mut g, "horse", "jueying", Zone::Equip(1, Slot::HorseOffense));
    add(&mut g, "armor", "bagua_zhen", Zone::Equip(1, Slot::Armor));
    sanguosha3::kernel::resolve::push_ask(
        &mut g,
        "qiaomeng",
        Stage::Choose,
        0,
        data(&[("event", "DamageAfter".into()), ("victim", 1.into()), ("by", 0.into()), ("card", "sha1".into()), ("amount", 1.into())]),
    );
    resolve::advance(&mut g).unwrap();
    submit(&mut g, "use");
    submit(&mut g, "equip:horse");
    assert_eq!(g.cards.get("horse"), Some(&Zone::Hand(0)), "坐骑牌改为获得");
    // 非坐骑 → 弃置。
    sanguosha3::kernel::resolve::push_ask(
        &mut g,
        "qiaomeng",
        Stage::Choose,
        0,
        data(&[("event", "DamageAfter".into()), ("victim", 1.into()), ("by", 0.into()), ("card", "sha1".into()), ("amount", 1.into())]),
    );
    resolve::advance(&mut g).unwrap();
    submit(&mut g, "use");
    submit(&mut g, "equip:armor");
    assert_eq!(g.cards.get("armor"), Some(&Zone::Discard), "非坐骑 → 弃置");
}

// ---------------------------------------------------------------------------
// 界貂蝉 · 离间 / 闭月
// ---------------------------------------------------------------------------

#[test]
fn lijian_discards_card_and_starts_no_wuxie_duel() {
    let mut g = game(4, &["lijian"]);
    g.turn = 0;
    g.phase = Phase::Play;
    g.seats[3].male = false;
    add(&mut g, "own", "tao", Zone::Hand(0));
    add(&mut g, "sh1", "sha", Zone::Hand(1));
    add(&mut g, "sh2", "sha", Zone::Hand(2));
    add(&mut g, "w1", "wuxie_keji", Zone::Hand(3));
    use_active(&mut g, 0, "lijian");
    submit(&mut g, "card:own");
    submit(&mut g, "t:1");
    eprintln!("DBG after submit stack={} answer={:?}/{:?}", g.stack.len(), g.answer, g.answer_skill);
    let ids = pending_ids(&g);
    assert!(!ids.contains(&"t:3".to_string()), "女性不能作目标");
    assert!(!ids.contains(&"t:1".to_string()), "发起者不能同时作目标");
    submit(&mut g, "t:2");
    assert_eq!(g.cards.get("own"), Some(&Zone::Discard), "承载牌一次性弃置");
    assert!(g.counter("phase.0.lijian") > 0, "本阶段限一次");
    // 不可被【无懈可击】抵消：直接进入决斗询问（座位 3 手里有无懈也不出现无懈窗口）。
    let kind = g.pending.as_ref().map(|p| p.kind().to_string());
    assert_eq!(kind, Some("duel_ask".to_string()), "跳过无懈窗口，直接决斗");
}

#[test]
fn lijian_requires_two_male_targets() {
    let mut g = game(3, &["lijian"]);
    g.turn = 0;
    g.phase = Phase::Play;
    g.seats[1].male = false;
    g.seats[2].male = false;
    add(&mut g, "own", "tao", Zone::Hand(0));
    assert!(
        !skills::active_actions(&g, 0).iter().any(|c| *c == "lijian"),
        "无两名男性角色不能发动"
    );
}

#[test]
fn biyue_draws_two_with_empty_hand_and_one_otherwise() {
    let mut g = game(2, &["biyue"]);
    g.turn = 0;
    g.phase = Phase::End;
    add_deck(&mut g, 4);
    sanguosha3::kernel::resolve::push_ask(
        &mut g,
        "biyue",
        Stage::Choose,
        0,
        data(&[("event", "PhaseStart".into()), ("phase", "End".into())]),
    );
    resolve::advance(&mut g).unwrap();
    submit(&mut g, "use");
    assert_eq!(g.hand_size(0), 2, "结束阶段无手牌 → 摸 2");
    // 有手牌 → 摸 1。
    let mut g2 = game(2, &["biyue"]);
    g2.turn = 0;
    g2.phase = Phase::End;
    add_deck(&mut g2, 4);
    add(&mut g2, "keep", "tao", Zone::Hand(0));
    sanguosha3::kernel::resolve::push_ask(
        &mut g2,
        "biyue",
        Stage::Choose,
        0,
        data(&[("event", "PhaseStart".into()), ("phase", "End".into())]),
    );
    resolve::advance(&mut g2).unwrap();
    submit(&mut g2, "use");
    assert_eq!(g2.hand_size(0), 2, "有手牌 → 摸 1（1+1）");
}

#[test]
fn biyue_can_decline() {
    let mut g = game(2, &["biyue"]);
    g.turn = 0;
    g.phase = Phase::End;
    add_deck(&mut g, 4);
    sanguosha3::kernel::resolve::push_ask(
        &mut g,
        "biyue",
        Stage::Choose,
        0,
        data(&[("event", "PhaseStart".into()), ("phase", "End".into())]),
    );
    resolve::advance(&mut g).unwrap();
    submit(&mut g, "pass");
    assert_eq!(g.hand_size(0), 0, "放弃不摸牌");
}


