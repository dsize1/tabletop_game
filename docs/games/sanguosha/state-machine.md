# 三国杀状态机设计

> 状态：**P0 草案 v0.1（2026-10-26）**。配套 `rules.md`、`data-model.md`（状态 JSON）、`legal-actions.md`、`rule-engine-permission.md`。
> 实现见 P2 `crates/engines/sanguosha`；本文约束状态字段、阶段与事件，不允许实现时自行发明。

## 1. 状态分层

```
GameState（game-engine-core 信封）
├─ game_id / room_id / group_id
├─ status: Active | Finished
├─ players: Vec<openid>          # 座位顺序（含已死亡，便于回放）
└─ data: SanguoshaState          # 引擎私有（schema_version）
```

`SanguoshaState`（详见 `data-model.md` §4.1）：

| 字段 | 说明 |
|---|---|
| `schema_version` | 私有状态版本，迁移入口 |
| `mode` | `identity`（本期）/ `guozhan`（预留） |
| `phase` | `lobby` / `setup` / `turn` / `dying` / `finished` / `aborted` |
| `host` | 房主 openid |
| `config` | 规则配置：`lord_candidate_count` / `other_candidate_count` / `max_hand_cards` / `base_attack_range` / `base_defense_range`（默认 5 / 5 / 20 / 1 / 1，可配置） |
| `player_count` / `identity_variant` | 人数与双内变体 |
| `round` | 轮次（以主公回合为一轮起点，按实现定义） |
| `turn` | `{ seat, phase, extra_phases }` 当前回合 |
| `seats[]` | 身份/武将/体力/手牌/装备/判定区/标记 |
| `cards{}` | 牌实例：定义、花色、点数、区域、归属 |
| `deck` | `draw` / `discard` / `void` 牌实例 id |
| `pending` | 当前交互（唯一） |
| `pending_queue` | 同一次结算产生的后续交互队列 |
| `special_zones` | 观星/五谷等特殊区 |
| `skill_subscriptions` | 技能事件订阅（重放/调试） |
| `winner` | 终局结果 |

## 2. 全局状态

| 全局 `phase` | 进入条件 | 退出 |
|---|---|---|
| `lobby` | `create_game` 后 | 房主 `lobby:start` 且人数合法 → `setup` |
| `setup` | 开赛 | 发身份/选将/初始手牌完成 → `turn` |
| `turn` | 正常对局 | 胜负判定/中断 → `finished` / `aborted` |
| `dying` | 任意角色体力 ≤ 0 | 脱离濒死 → 回到原结算点；死亡 → `turn` |
| `finished` | 胜负产生 | 终态 |
| `aborted` | 房主中断/异常 | 终态 |

- `dying` 是**中断态**：进入时保存当前结算栈位置，脱离后恢复；不得丢弃原 `pending`。
- `finished` / `aborted` 后拒绝一切写动作（`resume`/`hint` 只读除外）。

### 2.1 选将（setup）

- 主公候选 = 全部主公武将 + `config.lord_candidate_count` 张非主公武将（默认 5）；主公选 1。
- 主公未选武将进入武将池；从主公左手侧开始，每名其他玩家**同时**获得 `config.other_candidate_count` 张候选（默认 5，玩家之间不重复），各选 1；未选候选回池。
- 候选/池只取 `status=live` 的武将。
- **实现口径（P2.1，2026-10-27）**：引擎内置池暂为标准包 25 将（自动生成）；当 `M × 其他玩家数` 超过池子时按**均分降级**（每人不低于 1、不超过 M，余数给左手侧靠前的玩家），P4 扩池后恢复按 M 发满。
- 选将结果写入 `seats[].general`，进入 `turn`。

### 2.2 pending 类型（P2.2 实现）

