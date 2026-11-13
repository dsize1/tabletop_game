//! AI 可见信息视图（`SgsView`）。
//!
//! **硬约束（ADR-0032）**：AI 只允许读「当前玩家在 Web 端能看到的东西」——
//! 公共区（体力/手牌数/装备/判定区/标记/存活/公开身份）+ 本人私有（手牌明细、
//! 本人身份与 pending 选项）+ 按可见性下发的特殊交互区。
//!
//! 因此这里**只投影**、不引用 `SgsState`：handler 拿不到他人手牌、他人身份、牌堆顺序。
//! 同源口径见 `crate::view::view_data`（Web/公共视图）；两者若出现偏差以 `view` 为准。
//!
//! 不变量由 `ai::tests::hidden_info_invariance` 守护：篡改隐藏信息不得改变决策。

use crate::data;
use crate::state::{
    Category, EquipSlot, Identity, LogEntry, Observation, Phase, Pending, SgsState, Suit,
    TurnPhase, Visibility,
};
use serde_json::Value;
use std::collections::BTreeMap;

/// 一张牌的可见信息（含 AI 用的基准价值）。
#[derive(Debug, Clone)]
pub struct CardView {
    pub id: String,
    pub code: String,
    pub name: String,
    pub suit: Suit,
    pub rank: u8,
    pub category: Option<Category>,
    pub equip_slot: Option<EquipSlot>,
    pub delayed: bool,
}

impl CardView {
    /// 基准价值（见 `ai::card::base_value`）。
    pub fn value(&self) -> f64 {
        super::card::base_value(&self.code)
    }
}

/// 座位公开视图（本人座位额外带手牌明细）。
#[derive(Debug, Clone)]
pub struct SeatView {
    pub index: usize,
    pub player_id: String,
    pub general: Option<String>,
    pub general_name: String,
    /// 公开身份（主公 / 已死亡 / 本人）；其余为 `None`。
    pub identity: Option<Identity>,
    pub hp: i32,
    pub max_hp: i32,
    pub hand_count: usize,
    pub hand_limit: usize,
    pub equips: Vec<CardView>,
    pub judge_zone: Vec<CardView>,
    pub marks_count: BTreeMap<String, u32>,
    pub alive: bool,
    pub flipped: bool,
    pub chained: bool,
    pub is_bot: bool,
}

impl SeatView {
    /// 体力健康度 0..1（1 = 满血）。
    pub fn health(&self) -> f64 {
        if self.max_hp <= 0 {
            return 0.0;
        }
        (self.hp as f64 / self.max_hp as f64).clamp(0.0, 1.0)
    }

    /// 装备槽是否已有装备。
    pub fn has_equip(&self, slot: EquipSlot) -> bool {
        self.equips.iter().any(|c| c.equip_slot == Some(slot))
    }

    /// 某槽位的装备牌（若有）。
    pub fn equip(&self, slot: EquipSlot) -> Option<&CardView> {
        self.equips.iter().find(|c| c.equip_slot == Some(slot))
    }
}

/// 当前 pending 的可见部分（仅当 actor 是本人才有选项内容）。
#[derive(Debug, Clone)]
pub struct PendingView {
    pub kind: String,
    pub actor: String,
    pub skill: Option<String>,
    /// 选项（id, label）；非本人 actor 时为空。
    pub options: Vec<(String, String)>,
    pub data: Value,
}

/// 决策视图：一局里「当前玩家能看到的一切」。
#[derive(Debug, Clone)]
pub struct SgsView {
    /// 本人座位下标。
    pub me: usize,
    pub me_id: String,
    pub identity: Identity,
    pub general: Option<String>,
    pub phase: Phase,
    pub turn: usize,
    pub turn_phase: TurnPhase,
    pub round: u32,
    /// 本人手牌明细。
    pub hand: Vec<CardView>,
    /// 本人装备 / 判定区。
    pub equips: Vec<CardView>,
    pub judge_zone: Vec<CardView>,
    pub seats: Vec<SeatView>,
    /// 等待中的交互（可能是别人；选项内容只在本人的时候下发）。
    pub pending: Option<PendingView>,
    /// 可见的特殊交互区（`wugu` public / `guanxing` owner 等）。
    pub special_zones: BTreeMap<String, Value>,
    /// 牌堆剩余张数（公开摘要）。
    pub deck_remaining: usize,
    /// 弃牌堆顶（公开）。
    pub discard_top: Option<CardView>,
    /// 已知牌面（本人手牌 + 公开区装备/判定 + 特殊区亮出的牌）：`card_id → 牌面`。
    pub known_cards: BTreeMap<String, CardView>,
    /// 公开观测（谁对谁造成过伤害等）：身份/威胁推断的数据源（P6.5）。
    pub observations: Vec<Observation>,
    /// 公开出牌历史（尾部最近若干条；AI 用来做「本回合已发动过该技能」之类的重复守卫）。
    pub logs: Vec<LogEntry>,
}

