//! P4.4.2 一将成名（批次 Y5：8 将）技能帧。
//!
//! 覆盖：战绝/勤王（刘谌）、樵拾/燕语（夏侯氏）、怃戎/矢志（张嶷）、
//! 宴诛/兴学/诏缚（孙休）、安国（朱治）、邀名（全琮）、怀异（公孙渊）、急攻/饰非（郭图逢纪）。

use crate::resolve::{pop, push, push_discard, replace_top, top, DamageType, Frame};
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

fn cap_hand(st: &mut SgsState, seat: usize) {
    let over = st.seats[seat]
        .hand
        .len()
        .saturating_sub(st.config.max_hand_cards as usize);
    if over > 0 {
        push_discard(st, seat, over, "hand_cap");
    }
}

fn is_sha(st: &SgsState, card: &str) -> bool {
    st.cards
        .get(card)
        .is_some_and(|c| matches!(c.def.as_str(), "sha" | "huo_sha" | "lei_sha"))
}

fn is_shan(st: &SgsState, card: &str) -> bool {
    st.cards.get(card).is_some_and(|c| c.def == "shan")
}

fn min_hand(st: &SgsState) -> usize {
    st.alive_indices()
        .into_iter()
        .map(|i| st.seats[i].hand.len())
        .min()
        .unwrap_or(0)
}

fn min_hp(st: &SgsState) -> i32 {
    st.alive_indices()
        .into_iter()
        .map(|i| st.seats[i].hp)
        .min()
        .unwrap_or(0)
}

fn min_equip(st: &SgsState) -> usize {
    st.alive_indices()
        .into_iter()
        .map(|i| st.seats[i].equips.values().flatten().count())
        .min()
        .unwrap_or(0)
}

fn equip_count(st: &SgsState, seat: usize) -> usize {
    st.seats[seat].equips.values().flatten().count()
}

/// 随机使用一张手牌装备（安国 / 兴改用）。
fn use_random_equip(st: &mut SgsState, seat: usize, reason: &str) -> bool {
    let Some(card) = st.seats[seat]
        .hand
        .iter()
        .find(|id| {
            st.cards
                .get(*id)
                .and_then(|c| crate::data::card_def(&c.def))
                .is_some_and(|d| d.equip_slot.is_some())
        })
        .cloned()
    else {
        return false;
    };
    let slot = st
        .cards
        .get(&card)
        .and_then(|c| crate::data::card_def(&c.def))
        .and_then(|d| d.equip_slot);
    let Some(slot) = slot else {
        return false;
    };
    if st.slot_abolished(seat, slot) {
        return false;
    }
    let _ = turn::take_card(st, seat, &card);
    let old = st.seats[seat].equips.insert(slot, Some(card.clone()));
    if let Some(Some(old_id)) = old {
        turn::to_discard_card(st, &old_id);
        crate::skills::xiaoji_on_lose_equip(st, seat);
    }
    let label = turn::card_label(st, &card);
    st.push_log(
        Some(&pid(st, seat)),
        format!("{reason}：使用装备【{label}】"),
    );
    true
}

// ---------------------------------------------------------------------------
// 战绝（刘谌）：所有手牌当【决斗】；与受伤者各摸一张，摸过 2 张后本阶段失效。
// ---------------------------------------------------------------------------

pub(crate) fn zhanjue_available(st: &SgsState, seat: usize) -> bool {
    crate::skills::has(st, seat, "zhanjue")
        && !st.seats[seat].hand.is_empty()
        && st.counter("zhanjue_draws") < 2
        && st.alive_indices().into_iter().any(|i| i != seat)
}

pub(crate) fn use_zhanjue(st: &mut SgsState, seat: usize) -> Result<String, EngineError> {
    if !zhanjue_available(st, seat) {
        return Err(illegal("战绝不可用"));
    }
    let hand: Vec<String> = st.seats[seat].hand.clone();
    let first = hand[0].clone();
    for card in hand.iter().skip(1) {
        turn::take_card(st, seat, card)?;
        turn::to_discard_card(st, card);
    }
    turn::take_card(st, seat, &first)?;
    st.deck.processing.push(first.clone());
    st.set_counter("zhanjue_active", 1);
    st.virtual_defs.insert(first.clone(), "juedou".to_string());
    crate::resolve::start_virtual_trick(st, seat, &first, "juedou")?;
    st.push_log(Some(&pid(st, seat)), "战绝：将所有手牌当【决斗】使用");
    Ok("战绝：请选择【决斗】目标".to_string())
}

