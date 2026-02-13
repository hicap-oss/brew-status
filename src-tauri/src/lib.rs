mod commands;
mod models;
mod tray;
mod watcher;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_stats_cache,
            commands::get_today_summary,
            commands::get_history,
            commands::get_daily_token_totals,
            commands::resize_popup,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            tray::setup_tray(&handle)?;
            watcher::start_watcher(handle);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
