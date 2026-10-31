//! P4.4.2 一将成名（批次 Y2：8 将）技能帧。
//!
//! 覆盖：陷嗣（刘封）、夺刀/暗箭（潘璋马忠）、纵玄/直言（虞翻）、胆守（朱然）、
//! 惴恐/求援（伏皇后）、灭计/绝策/焚城（李儒）、恩怨/眩惑（法正）、无言/举荐（徐庶）。

use crate::resolve::{
    pop, push, push_discard, replace_top, top, DamageType, Frame, Outcome, ShaFrame, ShaStage,
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

/// 牌的类型（基本 1 / 锦囊 2 / 装备 4）。
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

fn is_trick(st: &SgsState, card_id: &str) -> bool {
    matches!(card_type_bit(st, card_id), 2)
}

fn is_equip(st: &SgsState, card_id: &str) -> bool {
    st.cards
        .get(card_id)
        .and_then(|c| crate::data::card_def(&c.def))
        .is_some_and(|d| d.equip_slot.is_some())
}

/// 弃置 `n` 张随机手牌（自动口径）；手牌不足返回 false。
fn discard_random_hand(st: &mut SgsState, seat: usize, n: usize, reason: &str) -> bool {
    if st.seats[seat].hand.len() < n {
        return false;
    }
    let mut picked = Vec::new();
    for _ in 0..n {
        let len = st.seats[seat].hand.len();
        let idx = st.next_index(len);
        picked.push(st.seats[seat].hand.remove(idx));
    }
    for card in &picked {
        let label = turn::card_label(st, card);
        turn::discard_by(st, Some(seat), card);
        st.push_log(Some(&pid(st, seat)), format!("{reason}：弃置【{label}】"));
    }
    true
}

/// 把牌置于牌堆顶（按传入顺序：先传入的在最下，最后传入的最先摸到）。
fn put_on_top(st: &mut SgsState, cards: &[String]) {
    for card in cards {
        st.deck.discard.retain(|c| c != card);
        st.deck.processing.retain(|c| c != card);
        st.deck.draw.push(card.clone());
    }
}

// ---------------------------------------------------------------------------
// 陷嗣（刘封）：准备阶段将至多两名角色各一张牌置于武将牌上为「逆」。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct XianSiFrame {
    pub actor: usize,
    pub selected: Vec<usize>,
}

pub(crate) fn step_xiansi(st: &mut SgsState, f: XianSiFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| *i != f.actor && turn::has_cards_to_take(st, *i))
        .map(|i| {
            let mark = if f.selected.contains(&i) { "✔ " } else { "" };
            PendingOption {
                id: format!("target:{}", st.seats[i].player_id),
                label: format!("{mark}取 {} 一张牌", st.seats[i].general_name),
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
        label: "不发动陷嗣".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("xiansi".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::XianSi(f));
    Ok(true)
}

pub(crate) fn act_xiansi(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::XianSi(f)) => f,
        _ => return Err(internal("栈顶不是陷嗣帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动陷嗣".to_string());
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
                return Err(illegal("陷嗣最多选择两名角色"));
            }
            selected.push(target);
        }
        replace_top(st, Frame::XianSi(XianSiFrame { selected, ..f }));
        return Ok("已更新陷嗣选择".to_string());
    }
    if action != "confirm" || f.selected.is_empty() {
        return Err(illegal("请选择至少 1 名角色"));
    }
    let mut taken = 0;
    for seat in &f.selected {
        if let Some(card) = crate::resolve_jiexian::take_one_from_region(st, *seat) {
            let label = turn::card_label(st, &card);
            crate::skills::add_mark(st, f.actor, "ni", &card);
            st.push_log(
                Some(&pid(st, f.actor)),
                format!(
                    "陷嗣：将 {} 的【{label}】置于武将牌上为「逆」",
                    st.seats[*seat].general_name
                ),
            );
            taken += 1;
        }
    }
    pop(st);
    Ok(format!("陷嗣：获得 {taken} 张「逆」"))
}

/// 其他角色出牌阶段：移去两张「逆」视为对刘封使用【杀】（虚拟杀，无距离）。
pub(crate) fn use_xiansi_sha(st: &mut SgsState, user: usize) -> Result<String, EngineError> {
    let Some(owner) = crate::skills::find_owner(st, "xiansi") else {
        return Err(illegal("场上没有陷嗣"));
    };
    if owner == user {
        return Err(illegal("不能对自己发动陷嗣"));
    }
    let marks = crate::skills::mark_cards(st, owner, "ni");
    if marks.len() < 2 {
        return Err(illegal("「逆」不足两张"));
    }
    if st.counter("xiansi_used") > 0 {
        return Err(illegal("本阶段已发动过陷嗣"));
    }
    if !st.seats[owner].alive {
        return Err(illegal("目标已阵亡"));
    }
    st.set_counter("xiansi_used", 1);
    for card in marks.iter().take(2) {
        crate::skills::remove_mark(st, owner, "ni", card);
        turn::discard_by(st, Some(owner), card);
    }
    st.push_log(
        Some(&pid(st, user)),
        format!(
            "陷嗣：移去两张「逆」，视为对 {} 使用【杀】",
            st.seats[owner].general_name
        ),
    );
    push(
        st,
        Frame::Sha(ShaFrame {
            card: String::new(),
            user,
            target: Some(owner),
            stage: ShaStage::WaitRespond,
            pending_targets: Vec::new(),
            fangtian: false,
            damage_type: DamageType::Normal,
            no_distance: true,
            virtual_card: true,
            tianyi: false,
            no_respond: false,
        }),
    );
    crate::resolve_skill::push_sha_target_response(
        st,
        user,
        owner,
        "",
        1,
        false,
        false,
        DamageType::Normal,
    );
    Ok("陷嗣：请响应【杀】".to_string())
}

// ---------------------------------------------------------------------------
// 夺刀（潘璋马忠）：被【杀】伤害后可弃一张牌获得来源的武器。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DuoDaoFrame {
    pub owner: usize,
    pub source: usize,
}

