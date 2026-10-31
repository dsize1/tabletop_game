//! P4.4.2 界限突破（批次 J2：8 将）技能帧。
//!
//! 覆盖：界激昂（弃牌回收）、界魂姿（结束阶段二选一）、界制霸（主动拼点）、
//! 设变（翻面移动装备）、飘零（结束阶段判定）、武烈（限定技「烈」标记）、
//! 界涅槃（授予八阵/火计/看破）、界连环（多指定一个目标）。
//! 同名技能 code 复用（ADR-0024），差异按 `skills::is_jiexian` 分支。

use crate::internal;
use crate::resolve::{
    discard_from_processing, pop, push, push_discard, replace_top, top, Frame, Outcome,
};
use crate::skills::{has, is_jiexian};
use crate::state::*;
use crate::turn;
use game_engine_core::EngineError;
use serde_json::json;

fn pid(st: &SgsState, seat: usize) -> String {
    st.seats[seat].player_id.clone()
}

fn pass() -> PendingOption {
    PendingOption {
        id: "pass".into(),
        label: "不发动".into(),
    }
}

fn draw_n(st: &mut SgsState, seat: usize, n: u32) -> usize {
    let mut ev = Vec::new();
    turn::draw_cards(st, seat, n, &mut ev)
}

// ---------------------------------------------------------------------------
// 界激昂（界孙策）：每回合首次【决斗】/红色【杀】因弃置进入弃牌堆 → 可失去 1 点体力获得之
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JiangJieFrame {
    pub owner: usize,
    pub card: String,
}

fn is_jiang_card(st: &SgsState, card: &str) -> bool {
    let Some(inst) = st.cards.get(card) else {
        return false;
    };
    if inst.def == "juedou" {
        return true;
    }
    matches!(inst.def.as_str(), "sha" | "huo_sha" | "lei_sha") && crate::skills::is_red(inst.suit)
}

/// 弃牌钩子：界孙策的激昂（每回合一次）。
pub(crate) fn jiang_jie_on_discard(st: &mut SgsState, card: &str) {
    if st.counter("jiang_jie_used") > 0 || !is_jiang_card(st, card) {
        return;
    }
    let Some(owner) = st
        .alive_indices()
        .into_iter()
        .find(|i| has(st, *i, "jiang") && is_jiexian(st, *i) && st.seats[*i].hp > 0)
    else {
        return;
    };
    st.set_counter("jiang_jie_used", 1);
    push(
        st,
        Frame::JiangJie(JiangJieFrame {
            owner,
            card: card.to_string(),
        }),
    );
}

pub(crate) fn step_jiang_jie(st: &mut SgsState, f: JiangJieFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("jiang_jie".into()),
        options: vec![
            PendingOption {
                id: "choose:jiang_jie".into(),
                label: format!(
                    "激昂：失去 1 点体力获得【{}】",
                    turn::card_label(st, &f.card)
                ),
            },
            pass(),
        ],
        data: json!({ "skill": "jiang_jie" }),
    });
    replace_top(st, Frame::JiangJie(f));
    Ok(true)
}

pub(crate) fn act_jiang_jie(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::JiangJie(f)) => f,
        _ => return Err(internal("栈顶不是激昂帧")),
    };
    if action != "choose:jiang_jie" {
        pop(st);
        return Ok("不发动激昂".to_string());
    }
    pop(st);
    st.deck.discard.retain(|c| c != &f.card);
    st.seats[f.owner].hand.push(f.card.clone());
    st.seats[f.owner].hp -= 1;
    st.push_log(
        Some(&pid(st, f.owner)),
        format!(
            "激昂：失去 1 点体力获得【{}】",
            turn::card_label(st, &f.card)
        ),
    );
    if st.seats[f.owner].hp <= 0 {
        crate::resolve::push_dying(st, f.owner, None);
    }
    Ok("激昂：已获得此牌".to_string())
}

// ---------------------------------------------------------------------------
// 界魂姿（界孙策）：觉醒当回合结束阶段，摸两张或回复 1 点
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HunziEndFrame {
    pub actor: usize,
}