| kind | actor | data | 动作 | 出现时机 |
|---|---|---|---|---|
| `target` | 出牌者 | `card/source/type` | `target:<player>` | 杀、乐不思蜀选目标 |
| `respond` | 被杀目标 | `card/source/target/damage` | `respond:<闪>` / `pass` | 杀结算，目标需出闪 |
| `dying` | 按响应顺序的持桃者 | `victim` | `respond:<桃>` / `pass` | 体力 ≤ 0 |
| `discard` | 需弃牌者 | `required/reason`（`hand_cap` / `discard_phase`） | `discard:<csv>`（可逐张） | 超过持有上限、弃牌阶段 |
| `wuxie` | 按座位顺序的**所有存活玩家** | `root` / `count` / `cursor` | `respond:<无懈>` / `pass` | 锦囊生效前、延时锦囊判定前 |
| `pick_card` | 锦囊使用者 | `target` / `options`（装备槽/手牌/判定区） | `pick:hand` / `pick:equip:<slot>` / `pick:judge:<id>` | 顺手牵羊、过河拆桥（无懈后） |
| `target`（借刀） | 锦囊使用者 | `stage`（`borrow_first`/`borrow_second`） | `target:<player>` / `confirm` / `cancel` | 借刀杀人二阶段目标 |
| `choice` | 技能拥有者 | `skill` / `card` | `choose:gain` / `pass` | 奸雄（第一批技能） |
| `choose_many` | 技能拥有者 | `purpose` / `selected` / `min` / `max` | `choose:<card>` / `confirm` / `cancel` | 制衡（弃 N 摸 N）、仁德（选牌）、离间（选 1 张） |
| `choice`（巡袭/接力） | 技能拥有者 | `skill` / `needed` | `choose:tuxi` / `respond:<card>` / `pass` | 突袭（放弃摸牌）、激将/护驾（同势力代出杀/闪） |
| `wugu` | 按选择顺序的当前玩家（**释放者先选**） | `card` / `remaining` / `chooser` / `zone` | `pick:<card_id>` | 五谷丰登（P2.5）；每人一次无懈窗口 |
| `arrange` | 技能拥有者 | `zone` / `rows` / `cols` / `slots` / `rev` / `interaction` | `arrange:<csv>` | 观星（P2.5）：准备阶段自动进入，不可取消 |

- P2.3 起由**结算栈**驱动：`resolution: Vec<Frame>`（Trick/Wuxie/PickCard/Aoe/Respond/Dying/Discard），`pending` 是栈顶投影；濒死与无懈嵌套通过压栈/弹栈恢复。
- 响应类询问（`wuxie`/`respond`/`dying`/接力）：**无手牌且无替代选项的玩家自动跳过**；**有手牌者一律显式询问**（无对应牌时唯一动作是 `pass`），不代选（2026-10-28 裁定，见 `special-zones.md` §4.1）。
- **P2.3** 已实现锦囊/无懈/濒死/弃牌/判定 pending；**P2.4** 新增 `choice`（奸雄）与 `choose_many`（制衡）；**P2.5** 新增 `wugu` 与 `arrange`（特殊区帧 `Frame::Wugu` / `Frame::Arrange`，见 `special-zones.md` §5/§6）。

- 只有 `pending.actor` 能提交；其他玩家提交返回权限错误。
- 阶段机在无 pending 时自动推进，停在**出牌阶段**等待输入。

## 3. 回合阶段

```
prepare → judge → draw → play → discard → end
```

| 阶段 | 默认行为 | 技能钩子 |
|---|---|---|
| `prepare` | 无 | `phase_start(prepare)` / `phase_end(prepare)` |
| `judge` | 判定区 LIFO 逐张判定 | `judge_before` / `judge_after` / 改判技能 |
| `draw` | 摸 2 张（可被修改/放弃）；摸后若手牌 > `max_hand_cards`，立即弃至上限 | `draw_count` / `draw_before` / `draw_after` / `hand_cap` |
| `play` | 循环执行合法动作，直到 `end_play` | `card_use_before/after`、`skill:*` |
| `discard` | 弃至**当前体力值**（技能可改）；持有上限 `config.max_hand_cards` 在摸牌/获得时检查 | `hand_limit` / `discard_before/after` |
| `end` | 无 | `phase_end(end)` / `turn_end` |

### 3.1 回合伪代码

```
start_turn(seat):
  turn = { seat, phase: prepare, extra_phases: [] }
  trigger(turn_start, seat)
  for phase in [prepare, judge, draw, play, discard, end] + extra_phases:
     turn.phase = phase
     trigger(phase_start, phase)
     run_phase(phase)
     trigger(phase_end, phase)
     if game.finished: return
  trigger(turn_end, seat)
  next_alive_seat()
```

- `run_phase` 只做默认行为；技能通过钩子**插入/跳过/重复**阶段，不修改核心循环代码。
- `extra_phases` 由技能写入（如额外判定/额外出牌），按插入点顺序执行。
- 阶段可以被技能跳过（如【乐不思蜀】跳过 `play`、兵粮寸断跳过 `draw`）。
- 一个阶段内的多步交互通过 `pending` 队列串行，不递归改状态。

## 4. 判定阶段（LIFO）

```
while judge_zone not empty:
  card = judge_zone.pop_last()        # 末尾=栈顶，后进先判
  judge_card = draw_top()
  trigger(judge_before, card, judge_card)
  # 改判技能按响应顺序可替换 judge_card
  trigger(judge_after, card, judge_card)
  resolve_delayed_trick(card, judge_card)
  move(card, discard)
```

- 判定牌公开；判定结果 = 花色 + 点数。
- 改判/响应顺序：从当前回合玩家（判定者）左手侧开始（`rules.md` §9）。
- 判定区不能有同名延时锦囊（`rules.md` §5.2）。

