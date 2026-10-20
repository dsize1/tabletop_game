# ADR-0015：切新路线时保留「消息驱动 + MCP 桥」口径下的阻碍、持久化改动与冗余清理（Rust 引擎可废弃）

- **状态**：提案（Proposed，2026-10-20；修订 v2：按「n8n 承担消息驱动/消费，LangGraph 承担状态管理」口径重写归属；v3：补充 n8n 将承接 Git/CI/自动测试/部署）
- **用户口径（2026-10-20，v2 澄清）**：
  1. 迁到新路线时**可以停止原开发线的使用**（不再用 pi 子会话执行 dev 任务）；
  2. **消息的驱动与消费由 n8n 这一层实现**（对应两份技术文档 §2 各层职责 / §3.2 改造后链路），**不是**由 agent-gateway 继续承担编排；
  3. **LangGraph 承担状态管理**（任务状态、分支、重试、checkpoint、interrupt/resume），**DeepAgents 承担智能执行**（规划、文件读写、子 Agent、上下文压缩、工具调用）；
  4. 仍**保留 MCP 原语**作为工具/数据契约；
  5. **原 Rust 引擎（`crates/dev-task` + `crates/dev-task-mcp`）可以废弃**；
  6. **n8n 层将承担未来的 Git / CI / 自动测试 / 部署能力**（未来态，不是现状迁移项）。
- **关系**：**修订 ADR-0013 D3/D6**（「Rust 为状态权威」→ 状态管理归 LangGraph，Rust 退为「索引/校验/审计的一侧」）；**修订 ADR-0014 P0**（原「现状栈内做 worker+租约」不再是目标形态，改为 n8n 驱动 + LangGraph 状态）；继承 ADR-0009（主动消息门禁）、ADR-0001（Streams/SQLite 传输与真相分工，但「真相」内容变化见 §3）
- **相关**：ADR-0013、ADR-0014、`docs/conventions/mcp-primitives.md`、`docs/conventions/persistence.md`、`docs/architecture.md`

## 结论一句话

口径成立，但**「n8n 驱动/消费 + LangGraph 状态」会把原来的四个不变量从 Rust 里赶出来**：白名单/归属、审计、任务索引（`#任务/#进度`/计划短链）、单活跃执行。**这四项必须在新栈里有明确落点（§2.4「L3 必补清单」），否则停掉 Rust 引擎的当天，群里就查不到任务、签不出短链、也证不了谁批的。**

MCP 原语保留的是**契约**（工具名 + 语义 + 校验结果），不是 Rust 实现；实现方可以是一个薄「桥服务」（§2.3）。

---

## 1. 目标形态（按 v2 口径，与两份文档一致）

```
QQ 群 → qq-gateway → qq.raw → bot-adapter → internal
  → agent-gateway【薄消费/入口】（只做三件事 + 出站出口）
        ├─ 识别 dev 消息（固定前缀；不做意图猜测）
        ├─ 落库（任务索引）+ 入队
        ├─ 触发 n8n Webhook（只传 task_id，不传正文）
        └─ 写 qq.push（被动/主动；唯一能发群消息的地方）
  → n8n【驱动 + 消费 + 未来 CI/CD】
        ├─ 拉 payload（调原语 get_task，只读）
        ├─ 调 LangGraph 启动/恢复 run（thread_id = sessionId）
        ├─ 审批节点（approve_plan / approve_verify / approve_deploy）
        ├─ 消费结果 → 回写索引 → 调原语 notify_user 通知
        ├─ 外部系统对接（Issue/PR/定时/CI）
        └─ 未来：Git 操作 / 触发 CI / 自动测试 / 部署（高危动作必过人工门禁）
  → LangGraph【状态管理】: thread_id / checkpoint / interrupt / resume / 重试 / 路由
  → DeepAgents【智能执行】: 规划 / worktree 文件读写 / 子 Agent / 上下文压缩 / MCP 工具
  → MCP 原语【契约层】: 任务索引读写、校验、计划解析、通知、能力自述
```

| 职责 | v2 归属 | 说明（平台约束 / 设计约束） |
|---|---|---|
| QQ 收发 | `qq-gateway`（不可替代） | 只有它持有 QQ 凭证与 `msg_id` 被动窗口能力；n8n 不得直连 QQ OpenAPI |
| 消息驱动（决定「下一步做什么」） | **n8n** | 触发、分支、审批推进、通知、外部系统对接 |
| 消息消费（结果回写） | **n8n** | 消费 LangGraph 结果/事件 → 回写索引 → 触发通知 |
| 状态管理（任务状态机） | **LangGraph** | checkpoint 是执行态真相；`thread_id = sessionId` |
| 智能执行 | **DeepAgents** | 在 worktree/沙箱内跑；不得直接改业务库 |
| 工具/数据接入 | **MCP 原语** | 保留契约；实现方为「桥服务」 |
| **Git / CI / 自动测试** | n8n 编排 + MCP 执行原语 | n8n 决定「什么时候跑」；具体动作（clone/PR/触发流水线）走 MCP，避免长在 workflow 里无法复用/测试 |
| **部署 / 回滚** | n8n 审批 + 部署执行器 | **必须人工审批**（ADR-0013 D7）；凭证不进 LLM 上下文 |
| 任务索引 + 审计 | 桥服务 + `server`（唯一 SQLite 写者） | 供 `#任务/#进度/#确认` 与计划短链使用；**不作为状态权威** |

