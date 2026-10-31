//! P4.4.2 界限突破（批次 J1：8 将）技能帧。
//!
//! 覆盖：诛害/潜心/荐言（界徐庶）、势斩（界华雄）、思蜀（界刘禅）、血裔（界袁绍）、
//! 界雷击（界张角）与界乱击（界袁绍）的交互部分。
//! 同名技能 code 复用（ADR-0024），差异按 `skills::is_jiexian` 分支。

use crate::internal;
use crate::resolve::{
    apply_damage, discard_from_processing, pop, push, replace_top, top, DamageType, Frame,
    ShaFrame, ShaStage,
};
use crate::skills::has;
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

fn pass() -> PendingOption {
    PendingOption {
        id: "pass".into(),
        label: "不发动".into(),
    }
}

// ---------------------------------------------------------------------------
// 诛害（界徐庶）：其他角色的结束阶段，若其本回合造成过伤害 → 视为对其使用【杀】
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZhuHaiFrame {
    pub owner: usize,
    pub target: usize,
}

/// 结束阶段检查：是否存在可发动诛害的持有者（供 turn 阶段钩子使用）。
pub(crate) fn zhuhai_available(st: &SgsState, turn_seat: usize) -> Option<usize> {
    if turn_seat >= st.seats.len() || st.counter(&format!("dealt:{turn_seat}")) == 0 {
        return None;
    }
    st.alive_indices().into_iter().find(|i| {
        *i != turn_seat
            && has(st, *i, "zhuhai")
            && st.counter(&format!("zhuhai_asked:{i}")) == 0
            && st.in_attack_range(*i, turn_seat)
    })
}

pub(crate) fn step_zhuhai(st: &mut SgsState, f: ZhuHaiFrame) -> Result<bool, EngineError> {
    st.set_counter(&format!("zhuhai_asked:{}", f.owner), 1);
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("zhuhai".into()),
        options: vec![
            PendingOption {
                id: "choose:zhuhai".into(),
                label: format!(
                    "诛害：视为对 {} 使用一张【杀】",
                    st.seats[f.target].general_name
                ),
            },
            pass(),
        ],
        data: json!({ "skill": "zhuhai", "target": st.seats[f.target].player_id }),
    });
    replace_top(st, Frame::ZhuHai(f));
    Ok(true)
}

pub(crate) fn act_zhuhai(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ZhuHai(f)) => f,
        _ => return Err(internal("栈顶不是诛害帧")),
    };
    if action != "choose:zhuhai" {
        pop(st);
        return Ok("不发动诛害".to_string());
    }
    pop(st);
    st.push_log(
        Some(&pid(st, f.owner)),
        format!(
            "诛害：视为对 {} 使用一张【杀】",
            st.seats[f.target].general_name
        ),
    );
    push(
        st,
        Frame::Sha(ShaFrame {
            card: String::new(),
            user: f.owner,
            target: Some(f.target),
            stage: ShaStage::WaitRespond,
            pending_targets: Vec::new(),
            fangtian: false,
            damage_type: DamageType::Normal,
            no_distance: false,
            virtual_card: true,
            tianyi: false,
            no_respond: false,
        }),
    );
    crate::resolve_skill::push_sha_target_response(
        st,
        f.owner,
        f.target,
        "",
        1,
        false,
        false,
        DamageType::Normal,
    );
    Ok("诛害：【杀】结算中".to_string())
}

/// 潜心（界徐庶，觉醒）：当你造成伤害后，若你已受伤 → 减 1 点体力上限并获得「荐言」。
pub(crate) fn qianxin_on_damage(st: &mut SgsState, seat: usize) {
    if !has(st, seat, "qianxin")
        || st.seats[seat].flags.contains_key("awaken_qianxin")
        || st.seats[seat].hp >= st.seats[seat].max_hp
    {
        return;
    }
    st.seats[seat].flags.insert("awaken_qianxin".into(), true);
    st.seats[seat].max_hp = (st.seats[seat].max_hp - 1).max(1);
    st.seats[seat].hp = st.seats[seat].hp.min(st.seats[seat].max_hp);
    crate::skills::grant_skill(st, seat, "jianyan");
    st.push_log(
        Some(&pid(st, seat)),
        "潜心：觉醒，减少 1 点体力上限并获得「荐言」",
    );
}

