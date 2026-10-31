//! P4.4.2 一将成名（批次 Y4：8 将）技能帧。
//!
//! 覆盖：谮毁/骄矜（孙鲁班）、奋励/平寇（朱桓）、慎行/秉壹（顾雍）、渐营/矢北（沮授）、
//! 窃听/献州（蔡夫人）、恢拓/明鉴/兴衰（曹叡）、千驹/倾袭（曹休）、活墨/佐定（钟繇）。

use crate::resolve::{
    apply_damage_now, pop, push, push_discard, replace_top, top, DamageType, Frame,
    QuickJudgeFrame, QuickJudgeStage,
};
use crate::state::*;
use crate::turn;
use crate::{illegal, internal};
use game_engine_core::EngineError;
use serde_json::json;

fn pid(st: &SgsState, seat: usize) -> String {
    st.seats[seat].player_id.clone()
}

fn draw_n(st: &mut SgsState, seat: usize, n: u32) -> usize {
    let mut ev = Vec::new();
    turn::draw_cards(st, seat, n, &mut ev)
}

fn cap_hand(st: &mut SgsState, seat: usize) {
    let over = st.seats[seat]
        .hand
        .len()
        .saturating_sub(st.config.max_hand_cards as usize);
    if over > 0 {
        push_discard(st, seat, over, "hand_cap");
    }
}

fn card_type_bit(st: &SgsState, card_id: &str) -> u32 {
    match st
        .cards
        .get(card_id)
        .and_then(|c| crate::data::card_def(&c.def))
        .map(|d| d.category)
    {
        Some(Category::Basic) => 1,
        Some(Category::Trick | Category::DelayedTrick) => 2,
        Some(Category::Weapon | Category::Armor | Category::Horse | Category::Treasure) => 4,
        None => 0,
    }
}

fn equip_count(st: &SgsState, seat: usize) -> usize {
    st.seats[seat].equips.values().flatten().count()
}

fn discard_random_hand(st: &mut SgsState, seat: usize, n: usize, reason: &str) -> bool {
    if st.seats[seat].hand.len() < n {
        return false;
    }
    let mut picked = Vec::new();
    for _ in 0..n {
        let len = st.seats[seat].hand.len();
        let idx = st.next_index(len);
        picked.push(st.seats[seat].hand.remove(idx));
    }
    for card in &picked {
        let label = turn::card_label(st, card);
        turn::discard_by(st, Some(seat), card);
        st.push_log(Some(&pid(st, seat)), format!("{reason}：弃置【{label}】"));
    }
    true
}

// ---------------------------------------------------------------------------
// 谮毁（孙鲁班）：使用【杀】指定唯一目标时，可令另一名角色代用或成为额外目标。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZanHuiStage {
    Helper,
    Choice,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZanHuiFrame {
    pub owner: usize,
    pub user: usize,
    pub target: usize,
    pub card: String,
    pub need: u32,
    pub no_shan: bool,
    pub ignore_armor: bool,
    pub damage_type: DamageType,
    pub stage: ZanHuiStage,
    pub helper: Option<usize>,
}

pub(crate) fn step_zanhui(st: &mut SgsState, f: ZanHuiFrame) -> Result<bool, EngineError> {
    let (options, actor) = match f.stage {
        ZanHuiStage::Helper => (
            st.alive_indices()
                .into_iter()
                .filter(|i| *i != f.owner && *i != f.user && *i != f.target)
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("令 {} 选择", st.seats[i].general_name),
                })
                .collect(),
            f.owner,
        ),
        ZanHuiStage::Choice => {
            let helper = f.helper.ok_or_else(|| internal("谮毁缺少协助者"))?;
            let mut options = Vec::new();
            if turn::has_cards_to_take(st, helper) {
                options.push(PendingOption {
                    id: "opt1".into(),
                    label: format!(
                        "交给 {} 一张牌，代替其成为此牌使用者",
                        st.seats[f.owner].general_name
                    ),
                });
            }
            options.push(PendingOption {
                id: "opt2".into(),
                label: "成为此【杀】的额外目标".into(),
            });
            (options, helper)
        }
    };
    if options.is_empty() {
        pop(st);
        continue_zanhui(st, &f);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, actor),
        skill: Some("zanhui".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::ZanHui(f));
    Ok(true)
}

fn continue_zanhui(st: &mut SgsState, f: &ZanHuiFrame) {
    crate::resolve_skill::push_sha_target_response(
        st,
        f.user,
        f.target,
        &f.card,
        f.need,
        f.no_shan,
        f.ignore_armor,
        f.damage_type,
    );
}

