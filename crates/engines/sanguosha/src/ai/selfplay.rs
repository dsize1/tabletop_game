//! 自对弈驱动（回归门禁 / 后续训练数据采集）。
//!
//! 与拉斯维加斯 `ai::run_self_play` 同形态：固定种子 → 确定性可复现；
//! 统计「终局率 / 非法动作数 / 卡死 / pending 覆盖 / 阵营胜率」，供
//! `cargo run -p sanguosha --example selfplay` 与 `npm run check:sgs-ai` 使用。

use crate::ai::policy::SgsPolicy;
use crate::state::{Phase, SgsState};
use game_engine_core::{EngineRegistry, GameManager, GameStatus, Policy, SeededRng};
use std::collections::{BTreeMap, BTreeSet};

/// 全部已实现武将（去重后 149 位）+ 默认牌堆（standard+junzheng = 160 张）。
///
/// 用户口径（2026-11-01）：灰度与自对弈默认用「已实现的去重全将 + 全卡牌」。
/// 注意：**不改 `Config.general_packs` 的全局默认值**（旧用例/旧快照依赖 standard）。
pub const FULL_GENERALS: &str = "standard+wind_fire_forest_mountain+yijiang+jiexian";

/// 单局自对弈结果。
#[derive(Debug, Clone, Default)]
pub struct SelfPlayResult {
    pub finished: bool,
    pub steps: usize,
    /// 非法动作次数（正常应恒为 0）。
    pub illegal: usize,
    /// 状态回环被破环器强行推进的次数。
    pub loops: usize,
    /// 首次回环的现场（kind/actor/动作/digest）。
    pub loop_detail: Option<String>,
    /// 卡住（无合法动作 / 无操作者）次数。
    pub stalled: usize,
    pub winner_team: Option<String>,
    /// 本局触发过的 pending kind（覆盖率统计）。
    pub pending_kinds: BTreeSet<String>,
    pub last_error: Option<String>,
    /// 可恢复的引擎拒绝（记录下来供缺陷清单使用；对局已回落继续）。
    pub errors: Vec<String>,
}

/// 批量结果。
#[derive(Debug, Clone, Default)]
pub struct BatchResult {
    pub games: usize,
    pub finished: usize,
    pub illegal: usize,
    pub loops: usize,
    pub stalled: usize,
    pub steps_total: usize,
    pub max_game_steps: usize,
    /// 阵营 → 胜场（`lord` / `rebel` / `spy` / `none`）。
    pub wins: BTreeMap<String, u32>,
    pub pending_kinds: BTreeSet<String>,
    /// 每局步数（便于看分布/长尾）。
    pub steps_per_game: Vec<usize>,
    /// 各局失败原因（非法动作 / 卡死），便于定位。
    pub errors: Vec<String>,
    /// 已回落的引擎拒绝（AI 与引擎合法集口径不一致的缺陷清单）。
    pub recovered: Vec<String>,
}

impl BatchResult {
    /// 终局率 0..1。
    pub fn finish_rate(&self) -> f64 {
        if self.games == 0 {
            return 0.0;
        }
        self.finished as f64 / self.games as f64
    }

    /// 一行摘要（脚本/CI 输出用）。
    pub fn summary(&self) -> String {
        format!(
            "games={} finished={} ({:.0}%) illegal={} stalled={} steps_total={} max_steps={} wins={:?} kinds={}",
            self.games,
            self.finished,
            self.finish_rate() * 100.0,
            self.illegal,
            self.stalled,
            self.steps_total,
            self.max_game_steps,
            self.wins,
            self.pending_kinds.len()
        )
    }
}