// ---------------------------------------------------------------------------
// 荐言（界徐庶，觉醒获得）：出牌阶段限一次，声明类别或颜色 → 亮出牌堆中一张牌 → 男性角色获得
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JianYanStage {
    Mode,
    Target,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JianYanFrame {
    pub actor: usize,
    pub stage: JianYanStage,
    pub card: Option<String>,
}

fn jianyan_matches(st: &SgsState, card: &str, mode: &str) -> bool {
    let Some(inst) = st.cards.get(card) else {
        return false;
    };
    let Some(def) = crate::data::card_def(&inst.def) else {
        return false;
    };
    match mode {
        "basic" => def.category == Category::Basic,
        "trick" => matches!(def.category, Category::Trick | Category::DelayedTrick),
        "equip" => matches!(
            def.category,
            Category::Weapon | Category::Armor | Category::Horse | Category::Treasure
        ),
        "red" => crate::skills::is_red(inst.suit),
        "black" => crate::skills::is_black(inst.suit),
        _ => false,
    }
}

fn jianyan_modes(st: &SgsState) -> Vec<(&'static str, &'static str)> {
    let mut out = Vec::new();
    for (mode, label) in [
        ("basic", "基本牌"),
        ("trick", "锦囊牌"),
        ("equip", "装备牌"),
        ("red", "红色牌"),
        ("black", "黑色牌"),
    ] {
        if st.deck.draw.iter().any(|c| jianyan_matches(st, c, mode)) {
            out.push((mode, label));
        }
    }
    out
}

pub(crate) fn step_jianyan(st: &mut SgsState, f: JianYanFrame) -> Result<bool, EngineError> {
    match f.stage {
        JianYanStage::Mode => {
            let options: Vec<PendingOption> = jianyan_modes(st)
                .into_iter()
                .map(|(mode, label)| PendingOption {
                    id: format!("mode:{mode}"),
                    label: format!("荐言：声明{label}"),
                })
                .collect();
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: pid(st, f.actor),
                skill: Some("jianyan".into()),
                options,
                data: json!({ "skill": "jianyan", "stage": "mode" }),
            });
            replace_top(st, Frame::JianYan(f));
            Ok(true)
        }
        JianYanStage::Target => {
            let mut options: Vec<PendingOption> = st
                .alive_indices()
                .into_iter()
                .filter(|i| crate::skills::is_male(st, *i))
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("荐言：令 {} 获得此牌", st.seats[i].general_name),
                })
                .collect();
            options.push(pass());
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: pid(st, f.actor),
                skill: Some("jianyan".into()),
                options,
                data: json!({ "skill": "jianyan", "stage": "target" }),
            });
            replace_top(st, Frame::JianYan(f));
            Ok(true)
        }
    }
}

