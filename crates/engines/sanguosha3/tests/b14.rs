use serde_json::json;
use sanguosha3::kernel::resolve::{self, Ask, Stage, Step};
use sanguosha3::kernel::{generals, skills, Card, Game, Phase, Seat, Slot, Suit, Zone};

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
    if g.pending.is_none() {
        eprintln!("DBG missing {ans}: stack={} actives={:?}", g.stack.len(), skills::active_actions(g, 0));
    }
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
// B14 技能门禁（J2：8 将 / 20 技能）
// ---------------------------------------------------------------------------

fn mk2(g: &mut Game, id: &str, def: &str, suit: Suit, rank: u8, zone: Zone) {
    g.card_defs.insert(
        id.into(),
        Card { id: id.into(), def: def.into(), suit, rank },
    );
    g.cards.insert(id.into(), zone);
}

fn use_active(g: &mut Game, seat: usize, code: &str) {
    skills::use_active(g, seat, code).expect("主动技可用");
}

// ------------------------------ 界孙策 -------------------------------------

#[test]
fn zhiba_other_side_challenge_and_lord_gets_both_cards() {
    let mut g = test_game(3);
    g.seats[0].identity = Some(sanguosha3::kernel::Identity::Lord);
    g.seats[0].skills = vec!["zhiba".into()];
    g.seats[1].kingdom = "wu".into();
    g.turn = 1;
    g.phase = Phase::Play;
    mk2(&mut g, "a1", "sha", Suit::Heart, 5, Zone::Hand(1)); // 挑战者小
    mk2(&mut g, "b1", "tao", Suit::Club, 9, Zone::Hand(0)); // 孙策大
    assert!(skills::other_side_actions(&g, 1).contains(&"zhiba"));
    skills::start_other(&mut g, 1, "zhiba").expect("可挑战");
    resolve::advance(&mut g).unwrap();
    submit(&mut g, "accept");
    // 拼点：发起者（挑战者 1）先选，然后孙策。
    submit(&mut g, "choose:a1");
    submit(&mut g, "choose:b1");
    // 挑战者没赢（5 ≤ 9）→ 孙策获得两张拼点牌。
    assert!(g.hand(0).contains(&"a1".to_string()) && g.hand(0).contains(&"b1".to_string()));
    // 反例：非吴势力不可挑战。
    let mut g2 = test_game(3);
    g2.seats[0].identity = Some(sanguosha3::kernel::Identity::Lord);
    g2.seats[0].skills = vec!["zhiba".into()];
    g2.seats[1].kingdom = "wei".into();
    g2.turn = 1;
    g2.phase = Phase::Play;
    mk2(&mut g2, "a1", "sha", Suit::Heart, 5, Zone::Hand(1));
    mk2(&mut g2, "b1", "tao", Suit::Club, 9, Zone::Hand(0));
    assert!(skills::other_side_actions(&g2, 1).is_empty());
}

#[test]
fn jiang_draws_on_red_slash_target_not_black() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["jiang".into()];
    mk2(&mut g, "dd", "shan", Suit::Club, 3, Zone::Draw);
    g.deck.push("dd".into());
    mk2(&mut g, "red", "sha", Suit::Heart, 5, Zone::Hand(0));
    g.use_targets.insert("red".into(), vec![1]);
    trigger(
        &mut g,
        "jiang",
        0,
        data(&[("event", "TargetChosen".into()), ("card", "red".into()), ("by", 0.into())]),
    );
    submit(&mut g, "use");
    assert_eq!(g.hand_size(0), 2, "红色【杀】指定目标后摸 1（含本牌）");
    // 反例：黑色【杀】不触发。
    let mut g2 = test_game(2);
    g2.seats[0].skills = vec!["jiang".into()];
    mk2(&mut g2, "blk", "sha", Suit::Spade, 5, Zone::Hand(0));
    g2.use_targets.insert("blk".into(), vec![1]);
    trigger(
        &mut g2,
        "jiang",
        0,
        data(&[("event", "TargetChosen".into()), ("card", "blk".into()), ("by", 0.into())]),
    );
    assert!(g2.pending.is_none());
}

#[test]
fn hunzi_awakens_at_one_hp_and_grants_skills() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["hunzi".into()];
    g.seats[0].hp = 1;
    trigger(&mut g, "hunzi", 0, data(&[("event", "PhaseStart".into()), ("phase", "Prepare".into())]));
    assert_eq!(g.seats[0].max_hp, 3, "上限 -1");
    assert!(skills::active(&g, 0, "yingzi") && skills::active(&g, 0, "yinghun"));
    // 反例：hp>1 不觉醒。
    let mut g2 = test_game(2);
    g2.seats[0].skills = vec!["hunzi".into()];
    g2.seats[0].hp = 2;
    trigger(&mut g2, "hunzi", 0, data(&[("event", "PhaseStart".into()), ("phase", "Prepare".into())]));
    assert_eq!(g2.seats[0].max_hp, 4);
}

