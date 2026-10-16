# 拉斯维加斯状态机设计

> 状态：定稿 v1.0（2026-10-16，用户裁定）。
> 通用信封 `GameState.status ∈ { Active, Finished }`；引擎内部 `phase` 更细。
> 本期范围：主流程（lobby/setup/turn/round_end/settling/finished/aborted）；小游戏仅抽取挂载、效果占位。

## 1. 阶段（phase）

| phase | 含义 | 玩家可否动作 |
|---|---|---|
| `lobby` | 开局前游戏设置阶段：等待 @bot 加入/退出 | 是（join/leave/start/abort） |
| `setup` | 每轮设置：拼赌场、抽 3 小游戏、铺钞票、发骰/筹码 | 否（引擎自动） |
| `turn_roll` | 当前玩家已自动掷骰，等待下注/弃回合 | 是（仅当前玩家） |
| `small_game` | 小游戏交互中（需玩家选择/投骰） | 是（触发者或暗定玩家） |
| `round_end` | 本轮所有骰子放完，结算前的小游戏处理（C2/B1 等） | 是（被触发的玩家） |
| `settling` | 逐赌场钞票分配 | 否（引擎自动） |
| `finished` | 三轮结束、已终局计分 | 否 |
| `aborted` | 房主中断结束（**不结算**） | 否 |

> `phase ∈ {lobby, setup, turn_roll, small_game, round_end, settling}` ⇒ `status=Active`；`{finished, aborted}` ⇒ `status=Finished`。

## 2. 状态迁移

```
create_game()                         # 群内无进行中对局
  ▼
lobby ──(join/leave/kick)──▶ lobby
  │  host: set_small_game_count(N)（可选） / 引擎 16→8→N ──▶ lobby
  │  host: start（人数 2–6）
  ▼
setup ──(自动)──▶ turn_roll            # 分配已选小游戏/铺钞票/发骰发筹码
  │
  │  当前玩家：place:n  或  reroll(付1筹码弃回合)
  ▼
small_game? ──触发──▶ small_game ──▶ turn_roll(下一位)
  │ 否
  ▼
下一位仍有骰子？ ──是──▶ turn_roll
  │ 否（所有玩家骰子用尽）
  ▼
round_end（C2 移除平手前 / B1 分配前等）
  ▼
settling（1→6 号赌场逐块分配）
  ▼
第 1/2 轮？ ──是──▶ setup（下一轮：重设钞票/抽小游戏/+2 筹码/重定起始玩家）
  │ 否（第 3 轮结束）
  ▼
finished（终局计分：钞票总额 + 筹码×$1万）

lobby / 任意 Active 阶段 ──host: abort──▶ aborted（不结算，直接终结）
```

## 3. 迁移表（草案）

| 当前 phase | 事件/动作 | 前置条件 | 目标 phase |
|---|---|---|---|
| （创建） | `create_game` | 该群无 Active 对局 | `lobby` |
| `lobby` | `join` | 未在对局列表/未被禁入 | `lobby` |
| `lobby` | `leave` | 在对局列表且未开赛；**发起人退出 → `aborted`（本局废弃）** | `lobby` / `aborted` |
| `lobby` | `kick` | 发起人/房主 + 目标在列表 | `lobby`（移除目标） |
| `lobby` | `set_small_game_count` | 发起人 + 指定 N∈3–6（默认 3，可选） | `lobby` |
| `lobby` | `start` | 发起人 + 人数 2–6 + 已选小游戏 | `setup` |
| `lobby` | `abort` | 发起人/房主 | `aborted` |
| `setup` | （自动完成） | — | `turn_roll` |
| `turn_roll` | `place:n` | 当前玩家 + `n ∈ roll` + 非禁止赌场 | `small_game` 或下一位 `turn_roll` / `round_end` |
| `turn_roll` | `reroll` | 当前玩家 + 筹码 ≥ 1 | 下一位 `turn_roll`（收回骰子） |
| `small_game` | 小游戏动作 | 触发者/相关玩家 | `turn_roll`（或结算子流程） |
| `round_end` | 插入结算（C2/B1） | 对应小游戏被抽中 | `settling` |
| `settling` | （自动） | — | `setup`（下一轮）或 `finished`（第 3 轮） |
| Active 任意 | `abort` | 房主/发起人 | `aborted` |
| `finished`/`aborted` | 任何动作 | — | 拒绝 |

## 4. 终局与中断

- **终局**：第 3 轮 `settling` 完成 → `finished`；计分 = 钞票总额 + 筹码×$1 万；平局依次比钞票张数、筹码数。
- **中断**：房主/发起人 `abort` → `aborted`，**不结算**，直接终结该群对局登记与 token 资源。

## 5. 掷骰与小游戏时序

- 掷骰是**回合开始时的系统动作**（引擎自动，写 `dice_rolled` 事件），玩家只做下注/弃回合/小游戏选择。
- 小游戏可插入不同时机：C2 在「移除平手前」、B1 在「分配钞票前」、C1 在「放第 5 颗骰子时」、其余多在「下注后」。实现需在 `settling`/`turn_roll` 内安排钩子（`small-game-hooks`）。
- 灰色骰（虚拟玩家）与黑色骰结果均入事件，保证重放确定性。

## 6. 边界与异常

- **中途退出**：开赛后不允许退出（用户规则 §8.5）；仅房主可 `kick` 或 `abort`。**发起人/开局人退出 → 本局废弃（`aborted`，不结算）**。
- **重复开桌**：该群已有 Active 对局时，`create_game` 拒绝（不覆盖）。
- **超时/离线**：**不设超时，也没有离线概念**（用户确认 2026-10-13）。游戏以群消息为主题与驱动，群消息天然持久化，玩家上下文不丢失；对局内容全部序列化，中断后尽可能恢复。
- **房间管理**：只有开局人/房主可踢人（`kick`）与 `abort`；加入/退出不限时。
- **人数/去重**：`start` 在引擎内校验人数 **2–6** 与 openid 去重。
- **lobby 消息驱动**：设置阶段的开赛询问等仍由群消息驱动，不经 agent 处理。

## 7. 已裁定与待确认

- 【已确认】发起人退出 → 本局废弃（`aborted`）。
- 【用户裁定 2026-10-16】每轮 `setup` 重新 16→8→N 抽小游戏；第 i 块挂到 `((i-1) mod 3)+1` 号赌场（N>3 可叠放）。
- 【阶段 3 前裁定】各小游戏与主体结算的精确交错顺序（C2/B1/C1/D1）。
- 【阶段 3 前裁定】F1 九格图案、各小游戏奖励数值档位。
