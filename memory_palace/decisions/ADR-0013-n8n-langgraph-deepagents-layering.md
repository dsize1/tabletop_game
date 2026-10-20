# ADR-0013：n8n + LangGraph + DeepAgents 三层编排架构（触发 / 编排 / 执行）与协议边界

- **状态**：提案（Proposed，2026-10-20）——**技术探讨，未采纳、未落地**，不产生任何实现约束
- **来源文档**（本次探讨输入）：
  1. 《n8n + LangGraph + DeepAgents 架构设计》（4 页 PDF，概念篇：分层 / 调用链 / MCP 与 A2A 分工 / 状态与持久化 / 安全边界 / 落地顺序）
  2. 《n8n + LangGraph + DeepAgents 架构设计与 dev 消息链路改造方案》（DOCX，落地篇：消息消费服务三件事 / n8n 节点顺序 / 队列与持久化定位 / LangGraph 与 DeepAgents 分工 / 审批恢复 / 安全 / 最终调用链）
- **相关**：ADR-0014（dev 消息链路改造与实现计划，本 ADR 的落地篇）、ADR-0001（Streams 传输 + SQLite 真相源）、ADR-0002（双模型 + MCP 化引擎）、ADR-0009（主动消息门禁）、`docs/architecture.md`、`docs/modules/dev-task.md`、`docs/modules/agent-gateway.md`、`docs/conventions/mcp-primitives.md`
- **本 ADR 不引入主动消息**（审批/通知沿用现有 dev 链路既有推送口径；任何新增主动推送须走 ADR-0009 门禁）

> 修订（2026-10-20）：用户补充口径——①「原 Rust 引擎可废弃」，对本文 **D3/D6「Rust 为状态权威」形成条件式修订** —— 权威位置取决于 ADR-0015 §2.4 选档（目标为 L3：状态迁 LangGraph，Rust 退为索引/校验/审计）；②「n8n 将承担 Git/CI/自动测试/部署」，其安全与凭证边界见 ADR-0015 §2.5（能力走 MCP、高危必人工门禁）。分层原则 D1/D2/D5/D7 不变。

## 结论一句话

**采纳三条原则：①分层职责划分（触发 / 编排 / 执行）作为认知框架；②工具一律 MCP 化，MCP 是唯一「工具」协议；③业务状态真相源唯一（Rust + SQLite），外部编排器只持有 task 引用。**

**不整体引入三层产品栈**：本项目现有 Node（pi-sdk）+ Rust（状态机）+ Redis/SQLite 已具备 L2/L3 的**等价能力**（见 §4 映射表），新栈只在 §6 的**触发条件**成立时按 ADR-0014 分阶段引入。

---

## 1. 背景

### 1.1 现状（改造对象）

当前 dev 消息链路（简版）：

```
QQ 群 → qq-gateway → qq.raw → bot-adapter → internal → agent-gateway(Node)
   → parseDevCommand（固定前缀，人=100% 置信度）
   → dev-task MCP（Rust：白名单 + 状态机 + SQLite dev_tasks）
   → DevSessionManager（pi-sdk 子会话，data/dev-sessions/{sessionId}）
   → pushActive（计划短链 / 结果）/ pushReply（受理回执、状态同步）
```

- 审批 = `#确认 <id>` → `confirm_task`（`awaiting_confirm → running` / `verifying → done`）→ `runDevResume` 恢复同一子会话。
- 已有「interrupt/resume」**语义**的等价物，但它绑定在**消息消费进程内**（`void runDevCreate/runDevResume`），进程重启即丢执行权（详见 ADR-0014 §1.2）。

### 1.2 两份文档的主张

- 分层：`消息接入 → 任务入口/持久化 → n8n 触发编排 → LangGraph 状态编排 → DeepAgents 智能执行 → MCP/A2A → 外部系统`。
- 分工：n8n 做「触发、审批、通知、系统集成」，**不硬扛 Agent 推理**；LangGraph 做状态机/检查点/中断恢复；DeepAgents 做「规划、文件读写、子 Agent、上下文压缩、MCP 调用」。
- 边界：**普通消息链路不进 n8n**；保留现有队列与持久化能力，但把它从「直接驱动 Agent」调整为「任务入口 + 状态源」；n8n 不管理 LangGraph checkpoint、不长期保存完整群消息。
- 安全：`LocalShellBackend` 不可裸开放；合并/部署/DB 变更必须人工审批；A2A 端点必须鉴权。
- 顺序：单链路 → MCP → 审批 → A2A → 最后才是 Rust 协议适配层（Rust 负责 A2A/MCP 桥接、n8n 对接、任务审计与安全策略，Python 作执行引擎）。

