//! 技能帧的步进与输入处理（奸雄 / 制衡）。
//!
//! 与 `resolve.rs` 的结算栈共用 `Frame` / `pending`；这里只放技能专属逻辑，
//! 避免主结算文件继续膨胀。

use crate::resolve::{
    pop, push_discard, replace_top, top, Frame, JianxiongFrame, MultiSelectFrame,
};
use crate::state::*;
use crate::turn;
use crate::{illegal, internal};
use game_engine_core::Action;
use game_engine_core::EngineError;
use serde_json::json;

// ---------------------------------------------------------------------------
// 寒冰剑 / 雌雄双股剑（P4.2）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HanbingFrame {
    pub user: usize,
    pub target: usize,
    pub card: String,
    /// 被防止的伤害值（含裸衣等加成）。
    pub damage: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CixiongFrame {
    pub user: usize,
    pub target: usize,
    pub card: String,
    pub need: u32,
    pub no_shan: bool,
    pub ignore_armor: bool,
}

/// TargetChosen 装备效果（P4.5：雌雄双股剑；异性目标可用）。
pub struct TargetChosenHandler {
    pub code: &'static str,
    pub available: fn(&SgsState, usize, usize) -> bool,
}

pub static TARGET_CHOSEN_HANDLERS: &[TargetChosenHandler] = &[TargetChosenHandler {
    code: "cixiong_shuanggujian",
    available: |st, user, target| {
        crate::skills::is_male(st, user) != crate::skills::is_male(st, target)
    },
}];

fn target_chosen_available(st: &SgsState, user: usize, target: usize) -> bool {
    let equipped: Vec<&'static str> =
        crate::skills::triggered(st, crate::skills::Trigger::TargetChosen)
            .into_iter()
            .filter(|(s, _)| *s == user)
            .map(|(_, d)| d.code)
            .collect();
    TARGET_CHOSEN_HANDLERS
        .iter()
        .any(|h| equipped.contains(&h.code) && (h.available)(st, user, target))
}

/// 给【杀】的目标压入【闪】响应帧（统一入口；雌雄双股剑在此插入选择）。
pub(crate) fn push_sha_target_response(
    st: &mut SgsState,
    user: usize,
    victim: usize,
    card: &str,
    need: u32,
    no_shan: bool,
    ignore_armor: bool,
    damage_type: crate::resolve::DamageType,
) {
    // 铁骑（judge_before）：判定后决定是否封锁【闪】。
    if crate::skills::has(st, user, "tieqi") {
        crate::resolve::push(
            st,
            Frame::QuickJudge(crate::resolve::QuickJudgeFrame {
                owner: user,
                reason: "tieqi".into(),
                stage: crate::resolve::QuickJudgeStage::Reveal,
                judge_card: None,
                rewrite_order: Vec::new(),
                rewrite_cursor: 0,
                data: json!({
                    "user": user,
                    "target": victim,
                    "card": card,
                    "need": need,
                    "ignore_armor": ignore_armor,
                }),
                saved_outcome: None,
            }),
        );
        return;
    }
    if !target_chosen_available(st, user, victim) {
        push_sha_respond_with(
            st,
            user,
            victim,
            card,
            need,
            no_shan,
            ignore_armor,
            damage_type,
        );
        return;
    }
    crate::resolve::push(
        st,
        Frame::Cixiong(CixiongFrame {
            user,
            target: victim,
            card: card.to_string(),
            need,
            no_shan,
            ignore_armor,
        }),
    );
}

/// 直接压入【杀】的【闪】响应帧。
pub(crate) fn push_sha_respond(
    st: &mut SgsState,
    user: usize,
    victim: usize,
    card: &str,
    need: u32,
    no_shan: bool,
    ignore_armor: bool,
) {
    push_sha_respond_with(
        st,
        user,
        victim,
        card,
        need,
        no_shan,
        ignore_armor,
        crate::resolve::DamageType::Normal,
    );
}

pub(crate) fn push_sha_respond_with(
    st: &mut SgsState,
    user: usize,
    victim: usize,
    card: &str,
    need: u32,
    no_shan: bool,
    ignore_armor: bool,
    damage_type: crate::resolve::DamageType,
) {
    crate::resolve::push(
        st,
        crate::resolve::Frame::Respond(crate::resolve::RespondFrame {
            kind: crate::resolve::RespondKind::Shan,
            actor: victim,
            card: card.to_string(),
            source: user,
            target: Some(victim),
            damage: 1,
            damage_type,
            consume_card: false,
            ignore_armor,
            bagua_used: false,
            need,
            played: 0,
            no_shan,
            duel_initiator: None,
            is_sha: true,
        }),
    );
}

