//! P4.4.2 一将成名（批次 Y1：8 将）技能帧。
//!
//! 覆盖：称象/仁心（曹冲）、自立/排异（钟会）、自守（刘表）、
//! 峻刑/御策（满宠）、龙吟（关平）、巧说（简雍）。

use crate::resolve::{pop, replace_top, top, DamageType, Frame, Outcome};
use crate::state::*;
use crate::turn;
use crate::{illegal, internal};
use game_engine_core::EngineError;
use serde_json::json;

fn pid(st: &SgsState, seat: usize) -> String {
    st.seats[seat].player_id.clone()
}

fn draw_n(st: &mut SgsState, seat: usize, n: u32) -> usize {
    let mut ev = Vec::new();
    turn::draw_cards(st, seat, n, &mut ev)
}

/// 牌的类型（峻刑/御策/精策口径：基本 / 锦囊 / 装备）。
pub(crate) fn card_type_bit(st: &SgsState, card_id: &str) -> u32 {
    match st
        .cards
        .get(card_id)
        .and_then(|c| crate::data::card_def(&c.def))
        .map(|d| d.category)
    {
        Some(Category::Basic) => 1,
        Some(Category::Trick | Category::DelayedTrick) => 2,
        Some(Category::Weapon | Category::Armor | Category::Horse | Category::Treasure) => 4,
        None => 0,
    }
}

fn cap_hand(st: &mut SgsState, seat: usize) {
    let over = st.seats[seat]
        .hand
        .len()
        .saturating_sub(st.config.max_hand_cards as usize);
    if over > 0 {
        crate::resolve::push_discard(st, seat, over, "hand_cap");
    }
}

// ---------------------------------------------------------------------------
// 称象（曹冲）：受到伤害后亮出牌堆顶 4 张，获得点数和 ≤13 的任意张。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChengXiangFrame {
    pub actor: usize,
    /// 亮出的牌（暂存于处理区 `deck.processing`）。
    pub cards: Vec<String>,
    pub selected: Vec<String>,
}

pub(crate) fn start_chengxiang(st: &mut SgsState, actor: usize) {
    let mut ev = Vec::new();
    let mut cards = Vec::new();
    for _ in 0..4 {
        match turn::draw_one(st, &mut ev) {
            Some(card) => {
                st.deck.processing.push(card.clone());
                cards.push(card);
            }
            None => break,
        }
    }
    if cards.is_empty() {
        return;
    }
    let labels: Vec<String> = cards
        .iter()
        .map(|id| {
            let label = turn::card_label(st, id);
            let rank = st.cards.get(id).map(|c| c.rank).unwrap_or(0);
            format!("{label}({rank})")
        })
        .collect();
    st.push_log(
        Some(&pid(st, actor)),
        format!("称象：亮出牌堆顶 {} 张：{}", cards.len(), labels.join("、")),
    );
    crate::resolve::push(
        st,
        Frame::ChengXiang(ChengXiangFrame {
            actor,
            cards,
            selected: Vec::new(),
        }),
    );
}

fn chengxiang_sum(st: &SgsState, cards: &[String]) -> u32 {
    cards
        .iter()
        .filter_map(|id| st.cards.get(id).map(|c| c.rank as u32))
        .sum()
}

