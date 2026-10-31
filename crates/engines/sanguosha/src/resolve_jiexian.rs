//! P4.4.2 界标 25 将（批次 2：蜀 7 将）技能帧。
//!
//! 覆盖：义绝（界关羽）、替身（界张飞）、涯角（界赵云）、界仁德、界铁骑、界集智/奇才。
//! 同名技能 code 复用（ADR-0024），差异按 `skills::is_jiexian` 分支。

use crate::internal;
use crate::resolve::{
    discard_from_processing, leave_processing, pop, push, replace_top, top, Frame, ShaFrame,
    ShaStage,
};
use crate::skills::{has, is_jiexian};
use crate::state::*;
use crate::turn;
use game_engine_core::EngineError;
use serde_json::json;

fn pid(st: &SgsState, seat: usize) -> String {
    st.seats[seat].player_id.clone()
}

fn draw_n(st: &mut SgsState, seat: usize, n: u32) -> usize {
    let mut ev = Vec::new();
    turn::draw_cards(st, seat, n, &mut ev)
}

// ---------------------------------------------------------------------------
// 义绝（界关羽）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum YiJueStage {
    /// 弃置一张牌。
    Card,
    /// 选择一名其他角色。
    Target,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct YiJueFrame {
    pub actor: usize,
    pub stage: YiJueStage,
}

pub(crate) fn step_yijue(st: &mut SgsState, f: YiJueFrame) -> Result<bool, EngineError> {
    match f.stage {
        YiJueStage::Card => {
            let options: Vec<PendingOption> = turn::hand_ids(st, f.actor)
                .into_iter()
                .map(|id| PendingOption {
                    id: format!("choose:{id}"),
                    label: format!("弃置 {}", turn::card_label(st, &id)),
                })
                .collect();
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: pid(st, f.actor),
                skill: Some("yijue".into()),
                options,
                data: json!({ "stage": "card" }),
            });
            replace_top(st, Frame::YiJue(f));
            Ok(true)
        }
        YiJueStage::Target => {
            let options: Vec<PendingOption> = st
                .alive_indices()
                .into_iter()
                .filter(|i| *i != f.actor && !st.seats[*i].hand.is_empty())
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("义绝 {}", st.seats[i].general_name),
                })
                .collect();
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: pid(st, f.actor),
                skill: Some("yijue".into()),
                options,
                data: json!({ "stage": "target" }),
            });
            replace_top(st, Frame::YiJue(f));
            Ok(true)
        }
    }
}

