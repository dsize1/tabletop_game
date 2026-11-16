//! 判定流程（mechanics `judge-rewrite`）：亮判定牌 → **改判窗口**（鬼才）→ 生效 → **天妒** → 落盘。
//!
//! 不变量：
//! 1. 判定牌只能从牌堆顶翻出（`judge::reveal`），摸牌方向钩子不影响判定；
//! 2. 改判只能通过 `Game.judge_rewrite` 单通道（技能写、流程读后清空），原判定牌进弃牌堆；
//! 3. 结果只通过 `Game.judge_result` 单通道交给发起方（父帧在下一轮进入时读，读后 `take`）；
//! 4. 判定牌最终去向：天妒获得 → 否则进弃牌堆（不进「使用/打出」路径）。
//!
//! 调用范式（父帧会一直被保留，子帧结算完父帧再次被 step）：
//! ```ignore
//! quick_judge::start(g, seat, "刚烈", None);
//! return Ok(Step::Next(Stage::Choose, json!({ "step": "judge_wait" })));
//! // 下次进入 "judge_wait" 时读 g.judge_result.take()
//! ```
//! 非帧发起方（例如 `flow::enter_dying`）用 `resume`：完成后把结果以 `step="result"` 的询问
//! 交给指定技能 handler 收口。

use serde_json::{json, Value};

use super::resolve::{Stage, Step};
use super::skills;
use super::{CardId, Game, Zone};

/// 工序名（唯一事实源；handler/父帧按它分支）。
pub const STEP_REVEAL: &str = "reveal";
pub const STEP_REWRITE: &str = "rewrite";
pub const STEP_EFFECTIVE: &str = "effective";
pub const STEP_AFTER_EFFECTIVE: &str = "after_effective";

/// 开始一次「带改判/天妒窗口」的判定。
///
/// `resume`：完成后的续接技能（非帧发起方用；如不屈）。帧发起方传 `None`，
/// 并自行以 `Step::Next` 保持父帧、下一轮读 `Game::judge_result`。
pub fn start(
    g: &mut Game,
    seat: usize,
    cause: &str,
    resume: Option<(&'static str, usize, Value)>,
) {
    let resume = resume.map(|(skill, actor, data)| {
        json!({ "skill": skill, "actor": actor, "data": data })
    });
    super::resolve::push_flow(
        g,
        super::resolve::FlowKind::QuickJudge,
        seat,
        Stage::QuickJudge,
        json!({
            "seat": seat,
            "cause": cause,
            "step": STEP_REVEAL,
            "card": Value::Null,
            "cursor": 0,
            "gain_asked": false,
            "resume": resume,
        }),
    );
}

fn card_of(d: &Value) -> CardId {
    d.get("card")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

/// 流程帧推进（`card_play::step_flow` 唯一调用点）。
pub fn step(g: &mut Game, data: &Value) -> Result<Step, String> {
    let seat = data.get("seat").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let cause = data
        .get("cause")
        .and_then(|v| v.as_str())
        .unwrap_or("判定")
        .to_string();
    let mut d = data.clone();
    let step = data
        .get("step")
        .and_then(|v| v.as_str())
        .unwrap_or(STEP_REVEAL);
    match step {
        STEP_REVEAL => match super::judge::reveal(g, seat, &cause) {
            Some(r) => {
                d["card"] = json!(r.card);
                d["step"] = json!(STEP_REWRITE);
                Ok(Step::Next(Stage::QuickJudge, d))
            }
            None => {
                g.judge_result = None;
                Ok(finish(g, &d))
            }
        },
        STEP_REWRITE => {
            // ① 应用上一次改判：原判定牌进弃牌堆，改判牌作为新判定牌。
            if let Some(new_card) = g.judge_rewrite.take() {
                let old = card_of(&d);
                if !old.is_empty()
                    && old != new_card
                    && matches!(g.cards.get(&old), Some(Zone::Processing))
                {
                    super::card_play::to_discard(g, &old);
                }
                d["card"] = json!(new_card);
            }
            // ② 链式询问：判定者左手侧起、判定者最后（mechanics `trigger-order`）。
            let card = card_of(&d);
            let holders = skills::judge_rewrite_holders(g, seat);
            let mut cursor = d.get("cursor").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            while cursor < holders.len() {
                let (who, code) = holders[cursor];
                cursor += 1;
                d["cursor"] = json!(cursor);
                // 无牌可换 → 不询问（constitution I1：只有合法集为空才不询问）。
                if skills::judge_rewrite_cards_for(g, who, code).is_empty() {
                    continue;
                }
                super::resolve::push_ask(
                    g,
                    code,
                    Stage::Choose,
                    who,
                    json!({ "step": "ask", "judge_seat": seat, "card": card, "cause": cause }),
                );
                return Ok(Step::Next(Stage::QuickJudge, d));
            }
            d["step"] = json!(STEP_EFFECTIVE);
            Ok(Step::Next(Stage::QuickJudge, d))
        }
        STEP_EFFECTIVE => {
            let card = card_of(&d);
            let r = super::judge::effective(g, seat, &cause, &card)
                .ok_or_else(|| "判定：判定牌实例缺失".to_string())?;
            g.judge_result = Some(r);
            d["step"] = json!(STEP_AFTER_EFFECTIVE);
            Ok(Step::Next(Stage::QuickJudge, d))
        }
        _ => {
            // 天妒：判定牌生效后，判定者可以获得**生效的这张**判定牌。
            let card = card_of(&d);
            let asked = d.get("gain_asked").and_then(|v| v.as_bool()).unwrap_or(false);
            if !asked && matches!(g.cards.get(&card), Some(Zone::Processing)) {
                if let Some(code) = skills::judge_gain_owner(g, seat) {
                    d["gain_asked"] = json!(true);
                    super::resolve::push_ask(
                        g,
                        code,
                        Stage::Choose,
                        seat,
                        json!({ "card": card, "cause": cause }),
                    );
                    return Ok(Step::Next(Stage::QuickJudge, d));
                }
            }
            // 判定生效后的统一钩子（雷击：鬼道改判自己的判定后仍可发动）。
            if !d.get("post").and_then(|v| v.as_bool()).unwrap_or(false) {
                d["post"] = json!(true);
                if skills::on_judge_effective(g, seat) {
                    return Ok(Step::Next(Stage::QuickJudge, d));
                }
            }
            if matches!(g.cards.get(&card), Some(Zone::Processing)) {
                super::card_play::to_discard(g, &card);
            }
            Ok(finish(g, &d))
        }
    }
}

/// 收尾：判定牌已落盘；有 `resume` 则把结果交给续接技能（`step="result"`）。
fn finish(g: &mut Game, d: &Value) -> Step {
    if let Some(r) = d.get("resume") {
        let skill = r.get("skill").and_then(|v| v.as_str()).unwrap_or_default();
        let actor = r.get("actor").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        if !skill.is_empty() {
            let mut data = r.get("data").cloned().unwrap_or_else(|| json!({}));
            if let Some(obj) = data.as_object_mut() {
                obj.insert("step".to_string(), json!("result"));
                // 把判定结果注入续接数据（技能无需再读全局通道）。
                if let Some(res) = &g.judge_result {
                    obj.insert("suit".to_string(), json!(format!("{:?}", res.suit)));
                    obj.insert("rank".to_string(), json!(res.rank));
                    obj.insert("judge_card".to_string(), json!(res.card));
                    obj.insert("red".to_string(), json!(res.suit.is_red()));
                }
            }
            super::resolve::push_ask(g, skill.to_string(), Stage::Choose, actor, data);
        }
    }
    Step::Done
}
