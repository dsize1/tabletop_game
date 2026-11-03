//! 局面/资源模板（`game_state_templates`，工作台预览基座）。
//!
//! 定位（`docs/games/sanguosha/data-model.md` §5、ADR-0033）：
//! - 模板是**预览的输入**，不是对局真相源；只由道具中台/工作台写入；
//! - `snapshot` 存引擎完整 `GameState` JSON（含私有数据），`meta` 存场景注入
//!   `SceneSpec` 等可选附加信息；
//! - 与 `game_state` / `game_events` 完全隔离：预览不会写对局表。

use rusqlite::{params, OptionalExtension};
use serde::Serialize;

use crate::{now_secs, Storage};

/// 一条模板。
#[derive(Debug, Clone, Serialize)]
pub struct TemplateRow {
    pub id: i64,
    pub game_id: String,
    pub name: String,
    pub source: String,
    /// 引擎完整 `GameState` JSON 字符串。
    pub snapshot: String,
    pub meta: Option<String>,
    pub created_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 新建模板入参。
pub struct TemplateInput<'a> {
    pub game_id: &'a str,
    pub name: &'a str,
    pub source: &'a str,
    /// 引擎完整 `GameState` JSON（对象）。
    pub snapshot: &'a str,
    pub meta: Option<&'a str>,
    pub created_by: Option<&'a str>,
}

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TemplateRow> {
    Ok(TemplateRow {
        id: row.get(0)?,
        game_id: row.get(1)?,
        name: row.get(2)?,
        source: row.get(3)?,
        snapshot: row.get(4)?,
        meta: row.get(5)?,
        created_by: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

const COLS: &str = "id, game_id, name, source, snapshot, meta, created_by, created_at, updated_at";

/// 校验模板入参（不依赖 DB，便于单测/服务层复用）。
pub fn validate_input(input: &TemplateInput<'_>) -> anyhow::Result<()> {
    let name = input.name.trim();
    if name.is_empty() {
        anyhow::bail!("模板名称不能为空");
    }
    if name.chars().count() > 120 {
        anyhow::bail!("模板名称过长（≤120 字符）");
    }
    if input.game_id.trim().is_empty() {
        anyhow::bail!("game_id 不能为空");
    }
    let source = input.source.trim();
    let source_ok = source == "manual"
        || source.starts_with("replay:")
        || source.starts_with("room:")
        || source.starts_with("import:")
        || source.starts_with("duplicate:");
    if !source_ok {
        anyhow::bail!(
            "source 非法（应为 manual / replay:<room>@<seq> / room:<room> / import:... / duplicate:<id>）"
        );
    }
    let value: serde_json::Value =
        serde_json::from_str(input.snapshot).map_err(|e| anyhow::anyhow!("snapshot 不是合法 JSON: {e}"))?;
    if !value.is_object() {
        anyhow::bail!("snapshot 必须是 JSON 对象（GameState）");
    }
    if input.snapshot.len() > 8 * 1024 * 1024 {
        anyhow::bail!("snapshot 过大（>8MB）");
    }
    if let Some(meta) = input.meta {
        let value: serde_json::Value =
            serde_json::from_str(meta).map_err(|e| anyhow::anyhow!("meta 不是合法 JSON: {e}"))?;
        if !value.is_object() {
            anyhow::bail!("meta 必须是 JSON 对象");
        }
        if meta.len() > 256 * 1024 {
            anyhow::bail!("meta 过大（>256KB）");
        }
    }
    Ok(())
}

impl Storage {
    /// 新建模板。
    pub fn create_template(&self, input: &TemplateInput<'_>) -> anyhow::Result<TemplateRow> {
        validate_input(input)?;
        let now = now_secs();
        let conn = self.lock();
        conn.execute(
            "INSERT INTO game_state_templates
             (game_id, name, source, snapshot, meta, created_by, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                input.game_id.trim(),
                input.name.trim(),
                input.source.trim(),
                input.snapshot,
                input.meta,
                input.created_by,
                now,
                now
            ],
        )?;
        let id = conn.last_insert_rowid();
        drop(conn);
        self.get_template(id)?
            .ok_or_else(|| anyhow::anyhow!("模板写入后读取失败（id={id}）"))
    }

    /// 读取单条模板。
    pub fn get_template(&self, id: i64) -> anyhow::Result<Option<TemplateRow>> {
        let conn = self.lock();
        let row = conn
            .query_row(
                &format!("SELECT {COLS} FROM game_state_templates WHERE id = ?1"),
                params![id],
                map_row,
            )
            .optional()?;
        Ok(row)
    }

    /// 列表（按 id 倒序）。
    pub fn list_templates(
        &self,
        game_id: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<TemplateRow>> {
        let limit = limit.clamp(1, 500);
        let offset = offset.max(0);
        let conn = self.lock();
        let mut rows = Vec::new();
        match game_id.map(str::trim).filter(|s| !s.is_empty()) {
            Some(game_id) => {
                let mut stmt = conn.prepare(&format!(
                    "SELECT {COLS} FROM game_state_templates WHERE game_id = ?1
                     ORDER BY id DESC LIMIT ?2 OFFSET ?3"
                ))?;
                let iter = stmt.query_map(params![game_id, limit, offset], map_row)?;
                for row in iter {
                    rows.push(row?);
                }
            }
            None => {
                let mut stmt = conn.prepare(&format!(
                    "SELECT {COLS} FROM game_state_templates
                     ORDER BY id DESC LIMIT ?1 OFFSET ?2"
                ))?;
                let iter = stmt.query_map(params![limit, offset], map_row)?;
                for row in iter {
                    rows.push(row?);
                }
            }
        }
        Ok(rows)
    }

    /// 更新模板（只更新提供的字段；`snapshot` 传入时会重新校验）。
    pub fn update_template(
        &self,
        id: i64,
        name: Option<&str>,
        snapshot: Option<&str>,
        meta: Option<Option<&str>>,
    ) -> anyhow::Result<Option<TemplateRow>> {
        let Some(current) = self.get_template(id)? else {
            return Ok(None);
        };
        let name = name.unwrap_or(&current.name).trim();
        let snapshot = snapshot.unwrap_or(&current.snapshot);
        let meta_owned: Option<String> = match meta {
            Some(v) => v.map(str::to_string),
            None => current.meta.clone(),
        };
        let input = TemplateInput {
            game_id: &current.game_id,
            name,
            source: &current.source,
            snapshot,
            meta: meta_owned.as_deref(),
            created_by: current.created_by.as_deref(),
        };
        validate_input(&input)?;
        let conn = self.lock();
        conn.execute(
            "UPDATE game_state_templates
             SET name = ?2, snapshot = ?3, meta = ?4, updated_at = ?5
             WHERE id = ?1",
            params![id, name, snapshot, meta_owned, now_secs()],
        )?;
        drop(conn);
        self.get_template(id)
    }

    /// 复制模板（新 id、新时间；来源标为 `duplicate:<id>`）。
    pub fn duplicate_template(&self, id: i64, name: Option<&str>) -> anyhow::Result<Option<TemplateRow>> {
        let Some(src) = self.get_template(id)? else {
            return Ok(None);
        };
        let new_name = name
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("{}（副本）", src.name));
        let source = format!("duplicate:{id}");
        let input = TemplateInput {
            game_id: &src.game_id,
            name: &new_name,
            source: &source,
            snapshot: &src.snapshot,
            meta: src.meta.as_deref(),
            created_by: src.created_by.as_deref(),
        };
        self.create_template(&input).map(Some)
    }

    /// 删除模板；返回是否真的删除了行。
    pub fn delete_template(&self, id: i64) -> anyhow::Result<bool> {
        let conn = self.lock();
        let n = conn.execute("DELETE FROM game_state_templates WHERE id = ?1", params![id])?;
        Ok(n > 0)
    }

    /// 模板计数（概览/分页用）。
    pub fn count_templates(&self, game_id: Option<&str>) -> anyhow::Result<i64> {
        let conn = self.lock();
        let n = match game_id.map(str::trim).filter(|s| !s.is_empty()) {
            Some(game_id) => conn.query_row(
                "SELECT COUNT(*) FROM game_state_templates WHERE game_id = ?1",
                params![game_id],
                |r| r.get(0),
            )?,
            None => conn.query_row("SELECT COUNT(*) FROM game_state_templates", [], |r| r.get(0))?,
        };
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> String {
        serde_json::json!({
            "game_id": "sanguosha",
            "room_id": "r1",
            "group_id": "g1",
            "status": "active",
            "players": ["p1"],
            "data": { "schema_version": 2 }
        })
        .to_string()
    }

    #[test]
    fn template_crud_and_duplicate() {
        let s = Storage::open(":memory:").unwrap();
        let snap = snapshot();
        let row = s
            .create_template(&TemplateInput {
                game_id: "sanguosha",
                name: "出牌阶段-有杀",
                source: "manual",
                snapshot: &snap,
                meta: Some(r#"{"scene":{"round":2}}"#),
                created_by: Some("u1"),
            })
            .unwrap();
        assert!(row.id > 0);
        assert_eq!(row.name, "出牌阶段-有杀");

        let list = s.list_templates(Some("sanguosha"), 10, 0).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, row.id);
        assert_eq!(s.count_templates(Some("sanguosha")).unwrap(), 1);
        assert_eq!(s.count_templates(Some("other")).unwrap(), 0);

        let updated = s
            .update_template(row.id, Some("改名"), None, Some(Some(r#"{"scene":{}}"#)))
            .unwrap()
            .unwrap();
        assert_eq!(updated.name, "改名");
        assert_eq!(updated.meta.as_deref(), Some(r#"{"scene":{}}"#));
        assert_eq!(updated.snapshot, snap);

        let dup = s.duplicate_template(row.id, Some("副本")).unwrap().unwrap();
        assert_ne!(dup.id, row.id);
        assert_eq!(dup.name, "副本");
        assert!(dup.source.starts_with("duplicate:"));
        assert_eq!(s.count_templates(Some("sanguosha")).unwrap(), 2);

        assert!(s.delete_template(row.id).unwrap());
        assert!(s.get_template(row.id).unwrap().is_none());
        assert!(!s.delete_template(row.id).unwrap());
    }

    #[test]
    fn template_input_validation() {
        let s = Storage::open(":memory:").unwrap();
        let snap = snapshot();
        let cases: Vec<(&str, &str, &str)> = vec![
            ("", "manual", &snap),
            ("x", "bogus", &snap),
            ("x", "manual", "not-json"),
            ("x", "manual", "[]"),
        ];
        for (name, source, snapshot) in cases {
            let r = s.create_template(&TemplateInput {
                game_id: "sanguosha",
                name,
                source,
                snapshot,
                meta: None,
                created_by: None,
            });
            assert!(r.is_err(), "应拒绝 name={name} source={source} snapshot={snapshot}");
        }
        // meta 非对象被拒
        let r = s.create_template(&TemplateInput {
            game_id: "sanguosha",
            name: "x",
            source: "manual",
            snapshot: &snap,
            meta: Some("[]"),
            created_by: None,
        });
        assert!(r.is_err());
        assert_eq!(s.count_templates(None).unwrap(), 0);
    }
}