pub(crate) fn step_cixiong(st: &mut SgsState, f: CixiongFrame) -> Result<bool, EngineError> {
    let mut options = Vec::new();
    if !st.seats[f.target].hand.is_empty() {
        options.push(PendingOption {
            id: "choose:discard".into(),
            label: "弃置一张手牌".into(),
        });
    }
    options.push(PendingOption {
        id: "choose:draw".into(),
        label: format!("令 {} 摸一张牌", st.seats[f.user].general_name),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.target].player_id.clone(),
        skill: Some("cixiong".into()),
        options,
        data: json!({
            "skill": "cixiong",
            "user": st.seats[f.user].player_id,
            "option_sources": { "choose:discard": "cixiong_shuanggujian", "choose:draw": "cixiong_shuanggujian" },
        }),
    });
    replace_top(st, Frame::Cixiong(f));
    Ok(true)
}

pub(crate) fn act_cixiong(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Cixiong(f)) => f,
        _ => return Err(internal("栈顶不是雌雄双股剑帧")),
    };
    if action == "choose:discard" {
        pop(st);
        crate::resolve::push(
            st,
            Frame::MultiSelect(MultiSelectFrame {
                actor: f.target,
                purpose: "cixiong".into(),
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
                }),
            }),
        );
        return Ok("请选择弃置的手牌".to_string());
    }
    if action != "choose:draw" {
        return Err(illegal("未知雌雄双股剑选项"));
    }
    pop(st);
    let mut ev = Vec::new();
    let drawn = turn::draw_cards(st, f.user, 1, &mut ev);
    let pid = st.seats[f.user].player_id.clone();
    st.push_log(Some(&pid), format!("雌雄双股剑：摸 {drawn} 张"));
    let over = st.seats[f.user]
        .hand
        .len()
        .saturating_sub(st.config.max_hand_cards as usize);
    if over > 0 {
        crate::resolve::push_discard(st, f.user, over, "hand_cap");
    }
    push_sha_respond(
        st,
        f.user,
        f.target,
        &f.card,
        f.need,
        f.no_shan,
        f.ignore_armor,
    );
    Ok("雌雄双股剑：攻击方摸 1 张".to_string())
}

pub(crate) fn step_hanbing(st: &mut SgsState, f: HanbingFrame) -> Result<bool, EngineError> {
    let Some(handler) = damage_before_handler(st, f.user, f.target) else {
        // handler 不可用：直接按正常伤害结算。
        pop(st);
        crate::resolve::apply_damage(
            st,
            f.target,
            f.damage,
            Some(f.user),
            crate::resolve::DamageType::Normal,
            Some(&f.card),
        );
        return Ok(false);
    };
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.user].player_id.clone(),
        skill: Some("hanbing".into()),
        options: vec![
            PendingOption {
                id: "choose:hanbing".into(),
                label: handler.label.into(),
            },
            PendingOption {
                id: "pass".into(),
                label: "正常造成伤害".into(),
            },
        ],
        data: json!({
            "skill": "hanbing",
            "target": f.target,
            "option_sources": { "choose:hanbing": handler.code },
        }),
    });
    replace_top(st, Frame::Hanbing(f));
    Ok(true)
}

/// 目标可被寒冰剑弃置的牌数（手牌 + 装备；判定区不计）。
// ---------------------------------------------------------------------------
// PlayPhase 装备效果注册表（P4.5 收尾：丈八蛇矛）
// ---------------------------------------------------------------------------

/// PlayPhase 装备效果处理器：出牌阶段主动选项（生成 + 执行）。
pub struct PlayPhaseHandler {
    pub code: &'static str,
    /// 生成出牌阶段选项（`targets` 为当前可指定的【杀】目标）。
    pub options: fn(&SgsState, usize, &[usize]) -> Vec<Action>,
    /// 执行选项；`Some(msg)` 表示已处理。
    pub act: fn(&mut SgsState, usize, &str) -> Result<Option<String>, EngineError>,
}

pub static PLAY_PHASE_HANDLERS: &[PlayPhaseHandler] = &[
    PlayPhaseHandler {
        code: "zhangba_shemao",
        options: zhangba_options,
        act: zhangba_act,
    },
    PlayPhaseHandler {
        code: "zhuque_yushan",
        options: zhuque_options,
        act: zhuque_act,
    },
];

