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
