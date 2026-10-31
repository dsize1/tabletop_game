//! P4.4.1 SP-2 技能帧（卢植/郝昭/陈到/诸葛瞻/周妃/毌丘俭/陆抗/张绣）。
//!
//! 机制口径见 ADR-0029 与 `skills.md` §2.6：
//! - 武将牌标记「任/荣/箜声」（复用 `Seat.marks`）
//! - 废除装备栏 / 判定区（`Seat.abolished_slots` / `judge_abolished`）
//! - 延迟结算（`SgsState.scheduled`，镇骨）
//! - 「不可响应」「本阶段不能再使用牌」「本回合手牌被封」
//! - 观看牌堆顶三张（罪论特殊区 `zuilun`）

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
// 卢植 · 明任 / 贞良
// ---------------------------------------------------------------------------

/// 明任初始化（finish_setup）：摸 2 张，然后将一张手牌置于武将牌上为「任」。
pub(crate) fn init_mingren(st: &mut SgsState, seat: usize) {
    if !skills::has(st, seat, "mingren") {
        return;
    }
    let drawn = draw_capped(st, seat, 2);
    st.push_log(
        Some(&st.seats[seat].player_id.clone()),
        format!("明任：摸 {drawn} 张，并选择一张手牌置于武将牌上"),
    );
    push(
        st,
        Frame::MultiSelect(MultiSelectFrame {
            actor: seat,
            purpose: "mingren_init".into(),
            selected: Vec::new(),
            min: 0,
            max: 1,
            data: json!({}),
        }),
    );
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MingRenFrame {
    pub actor: usize,
}

/// 结束阶段：可用手牌替换「任」。
pub(crate) fn step_mingren(st: &mut SgsState, f: MingRenFrame) -> Result<bool, EngineError> {
    if skills::mark_cards(st, f.actor, "ren").is_empty() || st.seats[f.actor].hand.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("mingren".into()),
        options: vec![
            PendingOption {
                id: "choose:mingren".into(),
                label: "明任：用手牌替换「任」".into(),
            },
            pass_option("不替换"),
        ],
        data: json!({ "skill": "mingren" }),
    });
    replace_top(st, Frame::MingRen(f));
    Ok(true)
}

pub(crate) fn act_mingren(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::MingRen(f)) => f,
        _ => return Err(internal("栈顶不是明任帧")),
    };
    if action != "choose:mingren" {
        pop(st);
        return Ok("不替换「任」".to_string());
    }
    replace_top(
        st,
        Frame::MultiSelect(MultiSelectFrame {
            actor: f.actor,
            purpose: "mingren".into(),
            selected: Vec::new(),
            min: 1,
            max: 1,
            data: json!({}),
        }),
    );
    Ok("明任：请选择替换「任」的手牌".to_string())
}

/// 明任替换（MultiSelect 确认后）：手牌 →「任」，原「任」→ 手牌。
pub(crate) fn mingren_swap(
    st: &mut SgsState,
    seat: usize,
    card: &str,
) -> Result<String, EngineError> {
    let ren = skills::mark_cards(st, seat, "ren")
        .into_iter()
        .next()
        .ok_or_else(|| illegal("没有「任」"))?;
    if turn::take_card(st, seat, card).is_err() {
        return Err(illegal("手牌中没有该牌"));
    }
    skills::remove_mark(st, seat, "ren", &ren);
    skills::add_mark(st, seat, "ren", card);
    st.seats[seat].hand.push(ren.clone());
    st.push_log(
        Some(&st.seats[seat].player_id.clone()),
        format!("明任：用【{}】替换「任」", turn::card_label(st, card)),
    );
    Ok("明任：已替换「任」".to_string())
}

/// 贞良（转换技）目标：攻击范围内的其他角色。
pub(crate) fn zhenliang_targets(st: &SgsState, actor: usize) -> Vec<usize> {
    st.alive_indices()
        .into_iter()
        .filter(|t| *t != actor && st.in_attack_range(actor, *t))
        .collect()
}

/// 贞良（阳）可用：有「任」且至少一张同色手牌 + 有目标。
pub(crate) fn zhenliang_available(st: &SgsState, actor: usize) -> bool {
    if !skills::has(st, actor, "zhenliang") || skills::yj_yin(st, actor, "zhenliang") {
        return false;
    }
    if st.counter("zhenliang_used") > 0 || zhenliang_targets(st, actor).is_empty() {
        return false;
    }
    if ren_color(st, actor).is_none() {
        return false;
    }
    same_color_hand(st, actor).is_some()
}

/// 「任」的颜色（红/黑）。
fn ren_color(st: &SgsState, seat: usize) -> Option<bool> {
    let ren = skills::mark_cards(st, seat, "ren").into_iter().next()?;
    st.cards.get(&ren).map(|c| skills::is_red(c.suit))
}

fn same_color_hand(st: &SgsState, seat: usize) -> Option<String> {
    let red = ren_color(st, seat)?;
    st.seats[seat]
        .hand
        .iter()
        .find(|id| {
            st.cards
                .get(*id)
                .is_some_and(|c| skills::is_red(c.suit) == red)
        })
        .cloned()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZhenLiangStage {
    Target,
    Discard,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZhenLiangFrame {
    pub actor: usize,
    pub stage: ZhenLiangStage,
    pub target: Option<usize>,
}

pub(crate) fn step_zhenliang(st: &mut SgsState, f: ZhenLiangFrame) -> Result<bool, EngineError> {
    match f.stage {
        ZhenLiangStage::Target => {
            let options: Vec<PendingOption> = zhenliang_targets(st, f.actor)
                .into_iter()
                .map(|t| PendingOption {
                    id: format!("target:{}", st.seats[t].player_id),
                    label: format!("贞良（阳）：{}", general_name(st, t)),
                })
                .collect();
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("zhenliang".into()),
                options,
                data: json!({ "stage": "zhenliang" }),
            });
            replace_top(st, Frame::ZhenLiang(f));
            Ok(true)
        }
        ZhenLiangStage::Discard => {
            let red = ren_color(st, f.actor);
            let options: Vec<PendingOption> = st.seats[f.actor]
                .hand
                .iter()
                .filter(|id| {
                    st.cards
                        .get(*id)
                        .is_some_and(|c| Some(skills::is_red(c.suit)) == red)
                })
                .map(|id| PendingOption {
                    id: format!("choose:{id}"),
                    label: format!("贞良：弃置【{}】", turn::card_label(st, id)),
                })
                .collect();
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("zhenliang".into()),
                options,
                data: json!({ "skill": "zhenliang" }),
            });
            replace_top(st, Frame::ZhenLiang(f));
            Ok(true)
        }
    }
}

