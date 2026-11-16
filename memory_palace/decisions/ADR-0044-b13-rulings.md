# ADR-0044：B13 口径裁定（化身池 / 蛊惑影响链 / 缠怨失效）

- **日期**：2026-11-16
- **状态**：**已裁定（用户）**
- **背景**：V3 技能**按批接入**，与 v2「147/147 全部实现」的事实不同：
  `skills::all()`（已注册）⊄ `ENABLED_SKILLS`（已收口）。B13（界左慈/界于吉）开工前必须定：
  ① 化身池的「已实现」判据；② 池内武将/技能的可选边界；③ 蛊惑的影响链；④ 缠怨的失效范围。
- **关联**：`docs/games/sanguosha/align/mechanics.json`（`general-pool` / `hidden-declaration`）、
  `align/B13.json.v3_rulings`、`docs/games/sanguosha/align/B13.md`。

## 裁定（用户逐条确认，2026-11-16）

| # | 议题 | 裁定 | 落地要点 |
|---|---|---|---|
| Q1 | 化身池「已实现」判据 | **已注册**（`skills::all()`），不要求已收口 | 化身/新生以**授予技形式**（`skills::grant`）实现，不依赖技能具体实现；未收口技能被授予也不会触发（`enabled()` 闸门），由 `check:sgs3-registry` 保证「注册 ⇒ 收口」 |
| Q2 | 可化身技能与池边界 | 可授予技能**排除锁定技 / 觉醒技 / 主公技**（实现另排除授予技）；无任何可授予技能的武将被**过滤出池**；不能化身**同场已存在**武将，也不能获得同场已存在技能；不能重复持有同一张化身牌 | `skills::huashen_skills_of` / `huashen_generals` / `huashen_choices` 单出口 |
| Q3 | — | 同 Q2 | — |
| Q4 | 池的动态性 | **动态**（按当前构建计算） | 回放只读快照 `Game.huashen`，不重算池 |
| Q5 | 蛊惑声明范围 / 影响链 | 可声明【无懈可击】；影响链须支持【杀】【闪】【桃】（濒死救援）【酒】（濒死自救）【无懈可击】 | 蛊惑作为【无懈可击】时**不再开无懈窗口**（防递归） |
| Q6 | 缠怨失效范围 | **所有技能**失效（含锁定技；装备特效按 `O3=A` 也算技能） | 独立闸门 `chanyuan.{seat}`，**不复用**回合制 `suppress.*`；持续整局、不可移除 |

## 已落地（本 ADR 之后）

- `kernel/mod.rs::init_huashen` 重写为 `skills::huashen_generals`（Live ∩ 可授予技能 ∩ 非同场 ∩ 非自己 ∩ 非已持有）；
- `kernel/skills.rs` 新增 `huashen_skills_of` / `huashen_generals` / `huashen_choices`（**单一事实源**，禁止技能里各写过滤）；
- `Huashen` 增加 `active_skill`（授予技形式记录），`Game::huashen_set_current` 负责势力/性别覆盖与恢复（恢复读 `Seat.general`，不存第二份事实）；
- `tests/kernel_mechanics.rs`：5 条用例（池可授予性 / 排除锁定·觉醒·主公·授予技 / 同场与自身排除 / 已拥有技能排除 / 势力性别覆盖与恢复）。

## 未 bump 机制词表 version（说明）

`general-pool` 的不变量**新增**了上述约束，但它的唯一消费者（化身/新生）尚未实现；
bump `version` 会让**全部 290 条**已确认技能记录 `< version` 而被判冲突（`check:sgs-align`），
对已冻结的 v2 记录毫无意义。故：`version` 保持 2，约束写进 `invariants` 并在 B13 两条技能的
`mechanics_version` 记为 2；若将来要与 v2 语义区分，再单独 bump 并只对活批次置 `blocked`。

## 未决（B13 开工时再定）

- 蛊惑「扣置牌」在视图层的呈现（现有 `hidden-declaration` 契约够用，端侧浮层形态待 B13 端侧接入）；
- 「缠怨」在视图的可见性（是否显示为标记；建议进 `marks_count`，B13 UI 段再定）。
