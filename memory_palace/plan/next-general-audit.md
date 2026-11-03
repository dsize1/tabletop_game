---
session_id: next-session
date: 2026-11-03
status: 规划中（待新会话执行）
baseline_commit: 358bd4a
---

# 任务：全武将批量验证（232 将 / 294 技能）

> **给新会话的开工说明**：这是独立任务文档，不依赖某个 `plan/<sessionId>.md`。新会话可直接说「继续全武将批量验证」，从本文 §3 第 1 批开始。
> 前置：工作台 + 沙盒创建对局 + 预览/AI + 测试玩家白名单已完成（M61–M64）；当前门禁 `sgs:check` 19/19、`cargo test -p sanguosha` 928 条。

## 1. 背景与现状

- 引擎已实现 **232 位武将 / 294 个技能**，覆盖标准 → 风林火山（含 SP）→ 一将成名 → 界限突破。
- 已有**批次开发流程**（`docs/games/sanguosha/batch-playbook.md`）解决「边实现边验证」：每批 8 将、每将 ≥3 用例、`sgs:check` 收尾。但这是**开发期验证**，不等于逐将的“技能可用性/语义走查”。
- 工作台当前能力：
  - `POST /internal/game/preview/sandbox`：5–8 人、指定武将/身份、全 bot、跳过选将、随机牌堆/手牌、可选 seed/模板；
  - `POST /internal/game/preview/{id}/drive`：AI 单步/连推；
  - `POST /internal/game/preview/scene`：对快照应用结构化场景（手牌/装备/判定/阶段/轮次/牌堆顶等）；
  - Web 预览页：视角切换、AI、重置、技能详情；
  - 测试玩家白名单：可登录工作台做人工走查。
- 尚未落地（本任务要补）：
  - `docs/games/sanguosha/general-audit.md`（矩阵）
  - `apps/scripts/sgs-general-audit.mjs`（自动审计）
  - `check:sgs-generals` 门禁（抽样）
  - 分批人工验收记录与缺陷清单

## 2. 目标与边界

### 目标

1. 为 **232 将 × 294 技能**建立可追踪矩阵：技能类型、是否有定向用例、自动审计结果、人工走查结果、失败/阻塞原因。
2. 为每个武将生成**最小可复现局面**（武将/身份/手牌/装备/判定/阶段/牌堆顶/seed），自动执行技能动作并校验引擎不报错、守恒、状态变化、pending 可继续。
3. 按 **标准 → 风 → 林 → 火 → 山（含 SP）→ 一将成名 → 界限突破** 分批人工走查；每批 ≤8 将或 ≤10 技能。
4. 发现缺陷立即修复并补 `tests/p106+` 回归；修复后重跑本批 + `sgs:check` + `check:sgs-ai`。
5. 结果为后续“批量验证脚本/回归集”沉淀可复用的模板与断言。

### 不做

- 不改规则口径（除明确 bug）；规则争议写入 `rules-rulings.md` 后另行裁定。
- 不做国战 / 双将 / 暗将。
- 不做 AI 训练/自对弈强化（P6.5/P6.6）。
- 不把审计脚本做成线上路径：仅本机/中台白名单可用；不写 `game_state`/`game_events`/`game_decision_log`。

## 3. 批次顺序与范围

| 批次 | 包 | 人数/将数 | 说明 |
|---|---|---|---|
| B0 | 标准 | 25 | 先跑通审计脚本与矩阵，作为样版 |
| B1 | 风 | 8 | 含 SP 风？按 `data.rs` pack 分组 |
| B2 | 林 | 8 | |
| B3 | 火 | 8 | |
| B4 | 山 | 8 | |
| B5 | SP-1/SP-2 | 16 | 风林火山收尾（48） |
| B6–B14 | 一将成名 | 72（9 批 ×8） | 沿用批次编号 Y1–Y7 的将组 |
| B15–B25 | 界限突破 | 87（J1–J8 已实现分组，按 8 将切） | 界版技能与标准同名技能共用 code，需 `is_jiexian` 分支 |

> 实际批次以 `docs/games/sanguosha/batch-playbook.md` + `npm run sgs:batch-status` 的武将清单为准；本文只固定顺序与粒度。

## 4. 每将验证流程（人工主导 + 机器辅助）

### 4.0 审计形式（先明确边界）

**结论：批量审计不是“让 agent 判断技能对不对”，而是“人工确认期望 + 机器提供证据”。**

原因：技能描述本身存在歧义/省略/版本差异时，agent 没有独立裁判权；若让 agent 自己推断“正确行为”，只会把实现偏差重新包装成“通过”。

标准流程：

