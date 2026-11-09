use super::{transcript, ActivityDetail, SESSION_LIMIT};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::SystemTime,
};

const HEAD_BYTES: u64 = 128 * 1024;
const TAIL_BYTES: u64 = 512 * 1024;
const EXCERPT_SESSIONS: usize = 128;
const EXCERPT_BYTES: usize = 8 * 1024 * 1024;
const DETAIL_CACHE_BYTES: usize = 16 * 1024 * 1024;

pub(super) struct CachedFile {
    length: u64,
    modified: SystemTime,
    row: ActivityDetail,
    excerpt_cached: bool,
    excerpt_budget: Option<usize>,
}

pub(super) fn is_transcript(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "jsonl")
        || path.to_string_lossy().ends_with(".jsonl.zst")
}

fn source_id(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path).to_string_lossy();
    relative
        .strip_suffix(".zst")
        .unwrap_or(&relative)
        .to_owned()
}

fn updated(modified: SystemTime) -> i64 {
    modified
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

pub(super) fn codex_detail(
    root: Option<&Path>,
    id: &str,
    before: Option<u64>,
    cache: &mut HashMap<(PathBuf, Option<u64>), CachedFile>,
) -> std::io::Result<Option<ActivityDetail>> {
    let Some(root) = root else {
        return Ok(None);
    };
    let root = root.canonicalize()?;
    let hints = super::metadata::codex(&root);
    let mut paths = hints
        .as_ref()
        .map(|hints| hints.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    if hints.is_none() {
        let mut files = Vec::new();
        visit(&root, 0, &mut 12_000, &mut files)?;
        paths.extend(files.into_iter().map(|(path, _, _)| path));
    }
    let Some(path) = paths
        .into_iter()
        .find(|path| super::opaque_id("codex", &source_id(&root, path)) == id)
    else {
        return Ok(None);
    };
    let metadata = fs::symlink_metadata(&path)?;
    let modified = metadata.modified()?;
    let key = (path.clone(), before);
    let mut row = if let Some(cached) = cache
        .get(&key)
        .filter(|entry| entry.length == metadata.len() && entry.modified == modified)
    {
        cached.row.clone()
    } else {
        let row = read_codex(&root, &path, modified, before)?;
        // Keep only a few decoded pages; browsing never expands an unbounded cache.
        let bytes = excerpt_size(&row);
        if cache.len() >= 16
            || cache
                .values()
                .map(|entry| excerpt_size(&entry.row))
                .sum::<usize>()
                + bytes
                > DETAIL_CACHE_BYTES
        {
            cache.clear();
        }
        if bytes <= DETAIL_CACHE_BYTES {
            cache.insert(
                key,
                CachedFile {
                    length: metadata.len(),
                    modified,
                    row: row.clone(),
                    excerpt_cached: true,
                    excerpt_budget: None,
                },
            );
        }
        row
    };
    if let Some(hint) = hints.as_ref().and_then(|hints| hints.get(&path)) {
        hint.apply(&mut row);
    }
    Ok(Some(row))
}

fn read_codex(
    root: &Path,
    path: &Path,
    modified: SystemTime,
    before: Option<u64>,
) -> std::io::Result<ActivityDetail> {
    if linked(&fs::symlink_metadata(path)?) || !path.canonicalize()?.starts_with(root) {
        return Err(std::io::ErrorKind::PermissionDenied.into());
    }
    let file = File::open(path)?;
    let reader: Box<dyn Read> = if path.extension().is_some_and(|ext| ext == "zst") {
        let mut decoder = zstd::stream::read::Decoder::new(file)?;
        decoder.window_log_max(27)?;
        Box::new(decoder)
    } else {
        Box::new(file)
    };
    let mut reader = BufReader::new(reader);
    let mut history =
        transcript::CodexHistory::new(&source_id(root, path), updated(modified), before);
    let mut line = Vec::new();
    let mut offset = 0_u64;
    loop {
        line.clear();
        // Bound a single source record, never discard the middle of a conversation.
        let size = reader
            .by_ref()
            .take(16 * 1024 * 1024 + 1)
            .read_until(b'\n', &mut line)?;
        if size == 0 {
            break;
        }
        if size > 16 * 1024 * 1024 {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        if line.ends_with(b"\n") {
            if let Ok(record) = serde_json::from_slice(&line) {
                history.record(offset, &record);
            }
        }
        offset += size as u64;
    }
    Ok(history.finish())
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
        cache.retain(|path, _| !path.starts_with(root));
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
        hints.is_none() && (visit(&root, 0, &mut budget, &mut files).is_err() || budget == 0);
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
    if provider != "codex" {
        files.truncate(SESSION_LIMIT);
    }
    let visible = files
        .iter()
        .map(|(path, _, _)| path)
        .collect::<HashSet<_>>();
    // Preserve unchanged catalog rows instead of invalidating every transcript
    // when a provider has more than 128 sessions. Prune only this provider root.
    cache.retain(|path, _| !path.starts_with(&root) || visible.contains(path));
    let mut rows = Vec::new();
    let mut failures = incomplete;
    let mut excerpt_bytes = EXCERPT_BYTES;
    for (index, (path, length, modified)) in files.into_iter().enumerate() {
        let keep_excerpt = index < EXCERPT_SESSIONS;
        let cached = cache.get(&path).filter(|entry| {
            entry.length == length
                && entry.modified == modified
                && (!keep_excerpt || entry.excerpt_cached)
                && (!keep_excerpt
                    || entry
                        .excerpt_budget
                        .map_or(true, |budget| excerpt_bytes <= budget))
        });
        let inherited_budget = cached.and_then(|entry| entry.excerpt_budget);
        let result = if provider == "codex"
            && hints.is_some()
            && path.extension().is_some_and(|ext| ext == "zst")
        {
            // The index supplies historical sidebar rows without decoding every archive on every poll.
            Ok(super::empty_detail(
                provider,
                &source_id(&root, &path),
                updated(modified),
            ))
        } else if let Some(cached) = cached {
            Ok(cached.row.clone())
        } else if provider == "codex" {
            read_codex(&root, &path, modified, None)
        } else {
            read_file(&root, &path, length).map(|records| {
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
            })
        };
        match result {
            Ok(mut row) => {
                let available = excerpt_bytes;
                let trimmed = limit_excerpt(&mut row, keep_excerpt, &mut excerpt_bytes);
                let hint = hints.as_ref().and_then(|hints| hints.get(&path));
                cache.insert(
                    path,
                    CachedFile {
                        length,
                        modified,
                        row: row.clone(),
                        excerpt_cached: keep_excerpt,
                        excerpt_budget: if trimmed {
                            Some(available)
                        } else if keep_excerpt {
                            inherited_budget
                        } else {
                            None
                        },
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
    rows.retain(|row| !row.internal_subagent);
    (if failures { "partial" } else { "available" }, rows)
}

fn excerpt_size(row: &ActivityDetail) -> usize {
    let string_size = |value: &Option<String>| value.as_ref().map_or(0, String::capacity);
    row.entries.len() * std::mem::size_of::<super::ActivityEntry>()
        + row.turns.len() * std::mem::size_of::<super::ActivityTurn>()
        + row
            .entries
            .iter()
            .map(|entry| {
                entry.id.capacity()
                    + entry.text.capacity()
                    + string_size(&entry.tool)
                    + string_size(&entry.turn_id)
                    + string_size(&entry.call_id)
            })
            .sum::<usize>()
        + row
            .turns
            .iter()
            .map(|turn| turn.id.capacity())
            .sum::<usize>()
}

fn limit_excerpt(row: &mut ActivityDetail, keep: bool, remaining: &mut usize) -> bool {
    if !keep {
        // Historical summaries stay visible and their full paginated transcript
        // is read by codex_detail on demand. Release the vectors' allocations too.
        row.entries = Vec::new();
        row.turns = Vec::new();
        row.next_before = None;
        return false;
    }
    // Match the allocations retained by clones, so the same unchanged catalog
    // has a stable byte budget across refreshes.
    for entry in &mut row.entries {
        entry.id.shrink_to_fit();
        entry.text.shrink_to_fit();
        for value in [&mut entry.tool, &mut entry.turn_id, &mut entry.call_id]
            .into_iter()
            .flatten()
        {
            value.shrink_to_fit();
        }
    }
    for turn in &mut row.turns {
        turn.id.shrink_to_fit();
    }
    let mut trimmed = false;
    while excerpt_size(row) > *remaining && !row.entries.is_empty() {
        row.entries.remove(0);
        row.turns.retain(|turn| {
            row.entries
                .iter()
                .any(|entry| entry.turn_id.as_ref() == Some(&turn.id))
        });
        trimmed = true;
    }
    if trimmed {
        row.summary.truncated = true;
    }
    row.entries.shrink_to_fit();
    row.turns.shrink_to_fit();
    *remaining = remaining.saturating_sub(excerpt_size(row));
    trimmed
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
        } else if metadata.is_file() && is_transcript(&entry.path()) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn catalog_excerpt_cache_has_a_total_byte_budget_without_limiting_detail() {
        let directory = tempfile::tempdir().unwrap();
        for index in 0..3 {
            let mut file =
                File::create(directory.path().join(format!("task-{index}.jsonl"))).unwrap();
            writeln!(file, "{}", serde_json::json!({"type":"response_item", "payload":{"type":"message", "role":"user", "content":[{"type":"input_text", "text":format!("Task {index}")}]}})).unwrap();
            for _ in 0..40 {
                writeln!(file, "{}", serde_json::json!({"type":"response_item", "payload":{"type":"message", "role":"assistant", "content":[{"type":"output_text", "text":"x".repeat(90_000)}]}})).unwrap();
            }
        }
        let mut cache = HashMap::new();
        for _ in 0..2 {
            let (status, rows) = read_provider("codex", Some(directory.path()), &mut cache);
            assert_eq!(status, "available");
            assert_eq!(rows.len(), 3);
            assert!(rows.iter().map(excerpt_size).sum::<usize>() <= EXCERPT_BYTES);
            assert!(
                cache
                    .values()
                    .map(|entry| excerpt_size(&entry.row))
                    .sum::<usize>()
                    <= EXCERPT_BYTES
            );
            let limited = rows.iter().find(|row| row.summary.truncated).unwrap();
            let detail = codex_detail(
                Some(directory.path()),
                &limited.summary.id,
                None,
                &mut HashMap::new(),
            )
            .unwrap()
            .unwrap();
            assert_eq!(detail.entries.len(), 41);
            assert!(!detail.summary.truncated);
        }
        let limited = cache
            .iter()
            .find(|(_, entry)| entry.row.summary.truncated)
            .unwrap()
            .0
            .clone();
        // Cache keys are canonical paths (including the Windows verbatim
        // prefix); compare the same path representation when keeping one file.
        for entry in fs::read_dir(directory.path().canonicalize().unwrap()).unwrap() {
            let path = entry.unwrap().path();
            if path != limited {
                fs::remove_file(path).unwrap();
            }
        }
        let (_, expanded) = read_provider("codex", Some(directory.path()), &mut cache);
        assert_eq!(expanded.len(), 1);
        assert_eq!(expanded[0].entries.len(), 41);
        assert!(!expanded[0].summary.truncated);
        let mut pages = HashMap::new();
        for before in 1..=6 {
            let row = codex_detail(
                Some(directory.path()),
                &expanded[0].summary.id,
                Some(u64::MAX - before),
                &mut pages,
            )
            .unwrap()
            .unwrap();
            assert_eq!(row.entries.len(), 41);
            assert!(
                pages
                    .values()
                    .map(|entry| excerpt_size(&entry.row))
                    .sum::<usize>()
                    <= DETAIL_CACHE_BYTES
            );
        }
    }
}
