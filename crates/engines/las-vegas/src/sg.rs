//! 小游戏（Small Games，A1–H2）子状态与效果。
//!
//! 阶段 3a：框架 + 设置摆放（F1/F2/G1）+ 自动类（C1/D1/A2）+ 结算钩子（B1/C2）。
//! 交互类（A1/B2/D2/E1/E2/F1/F2/G1/G2/H1/H2）留待 3b（`pending` 已预留）。
//!
//! 数值口径：用户规则文本为准；文本未给出的用下列常量并在文档标注【待确认】。

use crate::{LasVegasState, CASINOS};
use game_engine_core::{roll_die, Action, EngineEvent, EngineError, RandomSource};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

/// C2 厄运降临：最少骰子者向银行支付。
pub const C2_PENALTY: i64 = 50_000;
/// C1 掌声响起：大奖标记结算兑换。
pub const C1_REWARD: i64 = 100_000;
/// A2 大奖挑战：指示物各位置奖励（筹码）。【待确认：规则文本未给档位】
pub const A2_LADDER: [u32; 6] = [1, 2, 3, 4, 5, 6];
/// F1 阻止他：5 组灰骰图案。【待确认：图片约 3/3/3/1/1，规则文本记 9 颗】
pub const F1_GROUPS: [u32; 5] = [3, 2, 2, 1, 1];
/// G1 黑色摸彩箱：6 个标记金额（单位 $万）。【待确认：据攻略 2/2/6/0/0/0】
pub const G1_LOTTERY: [u32; 6] = [2, 2, 6, 0, 0, 0];
/// A1 幸运之拳：手心标记数 → 奖励（筹码）。【待确认】
pub const A1_LADDER: [u32; 6] = [1, 2, 3, 4, 5, 6];
/// B2 50%：每次猜对奖励（筹码）。【待确认】
pub const B2_REWARD: u32 = 1;
/// E1 禁止进入：箭头各位置奖励（筹码）。【待确认】
pub const E1_ARROW_LADDER: [u32; 6] = [1, 2, 3, 4, 5, 6];
/// G2 双倍颁奖：游戏场第一 / 第二奖励（钞票）。
pub const G2_FIRST: u32 = 60_000;
pub const G2_SECOND: u32 = 30_000;
/// H2 我的抉择：金框结算奖励（钞票）。
pub const H2_GOLD: u32 = 60_000;
/// H1 好骰难求：6 个格位奖励（筹码）。【待确认】
pub const H1_LADDER: [u32; 6] = [1, 2, 3, 3, 4, 6];

/// 待处理的小游戏交互（3b 用；3a 不产生）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SgPending {
    pub game: String,
    pub casino: usize,
    pub actor: String,
    pub stage: String,
    #[serde(default)]
    pub data: serde_json::Value,
}

/// 小游戏子状态（每轮重置；随 `LasVegasState` 序列化）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SgState {
    /// F2：每个赌场的灰骰数。
    #[serde(default)]
    pub gray: Vec<u32>,
    /// F1：5 组灰骰（未放入赌场）。
    #[serde(default)]
    pub gray_groups: Vec<u32>,
    /// E2：酒吧（玩家 → 骰数）。
    #[serde(default)]
    pub bars: BTreeMap<String, u32>,
    /// G2：游戏场（玩家 → 骰数）。
    #[serde(default)]
    pub game_field: BTreeMap<String, u32>,
    /// H2：金框（占用者 → 骰数）。
    #[serde(default)]
    pub gold_frame: Option<(String, u32)>,
    /// E1：禁止标记所在赌场（0-based）。
    #[serde(default)]
    pub banned: Option<usize>,
    /// E1：箭头标记位置。
    #[serde(default)]
    pub arrow: usize,
    /// G1：摸彩标记金额。
    #[serde(default)]
    pub lottery: Vec<u32>,
    /// A2：指示物位置。
    #[serde(default)]
    pub indicator: usize,
    /// C1：大奖标记（玩家 → 数量）。
    #[serde(default)]
    pub grand_prize: BTreeMap<String, u32>,
    /// D2：特权标记持有者。
    #[serde(default)]
    pub privilege: Option<String>,
    /// H1：6 个格位（占用者 + 骰数）。
    #[serde(default)]
    pub h1_slots: Vec<Option<(String, u32)>>,
    /// G2：本回合是否已移入游戏场。
    #[serde(default)]
    pub g2_used: bool,
    /// 大厅指定的开局小游戏（仅第 1 轮生效；start 后清空）。
    #[serde(default)]
    pub small_games_override: Option<Vec<String>>,
    /// H1：本回合是否已用「移 1 颗骰到格」（规则每回合 1 颗）。
    #[serde(default)]
    pub h1_used_turn: bool,
    /// B1：本轮结算前是否已执行（防暂停/恢复时重复加骰）。
    #[serde(default)]
    pub b1_done: bool,
    /// G1：本轮结算前是否已执行（每轮只执行一次）。
    #[serde(default)]
    pub g1_done: bool,
    /// 待处理交互（3b）。
    #[serde(default)]
    pub pending: Option<SgPending>,
}

impl SgState {
    /// 每轮设置：清空并按抽中的小游戏重摆灰骰/摸彩/指示物。
    pub fn setup_round(&mut self, tiles: &[String]) {
        *self = SgState {
            gray: vec![0; CASINOS],
            ..Default::default()
        };
        for t in tiles {
            match t.as_str() {
                "F1" => self.gray_groups = F1_GROUPS.to_vec(),
                "F2" => {
                    for c in 0..3.min(CASINOS) {
                        self.gray[c] += 1;
                    }
                    for c in 3..CASINOS {
                        self.gray[c] += 2;
                    }
                }
                "G1" => self.lottery = G1_LOTTERY.to_vec(),
                "H1" => self.h1_slots = vec![None; 6],
                _ => {}
            }
        }
    }

    /// 该赌场挂载的小游戏编号列表。
    pub fn tiles_at(&self, st: &LasVegasState, casino: usize) -> Vec<String> {
        st.casinos
            .get(casino)
            .map(|c| c.small_games.clone())
            .unwrap_or_default()
    }
}

/// 座次上玩家左边的人（前一位）。
pub(crate) fn left_player(st: &LasVegasState, player: &str) -> String {
    let n = st.seats.len();
    if n == 0 {
        return player.to_string();
    }
    let idx = st
        .seats
        .iter()
        .position(|s| s.player_id == player)
        .unwrap_or(0);
    st.seats[(idx + n - 1) % n].player_id.clone()
}

fn add_chips(st: &mut LasVegasState, p: &str, n: u32) {
    if let Some(s) = st.seats.iter_mut().find(|s| s.player_id == p) {
        s.chips += n;
    }
}

