//! P4.4.1 山包技能帧：巧变 / 挑衅 / 享乐 / 放权 / 制霸 / 直谏 / 固政 / 化身 / 悲歌 / 觉醒。

use crate::resolve::{pop, push, replace_top, top, DamageType, Frame, Outcome};
use crate::skills;
use crate::skills::is_jiexian;
use crate::state::*;
use crate::turn;
use crate::{illegal, internal};
use game_engine_core::EngineError;
use serde_json::json;

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

fn pass_option(label: &str) -> PendingOption {
    PendingOption {
        id: "pass".into(),
        label: label.into(),
    }
}

// ---------------------------------------------------------------------------
// 志继（姜维，觉醒）：无手牌时回复 1 或摸 2，减 1 上限并获得观星
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AwakenFrame {
    pub actor: usize,
}

pub(crate) fn step_awaken(st: &mut SgsState, f: AwakenFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("awaken".into()),
        options: vec![
            PendingOption {
                id: "choose:awaken:heal".into(),
                label: "志继：回复 1 点体力".into(),
            },
            PendingOption {
                id: "choose:awaken:draw".into(),
                label: "志继：摸 2 张牌".into(),
            },
        ],
        data: json!({ "skill": "awaken" }),
    });
    replace_top(st, Frame::Awaken(f));
    Ok(true)
}

pub(crate) fn act_awaken(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Awaken(f)) => f,
        _ => return Err(internal("栈顶不是觉醒帧")),
    };
    let pid = st.seats[f.actor].player_id.clone();
    pop(st);
    st.seats[f.actor].flags.insert("awaken_zhiji".into(), true);
    st.seats[f.actor].max_hp = (st.seats[f.actor].max_hp - 1).max(1);
    st.seats[f.actor].hp = st.seats[f.actor].hp.min(st.seats[f.actor].max_hp);
    if action == "choose:awaken:heal" {
        st.seats[f.actor].hp = (st.seats[f.actor].hp + 1).min(st.seats[f.actor].max_hp);
    } else if action == "choose:awaken:draw" {
        let mut ev = Vec::new();
        turn::draw_cards(st, f.actor, 2, &mut ev);
    } else {
        return Err(illegal("未知觉醒选项"));
    }
    skills::grant_skill(st, f.actor, "guanxing");
    st.push_log(Some(&pid), "志继：觉醒，减少 1 点体力上限并获得「观星」");
    Ok("志继：觉醒完成".to_string())
}

// ---------------------------------------------------------------------------
// 巧变（张郃）：弃 1 手牌并跳过一个阶段
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QiaobianFrame {
    pub actor: usize,
    pub remaining: u32,
}

pub(crate) fn step_qiaobian(st: &mut SgsState, f: QiaobianFrame) -> Result<bool, EngineError> {
    if f.remaining > 0 {
        // 获得至多两名角色各一张手牌（简化：一名一名选，可随时停止）。
        let mut options: Vec<PendingOption> = st
            .alive_indices()
            .into_iter()
            .filter(|i| *i != f.actor && !st.seats[*i].hand.is_empty())
            .map(|i| PendingOption {
                id: format!("target:{}", st.seats[i].player_id),
                label: format!("巧变：获得 {} 一张手牌", st.seats[i].general_name),
            })
            .collect();
        if options.is_empty() {
            pop(st);
            return Ok(false);
        }
        options.push(pass_option("停止获得"));
        st.pending = Some(Pending {
            kind: "target".into(),
            actor: st.seats[f.actor].player_id.clone(),
            skill: Some("qiaobian".into()),
            options,
            data: json!({ "stage": "qiaobian_gain" }),
        });
        replace_top(st, Frame::Qiaobian(f));
        return Ok(true);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("qiaobian".into()),
        options: vec![
            PendingOption {
                id: "choose:qiaobian:judge".into(),
                label: "巧变：跳过判定阶段".into(),
            },
            PendingOption {
                id: "choose:qiaobian:draw".into(),
                label: "巧变：跳过摸牌阶段并获得至多两名角色各一张手牌".into(),
            },
            PendingOption {
                id: "choose:qiaobian:play".into(),
                label: "巧变：跳过出牌阶段并移动场上一张牌".into(),
            },
            PendingOption {
                id: "choose:qiaobian:discard".into(),
                label: "巧变：跳过弃牌阶段".into(),
            },
            pass_option("不发动巧变"),
        ],
        data: json!({ "skill": "qiaobian" }),
    });
    replace_top(st, Frame::Qiaobian(f));
    Ok(true)
}

