# 长期记忆库（memory_palace）

存放跨会话的长期记忆，按需索引。

| 目录/文件 | 用途 |
|---|---|
| `plan/` | 实施计划与验收清单（按 sessionId 命名） |
| `decisions/` | 架构决策记录（ADR） |
| `glossary.md` | 项目术语表 |
| `milestones.md` | 里程碑与开发进度 |

## 使用约定

- 每次重大计划写入 `plan/<sessionId>.md`
- 重大技术决策写 ADR 到 `decisions/`
- 新术语补充到 `glossary.md`
