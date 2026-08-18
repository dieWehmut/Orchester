//! Asking the release index whether a newer Orchester has been published, and
//! running the install when a human asks for it.
//!
//! The launcher is distributed as an npm package, so the registry entry for that
//! package is the authority on what "latest" means — the same place an update
//! would come from. Nothing installs on its own: the check reports, the offer
//! waits for an answer, and only an explicit choice runs npm.
//!
//! Everything the registry says is treated as hostile. A version is only
//! accepted if it parses into [`Version`], and every rendered version is
//! reconstructed from those parsed numbers, so no byte of a network response is
//! ever printed to a terminal or spliced into a URL or an argument.

use std::cmp::Ordering;
use std::ffi::OsString;
use std::fmt;
use std::io::{self, BufRead, Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use futures::StreamExt;
use reqwest::header::ACCEPT;
use reqwest::redirect::Policy;
use thiserror::Error;

use crate::interactive::clean_transcript_text;
use crate::process::{command_invocation, resolve_command};

/// The npm package the launcher is published as.
const PACKAGE: &str = "@orchester/cli";

/// The registry document for whatever `latest` currently points at.
const RELEASE_ENDPOINT: &str = "https://registry.npmjs.org/@orchester/cli/latest";

/// npm's abbreviated manifest. Asking for it keeps the readme out of the
/// response, which is most of the size of a full one.
const ABBREVIATED_MANIFEST: &str = "application/vnd.npm.install-v1+json";

/// Far above a manifest without a readme, far below anything worth buffering.
const RESPONSE_LIMIT: usize = 64 * 1024;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

const USER_AGENT: &str = concat!("orchester/", env!("CARGO_PKG_VERSION"));

/// Longer than any release of ours and short enough that a hostile answer
/// cannot become a wall of text on the way to being rejected.
const MAX_VERSION_BYTES: usize = 32;

/// A global install fetches a package tree over the network, so this is
/// generous — but bounded, so a wedged npm cannot hold the session forever.
const INSTALL_TIMEOUT: Duration = Duration::from_secs(300);

const POLL_INTERVAL: Duration = Duration::from_millis(25);

/// npm says little with `--no-progress`; past this it is repeating itself, and
/// the session should not grow a buffer because a mirror is chatty.
const OUTPUT_LIMIT: usize = 32 * 1024;

/// A published version, parsed into the parts a comparison needs.
///
/// Build metadata is accepted and discarded, which is what semver says it means
/// for ordering. A pre-release tag is kept, because it decides whether the same
/// numbers are older or newer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    major: u64,
    minor: u64,
    patch: u64,
    pre: Option<String>,
}

impl Version {
    /// Parse a strict `MAJOR.MINOR.PATCH[-PRE][+BUILD]`.
    ///
    /// Strictness is the security boundary: the accepted alphabet is digits,
    /// dots and hyphens, so a value that reaches a terminal or a URL cannot
    /// carry an escape sequence, a quote or a path segment.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if text.is_empty() || text.len() > MAX_VERSION_BYTES {
            return None;
        }
        // Build metadata is ignored for ordering, but it still has to be
        // well-formed: accepting anything after `+` would accept anything.
        let text = match text.split_once('+') {
            Some((head, build)) if is_tag(build) => head,
            Some(_) => return None,
            None => text,
        };
        let (core, pre) = match text.split_once('-') {
            Some((core, pre)) if is_tag(pre) => (core, Some(pre.to_owned())),
            Some(_) => return None,
            None => (text, None),
        };

        let mut parts = core.split('.');
        let major = number(parts.next()?)?;
        let minor = number(parts.next()?)?;
        let patch = number(parts.next()?)?;
        if parts.next().is_some() {
            return None;
        }
        Some(Self {
            major,
            minor,
            patch,
            pre,
        })
    }

    /// The version this binary was built as.
    ///
    /// Infallible by construction — the string comes from our own manifest — and
    /// `the_running_version_parses` holds the build to that.
    pub fn running() -> Self {
        Self::parse(env!("CARGO_PKG_VERSION")).expect("our own version parses")
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| match (&self.pre, &other.pre) {
                // A release outranks any pre-release of the same numbers, so
                // running 1.2.0 is never told that 1.2.0-rc.1 is newer.
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                // Compared as text rather than by semver's identifier rules.
                // That is wrong for `rc.9` against `rc.10`, and both are still
                // behind the release that follows them, which is the only
                // comparison this check acts on.
                (Some(ours), Some(theirs)) => ours.cmp(theirs),
            })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            major,
            minor,
            patch,
            pre,
        } = self;
        write!(formatter, "{major}.{minor}.{patch}")?;
        match pre {
            Some(pre) => write!(formatter, "-{pre}"),
            None => Ok(()),
        }
    }
}