> **一个必须点明的平台事实**：文档说「n8n 负责通知」，但本项目通知只能落到 `qq.push`（由 gateway 发出）。因此 n8n 的「通知」= 调用原语 `notify_user`（桥 → `qq.push`），n8n 自身不持有 QQ 凭证。

---

## 2. 阻碍与问题

### 2.1 硬阻塞（不解决就不能停原执行线）

| 编号 | 问题 | 根因（现状事实） | 后果 | 缓解 |
|---|---|---|---|---|
| **B1** | MCP 无反向推送通道 | MCP stdio 是「client 请求→server 响应」；`qq.push` 只有 gateway 能写 | LangGraph/DeepAgents 无法主动通知群；n8n 若等结果再通知，只能同步阻塞 | 执行侧只写**事件/状态**；通知由 n8n 消费到事件后调 `notify_user` 原语 → gateway 写 `qq.push`。**任何「agent 直接发消息」都不允许** |
| **B2** | 长任务 × 同步调用 = 必然超时 | LangGraph run 可能分钟级；n8n HTTP 节点/MCP 调用都有超时 | 「触发并等待」的写法会超时、重试、重复执行 | 两段式：`start_run` 立即返回 `run_id`；结果靠 webhook 回调 / `get_run` 轮询。**等待不得发生在任何同步调用里** |
| **B3** | `confirm_task` 一函数两语义 | 现有实现：`awaiting_confirm→running`（确认计划）与 `verifying→done`（验收）同一原语 | LangGraph 两个 `interrupt` 点无法区分，审批可能错误恢复 | 拆 `approve_plan` / `approve_verify`（或加 `stage` 参数）；n8n 审批节点按 `stage` 分流 |
| **B4** | 状态权威迁移 → **索引投影**成为新风险 | 现状 `#任务/#进度` 直接读 `dev_tasks.status`；v2 后状态在 LangGraph | 投影滞后/漏写 → 用户看到「执行中」但实际已结束；或反之 | `dev_tasks` 明确降级为**只读投影**（由桥单向写入）；配 `last_synced_at` + 对账任务；判定一律以 LangGraph 为准 |
| **B5** | 无执行者/租约信息 | `dev_tasks` 只有 `status` | 停掉 pi 后 `running` 既无进程也无心跳，崩溃不可检测 | `dev_runs`（run 索引）+ `executor_ref`（thread_id / n8n execution_id）+ 心跳/超时标注；超时由 n8n 定时工作流兜底 |
| **B6** | 取消无法传递 | `cancel_task` 只改库；现状连 pi 会话都停不掉（TD-14.8） | `#取消` 后 LangGraph 继续跑、继续改 worktree、继续通知 | `cancel_run` 映射 LangGraph cancel；`run.status=cancelled` 后所有后续通知按幂等键丢弃 |
| **B7** | 白名单/归属被旁路 | 现有校验在 Rust 进程内；v2 后 n8n 是入口 | 改 n8n 工作流配置即可绕过 `DEV_WHITELIST` | **白名单数据源与判定留在 `server`**；n8n 只能调 `check_permission` 原语拿结果，不得自判；无权限时 n8n 直接终止 |
| **B8** | 计划文档格式是隐式契约 | `get_progress` 硬解析 `## 待办实现` / `## 验证事项` 的 `- [x]` | DeepAgents 的 `write_todos` 产出若无这两个分区，`#进度` 归零 | 计划格式写进原语契约；`plan_ready` 事件前校验格式，不合格拒绝；`plan_format` 列做版本判定 |
| **B9** | n8n 拉不到 payload | 现有 `dev-task-mcp` 是 stdio，n8n 无法直接调 | 「Webhook 只传 task_id → n8n 拉完整 payload」这条文档链路走不通 | 桥服务必须提供 **HTTP 只读接口**（`GET /internal/dev_task/{id}`）或 MCP-over-HTTP；这是新增项，不是「保持原语」能覆盖的 |
| **B10** | 通知原语不存在 | 现有原语无一能发消息 | n8n 无法完成「结果回写 + 通知」 | 新增 `notify_user(session_id, qq_id, text, mode)`：`mode=passive` 仅在窗口内可用，`mode=proactive` 受 ADR-0009 门禁约束（默认拒绝） |

### 2.2 次要问题（可并行处理，但会影响体验/排障）

