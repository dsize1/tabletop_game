//! P4.4.2 一将成名（批次 Y6：8 将）技能帧。
//!
//! 覆盖：矫诏/殚心（郭皇后）、督粮/腹鳞（李严）、匡弼（孙登）、止戈/宗祚（刘虞）、
//! 极奢/链祸（岑昏）、瑰藻/讥谀（孙资刘放）、寝情/贿生（黄皓）、滔乱（张让）。

use crate::resolve::{
    apply_damage_now, pop, push, push_discard, replace_top, top, DamageType, Frame, ShaStage,
};
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

fn card_type_bit(st: &SgsState, card_id: &str) -> u32 {
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

fn suit_bit(suit: Suit) -> u32 {
    match suit {
        Suit::Spade => 1,
        Suit::Heart => 2,
        Suit::Club => 4,
        Suit::Diamond => 8,
    }
}

// ---------------------------------------------------------------------------
// 矫诏（郭皇后）：展示手牌，令最近角色声明基本牌，本回合可将展示牌当之使用。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JiaoZhaoStage {
    Card,
    Declare,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JiaoZhaoFrame {
    pub actor: usize,
    pub stage: JiaoZhaoStage,
    pub card: Option<String>,
    pub declaimer: usize,
}

/// 矫诏的「本回合可用组合」：`flags` 中保存 `jiazhao:<card>:<code>`。
pub(crate) fn jiazhao_code(st: &SgsState, seat: usize, card: &str) -> Option<String> {
    let prefix = format!("jiazhao:{card}:");
    st.seats
        .get(seat)
        .and_then(|s| s.flags.keys().find(|k| k.starts_with(&prefix)).cloned())
        .map(|k| k[prefix.len()..].to_string())
}

pub(crate) fn clear_jiazhao(st: &mut SgsState, seat: usize) {
    if let Some(s) = st.seats.get_mut(seat) {
        s.flags.retain(|k, _| !k.starts_with("jiazhao:"));
    }
}

pub(crate) fn nearest_seat(st: &SgsState, actor: usize) -> Option<usize> {
    st.alive_indices()
        .into_iter()
        .filter(|i| *i != actor)
        .min_by_key(|i| (st.seat_distance(actor, *i), *i))
}

pub(crate) fn step_jiazhao(st: &mut SgsState, f: JiaoZhaoFrame) -> Result<bool, EngineError> {
    let (options, actor) = match f.stage {
        JiaoZhaoStage::Card => (
            turn::hand_ids(st, f.actor)
                .into_iter()
                .map(|id| PendingOption {
                    id: format!("choose:{id}"),
                    label: format!("展示 {}", turn::card_label(st, &id)),
                })
                .collect(),
            f.actor,
        ),
        JiaoZhaoStage::Declare => (
            ["sha", "tao", "jiu", "shan"]
                .iter()
                .map(|code| PendingOption {
                    id: format!("choose:{code}"),
                    label: format!(
                        "声明【{}】",
                        match *code {
                            "sha" => "杀",
                            "tao" => "桃",
                            "jiu" => "酒",
                            _ => "闪",
                        }
                    ),
                })
                .collect(),
            f.declaimer,
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
        skill: Some("jiazhao".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::JiaoZhao(f));
    Ok(true)
}

pub(crate) fn act_jiazhao(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::JiaoZhao(f)) => f,
        _ => return Err(internal("栈顶不是矫诏帧")),
    };
    match f.stage {
        JiaoZhaoStage::Card => {
            let card = action
                .strip_prefix("choose:")
                .ok_or_else(|| illegal("需要展示一张手牌"))?;
            if !st.seats[f.actor].hand.iter().any(|c| c == card) {
                return Err(illegal("手牌中没有该牌"));
            }
            let nearest = nearest_seat(st, f.actor).unwrap_or(f.actor);
            let modified = st.seats[f.actor].flags.contains_key("jiaozhao_modified");
            let declaimer = if modified { f.actor } else { nearest };
            st.set_counter("jiazhao_used", 1);
            let label = turn::card_label(st, card);
            st.push_log(
                Some(&pid(st, f.actor)),
                format!(
                    "矫诏：展示【{label}】，由 {} 声明基本牌",
                    st.seats[declaimer].general_name
                ),
            );
            replace_top(
                st,
                Frame::JiaoZhao(JiaoZhaoFrame {
                    stage: JiaoZhaoStage::Declare,
                    card: Some(card.to_string()),
                    declaimer,
                    ..f
                }),
            );
            Ok("矫诏：请声明一种基本牌".to_string())
        }
        JiaoZhaoStage::Declare => {
            let code = action
                .strip_prefix("choose:")
                .ok_or_else(|| illegal("需要声明一种基本牌"))?;
            if !["sha", "tao", "jiu", "shan"].contains(&code) {
                return Err(illegal("只能声明基本牌"));
            }
            let Some(card) = f.card.clone() else {
                return Err(internal("矫诏缺少展示牌"));
            };
            if !st.seats[f.actor].hand.iter().any(|c| c == &card) {
                return Err(illegal("展示牌已不在手牌"));
            }
            st.seats[f.actor]
                .flags
                .insert(format!("jiazhao:{card}:{code}"), true);
            let name = match code {
                "sha" => "杀",
                "tao" => "桃",
                "jiu" => "酒",
                _ => "闪",
            };
            let label = turn::card_label(st, &card);
            st.push_log(
                Some(&pid(st, f.declaimer)),
                format!("矫诏：声明【{name}】，{label} 本回合可当【{name}】使用"),
            );
            pop(st);
            Ok("矫诏结算完成".to_string())
        }
    }
}

/// 矫诏：使用展示牌当声明的基本牌。
pub(crate) fn use_jiazhao(
    st: &mut SgsState,
    seat: usize,
    card: &str,
) -> Result<String, EngineError> {
    let code = jiazhao_code(st, seat, card).ok_or_else(|| illegal("该牌没有矫诏声明"))?;
    if !st.seats[seat].hand.iter().any(|c| c == card) {
        return Err(illegal("手牌中没有该牌"));
    }
    let label = turn::card_label(st, card);
    match code.as_str() {
        "sha" => {
            if turn::sha_targets(st, seat).is_empty() {
                return Err(illegal("攻击范围内没有目标"));
            }
            if st.counter("sha_used") >= turn::sha_limit(st, seat) {
                return Err(illegal("本出牌阶段已使用过【杀】"));
            }
            if !st
                .seats
                .iter()
                .enumerate()
                .any(|(i, s)| i != seat && s.alive)
            {
                return Err(illegal("不能对自己使用"));
            }
            turn::take_card(st, seat, card)?;
            st.set_counter(&format!("jiazhao_sha:{card}"), 1);
            crate::resolve::start_sha_flags(
                st,
                seat,
                card,
                false,
                DamageType::Normal,
                false,
                false,
            )?;
            st.push_log(
                Some(&pid(st, seat)),
                format!("矫诏：将【{label}】当【杀】使用"),
            );
            Ok("矫诏：请选择【杀】的目标".to_string())
        }
        "tao" => {
            if st.seats[seat].hp >= st.seats[seat].max_hp {
                return Err(illegal("体力已满"));
            }
            turn::take_card(st, seat, card)?;
            turn::to_discard_card(st, card);
            st.seats[seat].hp += 1;
            let hp = st.seats[seat].hp;
            st.push_log(
                Some(&pid(st, seat)),
                format!("矫诏：将【{label}】当【桃】，回复体力至 {hp}"),
            );
            Ok("矫诏：回复 1 点体力".to_string())
        }
        "jiu" => {
            if st.counter("jiu_used") > 0 {
                return Err(illegal("本回合已使用过【酒】"));
            }
            let jiu = st.counter("jiu") + 1;
            st.set_counter("jiu", jiu);
            st.set_counter("jiu_used", 1);
            turn::take_card(st, seat, card)?;
            turn::to_discard_card(st, card);
            st.push_log(Some(&pid(st, seat)), format!("矫诏：将【{label}】当【酒】"));
            Ok("矫诏：已使用【酒】".to_string())
        }
        _ => Err(illegal("【闪】只能在响应时使用（矫诏暂不支持）")),
    }
}

// ---------------------------------------------------------------------------
// 殚心（郭皇后）：受到伤害后摸一张或修改矫诏。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DanXinFrame {
    pub owner: usize,
}

