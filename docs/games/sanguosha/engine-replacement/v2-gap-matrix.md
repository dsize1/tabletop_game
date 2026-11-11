# v2 缺口矩阵（S0，2026-11-10）

> 用途：v2 接管 `game_id=sanguosha` 前，把「引擎缺什么 / 视图缺什么 / 交互缺什么 / 哪里还绑 v1」变成可核对的清单。
> 机检：
> - `node apps/scripts/check-sgs2-view-parity.mjs`（字段契约，S0 报告 → S4 `--strict` 强制）
> - `node apps/scripts/check-sgs2-coupling.mjs`（v1 引用台账，白名单外即失败）
> 契约清单：`view-required.json`（80 字段）、`coupling-whitelist.json`。
> 裁定（用户 2026-11-10）：OQ1 A（v2 接管 id）/ OQ2 A（v2 输出 v1 兼容视图）/ OQ3 A（本轮不删 v1）/
> OQ4 A（启发式 AI）/ OQ5 A（沿用 special_zones）/ OQ6 A（中台切 v2）/ OQ7 A（回合外按座位轮询）/
> OQ8 保留双内奸 / OQ9 维持极简播报 / OQ10 A（存量房间中断）/ OQ11 素材 code 以脚本核对。

## 1. 引擎缺项

| # | 缺项 | 证据（现状） | 影响 | 处置 |
|---|---|---|---|---|
| E1 | 无大厅/选将 | `lib.rs::seats_for` 固定 `seat_template`（zhoufei/luzhi/caoren/zhoutai/guest），无 `lobby:*`、无 `Phase::Setup` | 群里 `#开局→#开始` 无法选将；Web 选将页无数据 | S1 |
| E2 | 身份局不完整 | `seats_for` 只写 `lord` / `rebel`；无忠臣/内奸、无配比、无死亡奖惩、无亮身份 | 5–8 人局规则不成立，胜负判定片面 | S1 |
| E3 | 无机器人座位 | `Seat` 无 `bot` 字段（`kernel/mod.rs`） | server `drive_bots` 无法识别机器人；`#加机器人` 不可用 | S1 |
| E4 | AI 只有「首个可行动作」 | `examples/sgs2-selfplay.rs::choose`（优先 respond/pick/use/end_play） | 机器人弱智（乱出牌/不救队友），群内体验不可用 | S2 |
| E5 | 无 `validate_state` | `EnginePlugin` 默认实现 | 工作台模板/预览无法校验 v2 快照 | S2 |
| E6 | 武器特效只做青釭剑 | `cards.rs::WeaponKind` 登记 11 种；`flow.rs` 只有 `WeaponKind::Qinggang` | 贯石/寒冰/雌雄/方天/麒麟/古锭/朱雀/诸葛连弩/青龙 全无效 | S2 |
| E7 | 谮毁「代用/额外目标」时序缺口 | 对齐单 B5 未销号（`plan` §9.2）；B8 已有 `sha_intercept` 窗口 | 该武将行为不完整 | S2 |
| E8 | 回合外主动技只问当前回合角色 | `lib.rs::legal_actions` 非当前回合只给 `legal_actions_out_of_turn`，但无人轮询 | 矫诏等「他人回合可发动」技能实际用不出 | S2 |
| E9 | `special_zones` 恒为 `[]` | `view.rs::root` 硬编码空数组 | 观星/五谷/蛊惑/化身的「特殊交互区」在 Web 上只能靠 pending 兜底 | S4（OQ5=A） |
| E10 | 无中断（abort） | 无 `abort` 字段/动作（v1 有 `abort_reason`） | 房主 `#中断` 不可用 | S1 |
| E11 | 武将包信息缺失 | `generals_generated::GeneralDef` 无 pack 字段（`engine-catalog.json` 有） | `#武将包` 过滤无数据源 | S1（生成器补 pack） |

## 2. 视图字段差异（v1 兼容目标 = `view-required.json` 的 80 项）

机检结果：v2 当前缺 **46/80**（`check-sgs2-view-parity.mjs` 首跑）。按组归类：

| 组 | 缺失字段 | 处置 |
|---|---|---|
| 根 | `game`（v2 是 `game_id`） | S4 增加 `game: "sanguosha"`（Web 分派键） |
| board 元信息 | `phase` `round` `turn_index` `turn{seat,phase,player_id}` `seat_count` `config` | S4 由 `g.turn/g.phase/g.round` + `Seat` 派生 |
| board 牌堆 | `deck{draw,discard,processing,void}` `discard_top` | S4 由 `ids_in(Zone)` 派生（现有 `draw_count`/`discard` 改名） |
| board 选将/终局 | `setup{lord,lord_picked,picked_count}` `aborted` `history` | S1（setup）/ S1（abort）/ S4（history 由 `g.log` 投影） |
| seat | `identity`（仅主公/已死亡） `general_name` `is_bot` | S1 身份 + S1 `bot` + S4 投影；`general_name` 由 `generals_generated` 查名 |
| player_area 基础 | `general` `general_gid` `kingdom` `general_name` `hp` `max_hp` `equips` `judge_zone` `marks` `marks_count` `transforms` `abolished_slots` `judge_abolished` `flipped` `chained` `skills` | S4（这些在 v2 已存在于 `public_seat`，只是 `player_area` 未投影） |
| player_area 交互 | `equipment_skills` `is_bot` `is_current` `setup{candidates,candidate_details,picked,is_lord}` | S1（选将）/ S4（其余） |

