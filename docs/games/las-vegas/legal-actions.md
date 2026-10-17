# 拉斯维加斯合法动作列表（Legal Actions）

> 状态：定稿 v1.0（2026-10-16，用户裁定，含 lobby；小游戏分支待阶段 3）。规则依据 `rules.md`。

## 1. 动作分层

拉斯维加斯的玩家可见动作分三类：

| 类别 | 阶段 | 动作空间 | 是否 wide_choice |
|---|---|---|---|
| **A. 大厅动作** | `lobby` | 6（join/leave/kick/select_small_games/start/abort） | 否 |
| **B. 回合动作** | `turn_roll` | ≤7（place:1–6 + reroll） | 否 |
| **C. 小游戏动作** | `small_game` / `round_end` | 视抽中的 3 个小游戏而定，单游戏分支 1–6，组合可能 **>26** | **是（需分组/wide_choice）** |

> 结论：**核心主流程（本期实现）动作空间 ≤ 26**（lobby 6 + place:1–6 + reroll；单回合并发合法集更小）；**小游戏分支叠加后（阶段 3）可超过 26**，届时按 `docs/modules/game-engine.md` 的 wide_choice 约定分组。
> wide_choice 分组：用户已同意（2026-10-13），**具体分组粒度待阶段 3 前讨论**。

## 2. 动作 id 编码

统一格式：`<域>:<操作>[:<参数>]`。

### A. 大厅（lobby）

| 动作 id | 描述 | 权限 |
|---|---|---|
| `lobby:join` | 加入当前游戏设置 | 群成员，未在列表 |
| `lobby:leave` | 退出游戏设置 | 在列表且未开赛 |
| `lobby:kick` | 踢出某玩家 | 发起人/房主 |
| `lobby:set_small_game_count` | 指定小游戏数量 N∈3–6（默认 3；每轮 setup 据此重抽） | 发起人/房主 |
| `lobby:start` | 发起游戏（结束设置阶段） | 发起人/房主，人数 2–6 |
| `lobby:abort` | 中断结束游戏（不结算） | 发起人/房主 |

### B. 回合（turn_roll）

| 动作 id | 描述 | 语义 |
|---|---|---|
| `place:1` … `place:6` | 把本轮掷出的该点数全部骰子放到对应赌场 | 同点数整组放置 |
| `reroll` | 支付 1 枚筹码，收回骰子并跳过此回合 | 筹码 ≥ 1 |

合法集 = `{ place:n | n ∈ 当前投掷点数集合 }`（∪ `{reroll}` 若筹码 ≥ 1）。
- 例：掷出 `[1,1,3,6]` → 合法 = `{place:1, place:3, place:6, reroll?}`。
- 禁止赌场（E1 禁止标记）对应的 `place:n` 不合法。

### C. 小游戏（small_game / turn_roll）

**已实现（阶段 3b）**，`get_legal_actions` 只返回当前确实可选的完整动作：

| 动作 id | 触发/归属 | 说明 |
|---|---|---|
| `sg:A1:guess:{1..6}` | pending（左手玩家） | 幸运之拳：猜标记数 |
| `sg:B2:guess:{high\|low}` | pending（触发者） | 50% 的机会：猜大小 |
| `sg:E1:ban:{casino}` / `sg:E1:pass` | pending（触发者） | 禁止进入：改禁止标记（改则箭头移动得奖） |
| `sg:F1:group:{g}` → `sg:F1:move:{g}:{c}` | pending（触发者，**两轮**） | 阻止他：先选组、再选赌场（避免 5×6 共 30 选项） |
| `sg:F2:take:{casino}:{chip\|money}` / `sg:F2:pass` | pending（触发者） | 危机就是转机：取灰骰换奖/放弃 |
| `sg:G1:split:{0..total}` → `sg:G1:pick:{0\|1}` | pending（左手分堆 → 赢家选） | 黑色摸彩箱：二选一 |
| `sg:H2:choose:{1..6}` | pending（触发者） | 我的抉择：选黑骰点数 |
| `sg:D2:use:{1..6}` | turn_roll（特权持有者） | 权力优势：跳过投掷放 1 颗 |
| `sg:G2:move:{casino}` | turn_roll（每轮一次） | 双倍颁奖：移入游戏场 |
| `sg:H1:move:{slot}` | turn_roll（可选） | 好骰难求：移 1 颗骰到格 |

> `A2`（大奖挑战）与 `E2`（醉倒了吗）为**自动结算**（无交互动作）；`C1`/`C2`/`B1` 为结算钩子。含参数的动作用 `:` 分隔。

## 3. 合法性判定（何时哪些动作合法）

| 阶段 | 合法集 |
|---|---|
| `lobby` | 依发送者身份：未加入者 `join`；已加入未开赛 `leave`；发起人 `start`/`abort`/`kick`/`set_small_game_count` |
| `turn_roll` | 当前玩家的 `place:n`（n ∈ 投掷点数、非禁止赌场）∪ `reroll`（筹码≥1） |
| `small_game` | 依触发的小游戏与相关玩家上下文 |
| `round_end` | 依 C2/B1 等触发的结算前动作 |
| `settling`/`finished`/`aborted` | 空集（拒绝） |

## 4. 与 MCP / 决策模型接口

- `get_legal_actions(room_id, player_id)` → `Vec<Action>{id, description}`；超过 26 条时按 wide_choice 分组。
- `execute_action(room_id, player_id, action)` **无条件重校验**：合法性 + 权限 + 状态机前置条件（见 `rule-engine-permission.md`）。
- 大厅动作为「群消息级」原语（join/leave/start/abort），也走同一 `execute_action` 通道（`action = lobby:*`），由引擎校验发起人/成员身份。

## 5. 设计约束

- 动作 id 稳定、机器可解析；description 面向中文零样本决策模型保持口语化。
- 小游戏分支多，id 必须带小游戏编号前缀，便于决策模型聚焦当前上下文。
- wide_choice 分组：用户已同意（2026-10-13），**具体分组粒度待实现阶段讨论**。
- 小游戏为 **16→8→N**（每字母组 A–H 各随机取 1 得 8，再随机取 N∈3–6；见 `props/small-games.md`）。参数动作 `sg:set_count:{N}` 用于设置阶段。
