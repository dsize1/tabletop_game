//! P4.4.2 一将成名（批次 2：8 将）技能帧。
//!
//! 覆盖：将驰（曹彰）、奇策/智愚（荀攸）、潜袭（马岱）、安恤/追忆（步练师）、
//! 疠火/醇醪（程普）、弓骑/解烦（韩当）。

use crate::internal;
use crate::resolve::{pop, replace_top, top, Frame};
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
// 将驰（曹彰）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JiangChiStage {
    Choice,
    Discard,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JiangChiFrame {
    pub actor: usize,
    pub stage: JiangChiStage,
}

pub(crate) fn step_jiangchi(st: &mut SgsState, f: JiangChiFrame) -> Result<bool, EngineError> {
    let options = match f.stage {
        JiangChiStage::Choice => vec![
            PendingOption {
                id: "opt1".into(),
                label: "将驰①：摸 1 张，本回合不能使用/打出【杀】且【杀】不计手牌上限".into(),
            },
            PendingOption {
                id: "opt2".into(),
                label: "将驰②：弃 1 张牌，本回合【杀】次数 +1 且无距离限制".into(),
            },
            PendingOption {
                id: "pass".into(),
                label: "不发动将驰".into(),
            },
        ],
        JiangChiStage::Discard => {
            let mut options: Vec<PendingOption> = turn::hand_ids(st, f.actor)
                .into_iter()
                .map(|id| PendingOption {
                    id: format!("choose:{id}"),
                    label: format!("弃置 {}", turn::card_label(st, &id)),
                })
                .collect();
            options.push(PendingOption {
                id: "pass".into(),
                label: "不发动将驰".into(),
            });
            options
        }
    };
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("jiangchi".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::JiangChi(f));
    Ok(true)
}

