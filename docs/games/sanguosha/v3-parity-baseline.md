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
