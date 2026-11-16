# v3-parity-baseline.md — v1 行为 oracle 对拍基线（M0）

> 状态：M0 进行中（2026-11-13 开工）。v1 已复活为 test-only oracle，本文记录对拍口径与白名单。
> 上游：ADR-0042 M0、`memory_palace/plan/next-v3-rebuild.md` §3-M0、
> `docs/games/sanguosha/engine-replacement/archive/parity-whitelist.json`（v1 已知缺陷白名单归档）。

## 0. 结论先说

- **v1 已复活**（git `f8a7db3^`），编译通过、**987 tests 全绿**，作为 V3 的**行为 oracle**（不注册网关、不上线）。
- **对拍口径**：同 seed + 同动作脚本回放，对比 **legal 动作集 / pending 序列 / 状态摘要（hp·手牌数·区域）/ 日志事件**；
  白名单外 0 差异。
- **v1 已知缺陷白名单 4 条**（按牌名枚举【杀】，影响火杀/雷杀判定），对拍时排除，避免「把缺陷当回归」。

## 1. v1 复活状态

| 项 | 值 |
|---|---|
| 复活来源 | git `f8a7db3^`（删除 v1 的前一个提交） |
| 位置 | `crates/engines/sanguosha`（已回 workspace members） |
| 规模 | 135 个 `.rs` 文件（`src/` 含 ai/resolve/resolve_*×20/skills/state/turn/view…） |
| 依赖 | `game-engine-core` + `serde`/`serde_json`（workspace 内，无新增） |
| EnginePlugin 对齐 | 与当前 `game-engine-core` trait 方法完全一致（game_id/rules/new_game/legal_actions/execute_action/public_state/player_view/summarize/validate_state/apply_scene） |
| 编译 | `cargo build -p sanguosha` 通过（2 warning：unreachable pattern / dead_code，无错误） |
| 测试 | `cargo test -p sanguosha` **987 passed / 0 failed** |
| 运行态 | 未注册进 `game-engine-mcp` / 网关（纯 test-only oracle） |

## 2. v1 已知缺陷白名单（对拍时排除）

> 来源：`engine-replacement/archive/parity-whitelist.json#v1_known_defects`（spike-report §12.2）。

| id | 位置 | 缺陷 | 影响 |
|---|---|---|---|
| `v1-chiyong-name-match` | `resolve.rs:6175` | 恃勇按物理牌名枚举【杀】（`sha\|huo_sha\|lei_sha`） | 语义判定，可能漏转化杀 |
| `v1-jiuchi-name-match` | `resolve.rs:6228` | 界酒池按物理牌名枚举【杀】 | 同上 |
| `v1-qiaomeng-name-match` | `resolve.rs` | 趫猛/利驭按物理牌名枚举【杀】 | 同上 |
| `v1-ai-is-sha-name-match` | `ai/card.rs:10` | AI 判断是否【杀】按牌名 | 只影响取舍，不参与规则 |

**注意**：v1 同类「按牌名枚举杀」仍有多处，白名单只登记已知/影响语义的 4 条；对拍时若发现新差异，先归入白名单再报警（防把缺陷当回归）。

## 3. v1 ↔ v2 已知差异（P0 / P1）

> 来源：`parity-whitelist.json#target_mode` + `v2_extra_defs`（M4 已对拍确认）。

### 3.1 P0（语义差异，需 V3 明确采用哪一侧口径）

| 牌 | v1 | v2 | 说明 |
|---|---|---|---|
| 杀 / 火杀 / 雷杀 | target_mode=None（按单目标） | Chosen（显式单目标） | 影响 legal 动作形状，语义一致 |
| 铁索连环 | None | Chosen（1–2 目标） | 语义差异：v1 未实现多目标 |
| 乐不思蜀 / 兵粮寸断 | None | Chosen（选目标） | 延时锦囊目标语义 |

### 3.2 P1（形状差异，不影响语义，白名单内）

| 牌 | v1 → v2 | 说明 |
|---|---|---|
| 无中生有 / 闪 / 桃 / 酒 / 闪电 / 无懈可击 | None/Single → SelfOnly | 只对自己/响应，主动使用被拒绝 |
| 木牛流马 | 无 → v2 额外生成 | 测试夹具用，不在 160 牌堆 |

## 4. 对拍机制设计（V3 批次开发的常驻工具）

### 4.1 对比维度（normalized，屏蔽两引擎 schema 差异）

| 维度 | v1 侧 | v2/v3 侧 | 归一化 |
|---|---|---|---|
| 合法动作集 | `legal_actions(state, pid)` | 同 | 排序后比对 id 集合（白名单外） |
| pending 序列 | `pending.kind` + actor | 同 | 比对「当前 pending kind + actor」序列 |
| 体力 | 各座位 hp | 同 | 座位序对齐 |
| 手牌数 | 各座位 hand.len() | 同 | 座位序对齐 |
| 区域 | 牌区归属（draw/discard/void/hand/equip/judge/mark） | 9 区 | 按牌 id 映射到归一化区域枚举 |
| 日志事件 | `game_events` | 同 | 按「事件类型 + 关键字段」归一化后比对 |

### 4.2 回放一致性前置（v1 已验证）

- v1 自带 `tests::replay::replay_reproduces_state_and_event_sequence` 与 `replay_is_stable_across_repeated_runs`（987 测试内，全绿）。
- 随机一律走 `RandomSource`/`rand_below`，同 seed 确定性 → 对拍可复现。

### 4.3 尚未落地（M0 后续 / V3 批次期）

| 项 | 说明 |
|---|---|
| replay 对拍脚本 | v1 与 v3 各跑同 seed 同动作脚本，输出归一化摘要 diff（当前只有口径，脚本待写） |
| v1 oracle 进 CI | 每次提交跑核心对拍（防漂移，§11.3） |
| 白名单有效性自检 | 检测白名单条目仍存在（防白名单腐烂，M4 已做一次） |

## 5. V3 使用方式

1. 每个迁移批次（B0→B20）实现后，同 seed 同动作脚本在 v1 与 v3 各跑一遍；
2. 对比 §4.1 的归一化维度，白名单外 0 差异才算收口；
3. 发现新差异 → 先判断「v1 缺陷 or v3 偏差」→ 归入白名单（若是 v1 缺陷）或修 v3（若是 v3 偏差）；
4. 白名单随 V3 推进逐步收紧（v1 已知缺陷在 V3 里应该已被修复，白名单条目最终归档）。

## 6. B1 迁移对拍结果（2026-11-13）

### 6.1 口径更正：主公**初始手牌 = 4**（旧裁定「6」作废）

- **证据**：
  - v1 `finish_setup`：`Config::initial_hand = DEFAULT_INITIAL_HAND = 4`，主公只 **+1 体力上限**（`lib.rs:356` 只改 `max_hp`）；
  - v2 `lobby.rs`：注释与实现同为「初始手牌 4 张」；
  - 旧对拍把「v1 **出牌阶段**手牌 6」误读为「初始手牌 6」——该 6 = **4 初始 + 摸牌阶段 2**（阶段口径错位）。
- **处置**：`turn::begin_game` 回正为「每人 `hand_cards` 张」，`parity_lord_first_turn_hand_is_six` 改为**同阶段**比对
  （v1 出牌阶段 ↔ v3 开局 + 摸牌阶段）；`tests/lifecycle.rs` 同步。
- **影响**：`memory_palace/plan/next-v3-collection.md` §9.2「主公初始手牌 +2」裁定作废（该文已加更正行）。
- **用户确认**：2026-11-13 用户确认「主公初始手牌 = 4」为最终口径（`rules.md` 已补「含主公」）。

### 6.2 v1 oracle 可比项（B1）

| 技能 | v1 事实（oracle） | v3 断言 | 结果 |
|---|---|---|---|
| 庸肆 | 主公袁术首回合手牌 10 = 4 + 2 + X(4，群/魏/吴/蜀) | `flow::draw_phase` = 2 + `kingdom_count()`，手牌 10 | ✅ 0 差异 |
| 怀橘 | 主公陆绩开局「橘」= 3；首回合手牌 7 = 4 + 2 + 1 | `fire_game_start` 3 橘；`draw_phase` = 2 + 1 | ✅ 0 差异 |
| 寸目 | 许攸首回合手牌 6（方向不改数量） | `flow::draw` 从牌堆底摸，手牌 6 | ✅ 0 差异 |

其余 15 技能（奇制/进趋/拒战/飞军/兵略/遗礼/整论/溃诛/掣政/立军/荐降/审时/成略/恃才/伪帝）：
v1 侧需要「同 seed 同动作脚本」跑到对应时点，依赖 M6 回合驱动与动作消费闭环（§4.3 未落地项），
当前以 **B1.md 权威描述逐条断言**（`tests/b1.rs`，35 用例，含每条「正例 + 反例」）作为批次门禁；
M6 落地后把上表扩展为全 18 技能脚本对拍。

## 7. M5.1 交互闭环 + 通用濒死/死亡 + 裁定落地（2026-11-13）

> 用户裁定（全 A）：先做 M5.1 再开 B2；通用 `FlowKind::Dying`；`DeferredKind::TurnEndCheck`；
> 对拍口径 = 对齐单断言 + 可比标量 oracle；触发类可选技能本期自动结算；接最小武将名录。

### 7.1 已落地（本次）

| 项 | 实现 | 门禁 |
|---|---|---|
| 提交唯一入口 | `resolve::submit`（actor / 合法选项 / 强制选择出口拒绝；失败不改状态） | `tests/m5.rs::submit_*` |
| 答案消费 | `resolve::take_answer`（只有发起待办的技能能取答案） | B1 主动技全链路用例 |
| 主动入口 | `skills::active_actions`（窗口 + `can_active` + 可用性）+ `skills::use_active` | `tests/m5.rs::active_actions_*` |
| 通用濒死 | `flow::enter_dying` → `FlowKind::Dying`（自己 → 下家；hp>0 才询问；每座位至多一个濒死帧） | `dying_rescue_by_tao` |
| 通用死亡 | `turn::settle_death`：弃牌 → 奖惩（杀反贼摸 3 / 主公杀忠臣弃光）→ 胜负 | `dying_without_rescue_*`、`lord_killing_loyalist_*` |
| 回合结束校验 | `DeferredKind::TurnEndCheck`（ORDER 第 4 位；未到结束阶段不派发；跨回合清理） | `turn_end_check_is_kept_*`、审时阴面用例 |
| 延迟项顺序 | `ORDER = Land → SkillTrigger → JudgePhase → TurnEndCheck → DeathCleanup`；已同步 `align/mechanics.json` | `tests/resolve.rs::order_table_is_explicit_and_documented` |
| 武将名录 | `generals_generated.rs`（232 将最小字段）+ `kernel::generals::{def,kingdom,apply_to_seat}` | `tests/generals.rs`（4 用例） |
| B1 主动技 | 进趋/飞军/遗礼/整论/审时（阳）/成略 改为 pending 多阶段；遗礼濒死补发走延迟队列 | `tests/b1.rs`（35 用例） |

### 7.2 本期明确未做（M5.2 / M6）

- 濒死仅支持【桃】（【酒】自救、技能救援、完杀/不屈等钩子未接）；
- 触发类可选技能仍自动结算（奇制/拒战/荐降/审时阴面/恃才/立军/溃诛），pass 出口待 M5.2；
- 回合/阶段驱动（M4.4）：尚无 `phase.*`/`turn.*` 计数器清理与摸牌阶段接线；
- v1 全流程动作脚本对拍（依赖上述驱动与动作消费）。

### 7.3 B1 收口清单（**历史快照：2026-11-13；已于 2026-11-15 收口，见 §7.4**）

> `check:sgs3` 全绿 ≠ 收口。B1 的「实现」已完成，但**收口（可玩 + 可验收 + 口径闭环）** 还差下列项。
> 2026-11-13 的裁定 4A/5A 只授权「批次内可延后」，不等于收口完成；**B2 必须在 B1 收口后开工**。

