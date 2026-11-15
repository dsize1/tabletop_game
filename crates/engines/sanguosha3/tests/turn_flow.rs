//! P1 回合驱动 + P6 生命周期门禁。
//!
//! 验收口径（B0 前置项）：
//! 1. 真实 turn loop：prepare→judge→draw→play→discard→end，产生 TurnStart/PhaseStart/PhaseEnd/TurnEnd；
//! 2. 无技能时回合能流转、阶段能推进、跳阶段/额外回合可插；
//! 3. 5–8 人局可从开局（发身份/选将/发牌）跑到终局（胜负置位后不再推进）；
//! 4. 中断（`abort`）清空结算栈与待办。

use sanguosha3::kernel::event::EventKind;
use sanguosha3::kernel::resolve::{submit, advance};
use sanguosha3::kernel::turn::{self, PHASES};
use sanguosha3::kernel::{
    Card, Game, Identity, Phase, Seat, Suit, Winner, Zone,
};

fn add_card(g: &mut Game, id: &str, def: &str, zone: Zone, suit: Suit, rank: u8) {
    g.cards.insert(id.to_string(), zone);
    g.card_defs.insert(
        id.to_string(),
        Card {
            id: id.to_string(),
            def: def.to_string(),
            suit,
            rank,
        },
    );
}

fn fill_deck(g: &mut Game, n: usize) {
    for i in 0..n {
        let id = format!("d{i}");
        add_card(g, &id, "sha", Zone::Draw, Suit::Spade, 7);
        g.deck.push(id);
    }
}

fn game3() -> Game {
    let mut g = turn::new_game(3);
    fill_deck(&mut g, 60);
    turn::begin_game(&mut g, &[Identity::Rebel, Identity::Loyalist], 4);
    g
}

fn count(g: &Game, k: EventKind) -> usize {
    g.log.iter().filter(|e| e.kind == k).count()
}

fn started_phases(g: &Game) -> Vec<Phase> {
    g.log
        .iter()
        .filter(|e| e.kind == EventKind::PhaseStart)
        .map(|e| e.phase)
        .collect()
}

fn pending_kind(g: &Game) -> (usize, String) {
    let p = g.pending.as_ref().expect("应有待办");
    (p.actor(), p.kind().to_string())
}

#[test]
fn start_runs_phases_and_stops_at_play_pending() {
    let mut g = game3();
    turn::start(&mut g).unwrap();
    // 有技能之外的交互：出牌阶段必须询问（询问完整性 I1）。
    assert_eq!(pending_kind(&g), (0, "play_act".into()));
    assert_eq!(g.phase, Phase::Play);
    assert_eq!(g.turn, 0, "主公先手");
    // 4 初始 + 摸牌阶段 2 = 6。
    assert_eq!(g.hand_size(0), 6);
    assert_eq!(g.hand_size(1), 4, "未到其回合的座位只有初始手牌");
    // 事件生产：TurnStart 1 / PhaseStart 依次为 Prepare, Judge, Draw, Play。
    assert_eq!(count(&g, EventKind::TurnStart), 1);
    assert_eq!(
        started_phases(&g),
        vec![Phase::Prepare, Phase::Judge, Phase::Draw, Phase::Play],
    );
    assert_eq!(count(&g, EventKind::PhaseEnd), 3, "前三阶段已结束");
    assert_eq!(g.check_conservation(), Ok(()));
    assert_eq!(g.check_no_orphan_frames(), Ok(()));
}

#[test]
fn end_play_runs_discard_and_advances_to_next_turn() {
    let mut g = game3();
    turn::start(&mut g).unwrap();
    submit(&mut g, 0, "end_play").unwrap();
    // 手牌 6 > 体力 4 → 弃牌阶段强制询问。
    assert_eq!(pending_kind(&g), (0, "discard_phase".into()));
    assert_eq!(g.phase, Phase::Discard);
    let hand = g.hand(0);
    submit(&mut g, 0, &format!("card:{}", hand[0])).unwrap();
    let hand = g.hand(0);
    submit(&mut g, 0, &format!("card:{}", hand[0])).unwrap();
    // 弃到上限 → 结束阶段 → 下一个回合（座位 1）出牌阶段。
    assert_eq!(count(&g, EventKind::TurnEnd), 1);
    assert_eq!(count(&g, EventKind::TurnStart), 2);
    assert_eq!(g.turn, 1);
    assert_eq!(pending_kind(&g), (1, "play_act".into()));
    assert_eq!(g.round, 0, "0→1 尚未绕圈，轮数不变");
}

#[test]
fn discard_phase_rejects_escape_option() {
    let mut g = game3();
    turn::start(&mut g).unwrap();
    submit(&mut g, 0, "end_play").unwrap();
    let err = submit(&mut g, 0, "pass").unwrap_err();
    assert!(err.contains("强制"), "强制选择不接受出口：{err}");
    assert_eq!(g.hand_size(0), 6, "拒绝后状态不变");
}

#[test]
fn round_increments_when_wrapping_to_lord() {
    let mut g = game3();
    turn::start(&mut g).unwrap();
    for _ in 0..3 {
        // 每个回合：结束出牌 → 弃到上限（可能有多次）→ 下一个回合。
        let seat = g.turn;
        assert_eq!(pending_kind(&g).0, seat);
        submit(&mut g, seat, "end_play").unwrap();
        while g.pending.as_ref().map(|p| p.kind() == "discard_phase").unwrap_or(false) {
            let hand = g.hand(seat);
            submit(&mut g, seat, &format!("card:{}", hand[0])).unwrap();
        }
    }
    // 3 人局：0→1→2→0 时轮数 +1。
    assert_eq!(g.turn, 0);
    assert_eq!(g.round, 1, "完成 0→1→2 一轮后轮数 +1");
}

