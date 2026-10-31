//! P4.4.2 一将成名帧（批次 1：曹植/高顺/陈宫/凌统/马谡/吴国太/徐盛/于禁）。
//!
//! 口径见 ADR-0024 与 `skills.md` §2.7：
//! - 同名去重（界优先，选将池）
//! - 弃牌堆来源钩子（落英/旋风）
//! - 移出游戏并回合结束回置（破军）
//! - 陷阵无视防具 / 追加目标 / 杀不计手牌上限

use crate::resolve::{pop, push, replace_top, top, DamageType, Frame, MultiSelectFrame};
use crate::skills;
use crate::state::*;
use crate::turn;
use crate::{illegal, internal};
use game_engine_core::EngineError;
use serde_json::json;

fn pass_option(label: &str) -> PendingOption {
    PendingOption {
        id: "pass".into(),
        label: label.into(),
    }
}

fn draw_capped(st: &mut SgsState, seat: usize, n: u32) -> usize {
    let mut ev = Vec::new();
    let drawn = turn::draw_cards(st, seat, n, &mut ev);
    let over = st.seats[seat]
        .hand
        .len()
        .saturating_sub(st.config.max_hand_cards as usize);
    if over > 0 {
        crate::resolve::push_discard(st, seat, over, "hand_cap");
    }
    drawn
}

fn general_name(st: &SgsState, seat: usize) -> String {
    st.seats[seat].general_name.clone()
}

fn equip_cards(st: &SgsState, seat: usize) -> Vec<(EquipSlot, String)> {
    EquipSlot::ALL
        .into_iter()
        .filter_map(|slot| {
            st.seats[seat]
                .equips
                .get(&slot)
                .and_then(|v| v.clone())
                .map(|card| (slot, card))
        })
        .collect()
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

/// 从目标处取一张牌：优先随机手牌，其次装备（用于简化口径的「弃置/移出/获得一张牌」）。
pub(crate) fn take_one_card_from(st: &mut SgsState, target: usize) -> Option<String> {
    if !st.seats[target].hand.is_empty() {
        let n = st.seats[target].hand.len();
        let idx = st.next_index(n);
        let card = st.seats[target].hand.remove(idx);
        skills::lianying_on_lose_last(st, target);
        return Some(card);
    }
    let (slot, card) = equip_cards(st, target).into_iter().next()?;
    st.seats[target].equips.insert(slot, None);
    skills::xiaoji_on_lose_equip(st, target);
    Some(card)
}

// ---------------------------------------------------------------------------
// 弃牌堆来源钩子（落英 / 旋风）
// ---------------------------------------------------------------------------

/// 牌进入弃牌堆（带来源座位与原因）；触发落英（其他角色梅花牌）。
pub(crate) fn on_card_discarded(st: &mut SgsState, from: Option<usize>, card: &str) {
    if !st.cards.contains_key(card) {
        return;
    }
    // 落英（曹植）：其他角色的梅花牌进入弃牌堆后，可获得之。
    if st.cards.get(card).is_some_and(|c| c.suit == Suit::Club) {
        if let Some(owner) = st.alive_indices().into_iter().find(|i| {
            skills::has(st, *i, "luoying") && Some(*i) != from
        }) {
            push(
                st,
                Frame::LuoYing(LuoYingFrame {
                    owner,
                    card: card.to_string(),
                }),
            );
        }
    }
}

/// 落英：从弃牌堆获得该牌（若仍在弃牌堆）。
pub(crate) fn luoying_gain(st: &mut SgsState, owner: usize, card: &str) -> bool {
    let Some(pos) = st.deck.discard.iter().position(|c| c == card) else {
        return false;
    };
    st.deck.discard.remove(pos);
    st.seats[owner].hand.push(card.to_string());
    crate::resolve_jiexian::qingjian_trigger(st, owner);
    let over = st.seats[owner]
        .hand
        .len()
        .saturating_sub(st.config.max_hand_cards as usize);
    if over > 0 {
        crate::resolve::push_discard(st, owner, over, "hand_cap");
    }
    true
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LuoYingFrame {
    pub owner: usize,
    pub card: String,
}

pub(crate) fn step_luoying(st: &mut SgsState, f: LuoYingFrame) -> Result<bool, EngineError> {
    if !st.deck.discard.iter().any(|c| c == &f.card) {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.owner].player_id.clone(),
        skill: Some("luoying".into()),
        options: vec![
            PendingOption {
                id: "choose:luoying".into(),
                label: format!("落英：获得【{}】", turn::card_label(st, &f.card)),
            },
            pass_option("不获得"),
        ],
        data: json!({ "skill": "luoying" }),
    });
    replace_top(st, Frame::LuoYing(f));
    Ok(true)
}

pub(crate) fn act_luoying(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::LuoYing(f)) => f,
        _ => return Err(internal("栈顶不是落英帧")),
    };
    pop(st);
    if action != "choose:luoying" {
        return Ok("不获得".to_string());
    }
    let label = turn::card_label(st, &f.card);
    if luoying_gain(st, f.owner, &f.card) {
        st.push_log(
            Some(&st.seats[f.owner].player_id.clone()),
            format!("落英：获得【{label}】"),
        );
        Ok(format!("落英：获得【{label}】"))
    } else {
        Ok("落英：该牌已不在弃牌堆".to_string())
    }
}

