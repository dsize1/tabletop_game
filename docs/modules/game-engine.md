# game-engine（游戏引擎框架，MCP 形态）

- **语言**：Rust
- **位置**：`crates/game-engine-core`（核心）+ `crates/engines/*`（具体引擎）+ MCP 暴露层

## 职责

- `game-engine-core`：EnginePlugin trait、引擎注册表、对局/回合状态管理
- `engines/las-vegas`：首个引擎「拉斯维加斯」
- MCP 暴露层：把 EnginePlugin 映射为 **MCP 统一原语**，以 stdio 暴露给 agent-gateway

## MCP 统一原语（所有引擎通用）

### 只读（查询）

| 工具 | 说明 |
|---|---|
| `get_rules(game_id)` | 游戏规则 |
| `get_active_game(group_id)` | 当前群活跃对局（无则空） |
| `get_state(game_id, room_id)` | 当前对局状态 |
| `get_legal_actions(game_id, room_id, player_id)` | 玩家合法动作列表 |

### 写入（受校验）

| 工具 | 说明 |
|---|---|
| `start_game(game_id, group_id, players)` | 开桌（确定性） |
| `execute_action(game_id, room_id, player_id, action)` | 执行动作（**无条件重校验**） |
| `handle_player_message(...)` | 预留：整条消息交给引擎（如需引擎侧编排） |

## 校验链（写入路径，确定性）

1. **合法性**：动作是否在合法动作集内
2. **权限**：是否轮到该玩家、资源是否归属其所有
3. **执行**：状态机迁移
4. **事件溯源**：append events + 更新快照（同一事务）

> 决策模型的输出只作为"建议"传入，引擎不信任任何上游（含 agent），必过 1/2 两步。

## 事件溯源

- `events` 表 append-only：`seq` 自增 + `event_id` 唯一 + 状态变更事件
- `game_state` 为快照，`version` 对齐最后一个已应用 event 的 `seq`
- 复盘 = 快照 + 重放其后 events；中断对局放宽到「最近操作可追溯」
- 详见 `docs/conventions/persistence.md`

## 约定

- 新引擎放 `crates/engines/<name>/`，实现 EnginePlugin 并在 server 注册
- 规则文档缺失时先补文档再实现
- 动作空间 ≤ 26 直接映射；超 26 依赖 decision-service 的 wide_choice 或引擎侧分组
