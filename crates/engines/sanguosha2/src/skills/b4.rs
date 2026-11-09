//! B4（刘封 / 潘璋马忠 / 朱然 / 曹真 / 韩浩史涣 / 陈群 / 吴懿 / 周仓）——按 `docs/games/sanguosha/align/B4.json` 第 7 段实现。
//!
//! 本批落实的裁定：
//! - 「逆」按**移出游戏**（void + 计数，同 B3「权」）；不可被常规获得/弃置；
//! - 司敌「本阶段不能使用和打出该颜色的牌」= **手牌冻结**（`freeze_cards_for_phase`，到期 = 阶段切换）；
//! - 新增 `chained_enter` 事件（`flow::chain_enter`；**所有**进入连环者都发）→ 法恩订阅；
//! - 奔袭第一段 = 固定 -1（等价自带进攻马，可与进攻马叠加，**不**按每张牌累加）；
//! - 「仅指定单一目标」读**目标列表**（`TargetChosen` 的 cause），【杀】同样走目标列表；
//! - 伤害判定一律读**伤害事件**（`damage-vs-hp-loss`），不用体力差。

use serde_json::json;

use crate::cards::{self, CardKind, TargetMode};
use crate::kernel::event::{EventKind, Window};
use crate::kernel::resolve::{Ask, Stage, Step};
use crate::kernel::{flow, CardId, Game, Phase, Slot, Zone};
use crate::presentation::{POutcome, Presentation};

use super::{active, other_alive, SkillDef};

// ---------------------------------------------------------------------------
// 通用小工具
// ---------------------------------------------------------------------------

fn play_phase(g: &Game, seat: usize) -> bool {
    g.turn == seat && g.phase == Phase::Play
}

fn kind_bit(g: &Game, id: &CardId) -> u32 {
    match g.def_of(id).kind {
        CardKind::Basic => 1,
        CardKind::Trick => 2,
        CardKind::Equip => 4,
    }
}

fn equip_ids(g: &Game, seat: usize) -> Vec<CardId> {
    g.ids_in(|z| matches!(z, Zone::Equip(s, _) if *s == seat))
}

fn hand_and_equip(g: &Game, seat: usize) -> Vec<CardId> {
    let mut out = g.hand(seat);
    out.extend(equip_ids(g, seat));
    out
}

/// 区域里的牌（**判定区 → 装备区 → 手牌**，与裁定「自动取牌」顺序一致）。
fn zone_cards_ordered(g: &Game, seat: usize) -> Vec<CardId> {
    let mut out = g.judge_zone(seat);
    out.extend(equip_ids(g, seat));
    out.extend(g.hand(seat));
    out
}

fn in_attack_range(g: &Game, from: usize, to: usize) -> bool {
    from != to && g.distance(from, to) <= g.attack_range(from)
}

fn label(g: &Game, id: &CardId) -> String {
    g.card_label(id)
}

fn zone_label(g: &Game, id: &CardId) -> &'static str {
    match g.zone_of(id) {
        Some(Zone::Judge(_)) => "判定区",
        Some(Zone::Equip(_, _)) => "装备区",
        Some(Zone::Hand(_)) => "手牌",
        Some(Zone::Void) => "移出游戏",
        _ => "弃牌堆",
    }
}

// ---------------------------------------------------------------------------
// 陷嗣（刘封）：准备阶段取至多两名角色各一张牌为「逆」（移出游戏）；
//           其他角色可移去两张「逆」视为对刘封使用【杀】（不限次数）
// ---------------------------------------------------------------------------