pub(crate) fn act_qiaobian(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Qiaobian(f)) => f,
        _ => return Err(internal("栈顶不是巧变帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动巧变".to_string());
    }
    let choice = action
        .strip_prefix("choose:qiaobian:")
        .ok_or_else(|| illegal("未知巧变选项"))?;
    if !crate::resolve_jiexian4::qiaobian_can_pay(st, f.actor) {
        return Err(illegal("巧变需要弃置一张手牌或移除 1 枚「变」"));
    }
    // 界巧变（界张郃）：无手牌时可用 1 枚「变」支付，直接进入所选阶段处理。
    if st.seats[f.actor].hand.is_empty() {
        if !crate::resolve_jiexian4::qiaobian_pay_bian(st, f.actor) {
            return Err(illegal("没有可支付的「变」"));
        }
        return apply_qiaobian_choice(st, f.actor, choice);
    }
    replace_top(
        st,
        Frame::MultiSelect(crate::resolve::MultiSelectFrame {
            actor: f.actor,
            purpose: "qiaobian".into(),
            selected: Vec::new(),
            min: 1,
            max: 1,
            data: json!({ "choice": choice }),
        }),
    );
    Ok("巧变：请选择弃置的手牌".to_string())
}

pub(crate) fn qiaobian_gain_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Qiaobian(f)) => f,
        _ => return Err(internal("栈顶不是巧变帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("停止获得手牌".to_string());
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
    if st.seats[target].hand.is_empty() {
        return Err(illegal("目标没有手牌"));
    }
    let n = st.seats[target].hand.len();
    let idx = st.next_index(n);
    let card = st.seats[target].hand.remove(idx);
    let label = turn::card_label(st, &card);
    st.seats[f.actor].hand.push(card);
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        format!(
            "巧变：获得 {} 一张手牌【{label}】",
            st.seats[target].general_name
        ),
    );
    let mut nf = f;
    nf.remaining = nf.remaining.saturating_sub(1);
    if nf.remaining == 0 {
        pop(st);
    } else {
        replace_top(st, Frame::Qiaobian(nf));
    }
    Ok("巧变：获得一张手牌".to_string())
}

// ---------------------------------------------------------------------------
// 挑衅（姜维）：令攻击范围含你的角色对你出杀，否则弃其一张牌
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TiaoxinStage {
    Target,
    Ask,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TiaoxinFrame {
    pub actor: usize,
    pub target: Option<usize>,
    pub stage: TiaoxinStage,
}

pub(crate) fn step_tiaoxin(st: &mut SgsState, f: TiaoxinFrame) -> Result<bool, EngineError> {
    match f.stage {
        TiaoxinStage::Target => {
            let options: Vec<PendingOption> = st
                .alive_indices()
                .into_iter()
                .filter(|t| {
                    *t != f.actor
                        && st.in_attack_range(*t, f.actor)
                        && (!st.seats[*t].hand.is_empty()
                            || st.seats[*t].equips.values().any(|c| c.is_some())
                            || !st.seats[*t].judge_zone.is_empty())
                })
                .map(|t| PendingOption {
                    id: format!("target:{}", st.seats[t].player_id),
                    label: format!("挑衅：{}", st.seats[t].general_name),
                })
                .collect();
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("tiaoxin".into()),
                options,
                data: json!({ "stage": "tiaoxin" }),
            });
            replace_top(st, Frame::Tiaoxin(f));
            Ok(true)
        }
        TiaoxinStage::Ask => {
            let Some(target) = f.target else {
                pop(st);
                return Ok(false);
            };
            let has_sha = !turn::hand_cards_with_def(st, target, "sha").is_empty();
            let mut options = Vec::new();
            if has_sha {
                options.push(PendingOption {
                    id: "choose:tiaoxin:sha".into(),
                    label: "挑衅：对姜维使用一张【杀】".into(),
                });
            }
            options.push(pass_option("不出【杀】（姜维弃你一张牌）"));
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: st.seats[target].player_id.clone(),
                skill: Some("tiaoxin".into()),
                options,
                data: json!({ "skill": "tiaoxin" }),
            });
            replace_top(st, Frame::Tiaoxin(f));
            Ok(true)
        }
    }
}

pub(crate) fn tiaoxin_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Tiaoxin(f)) => f,
        _ => return Err(internal("栈顶不是挑衅帧")),
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
        Frame::Tiaoxin(TiaoxinFrame {
            actor: f.actor,
            target: Some(target),
            stage: TiaoxinStage::Ask,
        }),
    );
    Ok("挑衅：等待目标响应".to_string())
}

