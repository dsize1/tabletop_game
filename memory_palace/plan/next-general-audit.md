---
session_id: next-session
date: 2026-11-03
status: 执行中（2026-11-05 M69：B0 收口准备 + B1–B2 机检/走查单完成，待人工签字）
baseline_commit: c3af1ba（原 358bd4a；2026-11-05 重划批次）
---

# 任务：全武将批量验证（148 将 / 295 技能；批次 B0–B20）

> **给新会话的开工说明**：这是独立任务文档，不依赖某个 `plan/<sessionId>.md`。新会话可直接说「继续全武将批量验证」，从 §7「待办」的下一批开始。
> 前置：工作台/沙盒/预览/M64 白名单已完成；M66（2026-11-05）已落地工具链 + B0 首跑；当前门禁 `sgs:check`（含 `check:sgs-generals`）。

## 1. 背景与现状

- 引擎已实现 **232 位武将 / 295 个技能**（含 7 个授予技），覆盖标准 → 风林火山（含 SP）→ 一将成名 → 界限突破。
- **验证范围 = 148 位**（口径 A：有界版同名的原版 84 位不入批次，见 §3）；机器证据 + 人工签字两轨并行。
- **2026-11-05 名称勘误**：gid=55「张昭张弘」为官网错别字，正确为「张昭张纮」（与`界张昭张纮`同名将）。修正后按口径 A 被去重 → 原 149 降为 148，B0 由 4 将变 3 将（曹仁/周泰/曹丕）。勘误表：`apps/scripts/lib/sgs-name-fixes.mjs`；门禁 `check:sgs-general-names`（编辑距离 + 官方名录比对）。
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

## 3. 批次顺序与范围（2026-11-05 重划：148 将 / B0–B20）

**池口径（方案 A，用户 2026-11-05 裁定）**：有界版同名的原版武将一律不进验证批次——标准包 25 位、风林火山 28 位、一将成名 30 位 + 名称勘误后新增的「张昭张纮」（原「张昭张弘」），共 **84 位排除**；剩余 **148 位** = `prefer_jiexian` 后的默认池（含名称勘误修正），因此 **无需改引擎/沙盒**（原「audit-only 全池」方案作废）。

> 排除口径：`界X` 存在 ⇒ 原版 `X` 排除（`data.rs` 中 `pack != "jiexian"` 且同名界版存在）。

| 批次 | 来源（包/子包） | 将数 | 武将 | 说明 |
|---|---|---|---|---|
| B0 | 风林火山·风/林/山（保留） | 3 | 曹仁、周泰、曹丕 | **流程验证批**（火包保留 0 位；张昭张纮与界版同名已按口径 A 排除） |
| B1 | 风林火山·SP-1 | 8 | 袁术、王基、严颜、王平、陆绩、孙亮、蒯越蒯良、许攸 | |
| B2 | 风林火山·SP-2 | 8 | 卢植、郝昭、陈到、诸葛瞻、周妃、毌丘俭、陆抗、张绣 | |
| B3 | 一将成名·批次1/2/Y1 保留 | 6 | 马谡、荀攸、将华雄、钟会、满宠、关平 | |
| B4 | 一将成名·Y2+Y3 保留 | 8 | 刘封、潘璋马忠、朱然、曹真、韩浩史涣、陈群、吴懿、周仓 | |
| B5 | 一将成名·Y4 保留 | 6 | 孙鲁班、朱桓、顾雍、曹叡、曹休、钟繇 | |
| B6 | 一将成名·Y5 保留 | 7 | 刘谌、张嶷、孙休、朱治、全琮、公孙渊、郭图逢纪 | |
| B7 | 一将成名·Y6 保留 | 7 | 李严、孙登、刘虞、岑昏、孙资刘放、黄皓、张让 | |
| B8 | 一将成名·Y7 保留 | 8 | 辛宪英、吴苋、徐氏、曹节、嵇康、秦宓、薛综、蔡邕 | |
| B9 | 界限突破·界标·蜀 | 7 | 界刘备、界关羽、界张飞、界赵云、界马超、界诸葛亮、界黄月英 | |
| B10 | 界限突破·界标·吴 | 8 | 界甘宁、界吕蒙、界黄盖、界周瑜、界大乔、界陆逊、界孙权、界孙尚香 | |
| B11 | 界限突破·界标·魏 | 8 | 界曹操、界司马懿、界夏侯惇、界张辽、界许褚、界郭嘉、界李典、界甄姬 | |
| B12 | 界限突破·界标·群 | 4 | 界华佗、界吕布、界公孙瓒、界貂蝉 | |
| B13 | 界限突破·J1 | 8 | 界黄忠、界徐庶、界华雄、界左慈、界张角、界于吉、界袁绍、界刘禅 | |
| B14 | 界限突破·J2 | 8 | 界孙策、界夏侯渊、界卧龙诸葛亮、界庞统、界魏延、界小乔、界孙坚、界庞德 | |
| B15 | 界限突破·J3 | 8 | 界太史慈、界董卓、界邓艾、界祝融、界姜维、界贾诩、界鲁肃、界张郃 | |
| B16 | 界限突破·J4 | 8 | 界孟获、界徐晃、界蔡文姬、界荀彧、界典韦、界颜良文丑、界张昭张纮、界徐盛 | |
| B17 | 界限突破·J5 | 8 | 界凌统、界陈宫、界曹彰、界简雍、界虞翻、界高顺、界法正、界程普 | |
| B18 | 界限突破·J6 | 8 | 界王异、界张春华、界刘表、界蔡夫人、界曹冲、界曹植、界吴国太、界廖化 | |
| B19 | 界限突破·J7 | 8 | 界李儒、界郭淮、界伏皇后、界韩当、界关兴张苞、界马岱、界郭皇后、界沮授 | |
| B20 | 界限突破·J8 | 4 | 界夏侯氏、界张松、界于禁、界步练师 | 收官 |

