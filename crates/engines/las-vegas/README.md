# 拉斯维加斯（Las Vegas）桌游引擎

首个桌游引擎，实现 `game-engine-core::EnginePlugin`（`game_id = "las-vegas"`）。

## 规则来源

- 规则：`docs/games/las-vegas/`（**定稿 v1.0，2026-10-16**）
- 权威文本：`memory_palace/artifacts/las-vegas-rules-source-2026-10-13.txt`（用户提供）
- 设计裁定：用户 2026-10-16（钞票 $1–9 万各 10 张、小游戏 N 默认 3 / 每轮重抽 / 从 1 号赌场起挂载、同分并列胜）

## 实现范围

| 阶段 | 内容 | 状态 |
|---|---|---|
| 阶段 0–2 | lobby（join/leave/kick/set_small_game_count/start/abort）→ setup → turn（自动掷骰 / `place:n` / `reroll`）→ 结算（移除平手集合算法）→ 三轮 → 终局计分 | ✅ 已实现 |
| 阶段 3 | A1–H2 共 16 块小游戏效果 + 黑骰/灰骰/标记/结算钩子 | ⏳ 待实现（当前仅 16→8→N 抽取并挂载，效果占位） |

- 私有状态 `LasVegasState.schema_version = 1`（主流程）；小游戏子状态预留 v2 迁移。
- `public_state` 把赌场钞票**面额脱敏为 0、保留张数**（结算前背面朝上）。
- 随机源经 `RandomSource` 注入（`SeededRng` 测试确定性 / `SystemRng` 默认）；随机结果写入事件。

## 动作

- `lobby:join` / `lobby:leave` / `lobby:kick:<openid>` / `lobby:set_small_game_count:<N>` / `lobby:start` / `lobby:abort`
- `place:1` … `place:6`（把本轮该点数全部骰子放到对应赌场；大骰权重 2）
- `reroll`（支付 1 筹码，弃本回合）

## 测试

```bash
# 引擎单测（结算集合算法、三轮终局、脱敏、权限、确定性）
cargo test -p las-vegas

# MCP 端到端冒烟（需先构建 game-engine-mcp）
cargo build -p game-engine-mcp
node apps/verify-las-vegas.mjs
```

> 注：若 `crates/target/debug/game-engine-mcp.exe` 正被 dev 服务占用，可改用
> `CARGO_TARGET_DIR=target-smoke cargo build -p game-engine-mcp`，
> 再 `node apps/verify-las-vegas.mjs crates/target-smoke/debug/game-engine-mcp.exe`。

## 测试 AI（阶段 6）

- `game-engine-core::Policy`：策略抽象（状态 + 合法动作 → 动作 id）；AlphaZero（阶段 7，待定）可替换实现。
- `las-vegas::ai::HeuristicPolicy`：启发式贪心，按攻略「期望分/骰」（基准 **2 分/骰**，1 分 = $1 万）对 `place:n`、`reroll`、`sg:*` 打分；钞票面额隐藏，故用**牌库分布**估第一/第二期望分。
- `las-vegas::ai::run_self_play(players, policies, seed, max_steps)`：自对弈驱动（固定种子可复现），返回终局 winners/scores/steps/事件数。
- 基线：`game-engine-core::RandomPolicy`；单测 `heuristic_beats_random_over_games` 断言启发式显著占优。

```bash
# 自对弈示例：5 局、4 人
cargo run -p las-vegas --example selfplay -- 5 4
```
