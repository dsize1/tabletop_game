# 三国杀批次开发 playbook（P4.4.2 harness）

> 目的：把「一将成名 Y2–Y7 / 界限突破余量」每批 8 将的重复劳动固化，减少漏项。
> 适用于 `docs/games/sanguosha/development-plan.md` 的批次表；模式来自批次 1–6 与 Y1（2026-10-30/31）。

## 1. 批次前必读

| 文档 | 用途 |
|---|---|
| `skills.md` §2.7.x | 已实现技能表（本批要续号） |
| `rules-rulings.md` §16/§21/§23 | 已有裁定口径（自动 vs 可选） |
| `development-plan.md` P4.4.2 | 批次表 + 已知边界 |
| `state-machine.md` / `legal-actions.md` / `interaction-primitives.md` | 涉及新交互原语时才需要 |
| `docs/conventions/coding-standards.md` | 编码与提交规范 |

数据已在 `data.rs` 全量生成（一将成名 72 位 / 界限突破 87 位），**不需要重跑生成脚本**；未实现的武将 `general_ready=false` 自动不进池。

> **批次类型**：一将成名（`Y*`，已 72/72 完成）与界限突破（`J*`，剩余 60 位）共用本流程。
> 界限突破批次额外注意：同名技能**复用 code**（ADR-0024），公共逻辑用 `skills::is_jiexian(st, seat)` 分支；不要改动标准版行为（回归 `tests/p46`–`p55`）。
> 取下一批：`npm run sgs:batch-scaffold -- --pack jiexian`。

## 2. 一批 8 将的标准流程

1. **开工脚手架**：`npm run sgs:batch-scaffold`（自动取下一批；也可 `--batch Y4` / `--gids 187,188 --module resolve_yijiang5 --tests p91`）→ 打印武将元数据、`skills.rs` 常量/match 模板、`skills.md` 表行、用例/文档/门禁清单；技能描述再查 `data/heroes.json`。
2. **注册技能**：`crates/engines/sanguosha/src/skills.rs`
   - 每位武将一个 `const XXX: &[SkillDef]`（`code/name/kind/implemented:true`）；
   - `match general { "sgs_hero_<gid>" => XXX, ... }`；
   - 觉醒/限定授予技能进 `EXTRA_SKILLS`（docs 同步 §4.1）。
3. **实现帧**：新文件 `resolve_yijiangN.rs`（按批次续号；也可按技能主题命名）。
   - `Frame` 变体 + `step_*` + `act_*`；
   - `resolve.rs` 三处注册：`Frame` 枚举、`step_frame` 分派、`execute_pending` 的 `skill` 分派；
   - 需要「使用/受伤/阶段」时机时，在 `turn.rs` / `resolve.rs::apply_damage_inner` 加钩子。
4. **用例**：`tests/p8N.rs`（Y1 用了 `p89`），**每将 ≥3 条**，覆盖：技能生效 / 边界不触发 / 与既有机制的交互。
5. **计数**：`tests/p67.rs` 的「一将成名已实现 N 位」+8（`npm run sgs:batch-status` 会校验）。
6. **文档**：`skills.md` 新 `§2.7.x` 表；`rules-rulings.md` 新一节；`development-plan.md` 批次记录（含已知边界）；`milestones.md` 新 M 条目。
7. **门禁 + 提交**：`npm run sgs:check`（必要时 `sgs:check:full`）全绿后提交，一条 commit 一批。

## 3. 代码模板

### 3.1 技能注册（skills.rs）

```rust
const ZHAOYUN2: &[SkillDef] = &[
    SkillDef { code: "longdan2", name: "龙胆", kind: SkillType::Normal, implemented: true },
    SkillDef { code: "yajiang", name: "涯角", kind: SkillType::Normal, implemented: true },
];
// match general { ... }
"sgs_hero_999" => ZHAOYUN2,
```