/// 朱雀羽扇：将普通【杀】当火杀使用。
fn zhuque_options(st: &SgsState, seat: usize, targets: &[usize]) -> Vec<Action> {
    if st.counter("sha_used") >= turn::sha_limit(st, seat) || targets.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for id in turn::hand_ids(st, seat) {
        if turn::card_def(st, &id).is_some_and(|d| d.code == "sha") {
            out.push(Action {
                id: format!("skill:zhuque:{id}"),
                description: format!("朱雀羽扇：将【{}】当火杀使用", turn::card_label(st, &id)),
            });
        }
    }
    out
}

fn zhuque_act(st: &mut SgsState, seat: usize, action: &str) -> Result<Option<String>, EngineError> {
    let Some(card_id) = action.strip_prefix("skill:zhuque:") else {
        return Ok(None);
    };
    if !st.seats[seat].hand.iter().any(|c| c == card_id) {
        return Err(illegal("手牌中没有该牌"));
    }
    if turn::card_def(st, card_id).map(|d| d.code) != Some("sha") {
        return Err(illegal("朱雀羽扇只能转化普通【杀】"));
    }
    if st.counter("sha_used") >= turn::sha_limit(st, seat) {
        return Err(illegal("本出牌阶段已使用过【杀】"));
    }
    if turn::sha_targets(st, seat).is_empty() {
        return Err(illegal("攻击范围内没有目标"));
    }
    let fangtian = crate::skills::has_equipment_skill(st, seat, "fangtian_huaji")
        && st.seats[seat].hand.len() == 1;
    let label = turn::card_label(st, card_id);
    turn::take_card(st, seat, card_id)?;
    crate::resolve::start_sha_with(
        st,
        seat,
        card_id,
        fangtian,
        crate::resolve::DamageType::Fire,
    )?;
    let pid = st.seats[seat].player_id.clone();
    st.push_log(Some(&pid), format!("朱雀羽扇：将【{label}】当火杀使用"));
    Ok(Some("请选择【杀】的目标".to_string()))
}

fn zhangba_options(st: &SgsState, seat: usize, targets: &[usize]) -> Vec<Action> {
    if st.seats[seat].hand.len() < 2
        || st.counter("sha_used") >= turn::sha_limit(st, seat)
        || targets.is_empty()
    {
        return Vec::new();
    }
    let hand = turn::hand_ids(st, seat);
    let mut out = Vec::new();
    for i in 0..hand.len() {
        for j in (i + 1)..hand.len() {
            out.push(Action {
                id: format!("skill:zhangba:{},{}", hand[i], hand[j]),
                description: format!(
                    "丈八蛇矛：将【{}】【{}】当【杀】使用",
                    turn::card_label(st, &hand[i]),
                    turn::card_label(st, &hand[j])
                ),
            });
        }
    }
    out
}

fn zhangba_act(
    st: &mut SgsState,
    seat: usize,
    action: &str,
) -> Result<Option<String>, EngineError> {
    let Some(pair) = action.strip_prefix("skill:zhangba:") else {
        return Ok(None);
    };
    let mut it = pair.split(',');
    let (Some(c1), Some(c2), None) = (it.next(), it.next(), it.next()) else {
        return Err(illegal("丈八蛇矛需要两张手牌"));
    };
    if c1 == c2 {
        return Err(illegal("丈八蛇矛需要两张不同的手牌"));
    }
    for c in [c1, c2] {
        if !st.seats[seat].hand.iter().any(|h| h == c) {
            return Err(illegal("手牌中没有该牌"));
        }
    }
    if st.counter("sha_used") >= turn::sha_limit(st, seat) {
        return Err(illegal("本出牌阶段已使用过【杀】"));
    }
    if turn::sha_targets(st, seat).is_empty() {
        return Err(illegal("攻击范围内没有目标"));
    }
    let label1 = turn::card_label(st, c1);
    let label2 = turn::card_label(st, c2);
    turn::take_card(st, seat, c1)?;
    turn::take_card(st, seat, c2)?;
    turn::to_discard_card(st, c2);
    crate::resolve::start_sha(st, seat, c1, false)?;
    let pid = st.seats[seat].player_id.clone();
    st.push_log(
        Some(&pid),
        format!("丈八蛇矛：将【{label1}】【{label2}】当【杀】使用"),
    );
    Ok(Some("请选择【杀】的目标".to_string()))
}

