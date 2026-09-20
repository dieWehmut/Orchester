//! The Orchester desktop shell.
//!
//! Closing the window must not end a run: a turn in flight is work the user
//! asked for, and the window is only the view onto it. So the close request
//! hides the window instead of destroying it, and the tray owns the only exit.

#![forbid(unsafe_code)]

mod tray_lifecycle;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, AppHandle, Manager, WindowEvent,
};
use tray_lifecycle::ExitGate;

const MAIN_WINDOW: &str = "main";

/// Called after the main webview exists; also reusable by the embedded-runtime shell.
pub fn setup_native_chrome(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        orchester_native_chrome::install(window.hwnd()?.0)?;
    }
    #[cfg(not(windows))]
    let _ = app;
    Ok(())
}

/// Shows and focuses the main window, restoring it if it was minimised.
///
/// Every entry that has to make the window visible goes through here, so a
/// restart from the tray behaves the same as a left click on the icon.
fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Builds the tray icon, its menu and both of its click behaviours.
///
/// The icon is required rather than optional: a tray without an icon is a tray
/// the user cannot see, and that would silently remove the only way to quit.
/// A tray failure aborts setup, rather than leaving a window that can hide forever.
pub fn setup_tray(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    app.manage(ExitGate::default());
    let show = MenuItem::with_id(app, "show", "Open Orchester", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Orchester", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &settings, &quit])?;

    let icon = app.default_window_icon().cloned().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "the bundled tray icon is missing",
        )
    })?;

    TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main_window(app),
            "settings" => {
                show_main_window(app);
                if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
                    if let Err(error) = window.eval("window.location.assign('/settings')") {
                        eprintln!("orchester-desktop: could not open settings: {error}");
                    }
                }
            }
            "quit" => {
                app.state::<ExitGate>().request_tray_exit();
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

pub fn handle_window_event(window: &tauri::Window, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        api.prevent_close();
        if let Err(error) = window.hide() {
            eprintln!("orchester-desktop: could not hide the window: {error}");
        }
    }
}

/// The embedded runtime can call this before handling its own RunEvent::Exit cleanup.
pub fn handle_run_event(app: &AppHandle, event: &tauri::RunEvent) {
    if let tauri::RunEvent::ExitRequested { api, .. } = event {
        app.state::<ExitGate>()
            .on_exit_requested(|| api.prevent_exit());
    }
}

pub fn run() -> tauri::Result<()> {
    tauri::Builder::default()
        .setup(|app| {
            setup_native_chrome(app)?;
            setup_tray(app)?;
            Ok(())
        })
        .on_window_event(handle_window_event)
        .build(tauri::generate_context!())?
        .run(|app, event| handle_run_event(app, &event));
    Ok(())
}
