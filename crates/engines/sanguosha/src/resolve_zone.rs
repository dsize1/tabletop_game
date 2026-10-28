//! P2.5 特殊交互区帧：五谷丰登（`wugu`）与观星（`guanxing` / `arrange`）。
//!
//! 设计口径（`docs/games/sanguosha/special-zones.md` §5/§6、`ui-spec.md` §7/§9.5）：
//! - 特殊区**只做数据契约**（P2），UI/拖拽在 P3；引擎只认最终提交（`arrange:<csv>`）。
//! - `wugu`：亮出 = 存活角色数 张牌（公开），**释放者先选**，之后按座位左手侧依次各选 1 张；
//!   每个选择者各自开一次无懈窗口（AOE 口径）；结束后剩余牌进弃牌堆。
//! - `guanxing`：准备阶段自动进入（P2.5 口径，不询问是否发动、不可取消）；取
//!   `k = min(存活角色数, 5)` 张进处理区，2 列 × 5 槽（`L1..L5` / `R1..R5`），
//!   提交 `arrange:<csv>`（按最终顺序，跳空压缩由端侧完成后提交有序 id 列表）；
//!   左列首张成为新牌堆顶，右列末张成为牌堆底。
//! - 两个区都**不允许部分状态**：非法/重放提交一律拒绝且不改状态。

use crate::resolve::{
    leave_processing, pop, push, replace_top, top, Frame, Outcome, WuxieFrame, WuxieRoot,
};
use crate::state::*;
use crate::turn;
use crate::{illegal, internal};
use game_engine_core::EngineError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

pub const WUGU_ZONE: &str = "wugu";
pub const GUANXING_ZONE: &str = "guanxing";
/// 观星槽位行/列数（`ui-spec.md` §7.1）。
pub const GUANXING_ROWS: usize = 5;
pub const GUANXING_COLS: usize = 2;
/// 观星最多观看的张数。
pub const GUANXING_MAX_CARDS: usize = 5;

// ---------------------------------------------------------------------------
// 五谷丰登（wugu）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WuguStage {
    /// 亮牌（从牌堆顶取 存活角色数 张，计入处理区）。
    Reveal,
    /// 为当前选择者开无懈窗口。
    Wuxie,
    WaitWuxie,
    /// 当前选择者挑 1 张。
    Pick,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WuguFrame {
    /// 【五谷丰登】本身（停在处理区，结算结束进弃牌堆）。
    pub card: String,
    /// 使用者。
    pub user: usize,
    /// 选择顺序：`[使用者]` + 使用者左手侧存活角色。
    pub choosers: Vec<usize>,
    pub cursor: usize,
    /// 亮出且尚未被选走的牌（处理区）。
    pub revealed: Vec<String>,
    /// 已选结果 `(座位, 牌)`，仅供日志/视图。
    pub picked: Vec<(usize, String)>,
    pub stage: WuguStage,
}

/// 五谷丰登的选择顺序：**释放者先选**，之后从使用者左手侧起的存活角色。
pub fn wugu_choosers(st: &SgsState, user: usize) -> Vec<usize> {
    let mut v = vec![user];
    v.extend(
        st.seats_from_left_of(user)
            .into_iter()
            .filter(|i| st.seats[*i].alive),
    );
    v
}

/// 开始五谷丰登（调用前牌已移入 `processing`）。
pub fn start_wugu(st: &mut SgsState, user: usize, card: &str) {
    let choosers = wugu_choosers(st, user);
    push(
        st,
        Frame::Wugu(WuguFrame {
            card: card.to_string(),
            user,
            choosers,
            cursor: 0,
            revealed: Vec::new(),
            picked: Vec::new(),
            stage: WuguStage::Reveal,
        }),
    );
}

