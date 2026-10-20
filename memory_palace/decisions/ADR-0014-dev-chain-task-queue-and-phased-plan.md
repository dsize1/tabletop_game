# ADR-0014：dev 消息链路改造 —— 任务入口化、可恢复执行与分阶段实现计划

- **状态**：提案（Proposed，2026-10-20）——**技术探讨稿，未采纳、不改代码、不建开发计划**
- **来源文档**：《n8n + LangGraph + DeepAgents 架构设计与 dev 消息链路改造方案》（DOCX，10 节）+《n8n + LangGraph + DeepAgents 架构设计》（PDF）
- **相关**：ADR-0013（分层与协议边界，原则篇）、ADR-0001（Streams + SQLite）、ADR-0002（MCP 化）、ADR-0009（主动消息门禁）、ADR-0012（端点交互冻结）、`docs/architecture.md`、`docs/modules/dev-task.md`、`docs/modules/agent-gateway.md`、`docs/conventions/persistence.md`
- **主动消息**：本 ADR **不新增**主动消息场景；审批/结果沿用现有 dev 链路口径（`pushReply` 被动 + 既有 `pushActive` 计划/结果推送）。若后续要把审批提醒改为新的主动推送，须先走 ADR-0009 门禁并人工确认。

> 修订（2026-10-20）：用户补充口径「原 Rust 引擎可废弃」+「消息驱动/消费与状态管理分别归 n8n / LangGraph」（ADR-0015）—— 本文 **P0 的「保留 Rust 状态机 + 先补队列/租约」不再是目标形态**；但仍继承其结论：`dev_task_events` / `dev_runs` / 投影与租约是切换前必补项（否则无审计、无执行者、无对账）。另：n8n 未来承接 Git/CI/测试/部署属 **P3+**，不得与 P0/P1 捆绑（ADR-0015 §2.5）。

## 结论一句话

文档的「最终调用链」在本项目**已有 80% 落点**：真正缺的只有 **①任务级队列 + 租约 + 重试**、**②执行与消息消费解耦**、**③执行隔离**、**④外部触发/审批 UI**。前三条用现状栈（Node + Rust + Redis + SQLite）分步做（P0）；LangGraph/DeepAgents（P1）与 n8n（P2）**按触发条件引入**，A2A（P3）暂不建。

---

## 1. 背景与现状事实

### 1.1 现状链路（代码事实）

```
QQ 群 → qq-gateway → qq.raw → bot-adapter → internal(Redis Stream, 消费者组)
  → agent-gateway:handle()（apps/agent-gateway/src/index.ts）
      ├─ parseDevCommand（固定前缀；人=100% 置信度）
      ├─ handleDevCreate → devTask.createTask → transition planning
      │     └─ pushReply 受理回执 → void runDevCreate(...)   ← fire-and-forget
      ├─ handleDevConfirm → devTask.confirmTask → void runDevResume(...)
      └─ handleDevAppend  → append_instruction → void runDevResume(...)
  → runDevCreate / runDevResume（src/index.ts:680 / :722）
      └─ devSessions.withLock(sessionId) → pi-sdk 子会话 prompt → 状态迁移 → pushActive
```

关键实现点：

| 事实 | 位置 |
|---|---|
| 子会话落盘 `data/dev-sessions/{sessionId}`，`resume()` 按 id 重新打开 | `src/dev-session.ts` |
| 任务状态机 `queued→planning→awaiting_confirm→running→verifying→done` + 确定性迁移表 | `crates/dev-task/src/lib.rs` |
| `dev_tasks` 表只有终态字段（`session_id/group_id/qq_id/title/plan_path/status/created_at/updated_at`） | `crates/storage/src/lib.rs` |
| 消费循环：`handle()` 返回即 `XACK` | `src/index.ts:processEntries` |
| `sessionId` = pi 子会话 id = 计划文档 id = `dev_tasks.session_id` = 未来 web/A2A 标识 | `docs/modules/dev-task.md`「统一键 sessionId」 |
| dev 执行使用宿主 `bash`/`edit`，cwd = `PROJECT_ROOT`（直接改工作区） | `src/dev-session.ts`、`.pi/mcp.json` |