/// 战绝：因该【决斗】受伤后，刘谌与受伤者各摸一张。
pub(crate) fn zhanjue_on_damage(
    st: &mut SgsState,
    source: Option<usize>,
    victim: usize,
    card: &str,
) {
    if st.counter("zhanjue_active") == 0 {
        return;
    }
    let is_juedou = st
        .virtual_defs
        .get(card)
        .map(|c| c == "juedou")
        .unwrap_or(false)
        || st.cards.get(card).is_some_and(|c| c.def == "juedou");
    if !is_juedou {
        return;
    }
    let Some(owner) = st
        .alive_indices()
        .into_iter()
        .find(|i| crate::skills::has(st, *i, "zhanjue"))
    else {
        return;
    };
    if source != Some(owner) && victim != owner {
        return;
    }
    let a = draw_n(st, owner, 1);
    let b = draw_n(st, victim, 1);
    let count = st.counter("zhanjue_draws") + a as u32;
    st.set_counter("zhanjue_draws", count);
    st.push_log(
        Some(&pid(st, owner)),
        format!(
            "战绝：刘谌摸 {a} 张，{} 摸 {b} 张",
            st.seats[victim].general_name
        ),
    );
    cap_hand(st, owner);
    cap_hand(st, victim);
}

/// 勤王：响应激将打出【杀】的角色摸一张。
pub(crate) fn qinwang_on_respond(st: &mut SgsState, responder: usize) {
    if st.counter("qinwang") == 0 || !crate::skills::has(st, responder, "qinwang") {
        // 勤王由刘谌发动；响应者摸牌不要求拥有勤王。
    }
    if st.counter("qinwang") == 0 {
        return;
    }
    let drawn = draw_n(st, responder, 1);
    st.push_log(
        Some(&pid(st, responder)),
        format!("勤王：代出【杀】，摸 {drawn} 张"),
    );
}

// ---------------------------------------------------------------------------
// 樵拾（夏侯氏）：其他角色结束阶段手牌数相等时，各摸一张。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QiaoShiFrame {
    pub owner: usize,
    pub current: usize,
}

pub(crate) fn qiaoshi_owner(st: &SgsState, current: usize) -> Option<usize> {
    st.alive_indices().into_iter().find(|i| {
        *i != current
            && crate::skills::has(st, *i, "qiaoshi")
            && st.seats[*i].hand.len() == st.seats[current].hand.len()
    })
}

pub(crate) fn step_qiaoshi(st: &mut SgsState, f: QiaoShiFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("qiaoshi".into()),
        options: vec![
            PendingOption {
                id: "opt1".into(),
                label: format!(
                    "樵拾：与你各摸一张（双方手牌数相同：{}）",
                    st.seats[f.current].hand.len()
                ),
            },
            PendingOption {
                id: "pass".into(),
                label: "不发动樵拾".into(),
            },
        ],
        data: json!({}),
    });
    replace_top(st, Frame::QiaoShi(f));
    Ok(true)
}

pub(crate) fn act_qiaoshi(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::QiaoShi(f)) => f,
        _ => return Err(internal("栈顶不是樵拾帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动樵拾".to_string());
    }
    if action != "opt1" {
        return Err(illegal("需要选择是否发动樵拾"));
    }
    let a = draw_n(st, f.owner, 1);
    let b = draw_n(st, f.current, 1);
    st.push_log(
        Some(&pid(st, f.owner)),
        format!(
            "樵拾：你摸 {a} 张，{} 摸 {b} 张",
            st.seats[f.current].general_name
        ),
    );
    cap_hand(st, f.owner);
    cap_hand(st, f.current);
    pop(st);
    Ok("樵拾结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 燕语（夏侯氏）：重铸【杀】；本阶段重铸 ≥2 张则出牌阶段结束时可令男性角色摸 2。
// ---------------------------------------------------------------------------

pub(crate) fn recast_sha(
    st: &mut SgsState,
    seat: usize,
    card: &str,
) -> Result<String, EngineError> {
    if !crate::skills::has(st, seat, "yanyu") {
        return Err(illegal("没有该技能"));
    }
    if !is_sha(st, card) {
        return Err(illegal("燕语只能重铸【杀】"));
    }
    let label = turn::card_label(st, card);
    turn::take_card(st, seat, card)?;
    turn::discard_by(st, Some(seat), card);
    let drawn = draw_n(st, seat, 1);
    st.bump_counter("yanyu_count");
    st.push_log(
        Some(&pid(st, seat)),
        format!("燕语：重铸【{label}】，摸 {drawn} 张"),
    );
    Ok("燕语：已重铸".to_string())
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct YanYuFrame {
    pub actor: usize,
}

pub(crate) fn step_yanyu(st: &mut SgsState, f: YanYuFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| *i != f.actor && crate::skills::is_male(st, *i))
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("令 {} 摸两张牌", st.seats[i].general_name),
        })
        .collect();
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动燕语".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("yanyu".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::YanYu(f));
    Ok(true)
}

