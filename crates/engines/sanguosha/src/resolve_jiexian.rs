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
pub(crate) fn discard_one_from(st: &mut SgsState, seat: usize) -> Option<String> {
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

// ---------------------------------------------------------------------------
// 奋威（界甘宁，限定技）：多目标锦囊可令任意个目标无效
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FenWeiFrame {
    pub owner: usize,
    pub targets: Vec<usize>,
    pub selected: Vec<usize>,
}

/// 多目标锦囊指定目标后，寻找可发动奋威的持有者并压入询问帧。
pub(crate) fn maybe_fenwei(st: &mut SgsState, targets: &[usize]) -> bool {
    if targets.len() < 2 {
        return false;
    }
    let owner = (0..st.seats.len()).find(|i| {
        st.seats[*i].alive
            && has(st, *i, "fenwei")
            && !st.seats[*i].flags.contains_key("fenwei_used")
    });
    let Some(owner) = owner else {
        return false;
    };
    push(
        st,
        Frame::FenWei(FenWeiFrame {
            owner,
            targets: targets.to_vec(),
            selected: Vec::new(),
        }),
    );
    true
}

pub(crate) fn step_fenwei(st: &mut SgsState, f: FenWeiFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = f
        .targets
        .iter()
        .map(|t| {
            let mark = if f.selected.contains(t) { "✔ " } else { "" };
            PendingOption {
                id: format!("choose:{}", st.seats[*t].player_id),
                label: format!("{mark}{}", st.seats[*t].general_name),
            }
        })
        .collect();
    options.push(PendingOption {
        id: "confirm".into(),
        label: format!("确认（令已选 {} 个目标无效）", f.selected.len()),
    });
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动奋威".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("fenwei".into()),
        options,
        data: json!({ "selected": f.selected.iter().map(|t| st.seats[*t].player_id.clone()).collect::<Vec<_>>() }),
    });
    replace_top(st, Frame::FenWei(f));
    Ok(true)
}

pub(crate) fn act_fenwei(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::FenWei(f)) => f,
        _ => return Err(internal("栈顶不是奋威帧")),
    };
    if let Some(target_pid) = action.strip_prefix("choose:") {
        let target = st
            .seat_index(target_pid)
            .ok_or_else(|| crate::illegal("目标不在座"))?;
        if !f.targets.contains(&target) {
            return Err(crate::illegal("该目标不在本锦囊目标内"));
        }
        let mut selected = f.selected.clone();
        if let Some(pos) = selected.iter().position(|t| *t == target) {
            selected.remove(pos);
        } else {
            selected.push(target);
        }
        replace_top(st, Frame::FenWei(FenWeiFrame { selected, ..f }));
        return Ok("已更新奋威目标".to_string());
    }
    if action == "pass" {
        pop(st);
        return Ok("不发动奋威".to_string());
    }
    if action != "confirm" {
        return Err(crate::illegal("未知的奋威选项"));
    }
    st.seats[f.owner].flags.insert("fenwei_used".into(), true);
    for target in &f.selected {
        st.set_counter(&format!("fenwei_void:{target}"), 1);
    }
    let names: Vec<String> = f
        .selected
        .iter()
        .map(|t| st.seats[*t].general_name.clone())
        .collect();
    st.push_log(
        Some(&pid(st, f.owner)),
        format!("奋威：令此锦囊对 {} 无效", names.join("、")),
    );
    pop(st);
    Ok("奋威结算完成".to_string())
}

/// 奋威：该目标是否已被此牌无效。
pub(crate) fn fenwei_void(st: &SgsState, target: usize) -> bool {
    st.counter(&format!("fenwei_void:{target}")) > 0
}

// ---------------------------------------------------------------------------
// 界反间（界周瑜）：展示并交给一张手牌，目标二选一
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JieFanJianStage {
    Card,
    Target,
    Choice,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JieFanJianFrame {
    pub actor: usize,
    pub stage: JieFanJianStage,
    pub card: Option<String>,
    pub target: Option<usize>,
}

pub(crate) fn step_jie_fanjian(st: &mut SgsState, f: JieFanJianFrame) -> Result<bool, EngineError> {
    let (options, actor, skill) = match f.stage {
        JieFanJianStage::Card => {
            let options = turn::hand_ids(st, f.actor)
                .into_iter()
                .map(|id| PendingOption {
                    id: format!("choose:{id}"),
                    label: format!("展示并交给 {}", turn::card_label(st, &id)),
                })
                .collect();
            (options, f.actor, "fanjian_jie")
        }
        JieFanJianStage::Target => {
            let options = st
                .alive_indices()
                .into_iter()
                .filter(|i| *i != f.actor)
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("反间 {}", st.seats[i].general_name),
                })
                .collect();
            (options, f.actor, "fanjian_jie")
        }
        JieFanJianStage::Choice => {
            let target = f.target.ok_or_else(|| internal("反间缺少目标"))?;
            let options = vec![
                PendingOption {
                    id: "discard".into(),
                    label: "展示所有手牌，弃置与此牌相同花色的牌".into(),
                },
                PendingOption {
                    id: "damage".into(),
                    label: "失去 1 点体力".into(),
                },
            ];
            (options, target, "fanjian_jie")
        }
    };
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, actor),
        skill: Some(skill.into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::JieFanJian(f));
    Ok(true)
}