| # | 收口项 | 现状 | 阻塞原因 |
|---|---|---|---|
| C1 | **阶段驱动**（PhaseStart/PhaseEnd/TurnStart/TurnEnd 派发 + `phase.*`/`turn.*` 清理） | ❌ 内核无任何阶段事件发射点 | 庸肆(弃牌阶段始)/进趋(结束)/遗礼(出牌始)/整论(摸牌)/溃诛(弃牌终)/掣政(出牌终)/怀橘(摸牌+1)/寸目 只能单测，真实回合**永不触发** |
| C2 | **触发类可选技能的 pass 出口**（询问完整性 I1） | ❌ 自动结算 | 奇制/拒战/荐降/审时阴面/恃才/立军/溃诛 无法放弃，玩家被迫结算 |
| C3 | **濒死扩展 M5.2**（【酒】自救、技能救援、完杀/不屈钩子、桃以外的救援） | ❌ 仅【桃】 | 遗礼「失体力致濒死」在真实对局里只有桃能救，口径不完整 |
| C4 | **v1 全流程脚本对拍**（同 seed 同动作脚本，18 技能） | ❌ 仅 3 项可比标量 | 依赖 C1；B1.md 的 15 个技能仍是「对齐单断言」 |
| C5 | **可运行引擎接入**（`EnginePlugin` + `game-engine-mcp`/网关，至少 test-only harness） | ❌ v3 是纯内核 | 无法真机跑、无法抽检 |
| C6 | **人工抽检**（B1.md 本批抽检表 2–3 个高风险技能） | ❌ 待抽检 | 依赖 C5 |

**收口顺序**：C1 → C2/C3 → C5 → C4 → C6（C2/C3 可与 C5 并行）。

> **B0 与本清单同缺口**：B0 的 7 技能也只是「声明 + handler + 单测」层级，同样未收口：
> 解围在 v3 是 no-op、颂威固定「座位 0 摸 1」（无魏势力/黑色判定判定）、放逐用伤害来源当目标（无玩家选择）、
> 不屈简化为摸一张当「创」；据守/放逐/颂威/奋激/行殇 依赖的结束阶段与判定事件**没有生产者**。
> 因此 M4.2 B0 的「✅」应读作「实现层完成」，收口随 C1–C6 一并结算。

### 7.4 B1 收口记录（2026-11-15，✅）

> 用户裁定：**内核完备后重验 B1**（不得按「批次需要时才补机制」推进）。B1 的 18 技能原先只有
> 「把 `ask.data` 手推给 handler」的单元用例，真实事件路由下 **8 个 handler 会代玩家选牌或静默不触发**
> （`pick_from` 兜底 `cands.first()` / `seat_of(ask,"target")` 在事件 payload 中不存在）。

| # | 收口项 | 结果 | 证据 |
|---|---|---|---|
| C1 | 阶段驱动 | ✅ | `turn::step_turn` 真实阶段机；阶段开始类技能**先询问再执行自动动作**（整论跳过摸牌、遗礼先结算）；`skills::has_phase_start_sub` 保证无技能时单步推进 |
| C2 | 触发类可选技能 pass 出口 | ✅ | 8 个 handler 重写为「`trigger_ask`（发动/放弃）→ 强制选牌 `must_pick`（`escape_mandatory`）→ `Next/NeedWith` 中间态」：奇制/拒战/掣政②/荐降/立军/恃才/溃诛/审时阴面；**禁止** `cands.first()` 代选 |
| C3 | 濒死扩展 | ✅ | `FlowKind::Dying` + `dying-aftermath`（兴衰求桃顺序 / 支援者脱困后受伤）+ 酒自救（P4/B0） |
| C4 | v1 全流程脚本对拍 | 记入验收待办 | 用户 2026-11-15 确认：本阶段只保留「可比标量 0 差异 + 对齐单断言」；脚本桥随 §11.2 风险清单 |
| C5 | 引擎接入 | ✅ | ADR-0043：V3 接管 `game_id=sanguosha`（`engine/{lobby,view,text,bot}` + `EnginePlugin`） |
| C6 | 人工抽检 | 记入验收待办 | 走查单 R1–R7（真机）由用户执行；代码批次不阻塞 |

**用例**：
- `tests/b1.rs` 35 条（handler 分支，按新的分阶段契约驱动）；
- `tests/b1_real.rs` 7 条（真实回合：整论/遗礼/庸肆/怀橘/进趋/寸目/挂载）；
- `tests/b1_real_more.rs` 9 条（真实路由：奇制/拒战阴阳/掣政②/荐降/立军/恃才/溃诛/审时阴面）。

**挂载**：`skills::ENABLED_SKILLS` 追加 B1 全部 18 个 code；选将收尾按「已注册 + 已收口」挂载
（`b0_real.rs::skill_mounting_only_enables_aligned_skills` 覆盖曹仁/袁术/未接入批次）。

**遗留**（不阻塞 B2）：
- 审时阴面的「观看其手牌」通过**私有 pending 选项**呈现（视图仍不泄露他人手牌明细）；
- 拒战/立军的「本回合不能再使用牌」用 `deny.*` 标记（随回合清理），与手牌冻结是不同事实；
- 立军的「交给」在事件路由后进行（【杀】可能已在弃牌堆，从当前区取回），真机抽检确认文案与时机。

**下一步**：B0 回归（内核新增机制后 7 技能重跑）→ B2（破势按原文补齐：条件「全栏废除 **或** 体力=1」/
摸至体力上限/失去决堰/获得怀柔）。

### 7.5 B2 收口记录（2026-11-15，✅）

> 批次：风林火山·SP-2（8 将 / 15 技能 + 授予技 清侧/怀柔 + 临时技 集智）。
> 用户裁定：**破势按原文补齐**（对齐单第 7 段是摘要）。

| 项 | 结果 |
|---|---|
| 技能 | `kernel/skills/b2.rs`：明任/贞良/镇骨/往烈/罪论/父荫/良姻/箜声/征荣/鸿举/谦节/决堰/破势/雄乱/从谏 + `huairou`/`qingce`/`jizhi` |
| 废除语义 | **只共享状态原语**（`Seat.abolished_slots` / `judge_abolished`）：决堰（限一次/单槽/本回合效果 + 永久废除该槽）、破势（觉醒全槽 + 栏内牌入弃牌堆）、雄乱（装备区 + 判定区） |
| 破势 | 按原文：准备阶段（装备栏均被废除 **或** 体力=1）→ 减 1 上限 → 废除全栏 → 摸至体力上限 → 失去决堰 → 授予怀柔 |
| 内核新增 | `Hook::UsePre` + `skills::fire_use_pre`（往烈：首张无距离 / 不可响应 + 手牌冻结）；`Game.use_targets`（从谏读目标列表）；`skills::revoke`（觉醒失去技能）；`ignore_distance` / `no_response_for` / `deny_target_card` / `deny_pindian` / `card_ineffective_for` 单出口 |
| 用例 | `tests/b2.rs` 20 条（每技能 ≥1 正例 + ≥1 反例） |
| 挂载 | `ENABLED_SKILLS` 追加 18 个 code（含授予/临时技） |

**已知边界（已登记，不阻塞 B3）**：
- 征荣 / 往烈的「先取荣 / 不能被响应」在 v3 事件路由中发生于**该牌结算之后**（`UsePre` 已覆盖往烈；征荣取荣晚于响应窗口）；
- 贞良（阴）在「使用/打出」路由后判定颜色（牌将在本次结算后落弃牌堆）；
- 箜声③「令一名角色使用剩余装备」简化为：移入其手牌后逐张装备，再失去 1 点体力；
- 决堰·宝物的「集智」用 `turn.<seat>.jizhi` 计数实现（不授予长期 SkillDef），随回合清理；
- 镇骨的「多则弃」由目标逐张选择（符合 I1），目标回合结束的第二次结算走 `DeferredKind::TurnEndCheck`；
- 谦节的拼点拦截在 `pindian::can_start` 单入口生效。

### 7.6 B3 收口记录（2026-11-15，✅）

> 批次：一将成名·批次1/2/Y1（5 将 / 9 技能 + 授予技 排异）。

| 项 | 结果 |
|---|---|
| 技能 | `kernel/skills/b3.rs`：散谣/制蛮/奇策/智愚/权计/自立/峻刑/御策/龙吟 + `paiyi`（授予） |
| 内核新增 | `Hook::DamageBefore` + `skills::ask_damage_before` + `flow::damage_apply`（制蛮：伤害结算前询问；拒绝后走 `damage_apply` 防递归）；`hand_limit_bonus` 合并权计（每「权」+1） |
| 关键口径 | 权计按**每点伤害**各询问一次（`points` 循环）；奇策用 `use_as` 承载全部手牌；智愚/御策展示走 `reveal`/`conceal`（当下公共可见 + 事件）；峻刑按弃牌**类型集合**判定「皆不同」；龙吟只询问从使用者左手侧起的第一个拥有者，`declare_sha_not_counted` 回退次数；排异移去「权」进弃牌堆 |
| 用例 | `tests/b3.rs` 10 条（每技能 ≥1 正例 + ≥1 反例） |
| 挂载 | `ENABLED_SKILLS` + B3 10 个 code（注册表 43 → 53） |

**已知边界（已登记，不阻塞 B4）**：
- 制蛮的「先取荣/先防止」在 v3 中通过 `DamageBefore` 帧完成，伤害事件在取牌后不再产生（符合「防止伤害 = 未造成伤害」）；
- 奇策以 `use_as` 成功为准设置 `phase.qice`（若之后取消使用，`cards_used` 会回退，但本阶段次数不回退——登记）；
- 御策「来源无手牌且你满体力」时不提供发动（handler 直接 Done，符合 B3 裁定）；
- 散谣的两个模式各自每阶段一次（`phase.sanyao_hp` / `phase.sanyao_hand`）。

### 7.7 B4 收口记录（2026-11-15，✅）

> 批次：一将成名·Y2+Y3（8 将 / 11 技能）。

| 项 | 结果 |
|---|---|
| 技能 | `kernel/skills/b4.rs`：陷嗣/夺刀/暗箭/胆守/司敌/慎断/勇略/品第/法恩/奔袭/忠勇 |
| 内核新增 | `EventKind::Flipped`（翻面事件，法恩订阅）；`flow::chain_enter`（品第/法恩）；`flow::damage_apply` 叠加 `skills::damage_bonus`（暗箭）；`UseOpts.ignore_armor/draw_on_damage`（奔袭）；`card_play::use_from_pile_as`（慎断：弃牌堆承载当兵粮）；他人侧技能入口 `skills::other_side_actions` / `start_other`（陷嗣） |
| 用例 | `tests/b4.rs` 11 条 |
| 注册/挂载 | 注册表 53 → 64；`ENABLED_SKILLS` +11 |

**已知边界（登记）**：
- 奔袭的四个选项通过 `UseOpts` 声明，`extra_targets`/`no_wuxie` 在本次使用内消费；「无视防具/伤害后摸牌」在【杀】结算路径生效；
- 司敌的出牌阶段开始/结束共用 `sidi.<owner>.<target>` 标记区分（冻结后置位）；
- 勇略「此杀未造成伤害则摸一」以当前日志判定（若【杀】尚未结算则可能提前判定，登记）；
- 忠勇的「目标响应的【闪】」未单独建模，只移交【杀】本身；
- 陷嗣的他人侧入口只在出牌阶段候选里出现（`build_play_pending`）。

### 7.8 B5 收口记录（2026-11-15，✅ · 验证批次收官）

> 批次：一将成名·Y4（6 将 / 13 技能）。B0–B5 全部收口。

| 项 | 结果 |
|---|---|
| 技能 | `kernel/skills/b5.rs`：谮毁/骄矜/奋励/平寇/慎行/秉壹/恢拓/明鉴/兴衰/千驹/倾袭/活墨/佐定 |
| 内核新增 | 受害者侧 `ask_damage_before_victim`（骄矜）；`flow::fire_dying_enter` + `Hook::DyingEnter`（兴衰）；`skills::distance_bonus`（千驹，接入 `gear::distance`）；明鉴下回合手牌上限/杀次数（b5 数值出口） |
| 用例 | `tests/b5.rs` 13 条 |
| 注册/挂载 | 注册表 64 → **77**；`ENABLED_SKILLS` +13 |

