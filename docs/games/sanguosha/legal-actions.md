# 三国杀合法动作设计

> 状态：**P0 草案 v0.1（2026-10-26）**。配套 `rules.md`、`state-machine.md`、`rule-engine-permission.md`、`docs/conventions/interaction-primitives.md`。
> 原则：动作 id 由引擎**确定性生成**；`execute_action` **无条件重校验**；`legal_actions` 只描述「当前一步」，多步交互由 `pending` 推进。

## 1. 动作 id 语法

```
lobby:<action>[:arg]
setup:<action>[:arg]
play:<card_instance_id>            # 选择一张手牌/装备牌作为「使用」
skill:<skill_code>[:arg]
end_play
respond:<card_instance_id>         # 响应（闪/杀/桃/无懈/酒）
pass                               # 放弃响应 / 放弃可选动作
discard:<card_instance_id,csv>     # 弃置多张
choose:<option_id|codes>           # pending 选项（编号/代码/多选）
target:<player_id|player_id,csv>   # 选择目标（子交互）
resume                             # 重发当前待办（不改状态）
hint                               # 同 resume（别名）
```

- `card_instance_id` 是牌实例 id（不是牌名），防止伪造/混淆。
- `skill_code` 来自中台/引擎技能注册表；参数用 `:` 分隔。
- 复杂参数统一放在 `pending` 的 `options` 中，不把自由文本塞进 action id。

## 2. 两级动作模型

| 层级 | 内容 | 用途 |
|---|---|---|
| **粗动作** | `play:<card>`、`skill:<code>`、`end_play`、`respond:<card>`、`pass` | 决策模型/Web 列表，尽量 ≤26 |
| **子交互** | `target:*`、`discard:*`、`choose:*`、`respond:*` | 由 `pending` 驱动，逐步完成 |

- 一次逻辑交互（如出杀）可能是：`play:c12` → `target:p2` → `respond:c7/pass`；
- `legal_actions` 每次只返回**当前步骤**的合法动作；`pending_queue` 中的后续步骤在前一步完成后才可见；
- Web 端可逐步提交；后续 `execute_actions` 批量提交仅作为优化（ADR-0012 前置缺口），不改变单步语义。

## 3. 大厅 / 准备

| 动作 | 权限 | 说明 |
|---|---|---|
| `lobby:join` | 群成员 | 加入等待厅（未在座位中） |
| `lobby:leave` | 在座玩家 | 退出（仅 lobby；房主退出 → 本局废弃） |
| `lobby:kick:<openid>` | 房主 | 踢出目标 |
| `lobby:start` | 房主 | 人数/身份配比合法 → `setup` |
| `lobby:abort` | 房主 | 中断，不结算 |
| `lobby:add_bot` | 房主 | 机器人补位（测试） |
| `setup:choose_general:<code>` | 当前选将玩家 | 主公：全部主公将 + N 张非主公（默认 N=5）中选 1；其他：同时获得 M 张候选（默认 M=5，玩家间不重复）中选 1；未选回池 |
| `setup:confirm` | 当前选将玩家 | 确认（如需要） |

## 4. 回合内动作

### 4.1 准备 / 判定 / 摸牌

- 默认自动，无玩家动作；
- 可选技能触发时，引擎下发 `pending`（`choose`/`respond`）；
- 判定改判：`skill:<judge_skill>:<card_id>` 或 `choose`，由技能定义。

### 4.2 出牌阶段

合法粗动作：

- `play:<card>`：使用手牌/装备区的牌（装备区的牌通常不能主动使用，仅技能可）；
- `skill:<code>[:arg]`：发动可主动发动的技能；
- `end_play`：结束出牌阶段。

卡牌合法性（默认）：

| 牌 | 合法条件 |
|---|---|
| 杀 | 出牌阶段；目标在攻击范围内；默认每阶段 1 次（诸葛连弩/技能例外）；目标为其他角色（方天画戟可额外目标） |
| 闪 | 仅响应需要【闪】的询问，不能主动使用 |
| 桃 | 出牌阶段对自己、且已受伤；或濒死救援（pending） |
| 酒 | 出牌阶段对自己、每回合限一次；或濒死对自己回复 |
| 装备 | 在手牌；目标槽位可替换（旧装备入弃牌堆）；槽位=武器/护具/进攻马/防御马/特殊（木牛流马预留） |
| 普通锦囊 | 满足目标/距离；可被无懈 |
| 延时锦囊 | 目标判定区无同名；置入后可被无懈 |
| 无懈可击 | 仅当锦囊结算 pending 时；任意玩家按响应顺序 |

子交互：

- `target:<player>` / `target:<p1,p2>`：选择 1..N 个合法目标；
- `discard:<c1,c2>`：弃置指定张数（手牌上限/技能代价）；
- `choose:<option>`：从 pending 选项中选择；
- 取消：若牌尚未结算，可 `pass`/`cancel`（由牌/技能定义；已支付代价不退还）。

