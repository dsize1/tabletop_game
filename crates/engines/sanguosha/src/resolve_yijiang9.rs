//! P4.4.2 一将成名（批次 Y7：8 将）技能帧。
//!
//! 覆盖：忠鉴/才识（辛宪英）、福绵/怠宴（吴苋）、问卦/伏诛（徐氏）、守玺/惠民（曹节）、
//! 清弦/绝响（嵇康）、谏征/专对/天辩（秦宓）、复难/诫训（薛综）、辟撰/通博（蔡邕）。

use crate::resolve::{pop, push_discard, replace_top, top, DamageType, Frame, Outcome};
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

fn card_def_code(st: &SgsState, card: &str) -> String {
    st.cards
        .get(card)
        .map(|c| c.def.clone())
        .unwrap_or_default()
}

fn is_equip(st: &SgsState, card: &str) -> bool {
    st.cards
        .get(card)
        .and_then(|c| crate::data::card_def(&c.def))
        .is_some_and(|d| d.equip_slot.is_some())
}

fn random_use_equip(st: &mut SgsState, seat: usize, reason: &str) -> Option<String> {
    let card = st.seats[seat]
        .hand
        .iter()
        .find(|id| is_equip(st, id))
        .cloned()?;
    let slot = st
        .cards
        .get(&card)
        .and_then(|c| crate::data::card_def(&c.def))
        .and_then(|d| d.equip_slot)?;
    if st.slot_abolished(seat, slot) {
        return None;
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
    Some(card)
}

fn draw_one_from_bottom(st: &mut SgsState) -> Option<String> {
    turn::draw_one_bottom_public(st)
}

// ---------------------------------------------------------------------------
// 忠鉴（辛宪英）：展示一张手牌与目标体力值等量手牌。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZhongJianStage {
    Card,
    Target,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZhongJianFrame {
    pub actor: usize,
    pub stage: ZhongJianStage,
    pub card: Option<String>,
    pub target: Option<usize>,
}

pub(crate) fn step_zhongjian(st: &mut SgsState, f: ZhongJianFrame) -> Result<bool, EngineError> {
    let (options, actor) = match f.stage {
        ZhongJianStage::Card => (
            turn::hand_ids(st, f.actor)
                .into_iter()
                .map(|id| PendingOption {
                    id: format!("choose:{id}"),
                    label: format!("展示 {}", turn::card_label(st, &id)),
                })
                .collect(),
            f.actor,
        ),
        ZhongJianStage::Target => (
            st.alive_indices()
                .into_iter()
                .filter(|i| *i != f.actor && !st.seats[*i].hand.is_empty())
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("查看 {} 手牌", st.seats[i].general_name),
                })
                .collect(),
            f.actor,
        ),
    };
    let (options, actor): (Vec<PendingOption>, usize) = (options, actor);
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, actor),
        skill: Some("zhongjian".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::ZhongJian(f));
    Ok(true)
}

pub(crate) fn act_zhongjian(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ZhongJian(f)) => f,
        _ => return Err(internal("栈顶不是忠鉴帧")),
    };
    match f.stage {
        ZhongJianStage::Card => {
            let card = action
                .strip_prefix("choose:")
                .ok_or_else(|| illegal("需要展示一张手牌"))?;
            if !st.seats[f.actor].hand.iter().any(|c| c == card) {
                return Err(illegal("手牌中没有该牌"));
            }
            replace_top(
                st,
                Frame::ZhongJian(ZhongJianFrame {
                    stage: ZhongJianStage::Target,
                    card: Some(card.to_string()),
                    ..f
                }),
            );
            Ok("忠鉴：请选择查看的角色".to_string())
        }
        ZhongJianStage::Target => {
            let target_pid = action
                .strip_prefix("target:")
                .ok_or_else(|| illegal("需要选择目标"))?;
            let target = st
                .seat_index(target_pid)
                .ok_or_else(|| illegal("目标不在座"))?;
            let Some(card) = f.card.clone() else {
                return Err(internal("忠鉴缺少展示牌"));
            };
            let n = st.seats[target]
                .hand
                .len()
                .min(st.seats[target].hp.max(0) as usize);
            let revealed: Vec<String> = st.seats[target].hand.iter().take(n).cloned().collect();
            let own = st.cards.get(&card).cloned();
            let own_label = turn::card_label(st, &card);
            let mut same_color = false;
            let mut same_rank = false;
            let mut shown = Vec::new();
            for id in &revealed {
                if let (Some(o), Some(r)) = (own.as_ref(), st.cards.get(id)) {
                    if crate::skills::is_red(o.suit) == crate::skills::is_red(r.suit) {
                        same_color = true;
                    }
                    if o.rank == r.rank {
                        same_rank = true;
                    }
                }
                shown.push(turn::card_label(st, id));
            }
            st.push_log(
                Some(&pid(st, f.actor)),
                format!(
                    "忠鉴：展示【{own_label}】，{} 展示 {} 张：{}",
                    st.seats[target].general_name,
                    revealed.len(),
                    if shown.is_empty() {
                        "（无）".to_string()
                    } else {
                        shown.join("、")
                    }
                ),
            );
            pop(st);
            if same_color {
                let drawn = draw_n(st, f.actor, 1);
                st.push_log(
                    Some(&pid(st, f.actor)),
                    format!("忠鉴：颜色相同，摸 {drawn} 张"),
                );
            }
            if same_rank {
                st.set_counter("zhongjian_extra", 1);
                st.push_log(
                    Some(&pid(st, f.actor)),
                    "忠鉴：点数相同，本回合可再发动一次",
                );
            }
            if !same_color && !same_rank {
                st.set_counter("zhongjian_penalty", 1);
                st.push_log(Some(&pid(st, f.actor)), "忠鉴：皆不包含，本回合手牌上限 -1");
            }
            let used = st.counter("zhongjian_used") + 1;
            st.set_counter("zhongjian_used", used);
            Ok("忠鉴结算完成".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// 才识（辛宪英）：摸牌阶段开始二选一。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CaiShiFrame {
    pub actor: usize,
}

pub(crate) fn step_caishi(st: &mut SgsState, f: CaiShiFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("caishi".into()),
        options: vec![
            PendingOption {
                id: "opt1".into(),
                label: "才识：本回合手牌上限 +1".into(),
            },
            PendingOption {
                id: "opt2".into(),
                label: "才识：回复 1 点体力，本回合不能对自己使用牌".into(),
            },
            PendingOption {
                id: "pass".into(),
                label: "不发动才识".into(),
            },
        ],
        data: json!({}),
    });
    replace_top(st, Frame::CaiShi(f));
    Ok(true)
}