/// 出牌阶段装备选项（按 `triggered(PlayPhase)` 收集）。
pub(crate) fn play_phase_options(st: &SgsState, seat: usize, targets: &[usize]) -> Vec<Action> {
    let equipped: Vec<&'static str> =
        crate::skills::triggered(st, crate::skills::Trigger::PlayPhase)
            .into_iter()
            .filter(|(s, _)| *s == seat)
            .map(|(_, d)| d.code)
            .collect();
    let mut out = Vec::new();
    for handler in PLAY_PHASE_HANDLERS {
        if equipped.contains(&handler.code) {
            out.extend((handler.options)(st, seat, targets));
        }
    }
    out
}

/// 执行出牌阶段装备选项（返回 `Some` 表示已处理）。
pub(crate) fn play_phase_act(
    st: &mut SgsState,
    seat: usize,
    action: &str,
) -> Result<Option<String>, EngineError> {
    let equipped: Vec<&'static str> =
        crate::skills::triggered(st, crate::skills::Trigger::PlayPhase)
            .into_iter()
            .filter(|(s, _)| *s == seat)
            .map(|(_, d)| d.code)
            .collect();
    for handler in PLAY_PHASE_HANDLERS {
        if !equipped.contains(&handler.code) {
            continue;
        }
        if let Some(msg) = (handler.act)(st, seat, action)? {
            return Ok(Some(msg));
        }
    }
    Ok(None)
}

/// DamageBefore 装备效果处理器（P4.5：寒冰剑）。
pub struct DamageBeforeHandler {
    pub code: &'static str,
    pub label: &'static str,
    /// 目标可弃牌数 ≥2 时可用。
    pub available: fn(&SgsState, usize) -> bool,
    /// 执行效果（防止伤害 + 弃置目标两张牌）。
    pub effect: fn(&mut SgsState, &HanbingFrame),
}

pub static DAMAGE_BEFORE_HANDLERS: &[DamageBeforeHandler] = &[DamageBeforeHandler {
    code: "hanbing_jian",
    label: "寒冰剑：防止此伤害，改为弃置目标两张牌",
    available: |st, target| hanbing_available(st, target) >= 2,
    effect: |st, f| {
        let mut discarded = Vec::new();
        for _ in 0..2 {
            if let Some(id) = discard_one_from(st, f.target) {
                turn::to_discard_card(st, &id);
                discarded.push(turn::card_label(st, &id));
            }
        }
        let pid = st.seats[f.user].player_id.clone();
        let target_name = st.seats[f.target].general_name.clone();
        st.push_log(
            Some(&pid),
            format!(
                "寒冰剑：防止伤害，弃置 {target_name} 的【{}】",
                discarded.join("】【")
            ),
        );
    },
}];

fn damage_before_handler(
    st: &SgsState,
    user: usize,
    target: usize,
) -> Option<&'static DamageBeforeHandler> {
    let equipped: Vec<&'static str> =
        crate::skills::triggered(st, crate::skills::Trigger::DamageBefore)
            .into_iter()
            .filter(|(s, _)| *s == user)
            .map(|(_, d)| d.code)
            .collect();
    DAMAGE_BEFORE_HANDLERS
        .iter()
        .find(|h| equipped.contains(&h.code) && (h.available)(st, target))
}

fn hanbing_available(st: &SgsState, seat: usize) -> usize {
    st.seats[seat].hand.len() + st.seats[seat].equips.values().flatten().count()
}

fn discard_one_from(st: &mut SgsState, seat: usize) -> Option<String> {
    if !st.seats[seat].hand.is_empty() {
        let n = st.seats[seat].hand.len();
        let idx = st.next_index(n);
        return Some(st.seats[seat].hand.remove(idx));
    }
    for slot in EquipSlot::ALL {
        if let Some(Some(id)) = st.seats[seat].equips.get(&slot).cloned() {
            st.seats[seat].equips.insert(slot, None);
            return Some(id);
        }
    }
    None
}