pub(crate) fn act_tiaoxin(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Tiaoxin(f)) => f,
        _ => return Err(internal("栈顶不是挑衅帧")),
    };
    let Some(target) = f.target else {
        return Err(illegal("挑衅缺少目标"));
    };
    pop(st);
    if action == "choose:tiaoxin:sha" {
        let Some(card) = turn::hand_cards_with_def(st, target, "sha")
            .into_iter()
            .next()
        else {
            return Err(illegal("没有【杀】"));
        };
        turn::take_card(st, target, &card)?;
        st.deck.processing.push(card.clone());
        // 界挑衅（界姜维）：此【杀】未造成伤害则仍弃其一张牌。
        if crate::skills::is_jiexian(st, f.actor) {
            st.set_counter("tiaoxin_hit", 0);
            crate::resolve_jiexian4::push_tiaoxin_wait(st, f.actor, target);
        }
        push(
            st,
            Frame::Sha(crate::resolve::ShaFrame {
                card: card.clone(),
                user: target,
                target: Some(f.actor),
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
            target,
            f.actor,
            &card,
            1,
            false,
            false,
            DamageType::Normal,
        );
        Ok("挑衅：目标使用【杀】".to_string())
    } else {
        push(
            st,
            Frame::PickCard(crate::resolve::PickCardFrame {
                card: "tiaoxin".into(),
                user: f.actor,
                target,
                mode: crate::resolve::PickMode::Tiaoxin,
            }),
        );
        Ok("挑衅：请弃置目标一张牌".to_string())
    }
}

// ---------------------------------------------------------------------------
// 享乐（刘禅）：成为【杀】目标后，使用者需弃基本牌，否则杀无效
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct XiangleFrame {
    pub user: usize,
    pub target: usize,
    pub card: String,
    pub need: u32,
    pub no_shan: bool,
    pub ignore_armor: bool,
    pub damage_type: DamageType,
}

pub(crate) fn step_xiangle(st: &mut SgsState, f: XiangleFrame) -> Result<bool, EngineError> {
    let has_basic = st.seats[f.user]
        .hand
        .iter()
        .any(|id| turn::card_def(st, id).is_some_and(|d| d.category == Category::Basic));
    let mut options = Vec::new();
    if has_basic {
        options.push(PendingOption {
            id: "choose:xiangle".into(),
            label: "享乐：弃置一张基本牌（使【杀】生效）".into(),
        });
    }
    options.push(pass_option("不弃牌（令此【杀】无效）"));
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.user].player_id.clone(),
        skill: Some("xiangle".into()),
        options,
        data: json!({ "skill": "xiangle", "target": st.seats[f.target].player_id }),
    });
    replace_top(st, Frame::Xiangle(f));
    Ok(true)
}

pub(crate) fn act_xiangle(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Xiangle(f)) => f,
        _ => return Err(internal("栈顶不是享乐帧")),
    };
    if action != "choose:xiangle" {
        // 杀无效：丢弃处理区的【杀】并结束本次杀。
        pop(st);
        crate::resolve::discard_from_processing(st, &f.card);
        pop(st);
        let pid = st.seats[f.target].player_id.clone();
        st.push_log(Some(&pid), "享乐：【杀】无效");
        return Ok("享乐：【杀】无效".to_string());
    }
    let options: Vec<PendingOption> = st.seats[f.user]
        .hand
        .iter()
        .filter(|id| turn::card_def(st, id).is_some_and(|d| d.category == Category::Basic))
        .map(|id| PendingOption {
            id: format!("choose:{id}"),
            label: format!("弃置基本牌【{}】", turn::card_label(st, id)),
        })
        .collect();
    if options.is_empty() {
        pop(st);
        crate::resolve::discard_from_processing(st, &f.card);
        pop(st);
        return Ok("享乐：没有基本牌，【杀】无效".to_string());
    }
    replace_top(
        st,
        Frame::MultiSelect(crate::resolve::MultiSelectFrame {
            actor: f.user,
            purpose: "xiangle".into(),
            selected: Vec::new(),
            min: 1,
            max: 1,
            data: json!({
                "user": f.user,
                "target": f.target,
                "card": f.card,
                "need": f.need,
                "no_shan": f.no_shan,
                "ignore_armor": f.ignore_armor,
                "damage_type": f.damage_type,
            }),
        }),
    );
    Ok("享乐：请选择弃置的基本牌".to_string())
}