pub(crate) fn act_yanyu(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::YanYu(f)) => f,
        _ => return Err(internal("栈顶不是燕语帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动燕语".to_string());
    }
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择目标"))?;
    let target = st
        .seat_index(target_pid)
        .ok_or_else(|| illegal("目标不在座"))?;
    let drawn = draw_n(st, target, 2);
    st.push_log(
        Some(&pid(st, f.actor)),
        format!("燕语：令 {} 摸 {drawn} 张", st.seats[target].general_name),
    );
    cap_hand(st, target);
    pop(st);
    Ok("燕语结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 怃戎（张嶷）：双方各展示一张手牌。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WuRongStage {
    Card,
    Target,
    WaitReveal,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WuRongFrame {
    pub actor: usize,
    pub stage: WuRongStage,
    pub card: Option<String>,
    pub target: Option<usize>,
}

pub(crate) fn step_wurong(st: &mut SgsState, f: WuRongFrame) -> Result<bool, EngineError> {
    let (options, actor): (Vec<PendingOption>, usize) = match f.stage {
        WuRongStage::Card => (
            turn::hand_ids(st, f.actor)
                .into_iter()
                .map(|id| PendingOption {
                    id: format!("choose:{id}"),
                    label: format!("展示 {}", turn::card_label(st, &id)),
                })
                .collect(),
            f.actor,
        ),
        WuRongStage::Target => (
            st.alive_indices()
                .into_iter()
                .filter(|i| *i != f.actor && !st.seats[*i].hand.is_empty())
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("与 {} 同时展示", st.seats[i].general_name),
                })
                .collect(),
            f.actor,
        ),
        WuRongStage::WaitReveal => {
            let target = f.target.ok_or_else(|| internal("怃戎缺少目标"))?;
            (
                st.seats[target]
                    .hand
                    .iter()
                    .map(|id| PendingOption {
                        id: format!("choose:{id}"),
                        label: format!("展示 {}", turn::card_label(st, id)),
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
        skill: Some("wurong".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::WuRong(f));
    Ok(true)
}

pub(crate) fn act_wurong(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::WuRong(f)) => f,
        _ => return Err(internal("栈顶不是怃戎帧")),
    };
    match f.stage {
        WuRongStage::Card => {
            let card = action
                .strip_prefix("choose:")
                .ok_or_else(|| illegal("需要展示一张手牌"))?;
            if !st.seats[f.actor].hand.iter().any(|c| c == card) {
                return Err(illegal("手牌中没有该牌"));
            }
            st.set_counter("wurong_used", 1);
            replace_top(
                st,
                Frame::WuRong(WuRongFrame {
                    stage: WuRongStage::Target,
                    card: Some(card.to_string()),
                    ..f
                }),
            );
            Ok("怃戎：请选择目标".to_string())
        }
        WuRongStage::Target => {
            let target_pid = action
                .strip_prefix("target:")
                .ok_or_else(|| illegal("需要选择目标"))?;
            let target = st
                .seat_index(target_pid)
                .ok_or_else(|| illegal("目标不在座"))?;
            st.push_log(
                Some(&pid(st, f.actor)),
                format!("怃戎：与 {} 同时展示手牌", st.seats[target].general_name),
            );
            replace_top(
                st,
                Frame::WuRong(WuRongFrame {
                    stage: WuRongStage::WaitReveal,
                    target: Some(target),
                    ..f
                }),
            );
            Ok("怃戎：请目标展示手牌".to_string())
        }
        WuRongStage::WaitReveal => {
            let target = f.target.ok_or_else(|| internal("怃戎缺少目标"))?;
            let theirs = action
                .strip_prefix("choose:")
                .ok_or_else(|| illegal("需要展示一张手牌"))?;
            if !st.seats[target].hand.iter().any(|c| c == theirs) {
                return Err(illegal("手牌中没有该牌"));
            }
            let Some(own) = f.card.clone() else {
                return Err(internal("怃戎缺少展示牌"));
            };
            let own_sha = is_sha(st, &own);
            let their_shan = is_shan(st, theirs);
            let own_label = turn::card_label(st, &own);
            let their_label = turn::card_label(st, theirs);
            st.push_log(
                Some(&pid(st, f.actor)),
                format!(
                    "怃戎：你展示【{own_label}】，{} 展示【{their_label}】",
                    st.seats[target].general_name
                ),
            );
            pop(st);
            if own_sha && !their_shan {
                turn::take_card(st, f.actor, &own)?;
                turn::discard_by(st, Some(f.actor), &own);
                st.push_log(
                    Some(&pid(st, f.actor)),
                    format!("怃戎：弃置【{own_label}】并造成 1 点伤害"),
                );
                crate::resolve::apply_damage(
                    st,
                    target,
                    1,
                    Some(f.actor),
                    DamageType::Normal,
                    None,
                );
                return Ok("怃戎：造成 1 点伤害".to_string());
            }
            if !own_sha && their_shan {
                turn::take_card(st, f.actor, &own)?;
                turn::discard_by(st, Some(f.actor), &own);
                if let Some(card) = crate::resolve_jiexian::take_one_from_region(st, target) {
                    let label = turn::card_label(st, &card);
                    st.seats[f.actor].hand.push(card);
                    st.push_log(
                        Some(&pid(st, f.actor)),
                        format!("怃戎：弃置【{own_label}】并获得【{label}】"),
                    );
                    cap_hand(st, f.actor);
                }
                return Ok("怃戎：获得其一张牌".to_string());
            }
            Ok("怃戎：均未生效".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// 宴诛（孙休）：目标二选一；兴学随之修改。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum YanZhuStage {
    Target,
    Choice,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct YanZhuFrame {
    pub actor: usize,
    pub stage: YanZhuStage,
    pub target: Option<usize>,
}

pub(crate) fn step_yanzhu(st: &mut SgsState, f: YanZhuFrame) -> Result<bool, EngineError> {
    let (options, actor) = match f.stage {
        YanZhuStage::Target => (
            st.alive_indices()
                .into_iter()
                .filter(|i| *i != f.actor)
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("令 {} 二选一", st.seats[i].general_name),
                })
                .collect(),
            f.actor,
        ),
        YanZhuStage::Choice => {
            let target = f.target.ok_or_else(|| internal("宴诛缺少目标"))?;
            let mut options: Vec<PendingOption> = Vec::new();
            if equip_count(st, target) > 0 {
                options.push(PendingOption {
                    id: "opt1".into(),
                    label: format!(
                        "交出装备区所有牌（{} 张），孙休失去宴诛并修改兴学",
                        equip_count(st, target)
                    ),
                });
            }
            if !st.seats[target].hand.is_empty() {
                options.push(PendingOption {
                    id: "opt2".into(),
                    label: "弃置一张手牌".into(),
                });
            }
            (options, target)
        }
    };
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, actor),
        skill: Some("yanzhu".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::YanZhu(f));
    Ok(true)
}

pub(crate) fn act_yanzhu(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::YanZhu(f)) => f,
        _ => return Err(internal("栈顶不是宴诛帧")),
    };
    match f.stage {
        YanZhuStage::Target => {
            let target_pid = action
                .strip_prefix("target:")
                .ok_or_else(|| illegal("需要选择目标"))?;
            let target = st
                .seat_index(target_pid)
                .ok_or_else(|| illegal("目标不在座"))?;
            replace_top(
                st,
                Frame::YanZhu(YanZhuFrame {
                    stage: YanZhuStage::Choice,
                    target: Some(target),
                    ..f
                }),
            );
            Ok("宴诛：请目标选择".to_string())
        }
        YanZhuStage::Choice => {
            let target = f.target.ok_or_else(|| internal("宴诛缺少目标"))?;
            if action == "opt1" {
                let cards: Vec<String> = EquipSlot::ALL
                    .iter()
                    .filter_map(|slot| st.seats[target].equips.get(slot).cloned().flatten())
                    .collect();
                for slot in EquipSlot::ALL {
                    st.seats[target].equips.insert(slot, None);
                }
                let n = cards.len();
                for card in cards {
                    st.seats[f.actor].hand.push(card);
                }
                crate::skills::lose_skill(st, f.actor, "yanzhu");
                st.seats[f.actor]
                    .flags
                    .insert("xingxue_upgraded".into(), true);
                st.push_log(
                    Some(&pid(st, target)),
                    format!("宴诛：交出 {n} 张装备牌，孙休失去宴诛并修改兴学"),
                );
                cap_hand(st, f.actor);
                pop(st);
                return Ok("宴诛结算完成".to_string());
            }
            if action != "opt2" {
                return Err(illegal("需要选择一项"));
            }
            if !st.seats[target].hand.is_empty() {
                let n = st.seats[target].hand.len();
                let idx = st.next_index(n);
                let card = st.seats[target].hand.remove(idx);
                let label = turn::card_label(st, &card);
                turn::discard_by(st, Some(target), &card);
                st.push_log(
                    Some(&pid(st, target)),
                    format!("宴诛：弃置一张手牌【{label}】"),
                );
            }
            pop(st);
            Ok("宴诛结算完成".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// 兴学（孙休）：结束阶段按体力值令角色摸牌并置顶；修改后只结算一次。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct XingXueFrame {
    pub actor: usize,
    pub remaining: u32,
}

pub(crate) fn step_xingxue(st: &mut SgsState, f: XingXueFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("令 {} 摸一张并置于牌堆顶", st.seats[i].general_name),
        })
        .collect();
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动兴学".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("xingxue".into()),
        options,
        data: json!({ "remaining": f.remaining }),
    });
    replace_top(st, Frame::XingXue(f));
    Ok(true)
}