### 1.3 为什么需要 ADR

这两份文档描述的是**「换一套 Agent 运行时栈」**级别的改动（Python + LangGraph + DeepAgents + n8n + Docker 编排），与项目既有的「Node 主持人 + Rust 规则引擎」边界直接冲突。若不先把**权威边界**与**引入条件**写清楚，后续会演化出双状态机、双真相源、双入口。

---

## 2. 决策

### D1 分层语义采纳，产品栈不采纳

- 「触发 / 编排 / 执行」作为**职责分层**采纳，进文档与评审口径。
- 但分层**不要求**落地为三个独立产品：职责可以由同一进程内的模块承担（现状即如此）。
- 判定标准：**只有当某一层的职责在当前技术栈内无法满足（而非「不够时髦」）时**，才允许引入该层的专用产品。

### D2 n8n 边界（若引入）

1. 只做：外部事件触发（Webhook / Issue / PR / 定时）、人工审批**呈现**、通知、系统集成、执行审计视图。
2. **不持有业务状态**：业务状态唯一在 SQLite（`dev_tasks` + `dev_task_events`），n8n 只持有 `task_id`。
3. **不写 SQLite**：需要读 payload 时走 server 内部只读 API（`GET /internal/dev_task/{sessionId}`）。
4. **不管理 Agent checkpoint**：`thread_id` 语义归执行层，n8n 只透传。
5. **不在关键路径**：n8n 停机不得阻断「QQ 内受理 → 计划 → 确认 → 执行 → 验收」主流程（对应「n8n 可降级」验证项，见 ADR-0014 §4.4）。

### D3 编排层：状态机 + thread_id + checkpointer + interrupt/resume 是刚需抽象，LangGraph 只是实现选项

- 刚需抽象：**任务状态机（确定性迁移）+ 单一会话标识（thread_id）+ 断点续跑（checkpointer）+ 人工中断/恢复（interrupt/resume）**。
- 现状已在 Rust + pi-sdk 侧成立：`crates/dev-task` 迁移表 + `sessionId` + `data/dev-sessions/*` + `#确认`。
- 因此 LangGraph 的引入前提是「现状**不满足**」，即 §6 的 T2（跨进程/跨机恢复、多 worker 并行）成立。
- 若引入：**状态迁移的权威仍在 Rust**（LangGraph 只能「请求」迁移，不能直接改库），LangGraph checkpointer 独立存储（不与 `tabletop.db` 争单写者，见 ADR-0014 §2 D6、§4.3）。

### D4 执行层：harness 能力需求已存在，pi-sdk 已覆盖；DeepAgents 为候选替代

| harness 能力 | 现状（pi-sdk） | DeepAgents |
|---|---|---|
| 任务规划 / todo | plan-workflow skill（`plan_record`/`plan_check` 写 6 分区计划） | `write_todos` / `read_todos`（opt-in） |
| 子 Agent 委派 | 无（单会话） | subagents / task 工具 |
| 上下文压缩 | 无（靠热窗口裁剪） | summarization middleware / context offload |
| 文件读写 / 命令 | 内置 `edit`/`write`/`bash`（**直接在宿主工作区**） | filesystem backends + 可选 shell backend |
| MCP 工具 | `createMcpExtension`（stdio，`.pi/mcp.json`） | 原生 MCP |
| 运行时 | 自有 agent loop | 编译为 `CompiledGraph`，跑在 LangGraph runtime |

- 结论：**能力不缺，缺的是隔离与可恢复性**（见 ADR-0014 §1.2 G-a/G-d）。
- 若替换：必须同时满足「MCP 化工具 + 沙箱化执行」，否则等于把宿主 shell 风险从 TS 迁到 Python，收益为负。
- `LocalShellBackend` 类能力**禁止**在宿主机裸开放（与文档 §6 一致）。

### D5 协议层：MCP 是唯一「工具」协议；A2A 按需，不预建

- MCP = Agent ↔ 工具/数据（Git、CI、测试、DB、知识库），本项目已有 3 个 stdio MCP server（`game-engine-mcp` / `dev-task-mcp` / `docs-mcp`）。
- A2A = Agent ↔ Agent（跨框架、第三方），需要 `agent-card.json` + 身份认证。
- **本项目当前没有跨 Agent 需求**：chat / game / dev 三线均由同一主持人编排，工具在 MCP 侧已足够。A2A 待 T3 成立再议（ADR-0014 §4.5 P3）。

