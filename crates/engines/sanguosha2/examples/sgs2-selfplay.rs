//! v2 自对弈（S2：**真实 AI** `sanguosha2::ai::BotBrain` 的 100 局回归）。
//!
//! 只通过 `EnginePlugin`（legal_actions / execute_action）+ AI 决策；不碰内核私有字段。
//! 与 server 的 `ai::drive_bots` 共用同一份策略（单一事实源）。确定性：同一 seed 结果可复现。
//!
//! 用法：
//!   cargo run -p sanguosha2 --example sgs2-selfplay -- --games 10 --players 5 --seed 1 --json
//!
//! 输出（--json）：
//!   { "engine":"sanguosha2", "games":10, "finished":10, "illegal":0, "loops":0,
//!     "stalled":0, "steps":1234, "max_steps":20000, "winners":{...},
//!     "pending_kinds":["card_response","discard_phase",...] }

use std::collections::{BTreeMap, BTreeSet, HashMap};

use game_engine_core::{EnginePlugin, GameState, SeededRng};
use sanguosha2::ai::BotBrain;
use sanguosha2::kernel::Game;
use sanguosha2::Sanguosha2Engine;

fn arg(name: &str, default: &str) -> String {
    let args: Vec<String> = std::env::args().collect();
    for (i, a) in args.iter().enumerate() {
        if a == &format!("--{name}") {
            if let Some(v) = args.get(i + 1) {
                if !v.starts_with("--") {
                    return v.clone();
                }
            }
        }
    }
    default.to_string()
}

fn flag(name: &str) -> bool {
    std::env::args().any(|a| a == format!("--{name}"))
}

fn parse_state(state: &GameState) -> Game {
    serde_json::from_value(state.data.clone()).expect("状态解析失败")
}

/// 动作 id 去掉「实体牌 id」后的签名：同面牌的不同实体 id 视为等价。
///
/// `use:c100:1` → `use:1`；`convert:huoji:c12:火攻:2` → `convert:huoji:火攻:2`；
/// `discard:c7` → `discard`；其它（`target:3` / `skill:x` …）原样返回。
fn action_signature(id: &str) -> String {
    let parts: Vec<&str> = id.split(':').collect();
    match parts.first().copied() {
        Some("use") => {
            let mut out = vec!["use"];
            out.extend(parts.iter().skip(2));
            out.join(":")
        }
        Some("convert") if parts.len() >= 4 => {
            let mut out: Vec<&str> = parts.iter().take(2).copied().collect();
            out.extend(parts.iter().skip(3));
            out.join(":")
        }
        // 单牌/双牌作用域：同面牌的不同实体 id 视为等价（文案已含牌面，选哪张效果一样）。
        Some("discard") | Some("pick") | Some("respond") | Some("recast") | Some("choose")
        | Some("card") | Some("reveal") | Some("pair") => {
            parts.first().copied().unwrap_or_default().to_string()
        }
        Some("vrespond") if parts.len() >= 2 => format!("vrespond:{}", parts[1]),
        Some("trim") if parts.len() >= 3 => format!("trim:{}", parts[2]),
        _ => id.to_string(),
    }
}

struct Summary {
    games: u32,
    finished: u32,
    illegal: u32,
    loops: u32,
    stalled: u32,
    /// 跨过终局阈值（`round > ENDGAME_ROUND`）的局数：**信息项**，不是失败。
    long: u32,
    /// 全场最大轮次（诊断「长局」用）。
    max_round: u32,
    /// 选项文案重复（相同文案 + 不同后果）的次数：端侧无法区分的交互缺陷。
    label_dups: u32,
    steps: u64,
    max_steps: u32,
    winners: BTreeMap<String, u32>,
    pending_kinds: BTreeSet<String>,
    errors: Vec<String>,
}

