# ADR-0008：引擎事件模型、RNG 注入与 lobby/视图原语

- 状态：已接受（Accepted，2026-10-16 用户裁定）
- 日期：2026-10-16
- 关联：ADR-0002（双模型/MCP/事件溯源）、ADR-0004（快照序列化与 version 对齐）、`docs/modules/game-engine.md`、`docs/games/las-vegas/*`

## 背景

拉斯维加斯引擎落地需要三项框架能力，现有 `game-engine-core` 不足：

1. 引擎无法产出**多条事件**：`ActionResult` 只有 `message`，`start_game`/`execute_action` 由 MCP 统一记一条泛化事件，`version` 硬编码 1/2（ADR-0004 已裁定需修），无法承载「一次动作触发自动掷骰/结算/多事件」。
2. RNG 不可注入：引擎内部直接 `rand::thread_rng()`，复盘无法确定性重放（ADR-0004 约定随机入事件，但需要可注入来源才能测试与重放）。
3. 缺 lobby 生命周期与公共/个人视图：`new_game` 直接开赛，无「创建/加入/开赛」；`get_state` 暴露完整状态（钞票面额未脱敏）。

## 决策

1. **引擎事件序列**：新增 `EngineEvent { event_type, actor: Option<String>, payload: serde_json::Value }`。
   - `EnginePlugin::new_game(...) -> (GameState, Vec<EngineEvent>)`；
   - `ActionResult` 增加 `events: Vec<EngineEvent>`。
   - 每次成功写入把事件逐条 append 到 `game_events`，快照 `version = 最后事件 seq`，**同一事务**（ADR-0004）。
2. **RNG 注入**：新增对象安全 trait `RandomSource { fn next_u64(&mut self) -> u64 }`；引擎提供 `SeededRng`（确定性，测试用）与 `SystemRng`（默认）。`EnginePlugin` 的写方法接收 `rng: &mut dyn RandomSource`；`GameManager` 持有 `Mutex<Box<dyn RandomSource>>`。
3. **lobby 原语形态**：除房间尚未创建时的 `create_game(game_id, group_id, host_id)` 为独立原语外，lobby 生命周期（join/leave/kick/set_small_game_count/start/abort）统一走 `execute_action(action=lobby:*)`。
4. **视图脱敏**：`EnginePlugin::public_state(&state) -> GameState`（默认直通）供 `get_state`；`player_view(&state, viewer)` 默认委托 `public_state`。新增 MCP 原语 `get_player_view(room_id, player_id)`。拉斯维加斯在 `public_state` 中把赌场钞票面额脱敏为张数。
5. **群内唯一活跃对局**：`GameManager::create_game` 在群内已有 Active（含 lobby）时**拒绝**，不再覆盖（修复现有 `start_game` 覆盖缺陷）。

## 理由

- 事件自述是「快照 + 重放」正确性的前提；引擎最了解状态迁移，事件由引擎产出最自然。
- 对象安全 trait + 方法参数注入，兼顾 `dyn EnginePlugin` 存储与确定性测试。
- 统一 `execute_action(lobby:*)` 收敛写入口，减少 MCP 原语面与校验分叉；`create_game` 因无 room 上下文必须独立。
- 默认 public/player 视图直通，保证 `rps` 等简单引擎零改动语义。

## 影响

- 修改 `EnginePlugin` trait 签名，需同步改 `rock-paper-scissors`（接受，见计划待确定项）。
- `storage::record_game_transition` 改为批量事件 + 返回真实 seq（或新增 `append_game_events`）。
- 新增 MCP 原语 `create_game` / `get_player_view`；`get_state` 语义变为脱敏公共视图。
- 数值约束：无新增外部数值约束（均为项目内确定值）。
