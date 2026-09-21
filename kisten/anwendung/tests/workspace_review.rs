//! The read-only workspace review, the data the Review tab filters.
//!
//! The design spec's Review tab filters changes by staged, unstaged, branch and
//! last turn. Three of those are facts about the git working copy, and section 0
//! of the design spec is explicit that the runtime is the source of truth: a
//! view may not invent a staging state the runtime cannot read. So the runtime
//! reads it, read-only, from `git status` and the branch's own upstream.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use orchester_anwendung::{WorkspaceReview, WorkspaceReviewError, WorkspaceReviewKind};

struct TempRepo(PathBuf);

impl TempRepo {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("orchester-review-{label}-{nonce}"));
        fs::create_dir_all(&path).expect("temporary directory");
        let repo = Self(path);
        repo.git(&["init", "--quiet"]);
        repo.write("src/committed.txt", "committed\n");
        repo.git(&["add", "."]);
        repo.git(&[
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "user.name=Test",
            "commit",
            "--quiet",
            "-m",
            "initial",
        ]);
        repo
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn git(&self, args: &[&str]) {
        let status = Command::new("git")
            .current_dir(&self.0)
            .args(args)
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?} failed");
    }

    fn write(&self, relative: &str, contents: &str) {
        let target = self.0.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).expect("parent directory");
        }
        fs::write(target, contents).expect("file written");
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn kind_of<'a>(review: &'a WorkspaceReview, path: &str) -> &'a WorkspaceReviewKind {
    &review
        .changes
        .iter()
        .find(|change| change.path == path)
        .unwrap_or_else(|| panic!("no review entry for {path}"))
        .kind
}

fn entry<'a>(
    review: &'a WorkspaceReview,
    path: &str,
) -> &'a orchester_anwendung::WorkspaceReviewChange {
    review
        .changes
        .iter()
        .find(|change| change.path == path)
        .unwrap_or_else(|| panic!("no review entry for {path}"))
}

#[test]
fn reports_the_branch_and_an_empty_working_copy() {
    let repo = TempRepo::new("clean");
    let review = WorkspaceReview::read(repo.path()).expect("review reads");

    // `git init` names the branch; on a machine with no advice configured it is
    // still a branch, and the review reports it rather than inventing one.
    assert!(review.branch.is_some());
    assert!(review.changes.is_empty());
    assert!(review.branch_changes.is_empty());
}

#[test]
fn separates_what_is_staged_from_what_is_not() {
    let repo = TempRepo::new("staged");
    repo.write("src/staged.txt", "staged\n");
    repo.git(&["add", "src/staged.txt"]);
    repo.write("src/committed.txt", "changed but not staged\n");
    repo.write("src/untracked.txt", "untracked\n");

    let review = WorkspaceReview::read(repo.path()).expect("review reads");

    let staged = entry(&review, "src/staged.txt");
    assert!(staged.staged, "an added path is staged");
    assert!(!staged.unstaged, "an added path has nothing unstaged");

    let modified = entry(&review, "src/committed.txt");
    assert!(
        !modified.staged,
        "a modification after adding is not staged"
    );
    assert!(modified.unstaged, "the same modification is unstaged work");

    let untracked = entry(&review, "src/untracked.txt");
    assert!(!untracked.staged);
    assert!(untracked.unstaged, "an untracked path is unstaged work");

    assert_eq!(
        *kind_of(&review, "src/staged.txt"),
        WorkspaceReviewKind::Added
    );
    assert_eq!(
        *kind_of(&review, "src/committed.txt"),
        WorkspaceReviewKind::Modified
    );
    assert_eq!(
        *kind_of(&review, "src/untracked.txt"),
        WorkspaceReviewKind::Untracked
    );
}

#[test]
fn reports_a_path_that_is_staged_and_then_changed_again_as_both() {
    // This is the case the two filters exist for: staging a file and then
    // editing it again leaves one path with staged and unstaged work at once,
    // and both filters have to find it.
    let repo = TempRepo::new("both");
    repo.write("src/committed.txt", "staged version\n");
    repo.git(&["add", "src/committed.txt"]);
    repo.write("src/committed.txt", "edited again\n");

    let review = WorkspaceReview::read(repo.path()).expect("review reads");
    let both = entry(&review, "src/committed.txt");

    assert!(both.staged, "the first version is staged");
    assert!(both.unstaged, "the second version is unstaged");
}

#[test]
fn lists_deleted_paths_without_treating_them_as_untracked() {
    let repo = TempRepo::new("deleted");
    repo.git(&["rm", "--quiet", "src/committed.txt"]);

    let review = WorkspaceReview::read(repo.path()).expect("review reads");

    assert_eq!(
        *kind_of(&review, "src/committed.txt"),
        WorkspaceReviewKind::Deleted
    );
    assert!(entry(&review, "src/committed.txt").staged);
}

