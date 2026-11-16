# V3 全量验收台账回落与复核（下一会话 0 号动作）

> 状态：**计划（待用户确认后执行）** · 创建 2026-11-16
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

- [ ] `docs/games/sanguosha/align/B19.json`、`B20.json`：`status: verified → implemented`，
      批次 `note` 追加「v2 签字 2026-11-10；**v3 待复核**」；跑 `--sync` 重生成同名 `.md`。
- [ ] `docs/games/sanguosha/general-audit.md`：12 行「人工通过」→「**v2 签字 / v3 待复核**」（人工列由用户确认后改）。
- [ ] `docs/games/sanguosha/legacy-retire.json`：`b20_verified_at` 增加说明「v2 口径，v3 未复核」（不动其他字段）。

### 步骤 B：align `current` 回写 V3（优先 B16–B20）

- [ ] 每批把 `current` 从「v2：…」改为「v3：…（`sanguosha3::kernel::skills::bN::…`）」，并按需补 `impl`/`tests` 锚点；
- [ ] 与近期整改保持一致：长标（内核目标单出口/无属性/结束阶段摸牌）、酣战（`fire_pindian_pre` + `pindian_after`）、
      屯田/急袭（田=实体牌）、虚拟牌无属性（`Suit::None` + `cards::attrs`）、若干「自动→询问」；
- [ ] `node apps/scripts/sgs-align-sheet.mjs --batch B16 --sync`（其余批同）→ 生成 `align/B*.md`。

### 步骤 C：门禁防再犯

- [ ] `check:sgs-align`：`verified` 必须声明 `engine`/`mechanics_version`（V3 收口不得沿用 v2 的 `verified`）；
- [ ] `check:sgs3-registry`：批次从「进行中 → 已收口」时，要求 align 该批 `status` 同步（避免再次继承）；
- [ ] `check:sgs-generals:matrix`：矩阵「人工结果」增加引擎列或备注列校验（`v2`/`v3`）。

### 步骤 D：V3 机器证据（147 将）

- [ ] `apps/scripts/sgs-general-audit.mjs` 适配 V3：V3 公共视图按宪法 I3 **不暴露 pending.options**
      → 改用创建 preview 时的 `viewer` 私有视图（`my_pending.options`）或引擎 `legal_actions` 驱动；
- [ ] 先 dry-run 一批：`npm run audit:sgs-generals -- B0 --seed 20261105`；
- [ ] 全量：`for b in B0…B20; do npm run audit:sgs-generals -- $b; done` → 重生成 `general-audit-evidence.md`（V3 口径 299 技能）。

### 步骤 E：人工走查与签字

- [ ] 每批走查单：`npm run sheet:sgs-audit B0`（→ `docs/games/sanguosha/audit-sheets/B0.md`）；
- [ ] 逐将走查（原文对照 / 歧义裁定 / 期望=实际 / 确认人+日期），由用户签字；
- [ ] 签字后才把该批 align `status` 置 `verified`、矩阵置「人工通过（v3）」。

## 3. 验证事项

- [ ] `npm run check:sgs-align` 绿（含新 `engine` 维度）
- [ ] `npm run check:sgs3:full` 绿（无代码回归）
- [ ] `npm run check:sgs-generals:matrix` 绿（147 行 / 21 批 / 技能齐全）
- [ ] `general-audit-evidence.md` 头显示 V3 口径（299 技能）且「失败 0 / 阻塞 0」
- [ ] 台账抽查：随机 5 行「人工通过」都能指出签字日期（v3）与确认人

## 4. 风险 / 注意

- **不得**由 agent 自行把矩阵置「人工通过」或把 align 置 `verified`（只能用户签字后改）；
- 步骤 A/B 是纯文档改动，可先做；步骤 D 需要 server/MCP 起来了才能跑（`audit:sgs-generals` 走 `/internal/*`）；
- B0–B18 的 align `current` 回写量大，建议按批推进（先 B16–B20，再 B11–B15，…，每批一条 commit）。

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