pub(crate) fn step_hunzi_end(st: &mut SgsState, f: HunziEndFrame) -> Result<bool, EngineError> {
    st.set_counter("hunzi_end_asked", 1);
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("hunzi_end".into()),
        options: vec![
            PendingOption {
                id: "choose:draw".into(),
                label: "魂姿：摸两张牌".into(),
            },
            PendingOption {
                id: "choose:heal".into(),
                label: "魂姿：回复 1 点体力".into(),
            },
        ],
        data: json!({ "skill": "hunzi_end" }),
    });
    replace_top(st, Frame::HunziEnd(f));
    Ok(true)
}

pub(crate) fn act_hunzi_end(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::HunziEnd(f)) => f,
        _ => return Err(internal("栈顶不是魂姿帧")),
    };
    pop(st);
    match action {
        "choose:draw" => {
            let drawn = draw_n(st, f.actor, 2);
            st.push_log(Some(&pid(st, f.actor)), format!("魂姿：摸 {drawn} 张"));
            Ok("魂姿：已摸两张".to_string())
        }
        "choose:heal" => {
            st.seats[f.actor].hp = (st.seats[f.actor].hp + 1).min(st.seats[f.actor].max_hp);
            let hp = st.seats[f.actor].hp;
            st.push_log(
                Some(&pid(st, f.actor)),
                format!("魂姿：回复 1 点体力（{hp}）"),
            );
            Ok("魂姿：已回复 1 点".to_string())
        }
        other => Err(crate::illegal(format!("未知魂姿选项：{other}"))),
    }
}

// ---------------------------------------------------------------------------
// 界制霸（界孙策主公）：出牌阶段限一次，主动与一名其他吴势力角色拼点
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZhiBaJieStage {
    Target,
    Wait,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZhiBaJieFrame {
    pub actor: usize,
    pub target: Option<usize>,
    pub stage: ZhiBaJieStage,
}

pub(crate) fn step_zhiba_jie(st: &mut SgsState, f: ZhiBaJieFrame) -> Result<bool, EngineError> {
    match f.stage {
        ZhiBaJieStage::Target => {
            let options: Vec<PendingOption> = st
                .alive_indices()
                .into_iter()
                .filter(|i| {
                    *i != f.actor
                        && crate::skills::kingdom_is(st, *i, "wu")
                        && !st.seats[*i].hand.is_empty()
                })
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("制霸：与 {} 拼点", st.seats[i].general_name),
                })
                .collect();
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: pid(st, f.actor),
                skill: Some("zhiba_jie".into()),
                options,
                data: json!({ "skill": "zhiba_jie", "stage": "target" }),
            });
            replace_top(st, Frame::ZhiBaJie(f));
            Ok(true)
        }
        ZhiBaJieStage::Wait => match st.outcome.take() {
            Some(Outcome::Pindian {
                winner,
                initiator_card,
                target_card,
                ..
            }) => {
                pop(st);
                if winner == Some(f.actor) {
                    let mut got = 0;
                    for id in [&initiator_card, &target_card] {
                        if st.deck.discard.iter().any(|c| c == id) {
                            st.deck.discard.retain(|c| c != id);
                            st.seats[f.actor].hand.push(id.clone());
                            got += 1;
                        }
                    }
                    st.push_log(
                        Some(&pid(st, f.actor)),
                        format!("制霸：拼点赢，获得 {got} 张拼点牌"),
                    );
                } else {
                    st.push_log(Some(&pid(st, f.actor)), "制霸：拼点没赢");
                }
                Ok(false)
            }
            _ => Err(internal("制霸缺少拼点结果")),
        },
    }
}

