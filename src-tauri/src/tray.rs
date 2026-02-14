use std::time::Duration;

use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};
use tauri_plugin_positioner::{Position, WindowExt};

pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let open_item = MenuItemBuilder::with_id("open", "Open App").build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
    let menu = MenuBuilder::new(app)
        .item(&open_item)
        .separator()
        .item(&quit_item)
        .build()?;

    let icon = Image::from_bytes(include_bytes!("../icons/32x32.png"))
        .expect("Failed to load tray icon");

    TrayIconBuilder::new()
        .icon(icon)
        .tooltip("Brew Status - Claude Code Usage")
        .show_menu_on_left_click(false)
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => {
                show_main_window(app);
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);

            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Down,
                ..
            } = event
            {
                toggle_popup(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

fn toggle_popup(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("popup") {
        show_popup(&window);
    } else {
        create_popup(app);
    }
}

fn show_popup(window: &WebviewWindow) {
    if window.move_window_constrained(Position::TrayCenter).is_err() {
        let _ = window.move_window(Position::BottomRight);
    }

    let _ = window.show();
    let _ = window.set_focus();

    if window.move_window_constrained(Position::TrayCenter).is_err() {
        let _ = window.move_window(Position::BottomRight);
    }

    // Snap popup flush against the taskbar using the work area bounds
    #[cfg(target_os = "windows")]
    {
        if let (Ok(pos), Ok(size)) = (window.outer_position(), window.outer_size()) {
            if let Some(work_bottom) = get_work_area_bottom(pos.x, pos.y) {
                let y = work_bottom - size.height as i32;
                let _ = window.set_position(tauri::PhysicalPosition::new(pos.x, y));
            }
        }
    }
}

#[cfg(target_os = "windows")]
pub fn get_work_area_bottom(x: i32, y: i32) -> Option<i32> {
    #[repr(C)]
    struct RECT {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    struct MONITORINFO {
        cb_size: u32,
        rc_monitor: RECT,
        rc_work: RECT,
        dw_flags: u32,
    }

    #[repr(C)]
    struct POINT {
        x: i32,
        y: i32,
    }

    type HMONITOR = isize;

    extern "system" {
        fn MonitorFromPoint(pt: POINT, dw_flags: u32) -> HMONITOR;
        fn GetMonitorInfoW(h_monitor: HMONITOR, lpmi: *mut MONITORINFO) -> i32;
    }

    const MONITOR_DEFAULTTONEAREST: u32 = 2;

    let monitor = unsafe { MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST) };
    if monitor == 0 {
        return None;
    }

    let mut info: MONITORINFO = unsafe { std::mem::zeroed() };
    info.cb_size = std::mem::size_of::<MONITORINFO>() as u32;

    if unsafe { GetMonitorInfoW(monitor, &mut info) } != 0 {
        Some(info.rc_work.bottom)
    } else {
        None
    }
}

fn create_popup(app: &AppHandle) {
    let window = WebviewWindowBuilder::new(app, "popup", WebviewUrl::App("src/popup.html".into()))
        .title("Brew Status")
        .inner_size(320.0, 580.0)
        .resizable(false)
        .decorations(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .visible(false)
        .build();

    if let Ok(win) = window {
        let popup = win.clone();
        win.on_window_event(move |event| {
            if let tauri::WindowEvent::Focused(false) = event {
                let popup = popup.clone();
                tauri::async_runtime::spawn(async move {
                    std::thread::sleep(Duration::from_millis(140));
                    if popup.is_visible().unwrap_or(false) && !popup.is_focused().unwrap_or(false) {
                        let _ = popup.hide();
                    }
                });
            }
        });

        show_popup(&win);
    }
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    } else {
        let window =
            WebviewWindowBuilder::new(app, "main", WebviewUrl::App("src/main.html".into()))
                .title("Brew Status - Claude Code Dashboard")
                .inner_size(900.0, 680.0)
                .min_inner_size(700.0, 500.0)
                .decorations(false)
                .visible(true)
                .build();

        if let Ok(win) = window {
            let _ = win.set_focus();
            let win_clone = win.clone();
            win.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = win_clone.destroy();
                }
            });
        }
    }
}