/// 状态摘要（用于空转检测：同一摘要再次出现 = 无进展回路）。
///
/// 只取「会随动作改变」的公共量，避免把隐藏信息混进来（也便于日志对比）。
pub fn state_digest(st: &SgsState) -> String {
    // pending 的 `data` 常含「已选牌」等无副作用的交互进度（多选 toggle），
    // 不含它会把正常的选牌过程误判成回环；含明文又会泄漏隐藏信息 → 取哈希。
    let pending = st
        .pending
        .as_ref()
        .map(|p| {
            let data = serde_json::to_string(&p.data).unwrap_or_default();
            let opts: Vec<&str> = p.options.iter().map(|o| o.id.as_str()).collect();
            format!(
                "{}:{}:{:x}:{:x}",
                p.kind,
                p.actor,
                fnv1a(&data),
                fnv1a(&opts.join("|"))
            )
        })
        .unwrap_or_default();
    let seats: Vec<String> = st
        .seats
        .iter()
        .map(|s| {
            format!(
                "{}:{}/{}:{}/{}/{}/{}",
                s.player_id,
                s.hp,
                s.max_hp,
                s.hand.len(),
                s.equips.values().flatten().count(),
                s.judge_zone.len(),
                u8::from(s.alive)
            )
        })
        .collect();
    // 注意：**不含** `history_seq`（每步都变，会掩盖回环）；
    // 只用「会随动作改变的可观测量」，同摘要再次出现即视为无进展。
    let zones: String = st
        .special_zones
        .iter()
        .map(|(id, z)| {
            let mut n = 0usize;
            for key in ["cards", "revealed"] {
                if let Some(arr) = z.payload.get(key).and_then(|v| v.as_array()) {
                    n += arr.len();
                }
            }
            let remaining = z
                .payload
                .get("remaining")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            format!("{id}:{n}/{remaining}")
        })
        .collect::<Vec<_>>()
        .join(",");
    // 计数/标记/结算栈：技能帧常常只改这些（如限定技 `*_used` 计数），
    // 不纳入会把「合法推进」误判成状态回环。
    let counters: String = st
        .turn_counters
        .iter()
        .filter(|(_, v)| **v > 0)
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(",");
    let flags: String = st
        .seats
        .iter()
        .flat_map(|s| s.flags.keys().cloned().collect::<Vec<_>>())
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "r{} t{}/{} {:?}/{:?} p[{}] picks{} draw{} discard{} proc{} z[{}] c[{}] f[{}] [{}]",
        st.round,
        st.turn,
        st.turn_index,
        st.phase,
        st.turn_phase,
        pending,
        st.setup.picks.len(),
        st.deck.draw.len(),
        st.deck.discard.len(),
        st.deck.processing.len(),
        zones,
        counters,
        flags,
        seats.join(",")
    )
}

/// 调试辅助：把当前 pending 的 kind/actor/skill 拼成短串（错误信息里用）。
trait PendingDebug {
    fn pending_kind_debug(&self) -> String;
}

impl PendingDebug for SgsState {
    fn pending_kind_debug(&self) -> String {
        match &self.pending {
            Some(p) => format!(
                "{}/{}/{} opts[{}] data={}",
                p.kind,
                p.skill.clone().unwrap_or_default(),
                p.actor,
                p.options
                    .iter()
                    .map(|o| o.id.clone())
                    .collect::<Vec<_>>()
                    .join("|"),
                serde_json::to_string(&p.data).unwrap_or_default()
            ),
            None => "-".into(),
        }
    }
}

fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn default_players(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("ai{}", i + 1)).collect()
}

/// 跑一局：`players` 个座位全由 `SgsPolicy` 操控。
pub fn run_self_play(players: usize, seed: u64, max_steps: usize) -> SelfPlayResult {
    run_self_play_with(players, seed, max_steps, FULL_GENERALS)
}

/// 同上，但可指定武将包组合（`general_mode`，如 `standard`）。
pub fn run_self_play_with(
    players: usize,
    seed: u64,
    max_steps: usize,
    general_mode: &str,
) -> SelfPlayResult {
    run_inner(players, seed, max_steps, general_mode, &mut |_, _, _, _| {})
}

/// 带逐步回调的版本（调试 `--trace` / 定位死循环）。
pub fn run_self_play_traced(
    players: usize,
    seed: u64,
    max_steps: usize,
    general_mode: &str,
    trace: &mut dyn FnMut(usize, &SgsState, &str, &str),
) -> SelfPlayResult {
    run_inner(players, seed, max_steps, general_mode, trace)
}

