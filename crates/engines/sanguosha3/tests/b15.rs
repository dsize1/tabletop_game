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
    // 酣战·拼点前：由参与者决定是否令对手随机拼点（独立窗口，不再内联天义）。
    submit(&mut g, "use");
    // 发起者（太史慈）仍需自选；目标为随机牌（不会出现 choose:b1）。
    submit(&mut g, "choose:a1");
    // 拼点后窗口：点数最大的实体【杀】(a1=9) → 询问是否获得。
    assert!(g.pending.is_some(), "拼点后应询问酣战是否获得【杀】");
    submit(&mut g, "gain:a1");
    assert!(g.hand(0).contains(&"a1".to_string()), "获得点数最大的拼点【杀】");
}

#[test]
fn hanzhan_windows_apply_to_any_pindian_source() {
    // 独立入口：非天义来源的拼点（驱虎/巧说等）同样触发酣战前后两段。
    let mut g = test_game(2);
    g.seats[0].skills = vec!["hanzhan".into()];
    g.turn = 0;
    mk(&mut g, "a1", "sha", Suit::Club, 9, Zone::Hand(0));
    mk(&mut g, "b1", "sha", Suit::Heart, 5, Zone::Hand(1));
    sanguosha3::kernel::pindian::start(&mut g, 0, 1, "测试拼点");
    resolve::advance(&mut g).expect("advance");
    // 前置窗口（持有者 0）
    assert_eq!(g.pending.as_ref().map(|p| p.actor()), Some(0));
    submit(&mut g, "use");
    submit(&mut g, "choose:a1");
    // 目标随机出牌 → 拼点结算 → 后置窗口（持有者 0）
    assert!(g.pending.is_some(), "后置窗口应询问");
    submit(&mut g, "gain:a1");
    assert!(g.hand(0).contains(&"a1".to_string()));
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
    assert_eq!(
        g.cards.get("sp"),
        Some(&Zone::Mark(0, "tian".to_string())),
        "非红桃 → 判定牌置于武将牌上为「田」"
    );
    assert_eq!(sanguosha3::kernel::skills::b15::tian_cards(&g, 0).len(), 1);
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
    assert!(sanguosha3::kernel::skills::b15::tian_cards(&g2, 0).is_empty(), "红桃不得田");
}

#[test]
fn zaoxian_awakens_at_three_tian_and_grants_jixi() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["zaoxian".into()];
    g.seats[0].hp = 4;
    g.turn = 0;
    g.phase = Phase::Prepare;
    for i in 0..3 {
        mk(&mut g, &format!("t{i}"), "sha", Suit::Club, 5, Zone::Mark(0, "tian".into()));
    }
    trigger(&mut g, "zaoxian", 0, data(&[("event", "PhaseStart".into()), ("phase", "Prepare".into())]));
    assert_eq!(g.seats[0].max_hp, 3, "上限 -1");
    assert!(skills::active(&g, 0, "jixi"), "获得【急袭】");
    assert_eq!(g.extra_turns, vec![0], "本回合后额外回合");
    // 反例：田 <3 不觉醒。
    let mut g2 = test_game(2);
    g2.seats[0].skills = vec!["zaoxian".into()];
    g2.turn = 0;
    g2.phase = Phase::Prepare;
    for i in 0..2 {
        mk(&mut g2, &format!("t{i}"), "sha", Suit::Club, 5, Zone::Mark(0, "tian".into()));
    }
    trigger(&mut g2, "zaoxian", 0, data(&[("event", "PhaseStart".into()), ("phase", "Prepare".into())]));
    assert_eq!(g2.seats[0].max_hp, 4);
}

