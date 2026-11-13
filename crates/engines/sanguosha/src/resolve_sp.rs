//! P4.4.1 SP-1 技能帧（袁术/王基/严颜/王平/陆绩/孙亮/蒯越蒯良/许攸）。
//!
//! 机制口径见 ADR-0028 与 `skills.md` §2.5：
//! - 转换技：`skills::yj_yin` / `toggle_yj`（拒战 / 审时 / 成略）
//! - 计数标记：「橘」`Seat.mark_counts`
//! - 牌堆底摸牌（寸目）/ 置顶（恃才）
//! - 伪帝借用主公技（`skills::has` 内处理）

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

/// 摸至指定张数（手牌不足 `n` 时补齐；已超过则不动）。
fn draw_to(st: &mut SgsState, seat: usize, n: usize) -> usize {
    let cur = st.seats[seat].hand.len();
    if cur >= n {
        return 0;
    }
    draw_capped(st, seat, (n - cur) as u32)
}

fn general_name(st: &SgsState, seat: usize) -> String {
    st.seats[seat].general_name.clone()
}

// ---------------------------------------------------------------------------
// 怀橘（陆绩）：开局 3 枚「橘」；伤害防止（resolve 内联）；摸牌 +1（turn 内联）
// ---------------------------------------------------------------------------

pub(crate) fn init_huaiju(st: &mut SgsState, seat: usize) {
    if !skills::has(st, seat, "huaiju") {
        return;
    }
    skills::add_ju(st, seat, 3);
    let pid = st.seats[seat].player_id.clone();
    st.push_log(Some(&pid), "怀橘：获得 3 枚「橘」");
}

/// 怀橘：受到伤害时防止并移除 1 枚「橘」。返回是否已防止。
pub(crate) fn huaiju_prevent(st: &mut SgsState, victim: usize) -> bool {
    if !st.seats.get(victim).is_some_and(|s| s.alive) || skills::ju_count(st, victim) == 0 {
        return false;
    }
    skills::remove_ju(st, victim, 1);
    let left = skills::ju_count(st, victim);
    let pid = st.seats[victim].player_id.clone();
    st.push_log(
        Some(&pid),
        format!("怀橘：移除 1 枚「橘」防止伤害（剩余 {left}）"),
    );
    true
}

// ---------------------------------------------------------------------------
// 遗礼（陆绩）：出牌阶段开始时失 1 体力或移 1 橘，令一名其他角色获得 1 橘
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum YiliStage {
    Cost,
    Target,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct YiliFrame {
    pub actor: usize,
    pub stage: YiliStage,
}

pub(crate) fn step_yili(st: &mut SgsState, f: YiliFrame) -> Result<bool, EngineError> {
    match f.stage {
        YiliStage::Cost => {
            let ju = skills::ju_count(st, f.actor);
            let mut options = vec![PendingOption {
                id: "choose:yili:hp".into(),
                label: "遗礼：失去 1 点体力".into(),
            }];
            if ju > 0 {
                options.push(PendingOption {
                    id: "choose:yili:ju".into(),
                    label: "遗礼：移除 1 枚「橘」".into(),
                });
            }
            options.push(pass_option("不发动遗礼"));
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("yili".into()),
                options,
                data: json!({ "skill": "yili" }),
            });
            replace_top(st, Frame::Yili(f));
            Ok(true)
        }
        YiliStage::Target => {
            let options: Vec<PendingOption> = st
                .alive_indices()
                .into_iter()
                .filter(|i| *i != f.actor)
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("遗礼：令 {} 获得 1 枚「橘」", general_name(st, i)),
                })
                .collect();
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("yili".into()),
                options,
                data: json!({ "stage": "yili" }),
            });
            replace_top(st, Frame::Yili(f));
            Ok(true)
        }
    }
}

pub(crate) fn act_yili(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Yili(f)) => f,
        _ => return Err(internal("栈顶不是遗礼帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动遗礼".to_string());
    }
    let pid = st.seats[f.actor].player_id.clone();
    match action {
        "choose:yili:hp" => {
            st.seats[f.actor].hp -= 1;
            let hp = st.seats[f.actor].hp;
            st.push_log(Some(&pid), format!("遗礼：失去 1 点体力（{hp}）"));
        }
        "choose:yili:ju" => {
            if skills::ju_count(st, f.actor) == 0 {
                return Err(illegal("没有「橘」可移除"));
            }
            skills::remove_ju(st, f.actor, 1);
            st.push_log(Some(&pid), "遗礼：移除 1 枚「橘」");
        }
        _ => return Err(illegal("未知遗礼选项")),
    }
    replace_top(
        st,
        Frame::Yili(YiliFrame {
            actor: f.actor,
            stage: YiliStage::Target,
        }),
    );
    Ok("遗礼：请选择获得「橘」的角色".to_string())
}

pub(crate) fn yili_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Yili(f)) => f,
        _ => return Err(internal("栈顶不是遗礼帧")),
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
    skills::add_ju(st, target, 1);
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        format!(
            "遗礼：令 {} 获得 1 枚「橘」",
            st.seats[target].general_name
        ),
    );
    if st.seats[f.actor].hp <= 0 {
        crate::resolve::push_dying(st, f.actor, None);
    }
    Ok("遗礼：已完成".to_string())
}

// ---------------------------------------------------------------------------
// 整论（陆绩）：无「橘」时摸牌阶段可跳过摸牌并获得 1 枚「橘」
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZhenglunFrame {
    pub actor: usize,
}

pub(crate) fn step_zhenglun(st: &mut SgsState, f: ZhenglunFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("zhenglun".into()),
        options: vec![
            PendingOption {
                id: "choose:zhenglun".into(),
                label: "整论：跳过摸牌阶段并获得 1 枚「橘」".into(),
            },
            pass_option("正常摸牌"),
        ],
        data: json!({ "skill": "zhenglun" }),
    });
    replace_top(st, Frame::Zhenglun(f));
    Ok(true)
}

