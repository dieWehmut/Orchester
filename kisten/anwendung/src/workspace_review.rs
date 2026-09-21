//! The read-only workspace review: the git facts the Review tab is built on.
//!
//! The design spec's Review tab filters changes by staged, unstaged, branch and
//! last turn. Three of those are facts about the git working copy, and section 0
//! of the design spec is explicit that the runtime is the source of truth (P8):
//! a view may not invent a staging state the runtime cannot read. So the runtime
//! reads it, read-only, from `git status` and the branch's own upstream.
//!
//! Two decisions are worth stating because they keep this honest:
//!
//! * **The review is read-only.** Every invocation is a query — `status`,
//!   `symbolic-ref`, `rev-parse`, `diff` — and `--no-optional-locks` keeps even
//!   the index refresh from writing, so asking what changed can never disturb a
//!   run that is changing it.
//! * **A directory that is not a repository is a state, not an error.** The
//!   caller gets `NotARepository` and renders an empty review, because Orchester
//!   runs against plain directories too and that is a designed state (P6).
//!
//! What is deliberately *not* here: the "last turn" filter. Which turn a change
//! belongs to is a fact about a run's event stream, not about the working copy,
//! and this crate reads no events. The frontend joins the two.

use std::path::Path;
use std::process::{Command, Output, Stdio};

use thiserror::Error;

/// What a path's change is, in the vocabulary the diff decorations use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceReviewKind {
    Added,
    Modified,
    Deleted,
    /// A path git is not tracking at all; staged work it has never seen.
    Untracked,
}

/// One path the working copy has changed, and where the change sits.
///
/// `staged` and `unstaged` are independent on purpose. Staging a file and then
/// editing it again is ordinary work, and it leaves one path with a version in
/// the index and a different version on disk; both filters have to find it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceReviewChange {
    pub path: String,
    pub kind: WorkspaceReviewKind,
    pub staged: bool,
    pub unstaged: bool,
}

/// The working copy's change set, and the branch's own committed work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceReview {
    /// The branch HEAD points at, or `None` when HEAD is detached.
    pub branch: Option<String>,
    /// Every path the working copy has touched: staged, unstaged or untracked.
    pub changes: Vec<WorkspaceReviewChange>,
    /// Paths this branch changed relative to its upstream, so "branch" can mean
    /// the work committed here rather than the work not yet committed.
    pub branch_changes: Vec<String>,
}

/// Why a review could not be read. Variants carry no user-controlled text, so
/// they are safe to surface in an error state or an audit line.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WorkspaceReviewError {
    /// The workspace is not a readable git work tree.
    #[error("the workspace is not a git work tree")]
    NotARepository,
    /// The workspace is a repository, but its state could not be read.
    #[error("the workspace review could not be read")]
    Unreadable,
}

impl WorkspaceReview {
    /// Read the working copy's review state without writing anything.
    pub fn read(workspace: &Path) -> Result<Self, WorkspaceReviewError> {
        if !is_work_tree(workspace) {
            return Err(WorkspaceReviewError::NotARepository);
        }

        let status = git(
            workspace,
            &[
                "--no-optional-locks",
                "status",
                "--porcelain=v1",
                "-z",
                "--untracked-files=all",
            ],
        )
        .map_err(|_| WorkspaceReviewError::Unreadable)?;
        if !status.status.success() {
            return Err(WorkspaceReviewError::Unreadable);
        }

        Ok(Self {
            branch: branch_of(workspace),
            changes: parse_status(&status.stdout),
            branch_changes: branch_changes(workspace),
        })
    }
}

/// Whether the given path is inside a git work tree.
///
/// Every way of failing answers the same question the same way: a directory that
/// does not exist, a machine without `git`, a bare repository and a plain folder
/// all have no readable working copy, so all of them are `NotARepository`.
fn is_work_tree(workspace: &Path) -> bool {
    git(workspace, &["rev-parse", "--is-inside-work-tree"])
        .map(|output| output.status.success() && output.stdout == b"true\n")
        .unwrap_or(false)
}

