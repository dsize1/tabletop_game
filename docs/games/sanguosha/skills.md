# 三国杀技能注册表与触发钩子（P2.4）

> 状态：**P2.5（2026-10-28）**：首批 8 位武将 14 个技能全部实现（每将 ≥3 条用例）+ **观星**（特殊区 `guanxing`，P2.5 契约）；装备可选效果实现青釭剑/八卦阵/仁王盾/诸葛连弩，其余随 P4。
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
| 诸葛亮 `sgs_hero_4` | 观星 | `guanxing` | 普通 | `phase_start(prepare)`：取 `min(存活角色数,5)` 张进特殊区 `guanxing`（2×5 槽），`arrange:<csv>` 一次性回写（P2.5；空城留 P4） |

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

## 4. 未实现（P4）

| 武将 | 技能 | 缺口 |
|---|---|---|
| — | — | 首批 8 将 + 观星已全部实现；其余武将（标准 25 → 包扩展）在 P4 补齐 |
| 诸葛亮 | 空城 | 锁定技：无手牌时不能成为【杀】/【决斗】目标（P4，与目标校验一起做） |
| 其他标准将 | — | 按 P4 包批次实现（每将 ≥3 用例） |

## 5. 装备可选效果（标准包）

| 装备 | 状态 | 实现要点 |
|---|---|---|
| 诸葛连弩 | ✅ | 出牌阶段【杀】无次数限制（`turn::legal_actions` / `play_card`） |
| 青釭剑 | ✅ | 【杀】无视目标防具（仁王盾无效、八卦阵不可用）；`RespondFrame.ignore_armor` |
| 八卦阵 | ✅ | 需要【闪】时多一个 `skill:bagua` 选项：判定红色视为出【闪】，黑色继续响应 |
| 仁王盾 | ✅ | 黑色【杀】无效（青釭剑可无视） |
| 进攻马 / 防御马 | ✅ | 距离修正（P2.2） |
| 寒冰剑 / 雌雄双股剑 / 贯石斧 / 青龙偃月刀 / 丈八蛇矛 / 方天画戟 / 麒麟弓 | ⏳ P4 | 需要 `damage_before` 窗口、多目标【杀】、双牌转化、被闪后追加结算等扩展；随 P4 装备技能一起实现 |

## 6. 测试

`cargo test -p sanguosha`：技能用例在 `src/tests/p24.rs`（奸雄、制衡、闭月、集智、奇才、克己、救援）、`p24b.rs`（仁德、离间、突袭、奇袭、激将、护驾）、`p24c.rs`（青釭剑/八卦阵/边界：无同势力、次数限制、突袭提前确认、奇袭仍有无懈窗口等）与 `p25.rs`（观星槽位/`arrange` 校验/可见性；五谷丰登顺序与无懈）。首批 8 将均已 ≥3 条用例；当前 98 用例（含结算栈/锦囊/无懈、特殊区契约、视图/原语门禁、事件重放与 400 步随机局守恒回归）。

## 7. 观星（P2.5 特殊区技能）

- 触发：拥有「观星」的玩家**准备阶段自动进入**（P2.5 口径：不询问是否发动、不可取消；每回合一次）。
- 牌：取 `k = min(存活角色数, 5)` 张进 `deck.processing`，写入 `special_zones.guanxing`（`visibility: owner`、`slots` 2×5、`rev`）。
- 提交：`arrange:<csv>`（最终顺序 = 新牌堆顶 → 底）；校验张数/去重/来源/处理区，重放与非法提交一律拒绝且不改状态。
- 详见 `special-zones.md` §5、`ui-spec.md` §7（P3 拖拽 UI）。
