//! M5.1 交互闭环 + 通用濒死/死亡（2026-11-13 裁定 A）。
//!
//! 覆盖：
//! 1. `resolve::submit` 的询问完整性（I1）：actor / 合法选项 / 强制选择出口拒绝；
//! 2. `skills::active_actions` / `use_active` 的单一入口（窗口 + 可用性）；
//! 3. 通用 `FlowKind::Dying`：出桃救回 / 无人救 → `settle_death`（弃牌 → 奖惩 → 胜负）。

use sanguosha3::kernel::flow;
use sanguosha3::kernel::resolve::{self, advance};
use sanguosha3::kernel::skills;
use sanguosha3::kernel::turn;
use sanguosha3::kernel::{Card, DamageKind, DamageSpec, Game, Identity, Phase, Seat, SourceKind, Suit, Winner, Zone};

fn seat_with(id: usize, skills: &[&str], identity: Option<Identity>) -> Seat {
    Seat {
        id,
        hp: 4,
        max_hp: 4,
        alive: true,
        skills: skills.iter().map(|s| s.to_string()).collect(),
        identity,
        ..Default::default()
    }
}

fn game(skills: &[&str]) -> Game {
    Game {
        seats: vec![
            seat_with(0, skills, Some(Identity::Lord)),
            seat_with(1, &[], Some(Identity::Rebel)),
            seat_with(2, &[], Some(Identity::Spy)),
        ],
        ..Default::default()
    }
}

fn add_card(g: &mut Game, id: &str, def: &str, zone: Zone) {
    g.cards.insert(id.to_string(), zone);
    g.card_defs.insert(
        id.to_string(),
        Card {
            id: id.to_string(),
            def: def.to_string(),
            suit: Suit::Spade,
            rank: 7,
        },
    );
}

fn add_deck(g: &mut Game, ids: &[&str]) {
    for id in ids {
        add_card(g, id, "sha", Zone::Draw);
        g.deck.push(id.to_string());
    }
}

fn hand_size(g: &Game, seat: usize) -> usize {
    g.hand_size(seat)
}

fn damage_for(g: &mut Game, from: usize, target: usize, amount: i32) {
    let armor = sanguosha3::kernel::gear::armor_of(g, target);
    flow::damage(
        g,
        DamageSpec {
            from: Some(from),
            target,
            amount,
            kind: DamageKind::Normal,
            source: SourceKind::Skill,
        },
        armor,
        0,
    );
}

// ---------------------------------------------------------------------------
// submit：询问完整性
// ---------------------------------------------------------------------------