pub(crate) fn step_danxin(st: &mut SgsState, f: DanXinFrame) -> Result<bool, EngineError> {
    let mut options = vec![PendingOption {
        id: "opt1".into(),
        label: "殚心：摸一张牌".into(),
    }];
    if !st.seats[f.owner].flags.contains_key("jiaozhao_modified") {
        options.push(PendingOption {
            id: "opt2".into(),
            label: "殚心：修改矫诏（改由自己声明基本牌）".into(),
        });
    }
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动殚心".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("danxin".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::DanXin(f));
    Ok(true)
}

pub(crate) fn act_danxin(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::DanXin(f)) => f,
        _ => return Err(internal("栈顶不是殚心帧")),
    };
    match action {
        "opt1" => {
            let drawn = draw_n(st, f.owner, 1);
            st.push_log(Some(&pid(st, f.owner)), format!("殚心：摸 {drawn} 张"));
        }
        "opt2" => {
            st.seats[f.owner]
                .flags
                .insert("jiaozhao_modified".into(), true);
            st.push_log(
                Some(&pid(st, f.owner)),
                "殚心：修改「矫诏」（改由自己声明基本牌）",
            );
        }
        "pass" => {}
        _ => return Err(illegal("需要选择一项")),
    }
    pop(st);
    Ok("殚心结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 督粮（李严）：获得一名其他角色一张手牌，然后选一项。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DuLiangStage {
    Target,
    Mode,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DuLiangFrame {
    pub actor: usize,
    pub stage: DuLiangStage,
    pub target: Option<usize>,
}

pub(crate) fn step_duliang(st: &mut SgsState, f: DuLiangFrame) -> Result<bool, EngineError> {
    let (options, actor) = match f.stage {
        DuLiangStage::Target => (
            st.alive_indices()
                .into_iter()
                .filter(|i| *i != f.actor && !st.seats[*i].hand.is_empty())
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("获得 {} 一张手牌", st.seats[i].general_name),
                })
                .collect(),
            f.actor,
        ),
        DuLiangStage::Mode => (
            vec![
                PendingOption {
                    id: "opt1".into(),
                    label: "令其观看牌堆顶两张牌，获得基本牌".into(),
                },
                PendingOption {
                    id: "opt2".into(),
                    label: "令其下回合摸牌阶段多摸一张".into(),
                },
            ],
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
        skill: Some("duliang".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::DuLiang(f));
    Ok(true)
}

pub(crate) fn act_duliang(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::DuLiang(f)) => f,
        _ => return Err(internal("栈顶不是督粮帧")),
    };
    match f.stage {
        DuLiangStage::Target => {
            let target_pid = action
                .strip_prefix("target:")
                .ok_or_else(|| illegal("需要选择目标"))?;
            let target = st
                .seat_index(target_pid)
                .ok_or_else(|| illegal("目标不在座"))?;
            if st.seats[target].hand.is_empty() {
                return Err(illegal("目标没有手牌"));
            }
            let n = st.seats[target].hand.len();
            let idx = st.next_index(n);
            let card = st.seats[target].hand.remove(idx);
            let label = turn::card_label(st, &card);
            st.seats[f.actor].hand.push(card);
            st.bump_counter("fulin_gain");
            st.set_counter("duliang_used", 1);
            st.push_log(
                Some(&pid(st, f.actor)),
                format!("督粮：获得 {} 的【{label}】", st.seats[target].general_name),
            );
            cap_hand(st, f.actor);
            replace_top(
                st,
                Frame::DuLiang(DuLiangFrame {
                    stage: DuLiangStage::Mode,
                    target: Some(target),
                    ..f
                }),
            );
            Ok("督粮：请选择一项".to_string())
        }
        DuLiangStage::Mode => {
            let target = f.target.ok_or_else(|| internal("督粮缺少目标"))?;
            if action == "opt1" {
                let mut ev = Vec::new();
                let mut cards = Vec::new();
                for _ in 0..2 {
                    if let Some(card) = turn::draw_one(st, &mut ev) {
                        cards.push(card);
                    }
                }
                let mut gained = 0;
                for card in &cards {
                    let basic = card_type_bit(st, card) == 1;
                    let label = turn::card_label(st, card);
                    if basic {
                        st.seats[target].hand.push(card.clone());
                        gained += 1;
                        st.push_log(
                            Some(&pid(st, f.actor)),
                            format!(
                                "督粮：{} 观看并获得了【{label}】",
                                st.seats[target].general_name
                            ),
                        );
                    } else {
                        st.deck.draw.push(card.clone());
                        st.push_log(
                            Some(&pid(st, f.actor)),
                            format!(
                                "督粮：{} 未获得非基本牌【{label}】",
                                st.seats[target].general_name
                            ),
                        );
                    }
                }
                cap_hand(st, target);
                pop(st);
                return Ok(format!("督粮：其获得 {gained} 张基本牌"));
            }
            if action != "opt2" {
                return Err(illegal("需要选择一项"));
            }
            st.seats[target].flags.insert("duliang".into(), true);
            st.push_log(
                Some(&pid(st, f.actor)),
                format!(
                    "督粮：{} 下回合摸牌阶段多摸 1 张",
                    st.seats[target].general_name
                ),
            );
            pop(st);
            Ok("督粮结算完成".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// 匡弼（孙登）：目标将至多三张牌置于孙登武将牌上，下回合开始时获得并令其摸等量。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KuangBiStage {
    Target,
    Give,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KuangBiFrame {
    pub actor: usize,
    pub stage: KuangBiStage,
    pub helper: Option<usize>,
    pub selected: Vec<String>,
}

pub(crate) fn step_kuangbi(st: &mut SgsState, f: KuangBiFrame) -> Result<bool, EngineError> {
    let (options, actor) = match f.stage {
        KuangBiStage::Target => (
            st.alive_indices()
                .into_iter()
                .filter(|i| *i != f.actor && !st.seats[*i].hand.is_empty())
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("令 {} 将至多三张牌置于你武将牌上", st.seats[i].general_name),
                })
                .collect(),
            f.actor,
        ),
        KuangBiStage::Give => {
            let helper = f.helper.ok_or_else(|| internal("匡弼缺少目标"))?;
            let mut options: Vec<PendingOption> = turn::hand_ids(st, helper)
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
                    label: format!("确认（{} 张）", f.selected.len()),
                });
            }
            options.push(PendingOption {
                id: "pass".into(),
                label: "不放置".into(),
            });
            (options, helper)
        }
    };
    let (options, actor): (Vec<PendingOption>, usize) = (options, actor);
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, actor),
        skill: Some("kuangbi".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::KuangBi(f));
    Ok(true)
}

