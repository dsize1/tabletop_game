//! 桌游道具中台（P1）：通用表读写、生命周期与官网武将导入。
//!
//! 只允许白名单表/列；写路径仍由 server 单点调用。

use crate::Storage;
use rusqlite::types::{Value as SqlValue, ValueRef};
use rusqlite::{params, params_from_iter, Row};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

pub const GAME_ID: &str = "sanguosha";

const TABLES: &[&str] = &[
    "prop_packs",
    "prop_assets",
    "sgs_identities",
    "sgs_identity_setup",
    "sgs_card_defs",
    "sgs_cards",
    "sgs_generals",
    "sgs_skills",
    "sgs_general_skills",
    "prop_status_log",
];

pub fn is_table(name: &str) -> bool {
    TABLES.contains(&name)
}

fn columns(table: &str) -> &'static [&'static str] {
    match table {
        "prop_packs" => &[
            "id",
            "game_id",
            "kind",
            "code",
            "name",
            "sort_order",
            "enabled",
            "created_at",
            "updated_at",
        ],
        "prop_assets" => &[
            "id",
            "game_id",
            "kind",
            "path",
            "mime",
            "width",
            "height",
            "bytes",
            "sha256",
            "created_at",
        ],
        "sgs_identities" => &[
            "code",
            "name",
            "team",
            "win_condition",
            "description",
            "image_asset_id",
            "sort_order",
            "status",
            "created_at",
            "updated_at",
        ],
        "sgs_identity_setup" => &["player_count", "variant", "identity_code", "count"],
        "sgs_card_defs" => &[
            "code",
            "name",
            "category",
            "description",
            "image_asset_id",
            "range_value",
            "target_mode",
            "equip_slot",
            "range_type",
            "status",
            "created_at",
            "updated_at",
        ],
        "sgs_cards" => &[
            "id",
            "pack_id",
            "def_code",
            "suit",
            "rank",
            "image_asset_id",
            "status",
            "created_at",
            "updated_at",
        ],
        "sgs_generals" => &[
            "code",
            "external_id",
            "name",
            "kingdom",
            "gender",
            "pack_id",
            "max_hp",
            "initial_hp",
            "quality",
            "description",
            "biography",
            "image_asset_id",
            "status",
            "created_at",
            "updated_at",
        ],
        "sgs_skills" => &[
            "code",
            "external_id",
            "name",
            "type",
            "description",
            "status",
            "created_at",
            "updated_at",
        ],
        "sgs_general_skills" => &["general_code", "skill_code", "sort_order"],
        "prop_status_log" => &[
            "id",
            "game_id",
            "entity_kind",
            "entity_code",
            "from_status",
            "to_status",
            "actor",
            "note",
            "created_at",
        ],
        _ => &[],
    }
}

fn pk_columns(table: &str, row: &Map<String, Value>) -> Option<Vec<&'static str>> {
    match table {
        "prop_packs" => Some(vec!["game_id", "kind", "code"]),
        "prop_assets" => Some(vec!["game_id", "path"]),
        "sgs_identities" => Some(vec!["code"]),
        "sgs_identity_setup" => Some(vec!["player_count", "variant", "identity_code"]),
        "sgs_card_defs" => Some(vec!["code"]),
        "sgs_cards" => row.get("id").map(|_| vec!["id"]),
        "sgs_generals" => Some(vec!["code"]),
        "sgs_skills" => Some(vec!["code"]),
        "sgs_general_skills" => Some(vec!["general_code", "skill_code"]),
        "prop_status_log" => row.get("id").map(|_| vec!["id"]),
        _ => None,
    }
}

fn has_status(table: &str) -> bool {
    matches!(
        table,
        "sgs_identities" | "sgs_card_defs" | "sgs_cards" | "sgs_generals" | "sgs_skills"
    )
}

fn row_to_json(row: &Row<'_>) -> rusqlite::Result<Value> {
    let mut map = Map::new();
    for i in 0..row.as_ref().column_count() {
        let name = row.as_ref().column_name(i)?.to_string();
        let v = match row.get_ref(i)? {
            ValueRef::Null => Value::Null,
            ValueRef::Integer(n) => json!(n),
            ValueRef::Real(f) => json!(f),
            ValueRef::Text(t) => json!(String::from_utf8_lossy(t).to_string()),
            ValueRef::Blob(b) => json!(format!("<blob:{}>", b.len())),
        };
        map.insert(name, v);
    }
    Ok(Value::Object(map))
}