fn number(text: &str) -> Option<u64> {
    // `str::parse` would accept `+1` and a leading zero run; a version segment
    // is decimal digits and nothing else.
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// Dot-separated identifiers, as a pre-release or build tag is written.
fn is_tag(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-')
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum UpdateCheckError {
    #[error("the release index could not be reached")]
    Unreachable,
    #[error("the release index answered {0}")]
    Status(u16),
    #[error("the release index did not name a version")]
    Unreadable,
}

/// What the running binary is, next to what has been published.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseStatus {
    pub running: Version,
    pub latest: Version,
}

impl ReleaseStatus {
    /// Strictly newer, so a build ahead of the registry — a local one — is never
    /// asked to downgrade itself.
    pub fn is_behind(&self) -> bool {
        self.latest > self.running
    }

    /// Where the published notes for `latest` live, when the repository this was
    /// built from is one that has such a page.
    pub fn notes_url(&self) -> Option<String> {
        release_notes_url(&self.latest)
    }
}

/// Ask the registry what `latest` is and say how the running binary compares.
pub async fn check() -> Result<ReleaseStatus, UpdateCheckError> {
    Ok(ReleaseStatus {
        running: Version::running(),
        latest: latest_release().await?,
    })
}

async fn latest_release() -> Result<Version, UpdateCheckError> {
    let client = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .user_agent(USER_AGENT)
        // Bounded rather than disabled: no credential travels with this request,
        // so a redirect leaks nothing, but a chain of them is a way to spend the
        // whole timeout.
        .redirect(Policy::limited(3))
        .build()
        .map_err(|_| UpdateCheckError::Unreachable)?;
    let response = client
        .get(RELEASE_ENDPOINT)
        .header(ACCEPT, ABBREVIATED_MANIFEST)
        .send()
        .await
        .map_err(|_| UpdateCheckError::Unreachable)?;
    let status = response.status();
    if !status.is_success() {
        return Err(UpdateCheckError::Status(status.as_u16()));
    }
    if response
        .content_length()
        .is_some_and(|length| length > RESPONSE_LIMIT as u64)
    {
        return Err(UpdateCheckError::Unreadable);
    }

    let mut stream = response.bytes_stream();
    let mut body = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| UpdateCheckError::Unreachable)?;
        if chunk.len() > RESPONSE_LIMIT.saturating_sub(body.len()) {
            return Err(UpdateCheckError::Unreadable);
        }
        body.extend_from_slice(&chunk);
    }
    parse_release(&body).ok_or(UpdateCheckError::Unreadable)
}

/// Pull the version out of a registry manifest.
///
/// Separated from the request so the shape the registry answers with can be
/// asserted without a network, which is where every field this trusts is
/// decided.
fn parse_release(body: &[u8]) -> Option<Version> {
    let document: serde_json::Value = serde_json::from_slice(body).ok()?;
    Version::parse(document.get("version")?.as_str()?)
}