pub(crate) fn act_kuangbi(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::KuangBi(f)) => f,
        _ => return Err(internal("栈顶不是匡弼帧")),
    };
    match f.stage {
        KuangBiStage::Target => {
            let target_pid = action
                .strip_prefix("target:")
                .ok_or_else(|| illegal("需要选择目标"))?;
            let helper = st
                .seat_index(target_pid)
                .ok_or_else(|| illegal("目标不在座"))?;
            st.set_counter("kuangbi_used", 1);
            replace_top(
                st,
                Frame::KuangBi(KuangBiFrame {
                    stage: KuangBiStage::Give,
                    helper: Some(helper),
                    selected: Vec::new(),
                    ..f
                }),
            );
            Ok("匡弼：请目标选择要放置的牌".to_string())
        }
        KuangBiStage::Give => {
            let helper = f.helper.ok_or_else(|| internal("匡弼缺少目标"))?;
            if action == "pass" {
                pop(st);
                return Ok("匡弼：未放置牌".to_string());
            }
            if let Some(card) = action.strip_prefix("choose:") {
                if !st.seats[helper].hand.iter().any(|c| c == card) {
                    return Err(illegal("手牌中没有该牌"));
                }
                let mut selected = f.selected.clone();
                if let Some(pos) = selected.iter().position(|c| c == card) {
                    selected.remove(pos);
                } else {
                    if selected.len() >= 3 {
                        return Err(illegal("匡弼最多三张"));
                    }
                    selected.push(card.to_string());
                }
                replace_top(st, Frame::KuangBi(KuangBiFrame { selected, ..f }));
                return Ok("已更新匡弼选择".to_string());
            }
            if action != "confirm" || f.selected.is_empty() {
                return Err(illegal("请选择至少 1 张牌"));
            }
            let n = f.selected.len();
            for card in &f.selected {
                turn::take_card(st, helper, card)?;
                crate::skills::add_mark(st, f.actor, "kuangbi", card);
            }
            st.set_counter("kuangbi_helper", helper as u32 + 1);
            st.push_log(
                Some(&pid(st, helper)),
                format!(
                    "匡弼：将 {n} 张牌置于 {} 武将牌上",
                    st.seats[f.actor].general_name
                ),
            );
            pop(st);
            Ok("匡弼结算完成".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// 止戈（刘虞）：令攻击范围含你的一名角色使用【杀】或交装备。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZhiGeStage {
    Target,
    Choice,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZhiGeFrame {
    pub actor: usize,
    pub stage: ZhiGeStage,
    pub target: Option<usize>,
}

pub(crate) fn step_zhige(st: &mut SgsState, f: ZhiGeFrame) -> Result<bool, EngineError> {
    let (options, actor) = match f.stage {
        ZhiGeStage::Target => (
            st.alive_indices()
                .into_iter()
                .filter(|i| *i != f.actor && st.in_attack_range(*i, f.actor))
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("令 {} 选择", st.seats[i].general_name),
                })
                .collect(),
            f.actor,
        ),
        ZhiGeStage::Choice => {
            let target = f.target.ok_or_else(|| internal("止戈缺少目标"))?;
            let mut options = Vec::new();
            if turn::hand_cards_with_def(st, target, "sha")
                .iter()
                .any(|id| !crate::skills::qianxi_banned(st, target, id))
            {
                options.push(PendingOption {
                    id: "opt1".into(),
                    label: format!("对 {} 使用一张【杀】", st.seats[f.actor].general_name),
                });
            }
            if st.seats[target].equips.values().any(|c| c.is_some()) {
                options.push(PendingOption {
                    id: "opt2".into(),
                    label: "交给其一张装备区的牌".into(),
                });
            }
            (options, target)
        }
    };
    let (options, actor): (Vec<PendingOption>, usize) = (options, actor);
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, actor),
        skill: Some("zhige".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::ZhiGe(f));
    Ok(true)
}

