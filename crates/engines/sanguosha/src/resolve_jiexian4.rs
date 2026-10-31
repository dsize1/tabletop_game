//! P4.4.2 界限突破（批次 J3：8 将）技能帧与钩子。
//!
//! 覆盖：酣战（界太史慈）、界挑衅（伤害条件 + 限两次）、长标（界祝融）、
//! 界乱武（结算后视为使用【杀】）、好施界回礼、巧变界「变」标记。
//! 同名技能 code 复用（ADR-0024），差异按 `skills::is_jiexian` 分支。

use crate::resolve::{pop, push, DamageType, Frame, ShaFrame, ShaStage};
use crate::skills::{has, is_jiexian};
use crate::state::*;
use crate::turn;
use game_engine_core::EngineError;
use serde_json::json;

fn pid(st: &SgsState, seat: usize) -> String {
    st.seats[seat].player_id.clone()
}

// ---------------------------------------------------------------------------
// 酣战（界太史慈）：拼点前对方随机手牌；拼点后获得点数最大的【杀】
// ---------------------------------------------------------------------------

/// 拼点参与者是否可在「对方选择拼点牌」时强制随机（界太史慈·酣战）。
pub(crate) fn hanzhan_forces_random(st: &SgsState, chooser: usize, other: usize) -> bool {
    st.seats
        .get(other)
        .is_some_and(|s| s.alive && has(st, other, "hanzhan") && is_jiexian(st, other))
        && chooser != other
}

