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

> **验证批次 ≠ 开发批次（P4.4.4）**：开发批次 = 本文件 + `sgs:batch-status`（232 将，含已被界版覆盖的同名原版）；验证批次 = `apps/scripts/lib/sgs-batches.mjs`（**147 将 / B0–B20**，有界版同名的原版 85 位不入批次（去重口径：名首「界」「将」均视为同名））。
> 验证口径与入口：`docs/games/sanguosha/general-audit.md`（人工矩阵）、`npm run check:sgs-generals`（门禁）、`npm run audit:sgs-generals -- <批次>`（机器证据）。

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
| `npm run sgs:check:fast` | Rust 单测 + skill-cases/skill-docs/engine-data/batch-status/pack-presets/pack-pool/view-contract/interaction | 改代码时快速自检 |
| `npm run sgs:check` | 上面 + `test:web` + `test:sgs-web` | **每批收尾必跑** |
| `npm run sgs:check:full` | 上面 + MCP 冒烟 `verify:sanguosha` | 批次合并/里程碑收尾 |
| `npm run sgs:batch-status` | 各包进度、批次表完成度、p67 计数一致性、下一批名单 | 开工前/提交前 |
| `npm run sgs:batch-scaffold` | 下一批开工脚手架（`--pack jiexian` 取界限突破） | 批次开工第一步 |

单项命令仍可单独跑：`check:sgs-skill-cases`、`check:sgs-skill-docs`、`check:sgs-engine-data`、`check:sgs-pack-presets`（预设 ↔ data.rs ↔ data-model）、`check:sgs-pack-pool`、`check:sgs-view-contract`、`check:interaction-primitives`、`test:sgs-web`、`verify:sanguosha`。

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

## 7. 界限突破批次（J*）check-list

> 适用于 `development-plan.md` 的 J1–J8 批次（每批 8 将，J8 为 4 将）。J1（26/304/446–451）为范本。

1. **开工**：`npm run sgs:batch-scaffold -- --pack jiexian`。脚手架会自动列出**同名技能复用 code** 清单（如 `烈弓→liegong`），并直接把这些 code 填进模板；占位符只留给新技能。
2. **同名技能**：全部复用 code + `skills::is_jiexian(st, seat)` 分支，**不改标准版行为**；改完回归 `tests/p46`–`p55` 与 `check:sgs-skill-cases`。
3. **授予技**（觉醒/主公技授予，如 J1 的「荐言 / 思蜀」）：进 `EXTRA_SKILLS` + `skills.md` §4.1 表格 + 授予点（`grant_skill`）在 `resolve_*.rs` 内。
4. **计数**：界版批次**没有** p67 断言；但 `tests/p67.rs::pack_pool_prefers_jiexian_variant` 的 `standard+jiexian` 池断言要按新增界将数同步（J1 27→35）。
5. **用例**：`check:sgs-skill-cases` 对**共享 code** 做去歧义——测试体内出现另一位 code 拥有者的 `sgs_hero_<gid>` 时，code 不再计入。因此每将用例请显式出现自己的 `sgs_hero_<gid>`。
6. **测试助手**（`tests/support.rs`，J1 起）：`find_free_card` / `give_filtered` / `put_on_top` / `put_in_judge_zone` / `pass_until_skill`（无懈窗口连点 pass）/ `stage_play` / `damage_and_advance`。
7. **文档**：`skills.md` 新 `§2.8.x` 表（含本批边界）+ `rules-rulings.md` 新一节 + `development-plan.md` 批次记录与表 ✅ + `milestones.md` 新 M 条目。
8. **门禁**：`npm run sgs:check`（11）/ `sgs:check:full`（12）；`sgs:batch-status` 显示本批 ✅ 且下一批正确；单条 commit。

### 7.1 J1 踩过的坑（供后续批次复用）