### 1.2 六个真实缺口（不依赖任何新栈就能确认）

| 编号 | 缺口 | 证据 | 后果 |
|---|---|---|---|
| **G-a** | 执行是 fire-and-forget：`void runDevCreate/runDevResume`，而 `handle()` 返回即 `XACK` | 代码（§1.1） | 进程崩溃/重启 → 执行权丢失，任务永久停在 `planning`/`running`，只能人工 `#取消` |
| **G-b** | 执行与消息消费同进程 | 同上 | 长任务与 `internal` 消费互相拖累；重启即全断；无法多 worker |
| **G-c** | 无任务级重试/租约/心跳 | 无相关表与字段 | 瞬时失败（LLM 超时、MCP 断连）直接 `failed`，需人工重开 |
| **G-d** | dev 执行无隔离（宿主 shell + 直接改 `PROJECT_ROOT`） | `src/dev-session.ts` | 一次错误 `rm`/错误分支即可污染工作区；与「文档 §6 安全边界」直接冲突 |
| **G-e** | 审计粒度不足：只有 `dev_tasks.status` 终态，无事件流水（谁在何时批准/重试） | `crates/storage/src/lib.rs`（无 events 表） | 事故复盘只能靠日志文本；无法回答「是谁批准的」 |
| **G-f** | 无外部触发（Issue/PR/定时/CI），只能群内发起 | 命令入口只有 `parseDevCommand` | dev 任务无法由代码事件驱动 |

> 注意：G-d 是**现状最大风险**，且与「是否用 n8n/LangGraph」**完全无关**——换栈不会自动解决，必须单独做。

### 1.3 文档主张与现状的差异（逐条对照）

| 文档要求 | 现状 | 差异性质 |
|---|---|---|
| 消费服务只做「识别 dev / 落库入队 / 触发」三件事 | agent-gateway 还兼执行 | 职责混杂（G-b） |
| 任务队列负责削峰、重试、顺序消费 | Redis Stream + 消费者组，但无任务级队列/重试 | 弱（G-c） |
| 持久化负责状态与审计 | 只有 `dev_tasks` 终态 | 审计缺失（G-e） |
| n8n 触发/审批/通知，不持有状态 | 无 n8n；命令与推送在网关 | 缺外部触发（G-f） |
| LangGraph：状态机 + checkpointer + interrupt | Rust 迁移表 + 会话文件 + `#确认` | 语义等价，耐久性弱 |
| DeepAgents：规划/子 Agent/摘要/MCP | pi-sdk + plan-workflow + MCP | 能力等价，隔离弱 |
| A2A 跨 Agent | 无需求 | 不适用 |
| 高危操作必须人工门禁 | `#确认` 已是门禁 | 已满足 |

---

## 2. 决策

### D1 消费服务「入口化」：识别 → 落库 → 入队 → 回执，不再执行

- `handleDevCreate` 只做：白名单校验 → `create_task` → `transition planning` → 入队 `dev.jobs` → **被动回执（含 sessionId）**。
- 执行搬到独立进程 `apps/dev-worker`（P0），与 `internal` 消费解耦。
- 对应文档 §3.2「消费服务只需完成三件事」，但**第三件事不是 n8n Webhook，而是 Redis Stream 入队**（P2 若引入 n8n，再加一条「可选触发」旁路）。

### D2 任务级队列：Redis Stream 传输 + SQLite 真相

- 传输：新 Stream `dev.jobs`（`MAXLEN ~ 1000`，消费者组 `dev-worker`），消息体只带 `{job_id, session_id, stage}`，**不带消息正文**。
- 真相：SQLite 新增 `dev_jobs`（租约/尝试次数/最后错误）+ `dev_task_events`（append-only 审计）。
- **ACK 语义改变**：`dev.jobs` 只在「阶段真正完成并落库」后 `XACK`（现状是 `handle()` 返回即 ACK）。
- 幂等：`job_id` 由 `(session_id, stage, attempt_epoch)` 派生或 uuid + `UNIQUE`；重复投递由 `dev_jobs.status` 阻塞（`leased`/`done` 直接跳过）。

