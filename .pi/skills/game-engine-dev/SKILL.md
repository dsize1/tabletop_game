---
name: game-engine-dev
description: 桌游引擎开发。使用于新增桌游引擎、实现 EnginePlugin trait、对局状态管理相关任务。
---

# 桌游引擎开发

- 引擎实现 `game-engine-core` 的 EnginePlugin trait
- 首个引擎：`crates/engines/las-vegas`（拉斯维加斯）
- 新引擎放入 `crates/engines/<name>/`，并在 `crates/server` 注册
- 规则文档缺失时先补 `docs/` 或引擎目录 README，再实现逻辑