- **判定前有无懈窗口**：`run_phase(.., TurnPhase::Judge, ..)` 会先停在 `kind=wuxie` 的 pending；用 `pass_until_skill(mgr, room, "<skill>", 8)` 连点 pass 再断言。
- **准备阶段类 pending**：`run_phase(.., TurnPhase::Prepare, ..)` 可能因「神速/巧变/洛神」等停住；断言阶段类效果时直接调用内部钩子（如 `awaken_triggers`）或在测试内显式消费 pending。
- **MCP 冒烟的死循环**：`apps/verify-sanguosha.mjs` 的准备阶段 pending 驱动器已改为「记录已用动作 + 优先 confirm」，避免多选类 pending 反复 toggle（J3 随机武将下偶发）。
- **界将可能是主公**：`set_general` 会按身份加 1 点体力上限（如界徐庶/界刘禅），断言请用**相对变化**而非绝对值。
- **`SgsState::hand_limit` ≠ `skills::hand_limit`**：技能加成（血裔/权计等）只在后者，断言手牌上限务必用 `skills::hand_limit`。
- **虚拟【杀】/【决斗】**：`ShaFrame{ card: "" }` + `push_sha_target_response`（诛害）；他人视为对你用【决斗】用 `RespondFrame(RespondKind::Duel)`（势斩）。
- **额外回合会在 `advance` 内被消费**：放权结算后断言 `st.turn == target`，不要断言 `extra_turn`。

## 8. P4.4.3 验收 checklist（包扩展收尾）

> 适用：一个新武将包（或一批）全部实现后、对外宣称「包可用」之前。

1. **包开关预设**：新增包后 `lib.rs::general_pack_presets()` 会自动枚举全量组合（`standard` + 其余包全部子集）；
   确认大厅描述人数 = 去重后选将池，且 `npm run check:sgs-pack-presets` 通过。
2. **同名去重**：`prefer_jiexian` 口径（ADR-0024 口径 A）在新包上仍生效——新包与已有包同名时，界优先。
3. **回归用例**（`tests/p67.rs`）：预设↔标签↔人数、大厅预设列表、目标包候选池、旧对局快照固定（旧局不回溯追加包）。
4. **文档同步**（缺一不可）：
   - `data-model.md` §3.7：静态数据表（generals/keys）、默认生成武将数、预设表（mode/标签/人数）、去重口径；
   - `legal-actions.md` §3：`lobby:set_general_packs:<mode>` 行；
   - `skills.md` §2.x/§4：本包已实现数/测试数；
   - `data/README.md`：生成命令口径（全量武将数）；
   - `development-plan.md` 验收记录 + `milestones.md` 新 M 条目。
5. **门禁**：`cargo test -p sanguosha` 全绿；`npm run sgs:check` / `sgs:check:full`（含 MCP 冒烟 + `standard+<新包>` 开关断言）；
   `npm run sgs:batch-status --check` 一致；单条 commit。

> **已执行**：P4.4.3（2026-10-31）界限突破验收，见 `development-plan.md` P4.4.3 记录 / `milestones.md` M56。

## 9. v2 迁移批次（B8+）开工命令

> 适用：B8 起把剩余武将迁到新内核 `crates/engines/sanguosha2`（v1 继续保留到退役签字）。
> 模板见 §3 的 v1 版本**不再适用**；v2 的写法是「声明 + handler + 真实结算入口」。

### 9.1 一批的标准流程

```bash
# 1) 建对齐单骨架（B8 起；6 段由 agent 填，第 6/7 段用户确认）
node apps/scripts/sgs-align-sheet.mjs --batch B8 --new

# 2) 看本批要做什么（武将/技能/已注册状态/待建文件）
#    同时输出「对齐单 status / 疑点未答数 / 机制缺口（MECHANISM_SUPPORT）→ 是否可开工」
npm run sgs2:batch -- B8

# 3) 实现：src/skills/b8.rs + 注册到 src/skills/mod.rs 的 SKILLS
#    牌结算/视为使用牌一律走：
#      Game::resolve_use（真实手牌）/ Game::use_virtual（纯虚拟）/ Game::use_as（转化牌）
#    禁止调用 Game::use_card（事件模拟入口；门禁 tests/m2_wiring.rs 会拦）

# 4) 用例：tests/b8.rs（每技能 ≥1 正例 + ≥1 反例；真实结算后仍不误触发）

# 5) 登记「本批已迁到 v2」：把 8 加入 apps/scripts/check-sgs2-registry.mjs 的 V2_BATCHES

# 6) 回写对齐单第 2 段（现实现）= 新引擎行为 + impl 锚点

# 7) 门禁
npm run check:sgs2          # 单 crate 全量：tests + metrics + registry + decision-log + scenes + ai + parity
npm run sgs:check:fast      # 全仓 23+ 步（含 v1 回归）
```