pub(crate) fn act_zhenglun(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Zhenglun(f)) => f,
        _ => return Err(internal("栈顶不是整论帧")),
    };
    pop(st);
    if action != "choose:zhenglun" {
        return Ok("整论：正常摸牌".to_string());
    }
    st.set_counter("skip_draw", 1);
    skills::add_ju(st, f.actor, 1);
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        "整论：跳过摸牌阶段，获得 1 枚「橘」",
    );
    Ok("整论：已跳过摸牌".to_string())
}

// ---------------------------------------------------------------------------
// 奇制（王基）：回合内使用非装备牌后，弃一名其他角色一张牌，然后其摸 1
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QizhiStage {
    Ask,
    Target,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QizhiFrame {
    pub actor: usize,
    pub stage: QizhiStage,
}

/// 使用非装备牌后的奇制触发点（由 `resolve` 的用牌入口调用）。
pub(crate) fn qizhi_trigger(st: &mut SgsState, user: usize) {
    if !skills::has(st, user, "qizhi") || st.turn != user {
        return;
    }
    let has_target = st.alive_indices().into_iter().any(|i| {
        i != user
            && (!st.seats[i].hand.is_empty()
                || !st.seats[i].judge_zone.is_empty()
                || st.seats[i].equips.values().any(|c| c.is_some()))
    });
    if !has_target {
        return;
    }
    push(
        st,
        Frame::Qizhi(QizhiFrame {
            actor: user,
            stage: QizhiStage::Ask,
        }),
    );
}

pub(crate) fn step_qizhi(st: &mut SgsState, f: QizhiFrame) -> Result<bool, EngineError> {
    match f.stage {
        QizhiStage::Ask => {
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("qizhi".into()),
                options: vec![
                    PendingOption {
                        id: "choose:qizhi".into(),
                        label: "奇制：弃置一名其他角色一张牌，然后其摸 1 张".into(),
                    },
                    pass_option("不发动奇制"),
                ],
                data: json!({ "skill": "qizhi" }),
            });
            replace_top(st, Frame::Qizhi(f));
            Ok(true)
        }
        QizhiStage::Target => {
            let options: Vec<PendingOption> = st
                .alive_indices()
                .into_iter()
                .filter(|i| {
                    *i != f.actor
                        && (!st.seats[*i].hand.is_empty()
                            || !st.seats[*i].judge_zone.is_empty()
                            || st.seats[*i].equips.values().any(|c| c.is_some()))
                })
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("奇制：{}", general_name(st, i)),
                })
                .collect();
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("qizhi".into()),
                options,
                data: json!({ "stage": "qizhi" }),
            });
            replace_top(st, Frame::Qizhi(f));
            Ok(true)
        }
    }
}

pub(crate) fn act_qizhi(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Qizhi(f)) => f,
        _ => return Err(internal("栈顶不是奇制帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动奇制".to_string());
    }
    if action != "choose:qizhi" {
        return Err(illegal("未知奇制选项"));
    }
    replace_top(
        st,
        Frame::Qizhi(QizhiFrame {
            actor: f.actor,
            stage: QizhiStage::Target,
        }),
    );
    Ok("奇制：请选择目标".to_string())
}

pub(crate) fn qizhi_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Qizhi(f)) => f,
        _ => return Err(internal("栈顶不是奇制帧")),
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
    push(
        st,
        Frame::PickCard(crate::resolve::PickCardFrame {
            card: "qizhi".into(),
            user: f.actor,
            target,
            mode: crate::resolve::PickMode::Qizhi,
        }),
    );
    Ok("奇制：请选择弃置哪张牌".to_string())
}

/// 奇制结算完成：计入本回合发动次数，目标摸 1。
pub(crate) fn qizhi_done(st: &mut SgsState, actor: usize, target: usize) {
    st.bump_counter("qizhi_count");
    let drawn = draw_capped(st, target, 1);
    let pid = st.seats[actor].player_id.clone();
    st.push_log(
        Some(&pid),
        format!(
            "奇制：弃置 {} 一张牌，其摸 {drawn} 张",
            general_name(st, target)
        ),
    );
}

// ---------------------------------------------------------------------------
// 进趋（王基）：结束阶段摸 2 张，然后将手牌弃至 X（X=本回合奇制次数）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JinzouFrame {
    pub actor: usize,
}

pub(crate) fn step_jinzou(st: &mut SgsState, f: JinzouFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("jinzou".into()),
        options: vec![
            PendingOption {
                id: "choose:jinzou".into(),
                label: "进趋：摸 2 张，然后将手牌弃至本回合奇制次数".into(),
            },
            pass_option("不发动进趋"),
        ],
        data: json!({ "skill": "jinzou" }),
    });
    replace_top(st, Frame::Jinzou(f));
    Ok(true)
}

pub(crate) fn act_jinzou(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Jinzou(f)) => f,
        _ => return Err(internal("栈顶不是进趋帧")),
    };
    if action != "choose:jinzou" {
        pop(st);
        return Ok("不发动进趋".to_string());
    }
    let drawn = draw_capped(st, f.actor, 2);
    let x = st.counter("qizhi_count") as usize;
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        format!("进趋：摸 {drawn} 张，弃至 {x} 张"),
    );
    let cur = st.seats[f.actor].hand.len();
    if cur > x {
        let need = cur - x;
        replace_top(
            st,
            Frame::MultiSelect(MultiSelectFrame {
                actor: f.actor,
                purpose: "jinzou".into(),
                selected: Vec::new(),
                min: need,
                max: need,
                data: json!({ "target": x }),
            }),
        );
    } else {
        pop(st);
    }
    Ok("进趋：已摸牌".to_string())
}

