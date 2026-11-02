# 三国杀运维手册（P8）

> 适用范围：本地/灰度环境的 server + web + agent-gateway + MCP。所有内部接口只监听 `127.0.0.1`，
> 不挂公网；公网请求统一经 web（短 token / 跨端登录）进入。

## 1. 服务与健康

| 组件 | 地址 | 健康检查 |
|---|---|---|
| server（Rust，单写者） | `127.0.0.1:3000` | `GET /health`（redis/sqlite） |
| web（Express BFF） | `127.0.0.1:18080` | `GET /health` |
| 指标 | `127.0.0.1:3000/internal/metrics` | JSON 计数器 |
| 决策服务（可选） | `127.0.0.1:8090` / llama `:8081` | 仅 P5/AI 对照，不参与走子 |
| agent-gateway / qq-gateway | Node（dev-up 拉起） | 日志 `[agent-gateway] 就绪` / `[qq] 出站消费者就绪` |

一键：`npm run dev:up`（前台）；停止：`npm run dev:down`（按 `data/run/dev-up.pid` 停整棵进程树）。
**不要**单独 `taskkill server.exe`（dev-up 会连带停掉整条链）。

## 2. 指标（`/internal/metrics`）

| 字段 | 含义 |
|---|---|
| `actions_total` / `actions_denied` | Web 动作总数 / 被引擎拒绝数 |
| `games_started` / `games_finished` | `lobby:start` 次数 / 终局次数 |
| `bot_steps` / `drives` | 机器人动作数 / 推进次数（含后台链式续推） |
| `decision_logs` / `decision_log_failures` | 决策日志写入成功/失败 |

日志关键字：`机器人推进 room=... steps=... stop=... illegal=...`、`机器人动作被拒`、
`写 decision log 失败`。结构化日志带 `room_id/seq/actor`（MCP 与 server 各自输出）。

## 3. 常见故障排查

### 3.1 页面停在「机器人行动中…」

1. 看 `bot_stop`：`human_turn` 正常；`max_steps` 会后台续推；`loop`/`engine_error`/`no_legal_actions` 需排障。
2. 手动续推：`POST http://127.0.0.1:3000/internal/game/sanguosha/<room>/drive`（本机）。
3. 看该房间决策日志最后几条：
   ```bash
   npm run sgs:export-decisions -- --room <room> --limit 20 | tail -5
   ```
4. 若 `illegal` 动作反复出现：记录 `chosen/reason`，用 `npm run sgs:replay -- --room <room>` 复现；
   属引擎合法集/结算不一致 → 开 B 系列回归用例。
5. 全 AI 局：server 到 200 步上限会自动后台链式续推（≤3000 步/局）；web 轮询发现存活者全 bot 时也会 `/drive` 兜底。

### 3.2 房间状态落后于 DB（`#开始` 后页面仍选将）

server `ensure_room` / MCP `sync_room` 按 `game_state.version` 自动 restore；若仍异常：
```bash
sqlite3 data/tabletop.db "select room_id,version,updated_at from game_state where room_id='<room>';"
```
重启 server/MCP（`npm run dev:down && npm run dev:up`）。不要双进程同时写同一房间。

### 3.3 卡在某 pending（choice/target/discard…）

- `#局面` 看等待者；Web 个人区看 pending 选项。
- 若确认是残留帧/交互不收敛：先 `drive`，再 `replay`；仍卡则 `lobby:abort` 中断本局并记录
  `room_id + seq + pending.kind/options + 最后历史`，补回归用例后重开。

### 3.4 决策日志/回放

```bash
npm run sgs:export-decisions -- --room <room> --out /tmp/decisions.jsonl
npm run sgs:replay -- --room <room> --from-seq 0 --json
```
- `mismatches=0` = 重放一致；旧局无 `game_replay_base` 时只打印时间线。
- 关日志：`SGS_DECISION_LOG_RATE=0`（不影响对局）；采样 `0~1`。

## 4. 灰度/回滚（无白名单）

- **不做群/openid 灰度白名单**：加入本局座位的群成员即可操作自己的回合；
  房主 / 群管理（QQ `member_role=owner/admin`）可执行开局管理动作（开始/中止/踢人/加机器人/武将包）。
- 紧急回滚：`SGS_ROLLOUT=off`（或 `SGS_ENABLED=0`）→ 网关拒绝新开三国杀；已完成/进行中对局可继续打完或 `#中断`。
- 中断：群内房主/群管理发 `#中断`（`lobby:abort`，不结算）；恢复：`#恢复` 只重签登录链接，不改状态。
- **主动消息**：默认禁用；本方案全部为被动回复（`reply_msg_id`）。任何超时/提醒类主动消息必须先人工确认，
  写入 `【主动消息已人工确认】` 并跑 `npm run check:proactive-gate`。

## 5. 权鉴边界

| 入口 | 校验 |
|---|---|
| Web 页面 `/tabletop_game/:gameId/player/:openid` | 短 token / 8h Cookie；资源范围 `game:{room}` |
| Web 动作 `/tabletop_game/:gameId/action` | token openid = 动作 player_id；跨房间 403；引擎再校验回合/座位 |
| `/internal/game/*` | **仅 127.0.0.1**，公网不可达 |
| 中台/预览（P5） | `props_admin` 会话 + 白名单；不直连 server |

负向用例：跨房间 token → 403；非当前行动者 → 403/400；已阵亡/非本局座位 → 403。
真机异常先看 `actions_denied` 与 web 日志 `[web] 提交动作失败`。

## 6. 验收/回归命令

```bash
cargo test -p sanguosha -p storage -p server -p game-engine-mcp
npm run test:agent-gateway
npm run sgs:check            # 17/17（含 e2e/机器人/登录）
node apps/verify-sanguosha.mjs   # 5 人全流程 + 8 人 lobby/选将/发牌/视图隔离
npm run check:props-boundary     # 中台/内部接口不公网
npm run check:proactive-gate     # 无主动消息
```
