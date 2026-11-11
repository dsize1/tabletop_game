# ADR-0039：事件「发生时刻/来源」快照 + 开局生命周期 + 主体窗口

- **状态**：Accepted（2026-11-11，引擎自检后实施；用户裁定「机制优先，禁止 per-skill 接线」）
- **相关**：ADR-0016（事件总线）、ADR-0038（规格即数据）、`docs/games/sanguosha/engine-self-audit.md`、
  `docs/games/sanguosha/rules-rulings.md` §44（奋激口径 A→B）
- **结论一句话**：事件在**入队时**打上 `turn/phase/lifecycle` 与 `by/from` 快照；
  窗口判定与 handler 只读快照；开局/发牌属于 `Lifecycle::Setup` 不触发技能；
  「主体窗口」由注册表声明（`SkillDef.subject_window`），不在 handler 里写接线。

## 背景（为什么要改内核而不是修技能）

2026-11-11 技能自检确认了三类**同源**缺陷：

1. 周泰·奋激在**开局发牌**时被询问（发牌被当成「获得」）。
2. 界夏侯惇·清俭在**摸牌阶段摸牌**后误触发（事件在摸牌阶段摸完、`begin_phase(Play)` 之后才路由）。
3. 诸葛瞻·罪论在**下一回合准备阶段**才结算，且 `turn.damage_by/discard_by` 已被清空（条件恒假、`turn.zuilun_done` 隔回合失效）。

共同根因：`Event` 只有 `kind/seat/card/cause/amount`；路由（`Sub::accepts`、handler）读的是**路由时**的
live `g.turn/g.phase`；发牌与对局动作共用同一条事件通道；牌移动没有「谁造成/从哪来」的字段。

2026-11-11 用户裁定青任：**禁止**用 `if g.turn == seat`、清空事件队列、按技能名白名单等接线式补丁；
必须补齐机制，后续还会上线更多武将。

## 决策

1. **事件快照**：`Event { turn, phase, lifecycle, by, from }`，全部 `#[serde(default)]` 兼容旧快照。
   `Game::emit(ev)` 是**唯一**入队出口（打点 `turn/phase/lifecycle`）；`events` 仍保留但约定只经 `emit`。
2. **窗口只读快照**：`Sub::accepts(ev)` 用 `ev.turn/ev.phase`；`Payload::Trigger` 透传快照；
   handler 从 `Ask.data["turn"]/["phase"]` 读「发生时刻」，不读 live 状态。
3. **生命周期**：`Lifecycle::Setup` 期间的任何事件不路由给技能；`finish_setup` 全程为 Setup，
   `begin_game_at` 切回 Turn（替代 `events.clear()` 补丁）。发牌/组牌堆都不再算「获得/移出游戏」。
4. **牌移动来源**：`Game.action_actor`（`begin_action_as`，由 `resolve::submit` / `card_play::start_use` 设置）；
   `to_discard` 写入 `Event.by/from`，`gain` 写入 `Event.from`；提供 `to_discard_by` 供技能/测试显式指定施动者。
5. **主体窗口**：`SkillDef.subject_window: Option<Window>`，路由层用事件快照判定「事件主体」是否在窗口内
   （如奋激的旧口径曾是 `OtherTurn`；**B 口径**下改用 `by/from`，不再使用该字段）。
6. **测试**：所有构造事件的地方改为 `g.emit(...)`；新增用例覆盖「事件时刻」「Setup 不触发」「来源判定」。

## 被否方案

| 方案 | 否决原因 |
|---|---|
| 每个误触发技能加 `if g.turn == seat` / `cause == ...` | 接线式补丁；下一个武将仍会踩；用户明令禁止 |
| `events.clear()` 清空发牌事件 | 只治发牌；摸牌阶段摸牌仍误触发（清俭实证） |
| 按技能名/白名单枚举「哪些算被其他角色弃置」 | 与 ADR-0038「规格即数据」冲突，且必然漏接 |
| 把「回合外」继续当窗口 | 与官方原文/新裁定不符（B 口径按来源，不按回合） |

## 影响与迁移

- 旧快照：新增字段 `serde(default)`，回放可读；`lifecycle` 缺省 `Turn`（不改变旧行为）。
- 16 个读 live `g.turn/g.phase` 的触发 handler：先由快照提供数据，按批迁移（本批已迁 fenji/qingjian；
  剩余清单见 `engine-self-audit.md` §2）。
- 阶段结束类技能（罪论/兴学/寝情）仍存在**结算滞后到下一回合**的问题：快照让判定正确，但 `turn.*` 已被清空。
  M12 的 B 方案（延迟切 turn）实测导致自对弈 `stalled`（seed=1 game 0），已回滚为 A；待改「PhaseEnd 携带 turn.* 计数快照」实现。
- 门禁：`gate_kernel_has_no_skill_branch` 仍覆盖 B0 七技；`tests/spike.rs` 等改用 `emit` 后通过。
- 回归：`cargo test -p sanguosha2` 全绿（含新增 5 条奋激用例）。

## 验证状态

- `cargo test -p sanguosha2` 全绿（本机实测）。
- 沙盒/147 将机检待重跑（本轮先修工具链，随后复跑）。
- 未完成：M12（PhaseEnd 时序）、M13 之外的「牌移动来源」覆盖面（技能内部 `move_card` 到非弃置区不计 `by`）。
