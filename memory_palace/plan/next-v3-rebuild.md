# 任务：V3 引擎重构（v2 废弃）

> 立项：2026-11-12（用户裁定，见 ADR-0042）。**给新会话的开工说明**：
> 本文件是独立任务文档，新会话直接说「开始 V3 重构」，从 §0 的 B0 前置项开始。

## 0. 进度回写 + B0 前置项（2026-11-13，新会话必读）

> **⚠️ 上一会话把「类型骨架 + 合成测试」误报为「M1–M4 完成」，现纠正：**
> - **真完成**：M0（v1 oracle 复活，987 tests）+ 类型骨架（Zone/Event/Pending/SkillDef/route/advance/flow/gear/view 等）；
> - **骨架非完成**：M1–M4 的「回合驱动 / 事件生产 / 交互 pending / 完整卡牌结算 / 生命周期」这五个「除技能外的基本机制」**全是骨架或未实现**；
> - **119 用例 = 机制门禁，不等于技能语义正确**（据守永不触发、解围 no-op、颂威硬编码、放逐无选择、不屈简化、奋激/行殇靠手推事件）。
>
> **从 B0 重新开始**：B0 定义为「先完成引擎骨架剩余（P1–P6 前置项），验收后，才进入 B0 武将技能」。
> B1–B20 同构：每批武将技能只是「技能接入」，前置机制已在 B0 前置项做实。

### 0.1 B0 前置项（✅ 2026-11-14 已验收）

**验收准则（DoD）：不添加任何英雄技能，游戏即可真机验证「引擎 + 手牌 + 装备牌 + 锦囊牌」。**

> **状态：P1–P6 全部落地**，证据见 `docs/games/sanguosha/v3-b0-prereq.md` + 门禁用例
> `tests/turn_flow.rs`(12) / `tests/interact.rs`(11) / `tests/cards_full.rs`(21) /
> `tests/equip_effects.rs`(19) / `tests/full_game.rs`(5)；`npm run check:sgs3` 全绿。
> **端侧接入已完成（2026-11-14，ADR-0043）**：V3 接管正式 id `sanguosha`（MCP/server 只注册 V3，
> v2 退役为参考实现），大厅/选将沿 v1/v2 动作协议、视图输出 v1/v2 同形 schema（Web 零改动），
> 机器人打分与推进在引擎内（`engine::bot`）。真机链路：`#开局`→`#加入`/`#加机器人`/`#开始`→选将→出牌→终局。

| # | 前置项 | 内容 | 验收口径 |
|---|---|---|---|
| P1 ✅ | 回合驱动 | 真实 turn loop（prepare→judge→draw→play→discard→end），产生 PhaseStart/PhaseEnd/TurnStart/TurnEnd 事件 | 无技能，回合能流转、阶段能推进、额外回合/跳阶段可插 |
| P2 ✅ | 事件生产 | 弃牌/使用牌/打出牌/死亡/获得牌 → **真实 emit**（不是测试手推 `g.events.push`） | 每条结算路径自动产事件 |
| P3 ✅ | 交互层 | pending 选目标/选牌/多选/二选一，接 advance 全流程（提交→重校验→promote） | 无技能，出杀选目标、濒死求桃、无懈链可交互 |
| P4 ✅ | 完整卡牌结算 | 杀/闪/桃/酒 + 普通锦囊（无中/拆/顺/决斗/南蛮/万箭/五谷/桃园/火攻/借刀/铁索）+ 延时锦囊（乐/兵/闪电 + 判定区 LIFO + 无懈链） | 每张牌可完整结算到弃牌堆/装备区/判定区 |
| P5 ✅ | 装备结算 | 武器范围 + 特效（青釭/连弩/丈八/方天/古锭/朱雀…）+ 防具（八卦/仁王/藤甲/白银）+ 坐骑距离；特效=特殊技能注入 | 每件装备特效生效 |
| P6 ✅ | 生命周期 | 开局（发身份/选将/初始手牌 4 张；主公只 +1 体力上限）/ 终局（胜负判定）/ 中断 | 5–8 人局可从开局到终局 |