#[test]
fn submit_rejects_wrong_actor_and_illegal_option() {
    let mut g = game(&["feijun"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add_card(&mut g, "own", "sha", Zone::Hand(0));
    add_card(&mut g, "t0", "tao", Zone::Hand(1));
    add_card(&mut g, "t1", "shan", Zone::Hand(1));
    skills::use_active(&mut g, 0, "feijun").unwrap();
    assert!(g.pending.is_some(), "可选技能必须显式询问");
    // 非本人提交
    assert!(resolve::submit(&mut g, 1, "use:1").is_err());
    // 非法选项
    assert!(resolve::submit(&mut g, 0, "no-such-option").is_err());
    // 失败不改状态（pending 仍在）
    assert!(g.pending.is_some());
    // 合法提交
    resolve::submit(&mut g, 0, "use:1").unwrap();
    assert!(g.pending.is_some(), "进入下一阶段（选代价）");
}

#[test]
fn submit_rejects_escape_of_mandatory_step() {
    let mut g = game(&["feijun"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add_card(&mut g, "own", "sha", Zone::Hand(0));
    add_card(&mut g, "t0", "tao", Zone::Hand(1));
    add_card(&mut g, "t1", "shan", Zone::Hand(1));
    skills::use_active(&mut g, 0, "feijun").unwrap();
    resolve::submit(&mut g, 0, "use:1").unwrap();
    resolve::submit(&mut g, 0, "card:own").unwrap();
    let p = g.pending.as_ref().expect("选目标待办");
    assert!(p.escape_hidden(), "强制选择类：出口只作类型占位");
    assert!(resolve::submit(&mut g, 0, "pass").is_err(), "强制选择不接受出口");
}

#[test]
fn active_actions_follow_window_and_limits() {
    let mut g = game(&["chenglue", "jinzou"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add_card(&mut g, "a", "sha", Zone::Hand(0));
    add_card(&mut g, "b", "tao", Zone::Hand(0));
    let acts = skills::active_actions(&g, 0);
    assert!(acts.contains(&"chenglue"), "出牌阶段：成略可发动");
    assert!(!acts.contains(&"jinzou"), "进趋只在结束阶段");
    // 非当前回合玩家没有主动入口
    assert!(skills::active_actions(&g, 1).iter().all(|c| *c != "chenglue"));
    // 每阶段一次：发动后入口消失
    skills::use_active(&mut g, 0, "chenglue").unwrap();
    resolve::submit(&mut g, 0, "use").unwrap();
    resolve::submit(&mut g, 0, "card:a").unwrap();
    resolve::submit(&mut g, 0, "card:b").unwrap();
    assert!(!skills::active_actions(&g, 0).contains(&"chenglue"));
}

// ---------------------------------------------------------------------------
// 通用濒死：出桃救回
// ---------------------------------------------------------------------------

#[test]
fn dying_rescue_by_tao() {
    let mut g = game(&[]);
    g.seats[1].hp = 1;
    add_card(&mut g, "peach", "tao", Zone::Hand(2));
    damage_for(&mut g, 0, 1, 1);
    assert_eq!(g.seats[1].hp, 0, "进入濒死");
    advance(&mut g).unwrap();
    // 询问顺序：濒死者本人 → 下家（座位 2）
    assert_eq!(g.pending.as_ref().map(|p| p.actor()), Some(1));
    resolve::submit(&mut g, 1, "pass").unwrap();
    assert_eq!(g.pending.as_ref().map(|p| p.actor()), Some(2));
    resolve::submit(&mut g, 2, "card:peach").unwrap();
    assert_eq!(g.seats[1].hp, 1, "出【桃】救回");
    assert!(g.seats[1].alive);
    assert_eq!(g.cards.get("peach"), Some(&Zone::Discard), "桃进弃牌堆");
    assert!(g.winner.is_none());
}

// ---------------------------------------------------------------------------
// 通用死亡：无人救 → 弃牌 + 奖惩 + 胜负
// ---------------------------------------------------------------------------

#[test]
fn dying_without_rescue_settles_death_and_rebel_reward() {
    let mut g = game(&[]);
    g.seats[1].hp = 1;
    add_card(&mut g, "h1", "sha", Zone::Hand(1));
    add_card(&mut g, "e1", "qinggang_jian", Zone::Equip(1, sanguosha3::kernel::Slot::Weapon));
    add_deck(&mut g, &["d0", "d1", "d2"]);
    damage_for(&mut g, 0, 1, 1);
    advance(&mut g).unwrap();
    // 三个存活座位依次放弃（1 → 2 → 0）
    for _ in 0..3 {
        let actor = g.pending.as_ref().map(|p| p.actor()).expect("濒死询问");
        resolve::submit(&mut g, actor, "pass").unwrap();
    }
    assert!(!g.seats[1].alive, "无人出桃 → 死亡");
    assert_eq!(g.cards.get("h1"), Some(&Zone::Discard), "死亡弃所有手牌");
    assert_eq!(g.cards.get("e1"), Some(&Zone::Discard), "死亡弃装备");
    assert_eq!(hand_size(&g, 0), 3, "杀死反贼：击杀者摸 3");
    assert_eq!(g.winner, None, "内奸仍存活 → 未终局");
}

#[test]
fn lord_killing_loyalist_discards_all_cards() {
    let mut g = game(&[]);
    g.seats[1].identity = Some(Identity::Loyalist);
    g.seats[1].hp = 1;
    add_card(&mut g, "l0", "sha", Zone::Hand(0));
    add_card(&mut g, "l1", "tao", Zone::Hand(0));
    add_card(&mut g, "lw", "qinggang_jian", Zone::Equip(0, sanguosha3::kernel::Slot::Weapon));
    damage_for(&mut g, 0, 1, 1);
    advance(&mut g).unwrap();
    for _ in 0..3 {
        let actor = g.pending.as_ref().map(|p| p.actor()).expect("濒死询问");
        resolve::submit(&mut g, actor, "pass").unwrap();
    }
    assert!(!g.seats[1].alive);
    assert_eq!(hand_size(&g, 0), 0, "主公杀忠臣：弃所有手牌");
    assert_eq!(g.cards.get("lw"), Some(&Zone::Discard), "主公杀忠臣：弃装备区牌");
}

#[test]
fn lord_death_ends_with_rebel_win() {
    let mut g = game(&[]);
    g.seats[0].hp = 1;
    damage_for(&mut g, 1, 0, 1);
    advance(&mut g).unwrap();
    for _ in 0..3 {
        let actor = g.pending.as_ref().map(|p| p.actor()).expect("濒死询问");
        resolve::submit(&mut g, actor, "pass").unwrap();
    }
    assert_eq!(g.winner, Some(Winner::Rebel), "主公死 → 反贼胜");
}

// ---------------------------------------------------------------------------
// 回合结束校验（TurnEndCheck）：只在结束阶段派发，跨回合清理
// ---------------------------------------------------------------------------

#[test]
fn turn_end_check_is_kept_until_end_phase_and_purged_next_turn() {
    use sanguosha3::kernel::deferred::{Deferred, DeferredKind, Payload};
    let mut g = game(&[]);
    g.turn = 0;
    g.phase = Phase::Play;
    g.deferred.push(Deferred {
        kind: DeferredKind::TurnEndCheck,
        payload: Payload::Ask {
            skill: "shenshi".into(),
            stage: sanguosha3::kernel::resolve::Stage::Choose,
            actor: 0,
            data: serde_json::json!({ "hook": "end", "turn": 0, "src": 1, "card": "x" }),
        },
    });
    advance(&mut g).unwrap();
    assert_eq!(g.deferred.len(), 1, "未到结束阶段：保留在队列");
    // 进入下一回合：陈旧项被清理
    g.turn = 1;
    g.phase = Phase::Play;
    advance(&mut g).unwrap();
    assert!(g.deferred.is_empty(), "跨回合后 TurnEndCheck 被清理");
}

#[test]
fn settle_death_is_idempotent() {
    let mut g = game(&[]);
    let evs = turn::settle_death(&mut g, 1, Some(0));
    assert_eq!(evs.len(), 1);
    assert!(turn::settle_death(&mut g, 1, Some(0)).is_empty(), "重复结算不重复触发");
}
