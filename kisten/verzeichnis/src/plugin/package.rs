use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

#[cfg(unix)]
use cap_fs_ext::OpenOptionsSyncExt;
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};

use super::PluginError;

const MAX_JSON_BYTES: u64 = 64 * 1024;
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_ROOT_ENTRIES: usize = 3;

pub(super) struct PackageContents {
    pub(super) package_json: String,
    pub(super) descriptor_json: String,
    pub(super) adapter_manifest: String,
}

pub(super) fn read(root: &Path) -> Result<PackageContents, PluginError> {
    let root = open_package_directory(root)?;
    require_exact_members(
        &root,
        &[
            OsStr::new("manifests"),
            OsStr::new("orchester-plugin.json"),
            OsStr::new("package.json"),
        ],
        MAX_ROOT_ENTRIES,
    )?;
    let manifests = root
        .open_dir_nofollow("manifests")
        .map_err(|_| PluginError::InvalidPackage)?;
    let package_json = read_bounded_utf8(&root, "package.json", MAX_JSON_BYTES)?;
    let descriptor_json = read_bounded_utf8(&root, "orchester-plugin.json", MAX_JSON_BYTES)?;
    let manifest_name = descriptor_manifest_name(&descriptor_json)?;
    require_exact_members(&manifests, &[manifest_name.as_os_str()], 1)?;
    let adapter_manifest = read_bounded_utf8(&manifests, &manifest_name, MAX_MANIFEST_BYTES)?;
    Ok(PackageContents {
        package_json,
        descriptor_json,
        adapter_manifest,
    })
}

fn descriptor_manifest_name(source: &str) -> Result<OsString, PluginError> {
    let value: serde_json::Value =
        serde_json::from_str(source).map_err(|_| PluginError::InvalidDescriptor)?;
    let relative = value
        .get("adapterManifest")
        .and_then(serde_json::Value::as_str)
        .ok_or(PluginError::InvalidDescriptor)?;
    let mut components = Path::new(relative).components();
    if components.next() != Some(Component::Normal(OsStr::new("manifests"))) {
        return Err(PluginError::InvalidDescriptor);
    }
    let Some(Component::Normal(name)) = components.next() else {
        return Err(PluginError::InvalidDescriptor);
    };
    if components.next().is_some() || name.is_empty() {
        return Err(PluginError::InvalidDescriptor);
    }
    Ok(name.to_os_string())
}

fn open_package_directory(requested: &Path) -> Result<Dir, PluginError> {
    let absolute = absolute_lexical_path(requested)?;
    let mut anchor = PathBuf::new();
    let mut components = Vec::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => anchor.push(component.as_os_str()),
            Component::Normal(value) => components.push(value.to_os_string()),
            Component::CurDir => {}
            Component::ParentDir => return Err(PluginError::Unavailable),
        }
    }
    if anchor.as_os_str().is_empty() || components.is_empty() {
        return Err(PluginError::Unavailable);
    }
    let mut directory = Dir::open_ambient_dir(anchor, cap_std::ambient_authority())
        .map_err(|_| PluginError::Unavailable)?;
    for component in components {
        directory = directory
            .open_dir_nofollow(component)
            .map_err(|_| PluginError::Unavailable)?;
    }
    Ok(directory)
}

fn absolute_lexical_path(requested: &Path) -> Result<PathBuf, PluginError> {
    if requested.as_os_str().is_empty() {
        return Err(PluginError::Unavailable);
    }
    let absolute = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|_| PluginError::Unavailable)?
            .join(requested)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::Normal(value) => normalized.push(value),
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(PluginError::Unavailable);
                }
            }
        }
    }
    if !normalized.is_absolute() {
        return Err(PluginError::Unavailable);
    }
    Ok(normalized)
}

fn require_exact_members(
    directory: &Dir,
    expected: &[&OsStr],
    maximum: usize,
) -> Result<(), PluginError> {
    let entries = directory
        .entries()
        .map_err(|_| PluginError::InvalidPackage)?;
    let mut names = Vec::with_capacity(expected.len());
    for entry in entries {
        if names.len() == maximum {
            return Err(PluginError::InvalidPackage);
        }
        let entry = entry.map_err(|_| PluginError::InvalidPackage)?;
        names.push(entry.file_name());
    }
    names.sort();
    let mut expected = expected
        .iter()
        .map(|name| name.to_os_string())
        .collect::<Vec<_>>();
    expected.sort();
    if names != expected {
        return Err(PluginError::InvalidPackage);
    }
    Ok(())
}

fn read_bounded_utf8(
    directory: &Dir,
    name: impl AsRef<Path>,
    maximum: u64,
) -> Result<String, PluginError> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    #[cfg(unix)]
    options.nonblock(true);
    let mut file = directory
        .open_with(name, &options)
        .map_err(|_| PluginError::InvalidPackage)?;
    let metadata = file.metadata().map_err(|_| PluginError::InvalidPackage)?;
    if !metadata.is_file() {
        return Err(PluginError::InvalidPackage);
    }
    if metadata.len() > maximum {
        return Err(PluginError::LimitExceeded);
    }
    let mut bytes = Vec::with_capacity(metadata.len().min(64 * 1024) as usize);
    Read::by_ref(&mut file)
        .take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| PluginError::InvalidPackage)?;
    if bytes.len() as u64 > maximum {
        return Err(PluginError::LimitExceeded);
    }
    String::from_utf8(bytes).map_err(|_| PluginError::InvalidPackage)
}