### 3.2 帧骨架（resolve_yijiangN.rs）

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DemoStage {
    Ask,
    Pick,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DemoFrame {
    pub actor: usize,
    pub stage: DemoStage,
}

pub(crate) fn step_demo(st: &mut SgsState, f: DemoFrame) -> Result<bool, EngineError> {
    // 1) 组装 options（可选目标/手牌/弃牌堆亮牌……）
    // 2) 无候选 → pop(st); return Ok(false)
    // 3) st.pending = Some(Pending { kind:"choice", actor: pid, skill: Some("demo"), ... })
    // 4) replace_top(st, Frame::Demo(f)); Ok(true)
}

pub(crate) fn act_demo(st: &mut SgsState, action: &str) -> Result<String, EngineError> {
    // 按 f.stage 分支；用 crate::illegal(...) 校验非法输入
    // 结束用 pop(st)，多阶段用 replace_top(...)
}
```

`resolve.rs` 注册三处：

```rust
// ① Frame 枚举
Demo(crate::resolve_yijiang4::DemoFrame),
// ② step_frame
Frame::Demo(f) => crate::resolve_yijiang4::step_demo(st, f),
// ③ execute_pending
Some("demo") => crate::resolve_yijiang4::act_demo(st, action)?,
```

### 3.3 用例骨架（tests/p8N.rs）

```rust
use super::p24::{set_general, start_and_pick};
use super::support::*; // other_seats / give_card / give_def / give_suit / give_rank / give_equip / set_top / find_cards / run_phase / pending_skill / pending_options_with
use super::{clear_hands, edit, manager, place_mark, state_of, take_card};
use crate::state::*;

#[test]
fn demo_triggers_and_applies() {
    let mgr = manager(2_000); // 每批取一段不冲突的种子区间
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut other, mut hp) = (0, String::new(), 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_999");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        other = other_seats(st, seat)[0];
        set_general(st, other, "sgs_hero_1");
        hp = st.seats[other].hp;
        crate::resolve::apply_damage(st, other, 1, Some(seat), crate::resolve::DamageType::Normal, None);
        let mut ev = Vec::new();
        crate::resolve::advance(st, &mut ev).unwrap();
    });
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("demo"));
    mgr.execute_action(&room, &pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[other].hp, hp - 1);
}
```

要点：
- 阶段类技能用 `run_phase(&mgr, &room, seat, TurnPhase::Prepare, true)`（内部已 `phase_step + advance + restore`）。
- 协议触发（`play:` / `skill:`）直接用 `mgr.execute_action`，`advance` 已包含。
- 牌数守恒由 `edit()` 内的 `check_conservation()` 校验：帧内临时亮牌请放 `deck.processing`，别让牌脱离区域。

### 3.4 文档模板

```md
<!-- skills.md -->
## 2.7.3 一将成名（批次 Y2：8 将 / N 技能）

| 武将 | 技能 | code | 类型 | 实现要点 |
|---|---|---|---|---|
| 刘封 `sgs_hero_179` | 陷嗣 | `xiansi` | 普通 | ……（自动/可选口径写清楚） |
```

```md
<!-- rules-rulings.md -->
## 24. P4.4.2 一将成名批次 Y2 裁定（YYYY-MM-DD）