pub(crate) fn act_yijue(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::YiJue(f)) => f,
        _ => return Err(internal("栈顶不是义绝帧")),
    };
    match f.stage {
        YiJueStage::Card => {
            let card = action
                .strip_prefix("choose:")
                .ok_or_else(|| crate::illegal("需要选择弃置的牌"))?;
            turn::take_card(st, f.actor, card)?;
            let label = turn::card_label(st, card);
            turn::discard_by(st, Some(f.actor), card);
            st.push_log(Some(&pid(st, f.actor)), format!("义绝：弃置【{label}】"));
            replace_top(
                st,
                Frame::YiJue(YiJueFrame {
                    actor: f.actor,
                    stage: YiJueStage::Target,
                }),
            );
            Ok("义绝：请选择目标".to_string())
        }
        YiJueStage::Target => {
            let target_pid = action
                .strip_prefix("target:")
                .ok_or_else(|| crate::illegal("需要选择目标"))?;
            let target = st
                .seat_index(target_pid)
                .ok_or_else(|| crate::illegal("目标不在座"))?;
            if target == f.actor || st.seats[target].hand.is_empty() {
                return Err(crate::illegal("目标不合法"));
            }
            let n = st.seats[target].hand.len();
            let pos = st.next_index(n);
            let card = st.seats[target].hand[pos].clone();
            let label = turn::card_label(st, &card);
            let red = st
                .cards
                .get(&card)
                .is_some_and(|c| crate::skills::is_red(c.suit));
            let owner = f.actor;
            if red {
                st.seats[target].hand.remove(pos);
                st.seats[owner].hand.push(card.clone());
                let mut healed = false;
                if st.seats[target].hp < st.seats[target].max_hp {
                    st.seats[target].hp += 1;
                    healed = true;
                }
                st.push_log(
                    Some(&pid(st, owner)),
                    format!(
                        "义绝：{} 展示【{label}】（红色），获得之{}",
                        st.seats[target].general_name,
                        if healed {
                            "，令其回复 1 点体力"
                        } else {
                            ""
                        }
                    ),
                );
            } else {
                // 黑色：目标本回合非锁定技失效且不能使用/打出手牌。
                st.set_counter(&format!("nonlocked_lost:{target}"), 1);
                st.set_counter(&format!("hand_sealed:{target}"), 1);
                st.set_counter(&format!("yijue_heart:{target}"), 1);
                st.push_log(
                    Some(&pid(st, owner)),
                    format!(
                        "义绝：{} 展示【{label}】（黑色），本回合非锁定技失效且不能使用/打出手牌",
                        st.seats[target].general_name
                    ),
                );
            }
            pop(st);
            Ok("义绝结算完成".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// 界仁德：本阶段第二张牌 → 视为使用一张基本牌
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RendeBasicFrame {
    pub actor: usize,
}

pub(crate) fn step_rende_basic(st: &mut SgsState, f: RendeBasicFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = Vec::new();
    let can_sha = st.counter("sha_used") < turn::sha_limit(st, f.actor)
        && !turn::sha_targets(st, f.actor).is_empty();
    if can_sha {
        options.push(PendingOption {
            id: "sha".into(),
            label: "视为使用【杀】".into(),
        });
    }
    if st.seats[f.actor].hp < st.seats[f.actor].max_hp {
        options.push(PendingOption {
            id: "tao".into(),
            label: "视为使用【桃】".into(),
        });
    }
    if st.counter("jiu_used") == 0 {
        options.push(PendingOption {
            id: "jiu".into(),
            label: "视为使用【酒】".into(),
        });
    }
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("rende_basic".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::RendeBasic(f));
    Ok(true)
}

pub(crate) fn act_rende_basic(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::RendeBasic(f)) => f,
        _ => return Err(internal("栈顶不是仁德·基本牌帧")),
    };
    let actor = f.actor;
    match action {
        "pass" => {
            pop(st);
            Ok("仁德：不视为使用基本牌".to_string())
        }
        "sha" => {
            pop(st);
            push(
                st,
                Frame::Sha(ShaFrame {
                    card: String::new(),
                    user: actor,
                    target: None,
                    stage: ShaStage::Target,
                    pending_targets: Vec::new(),
                    fangtian: false,
                    damage_type: crate::resolve::DamageType::Normal,
                    no_distance: false,
                    virtual_card: true,
                    tianyi: false,
                    no_respond: false,
                }),
            );
            st.push_log(Some(&pid(st, actor)), "仁德：视为使用【杀】");
            Ok("仁德：请选择【杀】的目标".to_string())
        }
        "tao" => {
            pop(st);
            st.seats[actor].hp = (st.seats[actor].hp + 1).min(st.seats[actor].max_hp);
            let hp = st.seats[actor].hp;
            st.push_log(
                Some(&pid(st, actor)),
                format!("仁德：视为使用【桃】，回复 1 点体力（{hp}）"),
            );
            Ok("仁德：回复 1 点体力".to_string())
        }
        "jiu" => {
            pop(st);
            st.set_counter("jiu_used", 1);
            st.set_counter("jiu", 1);
            st.push_log(
                Some(&pid(st, actor)),
                "仁德：视为使用【酒】（下一张【杀】伤害 +1）",
            );
            Ok("仁德：视为使用【酒】".to_string())
        }
        _ => Err(crate::illegal("未知的仁德选项")),
    }
}

// ---------------------------------------------------------------------------
// 涯角（界赵云）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct YaJiaoFrame {
    pub actor: usize,
    /// 于回合外使用/打出的牌。
    pub played: String,
    /// 展示的牌堆顶牌（在处理区）。
    pub revealed: String,
    /// 两张牌类型是否相同。
    pub same_type: bool,
    /// 类型不同时可弃牌的目标（攻击范围包含界赵云且有牌）。
    pub eligible: Vec<usize>,
}

/// 牌的类型分组：基本 / 锦囊 / 装备。
fn type_group(st: &SgsState, card: &str) -> u8 {
    let Some(def) = st
        .cards
        .get(card)
        .and_then(|c| crate::data::card_def(&c.def))
    else {
        return 0;
    };
    match def.category {
        Category::Basic => 1,
        Category::Trick | Category::DelayedTrick => 2,
        _ => 3,
    }
}

/// 回合外使用/打出牌后触发涯角（压入帧）。
pub(crate) fn yajiao_trigger(st: &mut SgsState, seat: usize, played: &str) -> bool {
    if !has(st, seat, "yajiao") || st.turn == seat || played.is_empty() {
        return false;
    }
    let mut ev = Vec::new();
    let Some(top_card) = turn::draw_one(st, &mut ev) else {
        return false;
    };
    st.deck.processing.push(top_card.clone());
    let same_type = type_group(st, played) == type_group(st, &top_card);
    let eligible: Vec<usize> = if same_type {
        Vec::new()
    } else {
        st.alive_indices()
            .into_iter()
            .filter(|v| {
                *v != seat
                    && st.in_attack_range(*v, seat)
                    && (!st.seats[*v].hand.is_empty()
                        || !st.seats[*v].judge_zone.is_empty()
                        || st.seats[*v].equips.values().any(|c| c.is_some()))
            })
            .collect()
    };
    let label = turn::card_label(st, &top_card);
    st.push_log(
        Some(&pid(st, seat)),
        format!(
            "涯角：展示牌堆顶【{label}】（{}）",
            if same_type {
                "类型相同"
            } else {
                "类型不同"
            }
        ),
    );
    if !same_type && eligible.is_empty() {
        discard_from_processing(st, &top_card);
        return false;
    }
    push(
        st,
        Frame::YaJiao(YaJiaoFrame {
            actor: seat,
            played: played.to_string(),
            revealed: top_card,
            same_type,
            eligible,
        }),
    );
    true
}

