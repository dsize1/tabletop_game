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

## 12. 牌结算（M1）的单一出口

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
