# 删除 v1（`crates/engines/sanguosha`）清单

> 裁定：`docs/games/sanguosha/legacy-retire.json` —— B20 验收签字 + 30 天 / 硬上限 2027-01-31；
> **删除前需用户二次确认**（本文件不执行删除）。
>
> 当前状态（2026-11-10）：**B0–B20 迁移完成 + B19/B20 用户签字**（退役时钟已启动：软期限 2026-12-10）；
> 但**仍不满足删除条件**：M5（注册/网关）、M6（Web 切 v2）、M8（灰度/回滚）、M7 剩余（技能清单事实源）未完成。

## 1. 前置条件（全部满足才可删）

- [ ] M2 已签字（B0–B7 接线 + 谮毁缺口处置）。
- [ ] M4-later 完成：v2 已注册 `game-engine-mcp`、网关开关可用、Web 工作台切 v2 并通过真机走查。
- [ ] 灰度期结束：白名单房间跑 v2 至少 N 天，无 P0/P1 缺陷；回滚演练过（engine 改回 v1）。
- [ ] **技能实现事实源迁移**：`check-sgs-*` 不再读 v1 `skills.rs`（改用 v2 注册表或 catalog JSON）。
- [ ] `check-sgs-parity` 白名单归档（`engine-replacement/parity-whitelist.json` → `archive/`）。
- [ ] `npm run sgs:check`（**非 --fast**）全绿；`check:sgs-retire` 提示可删。
- [ ] 用户明确确认删除（记录在计划/ADR）。

## 2. 删除步骤（确认后执行）

1. 归档对拍语料：`parity-whitelist.json`、`spike-report.md` §12.2、已知缺陷清单。
2. 删除 `crates/engines/sanguosha`；从 `crates/Cargo.toml` workspace members 移除。
3. 清理注册：`game-engine-mcp` / `agent-gateway` 中 `sanguosha`（v1）入口；保留别名或迁移说明。
4. 迁移剩余门禁数据源：
   - `sgs-general-audit` / `sgs-batch-*` / `p67.rs` 断言 → 归档或改读 catalog JSON；
   - `check-sgs-skill-cases` / `check-sgs-skill-docs` 的 `skills.rs` 解析 → v2 注册表；
   - `check-sgs-pack-presets` 的 v1 `lib.rs`/`state.rs` 断言 → v2 大厅实现（注册后）。
5. 删除 `check-sgs-parity.mjs`（迁移期工具）与 `check:sgs-parity` 脚本项。
6. 全仓 grep 悬挂引用：`crates/engines/sanguosha`、`sanguosha/src/data.rs`、`sanguosha/src/skills.rs`。
7. 跑 `npm run sgs:check`（全量）+ `node apps/scripts/check-sgs-retire.mjs`（应输出「旧 crate 已删除」）。

## 3. 删除后可保留的历史证据

- `docs/games/sanguosha/spike-report.md`（6 类缺陷 + 迁移前置）
- `docs/games/sanguosha/scenes/`（v2 场景）
- `memory_palace/plan/next-engine-replacement.md`（M1–M4 记录）
- `docs/games/sanguosha/data/engine-catalog.json`（武将/牌名录唯一机器源）