// ---------------------------------------------------------------------------
// 酒诗（曹植）：翻面视为使用【酒】；受伤后背面朝上可翻回
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JiuShiFrame {
    pub actor: usize,
}

pub(crate) fn step_jiushi(st: &mut SgsState, f: JiuShiFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.actor].player_id.clone(),
        options: vec![
            PendingOption {
                id: "choose:jiushi".into(),
                label: "酒诗：翻至背面，视为使用【酒】".into(),
            },
            pass_option("不发动"),
        ],
        skill: Some("jiushi".into()),
        data: json!({ "skill": "jiushi" }),
    });
    replace_top(st, Frame::JiuShi(f));
    Ok(true)
}

pub(crate) fn act_jiushi(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::JiuShi(f)) => f,
        _ => return Err(internal("栈顶不是酒诗帧")),
    };
    pop(st);
    if action != "choose:jiushi" {
        return Ok("不发动酒诗".to_string());
    }
    if st.seats[f.actor].flipped {
        return Ok("酒诗：已背面朝上".to_string());
    }
    st.seats[f.actor].flipped = true;
    st.set_counter("jiu_used", 1);
    st.set_counter("jiu", 1);
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        "酒诗：翻至背面，视为使用【酒】",
    );
    Ok("酒诗：已视为使用【酒】".to_string())
}

/// 酒诗：受到伤害后若背面朝上，可翻回正面（自动）。
pub(crate) fn jiushi_heal_flip(st: &mut SgsState, victim: usize) {
    if !skills::has(st, victim, "jiushi") || !st.seats[victim].flipped {
        return;
    }
    if st.seats[victim].hp <= 0 {
        return;
    }
    st.seats[victim].flipped = false;
    st.push_log(
        Some(&st.seats[victim].player_id.clone()),
        "酒诗：受伤后翻回正面",
    );
}

// ---------------------------------------------------------------------------
// 陷阵（高顺）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum XianZhenStage {
    Target,
    WaitPindian,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct XianZhenFrame {
    pub actor: usize,
    pub target: Option<usize>,
    pub stage: XianZhenStage,
}

pub(crate) fn step_xianzhen(st: &mut SgsState, f: XianZhenFrame) -> Result<bool, EngineError> {
    match f.stage {
        XianZhenStage::Target => {
            let options: Vec<PendingOption> = st
                .alive_indices()
                .into_iter()
                .filter(|i| *i != f.actor && !st.seats[*i].hand.is_empty())
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("陷阵：与 {} 拼点", general_name(st, i)),
                })
                .collect();
            if options.is_empty() || st.seats[f.actor].hand.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("xianzhen".into()),
                options,
                data: json!({ "stage": "xianzhen" }),
            });
            replace_top(st, Frame::XianZhen(f));
            Ok(true)
        }
        XianZhenStage::WaitPindian => match st.outcome.take() {
            Some(crate::resolve::Outcome::Pindian { winner, .. }) => {
                let target = f.target.unwrap_or(f.actor);
                let pid = st.seats[f.actor].player_id.clone();
                if winner == Some(f.actor) {
                    st.set_counter("xz_win", 1);
                    st.set_counter(&format!("xz_win:{}", f.actor), 1);
                    st.set_counter("xz_target", (target + 1) as u32);
                    st.set_counter("xz_no_armor", 1);
                    st.push_log(
                        Some(&pid),
                        format!(
                            "陷阵：拼点赢，本回合对 {} 无距离/次数限制并无视其防具",
                            general_name(st, target)
                        ),
                    );
                } else {
                    st.set_counter("xz_lose", 1);
                    st.set_counter("sha_blocked", 1);
                    st.push_log(Some(&pid), "陷阵：没赢，本回合不能使用【杀】且【杀】不计手牌上限");
                }
                pop(st);
                Ok(false)
            }
            _ => Err(internal("陷阵缺少拼点结果")),
        },
    }
}

pub(crate) fn xianzhen_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::XianZhen(f)) => f,
        _ => return Err(internal("栈顶不是陷阵帧")),
    };
    let pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择目标"))?;
    if !pending
        .options
        .iter()
        .any(|o| o.id == format!("target:{pid}"))
    {
        return Err(illegal("目标不合法"));
    }
    let target = st.seat_index(pid).ok_or_else(|| illegal("目标不在座"))?;
    replace_top(
        st,
        Frame::XianZhen(XianZhenFrame {
            actor: f.actor,
            target: Some(target),
            stage: XianZhenStage::WaitPindian,
        }),
    );
    crate::resolve_fire::start_pindian(st, f.actor, target, json!({ "skill": "xianzhen" }));
    Ok("陷阵：拼点开始".to_string())
}

