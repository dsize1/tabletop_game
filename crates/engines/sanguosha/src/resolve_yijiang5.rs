//! P4.4.2 一将成名（批次 Y3：8 将）技能帧。
//!
//! 覆盖：贞烈/秘计（王异）、父魂（关兴张苞）、司敌（曹真）、慎断/勇略（韩浩史涣）、
//! 品第/法恩（陈群）、奔袭（吴懿）、忠勇（周仓）、强识/献图（张松）。

use crate::resolve::{
    pop, push, push_discard, replace_top, top, DamageType, Frame, Outcome, ShaFrame, ShaStage,
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

pub(crate) fn card_type_bit(st: &SgsState, card_id: &str) -> u32 {
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

/// 从牌堆顶摸牌并放入手牌（复用 `draw_n`）。
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
// 贞烈（王异）：成为【杀】/普通锦囊目标后，可失去 1 点体力令此牌无效并弃使用者一张牌。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZhenLieKind {
    Sha,
    Trick,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZhenLieFrame {
    pub owner: usize,
    pub user: usize,
    pub card: String,
    pub kind: ZhenLieKind,
    // 【杀】的响应参数（pass 后继续正常响应）。
    pub need: u32,
    pub no_shan: bool,
    pub ignore_armor: bool,
    pub damage_type: DamageType,
}

pub(crate) fn step_zhenlie(st: &mut SgsState, f: ZhenLieFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("zhenlie".into()),
        options: vec![
            PendingOption {
                id: "opt1".into(),
                label: format!(
                    "贞烈：失去 1 点体力，令【{}】对你无效并弃置使用者一张牌",
                    turn::card_label(st, &f.card)
                ),
            },
            PendingOption {
                id: "pass".into(),
                label: "不发动贞烈".into(),
            },
        ],
        data: json!({ "kind": format!("{:?}", f.kind) }),
    });
    replace_top(st, Frame::ZhenLie(f));
    Ok(true)
}

pub(crate) fn act_zhenlie(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ZhenLie(f)) => f,
        _ => return Err(internal("栈顶不是贞烈帧")),
    };
    if action == "pass" {
        crate::skills::set_zhenlie_skip(st, &f.card, f.owner);
        pop(st);
        if f.kind == ZhenLieKind::Sha {
            crate::resolve_skill::push_sha_target_response(
                st,
                f.user,
                f.owner,
                &f.card,
                f.need,
                f.no_shan,
                f.ignore_armor,
                f.damage_type,
            );
        }
        return Ok("不发动贞烈".to_string());
    }
    if action != "opt1" {
        return Err(illegal("需要选择是否发动贞烈"));
    }
    st.seats[f.owner].hp -= 1;
    let hp = st.seats[f.owner].hp;
    crate::skills::zhaxiang_on_lose_hp(st, f.owner);
    crate::skills::set_zhenlie_void(st, &f.card, f.owner);
    if crate::skills::is_jiexian(st, f.owner) {
        // 界贞烈（界王异）：无效后二选一（获得使用者一张牌 / 本回合额外秘计）。
        st.push_log(
            Some(&pid(st, f.owner)),
            format!("贞烈：令此牌无效（体力 {hp}）"),
        );
        pop(st);
        if f.kind == ZhenLieKind::Sha {
            st.outcome = Some(Outcome::Respond {
                played: None,
                played_by: None,
            });
        }
        crate::resolve::push(
            st,
            Frame::ZhenLieJie(crate::resolve_jiexian7::ZhenLieJieFrame {
                owner: f.owner,
                user: f.user,
            }),
        );
        if hp <= 0 {
            crate::resolve::push_dying(st, f.owner, Some(f.user));
        }
        return Ok("贞烈：此牌对王异无效".to_string());
    }
    let stolen = crate::resolve_jiexian::take_one_from_region(st, f.user);
    if let Some(card) = &stolen {
        let label = turn::card_label(st, card);
        turn::discard_by(st, Some(f.user), card);
        st.push_log(
            Some(&pid(st, f.owner)),
            format!("贞烈：令此牌无效并弃置【{label}】（体力 {hp}）"),
        );
    } else {
        st.push_log(
            Some(&pid(st, f.owner)),
            format!("贞烈：令此牌无效（体力 {hp}）"),
        );
    }
    pop(st);
    if f.kind == ZhenLieKind::Sha {
        // 视为已被【闪】响应：`Sha WaitRespond` 会跳过该目标并继续后续目标。
        st.outcome = Some(Outcome::Respond {
            played: None,
            played_by: None,
        });
    }
    if hp <= 0 {
        crate::resolve::push_dying(st, f.owner, Some(f.user));
    }
    Ok("贞烈：此牌对王异无效".to_string())
}

// ---------------------------------------------------------------------------
// 秘计（王异）：结束阶段摸 X（已损失体力），然后可交给其他角色 X 张手牌。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MiJiStage {
    Select,
    Target,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MiJiFrame {
    pub actor: usize,
    pub stage: MiJiStage,
    pub x: u32,
    pub selected: Vec<String>,
}

