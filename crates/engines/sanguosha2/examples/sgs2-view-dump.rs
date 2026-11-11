//! 导出 v2 视图样本（S0 门禁 `check:sgs2-view-parity.mjs` 用）。
//!
//! 目的：v2 视图字段集必须覆盖 Web 端（`apps/web/src/view/sgs/types.ts`）消费的字段。
//! 本 example 只做「建局 → 投影」，不写任何业务逻辑；输出稳定 JSON 供脚本比对。
//!
//! 用法：
//!   cargo run -q -p sanguosha2 --example sgs2-view-dump -- --json
//!   cargo run -q -p sanguosha2 --example sgs2-view-dump -- --json --players 8

use game_engine_core::{EnginePlugin, SeededRng};
use sanguosha2::kernel::Game;
use sanguosha2::{view, Sanguosha2Engine};

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

fn main() {
    let players: usize = arg("players", "5").parse().unwrap_or(5);
    let seed: u64 = arg("seed", "1").parse().unwrap_or(1);

    use game_engine_core::EnginePlugin;

    let engine = Sanguosha2Engine;
    let mut rng = SeededRng::new(seed);
    let ids: Vec<String> = (0..players.max(5)).map(|i| format!("p{i}")).collect();

    // ① 选将阶段样本（`player_area.setup` 只在这个阶段存在）。
    let (mut lobby, _) = engine.new_game("r-setup", "g-setup", ids.clone(), &mut rng);
    let e = Sanguosha2Engine;
    let mut rng2 = game_engine_core::SeededRng::new(2);
    lobby = e
        .execute_action(&lobby, &ids[0], "lobby:start", &mut rng2)
        .expect("start")
        .state;
    let sg: Game = serde_json::from_value(lobby.data.clone()).expect("setup 状态解析失败");
    let lord = sg.setup.as_ref().map(|s| s.lord).unwrap_or(0);
    let player_setup = view::player_view(&sg, lord);

    // ② 已开局样本（board/seat/area 的常规字段）。
    let (state, _) = engine.new_started_game("r-view", "g-view", ids, &mut rng);
    let g: Game = serde_json::from_value(state.data.clone()).expect("v2 状态解析失败");
    let public = view::public_state(&g);
    let player = view::player_view(&g, 0);
    let out = serde_json::json!({
        "engine": "sanguosha2",
        "players": players,
        "public": public,
        "player": player,
        "player_setup": player_setup,
    });
    println!(
        "{}",
        serde_json::to_string(&out).expect("视图序列化失败")
    );
}