#[test]
fn jixi_consumes_a_tian_as_snatch() {
    let mut g = test_game(2);
    g.seats[0].skills = vec!["jixi".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    mk(&mut g, "t0", "sha", Suit::Club, 5, Zone::Mark(0, "tian".into()));
    mk(&mut g, "h1", "tao", Suit::Heart, 3, Zone::Hand(1));
    use_active(&mut g, 0, "jixi");
    submit(&mut g, "t:1");
    assert!(
        sanguosha3::kernel::skills::b15::tian_cards(&g, 0).is_empty(),
        "「田」作为承载牌被消耗"
    );
    assert!(!matches!(g.cards.get("t0"), Some(Zone::Mark(..))), "田不再是武将牌上的牌");
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
fn changbiao_slash_uses_kernel_targets_and_draws_at_end_phase() {
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
    // 目标选择走内核单出口（`sgs3_target`），不再由长标自建目标列表。
    assert_eq!(g.pending.as_ref().map(|p| p.kind()), Some("sgs3_target"));
    submit(&mut g, "t:1");
    // 目标无【闪】→ 造成伤害 → 命中（伤害事件，非体力差）。
    assert_eq!(g.counter("turn.0.changbiao_hit"), 1, "造成伤害 → 记录命中");
    assert_eq!(g.counter("turn.0.changbiao_x"), 2, "X = 承载牌数");
    // 时序（用户裁定）：出牌阶段结束 / 弃牌阶段开始 **不摸**；结束阶段开始才摸。
    trigger(&mut g, "changbiao", 0, data(&[("event", "PhaseStart".into()), ("phase", "Play".into())]));
    trigger(&mut g, "changbiao", 0, data(&[("event", "PhaseStart".into()), ("phase", "Discard".into())]));
    assert_eq!(g.hand_size(0), 0, "弃牌阶段之前不摸牌");
    let before = g.hand_size(0);
    trigger(&mut g, "changbiao", 0, data(&[("event", "PhaseStart".into()), ("phase", "End".into())]));
    assert_eq!(g.hand_size(0), before + 2, "结束阶段（弃牌之后）摸 X 张");
    // 被【闪】抵消 → 不摸牌。
    let mut g2 = test_game(2);
    g2.seats[0].skills = vec!["changbiao".into()];
    g2.turn = 0;
    g2.phase = Phase::Play;
    mk(&mut g2, "c1", "sha", Suit::Club, 5, Zone::Hand(0));
    mk(&mut g2, "s1", "shan", Suit::Heart, 3, Zone::Hand(1));
    add_deck2(&mut g2, 2);
    use_active(&mut g2, 0, "changbiao");
    submit(&mut g2, "c:c1");
    submit(&mut g2, "confirm");
    submit(&mut g2, "t:1");
    submit(&mut g2, "s:s1");
    trigger(&mut g2, "changbiao", 0, data(&[("event", "PhaseStart".into()), ("phase", "End".into())]));
    assert_eq!(g2.hand_size(0), 0, "被闪抵消 → 不摸牌");
}

#[test]
fn changbiao_virtual_slash_has_no_attributes() {
    // 差异1：多张承载 → 无花色/点数；颜色判定恒否（仁王盾不拦）、点数比较判负。
    let mut g = test_game(2);
    g.seats[0].skills = vec!["changbiao".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    mk(&mut g, "c1", "sha", Suit::Spade, 5, Zone::Hand(0));
    mk(&mut g, "c2", "tao", Suit::Heart, 6, Zone::Hand(0));
    add_deck2(&mut g, 2);
    use_active(&mut g, 0, "changbiao");
    submit(&mut g, "c:c1");
    submit(&mut g, "c:c2");
    submit(&mut g, "confirm");
    let vid = g
        .mark("turn.0.changbiao_vid")
        .expect("虚拟【杀】id 应登记");
    assert_eq!(
        sanguosha3::kernel::cards::suit_of(&g, vid),
        None,
        "多张承载 → 无花色（None）"
    );
    assert!(!sanguosha3::kernel::cards::is_black(&g, vid), "无属性 → 不是黑色");
    assert!(!sanguosha3::kernel::cards::is_red(&g, vid), "无属性 → 不是红色");
    assert_eq!(
        sanguosha3::kernel::cards::rank_of(&g, vid),
        None,
        "无属性 → 无点数（比较方按 0 处理）"
    );
}

#[test]
fn changbiao_with_fangtian_can_pick_three_targets() {
    // 差异4：承载牌离手后手牌为空 → 方天画戟「最后的手牌」→ 最多 3 名目标。
    let mut g = test_game(4);
    g.seats[0].skills = vec!["changbiao".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    mk(&mut g, "ft", "fangtian_huaji", Suit::Diamond, 12, Zone::Equip(0, sanguosha3::kernel::Slot::Weapon));
    mk(&mut g, "c1", "sha", Suit::Club, 5, Zone::Hand(0));
    add_deck2(&mut g, 4);
    use_active(&mut g, 0, "changbiao");
    submit(&mut g, "c:c1");
    submit(&mut g, "confirm");
    // 手牌已空 → 目标上限 3。
    submit(&mut g, "t:1");
    submit(&mut g, "t:2");
    submit(&mut g, "t:3");
    assert_eq!(g.counter("turn.0.changbiao_x"), 1);
}

#[test]
fn changbiao_targets_respect_kernel_legality_filters() {
    // 差异5：目标合法性走 `legal_targets_with`（陷阵没赢 → 不能对其使用【杀】）。
    let mut g = test_game(2);
    g.seats[0].skills = vec!["changbiao".into(), "xianzhen".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    g.set_counter("turn.0.xianzhen_block.1", 1);
    mk(&mut g, "c1", "sha", Suit::Club, 5, Zone::Hand(0));
    add_deck2(&mut g, 2);
    use_active(&mut g, 0, "changbiao");
    submit(&mut g, "c:c1");
    submit(&mut g, "confirm");
    let opts = ids(&g);
    assert!(
        opts.iter().all(|o| o != "t:1"),
        "陷阵没赢 → 该角色不能成为【杀】目标：{opts:?}"
    );
}

#[test]
fn changbiao_cancel_returns_carriers_and_refunds_once_per_phase() {
    // 差异8：目标阶段取消 → 承载牌回手 + 「本阶段一次」退还。
    let mut g = test_game(2);
    g.seats[0].skills = vec!["changbiao".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    mk(&mut g, "c1", "sha", Suit::Club, 5, Zone::Hand(0));
    add_deck2(&mut g, 2);
    use_active(&mut g, 0, "changbiao");
    submit(&mut g, "c:c1");
    submit(&mut g, "confirm");
    submit(&mut g, "cancel");
    assert!(g.hand(0).contains(&"c1".to_string()), "承载牌回手");
    assert_eq!(g.counter("phase.0.changbiao"), 0, "限一次已退还");
    assert!(skills::active_actions(&g, 0).iter().any(|c| *c == "changbiao"), "可再次发动");
}

#[test]
fn changbiao_virtual_slash_is_not_blocked_by_renwang() {
    // 差异1 语义：无属性【杀】不是黑色 → 仁王盾「黑色【杀】无效」不生效。
    let mut g = test_game(2);
    g.seats[0].skills = vec!["changbiao".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    mk(
        &mut g,
        "rw",
        "renwang_dun",
        Suit::Club,
        2,
        Zone::Equip(1, sanguosha3::kernel::Slot::Armor),
    );
    mk(&mut g, "c1", "sha", Suit::Spade, 5, Zone::Hand(0));
    mk(&mut g, "c2", "sha", Suit::Spade, 6, Zone::Hand(0));
    add_deck2(&mut g, 2);
    let hp = g.seats[1].hp;
    use_active(&mut g, 0, "changbiao");
    submit(&mut g, "c:c1");
    submit(&mut g, "c:c2");
    submit(&mut g, "confirm");
    submit(&mut g, "t:1");
    assert_eq!(g.seats[1].hp, hp - 1, "无属性【杀】不被仁王盾拦截");
}

#[test]
fn changbiao_carrier_candidates_exclude_frozen_cards() {
    // 差异6：冻结/花色封锁/颜色封锁的牌不作为承载牌候选（legal ≡ execute）。
    let mut g = test_game(2);
    g.seats[0].skills = vec!["changbiao".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    mk(&mut g, "c1", "sha", Suit::Club, 5, Zone::Hand(0));
    mk(&mut g, "c2", "sha", Suit::Club, 6, Zone::Hand(0));
    g.freeze_cards(
        0,
        &["c1".to_string()],
        sanguosha3::kernel::FreezeUntil::Turn,
        "测试冻结",
    );
    use_active(&mut g, 0, "changbiao");
    let opts = ids(&g);
    assert!(opts.iter().all(|o| o != "c:c1"), "冻结牌不可选：{opts:?}");
    assert!(opts.iter().any(|o| o == "c:c2"));
}

#[test]
fn changbiao_armor_absorbed_damage_still_counts_as_hit() {
    // 差异3：护甲吸收伤害后体力不变，但 `DamageAfter` 仍成立 → 仍算命中。
    let mut g = test_game(2);
    g.seats[0].skills = vec!["changbiao".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    g.seats[1].armor = 3;
    mk(&mut g, "c1", "sha", Suit::Club, 5, Zone::Hand(0));
    add_deck2(&mut g, 3);
    use_active(&mut g, 0, "changbiao");
    submit(&mut g, "c:c1");
    submit(&mut g, "confirm");
    submit(&mut g, "t:1");
    assert_eq!(g.seats[1].hp, 4, "护甲吸收，体力不变");
    assert_eq!(g.counter("turn.0.changbiao_hit"), 1, "仍算造成伤害");
    let before = g.hand_size(0);
    trigger(&mut g, "changbiao", 0, data(&[("event", "PhaseStart".into()), ("phase", "End".into())]));
    assert_eq!(g.hand_size(0), before + 1, "按 X=1 摸牌");
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
