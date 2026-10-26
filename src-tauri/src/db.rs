//! SQLite 巡检台账（F15）：rusqlite bundled，无系统依赖。

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::AppResult;

/// 台账记录
#[derive(Debug, Clone, serde::Serialize)]
pub struct LedgerRecord {
    pub id: i64,
    /// 杆塔号
    pub tower_no: String,
    /// 设备编号/名称
    pub equipment: String,
    /// 相别（A/B/C/N，可空）
    pub phase: String,
    /// 关联的辐射图路径
    pub image_path: String,
    pub max_temp: f64,
    pub delta_t: f64,
    /// 缺陷等级 0~3
    pub level: u8,
    /// 处理意见/备注
    pub note: String,
    /// 创建时间（本地时间字符串）
    pub created_at: String,
}

/// 新建/更新时的输入
#[derive(Debug, Clone, serde::Deserialize)]
pub struct LedgerInput {
    pub tower_no: String,
    pub equipment: String,
    #[serde(default)]
    pub phase: String,
    #[serde(default)]
    pub image_path: String,
    #[serde(default)]
    pub max_temp: f64,
    #[serde(default)]
    pub delta_t: f64,
    #[serde(default)]
    pub level: u8,
    #[serde(default)]
    pub note: String,
}

/// 打开（或创建）数据库并建表
pub fn open(path: &std::path::Path) -> AppResult<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建数据目录失败：{e}"))?;
    }
    let conn = Connection::open(path).map_err(|e| format!("打开数据库失败：{e}"))?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         CREATE TABLE IF NOT EXISTS ledger (
             id          INTEGER PRIMARY KEY AUTOINCREMENT,
             tower_no    TEXT NOT NULL DEFAULT '',
             equipment   TEXT NOT NULL DEFAULT '',
             phase       TEXT NOT NULL DEFAULT '',
             image_path  TEXT NOT NULL DEFAULT '',
             max_temp    REAL NOT NULL DEFAULT 0,
             delta_t     REAL NOT NULL DEFAULT 0,
             level       INTEGER NOT NULL DEFAULT 0,
             note        TEXT NOT NULL DEFAULT '',
             created_at  TEXT NOT NULL DEFAULT (datetime('now','localtime'))
         );
         CREATE INDEX IF NOT EXISTS idx_ledger_tower ON ledger(tower_no);
         CREATE INDEX IF NOT EXISTS idx_ledger_level ON ledger(level);",
    )
    .map_err(|e| format!("初始化数据表失败：{e}"))?;
    Ok(conn)
}

/// 新增记录，返回 id
pub fn insert(conn: &Connection, input: &LedgerInput) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO ledger (tower_no, equipment, phase, image_path, max_temp, delta_t, level, note)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            input.tower_no,
            input.equipment,
            input.phase,
            input.image_path,
            input.max_temp,
            input.delta_t,
            input.level,
            input.note
        ],
    )
    .map_err(db_err)?;
    Ok(conn.last_insert_rowid())
}

/// 更新记录
pub fn update(conn: &Connection, id: i64, input: &LedgerInput) -> AppResult<usize> {
    conn.execute(
        "UPDATE ledger SET tower_no=?1, equipment=?2, phase=?3, image_path=?4,
         max_temp=?5, delta_t=?6, level=?7, note=?8 WHERE id=?9",
        params![
            input.tower_no,
            input.equipment,
            input.phase,
            input.image_path,
            input.max_temp,
            input.delta_t,
            input.level,
            input.note,
            id
        ],
    )
    .map_err(db_err)
}

/// 删除记录
pub fn delete(conn: &Connection, id: i64) -> AppResult<usize> {
    conn.execute("DELETE FROM ledger WHERE id=?1", params![id]).map_err(db_err)
}