pub(crate) fn act_zanhui(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ZanHui(f)) => f,
        _ => return Err(internal("栈顶不是谮毁帧")),
    };
    match f.stage {
        ZanHuiStage::Helper => {
            let target_pid = action
                .strip_prefix("target:")
                .ok_or_else(|| illegal("需要选择协助角色"))?;
            let helper = st
                .seat_index(target_pid)
                .ok_or_else(|| illegal("目标不在座"))?;
            st.set_counter("zanhui_used", 1);
            st.push_log(
                Some(&pid(st, f.owner)),
                format!("谮毁：令 {} 二选一", st.seats[helper].general_name),
            );
            replace_top(
                st,
                Frame::ZanHui(ZanHuiFrame {
                    stage: ZanHuiStage::Choice,
                    helper: Some(helper),
                    ..f
                }),
            );
            Ok("谮毁：请协助者选择".to_string())
        }
        ZanHuiStage::Choice => {
            let helper = f.helper.ok_or_else(|| internal("谮毁缺少协助者"))?;
            if action == "opt1" {
                if let Some(card) = crate::resolve_jiexian::take_one_from_region(st, helper) {
                    let label = turn::card_label(st, &card);
                    st.seats[f.owner].hand.push(card.clone());
                    st.push_log(
                        Some(&pid(st, helper)),
                        format!(
                            "谮毁：交给 {} 一张牌【{label}】",
                            st.seats[f.owner].general_name
                        ),
                    );
                    cap_hand(st, f.owner);
                }
                // 更改此【杀】的使用者。
                for frame in st.resolution.iter_mut().rev() {
                    if let Frame::Sha(s) = frame {
                        s.user = helper;
                        break;
                    }
                }
                st.push_log(
                    Some(&pid(st, helper)),
                    format!(
                        "谮毁：代替 {} 成为此【杀】的使用者",
                        st.seats[f.user].general_name
                    ),
                );
                let need = if crate::skills::has(st, helper, "wushuang") {
                    2
                } else {
                    1
                };
                pop(st);
                crate::resolve_skill::push_sha_target_response(
                    st,
                    helper,
                    f.target,
                    &f.card,
                    need,
                    f.no_shan,
                    f.ignore_armor,
                    f.damage_type,
                );
                return Ok("谮毁结算完成".to_string());
            }
            if action != "opt2" {
                return Err(illegal("需要选择一项"));
            }
            for frame in st.resolution.iter_mut().rev() {
                if let Frame::Sha(s) = frame {
                    s.pending_targets.push(helper);
                    break;
                }
            }
            st.push_log(
                Some(&pid(st, helper)),
                format!(
                    "谮毁：{} 成为此【杀】的额外目标",
                    st.seats[helper].general_name
                ),
            );
            pop(st);
            continue_zanhui(st, &f);
            Ok("谮毁结算完成".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// 骄矜（孙鲁班）：男性角色对你造成伤害时，可弃一张装备牌令伤害 -1。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JiaoJinFrame {
    pub owner: usize,
    pub amount: i32,
    pub source: Option<usize>,
    pub damage_type: DamageType,
    pub card: Option<String>,
}

pub(crate) fn jiaojin_available(st: &SgsState, owner: usize) -> bool {
    st.seats[owner].hand.iter().any(|id| {
        st.cards
            .get(id)
            .and_then(|c| crate::data::card_def(&c.def))
            .is_some_and(|d| d.equip_slot.is_some())
    }) || st.seats[owner].equips.values().any(|c| c.is_some())
}

pub(crate) fn step_jiaojin(st: &mut SgsState, f: JiaoJinFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = Vec::new();
    for id in turn::hand_ids(st, f.owner) {
        let is_equip = st
            .cards
            .get(&id)
            .and_then(|c| crate::data::card_def(&c.def))
            .is_some_and(|d| d.equip_slot.is_some());
        if is_equip {
            options.push(PendingOption {
                id: format!("equip:{id}"),
                label: format!("弃置手牌 {}", turn::card_label(st, &id)),
            });
        }
    }
    for slot in EquipSlot::ALL {
        if let Some(Some(id)) = st.seats[f.owner].equips.get(&slot) {
            options.push(PendingOption {
                id: format!("equip:{id}"),
                label: format!("弃置装备区 {}", turn::card_label(st, id)),
            });
        }
    }
    options.push(PendingOption {
        id: "pass".into(),
        label: format!("不发动骄矜（受到 {amount} 点伤害）", amount = f.amount),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("jiaojin".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::JiaoJin(f));
    Ok(true)
}

pub(crate) fn act_jiaojin(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::JiaoJin(f)) => f,
        _ => return Err(internal("栈顶不是骄矜帧")),
    };
    if action == "pass" {
        pop(st);
        apply_damage_now(
            st,
            f.owner,
            f.amount,
            f.source,
            f.damage_type,
            f.card.as_deref(),
        );
        return Ok("不发动骄矜".to_string());
    }
    let card = action
        .strip_prefix("equip:")
        .ok_or_else(|| illegal("需要弃置一张装备牌"))?;
    turn::take_card(st, f.owner, card)?;
    let label = turn::card_label(st, card);
    turn::discard_by(st, Some(f.owner), card);
    st.push_log(
        Some(&pid(st, f.owner)),
        format!("骄矜：弃置【{label}】，伤害 -1"),
    );
    pop(st);
    let amount = (f.amount - 1).max(0);
    if amount > 0 {
        apply_damage_now(
            st,
            f.owner,
            amount,
            f.source,
            f.damage_type,
            f.card.as_deref(),
        );
    }
    Ok("骄矜结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 奋励（朱桓）：按条件跳过阶段；平寇：每跳过一个阶段造成 1 点伤害。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FenLiStage {
    Draw,
    Play,
    Discard,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FenLiFrame {
    pub actor: usize,
    pub stage: FenLiStage,
}

pub(crate) fn fenli_condition(st: &SgsState, seat: usize, stage: FenLiStage) -> bool {
    match stage {
        FenLiStage::Draw => {
            let mine = st.seats[seat].hand.len();
            st.alive_indices()
                .into_iter()
                .all(|i| i == seat || st.seats[i].hand.len() <= mine)
        }
        FenLiStage::Play => {
            let mine = st.seats[seat].hp;
            st.alive_indices()
                .into_iter()
                .all(|i| i == seat || st.seats[i].hp <= mine)
        }
        FenLiStage::Discard => {
            let mine = equip_count(st, seat);
            mine > 0
                && st
                    .alive_indices()
                    .into_iter()
                    .all(|i| i == seat || equip_count(st, i) <= mine)
        }
    }
}

pub(crate) fn step_fenli(st: &mut SgsState, f: FenLiFrame) -> Result<bool, EngineError> {
    let label = match f.stage {
        FenLiStage::Draw => "跳过摸牌阶段",
        FenLiStage::Play => "跳过出牌阶段",
        FenLiStage::Discard => "跳过弃牌阶段",
    };
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("fenli".into()),
        options: vec![
            PendingOption {
                id: "opt1".into(),
                label: format!("奋励：{label}（平寇可造成 1 点伤害）"),
            },
            PendingOption {
                id: "pass".into(),
                label: "不跳过".into(),
            },
        ],
        data: json!({ "stage": format!("{:?}", f.stage) }),
    });
    replace_top(st, Frame::FenLi(f));
    Ok(true)
}

pub(crate) fn act_fenli(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::FenLi(f)) => f,
        _ => return Err(internal("栈顶不是奋励帧")),
    };
    pop(st);
    if action != "opt1" {
        return Ok("不发动奋励".to_string());
    }
    let skipped = st.counter("fenli_skipped") + 1;
    st.set_counter("fenli_skipped", skipped);
    match f.stage {
        FenLiStage::Draw => st.set_counter("skip_draw", 1),
        FenLiStage::Play => st.set_counter("skip_play", 1),
        FenLiStage::Discard => st.set_counter("fenli_skip_discard", 1),
    }
    st.push_log(Some(&pid(st, f.actor)), "奋励：跳过阶段");
    Ok("奋励：已跳过阶段".to_string())
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PingKouFrame {
    pub actor: usize,
    pub remaining: u32,
}

pub(crate) fn step_pingkou(st: &mut SgsState, f: PingKouFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| *i != f.actor)
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("对 {} 造成 1 点伤害", st.seats[i].general_name),
        })
        .collect();
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动平寇".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("pingkou".into()),
        options,
        data: json!({ "remaining": f.remaining }),
    });
    replace_top(st, Frame::PingKou(f));
    Ok(true)
}