| 编号 | 问题 | 说明 |
|---|---|---|
| S1 | 幂等三重叠加 | `interrupt` 重跑节点 + n8n 重试 + 桥重试 → 重复执行/重复通知。需要 `idempotency_key = (session_id, stage, attempt)` |
| S2 | 鉴权三处不一致 | MCP stdio 无鉴权；n8n Webhook 需 HMAC/token；LangGraph Server 需 API key；桥的 HTTP 接口需内网 token |
| S3 | 观测链断裂 | 现在靠 `cid=message_id` + `sessionId`；v2 多出 `run_id` / n8n `execution_id` / `thread_id`，无统一 trace 则排障断档 |
| S4 | 单活跃执行 | 现状靠 `withLock(sessionId)`；v2 后锁在 LangGraph 内，但「同一任务并发两个 run」仍可能由 n8n 重试触发 → 需 DB 唯一约束 + n8n 侧去重 |
| S5 | 执行隔离未解决 | 换栈不解决宿主 shell 风险；worktree/沙箱必须独立立项 |
| S6 | 工具面缺口 | pi 的 `plan_record`/`plan_check`/`codemode`/playwright 在 DeepAgents 无等价物 |
| S7 | n8n 成为单点 | n8n 停机 → 任务无法推进（这次是**关键路径**，不是旁路）。需明确「n8n 挂了时用户看到什么」：`#任务/#进度` 仍可读（投影），但不能推进 |
| S8 | 成本与质量回归 | DeepAgents 的 todo/子 Agent/摘要放大 token；模型/思考行为与 `deepseek-v4-pro` 不同 |
| S9 | 推送口径 | `notify_user` 的主动模式必须过 ADR-0009；默认只允许 `passive` |

### 2.3 「谁实现原语」：桥服务的定位

保留原语 ≠ 保留 Rust 引擎。原语由一个新的**薄桥服务**实现（无状态机、不判状态合法性，只做「落库/查询/校验/转发」）：

| 候选 | 优点 | 缺点 |
|---|---|---|
| **Node 薄服务（推荐起步）** | 与 gateway 同栈、可复用 `dev-task-client` 与 `qq.push` 逻辑；部署最简 | 与 n8n/Python 之间多一跳 HTTP |
| Python MCP（与 LangGraph 同栈） | 少一跳；可直接被 LangGraph 当 MCP 工具 | 需要再实现一遍 `qq.push` 交互/鉴权；与 Rust 校验割裂 |
| 保留 Rust `dev-task-mcp`（只删状态机） | 复用 rmcp + schema；白名单/归属就近 | 仍是「没删干净」，与「Rust 引擎废弃」口径有张力 |

> 推荐：**Node 薄桥**承接原语 + 校验调用（`server` 内部 API），n8n 通过 HTTP/MCP 调它；Rust 侧只保留 `server` 的存储与校验端点。

### 2.4 「Rust 引擎可废弃」的三档 + L3 必补清单

| 档 | 废弃什么 | 状态权威 | 与 v2 口径 | 评价 |
|---|---|---|---|---|
| **L1** 只废执行 | pi 子会话 | Rust 迁移表 | ❌ 不符（n8n 未驱动） | 改动最小，等于没切 |
| **L2** 废引擎+MCP，状态留 Rust | `crates/dev-task*` / `dev-task-client.ts` | Rust `server` | ⚠️ 部分符合（驱动在 n8n，状态仍在 Rust） | 可作为**过渡档**（先跑通 n8n 驱动，再迁状态） |
| **L3** 废状态层（**目标档**） | 上者 + `dev_tasks` 判定逻辑 | **LangGraph** | ✅ 符合 | 必须先补下方 6 项，否则断裂 |

**L3 必补清单（缺一项就不建议切）**

| # | 不变量 | 迁移后落点 | 具体做法 |
|---|---|---|---|
| 1 | 白名单 / 任务归属 | `server`（数据 + 判定） | n8n 调 `check_permission(op, session_id, qq_id)`；`server` 返回 allow/deny + 原因；n8n 只信这个结果 |
| 2 | 审计（谁在何时批准） | `dev_task_events`（桥写）+ n8n 执行记录 | 事件含 `actor` / `kind` / `run_id` / `n8n_execution_id`；n8n 侧不存消息正文 |
| 3 | 任务索引（`#任务/#进度`） | `dev_tasks` **投影**（桥写） | 从 LangGraph checkpoint/run 结果单向投影；`last_synced_at`；对账任务比对差异并告警 |
| 4 | 计划短链 | 计划文档 + 索引 `plan_path` | `/resources/dev_task/{id}/plan` 继续读索引；DeepAgents 必须把计划写到 `memory_palace/plan/{id}.md`（不能只存 checkpoint） |
| 5 | 单活跃执行 | LangGraph + `dev_runs` 唯一索引 | `UNIQUE(session_id) WHERE status IN ('queued','running')`；n8n 重试前先查 |
| 6 | 取消 | `cancel_run` → LangGraph cancel | 事件 + 通知丢弃幂等键；取消后 `dev_runs.status=cancelled` |

> 结论：**L3 可以做，但它不是「删代码」，而是「把 6 个不变量搬到新位置并加对账」**。这也解释了为什么「先补表 → 再切驱动 → 最后删引擎」的顺序不能省。

### 2.5 n8n 承接 Git / CI / 自动测试 / 部署（未来态）带来的新增阻碍