pub(crate) fn act_zhige(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ZhiGe(f)) => f,
        _ => return Err(internal("栈顶不是止戈帧")),
    };
    match f.stage {
        ZhiGeStage::Target => {
            let target_pid = action
                .strip_prefix("target:")
                .ok_or_else(|| illegal("需要选择目标"))?;
            let target = st
                .seat_index(target_pid)
                .ok_or_else(|| illegal("目标不在座"))?;
            st.set_counter("zhige_used", 1);
            replace_top(
                st,
                Frame::ZhiGe(ZhiGeFrame {
                    stage: ZhiGeStage::Choice,
                    target: Some(target),
                    ..f
                }),
            );
            Ok("止戈：请目标选择".to_string())
        }
        ZhiGeStage::Choice => {
            let target = f.target.ok_or_else(|| internal("止戈缺少目标"))?;
            if action == "opt1" {
                let Some(sha) = turn::hand_cards_with_def(st, target, "sha")
                    .into_iter()
                    .next()
                else {
                    return Err(illegal("没有【杀】"));
                };
                turn::take_card(st, target, &sha)?;
                st.push_log(
                    Some(&pid(st, target)),
                    format!("止戈：对 {} 使用【杀】", st.seats[f.actor].general_name),
                );
                pop(st);
                crate::resolve::start_sha_flags(
                    st,
                    target,
                    &sha,
                    false,
                    DamageType::Normal,
                    false,
                    false,
                )?;
                // 直接指定唯一目标（刘虞）。
                for frame in st.resolution.iter_mut().rev() {
                    if let Frame::Sha(s) = frame {
                        s.target = Some(f.actor);
                        s.stage = ShaStage::WaitRespond;
                        break;
                    }
                }
                crate::resolve_skill::push_sha_target_response(
                    st,
                    target,
                    f.actor,
                    &sha,
                    1,
                    false,
                    false,
                    DamageType::Normal,
                );
                return Ok("止戈：视为对刘虞使用【杀】".to_string());
            }
            if action != "opt2" {
                return Err(illegal("需要选择一项"));
            }
            if let Some(card) = crate::resolve_jiexian::take_one_from_region(st, target) {
                let label = turn::card_label(st, &card);
                st.seats[f.actor].hand.push(card);
                st.push_log(Some(&pid(st, target)), format!("止戈：交给刘虞【{label}】"));
                cap_hand(st, f.actor);
            }
            pop(st);
            Ok("止戈结算完成".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// 宗祚（刘虞，锁定）：开局按势力数 +体力上限；势力最后一人死亡时 -1。
// ---------------------------------------------------------------------------

pub(crate) fn init_zongzuo(st: &mut SgsState) {
    let kingdoms = crate::skills::kingdom_count(st);
    let seats: Vec<usize> = st
        .alive_indices()
        .into_iter()
        .filter(|i| crate::skills::has(st, *i, "zongzuo"))
        .collect();
    for seat in seats {
        let max = (st.seats[seat].max_hp + kingdoms as i32).min(MAX_HP_LIMIT);
        st.seats[seat].max_hp = max;
        st.seats[seat].hp = (st.seats[seat].hp + 1).min(max);
        if st.seats[seat].hp <= 0 {
            st.seats[seat].hp = max;
        }
        st.push_log(
            Some(&pid(st, seat)),
            format!("宗祚：全场 {kingdoms} 个势力，体力上限 +{kingdoms} 并回复 1 点体力"),
        );
    }
}

/// 某势力最后一名角色死亡后，刘虞失去 1 点体力上限。
pub(crate) fn zongzuo_on_death(st: &mut SgsState, dead_seat: usize) {
    let Some(dead_kingdom) = st.seats[dead_seat]
        .general
        .as_deref()
        .and_then(crate::data::general)
        .map(|g| g.kingdom)
    else {
        return;
    };
    let still_alive = st.alive_indices().into_iter().any(|i| {
        st.seats[i]
            .general
            .as_deref()
            .and_then(crate::data::general)
            .is_some_and(|g| g.kingdom == dead_kingdom)
    });
    if still_alive {
        return;
    }
    for seat in st.alive_indices() {
        if crate::skills::has(st, seat, "zongzuo") {
            st.seats[seat].max_hp = (st.seats[seat].max_hp - 1).max(1);
            st.seats[seat].hp = st.seats[seat].hp.min(st.seats[seat].max_hp);
            st.push_log(
                Some(&pid(st, seat)),
                format!("宗祚：{dead_kingdom} 势力最后一名角色死亡，体力上限 -1"),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 极奢（岑昏）：摸一张并令本回合手牌上限 -1；结束阶段无手牌时令角色横置。
// ---------------------------------------------------------------------------

pub(crate) fn jishe_available(st: &SgsState, seat: usize) -> bool {
    if !crate::skills::has(st, seat, "jishe") {
        return false;
    }
    let base = st.seats[seat].hp.max(0) as u32;
    st.counter("jishe_used") < base
}

pub(crate) fn use_jishe(st: &mut SgsState, seat: usize) -> Result<String, EngineError> {
    if !jishe_available(st, seat) {
        return Err(illegal("极奢：手牌上限已为 0"));
    }
    let used = st.counter("jishe_used") + 1;
    st.set_counter("jishe_used", used);
    let drawn = draw_n(st, seat, 1);
    st.push_log(
        Some(&pid(st, seat)),
        format!("极奢：摸 {drawn} 张，本回合手牌上限 -1（已 {used}）"),
    );
    Ok("极奢：已摸牌".to_string())
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JiSheEndFrame {
    pub actor: usize,
    pub remaining: u32,
}

pub(crate) fn step_jishe_end(st: &mut SgsState, f: JiSheEndFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| !st.seats[*i].chained)
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("令 {} 进入连环状态", st.seats[i].general_name),
        })
        .collect();
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动极奢".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("jishe".into()),
        options,
        data: json!({ "remaining": f.remaining }),
    });
    replace_top(st, Frame::JiSheEnd(f));
    Ok(true)
}

pub(crate) fn act_jishe_end(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::JiSheEnd(f)) => f,
        _ => return Err(internal("栈顶不是极奢帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动极奢".to_string());
    }
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择目标"))?;
    let target = st
        .seat_index(target_pid)
        .ok_or_else(|| illegal("目标不在座"))?;
    st.seats[target].chained = true;
    st.push_log(
        Some(&pid(st, f.actor)),
        format!("极奢：{} 进入连环状态", st.seats[target].general_name),
    );
    crate::skills::faen_trigger(st, target);
    let remaining = f.remaining.saturating_sub(1);
    if remaining > 0 && st.alive_indices().into_iter().any(|i| !st.seats[i].chained) {
        replace_top(st, Frame::JiSheEnd(JiSheEndFrame { remaining, ..f }));
    } else {
        pop(st);
    }
    Ok("极奢结算完成".to_string())
}

/// 链祸（岑昏，锁定）：受到火焰伤害且处于连环状态（传导起点）时伤害 +1。
pub(crate) fn lianhuo_bonus(st: &SgsState, victim: usize, dtype: DamageType) -> i32 {
    if crate::skills::has(st, victim, "lianhuo")
        && dtype == DamageType::Fire
        && st.seats.get(victim).is_some_and(|s| s.chained)
    {
        1
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// 瑰藻（孙资刘放）：弃牌阶段弃置 ≥2 张且花色均不同 → 回复 1 或摸 1。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GuiZaoFrame {
    pub owner: usize,
}

pub(crate) fn guizao_ready(st: &SgsState, seat: usize) -> bool {
    if !crate::skills::has(st, seat, "guizao") || st.counter("guizao_offered") > 0 {
        return false;
    }
    if st.phase_discarded.len() < 2 {
        return false;
    }
    let mut seen = std::collections::BTreeSet::new();
    for card in &st.phase_discarded {
        if let Some(inst) = st.cards.get(card) {
            seen.insert(suit_bit(inst.suit));
        }
    }
    seen.len() == st.phase_discarded.len()
}

pub(crate) fn step_guizao(st: &mut SgsState, f: GuiZaoFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("guizao".into()),
        options: vec![
            PendingOption {
                id: "opt1".into(),
                label: "瑰藻：回复 1 点体力".into(),
            },
            PendingOption {
                id: "opt2".into(),
                label: "瑰藻：摸一张牌".into(),
            },
            PendingOption {
                id: "pass".into(),
                label: "不发动瑰藻".into(),
            },
        ],
        data: json!({}),
    });
    replace_top(st, Frame::GuiZao(f));
    Ok(true)
}

pub(crate) fn act_guizao(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::GuiZao(f)) => f,
        _ => return Err(internal("栈顶不是瑰藻帧")),
    };
    st.set_counter("guizao_offered", 1);
    match action {
        "opt1" => {
            if st.seats[f.owner].hp < st.seats[f.owner].max_hp {
                st.seats[f.owner].hp += 1;
            }
            let hp = st.seats[f.owner].hp;
            st.push_log(
                Some(&pid(st, f.owner)),
                format!("瑰藻：回复 1 点体力（{hp}）"),
            );
        }
        "opt2" => {
            let drawn = draw_n(st, f.owner, 1);
            st.push_log(Some(&pid(st, f.owner)), format!("瑰藻：摸 {drawn} 张"));
        }
        "pass" => {}
        _ => return Err(illegal("需要选择一项")),
    }
    pop(st);
    Ok("瑰藻结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 讥谀（孙资刘放）：令一名角色弃一张手牌，本回合不能使用该花色；黑桃则其失体力、自己翻面。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JiYuFrame {
    pub actor: usize,
}