fn main() {
    let games: u32 = arg("games", "10").parse().unwrap_or(10);
    let players: usize = arg("players", "5").parse().unwrap_or(5);
    let seed: u64 = arg("seed", "1").parse().unwrap_or(1);
    let max_steps: u32 = arg("max-steps", "20000").parse().unwrap_or(20000);
    let json_out = flag("json");
    let log_path = arg("log", "");
    let mut decision_log: Vec<serde_json::Value> = Vec::new();
    let mut log_step = 0u32;

    let engine = Sanguosha2Engine;
    let mut s = Summary {
        games,
        finished: 0,
        illegal: 0,
        loops: 0,
        stalled: 0,
        long: 0,
        max_round: 0,
        label_dups: 0,
        steps: 0,
        max_steps,
        winners: BTreeMap::new(),
        pending_kinds: BTreeSet::new(),
        errors: Vec::new(),
    };

    for game_idx in 0..games {
        log_step = 0;
        let mut rng = SeededRng::new(seed + game_idx as u64 * 7919);
        let players_ids: Vec<String> = (0..players).map(|i| format!("p{i}")).collect();
        let (mut state, _) = engine.new_started_game(
            &format!("r{game_idx}"),
            "g1",
            players_ids.clone(),
            &mut rng,
        );
        let mut seen: HashMap<String, u32> = HashMap::new();
        let mut brain = BotBrain::new();
        let mut steps = 0u32;
        let mut done = false;
        let mut long_game = false;

        while steps < max_steps {
            let g = parse_state(&state);
            if g.finished {
                done = true;
                let w = g.winner.clone().unwrap_or_else(|| "unknown".to_string());
                *s.winners.entry(w).or_insert(0) += 1;
                break;
            }
            if g.round > s.max_round {
                s.max_round = g.round;
            }
            if g.round > sanguosha2::kernel::ENDGAME_ROUND {
                long_game = true;
            }
            if let Some(p) = &g.pending {
                s.pending_kinds.insert(p.kind().to_string());
            }
            let actor = match &g.pending {
                Some(p) => g.seats[p.actor()].name.clone(),
                None => g.seats[g.turn].name.clone(),
            };
            let actions = engine.legal_actions(&state, &actor);
            // 门禁：**文案相同但后果不同**的选项 → 端侧无法区分（Web 按钮/QQ 清单同源）。
            // 2026-11-11 真机：「使用【杀】」对所有目标都一样。允许「同面牌」的重复文案
            // （两张一模一样的红桃2闪，弃哪张都一样）——按「去掉实体牌 id 后的动作签名」比对。
            if !actions.is_empty() {
                let mut by_label: std::collections::HashMap<&str, String> =
                    std::collections::HashMap::new();
                for a in &actions {
                    let sig = action_signature(&a.id);
                    match by_label.get(a.description.as_str()) {
                        Some(prev) if *prev != sig => {
                            s.label_dups += 1;
                            s.errors.push(format!(
                                "game {game_idx}: 选项文案相同但后果不同（pending={:?} skill={:?}）：{} | {} vs {}",
                                g.pending.as_ref().map(|p| p.kind()),
                                g.pending.as_ref().and_then(|p| p.skill()),
                                a.description,
                                prev,
                                sig
                            ));
                            break;
                        }
                        _ => {
                            by_label.insert(a.description.as_str(), sig);
                        }
                    }
                }
            }
            let Some(seat) = sanguosha2::view::seat_of(&g, &actor) else {
                s.stalled += 1;
                s.errors.push(format!("game {game_idx}: {actor} 不在座"));
                break;
            };
            let Some(choice) = brain.choose(&g, seat, &actions) else {
                s.loops += 1;
                s.errors.push(format!("game {game_idx}: loop（无可试动作）"));
                break;
            };
            let action = choice.action.clone();
            if choice.loop_breaker {
                s.loops = s.loops.saturating_add(0);
            }

            // 环检测（第三层兜底）：只看**交互态**（pending）且带技能名。
            // 出牌阶段「发动技能 → 放弃」会回到完全相同的非 pending 状态（正常），不参与判环。
            let hp: Vec<i32> = g.seats.iter().map(|x| x.hp).collect();
            let hands: Vec<usize> = (0..g.seats.len()).map(|i| g.hand(i).len()).collect();
            let trackable = matches!(
                g.pending.as_ref().map(|p| p.kind()),
                Some(kind) if !matches!(kind, "wuxie" | "harvest")
            );
            if trackable {
                // 签名含 `round`：跨轮重复（如每轮结束阶段都可发动的技能）不算死循环，
                // 只有**同一轮内**状态完全重复 13 次才判环（长局/僵局由终局房规收束，ADR-0040）。
                let sig = format!(
                    "{:?}",
                    (
                        g.round,
                        g.turn,
                        g.turn_seq,
                        g.phase,
                        g.pending.as_ref().map(|p| p.kind()),
                        g.pending.as_ref().and_then(|p| p.skill()),
                        actor.clone(),
                        action.clone(),
                        hp.clone(),
                        hands.clone()
                    )
                );
                *seen.entry(sig).or_insert(0) += 1;
                if seen.values().copied().max().unwrap_or(0) > 12 {
                    s.loops += 1;
                    s.errors.push(format!("game {game_idx}: loop detected at {action}"));
                    break;
                }
            }

            // 决策日志（每步一行，供 `check:sgs2-decision-log` 校验/排障）。
            if !log_path.is_empty() {
                log_step += 1;
                decision_log.push(serde_json::json!({
                    "game": game_idx,
                    "step": log_step,
                    "turn": g.turn,
                    "round": g.round,
                    "phase": format!("{:?}", g.phase),
                    "actor": actor,
                    "pending": g.pending.as_ref().map(|p| p.kind().to_string()),
                    "pending_skill": g.pending.as_ref().and_then(|p| p.skill().map(str::to_string)),
                    "legal_count": actions.len(),
                    "legal": actions.iter().map(|a| a.id.clone()).collect::<Vec<_>>(),
                    "chosen": action.clone(),
                    "hp": hp.clone(),
                    "hand_counts": hands.clone(),
                    "deck_count": g.ids_in(|z| *z == sanguosha2::kernel::Zone::Draw).len(),
                }));
            }

            match engine.execute_action(&state, &actor, &action, &mut rng) {
                Ok(res) => {
                    steps += 1;
                    s.steps += 1;
                    state = res.state;
                    if res.finished {
                        let g = parse_state(&state);
                        let w = g.winner.clone().unwrap_or_else(|| "unknown".to_string());
                        *s.winners.entry(w).or_insert(0) += 1;
                        done = true;
                        break;
                    }
                }
                Err(e) => {
                    s.illegal += 1;
                    s.errors.push(format!(
                        "game {game_idx}: illegal {action}: {e}（pending={:?}/{:?}）",
                        g.pending.as_ref().map(|p| p.kind().to_string()),
                        g.pending.as_ref().and_then(|p| p.skill())
                    ));
                    // 与 `ai::drive_bots` 同口径：被拒后回落到 pass/confirm/end_play/其它动作，
                    // 不让一局因单步非法而整局中止（非法次数仍计入统计）。
                    let fallback = ["pass", "confirm", "end_play"]
                        .iter()
                        .find(|id| actions.iter().any(|a| a.id == **id))
                        .map(|id| (*id).to_string())
                        .or_else(|| {
                            actions
                                .iter()
                                .map(|a| a.id.clone())
                                .find(|id| id != &action)
                        });
                    let Some(fb) = fallback else {
                        break;
                    };
                    match engine.execute_action(&state, &actor, &fb, &mut rng) {
                        Ok(res) => {
                            steps += 1;
                            s.steps += 1;
                            state = res.state;
                            if res.finished {
                                let g = parse_state(&state);
                                let w = g.winner.clone().unwrap_or_else(|| "unknown".to_string());
                                *s.winners.entry(w).or_insert(0) += 1;
                                done = true;
                                break;
                            }
                        }
                        Err(e2) => {
                            s.errors.push(format!(
                                "game {game_idx}: fallback {fb} 也失败：{e2}"
                            ));
                            break;
                        }
                    }
                }
            }
        }
        if long_game {
            s.long += 1;
        }
        if !done {
            s.stalled += 1;
        } else {
            s.finished += 1;
        }
    }

    if !log_path.is_empty() {
        let body = decision_log
            .iter()
            .map(|v| serde_json::to_string(v).unwrap())
            .collect::<Vec<_>>()
            .join("
");
        if let Err(e) = std::fs::write(&log_path, format!("{body}
")) {
            eprintln!("[sgs2-selfplay] 决策日志写入失败 {log_path}: {e}");
        }
    }

    if json_out {
        let out = serde_json::json!({
            "engine": "sanguosha2",
            "games": s.games,
            "players": players,
            "seed": seed,
            "finished": s.finished,
            "illegal": s.illegal,
            "loops": s.loops,
            "stalled": s.stalled,
            "long": s.long,
            "max_round": s.max_round,
            "endgame_round": sanguosha2::kernel::ENDGAME_ROUND,
            "label_dups": s.label_dups,
            "steps": s.steps,
            "max_steps": s.max_steps,
            "winners": s.winners,
            "pending_kinds": s.pending_kinds,
            "errors": s.errors,
        });
        println!("{}", serde_json::to_string(&out).unwrap());
    } else {
        println!(
            "[sgs2-selfplay] {} 局 / 终局 {} / illegal {} / loops {} / stalled {} / long {} / 最大轮次 {} / 文案重复 {} / 步数 {}",
            s.games, s.finished, s.illegal, s.loops, s.stalled, s.long, s.max_round, s.label_dups, s.steps
        );
        println!("  winners: {:?}", s.winners);
        println!("  pending kinds: {:?}", s.pending_kinds);
        for e in &s.errors {
            println!("  ! {e}");
        }
    }
}
