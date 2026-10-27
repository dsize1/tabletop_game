# 交互原语：单选 / 多选 / 分区 / 级联 / 接力

> 状态：已采纳（v1.0，2026-10-19，用户裁定）。决策记录：`memory_palace/decisions/ADR-0012-interaction-primitives.md`。
> 目的：为 **APP / WEB 端侧**的交互界面预留引擎侧语义；**Q 群消息侧保持现状不变**（现状即定稿）。
> 关联：`docs/modules/game-engine.md`、`docs/games/las-vegas/legal-actions.md`、`docs/modules/board-view.md`、`docs/conventions/mcp-primitives.md`。

## 1. 动机

- 引擎目前只产出扁平列表 `Action { id, description }`（`crates/game-engine-core/src/lib.rs`），消费端只能渲染成「编号清单 + 输命令」。
- Q 群侧把这套做到了极致（`#选 N` / `#放 N` / `#选 代码`，尽量少打字），**今后不再改动**（用户 2026-10-19）。
- APP / WEB 端要的是「一次会话内完成一次交互」，因此需要引擎侧说清一次逻辑交互的**结构**：几步、每步单选/多选/分区、谁能操作、是否密文。
- **引擎侧只定义语义，控件由消费端决定**；Q 侧不消费这套语义也完全自洽。

## 2. 原语

分类轴：**基数**（one / many / partition）× **可见性**（plain / masked）× **步数与主体**（单步 / 级联 / 接力 / 异步）。

| 原语 | 语义 | 基数 | 合适控件 | 拉斯维加斯实例 |
|---|---|---|---|---|
| **单选 `one`** | 在候选集中取 1 个 | 1 | 单选列表 / radio | `place:n`、`reroll`、`sg:B1:put:{c}`、`sg:B2:guess`、`sg:E1:ban`、`sg:G2:move:{k}`、`sg:H1:slot:{n}`、`sg:D2:use:{n}` |
| **多选 `many`** | 取任意子集，**无补集语义** | 0..n | 复选列表 | **暂无实例（原语保留，用户裁定 2026-10-19）** |
| **分区 `partition`** | 候选集二分：选中→A 堆，未选→B 堆 | 补集 | **穿梭框（唯一合适控件）** | `sg:G1:split:{codes}` |
| **有序 `ordered`** | 取全部候选并**给出顺序**（顺序本身是结果） | n | 槽位列表 + 拖拽排序（点击移位兜底） | **观星：2×5 槽排序**（`arrange:<csv>`；见 `docs/games/sanguosha/ui-spec.md` §7） |
| **级联 `cascade`** | **同一主体 + 连续**的多步，每步是上面三种之一 | — | 向导 / stepper，一次提交 | F2（3–4 步）、H2（1–3 步，含循环）、F1（2 步）、H1（2 步） |
| **接力 `handoff`** | 上一步与下一步**主体不同** | — | 各端各自完成，**不得合并为一次提交** | A1（下注者选码 → 左手玩家猜）、G1（左手玩家分堆 → 赢家选堆） |
| **异步等待 `async gate`** | 需跨越他人动作 / 阶段交界才能继续 | — | 分段提示 + 等待态 | B1（`round_end` 结算前）、C2 |

> 「加密」不是独立形态，而是**正交的可见性轴** `vision = masked`：引擎只下发一次性代码，代码↔真义映射只出现在本人 `player_view`。

## 3. 判定规则（用户裁定 2026-10-19）

1. **级联 = 同一主体 + 连续**。只有同时满足这两条的多步交互才可归为级联；级联的价值是**一次消息 / 一次会话内完成交互**。
2. **主体不同，或不能连续（异步）→ 都不是级联**，必须拆成独立交互（接力 / 异步等待）。每个独立段内部若仍是同主体多步，可各自再构成级联。
3. 若对「A 堆 / B 堆」**分别设置会让交互过于复杂**，合适的控件只有**穿梭框**（即 `partition` 原语）。
4. 同 2：异步 / 跨主体一律不套用级联。

> 推论：Q 群消息侧本来就是「一条消息完成一步」，所以级联在 Q 侧**没有收益**（只能靠多轮 `#选 N`）；级联的收益只存在于 APP / WEB。

## 4. 引擎侧描述符（草案，未实现）

```ts
interface Interaction {
  game: string;              // 小游戏号（A1/F1/F2/G1/H1/H2/B1/…）；非小游戏为 "core" / "lobby"
  actor: string;             // openid，或角色表达式（"left_of:{id}" / "winner_of:{casino}"）
  steps: InteractionStep[];  // 长度 1 = 单步；>1 且同主体 = 级联
  handoff?: { afterStep: number; next: string }; // 接力点（非级联）
}

interface InteractionStep {
  kind: "one" | "many" | "partition" | "ordered";
  min?: number; max?: number;   // kind=many/partition 的基数约束；partition 默认 0..n
  slots?: string[];             // kind=ordered 的槽位 id（顺序即槽位声明顺序）；三国杀观星：L1..L5,R1..R5
  vision: "plain" | "masked";   // masked 需短时 token 的个人视图
  options: { id: string; label: string; value?: unknown }[];
  dependsOn?: number;           // 候选集依赖第 n 步的选择
  optional?: boolean;           // 条件步（可能不出现）
  loop?: boolean;               // 可回到第 1 步（如 H2 的 4 点重掷）
}
```

约束：

- `options[].label` 必须可直接展示；`masked` 时引擎只给代码，真义在 `player_view`（复用现有 `sg.pending_options` 结构）。
- `kind=ordered` 只表达「槽位 + 最终顺序」：`slots` 的声明顺序即默认顺序，提交时只传**跳空后的有序 id 列表**（`arrange:<csv>`）；**手势、长按、吸附半径等一律属端侧实现细节**，不得写进契约。
- 若落地，`game-engine-core::Action` **只允许追加 `#[serde(default)]` 字段**，**不得修改或重排现有 `id` 与 `description`**（见 §6）。

