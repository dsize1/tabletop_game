# specs/sanguosha — 引擎规格层（spec-kit）

> 目的：把「引擎语义」从实现约定升级为**可校验的规格**。规格是唯一事实源，
> 门禁 `npm run check:spec-conformance` 逐条校验「规格 ↔ 代码 ↔ 用例」三者一致。

## 目录

| 路径 | 内容 |
|---|---|
| `constitution.md` | **引擎语义宪法**：I1–I3 不变量 + 例外表 + 两条硬约束（一个语义一个入口 / 帧数据只写发起帧） |
| `state/spec.md` | 状态模型（体力 / 存活 / 死亡 / 身份 / 区域 / 生命周期）与不变量 |
| `interaction/spec.md` | 询问原语（always-ask 判据、出口语义、pending.data 契约、文案唯一） |
| `legality/spec.md` | 合法动作集契约（legal_actions ≡ execute、空集语义、条件 ≠ 跳过） |
| `skills/README.md` | 技能契约模板与分批落地方式（296 技能） |
| `contracts/*.json` | **机器可校验**的规格片段：规则 id / 用例锚点 / 禁止模式 / 约定规则声明 |
| `contracts/conventions.json` | C1–C6「迁移欠账/白名单类」约定的**规则声明**（检测实现在脚本；id 必须一一对应） |

## 校验

```
npm run check:spec-conformance     # 规格 ↔ 代码 ↔ 用例
```

## 一个规则只有一个事实源

| 规则 | 事实源 | 执行者 |
|---|---|---|
| I1 询问完整性（有手牌必问 / 不得代判） | `contracts/interaction.json` | `check:spec-conformance` |
| I2 状态语义（存活/死亡/身份公开/未选将不展示） | `contracts/state.json` | `check:spec-conformance` |
| L1/L2 合法动作集（legal ≡ execute / 空集语义） | `contracts/legality.json` | `check:spec-conformance` |
| C1–C6 迁移欠账（体力直写 / 内核技能名 / 标记 / 计数器 / 跳过出口 / kind） | `contracts/conventions.json` | `check:sgs2-conventions`（脚本只提供检测实现，id 双向校验） |

> 2026-11-12：C7（询问完整性）/ C8（存活语义）已从约定脚本**移入** spec 契约（I1 / I2），
> 脚本与契约不再各写一遍；两边 id 少一个即红。

校验项（当前）：

1. **每条规则必须有用例锚点**（`anchor: tests/<file>::<fn>`），用例函数不存在 → 红；
2. **禁止模式**（`forbid`）在指定文件内必须 0 命中：如「视图用 hp 推导存活」「内核按条件跳过询问」；
3. **技能注册表快照**（`contracts/skills.json`）必须与 `src/skills/*.rs` 的 `code`/`kind` 完全一致（无遗漏、无多余、类型一致）。

## 与既有产物的关系

| 既有 | 定位 | 是否重复 |
|---|---|---|
| `docs/games/sanguosha/align/B*.json` | **技能语义**裁定（775 条疑点） | 不重复：spec 管**通用机制**（状态/交互/合法集） |
| `docs/games/sanguosha/skills.md` | 技能索引（人读） | 不重复：spec 管契约与门禁 |
| `check:sgs2-conventions` C1–C8 | 由脚本硬编码的约定 | spec 化后逐步改为读 `contracts/*.json`（C7/C8 已由本门禁覆盖） |
