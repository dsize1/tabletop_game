# command-router（指令路由）

> ⚠️ 已并入 `agent-gateway`（主持人）。本模块不再作为独立 crate 存在。

## 原职责

- 解析指令（前缀 / @ 命令）
- 按会话路由到对应游戏引擎
- 维护会话上下文

## 现归属

指令解析与路由由 **agent-gateway（Node, pi-sdk）** 承担：意图分类（决策模型）+ 会话编排取代了原确定性前缀解析。相关设计见：

- `docs/modules/agent-gateway.md`
- `docs/architecture.md` §6