fn add_note(st: &mut LasVegasState, p: &str, v: u32) {
    if let Some(s) = st.seats.iter_mut().find(|s| s.player_id == p) {
        s.banknotes.push(v);
    }
}

fn add_casino_dice(st: &mut LasVegasState, casino: usize, p: &str, w: u32) {
    *st.casinos[casino].dice.entry(p.to_string()).or_insert(0) += w;
}

/// H1：占用某格（格内他人骰子移到对应点数赌场）。
fn h1_slot_put(st: &mut LasVegasState, player: &str, slot: usize) {
    if st.sg.h1_slots.len() < 6 {
        st.sg.h1_slots = vec![None; 6];
    }
    match st.sg.h1_slots[slot - 1].clone() {
        Some((o, n)) if o == player => {
            st.sg.h1_slots[slot - 1] = Some((o, n + 1));
        }
        Some((o, n)) => {
            add_casino_dice(st, slot - 1, &o, n);
            st.sg.h1_slots[slot - 1] = Some((player.to_string(), 1));
        }
        None => st.sg.h1_slots[slot - 1] = Some((player.to_string(), 1)),
    }
}

/// D2 权力优势：某赌场唯一最多者持有特权标记；并列则回场。
fn update_privilege(st: &mut LasVegasState, casino: usize) {
    if !st.casinos[casino].small_games.iter().any(|g| g == "D2") {
        return;
    }
    let mut counts: Vec<(String, u32)> = st
        .seats
        .iter()
        .map(|s| {
            (
                s.player_id.clone(),
                st.casinos[casino].dice.get(&s.player_id).copied().unwrap_or(0),
            )
        })
        .collect();
    counts.sort_by(|a, b| b.1.cmp(&a.1));
    let leader = counts.first().cloned();
    let tied = counts.len() > 1 && counts[1].1 == leader.as_ref().map(|x| x.1).unwrap_or(0);
    st.sg.privilege = match leader {
        Some((p, c)) if c > 0 && !tied => Some(p),
        _ => None,
    };
}

/// 下注后触发的自动小游戏（C1/D1/A2）与交互类触发。
pub(crate) fn on_place(
    st: &mut LasVegasState,
    casino: usize,
    player: &str,
    rng: &mut dyn RandomSource,
    events: &mut Vec<EngineEvent>,
) {
    let tiles = st.casinos[casino].small_games.clone();
    for t in tiles {
        match t.as_str() {
            // C1 掌声响起：在某赌场放第 5 个骰子 → 得大奖标记（结算兑换 $10 万）。
            "C1" => {
                let c = st.casinos[casino].dice.get(player).copied().unwrap_or(0);
                if c == 5 {
                    *st.sg.grand_prize.entry(player.to_string()).or_insert(0) += 1;
                    events.push(EngineEvent::new(
                        "small_game",
                        json!({ "game": "C1", "player": player, "casino": casino + 1, "effect": "grand_prize" }),
                    ));
                }
            }
            // D1 发薪愉快：统计有自己骰子的赌场数（含当前）→ 1/2 筹码，>=3 得 count×$1万钞票。
            "D1" => {
                let count = st
                    .casinos
                    .iter()
                    .filter(|c| c.dice.get(player).copied().unwrap_or(0) > 0)
                    .count() as u32;
                let desc = if count <= 2 {
                    if let Some(s) = st.seats.iter_mut().find(|s| s.player_id == player) {
                        s.chips += count;
                    }
                    json!({ "game": "D1", "player": player, "casinos": count, "chips": count })
                } else {
                    let note = count * 10_000;
                    if let Some(s) = st.seats.iter_mut().find(|s| s.player_id == player) {
                        s.banknotes.push(note);
                    }
                    json!({ "game": "D1", "player": player, "casinos": count, "note": note })
                };
                events.push(EngineEvent::new("small_game", desc));
            }
            // A2 大奖挑战：投 2 黑骰，和=7 或同点 → 拿指示物奖励并回位；否则指示物前进一格。
            "A2" => {
                let a = roll_die(rng);
                let b = roll_die(rng);
                let hit = a == b || a + b == 7;
                if hit {
                    let reward = A2_LADDER
                        .get(st.sg.indicator % A2_LADDER.len())
                        .copied()
                        .unwrap_or(0);
                    if let Some(s) = st.seats.iter_mut().find(|s| s.player_id == player) {
                        s.chips += reward;
                    }
                    st.sg.indicator = 0;
                    events.push(EngineEvent::new(
                        "small_game",
                        json!({ "game": "A2", "player": player, "black": [a, b], "hit": true, "chips": reward }),
                    ));
                } else {
                    st.sg.indicator += 1;
                    events.push(EngineEvent::new(
                        "small_game",
                        json!({ "game": "A2", "player": player, "black": [a, b], "hit": false, "indicator": st.sg.indicator }),
                    ));
                }
            }
            _ => {}
        }
    }
    update_privilege(st, casino);
    trigger_interactive(st, casino, player, rng, events);
}