pub(crate) fn jiyu_available(st: &SgsState, seat: usize) -> bool {
    crate::skills::has(st, seat, "jiyu")
        && !st.seats[seat].hand.is_empty()
        && st.alive_indices().into_iter().any(|i| {
            i != seat
                && !st.seats[i].hand.is_empty()
                && st.counter(&format!("jiyu_target:{i}")) == 0
        })
}

pub(crate) fn step_jiyu(st: &mut SgsState, f: JiYuFrame) -> Result<bool, EngineError> {
    let options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| {
            *i != f.actor
                && !st.seats[*i].hand.is_empty()
                && st.counter(&format!("jiyu_target:{i}")) == 0
        })
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("令 {} 弃置一张手牌", st.seats[i].general_name),
        })
        .collect();
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("jiyu".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::JiYu(f));
    Ok(true)
}

pub(crate) fn act_jiyu(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::JiYu(f)) => f,
        _ => return Err(internal("栈顶不是讥谀帧")),
    };
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择目标"))?;
    let target = st
        .seat_index(target_pid)
        .ok_or_else(|| illegal("目标不在座"))?;
    if st.seats[target].hand.is_empty() {
        return Err(illegal("目标没有手牌"));
    }
    let n = st.seats[target].hand.len();
    let idx = st.next_index(n);
    let card = st.seats[target].hand.remove(idx);
    let suit = st.cards.get(&card).map(|c| c.suit);
    let label = turn::card_label(st, &card);
    turn::discard_by(st, Some(target), &card);
    st.set_counter(&format!("jiyu_target:{target}"), 1);
    if let Some(suit) = suit {
        let banned = st.counter("jiyu_banned") | suit_bit(suit);
        st.set_counter("jiyu_banned", banned);
        st.push_log(
            Some(&pid(st, f.actor)),
            format!(
                "讥谀：{} 弃置【{label}】，你本回合不能使用该花色",
                st.seats[target].general_name
            ),
        );
        if suit == Suit::Spade {
            st.seats[target].hp -= 1;
            let hp = st.seats[target].hp;
            st.seats[f.actor].flipped = !st.seats[f.actor].flipped;
            crate::resolve_jiexian3::shebian_on_flip(st, f.actor);
            st.push_log(
                Some(&pid(st, f.actor)),
                format!(
                    "讥谀：黑桃，{} 失去 1 点体力（{hp}），你翻面",
                    st.seats[target].general_name
                ),
            );
            crate::skills::zhaxiang_on_lose_hp(st, target);
            if hp <= 0 {
                crate::resolve::push_dying(st, target, None);
            }
        }
    }
    pop(st);
    Ok("讥谀结算完成".to_string())
}