## 5. 拉斯维加斯逐游戏标注

| 小游戏 / 动作 | 交互链 | 主体 | 类别 | 步类型 |
|---|---|---|---|---|
| `place:n` / `reroll` / `sg:D2:use:n` | 单步 | 当前玩家 | 单步 | one |
| `sg:B2:guess` / `sg:E1:ban` / `sg:G2:move` / `sg:B1:put` / `sg:H1:slot` | 单步 | 触发者/胜者 | 单步 | one |
| `sg:F1:group` → `sg:F1:move` | 选组 → 选赌场 | 同人 | **级联** | one → one |
| `sg:F2:take` → `group` → (`put`\|`takeback`) → 目标 | 灰骰 → 组 → 动作 → 目标 | 同人 | **级联**（含条件步） | one ×3–4 |
| `sg:H1:take` → `slot` / `skip` | 来源 → 格位 | 同人 | **级联** | one → one |
| `sg:H2:choose` → (`put`\|`takeback`) → 目标 | 点数 → 动作 → 目标；4 点回第 1 步 | 同人 | **级联**（含循环边） | one ×1–3 |
| `sg:code:{code}` → 猜 1–3 | 下注者选码 → 左手玩家猜 | **不同人** | **接力** | masked one → one |
| `sg:G1:split:{codes}` → `sg:G1:pick:{0\|1}` | 左手玩家分堆 → 赢家选堆 | **不同人** | **接力** | masked **partition** → one |
| `sg:B1:put`（`round_end` 前） | 结算前插入 | 胜者 | **异步等待** | one |
| A2 / C1 / D1 / E2 / C2 | 无交互（自动结算） | — | — | — |

## 6. 对 Q 群消息侧的影响（结论：零改动）

现状链路：`get_legal_actions` → agent-gateway 编号清单 → 玩家 `#选 N` → 取 `legal[n-1].id`（`apps/agent-gateway/src/index.ts`、`game-command.ts`）。

- **附加字段对 Q 侧无感**：Q 侧只读 `{id, description}`，JSON 解析忽略未知字段。新增 `interaction` 属**只增不改**。
- **列表顺序与条数不得变**：`#选 N` 依赖下标。级联**不得默认**合并成一个「一键完成」动作（会改变条数与顺序，并让 Q 提示退化）。若将来要合并，必须：① 默认关闭；② 显式开关（lobby 设定或独立命令）；③ 保留原有分步 id。
- **`description` 保持人类可读**：结构化字段不得替代文案。
- 因此 Q 侧的多步交互**仍是多轮 `#选 N`、多条消息**——这是 Q 侧的最终形态（用户 2026-10-19 确认）。
- 密文交互不变：牌面页看代码 + 群里发 `#选 代码`；token 5 分钟、过期 `#恢复` 重发。
- `pending_queue` 排队、`resume` / `hint` 恢复语义均不受影响。

## 7. 消费端渲染对照

| 原语 | Q 群（现状，定稿） | WEB / APP（规划） |
|---|---|---|
| `one` | 编号清单 + `#选 N` | 单选列表点击，提交 1 步 |
| `many` | 同左，多次消息 | 复选列表 + 提交 |
| `partition` | `#选 A，B`（逗号分隔多代码） | **穿梭框**（候选 / A 堆），一次提交生成 `sg:G1:split:{codes}` |
| `ordered` | 后置（观星等特殊区不做群交互） | **槽位 + 拖拽排序**，点击移位兜底；一次提交 `arrange:<csv>` |
| `masked` | 牌面页只读 + 群里发代码 | token 面板内直接点选（同一 `player_view` 数据） |
| 级联 | 多轮 `#选 N`（多消息，合理且唯一） | 向导 / stepper，全部选完**一次提交** |
| 接力 | 每人各自 `#选 N` | 各自完成，**不允许代填他人** |
| 异步等待 | 各阶段提示 | 等待态展示 |

## 8. 端侧落地的前置缺口（待定）

1. **批量提交语义**：级联要「一次提交」，需要 `execute_actions(room_id, player_id, [action...])`，服务端顺序执行 + 整链校验；任一步失败**不得留下部分状态**（事务/回滚）。当前仅支持单 action。
2. **`get_interaction` 原语（可选）**：与 `get_legal_actions` 并存，Q 侧不调用，WEB/APP 调用；命名与归属按 `docs/conventions/mcp-primitives.md` 规范确定。
3. **`partition` 的 id 编码**：`sg:G1:split:{code1,code2}` 已存在，端侧可直接复用，无需新 id。
4. **`masked` 的 token 粒度**：现为整页短 token；端侧若需局部刷新再评估是否拆分（暂不变）。
5. 以上均**不阻塞**当前 Q 侧，可在 APP / WEB 立项时再定。

## 9. 待确认

- 是否把 §4 描述符升为 MCP 正式原语（会新增 `get_interaction`），需同步能力描述符。
- 级联「一次提交」的事务边界放在引擎侧还是 server 侧。

> 已裁定（2026-10-27，三国杀 P2/P3）：新增 `ordered` 原语（`slots` + 跳空有序 id 列表，提交 `arrange:<csv>`）；观星是该原语的首个实例，交互规格见 `docs/games/sanguosha/ui-spec.md` §7；`execute_actions` 当前**无必需场景**，推迟到 P3/按需，UI 改为「草稿 + 按 pending 逐条提交」。
> 已裁定（2026-10-19）：**保留 `many`**（虽无实例，但不做「暂时不需要就剪掉」的原语裁剪）。