fn run_inner(
    players: usize,
    seed: u64,
    max_steps: usize,
    general_mode: &str,
    trace: &mut dyn FnMut(usize, &SgsState, &str, &str),
) -> SelfPlayResult {
    let mut result = SelfPlayResult::default();
    if players < 5 || players > 8 {
        result.last_error = Some(format!("人数非法：{players}（5–8）"));
        return result;
    }

    let ids = default_players(players);
    let mut reg = EngineRegistry::new();
    reg.register(crate::SanguoshaEngine);
    let mgr = GameManager::with_rng(reg, Box::new(SeededRng::new(seed)));
    let group = format!("selfplay-{seed}");

    let Ok((created, _)) = mgr.create_game("sanguosha", &group, &ids[0]) else {
        result.last_error = Some("create_game 失败".into());
        return result;
    };
    let room = created.room_id.clone();
    for pid in &ids[1..] {
        let _ = mgr.execute_action(&room, pid, "lobby:join");
    }
    if !general_mode.is_empty() {
        let _ = mgr.execute_action(
            &room,
            &ids[0],
            &format!("lobby:set_general_packs:{general_mode}"),
        );
    }

    let policy = SgsPolicy;
    let mut rng = SeededRng::new(seed ^ 0x5DEE_CE66_D1CE_1234);
    let mut steps = 0usize;
    // 空转守卫（两层）：
    // 1) 同一 (actor, action) 连续重复 → 明显的原地打转；
    // 2) 状态摘要回环（最近 24 步内再次出现）→ 覆盖 2 步/多步循环（如「激将↔放弃」）。
    let mut repeat_key = String::new();
    let mut repeat_count = 0usize;
    let mut recent: std::collections::VecDeque<String> = std::collections::VecDeque::new();
    // 回环破环器：同一状态摘要下已试过的动作（仿 `apps/verify-sanguosha.mjs` 的安全推进）。
    let mut tried: std::collections::BTreeMap<String, BTreeSet<String>> =
        std::collections::BTreeMap::new();

    while steps < max_steps {
        let Some(state) = mgr.get_state(&room) else {
            result.stalled += 1;
            result.last_error = Some("状态丢失".into());
            break;
        };
        if state.status == GameStatus::Finished {
            result.finished = true;
            break;
        }
        let Ok(st) = SgsState::parse(&state.data) else {
            result.last_error = Some("状态解析失败".into());
            break;
        };
        if let Some(p) = &st.pending {
            result.pending_kinds.insert(p.kind.clone());
        }

        // 找当前操作者：pending.actor → 选将阶段未选者 → 当前回合玩家。
        let actor = if let Some(p) = &st.pending {
            Some(p.actor.clone())
        } else if st.phase == Phase::Setup {
            st.seats
                .iter()
                .map(|s| s.player_id.clone())
                .find(|pid| {
                    !mgr.legal_actions(&room, pid)
                        .unwrap_or_default()
                        .is_empty()
                })
        } else {
            st.seats.get(st.turn).map(|s| s.player_id.clone())
        };
        let Some(actor) = actor else {
            result.stalled += 1;
            result.last_error = Some("没有可操作者".into());
            break;
        };

        let legal = match mgr.legal_actions(&room, &actor) {
            Ok(l) => l,
            Err(e) => {
                result.last_error = Some(format!("legal_actions 失败: {e}"));
                break;
            }
        };
        if legal.is_empty() {
            result.stalled += 1;
            let kind = st.pending_kind_debug();
            result.last_error = Some(format!(
                "无合法动作（phase={:?} turn_phase={:?} actor={actor} pending={kind} stack={}）",
                st.phase,
                st.turn_phase,
                st.resolution.len()
            ));
            break;
        }
        // 无懈链/五谷等「逐人询问」的交互天然会出现「同一状态摘要再次出现」的合法轮次
        // （构造上由 `passed >= order.len()` / 亮牌数递减保证收敛），不参与回环检测，
        // 否则会把正常的多轮询问误判成死循环并强行改判动作。
        // 只在「交互中（有 pending）」做回环检测：
        // - 无 pending 的重复（如「当先」额的出牌阶段结束后回到同一玩家的出牌阶段、
        //   摸牌被跳过 → 可见状态与前一阶段相同）属于合法流程；
        // - 无懈链/五谷逐人询问天然多轮重复，由构造保证收敛。
        let loop_trackable = matches!(
            st.pending.as_ref().map(|p| p.kind.as_str()),
            Some(kind) if !matches!(kind, "wuxie" | "wugu")
        );
        let digest = state_digest(&st);
        let mut action = match policy.choose(&state, &actor, &legal, &mut rng) {
            Some(a) => a,
            None => {
                result.stalled += 1;
                result.last_error = Some("策略未给出动作".into());
                break;
            }
        };
        if loop_trackable && recent.contains(&digest) {
            // 回环：换一个该状态下没试过的动作，强行推进（仿 `apps/verify-sanguosha.mjs`）
            // 而不是直接判卡死——引擎里「技能发动失败后仍保持合法」的帧需要外部打破。
            let seen = tried.entry(digest.clone()).or_default();
            match legal.iter().find(|a| !seen.contains(&a.id) && a.id != action) {
                Some(a) => {
                    seen.insert(a.id.clone());
                    result.loops += 1;
                    if result.loop_detail.is_none() {
                        result.loop_detail = Some(format!(
                            "step {steps} kind={} actor={actor} 首轮={action} 改判={} digest={digest}",
                            st.pending.as_ref().map(|p| p.kind.as_str()).unwrap_or("-"),
                            a.id
                        ));
                    }
                    action = a.id.clone();
                }
                None => {
                    result.stalled += 1;
                    result.last_error = Some(format!(
                        "空转守卫：状态回环且无可试动作（step {steps}，digest={digest}）"
                    ));
                    break;
                }
            }
        } else if loop_trackable {
            recent.push_back(digest);
            if recent.len() > 24 {
                recent.pop_front();
            }
        }
        let key = format!("{actor}|{action}");
        if key == repeat_key {
            repeat_count += 1;
            if repeat_count >= 8 {
                result.stalled += 1;
                result.last_error = Some(format!("空转守卫：{key} 连续重复 {repeat_count} 次"));
                break;
            }
        } else {
            repeat_key = key;
            repeat_count = 1;
        }
        trace(steps, &st, &actor, &action);
        match mgr.execute_action(&room, &actor, &action) {
            Ok(res) => {
                steps += 1;
                if res.finished {
                    result.finished = true;
                    break;
                }
            }
            Err(e) => {
                // 兜底（安全推进）：引擎拒绝时退到 pass / end_play / 首个合法动作，
                // 让对局继续跑完（这一层保证「AI 与引擎的合法集口径不一致」不会卡死全局）。
                result.illegal += 1;
                let engine_error = format!("{e}");
                let fallback = ["pass", "confirm", "end_play"]
                    .iter()
                    .find(|id| legal.iter().any(|a| a.id == **id))
                    .map(|id| (*id).to_string())
                    .or_else(|| legal.iter().map(|a| a.id.clone()).find(|id| id != &action));
                let Some(fb) = fallback else {
                    let kind = st.pending_kind_debug();
                    result.last_error =
                        Some(format!("非法动作 {action}（pending={kind}）: {engine_error}"));
                    break;
                };
                match mgr.execute_action(&room, &actor, &fb) {
                    Ok(res) => {
                        steps += 1;
                        let kind = st.pending_kind_debug();
                        let before: Vec<String> = st
                            .seat(&actor)
                            .map(|s| s.hand.clone())
                            .unwrap_or_default();
                        result.errors.push(format!(
                            "{action} → 回落 {fb}（pending={kind} hand_before={before:?} {engine_error}）"
                        ));
                        if res.finished {
                            result.finished = true;
                            break;
                        }
                    }
                    Err(e2) => {
                        let kind = st.pending_kind_debug();
                        result.last_error = Some(format!(
                            "非法动作 {action}（pending={kind}）: {engine_error}；回落 {fb} 也失败: {e2}"
                        ));
                        break;
                    }
                }
            }
        }
    }

    result.steps = steps;
    // 终局阵营（若已完成）。
    if let Some(state) = mgr.get_state(&room) {
        if let Ok(st) = SgsState::parse(&state.data) {
            result.winner_team = st.winner.as_ref().map(|w| w.team.clone());
        }
    }
    result
}