pub(crate) fn act_xingxue(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::XingXue(f)) => f,
        _ => return Err(internal("栈顶不是兴学帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动兴学".to_string());
    }
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择目标"))?;
    let target = st
        .seat_index(target_pid)
        .ok_or_else(|| illegal("目标不在座"))?;
    let drawn = draw_n(st, target, 1);
    if drawn > 0 && !st.seats[target].hand.is_empty() {
        let n = st.seats[target].hand.len();
        let idx = st.next_index(n);
        let card = st.seats[target].hand.remove(idx);
        st.deck.draw.push(card.clone());
        let label = turn::card_label(st, &card);
        st.push_log(
            Some(&pid(st, f.actor)),
            format!(
                "兴学：{} 摸 {drawn} 张并将【{label}】置于牌堆顶",
                st.seats[target].general_name
            ),
        );
    } else if drawn > 0 {
        st.push_log(
            Some(&pid(st, f.actor)),
            format!("兴学：{} 摸 {drawn} 张", st.seats[target].general_name),
        );
    }
    let remaining = f.remaining.saturating_sub(1);
    if remaining > 0 && !st.seats[f.actor].flags.contains_key("xingxue_upgraded") {
        replace_top(st, Frame::XingXue(XingXueFrame { remaining, ..f }));
    } else {
        pop(st);
    }
    Ok("兴学结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 诏缚（孙休，主公技/限定）：选择至多两名角色，视为在吴势力攻击范围内。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZhaoFuFrame {
    pub actor: usize,
    pub selected: Vec<usize>,
}