pub(crate) fn step_duodao(st: &mut SgsState, f: DuoDaoFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = turn::hand_ids(st, f.owner)
        .into_iter()
        .map(|id| PendingOption {
            id: format!("choose:{id}"),
            label: format!("弃置 {}", turn::card_label(st, &id)),
        })
        .collect();
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动夺刀".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("duodao".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::DuoDao(f));
    Ok(true)
}

pub(crate) fn act_duodao(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::DuoDao(f)) => f,
        _ => return Err(internal("栈顶不是夺刀帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动夺刀".to_string());
    }
    let card = action
        .strip_prefix("choose:")
        .ok_or_else(|| illegal("需要弃置一张牌"))?;
    turn::take_card(st, f.owner, card)?;
    let label = turn::card_label(st, card);
    turn::discard_by(st, Some(f.owner), card);
    let weapon = st.seats[f.source]
        .equips
        .get(&EquipSlot::Weapon)
        .cloned()
        .flatten();
    let msg = if let Some(weapon) = weapon {
        st.seats[f.source].equips.insert(EquipSlot::Weapon, None);
        let wlabel = turn::card_label(st, &weapon);
        st.seats[f.owner].hand.push(weapon.clone());
        crate::skills::xiaoji_on_lose_equip(st, f.source);
        st.push_log(
            Some(&pid(st, f.owner)),
            format!(
                "夺刀：弃置【{label}】，获得 {} 的武器【{wlabel}】",
                st.seats[f.source].general_name
            ),
        );
        cap_hand(st, f.owner);
        format!("夺刀：获得【{wlabel}】")
    } else {
        st.push_log(Some(&pid(st, f.owner)), format!("夺刀：弃置【{label}】"));
        "夺刀：来源没有武器".to_string()
    };
    pop(st);
    Ok(msg)
}

// ---------------------------------------------------------------------------
// 纵玄（虞翻）：你的牌因弃置进入弃牌堆后，可将其中任意张置于牌堆顶。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZongXuanFrame {
    pub actor: usize,
    pub cards: Vec<String>,
    pub selected: Vec<String>,
}

pub(crate) fn step_zongxuan(st: &mut SgsState, f: ZongXuanFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = f
        .cards
        .iter()
        .filter(|id| st.deck.discard.iter().any(|c| c == *id))
        .map(|id| {
            let mark = if f.selected.contains(id) { "✔ " } else { "" };
            PendingOption {
                id: format!("choose:{id}"),
                label: format!("{mark}{}", turn::card_label(st, id)),
            }
        })
        .collect();
    if !f.selected.is_empty() {
        options.push(PendingOption {
            id: "confirm".into(),
            label: format!("确认置于牌堆顶（{} 张）", f.selected.len()),
        });
    }
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动纵玄".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("zongxuan".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::ZongXuan(f));
    Ok(true)
}

pub(crate) fn act_zongxuan(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ZongXuan(f)) => f,
        _ => return Err(internal("栈顶不是纵玄帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动纵玄".to_string());
    }
    if let Some(card) = action.strip_prefix("choose:") {
        if !f.cards.iter().any(|c| c == card) {
            return Err(illegal("该牌不在本次弃置中"));
        }
        let mut selected = f.selected.clone();
        if let Some(pos) = selected.iter().position(|c| c == card) {
            selected.remove(pos);
        } else {
            selected.push(card.to_string());
        }
        replace_top(st, Frame::ZongXuan(ZongXuanFrame { selected, ..f }));
        return Ok("已更新纵玄选择".to_string());
    }
    if action != "confirm" || f.selected.is_empty() {
        return Err(illegal("请选择至少 1 张牌"));
    }
    let n = f.selected.len();
    put_on_top(st, &f.selected);
    st.push_log(
        Some(&pid(st, f.actor)),
        format!("纵玄：将 {n} 张弃置牌置于牌堆顶"),
    );
    pop(st);
    Ok("纵玄结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 直言（虞翻）：结束阶段令一名角色摸 1 展示，装备牌则使用并回复 1。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZhiYanFrame {
    pub actor: usize,
}

pub(crate) fn step_zhiyan(st: &mut SgsState, f: ZhiYanFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("令 {} 摸一张并展示", st.seats[i].general_name),
        })
        .collect();
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动直言".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("zhiyan".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::ZhiYan(f));
    Ok(true)
}