合计 **148 将 / 21 批**（3+8+8+6+8+6+7+7+8 = 61 → 风林火山 19 + 一将成名 42；界标 27 + J1–J8 60 = 87）。

> 批次表单一事实源：`apps/scripts/lib/sgs-batches.mjs`（矩阵、审计脚本、门禁、`sgs:batch-status` 交叉校验均读它，防漂移）。
> 验证批次 ≠ 开发批次：开发批次见 `docs/games/sanguosha/batch-playbook.md` + `npm run sgs:batch-status`（含已排除的原版武将）。

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

### 5.1 矩阵文档（已生成，2026-11-05）

`docs/games/sanguosha/general-audit.md`（148 行 / 21 批；骨架由 `npm run gen:sgs-audit-matrix` 生成，**已存在则不覆写**，人工列只能人工改）（原建议列见下，实际列见文件头）

建议列：

| 包 | 武将 | code | 技能 | 类型 | 实现 | 定向用例 | 原文/歧义状态 | 自动结果 | 人工结果 | 确认人 | 复现模板/seed | 备注 |
|---|---|---|---|---|---|---|---|---|---|---|

状态枚举：`未验证 / 自动通过 / 自动失败 / 待裁定 / 人工通过 / 人工失败 / 阻塞`。

> `自动通过` 只代表机械校验通过；只有 `人工通过` 才算验收完成。
> 机器证据另存 `docs/games/sanguosha/general-audit-evidence.md`（生成、勿手改）+ `data/audit/<batch>.json`。

### 5.2 其他记录

- 每批收尾：`docs/games/sanguosha/development-plan.md` 新增「P4.4.4 全武将验证」小节
- 里程碑：`memory_palace/milestones.md` 新增 M65+（每批一条或每阶段一条）
- 规则争议：`docs/games/sanguosha/rules-rulings.md`
- 失败/证据 JSON：`data/audit/<batch>.json`（不入库，可本地保留）
- 矩阵↔名录一致性门禁：`npm run check:sgs-generals:matrix`（148 行 / 批次归属 / 技能 code 齐全）
- 测试用例：`crates/engines/sanguosha/src/tests/p106.rs` 起按批续号

## 6. 待确定事项（开工前确认）

1. **232 vs 149（已裁定 2026-11-05 = 方案 A）**：有界版同名的原版（83 位）不进批次，保留 149 位 = 默认 149 池，**不改引擎/沙盒**。B0「标准 25 将」作废（原标将全在排除名单，火包保留 0 位），改为 B0 风林山保留 4 将做流程验证。
   - 原备选：B 方案给沙盒加 audit-only `pool:"all"`（作废）；C 方案 scene 注入（作废）。
2. **开启技能（左慈/陆绩/刘虞/界袁绍/界张郃/卢植）**：必须走正常开局；若用 scene 后注入武将，开局技能不会执行。审计脚本应优先用 sandbox 指定武将，避免 scene 改武将。
3. **场景元数据来源**：每个技能的最小场景是否手写 JSON（`general-audit-scenarios.json`）还是脚本按技能类型启发式生成？建议首版手写高频技能 + 其余通用场景。
4. **断言强度**：自动审计只做机械校验；`expected` 必须由人工确认。语义/歧义判定一律人工，agent 不得自行推断“正确行为”。
5. **随机技能**：判定/摸牌随机时用 seed + `deck_top`；无法完全确定的技能只断言不变量。
6. **人工验收人**：技能文本歧义与“人工通过”必须由谁签字？仅开发本人，还是测试玩家（巴萨小六/乌索布/竹笛）参与？建议：开发本人对规则负责，测试玩家做体验/边界走查。
7. **批次粒度**：是否严格 ≤8 将/批（建议是）；跨包同名界版是否合并验证（建议按包分开，避免漏界版分支）。

