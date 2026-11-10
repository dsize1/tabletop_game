//! 规则流程（内核级、与具体技能无关）：伤害 / 濒死 / 死亡清理 / 判定 / 无懈窗口。
//!
//! 与技能的接口只有两类：
//! - **钩子**（`skills::dying_hooks` / `skills::on_flip_to_face_up` / `skills::wuxie_options`）；
//! - **事件**（Death / DamageAfter / JudgeEffective …，技能订阅后自行询问）。
//!
//! 本文件**不得**出现任何武将技能名（由 `tests/b0.rs::gate_kernel_has_no_skill_branch` 断言）。

use serde_json::json;

use super::deferred::{Deferred, DeferredKind, Payload};
use super::event::{Event, EventKind};
use super::resolve::{Ask, Frame, Stage, Step};
use super::{CardId, Game, Zone};
use crate::presentation::{POutcome, Presentation};
use crate::skills;

// ---------------------------------------------------------------------------
// 伤害 / 濒死 / 死亡
// ---------------------------------------------------------------------------

/// 统一伤害出口：扣血 → 展示 → 濒死。
pub fn damage(g: &mut Game, source: Option<usize>, victim: usize, amount: i32) {
    damage_with(g, source, victim, amount, None);
}

/// 带伤害牌属性的伤害入口（属性/牌名只用于展示与「受到伤害」判定，见 `damage-vs-hp-loss`）。
///
/// 「造成伤害时」的拦截（如制蛮）在**扣血之前**询问：技能压入 Ask 帧并返回 true，
/// 本次伤害由技能决定是否补结算（`damage_now` 绕过所有前置钩子，避免递归）。
pub fn damage_with(
    g: &mut Game,
    source: Option<usize>,
    victim: usize,
    amount: i32,
    card: Option<&CardId>,
) {
    let dtype = card
        .map(|c| g.def_of(c).damage_kind)
        .unwrap_or(crate::cards::DamageKind::Normal);
    damage_typed(g, source, victim, amount, dtype, card, false);
}

/// 带**伤害属性**与**连环传导起点**标记的伤害入口（链祸等按属性判定的技能用）。
pub fn damage_typed(
    g: &mut Game,
    source: Option<usize>,
    victim: usize,
    amount: i32,
    dtype: crate::cards::DamageKind,
    card: Option<&CardId>,
    chain_origin: bool,
) {
    let def = card.map(|c| g.def_of(c));
    damage_impl(g, source, victim, amount, dtype, def.as_ref(), card, chain_origin);
}

/// 带**牌定义**的伤害入口（虚拟牌没有实体牌实例，但仍有 `cards::def` 属性）。
///
/// 防具判定（仁王盾/藤甲）与数值链（藤甲 +1 / 白银狮子最后）都读这里的 `def`，
/// 因此「视为使用【火杀】」也能正确吃到藤甲 +1。
pub fn damage_def(
    g: &mut Game,
    source: Option<usize>,
    victim: usize,
    amount: i32,
    def: &crate::cards::CardDef,
    card: Option<&CardId>,
    chain_origin: bool,
) {
    damage_impl(
        g,
        source,
        victim,
        amount,
        def.damage_kind,
        Some(def),
        card,
        chain_origin,
    );
}

fn damage_impl(
    g: &mut Game,
    source: Option<usize>,
    victim: usize,
    amount: i32,
    dtype: crate::cards::DamageKind,
    def: Option<&crate::cards::CardDef>,
    card: Option<&CardId>,
    chain_origin: bool,
) {
    if g.seats[victim].hp <= 0 {
        return;
    }
    // 绝情（界张春华，锁定）：你即将造成的伤害视为失去体力（整次数值一次失去，自伤同样转化）。
    if crate::skills::damage_to_hp_loss(g, source, victim) && amount > 0 {
        crate::skills::on_damage_to_hp_loss(g, source, victim, amount, card);
        lose_hp_by(g, victim, amount, "jueqing", source);
        return;
    }
    if armor_prevents(g, source, victim, def, card, dtype) {
        g.push_log(format!(
            "{} 的防具使【{}】无效",
            g.seats[victim].name,
            card.map(|c| g.name_of(c))
                .unwrap_or_else(|| "虚拟牌".to_string())
        ));
        return;
    }
    // 「受到伤害时」受害者侧拦截（天香）：由技能接管本次伤害。
    if crate::skills::damage_before_victim(g, source, victim, amount, card) {
        return;
    }
    if crate::skills::damage_prevent(g, victim) {
        return;
    }
    if let Some(src) = source {
        if src != victim
            && crate::skills::damage_before_typed(g, src, victim, amount, card, dtype, chain_origin)
        {
            return;
        }
    }
    if source.is_none() && crate::skills::damage_before_sourceless(g, victim, amount, card) {
        return;
    }
    damage_now_impl(g, source, victim, amount, card, dtype, def, chain_origin, false);
}