pub(crate) fn zhenliang_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ZhenLiang(f)) => f,
        _ => return Err(internal("栈顶不是贞良帧")),
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
        Frame::ZhenLiang(ZhenLiangFrame {
            actor: f.actor,
            stage: ZhenLiangStage::Discard,
            target: Some(target),
        }),
    );
    Ok("贞良：请弃置一张与「任」同色的牌".to_string())
}

pub(crate) fn act_zhenliang(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ZhenLiang(f)) => f,
        _ => return Err(internal("栈顶不是贞良帧")),
    };
    let Some(target) = f.target else {
        return Err(illegal("贞良缺少目标"));
    };
    let card = action
        .strip_prefix("choose:")
        .ok_or_else(|| illegal("需要选择弃置的牌"))?;
    if turn::take_card(st, f.actor, card).is_err() {
        return Err(illegal("手牌中没有该牌"));
    }
    turn::to_discard_card(st, card);
    pop(st);
    skills::toggle_yj(st, f.actor, "zhenliang");
    st.set_counter("zhenliang_used", 1);
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        format!(
            "贞良（阳）：弃置【{}】对 {} 造成 1 点伤害",
            turn::card_label(st, card),
            general_name(st, target)
        ),
    );
    crate::resolve::apply_damage(st, target, 1, Some(f.actor), DamageType::Normal, None);
    Ok("贞良（阳）：已完成".to_string())
}

/// 贞良（阴）触发点：回合外使用/打出的牌进入弃牌堆且与「任」同色。
pub(crate) fn push_zhenliang_yin(st: &mut SgsState, owner: usize, card: &str) {
    if !skills::has(st, owner, "zhenliang") || !skills::yj_yin(st, owner, "zhenliang") {
        return;
    }
    if st.turn == owner || st.counter("zhenliang_yin_done") > 0 {
        return;
    }
    let Some(red) = ren_color(st, owner) else {
        return;
    };
    if !st
        .cards
        .get(card)
        .is_some_and(|c| skills::is_red(c.suit) == red)
    {
        return;
    }
    push(
        st,
        Frame::ZhenLiangYin(ZhenLiangYinFrame {
            owner,
            card: card.to_string(),
            stage: ZhenLiangYinStage::Ask,
        }),
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZhenLiangYinStage {
    Ask,
    Target,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZhenLiangYinFrame {
    pub owner: usize,
    pub card: String,
    pub stage: ZhenLiangYinStage,
}

pub(crate) fn step_zhenliang_yin(
    st: &mut SgsState,
    f: ZhenLiangYinFrame,
) -> Result<bool, EngineError> {
    match f.stage {
        ZhenLiangYinStage::Ask => {
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: st.seats[f.owner].player_id.clone(),
                skill: Some("zhenliang".into()),
                options: vec![
                    PendingOption {
                        id: "choose:zhenliang_yin".into(),
                        label: "贞良（阴）：令一名角色摸 1 张".into(),
                    },
                    pass_option("不发动贞良（阴）"),
                ],
                data: json!({ "skill": "zhenliang" }),
            });
            replace_top(st, Frame::ZhenLiangYin(f));
            Ok(true)
        }
        ZhenLiangYinStage::Target => {
            let options: Vec<PendingOption> = st
                .alive_indices()
                .into_iter()
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("贞良（阴）：令 {} 摸 1 张", general_name(st, i)),
                })
                .collect();
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.owner].player_id.clone(),
                skill: Some("zhenliang".into()),
                options,
                data: json!({ "stage": "zhenliang_yin" }),
            });
            replace_top(st, Frame::ZhenLiangYin(f));
            Ok(true)
        }
    }
}

pub(crate) fn act_zhenliang_yin(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ZhenLiangYin(f)) => f,
        _ => return Err(internal("栈顶不是贞良（阴）帧")),
    };
    if action != "choose:zhenliang_yin" {
        pop(st);
        return Ok("不发动贞良（阴）".to_string());
    }
    replace_top(
        st,
        Frame::ZhenLiangYin(ZhenLiangYinFrame {
            stage: ZhenLiangYinStage::Target,
            ..f
        }),
    );
    Ok("贞良（阴）：请选择摸牌角色".to_string())
}

pub(crate) fn zhenliang_yin_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ZhenLiangYin(f)) => f,
        _ => return Err(internal("栈顶不是贞良（阴）帧")),
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
    st.set_counter("zhenliang_yin_done", 1);
    skills::toggle_yj(st, f.owner, "zhenliang");
    let drawn = draw_capped(st, target, 1);
    st.push_log(
        Some(&st.seats[f.owner].player_id.clone()),
        format!(
            "贞良（阴）：【{}】入弃牌堆，令 {} 摸 {drawn} 张",
            turn::card_label(st, &f.card),
            general_name(st, target)
        ),
    );
    Ok("贞良（阴）：已完成".to_string())
}

// ---------------------------------------------------------------------------
// 郝昭 · 镇骨（本回合结束 + 目标下回合结束各调整一次）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZhenGuFrame {
    pub actor: usize,
}

pub(crate) fn step_zhengu(st: &mut SgsState, f: ZhenGuFrame) -> Result<bool, EngineError> {
    let options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| *i != f.actor)
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("镇骨：{}", general_name(st, i)),
        })
        .collect();
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "target".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("zhengu".into()),
        options,
        data: json!({ "stage": "zhengu" }),
    });
    replace_top(st, Frame::ZhenGu(f));
    Ok(true)
}

pub(crate) fn zhengu_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ZhenGu(f)) => f,
        _ => return Err(internal("栈顶不是镇骨帧")),
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
    zhengu_adjust(st, f.actor, target);
    st.scheduled.push(ScheduledEffect {
        owner: f.actor,
        target,
        kind: "zhengu".into(),
        due_seat: target,
    });
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        format!(
            "镇骨：{} 本回合与其下回合结束时手牌调整至与你相同",
            general_name(st, target)
        ),
    );
    Ok("镇骨：已选定目标".to_string())
}