pub(crate) fn act_zhiba_jie(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ZhiBaJie(f)) => f,
        _ => return Err(internal("栈顶不是制霸帧")),
    };
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| crate::illegal("需要选择其他吴势力角色"))?;
    let target = st
        .seat_index(target_pid)
        .ok_or_else(|| crate::illegal("目标不在座"))?;
    if target == f.actor || !crate::skills::kingdom_is(st, target, "wu") {
        return Err(crate::illegal("制霸只能与其他吴势力角色拼点"));
    }
    st.set_counter("zhiba_jie_used", 1);
    st.push_log(
        Some(&pid(st, f.actor)),
        format!("制霸：与 {} 拼点", st.seats[target].general_name),
    );
    replace_top(
        st,
        Frame::ZhiBaJie(ZhiBaJieFrame {
            actor: f.actor,
            target: Some(target),
            stage: ZhiBaJieStage::Wait,
        }),
    );
    crate::resolve_fire::start_pindian(st, f.actor, target, json!({ "skill": "zhiba_jie" }));
    Ok("制霸：拼点开始".to_string())
}

// ---------------------------------------------------------------------------
// 设变（界夏侯渊）：翻面时，可移动场上的一张装备牌
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SheBianFrame {
    pub owner: usize,
}

fn equip_target_for(st: &SgsState, from: usize, slot: EquipSlot, exclude: usize) -> Option<usize> {
    st.alive_indices()
        .into_iter()
        .find(|i| *i != exclude && st.seats[*i].equips.get(&slot).cloned().flatten().is_none())
        .filter(|i| *i != from)
}

/// 翻面钩子：设变（界夏侯渊）。
pub(crate) fn shebian_on_flip(st: &mut SgsState, seat: usize) {
    if !has(st, seat, "shebian") || st.counter("shebian_used") > 0 {
        return;
    }
    let movable = st.alive_indices().into_iter().any(|i| {
        EquipSlot::ALL.iter().any(|slot| {
            st.seats[i].equips.get(slot).cloned().flatten().is_some()
                && equip_target_for(st, i, *slot, i).is_some()
        })
    });
    if !movable {
        return;
    }
    push(st, Frame::SheBian(SheBianFrame { owner: seat }));
}

pub(crate) fn step_shebian(st: &mut SgsState, f: SheBianFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = Vec::new();
    for i in st.alive_indices() {
        for slot in EquipSlot::ALL {
            let Some(card) = st.seats[i].equips.get(&slot).cloned().flatten() else {
                continue;
            };
            let Some(to) = equip_target_for(st, i, slot, i) else {
                continue;
            };
            options.push(PendingOption {
                id: format!("move:{}:{}", st.seats[i].player_id, slot_name(slot)),
                label: format!(
                    "设变：将 {} 的【{}】移到 {}",
                    st.seats[i].general_name,
                    turn::card_label(st, &card),
                    st.seats[to].general_name
                ),
            });
        }
    }
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    options.push(pass());
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("shebian".into()),
        options,
        data: json!({ "skill": "shebian" }),
    });
    replace_top(st, Frame::SheBian(f));
    Ok(true)
}

fn slot_name(slot: EquipSlot) -> &'static str {
    match slot {
        EquipSlot::Weapon => "weapon",
        EquipSlot::Armor => "armor",
        EquipSlot::HorseOffense => "horse_offense",
        EquipSlot::HorseDefense => "horse_defense",
        EquipSlot::Treasure => "treasure",
    }
}

fn parse_slot(name: &str) -> Option<EquipSlot> {
    Some(match name {
        "weapon" => EquipSlot::Weapon,
        "armor" => EquipSlot::Armor,
        "horse_offense" => EquipSlot::HorseOffense,
        "horse_defense" => EquipSlot::HorseDefense,
        "treasure" => EquipSlot::Treasure,
        _ => return None,
    })
}