pub(crate) fn step_miji(st: &mut SgsState, f: MiJiFrame) -> Result<bool, EngineError> {
    let (options, actor) = match f.stage {
        MiJiStage::Select => {
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
            if !f.selected.is_empty() {
                options.push(PendingOption {
                    id: "confirm".into(),
                    label: format!("确认交给（{} 张）", f.selected.len()),
                });
            }
            options.push(PendingOption {
                id: "pass".into(),
                label: "不交给其他角色".into(),
            });
            (options, f.actor)
        }
        MiJiStage::Target => (
            st.alive_indices()
                .into_iter()
                .filter(|i| *i != f.actor)
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!(
                        "交给 {}（{} 张）",
                        st.seats[i].general_name,
                        f.selected.len()
                    ),
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
        skill: Some("miji".into()),
        options,
        data: json!({ "x": f.x }),
    });
    replace_top(st, Frame::MiJi(f));
    Ok(true)
}

pub(crate) fn act_miji(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::MiJi(f)) => f,
        _ => return Err(internal("栈顶不是秘计帧")),
    };
    match f.stage {
        MiJiStage::Select => {
            if action == "pass" {
                pop(st);
                return Ok("秘计：不交给其他角色".to_string());
            }
            if let Some(card) = action.strip_prefix("choose:") {
                if !st.seats[f.actor].hand.iter().any(|c| c == card) {
                    return Err(illegal("手牌中没有该牌"));
                }
                let mut selected = f.selected.clone();
                if let Some(pos) = selected.iter().position(|c| c == card) {
                    selected.remove(pos);
                } else {
                    if selected.len() >= f.x as usize {
                        return Err(illegal(format!("秘计最多交给 {} 张", f.x)));
                    }
                    selected.push(card.to_string());
                }
                replace_top(st, Frame::MiJi(MiJiFrame { selected, ..f }));
                return Ok("已更新秘计选择".to_string());
            }
            if action != "confirm" || f.selected.is_empty() {
                return Err(illegal("请选择至少 1 张手牌"));
            }
            replace_top(
                st,
                Frame::MiJi(MiJiFrame {
                    stage: MiJiStage::Target,
                    ..f
                }),
            );
            Ok("秘计：请选择交给的角色".to_string())
        }
        MiJiStage::Target => {
            let target_pid = action
                .strip_prefix("target:")
                .ok_or_else(|| illegal("需要选择目标"))?;
            let target = st
                .seat_index(target_pid)
                .ok_or_else(|| illegal("目标不在座"))?;
            let n = f.selected.len();
            for card in &f.selected {
                turn::take_card(st, f.actor, card)?;
                st.seats[target].hand.push(card.clone());
            }
            st.push_log(
                Some(&pid(st, f.actor)),
                format!("秘计：交给 {} {n} 张手牌", st.seats[target].general_name),
            );
            cap_hand(st, target);
            pop(st);
            Ok("秘计结算完成".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// 父魂（关兴张苞）：两张手牌当【杀】；出牌阶段以此法造成伤害后获得武圣/咆哮。
// ---------------------------------------------------------------------------

/// 父魂：使用两张手牌当【杀】（主动）。
pub(crate) fn use_fuhen_sha(
    st: &mut SgsState,
    seat: usize,
    c1: &str,
    c2: &str,
) -> Result<String, EngineError> {
    if !crate::skills::has(st, seat, "fuhen") {
        return Err(illegal("没有该技能"));
    }
    if c1 == c2 {
        return Err(illegal("父魂需要两张不同的手牌"));
    }
    for card in [c1, c2] {
        if !st.seats[seat].hand.iter().any(|c| c == card) {
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
    turn::take_card(st, seat, c2)?;
    turn::to_discard_card(st, c2);
    turn::take_card(st, seat, c1)?;
    st.set_counter(&format!("fuhen:{c1}"), 1);
    let fangtian = crate::skills::has_equipment_skill(st, seat, "fangtian_huaji")
        && st.seats[seat].hand.is_empty();
    crate::resolve::start_sha_flags(st, seat, c1, fangtian, DamageType::Normal, false, false)?;
    st.push_log(
        Some(&pid(st, seat)),
        format!("父魂：将【{label1}】【{label2}】当【杀】使用"),
    );
    Ok("请选择【杀】的目标".to_string())
}

/// 父魂：两张手牌当【杀】打出（响应）。
pub(crate) fn fuhen_respond_options(st: &SgsState, actor: usize) -> Vec<PendingOption> {
    if !crate::skills::has(st, actor, "fuhen") {
        return Vec::new();
    }
    let hand: Vec<String> = st.seats[actor].hand.clone();
    let mut out = Vec::new();
    for i in 0..hand.len() {
        for j in (i + 1)..hand.len() {
            out.push(PendingOption {
                id: format!("respond:{},{}", hand[i], hand[j]),
                label: format!(
                    "父魂：将【{}】【{}】当【杀】打出",
                    turn::card_label(st, &hand[i]),
                    turn::card_label(st, &hand[j])
                ),
            });
        }
    }
    out
}

/// 父魂：以此法造成伤害后，本回合获得武圣/咆哮（下回合开始移除）。
pub(crate) fn fuhen_on_damage(st: &mut SgsState, source: usize, card: &str) {
    let key = format!("fuhen:{card}");
    if st.counter(&key) == 0 || !crate::skills::has(st, source, "fuhen") {
        return;
    }
    st.set_counter(&key, 0);
    crate::skills::grant_skill(st, source, "wusheng");
    crate::skills::grant_skill(st, source, "paoxiao");
    st.seats[source].flags.insert("fuhen_granted".into(), true);
    st.push_log(
        Some(&pid(st, source)),
        "父魂：以此法造成伤害，本回合获得「武圣」「咆哮」",
    );
}

// ---------------------------------------------------------------------------
// 司敌（曹真）：其他角色出牌阶段开始时可弃同色非基本牌封锁颜色。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SiDiFrame {
    pub owner: usize,
    pub current: usize,
}

/// 司敌可用费用：与曹真装备区任一牌同色的非基本手牌。
pub(crate) fn sidi_costs(st: &SgsState, owner: usize) -> Vec<String> {
    let equips: Vec<bool> = EquipSlot::ALL
        .iter()
        .filter_map(|slot| {
            st.seats[owner]
                .equips
                .get(slot)
                .and_then(|c| c.as_ref())
                .and_then(|id| st.cards.get(id))
                .map(|c| crate::skills::is_red(c.suit))
        })
        .collect();
    if equips.is_empty() {
        return Vec::new();
    }
    turn::hand_ids(st, owner)
        .into_iter()
        .filter(|id| {
            let not_basic = card_type_bit(st, id) != 1;
            not_basic
                && st.cards.get(id).is_some_and(|c| {
                    let red = crate::skills::is_red(c.suit);
                    equips.contains(&red)
                })
        })
        .collect()
}

pub(crate) fn step_sidi(st: &mut SgsState, f: SiDiFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = sidi_costs(st, f.owner)
        .into_iter()
        .map(|id| PendingOption {
            id: format!("choose:{id}"),
            label: format!(
                "弃置 {} 封锁 {} 的颜色",
                turn::card_label(st, &id),
                st.seats[f.current].general_name
            ),
        })
        .collect();
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动司敌".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("sidi".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::SiDi(f));
    Ok(true)
}

pub(crate) fn act_sidi(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::SiDi(f)) => f,
        _ => return Err(internal("栈顶不是司敌帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动司敌".to_string());
    }
    let card = action
        .strip_prefix("choose:")
        .ok_or_else(|| illegal("需要选择一张非基本牌"))?;
    if !sidi_costs(st, f.owner).contains(&card.to_string()) {
        return Err(illegal("该牌不满足司敌费用"));
    }
    let red = st
        .cards
        .get(card)
        .is_some_and(|c| crate::skills::is_red(c.suit));
    let label = turn::card_label(st, card);
    turn::take_card(st, f.owner, card)?;
    turn::discard_by(st, Some(f.owner), card);
    let bit = if red { 1 } else { 2 };
    st.set_counter(&format!("sidiqu:{}", f.current), bit);
    st.set_counter("sidi_owner", f.owner as u32 + 1);
    st.set_counter("sidi_target", f.current as u32 + 1);
    st.push_log(
        Some(&pid(st, f.owner)),
        format!(
            "司敌：弃置【{label}】，{} 本阶段不能使用/打出{}色牌",
            st.seats[f.current].general_name,
            if red { "红" } else { "黑" }
        ),
    );
    pop(st);
    Ok("司敌结算完成".to_string())
}

/// 司敌：该角色出牌阶段结束时，若未使用过【杀】，曹真视为对其使用一张【杀】。
pub(crate) fn sidi_end_phase(st: &mut SgsState, current: usize) {
    let owner_plus = st.counter("sidi_owner");
    if owner_plus == 0 {
        return;
    }
    let owner = (owner_plus - 1) as usize;
    let target_plus = st.counter("sidi_target");
    st.set_counter("sidi_owner", 0);
    st.set_counter("sidi_target", 0);
    if target_plus == 0 || (target_plus - 1) as usize != current {
        return;
    }
    if !st.seats[owner].alive || !st.seats[current].alive {
        return;
    }
    if st.counter("sha_used") > 0 {
        st.push_log(
            Some(&pid(st, owner)),
            format!(
                "司敌：{} 本阶段使用过【杀】，不触发",
                st.seats[current].general_name
            ),
        );
        return;
    }
    st.push_log(
        Some(&pid(st, owner)),
        format!(
            "司敌：{} 未使用【杀】，视为对其使用【杀】",
            st.seats[current].general_name
        ),
    );
    push(
        st,
        Frame::Sha(ShaFrame {
            card: String::new(),
            user: owner,
            target: Some(current),
            stage: ShaStage::WaitRespond,
            pending_targets: Vec::new(),
            fangtian: false,
            damage_type: DamageType::Normal,
            no_distance: true,
            virtual_card: true,
            tianyi: false,
            no_respond: false,
        }),
    );
    crate::resolve_skill::push_sha_target_response(
        st,
        owner,
        current,
        "",
        1,
        false,
        false,
        DamageType::Normal,
    );
}

// ---------------------------------------------------------------------------
// 慎断（韩浩史涣）：黑色基本牌被弃置后，可当无距离【兵粮寸断】使用。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ShanDuanFrame {
    pub owner: usize,
    pub card: String,
}

pub(crate) fn step_shenduan(st: &mut SgsState, f: ShanDuanFrame) -> Result<bool, EngineError> {
    if !st.deck.discard.iter().any(|c| c == &f.card) {
        pop(st);
        return Ok(false);
    }
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("shenduan".into()),
        options: vec![
            PendingOption {
                id: "opt1".into(),
                label: format!(
                    "慎断：将【{}】当无距离限制的【兵粮寸断】使用",
                    turn::card_label(st, &f.card)
                ),
            },
            PendingOption {
                id: "pass".into(),
                label: "不发动慎断".into(),
            },
        ],
        data: json!({}),
    });
    replace_top(st, Frame::ShanDuan(f));
    Ok(true)
}