// ---------------------------------------------------------------------------
// 拒战（严颜，转换技）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JuzhanFrame {
    pub owner: usize,
    /// 阳：杀的使用者；阴：【杀】的目标。
    pub other: usize,
    pub yin: bool,
}

pub(crate) fn push_juzhan_yang(st: &mut SgsState, owner: usize, user: usize) {
    if !skills::has(st, owner, "juzhan") || skills::yj_yin(st, owner, "juzhan") || user == owner {
        return;
    }
    push(
        st,
        Frame::Juzhan(JuzhanFrame {
            owner,
            other: user,
            yin: false,
        }),
    );
}

pub(crate) fn push_juzhan_yin(st: &mut SgsState, owner: usize, target: usize) {
    if !skills::has(st, owner, "juzhan") || !skills::yj_yin(st, owner, "juzhan") || target == owner {
        return;
    }
    push(
        st,
        Frame::Juzhan(JuzhanFrame {
            owner,
            other: target,
            yin: true,
        }),
    );
}

pub(crate) fn step_juzhan(st: &mut SgsState, f: JuzhanFrame) -> Result<bool, EngineError> {
    let label = if f.yin {
        format!(
            "拒战（阴）：获得 {} 一张牌，本回合不能再对其使用牌",
            general_name(st, f.other)
        )
    } else {
        format!(
            "拒战（阳）：与 {} 各摸 1 张，其本回合不能再对你使用牌",
            general_name(st, f.other)
        )
    };
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.owner].player_id.clone(),
        skill: Some("juzhan".into()),
        options: vec![
            PendingOption {
                id: "choose:juzhan".into(),
                label,
            },
            pass_option("不发动拒战"),
        ],
        data: json!({ "skill": "juzhan" }),
    });
    replace_top(st, Frame::Juzhan(f));
    Ok(true)
}

pub(crate) fn act_juzhan(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Juzhan(f)) => f,
        _ => return Err(internal("栈顶不是拒战帧")),
    };
    if action != "choose:juzhan" {
        pop(st);
        return Ok("不发动拒战".to_string());
    }
    let pid = st.seats[f.owner].player_id.clone();
    if f.yin {
        pop(st);
        push(
            st,
            Frame::PickCard(crate::resolve::PickCardFrame {
                card: "juzhan".into(),
                user: f.owner,
                target: f.other,
                mode: crate::resolve::PickMode::Juzhan,
            }),
        );
        return Ok("拒战（阴）：请选择获得哪张牌".to_string());
    }
    pop(st);
    let a = draw_capped(st, f.owner, 1);
    let b = draw_capped(st, f.other, 1);
    skills::set_deny_target(st, f.other, f.owner);
    skills::toggle_yj(st, f.owner, "juzhan");
    st.push_log(
        Some(&pid),
        format!(
            "拒战（阳）：与 {} 各摸 {a}/{b} 张，其本回合不能再对你使用牌",
            general_name(st, f.other)
        ),
    );
    Ok("拒战（阳）：已完成".to_string())
}

/// 拒战（阴）结算完成（PickCard 之后调用）。
pub(crate) fn juzhan_yin_done(st: &mut SgsState, owner: usize, other: usize) {
    skills::set_deny_target(st, owner, other);
    skills::toggle_yj(st, owner, "juzhan");
    st.push_log(
        Some(&st.seats[owner].player_id.clone()),
        format!(
            "拒战（阴）：获得 {} 一张牌，本回合不能再对其使用牌",
            general_name(st, other)
        ),
    );
}

// ---------------------------------------------------------------------------
// 飞军（王平）：弃一张牌，令手牌多于你的角色交给你一张牌，
// 或令装备区牌多于你的角色弃置一张装备
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeijunStage {
    Choose,
    Target,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FeijunFrame {
    pub actor: usize,
    pub stage: FeijunStage,
    /// `1` = 交给你一张牌；`2` = 弃置一张装备。
    pub option: u8,
    pub target: Option<usize>,
}

fn equip_count(st: &SgsState, seat: usize) -> usize {
    st.seats[seat].equips.values().filter(|c| c.is_some()).count()
}

pub(crate) fn feijun_targets(st: &SgsState, actor: usize, option: u8) -> Vec<usize> {
    st.alive_indices()
        .into_iter()
        .filter(|i| {
            if *i == actor {
                return false;
            }
            match option {
                1 => st.seats[*i].hand.len() > st.seats[actor].hand.len(),
                2 => equip_count(st, *i) > equip_count(st, actor),
                _ => false,
            }
        })
        .collect()
}

pub(crate) fn step_feijun(st: &mut SgsState, f: FeijunFrame) -> Result<bool, EngineError> {
    match f.stage {
        FeijunStage::Choose => {
            let mut options = Vec::new();
            if !feijun_targets(st, f.actor, 1).is_empty() {
                options.push(PendingOption {
                    id: "choose:feijun:take".into(),
                    label: "飞军：令手牌数大于你的角色交给你一张牌".into(),
                });
            }
            if !feijun_targets(st, f.actor, 2).is_empty() {
                options.push(PendingOption {
                    id: "choose:feijun:equip".into(),
                    label: "飞军：令装备区牌数大于你的角色弃置一张装备".into(),
                });
            }
            if options.is_empty() || st.seats[f.actor].hand.is_empty() {
                pop(st);
                return Ok(false);
            }
            options.push(pass_option("不发动飞军"));
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("feijun".into()),
                options,
                data: json!({ "skill": "feijun" }),
            });
            replace_top(st, Frame::Feijun(f));
            Ok(true)
        }
        FeijunStage::Target => {
            let options: Vec<PendingOption> = feijun_targets(st, f.actor, f.option)
                .into_iter()
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("飞军：{}", general_name(st, i)),
                })
                .collect();
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("feijun".into()),
                options,
                data: json!({ "stage": "feijun", "option": f.option }),
            });
            replace_top(st, Frame::Feijun(f));
            Ok(true)
        }
    }
}