/// 镇骨结算：目标手牌调整至与郝昭相同（摸至多补到 5；超出弃置）。
pub(crate) fn zhengu_adjust(st: &mut SgsState, owner: usize, target: usize) {
    if !st.seats.get(target).is_some_and(|s| s.alive) {
        return;
    }
    let want = if st.seats.get(owner).is_some_and(|s| s.alive) {
        st.seats[owner].hand.len().min(5)
    } else {
        return;
    };
    let cur = st.seats[target].hand.len();
    if cur < want {
        let drawn = draw_capped(st, target, (want - cur) as u32);
        st.push_log(
            None,
            format!(
                "镇骨：{} 摸 {drawn} 张（手牌调整至 {want}）",
                general_name(st, target)
            ),
        );
    } else if cur > want {
        let over = cur - want;
        crate::resolve::push_discard(st, target, over, "zhengu");
        st.push_log(
            None,
            format!(
                "镇骨：{} 弃置 {over} 张（手牌调整至 {want}）",
                general_name(st, target)
            ),
        );
    }
}

// ---------------------------------------------------------------------------
// 陈到 · 往烈（首张牌无距离；基本/锦囊不可响应 + 本阶段封禁）
// ---------------------------------------------------------------------------

/// 往烈可用：本阶段未发动、有基本牌或普通锦囊可出。
pub(crate) fn wanglie_available(st: &SgsState, actor: usize) -> bool {
    skills::has(st, actor, "wanglie")
        && st.counter("wanglie_used") == 0
        && !skills::no_card_use(st, actor)
}

// ---------------------------------------------------------------------------
// 诸葛瞻 · 罪论 / 父荫
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZuiLunStage {
    Pick,
    Zero,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZuiLunFrame {
    pub actor: usize,
    pub cards: Vec<String>,
    pub stage: ZuiLunStage,
    pub picks: u32,
}

/// 罪论条件数（①本回合造成过伤害；②本回合未弃置过牌；③手牌数全场最少）。
pub(crate) fn zuilun_count(st: &SgsState, actor: usize) -> u32 {
    let mut k = 0;
    if st.counter("damage_dealt") > 0 {
        k += 1;
    }
    if st.counter("discard_done") == 0 {
        k += 1;
    }
    let mine = st.seats[actor].hand.len();
    if st
        .alive_indices()
        .into_iter()
        .all(|i| i == actor || st.seats[i].hand.len() >= mine)
    {
        k += 1;
    }
    k
}

pub(crate) fn start_zuilun(st: &mut SgsState) -> bool {
    let actor = st.turn;
    if !skills::has(st, actor, "zuilun") || st.counter("zuilun_done") > 0 {
        return false;
    }
    st.set_counter("zuilun_done", 1);
    let mut cards = Vec::new();
    let mut ev = Vec::new();
    for _ in 0..3 {
        let Some(card) = turn::draw_top_public(st, &mut ev) else {
            break;
        };
        st.deck.processing.push(card.clone());
        cards.push(card);
    }
    let k = zuilun_count(st, actor);
    // 特殊区（仅本人可见）：亮出的三张。
    let mut payload = serde_json::Map::new();
    payload.insert(
        "cards".into(),
        json!(cards
            .iter()
            .map(|c| crate::view::card_view(st, c))
            .collect::<Vec<_>>()),
    );
    payload.insert("conditions".into(), json!(k));
    st.special_zones.insert(
        "zuilun".into(),
        SpecialZone {
            owner: Some(st.seats[actor].player_id.clone()),
            visibility: Visibility::Owner,
            payload,
        },
    );
    let stage = if k == 0 {
        ZuiLunStage::Zero
    } else {
        ZuiLunStage::Pick
    };
    push(
        st,
        Frame::ZuiLun(ZuiLunFrame {
            actor,
            cards,
            stage,
            picks: k,
        }),
    );
    true
}

pub(crate) fn step_zuilun(st: &mut SgsState, f: ZuiLunFrame) -> Result<bool, EngineError> {
    match f.stage {
        ZuiLunStage::Pick => {
            let options: Vec<PendingOption> = f
                .cards
                .iter()
                .map(|id| PendingOption {
                    id: format!("choose:{id}"),
                    label: format!("罪论：获得【{}】", turn::card_label(st, id)),
                })
                .collect();
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("zuilun".into()),
                options,
                data: json!({ "skill": "zuilun", "remaining": f.picks }),
            });
            replace_top(st, Frame::ZuiLun(f));
            Ok(true)
        }
        ZuiLunStage::Zero => {
            let options: Vec<PendingOption> = st
                .alive_indices()
                .into_iter()
                .filter(|i| *i != f.actor)
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("罪论：与 {} 各失去 1 点体力", general_name(st, i)),
                })
                .collect();
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.actor].player_id.clone(),
                skill: Some("zuilun".into()),
                options,
                data: json!({ "stage": "zuilun_zero" }),
            });
            replace_top(st, Frame::ZuiLun(f));
            Ok(true)
        }
    }
}

pub(crate) fn act_zuilun(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ZuiLun(f)) => f,
        _ => return Err(internal("栈顶不是罪论帧")),
    };
    if f.stage != ZuiLunStage::Pick {
        return Err(illegal("请通过目标界面继续罪论"));
    }
    let card = action
        .strip_prefix("choose:")
        .ok_or_else(|| illegal("需要选择获得的牌"))?;
    if !f.cards.iter().any(|c| c == card) {
        return Err(illegal("该牌不在亮出的三张中"));
    }
    crate::resolve::leave_processing(st, card);
    st.seats[f.actor].hand.push(card.to_string());
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        format!("罪论：获得【{}】", turn::card_label(st, card)),
    );
    let mut cards: Vec<String> = f.cards.iter().filter(|c| *c != card).cloned().collect();
    let mut picks = f.picks.saturating_sub(1);
    if picks > 0 && !cards.is_empty() {
        replace_top(
            st,
            Frame::ZuiLun(ZuiLunFrame {
                actor: f.actor,
                cards,
                stage: ZuiLunStage::Pick,
                picks,
            }),
        );
        return Ok("罪论：可继续获得一张".to_string());
    }
    // 剩余牌按原顺序放回牌堆顶。
    while let Some(id) = cards.pop() {
        crate::resolve::leave_processing(st, &id);
        st.deck.draw.push(id);
    }
    st.special_zones.remove("zuilun");
    pop(st);
    let over = st.seats[f.actor]
        .hand
        .len()
        .saturating_sub(st.config.max_hand_cards as usize);
    if over > 0 {
        crate::resolve::push_discard(st, f.actor, over, "hand_cap");
    }
    picks = 0;
    let _ = picks;
    Ok("罪论：已完成".to_string())
}

