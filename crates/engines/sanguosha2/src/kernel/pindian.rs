//! 拼点内核流程（mechanics `pindian`）：双方暗选 → 同时亮牌 → 比有效点数 → 拼点牌进弃牌堆。
//!
//! 不变量（用户 2026-11-06 / 2026-11-09 裁定）：
//! 1. **选择期不公开**（同怃戎）：`Step::Need` 期间不 reveal、不写牌面日志；
//! 2. 双方各自独立选择，选完才一起亮牌；
//! 3. 比较的是**有效点数**（`skills::pindian_rank`，天辩的红桃视为 K）；平局 = 无人赢；
//! 4. 拼点牌结算后进弃牌堆；若技能描述允许「获得拼点牌」，由技能在该描述下另行处理；
//! 5. 本文件**不得**出现任何武将技能名（钩子一律走 `skills::pindian_*`）。

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::resolve::{Ask, FlowFrame, FlowKind, Frame, Stage, Step};
use super::{CardId, Game};
use crate::skills;

/// 拼点结果（唯一结果通道：内核写、发起技能读后清空）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PindianResult {
    pub initiator: usize,
    pub target: usize,
    pub a: CardId,
    pub b: CardId,
    pub rank_a: u8,
    pub rank_b: u8,
    /// `None` = 平局（双方都未赢）
    pub winner: Option<usize>,
}

impl PindianResult {
    pub fn initiator_won(&self) -> bool {
        self.winner == Some(self.initiator)
    }
}

/// 发起拼点：压入拼点流程帧（不推进，由外层 `advance` 驱动）。
pub fn start(g: &mut Game, initiator: usize, target: usize, cause: &str) {
    debug_assert!(initiator != target, "拼点双方不能是同一角色");
    // 拼点前窗口（酣战：询问是否令对方随机手牌拼点）。
    let pre = skills::pindian_pre_owners(g, initiator, target);
    let step = if pre.is_empty() { "init" } else { "pre" };
    let pairs: Vec<serde_json::Value> = pre
        .iter()
        .map(|(o, other)| json!({ "owner": o, "other": other }))
        .collect();
    g.stack.push(Frame::Flow(FlowFrame {
        kind: FlowKind::Pindian,
        skill: cause.to_string(),
        actor: initiator,
        stage: Stage::PindianInit,
        data: json!({
            "initiator": initiator,
            "target": target,
            "a": null,
            "b": null,
            "step": step,
            "pre": pairs,
            "pre_cursor": 0,
        }),
    }));
}

fn seat_of(data: &serde_json::Value, key: &str) -> usize {
    data[key].as_u64().unwrap_or(0) as usize
}

fn card_of(data: &serde_json::Value, key: &str) -> Option<CardId> {
    data[key].as_str().filter(|s| !s.is_empty()).map(|s| s.to_string())
}

/// 拼点选牌询问（附加选项由技能钩子提供：天辩「用牌堆顶的牌」）。
///
/// 用 `g.stack.last_mut()` 保存 data（同 `card_play::need_at`，`Step::NeedAt` 不保存 data）。
fn ask_card(g: &mut Game, actor: usize, stage: Stage, data: serde_json::Value) -> Result<Step, String> {
    let mut b = skills::builder(actor, "pindian").skill("pindian");
    for id in g.hand(actor) {
        b = b.option(
            format!("choose:{id}"),
            format!("用【{}】拼点", g.card_label(&id)),
        );
    }
    for (id, label) in skills::pindian_options(g, actor) {
        b = b.option(id, label);
    }
    match g.stack.last_mut() {
        Some(Frame::Flow(f)) => {
            f.stage = stage;
            f.data = data;
        }
        _ => return Err("拼点：当前结算帧不可保存进度".into()),
    }
    Ok(Step::Need(b.escape("取消拼点（视为未赢）").build()))
}

/// 酣战：随机取一张手牌（不询问）。
fn random_choice(g: &mut Game, seat: usize) -> Option<CardId> {
    let n = g.hand(seat).len();
    if n == 0 {
        return None;
    }
    let idx = g.rand_below(n);
    Some(g.hand(seat)[idx].clone())
}