/// 陷阵目标（拼点赢的目标）。
pub(crate) fn xianzhen_win_target(st: &SgsState) -> Option<usize> {
    let t = st.counter("xz_target");
    if t == 0 {
        None
    } else {
        Some(t as usize - 1)
    }
}

/// 陷阵无视防具（仅对被拼点目标且拼点赢）。
pub(crate) fn xianzhen_ignore_armor(st: &SgsState, victim: usize) -> bool {
    xianzhen_win_target(st) == Some(victim) && st.counter("xz_no_armor") > 0
}

// ---------------------------------------------------------------------------
// 明策（陈宫）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MingCeStage {
    Card,
    Target,
    Victory,
    Choice,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MingCeFrame {
    pub actor: usize,
    pub card: Option<String>,
    pub target: Option<usize>,
    pub stage: MingCeStage,
}

pub(crate) fn step_mingce(st: &mut SgsState, f: MingCeFrame) -> Result<bool, EngineError> {
    match f.stage {
        MingCeStage::Card => {
            replace_top(
                st,
                Frame::MultiSelect(MultiSelectFrame {
                    actor: f.actor,
                    purpose: "mingce".into(),
                    selected: Vec::new(),
                    min: 1,
                    max: 1,
                    data: json!({ "allowed": mingce_cards(st, f.actor) }),
                }),
            );
            Ok(false)
        }
        MingCeStage::Target => {
            let options: Vec<PendingOption> = st
                .alive_indices()
                .into_iter()
                .filter(|i| *i != f.actor)
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("明策：交给 {}", general_name(st, i)),
                })
                .collect();
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("mingce".into()),
                options,
                data: json!({ "stage": "mingce" }),
            });
            replace_top(st, Frame::MingCe(f));
            Ok(true)
        }
        MingCeStage::Victory => {
            let target = f.target.unwrap_or(f.actor);
            let options: Vec<PendingOption> = st
                .alive_indices()
                .into_iter()
                .filter(|i| {
                    *i != target
                        && *i != f.actor
                        && st.in_attack_range(target, *i)
                        && !skills::kongcheng_blocks(st, *i)
                        && !skills::deny_target(st, target, *i)
                })
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("明策：令 {} 视为对 {} 使用【杀】", general_name(st, target), general_name(st, i)),
                })
                .collect();
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("mingce".into()),
                options,
                data: json!({ "stage": "mingce_victim" }),
            });
            replace_top(st, Frame::MingCe(f));
            Ok(true)
        }
        MingCeStage::Choice => {
            let target = f.target.unwrap_or(f.actor);
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: st.seats[target].player_id.clone(),
                skill: Some("mingce".into()),
                options: vec![
                    PendingOption {
                        id: "choose:mingce:sha".into(),
                        label: "明策：视为对陈宫指定的角色使用【杀】".into(),
                    },
                    PendingOption {
                        id: "choose:mingce:draw".into(),
                        label: "明策：摸一张牌".into(),
                    },
                ],
                data: json!({ "skill": "mingce" }),
            });
            replace_top(st, Frame::MingCe(f));
            Ok(true)
        }
    }
}

/// 明策可交出的手牌（【杀】或装备牌）。
pub(crate) fn mingce_cards(st: &SgsState, actor: usize) -> Vec<String> {
    st.seats[actor]
        .hand
        .iter()
        .filter(|id| {
            turn::card_def(st, id).is_some_and(|d| {
                matches!(d.code, "sha" | "huo_sha" | "lei_sha") || d.equip_slot.is_some()
            })
        })
        .cloned()
        .collect()
}

pub(crate) fn mingce_confirm(
    st: &mut SgsState,
    f: &MultiSelectFrame,
) -> Result<String, EngineError> {
    if f.selected.len() != 1 {
        return Err(illegal("明策需选择 1 张牌"));
    }
    replace_top(
        st,
        Frame::MingCe(MingCeFrame {
            actor: f.actor,
            card: Some(f.selected[0].clone()),
            target: None,
            stage: MingCeStage::Target,
        }),
    );
    Ok("明策：请选择交牌角色".to_string())
}

