use std::fmt;

use super::{EvolutionDigest, EvolutionError};

#[derive(Clone, Default, PartialEq, Eq)]
pub struct EvaluationSnapshotInput {
    corpus: Option<EvolutionDigest>,
    evaluator: Option<EvolutionDigest>,
    config: Option<EvolutionDigest>,
    policy: Option<EvolutionDigest>,
    catalog: Option<EvolutionDigest>,
    environment: Option<EvolutionDigest>,
}

impl EvaluationSnapshotInput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_corpus(mut self, digest: EvolutionDigest) -> Self {
        self.corpus = Some(digest);
        self
    }

    pub fn with_evaluator(mut self, digest: EvolutionDigest) -> Self {
        self.evaluator = Some(digest);
        self
    }

    pub fn with_config(mut self, digest: EvolutionDigest) -> Self {
        self.config = Some(digest);
        self
    }

    pub fn with_policy(mut self, digest: EvolutionDigest) -> Self {
        self.policy = Some(digest);
        self
    }

    pub fn with_catalog(mut self, digest: EvolutionDigest) -> Self {
        self.catalog = Some(digest);
        self
    }

    pub fn with_environment(mut self, digest: EvolutionDigest) -> Self {
        self.environment = Some(digest);
        self
    }

    pub(super) fn require_complete(self) -> Result<EvaluationSnapshots, EvolutionError> {
        Ok(EvaluationSnapshots {
            corpus: self.corpus.ok_or(EvolutionError::MissingSnapshot)?,
            evaluator: self.evaluator.ok_or(EvolutionError::MissingSnapshot)?,
            config: self.config.ok_or(EvolutionError::MissingSnapshot)?,
            policy: self.policy.ok_or(EvolutionError::MissingSnapshot)?,
            catalog: self.catalog.ok_or(EvolutionError::MissingSnapshot)?,
            environment: self.environment.ok_or(EvolutionError::MissingSnapshot)?,
        })
    }
}

impl fmt::Debug for EvaluationSnapshotInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EvaluationSnapshotInput")
            .field("corpus_present", &self.corpus.is_some())
            .field("evaluator_present", &self.evaluator.is_some())
            .field("config_present", &self.config.is_some())
            .field("policy_present", &self.policy.is_some())
            .field("catalog_present", &self.catalog.is_some())
            .field("environment_present", &self.environment.is_some())
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct EvaluationSnapshots {
    corpus: EvolutionDigest,
    evaluator: EvolutionDigest,
    config: EvolutionDigest,
    policy: EvolutionDigest,
    catalog: EvolutionDigest,
    environment: EvolutionDigest,
}

impl EvaluationSnapshots {
    pub fn corpus(&self) -> &EvolutionDigest {
        &self.corpus
    }

    pub fn evaluator(&self) -> &EvolutionDigest {
        &self.evaluator
    }

    pub fn config(&self) -> &EvolutionDigest {
        &self.config
    }

    pub fn policy(&self) -> &EvolutionDigest {
        &self.policy
    }

    pub fn catalog(&self) -> &EvolutionDigest {
        &self.catalog
    }

    pub fn environment(&self) -> &EvolutionDigest {
        &self.environment
    }
}

impl fmt::Debug for EvaluationSnapshots {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EvaluationSnapshots")
            .field("corpus", &self.corpus)
            .field("evaluator", &self.evaluator)
            .field("config", &self.config)
            .field("policy", &self.policy)
            .field("catalog", &self.catalog)
            .field("environment", &self.environment)
            .finish()
    }
}