## 7. 待办实现

### 已完成（M66，2026-11-05）

- [x] 批次重划：148 将 / B0–B20（`apps/scripts/lib/sgs-batches.mjs`，自检无重无漏）
- [x] 名录解析 `apps/scripts/lib/sgs-catalog.mjs`（232 将 / 295 技能 / 149 审计 / 83 排除）
- [x] 审计脚本 `apps/scripts/sgs-general-audit.mjs`（临时 server + 预览会话 L1 走查 + 机械断言 + JSON 证据）
- [x] 矩阵骨架 `docs/games/sanguosha/general-audit.md`（148 行，人工列留空）
- [x] 生成证据 `docs/games/sanguosha/general-audit-evidence.md`
- [x] L2 场景台账 `docs/games/sanguosha/general-audit-scenarios.json`（B0，`expected` 待人工）
- [x] `check:sgs-generals`（矩阵一致性 + 抽样 8 将）并入 `sgs:check` 非 fast；fast 增矩阵一致性步骤
- [x] B0（曹仁/周泰/曹丕/张昭张弘）L1 首跑：触达 2 / 未触达 2 / 失败 0；不变量全 ✓、repeat=2 可复现、真相源未写
- [x] 文档收尾：development-plan P4.4.4 / milestones M66 / AGENTS 索引 / batch-playbook 交叉引用

### 已完成（M69，2026-11-05：B0 收口准备 + B1–B2 机检）

- [x] 走查单生成器 `apps/scripts/sgs-audit-sheet.mjs`（`npm run sheet:sgs-audit B0`）：把 `skills.md` 原文 + 技能类型 + `data/audit/<batch>.json` 机检 + 按类型判定点 + 场景构造提示合成人工签字单（**不生成 expected/结论**）→ `docs/games/sanguosha/audit-sheets/<BATCH>.md`
- [x] **开局 pending 丢失缺陷（工具链）**：`POST /internal/game/preview` 带 `scene` 注入会丢弃开局技能压入的 pending（卢植·明任/陆绩·怀橘 走查必卡）。新增审计开关 `--no-scene`（`npm run audit:sgs-generals:no-scene B2`）直接从沙盒开局快照走；实测 卢植 → 触达 `mingren,zhenliang`（场景注入路径为 0 触达）
- [x] B0 机检复跑（repeat=2）：3 将 / 7 技能 → 触达 2 · 未触达 1（曹丕 3 技能均为触发/主公）· 失败 0 · 真相源未写 ✓
- [x] B1 机检（repeat=2）：8 将 / 18 技能 → 触达 6（袁术`yongsi`/王基`jinzou`/陆绩`yili`/孙亮`chezheng,kuizhu`/蒯越蒯良`shenshi`/许攸`chenglue`）· 未触达 2（严颜、王平）· 失败 0
- [x] B2 机检（repeat=2）：8 将 / 15 技能 → 触达 6（郝昭`zhengu`/陈到`wanglie`/诸葛瞻`zuilun`/周妃`kongsheng`/陆抗`jueyan`/张绣`xiongluan`）· 未触达 2（卢植、毌丘俭）· 失败 0
- [x] `--no-scene` 补测（保留开局 pending，逐批落 `data/audit/<batch>-noscene.json`）：B0 触达 2/7、B1 触达 8/18（严颜`juzhan` 新增）、B2 触达 7/15（卢植`mingren,zhenliang` 新增）；仍不触达的 王平/毌丘俭 为条件不满足（飞军需他人牌数多于你、征荣需目标手牌 ≥ 你且需出杀）
- [x] L2 台账补齐：B2 新增 15 条场景骨架（B1 已有 18 条）；清理名称勘误后残留的 张昭张弘（`zhijian`/`guzheng`）2 条；台账 40 条 / `expected` 仍全为 null
- [x] 走查单 B0/B1/B2 生成（`docs/games/sanguosha/audit-sheets/`）：单内同时展示 `scene` 与 `no-scene` 两路机检，开局技单独标注

### 待办（后续会话，按批次推进）

