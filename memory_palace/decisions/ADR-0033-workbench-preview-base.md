# ADR-0033：工作台基座（模板 / 预览 / AI 调试 / 决策日志）与预览隔离

- **状态**：已采纳（2026-11-03）
- **相关**：`docs/games/sanguosha/plan.md`（三期需求）、`docs/games/sanguosha/development-plan.md` P5、`docs/games/sanguosha/data-model.md` §5、ADR-0016（规则内核刚性）、ADR-0017（中台）、ADR-0018（工作台与 n8n）、ADR-0020（Web 三分区）、ADR-0031（Web 跨端登录）、ADR-0032（AI 分层）
- **结论一句话**：本轮工作台先做**基座四件套**（模板 / 预览 / AI 调试 / 决策日志），预览是**独立内存态仿真**，不写任何对局真相源；n8n/Vue Flow 有向图编排留到武将批量验证之后另立 ADR。

## 背景

1. 用户 2026-11-03 裁定：P8 之后进入「工作台 + 预览基座」，用于批量验证武将；本轮只做模板/预览/AI 调试/决策日志，不做图编排。
2. 三国杀武将描述不严谨、规则复杂，需要能**指定武将/手牌/装备/阶段/牌堆顶**的最小局面，并在真实引擎里逐步执行、看合法动作与 AI 决策。
3. 若预览直接复用线上 `GameManager` / `game_state` / `game_events`，会污染真相源、干扰真实对局、让回放/决策日志失真；必须强隔离。
4. 引擎已有 `EnginePlugin`（`new_game` / `legal_actions` / `execute_action` / `public_state` / `player_view`），但缺少「校验快照」与「场景注入」原语。

## 决策

### 1. 范围（本轮）

| 做 | 不做（后续另立 ADR） |
|---|---|
| `game_state_templates` 模板 CRUD | n8n / Vue Flow 有向图编排 |
| 结构化 `SceneSpec` 场景注入 | 图 → 代码生成 |
| 独立内存预览会话（create/action/drive/reset/delete） | 预览状态持久化 / 跨重启恢复 |
| AI 单步/连推 + 决策轨迹展示 | 多用户并发预览协作 |
| 决策日志只读查询 + 导出 | 模板公网分享 |

### 2. 预览隔离（硬约束）

- 每个预览会话一个**独立 `GameManager`**（共享 `EngineRegistry`，不共享房间表）；不进入 `AppState.games`。
- 房间号强制 `preview:<uuid>`，`group_id` 同步改写；**不注册 `active_by_group`**，不影响同群真实对局。
- **不写** `game_state` / `game_events` / `game_decision_log`；步骤时间线只存在会话内存（≤500 步）。
- 会话空闲 TTL 默认 2h、单进程上限 8 个（`SGS_PREVIEW_TTL_SECS` / `SGS_PREVIEW_MAX_SESSIONS`）。
- 准入：`/internal/game/preview/*` 仅本机 + `require_admin`（共享 token 或白名单 actor）；Web 端先过 `propsSession` 再代理，浏览器不直连 server。

### 3. 引擎原语（新增）

`EnginePlugin` 增加两个默认方法（其它引擎可不实现）：

- `validate_state(&GameState) -> Result<(), String>`：schema/座位/资源守恒/武将技能 code 校验；
- `apply_scene(&GameState, &Value) -> Result<GameState, String>`：按引擎自定义 `SceneSpec` 注入场景，返回新状态。

三国杀实现：

- 校验：`schema_version`、5–8 人、players↔seats 一致、未知武将/技能、牌实例引用唯一且全覆盖、`check_conservation`。
- 场景：座位（武将/身份/体力/上限/存活/机器人/横置/翻面/手牌/装备/判定区/标记/flags/额外技能）、回合/阶段/轮次、牌堆顶、随机种子、本回合计数器。
- 资源只做**移动**：手牌/装备/判定/牌堆顶的卡牌只能从摸牌堆/弃牌堆/移出区取（牌实例 id 或定义码），不凭空造牌；注入后再次守恒校验。

### 4. 数据与接口

- `game_state_templates(id, game_id, name, source, snapshot, meta, created_by, created_at, updated_at)`；`meta.scene` 存场景注入；`source` = `manual` / `room:<room>` / `replay:<room>@<seq>` / `duplicate:<id>`。
- 模板 API：`/internal/gameprops/templates`（列表/详情/创建/更新/删除/复制/从对局抓取/从回放抓取）。
- 预览 API：`/internal/game/preview`（列表/创建/场景校验）、`/{id}`（GET/DELETE）、`/{id}/action|drive|reset`。
- 预览返回与 `/resources/game` 同形的三分区视图 + `actions`，Web 预览页直接复用 P3 渲染与交互。
- 决策日志只读：`GET /internal/gameprops/decision-logs?game_id=&room_id=&since_seq=&limit=`，`player_id` 输出 sha256 前 16 位（与 `sgs:export-decisions` 一致）。

### 5. 前端

- `apps/props-admin` 新增 `/workbench` 四 TAB（模板 / 预览 / AI 调试 / 决策日志），预览 TAB 用 iframe 嵌 `/tabletop_game/:gameId/preview`。
- Web 预览页走 `propsSession`（`tg_props` 会话 + 白名单），**不要求** `game:` token；页内工具栏支持视角切换、AI 单步/连推、重置。
- 预览页只用于本机/白名单运维，不出现在玩家入口；不新增任何主动消息。

## 影响

- 新增 `crates/storage/src/templates.rs`、`crates/server/src/{preview,template_routes}.rs`、`crates/engines/sanguosha/src/scene.rs`、`apps/props-admin/src/pages/WorkbenchPage.vue`。
- `game-engine-core` / `sanguosha` / `server` / `web` / `props-admin` 均有改动；不影响线上对局写路径。
- 预览会话重启即丢（内存态），属预期；模板持久化在 SQLite。
- **开发环境限制**：Vite 中台（5174）与 web（18080）跨站，`SameSite=Lax` 的 `tg_props` 不会随 iframe 发送；本地走工作台预览请用同源构建页 `http://127.0.0.1:18080/props-admin/workbench`（先 `npm run build:props`）。生产环境 `/props-admin` 与预览页同源，无此问题。
- 模板可含他人手牌/牌堆顺序（隐藏信息）：仅本机/白名单可见；导出默认由人工确认，不做公网分享。

## 依据与验证状态

- 预览隔离、模板 CRUD、场景注入、动作/AI/重置/销毁、鉴权负向、真相源行数不变：`npm run check:sgs-preview`（临时 server + MCP，**已验证通过**）。
- 页面链路（预览页工具栏 / 工作台四 TAB / iframe / 视角切换 / 未登录 403 / AI/重置）：`npm run check:sgs-preview:ui`（临时 web + Playwright，**已验证通过**；缺浏览器时自动 skip）。
- 引擎场景/校验单测：`crates/engines/sanguosha/src/tests/scene.rs`（4 条，**通过**）。
- 模板存储单测：`crates/storage/src/templates.rs`（CRUD/复制/删除/校验，**通过**）。
- 数值约束（TTL 2h / 8 会话 / 500 步）均为**内部工程默认值**，非外部平台约束，可配。
- n8n/Vue Flow 能力与性能：本轮不验证（按用户裁定延后）。