/// 构造一张牌的可见视图（牌实例不存在时返回 `None`；生效定义优先，兼容转化牌）。
pub fn card_view(st: &SgsState, id: &str) -> Option<CardView> {
    let inst = st.cards.get(id)?;
    let code = st.effective_def_code(id).unwrap_or(&inst.def).to_string();
    let def = data::card_def(&code);
    Some(CardView {
        id: id.to_string(),
        name: def.map(|d| d.name.to_string()).unwrap_or_else(|| code.clone()),
        code,
        suit: inst.suit,
        rank: inst.rank,
        category: def.map(|d| d.category),
        equip_slot: def.and_then(|d| d.equip_slot),
        delayed: def.map(|d| d.delayed).unwrap_or(false),
    })
}

fn card_views(st: &SgsState, ids: &[String]) -> Vec<CardView> {
    ids.iter().filter_map(|id| card_view(st, id)).collect()
}

fn equips_view(st: &SgsState, seat: &crate::state::Seat) -> Vec<CardView> {
    let mut out = Vec::new();
    for slot in EquipSlot::ALL {
        if let Some(Some(id)) = seat.equips.get(&slot) {
            if let Some(v) = card_view(st, id) {
                out.push(v);
            }
        }
    }
    out
}

fn pending_view(_st: &SgsState, p: &Pending, viewer: &str) -> PendingView {
    let mine = p.actor == viewer;
    PendingView {
        kind: p.kind.clone(),
        actor: p.actor.clone(),
        skill: p.skill.clone(),
        options: if mine {
            p.options
                .iter()
                .map(|o| (o.id.clone(), o.label.clone()))
                .collect()
        } else {
            Vec::new()
        },
        data: if mine { p.data.clone() } else { Value::Null },
    }
}

fn visible_zones(st: &SgsState, viewer: &str) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    for (id, zone) in &st.special_zones {
        let visible = zone.visibility == Visibility::Public
            || zone.owner.as_deref() == Some(viewer);
        if !visible {
            continue;
        }
        let mut obj = serde_json::Map::new();
        if let Some(owner) = &zone.owner {
            obj.insert("owner".into(), Value::String(owner.clone()));
        }
        for (k, v) in &zone.payload {
            obj.insert(k.clone(), v.clone());
        }
        out.insert(id.clone(), Value::Object(obj));
    }
    out
}

/// 从特殊区亮出的牌里收集已知牌面（如五谷丰登 `cards`）。
fn collect_zone_cards(zones: &BTreeMap<String, Value>, out: &mut BTreeMap<String, CardView>) {
    for zone in zones.values() {
        let Some(obj) = zone.as_object() else { continue };
        for key in ["cards", "revealed"] {
            let Some(arr) = obj.get(key).and_then(Value::as_array) else {
                continue;
            };
            for item in arr {
                let Some(id) = item.get("id").and_then(Value::as_str) else {
                    continue;
                };
                let Some(code) = item.get("def").and_then(Value::as_str) else {
                    continue;
                };
                let def = data::card_def(code);
                let suit = item
                    .get("suit")
                    .and_then(|v| serde_json::from_value::<Suit>(v.clone()).ok())
                    .unwrap_or(Suit::Spade);
                out.insert(
                    id.to_string(),
                    CardView {
                        id: id.to_string(),
                        code: code.to_string(),
                        name: def
                            .map(|d| d.name.to_string())
                            .unwrap_or_else(|| code.to_string()),
                        suit,
                        rank: item.get("rank").and_then(Value::as_u64).unwrap_or(0) as u8,
                        category: def.map(|d| d.category),
                        equip_slot: def.and_then(|d| d.equip_slot),
                        delayed: def.map(|d| d.delayed).unwrap_or(false),
                    },
                );
            }
        }
    }
}