#[test]
fn judge_phase_skips_play_when_lebu_judged_black() {
    let mut g = game3();
    // 判定区一张乐不思蜀；牌堆顶为黑桃（非红桃 → 跳过出牌阶段）。
    add_card(&mut g, "le1", "lebu_sishu", Zone::Judge(0), Suit::Spade, 5);
    g.judge_stacks.entry(0).or_default().push("le1".into());
    add_card(&mut g, "jt", "sha", Zone::Draw, Suit::Spade, 9);
    g.deck.push("jt".into());
    turn::start(&mut g).unwrap();
    assert!(
        !started_phases(&g).contains(&Phase::Play),
        "被乐：出牌阶段不发生（无 PhaseStart）"
    );
    assert_eq!(g.cards.get("le1"), Some(&Zone::Discard), "判定后进弃牌堆");
    assert_eq!(pending_kind(&g), (0, "discard_phase".into()));
    assert!(g.log.iter().any(|e| e.kind == EventKind::JudgeRevealed));
}

#[test]
fn judge_phase_keeps_play_when_lebu_judged_heart() {
    let mut g = game3();
    add_card(&mut g, "le1", "lebu_sishu", Zone::Judge(0), Suit::Club, 5);
    g.judge_stacks.entry(0).or_default().push("le1".into());
    add_card(&mut g, "jt", "tao", Zone::Draw, Suit::Heart, 9);
    g.deck.push("jt".into());
    turn::start(&mut g).unwrap();
    assert!(started_phases(&g).contains(&Phase::Play), "红桃：乐不思蜀无效");
    assert_eq!(pending_kind(&g), (0, "play_act".into()));
}

#[test]
fn extra_turn_is_consumed_before_normal_order() {
    let mut g = game3();
    turn::start(&mut g).unwrap();
    turn::extra_turn(&mut g, 2);
    submit(&mut g, 0, "end_play").unwrap();
    while g.pending.as_ref().map(|p| p.kind() == "discard_phase").unwrap_or(false) {
        let hand = g.hand(0);
        submit(&mut g, 0, &format!("card:{}", hand[0])).unwrap();
    }
    assert_eq!(g.turn, 2, "额外回合优先于顺时针下家");
}

#[test]
fn flipped_seat_skips_its_turn() {
    let mut g = game3();
    g.seats[1].flipped = true;
    turn::start(&mut g).unwrap();
    submit(&mut g, 0, "end_play").unwrap();
    while g.pending.as_ref().map(|p| p.kind() == "discard_phase").unwrap_or(false) {
        let hand = g.hand(0);
        submit(&mut g, 0, &format!("card:{}", hand[0])).unwrap();
    }
    assert_eq!(g.turn, 2, "翻面的座位 1 跳过回合");
    assert!(!g.seats[1].flipped, "翻面状态已消费");
}

#[test]
fn winner_stops_turn_loop() {
    let mut g = game3();
    turn::start(&mut g).unwrap();
    g.winner = Some(Winner::Rebel);
    submit(&mut g, 0, "end_play").unwrap();
    assert!(g.pending.is_none(), "终局后不再询问");
    assert_eq!(count(&g, EventKind::TurnStart), 1, "不再开新回合");
}

#[test]
fn abort_clears_stack_and_pending() {
    let mut g = game3();
    turn::start(&mut g).unwrap();
    assert!(g.pending.is_some());
    turn::abort(&mut g);
    assert!(g.pending.is_none());
    assert!(g.stack.is_empty());
    assert!(g.events.is_empty());
    assert!(!g.started);
}

#[test]
fn begin_game_with_generals_sets_kingdom_hp_and_gender() {
    let mut g = turn::new_game(3);
    fill_deck(&mut g, 60);
    // 曹仁（魏 4 体力）/ 陆绩（吴 3 体力）/ 貂蝉（群 3 体力·女）。
    turn::begin_game_with_generals(
        &mut g,
        &[Identity::Rebel, Identity::Loyalist],
        &["sgs_hero_29", "sgs_hero_402", "sgs_hero_100"],
        4,
    );
    assert_eq!(g.seats[0].kingdom, "wei");
    assert_eq!(g.seats[0].max_hp, 5, "主公 +1 体力上限");
    assert_eq!(g.seats[0].hp, 5);
    assert_eq!(g.seats[1].max_hp, 3);
    assert_eq!(g.seats[2].kingdom, "qun", "袁术=群");
    assert!(g.started);
    // 5 人局也能开局（P6 5–8 人）。
    let mut big = turn::new_game(8);
    fill_deck(&mut big, 100);
    let identities = vec![
        Identity::Rebel,
        Identity::Loyalist,
        Identity::Rebel,
        Identity::Spy,
        Identity::Loyalist,
        Identity::Rebel,
        Identity::Loyalist,
    ];
    turn::begin_game(&mut big, &identities, 4);
    turn::start(&mut big).unwrap();
    assert_eq!(big.seats.len(), 8);
    assert_eq!(pending_kind(&big), (0, "play_act".into()));
}

#[test]
fn phase_table_is_the_single_source() {
    assert_eq!(
        PHASES,
        [
            Phase::Prepare,
            Phase::Judge,
            Phase::Draw,
            Phase::Play,
            Phase::Discard,
            Phase::End
        ]
    );
    // 座位全死亡也不 panic（end_turn 返回 None）。
    let mut g = Game {
        seats: vec![Seat {
            id: 0,
            hp: 4,
            max_hp: 4,
            alive: false,
            ..Default::default()
        }],
        ..Default::default()
    };
    advance(&mut g).unwrap();
}