fn branch_of(workspace: &Path) -> Option<String> {
    let output = git(workspace, &["symbolic-ref", "--short", "-q", "HEAD"]).ok()?;
    if !output.status.success() {
        return None;
    }
    non_empty_line(&output.stdout)
}

/// The paths this branch changed relative to its upstream.
///
/// A branch with no upstream has no such set, which is not a failure: an
/// unpublished branch simply has nothing to compare against, so the answer is
/// empty rather than an error.
fn branch_changes(workspace: &Path) -> Vec<String> {
    let Some(upstream) = upstream_of(workspace) else {
        return Vec::new();
    };

    // Three dots, because the question is "what did this branch do", not "how
    // does it differ from every commit upstream has made since". Without a merge
    // base the diff would list upstream's own work as this branch's.
    let range = format!("{upstream}...HEAD");
    let Ok(output) = git(workspace, &["diff", "--name-only", "-z", &range]) else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    nul_fields(&output.stdout)
        .map(|field| String::from_utf8_lossy(field).into_owned())
        .collect()
}

/// The full ref name of the current branch's upstream, if it has one.
fn upstream_of(workspace: &Path) -> Option<String> {
    let output = git(
        workspace,
        &[
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ],
    )
    .ok()?;
    if !output.status.success() {
        return None;
    }
    non_empty_line(&output.stdout)
}

/// Parse `git status --porcelain=v1 -z` into the review's change list.
///
/// The `-z` form is NUL-separated and never quotes a path, which is what lets a
/// path with a space or a non-ASCII byte survive the trip. It has one wrinkle: a
/// rename or a copy spends a second field on the path the entry came from, so
/// the parser has to consume it rather than read it as another entry.
fn parse_status(stdout: &[u8]) -> Vec<WorkspaceReviewChange> {
    let fields: Vec<&[u8]> = nul_fields(stdout).collect();

    let mut changes = Vec::new();
    let mut index = 0;
    while index < fields.len() {
        let field = fields[index];
        index += 1;

        // "XY PATH": two status columns, a space, then the path. A field too
        // short to hold that is not an entry this parser understands, and
        // guessing at it would invent a path the working copy never reported.
        if field.len() < 4 || field[2] != b' ' {
            continue;
        }
        let (x, y) = (field[0], field[1]);

        // A rename or a copy reports its source in the next NUL-separated field.
        if matches!(x, b'R' | b'C') {
            index += 1;
        }

        changes.push(WorkspaceReviewChange {
            path: String::from_utf8_lossy(&field[3..]).into_owned(),
            kind: classify(x, y),
            staged: x != b' ' && x != b'?',
            unstaged: (y != b' ' && y != b'?') || x == b'?',
        });
    }

    changes.sort_by(|left, right| left.path.cmp(&right.path));
    changes
}

/// Which decoration a status entry gets.
///
/// The index column wins when both columns carry a change: the staged version is
/// what a review reads first, and a rename is reported as a modification because
/// the design spec's decoration vocabulary is add / remove / modify and the path
/// is still present.
fn classify(x: u8, y: u8) -> WorkspaceReviewKind {
    if x == b'?' {
        return WorkspaceReviewKind::Untracked;
    }
    let code = if x != b' ' { x } else { y };
    match code {
        b'A' | b'C' => WorkspaceReviewKind::Added,
        b'D' => WorkspaceReviewKind::Deleted,
        _ => WorkspaceReviewKind::Modified,
    }
}

/// The non-empty NUL-separated fields of a git `-z` payload.
fn nul_fields(stdout: &[u8]) -> impl Iterator<Item = &[u8]> {
    stdout
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
}

fn non_empty_line(stdout: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(stdout);
    let line = text.trim();
    if line.is_empty() {
        None
    } else {
        Some(line.to_owned())
    }
}

/// Run one read-only git query in the workspace.
///
/// The child gets no stdin and no terminal prompt, so a missing credential or a
/// paused hook can never turn a status read into a hung frontend.
fn git(workspace: &Path, args: &[&str]) -> std::io::Result<Output> {
    Command::new("git")
        .current_dir(workspace)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .output()
}