pub(crate) fn act_pingkou(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::PingKou(f)) => f,
        _ => return Err(internal("栈顶不是平寇帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动平寇".to_string());
    }
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择目标"))?;
    let target = st
        .seat_index(target_pid)
        .ok_or_else(|| illegal("目标不在座"))?;
    st.push_log(
        Some(&pid(st, f.actor)),
        format!("平寇：对 {} 造成 1 点伤害", st.seats[target].general_name),
    );
    let remaining = f.remaining.saturating_sub(1);
    if remaining > 0 {
        replace_top(st, Frame::PingKou(PingKouFrame { remaining, ..f }));
    } else {
        pop(st);
    }
    crate::resolve::apply_damage(st, target, 1, Some(f.actor), DamageType::Normal, None);
    Ok("平寇结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 慎行（顾雍）：弃两张牌摸一张。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ShenXingFrame {
    pub actor: usize,
    pub selected: Vec<String>,
}

pub(crate) fn step_shenxing(st: &mut SgsState, f: ShenXingFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = turn::hand_ids(st, f.actor)
        .into_iter()
        .map(|id| {
            let mark = if f.selected.contains(&id) { "✔ " } else { "" };
            PendingOption {
                id: format!("choose:{id}"),
                label: format!("{mark}{}", turn::card_label(st, &id)),
            }
        })
        .collect();
    if f.selected.len() == 2 {
        options.push(PendingOption {
            id: "confirm".into(),
            label: "确认弃置 2 张，摸 1 张".into(),
        });
    }
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动慎行".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("shenxing".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::ShenXing(f));
    Ok(true)
}

pub(crate) fn act_shenxing(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ShenXing(f)) => f,
        _ => return Err(internal("栈顶不是慎行帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动慎行".to_string());
    }
    if let Some(card) = action.strip_prefix("choose:") {
        if !st.seats[f.actor].hand.iter().any(|c| c == card) {
            return Err(illegal("手牌中没有该牌"));
        }
        let mut selected = f.selected.clone();
        if let Some(pos) = selected.iter().position(|c| c == card) {
            selected.remove(pos);
        } else {
            if selected.len() >= 2 {
                return Err(illegal("慎行只需弃置 2 张"));
            }
            selected.push(card.to_string());
        }
        replace_top(st, Frame::ShenXing(ShenXingFrame { selected, ..f }));
        return Ok("已更新慎行选择".to_string());
    }
    if action != "confirm" || f.selected.len() != 2 {
        return Err(illegal("慎行需要弃置 2 张牌"));
    }
    for card in &f.selected {
        turn::take_card(st, f.actor, card)?;
        turn::discard_by(st, Some(f.actor), card);
    }
    let drawn = draw_n(st, f.actor, 1);
    st.push_log(
        Some(&pid(st, f.actor)),
        format!("慎行：弃置 2 张，摸 {drawn} 张"),
    );
    pop(st);
    Ok("慎行结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 秉壹（顾雍）：自己的牌被弃置后，可展示手牌；颜色均相同则相关角色各摸一张。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BingYiFrame {
    pub owner: usize,
}

