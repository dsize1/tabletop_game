# V3 全量验收台账回落与复核（下一会话 0 号动作）

> 状态：**执行中（步骤 A–C ✅ / 步骤 D ⛔ 阻塞于「V3 工作台沙盒」/ 步骤 E 待用户签字）** · 创建 2026-11-16 · 更新 2026-11-16
> 触发：用户追问「147 将技能是否都已验证」。结论是**实现全齐、验收几乎为零**，
> 且 align 台账里存在 **v2 时代签字被 V3 继承**的「假绿」，必须先回落台账、再跑 V3 验证。
> 本文件只做**文档与门禁**（不改技能行为）；技能整改见 `v3-parity-baseline.md §7.35/§7.36`。

## 0. 事实基线（2026-11-16 核查）

| 维度 | 事实 | 证据 |
|---|---|---|
| 实现 | 147/147 将、299/299 技能已注册 + 已收口（`ENABLED_SKILLS`）+ 每 code 有测试锚点 | `npm run check:sgs3:full` 绿；`check:sgs3-registry` 299/299 |
| 人工验收 | **12 行「人工通过」/ 135 行「未验证」**，且 12 行是 **2026-11-10（v2 期）**签字 | `docs/games/sanguosha/general-audit.md`（`check:sgs-generals:matrix` 147 行） |
| align 状态 | `verified` 23 技能（B19 15 + B20 8），`implemented` 276 技能 | `align/B19.json`、`B20.json`：`status=verified, confirmed_at=2026-11-10` |
| 台账语义 | 所有批 `current` 字段仍写 **v2 实现**（`v2：…`），与 V3 实际行为已脱节（长标/酣战/屯田/急袭/虚拟牌无属性…） | `align/B*.json` 的 `current` |
| V3 机器证据 | `general-audit-evidence.md` 生成于 **2026-11-11**、口径「288 技能」（v2），V3 从未重跑 | 该文件头 + "288 技能" |
| V3 人工抽检 | 文档自己标注「R1–R7 待人工」= 未执行 | `v3-parity-baseline` §7.x、`next-v3-rebuild` 进度表 |

**根因**：V3 重写了内核与技能（`sanguosha3`，2026-11-13 起），但批次流程只要求「注册/收口/用例锚点」，
**没有任何门禁把 v2 的 `verified` 回落**，于是 `verified` 被继承成假绿。

## 1. 目标

1. 台账诚实：`verified` / 「人工通过」必须明确指向**哪个引擎（v2 / v3）**；
2. 语义对齐：align `current` 回写为 **V3 实现**（B16–B20 优先，因近期刚整改）；
3. 机器证据覆盖 V3（147 将可达/不报错/守恒/pending 可续/可复现）；
4. 人工走查 135 将 + B19/B20 复核，逐批签字后才允许 `verified`。

## 2. 待办（分 5 步，先文档后代码）

### 步骤 A：台账回落（只改文档，最快止血）

- [x] `docs/games/sanguosha/align/B19.json`、`B20.json`：`status: verified → implemented`（23 技能同步回落），
      批次 `note` 追加「v2 签字 2026-11-10；**v3 待复核**」；已 `--sync` 重生成同名 `.md`。
- [x] `docs/games/sanguosha/general-audit.md`：12 行「人工通过」→「**v2 签字 / v3 待复核**」（确认人改「用户 2026-11-10（v2）」，备注加【v2 口径，V3 未复核】）。
- [x] `docs/games/sanguosha/legacy-retire.json`：新增 `b20_verified_at_note`＝「v2 口径（2026-11-10 用户签字）；v3 未复核」（其他字段未动）。
- [x] 顺带：新门禁抓到 `B1`/`B2` 已收口但批次状态仍 `confirmed` → 同步为 `implemented`（避免同样的「台账没跟上 V3」）。

### 步骤 B：align `current` 回写 V3（B11–B20 已完成）

- [x] `B16`–`B20` 共 **68 技能**（B16 14 / B17 14 / B18 17 / B19 15+马术 / B20 8）；
- [x] `B11`–`B15` 共 **77 技能**（B11 14 / B12 8 / B13 16 / B14 20 / B15 19）；
      `current` 改为「v3：…」，`impl` 改为 `sanguosha3::kernel::skills::bN::…`，`tests` 改为 `sanguosha3/tests/bN.rs` 的用例名，`confidence=verified`；
      批次 `engine` 置 `v3`（B0–B10 仍为 `v2`，待后续按同一流程推进）。
- [x] 覆盖近期整改口径：长标（目标单出口/无属性/结束阶段摸牌）、酣战（`fire_pindian_pre` + `pindian_after`）、
      屯田/急袭（田=实体牌）、虚拟牌无属性（`Suit::None` + `cards::attrs`）、「自动→询问」等。
- [x] `sgs-align-sheet --batch --sync` → 重生成 `align/B*.md`；B11–B20 走查单同步重生。
- [x] 待复核分歧已写入 `current`：`changbiao` 摸牌时机、`dimeng` 弃牌时机（V3 实现在结束阶段开始，`agreed` 写「出牌阶段结束」）。
- 说明：`q.test`（775 条裁定用例锚点）仍指向 v2 用例库（历史语料），本轮未迁移；技能级 `impl`/`tests` 已按引擎分派校验。

### 步骤 C：门禁防再犯 — ✅ 已完成

- [x] `check:sgs-align`：`verified` 必须「批次 `engine=v3` + `confirmed_at ≥ 2026-11-13` + `impl` 以 `sanguosha3::` 开头」；
      批次 `verified` ⇒ 全技能 verified；`engine` 非法值报错。
