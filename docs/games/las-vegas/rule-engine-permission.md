# 拉斯维加斯规则引擎与权限校验设计

> 状态：草案 v0.2（据用户裁定规则重写，含 lobby 权限与小游戏结算）。

## 1. 校验链（写入路径，确定性，无条件重校验）

与 `docs/modules/game-engine.md` 一致，四步：

1. **合法性**：动作是否在当前合法动作集内（见 `legal-actions.md`）。
2. **权限**：发起人/房主、成员身份、回合归属、资源归属。
3. **执行**：状态机迁移（见 `state-machine.md`）。
4. **事件溯源**：append event + 更新快照（同一事务）。

> 引擎不信任任何上游（含 agent / 决策模型），`execute_action` 必须重新完整校验。

## 2. 合法性校验

| 检查 | 规则 | 拒绝错误 |
|---|---|---|
| 对局状态 | `status == Active` 且 `phase` 与动作匹配 | `IllegalAction("对局已结束/阶段不符")` |
| 动作格式 | `lobby:*` / `place:n` / `reroll` / `sg:*` 可解析 | `IllegalAction("无法识别的动作")` |
| 点数存在 | `place:n` 且 `n ∈ roll.small ∪ {roll.big}` | `IllegalAction("本轮未掷出该点数")` |
| 禁止赌场 | `place:n` 且 `n != markers.banned_casino` | `IllegalAction("该赌场被禁止进入")` |
| 小游戏分支 | 与当前挂载小游戏/上下文匹配 | `IllegalAction("小游戏动作非法")` |

## 3. 权限校验

| 检查 | 规则 | 拒绝错误 |
|---|---|---|
| 大厅-加入 | 群成员、未在列表、无 Active 竞争对局 | `PermissionDenied` |
| 大厅-退出 | 在列表且 `phase==lobby` | `PermissionDenied` |
| 大厅-开赛 | `player_id == host` 且人数 ≥ 最少人数 | `PermissionDenied("非发起人/人数不足")` |
| 大厅-中止 | `player_id == host`（房主/发起人） | `PermissionDenied("仅房主可中断")` |
| 大厅-踢人 | `player_id == host` 且目标在列表 | `PermissionDenied("仅房主可踢人")` |
| 大厅-设小游戏数 | `player_id == host` 且 N∈3–6（可选） | `PermissionDenied("仅房主可设")` |
| 大厅-发起人退出 | 退出者为发起人 → 本局废弃（`aborted`） | — |
| 在局 | 开赛后 `player_id ∈ seats[].player_id` | `PermissionDenied("玩家不在本局")` |
| 轮次 | `place:n`/`reroll` 时 `player_id == seats[turn].player_id` | `PermissionDenied("未轮到你")` |
| 筹码 | `reroll` 需 `chips >= 1` | `IllegalAction("筹码不足")` |
| 小游戏归属 | 动作发起者须为触发者/规则指定的玩家（如 A1 猜数者在其左、G1 分堆者） | `PermissionDenied` |
| 开赛后退局 | `phase != lobby` 时 `lobby:leave` 拒绝 | `PermissionDenied("开赛后不可退出")` |

## 4. 执行（状态迁移）

### 4.1 大厅

- `join`：加入 `seats`（未开赛）；`leave`：移除（**发起人退出 → `aborted`**）；`kick`：移除目标；`set_small_game_count`：记录 N∈3–6；`start`：进入 `setup`（引擎 16→8→N 随机抽小游戏）；`abort`：`phase=aborted`、`status=Finished`（**不结算**）。

### 4.2 回合

`place:n` 通过校验后：
1. 从 `roll` 取出所有点数 n 的骰子（小骰按颗数、大骰若为 n 计权重 2）；
2. `casinos[n-1].dice[player] += 权重`；
3. `seats[turn].small_remaining/big_remaining` 相应扣减；
4. 触发小游戏判定（1–3 号赌场下注 → `small_game` 钩子）；
5. 下一位玩家自动掷骰（写 `dice_rolled`）；全部骰子用尽 → `round_end`。

`reroll`：扣 1 筹码，收回本轮骰子，跳过本回合（写 `reroll_paid`）。

## 5. 小游戏与结算（确定性，引擎内计算，不用 LLM）

### 5.1 小游戏钩子

| 时机 | 小游戏 | 说明 |
|---|---|---|
| 下注后 | A1/A2/B2/D1/D2/E1/E2/F1/F2/H1/H2 | `turn_roll` 内联子流程 |
| 放第 5 颗骰子时 | C1 | 即时检查 |
| 每轮分配钞票前 | B1 | `round_end` 钩子 |
| 移除平手前 | C2 | `round_end` 钩子 |

- 所有随机（黑骰 2 颗、摸彩分堆、A1 手内标记数）写入事件。

### 5.2 钞票分配（逐赌场 1→6）

1. （若有 C2）先执行 C2 支付。
2. **移除平手（集合算法，用户裁定）**：取该赌场骰数 > 0 的玩家（含灰色骰虚拟玩家），按骰数分组；**任何骰数相同的组（≥2 名）整组剔除**。
3. **排名**：剩余玩家按骰数降序，**前 2 名**分别拿该赌场**最高 / 次高**面额钞票；等效骰数按「大骰在赌场权重 2、其余一律 1」计算。
4. 累加至 `seats[].banknotes` / `banknote_count`。
5. 灰色骰（`"gray"`）参与剔除与排名，但不获得钞票。

伪代码：
```
counts = {p: effective_dice(p, casino) for p in players+gray if effective_dice > 0}
by_value = group counts by dice value
kept = [p for p,v in counts if size(by_value[v]) == 1]
ranking = sort(kept by counts[p] desc)
assign(ranking[0], highest_note); assign(ranking[1], second_note)
```

> 示例见 `rules.md` §5（`{红3,蓝2,黄2,绿1}` → `[红3,绿1]`；`{红3,蓝3,黄2,绿1}` → `[黄2,绿1]`）。名次不足 2 人时空缺不发。

### 5.3 终局

- 第 3 轮 `settling` 完成 → `scores[p] = Σbanknotes + chips×10000`。
- 比较：总分 → 钞票张数 → 筹码数。

## 6. 事件溯源对齐

- 每次 `execute_action` 成功 → 同一事务：`INSERT game_events` + `UPDATE game_state`。
- 掷骰/抽小游戏/洗牌/结算/中止均写事件；`version` 对齐最新 `seq`（见 ADR-0004）。