### D6 禁止双真相源

- 业务状态真相源：SQLite（`dev_tasks` / `dev_task_events`），写入收敛到 Rust。
- 传输：Redis Streams（削峰、重试、顺序）。
- 执行层状态（checkpoint）：执行层私有存储，**不与业务库同库同写者**。
- 任何「n8n 状态 / LangGraph 状态 / SQLite 状态」三者互为镜像的方案**不予采纳**（历史上 ADR-0001 已确立单写者边界）。

### D7 安全边界（与文档 §6 对齐，并补本项目实际缺口）

1. dev 执行当前**直接跑在宿主工作区**（`PROJECT_ROOT` + `bash`）——这是**现状最大风险**，与是否引入新栈无关；隔离（git worktree + 容器/沙箱）必须先于「接更多执行能力」落地（ADR-0014 §4.2 P0.5）。
2. 合并 / 部署 / 数据库变更：**人工门禁**。门禁实现可以是 QQ `#确认`（现状），也可以是 n8n 审批节点（未来）；**n8n 不是门禁的唯一形式**。
3. MCP stdio 只在本机进程间；若要给远程执行层用，走 HTTP/SSE 代理 + 认证，**禁止直接把本地 stdio server 暴露公网**。
4. 审计记录中不长期保存完整群消息正文（n8n 侧只留 `task_id` + 脱敏摘要）。

---

## 3. 方案对比（为什么「不整体引入」）

| 维度 | 现状（Node pi-sdk + Rust 状态机） | 文档方案（n8n + LangGraph + DeepAgents） |
|---|---|---|
| 运行时数量 | Node + Rust + Redis + SQLite（+ 本地 llama-server） | 上者 + Python + LangGraph Server + n8n + （通常）Postgres/队列 |
| 状态权威 | Rust 状态机（单一） | Rust 与 LangGraph **双状态机**（文档未消解，见 R1） |
| 断点续跑 | 会话文件 + 迁移表，**无租约/重试**（弱） | Checkpointer 原生（强） |
| 跨进程恢复 | ❌（`void run…`，ACK 即丢） | ✅ |
| 并行多任务 | ❌（同一子会话串行锁） | ✅（thread 级） |
| 工具协议 | MCP stdio ✅ | MCP ✅（需 transport 适配） |
| 沙箱 | ❌（宿主 shell） | 可选（文档亦仅「推荐」） |
| 外部触发 | ❌（只有 QQ） | ✅（Webhook / Issue / 定时） |
| 审批 UI | QQ 文本命令 | n8n 表单 + QQ 通知 |
| 单人维护成本 | 低 | 高（4 语言/5 进程/版本矩阵） |
| 许可 | 全开源 | n8n 为 fair-code「Sustainable Use License」（非 OSI，**待验证**） |

**判断**：文档方案解决的是「**执行可靠性与外部触发**」两类问题，而不是「Agent 不够聪明」。这两类问题在现状栈内**可以分步解决**（ADR-0014 P0），且成本远低于换栈。因此本 ADR 取「原则采纳 + 触发条件引入」。

---

## 4. 现状映射（文档组件 → 本项目已有实现）

| 文档组件 | 本项目等价物 | 位置 | 结论 |
|---|---|---|---|
| 群消息消费服务 | qq-gateway + bot-adapter + agent-gateway | `apps/qq-gateway`、`crates/bot-adapter`、`apps/agent-gateway` | 保留 |
| 消息缓冲队列 / 持久化 | Redis Stream `internal`（消费者组 + ACK）+ SQLite `dev_tasks` | `docs/conventions/persistence.md` | 保留，需补「任务级」队列（ADR-0014 P0） |
| n8n 触发与编排层 | 显式命令（`parseDevCommand`，人=100% 置信度）+ 决策模型只在「发起」上给建议 | `src/dev-command.ts`、`src/index.ts` | **等价但弱**（无外部触发） |
| n8n 审批节点 | `#确认` / `#取消` → `confirm_task` / `cancel_task` | `crates/dev-task`、`src/index.ts` | **等价** |
| n8n 通知节点 | `pushReply`（被动）+ `pushActive`（计划/结果） | `src/index.ts` | 等价（口径见 ADR-0009） |
| LangGraph 编排 + 检查点 | `crates/dev-task` 迁移表 + `SessionManager` 会话文件 + per-session 串行锁 | `crates/dev-task/src/lib.rs`、`src/dev-session.ts` | **语义等价，耐久性弱** |
| LangGraph interrupt/resume | `awaiting_confirm` / `verifying` + `runDevResume` | 同上 | **语义等价** |
| DeepAgents 执行 harness | pi-sdk 子会话（plan-workflow skill + `codemode` + MCP + `bash`） | `src/dev-session.ts` | **能力等价（隔离弱）** |
| MCP 协议层 | `game-engine-mcp` / `dev-task-mcp` / `docs-mcp`（stdio） | `crates/*-mcp` | 保留（唯一工具协议） |
| A2A 协议层 | 无 | — | 按需（P3） |
| A2A Task ID / thread_id | `sessionId`（= 计划文档 id = web 页标识 = `dev_tasks.session_id`） | — | **已有统一键**，直接复用 |
| Rust 协议适配层 | server（内部 HTTP）/ Rust 状态机 | `crates/server`、`crates/dev-task` | 已是权威侧 |