fn wugu_zone_payload(st: &SgsState, f: &WuguFrame) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert(
        "cards".into(),
        Value::Array(
            f.revealed
                .iter()
                .map(|id| crate::view::card_view(st, id))
                .collect(),
        ),
    );
    m.insert(
        "choosers".into(),
        Value::Array(
            f.choosers
                .iter()
                .map(|i| json!(st.seats[*i].player_id))
                .collect(),
        ),
    );
    m.insert("cursor".into(), json!(f.cursor));
    m.insert(
        "current".into(),
        match f.choosers.get(f.cursor) {
            Some(i) => json!(st.seats[*i].player_id),
            None => Value::Null,
        },
    );
    m.insert(
        "picked".into(),
        Value::Array(
            f.picked
                .iter()
                .map(|(seat, id)| {
                    json!({
                        "player": st.seats[*seat].player_id,
                        "card": crate::view::card_view(st, id),
                    })
                })
                .collect(),
        ),
    );
    m.insert(
        "interaction".into(),
        json!({ "kind": "one", "commit": "pick" }),
    );
    m
}

fn write_wugu_zone(st: &mut SgsState, f: &WuguFrame) {
    let payload = wugu_zone_payload(st, f);
    st.special_zones.insert(
        WUGU_ZONE.to_string(),
        SpecialZone {
            owner: None,
            visibility: Visibility::Public,
            payload,
        },
    );
}

fn finish_wugu(st: &mut SgsState, f: WuguFrame) {
    let remaining = f.revealed.len();
    for id in &f.revealed {
        leave_processing(st, id);
        turn::to_discard_card(st, id);
    }
    leave_processing(st, &f.card);
    turn::to_discard_card(st, &f.card);
    st.special_zones.remove(WUGU_ZONE);
    st.push_log(
        None,
        if remaining > 0 {
            format!("【五谷丰登】结算完毕，剩余 {remaining} 张置入弃牌堆")
        } else {
            "【五谷丰登】结算完毕".to_string()
        },
    );
    pop(st);
}

pub(crate) fn step_wugu(
    st: &mut SgsState,
    mut f: WuguFrame,
    events: &mut Vec<game_engine_core::EngineEvent>,
) -> Result<bool, EngineError> {
    match f.stage {
        WuguStage::Reveal => {
            let n = f.choosers.len();
            let mut revealed = Vec::with_capacity(n);
            for _ in 0..n {
                let Some(id) = turn::draw_one(st, events) else {
                    break;
                };
                st.deck.processing.push(id.clone());
                revealed.push(id);
            }
            f.revealed = revealed;
            write_wugu_zone(st, &f);
            st.push_log(
                Some(&st.seats[f.user].player_id.clone()),
                format!("【五谷丰登】亮出 {} 张牌", f.revealed.len()),
            );
            f.stage = WuguStage::Wuxie;
            replace_top(st, Frame::Wugu(f));
            Ok(false)
        }
        WuguStage::Wuxie => {
            while f.cursor < f.choosers.len() && !st.seats[f.choosers[f.cursor]].alive {
                f.cursor += 1;
            }
            if f.cursor >= f.choosers.len() || f.revealed.is_empty() {
                finish_wugu(st, f);
                return Ok(false);
            }
            let chooser = f.choosers[f.cursor];
            f.stage = WuguStage::WaitWuxie;
            replace_top(st, Frame::Wugu(f.clone()));
            push(
                st,
                Frame::Wuxie(WuxieFrame {
                    root: WuxieRoot::Trick {
                        card: f.card.clone(),
                        user: f.user,
                        target: chooser,
                    },
                    count: 0,
                    order: crate::resolve::wuxie_order_of(st, f.user),
                    cursor: 0,
                    passed: 0,
                }),
            );
            Ok(false)
        }
        WuguStage::WaitWuxie => match st.outcome.take() {
            Some(Outcome::Wuxie { cancelled }) => {
                if cancelled {
                    let chooser = f.choosers[f.cursor];
                    st.push_log(
                        None,
                        format!(
                            "【五谷丰登】对 {} 的效果被无懈，跳过选择",
                            st.seats[chooser].general_name
                        ),
                    );
                    f.cursor += 1;
                    f.stage = WuguStage::Wuxie;
                } else {
                    f.stage = WuguStage::Pick;
                }
                replace_top(st, Frame::Wugu(f));
                Ok(false)
            }
            _ => Err(internal("缺少五谷丰登无懈结果")),
        },
        WuguStage::Pick => {
            let chooser = f.choosers[f.cursor];
            let needed = st.config.max_hand_cards as usize;
            let options: Vec<PendingOption> = f
                .revealed
                .iter()
                .map(|id| PendingOption {
                    id: format!("pick:{id}"),
                    label: format!(
                        "选择【{}】（{}/{}）",
                        turn::card_label(st, id),
                        st.seats[chooser].hand.len() + 1,
                        needed
                    ),
                })
                .collect();
            st.pending = Some(Pending {
                kind: WUGU_ZONE.into(),
                actor: st.seats[chooser].player_id.clone(),
                skill: None,
                options,
                data: json!({
                    "card": f.card,
                    "remaining": f.revealed.len(),
                    "chooser": st.seats[chooser].player_id,
                    "zone": WUGU_ZONE,
                }),
            });
            replace_top(st, Frame::Wugu(f));
            Ok(true)
        }
    }
}