// ---------------------------------------------------------------------------
// 放权（刘禅）：跳过出牌阶段；结束时可弃 1 手牌令其他角色执行额外回合
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FangquanStage {
    Target,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FangquanFrame {
    pub actor: usize,
    pub stage: FangquanStage,
}

pub(crate) fn step_fangquan(st: &mut SgsState, f: FangquanFrame) -> Result<bool, EngineError> {
    let options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| *i != f.actor)
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("放权：令 {} 执行额外回合", st.seats[i].general_name),
        })
        .collect();
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "target".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("fangquan".into()),
        options,
        data: json!({ "stage": "fangquan" }),
    });
    replace_top(st, Frame::Fangquan(f));
    Ok(true)
}

pub(crate) fn fangquan_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Fangquan(f)) => f,
        _ => return Err(internal("栈顶不是放权帧")),
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
    if st.seats[f.actor].hand.is_empty() {
        st.extra_turn = Some(target);
        return Ok("放权：没有手牌可弃，仍授予额外回合".to_string());
    }
    // 简化：弃置一张手牌（自动取第一张）并授予额外回合。
    let card = st.seats[f.actor].hand[0].clone();
    turn::take_card(st, f.actor, &card)?;
    turn::to_discard_card(st, &card);
    st.extra_turn = Some(target);
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        format!(
            "放权：弃置【{}】，令 {} 执行额外回合",
            turn::card_label(st, &card),
            st.seats[target].general_name
        ),
    );
    Ok("放权：已授予额外回合".to_string())
}

// ---------------------------------------------------------------------------
// 制霸（孙策主公技）：其他吴势力可与其拼点；没赢则孙策获得两张拼点牌
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZhibaStage {
    LordChoice,
    WaitPindian,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZhibaFrame {
    pub challenger: usize,
    pub lord: usize,
    pub stage: ZhibaStage,
}

pub(crate) fn step_zhiba(st: &mut SgsState, f: ZhibaFrame) -> Result<bool, EngineError> {
    match f.stage {
        ZhibaStage::LordChoice => {
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: st.seats[f.lord].player_id.clone(),
                skill: Some("zhiba".into()),
                options: vec![
                    PendingOption {
                        id: "choose:zhiba".into(),
                        label: "制霸：接受拼点".into(),
                    },
                    pass_option("拒绝拼点（已发动魂姿）"),
                ],
                data: json!({ "skill": "zhiba" }),
            });
            replace_top(st, Frame::Zhiba(f));
            Ok(true)
        }
        ZhibaStage::WaitPindian => match st.outcome.take() {
            Some(Outcome::Pindian {
                winner,
                initiator,
                initiator_card,
                target_card,
                ..
            }) => {
                if winner != Some(f.challenger) {
                    let mut moved = 0;
                    for id in [&initiator_card, &target_card] {
                        if st.deck.discard.iter().any(|c| c == id) {
                            st.deck.discard.retain(|c| c != id);
                            st.seats[f.lord].hand.push(id.clone());
                            moved += 1;
                        }
                    }
                    st.push_log(
                        Some(&st.seats[f.lord].player_id.clone()),
                        format!("制霸：挑战者没赢，获得 {moved} 张拼点牌"),
                    );
                }
                let _ = initiator;
                pop(st);
                Ok(false)
            }
            _ => Err(internal("制霸缺少拼点结果")),
        },
    }
}

pub(crate) fn act_zhiba(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Zhiba(f)) => f,
        _ => return Err(internal("栈顶不是制霸帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("拒绝拼点".to_string());
    }
    replace_top(
        st,
        Frame::Zhiba(ZhibaFrame {
            challenger: f.challenger,
            lord: f.lord,
            stage: ZhibaStage::WaitPindian,
        }),
    );
    crate::resolve_fire::start_pindian(st, f.challenger, f.lord, json!({ "skill": "zhiba" }));
    Ok("制霸：拼点开始".to_string())
}

// ---------------------------------------------------------------------------
// 直谏（张昭张纮）：将手牌装备置入其他角色装备区，然后摸 1
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZhijianFrame {
    pub actor: usize,
    pub card: String,
}

pub(crate) fn step_zhijian(st: &mut SgsState, f: ZhijianFrame) -> Result<bool, EngineError> {
    let options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| *i != f.actor)
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("直谏：置入 {} 的装备区", st.seats[i].general_name),
        })
        .collect();
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "target".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("zhijian".into()),
        options,
        data: json!({ "stage": "zhijian" }),
    });
    replace_top(st, Frame::Zhijian(f));
    Ok(true)
}

