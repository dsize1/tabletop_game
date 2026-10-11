# 术语表

| 术语 | 含义 |
|---|---|
| qq-gateway | QQ 官方协议接入服务（Node/TS） |
| agent-gateway | 主持人服务（Node/TS + pi-sdk），编排意图/决策/执行/叙述 |
| internal event | 内部统一消息事件（QQ 事件标准化后的形态） |
| qq.raw | Redis Stream：qq-gateway 写入的原始 QQ 群事件 |
| internal | Redis Stream：bot-adapter 写入、agent-gateway 消费的归一化事件 |
| qq.push | Redis Stream：agent-gateway/push 写入、qq-gateway 消费的推送指令 |
| EnginePlugin | game-engine-core 定义的引擎抽象 trait |
| las-vegas | 首个桌游引擎「拉斯维加斯」 |
| 短期 token | Web 鉴权用的短时效 token（个人资源访问） |
| 主持人（LLM） | 远程 LLM（型号待定），负责叙述、闲聊、开发任务、编排 |
| SystemOne / 决策模型 | StartLux-Decision-4B（本地 GGUF），暴露 `/v1/systemone`，只建议不写状态 |
| 规则引擎 | game-engine 的确定性校验与状态机（合法性/权限/执行/事件溯源） |
| typed decision | 决策模型的问答类型：`choice`（多选一）/ `noul`（是/否）/ `score`（分级） |
| MCP 统一原语 | 各引擎标准化为 MCP 工具（只读查询 + 受校验写入） |
| 事件溯源 | append-only `game_events` + `game_state` 快照（version 对齐） |
| 置信度 | TypeSafe 公式 `(p_max − 1/n) / (1 − 1/n)`，用于阈值兜底 |
| wide_choice | 决策模型单题超 26 选项时的两阶段分组机制 |
| 被动回复 / 主动发送 | QQ 消息回复两种方式：带 msg_id 回复（群聊 5 分钟、每条最多 5 次）/ POST 主动推送（群 1000 条/天） |