pub(crate) fn act_jianyan(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::JianYan(f)) => f,
        _ => return Err(internal("栈顶不是荐言帧")),
    };
    match f.stage {
        JianYanStage::Mode => {
            let mode = action
                .strip_prefix("mode:")
                .ok_or_else(|| crate::illegal("需要选择类别或颜色"))?;
            if !jianyan_modes(st).iter().any(|(m, _)| *m == mode) {
                return Err(crate::illegal("牌堆中没有该类别/颜色的牌"));
            }
            let pool: Vec<String> = st
                .deck
                .draw
                .iter()
                .filter(|c| jianyan_matches(st, c, mode))
                .cloned()
                .collect();
            let pick = st.next_index(pool.len());
            let card = pool[pick].clone();
            st.deck.draw.retain(|c| c != &card);
            st.deck.processing.push(card.clone());
            st.set_counter("jianyan_used", 1);
            st.push_log(
                Some(&pid(st, f.actor)),
                format!("荐言：亮出牌堆中一张牌 {}", turn::card_label(st, &card)),
            );
            replace_top(
                st,
                Frame::JianYan(JianYanFrame {
                    actor: f.actor,
                    stage: JianYanStage::Target,
                    card: Some(card),
                }),
            );
            Ok("荐言：请选择获得此牌的男性角色".to_string())
        }
        JianYanStage::Target => {
            let card = f.card.clone().ok_or_else(|| internal("荐言缺少亮出的牌"))?;
            if action == "pass" {
                pop(st);
                discard_from_processing(st, &card);
                st.push_log(
                    Some(&pid(st, f.actor)),
                    "荐言：没有男性角色获得此牌，置入弃牌堆",
                );
                return Ok("荐言：此牌置入弃牌堆".to_string());
            }
            let target_pid = action
                .strip_prefix("target:")
                .ok_or_else(|| crate::illegal("需要选择男性角色"))?;
            let target = st
                .seat_index(target_pid)
                .ok_or_else(|| crate::illegal("目标不在座"))?;
            if !crate::skills::is_male(st, target) {
                return Err(crate::illegal("荐言只能令男性角色获得此牌"));
            }
            st.deck.processing.retain(|c| c != &card);
            st.seats[target].hand.push(card.clone());
            let label = turn::card_label(st, &card);
            pop(st);
            st.push_log(
                Some(&pid(st, f.actor)),
                format!("荐言：令 {} 获得【{label}】", st.seats[target].general_name),
            );
            Ok("荐言：已交付".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// 势斩（界华雄）：出牌阶段限两次，令一名其他角色视为对你使用一张【决斗】
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ShiZhanFrame {
    pub actor: usize,
}

pub(crate) fn step_shizhan(st: &mut SgsState, f: ShiZhanFrame) -> Result<bool, EngineError> {
    let options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| *i != f.actor)
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("势斩：令 {} 视为对你使用【决斗】", st.seats[i].general_name),
        })
        .collect();
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "target".into(),
        actor: pid(st, f.actor),
        skill: Some("shizhan".into()),
        options,
        data: json!({ "skill": "shizhan" }),
    });
    replace_top(st, Frame::ShiZhan(f));
    Ok(true)
}

pub(crate) fn act_shizhan(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ShiZhan(f)) => f,
        _ => return Err(internal("栈顶不是势斩帧")),
    };
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| crate::illegal("需要选择一名其他角色"))?;
    let duelist = st
        .seat_index(target_pid)
        .ok_or_else(|| crate::illegal("目标不在座"))?;
    if duelist == f.actor {
        return Err(crate::illegal("势斩不能选择自己"));
    }
    pop(st);
    let used = st.counter("shizhan_used") + 1;
    st.set_counter("shizhan_used", used);
    st.push_log(
        Some(&pid(st, f.actor)),
        format!(
            "势斩：{} 视为对 {} 使用【决斗】",
            st.seats[duelist].general_name, st.seats[f.actor].general_name
        ),
    );
    // 决斗的目标（华雄）先出【杀】。
    push(
        st,
        Frame::Respond(crate::resolve::RespondFrame {
            kind: crate::resolve::RespondKind::Duel,
            actor: f.actor,
            card: "shizhan".into(),
            source: duelist,
            target: Some(duelist),
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
    Ok("势斩：请出【杀】".to_string())
}

// ---------------------------------------------------------------------------
// 思蜀（界刘禅，觉醒获得）：出牌阶段开始时，令一名角色本局【乐不思蜀】判定效果反转
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SiShuFrame {
    pub actor: usize,
}

pub(crate) fn step_sishu(st: &mut SgsState, f: SiShuFrame) -> Result<bool, EngineError> {
    st.set_counter("sishu_asked", 1);
    let mut options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!(
                "思蜀：令 {} 的【乐不思蜀】判定效果本局反转",
                st.seats[i].general_name
            ),
        })
        .collect();
    options.push(pass());
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("sishu".into()),
        options,
        data: json!({ "skill": "sishu" }),
    });
    replace_top(st, Frame::SiShu(f));
    Ok(true)
}