/// 直接结算伤害（**不**再走「造成伤害时」拦截；技能自行补结算时用）。
pub fn damage_now(
    g: &mut Game,
    source: Option<usize>,
    victim: usize,
    amount: i32,
    card: Option<&CardId>,
) {
    let dtype = card
        .map(|c| g.def_of(c).damage_kind)
        .unwrap_or(crate::cards::DamageKind::Normal);
    let def = card.map(|c| g.def_of(c));
    damage_now_impl(g, source, victim, amount, card, dtype, def.as_ref(), false, false);
}

/// 直接结算伤害且**不受防具影响**（驱虎：伤害不被防具减免/防止）。
pub fn damage_now_no_armor(g: &mut Game, source: Option<usize>, victim: usize, amount: i32) {
    damage_now_impl(
        g,
        source,
        victim,
        amount,
        None,
        crate::cards::DamageKind::Normal,
        None,
        false,
        true,
    );
}

pub fn damage_now_typed(
    g: &mut Game,
    source: Option<usize>,
    victim: usize,
    amount: i32,
    card: Option<&CardId>,
    dtype: crate::cards::DamageKind,
    chain_origin: bool,
) {
    let def = card.map(|c| g.def_of(c));
    damage_now_impl(
        g,
        source,
        victim,
        amount,
        card,
        dtype,
        def.as_ref(),
        chain_origin,
        false,
    );
}

fn damage_now_impl(
    g: &mut Game,
    source: Option<usize>,
    victim: usize,
    amount: i32,
    card: Option<&CardId>,
    dtype: crate::cards::DamageKind,
    def: Option<&crate::cards::CardDef>,
    chain_origin: bool,
    ignore_armor: bool,
) {
    if g.seats[victim].hp <= 0 {
        return;
    }
    if !ignore_armor && armor_prevents(g, source, victim, def, card, dtype) {
        return;
    }
    // 「造成伤害时」的数值加成（暗箭/链祸等）：先加成，再做「受到的伤害改为 1」类终局修正。
    let mut amount = amount
        + source
            .map(|s| crate::skills::damage_bonus_typed(g, s, victim, card, dtype, chain_origin))
            .unwrap_or_else(|| {
                crate::skills::damage_bonus_sourceless(g, victim, dtype, chain_origin)
            });
    // 装备数值链（顺序固定）：技能加成 → 藤甲（火伤 +1）→ 白银狮子（最后，>1 改为 1）。
    if !ignore_armor
        && !armor_ignored(g, source, victim, def)
        && g.armor(victim) == crate::cards::ArmorKind::Tengjia
        && dtype == crate::cards::DamageKind::Fire
    {
        amount += 1;
    }
    if !ignore_armor
        && !armor_ignored(g, source, victim, def)
        && g.armor(victim) == crate::cards::ArmorKind::Baiyin
        && amount > 1
    {
        amount = 1;
    }
    // 狞恶（界典韦）：「每回合第二次受到伤害」按受害者计数。
    g.bump(&format!("turn.damage_taken:{victim}"));
    // 「此牌造成过伤害」（疠火「此牌结算后，若其造成伤害」；结算完成时读取）。
    if let Some(card) = card {
        g.bump(&format!("turn.damage_card:{card}"));
    }
    if let Some(s) = source {
        g.bump(&format!("turn.damage_by:{s}"));
        // 本局累计造成的伤害数（伏枥：X > 你造成的伤害数才翻面）。
        g.bump(&format!("total.damage_dealt:{s}"));
        // 咆哮：本次【杀】已消费「下一次伤害 +1」。
        if card.map(|c| g.def_of(c).tags.sha).unwrap_or(false) {
            g.counters.insert("turn.paoxiao_bonus".to_string(), 0);
        }
    }
    g.seats[victim].hp -= amount;
    // 「本阶段是否有人受到过伤害」（佐定等）。
    g.bump("phase.damage");
    // 「本阶段造成的伤害值」（急攻等）：只统计有来源的伤害。
    if source.is_some() {
        g.bump("phase.damage_dealt");
    }
    g.presentation.push(
        Presentation::new(
            "damage",
            POutcome::Damage {
                amount: amount.max(0),
            },
        )
        .target(victim)
        .fx("damage"),
    );
    let src = source.map(|s| g.seats[s].name.clone()).unwrap_or_default();
    g.push_log(format!(
        "{src} 对 {} 造成 {amount} 点伤害（{}）",
        g.seats[victim].name, g.seats[victim].hp
    ));
    // 「受到伤害后」事件（放逐/反馈/遗计…由此订阅）；`amount` = 本次伤害点数（权计按点触发）。
    let mut ev = Event::with_amount(
        EventKind::DamageAfter,
        Some(victim),
        source.map(|s| s.to_string()).as_deref(),
        amount,
    );
    ev.card = card.cloned();
    g.events.push(ev);
    // 属性伤害 + 连环状态 → 解除连环并传导给其他连环角色（链祸等技能按 `chain_origin` 判定）。
    if dtype != crate::cards::DamageKind::Normal && g.seats[victim].chained && !chain_origin {
        chain_leave(g, victim);
        let others: Vec<usize> = (0..g.seats.len())
            .filter(|i| *i != victim && g.seats[*i].hp > 0 && g.seats[*i].chained)
            .collect();
        for other in others {
            chain_leave(g, other);
            damage_now_impl(g, source, other, amount, card, dtype, def, true, false);
        }
    }
    if g.seats[victim].hp <= 0 {
        start_dying(g, victim, source);
    }
}

