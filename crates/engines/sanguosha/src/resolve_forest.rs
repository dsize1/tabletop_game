//! P4.4.1 林包技能帧：行殇 / 放逐 / 英魂 / 崩坏 / 烈刃 / 再起 / 好施 / 缔盟 / 乱武。

use crate::resolve::{pop, push, replace_top, top, DamageType, Frame, Outcome};
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

fn zone_cards(st: &mut SgsState, seat: usize) -> Vec<String> {
    let mut out = std::mem::take(&mut st.seats[seat].hand);
    out.extend(st.seats[seat].equips.values_mut().filter_map(Option::take));
    out.extend(std::mem::take(&mut st.seats[seat].judge_zone));
    out
}

// ---------------------------------------------------------------------------
// 行殇（曹丕）：其他角色死亡时，可获得其所有牌
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct XingshangFrame {
    pub victim: usize,
    pub owner: usize,
}

pub(crate) fn step_xingshang(st: &mut SgsState, f: XingshangFrame) -> Result<bool, EngineError> {
    let has_cards = !st.seats[f.victim].hand.is_empty()
        || !st.seats[f.victim].judge_zone.is_empty()
        || st.seats[f.victim].equips.values().any(|c| c.is_some());
    if !has_cards {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.owner].player_id.clone(),
        skill: Some("xingshang".into()),
        options: vec![
            PendingOption {
                id: "choose:xingshang".into(),
                label: format!("行殇：获得 {} 的所有牌", st.seats[f.victim].general_name),
            },
            pass_option("不发动行殇"),
        ],
        data: json!({ "skill": "xingshang", "victim": st.seats[f.victim].player_id }),
    });
    replace_top(st, Frame::Xingshang(f));
    Ok(true)
}

pub(crate) fn act_xingshang(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Xingshang(f)) => f,
        _ => return Err(internal("栈顶不是行殇帧")),
    };
    let cards = zone_cards(st, f.victim);
    let pid = st.seats[f.owner].player_id.clone();
    if action == "choose:xingshang" {
        let n = cards.len();
        for card in cards {
            st.seats[f.owner].hand.push(card);
        }
        let over = st.seats[f.owner]
            .hand
            .len()
            .saturating_sub(st.config.max_hand_cards as usize);
        if over > 0 {
            crate::resolve::push_discard(st, f.owner, over, "hand_cap");
        }
        st.push_log(
            Some(&pid),
            format!("行殇：获得 {} 的 {n} 张牌", st.seats[f.victim].general_name),
        );
        pop(st);
        Ok(format!("行殇：获得 {n} 张牌"))
    } else {
        for card in cards {
            turn::to_discard_card(st, &card);
        }
        pop(st);
        Ok("不发动行殇".to_string())
    }
}

// ---------------------------------------------------------------------------
// 放逐（曹丕）：受到伤害后，令一名其他角色翻面并摸 X 张
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FangzhuFrame {
    pub actor: usize,
}

pub(crate) fn step_fangzhu(st: &mut SgsState, f: FangzhuFrame) -> Result<bool, EngineError> {
    let options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| *i != f.actor)
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("放逐：令 {} 翻面并摸牌", st.seats[i].general_name),
        })
        .collect();
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "target".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("fangzhu".into()),
        options,
        data: json!({ "stage": "fangzhu" }),
    });
    replace_top(st, Frame::Fangzhu(f));
    Ok(true)
}

pub(crate) fn fangzhu_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Fangzhu(f)) => f,
        _ => return Err(internal("栈顶不是放逐帧")),
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
    st.seats[target].flipped = !st.seats[target].flipped;
    crate::resolve_jiexian3::shebian_on_flip(st, target);
    let lost = (st.seats[f.actor].max_hp - st.seats[f.actor].hp).max(0) as u32;
    let drawn = draw_capped(st, target, lost.min(5));
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        format!(
            "放逐：令 {} 翻面并摸 {drawn} 张",
            st.seats[target].general_name
        ),
    );
    Ok(format!(
        "放逐：{} 翻面并摸 {drawn} 张",
        st.seats[target].general_name
    ))
}