pub(crate) fn act_shebian(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::SheBian(f)) => f,
        _ => return Err(internal("栈顶不是设变帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动设变".to_string());
    }
    let rest = action
        .strip_prefix("move:")
        .ok_or_else(|| crate::illegal("需要选择要移动的装备"))?;
    let (owner_pid, slot_name) = rest
        .split_once(':')
        .ok_or_else(|| crate::illegal("设变参数不合法"))?;
    let from = st
        .seat_index(owner_pid)
        .ok_or_else(|| crate::illegal("装备持有者不在座"))?;
    let slot = parse_slot(slot_name).ok_or_else(|| crate::illegal("未知装备槽"))?;
    let Some(card) = st.seats[from].equips.get(&slot).cloned().flatten() else {
        return Err(crate::illegal("该槽位没有装备"));
    };
    let to = equip_target_for(st, from, slot, from)
        .ok_or_else(|| crate::illegal("没有可放置的目标角色"))?;
    st.seats[from].equips.insert(slot, None);
    st.seats[to].equips.insert(slot, Some(card.clone()));
    st.set_counter("shebian_used", 1);
    pop(st);
    st.push_log(
        Some(&pid(st, f.owner)),
        format!(
            "设变：将 {} 的【{}】移到 {}",
            st.seats[from].general_name,
            turn::card_label(st, &card),
            st.seats[to].general_name
        ),
    );
    Ok("设变：已移动装备".to_string())
}

// ---------------------------------------------------------------------------
// 飘零（界小乔）：结束阶段判定，红桃 → 置顶或交给一名角色（自己则弃一张牌）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PiaoLingPlaceFrame {
    pub actor: usize,
    pub card: String,
}

/// 结束阶段触发：推入判定（`QuickJudge` reason = piaoling）。
pub(crate) fn piaoling_trigger(st: &mut SgsState, seat: usize) {
    if !has(st, seat, "piaoling") || st.counter("piaoling_asked") > 0 {
        return;
    }
    st.set_counter("piaoling_asked", 1);
    push(
        st,
        Frame::QuickJudge(crate::resolve::QuickJudgeFrame {
            owner: seat,
            reason: "piaoling".into(),
            stage: crate::resolve::QuickJudgeStage::Reveal,
            judge_card: None,
            rewrite_order: Vec::new(),
            rewrite_cursor: 0,
            data: json!({ "user": seat }),
            saved_outcome: None,
        }),
    );
}

pub(crate) fn step_piaoling_place(
    st: &mut SgsState,
    f: PiaoLingPlaceFrame,
) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("飘零：将判定牌交给 {}", st.seats[i].general_name),
        })
        .collect();
    options.push(PendingOption {
        id: "top".into(),
        label: "飘零：将判定牌置于牌堆顶".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("piaoling".into()),
        options,
        data: json!({ "skill": "piaoling" }),
    });
    replace_top(st, Frame::PiaoLingPlace(f));
    Ok(true)
}

pub(crate) fn act_piaoling_place(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::PiaoLingPlace(f)) => f,
        _ => return Err(internal("栈顶不是飘零帧")),
    };
    if action == "top" {
        pop(st);
        // 判定牌已由判定结算置入弃牌堆/处理区，先摘出再放回牌堆顶。
        st.deck.processing.retain(|c| c != &f.card);
        st.deck.discard.retain(|c| c != &f.card);
        st.deck.draw.push(f.card.clone());
        st.push_log(
            Some(&pid(st, f.actor)),
            format!(
                "飘零：将判定牌【{}】置于牌堆顶",
                turn::card_label(st, &f.card)
            ),
        );
        return Ok("飘零：已置于牌堆顶".to_string());
    }
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| crate::illegal("需要选择一名角色或置顶"))?;
    let target = st
        .seat_index(target_pid)
        .ok_or_else(|| crate::illegal("目标不在座"))?;
    pop(st);
    st.deck.processing.retain(|c| c != &f.card);
    st.deck.discard.retain(|c| c != &f.card);
    st.seats[target].hand.push(f.card.clone());
    st.push_log(
        Some(&pid(st, f.actor)),
        format!(
            "飘零：将判定牌【{}】交给 {}",
            turn::card_label(st, &f.card),
            st.seats[target].general_name
        ),
    );
    if target == f.actor {
        push_discard(st, f.actor, 1, "piaoling");
    }
    Ok("飘零：已交付".to_string())
}

