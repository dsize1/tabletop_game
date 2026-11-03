# ADR-0034：工作台「创建对局」（沙盒开局）——复用生产开局后半段，只产预览会话

- **状态**：已采纳（2026-11-03）
- **相关**：ADR-0018（工作台与 n8n）、ADR-0033（工作台基座与预览隔离）、`docs/modules/props-admin.md`、`docs/modules/game-engine.md`、`docs/games/sanguosha/development-plan.md` P5
- **结论一句话**：工作台新增「创建对局」抽屉表单；按 5–8 人、逐座位指定武将/身份（全 bot）生成一局**已开局**状态，**只创建预览会话**（可选保存模板），复用引擎生产开局的 `finish_setup`，**不改 Q 群消息驱动开局流程、不写对局真相源**。

## 背景

1. 工作台此前只能「抓取当前对局」或从回放生成模板，无法从零构造「指定武将/身份、全 bot、跳过选将、主公先手、随机牌堆/手牌」的验证局面。
2. 武将批量验证需要稳定、可复现的最小局面；手动 lobby/选将流程无法精确指定身份，且会受随机候选池影响。
3. 用户 2026-11-03 明确：座位范围 #1–#N；创建预览会话、不改现有实现；身份必须按标准配比；武将使用默认 149 去重池；不支持包选择（全量武将/全量卡牌）；支持随机种子；全 bot 后不自动连推；指定武将的开局技能照常执行。
4. 如果另起一套「沙盒开局」逻辑，会与生产 `finish_setup` 分叉（牌堆、手牌、开局技能、主公先手容易不一致）；必须在生产路径上复用。

## 决策

### 1. 边界

| 做 | 不做 |
|---|---|
| 工作台预览会话（`preview:<uuid>`） | 真实线上房间 / 写入 `game_state` / `game_events` |
| 复用 `finish_setup` 生产开局 | 修改 Q 群 `#开局/#加入/#开始` 行为 |
| 可选保存模板（`game_state_templates`） | 图编排 / 代码生成 |
| 逐座位指定武将/身份（标准配比） | 任意身份组合（破坏主公/胜负规则） |
| 随机种子复现 | 包选择（暂不支持） |

### 2. 引擎

- 将 `finish_setup` 的可见性改为 `pub(crate)`（**仅可见性**，逻辑零改动）；沙盒在 crate 内复用同一函数：
  1. `SanguoshaEngine::new_game` 建 lobby 骨架（复用 state 字段/默认 `Config`/seed）；
  2. 按 `SandboxSpec` 设置座位 `player_id`（默认 `bot:1..N`）、`is_bot=true`、身份、`setup.picks`；
  3. 调 `finish_setup`：定体力、建牌堆、随机洗牌/发初始手牌、跑开局技能、主公先手、`resolve::advance`。
- 身份：从标准配比 `identity_list(n,false)` 中扣除用户指定项（超配比报错），剩余随机补足，保证恰好 1 主公。
- 武将：默认 **149 去重池**（全量武将包 + 技能已实装 + 同名「界优先」）；指定项必须命中池且不重复，剩余随机补足。
- 牌堆：`Config.packs = standard+junzheng`（引擎全部牌堆）；每人初始手牌 4 张；seed 由调用方注入。
- 产出 `GameState` 先过 `scene::validate_state`（守恒/引用/主公唯一等）。
- `sandbox_meta()` 返回表单所需的 149 将 + 身份配比 + 默认包信息。

### 3. Server

- `GET /internal/game/preview/meta`：沙盒表单数据（`require_admin`）。
- `POST /internal/game/preview/sandbox`：`{players, seed?, seats:[{index,general?,identity?}], save_template?, name?}` → 生成状态 →（可选）写 `game_state_templates` → 创建预览会话 → 返回视图 + `seed` + `template_id`。
- 复用现有 `/internal/game/preview/*` Web 代理与 `propsSession` 准入；浏览器不直连 server。
- 预览会话仍不写 `game_state`/`game_events`/`game_decision_log`。

### 4. 前端（props-admin `/workbench` → 模板 TAB）

- 「创建对局」按钮打开 `n-drawer` 抽屉：
  - 玩家数 5–8（必填，默认 5）；
  - 逐行 `#1..#N`：武将（`filterable`，名称不区分大小写子串匹配；其他座位已选 code 置灰）+ 身份（按剩余配比置灰）；
  - 可选随机种子、可选「保存为模板」+ 模板名；
  - 提交后直接切到预览 TAB 并加载 iframe；默认视角 `bot:1`。
- 不自动 AI 连推；由工作台「AI 单步/连推」或代操作。

## 影响

- 新增 `crates/engines/sanguosha/src/sandbox.rs`、`/internal/game/preview/{meta,sandbox}`、props-admin 抽屉表单、harness `sandbox` 段。
- `finish_setup` 仅改可见性；Q 群开局仍走原 `execute_action` 路径，回归门禁覆盖。
- 模板可选保存：快照是**已开局完整状态**（`source=manual`，`meta.sandbox` 记录人数/seed/座位指定），后续可复用/复制。
- 149 去重池意味着原版与界版同名时只能选界版（与正常开局一致）；若后续要「全 232 将」需另立需求。
- 沙盒状态在内存，重启即丢；保存模板后可跨重启复用。

## 依据与验证状态

- 引擎单测：指定/随机、去重、身份配比、负向、确定性、开局技能（陆绩怀橘）——`crates/engines/sanguosha/src/tests/sandbox.rs` **通过**。
- API/UI harness：`npm run check:sgs-preview --ui`（meta/sandbox/负向/隔离/抽屉/iframe）**通过**。
- Q 流程回归：`npm run sgs:check` 19/19（含 `test:sgs-web`、`test:sgs-bot`、`verify:sanguosha`）。
- 数值约束：149 去重池、4 张初始手牌、standard+junzheng 均来自引擎现有常量/配置，不是新增外部约束。
