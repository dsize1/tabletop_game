//! 代出/代响应接力（激将/勤王的内核流程帧）。
//!
//! 语义（`align/B6.json` 勤王 + `align/B9.json` 激将）：
//! 1. 按**座位顺序**逐个询问「同势力、存活、有可代出【杀】」的角色；
//! 2. 代出者打出/使用其手牌中的【杀】（武将转化技由 `skills::response_as` 统一判定）；
//! 3. `for_use = true`（视为**发起者使用**）：牌不转手，由发起者在下一步选目标，
//!    伤害来源 = 发起者（`card_play::use_relay`）；
//! 4. `for_use = false`（视为**发起者打出**）：牌直接进弃牌堆，结果由上层响应窗口消费。
//!
//! 结果通道：`relay.result.<owner>` 文本标记（`played:<card>:<by>` / `declined`），
//! 由发起技能读取后清除；内核不在 `Game` 上加第二个结果字段。

use serde_json::json;

use super::card_play;
use super::cards::ResponseKind;
use super::pending::PendingBuilder;
use super::resolve::{take_answer, FlowKind, Stage, Step};
use super::{CardId, Game};

#[derive(Debug, Clone)]
pub struct RelayOpts {
    /// 可代出的势力（空串 = 不限势力）。
    pub kingdom: String,
    /// 代出的响应种类（激将/勤王 = 【杀】；护驾 = 【闪】）。
    pub kind: ResponseKind,
    /// true = 代出的牌被视为发起者**使用**；false = 视为**打出**。
    pub for_use: bool,
    /// 事件来源文案（激将/勤王/护驾）。
    pub cause: &'static str,
}

/// 可代出【杀】的座位（发起者左手侧起、同势力、存活、有可响应的手牌）。
pub fn order(g: &Game, owner: usize, kingdom: &str, kind: ResponseKind) -> Vec<usize> {
    let n = g.seats.len();
    (1..n)
        .map(|k| (owner + k) % n)
        .filter(|&s| {
            let seat = &g.seats[s];
            seat.alive
                && s != owner
                && (kingdom.is_empty() || seat.kingdom == kingdom)
                && g.hand(s)
                    .iter()
                    .any(|id| super::skills::response_as(g, s, id, kind).is_some())
        })
        .collect()
}

/// 发起接力：返回 false = 无人可代出（不压帧）。
pub fn start(g: &mut Game, owner: usize, opts: RelayOpts) -> bool {
    let order = order(g, owner, &opts.kingdom, opts.kind);
    if order.is_empty() {
        return false;
    }
    g.marks.remove(&format!("relay.result.{owner}"));
    super::resolve::push_flow(
        g,
        FlowKind::Relay,
        owner,
        Stage::Choose,
        json!({
            "owner": owner,
            "order": order,
            "cursor": 0,
            "for_use": opts.for_use,
            "kind": format!("{:?}", opts.kind),
            "cause": opts.cause,
            "step": "ask",
        }),
    );
    true
}

/// 接力流程帧 handler（`FlowKind::Relay`）。
pub fn step_relay(g: &mut Game, data: &serde_json::Value) -> Result<Step, String> {
    let owner = data.get("owner").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let order: Vec<usize> = data
        .get("order")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_u64()).map(|v| v as usize).collect())
        .unwrap_or_default();
    let mut cursor = data.get("cursor").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let for_use = data.get("for_use").and_then(|v| v.as_bool()).unwrap_or(false);
    let cause = data
        .get("cause")
        .and_then(|v| v.as_str())
        .unwrap_or("接力")
        .to_string();
    // 响应种类（默认【杀】，兼容既有数据）。
    let kind = match data.get("kind").and_then(|v| v.as_str()) {
        Some("Shan") => ResponseKind::Shan,
        Some("Wuxie") => ResponseKind::Wuxie,
        _ => ResponseKind::Sha,
    };
    while cursor < order.len() {
        let by = order[cursor];
        let alive = g.seats.get(by).map(|s| s.alive).unwrap_or(false);
        if !alive {
            cursor += 1;
            continue;
        }
        let cands: Vec<CardId> = g
            .hand(by)
            .into_iter()
            .filter(|id| super::skills::response_as(g, by, id, kind).is_some())
            .collect();
        if cands.is_empty() {
            cursor += 1;
            continue;
        }
        match take_answer(g, "relay") {
            None => {
                let mut b = PendingBuilder::new(by, "choose").skill("relay");
                for id in &cands {
                    b = b.option(
                        format!("respond:{id}"),
                        format!(
                            "代替{}打出【{}】",
                            g.seats.get(owner).map(|s| s.name.clone()).unwrap_or_default(),
                            g.card_defs
                                .get(id)
                                .and_then(|c| super::cards::def(&c.def))
                                .map(|d| d.name)
                                .unwrap_or("?")
                        ),
                    );
                }
                return Ok(Step::Need(
                    b.escape("不代出")
                        .data(json!({
                            "owner": owner,
                            "order": order,
                            "cursor": cursor,
                            "for_use": for_use,
                            "kind": format!("{kind:?}"),
                            "cause": cause.clone(),
                            "step": "ask",
                        }))
                        .build(),
                ));
            }
            Some(a) if a == "pass" => {
                cursor += 1;
                return Ok(Step::Next(
                    Stage::Choose,
                    json!({
                        "owner": owner,
                        "order": order,
                        "cursor": cursor,
                        "for_use": for_use,
                        "kind": format!("{kind:?}"),
                        "cause": cause.clone(),
                        "step": "ask",
                    }),
                ));
            }
            Some(a) if a.starts_with("respond:") => {
                let id = a[8..].to_string();
                if !cands.iter().any(|c| *c == id) {
                    return Err("接力：只能代出可响应的牌".into());
                }
                if for_use {
                    // 牌不转手：留在代出者手牌，由发起者用 `use_relay` 作为承载牌使用。
                } else {
                    card_play::played(g, by, &id, &cause)?;
                }
                g.set_mark(&format!("relay.result.{owner}"), format!("played:{id}:{by}"));
                return Ok(Step::Done);
            }
            Some(o) => return Err(format!("接力：未知选项 {o}")),
        }
    }
    g.set_mark(&format!("relay.result.{owner}"), "declined");
    Ok(Step::Done)
}