// ---------------------------------------------------------------------------
// 英魂（孙坚）：准备阶段已受伤 → 选一名其他角色二选一
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum YinghunStage {
    Target,
    Option,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct YinghunFrame {
    pub actor: usize,
    pub target: Option<usize>,
    pub stage: YinghunStage,
}

pub(crate) fn step_yinghun(st: &mut SgsState, f: YinghunFrame) -> Result<bool, EngineError> {
    let x = (st.seats[f.actor].max_hp - st.seats[f.actor].hp).max(0);
    match f.stage {
        YinghunStage::Target => {
            let options: Vec<PendingOption> = st
                .alive_indices()
                .into_iter()
                .filter(|i| *i != f.actor)
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("英魂：选择 {}", st.seats[i].general_name),
                })
                .collect();
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("yinghun".into()),
                options,
                data: json!({ "stage": "yinghun" }),
            });
            replace_top(st, Frame::Yinghun(f));
            Ok(true)
        }
        YinghunStage::Option => {
            let Some(target) = f.target else {
                pop(st);
                return Ok(false);
            };
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("yinghun".into()),
                options: vec![
                    PendingOption {
                        id: "choose:yinghun:1".into(),
                        label: format!(
                            "令 {} 摸 {x} 张，然后弃置 1 张",
                            st.seats[target].general_name
                        ),
                    },
                    PendingOption {
                        id: "choose:yinghun:2".into(),
                        label: format!(
                            "令 {} 摸 1 张，然后弃置 {x} 张",
                            st.seats[target].general_name
                        ),
                    },
                ],
                data: json!({ "skill": "yinghun", "target": st.seats[target].player_id }),
            });
            replace_top(st, Frame::Yinghun(f));
            Ok(true)
        }
    }
}

pub(crate) fn yinghun_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Yinghun(f)) => f,
        _ => return Err(internal("栈顶不是英魂帧")),
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
        Frame::Yinghun(YinghunFrame {
            actor: f.actor,
            target: Some(target),
            stage: YinghunStage::Option,
        }),
    );
    Ok("英魂：请选择一项".to_string())
}

pub(crate) fn act_yinghun(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Yinghun(f)) => f,
        _ => return Err(internal("栈顶不是英魂帧")),
    };
    let Some(target) = f.target else {
        return Err(illegal("英魂缺少目标"));
    };
    let x = (st.seats[f.actor].max_hp - st.seats[f.actor].hp).max(0) as u32;
    pop(st);
    let pid = st.seats[f.actor].player_id.clone();
    match action {
        "choose:yinghun:1" => {
            let drawn = draw_capped(st, target, x);
            let req = usize::from(!st.seats[target].hand.is_empty());
            if req > 0 {
                crate::resolve::push_discard(st, target, req, "yinghun");
            }
            st.push_log(
                Some(&pid),
                format!(
                    "英魂：令 {} 摸 {drawn} 张并弃 1 张",
                    st.seats[target].general_name
                ),
            );
            Ok("英魂：目标摸牌并弃 1 张".to_string())
        }
        "choose:yinghun:2" => {
            let drawn = draw_capped(st, target, 1);
            let req = (x as usize).min(st.seats[target].hand.len());
            if req > 0 {
                crate::resolve::push_discard(st, target, req, "yinghun");
            }
            st.push_log(
                Some(&pid),
                format!(
                    "英魂：令 {} 摸 {drawn} 张并弃 {x} 张",
                    st.seats[target].general_name
                ),
            );
            Ok("英魂：目标摸牌并弃牌".to_string())
        }
        _ => Err(illegal("未知英魂选项")),
    }
}