pub(crate) fn act_zhiyan(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ZhiYan(f)) => f,
        _ => return Err(internal("栈顶不是直言帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动直言".to_string());
    }
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择目标"))?;
    let target = st
        .seat_index(target_pid)
        .ok_or_else(|| illegal("目标不在座"))?;
    let drawn = draw_n(st, target, 1);
    let Some(card) = st.seats[target].hand.last().cloned() else {
        pop(st);
        return Ok("直言：没有摸到牌".to_string());
    };
    let label = turn::card_label(st, &card);
    st.push_log(
        Some(&pid(st, f.actor)),
        format!(
            "直言：{} 摸 {drawn} 张并展示【{label}】",
            st.seats[target].general_name
        ),
    );
    // 界直言（界虞翻）：非装备牌且其体力值不等于你 → 其失去 1 点体力。
    if drawn > 0 && !is_equip(st, &card) && crate::skills::is_jiexian(st, f.actor) {
        if st.seats[target].hp != st.seats[f.actor].hp {
            st.seats[target].hp -= 1;
            let hp = st.seats[target].hp;
            st.push_log(
                Some(&pid(st, f.actor)),
                format!(
                    "直言：{} 展示非装备牌，体力值与虞翻不同，失去 1 点体力（{hp}）",
                    st.seats[target].general_name
                ),
            );
            if hp <= 0 {
                crate::resolve::push_dying(st, target, Some(f.actor));
            }
        }
    }
    if drawn > 0 && is_equip(st, &card) {
        let slot = st
            .cards
            .get(&card)
            .and_then(|c| crate::data::card_def(&c.def))
            .and_then(|d| d.equip_slot)
            .ok_or_else(|| internal("直言：装备槽解析失败"))?;
        turn::take_card(st, target, &card)?;
        let old = st.seats[target].equips.insert(slot, Some(card.clone()));
        if let Some(Some(old_id)) = old {
            turn::to_discard_card(st, &old_id);
            crate::skills::xiaoji_on_lose_equip(st, target);
        }
        if st.seats[target].hp < st.seats[target].max_hp {
            st.seats[target].hp += 1;
        }
        let hp = st.seats[target].hp;
        st.push_log(
            Some(&pid(st, f.actor)),
            format!(
                "直言：{} 使用装备【{label}】并回复 1 点体力（{hp}）",
                st.seats[target].general_name
            ),
        );
    }
    pop(st);
    Ok("直言结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 胆守（朱然）：弃 X 张牌（X=本阶段发动次数）选择攻击范围内角色。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DanShouStage {
    Cost,
    Target,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DanShouFrame {
    pub actor: usize,
    pub stage: DanShouStage,
    pub x: u32,
    pub selected: Vec<String>,
}

pub(crate) fn danshou_x(st: &SgsState) -> u32 {
    st.counter("danshou_used") + 1
}

pub(crate) fn step_danshou(st: &mut SgsState, f: DanShouFrame) -> Result<bool, EngineError> {
    let (options, actor) = match f.stage {
        DanShouStage::Cost => {
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
            if f.selected.len() == f.x as usize {
                options.push(PendingOption {
                    id: "confirm".into(),
                    label: format!("确认弃置 {} 张", f.x),
                });
            }
            options.push(PendingOption {
                id: "pass".into(),
                label: "不发动胆守".into(),
            });
            (options, f.actor)
        }
        DanShouStage::Target => (
            st.alive_indices()
                .into_iter()
                .filter(|i| *i != f.actor && st.in_attack_range(f.actor, *i))
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("对 {} 发动胆守", st.seats[i].general_name),
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
        skill: Some("danshou".into()),
        options,
        data: json!({ "x": f.x }),
    });
    replace_top(st, Frame::DanShou(f));
    Ok(true)
}

pub(crate) fn act_danshou(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::DanShou(f)) => f,
        _ => return Err(internal("栈顶不是胆守帧")),
    };
    match f.stage {
        DanShouStage::Cost => {
            if action == "pass" {
                pop(st);
                return Ok("不发动胆守".to_string());
            }
            if let Some(card) = action.strip_prefix("choose:") {
                if !st.seats[f.actor].hand.iter().any(|c| c == card) {
                    return Err(illegal("手牌中没有该牌"));
                }
                let mut selected = f.selected.clone();
                if let Some(pos) = selected.iter().position(|c| c == card) {
                    selected.remove(pos);
                } else {
                    if selected.len() >= f.x as usize {
                        return Err(illegal(format!("胆守本次需弃 {} 张", f.x)));
                    }
                    selected.push(card.to_string());
                }
                replace_top(st, Frame::DanShou(DanShouFrame { selected, ..f }));
                return Ok("已更新胆守选择".to_string());
            }
            if action != "confirm" || f.selected.len() != f.x as usize {
                return Err(illegal(format!("胆守本次需弃 {} 张", f.x)));
            }
            for card in &f.selected {
                turn::take_card(st, f.actor, card)?;
                turn::discard_by(st, Some(f.actor), card);
            }
            st.set_counter("danshou_used", f.x);
            st.push_log(
                Some(&pid(st, f.actor)),
                format!("胆守：弃置 {} 张牌，请选择目标", f.x),
            );
            replace_top(
                st,
                Frame::DanShou(DanShouFrame {
                    stage: DanShouStage::Target,
                    ..f
                }),
            );
            Ok("胆守：请选择目标".to_string())
        }
        DanShouStage::Target => {
            let target_pid = action
                .strip_prefix("target:")
                .ok_or_else(|| illegal("需要选择目标"))?;
            let target = st
                .seat_index(target_pid)
                .ok_or_else(|| illegal("目标不在座"))?;
            let name = st.seats[target].general_name.clone();
            pop(st);
            match f.x {
                1 => {
                    if let Some(card) = crate::resolve_jiexian::take_one_from_region(st, target) {
                        let label = turn::card_label(st, &card);
                        turn::discard_by(st, Some(target), &card);
                        st.push_log(
                            Some(&pid(st, f.actor)),
                            format!("胆守①：弃置 {name} 的【{label}】"),
                        );
                    }
                    Ok("胆守①结算完成".to_string())
                }
                2 => {
                    if let Some(card) = crate::resolve_jiexian::take_one_from_region(st, target) {
                        let label = turn::card_label(st, &card);
                        st.seats[f.actor].hand.push(card);
                        st.push_log(
                            Some(&pid(st, f.actor)),
                            format!("胆守②：{name} 交给【{label}】"),
                        );
                        cap_hand(st, f.actor);
                    }
                    Ok("胆守②结算完成".to_string())
                }
                3 => {
                    st.push_log(
                        Some(&pid(st, f.actor)),
                        format!("胆守③：对 {name} 造成 1 点伤害"),
                    );
                    crate::resolve::apply_damage(
                        st,
                        target,
                        1,
                        Some(f.actor),
                        DamageType::Normal,
                        None,
                    );
                    Ok("胆守③结算完成".to_string())
                }
                _ => {
                    let a = draw_n(st, f.actor, 2);
                    let b = draw_n(st, target, 2);
                    st.push_log(
                        Some(&pid(st, f.actor)),
                        format!("胆守④：你摸 {a} 张，{name} 摸 {b} 张"),
                    );
                    cap_hand(st, f.actor);
                    cap_hand(st, target);
                    Ok("胆守④结算完成".to_string())
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 惴恐（伏皇后）：其他角色回合开始时拼点。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZhuiKongStage {
    Ask,
    WaitPindian,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZhuiKongFrame {
    pub owner: usize,
    pub current: usize,
    pub stage: ZhuiKongStage,
}

pub(crate) fn zhuikong_owner(st: &SgsState, current: usize) -> Option<usize> {
    st.alive_indices().into_iter().find(|i| {
        *i != current
            && crate::skills::has(st, *i, "zhuikong")
            && st.seats[*i].hp < st.seats[*i].max_hp
            && !st.seats[current].hand.is_empty()
            && !st.seats[*i].hand.is_empty()
    })
}

pub(crate) fn step_zhuikong(st: &mut SgsState, f: ZhuiKongFrame) -> Result<bool, EngineError> {
    if f.stage == ZhuiKongStage::WaitPindian {
        if let Some(Outcome::Pindian { winner, .. }) = st.outcome.take() {
            if winner == Some(f.owner) {
                for i in st.alive_indices() {
                    if i != f.current {
                        crate::skills::set_deny_target(st, f.current, i);
                    }
                }
                st.push_log(
                    Some(&pid(st, f.owner)),
                    format!(
                        "惴恐：拼点赢，{} 本回合不能对其他角色使用牌",
                        st.seats[f.current].general_name
                    ),
                );
            } else {
                st.set_counter(&format!("zhuikong:{}:{}", f.current, f.owner), 1);
                st.set_counter(&format!("zhuikong:{}:{}", f.owner, f.current), 1);
                st.push_log(
                    Some(&pid(st, f.owner)),
                    format!(
                        "惴恐：拼点没赢，{} 与你距离视为 1",
                        st.seats[f.current].general_name
                    ),
                );
            }
        }
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("zhuikong".into()),
        options: vec![
            PendingOption {
                id: "opt1".into(),
                label: format!("惴恐：与 {} 拼点", st.seats[f.current].general_name),
            },
            PendingOption {
                id: "pass".into(),
                label: "不发动惴恐".into(),
            },
        ],
        data: json!({}),
    });
    replace_top(st, Frame::ZhuiKong(f));
    Ok(true)
}

pub(crate) fn act_zhuikong(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ZhuiKong(f)) => f,
        _ => return Err(internal("栈顶不是惴恐帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动惴恐".to_string());
    }
    if action != "opt1" {
        return Err(illegal("需要选择是否拼点"));
    }
    st.push_log(
        Some(&pid(st, f.owner)),
        format!("惴恐：与 {} 拼点", st.seats[f.current].general_name),
    );
    replace_top(
        st,
        Frame::ZhuiKong(ZhuiKongFrame {
            stage: ZhuiKongStage::WaitPindian,
            ..f
        }),
    );
    crate::resolve_fire::start_pindian(st, f.owner, f.current, json!({ "skill": "zhuikong" }));
    Ok("惴恐：请选择拼点手牌".to_string())
}

// ---------------------------------------------------------------------------
// 求援（伏皇后）：成为【杀】目标时令另一名角色交【闪】或成为额外目标。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QiuYuanStage {
    Helper,
    Choice,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QiuYuanFrame {
    pub user: usize,
    pub target: usize,
    pub card: String,
    pub need: u32,
    pub no_shan: bool,
    pub ignore_armor: bool,
    pub damage_type: DamageType,
    pub stage: QiuYuanStage,
    pub helper: Option<usize>,
}

pub(crate) fn step_qiuyuan(st: &mut SgsState, f: QiuYuanFrame) -> Result<bool, EngineError> {
    let (options, actor) = match f.stage {
        QiuYuanStage::Helper => (
            st.alive_indices()
                .into_iter()
                .filter(|i| *i != f.user && *i != f.target)
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("令 {} 求援", st.seats[i].general_name),
                })
                .collect(),
            f.target,
        ),
        QiuYuanStage::Choice => {
            let helper = f.helper.ok_or_else(|| internal("求援缺少协助者"))?;
            let mut options = Vec::new();
            if st.seats[helper]
                .hand
                .iter()
                .any(|id| turn::card_def(st, id).is_some_and(|d| d.code == "shan"))
            {
                options.push(PendingOption {
                    id: "opt1".into(),
                    label: format!("交给 {} 一张【闪】", st.seats[f.target].general_name),
                });
            }
            options.push(PendingOption {
                id: "opt2".into(),
                label: "成为此【杀】的额外目标".into(),
            });
            (options, helper)
        }
    };
    if options.is_empty() {
        pop(st);
        continue_qiuyuan(st, &f);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, actor),
        skill: Some("qiuyuan".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::QiuYuan(f));
    Ok(true)
}

fn continue_qiuyuan(st: &mut SgsState, f: &QiuYuanFrame) {
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

pub(crate) fn act_qiuyuan(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::QiuYuan(f)) => f,
        _ => return Err(internal("栈顶不是求援帧")),
    };
    match f.stage {
        QiuYuanStage::Helper => {
            let target_pid = action
                .strip_prefix("target:")
                .ok_or_else(|| illegal("需要选择协助角色"))?;
            let helper = st
                .seat_index(target_pid)
                .ok_or_else(|| illegal("目标不在座"))?;
            st.push_log(
                Some(&pid(st, f.target)),
                format!(
                    "求援：令 {} 选择交【闪】或成为额外目标",
                    st.seats[helper].general_name
                ),
            );
            replace_top(
                st,
                Frame::QiuYuan(QiuYuanFrame {
                    stage: QiuYuanStage::Choice,
                    helper: Some(helper),
                    ..f
                }),
            );
            Ok("求援：请协助者选择".to_string())
        }
        QiuYuanStage::Choice => {
            let helper = f.helper.ok_or_else(|| internal("求援缺少协助者"))?;
            if action == "pass" {
                pop(st);
                continue_qiuyuan(st, &f);
                return Ok("求援：放弃".to_string());
            }
            if action == "opt1" {
                let Some(shan) = st.seats[helper]
                    .hand
                    .iter()
                    .find(|id| turn::card_def(st, id).is_some_and(|d| d.code == "shan"))
                    .cloned()
                else {
                    return Err(illegal("没有【闪】"));
                };
                turn::take_card(st, helper, &shan)?;
                st.seats[f.target].hand.push(shan.clone());
                st.push_log(
                    Some(&pid(st, helper)),
                    format!("求援：交给 {} 一张【闪】", st.seats[f.target].general_name),
                );
                pop(st);
                continue_qiuyuan(st, &f);
                return Ok("求援：已交给【闪】".to_string());
            }
            if action != "opt2" {
                return Err(illegal("需要选择一项"));
            }
            // 成为额外目标：挂到当前【杀】帧的后续目标队列。
            for frame in st.resolution.iter_mut().rev() {
                if let Frame::Sha(s) = frame {
                    s.pending_targets.push(helper);
                    break;
                }
            }
            st.push_log(
                Some(&pid(st, helper)),
                format!(
                    "求援：{} 成为此【杀】的额外目标",
                    st.seats[helper].general_name
                ),
            );
            pop(st);
            continue_qiuyuan(st, &f);
            Ok("求援：额外目标已加入".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// 灭计（李儒）：黑色锦囊置于牌堆顶，令目标弃锦囊或两张非锦囊。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MieJiStage {
    Card,
    Target,
    Choice,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MieJiFrame {
    pub actor: usize,
    pub stage: MieJiStage,
    pub card: Option<String>,
    pub target: Option<usize>,
}

pub(crate) fn step_mieji(st: &mut SgsState, f: MieJiFrame) -> Result<bool, EngineError> {
    let (options, actor) = match f.stage {
        MieJiStage::Card => (
            turn::hand_ids(st, f.actor)
                .into_iter()
                .filter(|id| {
                    is_trick(st, id)
                        && (crate::skills::is_jiexian(st, f.actor)
                            || st
                                .cards
                                .get(id)
                                .is_some_and(|c| crate::skills::is_black(c.suit)))
                })
                .map(|id| PendingOption {
                    id: format!("choose:{id}"),
                    label: format!("将 {} 置于牌堆顶", turn::card_label(st, &id)),
                })
                .collect(),
            f.actor,
        ),
        MieJiStage::Target => (
            st.alive_indices()
                .into_iter()
                .filter(|i| *i != f.actor && !st.seats[*i].hand.is_empty())
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("令 {} 选择", st.seats[i].general_name),
                })
                .collect(),
            f.actor,
        ),
        MieJiStage::Choice => {
            let target = f.target.ok_or_else(|| internal("灭计缺少目标"))?;
            let mut options = Vec::new();
            if st.seats[target].hand.iter().any(|id| is_trick(st, id)) {
                options.push(PendingOption {
                    id: "opt1".into(),
                    label: "弃置一张锦囊牌".into(),
                });
            }
            if crate::skills::is_jiexian(st, f.actor) {
                // 界灭计（界李儒）：依次弃置两张牌（不限类型）。
                if st.seats[target].hand.len() >= 2 {
                    options.push(PendingOption {
                        id: "opt2".into(),
                        label: "依次弃置两张牌".into(),
                    });
                }
            } else {
                let non_trick = st.seats[target]
                    .hand
                    .iter()
                    .filter(|id| !is_trick(st, id))
                    .count();
                if non_trick >= 2 {
                    options.push(PendingOption {
                        id: "opt2".into(),
                        label: "依次弃置两张非锦囊牌".into(),
                    });
                }
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
        skill: Some("mieji".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::MieJi(f));
    Ok(true)
}

pub(crate) fn act_mieji(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::MieJi(f)) => f,
        _ => return Err(internal("栈顶不是灭计帧")),
    };
    match f.stage {
        MieJiStage::Card => {
            let card = action
                .strip_prefix("choose:")
                .ok_or_else(|| illegal("需要选择一张黑色锦囊牌"))?;
            if !st.seats[f.actor].hand.iter().any(|c| c == card) {
                return Err(illegal("手牌中没有该牌"));
            }
            if !is_trick(st, card)
                || (!crate::skills::is_jiexian(st, f.actor)
                    && !st
                        .cards
                        .get(card)
                        .is_some_and(|c| crate::skills::is_black(c.suit)))
            {
                return Err(illegal("灭计需要锦囊牌（标准版需为黑色）"));
            }
            turn::take_card(st, f.actor, card)?;
            let label = turn::card_label(st, card);
            st.deck.draw.push(card.to_string());
            st.set_counter("mieji_used", 1);
            st.push_log(
                Some(&pid(st, f.actor)),
                format!("灭计：将【{label}】置于牌堆顶"),
            );
            replace_top(
                st,
                Frame::MieJi(MieJiFrame {
                    stage: MieJiStage::Target,
                    card: Some(card.to_string()),
                    ..f
                }),
            );
            Ok("灭计：请选择目标".to_string())
        }
        MieJiStage::Target => {
            let target_pid = action
                .strip_prefix("target:")
                .ok_or_else(|| illegal("需要选择目标"))?;
            let target = st
                .seat_index(target_pid)
                .ok_or_else(|| illegal("目标不在座"))?;
            replace_top(
                st,
                Frame::MieJi(MieJiFrame {
                    stage: MieJiStage::Choice,
                    target: Some(target),
                    ..f
                }),
            );
            Ok("灭计：请目标选择".to_string())
        }
        MieJiStage::Choice => {
            let target = f.target.ok_or_else(|| internal("灭计缺少目标"))?;
            if action == "opt1" {
                let card = st.seats[target]
                    .hand
                    .iter()
                    .find(|id| is_trick(st, id))
                    .cloned()
                    .ok_or_else(|| illegal("没有锦囊牌"))?;
                turn::take_card(st, target, &card)?;
                let label = turn::card_label(st, &card);
                turn::discard_by(st, Some(target), &card);
                st.push_log(
                    Some(&pid(st, target)),
                    format!("灭计：弃置锦囊牌【{label}】"),
                );
                pop(st);
                return Ok("灭计结算完成".to_string());
            }
            if action != "opt2" {
                return Err(illegal("需要选择一项"));
            }
            let jie = crate::skills::is_jiexian(st, f.actor);
            let cards: Vec<String> = st.seats[target]
                .hand
                .iter()
                .filter(|id| jie || !is_trick(st, id))
                .take(2)
                .cloned()
                .collect();
            if cards.len() < 2 {
                return Err(illegal("可选牌不足两张"));
            }
            for card in &cards {
                turn::take_card(st, target, card)?;
                let label = turn::card_label(st, card);
                turn::discard_by(st, Some(target), card);
                st.push_log(
                    Some(&pid(st, target)),
                    format!("灭计：弃置非锦囊牌【{label}】"),
                );
            }
            pop(st);
            Ok("灭计结算完成".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// 绝策（李儒）：结束阶段对一名没有手牌的其他角色造成 1 点伤害。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JueCeFrame {
    pub actor: usize,
}

pub(crate) fn juece_targets(st: &SgsState, actor: usize) -> Vec<usize> {
    // 界绝策（界李儒）：目标手牌数小于等于你（标准为无手牌）。
    let jie = crate::skills::is_jiexian(st, actor);
    let actor_hand = st.seats[actor].hand.len();
    st.alive_indices()
        .into_iter()
        .filter(|i| {
            *i != actor
                && if jie {
                    st.seats[*i].hand.len() <= actor_hand
                } else {
                    st.seats[*i].hand.is_empty()
                }
        })
        .collect()
}

pub(crate) fn step_juece(st: &mut SgsState, f: JueCeFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = juece_targets(st, f.actor)
        .into_iter()
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("对 {} 造成 1 点伤害", st.seats[i].general_name),
        })
        .collect();
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动绝策".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("juece".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::JueCe(f));
    Ok(true)
}

pub(crate) fn act_juece(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::JueCe(f)) => f,
        _ => return Err(internal("栈顶不是绝策帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动绝策".to_string());
    }
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择目标"))?;
    let target = st
        .seat_index(target_pid)
        .ok_or_else(|| illegal("目标不在座"))?;
    if !crate::skills::is_jiexian(st, f.actor) && !st.seats[target].hand.is_empty() {
        return Err(illegal("绝策目标必须没有手牌"));
    }
    st.push_log(
        Some(&pid(st, f.actor)),
        format!("绝策：对 {} 造成 1 点伤害", st.seats[target].general_name),
    );
    pop(st);
    crate::resolve::apply_damage(st, target, 1, Some(f.actor), DamageType::Normal, None);
    Ok("绝策结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 焚城（李儒，限定）：其他角色依次弃更多牌或受 2 点火焰伤害。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FenChengFrame {
    pub actor: usize,
    pub order: Vec<usize>,
    pub cursor: usize,
    pub last: usize,
}

pub(crate) fn step_fencheng(st: &mut SgsState, f: FenChengFrame) -> Result<bool, EngineError> {
    let mut cursor = f.cursor;
    while cursor < f.order.len() && !st.seats[f.order[cursor]].alive {
        cursor += 1;
    }
    if cursor >= f.order.len() {
        pop(st);
        return Ok(false);
    }
    let seat = f.order[cursor];
    let need = f.last + 1;
    let mut options = Vec::new();
    if st.seats[seat].hand.len() >= need {
        options.push(PendingOption {
            id: "opt1".into(),
            label: format!("焚城：弃置 {need} 张牌"),
        });
    }
    options.push(PendingOption {
        id: "opt2".into(),
        label: "焚城：受到 2 点火焰伤害".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, seat),
        skill: Some("fencheng".into()),
        options,
        data: json!({ "need": need }),
    });
    replace_top(st, Frame::FenCheng(FenChengFrame { cursor, ..f }));
    Ok(true)
}

pub(crate) fn act_fencheng(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::FenCheng(f)) => f,
        _ => return Err(internal("栈顶不是焚城帧")),
    };
    let seat = f.order[f.cursor];
    let need = f.last + 1;
    let mut next = f.clone();
    next.cursor += 1;
    if action == "opt1" {
        if !discard_random_hand(st, seat, need, "焚城") {
            return Err(illegal("手牌不足"));
        }
        next.last = need;
        st.push_log(Some(&pid(st, seat)), format!("焚城：弃置 {need} 张牌"));
        replace_top(st, Frame::FenCheng(next));
        return Ok("焚城：已弃牌".to_string());
    }
    if action != "opt2" {
        return Err(illegal("需要选择一项"));
    }
    st.push_log(Some(&pid(st, seat)), "焚城：受到 2 点火焰伤害".to_string());
    replace_top(st, Frame::FenCheng(next));
    crate::resolve::apply_damage(st, seat, 2, Some(f.actor), DamageType::Fire, None);
    Ok("焚城：受到 2 点火焰伤害".to_string())
}

// ---------------------------------------------------------------------------
// 恩怨（法正）：受伤后来源交一张手牌或失去 1 点体力。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EnYuanFrame {
    pub owner: usize,
    pub source: usize,
}