/// GitHub publishes release notes under a `v`-prefixed tag, which is the tag our
/// own releases use. A repository hosted anywhere else has no page this can
/// name, and guessing one would print a link that 404s.
fn release_notes_url(version: &Version) -> Option<String> {
    let repository = env!("CARGO_PKG_REPOSITORY").trim_end_matches('/');
    repository
        .strip_prefix("https://github.com/")
        .filter(|path| path.split('/').filter(|part| !part.is_empty()).count() == 2)
        .map(|_| format!("{repository}/releases/tag/v{version}"))
}

/// The command that moves this installation to `version`, as a human would type
/// it.
///
/// The version is spliced from parsed numbers, so this can be printed and can be
/// handed to npm without either becoming an injection point. The flags the real
/// invocation adds are left out: they only suppress npm's noise, and a line a
/// human is meant to be able to retype should not carry them.
pub fn update_command(version: &Version) -> String {
    format!("npm install -g {PACKAGE}@{version}")
}

/// Report how the running binary compares to the published release.
///
/// Written into whatever the caller owns: the real terminal from a line-based
/// prompt, or a buffer that the full-screen chat turns into an overlay.
pub fn render_status(out: &mut impl Write, status: &ReleaseStatus) -> io::Result<()> {
    if !status.is_behind() {
        return writeln!(
            out,
            "Orchester {} is the published release.",
            status.running
        );
    }
    writeln!(
        out,
        "\u{2728} Update available! {} -> {}",
        status.running, status.latest
    )?;
    match status.notes_url() {
        Some(notes) => writeln!(out, "Release notes: {notes}"),
        None => Ok(()),
    }
}

/// What a human chose when told a newer release exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateChoice {
    Install,
    Skip,
}