impl SgsView {
    /// 投影出 `viewer` 的可见信息；`viewer` 不在座时返回 `None`（如大厅）。
    pub fn from_state(st: &SgsState, viewer: &str) -> Option<Self> {
        let me = st.seat_index(viewer)?;
        let mine = &st.seats[me];

        let seats: Vec<SeatView> = st
            .seats
            .iter()
            .enumerate()
            .map(|(i, s)| SeatView {
                index: i,
                player_id: s.player_id.clone(),
                general: s.general.clone(),
                general_name: if s.general_name.is_empty() {
                    s.general
                        .as_deref()
                        .and_then(data::general)
                        .map(|g| g.name.to_string())
                        .unwrap_or_default()
                } else {
                    s.general_name.clone()
                },
                // 公开身份：主公 / 已死亡 / 本人；其余隐藏。
                identity: if s.identity.is_lord() || !s.alive || i == me {
                    Some(s.identity)
                } else {
                    None
                },
                hp: s.hp,
                max_hp: s.max_hp,
                hand_count: s.hand.len(),
                hand_limit: crate::skills::hand_limit(st, i),
                equips: equips_view(st, s),
                judge_zone: card_views(st, &s.judge_zone),
                marks_count: s
                    .mark_counts
                    .iter()
                    .filter(|(_, n)| **n > 0)
                    .map(|(k, v)| (k.clone(), *v))
                    .collect(),
                alive: s.alive,
                flipped: s.flipped,
                chained: s.chained,
                is_bot: s.is_bot,
            })
            .collect();

        let special_zones = visible_zones(st, viewer);
        let mut known_cards: BTreeMap<String, CardView> = BTreeMap::new();
        for (i, seat) in st.seats.iter().enumerate() {
            // 本人手牌 + 全场公开的装备/判定区。
            let ids: Vec<String> = if i == me {
                seat.hand.iter().cloned().collect()
            } else {
                Vec::new()
            };
            for id in ids {
                if let Some(v) = card_view(st, &id) {
                    known_cards.insert(id, v);
                }
            }
            for slot in EquipSlot::ALL {
                if let Some(Some(id)) = seat.equips.get(&slot) {
                    if let Some(v) = card_view(st, id) {
                        known_cards.insert(id.clone(), v);
                    }
                }
            }
            for id in &seat.judge_zone {
                if let Some(v) = card_view(st, id) {
                    known_cards.insert(id.clone(), v);
                }
            }
        }
        collect_zone_cards(&special_zones, &mut known_cards);

        Some(Self {
            me,
            me_id: viewer.to_string(),
            identity: mine.identity,
            general: mine.general.clone(),
            phase: st.phase,
            turn: st.turn,
            turn_phase: st.turn_phase,
            round: st.round,
            hand: card_views(st, &mine.hand),
            equips: equips_view(st, mine),
            judge_zone: card_views(st, &mine.judge_zone),
            seats,
            pending: st.pending.as_ref().map(|p| pending_view(st, p, viewer)),
            special_zones,
            observations: st.observations.clone(),
            logs: st.history.iter().rev().take(48).cloned().collect::<Vec<_>>().into_iter().rev().collect(),
            deck_remaining: st.deck.draw.len(),
            discard_top: st.deck.discard.last().and_then(|id| card_view(st, id)),
            known_cards,
        })
    }

    /// 本人座位。
    pub fn me_seat(&self) -> &SeatView {
        &self.seats[self.me]
    }

    /// 存活座位下标（含本人）。
    pub fn alive_indices(&self) -> Vec<usize> {
        self.seats
            .iter()
            .filter(|s| s.alive)
            .map(|s| s.index)
            .collect()
    }

    /// 按 player_id 找座位。
    pub fn seat_of(&self, player_id: &str) -> Option<&SeatView> {
        self.seats.iter().find(|s| s.player_id == player_id)
    }

    /// 手牌里各牌型的数量。
    pub fn hand_count_of(&self, pred: impl Fn(&str) -> bool) -> usize {
        self.hand.iter().filter(|c| pred(&c.code)).count()
    }

    /// 当前 pending 的类型（无则空串）。
    pub fn pending_kind(&self) -> &str {
        self.pending.as_ref().map(|p| p.kind.as_str()).unwrap_or("")
    }

    /// 当前 pending 的 actor 是否本人。
    pub fn pending_is_mine(&self) -> bool {
        self.pending
            .as_ref()
            .map(|p| p.actor == self.me_id)
            .unwrap_or(false)
    }
}