pub(crate) fn act_feijun(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Feijun(f)) => f,
        _ => return Err(internal("栈顶不是飞军帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动飞军".to_string());
    }
    let option = match action {
        "choose:feijun:take" => 1,
        "choose:feijun:equip" => 2,
        _ => return Err(illegal("未知飞军选项")),
    };
    replace_top(
        st,
        Frame::Feijun(FeijunFrame {
            actor: f.actor,
            stage: FeijunStage::Target,
            option,
            target: None,
        }),
    );
    Ok("飞军：请选择目标".to_string())
}

pub(crate) fn feijun_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Feijun(f)) => f,
        _ => return Err(internal("栈顶不是飞军帧")),
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
    // 弃一张牌作为代价（手牌多选；由 `multiselect_confirm("feijun")` 结算）。
    replace_top(
        st,
        Frame::MultiSelect(MultiSelectFrame {
            actor: f.actor,
            purpose: "feijun".into(),
            selected: Vec::new(),
            min: 1,
            max: 1,
            data: json!({ "option": f.option, "target": target }),
        }),
    );
    Ok("飞军：请选择弃置的牌".to_string())
}

/// 飞军结算（MultiSelect 确认后调用）。
pub(crate) fn feijun_done(
    st: &mut SgsState,
    actor: usize,
    target: usize,
    option: u8,
) -> Result<String, EngineError> {
    st.set_counter("feijun_used", 1);
    let pid = st.seats[actor].player_id.clone();
    if option == 1 {
        if st.seats[target].hand.is_empty() {
            st.push_log(Some(&pid), "飞军：目标没有手牌，未获得牌");
        } else {
            let n = st.seats[target].hand.len();
            let idx = st.next_index(n);
            let card = st.seats[target].hand.remove(idx);
            let label = turn::card_label(st, &card);
            st.seats[actor].hand.push(card);
            st.push_log(
                Some(&pid),
                format!(
                    "飞军：{} 交给王平一张手牌【{label}】",
                    general_name(st, target)
                ),
            );
        }
    } else {
        let slot = EquipSlot::ALL.into_iter().find(|slot| {
            st.seats[target]
                .equips
                .get(slot)
                .map(|v| v.is_some())
                .unwrap_or(false)
        });
        if let Some(slot) = slot {
            if let Some(Some(card)) = st.seats[target].equips.insert(slot, None) {
                let label = turn::card_label(st, &card);
                turn::to_discard_card(st, &card);
                skills::xiaoji_on_lose_equip(st, target);
                st.push_log(
                    Some(&pid),
                    format!("飞军：{} 弃置装备【{label}】", general_name(st, target)),
                );
            }
        }
    }
    // 兵略：首次对一名角色发动飞军时摸 2 张。
    if skills::has(st, actor, "binglue") {
        let key = format!("feijun:{target}");
        if !st.seats[actor].flags.contains_key(&key) {
            st.seats[actor].flags.insert(key, true);
            let drawn = draw_capped(st, actor, 2);
            st.push_log(
                Some(&pid),
                format!(
                    "兵略：首次对 {} 发动飞军，摸 {drawn} 张",
                    general_name(st, target)
                ),
            );
        }
    }
    let over = st.seats[actor]
        .hand
        .len()
        .saturating_sub(st.config.max_hand_cards as usize);
    if over > 0 {
        crate::resolve::push_discard(st, actor, over, "hand_cap");
    }
    Ok("飞军：已完成".to_string())
}

// ---------------------------------------------------------------------------
// 荐降（蒯越蒯良）：成为其他角色使用牌的目标后，令手牌最少者摸 1
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JianxiangFrame {
    pub actor: usize,
    pub source: usize,
}

pub(crate) fn step_jianxiang(st: &mut SgsState, f: JianxiangFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("jianxiang".into()),
        options: vec![
            PendingOption {
                id: "choose:jianxiang".into(),
                label: "荐降：令手牌数最少的一名角色摸 1 张".into(),
            },
            pass_option("不发动荐降"),
        ],
        data: json!({ "skill": "jianxiang", "source": f.source }),
    });
    replace_top(st, Frame::Jianxiang(f));
    Ok(true)
}

pub(crate) fn act_jianxiang(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Jianxiang(f)) => f,
        _ => return Err(internal("栈顶不是荐降帧")),
    };
    pop(st);
    if action != "choose:jianxiang" {
        return Ok("不发动荐降".to_string());
    }
    let min = st
        .alive_indices()
        .into_iter()
        .map(|i| st.seats[i].hand.len())
        .min()
        .unwrap_or(0);
    let candidates: Vec<usize> = st
        .alive_indices()
        .into_iter()
        .filter(|i| st.seats[*i].hand.len() == min)
        .collect();
    if candidates.is_empty() {
        return Ok("荐降：没有可选角色".to_string());
    }
    let target = candidates[st.next_index(candidates.len())];
    let drawn = draw_capped(st, target, 1);
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        format!("荐降：令 {} 摸 {drawn} 张", general_name(st, target)),
    );
    Ok("荐降：已完成".to_string())
}

// ---------------------------------------------------------------------------
// 审时（蒯越蒯良，转换技）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShenshiStage {
    /// 阴：是否发动（观看手牌）。
    YinAsk,
    /// 阳：选择交给目标的牌。
    YangSelect,
    /// 阳：选择目标（手牌最多者）。
    YangTarget,
    /// 阳：目标死亡后，选择手牌摸至 4 张的角色。
    YangFill,
    /// 阴：选择交给其的牌。
    YinSelect,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ShenshiFrame {
    pub actor: usize,
    pub stage: ShenshiStage,
    /// 阴面：伤害来源。
    pub source: Option<usize>,
    /// 阳面：交出的牌。
    pub card: Option<String>,
}