/// 讥谀：本回合被封锁的花色。
pub(crate) fn jiyu_banned(st: &SgsState, seat: usize, card_id: &str) -> bool {
    if !crate::skills::has(st, seat, "jiyu") {
        return false;
    }
    let banned = st.counter("jiyu_banned");
    if banned == 0 {
        return false;
    }
    st.cards
        .get(card_id)
        .is_some_and(|c| banned & suit_bit(c.suit) != 0)
}

// ---------------------------------------------------------------------------
// 寝情（黄皓）：结束阶段弃置攻击范围含主公的角色各一张牌，再令其各摸一张。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QinQingFrame {
    pub actor: usize,
    pub selected: Vec<usize>,
}

pub(crate) fn step_qinqing(st: &mut SgsState, f: QinQingFrame) -> Result<bool, EngineError> {
    let Some(lord) = st.lord_index() else {
        pop(st);
        return Ok(false);
    };
    let mut options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| {
            *i != f.actor
                && *i != lord
                && st.in_attack_range(*i, lord)
                && crate::turn::has_cards_to_take(st, *i)
        })
        .map(|i| {
            let mark = if f.selected.contains(&i) { "✔ " } else { "" };
            PendingOption {
                id: format!("target:{}", st.seats[i].player_id),
                label: format!("{mark}{}", st.seats[i].general_name),
            }
        })
        .collect();
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    if !f.selected.is_empty() {
        options.push(PendingOption {
            id: "confirm".into(),
            label: format!("确认（{} 名角色）", f.selected.len()),
        });
    }
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动寝情".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("qinqing".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::QinQing(f));
    Ok(true)
}