// ------------------------------ 界夏侯渊 -----------------------------------

#[test]
fn shensu_skips_phases_and_uses_virtual_slash() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["shensu".into()];
    g.turn = 0;
    g.phase = Phase::Prepare;
    trigger(&mut g, "shensu", 0, data(&[("event", "PhaseStart".into()), ("phase", "Prepare".into())]));
    let opts = ids(&g);
    assert!(opts.contains(&"o1".to_string()) && opts.contains(&"o3".to_string()));
    assert!(!opts.contains(&"o2".to_string()), "无装备牌不能选②");
    submit(&mut g, "o1");
    eprintln!("DBG after o1 stack={} pending={:?} skipped={:?}", g.stack.len(), g.pending.as_ref().map(|p| p.kind().to_string()), g.skipped);
    assert!(g.skipped.contains(&Phase::Judge) && g.skipped.contains(&Phase::Draw));
    submit(&mut g, "o3");
    assert!(g.seats[0].flipped, "翻面");
    assert!(g.skipped.contains(&Phase::Discard));
    submit(&mut g, "confirm");
    assert!(g.pending.is_some(), "视为使用【杀】（进入目标/结算）");
}

#[test]
fn shebian_moves_equip_on_flip() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["shebian".into()];
    mk2(&mut g, "w1", "qinggang_jian", Suit::Spade, 5, Zone::Equip(0, Slot::Weapon));
    trigger(&mut g, "shebian", 0, data(&[("hook", "Flip".into()), ("step", "ask".into())]));
    submit(&mut g, "e:0:w1");
    submit(&mut g, "t:1");
    assert_eq!(g.cards.get("w1"), Some(&Zone::Equip(1, Slot::Weapon)));
    // 反例：场上无装备 → 不询问。
    let mut g2 = test_game(3);
    g2.seats[0].skills = vec!["shebian".into()];
    trigger(&mut g2, "shebian", 0, data(&[("hook", "Flip".into()), ("step", "ask".into())]));
    assert!(g2.pending.is_none());
}

// --------------------------- 界卧龙诸葛亮 ----------------------------------

#[test]
fn bazhen_virtual_armor_only_without_real_armor() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["bazhen".into()];
    assert_eq!(
        sanguosha3::kernel::gear::armor_of(&g, 0),
        sanguosha3::kernel::ArmorKind::Bagua,
        "无防具 → 视为八卦阵"
    );
    mk2(&mut g, "a1", "renwang_dun", Suit::Club, 2, Zone::Equip(0, Slot::Armor));
    assert_eq!(
        sanguosha3::kernel::gear::armor_of(&g, 0),
        sanguosha3::kernel::ArmorKind::Renwang,
        "真实防具优先，八阵失效"
    );
}

#[test]
fn cangzhuo_tricks_not_counted_until_trick_used() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["cangzhuo".into()];
    mk2(&mut g, "t1", "wuzhong_shengyou", Suit::Heart, 3, Zone::Hand(0));
    mk2(&mut g, "t2", "guohe_chaiqiao", Suit::Spade, 4, Zone::Hand(0));
    mk2(&mut g, "b1", "sha", Suit::Club, 5, Zone::Hand(0));
    assert_eq!(skills::hand_size_for_discard(&g, 0), 1, "锦囊不计入");
    g.bump("turn.0.trick_used");
    assert_eq!(skills::hand_size_for_discard(&g, 0), 3, "用过锦囊后恢复");
}

#[test]
fn huoji_and_kanpo_conversions() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["huoji".into(), "kanpo".into()];
    mk2(&mut g, "red", "sha", Suit::Heart, 5, Zone::Hand(0));
    mk2(&mut g, "blk", "sha", Suit::Spade, 5, Zone::Hand(0));
    assert_eq!(skills::use_alias(&g, 0, "red"), Some("huogong"), "红牌当火攻");
    assert_eq!(skills::use_alias(&g, 0, "blk"), None);
    assert_eq!(
        skills::response_as(&g, 0, "blk", sanguosha3::kernel::cards::ResponseKind::Wuxie),
        Some("wuxie_keji"),
        "黑牌当无懈"
    );
    assert!(skills::wuxie_no_chain(&g, 0, "blk"), "转化的无懈不可被响应");
    // 反例：红牌不能看破。
    assert_eq!(
        skills::response_as(&g, 0, "red", sanguosha3::kernel::cards::ResponseKind::Wuxie),
        None
    );
}

