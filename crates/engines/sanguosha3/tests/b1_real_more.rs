//! B1 收口（续）：8 个「事件触发类」技能的真实路由 + 真实待办用例。
//!
//! 这些技能此前依赖测试注入的 `ask.data`，真实事件路由下会代选（`cands.first()`）
//! 或静默 `Done`。本文件验证：**先询问发动/放弃 → 强制选牌（不得代选）→ 真实结算**。

mod common;

use common::{begin, game, pending_actor, submit};
use sanguosha3::kernel::skills;
use sanguosha3::kernel::turn;
use sanguosha3::kernel::{Game, Identity, Phase, Zone};

fn give(g: &mut Game, seat: usize, id: &str, def: &str) {
    common::add(g, id, def, Zone::Hand(seat));
}

fn pending_skill(g: &Game) -> Option<String> {
    g.pending.as_ref().and_then(|p| p.skill().map(|s| s.to_string()))
}

fn answer(g: &mut Game, option: &str) {
    let actor = pending_actor(g).expect("有 pending 必有 actor");
    submit(g, actor, option).expect("提交待办");
}

fn start_with(g: &mut Game, seat: usize, skills_list: &[&str]) {
    g.seats[seat].skills = skills_list.iter().map(|s| s.to_string()).collect();
    turn::start(g).expect("开局");
}

#[test]
fn qizhi_asks_then_forces_pick_in_real_play() {
    let mut g = game(3);
    begin(&mut g);
    common::clear_hand(&mut g, 0);
    common::clear_hand(&mut g, 1);
    give(&mut g, 0, "s1", "sha");
    give(&mut g, 0, "sp", "tao");
    common::add(&mut g, "f1", "shan", Zone::Hand(1));
    common::add(&mut g, "f2", "shan", Zone::Hand(1));
    start_with(&mut g, 0, &["qizhi"]);
    submit(&mut g, 0, "card:s1").expect("出杀");
    answer(&mut g, "t:1");
    answer(&mut g, "pass"); // 不闪避，保留目标手牌供奇制选
    assert_eq!(pending_skill(&g), Some("qizhi".into()), "奇制必须询问");
    let before = g.hand_size(1);
    answer(&mut g, "use");
    assert_eq!(
        g.pending.as_ref().map(|p| p.kind().to_string()),
        Some("pick_other".into())
    );
    let pick = g.hand(1)[0].clone();
    answer(&mut g, &format!("card:{pick}"));
    assert_eq!(g.counter("turn.0.qizhi"), 1, "奇制计数 +1");
    assert_eq!(g.hand_size(1), before, "弃一张 + 摸一张");
}

#[test]
fn juzhan_yang_asks_in_real_other_turn() {
    let mut g = game(3);
    begin(&mut g);
    common::clear_hand(&mut g, 0);
    common::clear_hand(&mut g, 1);
    give(&mut g, 0, "s1", "sha");
    give(&mut g, 0, "sp", "tao");
    start_with(&mut g, 1, &["juzhan"]);
    submit(&mut g, 0, "card:s1").expect("出杀");
    answer(&mut g, "t:1");
    // 目标无【闪】→ 直接结算，随后拒战询问（阳面）。
    assert_eq!(pending_skill(&g), Some("juzhan".into()), "拒战阳面必须询问");
    let hand0 = g.hand_size(0);
    let hand1 = g.hand_size(1);
    answer(&mut g, "pass");
    assert_eq!(g.hand_size(0), hand0, "放弃不摸牌");
    assert_eq!(g.hand_size(1), hand1);
    assert_eq!(skills::face_of(&g, 1, "juzhan"), 0, "放弃不转面");
}

#[test]
fn juzhan_yin_asks_in_real_own_turn() {
    let mut g = game(3);
    begin(&mut g);
    common::clear_hand(&mut g, 0);
    common::clear_hand(&mut g, 1);
    give(&mut g, 0, "s1", "sha");
    give(&mut g, 0, "sp", "tao");
    common::add(&mut g, "f1", "shan", Zone::Hand(1));
    common::add(&mut g, "f2", "shan", Zone::Hand(1));
    start_with(&mut g, 0, &["juzhan"]);
    skills::flip_face(&mut g, 0, "juzhan");
    submit(&mut g, 0, "card:s1").expect("出杀");
    answer(&mut g, "t:1");
    answer(&mut g, "pass");
    assert_eq!(pending_skill(&g), Some("juzhan".into()));
    let pick = g.hand(1)[0].clone();
    answer(&mut g, "use");
    answer(&mut g, &format!("card:{pick}"));
    assert!(g.hand(0).contains(&pick), "获得目标一张牌");
    assert_eq!(skills::face_of(&g, 0, "juzhan"), 0, "发动后转面（1→0）");
}

#[test]
fn chezheng_asks_at_real_play_phase_end() {
    let mut g = game(4);
    begin(&mut g);
    common::clear_hand(&mut g, 0);
    common::clear_hand(&mut g, 2);
    give(&mut g, 0, "sp", "tao");
    common::add(&mut g, "x2", "shan", Zone::Hand(2));
    start_with(&mut g, 0, &["chezheng"]);
    assert!(!sanguosha3::kernel::skills::b1::in_attack_range(&g, 2, 0));
    submit(&mut g, 0, "end_play").expect("结束出牌");
    assert_eq!(pending_skill(&g), Some("chezheng".into()), "掣政②必须询问");
    let target_card = g.hand(2)[0].clone();
    answer(&mut g, "use");
    answer(&mut g, "seat:2");
    answer(&mut g, &format!("card:{target_card}"));
    assert_eq!(g.cards.get(&target_card), Some(&Zone::Discard), "弃其一张牌");
}

