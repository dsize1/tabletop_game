---
name: qq-bot-dev
description: QQ 官方机器人（群）接入开发。使用于 qq-gateway 事件接收、群消息发送、QQ OpenAPI 调用相关任务。
---

# QQ Bot 开发

目标 **QQ 群**，官方 API v2。约定见 `.pi/prompts/qq-bot-conventions.md`。

- 事件接收：官方 Node SDK（`@tencent-connect/qqbot-nodejs`）的群事件回调
- 原始事件写入 Redis Stream `qq.raw`
- 消息发送：消费 Redis Stream `qq.push`，调用群消息发送 API
- 接入票据（AppID/Secret/Token）走环境变量，勿入库
