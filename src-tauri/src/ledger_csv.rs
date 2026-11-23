//! 巡检台账 CSV 导入 / 导出（F15 扩展）。
//!
//! 不引入 csv crate：写入按需加引号转义，读取支持引号内的逗号、换行与 `""` 转义，
//! 保证 Excel 中编辑过的备注还能原样导回。

use std::io::{BufWriter, Write};
use std::path::Path;

use crate::db::{LedgerInput, LedgerRecord};
use crate::error::AppResult;
use crate::utils::level_name;

const HEADER: [&str; 9] = [
    "杆塔号",
    "设备/部位",
    "相别",
    "图片路径",
    "最高温(℃)",
    "温差ΔT(℃)",
    "缺陷等级",
    "备注",
    "录入时间",
];

/// 导出全部台账记录为 CSV（UTF-8 BOM，Excel 可直接打开）
pub fn export_ledger_csv(records: &[LedgerRecord], path: &Path) -> AppResult<usize> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败：{e}"))?;
    }
    let file = std::fs::File::create(path).map_err(|e| format!("创建文件失败：{e}"))?;
    let mut w = BufWriter::new(file);
    w.write_all(b"\xEF\xBB\xBF").map_err(io_err)?;
    writeln!(w, "{}", HEADER.join(",")).map_err(io_err)?;
    for r in records {
        let row = [
            escape(&r.tower_no),
            escape(&r.equipment),
            escape(&r.phase),
            escape(&r.image_path),
            format!("{:.2}", r.max_temp),
            format!("{:.2}", r.delta_t),
            escape(level_name(r.level)),
            escape(&r.note),
            escape(&r.created_at),
        ];
        writeln!(w, "{}", row.join(",")).map_err(io_err)?;
    }
    w.flush().map_err(io_err)?;
    Ok(records.len())
}

/// 读取 CSV 并转换为台账输入；表头、BOM、空行自动跳过。
pub fn import_ledger_csv(path: &Path) -> AppResult<Vec<LedgerInput>> {
    let bytes = std::fs::read(path).map_err(|e| format!("读取 CSV 失败：{e}"))?;
    let text = String::from_utf8_lossy(&bytes);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let mut rows = parse_records(text);
    if rows.first().map(|r| r.first().map(|s| s.trim()) == Some("杆塔号")).unwrap_or(false) {
        rows.remove(0);
    }
    Ok(rows.iter().filter_map(|row| row_to_input(row)).collect())
}

fn row_to_input(row: &[String]) -> Option<LedgerInput> {
    if row.iter().all(|f| f.trim().is_empty()) {
        return None;
    }
    let get = |i: usize| row.get(i).map(|s| s.trim().to_string()).unwrap_or_default();
    Some(LedgerInput {
        tower_no: get(0),
        equipment: get(1),
        phase: get(2),
        image_path: get(3),
        max_temp: get(4).parse::<f64>().unwrap_or(0.0),
        delta_t: get(5).parse::<f64>().unwrap_or(0.0),
        level: parse_level(&get(6)),
        note: get(7),
    })
}

/// 等级同时接受中文名与 0~3 数字
fn parse_level(s: &str) -> u8 {
    match s {
        "紧急" => 3,
        "重大" => 2,
        "一般" => 1,
        "其他" => 0,
        _ => s.parse::<i64>().map(|v| v.clamp(0, 3) as u8).unwrap_or(0),
    }
}

fn escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// 极简 CSV 解析：支持双引号包裹、`""` 转义与引号内换行
fn parse_records(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if in_quotes {
            match c {
                '"' => {
                    if chars.peek() == Some(&'"') {
                        chars.next();
                        field.push('"');
                    } else {
                        in_quotes = false;
                    }
                }
                _ => field.push(c),
            }
            continue;
        }
        match c {
            '"' => in_quotes = true,
            ',' => row.push(std::mem::take(&mut field)),
            '\r' => {}
            '\n' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            _ => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

fn io_err(e: std::io::Error) -> String {
    format!("写入 CSV 失败：{e}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ir_ledger_csv_{}_{}_{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn record(note: &str) -> LedgerRecord {
        LedgerRecord {
            id: 1,
            tower_no: "N-12".into(),
            equipment: "隔离开关, A相".into(),
            phase: "A".into(),
            image_path: "D:/巡检/1.JPG".into(),
            max_temp: 92.5,
            delta_t: 18.2,
            level: 2,
            note: note.into(),
            created_at: "2026-11-23 10:00:00".into(),
        }
    }

    #[test]
    fn export_import_round_trip_keeps_quoted_fields() {
        let dir = scratch("roundtrip");
        let path = dir.join("ledger.csv");
        let note = "发热明显，建议\"停电\"处理\n并复测";
        assert_eq!(export_ledger_csv(&[record(note)], &path).unwrap(), 1);

        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[..3], b"\xEF\xBB\xBF");

        let inputs = import_ledger_csv(&path).unwrap();
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].tower_no, "N-12");
        assert_eq!(inputs[0].equipment, "隔离开关, A相");
        assert_eq!(inputs[0].max_temp, 92.5);
        assert_eq!(inputs[0].level, 2);
        assert_eq!(inputs[0].note, note);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn import_accepts_numeric_level_and_skips_empty_rows() {
        let dir = scratch("numeric");
        let path = dir.join("in.csv");
        let text = "杆塔号,设备/部位,相别,图片路径,最高温(℃),温差ΔT(℃),缺陷等级,备注,录入时间\nN-1,接头,A,/a.jpg,80,12,3,尽快处理,2026-11-23\n,,,,,,,,\n";
        std::fs::write(&path, text).unwrap();
        let inputs = import_ledger_csv(&path).unwrap();
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].level, 3);
        assert_eq!(inputs[0].note, "尽快处理");
        std::fs::remove_dir_all(&dir).ok();
    }
}