pub(crate) fn zhijian_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Zhijian(f)) => f,
        _ => return Err(internal("栈顶不是直谏帧")),
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
    let def = turn::card_def(st, &f.card).ok_or_else(|| illegal("未知装备"))?;
    let slot = def.equip_slot.ok_or_else(|| illegal("不是装备牌"))?;
    if !st.seats[f.actor].hand.iter().any(|c| c == &f.card) {
        return Err(illegal("手牌中没有该牌"));
    }
    turn::take_card(st, f.actor, &f.card)?;
    let old = st.seats[target].equips.insert(slot, Some(f.card.clone()));
    if let Some(Some(old_id)) = old {
        turn::to_discard_card(st, &old_id);
        skills::xiaoji_on_lose_equip(st, target);
    }
    let drawn = draw_capped(st, f.actor, 1);
    pop(st);
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        format!(
            "直谏：将【{}】置入 {} 装备区并摸 {drawn} 张",
            turn::card_label(st, &f.card),
            st.seats[target].general_name
        ),
    );
    Ok("直谏：已完成".to_string())
}

// ---------------------------------------------------------------------------
// 固政（张昭张纮）：其他角色弃牌阶段结束时，令其获得一张弃牌，然后获得剩余
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GuzhengFrame {
    pub owner: usize,
    pub discarder: usize,
    pub cards: Vec<String>,
}

pub(crate) fn step_guzheng(st: &mut SgsState, f: GuzhengFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.owner].player_id.clone(),
        skill: Some("guzheng".into()),
        options: vec![
            PendingOption {
                id: "choose:guzheng".into(),
                label: format!(
                    "固政：令 {} 获得一张弃牌，你获得剩余",
                    st.seats[f.discarder].general_name
                ),
            },
            pass_option("不发动固政"),
        ],
        data: json!({ "skill": "guzheng" }),
    });
    replace_top(st, Frame::Guzheng(f));
    Ok(true)
}

pub(crate) fn act_guzheng(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Guzheng(f)) => f,
        _ => return Err(internal("栈顶不是固政帧")),
    };
    pop(st);
    if action != "choose:guzheng" {
        st.turn_phase = TurnPhase::End;
        return Ok("不发动固政".to_string());
    }
    let mut cards = f.cards;
    let mut given = 0;
    let mut gained = 0;
    if !cards.is_empty() {
        let first = cards.remove(0);
        if st.deck.discard.iter().any(|c| c == &first) {
            st.deck.discard.retain(|c| c != &first);
            st.seats[f.discarder].hand.push(first);
            given += 1;
        }
    }
    for id in cards {
        if st.deck.discard.iter().any(|c| c == &id) {
            st.deck.discard.retain(|c| c != &id);
            st.seats[f.owner].hand.push(id);
            gained += 1;
        }
    }
    let over = st.seats[f.owner]
        .hand
        .len()
        .saturating_sub(st.config.max_hand_cards as usize);
    if over > 0 {
        crate::resolve::push_discard(st, f.owner, over, "hand_cap");
    }
    st.push_log(
        Some(&st.seats[f.owner].player_id.clone()),
        format!("固政：{given} 张交给弃牌者，{gained} 张归自己"),
    );
    // 固政在弃牌阶段结束后结算，不再回到弃牌阶段重复弃牌。
    st.turn_phase = TurnPhase::End;
    Ok("固政：已完成".to_string())
}

// ---------------------------------------------------------------------------
// 化身（左慈）：展示的化身牌与技能替换
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HuashenFrame {
    pub actor: usize,
}

pub(crate) fn step_huashen(st: &mut SgsState, f: HuashenFrame) -> Result<bool, EngineError> {
    let active = st.seats[f.actor].huashen_active.clone();
    let other = st.seats[f.actor]
        .huashen
        .iter()
        .find(|g| Some(*g) != active.as_ref())
        .cloned();
    let jie = is_jiexian(st, f.actor);
    if other.is_none() && !jie {
        pop(st);
        return Ok(false);
    }
    let mut options: Vec<PendingOption> = Vec::new();
    if let Some(other) = &other {
        options.push(PendingOption {
            id: "choose:huashen".into(),
            label: format!(
                "化身：替换为 {}",
                crate::data::general(other)
                    .map(|g| g.name)
                    .unwrap_or(other)
            ),
        });
    }
    // 界化身：移去至多两张化身牌并获得等量新的化身牌。
    if jie {
        options.push(PendingOption {
            id: "choose:exchange".into(),
            label: "化身：移去至多两张化身牌，获得等量新的化身牌".into(),
        });
    }
    options.push(pass_option("保持当前化身"));
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("huashen".into()),
        options,
        data: json!({ "skill": "huashen" }),
    });
    replace_top(st, Frame::Huashen(f));
    Ok(true)
}