pub(crate) fn act_jiangchi(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::JiangChi(f)) => f,
        _ => return Err(internal("栈顶不是将驰帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动将驰".to_string());
    }
    match f.stage {
        JiangChiStage::Choice => {
            if action == "opt1" {
                let drawn = draw_n(st, f.actor, 1);
                st.set_counter(&format!("sha_banned:{}", f.actor), 1);
                st.push_log(
                    Some(&pid(st, f.actor)),
                    format!("将驰：摸 {drawn} 张，本回合不能使用/打出【杀】"),
                );
                pop(st);
                Ok("将驰①结算完成".to_string())
            } else if action == "opt2" {
                replace_top(
                    st,
                    Frame::JiangChi(JiangChiFrame {
                        stage: JiangChiStage::Discard,
                        ..f
                    }),
                );
                Ok("将驰②：请选择弃置的牌".to_string())
            } else {
                Err(crate::illegal("未知的将驰选项"))
            }
        }
        JiangChiStage::Discard => {
            let card = action
                .strip_prefix("choose:")
                .ok_or_else(|| crate::illegal("需要选择弃置的牌"))?;
            turn::take_card(st, f.actor, card)?;
            let label = turn::card_label(st, card);
            turn::discard_by(st, Some(f.actor), card);
            st.set_counter("jiangchi_bonus", 1);
            st.set_counter("jiangchi_no_distance", 1);
            st.push_log(
                Some(&pid(st, f.actor)),
                format!("将驰：弃置【{label}】，本回合【杀】次数 +1 且无距离限制"),
            );
            pop(st);
            Ok("将驰②结算完成".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// 奇策（荀攸）：所有手牌当任意普通锦囊使用
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QiceFrame {
    pub actor: usize,
}

pub(crate) const QICE_CODES: &[&str] = &[
    "wuzhong_shengyou",
    "guohe_chaiqiao",
    "shunshou_qianyang",
    "juedou",
    "nanman_ruqin",
    "wanjian_qifa",
    "taoyuan_jieyi",
    "wugu_fengdeng",
    "huo_gong",
    "jiedao_sharen",
    "tiesuo_lianhuan",
];

pub(crate) fn step_qice(st: &mut SgsState, f: QiceFrame) -> Result<bool, EngineError> {
    let options: Vec<PendingOption> = QICE_CODES
        .iter()
        .map(|code| {
            let name = crate::data::card_def(code).map(|d| d.name).unwrap_or(code);
            PendingOption {
                id: format!("choose:{code}"),
                label: format!("所有手牌当【{name}】使用"),
            }
        })
        .collect();
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("qice".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::Qice(f));
    Ok(true)
}

pub(crate) fn act_qice(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Qice(f)) => f,
        _ => return Err(internal("栈顶不是奇策帧")),
    };
    let code = action
        .strip_prefix("choose:")
        .ok_or_else(|| crate::illegal("需要选择锦囊牌名"))?;
    if !QICE_CODES.contains(&code) {
        return Err(crate::illegal("奇策只能声明普通锦囊牌"));
    }
    if st.seats[f.actor].hand.is_empty() {
        return Err(crate::illegal("没有手牌可发动奇策"));
    }
    let hand = std::mem::take(&mut st.seats[f.actor].hand);
    let base = hand[0].clone();
    for id in hand.iter().skip(1) {
        turn::discard_by(st, Some(f.actor), id);
    }
    st.deck.processing.push(base.clone());
    st.virtual_defs.insert(base.clone(), code.to_string());
    st.set_counter("qice_used", 1);
    st.push_log(
        Some(&pid(st, f.actor)),
        format!(
            "奇策：将 {} 张手牌当【{}】使用",
            hand.len(),
            crate::data::card_def(code).map(|d| d.name).unwrap_or(code)
        ),
    );
    pop(st);
    crate::resolve::start_virtual_trick(st, f.actor, &base, code)?;
    Ok("奇策：请选择目标".to_string())
}

// ---------------------------------------------------------------------------
// 潜袭（马岱）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QianXiStage {
    Discard,
    Target,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QianXiFrame {
    pub actor: usize,
    pub stage: QianXiStage,
    /// 1 = 红，2 = 黑。
    pub color: u32,
}

pub(crate) fn step_qianxi(st: &mut SgsState, f: QianXiFrame) -> Result<bool, EngineError> {
    let (options, actor) = match f.stage {
        QianXiStage::Discard => (
            turn::hand_ids(st, f.actor)
                .into_iter()
                .map(|id| PendingOption {
                    id: format!("choose:{id}"),
                    label: format!("弃置 {}", turn::card_label(st, &id)),
                })
                .collect(),
            f.actor,
        ),
        QianXiStage::Target => {
            let options: Vec<PendingOption> = st
                .alive_indices()
                .into_iter()
                .filter(|i| *i != f.actor && st.seat_distance(f.actor, *i) == 1)
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("令 {} 本回合不能使用该颜色手牌", st.seats[i].general_name),
                })
                .collect();
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
        skill: Some("qianxi".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::QianXi(f));
    Ok(true)
}

pub(crate) fn act_qianxi(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::QianXi(f)) => f,
        _ => return Err(internal("栈顶不是潜袭帧")),
    };
    match f.stage {
        QianXiStage::Discard => {
            let card = action
                .strip_prefix("choose:")
                .ok_or_else(|| crate::illegal("需要弃置一张牌"))?;
            turn::take_card(st, f.actor, card)?;
            let red = st
                .cards
                .get(card)
                .is_some_and(|c| crate::skills::is_red(c.suit));
            let color = if red { 1 } else { 2 };
            let label = turn::card_label(st, card);
            turn::discard_by(st, Some(f.actor), card);
            st.push_log(
                Some(&pid(st, f.actor)),
                format!(
                    "潜袭：弃置【{label}】（{}），选择距离 1 的角色",
                    if red { "红色" } else { "黑色" }
                ),
            );
            replace_top(
                st,
                Frame::QianXi(QianXiFrame {
                    stage: QianXiStage::Target,
                    color,
                    ..f
                }),
            );
            Ok("潜袭：请选择目标".to_string())
        }
        QianXiStage::Target => {
            let target_pid = action
                .strip_prefix("target:")
                .ok_or_else(|| crate::illegal("需要选择目标"))?;
            let target = st
                .seat_index(target_pid)
                .ok_or_else(|| crate::illegal("目标不在座"))?;
            st.set_counter(&format!("qianxi:{target}"), f.color);
            st.push_log(
                Some(&pid(st, f.actor)),
                format!(
                    "潜袭：{} 本回合不能使用/打出{}色手牌",
                    st.seats[target].general_name,
                    if f.color == 1 { "红" } else { "黑" }
                ),
            );
            pop(st);
            Ok("潜袭结算完成".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// 安恤 / 追忆（步练师）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnXuStage {
    TargetA,
    TargetB,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AnXuFrame {
    pub actor: usize,
    pub stage: AnXuStage,
    pub a: Option<usize>,
}

pub(crate) fn step_anxu(st: &mut SgsState, f: AnXuFrame) -> Result<bool, EngineError> {
    let options: Vec<PendingOption> = match f.stage {
        AnXuStage::TargetA => st
            .alive_indices()
            .into_iter()
            .filter(|i| *i != f.actor)
            .map(|i| PendingOption {
                id: format!("target:{}", st.seats[i].player_id),
                label: format!("令 {} 展示并获牌", st.seats[i].general_name),
            })
            .collect(),
        AnXuStage::TargetB => {
            let a = f.a.ok_or_else(|| internal("安恤缺少目标 A"))?;
            let n = st.seats[a].hand.len();
            st.alive_indices()
                .into_iter()
                .filter(|i| *i != f.actor && *i != a && st.seats[*i].hand.len() > n)
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!(
                        "从 {}（{} 张）获得一张",
                        st.seats[i].general_name,
                        st.seats[i].hand.len()
                    ),
                })
                .collect()
        }
    };
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("anxu".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::AnXu(f));
    Ok(true)
}

pub(crate) fn act_anxu(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::AnXu(f)) => f,
        _ => return Err(internal("栈顶不是安恤帧")),
    };
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| crate::illegal("需要选择目标"))?;
    let target = st
        .seat_index(target_pid)
        .ok_or_else(|| crate::illegal("目标不在座"))?;
    match f.stage {
        AnXuStage::TargetA => {
            st.push_log(
                Some(&pid(st, f.actor)),
                format!("安恤：{} 展示手牌", st.seats[target].general_name),
            );
            replace_top(
                st,
                Frame::AnXu(AnXuFrame {
                    stage: AnXuStage::TargetB,
                    a: Some(target),
                    ..f
                }),
            );
            Ok("安恤：请选择手牌更多的角色".to_string())
        }
        AnXuStage::TargetB => {
            let a = f.a.ok_or_else(|| internal("安恤缺少目标 A"))?;
            if st.seats[target].hand.is_empty() {
                return Err(crate::illegal("目标没有手牌"));
            }
            let n = st.seats[target].hand.len();
            let idx = st.next_index(n);
            let card = st.seats[target].hand.remove(idx);
            crate::skills::lianying_on_lose_last(st, target);
            let spade = st.cards.get(&card).is_some_and(|c| c.suit == Suit::Spade);
            let label = turn::card_label(st, &card);
            st.seats[a].hand.push(card.clone());
            st.set_counter("anxu_used", 1);
            st.push_log(
                Some(&pid(st, f.actor)),
                format!(
                    "安恤：{} 获得 {} 的【{label}】",
                    st.seats[a].general_name, st.seats[target].general_name
                ),
            );
            if !spade {
                let drawn = draw_n(st, f.actor, 1);
                st.push_log(
                    Some(&pid(st, f.actor)),
                    format!("安恤：获得的牌不为黑桃，摸 {drawn} 张"),
                );
            }
            let over = st.seats[a]
                .hand
                .len()
                .saturating_sub(st.config.max_hand_cards as usize);
            if over > 0 {
                crate::resolve::push_discard(st, a, over, "hand_cap");
            }
            pop(st);
            Ok("安恤结算完成".to_string())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZhuiYiFrame {
    pub dead: usize,
    pub killer: Option<usize>,
}

pub(crate) fn step_zhuiyi(st: &mut SgsState, f: ZhuiYiFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| *i != f.dead && Some(*i) != f.killer)
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("令 {} 摸 3 并回复 1", st.seats[i].general_name),
        })
        .collect();
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动追忆".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.dead),
        skill: Some("zhuiyi".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::ZhuiYi(f));
    Ok(true)
}