pub(crate) fn act_caishi(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::CaiShi(f)) => f,
        _ => return Err(internal("栈顶不是才识帧")),
    };
    match action {
        "opt1" => {
            st.set_counter("caishi_limit", 1);
            st.push_log(Some(&pid(st, f.actor)), "才识：本回合手牌上限 +1");
        }
        "opt2" => {
            if st.seats[f.actor].hp < st.seats[f.actor].max_hp {
                st.seats[f.actor].hp += 1;
            }
            st.seats[f.actor]
                .flags
                .insert("caishi_no_self".into(), true);
            st.push_log(
                Some(&pid(st, f.actor)),
                "才识：回复 1 点体力，本回合不能对自己使用牌",
            );
        }
        "pass" => {}
        _ => return Err(illegal("需要选择一项")),
    }
    pop(st);
    Ok("才识结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 福绵（吴苋）：准备阶段二选一，切换选项时数值 +1。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FuMianFrame {
    pub actor: usize,
}

fn fumian_value(st: &SgsState, actor: usize, mode: u32) -> u32 {
    let last = st.seats[actor]
        .mark_counts
        .get("fumian_mode")
        .copied()
        .unwrap_or(0);
    let value = st.seats[actor]
        .mark_counts
        .get("fumian_value")
        .copied()
        .unwrap_or(1);
    if last == 0 || last == mode {
        value.max(1)
    } else {
        value + 1
    }
}

pub(crate) fn step_fumian(st: &mut SgsState, f: FuMianFrame) -> Result<bool, EngineError> {
    let v1 = fumian_value(st, f.actor, 1);
    let v2 = fumian_value(st, f.actor, 2);
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("fumian".into()),
        options: vec![
            PendingOption {
                id: "opt1".into(),
                label: format!("福绵①：摸牌阶段多摸 {v1} 张"),
            },
            PendingOption {
                id: "opt2".into(),
                label: format!("福绵②：本回合红色【杀】可多指定 {v2} 个目标（无距离）"),
            },
            PendingOption {
                id: "pass".into(),
                label: "不发动福绵".into(),
            },
        ],
        data: json!({}),
    });
    replace_top(st, Frame::FuMian(f));
    Ok(true)
}

pub(crate) fn act_fumian(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::FuMian(f)) => f,
        _ => return Err(internal("栈顶不是福绵帧")),
    };
    let mode = match action {
        "opt1" => 1u32,
        "opt2" => 2u32,
        "pass" => {
            pop(st);
            return Ok("不发动福绵".to_string());
        }
        _ => return Err(illegal("需要选择一项")),
    };
    let value = fumian_value(st, f.actor, mode);
    if let Some(seat) = st.seats.get_mut(f.actor) {
        seat.mark_counts.insert("fumian_mode".into(), mode);
        seat.mark_counts.insert("fumian_value".into(), value);
    }
    if mode == 1 {
        st.set_counter("fumian_draw", value);
    } else {
        st.set_counter("fumian_multi", value);
    }
    st.push_log(
        Some(&pid(st, f.actor)),
        format!(
            "福绵：选择{}（数值 {value}）",
            if mode == 1 {
                "多摸牌"
            } else {
                "红杀多目标"
            }
        ),
    );
    pop(st);
    Ok("福绵结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 怠宴（吴苋）：结束阶段令一名其他角色获得红桃基本牌；上回合目标则失体力。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DaiYanFrame {
    pub actor: usize,
}

pub(crate) fn step_daiyan(st: &mut SgsState, f: DaiYanFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| *i != f.actor)
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("令 {} 获得牌堆中的红桃基本牌", st.seats[i].general_name),
        })
        .collect();
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动怠宴".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("daiyan".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::DaiYan(f));
    Ok(true)
}