pub(crate) fn zuilun_zero_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ZuiLun(f)) => f,
        _ => return Err(internal("栈顶不是罪论帧")),
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
    // 亮出的牌按原顺序放回牌堆顶。
    let mut cards = f.cards.clone();
    while let Some(id) = cards.pop() {
        crate::resolve::leave_processing(st, &id);
        st.deck.draw.push(id);
    }
    st.special_zones.remove("zuilun");
    pop(st);
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        format!(
            "罪论：条件均不满足，与 {} 各失去 1 点体力",
            general_name(st, target)
        ),
    );
    for seat in [f.actor, target] {
        st.seats[seat].hp -= 1;
    }
    for seat in [f.actor, target] {
        if st.seats[seat].hp <= 0 {
            crate::resolve::push_dying(st, seat, None);
        }
    }
    Ok("罪论：均不满足".to_string())
}

/// 父荫（锁定）：每回合首次成为【杀】/【决斗】目标 → 此牌无效。
pub(crate) fn fuyin_take(st: &mut SgsState, owner: usize, user: usize) -> bool {
    if !skills::fuyin_blocks(st, owner, user) {
        return false;
    }
    let key = format!("fuyin:{}", owner);
    if st.counter(&key) > 0 {
        return false;
    }
    st.set_counter(&key, 1);
    st.set_counter("fuyin_done", 1);
    st.push_log(
        Some(&st.seats[owner].player_id.clone()),
        format!(
            "父荫：{} 的手牌数不少于你，此牌对你无效",
            general_name(st, user)
        ),
    );
    true
}

// ---------------------------------------------------------------------------
// 周妃 · 良姻 / 箜声
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiangYinStage {
    Choice,
    Target,
    Heal,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LiangYinFrame {
    pub owner: usize,
    pub stage: LiangYinStage,
    /// `true` = 各摸一张；`false` = 各弃一张。
    pub draw: bool,
    pub other: Option<usize>,
}

/// 良姻触发点：有牌移出/移入游戏后（每回合首次）。
pub(crate) fn liangyin_trigger(st: &mut SgsState) {
    let Some(owner) = st
        .alive_indices()
        .into_iter()
        .find(|i| skills::has(st, *i, "liangyin"))
    else {
        return;
    };
    if st.alive_indices().len() < 2 || !skills::first_each_turn(st, "liangyin") {
        return;
    }
    push(
        st,
        Frame::LiangYin(LiangYinFrame {
            owner,
            stage: LiangYinStage::Choice,
            draw: true,
            other: None,
        }),
    );
}

pub(crate) fn step_liangyin(st: &mut SgsState, f: LiangYinFrame) -> Result<bool, EngineError> {
    match f.stage {
        LiangYinStage::Choice => {
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: st.seats[f.owner].player_id.clone(),
                skill: Some("liangyin".into()),
                options: vec![
                    PendingOption {
                        id: "choose:liangyin:draw".into(),
                        label: "良姻：你与一名其他角色各摸 1 张".into(),
                    },
                    PendingOption {
                        id: "choose:liangyin:discard".into(),
                        label: "良姻：你与一名其他角色各弃 1 张".into(),
                    },
                    pass_option("不发动良姻"),
                ],
                data: json!({ "skill": "liangyin" }),
            });
            replace_top(st, Frame::LiangYin(f));
            Ok(true)
        }
        LiangYinStage::Target => {
            let options: Vec<PendingOption> = st
                .alive_indices()
                .into_iter()
                .filter(|i| *i != f.owner)
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("良姻：{}", general_name(st, i)),
                })
                .collect();
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.owner].player_id.clone(),
                skill: Some("liangyin".into()),
                options,
                data: json!({ "stage": "liangyin" }),
            });
            replace_top(st, Frame::LiangYin(f));
            Ok(true)
        }
        LiangYinStage::Heal => {
            let x = skills::mark_cards(st, f.owner, "kong").len();
            let options: Vec<PendingOption> = [Some(f.owner), f.other]
                .into_iter()
                .flatten()
                .filter(|i| st.seats[*i].hand.len() == x)
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("良姻：令 {} 回复 1 点体力", general_name(st, i)),
                })
                .collect();
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            let mut options = options;
            options.push(pass_option("不回复"));
            st.pending = Some(Pending {
                kind: "target".into(),
                actor: st.seats[f.owner].player_id.clone(),
                skill: Some("liangyin".into()),
                options,
                data: json!({ "stage": "liangyin_heal" }),
            });
            replace_top(st, Frame::LiangYin(f));
            Ok(true)
        }
    }
}

pub(crate) fn act_liangyin(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::LiangYin(f)) => f,
        _ => return Err(internal("栈顶不是良姻帧")),
    };
    let draw = match action {
        "choose:liangyin:draw" => true,
        "choose:liangyin:discard" => false,
        _ => {
            pop(st);
            return Ok("不发动良姻".to_string());
        }
    };
    st.set_counter("liangyin_done", 1);
    replace_top(
        st,
        Frame::LiangYin(LiangYinFrame {
            owner: f.owner,
            stage: LiangYinStage::Target,
            draw,
            other: None,
        }),
    );
    Ok("良姻：请选择一名其他角色".to_string())
}

