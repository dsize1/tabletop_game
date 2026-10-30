# 三国杀技能注册表与触发钩子（P2.4）

> 状态：**P4.1 已完成（2026-10-30 收口）**。标准 25 将 / 标准包全部技能（49 技能 + 5 件装备效果）已实现：鬼才改判窗口、转化（武圣/龙胆/倾国/急救/国色）、锁定/触发（咆哮/马术/英姿/裸衣/天妒/谦逊/连营/枭姬/空城/无双）、主动与伤害后（苦肉/青囊/结姻/反馈/遗计/刚烈）、判定/选择（铁骑/洛神/反间）与流离。用例 **164** 条；门禁 `check:sgs-skill-cases`（25/25 每人 ≥3 条）。
> 关联：`generals.md`、`state-machine.md` §2.2（pending）、`legal-actions.md` §4.7、`development-plan.md` P2.4、ADR-0016（技能事件总线）。

## 1. 设计

- 技能按**武将**绑定（`sgs_generals.code`），引擎侧用固定顺序派发：
  **技能声明优先级 → 当前回合玩家左手侧座位顺序 → 注册顺序**（不依赖 HashMap 迭代）。
- 需要玩家决策的技能压入**结算栈帧**，与锦囊/无懈共用 pending：
  - `choice`（是/否二选一，如奸雄）
  - `choose_many`（多选 + 确认/取消，如制衡）
- 技能动作 id：`skill:<code>`（主动技）；多选内部用 `choose:<card_id>` / `confirm` / `cancel`。
- 技能只读引擎状态，不改规则；新技能必须走代码 + 用例（工作台不得改规则，ADR-0016）。

## 2. 已实现（首批 8 将）

