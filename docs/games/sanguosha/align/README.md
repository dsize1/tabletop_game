# 技能对齐单（147 将 / B0–B20）

> 目的：把「官网技能文本的歧义」在**实现之前**变成**你确认过的可执行描述**，避免「实现与期望不一致」反复返工。
> 机器文件 = `align/<B>.json`（单一事实源）；人读文件 = `align/<B>.md`（由脚本生成，**勿手改**）。
> 门禁：`npm run check:sgs-align`（一致性 / 必填 / 正反例 / 机制引用 / 冲突阻断 / 覆盖 147 将）。

## 1. 每技能 7 段

| # | 段 | 谁填 | 说明 |
|---|---|---|---|
| 1 | 官方原文 | 脚本自动（`data/heroes.json` 按 gid+技能名） | 逐字，禁止手抄改写 |
| 2 | 现实现分步陈述 | agent | 时机 / 窗口 / 条件 / 步骤顺序 / 边界，按**代码实际行为**陈述，不美化 |
| 3 | 与原文差异 | agent | 原文没写但代码这么做的、或代码没做的，逐条列 |
| 4 | 疑点 + 候选 | agent | 编号 `Q1..Qn`，给候选 A/B/C + 倾向 + 影响面；**禁止开放式提问** |
| 5 | 机制依赖 | agent | 引用 `mechanics.json` 的机制 id（改动机制会触发跨批次冲突） |
| 6 | 动画 / UI 期望 | **用户** | 提示文案 / 浮层交互 / 动效 key / 是否逐张亮牌 / 群侧播报；不做的写「本期不做」 |
| 7 | **对齐后描述（权威）** | **用户**（口述，agent 落笔） | 严格按此实现；实现只引用条目，不复制描述 |

## 2. 状态机

```
draft（脚本生成） → discussed（agent 填完 2–5） → confirmed（你确认 + 填 7）
   → implemented（新 crate 实现 + 正反例） → verified（你抽检签字）
blocked（与已对齐批次/机制冲突，待你复核）
```

- **必须对齐后执行**：`draft/discussed` 的技能不得实现；一批内存在未对齐技能 → 整批阻断。
- 冲突命中（机制词表版本变更 / 同术语异义 / 同一时序矛盾）→ 该批 `blocked`，门禁失败直到你复核。

## 3. 你的成本模型（默认同意 + 只答疑点）

- 无疑点的技能：你只需在看板上确认「同意现实现」。
- 有疑点：每条疑点给候选与影响，按批 20–30 条一次答完；你的回答直接写进第 7 段。
- 每批结束你会收到「疑点汇总表」（本文件顶部），而不是逐技能长文。

## 4. 验收（抽检制，2026-11-06 起试跑 3 批）

1. 你确认对齐单（语义契约）；
2. 你抽检 2–3 个高风险技能，在 Web 工作台真机走查；
3. 机检覆盖全批：`check:sgs-align` + 守恒 + 视图契约 + 旧引擎对拍。
抽检记录写在本文件末尾「本批抽检」表，并回写 `general-audit.md` 人工列。

## 5. 每批 DoD

- 对齐单：该批**全部**技能 `confirmed` 以上，第 7 段非空，疑点全部有答案；
- 实现：新 crate 覆盖该批全部已对齐技能，每技能 ≥1 正例 + ≥1 反例；
- 门禁：`check:sgs-align`、`check:sgs-arch`、牌数守恒、视图契约全绿；旧引擎已有该将时对拍摘要一致；
- 你抽检签字。

## 6. 命令

```bash
node apps/scripts/sgs-align-sheet.mjs --batch B0 --new      # 生成骨架（仅当 json 不存在）
node apps/scripts/sgs-align-sheet.mjs --batch B0 --sync     # json → md
node apps/scripts/sgs-align-sheet.mjs --batch B0 --check    # 单批校验
node apps/scripts/check-sgs-align.mjs                       # 全局门禁（含 148 覆盖/冲突）
```

## 7. 为何用 JSON 而不是 YAML

本仓 Node 侧无 yaml 依赖（无根 `node_modules`），引入新依赖违背「轻量」；JSON 同样可 git diff、可机读，且零依赖。

---

## 8. 进度与「新会话开工」（2026-11-06 收口）

