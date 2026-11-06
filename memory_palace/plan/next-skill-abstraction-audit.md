---
session_id: next-session
date: 2026-11-05
status: 待启动（B2 走查暂停，先做架构评估）
baseline_commit: 见 git log（2026-11-05 全武将验证会话结束点）
---

# 任务：技能/标记/虚拟牌 抽象与解耦评估（B2 走查前置）

> **给新会话的开工说明**：这是独立任务文档。新会话直接说「做技能抽象评估」，从 §3 开始。
> 背景：2026-11-05 一整天都在 B2（8 将）修缺陷，修一个冒一个，根因是**设计层面的强耦合**（见 §2）。
> 用户要求：**先评估影响范围并重构，再继续批量验证**，否则 148 将走查推不动。

## 1. 本会话已落地的修复（12 项，全部带回归）

| # | 问题 | 修复 | 回归 |
|---|---|---|---|
| 1 | 拒战（阴）获得牌后误弃牌 | `hand_cap` 只按持有上限；手牌上限只在弃牌阶段生效 | `p107::juzhan_yin_gain_does_not_ask_discard` 等 4 条 |
| 2 | 溃诛给自己补牌后二次弃牌 | 弃牌阶段只按上限弃一次（`discard_limit_satisfied`） | `p107::kuizhu_self_draw_does_not_ask_second_discard` |
| 3 | 濒死被救起后吞「受到伤害后」钩子 | 抽 `damage_after` + `Frame::DamageAfter`（压在 Dying 之下） | `p107` 4 条（审时阴/遗计/反馈/节命） |
| 4 | 急救：红【酒】当【桃】救他人被拒 | 「当桃的转化」优先于酒自救判定 | `p42::jijiu_red_jiu_can_rescue_others_as_tao` |
| 5 | 箜声结束阶段「不使用装备」→ 牌随帧消失 | 装备留在「箜声」标记（不再弃置） | `p79::kongsheng_skip_...keeps_cards_in_marks` |
| 6 | 箜声标记全是装备仍询问/触发良姻 | 全装备 → 什么都不发生 | `p79::kongsheng_all_equip_end_phase_does_nothing` |
| 7 | 箜声结束阶段时序（①取回→②良姻→③回血→④使用装备） | 帧内 Return 段 + `kongsheng_use_queue` 排队 | `p79::kongsheng_end_phase_order_liangyin_before_equip_use` |
| 8 | 明任替换死循环 | handler/swap 各自 pop 帧 | `p78::mingren_swap_ends_after_one_replacement` |
| 9 | 明任摸 2 且开局摸不到牌 | 摸 1 + 调用点移到发牌之后 | `sandbox::sandbox_mingren_draws_one_after_deal` |
| 10 | 罪论排序（可选牌序）+ 重复 id 校验 | `ZuiLunStage::Order` + 排列校验 | `p78::zuilun_one_condition_orders_remaining_two` 等 2 条 |
| 11 | 破势觉醒不废除装备栏 | 觉醒废除全部栏位 + 栏内装备进弃牌堆 | `p78::poshi_awaken_abolishes_all_slots` |
| 12 | 守恒统计漏算「结算帧持有的牌」（误报缺牌） | `CardCounts.in_frames` + `cards_in_frames()` | `p79::conservation_counts_cards_held_in_frames` |

另有 UI/文档类修复：转换技阴/阳状态公开下发（贞良漏项）、武将详情展示、目标浮层补 `pass` 出口、
授予技（怀柔/清侧）文本补进 `skill-text.ts` 生成器、批次走查单生成器 `sgs-audit-sheet.mjs`、`--no-scene` 开局技机检。

## 2. 暴露的设计缺陷（本会话反复踩坑的根因）

### 2.1 技能触发「就地压帧」与父帧 pop 冲突（命中 5 次）
- 现象：技能在别人的结算过程中 `push(Frame)`，父帧随后 `pop`，把新帧弹掉 → **技能永不触发 / 帧内牌丢失**。
- 本会话命中：明任替换、箜声（两处）、良姻（回血）、贞良（阴）。
- 现状对策：各自加「栈空队列」（`zhenliang_yin_queue` / `liangyin_heal_queue` / `kongsheng_use_queue`）。
- **缺陷**：这是逐点打补丁，没有统一「延后触发」机制 → 新增技能必然再踩。

### 2.2 帧里暂存牌 = 隐形丢牌源
- 现象：牌从区域取出、暂存在 `XxxFrame.cards`，若帧被丢弃/替换，牌从所有区域消失。
- 已修：箜声三处 + 守恒统计（`in_frames`）。
- **缺陷**：没有「帧持牌必须落盘」的强制约束（无类型层面的 owner 概念）。

### 2.3 「移出/移入游戏」原先 7 处硬编码（本次已抽象为事件）
- 旧：`屯田/权计/箜声×3/to_void_card/from_void_card` 各自手写 `liangyin_trigger`。
- 新：`skills::add_mark` / `remove_mark` / `turn::to_void_card` / `from_void_card` 统一上报
  `state::CardMoveEvent`，`advance` 在栈空派发 → 良姻订阅。**技能不再硬编码触发点**。