pub(crate) fn step_chengxiang(st: &mut SgsState, f: ChengXiangFrame) -> Result<bool, EngineError> {
    let sum = chengxiang_sum(st, &f.selected);
    let mut options: Vec<PendingOption> = Vec::new();
    for id in &f.cards {
        let rank = st.cards.get(id).map(|c| c.rank as u32).unwrap_or(0);
        let picked = f.selected.contains(id);
        let label = turn::card_label(st, id);
        if picked {
            options.push(PendingOption {
                id: format!("choose:{id}"),
                label: format!("✔ {label}({rank})（点数和 {sum}）"),
            });
        } else if sum + rank <= 13 {
            options.push(PendingOption {
                id: format!("choose:{id}"),
                label: format!("{label}({rank})"),
            });
        }
    }
    if !f.selected.is_empty() {
        options.push(PendingOption {
            id: "confirm".into(),
            label: format!("确认获得（点数和 {sum}）"),
        });
    }
    options.push(PendingOption {
        id: "pass".into(),
        label: "不获得任何牌".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("chengxiang".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::ChengXiang(f));
    Ok(true)
}

pub(crate) fn act_chengxiang(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ChengXiang(f)) => f,
        _ => return Err(internal("栈顶不是称象帧")),
    };
    if let Some(card) = action.strip_prefix("choose:") {
        if !f.cards.iter().any(|c| c == card) {
            return Err(illegal("该牌不在亮出的牌中"));
        }
        let mut selected = f.selected.clone();
        if let Some(pos) = selected.iter().position(|c| c == card) {
            selected.remove(pos);
        } else {
            let rank = st.cards.get(card).map(|c| c.rank as u32).unwrap_or(0);
            if chengxiang_sum(st, &selected) + rank > 13 {
                return Err(illegal("点数和不能超过 13"));
            }
            selected.push(card.to_string());
        }
        replace_top(st, Frame::ChengXiang(ChengXiangFrame { selected, ..f }));
        return Ok("已更新称象选择".to_string());
    }
    if action == "pass" {
        for id in &f.cards {
            st.deck.processing.retain(|c| c != id);
            turn::to_discard_card(st, id);
        }
        pop(st);
        return Ok("称象：不获得任何牌".to_string());
    }
    if action != "confirm" || f.selected.is_empty() {
        return Err(illegal("请选择至少 1 张牌"));
    }
    let gained = f.selected.len();
    for id in &f.cards {
        st.deck.processing.retain(|c| c != id);
        if f.selected.contains(id) {
            st.seats[f.actor].hand.push(id.clone());
        } else {
            turn::to_discard_card(st, id);
        }
    }
    st.push_log(Some(&pid(st, f.actor)), format!("称象：获得 {gained} 张牌"));
    cap_hand(st, f.actor);
    pop(st);
    Ok("称象结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 仁心（曹冲）：体力值为 1 的其他角色受到伤害时，弃一张装备牌并翻面防止之。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RenXinFrame {
    pub owner: usize,
    pub victim: usize,
    pub amount: i32,
    pub source: Option<usize>,
    pub damage_type: DamageType,
    pub card: Option<String>,
}

/// 仁心候选：所有拥有「仁心」且有装备牌（手牌/装备区）的存活角色。
pub(crate) fn renxin_owner(st: &SgsState, victim: usize) -> Option<usize> {
    st.alive_indices().into_iter().find(|i| {
        *i != victim
            && crate::skills::has(st, *i, "renxin")
            && (st.seats[*i].hand.iter().any(|id| {
                st.cards
                    .get(id)
                    .and_then(|c| crate::data::card_def(&c.def))
                    .is_some_and(|d| d.equip_slot.is_some())
            }) || st.seats[*i].equips.values().any(|c| c.is_some()))
    })
}

fn renxin_cost_options(st: &SgsState, owner: usize) -> Vec<PendingOption> {
    let mut options = Vec::new();
    for id in turn::hand_ids(st, owner) {
        let is_equip = st
            .cards
            .get(&id)
            .and_then(|c| crate::data::card_def(&c.def))
            .is_some_and(|d| d.equip_slot.is_some());
        if is_equip {
            options.push(PendingOption {
                id: format!("equip:{id}"),
                label: format!("弃置手牌 {}", turn::card_label(st, &id)),
            });
        }
    }
    for slot in EquipSlot::ALL {
        if let Some(Some(id)) = st.seats[owner].equips.get(&slot) {
            options.push(PendingOption {
                id: format!("equip:{id}"),
                label: format!("弃置装备区 {}", turn::card_label(st, id)),
            });
        }
    }
    options
}

pub(crate) fn step_renxin(st: &mut SgsState, f: RenXinFrame) -> Result<bool, EngineError> {
    let mut options = renxin_cost_options(st, f.owner);
    options.push(PendingOption {
        id: "pass".into(),
        label: format!(
            "不发动仁心（{} 受到 {amount} 点伤害）",
            st.seats[f.victim].general_name,
            amount = f.amount
        ),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("renxin".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::RenXin(f));
    Ok(true)
}

pub(crate) fn act_renxin(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::RenXin(f)) => f,
        _ => return Err(internal("栈顶不是仁心帧")),
    };
    if action == "pass" {
        pop(st);
        crate::resolve::apply_damage_now(
            st,
            f.victim,
            f.amount,
            f.source,
            f.damage_type,
            f.card.as_deref(),
        );
        return Ok("不发动仁心".to_string());
    }
    let card = action
        .strip_prefix("equip:")
        .ok_or_else(|| illegal("需要弃置一张装备牌"))?;
    turn::take_card(st, f.owner, card)?;
    let label = turn::card_label(st, card);
    turn::discard_by(st, Some(f.owner), card);
    st.seats[f.owner].flipped = !st.seats[f.owner].flipped;
    crate::resolve_jiexian3::shebian_on_flip(st, f.owner);
    st.push_log(
        Some(&pid(st, f.owner)),
        format!(
            "仁心：弃置【{label}】并翻面，防止 {} 受到的 {amount} 点伤害",
            st.seats[f.victim].general_name,
            amount = f.amount
        ),
    );
    pop(st);
    Ok("仁心：已防止此伤害".to_string())
}

// ---------------------------------------------------------------------------
// 自立（钟会，觉醒）：准备阶段「权」≥3 → 回复 1 或摸 2，减体力上限，获得排异。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZiLiFrame {
    pub actor: usize,
}