pub(crate) fn shenshi_targets(st: &SgsState, actor: usize) -> Vec<usize> {
    let max = st
        .alive_indices()
        .into_iter()
        .filter(|i| *i != actor)
        .map(|i| st.seats[i].hand.len())
        .max()
        .unwrap_or(0);
    st.alive_indices()
        .into_iter()
        .filter(|i| *i != actor && st.seats[*i].hand.len() == max)
        .collect()
}

/// 审时（阴）触发点：其他角色对你造成伤害后。
pub(crate) fn push_shenshi_yin(st: &mut SgsState, actor: usize, source: usize) {
    if !skills::has(st, actor, "shenshi") || !skills::yj_yin(st, actor, "shenshi") {
        return;
    }
    if st.seats[actor].hand.is_empty() {
        return;
    }
    push(
        st,
        Frame::Shenshi(ShenshiFrame {
            actor,
            stage: ShenshiStage::YinAsk,
            source: Some(source),
            card: None,
        }),
    );
}

pub(crate) fn step_shenshi(st: &mut SgsState, f: ShenshiFrame) -> Result<bool, EngineError> {
    match f.stage {
        ShenshiStage::YinAsk => {
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("shenshi".into()),
                options: vec![
                    PendingOption {
                        id: "choose:shenshi".into(),
                        label: "审时（阴）：观看其手牌并交给其一张牌".into(),
                    },
                    pass_option("不发动审时（阴）"),
                ],
                data: json!({ "skill": "shenshi" }),
            });
            replace_top(st, Frame::Shenshi(f));
            Ok(true)
        }
        ShenshiStage::YangSelect => {
            replace_top(
                st,
                Frame::MultiSelect(MultiSelectFrame {
                    actor: f.actor,
                    purpose: "shenshi_yang".into(),
                    selected: Vec::new(),
                    min: 1,
                    max: 1,
                    data: json!({}),
                }),
            );
            Ok(false)
        }
        ShenshiStage::YangTarget => {
            let options: Vec<PendingOption> = shenshi_targets(st, f.actor)
                .into_iter()
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("审时：{}（手牌最多）", general_name(st, i)),
                })
                .collect();
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("shenshi".into()),
                options,
                data: json!({ "stage": "shenshi" }),
            });
            replace_top(st, Frame::Shenshi(f));
            Ok(true)
        }
        ShenshiStage::YangFill => {
            let options: Vec<PendingOption> = st
                .alive_indices()
                .into_iter()
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("审时：令 {} 摸至 4 张", general_name(st, i)),
                })
                .collect();
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("shenshi".into()),
                options,
                data: json!({ "stage": "shenshi_fill" }),
            });
            replace_top(st, Frame::Shenshi(f));
            Ok(true)
        }
        ShenshiStage::YinSelect => {
            let source = f.source.unwrap_or(f.actor);
            let revealed: Vec<String> = st.seats[source]
                .hand
                .iter()
                .map(|id| turn::card_label(st, id))
                .collect();
            st.push_log(
                Some(&st.seats[f.actor].player_id.clone()),
                format!(
                    "审时（阴）：观看 {} 的手牌（{}）",
                    general_name(st, source),
                    revealed.join("、")
                ),
            );
            replace_top(
                st,
                Frame::MultiSelect(MultiSelectFrame {
                    actor: f.actor,
                    purpose: "shenshi_yin".into(),
                    selected: Vec::new(),
                    min: 1,
                    max: 1,
                    data: json!({ "source": source, "revealed": revealed }),
                }),
            );
            Ok(false)
        }
    }
}

pub(crate) fn act_shenshi(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Shenshi(f)) => f,
        _ => return Err(internal("栈顶不是审时帧")),
    };
    if f.stage != ShenshiStage::YinAsk {
        return Err(illegal("请通过多选/目标界面继续审时"));
    }
    if action != "choose:shenshi" {
        pop(st);
        return Ok("不发动审时（阴）".to_string());
    }
    replace_top(
        st,
        Frame::Shenshi(ShenshiFrame {
            actor: f.actor,
            stage: ShenshiStage::YinSelect,
            source: f.source,
            card: None,
        }),
    );
    Ok("审时（阴）：请选择交给其的牌".to_string())
}

pub(crate) fn shenshi_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Shenshi(f)) => f,
        _ => return Err(internal("栈顶不是审时帧")),
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
    if f.stage == ShenshiStage::YangFill {
        pop(st);
        let drawn = draw_to(st, target, 4);
        st.push_log(
            Some(&st.seats[f.actor].player_id.clone()),
            format!(
                "审时：令 {} 摸至 4 张（摸 {drawn} 张）",
                general_name(st, target)
            ),
        );
        return Ok("审时：已完成".to_string());
    }
    let Some(card) = f.card.clone() else {
        pop(st);
        return Err(illegal("审时缺少交出的牌"));
    };
    if !st.seats[f.actor].hand.iter().any(|c| c == &card) {
        pop(st);
        return Err(illegal("手牌中没有该牌"));
    }
    let actor_pid = st.seats[f.actor].player_id.clone();
    turn::take_card(st, f.actor, &card)?;
    st.seats[target].hand.push(card.clone());
    st.push_log(
        Some(&actor_pid),
        format!(
            "审时（阳）：交给 {} 【{}】并造成 1 点伤害",
            general_name(st, target),
            turn::card_label(st, &card)
        ),
    );
    pop(st);
    skills::toggle_yj(st, f.actor, "shenshi");
    let hp_before = st.seats[target].hp;
    crate::resolve::apply_damage(st, target, 1, Some(f.actor), DamageType::Normal, None);
    if hp_before > 0 && st.seats[target].hp <= 0 {
        push(
            st,
            Frame::Shenshi(ShenshiFrame {
                actor: f.actor,
                stage: ShenshiStage::YangFill,
                source: None,
                card: None,
            }),
        );
    }
    Ok("审时（阳）：已完成".to_string())
}