pub(crate) fn act_daiyan(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::DaiYan(f)) => f,
        _ => return Err(internal("栈顶不是怠宴帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动怠宴".to_string());
    }
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择目标"))?;
    let target = st
        .seat_index(target_pid)
        .ok_or_else(|| illegal("目标不在座"))?;
    // 从牌堆找一张红桃基本牌。
    let found = st.deck.draw.iter().position(|id| {
        st.cards.get(id).is_some_and(|c| {
            c.suit == Suit::Heart
                && crate::data::card_def(&c.def).is_some_and(|d| d.category == Category::Basic)
        })
    });
    if let Some(pos) = found {
        let card = st.deck.draw.remove(pos);
        let label = turn::card_label(st, &card);
        st.seats[target].hand.push(card);
        st.push_log(
            Some(&pid(st, f.actor)),
            format!("怠宴：{} 获得【{label}】", st.seats[target].general_name),
        );
        cap_hand(st, target);
    } else {
        st.push_log(Some(&pid(st, f.actor)), "怠宴：牌堆中没有红桃基本牌");
    }
    let last = st.seats[f.actor]
        .mark_counts
        .get("daiyan_target")
        .copied()
        .unwrap_or(0);
    if last == target as u32 + 1 {
        st.seats[target].hp -= 1;
        let hp = st.seats[target].hp;
        st.push_log(
            Some(&pid(st, f.actor)),
            format!(
                "怠宴：上回合也对其发动过，{} 失去 1 点体力（{hp}）",
                st.seats[target].general_name
            ),
        );
        crate::skills::zhaxiang_on_lose_hp(st, target);
        if hp <= 0 {
            crate::resolve::push_dying(st, target, None);
        }
    }
    st.seats[f.actor]
        .mark_counts
        .insert("daiyan_target".into(), target as u32 + 1);
    pop(st);
    Ok("怠宴结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 问卦（徐氏）：当前角色交一张牌给徐氏，徐氏置顶/置底，双方从另一端摸一张。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WenGuaStage {
    Card,
    Choice,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WenGuaFrame {
    pub actor: usize,
    pub owner: usize,
    pub stage: WenGuaStage,
    pub card: Option<String>,
}

pub(crate) fn wengua_owner(st: &SgsState, actor: usize) -> Option<usize> {
    st.alive_indices()
        .into_iter()
        .find(|i| *i != actor && crate::skills::has(st, *i, "wengua"))
}

pub(crate) fn step_wengua(st: &mut SgsState, f: WenGuaFrame) -> Result<bool, EngineError> {
    let (options, actor) = match f.stage {
        WenGuaStage::Card => (
            turn::hand_ids(st, f.actor)
                .into_iter()
                .map(|id| PendingOption {
                    id: format!("choose:{id}"),
                    label: format!("交给徐氏 {}", turn::card_label(st, &id)),
                })
                .collect(),
            f.actor,
        ),
        WenGuaStage::Choice => (
            vec![
                PendingOption {
                    id: "opt1".into(),
                    label: "问卦：置于牌堆顶（双方从牌堆底摸一张）".into(),
                },
                PendingOption {
                    id: "opt2".into(),
                    label: "问卦：置于牌堆底（双方从牌堆顶摸一张）".into(),
                },
            ],
            f.owner,
        ),
    };
    let (options, actor): (Vec<PendingOption>, usize) = (options, actor);
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, actor),
        skill: Some("wengua".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::WenGua(f));
    Ok(true)
}

pub(crate) fn act_wengua(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::WenGua(f)) => f,
        _ => return Err(internal("栈顶不是问卦帧")),
    };
    match f.stage {
        WenGuaStage::Card => {
            let card = action
                .strip_prefix("choose:")
                .ok_or_else(|| illegal("需要选择一张牌"))?;
            if !st.seats[f.actor].hand.iter().any(|c| c == card) {
                return Err(illegal("手牌中没有该牌"));
            }
            replace_top(
                st,
                Frame::WenGua(WenGuaFrame {
                    stage: WenGuaStage::Choice,
                    card: Some(card.to_string()),
                    ..f
                }),
            );
            Ok("问卦：请选择置顶或置底".to_string())
        }
        WenGuaStage::Choice => {
            let Some(card) = f.card.clone() else {
                return Err(internal("问卦缺少牌"));
            };
            if !st.seats[f.actor].hand.iter().any(|c| c == &card) {
                return Err(illegal("该牌已不在手牌"));
            }
            let _ = turn::take_card(st, f.actor, &card);
            let top = action == "opt1";
            if top {
                st.deck.draw.push(card.clone());
            } else {
                st.deck.draw.insert(0, card.clone());
            }
            let label = turn::card_label(st, &card);
            st.push_log(
                Some(&pid(st, f.owner)),
                format!(
                    "问卦：将【{label}】置于牌堆{}",
                    if top { "顶" } else { "底" }
                ),
            );
            let (a, b) = if top {
                let a = draw_one_from_bottom(st);
                let b = draw_one_from_bottom(st);
                (a, b)
            } else {
                let mut ev = Vec::new();
                let a = turn::draw_one(st, &mut ev);
                let b = turn::draw_one(st, &mut ev);
                (a, b)
            };
            if let Some(card) = a {
                st.seats[f.owner].hand.push(card);
            }
            if let Some(card) = b {
                st.seats[f.actor].hand.push(card);
            }
            st.bump_counter(&format!("wengua:{}", f.actor));
            pop(st);
            Ok("问卦结算完成".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// 伏诛（徐氏）：男性角色结束阶段，牌堆 ≤ 体力值×10 时依此对其使用【杀】。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FuZhuFrame {
    pub owner: usize,
    pub current: usize,
}

pub(crate) fn fuzhu_owner(st: &SgsState, current: usize) -> Option<usize> {
    if !crate::skills::is_male(st, current) {
        return None;
    }
    st.alive_indices().into_iter().find(|i| {
        *i != current
            && crate::skills::has(st, *i, "fuzhu")
            && st.deck.draw.len() <= st.seats[*i].hp.max(0) as usize * 10
    })
}

pub(crate) fn step_fuzhu(st: &mut SgsState, f: FuZhuFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("fuzhu".into()),
        options: vec![
            PendingOption {
                id: "opt1".into(),
                label: format!(
                    "伏诛：依次对 {} 使用牌堆中的【杀】",
                    st.seats[f.current].general_name
                ),
            },
            PendingOption {
                id: "pass".into(),
                label: "不发动伏诛".into(),
            },
        ],
        data: json!({}),
    });
    replace_top(st, Frame::FuZhu(f));
    Ok(true)
}

