//! 牌结算单一流程：使用 → 目标 → 响应 → 无懈 → 结算 → 弃牌堆。
//!
//! 不变量（mechanics `use-vs-play` / `target-list` / `response-cards` / `no-wuxie`）：
//! 1. **使用 ≠ 打出**：「使用」= 主动进入结算；「打出」= 响应提供牌（不进结算）。
//!    闪只能响应、不能主动使用；杀在响应里是「打出」，不计入使用次数。
//! 2. **目标列表**：每次使用记录 `targets: Vec<seat>`，技能只读列表，不按牌名反推目标模式。
//! 3. **响应牌**读 `CardDef.tags`（`responds_as`），不按牌名枚举。
//! 4. **无懈链**：奇偶层判定（奇数 = 锦囊被抵消）；「不能被无懈」是牌属性的独立事实。

use super::cards::{CardDef, ResponseKind, valid_target};
use super::event::Event;
use super::flow;
use super::{ArmorKind, CardId, DamageKind, DamageSpec, Game, SourceKind, Zone};

/// 出牌阶段可主动使用的手牌（闪/桃满血等被排除；响应牌不出现在主动使用里）。
///
/// `defs` = 牌定义表查表（生成物；内核不按牌名字面量判定）。
pub fn legal_uses(g: &Game, seat: usize, defs: &dyn Fn(&str) -> Option<CardDef>) -> Vec<String> {
    let mut out = vec![];
    for (id, zone) in &g.cards {
        if *zone != Zone::Hand(seat) {
            continue;
        }
        let Some(c) = g.card_defs.get(id) else { continue };
        let Some(d) = defs(&c.def) else { continue };
        // 闪不能主动使用（只能响应）；桃满血不能使用。
        if d.tags.shan {
            continue;
        }
        if d.tags.tao && g.seats[seat].hp >= g.seats[seat].max_hp {
            continue;
        }
        out.push(id.clone());
    }
    out
}

/// 使用【桃】→ 回血（唯一出口 `flow::heal`，满血回 0）。
pub fn resolve_tao(g: &mut Game, seat: usize) -> Vec<Event> {
    flow::heal(g, seat, 1, "桃")
}

/// 无懈链：奇数层 = 锦囊被抵消（`layers` = 已打出的无懈张数）。
pub fn wuxie_cancelled(layers: usize) -> bool {
    layers % 2 == 1
}

/// 一张牌能否指定该目标（默认规则；技能覆盖走钩子，M2 接入）。
pub fn can_target(def: &CardDef, self_seat: usize, target: usize, alive: bool) -> bool {
    valid_target(def.target_mode, self_seat, target, alive)
}

/// 牌进弃牌堆（唯一出口，`lose-vs-discard`：弃置是离开手牌/装备区进弃牌堆）。
pub fn to_discard(g: &mut Game, id: &str) {
    g.cards.insert(id.to_string(), Zone::Discard);
}

/// 响应牌判定：读标签，不按牌名。
pub fn can_respond(def: &CardDef, kind: ResponseKind) -> bool {
    def.responds_as(kind)
}

/// 【杀】结算结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaResult {
    /// 目标出【闪】，抵消。
    Blocked,
    /// 命中，造成点数（含【酒】加成）。
    Hit { damage: i32 },
}

/// 使用【酒】：本回合下一张【杀】伤害 +1（出牌阶段；濒死自救走 `flow::rescue`）。
pub fn use_jiu(g: &mut Game) {
    g.sha_bonus = 1;
}

/// 使用【杀】结算：`shan` = 目标是否打出【闪】。
/// 【酒】加成在使用时消费（无论命中与否，「酒」已花费）。
pub fn resolve_sha(g: &mut Game, from: usize, target: usize, shan: bool, armor: ArmorKind) -> ShaResult {
    let bonus = g.sha_bonus;
    g.sha_bonus = 0;
    if shan {
        return ShaResult::Blocked;
    }
    let spec = DamageSpec {
        from: Some(from),
        target,
        amount: 1 + bonus,
        kind: DamageKind::Normal,
        source: SourceKind::Slash,
    };
    flow::damage(g, spec, armor, 0);
    ShaResult::Hit { damage: 1 + bonus }
}