pub(crate) fn act_jie_fanjian(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::JieFanJian(f)) => f,
        _ => return Err(internal("栈顶不是界反间帧")),
    };
    match f.stage {
        JieFanJianStage::Card => {
            let card = action
                .strip_prefix("choose:")
                .ok_or_else(|| crate::illegal("需要选择一张手牌"))?;
            if !st.seats[f.actor].hand.iter().any(|c| c == card) {
                return Err(crate::illegal("手牌中没有该牌"));
            }
            let label = turn::card_label(st, card);
            st.push_log(
                Some(&pid(st, f.actor)),
                format!("反间：展示并交给一张手牌【{label}】"),
            );
            replace_top(
                st,
                Frame::JieFanJian(JieFanJianFrame {
                    stage: JieFanJianStage::Target,
                    card: Some(card.to_string()),
                    target: None,
                    ..f
                }),
            );
            Ok("反间：请选择目标".to_string())
        }
        JieFanJianStage::Target => {
            let target_pid = action
                .strip_prefix("target:")
                .ok_or_else(|| crate::illegal("需要选择目标"))?;
            let target = st
                .seat_index(target_pid)
                .ok_or_else(|| crate::illegal("目标不在座"))?;
            let card = f.card.clone().ok_or_else(|| internal("反间缺少牌"))?;
            turn::take_card(st, f.actor, &card)?;
            st.seats[target].hand.push(card.clone());
            st.set_counter("fanjian_used", 1);
            replace_top(
                st,
                Frame::JieFanJian(JieFanJianFrame {
                    stage: JieFanJianStage::Choice,
                    card: Some(card),
                    target: Some(target),
                    ..f
                }),
            );
            Ok("反间：等待目标选择".to_string())
        }
        JieFanJianStage::Choice => {
            let target = f.target.ok_or_else(|| internal("反间缺少目标"))?;
            let card = f.card.clone().ok_or_else(|| internal("反间缺少牌"))?;
            match action {
                "damage" => {
                    pop(st);
                    crate::resolve::apply_damage(
                        st,
                        target,
                        1,
                        Some(f.actor),
                        crate::resolve::DamageType::Normal,
                        Some(&card),
                    );
                    st.push_log(Some(&pid(st, target)), "反间：选择失去 1 点体力");
                    Ok("反间：目标失去 1 点体力".to_string())
                }
                "discard" => {
                    let Some(inst) = st.cards.get(&card).cloned() else {
                        return Err(internal("反间牌缺少实例"));
                    };
                    let suit = inst.suit;
                    let hand = st.seats[target].hand.clone();
                    let mut discarded = 0;
                    for id in hand {
                        if st.cards.get(&id).is_some_and(|c| c.suit == suit) {
                            turn::take_card(st, target, &id)?;
                            turn::discard_by(st, Some(target), &id);
                            discarded += 1;
                        }
                    }
                    pop(st);
                    st.push_log(
                        Some(&pid(st, f.actor)),
                        format!(
                            "反间：{} 展示手牌并弃置 {discarded} 张同花色牌",
                            st.seats[target].general_name
                        ),
                    );
                    Ok(format!("反间：弃置 {discarded} 张同花色牌"))
                }
                _ => Err(crate::illegal("未知的反间选项")),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 界国色（界大乔）：选项二「弃方片 + 场上【乐不思蜀】」
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GuoSeFrame {
    pub actor: usize,
}

pub(crate) fn step_guose(st: &mut SgsState, f: GuoSeFrame) -> Result<bool, EngineError> {
    let options: Vec<PendingOption> = turn::hand_ids(st, f.actor)
        .into_iter()
        .filter(|id| st.cards.get(id).is_some_and(|c| c.suit == Suit::Diamond))
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
        skill: Some("guose".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::GuoSe(f));
    Ok(true)
}

/// 场上第一张【乐不思蜀】（座位序、判定区序），返回 (座位, 牌 id)。
pub(crate) fn find_lebu(st: &SgsState) -> Option<(usize, String)> {
    for seat in st.alive_indices() {
        for id in &st.seats[seat].judge_zone {
            if st.effective_def_code(id) == Some("lebu_sishu") {
                return Some((seat, id.clone()));
            }
        }
    }
    None
}

pub(crate) fn act_guose(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::GuoSe(f)) => f,
        _ => return Err(internal("栈顶不是国色帧")),
    };
    let card = action
        .strip_prefix("choose:")
        .ok_or_else(|| crate::illegal("需要选择方片牌"))?;
    if !st.seats[f.actor].hand.iter().any(|c| c == card)
        || !st.cards.get(card).is_some_and(|c| c.suit == Suit::Diamond)
    {
        return Err(crate::illegal("国色需弃置一张方片牌"));
    }
    let Some((owner, lebu)) = find_lebu(st) else {
        return Err(crate::illegal("场上没有【乐不思蜀】"));
    };
    turn::take_card(st, f.actor, card)?;
    turn::discard_by(st, Some(f.actor), card);
    if let Some(pos) = st.seats[owner].judge_zone.iter().position(|c| c == &lebu) {
        st.seats[owner].judge_zone.remove(pos);
    }
    turn::discard_by(st, Some(owner), &lebu);
    st.set_counter("guose_used", 1);
    let label = turn::card_label(st, &lebu);
    st.push_log(
        Some(&pid(st, f.actor)),
        format!("国色：弃置方片牌与{label}"),
    );
    let mut ev = Vec::new();
    let _ = turn::draw_cards(st, f.actor, 1, &mut ev);
    pop(st);
    Ok("国色：弃置乐不思蜀并摸 1 张".to_string())
}

// ---------------------------------------------------------------------------
// 界勤学（觉醒）/ 攻心（授予）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QinXueFrame {
    pub actor: usize,
}