### 9.2 v2 专有门禁（B8+ 每批都会用到）

| 门禁 | 作用 |
|---|---|
| `npm run check:sgs2-registry` | 注册表唯一性 / 转换技清单 / **对齐单（V2_BATCHES）↔ 注册表精确一致** / 每个 code 必须有 `#[test]` 覆盖 / **对齐单 `tests` 与 `impl` 锚点必须真实存在**（B17 起：`bN::fn` 必须在 `tests/bN.rs`、`sanguosha2::skills::m::CONST` 必须能找到 `pub const`；防手写 typo/改名漏改） / **机制用例锚点**（措辞类走 `MECHANISM_ANCHOR_WAIVED`） / 授予型子技能走 `GRANT_ONLY` 白名单 |
| `npm run check:sgs2-decision-log` | AI 决策日志 schema（`legal` 必须包含 `chosen`、候选 ≤64、step 单调） |
| `npm run check:sgs2-ai` | 10 局自对弈：终局率 / illegal / loop / stalled / pending 覆盖 |
| `npm run check:sgs-parity` | v1↔v2 数据/属性/技能覆盖（迁移期；删 v1 时归档） |
| `npm run check:sgs2-plugin` | `EnginePlugin` 完备性 / 视图脱敏 / legal≡execute |

### 9.2.1 填第 2 段（现实现）的辅助

```bash
# 列 v1 实现锚点（act_/step_ 函数 + 技能 code 出现处），填第 2 段时先看这些文件:行
node apps/scripts/sgs-align-sheet.mjs --batch B9 --v1-anchors
```

### 9.2.2 每批收尾的「harness 自查」（B17 起固化）

| 检查 | 落点 | 说明 |
|---|---|---|
| 技能 code ↔ 名称撞车 | `npm run check:sgs-align` | 同一 code 在名录里对应 ≥2 个不同技能名 → 失败（如 宗室/纵适 曾共用 `zongshi`）；界/标同名复用不受影响。 |
| 对齐单 tests/impl 锚点 | `npm run check:sgs2-registry` | `tests: ["bN::fn"]` / `impl: "sanguosha2::skills::m::CONST"` 必须真实存在。 |
| 牌名硬编码 | `cargo test -p sanguosha2 --test mechanisms gate_no_card_name_comparison` | 文件清单已覆盖全部技能模块 + kernel 流程；历史例外在 `ALLOW` 显式登记，新增代码不得再加。 |
| 单动作批次不变量 | `tests/mechanisms.rs::action_batches_are_cleared_between_actions` / `gain_batch_tracks_cross_seat_gain_not_draw` | `lost_batch`/`equip_lost_batch`/`gain_batch`/`action_seen` 必须由 `begin_action` 清空；摸牌不算「获得」。 |
| 界面标记 | 对齐单第 6 段 + `view::marks_count_view` | 新增纯计数标记（如「巧」）必须进视图投影并在对齐单 UI 段声明；否则 Web/契约看不到。 |

### 9.3 v2 写法要点（与 v1 最大的不同）