/// 青釭剑：使用【杀】时无视目标防具（只对【杀】生效）。
fn armor_ignored(
    g: &Game,
    source: Option<usize>,
    victim: usize,
    def: Option<&crate::cards::CardDef>,
) -> bool {
    let Some(def) = def else { return false };
    if !def.tags.sha {
        return false;
    }
    // 陷阵（界高顺，赢）：无视被拼点目标的防具。
    if source.is_some_and(|s| crate::skills::armor_ignored_by_skill(g, s, victim)) {
        return true;
    }
    source
        .map(|s| g.weapon(s) == crate::cards::WeaponKind::Qinggang)
        .unwrap_or(false)
}

/// 防具使本次伤害无效（仁王盾 / 藤甲）。返回 true = 伤害被防止。
fn armor_prevents(
    g: &Game,
    source: Option<usize>,
    victim: usize,
    def: Option<&crate::cards::CardDef>,
    card: Option<&CardId>,
    dtype: crate::cards::DamageKind,
) -> bool {
    let Some(def) = def else { return false };
    if g.seats[victim].hp <= 0 {
        return false;
    }
    let armor = g.armor(victim);
    if armor == crate::cards::ArmorKind::None || armor_ignored(g, source, victim, Some(def)) {
        return false;
    }
    match armor {
        // 仁王盾：黑色【杀】对你无效（虚拟牌无花色 → 不构成「黑色」）。
        crate::cards::ArmorKind::Renwang => {
            def.tags.sha && card.map(|c| !g.is_red(c)).unwrap_or(false)
        }
        // 藤甲：普通【杀】/【南蛮入侵】/【万箭齐发】对你无效（火伤在数值链里 +1）。
        crate::cards::ArmorKind::Tengjia => {
            (def.tags.sha && dtype == crate::cards::DamageKind::Normal)
                || (def.damage_trick
                    && def.tags.target_mode == Some(crate::cards::TargetMode::AllAlive))
        }
        _ => false,
    }
}

/// 回复体力（唯一入口；不产生伤害事件）。
pub fn heal(g: &mut Game, seat: usize, n: i32, cause: &str) {
    if n <= 0 || g.seats[seat].hp <= 0 {
        return;
    }
    // 救援（主公技）：其他吴势力角色于其回合内回复体力时，可改为令主公回复。
    if crate::skills::start_jiuyuan(g, seat, n, cause) {
        return;
    }
    let before = g.seats[seat].hp;
    g.seats[seat].hp = (before + n).min(g.seats[seat].max_hp);
    let healed = g.seats[seat].hp - before;
    if healed <= 0 {
        return;
    }
    g.events.push(Event::with_amount(
        EventKind::Healed,
        Some(seat),
        Some(cause),
        healed,
    ));
    g.presentation.push(
        Presentation::new("heal", POutcome::Heal { amount: healed })
            .target(seat)
            .fx("heal"),
    );
    g.push_log(format!(
        "{} 回复 {healed} 点体力（体力 {}，原因：{cause}）",
        g.seats[seat].name, g.seats[seat].hp
    ));
}