/// 审时（阳）确认交出的牌（`multiselect_confirm` 内调用）。
pub(crate) fn shenshi_yang_confirm(st: &mut SgsState, f: &MultiSelectFrame) -> Result<String, EngineError> {
    if f.selected.len() != 1 {
        return Err(illegal("审时需选择 1 张手牌"));
    }
    replace_top(
        st,
        Frame::Shenshi(ShenshiFrame {
            actor: f.actor,
            stage: ShenshiStage::YangTarget,
            source: None,
            card: Some(f.selected[0].clone()),
        }),
    );
    Ok("审时：请选择目标".to_string())
}

/// 审时（阴）确认交出的牌（`multiselect_confirm` 内调用）。
pub(crate) fn shenshi_yin_confirm(st: &mut SgsState, f: &MultiSelectFrame) -> Result<String, EngineError> {
    if f.selected.len() != 1 {
        return Err(illegal("审时需选择 1 张手牌"));
    }
    let source = f.data["source"].as_u64().unwrap_or(f.actor as u64) as usize;
    let card = f.selected[0].clone();
    if turn::take_card(st, f.actor, &card).is_err() {
        return Err(illegal("手牌中没有该牌"));
    }
    st.seats[source].hand.push(card.clone());
    st.shenshi_marks.push(ShenshiMark {
        owner: f.actor,
        target: source,
        card: card.clone(),
    });
    skills::toggle_yj(st, f.actor, "shenshi");
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        format!(
            "审时（阴）：交给 {} 【{}】",
            general_name(st, source),
            turn::card_label(st, &card)
        ),
    );
    pop(st);
    Ok("审时（阴）：已完成".to_string())
}

/// 回合结束阶段校验审时（阴）：牌仍在目标手牌 → 摸至 4 张。
pub(crate) fn shenshi_end_turn(st: &mut SgsState) {
    if st.shenshi_marks.is_empty() {
        return;
    }
    let marks = std::mem::take(&mut st.shenshi_marks);
    for mark in marks {
        let still_there = st.seats[mark.target].alive
            && st.seats[mark.target].hand.iter().any(|c| c == &mark.card);
        if !still_there {
            continue;
        }
        let owner = mark.owner;
        if st.seats.get(owner).is_some_and(|s| s.alive) {
            let drawn = draw_to(st, owner, 4);
            st.push_log(
                Some(&st.seats[owner].player_id.clone()),
                format!(
                    "审时（阴）：{} 未失去该牌，摸至 4 张（摸 {drawn} 张）",
                    general_name(st, mark.target)
                ),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 成略（许攸，转换技）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChenglueFrame {
    pub actor: usize,
}

pub(crate) fn step_chenglue(st: &mut SgsState, f: ChenglueFrame) -> Result<bool, EngineError> {
    let yin = skills::yj_yin(st, f.actor, "chenglue");
    let label = if yin {
        "成略（阴）：摸 2 张并弃置 1 张手牌".to_string()
    } else {
        "成略（阳）：摸 1 张并弃置 2 张手牌".to_string()
    };
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("chenglue".into()),
        options: vec![
            PendingOption {
                id: "choose:chenglue".into(),
                label,
            },
            pass_option("不发动成略"),
        ],
        data: json!({ "skill": "chenglue", "yin": yin }),
    });
    replace_top(st, Frame::Chenglue(f));
    Ok(true)
}

pub(crate) fn act_chenglue(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Chenglue(f)) => f,
        _ => return Err(internal("栈顶不是成略帧")),
    };
    if action != "choose:chenglue" {
        pop(st);
        return Ok("不发动成略".to_string());
    }
    let yin = skills::yj_yin(st, f.actor, "chenglue");
    let draw_n = if yin { 2 } else { 1 };
    let discard_n = if yin { 1 } else { 2 };
    let drawn = draw_capped(st, f.actor, draw_n);
    st.set_counter("chenglue_used", 1);
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        format!("成略：摸 {drawn} 张，需弃置 {discard_n} 张"),
    );
    if st.seats[f.actor].hand.len() < discard_n as usize {
        pop(st);
        return Ok("成略：手牌不足，未弃牌".to_string());
    }
    replace_top(
        st,
        Frame::MultiSelect(MultiSelectFrame {
            actor: f.actor,
            purpose: "chenglue".into(),
            selected: Vec::new(),
            min: discard_n,
            max: discard_n,
            data: json!({ "yin": yin }),
        }),
    );
    Ok("成略：请选择要弃置的手牌".to_string())
}

