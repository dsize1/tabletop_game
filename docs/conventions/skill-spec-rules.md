# 技能实现规范（规格即数据 / 禁止硬编码）

> 状态：**生效**（2026-11-06 用户裁定）。适用于 `crates/engines/sanguosha*` 与后续所有桌游技能实现。
> 违反其中任一条 = 该技能**不得**标记 implemented（即使用例绿）。

## 0. 一句话

**牌的类型/目标/属性必须是数据字段，技能只读字段；禁止用牌名 `match`、禁止 per-card 接线。**

## 1. 牌的目标：**目标列表**，不是枚举

- 每张牌定义声明「可指定目标」；每次**使用/打出**记录 `targets: Vec<目标标识>`（座位号或 playerId，可为空）。
- 技能判定「是否指定过目标 / 目标集合」**只能读该列表**。
- 多目标锦囊的**每个目标都可单独引用**（如从谏：候选 = 列表排除自己 + 放弃）。

❌ 反例：`TargetKind { None, One, Many }`（丢了成员，无法回答「另一个目标是谁」）
✅ 正例：`targets: Vec<SeatId>` + `target_chosen` 事件

## 2. 「不能使用牌」= **手牌冻结** 机制

- 「本阶段不能再使用牌」一律实现为 `hand-freeze`：冻结指定手牌，**本回合内不能主动使用/打出**，回合结束自动解冻。
- 冻结不阻止被弃置/被获得/被移出游戏；冻结必须绑定「冻结者 + 手牌集合 + 到期时机」，禁止跨回合残留。

## 3. 牌的类别/子类：**卡牌属性**，禁止牌名枚举

- 需要「基本牌/锦囊/装备」「是否**伤害锦囊**」「是否【杀】/【闪】/【桃】」时，**读牌定义上的属性字段**。
- 伤害锦囊（目前）：**决斗、万箭齐发、南蛮入侵、火攻**（用户 2026-11-06 确认；新增牌必须显式标注）。

❌ 反例：`fn is_basic(name) { matches!(name, "杀"|"闪"|"桃"|"酒") }`、`c.name != "杀"`、濒死里 `name == "桃"`、无懈窗口 `c.name == "无懈可击"`
✅ 正例：`Card { kind: Basic|Trick|Equip, damage_trick: bool, tags: {Sha, Shan, Tao, Jiu, Wuxie} }`

## 4. 转换技：发动后切换 + 每面声明窗口

- 阳/阴是**两个技能面**，各自声明窗口（谁的回合 + 阶段）；**禁止由触发点隐式决定**。
- **发动后才切换**（锁定触发必须发动的技能除外）。

## 5. 其它既有硬规则（来自 ADR-0016 / 计划）

- 技能状态只查 `skills::active()`（禁止 flags/counters/marks 各自表达「拥有技能」）。
- legal 与 execute 调**同一个** `can_*`。
- Pending 必须带出口（`PendingBuilder` 类型级）。
- 技能不得就地压/弹帧（只返回 `Step`）。
- 任何「全局事件」必须带**生效窗口**。

## 6. 体力扣除：**造成伤害** vs **失去（流失）体力**（2026-11-06 用户裁定）

- 「造成 1 点伤害」与「失去 / 流失 1 点体力」是**两种不同场景**，事件、触发、文案都必须区分。
- 「受到伤害后」类技能（反馈 / 权计 / 刚烈 / 智愚 / 御策 / 放逐 / 遗计 …）**只**响应伤害事件；失去 / 流失体力**不得**触发。
- 失去体力走独立入口（`lose_hp` 一类），**禁止**复用 `apply_damage` 伪造伤害事件。
- 伤害事件必须带属性区分来源：是否「攻击性扣血」（造成伤害）、伤害类型（普通 / 火 / 雷）、是否由【酒】等加成；体力上限变化（减上限）是第三类动作，同样需要独立属性 / 事件，不得冒充伤害或失去体力。
- 需要区分「扣体力的来源」时读事件属性，禁止按牌名 / 技能名枚举。

## 7. 「弃置」的契约（2026-11-06 用户裁定）