pub(crate) fn act_fuzhu(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::FuZhu(f)) => f,
        _ => return Err(internal("栈顶不是伏诛帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动伏诛".to_string());
    }
    if action != "opt1" {
        return Err(illegal("需要选择一项"));
    }
    let limit = st.alive_indices().len();
    let mut cards = Vec::new();
    let mut i = 0;
    while i < st.deck.draw.len() && cards.len() < limit {
        let id = st.deck.draw[i].clone();
        if st
            .cards
            .get(&id)
            .is_some_and(|c| matches!(c.def.as_str(), "sha" | "huo_sha" | "lei_sha"))
        {
            cards.push(id);
            st.deck.draw.remove(i);
        } else {
            i += 1;
        }
    }
    st.push_log(
        Some(&pid(st, f.owner)),
        format!(
            "伏诛：对 {} 依次使用 {} 张【杀】",
            st.seats[f.current].general_name,
            cards.len()
        ),
    );
    for card in &cards {
        let label = turn::card_label(st, card);
        turn::discard_by(st, Some(f.owner), card);
        st.push_log(
            Some(&pid(st, f.owner)),
            format!("伏诛：【{label}】造成 1 点伤害"),
        );
        crate::resolve::apply_damage(st, f.current, 1, Some(f.owner), DamageType::Normal, None);
    }
    let mut deck = std::mem::take(&mut st.deck.draw);
    st.shuffle(&mut deck);
    st.deck.draw = deck;
    st.push_log(Some(&pid(st, f.owner)), "伏诛：洗牌");
    pop(st);
    Ok("伏诛结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 守玺（曹节）：成为【杀】目标后声明牌名，令使用者二选一。
// ---------------------------------------------------------------------------

const SHOUXI_NAMES: [(&str, &str); 10] = [
    ("sha", "杀"),
    ("shan", "闪"),
    ("tao", "桃"),
    ("jiu", "酒"),
    ("guohe_chaiqiao", "过河拆桥"),
    ("shunshou_qianyang", "顺手牵羊"),
    ("wuzhong_shengyou", "无中生有"),
    ("juedou", "决斗"),
    ("nanman_ruqin", "南蛮入侵"),
    ("wanjian_qifa", "万箭齐发"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShouXiStage {
    Declare,
    Choice,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ShouXiFrame {
    pub owner: usize,
    pub user: usize,
    pub sha: String,
    pub stage: ShouXiStage,
    pub name: Option<String>,
    pub need: u32,
    pub no_shan: bool,
    pub ignore_armor: bool,
    pub damage_type: DamageType,
}

pub(crate) fn step_shouxi(st: &mut SgsState, f: ShouXiFrame) -> Result<bool, EngineError> {
    let (options, actor) = match f.stage {
        ShouXiStage::Declare => (
            SHOUXI_NAMES
                .iter()
                .filter(|(code, _)| {
                    !st.seats[f.owner]
                        .flags
                        .contains_key(&format!("shouxi_used:{code}"))
                })
                .map(|(code, name)| PendingOption {
                    id: format!("choose:{code}"),
                    label: format!("声明【{name}】"),
                })
                .collect(),
            f.owner,
        ),
        ShouXiStage::Choice => {
            let name = f.name.clone().unwrap_or_default();
            (
                vec![
                    PendingOption {
                        id: "opt1".into(),
                        label: format!("弃置一张【{}】，并获得其一张牌", card_name_of(&name)),
                    },
                    PendingOption {
                        id: "opt2".into(),
                        label: "令此【杀】对其无效".into(),
                    },
                ],
                f.user,
            )
        }
    };
    let (options, actor): (Vec<PendingOption>, usize) = (options, actor);
    if options.is_empty() {
        pop(st);
        continue_shouxi(st, &f);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, actor),
        skill: Some("shouxi".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::ShouXi(f));
    Ok(true)
}

fn card_name_of(code: &str) -> String {
    SHOUXI_NAMES
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, n)| n.to_string())
        .unwrap_or_else(|| code.to_string())
}

fn continue_shouxi(st: &mut SgsState, f: &ShouXiFrame) {
    crate::resolve_skill::push_sha_target_response(
        st,
        f.user,
        f.owner,
        &f.sha,
        f.need,
        f.no_shan,
        f.ignore_armor,
        f.damage_type,
    );
}

pub(crate) fn act_shouxi(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ShouXi(f)) => f,
        _ => return Err(internal("栈顶不是守玺帧")),
    };
    match f.stage {
        ShouXiStage::Declare => {
            let code = action
                .strip_prefix("choose:")
                .ok_or_else(|| illegal("需要声明一种牌名"))?
                .to_string();
            if !SHOUXI_NAMES.iter().any(|(c, _)| *c == code) {
                return Err(illegal("不支持的牌名"));
            }
            st.seats[f.owner]
                .flags
                .insert(format!("shouxi_used:{code}"), true);
            st.push_log(
                Some(&pid(st, f.owner)),
                format!("守玺：声明【{}】", card_name_of(&code)),
            );
            replace_top(
                st,
                Frame::ShouXi(ShouXiFrame {
                    stage: ShouXiStage::Choice,
                    name: Some(code),
                    ..f
                }),
            );
            Ok("守玺：请使用者选择".to_string())
        }
        ShouXiStage::Choice => {
            if action == "opt1" {
                if let Some(name) = f.name.clone() {
                    let pos = st.seats[f.user]
                        .hand
                        .iter()
                        .position(|id| card_def_code(st, id) == *name);
                    if let Some(pos) = pos {
                        let card = st.seats[f.user].hand.remove(pos);
                        let label = turn::card_label(st, &card);
                        turn::discard_by(st, Some(f.user), &card);
                        st.push_log(Some(&pid(st, f.user)), format!("守玺：弃置【{label}】"));
                        if let Some(taken) =
                            crate::resolve_jiexian::take_one_from_region(st, f.owner)
                        {
                            let tlabel = turn::card_label(st, &taken);
                            st.seats[f.user].hand.push(taken);
                            st.push_log(
                                Some(&pid(st, f.user)),
                                format!("守玺：获得曹节【{tlabel}】，此【杀】照常结算"),
                            );
                        }
                    } else {
                        st.push_log(
                            Some(&pid(st, f.user)),
                            "守玺：没有声明的牌，此【杀】照常结算".to_string(),
                        );
                    }
                }
                pop(st);
                continue_shouxi(st, &f);
                return Ok("守玺结算完成".to_string());
            }
            if action != "opt2" {
                return Err(illegal("需要选择一项"));
            }
            st.push_log(
                Some(&pid(st, f.user)),
                format!("守玺：令此【杀】对 {} 无效", st.seats[f.owner].general_name),
            );
            pop(st);
            st.outcome = Some(Outcome::Respond {
                played: None,
                played_by: None,
            });
            Ok("守玺：此【杀】无效".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// 惠民（曹节）：结束阶段摸 X 张并依次分配。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HuiMinFrame {
    pub owner: usize,
    pub cards: Vec<String>,
}

pub(crate) fn huimin_x(st: &SgsState, _owner: usize) -> u32 {
    st.alive_indices()
        .into_iter()
        .filter(|i| st.seats[*i].hand.len() < st.seats[*i].hp.max(0) as usize)
        .count() as u32
}

pub(crate) fn step_huimin(st: &mut SgsState, f: HuiMinFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("从 {} 开始依次获得", st.seats[i].general_name),
        })
        .collect();
    options.push(PendingOption {
        id: "pass".into(),
        label: "取消分配（牌留给自己）".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("huimin".into()),
        options,
        data: json!({ "cards": f.cards }),
    });
    replace_top(st, Frame::HuiMin(f));
    Ok(true)
}

pub(crate) fn act_huimin(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::HuiMin(f)) => f,
        _ => return Err(internal("栈顶不是惠民帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("惠民：取消分配".to_string());
    }
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择起始角色"))?;
    let start = st
        .seat_index(target_pid)
        .ok_or_else(|| illegal("目标不在座"))?;
    let mut order = vec![start];
    order.extend(st.seats_from_left_of(start));
    let mut cards = f.cards.clone();
    let mut idx = 0;
    for seat in order {
        if cards.is_empty() {
            break;
        }
        if !st.seats[seat].alive {
            continue;
        }
        let card = cards.remove(0);
        let label = turn::card_label(st, &card);
        // 牌本来就在曹节手中，直接转交。
        if let Some(pos) = st.seats[f.owner].hand.iter().position(|c| c == &card) {
            st.seats[f.owner].hand.remove(pos);
        }
        st.seats[seat].hand.push(card);
        st.push_log(
            Some(&pid(st, f.owner)),
            format!("惠民：{} 获得【{label}】", st.seats[seat].general_name),
        );
        cap_hand(st, seat);
        idx += 1;
    }
    pop(st);
    Ok(format!("惠民：分配 {idx} 张"))
}

// ---------------------------------------------------------------------------
// 清弦（嵇康）：受到伤害后令来源二选一。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QingXianFrame {
    pub owner: usize,
    pub target: usize,
}