### D3 租约 + 心跳 + 有限重试（等价于最小 checkpointer）

| 参数 | 默认 | 说明 |
|---|---|---|
| `DEV_JOB_LEASE_SEC` | 120 | 租约时长；worker 必须在此期间心跳 |
| `DEV_JOB_HEARTBEAT_SEC` | 30 | 心跳间隔 |
| `DEV_JOB_MAX_ATTEMPTS` | 3 | 超过 → `failed` + 事件 `lease_expired` 告警 |
| `DEV_JOB_REAPER_SEC` | 60 | 过期租约的回收扫描周期 |

- 原子领取：`UPDATE dev_jobs SET status='leased', lease_owner=?, lease_until=? WHERE job_id=? AND status='queued' AND (lease_until IS NULL OR lease_until < now)`，受影响行数 = 1 才算领到。
- 过期回收：`status='leased' AND lease_until < now` → `attempts+1` → 回 `queued` 并重新 `XADD`。
- 恢复语义：worker 领取后若 `stage=execute`，用 `devSessions.resume(sessionId)` 续跑（子会话文件已在盘上），**不重跑规划**。

### D4 审批 = interrupt/resume，权威仍在 Rust

- `#确认 <id>` → `confirm_task`（Rust 判定迁移合法性）→ 记录 `approved` 事件 → 入队 `stage=execute`。
- 拒绝/取消 → `cancel_task` → 记录 `rejected`/`cancelled` 事件。
- LangGraph（P1 若引入）只能**请求**迁移：节点内调用 MCP `transition_task`，不得直连 `tabletop.db`（ADR-0013 D3/D6）。
- 文档 §4 注（`interrupt()` 恢复会从节点开头重跑）→ 对应约束：**副作用必须在中断点之后，或用 `job_id`/worktree 幂等**。

### D5 执行隔离（P0 之后立刻做，优先级高于 P1/P2）

- 每个 dev 任务在 `git worktree` 中执行（`worktrees/{sessionId}`，分支 `dev/{sessionId}`），不再直接改 `PROJECT_ROOT`。
- 合并回主干 = 高危操作 → 人工（现状 `#确认` 验收即可作为合并门禁）。
- 容器化（禁网/白名单、只挂 worktree、不挂 `docker.sock`）作为后续强化项，见 TD-13.5。

### D6 LangGraph + DeepAgents（P1）：只做「执行层替换」，不接管状态权威

- 形态：Python worker（`agents/dev_executor/`），LangGraph Server，`thread_id = sessionId`。
- 图：`plan → interrupt(approval) → execute → interrupt(verify) → summarize`。
- Checkpointer 独立存储 `data/langgraph/checkpoints.sqlite`，**不与 `tabletop.db` 同库**（避开单写者约束）。
- 工具：`langchain-mcp-adapters` 以 stdio 起 `dev-task-mcp` / `docs-mcp`；不接 `game-engine-mcp`。
- 文件系统：worktree 挂载 + filesystem backend；**禁用 `LocalShellBackend` 类宿主 shell**。
- 开关：`DEV_EXECUTOR=pi|langgraph`（硬隔离，禁止同一 `sessionId` 双驱动）。

### D7 n8n（P2）：只做触发/审批呈现/通知/集成

- 部署：`docker-compose.yml` 新增 `n8n` 服务，`profiles: ["n8n"]`（默认不启动）。
- 最小工作流：`Webhook(HMAC) → HTTP GET /internal/dev_task/{id}（只读）→ HTTP POST LangGraph run`。
- 审批：n8n Wait/表单 → 回调 server `/internal/dev_task/{id}/approve`（网关鉴权）→ 复用 D4 的 resume 通道。
- **禁止**：n8n 直写 SQLite、n8n 管理 checkpoint、n8n 存完整群消息正文。
- **降级验证**：停掉 n8n，QQ 内主流程必须仍可用（否则设计不合格）。

### D8 不新增主动消息，A2A 不做

- 通知沿用现有 `pushReply`/既有 `pushActive`；新增主动推送场景须走 ADR-0009。
- A2A 待 T3（ADR-0013 §6）成立再议；当前 `sessionId` 已具备未来 `task_id` 语义。