pub(crate) fn step_zhaofu(st: &mut SgsState, f: ZhaoFuFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| *i != f.actor)
        .map(|i| {
            let mark = if f.selected.contains(&i) { "✔ " } else { "" };
            PendingOption {
                id: format!("target:{}", st.seats[i].player_id),
                label: format!("{mark}{}", st.seats[i].general_name),
            }
        })
        .collect();
    if !f.selected.is_empty() {
        options.push(PendingOption {
            id: "confirm".into(),
            label: format!("确认（{} 名角色）", f.selected.len()),
        });
    }
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动诏缚".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("zhaofu".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::ZhaoFu(f));
    Ok(true)
}

pub(crate) fn act_zhaofu(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ZhaoFu(f)) => f,
        _ => return Err(internal("栈顶不是诏缚帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动诏缚".to_string());
    }
    if let Some(target_pid) = action.strip_prefix("target:") {
        let target = st
            .seat_index(target_pid)
            .ok_or_else(|| illegal("目标不在座"))?;
        let mut selected = f.selected.clone();
        if let Some(pos) = selected.iter().position(|i| *i == target) {
            selected.remove(pos);
        } else {
            if selected.len() >= 2 {
                return Err(illegal("诏缚最多选择两名角色"));
            }
            selected.push(target);
        }
        replace_top(st, Frame::ZhaoFu(ZhaoFuFrame { selected, ..f }));
        return Ok("已更新诏缚选择".to_string());
    }
    if action != "confirm" || f.selected.is_empty() {
        return Err(illegal("请选择至少 1 名角色"));
    }
    for seat in &f.selected {
        st.seats[*seat].flags.insert("zhaofu".into(), true);
    }
    st.seats[f.actor].flags.insert("zhaofu_used".into(), true);
    st.push_log(
        Some(&pid(st, f.actor)),
        format!(
            "诏缚：{} 视为在所有其他吴势力角色攻击范围内",
            f.selected.len()
        ),
    );
    pop(st);
    Ok("诏缚结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 安国（朱治）：按全场最少的三项条件执行，然后朱治补做未满足项。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AnGuoFrame {
    pub actor: usize,
}

