//! Asking the release index whether a newer Orchester has been published.
//!
//! The launcher is distributed as an npm package, so the registry entry for that
//! package is the authority on what "latest" means — the same place an update
//! would come from. Nothing here writes anything: the check reports, and
//! installing is a separate, explicit step.
//!
//! Everything the registry says is treated as hostile. A version is only
//! accepted if it parses into [`Version`], and every rendered version is
//! reconstructed from those parsed numbers, so no byte of a network response is
//! ever printed to a terminal or spliced into a URL.

use std::cmp::Ordering;
use std::fmt;
use std::io::{self, Write};
use std::time::Duration;

use futures::StreamExt;
use reqwest::header::ACCEPT;
use reqwest::redirect::Policy;
use thiserror::Error;

/// What a human has to run to move to the published release.
pub const UPDATE_COMMAND: &str = "npm install -g @orchester/cli";

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

/// Report the outcome of a check as text.
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
    if let Some(notes) = status.notes_url() {
        writeln!(out, "Release notes: {notes}")?;
    }
    writeln!(out, "Run `{UPDATE_COMMAND}` to update.")
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
    fn being_behind_is_reported_with_the_two_versions_and_the_command() {
        let text = rendered("0.1.2", "0.2.0");

        assert!(text.contains("Update available! 0.1.2 -> 0.2.0"));
        assert!(text.contains("/releases/tag/v0.2.0"));
        assert!(text.contains(UPDATE_COMMAND));
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
}