pub(crate) fn act_qinqing(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::QinQing(f)) => f,
        _ => return Err(internal("栈顶不是寝情帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动寝情".to_string());
    }
    if let Some(target_pid) = action.strip_prefix("target:") {
        let target = st
            .seat_index(target_pid)
            .ok_or_else(|| illegal("目标不在座"))?;
        let mut selected = f.selected.clone();
        if let Some(pos) = selected.iter().position(|i| *i == target) {
            selected.remove(pos);
        } else {
            selected.push(target);
        }
        replace_top(st, Frame::QinQing(QinQingFrame { selected, ..f }));
        return Ok("已更新寝情选择".to_string());
    }
    if action != "confirm" || f.selected.is_empty() {
        return Err(illegal("请选择至少 1 名角色"));
    }
    let lord = st.lord_index().unwrap_or(f.actor);
    let lord_hand = st.seats[lord].hand.len();
    let mut extra = 0;
    for seat in &f.selected {
        if let Some(card) = crate::resolve_jiexian::take_one_from_region(st, *seat) {
            let label = turn::card_label(st, &card);
            turn::discard_by(st, Some(*seat), &card);
            st.push_log(
                Some(&pid(st, f.actor)),
                format!("寝情：弃置 {} 的【{label}】", st.seats[*seat].general_name),
            );
        }
        let drawn = draw_n(st, *seat, 1);
        if st.seats[*seat].hand.len() > lord_hand {
            extra += 1;
        }
        st.push_log(Some(&pid(st, *seat)), format!("寝情：摸 {drawn} 张"));
        cap_hand(st, *seat);
    }
    if extra > 0 {
        let drawn = draw_n(st, f.actor, extra);
        st.push_log(
            Some(&pid(st, f.actor)),
            format!("寝情：{extra} 名角色手牌多于主公，你摸 {drawn} 张"),
        );
    }
    pop(st);
    Ok("寝情结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 贿生（黄皓）：受到其他角色伤害时，令其观看你一张牌并选择。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HuiShengFrame {
    pub owner: usize,
    pub source: usize,
    pub amount: i32,
    pub card: String,
    pub damage_type: DamageType,
    pub damage_card: Option<String>,
}

pub(crate) fn huisheng_available(st: &SgsState, owner: usize, source: usize) -> bool {
    crate::skills::has(st, owner, "huisheng")
        && owner != source
        && !st.seats[owner]
            .flags
            .contains_key(&format!("huisheng_no:{source}"))
}

pub(crate) fn step_huisheng(st: &mut SgsState, f: HuiShengFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.source),
        skill: Some("huisheng".into()),
        options: vec![
            PendingOption {
                id: "opt1".into(),
                label: format!(
                    "获得其展示的【{}】，防止此伤害（本局不能对其再发动贿生）",
                    turn::card_label(st, &f.card)
                ),
            },
            PendingOption {
                id: "opt2".into(),
                label: "弃置一张手牌，伤害照常结算".into(),
            },
        ],
        data: json!({}),
    });
    replace_top(st, Frame::HuiSheng(f));
    Ok(true)
}

pub(crate) fn act_huisheng(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::HuiSheng(f)) => f,
        _ => return Err(internal("栈顶不是贿生帧")),
    };
    if action == "opt1" {
        if st.seats[f.owner].hand.iter().any(|c| c == &f.card) {
            let _ = turn::take_card(st, f.owner, &f.card);
            st.seats[f.source].hand.push(f.card.clone());
        }
        st.seats[f.owner]
            .flags
            .insert(format!("huisheng_no:{}", f.source), true);
        let label = turn::card_label(st, &f.card);
        st.push_log(
            Some(&pid(st, f.source)),
            format!(
                "贿生：获得 {} 的【{label}】，防止此伤害",
                st.seats[f.owner].general_name
            ),
        );
        cap_hand(st, f.source);
        pop(st);
        return Ok("贿生：伤害被防止".to_string());
    }
    if action != "opt2" {
        return Err(illegal("需要选择一项"));
    }
    if !st.seats[f.source].hand.is_empty() {
        let n = st.seats[f.source].hand.len();
        let idx = st.next_index(n);
        let card = st.seats[f.source].hand.remove(idx);
        let label = turn::card_label(st, &card);
        turn::discard_by(st, Some(f.source), &card);
        st.push_log(
            Some(&pid(st, f.source)),
            format!("贿生：弃置【{label}】，伤害照常结算"),
        );
    }
    pop(st);
    apply_damage_now(
        st,
        f.owner,
        f.amount,
        Some(f.source),
        f.damage_type,
        f.damage_card.as_deref(),
    );
    Ok("贿生：伤害照常结算".to_string())
}

// ---------------------------------------------------------------------------
// 滔乱（张让）：一张牌当任意基本牌/普通锦囊；每种牌名限一次；之后令他人二选一。
// ---------------------------------------------------------------------------

const TAOLUAN_TRICKS: [&str; 6] = [
    "wuzhong_shengyou",
    "guohe_chaiqiao",
    "shunshou_qianyang",
    "juedou",
    "nanman_ruqin",
    "wanjian_qifa",
];

fn taoluan_used(st: &SgsState, seat: usize, code: &str) -> bool {
    st.seats
        .get(seat)
        .is_some_and(|s| s.flags.contains_key(&format!("taoluan_used:{code}")))
}