pub(crate) fn step_anguo(st: &mut SgsState, f: AnGuoFrame) -> Result<bool, EngineError> {
    let options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| *i != f.actor)
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("对 {} 发动安国", st.seats[i].general_name),
        })
        .collect();
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("anguo".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::AnGuo(f));
    Ok(true)
}

pub(crate) fn act_anguo(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::AnGuo(f)) => f,
        _ => return Err(internal("栈顶不是安国帧")),
    };
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择目标"))?;
    let target = st
        .seat_index(target_pid)
        .ok_or_else(|| illegal("目标不在座"))?;
    pop(st);
    st.set_counter("anguo_used", 1);
    let (mh, mhp, me) = (min_hand(st), min_hp(st), min_equip(st));
    let t_hand_min = st.seats[target].hand.len() == mh;
    let t_hp_min = st.seats[target].hp == mhp;
    let t_equip_min = equip_count(st, target) == me;
    let mut applied = Vec::new();
    if t_hand_min {
        let drawn = draw_n(st, target, 1);
        st.push_log(
            Some(&pid(st, f.actor)),
            format!("安国：{} 摸 {drawn} 张", st.seats[target].general_name),
        );
        applied.push("hand");
    }
    if t_hp_min {
        if st.seats[target].hp < st.seats[target].max_hp {
            st.seats[target].hp += 1;
        }
        st.push_log(
            Some(&pid(st, f.actor)),
            format!("安国：{} 回复 1 点体力", st.seats[target].general_name),
        );
        applied.push("hp");
    }
    if t_equip_min && use_random_equip(st, target, "安国") {
        applied.push("equip");
    }
    cap_hand(st, target);
    // 朱治补做未满足但自己满足的条件。
    let actor = f.actor;
    if !applied.contains(&"hand") && st.seats[actor].hand.len() == mh {
        let drawn = draw_n(st, actor, 1);
        st.push_log(Some(&pid(st, actor)), format!("安国：朱治摸 {drawn} 张"));
        cap_hand(st, actor);
    }
    if !applied.contains(&"hp")
        && st.seats[actor].hp == mhp
        && st.seats[actor].hp < st.seats[actor].max_hp
    {
        st.seats[actor].hp += 1;
        st.push_log(Some(&pid(st, actor)), "安国：朱治回复 1 点体力");
    }
    if !applied.contains(&"equip") && equip_count(st, actor) == me {
        use_random_equip(st, actor, "安国");
    }
    Ok("安国结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 邀名（全琮）：造成/受到伤害后，每回合限一次。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct YaoMingFrame {
    pub owner: usize,
}

pub(crate) fn yaoming_trigger(st: &mut SgsState, owner: usize) {
    if !crate::skills::has(st, owner, "yaoming") || st.counter("yaoming_used") > 0 {
        return;
    }
    let mine = st.seats[owner].hand.len();
    let has_option = st
        .alive_indices()
        .into_iter()
        .any(|i| i != owner && st.seats[i].hand.len() != mine);
    if !has_option {
        return;
    }
    st.set_counter("yaoming_used", 1);
    push(st, Frame::YaoMing(YaoMingFrame { owner }));
}