// ---------------------------------------------------------------------------
// 武烈（界孙坚，限定技）：结束阶段失去任意点体力，令等量角色获得「烈」
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WuLieStage {
    Amount,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WuLieFrame {
    pub actor: usize,
    pub stage: WuLieStage,
}

pub(crate) fn step_wulie(st: &mut SgsState, f: WuLieFrame) -> Result<bool, EngineError> {
    let hp = st.seats[f.actor].hp.max(0) as u32;
    let others = st
        .alive_indices()
        .into_iter()
        .filter(|i| *i != f.actor)
        .count() as u32;
    if hp == 0 || others == 0 {
        pop(st);
        return Ok(false);
    }
    let mut options: Vec<PendingOption> = (1..=hp.min(others))
        .map(|n| PendingOption {
            id: format!("choose:wulie:{n}"),
            label: format!("武烈：失去 {n} 点体力，令 {n} 名其他角色获得「烈」"),
        })
        .collect();
    options.push(pass());
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("wulie".into()),
        options,
        data: json!({ "skill": "wulie" }),
    });
    replace_top(st, Frame::WuLie(f));
    Ok(true)
}

pub(crate) fn act_wulie(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::WuLie(f)) => f,
        _ => return Err(internal("栈顶不是武烈帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动武烈".to_string());
    }
    let n: u32 = action
        .strip_prefix("choose:wulie:")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| crate::illegal("未知武烈选项"))?;
    if n == 0 || n as i32 > st.seats[f.actor].hp {
        return Err(crate::illegal("武烈失去体力数不合法"));
    }
    let targets: Vec<usize> = st
        .alive_indices()
        .into_iter()
        .filter(|i| *i != f.actor)
        .take(n as usize)
        .collect();
    st.seats[f.actor].flags.insert("wulie_used".into(), true);
    st.seats[f.actor].hp -= targets.len() as i32;
    for t in &targets {
        *st.seats[*t].mark_counts.entry("lie".into()).or_insert(0) += 1;
    }
    let hp = st.seats[f.actor].hp;
    let names: Vec<String> = targets
        .iter()
        .map(|t| st.seats[*t].general_name.clone())
        .collect();
    pop(st);
    st.push_log(
        Some(&pid(st, f.actor)),
        format!(
            "武烈：失去 {} 点体力（{hp}），令 {} 获得「烈」",
            targets.len(),
            names.join("、")
        ),
    );
    if hp <= 0 {
        crate::resolve::push_dying(st, f.actor, None);
    }
    Ok("武烈：已授予「烈」".to_string())
}

/// 「烈」标记是否存在（受伤害时防止并移除）。
pub fn lie_blocks_damage(st: &mut SgsState, victim: usize) -> bool {
    let Some(seat) = st.seats.get_mut(victim) else {
        return false;
    };
    let cur = seat.mark_counts.get("lie").copied().unwrap_or(0);
    if cur == 0 {
        return false;
    }
    if cur == 1 {
        seat.mark_counts.remove("lie");
    } else {
        seat.mark_counts.insert("lie".into(), cur - 1);
    }
    true
}

// ---------------------------------------------------------------------------
// 界涅槃（界庞统）：结算后获得「八阵」「火计」「看破」中的一个
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NiePanSkillFrame {
    pub actor: usize,
}

pub(crate) fn step_niepan_skill(
    st: &mut SgsState,
    f: NiePanSkillFrame,
) -> Result<bool, EngineError> {
    let options: Vec<PendingOption> = [("bazhen", "八阵"), ("huoji", "火计"), ("kanpo", "看破")]
        .into_iter()
        .map(|(code, name)| PendingOption {
            id: format!("choose:{code}"),
            label: format!("涅槃：获得「{name}」"),
        })
        .collect();
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("niepan_skill".into()),
        options,
        data: json!({ "skill": "niepan_skill" }),
    });
    replace_top(st, Frame::NiePanSkill(f));
    Ok(true)
}