pub(crate) fn act_shenduan(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ShanDuan(f)) => f,
        _ => return Err(internal("栈顶不是慎断帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动慎断".to_string());
    }
    if action != "opt1" {
        return Err(illegal("需要选择是否发动慎断"));
    }
    let Some(pos) = st.deck.discard.iter().position(|c| c == &f.card) else {
        return Err(illegal("该牌已不在弃牌堆"));
    };
    st.deck.discard.remove(pos);
    st.deck.processing.push(f.card.clone());
    pop(st);
    crate::resolve::start_virtual_trick(st, f.owner, &f.card, "bingliang_cunduan")?;
    st.push_log(
        Some(&pid(st, f.owner)),
        format!(
            "慎断：将【{}】当【兵粮寸断】使用",
            turn::card_label(st, &f.card)
        ),
    );
    Ok("慎断：请选择兵粮寸断目标".to_string())
}

// ---------------------------------------------------------------------------
// 勇略（韩浩史涣）：判定阶段开始时可弃置判定牌视为【杀】；未造成伤害摸 1。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct YongLueFrame {
    pub owner: usize,
    pub current: usize,
}

pub(crate) fn yonglue_owner(st: &SgsState, current: usize) -> Option<usize> {
    st.alive_indices().into_iter().find(|i| {
        *i != current
            && crate::skills::has(st, *i, "yonglue")
            && st.in_attack_range(*i, current)
            && !st.seats[current].judge_zone.is_empty()
    })
}

