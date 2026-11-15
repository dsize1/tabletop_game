# ADR-0043：V3 接管正式引擎 id `sanguosha`，v2 退役为参考实现

- **日期**：2026-11-14
- **状态**：**已裁定（用户）**
- **决策**：
  1. `sanguosha3`（V3 内核）**接管正式引擎 id `sanguosha`**：`game-engine-mcp` 与 `server` 注册表只注册 V3；
  2. `sanguosha2` **不再注册、不再修补**（保留 crate 作为参考实现；仅 `道具中台` 目录与 `sgs-replay` 工具仍读它的
     生成数据/回放逻辑，属数据读取，不参与对局）；
  3. 选将/大厅**不重做协议**：沿用 v1/v2 已确定的动作 id（`lobby:join|leave|start|abort|add_bot|kick:*|set_*`、
     `setup:choose_general:<code>`），网关与 Web 零改动；
  4. 机器人推进与打分**放在 V3 引擎内**（`engine::bot`），server 只调用，不再依赖 v2 AI；
  5. 本阶段（21 批次完成前）**不挂武将技能**：武将达到「可显示/可开局」即可，对局完整性由 B0 前置项保证。

## 背景

- v2 的天花板已在 ADR-0042 记录（7 类缺陷 + 「报一个修一个」的过程问题）；
- V3 的 B0 前置项（P1–P6）已完成并验收（`docs/games/sanguosha/v3-b0-prereq.md`）：
  无技能即可完整对局（回合/交互/卡牌/装备/判定/终局）；
- 但 V3 未接线：MCP/server 仍注册 v2，Web 仍展示 v2 数据；`#开局` 无法进入 V3。

## 接线方式（关键约束）

| 层 | 做法 | 理由 |
|---|---|---|
| 视图 | V3 输出 **v1/v2 同形** `SgsViewData`（`game:"sanguosha"` + `public_board`/`player_area`/`special_zones`） | Web 三分区页/客户端零改动；隐藏信息仍只在 `player_area`（I3） |
| 动作 | `legal_actions` = 当前 pending 选项（强制选择不暴露占位出口）；`execute_action` = `resolve::submit` | **legal ≡ execute 同源**，且 Web 提交的就是选项 id |
| 大厅/选将 | 同一套动作 id；候选 = **Live 147**（232 名录去掉 85 个界化原版）+ 武将包过滤，不按技能过滤 | 与 v2/批次/中台同口径，避免「张辽/界张辽」同局；按技能过滤会导致「无将可选」 |
| 随机性 | 洗牌/发身份/发候选走注入 `RandomSource`；牌堆重洗走内核 `Game.rng`（可序列化） | 回放/对拍可复现 |
| 机器人 | `engine::bot`：打分策略（敌我按公开身份）+ `drive_bots`（server `/drive` 调用） | AI 与武将批次解耦；单人也能真机 |
| 历史 | `Game.log` 封顶 400 条，`Event.seq` 单调 | 快照每次动作整份序列化，无上限会 O(n²) |

## 后果

**正面**

- 真机路径打通：`#开局 三国杀` → `#加入`/`#加机器人`/`#开始` → Web 选将 → 出牌/响应 → 终局；
- 21 批次完成前即可「正常游戏」，技能缺失只影响武将强度，不影响规则闭环；
- v2 的 AI 打分/视图契约/端侧交互**不重写、只对接**，减少迁移面。

**代价/遗留**

- 选将候选**不按技能实现度过滤** → 玩家可能选到「技能未接入」的武将（当前预期行为，B0–B20 逐批补齐）；
- v2 crate 仍在仓库：**回放工具已切 V3 感知**（按快照形态分派，旧 v2 局仍可回放）；
  **道具中台仍读 v2 数据**（目录/已实现口径需产品确认后再迁，迁移后即可删 v2）；
- v2 专属浮层（拼点/观星/蛊惑/化身）在 V3 尚无对应 pending 形态 → 相关武将批次时补充新特殊区；
- `sgs-replay`（v2 回放对拍工具）暂时只对 v2 快照有效。

## 验证

- `crates/engines/sanguosha3/tests/engine_plugin.rs`（6）：接管 id、大厅→选将→出牌、视图契约、legal≡execute、机器人跑到终局、武将包过滤；
- `cargo test -p sanguosha3 -p game-engine-mcp -p server`；`npm run test:agent-gateway`、`npm run test:web` 全绿。
