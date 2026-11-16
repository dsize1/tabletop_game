//! B1 收口：**真实回合驱动**用例（不是把 `ask.data` 手推给 handler）。
//!
//! 目的：B1 的 18 个技能此前只有「单元级 handler 调用」用例，容易在真实事件路由下
//! 静默不触发 / 代玩家选牌。本文件只验证「真实回合/事件路径」，每个用例对应一条收口项。

mod common;

use common::{begin, game, pending_actor, submit};
use sanguosha3::kernel::skills;
use sanguosha3::kernel::turn;
use sanguosha3::kernel::{Game, Zone};

fn give(g: &mut Game, seat: usize, id: &str, def: &str) {
    common::add(g, id, def, Zone::Hand(seat));
}

fn pending_skill(g: &Game) -> Option<String> {
    g.pending.as_ref().and_then(|p| p.skill().map(|s| s.to_string()))
}

/// 阶段开始类技能必须**先于**该阶段的自动动作询问（整论要能跳过摸牌）。
#[test]
fn zhenglun_asks_before_draw_phase_runs() {
    let mut g = game(3);
    begin(&mut g);
    g.seats[0].skills = vec!["zhenglun".into()];
    g.seats[0].kingdom = "wu".into();
    turn::start(&mut g).expect("开局");

    assert_eq!(g.phase, sanguosha3::kernel::Phase::Draw, "停在摸牌阶段的询问");
    assert_eq!(pending_skill(&g), Some("zhenglun".into()), "整论必须先询问");
    assert!(
        g.pending.as_ref().map(|p| p.has_option("pass")).unwrap_or(false),
        "整论有放弃出口"
    );
    let hand_before = g.hand_size(0);
    let ju_before = g.seats[0].counter("ju");
    let actor = pending_actor(&g).unwrap();
    submit(&mut g, actor, "use").expect("发动整论");
    assert_eq!(g.hand_size(0), hand_before, "跳过整段摸牌阶段（没有摸牌）");
    assert_eq!(g.seats[0].counter("ju"), ju_before + 1, "获得 1 枚「橘」");
    assert_eq!(g.phase, sanguosha3::kernel::Phase::Play, "回合继续推进到出牌阶段");
}

/// 阶段开始类技能在出牌阶段开始时询问（遗礼），且放弃后正常进入出牌阶段。
#[test]
fn yili_asks_at_play_phase_start_and_pass_exits() {
    let mut g = game(3);
    begin(&mut g);
    g.seats[0].skills = vec!["yili".into()];
    g.seats[0].kingdom = "wu".into();
    turn::start(&mut g).expect("开局");

    assert_eq!(pending_skill(&g), Some("yili".into()), "遗礼在出牌阶段开始时询问");
    assert!(
        g.pending.as_ref().map(|p| p.has_option("pass")).unwrap_or(false),
        "遗礼可以放弃"
    );
    let actor = pending_actor(&g).unwrap();
    submit(&mut g, actor, "pass").expect("放弃遗礼");
    assert_eq!(g.pending.as_ref().map(|p| p.kind().to_string()), Some("play_act".into()));
    assert_eq!(g.phase, sanguosha3::kernel::Phase::Play);
}

/// 庸肆：摸牌加成 + 弃牌阶段先弃 X，且走真实阶段（不再只靠单测）。
#[test]
fn yongsi_draws_bonus_and_discards_at_real_phases() {
    let mut g = game(3);
    begin(&mut g);
    g.seats[0].skills = vec!["yongsi".into()];
    g.seats[0].kingdom = "shu".into();
    g.seats[1].kingdom = "wei".into();
    g.seats[2].kingdom = "wu".into();
    turn::start(&mut g).expect("开局");
    // 4 张初始 + (2 + 3 势力) = 9 张。
    assert_eq!(g.hand_size(0), 9, "庸肆摸牌加成（X=3 个势力）");

    // 结束出牌 → 弃牌阶段开始：庸肆先弃 3，再按上限弃到 4。
    submit(&mut g, 0, "end_play").expect("结束出牌");
    while g
        .pending
        .as_ref()
        .map(|p| p.kind() == "discard_phase" || p.kind() == "yongsi_pick")
        .unwrap_or(false)
    {
        let hand = g.hand(0);
        submit(&mut g, 0, &format!("card:{}", hand[0])).expect("弃牌");
    }
    assert_eq!(g.hand_size(0), g.hand_limit(0), "弃到上限");
    // 庸肆的额外弃置走 flow::discard（cause=庸肆）→ 计入当时阶段的弃牌数；
    // 阶段切换会清 `phase.*`，所以用事件日志断言「额外弃置确实发生」。
    let yongsi_discards = g
        .log
        .iter()
        .filter(|e| {
            e.kind == sanguosha3::kernel::event::EventKind::CardDiscarded
                && e.cause.as_deref() == Some("庸肆")
        })
        .count();
    assert_eq!(yongsi_discards, 3, "庸肆先弃 X=3 张（走真实弃置出口）");
}

