# ADR-0022：装备效果技能化（Equipment-as-Skills）

- **状态**：已采纳（2026-10-30，用户确认「先补 ADR 并立项」）
- **关联**：ADR-0016（引擎内核：规则在代码、中台只读元数据）、ADR-0017（道具中台数据模型）、`docs/games/sanguosha/skills.md` §5、`development-plan.md` P4.2
- **影响范围**：`crates/engines/sanguosha`（技能注册表/hook 派发/装备效果）、Web 视图投影（P4.2 后续）

## 1. 背景

标准包 7 件装备效果当前散落在引擎各时机点：

| 装备 | 现状实现点 | 形态 |
|---|---|---|
| 诸葛连弩 / 马匹 / 青釭剑 / 仁王盾 | `sha_limit` / `in_attack_range` / `ignore_armor` 内联 | 被动（锁定） |
| 八卦阵 | `respond_options` 插入 `skill:bagua` | 窗口内可选 |
| 麒麟弓 | 伤害后压 `choice` pending | 窗口内可选 |
| 寒冰剑 | `respond_give_up` → `Hanbing` 帧（damage_before） | 窗口内可选 |
| 青龙偃月刀 / 贯石斧 | 闪成功后 `AfterDodge` 帧 | 窗口内可选 |
| 雌雄双股剑 | 指定目标后 `Cixiong` 帧 | 窗口内可选 |
| 丈八蛇矛 | 出牌阶段 `skill:zhangba:<c1>,<c2>` | 出牌阶段主动 |

问题：每新增一件装备都要在对应时机手写 if 分支/新帧；装备没有 `SkillDef` 条目，`skills.rs` 只登记武将技能；端侧无法统一展示「装备技能」。

## 2. 决策

1. **装备效果按技能建模**：`SkillDef.owner` 扩展为 `General(gid) | Equipment(card_code)`；`has(st, seat, code)` 同时检查武将技能与已装备牌技能。
2. **时机由引擎开窗口，效果由技能注册表实现**：
   - 引擎保留并统一 `Trigger` 窗口：`Passive` / `PlayPhase` / `TargetChosen` / `DamageBefore` / `AfterDodge` / `JudgeBefore`；
   - 窗口内通过 `skills::triggered(st, Trigger)` 收集持有者，按「装备槽顺序 → 座位顺序 → 注册顺序」确定性派发；
   - 技能实现各自只处理「生成选项/执行效果」，不再在调用点写装备判断。
3. **「点击武器启用」的边界**：
   - 仅**无窗口主动技**（出牌阶段可随时发动，如丈八蛇矛的转化出杀）适合常驻可点击入口；
   - **窗口类效果**（damage_before / after_dodge / judge_before）必须由引擎在时机点下发 pending；端侧把该 pending **渲染为武器卡高亮 + 浮层选项**（点击武器 = 提交 `choose:*`），而不是提前点击。
4. **规则仍在 Rust 代码**：注册表是代码内静态表；中台（`sgs_skills`）只存名称/类型/描述/图片等元数据，不得配置触发时机与结算逻辑（ADR-0016 不变）。
5. **视图扩展（后续）**：`player_area.equipment_skills[] = { slot, code, name, trigger, activatable }`，供 Web 渲染可点武器/高亮；不改变现有三分区契约语义（新增字段需同步 `view_contract` 期望集）。

## 3. 迁移计划（P4.2 收尾后立项执行）

1. `skills.rs` 引入 `SkillOwner` + `Trigger` + `triggered()`；`has()` 同时查装备技能。
2. 迁移 7 件装备到注册表（诸葛连弩/八卦/麒麟/寒冰/青龙/贯石/雌雄）+ 丈八，删除各处 `has_equip` 内联分支与专用 Frame 的重复胶水。
3. 通用判定窗口 `judge_before` 先落地（铁骑/刚烈/八卦/洛神统一走它，支持鬼才改判），再迁移装备技能。
4. 视图 `equipment_skills` + Web 武器高亮/浮层；补 e2e（窗口内高亮武器 → 点击 → 选项）。
5. 回归门禁：`cargo test -p sanguosha`、`check:sgs-view-contract`（新增字段）、`check:interaction-primitives`（新增 pending 类型需登记）。

## 4. 后果

- 新增装备/后续包只需：加一行 `SkillDef` + 一个效果函数；调用点不再扩散 if 分支。
- 窗口设计仍由引擎决定（可插拔不消灭时机窗口），因此**窗口先通用、再迁移**是正确顺序。
- 短期存在「旧内联 + 新注册表」双轨期；以迁移清单为准，迁移完成前不新增内联装备判断。