// ---------------------------------------------------------------------------
// 崩坏（董卓）：结束阶段，若非体力最低，失 1 体力或减 1 体力上限
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BengkuaiFrame {
    pub actor: usize,
}

pub(crate) fn step_bengkuai(st: &mut SgsState, f: BengkuaiFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("bengkuai".into()),
        options: vec![
            PendingOption {
                id: "choose:bengkuai:hp".into(),
                label: "崩坏：失去 1 点体力".into(),
            },
            PendingOption {
                id: "choose:bengkuai:max".into(),
                label: "崩坏：减少 1 点体力上限".into(),
            },
        ],
        data: json!({ "skill": "bengkuai" }),
    });
    replace_top(st, Frame::Bengkuai(f));
    Ok(true)
}

pub(crate) fn act_bengkuai(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Bengkuai(f)) => f,
        _ => return Err(internal("栈顶不是崩坏帧")),
    };
    let pid = st.seats[f.actor].player_id.clone();
    pop(st);
    match action {
        "choose:bengkuai:hp" => {
            st.seats[f.actor].hp -= 1;
            let hp = st.seats[f.actor].hp;
            st.push_log(Some(&pid), format!("崩坏：失去 1 点体力（{hp}）"));
            if hp <= 0 {
                crate::resolve::push_dying(st, f.actor, None);
            }
            Ok("崩坏：失去 1 点体力".to_string())
        }
        "choose:bengkuai:max" => {
            st.seats[f.actor].max_hp = (st.seats[f.actor].max_hp - 1).max(1);
            st.seats[f.actor].hp = st.seats[f.actor].hp.min(st.seats[f.actor].max_hp);
            let (hp, max) = (st.seats[f.actor].hp, st.seats[f.actor].max_hp);
            st.push_log(Some(&pid), format!("崩坏：减少 1 点体力上限（{hp}/{max}）"));
            if hp <= 0 {
                crate::resolve::push_dying(st, f.actor, None);
            }
            Ok("崩坏：减少 1 点体力上限".to_string())
        }
        _ => Err(illegal("未知崩坏选项")),
    }
}

// ---------------------------------------------------------------------------
// 烈刃（祝融）：使用【杀】造成伤害后，可与其拼点；赢则获得其一张牌
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiewenStage {
    Start,
    Wait,
    Choose,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LiewenFrame {
    pub actor: usize,
    pub target: usize,
    pub stage: LiewenStage,
    /// 保存父帧（如【杀】的响应结果）的 outcome，避免被拼点覆盖。
    pub saved_outcome: Option<Outcome>,
}

pub(crate) fn step_liewen(st: &mut SgsState, f: LiewenFrame) -> Result<bool, EngineError> {
    match f.stage {
        LiewenStage::Start => {
            if st.seats[f.actor].hand.is_empty() || st.seats[f.target].hand.is_empty() {
                pop(st);
                return Ok(false);
            }
            let saved = st.outcome.take();
            replace_top(
                st,
                Frame::Liewen(LiewenFrame {
                    actor: f.actor,
                    target: f.target,
                    stage: LiewenStage::Wait,
                    saved_outcome: saved,
                }),
            );
            crate::resolve_fire::start_pindian(st, f.actor, f.target, json!({ "skill": "liewen" }));
            Ok(false)
        }
        LiewenStage::Wait => match st.outcome.take() {
            Some(Outcome::Pindian { winner, .. }) => {
                if winner == Some(f.actor) {
                    replace_top(
                        st,
                        Frame::Liewen(LiewenFrame {
                            actor: f.actor,
                            target: f.target,
                            stage: LiewenStage::Choose,
                            saved_outcome: f.saved_outcome,
                        }),
                    );
                    Ok(false)
                } else {
                    pop(st);
                    st.outcome = f.saved_outcome;
                    Ok(false)
                }
            }
            _ => Err(internal("烈刃缺少拼点结果")),
        },
        LiewenStage::Choose => {
            let mut options = Vec::new();
            for (slot, card) in &st.seats[f.target].equips {
                if let Some(id) = card {
                    options.push(PendingOption {
                        id: format!("pick:equip:{slot:?}"),
                        label: format!("烈刃：获得装备【{}】", turn::card_label(st, id)),
                    });
                }
            }
            if !st.seats[f.target].hand.is_empty() {
                options.push(PendingOption {
                    id: "pick:hand".into(),
                    label: "烈刃：获得一张手牌（随机）".into(),
                });
            }
            for id in &st.seats[f.target].judge_zone {
                options.push(PendingOption {
                    id: format!("pick:judge:{id}"),
                    label: format!("烈刃：获得判定区【{}】", turn::card_label(st, id)),
                });
            }
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("liewen".into()),
                options,
                data: json!({ "skill": "liewen", "target": st.seats[f.target].player_id }),
            });
            replace_top(st, Frame::Liewen(f));
            Ok(true)
        }
    }
}