- **弃置** = 由弃置动作离开**手牌 / 装备区**进入弃牌堆；**判定区的牌移出不算弃置**（即使进了弃牌堆）。
- 内核只有一个弃置出口（`Game::to_discard(id, cause)`）；技能只能订阅 `CardDiscarded`，禁止各自判断「算不算弃置」。
- 判定区进出必须发统一事件：`JudgeZoneEntered` / `JudgeZoneLeft`（离开判定区**不**发 `CardDiscarded`）。
- **弃置 ≠ 使用**：`CardDiscarded` 与 `CardUsed` 互斥；弃置不得触发「使用牌」类技能（奇制/往烈/奔袭距离…）。
- 弃置原因必须透传（`hand_cap` / `discard_phase` / 技能名），用于文案与审计。

## 8. 自检清单（提交前逐条回答）

1. 牌的类型/目标/属性是否有字段？我是否 `match` 了牌名？
2. 多目标场景能否拿到**每一个**目标？还是只有「多个」这个事实？
3. 「不能使用牌」是否走的冻结机制？冻结有到期吗？
4. 转换技两面窗口写了吗？切换时机是「发动后」吗？
5. 技能状态是否只走 `active()`？legal/execute 同源吗？pending 有出口吗？
6. 这个技能该响应「受到伤害」还是「失去体力」？我是否用 `apply_damage` 伪造了失去体力（或反之）？伤害事件是否带够属性（攻击性扣血 / 伤害类型 / 加成来源）？
7. 这个技能响应「弃置」还是「使用」？判定区的牌算不算（**不算**）？我是否自己判断了「算不算弃置」而不是订阅 `CardDiscarded`？

## 9. 门禁与复查

- `check:sgs-align`：对齐单必填字段、正/反例、机制引用、原文限定语复述。
- `cargo test -p sanguosha2`：内核「禁止出现技能名」、单一延迟队列、pending 出口、守恒/无帧遗留。
- `tests/cards.rs::gate_kernel_has_no_card_name_branch`：内核（`kernel/*`）不得出现牌名比较/构造
  （`== "杀"` / `cards::def("…")`）；牌名只允许出现在日志/展示文案里。
- `tests/cards.rs::gate_card_play_never_touches_hp_directly`：牌结算不得直接改 `hp`（必须走 `flow::*`）。
- 已知违反项：**已清零**（M1，2026-11-06）：`kernel/flow.rs` 的桃/酒/无懈判定已改读 `cards::def().tags`；
  `skills/b1b.rs` 已改用 `cards::is_basic` / `tags().sha`。

## 10. B8 跨批机制（2026-11-09）

- **拼点**（`pindian`）：`kernel/pindian.rs`（`FlowKind::Pindian`）——双方**暗选**（选择期不公开，同怃戎）→
  同时亮牌 → 比**有效点数** → 拼点牌进弃牌堆；平局 = 无赢家。技能只能通过通用钩子接入：
  `skills::pindian_options` / `pindian_take`（附加选项，如天辩「牌堆顶」）、`skills::pindian_rank`
  （有效点数，如红桃视为 K）。内核**不得**出现技能名。
- **目标数修正**（`target-extra`）：`skills::extra_targets`（红色基本牌/红色普通锦囊 +N 目标、无距离）；
  在 `card_play::planned_targets` 统一生效，单目标锦囊由 `card_play::advance_target` 逐目标循环结算；
  延时锦囊不参与（一张物理牌只能进一个判定区）。
- **牌堆承载使用**（`deck-carrier`）：`start_carrier` 允许承载牌来自 `Zone::Draw`（伏诛「使用牌堆中的【杀】」），
  结算后进弃牌堆；张数上限与洗牌由技能声明。
- **结算前【杀】介入窗口**：`skills::sha_intercept_skills`（守玺/谏征/专对）。**不要**用 `TargetChosen`
  事件做「指定目标后」——事件在牌结算**之后**才路由（谮毁遗留同因）。
  结果通道 `Game.sha_intercept`（Continue / Immune / Unresponsive / Replace）。
- **手牌上限**：`skills::hand_limit_delta`（才识 +1 / 忠鉴 -1 / 辟撰「书」数），内核 `Game::hand_limit` 统一叠加。
- 「随机」一律走 `Game::rand_below`（确定性、可序列化，回放一致）；`Game::shuffle_draw` 同源。

