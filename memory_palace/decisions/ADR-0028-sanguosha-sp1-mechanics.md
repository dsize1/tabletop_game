# ADR-0028：三国杀 P4.4.1 SP-1 机制（转换技 / 计数标记 / 牌堆底 / 伪帝 / 拒战禁指）

- **状态**：已采纳（2026-10-30，P4.4.1 SP-1 批次落地）
- **关联**：ADR-0016、ADR-0023（标记/翻面）、ADR-0025（拼点）、ADR-0026（林包）、ADR-0027（山包）、`skills.md` §2.5、`rules-rulings.md` §14
- **影响范围**：`crates/engines/sanguosha`（新增 `resolve_sp.rs`、`Seat.mark_counts`、`SgsState.shenshi_marks`、`turn.rs` 阶段钩子、`resolve.rs` 用牌/伤害钩子、`view.rs` `marks_count`/`transforms`）

## 1. 背景

SP-1 8 将（袁术/王基/严颜/王平/陆绩/孙亮/蒯越蒯良/许攸）引入 6 类此前没有的能力：

| 机制 | 使用者 | 缺口 |
|---|---|---|
| 转换技（阳/阴面） | 严颜·拒战、蒯越蒯良·审时、许攸·成略 | 无「技能状态位」 |
| 非牌标记（「橘」） | 陆绩·怀橘 | `marks` 只能存牌实例，参与牌数守恒 |
| 牌堆底摸牌 / 置顶 | 许攸·寸目、恃才 | 摸牌固定从顶取；无「回置牌堆顶」原语 |
| 借用主公技 | 袁术·伪帝 | `has()` 只看自身武将技能 |
| 回合内禁止指定目标 | 严颜·拒战 | 目标枚举无「本回合禁用」过滤 |
| 出牌阶段伤害防止 / 用牌计数 | 孙亮·掣政 | 伤害链无「阶段+距离」防止；无本阶段用牌数 |
| 私有信息（观看手牌） | 蒯越蒯良·审时（阴） | 仅公开亮牌（火攻），无「只看不给别人看」 |

## 2. 决策

1. **转换技**：状态存 `Seat.flags["yj:<skill_code>"]`（`true` = 阴，缺省 = 阳）；`skills::yj_yin/toggle_yj` 读写。**每次发动后切换**，不随回合重置。视图 `player_area.transforms` 下发 `{code: "yang"|"yin"}`。
2. **计数标记**：新增 `Seat.mark_counts: BTreeMap<String,u32>`（与牌标记 `marks` 并存）；`skills::ju_count/add_ju/remove_ju`。**不参与牌数守恒**。视图公共区/个人区都下发 `marks_count`（陆绩「橘」为公共信息）。
3. **牌堆底 / 置顶**：`turn::draw_cards` 判断 `cunmu`，从 `deck.draw[0]` 取（`deck.draw` 末尾 = 顶）；`skills::shicai_trigger` 把处理区的牌移入 `deck.draw` 末尾并摸 1（寸目时摸底）。
4. **伪帝**：`skills::has` 在自身技能未命中后，若拥有 `weidi` 且非主公，则查真实主公武将的 **kind=Lord 且 implemented** 的技能；觉醒技（若愚）不借。
5. **回合内禁止指定目标（拒战）**：`turn_counters["deny:<user>:<target>"]`；`skills::deny_target/set_deny_target`。过滤点：`turn::sha_targets_with`、`resolve::trick_targets`（AOE）、`resolve::act_target` 单目标锦囊候选、【杀】目标枚举（`step_sha` 的无距离分支）。
6. **阶段统计**：`turn_counters["play_cards_used"]` 在 `start_sha_flags`/`start_trick_code`/桃/酒/装备分支自增；弃牌数复用 `phase_discarded`。掣政伤害防止在 `apply_damage_inner` 最前（怀橘之后、天香之前）。
7. **私有观看手牌（审时阴）**：牌面写入历史日志（`push_log` 仅本端视角可见于行动者日志流；视图不下发手牌明细），交付记录写 `SgsState.shenshi_marks[{owner,target,card}]`，回合结束阶段校验牌是否仍在目标手牌。
8. **奇制 / 恃才 / 立军 / 荐降触发点**：集中在用牌入口（`start_sha_flags` / `start_trick_code`）与 `act_target`，避免每张牌单独分支；`Frame::Qizhi` 在【杀】/锦囊结算前弹出，`Frame::Juzhan` 在【闪】响应帧之上先结算。
9. **伤害/摸牌内联**：庸肆（摸牌 +X、弃牌先弃 X）、怀橘（摸牌 +1）直接写在 `turn::phase_step`；整论/遗礼/进趋/溃诛/掣政用帧（`Zhenglun`/`Yili`/`Jinzou`/`Kuizhu`/`Chezheng`）。

## 3. 已知边界

- **奇制**：每次「使用非装备牌」结算一次（AOE 不按目标数叠加）；可弃手牌（随机）、装备或判定区牌。
- **进趋**：X=0 时弃光手牌；弃牌由玩家逐张自选（`MultiSelect`）。
- **飞军**：代价只支持弃手牌；选项一交牌由目标随机给一张；选项二弃装备为「第一件非空槽位」。
- **兵略**：以「首次对某座位发动飞军」为判据（同一角色第二次不触发）。
- **审时（阴）**：手牌明细只写历史日志（不通过视图下发）；「未失去该牌」以牌实例是否仍在目标手牌为准；结算在结束阶段自动执行（不询问）。
- **恃才**：装备牌与延时锦囊不触发；牌回置牌堆顶后由寸目从底摸，等价于「弃一张换一张」。
- **掣政**：结束阶段弃牌由孙亮选目标、牌随机；跳过出牌阶段（放权/巧变）同样触发统计。
- **立军**：孙亮侧的收益（摸 1 + 杀次数 +1）自动生效，未开放「是否令其摸牌」的询问。
- **伪帝**：不借用主公身份相关加成（如血裔按「拥有技能」生效，主公判定仍看真实主公）。
