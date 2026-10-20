# push（出站推送 / `qq.push`）

> **状态变更（2026-10-20）**：原 Rust crate `crates/push` 已删除（全仓无调用方）。
> 出站推送实际由 **`apps/agent-gateway`（Node）直接 `XADD qq.push`** 完成；`qq-gateway` 消费并调 QQ API。
> 本文件保留为「出站通道的约定与约束」说明，不再是模块实现文档。

## 职责归属

| 角色 | 谁做 | 说明 |
|---|---|---|
| 生产 | `agent-gateway`（`pushReply` / `pushActive`） | 被动回复带 `reply_msg_id`；主动发送不带 |
| 消费 | `qq-gateway` | 消费者组 + ACK，调 QQ OpenAPI 发群消息 |
| 归档 | `bot-adapter::run_push_archiver` | 订阅 `qq.push` → `group_messages(direction=out)` |
| 验证脚本 | `apps/verify-push.mjs` | 注入一条主动消息指令，观察出站消费/ACK |

- 消息体：`shared::PushEvent`（`event_id` / `group_id` / `content` / `reply_msg_id?` / `msg_seq?` / `at_member_id?`）。
- 写入：`XADD qq.push MAXLEN ~ 1000`；`event_id` 为空时由生产方补 uuid v4。
- **Rust 侧不再提供「全局广播 / 个人定向推送」封装**；需要新推送场景时，在 `agent-gateway` 里按 ADR-0009 口径实现，不要恢复 crate。

## 主动消息限制与设计约束

「全局广播 / 个人定向推送」落到 QQ 侧即**主动消息**，受官方频控（见 `docs/modules/qq-gateway.md`）：

- 群聊主动消息：Bot 维度 60/qpm（未认证 30/qpm）、单关系 20/qpm、每日上限 1000 条/群
- 用户可在客户端关闭「允许主动发送」，主动消息可能发送失败
- 默认策略：主动消息**默认禁用**（`PROACTIVE_MESSAGES_ENABLED=0`），被动回复超限默认丢弃；见 ADR-0009、`docs/conventions/coding-standards.md` 门禁

因此出站设计：
- 优先把通知**挂靠到被动回复**（玩家刚互动时顺带回复；被动消息每条可回复 5 次、有效期 5 分钟）
- 主动推送做**合并/节流**，并对每个群/用户记账预算，避免打满 qpm 与每日上限
- 发送失败（用户关闭开关 / 超限）需有兜底：记录失败，降级为下次被动回复时补发

## 测试方式

- **出站验证脚本**：`apps/verify-push.mjs` 向 `qq.push` 注入一条「主动消息」指令（`reply_msg_id=null`），观察 qq-gateway PEL/ACK 与消费者组信息；用法 `node apps/verify-push.mjs [group_openid] [content] [--no-cleanup]`
- **真机（可选）**：传真实群 openid（`PUSH_TEST_GROUP` 或首个位置参数）发一条主动消息，需 Redis + qq-gateway（QQ 凭证 + 机器人加群 + WebSocket 在线）；会消耗主动消息日配额（1000/群/天）
- 原 `cargo test -p push`（`ensure_event_id` / 序列化 round-trip）随 crate 删除一并移除

## 关联

- `docs/modules/qq-gateway.md`（消费端与频控细节）
- `docs/conventions/persistence.md`（`qq.push` 保留策略与 ACK 约定）
- `docs/conventions/external-constraints.md`（配额来源与验证状态）
- ADR-0009（主动消息门禁）
