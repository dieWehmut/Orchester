#![forbid(unsafe_code)]

mod runtime;

use std::sync::Mutex;

use orchester_netz::EmbeddedServer;
use tauri::{Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};

pub fn run() -> tauri::Result<()> {
    let app = tauri::Builder::default()
        .setup(|app| {
            let paths = runtime::desktop_paths()?;
            let assets = runtime::web_assets(app.path().resource_dir()?)?;
            let server = tauri::async_runtime::block_on(EmbeddedServer::start(paths, assets))?;
            let origin: tauri::Url = server.origin().parse()?;
            let launch_url = server.launch_url().parse()?;
            app.add_capability(runtime::window_capability(&origin))?;
            app.manage(Mutex::new(Some(server)));

            let mut config = app.config().app.windows[0].clone();
            config.url = WebviewUrl::External(launch_url);
            WebviewWindowBuilder::from_config(app, &config)?
                .on_navigation(move |url| runtime::navigation_allowed(&origin, url))
                .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
                .build()?;
            Ok(())
        })
        .build(tauri::generate_context!())?;

    app.run(|app, event| {
        if let RunEvent::Exit = event {
            if let Some(state) = app.try_state::<Mutex<Option<EmbeddedServer>>>() {
                if let Ok(mut guard) = state.lock() {
                    if let Some(mut server) = guard.take() {
                        // Release the listener and notify monitors, runs and sockets before
                        // the GUI event loop destroys the process-owned async runtime.
                        let _ = tauri::async_runtime::block_on(server.shutdown());
                    }
                }
            }
        }
    });
    Ok(())
}
