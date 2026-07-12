use std::fs;
use std::path::{Path, PathBuf};

use orchester_laufzeit::harness::files::{EntryKind, FileToolError, FileToolLimits, FileTools};

fn workspace(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "orchester-file-tools-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("src/nested")).unwrap();
    fs::write(
        root.join("src/lib.rs"),
        "fn main() {\n  println!(\"hello\");\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src/nested/mod.rs"),
        "pub const VALUE: &str = \"needle\";\n",
    )
    .unwrap();
    fs::create_dir_all(root.join(".orchester")).unwrap();
    fs::write(root.join(".orchester/secret"), "sk-should-not-be-readable").unwrap();
    root
}

#[test]
fn reads_bounded_line_ranges_and_lists_capability_relative_entries() {
    let root = workspace("read-list");
    let tools = FileTools::new(&root, FileToolLimits::default()).unwrap();
    let read = tools
        .read_file(Path::new("src/lib.rs"), Some(2), Some(2))
        .unwrap();
    assert_eq!(read.content, "  println!(\"hello\");");
    assert_eq!(read.lines, 1);

    let listed = tools.list_files(Path::new("src"), 2).unwrap();
    assert!(listed
        .entries
        .iter()
        .any(|entry| entry.path == Path::new("src/lib.rs") && entry.kind == EntryKind::File));
    assert!(listed.entries.iter().any(|entry| {
        entry.path == Path::new("src/nested") && entry.kind == EntryKind::Directory
    }));
    drop(tools);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn search_is_bounded_and_protected_paths_are_rejected() {
    let root = workspace("search");
    let tools = FileTools::new(
        &root,
        FileToolLimits {
            max_read_bytes: 1024,
            max_list_entries: 32,
            max_search_matches: 4,
            max_match_bytes: 128,
        },
    )
    .unwrap();
    let result = tools.search_text(Path::new("src"), "needle").unwrap();
    assert_eq!(result.matches.len(), 1);
    assert_eq!(result.matches[0].line, 1);
    assert!(matches!(
        tools.read_file(Path::new(".orchester/secret"), None, None),
        Err(FileToolError::Guard(_))
    ));
    assert!(matches!(
        tools.list_files(Path::new("../"), 2),
        Err(FileToolError::Guard(_))
    ));
    drop(tools);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn read_and_list_limits_fail_closed_before_unbounded_output() {
    let root = workspace("limits");
    fs::write(root.join("src/large.txt"), "x".repeat(1024)).unwrap();
    fs::write(root.join("src/extra.txt"), "extra").unwrap();
    let tools = FileTools::new(
        &root,
        FileToolLimits {
            max_read_bytes: 32,
            max_list_entries: 1,
            max_search_matches: 1,
            max_match_bytes: 32,
        },
    )
    .unwrap();
    assert!(matches!(
        tools.read_file(Path::new("src/large.txt"), None, None),
        Err(FileToolError::LimitExceeded)
    ));
    assert!(matches!(
        tools.list_files(Path::new("src"), 2),
        Err(FileToolError::LimitExceeded)
    ));
    drop(tools);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn configured_limits_have_absolute_safety_bounds() {
    let root = workspace("config-bounds");
    assert!(matches!(
        FileTools::new(
            &root,
            FileToolLimits {
                max_read_bytes: u64::MAX,
                ..FileToolLimits::default()
            }
        ),
        Err(FileToolError::InvalidInput)
    ));
    assert!(matches!(
        FileTools::new(
            &root,
            FileToolLimits {
                max_list_entries: usize::MAX,
                ..FileToolLimits::default()
            }
        ),
        Err(FileToolError::InvalidInput)
    ));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn read_and_search_redact_common_provider_credentials() {
    let root = workspace("redaction");
    let secret = "sk-live-file-tool-secret";
    fs::write(
        root.join("src/config.env"),
        format!("OPENAI_API_KEY={secret}\nendpoint=https://example.invalid\n"),
    )
    .unwrap();
    let tools = FileTools::new(&root, FileToolLimits::default()).unwrap();
    let read = tools
        .read_file(Path::new("src/config.env"), None, None)
        .unwrap();
    assert!(!read.content.contains(secret));
    assert!(read.content.contains("[REDACTED]"));
    let search = tools
        .search_text(Path::new("src"), "OPENAI_API_KEY")
        .unwrap();
    assert_eq!(search.matches.len(), 1);
    assert!(!search.matches[0].text.contains(secret));
    drop(tools);
    fs::remove_dir_all(root).unwrap();
}