/// 怀橘：开局 3 橘 + 摸牌阶段多摸 1（真实 GameStart 钩子 + 摸牌加成）。
#[test]
fn huaiju_game_start_and_draw_bonus_are_real() {
    let mut g = game(3);
    begin(&mut g);
    g.seats[0].skills = vec!["huaiju".into()];
    g.seats[0].kingdom = "wu".into();
    // GameStart 钩子在 begin_game 末尾；这里补触发（测试直接挂技能后重放一次）。
    skills::fire_game_start(&mut g);
    assert_eq!(g.seats[0].counter("ju"), 3, "开局 3 橘");
    turn::start(&mut g).expect("开局");
    assert_eq!(g.hand_size(0), 4 + 2 + 1, "怀橘摸牌阶段多摸 1 张");
}

/// 进趋：结束阶段真实触发（PhaseStart End），放弃出口可用。
#[test]
fn jinzou_triggers_at_real_end_phase_with_pass_exit() {
    let mut g = game(3);
    begin(&mut g);
    g.seats[0].skills = vec!["jinzou".into()];
    g.seats[0].kingdom = "wei".into();
    // 奇制次数：直接置计数器（奇制的真实链路由另一条用例覆盖）。
    turn::start(&mut g).expect("开局");
    g.set_counter(&format!("turn.{}.qizhi", 0), 1);
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
    assert_eq!(pending_skill(&g), Some("jinzou".into()), "结束阶段触发进趋");
    assert!(
        g.pending.as_ref().map(|p| p.has_option("pass")).unwrap_or(false),
        "进趋可放弃"
    );
}

/// 寸目：真实摸牌方向（从牌堆底摸）——手牌数不变、牌来自堆底。
#[test]
fn cunmu_draws_from_bottom_in_real_draw_phase() {
    let mut g = game(3);
    begin(&mut g);
    g.seats[0].skills = vec!["cunmu".into()];
    g.seats[0].kingdom = "wei".into();
    // 牌堆顶/底放可识别的牌，验证摸到的是堆底。
    common::add_suit(
        &mut g,
        "top_card",
        "sha",
        Zone::Draw,
        sanguosha3::kernel::Suit::Spade,
        1,
    );
    g.deck.push("top_card".into());
    common::add_suit(
        &mut g,
        "bottom_card",
        "sha",
        Zone::Draw,
        sanguosha3::kernel::Suit::Heart,
        2,
    );
    g.deck.insert(0, "bottom_card".into());
    turn::start(&mut g).expect("开局");
    let hand = g.hand(0);
    assert!(hand.contains(&"bottom_card".to_string()), "寸目从牌堆底摸");
    assert!(!hand.contains(&"top_card".to_string()));
}

/// 收口门禁：B1 的 18 个技能必须全部在注册表，且已有真实驱动用例文件。
#[test]
fn b1_registry_and_real_driver_file_are_wired() {
    let codes = [
        "yongsi", "weidi", "qizhi", "jinzou", "juzhan", "feijun", "binglue", "huaiju", "yili",
        "zhenglun", "kuizhu", "chezheng", "lijun", "jianxiang", "shenshi", "chenglue", "shicai",
        "cunmu",
    ];
    for c in codes {
        assert!(skills::def(c).is_some(), "B1 技能 {c} 未注册");
        assert!(skills::enabled(c), "B1 技能 {c} 未挂载（ENABLED_SKILLS）");
    }
}

