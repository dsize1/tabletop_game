//! SQLite 持久化（真相源）。
//!
//! WAL 模式 + busy_timeout + event_id 唯一键去重（幂等）。
//! 多进程（server / game-engine-mcp）各持连接，WAL 下并发写安全。

pub mod gameprops;

use rusqlite::{params, Connection};
use std::sync::Mutex;

/// 单条待写入的引擎事件（借用避免克隆）。
pub struct GameEventRecord<'a> {
    pub event_id: &'a str,
    pub event_type: &'a str,
    pub actor: Option<&'a str>,
    pub payload: &'a str,
}

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
        // dev_tasks 加列（幂等）：ADR-0015 §3.3（stage/executor/... 投影列）
        ensure_dev_task_columns(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub(crate) fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
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
            params![
                event_id,
                direction,
                group_id,
                qq_id,
                content,
                raw_payload,
                now_secs()
            ],
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
            params![
                event_id,
                game_id,
                room_id,
                event_type,
                actor,
                payload,
                now_secs()
            ],
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

    /// 批量写入引擎事件 + 更新快照（同一事务）；返回最后一条事件的自增 seq。
    /// 无事件时保持原 version。
    pub fn append_game_events(
        &self,
        game_id: &str,
        room_id: &str,
        events: &[GameEventRecord<'_>],
        state_json: &str,
    ) -> anyhow::Result<i64> {
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        let mut last_seq = 0i64;
        for e in events {
            tx.execute(
                "INSERT OR IGNORE INTO game_events
                 (event_id, game_id, room_id, event_type, actor, payload, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    e.event_id,
                    game_id,
                    room_id,
                    e.event_type,
                    e.actor,
                    e.payload,
                    now_secs()
                ],
            )?;
            let seq: i64 = tx.query_row(
                "SELECT seq FROM game_events WHERE event_id = ?1",
                params![e.event_id],
                |row| row.get(0),
            )?;
            if seq > last_seq {
                last_seq = seq;
            }
        }
        // 无事件时保持原 version（不把快照 version 拉回 0）。
        if events.is_empty() {
            last_seq = tx
                .query_row(
                    "SELECT version FROM game_state WHERE game_id = ?1 AND room_id = ?2",
                    params![game_id, room_id],
                    |row| row.get(0),
                )
                .unwrap_or(0);
        }
        tx.execute(
            "INSERT INTO game_state (game_id, room_id, state, version, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(game_id, room_id) DO UPDATE SET
               state=excluded.state, version=excluded.version, updated_at=excluded.updated_at",
            params![game_id, room_id, state_json, last_seq, now_secs()],
        )?;
        tx.commit()?;
        Ok(last_seq)
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

    /// 读取指定对局的快照 state JSON（game_state 真相源）。
    pub fn get_game_snapshot(
        &self,
        game_id: &str,
        room_id: &str,
    ) -> anyhow::Result<Option<String>> {
        let conn = self.lock();
        let mut stmt =
            conn.prepare("SELECT state FROM game_state WHERE game_id = ?1 AND room_id = ?2")?;
        let mut rows = stmt.query(params![game_id, room_id])?;
        if let Some(r) = rows.next()? {
            Ok(Some(r.get(0)?))
        } else {
            Ok(None)
        }
    }

    /// 读取指定对局快照的 version（应等于最后已应用事件的 seq）。
    pub fn game_snapshot_version(
        &self,
        game_id: &str,
        room_id: &str,
    ) -> anyhow::Result<Option<i64>> {
        let conn = self.lock();
        let mut stmt =
            conn.prepare("SELECT version FROM game_state WHERE game_id = ?1 AND room_id = ?2")?;
        let mut rows = stmt.query(params![game_id, room_id])?;
        if let Some(r) = rows.next()? {
            Ok(Some(r.get(0)?))
        } else {
            Ok(None)
        }
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

    /// 记录计划格式标识（`plan6-v1` 等；ADR-0015 §3.3 防 `get_progress` 解析失效）。
    pub fn set_dev_task_plan_format(
        &self,
        session_id: &str,
        plan_format: &str,
    ) -> anyhow::Result<()> {
        let conn = self.lock();
        conn.execute(
            "UPDATE dev_tasks SET plan_format=?1, last_synced_at=?2, updated_at=?2 WHERE session_id=?3",
            params![plan_format, now_secs(), session_id],
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
            "SELECT session_id, group_id, qq_id, title, plan_path, status, created_at, updated_at,
                    stage, executor, executor_ref, contract_version, plan_format, last_synced_at, sync_source
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
                    "SELECT session_id, group_id, qq_id, title, plan_path, status, created_at, updated_at,
                            stage, executor, executor_ref, contract_version, plan_format, last_synced_at, sync_source
                     FROM dev_tasks WHERE status=?1 ORDER BY created_at DESC",
                )?;
                let rows = stmt.query_map(params![s], map_dev_task)?;
                for r in rows {
                    out.push(r?);
                }
            }
            None => {
                let mut stmt = conn.prepare(
                    "SELECT session_id, group_id, qq_id, title, plan_path, status, created_at, updated_at,
                            stage, executor, executor_ref, contract_version, plan_format, last_synced_at, sync_source
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

    // ---- dev_task_events（审计 + 出站通知源；ADR-0015 §3.2）----

    /// 追加一条任务事件（`event_id` 幂等）；返回是否实际插入。
    pub fn append_dev_task_event(
        &self,
        event_id: &str,
        session_id: &str,
        kind: &str,
        actor: Option<&str>,
        payload: &str,
    ) -> anyhow::Result<bool> {
        let conn = self.lock();
        let n = conn.execute(
            "INSERT OR IGNORE INTO dev_task_events
             (event_id, session_id, kind, actor, payload, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![event_id, session_id, kind, actor, payload, now_secs()],
        )?;
        Ok(n > 0)
    }

    /// 按 `since_id`（不含）增量查询任务事件，按 id 升序。
    pub fn list_dev_task_events(
        &self,
        session_id: &str,
        since_id: i64,
        limit: usize,
    ) -> anyhow::Result<Vec<DevTaskEventRow>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT id, event_id, session_id, kind, actor, payload, created_at
             FROM dev_task_events WHERE session_id=?1 AND id>?2 ORDER BY id ASC LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![session_id, since_id, limit as i64], |row| {
            Ok(DevTaskEventRow {
                id: row.get(0)?,
                event_id: row.get(1)?,
                session_id: row.get(2)?,
                kind: row.get(3)?,
                actor: row.get(4)?,
                payload: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    // ---- dev_runs（执行调度；单活跃 run 由唯一索引兼底）----

    /// 插入一条 run（`status=queued`）。同任务已有活跃 run 时报错（唯一索引）。
    pub fn insert_dev_run(
        &self,
        run_id: &str,
        session_id: &str,
        stage: &str,
        executor: &str,
        branch: Option<&str>,
        commit_sha: Option<&str>,
        idempotency_key: Option<&str>,
    ) -> anyhow::Result<()> {
        let conn = self.lock();
        conn.execute(
            "INSERT INTO dev_runs
             (run_id, session_id, stage, executor, status, attempt, branch, commit_sha,
              idempotency_key, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'queued', 1, ?5, ?6, ?7, ?8, ?8)",
            params![
                run_id,
                session_id,
                stage,
                executor,
                branch,
                commit_sha,
                idempotency_key,
                now_secs()
            ],
        )?;
        Ok(())
    }

    /// 推进 run 状态（可选回填 `executor_ref`/错误/结束时间）。
    pub fn set_dev_run_status(
        &self,
        run_id: &str,
        status: &str,
        executor_ref: Option<&str>,
        last_error: Option<&str>,
    ) -> anyhow::Result<()> {
        let conn = self.lock();
        let now = now_secs();
        let ended = matches!(status, "succeeded" | "failed" | "cancelled").then(|| now.clone());
        conn.execute(
            "UPDATE dev_runs SET status=?1,
                executor_ref=COALESCE(?2, executor_ref),
                last_error=COALESCE(?3, last_error),
                started_at=CASE WHEN ?1='running' AND started_at IS NULL THEN ?4 ELSE started_at END,
                ended_at=COALESCE(?5, ended_at),
                updated_at=?4
             WHERE run_id=?6",
            params![status, executor_ref, last_error, now, ended, run_id],
        )?;
        Ok(())
    }

    /// 查同一任务当前活跃 run（queued/running）。
    pub fn get_active_dev_run(&self, session_id: &str) -> anyhow::Result<Option<DevRunRow>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT run_id, session_id, stage, executor, executor_ref, status, attempt, branch,
                    commit_sha, idempotency_key, started_at, ended_at, last_error, created_at, updated_at
             FROM dev_runs WHERE session_id=?1 AND status IN ('queued','running')
             ORDER BY created_at DESC LIMIT 1",
        )?;
        let mut rows = stmt.query_map(params![session_id], map_dev_run)?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    // ---- dev_artifacts（产物：worktree/commit/测试报告/镜像/部署）----

    /// 追加一条产物记录。
    pub fn insert_dev_artifact(
        &self,
        session_id: &str,
        kind: &str,
        r#ref: &str,
        digest: Option<&str>,
    ) -> anyhow::Result<()> {
        let conn = self.lock();
        conn.execute(
            "INSERT INTO dev_artifacts (session_id, kind, ref, digest, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![session_id, kind, r#ref, digest, now_secs()],
        )?;
        Ok(())
    }

    /// 列表任务产物（按 id 升序）。
    pub fn list_dev_artifacts(&self, session_id: &str) -> anyhow::Result<Vec<DevArtifactRow>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, kind, ref, digest, created_at
             FROM dev_artifacts WHERE session_id=?1 ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![session_id], |row| {
            Ok(DevArtifactRow {
                id: row.get(0)?,
                session_id: row.get(1)?,
                kind: row.get(2)?,
                r#ref: row.get(3)?,
                digest: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    // ---- 投影（事件 → dev_tasks；ADR-0015 §3.3）----

    /// 更新任务投影列（`status/stage/executor/...`）；权威在 LangGraph，此处只落影子。
    pub fn update_dev_task_projection(
        &self,
        session_id: &str,
        status: &str,
        stage: Option<&str>,
        executor: Option<&str>,
        executor_ref: Option<&str>,
        plan_format: Option<&str>,
        sync_source: &str,
    ) -> anyhow::Result<()> {
        let conn = self.lock();
        let now = now_secs();
        conn.execute(
            "UPDATE dev_tasks SET status=?1,
                stage=COALESCE(?2, stage),
                executor=COALESCE(?3, executor),
                executor_ref=COALESCE(?4, executor_ref),
                plan_format=COALESCE(?5, plan_format),
                sync_source=?6,
                last_synced_at=?7,
                updated_at=?7
             WHERE session_id=?8",
            params![
                status,
                stage,
                executor,
                executor_ref,
                plan_format,
                sync_source,
                now,
                session_id
            ],
        )?;
        Ok(())
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
    pub fn get_chat_window(&self, from: &str, to: &str) -> anyhow::Result<Vec<ChatMessageRow>> {
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
///
/// `stage/executor/executor_ref/...` 为 v2 投影列（ADR-0015 §3.3）：
/// 状态权威在 LangGraph 后，本表**只作投影**，不得用于合法性判定。
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
    pub stage: Option<String>,
    pub executor: Option<String>,
    pub executor_ref: Option<String>,
    pub contract_version: Option<i64>,
    pub plan_format: Option<String>,
    pub last_synced_at: Option<String>,
    pub sync_source: Option<String>,
}

/// 开发任务事件（`dev_task_events` 行；审计 + 出站通知源）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct DevTaskEventRow {
    pub id: i64,
    pub event_id: String,
    pub session_id: String,
    pub kind: String,
    pub actor: Option<String>,
    pub payload: Option<String>,
    pub created_at: String,
}

/// 执行 run（`dev_runs` 行；单活跃 run 约束在这里生效）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct DevRunRow {
    pub run_id: String,
    pub session_id: String,
    pub stage: String,
    pub executor: String,
    pub executor_ref: Option<String>,
    pub status: String,
    pub attempt: i64,
    pub branch: Option<String>,
    pub commit_sha: Option<String>,
    pub idempotency_key: Option<String>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 产物（`dev_artifacts` 行；worktree / commit / 测试报告 / 镜像 / 部署）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct DevArtifactRow {
    pub id: i64,
    pub session_id: String,
    pub kind: String,
    pub r#ref: String,
    pub digest: Option<String>,
    pub created_at: String,
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
        stage: row.get(8)?,
        executor: row.get(9)?,
        executor_ref: row.get(10)?,
        contract_version: row.get(11)?,
        plan_format: row.get(12)?,
        last_synced_at: row.get(13)?,
        sync_source: row.get(14)?,
    })
}

fn map_dev_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<DevRunRow> {
    Ok(DevRunRow {
        run_id: row.get(0)?,
        session_id: row.get(1)?,
        stage: row.get(2)?,
        executor: row.get(3)?,
        executor_ref: row.get(4)?,
        status: row.get(5)?,
        attempt: row.get(6)?,
        branch: row.get(7)?,
        commit_sha: row.get(8)?,
        idempotency_key: row.get(9)?,
        started_at: row.get(10)?,
        ended_at: row.get(11)?,
        last_error: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

/// 轻量健康检查：尝试打开并查询 SELECT 1（不建表）。
pub fn check(path: &str) -> bool {
    match Connection::open(path) {
        Ok(conn) => conn.query_row("SELECT 1", [], |_| Ok(())).is_ok(),
        Err(_) => false,
    }
}

pub(crate) fn now_secs() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

/// 给旧库的 `dev_tasks` 补投影列（幂等；ADR-0015 §3.3）。
///
/// 只新增 NULL 默认列，不重建表、不改既有数据。
fn ensure_dev_task_columns(conn: &Connection) -> anyhow::Result<()> {
    const COLS: &[(&str, &str)] = &[
        ("stage", "TEXT"),
        ("executor", "TEXT"),
        ("executor_ref", "TEXT"),
        ("contract_version", "INTEGER"),
        ("plan_format", "TEXT"),
        ("last_synced_at", "TEXT"),
        ("sync_source", "TEXT"),
    ];
    for (name, ty) in COLS {
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('dev_tasks') WHERE name=?1",
            params![name],
            |r| r.get(0),
        )?;
        if exists == 0 {
            conn.execute(&format!("ALTER TABLE dev_tasks ADD COLUMN {name} {ty}"), [])?;
        }
    }
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_dev_tasks_status ON dev_tasks(status, updated_at);",
    )?;
    Ok(())
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
CREATE TABLE IF NOT EXISTS dev_task_events (
    id          INTEGER PRIMARY KEY,
    event_id    TEXT NOT NULL UNIQUE,
    session_id  TEXT NOT NULL,
    kind        TEXT NOT NULL,
    actor       TEXT,
    payload     TEXT,
    created_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_dev_task_events_session ON dev_task_events(session_id, id);
CREATE TABLE IF NOT EXISTS dev_runs (
    run_id          TEXT PRIMARY KEY,
    session_id      TEXT NOT NULL,
    stage           TEXT NOT NULL,
    executor        TEXT NOT NULL,
    executor_ref    TEXT,
    status          TEXT NOT NULL,
    attempt         INTEGER NOT NULL DEFAULT 1,
    branch          TEXT,
    commit_sha      TEXT,
    idempotency_key TEXT UNIQUE,
    started_at      TEXT,
    ended_at        TEXT,
    last_error      TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);
-- 单活跃 run：同一任务同时只能有一个 queued/running（ADR-0015 §2.4 第 5 项）
CREATE UNIQUE INDEX IF NOT EXISTS uq_dev_runs_active
    ON dev_runs(session_id) WHERE status IN ('queued','running');
CREATE TABLE IF NOT EXISTS dev_artifacts (
    id          INTEGER PRIMARY KEY,
    session_id  TEXT NOT NULL,
    kind        TEXT NOT NULL,
    ref         TEXT NOT NULL,
    digest      TEXT,
    created_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_dev_artifacts_session ON dev_artifacts(session_id, id);
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
-- ===== 桌游道具中台（三国杀 P1）=====
CREATE TABLE IF NOT EXISTS prop_packs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id     TEXT    NOT NULL,
    kind        TEXT    NOT NULL,
    code        TEXT    NOT NULL,
    name        TEXT    NOT NULL,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    enabled     INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT    NOT NULL,
    updated_at  TEXT    NOT NULL,
    UNIQUE (game_id, kind, code)
);
CREATE TABLE IF NOT EXISTS prop_assets (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id     TEXT    NOT NULL,
    kind        TEXT    NOT NULL,
    path        TEXT    NOT NULL,
    mime        TEXT    NOT NULL,
    width       INTEGER,
    height      INTEGER,
    bytes       INTEGER,
    sha256      TEXT,
    created_at  TEXT    NOT NULL,
    UNIQUE (game_id, path)
);
CREATE TABLE IF NOT EXISTS sgs_identities (
    code            TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    team            TEXT NOT NULL CHECK (team IN ('lord','rebel','spy')),
    win_condition   TEXT NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    image_asset_id  INTEGER REFERENCES prop_assets(id),
    sort_order      INTEGER NOT NULL DEFAULT 0,
    status          TEXT NOT NULL DEFAULT 'draft'
                    CHECK (status IN ('draft','testing','review','live','offline')),
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS sgs_identity_setup (
    player_count    INTEGER NOT NULL CHECK (player_count BETWEEN 5 AND 8),
    variant         TEXT    NOT NULL DEFAULT 'default'
                    CHECK (variant IN ('default','double_spy')),
    identity_code   TEXT    NOT NULL REFERENCES sgs_identities(code),
    count           INTEGER NOT NULL CHECK (count >= 0),
    PRIMARY KEY (player_count, variant, identity_code)
);
CREATE TABLE IF NOT EXISTS sgs_card_defs (
    code            TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    category        TEXT NOT NULL CHECK (category IN
                      ('basic','weapon','armor','horse','treasure','trick','delayed_trick')),
    description     TEXT NOT NULL DEFAULT '',
    image_asset_id  INTEGER REFERENCES prop_assets(id),
    range_value     INTEGER,
    target_mode     TEXT CHECK (target_mode IN ('single','aoe')),
    equip_slot      TEXT CHECK (equip_slot IN
                      ('weapon','armor','horse_offense','horse_defense','treasure')),
    range_type      TEXT CHECK (range_type IN ('attack','defense')),
    status          TEXT NOT NULL DEFAULT 'draft'
                    CHECK (status IN ('draft','testing','review','live','offline')),
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    CHECK ((category IN ('weapon','armor','horse','treasure')) = (equip_slot IS NOT NULL)),
    CHECK ((category = 'horse') = (range_type IS NOT NULL)),
    CHECK (range_type IS NULL OR
           (range_type = 'attack'  AND equip_slot = 'horse_offense' AND range_value = 1) OR
           (range_type = 'defense' AND equip_slot = 'horse_defense' AND range_value = 1))
);
CREATE TABLE IF NOT EXISTS sgs_cards (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    pack_id         INTEGER NOT NULL REFERENCES prop_packs(id),
    def_code        TEXT    NOT NULL REFERENCES sgs_card_defs(code),
    suit            TEXT CHECK (suit IN ('spade','heart','club','diamond')),
    rank            INTEGER CHECK (rank BETWEEN 1 AND 13),
    image_asset_id  INTEGER REFERENCES prop_assets(id),
    status          TEXT NOT NULL DEFAULT 'draft'
                    CHECK (status IN ('draft','testing','review','live','offline')),
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_sgs_cards_unique_full
    ON sgs_cards (pack_id, def_code, suit, rank)
    WHERE suit IS NOT NULL AND rank IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_sgs_cards_def ON sgs_cards(def_code);
CREATE TABLE IF NOT EXISTS sgs_generals (
    code            TEXT PRIMARY KEY,
    external_id     TEXT UNIQUE,
    name            TEXT NOT NULL,
    kingdom         TEXT NOT NULL CHECK (kingdom IN ('wei','shu','wu','qun','jin','god')),
    gender          TEXT NOT NULL DEFAULT 'unknown'
                    CHECK (gender IN ('male','female','unknown')),
    pack_id         INTEGER NOT NULL REFERENCES prop_packs(id),
    max_hp          REAL NOT NULL CHECK (max_hp > 0 AND max_hp <= 15),
    initial_hp      REAL NOT NULL DEFAULT 0 CHECK (initial_hp >= 0 AND initial_hp <= 15),
    quality         TEXT,
    description     TEXT NOT NULL DEFAULT '',
    biography       TEXT NOT NULL DEFAULT '',
    image_asset_id  INTEGER REFERENCES prop_assets(id),
    status          TEXT NOT NULL DEFAULT 'draft'
                    CHECK (status IN ('draft','testing','review','live','offline')),
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    CHECK (initial_hp = 0 OR initial_hp <= max_hp)
);
CREATE INDEX IF NOT EXISTS idx_sgs_generals_pack ON sgs_generals(pack_id);
CREATE TABLE IF NOT EXISTS sgs_skills (
    code            TEXT PRIMARY KEY,
    external_id     TEXT UNIQUE,
    name            TEXT NOT NULL,
    type            TEXT NOT NULL CHECK (type IN
                      ('lord','normal','locked','limited','awakening')),
    description     TEXT NOT NULL DEFAULT '',
    status          TEXT NOT NULL DEFAULT 'draft'
                    CHECK (status IN ('draft','testing','review','live','offline')),
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_sgs_skills_name ON sgs_skills(name);
CREATE TABLE IF NOT EXISTS sgs_general_skills (
    general_code    TEXT    NOT NULL REFERENCES sgs_generals(code),
    skill_code      TEXT    NOT NULL REFERENCES sgs_skills(code),
    sort_order      INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (general_code, skill_code)
);
CREATE TABLE IF NOT EXISTS prop_status_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id     TEXT    NOT NULL,
    entity_kind TEXT    NOT NULL CHECK (entity_kind IN
                  ('identity','card_def','card','general','skill')),
    entity_code TEXT    NOT NULL,
    from_status TEXT,
    to_status   TEXT    NOT NULL,
    actor       TEXT,
    note        TEXT,
    created_at  TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_prop_status_log_entity
    ON prop_status_log (game_id, entity_kind, entity_code, created_at);
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
        s.insert_chat_message(
            "e2",
            "u1",
            "g1",
            "assistant",
            "回复",
            "2026-02-14T00:00:01Z",
        )
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
    fn append_game_events_aligns_version_to_last_seq() {
        let s = Storage::open(":memory:").unwrap();
        let e1 = GameEventRecord {
            event_id: "ev1",
            event_type: "game_created",
            actor: None,
            payload: "{}",
        };
        let seq1 = s
            .append_game_events("las-vegas", "room1", &[e1], "{\"a\":1}")
            .unwrap();
        assert_eq!(seq1, 1);
        assert_eq!(
            s.game_snapshot_version("las-vegas", "room1").unwrap(),
            Some(1)
        );

        let e2 = GameEventRecord {
            event_id: "ev2",
            event_type: "dice_rolled",
            actor: Some("p1"),
            payload: "{}",
        };
        let e3 = GameEventRecord {
            event_id: "ev3",
            event_type: "dice_placed",
            actor: Some("p1"),
            payload: "{}",
        };
        let seq2 = s
            .append_game_events("las-vegas", "room1", &[e2, e3], "{\"a\":2}")
            .unwrap();
        assert_eq!(seq2, seq1 + 2);
        assert_eq!(
            s.game_snapshot_version("las-vegas", "room1").unwrap(),
            Some(seq2)
        );

        // 幂等：重复 event_id 不新增事件，version 不回退
        let dup = GameEventRecord {
            event_id: "ev3",
            event_type: "dice_placed",
            actor: Some("p1"),
            payload: "{}",
        };
        let seq3 = s
            .append_game_events("las-vegas", "room1", &[dup], "{\"a\":3}")
            .unwrap();
        assert_eq!(seq3, seq2);
        assert_eq!(
            s.game_snapshot_version("las-vegas", "room1").unwrap(),
            Some(seq2)
        );
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

    // ---- dev v2 底座（ADR-0015 §3.2/§3.3）----

    #[test]
    fn dev_schema_migration_idempotent_on_legacy_db() {
        let path = tmp_db("devschema");
        let _ = std::fs::remove_file(&path);
        // 旧库：只有 v1 的 8 列（无 stage/executor/...）
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE dev_tasks (
                    session_id TEXT PRIMARY KEY,
                    group_id   TEXT NOT NULL,
                    qq_id      TEXT NOT NULL,
                    title      TEXT,
                    plan_path  TEXT,
                    status     TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL);
                 INSERT INTO dev_tasks (session_id,group_id,qq_id,status,created_at,updated_at)
                 VALUES ('s1','g1','u1','queued','1','1');",
            )
            .unwrap();
        }
        // 连续 open 两次：迁移必须幂等（第二次不报错、不丢数据）
        drop(Storage::open(path.to_str().unwrap()).unwrap());
        let s = Storage::open(path.to_str().unwrap()).unwrap();

        let t = s.get_dev_task("s1").unwrap().unwrap();
        assert_eq!(t.status, "queued");
        assert!(t.stage.is_none(), "旧数据投影列为 NULL");

        let tables: i64 = s
            .lock()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type='table' AND name IN ('dev_task_events','dev_runs','dev_artifacts')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tables, 3, "三张新表应已创建");

        drop(s);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn dev_events_idempotent_and_incremental() {
        let s = Storage::open(":memory:").unwrap();
        assert!(s
            .append_dev_task_event("e1", "s1", "created", Some("u1"), "{}")
            .unwrap());
        // 幂等：同 event_id 不重复插入
        assert!(!s
            .append_dev_task_event("e1", "s1", "created", Some("u1"), "{}")
            .unwrap());
        s.append_dev_task_event("e2", "s1", "plan_ready", None, "{}")
            .unwrap();
        s.append_dev_task_event("e3", "s2", "created", None, "{}")
            .unwrap();

        let all = s.list_dev_task_events("s1", 0, 10).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].kind, "created");
        // since_id 不含自身
        let after = s.list_dev_task_events("s1", all[0].id, 10).unwrap();
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].kind, "plan_ready");
        // 任务间隔离
        assert_eq!(s.list_dev_task_events("s2", 0, 10).unwrap().len(), 1);
    }

    #[test]
    fn dev_run_single_active_and_status_transition() {
        let s = Storage::open(":memory:").unwrap();
        s.insert_dev_run("r1", "s1", "plan", "langgraph", None, None, Some("k1"))
            .unwrap();
        // 同任务已有活跃 run → 插入失败（唯一索引）
        assert!(s
            .insert_dev_run("r2", "s1", "execute", "langgraph", None, None, Some("k2"))
            .is_err());
        // 不同任务互不影响
        assert!(s
            .insert_dev_run("r3", "s2", "plan", "langgraph", None, None, Some("k3"))
            .is_ok());

        s.set_dev_run_status("r1", "running", Some("thread-1"), None)
            .unwrap();
        let active = s.get_active_dev_run("s1").unwrap().unwrap();
        assert_eq!(active.status, "running");
        assert_eq!(active.executor_ref.as_deref(), Some("thread-1"));
        assert!(active.started_at.is_some());

        // 终态后不再占用「单活跃」名额
        s.set_dev_run_status("r1", "succeeded", None, None).unwrap();
        assert!(s.get_active_dev_run("s1").unwrap().is_none());
        assert!(s
            .insert_dev_run("r4", "s1", "verify", "langgraph", None, None, Some("k4"))
            .is_ok());
    }

    #[test]
    fn dev_run_concurrent_insert_single_winner() {
        use std::sync::{Arc, Barrier};
        let path = tmp_db("runconc");
        let _ = std::fs::remove_file(&path);
        let s1 = Arc::new(Storage::open(path.to_str().unwrap()).unwrap());
        let s2 = Arc::new(Storage::open(path.to_str().unwrap()).unwrap());
        let barrier = Arc::new(Barrier::new(2));

        let mut handles = Vec::new();
        for (i, s) in [(0, s1.clone()), (1, s2.clone())] {
            let b = barrier.clone();
            handles.push(std::thread::spawn(move || {
                b.wait();
                s.insert_dev_run(
                    &format!("r{i}"),
                    "s-conc",
                    "plan",
                    "langgraph",
                    None,
                    None,
                    Some(&format!("k{i}")),
                )
                .is_ok()
            }));
        }
        let ok_count = handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .filter(|ok| *ok)
            .count();
        assert_eq!(ok_count, 1, "并发插入应只有一个成功（单活跃唯一索引）");
        assert!(s1.get_active_dev_run("s-conc").unwrap().is_some());
        drop((s1, s2));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn dev_projection_and_artifacts() {
        let s = Storage::open(":memory:").unwrap();
        s.create_dev_task("s1", "g1", "u1", "queued").unwrap();
        s.update_dev_task_projection(
            "s1",
            "awaiting_confirm",
            Some("plan"),
            Some("langgraph"),
            Some("thread-1"),
            Some("plan6-v1"),
            "n8n",
        )
        .unwrap();
        let t = s.get_dev_task("s1").unwrap().unwrap();
        assert_eq!(t.status, "awaiting_confirm");
        assert_eq!(t.stage.as_deref(), Some("plan"));
        assert_eq!(t.executor.as_deref(), Some("langgraph"));
        assert_eq!(t.executor_ref.as_deref(), Some("thread-1"));
        assert_eq!(t.plan_format.as_deref(), Some("plan6-v1"));
        assert_eq!(t.sync_source.as_deref(), Some("n8n"));
        assert!(t.last_synced_at.is_some());

        // COALESCE 语义：None 不覆盖已有值
        s.update_dev_task_projection("s1", "running", None, None, None, None, "bridge")
            .unwrap();
        let t2 = s.get_dev_task("s1").unwrap().unwrap();
        assert_eq!(t2.status, "running");
        assert_eq!(t2.stage.as_deref(), Some("plan"));
        assert_eq!(t2.sync_source.as_deref(), Some("bridge"));

        s.insert_dev_artifact("s1", "commit", "deadbeef", None)
            .unwrap();
        s.insert_dev_artifact("s1", "test_report", "logs/ci.json", Some("md5:1"))
            .unwrap();
        let arts = s.list_dev_artifacts("s1").unwrap();
        assert_eq!(arts.len(), 2);
        assert_eq!(arts[0].kind, "commit");
        assert_eq!(arts[1].r#ref, "logs/ci.json");
        assert_eq!(arts[1].digest.as_deref(), Some("md5:1"));
    }
}
