use std::{io, path::PathBuf};

use orchester_anwendung::OrchesterPaths;
use tauri::{ipc::CapabilityBuilder, Url};

pub(crate) fn desktop_paths() -> io::Result<OrchesterPaths> {
    let home = orchester_laufzeit::harness::orchester_home()
        .filter(|path| path.is_absolute())
        .ok_or_else(|| io::Error::other("Orchester requires an absolute user home directory"))?;
    // A shortcut's working directory may be the installation directory or
    // System32. Keep the initial workspace in writable per-user storage.
    let workspace = home.join("workspaces").join("default");
    std::fs::create_dir_all(&workspace)?;
    Ok(OrchesterPaths::new(home, workspace))
}

pub(crate) fn web_assets(resource_directory: PathBuf) -> io::Result<PathBuf> {
    let packaged = resource_directory.join("web");
    if packaged.join("index.html").is_file() {
        return Ok(packaged);
    }
    #[cfg(debug_assertions)]
    {
        let development = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web/dist");
        if development.join("index.html").is_file() {
            return Ok(development);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "Bundled web/index.html is missing",
    ))
}

pub(crate) fn navigation_allowed(origin: &Url, target: &Url) -> bool {
    target.origin() == origin.origin()
        && target.username().is_empty()
        && target.password().is_none()
}

pub(crate) fn window_capability(origin: &Url) -> CapabilityBuilder {
    CapabilityBuilder::new("embedded-runtime-window")
        .local(false)
        .window("main")
        .remote(format!("{}/*", origin.origin().ascii_serialization()))
        .permission("core:window:allow-close")
        .permission("core:window:allow-minimize")
        .permission("core:window:allow-toggle-maximize")
        .permission("core:window:allow-start-dragging")
        .permission("core:window:allow-is-maximized")
        .permission("core:event:allow-listen")
        .permission("core:event:allow-unlisten")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::ipc::RuntimeCapability;
    use tauri::utils::acl::capability::CapabilityFile;

    #[test]
    fn navigation_is_bound_to_the_actual_runtime_origin() {
        let origin = Url::parse("http://127.0.0.1:49152").unwrap();
        assert!(navigation_allowed(
            &origin,
            &origin.join("/settings").unwrap()
        ));
        for target in [
            "https://example.com",
            "http://127.0.0.1:49153",
            "http://localhost:49152",
            "https://127.0.0.1:49152",
            "http://user@127.0.0.1:49152",
            "file:///index.html",
            "data:text/html,test",
            "javascript:alert(1)",
        ] {
            assert!(
                !navigation_allowed(&origin, &Url::parse(target).unwrap()),
                "{target}"
            );
        }
    }

    #[test]
    fn capability_grants_only_window_controls_to_the_exact_local_runtime() {
        let origin = Url::parse("http://127.0.0.1:49152").unwrap();
        let CapabilityFile::Capability(capability) = window_capability(&origin).build() else {
            panic!("the desktop capability must be a single capability");
        };
        assert_eq!(capability.identifier, "embedded-runtime-window");
        assert!(!capability.local);
        assert_eq!(capability.windows, ["main"]);
        assert_eq!(
            capability.remote.as_ref().map(|remote| remote.urls.as_slice()),
            Some(["http://127.0.0.1:49152/*".to_owned()].as_slice())
        );
        let rendered = serde_json::to_value(&capability).unwrap();
        assert_eq!(rendered["permissions"].as_array().unwrap().len(), 7);
        for permission in rendered["permissions"].as_array().unwrap() {
            let permission = permission.as_str().unwrap();
            assert!(
                permission.starts_with("core:window:")
                    || permission == "core:event:allow-listen"
                    || permission == "core:event:allow-unlisten"
            );
        }
    }
}