| 武将 | 技能 | code | 类型 | 触发/效果 |
|---|---|---|---|---|
| 曹操 `sgs_hero_15` | 奸雄 | `jianxiong` | 普通 | `damage_after`：受伤且存活时，可选择获得造成伤害的牌（`choice` 帧） |
| 孙权 `sgs_hero_8` | 制衡 | `zhiheng` | 普通 | 出牌阶段主动（每阶段 1 次）：弃任意张手牌、摸等量（`choose_many` 帧） |
| 孙权 `sgs_hero_8` | 救援 | `jiuyuan` | 主公技 | 其他**吴**势力角色对拥有此技的主公使用【桃】时，回复 +1 |
| 貂蝉 `sgs_hero_24` | 闭月 | `biyue` | 普通 | `phase_start(end)`：摸 1 |
| 黄月英 `sgs_hero_7` | 集智 | `jizhi` | 普通 | 使用非延时锦囊（含无懈）后摸 1 |
| 黄月英 `sgs_hero_7` | 奇才 | `qicai` | 锁定 | 使用锦囊无距离限制（顺手等） |
| 吕蒙 `sgs_hero_10` | 克己 | `keji` | 普通 | 出牌阶段未使用【杀】→ 跳过弃牌阶段（当前自动跳过） |
| 诸葛亮 `sgs_hero_4` | 观星 | `guanxing` | 普通 | `phase_start(prepare)`：取 `min(存活角色数,5)` 张进特殊区 `guanxing`（2×5 槽），`arrange:<csv>` 一次性回写（P2.5） |
| 诸葛亮 `sgs_hero_4` | 空城 | `kongcheng` | 锁定 | 无手牌时不能成为【杀】/【决斗】目标（含借刀杀人的虚拟【杀】、离间视为决斗）；有手牌立即恢复可指定（P4.1） |
| 司马懿 `sgs_hero_16` | 鬼才 | `guicai` | 普通 | `judge_before`：判定牌亮出后自动询问（`judge_rewrite`），可打出任意手牌替换判定牌或 `pass`；顺序从判定者左手侧起、判定者最后（P4.1） |
| 关羽 `sgs_hero_2` | 武圣 | `wusheng` | 普通 | 红色手牌当【杀】使用/打出（`skill:wusheng:<card>` / 响应 `respond:<card>`；含南蛮/决斗/借刀响应）（P4.1） |
| 赵云 `sgs_hero_5` | 龙胆 | `longdan` | 普通 | 【杀】↔【闪】互转：出牌阶段【闪】当【杀】（`skill:longdan:<card>`），响应时【杀】当【闪】（P4.1） |
| 甄姬 `sgs_hero_21` | 倾国 | `qingguo` | 普通 | 黑色手牌当【闪】打出（仅响应；`respond:<card>`）（P4.1） |
| 华佗 `sgs_hero_22` | 急救 | `jijiu` | 普通 | **回合外**红色手牌当【桃】使用（濒死 `dying` pending 的 `respond:<card>`）；自己回合内不可用（P4.1） |
| 华佗 `sgs_hero_22` | 青囊 | `qingnang` | 普通 | 出牌阶段限一次：弃 1 张手牌，令一名已受伤角色回复 1（`skill:qingnang` → `choose_many(1)` → `target`）（P4.1-C2） |
| 黄盖 `sgs_hero_11` | 苦肉 | `kurou` | 普通 | 出牌阶段可多次：失去 1 点体力，摸 2 张（`skill:kurou`；1 体力发动会进入濒死）（P4.1-C2） |
| 孙尚香 `sgs_hero_25` | 结姻 | `jieyin` | 普通 | 出牌阶段限一次：弃 2 张手牌，与一名已受伤男性各回复 1（`skill:jieyin` → `choose_many(2)` → `target`）（P4.1-C2） |
| 司马懿 `sgs_hero_16` | 反馈 | `fankui` | 普通 | 受到伤害后获得伤害来源一张牌（`pick_card`：手牌随机/装备；不含判定区）（P4.1-C2） |
| 郭嘉 `sgs_hero_20` | 遗计 | `yiji` | 普通 | 受到伤害后摸 2 张，并逐张分配给任意角色（含自己；`target` 逐张选择）（P4.1-C2） |
| 吕布 `sgs_hero_23` | 无双 | `wushuang` | 锁定 | 使用【杀】目标需连续两张【闪】；使用【决斗】时对方每次需连续两张【杀】（`RespondFrame.need/played`）（P4.1-C2-2） |
| 马超 `sgs_hero_6` | 铁骑 | `tieqi` | 普通 | 使用【杀】指定目标后判定，红色 → 目标不能使用【闪】（`RespondFrame.no_shan`，自动放弃响应）（P4.1-C2-2，**自动判定**） |
| 夏侯惇 `sgs_hero_17` | 刚烈 | `ganglie` | 普通 | 受到伤害后判定，非红桃 → 伤害来源二选一：弃 2 张手牌（`choose_many`）或受 1 点伤害（P4.1-C2-2，**自动判定**） |
| 甄姬 `sgs_hero_21` | 洛神 | `luoshen` | 普通 | 准备阶段可反复判定：黑色获得该牌并继续，红色结束（`choice` 逐次询问）（P4.1-C2-2） |
| 周瑜 `sgs_hero_12` | 反间 | `fanjian` | 普通 | 每阶段一次：令一名其他角色选花色，获得你一张随机手牌；花色不同则受 1 点伤害（目标选花色用 `choice`）（P4.1-C2-2） |
| 大乔 `sgs_hero_13` | 流离 | `liuli` | 普通 | 成为【杀】目标时，可弃 1 张手牌（`choice:choose:<card>`），将【杀】转移给自己攻击范围内、非使用者的其他角色；转移后按新目标结算（铁骑封锁不继承）（P4.1） |
| 大乔 `sgs_hero_13` | 国色 | `guose` | 普通 | 方块手牌当【乐不思蜀】使用（`skill:guose:<card>`）；用 `virtual_defs` 记录「牌实例 → 乐不思蜀」，判定/同名检查按生效定义结算（P4.1 收尾） |
| 张飞 `sgs_hero_3` | 咆哮 | `paoxiao` | 锁定 | 出牌阶段使用【杀】无次数限制（`turn::sha_limit`，与诸葛连弩同口径）（P4.1-C1） |
| 马超 `sgs_hero_6` | 马术 | `mashu` | 锁定 | 计算与其他角色的距离 -1（攻击范围与顺手距离均生效）（P4.1-C1） |
| 周瑜 `sgs_hero_12` | 英姿 | `yingzi` | 锁定 | 摸牌阶段多摸 1 张（P4.1-C1） |
| 许褚 `sgs_hero_19` | 裸衣 | `luoyi` | 普通 | 摸牌阶段**手动询问**（`choice`，`skill=luoyi`）：`choose:luoyi` = 少摸 1 张且本回合【杀】/【决斗】伤害 +1；`pass` = 正常摸 2 张（P4.1-C1） |
| 郭嘉 `sgs_hero_20` | 天妒 | `tiandu` | 普通 | 判定牌生效后获得之（**自动获得**，不再进弃牌堆）（P4.1-C1） |
| 陆逊 `sgs_hero_14` | 谦逊 | `qianxun` | 锁定 | 不能成为【顺手牵羊】/【过河拆桥】的目标（含奇袭转化）（P4.1-C1） |
| 陆逊 `sgs_hero_14` | 连营 | `lianying` | 普通 | 失去最后一张手牌后摸 1 张（**自动发动**；覆盖使用/打出/弃置/被拿）（P4.1-C1） |
| 孙尚香 `sgs_hero_25` | 枭姬 | `xiaoji` | 普通 | 失去装备区的牌后摸 2 张（**自动发动**；换装/被拆/麒麟弓弃坐骑）（P4.1-C1） |