pub(crate) fn mingce_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::MingCe(f)) => f,
        _ => return Err(internal("栈顶不是明策帧")),
    };
    let stage = pending.data["stage"].as_str().unwrap_or("mingce");
    let pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择目标"))?;
    if !pending
        .options
        .iter()
        .any(|o| o.id == format!("target:{pid}"))
    {
        return Err(illegal("目标不合法"));
    }
    let seat = st.seat_index(pid).ok_or_else(|| illegal("目标不在座"))?;
    if stage == "mingce_victim" {
        // 视为使用【杀】（虚拟杀，不可响应保持默认）。
        let user = f.target.unwrap_or(f.actor);
        pop(st);
        if let Some(card) = f.card.clone() {
            let _ = card;
        }
        st.push_log(
            Some(&st.seats[user].player_id.clone()),
            format!("明策：视为对 {} 使用【杀】", general_name(st, seat)),
        );
        // 虚拟【杀】：不产生牌实例（不进处理区，保持牌数守恒）。
        crate::resolve::push(
            st,
            Frame::Sha(crate::resolve::ShaFrame {
                card: String::new(),
                user,
                target: Some(seat),
                stage: crate::resolve::ShaStage::WaitRespond,
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
            seat,
            "",
            1,
            false,
            false,
            DamageType::Normal,
        );
        return Ok("明策：视为使用【杀】".to_string());
    }
    // 交给目标牌 → 目标二选一。
    let Some(card) = f.card.clone() else {
        return Err(illegal("明策缺少牌"));
    };
    if turn::take_card(st, f.actor, &card).is_err() {
        return Err(illegal("手牌中没有该牌"));
    }
    st.seats[seat].hand.push(card.clone());
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        format!(
            "明策：将【{}】交给 {}",
            turn::card_label(st, &card),
            general_name(st, seat)
        ),
    );
    replace_top(
        st,
        Frame::MingCe(MingCeFrame {
            actor: f.actor,
            card: Some(card),
            target: Some(seat),
            stage: MingCeStage::Choice,
        }),
    );
    Ok("明策：等待目标选择".to_string())
}

pub(crate) fn act_mingce(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::MingCe(f)) => f,
        _ => return Err(internal("栈顶不是明策帧")),
    };
    let Some(target) = f.target else {
        return Err(illegal("明策缺少目标"));
    };
    if action == "choose:mingce:draw" {
        pop(st);
        let drawn = draw_capped(st, target, 1);
        st.push_log(
            Some(&st.seats[target].player_id.clone()),
            format!("明策：{} 选择摸 {drawn} 张", general_name(st, target)),
        );
        return Ok("明策：目标摸牌".to_string());
    }
    if action != "choose:mingce:sha" {
        return Err(illegal("未知明策选项"));
    }
    replace_top(
        st,
        Frame::MingCe(MingCeFrame {
            actor: f.actor,
            card: f.card,
            target: Some(target),
            stage: MingCeStage::Victory,
        }),
    );
    Ok("明策：请选择【杀】的目标".to_string())
}

// ---------------------------------------------------------------------------
// 智迟（陈宫，锁定）：回合外受到伤害后，本回合【杀】与普通锦囊对其无效
// ---------------------------------------------------------------------------

pub(crate) fn zhichi_active(st: &SgsState, seat: usize) -> bool {
    st.counter(&format!("zhichi:{seat}")) > 0
}

pub(crate) fn zhichi_on_damage(st: &mut SgsState, victim: usize) {
    if !skills::has(st, victim, "zhichi") || st.turn == victim {
        return;
    }
    if st.seats[victim].hp <= 0 {
        return;
    }
    st.set_counter(&format!("zhichi:{victim}"), 1);
    st.push_log(
        Some(&st.seats[victim].player_id.clone()),
        "智迟：本回合【杀】与普通锦囊对你无效",
    );
}

// ---------------------------------------------------------------------------
// 旋风（凌统）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct XuanFengFrame {
    pub owner: usize,
    pub remaining: u32,
    pub chosen: Vec<usize>,
}

pub(crate) fn push_xuanfeng(st: &mut SgsState) {
    let Some(owner) = st
        .alive_indices()
        .into_iter()
        .find(|i| skills::has(st, *i, "xuanfeng"))
    else {
        return;
    };
    if st.counter(&format!("xuanfeng:{}", owner)) > 0 {
        return;
    }
    st.set_counter(&format!("xuanfeng:{}", owner), 1);
    push(
        st,
        Frame::XuanFeng(XuanFengFrame {
            owner,
            remaining: 2,
            chosen: Vec::new(),
        }),
    );
}

pub(crate) fn step_xuanfeng(st: &mut SgsState, f: XuanFengFrame) -> Result<bool, EngineError> {
    if f.remaining == 0 {
        pop(st);
        return Ok(false);
    }
    let mut options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| {
            *i != f.owner
                && (!st.seats[*i].hand.is_empty()
                    || st.seats[*i].equips.values().any(|c| c.is_some()))
        })
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("旋风：弃置 {} 一张牌", general_name(st, i)),
        })
        .collect();
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    options.push(pass_option("停止发动旋风"));
    st.pending = Some(Pending {
        kind: "target".into(),
        actor: st.seats[f.owner].player_id.clone(),
        skill: Some("xuanfeng".into()),
        options,
        data: json!({ "stage": "xuanfeng", "remaining": f.remaining }),
    });
    replace_top(st, Frame::XuanFeng(f));
    Ok(true)
}