## 11. B9 跨批机制（2026-11-09）

- **被动转化技**（武圣/龙胆）：`skills::conversions` 返回「承载牌 → 生效牌名」列表；
  `legal_uses` 追加 `convert:<skill>:<承载牌>:<生效牌名>[:目标]`（主动使用，走 `use_as` 严格校验），
  响应窗口追加 `vrespond:<skill>:<承载牌>`（打出）。内核不含技能名。
- **主公技接力**（激将）：响应窗口 `jijiang` → 按座位顺序询问同势力角色代出【杀】；
  结果经 `Game.relay_played`（(牌, 代出者)）/ `Game.relay_none` 回传响应流程
  （`card_play` 的 `relay_wait` 循环：发起 → 挂起 → 重入消费）。
- **非锁定技失效**（义绝·黑 / 铁骑）：唯一闸门在 `skills::active`（`turn.nonlocked_lost:<seat>` +
  `LOCKED_SKILLS` 清单）；「不能使用或打出手牌」用 `flags["hand_sealed"]` + `Game::can_use_card`，
  回合切换自动清除。禁止各技能自行检查计数。
- **数值钩子**：`skills::distance_delta`（马术）、`sha_limit_override`（咆哮）、
  `sha_no_distance`（界武圣·方片【杀】）、`trick_no_distance`（奇才）、
  `prevent_equip_discard`（奇才防弃置）、`paoxiao_bonus`/`yijue_heart_bonus`（加伤，
  在 `damage_bonus_typed` 内叠加，白银狮子仍在最后压回 1）。

## 12. B10 跨批机制（2026-11-09）

- **额外回合**（博图）：`Game.extra_turn` + 驱动器在 End 阶段后插入一个 Prepare 起的额外回合
  （`extra_turn_active` 防递归）；上限按轮计数（座位 counters `botu:<round>`）。
- **跳过弃牌阶段**（克己）：`skills::skip_discard_phase` → 驱动器显式跳过并播报（不是把上限设为无限）。
- **赋值型手牌上限**（界英姿）：`skills::hand_limit_override`（覆盖而非叠加）。
- **多目标无效**（奋威）：任意「目标列表为多人」的牌在结算前询问；被选目标进 `Game.fenwei_void`，
  `sha_loop` / `aoe` 等逐目标循环跳过，`finish` 清空。
- **结算前手牌移出**（谦逊）：普通锦囊唯一目标时询问；移出的牌进 `Game.void_return`，回合切换归还。
- **手牌归零批次 X**（连营）：`Game.hand_loss_batch`（每动作一档，`begin_action` 重置）→ 手牌归零时
  写 `lianying_pending`，栈空后由 `advance` 询问「至多 X 名各摸 1」。
- **回复前改向**（救援）：`flow::heal` 前置 `skills::start_jiuyuan`（可选询问；拒绝时按正常回复落盘）。
- **装备流失钩子**（枭姬）：`move_card` 在离开 `Zone::Equip` 时调 `skills::on_equip_lost`。

## 13. B11 跨批机制（2026-11-09）

- **技能判定子流程**（洛神/刚烈等）：`card_play::start_quick_judge` → `FlowKind::QuickJudge`：
  亮判定牌（落在 `Zone::Frame(..)`）→ 鬼才改判 → 生效 → 天妒；结果写 `Game.judge_result`。
  收尾**必须按牌区判断**（只在 Frame/Draw/Processing 时进弃牌堆），否则会把天妒获得的牌再弃掉。
- **改判**（鬼才）：`Game.judge_rewrite` + `skills::start_judge_rewrite`；改判牌可来自手牌或装备区
  （装备区会触发失去装备钩子），原判定牌进弃牌堆。
- **判定生效后**（天妒）：`skills::start_tiandu`，取得**改判后生效**的那张。
- **【闪】的主公技接力**（护驾）：`relay_options(kind=Shan)` → `b11::start_relay`（魏势力代出【闪】）；
  与 B9 激将（【杀】）共用 `relay_played`/`relay_none` 通道。