**现状事实**：仓库只有 `.github/workflows/contribution-license.yml`（校验 PR 描述勾选），**没有 build/test/deploy 流水线**；部署靠本地 `dev-up` + `deploy/cloudflared` 隧道，无 staging/prod 分环境。所以这块是**净新增能力**，不是“迁移旧链路”；但其凭证与门禁直接影响现有 dev 线的安全模型。

| 编号 | 问题 | 后果 | 缓解 |
|---|---|---|---|
| **C1** | **凭证进错层** | 若把 Git/CI/部署 token 交给 DeepAgents 或写进计划文档，一次 prompt injection 即可推分支/改流水线 | 凭证只存在执行器/ n8n credential store；LLM 只能**请求动作**，不能携带凭证；计划文档禁止写 secret |
| **C2** | **高危动作被自动化** | 合并、部署、DB 变更一旦由 Agent 自主执行，事故不可逆 | 三者一律走 `approve_deploy` 人工节点（ADR-0013 D7）；自动测试可自动，但失败必须**阻断**部署 |
| **C3** | **n8n 变成唯一入口** | 当前 n8n 已在关键路径；再接 CI/部署后，n8n 挂了 = 不能提交不能发布 | 保留「旁路脚本」：`git push` / 本地 `npm test` / 手动部署流程必须仍可用（写进 TD-15.13） |
| **C4** | **分支/环境并发** | 同一分支多个 dev 任务同时跑 CI，worktree 与镜像 tag 冲突 | 分支命名固定 `dev/{sessionId}`；合并前 rebase；`dev_runs` 单活跃约束；部署按环境串行（`staging` 先于 `prod`） |
| **C5** | **可观测断层** | 出现「哪个 commit 上的测试通过、部的是哪个镜像」无法回答 | `dev_artifacts` 记录 `commit_sha` / `branch` / `test_report` / `image_tag`；通知与审批均引用 `commit_sha` |
| **C6** | **回滚无预案** | 部署失败只能手忙 | 部署前必须产出回滚点（上一镜像 tag / 上一 commit）；`approve_deploy` 同时确认回滚方案 |
| **C7** | **测试资源不足** | 本项目测试需要 Redis（集成）、llama-server（容量），CI runner 上没有 | 分层：单测（离线）进 CI 必跑；集成/真机测试标记 `manual`，不阻塞合并但阻塞部署 |
| **C8** | **能力长在 workflow 里** | 把「clone / PR / 触发流水线 / 部署」写成 n8n 节点，换仓库/换平台就得重画，也无法被 LangGraph 复用 | **能力走 MCP**（文档 §4）：`git-mcp` / `ci-mcp` / `deploy-mcp`；n8n 只编排与审批，不实现细节 |
| **C9** | **部署结果通知口径** | 部署成功/失败通常要主动推送，而主动消息默认禁用 | 默认只回现状（`#任务` / 网页）；若要主动推送，先过 ADR-0009 门禁（TD-15.14） |

**阶段定位**：C1–C9 属 **P3+**（n8n 已稳定运行且 dev 链路切完后才做），不应与 P0/P1 捆绑。否则会把「换执行器」和「新建 CI/CD」两件事绑在一起，出事无法归因。

---

## 3. 持久化：改什么、留什么、删什么

### 3.1 保留不动

| 存储 | 原因 |
|---|---|
| `group_messages`（in/out 全量留档） | 唯一可信的「群里到底说了什么」；n8n 侧禁止存正文 |
| `memory_palace/plan/{sessionId}.md` | **计划真相** + `#进度` 解析对象 + 短链读取对象；不得只存 checkpoint |
| `chat_history` / `user_profile` / `game_events` / `game_state` | 与 dev 无关 |
| Redis `internal` / `qq.push` / `chat.log` | 传输层语义不变 |
| Redis `msgseq:{message_id}`、`intent:pending:*`、auth token（`dev_task:<id>`） | 被动计数、意图确认、短链鉴权；与执行位置无关 |
| `dev_tasks` **表结构本身** | 索引/投影载体；删表 = 删掉 `#任务/#进度` 与短链 |

### 3.2 必须新增

