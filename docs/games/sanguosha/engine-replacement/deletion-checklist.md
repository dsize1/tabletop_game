# 删除 v1（`crates/engines/sanguosha`）清单

> 裁定：`docs/games/sanguosha/legacy-retire.json` —— B20 验收签字 + 30 天 / 硬上限 2027-01-31；
> **已删除（2026-11-10，用户签字）**：`crates/engines/sanguosha` 已移除；删除后 `npm run sgs:check` **30/30**。
>
> 状态：**B0–B20 迁移完成 + B19/B20 签字**；S1–S6b 全部完成（v2 接管 `game_id=sanguosha`、
> MCP/server/Web/网关/中台切 v2、门禁事实源迁 v2、`skills.md` 迁 v2、v1 crate 删除）。
> 待用户后续统一验证 B0–B18 人工列（机器证据已齐）。

## 1. 前置条件（全部满足才可删）

- [x] M2 已签字（B0–B7 接线 + 谮毁缺口处置；S2 已销号：`Game.use_pre` + `FlowKind::PreUse`）。
- [x] M5 完成：v2 注册 `game-engine-mcp`、网关 `GAME_OPTIONS` 沿用 `sanguosha`（id 不变）、server 执行宿主切 v2。
- [x] M6 完成：Web 工作台切 v2（视图 v1 兼容 `check:sgs2-view-parity --strict` 80/80；`test:sgs-web` / `test:sgs-bot` / `workbench-preview` 全绿）。
- [x] 灰度期：**M8 未做真实时长灰度**（本轮只做删除演练；正式灰度见 §2.5）。
- [x] **技能实现事实源迁移**：`check:sgs-counts` / `check:sgs-align` / `sgs-data-lib` / `sgs-catalog` 全部改读 v2 生成物 + v2 注册表。
- [x] `check-sgs-parity` 在 v1 缺失时自动跳过（白名单归档见 §3）。
- [x] `npm run sgs:check`（非 --fast）全绿；删除演练后仍 **30/30 全绿**。
- [x] **用户明确确认删除**（2026-11-10 口头签字：「1 同意删除；2 做；3 不补，待我验证后一起补」）。

## 2. 删除步骤（✅ 已于 2026-11-10 执行）

1. （可选）归档对拍语料：`parity-whitelist.json`、`spike-report.md` §12.2。
2. 删除 `crates/engines/sanguosha`；从 `crates/Cargo.toml` workspace members 移除 `"engines/sanguosha"`。
3. 清理注册：server/MCP 已无 v1 依赖；网关只有 v1/v2 兼容座位适配器（可简化）。
4. 门禁已迁移（S6）：`skill-cases`/`counts`/`align`/`pack-presets`/`data-lib`/`sgs-catalog` 不再读 v1；
   `skill-docs` 在 v1 缺失时跳过（文案迁移见 S6-b）。
5. `check-sgs-parity.mjs` 保留为「v1 缺失自动跳过」的历史工具（或删除）。
6. 全仓 grep 悬挂引用（脚本/docs 历史记录除外）：
   `rg "crates/engines/sanguosha\b" apps crates --glob '!**/sanguosha2/**'`
7. 跑 `npm run sgs:check`（全量）+ `node apps/scripts/check-sgs-retire.mjs`（应输出「旧 crate 已删除」）。

## 2.5 删除演练（2026-11-10，已完成）

方式：临时把 `crates/engines/sanguosha` 移出仓库 + 从 workspace members 移除 → 跑全量门禁 → 恢复。

| 检查 | 结果 |
|---|---|
| `cargo build -p server -p game-engine-mcp` | ✅（无 v1 依赖，10.5s） |
| `npm run sgs:check`（全量 30 步） | ✅ **30/30**（含 preview/build:props/test:web/test:sgs-web/test:sgs-bot/decision-log） |
| `check:sgs-retire` | ✅ 输出「旧 crate 已删除，无需提醒」 |
| `gen --check` | ✅ v1 已删除，跳过 v1 产物（v2 + catalog 仍校验） |
| `check-sgs-parity` | ✅ 自动跳过 |
| `skill-docs` | ✅ 自动跳过（文案迁移属 S6-b） |
| `counts`/`align`/`pack-presets`/`batch-status`/`skill-cases`/`metrics` | ✅ 全绿（事实源已切 v2） |
| `check:sgs-view-contract` | ✅ 只跑 v2（脚本已去掉 v1 部分） |

结论：**删除 v1 在技术上已无阻塞**；剩余只有「用户签字」与「`skills.md` 文案事实源（S6-b）」。

## 3. 删除后可保留的历史证据

- `docs/games/sanguosha/spike-report.md`（6 类缺陷 + 迁移前置）
- `docs/games/sanguosha/scenes/`（v2 场景）
- `memory_palace/plan/next-engine-replacement.md`（M1–M4 记录）
- `docs/games/sanguosha/data/engine-catalog.json`（武将/牌名录唯一机器源）
- `docs/games/sanguosha/engine-replacement/parity-whitelist.json`（对拍白名单；删除后归档）