pub(crate) fn act_huashen(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Huashen(f)) => f,
        _ => return Err(internal("栈顶不是化身帧")),
    };
    if action != "choose:huashen" && action != "choose:exchange" {
        pop(st);
        return Ok("保持当前化身".to_string());
    }
    if action == "choose:exchange" {
        pop(st);
        return Ok(huashen_exchange(st, f.actor));
    }
    let active = st.seats[f.actor].huashen_active.clone();
    let other = st.seats[f.actor]
        .huashen
        .iter()
        .find(|g| Some(*g) != active.as_ref())
        .cloned()
        .ok_or_else(|| illegal("没有其他化身牌"))?;
    if let Some(old) = active {
        if let Some(skill) = skills::first_implemented_skill(&old) {
            skills::revoke_skill(st, f.actor, skill);
        }
    }
    st.seats[f.actor].huashen_active = Some(other.clone());
    if let Some(skill) = skills::first_implemented_skill(&other) {
        skills::grant_skill(st, f.actor, skill);
    }
    pop(st);
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        format!(
            "化身：替换为 {}",
            crate::data::general(&other)
                .map(|g| g.name)
                .unwrap_or(&other)
        ),
    );
    Ok("化身：已替换".to_string())
}

// ---------------------------------------------------------------------------
// 悲歌（蔡文姬）：角色受到【杀】伤害后，弃一张牌令其判定
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BeigeFrame {
    pub owner: usize,
    pub target: usize,
    pub source: Option<usize>,
}

pub(crate) fn step_beige(st: &mut SgsState, f: BeigeFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = st.seats[f.owner]
        .hand
        .iter()
        .map(|id| PendingOption {
            id: format!("choose:{id}"),
            label: format!("悲歌：弃置【{}】", turn::card_label(st, id)),
        })
        .collect();
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    options.push(pass_option("不发动悲歌"));
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.owner].player_id.clone(),
        skill: Some("beige".into()),
        options,
        data: json!({ "skill": "beige", "target": st.seats[f.target].player_id }),
    });
    replace_top(st, Frame::Beige(f));
    Ok(true)
}

pub(crate) fn act_beige(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Beige(f)) => f,
        _ => return Err(internal("栈顶不是悲歌帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动悲歌".to_string());
    }
    let card = action
        .strip_prefix("choose:")
        .ok_or_else(|| illegal("需要选择弃置的牌或放弃"))?;
    if !st.seats[f.owner].hand.iter().any(|c| c == card) {
        return Err(illegal("手牌中没有该牌"));
    }
    turn::take_card(st, f.owner, card)?;
    turn::to_discard_card(st, card);
    pop(st);
    st.push_log(
        Some(&st.seats[f.owner].player_id.clone()),
        format!(
            "悲歌：弃置【{}】，令 {} 判定",
            turn::card_label(st, card),
            st.seats[f.target].general_name
        ),
    );
    push(
        st,
        Frame::QuickJudge(crate::resolve::QuickJudgeFrame {
            owner: f.owner,
            reason: "beige".into(),
            stage: crate::resolve::QuickJudgeStage::Reveal,
            judge_card: None,
            rewrite_order: Vec::new(),
            rewrite_cursor: 0,
            data: json!({ "target": f.target, "source": f.source }),
            saved_outcome: None,
        }),
    );
    Ok("悲歌：判定中".to_string())
}

/// 界化身：移去至多两张（优先非展示的）化身牌，获得等量新的化身牌。
fn huashen_exchange(st: &mut SgsState, actor: usize) -> String {
    let active = st.seats[actor].huashen_active.clone();
    let all = st.seats[actor].huashen.clone();
    let mut removed = 0;
    let mut keep = Vec::new();
    for g in all {
        if Some(&g) == active.as_ref() || removed >= 2 {
            keep.push(g);
        } else {
            removed += 1;
        }
    }
    if removed == 0 {
        return "化身：没有可移去的化身牌".to_string();
    }
    st.seats[actor].huashen = keep;
    let mut gained = Vec::new();
    for _ in 0..removed {
        let pool: Vec<String> = crate::data::general_codes_in_packs(&st.config.general_packs)
            .into_iter()
            .filter(|code| skills::general_ready(code))
            .map(str::to_string)
            .filter(|code| !st.seats[actor].huashen.contains(code))
            .collect();
        if pool.is_empty() {
            break;
        }
        let idx = st.next_index(pool.len());
        let code = pool[idx].clone();
        st.seats[actor].huashen.push(code.clone());
        gained.push(
            crate::data::general(&code)
                .map(|g| g.name.to_string())
                .unwrap_or(code),
        );
    }
    st.push_log(
        Some(&st.seats[actor].player_id.clone()),
        format!(
            "化身：移去 {removed} 张化身牌，获得 {}",
            gained.join("、")
        ),
    );
    format!("化身：已换入 {}", gained.join("、"))
}

