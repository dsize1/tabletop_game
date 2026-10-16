# game-engine（游戏引擎框架，MCP 形态）

- **语言**：Rust
- **位置**：`crates/game-engine-core`（核心）+ `crates/engines/*`（具体引擎）+ MCP 暴露层

## 职责

- `game-engine-core`：EnginePlugin trait、引擎注册表、对局/回合状态管理
- `engines/las-vegas`：首个引擎「拉斯维加斯」
- MCP 暴露层：把 EnginePlugin 映射为 **MCP 统一原语**，以 stdio 暴露给 agent-gateway

## MCP 统一原语（所有引擎通用）

> 原语定义、能力描述符（`name/rw/description/usage/example`）与能力查询规范见 `docs/conventions/mcp-primitives.md`。

### 只读（查询）

| 工具 | 说明 |
|---|---|
| `get_rules(game_id)` | 游戏规则 |
| `get_active_game(group_id)` | 当前群活跃对局（无则空） |
| `get_state(game_id, room_id)` | 当前对局状态 |
| `state_summary(room_id)` | **局面摘要**（紧凑文本视图，给主持人叙述；引擎自定义 `summarize` + 统一截断 `SUMMARY_MAX_CHARS=300`） |
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

## MCP 统一原语评估（针对拉斯维加斯，v0.2）

> 状态：草案 v0.2（据用户裁定规则重写）。评估时机：规则/动作/状态机定稿后进行。

### 结论：保留现有原语 + **新增开局大厅（lobby）相关原语**

拉斯维加斯引入「开局前游戏设置阶段」（用户需求），需要一组**对局生命周期**原语（创建/加入/退出/开赛/中止）。其余原语保留。

### 1. 文档与实现签名不一致（需统一）

| 原语 | 文档签名 | 实现签名 | 处置 |
|---|---|---|---|
| get_state | `(game_id, room_id)` | `(room_id)` | room_id 唯一即可，文档去 game_id |
| get_legal_actions | `(game_id, room_id, player_id)` | `(room_id, player_id)` | 同上 |
| execute_action | `(game_id, room_id, player_id, action)` | `(room_id, player_id, action)` | 同上 |
| start_game | `(game_id, group_id, players)` | 同 | 需增加人数/去重校验 |

### 2. 逐原语评估

| 原语 | 拉斯维加斯需求 | 结论 |
|---|---|---|
| get_rules | 返回 String 给 LLM | 保留；web 展示规则走 server HTTP 资源 API，不经 MCP |
| get_active_game | 查群内活跃对局 | 保留 |
| get_state | 返回对局状态；钞票面额脱敏 | 保留，补「玩家视图脱敏」说明 |
| get_legal_actions | lobby + place:n + reroll + 小游戏分支 | 保留；**>26 时需 wide_choice/分组** |
| start_game | 由 lobby 阶段派生（不再直接开桌） | 增强：改为「lobby → start」两步 |
| execute_action | 无条件重校验 + 执行 + 事件溯源 | 保留（lobby 动作也走此通道） |
| handle_player_message | 预留 | 暂不启用 |

### 3. 新增原语（lobby 生命周期，建议）

| 新原语 | 说明 | 校验 |
|---|---|---|
| `create_game(game_id, group_id, host_id)` | 群内无 Active 对局时创建，进入 `lobby` | 无竞争对局 |
| `join_game(room_id, player_id)` | @bot 加入游戏列表 | 群成员、未在列表 |
| `leave_game(room_id, player_id)` | 退出（仅开赛前） | 在列表且 `phase=lobby` |
| `start_game_from_lobby(room_id, host_id)` | 发起人开赛，进入 `setup`/首轮 | 发起人、人数达标 |
| `abort_game(room_id, host_id)` | 房主中断结束，**不结算**直接终结 | 房主/发起人 |
| `kick_player(room_id, host_id, target_id)` | 房主踢出某玩家（仅 lobby） | 房主/发起人 |

> 注：上述原语也可统一并入 `execute_action(action=lobby:*)`；本表按「生命周期原语」列出，便于网关/主持人调用。
> 与 dev-task 的「token 重签」无关；游戏侧 token 由 `auth` 签发，见 `docs/modules/auth.md`。

### 4. 缺口与处置（待确认）

| 缺口 | 影响 | 处置建议 |
|---|---|---|
| 无 get_player_view | 骰子公开、钞票隐藏；其余公开 | 先以 get_state 脱敏实现；未来隐藏手牌引擎再抽象 |
| 无 resign/弃局 | 开赛后不允许退出；仅房主 abort | **新增 `abort_game`（房主，不结算）**；玩家 resign【待确认】 |
| 无对局超时/挂起 | 玩家长时间不动作 | **不设超时、无离线概念**；群消息持久化，中断后从可恢复点恢复 |
| 随机结果不入事件（RNG 不可注入） | 复盘无法确定性重放 | ADR-0004 裁定：随机结果入事件、RNG 可注入（实现阶段做） |
| version 硬编码 1/2 | 多轮对局快照 version 错乱 | ADR-0004 裁定：对齐 game_events.seq（实现阶段修） |

### 5. 两类出口边界（明确）

- **MCP 原语**：agent-gateway / 主持人 / 决策模型的**运行时**入口（规则查询、lobby 生命周期、开桌、状态、合法动作、执行）。
- **server HTTP 资源 API**：web 站点的**展示**入口（规则文件、道具目录、对局公共视图），不经 MCP、不直连 SQLite。
- 道具/规则文件放公共目录（`docs/games/<game_id>/`），web 经 server 资源 API 读取（根路径待确认）。