pub(crate) fn act_sishu(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::SiShu(f)) => f,
        _ => return Err(internal("栈顶不是思蜀帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动思蜀".to_string());
    }
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| crate::illegal("需要选择一名角色"))?;
    let target = st
        .seat_index(target_pid)
        .ok_or_else(|| crate::illegal("目标不在座"))?;
    // 覆盖前一次选择：清除旧的标记。
    let prev = st.seats[f.actor]
        .mark_counts
        .get("sishu_target")
        .copied()
        .unwrap_or(0) as usize;
    if prev < st.seats.len() {
        st.seats[prev].flags.remove("sishu_reverse");
    }
    st.seats[f.actor]
        .mark_counts
        .insert("sishu_target".into(), target as u32);
    st.seats[target].flags.insert("sishu_reverse".into(), true);
    pop(st);
    st.push_log(
        Some(&pid(st, f.actor)),
        format!(
            "思蜀：{} 的【乐不思蜀】判定效果本局反转",
            st.seats[target].general_name
        ),
    );
    Ok("思蜀：已生效".to_string())
}

// ---------------------------------------------------------------------------
// 血裔（界袁绍）：出牌阶段开始时，可移去 1 枚「裔」并摸一张牌
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct XueYiFrame {
    pub actor: usize,
}

pub(crate) fn step_xueyi(st: &mut SgsState, f: XueYiFrame) -> Result<bool, EngineError> {
    st.set_counter("xueyi_asked", 1);
    if crate::skills::xueyi_marks(st, f.actor) == 0 {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("xueyi".into()),
        options: vec![
            PendingOption {
                id: "choose:xueyi".into(),
                label: format!(
                    "血裔：移去 1 枚「裔」（剩 {}）并摸一张牌",
                    crate::skills::xueyi_marks(st, f.actor) - 1
                ),
            },
            pass(),
        ],
        data: json!({ "skill": "xueyi" }),
    });
    replace_top(st, Frame::XueYi(f));
    Ok(true)
}

pub(crate) fn act_xueyi(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::XueYi(f)) => f,
        _ => return Err(internal("栈顶不是血裔帧")),
    };
    if action != "choose:xueyi" {
        pop(st);
        return Ok("不发动血裔".to_string());
    }
    if !crate::skills::remove_xueyi_mark(st, f.actor) {
        pop(st);
        return Err(crate::illegal("没有「裔」标记"));
    }
    let drawn = draw_n(st, f.actor, 1);
    pop(st);
    st.push_log(
        Some(&pid(st, f.actor)),
        format!("血裔：移去 1 枚「裔」，摸 {drawn} 张"),
    );
    Ok("血裔：已摸一张".to_string())
}

// ---------------------------------------------------------------------------
// 界雷击（界张角）：判定后按结果选择目标
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LeijiJieFrame {
    pub owner: usize,
    pub damage: u32,
}

pub(crate) fn step_leiji_jie(st: &mut SgsState, f: LeijiJieFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!(
                "雷击：对 {} 造成 {} 点雷电伤害",
                st.seats[i].general_name, f.damage
            ),
        })
        .collect();
    options.push(pass());
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("leiji_jie".into()),
        options,
        data: json!({ "skill": "leiji_jie", "stage": "jie" }),
    });
    replace_top(st, Frame::LeijiJie(f));
    Ok(true)
}