/// 跑一批（seed = `seed_base + i`），聚合统计。
pub fn run_batch(
    games: usize,
    players: usize,
    seed_base: u64,
    max_steps: usize,
    general_mode: &str,
) -> BatchResult {
    let mut batch = BatchResult {
        games,
        ..Default::default()
    };
    for i in 0..games {
        let r = run_self_play_with(players, seed_base + i as u64, max_steps, general_mode);
        batch.finished += usize::from(r.finished);
        batch.illegal += r.illegal;
        batch.loops += r.loops;
        batch.stalled += r.stalled;
        batch.steps_total += r.steps;
        batch.steps_per_game.push(r.steps);
        batch.max_game_steps = batch.max_game_steps.max(r.steps);
        batch.pending_kinds.extend(r.pending_kinds);
        if r.loops > 0 {
            batch.errors.push(format!(
                "seed={}: 破环器触发 {} 次（已恢复）：{}",
                seed_base + i as u64,
                r.loops,
                r.loop_detail.clone().unwrap_or_default()
            ));
        }
        if let Some(err) = r.last_error {
            batch.errors.push(format!("seed={}: {err}", seed_base + i as u64));
        }
        for e in &r.errors {
            batch.recovered.push(format!("seed={}: {e}", seed_base + i as u64));
        }
        let team = r.winner_team.unwrap_or_else(|| "none".to_string());
        *batch.wins.entry(team).or_insert(0) += 1;
    }
    batch
}
