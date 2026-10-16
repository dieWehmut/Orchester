use super::{transcript, ActivityDetail, SESSION_LIMIT};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::SystemTime,
};

const HEAD_BYTES: u64 = 128 * 1024;
const TAIL_BYTES: u64 = 512 * 1024;

pub(super) struct CachedFile {
    length: u64,
    modified: SystemTime,
    row: ActivityDetail,
}

pub(super) fn linked(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    metadata.file_type().is_symlink()
}

pub(super) fn read_provider(
    provider: &str,
    root: Option<&Path>,
    cache: &mut HashMap<PathBuf, CachedFile>,
) -> (&'static str, Vec<ActivityDetail>) {
    let Some(root) = root else {
        return ("not_found", Vec::new());
    };
    if !root.exists() {
        return ("not_found", Vec::new());
    }
    let Ok(root) = root.canonicalize() else {
        return ("unavailable", Vec::new());
    };
    let hints = (provider == "codex")
        .then(|| super::metadata::codex(&root))
        .flatten();
    let mut files = hints
        .as_ref()
        .map(|hints| {
            hints
                .keys()
                .filter_map(|path| {
                    let metadata = fs::metadata(path).ok()?;
                    Some((path.clone(), metadata.len(), metadata.modified().ok()?))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut budget = 12_000;
    let incomplete =
        files.is_empty() && (visit(&root, 0, &mut budget, &mut files).is_err() || budget == 0);
    if incomplete && files.is_empty() {
        return ("unavailable", Vec::new());
    }
    let pinned = |path: &PathBuf| {
        hints
            .as_ref()
            .and_then(|hints| hints.get(path))
            .is_some_and(|hint| hint.pinned())
    };
    files.sort_by(|a, b| {
        pinned(&b.0)
            .cmp(&pinned(&a.0))
            .then(b.2.cmp(&a.2))
            .then(a.0.cmp(&b.0))
    });
    files.truncate(SESSION_LIMIT);
    let mut rows = Vec::new();
    let mut failures = incomplete;
    for (path, length, modified) in files {
        if let Some(cached) = cache
            .get(&path)
            .filter(|entry| entry.length == length && entry.modified == modified)
        {
            let mut row = cached.row.clone();
            if let Some(hint) = hints.as_ref().and_then(|hints| hints.get(&path)) {
                hint.apply(&mut row);
            }
            rows.push(row);
            continue;
        }
        let result = read_file(&root, &path, length).map(|records| {
            let updated = modified
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                .min(i64::MAX as u128) as i64;
            let mut row = transcript::parse(
                provider,
                &path.strip_prefix(&root).unwrap_or(&path).to_string_lossy(),
                updated,
                &records,
            );
            row.summary.truncated |= length > HEAD_BYTES + TAIL_BYTES;
            row
        });
        match result {
            Ok(mut row) => {
                let hint = hints.as_ref().and_then(|hints| hints.get(&path));
                cache.insert(
                    path,
                    CachedFile {
                        length,
                        modified,
                        row: row.clone(),
                    },
                );
                if let Some(hint) = hint {
                    hint.apply(&mut row);
                }
                rows.push(row);
            }
            Err(_) => failures = true,
        }
    }
    (if failures { "partial" } else { "available" }, rows)
}

fn visit(
    root: &Path,
    depth: usize,
    budget: &mut usize,
    files: &mut Vec<(PathBuf, u64, SystemTime)>,
) -> std::io::Result<()> {
    if depth > 6 || *budget == 0 {
        return Ok(());
    }
    let mut entries = fs::read_dir(root)?
        .take(*budget)
        .collect::<Result<Vec<_>, _>>()?;
    // Codex uses year/month/day folders. Search newest folders before spending
    // the bounded traversal budget on historical or nested agent records.
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.file_name()));
    for entry in entries {
        if *budget == 0 {
            break;
        }
        *budget -= 1;
        let metadata = fs::symlink_metadata(entry.path())?;
        if linked(&metadata) {
            continue;
        }
        if metadata.is_dir() {
            visit(&entry.path(), depth + 1, budget, files)?;
        } else if metadata.is_file() && entry.path().extension().is_some_and(|ext| ext == "jsonl") {
            files.push((entry.path(), metadata.len(), metadata.modified()?));
        }
    }
    Ok(())
}

fn read_file(
    root: &Path,
    path: &Path,
    length: u64,
) -> std::io::Result<Vec<(u64, serde_json::Value)>> {
    if linked(&fs::symlink_metadata(path)?) || !path.canonicalize()?.starts_with(root) {
        return Err(std::io::ErrorKind::PermissionDenied.into());
    }
    let mut file = File::open(path)?;
    let mut records = Vec::new();
    if length <= HEAD_BYTES + TAIL_BYTES {
        let mut bytes = Vec::new();
        file.take(HEAD_BYTES + TAIL_BYTES).read_to_end(&mut bytes)?;
        parse_lines(&bytes, 0, &mut records);
    } else {
        let mut head = vec![0; HEAD_BYTES as usize];
        file.read_exact(&mut head)?;
        parse_lines(&head, 0, &mut records);
        let start = length - TAIL_BYTES;
        file.seek(SeekFrom::Start(start))?;
        let mut tail = Vec::new();
        file.take(TAIL_BYTES).read_to_end(&mut tail)?;
        if let Some(newline) = tail.iter().position(|byte| *byte == b'\n') {
            parse_lines(
                &tail[newline + 1..],
                start + newline as u64 + 1,
                &mut records,
            );
        }
    }
    Ok(records)
}

fn parse_lines(bytes: &[u8], mut offset: u64, records: &mut Vec<(u64, serde_json::Value)>) {
    for line in bytes.split_inclusive(|byte| *byte == b'\n') {
        // Ignore an in-flight last line; the next refresh reads its completed form.
        if line.ends_with(b"\n") {
            if let Ok(record) = serde_json::from_slice(line) {
                records.push((offset, record));
            }
        }
        offset += line.len() as u64;
    }
}