pub(crate) fn step_qingxian(st: &mut SgsState, f: QingXianFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.target),
        skill: Some("qingxian".into()),
        options: vec![
            PendingOption {
                id: "opt1".into(),
                label: "清弦：失去 1 点体力，随机使用一张装备牌".into(),
            },
            PendingOption {
                id: "opt2".into(),
                label: "清弦：回复 1 点体力，弃置一张装备牌".into(),
            },
        ],
        data: json!({}),
    });
    replace_top(st, Frame::QingXian(f));
    Ok(true)
}

pub(crate) fn act_qingxian(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::QingXian(f)) => f,
        _ => return Err(internal("栈顶不是清弦帧")),
    };
    let target = f.target;
    pop(st);
    let mut club = false;
    if action == "opt1" {
        st.seats[target].hp -= 1;
        let hp = st.seats[target].hp;
        st.push_log(
            Some(&pid(st, target)),
            format!("清弦：失去 1 点体力（{hp}）"),
        );
        crate::skills::zhaxiang_on_lose_hp(st, target);
        if let Some(card) = random_use_equip(st, target, "清弦") {
            club = st.cards.get(&card).is_some_and(|c| c.suit == Suit::Club);
        }
        if hp <= 0 {
            crate::resolve::push_dying(st, target, None);
        }
    } else if action == "opt2" {
        if st.seats[target].hp < st.seats[target].max_hp {
            st.seats[target].hp += 1;
        }
        let hp = st.seats[target].hp;
        st.push_log(
            Some(&pid(st, target)),
            format!("清弦：回复 1 点体力（{hp}）"),
        );
        let equip = EquipSlot::ALL.iter().find_map(|slot| {
            st.seats[target]
                .equips
                .get(slot)
                .cloned()
                .flatten()
                .map(|c| (*slot, c))
        });
        if let Some((slot, card)) = equip {
            st.seats[target].equips.insert(slot, None);
            let label = turn::card_label(st, &card);
            turn::discard_by(st, Some(target), &card);
            club = st.cards.get(&card).is_some_and(|c| c.suit == Suit::Club);
            st.push_log(Some(&pid(st, target)), format!("清弦：弃置装备【{label}】"));
            crate::skills::xiaoji_on_lose_equip(st, target);
        }
    } else {
        return Err(illegal("需要选择一项"));
    }
    if club {
        let drawn = draw_n(st, f.owner, 1);
        st.push_log(
            Some(&pid(st, f.owner)),
            format!("清弦：梅花牌，嵇康摸 {drawn} 张"),
        );
    }
    Ok("清弦结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 绝响（嵇康）：死亡时令一名角色获得技能并受梅花保护。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JueXiangFrame {
    pub dead: usize,
}

pub(crate) fn step_juexiang(st: &mut SgsState, f: JueXiangFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| *i != f.dead)
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("令 {} 获得「清弦」并受梅花保护", st.seats[i].general_name),
        })
        .collect();
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动绝响".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.dead),
        skill: Some("juexiang".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::JueXiang(f));
    Ok(true)
}

pub(crate) fn act_juexiang(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::JueXiang(f)) => f,
        _ => return Err(internal("栈顶不是绝响帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动绝响".to_string());
    }
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择目标"))?;
    let target = st
        .seat_index(target_pid)
        .ok_or_else(|| illegal("目标不在座"))?;
    crate::skills::grant_skill(st, target, "qingxian");
    st.seats[target].flags.insert("juexiang".into(), true);
    st.push_log(
        Some(&pid(st, f.dead)),
        format!(
            "绝响：{} 获得「清弦」，直到其下回合开始不能成为梅花牌目标",
            st.seats[target].general_name
        ),
    );
    pop(st);
    Ok("绝响结算完成".to_string())
}