/// Offer the update and read the answer.
///
/// Only the install option installs. Every other answer — a blank line, a typo,
/// or end of input from a piped session — skips, because replacing the binary a
/// human is running is not a thing to do on a guess.
pub fn prompt_update_choice(
    input: &mut impl BufRead,
    out: &mut impl Write,
    status: &ReleaseStatus,
) -> io::Result<UpdateChoice> {
    render_status(out, status)?;
    writeln!(out)?;
    writeln!(
        out,
        "  1. Update now (runs `{}`)",
        update_command(&status.latest)
    )?;
    writeln!(out, "  2. Skip")?;
    write!(out, "Choose [1-2]: ")?;
    out.flush()?;

    let mut answer = String::new();
    if input.read_line(&mut answer)? == 0 {
        writeln!(out)?;
        return Ok(UpdateChoice::Skip);
    }
    Ok(match answer.trim() {
        "1" => UpdateChoice::Install,
        _ => UpdateChoice::Skip,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum InstallError {
    #[error("npm is not on PATH")]
    NpmUnavailable,
    #[error("the npm launcher would have to go through a shell")]
    UnsafeNpmLauncher,
    #[error("npm could not be started")]
    NotStarted,
    #[error("npm did not finish in time")]
    TimedOut,
}

/// What npm did, as a human needs to see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallReport {
    pub succeeded: bool,
    pub output: String,
}

/// Install `version` globally through npm.
///
/// The exact version is pinned rather than `latest`, so what lands is what the
/// human was shown and agreed to, even if the registry moves in between.
pub fn install(version: &Version) -> Result<InstallReport, InstallError> {
    let executable = resolve_command("npm").ok_or(InstallError::NpmUnavailable)?;
    // `@orchester/cli` declares no install scripts — the platform binary arrives
    // as an optional dependency — so refusing to run any is free here, and it
    // keeps a compromised dependency in the tree from executing on this machine.
    let arguments = vec![
        OsString::from("install"),
        OsString::from("--global"),
        OsString::from("--ignore-scripts"),
        OsString::from("--no-audit"),
        OsString::from("--no-fund"),
        OsString::from("--no-progress"),
        // The spec is built from parsed numbers and cannot look like a flag, but
        // `--` means npm never has to be trusted to agree about that.
        OsString::from("--"),
        OsString::from(format!("{PACKAGE}@{version}")),
    ];
    let invocation = command_invocation(&executable, arguments);
    if invocation.uses_shell() {
        return Err(InstallError::UnsafeNpmLauncher);
    }

    let mut command = Command::new(&invocation.program);
    command
        .args(&invocation.args)
        // A global install can ask to confirm something; with no stdin it fails
        // and says so instead of waiting for a keystroke nobody will send.
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in &invocation.envs {
        command.env(key, value);
    }
    let mut child = command.spawn().map_err(|_| InstallError::NotStarted)?;
    // Each pipe is drained by a thread of its own: npm writes diagnostics to
    // stderr and results to stdout, and reading one to the end first would block
    // forever once the other filled its buffer.
    let stdout = child.stdout.take().map(drain_on_thread);
    let stderr = child.stderr.take().map(drain_on_thread);

    let deadline = Instant::now() + INSTALL_TIMEOUT;
    let status = loop {
        match child.try_wait().map_err(|_| InstallError::NotStarted)? {
            Some(status) => break status,
            None if Instant::now() < deadline => thread::sleep(POLL_INTERVAL),
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(InstallError::TimedOut);
            }
        }
    };

    let mut output = collected(stdout);
    output.push_str(&collected(stderr));
    Ok(InstallReport {
        succeeded: status.success(),
        // npm draws its own colours and cursor moves. This is the last point
        // before those bytes reach a terminal, so they stop here.
        output: clean_transcript_text(&output),
    })
}

/// Report what an install attempt did.
///
/// A failure is reported, not returned: the session survives it, and the human
/// is left holding the command they can run by hand.
pub fn render_install(
    out: &mut impl Write,
    version: &Version,
    result: &Result<InstallReport, InstallError>,
) -> io::Result<()> {
    let command = update_command(version);
    writeln!(out, "Updating Orchester via `{command}`...")?;
    match result {
        Ok(report) => {
            if !report.output.is_empty() {
                writeln!(out, "{}", report.output)?;
            }
            if report.succeeded {
                writeln!(
                    out,
                    "\u{1f389} Update ran successfully! Please restart Orchester."
                )
            } else {
                writeln!(out, "Update failed. Run `{command}` to see why.")
            }
        }
        Err(error) => writeln!(out, "Update failed: {error}. Run `{command}` to update."),
    }
}

fn drain_on_thread(source: impl Read + Send + 'static) -> thread::JoinHandle<String> {
    thread::spawn(move || drain(source))
}

fn collected(reader: Option<thread::JoinHandle<String>>) -> String {
    reader
        .and_then(|reader| reader.join().ok())
        .unwrap_or_default()
}

/// Read a pipe to its end, keeping the first [`OUTPUT_LIMIT`] bytes.
///
/// Reading past the limit rather than stopping at it matters: a reader that
/// walks away leaves npm blocked on a full pipe until the timeout kills it,
/// which would turn a chatty success into a reported failure.
fn drain(mut source: impl Read) -> String {
    let mut kept = Vec::new();
    let mut buffer = [0u8; 8 * 1024];
    let mut truncated = false;
    loop {
        let read = match source.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => read,
        };
        let room = OUTPUT_LIMIT.saturating_sub(kept.len());
        truncated |= read > room;
        kept.extend_from_slice(&buffer[..read.min(room)]);
    }
    let mut text = String::from_utf8_lossy(&kept).into_owned();
    if truncated {
        text.push_str("\n... npm printed more than is shown here.");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(text: &str) -> Version {
        Version::parse(text).expect("parses")
    }

    fn rendered(running: &str, latest: &str) -> String {
        let mut out = Vec::new();
        render_status(
            &mut out,
            &ReleaseStatus {
                running: version(running),
                latest: version(latest),
            },
        )
        .expect("render");
        String::from_utf8(out).expect("UTF-8")
    }

    /// Drive the offer from a scripted answer, as a piped session would.
    fn offered(answer: &str) -> (UpdateChoice, String) {
        let status = ReleaseStatus {
            running: version("0.1.2"),
            latest: version("0.2.0"),
        };
        let mut input = io::Cursor::new(answer.as_bytes());
        let mut out = Vec::new();
        let choice = prompt_update_choice(&mut input, &mut out, &status).expect("prompt");
        (choice, String::from_utf8(out).expect("UTF-8"))
    }

    fn installed(result: Result<InstallReport, InstallError>) -> String {
        let mut out = Vec::new();
        render_install(&mut out, &version("0.2.0"), &result).expect("render");
        String::from_utf8(out).expect("UTF-8")
    }

    /// `Version::running` unwraps, so the manifest version has to stay a shape
    /// this parses. A release that renamed itself would fail here, not at
    /// startup on a user's machine.
    #[test]
    fn the_running_version_parses() {
        assert_eq!(
            Version::running().to_string(),
            env!("CARGO_PKG_VERSION"),
            "the parsed version must round-trip, or comparisons are against something else"
        );
    }

    #[test]
    fn versions_order_by_number_and_not_by_text() {
        assert!(version("0.10.0") > version("0.9.9"));
        assert!(version("1.0.0") > version("0.147.0"));
        assert!(version("0.1.3") > version("0.1.2"));
        assert_eq!(version("1.2.3"), version("1.2.3"));
        // Build metadata is not part of the identity a comparison uses.
        assert_eq!(version("1.2.3+build.9"), version("1.2.3"));
    }

    #[test]
    fn a_release_outranks_its_own_pre_releases() {
        assert!(version("1.2.0") > version("1.2.0-rc.1"));
        assert!(version("1.2.0-rc.2") > version("1.2.0-rc.1"));
        assert!(version("1.2.1-rc.1") > version("1.2.0"));
    }

    /// Everything here arrives from the network, so the parser is the only thing
    /// standing between a registry answer and a terminal.
    #[test]
    fn a_version_that_could_carry_an_escape_or_a_path_is_refused() {
        for hostile in [
            "1.2.3\u{1b}[31m",
            "1.2.3/../../etc",
            "1.2.3; rm -rf /",
            "1.2.3 && npm i evil",
            "latest",
            "1.2",
            "1.2.3.4",
            "1.2.x",
            "-1.2.3",
            "+1.2.3",
            "1.2.3-",
            "1.2.3+",
            "",
            "   ",
        ] {
            assert!(
                Version::parse(hostile).is_none(),
                "must be refused: {hostile:?}"
            );
        }
        assert!(
            Version::parse(&format!("1.2.3-{}", "a".repeat(MAX_VERSION_BYTES))).is_none(),
            "an oversized tag must be refused before it is inspected"
        );
    }

    #[test]
    fn a_registry_manifest_is_read_for_its_version_alone() {
        let body = br#"{"name":"@orchester/cli","version":"0.2.0","dist":{}}"#;

        assert_eq!(parse_release(body), Some(version("0.2.0")));
        assert_eq!(parse_release(br#"{"name":"@orchester/cli"}"#), None);
        assert_eq!(parse_release(br#"{"version":"nightly"}"#), None);
        assert_eq!(parse_release(b"not json"), None);
    }

    #[test]
    fn release_notes_point_at_the_tag_of_the_published_version() {
        let notes = release_notes_url(&version("0.2.0")).expect("a GitHub repository");

        assert!(notes.starts_with(env!("CARGO_PKG_REPOSITORY")));
        assert!(notes.ends_with("/releases/tag/v0.2.0"));
    }

    #[test]
    fn being_behind_is_reported_with_the_two_versions_and_the_notes() {
        let text = rendered("0.1.2", "0.2.0");

        assert!(text.contains("Update available! 0.1.2 -> 0.2.0"));
        assert!(text.contains("/releases/tag/v0.2.0"));
    }

    /// A local build ahead of the registry is common while developing, and being
    /// told to downgrade would be wrong every time.
    #[test]
    fn being_current_or_ahead_offers_nothing() {
        for (running, latest) in [("0.2.0", "0.2.0"), ("0.3.0", "0.2.0")] {
            let text = rendered(running, latest);
            assert!(text.contains("is the published release"), "{text}");
            assert!(!text.contains("Update available"), "{text}");
        }
    }

    /// The command is printed for a human to retype and is also what npm is
    /// asked to do, so the version in it has to be the one that was offered.
    #[test]
    fn the_update_command_pins_the_published_version() {
        assert_eq!(
            update_command(&version("0.2.0")),
            "npm install -g @orchester/cli@0.2.0"
        );
    }

    #[test]
    fn the_offer_names_both_choices_and_the_command_it_would_run() {
        let (choice, text) = offered("1\n");

        assert_eq!(choice, UpdateChoice::Install);
        assert!(text.contains("Update available! 0.1.2 -> 0.2.0"), "{text}");
        assert!(
            text.contains("1. Update now (runs `npm install -g @orchester/cli@0.2.0`)"),
            "{text}"
        );
        assert!(text.contains("2. Skip"), "{text}");
    }

    /// Replacing the binary a human is running is not something to do on a
    /// guess, so only the install answer installs — a stray keystroke, an empty
    /// line, or a piped session that answers nothing all skip.
    #[test]
    fn anything_but_the_install_answer_skips() {
        for answer in ["2\n", "\n", "y\n", "1 2\n", "11\n", ""] {
            let (choice, _) = offered(answer);
            assert_eq!(choice, UpdateChoice::Skip, "must skip: {answer:?}");
        }
    }

    #[test]
    fn a_successful_install_shows_npm_output_and_asks_for_a_restart() {
        let text = installed(Ok(InstallReport {
            succeeded: true,
            output: "added 1 package".into(),
        }));

        assert!(text.contains("Updating Orchester via `npm install -g @orchester/cli@0.2.0`"));
        assert!(text.contains("added 1 package"), "{text}");
        assert!(text.contains("Please restart Orchester."), "{text}");
    }

    /// A failed install must leave the human with the command rather than a
    /// dead end, and must not claim a restart will help.
    #[test]
    fn a_failed_install_hands_back_the_command() {
        let failed = installed(Ok(InstallReport {
            succeeded: false,
            output: "npm error EACCES".into(),
        }));
        assert!(failed.contains("npm error EACCES"), "{failed}");
        assert!(
            failed.contains("Run `npm install -g @orchester/cli@0.2.0`"),
            "{failed}"
        );
        assert!(!failed.contains("successfully"), "{failed}");

        let missing = installed(Err(InstallError::NpmUnavailable));
        assert!(missing.contains("npm is not on PATH"), "{missing}");
        assert!(
            missing.contains("Run `npm install -g @orchester/cli@0.2.0`"),
            "{missing}"
        );
    }

    /// npm paints its own colours and moves the cursor. Those bytes are drained
    /// from a pipe, so the drain is the last place they can be stopped.
    #[test]
    fn npm_output_keeps_its_lines_and_loses_its_control_bytes() {
        let noisy = "\x1b[32madded\x1b[0m 1 package\r\nin 2s\x07\n";

        assert_eq!(
            clean_transcript_text(&drain(io::Cursor::new(noisy))),
            "added 1 package\nin 2s\\u{7}"
        );
    }

    /// A chatty mirror must not grow the session's memory, and the drain has to
    /// keep reading anyway or npm blocks on a full pipe until it is killed.
    #[test]
    fn oversized_npm_output_is_cut_and_says_so() {
        let flood = "x".repeat(OUTPUT_LIMIT * 2);

        let text = drain(io::Cursor::new(flood));

        assert!(text.starts_with(&"x".repeat(OUTPUT_LIMIT)));
        assert!(text.ends_with("... npm printed more than is shown here."));
        assert!(text.len() < OUTPUT_LIMIT * 2);
    }
}
