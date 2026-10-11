# ADR-0001：Redis Streams 传输 + SQLite 真相源

- 状态：已采纳（Accepted）
- 日期：2026-10-08

## 背景

项目需要持久化三类数据：群消息（进/出）、玩家执行记录、游戏对局状态。
系统为 Node（qq-gateway、web）与 Rust（server 单进程多模块）混合，跨进程、跨语言协作。
基础设施已有 Redis 与 SQLite，希望在不引入额外组件的前提下满足消息传递与持久化需求。

## 决策

1. **消息总线**：用 Redis Streams 承载跨进程事件传输（`qq.raw` 入站 / `qq.push` 出站），消费者组消费。
2. **持久化真相源**：SQLite 作为唯一持久化存储，WAL 模式 + 单写者（写入收敛到 Rust `server`）。
3. **可靠性纪律**：消费端「先写 SQLite 成功，再 XACK」；事件带 `event_id`，SQLite 用唯一键去重；Stream 用 `MAXLEN ~ 1000` 裁剪。

## 理由

- 备选 Kafka / RabbitMQ / NATS：功能更强，但运维成本高；当前体量（QQ 群桌游助手）用不上。
- 备选 gRPC / HTTP 直连：同步耦合、无法削峰，不适合事件链路；仅用于 web→server 的同步查询（鉴权 / 资源查询）。
- 复用已有 Redis / SQLite，零新增组件；Node/Rust 各配客户端即可跨语言通信。

## 影响

- 正面：零额外运维、异步解耦、可缓冲削峰、PEL 重投防丢。
- 代价：at-least-once 需业务幂等（`event_id` 去重）；SQLite 单写者限制并发写；Redis 内存需裁剪。
- 升级信号：吞吐上千/秒、需死信/延迟队列/事务性 outbox、多进程并发写时，迁 Kafka/NATS 或 Postgres。
- 详见 `docs/conventions/persistence.md`。