> **2026-11-15 口径更正（用户裁定）**：P1–P6 只是「基础流程」，**不等于内核完备**；
> 对照 `align/mechanics.json` 47 条机制词表时仍有 18 条缺口，据此开始武将批次属严重漂移。
> 已补全并以门禁固化：`apps/scripts/lib/sgs3-mechanics.mjs` + `docs/games/sanguosha/v3-kernel-audit.md` +
> `npm run check:sgs3-kernel`（46/46 锚点，缺一条即红）+ `tests/kernel_mechanics.rs`（25 条）。
> **B0/B1 已写的武将技能必须在「内核完备」后重验，不再按「批次需要时才补机制」推进。**
> B0 的 7 个 handler 已写但依赖 P1–P6（据守依赖 P1 回合驱动、颂威依赖 P2+P3 判定交互、放逐依赖 P3 选目标、
> 不屈依赖 P4 濒死流程、解围依赖 P4 转化牌、奋激/行殇依赖 P2 事件生产），**必须先做实前置项，B0 技能才「被触发、有选择、真结算」**。

## 1. 结论先说

- v2（`crates/engines/sanguosha2`）**不再修补**，作为参考实现保留；
- V3 的目标不是「再写一遍」，而是**先有可机器验证的语义基线，再写实现**：
  v1 作为行为 oracle + spec-kit 契约驱动 + 每批次对拍清零。

## 2. 必须继承的资产（不要重写）

| 资产 | 位置 | 用途 |
|---|---|---|
| 语义宪法（I1 询问完整性 / I2 状态语义 / I3 视图不判业务 + 两条硬约束） | `specs/sanguosha/constitution.md` | V3 的最高约束，逐条转成用例与门禁 |
| 契约门禁 | `specs/sanguosha/contracts/*.json` + `check:spec-conformance` | 规则↔用例锚点↔禁止模式 |
| 约定门禁（C1–C6 声明化） | `contracts/conventions.json` + `check:sgs2-conventions` | 体力单出口/内核去技能名/标记/计数器/出口/kind |
| 775 条裁定↔用例锚点 | `docs/games/sanguosha/align/B*.json` | 技能语义的验收清单（V3 可直接复用） |
| 296 技能注册表快照 | `specs/sanguosha/contracts/skills.json` | V3 技能契约的覆盖校验基线 |
| 口径类用例（V3 必过） | `tests/ask_completeness.rs`、`tests/state_semantics.rs`、`tests/b3.rs::yuce_opt1_must_discard`、`tests/b9.rs::jijiang_*`、`tests/b15.rs::tuotian_ignores_other_players_card_loss`、`tests/b20.rs::yanyu_ignores_other_players_lost_sha` | 把 v2 的坑变成回归 |
| 端侧多段交互形态 | `docs/conventions/interaction-primitives.md` + `apps/web/src/view/sgs/targetPicker.ts` | 选牌 → 多选目标 → 确认；上下界由合法集派生 |
| 数据生成器 | `gen:sgs-engine-data`（160 牌 / 232 将 / 296 技能） | 数据不重写，只重写行为 |

## 3. 里程碑（建议顺序，M0 不可跳过）

### M0 行为基线：v1 作为 oracle（0.5–1 天）
- 从 git `f8a7db3^` 复活 v1 为 **test-only crate**（不注册网关、不上线）；
- 同 seed / 同动作脚本回放：对比 **legal 动作集 / pending 序列 / 状态（hp·手牌数·区域）/ 日志事件**；
- 产出 `docs/games/sanguosha/v3-parity-baseline.md`：v1–v2 差异清单（P0/P1）+ v1 已知缺陷白名单（3 条可直接复用）。

