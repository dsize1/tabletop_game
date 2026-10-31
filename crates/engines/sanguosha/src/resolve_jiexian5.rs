//! P4.4.2 界限突破（批次 J4：8 将）技能帧与钩子。
//!
//! 覆盖：界再起（界孟获）、界截辎（界徐晃）、狞恶（界典韦）、界双雄（界颜良文丑）、
//! 界固政（界张昭张纮）、界悲歌（界蔡文姬）、界节命（界荀彧）、界破军（界徐盛）。
//! 同名技能 code 复用（ADR-0024），差异按 `skills::is_jiexian` 分支。

use crate::internal;
use crate::resolve::{pop, push, replace_top, top, Frame};
use crate::skills::{has, is_jiexian};
use crate::state::*;
use crate::turn;
use game_engine_core::EngineError;
use serde_json::json;

fn pid(st: &SgsState, seat: usize) -> String {
    st.seats[seat].player_id.clone()
}

fn pass() -> PendingOption {
    PendingOption {
        id: "pass".into(),
        label: "不发动".into(),
    }
}

// ---------------------------------------------------------------------------
// 界再起（界孟获）：结束阶段，本回合每有一张红牌置入弃牌堆 → 令一名角色摸 1 或你回复 1
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZaiQiStage {
    Target,
    Choice,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZaiQiJieFrame {
    pub actor: usize,
    pub remaining: u32,
    pub stage: ZaiQiStage,
    pub target: Option<usize>,
}

/// 结束阶段触发（红牌计数由 `skills::note_discard_suit` 累计到 `red_discarded`）。
pub(crate) fn zaizhi_jie_trigger(st: &mut SgsState, seat: usize) -> bool {
    if !has(st, seat, "zaizhi")
        || !is_jiexian(st, seat)
        || st.counter("zaizhi_jie_asked") > 0
        || st.counter("red_discarded") == 0
    {
        return false;
    }
    st.set_counter("zaizhi_jie_asked", 1);
    push(
        st,
        Frame::ZaiQiJie(ZaiQiJieFrame {
            actor: seat,
            remaining: st.counter("red_discarded"),
            stage: ZaiQiStage::Target,
            target: None,
        }),
    );
    true
}

pub(crate) fn step_zaizhi_jie(st: &mut SgsState, f: ZaiQiJieFrame) -> Result<bool, EngineError> {
    if f.remaining == 0 {
        pop(st);
        return Ok(false);
    }
    match f.stage {
        ZaiQiStage::Target => {
            let mut options: Vec<PendingOption> = st
                .alive_indices()
                .into_iter()
                .map(|i| PendingOption {
                    id: format!("target:{}", st.seats[i].player_id),
                    label: format!("再起：选择 {}", st.seats[i].general_name),
                })
                .collect();
            options.push(pass());
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: pid(st, f.actor),
                skill: Some("zaizhi_jie".into()),
                options,
                data: json!({ "skill": "zaizhi", "stage": "target", "remaining": f.remaining }),
            });
            replace_top(st, Frame::ZaiQiJie(f));
            Ok(true)
        }
        ZaiQiStage::Choice => {
            let Some(target) = f.target else {
                pop(st);
                return Ok(false);
            };
            st.pending = Some(Pending {
                kind: "choice".into(),
                actor: pid(st, target),
                skill: Some("zaizhi_jie_choice".into()),
                options: vec![
                    PendingOption {
                        id: "choose:draw".into(),
                        label: "再起：摸一张牌".into(),
                    },
                    PendingOption {
                        id: "choose:heal".into(),
                        label: format!("再起：令 {} 回复 1 点体力", st.seats[f.actor].general_name),
                    },
                ],
                data: json!({ "skill": "zaizhi_choice", "owner": f.actor }),
            });
            replace_top(st, Frame::ZaiQiJie(f));
            Ok(true)
        }
    }
}