pub(crate) fn act_hanbing(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Hanbing(f)) => f,
        _ => return Err(internal("栈顶不是寒冰剑帧")),
    };
    pop(st);
    if action != "choose:hanbing" {
        crate::resolve::apply_damage(
            st,
            f.target,
            f.damage,
            Some(f.user),
            crate::resolve::DamageType::Normal,
            Some(&f.card),
        );
        return Ok("寒冰剑：正常造成伤害".to_string());
    }
    let Some(handler) = damage_before_handler(st, f.user, f.target) else {
        crate::resolve::apply_damage(
            st,
            f.target,
            f.damage,
            Some(f.user),
            crate::resolve::DamageType::Normal,
            Some(&f.card),
        );
        return Ok("正常造成伤害".to_string());
    };
    (handler.effect)(st, &f);
    Ok("寒冰剑：已防止伤害并弃置两张牌".to_string())
}

/// 寒冰剑是否可发动（武器 + 目标至少两张可弃牌）。
pub(crate) fn hanbing_available_for(st: &SgsState, user: usize, target: usize) -> bool {
    crate::skills::has_equipment_skill(st, user, "hanbing_jian")
        && hanbing_available(st, target) >= 2
}

// ---------------------------------------------------------------------------
// 青龙偃月刀 / 贯石斧（P4.2：被【闪】抵消后的追加窗口）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AfterDodgeFrame {
    /// 【杀】的使用者（武器持有者）。
    pub user: usize,
    /// 【杀】的目标。
    pub target: usize,
    /// 【杀】的牌实例。
    pub card: String,
}

/// AfterDodge 装备效果处理器（P4.5 阶段三：效果函数注册表）。
pub struct AfterDodgeHandler {
    /// 装备技能 code（与 `EQUIPMENT_SKILLS.code` 一致）。
    pub code: &'static str,
    /// 生成该装备在窗口内的选项（空 = 当前不可用）。
    pub options: fn(&SgsState, &AfterDodgeFrame) -> Vec<PendingOption>,
    /// 执行选项；`Some(msg)` 表示已处理。
    pub act: fn(&mut SgsState, &AfterDodgeFrame, &str) -> Result<Option<String>, EngineError>,
}

pub static AFTER_DODGE_HANDLERS: &[AfterDodgeHandler] = &[
    AfterDodgeHandler {
        code: "qinglong_yanyue",
        options: qinglong_options,
        act: qinglong_act,
    },
    AfterDodgeHandler {
        code: "guanshi_fu",
        options: guanshi_options,
        act: guanshi_act,
    },
];

fn qinglong_options(st: &SgsState, f: &AfterDodgeFrame) -> Vec<PendingOption> {
    let mut out = Vec::new();
    for id in turn::hand_ids(st, f.user) {
        if turn::card_def(st, &id).is_some_and(|d| d.code == "sha") {
            out.push(PendingOption {
                id: format!("choose:{id}"),
                label: format!("青龙偃月刀：再使用【{}】", turn::card_label(st, &id)),
            });
        }
    }
    out
}

fn qinglong_act(
    st: &mut SgsState,
    f: &AfterDodgeFrame,
    action: &str,
) -> Result<Option<String>, EngineError> {
    let Some(card_id) = action.strip_prefix("choose:") else {
        return Ok(None);
    };
    if card_id == "guanshi" {
        return Ok(None);
    }
    if !st.seats[f.user].hand.iter().any(|c| c == card_id) {
        return Err(illegal("手牌中没有该牌"));
    }
    if turn::card_def(st, card_id).map(|d| d.code) != Some("sha") {
        return Ok(None);
    }
    pop(st);
    // 原【杀】已被【闪】抵消：先结算并出栈（避免与新【杀】共用 outcome）。
    st.outcome = None;
    if matches!(top(st), Some(Frame::Sha(_))) {
        pop(st);
    }
    crate::resolve::discard_from_processing(st, &f.card);
    turn::take_card(st, f.user, card_id)?;
    st.deck.processing.push(card_id.to_string());
    let pid = st.seats[f.user].player_id.clone();
    st.push_log(
        Some(&pid),
        format!(
            "青龙偃月刀：对被闪目标再使用【{}】",
            turn::card_label(st, card_id)
        ),
    );
    crate::resolve::push(
        st,
        crate::resolve::Frame::Sha(crate::resolve::ShaFrame {
            card: card_id.to_string(),
            user: f.user,
            target: Some(f.target),
            stage: crate::resolve::ShaStage::WaitRespond,
            pending_targets: Vec::new(),
            fangtian: false,
            damage_type: crate::resolve::DamageType::Normal,
            no_distance: false,
            no_respond: false,
            virtual_card: false,
            tianyi: false,
        }),
    );
    let need = if crate::skills::has(st, f.user, "wushuang") {
        2
    } else {
        1
    };
    push_sha_target_response(
        st,
        f.user,
        f.target,
        card_id,
        need,
        false,
        crate::skills::has_equipment_skill(st, f.user, "qinggang_jian"),
        crate::resolve::DamageType::Normal,
    );
    Ok(Some("青龙偃月刀：等待目标再次响应".to_string()))
}