- **倾国**：`vrespond` 允许「自己装备区」的黑色牌作为【闪】打出。
- **多步技能实现约定**（本批教训）：`Step::Next` 会**替换**触发帧的 `data`——事件触发的技能跨步后会丢
  `seat/cause/card` 上下文。**一律「答完即压新帧」**（`resolve::push_ask` 带上下文）或把状态存进 `turn.*` 计数。

## 14. 牌结算（M1）的单一出口

- 牌定义是**生成物**：`docs/games/sanguosha/data/cards.json` → `npm run gen:sgs-engine-data`
  → `crates/engines/sanguosha2/src/cards_generated.rs`（160 张 + 属性/效果表）。
  手写代码**只读** `cards::def()`，不得在运行时读 JSON，也不得手写牌名白名单。
- 使用一张牌有三个**真实结算**入口（M2 起技能全部走这里）：
  `Game::resolve_use`（普通使用）/ `Game::use_virtual`（纯虚拟牌）/ `Game::use_as`（转化牌 = 实体承载 + 生效牌名）。
  `Game::use_card` 仅保留为「事件模拟」兼容入口，**技能代码禁止调用**（门禁 `tests/m2_wiring.rs`）。
- 牌结算的**唯一**伤害/回血出口是 `flow::damage*` / `flow::lose_hp` / `flow::heal`；
  装备数值链顺序固定：技能加成 → 藤甲（火焰 +1）→ 白银狮子（最后，>1 改为 1）。
- 无懈链复用 `flow::start_wuxie`，结果经 `Game::wuxie_result` 回传父流程（不与牌结算各写一半）。
- 延时锦囊在判定阶段按 `Game::judge_zone_lifo` 结算（后置先判）；判定区进出只发
  `JudgeZoneEntered` / `JudgeZoneLeft`，不算弃置。

## 15. B12 跨批机制（2026-11-09）

- **多张响应**（`multi-response`，无双）：同一响应窗口可要求响应者**连续**打出 N 张同类响应牌。
  声明入口 `skills::response_need(g, responder, opponent, kind, effect)`；内核在 `card_play::sha_loop`
  （【杀】的【闪】）与 `card_play::duel`（每次出【杀】）用使用帧 `data["need"]/data["got"]` 计数，
  **技能不得删改响应窗口**。不足 N 张 = 未抵消，已打出的牌不退；「不能被响应」的牌整段跳过计数。
- **不能被无懈可击抵消**（`no-wuxie`，离间）：`UseOpts::no_wuxie` → 使用帧 `data["no_wuxie"]`，
  `card_play::play` 的无懈窗口据此跳过。与「不能被响应」（`unresponsive`，闪/杀）是**两个不同事实**，
  禁止复用 `unresponsive`（否则会把【决斗】的出【杀】响应一起关掉）。技能入口
  `Game::use_virtual_no_wuxie`。
- **濒死救援的转化【桃】**（急救）：`skills::conversions` 返回 `to: "桃"` 的项，濒死流程通过
  `skills::dying_conversions` 生成 `vrespond:<skill>:<承载牌>` 选项（legal 与 execute 同源），
  承载牌可来自手牌或装备区（装备区走统一移动 → 触发失去装备钩子）。
- **参与者自选弃牌**（除疠）：由**被弃置的角色本人**（pending actor = 该角色）从自己的手牌/装备区
  选一张，而不是由技能持有者代选；「区域里的一张牌」在弃置类效果中**不含判定区**
  （判定区离开不算弃置，见 `discard-contract`）。
- **距离双向修正**：`skills::distance_delta`（我 → 他人）与 `skills::distance_to_delta`（他人 → 我）；
  两者都在 `Game::distance_raw` 的单一入口累加，最终 `Game::distance` 保证下限 1。

## 16. B13 跨批机制（2026-11-09）

- **武将牌池**（`general-pool`，化身/新生）：生成物 `generals_generated.rs`（`gen:sgs-engine-data` 从
  `engine-catalog.json` 生成，带 `status`）。状态口径与中台 `sgs_generals.status` 一致：
  `live`（147 审计池）/ `offline`（被「界/将」同名去重排除的原版）/ `review`（技能未完成）。
  化身只取 `live()`；化身状态存 `Game.huashen`（不占牌区），切换时覆盖 `Seat.kingdom/male` 并保存原值。