### 4.3 弃牌阶段

- `discard:<c1,c2,...>`：弃牌阶段恰好弃置 `手牌数 - 当前体力值` 张（技能可改体力/上限）；
- 超出/不足均非法；
- 弃置完成后进入结束阶段。

### 4.3.1 持有上限

- 任何时刻手牌上限 `config.max_hand_cards`（默认 20）；摸牌/获得牌后超出时，引擎创建弃牌 `pending`（弃至上限）后再继续；
- 该 pending 与弃牌阶段独立。

### 4.4 响应 / 濒死

- 需要【闪】/【无懈】/【桃】/【酒】时，引擎创建 `pending`：
  - `respond:<card_id>`：打出对应牌；
  - `skill:<code>`：用技能转化/替代（如武圣、龙胆、急救）；
  - `pass`：放弃响应。
- 响应顺序由引擎计算：闪/桃/濒死/改判/技能从当前玩家（判定者为当前回合玩家）左手侧开始；**无懈链从原锦囊使用者左手侧开始，每次有人打出无懈重新从该使用者左手侧开始，允许同一玩家多次响应**。只有当前 `actor` 能提交；其他人提交返回权限错误。
- 濒死求桃：每轮可多次使用【桃】/【酒】，直到体力 > 0 或放弃。

## 5. 合法动作列表生成

### 5.1 输入

- `room_id`、`player_id`；
- 引擎内部状态（完整）；
- 当前 `pending`（若有）。

### 5.2 输出

```json
[
  { "id": "play:c12", "description": "使用【杀】（黑桃7）" },
  { "id": "skill:jijiang", "description": "发动【激将】" },
  { "id": "end_play", "description": "结束出牌阶段" }
]
```

- `description` 必须人类可读，含牌名/花色点数/目标提示（但不泄露他人手牌）；
- 不在合法集内的 id 一律拒绝；
- 顺序稳定（按手牌顺序 + 技能注册顺序），Q 侧 `#选 N` 依赖下标（ADR-0012 冻结）。

### 5.3 数量控制

- 粗动作目标 ≤ **26**（`GAME_ACTION_CHOICE_LIMIT`）；
- 超限时引擎给出**候选剪枝**（必选响应 > 可致死 > 资源收益 > 其他），完整合法集进决策日志（ADR-0019）；
- Web 可展开全部；Q 侧后置，若启用则分页/`#动作 <id>`；
- **不得**通过合并/改序改变 `legal_actions` 的下标语义。

## 6. 特殊区动作

| 区域 | 动作 | 说明 |
|---|---|---|
| 观星 `guanxing` | `choose:<ordered codes>` / 后续 `ordered` 原语 | 多张排序；Web 拖拽，Q 后置 |
| 五谷丰登 `wugu` | `choose:<option>` | 按座位顺序每人选 1 |
| 判定 `judge` | 自动 / `skill:<改判>` | 判定牌公开 |
| 濒死 `dying` | `respond:<桃/酒>` / `pass` | 响应顺序 |
| 无懈链 `wuxie_chain` | `respond:<无懈>` / `pass` | 从原锦囊使用者左手侧开始，可递归抵消，允许同一玩家多次响应 |

- 特殊区动作同样走 `execute_action` 重校验；
- 新增特殊区先登记 `special-zones.md`，再定义动作 id。

## 7. 无状态动作

| 动作 | 说明 |
|---|---|
| `resume` / `hint` | 不改状态、不产事件；重发当前 `pending`/下一步提示，随消息重签个人视图 token |
| `state_summary` | 只读，不属于动作 |

## 8. 错误约定

- 非法动作：`错误: <原因>`（如「未轮到你」「目标不在攻击范围」「牌不在手牌」）；
- 权限错误与非法动作均不改状态；
- 错误文案不得泄露隐藏信息（他人手牌、身份、牌堆顺序）；
- 所有成功动作产出事件（`ActionResult.events`），版本对齐最后事件 seq。

## 9. 示例

```
# 刘备出杀
play:c12 → target:p2 → p2 respond:c7 / pass → damage/dying → ...
# 弃牌
discard:c3,c9
# 无懈链
respond:c21（无懈）→ respond:c22（反无懈）→ pass
# 观星（后续）
choose:c1,c3,c2
```

## 10. P0 待确认

1. `cancel` 是否作为独立动作 id（当前用 `pass` 表达放弃）。
2. 多目标选择的顺序是否影响结算顺序（默认按玩家选择顺序）。
3. 观星 `ordered` 原语的具体编码（P2 定）。
4. 持有上限超出时的弃置交互（玩家选择/自动）P2 定。
