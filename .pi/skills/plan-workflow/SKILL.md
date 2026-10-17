---
description: 计划工作流（围栏→修正→执行→验收→harness loop）。用于需要「先规划、人工确认后再实施」的开发任务：生成 6 分区计划文件、多轮修正、人工确认解除围栏后执行、核对待办/验证/变动文件、人工确认完成、询问并实施 harness 优化 loop、人工验收终结。
---

# 计划工作流（plan-workflow）

配套 `plan-memory` 扩展（`.pi/extensions/plan-memory/`）使用。核心原则：**围栏内只规划，人工确认才动手，每一步关键节点都由人工把关。**

计划文档位置：`memory_palace/plan/{sessionId}.md`，6 个分区：

1. `## 计划`
2. `## 待办实现`
3. `## 待确定事项`
4. `## act 阶段变动的文件列表`
5. `## 验证事项`
6. `## harness 优化项`

可用原语：

| 原语 | 作用 |
|---|---|
| `/plan` | 切换只读围栏（禁用 edit/write，bash 白名单） |
| `plan_record(section, items, mode)` | 写入/替换某个分区条目 |
| `plan_check(section, patterns, done)` | 勾选「待办实现/验证事项」复选框 |
| `/plan-stage <plan\|execute\|harness\|done>` | 更新文档 `status` 阶段 |
| `/plan-save`、`/plan-show` | 编辑合并 / 预览文档 |

---

## 阶段 A：规划（围栏 ON）

1. `/plan` 进入只读计划模式（若未开启），确认 edit/write 被禁用、bash 仅白名单。
2. 按计划生成 6 分区文件：用 `plan_record` 分别写入「计划 / 待办实现 / 待确定事项 / 验证事项 / harness 优化项」；`/plan-stage plan`（status=规划中）。
3. **多轮修正**：用户会针对「待确定事项」「计划」提出修改意见，逐轮用 `plan_record(mode="replace")` 或 `/plan-save` 更新。**围栏内严禁改代码**，只更新计划文档。
4. 修正持续到用户明确表示满意为止。

## 阶段 B：解除围栏，执行（人工确认）

5. **必须等待用户明确确认**（例如「确认执行」「开始吧」）。**不得自行解除围栏。**
6. 用户确认后：`/plan` 解除只读，`/plan-stage execute`。

## 阶段 C：执行计划 loop

7. 按「计划」逐步实施；每完成一步：
   - `plan_check(section="todo", patterns=[该步骤关键短语])` 勾掉待办；
   - 向「act 阶段变动的文件列表」追加实际改动的文件：`plan_record(section="actFiles", items=[文件路径])`。
8. 全部计划项完成后，逐条核对「验证事项」，用 `plan_check(section="verify", patterns=[...])` 标记通过项；未通过项回到步骤 7 继续。

## 阶段 D：人工确认完成

9. 汇报「待办 / 验证 / 变动文件」三份清单的完成情况，**询问用户：是否完成、目标是否一致**。
   - 未达成 → 回到步骤 7 继续执行。
   - 达成 → 进入阶段 E。

## 阶段 E：harness 优化 loop（可选，需询问）

10. **询问用户：是否发起 harness 优化 loop？**
    - **否** → `/plan-stage done`，终结任务。
    - **是** → `/plan-stage harness`，针对「harness 优化项」逐项实施优化（可新增/修订测试、脚本、脚手架、工具链等）。

## 阶段 F：人工验收 harness 成果

11. 汇报 harness 优化成果，**交由人工验收**。
    - 验收通过 → `/plan-stage done`，终结任务。
    - 不通过 → 回到步骤 10 继续优化。

## 硬性约束

- 阶段 A 内任何 `edit/write`、非白名单 bash 都会被扩展拦截；不要尝试绕过围栏。
- 解除围栏、发起 harness loop、任务终结这三个动作，**都必须由用户确认**。
- 每一步的产物（计划、清单、完成状态、阶段）都要实时落到 `memory_palace/plan/{sessionId}.md`，保证跨会话可续。
- 计划 / ADR 中的**数值型外部约束**（时间窗口、频控、延迟、上限）必须标注来源与验证状态；未经实测的结论标「待验证」，不得写死为实现约束。涉及外部约束先查 `docs/conventions/external-constraints.md`。
- **主动消息门禁**：计划若含主动消息（proactive / `pushActive` / 无 `reply_msg_id`），必须列入「待确定事项」，**执行前由用户明确确认**，并在计划写入 `【主动消息已人工确认】`；未确认不得实施。可用 `npm run check:proactive-gate` 自检。