/// 五谷丰登选牌：一次提交 1 张。
pub(crate) fn act_wugu(
    st: &mut SgsState,
    _pending: &Pending,
    action: &str,
) -> Result<String, EngineError> {
    let mut f = match top(st).cloned() {
        Some(Frame::Wugu(f)) => f,
        _ => return Err(internal("栈顶不是五谷丰登帧")),
    };
    if f.stage != WuguStage::Pick {
        return Err(internal("五谷丰登当前不在选牌阶段"));
    }
    let card_id = action
        .strip_prefix("pick:")
        .ok_or_else(|| illegal("需要选择一张亮出的牌"))?
        .to_string();
    let pos = f
        .revealed
        .iter()
        .position(|c| c == &card_id)
        .ok_or_else(|| illegal("该牌不在五谷丰登亮出的牌中"))?;
    let chooser = f.choosers[f.cursor];
    leave_processing(st, &card_id);
    st.seats[chooser].hand.push(card_id.clone());
    f.revealed.remove(pos);
    f.picked.push((chooser, card_id.clone()));
    f.cursor += 1;
    f.stage = WuguStage::Wuxie;
    replace_top(st, Frame::Wugu(f.clone()));
    write_wugu_zone(st, &f);

    let over = st.seats[chooser]
        .hand
        .len()
        .saturating_sub(st.config.max_hand_cards as usize);
    if over > 0 {
        crate::resolve::push_discard(st, chooser, over, "hand_cap");
    }
    let msg = format!(
        "{} 获得【{}】",
        st.seats[chooser].general_name,
        turn::card_label(st, &card_id)
    );
    st.push_log(Some(&st.seats[chooser].player_id.clone()), msg.clone());
    Ok(msg)
}

// ---------------------------------------------------------------------------
// 观星（guanxing → arrange）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArrangeStage {
    /// 取牌 + 建特殊区（写入 `slots` 初始分布）。
    Deal,
    /// 等待 `arrange:<csv>` 一次性提交。
    Await,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArrangeFrame {
    pub owner: usize,
    /// 特殊区 id（当前只有 `guanxing`）。
    pub zone: String,
    /// 技能 code（观星 = `guanxing`）。
    pub skill: String,
    pub stage: ArrangeStage,
    /// 本次取出的牌（处理区）。
    pub cards: Vec<String>,
}

/// 观星张数：`min(存活角色数, 5)`。
pub fn guanxing_count(st: &SgsState) -> usize {
    st.alive_indices().len().clamp(1, GUANXING_MAX_CARDS)
}

/// 起始帧（准备阶段由 `turn::phase_step` 压入）。
pub fn start_guanxing(owner: usize) -> Frame {
    Frame::Arrange(ArrangeFrame {
        owner,
        zone: GUANXING_ZONE.to_string(),
        skill: "guanxing".to_string(),
        stage: ArrangeStage::Deal,
        cards: Vec::new(),
    })
}

fn slot_id(col: usize, row: usize) -> String {
    let prefix = if col == 0 { "L" } else { "R" };
    format!("{prefix}{row}")
}