pub(crate) fn act_zaizhi_jie(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::ZaiQiJie(f)) => f,
        _ => return Err(internal("栈顶不是再起帧")),
    };
    match f.stage {
        ZaiQiStage::Target => {
            if action == "pass" {
                pop(st);
                return Ok("不发动再起".to_string());
            }
            let target_pid = action
                .strip_prefix("target:")
                .ok_or_else(|| crate::illegal("需要选择一名角色"))?;
            let target = st
                .seat_index(target_pid)
                .ok_or_else(|| crate::illegal("目标不在座"))?;
            replace_top(
                st,
                Frame::ZaiQiJie(ZaiQiJieFrame {
                    actor: f.actor,
                    remaining: f.remaining,
                    stage: ZaiQiStage::Choice,
                    target: Some(target),
                }),
            );
            Ok("再起：请目标选择".to_string())
        }
        ZaiQiStage::Choice => {
            let target = f.target.ok_or_else(|| internal("再起缺少目标"))?;
            let mut nf = f.clone();
            nf.remaining = nf.remaining.saturating_sub(1);
            nf.stage = ZaiQiStage::Target;
            nf.target = None;
            match action {
                "choose:draw" => {
                    let mut ev = Vec::new();
                    let drawn = turn::draw_cards(st, target, 1, &mut ev);
                    st.push_log(Some(&pid(st, target)), format!("再起：摸 {drawn} 张"));
                }
                "choose:heal" => {
                    st.seats[f.actor].hp = (st.seats[f.actor].hp + 1).min(st.seats[f.actor].max_hp);
                    let hp = st.seats[f.actor].hp;
                    st.push_log(
                        Some(&pid(st, f.actor)),
                        format!("再起：回复 1 点体力（{hp}）"),
                    );
                }
                _ => return Err(crate::illegal("未知再起选项")),
            }
            replace_top(st, Frame::ZaiQiJie(nf));
            Ok("再起：已结算一次".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// 界截辎（界徐晃）：其他角色跳过摸牌阶段后，选一名角色获得「辎」或摸一张
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JieZiJieFrame {
    pub owner: usize,
}

/// 有「辎」的座位数量。
pub fn zai_marks(st: &SgsState, seat: usize) -> u32 {
    st.seats
        .get(seat)
        .and_then(|s| s.mark_counts.get("zai"))
        .copied()
        .unwrap_or(0)
}

fn add_zai(st: &mut SgsState, seat: usize) {
    if let Some(s) = st.seats.get_mut(seat) {
        *s.mark_counts.entry("zai".into()).or_insert(0) += 1;
    }
}

fn remove_zai(st: &mut SgsState, seat: usize) {
    if let Some(s) = st.seats.get_mut(seat) {
        match s.mark_counts.get("zai").copied().unwrap_or(0) {
            0 => {}
            1 => {
                s.mark_counts.remove("zai");
            }
            n => {
                s.mark_counts.insert("zai".into(), n - 1);
            }
        }
    }
}

/// 跳过摸牌阶段后的截辎钩子：界版询问选择；标准版自动摸 1（在 skills 内处理）。
pub(crate) fn push_jiezi_jie(st: &mut SgsState, owner: usize) -> bool {
    if !has(st, owner, "jiezi") || !is_jiexian(st, owner) {
        return false;
    }
    push(st, Frame::JieZiJie(JieZiJieFrame { owner }));
    true
}

pub(crate) fn step_jiezi_jie(st: &mut SgsState, f: JieZiJieFrame) -> Result<bool, EngineError> {
    let mut options: Vec<PendingOption> = st
        .alive_indices()
        .into_iter()
        .map(|i| PendingOption {
            id: format!("target:{}", st.seats[i].player_id),
            label: format!(
                "截辎：选择 {}（手牌 {}）",
                st.seats[i].general_name,
                st.seats[i].hand.len()
            ),
        })
        .collect();
    options.push(pass());
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("jiezi".into()),
        options,
        data: json!({ "skill": "jiezi" }),
    });
    replace_top(st, Frame::JieZiJie(f));
    Ok(true)
}

pub(crate) fn act_jiezi_jie(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::JieZiJie(f)) => f,
        _ => return Err(internal("栈顶不是截辎帧")),
    };
    if action == "pass" {
        pop(st);
        return Ok("不发动截辎".to_string());
    }
    let target_pid = action
        .strip_prefix("target:")
        .ok_or_else(|| crate::illegal("需要选择一名角色"))?;
    let target = st
        .seat_index(target_pid)
        .ok_or_else(|| crate::illegal("目标不在座"))?;
    let min_hand = st
        .alive_indices()
        .into_iter()
        .map(|i| st.seats[i].hand.len())
        .min()
        .unwrap_or(0);
    let eligible = st.seats[target].hand.len() == min_hand && zai_marks(st, target) == 0;
    pop(st);
    if eligible {
        add_zai(st, target);
        st.push_log(
            Some(&pid(st, f.owner)),
            format!(
                "截辎：{} 获得「辎」（手牌全场最少）",
                st.seats[target].general_name
            ),
        );
        Ok("截辎：已给予「辎」".to_string())
    } else {
        let mut ev = Vec::new();
        let drawn = turn::draw_cards(st, target, 1, &mut ev);
        st.push_log(
            Some(&pid(st, f.owner)),
            format!(
                "截辎：{} 摸 {drawn} 张（不满足「辎」条件）",
                st.seats[target].general_name
            ),
        );
        Ok("截辎：令其摸一张".to_string())
    }
}