/// 绝响：梅花牌不能指定受保护角色。
pub(crate) fn juexiang_blocks(st: &SgsState, target: usize, card: Option<&str>) -> bool {
    if !st
        .seats
        .get(target)
        .is_some_and(|s| s.flags.contains_key("juexiang"))
    {
        return false;
    }
    card.and_then(|id| st.cards.get(id))
        .is_some_and(|c| c.suit == Suit::Club)
}

// ---------------------------------------------------------------------------
// 谏征（秦宓）：其他角色【杀】指定目标时，可置于牌堆顶取消目标。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JianZhengFrame {
    pub owner: usize,
    pub user: usize,
    pub target: usize,
    pub card: String,
    pub need: u32,
    pub no_shan: bool,
    pub ignore_armor: bool,
    pub damage_type: DamageType,
}

pub(crate) fn jianzheng_available(st: &SgsState, user: usize, target: usize) -> Option<usize> {
    if user == target {
        return None;
    }
    st.alive_indices().into_iter().find(|i| {
        *i != user
            && *i != target
            && crate::skills::has(st, *i, "jianzheng")
            && st.in_attack_range(user, *i)
            && !st.seats[*i].hand.is_empty()
    })
}

pub(crate) fn step_jianzheng(st: &mut SgsState, f: JianZhengFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("jianzheng".into()),
        options: vec![
            PendingOption {
                id: "opt1".into(),
                label: "谏征：将一张手牌置于牌堆顶，取消所有目标".into(),
            },
            PendingOption {
                id: "pass".into(),
                label: "不发动谏征".into(),
            },
        ],
        data: json!({}),
    });
    replace_top(st, Frame::JianZheng(f));
    Ok(true)
}

pub(crate) fn act_jianzheng(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::JianZheng(f)) => f,
        _ => return Err(internal("栈顶不是谏征帧")),
    };
    if action == "pass" {
        pop(st);
        crate::resolve_skill::push_sha_target_response(
            st,
            f.user,
            f.target,
            &f.card,
            f.need,
            f.no_shan,
            f.ignore_armor,
            f.damage_type,
        );
        return Ok("不发动谏征".to_string());
    }
    if action != "opt1" {
        return Err(illegal("需要选择是否发动谏征"));
    }
    let Some(card) = st.seats[f.owner].hand.first().cloned() else {
        return Err(illegal("没有手牌"));
    };
    let _ = turn::take_card(st, f.owner, &card);
    st.deck.draw.push(card.clone());
    let label = turn::card_label(st, &card);
    st.push_log(
        Some(&pid(st, f.owner)),
        format!("谏征：将【{label}】置于牌堆顶，取消【杀】的目标"),
    );
    let black = st
        .cards
        .get(&f.card)
        .is_some_and(|c| crate::skills::is_black(c.suit));
    pop(st);
    if black {
        // 黑色【杀】：目标全部取消。
        for frame in st.resolution.iter_mut().rev() {
            if let Frame::Sha(s) = frame {
                s.target = None;
                s.pending_targets.clear();
                s.stage = crate::resolve::ShaStage::WaitRespond;
                break;
            }
        }
        st.outcome = Some(Outcome::Respond {
            played: None,
            played_by: None,
        });
        return Ok("谏征：取消此【杀】的所有目标".to_string());
    }
    // 非黑色【杀】：秦宓成为目标。
    for frame in st.resolution.iter_mut().rev() {
        if let Frame::Sha(s) = frame {
            s.target = Some(f.owner);
            break;
        }
    }
    st.push_log(
        Some(&pid(st, f.owner)),
        "谏征：此【杀】不为黑色，秦宓成为目标",
    );
    crate::resolve_skill::push_sha_target_response(
        st,
        f.user,
        f.owner,
        &f.card,
        f.needs_for(f.owner),
        f.no_shan,
        f.ignore_armor,
        f.damage_type,
    );
    Ok("谏征结算完成".to_string())
}

impl JianZhengFrame {
    fn needs_for(&self, _owner: usize) -> u32 {
        self.need
    }
}

// ---------------------------------------------------------------------------
// 专对（秦宓）：拼点；赢则【杀】不可响应 / 无效。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZhuanDuiKind {
    User,
    Target,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZhuanDuiStage {
    Ask,
    WaitPindian,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZhuanDuiFrame {
    pub owner: usize,
    pub user: usize,
    pub target: usize,
    pub card: String,
    pub kind: ZhuanDuiKind,
    pub stage: ZhuanDuiStage,
    pub need: u32,
    pub no_shan: bool,
    pub ignore_armor: bool,
    pub damage_type: DamageType,
}

pub(crate) fn step_zhuandui(st: &mut SgsState, f: ZhuanDuiFrame) -> Result<bool, EngineError> {
    if f.stage == ZhuanDuiStage::WaitPindian {
        if let Some(Outcome::Pindian { winner, .. }) = st.outcome.take() {
            let won = winner == Some(f.owner);
            match f.kind {
                ZhuanDuiKind::User => {
                    if won {
                        st.push_log(Some(&pid(st, f.owner)), "专对：拼点赢，此【杀】不可响应");
                        for frame in st.resolution.iter_mut().rev() {
                            if let Frame::Sha(s) = frame {
                                s.no_respond = true;
                                break;
                            }
                        }
                    }
                }
                ZhuanDuiKind::Target => {
                    if won {
                        st.push_log(Some(&pid(st, f.owner)), "专对：拼点赢，此【杀】对其无效");
                        pop(st);
                        st.outcome = Some(Outcome::Respond {
                            played: None,
                            played_by: None,
                        });
                        return Ok(false);
                    }
                }
            }
        }
        pop(st);
        if f.kind == ZhuanDuiKind::Target {
            crate::resolve_skill::push_sha_target_response(
                st,
                f.user,
                f.target,
                &f.card,
                f.need,
                f.no_shan,
                f.ignore_armor,
                f.damage_type,
            );
        }
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("zhuandui".into()),
        options: vec![
            PendingOption {
                id: "opt1".into(),
                label: "专对：拼点".into(),
            },
            PendingOption {
                id: "pass".into(),
                label: "不发动专对".into(),
            },
        ],
        data: json!({}),
    });
    replace_top(st, Frame::ZhuanDui(f));
    Ok(true)
}