#[test]
fn keeps_paths_relative_to_the_workspace() {
    // The browser must never receive an absolute path: rule 1 of the frontends
    // plan forbids exposing the workspace root, and a relative path is also
    // what the file tree actually renders.
    let repo = TempRepo::new("relative");
    repo.write("deep/nested/file.txt", "content\n");

    let review = WorkspaceReview::read(repo.path()).expect("review reads");
    let path = &entry(&review, "deep/nested/file.txt").path;

    assert!(!path.starts_with('/'));
    assert!(!path.contains(repo.path().to_string_lossy().as_ref()));
}

#[test]
fn lists_the_paths_this_branch_added_over_its_upstream() {
    // "Branch" in the Review tab means the work committed on this branch, which
    // is a different set from the working copy's staged and unstaged changes.
    let origin = TempRepo::new("branch-origin");
    let clone = TempRepo::new("branch-clone");
    let clone_path = clone.path().join("work");
    let status = Command::new("git")
        .current_dir(clone.path())
        .args([
            "clone",
            "--quiet",
            origin.path().to_string_lossy().as_ref(),
            clone_path.to_string_lossy().as_ref(),
        ])
        .status()
        .expect("git clone runs");
    assert!(status.success(), "clone succeeds");
    let clone = TempRepo(clone_path);

    clone.write("src/on-branch.txt", "branch work\n");
    clone.git(&["add", "src/on-branch.txt"]);
    clone.git(&[
        "-c",
        "user.email=test@example.invalid",
        "-c",
        "user.name=Test",
        "commit",
        "--quiet",
        "-m",
        "branch work",
    ]);

    let review = WorkspaceReview::read(clone.path()).expect("review reads");

    assert!(
        review
            .branch_changes
            .iter()
            .any(|path| path == "src/on-branch.txt"),
        "the committed path is on the branch, got {:?}",
        review.branch_changes
    );
    assert!(
        review.changes.is_empty(),
        "the working copy is clean even though the branch has work"
    );
}

#[test]
fn reports_a_directory_that_is_not_a_repository_rather_than_failing() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("orchester-review-plain-{nonce}"));
    fs::create_dir_all(&path).expect("temporary directory");

    let error = WorkspaceReview::read(&path).expect_err("a plain directory is not a repository");

    assert!(matches!(error, WorkspaceReviewError::NotARepository));
    let _ = fs::remove_dir_all(&path);
}

#[test]
fn keeps_a_path_that_contains_a_space_in_one_piece() {
    // The NUL-separated status format exists so a path never needs quoting. A
    // space in a name is the common case that the line-oriented format would
    // have split into two entries.
    let repo = TempRepo::new("spaced");
    repo.write("src/a file with spaces.txt", "content\n");

    let review = WorkspaceReview::read(repo.path()).expect("review reads");

    assert_eq!(review.changes.len(), 1, "one path is one entry");
    let path = &entry(&review, "src/a file with spaces.txt").path;
    assert_eq!(path, "src/a file with spaces.txt");
}

#[test]
fn does_not_read_a_rename_source_as_a_separate_entry() {
    // git status -z spends a second field on the path a rename came from. If
    // the parser counted that field as an entry it would report a file that no
    // longer exists, so the count is what this pins.
    let repo = TempRepo::new("renamed");
    repo.write("src/before.txt", "content\n");
    repo.git(&["add", "src/before.txt"]);
    repo.git(&[
        "-c",
        "user.email=test@example.invalid",
        "-c",
        "user.name=Test",
        "commit",
        "--quiet",
        "-m",
        "add before",
    ]);
    repo.git(&["mv", "src/before.txt", "src/after.txt"]);

    let review = WorkspaceReview::read(repo.path()).expect("review reads");

    assert_eq!(review.changes.len(), 1, "got {:?}", review.changes);
    let renamed = entry(&review, "src/after.txt");
    assert!(renamed.staged, "the rename is staged work");
    // The design spec's decoration vocabulary is add / remove / modify, and the
    // path is still present, so a rename reads as a modification.
    assert_eq!(renamed.kind, WorkspaceReviewKind::Modified);
}

#[test]
fn reports_an_empty_branch_set_for_a_branch_with_no_upstream() {
    // A branch nobody published has nothing to compare against. That is not an
    // error and must not be reported as one.
    let repo = TempRepo::new("no-upstream");
    repo.write("src/local-only.txt", "local\n");

    let review = WorkspaceReview::read(repo.path()).expect("review reads");

    assert!(review.branch.is_some());
    assert!(
        review.branch_changes.is_empty(),
        "an unpublished branch has no upstream work, got {:?}",
        review.branch_changes
    );
}

#[test]
fn reads_a_workspace_that_is_a_subdirectory_of_the_repository() {
    // The runtime may be pointed at a subdirectory; the review still describes
    // the repository the workspace belongs to.
    let repo = TempRepo::new("subdir");
    repo.write("src/nested/file.txt", "content\n");

    let subdirectory = repo.path().join("src/nested");
    let review = WorkspaceReview::read(&subdirectory).expect("review reads");

    assert!(
        review
            .changes
            .iter()
            .any(|change| change.path == "src/nested/file.txt"),
        "the repository's own status is what is read, got {:?}",
        review.changes
    );
}
