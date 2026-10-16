//! SQLite 持久化（真相源）。
//!
//! WAL 模式 + busy_timeout + event_id 唯一键去重（幂等）。
//! 多进程（server / game-engine-mcp）各持连接，WAL 下并发写安全。

use rusqlite::{params, Connection};
use std::sync::Mutex;

pub struct Storage {
    conn: Mutex<Connection>,
}

impl Storage {
    pub fn open(path: &str) -> anyhow::Result<Self> {
        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA busy_timeout=5000;
             PRAGMA foreign_keys=ON;",
        )?;
        // 迁移：chat_history 旧结构（群级，无 openid/event_id）→ 新 per-user 结构。
        // 旧表无任何写入方，安全重建（数据不作保留）。
        migrate_chat_history(&conn)?;
        conn.execute_batch(CREATE_TABLES)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().expect("storage mutex poisoned")
    }

    /// 写入一条群消息留档（进/出），event_id 幂等去重。
    pub fn insert_group_message(
        &self,
        event_id: &str,
        direction: &str,
        group_id: &str,
        qq_id: Option<&str>,
        content: &str,
        raw_payload: Option<&str>,
    ) -> anyhow::Result<()> {
        let conn = self.lock();
        conn.execute(
            "INSERT OR IGNORE INTO group_messages
             (event_id, direction, group_id, qq_id, content, raw_payload, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![event_id, direction, group_id, qq_id, content, raw_payload, now_secs()],
        )?;
        Ok(())
    }

    /// 记录一次游戏状态迁移：append event + 更新快照（同一事务）。
    pub fn record_game_transition(
        &self,
        event_id: &str,
        game_id: &str,
        room_id: &str,
        event_type: &str,
        actor: Option<&str>,
        payload: &str,
        state_json: &str,
        version: i64,
    ) -> anyhow::Result<()> {
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO game_events
             (event_id, game_id, room_id, event_type, actor, payload, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![event_id, game_id, room_id, event_type, actor, payload, now_secs()],
        )?;
        tx.execute(
            "INSERT INTO game_state (game_id, room_id, state, version, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(game_id, room_id) DO UPDATE SET
               state=excluded.state, version=excluded.version, updated_at=excluded.updated_at",
            params![game_id, room_id, state_json, version, now_secs()],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// 读取所有对局快照（state JSON），用于引擎重启后恢复活跃对局。
    pub fn load_game_snapshots(&self) -> anyhow::Result<Vec<String>> {
        let conn = self.lock();
        let mut stmt = conn.prepare("SELECT state FROM game_state")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    // ---- dev_tasks ----

    /// 创建开发任务（受理回执前调用；title/plan_path 后续回填）。
    pub fn create_dev_task(
        &self,
        session_id: &str,
        group_id: &str,
        qq_id: &str,
        status: &str,
    ) -> anyhow::Result<()> {
        let conn = self.lock();
        conn.execute(
            "INSERT OR IGNORE INTO dev_tasks
             (session_id, group_id, qq_id, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![session_id, group_id, qq_id, status, now_secs()],
        )?;
        Ok(())
    }

    /// 回填任务标题（子会话计划生成后调用）。
    pub fn set_dev_task_title(&self, session_id: &str, title: &str) -> anyhow::Result<()> {
        let conn = self.lock();
        conn.execute(
            "UPDATE dev_tasks SET title=?1, updated_at=?2 WHERE session_id=?3",
            params![title, now_secs(), session_id],
        )?;
        Ok(())
    }

    /// 记录计划文档相对路径（memory_palace/plan/{session_id}.md）。
    pub fn set_dev_task_plan_path(&self, session_id: &str, plan_path: &str) -> anyhow::Result<()> {
        let conn = self.lock();
        conn.execute(
            "UPDATE dev_tasks SET plan_path=?1, updated_at=?2 WHERE session_id=?3",
            params![plan_path, now_secs(), session_id],
        )?;
        Ok(())
    }

    /// 更新任务状态（状态机由网关+引擎确定性驱动）。
    pub fn set_dev_task_status(&self, session_id: &str, status: &str) -> anyhow::Result<()> {
        let conn = self.lock();
        conn.execute(
            "UPDATE dev_tasks SET status=?1, updated_at=?2 WHERE session_id=?3",
            params![status, now_secs(), session_id],
        )?;
        Ok(())
    }

    /// 查询单个任务（无则 None）。
    pub fn get_dev_task(&self, session_id: &str) -> anyhow::Result<Option<DevTask>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT session_id, group_id, qq_id, title, plan_path, status, created_at, updated_at
             FROM dev_tasks WHERE session_id=?1",
        )?;
        let mut rows = stmt.query_map(params![session_id], map_dev_task)?;
        Ok(rows.next().transpose()?)
    }

    /// 列出全部任务（可选按状态过滤），按创建时间倒序。
    pub fn list_dev_tasks(&self, status_filter: Option<&str>) -> anyhow::Result<Vec<DevTask>> {
        let conn = self.lock();
        let mut out = Vec::new();
        match status_filter {
            Some(s) => {
                let mut stmt = conn.prepare(
                    "SELECT session_id, group_id, qq_id, title, plan_path, status, created_at, updated_at
                     FROM dev_tasks WHERE status=?1 ORDER BY created_at DESC",
                )?;
                let rows = stmt.query_map(params![s], map_dev_task)?;
                for r in rows {
                    out.push(r?);
                }
            }
            None => {
                let mut stmt = conn.prepare(
                    "SELECT session_id, group_id, qq_id, title, plan_path, status, created_at, updated_at
                     FROM dev_tasks ORDER BY created_at DESC",
                )?;
                let rows = stmt.query_map([], map_dev_task)?;
                for r in rows {
                    out.push(r?);
                }
            }
        }
        Ok(out)
    }

    // ---- chat_history（per-user 对话，画像蒸馏数据源；ADR-0007） ----

    /// 追加一条 per-user 对话（event_id 幂等去重；role: user / assistant）。
    pub fn insert_chat_message(
        &self,
        event_id: &str,
        openid: &str,
        group_id: &str,
        role: &str,
        content: &str,
        created_at: &str,
    ) -> anyhow::Result<()> {
        let conn = self.lock();
        conn.execute(
            "INSERT OR IGNORE INTO chat_history
             (event_id, openid, group_id, role, content, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![event_id, openid, group_id, role, content, created_at],
        )?;
        Ok(())
    }

    /// 读窗口内对话（`[from, to)`，ISO8601 字符串区间），按 openid、时间升序。
    pub fn get_chat_window(
        &self,
        from: &str,
        to: &str,
    ) -> anyhow::Result<Vec<ChatMessageRow>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT openid, group_id, role, content, created_at FROM chat_history
             WHERE created_at >= ?1 AND created_at < ?2
             ORDER BY openid, created_at",
        )?;
        let rows = stmt.query_map(params![from, to], |row| {
            Ok(ChatMessageRow {
                openid: row.get(0)?,
                group_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// 窗口内出现过的 openid（画像蒸馏的活跃集合），去重升序。
    pub fn list_active_openids(&self, from: &str, to: &str) -> anyhow::Result<Vec<String>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT openid FROM chat_history
             WHERE created_at >= ?1 AND created_at < ?2
             ORDER BY openid",
        )?;
        let rows = stmt.query_map(params![from, to], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    // ---- user_profile（全局人物画像，跨群；ADR-0007 决策 9-16） ----

    /// 写入/更新一条画像（`distilled_at` 兼作增量游标）。
    pub fn upsert_profile(
        &self,
        openid: &str,
        profile_text: &str,
        distilled_at: Option<&str>,
        updated_at: &str,
    ) -> anyhow::Result<()> {
        let conn = self.lock();
        conn.execute(
            "INSERT INTO user_profile (openid, profile_text, distilled_at, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(openid) DO UPDATE SET
               profile_text = excluded.profile_text,
               distilled_at = excluded.distilled_at,
               updated_at  = excluded.updated_at",
            params![openid, profile_text, distilled_at, updated_at],
        )?;
        Ok(())
    }

    /// 读一条画像（无 → None）。
    pub fn get_profile(&self, openid: &str) -> anyhow::Result<Option<UserProfile>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT openid, profile_text, distilled_at, updated_at
             FROM user_profile WHERE openid = ?1",
        )?;
        let mut rows = stmt.query_map(params![openid], |row| {
            Ok(UserProfile {
                openid: row.get(0)?,
                profile_text: row.get(1)?,
                distilled_at: row.get(2)?,
                updated_at: row.get(3)?,
            })
        })?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }
}

/// 开发任务记录（dev_tasks 表行）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct DevTask {
    pub session_id: String,
    pub group_id: String,
    pub qq_id: String,
    pub title: Option<String>,
    pub plan_path: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

/// 对话单条（chat_history 表行；用于画像蒸馏）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct ChatMessageRow {
    pub openid: String,
    pub group_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

/// 全局人物画像（user_profile 表行；跨群，ADR-0007）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct UserProfile {
    pub openid: String,
    pub profile_text: String,
    /// 上次蒸馏时刻（增量游标）
    pub distilled_at: Option<String>,
    pub updated_at: String,
}

fn map_dev_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<DevTask> {
    Ok(DevTask {
        session_id: row.get(0)?,
        group_id: row.get(1)?,
        qq_id: row.get(2)?,
        title: row.get(3)?,
        plan_path: row.get(4)?,
        status: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

/// 轻量健康检查：尝试打开并查询 SELECT 1（不建表）。
pub fn check(path: &str) -> bool {
    match Connection::open(path) {
        Ok(conn) => conn.query_row("SELECT 1", [], |_| Ok(())).is_ok(),
        Err(_) => false,
    }
}

fn now_secs() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

/// 迁移：chat_history 旧结构（群级，缺 `openid`/`event_id`）→ 新 per-user 结构。
/// 旧表无任何写入方（历史遗留），发现旧结构则直接重建，数据不作保留。
fn migrate_chat_history(conn: &Connection) -> anyhow::Result<()> {
    let table_exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='chat_history'",
        [],
        |r| r.get(0),
    )?;
    if table_exists == 0 {
        return Ok(());
    }
    let has_openid: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('chat_history') WHERE name='openid'",
        [],
        |r| r.get(0),
    )?;
    if has_openid == 0 {
        conn.execute_batch("DROP TABLE chat_history;")?;
    }
    Ok(())
}

const CREATE_TABLES: &str = "
CREATE TABLE IF NOT EXISTS group_messages (
    id          INTEGER PRIMARY KEY,
    event_id    TEXT    UNIQUE NOT NULL,
    direction   TEXT    NOT NULL,
    group_id    TEXT    NOT NULL,
    qq_id       TEXT,
    content     TEXT,
    raw_payload TEXT,
    created_at  TEXT    NOT NULL
);
CREATE TABLE IF NOT EXISTS game_events (
    seq        INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id   TEXT    UNIQUE NOT NULL,
    game_id    TEXT    NOT NULL,
    room_id    TEXT    NOT NULL,
    event_type TEXT    NOT NULL,
    actor      TEXT,
    payload    TEXT,
    created_at TEXT    NOT NULL
);
CREATE TABLE IF NOT EXISTS game_state (
    game_id    TEXT NOT NULL,
    room_id    TEXT NOT NULL,
    state      TEXT NOT NULL,
    version    INTEGER NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (game_id, room_id)
);
CREATE TABLE IF NOT EXISTS dev_tasks (
    session_id TEXT PRIMARY KEY,
    group_id   TEXT NOT NULL,
    qq_id      TEXT NOT NULL,
    title      TEXT,
    plan_path  TEXT,
    status     TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS chat_history (
    id         INTEGER PRIMARY KEY,
    event_id   TEXT    NOT NULL UNIQUE,
    openid     TEXT    NOT NULL,
    group_id   TEXT    NOT NULL,
    role       TEXT    NOT NULL,
    content    TEXT    NOT NULL,
    created_at TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_chat_history_openid_time ON chat_history(openid, created_at);
CREATE TABLE IF NOT EXISTS user_profile (
    openid       TEXT PRIMARY KEY,
    profile_text TEXT NOT NULL,
    distilled_at TEXT,
    updated_at   TEXT NOT NULL
);
";

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_db(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("tt_{}_{}.db", tag, std::process::id()))
    }

    #[test]
    fn chat_history_insert_idempotent_and_window() {
        let s = Storage::open(":memory:").unwrap();
        s.insert_chat_message("e1", "u1", "g1", "user", "你好", "2026-02-14T00:00:00Z")
            .unwrap();
        s.insert_chat_message("e2", "u1", "g1", "assistant", "回复", "2026-02-14T00:00:01Z")
            .unwrap();
        // 幂等：同 event_id 重复插入不增加
        s.insert_chat_message("e1", "u1", "g1", "user", "你好", "2026-02-14T00:00:00Z")
            .unwrap();
        let rows = s
            .get_chat_window("2026-02-14T00:00:00Z", "2026-02-15T00:00:00Z")
            .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].openid, "u1");
        assert_eq!(rows[0].role, "user");
        assert_eq!(rows[1].role, "assistant");
        // 窗口外（右开区间）
        let none = s
            .get_chat_window("2026-02-15T00:00:00Z", "2026-02-16T00:00:00Z")
            .unwrap();
        assert_eq!(none.len(), 0);
    }

    #[test]
    fn migrates_legacy_chat_history() {
        let path = tmp_db("mig");
        let _ = std::fs::remove_file(&path);
        // 先建旧结构（缺 openid/event_id）
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE chat_history (
                    id INTEGER PRIMARY KEY,
                    group_id TEXT NOT NULL,
                    role TEXT NOT NULL,
                    qq_id TEXT,
                    content TEXT NOT NULL,
                    created_at TEXT NOT NULL);",
            )
            .unwrap();
        }
        // open 触发迁移（旧表被重建为新结构）
        let s = Storage::open(path.to_str().unwrap()).unwrap();
        s.insert_chat_message("e1", "u1", "g1", "user", "hi", "2026-02-14T00:00:00Z")
            .unwrap();
        let rows = s
            .get_chat_window("2026-02-13T00:00:00Z", "2026-02-15T00:00:00Z")
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].content, "hi");
        drop(s);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn profile_upsert_get_and_active_openids() {
        let s = Storage::open(":memory:").unwrap();
        s.insert_chat_message("e1", "u1", "g1", "user", "a", "2026-02-14T01:00:00Z")
            .unwrap();
        s.insert_chat_message("e2", "u2", "g1", "user", "b", "2026-02-14T02:00:00Z")
            .unwrap();
        s.insert_chat_message("e3", "u1", "g1", "user", "c", "2026-02-14T03:00:00Z")
            .unwrap();
        let ids = s
            .list_active_openids("2026-02-14T00:00:00Z", "2026-02-15T00:00:00Z")
            .unwrap();
        assert_eq!(ids, vec!["u1".to_string(), "u2".to_string()]);

        assert!(s.get_profile("u1").unwrap().is_none());
        s.upsert_profile(
            "u1",
            "喜欢竞技",
            Some("2026-02-14T04:00:00Z"),
            "2026-02-14T04:00:00Z",
        )
        .unwrap();
        let p = s.get_profile("u1").unwrap().unwrap();
        assert_eq!(p.profile_text, "喜欢竞技");
        assert_eq!(p.distilled_at.as_deref(), Some("2026-02-14T04:00:00Z"));
        // upsert 覆盖
        s.upsert_profile(
            "u1",
            "喜欢合作",
            Some("2026-02-15T04:00:00Z"),
            "2026-02-15T04:00:00Z",
        )
        .unwrap();
        let p2 = s.get_profile("u1").unwrap().unwrap();
        assert_eq!(p2.profile_text, "喜欢合作");
        assert_eq!(p2.distilled_at.as_deref(), Some("2026-02-15T04:00:00Z"));
    }
}