pub(crate) fn act_zhuandui(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ZhuanDui(f)) => f,
        _ => return Err(internal("栈顶不是专对帧")),
    };
    if action == "pass" {
        pop(st);
        if f.kind == ZhuanDuiKind::Target {
            crate::resolve_skill::push_sha_target_response(
                st,
                f.user,
                f.target,
                &f.card,
                f.need,
                f.no_shan,
                f.ignore_armor,
                f.damage_type,
            );
        }
        return Ok("不发动专对".to_string());
    }
    if action != "opt1" {
        return Err(illegal("需要选择是否拼点"));
    }
    let other = if f.kind == ZhuanDuiKind::User {
        f.target
    } else {
        f.user
    };
    if st.seats[f.owner].hand.is_empty() || st.seats[other].hand.is_empty() {
        return Err(illegal("拼点双方都需要手牌"));
    }
    st.push_log(Some(&pid(st, f.owner)), "专对：拼点");
    replace_top(
        st,
        Frame::ZhuanDui(ZhuanDuiFrame {
            stage: ZhuanDuiStage::WaitPindian,
            ..f
        }),
    );
    crate::resolve_fire::start_pindian(st, f.owner, other, json!({ "skill": "zhuandui" }));
    Ok("专对：请选择拼点手牌".to_string())
}

// ---------------------------------------------------------------------------
// 复难（薛综）：响应牌与使用牌交换。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FuNanFrame {
    pub owner: usize,
    pub responder: usize,
    pub used: String,
    pub respond: String,
}

pub(crate) fn step_funan(st: &mut SgsState, f: FuNanFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("funan".into()),
        options: vec![
            PendingOption {
                id: "opt1".into(),
                label: "复难：令其获得你使用的牌（本回合其不能使用此牌），你获得其响应牌".into(),
            },
            PendingOption {
                id: "pass".into(),
                label: "不发动复难".into(),
            },
        ],
        data: json!({}),
    });
    replace_top(st, Frame::FuNan(f));
    Ok(true)
}

pub(crate) fn act_funan(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::FuNan(f)) => f,
        _ => return Err(internal("栈顶不是复难帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动复难".to_string());
    }
    if action != "opt1" {
        return Err(illegal("需要选择一项"));
    }
    // 从弃牌堆/处理区取回两张牌。
    if let Some(pos) = st.deck.discard.iter().position(|c| c == &f.used) {
        st.deck.discard.remove(pos);
    }
    if let Some(pos) = st.deck.discard.iter().position(|c| c == &f.respond) {
        st.deck.discard.remove(pos);
    }
    st.seats[f.responder].hand.push(f.used.clone());
    st.seats[f.responder]
        .flags
        .insert(format!("funan_no:{}", f.used), true);
    st.seats[f.owner].hand.push(f.respond.clone());
    let used_label = turn::card_label(st, &f.used);
    let respond_label = turn::card_label(st, &f.respond);
    st.push_log(
        Some(&pid(st, f.owner)),
        format!(
            "复难：{} 获得【{used_label}】（本回合不能用），薛综获得【{respond_label}】",
            st.seats[f.responder].general_name
        ),
    );
    cap_hand(st, f.responder);
    cap_hand(st, f.owner);
    pop(st);
    Ok("复难结算完成".to_string())
}

/// 复难：本回合被禁止使用的牌。
pub(crate) fn funan_banned(st: &SgsState, seat: usize, card: &str) -> bool {
    st.seats
        .get(seat)
        .is_some_and(|s| s.flags.contains_key(&format!("funan_no:{card}")))
}

pub(crate) fn clear_funan(st: &mut SgsState, seat: usize) {
    if let Some(s) = st.seats.get_mut(seat) {
        s.flags.retain(|k, _| !k.starts_with("funan_no:"));
    }
}

// ---------------------------------------------------------------------------
// 诫训（薛综）：结束阶段令一名角色摸方片数张，然后弃 X 张。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JieXunFrame {
    pub owner: usize,
}

pub(crate) fn diamonds_in_play(st: &SgsState) -> u32 {
    let mut n = 0u32;
    for seat in &st.seats {
        for id in seat
            .hand
            .iter()
            .chain(seat.judge_zone.iter())
            .chain(seat.equips.values().flatten())
            .chain(seat.marks.values().flatten())
        {
            if st.cards.get(id).is_some_and(|c| c.suit == Suit::Diamond) {
                n += 1;
            }
        }
    }
    n
}

pub(crate) fn step_jiexun(st: &mut SgsState, f: JieXunFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| *i != f.owner)
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("对 {} 发动诫训", st.seats[i].general_name),
        })
        .collect();
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动诫训".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("jiexun".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::JieXun(f));
    Ok(true)
}

