# 合法动作集契约（legality）

## 1. 单一事实源

**`legal_actions` 与 `execute` 必须同源**：前者枚举的每个动作，后者必须能执行且结果一致；
后者拒绝的动作，前者不得枚举。

- 手牌动作：`card_play::legal_uses`（`use:{id}[:{targets}]` / `recast:{id}`），逐条用 `plan_use` 复核；
- 技能主动技：`skills::legal_actions`（`skill:{code}`），条件由各技能 `can_active` 给出；
- 待办选项：`Pending::options`（强制选择类过滤占位出口）。

## 2. 空集语义（I1 的另一面）

「合法集为空」是**唯一**允许引擎不问 / 不枚举的理由。禁止把「条件判定」当作跳过理由：

| 反例 | 正确形态 |
|---|---|
| `if !has_peach { 跳过濒死询问 }` | 手牌非空就必须问（选项只有「放弃」） |
| `if hand.is_empty() { 不询问闪 }` | ✔ 合法集为空（无牌可打） |
| `if 不能响应 { 跳过 }` | ✔ 合法集为空（规则禁止）——但要与「没牌」区分开，写在 `forbidden` 分支 |

## 3. 条件 ≠ 跳过

发动条件（拼点双方有牌、目标有手牌、激将有同势力角色…）应表达为**合法集的有无**，
而不是「先判定再默默不执行」：条件不成立 → 动作不出现；条件成立 → 必须出现。

## 4. 门禁

- `contracts/legality.json`：`src/skills/**` 禁止调用 `Game::use_card(`（事件模拟入口）；
- 用例：`tests/m2_wiring.rs`（legal ≡ execute 门禁）、`tests/plugin_contract.rs`。