/// 初始化身（finish_setup 调用）：随机两张已实现武将牌，展示第一张并获得其一个技能。
pub(crate) fn init_huashen(st: &mut SgsState, seat: usize) {
    if !skills::has(st, seat, "huashen") {
        return;
    }
    // 界化身：开局获得三张化身牌。
    let want = if is_jiexian(st, seat) { 3 } else { 2 };
    let pool: Vec<String> = crate::data::general_codes_in_packs(&st.config.general_packs)
        .into_iter()
        .filter(|code| skills::general_ready(code))
        .map(str::to_string)
        .collect();
    if pool.len() < want {
        return;
    }
    let mut picked = Vec::new();
    while picked.len() < want {
        let idx = (st.next_u64() as usize) % pool.len();
        let code = pool[idx].clone();
        if !picked.contains(&code) {
            picked.push(code);
        }
    }
    st.seats[seat].huashen = picked.clone();
    st.seats[seat].huashen_active = picked.first().cloned();
    if let Some(skill) = picked
        .first()
        .and_then(|g| skills::first_implemented_skill(g))
    {
        skills::grant_skill(st, seat, skill);
    }
    let names: Vec<String> = picked
        .iter()
        .map(|g| {
            crate::data::general(g)
                .map(|d| d.name.to_string())
                .unwrap_or_else(|| g.clone())
        })
        .collect();
    st.push_log(
        Some(&st.seats[seat].player_id.clone()),
        format!("化身：获得 {}", names.join("、")),
    );
}

// ---------------------------------------------------------------------------
// 觉醒 / 新生 / 关联查询
// ---------------------------------------------------------------------------

pub(crate) fn awaken_triggers(st: &mut SgsState) {
    let seat = st.turn;
    let pid = st.seats[seat].player_id.clone();
    // 凿险：田 ≥3 → 减 1 上限并获得急袭。
    if skills::has(st, seat, "zaoxian")
        && !st.seats[seat].flags.contains_key("awaken_zaoxian")
        && st.seats[seat]
            .marks
            .get("tian")
            .is_some_and(|v| v.len() >= 3)
    {
        st.seats[seat].flags.insert("awaken_zaoxian".into(), true);
        st.seats[seat].max_hp = (st.seats[seat].max_hp - 1).max(1);
        st.seats[seat].hp = st.seats[seat].hp.min(st.seats[seat].max_hp);
        skills::grant_skill(st, seat, "jixi");
        // 界凿险（界邓艾）：本回合结束后执行一个额外回合。
        if skills::is_jiexian(st, seat) {
            crate::resolve_jiexian4::mark_zaoxian_extra_turn(st, seat);
        }
        st.push_log(
            Some(&pid),
            "凿险：觉醒，减少 1 点体力上限并获得「急袭」",
        );
    }
    // 志继：无手牌 → 二选一，减 1 上限并获得观星（帧内处理）。
    if skills::has(st, seat, "zhiji")
        && !st.seats[seat].flags.contains_key("awaken_zhiji")
        && st.seats[seat].hand.is_empty()
    {
        st.seats[seat].flags.insert("awaken_zhiji".into(), true);
        push(st, Frame::Awaken(AwakenFrame { actor: seat }));
    }
    // 魂姿：体力为 1 → 减 1 上限并获得英姿/英魂。
    if skills::has(st, seat, "hunzi")
        && !st.seats[seat].flags.contains_key("awaken_hunzi")
        && st.seats[seat].hp == 1
    {
        st.seats[seat].flags.insert("awaken_hunzi".into(), true);
        st.seats[seat].max_hp = (st.seats[seat].max_hp - 1).max(1);
        st.seats[seat].hp = st.seats[seat].hp.min(st.seats[seat].max_hp);
        skills::grant_skill(st, seat, "yingzi");
        skills::grant_skill(st, seat, "yinghun");
        // 界魂姿（界孙策）：本回合结束阶段摸两张或回复 1 点。
        if skills::is_jiexian(st, seat) {
            st.seats[seat].flags.insert("hunzi_end".into(), true);
        }
        st.push_log(
            Some(&pid),
            "魂姿：觉醒，减少 1 点体力上限并获得「英姿」「英魂」",
        );
    }
    // 若愚：主公且体力最小 → 增 1 上限、回复并获得激将（界版回复至 3 并获得思蜀）。
    if skills::has(st, seat, "ruoyu")
        && skills::is_lord(st, seat)
        && !st.seats[seat].flags.contains_key("awaken_ruoyu")
        && st
            .alive_indices()
            .into_iter()
            .all(|i| i == seat || st.seats[i].hp >= st.seats[seat].hp)
    {
        st.seats[seat].flags.insert("awaken_ruoyu".into(), true);
        st.seats[seat].max_hp += 1;
        if is_jiexian(st, seat) {
            st.seats[seat].hp = 3.min(st.seats[seat].max_hp);
            skills::grant_skill(st, seat, "jijiang");
            skills::grant_skill(st, seat, "sishu");
            st.push_log(
                Some(&pid),
                "若愚：觉醒，增加 1 点体力上限，回复至 3 点体力并获得「激将」「思蜀」",
            );
        } else {
            st.seats[seat].hp = (st.seats[seat].hp + 1).min(st.seats[seat].max_hp);
            skills::grant_skill(st, seat, "jijiang");
            st.push_log(Some(&pid), "若愚：觉醒，增加 1 点体力上限并获得「激将」");
        }
    }
}