pub(crate) fn liangyin_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::LiangYin(f)) => f,
        _ => return Err(internal("栈顶不是良姻帧")),
    };
    if f.stage == LiangYinStage::Heal {
        if action == "pass" {
            pop(st);
            return Ok("良姻：不回复".to_string());
        }
        let pid = action
            .strip_prefix("target:")
            .ok_or_else(|| illegal("需要选择目标"))?;
        let seat = st.seat_index(pid).ok_or_else(|| illegal("目标不在座"))?;
        pop(st);
        st.seats[seat].hp = (st.seats[seat].hp + 1).min(st.seats[seat].max_hp);
        st.push_log(
            Some(&st.seats[f.owner].player_id.clone()),
            format!("良姻：令 {} 回复 1 点体力", general_name(st, seat)),
        );
        return Ok("良姻：已完成".to_string());
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
    let other = st.seat_index(pid).ok_or_else(|| illegal("目标不在座"))?;
    if f.draw {
        let a = draw_capped(st, f.owner, 1);
        let b = draw_capped(st, other, 1);
        st.push_log(
            Some(&st.seats[f.owner].player_id.clone()),
            format!("良姻：与 {} 各摸 1 张（{a}/{b}）", general_name(st, other)),
        );
    } else {
        for seat in [f.owner, other] {
            if let Some(card) = st.seats[seat].hand.first().cloned() {
                if turn::take_card(st, seat, &card).is_ok() {
                    turn::to_discard_card(st, &card);
                }
            }
        }
        st.push_log(
            Some(&st.seats[f.owner].player_id.clone()),
            format!("良姻：与 {} 各弃 1 张", general_name(st, other)),
        );
    }
    replace_top(
        st,
        Frame::LiangYin(LiangYinFrame {
            owner: f.owner,
            stage: LiangYinStage::Heal,
            draw: f.draw,
            other: Some(other),
        }),
    );
    Ok("良姻：可令手牌数为箜声数的角色回复 1 点体力".to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KongShengStage {
    /// 准备阶段：选择置于武将牌上的牌。
    Keep,
    /// 结束阶段：选择使用剩余装备牌的角色。
    UseEquip,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KongShengFrame {
    pub actor: usize,
    pub stage: KongShengStage,
    pub cards: Vec<String>,
}

pub(crate) fn step_kongsheng(st: &mut SgsState, f: KongShengFrame) -> Result<bool, EngineError> {
    match f.stage {
        KongShengStage::Keep => {
            if st.seats[f.actor].hand.is_empty() {
                pop(st);
                return Ok(false);
            }
            replace_top(
                st,
                Frame::MultiSelect(MultiSelectFrame {
                    actor: f.actor,
                    purpose: "kongsheng".into(),
                    selected: Vec::new(),
                    min: 0,
                    max: st.seats[f.actor].hand.len(),
                    data: json!({}),
                }),
            );
            Ok(false)
        }
        KongShengStage::UseEquip => {
            let options: Vec<PendingOption> = st
                .alive_indices()
                .into_iter()
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!(
                        "箜声：令 {} 使用剩余装备并失去 1 点体力",
                        general_name(st, i)
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
                skill: Some("kongsheng".into()),
                options,
                data: json!({ "stage": "kongsheng" }),
            });
            replace_top(st, Frame::KongSheng(f));
            Ok(true)
        }
    }
}

pub(crate) fn kongsheng_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::KongSheng(f)) => f,
        _ => return Err(internal("栈顶不是箜声帧")),
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
    for card in f.cards.iter() {
        if turn::card_def(st, card).is_some_and(|d| d.equip_slot.is_some()) {
            // 装备牌：置入目标装备区（废除栏则弃置）。
            let slot = turn::card_def(st, card)
                .and_then(|d| d.equip_slot)
                .expect("equip slot");
            if st.slot_abolished(target, slot) {
                turn::to_discard_card(st, card);
                continue;
            }
            let old = st.seats[target].equips.insert(slot, Some(card.clone()));
            if let Some(Some(old_id)) = old {
                turn::to_discard_card(st, &old_id);
                skills::xiaoji_on_lose_equip(st, target);
            }
        } else {
            turn::to_discard_card(st, card);
        }
    }
    st.seats[target].hp -= 1;
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        format!(
            "箜声：令 {} 使用剩余装备并失去 1 点体力",
            general_name(st, target)
        ),
    );
    if st.seats[target].hp <= 0 {
        crate::resolve::push_dying(st, target, None);
    }
    Ok("箜声：已完成".to_string())
}

// ---------------------------------------------------------------------------
// 毌丘俭 · 征荣 / 鸿举
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZhengRongFrame {
    pub actor: usize,
    pub card: String,
}

/// 征荣触发点：使用【杀】或伤害锦囊时（调用方判定牌名）。
pub(crate) fn zhengrong_trigger(st: &mut SgsState, user: usize, card: &str) {
    if !skills::has(st, user, "zhengrong") || st.turn != user {
        return;
    }
    let has_target = st.alive_indices().into_iter().any(|i| {
        i != user
            && st.seats[i].hand.len() >= st.seats[user].hand.len()
            && !st.seats[i].hand.is_empty()
    });
    if !has_target {
        return;
    }
    push(
        st,
        Frame::ZhengRong(ZhengRongFrame {
            actor: user,
            card: card.to_string(),
        }),
    );
}

pub(crate) fn step_zhengrong(st: &mut SgsState, f: ZhengRongFrame) -> Result<bool, EngineError> {
    let options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| {
            *i != f.actor
                && st.seats[*i].hand.len() >= st.seats[f.actor].hand.len()
                && !st.seats[*i].hand.is_empty()
        })
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("征荣：{}", general_name(st, i)),
        })
        .collect();
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    let mut options = options;
    options.push(pass_option("不发动征荣"));
    st.pending = Some(Pending {
        kind: "target".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("zhengrong".into()),
        options,
        data: json!({ "stage": "zhengrong" }),
    });
    replace_top(st, Frame::ZhengRong(f));
    Ok(true)
}

pub(crate) fn zhengrong_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ZhengRong(f)) => f,
        _ => return Err(internal("栈顶不是征荣帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动征荣".to_string());
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
        pop(st);
        return Ok("征荣：目标没有手牌".to_string());
    }
    let n = st.seats[target].hand.len();
    let idx = st.next_index(n);
    let card = st.seats[target].hand.remove(idx);
    skills::add_mark(st, f.actor, "rong", &card);
    let label = turn::card_label(st, &card);
    pop(st);
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        format!(
            "征荣：将 {} 的一张牌【{label}】置于武将牌上为「荣」",
            general_name(st, target)
        ),
    );
    Ok("征荣：已置「荣」".to_string())
}