pub(crate) fn step_yonglue(st: &mut SgsState, f: YongLueFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = st.seats[f.current]
        .judge_zone
        .iter()
        .map(|id| PendingOption {
            id: format!("choose:{id}"),
            label: format!(
                "弃置 {} 的判定牌【{}】视为对其使用【杀】",
                st.seats[f.current].general_name,
                turn::card_label(st, id)
            ),
        })
        .collect();
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动勇略".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("yonglue".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::YongLue(f));
    Ok(true)
}

pub(crate) fn act_yonglue(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::YongLue(f)) => f,
        _ => return Err(internal("栈顶不是勇略帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动勇略".to_string());
    }
    let card = action
        .strip_prefix("choose:")
        .ok_or_else(|| illegal("需要选择判定区的牌"))?;
    let Some(pos) = st.seats[f.current]
        .judge_zone
        .iter()
        .position(|c| c == card)
    else {
        return Err(illegal("判定区没有该牌"));
    };
    st.seats[f.current].judge_zone.remove(pos);
    turn::discard_by(st, Some(f.current), card);
    st.set_counter("yonglue_owner", f.owner as u32 + 1);
    st.set_counter("yonglue_hp", st.seats[f.current].hp.max(0) as u32);
    st.push_log(
        Some(&pid(st, f.owner)),
        format!(
            "勇略：弃置 {} 的判定牌【{}】，视为对其使用【杀】",
            st.seats[f.current].general_name,
            turn::card_label(st, card)
        ),
    );
    pop(st);
    push(
        st,
        Frame::Sha(ShaFrame {
            card: card.to_string(),
            user: f.owner,
            target: Some(f.current),
            stage: ShaStage::WaitRespond,
            pending_targets: Vec::new(),
            fangtian: false,
            damage_type: DamageType::Normal,
            no_distance: true,
            virtual_card: true,
            tianyi: false,
            no_respond: false,
        }),
    );
    crate::resolve_skill::push_sha_target_response(
        st,
        f.owner,
        f.current,
        card,
        1,
        false,
        false,
        DamageType::Normal,
    );
    Ok("勇略：请响应【杀】".to_string())
}

/// 勇略：该【杀】结算完毕，若未造成伤害则摸一张。
pub(crate) fn yonglue_on_sha_finished(st: &mut SgsState, target: Option<usize>) {
    let owner_plus = st.counter("yonglue_owner");
    if owner_plus == 0 {
        return;
    }
    let owner = (owner_plus - 1) as usize;
    let before = st.counter("yonglue_hp");
    st.set_counter("yonglue_owner", 0);
    st.set_counter("yonglue_hp", 0);
    let Some(t) = target else {
        return;
    };
    let now = st.seats[t].hp.max(0) as u32;
    if now >= before {
        let drawn = draw_n(st, owner, 1);
        st.push_log(
            Some(&pid(st, owner)),
            format!("勇略：此【杀】未造成伤害，摸 {drawn} 张"),
        );
    }
}

// ---------------------------------------------------------------------------
// 品第（陈群）：弃本阶段未用过的类型牌，令一名角色摸/弃 X 张；已受伤则自己横置。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinDiStage {
    Card,
    Target,
    Mode,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PinDiFrame {
    pub actor: usize,
    pub stage: PinDiStage,
    pub x: u32,
    pub target: Option<usize>,
}

pub(crate) fn pindi_cards(st: &SgsState, actor: usize) -> Vec<String> {
    let used = st.counter("pindi_types");
    turn::hand_ids(st, actor)
        .into_iter()
        .filter(|id| card_type_bit(st, id) & used == 0)
        .collect()
}