---

## 3. 目标链路（改造后，P0 完成态）

```
QQ 群 → qq-gateway → qq.raw → bot-adapter → internal
   → agent-gateway（只做入口）
        ├─ 识别（固定前缀）→ 白名单
        ├─ dev_tasks + dev_task_events 落库
        ├─ XADD dev.jobs {job_id, session_id, stage}
        └─ pushReply 受理回执（同步，<1s）
   → dev-worker（独立进程）
        ├─ XREADGROUP claim（租约）
        ├─ pi 子会话（worktree 内）执行 plan/execute/verify
        ├─ heartbeat / finish（→ Rust 状态迁移 + 事件）
        ├─ 终态：pushActive 结果（既有口径）
        └─ 失败/超租约 → 重投（attempts ≤ 3）
   → Rust dev-task（权威状态机 + 审计）
```

### 3.1 数据模型（P0 新增）

```sql
-- 任务事件（append-only 审计；不含消息正文，payload 存脱敏摘要）
CREATE TABLE IF NOT EXISTS dev_task_events (
    id         INTEGER PRIMARY KEY,
    event_id   TEXT NOT NULL UNIQUE,
    session_id TEXT NOT NULL,
    kind       TEXT NOT NULL,  -- created|plan_ready|approval_requested|approved|rejected
                               -- |resumed|executed|verified|failed|cancelled|lease_expired
    actor      TEXT,           -- openid | worker:<id> | system
    payload    TEXT,           -- JSON
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_dev_task_events_session ON dev_task_events(session_id, id);

-- 任务级作业（队列真相 + 租约）
CREATE TABLE IF NOT EXISTS dev_jobs (
    job_id      TEXT PRIMARY KEY,
    session_id  TEXT NOT NULL,
    stage       TEXT NOT NULL,  -- plan|execute|verify
    status      TEXT NOT NULL,  -- queued|leased|done|failed
    attempts    INTEGER NOT NULL DEFAULT 0,
    lease_owner TEXT,
    lease_until TEXT,
    last_error  TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_dev_jobs_status ON dev_jobs(status, lease_until);
```

> 表放 `crates/storage`（与 `dev_tasks` 同库）；写入路径收敛在 Rust（`crates/dev-task` + `dev-task-mcp`），Node 侧不直连 SQLite（延续 ADR-0001 单写者）。

### 3.2 新增/变更的 MCP 原语（运维面，非模型工具）

| 原语 | 读写 | 说明 | 校验 |
|---|---|---|---|
| `claim_job(worker_id, lease_sec)` | 写 | 原子领取一个 `queued` 作业 | `worker_id` 非空；租约参数范围 |
| `heartbeat_job(job_id, worker_id, lease_sec)` | 写 | 续租；非持有者调用报错 | 持有者校验 |
| `finish_job(job_id, worker_id, ok, error?)` | 写 | 完成/失败收尾（含状态迁移与事件） | 持有者 + 迁移表 |

- 按 `docs/conventions/mcp-primitives.md`：`description` 带 `[写]` 前缀；能力描述符的 `example` 写「（网关内部；由 dev-worker 调用，不对用户暴露）」，**不进 `#能力` 渲染**（`formatCapabilities` 过滤 internal 标记）。

---

## 4. 实现计划（分阶段，可独立回滚）

> 所有阶段均为**探讨性排期**，不建开发计划；工作量仅作量级参考（以「一人 + coding agent」估）。

### 4.1 P0：入口化 + 队列 + 租约（必做；不需要新栈）

**前置**：无（Redis/SQLite 已有）。

| # | 改动点 | 文件 |
|---|---|---|
| 1 | 新增两张表 + DAO | `crates/storage/src/lib.rs` |
| 2 | `JobManager`（claim/heartbeat/finish/reaper）+ 单测 | `crates/dev-task/src/lib.rs` |
| 3 | 3 个运维原语 + 能力描述符 | `crates/dev-task-mcp/src/main.rs` |
| 4 | 入口化：`handleDevCreate/Confirm/Append` 改为「落库 + 入队 + 回执」；保留 `DEV_INLINE_EXEC=1` 旧路径 | `apps/agent-gateway/src/index.ts` |
| 5 | 新进程 `dev-worker`：消费 `dev.jobs`、心跳、串行锁、终态推送（复用 `DevSessionManager`） | `apps/dev-worker/src/*`（新） |
| 6 | 启动编排：先 build，再拉起 dev-worker | `apps/scripts/dev-up.mjs` |
| 7 | 文档同步：架构 §4/§5、`modules/dev-task.md`、`modules/agent-gateway.md`（新增 `modules/dev-worker.md`） | `docs/` |