pub(crate) fn act_liewen(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Liewen(f)) => f,
        _ => return Err(internal("栈顶不是烈刃帧")),
    };
    let card = if let Some(slot) = action.strip_prefix("pick:equip:") {
        let slot = match slot {
            "Weapon" => EquipSlot::Weapon,
            "Armor" => EquipSlot::Armor,
            "HorseOffense" => EquipSlot::HorseOffense,
            "HorseDefense" => EquipSlot::HorseDefense,
            "Treasure" => EquipSlot::Treasure,
            _ => return Err(illegal("未知装备槽")),
        };
        let Some(Some(id)) = st.seats[f.target].equips.insert(slot, None) else {
            return Err(illegal("该装备槽没有牌"));
        };
        id
    } else if action == "pick:hand" {
        if st.seats[f.target].hand.is_empty() {
            return Err(illegal("目标没有手牌"));
        }
        let n = st.seats[f.target].hand.len();
        let idx = st.next_index(n);
        st.seats[f.target].hand.remove(idx)
    } else if let Some(id) = action.strip_prefix("pick:judge:") {
        let pos = st.seats[f.target]
            .judge_zone
            .iter()
            .position(|c| c == id)
            .ok_or_else(|| illegal("判定区没有该牌"))?;
        st.seats[f.target].judge_zone.remove(pos)
    } else {
        return Err(illegal("未知烈刃选项"));
    };
    let label = turn::card_label(st, &card);
    st.seats[f.actor].hand.push(card);
    pop(st);
    st.outcome = f.saved_outcome;
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        format!("烈刃：拼点赢，获得【{label}】"),
    );
    Ok(format!("烈刃：获得【{label}】"))
}

// ---------------------------------------------------------------------------
// 再起（孟获）：摸牌阶段，若已受伤，改为亮 X 张（X=已损失体力），红桃回血
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZaizhiFrame {
    pub actor: usize,
}

pub(crate) fn step_zaizhi(st: &mut SgsState, f: ZaizhiFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("zaizhi".into()),
        options: vec![
            PendingOption {
                id: "choose:zaizhi".into(),
                label: "再起：亮出 X 张牌（X=已损失体力），红桃回复，获得全部亮出牌".into(),
            },
            pass_option("正常摸牌"),
        ],
        data: json!({ "skill": "zaizhi" }),
    });
    replace_top(st, Frame::Zaizhi(f));
    Ok(true)
}

