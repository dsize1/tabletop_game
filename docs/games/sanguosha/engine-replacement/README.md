# 引擎替换（v1 → v2）M4 资料

> 计划：`memory_palace/plan/next-engine-replacement.md`（M1–M4）。
> 状态（2026-11-10）：M1/M2/M3 已完成；**B0–B20 全部迁移完成**（147/147，v2 注册表 295 技能）；
> B19/B20 已用户签字。**S1–S5 + S2-b + S4-b 已完成**：v2 接管 `game_id=sanguosha`，
> MCP/server/Web/网关/中台全部切 v2，门禁事实源已迁 v2；**删除 v1 演练通过（`sgs:check` 30/30）**，
> 只差用户签字 + `skills.md` 文案迁移（S6-b）。见 `deletion-checklist.md` §2.5。
> 退役时钟：`npm run check:sgs-retire`（软期限 2026-12-10 / 硬上限 2027-01-31）。

| 文件 | 内容 |
|---|---|
| `registration.md` | 注册所需接口清单、灰度方案、**当前阻塞项**（不开发，只登记） |
| `deletion-checklist.md` | 删除 v1 的前置条件与步骤（需用户二次确认；当前**不满足**） |
| `parity-whitelist.json` | v1 ↔ v2 对拍白名单（OQ4：v1 已知缺陷 + 预期差异） |

## 本轮 M4 已落地

1. **可注册性检查**：`cargo test -p sanguosha2 plugin_contract`（6 条）+ `check:sgs2-plugin`。
2. **对拍工具**：`npm run check:sgs-parity`
   - 160 张物理牌（名称/花色/点数/顺序）v1 ≡ v2；
   - 牌定义（类别/槽位/武器范围/坐骑/延时）一致；目标模式差异走白名单；
   - v2 已实现技能 ⊆ v1 implemented（97 ⊆ 301）；
   - 白名单有效性自检（v1 已知缺陷片段仍存在，防白名单腐烂）。
3. **门禁数据源迁移**：`docs/games/sanguosha/data/engine-catalog.json`（生成物）取代 v1 `data.rs`
   作为脚本/门禁的武将+牌名录来源：
   - `lib/sgs-catalog.mjs` / `sgs-data-lib.mjs` / `general-names` / `skill-cases` / `skill-docs` /
     `gen-sgs-skill-text` / `pack-presets` / `align` 均已切换；
   - `gen:sgs-engine-data --check` 同时校验 v1 `data.rs` + v2 `cards_generated.rs` + catalog JSON。
4. **未做（按 OQ3 后移）**：`game-engine-mcp` 注册、网关开关、Web 工作台切换、v1 删除。

## 仍依赖 v1 源的部分（删除 v1 前必须处理）

| 依赖 | 说明 | 处理时机 |
|---|---|---|
| `crates/engines/sanguosha/src/skills.rs` | 技能实现表（`implemented` 标记）仍是 v1 事实源；**v2 注册表已有 B0–B20 的 288 个对齐 code（注册总数 295，含授予技）** | v1 删除前把「已实现技能清单」迁到 v2 注册表或 JSON（M7） |
| `check-sgs-pack-presets` 的 `lib.rs`/`state.rs` 断言 | v1 大厅预设/默认包实现事实 | 随注册/大厅迁移到 v2 |
| `sgs-general-audit` / `sgs-batch-*` 的 `p67.rs` / 场景 | v1 走查基线 | 随 v1 退役归档（对拍语料保留） |
| `check-sgs-parity` 本身 | 对拍只在迁移期有意义 | 删除 v1 时删除该脚本（白名单归档） |