// ---------------------------------------------------------------------------
// 孙亮 · 溃诛 / 掣政 / 立军
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KuizhuStage {
    Choice,
    Targets,
    Damage,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KuizhuFrame {
    pub actor: usize,
    pub stage: KuizhuStage,
    pub chosen: Vec<usize>,
    pub x: u32,
}

pub(crate) fn step_kuizhu(st: &mut SgsState, f: KuizhuFrame) -> Result<bool, EngineError> {
    match f.stage {
        KuizhuStage::Choice => {
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("kuizhu".into()),
                options: vec![
                    PendingOption {
                        id: "choose:kuizhu:draw".into(),
                        label: format!("溃诛：令至多 {} 名角色各摸 1 张", f.x),
                    },
                    PendingOption {
                        id: "choose:kuizhu:damage".into(),
                        label: format!(
                            "溃诛：对体力值之和为 {} 的任意名角色各造成 1 点伤害",
                            f.x
                        ),
                    },
                    pass_option("不发动溃诛"),
                ],
                data: json!({ "skill": "kuizhu", "x": f.x }),
            });
            replace_top(st, Frame::Kuizhu(f));
            Ok(true)
        }
        KuizhuStage::Targets => {
            let mut options: Vec<PendingOption> = st
                .alive_indices()
                .into_iter()
                .filter(|i| !f.chosen.contains(i))
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("溃诛：令 {} 摸 1 张", general_name(st, i)),
                })
                .collect();
            if !f.chosen.is_empty() {
                options.push(PendingOption {
                    id: "confirm".into(),
                    label: format!("确认（已选 {} 名）", f.chosen.len()),
                });
            }
            options.push(pass_option("取消选择"));
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("kuizhu".into()),
                options,
                data: json!({ "stage": "kuizhu_draw", "x": f.x, "chosen": f.chosen }),
            });
            replace_top(st, Frame::Kuizhu(f));
            Ok(true)
        }
        KuizhuStage::Damage => {
            let alive = st.alive_indices();
            let mut options: Vec<PendingOption> = Vec::new();
            for mask in 1u32..(1u32 << alive.len()) {
                let mut seats = Vec::new();
                let mut sum = 0i32;
                for (bit, seat) in alive.iter().enumerate() {
                    if mask & (1 << bit) != 0 {
                        seats.push(*seat);
                        sum += st.seats[*seat].hp.max(0);
                    }
                }
                if sum == f.x as i32 && !seats.is_empty() {
                    let ids: Vec<String> = seats
                        .iter()
                        .map(|i| st.seats[*i].player_id.clone())
                        .collect();
                    let names: Vec<String> =
                        seats.iter().map(|i| general_name(st, *i)).collect();
                    options.push(PendingOption {
                        id: format!("combo:{}", ids.join(",")),
                        label: format!("溃诛：对 {} 各造成 1 点伤害", names.join("、")),
                    });
                }
            }
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            options.push(pass_option("取消"));
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("kuizhu".into()),
                options,
                data: json!({ "stage": "kuizhu_damage" }),
            });
            replace_top(st, Frame::Kuizhu(f));
            Ok(true)
        }
    }
}

pub(crate) fn act_kuizhu(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Kuizhu(f)) => f,
        _ => return Err(internal("栈顶不是溃诛帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动溃诛".to_string());
    }
    if f.stage != KuizhuStage::Choice {
        return Err(illegal("请通过目标界面继续溃诛"));
    }
    match action {
        "choose:kuizhu:draw" => {
            replace_top(
                st,
                Frame::Kuizhu(KuizhuFrame {
                    actor: f.actor,
                    stage: KuizhuStage::Targets,
                    chosen: Vec::new(),
                    x: f.x,
                }),
            );
            Ok("溃诛：请选择摸牌角色".to_string())
        }
        "choose:kuizhu:damage" => {
            replace_top(
                st,
                Frame::Kuizhu(KuizhuFrame {
                    actor: f.actor,
                    stage: KuizhuStage::Damage,
                    chosen: Vec::new(),
                    x: f.x,
                }),
            );
            Ok("溃诛：请选择受到伤害的角色组合".to_string())
        }
        _ => Err(illegal("未知溃诛选项")),
    }
}

pub(crate) fn kuizhu_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<Option<String>, EngineError> {
    let Some(Frame::Kuizhu(f)) = top(st).cloned() else {
        return Ok(None);
    };
    match f.stage {
        KuizhuStage::Choice => Ok(None),
        KuizhuStage::Targets => {
            if action == "pass" {
                pop(st);
                return Ok(Some("溃诛：取消选择".to_string()));
            }
            if action == "confirm" {
                let chosen = f.chosen.clone();
                pop(st);
                let mut total = 0;
                for seat in chosen.iter().copied() {
                    total += draw_capped(st, seat, 1);
                }
                st.push_log(
                    Some(&st.seats[f.actor].player_id.clone()),
                    format!("溃诛：令 {} 名角色各摸 1 张（共 {total} 张）", chosen.len()),
                );
                return Ok(Some("溃诛：已完成".to_string()));
            }
            if let Some(csv) = action.strip_prefix("combo:") {
                let seats: Vec<usize> = csv
                    .split(',')
                    .map(str::trim)
                    .filter_map(|pid| st.seat_index(pid))
                    .collect();
                if seats.is_empty() {
                    return Err(illegal("目标不合法"));
                }
                pop(st);
                for seat in seats.iter().copied() {
                    crate::resolve::apply_damage(
                        st,
                        seat,
                        1,
                        Some(f.actor),
                        DamageType::Normal,
                        None,
                    );
                }
                st.push_log(
                    Some(&st.seats[f.actor].player_id.clone()),
                    format!("溃诛：对 {} 名角色各造成 1 点伤害", seats.len()),
                );
                return Ok(Some("溃诛：已完成".to_string()));
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
            let mut chosen = f.chosen.clone();
            chosen.push(target);
            if chosen.len() as u32 >= f.x {
                pop(st);
                let mut total = 0;
                for seat in chosen.iter().copied() {
                    total += draw_capped(st, seat, 1);
                }
                st.push_log(
                    Some(&st.seats[f.actor].player_id.clone()),
                    format!("溃诛：令 {} 名角色各摸 1 张（共 {total} 张）", chosen.len()),
                );
                return Ok(Some("溃诛：已完成".to_string()));
            }
            replace_top(
                st,
                Frame::Kuizhu(KuizhuFrame {
                    actor: f.actor,
                    stage: KuizhuStage::Targets,
                    chosen,
                    x: f.x,
                }),
            );
            Ok(Some("溃诛：可继续选择".to_string()))
        }
        KuizhuStage::Damage => {
            if action == "pass" {
                pop(st);
                return Ok(Some("溃诛：取消".to_string()));
            }
            let csv = action
                .strip_prefix("combo:")
                .ok_or_else(|| illegal("需要选择角色组合"))?;
            if !pending
                .options
                .iter()
                .any(|o| o.id == format!("combo:{csv}"))
            {
                return Err(illegal("组合不合法"));
            }
            let seats: Vec<usize> = csv
                .split(',')
                .map(str::trim)
                .filter_map(|pid| st.seat_index(pid))
                .collect();
            pop(st);
            for seat in seats.iter().copied() {
                crate::resolve::apply_damage(
                    st,
                    seat,
                    1,
                    Some(f.actor),
                    DamageType::Normal,
                    None,
                );
            }
            st.push_log(
                Some(&st.seats[f.actor].player_id.clone()),
                format!("溃诛：对 {} 名角色各造成 1 点伤害", seats.len()),
            );
            Ok(Some("溃诛：已完成".to_string()))
        }
    }
}

/// 掣政：出牌阶段内，目标攻击范围不含孙亮的伤害被防止。
pub(crate) fn chezheng_blocks(st: &SgsState, source: usize, victim: usize) -> bool {
    if st.turn != source || st.turn_phase != TurnPhase::Play {
        return false;
    }
    if !skills::has(st, source, "chezheng") {
        return false;
    }
    !st.in_attack_range(victim, source)
}

/// 掣政：攻击范围不含你的其他存活角色。
pub(crate) fn chezheng_outsiders(st: &SgsState, actor: usize) -> Vec<usize> {
    st.alive_indices()
        .into_iter()
        .filter(|i| *i != actor && !st.in_attack_range(*i, actor))
        .collect()
}

/// 出牌阶段结束时：若本阶段使用牌数 < 这些角色数，弃置其中一名角色一张牌。
pub(crate) fn chezheng_end_phase(st: &mut SgsState, actor: usize) {
    if !skills::has(st, actor, "chezheng") || st.counter("chezheng_done") > 0 {
        return;
    }
    let outsiders = chezheng_outsiders(st, actor);
    let used = st.counter("play_cards_used") as usize;
    if used >= outsiders.len() || outsiders.is_empty() {
        return;
    }
    let has_target = outsiders.iter().any(|i| {
        !st.seats[*i].hand.is_empty() || st.seats[*i].equips.values().any(|c| c.is_some())
    });
    if !has_target {
        return;
    }
    st.set_counter("chezheng_done", 1);
    push(st, Frame::Chezheng(ChezhengFrame { actor }));
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChezhengFrame {
    pub actor: usize,
}

pub(crate) fn step_chezheng(st: &mut SgsState, f: ChezhengFrame) -> Result<bool, EngineError> {
    let options: Vec<PendingOption> = chezheng_outsiders(st, f.actor)
        .into_iter()
        .filter(|i| {
            !st.seats[*i].hand.is_empty() || st.seats[*i].equips.values().any(|c| c.is_some())
        })
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("掣政：弃置 {} 一张牌", general_name(st, i)),
        })
        .collect();
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "target".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("chezheng".into()),
        options,
        data: json!({ "stage": "chezheng" }),
    });
    replace_top(st, Frame::Chezheng(f));
    Ok(true)
}

