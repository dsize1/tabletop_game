# auth（鉴权）

- **语言**：Rust（lib）
- **位置**：`crates/auth`

## 职责

- 玩家身份与会话
- 短期 token 签发 / 校验
- 个人资源归属校验（防越权）

## 约定

- 通过 `server` 暴露 HTTP API
- 个人资源请求必须携带有效 token，且归属一致
- Web 不直连 SQLite，统一经资源查询 API