**时序（plan 阶段）**

```
gateway: create_task → planning → event(created) → XADD dev.jobs(plan) → pushReply 回执
worker : claim → lease(120s) → spawn pi session → 写计划 → update_task_meta
         → transition awaiting_confirm → event(plan_ready) → XADD dev.jobs? (无，等人工)
         → pushActive 计划短链 → finish_job(ok)
gateway: #确认 → confirm_task(running) → event(approved) → XADD dev.jobs(execute)
worker : claim → resume pi session → 执行 → transition verifying → event(executed) → pushActive
gateway: #确认 → confirm_task(done) → event(verified)
```

**验证**

- 单测：两 worker 抢同一 job 只有一个成功；心跳续租；租约过期重投；`attempts=3` 后 `failed`。
- 故障注入：worker 执行中 `kill -9` → ≤ `LEASE+REAPER`（≈180s）内被重投并 `resume` 续跑，**规划不重跑**。
- 入口回归：gateway 重启不影响 `leased` 中作业（由 worker 持有租约）。
- 端到端：`#发起` → 计划短链 → `#确认` → 执行 → `#确认` → `done`；`#进度` / `#补充` 正常。
- 全量：`npm test`、`cargo test --workspace`、`node apps/scripts/status.mjs`。

**回滚**：`DEV_QUEUE_ENABLED=0` → 走 `DEV_INLINE_EXEC` 旧路径（代码保留至 P1 决策）。

**工作量**：≈ 2–4 天（含测试与故障注入）。

### 4.2 P0.5：执行隔离（必做，可与 P0 并行）

| # | 改动 | 说明 |
|---|---|---|
| 1 | `git worktree add worktrees/{sessionId} -b dev/{sessionId}` | worker 领取 `plan/execute` 作业时按需创建/复用 |
| 2 | 子会话 cwd 指向 worktree | `dev-session.ts` 增加 `cwd` 参数（默认 `PROJECT_ROOT` 保持不变以兼容 chat/宿主工具） |
| 3 | 合并门禁 | 验收 `#确认` 后才允许合并；合并动作本身不由 Agent 执行 |

**验证**：任务执行期间 `git status` 在主干无改动；worktree 清理策略（保留 N 天）。
**回滚**：`DEV_WORKTREE=0`。
**工作量**：≈ 1–2 天。

### 4.3 P1：LangGraph + DeepAgents 执行层（条件触发，可选）

**准入条件（全部满足才做）**：T2 在 P0 后仍不满足（需要多机/并行/更细粒度中断）+ TD-13.2 评估通过（plan-workflow 在 Python 侧重实现成本可接受）+ 沙箱方案已定（P0.5 完成）。

| # | 改动 | 说明 |
|---|---|---|
| 1 | `agents/dev_executor/`（Python + uv，`pyproject.toml` 锁版本） | 图定义、状态 schema |
| 2 | LangGraph Server + checkpointer | `data/langgraph/checkpoints.sqlite`，`thread_id=sessionId` |
| 3 | 节点内状态迁移走 MCP | `transition_task`；不直连 SQLite |
| 4 | `langchain-mcp-adapters` 起 stdio MCP | `dev-task-mcp`、`docs-mcp` |
| 5 | DeepAgents harness | `write_todos` 对应计划 6 分区；禁用宿主 shell backend |
| 6 | worker 变调度器 | `DEV_EXECUTOR=langgraph` → HTTP `/threads/{id}/runs` |
| 7 | 幂等 | 中断点后置副作用；`job_id` 去重；worktree 复用 |