pub(crate) fn act_niepan_skill(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::NiePanSkill(f)) => f,
        _ => return Err(internal("栈顶不是涅槃帧")),
    };
    let code = action
        .strip_prefix("choose:")
        .ok_or_else(|| crate::illegal("需要选择获得的技能"))?;
    if !matches!(code, "bazhen" | "huoji" | "kanpo") {
        return Err(crate::illegal("涅槃只能获得八阵/火计/看破"));
    }
    crate::skills::grant_skill(st, f.actor, code);
    let name = crate::skills::def_of(code).map(|d| d.name).unwrap_or(code);
    pop(st);
    st.push_log(Some(&pid(st, f.actor)), format!("涅槃：获得「{name}」"));
    Ok(format!("涅槃：获得「{name}」"))
}

// ---------------------------------------------------------------------------
// 界连环（界庞统）：使用【铁索连环】可多指定一个目标
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LianHuanPickFrame {
    pub actor: usize,
    pub card: String,
    pub first: Option<usize>,
}

pub(crate) fn step_lianhuan_pick(
    st: &mut SgsState,
    f: LianHuanPickFrame,
) -> Result<bool, EngineError> {
    // 铁索连环的目标在 Trick 帧的 Target 阶段选择（`trick_targets` 不覆盖），此处直接列候选。
    let targets: Vec<usize> = st
        .alive_indices()
        .into_iter()
        .filter(|i| *i != f.actor && !st.seats[*i].judge_abolished)
        .collect();
    let (pool, stage_label, max) = match f.first {
        None => (targets, "请选择第一个目标", false),
        Some(first) => (
            targets.into_iter().filter(|t| *t != first).collect(),
            "可选择第二个目标（多指定一个）",
            true,
        ),
    };
    let mut options: Vec<PendingOption> = pool
        .iter()
        .map(|t| PendingOption {
            id: format!("target:{}", st.seats[*t].player_id),
            label: format!("铁索连环：{}", st.seats[*t].general_name),
        })
        .collect();
    if options.is_empty() {
        let Some(first) = f.first else {
            pop(st);
            return Ok(false);
        };
        pop(st);
        crate::resolve::start_virtual_trick_targets(
            st,
            f.actor,
            &f.card,
            "tiesuo_lianhuan",
            vec![first],
        )?;
        return Ok(false);
    }
    if max {
        options.push(PendingOption {
            id: "keep".into(),
            label: "就选一个目标".into(),
        });
    } else {
        options.push(pass());
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("lianhuan_pick".into()),
        options,
        data: json!({ "skill": "lianhuan_pick", "hint": stage_label }),
    });
    replace_top(st, Frame::LianHuanPick(f));
    Ok(true)
}

pub(crate) fn act_lianhuan_pick(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::LianHuanPick(f)) => f,
        _ => return Err(internal("栈顶不是铁索连环帧")),
    };
    let mut targets: Vec<usize> = Vec::new();
    if let Some(first) = f.first {
        targets.push(first);
    }
    let chosen = if action == "keep" || action == "pass" {
        None
    } else {
        let target_pid = action
            .strip_prefix("target:")
            .ok_or_else(|| crate::illegal("需要选择目标"))?;
        Some(
            st.seat_index(target_pid)
                .ok_or_else(|| crate::illegal("目标不在座"))?,
        )
    };
    match (f.first, chosen) {
        (None, None) => {
            pop(st);
            discard_from_processing(st, &f.card);
            return Ok("取消铁索连环".to_string());
        }
        (None, Some(t)) => {
            replace_top(
                st,
                Frame::LianHuanPick(LianHuanPickFrame {
                    actor: f.actor,
                    card: f.card,
                    first: Some(t),
                }),
            );
            return Ok("铁索连环：请选择第二个目标（或就选一个）".to_string());
        }
        (Some(_), opt) => {
            if let Some(t) = opt {
                targets.push(t);
            }
            pop(st);
            crate::resolve::start_virtual_trick_targets(
                st,
                f.actor,
                &f.card,
                "tiesuo_lianhuan",
                targets,
            )?;
            Ok("铁索连环：已选择目标".to_string())
        }
    }
}