/// 有「辎」的角色进入摸牌阶段：移除「辎」并记录额外摸牌（阶段末多摸 2 张）。
pub(crate) fn jiezi_jie_extra_draw(st: &mut SgsState, seat: usize) -> bool {
    if zai_marks(st, seat) == 0 {
        return false;
    }
    remove_zai(st, seat);
    st.set_counter("zai_extra_draw", 1);
    st.push_log(
        Some(&pid(st, seat)),
        "截辎：「辎」移除，本摸牌阶段结束后执行额外摸牌阶段（多摸 2 张）",
    );
    true
}

// ---------------------------------------------------------------------------
// 狞恶（界典韦）：一名角色每回合第二次受到伤害后 → 你摸 1 并弃置其一张牌
// ---------------------------------------------------------------------------

/// 伤害后钩子（自动）：`dmg_count:<seat>` 为本回合受伤次数。
pub(crate) fn ninge_on_damage(st: &mut SgsState, victim: usize, source: Option<usize>) {
    let key = format!("dmg_count:{victim}");
    let n = st.counter(&key) + 1;
    st.set_counter(&key, n);
    if n != 2 {
        return;
    }
    let owner = st
        .alive_indices()
        .into_iter()
        .find(|i| has(st, *i, "ninge") && (*i == victim || Some(*i) == source));
    let Some(owner) = owner else {
        return;
    };
    let mut ev = Vec::new();
    let drawn = turn::draw_cards(st, owner, 1, &mut ev);
    let discarded = crate::resolve_jiexian::discard_one_from(st, victim);
    let what = discarded
        .map(|c| turn::card_label(st, &c))
        .unwrap_or_else(|| "（无牌可弃）".to_string());
    st.push_log(
        Some(&pid(st, owner)),
        format!(
            "狞恶：{} 本回合第二次受伤，你摸 {drawn} 张并弃置其{what}",
            st.seats[victim].general_name
        ),
    );
}

// ---------------------------------------------------------------------------
// 界双雄（界颜良文丑）：摸牌阶段结束可弃一张牌 → 本回合异色牌当【决斗】；结束阶段获得伤害牌
// ---------------------------------------------------------------------------

/// 双雄界：记录弃牌颜色（`true` = 红色）。
pub(crate) fn set_shuangxiong_jie_color(st: &mut SgsState, card: &str) {
    let red = st
        .cards
        .get(card)
        .is_some_and(|c| crate::skills::is_red(c.suit));
    st.set_counter("shuangxiong_color", if red { 1 } else { 2 });
}

/// 双雄界：记录「本回合对你造成过伤害的牌」。
pub(crate) fn shuangxiong_jie_on_damage(st: &mut SgsState, victim: usize, card: Option<&str>) {
    let Some(card) = card.filter(|c| !c.is_empty()) else {
        return;
    };
    if has(st, victim, "shuangxiong") && is_jiexian(st, victim) {
        st.seats[victim]
            .flags
            .insert(format!("sx_dmg:{card}"), true);
    }
}

/// 双雄界：结束阶段获得这些牌（从弃牌堆/处理区取回）。
pub(crate) fn shuangxiong_jie_gain(st: &mut SgsState, seat: usize) {
    if !has(st, seat, "shuangxiong") || !is_jiexian(st, seat) {
        return;
    }
    let keys: Vec<String> = st.seats[seat]
        .flags
        .keys()
        .filter(|k| k.starts_with("sx_dmg:"))
        .cloned()
        .collect();
    let mut got = 0;
    for k in keys {
        st.seats[seat].flags.remove(&k);
        let card = k.trim_start_matches("sx_dmg:").to_string();
        if st.deck.discard.iter().any(|c| c == &card) {
            st.deck.discard.retain(|c| c != &card);
        } else if st.deck.processing.iter().any(|c| c == &card) {
            st.deck.processing.retain(|c| c != &card);
        } else {
            continue;
        }
        st.seats[seat].hand.push(card);
        got += 1;
    }
    if got > 0 {
        st.push_log(
            Some(&pid(st, seat)),
            format!("双雄：获得本回合对你造成过伤害的 {got} 张牌"),
        );
    }
}

