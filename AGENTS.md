# AGENTS.md — 桌游助手（QQ Bot Board Game Assistant）

> 本文件是项目入口。请**按需加载**（渐进披露），不要一次性读完所有文档。

## 项目是什么

基于 **QQ 群机器人**（官方 bot.q.qq.com API v2）的桌游助手：

- 接入 QQ 群，将群消息路由到各桌游引擎
- Web **独立站点**展示游戏公共资源与玩家个人资源（短期 token 鉴权）
- 首个桌游引擎：「拉斯维加斯」

## 技术栈

- **Node.js (TypeScript)**：QQ 接入 `qq-gateway`、资源展示 `web`（Express）
- **Rust (Axum)**：bot-adapter / game-engine-core / engines / auth / server（单进程多模块）
- **Redis**（Streams 总线 + 缓存）、**SQLite**（持久化）

## 按需加载索引

| 你要做什么 | 先读 |
|---|---|
| 了解整体架构 | `docs/architecture.md` |
| LLM 思考开关 / 联网形式（chat / game / dev-task） | `docs/architecture.md` §3.1 |
| 内部消息事件协议 | `docs/conventions/message-protocol.md` |
| 交互原语（单选/多选/分区/级联/接力，端侧交互语义） | `docs/conventions/interaction-primitives.md` |
| 持久化 / 消息总线约定 | `docs/conventions/persistence.md` |
| 外部约束事实登记表（频控/窗口/延迟） | `docs/conventions/external-constraints.md` |
| 小游戏 16 块速查总览（最终口径） | `docs/games/las-vegas/small-games-overview.md` |
| 小游戏奖励期望值评估（平衡） | `docs/games/las-vegas/balance-evaluation.md` |
| 小游戏 16 阶段对齐计划 / 进度 | `docs/games/las-vegas/small-games-alignment.md` |
| 三国杀立项：阶段计划 / 数据模型 | `docs/games/sanguosha/development-plan.md`、`docs/games/sanguosha/data-model.md` |
| 三国杀规则 / 牌表 / 状态机 / 动作 / 校验 | `docs/games/sanguosha/rules.md`、`cards.md`、`state-machine.md`、`legal-actions.md`、`rule-engine-permission.md` |
| 三国杀视图 / 布局 / 手势 / 浮层 / 观星交互 | `docs/games/sanguosha/ui-spec.md` |
| 三国杀 Web 真机验收清单（人工） | `docs/games/sanguosha/p3-manual-test.md` |
| 三国杀技能注册表 / 触发钩子 / 实现进度 | `docs/games/sanguosha/skills.md` |
| 三国杀规则裁定 / 特殊交互区 | `docs/games/sanguosha/rules-rulings.md`、`docs/games/sanguosha/special-zones.md` |
| 三国杀武将抓取数据（696 位） | `docs/games/sanguosha/data/README.md`、`docs/games/sanguosha/data/heroes.json` |
| 三国杀武将表（自动生成） | `docs/games/sanguosha/generals.md` |
| MCP 统一原语 / 能力描述符规范 | `docs/conventions/mcp-primitives.md` |
| 本地开发环境（Docker/Redis） | `docs/dev-environment.md` |
| 编码规范 | `docs/conventions/coding-standards.md` |
| 某模块职责/接口 | `docs/modules/<模块>.md` |
| 道具中台（P1，三国杀） | `docs/modules/props-admin.md` |
| 版图页 / 素材流水线 | `docs/modules/board-view.md`、`docs/games/las-vegas/props/asset-mapping.md`、`board-layout.md` |
| 长期记忆 / 计划 / 决策 | `memory_palace/README.md` |
| 当前计划与验收清单 | `memory_palace/plan/` |
| 开发进度 / 里程碑 | `memory_palace/milestones.md` |
| 工作区规则（rules） | `.pi/prompts/`（用 `/` 命令加载） |
| 专项技能（skills） | `.pi/skills/`（用 `/skill:name` 加载） |

## 工作约定

- rules 位于 `.pi/prompts/`，用 `/架构` 类命令加载；skills 位于 `.pi/skills/`
- 修改架构先更新 `docs/architecture.md` 与相关 `docs/modules/` 文档
- 重大技术决策记入 `memory_palace/decisions/`（ADR）
- 新增桌游引擎：在 `crates/engines/<name>/` 建 crate，并在 `crates/game-engine-mcp` 注册；再往 `apps/agent-gateway/src/games.ts` 的 `GAME_OPTIONS` 追加一项（`#开局` 菜单自动出现）
- **道具中台边界**：`apps/props-admin` 与 `/internal/gameprops/*` 在 P1 **仅本机**（Vite dev/preview + server 127.0.0.1）；**不挂公网 web/tunnel**，待 P8 完成游戏权鉴后再暴露。
- **主动消息门禁**：方案含主动消息（proactive / `pushActive` / 无 `reply_msg_id`）必须**人工确认**并写入 `【主动消息已人工确认】`，`npm run check:proactive-gate` 校验（见 `docs/conventions/coding-standards.md`、ADR-0009）
