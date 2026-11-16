//! 批次技能共享工具（b6 起；b0–b5 各自内联，这里只给新批次复用）。
//!
//! 约定：这里只放**纯工具**（选牌/文案/座位查询），不放任何具体技能语义，
//! 也不新增内核事实（计数/事件仍走 `Game::bump` / `flow::*`）。

use serde_json::Value;

use super::super::cards;
use super::super::pending::PendingBuilder;
use super::super::resolve::Step;
use super::super::{CardId, Game, Zone};

pub fn turn_key(seat: usize, key: &str) -> String {
    format!("turn.{seat}.{key}")
}

pub fn phase_key(seat: usize, key: &str) -> String {
    format!("phase.{seat}.{key}")
}

pub fn seat_of(ask: &super::super::resolve::Ask, key: &str) -> Option<usize> {
    ask.data.get(key).and_then(|v| v.as_u64()).map(|v| v as usize)
}

pub fn card_of(ask: &super::super::resolve::Ask, key: &str) -> Option<CardId> {
    ask.data.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

pub fn seats_of(ask: &super::super::resolve::Ask, key: &str) -> Vec<usize> {
    ask.data
        .get(key)
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_u64())
                .map(|v| v as usize)
                .collect()
        })
        .unwrap_or_default()
}

/// 自己可控的牌（手牌 + 装备区，按牌 id 排序 = 确定性）。
pub fn own_cards(g: &Game, seat: usize) -> Vec<CardId> {
    let mut ids = g.hand(seat);
    ids.extend(equip_cards(g, seat));
    ids.sort();
    ids
}

pub fn equip_cards(g: &Game, seat: usize) -> Vec<CardId> {
    let mut ids: Vec<CardId> = g
        .cards
        .iter()
        .filter(|(_, z)| matches!(z, Zone::Equip(s, _) if *s == seat))
        .map(|(id, _)| id.clone())
        .collect();
    ids.sort();
    ids
}

pub fn equip_count(g: &Game, seat: usize) -> usize {
    equip_cards(g, seat).len()
}

/// 目标身上「获得/弃置一张牌」的合法区（手牌 + 装备区）。
pub fn target_cards(g: &Game, target: usize) -> Vec<CardId> {
    own_cards(g, target)
}

pub fn card_label(g: &Game, id: &str) -> String {
    g.card_defs
        .get(id)
        .and_then(|c| cards::def(&c.def))
        .map(|d| d.name.to_string())
        .unwrap_or_else(|| id.to_string())
}

pub fn def_of(g: &Game, id: &str) -> Option<&'static cards::CardDef> {
    g.card_defs.get(id).and_then(|c| cards::def(&c.def))
}

pub fn alive_others(g: &Game, seat: usize) -> Vec<usize> {
    (0..g.seats.len())
        .filter(|&i| i != seat && g.seats[i].alive)
        .collect()
}

pub fn alive_seats(g: &Game) -> Vec<usize> {
    g.alive_seats()
}

pub fn seat_name(g: &Game, seat: usize) -> String {
    g.seats
        .get(seat)
        .map(|s| s.name.clone())
        .unwrap_or_else(|| format!("座位{seat}"))
}

/// 触发类可选技能的统一「发动/放弃」询问（首帧；`Step::Need` 不改 data）。
pub fn trigger_ask(actor: usize, skill: &'static str, label: &str, data: Value) -> Step {
    Step::Need(
        PendingBuilder::new(actor, "choose")
            .skill(skill)
            .option("use", label)
            .escape("不发动")
            .data(data)
            .build(),
    )
}

/// 强制选牌询问（**禁止**用 `cands.first()` 兜底代选，constitution I1）。
pub fn must_pick(
    g: &Game,
    actor: usize,
    skill: &'static str,
    kind: &'static str,
    verb: &str,
    cands: &[CardId],
    data: Value,
) -> Step {
    let mut b = PendingBuilder::new(actor, kind).skill(skill);
    for id in cands {
        b = b.option(format!("card:{id}"), format!("{verb}【{}】", card_label(g, id)));
    }
    Step::Need(b.escape_mandatory("（必选一张）").data(data).build())
}

/// 选座位询问（可选出口）。
pub fn pick_seat(
    g: &Game,
    actor: usize,
    skill: &'static str,
    verb: &str,
    cands: &[usize],
    data: Value,
) -> Step {
    let mut b = PendingBuilder::new(actor, "target").skill(skill);
    for s in cands {
        b = b.option(format!("t:{s}"), format!("{verb}{}", seat_name(g, *s)));
    }
    Step::Need(b.escape("取消").data(data).build())
}

/// 从 a 座位的牌（手牌+装备）里选一张（由 `actor` 选，允许是他人）。
pub fn pick_from_seat(
    g: &Game,
    actor: usize,
    owner: usize,
    skill: &'static str,
    verb: &str,
    data: Value,
) -> Option<Step> {
    let cands = target_cards(g, owner);
    if cands.is_empty() {
        return None;
    }
    let mut b = PendingBuilder::new(actor, "pick_other").skill(skill);
    for id in &cands {
        b = b.option(
            format!("card:{id}"),
            format!("{verb}{}的【{}】", seat_name(g, owner), card_label(g, id)),
        );
    }
    Some(Step::Need(b.escape_mandatory("（必选一张）").data(data).build()))
}