pub(crate) fn act_zhuiyi(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ZhuiYi(f)) => f,
        _ => return Err(internal("栈顶不是追忆帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动追忆".to_string());
    }
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| crate::illegal("需要选择目标"))?;
    let target = st
        .seat_index(target_pid)
        .ok_or_else(|| crate::illegal("目标不在座"))?;
    let drawn = draw_n(st, target, 3);
    if st.seats[target].hp < st.seats[target].max_hp {
        st.seats[target].hp += 1;
    }
    st.push_log(
        Some(&pid(st, f.dead)),
        format!(
            "追忆：{} 摸 {drawn} 张并回复 1 点体力",
            st.seats[target].general_name
        ),
    );
    let over = st.seats[target]
        .hand
        .len()
        .saturating_sub(st.config.max_hand_cards as usize);
    if over > 0 {
        crate::resolve::push_discard(st, target, over, "hand_cap");
    }
    pop(st);
    Ok("追忆结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 疠火 / 醇醪（程普）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LiHuoFrame {
    pub actor: usize,
    pub card: String,
    pub fangtian: bool,
    pub no_distance: bool,
}

pub(crate) fn step_lihuo(st: &mut SgsState, f: LiHuoFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("lihuo".into()),
        options: vec![
            PendingOption {
                id: "choose:lihuo".into(),
                label:
                    "疠火：将此【杀】视为火【杀】（可多指定一个目标，造成伤害后你失去 1 点体力）"
                        .into(),
            },
            PendingOption {
                id: "pass".into(),
                label: "按普通【杀】使用".into(),
            },
        ],
        data: json!({}),
    });
    replace_top(st, Frame::LiHuo(f));
    Ok(true)
}

