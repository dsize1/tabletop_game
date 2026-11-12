# 技能契约（skills）

> 296 个技能，按验证批次（B0–B20）分批落地；语义裁定仍以 `docs/games/sanguosha/align/B*.json` 为准，
> 本目录只补「**契约化**」部分：每个技能声明它的**交互面**（触发 / 窗口 / 可选性 / 目标 / 待办）。

## 契约模板（每个技能一段，可由 `align` 单 + 注册表生成）

```
### <code> <名称>（<kind>，批次 Bn）

- 触发：<EventKind 列表 / 主动>
- 窗口：<Window>
- 合法集：<can_active 条件 / 触发条件>（条件不成立 → 动作不可见，不得静默执行）
- 交互：<有无 pending>；出口语义：<escape / escape_mandatory / 无>
- 目标：<目标规则 / 无>
- 待办 data：<端侧要读的字段>
- 用例锚点：<tests/bX.rs::fn>（≥1）
- 对齐单：align/Bn.json#<code>（疑点裁定已 implemented）
```

## 落地方式（分批）

1. 每批（≤8 将）先由脚本从注册表 + `align/Bn.json` 生成草稿（`status: draft`）；
2. 走查时逐条确认「交互面」并补用例锚点（`status: implemented`）；
3. `check:spec-conformance` 要求：**每个注册技能都必须出现在某个批次契约文件里**，
   且 `status: implemented` 的条目必须有存在的用例锚点。

## 生成与校验（不手写）

```bash
npm run gen:spec-skills                     # 生成/刷新 B*.md + granted.md + index.json
node apps/scripts/gen-spec-skills.mjs --check   # 只校验是否最新（已并入 check:sgs2）
npm run spec:anchors -- B0,B1,B2            # 回填 align 的 questions[].test 锚点
npm run check:spec-conformance              # 覆盖 + 锚点存在性 + 注册表一致
```

数据源：Rust 注册表导出（code/name/kind/triggers/window/active）+ `align/B*.json`
（original / current / questions[].test / status）。**本目录所有文件都是生成物**，手改会被覆盖。

## 状态口径

| 状态 | 含义 | 门禁 |
|---|---|---|
| `verified` | 对齐单已裁定 + **有用例锚点**（锚点函数必须存在） | 缺锚点 → 红 |
| `implemented` | 对齐单已裁定，但锚点待回填（M10 legacy 欠账，显式计数） | 计数上报，不阻塞 |
| `draft` | 待走查 | 不校验 |

技能级锚点：没有裁定项（questions 为空）的技能用 align 的 `test` 字段（由 `spec:anchors` 回填）。

## 当前进度（2026-11-12）

| 项 | 数量 |
|---|---|
| 注册技能 | 296 |
| 契约覆盖 | 296（B0–B20 + granted） |
| `verified`（B0–B2 已完成回填） | **40**（B0 7 / B1 18 / B2 15） |
| `implemented`（待回填锚点） | 256 |
| 授予技 / 无对齐单条目 | 8（`granted.md`） |