**验证**：同一 `thread_id` 中断→恢复；`interrupt` 前副作用不重复（实测文档 §4 注）；拒绝路径；`recursion_limit`/超时；token 计量。
**回滚**：`DEV_EXECUTOR=pi` 秒切。
**工作量**：≈ 1.5–3 周（含图重写与计划文档语义对齐）。

### 4.4 P2：n8n 触发/审批/集成（条件触发，可选）

**准入条件**：T1 或 T4 成立（真的有 Issue/PR/定时触发，或需要非文本审批 UI）。

| # | 改动 | 说明 |
|---|---|---|
| 1 | compose 新增 `n8n`（`profiles: ["n8n"]`，卷 `n8n-data`，端口 5678） | 默认不启动 |
| 2 | 反代 + 认证 | 与 web 同域反代；Webhook 用 HMAC/静态 token + 重放保护 |
| 3 | 最小工作流 | Webhook → GET 只读 payload → POST run |
| 4 | 审批回调 | `POST /internal/dev_task/{id}/approve`（网关鉴权）→ 复用 D4 |
| 5 | 通知 | 复用 `qq.push`；不新增主动消息场景 |
| 6 | 审计导出 | n8n 只展示 `task_id` + 脱敏摘要 |

**验证**：n8n 停机主流程不受影响；webhook 重放被拒；执行记录无正文；n8n 资源占用实测。
**回滚**：停容器 + 关 webhook（QQ 链路本就不依赖）。
**工作量**：≈ 3–5 天（含 compose/反代/联调）。

### 4.5 P3：A2A（不做）

- 仅在 T3（对外暴露/跨框架协作）出现时启动；届时 `sessionId` 即 `task_id`，需 `agent-card.json` + JWT/mTLS。

### 4.6 阶段汇总

| 阶段 | 解决 | 新进程 | 新依赖 | 回滚开关 | 退出/放弃条件 |
|---|---|---|---|---|---|
| P0 | G-a/G-b/G-c/G-e | `dev-worker` | 无 | `DEV_QUEUE_ENABLED` | 故障注入 3 次全通过则收尾；连续 2 次重投失败率 >20% 则回到 inline 并重新设计 |
| P0.5 | G-d | 无 | git worktree | `DEV_WORKTREE` | 主干零污染验证通过 |
| P1 | 执行力/中断粒度 | Python + LangGraph | LangGraph/DeepAgents | `DEV_EXECUTOR` | 4 周内无法复现 P0 全部验证项 → 放弃 P1 |
| P2 | G-f + 审批 UI | n8n | n8n（fair-code） | 停容器 | 一个月内无外部触发使用 → 下线 |
| P3 | — | — | — | — | 无需求不做 |

---

## 5. 风险与反模式

| 编号 | 风险 / 反模式 | 缓解 |
|---|---|---|
| R1 | 双状态机：Rust 迁移表 + LangGraph 图都改状态 | 只允许 Rust 迁移；LangGraph 走 MCP 请求（D4） |
| R2 | 双 checkpointer：pi 会话文件 + LangGraph checkpoint 同时驱动同一 `sessionId` | `DEV_EXECUTOR` 硬开关；禁止灰度 |
| R3 | 重复副作用：`interrupt` 恢复重跑节点 → 重复提交、重复推送 | 副作用后置 + `job_id` 幂等 + 推送去重（`event_id`） |
| R4 | 受理回执被拖进异步链（n8n 网络跳数） | 回执留在同步路径（D1） |
| R5 | 「两套并行」长期不收敛 | 每阶段有退出条件；P0 结束后删除旧路径 |
| R6 | n8n 记录泄露群消息正文 | 只存 `task_id` + 摘要（D7） |
| R7 | 把 `LocalShellBackend` 当默认执行环境 | 明确禁用；worktree + 容器（D5/P0.5） |
| R8 | 用 n8n 做「万能编排」导致业务逻辑外流 | 编排只到「触发/审批/通知」，业务校验留在 Rust |

---

## 6. 影响

**正面**

- 崩溃可恢复、失败可重试、职责清晰（入口 / 执行 / 权威三方分离）。
- 审计可回溯（`dev_task_events`）：谁在何时批准/拒绝/重试。
- 为 P1/P2 留出干净切口，且**不做也能独立获益**（P0/P0.5 与文档架构无关，纯可靠性）。