/// 鸿举（觉醒）：准备阶段「荣」≥3 → 手牌替换等量「荣」、减 1 上限、获得清侧。
pub(crate) fn hongju_awaken(st: &mut SgsState, seat: usize) -> bool {
    if !skills::has(st, seat, "hongju") || st.seats[seat].flags.contains_key("awaken_hongju") {
        return false;
    }
    let rong = skills::mark_cards(st, seat, "rong");
    if rong.len() < 3 {
        return false;
    }
    st.seats[seat].flags.insert("awaken_hongju".into(), true);
    st.seats[seat].max_hp = (st.seats[seat].max_hp - 1).max(1);
    st.seats[seat].hp = st.seats[seat].hp.min(st.seats[seat].max_hp);
    skills::grant_skill(st, seat, "qingce");
    st.push_log(
        Some(&st.seats[seat].player_id.clone()),
        "鸿举：觉醒，减少 1 点体力上限并获得「清侧」",
    );
    let n = rong.len().min(st.seats[seat].hand.len());
    if n > 0 {
        push(
            st,
            Frame::MultiSelect(MultiSelectFrame {
                actor: seat,
                purpose: "hongju".into(),
                selected: Vec::new(),
                min: n,
                max: n,
                data: json!({}),
            }),
        );
    }
    true
}

/// 鸿举替换：手牌 →「荣」，等量「荣」→ 手牌。
pub(crate) fn hongju_swap(
    st: &mut SgsState,
    seat: usize,
    hand_cards: &[String],
) -> Result<String, EngineError> {
    let mut rong = skills::mark_cards(st, seat, "rong");
    for card in hand_cards {
        if turn::take_card(st, seat, card).is_err() {
            continue;
        }
        skills::add_mark(st, seat, "rong", card);
    }
    for card in rong.drain(..).take(hand_cards.len()) {
        skills::remove_mark(st, seat, "rong", &card);
        st.seats[seat].hand.push(card);
    }
    st.push_log(
        Some(&st.seats[seat].player_id.clone()),
        format!("鸿举：用 {} 张手牌替换等量「荣」", hand_cards.len()),
    );
    Ok("鸿举：已替换".to_string())
}

// ---------------------------------------------------------------------------
// 陆抗 · 谦节 / 决堰 / 破势
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JueYanSlot {
    Weapon,
    Armor,
    Horse,
    Treasure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JueYanFrame {
    pub actor: usize,
}

/// 决堰可废除的栏位（未废除）。
pub(crate) fn jueyan_slots(st: &SgsState, actor: usize) -> Vec<(JueYanSlot, EquipSlot)> {
    let mut out = Vec::new();
    let check = |slot: EquipSlot, tag: JueYanSlot, out: &mut Vec<(JueYanSlot, EquipSlot)>| {
        if !st.slot_abolished(actor, slot) {
            out.push((tag, slot));
        }
    };
    check(EquipSlot::Weapon, JueYanSlot::Weapon, &mut out);
    check(EquipSlot::Armor, JueYanSlot::Armor, &mut out);
    if !st.slot_abolished(actor, EquipSlot::HorseOffense) {
        out.push((JueYanSlot::Horse, EquipSlot::HorseOffense));
    } else if !st.slot_abolished(actor, EquipSlot::HorseDefense) {
        out.push((JueYanSlot::Horse, EquipSlot::HorseDefense));
    }
    check(EquipSlot::Treasure, JueYanSlot::Treasure, &mut out);
    out
}

pub(crate) fn step_jueyan(st: &mut SgsState, f: JueYanFrame) -> Result<bool, EngineError> {
    let slots = jueyan_slots(st, f.actor);
    if slots.is_empty() || st.counter("jueyan_used") > 0 {
        pop(st);
        return Ok(false);
    }
    let options: Vec<PendingOption> = slots
        .into_iter()
        .map(|(tag, _)| {
            let label = match tag {
                JueYanSlot::Weapon => "武器栏：本回合【杀】次数 +3",
                JueYanSlot::Armor => "防具栏：摸 3 张且手牌上限 +3",
                JueYanSlot::Horse => "坐骑栏：本回合使用牌无距离限制",
                JueYanSlot::Treasure => "宝物栏：本回合获得「集智」",
            };
            PendingOption {
                id: match tag {
                    JueYanSlot::Weapon => "choose:jueyan:weapon".to_string(),
                    JueYanSlot::Armor => "choose:jueyan:armor".to_string(),
                    JueYanSlot::Horse => "choose:jueyan:horse".to_string(),
                    JueYanSlot::Treasure => "choose:jueyan:treasure".to_string(),
                },
                label: format!("决堰：{label}"),
            }
        })
        .collect();
    let mut options = options;
    options.push(pass_option("不发动决堰"));
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("jueyan".into()),
        options,
        data: json!({ "skill": "jueyan" }),
    });
    replace_top(st, Frame::JueYan(f));
    Ok(true)
}

pub(crate) fn act_jueyan(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::JueYan(f)) => f,
        _ => return Err(internal("栈顶不是决堰帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动决堰".to_string());
    }
    let tag = match action {
        "choose:jueyan:weapon" => JueYanSlot::Weapon,
        "choose:jueyan:armor" => JueYanSlot::Armor,
        "choose:jueyan:horse" => JueYanSlot::Horse,
        "choose:jueyan:treasure" => JueYanSlot::Treasure,
        _ => return Err(illegal("未知决堰选项")),
    };
    let Some((_, slot)) = jueyan_slots(st, f.actor)
        .into_iter()
        .find(|(t, _)| *t == tag)
    else {
        return Err(illegal("该装备栏不可废除"));
    };
    skills::abolish_slot(st, f.actor, slot);
    st.set_counter("jueyan_used", 1);
    let pid = st.seats[f.actor].player_id.clone();
    match tag {
        JueYanSlot::Weapon => {
            st.set_counter("jueyan_sha_bonus", 3);
            st.push_log(Some(&pid), "决堰：废除武器栏，本回合【杀】次数 +3");
        }
        JueYanSlot::Armor => {
            st.set_counter("jueyan_hand_limit", 3);
            let drawn = draw_capped(st, f.actor, 3);
            st.push_log(
                Some(&pid),
                format!("决堰：废除防具栏，摸 {drawn} 张且手牌上限 +3"),
            );
        }
        JueYanSlot::Horse => {
            st.set_counter("jueyan_no_distance", 1);
            st.push_log(Some(&pid), "决堰：废除坐骑栏，本回合使用牌无距离限制");
        }
        JueYanSlot::Treasure => {
            st.set_counter("jueyan_jizhi", 1);
            st.push_log(Some(&pid), "决堰：废除宝物栏，本回合获得「集智」");
        }
    }
    pop(st);
    Ok("决堰：已废除装备栏".to_string())
}