pub(crate) fn xuanfeng_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::XuanFeng(f)) => f,
        _ => return Err(internal("栈顶不是旋风帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("停止发动旋风".to_string());
    }
    let pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择目标"))?;
    if !pending
        .options
        .iter()
        .any(|o| o.id == format!("target:{pid}"))
    {
        return Err(illegal("目标不合法"));
    }
    let target = st.seat_index(pid).ok_or_else(|| illegal("目标不在座"))?;
    if let Some(card) = take_one_card_from(st, target) {
        st.push_log(
            Some(&st.seats[f.owner].player_id.clone()),
            format!(
                "旋风：弃置 {} 的【{}】",
                general_name(st, target),
                turn::card_label(st, &card)
            ),
        );
        turn::to_discard_card(st, &card);
        on_card_discarded(st, Some(target), &card);
    }
    let mut chosen = f.chosen.clone();
    chosen.push(target);
    let remaining = f.remaining.saturating_sub(1);
    if remaining == 0 {
        pop(st);
        Ok("旋风：已完成".to_string())
    } else {
        replace_top(
            st,
            Frame::XuanFeng(XuanFengFrame {
                owner: f.owner,
                remaining,
                chosen,
            }),
        );
        Ok("旋风：可继续选择".to_string())
    }
}

// ---------------------------------------------------------------------------
// 散谣 / 制蛮（马谡）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SanYaoMode {
    Hp,
    Hand,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SanYaoFrame {
    pub actor: usize,
    pub mode: SanYaoMode,
    pub stage: crate::resolve_sp::YiliStage,
}

pub(crate) fn step_sanyao(st: &mut SgsState, f: SanYaoFrame) -> Result<bool, EngineError> {
    match f.stage {
        crate::resolve_sp::YiliStage::Cost => {
            replace_top(
                st,
                Frame::MultiSelect(MultiSelectFrame {
                    actor: f.actor,
                    purpose: "sanyao".into(),
                    selected: Vec::new(),
                    min: 1,
                    max: 1,
                    data: json!({ "mode": if f.mode == SanYaoMode::Hp { "hp" } else { "hand" } }),
                }),
            );
            Ok(false)
        }
        crate::resolve_sp::YiliStage::Target => {
            let options: Vec<PendingOption> = sanyao_targets(st, f.actor, f.mode)
                .into_iter()
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("散谣：对 {} 造成 1 点伤害", general_name(st, i)),
                })
                .collect();
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("sanyao".into()),
                options,
                data: json!({ "stage": "sanyao" }),
            });
            replace_top(st, Frame::SanYao(f));
            Ok(true)
        }
    }
}

pub(crate) fn sanyao_targets(st: &SgsState, actor: usize, mode: SanYaoMode) -> Vec<usize> {
    let others: Vec<usize> = st.alive_indices().into_iter().filter(|i| *i != actor).collect();
    if others.is_empty() {
        return Vec::new();
    }
    let key = |st: &SgsState, i: usize| match mode {
        SanYaoMode::Hp => st.seats[i].hp,
        SanYaoMode::Hand => st.seats[i].hand.len() as i32,
    };
    let best = others.iter().map(|i| key(st, *i)).max().unwrap_or(0);
    others
        .into_iter()
        .filter(|i| key(st, *i) == best)
        .collect()
}

pub(crate) fn act_sanyao(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    if action == "pass" {
        pop(st);
        return Ok("不发动散谣".to_string());
    }
    Err(illegal("请通过多选/目标界面继续散谣"))
}

pub(crate) fn sanyao_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::SanYao(f)) => f,
        _ => return Err(internal("栈顶不是散谣帧")),
    };
    let pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择目标"))?;
    if !pending
        .options
        .iter()
        .any(|o| o.id == format!("target:{pid}"))
    {
        return Err(illegal("目标不合法"));
    }
    let target = st.seat_index(pid).ok_or_else(|| illegal("目标不在座"))?;
    pop(st);
    st.set_counter(
        match f.mode {
            SanYaoMode::Hp => "sanyao_hp_used",
            SanYaoMode::Hand => "sanyao_hand_used",
        },
        1,
    );
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        format!("散谣：对 {} 造成 1 点伤害", general_name(st, target)),
    );
    crate::resolve::apply_damage(st, target, 1, Some(f.actor), DamageType::Normal, None);
    Ok("散谣：已完成".to_string())
}

pub(crate) fn sanyao_confirm(
    st: &mut SgsState,
    f: &MultiSelectFrame,
) -> Result<String, EngineError> {
    if f.selected.len() != 1 {
        return Err(illegal("散谣需弃置 1 张牌"));
    }
    let card = f.selected[0].clone();
    if turn::take_card(st, f.actor, &card).is_err() {
        return Err(illegal("手牌中没有该牌"));
    }
    turn::to_discard_card(st, &card);
    on_card_discarded(st, Some(f.actor), &card);
    let mode = if f.data["mode"].as_str() == Some("hp") {
        SanYaoMode::Hp
    } else {
        SanYaoMode::Hand
    };
    replace_top(
        st,
        Frame::SanYao(SanYaoFrame {
            actor: f.actor,
            mode,
            stage: crate::resolve_sp::YiliStage::Target,
        }),
    );
    Ok("散谣：请选择目标".to_string())
}

/// 制蛮（马谡）：造成伤害时可防止并获得其一张牌。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZhiManFrame {
    pub owner: usize,
    pub victim: usize,
    pub amount: i32,
    pub damage_type: DamageType,
    pub card: Option<String>,
}