/// 无中生有：摸 2 张。
pub fn resolve_wuzhong(g: &mut Game, seat: usize) -> Vec<CardId> {
    flow::draw(g, seat, 2)
}

/// 过河拆桥：弃目标一张牌（`card` 必须是目标手牌/装备区/判定区的牌）。
pub fn resolve_guohe(g: &mut Game, target: usize, card: &str) -> bool {
    // 校验牌确实在目标的合法区（手牌/装备/判定区）。
    let ok = matches!(
        g.cards.get(card),
        Some(Zone::Hand(t)) | Some(Zone::Equip(t, _)) | Some(Zone::Judge(t)) if *t == target
    );
    if ok {
        g.cards.insert(card.to_string(), Zone::Discard);
    }
    ok
}

/// 延时锦囊进目标判定区（同名不可重复放）。
pub fn place_delayed(g: &mut Game, target: usize, card: &str, def_code: &str) -> bool {
    let stack = g.judge_stacks.entry(target).or_default();
    // 同名延时锦囊不可重复（按牌定义 code）。
    if stack.iter().any(|id| g.card_defs.get(id).map(|c| c.def == def_code).unwrap_or(false)) {
        return false;
    }
    g.cards.insert(card.to_string(), Zone::Judge(target));
    stack.push(card.to_string());
    true
}

/// 判定区 LIFO：后置先判。返回下一个要判定的牌（栈顶），并移出判定区。
pub fn judge_zone_lifo(g: &mut Game, seat: usize) -> Option<String> {
    let id = g.judge_stacks.get_mut(&seat)?.pop()?;
    // 判定后进弃牌堆（闪电类技能可移走，M2 技能接入）
    g.cards.insert(id.clone(), Zone::Discard);
    Some(id)
}

/// 锦囊结算（无懈包装）：`layers` = 无懈张数，奇数 = 锦囊被抵消。
pub fn trick_cancelled_by_wuxie(layers: usize) -> bool {
    wuxie_cancelled(layers)
}

/// 流程帧 handler（`resolve::advance` 的 `Flow` 分支）：牌结算同步版。
/// 完整交互（选目标/响应窗口 pending）在 M5 接入；M4.3b 先跑通杀/桃的同步结算。
pub fn step_flow(g: &mut Game, kind: super::resolve::FlowKind, data: &serde_json::Value, actor: usize) -> Result<super::resolve::Step, String> {
    match kind {
        super::resolve::FlowKind::Card => {
            let card = data.get("card").and_then(|c| c.as_str()).ok_or("流程帧缺 card")?;
            let target = data.get("target").and_then(|t| t.as_u64()).ok_or("流程帧缺 target")? as usize;
            let code = g.card_defs.get(card).map(|c| c.def.clone()).ok_or("牌无定义")?;
            let def = super::cards::def(&code).ok_or("未知牌定义")?;
            // 使用计数（掣政②：含装备牌与响应打出，B1 Q17 裁定 B）。
            g.bump(&format!("turn.{actor}.cards_used"));
            // 使用事件（恃才/立军订阅）——快照带使用时刻的 turn/phase。
            let mut used_ev = super::event::Event::card(
                super::event::EventKind::CardUsed,
                Some(actor),
                card.to_string(),
                Some("使用".to_string()),
            );
            used_ev.by = Some(actor);
            used_ev.turn = g.turn;
            used_ev.phase = g.phase;
            g.events.push(used_ev);
            // 牌对目标无效（掣政①）：不进结算、不指定目标，但已计入使用数。
            if super::skills::card_ineffective(g, actor, target) {
                to_discard(g, card);
                return Ok(super::resolve::Step::Done);
            }
            // 指定目标事件（奇制/拒战/荐降订阅）：目标按统一事件，不按牌名枚举。
            let mut tgt_ev = super::event::Event::card(
                super::event::EventKind::TargetChosen,
                Some(target),
                card.to_string(),
                Some("目标".to_string()),
            );
            tgt_ev.by = Some(actor);
            tgt_ev.turn = g.turn;
            tgt_ev.phase = g.phase;
            g.events.push(tgt_ev);
            if def.tags.sha {
                let armor = super::gear::armor_of(g, target);
                let _ = resolve_sha(g, actor, target, false, armor);
            } else if def.tags.tao {
                let _ = resolve_tao(g, actor);
            }
            to_discard(g, card);
            Ok(super::resolve::Step::Done)
        }
        super::resolve::FlowKind::Dying => step_dying(g, data),
        _ => Ok(super::resolve::Step::Done),
    }
}

