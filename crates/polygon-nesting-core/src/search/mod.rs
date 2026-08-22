//! Search state, canonical keys, and scoring. TS counterparts:
//! `src/workers/algorithm/irregular/irregularBeamState.ts`,
//! `irregularPlacementScorer.ts`, `irregularLayoutScorer.ts`,
//! `irregularScoreGrid.ts`, and `src/workers/algorithm/sortPiecesForNesting.ts`.

pub mod beam_state;
/// The relaxed lane's depth clock. Compiled only under
/// `compression-schedule`; with the feature off the module does not exist and
/// nothing in the lane names it.
#[cfg(feature = "compression-schedule")]
pub mod compression_schedule;
pub mod gap_regions;
pub mod general_fast;
#[cfg(feature = "jagua-experimental")]
pub mod general_hazard;
pub mod general_micro_legalization;
pub mod general_relaxed;
/// Gate A's shadow instrument: three verdicts on one imported pose set, and the
/// per-pair slacks behind them. Compiled only under `import-gate-shadow`;
/// nothing in `src/` outside it names this module, and it publishes nothing.
#[cfg(feature = "import-gate-shadow")]
pub mod import_gate;
pub mod kernel;
pub mod layout_scorer;
pub mod placement_scorer;
/// The anytime portfolio coordinator. Requires the deep operators, so it is
/// compiled only where they are.
#[cfg(feature = "jagua-experimental")]
pub mod portfolio;
pub mod score_grid;
pub mod shadow_rescore;
pub mod sort_pieces;
pub mod strict_decoder;
pub mod strict_family;
/// The parallel work currency: a price for every operator class in one unit.
/// Spec-keyed and off by default; nothing here is read by the shipped meter.
#[cfg(feature = "jagua-experimental")]
pub mod work_currency;