pub(crate) fn bingyi_after_discard(st: &mut SgsState, seat: usize) {
    if !crate::skills::has(st, seat, "bingyi") {
        return;
    }
    if st.counter("bingyi_offered") > 0 {
        return;
    }
    st.set_counter("bingyi_offered", 1);
    push(st, Frame::BingYi(BingYiFrame { owner: seat }));
}

pub(crate) fn step_bingyi(st: &mut SgsState, f: BingYiFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("bingyi".into()),
        options: vec![
            PendingOption {
                id: "opt1".into(),
                label: "秉壹：展示所有手牌（颜色均相同则相关角色各摸一张）".into(),
            },
            PendingOption {
                id: "pass".into(),
                label: "不发动秉壹".into(),
            },
        ],
        data: json!({}),
    });
    replace_top(st, Frame::BingYi(f));
    Ok(true)
}

pub(crate) fn act_bingyi(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::BingYi(f)) => f,
        _ => return Err(internal("栈顶不是秉壹帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动秉壹".to_string());
    }
    if action != "opt1" {
        return Err(illegal("需要选择是否发动秉壹"));
    }
    let colors: Vec<bool> = st.seats[f.owner]
        .hand
        .iter()
        .filter_map(|id| st.cards.get(id).map(|c| crate::skills::is_red(c.suit)))
        .collect();
    let same = !colors.is_empty() && colors.iter().all(|c| *c == colors[0]);
    st.push_log(
        Some(&pid(st, f.owner)),
        format!(
            "秉壹：展示所有手牌（{}）",
            if same {
                "颜色均相同"
            } else {
                "颜色不同"
            }
        ),
    );
    if same {
        let x = st.seats[f.owner].hand.len();
        let drawn = draw_n(st, f.owner, 1);
        st.push_log(
            Some(&pid(st, f.owner)),
            format!("秉壹：摸 {drawn} 张，另至多 {x} 名角色各摸 1 张"),
        );
        let targets: Vec<usize> = st
            .alive_indices()
            .into_iter()
            .filter(|i| *i != f.owner)
            .take(x)
            .collect();
        for t in targets {
            let d = draw_n(st, t, 1);
            st.push_log(Some(&pid(st, t)), format!("秉壹：摸 {d} 张"));
            cap_hand(st, t);
        }
    }
    pop(st);
    Ok("秉壹结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 渐营 / 矢北（沮授）
// ---------------------------------------------------------------------------

/// 渐营：使用牌时若与本阶段上一张牌点数或花色相同，摸一张（自动）。
pub(crate) fn jianying_on_use(st: &mut SgsState, seat: usize, card_id: &str) {
    if !crate::skills::has(st, seat, "jianying") {
        return;
    }
    let Some(inst) = st.cards.get(card_id) else {
        return;
    };
    let suit = match inst.suit {
        Suit::Spade => 1u32,
        Suit::Heart => 2,
        Suit::Club => 3,
        Suit::Diamond => 4,
    };
    let rank = inst.rank as u32;
    let prev_suit = st.counter("jy_suit");
    let prev_rank = st.counter("jy_rank");
    st.set_counter("jy_suit", suit);
    st.set_counter("jy_rank", rank);
    if prev_suit == 0 && prev_rank == 0 {
        return;
    }
    if prev_suit == suit || prev_rank == rank {
        let drawn = draw_n(st, seat, 1);
        let label = turn::card_label(st, card_id);
        st.push_log(
            Some(&pid(st, seat)),
            format!("渐营：与上一张【{label}】同花色/点数，摸 {drawn} 张"),
        );
    }
}

/// 矢北（锁定）：每回合首次受伤后回复 1；本回合再受伤则失去 1 点体力。
pub(crate) fn shibei_on_damage(st: &mut SgsState, victim: usize) {
    if !crate::skills::has(st, victim, "shibei") {
        return;
    }
    let hits = st.counter("shibei_hits");
    st.set_counter("shibei_hits", hits + 1);
    if hits == 0 {
        if st.seats[victim].hp < st.seats[victim].max_hp {
            st.seats[victim].hp += 1;
        }
        let hp = st.seats[victim].hp;
        st.push_log(
            Some(&pid(st, victim)),
            format!("矢北：本回合首次受伤，回复 1 点体力（{hp}）"),
        );
    } else {
        st.seats[victim].hp -= 1;
        let hp = st.seats[victim].hp;
        st.push_log(
            Some(&pid(st, victim)),
            format!("矢北：本回合再次受伤，失去 1 点体力（{hp}）"),
        );
        crate::skills::zhaxiang_on_lose_hp(st, victim);
        if hp <= 0 {
            crate::resolve::push_dying(st, victim, None);
        }
    }
}

// ---------------------------------------------------------------------------
// 窃听（蔡夫人）：其他角色回合结束后，若其未对其他角色使用过牌，二选一。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QieTingFrame {
    pub owner: usize,
    pub current: usize,
}