## 5. 交互与 pending 队列

### 5.1 单活跃 pending

```jsonc
{
  "kind": "ask_card | ask_target | ask_choice | ask_discard | ask_respond",
  "actor": "openid",
  "skill": "skill_code | null",
  "options": [{ "id": "...", "label": "...", "value": {} }],
  "data": {},
  "created_by_event": "event_id"
}
```

- 同一时刻**只有一个** `pending`（`actor` 明确）；其他同时产生的询问进 `pending_queue`，前一个解决后 `promote`。
- `pending` 解决后立即清除；若产生新询问，入队。
- **无超时**：pending 可长期等待，玩家后续动作恢复。
- 非法/越权响应不动状态，返回错误。
- **响应顺序**：闪/桃/濒死/改判/技能从当前玩家（判定者为当前回合玩家）左手侧开始；**无懈链**从原锦囊使用者左手侧开始，每次有人打出无懈都重新从该使用者左手侧开始，允许同一玩家多次响应。

### 5.2 典型链

```
出牌 play:<card>
  → pending ask_target（选目标）
  → pending ask_card（如需额外弃牌/判定）
  → resolve card effect
  → pending queue 中下一个（如濒死求桃）
```

- 跨玩家响应（闪/无懈/桃）也是 `pending`，`actor` 按响应顺序；Web 显示等待态，Q 后置。

## 6. 技能事件总线

### 6.1 事件

| 事件 | 时机 |
|---|---|
| `game_start` | 发身份/选将完成 |
| `turn_start` / `turn_end` | 回合开始/结束 |
| `phase_start` / `phase_end` | 各阶段 |
| `card_use_before` / `card_use_after` | 使用牌前后 |
| `card_targeted` | 指定目标后 |
| `damage_before` / `damage_after` | 伤害前后 |
| `dying` / `death` | 濒死/死亡 |
| `judge_before` / `judge_after` | 判定前后 |
| `draw_count` | 计算摸牌数 |
| `hand_limit` | 计算弃牌阶段手牌上限（当前体力值/技能） |
| `hand_cap` | 检查持有上限 `max_hand_cards`（摸牌/获得后） |
| `equip_change` | 装备变化 |
| `card_lost` | 牌离开区域 |

### 6.2 排序

1. 技能声明优先级（数值小先）；
2. 距离当前回合玩家的座位顺序（顺时针）；
3. 注册顺序。
- **禁止**依赖 HashMap/HashSet 迭代顺序；所有遍历按座位数组。

### 6.3 结算栈

- 技能触发统一入栈；执行完成后弹栈。
- 触发中产生的新询问进 `pending_queue`，不递归改状态。
- 事件与状态变更同一事务落库（ADR-0004/0008）。

## 7. 濒死与死亡状态

```
when hp <= 0 and not dying:
  dying = true
  save_resume_point()
  trigger(dying)
  ask_rescue_order(current_player_left)
  if hp > 0: dying = false; restore_resume_point()
  else: death(player); rewards(); check_win()
```

- 濒死求桃按响应顺序逐个询问；无超时。
- 死亡：身份公开、弃置所有牌、移出距离/响应；伤害来源获得奖惩（杀反摸 3；主杀忠弃全部）。
- 死亡后立即检查胜负；主公死亡 → 反贼胜。

## 8. 视图投影

| 投影 | 内容 | 出口 |
|---|---|---|
| `public_board` | 座位、公开身份、武将/体力/装备/判定区、弃牌堆、牌堆余量、当前轮次/阶段、公共特殊区 | `get_state` / Web 公共区 |
| `player_area` | 本人手牌、装备（武器/护具/进攻马/防御马/特殊）、技能/可用动作、个人 pending、加密选项、私有标记 | `get_player_view` / Web 个人区 |
| `special_zones` | 观星/五谷/判定/濒死/无懈链，按可见性 | 授权玩家/公共 |

- 引擎内部完整状态；投影时脱敏；错误信息不得泄露隐藏牌/身份。
- 交互只能基于 `player_area` / 授权 `special_zones` 发起（ADR-0020）。

## 9. 终局与中断

- 胜负判定在每个死亡/结算后执行；`winner = { team, players }`。
- 终局：`status=Finished`，`phase=finished`，拒绝写动作。
- 中断：房主 `lobby:abort`（仅 lobby）或后续版本 `game:abort`；`phase=aborted`，不结算。
- 回放：`game_state` 快照 + `game_events` 重放；随机结果已入事件，不重新随机。

## 10. 与国战接缝（不实现）

- `mode=guozhan` 预留：`revealed`、`second_general`、`faction_team`、`ambition` 字段。
- 身份局不写入这些字段；国战实现时新增 `mode/guozhan.rs`，不改身份局流程代码。