/// 交互类小游戏触发（下注后）：设置 pending（一次只挂一个）。
pub(crate) fn trigger_interactive(
    st: &mut LasVegasState,
    casino: usize,
    player: &str,
    rng: &mut dyn RandomSource,
    events: &mut Vec<EngineEvent>,
) {
    if st.sg.pending.is_some() {
        return;
    }
    let tiles = st.casinos[casino].small_games.clone();
    for t in tiles {
        let pending = match t.as_str() {
            "E2" => {
                e2_effect(st, player, events);
                None
            }
            "A1" => {
                let n = roll_die(rng) as u32;
                Some(SgPending {
                    game: "A1".into(),
                    casino,
                    actor: left_player(st, player),
                    stage: "guess".into(),
                    data: json!({ "initiator": player, "n": n }),
                })
            }
            "B2" => {
                let a = roll_die(rng) as u32;
                let b = roll_die(rng) as u32;
                Some(SgPending {
                    game: "B2".into(),
                    casino,
                    actor: player.to_string(),
                    stage: "guess".into(),
                    data: json!({ "prev": a + b }),
                })
            }
            "E1" => Some(SgPending {
                game: "E1".into(),
                casino,
                actor: player.to_string(),
                stage: "choose".into(),
                data: json!({}),
            }),
            "F1" => {
                if st.sg.gray_groups.iter().any(|g| *g > 0) {
                    Some(SgPending {
                        game: "F1".into(),
                        casino,
                        actor: player.to_string(),
                        // 两轮交互：先选组（group），再选赌场（casino）
                        stage: "group".into(),
                        data: json!({}),
                    })
                } else {
                    None
                }
            }
            "F2" => {
                if st.sg.gray.iter().any(|g| *g > 0) {
                    Some(SgPending {
                        game: "F2".into(),
                        casino,
                        actor: player.to_string(),
                        stage: "take".into(),
                        data: json!({}),
                    })
                } else {
                    None
                }
            }
            "H2" => {
                let a = roll_die(rng) as u32;
                let b = roll_die(rng) as u32;
                Some(SgPending {
                    game: "H2".into(),
                    casino,
                    actor: player.to_string(),
                    stage: "choose".into(),
                    data: json!({ "dice": [a, b] }),
                })
            }
            // H1 好骰难求：刚在该场下注后，可选择把 1 颗已下注骰移入格子（可选，可跳过）。
            "H1" => {
                let c = st.casinos[casino].dice.get(player).copied().unwrap_or(0);
                if c > 0 && !st.sg.h1_used_turn {
                    Some(SgPending {
                        game: "H1".into(),
                        casino,
                        actor: player.to_string(),
                        stage: "post_bet".into(),
                        data: json!({}),
                    })
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(p) = pending {
            events.push(EngineEvent::new(
                "small_game_pending",
                json!({ "game": p.game, "actor": p.actor, "casino": casino + 1 }),
            ));
            st.sg.pending = Some(p);
            return;
        }
    }
}

/// E2 酩酊大醉（自动）：其他玩家各放 1 颗入手骰到酒吧（上限 2），触发者收回自己酒吧骰。
fn e2_effect(st: &mut LasVegasState, initiator: &str, events: &mut Vec<EngineEvent>) {
    let others: Vec<String> = st.seats.iter().map(|s| s.player_id.clone()).collect();
    for p in others {
        if p == initiator {
            continue;
        }
        let bar = st.sg.bars.get(&p).copied().unwrap_or(0);
        if bar >= 2 {
            continue;
        }
        let seat = st.seats.iter_mut().find(|s| s.player_id == p).unwrap();
        if seat.small_remaining > 0 {
            seat.small_remaining -= 1;
        } else if seat.big_remaining > 0 {
            seat.big_remaining -= 1;
        } else {
            continue;
        }
        *st.sg.bars.entry(p.clone()).or_insert(0) += 1;
    }
    let mine = st.sg.bars.remove(initiator).unwrap_or(0);
    if mine > 0 {
        if let Some(s) = st.seats.iter_mut().find(|s| s.player_id == initiator) {
            s.small_remaining += mine as u8;
        }
    }
    events.push(EngineEvent::new(
        "small_game",
        json!({ "game": "E2", "player": initiator, "bar_recalled": mine }),
    ));
}

/// pending 状态下该玩家的合法 `sg:*` 动作。
pub(crate) fn legal_for_pending(st: &LasVegasState, player: &str) -> Vec<Action> {
    let Some(p) = st.sg.pending.clone() else {
        return vec![];
    };
    if p.actor != player {
        return vec![];
    }
    match p.game.as_str() {
        "A1" => (1..=6)
            .map(|k| Action { id: format!("sg:A1:guess:{k}"), description: format!("猜 {k} 枚") })
            .collect(),
        "B2" => vec![
            Action { id: "sg:B2:guess:high".into(), description: "猜比上次大".into() },
            Action { id: "sg:B2:guess:low".into(), description: "猜比上次小".into() },
        ],
        "E1" => {
            let mut v: Vec<Action> = (1..=CASINOS)
                .filter(|c| *c != p.casino + 1)
                .map(|c| Action { id: format!("sg:E1:ban:{c}"), description: format!("禁止 {c} 号赌场") })
                .collect();
            v.push(Action { id: "sg:E1:pass".into(), description: "不改禁止标记".into() });
            v
        }
        "F1" => {
            if p.stage == "group" {
                st.sg
                    .gray_groups
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| **c > 0)
                    .map(|(g, c)| Action {
                        id: format!("sg:F1:group:{g}"),
                        description: format!("选第 {} 组（{c} 颗灰骰）", g + 1),
                    })
                    .collect()
            } else {
                let g = p.data.get("group").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let cnt = st.sg.gray_groups.get(g).copied().unwrap_or(0);
                (1..=CASINOS)
                    .map(|c| Action {
                        id: format!("sg:F1:move:{g}:{c}"),
                        description: format!("把第 {} 组的 {cnt} 颗灰骰放到 {c} 号赌场", g + 1),
                    })
                    .collect()
            }
        }
        "F2" => {
            let mut v = vec![];
            for (c, cnt) in st.sg.gray.iter().enumerate() {
                if *cnt > 0 {
                    v.push(Action { id: format!("sg:F2:take:{}:chip", c + 1), description: format!("取 {} 号灰骰换 1 筹码", c + 1) });
                    v.push(Action { id: format!("sg:F2:take:{}:money", c + 1), description: format!("取 {} 号灰骰换 $3万", c + 1) });
                }
            }
            v.push(Action { id: "sg:F2:pass".into(), description: "放弃".into() });
            v
        }
        "G1" => {
            if p.stage == "split" {
                let total = p.data.get("total").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                (0..=total)
                    .map(|a| Action { id: format!("sg:G1:split:{a}"), description: format!("分成 {a} / {} 万", total - a) })
                    .collect()
            } else {
                vec![
                    Action { id: "sg:G1:pick:0".into(), description: "选左堆".into() },
                    Action { id: "sg:G1:pick:1".into(), description: "选右堆".into() },
                ]
            }
        }
        "H2" => {
            let dice: Vec<u32> = p
                .data
                .get("dice")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            dice.into_iter()
                .map(|d| Action { id: format!("sg:H2:choose:{d}"), description: format!("选点数 {d}") })
                .collect()
        }
        "H1" => {
            let mut v: Vec<Action> = (1..=6)
                .map(|slot| Action {
                    id: format!("sg:H1:move:{slot}"),
                    description: format!("移 1 颗已下注骰到 {slot} 号格"),
                })
                .collect();
            v.push(Action { id: "sg:H1:skip".into(), description: "跳过好骰难求".into() });
            v
        }
        _ => vec![],
    }
}

/// 解析一个 `sg:*` 交互动作；返回（是否完成，事件，文案）。未完成时 pending 已更新。
pub(crate) fn resolve(
    st: &mut LasVegasState,
    action: &str,
    player: &str,
    rng: &mut dyn RandomSource,
) -> Result<(bool, Vec<EngineEvent>, String), EngineError> {
    let Some(p) = st.sg.pending.clone() else {
        return Err(EngineError::IllegalAction("当前没有小游戏交互".into()));
    };
    if p.actor != player {
        return Err(EngineError::PermissionDenied("未轮到你在小游戏中操作".into()));
    }
    let rest = action
        .strip_prefix("sg:")
        .ok_or_else(|| EngineError::IllegalAction("非法小游戏动作".into()))?;
    let mut events = Vec::new();
    let msg;
    match p.game.as_str() {
        "A1" => {
            let k: u32 = rest
                .strip_prefix("A1:guess:")
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| EngineError::IllegalAction("A1 参数错误".into()))?;
            let n = p.data.get("n").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
            let initiator = p.data.get("initiator").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if k == n {
                msg = format!("猜对了，{initiator} 无奖励");
            } else {
                let r = A1_LADDER[(n as usize - 1).min(A1_LADDER.len() - 1)];
                add_chips(st, &initiator, r);
                msg = format!("猜错了，{initiator} 得 {r} 筹码");
            }
            events.push(EngineEvent::new("small_game", json!({ "game": "A1", "guess": k, "n": n })));
            Ok((true, events, msg))
        }
        "B2" => {
            let high = rest.ends_with("high");
            let a = roll_die(rng) as u32;
            let b = roll_die(rng) as u32;
            let new = a + b;
            let prev = p.data.get("prev").and_then(|v| v.as_u64()).unwrap_or(7) as u32;
            let correct = (high && new > prev) || (!high && new < prev);
            if correct {
                add_chips(st, player, B2_REWARD);
                msg = format!("掷出 {new}，猜对，得 {B2_REWARD} 筹码");
            } else {
                msg = format!("掷出 {new}（上次 {prev}），猜错，无奖励");
            }
            events.push(EngineEvent::new("small_game", json!({ "game": "B2", "black": [a, b], "correct": correct })));
            Ok((true, events, msg))
        }
        "E1" => {
            if rest == "E1:pass" {
                Ok((true, events, "未改变禁止标记".into()))
            } else {
                let c: usize = rest
                    .strip_prefix("E1:ban:")
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| EngineError::IllegalAction("E1 参数错误".into()))?;
                if c < 1 || c > CASINOS || c - 1 == p.casino {
                    return Err(EngineError::IllegalAction("禁止标记位置非法".into()));
                }
                st.sg.banned = Some(c - 1);
                let reward = E1_ARROW_LADDER[st.sg.arrow % E1_ARROW_LADDER.len()];
                add_chips(st, player, reward);
                st.sg.arrow = (st.sg.arrow + 1) % E1_ARROW_LADDER.len();
                events.push(EngineEvent::new("small_game", json!({ "game": "E1", "banned": c, "reward": reward })));
                Ok((true, events, format!("禁止 {c} 号赌场，箭头移动得 {reward} 筹码")))
            }
        }
        "F1" => {
            // 第一轮：选组 → 进入第二轮（选赌场）
            if let Some(gstr) = rest.strip_prefix("F1:group:") {
                let g: usize = gstr
                    .parse()
                    .map_err(|_| EngineError::IllegalAction("F1 组号错误".into()))?;
                if g >= st.sg.gray_groups.len() || st.sg.gray_groups[g] == 0 {
                    return Err(EngineError::IllegalAction("该组灰骰不存在或已空".into()));
                }
                let mut data = p.data.clone();
                data["group"] = json!(g);
                st.sg.pending = Some(SgPending {
                    game: "F1".into(),
                    casino: p.casino,
                    actor: p.actor.clone(),
                    stage: "casino".into(),
                    data,
                });
                let cnt = st.sg.gray_groups[g];
                return Ok((
                    false,
                    events,
                    format!("已选第 {} 组（{cnt} 颗），请选择要放入的赌场", g + 1),
                ));
            }
            let parts: Vec<&str> = rest.trim_start_matches("F1:move:").split(':').collect();
            let g: usize = parts.first().and_then(|s| s.parse().ok()).unwrap_or(usize::MAX);
            let c: usize = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
            if g >= st.sg.gray_groups.len() || c < 1 || c > CASINOS {
                return Err(EngineError::IllegalAction("F1 参数错误".into()));
            }
            let cnt = st.sg.gray_groups[g];
            if cnt == 0 {
                return Err(EngineError::IllegalAction("该组灰骰已空".into()));
            }
            st.sg.gray_groups[g] = 0;
            add_casino_dice(st, c - 1, "gray", cnt);
            events.push(EngineEvent::new("small_game", json!({ "game": "F1", "group": g, "casino": c, "count": cnt })));
            Ok((true, events, format!("把 {cnt} 颗灰骰放到 {c} 号赌场")))
        }
        "F2" => {
            if rest == "F2:pass" {
                return Ok((true, events, "放弃危机就是转机".into()));
            }
            let parts: Vec<&str> = rest.trim_start_matches("F2:take:").split(':').collect();
            let c: usize = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
            let kind = parts.get(1).copied().unwrap_or("chip");
            if c < 1 || c > CASINOS || st.sg.gray[c - 1] == 0 {
                return Err(EngineError::IllegalAction("F2 参数错误".into()));
            }
            st.sg.gray[c - 1] -= 1;
            if kind == "money" {
                add_note(st, player, 30_000);
                msg = "取灰骰换 $3万".into();
            } else {
                add_chips(st, player, 1);
                msg = "取灰骰换 1 筹码".into();
            }
            events.push(EngineEvent::new("small_game", json!({ "game": "F2", "casino": c, "reward": kind })));
            Ok((true, events, msg))
        }
        "G1" => {
            if p.stage == "split" {
                let a: u32 = rest
                    .strip_prefix("G1:split:")
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| EngineError::IllegalAction("G1 参数错误".into()))?;
                let total = p.data.get("total").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                if a > total {
                    return Err(EngineError::IllegalAction("分堆金额非法".into()));
                }
                let winner = p.data.get("winner").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let data = json!({ "winner": winner, "total": total, "piles": [a, total - a] });
                st.sg.pending = Some(SgPending { game: "G1".into(), casino: p.casino, actor: winner.clone(), stage: "pick".into(), data });
                Ok((false, events, format!("已分成 {a} / {} 万，等待 {winner} 选堆", total - a)))
            } else {
                let idx: usize = rest
                    .strip_prefix("G1:pick:")
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| EngineError::IllegalAction("G1 参数错误".into()))?;
                let piles: Vec<u32> = p.data.get("piles").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
                let gain = piles.get(idx).copied().unwrap_or(0);
                if gain > 0 {
                    add_note(st, player, gain * 10_000);
                }
                events.push(EngineEvent::new("small_game", json!({ "game": "G1", "pick": idx, "gain": gain })));
                Ok((true, events, format!("选到 {gain} 万")))
            }
        }
        "H2" => {
            let d: u32 = rest
                .strip_prefix("H2:choose:")
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| EngineError::IllegalAction("H2 参数错误".into()))?;
            msg = match d {
                1 => { add_chips(st, player, 1); "得 1 筹码".into() }
                2 => { add_chips(st, player, 2); "得 2 筹码".into() }
                3 => { add_note(st, player, 30_000); "得 $3万".into() }
                4 => { add_chips(st, player, 1); "触发其他小游戏（简化为 1 筹码）".into() }
                5 => "调整 1 颗骰子（简化：无效果）".into(),
                6 => {
                    if let Some((o, n)) = st.sg.gold_frame.take() {
                        if o != player {
                            if let Some(s) = st.seats.iter_mut().find(|s| s.player_id == o) {
                                s.small_remaining += n as u8;
                            }
                        }
                    }
                    st.sg.gold_frame = Some((player.to_string(), 1));
                    "放 1 颗骰子入金框（结算 $6万）".into()
                }
                _ => return Err(EngineError::IllegalAction("H2 点数须 1–6".into())),
            };
            events.push(EngineEvent::new("small_game", json!({ "game": "H2", "choose": d })));
            Ok((true, events, msg))
        }
        "H1" => {
            if rest == "H1:skip" {
                events.push(EngineEvent::new("small_game", json!({ "game": "H1", "player": player, "skip": true })));
                return Ok((true, events, "跳过好骰难求".into()));
            }
            let slot: usize = rest
                .strip_prefix("H1:move:")
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| EngineError::IllegalAction("H1 参数错误".into()))?;
            if !(1..=6).contains(&slot) {
                return Err(EngineError::IllegalAction("H1 格位非法".into()));
            }
            let cnt = st.casinos[p.casino].dice.get(player).copied().unwrap_or(0);
            if cnt == 0 {
                return Err(EngineError::IllegalAction("该赌场没有你的骰子".into()));
            }
            if cnt == 1 {
                st.casinos[p.casino].dice.remove(player);
            } else if let Some(d) = st.casinos[p.casino].dice.get_mut(player) {
                *d -= 1;
            }
            h1_slot_put(st, player, slot);
            st.sg.h1_used_turn = true;
            events.push(EngineEvent::new(
                "small_game",
                json!({ "game": "H1", "player": player, "slot": slot, "source": "bet" }),
            ));
            Ok((true, events, format!("移 1 颗已下注骰到 {slot} 号格")))
        }
        _ => Err(EngineError::IllegalAction("未知小游戏交互".into())),
    }
}

