# ADR-0020：多端交互形态 —— Web 优先 + WebView 复用 + 游戏区域三分

- **状态**：**已采纳**（2026-10-30；P3 已按本 ADR 落地 Web 三分区页面与 server 动作宿主）
- **相关**：`docs/games/sanguosha/plan.md`、`docs/games/sanguosha/development-plan.md`（P3/P7/P8）、`docs/games/sanguosha/data-model.md`（§4.2）、ADR-0010（版图渲染/双模式）、ADR-0011（移动端形态 PWA/Tauri）、ADR-0012（交互原语）、ADR-0016（引擎内核）、ADR-0017（中台）
- **结论一句话**：三国杀的**主交互面是 Web 页**，多端通过 WebView 复用；页面按**公共局面区（只读）/ 玩家个人资源区（交互基座）/ 特殊交互区**三分；**Q 群后置**，只做开局设定、轮次/状态查询与牌局链接；**完整权鉴放到上线阶段**。

## 背景

1. 立项文档原方案是「Q 群推动游戏进程 + Web 展示状态」。实际评估后用户裁定：**Q 侧优先级后置**，Web 页提前，并应在武将设计工作台之前完成。
2. 三国杀交互复杂度远高于拉斯维加斯：手牌/装备/判定区/技能选择/观星/五谷丰登等，群消息逐动作推动体验差、消息频控压力大。
3. 项目已有 Web 经验（ADR-0010：纯 TS 视图 + 短 token + 轮询；ADR-0011：PWA 优先、Tauri 2 壳加载线上页、不启用 IPC），可直接复用「WebView 加载线上页」的多端路径。
4. 拉斯维加斯当前信息公开，未真正验证「个人资源隔离 + 权鉴」；三国杀手牌/身份必须隔离，权鉴模型需要专项设计。
5. Q 群仍有价值：拉人、开局、查看轮次、拿链接；不需要承担局面播报与逐动作交互。

## 决策

### 1. 交互面：Web 优先，Q 精简

| 端 | 职责 | 不做什么 |
|---|---|---|
| **Web**（`apps/web` 三国杀页） | 公共局面展示；个人资源区交互；特殊区交互；轮询刷新；短 token | 不直连 SQLite；不承载规则校验 |
| **WebView**（Tauri 2 / 原生壳） | 加载同一 Web 页；不启用平台专有 API；后续按需桥接 | 不复制一套前端 |
| **Q 群** | 开局设定（`#开局 三国杀` / `#加入` / `#开始` / `#中断`）；轮次/状态查询（`#局面`）；牌局链接；`#恢复` 重签 | **不播报公共局面**、不做逐动作交互、不做 LLM 局面叙述 |
| APP（未来） | 同 Web/WebView，按需增强 | 首期不建原生 App（ADR-0011） |

- Web 页在 **P3** 完成，**先于工作台（P5）**；工作台预览复用同一套视图组件。
- Q 群接入放 **P7**（后置）；引擎交互原语（`many` min/max、多目标、排序、响应队列）**先行在 P2.4 实现**，Q 只是后续命令映射。

### 2. 视图三分区（硬契约）

| 区 | 内容 | 读写 |
|---|---|---|
| **公共局面区 `public_board`** | 座位顺序、公开身份、武将/体力/装备/判定区、弃牌堆、牌堆余量、当前轮次/阶段、公共特殊区 | **只读** |
| **玩家个人资源区 `player_area`** | 本人手牌明细、本人技能与可用动作、个人 pending、加密选项代码、私有标记 | **交互基座**（仅本人） |
| **特殊交互区 `special_zones`** | 观星牌堆顶、五谷丰登候选/选择、判定展示、濒死求桃、无懈链等，按技能声明的可见性 | 被授权玩家 / 公共（按声明） |

- 引擎内部保存完整状态；`get_state` / `get_player_view` 按上述三区投影（`data-model.md` §4.2）。
- 动作只能基于 `player_area` / 授权 `special_zones` 发起；公共区不可提交任何动作。
- 视图契约是**跨端唯一契约**：Web / WebView / 未来 APP 都消费同一结构；Q 链接进入的也是同一 Web 页。

### 3. 权鉴分层（后置但先留位）

- **现阶段**（P2–P7）：沿用短期 token（`auth` 服务，绑定 openid + 资源范围）；个人资源区靠 token 隔离；越权 403。
- **上线阶段**（P8）专项：
  - 梳理 token scope（`game:{room_id}` 是否足够；是否需要 `player:{openid}` / 区域级 scope）；
  - 动作级校验：服务端 + 引擎双重（引擎已是最后防线）；
  - 链接泄露/转发、token 过期、多人共用设备等场景用例；
  - 结论回写本 ADR 或新 ADR。
- 不因「拉斯维加斯信息公开」而默认三国杀也公开；三国杀按**默认私有**设计。

### 4. 兼容与演进

