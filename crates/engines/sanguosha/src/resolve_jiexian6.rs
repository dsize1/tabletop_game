//! P4.4.2 界限突破（批次 J5：8 将）的公共小钩子。
//!
//! 覆盖：界旋风（弃牌累计近似）、界明策（伤害后各摸 1）、界将驰、界巧说、
//! 界直言（上家结束阶段 + 非装备失体力）、界禁酒、界恩怨（红色手牌）、
//! 界疠火（弃牌或失体力）、界醇醪（相邻角色【杀】自动入「醇」）。

use crate::skills::{has, is_jiexian};
use crate::state::*;
use crate::turn;

fn pid(st: &SgsState, seat: usize) -> String {
    st.seats[seat].player_id.clone()
}

/// 座位是否相邻（存活座位环）。
pub(crate) fn adjacent(st: &SgsState, a: usize, b: usize) -> bool {
    if a == b {
        return false;
    }
    let alive = st.alive_indices();
    let n = alive.len();
    if n <= 1 {
        return false;
    }
    let ia = alive.iter().position(|i| *i == a);
    let ib = alive.iter().position(|i| *i == b);
    match (ia, ib) {
        (Some(ia), Some(ib)) => {
            let d = ia.abs_diff(ib);
            d == 1 || d + 1 == n
        }
        _ => false,
    }
}

/// 上家（存活座位环中位于 `seat` 之前的一位）。
pub(crate) fn upper_seat(st: &SgsState, seat: usize) -> Option<usize> {
    let alive = st.alive_indices();
    let n = alive.len();
    if n <= 1 {
        return None;
    }
    let idx = alive.iter().position(|i| *i == seat)?;
    Some(alive[(idx + n - 1) % n])
}

/// 界醇醪（界程普）：把因弃置进入弃牌堆的【杀】置于「醇」（至多 9 张）。
pub(crate) fn chunlao_place(st: &mut SgsState, owner: usize, card_id: &str) {
    let cur = st.seats[owner]
        .marks
        .get("chun")
        .map(|v| v.len())
        .unwrap_or(0);
    if cur >= 9 {
        return;
    }
    if !st.deck.discard.iter().any(|c| c == card_id) {
        return;
    }
    st.deck.discard.retain(|c| c != card_id);
    st.seats[owner]
        .marks
        .entry("chun".into())
        .or_default()
        .push(card_id.to_string());
    st.push_log(
        Some(&pid(st, owner)),
        format!(
            "醇醪：将【{}】置于武将牌上为「醇」（{}）",
            turn::card_label(st, card_id),
            cur + 1
        ),
    );
}

/// 界明策：虚拟【杀】造成伤害后，陈宫与其各摸一张牌。
pub(crate) fn mingce_on_damage(st: &mut SgsState, source: usize) {
    let keys: Vec<String> = st.seats[source]
        .flags
        .keys()
        .filter(|k| k.starts_with("mingce_sha:"))
        .cloned()
        .collect();
    for k in keys {
        st.seats[source].flags.remove(&k);
        let owner: usize = k
            .trim_start_matches("mingce_sha:")
            .parse()
            .unwrap_or(usize::MAX);
        if owner >= st.seats.len() {
            continue;
        }
        let mut ev = Vec::new();
        let a = turn::draw_cards(st, source, 1, &mut ev);
        let b = turn::draw_cards(st, owner, 1, &mut ev);
        st.push_log(
            Some(&pid(st, owner)),
            format!("明策：此【杀】造成伤害，双方各摸 1 张（{a} / {b}）"),
        );
    }
}

/// 界疠火（界程普）：视为火【杀】造成伤害后，弃一张牌或失去 1 点体力（自动：有牌弃牌）。
pub(crate) fn lihuo_jie_cost(st: &mut SgsState, seat: usize) {
    if !st.seats[seat].hand.is_empty() {
        let idx = st.next_index(st.seats[seat].hand.len());
        let card = st.seats[seat].hand.remove(idx);
        let label = turn::card_label(st, &card);
        st.deck.discard.push(card);
        st.push_log(
            Some(&pid(st, seat)),
            format!("疠火：弃置【{label}】作为代价"),
        );
        return;
    }
    st.seats[seat].hp -= 1;
    let hp = st.seats[seat].hp;
    st.push_log(
        Some(&pid(st, seat)),
        format!("疠火：失去 1 点体力（体力 {hp}）"),
    );
    crate::skills::zhaxiang_on_lose_hp(st, seat);
    if hp <= 0 {
        crate::resolve::push_dying(st, seat, None);
    }
}

/// 界直言（界虞翻）：上家的结束阶段也可发动（每回合限一次）。
pub(crate) fn zhiyan_upper_trigger(st: &mut SgsState, turn_seat: usize) {
    let owners: Vec<usize> = st
        .alive_indices()
        .into_iter()
        .filter(|i| {
            has(st, *i, "zhiyan")
                && is_jiexian(st, *i)
                && upper_seat(st, *i) == Some(turn_seat)
                && st.counter(&format!("zhiyan_upper:{i}")) == 0
        })
        .collect();
    for owner in owners {
        st.set_counter(&format!("zhiyan_upper:{owner}"), 1);
        crate::resolve::push(
            st,
            crate::resolve::Frame::ZhiYan(crate::resolve_yijiang4::ZhiYanFrame { actor: owner }),
        );
    }
}

/// 界禁酒（界高顺）：其他角色不能于高顺的回合使用【酒】。
pub(crate) fn jinjiu_blocks_jiu(st: &SgsState, user: usize) -> bool {
    if st.turn == user {
        return false;
    }
    st.seats.get(st.turn).is_some_and(|_| {
        st.alive_indices()
            .into_iter()
            .any(|i| i == st.turn && has(st, i, "jinjiu") && is_jiexian(st, i))
    })
}