/// 破势（觉醒）：准备阶段装备栏全废除或体力为 1 → 减 1 上限、摸至体力上限、失去决堰、获得怀柔。
pub(crate) fn poshi_awaken(st: &mut SgsState, seat: usize) -> bool {
    if !skills::has(st, seat, "poshi") || st.seats[seat].flags.contains_key("awaken_poshi") {
        return false;
    }
    let all_abolished = st.seats[seat].abolished_slots.len() >= EquipSlot::ALL.len();
    if !all_abolished && st.seats[seat].hp != 1 {
        return false;
    }
    st.seats[seat].flags.insert("awaken_poshi".into(), true);
    st.seats[seat].max_hp = (st.seats[seat].max_hp - 1).max(1);
    st.seats[seat].hp = st.seats[seat].hp.min(st.seats[seat].max_hp);
    skills::lose_skill(st, seat, "jueyan");
    skills::grant_skill(st, seat, "huairou");
    let drawn = draw_to(st, seat, st.seats[seat].max_hp.max(0) as usize);
    st.push_log(
        Some(&st.seats[seat].player_id.clone()),
        format!("破势：觉醒，减少 1 点体力上限、摸 {drawn} 张、获得「怀柔」"),
    );
    true
}

// ---------------------------------------------------------------------------
// 张绣 · 雄乱（限定）/ 从谏
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct XiongLuanFrame {
    pub actor: usize,
}

pub(crate) fn step_xiongluan(st: &mut SgsState, f: XiongLuanFrame) -> Result<bool, EngineError> {
    let options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| *i != f.actor)
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("雄乱：{}", general_name(st, i)),
        })
        .collect();
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "target".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("xiongluan".into()),
        options,
        data: json!({ "stage": "xiongluan" }),
    });
    replace_top(st, Frame::XiongLuan(f));
    Ok(true)
}

pub(crate) fn xiongluan_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::XiongLuan(f)) => f,
        _ => return Err(internal("栈顶不是雄乱帧")),
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
    // 废除判定区：弃置现有判定牌。
    st.seats[f.actor].judge_abolished = true;
    let judge: Vec<String> = std::mem::take(&mut st.seats[f.actor].judge_zone);
    for card in judge {
        turn::to_discard_card(st, &card);
    }
    // 废除全部装备栏。
    for slot in EquipSlot::ALL {
        skills::abolish_slot(st, f.actor, slot);
    }
    st.set_counter("xl_target", (target + 1) as u32);
    st.set_counter(&format!("xl_seat:{}", f.actor), 1);
    st.set_counter(&format!("hand_sealed:{target}"), 1);
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        format!(
            "雄乱：废除判定区与装备区，本回合对 {} 使用牌无距离与次数限制，其不能使用/打出手牌",
            general_name(st, target)
        ),
    );
    Ok("雄乱：已发动".to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CongJianStage {
    Ask,
    Give,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CongJianFrame {
    pub owner: usize,
    pub card: String,
    pub targets: Vec<usize>,
    pub stage: CongJianStage,
}

/// 从谏触发点：成为目标数 >1 的锦囊目标后。
pub(crate) fn congjian_trigger(st: &mut SgsState, card: &str, targets: &[usize]) {
    if targets.len() < 2 {
        return;
    }
    let key = format!("cj:{card}");
    if st.counter(&key) > 0 {
        return;
    }
    st.set_counter(&key, 1);
    for t in targets.iter().copied() {
        if skills::has(st, t, "congjian") && !st.seats[t].hand.is_empty() {
            push(
                st,
                Frame::CongJian(CongJianFrame {
                    owner: t,
                    card: card.to_string(),
                    targets: targets.to_vec(),
                    stage: CongJianStage::Ask,
                }),
            );
        }
    }
}

pub(crate) fn step_congjian(st: &mut SgsState, f: CongJianFrame) -> Result<bool, EngineError> {
    if f.stage == CongJianStage::Give {
        let others: Vec<usize> = f
            .targets
            .iter()
            .copied()
            .filter(|t| *t != f.owner)
            .collect();
        let options: Vec<PendingOption> = others
            .into_iter()
            .map(|t| PendingOption {
                id: format!("target:{}", st.seats[t].player_id),
                label: format!("从谏：交给 {}", general_name(st, t)),
            })
            .collect();
        if options.is_empty() {
            pop(st);
            return Ok(false);
        }
        st.pending = Some(Pending {
            kind: "target".into(),
            actor: st.seats[f.owner].player_id.clone(),
            skill: Some("congjian".into()),
            options,
            data: json!({ "stage": "congjian" }),
        });
        replace_top(st, Frame::CongJian(f));
        return Ok(true);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.owner].player_id.clone(),
        skill: Some("congjian".into()),
        options: vec![
            PendingOption {
                id: "choose:congjian".into(),
                label: "从谏：交给其中一名目标角色一张牌，然后摸 1（装备牌则摸 2）".into(),
            },
            pass_option("不发动从谏"),
        ],
        data: json!({ "skill": "congjian" }),
    });
    replace_top(st, Frame::CongJian(f));
    Ok(true)
}

