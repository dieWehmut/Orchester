use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use thiserror::Error;

const MAX_ARCHIVE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_MEMBER_BYTES: u64 = 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 2 * 1024 * 1024;
const MEMBER_COUNT: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ArchiveError {
    #[error("invalid plugin archive")]
    Invalid,
}

pub fn materialize(
    archive_path: &Path,
    destination: &Path,
    name: &str,
) -> Result<(), ArchiveError> {
    let metadata = fs::symlink_metadata(archive_path).map_err(|_| ArchiveError::Invalid)?;
    if !metadata.is_file() || is_link_or_reparse(&metadata) || metadata.len() > MAX_ARCHIVE_BYTES {
        return Err(ArchiveError::Invalid);
    }
    let file = fs::File::open(archive_path).map_err(|_| ArchiveError::Invalid)?;
    let decoder = GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    let entries = archive.entries().map_err(|_| ArchiveError::Invalid)?;
    let expected = expected_paths(name);
    let mut contents = BTreeMap::new();
    let mut total = 0_u64;
    for entry in entries {
        if contents.len() == MEMBER_COUNT {
            return Err(ArchiveError::Invalid);
        }
        let mut entry = entry.map_err(|_| ArchiveError::Invalid)?;
        if !entry.header().entry_type().is_file() || entry.size() > MAX_MEMBER_BYTES {
            return Err(ArchiveError::Invalid);
        }
        let path = entry
            .path()
            .map_err(|_| ArchiveError::Invalid)?
            .into_owned();
        if !expected.contains(&path) || contents.contains_key(&path) {
            return Err(ArchiveError::Invalid);
        }
        total = total
            .checked_add(entry.size())
            .filter(|size| *size <= MAX_TOTAL_BYTES)
            .ok_or(ArchiveError::Invalid)?;
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        Read::by_ref(&mut entry)
            .take(MAX_MEMBER_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| ArchiveError::Invalid)?;
        if bytes.len() as u64 != entry.size() {
            return Err(ArchiveError::Invalid);
        }
        contents.insert(path, bytes);
    }
    if contents.len() != MEMBER_COUNT {
        return Err(ArchiveError::Invalid);
    }

    fs::create_dir(destination).map_err(|_| ArchiveError::Invalid)?;
    fs::create_dir(destination.join("manifests")).map_err(|_| ArchiveError::Invalid)?;
    write_member(
        destination.join("package.json"),
        contents.remove(Path::new("package/package.json")),
    )?;
    write_member(
        destination.join("orchester-plugin.json"),
        contents.remove(Path::new("package/orchester-plugin.json")),
    )?;
    write_member(
        destination.join("manifests").join(format!("{name}.toml")),
        contents.remove(&PathBuf::from(format!("package/manifests/{name}.toml"))),
    )
}

fn expected_paths(name: &str) -> [PathBuf; MEMBER_COUNT] {
    [
        PathBuf::from("package/package.json"),
        PathBuf::from("package/orchester-plugin.json"),
        PathBuf::from(format!("package/manifests/{name}.toml")),
    ]
}

fn write_member(path: PathBuf, bytes: Option<Vec<u8>>) -> Result<(), ArchiveError> {
    fs::write(path, bytes.ok_or(ArchiveError::Invalid)?).map_err(|_| ArchiveError::Invalid)
}

fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}