/// 濒死救援流程（通用，所有“致濒死”渠道共用）。
///
/// 询问顺序：濒死者本人先，然后从其下家顺时针存活角色；逐个给「桃 / 不救」。
/// 无人出桃 → `turn::settle_death`（弃牌 → 奖惩 → 胜负）。
/// M5.1 仅支持【桃】（【酒】自救与技能救援在 M5.2 扩展）。
fn step_dying(g: &mut Game, data: &serde_json::Value) -> Result<super::resolve::Step, String> {
    use super::resolve::{take_answer, Stage, Step};
    let target = data.get("target").and_then(|v| v.as_u64()).ok_or("濒死帧缺 target")? as usize;
    let killer = data.get("by").and_then(|v| v.as_u64()).map(|v| v as usize);
    let idx = data.get("idx").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    // 已脱险（被桃/技能救回）：流程结束。
    if !super::flow::is_dying(&g.seats[target]) {
        return Ok(Step::Done);
    }
    let n = g.seats.len();
    let order: Vec<usize> = (0..n)
        .map(|k| (target + k) % n)
        // 与 v2 `dying_order` 同口径：自己 → 自己下家；**hp>0** 的存活角色才询问。
        .filter(|&s| g.seats[s].alive && (g.seats[s].hp > 0 || s == target))
        .collect();
    if idx >= order.len() {
        let evs = super::turn::settle_death(g, target, killer);
        g.events.extend(evs);
        return Ok(Step::Done);
    }
    let asker = order[idx];
    if let Some(ans) = take_answer(g, "dying") {
        if let Some(card) = ans.strip_prefix("card:") {
            let in_hand = matches!(g.cards.get(card), Some(super::Zone::Hand(s)) if *s == asker);
            let is_tao = g
                .card_defs
                .get(card)
                .and_then(|c| super::cards::def(&c.def))
                .map(|d| d.tags.tao)
                .unwrap_or(false);
            if !in_hand || !is_tao {
                return Err("濒死救援：只能打出【桃】".into());
            }
            super::flow::discard(g, asker, card, "濒死救援");
            super::flow::rescue(g, target, 1, "桃");
            if !super::flow::is_dying(&g.seats[target]) {
                return Ok(Step::Done);
            }
        } else if ans != "pass" {
            return Err(format!("濒死救援：未知选项 {ans}"));
        }
        return Ok(Step::Next(
            Stage::DyingAsk,
            serde_json::json!({ "target": target, "by": killer, "idx": idx + 1 }),
        ));
    }
    let mut b = super::pending::PendingBuilder::new(asker, "dying_ask").skill("dying");
    for id in g.hand(asker) {
        let is_tao = g
            .card_defs
            .get(&id)
            .and_then(|c| super::cards::def(&c.def))
            .map(|d| d.tags.tao)
            .unwrap_or(false);
        if is_tao {
            b = b.option(format!("card:{id}"), "打出【桃】");
        }
    }
    let p = b
        .escape("不出桃")
        .data(serde_json::json!({ "target": target, "idx": idx }))
        .build();
    Ok(Step::Need(p))
}