**已知边界（登记）**：
- 谮毁的「交牌并代替使用者」只实现「交牌」，使用者的变更未建模（登记）；
- 骄矜/倾袭走 `DamageBefore` 帧，伤害在 handler 内 `damage_apply`；
- 奋励的「严格最大」按存活其他角色逐项比较；平寇按 `turn.<seat>.pingkou` 计数逐个询问；
- 兴衰：支援者代价通过 `AfterDying` 在濒死收口后派发（用户 2026-11-15 裁定：即使主公死亡仍受伤）；
- 火墨的「本回合未用过的基本牌名」用 `turn.<seat>.huomo.<def>` 计数；活墨的牌名集合限 杀/桃/酒；
- 平寇/佐定等按回合日志判定（与事件快照一致）。

**验证批次状态**：B0–B5 全部收口（77 个注册技能）；后续批次（B6–B20）按同一 DoD 推进。

### 7.9 B6 收口记录（2026-11-16，✅）

> 批次：一将成名·Y5（7 将 / 12 技能）。

| 项 | 结果 |
|---|---|
| 技能 | `kernel/skills/b6.rs`：战绝/勤王/怃戎/矢志/宴诛/兴学/诏缚/安国/邀名/怀异/急攻/饰非 |
| 内核新增 | `kernel/relay.rs` + `FlowKind::Relay`（激将/勤王代出接力，结果走 `relay.result.<owner>` 标记）；`card_play::use_relay`（代出牌视为发起者使用、伤害来源=发起者）；`card_play::equip_from_pile`（安国：牌堆/弃牌堆装备直接置入装备区）；`skills::use_alias` / `skills::response_as` + `CardAs`（矢志的**替代**语义，出牌阶段 `alias:<id>` 入口 + 响应窗口统一过滤）；`skills::shan_alternatives`（饰非替代响应，`response.sub.<seat>` 成功标记）；`gear::within_attack_range`（诏缚统一「能否打到」入口）；`flow::damage_one` 伤害记账（`turn/phase/total.damage_dealt`，急攻赋值手牌上限的唯一来源） |
| 共同件 | `kernel/skills/common.rs`（b6+ 共享选牌/文案工具，b0–b5 不动） |
| 用例 | `tests/b6.rs` 20 条；`tests/skills.rs` 注册表 77 → **89** |
| 挂载 | `ENABLED_SKILLS` +12 |

**已知边界（登记）**：
- 勤王/激将的代出接力只接受**实际【杀】**（`skills::response_as`）；转化技（武圣/龙胆）在 B9/B10 接入后自动生效；
- 勤王取消使用目标时，代出牌留在代出者手牌（未消耗），与「激将未成立」一致；
- 怃戎的双方选牌期间不公开（帧数据保存己方牌），结算时 `Game::reveal` 一起亮牌、随后 `conceal`；
- 宴诛 opt1 的「修改兴学」用 `xingxue.modified.<seat>` 计数表达，兴学结算次数=1；
- 诏缚用 `zhaofu.<chosen>` 计数持久到本局结束（主公技效果不依赖技能持有者存活）；
- 安国「随机使用装备牌」按牌堆顶第一件装备（无则弃牌堆首张）确定性取牌；
- 急攻手牌上限为**赋值型**，覆盖其他 +N 修正（`skills::hand_limit_override` 先问 b6）；
- 饰非覆盖【杀】与【万箭齐发】的【闪】询问；【借刀杀人】的闪询问暂未接入替代入口（登记）。

### 7.10 B7 收口记录（2026-11-16，✅）

> 批次：一将成名·Y6（7 将 / 12 技能）。

| 项 | 结果 |
|---|---|
| 技能 | `kernel/skills/b7.rs`：督粮/腹鳞/匡弼/止戈/宗祚/极奢/链祸/瑰藻/讥谀/寝情/贿生/滔乱 |
| 内核新增 | `skills::on_death`（`turn::finish_death` 唯一调用点，宗祚）；`skills::lianhuo_bonus` + `flow::propagate_chain(extra)`（链祸加伤随传导）；`flow::gain` 增加 `turn.<seat>.gained_cards` 记账（腹鳞）；`skills::draw_bonus` 改 `&mut`（督粮下回合 +1 消费）；`event_payload` 增加 `event` 字段（声明式订阅外的种类判定） |
| 用例 | `tests/b7.rs` 20 条；注册表 89 → **101** |
| 挂载 | `ENABLED_SKILLS` +12 |

**已知边界（登记）**：
- 督粮的「私密观看牌堆顶两张」用「只发给该角色的 pending」实现（公共区不 `reveal`），基本牌自动获得、其余按原顺序留在牌堆顶；
- 匡弼的「匡弼」牌进 `Zone::Mark(seat,"kuangbi")`（`CardOutOfGame`/`CardIntoGame`），放置者用 `kuangbi.src.<card>` 标记，回合开始 `TurnStart` 订阅收口；
- 宗祚按**开局快照**势力数 +体力上限/回复；势力最后一人死亡由 `finish_death → on_death` 触发（`flow::lose_max_hp`）；
- 链祸按伤害事件属性字段判定（不按牌名），+1 随传导生效，白银狮子仍最后结算；
- 讥谀的同花色封锁用 `Game::freeze_cards(FreezeUntil::Turn)`（只冻结当前手牌，后续获得的同花色牌不补冻，登记）；
- 寝情的手牌弃置为随机（`rand_below`），装备区由黄皓指定；
- 贿生的 opt1 用 `huisheng.used.<victim>.<source>` 每回合计数（回合结束自然失效）；
- 滔乱的「响应场景」（无懈链/万箭/南蛮只能当对应响应牌）**未实现**，当前只支持出牌阶段主动转化（登记为下一批次前补齐的缺口）。

### 7.11 B8 收口记录（2026-11-16，✅）

> 批次：一将成名·Y7（8 将 / 17 技能 + 4 条授予弦技）。

| 项 | 结果 |
|---|---|
| 技能 | `kernel/skills/b8.rs`：忠鉴/才识/福绵/怠宴/问卦/伏诛/守玺/惠民/清弦/绝响（+和弦/柔弦/烈弦/激弦）/谏征/专对/天辩/复难/诫训/辟撰/通博 |
| 内核新增 | `Hook::ShaTargeted` + `step_target` 目标级技能窗口（谏征重写目标 / 专对「不能响应」）；`skills::pindian_deck_option` + `pindian_rank`（天辩：牌堆顶拼点 / 红桃视为 K）；`skills::on_turn_start`（绝响保护到期）；`skills::on_use_start`（福绵：使用前的目标数/距离声明）；`flow::draw_bottom`（问卦）、`flow::shuffle_deck`（伏诛）；`flow.card`/`responds.<card>`/`used_by.<card>` 事实（复难）；`step_effect` 起始按实例剔除无效目标（绝响梅花保护）；`skills::deny_target`（才识：不能对自己使用牌） |
| 用例 | `tests/b8.rs` 22 条；注册表 101 → **122** |
| 挂载 | `ENABLED_SKILLS` +21 |

**已知边界（登记）**：
- 忠鉴「展示与其体力值等量张手牌」按体力值上限取数（手牌不足时展示全部）；
- 才识的「本回合不能对自己使用牌」拦截 `deny_target` 入口（含装备与【桃】/【酒】/【无中生有】）；
- 福绵的红色牌「多指定 N 个目标」在 `use_start` 同步声明（只抬高上限、不改最少目标数；本回合限一次）；
- 问卦的置牌用「牌回牌堆」实现（不发弃置/获得事件，登记）；徐氏本人不可发起；
- 伏诛按 `agreed` 用牌堆牌为承载牌真实结算（可【闪】，防具/属性照常），上限=游戏人数，结算后 `shuffle_deck`；
- 守玺的「每种牌名每回合限一次」用 `turn.<seat>.shouxi.<def>` 计数；
- 惠民按座位顺序逐个询问（同五谷），未被取走的牌进弃牌堆；
- 绝响的「令一名角色」通过 `Deferred::SkillTrigger` 在栈空后询问；保护按 `juexiang.protect.<seat>`（到其下回合开始清除）；
- 专对每个【杀】的「使用者/目标」时机各询问一次（`zhuandui.once.<card>.*`），拼点走内核 `pindian`（同时暗选）；
- 天辩的「牌堆顶拼点」只在双方仍有手牌时可用（拼点入口保留「双方有手牌」前置）；
- 复难按牌 id 交换（虚拟牌不参与，登记），升级后的复难plus 仅获得响应牌；
- 诫训的「场上方片牌」只统计装备区+判定区；「弃置 Y 张」Y=自游戏开始发动次数（从 1 起）；
- 辟撰的「书」进 `Zone::Mark("shu")` 并公共可见（`reveal`），每张 +1 手牌上限；
- 通博按「一次一张替换」循环（多选交互留待特殊交互区），四花色齐后可全部交给一名其他角色。

### 7.12 B9 收口记录（2026-11-16，✅）

> 批次：界限突破·界标·蜀（7 将 / 14 技能；其中「集智」与 B2 共享 code 并按界版升级）。

| 项 | 结果 |
|---|---|
| 技能 | `kernel/skills/b9.rs`：仁德/激将/义绝/武圣/替身/咆哮/涯角/龙胆/马术/铁骑/空城/观星/奇才；`b2::jizhi` 升级为界集智（非转化锦囊 + 摸到基本牌可弃置换上限） |
| 内核新增 | `skills::active` 增加「非锁定技失效」抑制（义绝/铁骑）；`skills::sha_damage_bonus`（义绝红桃【杀】/咆哮）；`skills::on_sha_dodged`（被【闪】抵消回调）；`skills::on_relay_sha`（代出【杀】回调，激将摸牌）；`skills::trick_no_distance`（奇才：顺手/兵粮无距离）；`skills::prevent_discard`（奇才：防止他人弃置防具/宝物）；`CardAs` 替代链扩展（武圣/龙胆）；`Hook::ShaTargeted` 铁骑接入 |
| 用例 | `tests/b9.rs` 19 条；注册表 122 → **135** |
| 挂载 | `ENABLED_SKILLS` +13 |

**已知边界（登记）**：
- 仁德的「视为使用基本牌」按跨次累计第 2 张触发一次；候选过滤【杀】次数与【酒】次数（【闪】不提供）；
- 激将的响应接力复用 `kernel/relay`（`for_use=true`），打出接力（响应窗口）仍走同一入口的后续批次补齐；
- 义绝的「非锁定技失效」用 `suppress.<seat>.<turn>` 计数（锁定技/装备特效不受影响），手牌封锁用 `freeze_hand(Turn)`；
- 龙胆/武圣的替代在 `skills::use_alias`/`response_as` 单入口，方片【杀】无距离在 `on_use_start` 声明；
- 咆哮的加成在命中时消费（`turn.<seat>.paoxiao_bonus`），白银狮子仍最后压为 1；
- 涯角的「展示牌堆顶」→ 同类型获得/不同类型弃置；不同类型的手牌弃置为随机；
- 铁骑判定后只对首个目标询问弃牌（多目标【杀】登记为边界）；
- 观星的排列为「逐张置顶/置底」（不提供任意顺序拖拽）；结束阶段再次发动不触发「再再发动」；
- 奇才只拦「弃置」不拦「获得」；锦囊无距离只覆盖顺手牵羊/兵粮寸断的距离检查。

### 7.13 B10 收口记录（2026-11-16，✅ · B5–B10 收官）

> 批次：界限突破·界标·吴（8 将 / 17 技能）。

