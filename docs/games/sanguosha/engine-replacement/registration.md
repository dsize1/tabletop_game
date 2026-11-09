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

## 2. 注册步骤（后移，不在本轮）

1. **crate 注册**：`crates/game-engine-mcp` 的依赖表 + `main.rs` 注册 `Sanguosha2Engine`（game_id `sanguosha2`）。
   - 注意：v1 仍注册为 `sanguosha`；两套并存期间 game_id 必须不同（`sanguosha` / `sanguosha2`）。
2. **网关菜单**：`apps/agent-gateway/src/games.ts::GAME_OPTIONS` 追加一项（`#开局` 菜单自动出现）。
   - 默认**不**暴露给群：受 `SGS_ENGINE`/房间白名单开关控制，默认 v1（OQ3）。
3. **灰度**：按房间 `engine` 字段选择；白名单群 + 只读观战先跑；回滚 = 改房间 engine 回 `sanguosha`。
4. **Web 工作台**：v2 `view` schema（`schema_version=1`）适配：
   - `public_board.seats[].equips/judge_zone/marks`、`player_area.hand/pending`；
   - `pending.options` 渲染（沿用交互原语）；`presentation` 文案；
   - 先只读/沙盒，再开操作。
5. **门禁**：`check:sgs-view-contract`、`check:sgs2-ai` 已在 `sgs:check:fast`；注册后把
   `check:sgs-ai` / `check:sgs-view-contract` 的主口径切到 v2（v1 保留到退役）。

## 3. 当前阻塞项（注册前必须解决）

| # | 阻塞 | 影响 | 计划 |
|---|---|---|---|
| B1 | **选将/大厅未实现**：`new_game` 用固定 `seat_template`（周妃/卢植/曹仁/周泰/guest） | QQ/Web 玩家无法选将，武将只有 5 个模板 | 注册前补「大厅选将 + 武将池过滤」（v2 版 `lobby:*`） |
| B2 | **仅 B0–B9 的 128 个技能**（147 将中 67 将）；B9（2026-11-09）完成界标·蜀 14 技能 | 注册后只能开放对应武将，否则技能缺失 | 继续按 B10–B20 批次迁移 |
| B3 | **谮毁「代用/额外目标」**时序缺口（§9.2 未销号） | 该武将行为不完整 | 结算前钩子帧（M4 之后） |
| B4 | **身份局最小口径**（主公 vs 反贼，无忠臣/内奸） | 8 人局/身份胜利条件不完整 | 注册前补完整身份分配与胜负 |
| B5 | **AI 仅最小策略**（check:sgs2-ai 用） | 群内机器人强度不足 | 沿用 v1 `ai/` 的分层思路迁移（M4 之后） |
| B6 | `validate_state` 未实现 | 工作台模板/预览无法校验 v2 快照 | 注册前补 |

## 4. 不变量（注册时可直接复用的门禁）

- `tests/plugin_contract.rs`：接口完备性 / 视图脱敏 / legal≡execute / pending 出口 / 状态映射。
- `tests/view_contract.rs`：视图字段集冻结（新增字段必须改契约）。
- `check:sgs2-ai`：10 局自对弈（终局率 / illegal / loop / stalled / pending 覆盖）。
- `check:sgs-parity`：v1↔v2 数据/属性/技能覆盖（迁移期）。
