//! 自对弈示例：用启发式 AI 跑 N 局并打印结果（用于观察/回归）。
//!
//! 运行：`cargo run -p las-vegas --example selfplay -- [局数] [人数]`
use game_engine_core::Policy;
use las_vegas::ai::{run_self_play, HeuristicPolicy};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let n: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(5);
    let pc: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(4).max(2);

    let names: Vec<String> = (0..pc).map(|i| format!("p{i}")).collect();
    let players: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
    let policy = HeuristicPolicy;
    let policies: Vec<&dyn Policy> = (0..pc).map(|_| &policy as &dyn Policy).collect();

    let mut finished = 0;
    for seed in 1..=n {
        let r = run_self_play(&players, &policies, seed, 20000);
        if r.finished {
            finished += 1;
        }
        println!(
            "seed={seed} finished={} steps={} events={} winners={:?} scores={:?}",
            r.finished, r.steps, r.event_count, r.winners, r.scores
        );
    }
    println!("{finished}/{n} 局完成（{pc} 人）");
}
