# ADR-0035：测试玩家白名单 —— 中台仅「工作台」最小权限

- **状态**：已采纳（2026-11-03）
- **相关**：ADR-0031（Web 跨端登录/中台准入）、ADR-0033（工作台基座）、ADR-0034（沙盒创建对局）、`docs/modules/props-admin.md`、`apps/agent-gateway/.env.example`
- **结论一句话**：新增 `TEST_PLAYER_WHITELIST`；名单内 openid 只获得中台 **workbench** 权限（仅工作台菜单/工作台 API），页面直链与越权 API 均被服务端拒绝；DEV/PROPS 白名单仍是 Admin 全量权限。

## 背景

1. 工作台需要给「测试玩家」使用（创建对局、预览、AI 调试、看决策日志），但不希望他们看到/操作道具中台的其他菜单（道具包、武将、牌表、导入、生命周期等）。
2. 仅靠前端隐藏菜单不够：手动改地址栏或直接调内部 API 仍会越权，必须在 web 与 server 两侧同时收口。
3. 现有 `DEV_WHITELIST` / `PROPS_ADMIN_WHITELIST` 都是全量中台权限；需要一个更小的角色。

## 决策

### 1. 三个白名单与优先级

| 环境变量 | 角色 | 权限 |
|---|---|---|
| `DEV_WHITELIST` | 开发/运维 | 中台 **Admin**（全部菜单/API）+ 原有开发入口 |
| `PROPS_ADMIN_WHITELIST` | 中台管理员 | 中台 **Admin**（与 DEV 并集；未设置时回落 DEV） |
| `TEST_PLAYER_WHITELIST` | 测试玩家 | 中台 **Workbench**（仅工作台） |

- 同一 openid 命中多个名单时 **Admin 优先**。
- `props_scope(openid)`：`Admin` / `Workbench` / `None`。
- 中台登录（`scope=props`）允许 Admin 或 Workbench；Workbench 的确认跳转被服务端强制规范为 `/props-admin/workbench`（即使链接 target 是 `/props-admin/`）。

### 2. Web 层（apps/web）

- `propsSession` 仍要求有效会话 + `props_scope != null`（不再只判 `props_whitelisted`）。
- **路径门禁**（防手动改地址栏）：
  - Admin：`/props-admin/**` 全部放行。
  - Workbench：仅放行 `/props-admin/workbench`、`/props-admin/assets/**`、`/props-admin/index.html`、`/props-admin/favicon.ico`；其余路径返回 **401**（含 `/props-admin/`、`/props-admin/packs` 等）。
  - 静态资源必须先于 SPA fallback 判定，避免工作台页面加载不到 JS/CSS。
- `/internal/gameprops/*`、`/internal/game/preview/*` 仍只经 web 代理（浏览器不直连 server）。

### 3. Server 层（纵深）

- `web_auth_routes::props_scope` 统一解析角色；`whoami` 返回 `props_scope` / `props_menus`。
- 共享 token 与 actor 的优先级改为 **actor first**：
  - 有 `x-props-actor` 时只按 actor 的角色判定（即使 web 代理同时带了 `x-props-admin-token`）；
  - 无 actor 时才走共享 token / 本机开发放行（脚本/内部调用）。
  - 这修复了「代理带内部 token 会让 Workbench 账号拿到 Admin」的越权面。
- API 分层：
  - `require_admin`：仅 Admin（道具包/武将/牌表/导入/概览/生命周期等全部非工作台接口）。
  - `require_workbench`：Admin 或 Workbench（模板 CRUD、预览、沙盒、决策日志只读）。
  - `GET /internal/gameprops/session`：返回当前 `{openid, scope, menus}`，供 SPA 裁剪菜单与路由守卫。
- 越权 API 返回 **403**；页面路径无权限返回 **401**（按用户要求）。

### 4. Agent Gateway

- `#中台` 同时允许 `DEV_WHITELIST` 与 `TEST_PLAYER_WHITELIST`。
- 登录链接 target：DEV → `/props-admin/`；测试玩家 → `/props-admin/workbench`。
- `dev-up` 把 `TEST_PLAYER_WHITELIST` 从 `apps/agent-gateway/.env` 传给 server；`.env.example` 增加该键。

### 5. 前端 SPA

- 启动时调 `/internal/gameprops/session`：
  - `scope=workbench` → 侧栏只渲染「工作台」；若当前路由不是 `/workbench` 自动替换；隐藏 Admin Token 输入。
  - `scope=admin` → 全部菜单（行为不变）。
- 前端裁剪只是 UX；服务端路径/API 门禁才是安全边界。

## 影响

- 新增 `TEST_PLAYER_WHITELIST`（server / agent-gateway / dev-up / .env.example）。
- `whoami` 响应新增 `props_scope` / `props_menus`；web `Whoami` 类型同步。
- `require_admin` 的 actor-first 语义变化：带 actor 的内部调用不再能用共享 token 绕过 actor 权限；无 actor 的脚本/本机调用不变。
- Workbench 账号可以使用工作台的模板 CRUD/预览/沙盒/决策日志；不能访问中台其他菜单与 API。
- Q 群对局流程不受影响。

## 依据与验证状态

- `check:sgs-preview --ui`：测试玩家 `scope=workbench`、菜单仅工作台、`/props-admin/workbench` 200、`/props-admin/` 与 `/props-admin/packs` 401、概览 API 403（**已验证**）。
- `check:props-boundary`：静态检查三个白名单/路径门禁/ADR 存在（**已验证**）。
- `sgs:check` 19/19（Q 流程回归）。
- 数值/名单均来自 env，无新增外部约束。