| 批次 | 状态 | 说明 |
|---|---|---|
| B0（3 将 / 7 技能） | **7/7 implemented** | 据守/解围/不屈/奋激/行殇/放逐/颂威 |
| B1（8 将 / 18 技能） | **18/18 implemented** | 含庸肆/奇制/进趋/怀橘/寸目/伪帝/拒战/飞军/兵略/遗礼/整论/溃诛/掣政/立军/荐降/审时/成略/恃才 |
| B2（8 将 / 15 技能） | **15/15 implemented** | 明任/贞良/镇骨/往烈/罪论/父荫/良姻/箜声/征荣/鸿举/谦节/决堰/破势/雄乱/从谏 |
| B3（5 将 / 9 技能） | **9/9 implemented（待抽检）** | 马谡/荀攸/钟会/满宠/关平；26 疑点裁定 + 实现（`sanguosha2/src/skills/b3.rs`，25 用例）。（原成员「将华雄」按用户裁定移除：与界华雄重复） |
| B4（8 将 / 11 技能） | **11/11 implemented（待抽检）** | 刘封/潘璋马忠/朱然/曹真/韩浩史涣/陈群/吴懿/周仓；30 疑点裁定 + 实现（`sanguosha2/src/skills/b4.rs`，30 用例）；新增 `chained_enter` 事件 / 阶段级手牌冻结 |
| B5（6 将 / 13 技能） | **13/13 implemented（待抽检）** | 孙鲁班/朱桓/顾雍/曹叡/曹休/钟繇；30 疑点裁定 + 实现（`sanguosha2/src/skills/b5.rs`，30 用例）；新增兴衰式濒死流程 / 「造成伤害时」钩子链 |
| B6（7 将 / 12 技能） | **12/12 implemented（待抽检）** | 刘谌/张嶷/孙休/朱治/全琮/公孙渊/郭图逢纪；28 疑点裁定 + 实现（`sanguosha2/src/skills/b6.rs`，26 用例）；新增 `reveal`（立即公共可见）/ 统一攻击范围入口 |
| B7（7 将 / 12 技能） | **12/12 implemented（待抽检）** | 李严/孙登/刘虞/岑昏/孙资刘放/黄皓/张让；28 疑点裁定 + 实现（`sanguosha2/src/skills/b7.rs`，17 用例）；新增**伤害属性字段** / **响应牌机制** / 私密展示 |
| B8（8 将 / 17 技能） | **17/17 implemented** | 辛宪英/吴苋/徐氏/曹节/嵇康/秦宓/薛综/蔡邕；拼点 / 目标数修正 / 牌堆承载使用 |
| B9（7 将 / 14 技能） | **14/14 implemented** | 界标蜀（刘备/关羽/张飞/赵云/马超/诸葛亮/黄月英）；被动转化技 / 主公技接力 / 非锁定技失效 |
| B10（8 将 / 17 技能） | **17/17 implemented** | 界标吴；额外回合 / 赋值型手牌上限 / 多目标无效 / 手牌移出 / 连营批次 |
| B11（8 将 / 14 技能） | **14/14 implemented** | 界标魏；技能判定子流程（QuickJudge）/ 改判 / 天妒 / 【闪】接力 |
| B12（4 将 / 8 技能） | **8/8 implemented** | 界标群（华佗/吕布/公孙瓒/貂蝉）；**多张响应**（`multi-response`）/ **不能被无懈**（`no-wuxie`）/ 濒死转化【桃】/ 反向距离修正 |

**新会话直接照做：**

```bash
# 1) 出下一批骨架（自动带官方原文，已清洗重复拼接噪声）
node apps/scripts/sgs-align-sheet.mjs --batch B7 --new
# 2) 填「现实现/差异/疑点」（agent 干活；每条技能 ≥3 条陈述；疑点给候选+倾向+影响）
# 3) 渲染 + 自检
node apps/scripts/sgs-align-sheet.mjs --batch B7 --check
# 4) 交用户裁定 → 落「对齐后描述」→ confirmed → 实现
```

**开工必读（按顺序）**

1. `docs/conventions/skill-spec-rules.md` —— 4 条强制规则（目标列表 / 手牌冻结 / 卡牌属性 / 转换技双窗口）
2. `docs/games/sanguosha/align/mechanics.json` —— 14 条共享机制与不变量
3. `docs/games/sanguosha/spike-report.md` —— 6 类缺陷与结构性对策
4. 本文件 §1–§7 —— 7 段格式、状态机、DoD、冲突阻断

**已知欠账（不在 B3 之前做也行，但别忘）**

- **Web 工作台仍跑旧引擎**（`sanguosha`），新 crate 尚未注册（M4，OQ3：重构完成前不开发注册/网关）。
  新 crate 的 v2 视图契约/AI 自对弈门禁已就绪（`check:sgs-view-contract` 覆盖 v1+v2、`check:sgs2-ai`）。
- 距离系统已接武器范围/坐骑（M1），「往烈首张无距离」走统一入口。
- **卡牌结算引擎已落地（M1）**，B0–B7 的「视为使用牌」已改走真实结算（M2）：
  `Game::resolve_use`（普通使用）/ `use_virtual`（纯虚拟）/ `use_as`（转化牌）。
  `Game::use_card` 仅保留为「事件模拟」兼容入口（技能不得调用）。
  仍有一处未销号：谮毁的「代用 / 额外目标」需要在结算前修改目标（内核时序缺口），见
  `memory_palace/plan/next-engine-replacement.md` §9.2 台账。整体替换 v1 的计划与风险同见该文件。
- **门禁数据源已迁到生成物**：武将/牌名录读 `docs/games/sanguosha/data/engine-catalog.json`
  （`npm run gen:sgs-engine-data` 生成并 `--check`）；脚本不再解析 v1 `data.rs`。
  注册前置清单与删除清单见 `docs/games/sanguosha/engine-replacement/`。
- 罪论 `order:` 两选项在 Web 上需要视图层支持该前缀（与 8 类 pending 一致）。