| 项 | 结果 |
|---|---|
| 技能 | `kernel/skills/b10.rs`：奇袭/奋威/克己/勤学/博图/苦肉/诈降/英姿/反间/流离/国色/谦逊/连营/制衡/救援/枭姬/结姻 |
| 内核新增 | `skills::fire_multi_target`/`take_multi_invalid`（奋威：多目标牌任意无效）；`skills::skip_discard`（克己：显式跳过弃牌阶段）；`skills::on_equip_lost`（枭姬，`flow::leave_equip` 唯一调用点）；`skills::on_hand_empty`（连营：使用/打出/弃置/谦逊四条入口）；`skills::on_turn_end`（博图额外回合 / 谦逊归还手牌）；`skills::no_real_shan`（诈降红【杀】）；`skills::heal_redirect`（救援）；`skills::use_alias` 支持**装备区**承载牌（奇袭/国色），出牌阶段选项同步补齐；`hand_limit_override`（英姿赋值体力上限） |
| 用例 | `tests/b10.rs` 18 条；注册表 135 → **152** |
| 挂载 | `ENABLED_SKILLS` +17 |

**已知边界（登记）**：
- 奋威对「任意类型多目标牌」生效（目标确认后、无懈之前）；限定技用后不再询问；
- 勤学的觉醒收益已实现，**【攻心】未实现**（按 `align/B10` Q2=B 本批不做）；
- 博图按本回合进入弃牌堆的花色集合（`turn.<seat>.discard_suits`）判定，每轮上限 `min(存活,3)`；
- 诈降的「不能被真实【闪】抵消」只关真实【闪】选项（八卦/技能视为闪仍可用）；
- 英姿手牌上限 = 体力上限（赋值，覆盖其它加减）；
- 流离转移目标不可是使用者，且必须在自己的攻击范围内；
- 谦逊在**目标指定时**触发（`TargetChosen`），延时锦囊按放置时点处理（「判定生效时」登记为边界）；
- 连营的 X：谦逊移出 = 移出张数；使用/打出最后一张 = 1；弃置到 0 张按每次 1 计（细化「本次弃置张数」登记为边界）；
- 救援为**同步重定向**（满足条件即改为令主公回复，并在结算中令原角色摸一张），未做二次询问（登记）；
- 结姻的装备来源可为手牌或自己的装备区（后者经 `equip_to` 会触发枭姬）。

### 7.14 B6–B10 登记缺口收口（2026-11-16，✅）

> 口径（用户裁定）：**缺口必须在本批收口，不得拖到下一批开动。** 下表逐条对应 §7.9–§7.13 登记的缺口。

| # | 来源批次 | 缺口 | 收口方式 | 用例 |
|---|---|---|---|---|
| 1 | B7 | 滔乱的「响应场景」未实现 | `skills::response_alternatives(g, seat, kind, ctx)` 统一响应替代入口；滔乱覆盖 `(无懈,wuxie)/(闪,wanjian)/(杀,nanman)` 三场景（每牌名整局限一次） | `tests/gaps.rs::taoluan_responds_as_sha_in_nanman` / `_shan_in_wanjian` |
| 2 | B9 | 激将的「打出接力」缺失 | 同一入口：`Sha` 响应窗口提供 `alt:jijiang`（`relay::start(for_use=false)`，代出牌按「打出」消耗），并接入【决斗】/【南蛮】/【借刀】 | `tests/gaps.rs::jijiang_relays_sha_in_response_window` |
| 3 | B6 | 饰非未接入【借刀杀人】的【闪】询问 | 替代入口改为统一入口（所有需要【闪】的上下文都查询） | `tests/gaps.rs::shifei_alternative_in_wanjian` |
| 4 | B10 | 勤学的【攻心】未实现 | 注册 `gongxin`（grant_only）+ 实现（私密观看→展示红桃牌→弃置/置顶，出牌阶段限一次），勤学觉醒即 `grant` | `tests/gaps.rs::qinxue_grants_gongxin_and_it_discards_heart` |
| 5 | B10 | 救援为同步重定向（未询问） | `flow::heal` 改为「命令式询问」：`skills::heal_ask` 压询问帧，由 `jiuyuan` handler 收口（opt1 为主公回复+其摸1 / opt2 照常回复），`jiuyuan.busy` 防递归 | `tests/b10.rs::jiuyuan_redirects_heal_to_lord` |
| 6 | B10 | 连营「本次弃置张数」按 1 计 | 弃牌阶段在 `turn::step_discard` 按 `phase.discarded.Discard` 计数一次性触发；其余入口（使用/打出/谦逊）按 1 / 移出张数 | `tests/b10.rs::lianying_distributes_draws_after_hand_emptied` |
| 7 | B10 | 谦逊未覆盖「延时锦囊判定生效时」 | `turn::judge_phase` 增加 `skills::on_delayed_judge` 单出口（判定生效前压询问帧） | `tests/gaps.rs::qianxun_triggers_on_delayed_judge` |
| 8 | B9 | 观星不能任意排序 | 改为「每次任选一张剩余牌 → 顶/底」的交互，实现**任意顺序**（`top:<id>` / `bottom:<id>`） | `tests/b9.rs::guanxing_places_cards_and_repeats_at_end_if_all_bottom` |
| 9 | B9 | 铁骑多目标只询问首个目标 | 逐目标循环判定/询问；`UseOpts.unresponsive: Vec<usize>` 记录「逐目标不能使用【闪】」并由 `step_sha` 读取 | `tests/b9.rs::tieqi_judges_and_blocks_shan_without_discard` |
| 10 | B7 | 讥谀只冻结当前手牌，后续获得同花色仍可用 | 追加**花色封锁** `turn.<seat>.jiyu_lock.<花色>`，由 `skills::card_locked` 拦截使用/打出/响应/替代/`usable_hand` | `tests/gaps.rs::jiyu_suit_lock_blocks_use_and_response` |
| 11 | B8 | 天辩无手牌时不能拼点 | `pindian::can_start` 允许「无手牌但有天辩且牌堆非空」的一方 | `tests/gaps.rs::tianbian_allows_pindian_without_hand` |
| 12 | B8 | 问卦交牌无事件 | 交牌时 emit `CardLost`（原持有者/来源） | 复核于 `tests/b8.rs::wengua_other_side_places_card_and_draws` |

**保留（非引擎缺口，UI 形态）**：
- 通博的「任意张同时替换」引擎层已支持（逐张替换到放弃即任意张），差异仅在 Web 特殊交互区的展示形态（观星同源 UI 待 B11 之后统一实现）；
- 复难不交换**虚拟牌**（`v:` 无物理承载，规则上不可表示；已在实际牌路径完整实现）。

**门禁**：`cargo test -p sanguosha3` 全绿（新增 `tests/gaps.rs` 9 条）；`check:sgs3` 全绿；注册表 152 → **153**（+攻心）。

### 7.15 通博交互区 + 复难plus 收口（2026-11-16，✅）

| 项 | 结果 |
|---|---|
| 通博引擎 | `b8::tongbo` 重写为**两段式**：①`tongbo_swap`（左「书」/右「手牌」逐对交换，`swap:<book>:<hand>`，可反复，出口 `pass`=确认）→ ②四花色齐则 `target` **强制**交出所有「书」（不可放弃），否则结束；**装备区不参与**（候选仅手牌）；pending data 附带 `books/hand/labels` 供端侧渲染 |
| 通博端侧 | 新增 `apps/web/src/view/sgs/tongbo.ts`（纯函数：`tongboSwapAction` / `tongboColumns` / `renderTongboOverlay` / `tongboDropTarget`）+ `client/sgs.ts::initTongboDrag`（长按拖拽跨列交换、点击配对降级、确认按钮）；`pending.ts` 将 `tongbo_swap` 纳入阻断型与浮层分派 |
| 复难plus | 行为早已实现（诫训触发的 `funan.plus`：仅获得响应牌）；本次补**显示名同步** `skills::display_name`（复难 → 复难plus）并接入 V3 视图 `skills_view`（返回自带∪授予技能 + 显示名） |
| 用例 | `tests/b8.rs` 通博 3 条 + 复难plus 显示名 1 条；`apps/web/test-sgs.mts` 通博纯函数/渲染 1 组 |
| 文档 | `ui-spec.md §10`（通博交互规格）、本表 |

**门禁**：`cargo test -p sanguosha3` 全绿；`check:sgs3` 全绿；`tsc -p apps/web/tsconfig.json --noEmit` 通过；`tsx test-sgs.mts` 全绿；`build:client` 成功。

### 7.16 B11 收口（界标·魏：8 将 / 14 技能，2026-11-16，✅）

> 内核先行：本批新增机制 **`judge-rewrite`**（`align/mechanics.json` 47 条），
> 判定统一走流程帧 `kernel/quick_judge.rs`（亮牌 → 鬼才改判 → 生效 → 天妒 → 落盘）。

| 技能 | 收口要点 | 用例 |
|---|---|---|
| 奸雄 `jianxiong` | 「摸 1」与「获得伤害牌」**解耦**（虚拟牌/已离场也可摸 1）；伤害牌仍在处理区才询问获得 | `b11::jianxiong_*`（2） |
| 护驾 `hujia` | 主公【闪】响应替代走统一 `relay`（新增 `kind: Shan` 维度）；其他魏势力**回合外**使用/打出/代出【闪】可**选择**令主公摸 1（每回合限一次） | `b11::hujia_*`（2） |
| 反馈 `fankui` | 按**每 1 点伤害**逐次触发（多点伤害多次询问）；手牌随机、装备区可选 | `b11::fankui_*`（2） |
| 鬼才 `guicai` | 改判窗口：手牌 + 装备区；原判定牌进弃牌堆；无牌可换不询问；链式询问判定者最后 | `b11::guicai_*`（3） |
| 刚烈 `ganglie` | 判定红 → 对来源 1 点伤害；黑 → 弃置来源一张牌（手牌随机/装备区可选） | `b11::ganglie_*`（2） |
| 清俭 `qingjian` | 摸牌阶段外获得牌后（每回合一次）→ 展示并交出任意张（手牌/装备区，公示）→ **当前回合角色**按类型数 +手牌上限 | `b11::qingjian_*` |
| 突袭 `tuxi` | 摸牌阶段替代：N = 2 + 加成；先选摸 X（0≤X≤N），再向 N−X 名角色各拿一张手牌；`skip_draw` 单出口 | `b11::tuxi_*`（2） |
| 裸衣 `luoyi` | 亮牌堆顶 3 张；跳过摸牌则获得基本/武器/【决斗】+ 伤害 +1（到下回合开始）；放弃则 3 张进弃牌堆并正常摸牌 | `b11::luoyi_*`（2） |
| 天妒 `tiandu` | 判定牌生效后获得**生效的这张**（改判则获得改判牌） | `b11::tiandu_*`（2） |
| 遗计 `yiji` | 受到伤害后摸 2，逐张交给至多两名其他角色（合计至多两张，可自留） | `b11::yiji_*`（2） |
| 恂恂 `xunxun` | 摸牌阶段看顶 4 张，选 2 张置顶（**顺序可调**，先选先摸）、其余置底；不足 2 张不发动 | `b11::xunxun_*` |
| 忘隙 `wangxi` | 造成/受到 1 点伤害后摸 2 并交给其一张；对方已离场 → 两张都归自己 | `b11::wangxi_*`（2） |
| 倾国 `qingguo` | 黑色牌（手牌 + 装备区）当【闪】（响应替代单入口）；红色不出现 | `b11::qingguo_*` |
| 洛神 `luoshen` | 准备阶段反复判定（无上限）：黑色获得并继续；红色结束（牌进弃牌堆）；本回合获得牌不计入手牌上限（`hand_limit_bonus`） | `b11::luoshen_*`（2） |

**同时修掉的遗漏（B0–B10 审计）**：
1. `gongxin`（B10 勤学授予·攻心）**注册但未进 `ENABLED_SKILLS`** → 已补（否则任何钩子/订阅都会被 `enabled()` 静默拦住）；
2. 新增门禁 `npm run check:sgs3-registry`：`align/B*.json` ↔ 注册表 ↔ `ENABLED_SKILLS` ↔ `tests/bN.rs` 四方一致（已开工批次内**每个**技能都必须「已注册 ∩ 已收口 ∩ 有用例锚点」；
   不注册/不收口/无用例任一缺失即红）。`check:sgs3` 已包含该门禁。

**门禁**：`cargo test -p sanguosha3` 全绿（`tests/b11.rs` 26 条）；`check:sgs3` 全绿（含 47 条机制锚点 + 新版注册表门禁）。