pub(crate) fn qieting_owner(st: &SgsState, current: usize) -> Option<usize> {
    st.alive_indices().into_iter().find(|i| {
        *i != current && crate::skills::has(st, *i, "qieting") && st.counter("targeted_others") == 0
    })
}

pub(crate) fn step_qieting(st: &mut SgsState, f: QieTingFrame) -> Result<bool, EngineError> {
    let mut options = Vec::new();
    let mut has_equip = false;
    for slot in EquipSlot::ALL {
        if let Some(Some(id)) = st.seats[f.current].equips.get(&slot) {
            if st.seats[f.owner]
                .equips
                .get(&slot)
                .cloned()
                .flatten()
                .is_none()
            {
                has_equip = true;
                options.push(PendingOption {
                    id: format!("equip:{id}"),
                    label: format!("获得其装备区的 {}", turn::card_label(st, id)),
                });
            }
        }
    }
    if !has_equip {
        // 没有可放置的空装备槽时，只提供摸牌。
    }
    options.push(PendingOption {
        id: "draw".into(),
        label: "摸一张牌".into(),
    });
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动窃听".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("qieting".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::QieTing(f));
    Ok(true)
}

pub(crate) fn act_qieting(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::QieTing(f)) => f,
        _ => return Err(internal("栈顶不是窃听帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动窃听".to_string());
    }
    if action == "draw" {
        let drawn = draw_n(st, f.owner, 1);
        st.push_log(Some(&pid(st, f.owner)), format!("窃听：摸 {drawn} 张"));
        pop(st);
        return Ok("窃听：摸一张".to_string());
    }
    let card = action
        .strip_prefix("equip:")
        .ok_or_else(|| illegal("需要选择装备牌"))?;
    let slot = EquipSlot::ALL.into_iter().find(|slot| {
        st.seats[f.current]
            .equips
            .get(slot)
            .cloned()
            .flatten()
            .as_deref()
            == Some(card)
    });
    let Some(slot) = slot else {
        return Err(illegal("该牌不在其装备区"));
    };
    if st.seats[f.owner]
        .equips
        .get(&slot)
        .cloned()
        .flatten()
        .is_some()
    {
        return Err(illegal("你的对应装备槽已被占用"));
    }
    st.seats[f.current].equips.insert(slot, None);
    st.seats[f.owner]
        .equips
        .insert(slot, Some(card.to_string()));
    let label = turn::card_label(st, card);
    st.push_log(
        Some(&pid(st, f.owner)),
        format!("窃听：获得装备区的【{label}】"),
    );
    crate::skills::xiaoji_on_lose_equip(st, f.current);
    pop(st);
    Ok("窃听结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 献州（蔡夫人，限定）：交出所有装备牌，令接收者二选一。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum XianZhouStage {
    Target,
    Choice,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct XianZhouFrame {
    pub actor: usize,
    pub stage: XianZhouStage,
    pub receiver: Option<usize>,
    pub x: u32,
}

pub(crate) fn step_xianzhou(st: &mut SgsState, f: XianZhouFrame) -> Result<bool, EngineError> {
    let (options, actor) = match f.stage {
        XianZhouStage::Target => (
            st.alive_indices()
                .into_iter()
                .filter(|i| *i != f.actor)
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("将所有装备牌交给 {}", st.seats[i].general_name),
                })
                .collect(),
            f.actor,
        ),
        XianZhouStage::Choice => {
            let receiver = f.receiver.ok_or_else(|| internal("献州缺少接收者"))?;
            (
                vec![
                    PendingOption {
                        id: "opt1".into(),
                        label: format!("令 {} 回复 {} 点体力", st.seats[f.actor].general_name, f.x),
                    },
                    PendingOption {
                        id: "opt2".into(),
                        label: format!("对其攻击范围内至多 {} 名角色各造成 1 点伤害", f.x),
                    },
                ],
                receiver,
            )
        }
    };
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, actor),
        skill: Some("xianzhou".into()),
        options,
        data: json!({ "x": f.x }),
    });
    replace_top(st, Frame::XianZhou(f));
    Ok(true)
}

