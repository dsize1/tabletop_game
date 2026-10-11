# plan-memory 扩展

计划模式 + 计划落盘到长期文档 `memory_palace/plan/{sessionId}.md`。
配套工作流见 skill：`/skill:plan-workflow`（`.pi/skills/plan-workflow/SKILL.md`）。

## 命令

| 命令 | 说明 |
|---|---|
| `/plan` | 切换只读计划模式（禁用 edit/write，bash 仅白名单） |
| `/plan-save` | 保存计划。默认**分区合并**（只编辑 6 个受管分区，合并进现有文档）；`--rewrite` / `--full` / `-r` **全量重写** |
| `/plan-show` | 预览当前会话的计划文档 |
| `/plan-stage <plan\|execute\|harness\|done>` | 更新文档 frontmatter `status`（工作流阶段） |
| `Ctrl+Alt+P` | 快捷切换只读模式 |

## 工具

`plan_record(section, items, mode?)` — 让模型编程式写入分区：

- `plan` → `## 计划`（编号）
- `todo` → `## 待办实现`（复选框）
- `open` → `## 待确定事项`
- `actFiles` → `## act 阶段变动的文件列表`
- `verify` → `## 验证事项`（复选框）
- `harness` → `## harness 优化项`

`mode` 默认 `append`（追加），`replace` 为替换该分区。

`plan_check(section, patterns, done?)` — 勾选「待办实现 / 验证事项」复选框条目：
`section` 取 `todo` / `verify`，`patterns` 为匹配条目文本的子串列表，`done` 默认 `true`（标记 `[x]`）。

## 落盘格式

```markdown
---
session_id: <uuid>
date: 2026-10-11
status: 规划中
---

# 计划文档

## 计划
1. ...

## 待办实现
- [ ] ...

## 待确定事项
- ...

## act 阶段变动的文件列表
- ...

## 验证事项
- [ ] ...

## harness 优化项
- ...
```
