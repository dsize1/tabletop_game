# 任务：引擎 v2 卡牌结算 + 整体替换 v1（M1–M4）

> 立项日期：2026-11-06。状态：**M1–M3 已完成；M4 对拍/门禁数据源/注册清单已完成；注册/网关/Web/删 v1 按 OQ3 后移**（见 §8–§11）。
> 用户裁定：OQ1 暂停 B8–B20 对齐；OQ2 源 JSON + 生成 Rust 常量（见 §4.3）；OQ3 网关默认关且**重构前不开发**；OQ4 采用 v1 已知缺陷白名单；OQ5 仅 M2 / M4 各签一次。
> 结论先说：历史上定的就是**整体替换**（不是只替换武将技能），证据见 §1。

## 1. 「整体替换」是否是原定口径？

**是。** 依据（均为仓内既有裁定，不需重新讨论）：

| 出处 | 原文 | 含义 |
|---|---|---|
| `docs/games/sanguosha/legacy-retire.json` | `"decision": "新 crate 完成 148 将迁移并通过用户验收后删除旧 crate"`、`replacement: crates/engines/sanguosha2` | 终局是**删旧 crate** |
| `docs/games/sanguosha/spike-report.md` §4「尚未覆盖（**迁移时必须补**）」 | 1. **卡牌效果本体（杀/闪/桃/锦囊结算、濒死、判定、无懈链）**……2. 视图/引擎协议 3. AI 接口 4. 迁移对拍工具 5. 机制（化身/蛊惑/国战） | 牌结算属于**迁移前置**，不是可选项 |
| `spike-report.md` §5「GO 的三条附加条件」 | ②**B0 实现前补齐内核缺口**：牌/锦囊结算、濒死、判定、视图、AI 接口 | 已有偏差：B0–B7 实际只做了技能层（用 `use_virtual` 打桩） |
| `memory_palace/decisions/ADR-0038` §影响 | 旧 crate 退役计划（B20 验收 + 30 天 / 硬上限 2027-01-31） | 与上一致 |
| `docs/games/sanguosha/align/README.md` §8「已知欠账」 | Web 工作台跑的是旧引擎，新 crate 未注册 | 替换还包含**注册 + 视图适配** |

**因此任务范围** = 卡牌结算引擎 + B0–B7 接线复查 + 视图/协议 + AI 最小接口 + 注册/对拍/退役，而不是「只补技能」。

## 2. 现状盘点（量化）

| 项 | 旧引擎 v1 | 新内核 v2 |
|---|---|---|
| 源码规模 | 55,176 行（`crates/engines/sanguosha/src`） | ~8.9k 行（含 220 条用例） |
| 卡牌 | 160 张真实牌堆（`data.rs` 生成）+ 装备/锦囊/判定/无懈完整循环 | **M1 已补齐**：`cards_generated.rs` 160 张 + 44 条定义（生成物）；`cards.rs` 只读属性表 |
| 牌结算 | `play_card` / `start_sha_flags` / `start_trick_code`（逐牌型分支） | **M1 已补齐**：`kernel/card_play.rs`（`resolve_use` + `FlowKind::Card`/`Judge`）；`use_card` 保留为打桩入口（M2 迁移） |
| 伤害/濒死/判定 | 完整（但 6 条延迟队列 + 散装"禁止使用"） | `flow::damage*` / `dying` / `judge_*`（单一出口，已 220 用例覆盖） |
| 技能 | 232 将 / 294 技能（含缺陷） | B0–B7 共 **76 技能** implemented |
| 视图/AI | `view.rs` + `ai/` | **M3 已补齐**：`view.rs`（三分区 9 类区投影）+ `turn.rs`（回合驱动）+ 手牌动作枚举 + `examples/sgs2-selfplay`（AI 最小接口） |
| 注册 | `game-engine-mcp` 已注册 | **未注册**（M4，按 OQ3 后移） |
| 门禁 | `view-contract` / `sgs-ai` / `replay` 全部指向 v1 | `cargo test -p sanguosha2` 297 条 + 卡牌门禁（禁 hp 直改 / 禁牌名判定 / 每牌型必须分派） |

## 3. 里程碑（每个都是可验收的切片）

### M1 卡牌结算内核（最高风险，先做）—— ✅ 已完成（2026-11-06，见 §8）
1. **牌定义数据源**：用 `gen:sgs-engine-data` 从 v1 `data.rs` 生成 v2 的 160 张牌堆 + 装备/锦囊属性（单一 JSON，v2 只读它；以后 v1 删除也不丢数据）。
2. **使用→目标→响应→结算 单一流程**：`use_card` 变成真实结算入口（`play_card` 等价物），保留 `use_virtual` 兼容已实现技能；牌进入 `processing` → 结算 → 弃置/装备/判定区。
3. **基本牌**：杀（含火杀/雷杀属性）、闪、桃、酒（含酒加成标记）、濒死救援接入。
4. **装备**：武器范围/防具/坐骑/宝物接入统一数值链（顺序：技能加成 → 藤甲 → **白银狮子最后**）。
5. **锦囊**：普通锦囊 + 延时锦囊（判定区 LIFO）+ 无懈链（`ResponseKind::Wuxie`）。
6. **DoD**：新增 ≥60 条牌结算用例（每牌型 ≥1 正例 + ≥1 反例）；守恒 160 / 无帧遗留 / 单一延迟队列门禁全绿；单动作耗时基线（对比 v1 `--all` 66s 量级）。

### M2 B0–B7 接线复查（把打桩换成真结算）—— ✅ 已完成（2026-11-06，见 §9，待签字）
1. 逐条消掉对齐单里的「v2 无结算引擎 → 打桩」收口项（B3 奇策、B4 忠勇/勇略/司敌/陷嗣、B5 谮毁、B6 战绝/勤王/怃戎、B7 滔乱/止戈/督粮…）。
2. 每技能保留既有反例，并补「真实结算后仍不误触发」的断言。
3. 回写对齐单第 2 段（现实现）为**新引擎**行为，`impl` 锚点更新。
4. **DoD**：B0–B7 的 76 技能全部指向真实结算；`check:sgs-align` 无冲突。

### M3 视图 / 协议 + AI 最小接口 —— ✅ 已完成（2026-11-06，见 §10）
1. `public_state` / `player_view`：9 类区 + `revealed` / `private_reveals` 投影 + pending 结构（复用现有 `presentation`）。
2. AI：legal 枚举（已有）+ 观测流最小版 + 决策日志（复用 `check:sgs-decision-log` 口径）。
3. `check:sgs-view-contract` 改为可对 v2 运行。
4. **DoD**：视图契约门禁对 v2 全绿；AI 能与自己打满 10 局无 illegal/loop（沿用 `check:sgs-ai` 口径）。

