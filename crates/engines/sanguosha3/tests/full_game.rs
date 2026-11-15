//! P6 生命周期 + B0 前置项总验收：**不添加任何英雄技能**，5 人局从开局跑到终局。
//!
//! 验收口径（`memory_palace/plan/next-v3-rebuild.md` §0.1）：
//! 不添加任何英雄技能，游戏即可真机验证「引擎 + 手牌 + 装备牌 + 锦囊牌」。
//! 本文件用「自动策略」把一局打到底，并在**每一步**校验：
//! 1. 牌区守恒 / 无帧遗留（I2 + zone-ownership）；
//! 2. 询问完整性（每个 pending 都有出口，选项永不为空）；
//! 3. 事件生产（使用/摸牌/弃置/伤害/死亡/阶段/回合 都真实出现）。

mod common;

use common::{add, clear_hand, game, pending_actor, submit};
use sanguosha3::kernel::event::EventKind;
use sanguosha3::kernel::turn;
use sanguosha3::kernel::{Game, Identity, Winner, Zone};

fn pick(g: &Game) -> Option<String> {
    let p = g.pending.as_ref()?;
    let opts: Vec<String> = p.options().iter().map(|o| o.id.clone()).collect();
    let escape = p.escape_id().to_string();
    let has = |prefix: &str| opts.iter().find(|o| o.starts_with(prefix)).cloned();
    let kind = p.kind();
    // 每个 pending 都必须有出口（I1）。
    assert!(opts.contains(&escape), "{kind} 的出口 {escape} 不在选项里");
    assert!(!opts.is_empty(), "{kind} 选项为空");
    let choice = match kind {
        // 出牌阶段：贪心出牌（优先装备/杀/锦囊），无牌则结束。
        "play_act" => has("card:")
            .or_else(|| has("skill:"))
            .or_else(|| has("zhangba"))
            .unwrap_or(escape),
        // 需要选目标：优先确定，其次选目标，绝不取消。
        "sgs3_target" => {
            if opts.contains(&"ok".to_string()) {
                "ok".to_string()
            } else {
                has("t:").unwrap_or(escape)
            }
        }
        // 弃牌阶段：弃第一张。
        "discard_phase" => has("card:").unwrap_or(escape),
        // 响应：能出就出（保持游戏推进），否则放弃。
        "shan_ask" | "aoe_ask" | "duel_ask" | "dying" | "sha_blocked" | "wuxie" => {
            has("s:").or_else(|| has("p:")).or_else(|| has("card:")).or_else(|| has("again:"))
                .or_else(|| has("w:"))
                .unwrap_or(escape)
        }
        // 其余（选牌 / 二选一 / 五谷）：取第一个实质选项。
        _ => opts
            .iter()
            .find(|o| **o != escape)
            .cloned()
            .unwrap_or(escape),
    };
    Some(choice)
}

fn check_invariants(g: &Game, step: usize) {
    assert_eq!(g.check_conservation(), Ok(()), "第 {step} 步：牌区守恒");
    assert_eq!(g.check_no_orphan_frames(), Ok(()), "第 {step} 步：无帧遗留");
    for (id, zone) in &g.cards {
        assert!(g.card_defs.contains_key(id), "第 {step} 步：牌 {id} 无定义");
        assert!(!zone.is_frame(), "第 {step} 步：牌 {id} 停在帧暂存区");
    }
}

/// 自动对局：返回提交次数；`max_steps` 用尽仍未终局时返回 None。
fn run(g: &mut Game, max_steps: usize) -> Option<usize> {
    turn::start(g).expect("开局");
    for step in 0..max_steps {
        check_invariants(g, step);
        match pick(g) {
            Some(choice) => {
                let seat = pending_actor(g).expect("有 pending 必有 actor");
                submit(g, seat, &choice).unwrap_or_else(|e| panic!("第 {step} 步提交 {choice} 失败：{e}"));
            }
            None => return Some(step),
        }
    }
    None
}

#[test]
fn full_game_two_players_reaches_winner() {
    let mut g = game(2);
    turn::begin_game(&mut g, &[Identity::Rebel], 4);
    // 双方 1 体力；牌堆全是【杀】→ 先手一刀必死，可验证终局链路。
    g.seats[0].hp = 1;
    g.seats[1].hp = 1;
    let steps = run(&mut g, 200).expect("2 人局应能打到底");
    assert!(steps > 0);
    assert!(g.winner.is_some(), "终局已判定");
    assert!(g.log.iter().any(|e| e.kind == EventKind::Death));
    check_invariants(&g, steps);
}