pub(crate) fn zhiman_trigger(
    st: &mut SgsState,
    owner: usize,
    victim: usize,
    amount: i32,
    damage_type: DamageType,
    card: Option<&str>,
) -> bool {
    if !skills::has(st, owner, "zhiman") || owner == victim {
        return false;
    }
    if st.seats[victim].hand.is_empty() && equip_cards(st, victim).is_empty() {
        return false;
    }
    push(
        st,
        Frame::ZhiMan(ZhiManFrame {
            owner,
            victim,
            amount,
            damage_type,
            card: card.map(str::to_string),
        }),
    );
    true
}

pub(crate) fn step_zhiman(st: &mut SgsState, f: ZhiManFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.owner].player_id.clone(),
        skill: Some("zhiman".into()),
        options: vec![
            PendingOption {
                id: "choose:zhiman".into(),
                label: format!("制蛮：防止此伤害并获得 {} 一张牌", general_name(st, f.victim)),
            },
            pass_option("不发动制蛮"),
        ],
        data: json!({ "skill": "zhiman" }),
    });
    replace_top(st, Frame::ZhiMan(f));
    Ok(true)
}

pub(crate) fn act_zhiman(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ZhiMan(f)) => f,
        _ => return Err(internal("栈顶不是制蛮帧")),
    };
    pop(st);
    if action != "choose:zhiman" {
        // 不发动：按原伤害继续结算。
        crate::resolve::apply_damage_now(
            st,
            f.victim,
            f.amount,
            Some(f.owner),
            f.damage_type,
            f.card.as_deref(),
        );
        return Ok("不发动制蛮".to_string());
    }
    if let Some(card) = take_one_card_from(st, f.victim) {
        st.seats[f.owner].hand.push(card.clone());
        st.push_log(
            Some(&st.seats[f.owner].player_id.clone()),
            format!(
                "制蛮：防止伤害并获得 {} 的【{}】",
                general_name(st, f.victim),
                turn::card_label(st, &card)
            ),
        );
        st.set_counter("zhiman_activated", 1);
    }
    Ok("制蛮：已防止伤害".to_string())
}

// ---------------------------------------------------------------------------
// 甘露 / 补益（吴国太）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GanLuStage {
    First,
    Second,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GanLuFrame {
    pub actor: usize,
    pub stage: GanLuStage,
    pub first: Option<usize>,
}

pub(crate) fn step_ganlu(st: &mut SgsState, f: GanLuFrame) -> Result<bool, EngineError> {
    let first = f.first;
    let options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| *i != f.actor && !equip_cards(st, *i).is_empty())
        .filter(|i| Some(*i) != first)
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("甘露：{}（装备 {} 张）", general_name(st, i), equip_cards(st, i).len()),
        })
        .collect();
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "target".into(),
        actor: st.seats[f.actor].player_id.clone(),
        options,
        skill: Some("ganlu".into()),
        data: json!({ "stage": "ganlu" }),
    });
    replace_top(st, Frame::GanLu(f));
    Ok(true)
}

pub(crate) fn ganlu_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::GanLu(f)) => f,
        _ => return Err(internal("栈顶不是甘露帧")),
    };
    let pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择目标"))?;
    if !pending
        .options
        .iter()
        .any(|o| o.id == format!("target:{pid}"))
    {
        return Err(illegal("目标不合法"));
    }
    let seat = st.seat_index(pid).ok_or_else(|| illegal("目标不在座"))?;
    match f.stage {
        GanLuStage::First => {
            replace_top(
                st,
                Frame::GanLu(GanLuFrame {
                    actor: f.actor,
                    stage: GanLuStage::Second,
                    first: Some(seat),
                }),
            );
            Ok("甘露：请选择第二名角色".to_string())
        }
        GanLuStage::Second => {
            let a = f.first.unwrap_or(seat);
            let b = seat;
            let lost = (st.seats[f.actor].max_hp - st.seats[f.actor].hp).max(0) as usize;
            let na = equip_cards(st, a).len();
            let nb = equip_cards(st, b).len();
            pop(st);
            if na + nb == 0 || na.abs_diff(nb) > lost {
                st.push_log(
                    Some(&st.seats[f.actor].player_id.clone()),
                    "甘露：条件不满足，未交换装备区",
                );
                return Ok("甘露：条件不满足，未交换".to_string());
            }
            swap_equips(st, a, b);
            st.set_counter("ganlu_used", 1);
            st.push_log(
                Some(&st.seats[f.actor].player_id.clone()),
                format!(
                    "甘露：交换 {} 与 {} 装备区",
                    general_name(st, a),
                    general_name(st, b)
                ),
            );
            Ok("甘露：已交换装备区".to_string())
        }
    }
}

