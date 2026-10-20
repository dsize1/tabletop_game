# n8n（dev 线驱动层，shadow）

> 状态：**shadow（只读）**。ADR-0015 §2 / §7。n8n **不持有业务状态、不直连 SQLite、不存群消息正文**。

## 启动 / 停止

```bash
# 启动（profile，默认不启动；Redis 不受影响）
docker compose --profile n8n up -d n8n

# 健康
curl -s http://127.0.0.1:5678/healthz        # ok
# 容器内访问宿主机 server
docker exec tabletop-game-n8n wget -qO- http://host.docker.internal:3000/health

# 停止（不删卷）
docker compose --profile n8n stop n8n
docker compose --profile n8n down n8n         # 停止并删除容器（卷保留）
```

环境：`DEV_WEBHOOK_SECRET`（默认 `dev-secret-change-me`，**生产必改**）、`DEV_SERVER_URL`（默认 `http://host.docker.internal:3000`）。

> 必须开的两个 n8n 开关：
> - `N8N_BLOCK_ENV_ACCESS_IN_NODE=false`（表达式/Code 节点读 `$env`）
> - `NODE_FUNCTION_ALLOW_BUILTIN=crypto`（Code 节点 `require('crypto')` 做 HMAC）

**鉴权（HMAC + 重放窗口）**：`signature = HMAC-SHA256(secret, session_id + "." + timestamp)`，请求头：
`x-dev-signature`（hex）、`x-dev-timestamp`（epoch 秒，与服务端差 > 300s 拒绝）。

## 工作流：`dev-shadow`

文件：`deploy/n8n/workflows/dev-shadow.json`（8 节点，已实测可跑）。

```
Webhook(POST /webhook/dev-shadow)
  → Verify HMAC（签名校验 + 时间窗）
  → IF 签名通过 ──否──> Respond skip {shadow:true, reason}
        └─是→ GET $DEV_SERVER_URL/internal/dev_task/{session_id}
             → POST $DEV_SERVER_URL/internal/dev/permission {op:read, qq_id, session_id}
             → Decision（status==verifying && allow → would_run_ci）
             → Respond {shadow:true, status, permission, would_run_ci}
```

- **shadow**：只读 payload + 权限判定；**不推进状态、不发通知、不跑 CI**。
- 复用 ADR-0015 的 `commit` 语义留给 B9（CI/部署）。

## 导入 / 发布（CLI，n8n 2.44 实测流程）

```bash
# 1) 导入（MSYS_NO_PATHCONV 避免 Git Bash 把 /workflows 转成 Windows 路径）
MSYS_NO_PATHCONV=1 docker compose --profile n8n run --rm n8n import:workflow --input=/workflows/dev-shadow.json
# 2) 查 id
MSYS_NO_PATHCONV=1 docker exec tabletop-game-n8n n8n list:workflow
# 3) 发布 + 置 active（2.x 需要 publish；update:workflow 仅改标记）
MSYS_NO_PATHCONV=1 docker exec tabletop-game-n8n n8n publish:workflow --id=<id>
MSYS_NO_PATHCONV=1 docker exec tabletop-game-n8n n8n update:workflow --id=<id> --active=true
# 4) 重启生效
docker restart tabletop-game-n8n
```

**坑（实测）**：
- 重复导入会产生**同 path 的多个 workflow** → 后导入的注册失败（`UNIQUE constraint failed: webhook_entity.webhookPath`）。处理：先把旧 workflow `--active=false`（必要时重建 `n8n-data` 卷），再发布新的并重启。
- `n8n delete:workflow` 在 2.x 不存在（用 UI 删除或清卷）。

## 冒烟

```bash
SID=<verifying 状态的 session_id>
BODY="{\"session_id\":\"$SID\",\"qq_id\":\"<openid>\"}"
curl -s -X POST http://127.0.0.1:5678/webhook/dev-shadow \
  -H 'content-type: application/json' -H "x-dev-token: $DEV_WEBHOOK_TOKEN" -d "$BODY"
# {"shadow":true,"status":"verifying","permission":{"allow":true,"reason":null},"would_run_ci":true}

curl -s -X POST http://127.0.0.1:5678/webhook/dev-shadow \
  -H 'content-type: application/json' -H "x-dev-token: nope" -d "$BODY"
# {"shadow":true,"reason":"token_invalid"}
```

## 回滚 / 降级

- 停 n8n 即可：QQ 侧受理 / `#任务` / `#进度` / `#取消` 全部走 server + 桥，**不依赖 n8n**。
- 删除 `deploy/n8n`、`docker-compose.yml` 的 `n8n` 服务与 `n8n-data` 卷即完全移除。

## 加固清单

- [x] HMAC-SHA256（`session_id.timestamp`）+ 时间窗 ±300s（重放防护第一层）
- [ ] nonce/一次性签名（防窗口内重放；需要状态存储）
- [ ] 审批节点（`approve_plan` / `approve_deploy`）与 CI/部署工作流（B9）
- [ ] 密钥轮换流程