/// 勤学觉醒条件：手牌数比体力值多 2 或更多。
pub(crate) fn qinxue_ready(st: &SgsState, seat: usize) -> bool {
    has(st, seat, "qinxue")
        && !st.seats[seat].flags.contains_key("qinxue_awakened")
        && st.seats[seat].hand.len() as i32 >= st.seats[seat].hp + 2
}

pub(crate) fn step_qinxue(st: &mut SgsState, f: QinXueFrame) -> Result<bool, EngineError> {
    let options = vec![
        PendingOption {
            id: "heal".into(),
            label: "减 1 点体力上限并回复 1 点体力，获得【攻心】".into(),
        },
        PendingOption {
            id: "draw".into(),
            label: "减 1 点体力上限并摸 2 张，获得【攻心】".into(),
        },
        PendingOption {
            id: "pass".into(),
            label: "不发动勤学".into(),
        },
    ];
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("qinxue".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::QinXue(f));
    Ok(true)
}

pub(crate) fn act_qinxue(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::QinXue(f)) => f,
        _ => return Err(internal("栈顶不是勤学帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动勤学".to_string());
    }
    if action != "heal" && action != "draw" {
        return Err(crate::illegal("未知的勤学选项"));
    }
    let actor = f.actor;
    st.seats[actor].max_hp -= 1;
    if st.seats[actor].hp > st.seats[actor].max_hp {
        st.seats[actor].hp = st.seats[actor].max_hp;
    }
    st.seats[actor].flags.insert("qinxue_awakened".into(), true);
    crate::skills::grant_skill(st, actor, "gongxin");
    let mut ev = Vec::new();
    if action == "heal" {
        st.seats[actor].hp += 1;
    } else {
        let _ = turn::draw_cards(st, actor, 2, &mut ev);
    }
    st.push_log(
        Some(&pid(st, actor)),
        format!(
            "勤学：减 1 点体力上限，{}，获得【攻心】",
            if action == "heal" {
                "回复 1 点体力"
            } else {
                "摸 2 张"
            }
        ),
    );
    pop(st);
    Ok("勤学觉醒完成".to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GongXinStage {
    Target,
    Card,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GongXinFrame {
    pub actor: usize,
    pub stage: GongXinStage,
    pub target: Option<usize>,
}

pub(crate) fn step_gongxin(st: &mut SgsState, f: GongXinFrame) -> Result<bool, EngineError> {
    let (options, actor) = match f.stage {
        GongXinStage::Target => (
            st.alive_indices()
                .into_iter()
                .filter(|i| *i != f.actor && !st.seats[*i].hand.is_empty())
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("观看 {} 的手牌", st.seats[i].general_name),
                })
                .collect(),
            f.actor,
        ),
        GongXinStage::Card => {
            let target = f.target.ok_or_else(|| internal("攻心缺少目标"))?;
            let mut options: Vec<PendingOption> = st.seats[target]
                .hand
                .iter()
                .filter(|id| st.cards.get(*id).is_some_and(|c| c.suit == Suit::Heart))
                .flat_map(|id| {
                    [
                        PendingOption {
                            id: format!("discard:{id}"),
                            label: format!("弃置 {}", turn::card_label(st, id)),
                        },
                        PendingOption {
                            id: format!("top:{id}"),
                            label: format!("置于牌堆顶 {}", turn::card_label(st, id)),
                        },
                    ]
                })
                .collect();
            options.push(PendingOption {
                id: "pass".into(),
                label: "不弃置/不置顶".into(),
            });
            (options, f.actor)
        }
    };
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, actor),
        skill: Some("gongxin".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::GongXin(f));
    Ok(true)
}
pub(crate) fn act_gongxin(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::GongXin(f)) => f,
        _ => return Err(internal("栈顶不是攻心帧")),
    };
    match f.stage {
        GongXinStage::Target => {
            let target_pid = action
                .strip_prefix("target:")
                .ok_or_else(|| crate::illegal("需要选择目标"))?;
            let target = st
                .seat_index(target_pid)
                .ok_or_else(|| crate::illegal("目标不在座"))?;
            st.set_counter("gongxin_used", 1);
            st.push_log(
                Some(&pid(st, f.actor)),
                format!("攻心：观看 {} 的手牌", st.seats[target].general_name),
            );
            replace_top(
                st,
                Frame::GongXin(GongXinFrame {
                    stage: GongXinStage::Card,
                    target: Some(target),
                    ..f
                }),
            );
            Ok("攻心：请选择红桃牌".to_string())
        }
        GongXinStage::Card => {
            let target = f.target.ok_or_else(|| internal("攻心缺少目标"))?;
            if action == "pass" {
                pop(st);
                return Ok("攻心：不弃置".to_string());
            }
            if let Some(card) = action.strip_prefix("discard:") {
                if !st.seats[target].hand.iter().any(|c| c == card)
                    || !st.cards.get(card).is_some_and(|c| c.suit == Suit::Heart)
                {
                    return Err(crate::illegal("攻心只能选择红桃手牌"));
                }
                turn::take_card(st, target, card)?;
                turn::discard_by(st, Some(target), card);
                let label = turn::card_label(st, card);
                pop(st);
                st.push_log(Some(&pid(st, f.actor)), format!("攻心：弃置{label}"));
                return Ok("攻心：弃置红桃牌".to_string());
            }
            if let Some(card) = action.strip_prefix("top:") {
                if !st.seats[target].hand.iter().any(|c| c == card)
                    || !st.cards.get(card).is_some_and(|c| c.suit == Suit::Heart)
                {
                    return Err(crate::illegal("攻心只能选择红桃手牌"));
                }
                turn::take_card(st, target, card)?;
                st.deck.draw.push(card.to_string());
                let label = turn::card_label(st, card);
                pop(st);
                st.push_log(
                    Some(&pid(st, f.actor)),
                    format!("攻心：将{label}置于牌堆顶"),
                );
                return Ok("攻心：置于牌堆顶".to_string());
            }
            Err(crate::illegal("未知的攻心选项"))
        }
    }
}