## 3. 触发钩子（引擎接入点）

| 钩子 | 位置 | 已用技能 |
|---|---|---|
| `phase_start(prepare)` | `turn::phase_step` Prepare 阶段 | 观星（特殊区 `guanxing` + `arrange`） |
| `phase_start(end)` | `turn::phase_step` End 阶段 | 闭月 |
| `card_used(非延时锦囊)` | `turn::play_card` / `resolve::act_wuxie` | 集智 |
| `damage_after` | `resolve::apply_damage`（仅存活时） | 奸雄 |
| `hand_limit` | `turn::phase_step` Discard | 克己 |
| `trick_distance` | `turn::trick_distance_ok` | 奇才 |
| `tao_heal` | `resolve::act_dying` | 救援 |
| 主动技 | `turn::legal_actions` / `turn::execute` | 制衡、仁德、离间、奇袭 |
| 摸牌阶段替代 | `turn::phase_step` Draw | 突袭 |
| 响应接力（主公技） | `resolve::step_respond` → `Requisition` | 激将、护驾 |
| `judge_before`（改判） | `resolve::step_judge`（`Reveal → Rewrite → WaitRewrite → Effect`） | 鬼才 |
| 转化（主动） | `turn::legal_actions` / `turn::execute`（`skill:<转化技能>:<card>`） | 武圣、龙胆 |
| 转化（响应） | `resolve::respond_options` / `finish_respond_play` / `act_dying` | 武圣、龙胆、倾国、急救 |
| 距离计算 | `state::mashu_bonus`（攻击范围 + 顺手距离） | 马术 |
| 出杀次数上限 | `turn::sha_limit` | 咆哮 |
| 摸牌阶段数量 | `turn::phase_step` Draw | 英姿、裸衣 |
| 伤害加成 | `resolve::respond_give_up`（杀/决斗） | 裸衣 |
| 判定牌归属 | `skills::tiandu_gain`（`apply_judge_effect`） | 天妒 |
| 目标过滤（锁定） | `turn::legal_actions` / `resolve::step_trick` | 谦逊、空城 |
| 失去手牌 | `turn::take_card` / `act_pick` / 突袭 | 连营 |
| 失去装备 | 换装 / `act_pick` 装备 / 麒麟弓 | 枭姬 |
| 主动技（回复/自伤） | `turn::legal_actions` / `turn::execute` | 苦肉、青囊、结姻、反间 |
| 杀指定目标后 | `resolve::act_target`（判定 / 响应牌数） | 铁骑、无双 |
| 响应牌数 | `RespondFrame.need/played` + `finish_respond_play` | 无双 |
| 准备阶段判定 | `turn::phase_step` Prepare + `Luoshen` | 洛神 |
| 杀目标重定向 | `resolve::act_target` → `Liuli` 帧 | 流离 |
| 被闪后追加 | `finish_respond_play`（`is_sha`）→ `AfterDodge` 帧 | 青龙偃月刀、贯石斧 |
| damage_before | `respond_give_up`（`is_sha`）→ `Hanbing` 帧 | 寒冰剑 |
| judge_before（通用） | `QuickJudge` 帧 → `judge_rewrite` → 效果派发 | 铁骑、刚烈（洛神/八卦待迁移） |
| 杀指定目标后（异性） | `push_sha_target_response` → `Cixiong` 帧 | 雌雄双股剑 |
| 虚拟牌定义 | `SgsState.virtual_defs` + `effective_def_code`（判定区/判定结算） | 国色 |
| 伤害后判定 | `resolve::apply_damage` + `Ganglie` | 刚烈 |
| 伤害后获得/分配 | `resolve::apply_damage`（`PickCard` 反馈 / `Yiji` 遗计） | 反馈、遗计 |
| 目标过滤（锁定） | `turn::sha_targets` / `resolve::step_trick`（决斗）/ 借刀二阶段 / `resolve_more::male_targets` | 空城 |

## 4. 未实现