/// 失去（流失）体力：**不产生伤害事件**（mechanics `damage-vs-hp-loss`，2026-11-06 用户裁定）。
/// 「受到伤害后」类技能（反馈/权计/刚烈/智愚/御策…）**不得**由此触发。
pub fn lose_hp(g: &mut Game, seat: usize, n: i32, cause: &str) {
    lose_hp_by(g, seat, n, cause, None);
}

/// 带「伤害来源」的失去体力（绝情：死亡归属仍记来源）。
pub fn lose_hp_by(g: &mut Game, seat: usize, n: i32, cause: &str, source: Option<usize>) {
    if g.seats[seat].hp <= 0 || n <= 0 {
        return;
    }
    g.seats[seat].hp -= n;
    g.events.push(Event::with_amount(
        EventKind::HpLost,
        Some(seat),
        Some(cause),
        n,
    ));
    g.push_log(format!(
        "{} 失去 {n} 点体力（体力 {}，原因：{cause}）",
        g.seats[seat].name, g.seats[seat].hp
    ));
    if g.seats[seat].hp <= 0 {
        start_dying(g, seat, source);
    }
}

/// 濒死救援：把体力置为 `hp`（仁心/伏枥；`hp` 为正数，且不超过上限）。
pub fn revive_to(g: &mut Game, seat: usize, hp: i32, cause: &str) {
    let target = hp.clamp(1, g.seats[seat].max_hp.max(1));
    if g.seats[seat].hp >= target {
        return;
    }
    g.seats[seat].hp = target;
    g.events.push(Event::with_amount(
        EventKind::Healed,
        Some(seat),
        Some(cause),
        target,
    ));
    g.presentation.push(
        Presentation::new("heal", POutcome::Heal { amount: target })
            .target(seat)
            .fx("dying_heal"),
    );
    g.push_log(format!(
        "{} 回复至 {target} 点体力（原因：{cause}）",
        g.seats[seat].name
    ));
}

/// 濒死救援：回复 `n` 点体力（补益；濒死时体力 ≤0 也能生效）。
pub fn revive_add(g: &mut Game, seat: usize, n: i32, cause: &str) {
    if n <= 0 {
        return;
    }
    let before = g.seats[seat].hp;
    g.seats[seat].hp = (before + n).min(g.seats[seat].max_hp);
    let healed = g.seats[seat].hp - before;
    if healed <= 0 {
        return;
    }
    g.events.push(Event::with_amount(
        EventKind::Healed,
        Some(seat),
        Some(cause),
        healed,
    ));
    g.push_log(format!(
        "{} 回复 {healed} 点体力（体力 {}，原因：{cause}）",
        g.seats[seat].name, g.seats[seat].hp
    ));
}

/// 进入连环状态（铁索）：**所有**进入者都发 `ChainedEnter`（用户 2026-11-06 裁定）。
pub fn chain_enter(g: &mut Game, seat: usize) {
    if g.seats[seat].chained {
        return;
    }
    g.seats[seat].chained = true;
    g.events.push(Event::simple(
        EventKind::ChainedEnter,
        Some(seat),
        Some("chain"),
    ));
    g.push_log(format!("{} 进入连环状态", g.seats[seat].name));
}

/// 离开连环状态。
pub fn chain_leave(g: &mut Game, seat: usize) {
    if !g.seats[seat].chained {
        return;
    }
    g.seats[seat].chained = false;
    g.push_log(format!("{} 离开连环状态", g.seats[seat].name));
}

/// 体力上限变化（减上限；自立觉醒 / 恃勇 等共用同一事件与日志口径）。
pub fn lose_max_hp(g: &mut Game, seat: usize, n: i32, cause: &str) {
    if n <= 0 {
        return;
    }
    let max_hp = (g.seats[seat].max_hp - n).max(1);
    g.seats[seat].max_hp = max_hp;
    if g.seats[seat].hp > max_hp {
        g.seats[seat].hp = max_hp;
    }
    g.events.push(Event::with_amount(
        EventKind::MaxHpChanged,
        Some(seat),
        Some(cause),
        -n,
    ));
    g.push_log(format!(
        "{} 减少 {n} 点体力上限（上限 {}，体力 {}，原因：{cause}）",
        g.seats[seat].name, g.seats[seat].max_hp, g.seats[seat].hp
    ));
}