pub(crate) fn step_enyuan(st: &mut SgsState, f: EnYuanFrame) -> Result<bool, EngineError> {
    let mut options = Vec::new();
    // 界恩怨（界法正）：仅可交给红色手牌。
    let can_give = if crate::skills::is_jiexian(st, f.owner) {
        st.seats[f.source].hand.iter().any(|id| {
            crate::skills::effective_suit(st, f.source, id)
                .is_some_and(crate::skills::is_red)
        })
    } else {
        !st.seats[f.source].hand.is_empty()
    };
    if can_give {
        options.push(PendingOption {
            id: "opt1".into(),
            label: if crate::skills::is_jiexian(st, f.owner) {
                format!("交给 {} 一张红色手牌", st.seats[f.owner].general_name)
            } else {
                format!("交给 {} 一张手牌", st.seats[f.owner].general_name)
            },
        });
    }
    options.push(PendingOption {
        id: "opt2".into(),
        label: "失去 1 点体力".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.source),
        skill: Some("enyuan".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::EnYuan(f));
    Ok(true)
}

pub(crate) fn act_enyuan(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::EnYuan(f)) => f,
        _ => return Err(internal("栈顶不是恩怨帧")),
    };
    if action == "opt1" {
        let n = st.seats[f.source].hand.len();
        if n == 0 {
            return Err(illegal("没有手牌"));
        }
        let idx = if crate::skills::is_jiexian(st, f.owner) {
            st.seats[f.source]
                .hand
                .iter()
                .position(|id| {
                    crate::skills::effective_suit(st, f.source, id)
                        .is_some_and(crate::skills::is_red)
                })
                .ok_or_else(|| illegal("没有红色手牌"))?
        } else {
            st.next_index(n)
        };
        let card = st.seats[f.source].hand.remove(idx);
        let label = turn::card_label(st, &card);
        st.seats[f.owner].hand.push(card);
        st.push_log(
            Some(&pid(st, f.source)),
            format!(
                "恩怨：交给 {} 一张手牌【{label}】",
                st.seats[f.owner].general_name
            ),
        );
        cap_hand(st, f.owner);
        pop(st);
        return Ok("恩怨：已交出手牌".to_string());
    }
    if action != "opt2" {
        return Err(illegal("需要选择一项"));
    }
    st.seats[f.source].hp -= 1;
    let hp = st.seats[f.source].hp;
    st.push_log(
        Some(&pid(st, f.source)),
        format!("恩怨：失去 1 点体力（体力 {hp}）"),
    );
    crate::skills::zhaxiang_on_lose_hp(st, f.source);
    pop(st);
    if hp <= 0 {
        crate::resolve::push_dying(st, f.source, None);
    }
    Ok("恩怨：失去 1 点体力".to_string())
}