/// 回合内（非 pending）的小游戏动作：D2 特权 / G2 移游戏场 / H1 移格。
pub(crate) fn legal_turn_sg(st: &LasVegasState, player: &str) -> Vec<Action> {
    if st.phase != crate::Phase::TurnRoll {
        return vec![];
    }
    let Some(cur) = st.seats.get(st.turn) else {
        return vec![];
    };
    if cur.player_id != player {
        return vec![];
    }
    let mut out = vec![];
    if st.sg.privilege.as_deref() == Some(player) && cur.small_remaining > 0 {
        for n in 1..=CASINOS {
            out.push(Action { id: format!("sg:D2:use:{n}"), description: format!("用特权放 1 颗 {n} 点") });
        }
    }
    if !st.sg.g2_used {
        for ci in 0..CASINOS {
            if st.casinos[ci].small_games.iter().any(|g| g == "G2")
                && st.casinos[ci].dice.get(player).copied().unwrap_or(0) > 0
            {
                out.push(Action { id: format!("sg:G2:move:{}", ci + 1), description: format!("把 {} 号赌场自己骰子移入游戏场", ci + 1) });
            }
        }
    }
    out
}

/// 解析回合内小游戏动作；返回（事件，文案，是否推进回合）。非本类动作返回 None。
pub(crate) fn resolve_turn_sg(
    st: &mut LasVegasState,
    action: &str,
    player: &str,
    _rng: &mut dyn RandomSource,
) -> Option<Result<(Vec<EngineEvent>, String, bool), EngineError>> {
    let rest = action.strip_prefix("sg:")?;
    let mut events = Vec::new();
    if let Some(nstr) = rest.strip_prefix("D2:use:") {
        let n: usize = nstr.parse().ok().unwrap_or(0);
        if !(1..=CASINOS).contains(&n) {
            return Some(Err(EngineError::IllegalAction("D2 点数非法".into())));
        }
        if st.sg.privilege.as_deref() != Some(player) {
            return Some(Err(EngineError::PermissionDenied("你没有特权标记".into())));
        }
        let Some(seat) = st.seats.get_mut(st.turn) else {
            return Some(Err(EngineError::Internal("回合下标越界".into())));
        };
        if seat.small_remaining == 0 {
            return Some(Err(EngineError::IllegalAction("没有可用骰子".into())));
        }
        seat.small_remaining -= 1;
        add_casino_dice(st, n - 1, player, 1);
        events.push(EngineEvent::new("small_game", json!({ "game": "D2", "player": player, "point": n })));
        return Some(Ok((events, format!("用特权把 1 颗 {n} 点骰放到 {n} 号赌场"), true)));
    }
    if let Some(cstr) = rest.strip_prefix("G2:move:") {
        let c: usize = cstr.parse().ok().unwrap_or(0);
        if !(1..=CASINOS).contains(&c) {
            return Some(Err(EngineError::IllegalAction("G2 参数非法".into())));
        }
        let cnt = st.casinos[c - 1].dice.get(player).copied().unwrap_or(0);
        if cnt == 0 {
            return Some(Err(EngineError::IllegalAction("该赌场没有你的骰子".into())));
        }
        st.casinos[c - 1].dice.remove(player);
        *st.sg.game_field.entry(player.to_string()).or_insert(0) += cnt;
        st.sg.g2_used = true;
        events.push(EngineEvent::new("small_game", json!({ "game": "G2", "player": player, "casino": c, "count": cnt })));
        return Some(Ok((events, format!("把 {cnt} 颗骰子移入游戏场（结算 $6万/$3万）"), false)));
    }
    None
}

