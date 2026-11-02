//! 三国杀决策回放（P6.4）。
//!
//! 口径：`game_replay_base`（首个决策前快照）+ `game_decision_log`（按 seq）逐步重放：
//!   1) 对比当前 state_digest 与日志中动作前摘要；
//!   2) 执行日志里的 `chosen`（引擎无条件重校验）；
//!   3) 全部结束后对比 `game_state` 最新快照。
//!
//! 用法（仓库根）：
//!   cargo run --manifest-path crates/Cargo.toml -p server --bin sgs-replay -- --room <room_id>
//!   ... --from-seq 100 --to-seq 300 --step --json
//!
//! 说明：旧局（P6.4 之前）没有 `game_replay_base`，只能打印决策时间线；不阻塞。

use game_engine_core::{EngineRegistry, GameManager, GameState, SeededRng};
use std::io::{self, Write};

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let i = args.iter().position(|a| a == &format!("--{name}"))?;
    args.get(i + 1).cloned()
}

fn flag(name: &str) -> bool {
    std::env::args().any(|a| a == format!("--{name}"))
}

fn main() -> anyhow::Result<()> {
    let db_path = arg("db").unwrap_or_else(|| "data/tabletop.db".to_string());
    let room = arg("room").ok_or_else(|| anyhow::anyhow!("缺少 --room <room_id>"))?;
    let from_seq: i64 = arg("from-seq").and_then(|v| v.parse().ok()).unwrap_or(0);
    let to_seq: i64 = arg("to-seq")
        .and_then(|v| v.parse().ok())
        .unwrap_or(i64::MAX);
    let limit: usize = arg("limit").and_then(|v| v.parse().ok()).unwrap_or(100_000);
    let step = flag("step");
    let as_json = flag("json");

    let storage = storage::Storage::open(&db_path)?;
    let logs = storage.list_decision_logs("sanguosha", &room, from_seq, limit)?;
    let logs: Vec<_> = logs.into_iter().filter(|r| r.seq <= to_seq).collect();

    if logs.is_empty() {
        eprintln!("[sgs-replay] 没有决策日志 room={room} since={from_seq}");
        return Ok(());
    }

    let base = storage.get_replay_base(&room)?;
    let mut mismatches = Vec::new();
    let mut applied = 0usize;
    let mut restored = false;

    if let Some((game_id, base_seq, state_json)) = &base {
        let gs: GameState = serde_json::from_str(state_json)?;
        if gs.game_id != *game_id {
            anyhow::bail!("replay base game_id 不一致: {} != {game_id}", gs.game_id);
        }
        let mut reg = EngineRegistry::new();
        reg.register(sanguosha::SanguoshaEngine);
        let mgr = GameManager::with_rng(reg, Box::new(SeededRng::new(0)));
        mgr.restore(gs);
        restored = true;
        printf(
            as_json,
            &format!(
                "[sgs-replay] base seq={base_seq} room={room} logs={}",
                logs.len()
            ),
        );

        for row in &logs {
            let Some(chosen) = row.chosen.as_deref() else {
                continue;
            };
            let state = mgr
                .get_state(&room)
                .ok_or_else(|| anyhow::anyhow!("回放中状态丢失"))?;
            if let Some(expected) = row.state_digest.as_deref() {
                let actual = sanguosha::SgsState::parse(&state.data)
                    .ok()
                    .map(|s| sanguosha::ai::state_digest(&s))
                    .unwrap_or_default();
                if actual != expected {
                    mismatches.push(format!(
                        "seq={} digest 不一致 expected={expected} actual={actual}",
                        row.seq
                    ));
                }
            }
            printf(
                as_json,
                &format!(
                    "seq={} {} {} -> {} ({})",
                    row.seq,
                    row.player_id,
                    row.source,
                    chosen,
                    row.reason.as_deref().unwrap_or("-")
                ),
            );
            if step {
                print!("按回车继续…");
                io::stdout().flush().ok();
                let mut line = String::new();
                io::stdin().read_line(&mut line).ok();
            }
            match mgr.execute_action(&room, &row.player_id, chosen) {
                Ok(_) => applied += 1,
                Err(e) => {
                    mismatches.push(format!("seq={} 执行 {chosen} 失败: {e}", row.seq));
                    break;
                }
            }
        }

        // 最终快照对比（若日志覆盖到当前快照）。
        if let Some(final_gs) = mgr.get_state(&room) {
            if let Ok(Some(snapshot)) = storage.get_game_snapshot("sanguosha", &room) {
                if let Ok(snapshot_gs) = serde_json::from_str::<GameState>(&snapshot) {
                    let current = serde_json::to_value(&final_gs).ok();
                    let expected = serde_json::to_value(&snapshot_gs).ok();
                    if current != expected {
                        mismatches.push("最终状态与 game_state 快照不一致".to_string());
                    }
                }
            }
        }
    } else {
        printf(
            as_json,
            &format!("[sgs-replay] 无 replay base（旧局）：仅打印决策时间线 room={room}"),
        );
        for row in &logs {
            printf(
                as_json,
                &format!(
                    "seq={} {} {} -> {} ({})",
                    row.seq,
                    row.player_id,
                    row.source,
                    row.chosen.as_deref().unwrap_or("-"),
                    row.reason.as_deref().unwrap_or("-")
                ),
            );
        }
    }

    if as_json {
        println!(
            "{}",
            serde_json::json!({
                "room": room,
                "logs": logs.len(),
                "applied": applied,
                "restored": restored,
                "mismatches": mismatches,
            })
        );
    } else {
        println!(
            "[sgs-replay] logs={} applied={} restored={} mismatches={}",
            logs.len(),
            applied,
            restored,
            mismatches.len()
        );
        for m in &mismatches {
            eprintln!("  ✗ {m}");
        }
    }
    if !mismatches.is_empty() {
        std::process::exit(2);
    }
    Ok(())
}

fn printf(as_json: bool, msg: &str) {
    if !as_json {
        println!("{msg}");
    }
}