```sql
-- ① 审计 + 通知事件源（现在完全没有）
CREATE TABLE IF NOT EXISTS dev_task_events (
    id          INTEGER PRIMARY KEY,
    event_id    TEXT NOT NULL UNIQUE,
    session_id  TEXT NOT NULL,
    kind        TEXT NOT NULL,  -- created|plan_ready|approval_requested|approved|rejected
                                -- |run_started|executed|verified|failed|cancelled
                                -- |projection_stale|notify_sent|notify_dropped
                                -- |ci_triggered|ci_passed|ci_failed|deploy_requested
                                -- |deploy_approved|deployed|rolled_back
    actor       TEXT,           -- openid | n8n:<execution_id> | langgraph | system
    payload     TEXT,           -- JSON（脱敏；禁止存消息正文/LLM 全文）
    created_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_dev_task_events_session ON dev_task_events(session_id, id);

-- ② run 索引（替代 data/dev-sessions/* 作为「谁在跑」的可查记录）
CREATE TABLE IF NOT EXISTS dev_runs (
    run_id          TEXT PRIMARY KEY,   -- = sessionId:stage:attempt
    session_id      TEXT NOT NULL,
    stage           TEXT NOT NULL,      -- plan|execute|verify|ci|deploy
    executor        TEXT NOT NULL,      -- langgraph|ci|deployer（pi 仅 shadow 期）
    executor_ref    TEXT,               -- thread_id / n8n execution_id / ci run id
    status          TEXT NOT NULL,      -- queued|running|succeeded|failed|cancelled
    attempt         INTEGER NOT NULL DEFAULT 1,
    branch          TEXT,               -- dev/{sessionId}；部署时为发布分支
    commit_sha      TEXT,               -- 本次 run 对应的提交（可观测 C5）
    idempotency_key TEXT UNIQUE,
    started_at      TEXT,
    ended_at        TEXT,
    last_error      TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS uq_dev_runs_active
    ON dev_runs(session_id) WHERE status IN ('queued','running');

-- ③ 产物（未来 CI/部署的可观测依据）
CREATE TABLE IF NOT EXISTS dev_artifacts (
    id          INTEGER PRIMARY KEY,
    session_id  TEXT NOT NULL,
    kind        TEXT NOT NULL,   -- worktree|commit|test_report|image|deploy
    ref         TEXT NOT NULL,   -- 路径 / sha / tag / 环境名
    digest      TEXT,            -- 校验和（可选）
    created_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_dev_artifacts_session ON dev_artifacts(session_id, id);
```

- **checkpoint 不进 `tabletop.db`**：LangGraph checkpointer 用独立文件/库（如 `data/langgraph/checkpoints.sqlite`），避免多写者与 schema 耦合；备份/保留期单列策略（TD-15.12）。
- `dev_task_events` 是「审计 + 通知」的唯一输入：n8n 消费它做通知，bridge 消费它做投影，排障查它。

### 3.3 需要改列（`dev_tasks`，降级为投影）

| 列 | 类型 | 用途 |
|---|---|---|
| `stage` | TEXT | `plan`/`execute`/`verify`；区分「待确认计划」与「待验收」（B3） |
| `executor` | TEXT | `langgraph`（shadow 期可为 `pi`） |
| `executor_ref` | TEXT | thread_id / run_id（取消、排障、join） |
| `contract_version` | INTEGER | 原语契约版本（1=现状，2=新链路） |
| `plan_format` | TEXT | 计划格式标识（`plan6-v1`），防 B8 |
| `last_synced_at` | TEXT | 投影时间；与 `dev_task_events` 对比可发现滞后（B4） |
| `sync_source` | TEXT | 投影来源（`langgraph`/`n8n`/`manual`），出问题时能定位是谁写的 |

- 现有 8 列全保留；`ALTER TABLE ... ADD COLUMN` 可重复执行。
- **语义变更**：`dev_tasks.status` 从「权威」变为「LangGraph 状态的只读投影」；任何判定（能否确认、能否取消）都不得读它。
- 新增索引：`dev_tasks(status, updated_at)`、`dev_tasks(last_synced_at)`。

### 3.4 冗余：该删的（含删除条件）

| 对象 | 现状 | 处置 | 删除条件 |
|---|---|---|---|
| `crates/dev-task`（312 行 + 6 单测） | 状态机 + 白名单 + 归属校验 | L3：判定逻辑拆成「`server` 校验端点」+「投影」；crate 删除 | L3 六项不变量全部有落点 + 无引用（`grep 'dev_task::'` 为空） |
| `crates/dev-task-mcp`（261 行，11 原语） | MCP stdio + 直接开 SQLite | L3：进程删除，原语迁薄桥（Node/Python）；**不再开 SQLite** | 桥服务通过契约测试（同名同语义） |
| `crates/Cargo.toml` 两个 member | `dev-task` / `dev-task-mcp` | 删除 + `Cargo.lock` 重建 | 上两项完成 |
| `apps/agent-gateway/src/dev-task-client.ts` | MCP stdio 客户端，传 `SQLITE_PATH`/`DEV_WHITELIST` | 改为连薄桥；**不再传 `SQLITE_PATH`** | 桥连通 |
| `.pi/extensions/apps-startup/index.ts:93` `dev-task-mcp.exe` | 开发环境自动拉起 | 从清单移除 | 进程删除后 |
| `data/dev-sessions/*`（pi 会话文件） | 每任务一个 | shadow 期保留，切完归档后删 | ① shadow 通过 ② 无 running ③ 30 天归档 |
| `apps/agent-gateway/src/dev-session.ts` | `DevSessionManager`/`devTaskPrompt`/`withLock` | 整文件删除；gateway 不再依赖 `pi-coding-agent`（当前仅此文件 import） | 同上 |
| `runDevCreate` / `runDevResume` / `extractTitle` | 执行 + 脆解析标题 | 删除；标题由执行侧显式字段回填 | 同上 |
| `DEV_LLM_MODEL`（网关执行 env） | 子会话模型配置 | 迁 Python 侧；网关删除 | 同上 |
| `#任务` 里 `queued/planning` 的语义推导 | 读 `status` 推阶段 | 改为读投影 `status+stage`；`planning` 由 `stage=plan` 替代 | 投影上线后 |
| `transition_task` 原语 | 网关直接调它推进 | 保留语义（执行侧上报），改为桥内部调用；不再对网关暴露「任意迁移」 | 契约 v2 |
| `dev_tasks.status` 与 `dev_runs.status` 的重复 | 都有 queued/running | **不是冗余**：前者=对用户可见的投影，后者=执行调度态；禁止互相推导 | — |
| `verifying` 与 LangGraph 第二个 interrupt | 同一语义两处 | **保留**，用映射表 | — |
| n8n 执行数据里的正文/状态表 | — | **禁止**当真相、禁止存正文 | — |