fn sql_value(v: &Value) -> SqlValue {
    match v {
        Value::Null => SqlValue::Null,
        Value::Bool(b) => SqlValue::Integer(i64::from(*b)),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                SqlValue::Integer(i)
            } else if let Some(f) = n.as_f64() {
                SqlValue::Real(f)
            } else {
                SqlValue::Null
            }
        }
        Value::String(s) => SqlValue::Text(s.clone()),
        _ => SqlValue::Text(v.to_string()),
    }
}

fn query_all(
    conn: &rusqlite::Connection,
    sql: &str,
    args: Vec<SqlValue>,
) -> anyhow::Result<Vec<Value>> {
    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query(params_from_iter(args))?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        out.push(row_to_json(row)?);
    }
    Ok(out)
}

fn fnv1a(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn now() -> String {
    crate::now_secs()
}

impl Storage {
    /// 通用列表：status/q/limit/offset（只允许白名单表）。
    pub fn list_gameprops(
        &self,
        table: &str,
        status: Option<&str>,
        q: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<Value>> {
        if !is_table(table) {
            anyhow::bail!("未知表 {table}");
        }
        let mut sql = format!("SELECT * FROM {table} WHERE 1=1");
        let mut args: Vec<SqlValue> = Vec::new();
        if let Some(st) = status {
            if has_status(table) {
                sql.push_str(" AND status = ?");
                args.push(SqlValue::Text(st.to_string()));
            }
        }
        if let Some(q) = q.map(str::trim).filter(|s| !s.is_empty()) {
            let search_cols: &[&str] = match table {
                "sgs_cards" => &["def_code"],
                "sgs_general_skills" => &["general_code", "skill_code"],
                "sgs_identity_setup" => &["identity_code"],
                _ => &["code", "name"],
            };
            let mut parts = Vec::new();
            for col in search_cols {
                parts.push(format!("{col} LIKE ?"));
                args.push(SqlValue::Text(format!("%{q}%")));
            }
            if !parts.is_empty() {
                sql.push_str(" AND (");
                sql.push_str(&parts.join(" OR "));
                sql.push(')');
            }
        }
        sql.push_str(" ORDER BY ");
        sql.push_str(match table {
            "sgs_cards" | "prop_packs" | "prop_assets" => "id",
            "sgs_identity_setup" => "player_count, variant, identity_code",
            "sgs_general_skills" => "general_code, sort_order, skill_code",
            _ => "code",
        });
        sql.push_str(" LIMIT ? OFFSET ?");
        args.push(SqlValue::Integer(limit.clamp(1, 500)));
        args.push(SqlValue::Integer(offset.max(0)));
        let conn = self.lock();
        query_all(&conn, &sql, args)
    }

    /// 通用 upsert（按白名单列与主键）；返回写入后的行。
    pub fn upsert_gameprops(
        &self,
        table: &str,
        data: &Map<String, Value>,
    ) -> anyhow::Result<Value> {
        if !is_table(table) {
            anyhow::bail!("未知表 {table}");
        }
        let allowed = columns(table);
        let mut row = Map::new();
        for (k, v) in data {
            if allowed.contains(&k.as_str()) {
                row.insert(k.clone(), v.clone());
            }
        }
        // 自动时间戳
        if allowed.contains(&"created_at") && !row.contains_key("created_at") {
            row.insert("created_at".into(), json!(now()));
        }
        if allowed.contains(&"updated_at") {
            row.insert("updated_at".into(), json!(now()));
        }
        // 非 sgs_cards 不允许外部指定 id
        if table != "sgs_cards" {
            row.remove("id");
        }
        let cols: Vec<String> = row.keys().cloned().collect();
        if cols.is_empty() {
            anyhow::bail!("空数据");
        }
        let placeholders = vec!["?"; cols.len()].join(",");
        let pk = pk_columns(table, &row);
        let sql = if let Some(pk_cols) = pk
            .as_ref()
            .filter(|p| p.iter().all(|c| row.contains_key(*c)))
        {
            let conflict = pk_cols.join(",");
            let updates: Vec<String> = cols
                .iter()
                .filter(|c| !pk_cols.contains(&c.as_str()) && c.as_str() != "created_at")
                .map(|c| format!("{c}=excluded.{c}"))
                .collect();
            if updates.is_empty() {
                format!("INSERT INTO {table} ({}) VALUES ({placeholders}) ON CONFLICT({conflict}) DO NOTHING", cols.join(","))
            } else {
                format!(
                    "INSERT INTO {table} ({}) VALUES ({placeholders}) ON CONFLICT({conflict}) DO UPDATE SET {}",
                    cols.join(","),
                    updates.join(",")
                )
            }
        } else {
            format!(
                "INSERT INTO {table} ({}) VALUES ({placeholders})",
                cols.join(",")
            )
        };
        let args: Vec<SqlValue> = cols.iter().map(|c| sql_value(&row[c])).collect();
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        tx.execute(&sql, params_from_iter(args))?;
        let inserted_id = tx.last_insert_rowid();
        let result = if let Some(pk_cols) = pk
            .as_ref()
            .filter(|p| p.iter().all(|c| row.contains_key(*c)))
        {
            let where_sql = pk_cols
                .iter()
                .map(|c| format!("{c}=?"))
                .collect::<Vec<_>>()
                .join(" AND ");
            let args: Vec<SqlValue> = pk_cols.iter().map(|c| sql_value(&row[*c])).collect();
            let mut stmt = tx.prepare(&format!("SELECT * FROM {table} WHERE {where_sql}"))?;
            let mut rows = stmt.query(params_from_iter(args))?;
            match rows.next()? {
                Some(r) => row_to_json(r)?,
                None => Value::Null,
            }
        } else {
            let mut stmt = tx.prepare(&format!("SELECT * FROM {table} WHERE rowid = ?"))?;
            let mut rows = stmt.query(params![inserted_id])?;
            match rows.next()? {
                Some(r) => row_to_json(r)?,
                None => Value::Null,
            }
        };
        tx.commit()?;
        Ok(result)
    }

    /// 生命周期：更新 status + 写 prop_status_log。
    pub fn set_gameprops_status(
        &self,
        table: &str,
        entity_code: &str,
        to_status: &str,
        actor: &str,
        note: Option<&str>,
    ) -> anyhow::Result<Value> {
        if !has_status(table) {
            anyhow::bail!("表 {table} 不支持生命周期");
        }
        let pk = if table == "sgs_cards" { "id" } else { "code" };
        let kind = match table {
            "sgs_identities" => "identity",
            "sgs_card_defs" => "card_def",
            "sgs_cards" => "card",
            "sgs_generals" => "general",
            "sgs_skills" => "skill",
            _ => anyhow::bail!("未知实体"),
        };
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        let old: Option<String> = tx
            .query_row(
                &format!("SELECT status FROM {table} WHERE {pk} = ?"),
                params![entity_code],
                |r| r.get(0),
            )
            .ok();
        let Some(old_status) = old else {
            anyhow::bail!("记录不存在");
        };
        if columns(table).contains(&"updated_at") {
            tx.execute(
                &format!("UPDATE {table} SET status = ?, updated_at = ? WHERE {pk} = ?"),
                params![to_status, now(), entity_code],
            )?;
        } else {
            tx.execute(
                &format!("UPDATE {table} SET status = ? WHERE {pk} = ?"),
                params![to_status, entity_code],
            )?;
        }
        tx.execute(
            "INSERT INTO prop_status_log
             (game_id, entity_kind, entity_code, from_status, to_status, actor, note, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                GAME_ID,
                kind,
                entity_code,
                old_status,
                to_status,
                actor,
                note,
                now()
            ],
        )?;
        let out = {
            let mut stmt = tx.prepare(&format!("SELECT * FROM {table} WHERE {pk} = ?"))?;
            let mut rows = stmt.query(params![entity_code])?;
            match rows.next()? {
                Some(r) => row_to_json(r)?,
                None => Value::Null,
            }
        };
        tx.commit()?;
        Ok(out)
    }

    /// 初始化身份、身份配比、武将包/牌堆包（幂等）。
    pub fn seed_sgs_defaults(&self) -> anyhow::Result<Value> {
        let identities = [
            (
                "lord",
                "主公",
                "lord",
                "消灭所有反贼与内奸，主公存活",
                "身份公开；体力上限与初始体力 +1",
            ),
            (
                "loyalist",
                "忠臣",
                "lord",
                "保护主公，消灭所有反贼与内奸",
                "与主公同阵营",
            ),
            (
                "rebel",
                "反贼",
                "rebel",
                "杀死主公（不要求清场）",
                "主公死亡即反贼胜利",
            ),
            (
                "spy",
                "内奸",
                "spy",
                "除主公外其他玩家全部死亡后，由内奸杀死主公",
                "双内局互不结盟",
            ),
        ];
        for (idx, (code, name, team, win, desc)) in identities.iter().enumerate() {
            let mut row = Map::new();
            row.insert("code".into(), json!(code));
            row.insert("name".into(), json!(name));
            row.insert("team".into(), json!(team));
            row.insert("win_condition".into(), json!(win));
            row.insert("description".into(), json!(desc));
            row.insert("sort_order".into(), json!(idx as i64));
            row.insert("status".into(), json!("live"));
            self.upsert_gameprops("sgs_identities", &row)?;
        }
        // 5–8 人标准配比 + 8 人双内
        let setups: &[(i64, &str, &[(&str, i64)])] = &[
            (
                5,
                "default",
                &[("lord", 1), ("loyalist", 1), ("rebel", 2), ("spy", 1)],
            ),
            (
                6,
                "default",
                &[("lord", 1), ("loyalist", 1), ("rebel", 3), ("spy", 1)],
            ),
            (
                7,
                "default",
                &[("lord", 1), ("loyalist", 2), ("rebel", 3), ("spy", 1)],
            ),
            (
                8,
                "default",
                &[("lord", 1), ("loyalist", 2), ("rebel", 4), ("spy", 1)],
            ),
            (
                8,
                "double_spy",
                &[("lord", 1), ("loyalist", 1), ("rebel", 4), ("spy", 2)],
            ),
        ];
        for (players, variant, rows) in setups {
            for (identity, count) in rows.iter() {
                let mut row = Map::new();
                row.insert("player_count".into(), json!(players));
                row.insert("variant".into(), json!(variant));
                row.insert("identity_code".into(), json!(identity));
                row.insert("count".into(), json!(count));
                self.upsert_gameprops("sgs_identity_setup", &row)?;
            }
        }
        // 武将包 + 牌堆包
        let packs = [
            ("general", "standard", "标准", 10),
            ("general", "wind_fire_forest_mountain", "风林火山", 20),
            ("general", "yijiang", "一将成名", 30),
            ("general", "boundary_break", "界限突破", 40),
            ("general", "god", "神将", 50),
            ("general", "mou", "谋", 60),
            ("general", "clan", "门阀士族", 70),
            ("general", "demon", "魔", 80),
            ("general", "guozhan", "国战", 90),
            ("general", "other", "其他", 999),
            ("card", "standard", "标准牌堆", 10),
            ("card", "junzheng", "军争牌堆", 20),
        ];
        for (kind, code, name, sort) in packs {
            let mut row = Map::new();
            row.insert("game_id".into(), json!(GAME_ID));
            row.insert("kind".into(), json!(kind));
            row.insert("code".into(), json!(code));
            row.insert("name".into(), json!(name));
            row.insert("sort_order".into(), json!(sort));
            row.insert("enabled".into(), json!(1));
            self.upsert_gameprops("prop_packs", &row)?;
        }
        Ok(
            json!({ "identities": identities.len(), "setups": setups.iter().map(|(_, _, r)| r.len()).sum::<usize>(), "packs": packs.len() }),
        )
    }

    /// 导入官网武将（heroes.json）。幂等：generals.external_id / skills.name。
    pub fn import_sgs_heroes(&self, payload: &Value) -> anyhow::Result<Value> {
        let heroes = payload
            .get("heroes")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow::anyhow!("缺少 heroes 数组"))?;
        let mut packs: HashMap<&'static str, i64> = HashMap::new();
        let mut skill_codes: HashMap<String, String> = HashMap::new();
        let mut general_count = 0;
        let mut skill_count = 0;
        for h in heroes {
            let gid = h.get("gid").and_then(Value::as_i64).unwrap_or(0);
            let name = h.get("name").and_then(Value::as_str).unwrap_or("");
            if gid == 0 || name.is_empty() {
                continue;
            }
            let pack_cn = h.get("pack").and_then(Value::as_str).unwrap_or("其他");
            let pack_code = pack_code(pack_cn);
            let pack_id = if let Some(id) = packs.get(pack_code) {
                *id
            } else {
                let row = self.upsert_gameprops(
                    "prop_packs",
                    &json!({
                        "game_id": GAME_ID, "kind": "general", "code": pack_code,
                        "name": pack_cn, "sort_order": pack_sort(pack_code), "enabled": 1
                    })
                    .as_object()
                    .unwrap()
                    .clone(),
                )?;
                let id = row.get("id").and_then(Value::as_i64).unwrap_or(0);
                packs.insert(pack_code, id);
                id
            };
            let general_code = format!("sgs_hero_{gid}");
            let kingdom = match h.get("kingdom_code").and_then(Value::as_str).unwrap_or("") {
                "wei" => "wei",
                "shu" => "shu",
                "wu" => "wu",
                "qun" => "qun",
                "jin" => "jin",
                "shen" => "god",
                _ => "qun",
            };
            let hp = h.get("hp").and_then(Value::as_f64).unwrap_or(4.0);
            let mut general = Map::new();
            general.insert("code".into(), json!(general_code));
            general.insert(
                "external_id".into(),
                json!(format!("sanguosha.com:hero:{gid}")),
            );
            general.insert("name".into(), json!(name));
            general.insert("kingdom".into(), json!(kingdom));
            general.insert("gender".into(), json!("unknown"));
            general.insert("pack_id".into(), json!(pack_id));
            general.insert("max_hp".into(), json!(hp));
            general.insert("initial_hp".into(), json!(0));
            general.insert(
                "quality".into(),
                h.get("quality").cloned().unwrap_or(Value::Null),
            );
            general.insert(
                "description".into(),
                h.get("characteristic").cloned().unwrap_or(json!("")),
            );
            general.insert(
                "biography".into(),
                h.get("biography").cloned().unwrap_or(json!("")),
            );
            general.insert("status".into(), json!("draft"));
            self.upsert_gameprops("sgs_generals", &general)?;
            general_count += 1;

            if let Some(skills) = h.get("skills").and_then(Value::as_array) {
                for (idx, s) in skills.iter().enumerate() {
                    let sname = s.get("name").and_then(Value::as_str).unwrap_or("");
                    let desc = s.get("description").and_then(Value::as_str).unwrap_or("");
                    if sname.is_empty() || desc.is_empty() {
                        continue;
                    }
                    let scode = if let Some(c) = skill_codes.get(sname) {
                        c.clone()
                    } else {
                        let c = format!("sk_{:x}", fnv1a(sname));
                        let mut skill = Map::new();
                        skill.insert("code".into(), json!(c));
                        skill.insert(
                            "external_id".into(),
                            json!(format!("sanguosha.com:hero:{gid}#skill:{idx}")),
                        );
                        skill.insert("name".into(), json!(sname));
                        skill.insert(
                            "type".into(),
                            json!(normalize_skill_type(
                                s.get("type_guess").and_then(Value::as_str)
                            )),
                        );
                        skill.insert("description".into(), json!(desc));
                        skill.insert("status".into(), json!("draft"));
                        self.upsert_gameprops("sgs_skills", &skill)?;
                        skill_codes.insert(sname.to_string(), c.clone());
                        skill_count += 1;
                        c
                    };
                    let mut link = Map::new();
                    link.insert("general_code".into(), json!(general_code));
                    link.insert("skill_code".into(), json!(scode));
                    link.insert("sort_order".into(), json!(idx as i64));
                    self.upsert_gameprops("sgs_general_skills", &link)?;
                }
            }
        }
        Ok(json!({ "generals": general_count, "skills": skill_count }))
    }

    /// 汇总：各表计数 + 各状态计数。
    pub fn gameprops_summary(&self) -> anyhow::Result<Value> {
        let conn = self.lock();
        let mut out = Map::new();
        for table in [
            "prop_packs",
            "sgs_identities",
            "sgs_card_defs",
            "sgs_cards",
            "sgs_generals",
            "sgs_skills",
        ] {
            let count: i64 =
                conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))?;
            out.insert((*table).to_string(), json!(count));
            if has_status(table) {
                let mut stmt = conn.prepare(&format!(
                    "SELECT status, COUNT(*) FROM {table} GROUP BY status"
                ))?;
                let mut rows = stmt.query([])?;
                let mut by = Map::new();
                while let Some(r) = rows.next()? {
                    by.insert(r.get::<_, String>(0)?, json!(r.get::<_, i64>(1)?));
                }
                out.insert(format!("{table}_status"), Value::Object(by));
            }
        }
        Ok(Value::Object(out))
    }
}

