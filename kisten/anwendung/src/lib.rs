//! The Orchester application service: everything a frontend needs, and nothing
//! a frontend is.
//!
//! Orchester has one runtime and, from here on, several frontends: the terminal
//! UI, a local WebUI, a desktop shell. They ask the same questions — which
//! agents exist, which model is active, what did this run do — and they must get
//! the same answers.
//!
//! Before this crate the answers lived in `konsole`, which is a `[[bin]]` with no
//! library target. Nothing outside it could call them, so a second frontend
//! would have had to reimplement configuration loading, model selection and
//! runtime caching, and the two copies would have drifted the first time a
//! provider field changed. So the UI-agnostic half moves here and the terminal
//! keeps only its rendering.
//!
//! The rule that keeps this honest: **this crate depends on no UI crate and
//! returns no formatted text.** Everything it hands back is data — a struct, an
//! enum, or a stream of [`orchester_protokoll::Event`] — and the frontend
//! decides what it looks like.

mod paths;

pub use paths::OrchesterPaths;
