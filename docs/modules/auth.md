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