pub(crate) fn zili_ready(st: &SgsState, seat: usize) -> bool {
    crate::skills::has(st, seat, "zili")
        && !st.seats[seat].flags.contains_key("zili_done")
        && crate::skills::quan_count(st, seat) >= 3
}

pub(crate) fn zili_awaken(st: &mut SgsState, seat: usize) {
    if zili_ready(st, seat) {
        crate::resolve::push(st, Frame::ZiLi(ZiLiFrame { actor: seat }));
    }
}

pub(crate) fn step_zili(st: &mut SgsState, f: ZiLiFrame) -> Result<bool, EngineError> {
    let heal_ok = st.seats[f.actor].hp < st.seats[f.actor].max_hp;
    let mut options = Vec::new();
    if heal_ok {
        options.push(PendingOption {
            id: "opt1".into(),
            label: "自立：回复 1 点体力，然后减 1 点体力上限并获得「排异」".into(),
        });
    }
    options.push(PendingOption {
        id: "opt2".into(),
        label: "自立：摸 2 张牌，然后减 1 点体力上限并获得「排异」".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("zili".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::ZiLi(f));
    Ok(true)
}

pub(crate) fn act_zili(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ZiLi(f)) => f,
        _ => return Err(internal("栈顶不是自立帧")),
    };
    let actor = f.actor;
    st.seats[actor].flags.insert("zili_done".into(), true);
    let msg = if action == "opt1" {
        st.seats[actor].hp = (st.seats[actor].hp + 1).min(st.seats[actor].max_hp);
        format!("自立：回复至 {} 点体力", st.seats[actor].hp)
    } else if action == "opt2" {
        let drawn = draw_n(st, actor, 2);
        format!("自立：摸 {drawn} 张")
    } else {
        return Err(illegal("自立：需要选择回复体力或摸牌"));
    };
    st.seats[actor].max_hp = (st.seats[actor].max_hp - 1).max(1);
    st.seats[actor].hp = st.seats[actor].hp.min(st.seats[actor].max_hp);
    crate::skills::grant_skill(st, actor, "paiyi");
    st.push_log(
        Some(&pid(st, actor)),
        format!("{msg}；体力上限减 1，获得「排异」"),
    );
    pop(st);
    Ok("自立觉醒完成".to_string())
}