pub(crate) fn act_congjian(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::CongJian(f)) => f,
        _ => return Err(internal("栈顶不是从谏帧")),
    };
    if action != "choose:congjian" {
        pop(st);
        return Ok("不发动从谏".to_string());
    }
    replace_top(
        st,
        Frame::MultiSelect(MultiSelectFrame {
            actor: f.owner,
            purpose: "congjian".into(),
            selected: Vec::new(),
            min: 1,
            max: 1,
            data: json!({ "card": f.card, "targets": f.targets }),
        }),
    );
    Ok("从谏：请选择要交出的牌".to_string())
}

pub(crate) fn congjian_confirm(
    st: &mut SgsState,
    f: &MultiSelectFrame,
) -> Result<String, EngineError> {
    if f.selected.len() != 1 {
        return Err(illegal("从谏需选择 1 张手牌"));
    }
    let card = f.selected[0].clone();
    let targets: Vec<usize> = f.data["targets"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_u64().map(|n| n as usize))
                .collect()
        })
        .unwrap_or_default();
    let others: Vec<usize> = targets.into_iter().filter(|t| *t != f.actor).collect();
    if others.len() == 1 {
        return congjian_give(st, f.actor, &card, others[0]);
    }
    replace_top(
        st,
        Frame::CongJian(CongJianFrame {
            owner: f.actor,
            card,
            targets: others,
            stage: CongJianStage::Give,
        }),
    );
    Ok("从谏：请选择交牌对象".to_string())
}

pub(crate) fn congjian_target(
    st: &mut SgsState,
    pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
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
    let f = match top(st).cloned() {
        Some(Frame::CongJian(f)) => f,
        _ => return Err(internal("栈顶不是从谏帧")),
    };
    let target = st.seat_index(pid).ok_or_else(|| illegal("目标不在座"))?;
    let _ = pending;
    congjian_give(st, f.owner, &f.card, target)
}

fn congjian_give(
    st: &mut SgsState,
    owner: usize,
    card: &str,
    target: usize,
) -> Result<String, EngineError> {
    if turn::take_card(st, owner, card).is_err() {
        return Err(illegal("手牌中没有该牌"));
    }
    let is_equip = turn::card_def(st, card).is_some_and(|d| d.equip_slot.is_some());
    st.seats[target].hand.push(card.to_string());
    let drawn = draw_capped(st, owner, if is_equip { 2 } else { 1 });
    st.push_log(
        Some(&st.seats[owner].player_id.clone()),
        format!(
            "从谏：交给 {} 【{}】，摸 {drawn} 张",
            general_name(st, target),
            turn::card_label(st, card)
        ),
    );
    Ok("从谏：已完成".to_string())
}

// ---------------------------------------------------------------------------
// 清侧（毌丘俭·鸿举觉醒授予）：移去一张「荣」，弃置场上一张装备/延时锦囊
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QingCeFrame {
    pub actor: usize,
}

fn qingce_candidates(st: &SgsState, actor: usize) -> Vec<(usize, String, bool)> {
    // (座位, 牌 id, 是否装备区)；只允许其他角色的装备牌与判定区延时锦囊。
    let mut out = Vec::new();
    for seat in st.alive_indices() {
        if seat == actor {
            continue;
        }
        for slot in EquipSlot::ALL {
            if let Some(id) = st.seats[seat].equips.get(&slot).cloned().flatten() {
                out.push((seat, id, true));
            }
        }
        for id in &st.seats[seat].judge_zone {
            out.push((seat, id.clone(), false));
        }
    }
    out
}

pub(crate) fn step_qingce(st: &mut SgsState, f: QingCeFrame) -> Result<bool, EngineError> {
    let options: Vec<PendingOption> = qingce_candidates(st, f.actor)
        .into_iter()
        .map(|(seat, id, equip)| PendingOption {
            id: format!("card:{}:{}", st.seats[seat].player_id, id),
            label: format!(
                "{}{}的【{}】",
                st.seats[seat].general_name,
                if equip { "装备区" } else { "判定区" },
                turn::card_label(st, &id)
            ),
        })
        .collect();
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: st.seats[f.actor].player_id.clone(),
        skill: Some("qingce".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::QingCe(f));
    Ok(true)
}

pub(crate) fn act_qingce(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::QingCe(f)) => f,
        _ => return Err(internal("栈顶不是清侧帧")),
    };
    let rest = action
        .strip_prefix("card:")
        .ok_or_else(|| illegal("需要选择要弃置的场牌"))?;
    let (pid, card) = rest
        .split_once(':')
        .ok_or_else(|| illegal("清侧目标格式错误"))?;
    let owner = st.seat_index(pid).ok_or_else(|| illegal("目标不在座"))?;
    if owner == f.actor {
        return Err(illegal("清侧不能弃置自己的牌"));
    }
    let candidates = qingce_candidates(st, f.actor);
    if !candidates
        .iter()
        .any(|(seat, id, _)| *seat == owner && id == card)
    {
        return Err(illegal("该牌不在清侧可选范围"));
    }
    if skills::mark_cards(st, f.actor, "rong").is_empty() {
        return Err(illegal("没有「荣」可移去"));
    }
    let rong = skills::mark_cards(st, f.actor, "rong")
        .into_iter()
        .next()
        .unwrap_or_default();
    if !rong.is_empty() {
        skills::remove_mark(st, f.actor, "rong", &rong);
        turn::discard_by(st, Some(f.actor), &rong);
    }
    // 弃置目标牌：判定区 / 装备区。
    if let Some(pos) = st.seats[owner].judge_zone.iter().position(|c| c == card) {
        st.seats[owner].judge_zone.remove(pos);
    } else {
        let mut removed = false;
        for slot in EquipSlot::ALL {
            if st.seats[owner]
                .equips
                .get(&slot)
                .cloned()
                .flatten()
                .as_deref()
                == Some(card)
            {
                st.seats[owner].equips.insert(slot, None);
                skills::xiaoji_on_lose_equip(st, owner);
                removed = true;
                break;
            }
        }
        if !removed {
            return Err(illegal("目标牌已不在原区域"));
        }
    }
    let label = turn::card_label(st, card);
    turn::discard_by(st, Some(owner), card);
    st.push_log(
        Some(&st.seats[f.actor].player_id.clone()),
        format!(
            "清侧：移去一张「荣」，弃置 {} 的【{label}】",
            st.seats[owner].general_name
        ),
    );
    pop(st);
    Ok("清侧结算完成".to_string())
}