pub(crate) fn act_zaizhi(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Zaizhi(f)) => f,
        _ => return Err(internal("栈顶不是再起帧")),
    };
    if action != "choose:zaizhi" {
        pop(st);
        return Ok("正常摸牌".to_string());
    }
    pop(st);
    st.set_counter("skip_draw", 1);
    let x = (st.seats[f.actor].max_hp - st.seats[f.actor].hp).max(0) as u32;
    let mut events = Vec::new();
    let mut cards = Vec::new();
    for _ in 0..x {
        if let Some(id) = turn::draw_one(st, &mut events) {
            cards.push(id);
        }
    }
    let mut hearts = 0;
    for id in &cards {
        if st.cards.get(id).is_some_and(|c| c.suit == Suit::Heart) {
            hearts += 1;
        }
    }
    let healed = if hearts > 0 {
        let before = st.seats[f.actor].hp;
        st.seats[f.actor].hp = (st.seats[f.actor].hp + hearts).min(st.seats[f.actor].max_hp);
        st.seats[f.actor].hp - before
    } else {
        0
    };
    let n = cards.len();
    for id in cards {
        st.seats[f.actor].hand.push(id);
    }
    let over = st.seats[f.actor]
        .hand
        .len()
        .saturating_sub(st.config.max_hand_cards as usize);
    if over > 0 {
        crate::resolve::push_discard(st, f.actor, over, "hand_cap");
    }
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        format!("再起：亮 {n} 张，红桃 {hearts} 张，回复 {healed} 点体力"),
    );
    Ok(format!("再起：亮 {n} 张，回复 {healed} 点"))
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HaoshiOfferFrame {
    pub actor: usize,
}

pub(crate) fn step_haoshi_offer(
    st: &mut SgsState,
    f: HaoshiOfferFrame,
) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("haoshi_offer".into()),
        options: vec![
            PendingOption {
                id: "choose:haoshi".into(),
                label: "好施：本阶段多摸 2 张".into(),
            },
            pass_option("正常摸牌"),
        ],
        data: json!({ "skill": "haoshi" }),
    });
    replace_top(st, Frame::HaoshiOffer(f));
    Ok(true)
}

pub(crate) fn act_haoshi_offer(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::HaoshiOffer(f)) => f,
        _ => return Err(internal("栈顶不是好施帧")),
    };
    if action == "choose:haoshi" {
        st.set_counter("haoshi", 1);
        st.push_log(
            Some(&st.seats[f.actor].player_id.clone()),
            "好施：本阶段多摸 2 张",
        );
    }
    pop(st);
    Ok("好施：继续摸牌".to_string())
}

// ---------------------------------------------------------------------------
// 好施（鲁肃）：摸牌阶段多摸 2；手牌>5 时交一半给手牌最少者
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HaoshiStage {
    Select,
    Target,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HaoshiFrame {
    pub actor: usize,
    pub stage: HaoshiStage,
    pub selected: Vec<String>,
}

pub(crate) fn step_haoshi(st: &mut SgsState, f: HaoshiFrame) -> Result<bool, EngineError> {
    match f.stage {
        HaoshiStage::Select => {
            let n = st.seats[f.actor].hand.len() / 2;
            replace_top(
                st,
                Frame::MultiSelect(crate::resolve::MultiSelectFrame {
                    actor: f.actor,
                    purpose: "haoshi".into(),
                    selected: Vec::new(),
                    min: n,
                    max: n,
                    data: json!({}),
                }),
            );
            Ok(false)
        }
        HaoshiStage::Target => {
            let options = haoshi_targets(st, f.actor);
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("haoshi".into()),
                options,
                data: json!({ "stage": "haoshi" }),
            });
            replace_top(st, Frame::Haoshi(f));
            Ok(true)
        }
    }
}

fn haoshi_targets(st: &SgsState, actor: usize) -> Vec<PendingOption> {
    let min_hand = st
        .alive_indices()
        .into_iter()
        .filter(|i| *i != actor)
        .map(|i| st.seats[i].hand.len())
        .min()
        .unwrap_or(0);
    st.alive_indices()
        .into_iter()
        .filter(|i| *i != actor && st.seats[*i].hand.len() == min_hand)
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("好施：交给 {}", st.seats[i].general_name),
        })
        .collect()
}

