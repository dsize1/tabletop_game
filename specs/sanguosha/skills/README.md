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

## 当前状态

- 机器可校验的注册表快照：`../contracts/skills.json`（296 条 code + kind，与源码逐条比对）；
- 批次契约文件：待生成（`npm run gen:spec-skills`，C3 第二切片）。