// ---------------------------------------------------------------------------
// 界连营：栈空后选择一名角色摸 1
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LianYingFrame {
    pub actor: usize,
}

pub(crate) fn lianying_ready(st: &SgsState) -> Option<usize> {
    (0..st.seats.len())
        .find(|i| st.seats[*i].alive && st.counter(&format!("lianying_pending:{i}")) > 0)
}

pub(crate) fn step_lianying(st: &mut SgsState, f: LianYingFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("令 {} 摸 1 张", st.seats[i].general_name),
        })
        .collect();
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动连营".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("lianying".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::LianYing(f));
    Ok(true)
}

pub(crate) fn act_lianying(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::LianYing(f)) => f,
        _ => return Err(internal("栈顶不是连营帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动连营".to_string());
    }
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| crate::illegal("需要选择目标"))?;
    let target = st
        .seat_index(target_pid)
        .ok_or_else(|| crate::illegal("目标不在座"))?;
    let mut ev = Vec::new();
    let drawn = turn::draw_cards(st, target, 1, &mut ev);
    st.push_log(
        Some(&pid(st, f.actor)),
        format!("连营：令 {} 摸 {drawn} 张", st.seats[target].general_name),
    );
    pop(st);
    Ok("连营结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 诈降（界黄盖）：红杀无距离/不可抵消
// ---------------------------------------------------------------------------

pub(crate) fn zhaxiang_red_active(st: &SgsState, seat: usize, card: &str) -> bool {
    st.turn == seat
        && st.turn_phase == TurnPhase::Play
        && st.counter("zhaxiang_red") > 0
        && st
            .cards
            .get(card)
            .is_some_and(|c| crate::skills::is_red(c.suit))
}

// ---------------------------------------------------------------------------
// 清俭（界夏侯惇）：摸牌阶段外获得牌后，交给一名其他角色一张牌换手牌上限
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QingJianFrame {
    pub actor: usize,
}

/// 清俭触发：摸牌阶段外获得牌后（每回合限一次）。
pub(crate) fn qingjian_trigger(st: &mut SgsState, seat: usize) {
    if !has(st, seat, "qingjian")
        || st.counter("qingjian_used") > 0
        || st.counter("qingjian_offered") > 0
        || st.seats[seat].hand.is_empty()
        || (st.turn == seat && st.turn_phase == TurnPhase::Draw)
    {
        return;
    }
    st.set_counter("qingjian_offered", 1);
    push(st, Frame::QingJian(QingJianFrame { actor: seat }));
}

pub(crate) fn step_qingjian(st: &mut SgsState, f: QingJianFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| *i != f.actor)
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("交给 {} 一张牌（手牌上限 +1）", st.seats[i].general_name),
        })
        .collect();
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动清俭".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("qingjian".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::QingJian(f));
    Ok(true)
}