> **标准 25 将已全部实现（P4.1 收口，2026-10-30）**。后续包（风林火山 → 一将成名 → 界限突破）随 P4.4 扩展；
> 军争牌结算（P4.3）与标准包剩余装备效果（P4.2）不属于武将技能范围。

## 5. 装备可选效果（标准包）

> **P4.5 阶段一（2026-10-30）**：装备效果已登记为 `EQUIPMENT_SKILLS`（17 件：9 武器 + 2 防具 + 6 坐骑），带 `code/name/card/trigger`；引擎统一入口 `skills::has_equipment_skill`（判定是否生效）与 `skills::triggered(Trigger)`（按窗口收集持有者，座位升序）。`Trigger` 取值：`Passive / PlayPhase / TargetChosen / DamageBefore / AfterDodge / JudgeBefore`。**阶段二（已完成 2026-10-30）**：`player_area.equipment_skills[]` 已下发（`{slot, code, name, card, trigger}`，仅本人视图）；AfterDodge / DamageBefore 窗口改为 `triggered(Trigger)` 派发。**阶段三（已完成 2026-10-30）**：AfterDodge 窗口落地**效果函数注册表** `AFTER_DODGE_HANDLERS`（青龙偃月刀 / 贯石斧：`options` + `act` 函数指针），窗口只按 `triggered(Trigger)` 收集处理器，不再写装备 if 分支；同批迁移：**DamageBefore**（寒冰剑 `DAMAGE_BEFORE_HANDLERS`，可用性+效果函数）与 **TargetChosen**（雌雄双股剑 `TARGET_CHOSEN_HANDLERS`，异性目标可用性）；**阶段四（收尾，已完成 2026-10-30）**：**PlayPhase** 落地 `PLAY_PHASE_HANDLERS`（丈八蛇矛：出牌阶段选项生成+执行，出牌阶段只按 `triggered(PlayPhase)` 收集）；方天画戟（多目标修正）继续由 `triggered(TargetChosen)` + `ShaFrame.fangtian` 驱动。**Web 武器高亮（已完成 2026-10-30）**：窗口类 pending 的 `data.option_sources`（选项 → 装备 code）+ `player_area.equipment_skills` 驱动个人区武器卡高亮（`data-equipment-skill` / `.sgs-equip.active`）；出牌阶段丈八选项按 `skill:zhangba:` action 高亮。P4.5 全部完成。

| 装备 | 状态 | 实现要点 |
|---|---|---|
| 诸葛连弩 | ✅ | 出牌阶段【杀】无次数限制（`turn::legal_actions` / `play_card`） |
| 青釭剑 | ✅ | 【杀】无视目标防具（仁王盾无效、八卦阵不可用）；`RespondFrame.ignore_armor` |
| 八卦阵 | ✅ | 需要【闪】时多一个 `skill:bagua` 选项：判定红色视为出【闪】，黑色继续正常响应；**每次响应只能判一次**（`bagua_used`） |
| 仁王盾 | ✅ | 黑色【杀】无效（青釭剑可无视） |
| 麒麟弓 | ✅ | 【杀】造成伤害后，攻击方可弃置目标一张坐骑（`choice`，可 pass）；非【杀】不触发（P3 追加） |
| 进攻马 / 防御马 | ✅ | 距离修正（P2.2） |
| 青龙偃月刀 | ✅（P4.2） | 【杀】被【闪】抵消后，可对该目标再使用一张【杀】（`AfterDodge` 追加窗口，`is_sha` 判定；新【杀】重新计算无双/铁骑） |
| 贯石斧 | ✅（P4.2） | 【杀】被【闪】抵消后，可弃 2 张手牌使此【杀】依然造成伤害（`AfterDodge` → `MultiSelect(guanshi)` → `apply_damage`） |
| 寒冰剑 | ✅（P4.2） | damage_before：造成伤害时防止伤害，改为弃置目标两张牌（`Hanbing` 帧；目标不足 2 牌不询问；简化：弃置牌随机） |
| 雌雄双股剑 | ✅（P4.2） | 指定异性目标后由目标二选一：弃 1 张手牌（`MultiSelect`）或令攻击方摸 1（`Cixiong` 帧，先于【闪】响应） |
| 铁索连环 | ✅（P4.3） | 横置/重置目标（`Seat.chained`）；可重铸（`recast:<card>` 弃置并摸 1）；**火/雷属性伤害在同横置角色间传导**（传导前全部重置，避免级联） |
| 火攻 | ✅（P4.3） | 目标亮一张手牌（`data.revealed`），你可弃同花色手牌造成 1 点火焰伤害（`HuoGong` 帧，可放弃）；无懈/藤甲/铁索（后续）按火焰伤害处理 |
| 朱雀羽扇 | ✅（P4.3） | 出牌阶段将普通【杀】当火杀使用（`skill:zhuque:<card>`，PlayPhase 注册表；响应转化后续） |
| 兵粮寸断 | ✅（P4.3） | 延时锦囊（距离 ≤1）：判定非黑桃则跳过摸牌阶段；判定前无懈窗口与改判窗口复用延时锦囊流程 |
| 藤甲 / 白银狮子 / 古锭刀 | ✅（P4.3） | 藤甲：火焰伤害 +1、普通【杀】/南蛮/万箭无效；白银狮子：受到的伤害改为 1；古锭刀：目标无手牌时【杀】伤害 +1（均已入 EQUIPMENT_SKILLS） |
| 丈八蛇矛 | ✅（P4.2） | 两张手牌当【杀】：出牌阶段主动使用 `skill:zhangba:<c1>,<c2>`；需要【杀】时响应 `respond:<c1>,<c2>`（南蛮/决斗/借刀）；受出杀次数限制（主动使用） |
| 方天画戟 | ✅（P4.2） | 最后一张手牌的【杀】可额外指定至多两个目标（`ShaFrame.pending_targets` 顺序结算；组合选项 `targets:<p1,p2[,p3]>`；追加目标不重复触发流离） |

