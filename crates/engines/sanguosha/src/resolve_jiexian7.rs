//! P4.4.2 界限突破（批次 J6：8 将）技能帧与钩子。
//!
//! 覆盖：界贞烈（王异：无效后二选一）、翦灭（张春华）、界宗室（刘表：按势力挡伤）、
//! 界称象（曹冲：点数和 13 → 下次多亮一张）、界甘露（吴国太：超出差额弃牌）、
//! 界当先（廖化：额外阶段获【杀】且未造成伤害则自伤）。

use crate::internal;
use crate::resolve::{pop, push, replace_top, top, DamageType, Frame, RespondFrame, RespondKind};
use crate::skills::{has, is_jiexian};
use crate::state::*;
use crate::turn;
use game_engine_core::EngineError;
use serde_json::json;

fn pid(st: &SgsState, seat: usize) -> String {
    st.seats[seat].player_id.clone()
}

// ---------------------------------------------------------------------------
// 界贞烈（界王异）：令牌无效后，可获得使用者一张牌 或 本回合结束阶段发动一次秘计
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZhenLieJieFrame {
    pub owner: usize,
    pub user: usize,
}

pub(crate) fn step_zhenlie_jie(st: &mut SgsState, f: ZhenLieJieFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("zhenlie_jie".into()),
        options: vec![
            PendingOption {
                id: "choose:gain".into(),
                label: format!("贞烈：获得 {} 一张牌", st.seats[f.user].general_name),
            },
            PendingOption {
                id: "choose:miji".into(),
                label: "贞烈：本回合结束阶段发动一次「秘计」".into(),
            },
        ],
        data: json!({ "skill": "zhenlie_jie" }),
    });
    replace_top(st, Frame::ZhenLieJie(f));
    Ok(true)
}

pub(crate) fn act_zhenlie_jie(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ZhenLieJie(f)) => f,
        _ => return Err(internal("栈顶不是贞烈帧")),
    };
    pop(st);
    match action {
        "choose:gain" => {
            let stolen = crate::resolve_jiexian::take_one_from_region(st, f.user);
            match stolen {
                Some(card) => {
                    let label = turn::card_label(st, &card);
                    st.seats[f.owner].hand.push(card);
                    st.push_log(
                        Some(&pid(st, f.owner)),
                        format!("贞烈：获得 {} 的【{label}】", st.seats[f.user].general_name),
                    );
                    Ok("贞烈：已获得一张牌".to_string())
                }
                None => {
                    st.push_log(Some(&pid(st, f.owner)), "贞烈：对方没有可获得的牌");
                    Ok("贞烈：无牌可获得".to_string())
                }
            }
        }
        "choose:miji" => {
            st.set_counter("miji_extra", 1);
            st.push_log(
                Some(&pid(st, f.owner)),
                "贞烈：本回合结束阶段额外发动一次「秘计」",
            );
            Ok("贞烈：已记录额外秘计".to_string())
        }
        _ => Err(crate::illegal("未知贞烈选项")),
    }
}