pub(crate) fn step_yaoming(st: &mut SgsState, f: YaoMingFrame) -> Result<bool, EngineError> {
    let mine = st.seats[f.owner].hand.len();
    let mut options = Vec::new();
    for i in st.alive_indices() {
        if i == f.owner {
            continue;
        }
        if st.seats[i].hand.len() > mine {
            options.push(PendingOption {
                id: format!("discard:{}", st.seats[i].player_id),
                label: format!(
                    "弃置 {} 一张手牌（{} 张）",
                    st.seats[i].general_name,
                    st.seats[i].hand.len()
                ),
            });
        } else if st.seats[i].hand.len() < mine {
            options.push(PendingOption {
                id: format!("draw:{}", st.seats[i].player_id),
                label: format!(
                    "令 {} 摸一张牌（{} 张）",
                    st.seats[i].general_name,
                    st.seats[i].hand.len()
                ),
            });
        }
    }
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动邀名".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("yaoming".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::YaoMing(f));
    Ok(true)
}

pub(crate) fn act_yaoming(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::YaoMing(f)) => f,
        _ => return Err(internal("栈顶不是邀名帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动邀名".to_string());
    }
    if let Some(pid_str) = action.strip_prefix("discard:") {
        let target = st
            .seat_index(pid_str)
            .ok_or_else(|| illegal("目标不在座"))?;
        if st.seats[target].hand.is_empty() {
            return Err(illegal("目标没有手牌"));
        }
        let n = st.seats[target].hand.len();
        let idx = st.next_index(n);
        let card = st.seats[target].hand.remove(idx);
        let label = turn::card_label(st, &card);
        turn::discard_by(st, Some(target), &card);
        st.push_log(
            Some(&pid(st, f.owner)),
            format!("邀名：弃置 {} 的【{label}】", st.seats[target].general_name),
        );
        pop(st);
        return Ok("邀名结算完成".to_string());
    }
    if let Some(pid_str) = action.strip_prefix("draw:") {
        let target = st
            .seat_index(pid_str)
            .ok_or_else(|| illegal("目标不在座"))?;
        let drawn = draw_n(st, target, 1);
        st.push_log(
            Some(&pid(st, f.owner)),
            format!("邀名：令 {} 摸 {drawn} 张", st.seats[target].general_name),
        );
        cap_hand(st, target);
        pop(st);
        return Ok("邀名结算完成".to_string());
    }
    Err(illegal("需要选择一项"))
}

// ---------------------------------------------------------------------------
// 怀异（公孙渊）：展示手牌，弃一种颜色，按弃置数获得牌；获得 ≥2 失去 1 体力。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HuaiYiStage {
    Color,
    Gain,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HuaiYiFrame {
    pub actor: usize,
    pub stage: HuaiYiStage,
    pub remaining: u32,
    pub gained: u32,
}

pub(crate) fn step_huaiyi(st: &mut SgsState, f: HuaiYiFrame) -> Result<bool, EngineError> {
    let (options, actor) = match f.stage {
        HuaiYiStage::Color => (
            vec![
                PendingOption {
                    id: "opt_red".into(),
                    label: "弃置所有红色手牌".into(),
                },
                PendingOption {
                    id: "opt_black".into(),
                    label: "弃置所有黑色手牌".into(),
                },
                PendingOption {
                    id: "pass".into(),
                    label: "不发动怀异".into(),
                },
            ],
            f.actor,
        ),
        HuaiYiStage::Gain => (
            st.alive_indices()
                .into_iter()
                .filter(|i| turn::has_cards_to_take(st, *i))
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!(
                        "获得 {} 一张牌（还需 {}）",
                        st.seats[i].general_name, f.remaining
                    ),
                })
                .collect(),
            f.actor,
        ),
    };
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, actor),
        skill: Some("huaiyi".into()),
        options,
        data: json!({ "remaining": f.remaining, "gained": f.gained }),
    });
    replace_top(st, Frame::HuaiYi(f));
    Ok(true)
}

