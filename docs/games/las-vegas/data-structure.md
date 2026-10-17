# 拉斯维加斯数据结构设计

> 状态：定稿 v1.0（2026-10-16，用户裁定）。序列化方案裁定见 `memory_palace/decisions/ADR-0004-*.md`。
> 当前实现 `schema_version = 6`：主流程 + 小游戏子状态（`sg`：F2 九格/摸彩/指示物/大奖/特权/酒吧/游戏场/金框/禁止/箭头/pending）+ 赌场大骰追踪（`casinos[].big_dice`）。

## 1. 分层结构

`game-engine-core` 用「通用信封 + 引擎私有 JSON」两段式：

```
GameState（通用信封）
├─ game_id / room_id / group_id
├─ status: Active | Finished
├─ players: Vec<String>          # 座位顺序（开赛后固定）
└─ data: serde_json::Value       # 引擎私有状态（LasVegasState）
```

- 信封由 `game-engine-core` 统一管理；`data` 由 `las-vegas` 引擎强类型化 `LasVegasState` 后序列化。
- 大厅（lobby）阶段也复用该信封：`status=Active`，`data.phase=lobby`，此时 `players` 为已加入者。

## 2. 引擎私有状态（LasVegasState，v1）

```jsonc
{
  "schema_version": 6,
  "phase": "lobby",                 // lobby | setup | turn_roll | small_game | round_end | settling | finished | aborted
  "host": "openid",                 // 发起人/房主
  "round": 1,                       // 1..3
  "start_player": "openid",         // 本轮起始玩家
  "turn": 0,                        // 当前回合玩家在 seats 的下标
  "seats": [
    {
      "player_id": "openid",
      "small_remaining": 7,         // 手中剩余小骰
      "big_remaining": 1,           // 手中剩余大骰
      "chips": 2,
      "banknotes": [50000, 30000],  // 已得钞票面额（终局计分）
      "banknote_count": 2,
      "markers": ["grand_prize"]    // 持有的标记（大奖/特权等）
    }
  ],
  "roll": { "small": [1,3,6], "big": 2 },   // 当前玩家本轮投掷结果
  "casinos": [
    {
      "dice": { "openid": 3, "gray": 2 },   // 等效骰数（大骰权重 2）
      "big_dice": { "openid": 1 },           // 其中大骰颗数（H1 取骰不拆大骰）
      "banknotes": [30000, 30000],           // 2 张一组；结算前对玩家视图脱敏为张数
      "small_games": ["A2"]                  // 仅 1-3 号可能非空；N>3 可多块
    }
  ],
  "pools": {                        // 公共骰池/区域
    "game_field": [],               // 游戏场（G2）
    "bars": { "openid": 1 },        // 酒吧（E2）
    "gold_frame": [],               // 金框（H2）
    "gray_dice": [ { "casino": 1, "count": 1 } ],
    "black_dice": 2
  },
  "markers": {                      // 桌面标记
    "banned_casino": null,          // E1 禁止标记
    "arrow_pos": 0,                 // E1 箭头标记
    "lottery": [],                  // G1 黑色摸彩标记
    "indicator_pos": 0              // A2 指示物
  },
  "scores": {}                      // finished 后各玩家终局分（钞票总额 + 筹码*10000）
}
```

