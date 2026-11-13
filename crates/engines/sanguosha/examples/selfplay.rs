//! 三国杀自对弈（回归门禁 / 数据采集入口）。
//!
//! 用法：
//!   cargo run -p sanguosha --release --example selfplay -- --games 100 --players 5 --seed 1
//!   cargo run -p sanguosha --example selfplay -- --games 3 --json
//!
//! 输出（默认人读；`--json` 机读）：
//!   games / finished / illegal / stalled / steps_total / max_steps / wins / pending kinds
//!
//! 口径：5 人身份局、全机器人（`SgsPolicy`）、固定种子可复现；
//! 「无非法动作、无卡死」是 P6 验收硬指标（`npm run check:sgs-ai`）。

use std::time::Instant;

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let i = args.iter().position(|a| a == &format!("--{name}"))?;
    args.get(i + 1).cloned()
}

fn flag(name: &str) -> bool {
    std::env::args().any(|a| a == format!("--{name}"))
}

fn main() {
    let games: usize = arg("games")
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    let players: usize = arg("players")
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    let seed: u64 = arg("seed").and_then(|v| v.parse().ok()).unwrap_or(1);
    let max_steps: usize = arg("max-steps")
        .and_then(|v| v.parse().ok())
        .unwrap_or(8000);
    let generals = arg("generals").unwrap_or_else(|| sanguosha::ai::FULL_GENERALS.to_string());
    let json = flag("json");

    let started = Instant::now();
    let batch = if flag("trace") {
        let result = sanguosha::ai::run_self_play_traced(
            players,
            seed,
            max_steps,
            &generals,
            &mut |step, st, actor, action| {
                let pending = st
                    .pending
                    .as_ref()
                    .map(|p| {
                        format!(
                            "{}:{}[{}]",
                            p.kind,
                            p.skill.clone().unwrap_or_default(),
                            p.options
                                .iter()
                                .map(|o| o.id.clone())
                                .collect::<Vec<_>>()
                                .join("|")
                        )
                    })
                    .unwrap_or_else(|| "-".to_string());
                let stack: String = st
                    .resolution
                    .iter()
                    .rev()
                    .take(4)
                    .map(|f| {
                        let d = format!("{f:?}");
                        d.split(['(', '{']).next().unwrap_or("?").to_string()
                    })
                    .collect::<Vec<_>>()
                    .join("<");
                println!(
                    "#{step} round={} phase={:?}/{:?} pending={pending} stack={stack} actor={actor} → {action}",
                    st.round, st.phase, st.turn_phase
                );
            },
        );
        let mut out = sanguosha::ai::BatchResult {
            games: 1,
            ..Default::default()
        };
        out.finished += usize::from(result.finished);
        out.illegal += result.illegal;
        out.stalled += result.stalled;
        out.steps_total += result.steps;
        out.max_game_steps = result.steps;
        out.steps_per_game.push(result.steps);
        out.pending_kinds = result.pending_kinds;
        *out.wins
            .entry(result.winner_team.unwrap_or_else(|| "none".into()))
            .or_insert(0) += 1;
        out
    } else {
        sanguosha::ai::run_batch(games, players, seed, max_steps, &generals)
    };
    let elapsed = started.elapsed();

    if json {
        let kinds: Vec<&str> = batch.pending_kinds.iter().map(String::as_str).collect();
        println!(
            "{}",
            serde_json::json!({
                "games": batch.games,
                "finished": batch.finished,
                "finish_rate": batch.finish_rate(),
                "illegal": batch.illegal,
                "loops": batch.loops,
                "stalled": batch.stalled,
                "steps_total": batch.steps_total,
                "max_game_steps": batch.max_game_steps,
                "steps_per_game": batch.steps_per_game,
                "wins": batch.wins,
                "pending_kinds": kinds,
                "elapsed_ms": elapsed.as_millis(),
                "players": players,
                "seed": seed,
                "generals": generals,
                "errors": batch.errors,
                "recovered": batch.recovered,
            })
        );
    } else {
        println!("三国杀自对弈：{} 人局 × {} 局（seed={seed}）", players, games);
        println!("  将军配置：{generals}");
        println!("  {}", batch.summary());
        println!(
            "  pending kinds（{}）：{}",
            batch.pending_kinds.len(),
            batch.pending_kinds.iter().cloned().collect::<Vec<_>>().join(", ")
        );
        println!("  耗时：{:.2}s", elapsed.as_secs_f64());
    }

    // CI 口径：有非法动作或卡死即失败（终局率由调用方断言）。
    if batch.illegal > 0 || batch.stalled > 0 {
        std::process::exit(1);
    }
}