pub(crate) fn taoluan_options(st: &SgsState, seat: usize) -> Vec<(String, &'static str)> {
    if !crate::skills::has(st, seat, "taoluan")
        || st.counter("taoluan_disabled") > 0
        || st.phase == Phase::Dying
        || st.seats[seat].hand.is_empty()
    {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut codes: Vec<&'static str> = vec!["sha", "tao", "jiu"];
    codes.extend(TAOLUAN_TRICKS.iter().copied());
    for card in turn::hand_ids(st, seat) {
        for code in &codes {
            if taoluan_used(st, seat, code) {
                continue;
            }
            if *code == "tao" && st.seats[seat].hp >= st.seats[seat].max_hp {
                continue;
            }
            if *code == "jiu" && st.counter("jiu_used") > 0 {
                continue;
            }
            out.push((card.clone(), *code));
        }
    }
    out
}

pub(crate) fn use_taoluan(
    st: &mut SgsState,
    seat: usize,
    card: &str,
    code: &str,
) -> Result<String, EngineError> {
    if !taoluan_options(st, seat)
        .iter()
        .any(|(c, b)| c == card && *b == code)
    {
        return Err(illegal("滔乱：该组合不可用"));
    }
    let label = turn::card_label(st, card);
    st.seats[seat]
        .flags
        .insert(format!("taoluan_used:{code}"), true);
    let kind = card_type_bit(st, card);
    push(
        st,
        Frame::TaoLuan(TaoLuanFrame {
            actor: seat,
            stage: TaoLuanStage::Target,
            target: None,
            kind,
        }),
    );
    match code {
        "sha" => {
            if turn::sha_targets(st, seat).is_empty() {
                return Err(illegal("滔乱：攻击范围内没有目标"));
            }
            if st.counter("sha_used") >= turn::sha_limit(st, seat) {
                return Err(illegal("滔乱：本阶段已使用过【杀】"));
            }
            turn::take_card(st, seat, card)?;
            crate::resolve::start_sha_flags(
                st,
                seat,
                card,
                false,
                DamageType::Normal,
                false,
                false,
            )?;
            st.push_log(
                Some(&pid(st, seat)),
                format!("滔乱：将【{label}】当【杀】使用"),
            );
        }
        "tao" => {
            turn::take_card(st, seat, card)?;
            turn::to_discard_card(st, card);
            st.seats[seat].hp = (st.seats[seat].hp + 1).min(st.seats[seat].max_hp);
            st.push_log(
                Some(&pid(st, seat)),
                format!("滔乱：将【{label}】当【桃】使用"),
            );
        }
        "jiu" => {
            st.set_counter("jiu", 1);
            st.set_counter("jiu_used", 1);
            turn::take_card(st, seat, card)?;
            turn::to_discard_card(st, card);
            st.push_log(
                Some(&pid(st, seat)),
                format!("滔乱：将【{label}】当【酒】使用"),
            );
        }
        other => {
            if !TAOLUAN_TRICKS.contains(&other) {
                return Err(illegal("滔乱：不支持的牌名"));
            }
            turn::take_card(st, seat, card)?;
            st.deck.processing.push(card.to_string());
            crate::resolve::start_virtual_trick(st, seat, card, other)?;
            st.push_log(
                Some(&pid(st, seat)),
                format!("滔乱：将【{label}】当锦囊使用"),
            );
        }
    }
    Ok("滔乱：请选择令其选择一项的角色".to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaoLuanStage {
    Target,
    Choice,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TaoLuanFrame {
    pub actor: usize,
    pub stage: TaoLuanStage,
    pub target: Option<usize>,
    /// 滔乱所用牌的类型位图（用于「交一张类型不同的牌」）。
    pub kind: u32,
}

pub(crate) fn step_taoluan(st: &mut SgsState, f: TaoLuanFrame) -> Result<bool, EngineError> {
    let (options, actor) = match f.stage {
        TaoLuanStage::Target => (
            st.alive_indices()
                .into_iter()
                .filter(|i| *i != f.actor)
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("令 {} 选择一项", st.seats[i].general_name),
                })
                .collect(),
            f.actor,
        ),
        TaoLuanStage::Choice => {
            let target = f.target.ok_or_else(|| internal("滔乱缺少目标"))?;
            let mut options = Vec::new();
            if st.seats[target]
                .hand
                .iter()
                .any(|id| card_type_bit(st, id) != f.kind)
            {
                options.push(PendingOption {
                    id: "opt1".into(),
                    label: "交给其一张类型不同的牌".into(),
                });
            }
            options.push(PendingOption {
                id: "opt2".into(),
                label: "令其失去 1 点体力，本回合滔乱失效".into(),
            });
            (options, target)
        }
    };
    let (options, actor): (Vec<PendingOption>, usize) = (options, actor);
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, actor),
        skill: Some("taoluan".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::TaoLuan(f));
    Ok(true)
}

pub(crate) fn act_taoluan(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::TaoLuan(f)) => f,
        _ => return Err(internal("栈顶不是滔乱帧")),
    };
    match f.stage {
        TaoLuanStage::Target => {
            let target_pid = action
                .strip_prefix("target:")
                .ok_or_else(|| illegal("需要选择目标"))?;
            let target = st
                .seat_index(target_pid)
                .ok_or_else(|| illegal("目标不在座"))?;
            replace_top(
                st,
                Frame::TaoLuan(TaoLuanFrame {
                    stage: TaoLuanStage::Choice,
                    target: Some(target),
                    ..f
                }),
            );
            Ok("滔乱：请目标选择".to_string())
        }
        TaoLuanStage::Choice => {
            let target = f.target.ok_or_else(|| internal("滔乱缺少目标"))?;
            if action == "opt1" {
                let card = st.seats[target]
                    .hand
                    .iter()
                    .find(|id| card_type_bit(st, id) != f.kind)
                    .cloned()
                    .ok_or_else(|| illegal("没有类型不同的手牌"))?;
                let label = turn::card_label(st, &card);
                turn::take_card(st, target, &card)?;
                st.seats[f.actor].hand.push(card);
                st.push_log(Some(&pid(st, target)), format!("滔乱：交给张让【{label}】"));
                cap_hand(st, f.actor);
                pop(st);
                return Ok("滔乱结算完成".to_string());
            }
            if action != "opt2" {
                return Err(illegal("需要选择一项"));
            }
            st.seats[f.actor].hp -= 1;
            let hp = st.seats[f.actor].hp;
            st.set_counter("taoluan_disabled", 1);
            st.push_log(
                Some(&pid(st, target)),
                format!("滔乱：张让失去 1 点体力（{hp}），本回合滔乱失效"),
            );
            crate::skills::zhaxiang_on_lose_hp(st, f.actor);
            pop(st);
            if hp <= 0 {
                crate::resolve::push_dying(st, f.actor, None);
            }
            Ok("滔乱结算完成".to_string())
        }
    }
}
