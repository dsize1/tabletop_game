//! 拼点内核流程（mechanics `pindian`）：双方暗选 → 同时亮牌 → 比有效点数 → 拼点牌进弃牌堆。
//!
//! 不变量（`align/mechanics.json`）：
//! 1. **选择期不公开**：`pending` 只发给选择者本人（视图对他人隐藏 options/data）；
//! 2. 双方各自独立选择，选完才一起亮牌；
//! 3. 比较**有效点数**（牌面修正只作用于比较；本切片无改判钩子）；
//! 4. 平局 = 双方均未赢（`winner = None`）；
//! 5. 拼点牌结算后进弃牌堆；由内核流程帧驱动，技能只声明时机与结果消费方式。
//!
//! 结果唯一通道：`Game::take_pindian`（读完清空，禁止重复消费）。

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::pending::PendingBuilder;
use super::resolve::{push_flow, FlowKind, Stage, Step};
use super::{CardId, Game, Zone};

/// 拼点结果（内核写、发起技能读）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PindianResult {
    pub initiator: usize,
    pub target: usize,
    pub a: CardId,
    pub b: CardId,
    pub rank_a: u8,
    pub rank_b: u8,
    /// `None` = 平局（双方都未赢）。
    pub winner: Option<usize>,
}

impl PindianResult {
    pub fn initiator_won(&self) -> bool {
        self.winner == Some(self.initiator)
    }
}

/// 拼点能否成立：双方存活且都有手牌（合法集为空 → 技能入口不应出现）。
pub fn can_start(g: &Game, initiator: usize, target: usize) -> bool {
    if initiator == target {
        return false;
    }
    if super::skills::deny_pindian(g, target) {
        return false; // 谦节：不能成为其他角色拼点的目标
    }
    let ok = |i: usize| {
        g.seats
            .get(i)
            .map(|s| s.alive && !g.hand(i).is_empty())
            .unwrap_or(false)
    };
    ok(initiator) && ok(target)
}

/// 发起拼点：压入拼点流程帧（不推进，由外层 `advance` 驱动）。
pub fn start(g: &mut Game, initiator: usize, target: usize, cause: &str) {
    debug_assert!(initiator != target, "拼点双方不能是同一角色");
    if !can_start(g, initiator, target) {
        return;
    }
    push_flow(
        g,
        FlowKind::Pindian,
        initiator,
        Stage::PindianInit,
        json!({
            "initiator": initiator,
            "target": target,
            "a": null,
            "b": null,
            "step": "init",
            "cause": cause,
        }),
    );
}

fn seat_of(data: &serde_json::Value, key: &str) -> usize {
    data.get(key).and_then(|v| v.as_u64()).unwrap_or(0) as usize
}