// ---------------------------------------------------------------------------
// 排异（钟会，觉醒获得）：移去一张「权」，令一名角色摸 2；其手牌数大于你则受 1 伤。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PaiYiFrame {
    pub actor: usize,
}

pub(crate) fn step_paiyi(st: &mut SgsState, f: PaiYiFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("令 {} 摸 2 张牌", st.seats[i].general_name),
        })
        .collect();
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动排异".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("paiyi".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::PaiYi(f));
    Ok(true)
}

pub(crate) fn act_paiyi(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::PaiYi(f)) => f,
        _ => return Err(internal("栈顶不是排异帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动排异".to_string());
    }
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择目标"))?;
    let target = st
        .seat_index(target_pid)
        .ok_or_else(|| illegal("目标不在座"))?;
    let Some(card) = crate::skills::mark_cards(st, f.actor, "quan")
        .into_iter()
        .next()
    else {
        return Err(illegal("没有「权」"));
    };
    crate::skills::remove_mark(st, f.actor, "quan", &card);
    turn::to_discard_card(st, &card);
    st.set_counter("paiyi_used", 1);
    let drawn = draw_n(st, target, 2);
    st.push_log(
        Some(&pid(st, f.actor)),
        format!(
            "排异：移去一张「权」，{} 摸 {drawn} 张",
            st.seats[target].general_name
        ),
    );
    if st.seats[target].hand.len() > st.seats[f.actor].hand.len() {
        st.push_log(
            Some(&pid(st, f.actor)),
            format!(
                "排异：{} 手牌数大于你，对其造成 1 点伤害",
                st.seats[target].general_name
            ),
        );
        crate::resolve::apply_damage(st, target, 1, Some(f.actor), DamageType::Normal, None);
    }
    pop(st);
    Ok("排异结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 自守（刘表）：摸牌阶段可多摸 X（全场势力数），本回合防止对其他角色造成的伤害。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZiShouFrame {
    pub actor: usize,
}

pub(crate) fn step_zishou(st: &mut SgsState, f: ZiShouFrame) -> Result<bool, EngineError> {
    let x = crate::skills::kingdom_count(st);
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("zishou".into()),
        options: vec![
            PendingOption {
                id: "opt1".into(),
                label: format!("自守：多摸 {x} 张牌，本回合防止你造成的伤害"),
            },
            PendingOption {
                id: "pass".into(),
                label: "不发动自守".into(),
            },
        ],
        data: json!({}),
    });
    replace_top(st, Frame::ZiShou(f));
    Ok(true)
}

pub(crate) fn act_zishou(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ZiShou(f)) => f,
        _ => return Err(internal("栈顶不是自守帧")),
    };
    if action == "opt1" {
        let x = crate::skills::kingdom_count(st);
        st.set_counter("zishou_draw", x);
        st.set_counter("zishou", 1);
        st.push_log(
            Some(&pid(st, f.actor)),
            format!("自守：摸牌阶段多摸 {x} 张，本回合防止你造成的伤害"),
        );
        pop(st);
        return Ok("自守：已发动".to_string());
    }
    pop(st);
    Ok("不发动自守".to_string())
}

// ---------------------------------------------------------------------------
// 峻刑（满宠）：弃任意张手牌，令一名其他角色二选一。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JunXingStage {
    Select,
    Target,
    TargetChoice,
    Discard,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JunXingFrame {
    pub actor: usize,
    pub stage: JunXingStage,
    pub selected: Vec<String>,
    pub target: Option<usize>,
    /// 已弃置牌的类型位图（基本 1 / 锦囊 2 / 装备 4）。
    pub types: u32,
}