/// 查询：关键字（杆塔/设备/备注模糊）+ 等级过滤
pub fn query(
    conn: &Connection,
    keyword: &str,
    level: Option<u8>,
    limit: u32,
    offset: u32,
) -> AppResult<Vec<LedgerRecord>> {
    let mut sql = String::from(
        "SELECT id, tower_no, equipment, phase, image_path, max_temp, delta_t, level, note, created_at
         FROM ledger WHERE 1=1",
    );
    let mut args: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    if !keyword.is_empty() {
        sql.push_str(" AND (tower_no LIKE ? OR equipment LIKE ? OR note LIKE ?)");
        let pat = format!("%{keyword}%");
        args.push(Box::new(pat.clone()));
        args.push(Box::new(pat.clone()));
        args.push(Box::new(pat));
    }
    if let Some(lv) = level {
        sql.push_str(" AND level = ?");
        args.push(Box::new(lv as i64));
    }
    sql.push_str(" ORDER BY id DESC LIMIT ? OFFSET ?");
    args.push(Box::new(limit as i64));
    args.push(Box::new(offset as i64));

    let mut stmt = conn.prepare(&sql).map_err(db_err)?;
    let refs: Vec<&dyn rusqlite::types::ToSql> = args.iter().map(|b| b.as_ref()).collect();
    let rows = stmt
        .query_map(refs.as_slice(), |r| {
            Ok(LedgerRecord {
                id: r.get(0)?,
                tower_no: r.get(1)?,
                equipment: r.get(2)?,
                phase: r.get(3)?,
                image_path: r.get(4)?,
                max_temp: r.get(5)?,
                delta_t: r.get(6)?,
                level: r.get::<_, i64>(7)? as u8,
                note: r.get(8)?,
                created_at: r.get(9)?,
            })
        })
        .map_err(db_err)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(db_err)?);
    }
    Ok(out)
}

/// 统计（按等级计数）
pub fn count_by_level(conn: &Connection) -> AppResult<[i64; 4]> {
    let mut out = [0i64; 4];
    let mut stmt = conn
        .prepare("SELECT level, COUNT(*) FROM ledger GROUP BY level")
        .map_err(db_err)?;
    let rows = stmt
        .query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
        })
        .map_err(db_err)?;
    for row in rows {
        let (level, count) = row.map_err(db_err)?;
        if (0..4).contains(&level) {
            out[level as usize] = count;
        }
    }
    Ok(out)
}

/// 按 id 批量取记录（报告导出用）
pub fn get_by_ids(conn: &Connection, ids: &[i64]) -> AppResult<Vec<LedgerRecord>> {
    let mut out = Vec::new();
    for id in ids {
        let rec = conn
            .query_row(
                "SELECT id, tower_no, equipment, phase, image_path, max_temp, delta_t, level, note, created_at
                 FROM ledger WHERE id=?1",
                params![id],
                |r| {
                    Ok(LedgerRecord {
                        id: r.get(0)?,
                        tower_no: r.get(1)?,
                        equipment: r.get(2)?,
                        phase: r.get(3)?,
                        image_path: r.get(4)?,
                        max_temp: r.get(5)?,
                        delta_t: r.get(6)?,
                        level: r.get::<_, i64>(7)? as u8,
                        note: r.get(8)?,
                        created_at: r.get(9)?,
                    })
                },
            )
            .optional()
            .map_err(db_err)?;
        if let Some(rec) = rec {
            out.push(rec);
        }
    }
    Ok(out)
}

fn db_err(e: rusqlite::Error) -> String {
    format!("数据库操作失败：{e}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem_db() -> Connection {
        open(std::path::Path::new(":memory:")).unwrap()
    }

    fn sample() -> LedgerInput {
        LedgerInput {
            tower_no: "N-12".into(),
            equipment: "隔离开关".into(),
            phase: "A".into(),
            image_path: "D:/img/1.JPG".into(),
            max_temp: 92.5,
            delta_t: 18.2,
            level: 2,
            note: "发热明显".into(),
        }
    }

    #[test]
    fn insert_query_update_delete() {
        let conn = mem_db();
        let id = insert(&conn, &sample()).unwrap();
        assert!(id > 0);
        let all = query(&conn, "", None, 10, 0).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].tower_no, "N-12");
        // 关键字
        assert_eq!(query(&conn, "隔离", None, 10, 0).unwrap().len(), 1);
        assert_eq!(query(&conn, "不存在", None, 10, 0).unwrap().len(), 0);
        // 等级
        assert_eq!(query(&conn, "", Some(2), 10, 0).unwrap().len(), 1);
        assert_eq!(query(&conn, "", Some(3), 10, 0).unwrap().len(), 0);
        // 更新
        let mut input = sample();
        input.note = "已处理";
        update(&conn, id, &input).unwrap();
        assert_eq!(query(&conn, "已处理", None, 10, 0).unwrap().len(), 1);
        // 删除
        delete(&conn, id).unwrap();
        assert_eq!(query(&conn, "", None, 10, 0).unwrap().len(), 0);
    }

    #[test]
    fn count_by_level() {
        let conn = mem_db();
        insert(&conn, &sample()).unwrap();
        let mut s2 = sample();
        s2.level = 3;
        insert(&conn, &s2).unwrap();
        let counts = count_by_level(&conn).unwrap();
        assert_eq!(counts[2], 1);
        assert_eq!(counts[3], 1);
    }
}
