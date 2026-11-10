use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Manager, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

static NEXT_WINDOW: AtomicU64 = AtomicU64::new(1);

async fn show_conversation_window(window: WebviewWindow) -> Result<(), String> {
    let (sender, mut receiver) = tauri::async_runtime::channel(1);
    let ui_window = window.clone();
    // Window creation stays in the async command to avoid WebView2's synchronous
    // command deadlock. Windows subclass installation requires the owning UI thread.
    if window
        .run_on_main_thread(move || {
            let result = (|| {
                #[cfg(windows)]
                orchester_native_chrome::install(
                    ui_window.hwnd().map_err(|_| "Window handle unavailable")?.0,
                )
                .map_err(|_| "Window chrome unavailable")?;
                ui_window
                    .show()
                    .map_err(|_| "Could not show conversation window")?;
                ui_window
                    .set_focus()
                    .map_err(|_| "Could not focus conversation window")?;
                Ok(())
            })();
            if result.is_err() {
                let _ = ui_window.destroy();
            }
            let _ = sender.try_send(result);
        })
        .is_err()
    {
        let _ = window.destroy();
        return Err("Window UI thread unavailable".into());
    }
    receiver
        .recv()
        .await
        .ok_or_else(|| "Window UI thread unavailable".to_owned())?
}

fn conversation_url(origin: &Url, id: &str) -> Result<Url, String> {
    if id.len() != 34
        || !id.starts_with("s-")
        || !id.as_bytes()[2..].iter().all(u8::is_ascii_hexdigit)
    {
        return Err("Invalid conversation identifier".into());
    }
    if origin.scheme() != "http"
        || origin.host_str() != Some("127.0.0.1")
        || !origin.username().is_empty()
        || origin.password().is_some()
    {
        return Err("Invalid desktop origin".into());
    }
    let mut url = origin.clone();
    url.set_path("/workspace");
    url.set_query(None);
    url.set_fragment(None);
    url.query_pairs_mut().append_pair("session", id);
    Ok(url)
}

#[tauri::command]
pub async fn open_conversation_window(
    app: AppHandle,
    window: WebviewWindow,
    id: String,
) -> Result<(), String> {
    let main = app
        .get_webview_window("main")
        .ok_or("Main window unavailable")?;
    let origin = main.url().map_err(|_| "Desktop origin unavailable")?;
    let caller = window.url().map_err(|_| "Caller unavailable")?;
    if !(window.label() == "main" || window.label().starts_with("conversation-"))
        || !crate::runtime::navigation_allowed(&origin, &caller)
    {
        return Err("Unauthorized window".into());
    }
    let url = conversation_url(&origin, &id)?;
    let label = format!(
        "conversation-{}",
        NEXT_WINDOW.fetch_add(1, Ordering::Relaxed)
    );
    let mut config = app.config().app.windows[0].clone();
    config.label = label;
    // Install custom chrome before exposing the new window.
    config.visible = false;
    config.url = WebviewUrl::External(url);
    let new_window = WebviewWindowBuilder::from_config(&app, &config)
        .map_err(|_| "Window configuration unavailable")?
        .on_navigation(move |url| crate::runtime::navigation_allowed(&origin, url))
        .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
        .build()
        .map_err(|_| "Could not open conversation window")?;
    show_conversation_window(new_window).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scopes_and_cleans_conversation_url() {
        let id = "s-0123456789abcdef0123456789abcdef";
        let origin =
            Url::parse("http://127.0.0.1:12345/settings?secret=x#bootstrap=secret").unwrap();
        assert_eq!(
            conversation_url(&origin, id).unwrap().as_str(),
            format!("http://127.0.0.1:12345/workspace?session={id}")
        );
        for bad in [
            "s-../settings",
            "https://example.com",
            "s-0123456789abcdef0123456789abcdeg",
        ] {
            assert!(conversation_url(&origin, bad).is_err());
        }
        for external in [
            "https://example.com",
            "http://localhost:12345",
            "http://user@127.0.0.1:12345",
        ] {
            assert!(conversation_url(&Url::parse(external).unwrap(), id).is_err());
        }
    }
}