#[test]
fn full_game_five_players_without_skills() {
    let mut g = game(5);
    let identities = vec![
        Identity::Rebel,
        Identity::Loyalist,
        Identity::Rebel,
        Identity::Spy,
    ];
    turn::begin_game(&mut g, &identities, 4);
    // 让对局有终点：全场 2 体力（无技能也能靠【杀】推到底）。
    for s in &mut g.seats {
        s.hp = 2;
    }
    // 每人一张【闪】，确保「打出牌」事件（CardPlayed）也会真实发生。
    for s in 0..g.seats.len() {
        let id = format!("sh{s}");
        add(&mut g, &id, "shan", Zone::Hand(s));
    }
    let steps = run(&mut g, 5000).expect("5 人局应在预算内结束");
    assert!(g.winner.is_some(), "胜负已判定：{:?}", g.winner);
    check_invariants(&g, steps);

    // 事件生产（P2）：所有关键事实都由结算路径 emit，而不是测试手推。
    for kind in [
        EventKind::TurnStart,
        EventKind::TurnEnd,
        EventKind::PhaseStart,
        EventKind::PhaseEnd,
        EventKind::CardDrawn,
        EventKind::CardUsed,
        EventKind::CardPlayed,
        EventKind::CardLost,
        EventKind::CardDiscarded,
        EventKind::TargetChosen,
        EventKind::DamageAfter,
        EventKind::Death,
    ] {
        assert!(
            g.log.iter().any(|e| e.kind == kind),
            "事件 {kind:?} 未出现在对局日志里"
        );
    }
    // 事件快照必须带 turn/phase（路由只读快照）。
    let dmg = g.log.iter().find(|e| e.kind == EventKind::DamageAfter).unwrap();
    assert!(dmg.turn < g.seats.len());
    assert!(dmg.damage.is_some(), "伤害事件带二维");
}

#[test]
fn full_game_six_players_turn_loop_is_stable() {
    let mut g = game(6);
    let identities = vec![
        Identity::Rebel,
        Identity::Loyalist,
        Identity::Rebel,
        Identity::Spy,
        Identity::Loyalist,
    ];
    turn::begin_game(&mut g, &identities, 4);
    for s in &mut g.seats {
        s.hp = 3;
    }
    let steps = run(&mut g, 6000).expect("6 人局应在预算内结束");
    assert!(g.winner.is_some());
    // 人数为 6，主公先手；终局时死者牌区已清理。
    assert_eq!(g.seats.len(), 6);
    check_invariants(&g, steps);
}

#[test]
fn game_with_equipment_and_tricks_progresses() {
    // 手工给主公一手「装备 + 锦囊 + 杀」，确认这些牌在无技能状态下能打出并结算。
    let mut g = game(4);
    turn::begin_game(
        &mut g,
        &[Identity::Rebel, Identity::Loyalist, Identity::Spy],
        4,
    );
    clear_hand(&mut g, 0);
    for (id, def) in [
        ("e1", "qinggang_jian"),
        ("e2", "bagua_zhen"),
        ("t1", "wuzhong_shengyou"),
        ("t2", "nanman_ruqin"),
        ("t3", "lebu_sishu"),
        ("t4", "tiesuo_lianhuan"),
        ("t5", "wugu_fengdeng"),
        ("s1", "sha"),
    ] {
        add(&mut g, id, def, Zone::Hand(0));
    }
    for s in &mut g.seats {
        s.hp = 3;
    }
    let steps = run(&mut g, 5000).expect("有装备/锦囊的局应能结束");
    assert!(g.winner.is_some());
    // 装备/锦囊都真的用过（不是停在手牌里）。
    assert!(g
        .log
        .iter()
        .any(|e| e.kind == EventKind::CardUsed && e.card.as_deref() == Some("e1")));
    assert!(g
        .log
        .iter()
        .any(|e| e.kind == EventKind::CardUsed && e.card.as_deref() == Some("t3")));
    check_invariants(&g, steps);
}

#[test]
fn loser_identity_and_winner_values_are_consistent() {
    // 主死 → 反贼胜（单一胜负入口 `identity::check_winner`）。
    let mut g = game(3);
    turn::begin_game(&mut g, &[Identity::Rebel, Identity::Loyalist], 4);
    g.seats[0].alive = false;
    let w = turn::check_finished(&mut g);
    assert_eq!(w, Some(Winner::Rebel));
    let mut g2 = game(3);
    turn::begin_game(&mut g2, &[Identity::Rebel, Identity::Loyalist], 4);
    g2.seats[1].alive = false;
    g2.seats[2].alive = false;
    let w2 = turn::check_finished(&mut g2);
    assert_eq!(w2, Some(Winner::Lord), "反贼/内奸全灭 → 主忠胜");
}
