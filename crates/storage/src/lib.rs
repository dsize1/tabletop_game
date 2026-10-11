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

    // ---- chat_history ----

    /// 追加一条聊天历史（role: user / assistant）。
    pub fn insert_chat_history(
        &self,
        group_id: &str,
        role: &str,
        qq_id: Option<&str>,
        content: &str,
    ) -> anyhow::Result<()> {
        let conn = self.lock();
        conn.execute(
            "INSERT INTO chat_history (group_id, role, qq_id, content, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![group_id, role, qq_id, content, now_secs()],
        )?;
        Ok(())
    }

    /// 读取群最近 N 条聊天历史（时间升序）。
    pub fn get_chat_history(
        &self,
        group_id: &str,
        limit: i64,
    ) -> anyhow::Result<Vec<ChatTurn>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT role, content FROM chat_history
             WHERE group_id=?1 ORDER BY id DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![group_id, limit], |row| {
            Ok(ChatTurn {
                role: row.get(0)?,
                content: row.get(1)?,
            })
        })?;
        let mut out: Vec<ChatTurn> = rows.collect::<rusqlite::Result<_>>()?;
        out.reverse();
        Ok(out)
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

/// 聊天历史单条（用于注入 LLM 上下文）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct ChatTurn {
    pub role: String,
    pub content: String,
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
    group_id   TEXT NOT NULL,
    role       TEXT NOT NULL,
    qq_id      TEXT,
    content    TEXT NOT NULL,
    created_at TEXT NOT NULL
);
";
