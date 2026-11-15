//! 扣置声明 + 质疑链（mechanics `hidden-declaration`；蛊惑类技能）。
//!
//! 不变量（`align/mechanics.json`）：
//! 1. **扣置期间不公开**：承载牌留在手牌区（视图对他人隐藏手牌），不进入公共可见集合；
//! 2. 质疑是**一次性收集**的选择（逐个私有 pending，互不可见）；
//! 3. 真假只读承载牌的物理定义（`cards::def`），禁止按声明字符串猜测；
//! 4. 假 → 此牌作废（进弃牌堆），质疑者各摸一张；真 → 质疑者获得「缠怨」并自选弃一张牌或失去 1 点体力；
//! 5. 结果唯一通道：`Game.guhuo_result`（发起技能读后清空）。

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::pending::PendingBuilder;
use super::resolve::{push_flow, FlowKind, Stage, Step};
use super::{CardId, Game, Zone};

/// 扣置声明结果（内核写、发起技能读）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuhuoResult {
    pub user: usize,
    pub carrier: CardId,
    /// 声明的牌定义 code。
    pub declared: String,
    /// 承载牌真实定义 code。
    pub real: String,
    /// 声明是否为真。
    pub truth: bool,
    /// 质疑者（按座位顺序）。
    pub doubters: Vec<usize>,
}

/// 发起扣置声明：承载牌必须在该角色手牌；`declared` 必须是合法牌定义 code。
pub fn start(
    g: &mut Game,
    user: usize,
    carrier: &str,
    declared: &str,
    cause: &str,
) -> Result<(), String> {
    if !matches!(g.cards.get(carrier), Some(Zone::Hand(s)) if *s == user) {
        return Err("扣置声明：承载牌必须在自己手牌".into());
    }
    if super::cards::def(declared).is_none() {
        return Err(format!("扣置声明：未知牌名 {declared}"));
    }
    push_flow(
        g,
        FlowKind::Guhuo,
        user,
        Stage::Choose,
        json!({
            "user": user,
            "carrier": carrier,
            "declared": declared,
            "cause": cause,
            "step": "doubt",
            "idx": 0,
            "doubters": [],
        }),
    );
    Ok(())
}