fn empty_slots() -> Vec<(String, Option<String>)> {
    let mut v = Vec::with_capacity(GUANXING_ROWS * GUANXING_COLS);
    for col in 0..GUANXING_COLS {
        for row in 1..=GUANXING_ROWS {
            v.push((slot_id(col, row), None));
        }
    }
    v
}

fn slots_to_value(slots: &[(String, Option<String>)]) -> Value {
    let mut m = Map::new();
    for (k, v) in slots {
        m.insert(k.clone(), json!(v));
    }
    Value::Object(m)
}

/// 从特殊区读出当前槽位（供 `settle` 与校验共用）。
fn read_slots(st: &SgsState, zone: &str) -> Option<Vec<(String, Option<String>)>> {
    let payload = &st.special_zones.get(zone)?.payload;
    let obj = payload.get("slots")?.as_object()?;
    let mut out = Vec::new();
    for col in 0..GUANXING_COLS {
        for row in 1..=GUANXING_ROWS {
            let key = slot_id(col, row);
            let v = obj.get(&key).and_then(|v| v.as_str()).map(str::to_string);
            out.push((key, v));
        }
    }
    Some(out)
}

/// 结算顺序：每列跳过空槽、按上到下压紧，左列（`L1→L5`）接右列（`R1→R5`）。
pub fn settle_slots(slots: &[(String, Option<String>)]) -> Vec<String> {
    slots
        .iter()
        .filter_map(|(_, v)| v.clone())
        .collect::<Vec<String>>()
}

/// 当前特殊区的结算顺序（没有该区时为空）。
pub fn settle_zone(st: &SgsState, zone: &str) -> Vec<String> {
    read_slots(st, zone)
        .map(|slots| settle_slots(&slots))
        .unwrap_or_default()
}

fn write_guanxing_zone(st: &mut SgsState, f: &ArrangeFrame, slots: &[(String, Option<String>)]) {
    let mut payload = Map::new();
    payload.insert("rows".into(), json!(GUANXING_ROWS));
    payload.insert("cols".into(), json!(GUANXING_COLS));
    payload.insert("slots".into(), slots_to_value(slots));
    payload.insert("rev".into(), json!(1));
    payload.insert("skill".into(), json!(f.skill));
    payload.insert(
        "interaction".into(),
        json!({ "kind": "ordered", "commit": "arrange", "slots": "L1..L5,R1..R5" }),
    );
    st.special_zones.insert(
        f.zone.clone(),
        SpecialZone {
            owner: Some(st.seats[f.owner].player_id.clone()),
            visibility: Visibility::Owner,
            payload,
        },
    );
}

