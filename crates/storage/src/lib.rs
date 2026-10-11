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
";