fn card_of(data: &serde_json::Value, key: &str) -> Option<CardId> {
    data.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn hidden_escape(
    actor: usize,
    other: usize,
    game: &Game,
) -> PendingBuilder<super::pending::HasEscape> {
    let other_name = game
        .seats
        .get(other)
        .map(|s| s.name.clone())
        .unwrap_or_default();
    PendingBuilder::new(actor, "pindian")
        .skill("pindian")
        .data(json!({ "target": other_name, "stage": "choose" }))
        // 拼点一旦成立，双方都必须出一张手牌拼点（强制选择；出口只作类型占位）。
        .escape_mandatory("必须出一张手牌拼点")
}

/// 拼点流程帧 handler（`FlowKind::Pindian`）。
pub fn step_pindian(g: &mut Game, data: &serde_json::Value) -> Result<Step, String> {
    let initiator = seat_of(data, "initiator");
    let target = seat_of(data, "target");
    let a = card_of(data, "a");
    let b = card_of(data, "b");
    let step = data.get("step").and_then(|v| v.as_str()).unwrap_or("init");

    let mut next = data.clone();
    match step {
        "init" => {
            if a.is_none() {
                if let Some(ans) = super::resolve::take_answer(g, "pindian") {
                    let id = ans
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("拼点：未知选项 {ans}"))?;
                    let id = if id == "__deck" {
                        let top = g.deck.pop().ok_or("拼点：牌堆为空")?;
                        g.cards.insert(top.clone(), Zone::Processing);
                        top
                    } else {
                        id.to_string()
                    };
                    let id = id.as_str();
                    if !matches!(g.cards.get(id), Some(Zone::Hand(s)) if *s == initiator)
                        && !matches!(g.cards.get(id), Some(Zone::Processing))
                    {
                        return Err("拼点：只能用手牌拼点".into());
                    }
                    next["a"] = json!(id);
                    next["step"] = json!("resp");
                    return Ok(Step::Next(Stage::PindianResp, next));
                }
                let mut b_ = hidden_escape(initiator, target, g);
                for id in g.hand(initiator) {
                    b_ = b_.option(
                        format!("choose:{id}"),
                        format!(
                            "用【{}】拼点",
                            g.card_defs
                                .get(&id)
                                .and_then(|c| super::cards::def(&c.def))
                                .map(|d| d.name.to_string())
                                .unwrap_or_else(|| "?".into())
                        ),
                    );
                }
                if super::skills::pindian_deck_option(g, initiator) {
                    b_ = b_.option("choose:__deck", "用牌堆顶的牌拼点（天辩）");
                }
                return Ok(Step::NeedAt(Stage::PindianInit, b_.build()));
            }
            next["step"] = json!("resp");
            Ok(Step::Next(Stage::PindianResp, next))
        }
        "resp" => {
            if b.is_none() {
                if let Some(ans) = super::resolve::take_answer(g, "pindian") {
                    let id = ans
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("拼点：未知选项 {ans}"))?;
                    let id = if id == "__deck" {
                        let top = g.deck.pop().ok_or("拼点：牌堆为空")?;
                        g.cards.insert(top.clone(), Zone::Processing);
                        top
                    } else {
                        id.to_string()
                    };
                    let id = id.as_str();
                    if !matches!(g.cards.get(id), Some(Zone::Hand(s)) if *s == target)
                        && !matches!(g.cards.get(id), Some(Zone::Processing))
                    {
                        return Err("拼点：只能用手牌拼点".into());
                    }
                    next["b"] = json!(id);
                    next["step"] = json!("reveal");
                    return Ok(Step::Next(Stage::PindianReveal, next));
                }
                let mut b_ = hidden_escape(target, initiator, g);
                for id in g.hand(target) {
                    b_ = b_.option(
                        format!("choose:{id}"),
                        format!(
                            "用【{}】拼点",
                            g.card_defs
                                .get(&id)
                                .and_then(|c| super::cards::def(&c.def))
                                .map(|d| d.name.to_string())
                                .unwrap_or_else(|| "?".into())
                        ),
                    );
                }
                if super::skills::pindian_deck_option(g, target) {
                    b_ = b_.option("choose:__deck", "用牌堆顶的牌拼点（天辩）");
                }
                return Ok(Step::NeedAt(Stage::PindianResp, b_.build()));
            }
            next["step"] = json!("reveal");
            Ok(Step::Next(Stage::PindianReveal, next))
        }
        "reveal" => {
            let (Some(a), Some(b)) = (a, b) else {
                return Err("拼点：亮牌阶段缺牌".into());
            };
            ok_in_hand_or_processing(g, initiator, &a)?;
            ok_in_hand_or_processing(g, target, &b)?;
            // 同时亮牌（选择期结束才公开）。
            g.reveal(&[a.clone(), b.clone()], "拼点亮牌");
            let rank_a = super::skills::pindian_rank(g, initiator, &a);
            let rank_b = super::skills::pindian_rank(g, target, &b);
            let winner = if rank_a > rank_b {
                Some(initiator)
            } else if rank_b > rank_a {
                Some(target)
            } else {
                None
            };
            // 拼点牌进弃牌堆（唯一出口：进区 + 落盘事件）。
            for id in [&a, &b] {
                g.cards.insert(id.clone(), Zone::Discard);
                let ev = super::event::Event::card(
                    super::event::EventKind::CardDiscarded,
                    None,
                    id.clone(),
                    Some("拼点".to_string()),
                );
                let e = ev.snapshot(g);
                g.push_event(e);
            }
            g.conceal(&[a.clone(), b.clone()], "拼点结算完成");
            g.pindian_result = Some(PindianResult {
                initiator,
                target,
                a,
                b,
                rank_a,
                rank_b,
                winner,
            });
            Ok(Step::Done)
        }
        other => Err(format!("拼点：未知阶段 {other}")),
    }
}

fn ok_in_hand_or_processing(g: &Game, seat: usize, id: &str) -> Result<(), String> {
    match g.cards.get(id) {
        Some(Zone::Hand(s)) if *s == seat => Ok(()),
        // 已由技能预移到处理区（天辩/随机手牌等）也接受。
        Some(Zone::Processing) => Ok(()),
        _ => Err(format!("拼点牌 {id} 不在座位 {seat} 的手牌")),
    }
}
