//! M1.7 resolve 门禁：发起帧归属（唯一帧 id）+ 单延迟队列顺序表 + 帧 id 唯一。

use sanguosha3::kernel::deferred::{Deferred, DeferredKind, Payload, ORDER};
use sanguosha3::kernel::pending::PendingBuilder;
use sanguosha3::kernel::resolve::{
    apply_step, next_deferred_index, push_ask, push_flow, FlowKind, FrameInner, Stage, Step,
};
use sanguosha3::kernel::Game;
use serde_json::json;

// ---------------------------------------------------------------------------
// 发起帧归属（硬约束 B）：Step 作用于 frame_id，不是栈顶
// ---------------------------------------------------------------------------

#[test]
fn step_next_writes_initiating_frame_not_stack_top() {
    let mut g = Game::default();
    // 发起帧 A（id=0）
    push_ask(&mut g, "skill_a", Stage::Choose, 0, json!({}));
    // handler 运行中压入子帧 B（id=1，如濒死），并返回 Step::Next
    push_flow(&mut g, FlowKind::Dying, 1, Stage::DyingAsk, json!({}));

    // Step::Next 作用于发起帧 A（id=0），不是栈顶 B（id=1）
    apply_step(&mut g, 0, Step::Next(Stage::Target, json!({"x": 1}))).unwrap();

    // 栈顶是 B（id=1），其 data 未被污染
    assert_eq!(g.stack.len(), 2);
    let top = g.stack.last().unwrap();
    assert_eq!(top.id, 1);
    if let FrameInner::Flow(f) = &top.inner {
        assert_eq!(f.stage, Stage::DyingAsk, "子帧 B 阶段不受 Step::Next 影响");
    } else {
        panic!("栈顶应是流程帧");
    }
    // 发起帧 A（id=0）被改
    let a = g.stack.iter().find(|f| f.id == 0).unwrap();
    if let FrameInner::Ask(a) = &a.inner {
        assert_eq!(a.stage, Stage::Target);
        assert_eq!(a.data, json!({"x": 1}));
    } else {
        panic!("帧 0 应是技能帧");
    }
}

#[test]
fn step_done_removes_only_initiating_frame() {
    let mut g = Game::default();
    push_ask(&mut g, "a", Stage::Choose, 0, json!({}));
    push_ask(&mut g, "b", Stage::Choose, 0, json!({}));
    // Done 作用于发起帧 id=0，只删它，id=1 保留
    apply_step(&mut g, 0, Step::Done).unwrap();
    assert_eq!(g.stack.len(), 1);
    assert_eq!(g.stack[0].id, 1);
}

#[test]
fn apply_step_to_missing_frame_errors() {
    let mut g = Game::default();
    let r = apply_step(&mut g, 999, Step::Done);
    assert!(r.is_err(), "不存在的帧 id 应报错");
}

#[test]
fn frame_ids_are_unique_and_increasing() {
    let mut g = Game::default();
    push_ask(&mut g, "a", Stage::Choose, 0, json!({}));
    push_flow(&mut g, FlowKind::Dying, 0, Stage::DyingAsk, json!({}));
    push_ask(&mut g, "b", Stage::Target, 0, json!({}));
    let ids: Vec<u64> = g.stack.iter().map(|f| f.id).collect();
    assert_eq!(ids, vec![0, 1, 2], "帧 id 唯一且递增");
}

#[test]
fn need_sets_pending_and_keeps_frame() {
    let mut g = Game::default();
    push_ask(&mut g, "a", Stage::Choose, 0, json!({}));
    let p = PendingBuilder::new(0, "choice").escape("放弃").build();
    apply_step(&mut g, 0, Step::Need(p)).unwrap();
    assert!(g.pending.is_some());
    assert_eq!(g.stack.len(), 1, "Need 不弹帧，等待玩家");
}

// ---------------------------------------------------------------------------
// 单延迟队列：ORDER 顺序表
// ---------------------------------------------------------------------------

#[test]
fn deferred_drain_follows_order_table() {
    let mut g = Game::default();
    // 乱序插入
    g.deferred.push(Deferred {
        kind: DeferredKind::DeathCleanup,
        payload: Payload::JudgePhase { seat: 0 },
    });
    g.deferred.push(Deferred {
        kind: DeferredKind::Land,
        payload: Payload::Land { card: "c1".into(), to: sanguosha3::kernel::Zone::Discard },
    });
    g.deferred.push(Deferred {
        kind: DeferredKind::SkillTrigger,
        payload: Payload::JudgePhase { seat: 1 },
    });
    // 按 ORDER 顺序：Land < SkillTrigger < JudgePhase < DeathCleanup
    let mut drained = vec![];
    while let Some(i) = next_deferred_index(&g) {
        drained.push(g.deferred.remove(i).kind);
    }
    assert_eq!(
        drained,
        vec![DeferredKind::Land, DeferredKind::SkillTrigger, DeferredKind::DeathCleanup],
        "延迟队列按 ORDER 顺序表派发"
    );
}

#[test]
fn order_table_is_explicit_and_documented() {
    // 顺序表是常量即文档，防止顺序靠代码位置隐式决定。
    assert_eq!(
        ORDER,
        [
            DeferredKind::Land,
            DeferredKind::SkillTrigger,
            DeferredKind::JudgePhase,
            DeferredKind::TurnEndCheck,
            DeferredKind::DeathCleanup,
        ]
    );
}