### M1 内核：状态 / 事件 / 交互 / 合法集（最高风险，先做）
按 v2 的 7 类缺陷反推设计，每条先写用例再写实现：
1. **状态模型**：`hp` / `alive` / `dead` 三分显式；事件携带 `turn/phase/lifecycle/by/from` 快照；
2. **交互层**：`Interaction` 描述符（类型：选牌/选目标/多选/分区/确认）驱动端侧多段，
   `legal_actions` 只描述「可选动作 + 合法目标集合」，不把组合摊平成 N 个 id；
3. **询问不变量**：`must_ask` 单出口（I1），承诺步骤 `escape_mandatory` + 提交拒绝；
4. **帧数据归属**：进度数据只能写发起帧（硬约束 B）；
5. **事件归属**：每个订阅声明 `subject: Self/Other/By/From`，handler 首行校验（M105 教训）。

### M2 卡牌结算 + 生命周期
杀/闪/桃/酒/锦囊/装备/判定/无懈链；开局/改盘/终局/跨进程群登记（单一事实源：SQLite 快照，不靠内存）。

### M3 视图 + 端侧
三分区视图契约；端侧多段交互沿用 `targetPicker` 形态；日志文案显示名（武将名优先）。

### M4 技能分批（296 个，契约驱动）
用 `align/B*.json` 的 775 条裁定 + `contracts/skills.json` 覆盖校验；
每批收口 = 差分对拍 + 契约门禁 + 真机抽检，**不接受"形状绿、语义错"**。

### M5 验收
- 与 v1 对拍：P0 差异 0；v1 白名单内的已知缺陷例外；
- 真机：8 人局连续 10 局无 P0（含身份/询问/激将/多段交互/终局重开）；
- 门禁：`check:spec-conformance` + `check:sgs2-conventions`（沿用并扩展）+ 差分常驻。

## 4. 过程要求（针对 v2 的失败方式）

1. **规格先行**：每条语义先在 `specs/sanguosha/` 写契约 + 用例锚点，再写实现；无契约不合并；
2. **删旧引擎前抽契约**：任何"删除参考实现"的动作，前置条件是把它代码注释里的裁定/口径全部搬进 `specs/`；
3. **差分常驻**：v1 oracle 不只是一次性工具，进 CI（每次提交跑核心对拍）；
4. **禁止"报一个修一个"**：每发现一个缺陷，必须归入某类并补该类门禁/扫描，再修实例（M105 就是这样做对的）。

## 5. 复用 vs 重写

| 复用 | 重写 |
|---|---|
| `cards_generated.rs` / `generals_generated.rs`（数据） | 内核：`kernel/{mod,resolve,card_play,flow,pending}`（交互/事件/帧） |
| 视图契约与端侧（`apps/web`、`targetPicker`、`pipeline`） | 296 技能 handler 形态（改为契约驱动 + subject 声明） |
| server / gateway / 存储 / 认证接线 | 生命周期与跨进程状态同步 |
| align 单 / 门禁脚本 / 用例口径 | AI（改为消费 Interaction 描述符） |

## 6. 风险与前置

| 风险 | 对策 |
|---|---|
| v1 复活编译失败（`EnginePlugin` API 漂移） | 只做 test-only crate，必要时在本 crate 内做适配层，不碰主工程 |
| 差分噪音（v1 已知缺陷） | 先产出 v1 缺陷白名单（spike-report §12.2 已列 3 条） |
| 技能量太大（296） | 按 B0–B20 批次推进，每批 8 将、契约先行、对拍清零 |
| 时间 | V3 不做「一次性大爆炸」；M0–M3 内核先跑通 1 个批次（B0）再滚雪球 |

## 7. 进度（2026-11-13）