> 关键结论：文档里的「新架构」在本项目**大部分已有落点**，真正的增量只有三件事：**任务队列 + 租约**、**执行隔离**、**外部触发/审批 UI**。前两件不需要 n8n 或 Python；第三件才是 n8n 的正当用途。

---

## 5. 差距（与三层架构相关的，细节见 ADR-0014）

| 编号 | 差距 | 影响的层 | 是否可由现状栈补 |
|---|---|---|---|
| G-a | dev 执行是 `void run…` fire-and-forget：`handle()` 返回即 `XACK`，进程崩溃则执行权丢失，任务卡在 `planning/running` | 编排 | ✅（队列 + 租约 + 重试） |
| G-b | 执行与消息消费同进程：长任务占住消费循环 / 重启全断 | 编排 | ✅（独立 worker） |
| G-c | 无 A2A、MCP 仅 stdio，远程/多机不可达 | 协议 | ⚠️ 需要时再做 |
| G-d | dev 执行**无隔离**：宿主 `bash` + 直接改 `PROJECT_ROOT` | 执行 | ✅（worktree + 容器） |
| G-e | 审计粒度不足：只有 `dev_tasks.status` 终态，无事件流水（谁在何时批准什么） | 触发/编排 | ✅（`dev_task_events`） |
| G-f | 无外部触发（Issue/PR/定时/CI），只能群内发起 | 触发 | ❌ 只能 n8n/自建 webhook |

---

## 6. 引入触发条件（决定各层是否上线）

| 触发条件 | 含义 | 满足后才允许引入 |
|---|---|---|
| **T1** | 需要 **非 QQ 触发**（GitHub Issue/PR、定时、CI 失败重跑） | n8n（或等价 webhook 服务） |
| **T2** | 需要 **跨进程/跨机恢复、多 worker 并行、任务级重试** | 独立 worker + 租约（先做，见 ADR-0014 P0；仍不足再谈 LangGraph） |
| **T3** | 需要 **对外暴露 Agent 能力**或跨框架 Agent 协作 | A2A |
| **T4** | 需要 **非文本审批 UI**（表单/多人会签/审批 SLA） | n8n 审批节点（或自建 web 审批页） |

- T2 是**现在就该做**的（缺口真实存在，且不需要新栈）。
- T1/T4 是「想要」而非「现在痛」，n8n 的价值集中在这两条。
- T3 无需求 → 不做；避免「先建协议再找场景」。

---

## 7. 影响

**正面**

- 明确「权威在哪里」：状态机在 Rust，执行层可替换，触发层可替换，互不污染。
- 挡住一次高成本换栈：不需要为「更工程化」而同时维护 Python/Node/Rust/Docker 四套工具链。
- 为后续引入留出干净切口：worker（P0）→ LangGraph 执行层（P1）→ n8n 触发/审批（P2）→ A2A（P3），每步可独立回滚。

**代价 / 风险**

| 编号 | 风险 | 缓解 |
|---|---|---|
| R1 | **双状态机**：Rust 迁移表 vs LangGraph 图；两者都改状态必冲突 | 只允许 Rust 迁移状态；LangGraph 侧调用 MCP `transition_task` 请求迁移（ADR-0014 §2 D4、§4.3） |
| R2 | **双 checkpointer**：pi 会话文件 vs LangGraph checkpoint，迁移期同 `sessionId` 不可双驱动 | 用 `DEV_EXECUTOR=pi\|langgraph` 单开关硬隔离，禁止灰度并存 |
| R3 | 延迟：n8n/LangGraph 网络跳数进入「受理回执」路径 | 回执留在同步路径（现状 <1s），异步段才进队列 |
| R4 | 依赖膨胀 / 单人维护成本 | §6 触发条件作为准入门槛；n8n 默认不启动 |
| R5 | DeepAgents 生态变动快（部分能力仍标 Beta） | 锁版本 + 只用稳定面（filesystem/todo/subagents/MCP） |
| R6 | 数据合规：n8n 执行记录保留消息正文 | 只存 `task_id` + 脱敏摘要（D2.4） |
| R7 | 「两套并行」长期不收敛 | 每阶段设**退出条件**（ADR-0014 §4.6 阶段汇总），到点不达标即回退 |