pub(crate) fn step_pindi(st: &mut SgsState, f: PinDiFrame) -> Result<bool, EngineError> {
    let (options, actor) = match f.stage {
        PinDiStage::Card => (
            pindi_cards(st, f.actor)
                .into_iter()
                .map(|id| PendingOption {
                    id: format!("choose:{id}"),
                    label: format!("弃置 {}", turn::card_label(st, &id)),
                })
                .collect(),
            f.actor,
        ),
        PinDiStage::Target => (
            st.alive_indices()
                .into_iter()
                .filter(|i| *i != f.actor && st.counter(&format!("pindi_target:{i}")) == 0)
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("令 {} 摸/弃 {} 张", st.seats[i].general_name, f.x),
                })
                .collect(),
            f.actor,
        ),
        PinDiStage::Mode => (
            vec![
                PendingOption {
                    id: "opt1".into(),
                    label: format!("令其摸 {} 张牌", f.x),
                },
                PendingOption {
                    id: "opt2".into(),
                    label: format!("令其弃置 {} 张牌", f.x),
                },
            ],
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
        skill: Some("pindi".into()),
        options,
        data: json!({ "x": f.x }),
    });
    replace_top(st, Frame::PinDi(f));
    Ok(true)
}

pub(crate) fn act_pindi(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::PinDi(f)) => f,
        _ => return Err(internal("栈顶不是品第帧")),
    };
    match f.stage {
        PinDiStage::Card => {
            let card = action
                .strip_prefix("choose:")
                .ok_or_else(|| illegal("需要选择一张牌"))?;
            if !pindi_cards(st, f.actor).contains(&card.to_string()) {
                return Err(illegal("该牌类型本阶段已使用过"));
            }
            let bit = card_type_bit(st, card);
            turn::take_card(st, f.actor, card)?;
            let label = turn::card_label(st, card);
            turn::discard_by(st, Some(f.actor), card);
            let types = st.counter("pindi_types") | bit;
            st.set_counter("pindi_types", types);
            let x = st.counter("pindi_count") + 1;
            st.set_counter("pindi_count", x);
            st.push_log(
                Some(&pid(st, f.actor)),
                format!("品第：弃置【{label}】（X={x}）"),
            );
            let mut nf = f.clone();
            nf.stage = PinDiStage::Target;
            nf.x = x;
            replace_top(st, Frame::PinDi(nf));
            Ok("品第：请选择目标".to_string())
        }
        PinDiStage::Target => {
            let target_pid = action
                .strip_prefix("target:")
                .ok_or_else(|| illegal("需要选择目标"))?;
            let target = st
                .seat_index(target_pid)
                .ok_or_else(|| illegal("目标不在座"))?;
            let mut nf = f.clone();
            nf.stage = PinDiStage::Mode;
            nf.target = Some(target);
            replace_top(st, Frame::PinDi(nf));
            Ok("品第：请选择摸牌或弃牌".to_string())
        }
        PinDiStage::Mode => {
            let target = f.target.ok_or_else(|| internal("品第缺少目标"))?;
            let x = f.x;
            if action == "opt1" {
                let drawn = draw_n(st, target, x);
                st.push_log(
                    Some(&pid(st, f.actor)),
                    format!("品第：{} 摸 {drawn} 张", st.seats[target].general_name),
                );
                cap_hand(st, target);
            } else if action == "opt2" {
                discard_random_hand(st, target, x as usize, "品第");
            } else {
                return Err(illegal("需要选择一项"));
            }
            st.set_counter(&format!("pindi_target:{target}"), 1);
            if st.seats[target].hp < st.seats[target].max_hp && !st.seats[f.actor].chained {
                st.seats[f.actor].chained = true;
                st.push_log(
                    Some(&pid(st, f.actor)),
                    format!("品第：{} 已受伤，陈群横置", st.seats[target].general_name),
                );
                crate::skills::faen_trigger(st, f.actor);
            }
            pop(st);
            Ok("品第结算完成".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// 奔袭（吴懿，锁定）：单目标【杀】且距离均为 1 时选择至多两项强化。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BenXiFrame {
    pub actor: usize,
    pub card: String,
    pub fangtian: bool,
    pub selected: Vec<String>,
}

/// 奔袭：本回合与所有其他角色的距离均为 1（已计入每张牌的 -1）。
pub(crate) fn benxi_all_distance_one(st: &SgsState, actor: usize) -> bool {
    let bonus = crate::skills::benxi_bonus(st, actor).max(0) as usize;
    st.alive_indices()
        .into_iter()
        .all(|i| i == actor || st.seat_distance(actor, i).saturating_sub(bonus) <= 1)
}

pub(crate) fn step_benxi(st: &mut SgsState, f: BenXiFrame) -> Result<bool, EngineError> {
    let pool = [
        ("opt_multi", "目标数 +1"),
        ("opt_armor", "无视防具"),
        ("opt_nores", "不能被抵消"),
        ("opt_draw", "造成伤害后你摸一张"),
    ];
    let mut options: Vec<PendingOption> = pool
        .iter()
        .map(|(id, label)| {
            let mark = if f.selected.iter().any(|s| s == id) {
                "✔ "
            } else {
                ""
            };
            PendingOption {
                id: (*id).into(),
                label: format!("奔袭：{mark}{label}"),
            }
        })
        .collect();
    if !f.selected.is_empty() {
        options.push(PendingOption {
            id: "confirm".into(),
            label: format!("确认（{} 项）", f.selected.len()),
        });
    }
    options.push(PendingOption {
        id: "pass".into(),
        label: "不使用强化，按普通【杀】结算".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("benxi".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::BenXi(f));
    Ok(true)
}

pub(crate) fn act_benxi(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::BenXi(f)) => f,
        _ => return Err(internal("栈顶不是奔袭帧")),
    };
    if action == "pass" {
        pop(st);
        return start_benxi_sha(st, &f, &[]);
    }
    if action == "confirm" {
        if f.selected.is_empty() {
            return Err(illegal("请选择至少 1 项"));
        }
        let selected = f.selected.clone();
        pop(st);
        return start_benxi_sha(st, &f, &selected);
    }
    if !["opt_multi", "opt_armor", "opt_nores", "opt_draw"].contains(&action) {
        return Err(illegal("未知选项"));
    }
    let mut selected = f.selected.clone();
    if let Some(pos) = selected.iter().position(|s| s == action) {
        selected.remove(pos);
    } else {
        if selected.len() >= 2 {
            return Err(illegal("奔袭最多选择两项"));
        }
        selected.push(action.to_string());
    }
    replace_top(st, Frame::BenXi(BenXiFrame { selected, ..f }));
    Ok("已更新奔袭选择".to_string())
}

fn start_benxi_sha(
    st: &mut SgsState,
    f: &BenXiFrame,
    selected: &[String],
) -> Result<String, EngineError> {
    let card = f.card.clone();
    if !st.seats[f.actor].hand.iter().any(|c| c == &card) {
        return Err(illegal("手牌中没有该牌"));
    }
    turn::take_card(st, f.actor, &card)?;
    let multi = selected.iter().any(|s| s == "opt_multi");
    let armor = selected.iter().any(|s| s == "opt_armor");
    let nores = selected.iter().any(|s| s == "opt_nores");
    let draw = selected.iter().any(|s| s == "opt_draw");
    if armor {
        st.set_counter("benxi_armor", 1);
    }
    if draw {
        st.set_counter("benxi_draw", 1);
    }
    crate::resolve::start_sha_flags(
        st,
        f.actor,
        &card,
        f.fangtian,
        DamageType::Normal,
        false,
        multi,
    )?;
    if nores {
        crate::resolve::mark_sha_no_respond(st);
    }
    let names: Vec<String> = selected
        .iter()
        .map(|s| match s.as_str() {
            "opt_multi" => "目标数 +1",
            "opt_armor" => "无视防具",
            "opt_nores" => "不能被抵消",
            _ => "伤害后摸一张",
        })
        .map(str::to_string)
        .collect();
    st.push_log(
        Some(&pid(st, f.actor)),
        if names.is_empty() {
            "奔袭：按普通【杀】使用".to_string()
        } else {
            format!("奔袭：{}", names.join("、"))
        },
    );
    Ok("请选择【杀】的目标".to_string())
}

// ---------------------------------------------------------------------------
// 忠勇（周仓）：使用【杀】后，可将此【杀】或目标打出的【闪】交给其他角色。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZhongYongStage {
    Ask,
    Target,
    ReceiverSha,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZhongYongFrame {
    pub owner: usize,
    pub sha_card: String,
    pub respond_card: Option<String>,
    pub stage: ZhongYongStage,
    pub given: Option<String>,
    pub receiver: Option<usize>,
}

pub(crate) fn step_zhongyong(st: &mut SgsState, f: ZhongYongFrame) -> Result<bool, EngineError> {
    let (options, actor) = match f.stage {
        ZhongYongStage::Ask => {
            let mut options = Vec::new();
            if st.deck.discard.iter().any(|c| c == &f.sha_card) {
                options.push(PendingOption {
                    id: "give:sha".into(),
                    label: format!(
                        "将此【杀】（{}）交给一名其他角色",
                        turn::card_label(st, &f.sha_card)
                    ),
                });
            }
            if let Some(respond) = &f.respond_card {
                if st.deck.discard.iter().any(|c| c == respond) {
                    options.push(PendingOption {
                        id: "give:shan".into(),
                        label: format!(
                            "将其打出的【闪】（{}）交给一名其他角色",
                            turn::card_label(st, respond)
                        ),
                    });
                }
            }
            if options.is_empty() {
                pop(st);
                return Ok(false);
            }
            options.push(PendingOption {
                id: "pass".into(),
                label: "不发动忠勇".into(),
            });
            (options, f.owner)
        }
        ZhongYongStage::Target => (
            st.alive_indices()
                .into_iter()
                .filter(|i| *i != f.owner)
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("交给 {}", st.seats[i].general_name),
                })
                .collect(),
            f.owner,
        ),
        ZhongYongStage::ReceiverSha => {
            let receiver = f.receiver.ok_or_else(|| internal("忠勇缺少接收者"))?;
            (
                st.alive_indices()
                    .into_iter()
                    .filter(|i| *i != receiver && st.in_attack_range(f.owner, *i))
                    .map(|i| PendingOption {
                        id: format!("target:{}", st.seats[i].player_id),
                        label: format!(
                            "令 {} 对 {} 使用【杀】",
                            st.seats[receiver].general_name, st.seats[i].general_name
                        ),
                    })
                    .collect(),
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
        skill: Some("zhongyong".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::ZhongYong(f));
    Ok(true)
}

pub(crate) fn act_zhongyong(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ZhongYong(f)) => f,
        _ => return Err(internal("栈顶不是忠勇帧")),
    };
    match f.stage {
        ZhongYongStage::Ask => {
            if action == "pass" {
                pop(st);
                return Ok("不发动忠勇".to_string());
            }
            let card = match action {
                "give:sha" => f.sha_card.clone(),
                "give:shan" => f
                    .respond_card
                    .clone()
                    .ok_or_else(|| illegal("没有可给的【闪】"))?,
                _ => return Err(illegal("需要选择要交出的牌")),
            };
            replace_top(
                st,
                Frame::ZhongYong(f.clone_into_stage(ZhongYongStage::Target, Some(card))),
            );
            Ok("忠勇：请选择交给的角色".to_string())
        }
        ZhongYongStage::Target => {
            let target_pid = action
                .strip_prefix("target:")
                .ok_or_else(|| illegal("需要选择目标"))?;
            let receiver = st
                .seat_index(target_pid)
                .ok_or_else(|| illegal("目标不在座"))?;
            let Some(card) = f.given.clone() else {
                return Err(internal("忠勇缺少要交出的牌"));
            };
            let Some(pos) = st.deck.discard.iter().position(|c| c == &card) else {
                return Err(illegal("该牌已不在弃牌堆"));
            };
            st.deck.discard.remove(pos);
            let red = st
                .cards
                .get(&card)
                .is_some_and(|c| crate::skills::is_red(c.suit));
            let label = turn::card_label(st, &card);
            st.seats[receiver].hand.push(card);
            st.push_log(
                Some(&pid(st, f.owner)),
                format!(
                    "忠勇：将【{label}】交给 {}",
                    st.seats[receiver].general_name
                ),
            );
            cap_hand(st, receiver);
            if red
                && st
                    .alive_indices()
                    .into_iter()
                    .any(|i| i != receiver && st.in_attack_range(f.owner, i))
            {
                let mut nf = f.clone();
                nf.stage = ZhongYongStage::ReceiverSha;
                nf.receiver = Some(receiver);
                replace_top(st, Frame::ZhongYong(nf));
                return Ok("忠勇：红色牌，接收者可使用【杀】".to_string());
            }
            pop(st);
            Ok("忠勇结算完成".to_string())
        }
        ZhongYongStage::ReceiverSha => {
            let receiver = f.receiver.ok_or_else(|| internal("忠勇缺少接收者"))?;
            let target_pid = action
                .strip_prefix("target:")
                .ok_or_else(|| illegal("需要选择目标"))?;
            let target = st
                .seat_index(target_pid)
                .ok_or_else(|| illegal("目标不在座"))?;
            st.push_log(
                Some(&pid(st, receiver)),
                format!("忠勇：视为对 {} 使用【杀】", st.seats[target].general_name),
            );
            pop(st);
            push(
                st,
                Frame::Sha(ShaFrame {
                    card: String::new(),
                    user: receiver,
                    target: Some(target),
                    stage: ShaStage::WaitRespond,
                    pending_targets: Vec::new(),
                    fangtian: false,
                    damage_type: DamageType::Normal,
                    no_distance: true,
                    virtual_card: true,
                    tianyi: false,
                    no_respond: false,
                }),
            );
            crate::resolve_skill::push_sha_target_response(
                st,
                receiver,
                target,
                "",
                1,
                false,
                false,
                DamageType::Normal,
            );
            Ok("忠勇：视为使用【杀】".to_string())
        }
    }
}

impl ZhongYongFrame {
    fn clone_into_stage(&self, stage: ZhongYongStage, given: Option<String>) -> Self {
        Self {
            stage,
            given,
            ..self.clone()
        }
    }
}

// ---------------------------------------------------------------------------
// 强识（张松）：出牌阶段开始时展示一名其他角色一张手牌。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QiangShiFrame {
    pub actor: usize,
}