#[test]
fn lianhuan_club_alias_and_recast_counts_as_use() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["lianhuan".into()];
    mk2(&mut g, "c1", "sha", Suit::Club, 5, Zone::Hand(0));
    mk2(&mut g, "h1", "sha", Suit::Heart, 5, Zone::Hand(0));
    assert_eq!(skills::use_alias(&g, 0, "c1"), Some("tie_suo_lian_huan"));
    assert_eq!(skills::use_alias(&g, 0, "h1"), None, "红色牌不能连环");
    skills::on_recast(&mut g, 0, "c1");
    assert!(g.counter("turn.0.trick_used") > 0, "重铸算使用（藏拙口径）");
    assert!(g
        .log
        .iter()
        .any(|e| e.kind == sanguosha3::kernel::event::EventKind::CardUsed && e.card.as_deref() == Some("c1")));
}

#[test]
fn niepan_restores_and_grants_one_skill() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["niepan".into()];
    g.seats[0].hp = -1;
    g.seats[0].chained = true;
    g.seats[0].flipped = true;
    mk2(&mut g, "h1", "sha", Suit::Club, 5, Zone::Hand(0));
    mk2(&mut g, "e1", "qinggang_jian", Suit::Spade, 5, Zone::Equip(0, Slot::Weapon));
    for i in 0..3 {
        mk2(&mut g, &format!("d{i}"), "shan", Suit::Heart, 3, Zone::Draw);
        g.deck.push(format!("d{i}"));
    }
    assert!(!skills::b14::dying_options(&g, 0, 0).is_empty(), "濒死可用");
    trigger(&mut g, "niepan", 0, data(&[("step", "use".into())]));
    submit(&mut g, "use");
    submit(&mut g, "g:bazhen");
    assert_eq!(g.seats[0].hp, 3, "回复至 3");
    assert!(!g.seats[0].chained && !g.seats[0].flipped, "复原武将牌");
    assert_eq!(g.hand_size(0), 3, "摸三张");
    assert_eq!(g.cards.get("h1"), Some(&Zone::Discard), "弃置区域内所有牌");
    assert!(skills::active(&g, 0, "bazhen"));
    assert!(skills::limited_used(&g, 0, "niepan"), "限定技已用");
    assert!(skills::b14::dying_options(&g, 0, 0).is_empty(), "反例：已用后不再可用");
}

// ------------------------------ 界魏延 -------------------------------------

#[test]
fn kuanggu_heals_or_draws_within_distance_one() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["kuanggu".into()];
    g.seats[1].hp = 3;
    mk2(&mut g, "dd", "shan", Suit::Club, 3, Zone::Draw);
    g.deck.push("dd".into());
    trigger(
        &mut g,
        "kuanggu",
        0,
        data(&[("event", "DamageAfter".into()), ("victim", 1.into()), ("by", 0.into()), ("amount", 1.into())]),
    );
    submit(&mut g, "draw");
    assert_eq!(g.hand_size(0), 1, "距离 1 内 → 摸 1");
    // 反例：距离 2 不触发（4 人局 0→2 距离 2）。
    let mut g2 = test_game(4);
    g2.seats[0].skills = vec!["kuanggu".into()];
    trigger(
        &mut g2,
        "kuanggu",
        0,
        data(&[("event", "DamageAfter".into()), ("victim", 2.into()), ("by", 0.into()), ("amount", 1.into())]),
    );
    assert!(g2.pending.is_none());
}

#[test]
fn qimou_loses_hp_draws_and_boosts() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["qimou".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    for i in 0..2 {
        mk2(&mut g, &format!("d{i}"), "shan", Suit::Heart, 3, Zone::Draw);
        g.deck.push(format!("d{i}"));
    }
    use_active(&mut g, 0, "qimou");
    submit(&mut g, "x:2");
    assert_eq!(g.seats[0].hp, 2, "失去 2 点体力");
    assert_eq!(g.hand_size(0), 2, "摸 2 张");
    assert_eq!(g.counter("turn.0.qimou_x"), 2);
    assert!(g.counter("turn.0.sha_used") == 0);
    assert_eq!(skills::sha_extra(&g, 0), 2, "杀次数 +2");
    assert_eq!(skills::distance_bonus(&g, 0), -2, "距离 -2");
    // 反例：限定技只能一次。
    assert!(!skills::active_actions(&g, 0).iter().any(|c| *c == "qimou"));
}