| 字段 | 类型 | 说明 |
|---|---|---|
| `schema_version` | u32 | 私有结构 schema 版本（v6：主流程 + 小游戏 `sg` + 大骰追踪 + F2 九格 + E2 酒吧/H2 金框按骰型） |
| `sg` | object | 小游戏子状态（见 `sg.rs`：`f2_slots/gray_groups/bars/game_field/gold_frame/banned/arrow/lottery/indicator/c1_winner/privilege/h1_slots/g2_used/pending`）。`f2_slots: Vec<Option<u8>>` = F2 九格（`Some(来源赌场号)`；组 a=0–1、b=2–4、c=5–8，容量 2/3/4）；原 `sg.gray` 已在 v4 移除（灰骰直接落在 `casinos[].dice["gray"]`）。`pending.data` 中 A1 的 `perm`/`count` 在公共视图被抹除；`sg.pending_options`（`{game,stage,options:[{code,count,reward}]}`）**只出现在 `player_view`**，公共视图恒为 `null`；B1 的 `pending.data = {black:[a,b]}` |
| `phase` | enum | 见 `state-machine.md` |
| `host` | string | 发起人/房主（abort/start 权限） |
| `seats[]` | array | 座位；含大小骰、筹码、已得钞票、标记 |
| `roll` | object | 当前投掷（小骰点数数组 + 大骰点数） |
| `casinos[].dice` | map | 各玩家/灰色骰等效骰数（大骰权重 2） |
| `casinos[].big_dice` | map | 各玩家在该赌场的**大骰颗数**（已在 `dice` 中计入 2 点权重；H1 取骰时用，schema v3） |
| `casinos[].banknotes` | Vec<u32> | 每赌场 2 张钞票面额（**结算前脱敏为张数**） |
| `casinos[].small_games` | Vec<SmallGameId> | 挂载的小游戏（仅 1–3 号；N>3 可多块） |
| `pools` | object | 游戏场/酒吧/金框/灰骰/黑骰 |
| `markers` | object | 桌面标记状态 |

## 3. 信息可见性（脱敏）

- **公开**：骰子（小/大/灰/黑）、筹码、各赌场骰子、游戏场/酒吧/金框、标记归属、`round`/`turn`/`phase`。
- **隐藏**：`casinos[].banknotes` 的**面额**（仅暴露张数）；G1 摸彩标记内容。
- 实现：区分「完整状态」（引擎内部）与「公共视图」（`get_state` 前脱敏 `banknotes` 为张数）。
- 【裁定 2026-10-16】`get_state` 统一返回**公共视图**（脱敏）；新增 `get_player_view(room_id, player_id)` 返回个人视图。引擎以 `EnginePlugin::public_state` 实现脱敏。

## 3.1 钞票牌库

- 牌库：$3–$10 万（张数 11/11/13/15/13/11/9/7，共 90 张）；每轮 `setup` 从全部 90 张**重洗**，随机抽 6 组 × 2 张，按组总额降序铺到 6→1 号赌场（总额同则单张面额大者更高）。
- 抽牌结果为随机，必须写入 `round_setup` 事件 payload（见 ADR-0004）。

## 4. 事件 payload（事件溯源）

| 事件类型 | payload 关键字段 |
|---|---|
| `game_created` | host、group_id |
| `player_joined` / `player_left` | player_id |
| `game_started` | 座位顺序、起始玩家、抽中的 N 小游戏及挂载、各赌场钞票分配（**随机必须入事件**） |
| `round_setup` | round、重抽 N 小游戏及挂载、新钞票分配、每人 +2 筹码、新起始玩家 |
| `player_kicked` | host、target_id |
| `small_game_count_set` | N |
| `dice_rolled` | turn、小骰结果、大骰结果（**必须入事件**） |
| `dice_placed` | player_id、`place:n`、数量、目标赌场 |
| `reroll_paid` | player_id、消耗筹码 |
| `small_game_*` | 各小游戏动作与随机结果（黑骰/摸彩等） |
| `marker_changed` | 标记获得/转移/移动 |
| `round_settled` | 各赌场分配明细（含移除平手过程） |
| `game_settled` | 各玩家终局分 |
| `game_aborted` | host（不结算） |

- 随机结果（洗牌、掷骰、黑骰、摸彩）**一律入事件**，重放不重新随机（见 ADR-0004）。

## 5. 序列化方案裁定要点（详见 ADR-0004）

1. 快照 = 完整 `GameState` JSON（信封 + `data`）。
2. `game_state.version` 对齐 `game_events.seq`；修正现有硬编码 version=1/2。
3. `data.schema_version` 提供私有结构迁移入口（v1 = lobby/主流程/筹码；v2 = 小游戏子状态；v3 = 赌场大骰追踪；v4 = F2 九格、`sg.gray` 移除；v5 = E2 酒吧改 `{small,big}`；v6 = H2 金框改 `GoldFrame{owner,small,big}`）。
4. round-trip：`LasVegasState ↔ JSON` 无损往返；恢复 = 反序列化快照。