/// 新生：受到 1 点伤害后，获得一张新的化身牌（随机、不重复）。
pub(crate) fn xinsheng_gain(st: &mut SgsState, seat: usize) {
    if !skills::has(st, seat, "xinsheng") {
        return;
    }
    let pool: Vec<String> = crate::data::general_codes_in_packs(&st.config.general_packs)
        .into_iter()
        .filter(|code| skills::general_ready(code))
        .map(str::to_string)
        .filter(|code| !st.seats[seat].huashen.contains(code))
        .collect();
    if pool.is_empty() {
        return;
    }
    let idx = (st.next_u64() as usize) % pool.len();
    let code = pool[idx].clone();
    st.seats[seat].huashen.push(code.clone());
    let name = crate::data::general(&code)
        .map(|g| g.name.to_string())
        .unwrap_or(code);
    st.push_log(
        Some(&st.seats[seat].player_id.clone()),
        format!("新生：获得化身牌「{name}」"),
    );
}

/// 固政持有者（存活、非弃牌者）。
pub(crate) fn guzheng_owner(st: &SgsState, discarder: usize) -> Option<usize> {
    st.alive_indices()
        .into_iter()
        .find(|i| *i != discarder && skills::has(st, *i, "guzheng"))
}

/// 制霸主公（存活、具制霸且为主公）。
pub(crate) fn zhiba_lord(st: &SgsState, challenger: usize) -> Option<usize> {
    st.alive_indices()
        .into_iter()
        .find(|i| *i != challenger && skills::has(st, *i, "zhiba") && skills::is_lord(st, *i))
}

/// 悲歌持有者（存活、有手牌）。
pub(crate) fn beige_owner(st: &SgsState) -> Option<usize> {
    st.alive_indices()
        .into_iter()
        .find(|i| skills::has(st, *i, "beige") && !st.seats[*i].hand.is_empty())
}

/// 巧变选项结算（弃牌后或支付「变」后）。
pub(crate) fn apply_qiaobian_choice(
    st: &mut SgsState,
    actor: usize,
    choice: &str,
) -> Result<String, EngineError> {
    match choice {
        "judge" => {
            st.set_counter("qiaobian_skip_judge", 1);
            Ok("巧变：跳过判定阶段".to_string())
        }
        "draw" => {
            st.set_counter("skip_draw", 1);
            replace_top(
                st,
                Frame::Qiaobian(QiaobianFrame {
                    actor,
                    remaining: 2,
                }),
            );
            Ok("巧变：请选择获得手牌的角色".to_string())
        }
        "play" => {
            st.set_counter("skip_play", 1);
            replace_top(
                st,
                Frame::Jiewei(crate::resolve_wind::JieweiFrame {
                    actor,
                    stage: crate::resolve_wind::JieweiStage::Move,
                }),
            );
            Ok("巧变：请选择移动的场牌".to_string())
        }
        "discard" => {
            st.set_counter("qiaobian_skip_discard", 1);
            Ok("巧变：跳过弃牌阶段".to_string())
        }
        other => Err(illegal(format!("未知巧变选项：{other}"))),
    }
}