pub(crate) fn step_qiangshi(st: &mut SgsState, f: QiangShiFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .filter(|i| *i != f.actor && !st.seats[*i].hand.is_empty())
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!("展示 {} 一张手牌", st.seats[i].general_name),
        })
        .collect();
    if options.is_empty() {
        pop(st);
        return Ok(false);
    }
    options.push(PendingOption {
        id: "pass".into(),
        label: "不发动强识".into(),
    });
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.actor),
        skill: Some("qiangshi".into()),
        options,
        data: json!({}),
    });
    replace_top(st, Frame::QiangShi(f));
    Ok(true)
}

pub(crate) fn act_qiangshi(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::QiangShi(f)) => f,
        _ => return Err(internal("栈顶不是强识帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动强识".to_string());
    }
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| illegal("需要选择目标"))?;
    let target = st
        .seat_index(target_pid)
        .ok_or_else(|| illegal("目标不在座"))?;
    let n = st.seats[target].hand.len();
    if n == 0 {
        return Err(illegal("目标没有手牌"));
    }
    let idx = st.next_index(n);
    let card = st.seats[target].hand[idx].clone();
    let bit = card_type_bit(st, &card);
    st.set_counter("qiangshi_type", bit);
    let kind = match bit {
        1 => "基本牌",
        2 => "锦囊牌",
        _ => "装备牌",
    };
    st.push_log(
        Some(&pid(st, f.actor)),
        format!(
            "强识：展示 {} 的【{}】（{kind}），本阶段使用同类牌可摸一张",
            st.seats[target].general_name,
            turn::card_label(st, &card)
        ),
    );
    pop(st);
    Ok("强识结算完成".to_string())
}