fn swap_equips(st: &mut SgsState, a: usize, b: usize) {
    let ea: Vec<(EquipSlot, Option<String>)> = EquipSlot::ALL
        .into_iter()
        .map(|slot| (slot, st.seats[a].equips.get(&slot).cloned().flatten()))
        .collect();
    let eb: Vec<(EquipSlot, Option<String>)> = EquipSlot::ALL
        .into_iter()
        .map(|slot| (slot, st.seats[b].equips.get(&slot).cloned().flatten()))
        .collect();
    for (slot, card) in eb {
        st.seats[a].equips.insert(slot, card);
    }
    for (slot, card) in ea {
        st.seats[b].equips.insert(slot, card);
    }
}

/// 补益：角色进入濒死时，展示其一张手牌；非基本牌则弃置并回复 1。
pub(crate) fn buyi_on_dying(st: &mut SgsState, victim: usize) -> bool {
    let Some(owner) = st
        .alive_indices()
        .into_iter()
        .find(|i| *i != victim && skills::has(st, *i, "buyi"))
    else {
        return false;
    };
    if st.seats[victim].hand.is_empty() || st.counter(&format!("buyi:{}", victim)) > 0 {
        return false;
    }
    st.set_counter(&format!("buyi:{}", victim), 1);
    let n = st.seats[victim].hand.len();
    let idx = st.next_index(n);
    let card = st.seats[victim].hand[idx].clone();
    let is_basic = turn::card_def(st, &card)
        .is_some_and(|d| d.category == Category::Basic);
    let label = turn::card_label(st, &card);
    if is_basic {
        st.push_log(
            Some(&st.seats[owner].player_id.clone()),
            format!("补益：{} 亮出【{label}】（基本牌，无效）", general_name(st, victim)),
        );
        return false;
    }
    turn::take_card(st, victim, &card).ok();
    turn::to_discard_card(st, &card);
    st.seats[victim].hp += 1;
    st.push_log(
        Some(&st.seats[owner].player_id.clone()),
        format!(
            "补益：{} 弃置【{label}】并回复 1 点体力",
            general_name(st, victim)
        ),
    );
    st.seats[victim].hp > 0
}

// ---------------------------------------------------------------------------
// 破军（徐盛）：杀指定目标后，将至多 X 张牌移出游戏直到回合结束
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PoJunFrame {
    pub user: usize,
    pub target: usize,
    pub card: String,
    pub remaining: u32,
}

pub(crate) fn push_pojun(st: &mut SgsState, user: usize, target: usize, card: &str) {
    if !skills::has(st, user, "pojun") {
        return;
    }
    let x = st.seats[target].hp.max(0) as u32;
    if x == 0 {
        return;
    }
    if st.seats[target].hand.is_empty() && equip_cards(st, target).is_empty() {
        return;
    }
    push(
        st,
        Frame::PoJun(PoJunFrame {
            user,
            target,
            card: card.to_string(),
            remaining: x,
        }),
    );
}

pub(crate) fn step_pojun(st: &mut SgsState, f: PoJunFrame) -> Result<bool, EngineError> {
    if f.remaining == 0
        || (st.seats[f.target].hand.is_empty() && equip_cards(st, f.target).is_empty())
    {
        pop(st);
        return Ok(false);
    }
    let mut options: Vec<PendingOption> = Vec::new();
    if !st.seats[f.target].hand.is_empty() {
        options.push(PendingOption {
            id: "choose:pojun:hand".into(),
            label: format!("破军：移出目标一张手牌（共 {} 张）", st.seats[f.target].hand.len()),
        });
    }
    for (slot, card) in equip_cards(st, f.target) {
        options.push(PendingOption {
            id: format!("choose:pojun:equip:{}", slot_code(slot)),
            label: format!("破军：移出装备【{}】", turn::card_label(st, &card)),
        });
    }
    options.push(pass_option("停止移出"));
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.user].player_id.clone(),
        skill: Some("pojun".into()),
        options,
        data: json!({ "skill": "pojun", "remaining": f.remaining }),
    });
    replace_top(st, Frame::PoJun(f));
    Ok(true)
}

pub(crate) fn act_pojun(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::PoJun(f)) => f,
        _ => return Err(internal("栈顶不是破军帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("停止移出".to_string());
    }
    let card = if action == "choose:pojun:hand" {
        if st.seats[f.target].hand.is_empty() {
            return Err(illegal("目标没有手牌"));
        }
        let n = st.seats[f.target].hand.len();
        let idx = st.next_index(n);
        let card = st.seats[f.target].hand.remove(idx);
        skills::lianying_on_lose_last(st, f.target);
        card
    } else if let Some(code) = action.strip_prefix("choose:pojun:equip:") {
        let slot = slot_from_code(code).ok_or_else(|| illegal("非法装备槽"))?;
        let card = st.seats[f.target]
            .equips
            .get(&slot)
            .cloned()
            .flatten()
            .ok_or_else(|| illegal("该槽位没有装备"))?;
        st.seats[f.target].equips.insert(slot, None);
        skills::xiaoji_on_lose_equip(st, f.target);
        card
    } else {
        return Err(illegal("未知破军选项"));
    };
    st.deck.void.push(card.clone());
    st.void_return.push(crate::state::VoidReturn {
        owner: f.target,
        card: card.clone(),
        due_seat: st.turn,
    });
    st.push_log(
        Some(&st.seats[f.user].player_id.clone()),
        format!(
            "破军：将 {} 的【{}】移出游戏直到回合结束",
            general_name(st, f.target),
            turn::card_label(st, &card)
        ),
    );
    let remaining = f.remaining.saturating_sub(1);
    if remaining == 0
        || (st.seats[f.target].hand.is_empty() && equip_cards(st, f.target).is_empty())
    {
        pop(st);
        Ok("破军：已完成".to_string())
    } else {
        replace_top(
            st,
            Frame::PoJun(PoJunFrame {
                remaining,
                ..f
            }),
        );
        Ok("破军：可继续移出".to_string())
    }
}