- **扣置声明 + 同时质疑链**（`hidden-declaration`，蛊惑）：声明候选按牌定义属性筛选；质疑**先收集**
  全部「质疑/不质疑」再公开翻开；结果（真/假）与惩罚（质疑者自选弃牌或失去体力）逐层结算。
  响应窗口的**延后结果**走 `Game.guhuo_result`（`Some(Some(牌))` = 已响应 / `Some(None)` = 假牌作废），
  由 `apply_response` 返回 `None` 触发重入——这是既有「激将接力」模式的最小扩展，不是新子系统。
  声明覆盖**任意基本牌与普通锦囊**（含借刀/铁索：多目标走两段选择）；入口共四处：出牌阶段（主动技）、
  【闪】/【杀】响应（`response_options`）、**无懈窗口**
  （`wuxie_opts`/`wuxie_use` + `Game.skill_deferred`），以及**濒死救援**
  （`skills::dying_extra_options`/`dying_extra_use`，与急救的 `dying_conversions` 同构）。
- **群体锦囊少选目标**（`aoe-target-trim`，乱击）：`skills::aoe_trim_target` 在 `planned_targets` 的
  AllAlive 分支裁剪；裁剪必须发生在目标确定阶段（无懈/响应之前），`finish` 清理标记。
- **【杀】结算前介入**（`sha-intercept`，烈弓/享乐）：扩展 B8 `sha_intercept_skills` 的提供者清单；
  技能只能返回 `Continue / Immune / Unresponsive / Replace`，不得就地改目标或吞掉响应。
- **判定效果反转**（`judge-reversal`，思蜀）：`skills::sishu_reversed` 只改判定分支（红桃↔非红桃），
  同一时间只允许一个目标；再次指定同一角色 = 恢复常规。
- **范围覆盖**（`range-override`，烈弓）：`skills::range_override_reach` 接入 `Game::can_reach`，
  逐目标判定（不改写全局 `attack_range`）。
- **改判是一轮询问**：判定牌亮出后，从判定者左手侧起逐个询问有改判能力的角色（判定者最后），
  每个持有者只问一次；**延时锦囊判定也走这一轮**（此前 v2 只覆盖技能判定）。

## 17. B14 跨批机制（2026-11-09）

- **虚拟装备**（`virtual-equip`，八阵）：技能提供 `skills::virtual_armor` → `Game::armor` 单一入口；
  虚拟装备不占槽、不产生失去装备事件；有真实防具时失效。
- **装备移动**（`equip-move`，设变）：`Game::move_equip`（目标槽位必须为空、不能回原持有者），
  移动即「失去装备」，必须触发失去装备钩子。
- **统一翻面钩子**（`skills::on_flip`）：任何方向翻面都调用（解围仍只在翻至正面时生效）。
- **花色读取单入口**（`skills::effective_suit`）：红颜（黑桃→红桃）；八卦判定与延时锦囊判定都走它。
- **「不计入手牌数」≠「上限 +N」**（藏拙）：`skills::hand_limit_exempt` 只影响弃牌阶段的超限判定。
- **受害者侧「受到伤害时」拦截**（`skills::damage_before_victim`，天香）：在内核防具判定之后、
  伤害防止之前压帧接管；技能放弃时必须自行补结算原伤害。
- **濒死离场结算**（`skills::on_dying_left`）：脱离濒死后的一次性效果（奇谋补摸）在此消费。
- **限定技**：`flags.<code>_used` + 选项处统一检查；可在濒死窗口（涅槃）或结束阶段（武烈）触发。
- **无懈链的转化与不可响应**：无懈窗口收集 `skills::conversions` 中 `to` 为无懈的项（`vrespond:`）；
  `skills::wuxie_uncounterable` 为真时打出后直接 `wuxie_finish`，不再询问后续无懈。
- **火攻的颜色模式**（界火计）：`skills::fire_attack_color_mode` 让内核改为随机展示 + 同颜色弃牌。
- **主动技放弃防抖**：本阶段放弃后写 `phase.cancel_<code>`，同阶段不再提示（防 AI/端侧死循环）。

## 18. B15 跨批机制（2026-11-09）