// ---------------------------------------------------------------------------
// 界固政（界张昭张纮）：其他角色本阶段至少两张牌因弃置置入弃牌堆后 → 其获得一张，你获得其余
// ---------------------------------------------------------------------------

/// 弃牌钩子：界固政。
pub(crate) fn guzheng_jie_on_discard(st: &mut SgsState, seat: usize) {
    let owner = st.alive_indices().into_iter().find(|i| {
        *i != seat
            && has(st, *i, "guzheng")
            && is_jiexian(st, *i)
            && st.counter("guzheng_jie_used") == 0
    });
    let Some(owner) = owner else {
        return;
    };
    if st.counter(&format!("discarded_by:{seat}")) < 2 {
        return;
    }
    st.set_counter("guzheng_jie_used", 1);
    push(
        st,
        Frame::GuZhengJie(GuZhengJieFrame {
            owner,
            discarder: seat,
        }),
    );
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GuZhengJieFrame {
    pub owner: usize,
    pub discarder: usize,
}

pub(crate) fn step_guzheng_jie(st: &mut SgsState, f: GuZhengJieFrame) -> Result<bool, EngineError> {
    st.pending = Some(Pending {
        kind: "choice".into(),
        actor: pid(st, f.owner),
        skill: Some("guzheng_jie".into()),
        options: vec![
            PendingOption {
                id: "choose:guzheng".into(),
                label: format!(
                    "固政：令 {} 获得一张弃牌，你获得其余",
                    st.seats[f.discarder].general_name
                ),
            },
            pass(),
        ],
        data: json!({ "skill": "guzheng" }),
    });
    replace_top(st, Frame::GuZhengJie(f));
    Ok(true)
}

pub(crate) fn act_guzheng_jie(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::GuZhengJie(f)) => f,
        _ => return Err(internal("栈顶不是固政帧")),
    };
    pop(st);
    if action != "choose:guzheng" {
        return Ok("不发动固政".to_string());
    }
    // 该角色本阶段弃置的牌（`gz_jie:<seat>:<card>` 记录挂在弃置者身上，见 turn::discard_by）。
    let prefix = format!("gz_jie:{}:", f.discarder);
    let keys: Vec<String> = st.seats[f.discarder]
        .flags
        .keys()
        .filter(|k| k.starts_with(&prefix))
        .cloned()
        .collect();
    let mut cards: Vec<String> = Vec::new();
    for k in keys {
        st.seats[f.discarder].flags.remove(&k);
        cards.push(k.trim_start_matches(&prefix).to_string());
    }
    let mut moved = 0;
    for (i, card) in cards.iter().enumerate() {
        if !st.deck.discard.iter().any(|c| c == card) {
            continue;
        }
        st.deck.discard.retain(|c| c != card);
        let to = if i == 0 { f.discarder } else { f.owner };
        st.seats[to].hand.push(card.clone());
        moved += 1;
    }
    st.push_log(
        Some(&pid(st, f.owner)),
        format!(
            "固政：{} 获得一张弃牌，张昭张纮获得其余（共 {moved} 张）",
            st.seats[f.discarder].general_name
        ),
    );
    Ok("固政：已分配".to_string())
}

// ---------------------------------------------------------------------------
// 界破军（界徐盛）：【杀】对手牌与装备均不多于你的角色伤害 +1
// ---------------------------------------------------------------------------

pub(crate) fn pojun_damage_bonus(st: &SgsState, source: usize, victim: usize) -> i32 {
    if !has(st, source, "pojun") || !is_jiexian(st, source) {
        return 0;
    }
    let src_hand = st.seats[source].hand.len();
    let src_equip = st.seats[source]
        .equips
        .values()
        .filter(|c| c.is_some())
        .count();
    let dst_hand = st.seats[victim].hand.len();
    let dst_equip = st.seats[victim]
        .equips
        .values()
        .filter(|c| c.is_some())
        .count();
    i32::from(dst_hand <= src_hand && dst_equip <= src_equip)
}
