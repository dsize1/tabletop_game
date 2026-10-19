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
| `lobby:set_small_games:<csv>` | 指定**开局（第 1 轮）**小游戏列表，如 `H1,G1,F1`（须不同字母组、3–6 个） | 发起人/房主 |
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
| `sg:code:{代码}`（可多选：`sg:code:{A,B}`）/ `#选 A，B` | pending（下注者用**牌面页加密代码**选择 → 左手玩家猜） | 幸运之拳（重做版·裁定 2026-10-17）：代码↔枚数映射只在本人牌面页；猜错按实际枚数给 2 筹码 / $3 万 / $4 万 |
| `sg:B1:put:{1..6}` | pending（**结算阶段** B1 所在赌场胜者） | 主场时间（重做版·§7.4）：投 2 黑骰后选赌场，放「命中该点数」的 0–2 颗黑骰作为自己骰子 |
| `sg:B2:guess:{high\|low}` | pending（触发者） | 50% 的机会：猜大小 |
| `sg:E1:ban:{casino}` / `sg:E1:pass` | pending（触发者） | 禁止进入：放禁止标记（排除本场/已禁止）→ 箭头环形 +1 并领**前进后**格子奖励；pass = 放弃（不动箭头、无奖） |
| `sg:F1:group:{g}` → `sg:F1:move:{g}:{c}` | pending（触发者，**两轮**） | 阻止他：先选组、再选赌场（避免 5×6 共 30 选项） |
| `sg:F2:take:{casino}:{chip\|money}` / `sg:F2:pass` | pending（触发者） | 危机就是转机：取灰骰换奖/放弃 — **已废弃**，见下行重做版 |
| `sg:F2:take:{casino}` → `sg:F2:group:{a\|b\|c}` → （c 组）`sg:F2:put` / `sg:F2:takeback` → `sg:F2:put:{1..6}` / `sg:F2:takeback:{casino}` | pending（触发者，**多段**） | 危机就是转机（重做版·裁定 2026-10-17）：选场上灰骰 → 选未满的组（a=格1–2→1 筹码、b=格3–5→$3 万、c=格6–9→③/④）→ c 组再选放入点数或收回赌场（收回只列自己有骰的赌场）；手中无骰时跳过放入/收回选择直达收回。（**已实现**） |
| `sg:G1:split:{代码[,代码]}` → `sg:G1:pick:{0\|1}` | pending（左手玩家**多选加密代码**分 A 堆 → 赢家**明文**选 A/B） | 黑色摸彩箱：面额 `[2,2,6,0,0,0]` 万 |
| `sg:H2:choose:{1..6}` →（5 点）`sg:H2:put` / `sg:H2:takeback` → `sg:H2:put:{1..6}` / `sg:H2:takeback:{1..6}` | pending（触发者，**多段**） | 我的抉择（重做版）：选项按可用性过滤；4=重掷两骰；5=放入/收回二段；6=放金框 |
| `sg:D2:use:{1..6}` | turn_roll（特权持有者；本回合**不掷骰**，无 `place`/`reroll`） | 权力优势：手动放 1 颗指定点数（触发该赌场小游戏；E1 禁止赌场不列） |
| `sg:G2:move:{0..N}` | pending（**下注到 G2 赌场后**，每次下注都可） | 双倍颁奖：把 0–N 颗自己的骰子移入游戏场（0 = 放弃） |
| `sg:H1:take:{casino\|hand}` → `sg:H1:slot:{1..6}` / `sg:H1:skip` | pending（**下注到 H1 赌场后**，两段式） | 好骰难求（2026-10-17 修订）：先选来源（该赌场自己骰 / 手中骰优先小骰），再**自选 1–6 格**；挤走原骰（大还大、小还小） |

> **「下注」有两种形态**：`place:{n}`（掷骰后下注）与 `sg:D2:use:{n}`（特权下注）——两者都触发「下注后」类小游戏。
> `A2`（大奖挑战）、`C1`（掌声响起：颗数达 5 立即 $10 万）、`D1`、`E2`（酒吧灌酒）为**下注后自动结算**（无交互动作）；`C2` 为结算阶段自动罚金（按赌场顺序，无交互）；**`B1` 已改为结算阶段交互**（见上表 `sg:B1:put:{1..6}`）。含参数的动作用 `:` 分隔。

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
- 交互原语扩展时**只增不改**：`Action` 可追加 `#[serde(default)]` 字段（如交互描述符），但 `id` / `description` / 列表顺序与条数保持稳定（`#选 N` 依赖下标）。
- wide_choice 分组：用户已同意（2026-10-13），**具体分组粒度待实现阶段讨论**。
- 小游戏为 **16→8→N**（每字母组 A–H 各随机取 1 得 8，再随机取 N∈3–6；见 `props/small-games.md`）。参数动作 `sg:set_count:{N}` 用于设置阶段。

## 6. 交互类型标注（端侧语义）

> 定义与判定规则见 `docs/conventions/interaction-primitives.md`（已采纳 v1.0，2026-10-19，ADR-0012）。**Q 群侧不消费本表**（仍为编号清单 + `#选 N`）。

| 动作 / 小游戏 | 类别 | 步类型 | 主体 |
|---|---|---|---|
| `place:n`、`reroll`、`sg:D2:use:n` | 单步 | one | 当前玩家 |
| `sg:B2:guess`、`sg:E1:ban`、`sg:G2:move`、`sg:B1:put`、`sg:H1:slot` | 单步 | one | 触发者/胜者 |
| `sg:F1:group` → `sg:F1:move` | **级联** | one → one | 同人 |
| `sg:F2:take` → `group` → (`put` / `takeback`) → 目标 | **级联**（含条件步） | one ×3–4 | 同人 |
| `sg:H1:take` → `slot` / `skip` | **级联** | one → one | 同人 |
| `sg:H2:choose` → (`put` / `takeback`) → 目标 | **级联**（含循环边：4 点重掷） | one ×1–3 | 同人 |
| `sg:code:{code}` → 猜 1–3 | **接力** | masked one → one | 不同人（下注者 → 左手玩家） |
| `sg:G1:split:{codes}` → `sg:G1:pick:{0\|1}` | **接力** | masked **partition** → one | 不同人（左手玩家 → 赢家） |
| `sg:B1:put`（`round_end` 前） | **异步等待** | one | 胜者 |
| A2 / C1 / D1 / E2 / C2 | 无交互（自动结算） | — | — |

- **级联** 仅限「同一主体 + 连续」；跨主体或异步一律拆为接力 / 异步等待（用户裁定 2026-10-19）。
- **partition**（选中→A 堆、未选→B 堆）的唯一合适控件是**穿梭框**；不为 A/B 分别设置交互。
- APP/WEB 侧若要用「一次提交」，需先补批量执行原语；在此之前端侧也只能逐步提交。