pub(crate) fn act_huaiyi(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::HuaiYi(f)) => f,
        _ => return Err(internal("栈顶不是怀异帧")),
    };
    match f.stage {
        HuaiYiStage::Color => {
            if action == "pass" {
                pop(st);
                return Ok("不发动怀异".to_string());
            }
            let red = match action {
                "opt_red" => true,
                "opt_black" => false,
                _ => return Err(illegal("需要选择一种颜色")),
            };
            st.set_counter("huaiyi_used", 1);
            let cards: Vec<String> = st.seats[f.actor]
                .hand
                .iter()
                .filter(|id| {
                    st.cards
                        .get(*id)
                        .is_some_and(|c| crate::skills::is_red(c.suit) == red)
                })
                .cloned()
                .collect();
            let n = cards.len();
            if n == 0 {
                pop(st);
                return Ok("怀异：没有该颜色的手牌".to_string());
            }
            let names: Vec<String> = cards.iter().map(|c| turn::card_label(st, c)).collect();
            for card in &cards {
                turn::take_card(st, f.actor, card)?;
                turn::discard_by(st, Some(f.actor), card);
            }
            st.push_log(
                Some(&pid(st, f.actor)),
                format!(
                    "怀异：展示手牌，弃置 {} 张{}色牌（{}）",
                    n,
                    if red { "红" } else { "黑" },
                    names.join("、")
                ),
            );
            replace_top(
                st,
                Frame::HuaiYi(HuaiYiFrame {
                    stage: HuaiYiStage::Gain,
                    remaining: n as u32,
                    gained: 0,
                    ..f
                }),
            );
            Ok("怀异：请选择获得牌的角色".to_string())
        }
        HuaiYiStage::Gain => {
            if action == "pass" {
                pop(st);
                return finish_huaiyi(st, f);
            }
            let target_pid = action
                .strip_prefix("target:")
                .ok_or_else(|| illegal("需要选择目标"))?;
            let target = st
                .seat_index(target_pid)
                .ok_or_else(|| illegal("目标不在座"))?;
            if let Some(card) = crate::resolve_jiexian::take_one_from_region(st, target) {
                let label = turn::card_label(st, &card);
                st.seats[f.actor].hand.push(card);
                st.push_log(
                    Some(&pid(st, f.actor)),
                    format!("怀异：获得 {} 的【{label}】", st.seats[target].general_name),
                );
                cap_hand(st, f.actor);
            }
            let gained = f.gained + 1;
            let remaining = f.remaining.saturating_sub(1);
            if remaining > 0
                && st
                    .alive_indices()
                    .into_iter()
                    .any(|i| turn::has_cards_to_take(st, i))
            {
                replace_top(
                    st,
                    Frame::HuaiYi(HuaiYiFrame {
                        remaining,
                        gained,
                        ..f
                    }),
                );
                return Ok("怀异：继续选择获得牌".to_string());
            }
            pop(st);
            finish_huaiyi(
                st,
                HuaiYiFrame {
                    remaining,
                    gained,
                    ..f
                },
            )
        }
    }
}

fn finish_huaiyi(st: &mut SgsState, f: HuaiYiFrame) -> Result<String, EngineError> {
    if f.gained >= 2 {
        st.seats[f.actor].hp -= 1;
        let hp = st.seats[f.actor].hp;
        st.push_log(
            Some(&pid(st, f.actor)),
            format!("怀异：获得 {} 张牌，失去 1 点体力（{hp}）", f.gained),
        );
        crate::skills::zhaxiang_on_lose_hp(st, f.actor);
        if hp <= 0 {
            crate::resolve::push_dying(st, f.actor, None);
        }
    }
    Ok("怀异结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 急攻（郭图逢纪）：出牌阶段开始时摸 2；本回合手牌上限=本阶段伤害值。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JiGongFrame {
    pub actor: usize,
}

pub(crate) fn step_jigong(st: &mut SgsState, f: JiGongFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("jigong".into()),
        options: vec![
            PendingOption {
                id: "opt1".into(),
                label: "急攻：摸两张牌，本回合手牌上限等于本阶段造成的伤害值".into(),
            },
            PendingOption {
                id: "pass".into(),
                label: "不发动急攻".into(),
            },
        ],
        data: json!({}),
    });
    replace_top(st, Frame::JiGong(f));
    Ok(true)
}

pub(crate) fn act_jigong(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::JiGong(f)) => f,
        _ => return Err(internal("栈顶不是急攻帧")),
    };
    if action == "opt1" {
        let drawn = draw_n(st, f.actor, 2);
        st.seats[f.actor].flags.insert("jigong".into(), true);
        st.push_log(
            Some(&pid(st, f.actor)),
            format!("急攻：摸 {drawn} 张，本回合手牌上限改为本阶段伤害值"),
        );
    }
    pop(st);
    Ok("急攻结算完成".to_string())
}
