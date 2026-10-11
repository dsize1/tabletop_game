---
description: QQ 官方机器人（群）接入约定
---

# QQ Bot 接入约定

- 目标：**QQ 群**（官方 API v2，bot.q.qq.com）
- 群事件：`GROUP_AT_MESSAGE_CREATE`（@消息，默认）/ `GROUP_MESSAGE_CREATE`（全量消息，需开启「接收所有消息」）
- 使用官方 Node SDK（npm 包 `@tencent-connect/qqbot-nodejs`，精确版本以官方文档为准）
- 原始群事件**原样**写入 Redis Stream `qq.raw`
- 发送出口：消费 Redis Stream `qq.push`，调用 SDK 发群消息
- 发送限制：主动消息受频控（群 60/qpm、单关系 20/qpm、每日 1000 条/群），优先用被动回复（每条可回复 5 次、5 分钟内）