pub fn start_dying(g: &mut Game, victim: usize, source: Option<usize>) {
    // 完杀（界贾诩）：濒死锁定状态（其他角色非锁定技失效）。
    crate::skills::entering_dying(g, victim);
    g.stack.push(Frame::Flow(super::resolve::FlowFrame {
        kind: super::resolve::FlowKind::Dying,
        skill: "__dying".to_string(),
        actor: victim,
        stage: Stage::DyingAsk,
        data: json!({ "victim": victim, "source": source, "cursor": 0, "tried": false, "round": 0 }),
    }));
    // 「进入濒死时」技能（补益/仁心/伏枥）：压在濒死帧之上，先于求桃询问。
    crate::skills::on_dying_enter(g, victim, source);
}

/// 濒死询问顺序。
/// 默认：自己 → 自己左手侧依次；**拥有可用兴衰时**：从自己左手方开始先求桃，自己最后（用户 2026-11-06 裁定）。
fn dying_order(g: &Game, victim: usize) -> Vec<usize> {
    let xingshuai = crate::skills::xingshuai_available(g, victim);
    let mut v = if xingshuai {
        let mut order: Vec<usize> = g.seats_from_left_of(victim);
        order.push(victim);
        order
    } else {
        let mut order = vec![victim];
        order.extend(g.seats_from_left_of(victim));
        order
    };
    v.retain(|i| g.seats[*i].hp > 0 || *i == victim);
    v
}