// ------------------------------ 界小乔 -------------------------------------

#[test]
fn hongyan_treats_spade_as_heart_and_boosts_hand_limit() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["hongyan".into()];
    assert_eq!(skills::suit_for_use(&g, 0, Suit::Spade), Suit::Heart, "使用/打出时黑桃→红桃");
    assert_eq!(skills::suit_for_judge(&g, 0, Suit::Spade), Suit::Heart, "判定黑桃→红桃");
    mk2(&mut g, "eq", "qinggang_jian", Suit::Spade, 5, Zone::Equip(0, Slot::Weapon));
    assert_eq!(g.hand_limit(0), 4, "装备区红桃（黑桃视为红桃）→ 上限=体力上限");
    // 反例：装备区无红桃 → 不覆盖。
    let mut g2 = test_game(2);
    g2.seats[0].skills = vec!["hongyan".into()];
    mk2(&mut g2, "eq", "qinggang_jian", Suit::Club, 5, Zone::Equip(0, Slot::Weapon));
    assert_eq!(g2.hand_limit(0), 4, "上限仍为体力（4）");
}

#[test]
fn tianxiang_prevents_and_transfers_damage() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["tianxiang".into()];
    mk2(&mut g, "h1", "tao", Suit::Heart, 3, Zone::Hand(0));
    let spec = sanguosha3::kernel::DamageSpec {
        from: Some(1),
        target: 0,
        amount: 1,
        kind: sanguosha3::kernel::DamageKind::Normal,
        source: sanguosha3::kernel::SourceKind::Other,
    };
    let armor = sanguosha3::kernel::gear::armor_of(&g, 0);
    sanguosha3::kernel::flow::damage(&mut g, spec, armor, 0);
    resolve::advance(&mut g).unwrap();
    submit(&mut g, "use");
    submit(&mut g, "c:h1");
    submit(&mut g, "m1");
    submit(&mut g, "t:2");
    assert_eq!(g.seats[0].hp, 4, "原伤害被防止");
    assert_eq!(g.seats[2].hp, 3, "转移 1 点给目标");
    // 反例：无红桃牌 → 不询问（伤害正常结算）。
    let mut g2 = test_game(3);
    g2.seats[0].skills = vec!["tianxiang".into()];
    mk2(&mut g2, "b1", "sha", Suit::Spade, 3, Zone::Hand(0));
    let spec2 = sanguosha3::kernel::DamageSpec {
        from: Some(1),
        target: 0,
        amount: 1,
        kind: sanguosha3::kernel::DamageKind::Normal,
        source: sanguosha3::kernel::SourceKind::Other,
    };
    let armor2 = sanguosha3::kernel::gear::armor_of(&g2, 0);
    sanguosha3::kernel::flow::damage(&mut g2, spec2, armor2, 0);
    assert_eq!(g2.seats[0].hp, 3, "无红桃 → 正常受伤");
}

#[test]
fn piaoling_judges_and_puts_heart_on_top() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["piaoling".into()];
    g.turn = 0;
    g.phase = Phase::End;
    mk2(&mut g, "top", "sha", Suit::Heart, 7, Zone::Draw);
    g.deck.push("top".into());
    trigger(&mut g, "piaoling", 0, data(&[("event", "PhaseStart".into()), ("phase", "End".into())]));
    submit(&mut g, "top");
    assert_eq!(g.deck.last().map(|s| s.as_str()), Some("top"), "红桃判定牌置于牌堆顶");
    // 反例：非红桃不结算（方块）。
    let mut g2 = test_game(2);
    g2.seats[0].skills = vec!["piaoling".into()];
    g2.turn = 0;
    g2.phase = Phase::End;
    mk2(&mut g2, "top2", "sha", Suit::Spade, 7, Zone::Draw);
    g2.deck.push("top2".into());
    trigger(&mut g2, "piaoling", 0, data(&[("event", "PhaseStart".into()), ("phase", "End".into())]));
    assert!(g2.pending.is_none(), "非红桃 → 无后续询问");
}

// ------------------------------ 界孙坚 -------------------------------------

