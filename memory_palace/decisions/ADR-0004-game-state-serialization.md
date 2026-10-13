# ADR-0004：游戏状态序列化方案与 version 对齐

- 状态：提议中（Proposed，待用户确认）
- 日期：2026-10-12
- 关联：ADR-0002（事件溯源）、`docs/conventions/persistence.md`、`docs/games/las-vegas/data-structure.md`

## 背景

`game-engine-core` 采用「通用信封 `GameState` + 引擎私有 `data: serde_json::Value`」结构，事件溯源要求 `game_state.version` 对齐 `game_events.seq`。现状存在两个隐患：

1. `game-engine-mcp` 中 `record_game_transition` 的 `version` 被硬编码为 1（开桌）/ 2（执行动作），多轮对局（如拉斯维加斯一局多动作）会错乱。
2. 引擎私有状态在 `data` 中以无 schema 的 JSON 存放，缺少迁移入口；且随机结果（洗牌/掷骰）若不入事件，复盘无法确定性重放。

## 决策

1. **快照序列化**：`game_state.state` 存完整 `GameState` JSON（信封 + `data`）；`data` 内为引擎私有强类型结构（如 `LasVegasState`）序列化，并带 `schema_version` 字段。
2. **version 对齐**：`game_state.version` 必须等于该快照「最后一个已应用事件」的 `game_events.seq`；写入时取事件 INSERT 后返回的真实自增 seq，**废除硬编码 1/2**。
3. **事件自包含**：随机结果（开桌洗牌分配、每次掷骰结果）一律写入对应事件 `payload`；重放时不重新随机，保证确定性。
4. **迁移策略**：`data.schema_version` 作为引擎私有结构演进入口；加载快照时按版本迁移，未知高版本拒绝或保守降级（实现阶段再细化）。
5. **引擎 trait 不变**：`EnginePlugin` 继续以 `GameState` 为边界，私有强类型结构仅在引擎内部使用，避免破坏已有引擎（rps）。

## 理由

- 保持 `EnginePlugin` 稳定、各引擎私有状态隔离（备选：改 trait 用泛型/枚举，改动面大，收益低）。
- 事件自包含是事件溯源可重放的前提；随机不入事件则「复盘 = 快照 + 重放」不成立。
- version 对齐是「快照 + 增量重放」正确性的基础，属必须修复的既有缺陷。

## 影响

- 正面：多轮对局事件/快照一致性、可确定性复盘、私有结构可演进。
- 代价：实现阶段需改造 `record_game_transition`（返回/使用真实 seq）；RNG 需可注入（测试固定骰子序列）。
- 数值约束：无新增外部数值约束。

## 依据与验证状态

- 无外部数值依赖；「version 硬编码 1/2」为代码现状事实（已核实，见 `crates/game-engine-mcp/src/main.rs`）。
