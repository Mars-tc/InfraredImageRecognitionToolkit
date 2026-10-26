//! Word 巡检缺陷报告导出（F16）：docx-rs 生成，国网风格模板。

use std::path::Path;

use docx_rs::{AlignmentType, Docx, Paragraph, Run, Table, TableCell, TableRow, WidthType};

use crate::db::LedgerRecord;
use crate::error::AppResult;
use crate::utils::level_name;

/// 报告表头列
const HEADERS: [&str; 8] = [
    "序号", "杆塔号", "设备/部位", "相别", "最高温(℃)", "温差ΔT(℃)", "缺陷等级", "处理意见",
];

/// 导出巡检缺陷报告
///
/// * `records`   台账记录
/// * `path`      输出 .docx 路径
/// * `inspector` 检测人
/// * `date`      检测日期（如 2026-10-25）
/// * `org`       检测单位/班组
pub fn export_report(
    records: &[LedgerRecord],
    path: &Path,
    inspector: &str,
    date: &str,
    org: &str,
) -> AppResult<()> {
    if records.is_empty() {
        return Err("请先选择要导出的台账记录".to_string());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败：{e}"))?;
    }

    let critical = records.iter().filter(|r| r.level >= 3).count();
    let major = records.iter().filter(|r| r.level == 2).count();
    let minor = records.iter().filter(|r| r.level == 1).count();

    let mut doc = Docx::new();

    // 标题
    doc = doc.add_paragraph(
        Paragraph::new()
            .add_run(Run::new().add_text("配电网红外检测缺陷报告").bold().size(36))
            .align(AlignmentType::Center),
    );
    doc = doc.add_paragraph(
        Paragraph::new()
            .add_run(
                Run::new()
                    .add_text(format!("检测单位：{org}    检测人：{inspector}    检测日期：{date}"))
                    .size(21),
            )
            .align(AlignmentType::Center),
    );

    // 概况
    doc = doc.add_paragraph(
        Paragraph::new().add_run(
            Run::new()
                .add_text(format!(
                    "本次共检测缺陷 {n} 处，其中紧急 {critical} 处、重大 {major} 处、一般 {minor} 处。",
                    n = records.len()
                ))
                .size(21),
        ),
    );

    // 明细表
    let mut rows = Vec::new();
    rows.push(header_row());
    for (i, rec) in records.iter().enumerate() {
        rows.push(TableRow::new(vec![
            cell(&(i + 1).to_string()),
            cell(&rec.tower_no),
            cell(&rec.equipment),
            cell(&rec.phase),
            cell(&format!("{:.1}", rec.max_temp)),
            cell(&format!("{:.1}", rec.delta_t)),
            cell(level_name(rec.level)),
            cell(&rec.note),
        ]));
    }
    doc = doc.add_table(Table::new(rows).width(9360, WidthType::Dxa));

    // 落款
    doc = doc.add_paragraph(
        Paragraph::new()
            .add_run(Run::new().add_text(format!("\n检测人签字：________________    日期：{date}")).size(21))
            .align(AlignmentType::Right),
    );

    doc.build()
        .pack(std::fs::File::create(path).map_err(|e| format!("创建文件失败：{e}"))?)
        .map_err(|e| format!("写入报告失败：{e}"))?;
    Ok(())
}

fn header_row() -> TableRow {
    TableRow::new(HEADERS.iter().map(|h| cell(h)).collect())
}

fn cell(text: &str) -> TableCell {
    TableCell::new().add_paragraph(
        Paragraph::new().add_run(Run::new().add_text(text.to_string()).size(18)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_generates_file() {
        let rec = LedgerRecord {
            id: 1,
            tower_no: "N-12".into(),
            equipment: "隔离开关A相".into(),
            phase: "A".into(),
            image_path: "".into(),
            max_temp: 92.5,
            delta_t: 18.2,
            level: 2,
            note: "建议停电处理".into(),
            created_at: "2026-10-25".into(),
        };
        let path = std::env::temp_dir().join("ir_test_report.docx");
        export_report(&[rec], &path, "张三", "2026-10-25", "配电运维班").unwrap();
        let meta = std::fs::metadata(&path).unwrap();
        assert!(meta.len() > 1000);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn empty_records_rejected() {
        let path = std::env::temp_dir().join("ir_test_empty.docx");
        assert!(export_report(&[], &path, "x", "x", "x").is_err());
    }
}