// ---------------------------------------------------------------------------
// 献图（张松）：其他角色出牌阶段开始时摸 2 交 2；若其未杀死角色，你失去 1 点体力。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct XianTuFrame {
    pub owner: usize,
    pub target: usize,
}

pub(crate) fn step_xiantu(st: &mut SgsState, f: XianTuFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("xiantu".into()),
        options: vec![
            PendingOption {
                id: "opt1".into(),
                label: format!(
                    "献图：摸 2 张，交给 {} 两张手牌",
                    st.seats[f.target].general_name
                ),
            },
            PendingOption {
                id: "pass".into(),
                label: "不发动献图".into(),
            },
        ],
        data: json!({}),
    });
    replace_top(st, Frame::XianTu(f));
    Ok(true)
}

pub(crate) fn act_xiantu(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::XianTu(f)) => f,
        _ => return Err(internal("栈顶不是献图帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动献图".to_string());
    }
    if action != "opt1" {
        return Err(illegal("需要选择一项"));
    }
    let drawn = draw_n(st, f.owner, 2);
    let mut given = 0;
    for _ in 0..2 {
        if st.seats[f.owner].hand.is_empty() {
            break;
        }
        let n = st.seats[f.owner].hand.len();
        let idx = st.next_index(n);
        let card = st.seats[f.owner].hand.remove(idx);
        let label = turn::card_label(st, &card);
        st.seats[f.target].hand.push(card);
        st.push_log(
            Some(&pid(st, f.owner)),
            format!(
                "献图：将【{label}】交给 {}",
                st.seats[f.target].general_name
            ),
        );
        given += 1;
    }
    st.set_counter("xiantu_owner", f.owner as u32 + 1);
    st.set_counter("xiantu_target", f.target as u32 + 1);
    st.set_counter("xiantu_given", given);
    st.push_log(
        Some(&pid(st, f.owner)),
        format!("献图：摸 {drawn} 张，交出 {given} 张"),
    );
    cap_hand(st, f.target);
    pop(st);
    Ok("献图结算完成".to_string())
}

