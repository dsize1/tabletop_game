# 任务：引擎 v2 卡牌结算 + 整体替换 v1（M1–M4）

> 立项日期：2026-11-06。状态：**已裁定（2026-11-06），待新会话执行**。
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
| 卡牌 | 160 张真实牌堆（`data.rs` 生成）+ 装备/锦囊/判定/无懈完整循环 | `cards.rs` 手写 20 张测试牌 + 属性表（**无真实牌堆**） |
| 牌结算 | `play_card` / `start_sha_flags` / `start_trick_code`（逐牌型分支） | **无**：`use_card` 只发事件；`use_virtual` 无承载牌 |
| 伤害/濒死/判定 | 完整（但 6 条延迟队列 + 散装"禁止使用"） | `flow::damage*` / `dying` / `judge_*`（单一出口，已 220 用例覆盖） |
| 技能 | 232 将 / 294 技能（含缺陷） | B0–B7 共 **76 技能** implemented |
| 视图/AI | `view.rs` + `ai/` | 无（spike-report §4） |
| 注册 | `game-engine-mcp` 已注册 | **未注册** |
| 门禁 | `view-contract` / `sgs-ai` / `replay` 全部指向 v1 | `cargo test -p sanguosha2` + 5 条机制门禁 |

## 3. 里程碑（每个都是可验收的切片）

### M1 卡牌结算内核（最高风险，先做）
1. **牌定义数据源**：用 `gen:sgs-engine-data` 从 v1 `data.rs` 生成 v2 的 160 张牌堆 + 装备/锦囊属性（单一 JSON，v2 只读它；以后 v1 删除也不丢数据）。
2. **使用→目标→响应→结算 单一流程**：`use_card` 变成真实结算入口（`play_card` 等价物），保留 `use_virtual` 兼容已实现技能；牌进入 `processing` → 结算 → 弃置/装备/判定区。
3. **基本牌**：杀（含火杀/雷杀属性）、闪、桃、酒（含酒加成标记）、濒死救援接入。
4. **装备**：武器范围/防具/坐骑/宝物接入统一数值链（顺序：技能加成 → 藤甲 → **白银狮子最后**）。
5. **锦囊**：普通锦囊 + 延时锦囊（判定区 LIFO）+ 无懈链（`ResponseKind::Wuxie`）。
6. **DoD**：新增 ≥60 条牌结算用例（每牌型 ≥1 正例 + ≥1 反例）；守恒 160 / 无帧遗留 / 单一延迟队列门禁全绿；单动作耗时基线（对比 v1 `--all` 66s 量级）。

### M2 B0–B7 接线复查（把打桩换成真结算）
1. 逐条消掉对齐单里的「v2 无结算引擎 → 打桩」收口项（B3 奇策、B4 忠勇/勇略/司敌/陷嗣、B5 谮毁、B6 战绝/勤王/怃戎、B7 滔乱/止戈/督粮…）。
2. 每技能保留既有反例，并补「真实结算后仍不误触发」的断言。
3. 回写对齐单第 2 段（现实现）为**新引擎**行为，`impl` 锚点更新。
4. **DoD**：B0–B7 的 76 技能全部指向真实结算；`check:sgs-align` 无冲突。

### M3 视图 / 协议 + AI 最小接口
1. `public_state` / `player_view`：9 类区 + `revealed` / `private_reveals` 投影 + pending 结构（复用现有 `presentation`）。
2. AI：legal 枚举（已有）+ 观测流最小版 + 决策日志（复用 `check:sgs-decision-log` 口径）。
3. `check:sgs-view-contract` 改为可对 v2 运行。
4. **DoD**：视图契约门禁对 v2 全绿；AI 能与自己打满 10 局无 illegal/loop（沿用 `check:sgs-ai` 口径）。

### M4 对拍 / 门禁数据源 / 退役（注册/网关按 OQ3 后移，不在本轮）
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
| `cargo test -p sanguosha2` | 220 条 | M1 +60 条牌结算；M2 全批接线后不回退 |
| `check:sgs2:metrics` | 技能 LOC 比值 | 增加「牌结算 LOC / 牌型数」 |
| `check:sgs-counts` / `batch-consistency` | 解析 v1 `data.rs` | 改读 `data/*.json`（M4） |
| `check:sgs-align` | 已覆盖 52 将 | 冲突 0；打桩清单销号 |
| `check:sgs-view-contract` / `check:sgs-ai` | 只跑 v1 | 可对 v2 跑（M3） |
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
