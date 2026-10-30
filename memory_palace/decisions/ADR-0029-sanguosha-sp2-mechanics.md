# ADR-0029：三国杀 P4.4.1 SP-2 机制（废除装备栏 / 延迟结算 / 不可响应 / 观看牌堆顶）

- **状态**：已采纳（2026-10-30，P4.4.1 SP-2 批次落地；风林火山 48 将全部实现）
- **关联**：ADR-0016、ADR-0022（装备技能化）、ADR-0023（标记）、ADR-0027（觉醒/额外技能）、ADR-0028（SP-1）、`skills.md` §2.6、`rules-rulings.md` §15
- **影响范围**：`crates/engines/sanguosha`（新增 `resolve_sp2.rs`、`Seat.abolished_slots/judge_abolished`、`SgsState.scheduled`、`ShaFrame.no_respond`/`TrickFrame.no_wuxie`、`turn::to_void_card/from_void_card`、`view.rs` 新字段）

## 1. 背景

SP-2 8 将（卢植/郝昭/陈到/诸葛瞻/周妃/毌丘俭/陆抗/张绣）需要的机制缺口：

| 机制 | 使用者 | 缺口 |
|---|---|---|
| 武将牌标记（牌实例） | 卢植·明任、毌丘俭·征荣、周妃·箜声 | 有 `marks`，无统一读写 helper |
| 废除装备栏（永久） | 陆抗·决堰、张绣·雄乱 | 装备槽恒可装备/生效 |
| 废除判定区 | 张绣·雄乱 | 判定区恒可用 |
| 延迟到「下回合结束」 | 郝昭·镇骨 | 无定时效果队列 |
| 牌移出/移入游戏 | 周妃·良姻 | `deck.void` 无进出钩子 |
| 不可响应（杀/锦囊） | 陈到·往烈 | 只有 `no_shan`（烈弓）；锦囊无「跳过无懈」 |
| 本阶段不能再使用牌 / 手牌被封 | 陈到·往烈、张绣·雄乱 | 无出牌禁令 |
| 回合内统计（造成伤害/弃牌） | 诸葛瞻·罪论 | 无统计计数器 |
| 观看牌堆顶 N 张并选择 | 诸葛瞻·罪论 | 只有观星（全部回置排序） |
| 每回合首次触发 | 诸葛瞻·父荫、周妃·良姻 | 无统一「每回合首次」原语 |

## 2. 决策

1. **武将牌标记**：复用 `Seat.marks`（牌实例，计入牌数守恒，死亡弃置）；新增 `skills::mark_cards/add_mark/remove_mark`。标记名：`ren`（任）/`rong`（荣）/`kong`（箜声）。
2. **废除装备栏**：`Seat.abolished_slots: Vec<EquipSlot>`（`#[serde(default)]`，本局永久）。`SgsState::equip_def` 对废除槽返回 `None`（装备技能随之失效）；`skills::abolish_slot` 弃置该槽装备并触发枭姬；出牌阶段不可装备到废除槽（`play_card` 拒绝 + `legal_actions` 过滤）。
3. **废除判定区**：`Seat.judge_abolished: bool`；`resolve_sp2::xiongluan_target` 弃置现有判定牌并置位；延时锦囊目标枚举（乐不思蜀/兵粮寸断/闪电自身）与 `step_trick` 过滤该角色。
4. **延迟结算**：`SgsState.scheduled: Vec<ScheduledEffect{owner,target,kind,due_seat}>`；`turn::end_turn` 在切换前扫描 `due_seat == 当前座位` 的条目并执行。镇骨：选目标时立即结算一次，再登记 `due_seat = 目标`。
5. **牌移出/移入游戏**：`turn::to_void_card` / `from_void_card` 统一进出 `deck.void` 并触发 `resolve_sp2::liangyin_trigger`（每回合首次，`first_each_turn("liangyin")`）。
6. **不可响应**：`ShaFrame.no_respond`（目标不能出【闪】）+ `TrickFrame.no_wuxie`（`step_trick` 在 Wuxie 入口直接跳到 Effect）。往烈以独立动作 `skill:wanglie:<card>` 使用牌，使用后写 `turn_counters["no_card_use"]`。
7. **出牌禁令**：`turn_counters["no_card_use"]`（往烈，本阶段）与 `turn_counters["hand_sealed:<seat>"]`（雄乱，本回合）→ `legal_actions` 只保留 `end_play`；响应选项（`respond_options`）与濒死【桃】只给 `pass`。
8. **回合统计**：`turn_counters["damage_dealt"]`（`apply_damage_inner` 中 source == 当前回合玩家时 +1）；`turn_counters["discard_done"]`（`act_discard` 中弃牌者为当前回合玩家时 +1）。
9. **观看牌堆顶**：`resolve_sp2::start_zuilun` 抽 3 张进处理区 + 特殊区 `zuilun`（`visibility: Owner`，payload `cards/conditions`）；帧 `ZuiLun{cards,stage,picks}` 按条件数逐张选择，剩余按原顺序 `deck.draw.push` 回置；0 项时改为选一名其他角色、双方各失去 1 体力。
10. **每回合首次**：`skills::first_each_turn(st, key)`（`turn_counters["once:<key>"]`，回合结束自动清空）。
11. **觉醒**：`turn::phase_step` Prepare 中调用 `resolve_sp2::hongju_awaken` / `poshi_awaken`（与山包 `awaken_triggers` 并列）。
12. **失去基础技能**：新增 `skills::lose_skill`（`flags["lost:<code>"]`），破势用其失去「决堰」。

## 3. 已知边界

- **清侧（毌丘俭）/ 怀柔（陆抗）**：仓库无官方文本 → 登记 `EXTRA_SKILLS` 且 `implemented:false`；觉醒可授予但当前无效果。
- **良姻**：仅响应 `deck.void` 进出；当前引擎没有主动移出游戏的牌（木牛流马未实现）→ 实战基本不触发（测试通过 `turn::to_void_card` 覆盖）。
- **往烈**：仅支持【杀】与单目标普通锦囊（AOE/决斗/无中生有不可用）；「不能被响应」对【杀】= 目标不能出【闪】，对锦囊 = 跳过无懈窗口；「本阶段不能再使用牌」= 只禁主动使用，仍可打出响应牌。
- **雄乱**：封手牌不含装备区与技能（八卦阵/主公技仍可用）；废除装备栏同时弃置原装备；判定区废除为永久。
- **镇骨**：「手牌调整至与你相同（至多 5）」按 `min(郝昭手牌, 5)`；目标阵亡跳过；若郝昭已阵亡则不再结算。
- **罪论**：剩余牌按原顺序回置（未开放任意排序）；「手牌数全场最少」按「不大于其他所有存活角色」判定（并列算满足）。
- **父荫**：以「每回合首次」计数（`turn_counters`），同回合第二次不再生效。
- **决堰**：坐骑栏废除优先废进攻马槽（两者独立，再次发动可废另一槽）；宝物栏「集智」以 `turn_counters["jueyan_jizhi"]` 记录本回合生效。
- **谦节**：拼点屏蔽在目标枚举层实现（驱虎/天义/制霸/烈刃）；延时锦囊屏蔽在目标枚举层实现。
- **征荣**：触发点在使用时机（非目标确定后），目标为「手牌数 ≥ 你的任一其他角色」；置「荣」的牌从目标随机手牌取。