- **仍未覆盖**：延时锦囊（小乔♦当乐不思蜀、徐晃黑牌当兵粮寸断）的「移出/移入」——
  另：事件全局产生，**技能需自行按「生效窗口」过滤**（良姻/箜声已加「仅周妃回合内」门闩，复验 35）。
  判定区（`judge_zone`）的进入/离开没有统一钩子（散落在 `resolve.rs:2389/2399/3532`、`resolve_fire.rs:948`、
  `resolve_forest.rs:531`、`resolve_wind.rs:834/840`、`resolve_yijiang5.rs:722`、`resolve_sp2.rs:2188`…）。
  对良姻而言可达场景是**周妃自己判定阶段**延时锦囊判定后进弃牌堆（=移入）。

### 2.4 `pending` 缺统一「放弃出口」
- 现象：`target`/`choice` 类 pending 有时没有 `pass` → 客户端只能去点「结束出牌阶段」等不相关按钮 → 动作非法报错。
- 已修个案（良姻回血、贞良阴、箜声装备）。
- **缺陷**：pending 生成点分散（~50 处 `Pending { .. }`），没有「必须有放弃出口」的构造器/门禁。

### 2.5 视图白名单硬编码
- 转换技清单硬编码 → 漏 `zhenliang`（已改为 `skills::TRANSFORM_SKILLS` 单一事实源）。
- 同类风险：`view.rs` 里其它「技能清单」式白名单、Web 端 `optionButtons` 前缀白名单（漏前缀 = 浮层空列表）。

### 2.6 技能文本来自官网生成物，授予技缺失
- `skill-text.ts` 由 `heroes.json` 生成（只含武将自带技能）→ 怀柔/清侧 无描述。
- 已加 `GRANTED_SKILL_TEXT` 补丁表，但**机制上仍会漏**（新授予技必须人工补）。

### 2.7 虚拟牌/转化牌 无统一抽象
- 现状：`virtual_defs`（国色）、`skills::conversion_for`（武圣/龙胆/急救/酒池…）、各技能自建
  `skill:<code>:<card>` 动作 —— 三套并行。
- 与「移出/移入」「使用/打出」「响应」组合时需要穷举，难以证明覆盖完整。
- 用户 2026-11-05 指出的漏项：小乔方块牌当【乐不思蜀】、徐晃黑牌当【兵粮寸断】→ 属于虚拟牌 + 判定区 双重耦合。

### 2.8 结算顺序靠人记
- 箜声必须 ①取回→②良姻→③回血→④使用装备；良姻回血必须在奋激之后；奋激在栈空统一问。
- 现状：写在对应用例注释与 `rules-rulings.md`，没有「时序表」进代码结构。
- 建议：多阶段技能在 `skills.md` 里一张时序表 + 每个时序点一条回归（本会话对箜声已这样做）。

## 3. 新会话要做的评估（建议顺序）

1. **清点**：全库扫描
   - `push(Frame::` 出现在「父帧结算中途」的位置（潜在 2.1 类风险）；
   - `Frame` 里持有 `Vec<String>` 牌的位置（2.2 类）；
   - `Pending {` 生成点（2.4 类，检查是否都有 `pass`/`cancel`）；
   - `judge_zone` 进入/离开点（2.3 漏项）；
   - 视图/前端白名单（2.5 类）。
2. **设计统一机制**（建议）
   - `Trigger` 注册表：`on_card_move_out/in`、`on_gain`、`on_damage_after` 等事件 → 技能订阅，**统一「延迟到栈空派发」**；
   - `Deferred` 队列合并成一个（现在是 4 个队列：fenji/zhenliang/heal/kongsheng）并定义**固定派发顺序**；
   - `VCard`（虚拟牌）抽象：`def/suit/rank/source + origin cards`，统一 `use/respond` 判定；
   - `Pending` 构造器强制 `pass`（或 `PendingBuilder`），并加门禁脚本扫描缺少出口的 pending；
   - 视图白名单由注册表生成（技能 `kind` 驱动），不再手写。
3. **回归基线**：每步重构都保持 `cargo test -p sanguosha`（当前 981）+ `sgs:check:fast` + `check:sgs-ai:100` 全绿。
4. **再回 B2**：重构完成后从 周妃（良姻/箜声）、卢植（明任/贞良）、陆抗（决堰/破势/怀柔）、诸葛瞻（罪论）复测。

## 4. B2 当前状态（暂停点）

- 机检：`--batch B2 --repeat 2` 与 `--no-scene` 均 **失败 0**（触达 6/8 将）。
- 人工走查：卢植 明任/贞良（阴已通）、周妃 良姻/箜声（本次修完待复验）、诸葛瞻 罪论（排序已通）、
  陆抗 决堰/破势/怀柔（破势+怀柔已修）、毌丘俭 征荣（口径确认：手牌数条件未满足，非 bug）、郝昭/陈到/张绣 未走查。
- 结论：**B2 不签字**，等重构后统一复验。

## 5. 相关文件

- `docs/games/sanguosha/rules-rulings.md`（复验 18–34：本会话全部裁定）
- `docs/games/sanguosha/skills.md`（良姻/箜声/罪论/明任/破势 描述已更新）
- `memory_palace/plan/next-general-audit.md`（148 将验证计划，B2 待续）
- `crates/engines/sanguosha/src/{state,skills,turn,resolve,resolve_sp2,resolve_more}.rs`（事件/队列/帧）