- [ ] **B0 人工签字收口**（3 将 7 技能：曹仁 据守/解围、周泰 不屈/奋激、曹丕 行殇/放逐/颂威）→ 签字后写回矩阵人工列
- [ ] B1 人工走查（8 将 18 技能；严颜·拒战 / 王平·飞军+兵略 需按场景提示构造）
- [ ] B3–B8 一将成名（42 将）
- [ ] B9–B12 界标（27 将）
- [ ] B13–B20 界限突破 J1–J8（60 将）
- [ ] 148 将矩阵人工结果补全；失败项清零或登记阻塞（B1/B2 按批次修复 + 补回归）
- [ ] 补 L2 定向场景的 `expected`（场景骨架已由 `gen:sgs-audit-scenes` 生成，语义期望待人工；B0/B1/B2 已补齐骨架）
- [ ] 开发批次文档补「一将成名批次 1/2 + 界标 27 位」表行（`check:sgs-batch-consistency` info 提示；当前属早期批次）

## 8. 验证事项

- [x] 矩阵覆盖 148 将（批次范围）/ 295 技能，无未登记技能（`check:sgs-generals:matrix`）
- [x] 自动审计可重复：同 seed/批次 repeat=2 digest 一致；失败可复现
- [ ] 每个失败项有复现 batch+seed+动作链
- [x] 修复项有 `tests/p106+` 回归，`cargo test -p sanguosha` + `sgs:check` 全绿（本轮未改引擎；B0 无失败）
- [ ] 人工走查记录包含主动/触发/锁定/限定/觉醒/主公/转化/响应/视图/日志
- [x] 自动审计不写 `game_state`/`game_events`/`game_decision_log`，不影响真实对局（每次运行自检行数不变）
- [x] 测试玩家白名单账号可只访问工作台完成走查（M64 已落）

## 9. harness 优化项

- [x] `check:sgs-generals`：矩阵一致性（fast）+ 抽样 8 将机械审计（非 fast），失败输出清单
- [x] 失败隔离：单将异常不影响整批；JSON 汇总 + 退出码分类（触达/未触达≠失败）
- [x] 名录/批次单一事实源：`lib/sgs-catalog.mjs` + `lib/sgs-batches.mjs`（矩阵/审计/门禁共用）
- [x] 确定性：`--repeat 2` 逐将比对 digest（同 seed/场景）
- [x] 真相源隔离：临时 server + 临时 SQLite，前后 `game_state`/`game_events`/`game_decision_log` 行数自检
- [x] 场景生成器：`lib/sgs-scenes.mjs` + `--with-scenes/--scenes-only`（按 kind 预填 hand/equip/phase；`expected` 恒为 null 待人工）
- [x] 触达率报表：证据 md 增加「批次/类型」触达率表 + JSON `reach`
- [x] 审计结果 diff：`--diff <prev.json>`（状态翻转/触达增减）
- [x] 全量压力：`--all`（148 将 L1，约 66s）；自对弈 100 局由 `check:sgs-ai:100` 覆盖（100/100）
- [x] `check:sgs-decision-log` 自举（临时 server + MCP 现打一局，脱离开发库历史）
- [x] 验证批次 ↔ 去重池 ↔ 开发批次交叉校验：`check:sgs-batch-consistency`
- [x] `test:sgs-bot` 稳健化（弃牌一次凑齐；机器人推进以「回到真人回合」为证据）
- [x] 走查单生成器 `sgs-audit-sheet.mjs`：原文 + 机检 + 判定点 + 场景提示一页化，省去人工翻 skills.md/evidence（结论/确认人列留空）
- [x] 开局技机检：`--no-scene`（保留开局 pending），覆盖明任/怀橘/宗祚/血裔/巧变/化身 6 个开局技
- [ ] 走查单与矩阵联动：签字后回写矩阵 + 批次状态（当前靠人工复制，可后续做 `--apply-sign`）

## 10. 相关文件

- `docs/games/sanguosha/batch-playbook.md`（批次开发/DoD）
- `docs/games/sanguosha/development-plan.md`（P4.4.2/P4.4.3/P4.4.4/P5）
- `docs/games/sanguosha/general-audit.md`（人工矩阵 148 行）、`general-audit-evidence.md`（生成）、`general-audit-scenarios.json`（L2 台账）
- `apps/scripts/sgs-general-audit.mjs`（审计）、`apps/scripts/lib/sgs-catalog.mjs` / `sgs-batches.mjs`（名录/批次单一事实源）
- `apps/scripts/sgs-audit-sheet.mjs`（批次走查单）、`docs/games/sanguosha/audit-sheets/`（生成物，B0–B2 已出）
- `docs/games/sanguosha/skills.md`、`data.rs`、`skills.rs`（数据源）
- `docs/games/sanguosha/rules-rulings.md`（裁定）
- `crates/engines/sanguosha/src/tests/support.rs`（测试助手）
- `apps/scripts/check-sgs-preview.mjs`（临时 server/harness 样板）
- `memory_palace/plan/01a19ccc-dc1a-7549-aeb8-5e87e928ddde.md`（原始武将验证-1/2/3 提案）
- ADR-0033/0034（工作台/沙盒）
