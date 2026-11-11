# V2 引擎技能自检（机制缺口 + 迁移欠账）

> 生成：2026-11-11 · 范围：`crates/engines/sanguosha2`（296 个 SkillDef / 147 位 live 武将 / 775 条对齐单裁定项）
> 方法：静态扫描（SkillDef / handler / 内核字面量 / 视图投影 / 计数器 / 用例密度）+ 运行时探针（143 将沙盒开局 + 合法动作归属）
> 边界：本文件是**定性 + 修复方案**，不改代码、不判语义通过。原始数据：`data/selfaudit2.json`（本机）。

## 0. 结论（分类已修正）

上一版把「迁移欠账」误报成「机制缺口」。按用户口径重新分类：

### 0.1 真正的机制缺口（引擎合约层，必须先补）

| # | 缺口 | 状态 |
|---|---|---|
| **M1** | **事件缺「发生时刻」快照**（turn/phase/lifecycle）——窗口与 handler 读 live 状态 | **已补**（ADR-0039） |
| **M2** | **开局/发牌不是一等生命周期**——发牌事件进入技能触发通道 | **已补**（`Lifecycle::Setup`） |
| **M13** | **牌移动缺「施动者/来源」**——无法表达「被其他角色弃置/获得」 | **已补**（`Event.by/from` + `Game.action_actor`） |
| **M14** | **牌堆耗尽未把弃牌堆洗回**——双方无牌可摸/可出，回合空转不终局 | **已补**（`Game::refill_draw_from_discard`；自对弈 seed1 game0 由 stalled 2000 步 → 351 步终局） |
| **M15** | **并发响应窗口的陈旧回答**——旧窗口的 `respond:<牌>` 落到新窗口 → 非法动作 | **已补**（不持有该牌时按「未响应」处理并记日志；合法集只描述当前一步） |
| **M12** | **回合/阶段结束的结算时序**——`PhaseEnd` 在切 turn 后路由，`turn.*` 已清空 | **部分**：`begin_game_at` 伪 PhaseEnd 已修；B（延迟切 turn）在自对弈 seed1 出现卡死/回环（stalled），已回滚为 A，B 需换「计数快照」实现（未收口） |
| M9 | 「必选/可跳过」语义（`escape` vs `escape_mandatory`） | 门禁已拦（23 处显式白名单）；「空选项自动跳过」待实现 |
| M8 | 计数器作用域无注册（非标准键手写清理） | 门禁已拦（10 处显式白名单）；`Scope` 注册表待实现 |
| M11 | 开局「就绪」语义 | 探针已用「pending 分类」验证；`awaiting_input` API 待补 |

### 0.2 V2 全量替换中应完成但未收口的缺陷（属 bug/欠账，不是机制缺失）

这四项在上一会话 S0–S6 的验收里就应该关掉；现在呈报是因为**当时门禁没有覆盖到**（§1.3）：

| # | 缺陷 | 证据 |
|---|---|---|
| **B1** | 技能类型元数据丢失：`SkillDef` 无 `kind` | **已修**：`SkillKind` 注册字段（202 normal / 49 locked / 13 lord / 8 awaken / 12 limited），catalog 恢复类型列；主公技统一闸门（`lord_gate_ok`） |
| **B2** | 技能层体力写入未收口：26 处直接写 `Seat.hp` | **已修**：迁移到 `flow::heal/lose_hp/revive_to/revive_add/revive_to/clamp_hp/heal_raw`，`check:sgs2-conventions` C1 归零 |
| **B3** | 可见标记投影缺失：裔/变/辎 | **已修**：`VISIBLE_COUNTER_MARKS` 注册 + 视图投影（C3 门禁） |
| **B4** | 内核按技能名分支 17 处 | **门禁已拦**（C2，显式白名单 17 条）；抽通用钩子（V2.2）待排 |

### 0.3 本次已修的实现缺陷

- 奋激：口径由 A 改为 **B**（被其他角色弃置/获得；排除摸牌阶段摸牌与自己的弃牌）；不再用「回合外」。
- 清俭：改读 `cause == "draw_phase"`（不再读 live `phase`）。
- 据守：弃置步骤改 `escape_mandatory`（对齐单 B0 Q2）。
- 18 处 `can_active` 补归属校验（legal 不再暴露他人技能）。

### 0.4 是否需要 V3

**不需要整体重构**。M1/M2/M13 已在 V2 内以兼容方式落地；M12 是唯一可能触及 `begin_phase/tick` 时序的项，
可先按「读快照、结算滞后」运行，再决定是否重排时序。V3 的判断标准见 §5。

---

## 1. 方法与证据

### 1.1 静态扫描（296 SkillDef）

