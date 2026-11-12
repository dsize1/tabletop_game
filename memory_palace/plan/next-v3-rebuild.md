# 任务：V3 引擎重构（v2 废弃）

> 立项：2026-11-12（用户裁定，见 ADR-0042）。**给新会话的开工说明**：
> 本文件是独立任务文档，新会话直接说「开始 V3 重构」，从 §3 的 M0 开始。

## 1. 结论先说

- v2（`crates/engines/sanguosha2`）**不再修补**，作为参考实现保留；
- V3 的目标不是「再写一遍」，而是**先有可机器验证的语义基线，再写实现**：
  v1 作为行为 oracle + spec-kit 契约驱动 + 每批次对拍清零。

## 2. 必须继承的资产（不要重写）

| 资产 | 位置 | 用途 |
|---|---|---|
| 语义宪法（I1 询问完整性 / I2 状态语义 / I3 视图不判业务 + 两条硬约束） | `specs/sanguosha/constitution.md` | V3 的最高约束，逐条转成用例与门禁 |
| 契约门禁 | `specs/sanguosha/contracts/*.json` + `check:spec-conformance` | 规则↔用例锚点↔禁止模式 |
| 约定门禁（C1–C6 声明化） | `contracts/conventions.json` + `check:sgs2-conventions` | 体力单出口/内核去技能名/标记/计数器/出口/kind |
| 775 条裁定↔用例锚点 | `docs/games/sanguosha/align/B*.json` | 技能语义的验收清单（V3 可直接复用） |
| 296 技能注册表快照 | `specs/sanguosha/contracts/skills.json` | V3 技能契约的覆盖校验基线 |
| 口径类用例（V3 必过） | `tests/ask_completeness.rs`、`tests/state_semantics.rs`、`tests/b3.rs::yuce_opt1_must_discard`、`tests/b9.rs::jijiang_*`、`tests/b15.rs::tuotian_ignores_other_players_card_loss`、`tests/b20.rs::yanyu_ignores_other_players_lost_sha` | 把 v2 的坑变成回归 |
| 端侧多段交互形态 | `docs/conventions/interaction-primitives.md` + `apps/web/src/view/sgs/targetPicker.ts` | 选牌 → 多选目标 → 确认；上下界由合法集派生 |
| 数据生成器 | `gen:sgs-engine-data`（160 牌 / 232 将 / 296 技能） | 数据不重写，只重写行为 |

## 3. 里程碑（建议顺序，M0 不可跳过）

### M0 行为基线：v1 作为 oracle（0.5–1 天）
- 从 git `f8a7db3^` 复活 v1 为 **test-only crate**（不注册网关、不上线）；
- 同 seed / 同动作脚本回放：对比 **legal 动作集 / pending 序列 / 状态（hp·手牌数·区域）/ 日志事件**；
- 产出 `docs/games/sanguosha/v3-parity-baseline.md`：v1–v2 差异清单（P0/P1）+ v1 已知缺陷白名单（3 条可直接复用）。

### M1 内核：状态 / 事件 / 交互 / 合法集（最高风险，先做）
按 v2 的 7 类缺陷反推设计，每条先写用例再写实现：
1. **状态模型**：`hp` / `alive` / `dead` 三分显式；事件携带 `turn/phase/lifecycle/by/from` 快照；
2. **交互层**：`Interaction` 描述符（类型：选牌/选目标/多选/分区/确认）驱动端侧多段，
   `legal_actions` 只描述「可选动作 + 合法目标集合」，不把组合摊平成 N 个 id；
3. **询问不变量**：`must_ask` 单出口（I1），承诺步骤 `escape_mandatory` + 提交拒绝；
4. **帧数据归属**：进度数据只能写发起帧（硬约束 B）；
5. **事件归属**：每个订阅声明 `subject: Self/Other/By/From`，handler 首行校验（M105 教训）。

### M2 卡牌结算 + 生命周期
杀/闪/桃/酒/锦囊/装备/判定/无懈链；开局/改盘/终局/跨进程群登记（单一事实源：SQLite 快照，不靠内存）。

### M3 视图 + 端侧
三分区视图契约；端侧多段交互沿用 `targetPicker` 形态；日志文案显示名（武将名优先）。

### M4 技能分批（296 个，契约驱动）
用 `align/B*.json` 的 775 条裁定 + `contracts/skills.json` 覆盖校验；
每批收口 = 差分对拍 + 契约门禁 + 真机抽检，**不接受"形状绿、语义错"**。

### M5 验收
- 与 v1 对拍：P0 差异 0；v1 白名单内的已知缺陷例外；
- 真机：8 人局连续 10 局无 P0（含身份/询问/激将/多段交互/终局重开）；
- 门禁：`check:spec-conformance` + `check:sgs2-conventions`（沿用并扩展）+ 差分常驻。

## 4. 过程要求（针对 v2 的失败方式）

1. **规格先行**：每条语义先在 `specs/sanguosha/` 写契约 + 用例锚点，再写实现；无契约不合并；
2. **删旧引擎前抽契约**：任何"删除参考实现"的动作，前置条件是把它代码注释里的裁定/口径全部搬进 `specs/`；
3. **差分常驻**：v1 oracle 不只是一次性工具，进 CI（每次提交跑核心对拍）；
4. **禁止"报一个修一个"**：每发现一个缺陷，必须归入某类并补该类门禁/扫描，再修实例（M105 就是这样做对的）。

## 5. 复用 vs 重写

| 复用 | 重写 |
|---|---|
| `cards_generated.rs` / `generals_generated.rs`（数据） | 内核：`kernel/{mod,resolve,card_play,flow,pending}`（交互/事件/帧） |
| 视图契约与端侧（`apps/web`、`targetPicker`、`pipeline`） | 296 技能 handler 形态（改为契约驱动 + subject 声明） |
| server / gateway / 存储 / 认证接线 | 生命周期与跨进程状态同步 |
| align 单 / 门禁脚本 / 用例口径 | AI（改为消费 Interaction 描述符） |

## 6. 风险与前置

| 风险 | 对策 |
|---|---|
| v1 复活编译失败（`EnginePlugin` API 漂移） | 只做 test-only crate，必要时在本 crate 内做适配层，不碰主工程 |
| 差分噪音（v1 已知缺陷） | 先产出 v1 缺陷白名单（spike-report §12.2 已列 3 条） |
| 技能量太大（296） | 按 B0–B20 批次推进，每批 8 将、契约先行、对拍清零 |
| 时间 | V3 不做「一次性大爆炸」；M0–M3 内核先跑通 1 个批次（B0）再滚雪球 |
