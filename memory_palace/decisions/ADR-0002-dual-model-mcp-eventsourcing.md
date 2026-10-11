# ADR-0002：双模型 + MCP 化游戏引擎 + 事件溯源

- 状态：已采纳（Accepted，2026-10-11 修订）
- 日期：2026-10-11

> 修订（2026-10-11）：原决策 10 与「延迟可控」理由以「被动回复窗口 3s 超时」为依据，实为误解（群聊被动窗口 5 分钟 / 5 次，见 `docs/modules/qq-gateway.md`）。已修订为「模板摘要默认 + 可配置 LLM 情景叙述」。

## 背景

原 `command-router`（前缀/@ 确定性解析）+ `game-engine`（EnginePlugin）设计，无法满足「自然语言交互 + 游戏主持人叙事 + 安全执行」的需求。决定引入双模型（主持人 LLM + 决策模型 SystemOne），将游戏引擎 MCP 化，并以事件溯源保证可复盘。

## 决策

1. **双模型**：主持人 LLM 用远程 API（型号待定，如 DeepSeek 系列）；决策模型 SystemOne = StartLux-Decision-4B（Q8_0 GGUF，本地 `/v1/systemone`）。
2. **agent-gateway（Node, pi-sdk）为唯一编排者**：消费 `internal`，做意图分类、编排、叙述；原 command-router 职责并入此模块。
3. **game-engine 以 MCP 形态暴露统一原语**（stdio）：只读（规则/状态/合法动作/活跃对局）+ 受校验写入（开桌/执行动作）。
4. **决策模型只建议，不写状态**；所有动作经规则引擎无条件校验（合法性 + 权限）。
5. **权限/合法性 = 规则引擎确定性计算**，不用 LLM 判断。
6. **意图 + 动作合并为一次决策调用**：`/v1/systemone` 单次前向多问题，同一 state 快照。
7. **置信度阈值配置化 + 兜底**：低于阈值 → 澄清/回显合法动作；无效消息至少回一条。
8. **事件溯源**：`game_events` append-only + `game_state` 快照（version 对齐 seq）。
9. **只 @文本消息**：非 @、非文本在 bot-adapter 过滤。
10. **分层回复**：游戏交互默认回规则模板摘要（承上启下）+ 可配置 LLM 情景叙述（不逐条复述结果，结果详情走 web）；聊天 LLM 叙述；开发任务回执 + 异步推送。
11. **开发任务保留 + 管理员白名单**（`member_openid` 判断）；游戏激活时不接受开发任务，丢弃并告知。

## 理由

- **安全边界**：决策模型与规则引擎分离，LLM 无法直接改状态；权限确定性可审计。
- **延迟可控**：被动回复窗口实为 5 分钟（非 3s），StartLux-Decision-4B 本地 GGUF 与远程 LLM 叙述的延迟均远低于窗口；游戏交互默认模板摘要保证 <1s 承上启下，LLM 情景叙述为可选增强（可静默降级）。
- **统一原语**：MCP 标准化各引擎能力，新增引擎只需实现 EnginePlugin。
- **可复盘**：事件溯源 + 快照满足「复盘/追溯最近操作」。
- **模型选型**：StartLux-Decision-4B 在多数基准不输 9B，Q8_0 ≈ 4.3GB 显存；主持人 LLM 远程化免去本地 9B 部署。

## 影响

- 正面：职责清晰、安全、可复盘、延迟可控、易扩展引擎。
- 代价：
  - MCP / 双模型 / 事件溯源增加复杂度；
  - 主持人 LLM 远程化有**隐私出站**（群消息 + 对局数据 → 第三方 API）；
  - StartLux-Decision 权重 **CC BY-NC 4.0 非商用**，商用需单独授权；
  - StartLux-Decision 为英文微调，中文消息是零样本跨语言，需在验证阶段实测；若不理想，启用「本地 MT 小模型翻译 + 低置信才翻译」两级兜底（**暂不实现**，见 `docs/modules/decision-service.md`）；
  - 被动回复窗口（群聊 5 分钟 / 5 次）已按官方口径确认；聊天/游戏叙述均可在窗口内完成，仅开发结果需异步主动推送。

- 详见：`docs/architecture.md`、`docs/modules/agent-gateway.md`、`docs/modules/decision-service.md`、`docs/modules/game-engine.md`、`docs/conventions/persistence.md`