| 检查 | 依据规范 | 命中 | 归属 |
|---|---|---|---|
| `can_active` 是否校验 `active(g, seat, code)` | §5 legal≡execute | 0（已修 18 处） | 已修 |
| 技能直接写 `Seat.hp` | §6 失去体力独立入口 | **26** | B2 |
| 技能调用 `use_card(` | §14 事件模拟入口禁止 | 0 | — |
| 触发 handler 读 live `g.turn/g.phase` | §5 事件必须带窗口 | **16** | M1 受害面 |
| 内核出现技能 code 字面量 | §6/§9 内核不得出现技能名 | **17** | B4 |
| `Seat.counters/marks` 未进视图 | §9.2.2 可见状态 | **3 确认**（裔/变/辎） | B3 |
| 全局计数器无作用域前缀 | §12/§22 | ~16（另有变量名误报） | M8 |
| 「不弃置/跳过」类出口 | §7 弃置契约 | 23（据守已改） | M9 |
| 主公技身份闸门 | 主公技裁定 | 1 真实缺失（制霸被动） | 功能项 |
| 裁定项 ↔ 用例映射 | — | 775 裁定 / 610 用例 / 81 个 code 仅 1 条 | 门禁欠账 |

### 1.2 运行时探针（143 将沙盒，seed 20261111）

- 开局 pending：**29 例**。28 例是合法的「阶段开始/开局」技能；**1 例缺陷**（清俭，已修）。
- 座位 0 合法动作出现非本将技能：**0 例**。
- 147 将机检（B0–B20，repeat=2）：0 失败 / 0 阻塞 / 守恒 ✓ / 真相源未写 ✓（只证「不炸」）。

### 1.3 为什么上一轮（S0–S6）没有发现/收口

不是「没时间」，而是**门禁覆盖面小于规范**，形成沉默风险：

| 缺口 | 当时的状态 | 后果 |
|---|---|---|
| 全武将机检 | `sgs-general-audit.mjs` 仍是 v1 形状（读 `raw.data.seats[].player_id` / `st.deck` / `st.pending`）→ **B0 直接「阻塞 3」** | 147 将的「可达/守恒/确定性」证据实际为零，S6 验收只能看单测 |
| 体力单出口 | 门禁只有 `gate_card_play_never_touches_hp_directly`（**只扫 kernel/card_play.rs**） | 技能层 26 处直写从未被检查 |
| 视图 | `check:sgs2-view-parity` 只比 **v1 字段集**；`view_contract` 只校验已有字段 | 「技能新加的标记」没有投影也没人发现 |
| 技能类型 | v2 `SkillDef` 去掉了 v1 的 `SkillType`；`sgs-catalog` 统一回落 `kind="normal"` | 走查单/矩阵的类型列失真；限定/锁定/主公技无法统一校验 |
| 内核去名 | `gate_kernel_has_no_skill_branch_b0` 只扫 4 个文件 + B0 七技 | 其余 289 技与 kernel 其他文件的名分支漏检 |
| 事件时机 | 自对弈只验终局/illegal/loop/stalled | 「开局不能有获得类 pending」「事件发生时刻」无人检查 |
| 裁定→用例 | `check:sgs2-registry` 只保证每个 code ≥1 条同名用例 | 775 条裁定没有一条被绑定到断言（奋激 A/B 口径就是这样漂移的） |

**结论**：上一轮不是「按规范做完但测试漏了」，而是**验收门禁的自检范围小于规范要求**。这属于流程欠账，需要靠 §6 的门禁补齐来关，而不是靠下一次人工走查。

---

## 2. 机制缺口细节

### M1 事件快照（已补，ADR-0039）

- `Event` 增加 `turn/phase/lifecycle`（`serde(default)`）；`Game::emit` 唯一入队出口打点；
  `Sub::accepts`/`Payload::Trigger`/handler 全部读快照。
- 已迁移：fenji（改 B 口径，不再需要窗口）、qingjian（读 `cause`）。
- 剩余 14 个读 live 状态的 handler 需按清单逐批迁移（`fenji,hujia,tuotian,haoshi,chunlao,shangshi,luoying,jianying,yanyu,qiangshi,longyin,benxi,zuoding,jijiang,yajiao`）。

### M2 开局生命周期（已补）

- `finish_setup` 全程 `Lifecycle::Setup`；`begin_game_at` 切回 `Turn`；路由层丢弃 Setup 事件。
- 取代了 `events.clear()` 补丁；发牌/组牌堆不再算「获得/移出游戏」。

### M13 牌移动来源（已补）

- `Game.action_actor`（`begin_action_as`，`resolve::submit`/`card_play::start_use` 打点）；
  `to_discard` 写 `by/from`，`gain` 写 `from`；`to_discard_by` 供技能/测试显式施动者。
- 奋激 B 口径直接用 `by/from` 判定；后续「被其他角色…」类技能可复用。

### M12 回合/阶段结束结算时序（待裁定）