### M4 对拍 / 门禁数据源 / 退役（注册/网关按 OQ3 后移，不在本轮）—— ✅ 本轮范围完成（2026-11-06，见 §11）
1. `game-engine-mcp` 注册 v2 与网关接入：**重构完成前不开发**（OQ3）；本轮只产出「注册所需接口清单」与 `sanguosha2` 的 `EnginePlugin` 完备性检查。
2. 对拍工具：v1 replay → v2 逐动作摘要比对；**建立 v1 已知缺陷白名单**（否则把缺陷当回归）。
3. `check:sgs-*` 数据源从 v1 `data.rs` 迁到新 JSON（否则删 v1 会断门禁）。
4. Web 工作台切 v2（先只读/沙盒），人工走查 B0–B7 抽检单。
5. 删旧 crate（需用户二次确认，`legacy-retire.json` 规则不变）。
6. **DoD**：`npm run sgs:check` 全绿且全部指向 v2；v1 删除后仓内无悬挂引用（`check:sgs-retire` OK）。

## 4. 风险与干扰项（呈报）

### 4.1 高风险
| # | 风险 | 影响 | 对策 |
|---|---|---|---|
| R1 | **用例语义被改写**：B0–B7 的 220 条用例大量依赖「`use_card` 只发事件 + 打桩」的现状（如调用后立刻断言弃牌数） | 改 `use_card` 语义会一次打红几十条用例，容易误判为回归 | M1 先把 `use_card` 保持兼容，新增 `resolve_use` 真入口；用例迁移分批（按批开关） |
| R2 | **两条伤害/濒死路径**（v1 的 `apply_damage` + v2 `flow::damage`）若并存 → 重演 D 类缺陷 | 同一规则两处各写一半 | 内核只允许 `flow::*` 单出口；牌结算不得直接改 hp（门禁：静态扫描 `hp -=`） |
| R3 | **对拍基线不可信**：v1 有硬编码牌名（恃勇/界酒池/AI）、6 条私有延迟队列 | 对拍差异真假混杂，无法作为验收 | 先写「v1 已知缺陷白名单」（本次已在 spike-report §12.2 记录 3 处），对拍只对白名单外差异报警 |
| R4 | **门禁数据源耦合 v1**：`sgs-catalog.mjs` / counts / batch-consistency 都在解析 v1 `data.rs` | 删 v1 当天所有门禁失灵 | M4 前置：把名录/牌表生成到 `docs/.../data/*.json`，门禁改读 JSON |
| R5 | **注册即对外**：v2 注册后 QQ 群/Web 直接换引擎，但视图协议尚未对齐 | 玩家可见回归（牌面不显示、pending 卡死） | 灰度：网关开关默认关 + 白名单房间；M3 完成前不切默认 |

### 4.2 中风险 / 干扰项
| # | 干扰项 | 说明 | 建议 |
|---|---|---|---|
| D1 | **B8–B20 对齐（还剩 95 将）** 与引擎替换并行 | 对齐单第 2 段写的是「旧引擎现实现」；引擎一换，描述立刻过期 | **串行**：先 M1→M2（引擎+接线），B8 之后按新引擎写「现实现」；或 B8 期间只写「语义契约」不写代码行为 |
| D2 | **12 处打桩收口项**散落在 B3–B7 对齐单 note/第 7 段 | 换引擎后若不逐条回写，对齐单与实现脱节（正是 ADR-0038 要治的病） | M2 建立「打桩清单」逐条销号，销号必须有对应用例 |
| D3 | 对齐单「展示/私密展示/冻结/响应牌」等新机制刚落地 | 引擎改造容易把 `revealed`/`private_reveals`/`frozen_phase` 覆盖掉 | M1 把这三类状态纳入「牌区/状态不变量」测试（守恒、无残留） |
| D4 | 性能：v2 每次动作全量 serde `Game`（160 牌 + 全员技能） | 大牌堆 + 结算链可能变慢，影响 QQ 侧超时 | M1 建立 benchmark（单动作 p95），M4 再优化（如差分序列化） |
| D5 | 时间预算：v1 55k 行 → 需要重写的是「牌 + 视图 + AI」，不是全部 | 若按「1:1 重写」会拖到退役死线（2027-01-31）之后 | 只重写**行为**，不重写数据：牌表/武将表用生成器；技能按批迁移（B0–B7 已完成 76 个） |
| D6 | 4 个并行入口（Web/AI/replay/scenes）各自有脚本基线 | 切换引擎时脚本目标不同步 | M4 一次性把 `apps/scripts/*sgs*` 的目标引擎做成参数，默认 v2 |

### 4.3 开放项裁定结果（2026-11-06 用户裁定）

| 编号 | 裁定 | 对计划的影响 |
|---|---|---|
| OQ1 | **M1–M2 期间暂停 B8–B20 对齐** | B8 之后再按新引擎写「现实现」；本轮只做引擎 |
| OQ2 | **源 = JSON，v2 用生成的 Rust 常量**（见下「Q2 答疑」） | 与 v1 现状一致：`data/cards.json` 是唯一置信源，生成物进 v2 |
| OQ3 | **网关开关默认关**；且**重构完成前不开发注册/网关** | M4 的「注册 + 网关」整体后移，本轮只留接口约定 |
| OQ4 | **采用 v1 已知缺陷白名单**（我列、你确认） | 对拍工具先输出白名单清单供你确认 |
| OQ5 | **仅 M2 / M4 各签一次** | M1、M3 由自动化门禁验收，不打断你 |

#### OQ2 答疑：Rust 常量的置信源与差异

- **置信源（唯一）**：`docs/games/sanguosha/data/cards.json`（官方牌表抓取，108 标准 + 52 军争 = 160 张）与 `data/heroes.json`（武将/技能原文）。
- **Rust 常量是生成物**：`npm run gen:sgs-engine-data` 把两张 JSON → `crates/engines/sanguosha/src/data.rs`（`CardDef` / `PhysicalCard` / `GeneralDef`）；`--check` 校验「仓库文件与 JSON 一致」。**JSON 改了而 data.rs 没重新生成 → 门禁失败**。
- **v2 现状的差异**：`crates/engines/sanguosha2/src/cards.rs` 是**手写**的 20 张测试牌 + 名字 if/else 属性表（`def(name)`），不是生成物，也没有真实牌堆。
- **裁定落地方式**：JSON 仍是唯一源；生成器增加一个输出目标 `crates/engines/sanguosha2/src/cards_generated.rs`（常量表：160 张牌 + 属性 + 伤害属性 + 响应牌标签），v2 只读生成物。**不运行时读 JSON**（保持零运行时依赖 + 编译期校验），`gen --check` 同时校验 v1/v2 两个产物。这样「JSON vs Rust 常量」不是二选一：**JSON 是源，Rust 常量是产物**。

## 5. 门禁与度量（新增/改造）