pub(crate) fn chezheng_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Chezheng(f)) => f,
        _ => return Err(internal("栈顶不是掣政帧")),
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
    push(
        st,
        Frame::PickCard(crate::resolve::PickCardFrame {
            card: "chezheng".into(),
            user: f.actor,
            target,
            mode: crate::resolve::PickMode::Chezheng,
        }),
    );
    Ok("掣政：请选择弃置哪张牌".to_string())
}

// ---------------------------------------------------------------------------
// 立军（孙亮主公技）：其他吴势力使用【杀】后可交给孙亮，孙亮令其摸 1 且杀次数 +1
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LijunFrame {
    pub owner: usize,
    pub user: usize,
    pub card: String,
}

/// 立军触发点：其他吴势力角色（自己的出牌阶段）使用【杀】后。
pub(crate) fn lijun_trigger(st: &mut SgsState, user: usize, card: &str) {
    if st.turn != user || st.counter("lijun_used") > 0 {
        return;
    }
    if !skills::kingdom_is(st, user, "wu") {
        return;
    }
    let owner = st
        .alive_indices()
        .into_iter()
        .find(|i| *i != user && skills::has(st, *i, "lijun") && skills::is_lord(st, *i));
    let Some(owner) = owner else { return };
    push(
        st,
        Frame::Lijun(LijunFrame {
            owner,
            user,
            card: card.to_string(),
        }),
    );
}

pub(crate) fn step_lijun(st: &mut SgsState, f: LijunFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.user].player_id.clone(),
        skill: Some("lijun".into()),
        options: vec![
            PendingOption {
                id: "choose:lijun".into(),
                label: format!(
                    "立军：将此【杀】交给 {}，令其令你摸 1 张且本回合【杀】次数 +1",
                    general_name(st, f.owner)
                ),
            },
            pass_option("不发动立军"),
        ],
        data: json!({ "skill": "lijun" }),
    });
    replace_top(st, Frame::Lijun(f));
    Ok(true)
}

pub(crate) fn act_lijun(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Lijun(f)) => f,
        _ => return Err(internal("栈顶不是立军帧")),
    };
    pop(st);
    if action != "choose:lijun" {
        return Ok("不发动立军".to_string());
    }
    if !st.deck.processing.iter().any(|c| c == &f.card) {
        return Ok("立军：该【杀】已不在处理区".to_string());
    }
    crate::resolve::leave_processing(st, &f.card);
    st.seats[f.owner].hand.push(f.card.clone());
    st.set_counter("lijun_used", 1);
    st.bump_counter("lijun_sha_bonus");
    let drawn = draw_capped(st, f.user, 1);
    st.push_log(
        Some(&st.seats[f.user].player_id.clone()),
        format!(
            "立军：将【{}】交给 {}，{} 摸 {drawn} 张且本回合【杀】次数 +1",
            turn::card_label(st, &f.card),
            general_name(st, f.owner),
            general_name(st, f.user)
        ),
    );
    Ok("立军：已完成".to_string())
}
