# 编码规范

## Node / TypeScript

见 `.pi/prompts/node-conventions.md`（`/node-conventions`）。

## Rust

见 `.pi/prompts/rust-conventions.md`（`/rust-conventions`）。

## QQ Bot

见 `.pi/prompts/qq-bot-conventions.md`（`/qq-bot`）。

## 通用

- 修改架构先更新 `docs/architecture.md`
- 重大决策记 ADR 到 `memory_palace/decisions/`

## 事实 vs 假设（外部约束）

- 进入 ADR / 架构文档的**数值型约束**（时间窗口、频控、延迟、上限、配额）必须标注**来源**（官方文档 / 实测记录）+ **验证状态**（已验证 / 待验证）。
- 未经证实的结论/理解**不得**作为「决策 / 理由」写死为实现约束；只能作为「待验证假设」显式标注。
- 外部约束事实登记表见 `docs/conventions/external-constraints.md`（按需加载，渐进披露）。
