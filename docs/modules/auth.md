# auth（鉴权）

- **语言**：Rust（lib）
- **位置**：`crates/auth`
- **HTTP API**：经 `server` 暴露（`crates/server/src/auth_routes.rs`）

## 职责

- 短期 token 签发 / 校验 / 撤销
- token 绑定玩家身份（openid = `member_openid`）+ 资源范围（防越权）

## 语义（已与用户确认）

- **存储**：Redis。key `auth:token:{token}`（JSON 记录），TTL 可配（默认 8 小时，`AUTH_TOKEN_TTL` 秒）
- **签发** `issue(openid, resource)`：uuid v4 token；签发时校验 openid 非空；同步维护 `auth:resource:{resource}` 索引（SET）
- **校验** `verify(token)`：优先查 Redis；不存在/过期/记录损坏 → `AuthError::Invalid`（HTTP 401）
- **撤销**：`revoke(token)` 单点失效；`revoke_resource(resource)` 按资源清空（**局终态**：游戏结束时清除该资源下全部 token）
- **资源范围**：`dev_task:{sessionId}` / `game:{gameId}` 等；校验时与请求资源比对，不一致 → 403

## HTTP API（server 暴露）

| 方法/路径 | 入参 | 出参 | 错误 |
|---|---|---|---|
| `POST /auth/issue` | `{openid, resource}` | `{token, expires_in_secs}` | 400（空 openid/resource） |
| `POST /auth/verify` | `{token}` | `{valid, openid, resource}` | 401（无效/过期） |

## 约定

- 个人资源请求必须携带有效 token，且资源范围与请求一致
- Web 不直连 SQLite，统一经 server 资源查询 API
- 穿越层（cloudflared）只解决「可达」，不替代应用层鉴权


## Web 跨端权鉴（ADR-0031，2026-11-01）

> 背景：QQ 扫码登录在可预见期内不可行（QQ 互联网站应用须工信部备案 + 审核「在线游戏应用暂不合作」；机器人群事件 `union_openid` 实测为空且需平台运营特殊配置）。故改为**系统自建跨端确认**。

### 身份模型

- 账号键 = **`(group_id, member_openid)`**（`member_openid` 是群内标识，跨群不通用）。
- 表 `web_accounts(group_id, openid, passcode_hash, passcode_algo, created_at, updated_at, last_login_at)`；注册码用 **Argon2id** 加盐哈希，**没有任何明文字段**。
- 注册码 = 6 位 `[0-9a-zA-Z]`（用户设定）；错误 **3 次锁定 30 分钟**（`auth:webfail:*` / `auth:weblock:*`），错误文案统一为「注册码不正确」。

### 跨端确认流程

1. 群内 `#开局`（建局）/`#开始`（开赛）回执列出**所有非机器人玩家**的登录页链接；`#网页` 可随时重取本人链接。链接 = `{PUBLIC_BASE_URL}/login?t=<短效token>`。
2. 登录页展示「群昵称 + 开局摘要」，判定该 openid 是否已注册：未注册 → 设置注册码（两次输入一致）；已注册 → 输入注册码。
3. 通过后反显 **4 位数字验证码** + 「一键复制 `@机器人 #验证码#NNNN`」。
4. 玩家在群内回复该命令；机器人用 **(group, code) → 会话** 反查并比对**回复人 openid**（R2）：不一致 → 只回「不是你的链接」并给回复人重签自己的链接（不泄露归属）。
5. 命中 → 登录页长轮询（`/login/confirm`）拿到结果，由 web 写 **HttpOnly Cookie**（`tg_game` / `tg_props`，8h）并跳转 `target` 路由。

**关键语义**：短效 token / 会话状态 / 4 位码 / 目标路由**同生命周期**（同一 Redis 记录、同一 TTL，10 分钟；阶段转换时续期）；确认换票**一次性**（`consumed`，二次访问返回 `used`）；注册码**确认成功后才落库**（R1，错链注册不会污染目标账号）；重置（`#重置注册码`）复用同一流程，确认后不跳转。

### HTTP API

| 方法/路径 | 调用方 | 说明 |
|---|---|---|
| `POST /internal/auth/web/link` | agent-gateway（本机） | 申请短效登录链接；`scope=props` 时校验中台白名单 |
| `POST /internal/auth/web/verify` | agent-gateway（本机） | 群内验证码判定：码 + 回复人 openid → `confirmed`/`mismatch`/`unknown` |
| `GET /auth/web/status` | web 登录页 | 阶段/昵称/开局摘要/是否已确认/锁定剩余秒 |
| `POST /auth/web/register` \| `/login` | web 登录页 | 设置注册码（暂存哈希）/ 校验注册码 → 生成 4 位码 |
| `GET /auth/web/confirm` | web 登录页 | 长轮询（默认 20s，`WEB_CONFIRM_POLL_SECS`）→ 一次性 8h token |
| `GET /auth/web/whoami` | web（中台准入） | 校验会话 token + 返回 `props_whitelisted` |

### 与既有 token 的关系

- 正式 token 仍是 `AuthService` 的 8h token（`game:{room_id}` / `props_admin:all`），只是**只经 Cookie 下发**；`?token=` 保留给内部脚手架（`sgs:web-room`、MCP 调试）。
- `auth` 新增模块 `weblogin`（Redis 键 `auth:weblogin:*` / `auth:webcode:*` / `auth:webfail:*` / `auth:weblock:*`）。