pub(crate) fn step_junxing(st: &mut SgsState, f: JunXingFrame) -> Result<bool, EngineError> {
    let (options, actor) = match f.stage {
        JunXingStage::Select => {
            let mut options: Vec<PendingOption> = turn::hand_ids(st, f.actor)
                .into_iter()
                .map(|id| {
                    let mark = if f.selected.contains(&id) { "✔ " } else { "" };
                    PendingOption {
                        id: format!("choose:{id}"),
                        label: format!("{mark}{}", turn::card_label(st, &id)),
                    }
                })
                .collect();
            if !f.selected.is_empty() {
                options.push(PendingOption {
                    id: "confirm".into(),
                    label: format!("确认弃置 {} 张", f.selected.len()),
                });
            }
            options.push(PendingOption {
                id: "pass".into(),
                label: "不发动峻刑".into(),
            });
            (options, f.actor)
        }
        JunXingStage::Target => (
            st.alive_indices()
                .into_iter()
                .filter(|i| *i != f.actor && !st.seats[*i].hand.is_empty())
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("令 {} 二选一", st.seats[i].general_name),
                })
                .collect(),
            f.actor,
        ),
        JunXingStage::TargetChoice => {
            let target = f.target.ok_or_else(|| internal("峻刑缺少目标"))?;
            let mut options = Vec::new();
            if st.seats[target]
                .hand
                .iter()
                .any(|id| card_type_bit(st, id) & f.types == 0)
            {
                options.push(PendingOption {
                    id: "opt1".into(),
                    label: "弃置一张与你弃置牌类型皆不同的手牌".into(),
                });
            }
            options.push(PendingOption {
                id: "opt2".into(),
                label: format!("翻面，然后摸 {} 张牌", f.selected.len()),
            });
            (options, target)
        }
        JunXingStage::Discard => {
            let target = f.target.ok_or_else(|| internal("峻刑缺少目标"))?;
            (
                st.seats[target]
                    .hand
                    .iter()
                    .filter(|id| card_type_bit(st, id) & f.types == 0)
                    .map(|id| PendingOption {
                        id: format!("choose:{id}"),
                        label: format!("弃置 {}", turn::card_label(st, id)),
                    })
                    .collect(),
                target,
            )
        }
    };
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, actor),
        skill: Some("junxing".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::JunXing(f));
    Ok(true)
}