// ---------------------------------------------------------------------------
// 镇军（于禁）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZhenJunStage {
    Target,
    Choice,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZhenJunFrame {
    pub actor: usize,
    pub stage: ZhenJunStage,
    pub target: Option<usize>,
    /// 已弃置目标牌里的非装备牌数（用于二选一）。
    pub non_equip: u32,
}

pub(crate) fn step_zhenjun(st: &mut SgsState, f: ZhenJunFrame) -> Result<bool, EngineError> {
    match f.stage {
        ZhenJunStage::Target => {
            let options: Vec<PendingOption> = st
                .alive_indices()
                .into_iter()
                .filter(|i| zhenjun_x(st, *i) > 0)
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("镇军：{}（需弃 {} 张）", general_name(st, i), zhenjun_x(st, i)),
                })
                .collect();
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("zhenjun".into()),
                options,
                data: json!({ "stage": "zhenjun" }),
            });
            replace_top(st, Frame::ZhenJun(f));
            Ok(true)
        }
        ZhenJunStage::Choice => {
            let target = f.target.unwrap_or(f.actor);
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("zhenjun".into()),
                options: vec![
                    PendingOption {
                        id: "choose:zhenjun:discard".into(),
                        label: format!("镇军：你弃置 {} 张牌", f.non_equip),
                    },
                    PendingOption {
                        id: "choose:zhenjun:draw".into(),
                        label: format!("镇军：令 {} 摸 {} 张", general_name(st, target), f.non_equip),
                    },
                ],
                data: json!({ "skill": "zhenjun" }),
            });
            replace_top(st, Frame::ZhenJun(f));
            Ok(true)
        }
    }
}

/// 镇军 X：目标手牌数 - 体力值，至少 1。
pub(crate) fn zhenjun_x(st: &SgsState, target: usize) -> u32 {
    let hand = st.seats[target].hand.len() as i32;
    let hp = st.seats[target].hp.max(0);
    ((hand - hp).max(1)) as u32
}

pub(crate) fn zhenjun_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ZhenJun(f)) => f,
        _ => return Err(internal("栈顶不是镇军帧")),
    };
    let pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择目标"))?;
    if !pending
        .options
        .iter()
        .any(|o| o.id == format!("target:{pid}"))
    {
        return Err(illegal("目标不合法"));
    }
    let target = st.seat_index(pid).ok_or_else(|| illegal("目标不在座"))?;
    let x = zhenjun_x(st, target).min(st.seats[target].hand.len() as u32);
    let mut non_equip = 0;
    for _ in 0..x {
        if let Some(card) = take_one_card_from(st, target) {
            let is_equip = turn::card_def(st, &card).is_some_and(|d| d.equip_slot.is_some());
            if !is_equip {
                non_equip += 1;
            }
            turn::to_discard_card(st, &card);
            on_card_discarded(st, Some(target), &card);
        }
    }
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        format!("镇军：弃置 {} {x} 张牌", general_name(st, target)),
    );
    if non_equip == 0 {
        pop(st);
        return Ok("镇军：无高牌，结算完毕".to_string());
    }
    replace_top(
        st,
        Frame::ZhenJun(ZhenJunFrame {
            actor: f.actor,
            stage: ZhenJunStage::Choice,
            target: Some(target),
            non_equip,
        }),
    );
    Ok("镇军：请选择后续".to_string())
}

pub(crate) fn act_zhenjun(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ZhenJun(f)) => f,
        _ => return Err(internal("栈顶不是镇军帧")),
    };
    let target = f.target.unwrap_or(f.actor);
    pop(st);
    let n = f.non_equip;
    if action == "choose:zhenjun:draw" {
        let drawn = draw_capped(st, target, n);
        st.push_log(
            Some(&st.seats[target].player_id.clone()),
            format!("镇军：{} 摸 {drawn} 张", general_name(st, target)),
        );
        return Ok("镇军：目标摸牌".to_string());
    }
    // 自己弃置等量牌（手牌随机）
    let mut dropped = 0;
    for _ in 0..n {
        if let Some(card) = take_one_card_from(st, f.actor) {
            turn::to_discard_card(st, &card);
            on_card_discarded(st, Some(f.actor), &card);
            dropped += 1;
        }
    }
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        format!("镇军：弃置 {dropped} 张牌"),
    );
    Ok("镇军：已完成".to_string())
}