**现象**：`PhaseEnd`（cause=旧阶段）在 turn 切换时先入队、后路由；路由时 `g.turn` 已是下一位、
`turn.*` 已清空。结束阶段技能（罪论/兴学/寝情）在**下一回合的准备阶段**才弹出 pending。

**后果（已确认）**：
- 罪论：`turn.damage_by`/`turn.discard_by` 恒为 0 → 条件①②恒假；`turn.zuilun_done` 写入新回合 → **隔回合失效**。
- 兴学/寝情：效果与「每回合限一次」计数落在下一回合。

**两个方案**（需要用户裁定，见 §4-Q2）：
- **A（低风险）**：维持「结算滞后」，handler 读事件快照。判定正确，但玩家在下一回合才看到结算。
- **B（正解，需改时序）**：`PhaseEnd` 在切 turn 前结算；若产生 pending，则把「切 turn」作为延迟项
  （`Payload::SwitchTurn`）排在触发之后，pending 清空后继续。风险：所有回合结束类技能的时序/展示都会变。

### M9 / M8 / M11（契约/审计类）

- M9：对齐单为每个「必须选择」的步骤声明 `mandatory`，`check:sgs-align` 校验对应 `escape_mandatory`；
  引擎在「只有出口、没有实质选项」时自动跳过（替代「不弃置（无牌可弃）」这类伪装）。
- M8：`enum Scope { Action, Phase, Turn, Round, Game }` + 注册/门禁；`dying.*`、`chunlao_*`、`jiexun_count` 等显式声明。
- M11：`Game::awaiting_input()`；审计/网关用「是否等待输入」判断开局就绪，而不是假设 `phase=Play`。

---

## 3. V2 迁移欠账明细（B1–B4）

### B1 技能类型元数据

- `SkillDef` 无 `kind`；`sgs-catalog.mjs` 统一 `kind="normal"`。
- 影响：走查单类型列、限定技「每局一次」、锁定技不可放弃、主公技身份闸门、AI 裁剪全部失去统一依据。
- 方案：`SkillDef.kind: SkillKind`（生成物来源：heroes.json `type_guess`），`check:sgs2-registry` 校验
  「locked ⇒ 无 can_active」「limited ⇒ 有 used 守卫」「lord ⇒ 有 lord 闸门」。成本 1 天（机械）。

### B2 技能层体力写入（26 处）

`b0(2) b1b(1) b2(4) b3(3) b5(2) b6(2) b7(2) b9(1) b10(2) b13(3) b14(2) b17(1) kongsheng(1) liangyin(1)`；
另 2 处为 `==` 比较（误报）。
方案：`gate_skills_never_touch_hp_directly` + 迁移到 `flow::heal/lose_hp/revive_to`（复原/置为补 `heal_to`）。

### B3 可见标记（3 处确认）

裔（counters["yi"]）/变（bian）/辎（zai）未进 `marks_count_view`；而「书/醇」走 `Zone::Mark` 已被 `marks_view` 覆盖。
方案：注册表声明 `visible_marks`，视图自动投影 + `view_contract` 校验「声明即投影」。

### B4 内核技能名（17 处）

数值钩子 8（mashu/benxi/qianju/paoxiao/fulin/jishe/wanglie + guhuo/jijiang 响应）、
濒死接力 1（xingshuai）、拼点前 1（hanzhan）、旗标 2（jigong/zhaofu）、文案 1（jueqing）。
方案：抽 `skills::*` 通用钩子（distance/sha_limit/draw_bonus/dying_entry/pindian_pre/relay/flag），
`gate_kernel_has_no_skill_branch` 扩到全 296 code。

---

## 4. 待裁定

### Q2（已裁定：B，已实现）

**问题**：像罪论/兴学/寝情这类「结束阶段」技能，允许哪种结算时序？

- **A（当前实现 + 快照）**：玩家在自己的结束阶段发动 → 立即结算。**但当前代码实际是下一回合准备阶段才弹窗**，
  且 `turn.*` 已清空。要做到 A，必须改 `begin_phase`：`PhaseEnd` 先结算，产生 pending 时把「切 turn」延迟。
- **B（维持现状）**：承认「结束阶段技能在下一回合开始结算」，只保证判定读事件快照；
  罪论的条件读 `turn.*` 仍会错（需要把这些计数改成跨回合留存或随事件快照携带）。

**裁定（2026-11-11）**：B。尝试实现（`switch_to` 延迟切 turn）后在自对弈 `seed=1 game 0` 出现 `stalled`（2000 步未终局，
pending 覆盖 11 种），为免拖垮 AI 门禁**已回滚为 A 运行**；`begin_game_at` 伪 `PhaseEnd` 的修复保留（开局 pending 29→25）。
B 的正确实现待改：在 `PhaseEnd` 事件里携带「本回合计数快照」（`turn.*`），技能读快照而不是延迟切 turn。