fn pack_code(cn: &str) -> &'static str {
    match cn {
        "标准" => "standard",
        "风林火山" => "wind_fire_forest_mountain",
        "一将成名" => "yijiang",
        "界限突破" => "boundary_break",
        "神将" => "god",
        "谋" => "mou",
        "门阀士族" => "clan",
        "魔" => "demon",
        "国战" => "guozhan",
        _ => "other",
    }
}

fn pack_sort(code: &str) -> i64 {
    match code {
        "standard" => 10,
        "wind_fire_forest_mountain" => 20,
        "yijiang" => 30,
        "boundary_break" => 40,
        "god" => 50,
        "mou" => 60,
        "clan" => 70,
        "demon" => 80,
        "guozhan" => 90,
        _ => 999,
    }
}

fn normalize_skill_type(t: Option<&str>) -> &'static str {
    match t {
        Some("lord") => "lord",
        Some("locked") => "locked",
        Some("limited") => "limited",
        Some("awakening") => "awakening",
        _ => "normal",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_heroes_is_idempotent() {
        let s = Storage::open(":memory:").unwrap();
        let payload = json!({
            "heroes": [{
                "gid": 1, "name": "刘备", "pack": "标准", "kingdom_code": "shu",
                "hp": 4, "quality": "普通", "characteristic": "仁德", "biography": "先主",
                "skills": [
                    { "name": "仁德", "description": "分发手牌", "type_guess": "normal" },
                    { "name": "激将", "description": "主公技", "type_guess": "lord" }
                ]
            }]
        });
        let a = s.import_sgs_heroes(&payload).unwrap();
        assert_eq!(a["generals"], 1);
        assert_eq!(a["skills"], 2);
        let b = s.import_sgs_heroes(&payload).unwrap();
        assert_eq!(b["generals"], 1);
        let conn = s.lock();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM sgs_generals", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM sgs_skills", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 2);
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM sgs_general_skills", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 2);
    }

    #[test]
    fn seed_defaults_is_idempotent() {
        let s = Storage::open(":memory:").unwrap();
        let a = s.seed_sgs_defaults().unwrap();
        assert_eq!(a["identities"], 4);
        assert_eq!(a["packs"], 12);
        let b = s.seed_sgs_defaults().unwrap();
        assert_eq!(b["identities"], 4);
        let conn = s.lock();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM sgs_identity_setup", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 20);
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM prop_packs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 12);
    }

    #[test]
    fn status_transition_writes_log() {
        let s = Storage::open(":memory:").unwrap();
        let row = s
            .upsert_gameprops(
                "sgs_identities",
                &json!({
                    "code": "lord", "name": "主公", "team": "lord",
                    "win_condition": "杀光反贼与内奸", "status": "draft"
                })
                .as_object()
                .unwrap()
                .clone(),
            )
            .unwrap();
        assert_eq!(row["status"], "draft");
        let out = s
            .set_gameprops_status("sgs_identities", "lord", "review", "tester", Some("ok"))
            .unwrap();
        assert_eq!(out["status"], "review");
        let conn = s.lock();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM prop_status_log", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }
}
