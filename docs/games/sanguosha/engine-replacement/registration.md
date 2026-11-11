# v2 注册所需接口清单（M4-1，仅登记，不开发）

> 裁定：OQ3 —— **网关开关默认关；重构完成前不开发注册/网关**。
> 本文件只回答「如果今天要注册，还缺什么」，不修改 `game-engine-mcp` / `apps/agent-gateway`。

## 1. `EnginePlugin` 完备性（已具备 ✓）

| 方法 | v2 实现 | 证据 |
|---|---|---|
| `game_id` | `sanguosha2` | `tests/plugin_contract.rs::plugin_identity_and_rules` |
| `rules` | 一行规则摘要 | 同上 |
| `new_game` | 洗牌（注入 RNG）+ 发牌 + 身份（主公/反贼）+ `turn::begin_game` | `new_game_builds_playable_state` |
| `legal_actions` | pending 选项；出牌阶段的手牌动作（`use:*` / `recast:*`）+ 技能动作 + `end_play` | `legal_actions_are_accepted_by_execute` |
| `execute_action` | pending 提交 / 出牌 / 重铸 / 技能 / 结束出牌；每步 `tick` + 守恒/无帧校验 | 同上 |
| `public_state` | `view::public_state`（无任何私有信息） | `public_and_player_views_are_redacted` |
| `player_view` | `view::player_view`（只含本人手牌/身份/私有展示/pending 详情） | 同上 |
| `summarize` | 回合/阶段/当前/等待/存活 | `plugin_identity_and_rules` |
| `validate_state` | 默认实现（未覆盖） | —（注册前需补：牌区守恒 + schema 校验） |

其它不变量：
- 每个 pending 都有出口（`escape_id` 在 options 内）；
- `finished` 与 `GameState.status` 一致；
- 动作后 `check_conservation` + `check_no_orphan_frames` 全绿。

## 2. 注册步骤（**S3 已完成**，2026-11-10）

1. ✅ **crate 注册**：`crates/game-engine-mcp` 依赖改为 `sanguosha2`，注册 `Sanguosha2Engine`；
   **`game_id` 改为 `sanguosha`**（OQ1=A：v2 接管正式 id，v1 不再注册）。
2. ✅ **网关菜单**：`GAME_OPTIONS` 的 `sanguosha` 项继续由 v2 服务（id 不变）；座位读取改成
   v1/v2 适配器（`player_id`/`name`、`is_bot`/`bot`）。
3. **回滚**：OQ1 口径 = 部署回旧二进制（旧二进制里 v1 注册为 `sanguosha`）；不再做房间级 engine 开关。
4. ✅ **Web 工作台**：v2 视图输出 v1 兼容字段（S1/S3），`pending.actor` 改为 player_id；
   特殊交互区与新 pending kinds 的渲染在 S4。
5. ✅ **门禁**：`check:sgs2-*` 已在 `sgs:check:fast`；`verify:sanguosha.mjs` 已改为 v2 口径并通过。
6. ✅ **server 执行宿主**：`main.rs` 注册 v2；`game_routes` 的 AI 驱动/状态摘要/决策日志、
   `preview`（sandbox + apply_scene）、`bin/sgs-replay` 全部切 v2。

## 3. 当前阻塞项（注册前必须解决）

| # | 阻塞 | 影响 | 计划 |
|---|---|---|---|
| B1 | ~~**选将/大厅未实现**~~ → **S1 已解决**（`src/lobby.rs`：`lobby:*` + `setup:choose_general:*`，147 池 + 武将包过滤 + 机器人） | — | 已解决 |
| B2 | ~~仅 B0–B14 的 202 个技能~~ → **B0–B20 已完成**（147 将 / v2 注册表 296 技能，含授予技） | — | 已解决 |
| B3 | ~~**谮毁「代用/额外目标」**时序缺口（§9.2 未销号）~~ → **S2 已销号**（`Game.use_pre` + `FlowKind::PreUse`，`tests/pre_use.rs`） | — | 已解决 |
| B4 | ~~**身份局最小口径**~~ → **S1 已解决**（`src/identity.rs`：5–8 人配比 + 双内奸 + 奖惩 + 内奸胜负） | — | 已解决 |
| B5 | ~~**AI 仅最小策略**~~ → **S2 已解决**（`src/ai.rs`：`BotBrain` 启发式 + `drive_bots`，与 server 同签名） | — | 已解决 |
| B6 | ~~`validate_state` 未实现~~ → **S2 已解决**（守恒/无帧/待办出口/技能 code/武将校验，`plugin_contract` 用例） | — | 已解决 |

## 4. 不变量（注册时可直接复用的门禁）

- `tests/plugin_contract.rs`：接口完备性 / 视图脱敏 / legal≡execute / pending 出口 / 状态映射。
- `tests/view_contract.rs`：视图字段集冻结（新增字段必须改契约）。
- `check:sgs2-ai`：10 局自对弈（终局率 / illegal / loop / stalled / pending 覆盖）。
- `check:sgs-parity`：v1↔v2 数据/属性/技能覆盖（迁移期）。