pub(crate) fn act_xianzhou(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::XianZhou(f)) => f,
        _ => return Err(internal("栈顶不是献州帧")),
    };
    match f.stage {
        XianZhouStage::Target => {
            let target_pid = action
                .strip_prefix("target:")
                .ok_or_else(|| illegal("需要选择目标"))?;
            let receiver = st
                .seat_index(target_pid)
                .ok_or_else(|| illegal("目标不在座"))?;
            let cards: Vec<String> = EquipSlot::ALL
                .iter()
                .filter_map(|slot| st.seats[f.actor].equips.get(slot).cloned().flatten())
                .collect();
            if cards.is_empty() {
                return Err(illegal("献州需要装备区有牌"));
            }
            for slot in EquipSlot::ALL {
                st.seats[f.actor].equips.insert(slot, None);
            }
            let x = cards.len() as u32;
            for card in cards {
                st.seats[receiver].hand.push(card);
            }
            st.seats[f.actor].flags.insert("xianzhou_used".into(), true);
            st.push_log(
                Some(&pid(st, f.actor)),
                format!(
                    "献州：将 {x} 张装备牌交给 {}",
                    st.seats[receiver].general_name
                ),
            );
            cap_hand(st, receiver);
            replace_top(
                st,
                Frame::XianZhou(XianZhouFrame {
                    stage: XianZhouStage::Choice,
                    receiver: Some(receiver),
                    x,
                    ..f
                }),
            );
            Ok("献州：请接收者选择".to_string())
        }
        XianZhouStage::Choice => {
            let receiver = f.receiver.ok_or_else(|| internal("献州缺少接收者"))?;
            if action == "opt1" {
                let healed = f.x as i32;
                st.seats[f.actor].hp =
                    (st.seats[f.actor].hp + healed).min(st.seats[f.actor].max_hp);
                let hp = st.seats[f.actor].hp;
                st.push_log(
                    Some(&pid(st, receiver)),
                    format!(
                        "献州：令 {} 回复体力至 {hp}",
                        st.seats[f.actor].general_name
                    ),
                );
                pop(st);
                return Ok("献州结算完成".to_string());
            }
            if action != "opt2" {
                return Err(illegal("需要选择一项"));
            }
            let targets: Vec<usize> = st
                .alive_indices()
                .into_iter()
                .filter(|i| *i != receiver && st.in_attack_range(receiver, *i))
                .take(f.x as usize)
                .collect();
            st.push_log(
                Some(&pid(st, receiver)),
                format!("献州：对 {} 名角色各造成 1 点伤害", targets.len()),
            );
            pop(st);
            for t in targets {
                crate::resolve::apply_damage(st, t, 1, Some(receiver), DamageType::Normal, None);
            }
            Ok("献州结算完成".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// 恢拓（曹叡）：受到伤害后可令一名角色判定。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HuiTuoFrame {
    pub owner: usize,
    pub amount: i32,
}

pub(crate) fn step_huituo(st: &mut SgsState, f: HuiTuoFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("令 {} 判定", st.seats[i].general_name),
        })
        .collect();
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动恢拓".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("huituo".into()),
        options,
        data: json!({ "amount": f.amount }),
    });
    replace_top(st, Frame::HuiTuo(f));
    Ok(true)
}

pub(crate) fn act_huituo(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::HuiTuo(f)) => f,
        _ => return Err(internal("栈顶不是恢拓帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动恢拓".to_string());
    }
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择目标"))?;
    let target = st
        .seat_index(target_pid)
        .ok_or_else(|| illegal("目标不在座"))?;
    pop(st);
    push(
        st,
        Frame::QuickJudge(QuickJudgeFrame {
            owner: f.owner,
            reason: "huituo".into(),
            stage: QuickJudgeStage::Reveal,
            judge_card: None,
            rewrite_order: Vec::new(),
            rewrite_cursor: 0,
            data: json!({ "target": target, "amount": f.amount }),
            saved_outcome: None,
        }),
    );
    Ok("恢拓：判定中".to_string())
}

// ---------------------------------------------------------------------------
// 明鉴（曹叡）：交给一名其他角色所有手牌；其下回合手牌上限与【杀】次数 +1。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MingJianFrame {
    pub actor: usize,
}

pub(crate) fn step_mingjian(st: &mut SgsState, f: MingJianFrame) -> Result<bool, EngineError> {
    let options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| *i != f.actor)
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("将所有手牌交给 {}", st.seats[i].general_name),
        })
        .collect();
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("mingjian".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::MingJian(f));
    Ok(true)
}