pub const XIANSI: SkillDef = SkillDef {
    code: "xiansi",
    name: "陷嗣",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnPhase(Phase::Prepare),
    priority: 20,
    can_active: Some(can_xiansi),
    handler: xiansi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

/// 陷嗣拥有者（第一个存活的）。
fn xiansi_owner(g: &Game, user: usize) -> Option<usize> {
    let mut order = vec![user];
    order.extend(g.seats_from_left_of(user));
    order
        .into_iter()
        .find(|i| active(g, *i, "xiansi") && g.seats[*i].hp > 0)
}

/// 其他角色：只要刘封「逆」≥2 就能发动（不限次数，裁定 Q1=B）。
fn can_xiansi(g: &Game, seat: usize) -> bool {
    let Some(owner) = xiansi_owner(g, seat) else {
        return false;
    };
    owner != seat && g.seats[seat].hp > 0 && super::ni(g, owner) >= 2
}

fn xiansi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    // 主动使用（其他角色移去两张「逆」）
    if a.data.get("mode").and_then(|v| v.as_str()) == Some("use") {
        let Some(owner) = xiansi_owner(g, a.actor) else {
            return Ok(Step::Done);
        };
        if owner == a.actor || super::ni(g, owner) < 2 {
            return Ok(Step::Done);
        }
        // 移去两张「逆」→ 弃牌堆（「逆」是移出游戏的牌，移去 = 进弃牌堆）
        let removed = super::take_void_marked(g, owner, "ni", 2)?;
        if removed.len() < 2 {
            return Ok(Step::Done);
        }
        g.use_virtual(a.actor, "杀", vec![owner]);
        g.presentation.push(
            Presentation::new("xiansi", POutcome::Activated {
                skill: "陷嗣".to_string(),
            })
            .actor(a.actor)
            .target(owner)
            .fx("xiansi_sha"),
        );
        g.push_log(format!(
            "陷嗣：{} 移去两张「逆」，视为对 {} 使用【杀】",
            g.seats[a.actor].name, g.seats[owner].name
        ));
        return Ok(Step::Done);
    }
    // 准备阶段触发
    if g.answer.is_none() && a.data.get("event").is_some() {
        if a.data["cause"].as_str() != Some("Prepare") {
            return Ok(Step::Done);
        }
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(a.actor) {
            return Ok(Step::Done);
        }
        if g.counter("turn.xiansi_offered") > 0 {
            return Ok(Step::Done);
        }
    }
    match a.stage {
        Stage::Choose => {
            let picked: Vec<usize> = a.data["picked"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_u64().map(|n| n as usize)).collect())
                .unwrap_or_default();
            let cands: Vec<usize> = other_alive(g, a.actor)
                .into_iter()
                .filter(|i| !zone_cards_ordered(g, *i).is_empty())
                .filter(|i| picked.len() < 2 || picked.contains(i))
                .collect();
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choice").skill("xiansi");
                    for i in &cands {
                        let mark = if picked.contains(i) { "✔ " } else { "" };
                        b = b.option(
                            format!("target:{i}"),
                            format!("{mark}取 {} 一张牌为「逆」", g.seats[*i].name),
                        );
                    }
                    if !picked.is_empty() {
                        b = b.option("confirm", format!("确认（{} 名角色）", picked.len()));
                    }
                    Ok(Step::Need(b.escape("不发动陷嗣").build()))
                }
                Some("pass") => {
                    g.bump("turn.xiansi_offered");
                    Ok(Step::Done)
                }
                Some("confirm") => {
                    if picked.is_empty() {
                        return Err("陷嗣：至少选择 1 名角色".into());
                    }
                    g.bump("turn.xiansi_offered");
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "picked": picked, "cursor": 0 }),
                    ))
                }
                Some(act) => {
                    let target: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("陷嗣：未知选项 {act}"))?;
                    if !cands.contains(&target) {
                        return Err("陷嗣：该角色不可选".into());
                    }
                    let mut next = picked.clone();
                    match next.iter().position(|x| *x == target) {
                        Some(pos) => {
                            next.remove(pos);
                        }
                        None => next.push(target),
                    }
                    Ok(Step::Next(Stage::Choose, json!({ "picked": next })))
                }
            }
        }
        Stage::Target => {
            let picked: Vec<usize> = a.data["picked"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_u64().map(|n| n as usize)).collect())
                .unwrap_or_default();
            let cursor = a.data["cursor"].as_u64().unwrap_or(0) as usize;
            if cursor >= picked.len() {
                return Ok(Step::Done);
            }
            let target = picked[cursor];
            let cards = zone_cards_ordered(g, target);
            if cards.is_empty() {
                return Ok(Step::Next(
                    Stage::Target,
                    json!({ "picked": picked, "cursor": cursor + 1 }),
                ));
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choice").skill("xiansi");
                    for id in &cards {
                        let shown = if matches!(g.zone_of(id), Some(Zone::Hand(_))) {
                            "手牌（背面）".to_string()
                        } else {
                            label(g, id)
                        };
                        b = b.option(
                            format!("choose:{id}"),
                            format!("取 {} 的{}{shown}", g.seats[target].name, zone_label(g, id)),
                        );
                    }
                    Ok(Step::Need(
                        b.escape("跳过该角色（不取牌）").build(),
                    ))
                }
                Some("pass") => Ok(Step::Next(
                    Stage::Target,
                    json!({ "picked": picked, "cursor": cursor + 1 }),
                )),
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("陷嗣：未知选项 {act}"))?
                        .to_string();
                    if !cards.contains(&id) {
                        return Err("陷嗣：只能取该角色区域里的牌".into());
                    }
                    let label = label(g, &id);
                    // 「逆」= 移出游戏（用户裁定 Q3=A）+ 归属标记
                    g.to_void(&id)?;
                    super::tag_void(g, &id, a.actor, "ni");
                    g.presentation.push(
                        Presentation::new("xiansi", POutcome::MarkPlaced { n: 1 })
                            .actor(a.actor)
                            .target(target)
                            .cards(vec![id.clone()])
                            .fx("xiansi_ni"),
                    );
                    g.push_log(format!(
                        "陷嗣：将 {} 的【{label}】置于武将牌上为「逆」（共 {} 张，移出游戏）",
                        g.seats[target].name,
                        super::ni(g, a.actor)
                    ));
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "picked": picked, "cursor": cursor + 1 }),
                    ))
                }
            }
        }
        _ => Err("陷嗣：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 夺刀（潘璋马忠）：被【杀】伤害后，弃一张牌（手牌/装备区）获得其武器
// ---------------------------------------------------------------------------

pub const DUODAO: SkillDef = SkillDef {
    code: "duodao",
    name: "夺刀",
    triggers: &[EventKind::DamageAfter],
    window: Window::AnyTime,
    priority: 30,
    can_active: None,
    handler: duodao,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn weapon_of(g: &Game, seat: usize) -> Option<CardId> {
    g.ids_in(|z| matches!(z, Zone::Equip(s, Slot::Weapon) if *s == seat))
        .into_iter()
        .next()
}

fn duodao(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if g.answer.is_none() && a.data.get("event").is_some() {
        if !a.data["event"].as_str().unwrap_or("").starts_with("DamageAfter") {
            return Ok(Step::Done);
        }
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(a.actor) {
            return Ok(Step::Done);
        }
        if a.data["amount"].as_i64().unwrap_or(0) <= 0 || g.seats[a.actor].hp <= 0 {
            return Ok(Step::Done);
        }
        let source = a
            .data
            .get("cause")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<usize>().ok());
        let Some(source) = source else {
            return Ok(Step::Done);
        };
        if source == a.actor || weapon_of(g, source).is_none() {
            return Ok(Step::Done);
        }
        // 读**伤害事件**（裁定 Q2=A）：转化/虚拟【杀】也触发。
        let Some(card) = a.data.get("card").and_then(|v| v.as_str()).map(|s| s.to_string()) else {
            return Ok(Step::Done);
        };
        if !g.def_of(&card).tags.sha {
            return Ok(Step::Done);
        }
        if hand_and_equip(g, a.actor).is_empty() {
            return Ok(Step::Done);
        }
    }
    match a.stage {
        Stage::Choose => match g.answer.take().as_deref() {
            None => {
                let source = a
                    .data
                    .get("cause")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(0);
                Ok(Step::Need(
                    super::builder(a.actor, "choice")
                        .skill("duodao")
                        .option(
                            "choose:duodao",
                            format!(
                                "夺刀：弃置一张牌，获得 {} 的武器",
                                g.seats[source].name
                            ),
                        )
                        .escape("不发动夺刀")
                        .build(),
                ))
            }
            Some("pass") => Ok(Step::Done),
            Some("choose:duodao") => {
                let source = a
                    .data
                    .get("cause")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<usize>().ok())
                    .ok_or("夺刀：缺少伤害来源")?;
                Ok(Step::Next(Stage::DiscardOwner, json!({ "source": source })))
            }
            Some(other) => Err(format!("夺刀：未知选项 {other}")),
        },
        Stage::DiscardOwner => {
            let source = a.data["source"].as_u64().unwrap_or(0) as usize;
            // 裁定 Q3=A：发动前再校验武器仍在
            let Some(weapon) = weapon_of(g, source) else {
                g.push_log("夺刀：来源已无武器，取消".to_string());
                return Ok(Step::Done);
            };
            let cards = hand_and_equip(g, a.actor);
            if cards.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choose").skill("duodao");
                    for id in &cards {
                        b = b.option(
                            format!("choose:{id}"),
                            format!("弃置{}的 {}", zone_label(g, id), label(g, id)),
                        );
                    }
                    Ok(Step::Need(b.escape("不发动夺刀").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("夺刀：未知选项 {act}"))?
                        .to_string();
                    if !cards.contains(&id) {
                        return Err("夺刀：只能弃置自己的牌".into());
                    }
                    // 裁定 Q3=A：真正弃牌前再校验一次（武器在询问期间被移走则取消，不付成本）
                    if weapon_of(g, source).is_none() {
                        g.push_log("夺刀：来源已无武器，取消".to_string());
                        return Ok(Step::Done);
                    }
                    let spent = label(g, &id);
                    g.to_discard(&id, "duodao")?;
                    g.gain(&weapon, a.actor)?;
                    let wlabel = label(g, &weapon);
                    g.presentation.push(
                        Presentation::new("duodao", POutcome::Activated {
                            skill: "夺刀".to_string(),
                        })
                        .actor(a.actor)
                        .target(source)
                        .cards(vec![weapon.clone()])
                        .fx("duodao"),
                    );
                    g.push_log(format!(
                        "夺刀：弃置【{spent}】，获得 {} 的武器【{wlabel}】",
                        g.seats[source].name
                    ));
                    Ok(Step::Done)
                }
            }
        }
        _ => Err("夺刀：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 暗箭（潘璋马忠，锁定）：不在其攻击范围内时【杀】伤害 +1
// ---------------------------------------------------------------------------

pub const ANJIAN: SkillDef = SkillDef {
    code: "anjian",
    name: "暗箭",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: noop,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

/// 伤害数值加成（锁定技，无询问）：`flow::damage_now` 在扣血前调用。
/// 判定按**造成伤害时**的当前攻击范围（裁定 Q2=A）。
pub fn anjian_bonus(g: &Game, source: usize, victim: usize, card: Option<&CardId>) -> i32 {
    if !active(g, source, "anjian") || source == victim {
        return 0;
    }
    let Some(card) = card else { return 0 };
    if !g.def_of(card).tags.sha {
        return 0;
    }
    if in_attack_range(g, victim, source) {
        0
    } else {
        1
    }
}

fn noop(_g: &mut Game, _a: &Ask) -> Result<Step, String> {
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 胆守（朱然）：第 X 次发动弃 X 张牌（手牌/装备区，自动支付），只需选目标
// ---------------------------------------------------------------------------

pub const DANSHOU: SkillDef = SkillDef {
    code: "danshou",
    name: "胆守",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_danshou),
    handler: danshou,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn danshou_x(g: &Game) -> usize {
    g.counter("turn.danshou_used") as usize + 1
}

fn danshou_targets(g: &Game, seat: usize) -> Vec<usize> {
    other_alive(g, seat)
        .into_iter()
        .filter(|i| in_attack_range(g, seat, *i))
        .collect()
}

fn can_danshou(g: &Game, seat: usize) -> bool {
    active(g, seat, "danshou")
        && play_phase(g, seat)
        && hand_and_equip(g, seat).len() >= danshou_x(g)
        && !danshou_targets(g, seat).is_empty()
}

fn danshou(g: &mut Game, a: &Ask) -> Result<Step, String> {
    match a.stage {
        Stage::Choose => {
            let x = danshou_x(g);
            let cards = hand_and_equip(g, a.actor);
            if cards.len() < x {
                return Ok(Step::Done);
            }
            // 裁定 Q2=C：不提供选牌交互，按「手牌优先、再装备区」自动支付。
            let mut paid = Vec::new();
            for id in cards.into_iter().take(x) {
                g.to_discard(&id, "danshou")?;
                paid.push(id);
            }
            g.bump("turn.danshou_used");
            g.presentation.push(
                Presentation::new("danshou", POutcome::Discard { n: paid.len() as u32 })
                    .actor(a.actor)
                    .cards(paid.clone())
                    .fx("danshou_cost"),
            );
            g.push_log(format!("胆守：弃置 {} 张牌（X={x}）", paid.len()));
            Ok(Step::Next(Stage::Target, json!({ "x": x })))
        }
        Stage::Target => {
            let x = a.data["x"].as_u64().unwrap_or(1) as usize;
            let cands = danshou_targets(g, a.actor);
            if cands.is_empty() {
                // 裁定 Q3=A：没有目标不能发动（代价不退，属于操作失误提示）
                g.push_log("胆守：没有合法目标（代价不退）".to_string());
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "target").skill("danshou");
                    for i in &cands {
                        b = b.option(
                            format!("target:{i}"),
                            format!("对 {} 发动胆守（X={x}）", g.seats[*i].name),
                        );
                    }
                    Ok(Step::Need(b.escape("取消（代价不退）").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let target: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("胆守：目标不合法 {act}"))?;
                    if !cands.contains(&target) {
                        return Err("胆守：目标不在攻击范围内".into());
                    }
                    resolve_danshou(g, a.actor, target, x)?;
                    Ok(Step::Done)
                }
            }
        }
        _ => Err("胆守：未知阶段".into()),
    }
}

fn resolve_danshou(g: &mut Game, actor: usize, target: usize, x: usize) -> Result<(), String> {
    match x {
        1 => {
            // 弃置其一张牌（自动取：判定区 → 装备区 → 手牌）
            if let Some(id) = zone_cards_ordered(g, target).into_iter().next() {
                let label = label(g, &id);
                g.to_discard(&id, "danshou")?;
                g.push_log(format!(
                    "胆守①：弃置 {} 的【{label}】",
                    g.seats[target].name
                ));
            }
        }
        2 => {
            if let Some(id) = zone_cards_ordered(g, target).into_iter().next() {
                let label = label(g, &id);
                g.gain(&id, actor)?;
                g.push_log(format!(
                    "胆守②：{} 交给【{label}】",
                    g.seats[target].name
                ));
            }
        }
        3 => {
            g.push_log(format!("胆守③：对 {} 造成 1 点伤害", g.seats[target].name));
            flow::damage(g, Some(actor), target, 1);
        }
        _ => {
            let a = g.draw(actor, 2).len();
            let b = g.draw(target, 2).len();
            g.push_log(format!(
                "胆守④：你摸 {a} 张，{} 摸 {b} 张",
                g.seats[target].name
            ));
        }
    }
    g.presentation.push(
        Presentation::new("danshou", POutcome::Activated {
            skill: "胆守".to_string(),
        })
        .actor(actor)
        .target(target)
        .fx(format!("danshou_{x}")),
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 司敌（曹真）：其他角色出牌阶段开始时可弃同色非基本牌冻结其该色手牌；
//               阶段结束若其未使用【杀】，视为对其使用【杀】
// ---------------------------------------------------------------------------

pub const SIDI: SkillDef = SkillDef {
    code: "sidi",
    name: "司敌",
    triggers: &[EventKind::PhaseStart, EventKind::PhaseEnd],
    window: Window::AnyTime,
    priority: 25,
    can_active: None,
    handler: sidi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

/// 司敌费用（裁定 Q1=B）：装备区所有装备 + 与装备**同色**的非基本手牌。
fn sidi_costs(g: &Game, owner: usize) -> Vec<CardId> {
    let equips = equip_ids(g, owner);
    if equips.is_empty() {
        return Vec::new();
    }
    let colors: Vec<bool> = equips.iter().map(|id| g.is_red(id)).collect();
    let mut out: Vec<CardId> = equips.clone();
    out.extend(
        g.hand(owner)
            .into_iter()
            .filter(|id| g.def_of(id).kind != CardKind::Basic)
            .filter(|id| colors.contains(&g.is_red(id))),
    );
    out
}

fn sidi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    // 触发期校验只在「首次进入帧」（尚未提交回答）时做一次。
    if g.answer.is_none() {
        let Some(event) = a.data.get("event").and_then(|v| v.as_str()).map(str::to_string) else {
            return Ok(Step::Done);
        };
        let phase_end = event.starts_with("PhaseEnd");
        if a.data["cause"].as_str() != Some("Play") {
            return Ok(Step::Done);
        }
        let current = a.data["seat"].as_u64().map(|n| n as usize).unwrap_or(usize::MAX);
        if current == a.actor {
            return Ok(Step::Done);
        }
        if !phase_end {
            if g.counter("turn.sidi_offered") > 0 || sidi_costs(g, a.actor).is_empty() {
                return Ok(Step::Done);
            }
        } else {
            sidi_end_play(g, a.actor, current);
            return Ok(Step::Done);
        }
    }
    let current = a.data["seat"].as_u64().map(|n| n as usize).unwrap_or(usize::MAX);
    let costs = sidi_costs(g, a.actor);
    if costs.is_empty() {
        return Ok(Step::Done);
    }
    match a.stage {
        Stage::Choose => match g.answer.take().as_deref() {
            None => {
                let mut b = super::builder(a.actor, "choice").skill("sidi");
                for id in &costs {
                    let color = if g.is_red(id) { "红" } else { "黑" };
                    b = b.option(
                        format!("choose:{id}"),
                        format!(
                            "弃置{}的 {}，封锁 {}的{}色牌",
                            zone_label(g, id),
                            label(g, id),
                            g.seats[current].name,
                            color
                        ),
                    );
                }
                return Ok(Step::Need(b.escape("不发动司敌").build()));
            }
            Some("pass") => {
                g.bump("turn.sidi_offered");
                return Ok(Step::Done);
            }
            Some(act) => {
                let id = act
                    .strip_prefix("choose:")
                    .ok_or_else(|| format!("司敌：未知选项 {act}"))?
                    .to_string();
                if !costs.contains(&id) {
                    return Err("司敌：费用牌不合法".into());
                }
                let red = g.is_red(&id);
                let label = label(g, &id);
                g.to_discard(&id, "sidi")?;
                g.bump("turn.sidi_offered");
                g.bump(&format!("turn.sidi_target:{current}"));
                let frozen: Vec<CardId> = g
                    .hand(current)
                    .into_iter()
                    .filter(|c| g.is_red(c) == red)
                    .collect();
                g.freeze_cards_for_phase(current, &frozen);
                g.presentation.push(
                    Presentation::new("sidi", POutcome::Activated {
                        skill: "司敌".to_string(),
                    })
                    .actor(a.actor)
                    .target(current)
                    .cards(frozen.clone())
                    .fx("sidi_freeze"),
                );
                g.push_log(format!(
                    "司敌：弃置【{label}】，{} 本阶段不能使用/打出{}色牌（冻结 {} 张手牌）",
                    g.seats[current].name,
                    if red { "红" } else { "黑" },
                    frozen.len()
                ));
                return Ok(Step::Done);
            }
        },
        _ => Err("司敌：未知阶段".into()),
    }
}

/// 司敌第二时刻：该角色出牌阶段结束，若未使用【杀】→ 视为对其使用【杀】。
fn sidi_end_play(g: &mut Game, owner: usize, current: usize) {
    let key = format!("turn.sidi_target:{current}");
    if g.counter(&key) == 0 {
        return;
    }
    if g.seats[owner].hp <= 0 || g.seats[current].hp <= 0 {
        return;
    }
    if g.sha_used(current) > 0 {
        g.push_log(format!(
            "司敌：{} 本阶段使用过【杀】，不触发",
            g.seats[current].name
        ));
        return;
    }
    g.use_virtual(owner, "杀", vec![current]);
    g.presentation.push(
        Presentation::new("sidi", POutcome::Activated {
            skill: "司敌".to_string(),
        })
        .actor(owner)
        .target(current)
        .fx("sidi_sha"),
    );
    g.push_log(format!(
        "司敌：{} 未使用【杀】，视为对其使用【杀】",
        g.seats[current].name
    ));
}

// ---------------------------------------------------------------------------
// 慎断（韩浩史涣）：黑色基本牌被弃置后，可当无距离【兵粮寸断】使用
// ---------------------------------------------------------------------------

pub const SHENDUAN: SkillDef = SkillDef {
    code: "shenduan",
    name: "慎断",
    triggers: &[EventKind::CardDiscarded],
    window: Window::AnyTime,
    priority: 20,
    can_active: None,
    handler: shenduan,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn shenduan(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if g.answer.is_none() && a.data.get("event").is_some() {
        if !a.data["event"].as_str().unwrap_or("").starts_with("CardDiscarded") {
            return Ok(Step::Done);
        }
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(a.actor) {
            return Ok(Step::Done);
        }
        let Some(card) = a.data.get("card").and_then(|v| v.as_str()).map(|s| s.to_string()) else {
            return Ok(Step::Done);
        };
        // 同一张牌只问一次；牌已不在弃牌堆则不再询问（裁定 Q1=A）
        if g.zone_of(&card) != Some(&Zone::Discard) {
            return Ok(Step::Done);
        }
        let def = g.def_of(&card);
        if def.kind != CardKind::Basic || g.is_red(&card) {
            return Ok(Step::Done);
        }
        if g.counter(&format!("turn.shenduan_asked:{card}")) > 0 {
            return Ok(Step::Done);
        }
    }
    let card = a.data["card"].as_str().unwrap_or_default().to_string();
    match a.stage {
        Stage::Choose => match g.answer.take().as_deref() {
            None => {
                g.bump(&format!("turn.shenduan_asked:{card}"));
                Ok(Step::Need(
                    super::builder(a.actor, "choice")
                        .skill("shenduan")
                        .option(
                            "choose:shenduan",
                            format!("慎断：将【{}】当无距离限制的【兵粮寸断】使用", label(g, &card)),
                        )
                        .escape("不发动慎断")
                        .build(),
                ))
            }
            Some("pass") => Ok(Step::Done),
            Some("choose:shenduan") => Ok(Step::Next(Stage::Target, json!({ "card": card }))),
            Some(other) => Err(format!("慎断：未知选项 {other}")),
        },
        Stage::Target => {
            let card = a.data["card"].as_str().unwrap_or_default().to_string();
            let cands: Vec<usize> = other_alive(g, a.actor)
                .into_iter()
                .filter(|i| !g.seats[*i].judge_abolished)
                .collect();
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "target").skill("shenduan");
                    for i in &cands {
                        b = b.option(
                            format!("target:{i}"),
                            format!("对 {} 使用【兵粮寸断】（无距离）", g.seats[*i].name),
                        );
                    }
                    Ok(Step::Need(b.escape("取消（牌留在弃牌堆）").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let target: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("慎断：目标不合法 {act}"))?;
                    if !cands.contains(&target) {
                        return Err("慎断：目标不可选".into());
                    }
                    g.set_virtual(&card, "兵粮寸断", vec![card.clone()]);
                    g.use_card(a.actor, &card, vec![target]);
                    // v2 尚无延时锦囊结算：等价于结算完成 → 原牌进弃牌堆并清除虚拟定义。
                    if g.zone_of(&card) != Some(&Zone::Discard) {
                        g.to_discard(&card, "shenduan")?;
                    }
                    g.clear_virtual(&card);
                    g.push_log(format!(
                        "慎断：将【{}】当无距离限制的【兵粮寸断】对 {} 使用",
                        label(g, &card),
                        g.seats[target].name
                    ));
                    Ok(Step::Done)
                }
            }
        }
        _ => Err("慎断：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 勇略（韩浩史涣）：其他角色判定阶段开始时，弃其判定牌视为对其使用【杀】
// ---------------------------------------------------------------------------

pub const YONGLUE: SkillDef = SkillDef {
    code: "yonglue",
    name: "勇略",
    triggers: &[EventKind::PhaseStart],
    window: Window::AnyTime,
    priority: 24,
    can_active: None,
    handler: yonglue,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn yonglue(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if g.answer.is_none() && a.data.get("event").is_some() {
        if !a.data["event"].as_str().unwrap_or("").starts_with("PhaseStart") {
            return Ok(Step::Done);
        }
        if a.data["cause"].as_str() != Some("Judge") {
            return Ok(Step::Done);
        }
        let current = a.data["seat"].as_u64().map(|n| n as usize).unwrap_or(usize::MAX);
        if current == a.actor || g.counter("turn.yonglue_offered") > 0 {
            return Ok(Step::Done);
        }
        if !in_attack_range(g, a.actor, current) || g.judge_zone(current).is_empty() {
            return Ok(Step::Done);
        }
    }
    match a.stage {
        Stage::Choose => {
            let current = a.data["seat"].as_u64().map(|n| n as usize).unwrap_or(usize::MAX);
            let cards = g.judge_zone(current);
            if cards.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choice").skill("yonglue");
                    for id in &cards {
                        b = b.option(
                            format!("choose:{id}"),
                            format!(
                                "弃置 {} 的判定牌 {}，视为对其使用【杀】",
                                g.seats[current].name,
                                label(g, id)
                            ),
                        );
                    }
                    Ok(Step::Need(b.escape("不发动勇略").build()))
                }
                Some("pass") => {
                    g.bump("turn.yonglue_offered");
                    Ok(Step::Done)
                }
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("勇略：未知选项 {act}"))?
                        .to_string();
                    if !cards.contains(&id) {
                        return Err("勇略：只能弃置其判定区的牌".into());
                    }
                    g.to_discard(&id, "yonglue")?;
                    g.bump("turn.yonglue_offered");
                    // 「此【杀】未造成伤害则摸 1」由 `on_sha_finished` 结算（裁定 Q1=A：读伤害事件）。
                    super::set_yonglue_owner(g, a.actor);
                    g.bump(&format!("turn.yonglue_pending:{current}"));
                    g.use_virtual(a.actor, "杀", vec![current]);
                    g.presentation.push(
                        Presentation::new("yonglue", POutcome::Activated {
                            skill: "勇略".to_string(),
                        })
                        .actor(a.actor)
                        .target(current)
                        .fx("yonglue_sha"),
                    );
                    g.push_log(format!(
                        "勇略：弃置 {} 的判定牌，视为对其使用【杀】",
                        g.seats[current].name
                    ));
                    Ok(Step::Done)
                }
            }
        }
        _ => Err("勇略：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 品第（陈群）：弃未用过的类型牌 → 令一名其他角色摸/弃 X 张；已受伤则陈群连环
// ---------------------------------------------------------------------------

pub const PINDI: SkillDef = SkillDef {
    code: "pindi",
    name: "品第",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_pindi),
    handler: pindi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn pindi_cards(g: &Game, seat: usize) -> Vec<CardId> {
    let used = g.counter("turn.pindi_types");
    g.hand(seat)
        .into_iter()
        .filter(|id| kind_bit(g, id) & used == 0)
        .collect()
}

fn pindi_targets(g: &Game, seat: usize) -> Vec<usize> {
    other_alive(g, seat)
        .into_iter()
        .filter(|i| g.counter(&format!("turn.pindi_target:{i}")) == 0)
        .collect()
}

fn can_pindi(g: &Game, seat: usize) -> bool {
    active(g, seat, "pindi")
        && play_phase(g, seat)
        && !pindi_cards(g, seat).is_empty()
        && !pindi_targets(g, seat).is_empty()
}

fn pindi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    match a.stage {
        Stage::Choose => {
            let cards = pindi_cards(g, a.actor);
            if cards.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choose").skill("pindi");
                    for id in &cards {
                        b = b.option(
                            format!("choose:{id}"),
                            format!("弃置 {}", label(g, id)),
                        );
                    }
                    Ok(Step::Need(b.escape("不发动品第").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("品第：未知选项 {act}"))?
                        .to_string();
                    if !cards.contains(&id) {
                        return Err("品第：该牌类型本阶段已用过".into());
                    }
                    let bit = kind_bit(g, &id);
                    let label = label(g, &id);
                    g.to_discard(&id, "pindi")?;
                    let types = g.counter("turn.pindi_types") | bit;
                    g.counters.insert("turn.pindi_types".to_string(), types);
                    g.bump("turn.pindi_count");
                    let x = g.counter("turn.pindi_count");
                    g.push_log(format!("品第：弃置【{label}】（X={x}）"));
                    Ok(Step::Next(Stage::Target, json!({ "x": x })))
                }
            }
        }
        Stage::Target => {
            let x = a.data["x"].as_u64().unwrap_or(1) as usize;
            let cands = pindi_targets(g, a.actor);
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "target").skill("pindi");
                    for i in &cands {
                        b = b.option(
                            format!("target:{i}"),
                            format!("令 {} 摸/弃 {x} 张", g.seats[*i].name),
                        );
                    }
                    Ok(Step::Need(b.escape("取消（牌已弃置）").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let target: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("品第：目标不合法 {act}"))?;
                    if !cands.contains(&target) {
                        return Err("品第：该角色本回合已被选过".into());
                    }
                    Ok(Step::Next(
                        Stage::Mode,
                        json!({ "x": x, "target": target }),
                    ))
                }
            }
        }
        Stage::Mode => {
            let x = a.data["x"].as_u64().unwrap_or(1) as usize;
            let target = a.data["target"].as_u64().unwrap_or(0) as usize;
            match g.answer.take().as_deref() {
                None => Ok(Step::Need(
                    super::builder(a.actor, "choice")
                        .skill("pindi")
                        .option("opt1", format!("令其摸 {x} 张牌"))
                        .escape_with("opt2", format!("令其弃置 {x} 张牌"))
                        .build(),
                )),
                Some("opt1") => {
                    g.bump(&format!("turn.pindi_target:{target}"));
                    let n = g.draw(target, x).len();
                    g.push_log(format!("品第：{} 摸 {n} 张", g.seats[target].name));
                    after_pindi(g, a.actor, target)
                }
                Some("opt2") => {
                    g.bump(&format!("turn.pindi_target:{target}"));
                    // 裁定 Q1=A：由目标自己选弃哪张
                    Ok(Step::Next(
                        Stage::DiscardTarget,
                        json!({ "x": x, "target": target }),
                    ))
                }
                Some(other) => Err(format!("品第：未知选项 {other}")),
            }
        }
        Stage::DiscardTarget => {
            let x = a.data["x"].as_u64().unwrap_or(1) as usize;
            let target = a.data["target"].as_u64().unwrap_or(0) as usize;
            let hand = g.hand(target);
            if hand.is_empty() {
                return after_pindi(g, a.actor, target);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(target, "choose").skill("pindi");
                    for id in &hand {
                        b = b.option(
                            format!("choose:{id}"),
                            format!("弃置 {}", label(g, id)),
                        );
                    }
                    Ok(Step::Need(b.escape("不弃置（手牌不足）").build()))
                }
                Some("pass") => after_pindi(g, a.actor, target),
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("品第：未知选项 {act}"))?
                        .to_string();
                    if !hand.contains(&id) {
                        return Err("品第：只能弃置自己的手牌".into());
                    }
                    g.to_discard(&id, "pindi")?;
                    let left = g.counter(&format!("turn.pindi_discard:{target}")) + 1;
                    g.counters
                        .insert(format!("turn.pindi_discard:{target}"), left);
                    g.push_log(format!(
                        "品第：{} 弃置一张手牌",
                        g.seats[target].name
                    ));
                    if left as usize >= x || g.hand(target).is_empty() {
                        after_pindi(g, a.actor, target)
                    } else {
                        Ok(Step::Next(
                            Stage::DiscardTarget,
                            json!({ "x": x, "target": target }),
                        ))
                    }
                }
            }
        }
        _ => Err("品第：未知阶段".into()),
    }
}

