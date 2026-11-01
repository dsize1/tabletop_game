# ADR-0032：三国杀 AI 分层与「只用可见信息」不变量

- **状态**：已采纳（2026-11-01）
- **相关**：ADR-0019（AI 决策与回放）、ADR-0016（引擎内核）、`docs/games/sanguosha/ai.md`、`docs/games/sanguosha/development-plan.md` P6
- **结论一句话**：三国杀 AI 走「引擎内 Rust 策略（行为树 + 启发式评估）→ 引擎无条件重校验」，
  **并且 AI 只允许读 Web 玩家可见的信息**（`SgsView`）；SystemOne 不参与走子，MCTS 只允许局部一层搜索。

## 背景

1. `bot_action` 原仅对拉斯维加斯用启发式，其余引擎 `RandomPolicy`；三国杀多步交互（11 种 `pending.kind`）随机策略打不完一局。
2. 引擎私有状态 `GameState.data` 含他人手牌、他人身份、牌堆顺序。若 AI 直接读它，会「作弊」，
   且后续用它的决策做训练数据（模仿/蒸馏）会把隐藏信息泄漏进模型，训练出的策略在线上必然退化。
3. 100 局自对弈需要**本地高速 + 确定性**：SystemOne（4B）单次前向延迟与 26 选项上限不适合批量回归。

## 决策

### 1. 分层

```
SgsState ──SgsView::from_state(viewer)──> Evaluator ──handlers(打分)──> tree(取优) ──> Policy::choose
```

- 策略实现放 `crates/engines/sanguosha/src/ai/`，与拉斯维加斯 `ai::HeuristicPolicy` 同形态（`game_engine_core::Policy`）。
- 决策器**只输出动作 id**，合法性/权限由引擎重校验（defense in depth，与 ADR-0008 一致）。

### 2. 只用可见信息（硬约束）

- handler 的输入**只有** `SgsView`：公共区（体力/手牌数/装备/判定区/标记/存活/公开身份/弃牌顶/牌堆余量/可见特殊区）
  + 本人私有（手牌明细、本人身份、本人 pending 选项）。
- 允许公开事实：主公身份、已死亡角色身份、伤害归属（引擎写 `SgsState.observations`，公开信息）。
- 守护：`ai::tests::ai_view_hides_other_hands_and_identities`、`ai::tests::hidden_info_invariance`
  （篡改他人手牌/牌堆顺序后决策必须不变）。

### 3. 决策链不做的事

- **不做 SystemOne 走子**（延迟/确定性/批量回归不合适）；仅保留「离线对照实验」。
- **不做纯 MCTS**（动作空间 = 手牌 × 目标 × 技能，分支爆炸）；中期只做「候选 ≤8 的一层期望搜索」。

### 4. 剪枝

完整合法集 → 打分排序 → 取 ≤26（`GAME_ACTION_CHOICE_LIMIT`），保底 `pass`/`end_play`/最高分动作；
输出 `(candidates, full)` 供决策日志与后续搜索分支控制（ADR-0019）。

## 后果

- 好：AI 行为可解释、确定性、可 100 局回归；训练数据不含隐藏信息；引擎缺陷会被自对弈快速暴露。
- 代价：baseline 对未知身份保守（不主动出手），胜负分布偏反贼；需 P6.5 的信念建模改善。
- 约束：新增引擎技能/交互时，若产生新的 `pending.kind` 或语义，需同步 `ai/handlers` 与
  `ai::tree::KNOWN_PENDING_KINDS`（门禁用例会提示）。