pub(crate) fn act_leiji_jie(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::LeijiJie(f)) => f,
        _ => return Err(internal("栈顶不是界雷击帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("雷击：不造成伤害".to_string());
    }
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| crate::illegal("需要选择一名角色"))?;
    let target = st
        .seat_index(target_pid)
        .ok_or_else(|| crate::illegal("目标不在座"))?;
    pop(st);
    st.push_log(
        Some(&pid(st, f.owner)),
        format!(
            "雷击：对 {} 造成 {} 点雷电伤害",
            st.seats[target].general_name, f.damage
        ),
    );
    apply_damage(
        st,
        target,
        f.damage as i32,
        Some(f.owner),
        DamageType::Thunder,
        None,
    );
    Ok("雷击：已造成伤害".to_string())
}

// ---------------------------------------------------------------------------
// 界乱击（界袁绍）：使用【万箭齐发】可少选一个目标
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LuanJiPickFrame {
    pub actor: usize,
    pub card: String,
}

pub(crate) fn step_luanji_pick(st: &mut SgsState, f: LuanJiPickFrame) -> Result<bool, EngineError> {
    let targets = crate::resolve::trick_targets_for(st, "wanjian_qifa", f.actor, &f.card);
    if targets.len() < 2 {
        pop(st);
        crate::resolve::start_virtual_trick_targets(st, f.actor, &f.card, "wanjian_qifa", targets)?;
        return Ok(false);
    }
    let mut options: Vec<PendingOption> = targets
        .iter()
        .map(|t| PendingOption {
            id: format!("drop:{}", st.seats[*t].player_id),
            label: format!("少选一个目标：不指定 {}", st.seats[*t].general_name),
        })
        .collect();
    options.push(PendingOption {
        id: "keep".into(),
        label: "不减少目标".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("luanji_pick".into()),
        options,
        data: json!({ "skill": "luanji_pick", "stage": "pick" }),
    });
    replace_top(st, Frame::LuanJiPick(f));
    Ok(true)
}

pub(crate) fn act_luanji_pick(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::LuanJiPick(f)) => f,
        _ => return Err(internal("栈顶不是乱击帧")),
    };
    let mut targets = crate::resolve::trick_targets_for(st, "wanjian_qifa", f.actor, &f.card);
    if let Some(drop_pid) = action.strip_prefix("drop:") {
        let drop = st
            .seat_index(drop_pid)
            .ok_or_else(|| crate::illegal("目标不在座"))?;
        let before = targets.len();
        targets.retain(|t| *t != drop);
        if targets.len() == before {
            return Err(crate::illegal("该角色不在目标中"));
        }
        st.push_log(
            Some(&pid(st, f.actor)),
            format!("乱击：不指定 {}", st.seats[drop].general_name),
        );
    } else if action != "keep" {
        return Err(crate::illegal("需要选择少选的目标或保持"));
    }
    pop(st);
    crate::resolve::start_virtual_trick_targets(st, f.actor, &f.card, "wanjian_qifa", targets)?;
    Ok("乱击：请依次响应【万箭齐发】".to_string())
}

// ---------------------------------------------------------------------------
// 血裔（界袁绍，主公技）：开局获得「裔」标记
// ---------------------------------------------------------------------------

/// 开局初始化：全场每有一名群势力角色 → 界袁绍获得 2 枚「裔」。
pub(crate) fn init_xueyi(st: &mut SgsState) {
    let seats: Vec<usize> = st
        .alive_indices()
        .into_iter()
        .filter(|i| {
            has(st, *i, "xueyi")
                && crate::skills::is_lord(st, *i)
                && crate::skills::is_jiexian(st, *i)
        })
        .collect();
    for seat in seats {
        let qun = st
            .alive_indices()
            .into_iter()
            .filter(|i| crate::skills::kingdom_is(st, *i, "qun"))
            .count() as u32;
        if qun == 0 {
            continue;
        }
        crate::skills::add_xueyi_marks(st, seat, qun * 2);
        st.push_log(
            Some(&st.seats[seat].player_id.clone()),
            format!("血裔：全场 {qun} 名群势力角色，获得 {} 枚「裔」", qun * 2),
        );
    }
}
