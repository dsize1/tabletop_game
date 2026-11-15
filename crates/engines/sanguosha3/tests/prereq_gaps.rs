//! B0 前置项收尾（缺口回归）：内奸独胜 / 桃终局房规 / 兵粮距离 / 铁索重铸 /
//! 装备替换弃置事件 / 回合内阵亡立即结束回合。

mod common;

use common::{add, add_suit, begin, clear_hand, game, has_option, refresh, start_play, submit, use_card};
use sanguosha3::kernel::event::EventKind;
use sanguosha3::kernel::{flow, turn, Game, Identity, Phase, Slot, Suit, Winner, Zone};

// ---------------------------------------------------------------------------
// 内奸独胜（rules §3）
// ---------------------------------------------------------------------------

#[test]
fn spy_wins_only_when_lord_and_spy_are_the_last_two() {
    // 用户 2026-11-14 口径：内奸胜 = 主公死亡时**场上仅存活主公与内奸**（其余全部已死）。
    // 1) 仅剩主公 + 内奸，内奸杀主 → 内奸胜。
    let mut g = game(4);
    turn::begin_game(
        &mut g,
        &[Identity::Rebel, Identity::Rebel, Identity::Spy],
        4,
    );
    g.seats[1].alive = false;
    g.seats[2].alive = false;
    turn::settle_death(&mut g, 0, Some(3));
    assert_eq!(g.winner, Some(Winner::Spy), "仅存活主公+内奸时杀主 → 内奸胜");

    // 2) 主公死亡时仍有忠臣存活（反贼已灭）→ 不满足内奸条件 → 反贼胜（主公先死）。
    let mut g2 = game(4);
    turn::begin_game(
        &mut g2,
        &[Identity::Rebel, Identity::Loyalist, Identity::Spy],
        4,
    );
    g2.seats[1].alive = false; // 反贼已灭
    turn::settle_death(&mut g2, 0, Some(3));
    assert_eq!(
        g2.winner,
        Some(Winner::Rebel),
        "内奸未达成（还有忠臣存活）→ 反贼胜"
    );

    // 3) 主公死亡且仍有反贼存活 → 反贼胜。
    let mut g3 = game(4);
    turn::begin_game(
        &mut g3,
        &[Identity::Rebel, Identity::Loyalist, Identity::Spy],
        4,
    );
    turn::settle_death(&mut g3, 0, Some(1));
    assert_eq!(g3.winner, Some(Winner::Rebel));

    // 4) 双内局：两名内奸同时存活时杀主 → 不算内奸达成（只能有一名）→ 反贼胜。
    let mut g4 = game(8);
    let ids = vec![
        Identity::Loyalist,
        Identity::Rebel,
        Identity::Rebel,
        Identity::Rebel,
        Identity::Rebel,
        Identity::Spy,
        Identity::Spy,
    ];
    turn::begin_game(&mut g4, &ids, 4);
    for s in 1..8 {
        if g4.seats[s].identity == Some(Identity::Rebel) {
            g4.seats[s].alive = false;
        }
    }
    // 存活：主公(0) + 两名内奸(6,7)。
    turn::settle_death(&mut g4, 0, Some(6));
    assert_eq!(g4.winner, Some(Winner::Rebel), "双内同时存活 → 内奸未达成");

    // 5) 反贼+内奸全灭 → 主忠胜。
    let mut g5 = game(3);
    turn::begin_game(&mut g5, &[Identity::Rebel, Identity::Spy], 4);
    g5.seats[1].alive = false;
    g5.seats[2].alive = false;
    assert_eq!(turn::check_finished(&mut g5), Some(Winner::Lord));
}

// ---------------------------------------------------------------------------
// 终局房规（ADR-0040）：第 100 轮后【桃】/【桃园结义】回血无效
// ---------------------------------------------------------------------------

#[test]
fn tao_does_not_heal_after_endgame_round() {
    let mut g = game(3);
    start_play(&mut g);
    g.round = 101;
    g.seats[0].hp = 2;
    add(&mut g, "tao1", "tao", Zone::Hand(0));
    use_card(&mut g, 0, "tao1").unwrap();
    assert_eq!(g.seats[0].hp, 2, "第 100 轮后【桃】不回血");
    assert_eq!(g.cards.get("tao1"), Some(&Zone::Discard), "牌仍进弃牌堆");

    // 桃园结义同样不回血。
    g.seats[1].hp = 2;
    add(&mut g, "ty", "taoyuan_jieyi", Zone::Hand(0));
    use_card(&mut g, 0, "ty").unwrap();
    assert_eq!(g.seats[0].hp, 2);
    assert_eq!(g.seats[1].hp, 2);

    // 濒死救援不受限（round 101 也能被桃救回）。
    g.seats[1].hp = 0;
    add(&mut g, "tao2", "tao", Zone::Hand(1));
    // 清掉出牌阶段待办（测试直接注入濒死，真实对局由伤害路径压栈）。
    g.pending = None;
    flow::enter_dying(&mut g, 1, Some(0));
    sanguosha3::kernel::resolve::advance(&mut g).unwrap();
    assert!(has_option(&g, "card:tao2"), "濒死仍可出桃");
    let seat = g.pending.as_ref().unwrap().actor();
    submit(&mut g, seat, "card:tao2").unwrap();
    assert_eq!(g.seats[1].hp, 1, "濒死救援不受终局房规限制");
}