### 7.17 B12 收口（界标·群：4 将 / 8 技能，2026-11-16，✅）

| 技能 | 收口要点 | 用例 |
|---|---|---|
| 急救 `jijiu` | **回合外**红色牌（手牌 + 装备区；判定区不参与）当【桃】；只出现在濒死救援窗口（`ji:<牌>`），承载牌落盘（装备区走离开钩子） | `b12::jijiu_*`（3） |
| 除疠 `chuli` | 出牌阶段限一次；选**势力各不相同**且有牌的其他角色（逐个点选 + 确认）；参与者各自弃一张（手牌/装备区）、黑桃者各摸 1 | `b12::chuli_*`（2） |
| 无双 `wushuang` | 锁定：`skills::response_need` 统一给张数；【杀】需连续 2 张【闪】、决斗每次需 2 张【杀】（对方有【无双】即生效） | `b12::wushuang_*`（2） |
| 利驭 `liyu` | 自己【杀】造成伤害后获得其一张牌（手牌随机/装备区可选）；非装备 → 其摸 1；装备 → **由受伤者指定**另一角色，吕布视为对其使用【决斗】（真实结算） | `b12::liyu_*`（3） |
| 义从 `yicong` | 双向距离：我→他人 −1（`Hook::Distance`）；体力 ≤2 时他人→我 +1（`skills::distance_to_bonus` 接入 `gear::distance`）；下限 1 | `b12::yicong_*`（2） |
| 趫猛 `qiaomeng` | 自己【杀】造成伤害后弃置其一张牌；**坐骑牌**改为获得 | `b12::qiaomeng_*` |
| 离间 `lijian` | 出牌阶段限一次：弃一张牌（手牌/装备区）→ 男性发起者 → 男性目标（非空城）→ 视为【决斗】且**跳过无懈窗口**（`no_wuxie`） | `b12::lijian_*`（2） |
| 闭月 `biyue` | 结束阶段开始：无手牌摸 2、否则摸 1（发动瞬间判定一次；结束阶段摸牌不触发弃牌） | `b12::biyue_*`（2） |

**门禁**：`tests/b12.rs` 17 条；`check:sgs3` 全绿；注册表 175 / 收口 175。

**已知边界（登记，不阻塞 B13）**：
- 鬼才改判窗口覆盖**所有**判定（延时锦囊 / 八卦阵 / 不屈 / 铁骑 / 恢拓 / 技能判定）；
- 无双的「八卦阵判定视为的【闪】」按一次响应计 1（判定成功即算作一张【闪】，与 v2 同口径）；
- 利驭视为【决斗】的无懈窗口按规则保留（可被【无懈可击】响应）。

### 7.18 B13 口径裁定（2026-11-16，开工前，✅ 机制已落地）

> 逐条裁定与理由见 `memory_palace/decisions/ADR-0044-b13-rulings.md`；机器记录见 `align/B13.json.v3_rulings`。

| # | 裁定 | 机制落点 | 用例 |
|---|---|---|---|
| Q1 | 化身池判据 = **已注册**（不要求已收口）；化身/新生走**授予技**（`skills::grant`），不依赖技能实现 | `kernel/skills.rs::huashen_skills_of` | `kernel_mechanics::huashen_skills_exclude_locked_awaken_lord_and_grant_only` |
| Q2 | 可授予技能排除 **锁定技 / 觉醒技 / 主公技 /（实现）授予技**；无可授予技能 → 不入池；不能化身**同场存活武将**、不能获得**同场已存在技能**、不能重复持有化身牌 | `skills::{huashen_generals, huashen_choices}`、`mod.rs::init_huashen` | `huashen_pool_only_has_generals_with_grantable_skills` / `..._excludes_self_and_generals_on_field` / `huashen_choices_exclude_skills_owned_by_other_seats` |
| Q4 | 池**动态**（按当前构建）；回放只读快照，不重算 | `init_huashen` 每次按当前 `all()` 计算 | 同上 |
| Q5 | 蛊惑可声明【无懈可击】；影响链须支持【杀】【闪】【桃】（濒死）【酒】（自救）【无懈】；作为无懈时**不再开无懈窗口** | 待 B13：`guhuo` 接入 `response_alternatives` / `dying_alternatives` / 无懈窗口守卫 | 待 B13 |
| Q6 | 缠怨：体力=1 时**所有技能**失效（含锁定技；装备特效按 `O3=A` 也算）；持续整局；独立闸门 `chanyuan.{seat}`（不复用回合制 `suppress.*`） | 待 B13：`skills::active` 前置闸门 | 待 B13 |

**已落地**：`Huashen.active_skill`、`Game::huashen_set_current`（势力/性别覆盖与恢复，恢复读 `Seat.general`）；
`tests/kernel_mechanics.rs` 扩到 5 条化身用例；`check:sgs3` / `check:sgs-align` / `check:spec-conformance` 全绿。
**未 bump** `mechanics.json` version（新增约束只作用于未实现的化身/新生；bump 会把 290 条历史确认整体判冲突）。

### 7.19 B13 收口（J1：8 将 / 16 技能 + 授予技 荐言/思蜀，2026-11-16，✅）

| 技能 | 收口要点 | 用例 |
|---|---|---|
| 化身 `huashen` | 池见 §7.18；`TurnStart` / `TurnEnd` 各一次：①替换展示（→ 从该牌选一个可授予技能）②2 换 1（新牌不在刚移去的两张里，下次可再换到） | `b13::huashen_*`（6） |
| 新生 `xinsheng` | 每 1 点伤害各询问一次；池空不触发；只增 `Game.huashen` | `b13::xinsheng_*`（2） |
| 鬼道 `guidao` | 改判链按 `Hook::JudgeRewrite` 声明（**黑色**牌）；黑桃 2~9 改判后摸 1；改自己判定置标记 | `b13::guidao_*`（2） |
| 雷击 `leiji` | 使用/打出【闪】或使用【闪电】→ 判定；黑桃 2 点雷伤 / 梅花先回复 1 再 1 点 / 其他无事；鬼道联动经 `on_judge_effective`（顺序：天妒 → 雷击 → 落盘） | `b13::leiji_*`（3） |
| 烈弓 `liegong` | 距离 ≤ 此【杀】点数即可达（`range_override`，逐目标）；发动后按条件封【闪】/加伤（逐目标计数） | `b13::liegong_*`（2） |
| 诛害 `zhuhai` | 他人结束阶段且其本回合造成过伤害 → 对其使用手牌【杀】（强制目标、无距离；响应/伤害走真实结算） | `b13::zhuhai_*` |
| 潜心 `qianxin` | 造成伤害后已受伤 → 觉醒（上限 −1）+ 授予【荐言】 | `b13::qianxin_*` |
| 荐言 `jianyan` | 授予技；出牌阶段限一次：声明 基本/锦囊/装备/红色/黑色 → 亮牌堆第一张符合的牌 → 交给一名**男性**角色；无男性不可发动 | `b13::jianyan_*` |
| 耀武 `yaowu` | 锁定：红色伤害牌 → 来源摸 1（来源无效则不摸）；非红/虚拟牌 → 自己摸 1 | `b13::yaowu_*` |
| 势斩 `shizhan` | 出牌阶段限两次：令他人视为对华雄使用【决斗】（华雄先出【杀】；可被无懈） | `b13::shizhan_*` |
| 黄天 `huangtian` | 主公技；其他**群**势力角色的出牌阶段（`other_side_actions` 入口）可交【闪】或黑桃手牌给主公 | `b13::huangtian_*` |
| 乱击 `luanji` | 两张**同花色手牌**当【万箭齐发】（双承载牌）；可在目标确定前少选一个目标 | `b13::luanji_*` |
| 血裔 `xueyi` | 开局按群势力数（含自己）×2 得「裔」；出牌阶段开始可移 1 摸 1；手牌上限 +裔数 | `b13::xueyi_*` |
| 享乐 `xiangle` | 锁定：成为【杀】目标后由**使用者**弃一张基本牌，否则【杀】无效；无基本牌直接无效（不产生 pending） | `b13::xiangle_*` |
| 放权 `fangquan` | 出牌阶段主动跳过后（`skip_current_play`），弃牌阶段开始可弃一张手牌令一名其他角色额外回合；无手牌则不询问 | `b13::fangquan_*` |
| 若愚 `ruoyu` | 主公+觉醒：体力**严格**小于全场其他角色 → 上限 +1、体力 <3 设为 3、授予【激将】【思蜀】 | `b13::ruoyu_*` |
| 思蜀 `sishu` | 授予技；出牌阶段每回合一次：反转/恢复目标【乐不思蜀】判定（切换目标时旧目标恢复） | `b13::sishu_*` |

**蛊惑 `guhuo`（Q5 影响链，5 个窗口）**：出牌阶段（扣置→声明任意基本/普通锦囊→目标→同时质疑→翻开→为真按声明牌真实结算）；
【杀】【闪】【无懈可击】响应替代（`response_alternatives`，为真写 `response.sub`）；濒死【桃】/【酒】自救（`step_dying` 的 `guhuo` 选项 + `dying.guhuo` mark 回传）。
**缠怨**：`Game.counters["chanyuan.<seat>"]` 单一事实源；`skills::active` 在体力为 1 时返回 false（Q6=所有技能失效，含锁定技/装备特效）；
质疑链跳过缠怨者；为真时质疑者获得缠怨并自选弃 1 张手牌或失去 1 点体力（为假则质疑者各摸 1）。
**蛊惑作为【无懈可击】时不再开无懈窗口**（`step_wuxie` 的 `alt` 分支直接收口）。

**已知边界（登记，不影响 16/16 收口）**：
- 出牌阶段声明列表排除 **【闪】**（无主动使用语义）；延时锦囊按原文排除；
- 诛害为**强制目标**（同 v2 `strict=false`）：不校验距离/目标拦截（空城、帷幕等），响应与伤害仍走真实结算；
- 蛊惑目标数量：借刀杀人固定 2、铁索 1–2、群体锦囊自动全选，其余 1；
- 化身池「可授予技能」= 已注册且非 锁定/觉醒/主公/授予技（§7.18 Q1/Q2）。

**门禁**：`cargo test -p sanguosha3` 全绿（`tests/b13.rs` 29 条）；`check:sgs3:full` 全绿（47 机制锚点 + 注册表 193/193 + align 775 锚点 + spec 契约）。

### 7.20 B14 收口（J2：8 将 / 20 技能，2026-11-16，✅）

> K1'–K4 已全部落地（K1' 走「虚拟红桃牌」口径，并新增 `check:sgs3-suit` 门禁）。

**B14 清单**：界孙策（制霸/激昂/魂姿）、界夏侯渊（神速/设变）、界卧龙诸葛亮（八阵/藏拙/火计/看破）、
界庞统（连环/涅槃）、界魏延（狂骨/奇谋）、界小乔（红颜/天香/飘零）、界孙坚（英魂/武烈）、
界庞德（马术=复用 B9 / 鞬出）。

| # | 内核前置 | 现状 | 建议落点 | 需确认 |
|---|---|---|---|---|
| K1' | **红颜的「视为红桃」口径** | V3 无全局花色出口（~60 处直读 `.suit`） | **用户口径（2026-11-16）**：不做全局属性改写；改为「**使用/打出时**生成虚拟红桃牌（保留点数），结算走虚拟牌花色」。落点 4 处：①`card_play::push_virtual_use`（转化/虚拟牌花色按归属者红颜改写——唯一出口，含防具/属性/响应读取）②`judge::reveal/effective`（黑桃判定→红桃）③`step_fire_attack`（他人展示小乔的牌判色）④B14 自身读牌（天香候选/手牌上限） | ✅ 已定（见下） |
| K2 | **弃牌阶段手牌计数出口**（藏拙：锦囊不计入手牌数） | `turn::step_discard` 直接用 `hand_size` | 新增 `skills::hand_size_for_discard` 并替换该处 | 无需确认（单纯单出口） |
| K3 | **看破：无懈不可被响应** | 无懈链无「不可响应」标记 | `d["no_chain"]` + `skills::wuxie_no_chain`；无懈帧入口先判该标记 | 无需确认 |
| K4 | **连环：重铸算「使用牌」** | `recast` 明确「不计使用」 | `recast` 末尾 `skills::on_recast`（仅连环梅花重铸：记 `turn.<seat>.trick_used` + emit `CardUsed`） | ✅ 2026-11-16 用户确认：**接受重铸产生一次 `CardUsed` 事件**（影响面：订阅 `CardUsed` 的技能会看到该事件，重铸本身仍不进牌结算流程） |

