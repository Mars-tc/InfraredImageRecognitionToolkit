#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use ir_inspection_tool::commands::*;
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .manage(AppState::default())
        .setup(|app| {
            // 启动时读取缺陷阈值配置 + 数据库路径
            let state = app.state::<AppState>();
            if let Some(path) = ir_inspection_tool::settings::config_path(&app.config()) {
                let t = ir_inspection_tool::settings::load(&path);
                *state.thresholds.lock().unwrap() = t;
                *state.thresholds_path.lock().unwrap() = Some(path);
            }
            if let Some(dir) = tauri::api::path::app_data_dir(&app.config()) {
                *state.db_path.lock().unwrap() = Some(dir.join("ledger.db"));
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_image,
            get_temps,
            get_dimensions,
            set_emissivity,
            set_distance,
            roi_stats,
            get_settings,
            set_settings,
            phase_compare,
            batch_parse,
            export_csv,
            export_batch_csv,
            export_annotated,
            ledger_add,
            ledger_query,
            ledger_stats,
            ledger_update,
            ledger_delete,
            export_report
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