| 切片 | 状态 | 证据 |
|---|---|---|
| M0 v1 oracle 复活 | ✅ | `v3-parity-baseline.md §1`（987 tests） |
| M1 内核骨架（区/帧/事件/pending） | ✅ | `check:sgs3` 不变量 11 用例 |
| M2 卡牌结算 + 生命周期 + 数值链 | ✅ | `tests/{card_play,combat,flow,gear,tricks}` |
| M3 视图投影 | ✅ | `tests/view_contract` |
| M4.2 B0 技能（7） | 🟡 **已实现 / 未收口**（与 B1 同缺口） | `tests/b0`（7）；解围为 no-op、颂威固定座位 0、放逐无目标选择、不屈简化；据守依赖结束阶段（无阶段驱动） |
| M4.3 事件路由/生命周期/对拍 | ✅（路由机制） | `tests/{route,parity}`；事件**生产者**仍缺（阶段驱动 C1） |
| **M4.2b B1 技能（18）** | 🟡 **已实现 / 未收口** | `tests/b1`（35）+ v1 可比标量 3 项 0 差异；收口缺口见 `v3-parity-baseline.md §7.3`（阶段驱动/可选出口/濒死扩展/脚本对拍/引擎接入/抽检） |
| M5.1 交互闭环 + 通用濒死/死亡 + 武将名录 | ✅（框架切片） | `tests/m5`（9）、`tests/generals`（4）、`v3-parity-baseline.md §7` |
| **B0 前置 P1 回合驱动** | ✅ | `kernel/turn.rs` 阶段机 + `tests/turn_flow.rs`（12）；跳阶段/额外回合/翻面/轮数 |
| **B0 前置 P2 事件生产** | ✅ | `flow.rs`/`card_play.rs` 全部走 `Game::push_event`（含 `Game.log` 历史）；`tests/full_game.rs` 事件清单 |
| **B0 前置 P3 交互层** | ✅ | `pending` + `resolve::submit`（选目标/选牌/多选+确定/二选一）；`Step::NeedWith` 保存中间状态；`tests/interact.rs`（11） |
| **B0 前置 P4 完整卡牌结算** | ✅ | `cards::Effect` 分派 + 无懈链 + 判定区 LIFO + 濒死/酒自救；`tests/cards_full.rs`（21） |
| **B0 前置 P5 装备结算** | ✅ | 12 武器特效 + 4 防具 + 坐骑距离（`gear::` 单入口）；`tests/equip_effects.rs`（19） |
| **B0 前置 P6 生命周期** | ✅ | 开局（身份/选将/发牌）/ 终局 / 中断 + 5–8 人自动对局；`tests/full_game.rs`（5） |
| M5.2 濒死扩展（酒/技能救援/完杀）+ 触发类 pass 出口 | 🟡 | 酒自救/酒杀已通（P4）；技能救援与完杀随 B0 技能 |
| **端侧接入（`game_id=sanguosha` 接管）** | ✅ | `engine/{mod,lobby,view,text,bot}.rs` + `tests/engine_plugin.rs`（6）；MCP/server 注册 V3；ADR-0043 |
| **B0 前置项收尾（第二轮审计）** | ✅ | 内奸独胜（用户口径：仅主+内奸存活杀主）/ 桃终局房规 / 兵粮距离 / 铁索重铸 / 装备替换事件 / 死者回合结束 / 五谷落盘事件；`tests/prereq_gaps.rs`（7）+ 7 人局端到端 |
| **回放工具 V3 感知** | ✅ | `sgs-replay` 按快照形态分派 v3/v2 + v3 digest（单测）；道具中台仍读 v2（待口径确认） |
| **B0 技能（7/7 ✅）** | ✅ | 据守/解围/不屈/奋激/行殇/放逐/颂威 全部按 `align/B0.json` 收口并挂载；`tests/{b0_real,b0_batch,b0}.rs`（22）；挂载开关 `skills::ENABLED_SKILLS` |
| **B5 收口（6 将 / 13 技能 · B0–B5 收官）** | ✅ 2026-11-15 | `skills/b5.rs` + `tests/b5.rs`(13)；受害者伤害钩子（骄矜）/濒死进入钩子（兴衰）/千驹距离；注册表 77 |
| **B6 收口（7 将 / 12 技能）** | ✅ 2026-11-16 | `skills/b6.rs` + `tests/b6.rs`(20)；`relay` 代出接力 / `CardAs` 替代语义 / 饰非替代响应 / 诏缚攻击范围 / 急攻赋值手牌上限；注册表 89 |
| **B7 收口（7 将 / 12 技能）** | ✅ 2026-11-16 | `skills/b7.rs` + `tests/b7.rs`(20)；死亡钩子（宗祚）/链祸传导加伤/腹鳞获得记账/督粮摸牌加成/贿生伤害前替代/滔乱转化；注册表 101 |
| **B8 收口（8 将 / 17 技能 + 4 弦技）** | ✅ 2026-11-16 | `skills/b8.rs` + `tests/b8.rs`(22)；`ShaTargeted` 目标窗口（谏征/专对）/天辩拼点钩子/绝响保护/福绵使用前声明/复难事实链；注册表 122 |
| **B9 收口（界标·蜀 7 将 / 14 技能）** | ✅ 2026-11-16 | `skills/b9.rs` + `tests/b9.rs`(19)；非锁定技失效抑制 / 替代链（武圣/龙胆）/ 激将接力+摸牌 / 奇才防拆 / 观星；`b2::jizhi` 升级界版；注册表 135 |
| **B10 收口（界标·吴 8 将 / 17 技能 · B5–B10 收官）** | ✅ 2026-11-16 | `skills/b10.rs` + `tests/b10.rs`(18)；多目标无效/跳弃牌/枭姬失去装备/连营失去全部手牌/博图额外回合/诈降红杀/救援重定向/装备承载转化；注册表 152 |
| **B6–B10 缺口收口（12 条）** | ✅ 2026-11-16 | `v3-parity-baseline §7.14`；响应替代统一入口（滔乱三场景/激将打出接力/饰非借刀）+ 攻心 + 救援询问 + 连营批次 + 谦逊判定 + 观星任意序 + 铁骑逐目标 + 讥谀花色封锁 + 天辩无手牌；`tests/gaps.rs`(9)；注册表 153 |
| **通博交互区 + 复难plus** | ✅ 2026-11-16 | `ui-spec §10` + `v3-parity-baseline §7.15`；引擎两段式（`tongbo_swap` 交换 / 强制交出）、端侧 `tongbo.ts` 两列拖拽、复难plus 显示名；注意装备区不参与 |
| **B4 收口（8 将 / 11 技能）** | ✅ 2026-11-15 | `skills/b4.rs` + `tests/b4.rs`(11)；`Flipped`/`chain_enter`/`damage_bonus`/他人侧入口；注册表 64 |
| **B3 收口（5 将 / 9 技能 + 授予排异）** | ✅ 2026-11-15 | `skills/b3.rs` + `tests/b3.rs`(10)；`Hook::DamageBefore`（制蛮）；权计动态手牌上限；注册表 53 |
| **B2 收口（8 将 / 15 技能 + 授予 2 + 临时 1）** | ✅ 2026-11-15 | `skills/b2.rs` + `tests/b2.rs`(20)；`Hook::UsePre`/`use_targets`/`revoke` 单出口；破势按原文补齐；`v3-parity-baseline §7.5` |
| **B1 收口（18 技能）** | ✅ 2026-11-15 | 8 个 handler 重写为真实待办（不再 `cands.first()` 代选）；`tests/b1_real.rs`(7)+`b1_real_more.rs`(9)；`ENABLED_SKILLS` 挂载 18 个 code；`v3-parity-baseline §7.4` |
| **K 内核机制完备（46 条词表）** | ✅ 2026-11-15 | `v3-kernel-audit.md`（46/46）+ `tests/kernel_mechanics.rs`（25）+ `check:sgs3-kernel` 门禁；补 18 条缺口（pindian / virtual-card / hand-freeze / armor-points / reveal / no-wuxie / target-extra / multi-response / deck-carrier / aoe-target-trim / sha-intercept / judge-reversal / range-override / virtual-equip / equip-move / limit-skill / dying-aftermath / hidden-declaration） |
| **B0–B10 完整度审计（2026-11-16）** | ✅ | 145 技能全部「已注册 ∩ 已收口 ∩ 有断言用例」；修 `gongxin` 未挂载；新增 `check:sgs3-registry`（align ↔ 注册表 ↔ ENABLED ↔ tests/bN） |
| **B11 收口（界标·魏 8 将 / 14 技能）** | ✅ 2026-11-16 | `skills/b11.rs` + `tests/b11.rs`(26)；**新增机制 `judge-rewrite`**（`kernel/quick_judge.rs`：鬼才改判 → 生效 → 天妒，覆盖延时锦囊/八卦阵/不屈/铁骑/恢拓）；摸牌阶段替代（突袭/裸衣/恂恂）；`relay` 增加 `kind`（护驾代出【闪】）；注册表 167 |
| **B12 收口（界标·群 4 将 / 8 技能）** | ✅ 2026-11-16 | `skills/b12.rs` + `tests/b12.rs`(17)；急救（濒死转化）/无双（`response_need`）/义从（双向距离）/离间（无懈跳过）；注册表 175 |
| **B13 口径裁定 + 化身池机制（2026-11-16）** | ✅ | **ADR-0044**（Q1 已注册 / Q2 排除锁定·觉醒·主公·授予技 + 非同场武将/技能 + 不重复化身牌 / Q4 动态池 / Q5 蛊惑可声明无懈且影响链含杀闪桃酒无懈 / Q6 缠怨=所有技能失效·独立闸门）；`init_huashen` 重写 + `skills::{huashen_skills_of,huashen_generals,huashen_choices}` 单出口 + `huashen_set_current`；`tests/kernel_mechanics.rs` 5 条 |
| **B13 收口（J1 8 将 / 16 技能 + 授予技 荐言/思蜀）** | ✅ 2026-11-16 | 门禁新增 `V3_IN_PROGRESS_BATCHES`（进行中批次允许部分注册，但已注册必须收口+有用例，并输出 ⏳ 计数）；`skills/b13.rs` + `tests/b13.rs`（29）；judge-rewrite 泛化（`Hook::JudgeRewrite`）/ `on_judge_effective` / 蛊惑 5 窗口 / 缠怨闸门（Q6）/ 化身池 Q7–Q9；注册表 193 |
| **B14 收口（J2 8 将 / 20 技能）** | ✅ 2026-11-16 | `skills/b14.rs` + `tests/b14.rs`（19）；K1'（虚拟红桃 + `check:sgs3-suit`）/K2 藏拙计数出口/K3 看破无懈不可响应/K4 重铸算使用；注册表 212 |
| **B15 收口（J3 8 将 / 19 技能 + 授予技 急袭）** | ✅ 2026-11-16 | 已收口 肉林/崩坏/酒池/暴虐/挑衅/志继（`tests/b15.rs` 7 条）；`skills/b15.rs`（19 技能）+ `tests/b15.rs`（22）；K5–K12 内核前置先落（§7.23）；注册表 232 |
| **B15 内核前置 K5–K12 落地（2026-11-16）** | ✅ | `v3-parity-baseline §7.23`：AOE 目标排除（顺带修既有缺陷）/ `on_card_finished` / 实例级目标拒绝（帷幕）/ 回合内全防伤+摸2×/ 完杀求桃限制+濒死期失效闸门 / 任意区域移牌 / `flow::swap_hands` / 拼点随机牌 / 屯田距离 / 天义限制与加成；全量回归绿 |
| **B15 开工前置（2026-11-16）** | 🟡 | 8 项内核前置已登记 `v3-parity-baseline §7.21`（K5 AOE 目标排除/K6 回合内伤害防止+摸/K7 濒死可用者过滤+完杀闸门/K8 阶段跳过+任意区域移牌/K9 回礼者跨回合/K10 手牌交换/K11 乱武强制序列/K12 屯田·酣战·长标组合） |
| **下一片** | ⬜ | B16（J4：8 将 / 14 技能）→ B17–B20；真机抽检 R1–R7 待人工 |
| B2–B20 批量迁移 | ⬜ | — |