// ---------------------------------------------------------------------------
// 翦灭（界张春华）：同时选颜色 → 各弃该颜色手牌 → 弃牌较多者视为对对方使用【决斗】
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JianMieStage {
    Target,
    OwnerColor,
    TargetColor,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JianMieFrame {
    pub actor: usize,
    pub target: Option<usize>,
    pub stage: JianMieStage,
    pub owner_color: Option<bool>,
}

pub(crate) fn step_jianmie(st: &mut SgsState, f: JianMieFrame) -> Result<bool, EngineError> {
    match f.stage {
        JianMieStage::Target => {
            let options: Vec<PendingOption> = st
                .alive_indices()
                .into_iter()
                .filter(|i| *i != f.actor && !st.seats[*i].hand.is_empty())
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("翦灭：与 {} 同时选颜色", st.seats[i].general_name),
                })
                .collect();
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: pid(st, f.actor),
                skill: Some("jianmie".into()),
                options,
                data: json!({ "skill": "jianmie", "stage": "target" }),
            });
            replace_top(st, Frame::JianMie(f));
            Ok(true)
        }
        JianMieStage::OwnerColor => {
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: pid(st, f.actor),
                skill: Some("jianmie".into()),
                options: vec![
                    PendingOption {
                        id: "choose:red".into(),
                        label: "翦灭：选择红色".into(),
                    },
                    PendingOption {
                        id: "choose:black".into(),
                        label: "翦灭：选择黑色".into(),
                    },
                ],
                data: json!({ "skill": "jianmie", "stage": "owner_color" }),
            });
            replace_top(st, Frame::JianMie(f));
            Ok(true)
        }
        JianMieStage::TargetColor => {
            let Some(target) = f.target else {
                pop(st);
                return Ok(false);
            };
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: pid(st, target),
                skill: Some("jianmie".into()),
                options: vec![
                    PendingOption {
                        id: "choose:red".into(),
                        label: "翦灭：选择红色".into(),
                    },
                    PendingOption {
                        id: "choose:black".into(),
                        label: "翦灭：选择黑色".into(),
                    },
                ],
                data: json!({ "skill": "jianmie", "stage": "target_color" }),
            });
            replace_top(st, Frame::JianMie(f));
            Ok(true)
        }
    }
}

fn discard_color(st: &mut SgsState, seat: usize, red: bool) -> u32 {
    let hand = std::mem::take(&mut st.seats[seat].hand);
    let mut kept = Vec::new();
    let mut n = 0;
    for id in hand {
        let is_red = st
            .cards
            .get(&id)
            .is_some_and(|c| crate::skills::is_red(c.suit));
        if is_red == red {
            turn::discard_by(st, Some(seat), &id);
            n += 1;
        } else {
            kept.push(id);
        }
    }
    st.seats[seat].hand = kept;
    n
}