**代价**

- 新增一个常驻进程（`dev-worker`）与两张表；`XACK` 语义变更需回归 `#任务/#进度/#补充` 全部路径。
- worktree 引入磁盘占用与清理策略。
- 若上 P1/P2：多语言 + 多容器运维成本显著上升（ADR-0013 R4）。

---

## 7. 依据与验证状态

| 事实 | 取值 | 来源 | 验证状态 |
|---|---|---|---|
| 群聊被动回复窗口 / 次数 | 5 分钟 / 每条 5 次 | 官方口径 → `docs/conventions/external-constraints.md` | 已验证 |
| 现状 dev 执行 fire-and-forget + ACK 时机 | `void run…`、`handle()` 返回即 `XACK` | 代码（`src/index.ts`） | 已验证 |
| `dev_tasks` 现有字段 / 无审计表 | 见 §1.1 | `crates/storage/src/lib.rs` | 已验证 |
| `sessionId` 已是统一键 | 子会话 id = 计划 id = `dev_tasks.session_id` | `docs/modules/dev-task.md` | 已验证 |
| `interrupt()` 恢复从节点开头重跑 | — | 方案文档 §4 注 | **待验证**（P1 前实测） |
| LangGraph Checkpointer / `thread_id` 语义 | — | 方案文档 §5/§7 | **待验证**（P1 前核对官方文档） |
| DeepAgents 建于 LangGraph runtime | 官方原文 | LangChain《Deep Agents overview》 | 已验证（2026-10-20） |
| n8n 许可 / 资源占用 | 未知 | n8n 官方（未取到原文） | **待验证**（P2 前必核） |

---

## 8. 待确定事项

- **TD-14.1**：`dev-worker` 语言：Node（复用 `DevSessionManager`，零重写）vs Rust（与单写者一致但需重实现 pi 会话桥）。**倾向 Node**。
- **TD-14.2**：`dev.jobs` 是否与 `internal` 合并为一个 Stream？倾向**分开**（语义与保留期不同）。
- **TD-14.3**：租约过期后的重投策略：立即重投 vs 指数退避（1/2/4 分钟）。
- **TD-14.4**：`dev_task_events` 是否也记录「LLM 输入/输出摘要」？涉及正文留存与合规，倾向**只记里程碑**。
- **TD-14.5**：worktree 保留期与清理（`N` 天 / 任务终态后立即清理）。
- **TD-14.6**：P1 是否需要重实现 `plan_record` 6 分区语义（DeepAgents `write_todos` 语义不等价：只有勾选状态，没有「待确定事项 / 验证事项 / harness 优化项」分区）。
- **TD-14.7**：审批超时策略（现状无超时；P2 若做，超时默认**驳回**而非自动批准）。
- **TD-14.8**：是否需要 dev 任务的「取消正在执行」能力（现状 `cancel_task` 只改状态，停止不了已启动的 pi 会话）。
- **TD-14.9（已决，2026-10-20）**：执行器接入项目技能与 MCP 的口径——技能走**只读 backend**（`FilesystemBackend` 虚拟根 = `<repo>/.pi/skills`，`write/edit/delete/upload` 直接返回错误 + `FilesystemPermission(write, ["/", "/**"], deny)` 兜底；`skills=[("/", "项目技能")]`）；MCP 走 `langchain-mcp-adapters` 读 `.pi/mcp.json`，由 `DEV_MCP_SERVERS` 显式开启（**默认关**），加载失败只注记不阻断。理由：① deepagents 0.7.28 的 `CompositeBackend.ls` 路由前缀重映射错位（`/skills/x` → `/skill/x`）→ 挂 `/skills` 取不到 `SKILL.md`；② 原 `tools=[]`+`StateBackend` 让技能完全不可见；③ MCP 工具只有 coroutine，planner 必须 `ainvoke`（`recursion_limit` 提到 20，中间件 hook 也计步）。未决：playwright 是否进 planner（当前**不进**，plan 阶段无浏览器需求）、`rubric_grader`/`memory` 是否接入。