/// 拼点结束后：拥有酣战者获得两张拼点牌中点数最大的【杀】（若在弃牌堆）。
pub(crate) fn hanzhan_gain_sha(
    st: &mut SgsState,
    a_seat: usize,
    b_seat: usize,
    a_card: &str,
    b_card: &str,
) {
    for owner in [a_seat, b_seat] {
        if !(has(st, owner, "hanzhan") && is_jiexian(st, owner)) {
            continue;
        }
        let best = [a_card, b_card]
            .into_iter()
            .filter(|id| {
                st.cards
                    .get(*id)
                    .is_some_and(|c| matches!(c.def.as_str(), "sha" | "huo_sha" | "lei_sha"))
            })
            .max_by_key(|id| st.cards.get(*id).map(|c| c.rank).unwrap_or(0))
            .map(str::to_string);
        let Some(best) = best else { continue };
        if st.deck.discard.iter().any(|c| c == &best) {
            st.deck.discard.retain(|c| c != &best);
            st.seats[owner].hand.push(best.clone());
            st.push_log(
                Some(&pid(st, owner)),
                format!(
                    "酣战：获得拼点牌中点数最大的【{}】",
                    turn::card_label(st, &best)
                ),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 界挑衅（界姜维）：目标对姜维使用【杀】且造成伤害才免弃；本阶段限两次
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TiaoxinWaitFrame {
    pub owner: usize,
    pub target: usize,
    /// 结算前记录「是否已被此【杀】伤害」（伤害时打标记消耗）。
    pub hit: bool,
}

/// 【杀】造成伤害时：标记「挑衅已被此【杀】伤害」。
pub(crate) fn tiaoxin_on_damage(st: &mut SgsState, victim: usize, source: Option<usize>) {
    if !has(st, victim, "tiaoxin") || !is_jiexian(st, victim) {
        return;
    }
    if let Some(src) = source {
        if st
            .seats
            .get(victim)
            .is_some_and(|s| s.flags.contains_key(&format!("tiaoxin_src:{src}")))
        {
            st.set_counter("tiaoxin_hit", 1);
        }
    }
}

pub(crate) fn step_tiaoxin_wait(
    st: &mut SgsState,
    mut f: TiaoxinWaitFrame,
) -> Result<bool, EngineError> {
    // 由【杀】造成的伤害（apply_damage_inner 打 tiaoxin_hit 计数器）。
    if st.counter("tiaoxin_hit") > 0 {
        f.hit = true;
    }
    pop(st);
    if f.hit {
        st.push_log(
            Some(&pid(st, f.owner)),
            format!(
                "挑衅：{} 的【杀】造成了伤害，不弃其牌",
                st.seats[f.target].general_name
            ),
        );
        return Ok(false);
    }
    // 未造成伤害（或未出杀）：姜维弃置其一张牌。
    let discarded = crate::resolve_jiexian::discard_one_from(st, f.target);
    let what = discarded
        .map(|c| turn::card_label(st, &c))
        .unwrap_or_else(|| "（无牌可弃）".to_string());
    st.push_log(
        Some(&pid(st, f.owner)),
        format!(
            "挑衅：{} 未用【杀】造成伤害，弃置其{what}",
            st.seats[f.target].general_name
        ),
    );
    Ok(false)
}

/// 进入「等待【杀】结算」：压入挑衅等待帧（在【杀】帧之上）。
pub(crate) fn push_tiaoxin_wait(st: &mut SgsState, owner: usize, target: usize) {
    st.seats[owner]
        .flags
        .insert(format!("tiaoxin_src:{target}"), true);
    push(
        st,
        Frame::TiaoxinWait(TiaoxinWaitFrame {
            owner,
            target,
            hit: false,
        }),
    );
}

// ---------------------------------------------------------------------------
// 长标（界祝融）：任意张手牌当无距离【杀】；造成伤害则本阶段结束时摸等量张
// ---------------------------------------------------------------------------

/// 长标【杀】造成伤害：累计本阶段摸牌数。
pub(crate) fn changbiao_on_damage(st: &mut SgsState, source: usize, card: Option<&str>) {
    let Some(card) = card.filter(|c| !c.is_empty()) else {
        return;
    };
    let key = format!("changbiao_sha:{card}");
    if !has(st, source, "changbiao") || !st.seats[source].flags.contains_key(&key) {
        return;
    }
    st.seats[source].flags.remove(&key);
    st.bump_counter("changbiao_draw");
}

// ---------------------------------------------------------------------------
// 界乱武（界贾诩）：所有角色结算完毕后，视为使用一张无距离限制的【杀】
// ---------------------------------------------------------------------------

pub(crate) fn push_luanwu_final_sha(st: &mut SgsState, actor: usize) -> bool {
    if !has(st, actor, "luanwu") || !is_jiexian(st, actor) {
        return false;
    }
    if st.counter("luanwu_final_sha") > 0 || !st.seats[actor].alive {
        return false;
    }
    st.set_counter("luanwu_final_sha", 1);
    push(
        st,
        Frame::Sha(ShaFrame {
            card: String::new(),
            user: actor,
            target: None,
            stage: ShaStage::Target,
            pending_targets: Vec::new(),
            fangtian: false,
            damage_type: DamageType::Normal,
            no_distance: true,
            virtual_card: true,
            tianyi: false,
            no_respond: false,
        }),
    );
    true
}

// ---------------------------------------------------------------------------
// 好施界（界鲁肃）：给出牌后，成为【杀】/普通锦囊目标时其交给你一张手牌（自动）
// ---------------------------------------------------------------------------

/// 好施给出牌后，记录「回礼者」。
pub(crate) fn mark_haoshi_back(st: &mut SgsState, actor: usize, receiver: usize) {
    if !is_jiexian(st, actor) {
        return;
    }
    st.seats[actor]
        .flags
        .insert(format!("haoshi_back:{receiver}"), true);
}

/// 界鲁肃成为【杀】/普通锦囊目标时，回礼者自动交给其一张手牌。
pub(crate) fn haoshi_back_auto(st: &mut SgsState, target: usize) {
    if !has(st, target, "haoshi") || !is_jiexian(st, target) {
        return;
    }
    for helper in st.alive_indices() {
        if helper == target
            || !st.seats[target]
                .flags
                .contains_key(&format!("haoshi_back:{helper}"))
        {
            continue;
        }
        if st.seats[helper].hand.is_empty() {
            continue;
        }
        let idx = st.next_index(st.seats[helper].hand.len());
        let card = st.seats[helper].hand.remove(idx);
        let label = turn::card_label(st, &card);
        st.seats[target].hand.push(card);
        st.push_log(
            Some(&pid(st, helper)),
            format!(
                "好施：交给 {} 一张手牌（{label}）",
                st.seats[target].general_name
            ),
        );
    }
}

// ---------------------------------------------------------------------------
// 巧变界（界张郃）：「变」标记（开局 2 枚；结束阶段手牌数均不相同则 +1）
// ---------------------------------------------------------------------------

/// 巧变界：「变」标记数量。
pub fn bian_marks(st: &SgsState, seat: usize) -> u32 {
    st.seats
        .get(seat)
        .and_then(|s| s.mark_counts.get("bian"))
        .copied()
        .unwrap_or(0)
}

pub(crate) fn add_bian(st: &mut SgsState, seat: usize, n: u32) {
    if let Some(s) = st.seats.get_mut(seat) {
        *s.mark_counts.entry("bian".into()).or_insert(0) += n;
    }
}

fn remove_bian(st: &mut SgsState, seat: usize) -> bool {
    let Some(s) = st.seats.get_mut(seat) else {
        return false;
    };
    let cur = s.mark_counts.get("bian").copied().unwrap_or(0);
    if cur == 0 {
        return false;
    }
    if cur == 1 {
        s.mark_counts.remove("bian");
    } else {
        s.mark_counts.insert("bian".into(), cur - 1);
    }
    true
}

/// 开局初始化：界张郃获得 2 枚「变」。
pub(crate) fn init_qiaobian(st: &mut SgsState) {
    let seats: Vec<usize> = st
        .alive_indices()
        .into_iter()
        .filter(|i| has(st, *i, "qiaobian") && is_jiexian(st, *i))
        .collect();
    for seat in seats {
        add_bian(st, seat, 2);
        st.push_log(Some(&pid(st, seat)), "巧变：开局获得 2 枚「变」");
    }
}

/// 结束阶段：手牌数与以往每回合结束阶段的手牌数均不相等 → +1「变」。
pub(crate) fn qiaobian_end_check(st: &mut SgsState, seat: usize) {
    if !has(st, seat, "qiaobian") || !is_jiexian(st, seat) {
        return;
    }
    let n = st.seats[seat].hand.len();
    let key = format!("qjb_end:{n}");
    let seen = st.seats[seat].flags.contains_key(&key);
    st.seats[seat].flags.insert(key, true);
    if !seen {
        add_bian(st, seat, 1);
        st.push_log(
            Some(&pid(st, seat)),
            format!("巧变：结束阶段手牌数为 {n}（与此前均不同），获得 1 枚「变」"),
        );
    }
}

/// 巧变支付方式：优先弃一张手牌；无手牌时可用 1 枚「变」（界张郃）。
pub(crate) fn qiaobian_can_pay(st: &SgsState, seat: usize) -> bool {
    !st.seats[seat].hand.is_empty()
        || (has(st, seat, "qiaobian") && is_jiexian(st, seat) && bian_marks(st, seat) > 0)
}

/// 用「变」支付（无手牌时）。
pub(crate) fn qiaobian_pay_bian(st: &mut SgsState, seat: usize) -> bool {
    remove_bian(st, seat)
}

// ---------------------------------------------------------------------------
// 界贾诩·帷幕：回合内受到的伤害防止并摸 2 倍
// ---------------------------------------------------------------------------

pub(crate) fn weimu_prevent_damage(st: &mut SgsState, victim: usize, amount: i32) -> bool {
    if amount <= 0 || st.turn != victim || !has(st, victim, "weimu") || !is_jiexian(st, victim) {
        return false;
    }
    let mut ev = Vec::new();
    let drawn = turn::draw_cards(st, victim, (amount * 2) as u32, &mut ev);
    st.push_log(
        Some(&pid(st, victim)),
        format!("帷幕：防止回合内受到的 {amount} 点伤害并摸 {drawn} 张"),
    );
    true
}

// ---------------------------------------------------------------------------
// 界凿险（界邓艾）：本回合结束后执行一个额外回合
// ---------------------------------------------------------------------------

pub(crate) fn mark_zaoxian_extra_turn(st: &mut SgsState, seat: usize) {
    st.seats[seat].flags.insert("zaoxian_extra".into(), true);
    let _ = json!({});
}