/// 献图：该角色出牌阶段结束时，若其本阶段未杀死角色，献图者失去 1 点体力。
pub(crate) fn xiantu_end_phase(st: &mut SgsState, current: usize) {
    let owner_plus = st.counter("xiantu_owner");
    let target_plus = st.counter("xiantu_target");
    st.set_counter("xiantu_owner", 0);
    st.set_counter("xiantu_target", 0);
    if owner_plus == 0 || target_plus == 0 || (target_plus - 1) as usize != current {
        return;
    }
    let owner = (owner_plus - 1) as usize;
    if !st.seats[owner].alive {
        return;
    }
    // 界献图（界张松）：此阶段造成的伤害 < 你交给其的牌数 → 失去 1 点体力。
    if crate::skills::is_jiexian(st, owner) {
        let given = st.counter("xiantu_given");
        let dealt = st.counter(&format!("dealt:{current}"));
        if dealt >= given {
            st.push_log(
                Some(&pid(st, owner)),
                format!(
                    "献图：{} 本阶段造成伤害 {dealt} ≥ 交给的 {given} 张，不失去体力",
                    st.seats[current].general_name
                ),
            );
            return;
        }
    } else if st.counter("phase_kills") > 0 {
        st.push_log(
            Some(&pid(st, owner)),
            format!(
                "献图：{} 本阶段杀死过角色，不失去体力",
                st.seats[current].general_name
            ),
        );
        return;
    }
    st.seats[owner].hp -= 1;
    let hp = st.seats[owner].hp;
    st.push_log(
        Some(&pid(st, owner)),
        format!(
            "献图：{} 未杀死角色，你失去 1 点体力（{hp}）",
            st.seats[current].general_name
        ),
    );
    crate::skills::zhaxiang_on_lose_hp(st, owner);
    if hp <= 0 {
        crate::resolve::push_dying(st, owner, None);
    }
}

/// 忠勇（周仓）：使用【杀】后，可选择将此【杀】或目标打出的【闪】交给其他角色。
pub(crate) fn zhongyong_on_sha_finished(
    st: &mut SgsState,
    user: usize,
    target: Option<usize>,
    card: &str,
    respond: Option<String>,
) {
    if target.is_none() || !crate::skills::has(st, user, "zhongyong") || card.is_empty() {
        return;
    }
    let sha_ok = st.deck.discard.iter().any(|c| c == card);
    let respond_ok = respond
        .as_ref()
        .is_some_and(|r| st.deck.discard.iter().any(|c| c == r));
    if !sha_ok && !respond_ok {
        return;
    }
    push(
        st,
        Frame::ZhongYong(ZhongYongFrame {
            owner: user,
            sha_card: card.to_string(),
            respond_card: respond,
            stage: ZhongYongStage::Ask,
            given: None,
            receiver: None,
        }),
    );
}

// ---------------------------------------------------------------------------
// 勾子：牌进弃牌堆（慎断）
// ---------------------------------------------------------------------------

/// 慎断：黑色基本牌被弃置后，可选择当【兵粮寸断】使用。
pub(crate) fn shenduan_on_discarded(st: &mut SgsState, from: Option<usize>, card: &str) {
    let Some(seat) = from else {
        return;
    };
    if !crate::skills::has(st, seat, "shenduan") {
        return;
    }
    let black_basic = st.cards.get(card).is_some_and(|c| {
        crate::skills::is_black(c.suit)
            && crate::data::card_def(&c.def).is_some_and(|d| d.category == Category::Basic)
    });
    if !black_basic {
        return;
    }
    push(
        st,
        Frame::ShanDuan(ShanDuanFrame {
            owner: seat,
            card: card.to_string(),
        }),
    );
}

/// 对外暴露的牌类型位图（强识用）。
pub(crate) fn card_type_bit_public(st: &SgsState, card_id: &str) -> u32 {
    card_type_bit(st, card_id)
}