## 6. 测试

`cargo test -p sanguosha`：技能用例在 `src/tests/p24.rs`（奸雄、制衡、闭月、集智、奇才、克己、救援）、`p24b.rs`（仁德、离间、突袭、奇袭、激将、护驾）、`p24c.rs`（青釭剑/八卦阵/边界：无同势力、次数限制、突袭提前确认、奇袭仍有无懈窗口等）与 `p25.rs`（观星槽位/`arrange` 校验/可见性；五谷丰登顺序与无懈）。首批 8 将均已 ≥3 条用例；P4.1 新增 `p41.rs`（鬼才 8 条）、`p42.rs`（武圣/龙胆/倾国/急救 8 条）、`p43.rs`（C1 9 条）、`p44.rs`（C2-1 6 条）、`p45.rs`（C2-2 6 条）与 `p46.rs`（流离 3 条）与 `p47.rs`（D 回归 12 条）与 `p48.rs`（国色 3 条）；当前 **210** 用例（门禁 `npm run check:sgs-skill-cases` 校验标准 25 将每人 ≥3 条；含结算栈/锦囊/无懈、特殊区契约、视图/原语门禁、事件重放与 400 步随机局守恒回归）。

## 7. 观星（P2.5 特殊区技能）

- 触发：拥有「观星」的玩家**准备阶段自动进入**（P2.5 口径：不询问是否发动、不可取消；每回合一次）。
- 牌：取 `k = min(存活角色数, 5)` 张进 `deck.processing`，写入 `special_zones.guanxing`（`visibility: owner`、`slots` 2×5、`rev`）。
- 提交：`arrange:<csv>`（最终顺序 = 新牌堆顶 → 底）；校验张数/去重/来源/处理区，重放与非法提交一律拒绝且不改状态。
- 详见 `special-zones.md` §5、`ui-spec.md` §7（P3 拖拽 UI）。

## 8. 实现口径（自动/锁定化的技能）

以下技能规则为「可以」，引擎按**自动发动/锁定化**实现（与观星、克己一致；后续如需改为可选，统一加 `choice` 帧）：

| 技能 | 实现口径 |
|---|---|
| 天妒 | 判定牌生效后自动获得（不再询问） |
| 连营 / 枭姬 | 失去最后手牌 / 装备后自动摸牌 |

裸衣**不在此列**：它是手动触发（摸牌阶段 `choice` pending，可放弃并正常摸 2 张）。

遗计的口径偏差：规则为「每受到 1 点伤害后」触发，引擎当前**按每次伤害结算触发一次**（多伤事件只发一次）；分配部分已实现（逐张 `target`，可留给自己）。待 P4.2 `damage_before` 窗口统一后改为按点触发。

流离的口径偏差：弃置费用当前限定为**手牌**（不支持弃置装备区的牌），且转移目标不含【杀】使用者与空城角色；待 P4.2 装备/弃牌选择统一后补装备。

判定窗口口径（P4.2 收口）：**铁骑 / 刚烈 / 洛神 / 八卦阵均已接入通用 `judge_before`（`QuickJudge` 帧），全部支持鬼才改判**；与延时锦囊判定共用 `judge_rewrite` pending 与「判定者左手侧起、判定者最后」的询问顺序。