fn after_pindi(g: &mut Game, actor: usize, target: usize) -> Result<Step, String> {
    if g.seats[target].hp < g.seats[target].max_hp {
        // 进入连环状态 → 发 `chained_enter`（所有进入者都发，裁定 Q3=A）
        flow::chain_enter(g, actor);
    }
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 法恩（陈群）：任意角色翻至正面或进入连环状态后，可令其摸一张
// ---------------------------------------------------------------------------

pub const FAEN: SkillDef = SkillDef {
    code: "faen",
    name: "法恩",
    triggers: &[EventKind::ChainedEnter],
    window: Window::AnyTime,
    priority: 40,
    can_active: None,
    handler: faen,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn faen(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if g.answer.is_none() && a.data.get("event").is_some() {
        if !a.data["event"].as_str().unwrap_or("").starts_with("ChainedEnter") {
            return Ok(Step::Done);
        }
        // 受益者 = 进入连环的角色（可能是别人）
        let Some(beneficiary) = a.data["seat"].as_u64().map(|n| n as usize) else {
            return Ok(Step::Done);
        };
        if !g.seats[beneficiary].hp.gt(&0) {
            return Ok(Step::Done);
        }
    }
    let beneficiary = a
        .data
        .get("beneficiary")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .or_else(|| a.data["seat"].as_u64().map(|n| n as usize))
        .unwrap_or(a.actor);
    match a.stage {
        Stage::Choose => match g.answer.take().as_deref() {
            None => Ok(Step::Need(
                super::builder(a.actor, "choice")
                    .skill("faen")
                    .option(
                        "choose:faen",
                        format!("法恩：令 {} 摸一张牌", g.seats[beneficiary].name),
                    )
                    .escape("不发动法恩")
                    .build(),
            )),
            Some("pass") => {
                // 同一次连环只问一次
                g.bump(&format!("turn.faen_asked:{beneficiary}"));
                Ok(Step::Done)
            }
            Some("choose:faen") => {
                g.bump(&format!("turn.faen_asked:{beneficiary}"));
                let n = g.draw(beneficiary, 1).len();
                g.presentation.push(
                    Presentation::new("faen", POutcome::Draw { n: n as u32 })
                        .actor(a.actor)
                        .target(beneficiary)
                        .fx("faen"),
                );
                g.push_log(format!("法恩：{} 摸 {n} 张", g.seats[beneficiary].name));
                Ok(Step::Done)
            }
            Some(other) => Err(format!("法恩：未知选项 {other}")),
        },
        _ => Err("法恩：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 奔袭（吴懿，锁定）：距离 -1（固定，等价进攻马）+ 单目标【杀】/普通锦囊的至多两项强化
// ---------------------------------------------------------------------------

pub const BENXI: SkillDef = SkillDef {
    code: "benxi",
    name: "奔袭",
    triggers: &[EventKind::TargetChosen],
    window: Window::AnyTime,
    priority: 45,
    can_active: None,
    handler: benxi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

/// 目标列表（`TargetChosen` 的 cause，逗号分隔）。
fn target_list(a: &Ask) -> Vec<usize> {
    a.data
        .get("cause")
        .and_then(|v| v.as_str())
        .map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect())
        .unwrap_or_default()
}

fn benxi_legal_extra(g: &Game, actor: usize, used: &[usize]) -> Option<usize> {
    other_alive(g, actor)
        .into_iter()
        .find(|i| !used.contains(i) && in_attack_range(g, actor, *i))
}

fn benxi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if g.answer.is_none() && a.data.get("event").is_some() {
        if !a.data["event"].as_str().unwrap_or("").starts_with("TargetChosen") {
            return Ok(Step::Done);
        }
        // 使用者 == 吴懿，且在自己的回合内
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(a.actor) {
            return Ok(Step::Done);
        }
        if !active(g, a.actor, "benxi") || g.turn != a.actor {
            return Ok(Step::Done);
        }
        let Some(card) = a.data.get("card").and_then(|v| v.as_str()).map(|s| s.to_string()) else {
            return Ok(Step::Done);
        };
        // 只有【杀】或普通锦囊才可能触发
        let def = g.def_of(&card);
        let usable = def.tags.sha
            || (def.kind == CardKind::Trick && !def.tags.delayed && cards::target_mode(&g.name_of(&card)) == TargetMode::Chosen);
        if !usable {
            return Ok(Step::Done);
        }
        // 「仅指定单一目标」读目标列表长度（裁定 Q1=A）
        let targets = target_list(a);
        if targets.len() != 1 {
            return Ok(Step::Done);
        }
        // 与所有其他角色距离均为 1
        let all_one = other_alive(g, a.actor)
            .into_iter()
            .all(|i| g.distance(a.actor, i) <= 1);
        if !all_one {
            return Ok(Step::Done);
        }
        if g.counter(&format!("turn.benxi_asked:{card}")) > 0 {
            return Ok(Step::Done);
        }
        g.bump(&format!("turn.benxi_asked:{card}"));
    }
    let card = a.data["card"].as_str().unwrap_or_default().to_string();
    let targets = target_list(a);
    match a.stage {
        Stage::Choose => {
            let opts = [
                ("opt_multi", "目标数 +1"),
                ("opt_armor", "无视防具"),
                ("opt_nores", "不能被抵消"),
                ("opt_draw", "造成伤害后摸一张"),
            ];
            // 裁定 Q3：没有额外合法目标时「目标数+1」不生效（不可选）
            let multi_ok = benxi_legal_extra(g, a.actor, &targets).is_some();
            let selected: Vec<String> = a.data["selected"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                .unwrap_or_default();
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choose_many").skill("benxi");
                    for (id, label) in opts {
                        if id == "opt_multi" && !multi_ok {
                            continue;
                        }
                        let mark = if selected.iter().any(|s| s == id) { "✔ " } else { "" };
                        b = b.option(format!("{id}"), format!("奔袭：{mark}{label}"));
                    }
                    if !selected.is_empty() {
                        b = b.option("confirm", format!("确认（{} 项）", selected.len()));
                    }
                    Ok(Step::Need(b.escape("不使用强化").build()))
                }
                Some("pass") => {
                    g.push_log("奔袭：不使用强化".to_string());
                    Ok(Step::Done)
                }
                Some("confirm") => {
                    if selected.is_empty() {
                        return Err("奔袭：至少选择 1 项".into());
                    }
                    apply_benxi(g, a.actor, &card, &targets, &selected);
                    Ok(Step::Done)
                }
                Some(act) => {
                    if !["opt_multi", "opt_armor", "opt_nores", "opt_draw"].contains(&act) {
                        return Err(format!("奔袭：未知选项 {act}"));
                    }
                    let mut next = selected.clone();
                    match next.iter().position(|s| s == act) {
                        Some(pos) => {
                            next.remove(pos);
                        }
                        None => {
                            if next.len() >= 2 {
                                return Err("奔袭：最多选择两项".into());
                            }
                            next.push(act.to_string());
                        }
                    }
                    Ok(Step::Next(Stage::Choose, json!({ "selected": next })))
                }
            }
        }
        _ => Err("奔袭：未知阶段".into()),
    }
}

fn apply_benxi(g: &mut Game, actor: usize, card: &str, targets: &[usize], selected: &[String]) {
    let mut names = Vec::new();
    for s in selected {
        match s.as_str() {
            "opt_multi" => {
                // 目标数 +1：必须追加**合法**目标
                if let Some(extra) = benxi_legal_extra(g, actor, targets) {
                    g.counters
                        .insert("turn.benxi_extra_target".to_string(), extra as u32 + 1);
                    names.push(format!("目标数 +1（追加 {}）", g.seats[extra].name));
                }
            }
            "opt_armor" => {
                g.bump("turn.benxi_armor");
                names.push("无视防具".to_string());
            }
            "opt_nores" => {
                g.mark_unresponsive(&card.to_string());
                names.push("不能被抵消".to_string());
            }
            _ => {
                g.bump("turn.benxi_draw");
                names.push("造成伤害后摸一张".to_string());
            }
        }
    }
    g.presentation.push(
        Presentation::new("benxi", POutcome::Activated {
            skill: "奔袭".to_string(),
        })
        .actor(actor)
        .cards(vec![card.to_string()])
        .fx("benxi"),
    );
    g.push_log(format!("奔袭：{}", names.join("、")));
}

// ---------------------------------------------------------------------------
// 忠勇（周仓）：使用【杀】后可把此【杀】/目标打出的【闪】交给其他角色
// ---------------------------------------------------------------------------

pub const ZHONGYONG: SkillDef = SkillDef {
    code: "zhongyong",
    name: "忠勇",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: zhongyong,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

/// 【杀】结算完毕的接入点（由 `skills::on_sha_finished` 调用；v2 的【杀】结算引擎接入这里）。
/// 返回 true = 已压入忠勇询问帧。
pub fn zhongyong_after_sha(
    g: &mut Game,
    user: usize,
    target: Option<usize>,
    card: Option<&CardId>,
    respond: Option<&CardId>,
) -> bool {
    if !active(g, user, "zhongyong") {
        return false;
    }
    let Some(target) = target else { return false };
    let sha_ok = card.is_some_and(|c| g.zone_of(c) == Some(&Zone::Discard));
    let respond_ok = respond.is_some_and(|c| g.zone_of(c) == Some(&Zone::Discard));
    if !sha_ok && !respond_ok {
        return false;
    }
    crate::kernel::resolve::push_ask(
        g,
        "zhongyong",
        Stage::Choose,
        user,
        json!({
            "target": target,
            "sha": card.map(|c| c.to_string()),
            "shan": respond.map(|c| c.to_string()),
        }),
    );
    true
}

fn zhongyong(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let sha = a.data.get("sha").and_then(|v| v.as_str()).map(str::to_string);
    let shan = a.data.get("shan").and_then(|v| v.as_str()).map(str::to_string);
    match a.stage {
        Stage::Choose => {
            let mut opts: Vec<(String, String)> = Vec::new();
            if let Some(id) = &sha {
                if g.zone_of(id) == Some(&Zone::Discard) {
                    opts.push((
                        "give:sha".to_string(),
                        format!("将此【杀】（{}）交给一名其他角色", label(g, id)),
                    ));
                }
            }
            if let Some(id) = &shan {
                if g.zone_of(id) == Some(&Zone::Discard) {
                    opts.push((
                        "give:shan".to_string(),
                        format!("将此【闪】（{}）交给一名其他角色", label(g, id)),
                    ));
                }
            }
            if opts.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choice").skill("zhongyong");
                    for (id, label) in opts {
                        b = b.option(id, label);
                    }
                    Ok(Step::Need(b.escape("不发动忠勇").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some("give:sha") => {
                    let Some(card) = sha.clone() else {
                        return Err("忠勇：此【杀】已不在弃牌堆".into());
                    };
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "card": card, "sha": sha, "shan": shan }),
                    ))
                }
                Some("give:shan") => {
                    let Some(card) = shan.clone() else {
                        return Err("忠勇：此【闪】已不在弃牌堆".into());
                    };
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "card": card, "sha": sha, "shan": shan }),
                    ))
                }
                Some(other) => Err(format!("忠勇：未知选项 {other}")),
            }
        }
        Stage::Target => {
            let card = a.data["card"].as_str().unwrap_or_default().to_string();
            let cands = other_alive(g, a.actor);
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "target").skill("zhongyong");
                    for i in &cands {
                        b = b.option(format!("target:{i}"), format!("交给 {}", g.seats[*i].name));
                    }
                    Ok(Step::Need(b.escape("不交出").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let receiver: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("忠勇：目标不合法 {act}"))?;
                    if !cands.contains(&receiver) {
                        return Err("忠勇：接收者不合法".into());
                    }
                    // 「交给」= 获得语义（裁定 Q3=A）
                    g.gain(&card, receiver)?;
                    let label = label(g, &card);
                    g.push_log(format!(
                        "忠勇：将【{label}】交给 {}",
                        g.seats[receiver].name
                    ));
                    if g.is_red(&card) {
                        Ok(Step::Next(
                            Stage::PickOther,
                            json!({ "receiver": receiver }),
                        ))
                    } else {
                        Ok(Step::Done)
                    }
                }
            }
        }
        Stage::PickOther => {
            let receiver = a.data["receiver"].as_u64().unwrap_or(0) as usize;
            // 由**接收者**决定是否使用【杀】（裁定 Q1=A）
            let cands: Vec<usize> = other_alive(g, receiver)
                .into_iter()
                .filter(|i| in_attack_range(g, a.actor, *i))
                .collect();
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(receiver, "choice").skill("zhongyong");
                    b = b.option(
                        "choose:zhongyong",
                        "忠勇：对你攻击范围内一名角色使用一张【杀】",
                    );
                    Ok(Step::Need(b.escape("不使用【杀】").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some("choose:zhongyong") => Ok(Step::Next(
                    Stage::YinTarget,
                    json!({ "receiver": receiver }),
                )),
                Some(other) => Err(format!("忠勇：未知选项 {other}")),
            }
        }
        Stage::YinTarget => {
            let receiver = a.data["receiver"].as_u64().unwrap_or(0) as usize;
            let cands: Vec<usize> = other_alive(g, receiver)
                .into_iter()
                .filter(|i| in_attack_range(g, a.actor, *i))
                .collect();
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(receiver, "target").skill("zhongyong");
                    for i in &cands {
                        b = b.option(
                            format!("target:{i}"),
                            format!("对 {} 使用【杀】", g.seats[*i].name),
                        );
                    }
                    Ok(Step::Need(b.escape("不使用【杀】").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let target: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("忠勇：目标不合法 {act}"))?;
                    if !cands.contains(&target) {
                        return Err("忠勇：目标不在周仓的攻击范围内".into());
                    }
                    g.use_virtual(receiver, "杀", vec![target]);
                    g.presentation.push(
                        Presentation::new("zhongyong", POutcome::Activated {
                            skill: "忠勇".to_string(),
                        })
                        .actor(receiver)
                        .target(target)
                        .fx("zhongyong_sha"),
                    );
                    g.push_log(format!(
                        "忠勇：{} 视为对 {} 使用【杀】",
                        g.seats[receiver].name, g.seats[target].name
                    ));
                    Ok(Step::Done)
                }
            }
        }
        _ => Err("忠勇：未知阶段".into()),
    }
}
