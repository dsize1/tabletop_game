# 拉斯维加斯合法动作列表（Legal Actions）

> 状态：草案 v0.2（据用户裁定规则重写，含 lobby 与小游戏）。规则依据 `rules.md`。

## 1. 动作分层

拉斯维加斯的玩家可见动作分三类：

| 类别 | 阶段 | 动作空间 | 是否 wide_choice |
|---|---|---|---|
| **A. 大厅动作** | `lobby` | 6（join/leave/kick/select_small_games/start/abort） | 否 |
| **B. 回合动作** | `turn_roll` | ≤7（place:1–6 + reroll） | 否 |
| **C. 小游戏动作** | `small_game` / `round_end` | 视抽中的 3 个小游戏而定，单游戏分支 1–6，组合可能 **>26** | **是（需分组/wide_choice）** |

> 结论：**总体动作空间可超过 26**（8 种小游戏分支叠加），需按 `docs/modules/game-engine.md` 的 wide_choice 约定分组返回；单回合内实际并发合法集通常仍 ≤ 26。
> wide_choice 分组：用户已同意（2026-10-13），**具体分组粒度待实现阶段讨论**。

## 2. 动作 id 编码

统一格式：`<域>:<操作>[:<参数>]`。

### A. 大厅（lobby）

| 动作 id | 描述 | 权限 |
|---|---|---|
| `lobby:join` | 加入当前游戏设置 | 群成员，未在列表 |
| `lobby:leave` | 退出游戏设置 | 在列表且未开赛 |
| `lobby:kick` | 踢出某玩家 | 发起人/房主 |
| `lobby:set_small_game_count` | 指定小游戏数量 N∈3–6（默认 3） | 发起人/房主 |
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

### C. 小游戏（small_game / round_end）

按用户规则的 16 个编号定义分支（示例，完整前缀见 §4）：

| 动作 id 模式 | 说明 |
|---|---|
| `sg:A1:guess:{0..k}` | 幸运之拳：左手玩家猜标记数 |
| `sg:A2:roll` | 大奖挑战：投 2 黑骰 |
| `sg:B2:guess:{high\|low}` / `sg:B2:take` / `sg:B2:again` | 50% 的机会 |
| `sg:D1:take` | 发薪愉快：结算奖励 |
| `sg:D2:use` | 权力优势：使用特权标记指定点数放骰（配合 `point` 参数） |
| `sg:E1:ban:{casino}` / `sg:E1:arrow` | 禁止进入 |
| `sg:E2:take` | 醉倒了吗：收回酒吧骰子 |
| `sg:F1:move:{cell}:{casino}` | 阻止他：移动某格全部骰子 |
| `sg:F2:{chip\|money\|place\|recall\|pass}` | 危机就是转机：四选一/放弃 |
| `sg:G1:pick:{0\|1}` | 黑色摸彩箱：二选一 |
| `sg:G2:move` | 双倍颁奖：移入游戏场（仅一次） |
| `sg:H1:move:{source}` | 好骰难求：移动 1 颗骰子到格子 |
| `sg:H2:choose:{1..6}` | 我的抉择：选择黑骰点数 |

> 含参数的动作用 `:` 分隔；引擎在 `get_legal_actions` 中只返回**当前确实可选**的完整动作（含参数枚举）。

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