### 3.5 顺手改的关联点

| 位置 | 改动 | 原因 |
|---|---|---|
| `pushReply`/`pushActive` payload | 增加 `dev_session_id`（+`run_id`） | 出站与任务 join；不动 `group_messages` 结构（落 `raw_payload`） |
| `#任务`/`#进度` 渲染 | 读投影 `status+stage`，`running` 附 `executor/attempt/last_synced_at` | 让「投影滞后」可见 |
| `get_progress` | `plan_format` 不匹配时明确报错（不静默 0/0） | B8 |
| `cancel_task` | 同时置 `dev_runs=cancelled` + 调 LangGraph cancel | B6 |
| gateway 出站循环 | 消费 `dev_task_events` 的 `notify_*` → 写 `qq.push` | B1/B10；主动模式走 ADR-0009 |
| `server` | 新增 `/internal/dev/*`（任务索引读写、权限校验、事件追加、只读 payload） | B7/B9 |
| `docs/architecture.md` §3.1/§4/§11、`docs/modules/dev-task.md`、`docs/modules/agent-gateway.md` | 改为「n8n 驱动/消费 + LangGraph 状态 + DeepAgents 执行 + MCP 契约」 | 文档同步 |

---

## 4. 「保持原语」的契约边界（v2 建议）

| 原语 | 状态 | 说明 |
|---|---|---|
| `create_task` / `list_tasks` / `get_task` / `list_capabilities` / `prepare_task_link` / `append_instruction` | **不变** | 纯查询/授权；实现方换薄桥即可 |
| `confirm_task` | **拆** | `approve_plan` / `approve_verify`（B3）；n8n 审批节点按 `stage` 调 |
| `transition_task` | **收口** | 只允许执行侧/桥内部上报，不允许外部随意迁移 |
| `update_task_meta` | **加字段** | 增加 `plan_format`、`executor_ref` |
| `get_progress` | **加校验** | `plan_format` 不符 → `错误: 计划格式不符`（B8） |
| *新增* `start_run(session_id, stage)` / `finish_run(run_id, ok, error?)` / `cancel_run(session_id)` | **新增** | B2/B5/B6；`example` 标「（n8n/执行器内部）」 |
| *新增* `list_events(session_id, since_id?)` | **新增** | 通知桥 + 投影 + 排障 |
| *新增* `check_permission(op, session_id, qq_id)` | **新增** | B7；n8n 只信 `server` 的判定 |
| *新增* `notify_user(session_id, qq_id, text, mode)` | **新增** | B10；`passive` 默认，`proactive` 受 ADR-0009 门禁 |
| *新增（P3+）* `git_*` / `ci_*` / `deploy_*` | **新增** | C8；能力走 MCP（`git-mcp`/`ci-mcp`/`deploy-mcp`），n8n 只编排；高危动作必须先 `approve_deploy` |

> 原则：**用户侧命令与语义不变**（`#发起/#确认/#取消/#进度/#补充/#任务/#能力` 一字不改）；原语可增可拆，但**校验结果与错误语义必须一致**（否则用户会看到「之前能确认，现在不行」）。

---

## 5. 依据与验证状态

| 事实 | 来源 | 验证状态 |
|---|---|---|
| `dev_tasks` 仅 8 列（无 events/runs/租约） | `crates/storage/src/lib.rs` | 已验证 |
| `get_progress` 硬解析 `## 待办实现` / `## 验证事项` 复选框 | `crates/dev-task-mcp/src/main.rs::count_plan_items` | 已验证 |
| `confirm_task` 双语义 | `crates/dev-task/src/lib.rs::confirm_task` | 已验证 |
| `cancel_task` 不停执行 | 代码无 cancel 调用；ADR-0014 TD-14.8 | 已验证 |
| `pi-coding-agent` 仅被 `dev-session.ts` 引用 | `grep` 全仓 | 已验证 |
| 计划短链依赖 `dev_tasks.plan_path` + 文件 | `crates/server/src/resources_routes.rs` | 已验证 |
| Rust 引擎面：`dev-task` 312 行 / `dev-task-mcp` 261 行 | `wc -l` | 已验证 |
| `crates/Cargo.toml` 含两个 member；启动清单硬编码 `dev-task-mcp.exe` | `crates/Cargo.toml`、`.pi/extensions/apps-startup/index.ts:93` | 已验证 |
| MCP 无 server→client 反向主动推送（按现有用法） | 协议语义 + 本项目仅 `callTool` | 已验证（通知能力未实测） |
| LangGraph 支持自定义 `thread_id` / cancel；checkpointer SQLite saver | — | **待验证** |
| n8n 许可（fair-code）与资源占用 | — | **待验证** |
| n8n Webhook 重试/超时默认值 | — | **待验证**（直接影响 B2/S1 设计） |
| 现有 CI：仅 `.github/workflows/contribution-license.yml`（PR 描述校验），无 build/test/deploy 流水线 | 仓库 `.github/workflows/` | 已验证 |
| 现有部署：本地 `dev-up` + `deploy/cloudflared` 隧道，无 staging/prod 分环境 | 仓库 `deploy/`、`apps/scripts/dev-up.mjs` | 已验证 |
| 死代码清理已执行（`command-router` / `push` 删除，124 Rust tests + 16/16 Node tests 通过） | 本 ADR §8 | 已验证（2026-10-20） |
| 单测可离线（`npm test` / `cargo test`），集成测试需 Redis/llama-server | `apps/agent-gateway/test-all.mts`、`docs/dev-environment.md` | 已验证 |