fn guanshi_options(st: &SgsState, f: &AfterDodgeFrame) -> Vec<PendingOption> {
    if st.seats[f.user].hand.len() < 2 {
        return Vec::new();
    }
    vec![PendingOption {
        id: "choose:guanshi".into(),
        label: "贯石斧：弃置两张手牌，令此【杀】依然造成伤害".into(),
    }]
}

fn guanshi_act(
    st: &mut SgsState,
    f: &AfterDodgeFrame,
    action: &str,
) -> Result<Option<String>, EngineError> {
    if action != "choose:guanshi" {
        return Ok(None);
    }
    if st.seats[f.user].hand.len() < 2 {
        return Err(illegal("手牌不足 2 张"));
    }
    pop(st);
    crate::resolve::push(
        st,
        Frame::MultiSelect(crate::resolve::MultiSelectFrame {
            actor: f.user,
            purpose: "guanshi".into(),
            selected: Vec::new(),
            min: 2,
            max: 2,
            data: json!({ "target": f.target, "card": f.card }),
        }),
    );
    Ok(Some("请选择弃置的 2 张手牌".to_string()))
}

/// 当前座位已装备且可在此窗口生效的 AfterDodge 处理器（注册序）。
fn after_dodge_handlers(st: &SgsState, seat: usize) -> Vec<&'static AfterDodgeHandler> {
    let equipped: Vec<&'static str> =
        crate::skills::triggered(st, crate::skills::Trigger::AfterDodge)
            .into_iter()
            .filter(|(s, _)| *s == seat)
            .map(|(_, d)| d.code)
            .collect();
    AFTER_DODGE_HANDLERS
        .iter()
        .filter(|h| equipped.contains(&h.code))
        .collect()
}

/// 【闪】成功抵消后，若攻击方装备青龙偃月刀 / 贯石斧，压入追加窗口。
pub(crate) fn push_after_dodge_if_available(
    st: &mut SgsState,
    f: &crate::resolve::RespondFrame,
) -> bool {
    if f.kind != crate::resolve::RespondKind::Shan || !f.is_sha || f.consume_card {
        return false;
    }
    let frame = AfterDodgeFrame {
        user: f.source,
        target: f.actor,
        card: f.card.clone(),
    };
    let available = after_dodge_handlers(st, frame.user)
        .iter()
        .any(|h| !(h.options)(st, &frame).is_empty());
    if !available {
        return false;
    }
    crate::resolve::push(st, crate::resolve::Frame::AfterDodge(frame));
    true
}

pub(crate) fn step_after_dodge(st: &mut SgsState, f: AfterDodgeFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = Vec::new();
    for handler in after_dodge_handlers(st, f.user) {
        options.extend((handler.options)(st, &f));
    }
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    options.push(PendingOption {
        id: "pass".into(),
        label: "放弃".into(),
    });
    // 标注每个选项由哪件装备产生（前端据此高亮武器）。
    let mut sources = serde_json::Map::new();
    for handler in after_dodge_handlers(st, f.user) {
        for opt in (handler.options)(st, &f) {
            sources.insert(opt.id, json!(handler.code));
        }
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.user].player_id.clone(),
        skill: Some("after_dodge".into()),
        options,
        data: json!({ "skill": "after_dodge", "target": f.target, "card": f.card, "option_sources": sources }),
    });
    replace_top(st, Frame::AfterDodge(f));
    Ok(true)
}

pub(crate) fn act_after_dodge(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::AfterDodge(f)) => f,
        _ => return Err(internal("栈顶不是装备追加帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("放弃武器追加效果".to_string());
    }
    let handlers = after_dodge_handlers(st, f.user);
    for handler in handlers {
        if let Some(msg) = (handler.act)(st, &f, action)? {
            return Ok(msg);
        }
    }
    Err(illegal("该装备效果不可用"))
}

// ---------------------------------------------------------------------------
// 麒麟弓（P3 追加）：杀造成伤害后，攻击方可弃置目标一张坐骑（进攻/防御马）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QilinFrame {
    pub attacker: usize,
    pub target: usize,
}