pub fn step_dying(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let victim = a.data["victim"].as_u64().unwrap_or(0) as usize;
    if g.seats[victim].hp > 0 {
        // 完杀：濒死结算结束（救起）。
        crate::skills::leaving_dying(g);
        // 兴衰：脱离濒死**之后**，支援者各受到 1 点伤害（用户 2026-11-06 裁定）。
        crate::skills::settle_after_dying(g, victim);
        // 奇谋：救起后再摸牌。
        crate::skills::on_dying_left(g, victim);
        return Ok(Step::Done);
    }
    let tried = a.data["tried"].as_bool().unwrap_or(false);
    let cursor = a.data["cursor"].as_u64().unwrap_or(0) as usize;

    // 锁定技濒死钩子（如不屈）：每次濒死尝试一次。
    if !tried && skills::dying_hooks(g, victim) {
        let mut next = a.data.clone();
        next["tried"] = json!(true);
        return Ok(Step::Next(Stage::DyingAsk, next));
    }

    let order = dying_order(g, victim);
    if cursor >= order.len() {
        let source = a.data["source"].as_u64().map(|n| n as usize);
        die(g, victim, source);
        return Ok(Step::Done);
    }
    let asker = order[cursor];
    // 延后技能（蛊惑）结束后的结果消费。
    if let Some(res) = g.guhuo_result.take() {
        let mut next = a.data.clone();
        if let Some(_card) = res {
            g.seats[victim].hp += 1;
            g.presentation.push(
                Presentation::new("dying", POutcome::Heal { amount: 1 })
                    .actor(asker)
                    .target(victim)
                    .fx("dying_heal"),
            );
            g.push_log(format!(
                "{} 蛊惑救援 {}（体力 {}）",
                g.seats[asker].name, g.seats[victim].name, g.seats[victim].hp
            ));
            // 回到顶部：统一处理濒死锁定清理 + 兴衰/奇谋等离场结算。
            return Ok(Step::Next(Stage::DyingAsk, a.data.clone()));
        }
        next["cursor"] = json!(cursor + 1);
        return Ok(Step::Next(Stage::DyingAsk, next));
    }
    let mut b = skills::builder(asker, "dying").skill("dying");
    let mut has = false;
    // 求到自己时，若可发动兴衰，提供「发动兴衰」选项（自桃/自酒/兴衰三选一）。
    if asker == victim && skills::xingshuai_available(g, victim) {
        has = true;
        b = b.option("xingshuai", "发动兴衰（其他魏势力角色施救）");
    }
    let peach_blocked = crate::skills::dying_peach_blocked(g, asker, victim);
    // 禁酒（界高顺）：其回合内其他角色（含自己）不能使用【酒】。
    let jiu_blocked = crate::skills::jiu_blocked(g, asker);
    for id in g.hand(asker) {
        let tags = g.def_of(&id).tags;
        let usable = (tags.tao && !peach_blocked) || (tags.jiu && asker == victim && !jiu_blocked);
        if usable {
            has = true;
            b = b.option(
                format!("respond:{id}"),
                format!("用【{}】救援（{}）", g.card_label(&id), g.seats[victim].name),
            );
        }
    }
    // 界酒池（界董卓）：黑桃手牌当【酒】自救（仅濒死者本人；禁酒生效时不提供）。
    if asker == victim && !jiu_blocked {
        for id in crate::skills::dying_self_jiu(g, asker) {
            if g.def_of(&id).tags.jiu || g.def_of(&id).tags.tao {
                continue;
            }
            has = true;
            b = b.option(
                format!("jiuchi:{id}"),
                format!("酒池：将【{}】当【酒】自救", g.card_label(&id)),
            );
        }
    }
    // 被动转化【桃】（急救：回合外红色牌当【桃】；候选与 execute 共用 `dying_conversions`）。
    for conv in if peach_blocked {
        Vec::new()
    } else {
        crate::skills::dying_conversions(g, asker)
    } {
        has = true;
        b = b.option(
            format!("vrespond:{}:{}", conv.skill, conv.from),
            format!(
                "用【{}】当【桃】救援（{}）",
                g.card_label(&conv.from),
                g.seats[victim].name
            ),
        );
    }
    // 蛊惑（于吉）：扣置一张手牌当【桃】（延后结算：质疑链后再回传结果）。
    // 完杀：使用【桃】受限时蛊惑当【桃】同样不可用。
    for (id, label) in if peach_blocked {
        Vec::new()
    } else {
        crate::skills::dying_extra_options(g, asker)
    } {
        has = true;
        b = b.option(id, label);
    }
    if !has {
        let mut next = a.data.clone();
        next["cursor"] = json!(cursor + 1);
        return Ok(Step::Next(Stage::DyingAsk, next));
    }
    match g.answer.take().as_deref() {
        None => Ok(Step::Need(b.escape("放弃救援").build())),
        Some("pass") => {
            let mut next = a.data.clone();
            next["cursor"] = json!(cursor + 1);
            Ok(Step::Next(Stage::DyingAsk, next))
        }
        Some("xingshuai") => {
            crate::skills::start_xingshuai(g, victim);
            // 兴衰结算后重新检查体力（仍 <=0 则继续求桃）。
            Ok(Step::Next(Stage::DyingAsk, a.data.clone()))
        }
        Some(act) => {
            // 延后型技能选项（蛊惑）：启动质疑链，结束后由上面的结果通道重入。
            if crate::skills::dying_extra_use(g, asker, victim, act)? {
                return Ok(Step::Next(Stage::DyingAsk, a.data.clone()));
            }
            // 转化牌救援（急救）：承载牌进弃牌堆，体力 +1（与【桃】同一结算点）。
            if let Some(rest) = act.strip_prefix("vrespond:") {
                let mut parts = rest.splitn(2, ':');
                let skill = parts.next().unwrap_or_default().to_string();
                let from = parts.next().unwrap_or_default().to_string();
                let ok = crate::skills::dying_conversions(g, asker)
                    .into_iter()
                    .any(|c| c.skill == skill && c.from == from);
                if !ok {
                    return Err("濒死：非法的转化【桃】".into());
                }
                g.to_discard(&from, "dying_heal")?;
                g.seats[victim].hp += 1;
                g.presentation.push(
                    Presentation::new("dying", POutcome::Heal { amount: 1 })
                        .actor(asker)
                        .target(victim)
                        .cards(vec![from])
                        .fx("dying_heal"),
                );
                g.push_log(format!(
                    "{} 急救救援 {}（体力 {}）",
                    g.seats[asker].name, g.seats[victim].name, g.seats[victim].hp
                ));
                return Ok(Step::Next(Stage::DyingAsk, a.data.clone()));
            }
            if let Some(card) = act.strip_prefix("jiuchi:") {
                let card = card.to_string();
                let ok = crate::skills::dying_self_jiu(g, asker).contains(&card);
                if !ok {
                    return Err("濒死：非法的酒池转化".into());
                }
                g.to_discard(&card, "dying_jiu")?;
                g.seats[victim].hp += 1;
                g.presentation.push(
                    Presentation::new("dying", POutcome::Heal { amount: 1 })
                        .actor(asker)
                        .target(victim)
                        .cards(vec![card.clone()])
                        .fx("dying_heal"),
                );
                g.push_log(format!(
                    "{} 以酒池自救 {}（体力 {}）",
                    g.seats[asker].name, g.seats[victim].name, g.seats[victim].hp
                ));
                return Ok(Step::Next(Stage::DyingAsk, a.data.clone()));
            }
            let id = act
                .strip_prefix("respond:")
                .ok_or_else(|| format!("濒死：未知选项 {act}"))?
                .to_string();
            g.to_discard(&id, "dying_heal")?;
            g.seats[victim].hp += 1;
            g.presentation.push(
                Presentation::new("dying", POutcome::Heal { amount: 1 })
                    .actor(asker)
                    .target(victim)
                    .cards(vec![id])
                    .fx("dying_heal"),
            );
            g.push_log(format!(
                "{} 救援 {}（体力 {}）",
                g.seats[asker].name, g.seats[victim].name, g.seats[victim].hp
            ));
            // 回到顶部：统一处理濒死锁定清理 + 兴衰/奇谋等离场结算。
            Ok(Step::Next(Stage::DyingAsk, a.data.clone()))
        }
    }
}

