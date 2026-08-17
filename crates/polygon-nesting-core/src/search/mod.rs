//! Search state, canonical keys, and scoring. TS counterparts:
//! `src/workers/algorithm/irregular/irregularBeamState.ts`,
//! `irregularPlacementScorer.ts`, `irregularLayoutScorer.ts`,
//! `irregularScoreGrid.ts`, and `src/workers/algorithm/sortPiecesForNesting.ts`.

pub mod beam_state;
pub mod gap_regions;
pub mod general_fast;
#[cfg(feature = "jagua-experimental")]
pub mod general_hazard;
pub mod general_micro_legalization;
pub mod general_relaxed;
pub mod kernel;
pub mod layout_scorer;
pub mod placement_scorer;
pub mod score_grid;
pub mod shadow_rescore;
pub mod sort_pieces;
pub mod strict_decoder;
pub mod strict_family;