pub(crate) fn act_mingjian(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::MingJian(f)) => f,
        _ => return Err(internal("栈顶不是明鉴帧")),
    };
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择目标"))?;
    let target = st
        .seat_index(target_pid)
        .ok_or_else(|| illegal("目标不在座"))?;
    let cards = std::mem::take(&mut st.seats[f.actor].hand);
    let n = cards.len();
    for card in cards {
        st.seats[target].hand.push(card);
    }
    st.seats[target].flags.insert("mingjian".into(), true);
    st.set_counter("mingjian_used", 1);
    st.push_log(
        Some(&pid(st, f.actor)),
        format!(
            "明鉴：交给 {} {n} 张手牌，其下回合手牌上限与【杀】次数 +1",
            st.seats[target].general_name
        ),
    );
    cap_hand(st, target);
    pop(st);
    Ok("明鉴结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 兴衰（曹叡，主公技 / 限定）：濒死时其他魏势力角色回复其体力。
// ---------------------------------------------------------------------------

pub(crate) fn xingshuai_on_dying(st: &mut SgsState, victim: usize) -> bool {
    if !crate::skills::has(st, victim, "xingshuai")
        || !crate::skills::is_lord(st, victim)
        || st.seats[victim].flags.contains_key("xingshuai_used")
        || st.seats[victim].hp > 0
    {
        return false;
    }
    let helpers: Vec<usize> = st
        .alive_indices()
        .into_iter()
        .filter(|i| {
            *i != victim
                && st.seats[*i]
                    .general
                    .as_deref()
                    .and_then(crate::data::general)
                    .is_some_and(|g| g.kingdom == "wei")
        })
        .collect();
    if helpers.is_empty() {
        return false;
    }
    st.seats[victim].flags.insert("xingshuai_used".into(), true);
    for h in &helpers {
        if st.seats[victim].hp < st.seats[victim].max_hp {
            st.seats[victim].hp += 1;
        }
        st.push_log(
            Some(&pid(st, *h)),
            format!(
                "兴衰：{} 回复 1 点体力（{}）",
                st.seats[victim].general_name, st.seats[victim].hp
            ),
        );
    }
    for h in helpers {
        st.seats[h].hp -= 1;
        let hp = st.seats[h].hp;
        st.push_log(Some(&pid(st, h)), format!("兴衰：失去 1 点体力（{hp}）"));
        crate::skills::zhaxiang_on_lose_hp(st, h);
        if hp <= 0 {
            crate::resolve::push_dying(st, h, None);
            return st.seats[victim].hp > 0;
        }
    }
    st.seats[victim].hp > 0
}

// ---------------------------------------------------------------------------
// 倾袭（曹休）：【杀】造成伤害时令目标二选一。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QingXiFrame {
    pub user: usize,
    pub target: usize,
    pub card: String,
    pub damage: i32,
    pub damage_type: DamageType,
    pub range: u32,
}

pub(crate) fn qingxi_available(st: &SgsState, user: usize) -> Option<u32> {
    if !crate::skills::has(st, user, "qingxi") {
        return None;
    }
    st.equip_def(user, EquipSlot::Weapon)
        .and_then(|d| d.range_value)
        .map(|v| v.max(0) as u32)
}

pub(crate) fn step_qingxi(st: &mut SgsState, f: QingXiFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.target),
        skill: Some("qingxi".into()),
        options: vec![
            PendingOption {
                id: "opt1".into(),
                label: format!("弃置 {} 张手牌，然后弃置其武器", f.range),
            },
            PendingOption {
                id: "opt2".into(),
                label: "令此【杀】伤害 +1".into(),
            },
        ],
        data: json!({}),
    });
    replace_top(st, Frame::QingXi(f));
    Ok(true)
}

pub(crate) fn act_qingxi(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::QingXi(f)) => f,
        _ => return Err(internal("栈顶不是倾袭帧")),
    };
    pop(st);
    if action == "opt1" {
        let n = f.range as usize;
        if st.seats[f.target].hand.len() < n {
            return Err(illegal("手牌不足，不能选择弃牌"));
        }
        discard_random_hand(st, f.target, n, "倾袭");
        if let Some(Some(weapon)) = st.seats[f.user].equips.get(&EquipSlot::Weapon).cloned() {
            st.seats[f.user].equips.insert(EquipSlot::Weapon, None);
            turn::discard_by(st, Some(f.user), &weapon);
            crate::skills::xiaoji_on_lose_equip(st, f.user);
            let label = turn::card_label(st, &weapon);
            st.push_log(
                Some(&pid(st, f.target)),
                format!("倾袭：弃置其武器【{label}】"),
            );
        }
        apply_damage_now(
            st,
            f.target,
            f.damage,
            Some(f.user),
            f.damage_type,
            Some(&f.card),
        );
        return Ok("倾袭：弃牌并失去武器".to_string());
    }
    if action != "opt2" {
        return Err(illegal("需要选择一项"));
    }
    st.push_log(
        Some(&pid(st, f.target)),
        format!("倾袭：令此【杀】伤害 +1（{}）", f.damage + 1),
    );
    apply_damage_now(
        st,
        f.target,
        f.damage + 1,
        Some(f.user),
        f.damage_type,
        Some(&f.card),
    );
    Ok("倾袭：伤害 +1".to_string())
}

// ---------------------------------------------------------------------------
// 活墨（钟繇）：黑色非基本牌置于牌堆顶，视为使用一张不同牌名的基本牌。
// ---------------------------------------------------------------------------

/// 本回合已使用过的基本牌位图（sha=1 / tao=2 / jiu=4 / shan=8）。
pub(crate) fn note_basic_used(st: &mut SgsState, seat: usize, code: &str) {
    if !crate::skills::has(st, seat, "huomo") {
        return;
    }
    let bit = match code {
        "sha" | "huo_sha" | "lei_sha" => 1u32,
        "tao" => 2,
        "jiu" => 4,
        "shan" => 8,
        _ => return,
    };
    let used = st.counter("huomo_used") | bit;
    st.set_counter("huomo_used", used);
}