/// 死亡：发事件（行殇等订阅）+ 排一个「死亡清理」（在技能触发之后执行）。
pub fn die(g: &mut Game, victim: usize, source: Option<usize>) {
    // 完杀：濒死结算结束（死亡）。
    crate::skills::leaving_dying(g);
    // 死亡钩子（断肠 / 节命「死亡时」）：在死亡清理（延迟项）之前询问。
    crate::skills::on_death(g, victim, source);
    let name = g.seats[victim].name.clone();
    g.push_log(format!("{name} 死亡"));
    g.events
        .push(Event::simple(EventKind::Death, Some(victim), None));
    g.deferred.push(Deferred {
        kind: DeferredKind::DeathCleanup,
        payload: Payload::Ask {
            skill: "__death_cleanup".to_string(),
            stage: Stage::Choose,
            actor: victim,
            data: json!({ "victim": victim }),
        },
    });
}

/// 死亡清理：剩余牌进弃牌堆（行殇等已在此之前拿走）。
pub fn cleanup_dead(g: &mut Game, victim: usize) -> Result<(), String> {
    let ids: Vec<CardId> = g
        .ids_in(|z| {
            matches!(
                z,
                Zone::Hand(s) | Zone::Equip(s, _) | Zone::Judge(s) | Zone::Mark(s, _) if *s == victim
            )
        })
        .into_iter()
        .collect();
    for id in ids {
        g.to_discard(&id, "death_cleanup")?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 无懈窗口（解围：装备区牌当【无懈可击】）
// ---------------------------------------------------------------------------

pub fn start_wuxie(g: &mut Game, source: usize, card: CardId) {
    g.stack.push(Frame::Flow(super::resolve::FlowFrame {
        kind: super::resolve::FlowKind::Wuxie,
        skill: "__wuxie".to_string(),
        actor: source,
        stage: Stage::WuxieAsk,
        data: json!({ "source": source, "card": card, "cursor": 0, "count": 0 }),
    }));
}

pub fn step_wuxie(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let source = a.data["source"].as_u64().unwrap_or(0) as usize;
    let card = a.data["card"].as_str().unwrap_or("").to_string();
    let mut cursor = a.data["cursor"].as_u64().unwrap_or(0) as usize;
    let mut count = a.data["count"].as_u64().unwrap_or(0) as usize;
    let order: Vec<usize> = g
        .seats_from_left_of(source)
        .into_iter()
        .filter(|i| g.seats[*i].hp > 0)
        .collect();
    if cursor >= order.len() {
        let negated = count % 2 == 1;
        g.wuxie_result = Some(negated);
        let outcome = if negated {
            POutcome::TrickNegated
        } else {
            POutcome::TrickEffective
        };
        g.presentation.push(
            Presentation::new("wuxie", outcome.clone())
                .cards(vec![card.clone()])
                .fx("wuxie_result"),
        );
        g.push_log(format!(
            "无懈结算：共 {count} 张 → {}",
            if count % 2 == 1 {
                "锦囊被抵消"
            } else {
                "锦囊照常生效"
            }
        ));
        return Ok(Step::Done);
    }
    let asker = order[cursor];
    // 延后技能（蛊惑）结束后的结果消费：真 → 计一张并重开询问；假 → 该角色不响应。
    if let Some(res) = g.guhuo_result.take() {
        let mut next = a.data.clone();
        if res.is_some() {
            count += 1;
            cursor = 0;
        } else {
            cursor += 1;
        }
        next["cursor"] = json!(cursor);
        next["count"] = json!(count);
        return Ok(Step::Next(Stage::WuxieAsk, next));
    }
    let mut b = skills::builder(asker, "wuxie").skill("wuxie");
    let mut has = false;
    for id in g.hand(asker) {
        if g.def_of(&id).tags.wuxie {
            has = true;
            b = b.option(
                format!("respond:{id}"),
                format!("打出【{}】", g.card_label(&id)),
            );
        }
    }
    for (id, label) in skills::wuxie_options(g, asker) {
        has = true;
        b = b.option(id, label);
    }
    // 转化【无懈可击】（看破）：黑色牌（手牌/装备区）。
    for conv in skills::conversions(g, asker) {
        if crate::cards::def(conv.to).tags.wuxie && conv.playable {
            has = true;
            b = b.option(
                format!("vrespond:{}:{}", conv.skill, conv.from),
                format!("【{}】当【无懈可击】打出", g.card_label(&conv.from)),
            );
        }
    }
    if !has {
        cursor += 1;
        let mut next = a.data.clone();
        next["cursor"] = json!(cursor);
        return Ok(Step::Next(Stage::WuxieAsk, next));
    }
    match g.answer.take().as_deref() {
        None => Ok(Step::Need(b.escape("不使用无懈").build())),
        Some("pass") => {
            let mut next = a.data.clone();
            next["cursor"] = json!(cursor + 1);
            Ok(Step::Next(Stage::WuxieAsk, next))
        }
        Some(act) => {
            if let Some(rest) = act.strip_prefix("vrespond:") {
                // 转化【无懈可击】（看破）。
                let mut parts = rest.splitn(2, ':');
                let skill = parts.next().unwrap_or_default().to_string();
                let carrier = parts.next().unwrap_or_default().to_string();
                let ok = skills::conversions(g, asker)
                    .into_iter()
                    .any(|c| {
                        c.skill == skill
                            && c.from == carrier
                            && crate::cards::def(c.to).tags.wuxie
                    });
                if !ok {
                    return Err("无懈：非法的转化牌".into());
                }
                g.to_discard(&carrier, "wuxie")?;
                if skills::wuxie_uncounterable(g, asker) {
                    skills::wuxie_finish(g, &card, 1);
                    return Ok(Step::Done);
                }
                count += 1;
                cursor = 0;
                let mut next = a.data.clone();
                next["cursor"] = json!(cursor);
                next["count"] = json!(count);
                return Ok(Step::Next(Stage::WuxieAsk, next));
            } else if let Some(id) = act.strip_prefix("respond:") {
                g.to_discard(&id.to_string(), "wuxie")?;
            } else if !skills::wuxie_use(g, asker, act)? {
                // 技能提供的无懈替代选项（如解围：装备区牌当无懈）。
                return Err(format!("无懈：未知选项 {act}"));
            }
            // 技能声明「延后」（蛊惑：质疑链结束后重入本帧再计数）。
            if g.skill_deferred {
                g.skill_deferred = false;
                return Ok(Step::Next(Stage::WuxieAsk, a.data.clone()));
            }
            if skills::wuxie_uncounterable(g, asker) {
                skills::wuxie_finish(g, &card, 1);
                return Ok(Step::Done);
            }
            count += 1;
            cursor = 0; // 重新从使用者左手侧开始（嵌套）
            let mut next = a.data.clone();
            next["cursor"] = json!(cursor);
            next["count"] = json!(count);
            Ok(Step::Next(Stage::WuxieAsk, next))
        }
    }
}

// ---------------------------------------------------------------------------
// 判定
// ---------------------------------------------------------------------------

/// 亮出牌堆顶一张牌作为判定牌（进入处理区），发 `JudgeRevealed`。
pub fn judge_card(g: &mut Game, owner: usize) -> Option<CardId> {
    let id = g.draw_top()?;
    g.to_frame(&id).ok()?;
    g.events.push(Event::card(
        EventKind::JudgeRevealed,
        Some(owner),
        id.clone(),
        None,
    ));
    Some(id)
}

/// 判定生效（发 `JudgeEffective`；颂威等订阅）。
pub fn judge_effective(g: &mut Game, owner: usize, card: &CardId) {
    g.events.push(Event::card(
        EventKind::JudgeEffective,
        Some(owner),
        card.clone(),
        None,
    ));
}

/// 判定牌落盘（生效后进弃牌堆）。
pub fn judge_to_discard(g: &mut Game, card: &CardId) -> Result<(), String> {
    g.to_discard(card, "judge")
}