> 现有 v2 已有但 v1 没有的字段（`name_is_virtual` `damage_kind` `suit_label` `marks`/`revealed` 等）保留，Web 侧按可选字段消费。

## 3. pending kinds 差异（Web 交互缺口）

| v2 kind | 出现场景（示例） | Web `pending.ts` 现状 | 处置 |
|---|---|---|---|
| `choice` | 通用二选一（观星逐张顶/底、梁兴、贞良…）297 处 | 按钮渲染（无浮层） | S4：`skill=guanxing` 走观星浮层 |
| `target` | 选目标（119 处） | ✅ 浮层 | 复用 |
| `choose_many` | 多选手牌（16 处） | ✅ 浮层 | 复用 |
| `card` / `card_pick` / `card_target` / `card_response` | 牌堆/弃牌堆选牌、指定承载牌、响应牌 | ❌ 无 | S4 |
| `harvest` | 五谷丰登逐人取牌 | ❌ 无（v1 是 `wugu` special_zone） | S4（OQ5：zone + pending 混合） |
| `pindian` | 拼点（暗选） | ❌ 无 | S4 |
| `fire_attack` | 火攻亮牌/弃牌 | ❌ 无 | S4 |
| `discard_phase` | 弃牌阶段 | 仅 `kind === "discard"` 判断 | S4 兼容两种 |
| `dying` / `wuxie` | 濒死 / 无懈链 | ✅ | 复用（字段名核对：v2 `data.root`/`layers` 口径） |
| `collateral` / `qinwang_sha` | 借刀 / 勤王代出 | ❌ 无 | S4 |
| `choose` | 少量技能 | 按钮兜底 | S4 |
| v1 独有 `arrange` `judge_rewrite` `respond` `zhengu` `pick_card` | — | 现有渲染 | v2 无对应 kind，删除映射（保留无害） |

## 4. v1 耦合点（`check-sgs2-coupling.mjs` 首跑：8 文件 / 46 处）

| 文件 | 引用 | 处置切片 |
|---|---|---|
| `crates/server/src/main.rs` | ✅ S3 已切 v2 |
| `crates/server/src/game_routes.rs` | ✅ S3 已切 v2 |
| `crates/server/src/preview.rs` | ✅ S3 已切 v2（新增 `sanguosha2::sandbox`：沙盒开局 + 场景注入） |
| `crates/server/src/bin/sgs-replay.rs` | ✅ S3 已切 v2 |
| `crates/server/src/gameprops_routes.rs` | `catalog::{...}` / `catalog::general_status` | S5 |
| `crates/game-engine-mcp/src/main.rs` | ✅ S3 已切 v2（`verify:sanguosha` 通过） |
| `apps/agent-gateway/src/index.ts` | ✅ S3 改为 v1/v2 座位适配器（`seatPid`/`seatIsBot`） |
| `apps/agent-gateway/src/sanguosha-chat.ts` | ✅ S3 适配器（v1/v2 两种形状） |

> 说明：`apps/scripts/*` 里的 v1 数据源（`sgs-data-lib.mjs` / `check-sgs-skill-cases.mjs` / `p67` 等）属 M7（S6），
> 不在本门禁扫描范围（它们读写 v1 源码文本，不影响运行时）。

## 5. S1/S2 关闭情况（2026-11-10）

| 项 | 状态 |
|---|---|
| E1 大厅/选将、E2 身份局、E3 机器人座位、E10 中断、E11 武将包 | ✅ S1（`src/lobby.rs`、`src/identity.rs`、`Seat.bot`、生成物补 `gid`/`pack`） |
| E4 AI（启发式 + `drive_bots`）、E5 `validate_state` | ✅ S2（`src/ai.rs`、`plugin_contract` 用例） |
| E6 武器特效 9 种 | ✅ S2（`kernel/gear.rs` + `sha_loop` 数值链；`tests/weapons.rs` 10 条） |
| E7 谮毁「代用/额外目标」 | ✅ S2（`Game.use_pre` + `FlowKind::PreUse`；`tests/pre_use.rs` 4 条） |
| E8 回合外主动技 | ✅ S2（`drive_bots` 顺路询问机器人回合外窗口；人类窗口由 `legal_actions` 提供） |
| E9 `special_zones` | ⏳ S4 |
| §2 视图 46 字段 | ✅ S3（v1 兼容字段全部输出；`equipment_skills` 仍空数组）+ S4 只做前端渲染/特殊区 |
| §3 pending 新 kinds | ⏳ S4 |

## 6. 切片 ↔ 清单映射

| 切片 | 关闭清单项 |
|---|---|
| S1 | E1 E2 E3 E10 E11 + 视图 setup/identity/is_bot + pending（选将 `choice` 复用） |
| S2 | E4 E5 E6 E7 E8 |
| S3 | §4 全部（除 `gameprops_routes`） |
| S4 | §2 全 46 字段 + §3 新 kinds + E9 + W2/W3/W4 |
| S5 | `gameprops_routes` + 素材/状态 |
| S6 | M7 门禁事实源 + 停注册 |