pub(crate) fn act_jianmie(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::JianMie(f)) => f,
        _ => return Err(internal("栈顶不是翦灭帧")),
    };
    match f.stage {
        JianMieStage::Target => {
            let target_pid = action
                .strip_prefix("target:")
                .ok_or_else(|| crate::illegal("需要选择一名其他角色"))?;
            let target = st
                .seat_index(target_pid)
                .ok_or_else(|| crate::illegal("目标不在座"))?;
            replace_top(
                st,
                Frame::JianMie(JianMieFrame {
                    actor: f.actor,
                    target: Some(target),
                    stage: JianMieStage::OwnerColor,
                    owner_color: None,
                }),
            );
            Ok("翦灭：请选择颜色".to_string())
        }
        JianMieStage::OwnerColor => {
            let red = match action {
                "choose:red" => true,
                "choose:black" => false,
                _ => return Err(crate::illegal("需要选择颜色")),
            };
            replace_top(
                st,
                Frame::JianMie(JianMieFrame {
                    actor: f.actor,
                    target: f.target,
                    stage: JianMieStage::TargetColor,
                    owner_color: Some(red),
                }),
            );
            Ok("翦灭：等待对方选择颜色".to_string())
        }
        JianMieStage::TargetColor => {
            let Some(target) = f.target else {
                return Err(internal("翦灭缺少目标"));
            };
            let target_red = match action {
                "choose:red" => true,
                "choose:black" => false,
                _ => return Err(crate::illegal("需要选择颜色")),
            };
            let owner_red = f.owner_color.unwrap_or(true);
            let owner_n = discard_color(st, f.actor, owner_red);
            let target_n = discard_color(st, target, target_red);
            pop(st);
            st.push_log(
                Some(&pid(st, f.actor)),
                format!(
                    "翦灭：张春华弃 {owner_n} 张，{} 弃 {target_n} 张",
                    st.seats[target].general_name
                ),
            );
            // 弃牌较多者视为对另一方使用【决斗】；平手则不发生。
            let (user, victim) = if owner_n > target_n {
                (f.actor, target)
            } else if target_n > owner_n {
                (target, f.actor)
            } else {
                return Ok("翦灭：平手，无【决斗】".to_string());
            };
            push(
                st,
                Frame::Respond(RespondFrame {
                    kind: RespondKind::Duel,
                    actor: victim,
                    card: "jianmie".into(),
                    source: user,
                    target: Some(user),
                    damage: 1,
                    damage_type: DamageType::Normal,
                    consume_card: false,
                    ignore_armor: false,
                    bagua_used: false,
                    need: 1,
                    played: 0,
                    no_shan: false,
                    duel_initiator: None,
                    is_sha: false,
                }),
            );
            Ok("翦灭：弃牌较多者视为使用【决斗】".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// 界宗室（界刘表）：每个势力限一次，防止其他角色对你的伤害并令其获得你一张牌
// ---------------------------------------------------------------------------

pub(crate) fn zongshi_guard(st: &mut SgsState, victim: usize, source: Option<usize>) -> bool {
    let Some(src) = source else {
        return false;
    };
    if src == victim || !has(st, victim, "zongshi") || !is_jiexian(st, victim) {
        return false;
    }
    let kingdom = crate::skills::kingdom_of(st, src).to_string();
    if kingdom.is_empty() || st.counter(&format!("zongshi_guard:{kingdom}")) > 0 {
        return false;
    }
    st.set_counter(&format!("zongshi_guard:{kingdom}"), 1);
    let card = crate::resolve_jiexian::take_one_from_region(st, victim);
    match card {
        Some(card) => {
            let label = turn::card_label(st, &card);
            st.seats[src].hand.push(card);
            st.push_log(
                Some(&pid(st, victim)),
                format!(
                    "宗室：防止 {} 的伤害，其获得你的【{label}】",
                    st.seats[src].general_name
                ),
            );
        }
        None => {
            st.push_log(
                Some(&pid(st, victim)),
                format!("宗室：防止 {} 的伤害", st.seats[src].general_name),
            );
        }
    }
    true
}

// ---------------------------------------------------------------------------
// 界当先（界廖化）：额外出牌阶段开始时获得一张【杀】；该阶段未造成伤害则自伤 1
// ---------------------------------------------------------------------------

pub(crate) fn dangxian_jie_start(st: &mut SgsState, seat: usize) {
    if !has(st, seat, "dangxian") || !is_jiexian(st, seat) {
        return;
    }
    // 从牌堆（其次弃牌堆）获得一张【杀】。
    let sha = st
        .deck
        .draw
        .iter()
        .find(|id| {
            st.cards
                .get(*id)
                .is_some_and(|c| matches!(c.def.as_str(), "sha" | "huo_sha" | "lei_sha"))
        })
        .cloned()
        .or_else(|| {
            st.deck
                .discard
                .iter()
                .find(|id| {
                    st.cards
                        .get(*id)
                        .is_some_and(|c| matches!(c.def.as_str(), "sha" | "huo_sha" | "lei_sha"))
                })
                .cloned()
        });
    if let Some(card) = sha {
        st.deck.draw.retain(|c| c != &card);
        st.deck.discard.retain(|c| c != &card);
        let label = turn::card_label(st, &card);
        st.seats[seat].hand.push(card);
        st.push_log(
            Some(&pid(st, seat)),
            format!("当先：获得【{label}】（额外出牌阶段）"),
        );
    }
    st.set_counter("dangxian_jie_no_dmg", 1);
}

/// 该阶段造成伤害则清除自伤标记。
pub(crate) fn dangxian_jie_on_damage(st: &mut SgsState, source: Option<usize>) {
    if let Some(src) = source {
        if has(st, src, "dangxian") && is_jiexian(st, src) {
            st.set_counter("dangxian_jie_no_dmg", 0);
        }
    }
}

/// 额外出牌阶段结束：未造成伤害则自伤 1。
pub(crate) fn dangxian_jie_end(st: &mut SgsState, seat: usize) {
    if st.counter("dangxian_jie_no_dmg") == 0 {
        return;
    }
    st.set_counter("dangxian_jie_no_dmg", 0);
    if !st.seats[seat].alive {
        return;
    }
    st.push_log(
        Some(&pid(st, seat)),
        "当先：本阶段未造成伤害，对自己造成 1 点伤害",
    );
    crate::resolve::apply_damage(st, seat, 1, None, DamageType::Normal, None);
}
