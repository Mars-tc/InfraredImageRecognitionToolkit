//! Word 巡检缺陷报告导出（F16）：docx-rs 生成，国网风格模板。

use std::path::Path;

use docx_rs::{AlignmentType, Docx, Paragraph, Pic, Run, Table, TableCell, TableRow, WidthType};

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
/// * `with_images` 是否在报告末尾附缺陷图片（原图不可读时降级为文字提示）
pub fn export_report(
    records: &[LedgerRecord],
    path: &Path,
    inspector: &str,
    date: &str,
    org: &str,
    with_images: bool,
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

    // 缺陷图片小节：图片缺失/不可读时不阻断报告，只给文字提示
    if with_images {
        doc = doc.add_paragraph(
            Paragraph::new()
                .add_run(Run::new().add_text("缺陷图片").bold().size(28))
                .align(AlignmentType::Center),
        );
        for (i, rec) in records.iter().enumerate() {
            doc = doc.add_paragraph(
                Paragraph::new().add_run(
                    Run::new()
                        .add_text(format!(
                            "{}. {} / {} / {} / 最高温 {:.1}℃",
                            i + 1,
                            rec.tower_no,
                            rec.equipment,
                            if rec.phase.is_empty() { "-" } else { &rec.phase },
                            rec.max_temp
                        ))
                        .size(21),
                ),
            );
            if rec.image_path.trim().is_empty() {
                doc = doc.add_paragraph(
                    Paragraph::new().add_run(Run::new().add_text("（未关联图片）").size(18)),
                );
                continue;
            }
            match crate::thumbnail::encode_thumbnail_png(Path::new(&rec.image_path), 360) {
                Ok((png, w, h)) => {
                    let pic = Pic::new_with_dimensions(png, w, h);
                    doc = doc.add_paragraph(Paragraph::new().add_run(Run::new().add_image(pic)));
                }
                Err(_) => {
                    doc = doc.add_paragraph(
                        Paragraph::new().add_run(Run::new().add_text("（图片缺失或不可读）").size(18)),
                    );
                }
            }
        }
    }

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
        let path = std::env::temp_dir().join(format!(
            "ir_test_report_{}_{}.docx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        export_report(&[rec], &path, "张三", "2026-10-25", "配电运维班", false).unwrap();
        let meta = std::fs::metadata(&path).unwrap();
        assert!(meta.len() > 1000);
        assert_eq!(&std::fs::read(&path).unwrap()[..2], b"PK");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn empty_records_rejected() {
        let path = std::env::temp_dir().join(format!("ir_test_empty_{}.docx", std::process::id()));
        assert!(export_report(&[], &path, "x", "x", "x", false).is_err());
    }

    #[test]
    fn report_with_images_embeds_media_and_tolerates_missing() {
        let dir = std::env::temp_dir().join(format!(
            "ir_report_img_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let jpg = dir.join("defect.jpg");
        image::RgbImage::from_fn(200, 120, |x, y| image::Rgb([x as u8, y as u8, 90]))
            .save(&jpg)
            .unwrap();

        let mut rec = LedgerRecord {
            id: 1,
            tower_no: "N-12".into(),
            equipment: "隔离开关".into(),
            phase: "A".into(),
            image_path: jpg.to_string_lossy().to_string(),
            max_temp: 92.5,
            delta_t: 18.2,
            level: 2,
            note: "发热".into(),
            created_at: "2026-11-23".into(),
        };
        let mut missing = rec.clone();
        missing.id = 2;
        missing.tower_no = "N-13".into();
        missing.image_path = dir.join("missing.jpg").to_string_lossy().to_string();

        let path = dir.join("report.docx");
        export_report(&[rec.clone(), missing], &path, "张三", "2026-11-23", "班组", true).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(
            bytes.windows(b"word/media".len()).any(|w| w == b"word/media"),
            "报告应内嵌 word/media 图片"
        );

        // 未关联图片同样不应失败
        rec.image_path.clear();
        let empty = dir.join("report_empty.docx");
        export_report(&[rec], &empty, "张三", "2026-11-23", "班组", true).unwrap();
        assert_eq!(&std::fs::read(&empty).unwrap()[..2], b"PK");
        std::fs::remove_dir_all(&dir).ok();
    }
}