**K1' 说明（2026-11-16 用户裁定）**：**不引入**全局 `effective_suit` 回改 ~60 处读数。
「小乔打出的黑桃」在**使用/打出瞬间**变为「虚拟红桃牌（点数保留）」，结算只看这张虚拟牌；
承载牌（手牌/装备）本身**不修改、也不需要还原**（虚拟牌是独立牌实例，结算后承载牌照常进弃牌堆/留在装备区）。
未被该口径覆盖、需要单点处理的只有：①判定牌（改用判定结果出口）②**他人展示小乔的手牌**判色（火攻，1 处）③小乔自己的技能读牌（天香弃红桃、手牌上限）。
**红颜与武圣不可能共存**（用户 2026-11-16 指出，已核对）：化身是界左慈的技能，一个角色始终只有 1 个 `active_skill`，
替换展示 = 先撤销旧授予再授予新技能（一进一出）；且 Q2 已将**锁定技**排除在可授予技能之外 → 化身成小乔只能拿「天香/飘零」，拿不到红颜。

**读 B14 时已确认的语义要点（实施时照此，无需再问）**：
- 制霸：双向拼点（自己发起 / 他人挑战且可被拒绝）；**平局算挑战者没赢** → 孙策获得两张拼点牌（只取仍在弃牌堆的）；
- 激昂：同一张牌只触发一次（用 `turn.<seat>.jiang.once.<card>`）；「因弃置进弃牌堆」= 先在使用/成为目标时打标记，再在 `CardDiscarded` 时消费（每回合首次）；
- 魂姿：`Prepare` 且 hp==1 觉醒（上限−1，授予 英姿/英魂）；**本回合**结束阶段摸 2 或回复 1（用 `turn.<seat>.hunzi_end` 一次性）；
- 神速：至多三项、每项每回合一次；每选一项视为使用**无距离**的【杀】（`use_virtual` + `declare_no_distance`）；
- 设变：翻面钩子；移动场上一件装备到**另一名角色同槽空位**（`gear::move_equip`，不能移回原持有者）；
- 涅槃：濒死窗口（复用 B13 蛊惑式 `dying.*` mark 回传）；复原范围 = 弃判定区/手牌/装备区、`flipped=false`、断链、摸 3、回复至 min(3,上限)、失去涅槃，三选一授予（八阵/火计/看破）；
- 奇谋：失去 X 点体力（可到 0）；因 `lose_hp` 会压濒死帧，handler 用 `Step::Next` 保帧 → **濒死结算后**再摸 X 并写距离/次数修正；
- 天香：`Hook::DamageBefore` → 弃红桃防伤 + ①转移 1 点给其他角色（其摸 X=已损失体力，≤5）②令其他角色失去 1 点体力并获得该红桃牌；
- 飘零：结束阶段判定，红桃则置顶或交给一名角色；若给自己则弃一张手牌；
- 英魂：已受伤时选其他角色：①摸 X 弃 1 ②摸 1 弃 X；
- 武烈：限定技，结束阶段失去任意点体力（可濒死），每点令一名其他角色获得「烈」；有「烈」者受伤时移除 1 枚并**抵消该次全部伤害**（`skills::prevent_damage` 单出口）；
- 鞬出：`Hook::ShaTargeted`；弃其一张牌：基本牌 → 其获得此【杀】且**继续结算**；非基本牌 → 其不能抵消且本回合【杀】次数 +1（每回合最多 +1）。

**B14 收口结果**：`skills/b14.rs`（19 技能，马术复用 B9）+ `tests/b14.rs`（19 条，每技能正/反例）；
注册表 **212/212**；`check:sgs3:full` 全绿（含新增 `check:sgs3-suit`）。

| 技能 | 收口要点 |
|---|---|
| 制霸 | 双向拼点（自己发起 / 他人挑战且主公可拒绝）；平局算挑战者没赢；孙策得两张拼点牌 |
| 激昂 | 使用/成为【决斗】或红色【杀】目标后摸 1（同一张牌一次）；本回合首次因弃置进弃牌堆 → 失 1 体力获得 |
| 魂姿 | 准备阶段 hp==1 觉醒（上限−1，授予 英姿/英魂）；本回合结束阶段摸 2 或回复 1（一次性） |
| 神速 | 准备阶段至多三项（各一次）：跳判定+摸牌 / 弃装备牌跳摸牌... 每项视为使用**无距离**【杀】（自选目标） |
| 设变 | 翻面时移动场上一张装备到另一角色同槽空位（`gear::move_equip`，不能回原持有者） |
| 八阵 | `Hook::VirtualEquip(Armor, bagua_zhen)`；有真实防具时失效 |
| 藏拙 | 未使用锦囊时锦囊不计入手牌数（`skills::hand_size_for_discard` 单出口；上限不变） |
| 火计 / 连环 | 红牌当【火攻】/ 梅花当【铁索】；连环重铸算使用（`on_recast` → `CardUsed` + `trick_used`） |
| 看破 | 黑牌当【无懈】；转化的无懈**不可被响应**（`no_chain` 跳过无懈链） |
| 涅槃 | 濒死窗口（`dying.niepan` mark 回传）：弃所有牌、复原（翻面/横置/判定区）、摸 3、回复至 min(3,上限)、失去涅槃、三选一授予 |
| 狂骨 | 对距离 1 内造成伤害后（逐点）回复 1 或摸 1 |
| 奇谋 | 失去 X（可至 0）：濒死先结算，`Step::Next` 保帧后**再摸 X**；本回合距离 −X、杀次数 +X |
| 红颜 | 使用/打出时虚拟红桃（`suit_for_use`）、判定花色（`suit_for_judge`）、他人读牌（火攻 `suit_of_card`）；装备区红桃 → 上限=体力上限 |
| 天香 | `DamageBefore`：弃红桃防伤 + ①转移 1 点（目标摸已损失体力，≤5）②令其失 1 体力并获得该牌 |
| 飘零 | 结束阶段判定；红桃 → 置顶或交给角色（给自己则弃 1） |
| 英魂 | 已受伤：①令其摸 X 弃 1 ②摸 1 弃 X（X=已损失体力，无上限） |
| 武烈 | 结束阶段失去任意点体力（可濒死）：每点令一名其他角色得「烈」；有「烈」者受伤时移除 1 枚并抵消该次**全部**伤害（`prevent_damage` 单出口） |
| 鞬出 | `ShaTargeted`：弃其一张牌；基本牌 → 其获得此【杀】且继续结算；非基本 → 不可响应 + 本回合杀次数 +1（最多 +1） |

**已知边界（登记）**：
- 红颜的「视为红桃」只覆盖 4 个出口（使用/打出虚拟牌、判定、火攻判色、技能自身读牌）；其余技能直接读物理花色的点**不受**红颜影响（按 K1' 裁定，不做全局改写）；
- 神速的「视为使用【杀】」走 `use_virtual` + 自选目标（不校验距离），次数按内核 `use_start` 记账；
- 激昂的「首次弃置」以 `turn.<seat>.jiang.card.<card>` 标记 + `CardDiscarded` 消费（每回合一次）。

### 7.21 B15 开工前置（J3：8 将 / 19 技能，2026-11-16 · 未开工）

按 align 单逐技能读完后，B15 需要 **8 项内核前置**（比 B14 更重，涉及「谁可以被指定/谁能响应/阶段跳过/强制序列」）：

| # | 前置 | 涉及技能 | 现状 | 建议落点 |
|---|---|---|---|---|
| K5 | **AOE 目标排除**（巨象：不能成为【南蛮】目标） | 巨象 | `legal_targets` 有 `deny_target_card`/`card_ineffective`，但 AOE（AllAlive）在 `step_effect` 里整表生成 | 在 AOE 目标生成单点调用 `skills::aoe_excluded(g, actor, target, card)` |
| K6 | **自己回合内伤害防止 + 摸 2×点数**（帷幕） | 帷幕 | `prevent_damage` 是锁定防伤（返回 bool）；摸牌需在伤害入口一次完成 | 扩展 `prevent_damage` 为可返回「防止点数」，或新增 `Hook::DamagePreventAll` |
| K7 | **濒死窗口可用者过滤 + 濒死期技能失效闸门**（完杀） | 完杀 | 求桃窗口在 `step_dying` 内固定「自己→下家」，无「只有贾诩+濒死者可用桃」过滤；缠怨的失效闸门是整局制 | `skills::dying_can_use_tao(g, asker, target)` + `skills::suppress_in_dying(g, seat)` |
| K8 | **阶段跳过（判定区保留）+ 移动任意区域牌**（巧变） | 巧变 | `skip_phase` 已保留判定区（不结算不弃置）；但「把场上任意一张牌移动到另一角色合理区域」目前只有 `gear::move_equip`（装备区） | 新增 `skills::move_field_card` 单出口（手牌/装备/判定区） |
| K9 | **跨回合「回礼者」状态**（好施） | 好施 | 无 | Game marks/counters：`haoshi.giver`（覆盖）+ 鲁肃下回合开始清除 |
| K10 | **手牌交换事件**（缔盟） | 缔盟 | 无（`flow::gain` 是单向） | 新增 `flow::swap_hands(g, a, b)`：双方各失去再获得，事件成对 |
| K11 | **强制序列（乱武）** | 乱武 | 无「逐个其他角色二选一」的流程帧 | 新 `FlowKind::Luanwu`（座位序、每人二选一、完成后可选虚拟【杀】） |
| K12 | **屯田/酣战/烈刃/天义/长标/志继** | 6 技能 | 拼点已有 `pindian::start`（酣战需「对方随机牌」）；`push_virtual_use` 从 origin 取花色（长标需「无花色无点数」）；屯田需「回合外失牌 / 回合内弃杀」判定 | 分别：`pindian::start_random`、`push_virtual_use` 支持 `no_suit`、`CardLost`/`CardDiscarded` 订阅 + `田` 计数 + 距离钩子 |

**结论**：B15 先落 K5–K11（7 项内核），再写 19 技能；`K12` 是技能内组合。

### 7.22 B15 收口（J3：8 将 / 19 技能 + 授予技 急袭，2026-11-16，✅）

**第一组（6）**：

| 技能 | 收口要点 | 用例 |
|---|---|---|
| 肉林 `roulin` | 锁定；与女性互用【杀】需 2 张【闪】；与【无双】**相加**（3 张）。接入 `skills::response_need`（下限 vs 加成） | `b15::roulin_*` |
| 崩坏 `bengkuai` | 结束阶段：存在体力不大于你的其他角色（并列也算）→ 失 1 体力或减 1 上限；本回合酒【杀】造成伤害则失效 | `b15::bengkuai_*` |
| 酒池 `jiuchi` | 黑桃手牌当【酒】；【酒】**不限次数**且同回合**叠加**（`use_jiu` 累加 `sha_bonus`）；酒【杀】伤害后 `turn.<seat>.jiu_sha_hit` | `b15::jiuchi_*` |
| 暴虐 `baonue` | 主公技：其他**群**势力角色每造成 1 点伤害逐点询问 → 判定黑桃回 1 并获牌 | `b15::baonue_*` |
| 挑衅 `tiaoxin` | 出牌阶段先至多两次；目标不出【杀】/未造成伤害 → 姜维弃其一张；命中则本阶段不可再发动 | `b15::tiaoxin_*` |
| 志继 `zhiji` | 准备/结束阶段无手牌 → 回复 1 或摸 2（满体力只给摸 2）→ 上限 −1、获【观星】 | `b15::zhiji_*`（2） |