pub(crate) fn act_qingjian(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::QingJian(f)) => f,
        _ => return Err(internal("栈顶不是清俭帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动清俭".to_string());
    }
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| crate::illegal("需要选择目标"))?;
    let target = st
        .seat_index(target_pid)
        .ok_or_else(|| crate::illegal("目标不在座"))?;
    let Some(card) = st.seats[f.actor].hand.first().cloned() else {
        pop(st);
        return Ok("清俭：没有可交出的牌".to_string());
    };
    turn::take_card(st, f.actor, &card)?;
    st.seats[target].hand.push(card.clone());
    st.set_counter("qingjian_used", 1);
    st.bump_counter("qingjian_limit");
    let label = turn::card_label(st, &card);
    st.push_log(
        Some(&pid(st, f.actor)),
        format!(
            "清俭：交给 {} 【{label}】，当前回合角色手牌上限 +1",
            st.seats[target].general_name
        ),
    );
    pop(st);
    Ok("清俭结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 恂恂（界李典）：摸牌阶段开始观看牌堆顶 4 张，2 张置顶、2 张置底
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct XunXunFrame {
    pub actor: usize,
    pub cards: Vec<String>,
    pub selected: Vec<String>,
}

pub(crate) fn step_xunxun(st: &mut SgsState, mut f: XunXunFrame) -> Result<bool, EngineError> {
    if f.cards.is_empty() {
        let mut ev = Vec::new();
        for _ in 0..4 {
            let Some(id) = turn::draw_one(st, &mut ev) else {
                break;
            };
            st.deck.processing.push(id.clone());
            f.cards.push(id);
        }
        if f.cards.is_empty() {
            pop(st);
            return Ok(false);
        }
        let names: Vec<String> = f.cards.iter().map(|id| turn::card_label(st, id)).collect();
        st.push_log(
            Some(&pid(st, f.actor)),
            format!("恂恂：观看牌堆顶 {}", names.join("、")),
        );
    }
    let mut options: Vec<PendingOption> = f
        .cards
        .iter()
        .map(|id| {
            let mark = if f.selected.contains(id) { "✔ " } else { "" };
            PendingOption {
                id: format!("choose:{id}"),
                label: format!("{mark}{}", turn::card_label(st, id)),
            }
        })
        .collect();
    let need = f.cards.len().min(2);
    if f.selected.len() == need {
        options.push(PendingOption {
            id: "confirm".into(),
            label: "确认（其余置底）".into(),
        });
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("xunxun".into()),
        options,
        data: json!({ "selected": f.selected }),
    });
    replace_top(st, Frame::XunXun(f));
    Ok(true)
}

pub(crate) fn act_xunxun(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::XunXun(f)) => f,
        _ => return Err(internal("栈顶不是恂恂帧")),
    };
    if let Some(card) = action.strip_prefix("choose:") {
        if !f.cards.iter().any(|c| c == card) {
            return Err(crate::illegal("只能选择本次观看的牌"));
        }
        let mut selected = f.selected.clone();
        if let Some(pos) = selected.iter().position(|c| c == card) {
            selected.remove(pos);
        } else {
            if selected.len() >= f.cards.len().min(2) {
                return Err(crate::illegal("恂恂最多选择 2 张置顶"));
            }
            selected.push(card.to_string());
        }
        replace_top(st, Frame::XunXun(XunXunFrame { selected, ..f }));
        return Ok("已更新恂恂选择".to_string());
    }
    if action != "confirm" {
        return Err(crate::illegal("未知的恂恂选项"));
    }
    let need = f.cards.len().min(2);
    if f.selected.len() != need {
        return Err(crate::illegal("请选择 2 张置于牌堆顶"));
    }
    for id in &f.selected {
        leave_processing(st, id);
    }
    for id in f.cards.iter().filter(|c| !f.selected.contains(c)) {
        leave_processing(st, id);
        st.deck.draw.insert(0, id.clone());
    }
    for id in f.selected.iter().rev() {
        st.deck.draw.push(id.clone());
    }
    st.set_counter("xunxun_done", 1);
    st.push_log(
        Some(&pid(st, f.actor)),
        "恂恂：2 张置于牌堆顶，其余置于牌堆底",
    );
    pop(st);
    Ok("恂恂结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 忘隙（界李典）：造成/受到 1 点伤害后，摸 2 并交给对方其中一张
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WangXiFrame {
    pub actor: usize,
    pub target: usize,
    pub cards: Vec<String>,
}

pub(crate) fn wangxi_trigger(st: &mut SgsState, actor: usize, target: usize) {
    if actor == target || !st.seats.get(actor).is_some_and(|s| s.alive) || !has(st, actor, "wangxi")
    {
        return;
    }
    push(
        st,
        Frame::WangXi(WangXiFrame {
            actor,
            target,
            cards: Vec::new(),
        }),
    );
}

pub(crate) fn step_wangxi(st: &mut SgsState, f: WangXiFrame) -> Result<bool, EngineError> {
    let (options, actor) = if f.cards.is_empty() {
        (
            vec![
                PendingOption {
                    id: "choose:wangxi".into(),
                    label: format!(
                        "发动忘隙：摸 2 张并交给 {} 其中一张",
                        st.seats[f.target].general_name
                    ),
                },
                PendingOption {
                    id: "pass".into(),
                    label: "不发动忘隙".into(),
                },
            ],
            f.actor,
        )
    } else {
        (
            f.cards
                .iter()
                .map(|id| PendingOption {
                    id: format!("choose:{id}"),
                    label: format!("交给对方 {}", turn::card_label(st, id)),
                })
                .collect(),
            f.actor,
        )
    };
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, actor),
        skill: Some("wangxi".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::WangXi(f));
    Ok(true)
}