pub(crate) fn haoshi_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Haoshi(f)) => f,
        _ => return Err(internal("栈顶不是好施帧")),
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
    let n = f.selected.len();
    for card in &f.selected {
        if turn::take_card(st, f.actor, card).is_err() {
            return Err(internal("好施：手牌缺少所选牌"));
        }
        st.seats[target].hand.push(card.clone());
    }
    let over = st.seats[target]
        .hand
        .len()
        .saturating_sub(st.config.max_hand_cards as usize);
    if over > 0 {
        crate::resolve::push_discard(st, target, over, "hand_cap");
    }
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        format!("好施：交给 {} {n} 张牌", st.seats[target].general_name),
    );
    // 界好施（界鲁肃）：记录回礼者（此后成为【杀】/普通锦囊目标时可收其一张牌）。
    crate::resolve_jiexian4::mark_haoshi_back(st, f.actor, target);
    pop(st);
    Ok(format!(
        "好施：交给 {} {n} 张",
        st.seats[target].general_name
    ))
}

// ---------------------------------------------------------------------------
// 缔盟（鲁肃）：选两名其他角色，弃 X=手牌差，交换手牌
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DimengStage {
    First,
    Second,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DimengFrame {
    pub actor: usize,
    pub first: Option<usize>,
    pub stage: DimengStage,
}

pub(crate) fn step_dimeng(st: &mut SgsState, f: DimengFrame) -> Result<bool, EngineError> {
    let options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| *i != f.actor && Some(*i) != f.first)
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!(
                "缔盟：选择 {}（手牌 {}）",
                st.seats[i].general_name,
                st.seats[i].hand.len()
            ),
        })
        .collect();
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "target".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("dimeng".into()),
        options,
        data: json!({ "stage": "dimeng" }),
    });
    replace_top(st, Frame::Dimeng(f));
    Ok(true)
}

pub(crate) fn dimeng_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let mut f = match top(st).cloned() {
        Some(Frame::Dimeng(f)) => f,
        _ => return Err(internal("栈顶不是缔盟帧")),
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
    match f.stage {
        DimengStage::First => {
            f.first = Some(target);
            f.stage = DimengStage::Second;
            replace_top(st, Frame::Dimeng(f));
            Ok("缔盟：请选择第二名角色".to_string())
        }
        DimengStage::Second => {
            let Some(first) = f.first else {
                return Err(internal("缔盟缺少第一名角色"));
            };
            let x = st.seats[first]
                .hand
                .len()
                .abs_diff(st.seats[target].hand.len());
            if x > st.seats[f.actor].hand.len() {
                return Err(illegal("手牌不足，无法支付缔盟费用"));
            }
            if x == 0 {
                let a = std::mem::take(&mut st.seats[first].hand);
                let b = std::mem::take(&mut st.seats[target].hand);
                st.seats[first].hand = b;
                st.seats[target].hand = a;
                pop(st);
                st.push_log(
                    Some(&st.seats[f.actor].player_id.clone()),
                    format!(
                        "缔盟：交换 {} 与 {} 的手牌",
                        st.seats[first].general_name, st.seats[target].general_name
                    ),
                );
                return Ok("缔盟：已交换手牌".to_string());
            }
            replace_top(
                st,
                Frame::MultiSelect(crate::resolve::MultiSelectFrame {
                    actor: f.actor,
                    purpose: "dimeng".into(),
                    selected: Vec::new(),
                    min: x,
                    max: x,
                    data: json!({ "first": first, "second": target }),
                }),
            );
            Ok(format!("缔盟：请弃置 {x} 张手牌"))
        }
    }
}

// ---------------------------------------------------------------------------
// 乱武（贾诩，限定）：所有其他角色依次选择杀最近或失去 1 体力
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LuanwuFrame {
    pub actor: usize,
    pub order: Vec<usize>,
    pub cursor: usize,
}