| 机制 | 入口 | 说明 |
|---|---|---|
| 失去牌事件 | `EventKind::CardLost` | 牌离开**手牌/装备区**时由 `Game::move_card` 上报，`cause = to:<区域>`；判定区移出不算失去（仍只发 `JudgeZoneLeft`）。屯田等「失去牌后」技能订阅它。 |
| 拼点前窗口 | `kernel::pindian` 的 `pre` 阶段 + `skills::pindian_pre_owners/pre_apply` | 逐个询问拥有「拼点前」技能的参与者（酣战）；发动后该对手的拼点牌改随机（`pindian_random_forced`，读后清除）。 |
| 拼点后窗口 | `skills::pindian_after` | 拼点结算后给持有者压入询问；结果通过 `Ask.data` 传递（不抢 `Game.pindian_result`，发起技能仍可 `take`）。 |
| 多张响应相加 | `skills::response_need` = 无双 + 肉林 | 两个来源**相加**而非取 max（无双+肉林=3）。 |
| 【酒】叠加与不限次数 | `card_play::apply_jiu` / `jiu_take_bonus` / `skills::jiu_unlimited` | `ju_take_bonus` 返回并清零全部层数；界酒池放行 `Effect::Jiu` 的次数校验。 |
| 牌结算结束钩子 | `skills::on_card_finished` | `card_play::aoe` 收尾调用（巨象：其他角色的【南蛮入侵】结算结束后获得之，被无懈也获得）。 |
| 阶段前询问 | `kernel::resolve::begin_phase` → `skills::before_phase` | 返回 true 时暂停阶段切换并呈现询问；驱动器下次重试同一目标阶段（`turn.qiaobian_asked:<phase>` 防抖）。 |
| 摸牌阶段结束 | `turn::tick` → `skills::after_draw_phase` | 好施：摸牌后手牌 >5 交一半。 |
| 目标拦截带承载牌 | `SkillDef.target_blocker(g,target,source,牌名,carrier)` | 帷幕按**实体牌花色**拦黑色锦囊；AOE 目标枚举同样过滤。 |
| 濒死锁定 | `skills::entering_dying/leaving_dying` + `nonlocked_lost` | 完杀：濒死期间除贾诩与濒死者外其他角色非锁定技失效（`skills::active` 统一判定）。 |
| 额外回合即新回合 | `turn::tick` | 额外回合清 `turn.*` / `used_this_turn` / `uncounted`，递增 `turn_seq` 并发 `TurnStart`。 |

## 19. B16 跨批机制（2026-11-09）

| 机制 | 入口 | 说明 |
|---|---|---|
| 红牌进弃牌堆计数 | `Game::to_discard` → `turn.red_discarded` | 任何进入弃牌堆的红色实体牌都累计（再起的「每有一张红色牌」）。 |
| 每回合受伤次数 | `flow::damage_now_impl` → `turn.damage_taken:<victim>` | 狞恶「每回合第二次受到伤害」。 |
| 单次弃置批次 | `Game::begin_action` 清空 / `to_discard` 追加 → `Game.discard_batch` | 界固政「单次弃置动作 ≥2 张」。 |
| 跳过摸牌钩子 | `Game::draw_phase` 跳过分支 → `skills::on_skip_draw` | 截辎：任意角色跳过摸牌阶段后询问。 |
| 额外摸牌阶段 | `skills::after_draw_phase`（摸牌阶段结束） | 有「辎」者移除并再摸一次（2 + 摸牌加成）。 |
| 死亡钩子 | `flow::die` → `skills::on_death(victim, source)` | 断肠（杀死者 `skills_lost`）/ 节命死亡触发；在死亡清理（延迟项）之前询问。 |
| 永久失去技能 | `skills::active` 读 `Seat.flags["skills_lost"]` | 断肠：含锁定技/主公技。 |
| AOE 伤害来源替换 | `skills::aoe_damage_source`（`card_play::aoe`） | 祸首：其他角色使用的【南蛮入侵】来源改为祸首持有者。 |
| 兵粮距离 | `skills::supply_shortage_no_distance` | 【兵粮寸断】距离 1；奇才/慎断/界断粮（未造成伤害）例外。 |
| 无视防具伤害 | `flow::damage_now_no_armor` | 驱虎：伤害不被防具减免/防止。 |
| 回合末归还 | `Game.void_return` + `skills::return_void_cards` | 破军（`due = 当前 turn_seq`，回合切换即归还）；谦逊同口径。 |

