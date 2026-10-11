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