pub(crate) fn step_arrange(
    st: &mut SgsState,
    mut f: ArrangeFrame,
    events: &mut Vec<game_engine_core::EngineEvent>,
) -> Result<bool, EngineError> {
    match f.stage {
        ArrangeStage::Deal => {
            let n = guanxing_count(st);
            let mut cards = Vec::with_capacity(n);
            for _ in 0..n {
                let Some(id) = turn::draw_one(st, events) else {
                    break;
                };
                st.deck.processing.push(id.clone());
                cards.push(id);
            }
            if cards.is_empty() {
                st.push_log(
                    Some(&st.seats[f.owner].player_id.clone()),
                    "观星：牌堆不可用，跳过",
                );
                pop(st);
                return Ok(false);
            }
            let mut slots = empty_slots();
            for (k, id) in cards.iter().enumerate().take(GUANXING_ROWS) {
                // 初始分布：依次填左列 L1..Lk（右列为空）。
                slots[k].1 = Some(id.clone());
            }
            f.cards = cards;
            f.stage = ArrangeStage::Await;
            write_guanxing_zone(st, &f, &slots);
            let pid = st.seats[f.owner].player_id.clone();
            st.push_log(
                Some(&pid),
                format!("观星：观看牌堆顶 {} 张牌", f.cards.len()),
            );
            events.push(game_engine_core::EngineEvent::new(
                "guanxing_enter",
                json!({
                    "player": pid,
                    "count": f.cards.len(),
                    "rev": 1,
                    "cards": settle_slots(&slots),
                }),
            ));
            replace_top(st, Frame::Arrange(f));
            Ok(false)
        }
        ArrangeStage::Await => {
            let slots = read_slots(st, &f.zone).unwrap_or_else(empty_slots);
            // 特殊区是唯一真源：结算顺序一律从 `slots` 重算（不允许端侧另传槽位）。
            let order = settle_zone(st, &f.zone);
            st.pending = Some(Pending {
                kind: "arrange".into(),
                actor: st.seats[f.owner].player_id.clone(),
                skill: Some(f.skill.clone()),
                options: vec![PendingOption {
                    id: format!("arrange:{}", order.join(",")),
                    label: format!("确认观星顺序（左列→右列，共 {} 张）", order.len()),
                }],
                data: json!({
                    "zone": f.zone,
                    "rows": GUANXING_ROWS,
                    "cols": GUANXING_COLS,
                    "slots": slots_to_value(&slots),
                    "rev": st
                        .special_zones
                        .get(&f.zone)
                        .and_then(|z| z.payload.get("rev"))
                        .cloned()
                        .unwrap_or(json!(1)),
                    "interaction": {
                        "kind": "ordered",
                        "commit": "arrange",
                        "slots": "L1..L5,R1..R5"
                    },
                }),
            });
            replace_top(st, Frame::Arrange(f));
            Ok(true)
        }
    }
}

/// 观星提交：`arrange:<id,id,...>`（最终顺序 = 新牌堆顶 → 底）。
///
/// 校验：张数一致、无重复、全部来自本次取出的牌；非法/重放（区已消失或牌已回牌堆）一律拒绝。
pub(crate) fn act_arrange(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    let f = match top(st).cloned() {
        Some(Frame::Arrange(f)) => f,
        _ => return Err(internal("栈顶不是观星帧")),
    };
    if f.stage != ArrangeStage::Await {
        return Err(internal("观星当前不在提交阶段"));
    }
    let csv = action
        .strip_prefix("arrange:")
        .ok_or_else(|| illegal("需要提交观星顺序（arrange:<csv>）"))?;
    let ids: Vec<String> = csv
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    if ids.len() != f.cards.len() {
        return Err(illegal(format!(
            "观星需提交 {} 张牌的顺序（收到 {} 张）",
            f.cards.len(),
            ids.len()
        )));
    }
    let mut seen = BTreeSet::new();
    for id in &ids {
        if !seen.insert(id.clone()) {
            return Err(illegal("观星顺序中出现重复的牌"));
        }
        if !f.cards.contains(id) {
            return Err(illegal("观星顺序包含非本次取出的牌"));
        }
        if !st.deck.processing.iter().any(|c| c == id) {
            return Err(illegal("观星牌已不在处理区（可能已提交过）"));
        }
    }
    // 特殊区里的槽位必须与本次取出的牌一致（防止外部篡改后回写错牌）。
    let slots = read_slots(st, &f.zone).ok_or_else(|| internal("观星特殊区缺少槽位"))?;
    let mut zone_cards: Vec<String> = settle_slots(&slots);
    zone_cards.sort();
    let mut expect = f.cards.clone();
    expect.sort();
    if zone_cards != expect {
        return Err(internal("观星特殊区与取出牌不一致"));
    }

    // 回写牌堆：`draw` 末尾 = 牌堆顶，故按最终顺序倒序压入。
    for id in ids.iter().rev() {
        leave_processing(st, id);
        st.deck.draw.push(id.clone());
    }
    st.special_zones.remove(&f.zone);
    pop(st);
    let pid = st.seats[f.owner].player_id.clone();
    let names: Vec<String> = ids.iter().map(|id| turn::card_label(st, id)).collect();
    st.push_log(
        Some(&pid),
        format!("观星完成：牌堆顶顺序 {}", names.join(" → ")),
    );
    Ok(format!("观星完成（{} 张）", ids.len()))
}
