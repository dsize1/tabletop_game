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

## 时间与时区

- **不得依赖宿主本地时区**：容器默认 UTC。日历计算/格式化只允许两种写法——UTC 系列（`getUTCDay()` 等）或 `Intl.DateTimeFormat({ timeZone })`。本地时区 API（`getHours` / `getDate` / `toLocaleString` …）在源码中一律禁用，由 `apps/agent-gateway/test-time-guards.mts` 扫描护栏。
- **时区单点**：展示/注入模型的时区统一走 `apps/agent-gateway/src/time-context.ts` 的 `CHAT_TZ`（默认 `Asia/Shanghai`）；不要把时区字符串散落各处。
- **存储用绝对时间**：落库用 ISO 8601 UTC（`toISOString()`），进程内比较用 epoch ms（见 `docs/conventions/persistence.md`「时间戳约定」）。
- **时钟可注入**：任何依赖「当前时间」的函数都要接受可选 `now` / `opts.now`（默认 `Date.now()`），便于假时钟单测。不要在被调函数内部直接 `Date.now()` 后又无法覆盖。
- **内部时间信息不得出站**：用于模型推理的时间/边界标记（如 `[时间]`、`--- 话题切换 ---`）只存在于 LLM 上下文，断言工具 `assertNoInternalMarkers`（`apps/agent-gateway/test-helpers.mts`）。

## 事实 vs 假设（外部约束）

- 进入 ADR / 架构文档的**数值型约束**（时间窗口、频控、延迟、上限、配额）必须标注**来源**（官方文档 / 实测记录）+ **验证状态**（已验证 / 待验证）。
- 未经证实的结论/理解**不得**作为「决策 / 理由」写死为实现约束；只能作为「待验证假设」显式标注。
- 外部约束事实登记表见 `docs/conventions/external-constraints.md`（按需加载，渐进披露）。

## 开发门禁：主动消息（proactive）

> 状态：生效（2026-10-17，用户裁定；ADR-0009）。

- **默认禁用主动消息**（无 `reply_msg_id` 的 `pushActive` / 主动推送）；运行时被动回复超限默认**丢弃**（`PROACTIVE_MESSAGES_ENABLED=0`），不自动转主动。
- 任何**落地方案/计划**若包含主动消息，必须：① 在计划「待确定事项」列明用途与配额影响；② **经人工明确确认**；③ 在计划写入标记 `【主动消息已人工确认】`；④ 通过 `npm run check:proactive-gate`。
- 优先用**被动回复 + 合并消息/聚合**满足需求；不要把主动消息当作默认手段。
- 门禁依据数值（群 1000 条/天、60qpm、单关系 20qpm、每秒 ≤5）见 `docs/conventions/external-constraints.md`。