/// 目标当前可被弃置的坐骑槽（进攻马在前、防御马在后）。
pub(crate) fn qilin_horses(st: &SgsState, target: usize) -> Vec<EquipSlot> {
    [EquipSlot::HorseOffense, EquipSlot::HorseDefense]
        .into_iter()
        .filter(|slot| {
            st.seats[target]
                .equips
                .get(slot)
                .is_some_and(|v| v.is_some())
        })
        .collect()
}

fn horse_slot_code(slot: EquipSlot) -> &'static str {
    match slot {
        EquipSlot::HorseOffense => "horse_offense",
        EquipSlot::HorseDefense => "horse_defense",
        _ => "horse_offense",
    }
}

fn horse_slot_from_code(code: &str) -> Option<EquipSlot> {
    match code {
        "horse_offense" => Some(EquipSlot::HorseOffense),
        "horse_defense" => Some(EquipSlot::HorseDefense),
        _ => None,
    }
}

pub(crate) fn step_qilin(st: &mut SgsState, f: QilinFrame) -> Result<bool, EngineError> {
    let horses = qilin_horses(st, f.target);
    if horses.is_empty() || !st.seats[f.target].alive || !st.seats[f.attacker].alive {
        pop(st);
        return Ok(false);
    }
    let target_name = st.seats[f.target].general_name.clone();
    let mut options: Vec<PendingOption> = horses
        .iter()
        .map(|slot| {
            let id = st.seats[f.target].equips.get(slot).and_then(|v| v.clone());
            let label = match (&id, slot) {
                (Some(card), EquipSlot::HorseOffense) => {
                    format!(
                        "弃置{}的【{}】（进攻马）",
                        target_name,
                        turn::card_label(st, card)
                    )
                }
                (Some(card), _) => {
                    format!(
                        "弃置{}的【{}】（防御马）",
                        target_name,
                        turn::card_label(st, card)
                    )
                }
                (None, _) => String::new(),
            };
            PendingOption {
                id: format!("skill:qilin:{}", horse_slot_code(*slot)),
                label,
            }
        })
        .collect();
    options.push(PendingOption {
        id: "pass".into(),
        label: "不弃置".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.attacker].player_id.clone(),
        skill: Some("qilin".into()),
        options,
        data: json!({ "skill": "qilin", "target": st.seats[f.target].player_id }),
    });
    replace_top(st, Frame::Qilin(f));
    Ok(true)
}

pub(crate) fn act_qilin(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Qilin(f)) => f,
        _ => return Err(internal("栈顶不是麒麟弓帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不弃置坐骑".to_string());
    }
    let code = action
        .strip_prefix("skill:qilin:")
        .ok_or_else(|| illegal("需要选择要弃置的坐骑或选择不弃置"))?;
    let slot = horse_slot_from_code(code).ok_or_else(|| illegal("未知的坐骑槽"))?;
    let card = st.seats[f.target]
        .equips
        .get(&slot)
        .cloned()
        .flatten()
        .ok_or_else(|| illegal("该槽位没有坐骑"))?;
    st.seats[f.target].equips.insert(slot, None);
    turn::to_discard_card(st, &card);
    // 枭姬：失去装备区的牌后摸 2 张。
    crate::skills::xiaoji_on_lose_equip(st, f.target);
    let label = turn::card_label(st, &card);
    st.push_log(
        Some(&pending.actor),
        format!(
            "麒麟弓：弃置{}的【{}】",
            st.seats[f.target].general_name, label
        ),
    );
    pop(st);
    Ok(format!("麒麟弓：已弃置【{label}】"))
}

// ---------------------------------------------------------------------------
// 奸雄
// ---------------------------------------------------------------------------

pub(crate) fn step_jianxiong(st: &mut SgsState, f: JianxiongFrame) -> Result<bool, EngineError> {
    let name = turn::card_label(st, &f.card);
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("jianxiong".into()),
        options: vec![
            PendingOption {
                id: "choose:gain".into(),
                label: format!("获得【{name}】"),
            },
            PendingOption {
                id: "pass".into(),
                label: "不获得".into(),
            },
        ],
        data: json!({ "skill": "jianxiong", "card": f.card }),
    });
    replace_top(st, Frame::Jianxiong(f));
    Ok(true)
}

