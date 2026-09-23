//! The model a reader chose for the runs that follow.
//!
//! The runtime's own selection lives on a host as session state, and this server
//! builds one host per run - so a choice made in the browser would be gone
//! before the next run could read it. The choice is held here instead, applied
//! to each run's host before the run starts, and reported by the catalog route,
//! so the browser shows the model the next run will actually use rather than the
//! one the configuration file happens to name.

use std::sync::Arc;

use orchester_anwendung::{SelfAgentHost, SelfAgentHostError};
use orchester_laufzeit::harness::service::SelfAgentModelChoice;
use tokio::sync::Mutex;

/// How long a provider, profile or effort may be.
///
/// They are identifiers the configuration file already bounds, and the bound
/// here is the one that stops an unbounded string reaching a config lookup.
pub const MODEL_SELECTION_FIELD_MAX_CHARS: usize = 120;

/// A reader's choice, as the CLI's `/model` command expresses it.
///
/// Three optional pieces rather than one enum, because they are independent:
/// a provider can be switched while keeping the model, a profile names both, and
/// the effort is a session-only override on top of either.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModelSelection {
    pub provider: Option<String>,
    pub profile: Option<String>,
    /// `None` means the provider's own default, which is not the same as
    /// leaving whatever override was in force.
    pub effort: Option<String>,
}

impl ModelSelection {
    /// Whether this says anything at all: no provider, no profile, no effort is
    /// the configuration's own model, and applying it would only reload it.
    pub fn is_empty(&self) -> bool {
        self.provider.is_none() && self.profile.is_none() && self.effort.is_none()
    }

    /// Apply the choice to a host, exactly as the CLI's command does it.
    ///
    /// One place for the mapping, so a run and the catalog that describes it
    /// cannot answer differently about what is selected.
    pub fn apply(
        &self,
        host: &mut SelfAgentHost,
    ) -> Result<SelfAgentModelChoice, SelfAgentHostError> {
        match (&self.provider, &self.profile) {
            // A provider outranks a profile: it keeps the model and replaces any
            // named profile the session had selected.
            (Some(provider), _) => {
                host.select_model_provider_with_effort(provider, self.effort.as_deref())
            }
            (None, Some(profile)) => {
                host.select_model_profile_with_effort(profile, self.effort.as_deref())
            }
            (None, None) => host.select_configured_model_with_effort(self.effort.as_deref()),
        }
    }
}

/// The selection the server holds between requests.
///
/// Shared through the cloned `ServerContext`, so every route and every run sees
/// the same choice.
#[derive(Debug, Clone, Default)]
pub struct ModelSelectionStore {
    inner: Arc<Mutex<ModelSelection>>,
}

impl ModelSelectionStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn read(&self) -> ModelSelection {
        self.inner.lock().await.clone()
    }

    pub async fn write(&self, selection: ModelSelection) {
        *self.inner.lock().await = selection;
    }
}