fn nearest_target(st: &SgsState, actor: usize) -> Option<usize> {
    let mut best: Option<(usize, usize)> = None;
    for i in st.alive_indices() {
        if i == actor {
            continue;
        }
        let d = st.seat_distance(actor, i);
        if d == usize::MAX {
            continue;
        }
        if best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, i));
        }
    }
    best.map(|(_, i)| i)
}

pub(crate) fn step_luanwu(st: &mut SgsState, f: LuanwuFrame) -> Result<bool, EngineError> {
    let mut cursor = f.cursor;
    while cursor < f.order.len() && !st.seats[f.order[cursor]].alive {
        cursor += 1;
    }
    if cursor >= f.order.len() {
        pop(st);
        // 界乱武（界贾诩）：所有角色结算完毕后，视为使用一张无距离限制的【杀】。
        crate::resolve_jiexian4::push_luanwu_final_sha(st, f.actor);
        return Ok(false);
    }
    let seat = f.order[cursor];
    let sha = turn::hand_cards_with_def(st, seat, "sha")
        .into_iter()
        .next();
    let nearest = nearest_target(st, seat);
    let mut options = Vec::new();
    if sha.is_some() && nearest.is_some() {
        options.push(PendingOption {
            id: "choose:luanwu:sha".into(),
            label: format!(
                "乱武：对 {} 使用【杀】",
                st.seats[nearest.expect("nearest")].general_name
            ),
        });
    }
    options.push(PendingOption {
        id: "choose:luanwu:lose".into(),
        label: "乱武：失去 1 点体力".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[seat].player_id.clone(),
        skill: Some("luanwu".into()),
        options,
        data: json!({ "skill": "luanwu", "seat": st.seats[seat].player_id }),
    });
    replace_top(
        st,
        Frame::Luanwu(LuanwuFrame {
            actor: f.actor,
            order: f.order,
            cursor,
        }),
    );
    Ok(true)
}

pub(crate) fn act_luanwu(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Luanwu(f)) => f,
        _ => return Err(internal("栈顶不是乱武帧")),
    };
    let seat = f.order[f.cursor];
    let pid = st.seats[seat].player_id.clone();
    let mut nf = f.clone();
    nf.cursor += 1;
    match action {
        "choose:luanwu:lose" => {
            st.seats[seat].hp -= 1;
            let hp = st.seats[seat].hp;
            st.push_log(Some(&pid), format!("乱武：失去 1 点体力（{hp}）"));
            replace_top(st, Frame::Luanwu(nf));
            if hp <= 0 {
                crate::resolve::push_dying(st, seat, None);
            }
            Ok("乱武：失去 1 点体力".to_string())
        }
        "choose:luanwu:sha" => {
            let Some(card) = turn::hand_cards_with_def(st, seat, "sha")
                .into_iter()
                .next()
            else {
                return Err(illegal("没有【杀】"));
            };
            let Some(target) = nearest_target(st, seat) else {
                return Err(illegal("没有最近目标"));
            };
            turn::take_card(st, seat, &card)?;
            st.deck.processing.push(card.clone());
            st.push_log(
                Some(&pid),
                format!("乱武：对 {} 使用【杀】", st.seats[target].general_name),
            );
            replace_top(st, Frame::Luanwu(nf));
            push(
                st,
                Frame::Sha(crate::resolve::ShaFrame {
                    card: card.clone(),
                    user: seat,
                    target: Some(target),
                    stage: crate::resolve::ShaStage::WaitRespond,
                    pending_targets: Vec::new(),
                    fangtian: false,
                    damage_type: DamageType::Normal,
                    no_distance: false,
                    no_respond: false,
                    virtual_card: false,
                    tianyi: false,
                }),
            );
            crate::resolve_skill::push_sha_target_response(
                st,
                seat,
                target,
                &card,
                1,
                false,
                false,
                DamageType::Normal,
            );
            Ok("乱武：使用【杀】".to_string())
        }
        _ => Err(illegal("未知乱武选项")),
    }
}
