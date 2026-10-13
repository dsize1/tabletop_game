# MCP 统一原语规范

> 状态：生效（2026-10-13）。适用于所有以 MCP（stdio）暴露统一原语的服务：当前 `game-engine-mcp`、`dev-task-mcp`。
> 目的：统一「原语定义 → 能力描述符 → 能力查询」三层写法，避免各服务各写各的。
> 关联：`docs/modules/game-engine.md`、`docs/modules/dev-task.md`、`docs/modules/agent-gateway.md`。

## 1. 三层概念

| 层 | 载体 | 说明 |
|---|---|---|
| **原语（tool）** | MCP `tools/*` | 确定性、受校验、不信任上游 |
| **能力描述符** | 服务内 `capabilities()` | 每服务的能力元数据自述 |
| **能力查询** | `list_capabilities()` | 只读原语，返回能力描述符数组 |

## 2. 原语（tool）约定

- `description` 必须以 `[读]` 或 `[写]` 前缀开头（声明读写属性）。
- 写入原语走完整校验链（合法性 → 权限 → 执行 → 事件/落盘），**无条件重校验**，不信任上游。
- 参数/返回结构稳定、机器可解析；出错以文本 `错误: ...` 返回，不 panic。
- **禁止用正则/硬编码对用户自由文本判断意图**（见 `docs/modules/dev-task.md` 约定）：用户意图只能经**显式命令前缀**或决策原语进入。
- 新增原语时，**同步更新能力描述符**（含 `example`）。

## 3. 能力描述符 schema（`list_capabilities()` 返回）

返回 JSON 数组，每项：

| 字段 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `name` | string | 是 | 原语名（与 MCP tool name 一致） |
| `rw` | `"读"` \| `"写"` | 是 | 读写属性（与 description 前缀一致） |
| `description` | string | 是 | 一句话说明（不含 `[读]/[写]` 前缀） |
| `usage` | string | 是 | 调用签名，如 `create_task(session_id, group_id, qq_id)` |
| `example` | string | **是** | **使用示例**：优先给用户侧（QQ）命令示例；无用户命令的原语写「（网关内部；…）」说明 |

> `example` 缺失视为不规范（2026-10-13 起新增要求：能力查询必须带使用示例）。

示例：

```json
{
  "name": "create_task",
  "rw": "写",
  "description": "创建开发任务（白名单校验）",
  "usage": "create_task(session_id, group_id, qq_id)",
  "example": "#发起 <需求>（#开发 为别名）"
}
```

## 4. QQ `#能力` 渲染格式

`apps/agent-gateway` 的 `formatCapabilities(json)` 渲染为：

```
开发任务 MCP 能力：
<name> [<rw>] <description>
  <usage>
  示例：<example>
...
```

- 每条固定最多三行：`描述` / `用法` / `示例`；`example` 缺失时省略第三行（向后兼容）。
- 渲染结果需控制在 QQ 单条 **1500 字符**内（超长走摘要/分条）。
- 能力查询的命令入口为 `#能力`（见 `docs/modules/dev-task.md` 命令表）。

## 5. 现状

| 服务 | `list_capabilities` | `example` | 备注 |
|---|---|---|---|
| `dev-task-mcp` | ✅（11 项：5 读 + 6 写） | ✅ | QQ `#能力`，约 1027 字符 |
| `game-engine-mcp` | ❌ | — | 如需对外自述能力，按本规范补齐 |