#[test]
fn yinghun_draws_x_then_discards_one() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["yinghun".into()];
    g.seats[0].hp = 2; // 已损失 2
    g.turn = 0;
    g.phase = Phase::Prepare;
    mk2(&mut g, "d1", "sha", Suit::Club, 3, Zone::Draw);
    mk2(&mut g, "d2", "shan", Suit::Club, 3, Zone::Draw);
    g.deck.push("d1".into());
    g.deck.push("d2".into());
    trigger(&mut g, "yinghun", 0, data(&[("event", "PhaseStart".into()), ("phase", "Prepare".into())]));
    submit(&mut g, "m1");
    submit(&mut g, "t:1");
    submit(&mut g, "c:d1");
    assert_eq!(g.hand_size(1), 1, "摸 2 弃 1");
    // 反例：未受伤不触发。
    let mut g2 = test_game(3);
    g2.seats[0].skills = vec!["yinghun".into()];
    g2.turn = 0;
    g2.phase = Phase::Prepare;
    trigger(&mut g2, "yinghun", 0, data(&[("event", "PhaseStart".into()), ("phase", "Prepare".into())]));
    assert!(g2.pending.is_none());
}

#[test]
fn wulie_grants_lie_which_cancels_all_damage() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["wulie".into()];
    g.turn = 0;
    g.phase = Phase::End;
    trigger(&mut g, "wulie", 0, data(&[("event", "PhaseStart".into()), ("phase", "End".into())]));
    submit(&mut g, "use");
    submit(&mut g, "x:1");
    submit(&mut g, "t:1");
    assert_eq!(g.seats[0].hp, 3, "失去 1 点体力");
    assert_eq!(g.counter("lie.1"), 1, "目标获得「烈」");
    let spec = sanguosha3::kernel::DamageSpec {
        from: Some(0),
        target: 1,
        amount: 3,
        kind: sanguosha3::kernel::DamageKind::Normal,
        source: sanguosha3::kernel::SourceKind::Other,
    };
    let armor = sanguosha3::kernel::gear::armor_of(&g, 1);
    sanguosha3::kernel::flow::damage(&mut g, spec, armor, 0);
    assert_eq!(g.seats[1].hp, 4, "「烈」抵消该次全部伤害（3 点）");
    assert_eq!(g.counter("lie.1"), 0, "移除 1 枚「烈」");
    // 反例：无「烈」时正常受伤。
    let spec2 = sanguosha3::kernel::DamageSpec {
        from: Some(0),
        target: 1,
        amount: 1,
        kind: sanguosha3::kernel::DamageKind::Normal,
        source: sanguosha3::kernel::SourceKind::Other,
    };
    let armor2 = sanguosha3::kernel::gear::armor_of(&g, 1);
    sanguosha3::kernel::flow::damage(&mut g, spec2, armor2, 0);
    assert_eq!(g.seats[1].hp, 3);
}

// ------------------------------ 界庞德 -------------------------------------

#[test]
fn jiandu_basic_branch_gives_slash_and_continues() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["jiandu".into()];
    mk2(&mut g, "sha1", "sha", Suit::Club, 5, Zone::Processing);
    mk2(&mut g, "c1", "tao", Suit::Heart, 3, Zone::Hand(1));
    trigger(
        &mut g,
        "jiandu",
        0,
        data(&[("step", "sha_targeted".into()), ("card", "sha1".into()), ("targets", json!([1]))]),
    );
    submit(&mut g, "use");
    submit(&mut g, "c:c1");
    assert_eq!(g.cards.get("c1"), Some(&Zone::Hand(1)), "基本牌仍归目标");
    assert_eq!(g.cards.get("sha1"), Some(&Zone::Hand(1)), "其获得此【杀】");
    // 非基本牌分支。
    let mut g2 = test_game(2);
    g2.seats[0].skills = vec!["jiandu".into()];
    mk2(&mut g2, "sha2", "sha", Suit::Club, 5, Zone::Processing);
    mk2(&mut g2, "c2", "qinggang_jian", Suit::Spade, 3, Zone::Hand(1));
    trigger(
        &mut g2,
        "jiandu",
        0,
        data(&[("step", "sha_targeted".into()), ("card", "sha2".into()), ("targets", json!([1]))]),
    );
    submit(&mut g2, "use");
    submit(&mut g2, "c:c2");
    assert!(g2.use_opts("sha2").unresponsive.contains(&1), "其不能抵消");
    assert_eq!(g2.counter("turn.0.jiandu_sha"), 1, "本回合次数 +1（最多 +1）");
    assert_eq!(g2.cards.get("c2"), Some(&Zone::Discard), "非基本牌被弃置");
}

#[test]
fn mashu_is_shared_from_b9() {
    // 马术复用 B9 注册实现：本批不重写，只需确认距离修正仍生效。
    let mut g = test_game(3);
    g.seats[0].skills = vec!["mashu".into()];
    assert_eq!(sanguosha3::kernel::gear::distance(&g, 0, 2), 1, "0→2 环距 2 -1 = 1");
}