pub(crate) fn act_lihuo(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::LiHuo(f)) => f,
        _ => return Err(internal("栈顶不是疠火帧")),
    };
    if !st.seats[f.actor].hand.iter().any(|c| c == &f.card) {
        return Err(crate::illegal("手牌中没有该牌"));
    }
    pop(st);
    turn::take_card(st, f.actor, &f.card)?;
    if action == "choose:lihuo" {
        st.set_counter(&format!("lihuo:{}", f.card), 1);
        crate::resolve::start_sha_flags(
            st,
            f.actor,
            &f.card,
            f.fangtian,
            crate::resolve::DamageType::Fire,
            f.no_distance,
            true,
        )?;
        st.push_log(
            Some(&pid(st, f.actor)),
            "疠火：将普通【杀】当火【杀】使用（可多指定一个目标）",
        );
    } else {
        crate::resolve::start_sha_flags(
            st,
            f.actor,
            &f.card,
            f.fangtian,
            crate::resolve::DamageType::Normal,
            f.no_distance,
            false,
        )?;
        st.push_log(Some(&pid(st, f.actor)), "使用普通【杀】");
    }
    Ok("请选择【杀】的目标".to_string())
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChunLaoFrame {
    pub actor: usize,
    pub selected: Vec<String>,
}

pub(crate) fn step_chunlao(st: &mut SgsState, f: ChunLaoFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = turn::hand_ids(st, f.actor)
        .into_iter()
        .filter(|id| {
            st.cards
                .get(id)
                .is_some_and(|c| matches!(c.def.as_str(), "sha" | "huo_sha" | "lei_sha"))
        })
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
        label: "不发动醇醪".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("chunlao".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::ChunLao(f));
    Ok(true)
}