| 门禁 | 现状 | 目标 |
|---|---|---|
| `cargo test -p sanguosha2` | 297 条（M1 +75 牌结算） | M2 全批接线后不回退 |
| `check:sgs2:metrics` | 技能 LOC 比值 | 增加「牌结算 LOC / 牌型数」 |
| `check:sgs-counts` / `batch-consistency` | 解析 v1 `data.rs` | 改读 `data/*.json`（M4） |
| `check:sgs-align` | 已覆盖 52 将 | 冲突 0；打桩清单销号（13/14，谮毁见 §9.2） |
| `check:sgs-parity` | — | v1↔v2 数据/属性/技能覆盖（白名单外 0 差异） |
| `check:sgs2-plugin` | — | `EnginePlugin` 完备性 / 视图脱敏 / legal≡execute |
| `check:sgs-engine-data --check` | 校验 v1 `data.rs` | 同时校验 v2 `cards_generated.rs` + `engine-catalog.json` |
| `check:sgs-view-contract` / `check:sgs-ai` | v1 + v2 视图契约；v1 自对弈；`check:sgs2-ai` 跑 v2 自对弈 10 局 | M3 完成：`view-contract` 同时跑两引擎；`sgs2-ai` 纳入 `sgs:check:fast` |
| `check:sgs-retire` | 提醒 | 删 v1 后 OK |

## 6. 建议的执行顺序（最小可回滚）

```
M1-1 牌表生成 → M1-2 使用/目标/响应流程 → M1-3 基本牌 → M1-4 装备数值链
→ M1-5 锦囊+判定区+无懈 → M1-6 用例 60 条（可回滚点）
→ M2 接线并回写对齐单（可回滚点，你抽检）
→ M3 视图+AI（可回滚点）
→ M4 注册灰度 → 对拍 → 切门禁数据源 → 删 v1（你签字）
```

## 7. 新会话开工（照做即可）

```bash
# 0) 先读（按顺序）
#   memory_palace/plan/next-engine-replacement.md   ← 本文
#   docs/games/sanguosha/spike-report.md §4 §5      ← GO 条件与迁移前置
#   crates/engines/sanguosha2/src/kernel/{mod,flow}.rs、cards.rs
#   docs/games/sanguosha/align/mechanics.json

# 1) 生成 v2 牌表产物（M1-1 第一步）
node apps/scripts/gen-sanguosha-engine-data.mjs            # 现有：→ v1 data.rs
#   M1-1 需扩展：--out-v2 crates/engines/sanguosha2/src/cards_generated.rs
node apps/scripts/gen-sanguosha-engine-data.mjs --check    # 一致性门禁

# 2) 每完成一小步都要绿
cargo test -p sanguosha2
npm run sgs:check:fast
node apps/scripts/check-sgs2-metrics.mjs
```

**M1 的验收口径（不需要你签字）**：`cargo test -p sanguosha2` 全绿 + 新增 ≥60 条牌结算用例 + 守恒/无帧/单一延迟队列门禁 + 单动作耗时基线。
**需要你签字的只有**：M2（B0–B7 接线复查完成）、M4（删旧 crate 前）。

**纪律（写在最前面，避免重演旧缺陷）**
1. 牌结算只允许调用 `flow::damage*` / `flow::lose_hp` / `flow::chain_enter`；**禁止**在牌结算里直接改 `hp`。
2. 牌名/技能名不得进入判定：属性读 `cards::def()`，响应场景读 `ResponseKind`，伤害属性读 `damage_kind`。
3. 新增状态必须进「牌区不变量」与「状态不变量」测试（守恒 160、无帧遗留、无跨回合残留）。
4. 每个打桩销号都要有对应用例；对齐单第 2 段必须同步回写。

## 8. M1 执行记录（2026-11-06）

**结论：M1 六步全部落地，`cargo test -p sanguosha2` 全绿（新增 75 条 `tests/cards.rs` 用例）。**

| 步骤 | 落地内容 | 证据 |
|---|---|---|
| M1-1 牌表生成 | `gen-sanguosha-engine-data.mjs` 新增 `--out-v2`，产出 `sanguosha2/src/cards_generated.rs`（44 条定义 + 160 张物理牌）；`--check` 同时校验 v1/v2 | `npm run check:sgs-engine-data` |
| M1-1 数据读取 | `cards.rs` 只读生成物（`def` / `weapon_range` / `horse_bonus` / `deck()`），新增 `ArmorKind` / `WeaponKind` / `Effect`；`Slot` 由定义给出 | `tests/cards.rs::every_deck_card_has_definition` |
| M1-2 单一流程 | `kernel/card_play.rs`：`Game::resolve_use` + `FlowKind::Card`（使用→目标→响应→结算→弃置/装备/判定区）；`use_card` 保持兼容（R1） | `sha_*` / `wuzhong_*` / `guohe_*` |
| M1-3 基本牌 | 杀（火/雷属性、酒加成、属性伤害连环传导）、闪（仅响应 + 八卦阵判定）、桃、酒（每回合一次 + 下一杀 +1） | `sha_*` / `jiu_*` / `bagua_*` / `thunder_sha_propagates_through_chain` |
| M1-4 装备数值链 | 槽位替换、武器范围、坐骑距离（进攻 -1 / 防御 +1）、八卦/仁王盾/藤甲/白银狮子/青釭剑按 `ArmorKind`/`WeaponKind` 接入 `flow::damage_*`；顺序：技能加成 → 藤甲 → 白银狮子最后 | `baiyin_is_applied_after_skill_and_card_bonus` / `tengjia_fire_sha_against_tengjia_deals_two` |
| M1-5 锦囊 | 决斗 / 过河 / 顺手 / 无中生有 / 南蛮 / 万箭 / 桃园 / 五谷 / 火攻 / 借刀 / 铁索（含重铸）+ 延时（乐/兵/闪电，判定区 LIFO）+ 无懈链（奇偶层） | `duel_*` / `nanman_*` / `wuxie_chain_even_layers_restores_effect` / `judge_lifo_resolves_later_placed_first` |
| M1-6 用例/门禁 | 75 条牌结算用例（每牌型 ≥1 正例）；守恒/无帧不变量；单一延迟队列门禁更新（新增 `JudgePhase` 顺序位）；卡牌门禁（禁 hp 直改 / 禁牌名判定）；单动作耗时基线 | `tests/cards.rs::single_card_action_perf_baseline` |

**新增/变更的内核结构**

- `FlowKind::Card` / `FlowKind::Judge`（`resolve::advance` 分派），`Stage::CardAsk` / `CardPick` / `CardOpt` / `JudgeResolve`。
- `DeferredKind::JudgePhase`（顺序：技能触发之后、死亡清理之前）。
- `Game::wuxie_result`（内核子流程唯一结果通道）、`Game::judge_order`（判定区 LIFO）、
  `Game::armor/weapon/horse_bonus_of/discard_equip_slot`、`flow::heal`。
- 判定卡/装备牌不再直接改 `hp`；`flow::damage_typed` 内接入防具无效与数值链。

**已知欠账（移入 M2/M3）**