**实现中发现并绕开的坑（记录，供后续批次复用）**：
1. `DamageAfter` 等事件**只在栈顶是回合帧或栈空时路由**；技能帧存活期间收不到 → 挑衅改用「体力差」判定伤害是否造成（单一事实：体力值）；
2. 多步技能用 `Step::Next` **会整份替换 data**：`by/event/phase/left` 等首帧判据必须在后续步骤显式携带，或把守卫限制在首帧（暴虐/神速/魂姿都踩过）；
3. 限定技门禁（`limited_used`）只能在**首帧**校验，否则标记置位后的后续步骤会被自己挡住（涅槃/武烈/奇谋同型）。

**第二组（13）——实现要点**：

| 技能 | 收口方式 |
|---|---|
| 天义 `tianyi` | `target` →（有酣战则 `hanzhan_pre`）→ 拼点 → `wait`：赢→`tianyi_won`（次数+1/无距离/多一目标，经 `on_use_start` + `sha_extra`）；没赢→`tianyi_lost`（`begin_use` 拒绝使用【杀】，响应打出不受限） |
| 酣战 `hanzhan` | 拼点前（`hanzhan_pre`→`pindian::start_random`）+ 拼点后（`hanzhan`：获得两张拼点牌中最大点数的**实体【杀】**，同最大由玩家选） |
| 屯田 `tuotian` | `CardLost(Self)`（回合外）/`CardDiscarded(Self)`（回合内弃【杀】）→ 判定：非红桃 → 判定牌进 `Void`、`tian.<seat>`+1；每田距离 −1 |
| 凿险 `zaoxian` | `Prepare` 且 `tian>=3` → 上限 −1、授予【急袭】、本回合后额外回合 |
| 急袭 `jixi`（授予） | 出牌阶段限一次：消耗 1 枚「田」→ 视为使用【顺手牵羊】（田以计数表示，见边界） |
| 巨象 `juxiang` | 锁定：AOE 目标排除 + 结算结束后获得他人使用的【南蛮】（`aoe_excluded` / `on_card_finished`） |
| 烈刃 `liereng` | 自己【杀】造成伤害后（目标存活）→ 拼点；赢则随机获得其手牌/装备一张 |
| 长标 `changbiao` | 多选手牌 → `use_as(sha)` 无距离 → 比较目标体力判定命中 → **出牌阶段结束**摸 X（X=牌数）；本阶段限一次 |
| 帷幕 `weimu` | 锁定：黑色锦囊不能指定（实例级）+ 自己回合内伤害**全部**防止并摸 2×点数 |
| 乱武 `luanwu` | 限定技：按座位序逐个其他角色二选一（对距离最小者使用【杀】/失去 1 体力）；完毕后可视为使用无距离【杀】（不计次数） |
| 完杀 `wansha` | 锁定：贾诩回合内只有贾诩与濒死者可用【桃】；濒死结算中其他角色非锁定技失效 |
| 好施 `haoshi` | 摸牌阶段多摸 2；手牌 >5 交一半给手牌最少者（并列可选）→ 记回礼者；成为【杀】/普通锦囊目标后回礼者可交一张；鲁肃回合开始清除 |
| 缔盟 `dimeng` | 选两名其他角色（手牌差 ≤ 自己手牌）→ `flow::swap_hands`；**出牌阶段结束**时弃 X 张（X=手牌差） |
| 巧变 `qiaobian` | 开局 2「变」；判定/摸牌/出牌/弃牌阶段开始可（弃一张牌 / 移 1「变」）跳过：判定区保留、摸牌→获至多两名各一张随机手牌、出牌→移动场上一张牌、弃牌→直接跳；结束阶段手牌数为新值→+1「变」 |

**表示性边界（登记）**：
- 「田」以**计数**表示（判定牌进 `Void`），急袭消耗 1 枚；【顺手牵羊】不读牌面，行为等价；
- 长标的「多张转化无花色无点数」在 `Card.suit: Suit` 下无法表示 → 取占位值（本回合不可观测），行为等价；
- 酣战目前接在**天义**的拼点流程（太史慈唯一的拼点来源）；若将来出现其它让其发起拼点的技能，需再接 `pindian` 前置钩子。

**用例**：`tests/b15.rs` **22 条**（19 技能各含正/反例 + 授予技 急袭 + 巧变两条）。
**门禁**：`check:sgs3:full` 全绿；注册表 **232/232（B0–B15 已开工）**。

### 7.23 B15 内核前置落地（K5–K12，2026-11-16 ✅ · 技能待接）

> 口径：**先补内核再写技能**。以下 8 项均为**单一出口 + 惰性**（未注册对应技能时不生效），
> 已接入既有结算路径并通过全量回归（`cargo test -p sanguosha3` 全绿）。

| # | 内核落点 | 单出口 | 说明 |
|---|---|---|---|
| K5 | 群体牌目标排除（巨象） | `skills::aoe_excluded` ← `card_play::legal_targets_with` 的 AllAlive 分支 | 在此之前 AOE 目标**完全绕过**「不能成为目标」（原实现只在非 AllAlive 分支判定）——这是一个既有缺陷，本项一并修掉 |
| K5b | 结算结束后钩子（巨象获牌） | `skills::on_card_finished` ← `card_play::finish_card`（使用者由 `card.owner.<card>` 记录） | 虚拟牌与实体牌两条路径都调用；实体牌在落盘后调用 |
| K6 | 实例级「不能成为目标」（帷幕黑锦囊） | `skills::deny_target_instance` ← `legal_targets_with`（两种目标模式都判） | 需要**实体牌花色**，def-only 的旧钩子无法表达 |
| K6b | 回合内**全防伤 + 摸 2×点数** | `skills::try_full_prevent` ← `flow::damage_one`（防具数值链之前） | 现有 `prevent_damage` 只返回 bool，无法带回点数 |
| K7 | 完杀（濒死求桃限制） | `skills::dying_allow_tao` ← `card_play::step_dying` 的**选项生成 + 提交校验**两处（legal≡execute） | — |
| K7b | 濒死期非锁定技失效 | `b15::suppressed_in_dying` ← `skills::active` 前置闸门（扫描 Dying 帧） | **不得**在闸门内再用 `skills::active`（会无限递归）→ 用 `owns` 直查自带∪授予 |
| K8 | 任意区域移牌（巧变） | `skills::move_field_card` | 手牌→手牌 / 装备→同槽（空）否则手牌 / 判定→判定（同名去重，占则弃置） |
| K10 | 手牌交换（缔盟） | `flow::swap_hands` | 双方先快照，再各自 `gain` 到对方（成对 `CardLost`/`CardGained`） |
| K12a | 拼点目标随机牌（酣战） | `pindian::start_random` / `start_with_forced` | 目标不产生选择 pending，内核随机取一张手牌 |
| K12b | 屯田距离 / 天义限制与加成 | `skills::distance_bonus`（+田数）/ `sha_use_blocked` / `sha_extra` / `on_use_start` | 天义没赢：`begin_use` 拒绝使用【杀】（响应打出不受限） |

**回归**：`cargo test -p sanguosha3` 全绿；`check:sgs3`（含注册表 218/218 + suit 门禁）全绿。
**修复的既有缺陷**：AOE 目标不经「不能成为目标」判定（K5 顺带修掉，附回归用例待 B15 技能接入时补）。

### 7.24 B15 剩余 13 技能的实现要点（待写；本轮未落代码，树保持绿）

> 本轮已把内核前置（§7.23）与 6 个技能收口；剩余 13 个技能的**交互步骤设计**如下，
> 下一轮直接照此写 handler（**不要**再动内核）。所有口径来自 align B15，无需再确认。

| 技能 | 交互步骤（handler 状态机） | 已就绪的内核 |
|---|---|---|
| 天义 `tianyi` | `target`（选拼点对象）→（有酣战则 `hanzhan_pre`：是否令对方随机）→ 拼点 → `wait`：赢→`turn.<seat>.tianyi_won`；没赢→`tianyi_lost` → `hanzhan`（获得最大点数实体【杀】） | `pindian::{start,start_random}` / `sha_extra` / `sha_use_blocked` / `on_use_start`（无距离 + 多目标） |
| 酣战 `hanzhan` | 无独立入口：作为天义 `hanzhan_pre` + `hanzhan` 两段（若将来出现其它拼点来源，再接 `pindian` 前置钩子） | 同上 |
| 屯田 `tuotian` | 订阅 `CardLost(Self)`（回合外）/`CardDiscarded(Self)`（回合内弃【杀】）→ 询问 → `quick_judge` 续接 `result`：非红桃 → `tian.<seat>`+1，判定牌进 `Void` | `quick_judge` / `distance_bonus`（每田 −1） |
| 凿险 `zaoxian` | `Prepare` 且 `tian>=3` → 上限 −1、授予 `jixi`、`turn::extra_turn(owner)` | `extra_turn` |
| 急袭 `jixi`（授予） | 出牌阶段限一次：消耗 1 枚「田」→ `use_virtual(顺手牵羊, [目标])`。**田以计数表示**（判定牌在 Void）；因【顺手】不读牌面，行为等价（登记为表示性口径） | `use_virtual` |
| 巨象 `juxiang` | 无 handler：仅声明锁定技 | `aoe_excluded`（目标排除）/`on_card_finished`（结算后获得）——**已接线** |
| 烈刃 `liereng` | `DamageAfter(By)` 首帧校验目标存活 → 询问 → 拼点 → `wait`：赢则随机获得其手牌/装备一张 | `pindian::start` |
| 长标 `changbiao` | `pick`（多选手牌，`confirm` 收口）→ `target` → `use_as(sha, picked, [t])` + 无距离 → `wait`（比较目标体力判定是否造成伤害）→ 出牌阶段结束 `PhaseStart(End)` 若命中则摸 X（X = 牌数） | `use_as` / `declare_no_distance`。**多张转化「无花色无点数」**在现有 `Card.suit: Suit` 下无法表示 → 用占位值（本回合不可观测），登记为表示性边界 |
| 帷幕 `weimu` | 无 handler：仅声明锁定技 | `deny_target_instance`（黑锦囊目标）+ `try_full_prevent`（自己回合内全防伤并摸 2×）——**已接线** |
| 乱武 `luanwu` | `ask` → `seat`：按座位序逐个其他角色询问（`sha` 需手牌有【杀】且存在距离最小者 / `lose_hp`）→ `sha_target`（并列最小由该角色选）→ 全部完毕后 `final`：可选视为使用无距离【杀】（`declare_sha_not_counted`） | 无需新内核（技能帧 `Step::Next` 迭代） |
| 完杀 `wansha` | 无 handler：仅声明锁定技 | `dying_allow_tao` + `suppressed_in_dying`（`active` 闸门）——**已接线** |
| 好施 `haoshi` | `PhaseStart(Draw)`：多摸 2 → 手牌 >5 则交一半给手牌最少者（并列可选）→ 记 `haoshi.giver`；订阅 `TargetChosen(Self)`：回礼者可交一张；`TurnStart`：鲁肃回合开始清除回礼者 | 无需新内核（marks） |
| 缔盟 `dimeng` | `first` → `second`（校验手牌差 ≤ 自己手牌）→ `flow::swap_hands` → 记 `dimeng_x`；`PhaseStart(End)` → `cost` 逐张弃 X | `flow::swap_hands` |
| 巧变 `qiaobian` | `GameStart` +2「变」；`PhaseStart(Judge/Draw/Play/Discard)`：`ask`（弃一张牌 / 移 1 变）→ `pick` 或直接 `after`：判定→`skip_phase(Judge)`（判定区保留）；摸牌→`skip_phase(Draw)` + `draw_pick`（至多两名各一张随机手牌）；出牌→`skip_current_play` + `move_pick`（`move_field_card`）；弃牌→`skip_phase(Discard)`；`PhaseStart(End)`：手牌数为新值 → +1 变 | `skip_phase` / `skip_current_play` / `move_field_card` |

**harness 教训（本轮踩到）**：一次性 append 超长（>~30KB）的命令会被 shell 拒绝（`spawn ENAMETOOLONG`），
且**不会**写入文件——大段代码一律用 `write` 工具落盘后再拼接，禁止 heredoc 直填。