pub(crate) fn act_wangxi(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::WangXi(f)) => f,
        _ => return Err(internal("栈顶不是忘隙帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动忘隙".to_string());
    }
    if action == "choose:wangxi" {
        let mut ev = Vec::new();
        let mut drawn = Vec::new();
        for _ in 0..2 {
            if let Some(id) = turn::draw_one(st, &mut ev) {
                st.seats[f.actor].hand.push(id.clone());
                drawn.push(id);
            }
        }
        if drawn.is_empty() {
            pop(st);
            return Ok("忘隙：无牌可摸".to_string());
        }
        replace_top(st, Frame::WangXi(WangXiFrame { cards: drawn, ..f }));
        return Ok("忘隙：请选择交给对方的一张牌".to_string());
    }
    let card = action
        .strip_prefix("choose:")
        .ok_or_else(|| crate::illegal("需要选择交给对方的牌"))?;
    if !f.cards.iter().any(|c| c == card) {
        return Err(crate::illegal("只能交给对方本次摸到的牌"));
    }
    turn::take_card(st, f.actor, card)?;
    st.seats[f.target].hand.push(card.to_string());
    let label = turn::card_label(st, card);
    st.push_log(
        Some(&pid(st, f.actor)),
        format!(
            "忘隙：摸 2 张并交给 {} 【{label}】",
            st.seats[f.target].general_name
        ),
    );
    let over = st.seats[f.target]
        .hand
        .len()
        .saturating_sub(st.config.max_hand_cards as usize);
    if over > 0 {
        crate::resolve::push_discard(st, f.target, over, "hand_cap");
    }
    pop(st);
    Ok("忘隙结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 除疠（界华佗）：弃置你与势力各不相同的其他角色各一张牌，黑桃牌角色摸 1
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChuLiFrame {
    pub actor: usize,
    pub selected: Vec<usize>,
}

/// 从某座位区域取一张牌（不弃置；判定区 → 装备区 → 手牌随机）。
pub(crate) fn take_one_from_region(st: &mut SgsState, seat: usize) -> Option<String> {
    if let Some(card) = st.seats[seat].judge_zone.pop() {
        return Some(card);
    }
    for slot in EquipSlot::ALL {
        if let Some(card) = st.seats[seat].equips.get(&slot).cloned().flatten() {
            st.seats[seat].equips.insert(slot, None);
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
    crate::skills::lianying_on_lose_last(st, seat);
    Some(card)
}

pub(crate) fn step_chuli(st: &mut SgsState, f: ChuLiFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = Vec::new();
    for i in st.alive_indices() {
        if i == f.actor {
            continue;
        }
        let mark = if f.selected.contains(&i) { "✔ " } else { "" };
        let kingdom = crate::skills::kingdom_of(st, i);
        let dup = f
            .selected
            .iter()
            .any(|s| crate::skills::kingdom_of(st, *s) == kingdom);
        if !f.selected.contains(&i) && dup {
            continue;
        }
        options.push(PendingOption {
            id: format!("choose:{}", st.seats[i].player_id),
            label: format!("{mark}{}（{kingdom}）", st.seats[i].general_name),
        });
    }
    if !f.selected.is_empty() {
        options.push(PendingOption {
            id: "confirm".into(),
            label: format!("确认（{} 名角色 + 你）", f.selected.len()),
        });
    }
    options.push(PendingOption {
        id: "pass".into(),
        label: "取消".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("chuli".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::ChuLi(f));
    Ok(true)
}

pub(crate) fn act_chuli(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ChuLi(f)) => f,
        _ => return Err(internal("栈顶不是除疠帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("取消除疠".to_string());
    }
    if let Some(target_pid) = action.strip_prefix("choose:") {
        let target = st
            .seat_index(target_pid)
            .ok_or_else(|| crate::illegal("目标不在座"))?;
        if target == f.actor {
            return Err(crate::illegal("不能选择自己"));
        }
        let mut selected = f.selected.clone();
        if let Some(pos) = selected.iter().position(|s| *s == target) {
            selected.remove(pos);
        } else {
            let kingdom = crate::skills::kingdom_of(st, target);
            if selected
                .iter()
                .any(|s| crate::skills::kingdom_of(st, *s) == kingdom)
            {
                return Err(crate::illegal("除疠只能选择势力各不相同的角色"));
            }
            selected.push(target);
        }
        replace_top(st, Frame::ChuLi(ChuLiFrame { selected, ..f }));
        return Ok("已更新除疠选择".to_string());
    }
    if action != "confirm" {
        return Err(crate::illegal("未知的除疠选项"));
    }
    if f.selected.is_empty() {
        return Err(crate::illegal("除疠至少选择 1 名其他角色"));
    }
    let mut seats = vec![f.actor];
    seats.extend(f.selected.iter().copied());
    st.set_counter("chuli_used", 1);
    let mut log = Vec::new();
    for seat in seats {
        let Some(card) = take_one_from_region(st, seat) else {
            continue;
        };
        let label = turn::card_label(st, &card);
        let spade = st.cards.get(&card).is_some_and(|c| c.suit == Suit::Spade);
        turn::discard_by(st, Some(seat), &card);
        log.push(format!("{}【{label}】", st.seats[seat].general_name));
        if spade {
            let mut ev = Vec::new();
            let drawn = turn::draw_cards(st, seat, 1, &mut ev);
            st.push_log(
                Some(&pid(st, seat)),
                format!("除疠：黑桃牌被弃置，摸 {drawn} 张"),
            );
        }
    }
    st.push_log(
        Some(&pid(st, f.actor)),
        format!("除疠：弃置 {}", log.join("、")),
    );
    pop(st);
    Ok("除疠结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 利驭（界吕布）：杀造成伤害后可获得其一张牌；装备牌则视为对其指定的角色决斗
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiYuStage {
    Ask,
    Pick,
    Designate,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LiYuFrame {
    pub actor: usize,
    pub target: usize,
    pub stage: LiYuStage,
}

pub(crate) fn liyu_trigger(st: &mut SgsState, actor: usize, target: usize) {
    if actor == target || !has(st, actor, "liyu") {
        return;
    }
    if !st.seats.get(target).is_some_and(|s| s.alive) || !turn::has_cards_to_take(st, target) {
        return;
    }
    // 延后到栈空再询问，避免决斗 outcome 覆盖父帧（如【杀】的【闪】结算）。
    st.pending_liyu.push((actor, target));
}

pub(crate) fn step_liyu(st: &mut SgsState, f: LiYuFrame) -> Result<bool, EngineError> {
    let options: Vec<PendingOption> = match f.stage {
        LiYuStage::Ask => vec![
            PendingOption {
                id: "choose:liyu".into(),
                label: format!("发动利驭：获得 {} 一张牌", st.seats[f.target].general_name),
            },
            PendingOption {
                id: "pass".into(),
                label: "不发动利驭".into(),
            },
        ],
        LiYuStage::Pick => {
            let mut options: Vec<PendingOption> = Vec::new();
            if !st.seats[f.target].hand.is_empty() {
                options.push(PendingOption {
                    id: "pick:hand".into(),
                    label: format!("手牌：随机 1 张（共 {} 张）", st.seats[f.target].hand.len()),
                });
            }
            for slot in EquipSlot::ALL {
                if let Some(Some(id)) = st.seats[f.target].equips.get(&slot) {
                    options.push(PendingOption {
                        id: format!("pick:equip:{}", slot_code(slot)),
                        label: format!("装备区：{}", turn::card_label(st, id)),
                    });
                }
            }
            options.push(PendingOption {
                id: "pass".into(),
                label: "不获得".into(),
            });
            options
        }
        LiYuStage::Designate => st
            .alive_indices()
            .into_iter()
            .filter(|i| *i != f.actor && *i != f.target && !crate::skills::kongcheng_blocks(st, *i))
            .map(|i| PendingOption {
                id: format!("target:{}", st.seats[i].player_id),
                label: format!(
                    "令 {} 与 {} 决斗",
                    st.seats[f.actor].general_name, st.seats[i].general_name
                ),
            })
            .collect(),
    };
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("liyu".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::LiYu(f));
    Ok(true)
}

fn slot_code(slot: EquipSlot) -> &'static str {
    match slot {
        EquipSlot::Weapon => "weapon",
        EquipSlot::Armor => "armor",
        EquipSlot::HorseOffense => "horse_offense",
        EquipSlot::HorseDefense => "horse_defense",
        EquipSlot::Treasure => "treasure",
    }
}

fn slot_from_code(code: &str) -> Option<EquipSlot> {
    Some(match code {
        "weapon" => EquipSlot::Weapon,
        "armor" => EquipSlot::Armor,
        "horse_offense" => EquipSlot::HorseOffense,
        "horse_defense" => EquipSlot::HorseDefense,
        "treasure" => EquipSlot::Treasure,
        _ => return None,
    })
}

pub(crate) fn act_liyu(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::LiYu(f)) => f,
        _ => return Err(internal("栈顶不是利驭帧")),
    };
    match f.stage {
        LiYuStage::Ask => {
            if action == "pass" {
                pop(st);
                return Ok("不发动利驭".to_string());
            }
            if action != "choose:liyu" {
                return Err(crate::illegal("未知的利驭选项"));
            }
            replace_top(
                st,
                Frame::LiYu(LiYuFrame {
                    stage: LiYuStage::Pick,
                    ..f
                }),
            );
            Ok("利驭：请选择要获得的牌".to_string())
        }
        LiYuStage::Pick => {
            if action == "pass" {
                pop(st);
                return Ok("不发动利驭".to_string());
            }
            let picked = if let Some(slot) = action.strip_prefix("pick:equip:") {
                let slot = slot_from_code(slot).ok_or_else(|| crate::illegal("非法装备槽"))?;
                let card = st.seats[f.target]
                    .equips
                    .get(&slot)
                    .cloned()
                    .flatten()
                    .ok_or_else(|| crate::illegal("该槽位没有装备"))?;
                st.seats[f.target].equips.insert(slot, None);
                crate::skills::xiaoji_on_lose_equip(st, f.target);
                card
            } else if action == "pick:hand" {
                if st.seats[f.target].hand.is_empty() {
                    return Err(crate::illegal("目标没有手牌"));
                }
                let n = st.seats[f.target].hand.len();
                let idx = st.next_index(n);
                let card = st.seats[f.target].hand.remove(idx);
                crate::skills::lianying_on_lose_last(st, f.target);
                card
            } else {
                return Err(crate::illegal("未知的利驭拿牌选项"));
            };
            let label = turn::card_label(st, &picked);
            let equip = st
                .cards
                .get(&picked)
                .and_then(|c| crate::data::card_def(&c.def))
                .is_some_and(|d| d.equip_slot.is_some());
            st.seats[f.actor].hand.push(picked.clone());
            st.push_log(
                Some(&pid(st, f.actor)),
                format!(
                    "利驭：获得 {} 的【{label}】",
                    st.seats[f.target].general_name
                ),
            );
            if equip {
                replace_top(
                    st,
                    Frame::LiYu(LiYuFrame {
                        stage: LiYuStage::Designate,
                        ..f
                    }),
                );
                Ok("利驭：请由目标指定决斗对象".to_string())
            } else {
                let mut ev = Vec::new();
                let drawn = turn::draw_cards(st, f.target, 1, &mut ev);
                st.push_log(
                    Some(&pid(st, f.target)),
                    format!("利驭：获得的不是装备牌，摸 {drawn} 张"),
                );
                pop(st);
                Ok("利驭结算完成".to_string())
            }
        }
        LiYuStage::Designate => {
            let target_pid = action
                .strip_prefix("target:")
                .ok_or_else(|| crate::illegal("需要指定决斗对象"))?;
            let victim = st
                .seat_index(target_pid)
                .ok_or_else(|| crate::illegal("目标不在座"))?;
            if victim == f.actor || victim == f.target {
                return Err(crate::illegal("决斗对象不合法"));
            }
            pop(st);
            st.push_log(
                Some(&pid(st, f.actor)),
                format!(
                    "利驭：视为对 {} 使用【决斗】",
                    st.seats[victim].general_name
                ),
            );
            let need = if crate::skills::has(st, f.actor, "wushuang") {
                2
            } else {
                1
            };
            push(
                st,
                Frame::Respond(crate::resolve::RespondFrame {
                    kind: crate::resolve::RespondKind::Duel,
                    actor: victim,
                    card: String::new(),
                    source: f.actor,
                    target: Some(f.actor),
                    damage: 1,
                    damage_type: crate::resolve::DamageType::Normal,
                    consume_card: false,
                    ignore_armor: false,
                    bagua_used: false,
                    need,
                    played: 0,
                    no_shan: false,
                    duel_initiator: Some(f.actor),
                    is_sha: false,
                }),
            );
            Ok("利驭：决斗开始".to_string())
        }
    }
}
