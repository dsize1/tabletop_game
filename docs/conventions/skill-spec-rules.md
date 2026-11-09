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
