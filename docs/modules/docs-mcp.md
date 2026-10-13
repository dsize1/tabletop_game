# docs-mcp（本地开发文档预览，MCP 形态）

- **语言**：Rust
- **位置**：`crates/docs-mcp`
- **配套**：`apps/agent-gateway/src/docs-client.ts`（MCP 客户端）、`crates/server`（资源读取）、`apps/web`（页面）

## 职责

- 只读 MCP 原语：白名单校验后返回覆盖**全部文档根**的授权资源 `docs:all`，token 由网关签发
- 实际的树/文件读取由 server 资源 API 完成（web 不直连文件系统，沿用 BFF 边界）
- **只浏览**：不支持搜索 / 编辑 / 上传（搜索为后续 TODO）

## 配置（集中一处）

`config/dev-docs.json`（可用 `DOCS_CONFIG` 覆盖），由 docs-mcp 与 server 共读：

```json
{
  "roots": [
    { "name": "docs", "path": "docs" },
    { "name": "memory_palace", "path": "memory_palace" },
    { "name": "prompts", "path": ".pi/prompts" },
    { "name": "skills", "path": ".pi/skills" }
  ],
  "exclude": ["memory_palace/artifacts"]
}
```

- `roots`：可浏览根（`name` = 展示/资源标识，`path` = 仓库根相对路径）
- `exclude`：排除目录（仓库根相对，前缀匹配），默认排除 `memory_palace/artifacts`
- 文件不存在 / 解析失败 → 回退内置默认（与上面默认一致）

## MCP 统一原语

| 工具 | 说明 |
|---|---|
| `prepare_docs_link(qq_id)` | 只读：`DEV_WHITELIST` 校验 → `{resource:"docs:all", roots:[...]}`（token 由网关签发） |
| `list_capabilities()` | 只读：能力描述符数组（`name/rw/description/usage/example`） |

读写属性标注在各 tool 的 `description` 前缀（`[读]`）。能力描述符 schema 见 `docs/conventions/mcp-primitives.md`。

## 链路

```
QQ #文档 → agent-gateway（白名单）→ docs-mcp.prepare_docs_link → 网关 auth.issueToken(openid, "docs:all")
        → 短链 https://{域名}/dev_docs?token=xxx
浏览器 → web /dev_docs        → server /resources/docs/tree → 文件树（<details> 折叠；仅 .md 可点）
       → web /dev_docs/file   → server /resources/docs/file → .md 内容（marked 渲染）
```

## 环境变量

| 变量 | 默认 | 说明 |
|---|---|---|
| `DEV_WHITELIST` | 空 | 开发白名单 `member_openid`，逗号分隔（空白名单 = 全部拒绝） |
| `DOCS_CONFIG` | `config/dev-docs.json` | 配置文件路径 |
| `DOCS_MCP` | `crates/target/debug/docs-mcp(.exe)` | 二进制路径（agent-gateway 使用） |

## 安全

- 只读；root 固定白名单（配置）；exclude 过滤
- server 侧路径规范化 + 前缀校验，拒绝 `..` / 绝对路径 / symlink 逃逸
- token 绑定资源 `docs:all`：无效/过期 401、越权 403；仅 `.md` 可预览（其它 415）
- 访问限开发者白名单；不做写 / 上传 / 删除

## 约定

- 能力描述符 schema、`example` 字段校验见 `docs/conventions/mcp-primitives.md`
- **TODO(search)**：后续如需文件名搜索 / 正文 grep，新增只读原语，仍走白名单