/// 结算扩展：G2 游戏场、H2 金框、H1 格位。
pub(crate) fn settle_extras(st: &mut LasVegasState, events: &mut Vec<EngineEvent>) {
    let field: Vec<(String, u32)> = st.sg.game_field.iter().map(|(p, c)| (p.clone(), *c)).collect();
    if !field.is_empty() {
        let mut by_value: BTreeMap<u32, Vec<String>> = BTreeMap::new();
        for (p, c) in &field {
            if *c > 0 {
                by_value.entry(*c).or_default().push(p.clone());
            }
        }
        let mut kept: Vec<(String, u32)> = field
            .iter()
            .filter(|(_p, c)| *c > 0 && by_value.get(c).map(|v| v.len() == 1).unwrap_or(false))
            .map(|(p, c)| (p.clone(), *c))
            .collect();
        kept.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        if let Some((p, _)) = kept.first() {
            add_note(st, p, G2_FIRST);
        }
        if let Some((p, _)) = kept.get(1) {
            add_note(st, p, G2_SECOND);
        }
        events.push(EngineEvent::new("small_game", json!({ "game": "G2", "field": kept })));
    }
    st.sg.game_field.clear();
    if let Some((owner, n)) = st.sg.gold_frame.clone() {
        if n > 0 {
            add_note(st, &owner, H2_GOLD);
            events.push(EngineEvent::new("small_game", json!({ "game": "H2", "gold": owner, "reward": H2_GOLD })));
        }
    }
    st.sg.gold_frame = None;
    for slot in 0..st.sg.h1_slots.len() {
        if let Some((owner, n)) = st.sg.h1_slots[slot].clone() {
            if n > 0 {
                let reward = H1_LADDER[slot % H1_LADDER.len()] * n;
                add_chips(st, &owner, reward);
                events.push(EngineEvent::new("small_game", json!({ "game": "H1", "slot": slot + 1, "owner": owner, "reward": reward })));
            }
        }
    }
    st.sg.h1_slots = vec![None; 6];
}