## 6. 待确定事项

- **TD-15.1**：薄桥服务选型（Node / Python / 保留 Rust 薄层）——见 §2.3，倾向 Node 起步。
- **TD-15.2**：`contract_version` 是否要做握手原语（防旧 gateway 连新桥）。
- **TD-15.3**：事件→通知的通道：gateway 直接消费 `dev_task_events`，还是桥写 `dev.events` Stream？倾向前者（少一跳）。
- **TD-15.4**：投影由谁写：桥（n8n 结果回调时）还是 n8n 直接调 `update_task_projection` 原语？倾向**桥写**，n8n 不碰 DB。
- **TD-15.5**：对账频率与阈值（差异即告警 vs 自动修复）；自动修复有覆盖用户可见状态的风险。
- **TD-15.6**：checkpoint 保留期与迁移策略（LangGraph 升级可能改 schema）。
- **TD-15.7**：n8n 停机时的用户话术（`#任务/#进度` 可读但「不能推进」）。
- **TD-15.8**：`notify_user` 的 `proactive` 是否永远禁用（当前 ADR-0009 默认禁用；dev 现有 `pushActive` 属既有口径，需确认是否延续）。
- **TD-15.9**：`data/dev-sessions` 归档保留期（建议 30 天）与脱敏要求。
- **TD-15.10**：L3 切档窗口：先切「驱动」（n8n）还是先切「状态」（LangGraph）？建议 n8n 先以 shadow 方式并行（不推进真实状态），再切状态。
- **TD-15.11**：CI 平台选型：GitHub Actions（与现有 PR 门禁同源）vs 自建 runner vs n8n 直接调 API；建议**先用 GitHub Actions**（不引入新凭据面）。
- **TD-15.12**：CI/测试/部署的**触发条件**：每个 dev 任务都跑全量？还是仅 `verifying` 阶段跑一次？需定义哪些 job 阻塞合并、哪些只报警。
- **TD-15.13**：n8n 停机时的旁路要求：`git push` + 本地测试 + 手动部署是否必须保留（C3）。
- **TD-15.14**：部署结果是否主动通知（ADR-0009 门禁）；若只被动，用户如何得知（`#任务 <id>` / web 页）。
- **TD-15.15**：部署目标与环境拓扑（本机 docker compose / VPS / 隧道切换），以及是否需要 `staging`。
- **TD-15.16**：是否需要将 CI/测试结果回写为 `dev_artifacts` 之外的「质量门」字段（覆盖率/漏测）。

---

## 7. 附录 A：n8n CI / 部署工作流草图（MVP）

> 取舍：**只接 GitHub Actions、只在 `verifying` 跑一次、无 staging、无自动合并、部署仅手动工作流**（TD-15.11/15.12 按最简实现，不追求完备）。

### 7.1 CI（测试）链

```
[桥] verifying 到达
   └─ HTTP POST  n8n /webhook/dev-ci        {session_id, commit_sha, branch}
        │  HMAC 校验；只传 id，不传正文
        ▼
   [HTTP] GET  bridge /internal/dev_task/{id}          ← 拉 payload（B9）
        ▼
   [IF] status==verifying && permission==allow          ← 判定只信 server（B7）
        ├─ 否 → POST bridge /events {kind:ci_skipped} → 结束
        └─ 是 ▼
   [HTTP] POST api.github.com/actions/workflows/ci.yml/dispatches
          {ref: "dev/{session_id}"}                     ← 凭证只在 n8n credential（C1）
        ▼
   [Wait 60s] → [HTTP] GET /actions/runs?head_sha={commit_sha}    ← 轮询，最简
        ├─ running → 回 Wait（≤15 次 ≈ 15min）
        ├─ success →
        │    POST bridge /events   {kind:ci_passed, commit_sha}
        │    POST bridge /artifacts{kind:test_report, ref}
        │    notify_user(session_id, qq_id, "CI 通过，可 #确认 验收", passive)
        └─ failure/timeout →
             POST bridge /events   {kind:ci_failed}
             notify_user(..., passive)
             部署入口查 ci_passed 才放行（阻断）
```