pub fn step_pindian(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let initiator = seat_of(&a.data, "initiator");
    let target = seat_of(&a.data, "target");
    let mut data = a.data.clone();
    let step = data["step"].as_str().unwrap_or("init").to_string();

    match step.as_str() {
        // 拼点前：逐个询问拥有「拼点前」技能的参与者（酣战）。
        "pre" => {
            let list = data["pre"].as_array().cloned().unwrap_or_default();
            let mut cursor = data["pre_cursor"].as_u64().unwrap_or(0) as usize;
            if cursor >= list.len() {
                data["step"] = json!("init");
                return Ok(Step::Next(Stage::PindianInit, data));
            }
            let owner = list[cursor]["owner"].as_u64().unwrap_or(0) as usize;
            let other = list[cursor]["other"].as_u64().unwrap_or(0) as usize;
            let skill = skills::pindian_pre_skill(owner, g);
            let _ = skill;
            match g.answer.take().as_deref() {
                None => {
                    let mut b = skills::builder(owner, "choice").skill("hanzhan");
                    b = b.option(
                        "opt1",
                        format!("酣战：令 {} 用随机手牌拼点", g.seats[other].name),
                    );
                    Ok(Step::Need(b.escape("不发动酣战").build()))
                }
                Some("pass") => {
                    cursor += 1;
                    data["pre_cursor"] = json!(cursor);
                    Ok(Step::Next(Stage::PindianInit, data))
                }
                Some(_) => {
                    skills::pindian_pre_apply(g, owner, other);
                    cursor += 1;
                    data["pre_cursor"] = json!(cursor);
                    Ok(Step::Next(Stage::PindianInit, data))
                }
            }
        }
        "init" => {
            // 酣战：被强制随机的一方不能自选拼点牌。
            if g.answer.is_none() && skills::pindian_random_forced(g, initiator) {
                match random_choice(g, initiator) {
                    Some(card) => {
                        g.push_log(format!(
                            "酣战：{} 只能随机用手牌【{}】拼点",
                            g.seats[initiator].name,
                            g.card_label(&card)
                        ));
                        data["a"] = json!(card);
                        data["step"] = json!("resp");
                        return Ok(Step::Next(Stage::PindianResp, data));
                    }
                    None => return cancel(g, initiator, target),
                }
            }
            match g.answer.take() {
                None => ask_card(g, initiator, Stage::PindianInit, data),
                Some(ans) if ans == "pass" => cancel(g, initiator, target),
                Some(ans) => {
                    let card = resolve_choice(g, initiator, &ans)?;
                    data["a"] = json!(card);
                    data["step"] = json!("resp");
                    Ok(Step::Next(Stage::PindianResp, data))
                }
            }
        }
        "resp" => {
            if g.answer.is_none() && skills::pindian_random_forced(g, target) {
                match random_choice(g, target) {
                    Some(card) => {
                        g.push_log(format!(
                            "酣战：{} 只能随机用手牌【{}】拼点",
                            g.seats[target].name,
                            g.card_label(&card)
                        ));
                        data["b"] = json!(card);
                        settle(g, initiator, target, data)?;
                        return Ok(Step::Done);
                    }
                    None => return cancel(g, initiator, target),
                }
            }
            match g.answer.take() {
                None => ask_card(g, target, Stage::PindianResp, data),
                Some(ans) if ans == "pass" => cancel(g, initiator, target),
                Some(ans) => {
                    let card = resolve_choice(g, target, &ans)?;
                    data["b"] = json!(card);
                    settle(g, initiator, target, data)?;
                    Ok(Step::Done)
                }
            }
        }
        other => Err(format!("拼点：未知步骤 {other}")),
    }
}

/// 选项 → 实体牌 id（`choose:<id>` 手牌 / 技能钩子的附加选项）。
fn resolve_choice(g: &mut Game, actor: usize, ans: &str) -> Result<CardId, String> {
    if let Some(id) = ans.strip_prefix("choose:") {
        if g.hand(actor).iter().any(|c| c == id) {
            return Ok(id.to_string());
        }
        return Err("拼点：只能用自己的手牌拼点".into());
    }
    if let Some(card) = skills::pindian_take(g, actor, ans)? {
        return Ok(card);
    }
    Err(format!("拼点：非法选项 {ans}"))
}

/// 取消拼点（出口）：双方牌留在原处，结果按「平局/未赢」写入结果通道。
fn cancel(g: &mut Game, initiator: usize, target: usize) -> Result<Step, String> {
    g.push_log(format!(
        "拼点：{} 取消拼点（视为未赢）",
        g.seats[initiator].name
    ));
    g.pindian_result = Some(PindianResult {
        initiator,
        target,
        a: String::new(),
        b: String::new(),
        rank_a: 0,
        rank_b: 0,
        winner: None,
    });
    Ok(Step::Done)
}

fn settle(
    g: &mut Game,
    initiator: usize,
    target: usize,
    data: serde_json::Value,
) -> Result<(), String> {
    let a = card_of(&data, "a").ok_or_else(|| "拼点：缺少发起方拼点牌".to_string())?;
    let b = card_of(&data, "b").ok_or_else(|| "拼点：缺少目标拼点牌".to_string())?;
    let rank_a = skills::pindian_rank(g, initiator, &a);
    let rank_b = skills::pindian_rank(g, target, &b);
    // 亮牌：双方拼点牌立即进入公共可见区（选择期不公开）。
    g.reveal(&a, Some(initiator), "pindian");
    g.reveal(&b, Some(target), "pindian");
    let winner = if rank_a > rank_b {
        Some(initiator)
    } else if rank_b > rank_a {
        Some(target)
    } else {
        None
    };
    g.push_log(format!(
        "拼点：{}【{}】({rank_a}) vs {}【{}】({rank_b}) → {}",
        g.seats[initiator].name,
        g.card_label(&a),
        g.seats[target].name,
        g.card_label(&b),
        match winner {
            Some(w) if w == initiator => format!("{} 赢", g.seats[initiator].name),
            Some(w) => format!("{} 赢", g.seats[w].name),
            None => "平局（双方都未赢）".to_string(),
        }
    ));
    // 拼点牌进弃牌堆（技能描述未声明「可以获得拼点牌」）。
    for id in [&a, &b] {
        match g.zone_of(id) {
            // 牌堆顶拼来的牌（天辩）先经处理区，再进弃牌堆。
            Some(super::Zone::Draw) => {
                g.to_processing(id)?;
                g.to_discard(id, "pindian")?;
            }
            Some(_) => g.to_discard(id, "pindian")?,
            None => {}
        }
        g.conceal(id, "pindian");
    }
    g.pindian_result = Some(PindianResult {
        initiator,
        target,
        a: a.clone(),
        b: b.clone(),
        rank_a,
        rank_b,
        winner,
    });
    // 「拼点后」技能（酣战）：给持有者压入询问（读 data 而非结果通道，避免与发起技能抢 take）。
    skills::pindian_after(g, initiator, target, &a, &b, winner);
    Ok(())
}