/// 结算前的 B1 主场时间：对应赌场「将获最大面额」者投 2 黑骰，视为其骰子放入对应点数赌场。
pub(crate) fn b1_pre_settle(
    st: &mut LasVegasState,
    rng: &mut dyn RandomSource,
    events: &mut Vec<EngineEvent>,
) {
    if st.sg.b1_done {
        return;
    }
    st.sg.b1_done = true;
    for ci in 0..CASINOS {
        if !st.casinos[ci].small_games.iter().any(|g| g == "B1") {
            continue;
        }
        let Some(winner) = current_top(st, ci) else {
            continue;
        };
        let a = roll_die(rng);
        let b = roll_die(rng);
        for d in [a, b] {
            *st.casinos[(d - 1) as usize]
                .dice
                .entry(winner.clone())
                .or_insert(0) += 1;
        }
        events.push(EngineEvent::new(
            "small_game",
            json!({ "game": "B1", "casino": ci + 1, "player": winner, "black": [a, b] }),
        ));
    }
}

/// G1 结算前准备：找 G1 所在赌场，按「移除平手后」排名取**最高名次玩家**（灰骰不领奖）作为赢家。
/// 返回（赌场下标，赢家，摸彩总额万）。
pub(crate) fn g1_prepare(st: &LasVegasState) -> Option<(usize, String, u32)> {
    let total: u32 = st.sg.lottery.iter().sum();
    if total == 0 {
        return None;
    }
    for ci in 0..CASINOS {
        if !st.casinos[ci].small_games.iter().any(|g| g == "G1") {
            continue;
        }
        let mut by_value: BTreeMap<u32, Vec<String>> = BTreeMap::new();
        for (p, c) in &st.casinos[ci].dice {
            if *c > 0 {
                by_value.entry(*c).or_default().push(p.clone());
            }
        }
        let mut kept: Vec<(String, u32)> = st.casinos[ci]
            .dice
            .iter()
            .filter(|(p, c)| {
                **c > 0
                    && *p != "gray"
                    && by_value.get(c).map(|v| v.len() == 1).unwrap_or(false)
            })
            .map(|(p, c)| (p.clone(), *c))
            .collect();
        kept.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        if let Some((winner, _)) = kept.first() {
            return Some((ci, winner.clone(), total));
        }
        return None;
    }
    None
}

/// 启动 G1 交互（结算前）：左手玩家分堆。
pub(crate) fn start_g1_pending(
    st: &mut LasVegasState,
    casino: usize,
    winner: &str,
    total: u32,
    events: &mut Vec<EngineEvent>,
) {
    let splitter = left_player(st, winner);
    st.sg.pending = Some(SgPending {
        game: "G1".into(),
        casino,
        actor: splitter.clone(),
        stage: "split".into(),
        data: json!({ "winner": winner, "total": total }),
    });
    events.push(EngineEvent::new(
        "small_game_pending",
        json!({ "game": "G1", "actor": splitter, "casino": casino + 1, "winner": winner, "total": total }),
    ));
}

/// 单个赌场结算前（移除平手前）的 C2 厄运降临：最少骰子者（含 0）付 $5 万，不足付全部资产。
pub(crate) fn c2_pre_casino(
    st: &mut LasVegasState,
    casino: usize,
    events: &mut Vec<EngineEvent>,
) {
    if !st.casinos[casino].small_games.iter().any(|g| g == "C2") {
        return;
    }
    let counts: Vec<(String, u32)> = st
        .seats
        .iter()
        .map(|s| {
            (
                s.player_id.clone(),
                st.casinos[casino].dice.get(&s.player_id).copied().unwrap_or(0),
            )
        })
        .collect();
    let min = counts.iter().map(|x| x.1).min().unwrap_or(0);
    let payers: Vec<String> = counts
        .into_iter()
        .filter(|(_, c)| *c == min)
        .map(|(p, _)| p)
        .collect();
    for p in &payers {
        let paid = pay_assets(st, p, C2_PENALTY);
        events.push(EngineEvent::new(
            "small_game",
            json!({ "game": "C2", "casino": casino + 1, "player": p, "paid": paid }),
        ));
    }
}

/// 结算后：C1 大奖标记兑换为钞票。
pub(crate) fn post_settle(st: &mut LasVegasState, events: &mut Vec<EngineEvent>) {
    let prizes: Vec<(String, u32)> = st
        .sg
        .grand_prize
        .iter()
        .map(|(p, n)| (p.clone(), *n))
        .collect();
    for (p, n) in prizes {
        if n == 0 {
            continue;
        }
        for _ in 0..n {
            if let Some(s) = st.seats.iter_mut().find(|s| s.player_id == p) {
                s.banknotes.push(C1_REWARD as u32);
            }
        }
        events.push(EngineEvent::new(
            "small_game",
            json!({ "game": "C1", "player": p, "reward": C1_REWARD, "count": n }),
        ));
    }
}

/// 取某赌场「移除平手后」骰数最多者（B1 用）。
fn current_top(st: &LasVegasState, casino: usize) -> Option<String> {
    let mut by_value: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for (p, c) in &st.casinos[casino].dice {
        if *c > 0 {
            by_value.entry(*c).or_default().push(p.clone());
        }
    }
    let mut kept: Vec<(String, u32)> = st
        .casinos[casino]
        .dice
        .iter()
        .filter(|(_p, c)| **c > 0 && by_value.get(c).map(|v| v.len() == 1).unwrap_or(false))
        .map(|(p, c)| (p.clone(), *c))
        .collect();
    kept.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    kept.first().map(|x| x.0.clone())
}