1. `use_card`（打桩）仍存在，B3–B7 的技能尚未改走 `resolve_use`（M2 逐条接线）。
2. 武器特效未做（青龙追杀 / 贯石斧 / 寒冰剑 / 雌雄 / 方天 / 麒麟弓 / 古锭刀 / 朱雀羽扇 / 诸葛连弩连击）；
   M1 只做「范围 + 青釭剑无视防具」。
3. 视图/AI/注册仍未接 v2（M3/M4）；`lib.rs` 的 `legal_actions` 尚未枚举手牌动作。
4. 对拍白名单（OQ4）尚未产出（M4-2）。

## 9. M2 执行记录（2026-11-06）

**结论：B0–B7 里所有「视为使用牌 / 转化牌」的技能已改走真实结算，技能层不再调用事件模拟入口
`Game::use_card`；`cargo test -p sanguosha2` 309 条全绿（新增 `tests/m2_wiring.rs` 12 条）。**

### 9.1 内核新增（M2）

- `card_play::UseOpts { strict, damage_source }` + 三入口：
  - `resolve_use`：普通使用（手牌 + 自己出牌阶段，严格校验）；
  - `start_virtual` / `Game::use_virtual`：纯虚拟牌（无承载牌，如「视为使用【杀】」）；
  - `start_carrier` / `Game::use_as`：转化牌（实体承载 + 生效牌名 + origins）。
- 虚拟牌伤害：`flow::damage_def`（按 `cards::def` 走防具数值链；仁王盾对无色虚拟【杀】不生效）。
- 纯虚拟【杀】计入本阶段次数（`sha_used` 兼容 `turn.sha_virtual:{seat}`）。
- 伤害来源覆盖（勤王：代出的【杀】伤害来源视为刘谌）。
- 转化牌收尾：`finish` 统一处理 origins（奇策/战绝的其余手牌一起进弃牌堆）并清虚拟定义。
- `Stage::CardTarget`：技能声明的牌选目标（滔乱：借刀 2 个 / 铁索 1–2 个）。
- `Game::use_card` 降级为**仅事件模拟**（技能门禁禁止调用）。

### 9.2 打桩清单销号（D2）

| # | 批 | 技能 | 原打桩 | 现实现 | 状态 |
|---|---|---|---|---|---|
| 1 | B3 | 奇策 | `use_card` + 手动弃 origins | `use_as`（承载 + origins） | ✅ |
| 2 | B4 | 陷嗣 | `use_virtual(杀)` 只发事件 | 真实【杀】结算 | ✅ |
| 3 | B4 | 司敌 | `use_virtual(杀)` 只发事件 | 真实【杀】结算 | ✅ |
| 4 | B4 | 勇略 | `use_virtual(杀)` 只发事件 | 真实【杀】结算（命中/闪避驱动摸牌） | ✅ |
| 5 | B4 | 忠勇 | `use_virtual(杀)` 只发事件 | 真实【杀】结算 | ✅ |
| 6 | B4 | 慎断 | `use_card` + 「等价结算完成」 | `use_as`（弃牌堆承载，非严格/无距离）| ✅ |
| 7 | B5 | 活墨 | `use_virtual(杀, vec![])` + 直接改 hp | 真实选目标 + `flow::heal` + `apply_jiu` | ✅ |
| 8 | B5 | 谮毁 | 牌本体真实结算；「代用/额外目标」只记状态 | 需在**结算前**修改目标（当前事件在结算后路由） | ⚠️ 未销号 |
| 9 | B6 | 战绝 | `use_card` + 手动弃 origins | `use_as`（先选目标再结算） | ✅ |
| 10 | B6 | 勤王 | `use_virtual(杀, vec![])`（无目标） | 代出者选【杀】+目标；伤害来源=刘谌 | ✅ |
| 11 | B6 | 怃戎 | 无打桩（reveal/flow/gain 已是真实入口） | 复查通过 | ✅ |
| 12 | B7 | 滔乱 | `use_card` + 手动弃牌 | `use_as` + `CardTarget` 选目标 | ✅ |
| 13 | B7 | 止戈 | `use_virtual(杀)` 只发事件 | 真实【杀】结算 | ✅ |
| 14 | B7 | 督粮 | 无打桩（gain/draw/置顶已是真实入口） | 复查通过 | ✅ |

**未销号 1 项：谮毁「代用 / 额外目标」**。原因：当前 `TargetChosen` 事件在牌结算**之后**才路由
（`advance` 先清栈再派事件），无法在结算前改目标；需要「结算前钩子帧」或把目标修改做成
`start_use` 的同步钩子。影响：牌本体已真实结算，但「额外目标」不会真的再吃一次牌效果。
处置：M3/M4 前补内核时序（见 §9.4 遗留 1）。销号前该技能不得标记 fully-implemented。

### 9.3 新增用例（每技能「真实结算后不误触发」）

`crates/engines/sanguosha2/tests/m2_wiring.rs`（12 条）：

- 奇策：无中生有真实摸 2；决斗真实造成 1 伤；被无懈抵消后不摸牌。
- 慎断：距离 2 的目标也进判定区（无距离限制）。
- 活墨：【杀】真实选目标并造成伤害；【酒】走统一加成（下一张杀 2 伤）。
- 战绝：决斗被无懈抵消时不造成伤害、origins 仍进弃牌堆。
- 勤王：代出【杀】命中，伤害来源日志为刘谌。
- 滔乱：【杀】要求目标出【闪】；用过的牌名不再提供。
- 门禁：`src/skills/**` 不得出现 `use_card(`；纯虚拟【杀】计入 `sha_used`。

### 9.4 已知遗留（进入 M3/M4）

1. **谮毁 代用/额外目标**（见 9.2）：需要结算前修改目标的内核钩子。
2. 武器特效仍缺（青龙追杀/贯石斧/寒冰剑/雌雄/方天/麒麟弓/古锭刀/朱雀羽扇/诸葛连弩）——M1 已登记属性，M3/M4 补行为。
3. 洛神/铁索等「无懈链与延时锦囊」的响应入口已有，但 `万箭/南蛮` 中滔乱的响应牌限制只做了「声明牌名」层（`response_context`），未覆盖全部响应窗口。
4. `check:sgs-view-contract` / `check:sgs-ai` 仍只跑 v1（M3）。

## 10. M3 执行记录（2026-11-06）

**结论：v2 已具备「可玩的回合循环 + 三分区视图 + AI 最小接口」；视图契约与 AI 自对弈门禁对 v2 全绿。**