pub(crate) fn act_junxing(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::JunXing(f)) => f,
        _ => return Err(internal("栈顶不是峻刑帧")),
    };
    match f.stage {
        JunXingStage::Select => {
            if action == "pass" {
                pop(st);
                return Ok("不发动峻刑".to_string());
            }
            if let Some(card) = action.strip_prefix("choose:") {
                if !st.seats[f.actor].hand.iter().any(|c| c == card) {
                    return Err(illegal("手牌中没有该牌"));
                }
                let mut selected = f.selected.clone();
                if let Some(pos) = selected.iter().position(|c| c == card) {
                    selected.remove(pos);
                } else {
                    selected.push(card.to_string());
                }
                replace_top(st, Frame::JunXing(JunXingFrame { selected, ..f }));
                return Ok("已更新峻刑选择".to_string());
            }
            if action != "confirm" || f.selected.is_empty() {
                return Err(illegal("请选择至少 1 张手牌"));
            }
            let mut types = 0u32;
            for id in &f.selected {
                types |= card_type_bit(st, id);
                turn::take_card(st, f.actor, id)?;
                turn::discard_by(st, Some(f.actor), id);
            }
            st.push_log(
                Some(&pid(st, f.actor)),
                format!("峻刑：弃置 {} 张手牌", f.selected.len()),
            );
            replace_top(
                st,
                Frame::JunXing(JunXingFrame {
                    stage: JunXingStage::Target,
                    types,
                    ..f
                }),
            );
            Ok("峻刑：请选择目标".to_string())
        }
        JunXingStage::Target => {
            if action == "pass" {
                pop(st);
                return Ok("峻刑：无目标，结算取消".to_string());
            }
            let target_pid = action
                .strip_prefix("target:")
                .ok_or_else(|| illegal("需要选择目标"))?;
            let target = st
                .seat_index(target_pid)
                .ok_or_else(|| illegal("目标不在座"))?;
            replace_top(
                st,
                Frame::JunXing(JunXingFrame {
                    stage: JunXingStage::TargetChoice,
                    target: Some(target),
                    ..f
                }),
            );
            Ok("峻刑：请目标选择".to_string())
        }
        JunXingStage::TargetChoice => {
            let target = f.target.ok_or_else(|| internal("峻刑缺少目标"))?;
            if action == "opt1" {
                replace_top(
                    st,
                    Frame::JunXing(JunXingFrame {
                        stage: JunXingStage::Discard,
                        ..f
                    }),
                );
                return Ok("峻刑：请选择要弃置的手牌".to_string());
            }
            if action != "opt2" {
                return Err(illegal("需要选择一项"));
            }
            st.seats[target].flipped = !st.seats[target].flipped;
            crate::resolve_jiexian3::shebian_on_flip(st, target);
            let n = f.selected.len() as u32;
            let drawn = draw_n(st, target, n);
            st.push_log(
                Some(&pid(st, target)),
                format!("峻刑：翻面并摸 {drawn} 张牌"),
            );
            cap_hand(st, target);
            pop(st);
            Ok("峻刑结算完成".to_string())
        }
        JunXingStage::Discard => {
            let target = f.target.ok_or_else(|| internal("峻刑缺少目标"))?;
            let card = action
                .strip_prefix("choose:")
                .ok_or_else(|| illegal("需要选择一张手牌"))?;
            if !st.seats[target].hand.iter().any(|c| c == card) {
                return Err(illegal("手牌中没有该牌"));
            }
            if card_type_bit(st, card) & f.types != 0 {
                return Err(illegal("该牌的类型与已弃置的牌相同"));
            }
            turn::take_card(st, target, card)?;
            let label = turn::card_label(st, card);
            turn::discard_by(st, Some(target), card);
            st.push_log(
                Some(&pid(st, target)),
                format!("峻刑：弃置一张类型不同的手牌【{label}】"),
            );
            pop(st);
            Ok("峻刑结算完成".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// 御策（满宠）：受到伤害后展示一张手牌，令伤害来源二选一。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum YuCeStage {
    Reveal,
    Source,
    Discard,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct YuCeFrame {
    pub actor: usize,
    pub source: usize,
    pub stage: YuCeStage,
    pub card: Option<String>,
}

pub(crate) fn step_yuce(st: &mut SgsState, f: YuCeFrame) -> Result<bool, EngineError> {
    let (options, actor) = match f.stage {
        YuCeStage::Reveal => {
            let mut options: Vec<PendingOption> = turn::hand_ids(st, f.actor)
                .into_iter()
                .map(|id| PendingOption {
                    id: format!("choose:{id}"),
                    label: format!("展示 {}", turn::card_label(st, &id)),
                })
                .collect();
            options.push(PendingOption {
                id: "pass".into(),
                label: "不发动御策".into(),
            });
            (options, f.actor)
        }
        YuCeStage::Source => {
            let shown_type = f.card.as_deref().map(|c| card_type_bit(st, c)).unwrap_or(0);
            let mut options = Vec::new();
            if st.seats[f.source]
                .hand
                .iter()
                .any(|id| card_type_bit(st, id) != shown_type)
            {
                options.push(PendingOption {
                    id: "opt1".into(),
                    label: "弃置一张与展示牌类型不同的手牌".into(),
                });
            }
            options.push(PendingOption {
                id: "opt2".into(),
                label: format!("令 {} 回复 1 点体力", st.seats[f.actor].general_name),
            });
            (options, f.source)
        }
        YuCeStage::Discard => {
            let shown_type = f.card.as_deref().map(|c| card_type_bit(st, c)).unwrap_or(0);
            (
                st.seats[f.source]
                    .hand
                    .iter()
                    .filter(|id| card_type_bit(st, id) != shown_type)
                    .map(|id| PendingOption {
                        id: format!("choose:{id}"),
                        label: format!("弃置 {}", turn::card_label(st, id)),
                    })
                    .collect(),
                f.source,
            )
        }
    };
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, actor),
        skill: Some("yuce".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::YuCe(f));
    Ok(true)
}

pub(crate) fn act_yuce(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::YuCe(f)) => f,
        _ => return Err(internal("栈顶不是御策帧")),
    };
    match f.stage {
        YuCeStage::Reveal => {
            if action == "pass" {
                pop(st);
                return Ok("不发动御策".to_string());
            }
            let card = action
                .strip_prefix("choose:")
                .ok_or_else(|| illegal("需要展示一张手牌"))?;
            if !st.seats[f.actor].hand.iter().any(|c| c == card) {
                return Err(illegal("手牌中没有该牌"));
            }
            let label = turn::card_label(st, card);
            st.push_log(
                Some(&pid(st, f.actor)),
                format!("御策：展示手牌【{label}】"),
            );
            replace_top(
                st,
                Frame::YuCe(YuCeFrame {
                    stage: YuCeStage::Source,
                    card: Some(card.to_string()),
                    ..f
                }),
            );
            Ok("御策：请伤害来源选择".to_string())
        }
        YuCeStage::Source => {
            if action == "opt1" {
                replace_top(
                    st,
                    Frame::YuCe(YuCeFrame {
                        stage: YuCeStage::Discard,
                        ..f
                    }),
                );
                return Ok("御策：请选择要弃置的手牌".to_string());
            }
            if action != "opt2" {
                return Err(illegal("需要选择一项"));
            }
            if st.seats[f.actor].hp < st.seats[f.actor].max_hp {
                st.seats[f.actor].hp += 1;
            }
            let hp = st.seats[f.actor].hp;
            st.push_log(
                Some(&pid(st, f.actor)),
                format!(
                    "御策：{} 回复 1 点体力（体力 {hp}）",
                    st.seats[f.actor].general_name
                ),
            );
            pop(st);
            Ok("御策结算完成".to_string())
        }
        YuCeStage::Discard => {
            let card = action
                .strip_prefix("choose:")
                .ok_or_else(|| illegal("需要选择一张手牌"))?;
            if !st.seats[f.source].hand.iter().any(|c| c == card) {
                return Err(illegal("手牌中没有该牌"));
            }
            let shown_type = f.card.as_deref().map(|c| card_type_bit(st, c)).unwrap_or(0);
            if card_type_bit(st, card) == shown_type {
                return Err(illegal("该牌的类型与展示牌相同"));
            }
            turn::take_card(st, f.source, card)?;
            let label = turn::card_label(st, card);
            turn::discard_by(st, Some(f.source), card);
            st.push_log(
                Some(&pid(st, f.source)),
                format!("御策：弃置一张类型不同的手牌【{label}】"),
            );
            pop(st);
            Ok("御策结算完成".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// 龙吟（关平）：其他/自己于出牌阶段使用【杀】时，弃一张牌令其不计次数。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LongYinFrame {
    pub owner: usize,
    pub user: usize,
    pub card: String,
}

pub(crate) fn step_longyin(st: &mut SgsState, f: LongYinFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = turn::hand_ids(st, f.owner)
        .into_iter()
        .map(|id| PendingOption {
            id: format!("choose:{id}"),
            label: format!("弃置 {}", turn::card_label(st, &id)),
        })
        .collect();
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动龙吟".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("longyin".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::LongYin(f));
    Ok(true)
}

pub(crate) fn act_longyin(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::LongYin(f)) => f,
        _ => return Err(internal("栈顶不是龙吟帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动龙吟".to_string());
    }
    let card = action
        .strip_prefix("choose:")
        .ok_or_else(|| illegal("需要弃置一张牌"))?;
    if !st.seats[f.owner].hand.iter().any(|c| c == card) {
        return Err(illegal("手牌中没有该牌"));
    }
    turn::take_card(st, f.owner, card)?;
    let label = turn::card_label(st, card);
    turn::discard_by(st, Some(f.owner), card);
    let used = st.counter("sha_used");
    st.set_counter("sha_used", used.saturating_sub(1));
    let red = st
        .cards
        .get(&f.card)
        .is_some_and(|c| crate::skills::is_red(c.suit));
    let mut msg = format!("龙吟：弃置【{label}】，此【杀】不计入次数");
    if red {
        let drawn = draw_n(st, f.owner, 1);
        st.push_log(
            Some(&pid(st, f.owner)),
            format!("龙吟：红色【杀】，摸 {drawn} 张"),
        );
        msg.push_str("，摸 1 张");
    }
    st.push_log(Some(&pid(st, f.owner)), msg.clone());
    pop(st);
    Ok(msg)
}

// ---------------------------------------------------------------------------
// 巧说（简雍）：出牌阶段开始时拼点。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QiaoShuoStage {
    Target,
    WaitPindian,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QiaoShuoFrame {
    pub actor: usize,
    pub stage: QiaoShuoStage,
}

pub(crate) fn step_qiaoshuo(st: &mut SgsState, f: QiaoShuoFrame) -> Result<bool, EngineError> {
    if f.stage == QiaoShuoStage::WaitPindian {
        if let Some(Outcome::Pindian {
            winner,
            initiator,
            target,
            ..
        }) = st.outcome.take()
        {
            if winner == Some(f.actor) {
                st.set_counter("qiaoshuo_win", 1);
                st.push_log(
                    Some(&pid(st, f.actor)),
                    "巧说：拼点赢，本回合下一张【杀】可多指定一个目标",
                );
            } else {
                st.set_counter("qiaoshuo_fail", 1);
                st.push_log(
                    Some(&pid(st, f.actor)),
                    format!(
                        "巧说：拼点没赢（{} vs {}），本回合不能使用锦囊牌",
                        st.seats[initiator].general_name, st.seats[target].general_name
                    ),
                );
            }
            pop(st);
            return Ok(false);
        }
        pop(st);
        return Ok(false);
    }
    let mut options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| *i != f.actor && !st.seats[*i].hand.is_empty())
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("与 {} 拼点", st.seats[i].general_name),
        })
        .collect();
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动巧说".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("qiaoshuo".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::QiaoShuo(f));
    Ok(true)
}

pub(crate) fn act_qiaoshuo(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::QiaoShuo(f)) => f,
        _ => return Err(internal("栈顶不是巧说帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动巧说".to_string());
    }
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择拼点目标"))?;
    let target = st
        .seat_index(target_pid)
        .ok_or_else(|| illegal("目标不在座"))?;
    if st.seats[f.actor].hand.is_empty() || st.seats[target].hand.is_empty() {
        return Err(illegal("拼点双方都需要手牌"));
    }
    st.push_log(
        Some(&pid(st, f.actor)),
        format!("巧说：与 {} 拼点", st.seats[target].general_name),
    );
    replace_top(
        st,
        Frame::QiaoShuo(QiaoShuoFrame {
            stage: QiaoShuoStage::WaitPindian,
            ..f
        }),
    );
    crate::resolve_fire::start_pindian(st, f.actor, target, json!({ "skill": "qiaoshuo" }));
    Ok("巧说：请选择拼点手牌".to_string())
}
