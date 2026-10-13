# ADR-0005：本地开发文档预览（独立 docs-mcp + 授权短链）

- 状态：已采纳（Accepted）
- 日期：2026-10-13

## 背景

开发过程中需要随时查看本地开发文档（`docs/`、`memory_palace/`、`.pi/prompts/`、`.pi/skills/`）。
现有 web 只支持单个 dev-task 计划文档页。需要一个「查询能力返回授权链接、页内文件树可折叠、点文件看内容」的浏览系统。

## 决策

1. **独立 `docs-mcp`**（Rust stdio），不改 `dev-task-mcp`：职责单一，未来 game 侧可复用。
2. **单 token 覆盖全部根**：资源 `docs:all`，一次授权浏览全部配置根。
3. **配置集中一处** `config/dev-docs.json`（`roots` + `exclude`），docs-mcp 与 server 共读（读取模块放 `crates/shared`）。
4. **树/文件读取在 server**：web 不直连文件系统，沿用 BFF（`/resources/docs/tree`、`/resources/docs/file`）。
5. **零 JS 交互**：树用原生 `<details>/<summary>` 折叠；`.md` 用相对 `<a>` 链接导航（拼接当前链接）。
6. **仅 `.md` 可预览**，其它类型只显示名称；exclude 目录不显示；不做搜索（在 docs-mcp 备注 TODO）。

## 理由

- 与既有 dev_task 短链链路同构（MCP 校验 → 网关签发 token → web 校验 → server 读资源），复用 auth 鉴权与 BFF 边界。
- `<details>` + `<a>` 无需前端框架/JS，移动端可用，开发量最小（工作量中低，约 3.5–4 人日）。
- 单 token 简化心智，避免每根多次签发；token 仍为短期（TTL 8h，沿用 `auth` 默认）。

## 影响

- 新增 crate `docs-mcp`、`config/dev-docs.json`、server `docs_routes`、web `/dev_docs` 两路由、agent-gateway `#文档` 命令 + `docs-client.ts`。
- 安全面为「只读 + 白名单 + 路径校验 + 仅 .md」；不做写 / 上传 / 删除。
- 后续搜索、非 md 预览、语法高亮等按需增补（搜索已在 docs-mcp 备注 TODO）。

## 依据与验证状态

- 本文决策不依赖新的外部数值约束；token TTL（8h）沿用 `docs/architecture.md` §10 / `crates/auth` 既有约定（`AUTH_TOKEN_TTL` 可配）。