// ---------------------------------------------------------------------------
// P4：兵粮寸断「距离 1 以内」
// ---------------------------------------------------------------------------

#[test]
fn bingliang_requires_distance_one() {
    let mut g = game(4);
    start_play(&mut g);
    add(&mut g, "bing", "bingliang_cunduan", Zone::Hand(0));
    refresh(&mut g);
    assert!(has_option(&g, "card:bing"));
    use_card(&mut g, 0, "bing").unwrap();
    // 4 人局：座位 3 距离 1 可选，座位 2 距离 2 不可选。
    assert!(has_option(&g, "t:1") || has_option(&g, "t:3"), "相邻座位可选");
    assert!(!has_option(&g, "t:2"), "距离 2 不可选：{:?}", g.pending);
}

// ---------------------------------------------------------------------------
// P4：铁索连环「重铸」
// ---------------------------------------------------------------------------

#[test]
fn iron_chain_recast_discards_and_draws() {
    let mut g = game(3);
    start_play(&mut g);
    add(&mut g, "tie", "tiesuo_lianhuan", Zone::Hand(0));
    refresh(&mut g);
    assert!(has_option(&g, "recast:tie"), "出牌阶段有重铸入口");
    let hand_before = g.hand_size(0);
    submit(&mut g, 0, "recast:tie").unwrap();
    assert_eq!(g.cards.get("tie"), Some(&Zone::Discard), "重铸：弃置此牌");
    assert_eq!(g.hand_size(0), hand_before, "弃 1 摸 1，手牌数不变");
    assert!(
        !g.log
            .iter()
            .any(|e| e.kind == EventKind::CardUsed && e.card.as_deref() == Some("tie")),
        "重铸不是「使用」"
    );
    assert_eq!(g.pending.as_ref().unwrap().kind(), "play_act", "重铸后继续出牌阶段");
}

// ---------------------------------------------------------------------------
// P2：装备被替换 → 旧牌进弃牌堆且发 CardDiscarded
// ---------------------------------------------------------------------------

#[test]
fn equipment_replacement_emits_discarded_event() {
    let mut g = game(3);
    start_play(&mut g);
    add(&mut g, "w1", "qinggang_jian", Zone::Hand(0));
    add(&mut g, "w2", "zhuge_liannu", Zone::Hand(0));
    use_card(&mut g, 0, "w1").unwrap();
    use_card(&mut g, 0, "w2").unwrap();
    assert_eq!(g.cards.get("w1"), Some(&Zone::Discard));
    assert!(
        g.log.iter().any(|e| e.kind == EventKind::CardDiscarded
            && e.card.as_deref() == Some("w1")),
        "旧装备进弃牌堆要发 CardDiscarded"
    );
}

// ---------------------------------------------------------------------------
// P1：当前回合角色阵亡 → 立即结束其回合
// ---------------------------------------------------------------------------

#[test]
fn dead_current_seat_ends_its_turn() {
    // 主公先手打完 → 座位 1 的回合；此时座位 1 阵亡（如闪电/反伤）→ 其回合应立即结束。
    // 4 人局（1 主 + 2 反 + 1 忠）：杀 1 个反贼不会立刻终局。
    let mut g = game(4);
    begin(&mut g);
    turn::start(&mut g).unwrap();
    common::end_turn(&mut g).unwrap();
    assert_eq!(g.turn, 1, "轮到座位 1");
    g.pending = None;
    turn::settle_death(&mut g, 1, Some(2));
    assert!(!g.seats[1].alive);
    assert!(g.winner.is_none(), "主公仍在，对局继续");
    sanguosha3::kernel::resolve::advance(&mut g).unwrap();
    assert_ne!(g.turn, 1, "死者回合立即结束");
    assert_eq!(g.turn, 2, "顺位到下一个存活座位");
    assert_eq!(g.pending.as_ref().map(|p| p.actor()), Some(2));
}

// ---------------------------------------------------------------------------
// 守恒 / 无帧遗留（收尾回归）
// ---------------------------------------------------------------------------

#[test]
fn gap_fixes_keep_invariants() {
    let mut g: Game = game(5);
    begin(&mut g);
    for s in 0..5 {
        clear_hand(&mut g, s);
    }
    add_suit(&mut g, "c1", "tiesuo_lianhuan", Zone::Hand(0), Suit::Club, 10);
    add_suit(&mut g, "c2", "tao", Zone::Hand(0), Suit::Heart, 3);
    g.seats[0].hp = 5;
    turn::start(&mut g).unwrap();
    refresh(&mut g);
    if has_option(&g, "recast:c1") {
        submit(&mut g, 0, "recast:c1").unwrap();
    }
    assert_eq!(g.check_conservation(), Ok(()));
    assert_eq!(g.check_no_orphan_frames(), Ok(()));
    assert_eq!(g.phase, Phase::Play);
    let _ = Slot::Weapon;
}