pub(crate) fn act_chunlao(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ChunLao(f)) => f,
        _ => return Err(internal("栈顶不是醇醪帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动醇醪".to_string());
    }
    if let Some(card) = action.strip_prefix("choose:") {
        let mut selected = f.selected.clone();
        if let Some(pos) = selected.iter().position(|c| c == card) {
            selected.remove(pos);
        } else {
            if !st.seats[f.actor].hand.iter().any(|c| c == card) {
                return Err(crate::illegal("手牌中没有该牌"));
            }
            selected.push(card.to_string());
        }
        replace_top(st, Frame::ChunLao(ChunLaoFrame { selected, ..f }));
        return Ok("已更新醇醪选择".to_string());
    }
    if action != "confirm" || f.selected.is_empty() {
        return Err(crate::illegal("请选择至少 1 张【杀】"));
    }
    let n = f.selected.len();
    for id in &f.selected {
        turn::take_card(st, f.actor, id)?;
        crate::skills::add_mark(st, f.actor, "chun", id);
    }
    st.push_log(
        Some(&pid(st, f.actor)),
        format!("醇醪：将 {n} 张【杀】置于武将牌上"),
    );
    pop(st);
    Ok("醇醪结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 弓骑 / 解烦（韩当）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GongQiStage {
    Discard,
    Target,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GongQiFrame {
    pub actor: usize,
    pub stage: GongQiStage,
}

pub(crate) fn step_gongqi(st: &mut SgsState, f: GongQiFrame) -> Result<bool, EngineError> {
    let (options, actor): (Vec<PendingOption>, usize) = match f.stage {
        GongQiStage::Discard => (
            turn::hand_ids(st, f.actor)
                .into_iter()
                .map(|id| PendingOption {
                    id: format!("choose:{id}"),
                    label: format!("弃置 {}", turn::card_label(st, &id)),
                })
                .collect(),
            f.actor,
        ),
        GongQiStage::Target => (
            st.alive_indices()
                .into_iter()
                .filter(|i| *i != f.actor && turn::has_cards_to_take(st, *i))
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("弃置 {} 一张牌", st.seats[i].general_name),
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
        skill: Some("gongqi".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::GongQi(f));
    Ok(true)
}

pub(crate) fn act_gongqi(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::GongQi(f)) => f,
        _ => return Err(internal("栈顶不是弓骑帧")),
    };
    if f.stage == GongQiStage::Discard {
        let card = action
            .strip_prefix("choose:")
            .ok_or_else(|| crate::illegal("需要弃置一张牌"))?;
        turn::take_card(st, f.actor, card)?;
        let is_equip = st
            .cards
            .get(card)
            .and_then(|c| crate::data::card_def(&c.def))
            .is_some_and(|d| d.equip_slot.is_some());
        let label = turn::card_label(st, card);
        turn::discard_by(st, Some(f.actor), card);
        st.set_counter("gongqi_unlimited", 1);
        st.push_log(
            Some(&pid(st, f.actor)),
            format!("弓骑：弃置【{label}】，本回合攻击范围无限"),
        );
        if is_equip
            && st
                .alive_indices()
                .into_iter()
                .any(|i| i != f.actor && turn::has_cards_to_take(st, i))
        {
            replace_top(
                st,
                Frame::GongQi(GongQiFrame {
                    stage: GongQiStage::Target,
                    ..f
                }),
            );
            return Ok("弓骑：可弃置一名其他角色一张牌".to_string());
        }
        pop(st);
        return Ok("弓骑结算完成".to_string());
    }
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| crate::illegal("需要选择目标"))?;
    let target = st
        .seat_index(target_pid)
        .ok_or_else(|| crate::illegal("目标不在座"))?;
    if let Some(card) = crate::resolve_jiexian::take_one_from_region(st, target) {
        let label = turn::card_label(st, &card);
        turn::discard_by(st, Some(target), &card);
        st.push_log(
            Some(&pid(st, f.actor)),
            format!("弓骑：弃置 {} 的【{label}】", st.seats[target].general_name),
        );
    }
    pop(st);
    Ok("弓骑结算完成".to_string())
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JieFanFrame {
    pub actor: usize,
}

pub(crate) fn step_jiefan(st: &mut SgsState, f: JieFanFrame) -> Result<bool, EngineError> {
    let options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("选择 {}", st.seats[i].general_name),
        })
        .collect();
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("jiefan".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::JieFan(f));
    Ok(true)
}

pub(crate) fn act_jiefan(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::JieFan(f)) => f,
        _ => return Err(internal("栈顶不是解烦帧")),
    };
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| crate::illegal("需要选择目标"))?;
    let target = st
        .seat_index(target_pid)
        .ok_or_else(|| crate::illegal("目标不在座"))?;
    st.seats[f.actor].flags.insert("jiefan_used".into(), true);
    let voters: Vec<usize> = st
        .alive_indices()
        .into_iter()
        .filter(|i| *i != target && st.in_attack_range(*i, target))
        .collect();
    for seat in voters {
        // 自动口径：优先弃置武器牌（手牌→装备区），否则令目标摸 1。
        let hand_weapon = st.seats[seat].hand.iter().find_map(|id| {
            st.cards.get(id).and_then(|c| {
                let d = crate::data::card_def(&c.def)?;
                (d.equip_slot == Some(EquipSlot::Weapon)).then(|| id.clone())
            })
        });
        if let Some(card) = hand_weapon {
            turn::take_card(st, seat, &card)?;
            let label = turn::card_label(st, &card);
            turn::discard_by(st, Some(seat), &card);
            st.push_log(Some(&pid(st, seat)), format!("解烦：弃置武器牌【{label}】"));
        } else if st.equip_def(seat, EquipSlot::Weapon).is_some() {
            if let Some(card) = st.seats[seat]
                .equips
                .get(&EquipSlot::Weapon)
                .cloned()
                .flatten()
            {
                st.seats[seat].equips.insert(EquipSlot::Weapon, None);
                crate::skills::xiaoji_on_lose_equip(st, seat);
                let label = turn::card_label(st, &card);
                turn::discard_by(st, Some(seat), &card);
                st.push_log(Some(&pid(st, seat)), format!("解烦：弃置武器牌【{label}】"));
            }
        } else {
            let drawn = draw_n(st, target, 1);
            st.push_log(
                Some(&pid(st, seat)),
                format!("解烦：{} 摸 {drawn} 张", st.seats[target].general_name),
            );
        }
    }
    pop(st);
    Ok("解烦结算完成".to_string())
}