### 7.25 B16–B20 开工交接（新会话从这里开始，2026-11-16）

**起点**：`已开工 B0–B15，注册表 232/232`（一条命令确认：`npm run check:sgs3`）。

| 批次 | 来源 | 规模 | 武将 |
|---|---|---|---|
| B16 | 界限突破·J4 | 8 将 / 14 技能 | 界孟获、界徐晃、界蔡文姬、界荀彧、界典韦、界颜良文丑、界张昭张纮、界徐盛 |
| B17 | 界限突破·J5 | 8 将 / 14 技能 | 界凌统、界陈宫、界曹彰、界简雍、界虞翻、界高顺、界法正、界程普 |
| B18 | 界限突破·J6 | 8 将 / 17 技能 | 界王异、界张春华、界刘表、界蔡夫人、界曹冲、界曹植、界吴国太、界廖化 |
| B19 | 界限突破·J7 | 8 将 / 15 技能 | 界李儒、界郭淮、界伏皇后、界韩当、界关兴张苞、界马岱、界郭皇后、界沮授 |
| B20 | 界限突破·J8 | 4 将 / 8 技能 | 界夏侯氏、界张松、界于禁、界步练师 |

**每批标准流程（B11–B15 已验证）**
1. 读 `align/B<NN>.json` 的 `agreed` + `assertions`（55 条裁定已全部经用户确认，**不要再问口径**）；
2. **先补内核前置**（若有）：单一出口 + 门禁，绝不写成"技能里各自造"；
3. 写 `kernel/skills/b<NN>.rs`（`SKILLS` 声明 + handler + `availability`），在 `skills.rs` 三处接线
   （`mod` / `all()` / `handler()` / `availability()` / `ENABLED_SKILLS`）；
4. 写 `tests/b<NN>.rs`（**每技能正例 + 反例**）；
5. `check:sgs3-registry` 的 `V3_IN_PROGRESS_BATCHES` 加本批 → 全量注册后移入 `V3_STARTED_BATCHES`；
6. 跑 `npm run check:sgs3:full`；更新 `§7.2x` 收口表 + `next-v3-rebuild` 进度表。

**必须复用的踩坑清单**
- 事件**只在栈顶是回合帧或栈空时路由**：技能帧存活期间收不到 `DamageAfter` 等事件 → 用"体力差"等状态判定；
- `Step::Next` **整份替换 data**：`by/event/phase/left` 等首帧判据必须在后续步骤显式携带，或守卫只在首帧；
- **限定技门禁只在首帧**校验（标记置位后会挡住自己）；
- 大段代码**必须用 `write` 工具**落盘再拼接（heredoc 超长会 `spawn ENAMETOOLONG` 且静默不写入）。

**已知/表示性边界**（勿当缺陷修）：田=计数（急袭消耗 1）、长标多张转化无花色（占位值）、
酣战接在天义流程、蛊惑不可被无懈响应自身、红颜仅 4 个出口。

### 7.26 B16–B20 内核前置第一批（K13/K14/K16/K23，2026-11-16）

> 用户裁定：**机制优先**；行殇等不得在内核硬编码。Q2=A（不进 mechanics.json，回归旧批次）；Q5=A（`on_turn_end` 在 `turn.*` 清空前）。

| # | 内核能力 | 落点 | 用例 |
|---|---|---|---|
| K13 | 单动作批次（action_seq / begin_action_as / lost_batch / gain_batch / discard_batch / action_seen） | `kernel/mod.rs`、`resolve::submit`、`flow::note_lost` | `tests/prereq_b16_20.rs::action_batches_are_cleared_between_actions` |
| K14 | 进入弃牌堆事实（CardToDiscard，cause 区分；红色实体牌计数 `turn.red_to_discard`） | `flow::record_to_discard` / `card_play::to_discard_with` / `judge` / `pindian` / `guhuo` | `tests/prereq_b16_20.rs::card_to_discard_is_emitted_for_discard_and_red_counter_tracks_physical_suit` |
| K16a | 死亡生命周期：`Death`（死亡时窗口）→ 死亡弃牌 `CardToDiscard(cause=死亡)` → `DeathDiscarded`（行殇订阅）→ 判定区 `JudgeZoneLeft` → 奖惩/胜负；`DeathCleanup` 延迟项 | `turn::settle_death/death_cleanup`、`deferred::Payload::DeathCleanup`、`b0::xingshang` | `tests/prereq_b16_20.rs::death_order_is_death_then_discard_then_death_discarded_and_xingshang_gains_from_discard`；`tests/b0_batch.rs`、`tests/route.rs` |
| K23 | 机制偏离整改：不屈走 `Hook::Dying`、解围走 `Hook::WuxieOpt`、涅槃/蛊惑并入通用濒死选项（`ds:` + handler 起始 step）、天妒走 `judge_gain_owner`、hp/max_hp 直写改 `flow::restore_hp/gain_max_hp` | `flow`/`card_play`/`quick_judge`/`skills`/`b0`/`b3`/`b7`/`b11`/`b12`/`b13`/`b14` | `npm run check:sgs3-conventions` + B0–B15 全量回归 |

**口径**：
- 死亡弃牌**不发** `CardDiscarded`（弃置契约不变）；行殇只处理 `DeathDiscarded.cards`（手牌+装备，不含判定区）。
- 不屈/涅槃/仁心/伏枥等「回复至 X」走 `flow::restore_hp`（不触发救援改向）；体力上限变化走 `flow::gain_max_hp`。
- `guhuo.rs` 作为 hidden-declaration 机制专用流程保留（C3 门禁白名单，理由：蛊惑机制子系统；后续若通用化再移除）。

### 7.26b 机制偏离审计（用户 2026-11-16 指出「严重偏离」）

- 事实：行殇在 V3 **B0 收口**（commit `197c28b`）被写成 `settle_death` 硬编码预询问；`60d50f8` 的 46/46 内核门禁只校验 47 条机制锚点存在，未校验「内核零技能名分支」，故未拦住。
- 整改：已移除 `xingshang`/`buqu`/`jiewei`/`niepan`/`guhuo` 的内核硬编码分支；新增 `check:sgs3-conventions` 门禁（C1 技能名分支 / C2 直写 hp / C3 内核技能 code 字面量 / C4 绕过弃牌堆单出口），并接入 `check:sgs3`。
- 回归：`cargo test -p sanguosha3` 全绿（B0–B15 全部既有用例）；`npm run check:sgs3` 全绿。
- 剩余（后续 P0）：K15 额外阶段/跳过摸牌、K17 装备区单出口、K18 牌标记/回合末归还、K19 伤害事实扩展、K20 目标/响应限制、K21 数值与计数域、K22 回合外/重铸/两牌打出。

### 7.26c B16–B20 内核前置第二批（K15/K17/K18/K19，2026-11-16）

| # | 内核能力 | 落点 | 用例 |
|---|---|---|---|
| K15 | 额外摸牌阶段（截辎）/ 额外出牌阶段（当先）骨架 + 跳过摸牌钩子；`Game.extra_play` | `turn::finish_phase`、`skills::on_skip_draw/take_extra_draw/take_extra_play` | 由 B16/B18 技能接入时补驱动用例 |
| K17 | 手牌装备置入他人装备区（替换）/ 两角色装备区交换 | `gear::place_from_hand` / `gear::swap_zones` | `tests/prereq_b16_20.rs::gear_*` |
| K18 | 牌级标记（离开手牌清除）+ 移出游戏至回合末归还 | `Game.card_marks/void_return`、`flow::exile_until_turn_end/return_due_void`、`turn::end_turn` | `tests/prereq_b16_20.rs::card_mark_*` / `void_return_*` |
| K19 | 伤害事件携带牌 id（`turn/phase.damage_card.<card>`）、伤害转失去体力、AOE 伤害来源替换 | `Game.damage_card`、`flow::damage_one/lose_hp_by`、`skills::damage_to_hp_loss/aoe_damage_source` | `tests/prereq_b16_20.rs::damage_event_carries_card_and_counters` |
| K16b | 永久失去所有技能（断肠） | `Seat.skills_lost` + `skills::revoke_all` + `active` 闸门 | 由 B16 断肠接入时补用例 |

**执行顺序调整（机制优先按批次）**：B16 所需前置（K13/K14/K16a/K16b/K17/K18/K19）已落地；
B17 所需 K20（目标/响应限制）与 K21/K22 在 **B17 开工前**落地，不阻塞 B16。该拆分不降低门槛：
每批开工前仍要求「本批所需机制 + 门禁 + 用例」全绿。

### 7.27 B16 收口（J4：8 将 / 14 技能，2026-11-16，✅）

**武将/技能**：界孟获（祸首/再起）、界徐晃（断粮/截辎）、界蔡文姬（断肠/悲歌）、界荀彧（驱虎/节命）、
界典韦（强袭/狞恶）、界颜良文丑（双雄）、界张昭张纮（直谏/固政）、界徐盛（破军）。

| 技能 | 收口要点 |
|---|---|
| 祸首 | AOE 目标排除（南蛮不能指定祸首持有者）+ `aoe_damage_source` 来源替换（自己使用不替换） |
| 再起 | 结束阶段按 `turn.red_to_discard` 逐次：目标自选摸 1 / 令孟获回复 1；可重复选同一目标 |
| 断粮 | 黑色基本/装备牌当【兵粮寸断】；本回合未造成伤害时 `trick_no_distance`（仅兵粮） |
| 截辎 | `on_skip_draw` 询问；手牌最少且无「辎」→ 给「辎」；`take_extra_draw` 在摸牌阶段结束重跑 Draw 阶段 |
| 断肠 | `Death` 死亡时窗口 → `revoke_all(killer)`；先于死亡弃牌（杀死者=曹丕时行殇不再触发） |
| 悲歌 | 杀伤害后弃一张牌 → `quick_judge`；红桃回复/方片摸2/梅花来源弃2/黑桃翻面；界版同花色获判定牌、同点数获弃置牌 |
| 驱虎 | 拼点；赢→令目标对其攻击范围内角色造成 1 点伤害；没赢→目标对荀彧造成 1 点（均无防具减免） |
| 节命 | `DamageAfter`/`Death` → 目标摸 X 并弃至 X（X=min(5,体力上限)）；死亡时窗口可 pending |
| 强袭 | 出牌阶段限两次；失 1 体力或弃武器 → 对本回合未指定过的角色 1 点伤害（无距离） |
| 狞恶 | 每名角色每回合第二次受伤后，受害者/来源为典韦 → 摸 1 并弃其一张（装备具体/手牌随机） |
| 双雄 | 摸牌阶段结束弃一张 → 本回合异色牌当【决斗】；结束阶段取回本回合伤你的牌 |
| 直谏 | 手牌装备置入其他角色装备区（替换旧装备走失去钩子）→ 摸 1 |
| 固政 | 其他角色单次弃置 ≥2 → 弃置者收回一张，张昭张纮获得其余；每阶段一次 |
| 破军 | `ShaTargeted` 逐目标移出至多 X 张至回合末（`void_return`）；手牌+装备数不多于你时杀伤害 +1 |

**用例**：`tests/b16.rs` 19 条（14 技能正/反例 + 若干边界）；`cargo test -p sanguosha3` 全绿；`npm run check:sgs3:full` 全绿。
**注册表**：B16 14/14，注册表 **246/246**，下一批 B17。

**已知边界（登记）**：
- 截辎的「额外摸牌阶段」通过 `turn::finish_phase` 重跑 Draw 阶段实现（PhaseStart/End 齐全）；技能驱动用例在 `tests/b16.rs` 直接验证 `take_extra_draw`，完整回合驱动待真机抽检。
- 悲歌梅花分支（来源弃 2）已实现但未纳入本批用例；后续抽检补。
- 狞恶的「每回合第二次受伤」以 `turn.<victim>.ninge_hits` 计数；重复武将/多持有者不在本批范围。
- 双雄取回范围 = 本回合对**你**造成过伤害的牌（`turn.<owner>.damage_card_taken.*`）；已不在弃牌堆/处理区/虚空的牌不取回。