| 交付 | 内容 | 证据 |
|---|---|---|
| 回合驱动 | `src/turn.rs`：`begin_game` / `tick`（准备→判定→摸牌→出牌→弃牌→结束）/ 弃牌阶段流程帧 / 胜负判定（主公 vs 反贼） | `tests/view_contract.rs`、`examples/sgs2-selfplay` |
| 视图 | `src/view.rs`：`public_state` / `player_view`（`public_board` + `player_area` + `special_zones`） | `tests/view_contract.rs`（9 条） |
| 9 类区投影 | Draw/Discard/Void/Processing 只给数量；Hand 只在 `player_area`；Equip/Judge/Mark 公共可见；Frame 只给数量 | `view_contract_nine_zones_are_projected` |
| 隐藏信息 | 他人手牌明细、牌堆顺序、公共 pending 的 options/data、他人身份、私密展示 | `view_contract_hand_and_identity_hidden_from_public` / `view_contract_private_reveal_only_for_viewer` |
| legal 枚举 | `card_play::legal_uses`：`use:{id}[:{t1,t2}]` + `recast:{id}`，与 `plan_use` 同源 | `legal_actions` 与 `execute_action` 共用 |
| AI | `examples/sgs2-selfplay`：仅用 `EnginePlugin` + 原始 `Game` 决策；确定性（seed）；环/卡死/非法三类统计 | `npm run check:sgs2-ai` |
| 门禁 | `check:sgs-view-contract` 改为 v1+v2；新增 `check:sgs2-ai` 并加入 `sgs:check:fast` | `sgs:check:fast` 21/21 |

**关键口径**

- 视图字段集由 `tests/view_contract.rs` 冻结（新增字段必须先改契约）。
- `public_state` 的 `player_area` 恒为空（无任何私有信息）；`player_view` 只含本人私有信息。
- 公共 `pending` 只给 `{kind, actor, skill}`；`options/data/escape` 只出现在**该行动者**的个人视图。
- 判定区按 **LIFO** 投影（与判定阶段结算顺序一致）。
- 回合驱动只在 `pending` 为空且结算栈为空时推进；弃牌阶段用内核流程帧询问（pending 必带出口）。
- 胜负只由 `turn::check_finished` 判定（存在 `identity=lord` 时；单测夹具无主公 → 不判定）。

**M3 度量**

- `cargo test -p sanguosha2`：318 条（新增 `tests/view_contract.rs` 9 条）。
- `npm run check:sgs2-ai`：10 局 × 5 人 / 终局 10 / illegal 0 / loops 0 / stalled 0 / pending kinds 11 / 约 4.5s。
- `npm run sgs:check:fast`：21/21。

**M3 遗留（进入 M4）**

1. 注册 / 网关（OQ3：重构完成前不开发；M4 先产出接口清单与完备性检查）。
2. 对拍 + v1 已知缺陷白名单（M4）。
3. 门禁数据源从 v1 `data.rs` 迁到 JSON（M4）。
4. 谮毁「代用/额外目标」的内核时序缺口（§9.2 未销号）。
5. 视图字段目前只覆盖 v2 自有 schema；Web 工作台/群侧文案适配随注册一起做（M4）。

## 11. M4 执行记录（2026-11-06，本轮范围）

**范围裁定（OQ3）**：注册 / 网关 / Web 切换 / 删 v1 **不在本轮**；本轮只做
「对拍 + 门禁数据源迁移 + 注册所需接口清单 + 删除清单」。

| 交付 | 内容 | 证据 |
|---|---|---|
| 可注册性检查 | `tests/plugin_contract.rs`（6 条）：接口完备 / 视图脱敏 / legal≡execute / pending 出口 / 状态映射 / 摘要 | `npm run check:sgs2-plugin` |
| 对拍工具 | `apps/scripts/check-sgs-parity.mjs`：160 张牌（名称/花色/点数/顺序）、牌定义（类别/槽位/范围/坐骑/延时）、目标模式（白名单）、技能覆盖（v2 ⊆ v1） | `npm run check:sgs-parity` |
| 对拍白名单 | `docs/games/sanguosha/engine-replacement/parity-whitelist.json`：v1 已知缺陷（jiuchi/qiaomeng/AI is_sha 等）+ 预期差异（目标模式 12 项、木牛流马）+ 白名单有效性自检 | 同上 |
| 门禁数据源迁移 | `docs/games/sanguosha/data/engine-catalog.json`（生成物：232 将 + 43 牌定义 + 2 牌堆）；`sgs-catalog` / `sgs-data-lib` / `general-names` / `skill-cases` / `skill-docs` / `gen-sgs-skill-text` / `pack-presets` / `align` 全部改读 JSON | `gen:sgs-engine-data --check`（v1+v2+catalog 三产物） |
| 注册资料 | `docs/games/sanguosha/engine-replacement/registration.md`：接口清单 / 注册步骤 / 灰度方案 / **6 个阻塞项**（选将、B8–B20、谮毁、身份局、AI、validate_state） | 文档 |
| 删除清单 | `engine-replacement/deletion-checklist.md`：前置条件 / 步骤 / 当前不满足项 | 文档 |
| 门禁接线 | `sgs:check:fast` 新增 `sgs2-plugin`、`parity`（23 步）；`engine-data` 名称/说明更新 | 23/23 |

**对拍结论（本轮）**