#[test]
fn jianxiang_asks_in_real_target_event() {
    let mut g = game(3);
    begin(&mut g);
    common::clear_hand(&mut g, 0);
    common::clear_hand(&mut g, 1);
    give(&mut g, 0, "s1", "sha");
    give(&mut g, 0, "sp", "tao");
    g.seats[1].skills = vec!["jianxiang".into()];
    turn::start(&mut g).expect("开局");
    submit(&mut g, 0, "card:s1").expect("出杀");
    answer(&mut g, "t:1");
    // 目标无【闪】→ 直接结算；荐降属于目标（座位 1）。
    assert_eq!(pending_skill(&g), Some("jianxiang".into()), "荐降必须询问");
    let before = g.hand_size(1) + g.hand_size(2);
    answer(&mut g, "use");
    if pending_skill(&g).as_deref() == Some("jianxiang") {
        answer(&mut g, "seat:1");
    }
    assert_eq!(g.hand_size(1) + g.hand_size(2), before + 1, "手牌最少者摸一张");
}

#[test]
fn lijun_asks_user_then_lord_in_real_use() {
    let mut g = game(3);
    begin(&mut g);
    common::clear_hand(&mut g, 0);
    common::clear_hand(&mut g, 1);
    give(&mut g, 1, "s1", "sha");
    give(&mut g, 1, "sp", "tao");
    g.seats[0].skills = vec!["lijun".into()];
    g.seats[0].identity = Some(Identity::Lord);
    g.seats[0].kingdom = "wu".into();
    g.seats[1].kingdom = "wu".into();
    g.turn = 1;
    g.phase = Phase::Play;
    sanguosha3::kernel::card_play::begin_use(&mut g, 1, "s1", false).unwrap();
    sanguosha3::kernel::resolve::advance(&mut g).unwrap();
    answer(&mut g, "t:2");
    assert_eq!(pending_skill(&g), Some("lijun".into()), "先问吴角色是否交牌");
    assert_eq!(pending_actor(&g), Some(1), "交牌询问属于使用者");
    answer(&mut g, "use");
    assert_eq!(pending_actor(&g), Some(0), "加成询问属于孙亮");
    let hand1 = g.hand_size(1);
    answer(&mut g, "use");
    assert!(g.hand(0).contains(&"s1".to_string()), "杀交给孙亮");
    assert_eq!(g.hand_size(1), hand1 + 1, "其摸一张");
    assert_eq!(skills::b1::sha_extra(&g, 1), 1, "本回合【杀】次数 +1");
}

#[test]
fn shicai_asks_on_real_card_use() {
    let mut g = game(3);
    begin(&mut g);
    common::clear_hand(&mut g, 0);
    give(&mut g, 0, "t1", "wuzhong_shengyou");
    give(&mut g, 0, "sp", "tao");
    start_with(&mut g, 0, &["shicai"]);
    submit(&mut g, 0, "card:t1").expect("使用无中");
    while g
        .pending
        .as_ref()
        .map(|p| p.kind() == "wuxie")
        .unwrap_or(false)
    {
        answer(&mut g, "pass");
    }
    assert_eq!(pending_skill(&g), Some("shicai".into()), "恃才必须询问");
    answer(&mut g, "pass");
    assert_eq!(g.counter("turn.0.shicai.trick"), 0, "放弃不消耗次数");
    assert_eq!(g.cards.get("t1"), Some(&Zone::Discard), "放弃时牌留在弃牌堆");
}

#[test]
fn kuizhu_asks_at_real_discard_phase_end() {
    let mut g = game(3);
    begin(&mut g);
    common::clear_hand(&mut g, 0);
    for i in 0..6 {
        give(&mut g, 0, &format!("c{i}"), "sha");
    }
    start_with(&mut g, 0, &["kuizhu"]);
    submit(&mut g, 0, "end_play").expect("结束出牌");
    while g
        .pending
        .as_ref()
        .map(|p| p.kind() == "discard_phase")
        .unwrap_or(false)
    {
        let hand = g.hand(0);
        submit(&mut g, 0, &format!("card:{}", hand[0])).expect("弃牌");
    }
    assert_eq!(pending_skill(&g), Some("kuizhu".into()), "溃诛必须询问");
    let before = g.hand_size(1);
    answer(&mut g, "use:1");
    answer(&mut g, "t:1");
    answer(&mut g, "ok");
    // 溃诛结算后回合继续推进到座位 1 的摸牌阶段（+2），故净增 = 1 + 2。
    assert_eq!(g.hand_size(1), before + 3, "溃诛令其摸一张（随后进入其摸牌阶段 +2）");
}

#[test]
fn shenshi_yin_asks_on_real_damage() {
    let mut g = game(3);
    begin(&mut g);
    common::clear_hand(&mut g, 0);
    give(&mut g, 0, "mine", "sha");
    give(&mut g, 0, "sp", "tao");
    g.seats[0].skills = vec!["shenshi".into()];
    g.turn = 1; // 回合外
    skills::flip_face(&mut g, 0, "shenshi");
    let spec = sanguosha3::kernel::DamageSpec {
        from: Some(1),
        target: 0,
        amount: 1,
        kind: sanguosha3::kernel::DamageKind::Normal,
        source: sanguosha3::kernel::SourceKind::Other,
    };
    sanguosha3::kernel::flow::damage(&mut g, spec, sanguosha3::kernel::ArmorKind::None, 0);
    sanguosha3::kernel::resolve::advance(&mut g).unwrap();
    assert_eq!(pending_skill(&g), Some("shenshi".into()), "审时阴面必须询问");
    answer(&mut g, "use");
    answer(&mut g, "card:mine");
    assert!(g.hand(1).contains(&"mine".to_string()), "交给其一张牌");
    assert_eq!(skills::face_of(&g, 0, "shenshi"), 0, "发动后转面（1→0）");
}
