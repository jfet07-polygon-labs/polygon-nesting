use serde::{Deserialize, Serialize};

use crate::{LayoutScoreSummary, StateSnapshot};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SequencedEngineEvent {
    pub ordinal: u64,
    pub event: EngineEvent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum EngineEvent {
    PortfolioProgress {
        progress: PortfolioProgress,
    },
    StateSnapshot {
        snapshot: StateSnapshot,
        beam_width: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortfolioPhase {
    SharedArchive,
    ShortSideProfile,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortfolioProgress {
    pub phase: PortfolioPhase,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_score: Option<LayoutScoreSummary>,
    pub elapsed_ms: f64,
}
