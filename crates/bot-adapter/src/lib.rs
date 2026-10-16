//! QQ 群事件 → 内部统一消息事件
//!
//! 订阅 Redis Stream `qq.raw`，过滤 @文本消息，归一化为 `InternalEvent` 写入 `internal`。

use redis::aio::ConnectionManager;
use redis::streams::{StreamMaxlen, StreamReadOptions, StreamReadReply};
use redis::{AsyncCommands, RedisResult};
use shared::{InternalEvent, MessageGroupAt, QqRawEvent};
use std::sync::Arc;
use storage::Storage;
use tracing::{debug, info, warn};

const RAW_STREAM: &str = "qq.raw";
const INTERNAL_STREAM: &str = "internal";
const GROUP: &str = "bot-adapter";
const CONSUMER: &str = "bot-adapter-1";
const MAXLEN: usize = 1000;

const PUSH_STREAM: &str = "qq.push";
const PUSH_GROUP: &str = "bot-adapter-push";
const PUSH_CONSUMER: &str = "bot-adapter-push-1";

const CHAT_LOG_STREAM: &str = "chat.log";
const CHAT_LOG_GROUP: &str = "server-chat-log";
const CHAT_LOG_CONSUMER: &str = "server-chat-log-1";

/// 启动 bot-adapter 消费循环（阻塞运行，直到进程退出）。
pub async fn run(redis_url: &str, storage: Arc<Storage>) -> anyhow::Result<()> {
    let client = redis::Client::open(redis_url)?;
    let mut conn = ConnectionManager::new(client).await?;

    ensure_group(&mut conn).await?;
    info!("bot-adapter 已启动，订阅 {RAW_STREAM} → {INTERNAL_STREAM}");

    // 启动时重投 PEL 中未 ACK 的历史消息
    match read_batch(&mut conn, "0").await {
        Ok(pending) => {
            if !pending.is_empty() {
                info!("重投 PEL 中 {} 条消息", pending.len());
                for (id, raw_json) in pending {
                    if let Err(e) = process_one(&mut conn, &storage, &id, &raw_json).await {
                        warn!("重投消息 {id} 失败: {e:#}");
                    }
                }
            }
        }
        Err(e) => warn!("读取 PEL 失败: {e:#}"),
    }

    loop {
        match read_batch(&mut conn, ">").await {
            Ok(entries) => {
                for (id, raw_json) in entries {
                    if let Err(e) = process_one(&mut conn, &storage, &id, &raw_json).await {
                        warn!("处理消息 {id} 失败: {e:#}");
                    }
                }
            }
            Err(e) => {
                // group 被外部删除（如清理脚本）时重建，避免永久 NOGROUP
                if e.to_string().contains("NOGROUP") {
                    if let Err(e2) = ensure_group(&mut conn).await {
                        warn!("重建消费者组失败: {e2:#}");
                    }
                }
                warn!("读取 {RAW_STREAM} 失败: {e:#}，1s 后重试");
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
    }
}

/// 确保消费者组存在（幂等：组已存在则忽略 BUSYGROUP）。
async fn ensure_group(conn: &mut ConnectionManager) -> RedisResult<()> {
    let res: RedisResult<()> = redis::cmd("XGROUP")
        .arg("CREATE")
        .arg(RAW_STREAM)
        .arg(GROUP)
        .arg("0")
        .arg("MKSTREAM")
        .query_async(conn)
        .await;
    match res {
        Ok(()) => Ok(()),
        Err(e) if e.to_string().contains("BUSYGROUP") => Ok(()),
        Err(e) => Err(e),
    }
}

/// 从 `qq.raw` 消费者组读一批。`id` 为 `>` 读新消息（阻塞 2s），为 `0` 读 PEL（不阻塞）。
async fn read_batch(conn: &mut ConnectionManager, id: &str) -> RedisResult<Vec<(String, String)>> {
    let mut opts = StreamReadOptions::default().group(GROUP, CONSUMER).count(100);
    if id == ">" {
        opts = opts.block(2000);
    }
    let reply: StreamReadReply = conn.xread_options(&[RAW_STREAM], &[id], &opts).await?;

    let mut out = Vec::new();
    for key in reply.keys {
        for entry in key.ids {
            if let Some(redis::Value::Data(bytes)) = entry.map.get("event") {
                if let Ok(s) = String::from_utf8(bytes.clone()) {
                    out.push((entry.id, s));
                }
            }
        }
    }
    Ok(out)
}

/// 处理单条消息：过滤 → 写 SQLite 留档 → 归一化 → 写 internal → ACK。
async fn process_one(
    conn: &mut ConnectionManager,
    storage: &Arc<Storage>,
    id: &str,
    raw_json: &str,
) -> anyhow::Result<()> {
    let raw: QqRawEvent = serde_json::from_str(raw_json)?;

    let is_at = raw.event_type == "GROUP_AT_MESSAGE_CREATE";
    let is_text = raw.raw.message_type == 0;

    if is_at && is_text {
        let event_id = shared::new_event_id();
        let content = raw.raw.content.trim();

        // 先写 SQLite（幂等去重），成功再写 internal
        storage.insert_group_message(
            &event_id,
            "in",
            &raw.raw.group_openid,
            Some(&raw.raw.author.member_openid),
            content,
            Some(raw_json),
        )?;

        let event = InternalEvent::MessageGroupAt(MessageGroupAt {
            event_id,
            source: "qq-gateway".to_string(),
            timestamp: raw.raw.timestamp.clone(),
            group_id: raw.raw.group_openid.clone(),
            sender_id: raw.raw.author.member_openid.clone(),
            sender_name: raw.raw.author.username.clone(),
            message_id: raw.raw.id.clone(),
            content: content.to_string(),
        });
        write_internal(conn, &event).await?;
        debug!(
            "→ internal {} [group={} sender={}]",
            event.event_id(),
            raw.raw.group_openid,
            raw.raw.author.username
        );
    }

    // 无论是否过滤，处理完毕即 ACK
    conn.xack::<_, _, _, ()>(RAW_STREAM, GROUP, &[id]).await?;
    Ok(())
}

/// 写入 `internal`（MAXLEN 裁剪）。
async fn write_internal(conn: &mut ConnectionManager, event: &InternalEvent) -> RedisResult<()> {
    let json = serde_json::to_string(event).expect("serialize InternalEvent");
    conn.xadd_maxlen::<_, _, _, _, ()>(
        INTERNAL_STREAM,
        StreamMaxlen::Approx(MAXLEN),
        "*",
        &[("event", json.as_str())],
    )
    .await?;
    Ok(())
}

/// 出站归档：订阅 `qq.push`，把我方发出的群消息留档到 group_messages（direction=out）。
/// 与 qq-gateway（发送）各用独立消费者组，互不影响；event_id 幂等去重。
pub async fn run_push_archiver(
    redis_url: &str,
    storage: Arc<Storage>,
) -> anyhow::Result<()> {
    let client = redis::Client::open(redis_url)?;
    let mut conn = ConnectionManager::new(client).await?;

    ensure_push_group(&mut conn).await?;
    info!("push 归档已启动，订阅 {PUSH_STREAM} → group_messages(out)");

    // 启动时重投 PEL 中未 ACK 的历史消息
    match read_push_batch(&mut conn, "0").await {
        Ok(pending) => {
            for (id, raw_json) in pending {
                if let Err(e) = archive_one(&storage, &raw_json).await {
                    warn!("归档消息 {id} 失败: {e:#}");
                }
                if let Err(e) = conn
                    .xack::<_, _, _, ()>(PUSH_STREAM, PUSH_GROUP, &[&id])
                    .await
                {
                    warn!("ACK {id} 失败: {e:#}");
                }
            }
        }
        Err(e) => warn!("读取 push PEL 失败: {e:#}"),
    }

    loop {
        match read_push_batch(&mut conn, ">").await {
            Ok(entries) => {
                for (id, raw_json) in entries {
                    if let Err(e) = archive_one(&storage, &raw_json).await {
                        warn!("归档消息 {id} 失败: {e:#}");
                    }
                    if let Err(e) = conn
                        .xack::<_, _, _, ()>(PUSH_STREAM, PUSH_GROUP, &[&id])
                        .await
                    {
                        warn!("ACK {id} 失败: {e:#}");
                    }
                }
            }
            Err(e) => {
                if e.to_string().contains("NOGROUP") {
                    if let Err(e2) = ensure_push_group(&mut conn).await {
                        warn!("重建 push 消费者组失败: {e2:#}");
                    }
                }
                warn!("读取 {PUSH_STREAM} 失败: {e:#}，1s 后重试");
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
    }
}

async fn ensure_push_group(conn: &mut ConnectionManager) -> RedisResult<()> {
    let res: RedisResult<()> = redis::cmd("XGROUP")
        .arg("CREATE")
        .arg(PUSH_STREAM)
        .arg(PUSH_GROUP)
        .arg("0")
        .arg("MKSTREAM")
        .query_async(conn)
        .await;
    match res {
        Ok(()) => Ok(()),
        Err(e) if e.to_string().contains("BUSYGROUP") => Ok(()),
        Err(e) => Err(e),
    }
}

async fn read_push_batch(
    conn: &mut ConnectionManager,
    id: &str,
) -> RedisResult<Vec<(String, String)>> {
    let mut opts = StreamReadOptions::default()
        .group(PUSH_GROUP, PUSH_CONSUMER)
        .count(100);
    if id == ">" {
        opts = opts.block(2000);
    }
    let reply: StreamReadReply = conn.xread_options(&[PUSH_STREAM], &[id], &opts).await?;

    let mut out = Vec::new();
    for key in reply.keys {
        for entry in key.ids {
            if let Some(redis::Value::Data(bytes)) = entry.map.get("event") {
                if let Ok(s) = String::from_utf8(bytes.clone()) {
                    out.push((entry.id, s));
                }
            }
        }
    }
    Ok(out)
}

async fn archive_one(storage: &Arc<Storage>, raw_json: &str) -> anyhow::Result<()> {
    let push: shared::PushEvent = serde_json::from_str(raw_json)?;
    storage.insert_group_message(
        &push.event_id,
        "out",
        &push.group_id,
        None, // 机器人自身发出的消息，无发送者 openid
        &push.content,
        Some(raw_json),
    )?;
    Ok(())
}

/// 对话归档：订阅 `chat.log`，把 agent-gateway 发出的 per-user 对话落 `chat_history`
/// （chat + game 分支；画像蒸馏数据源，ADR-0007）。独立消费者组；event_id 幂等去重。
pub async fn run_chat_log_archiver(
    redis_url: &str,
    storage: Arc<Storage>,
) -> anyhow::Result<()> {
    let client = redis::Client::open(redis_url)?;
    let mut conn = ConnectionManager::new(client).await?;

    ensure_chat_log_group(&mut conn).await?;
    info!("对话归档已启动，订阅 {CHAT_LOG_STREAM} → chat_history");

    // 启动时重投 PEL 中未 ACK 的历史消息
    match read_chat_log_batch(&mut conn, "0").await {
        Ok(pending) => {
            for (id, raw_json) in pending {
                if let Err(e) = archive_chat_log(&storage, &raw_json).await {
                    warn!("归档对话 {id} 失败: {e:#}");
                }
                if let Err(e) = conn
                    .xack::<_, _, _, ()>(CHAT_LOG_STREAM, CHAT_LOG_GROUP, &[&id])
                    .await
                {
                    warn!("ACK {id} 失败: {e:#}");
                }
            }
        }
        Err(e) => warn!("读取 chat.log PEL 失败: {e:#}"),
    }

    loop {
        match read_chat_log_batch(&mut conn, ">").await {
            Ok(entries) => {
                for (id, raw_json) in entries {
                    if let Err(e) = archive_chat_log(&storage, &raw_json).await {
                        warn!("归档对话 {id} 失败: {e:#}");
                    }
                    if let Err(e) = conn
                        .xack::<_, _, _, ()>(CHAT_LOG_STREAM, CHAT_LOG_GROUP, &[&id])
                        .await
                    {
                        warn!("ACK {id} 失败: {e:#}");
                    }
                }
            }
            Err(e) => {
                if e.to_string().contains("NOGROUP") {
                    if let Err(e2) = ensure_chat_log_group(&mut conn).await {
                        warn!("重建 chat.log 消费者组失败: {e2:#}");
                    }
                }
                warn!("读取 {CHAT_LOG_STREAM} 失败: {e:#}，1s 后重试");
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
    }
}

async fn ensure_chat_log_group(conn: &mut ConnectionManager) -> RedisResult<()> {
    let res: RedisResult<()> = redis::cmd("XGROUP")
        .arg("CREATE")
        .arg(CHAT_LOG_STREAM)
        .arg(CHAT_LOG_GROUP)
        .arg("0")
        .arg("MKSTREAM")
        .query_async(conn)
        .await;
    match res {
        Ok(()) => Ok(()),
        Err(e) if e.to_string().contains("BUSYGROUP") => Ok(()),
        Err(e) => Err(e),
    }
}

async fn read_chat_log_batch(
    conn: &mut ConnectionManager,
    id: &str,
) -> RedisResult<Vec<(String, String)>> {
    let mut opts = StreamReadOptions::default()
        .group(CHAT_LOG_GROUP, CHAT_LOG_CONSUMER)
        .count(100);
    if id == ">" {
        opts = opts.block(2000);
    }
    let reply: StreamReadReply = conn.xread_options(&[CHAT_LOG_STREAM], &[id], &opts).await?;

    let mut out = Vec::new();
    for key in reply.keys {
        for entry in key.ids {
            if let Some(redis::Value::Data(bytes)) = entry.map.get("event") {
                if let Ok(s) = String::from_utf8(bytes.clone()) {
                    out.push((entry.id, s));
                }
            }
        }
    }
    Ok(out)
}

async fn archive_chat_log(storage: &Arc<Storage>, raw_json: &str) -> anyhow::Result<()> {
    let ev: shared::ChatLogEvent = serde_json::from_str(raw_json)?;
    storage.insert_chat_message(
        &ev.event_id,
        &ev.openid,
        &ev.group_id,
        &ev.role,
        &ev.content,
        &ev.created_at,
    )?;
    Ok(())
}