pub(crate) fn step_yajiao(st: &mut SgsState, f: YaJiaoFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = Vec::new();
    if f.same_type {
        for seat in st.alive_indices() {
            options.push(PendingOption {
                id: format!("target:{}", st.seats[seat].player_id),
                label: format!("{} 获得展示牌", st.seats[seat].general_name),
            });
        }
    } else {
        for seat in &f.eligible {
            options.push(PendingOption {
                id: format!("discard:{}", st.seats[*seat].player_id),
                label: format!("弃置 {} 一张牌", st.seats[*seat].general_name),
            });
        }
    }
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("yajiao".into()),
        options,
        data: json!({ "same_type": f.same_type }),
    });
    replace_top(st, Frame::YaJiao(f));
    Ok(true)
}

/// 从某座位区域取一张牌弃置（判定区 → 装备区 → 手牌随机），返回牌 id。
fn discard_one_from(st: &mut SgsState, seat: usize) -> Option<String> {
    if let Some(card) = st.seats[seat].judge_zone.pop() {
        turn::discard_by(st, Some(seat), &card);
        return Some(card);
    }
    for slot in EquipSlot::ALL {
        if let Some(card) = st.seats[seat].equips.get(&slot).cloned().flatten() {
            st.seats[seat].equips.insert(slot, None);
            turn::discard_by(st, Some(seat), &card);
            crate::skills::xiaoji_on_lose_equip(st, seat);
            return Some(card);
        }
    }
    if st.seats[seat].hand.is_empty() {
        return None;
    }
    let n = st.seats[seat].hand.len();
    let idx = st.next_index(n);
    let card = st.seats[seat].hand.remove(idx);
    turn::discard_by(st, Some(seat), &card);
    crate::skills::lianying_on_lose_last(st, seat);
    Some(card)
}

pub(crate) fn act_yajiao(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::YaJiao(f)) => f,
        _ => return Err(internal("栈顶不是涯角帧")),
    };
    let revealed_label = turn::card_label(st, &f.revealed);
    if let Some(target_pid) = action.strip_prefix("target:") {
        let target = st
            .seat_index(target_pid)
            .ok_or_else(|| crate::illegal("目标不在座"))?;
        leave_processing(st, &f.revealed);
        st.seats[target].hand.push(f.revealed.clone());
        st.push_log(
            Some(&pid(st, f.actor)),
            format!(
                "涯角：令 {} 获得【{revealed_label}】",
                st.seats[target].general_name
            ),
        );
        pop(st);
        return Ok("涯角：展示牌已交出".to_string());
    }
    if let Some(target_pid) = action.strip_prefix("discard:") {
        let target = st
            .seat_index(target_pid)
            .ok_or_else(|| crate::illegal("目标不在座"))?;
        if !f.eligible.contains(&target) {
            return Err(crate::illegal("该角色不在涯角范围内"));
        }
        if let Some(card) = discard_one_from(st, target) {
            let label = turn::card_label(st, &card);
            st.push_log(
                Some(&pid(st, f.actor)),
                format!("涯角：弃置 {} 的【{label}】", st.seats[target].general_name),
            );
        }
        discard_from_processing(st, &f.revealed);
        pop(st);
        return Ok("涯角：已弃置目标一张牌".to_string());
    }
    if action == "pass" {
        discard_from_processing(st, &f.revealed);
        pop(st);
        return Ok("涯角：不发动".to_string());
    }
    Err(crate::illegal("未知的涯角选项"))
}

// ---------------------------------------------------------------------------
// 界铁骑：目标弃同花色牌，否则不能使用【闪】
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TieQiFrame {
    pub user: usize,
    pub target: usize,
    pub card: String,
    pub need: u32,
    pub ignore_armor: bool,
    pub suit: Suit,
}