/// 质疑链流程帧 handler（`FlowKind::Guhuo`）。
pub fn step_guhuo(g: &mut Game, data: &serde_json::Value) -> Result<Step, String> {
    let user = data.get("user").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let carrier = data
        .get("carrier")
        .and_then(|v| v.as_str())
        .ok_or("扣置声明：缺承载牌")?
        .to_string();
    let declared = data
        .get("declared")
        .and_then(|v| v.as_str())
        .ok_or("扣置声明：缺声明牌名")?
        .to_string();
    let step = data.get("step").and_then(|v| v.as_str()).unwrap_or("doubt");

    let mut next = data.clone();
    match step {
        "doubt" => {
            // 其他存活角色按座位顺序逐个私有询问。
            let idx = data.get("idx").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let order: Vec<usize> = (0..g.seats.len())
                .filter(|&s| s != user && g.seats[s].alive)
                .collect();
            if idx >= order.len() {
                next["step"] = json!("reveal");
                return Ok(Step::Next(Stage::Confirm, next));
            }
            let asker = order[idx];
            if let Some(ans) = super::resolve::take_answer(g, "guhuo") {
                if ans == "doubt" {
                    let mut list = next["doubters"].as_array().cloned().unwrap_or_default();
                    list.push(json!(asker));
                    next["doubters"] = json!(list);
                } else if ans != "pass" {
                    return Err(format!("扣置声明：未知选项 {ans}"));
                }
                next["idx"] = json!(idx + 1);
                return Ok(Step::Next(Stage::Choose, next));
            }
            let p = PendingBuilder::new(asker, "guhuo")
                .skill("guhuo")
                .option("doubt", "质疑（若为假，你摸一张）")
                .data(json!({ "stage": "doubt", "carrier": carrier }))
                .escape("不质疑")
                .build();
            return Ok(Step::NeedAt(Stage::Choose, p));
        }
        "reveal" => {
            // 扣置期结束：当下公开承载牌（真假只读物理定义）。
            g.reveal(&[carrier.clone()], "扣置声明亮牌");
            let real = g
                .card_defs
                .get(&carrier)
                .map(|c| c.def.clone())
                .ok_or("扣置声明：承载牌无定义")?;
            let truth = real == declared;
            let doubters: Vec<usize> = next["doubters"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_u64().map(|x| x as usize))
                        .collect()
                })
                .unwrap_or_default();
            // 假：此牌作废（进弃牌堆），质疑者各摸一张。
            if !truth {
                g.cards.insert(carrier.clone(), Zone::Discard);
                let ev = super::event::Event::card(
                    super::event::EventKind::CardDiscarded,
                    Some(user),
                    carrier.clone(),
                    Some("扣置声明为假".to_string()),
                );
                let e = ev.snapshot(g);
                g.push_event(e);
                for d in &doubters {
                    super::flow::draw(g, *d, 1);
                }
            } else {
                // 真：质疑者获得「缠怨」（独立状态），并逐个自选弃一张牌或失去 1 点体力。
                for d in &doubters {
                    if let Some(s) = g.seats.get_mut(*d) {
                        s.add_counter("chanyuan", 1);
                    }
                }
            }
            g.conceal(&[carrier.clone()], "扣置声明结算完成");
            next["step"] = json!(if truth { "penalty" } else { "done" });
            next["truth"] = json!(truth);
            next["real"] = json!(real);
            next["cursor"] = json!(0);
            if truth && !doubters.is_empty() {
                return Ok(Step::Next(Stage::Confirm, next));
            }
            g.guhuo_result = Some(GuhuoResult {
                user,
                carrier: carrier.clone(),
                declared,
                real,
                truth,
                doubters,
            });
            Ok(Step::Done)
        }
        "penalty" => {
            let truth = data.get("truth").and_then(|v| v.as_bool()).unwrap_or(false);
            let real = data
                .get("real")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let doubters: Vec<usize> = data
                .get("doubters")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_u64().map(|x| x as usize))
                        .collect()
                })
                .unwrap_or_default();
            let mut cursor = data.get("cursor").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            if cursor >= doubters.len() {
                g.guhuo_result = Some(GuhuoResult {
                    user,
                    carrier: carrier.clone(),
                    declared,
                    real,
                    truth,
                    doubters,
                });
                return Ok(Step::Done);
            }
            let d = doubters[cursor];
            if let Some(ans) = super::resolve::take_answer(g, "guhuo") {
                if let Some(id) = ans.strip_prefix("discard:") {
                    if !matches!(g.cards.get(id), Some(Zone::Hand(s)) if *s == d) {
                        return Err("扣置声明：只能弃置自己的手牌".into());
                    }
                    super::flow::discard(g, d, id, "缠怨");
                } else if ans == "lose_hp" {
                    super::flow::lose_hp(g, d, 1, "缠怨");
                } else {
                    return Err(format!("扣置声明：未知选项 {ans}"));
                }
                cursor += 1;
                next["cursor"] = json!(cursor);
                return Ok(Step::Next(Stage::Confirm, next));
            }
            let mut b = PendingBuilder::new(d, "guhuo")
                .skill("guhuo")
                .data(json!({ "stage": "penalty" }));
            for id in g.hand(d) {
                b = b.option(format!("discard:{id}"), "弃置一张手牌");
            }
            b = b.option("lose_hp", "失去 1 点体力");
            Ok(Step::NeedAt(
                Stage::Confirm,
                b.escape_mandatory("必须选择一项").build(),
            ))
        }
        "done" => {
            g.guhuo_result = Some(GuhuoResult {
                user,
                carrier,
                declared,
                real: data
                    .get("real")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                truth: data.get("truth").and_then(|v| v.as_bool()).unwrap_or(false),
                doubters: data
                    .get("doubters")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_u64().map(|x| x as usize))
                            .collect()
                    })
                    .unwrap_or_default(),
            });
            Ok(Step::Done)
        }
        other => Err(format!("扣置声明：未知阶段 {other}")),
    }
}