- **技能状态只查 `skills::active`**；触发用 `SkillDef.triggers + Window`，不要在流程里写武将名分支。
- **handler 只返回 `Step`**（不就地压/弹帧）；需要询问就 `Step::Need`（pending 必须带出口）。
- **牌结算**：基本牌/锦囊/装备/判定区/无懈链已由 `kernel/card_play.rs` 统一；技能只判属性（`cards::def`）与发事件。
- **伤害/回血**：`flow::damage*` / `flow::lose_hp` / `flow::heal`；禁止直接改 `hp`。
- **新增状态**：必须进 `check_conservation` / `check_no_orphan_frames` 覆盖范围（必要时补不变量用例）。
- 对齐单第 2 段写**新引擎行为**；旧引擎描述只保留在第 1 段原文与历史记录里。

### 9.4 B18 开工交接（2026-11-10，B17 收口后）

> 基线：B0–B17 全部 implemented；`cargo test -p sanguosha2` 620+ 全绿；`check:sgs2` / `sgs:check:fast` 全绿。

- **批次**：B18 界限突破·J6（8 将 / 17 技能）
  | 武将 | gid | 技能（code） |
  |---|---|---|
  | 界王异 | 621 | 贞烈（`zhenlie`）、秘计（`miji`） |
  | 界张春华 | 625 | 绝情（`jueqing`）、伤逝（`shangshi`）、翦灭（`jianmie`） |
  | 界刘表 | 626 | 自守（`zishou`）、宗室（`zongshi`） |
  | 界蔡夫人 | 627 | 献州（`xianzhou`）、窃听（`qieting`） |
  | 界曹冲 | 628 | 称象（`chengxiang`）、仁心（`renxin`） |
  | 界曹植 | 629 | 落英（`luoying`）、酒诗（`jiushi`） |
  | 界吴国太 | 640 | 甘露（`ganlu`）、补益（`buyi`） |
  | 界廖化 | 641 | 当先（`dangxian`）、伏枥（`fuli`） |
- **命令序列**
  1. `node apps/scripts/sgs-align-sheet.mjs --batch B18 --new` → `--sync`
  2. 填第 2–5 段（`--v1-anchors` 找 v1 锚点）→ `npm run sgs2:batch -- B18`
  3. 交用户逐条裁定 → 回写第 6/7 段 → `status=confirmed`
  4. 实现 `crates/engines/sanguosha2/src/skills/b18.rs` + `tests/b18.rs`（每技能 ≥1 正例 + ≥1 反例）
  5. 注册 `SKILLS` + `V2_BATCHES` 加 18
  6. 回写对齐单第 2 段/impl/tests → 文档（skills/rules-rulings/skill-spec-rules/development-plan/milestones）
  7. `npm run check:sgs2` + `npm run sgs:check:fast` 全绿，一条 commit 一批
- **可复用钩子（勿重写）**：`skills::extra_targets` / `consume_extra_targets` / `extra_targets_no_distance`、`target_no_distance`、
  `sha_limit_exempt` / `sha_blocked_for`、`sha_limit_bonus` / `sha_limit_penalty`、`jiu_blocked`、`effective_rank`、
  `on_card_use`（每张牌使用钩子）、`on_card_finished`（此牌结算后）、`on_equip_lost`、`dying_extra_options/use`、
  `pindian_after`、`after_draw_phase`、`gain_batch`/`lost_batch`/`equip_lost_batch`/`action_seen`、`turn.damage_card:<card>`、`trim:` 动作入口。
- **pitfalls（B17 新增）**
  * `can_active` 必须与 handler 的 `active(g, seat, code)` 同源；否则 AI 会在「可发动 → 立即 Done」上空转（`check:sgs2-ai` 判 loop）。
  * 新增纯计数标记（如「巧」）要同时：`turn.*` 计数（回合清空）+ `view::marks_count_view` 投影 + 对齐单 UI 段。
  * 修改技能 code（撞车改名）要同步 5 处：v1 `skills.rs`、对齐单 `code`、`V2_BATCHES`/注册表、`general-audit.md` 矩阵、skills.md 表；改完跑 `check:sgs-align` + `check:sgs2-registry`（锚点校验）+ `sgs:check`。
  * 「延时锦囊额外目标」不要实现虚拟判定牌（用户 2026-11-10 裁定）；延时锦囊一律不参与 ±1。