/// 活墨可用选项：(黑色非基本牌, 基本牌 code)。
pub(crate) fn huomo_options(st: &SgsState, seat: usize) -> Vec<(String, &'static str)> {
    if !crate::skills::has(st, seat, "huomo") {
        return Vec::new();
    }
    let used = st.counter("huomo_used");
    let basics = [("sha", 1u32), ("tao", 2), ("jiu", 4)];
    let mut out = Vec::new();
    for card in turn::hand_ids(st, seat) {
        let Some(inst) = st.cards.get(&card) else {
            continue;
        };
        if !crate::skills::is_black(inst.suit) || card_type_bit(st, &card) == 1 {
            continue;
        }
        for (code, bit) in basics {
            if used & bit != 0 {
                continue;
            }
            if code == "tao" && st.seats[seat].hp >= st.seats[seat].max_hp {
                continue;
            }
            if code == "jiu" && st.counter("jiu_used") > 0 {
                continue;
            }
            out.push((card.clone(), code));
        }
    }
    out
}

/// 活墨：执行一次转化（返回提示文本）。
pub(crate) fn use_huomo(
    st: &mut SgsState,
    seat: usize,
    card: &str,
    code: &str,
) -> Result<String, EngineError> {
    if !huomo_options(st, seat)
        .iter()
        .any(|(c, b)| c == card && *b == code)
    {
        return Err(illegal("活墨：该组合不可用"));
    }
    let label = turn::card_label(st, card);
    turn::take_card(st, seat, card)?;
    st.deck.draw.push(card.to_string());
    st.set_counter(
        "huomo_used",
        st.counter("huomo_used")
            | match code {
                "sha" => 1,
                "tao" => 2,
                _ => 4,
            },
    );
    match code {
        "sha" => {
            if turn::sha_targets(st, seat).is_empty() {
                return Err(illegal("活墨：攻击范围内没有目标"));
            }
            if st.counter("sha_used") >= turn::sha_limit(st, seat) {
                return Err(illegal("活墨：本阶段已使用过【杀】"));
            }
            let fangtian = crate::skills::has_equipment_skill(st, seat, "fangtian_huaji")
                && st.seats[seat].hand.is_empty();
            st.bump_counter("sha_used");
            if st.turn == seat {
                st.bump_counter("play_cards_used");
            }
            push(
                st,
                Frame::Sha(crate::resolve::ShaFrame {
                    card: String::new(),
                    user: seat,
                    target: None,
                    stage: crate::resolve::ShaStage::Target,
                    pending_targets: Vec::new(),
                    fangtian,
                    damage_type: DamageType::Normal,
                    no_distance: false,
                    virtual_card: true,
                    tianyi: false,
                    no_respond: false,
                }),
            );
            st.push_log(
                Some(&pid(st, seat)),
                format!("活墨：将【{label}】置于牌堆顶，视为使用【杀】"),
            );
            Ok("活墨：请选择【杀】的目标".to_string())
        }
        "tao" => {
            st.seats[seat].hp = (st.seats[seat].hp + 1).min(st.seats[seat].max_hp);
            let hp = st.seats[seat].hp;
            st.push_log(
                Some(&pid(st, seat)),
                format!("活墨：将【{label}】置于牌堆顶，视为使用【桃】（体力 {hp}）"),
            );
            Ok("活墨：回复 1 点体力".to_string())
        }
        _ => {
            st.set_counter("jiu_used", 1);
            st.set_counter("jiu", 1);
            st.push_log(
                Some(&pid(st, seat)),
                format!("活墨：将【{label}】置于牌堆顶，视为使用【酒】"),
            );
            Ok("活墨：已使用【酒】".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// 佐定（钟繇）：其他角色出牌阶段使用黑桃牌指定目标后，若本阶段无伤害，令其摸 1。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZuoDingFrame {
    pub owner: usize,
    pub targets: Vec<usize>,
}

pub(crate) fn zuoding_owner(st: &SgsState, user: usize) -> Option<usize> {
    if st.counter("phase_damage") > 0 {
        return None;
    }
    st.alive_indices()
        .into_iter()
        .find(|i| *i != user && crate::skills::has(st, *i, "zuoding"))
}

pub(crate) fn step_zuoding(st: &mut SgsState, f: ZuoDingFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = f
        .targets
        .iter()
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[*i].player_id),
            label: format!("令 {} 摸一张牌", st.seats[*i].general_name),
        })
        .collect();
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动佐定".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("zuoding".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::ZuoDing(f));
    Ok(true)
}

pub(crate) fn act_zuoding(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ZuoDing(f)) => f,
        _ => return Err(internal("栈顶不是佐定帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动佐定".to_string());
    }
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择目标"))?;
    let target = st
        .seat_index(target_pid)
        .ok_or_else(|| illegal("目标不在座"))?;
    let drawn = draw_n(st, target, 1);
    st.push_log(
        Some(&pid(st, f.owner)),
        format!("佐定：令 {} 摸 {drawn} 张", st.seats[target].general_name),
    );
    pop(st);
    Ok("佐定结算完成".to_string())
}

/// 佐定触发（【杀】指定目标后 / 锦囊目标结算时调用）。
pub(crate) fn zuoding_trigger(st: &mut SgsState, user: usize, card: &str, target: usize) {
    if user == target {
        return;
    }
    let spade = st.cards.get(card).is_some_and(|c| c.suit == Suit::Spade);
    if !spade {
        return;
    }
    let Some(owner) = zuoding_owner(st, user) else {
        return;
    };
    push(
        st,
        Frame::ZuoDing(ZuoDingFrame {
            owner,
            targets: vec![target],
        }),
    );
}