1. **文本审计（人工为主）**：逐技能对照官方文本/`skills.md`/`rules-rulings.md`/用户裁定，记录“原文 + 歧义点 + 期望行为”。
2. **期望确认（人工）**：歧义项先裁定；未裁定项状态记 `待裁定`，**不得标记通过**。
3. **场景生成（机器）**：agent 按人工确认的期望生成最小场景（武将/手牌/装备/阶段/seed）。
4. **机械校验（机器）**：合法动作存在、执行不报错、牌数守恒、pending 可继续、状态变化符合 `expected`（`expected` 由人工确认，不由 agent 推断）。
5. **人工走查（人工）**：工作台预览页逐项确认时机/目标/结算/取消/视图/日志；人工签字才算通过。
6. **修复回归（机器）**：agent 按人工判定的“实现 bug”修复并补 `tests/p106+`；规则歧义不得由 agent 自行改口径。
7. **批次签字（人工）**：每批汇总通过/失败/待裁定；agent 不得自行关闭批次。

人工走查记录的最小字段：

```
武将 / 技能 code / 原文与来源 / 歧义点或裁定 / 期望行为（人工确认）
场景(seed/模板) / 实际操作 / 期望结果 / 实际结果 / 结论(通过/失败/待裁定) / 确认人+日期
```

### 4.1 最小场景设计

对每个武将（目标座位 `T`）：

1. 5 人局（或 8 人，按技能需要）；目标武将放 `T`，身份按技能需要（主公技/忠臣/反贼等）。
2. 用 `seed` 固定随机；需要确定性摸牌/判定时用 `deck_top`。
3. 用 `SceneSpec` 设置：手牌、装备、判定区、横置、标记、阶段（`play`/`judge`/`draw`/`discard`/`end`）、回合/轮次、计数器。
4. 其他座位给“友/敌”最小配置（有手牌/无手牌/有装备/空城等）。

### 4.2 自动审计（脚本，只做机械证据）

Pseudo-flow：

```
1. create sandbox (target general + identities + seed)
2. GET raw snapshot
3. POST /preview/scene  (hand/equips/judge/phase/deck_top...)
4. POST /preview  (inline final snapshot) 或复用模板
5. GET legal actions for actor
6. 找到目标技能动作（id: skill:<code> / play:<card> / 转化技动作）
7. 执行动作；若返回 pending，按 skill scenario 选择选项（或 AI drive 一步）
8. 断言（`expected` 必须来自 §4.0 的人工确认，不能由脚本猜测）：
   - 无 EngineError / Internal / 非法动作
   - 牌数守恒
   - 预期区域/体力/标记/阶段变化（人工确认过的 expected）
   - pending 可继续（drive 不卡死）
   - 事件/日志包含技能触发
9. 输出 {general, skill, scenario, action, result, error, seed, template_id}
```

脚本建议命令：

```bash
node apps/scripts/sgs-general-audit.mjs --pack standard
node apps/scripts/sgs-general-audit.mjs --general sgs_hero_1
node apps/scripts/sgs-general-audit.mjs --batch B0 --out data/audit/standard.json
node apps/scripts/sgs-general-audit.mjs --check --sample 8   # CI 抽样
```

依赖：临时 server（参考 `apps/scripts/check-sgs-preview.mjs` 的启动方式）+ 已构建 server；不要求 QQ/Redis? auth 需要 Redis；可复用现有 skip 逻辑。

### 4.3 人工走查（预览页，唯一验收依据）

每将至少检查：

- 主动技：按钮出现（合法时）、目标/选牌、取消、无牌/无目标边界、结算文案
- 触发技：时机（准备/判定/摸牌/出牌/弃牌/结束/回合外）、是否可选、拒绝分支
- 锁定技：自动生效、不可取消、与装备/身份/横置的交互
- 限定/觉醒：每局一次、觉醒条件、觉醒后技能替换
- 主公技：主公/非主公、势力响应、拒绝
- 转化技：卡牌→牌名、目标合法性、颜色/花色限制
- 响应/选择：无懈链、濒死、多选、排序、超时/取消
- 视图：公共区不泄漏隐藏信息、个人区手牌/技能/特殊区正确、他人技能详情可查看
- 日志：历史/事件与操作一致
- 技能文本歧义是否已裁定（未裁定不得通过）

发现问题 → 记录到矩阵 → 复现模板/seed → **人工判定“实现 bug / 规则歧义 / 描述待补”** → 实现 bug 才由 agent 修代码并补 `tests/p106+`；歧义先更新 `rules-rulings.md` 或回到用户确认。

> **agent 不得自行把技能标记为“人工通过”**；矩阵的最终状态必须由人工确认人签字。

## 5. 结果记录

### 5.1 矩阵文档（新建）

`docs/games/sanguosha/general-audit.md`

建议列：

| 包 | 武将 | code | 技能 | 类型 | 实现 | 定向用例 | 原文/歧义状态 | 自动结果 | 人工结果 | 确认人 | 复现模板/seed | 备注 |
|---|---|---|---|---|---|---|---|---|---|---|

状态枚举：`未验证 / 自动通过 / 自动失败 / 待裁定 / 人工通过 / 人工失败 / 阻塞`。

> `自动通过` 只代表机械校验通过；只有 `人工通过` 才算验收完成。

### 5.2 其他记录