- [x] `check:sgs-align`（新增）：`engine=v3` 批次逐技能校验 `impl` 的 `bN::fn` 在 `sanguosha3/src/kernel/skills/bN.rs` 存在、
      `tests` 的 `bN::fn` 在 `sanguosha3/tests/bN.rs` 存在（已用故意写错验证会失败）。
- [x] `check:sgs3-registry`：批次已收口 ⇒ align 批次状态必须 ≥ `implemented`；`verified` 必须 `engine=v3` 且 `confirmed_at ≥ 2026-11-13`。
- [x] `check:sgs-generals:matrix`：人工结果枚举校验；`人工通过` 必须含 **v3** 签字与日期；非「未验证」行必须在确认人/备注标明 `v2`/`v3`。
- [x] `check:sgs2-registry`：对 `engine=v3` 的批次跳过 v2 锚点校验（改由 `check:sgs-align` 按 v3 源码/用例校验）。

### 步骤 D：V3 机器证据（147 将）— ⛔ 阻塞（发现未登记前置）

- [ ] `audit:sgs-generals` 适配 V3 —— **未实施**：workbench 仍是 v2 基座，V3 无法建沙盒/注入场景。
  - 实测（server 已重建）：`npm run audit:sgs-generals -- B0` → 3/3 失败，`游戏 sanguosha 不支持场景注入`；
  - 根因：`crates/server/src/preview.rs` 的 `/internal/game/preview/sandbox` 直接调 `sanguosha2::sandbox::new_sandbox_game`；
    `preview.rs` 的 `seat_summary` / `current_actor` / `drive` 全部走 `sanguosha2::ai`；V3 `SanguoshaEngine` 未实现 `apply_scene`；
    v2 快照喂给 V3 引擎会被 `validate_state` 拒绝。
  - 附带修复：`crates/server/src/gameprops_routes.rs` 的 `Suit` 匹配漏 `Suit::None` 导致 **server 编译失败**（已修，`cargo build -p server` 绿）。
  - 结论：需要**新开任务**「V3 工作台沙盒/场景注入 + drive 适配（preview v3 基座）」，完成后才能跑步骤 D；
    证据文件 `general-audit-evidence.md` / `data/audit/*` 保持 v2 旧值（**不得当 V3 证据用**）。
- [ ] 先 dry-run 一批：`npm run audit:sgs-generals -- B0 --seed 20261105`（等 v3 沙盒就绪）
- [ ] 全量：`for b in B0…B20; do npm run audit:sgs-generals -- $b; done` → 重生成 `general-audit-evidence.md`（V3 口径 299 技能）

### 步骤 E：人工走查与签字（待用户）

- [ ] 每批走查单：`npm run sheet:sgs-audit B0`（→ `docs/games/sanguosha/audit-sheets/B0.md`）；
- [ ] 逐将走查（原文对照 / 歧义裁定 / 期望=实际 / 确认人+日期），由用户签字；
- [ ] 签字后才把该批 align `status` 置 `verified`、矩阵置「人工通过（v3）」。

## 2.1 本轮附加修复（非计划项，但阻塞执行）

- `apps/scripts/sgs-align-sheet.mjs`：顶层 `return`（ESM 非法）导致脚本直接 `SyntaxError` → 改 `process.exit(0)`；否则步骤 A 无法 `--sync`。
- `crates/server/src/gameprops_routes.rs`：`Suit::None` 匹配缺失 → server 编译失败；物理牌表按「跳过 + 注释」处理（虚拟牌无属性不影响中台）。

## 3. 验证事项

- [x] `npm run check:sgs-align` 绿（含新 `engine` 维度 + v3 锚点存在性）
- [x] `npm run check:sgs3:full` 绿（`cargo test -p sanguosha3` + kernel/registry/suit/conventions + align + spec-conformance 全过）
- [x] `npm run check:sgs-generals:matrix` 绿（147 行 / 21 批 / 技能齐全 + 人工列引擎口径）
- [ ] `general-audit-evidence.md` 头显示 V3 口径（299 技能）且「失败 0 / 阻塞 0」 → **阻塞于步骤 D**
- [ ] 台账抽查：随机 5 行「人工通过」都能指出签字日期（v3）与确认人 → 当前无 v3 签字（符合预期）

## 4. 风险 / 注意

- **不得**由 agent 自行把矩阵置「人工通过」或把 align 置 `verified`（只能用户签字后改）；
- 步骤 A/B 是纯文档改动，可先做；步骤 D 需要 server/MCP 起来了才能跑（`audit:sgs-generals` 走 `/internal/*`）；
- **新增发现（2026-11-16）**：workbench preview/sandbox 仍是 **v2 基座**（`preview.rs` 直调 `sanguosha2::sandbox`/`sanguosha2::ai`，
  V3 `apply_scene` 未实现）→ 步骤 D 必须以「V3 工作台沙盒/场景注入 + drive 适配」为前置任务，不能只改审计脚本；
- `general-audit-evidence.md` 现有内容仍是 **v2 口径**（288 技能），不得作为 V3 验收证据引用；
- B0–B15 的 align `current` 回写量大，建议按批推进（先 B16–B20 已完成，再 B11–B15，…，每批一条 commit）。

## 5. 命令速查

```bash
# 文档回落
node apps/scripts/sgs-align-sheet.mjs --batch B19 --sync
npm run check:sgs-generals:matrix

# V3 机器证据（先 dry-run）
npm run audit:sgs-generals -- B0 --seed 20261105

# 走查单
npm run sheet:sgs-audit B0

# 门禁
npm run check:sgs3:full
```