**明确不做**

- 普通消息（chat / game）链路不接 n8n（与文档 §3.3 一致）。
- 不让 n8n 管理 checkpoint、不复制白名单/状态校验到 Python。
- 不为「架构完整性」预建 A2A 端点。

---

## 8. 依据与验证状态

| 事实 / 数值 | 取值 | 来源 | 验证状态 | 复核日期 |
|---|---|---|---|---|
| 群聊被动回复窗口 / 次数 | 5 分钟 / 每条 5 次 | 官方「消息收发概述」→ `docs/conventions/external-constraints.md` | 已验证 | 2026-10-11 |
| 主动消息配额（群） | 1000 条/群/天、60qpm、单关系 20qpm、≤5/s | 同上 | 已验证 | 2026-10-11 |
| DeepAgents 构建于 LangGraph runtime（durable execution / HITL） | 「standalone library built on top of LangChain's core building blocks…uses the LangGraph runtime for durable execution, streaming, human-in-the-loop」 | LangChain 官方文档《Deep Agents overview》 | 已验证（原文摘录） | 2026-10-20 |
| DeepAgents 能力面（task planning `write_todos`、subagents、summarization、filesystem backends、MCP） | 同上 | LangChain 官方文档 | 已验证（页面能力清单） | 2026-10-20 |
| DeepAgents backend 具体类名（`StateBackend` / `FilesystemBackend` / `StoreBackend` / `CompositeBackend` / `LocalShellBackend`） | — | 方案文档 §5/§6 引用 | 待验证（未核对 API 文档） | 2026-10-20 |
| LangGraph `interrupt()` 恢复时**从节点开头重跑** | — | 方案文档 §4 注 | 待验证（需实测确认副作用幂等要求） | 2026-10-20 |
| n8n 许可 | 「Sustainable Use License」（fair-code，非 OSI） | 方案文档未涉及；n8n 官方文档（页面为 JS 渲染，未取到原文） | 待验证 | 2026-10-20 |
| n8n / LangGraph Server 常驻资源占用 | 未知 | — | 待验证（若采用 P2 需实测） | 2026-10-20 |
| dev 执行崩溃丢执行权 | 现象已确证（`void run…` + `handle()` 返回即 `XACK`） | 代码实测（`src/index.ts` `handleDevCreate` / `processEntries`） | 已验证 | 2026-10-20 |

---

## 9. 待确定事项

- **TD-13.1**：是否真的有「非 QQ 触发」需求（T1）？若只是自己用，Issue/PR 触发的边际收益可能低于 n8n 的运维成本。
- **TD-13.2**：`DEV_EXECUTOR` 迁移是否值得？pi-sdk 的 cwd/技能/计划文档耦合较深，换 DeepAgents 等于重写 plan-workflow（计划文档 6 分区、`plan_record` 语义需在 Python 侧重实现）。
- **TD-13.3**：checkpointer 选型（若 P1）：SQLite（单机、无额外依赖）vs Postgres（多机）；倾向 SQLite 独立文件。
- **TD-13.4**：审批是否要「多人会签 / 超时自动驳回」？现状单人单确认；若只是单人用，n8n 审批节点收益低。
- **TD-13.5**：执行隔离形态：git worktree（轻）vs 容器（重）vs worktree + 容器（最稳）；与 G-d 强相关，需在 P0 之后单独决策。
- **TD-13.6**：模型成本：DeepAgents 的 todo/子 Agent/摘要会显著放大 token；本项目 LLM 为远程计费，需先量。

## 10. 下一步

- 落地分析、差距细则与分阶段实现计划（含文件级改动点、验证、回滚、退出条件）见 **ADR-0014**。
- 本文与 ADR-0014 均为**探讨稿**：不改代码、不建开发计划、不写 `【主动消息已人工确认】` 标记；若后续立项，须先回答 §9 的 TD。