## 20. B17 跨批机制（2026-11-10）

| 机制 | 入口 | 说明 |
|---|---|---|
| 单动作失去批次 | `Game.move_card` 追加 / `begin_action` 清空 → `lost_batch` | 旋风「一次性失去 ≥2 张」（含被获得/移出/置于武将牌上）。 |
| 单动作失去装备 | `Game.move_card` → `equip_lost_batch` | 旋风「失去装备区里的牌后」（不与枭姬同步压帧，改由 `CardLost` 事件处理）。 |
| 单动作获得批次 | `Game.gain` 追加 → `gain_batch` | 恩怨「单次获得一名其他角色 ≥2 张牌」。 |
| 单动作技能去重 | `Game.action_seen`（`begin_action` 清空） | 一个动作内多个同类事件只触发一次（旋风/纵玄/恩怨）。 |
| 此牌造成伤害 | `flow::damage_now_impl` → `turn.damage_card:<card>` | 疠火「此牌结算后，若其造成伤害」（结算完成时同步读取，不依赖 `DamageAfter` 事件排队）。 |
| 少指定目标（-1） | `legal_uses` 产出 `trim:<牌>:<目标>` → `lib.rs` 设 `turn.aoe_trim` → `planned_targets` 裁剪 | 巧说：群体锦囊（南蛮/万箭/桃园/五谷）少指定一个目标。 |
| 额外目标的距离语义 | `skills::extra_targets_no_distance` | 福绵②/天义的多目标含「无距离」；巧说/陷阵/疠火的多目标**不**自动获得无距离。 |
| 额外目标不覆盖延时锦囊 | `b17::qiaoshuo_extra_targets` 对 `def.tags.delayed` 直接返回 0 | 用户 2026-11-10 裁定：废除延时锦囊的额外目标支持（不引入虚拟判定牌）；延时锦囊不适用 ±1、也不消耗巧说的「下一张牌」效果。 |
| 回合内计数标记（巧） | `skills::qiao`（`turn.qiao:<seat>` 计数）/ `skills::on_card_use`（`start_use` 调用）/ `view::marks_count_view` | 用户 2026-11-10 裁定：巧说每赢一次 +1 枚「巧」，每出一张符合条件手牌（杀/决斗/拆/顺/火攻/群体锦囊；借刀/铁索等固定目标结构不参与）消耗 1 枚，拼点失败或回合结束清空；视图投影为 `marks_count.qiao`。 |
| 目标级距离/次数/禁用 | `skills::target_no_distance`（`Game::distance`）/ `sha_limit_exempt` / `sha_blocked_for`（`planned_targets`） | 陷阵：赢 → 对目标无距离、【杀】不受次数；没赢 → 只对该角色禁用【杀】。 |
| 次数修正双向 | `skills::sha_limit_bonus` / `sha_limit_penalty`（`Game::sha_limit`） | 将驰：②+1；①-1（与其它加成同层，可为 0）。 |
| 无效=不能成为目标 | `target_blocker` + `skills::trick_blocked` | 智迟；巧说没赢的锦囊封锁（`plan_use` 拦截）。 |
| 禁酒 | `skills::jiu_blocked` / `effective_rank` | 高顺【酒】→点数 K 的【杀】；其回合内其他角色不能用【酒】（出牌/濒死/技能视为使用统一拦截）。 |
| 纵适 code | `zongshi_jie` | 与界刘表【宗室】`zongshi` 解耦（ADR-0024 同音撞车另起 code）；v1 同步改名以保持目录一致。 |

## 21. B18 跨批机制（2026-11-10）