- **技能**：一句规则 + 引擎口径（可选按自动、目标如何枚举、牌去哪里）。
```

```md
<!-- development-plan.md，插在上一批记录之后 -->
**P4.4.2 批次 Y2（一将成名 8 将：179–186，已完成 YYYY-MM-DD）**：
- 机制：…（新 Frame/计数器/标记）
- 技能：…
- 用例：`tests/p90.rs` 24 条；`cargo test -p sanguosha` NNN 全绿；`p67` 一将成名已实现数更新为 32。
- 已知边界：…
```

## 4. 门禁

| 命令 | 内容 | 何时用 |
|---|---|---|
| `npm run sgs:check:fast` | Rust 单测 + skill-cases/skill-docs/engine-data/batch-status/pack-pool/view-contract/interaction | 改代码时快速自检 |
| `npm run sgs:check` | 上面 + `test:web` + `test:sgs-web` | **每批收尾必跑** |
| `npm run sgs:check:full` | 上面 + MCP 冒烟 `verify:sanguosha` | 批次合并/里程碑收尾 |
| `npm run sgs:batch-status` | 各包进度、批次表完成度、p67 计数一致性、下一批名单 | 开工前/提交前 |
| `npm run sgs:batch-scaffold` | 下一批开工脚手架（`--pack jiexian` 取界限突破） | 批次开工第一步 |

单项命令仍可单独跑：`check:sgs-skill-cases`、`check:sgs-skill-docs`、`check:sgs-engine-data`、`check:sgs-pack-pool`、`check:sgs-view-contract`、`check:interaction-primitives`、`test:sgs-web`、`verify:sanguosha`。

## 5. 常见坑

- **`pending.actor` 必须是当前操作者**：多阶段帧中途切换操作者（如峻刑的目标二选一、御策的伤害来源）时，`step_*` 里的 `actor` 要按阶段改。
- **`pop` / `replace_top` 顺序**：压入新帧要在 `pop`/`replace_top` 之后（否则会被覆盖），伤害后技能尤其注意。
- **可选技能口径**：仓库既有习惯是「能自动就自动 + 文档写边界」；涉及选牌/选目标时优先给 `pass`/`cancel` 选项，避免死锁。
- **牌数守恒**：临时亮出的牌放 `deck.processing`；武将牌上的牌用 `skills::add_mark`（计入 `marks`），不要直接塞 `hand`。
- **手牌上限**：改上限一律加到 `skills::hand_limit`（视图 `hand_limit` 自动跟随），不要改 `SgsState::hand_limit`。
- **同名/同拼音技能 code**：技能 code 复用见 ADR-0024；不同技能的拼音撞车（如宗室/纵适）另起 code 并在文档登记。
- **p67 计数**：+8 是硬编码断言，`sgs:batch-status --check` 会抓不一致。
- **用例的「覆盖统计」是文本匹配**：`check:sgs-skill-cases` 按「测试体内是否出现 `sgs_hero_<gid>` 或 `"<code>"`」计数，且用例体在下一个**顶层 `fn`** 处截断（2026-10-31 修复）。因此：①批次私有助手放在文件顶部（或在 `support.rs`），不要夹在用例之间；②用例里要显式出现武将 code 或技能 code（如 `assert!(skills::has(&st, seat, "danshou"))`），否则把 `set_general` 全放进助手会导致该将覆盖为 0。
- **`cargo fmt`**：只对自己新增/修改的文件跑 `rustfmt`（仓库既有文件存在未格式化片段，整包 fmt 会产生无关 diff）。
- **用例要抗随机**：牌堆/座次/候选武将都是随机的，不要断言绝对张数或固定座位。常见做法：
  1. 断言历史文本（`st.history.iter().any(|e| e.text.contains("..."))`）；
  2. 断言「相对变化」（`>= before`、`len() > before`）而非精确值；
  3. 需要特定花色/牌名时用 `support::give_def_suit` / `give_rank` / `equip_slot_card`；
  4. 需要指定座位时用 `seats_from_left_of` 或 `find(|i| st.in_attack_range(...))`，不要假定 `other_seats(...)[0]` 一定在攻击范围内。

## 6. 批次完成定义（DoD）

- [ ] 8 将均 `implemented:true`，`general_ready` 为真，选将池可见；
- [ ] 每将 ≥3 条用例（`check:sgs-skill-cases` 通过）；
- [ ] `npm run sgs:check` 全绿（必要时 `--full`）；
- [ ] `skills.md` / `rules-rulings.md` / `development-plan.md` / `milestones.md` 已更新；
- [ ] `npm run sgs:batch-status` 显示本批 ✅ 且下一批名单正确；
- [ ] 单条 commit 提交。