/// 界铁骑判定后：令目标弃一张与判定花色相同的牌，否则封锁【闪】。
pub(crate) fn push_tieqi(
    st: &mut SgsState,
    user: usize,
    target: usize,
    card: &str,
    need: u32,
    ignore_armor: bool,
    suit: Suit,
) {
    // 非锁定技失效（本回合）。
    if st.seats.get(target).is_some_and(|s| s.alive) {
        st.set_counter(&format!("nonlocked_lost:{target}"), 1);
    }
    let suits: Vec<String> = turn::hand_ids(st, target)
        .into_iter()
        .filter(|id| st.cards.get(id).is_some_and(|c| c.suit == suit))
        .collect();
    if suits.is_empty() {
        st.push_log(
            None,
            format!(
                "铁骑：{} 无同花色牌可弃，不能使用【闪】",
                st.seats[target].general_name
            ),
        );
        crate::resolve_skill::push_sha_respond(st, user, target, card, need, true, ignore_armor);
        return;
    }
    push(
        st,
        Frame::TieQi(TieQiFrame {
            user,
            target,
            card: card.to_string(),
            need,
            ignore_armor,
            suit,
        }),
    );
}

pub(crate) fn step_tieqi(st: &mut SgsState, f: TieQiFrame) -> Result<bool, EngineError> {
    let options: Vec<PendingOption> = turn::hand_ids(st, f.target)
        .into_iter()
        .filter(|id| st.cards.get(id).is_some_and(|c| c.suit == f.suit))
        .map(|id| PendingOption {
            id: format!("discard:{id}"),
            label: format!("弃置 {}", turn::card_label(st, &id)),
        })
        .collect();
    let mut options = options;
    options.push(PendingOption {
        id: "pass".into(),
        label: "不弃置（不能用【闪】）".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.target),
        skill: Some("tieqi".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::TieQi(f));
    Ok(true)
}

pub(crate) fn act_tieqi(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::TieQi(f)) => f,
        _ => return Err(internal("栈顶不是铁骑帧")),
    };
    let mut no_shan = true;
    if let Some(card) = action.strip_prefix("discard:") {
        if !st.seats[f.target].hand.iter().any(|c| c == card)
            || !st.cards.get(card).is_some_and(|c| c.suit == f.suit)
        {
            return Err(crate::illegal("只能弃置同花色的手牌"));
        }
        turn::take_card(st, f.target, card)?;
        let label = turn::card_label(st, card);
        turn::discard_by(st, Some(f.target), card);
        no_shan = false;
        st.push_log(
            Some(&pid(st, f.target)),
            format!("铁骑：弃置【{label}】，可以使用【闪】"),
        );
    } else if action != "pass" {
        return Err(crate::illegal("未知的铁骑选项"));
    } else {
        st.push_log(Some(&pid(st, f.target)), "铁骑：不弃牌，不能使用【闪】");
    }
    pop(st);
    crate::resolve_skill::push_sha_respond(
        st,
        f.user,
        f.target,
        &f.card,
        f.need,
        no_shan,
        f.ignore_armor,
    );
    Ok(if no_shan {
        "铁骑：不能使用【闪】".to_string()
    } else {
        "铁骑：已弃置同花色牌".to_string()
    })
}

// ---------------------------------------------------------------------------
// 替身（界张飞，限定技）/ 界激将
// ---------------------------------------------------------------------------

/// 替身：准备阶段回复所有体力，每回复 1 点摸一张（限定技，一次）。
pub(crate) fn tishen_prepare(st: &mut SgsState, seat: usize) -> bool {
    if !has(st, seat, "tishen")
        || st.seats[seat].flags.contains_key("tishen_used")
        || st.seats[seat].hp >= st.seats[seat].max_hp
        || st.seats[seat].hp <= 0
    {
        return false;
    }
    st.seats[seat].flags.insert("tishen_used".into(), true);
    let healed = (st.seats[seat].max_hp - st.seats[seat].hp).max(0) as u32;
    st.seats[seat].hp = st.seats[seat].max_hp;
    let drawn = draw_n(st, seat, healed);
    st.push_log(
        Some(&pid(st, seat)),
        format!("替身：回复 {healed} 点体力，摸 {drawn} 张"),
    );
    true
}

/// 界激将：蜀势力角色于其回合外替主公打出【杀】后，主公摸 1（每回合限一次）。
pub(crate) fn jijiang_lord_draw(st: &mut SgsState, by: usize) {
    if st.turn == by {
        return;
    }
    let Some(lord) = st.lord_index() else {
        return;
    };
    if !has(st, lord, "jijiang") || !is_jiexian(st, lord) {
        return;
    }
    if st.counter("jijiang_lord_draw") > 0 {
        return;
    }
    st.set_counter("jijiang_lord_draw", 1);
    let drawn = draw_n(st, lord, 1);
    st.push_log(
        Some(&pid(st, lord)),
        format!("激将：其他蜀势力角色回合外代出【杀】，摸 {drawn} 张"),
    );
}