- 物理牌堆：v1 160 张 ≡ v2 160 张（名称/花色/点数/**顺序**全一致）。
- 牌定义：43 条（类别/槽位/武器范围/坐骑/延时）全一致；目标模式 12 项白名单差异
  （杀/火杀/雷杀、无中生有、闪/桃/酒/无懈、铁索、乐/兵、闪电）。
- 技能覆盖：v2 注册 97 个 ⊆ v1 implemented 301 个（无 v2 独有的「已实现」技能）。
- 白名单有效性：3 组 v1 已知缺陷片段仍存在（白名单未腐烂）；v1 删除时归档。

**DoD 对照**

| 原 DoD | 状态 |
|---|---|
| 注册接口清单 + `EnginePlugin` 完备性 | ✅ |
| 对拍 + v1 已知缺陷白名单 | ✅ |
| `check:sgs-*` 数据源从 v1 `data.rs` 迁到 JSON | ✅（`skills.rs` 实现表留到删 v1 前，列入 deletion-checklist §1） |
| Web 工作台切 v2（只读/沙盒） | ⏸ 按 OQ3 后移（依赖注册） |
| 删旧 crate | ⏸ **未执行**；`deletion-checklist.md` 当前前置条件不满足，需用户二次确认 |
| `npm run sgs:check` 全绿且全部指向 v2 | ⏸ 部分：可对 v2 跑的门禁（view-contract / ai / plugin / parity）全绿；`--fast` 23/23；全量 `sgs:check` 仍含 v1 Web/e2e（注册后切） |

**注册前阻塞（详见 `registration.md` §3）**

1. B1 选将/大厅未实现（固定 5 个 seat template）。
2. B2 仅 B0–B7 的 97 技能（52/147 将）。
3. B3 谮毁「代用/额外目标」时序缺口。
4. B4 身份局最小口径（主公 vs 反贼）。
5. B5 AI 仅最小策略。
6. B6 `validate_state` 未实现。

## 12. Harness 优化 loop（2026-11-06，B8–B20 开工前）

**目标**：下一会话直接开始 B8 迁移，不再造脚手架；所有「防漏登记 / 防漏测 / 决策可排障」能力前置。

### 12.1 新增工具

| 工具 | 作用 | 入口 |
|---|---|---|
| `examples/sgs2-registry.rs` | 导出 `skills::SKILLS` 为稳定 JSON（code/name/triggers/window/hooks/transform） | `cargo run -q -p sanguosha2 --example sgs2-registry -- --json` |
| `check-sgs2-registry.mjs` | 注册表唯一性 / 转换技清单 / **B0–B7 对齐单 ↔ 注册表精确一致（97=97）** / 每个 code 必须有 `#[test]` 覆盖（间接覆盖走白名单） | `npm run check:sgs2-registry` |
| `check-sgs2-decision-log.mjs` | AI 决策日志 schema：必填字段、`chosen ∈ legal`、候选 ≤64、step 单调、覆盖局数 | `npm run check:sgs2-decision-log` |
| `sgs2-selfplay --log` | 决策日志 JSONL（每步：actor/phase/pending/legal/chosen/hp/hands/deck） | `npm run check:sgs2-ai` / 直接跑 example |
| `sgs2-batch-scaffold.mjs` | 读对齐单 + 注册表，输出「本批要做什么/缺哪些技能/文件在哪/下一步」 | `npm run sgs2:batch -- B8` |
| `check:sgs2` 聚合命令 | 单 crate 全量门禁（tests + metrics + registry + decision-log + scenes + ai + parity） | `npm run check:sgs2` |

### 12.2 门禁接线

- `sgs:check:fast`：新增 `sgs2-registry`、`sgs2-decision-log` 两 step（现 **25 步**）。
- `V2_BATCHES`（`check-sgs2-registry.mjs`）= B0–B7：**每迁一批就把批次号加进去**，gate 随之要求
  「该批对齐单的 code 与 v2 注册表精确一致」→ 天然防止「文档写了、注册表漏了」。
- 用例覆盖 gate：注册表每个 code 必须出现在某个 `#[test]` 函数体；间接覆盖需在 `ALLOW_INDIRECT` 注明原因
  （当前 3 个：yongsi/huaiju/cunmu，由 B1 行为断言覆盖）。

### 12.3 文档

- `docs/games/sanguosha/batch-playbook.md` §9：v2 迁移批次开工命令 / v2 专有门禁 / 写法要点。
- `AGENTS.md`：新增引擎替换、v2 门禁、B8+ 脚手架的按需加载入口。
- `docs/games/sanguosha/engine-replacement/README.md`：M4 资料索引（已含）。

### 12.4 验收

- `npm run check:sgs2`：全绿（324 tests + metrics + registry 97 + decision-log 2 局 + scenes 12 + ai 10 局 + parity）。
- `npm run sgs:check:fast`：**25/25** 全绿。
- `npm run sgs2:batch -- B7`：输出「全部技能已注册」；`B8`（尚未建单）给出 `--new` 指引。

### 12.5 下一会话（B8–B20）建议顺序

1. `node apps/scripts/sgs-align-sheet.mjs --batch B8 --new` → 填第 1–5 段，第 6/7 段等用户确认。
2. `npm run sgs2:batch -- B8` → 按清单实现/注册/用例。
3. `V2_BATCHES` 加 8 → `npm run check:sgs2` → 回写对齐单第 2 段。
4. 每批完成后 `npm run sgs:check:fast`；用户抽检后再进下一批。
5. 全部 147 将迁移完成后，才回到注册/网关/Web/删 v1（`engine-replacement/registration.md`、`deletion-checklist.md`）。

## 13. B8 执行记录（2026-11-09，v2 迁移批次首跑）

**结论：B8「一将成名·Y7」8 将 / 17 技能 + 绝响授予的 4 条子技能已在新内核落地；`cargo test -p sanguosha2`
367 条全绿（新增 `tests/b8.rs` 43 条），`check:sgs2` 与 `sgs:check:fast`（25/25）全绿。**

### 13.1 内核增量（B8 引出，机制词表新增 4 条）

| 机制 | 落点 | 说明 |
|---|---|---|
| `pindian` 拼点 | `kernel/pindian.rs`（`FlowKind::Pindian`） | 双方**暗选**（选择期不公开，同怃戎）→ 同时亮牌 → 比**有效点数** → 拼点牌进弃牌堆；平局=无赢家。天辩通过通用钩子 `pindian_options/pindian_take/pindian_rank` 接线，内核无技能名 |
| `target-extra` 目标数修正 | `card_play::planned_targets` + `advance_target` | 红色基本牌/红色普通锦囊可多指定 N 个目标（无距离）；单目标锦囊逐目标循环；延时锦囊不参与 |
| `deck-carrier` 牌堆承载 | `card_play::plan_use`（`Zone::Draw` 承载） | 伏诛「使用牌堆中的【杀】」走真实结算（出闪/防具/属性），结算后进弃牌堆并洗牌 |
| `card-gained` 获得语义 | `Game::gain`（复用） | 怠宴/惠民/复难/诫训/通博的获得统一走事件链 |

另外新增（非机制词表）：**结算前【杀】介入窗口**（`skills::sha_intercept_skills` + `Game.sha_intercept`），
解决「`TargetChosen` 事件在牌结算**之后**才路由」的时序缺口——这正是 §9.2 谮毁未销号的同因缺陷；
守玺/谏征/专对已改走该窗口。手牌上限通用钩子（`skills::hand_limit_delta`）、跨回合记忆
（`Game.turn_seq` + 座位计数）、确定性技能随机源（`Game::rand_below` / `shuffle_draw`）同批引入。

### 13.2 与 v1 的行为差异（已写进对齐单第 7 段）

- 忠鉴：同色改**二选一**（摸牌/弃置），展示牌由该角色自选并公共展示。
- 才识：满体力时 opt2 不可选；「不能对自己使用牌」含装备牌。
- 福绵②：覆盖红色基本牌与红色普通锦囊（v1 只对红色【杀】）；数值切换 +1、下一回合重置为 1。
- 怠宴：红桃基本牌「牌堆顶→底第一张，无则遍历弃牌堆」；严格「上一个回合」判定。
- 伏诛：真实结算（v1 直接 1 点伤害）；上限=当局游戏人数；目标离场停止。
- 守玺：牌名＝全部非装备牌（含延时锦囊）；每种**每回合**限一次；opt1 无该牌时不可选；由曹节选被获得的牌。
- 惠民：摸 X 张公共展示后，按座位顺序**每名角色依次选择**（同五谷，v1 是固定发放）。
- 清弦：补「回复体力后」分支；全局濒死判定；「随机使用装备牌」＝牌堆顶向下第一张装备牌（无则遍历弃牌堆）。
- 绝响：真随机授予四条弦技（v1 授予「清弦」）。
- 谏征/专对：改在**结算前**介入；专对取消 v1 的「每回合限一次」，改为「每张【杀】每个时机只问一次」。
- 复难：「不能使用或打出」走**手牌冻结**（按牌 id）并覆盖「打出」；诫训后升级为「复难plus」（名称与描述同步替换）。
- 诫训：X=场上**公开**方片（装备区+判定区）；Y 从 1 开始；弃置后**最终**手牌+装备区皆空才失技。
- 辟撰：改为可选；「书」公共可见。
- 通博：任意张牌（手牌/装备区）等量替换 + 四花色齐时交给一名其他角色；槽位严格校验。

### 13.3 已知遗留

1. **通博特殊交互区 DOM**：本轮只做引擎侧协议/槽位校验（`own:hand:*` / `own:equip:<slot>:*`）与等量替换，
   观星式交互区 UI 随注册/Web 阶段落地（`registration.md` OQ3）。
2. 复难触发目前接在「响应牌打出」出口（`apply_response`），覆盖【闪】/【杀】/【无懈】；若未来出现
   「非牌响应」（技能视为打出），需在同一出口补 `None` 承载。
3. 拼点的「可获得拼点牌」类技能尚未出现；新增时走技能描述显式声明（mechanics `pindian` 不变量 5）。

### 12.6 B8 后 harness 优化 loop（2026-11-09）

开工对象＝计划「harness 优化项」；全部落地且 `sgs:check:fast` 25/25 复跑通过。

| # | 优化 | 落点 | 效果 |
|---|---|---|---|
| H1 | **机制用例锚点** | `check-sgs2-registry.mjs`：`MECHANISM_ANCHORS`（机制 id → 测试锚点）+ `MECHANISM_ANCHOR_WAIVED`（措辞类豁免须给理由） | 对齐单声明过的每个机制都必须在 `tests/*.rs` 有内核级用例；防「机制只在单个技能用例里间接覆盖」。首跑发现 `lose-vs-discard`/`target-list`/`wording-*` 三类缺口并补齐 |
| H2 | **批次可开工判据** | `sgs2-batch-scaffold.mjs`：读 `<B>.json` 输出 status / 疑点未答数 / `MECHANISM_SUPPORT` 机制缺口 / `✅ 可开工` | 迁移前不用再人工翻文档判断「能不能开工」；未登记机制会显式报 `❓` |
| H3 | **迁移趋势行** | `check-sgs2-metrics.mjs`：v2 注册技能 / v1 已实现 / 对齐单批数与疑点未答数 / 每批已实现分布 / 技能文件行数 | 趋势可见（当前 114/301，疑点未答 0，b8=2508 行） |
| H4 | **第 2 段锚点报告** | `sgs-align-sheet.mjs --batch <B> --v1-anchors` | 填「现实现」前先拿到 v1 的 `act_/step_` 函数与钩子位置（只打印，不伪造 prose） |

**结论**：B9 起的固定顺序变为 `sgs2:batch -- B9`（看是否 🟢 可开工）→ `--v1-anchors` 填 §2 →
裁定 → 实现 → `check:sgs2` → `sgs:check:fast`。

## 14. B9 执行记录（2026-11-09，界标·蜀）

**口径**（用户裁定）：标将已基本废弃 → 界将技能**按 v1 实现**；v2 不保留 `is_jiexian` 分支，
同一 code 只实现界版语义（将来迁标将若需差异，用角色属性参数化）。

| 交付 | 内容 |
|---|---|
| 对齐单 | `align/B9.json` 14 技能 / 17 疑点（5 处非 A：仁德候选含火杀/雷杀、【闪】不列出；激将摸牌扩到其自己回合外使用/打出并让其选择；义绝由目标自选展示且目标须有牌；铁骑弃牌含装备区；集智改可选），第 7 段确认 → implemented |
| 技能 | `src/skills/b9.rs`：仁德/激将、义绝/武圣、替身/咆哮、涯角/龙胆、马术/铁骑、空城/观星、集智/奇才 |
| 用例 | `tests/b9.rs` 27 条（每技能正例 + 反例，含转化牌/接力/观星双窗口/铁骑判定/白银狮子压制） |
| 内核 | 被动转化技（`convert:` / `vrespond:`）、主公技接力（`relay_played`/`relay_none` + `relay_wait`）、非锁定技失效闸门（`skills::active` + `LOCKED_SKILLS`）、手牌封印（`hand_sealed`）、数值钩子（`distance_delta` / `sha_limit_override` / `sha_no_distance` / `trick_no_distance` / `prevent_equip_discard`）、伤害加成（咆哮/义绝接入 `damage_bonus_typed`） |
| 门禁 | `cargo test -p sanguosha2` 391 全绿；`check:sgs2` 全绿（注册 132 / 对齐单 128 / 机制锚点）；`sgs:check:fast` 25/25 |

**已知遗留**

1. 激将「替主公**使用**【杀】」（出牌阶段代用）未做——v1 亦只做响应接力；已在对齐单第 3/7 段登记。
2. `LOCKED_SKILLS` 目前是手写清单（B0–B9 范围内的锁定技）；后续批次新增锁定技时必须同步登记，
   （建议后续 harness 化：注册表导出 `kind` 后改为派生）。
3. 奇才的「防止弃置」只接在过河拆桥（与技能侧的 `prevent_equip_discard` 助手）；其余弃置入口
   （顺手牵羊的获得、忠鉴/诫训弃牌）按裁定「只拦弃置」处理，忠鉴/诫训已可用该助手扩展到防弃置。

## 15. B10 执行记录（2026-11-09，界标·吴）

| 交付 | 内容 |
|---|---|
| 对齐单 | `align/B10.json` 17 技能 / 25 疑点（用户逐条裁定）→ implemented |
| 技能 | 奇袭/奋威（界甘宁）、克己/勤学/博图（界吕蒙）、苦肉/诈降（界黄盖）、英姿/反间（界周瑜）、流离/国色（界大乔）、谦逊/连营（界陆逊）、制衡/救援（界孙权）、枭姬/结姻（界孙尚香） |
| 内核 | `extra_turn`（额外回合）、显式跳过弃牌阶段、赋值型手牌上限、多目标无效 `fenwei_void`、结算前手牌移出 `void_return`、手牌归零批次 + 连营询问、回复前改向（救援）、装备流失钩子（枭姬）、被动转化支持装备区承载（奇袭） |
| 用例 | `tests/b10.rs` 19 条 |
| 门禁 | `cargo test -p sanguosha2` 410 全绿；`check:sgs2` 全绿（注册 149 / 对齐单 145）；`sgs:check:fast` 25/25 |

**追加（同日，用户裁定「需要实现」）**：
- 【攻心】已实现：`b10::GONGXIN`（授予型，`GRANT_ONLY`），勤学觉醒时 `grant_skill("gongxin")`；
  「观看」用 `reveal_private`（私密展示给使用者），弃置/置顶仅限其红桃手牌，出牌阶段限一次。
- 「黑色牌当【兵粮寸断】」的**延时锦囊转化**路径已验证可用（`use_as` + 严格校验），
  界徐晃·断粮可直接复用 `conversions` 钩子（`to: "兵粮寸断"`，`playable: false`）。

**已知遗留**：①制衡/苦肉/结姻等「装备区」选牌已支持，但 Web 侧展示待注册阶段；
②奇袭的装备区承载依赖 `plan_use` 的非严格分支，若将来出现「装备区牌当锦囊」的更泛需求，
应把 `UseOpts` 升级为显式的「来源区」参数；③攻心的「观看」目前用私密展示实现，
若未来需要「观看后可展示其中一张给对方看」，需补一个公开展示步骤。

## 16. B11 实施记录（2026-11-09，已收口）

**结论：B11 已整批收口**（14 技能 + 14 用例，`cargo test -p sanguosha2` 427 全绿；`check:sgs2` 与
`sgs:check:fast` 全绿）。中途两轮失败后定位到下面两条缺陷，第三轮按修正落地。

### 16.1 已建立的内核窗口（本轮验证可行、随下轮一起落地）

| 窗口 | 设计 | 状态 |
|---|---|---|
| 技能判定子流程 | `FlowKind::QuickJudge`：亮判定牌 → 鬼才改判 → 生效 → 天妒；结果写 `Game.judge_result` | 结构可行；收尾易错（见 16.2-A） |
| 改判 | `Game.judge_rewrite: Option<CardId>` + `skills::start_judge_rewrite`（手牌/装备区牌均可） | 可行 |
| 天妒 | `skills::start_tiandu`（判定生效后询问获得） | 可行 |
| 【闪】接力 | `relay_options(kind=Shan)` → 护驾（魏势力代出） | 可行（用例已过） |
| 倾国装备区 | `apply_response` 的 `vrespond` 允许「自己装备区」承载 | 可行（用例已过） |
| 清俭/洛神上限 | `hand_limit_delta` 增加 `turn.qingjian_limit:{seat}` / `turn.luoshen_free:{seat}` | 可行 |

### 16.2 两条真实缺陷（下轮先修这两条）

- **A. 判定牌收尾的牌区判断**：`flow::judge_card` 把判定牌放进 **`Zone::Frame(..)`**（不是 Processing）。
  QuickJudge 收尾必须 `matches!(zone, Some(Zone::Frame(_)) | Some(Zone::Draw) | Some(Zone::Processing))` 才弃置，
  否则会把**天妒已经获得**的牌再弃掉（天妒用例失败的直接原因）。
- **B. `Step::Next` 会**替换**触发帧的 `data`**：事件触发（`DamageAfter` / `CardGained` / `PhaseStart`）的技能
  一旦跨步，`a.data["seat"/"cause"/"card"]` 全部丢失 → 后续分支读到 None 直接 `Done`（表现为「提交后续动作时没有待办」）。
  **约定**：事件触发技能的第一步就把需要的上下文写进**持久计数**（`turn.<skill>_*`），跨步只读计数。
  本轮已按此改造的有 fankui（left/src）、tuxi（left）、luoshen（judged）、ganglie（src）——仍有个别分支遗漏。

### 16.3 最终结果

- 14 技能全部实现并通过用例；关键修正：
  1. QuickJudge 收尾按牌区判断（缺陷 A）；
  2. 多步技能改为**「答完即压新帧」**（缺陷 B 的根治：新帧自带 data，不依赖 `Step::Next`）；
  3. `skill-spec-rules.md` §13 把这两条写成约定，后续批次直接遵循。

### 16.4 下轮开工顺序（机械执行）

1. 先修 A（一行）+ 给 QuickJudge 补 2 条内核用例（天妒获得后不进弃牌堆 / 鬼才改判后原牌进弃牌堆）。
2. 再修 B：把 qingjian/wangxi/ganglie/yiji 的所有跨步上下文搬到 `turn.*` 计数，并加 1 条回归
   「事件触发帧跨步后 data 丢失不影响语义」。
3. 再按 `align/B11.json` 第 7 段实现 14 技能（文件已写过两版，可复用结构）+ `tests/b11.rs` 15 条。
4. `V2_BATCHES` 加 11 → `check:sgs2` → `sgs:check:fast` → 回写 impl/tests → M83 → 单条 commit。


## 17. B12 实施记录（2026-11-09，已收口）

**结论：B12 已整批收口**（4 将 / 8 技能 + 18 用例，`cargo test -p sanguosha2` 448 全绿；
`check:sgs2` 与 `sgs:check:fast` 全绿）。本批没有走「先失败两轮」的弯路，因为两条内核缺口在
对齐阶段就被登记成机制条目（`multi-response` / `no-wuxie`），实现前先补内核再写技能。

### 17.1 新增的内核能力

| 能力 | 落点 | 备注 |
|---|---|---|
| 多张响应（`multi-response`） | `skills::response_need` + `card_play::sha_loop` / `duel` 的 `data[need]/[got]` 计数 | 技能只声明张数，不改响应窗口；不足 N 张 = 未抵消 |
| 不能被无懈可击抵消（`no-wuxie`） | `UseOpts::no_wuxie` → 使用帧 `data["no_wuxie"]`；`Game::use_virtual_no_wuxie` | 与 `unresponsive`（闪/杀不响应）**分开**，否则会误关【决斗】的出杀响应 |
| 濒死救援的转化【桃】 | `skills::dying_conversions` + `flow::step_dying` 的 `vrespond:<skill>:<牌>` | 承载牌可来自装备区（走统一移动，触发失去装备钩子） |
| 反向距离修正 | `skills::distance_to_delta` → `Game::distance_raw` | 义从「他人算到我 +1」；距离链统一改 `i32` 累加、下限 1 |

### 17.2 语义裁定里值得记住的（用户 2026-11-09）

- **除疠**：弃牌由**被弃置的角色本人**选（不是华佗代选）；判定区不参与、离开判定区不算弃置。
- **利驭**：装备牌分支的【决斗】对象由**受伤角色**指定（v1 是吕布指定，属实现偏差）。
- **无双**：决斗时「与吕布决斗的对方」每次两张【杀】，**不论决斗由谁发起**（v1 只做了吕布发起一侧）。
- **闭月**：结束阶段在弃牌阶段**之后**，本次摸牌不会再触发弃牌，超上限的牌留到下个弃牌阶段。

### 17.3 下轮（B13 界标·J1）开工顺序

1. `node apps/scripts/sgs-align-sheet.mjs --batch B13 --new` → 填 2–5 段 → 交用户裁定。
2. 先看 `npm run sgs2:batch -- B13` 的机制缺口（`multi-response` / `no-wuxie` 已 ok）。
3. 实现 `skills/b13.rs` + `tests/b13.rs` → `V2_BATCHES` 加 13 → 回写 impl/tests → M85 → 单条 commit。