/// 恩怨（法正）获得牌钩子：获得一名其他角色至少两张牌后，令其摸一张（自动）。
pub(crate) fn enyuan_on_gain(st: &mut SgsState, gainer: usize, from: usize, n: usize) {
    if n < 2 || gainer == from || !crate::skills::has(st, gainer, "enyuan") {
        return;
    }
    let drawn = draw_n(st, from, 1);
    st.push_log(
        Some(&pid(st, gainer)),
        format!(
            "恩怨：{} 因你获得其 {n} 张牌摸 {drawn} 张",
            st.seats[from].general_name
        ),
    );
}

// ---------------------------------------------------------------------------
// 眩惑（法正）：摸牌阶段改为令一名其他角色摸 2 并二选一。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum XuanHuoStage {
    Offer,
    Target,
    VictimChoice,
    ShaTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct XuanHuoFrame {
    pub actor: usize,
    pub stage: XuanHuoStage,
    pub helper: Option<usize>,
}

pub(crate) fn step_xuanhuo(st: &mut SgsState, f: XuanHuoFrame) -> Result<bool, EngineError> {
    let (options, actor) = match f.stage {
        XuanHuoStage::Offer => (
            vec![
                PendingOption {
                    id: "opt1".into(),
                    label: "眩惑：改为令一名其他角色摸 2 张，然后其二选一".into(),
                },
                PendingOption {
                    id: "pass".into(),
                    label: "正常摸牌".into(),
                },
            ],
            f.actor,
        ),
        XuanHuoStage::Target => (
            st.alive_indices()
                .into_iter()
                .filter(|i| *i != f.actor)
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("令 {} 摸 2 张并选择", st.seats[i].general_name),
                })
                .collect(),
            f.actor,
        ),
        XuanHuoStage::VictimChoice => {
            let helper = f.helper.ok_or_else(|| internal("眩惑缺少协助者"))?;
            let mut options = Vec::new();
            if st
                .alive_indices()
                .into_iter()
                .any(|i| i != helper && st.in_attack_range(helper, i))
            {
                options.push(PendingOption {
                    id: "opt1".into(),
                    label: "视为对法正指定的角色使用一张【杀】".into(),
                });
            }
            options.push(PendingOption {
                id: "opt2".into(),
                label: format!("令 {} 获得你两张牌", st.seats[f.actor].general_name),
            });
            (options, helper)
        }
        XuanHuoStage::ShaTarget => {
            let helper = f.helper.ok_or_else(|| internal("眩惑缺少协助者"))?;
            (
                st.alive_indices()
                    .into_iter()
                    .filter(|i| *i != helper && st.in_attack_range(helper, *i))
                    .map(|i| PendingOption {
                        id: format!("target:{}", st.seats[i].player_id),
                        label: format!(
                            "令 {} 对 {} 使用【杀】",
                            st.seats[helper].general_name, st.seats[i].general_name
                        ),
                    })
                    .collect(),
                f.actor,
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
        skill: Some("xuanhuo".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::XuanHuo(f));
    Ok(true)
}

pub(crate) fn act_xuanhuo(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::XuanHuo(f)) => f,
        _ => return Err(internal("栈顶不是眩惑帧")),
    };
    match f.stage {
        XuanHuoStage::Offer => {
            if action == "pass" {
                pop(st);
                return Ok("眩惑：正常摸牌".to_string());
            }
            if action != "opt1" {
                return Err(illegal("需要选择一项"));
            }
            st.set_counter("skip_draw", 1);
            replace_top(
                st,
                Frame::XuanHuo(XuanHuoFrame {
                    stage: XuanHuoStage::Target,
                    ..f
                }),
            );
            Ok("眩惑：请选择角色".to_string())
        }
        XuanHuoStage::Target => {
            let target_pid = action
                .strip_prefix("target:")
                .ok_or_else(|| illegal("需要选择目标"))?;
            let helper = st
                .seat_index(target_pid)
                .ok_or_else(|| illegal("目标不在座"))?;
            let drawn = draw_n(st, helper, 2);
            st.push_log(
                Some(&pid(st, f.actor)),
                format!(
                    "眩惑：{} 摸 {drawn} 张并选择",
                    st.seats[helper].general_name
                ),
            );
            replace_top(
                st,
                Frame::XuanHuo(XuanHuoFrame {
                    stage: XuanHuoStage::VictimChoice,
                    helper: Some(helper),
                    ..f
                }),
            );
            Ok("眩惑：请协助者选择".to_string())
        }
        XuanHuoStage::VictimChoice => {
            let helper = f.helper.ok_or_else(|| internal("眩惑缺少协助者"))?;
            if action == "opt1" {
                replace_top(
                    st,
                    Frame::XuanHuo(XuanHuoFrame {
                        stage: XuanHuoStage::ShaTarget,
                        ..f
                    }),
                );
                return Ok("眩惑：请指定【杀】的目标".to_string());
            }
            if action != "opt2" {
                return Err(illegal("需要选择一项"));
            }
            let mut gained = 0;
            for _ in 0..2 {
                if let Some(card) = crate::resolve_jiexian::take_one_from_region(st, helper) {
                    let label = turn::card_label(st, &card);
                    st.seats[f.actor].hand.push(card);
                    st.push_log(
                        Some(&pid(st, f.actor)),
                        format!("眩惑：获得 {} 的【{label}】", st.seats[helper].general_name),
                    );
                    gained += 1;
                }
            }
            cap_hand(st, f.actor);
            enyuan_on_gain(st, f.actor, helper, gained);
            pop(st);
            Ok("眩惑结算完成".to_string())
        }
        XuanHuoStage::ShaTarget => {
            let helper = f.helper.ok_or_else(|| internal("眩惑缺少协助者"))?;
            let target_pid = action
                .strip_prefix("target:")
                .ok_or_else(|| illegal("需要选择目标"))?;
            let target = st
                .seat_index(target_pid)
                .ok_or_else(|| illegal("目标不在座"))?;
            st.push_log(
                Some(&pid(st, helper)),
                format!("眩惑：视为对 {} 使用【杀】", st.seats[target].general_name),
            );
            pop(st);
            push(
                st,
                Frame::Sha(ShaFrame {
                    card: String::new(),
                    user: helper,
                    target: Some(target),
                    stage: ShaStage::WaitRespond,
                    pending_targets: Vec::new(),
                    fangtian: false,
                    damage_type: DamageType::Normal,
                    no_distance: true,
                    virtual_card: true,
                    tianyi: false,
                    no_respond: false,
                }),
            );
            crate::resolve_skill::push_sha_target_response(
                st,
                helper,
                target,
                "",
                1,
                false,
                false,
                DamageType::Normal,
            );
            Ok("眩惑：视为使用【杀】".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// 举荐（徐庶）：结束阶段弃一张非基本牌，令一名其他角色三选一。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JuJianStage {
    Card,
    Target,
    Choice,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JuJianFrame {
    pub actor: usize,
    pub stage: JuJianStage,
    pub card: Option<String>,
    pub target: Option<usize>,
}

pub(crate) fn jujian_cards(st: &SgsState, actor: usize) -> Vec<String> {
    turn::hand_ids(st, actor)
        .into_iter()
        .filter(|id| card_type_bit(st, id) != 1)
        .collect()
}

pub(crate) fn step_jujian(st: &mut SgsState, f: JuJianFrame) -> Result<bool, EngineError> {
    let (options, actor) = match f.stage {
        JuJianStage::Card => {
            let mut options: Vec<PendingOption> = jujian_cards(st, f.actor)
                .into_iter()
                .map(|id| PendingOption {
                    id: format!("choose:{id}"),
                    label: format!("弃置 {}", turn::card_label(st, &id)),
                })
                .collect();
            options.push(PendingOption {
                id: "pass".into(),
                label: "不发动举荐".into(),
            });
            (options, f.actor)
        }
        JuJianStage::Target => (
            st.alive_indices()
                .into_iter()
                .filter(|i| *i != f.actor)
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("令 {} 三选一", st.seats[i].general_name),
                })
                .collect(),
            f.actor,
        ),
        JuJianStage::Choice => {
            let target = f.target.ok_or_else(|| internal("举荐缺少目标"))?;
            (
                vec![
                    PendingOption {
                        id: "opt1".into(),
                        label: "摸两张牌".into(),
                    },
                    PendingOption {
                        id: "opt2".into(),
                        label: "回复 1 点体力".into(),
                    },
                    PendingOption {
                        id: "opt3".into(),
                        label: "复原武将牌（翻回正面）".into(),
                    },
                ],
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
        skill: Some("jujian".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::JuJian(f));
    Ok(true)
}

pub(crate) fn act_jujian(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::JuJian(f)) => f,
        _ => return Err(internal("栈顶不是举荐帧")),
    };
    match f.stage {
        JuJianStage::Card => {
            if action == "pass" {
                pop(st);
                return Ok("不发动举荐".to_string());
            }
            let card = action
                .strip_prefix("choose:")
                .ok_or_else(|| illegal("需要选择一张非基本牌"))?;
            if !jujian_cards(st, f.actor).contains(&card.to_string()) {
                return Err(illegal("需要弃置一张非基本牌"));
            }
            turn::take_card(st, f.actor, card)?;
            let label = turn::card_label(st, card);
            turn::discard_by(st, Some(f.actor), card);
            st.push_log(Some(&pid(st, f.actor)), format!("举荐：弃置【{label}】"));
            replace_top(
                st,
                Frame::JuJian(JuJianFrame {
                    stage: JuJianStage::Target,
                    card: Some(card.to_string()),
                    ..f
                }),
            );
            Ok("举荐：请选择目标".to_string())
        }
        JuJianStage::Target => {
            let target_pid = action
                .strip_prefix("target:")
                .ok_or_else(|| illegal("需要选择目标"))?;
            let target = st
                .seat_index(target_pid)
                .ok_or_else(|| illegal("目标不在座"))?;
            replace_top(
                st,
                Frame::JuJian(JuJianFrame {
                    stage: JuJianStage::Choice,
                    target: Some(target),
                    ..f
                }),
            );
            Ok("举荐：请目标选择".to_string())
        }
        JuJianStage::Choice => {
            let target = f.target.ok_or_else(|| internal("举荐缺少目标"))?;
            let name = st.seats[target].general_name.clone();
            pop(st);
            match action {
                "opt1" => {
                    let drawn = draw_n(st, target, 2);
                    st.push_log(
                        Some(&pid(st, target)),
                        format!("举荐：{name} 摸 {drawn} 张"),
                    );
                    cap_hand(st, target);
                    Ok("举荐：摸两张".to_string())
                }
                "opt2" => {
                    if st.seats[target].hp < st.seats[target].max_hp {
                        st.seats[target].hp += 1;
                    }
                    let hp = st.seats[target].hp;
                    st.push_log(
                        Some(&pid(st, target)),
                        format!("举荐：{name} 回复 1 点体力（{hp}）"),
                    );
                    Ok("举荐：回复 1 点体力".to_string())
                }
                "opt3" => {
                    let was = crate::skills::restore_general(st, target);
                    st.push_log(
                        Some(&pid(st, target)),
                        format!(
                            "举荐：{name} {}",
                            if was {
                                "复原武将牌（翻回正面）"
                            } else {
                                "武将牌已正面朝上"
                            }
                        ),
                    );
                    Ok("举荐：复原武将牌".to_string())
                }
                _ => Err(illegal("需要选择一项")),
            }
        }
    }
}