### Q-spec-kit（再评估）

见 `ADR-0038` 新增「2026-11-11 复核」段：**维持不引入 GitHub spec-kit**；
但把「规格即数据」推进到**可执行 schema**（每个技能：触发事件/窗口/主体窗口/必选步骤/可见标记/类型/
裁定→用例锚点），由生成器产出骨架、门禁校验。理由：本轮 4 类欠账都不是「规格写得不够多」，
而是「规格字段无法表达/门禁不校验」。

---

## 5. 是否需要 V3（判断标准不变）

满足任意两条才立项：
1. M1/M12 改造导致 >30% 既有用例重写且无法兼容；
2. 技能注册表需破坏性重排（DSL 化）且 296 个 handler 无法渐进迁移；
3. 回放/决策日志与旧快照无法共存；
4. B4 内核去名后仍 >20 处不可通用化的时序特例。

当前：1–3 未触发（新增字段均 `serde(default)`），4 约 4 处（蛊惑/激将/兴衰/酣战）→ **不触发**。
结论：**V2 收口即可**，把 B1–B4 作为「迁移欠账」逐项关门禁。

---

## 6. 门禁补齐（防复发）

| 门禁 | 作用 | 状态 |
|---|---|---|
| `check:sgs2-conventions`（新增） | C1 体力直写 / C2 内核技能名 / C3 标记投影 / C4 计数器作用域 / C5 可跳过出口 / C6 kind 一致性；白名单逐条理由 | **已接入** `check:sgs2` 与 `sgs:check:fast` |
| `check:sgs2-registry` 扩展 | 裁定↔用例锚点；`kind` 派生校验；`subject_window`/`visible_marks` 声明合法 | 部分（kind 已生成、裁定锚点待补） |
| `gate_skills_never_touch_hp_directly` | 技能层不得直写 `Seat.hp` |
| `gate_kernel_has_no_skill_branch` 扩展 | 扫描全部 kernel 文件 + 296 code |
| `check:sgs2-view-parity` 扩展 | 「技能声明的可见标记」必须出现在 `view` |
| `self-audit` 脚本 | 把本次的静态扫描 + 143 将开局探针纳入 `sgs:check:fast`（只读、无 Redis） |
| 事件时机用例 | `Event.turn/phase/lifecycle` 快照、Setup 不触发、`by/from` 来源 |

## 7. 本轮收口状态（2026-11-11）

| 项 | 状态 | 落点 |
|---|---|---|
| 奋激口径 A→B | 已修 | `b0::fenji_subject` + `Event.by/from`；`align/B0.md`、`rules-rulings §44` |
| M1 事件快照 | 已修 | `Event.turn/phase/lifecycle` + `Game::emit` |
| M2 开局生命周期 | 已修 | `Lifecycle::Setup` |
| M13 牌移动来源 | 已修 | `action_actor` + `to_discard_by` |
| M12 结束阶段时序（B） | **回滚**（A 运行） | 延迟切回合导致自对弈 stalled；保留 `switch_to` 结构待改「计数快照」；伪 PhaseEnd 修复保留 |
| B1 kind | 已修 | `SkillKind` + `lord_gate_ok` + catalog 类型列 |
| B2 体力单出口 | 已修（26→0） | `flow::heal/lose_hp/revive_*/clamp_hp/heal_raw` |
| B3 标记投影 | 已修 | `VISIBLE_COUNTER_MARKS` |
| B4 内核技能名 | 门禁已拦（17 白名单） | `conventions-allowlist.json`；抽钩子排 V2.2 |
| M9/M8 | 门禁已拦（23/10 白名单） | 空选项自动跳过、Scope 注册表待实现 |
| M10 裁定↔用例 | **未收口** | 775 裁定 / 610 用例无绑定；需 schema 字段 + 分批补锚点 |
| M14 牌堆重洗 | **已修** | `refill_draw_from_discard`（摸牌前重洗）；用例 `mechanisms::draw_refills_from_discard_when_deck_empty` |
| AI 自对弈（`check:sgs2-ai`） | **部分** | 已修：牌堆重洗（stalled 主因）、陈旧响应（`respond` 不在手牌视为未响应，seed2 已 0 illegal）；剩余 1 回环 + 1 长局 stalled 待查 |
| 剩余 14 个 handler 读 live turn/phase | **未收口** | 清单见 §2-M1，按批迁移（优先 PhaseEnd 类） |
| 制霸被动挑战 | **未实现**（功能缺失，非机制） | `align/B14.md §7` 有、§2 无 |
| `awaiting_input` API | 未补 | 探针已用 pending 分类替代 |

> 剩余项均已在门禁/白名单/台账显式登记，不再是「沉默实现」；但 M10、14 handler、制霸被动仍需在 V2 收口批次完成。