- 每批收尾：`docs/games/sanguosha/development-plan.md` 新增「P4.4.4 全武将验证」小节
- 里程碑：`memory_palace/milestones.md` 新增 M65+（每批一条或每阶段一条）
- 规则争议：`docs/games/sanguosha/rules-rulings.md`
- 失败清单 JSON：`data/audit/<pack>.json`（不入库，可本地保留）
- 测试用例：`crates/engines/sanguosha/src/tests/p106.rs` 起按批续号

## 6. 待确定事项（开工前确认）

1. **232 vs 149**：工作台沙盒表单/`sandbox_meta` 使用 149 去重池；全量审计要覆盖 232。
   - 方案 A：审计仅 149（与正常选将池一致，推荐先跑）；
   - 方案 B：给沙盒 API 增加 audit-only `pool:"all"`（可测原版+界版，需改引擎/接口）；
   - 方案 C：用 `/preview/scene` 注入被去重武将（可绕过池，但开局技能不会重跑）。
   - **建议**：先 A 跑通流程，再评估 B 补全 232。
2. **开启技能（左慈/陆绩/刘虞/界袁绍/界张郃/卢植）**：必须走正常开局；若用 scene 后注入武将，开局技能不会执行。审计脚本应优先用 sandbox 指定武将，避免 scene 改武将。
3. **场景元数据来源**：每个技能的最小场景是否手写 JSON（`general-audit-scenarios.json`）还是脚本按技能类型启发式生成？建议首版手写高频技能 + 其余通用场景。
4. **断言强度**：自动审计只做机械校验；`expected` 必须由人工确认。语义/歧义判定一律人工，agent 不得自行推断“正确行为”。
5. **随机技能**：判定/摸牌随机时用 seed + `deck_top`；无法完全确定的技能只断言不变量。
6. **人工验收人**：技能文本歧义与“人工通过”必须由谁签字？仅开发本人，还是测试玩家（巴萨小六/乌索布/竹笛）参与？建议：开发本人对规则负责，测试玩家做体验/边界走查。
7. **批次粒度**：是否严格 ≤8 将/批（建议是）；跨包同名界版是否合并验证（建议按包分开，避免漏界版分支）。

## 7. 待办实现

- [ ] 建 `general-audit.md` 矩阵骨架（232 将/294 技能，状态列）
- [ ] 写 `apps/scripts/sgs-general-audit.mjs`（启动临时 server/sandbox/scene/drive/断言/JSON 输出）
- [ ] 写 `check:sgs-generals` 门禁（抽样 8 将）并入 `sgs:check` 非 fast
- [ ] B0 标准 25 将：自动 + 人工走查 + 修复 + 回归
- [ ] B1–B5 风/林/火/山/SP（48 将）
- [ ] B6–B14 一将成名（72 将）
- [ ] B15–B25 界限突破（87 将）
- [ ] 全量 232 将矩阵补全；失败项清零或登记阻塞
- [ ] 文档/里程碑/门禁收尾

## 8. 验证事项

- [ ] 矩阵覆盖 232 将/294 技能，无未登记技能
- [ ] 自动审计可重复：同 seed/模板结果一致；失败可复现
- [ ] 每个失败项有复现模板/种子/动作链
- [ ] 修复项有 `tests/p106+` 回归，`cargo test -p sanguosha` + `sgs:check` 全绿
- [ ] 人工走查记录包含主动/触发/锁定/限定/觉醒/主公/转化/响应/视图/日志
- [ ] 自动审计不写 `game_state`/`game_events`/`game_decision_log`，不影响真实对局
- [ ] 测试玩家白名单账号可只访问工作台完成走查

## 9. harness 优化项

- `check:sgs-generals`：抽样 8 将自动审计（CI 非 fast），输出失败清单
- 固定 fixture：standard 25 将 × 每将 1 主动/1 触发场景
- 场景生成器：按技能类型（主动/触发/锁定/限定/觉醒/主公/转化）生成最小 hand/equip/judge/phase
- 失败隔离：单个技能 internal error 不影响整批，JSON 汇总
- 压力回归：全将 100 局 + 8 人局 20 局自对弈（终局率/非法/卡死/最长步数）
- 文档校验：矩阵行数/技能 code 与 `skills.rs` 一致（防止漏项/错字）

## 10. 相关文件

- `docs/games/sanguosha/batch-playbook.md`（批次开发/DoD）
- `docs/games/sanguosha/development-plan.md`（P4.4.2/P4.4.3/P5）
- `docs/games/sanguosha/skills.md`、`data.rs`、`skills.rs`（数据源）
- `docs/games/sanguosha/rules-rulings.md`（裁定）
- `crates/engines/sanguosha/src/tests/support.rs`（测试助手）
- `apps/scripts/check-sgs-preview.mjs`（临时 server/harness 样板）
- `memory_palace/plan/01a19ccc-dc1a-7549-aeb8-5e87e928ddde.md`（原始武将验证-1/2/3 提案）
- ADR-0033/0034（工作台/沙盒）