### 7.2 部署链（手动，不新增 QQ 命令）

```
[Form 触发 n8n /webhook/dev-deploy]  {session_id, env, image_tag}
   ▼
[HTTP] GET bridge /internal/dev_task/{id}    ← commit_sha / ci 状态 / 回滚点
   ▼
[IF] ci_passed && 有回滚点                    ← C2 / C6
   ├─ 否 → deploy_rejected
   └─ 是 ▼
[Wait-Form 人工审批]  审批人 + 确认回滚方案     ← 高危必人工
   ├─ 驳回 → deploy_rejected
   └─ 批准 ▼
[HTTP] 调部署执行器（MCP `deploy_apply` / gh workflow）
   ▼
[HTTP] POST bridge /events {kind:deployed|failed}
[HTTP] POST bridge /artifacts {kind:image|deploy}
[MCP] notify_user(..., passive)               ← 主动推送需过 ADR-0009
```

### 7.3 MVP 明确不做

| 不做 | 理由 |
|---|---|
| staging/prod 双环境 | 现无环境拓扑（只有 `dev-up` + cloudflared 隧道）→ TD-15.15 |
| 自动合并 | 高危，永久人工 |
| 每次 commit 跑全量 | 集成测试需 Redis/llama-server → 只跑离线单测 |
| 部署结果主动推送 | ADR-0009 默认禁用 → TD-15.14 |
| 群内 CI 查询命令 | 避免扩命令面；先用 `#进度` 带出状态 |

### 7.4 关键不变量

1. n8n **只编排与审批**，Git/CI/部署动作走 MCP/HTTP 原语（C8）。
2. 凭证只存在 n8n credential / 执行器，LLM 不接触（C1）。
3. `commit_sha` 贯穿 `dev_task_events` 与 `dev_artifacts`（C5）。
4. 测试失败 → 部署入口必须拒绝（C2）。
5. n8n 停机 → 旁路（`git push` / 本地测试 / 手动部署）仍可用（C3）。

---

## 8. 附录 B：死代码清理记录（2026-10-20 执行）

### 8.1 清理对象（审计结论：上一代遗留，全仓无调用）

| 对象 | 原证据 | 处置 |
|---|---|---|
| `crates/command-router`（3 行占位） | 无 `command_router::` 调用；职责已并入 agent-gateway（ADR-0002） | **删除 crate** |
| `crates/server/Cargo.toml` 的 `command-router` 依赖 | `crates/server/src/**` 零引用 | **删除依赖** |
| `crates/push`（91 行） | 全仓无 `push::` 调用；出站实际由 agent-gateway Node 写 `qq.push` | **删除 crate** |
| `crates/server/Cargo.toml` 的 `push` 依赖 | 零引用 | **删除依赖** |
| `crates/Cargo.toml` 两个 member | — | **移除** |
| `docs/modules/command-router.md` | 自身已写「不再作为独立 crate 存在」 | **删除文件** |
| `docs/modules/push.md` | 描述已删除的 crate | **重写**为「出站通道约定」，保留频控与测试方式（`qq-gateway.md` 仍引用） |
| `AGENTS.md` 技术栈行 / `README.md` 模块表 / `docs/architecture.md` §2 / `server/src/main.rs` 文档注释 | 仍列死模块 | **同步修正**（并顺手修正 README 中 las-vegas「（空）」的过期描述） |

### 8.2 验证（实际执行结果）

```bash
cargo check --workspace          # ✅ 通过
cargo test --workspace           # ✅ 124 tests（storage 8 / auth 5 / game-engine-core 9 / las-vegas 83 / dev-task 8 / 其他 11）全绿
Cargo.lock 中 command-router / push 条目 = 0   # ✅ 已随 members 移除
npm run test:agent-gateway       # ✅ 16/16 测试文件通过（2.1s）
CARGO_TARGET_DIR=target-smoke cargo build --workspace  # ✅ 20.2s（见下方说明）
```

- 主 target 目录的 `cargo build` 当前会在链接 `server.exe` 时失败：**文件被正在运行的 dev-up 进程占用（os error 5）**，不是编译错误。用独立 target 目录已验证完整构建通过；下次重启 dev-up（先停旧进程）即会正常重建。
- 未触及：`shared::PushEvent`（被 bot-adapter 归档使用，保留）。

### 8.3 明确**未**清理的部分（避免误解）

| 仍在 | 原因 |
|---|---|
| `crates/dev-task` / `crates/dev-task-mcp` / `apps/agent-gateway/src/dev-task-client.ts` / `dev-session.ts` / `runDev*` | **按设计仍在链上**（状态权威 + 执行），需等 §2.4 L3 六项不变量落地后才删 |
| `data/dev-sessions/*` | shadow 期回滚依赖 |
| `dev_tasks` 表 | `#任务/#进度` 与计划短链依赖 |
| `.pi/extensions/apps-startup/index.ts` 的 `dev-task-mcp.exe` | 进程仍在用 |
