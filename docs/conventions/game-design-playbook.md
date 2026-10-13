# 游戏设计阶段规范与经验（Game Design Playbook）

> 状态：草案 v0.1（待确认）。本文沉淀「拉斯维加斯引擎开发前设计」的执行轨迹，供后续引擎直接套用。
> 配套 skill：`.pi/skills/game-engine-dev/SKILL.md`；模块约定：`docs/modules/game-engine.md`。

## 0. 定位

新增桌游引擎前，**先完成纯文档设计**（零代码），再实现。设计阶段产出按固定目录落盘：

```
docs/games/<game_id>/
  rules.md                 # 规则文档（数值约束标来源/验证状态）
  props/
    index.md               # 道具总览 + 路径分类约定
    <dice|casinos|...>.md  # 按道具类型细分
  legal-actions.md         # 合法动作列表
  data-structure.md        # 数据结构 + 序列化
  state-machine.md         # 状态机
  rule-engine-permission.md# 规则引擎 + 权限校验
```

## 1. 规则与道具收集

- **先钉版本口径**：同一桌游常有多个版本/扩展（如 Las Vegas 标准版 vs Boulevard），必须先确定玩哪个版本。
- **数值约束标来源 + 验证状态**：人数、组件数量、面额等一律「待验证」起步，经规则书/实测确认后才可作为实现约束（`docs/conventions/coding-standards.md`）。
- **道具按文件路径分类、类型间细分**：`props/<类型>.md`，类型内部再按颜色/编号/面额细分小节。
- **明确信息可见性**：哪些公开、哪些隐藏（钞票面额隐藏、骰子公开），直接影响数据结构与 get_state 脱敏。

## 2. 合法动作列表

- **区分「玩家动作」与「系统事件」**：掷骰/洗牌/结算通常是引擎自动执行，不是玩家动作。
- **动作 id 编码**：稳定机器可解析（`place:n`）+ 描述面向决策模型口语化。
- **动作空间 ≤ 26 检查**：超 26 需 wide_choice/分组（`docs/modules/game-engine.md`）。

## 3. 数据结构

- 沿用 `GameState` 通用信封 + `data` 引擎私有 JSON；引擎内部定义强类型 `XxxState` 序列化进 `data`。
- 私有结构带 `schema_version` 便于迁移。
- **随机结果必须入事件**（洗牌/掷骰），重放不重新随机。

## 4. 状态机

- phase 明确；迁移表覆盖「当前 phase × 事件/动作 × 前置条件 × 目标 phase」。
- 终局条件显式；边界（中途退出/悬挂/超时/重复开桌/人数校验）显式列出并确认。

## 5. 规则引擎与权限

- 校验链四步：合法性 → 权限 → 执行 → 事件溯源（同一事务）。
- 结算等确定性逻辑在引擎内实现，**不用 LLM 判断**；LLM 只做叙述。

## 6. MCP 统一原语评估

- 在规则/动作/状态机**定稿后**评估原语缺什么（顺序后置）。
- 区分两类出口：MCP 原语（LLM/agent 运行时）+ server HTTP 资源 API（web 展示）。
- 结论三选一：保留 / 小幅增强 / 重构，附理由与影响。

## 7. 执行轨迹复盘（本次经验）

1. 先读 `docs/architecture.md` + `docs/modules/game-engine.md` + `game-engine-core` 现状，摸清已有抽象与既有缺陷（version 硬编码、RNG 不可注入）。
2. 规则是地基：规则未定，动作/状态机/结算都会返工 → 先出 rules + props，标「待确认」点。
3. 每份文档草案标注「待确认/待核实」，避免把假设写成实现约束。
4. 序列化/version 对齐等横切决策上升为 ADR。
5. 设计阶段零代码：只产文档 + ADR + 术语表；代码修改留给后续实现里程碑。

### 7.1 教训（v0.2 追加，来自 18b41b34 复盘）

> 复盘原始证据已归档：`memory_palace/artifacts/2026-10-12T23-36-44-531Z_18b41b34-*.jsonl`（子会话轨迹）、`memory_palace/artifacts/las-vegas-rules-source-2026-10-13.txt`（权威规则文本）。

- **规则必须用用户提供的权威文本**：不得凭模型记忆自编版本（v0.1 曾自编 Ravensburger 标准版，与用户《Las Vegas Royale》完全不同，导致整批文档返工）。用户文本应落盘备份（本仓 `logs/`）。
- **不忽视「开赛前游戏设置阶段」**：多人桌游常有 lobby（发起/加入/退出/开赛/中断），必须在状态机与 MCP 原语中显式建模，不能只从 `awaiting_*` 开始。
- **动作空间评估要含小游戏/扩展分支**：主流程 ≤6 不代表整体 ≤26；`Las Vegas Royale` 的 16 个小游戏分支叠加后超 26，须 wide_choice。
- **指令路由不得用正则/硬编码**（跨模块经验）：dev 任务链路曾用 `确认|执行` 子串正则误判用户意图；此类判断应由确定性原语/显式状态承担。

## 8. 通用清单（新引擎照抄）

- [ ] 版本口径已钉死（规则文本来源已固化备份）
- [ ] rules.md（数值约束全标来源/验证状态）
- [ ] props/（按类型细分，路径分类）
- [ ] legal-actions.md（动作编码 + 动作空间 ≤26 结论）
- [ ] data-structure.md（信封 + 强类型私有状态 + 事件 payload + 脱敏）
- [ ] state-machine.md（迁移表 + 终局 + 边界；含 lobby/setup 等开赛前阶段）
- [ ] rule-engine-permission.md（四步校验链 + 确定性结算）
- [ ] MCP 原语评估结论（含生命周期原语）
- [ ] ADR（序列化/横切决策）
- [ ] glossary 术语更新