- Web 页要求：响应式布局、无浏览器专有 API 依赖、可被 WebView 加载；轮询沿用 10s + 激活即刷；无 JS 降级文本。
- 交互提交先按「单动作 + pending 步进」设计；批量 `execute_actions` 作为后续优化（ADR-0012 前置缺口），不阻塞首版。
- 特殊区为通用占位（`special_zones`），新增技能只需声明区域与可见性，不改视图契约；**所有特殊区必须登记到 `docs/games/sanguosha/special-zones.md`**（用户 2026-10-25 裁定）。
- 可见性当前实现 `owner/public`，**预留 `all/team`**（未实现）；观星排序（`ordered`）在 P2 补原语、P3 实现 Web 拖拽。

## 理由（取舍）

| 方案 | 结论 |
|---|---|
| Q 群逐动作推动（原方案） | 否/后置：消息频控、隐藏信息、复杂选择体验差；用户 2026-10-25 裁定后置 |
| Q 群 + Web 双主交互面 | 否：两套交互都要维护，测试成本翻倍 |
| 每个端各写一套前端 | 否：视图契约已统一，WebView 复用成本最低 |
| 只做 Web、不做 Q | 否：Q 仍承担拉人/开局/状态/链接，链路需要 |
| **Web 主交互 + WebView 复用 + Q 精简 + 区域三分** | **采纳**：体验、维护成本、隐藏信息与后续权鉴都可控 |

### 5. 动作执行宿主与投影（2026-10-28 补充，用户裁定）

- **Web 动作由 server 执行（方案 A）**：`web POST /tabletop_game/:gameId/action` → `server POST /internal/game/{gameId}/{roomId}/action`（仅本机）→ 引擎执行 → `game_events` + `game_state` 快照。
  - 理由：MCP 是 gateway 以 stdio 拉起的进程，Web 无法直接复用；storage 无 CAS（last-write-wins），必须**单写者**。
  - 约定：P3–P6 三国杀动作只由 Web/server 提交，MCP 仅建局/种子局（不并发）；P7 接入 Q 时统一走同一执行入口。
- **引擎投影不是权限**：`/resources/game/{gameId}` 必须按 `game_id` 分派到引擎 `public_state` / `player_view`（P3 前只认 las-vegas，其它引擎原样下发内部状态 → 会泄漏手牌/身份/牌堆顺序）。
- **权限（QQ 扫码登录 → openId 认证、token scope、动作级校验）仍按 §3 推到 P8**；P3 沿用 openid 绑定短 token + `game:{room_id}` 范围。

## 影响

- **引擎**：`public_state` / `player_view` 按三区投影；`special_zones` 纳入契约；新增交互原语（P2.4）。
- **Web**：新增三分区页面与交互 API；工作台预览复用；WebView 冒烟测试。
- **agent-gateway**：三国杀不启用局面叙述；`#局面` 只回轮次/阶段/链接；Q 命令在 P7 落地。
- **auth/server**：P8 评估 token scope 与动作级校验；可能需要新增区域级 scope。
- **道具中台**：P1 仅本机（Vite dev/preview + server 127.0.0.1），**不随玩家 Web 挂公网**；P8 权鉴完成后再暴露 `/props-admin` 与 `/internal/gameprops`。
- **文档**：`data-model.md` §4.2、`development-plan.md` P3/P7/P8 与本 ADR 同步；`docs/architecture.md` 的端侧描述后续按实现更新。
- **风险**：Q 后置会弱化群内体验（用户已确认）；WebView 兼容与权鉴需专项验证。

## 实施进度（2026-10-28，P3 完成）

- **Web 页面**：`apps/web` 三国杀页（三分区 + 浮层 + 手势 + 观星拖拽）已实现；SSR + 10s 轮询；无 JS 文本态含动作表单（`arrange:<csv>` 文字提交实测）。
- **动作执行**：按 §5 补充——server 内置执行（`/internal/game/{gameId}/{roomId}/action`）+ web 转发；单写者约定执行中。
- **权限**：仍用 openid 绑定短 token（`game:{room_id}`）；QQ 扫码登录 → openId 认证与动作级鉴权按 §3 留 P8。
- **验证**：四档布局无溢出、隐藏信息不泄漏、观星/五谷/无懈/拿牌链路 e2e；真机（Android/iOS）待人工。

## 依据与验证状态

- 现状 Web 能力（SSR + 轮询 + 短 token + WebView 可加载）：`docs/modules/web.md`、ADR-0010/0011，**已验证**。
- 「拉斯维加斯信息公开、权鉴未实测」：本仓库现状 + 用户说明，**已验证（事实）**；三国杀权鉴模型**待 P8 验证**。
- WebView 兼容性（Tauri 2 / Android WebView 加载线上页）：ADR-0011 已选型，但三国杀复杂交互下的性能/兼容**待 P3 实测**。
- Q 群 5 条/消息、5 分钟窗口：来源 `docs/conventions/external-constraints.md`，**已验证**；本 ADR 通过「Q 不逐动作」降低其影响，不新增外部数值约束。
