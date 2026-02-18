mod commands;
mod models;
mod tray;
mod watcher;

use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_updater::UpdaterExt;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_stats_cache,
            commands::get_today_summary,
            commands::get_history,
            commands::get_daily_token_totals,
            commands::resize_popup,
            commands::get_usage_limits,
            commands::get_profile,
            commands::get_app_version,
            commands::check_for_updates,
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            #[cfg(target_os = "macos")]
            {
                let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }

            tray::setup_tray(&handle)?;
            watcher::start_watcher(handle.clone());

            let update_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                check_for_updates(update_handle).await;
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn check_file_path() -> Option<std::path::PathBuf> {
    dirs::data_dir().map(|d| d.join("com.brewstatus.app").join("last_update_check"))
}

fn should_check() -> bool {
    let Some(path) = check_file_path() else {
        return true;
    };
    match fs::read_to_string(&path) {
        Ok(contents) => {
            let last: u64 = contents.trim().parse().unwrap_or(0);
            now_secs().saturating_sub(last) >= 86400
        }
        Err(_) => true,
    }
}

fn write_check_timestamp() {
    if let Some(path) = check_file_path() {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&path, now_secs().to_string());
    }
}

pub async fn prompt_install(update: tauri_plugin_updater::Update, handle: tauri::AppHandle) {
    let version = update.version.clone();

    let (sender, receiver) = std::sync::mpsc::channel();
    handle
        .dialog()
        .message(format!(
            "Brew Status v{version} is available.\n\nWould you like to install the update now?"
        ))
        .title("Update Available")
        .kind(MessageDialogKind::Info)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Install".into(),
            "Later".into(),
        ))
        .show(move |confirmed| {
            let _ = sender.send(confirmed);
        });

    let confirmed = receiver.recv().unwrap_or(false);

    if confirmed {
        if let Err(e) = update.download_and_install(|_, _| {}, || {}).await {
            eprintln!("update install error: {e}");
        }
    }
}

async fn check_for_updates(handle: tauri::AppHandle) {
    if !should_check() {
        return;
    }

    let updater = match handle.updater() {
        Ok(u) => u,
        Err(e) => {
            eprintln!("updater init error: {e}");
            return;
        }
    };

    let update = match updater.check().await {
        Ok(Some(update)) => update,
        Ok(None) => {
            write_check_timestamp();
            return;
        }
        Err(e) => {
            eprintln!("update check error: {e}");
            return;
        }
    };

    let version = update.version.clone();

    // Send a Windows notification
    let _ = handle
        .notification()
        .builder()
        .title("Brew Status Update")
        .body(format!("Brew Status v{version} is available"))
        .show();

    write_check_timestamp();

    prompt_install(update, handle).await;
}
