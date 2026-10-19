# 架构决策记录（ADR）

## 格式

文件名：`ADR-<编号>-<标题>.md`

内容包含：

- **背景**：为什么需要做决策
- **决策**：选择了什么
- **理由**：备选方案与取舍
- **影响**：对项目的影响
- **依据与验证状态**（决策依赖外部数值约束时必填）：每条数值约束标注**来源**（官方文档 / 实测）+ **验证状态**（已验证 / 待验证）；未经证实的结论不得作为「决策 / 理由」写死为实现约束。登记见 `docs/conventions/external-constraints.md`，规则见 `docs/conventions/coding-standards.md`。

## 记录

- [ADR-0001：Redis Streams 传输 + SQLite 真相源](./ADR-0001-redis-streams-sqlite.md)
- [ADR-0002：双模型 + MCP 化游戏引擎 + 事件溯源](./ADR-0002-dual-model-mcp-eventsourcing.md)
- [ADR-0003：web 公网可达 —— Cloudflare Tunnel 首选 + VPS 内网穿透兜底](./ADR-0003-web-public-access-tunnel-vps.md)
- [ADR-0004：游戏状态序列化](./ADR-0004-game-state-serialization.md)
- [ADR-0005：本地开发文档预览（独立 docs-mcp + 授权短链）](./ADR-0005-dev-docs-preview.md)
- [ADR-0006：时间感知与话题切换（上下文私有层 + 确定性门控）](./ADR-0006-time-awareness-topic-switch.md)
- [ADR-0007：会话隔离与全局人物画像层（三线内容隔离 + 日批蒸馏）](./ADR-0007-session-isolation-user-profile.md)
- [ADR-0008：引擎事件模型、RNG 注入与 lobby/视图原语](./ADR-0008-engine-events-rng-lobby.md)
- [ADR-0010：版图渲染、页内双模式与素材加工](./ADR-0010-board-rendering-dualmode-assets.md)
- [ADR-0011：移动端形态 —— PWA 优先、Tauri 2 壳次之，RN/Expo 延后](./ADR-0011-mobile-app-form-pwa-tauri2-vs-rn.md)