pub(crate) fn act_jianxiong(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Jianxiong(f)) => f,
        _ => return Err(internal("栈顶不是奸雄帧")),
    };
    let name = turn::card_label(st, &f.card);
    if action == "choose:gain" {
        if st.deck.processing.iter().any(|c| c == &f.card) {
            st.deck.processing.retain(|c| c != &f.card);
            st.seats[f.actor].hand.push(f.card.clone());
            st.push_log(
                Some(&st.seats[f.actor].player_id.clone()),
                format!("奸雄：获得【{name}】"),
            );
            let over = st.seats[f.actor]
                .hand
                .len()
                .saturating_sub(st.config.max_hand_cards as usize);
            push_discard(st, f.actor, over, "hand_cap");
        }
    }
    pop(st);
    Ok(format!("奸雄：{name}"))
}

// ---------------------------------------------------------------------------
// 多选（制衡等）
// ---------------------------------------------------------------------------

pub(crate) fn step_multiselect(
    st: &mut SgsState,
    f: MultiSelectFrame,
) -> Result<bool, EngineError> {
    let allowed: Option<Vec<String>> = f.data.get("allowed").and_then(|v| v.as_array()).map(|a| {
        a.iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect()
    });
    let mut options: Vec<PendingOption> = Vec::new();
    for id in turn::hand_ids(st, f.actor) {
        if allowed.as_ref().is_some_and(|a| !a.contains(&id)) {
            continue;
        }
        let mark = if f.selected.contains(&id) { "✔ " } else { "" };
        options.push(PendingOption {
            id: format!("choose:{id}"),
            label: format!("{mark}{}", turn::card_label(st, &id)),
        });
    }
    if f.selected.len() >= f.min && !f.selected.is_empty() {
        options.push(PendingOption {
            id: "confirm".into(),
            label: format!("确认（已选 {} 张）", f.selected.len()),
        });
    }
    if f.min == 0 {
        options.push(PendingOption {
            id: "cancel".into(),
            label: "取消".into(),
        });
    }
    st.pending = Some(Pending {
        kind: "choose_many".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some(f.purpose.clone()),
        options,
        data: json!({
            "purpose": f.purpose,
            "selected": f.selected,
            "min": f.min,
            "max": f.max,
        }),
    });
    replace_top(st, Frame::MultiSelect(f));
    Ok(true)
}

pub(crate) fn act_multiselect(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let mut f = match top(st).cloned() {
        Some(Frame::MultiSelect(f)) => f,
        _ => return Err(internal("栈顶不是多选帧")),
    };
    if action == "cancel" {
        if f.min > 0 {
            return Err(illegal("至少需要选择 1 张"));
        }
        pop(st);
        return Ok("已取消".to_string());
    }
    if action == "confirm" {
        if f.selected.len() < f.min {
            return Err(illegal("选择数量不足"));
        }
        if f.selected.len() > f.max {
            return Err(illegal("选择数量过多"));
        }
        if let Some(result) = crate::resolve_more::multiselect_confirm(st, &f) {
            return result;
        }
        match f.purpose.as_str() {
            "zhiheng" => {
                let n = f.selected.len();
                for id in &f.selected {
                    turn::take_card(st, f.actor, id)?;
                    turn::to_discard_card(st, id);
                }
                st.set_counter("zhiheng_used", 1);
                let mut events = Vec::new();
                let drawn = turn::draw_cards(st, f.actor, n as u32, &mut events);
                st.push_log(
                    Some(&st.seats[f.actor].player_id.clone()),
                    format!("制衡：弃置 {n} 张，摸 {drawn} 张"),
                );
                pop(st);
                Ok(format!("制衡：弃 {n} 摸 {drawn}"))
            }
            other => Err(internal(format!("未实现的多选用途：{other}"))),
        }
    } else {
        let card_id = action
            .strip_prefix("choose:")
            .ok_or_else(|| illegal("未知选项"))?;
        if !st.seats[f.actor].hand.iter().any(|c| c == card_id) {
            return Err(illegal("手牌中没有该牌"));
        }
        if f.data
            .get("allowed")
            .and_then(|v| v.as_array())
            .is_some_and(|a| !a.iter().any(|v| v.as_str() == Some(card_id)))
        {
            return Err(illegal("该牌不在可选范围"));
        }
        if let Some(pos) = f.selected.iter().position(|c| c == card_id) {
            f.selected.remove(pos);
        } else {
            if f.selected.len() >= f.max {
                return Err(illegal("选择数量已达上限"));
            }
            f.selected.push(card_id.to_string());
        }
        replace_top(st, Frame::MultiSelect(f));
        Ok("已选择".to_string())
    }
}