| 机制 | 入口 | 说明 |
|---|---|---|
| 伤害转失去体力 | `flow::damage_impl` → `skills::damage_to_hp_loss` / `flow::lose_hp_by` | 绝情：整次数值一次失去、保留来源用于死亡归属；不产生伤害事件。 |
| 濒死时（第三方）技能 | `flow::start_dying` → `skills::on_dying_enter` | 补益/仁心/伏枥：在求桃之前压入询问；伏枥自救优先，其次补益/仁心。 |
| 濒死救援入口 | `flow::revive_to` / `flow::revive_add` | 仁心/伏枥回复至 X；补益回复 1（濒死时体力 ≤0 也能生效）。 |
| 锦囊目标拦截 | `card_play::step_card` → `skills::trick_intercept_owner` + `Game.trick_intercept` | 贞烈：逐目标询问，令此牌对该目标无效（目标列表裁剪）。 |
| 回合结束钩子 | `resolve::begin_phase` 换人前 → `skills::on_turn_end` | 窃听：在清空 `turn.` 计数之前读取「未造成伤害/未对其他角色用牌」。 |
| 额外出牌阶段 | `turn::tick` Prepare 分支 → `skills::start_extra_play_phase`；`end_play` → `skills::on_end_play` | 当先：准备阶段后、判定阶段前插入 Play；结束回判定阶段。 |
| 本回合使用过伤害牌 | `card_play::start_use` → `skills::on_card_use`（b18） | 自守：`turn.damage_card_used:<seat>`。 |
| 单张牌无距离 | `Game.no_distance_cards` / `skills::card_no_distance` | 当先获得的【杀】、酒诗的落英牌。 |
| 落英标记 | `Game.luoying_cards`（离开手牌即清除）+ 视图 `card.marks` | 酒诗：背面朝上时无距离且不能被响应。 |
| 整局伤害累计 | `flow::damage_now_impl` → `total.damage_dealt:<seat>` | 伏枥：X > 本局造成伤害数才翻面。 |

## 22. B19 跨批机制（2026-11-10）

| 机制 | 入口 | 说明 |
|---|---|---|
| 护甲（独立防护值） | `Seat.armor` + `Game::add_armor` + `EventKind::ArmorChanged` + `flow::damage_now_impl` 前置扣甲 | 矢北：先扣护甲、溢出扣体力；伤害事件仍成立（amount = 原伤害值）；视图 `marks_count.hu`（显示「护N」）。 |
| 结算中追加目标 | `Game.extra_targets_mid` → `sha_loop` / 锦囊拦截段消费 | 求援②：追加到目标列表末尾，不重置结算进度（区别于 `ShaIntercept::Replace`）。 |
| 同色响应限制 | `Game.response_color_lock`（`finish` 清空）→ `response_options` + `apply_response` | 父魂：本张【杀】的【闪】响应只能用同色手牌（legal 与 execute 同源）。 |
| 两张手牌当【杀】打出 | `skills::two_card_sha` / `two_card_sha_pairs` → `pair:c1,c2` 响应选项 | 父魂的打出通道（内核不出现技能名）。 |
| 手牌颜色封锁 | `Game::can_use_card` → `skills::hand_color_blocked` | 潜袭：本回合不能用/打出该颜色手牌（含之后摸到的）。 |
| 「不能对其他角色使用牌」 | `plan_use` → `skills::others_use_blocked` | 惴恐（赢）：按目标列表判定；对自己使用不受限；**技能视为使用（`opts.strict == false`）不受封锁**（用户 2026-11-10 裁定）。 |
| 攻击范围无限 | `Game::can_reach` → `skills::infinite_range` | 弓骑：其他角色恒可达（不改 `attack_range` 数值）。 |
| 花色【杀】不限次数 | `planned_targets` → `skills::sha_unlimited_card` | 弓骑：与弃置牌同花色的【杀】跳过次数校验（legal 同源）。 |
| 本轮级计数 | `g.round += 1` 时 `clear_prefix("round.")` | 矫诏：`round.used:<牌名>`（本轮所有角色的使用+打出都记）。 |
| 回合外主动技 | `skills::legal_actions_out_of_turn`（`b19::OUT_OF_TURN_SKILLS`） | 矫诏 修改 1 起可回合外发动；驱动器目前仍只询问当前回合角色（登记边界）。 |
| 花色覆盖（视为） | `skills::effective_suit` → `b19::jy_suit_override` | 渐营②：花色视为本阶段上一张牌的花色。 |
| 技能显示名动态化 | `skills::b19::display_name` | 矫诏 → 矫诏plus → 矫诏plusplus（UI/legal 标签/播报共用）。 |