pub(crate) fn act_jiexun(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::JieXun(f)) => f,
        _ => return Err(internal("栈顶不是诫训帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动诫训".to_string());
    }
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择目标"))?;
    let target = st
        .seat_index(target_pid)
        .ok_or_else(|| illegal("目标不在座"))?;
    let x = diamonds_in_play(st);
    let drawn = draw_n(st, target, x);
    let used = st.counter("jiexun_used");
    st.set_counter("jiexun_used", used + 1);
    st.push_log(
        Some(&pid(st, f.owner)),
        format!(
            "诫训：{} 摸 {drawn} 张方片牌，然后弃 {used} 张",
            st.seats[target].general_name
        ),
    );
    cap_hand(st, target);
    for _ in 0..used {
        if st.seats[target].hand.is_empty() {
            break;
        }
        let n = st.seats[target].hand.len();
        let idx = st.next_index(n);
        let card = st.seats[target].hand.remove(idx);
        let label = turn::card_label(st, &card);
        turn::discard_by(st, Some(target), &card);
        st.push_log(Some(&pid(st, target)), format!("诫训：弃置【{label}】"));
    }
    let empty = st.seats[target].hand.is_empty()
        && st.seats[target].equips.values().all(|c| c.is_none())
        && st.seats[target].judge_zone.is_empty();
    if empty && used > 0 {
        crate::skills::lose_skill(st, f.owner, "jiexun");
        st.seats[f.owner]
            .flags
            .insert("funan_upgraded".into(), true);
        st.push_log(
            Some(&pid(st, f.owner)),
            "诫训：其弃置所有牌，薛综失去诫训并修改复难",
        );
    }
    pop(st);
    Ok("诫训结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 辟撰 / 通博（蔡邕）
// ---------------------------------------------------------------------------

pub(crate) fn bizhuan_add(st: &mut SgsState, seat: usize, reason: &str) {
    if !crate::skills::has(st, seat, "bizhuan") {
        return;
    }
    if crate::skills::mark_cards(st, seat, "shu").len() >= 4 {
        return;
    }
    let mut ev = Vec::new();
    if let Some(card) = turn::draw_one(st, &mut ev) {
        let label = turn::card_label(st, &card);
        crate::skills::add_mark(st, seat, "shu", &card);
        st.push_log(
            Some(&pid(st, seat)),
            format!("辟撰（{reason}）：将【{label}】置于武将牌上为「书」"),
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TongBoStage {
    Swap,
    Shu,
    Give,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TongBoFrame {
    pub owner: usize,
    pub stage: TongBoStage,
    pub card: Option<String>,
}

pub(crate) fn step_tongbo(st: &mut SgsState, f: TongBoFrame) -> Result<bool, EngineError> {
    let (options, actor) = match f.stage {
        TongBoStage::Swap => (
            turn::hand_ids(st, f.owner)
                .into_iter()
                .map(|id| PendingOption {
                    id: format!("choose:{id}"),
                    label: format!("用 {} 替换一张「书」", turn::card_label(st, &id)),
                })
                .collect(),
            f.owner,
        ),
        TongBoStage::Shu => (
            crate::skills::mark_cards(st, f.owner, "shu")
                .into_iter()
                .map(|id| PendingOption {
                    id: format!("choose:{id}"),
                    label: format!("替换「书」{}", turn::card_label(st, &id)),
                })
                .collect(),
            f.owner,
        ),
        TongBoStage::Give => (
            st.alive_indices()
                .into_iter()
                .filter(|i| *i != f.owner)
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("将所有「书」交给 {}", st.seats[i].general_name),
                })
                .collect(),
            f.owner,
        ),
    };
    let (options, actor): (Vec<PendingOption>, usize) = (options, actor);
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, actor),
        skill: Some("tongbo".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::TongBo(f));
    Ok(true)
}

pub(crate) fn act_tongbo(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::TongBo(f)) => f,
        _ => return Err(internal("栈顶不是通博帧")),
    };
    match f.stage {
        TongBoStage::Swap => {
            let card = action
                .strip_prefix("choose:")
                .ok_or_else(|| illegal("需要选择一张手牌"))?;
            if !st.seats[f.owner].hand.iter().any(|c| c == card) {
                return Err(illegal("手牌中没有该牌"));
            }
            replace_top(
                st,
                Frame::TongBo(TongBoFrame {
                    stage: TongBoStage::Shu,
                    card: Some(card.to_string()),
                    ..f
                }),
            );
            Ok("通博：请选择替换的「书」".to_string())
        }
        TongBoStage::Shu => {
            let card = action
                .strip_prefix("choose:")
                .ok_or_else(|| illegal("需要选择一张「书」"))?;
            let Some(hand) = f.card.clone() else {
                return Err(internal("通博缺少手牌"));
            };
            if !crate::skills::mark_cards(st, f.owner, "shu").contains(&card.to_string()) {
                return Err(illegal("武将牌上没有该「书」"));
            }
            let _ = turn::take_card(st, f.owner, &hand);
            crate::skills::remove_mark(st, f.owner, "shu", card);
            crate::skills::add_mark(st, f.owner, "shu", &hand);
            st.seats[f.owner].hand.push(card.to_string());
            let hl = turn::card_label(st, &hand);
            let sl = turn::card_label(st, card);
            st.push_log(
                Some(&pid(st, f.owner)),
                format!("通博：用【{hl}】替换「书」【{sl}】"),
            );
            // 检查「书」是否包含四种花色。
            let mut suits = std::collections::BTreeSet::new();
            for id in crate::skills::mark_cards(st, f.owner, "shu") {
                if let Some(c) = st.cards.get(&id) {
                    suits.insert(match c.suit {
                        Suit::Spade => 1,
                        Suit::Heart => 2,
                        Suit::Club => 3,
                        Suit::Diamond => 4,
                    });
                }
            }
            if suits.len() == 4 {
                replace_top(
                    st,
                    Frame::TongBo(TongBoFrame {
                        stage: TongBoStage::Give,
                        card: None,
                        ..f
                    }),
                );
                return Ok("通博：「书」已含四种花色，请选择交给的角色".to_string());
            }
            pop(st);
            Ok("通博结算完成".to_string())
        }
        TongBoStage::Give => {
            let target_pid = action
                .strip_prefix("target:")
                .ok_or_else(|| illegal("需要选择目标"))?;
            let target = st
                .seat_index(target_pid)
                .ok_or_else(|| illegal("目标不在座"))?;
            let cards = crate::skills::mark_cards(st, f.owner, "shu");
            let n = cards.len();
            for card in &cards {
                crate::skills::remove_mark(st, f.owner, "shu", card);
                st.seats[target].hand.push(card.clone());
            }
            st.push_log(
                Some(&pid(st, f.owner)),
                format!(
                    "通博：将 {n} 张「书」交给 {}",
                    st.seats[target].general_name
                ),
            );
            cap_hand(st, target);
            pop(st);
            Ok("通博结算完成".to_string())
        }
    }
}