/// 支付资产（先筹码，再面额从高到低的钞票；超出欠额由银行找零），不足付全部；返回实际支付额。
pub(crate) fn pay_assets(st: &mut LasVegasState, player: &str, amount: i64) -> i64 {
    let Some(seat) = st.seats.iter_mut().find(|s| s.player_id == player) else {
        return 0;
    };
    let mut owed = amount;
    let chip_value = crate::CHIP_VALUE;
    while owed >= chip_value && seat.chips > 0 {
        seat.chips -= 1;
        owed -= chip_value;
    }
    seat.banknotes.sort_unstable_by(|a, b| b.cmp(a));
    while owed > 0 {
        let Some(note) = seat.banknotes.first().copied() else {
            break;
        };
        seat.banknotes.remove(0);
        if (note as i64) <= owed {
            owed -= note as i64;
        } else {
            // 银行找零：把差额作为新钞票退回
            let change = note as i64 - owed;
            owed = 0;
            seat.banknotes.push(change as u32);
            seat.banknotes.sort_unstable_by(|a, b| b.cmp(a));
        }
    }
    amount - owed
}

/// 小游戏规则文本（A1–H2，大小写不敏感）。
pub fn rule_text(id: &str) -> Option<&'static str> {
    let id = id.trim().to_ascii_uppercase();
    Some(match id.as_str() {
        "A1" => "幸运之拳：下注后，你左边的玩家猜你手中标记数；猜错你按标记数得奖励，猜对无奖励。",
        "A2" => "大奖挑战：投 2 黑骰，和为 7 或点数相同 → 领指示物奖励并回位；否则指示物前进一格。",
        "B1" => "主场时间：每轮分配钞票前，将获该场最大面额者投 2 黑骰，按点数加骰后再结算。",
        "B2" => "50% 的机会：投 2 黑骰猜高于/低于上次结果；猜对得奖，猜错无奖励。",
        "C1" => "掌声响起：在该场放第 5 颗骰子者得大奖标记，结算时兑换 $10 万。",
        "C2" => "厄运降临：结算移除平手前，该场骰子最少者（含 0 颗）付 $5 万，不足付全部资产。",
        "D1" => "发薪愉快：下注后按你有骰子的赌场数：1→1 筹码、2→2 筹码、≥3→数量×$1 万钞票。",
        "D2" => "权力优势：该场骰数最多者得特权标记；可跳过投掷，放 1 颗指定点数骰。",
        "E1" => "禁止进入：下注后可改禁止标记（除当前场）并移动箭头得奖；被禁场不能放入/拿回骰子。",
        "E2" => "醉倒了吗：其他玩家各放 1 颗入手骰进酒吧（每人上限 2），你可收回自己酒吧骰。",
        "F1" => "阻止他：设置阶段放灰骰（5 组）；下注后先选 1 组、再选赌场放入；灰骰作为虚拟玩家参与排名。",
        "F2" => "危机就是转机：设置阶段 1–3 各 1、4–6 各 2 灰骰；下注后取 1 颗灰骰换奖励或放弃。",
        "G1" => "黑色摸彩箱：赢得该场最大面额者，由左手玩家把 6 个摸彩标记分 2 堆，你二选一领奖。",
        "G2" => "双倍颁奖：你的回合可把该场自己骰子移入游戏场（仅一次）；结算最多者 $6 万、第二 $3 万。",
        "H1" => "好骰难求：投掷/下注后移 1 颗骰到对应格；结算按格位得奖励。",
        "H2" => "我的抉择：下注后投 2 黑骰选 1 颗：1/2→筹码、3→$3 万、4→触发其他小游戏、5→放/收 1 颗、6→金框 $6 万。",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Casino, Phase, Roll, Seat, CHIP_VALUE};
    use game_engine_core::SeededRng;

    fn state_with_casinos(tiles: &[&str]) -> LasVegasState {
        let mut casinos: Vec<Casino> = (0..CASINOS).map(|_| Casino::empty()).collect();
        for (i, t) in tiles.iter().enumerate() {
            casinos[i % 3].small_games.push((*t).to_string());
        }
        LasVegasState {
            schema_version: 2,
            phase: Phase::TurnRoll,
            host: "h".into(),
            round: 1,
            start_player: Some("h".into()),
            turn: 0,
            seats: vec![Seat::new("h".into()), Seat::new("p2".into())],
            roll: Roll::default(),
            casinos,
            small_game_count: 3,
            scores: BTreeMap::new(),
            winners: Vec::new(),
            sg: SgState::default(),
        }
    }

    #[test]
    fn setup_f1_f2_g1() {
        let mut sg = SgState::default();
        sg.setup_round(&["F1".into()]);
        assert_eq!(sg.gray_groups, F1_GROUPS.to_vec());
        assert_eq!(sg.gray_groups.iter().sum::<u32>(), 9);

        let mut sg = SgState::default();
        sg.setup_round(&["F2".into()]);
        assert_eq!(&sg.gray[0..3], &[1, 1, 1]);
        assert_eq!(&sg.gray[3..6], &[2, 2, 2]);
        assert_eq!(sg.gray.iter().sum::<u32>(), 9);

        let mut sg = SgState::default();
        sg.setup_round(&["G1".into()]);
        assert_eq!(sg.lottery, G1_LOTTERY.to_vec());
    }

    #[test]
    fn c1_grants_on_fifth_die_and_settles() {
        let mut st = state_with_casinos(&["C1", "X", "Y"]);
        let mut rng = SeededRng::new(1);
        let mut ev = Vec::new();
        st.casinos[0].dice.insert("h".into(), 5);
        on_place(&mut st, 0, "h", &mut rng, &mut ev);
        assert_eq!(st.sg.grand_prize.get("h"), Some(&1));
        post_settle(&mut st, &mut ev);
        let seat = st.seats.iter().find(|s| s.player_id == "h").unwrap();
        assert_eq!(seat.banknotes, vec![C1_REWARD as u32]);
    }

    #[test]
    fn d1_rewards_by_casino_count() {
        let mut st = state_with_casinos(&["D1", "X", "Y"]);
        let mut rng = SeededRng::new(2);
        let mut ev = Vec::new();
        st.casinos[0].dice.insert("h".into(), 1);
        on_place(&mut st, 0, "h", &mut rng, &mut ev);
        // 只有 1 个赌场有骰子 → 1 筹码
        assert_eq!(st.seats[0].chips, crate::INITIAL_CHIPS + 1);
        // 再占 2 个赌场 → 3 个 → count×1万 钞票
        st.casinos[1].dice.insert("h".into(), 1);
        st.casinos[2].dice.insert("h".into(), 1);
        on_place(&mut st, 0, "h", &mut rng, &mut ev);
        assert_eq!(st.seats[0].banknotes, vec![30_000]);
    }

    #[test]
    fn c2_min_dice_pays_penalty() {
        let mut st = state_with_casinos(&["Y", "X", "Z"]);
        st.casinos[0].small_games = vec!["C2".into()];
        st.casinos[0].dice.insert("h".into(), 3);
        // p2 在该赌场 0 颗 → 付 5 万
        st.seats[1].chips = 3; // 3 万
        st.seats[1].banknotes = vec![50_000];
        let mut ev = Vec::new();
        c2_pre_casino(&mut st, 0, &mut ev);
        let p2 = st.seats.iter().find(|s| s.player_id == "p2").unwrap();
        assert_eq!(p2.chips, 0);
        assert_eq!(p2.banknotes, vec![30_000]); // 50k note 支付 50k? 支付 20k 后剩余？
        // h 有骰子（非最少）不付
        assert_eq!(st.seats[0].chips, crate::INITIAL_CHIPS);
    }

    #[test]
    fn rule_text_case_insensitive() {
        assert!(rule_text("f1").is_some());
        assert!(rule_text("F1").is_some());
        assert_eq!(rule_text("f1"), rule_text("F1"));
        assert!(rule_text("H2").unwrap().contains("金框"));
        assert!(rule_text("Z9").is_none());
    }

    #[test]
    fn a1_guess_wrong_rewards_initiator() {
        let mut st = state_with_casinos(&[]);
        st.sg.pending = Some(SgPending {
            game: "A1".into(),
            casino: 0,
            actor: "p2".into(),
            stage: "guess".into(),
            data: json!({ "initiator": "h", "n": 5 }),
        });
        let mut rng = SeededRng::new(1);
        let (done, _ev, _m) = resolve(&mut st, "sg:A1:guess:2", "p2", &mut rng).unwrap();
        assert!(done);
        assert_eq!(st.seats[0].chips, crate::INITIAL_CHIPS + A1_LADDER[4]);
    }

    #[test]
    fn e1_ban_moves_arrow_and_rewards() {
        let mut st = state_with_casinos(&[]);
        st.sg.pending = Some(SgPending { game: "E1".into(), casino: 0, actor: "h".into(), stage: "choose".into(), data: json!({}) });
        let mut rng = SeededRng::new(1);
        let (done, _ev, _m) = resolve(&mut st, "sg:E1:ban:4", "h", &mut rng).unwrap();
        assert!(done);
        assert_eq!(st.sg.banned, Some(3));
        assert_eq!(st.seats[0].chips, crate::INITIAL_CHIPS + E1_ARROW_LADDER[0]);
        assert_eq!(st.sg.arrow, 1);
    }

    #[test]
    fn f1_moves_gray_group() {
        let mut st = state_with_casinos(&["F1"]);
        st.sg.gray_groups = F1_GROUPS.to_vec();
        st.sg.pending = Some(SgPending { game: "F1".into(), casino: 0, actor: "h".into(), stage: "move".into(), data: json!({}) });
        let mut rng = SeededRng::new(1);
        let (done, _ev, _m) = resolve(&mut st, "sg:F1:move:0:3", "h", &mut rng).unwrap();
        assert!(done);
        assert_eq!(st.casinos[2].dice.get("gray").copied(), Some(F1_GROUPS[0]));
        assert_eq!(st.sg.gray_groups[0], 0);
    }

    #[test]
    fn g1_split_then_pick() {
        let mut st = state_with_casinos(&["G1"]);
        st.sg.lottery = G1_LOTTERY.to_vec();
        st.sg.pending = Some(SgPending {
            game: "G1".into(),
            casino: 0,
            actor: "p2".into(),
            stage: "split".into(),
            data: json!({ "winner": "h", "total": 10 }),
        });
        let mut rng = SeededRng::new(1);
        let (done, _ev, _m) = resolve(&mut st, "sg:G1:split:6", "p2", &mut rng).unwrap();
        assert!(!done);
        assert_eq!(st.sg.pending.as_ref().unwrap().actor, "h");
        let (done2, _e2, _m2) = resolve(&mut st, "sg:G1:pick:0", "h", &mut rng).unwrap();
        assert!(done2);
        assert_eq!(st.seats[0].banknotes, vec![60_000]);
    }

    #[test]
    fn h2_gold_frame_settles() {
        let mut st = state_with_casinos(&["H2"]);
        st.sg.pending = Some(SgPending { game: "H2".into(), casino: 0, actor: "h".into(), stage: "choose".into(), data: json!({ "dice": [3, 6] }) });
        let mut rng = SeededRng::new(1);
        let (done, _ev, _m) = resolve(&mut st, "sg:H2:choose:6", "h", &mut rng).unwrap();
        assert!(done);
        assert_eq!(st.sg.gold_frame, Some(("h".into(), 1)));
        let mut ev = Vec::new();
        settle_extras(&mut st, &mut ev);
        assert_eq!(st.seats[0].banknotes, vec![H2_GOLD]);
    }

    #[test]
    fn d2_privilege_updates_on_place() {
        let mut st = state_with_casinos(&["D2"]);
        st.casinos[0].dice.insert("h".into(), 3);
        update_privilege(&mut st, 0);
        assert_eq!(st.sg.privilege.as_deref(), Some("h"));
        // p2 追平 → 回场
        st.casinos[0].dice.insert("p2".into(), 3);
        update_privilege(&mut st, 0);
        assert_eq!(st.sg.privilege, None);
        // p2 超过 → 转移
        st.casinos[0].dice.insert("p2".into(), 4);
        update_privilege(&mut st, 0);
        assert_eq!(st.sg.privilege.as_deref(), Some("p2"));
    }

    #[test]
    fn pay_assets_uses_chips_then_notes() {
        let mut st = state_with_casinos(&[]);
        st.seats[0].chips = 2; // 2 万
        st.seats[0].banknotes = vec![30_000, 40_000];
        let paid = pay_assets(&mut st, "h", 50_000);
        assert_eq!(paid, 50_000);
        assert_eq!(st.seats[0].chips, 0);
        // 筹码 2 万 + 付 4 万钞票，银行找零 1 万；余 3 万 + 1 万
        assert_eq!(st.seats[0].banknotes, vec![30_000, 10_000]);
        let _ = CHIP_VALUE;
    }
}
