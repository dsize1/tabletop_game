# ADR-0019：三国杀 AI 决策与对局录制回放（训练数据铺路）

- **状态**：提案·待评审（2026-10-25）
- **相关**：`docs/games/sanguosha/plan.md`（机器人维度）、`docs/games/sanguosha/development-plan.md` P6、`docs/games/sanguosha/data-model.md`（`game_decision_log`）；ADR-0004（事件溯源）、ADR-0008（RNG 注入 / Policy 抽象）、ADR-0016（引擎内核）
- **结论一句话**：机器人继续走「引擎给候选 → SystemOne 选一个 → 引擎无条件校验」；**完整合法动作集与选择结果单独落决策日志**，与事件溯源分离；回放 = 快照 + 事件重放，导出 JSONL 供后续训练。

## 背景

1. 立项文档要求：机器人先与拉斯维加斯一致，由 SystemOne 按当前最优决策；同时要能**收集并重放**一局游戏，为后续 AI 训练铺路。
2. 三国杀的动作空间远大于拉斯维加斯（手牌 × 目标 × 技能），现有「≤26 个动作交给 SystemOne 单选」的路径（`GAME_ACTION_CHOICE_LIMIT=26`，`apps/agent-gateway/src/game-command.ts`）不能直接套用。
3. 现有 `Policy` 抽象（`crates/game-engine-core`）与 `HeuristicPolicy`（拉斯维加斯）可复用；`RandomPolicy` 是基线。
4. 事件溯源已保证对局可确定性重放（随机入事件、RNG 可注入），但**没有记录「当时有哪些选择、模型为什么这么选」**，无法做监督学习/离线评估。

## 决策

### 1. 决策链（三层，规则仍刚性）

```
引擎 legal_actions（完整集）
      │
      ├─ 候选剪枝（引擎/策略侧启发式，≤26）──> SystemOne choice ──> 引擎 execute_action（重校验）
      │                                            │
      └─ 完整集 + 候选 + 选择 ───────────────────> game_decision_log
```

- **候选剪枝在引擎/策略侧，不在网关**：只有引擎知道「安全动作/必选动作/收益排序」；剪枝结果必须仍在完整合法集内。
- 剪枝不足 26 时全量给出；超过 26 时按规则优先级（必选响应 > 可致死 > 资源收益 > 其他）截断，并保留「查看全部」兜底路径（Q：清单分页/`#动作 <id>`）。
- 人类玩家动作同样记录（`source='human'`），用于模仿学习与对照。
- 模型输出仍只建议：越权/非法一律由引擎拒绝（ADR-0008/0016）。

### 2. 决策日志（非真相源）

新增 `game_decision_log`（见 `data-model.md`）：

| 字段 | 说明 |
|---|---|
| `game_id` / `room_id` / `seq` | `seq` 关联动作落库后的 `game_events.seq` |
| `player_id` | 决策者（导出时哈希） |
| `source` | `human` / `systemone` / `bot` / `random` |
| `legal_actions` | 完整合法集 JSON（`[{id,description}]`） |
| `candidates` | 剪枝后候选 JSON（≤26） |
| `chosen` / `confidence` / `latency_ms` | 选择与模型元数据 |

- 与 `game_events` **同事务写**（保证 seq 对齐）；不可写时降级为「丢弃日志」，不得阻塞对局。
- 支持采样率（`SGS_DECISION_LOG_RATE`，默认 1；生产可降）；支持整体关闭。
- 日志表可裁剪/归档，不影响对局真相。

### 3. 回放

- 单位：`room_id`；入口 `game_state` 快照 + 其后 `game_events`。
- 重放器（Rust 测试工具 + 脚本）：
  1. 校验 `version == 最后 seq`；
  2. 逐事件重放 → 状态哈希与快照对比（一致性）；
  3. 关联 `game_decision_log`，输出「状态 → 可选 → 选择」时间线。
- 提供两种回放：
  - **确定性重放**：只依赖事件（不需要模型）；
  - **决策回放**：叠加决策日志，用于分析模型行为。
- 随机来源：事件内已含随机结果；重放不重新随机（ADR-0004）。

### 4. 导出与训练数据

- 导出 JSONL，一条决策一行：`{schema_version, game_id, room_id_hash, seq, mode, state_digest, player_hash, legal_actions, candidates, chosen, source, confidence, latency_ms}`。
- `state_digest`：对 `public_state`/`player_view` 的稳定哈希 + 可选完整状态引用（默认不导出完整手牌，避免泄露）。
- **匿名化**：`openid` 用 HMAC-SHA256（带项目密钥）哈希；群 id 同样哈希；导出脚本不落原始 openid。
- 用途：监督学习（人类动作）、离线评估（模型 vs 人类）、自对弈数据（`RandomPolicy` / `HeuristicPolicy` / 模型策略）。

### 5. 自对弈与平衡

- 复用 `Policy` trait：新增 `sanguosha` 启发式策略（首期：保命 > 击杀 > 资源 > 随机），`RandomPolicy` 作基线。
- 自对弈脚本固定 seed、输出胜率/平均回合/技能触发次数；用于技能平衡与回归。
- 自对弈**不写**生产 `game_state`（独立 room/临时 DB），避免污染线上数据。

## 理由（取舍）

| 方案 | 结论 |
|---|---|
| 直接把完整合法集交给 SystemOne | 否：远超单题选择能力，延迟与准确率都会崩 |
| 网关侧剪枝 | 否：网关不理解规则，剪枝会引入非法/错误动作 |
| 决策日志并入 `game_events` | 否：日志可丢失/可采样，且体积远大于事件；混入会破坏事件溯源的稳定性 |
| 只存 `chosen` 不存候选/合法集 | 否：无法做离线评估（不知道当时的选择空间） |
| **引擎剪枝 + 独立决策日志 + 事件重放** | **采纳**：兼顾线上延迟、可回放、可训练 |

## 影响

- `crates/storage` 新增 `game_decision_log`；`game-engine-mcp` 写路径在落事件后写日志（同事务/失败降级）。
- agent-gateway 决策链改为「取候选 → SystemOne → 执行」，需与现有 `chooseGameAction` 兼容（拉斯维加斯路径不变）。
- 导出/回放脚本归 `apps/scripts` 或 `crates/engines/sanguosha/examples`；文档补「如何导出/回放」。
- 隐私：导出默认哈希；原始 openid 仅留在 SQLite 真相源（与现有 chat/profile 同等级）。
- 本 ADR **不引入主动消息**：AI 决策结果仍随用户动作被动回复（ADR-0009 门禁不受影响）。
- Web 优先（ADR-0020）：人类动作在 Web 个人资源区提交，同样进 `game_decision_log`；Q 后置不影响数据采集。

## 依据与验证状态

- `GAME_ACTION_CHOICE_LIMIT=26` 与 SystemOne `choice` 题型的现状：来源仓库代码（`apps/agent-gateway/src/game-command.ts` / `decision.ts` / `systemone.ts`），**已验证**。
- 事件溯源与 RNG 注入：ADR-0004/0008 + `crates/game-engine-core`，**已验证**。
- 候选剪枝的收益/准确率：**待验证**（P6 用 100 局自对弈/人类对局对比；不合格则调整剪枝规则，不改变本 ADR 的日志/回放设计）。
- 决策日志存储成本：**待实测**（按 180 条/局、每条约 1–3KB 估算；`SGS_DECISION_LOG_RATE` 可降采样）。
- 本 ADR 不新增外部数值约束（无频控/配额）。
