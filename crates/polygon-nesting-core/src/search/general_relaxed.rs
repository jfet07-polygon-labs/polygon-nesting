//! Bounded overlap-relaxation search for the opt-in general engine.
//!
//! The constructor remains the protected anytime incumbent. This module
//! searches complete, temporarily infeasible layouts with a cheap convex-cell
//! surrogate and can replace that incumbent only after both publication gates
//! in `general_fast` accept a strict improvement.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::canonical_grid::{from_grid, to_grid_mm};
use crate::clipper::core::PointInPolygonResult;
use crate::domain::{IrregularBounds, IrregularPoint};
use crate::geometry::convex::bounds_for_points;
use crate::geometry::general_polygon::{GeneralPolygonError, PolygonSet};
use crate::geometry::predicates::orientation;
use crate::nfp_ifp::compute_relative_nfp_boundary_reference;
use crate::parallel::map_slice_with_job_pool;
use crate::profiling::{self, Counter, Phase};
use crate::quality_trace;
use crate::search::general_fast::{
    collision_expansion_mm, collision_sheet_inset_mm, collision_sheet_short_axis_mm,
    polygons_overlap_exact, validate_and_measure_placements, GeneralFastError, GeneralFastPiece,
    GeneralFastPlacement, GeneralFastResult, GeneralFastSettings, GeneralPlacementMetrics,
};
use crate::search::general_micro_legalization::{
    GeneralGlobalLegalizationDiagnostics, GeneralMicroLegalizationDiagnostics,
};
use crate::search::kernel::{
    ExplorationKernel, KernelPose, KernelProbes, LegacyKernel, PairRow, PosedShape, LEGACY,
};
#[cfg(feature = "compression-schedule")]
use crate::search::compression_schedule::{
    CompressionRepairPolicy, CompressionSchedule, CompressionScheduleSettings,
    GeneralCompressionScheduleDiagnostics, GeneralCompressionScheduleStepRow,
};
#[cfg(feature = "shadow-rescore")]
use crate::search::shadow_rescore;
// The added contract-validity and raw-depth reporting is reachable only through
// the persistent-vacancy arms, which the experimental feature gates.
#[cfg(feature = "jagua-experimental")]
use crate::search::general_fast::validate_placements_against_contract;
#[cfg(feature = "jagua-experimental")]
use crate::validation::general_polygon::{raw_source_long_axis_depth_mm, GeneralPlacement};
// Re-exported into the persistent-vacancy module by its `use super::*`, which
// is where the conflict-targeted re-placement repair consumes them.
#[cfg(feature = "jagua-experimental")]
use crate::search::general_micro_legalization::{
    global_legalize, micro_legalization_component_limit, micro_legalize,
    replacement_ejection_limit, separating_translation, survey_layout_violations, LayoutViolations,
};

#[cfg(feature = "jagua-experimental")]
#[path = "general_persistent_vacancy.rs"]
mod persistent_vacancy;

#[cfg(feature = "jagua-experimental")]
use crate::search::general_hazard::{
    GeneralHazardPose, GeneralHazardQuery, JaguaHazardCatalog, JaguaHazardIndex,
};
const ANGLE_KEY_SCALE: f64 = 1_000_000.0;
const SURROGATE_ANGLE_STEP_DEG: f64 = 2.5;
const MAX_CELLS_PER_PIECE: usize = 512;
const MAX_CELLS_PER_JOB: usize = 524_288;
const CELL_INDEX_SIDE: usize = 8;
const PIECE_INDEX_SIDE: usize = 16;
const LOCAL_DESCENT_STARTS: usize = 3;
const UNIQUE_SAMPLE_POSITION_RATIO: f64 = 0.05;
const UNIQUE_SAMPLE_ANGLE_DEG: f64 = 1.0;
const OVERLAP_PROXY_EPSILON_DIAMETER_RATIO: f64 = 0.01;
const EJECTION_CHAIN_MAX_DONORS: usize = 4;
const EJECTION_CHAIN_DIVERSITY: usize = 3;
const ENABLE_EJECTION_CHAIN: bool = false;
const PRE_REFINEMENT_INITIAL_RATIO: f64 = 0.25;
const PRE_REFINEMENT_LIMIT_RATIO: f64 = 0.02;
const FINAL_REFINEMENT_INITIAL_RATIO: f64 = 0.01;
const FINAL_REFINEMENT_LIMIT_RATIO: f64 = 0.001;
const REFINEMENT_SUCCESS_MULTIPLIER: f64 = 1.1;
const REFINEMENT_FAILURE_MULTIPLIER: f64 = 0.5;
const MAX_NFP_COMPONENTS_PER_MOVE: usize = 4_096;
const MAX_AXIS_EVENTS_PER_MOVE: usize = 16_384;
const MAX_LANE_NFP_COMPONENTS: usize = 65_536;
const MAX_SHARED_NFP_COMPONENTS: usize = MAX_LANE_NFP_COMPONENTS;
const MAX_SHARED_NFP_ESTIMATED_BYTES: usize = 32 * 1024 * 1024;
const MAX_TRIANGLE_NFP_POINTS: usize = 6;
const AXIS_MINIMIZATION_PASSES: usize = 4;
const AXIS_RETAINED_CANDIDATES: usize = 4;
const ENABLE_NFP_AXIS_MINIMIZER: bool = false;
/// How many spare candidate-row buffers one lane keeps under
/// `relaxed-row-buffer-reuse`.
///
/// Two is what the refinement loop actually retires per iteration — the loser
/// of the paired probe and the incumbent it displaced — so a deeper pool would
/// only hold memory the loop never asks for again. Four leaves headroom for the
/// sample loops without becoming a cache.
#[cfg(feature = "relaxed-row-buffer-reuse")]
const ROW_BUFFER_POOL_CAPACITY: usize = 4;
const DIRECTIONAL_LANE_UNSCORABLE: &str = "directional penetration lane is unscorable";
const COUPLED_SEPARATOR_SEED_DOMAIN: u64 = 0x4350_4C44_5350_5231;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_SEED_DOMAIN: u64 = 0x4352_5549_4E5F_5231;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_ANGLE_SEED_DOMAIN: u64 = 0x4352_5549_4E5F_4131;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_POSITION_SEED_DOMAIN: u64 = 0x4352_5549_4E5F_5031;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_DIVERSITY_SEED_DOMAIN: u64 = 0x4352_5549_4E5F_4431;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_RETRY_SEED_DOMAIN: u64 = 0x4352_5549_4E5F_5331;
#[cfg(feature = "jagua-experimental")]
const PRECOMPRESSION_FRONTIER_SEED_DOMAIN: u64 = 0x5052_4543_4F4D_5031;
#[cfg(feature = "jagua-experimental")]
const PRECOMPRESSION_HANDOFF_SEED_DOMAIN: u64 = 0x5052_4548_414E_4431;
// Mode 22 (alternation fixpoint) and mode 23 (recombination) promote two
// mechanisms that were previously driven from outside the engine by an
// external process alternating/crossing CLI invocations. Both are plain
// deterministic orchestration over the existing separator (mode 0) and
// descent (mode 11) machinery; neither introduces new search primitives.
#[cfg(feature = "jagua-experimental")]
const ALTERNATION_SEED_DOMAIN: u64 = 0x414C_5445_524E_3232;
#[cfg(feature = "jagua-experimental")]
const RECOMBINATION_SEED_DOMAIN: u64 = 0x5245_434F_4D42_3233;
// The alternation loop runs at most this many separator/descent cycles
// before it is declared non-convergent; a joint fixpoint (neither arm
// improves) may stop it earlier.
#[cfg(feature = "jagua-experimental")]
pub(super) const ALTERNATION_MAX_CYCLES: usize = 6;
// Each descent-arm target steps the current best by the same rung already
// used elsewhere in this experiment family for a bounded escape hop
// (`persistent_vacancy::CONSTRUCTION_DROP_LADDER_MM[1]`), rather than
// introducing a new tuned literal.
#[cfg(feature = "jagua-experimental")]
const ALTERNATION_DESCENT_TARGET_STEP_MM: f64 = persistent_vacancy::CONSTRUCTION_DROP_LADDER_MM[1];
// Mode 26 (clamped-sheet ladder compression) removes the depth-ward room the
// separator otherwise relaxes into. It is the same plain deterministic
// orchestration over the mode-0 pipeline that mode 22 is; the only new idea is
// that every step hands that pipeline a *shorter sheet* instead of a lower
// objective.
#[cfg(feature = "jagua-experimental")]
const LADDER_COMPRESSION_SEED_DOMAIN: u64 = 0x4C41_4444_4552_3236;
// Bounds between the parent depth and the requested final bound are walked in
// at most this many steps, each warm-started from the previous step's state.
#[cfg(feature = "jagua-experimental")]
const LADDER_COMPRESSION_STEPS: usize = 8;
// How many times a single rung arm may be re-run before the rung gives up.
// `run_coupled_separator_arm` breaks on its first failed target, so one
// unlucky draw - a target that aborts on a rollback rescore disagreement, or a
// terminal state whose residue is too coarse to project - otherwise costs the
// whole rung. Attempts are salted into the seed derivation, so each retry is a
// genuinely different deterministic draw rather than a repeat, and an attempt
// that publishes ends the loop immediately, so the extra work is only spent on
// rungs that would have failed outright.
#[cfg(feature = "jagua-experimental")]
const LADDER_COMPRESSION_RUNG_ATTEMPTS: usize = 3;
// Seed slot for the attempt salt, kept clear of the target/worker slots
// `run_coupled_separator_arm` derives from the same seed.
#[cfg(feature = "jagua-experimental")]
const LADDER_COMPRESSION_ATTEMPT_SLOT: usize = usize::MAX - 96;
// Mode 27 (micro-legalization probe) runs the repair pass directly on its
// parent fixture and reports what it found, with no ladder and no bound. It is
// the standalone instrument for the same pass mode 26 uses per rung.
#[cfg(feature = "jagua-experimental")]
const MICRO_LEGALIZATION_SEED_DOMAIN: u64 = 0x4D49_4352_4F4C_3237;
// Modes 30 and 31 are the standalone global pressure-balanced legalization
// probes: 30 measures and solves the parent under its own sheet, 31 under an
// explicit depth bound, exactly the way mode 27 and modes 28/29 pair up.
#[cfg(feature = "jagua-experimental")]
const GLOBAL_LEGALIZATION_SEED_DOMAIN: u64 = 0x474C_4F42_414C_3330;
// Mode 34 is the compression schedule: the same clamp mode 26 buys by
// rebuilding a whole pipeline per rung, bought instead one canonical grid unit
// at a time inside a single lane's sweeps.
#[cfg(all(feature = "jagua-experimental", feature = "compression-schedule"))]
const COMPRESSION_SCHEDULE_SEED_DOMAIN: u64 = 0x434F_4D50_5343_4834;
// The four repair tiers a mode-26 rung may publish through, reported per rung
// so a ladder table can say which mechanism reached which residue. Tier one is
// mode 27's translation-only projection; tier two is mode 28's
// conflict-targeted re-placement, attempted only when tier one produced
// nothing; tier three is mode 29's joint multi-piece re-placement, attempted
// only when tier two produced nothing either; tier four is mode 31's global
// pressure-balanced legalization, attempted only when all three local tiers
// produced nothing. The tiers run in strictly increasing order of the
// correction they can express, so a later tier can only ever *add* a
// publication to a rung an earlier one already failed.
#[cfg(feature = "jagua-experimental")]
const LADDER_REPAIR_TIER_MICRO: &str = "microLegalization";
#[cfg(feature = "jagua-experimental")]
const LADDER_REPAIR_TIER_REPLACEMENT: &str = "replacement";
#[cfg(feature = "jagua-experimental")]
const LADDER_REPAIR_TIER_JOINT: &str = "jointReplacement";
#[cfg(feature = "jagua-experimental")]
const LADDER_REPAIR_TIER_GLOBAL: &str = "globalLegalization";
// Mode 24 (bounded-depth reinsertion) tests compression by ejection and
// reconstruction rather than compression by overlap: it ejects exactly the
// pieces that stick out past a hard bound and rebuilds them with the
// construction insertion machinery under a sheet clamped to that bound, so a
// pose that would exceed the bound is never confirmed in the first place.
// The mechanism lives in the persistent-vacancy module beside the
// construction primitives it reuses; only the mode dispatch is here.
#[cfg(feature = "jagua-experimental")]
const COUPLED_SEPARATOR_TARGETS: usize = 32;
const COUPLED_SEPARATOR_WORKERS: usize = 8;
const COUPLED_SEPARATOR_ROUNDS: usize = 40;
#[cfg(feature = "jagua-experimental")]
const COUPLED_SEPARATOR_NO_IMPROVEMENT_LIMIT: usize = 10;
#[cfg(feature = "jagua-experimental")]
const COUPLED_SEPARATOR_STRIKE_LIMIT: usize = 3;
/// The separator's own relative contraction quantum, and therefore the smallest
/// rung [`ladder_compression_bounds`] will ever walk: a ladder rung is never
/// finer than this fraction of the parent's own depth.
///
/// Public because the coordinator derives a ladder's *rung count* from it - a
/// two-rung ladder is a drop of `2 * depth * ratio`, which is a length the
/// request supplies rather than a millimetre the schedule carries.
#[cfg(feature = "jagua-experimental")]
pub const COUPLED_SEPARATOR_CONTRACTION_RATIO: f64 = 0.001;
#[cfg(feature = "jagua-experimental")]
const COUPLED_SEPARATOR_SUBSTANTIAL_RATIO: f64 = 0.98;
/// How far two readings of one pole pressure may sit apart, in `f32` units in
/// the last place, before a
/// [`CoupledRollbackComparison::ToleratesPoleRounding`] arm calls them
/// different measurements rather than one measurement rounded twice.
///
/// The pressure is an `f32` pole-pair series; reversing the summation order
/// perturbs the last bits of the accumulator and of the pole coordinates
/// feeding it, so the gap is a handful of ulps rather than exactly one, and it
/// grows with the number of pole pairs summed.
///
/// The value is bracketed rather than guessed. Below it: the widest gap
/// measured over ten mixed-61 mode-26 ladders was 7 ulps. Above it, by a wide
/// margin: the only agreement the engine actually promises between the two
/// readings is the `1e-3` *relative* bound pinned by
/// `collision_pressure_is_direction_dependent_in_its_low_bits`, which is some
/// 8000 ulps. Sitting at 64 leaves an order of magnitude over what the search
/// produces while staying two orders inside what the geometry guarantees, so a
/// genuine bookkeeping drift - which is a relative-`O(1)` disagreement, not a
/// rounding one - is still refused.
///
/// It is measured, not assumed: every arm reports the widest gap it observed
/// next to the count it tolerated, so a run that needs more than this says so
/// instead of silently succeeding.
///
/// The budget applies **only** to magnitudes that are pole pressures and are
/// exactly `f32`-valued - see [`RollbackMagnitude`]. Boundary penalties and the
/// running sums are `f64` all the way down and keep the one-`f64`-ulp rule;
/// spending an `f32`-denominated budget on them would have admitted gaps some
/// ten orders of magnitude wider than the rounding this constant describes.
#[cfg(feature = "jagua-experimental")]
const COUPLED_ROLLBACK_PRESSURE_ULP_BUDGET: u32 = 64;
/// The prefix a contraction target's failure reason carries when a rollback
/// comparison refused the rescore. Shared so the ladder can recognise the abort
/// it is measuring instead of matching a phrase by hand.
#[cfg(feature = "jagua-experimental")]
const ROLLBACK_DISAGREEMENT_ABORT: &str = "rollback tracker disagrees with a complete rescore";
#[cfg(feature = "jagua-experimental")]
const COUPLED_SEPARATOR_WORKER_QUERY_CAP: usize = 420_000;
#[cfg(feature = "jagua-experimental")]
const COUPLED_SEPARATOR_WORKER_PRESSURE_CAP: usize = 4_000_000;
#[cfg(feature = "jagua-experimental")]
const COUPLED_SEPARATOR_WORKER_CONFIRMATION_CAP: usize = 2_440;
#[cfg(feature = "jagua-experimental")]
const COUPLED_SEPARATOR_WORKER_UPDATE_CAP: usize = 2_440;
#[cfg(feature = "jagua-experimental")]
const COUPLED_SEPARATOR_WORKER_LAYOUT_LOAD_CAP: usize = 40;
#[cfg(feature = "jagua-experimental")]
const COUPLED_SEPARATOR_WORKER_FULL_SCORE_PAIR_VISIT_CAP: usize = 73_200;
#[cfg(feature = "jagua-experimental")]
const COUPLED_SEPARATOR_AUDITOR_FULL_SCORES: usize = 5;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_REMOVED_PIECES: usize = 3;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_BEAM_WIDTH: usize = 4;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_ORIENTATIONS_PER_PARENT: usize = 12;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_POSES_PER_STREAM: usize = 64;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_FINALISTS_PER_STREAM: usize = 4;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_STREAM_CAP: usize = 108;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_QUERY_CAP: usize = 6_912;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_FINALIST_CAP: usize = 432;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_TRANSFORMED_VERTEX_CAP: usize = 262_144;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_FEATURE_VISIT_CAP: usize = 131_072;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_CONTACT_ATTEMPT_CAP: usize = 131_072;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_PROPOSAL_CAP: usize = 32_768;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_CLIPPER_INPUT_VERTEX_CAP: usize = 8_000_000;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_CLIPPER_OUTPUT_VERTEX_CAP: usize = 2_000_000;
#[cfg(feature = "jagua-experimental")]
const PRECOMPRESSION_FRONTIER_FULL_SCORE_CAP: usize = 9;
#[cfg(feature = "jagua-experimental")]
const PRECOMPRESSION_FRONTIER_PAIR_VISIT_CAP: usize = 16_470;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneralRelaxedCollisionBackend {
    RollbackTriangle,
    DynamicHazard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneralRelaxedAngleSeedPolicy {
    CurrentOnly,
    StructuredGrid,
    ContinuousUniform,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneralRelaxedPressureModel {
    StructuredTrianglePoles,
    DirectionalPenetration,
    ContinuousTrianglePoles,
    DynamicPoles,
}

#[derive(Clone, Copy, Debug)]
pub struct GeneralAngularRepairSettings {
    pub neighborhood_size: usize,
    pub successors: usize,
    pub complete_query_budget: usize,
    pub retained_confirmation_budget: usize,
    pub early_stop_queries: usize,
}

impl GeneralAngularRepairSettings {
    pub const fn disabled() -> Self {
        Self {
            neighborhood_size: 0,
            successors: 0,
            complete_query_budget: 0,
            retained_confirmation_budget: 0,
            early_stop_queries: 0,
        }
    }

    pub const fn bounded_probe() -> Self {
        Self {
            neighborhood_size: 10,
            successors: 1,
            complete_query_budget: 2_048,
            retained_confirmation_budget: 64,
            early_stop_queries: 512,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GeneralRelaxedSettings {
    pub seed: u64,
    pub epochs: usize,
    pub lanes: usize,
    pub sweeps_per_epoch: usize,
    pub global_samples_per_move: usize,
    pub focused_samples_per_move: usize,
    pub refinement_rounds: usize,
    pub initial_shrink_ratio: f64,
    pub minimum_shrink_ratio: f64,
    pub synchronize_lanes: bool,
    pub collision_backend: GeneralRelaxedCollisionBackend,
    pub angle_seed_policy: GeneralRelaxedAngleSeedPolicy,
    pub pressure_model: GeneralRelaxedPressureModel,
    pub angular_repair: GeneralAngularRepairSettings,
    pub coupled_dynamic_separator: bool,
    pub precompression_frontier_vacancy_mode: usize,
    pub persistent_vacancy_mode: usize,
    pub persistent_vacancy_target_depth_mm: Option<f64>,
    /// Lets the pinned-parent band (modes 9-21 and 25) descend from the
    /// coupled arm this same process produced, instead of requiring a fixture.
    ///
    /// **Off by default, and it must stay off in anything that publishes a
    /// comparable number.** The pin is what makes those modes' reported
    /// numbers reproducible: a fixture carries a frozen fingerprint and depth,
    /// and the arm re-derives both on load. An in-process parent carries
    /// neither, so an arm run this way is a *measurement of a search*, not a
    /// replay of a state, and its result may not be quoted against a pinned
    /// one.
    ///
    /// It exists because the review's ten-second portfolio allocates a slice
    /// to "one or two fast mode-20-derived basin constructors" from request
    /// only, and that path is otherwise closed: `run_population` refuses an
    /// unpinned parent before it does any work, so no single process can
    /// currently measure what a from-request mode-20 basin costs or is worth.
    /// Measuring that is the whole point of the quality frontier trace.
    pub persistent_vacancy_allow_unpinned_parent: bool,
    /// Restricts modes 20/25's construction sweep to a window of its insertion
    /// orders: `Some((first, count))` runs restart indices
    /// `first .. first + count`, wrapped into the constructor's own restart
    /// count, instead of all of them.
    ///
    /// `None` - the default, and the only value any CLI invocation produces -
    /// runs the full sweep, so the constructor is unchanged.
    ///
    /// This exists for the portfolio coordinator, which needs *one* basin per
    /// slot rather than the best of eight: mode 20 returns the best of its
    /// whole sweep, and a coordinator that wants several structurally distinct
    /// tickets has to draw them one at a time. It is a **budget** knob, not a
    /// quality knob - the ledger's cell-size sweep is the standing warning
    /// against treating any constructor parameter as tunable.
    pub construction_restart_window: Option<(usize, usize)>,
    /// Overrides the number of void-grid cells the mode-20/25 constructor's
    /// trapped-void evaluator derives per narrowest-piece extent.
    ///
    /// `None` - the default - keeps the calibrated divisor. Only the
    /// `fast-constructor-profile` evaluator reads it; the legacy raster has no
    /// derived cell to override and ignores it.
    ///
    /// This is the coordinator's basin *lottery ticket*, and the word is
    /// deliberate. The ledger measured eighteen cell sizes over one stream:
    /// eighteen distinct endpoints, twelve on a 179-181 mm plateau and six in a
    /// 169.5-174.3 mm basin, with Pearson(immediate, descended) = -0.212 and no
    /// contiguous region of good values. So a coordinator salts this across
    /// basin slots to buy variance; it must never *tune* it, and no value here
    /// may be presented as better than another.
    pub construction_void_cell_divisor: Option<f64>,
    /// Caps mode 22's alternation cycles, so a coordinator can spend a
    /// *quantum* of alternation on each of several archived basins instead of
    /// running one basin to its fixpoint.
    ///
    /// `None` - the default - runs the mode's own cycle bound. A value larger
    /// than that bound is clamped to it, so this can only ever shorten a run.
    pub alternation_max_cycles: Option<usize>,
    /// Arms the lane-owned compression schedule.
    ///
    /// `None` - the default, and what every existing caller constructs -
    /// leaves `move_sweep` reading `state.strip_depth_mm` exactly as it does
    /// today. `Some` makes the lane advance a depth clock at every sweep entry
    /// instead, which is a trajectory change and must be gated on quality.
    ///
    /// Compiled only under `compression-schedule`: with the feature off the
    /// field does not exist, so no caller can name it and no build carries a
    /// branch on it. See [`crate::search::compression_schedule`].
    #[cfg(feature = "compression-schedule")]
    pub compression_schedule: Option<crate::search::compression_schedule::CompressionScheduleSettings>,
}

impl GeneralRelaxedSettings {
    pub fn mixed_61_probe(seed: u64, lanes: usize) -> Self {
        Self {
            seed,
            epochs: 12,
            lanes: lanes.max(1),
            sweeps_per_epoch: 12,
            global_samples_per_move: 36,
            focused_samples_per_move: 36,
            refinement_rounds: 3,
            initial_shrink_ratio: 0.02,
            minimum_shrink_ratio: 0.001,
            synchronize_lanes: false,
            collision_backend: GeneralRelaxedCollisionBackend::RollbackTriangle,
            angle_seed_policy: GeneralRelaxedAngleSeedPolicy::StructuredGrid,
            pressure_model: GeneralRelaxedPressureModel::StructuredTrianglePoles,
            angular_repair: GeneralAngularRepairSettings::disabled(),
            coupled_dynamic_separator: false,
            precompression_frontier_vacancy_mode: 0,
            persistent_vacancy_mode: 0,
            persistent_vacancy_target_depth_mm: None,
            persistent_vacancy_allow_unpinned_parent: false,
            construction_restart_window: None,
            construction_void_cell_divisor: None,
            alternation_max_cycles: None,
            #[cfg(feature = "compression-schedule")]
            compression_schedule: None,
        }
    }

    pub fn mixed_61_dynamic_hazard_probe(seed: u64, lanes: usize) -> Self {
        Self {
            collision_backend: GeneralRelaxedCollisionBackend::DynamicHazard,
            angular_repair: GeneralAngularRepairSettings::disabled(),
            ..Self::mixed_61_probe(seed, lanes)
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralRelaxedDiagnostics {
    pub epochs_attempted: usize,
    pub epochs_improved: usize,
    pub oriented_surrogate_builds: usize,
    pub generated_cells: usize,
    pub ejection_chain_evaluations: usize,
    pub ejection_chain_accepts: usize,
    pub surrogate_evaluations: usize,
    pub piece_broad_phase_probes: usize,
    pub cell_index_probes: usize,
    pub sat_tests: usize,
    pub pair_nfp_builds: usize,
    pub pair_nfp_components: usize,
    pub shared_pair_nfp_entries: usize,
    pub shared_pair_nfp_components: usize,
    pub shared_pair_nfp_estimated_bytes: usize,
    pub shared_pair_nfp_adoptions: usize,
    pub directional_pair_evaluations: usize,
    pub directional_exact_confirmations: usize,
    pub directional_cache_hits: usize,
    pub directional_cache_misses: usize,
    pub directional_component_visits: usize,
    pub directional_intervals_produced: usize,
    pub directional_intervals_merged: usize,
    pub directional_over_budget_candidates: usize,
    pub directional_zero_penetration_inconsistencies: usize,
    pub directional_lane_rejections: usize,
    pub directional_relocations: usize,
    pub directional_rejected_contractions: usize,
    pub directional_containment_rejections: usize,
    pub directional_initial_pair_loss: GeneralRelaxedLossDistribution,
    pub directional_initial_boundary_loss: GeneralRelaxedLossDistribution,
    pub directional_accepted_pair_loss: GeneralRelaxedLossDistribution,
    pub directional_accepted_boundary_loss: GeneralRelaxedLossDistribution,
    pub axis_events: usize,
    pub axis_candidate_evaluations: usize,
    pub dynamic_hazard_queries: usize,
    pub dynamic_hazard_updates: usize,
    pub dynamic_pressure_evaluations: usize,
    pub translation_evaluations: usize,
    pub rotation_evaluations: usize,
    pub retained_f64_confirmations: usize,
    pub confirmed_pair_additions: usize,
    pub confirmed_pair_removals: usize,
    pub accepted_moves: usize,
    pub angular_repair_successors: usize,
    pub angular_repair_improvements: usize,
    pub angular_repair_queries: usize,
    pub angular_repair_base_loss: Option<f64>,
    pub angular_repair_control_loss: Option<f64>,
    pub angular_repair_rotation_loss: Option<f64>,
    pub surrogate_feasible_states: usize,
    pub exact_rejected_states: usize,
    pub exact_valid_non_improvements: usize,
    pub exact_rejection_reasons: Vec<String>,
    pub skipped_reason: Option<String>,
    pub epochs: Vec<GeneralRelaxedEpochDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coupled_dynamic_separator: Option<GeneralCoupledSeparatorDiagnostics>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralCoupledSeparatorDiagnostics {
    pub seed_domain: u64,
    pub control: GeneralCoupledSeparatorArmDiagnostics,
    pub treatment: GeneralCoupledSeparatorArmDiagnostics,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boundary_projection_treatment: Option<GeneralCoupledSeparatorArmDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict_ruin_recreate: Option<GeneralConflictRuinDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub precompression_frontier_vacancy: Option<GeneralPrecompressionFrontierVacancyDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persistent_vacancy_population: Option<GeneralPersistentVacancyDiagnostics>,
}

/// A pinned persistent-vacancy parent layout loaded from a committed fixture.
///
/// The frozen `b9335a72...` parent is a fingerprint of the boundary-projection
/// trajectory on the canonical Apple M4 Max platform. Arbitrary-angle
/// trigonometry is not promised byte-identical across numeric platforms, so a
/// different machine cannot reproduce that parent in-run. The fixture supplies
/// the identical placements explicitly; every frozen fingerprint, depth, and
/// dual-validation check still runs against the compiled-in constants.
#[derive(Clone, Debug)]
pub struct GeneralPersistentVacancyPinnedParent {
    pub placements: Vec<GeneralFastPlacement>,
    pub source: String,
    pub source_sha256: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyDiagnostics {
    pub mode: usize,
    pub attempted: bool,
    pub seed_domain: u64,
    pub target_depth_mm: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_source: Option<String>,
    pub parent_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_independent_depth_mm: Option<f64>,
    pub initial_state_fingerprint: Option<String>,
    pub initial_active_piece_ids: Vec<String>,
    pub initial_inactive_piece_ids: Vec<String>,
    pub initial_inactive_order_hash: Option<String>,
    pub layers_completed: usize,
    pub direct_insertions: usize,
    pub ejection_insertions: usize,
    pub immediate_reversals_rejected: usize,
    pub deduplicated_states: usize,
    pub distinct_signatures_retained: usize,
    pub complete_states: usize,
    pub publication_rejections: usize,
    /// The *composite* verdict: the reported layout is admissible to this
    /// run's search envelope **and** valid under the clearance contract (the
    /// `validate_and_measure_placements` check).
    ///
    /// Unchanged in meaning; read it together with `contract_valid`, which
    /// separates the two halves.
    pub exact_valid: bool,
    /// The *contract* verdict for the layout this report describes: the
    /// raw-source exact validator's answer alone, with no search envelope in
    /// it (the `validate_placements_against_contract` check).
    ///
    /// The subject is the arm's published placements when it published, and
    /// otherwise the parent layout `parent_fingerprint` names - i.e. always the
    /// layout the rest of these fields are about. It is `false` when there is
    /// no complete layout to judge, which is the same convention `exact_valid`
    /// already follows.
    ///
    /// `exact_valid` additionally requires every canonical collision polygon,
    /// expanded by `search_offset_allowance_mm` among other terms, to fit the
    /// sheet and stay pairwise disjoint. So `contract_valid && !exact_valid` is
    /// a real and unremarkable state: it is what a pinned record fixture found
    /// under a narrow allowance looks like when it is replayed under a wider
    /// one. Without this field that replay is indistinguishable from an
    /// actually illegal layout.
    ///
    /// Reporting only. Acceptance keeps using `exact_valid`.
    pub contract_valid: bool,
    /// The reported layout's depth, measured through `PolygonSet::bounds` and
    /// therefore snapped to the 0.001 mm canonical grid.
    pub independent_depth_mm: Option<f64>,
    /// The same depth measured on the untouched `f64` source rings, which
    /// cannot round in either direction. Compare *this* against a hard
    /// threshold; `independent_depth_mm` can snap across one by up to half a
    /// grid step. See
    /// [`crate::validation::general_polygon::raw_source_long_axis_depth_mm`].
    pub raw_source_depth_mm: Option<f64>,
    pub final_placement_fingerprint: Option<String>,
    pub final_placements: Vec<GeneralCoupledSeparatorPlacementDiagnostics>,
    pub work: GeneralPersistentVacancyWorkDiagnostics,
    pub layers: Vec<GeneralPersistentVacancyLayerDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settle: Option<GeneralPersistentVacancySettleDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reconstruction: Option<GeneralPersistentVacancyReconstructionDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub construction: Option<GeneralPersistentVacancyConstructionDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_drop: Option<GeneralPersistentVacancyGroupDropDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lns: Option<GeneralPersistentVacancyLnsDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontier_feasibility: Option<Vec<GeneralPersistentVacancyFeasibilityRow>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive: Option<GeneralPersistentVacancyArchiveDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alternation: Option<GeneralPersistentVacancyAlternationDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recombination: Option<GeneralPersistentVacancyRecombinationDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounded_reinsertion: Option<GeneralPersistentVacancyBoundedReinsertionDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ladder_compression: Option<GeneralPersistentVacancyLadderCompressionDiagnostics>,
    /// Mode 27: the standalone micro-legalization probe run on the parent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub micro_legalization: Option<GeneralMicroLegalizationDiagnostics>,
    /// Mode 28: the standalone conflict-targeted re-placement repair run on
    /// the parent under the requested bound.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement_repair: Option<GeneralReplacementRepairDiagnostics>,
    /// Mode 29: the standalone joint multi-piece re-placement repair run on the
    /// parent under the requested bound.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub joint_replacement: Option<GeneralJointReplacementDiagnostics>,
    /// Modes 30 and 31: the standalone global pressure-balanced legalization
    /// run on the parent, under the parent's own sheet (30) or under the
    /// requested depth bound (31).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_legalization: Option<GeneralGlobalLegalizationDiagnostics>,
    /// Mode 34: the lane-owned compression schedule's own report.
    ///
    /// Compiled only under `compression-schedule`, so a default build's
    /// document has neither the field nor a `null` where it would be.
    #[cfg(feature = "compression-schedule")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression_schedule: Option<GeneralCompressionScheduleDiagnostics>,
    pub cap_exhausted: Option<String>,
    pub failure_reason: Option<String>,
}

/// One profiling phase's contribution to one mode-26 region.
///
/// Diagnostics only, and only in a `search-profiling` build. See
/// [`GeneralPersistentVacancyLadderAnatomy`] for the tier contract.
#[cfg(feature = "search-profiling")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralLadderPhaseDelta {
    /// Wall-clock milliseconds this phase accumulated inside the region,
    /// summed over every thread that recorded any.
    pub milliseconds: f64,
    /// How many times the phase was entered inside the region.
    pub calls: u64,
}

/// The wall-clock anatomy of one mode-26 region: an arm, a rung, or the whole
/// ladder.
///
/// # Why this is a `search-profiling` field rather than an ordinary one
///
/// The four pinned regression gates measure the default build, and the default
/// build must not acquire a clock read, a snapshot, or a serialised field it
/// did not have. So the whole block is `#[cfg(feature = "search-profiling")]`:
/// without the feature the field does not exist, no `Instant` is read, and the
/// generated code is the one the gates pin. With the feature it costs one
/// `Instant::now` pair and one [`crate::profiling::snapshot`] per *arm* — tens
/// of them in a whole ladder, against arms that run for seconds each — so the
/// measurement it reports is not the measurement it perturbs.
///
/// Like everything in [`crate::profiling`], these are wall-clock quantities and
/// are therefore not reproducible; nothing here may ever reach a search
/// decision, and nothing does — the ladder writes these fields and never reads
/// them back.
///
/// `phases` and `counters` are *deltas* of the process-wide profiling totals
/// across the region, keyed by the stable names in
/// [`crate::profiling::Phase::name`] and [`crate::profiling::Counter::name`].
/// A mode-26 ladder is strictly sequential between arms, so a delta taken
/// around one arm is that arm's own work even though the arm itself runs on a
/// pool of threads. Phases that
/// [`crate::profiling::Phase::is_enclosing`] must be excluded from a share
/// table, exactly as in the whole-run profile.
#[cfg(feature = "search-profiling")]
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyLadderAnatomy {
    /// Wall-clock milliseconds spent in the region as a whole.
    pub wall_ms: f64,
    /// The clamped mode-0 pipeline run (relaxed epochs plus the coupled
    /// separator). Zero outside an arm.
    pub separator_ms: f64,
    /// `coupled_independent_source_depth` on the arm's own state.
    pub depth_measure_ms: f64,
    /// `count_exact_overlap_pairs` on the arm's own state.
    pub overlap_count_ms: f64,
    /// `validate_and_measure_placements` on the arm's own state.
    pub exact_validate_ms: f64,
    /// Repair tier one: `micro_legalize`.
    pub micro_legalization_ms: f64,
    /// Repair tier two: single-piece conflict-targeted re-placement.
    pub replacement_repair_ms: f64,
    /// Repair tier three: joint multi-piece re-placement.
    pub joint_replacement_ms: f64,
    /// Repair tier four: the global pressure-balanced program (mode 31).
    pub global_legalization_ms: f64,
    /// Fingerprinting, cloning and diagnostics assembly around the arms; only
    /// filled on a rung and on the ladder, where it is the orchestration cost
    /// that is not inside any arm.
    pub orchestration_ms: f64,
    /// Per-phase deltas across the region.
    pub phases: BTreeMap<String, GeneralLadderPhaseDelta>,
    /// Per-counter deltas across the region.
    pub counters: BTreeMap<String, u64>,
}

/// Mode-26 (clamped-sheet ladder compression) diagnostics: the ladder of
/// effective sheet long-axis bounds walked from the parent's own depth down to
/// the requested final bound, one row per step.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyLadderCompressionDiagnostics {
    /// The parent's independently measured depth, i.e. the ladder's top rung.
    pub parent_depth_mm: f64,
    /// The requested final bound (CLI arg 45), i.e. the ladder's bottom rung.
    pub final_bound_mm: f64,
    /// The uniform bound decrement actually used between consecutive rungs.
    pub step_mm: f64,
    pub steps_planned: usize,
    pub steps_run: usize,
    /// The step whose state was published, or `None` when no step beat the
    /// parent and the parent itself is published.
    pub published_step: Option<usize>,
    /// The published state's own bound, when a step produced it.
    pub published_bound_mm: Option<f64>,
    /// The whole ladder's wall-clock anatomy. `search-profiling` builds only.
    #[cfg(feature = "search-profiling")]
    pub anatomy: GeneralPersistentVacancyLadderAnatomy,
    pub steps: Vec<GeneralPersistentVacancyLadderStepDiagnostics>,
}

/// One rung of the mode-26 ladder. Each rung runs the clamped pipeline from
/// up to two warm starts (see `GeneralPersistentVacancyLadderArmDiagnostics`)
/// and keeps the best of both.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyLadderStepDiagnostics {
    pub step: usize,
    /// The effective sheet long axis this rung handed the search.
    pub bound_mm: f64,
    /// The strip depth the warm-start incumbents were handed at, one separator
    /// contraction above `bound_mm`.
    pub seed_depth_mm: f64,
    /// This rung's wall-clock anatomy. `search-profiling` builds only.
    #[cfg(feature = "search-profiling")]
    pub anatomy: GeneralPersistentVacancyLadderAnatomy,
    pub arms: Vec<GeneralPersistentVacancyLadderArmDiagnostics>,
    /// How many arm attempts this rung spent across both warm starts.
    pub attempts_run: usize,
    /// Whether this rung produced a new deepest exact-valid publication.
    pub improved_publication: bool,
    /// Whether that publication came from the micro-legalization pass rather
    /// than from the separator legalizing a target on its own.
    pub published_by_micro_legalization: bool,
    /// Which repair tier produced the publication, when a repair did:
    /// `microLegalization` or `replacement`. `None` means the separator
    /// legalized the target itself.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_repair_tier: Option<String>,
    /// How many rollback comparisons this rung's arms accepted as the same
    /// reading despite a bitwise difference, summed over every arm and every
    /// contraction target. Under the mode-26 clamp these are the rollbacks the
    /// bit-exact rule used to abort the target over; see the
    /// `CoupledRollbackComparison` policy.
    #[serde(skip_serializing_if = "usize_is_zero")]
    pub rollback_disagreements_tolerated: usize,
    /// The widest `f32`-ulp gap any of those comparisons saw, tolerated or not.
    #[serde(skip_serializing_if = "u32_is_zero")]
    pub rollback_disagreement_max_pressure_ulps: u32,
    /// The deepest exact-valid depth known after this rung.
    pub published_depth_mm_after: f64,
    /// The compression frontier's measured depth after this rung, feasible or
    /// not.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chained_depth_mm_after: Option<f64>,
    /// Whether the rung moved the compression frontier at all.
    pub chain_advanced: bool,
}

/// One warm start of one mode-26 rung.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyLadderArmDiagnostics {
    /// `feasible` for the arm warm-started from the deepest exact-valid state
    /// known, `compression` for the arm warm-started from the (possibly
    /// infeasible) compression frontier.
    pub role: String,
    /// Provenance of the warm-start state: `parent`, `step{k}` for a rung's
    /// exact-accepted state, or `step{k}:terminal` for a rung's terminal
    /// minimum-loss state.
    pub warm_start_source: String,
    /// The warm-start state's own measured depth under the real request, which
    /// exceeds `bound_mm` whenever the clamp is doing work.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warm_start_depth_mm: Option<f64>,
    pub separator_attempted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub separator_skipped_reason: Option<String>,
    /// The clamped arm's own reported final depth.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arm_final_depth_mm: Option<f64>,
    /// How many contraction targets the clamped arm attempted and accepted,
    /// and how many epochs of the legacy relaxed loop improved before it.
    /// Together these say whether a failed arm stalled in the separator or
    /// never got a foothold at all.
    pub arm_targets_attempted: usize,
    pub arm_targets_accepted: usize,
    pub epochs_improved: usize,
    /// How many rollback comparisons this arm's contraction targets accepted as
    /// the same reading despite a bitwise difference.
    #[serde(skip_serializing_if = "usize_is_zero")]
    pub rollback_disagreements_tolerated: usize,
    /// The widest `f32`-ulp gap any of those comparisons saw, tolerated or not.
    /// A value at or above `COUPLED_ROLLBACK_PRESSURE_ULP_BUDGET` means the
    /// arm hit the edge of the budget and the budget is the thing under test.
    #[serde(skip_serializing_if = "u32_is_zero")]
    pub rollback_disagreement_max_pressure_ulps: u32,
    /// Whether this arm's separator run ended on a rollback the comparison
    /// refused. That is the abort the tolerant policy exists to remove, so it
    /// is reported as its own flag rather than left inside the skip reason.
    #[serde(skip_serializing_if = "bool_is_false")]
    pub aborted_by_rollback_disagreement: bool,
    /// Which rollback comparison the clamped separator ran under. Every mode-26
    /// rung reports `toleratesPoleRounding`; the field is the ladder's own
    /// record that the clamp, and only the clamp, opted in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rollback_comparison: Option<String>,
    /// Residual loss of the arm's terminal state, when it ended on a failed
    /// target.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_collision_pairs: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_boundary_violations: Option<usize>,
    /// Whether the state this arm produced is the separator's terminal
    /// (generally infeasible) state rather than an exact-accepted one.
    pub from_terminal: bool,
    /// The arm's resulting state re-measured against the real request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub converged_depth_mm: Option<f64>,
    /// How far that state still protrudes past the rung's bound; `0.0` means
    /// the clamp was fully honoured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bound_excess_mm: Option<f64>,
    /// Exact pairwise source-polygon overlap residue in that state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlap_pairs: Option<usize>,
    /// Whether that state validates against the real request.
    pub exact_valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exact_rejection_reason: Option<String>,
    pub state_fingerprint: Option<String>,
    /// Which retry of this rung arm produced the row, counting from zero.
    pub attempt: usize,
    /// This arm's wall-clock anatomy. `search-profiling` builds only.
    #[cfg(feature = "search-profiling")]
    pub anatomy: GeneralPersistentVacancyLadderAnatomy,
    /// The micro-legalization pass run on the arm's rejected state, when one
    /// was attempted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub micro_legalization: Option<GeneralMicroLegalizationDiagnostics>,
    /// The depth of the micro-legalized state, when the pass published one.
    /// This is the arm's publication candidate whenever `exact_valid` is
    /// false.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub micro_legalized_depth_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub micro_legalized_fingerprint: Option<String>,
    /// The second-tier conflict-targeted re-placement repair, run on the
    /// arm's rejected state only when the micro-legalizer refused or failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement_repair: Option<GeneralReplacementRepairDiagnostics>,
    /// The depth of the re-placed state, when that tier published one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement_repaired_depth_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement_repaired_fingerprint: Option<String>,
    /// The third-tier joint multi-piece re-placement repair, run on the arm's
    /// rejected state only when both the micro-legalizer and the single-piece
    /// re-placement refused or failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub joint_replacement: Option<GeneralJointReplacementDiagnostics>,
    /// The depth of the jointly re-placed state, when that tier published one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub joint_replaced_depth_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub joint_replaced_fingerprint: Option<String>,
    /// The fourth-tier global pressure-balanced legalization, run on the arm's
    /// rejected state only when all three local tiers refused or failed. Every
    /// piece is a variable here, so this is the only tier that can answer a
    /// residue whose room is somewhere else in the layout.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_legalization: Option<GeneralGlobalLegalizationDiagnostics>,
    /// The depth of the globally legalized state, when that tier published one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_legalized_depth_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_legalized_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
}

/// Mode-22 (alternation fixpoint) diagnostics: one row per cycle of the
/// separator/descent alternation, plus the total cycle count.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyAlternationDiagnostics {
    pub cycles_run: usize,
    pub cycles: Vec<GeneralPersistentVacancyAlternationCycleDiagnostics>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyAlternationCycleDiagnostics {
    pub cycle: usize,
    pub separator_attempted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub separator_depth_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub separator_independent_depth_mm: Option<f64>,
    pub separator_improved: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub separator_failure_reason: Option<String>,
    pub descent_attempted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub descent_target_depth_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub descent_independent_depth_mm: Option<f64>,
    pub descent_improved: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub descent_failure_reason: Option<String>,
}

/// Mode-23 (recombination) diagnostics: the scale-free cut applied to
/// parent A's measured short-axis span, the resulting seam composition, and
/// the legalized outcome.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyRecombinationDiagnostics {
    pub cut_fraction: f64,
    pub short_axis_threshold_mm: f64,
    pub pieces_from_parent_a: usize,
    pub pieces_from_parent_b: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hybrid_overlap_pairs: Option<usize>,
    pub hybrid_independent_depth_mm: f64,
    pub legalization_seed_depth_mm: f64,
    pub legalized_depth_mm: f64,
}

/// Mode-24 (bounded-depth reinsertion) diagnostics: the hard bound, the
/// ejection/reinsertion census it induced on the parent, one row per
/// reinserted piece in the order they were replaced, and the final measured
/// depth when every ejected piece found a pose inside the bound.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyBoundedReinsertionDiagnostics {
    pub bound_mm: f64,
    pub parent_depth_mm: f64,
    pub kept_count: usize,
    pub ejected_count: usize,
    pub reinserted_count: usize,
    /// Reinsertion attempts in the order they ran (displaced pieces by
    /// descending area, `pieceId` breaking ties). A failing run stops at the
    /// first piece with no in-bound pose, so this is a prefix of the
    /// ejected set rather than a permutation of it.
    pub pieces: Vec<GeneralPersistentVacancyBoundedReinsertionPieceRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_piece_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_depth_mm: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyBoundedReinsertionPieceRow {
    pub piece_id: String,
    /// The piece's own long-axis extent in the parent layout, which is what
    /// put it over the bound and got it ejected.
    pub parent_extent_mm: f64,
    /// Exact-valid poses the construction machinery returned for this slot.
    pub candidates_considered: usize,
    /// Candidates discarded because their measured extent exceeded the bound.
    /// The insertion sheet is already clamped to the bound, so this counting
    /// only ever fires if the two measures disagree; it is the explicit
    /// contract check behind the geometric one.
    pub bound_rejections: usize,
    pub reinserted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placed_extent_mm: Option<f64>,
    /// Poses seeded at this piece's own vacated pose, and the exact-valid
    /// finalists they produced.
    pub anchor_local_candidates: usize,
    pub anchor_local_finalists: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
}

/// Mode-28 (conflict-targeted re-placement) diagnostics: the violation graph
/// the pass was pointed at, the ejection set it derived from it, and what the
/// clamped insertion machinery managed to do with the vacated space.
///
/// Also carried per mode-26 rung arm, where this pass is the second repair
/// tier: it runs only after the micro-legalizer has refused or failed.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralReplacementRepairDiagnostics {
    /// Whether the pass got past admission and actually re-placed anything.
    pub attempted: bool,
    /// The clamped sheet long axis every re-placed pose had to fit inside.
    pub bound_mm: f64,
    /// The input state's residue against the bare publication contracts.
    pub violating_pairs: usize,
    pub boundary_pieces: usize,
    pub material_pairs: usize,
    pub collision_pairs: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_material_deficit_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_envelope_push_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_boundary_deficit_mm: Option<f64>,
    /// The violation graph's connected components, and the admissible size of
    /// the largest one.
    pub component_count: usize,
    pub largest_component_pieces: usize,
    pub component_limit: usize,
    /// The largest ejection set the pass would have attempted.
    pub ejection_limit: usize,
    pub ejected_count: usize,
    /// The ejected pieces in placement-slot order. `pieces` reports the same
    /// set in the order they were actually re-placed.
    pub ejected_piece_ids: Vec<String>,
    /// Total violation mass incident to each ejected piece, aligned with
    /// `ejected_piece_ids`. This is the quantity the ejection choice
    /// maximizes.
    pub ejected_mass_mm: Vec<f64>,
    /// Hash of the re-placement order, which is the pass's determinism anchor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ejected_order_hash: Option<String>,
    /// The residue left in the layout once the ejection set was removed. The
    /// pair count is zero by construction; a boundary residue may survive and
    /// is handed to the micro-legalizer.
    pub kept_violating_pairs: usize,
    pub kept_boundary_pieces: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kept_micro_legalization: Option<GeneralMicroLegalizationDiagnostics>,
    /// The anchor-local seeding's aimed input: the length of the single-piece
    /// separating projection computed for each ejected piece, aligned with the
    /// re-placement order, plus how many of those projections reached a
    /// fixpoint and how many could not be measured at all.
    pub projected_displacements_mm: Vec<f64>,
    pub projections_converged: usize,
    pub projection_failures: usize,
    /// One row per attempted piece, in re-placement order. A failing pass
    /// stops at the first piece with no in-bound pose, so this is a prefix of
    /// the ejection set rather than a permutation of it.
    pub pieces: Vec<GeneralReplacementRepairPieceRow>,
    pub replaced_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_piece_id: Option<String>,
    /// Whether the authoritative validator accepted the re-placed state.
    pub exact_valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cap_exhausted: Option<String>,
    pub work: GeneralPersistentVacancyWorkDiagnostics,
}

/// Mode-29 (joint multi-piece re-placement) diagnostics: the violation graph
/// the pass was pointed at, the whole-component ejection set it derived from
/// it, and every insertion order it tried on that set.
///
/// Also carried per mode-26 rung arm, where this pass is the third repair
/// tier: it runs only after both the micro-legalizer and the single-piece
/// re-placement have refused or failed.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralJointReplacementDiagnostics {
    /// Whether the pass got past admission and actually re-placed anything.
    pub attempted: bool,
    /// The clamped sheet long axis every re-placed pose had to fit inside.
    pub bound_mm: f64,
    /// The input state's residue against the bare publication contracts.
    pub violating_pairs: usize,
    pub boundary_pieces: usize,
    pub material_pairs: usize,
    pub collision_pairs: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_material_deficit_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_envelope_push_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_boundary_deficit_mm: Option<f64>,
    pub component_count: usize,
    pub largest_component_pieces: usize,
    pub component_limit: usize,
    pub ejection_limit: usize,
    /// The joint ejection set: every piece of every pair-bearing component,
    /// in placement-slot order. This is what separates the tier from mode 28,
    /// whose set is a vertex cover of the same graph.
    pub ejected_count: usize,
    pub ejected_piece_ids: Vec<String>,
    /// Total violation mass incident to each ejected piece, aligned with
    /// `ejectedPieceIds`. On the residue class this tier exists for, these are
    /// millimetres rather than microns.
    pub ejected_mass_mm: Vec<f64>,
    /// The residue left once the whole set was removed. The pair count is zero
    /// by construction - both endpoints of every violating pair are ejected -
    /// and a boundary residue is handed to the micro-legalizer.
    pub kept_violating_pairs: usize,
    pub kept_boundary_pieces: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kept_micro_legalization: Option<GeneralMicroLegalizationDiagnostics>,
    /// The anchor-local seeding's aimed input, as in mode 28: the length of
    /// each ejected piece's single-piece separating projection, aligned with
    /// the ejection set.
    pub projected_displacements_mm: Vec<f64>,
    pub projections_converged: usize,
    pub projection_failures: usize,
    /// The insertion orders the pass planned and the ones it actually spent,
    /// summed over every component pass. `ordersExhaustive` says whether the
    /// plan was every permutation of the set or the bounded rotation family a
    /// larger set falls back to.
    pub orders_planned: usize,
    pub orders_tried: usize,
    pub orders_exhaustive: bool,
    /// The pose-swap round: how many exchanges were available under the cap,
    /// how many rounds ran, and how many exchanges were actually attempted.
    pub swap_pairs_planned: usize,
    pub swap_rounds_run: usize,
    pub swap_attempts_tried: usize,
    /// The finalist-combination beam: how many non-greedy rank combinations the
    /// pass actually spent, summed over every component pass. The beam runs
    /// after the orders and the swap round, so it only ever reaches states
    /// nothing before it could.
    pub beam_combinations_tried: usize,
    /// The connected components the pass worked through, one row each, in the
    /// order it repaired them.
    pub components: Vec<GeneralJointReplacementComponentRow>,
    pub component_passes_run: usize,
    pub components_repaired: usize,
    pub components_refused: usize,
    /// One row per attempted order, in the order they were attempted, across
    /// every component pass.
    pub orders: Vec<GeneralJointReplacementOrderRow>,
    /// The ordinal of the order that published, its hash, whether it came from
    /// the swap round rather than the plain enumeration, and the finalist ranks
    /// it committed when the beam produced it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_order: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_order_hash: Option<String>,
    pub accepted_by_swap: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_finalist_ranks: Option<Vec<usize>>,
    /// Whether the authoritative validator accepted the jointly re-placed
    /// state.
    pub exact_valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cap_exhausted: Option<String>,
    pub work: GeneralPersistentVacancyWorkDiagnostics,
}

/// One connected violation component of one joint re-placement pass.
///
/// The tier repairs the violation graph one component at a time and re-surveys
/// the whole layout between them, so this is where a multi-cluster residue
/// becomes readable: which cluster was targeted, what it cost, and what the
/// layout's residue was before and after it.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralJointReplacementComponentRow {
    /// Position in the pass's component sequence.
    pub pass: usize,
    /// The component's pieces, in placement-slot order, and the violation mass
    /// incident to each.
    pub piece_ids: Vec<String>,
    pub incident_mass_mm: Vec<f64>,
    /// The whole layout's residue as this pass found it and as it left it.
    pub violating_pairs_before: usize,
    pub violating_pairs_after: usize,
    pub boundary_pieces_before: usize,
    pub boundary_pieces_after: usize,
    /// The residue left among the kept pieces once this component was removed.
    /// A non-zero pair count here is another component's conflict, which a
    /// later pass owns.
    pub kept_violating_pairs: usize,
    pub kept_boundary_pieces: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kept_micro_legalization: Option<GeneralMicroLegalizationDiagnostics>,
    pub orders_planned: usize,
    pub orders_tried: usize,
    pub swap_attempts_tried: usize,
    pub beam_combinations_planned: usize,
    pub beam_combinations_tried: usize,
    /// Whether this component's conflict was cleared, and by which attempt.
    pub repaired: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_ordinal: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_order_hash: Option<String>,
    pub accepted_by_swap: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_finalist_ranks: Option<Vec<usize>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<String>,
}

/// One insertion order of one joint re-placement attempt.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralJointReplacementOrderRow {
    /// Position in the pass's own attempt sequence, counting the swap round's
    /// attempts after the plain enumeration's and the beam's after those,
    /// across every component pass.
    pub ordinal: usize,
    /// Which component pass this attempt belongs to.
    pub component_pass: usize,
    /// The order itself, and its hash - the determinism anchor, exactly as in
    /// mode 28.
    pub order_piece_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_hash: Option<String>,
    /// The two pieces whose vacated poses were exchanged before this attempt,
    /// when it belongs to the swap round. `None` for a plain order.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub swap_pair: Option<Vec<String>>,
    /// The finalist rank each piece committed, when this attempt came from the
    /// combination beam. `None` is the greedy shallowest-first commit, which is
    /// every order and every swap attempt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finalist_ranks: Option<Vec<usize>>,
    /// One row per attempted piece, in this order. A failing attempt stops at
    /// the first piece with no in-bound pose, so this is a prefix of the
    /// ejection set rather than a permutation of it.
    pub pieces: Vec<GeneralReplacementRepairPieceRow>,
    pub replaced_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_piece_id: Option<String>,
    pub exact_valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralReplacementRepairPieceRow {
    pub piece_id: String,
    /// Exact-valid poses the construction machinery returned for this slot.
    ///
    /// Zero here is the diagnostically interesting case, and is *not* the same
    /// failure as a non-zero count with every candidate out of bound: it says
    /// the insertion machinery confirmed no legal pose for this piece anywhere
    /// on the clamped sheet, vacated space included.
    pub candidates_considered: usize,
    /// Candidates discarded because their measured extent exceeded the bound.
    /// The insertion sheet is already clamped to the bound, so this only fires
    /// if the two measures disagree; it is the explicit contract check behind
    /// the geometric one.
    pub bound_rejections: usize,
    pub replaced: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placed_extent_mm: Option<f64>,
    /// Poses seeded at this piece's own vacated pose, and the exact-valid
    /// finalists they produced. The interior-pocket instrument: a zero here
    /// with a non-zero `candidatesConsidered` says the piece was re-placed
    /// from the skyline, and a non-zero `anchorLocalFinalists` says the
    /// pocket itself was reachable.
    pub anchor_local_candidates: usize,
    pub anchor_local_finalists: usize,
    /// Modes 32 and 33 only: the orientation-perturbation stream's own
    /// candidates, rows, finalists and accepted-pose attribution for this
    /// piece. Absent for every mode that does not arm the stream, which is what
    /// keeps modes 24, 28 and 29 byte-identical.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orientation: Option<GeneralOrientationSeedingRow>,
}

/// Orientation-perturbed re-insertion (modes 32 and 33): what the continuous
/// angle ladder cost for one re-placed piece and, above all, whether the pose
/// the piece actually committed to came from it.
///
/// The four `accepted*` counters are mutually exclusive and sum to 1 on a piece
/// that found a pose and to 0 on a piece that did not, so summing them over a
/// run's rows is the attribution the whole mechanism is judged on: a non-zero
/// `acceptedOrientation` is the *only* evidence that continuous-angle
/// re-insertion did work no translation could have done.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralOrientationSeedingRow {
    /// Orientation-perturbed variants of the vacated pose seeded for this
    /// piece, the candidate poses they generated, the charged confirmation rows
    /// those poses spent, and the exact-valid finalists they produced.
    pub variants: usize,
    pub candidates: usize,
    pub rows: usize,
    pub finalists: usize,
    /// The vacated pose itself.
    pub accepted_vacated: usize,
    /// An anchor-local candidate that is not the vacated pose: the projection
    /// trajectory, a peer's pocket, or the displacement cloud - all at the
    /// vacated orientation.
    pub accepted_anchor_local: usize,
    /// An orientation-perturbed candidate.
    pub accepted_orientation: usize,
    /// A skyline-station or shelf candidate.
    pub accepted_station: usize,
    /// The accepted pose's own orientation, its signed offset from the vacated
    /// orientation on the angle grid, and whether it flipped the mirror state.
    /// Absent when the piece found no pose.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_rotation_deg: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_rotation_delta_deg: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_mirror_flipped: Option<bool>,
}

/// Serialization guard for the orientation-perturbation counters: a mode that
/// does not arm the stream must emit exactly the JSON it emitted before the
/// stream existed.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero_usize(value: &usize) -> bool {
    *value == 0
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyReconstructionDiagnostics {
    pub insertions: usize,
    pub exact_rows: usize,
    pub rows_per_piece_cap: usize,
    pub deferred_first_pass: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_piece_id: Option<String>,
    pub failed_piece_count: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyConstructionDiagnostics {
    pub restarts: usize,
    pub beam_width: usize,
    pub hint_stations_per_slot: usize,
    pub rows_per_piece_cap: usize,
    pub finalists_per_slot: usize,
    pub slots: usize,
    pub exact_rows: usize,
    pub children_generated: usize,
    pub children_deduplicated: usize,
    pub shelf_finalists: usize,
    pub void_scans: usize,
    pub fixture_prior_finalists: usize,
    pub zero_prior_finalists: usize,
    pub complete_candidates: usize,
    pub audited_candidates: usize,
    /// Anchor-local re-insertion (modes 24 and 28 only): candidate poses
    /// seeded at a re-placed piece's own vacated pose, the charged
    /// confirmation rows they spent, and the exact-valid finalists they
    /// produced. A from-scratch construction has no vacated pose, so all three
    /// stay zero there.
    pub anchor_local_candidates: usize,
    pub anchor_local_rows: usize,
    pub anchor_local_finalists: usize,
    /// Orientation-perturbed re-insertion (modes 32 and 33 only): candidate
    /// poses seeded at a rotated or mirrored variant of the vacated pose, the
    /// charged confirmation rows they spent, and the exact-valid finalists they
    /// produced. All three stay zero for every mode that does not arm the
    /// stream, which is what keeps their diagnostics byte-identical.
    #[serde(skip_serializing_if = "is_zero_usize")]
    pub orientation_candidates: usize,
    #[serde(skip_serializing_if = "is_zero_usize")]
    pub orientation_rows: usize,
    #[serde(skip_serializing_if = "is_zero_usize")]
    pub orientation_finalists: usize,
    /// Mode 25 only: the off-beam best-ever expansion parent is armed.
    pub best_ever_parent_enabled: bool,
    /// Extra expansions spent on an elite the retention step did not keep.
    pub best_ever_parent_expansions: usize,
    /// Children of those expansions that won a beam slot on their own merits.
    pub best_ever_parent_children_retained: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_restart_ordinal: Option<usize>,
    pub restart_rows: Vec<GeneralPersistentVacancyConstructionRestartRow>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyConstructionRestartRow {
    pub order: String,
    pub complete: bool,
    /// Layers of this restart whose elite was off-beam and therefore funded
    /// the extra best-ever parent expansion (mode 25 only).
    pub best_ever_parent_layers: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontier_grid: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trapped_void_cells: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub independent_depth_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyFeasibilityRow {
    pub piece_id: String,
    pub piece_frontier_grid: i64,
    pub lattice_poses_screened: usize,
    pub exact_valid_sub_frontier_poses: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_sub_frontier_grid: Option<i64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyLnsDiagnostics {
    pub rounds: usize,
    pub bridge_void_scans: usize,
    pub bridge_selections: usize,
    pub rounds_accepted: usize,
    pub rounds_reverted: usize,
    pub reinsertions: usize,
    pub reinsert_failures: usize,
    pub separation_moves: usize,
    pub separation_probes: usize,
    pub separation_zero_overlap: usize,
    pub separation_recruits: usize,
    pub separation_pair_moves: usize,
    pub separation_weight_bumps: usize,
    pub separation_relocations: usize,
    pub rounds_wandered: usize,
    pub optimizer_improvements: usize,
    pub frontier_before_grid: i64,
    pub frontier_after_grid: i64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyGroupDropDiagnostics {
    pub rounds: usize,
    pub cuts_evaluated: usize,
    pub probes: usize,
    pub accepted_drops: usize,
    pub frontier_before_grid: i64,
    pub frontier_after_grid: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancySettleDiagnostics {
    pub sweeps: usize,
    pub attempts: usize,
    pub accepted_moves: usize,
    pub exact_rows: usize,
    pub frontier_before_grid: i64,
    pub frontier_after_grid: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyArchiveDiagnostics {
    pub stagnation_threshold_layers: usize,
    pub revival_cooldown_layers: usize,
    pub max_revival_expansions: usize,
    pub revival_policy: String,
    pub revivals_expanded: usize,
    pub revivals_skipped: usize,
    pub revival_children_generated: usize,
    pub revival_children_retained: usize,
    pub archive_peak_bytes: usize,
    pub final_archived_area_fingerprint: Option<String>,
    pub final_archived_count_fingerprint: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyArchiveLayerDiagnostics {
    pub layers_since_improvement: usize,
    pub revival_attempted: bool,
    pub revival_expanded: bool,
    pub revival_kind: Option<String>,
    pub revived_state_fingerprint: Option<String>,
    pub replaced_state_fingerprint: Option<String>,
    pub skipped_reason: Option<String>,
    pub revival_children_generated: usize,
    pub revival_children_retained: usize,
    pub archived_area_updated: bool,
    pub archived_count_updated: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyWorkDiagnostics {
    pub selected_piece_slots: usize,
    pub orientation_streams: usize,
    pub source_feature_visits: usize,
    pub position_source_attempts: usize,
    pub returned_positions: usize,
    pub hazard_queries: usize,
    pub proxy_pressure_visits: usize,
    pub exact_finalist_rows: usize,
    pub experimental_collision_builds: usize,
    pub validator_collision_builds: usize,
    pub experimental_pair_visits: usize,
    pub validator_pair_visits: usize,
    pub transformed_collision_vertices: usize,
    pub clipper_input_vertices: usize,
    pub clipper_output_vertices: usize,
    pub partial_audits: usize,
    pub complete_audits: usize,
    pub retained_peak_bytes: usize,
    pub selector_diagnostic_peak_bytes: usize,
    pub total_retained_peak_bytes: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyLayerDiagnostics {
    pub layer: usize,
    pub parents: usize,
    pub generated_children: usize,
    pub retained_states: usize,
    pub distinct_contact_signatures: usize,
    pub selected_piece_ids: Vec<String>,
    pub parent_selections: Vec<GeneralPersistentVacancyParentSelectionDiagnostics>,
    pub direct_insertions: usize,
    pub ejection_insertions: usize,
    pub best_inactive_piece_count: usize,
    pub best_inactive_piece_ids: Vec<String>,
    pub best_inactive_area_grid2: String,
    pub best_state_fingerprint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elite: Option<GeneralPersistentVacancyEliteLayerDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive: Option<GeneralPersistentVacancyArchiveLayerDiagnostics>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyEliteLayerDiagnostics {
    pub entering_population_hash: String,
    pub ordinary_child_order_hash: String,
    pub complete_candidate_order_hash: String,
    pub pre_carryover_work: GeneralPersistentVacancyWorkDiagnostics,
    pub area_elite_fingerprint: String,
    pub area_elite_inactive_piece_count: usize,
    pub area_elite_inactive_area_grid2: String,
    pub count_elite_fingerprint: String,
    pub count_elite_inactive_piece_count: usize,
    pub count_elite_inactive_area_grid2: String,
    pub best_ever_area_elite_fingerprint: String,
    pub best_ever_area_elite_inactive_piece_count: usize,
    pub best_ever_area_elite_inactive_area_grid2: String,
    pub best_ever_count_elite_fingerprint: String,
    pub best_ever_count_elite_inactive_piece_count: usize,
    pub best_ever_count_elite_inactive_area_grid2: String,
    pub offered_carryover_fingerprints: Vec<String>,
    pub offered_carryovers_distinct: bool,
    pub retained_carryover_fingerprints: Vec<String>,
    pub expanded_carryover_fingerprints: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyParentSelectionDiagnostics {
    pub parent_state_fingerprint: String,
    pub inactive_order_hash: String,
    pub scheduler_family: String,
    pub hardest_piece_id: String,
    pub rotation_start_index: Option<usize>,
    pub coverage_piece_id: Option<String>,
    pub transition_seed: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revived: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relocated_piece_id: Option<String>,
    pub slots: Vec<GeneralPersistentVacancySelectionSlotDiagnostics>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancySelectionSlotDiagnostics {
    pub selected_ordinal: usize,
    pub piece_id: String,
    pub angle_seed: u64,
    pub diversity_seed: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPrecompressionFrontierVacancyDiagnostics {
    pub mode: usize,
    pub attempted: bool,
    pub target_depth_mm: Option<f64>,
    pub incumbent_strip_depth_mm: Option<f64>,
    pub checkpoint_fingerprint: Option<String>,
    pub selected_piece_ids: Vec<String>,
    pub incumbent_parent_fingerprint: Option<String>,
    pub eligible_parent_fingerprints: Vec<String>,
    pub selected_parent_fingerprint: Option<String>,
    pub selected_parent_depth_mm: Option<f64>,
    pub selected_compressed_raw_loss: Option<f64>,
    pub full_scores: usize,
    pub full_score_pair_visits: usize,
    pub rebuilt_children: Vec<GeneralPrecompressionFrontierChildDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rebuilt_child_record_hash: Option<String>,
    pub rebuild: GeneralConflictRuinRebuildDiagnostics,
    pub control: Option<GeneralCoupledSeparatorTargetDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage_a_seed_domain: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage_a_target_seed: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage_a_compression_seed: Option<u64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stage_a_worker_seeds: Vec<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage_a: Option<GeneralConflictRuinArmDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage_a_independent_audit: Option<GeneralPrecompressionIndependentAuditDiagnostics>,
    pub treatment: GeneralConflictRuinArmDiagnostics,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation_counts: Option<GeneralPrecompressionValidationDiagnostics>,
    pub mechanism_passed: bool,
    pub skipped_reason: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPrecompressionIndependentAuditDiagnostics {
    pub attempted: bool,
    pub fresh_score_agreement: bool,
    pub final_positive_pairs: Option<usize>,
    pub final_boundary_violations: Option<usize>,
    pub final_boundary_loss: Option<f64>,
    pub positive_boundary_rows: Vec<GeneralPrecompressionBoundaryRowDiagnostics>,
    pub audited_placement_fingerprint: Option<String>,
    pub independent_audit_valid: bool,
    pub independent_audit_count: usize,
    pub used_short_axis_span_mm: Option<f64>,
    pub used_long_axis_depth_mm: Option<f64>,
    pub unused_short_axis_projection_mm: Option<f64>,
    pub occupied_envelope_area_mm2: Option<f64>,
    pub rejection_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPrecompressionBoundaryRowDiagnostics {
    pub piece_id: String,
    pub violations: usize,
    pub raw_loss: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPrecompressionValidationDiagnostics {
    pub incumbent: usize,
    pub rebuilt_children: usize,
    pub stage_a: usize,
    pub stage_b: usize,
    pub total: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPrecompressionFrontierChildDiagnostics {
    pub beam_ordinal: usize,
    pub fingerprint: String,
    pub exact_overlap_area_mm2: f64,
    pub exact_positive_overlap_pairs: usize,
    pub frontier_depth_mm: f64,
    pub fresh_raw_loss: f64,
    pub fresh_positive_pairs: usize,
    pub fresh_feasible: bool,
    pub publication_valid: bool,
    pub publication_rejection_reason: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralConflictRuinDiagnostics {
    pub attempted: bool,
    pub seed_domain: u64,
    pub target_depth_mm: Option<f64>,
    pub checkpoint_fingerprint: Option<String>,
    pub selector_mode: Option<String>,
    pub root_piece_id: Option<String>,
    pub root_boundary_loss: Option<f64>,
    pub root_probe_pose: Option<GeneralCoupledSeparatorPlacementDiagnostics>,
    pub root_probe_blockers: Vec<GeneralConflictRuinBlockerDiagnostics>,
    pub root_probe_tracker_loss: Option<f64>,
    pub root_probe_tracker_boundary_loss: Option<f64>,
    pub root_probe_tracker_positive_pairs: Option<usize>,
    pub root_probe_tracker_feasible: Option<bool>,
    pub root_probe_exact_valid: Option<bool>,
    pub root_probe_exact_depth_mm: Option<f64>,
    pub root_probe_improves_incumbent: Option<bool>,
    pub root_probe_exact_rejection_reason: Option<String>,
    pub root_probe_state_fingerprint: Option<String>,
    pub removed_piece_ids: Vec<String>,
    pub removal_order_piece_ids: Vec<String>,
    pub rebuild: GeneralConflictRuinRebuildDiagnostics,
    pub retry_control: GeneralConflictRuinArmDiagnostics,
    pub treatment: GeneralConflictRuinArmDiagnostics,
    pub skipped_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralConflictRuinBlockerDiagnostics {
    pub piece_id: String,
    pub proxy_pressure: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralConflictRuinRebuildDiagnostics {
    pub elapsed_ms: f64,
    pub initial_exact_overlap_area_mm2: f64,
    pub selected_exact_overlap_area_mm2: Option<f64>,
    pub initial_positive_overlap_pairs: usize,
    pub selected_positive_overlap_pairs: Option<usize>,
    pub parent_orientation_streams: usize,
    pub cheap_queries: usize,
    pub exact_finalists: usize,
    pub exact_pair_intersection_limit: usize,
    pub exact_pair_intersections: usize,
    pub required_current_finalists: usize,
    pub orientation_build_limit: usize,
    pub orientation_builds: usize,
    pub transformed_output_vertices: usize,
    pub feature_visits: usize,
    pub pre_dedup_contact_attempts: usize,
    pub deduplicated_proposals: usize,
    pub clipper_input_vertices: usize,
    pub clipper_output_vertices: usize,
    pub partials_retained: usize,
    pub selected_state_fingerprint: Option<String>,
    pub cap_exhausted: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralConflictRuinArmDiagnostics {
    pub attempted: bool,
    pub applied_rebuild: bool,
    pub elapsed_ms: f64,
    pub initial_state_fingerprint: Option<String>,
    pub final_state_fingerprint: Option<String>,
    pub exact_valid: bool,
    pub accepted_depth_mm: Option<f64>,
    pub final_placement_fingerprint: Option<String>,
    pub final_placements: Vec<GeneralCoupledSeparatorPlacementDiagnostics>,
    pub work: GeneralConflictRuinRetryWorkDiagnostics,
    pub target: Option<GeneralCoupledSeparatorTargetDiagnostics>,
    pub failure_reason: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralConflictRuinRetryWorkDiagnostics {
    pub worker_sweeps: usize,
    pub dynamic_queries: usize,
    pub pressure_evaluations: usize,
    pub retained_confirmations: usize,
    pub hazard_updates: usize,
    pub layout_loads: usize,
    pub index_builds: usize,
    pub worker_full_score_pair_visits: usize,
    pub auditor_full_score_pair_visits: usize,
    pub auditor_dynamic_queries: usize,
    pub auditor_pressure_evaluations: usize,
    pub auditor_layout_loads: usize,
    pub auditor_index_builds: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralCoupledSeparatorArmDiagnostics {
    pub pressure_model: String,
    pub attempted: bool,
    pub targets_attempted: usize,
    pub targets_accepted: usize,
    pub initial_depth_mm: f64,
    pub final_depth_mm: f64,
    pub worker_sweeps: usize,
    pub dynamic_queries: usize,
    pub pressure_evaluations: usize,
    pub retained_confirmations: usize,
    pub hazard_updates: usize,
    pub layout_loads: usize,
    pub catalog_builds: usize,
    pub immutable_variant_builds: usize,
    pub index_builds: usize,
    pub worker_full_score_pair_visits: usize,
    pub auditor_full_score_pair_visits: usize,
    pub auditor_dynamic_queries: usize,
    pub auditor_pressure_evaluations: usize,
    pub auditor_layout_loads: usize,
    pub auditor_index_builds: usize,
    pub independently_measured_final_depth_mm: Option<f64>,
    pub final_placement_fingerprint: Option<String>,
    pub final_placements: Vec<GeneralCoupledSeparatorPlacementDiagnostics>,
    /// The minimum-loss state of the arm's last, failed contraction target.
    ///
    /// `final_placements` only ever reflects *exact-accepted* states, so an
    /// arm whose first target fails reports its own input back unchanged and
    /// the compression work that target actually did is invisible from
    /// outside. This records that terminal state instead. It is generally
    /// *infeasible* - it is precisely the state the arm could not legalize -
    /// so it is diagnostics and warm-start material only, never a publication
    /// candidate. Empty whenever the arm ended without a failed-target
    /// checkpoint.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub terminal_placements: Vec<GeneralCoupledSeparatorPlacementDiagnostics>,
    /// The strip depth that terminal state was scored against.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_strip_depth_mm: Option<f64>,
    /// Residual collision and boundary loss of that terminal state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_collision_pairs: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_boundary_violations: Option<usize>,
    pub skipped_reason: Option<String>,
    /// Which rollback comparison this arm's contraction targets ran under, when
    /// it was anything other than the bit-exact default. `None` is the exact
    /// comparison, which is every arm outside the
    /// mode-26 sheet clamp, so the field never appears in those modes'
    /// diagnostics.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rollback_comparison: Option<String>,
    pub targets: Vec<GeneralCoupledSeparatorTargetDiagnostics>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralCoupledSeparatorPlacementDiagnostics {
    pub piece_id: String,
    pub rotation_deg: f64,
    pub mirrored: bool,
    pub translate_short_axis: f64,
    pub translate_long_axis: f64,
}

/// Serialization guards for counters that are only ever non-zero on the
/// mode-26 tolerant path, so every other mode's diagnostics stay byte-for-byte
/// what they were before the counter existed.
fn usize_is_zero(value: &usize) -> bool {
    *value == 0
}

fn u32_is_zero(value: &u32) -> bool {
    *value == 0
}

fn bool_is_false(value: &bool) -> bool {
    !*value
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralCoupledSeparatorTargetDiagnostics {
    pub ordinal: usize,
    pub target_depth_mm: f64,
    pub compression_split_mm: f64,
    pub target_seed: u64,
    pub compression_seed: u64,
    pub worker_seeds: Vec<u64>,
    pub initial_state_fingerprint: String,
    pub final_state_fingerprint: String,
    pub rounds: usize,
    pub strikes: usize,
    pub rollbacks: usize,
    pub full_rescore_agreements: usize,
    /// How many loss magnitudes this target's rollback comparisons accepted as
    /// the same reading despite differing bitwise. Always `0` - and omitted
    /// from the serialized diagnostics - for an arm running the default exact
    /// comparison, which is every arm outside the mode-26 sheet clamp.
    #[serde(skip_serializing_if = "usize_is_zero")]
    pub rollback_disagreements_tolerated: usize,
    /// The widest gap, in `f32` units in the last place, between two readings
    /// of one magnitude that differed bitwise - whether or not it was
    /// tolerated. This is what makes the tolerance budget measurable rather
    /// than assumed.
    #[serde(skip_serializing_if = "u32_is_zero")]
    pub rollback_disagreement_max_pressure_ulps: u32,
    pub initial_raw_loss: f64,
    pub minimum_raw_loss: f64,
    pub final_raw_loss: f64,
    pub final_weighted_loss: f64,
    pub feasible: bool,
    pub exact_valid: bool,
    pub exact_accepted: bool,
    pub exact_rejection_reason: Option<String>,
    pub accepted_depth_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boundary_projection: Option<GeneralBoundaryProjectionDiagnostics>,
    pub cap_exhausted: Option<String>,
    pub failure_reason: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralBoundaryProjectionDiagnostics {
    pub attempted: bool,
    pub root_piece_id: Option<String>,
    pub root_boundary_loss: Option<f64>,
    pub projected_pose: Option<GeneralCoupledSeparatorPlacementDiagnostics>,
    pub projected_pieces: Vec<GeneralCoupledSeparatorPlacementDiagnostics>,
    pub exact_valid: bool,
    pub exact_accepted: bool,
    pub exact_depth_mm: Option<f64>,
    pub state_fingerprint: Option<String>,
    pub rejection_reason: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralRelaxedLossDistribution {
    pub samples: usize,
    pub sum: f64,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
}

impl GeneralRelaxedLossDistribution {
    fn observe(&mut self, value: f64) {
        self.samples = self.samples.saturating_add(1);
        self.sum += value;
        self.minimum = Some(self.minimum.map_or(value, |minimum| minimum.min(value)));
        self.maximum = Some(self.maximum.map_or(value, |maximum| maximum.max(value)));
    }

    fn merge(&mut self, other: Self) {
        self.samples = self.samples.saturating_add(other.samples);
        self.sum += other.sum;
        if let Some(minimum) = other.minimum {
            self.minimum = Some(self.minimum.map_or(minimum, |current| current.min(minimum)));
        }
        if let Some(maximum) = other.maximum {
            self.maximum = Some(self.maximum.map_or(maximum, |current| current.max(maximum)));
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralRelaxedEpochDiagnostics {
    pub epoch: usize,
    pub selected_lane: usize,
    pub restart_disruptions: usize,
    pub target_depth_mm: f64,
    pub weighted_loss: f64,
    pub collision_pairs: usize,
    pub blocking_pairs: Vec<GeneralRelaxedPairDiagnostics>,
    pub boundary_violations: usize,
    pub boundary_piece_ids: Vec<String>,
    pub surrogate_feasible: bool,
    pub exact_valid: bool,
    pub exact_accepted: bool,
    pub translation_evaluations: usize,
    pub rotation_evaluations: usize,
    pub complete_queries: usize,
    pub retained_f64_confirmations: usize,
    pub accepted_moves: usize,
    pub incumbent_depth_before_mm: f64,
    pub incumbent_depth_after_mm: f64,
    pub incumbent_depth_delta_mm: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralRelaxedPairDiagnostics {
    pub first_piece_id: String,
    pub second_piece_id: String,
    pub raw_penalty: f64,
    pub guided_weight: f64,
    pub weighted_pressure: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeneralRelaxedOutcome {
    pub result: GeneralFastResult,
    pub diagnostics: GeneralRelaxedDiagnostics,
}

#[derive(Clone, Copy, Debug)]
struct Triangle {
    points: [IrregularPoint; 3],
    bounds: IrregularBounds,
}

#[derive(Clone, Copy)]
struct Pole {
    center: IrregularPoint,
    radius: f64,
}

impl Triangle {
    fn new(points: [IrregularPoint; 3]) -> Self {
        let bounds = IrregularBounds::new(
            points
                .iter()
                .map(|point| point.x)
                .fold(f64::INFINITY, f64::min),
            points
                .iter()
                .map(|point| point.y)
                .fold(f64::INFINITY, f64::min),
            points
                .iter()
                .map(|point| point.x)
                .fold(f64::NEG_INFINITY, f64::max),
            points
                .iter()
                .map(|point| point.y)
                .fold(f64::NEG_INFINITY, f64::max),
        );
        Self { points, bounds }
    }
}

/// One cell's separating axes, precomputed for the *first* operand of a
/// surrogate pair test.
///
/// [`triangle_penetration`] is called from the proxy collider with the first
/// triangle at a zero translation, so everything it derives from that triangle
/// alone - its three edge normals, and its own extent along each of them - is a
/// function of the cell and nothing else. It was being re-derived on every one
/// of the 80.4M narrow-phase tests a mode-22 stream runs, and the three
/// `hypot` calls it needs are 68.6% of that stream's axis work.
///
/// Every field is the bit pattern the deriving path produced. `points` is the
/// triangle after the `+ 0.0` the collider's translation applies (`x + 0.0` is
/// `x` for every `x` except `-0.0`, so the normalisation is reproduced rather
/// than assumed away), the edges are taken from those points in the same
/// order, and `self_min`/`self_max` are that triangle's own projection onto its
/// own axis. A `degenerate` edge is one whose length was exactly zero, which is
/// the case the deriving path answered with `None`.
#[derive(Clone, Copy)]
struct CellAxes {
    points: [IrregularPoint; 3],
    edges: [CellEdge; 3],
}

#[derive(Clone, Copy)]
struct CellEdge {
    axis_x: f64,
    axis_y: f64,
    self_min: f64,
    self_max: f64,
    degenerate: bool,
}

impl CellAxes {
    /// Derives one cell's axes exactly as [`triangle_penetration`] would, for a
    /// first operand translated by `(0.0, 0.0)`.
    fn new(cell: Triangle) -> Self {
        let points = cell
            .points
            .map(|point| IrregularPoint::new(point.x + 0.0, point.y + 0.0));
        let edges = std::array::from_fn(|index| {
            let edge_x = points[(index + 1) % 3].x - points[index].x;
            let edge_y = points[(index + 1) % 3].y - points[index].y;
            let length = proxy_hypot(edge_x, edge_y);
            if length == 0.0 {
                return CellEdge {
                    axis_x: 0.0,
                    axis_y: 0.0,
                    self_min: 0.0,
                    self_max: 0.0,
                    degenerate: true,
                };
            }
            let axis_x = -edge_y / length;
            let axis_y = edge_x / length;
            let (self_min, self_max) = project_triangle(&points, axis_x, axis_y);
            CellEdge {
                axis_x,
                axis_y,
                self_min,
                self_max,
                degenerate: false,
            }
        });
        Self { points, edges }
    }
}

/// A bin grid over one shape's cells, stored as one bitmask per bin.
///
/// The membership lists used to be `Vec<Vec<usize>>`, and a query walked every
/// list in the covered bin rectangle setting one bit per entry. On a mode-22
/// stream that was 1.54 *billion* pointer-chased iterations to produce 157M
/// distinct bits, because a cell sits in every bin its extent touches and a
/// query covers 9.4 bins on average. The same answer is the OR of the bins'
/// precomputed masks, which is the identical bit set - `|` is idempotent and
/// commutative, so neither the duplication nor the visit order was ever
/// observable - read from one contiguous array.
///
/// `words` is `ceil(cells / 64)`, and it is what a query zeroes and a caller
/// scans. The declared cap is [`MAX_CELLS_PER_PIECE`] = 512, but real surrogates
/// carry four or five cells, so the previous fixed eight-word mask spent seven
/// eighths of its zeroing and scanning on words that no cell can ever occupy.
#[derive(Clone)]
struct CellIndex {
    bounds: IrregularBounds,
    /// `CELL_INDEX_SIDE * CELL_INDEX_SIDE * words` masks, bin-major.
    bin_masks: Vec<u64>,
    words: usize,
    /// The bin spans [`bin_range`] derives from `bounds`, hoisted out of the
    /// query: same expressions, same values, evaluated once per shape.
    span_x: f64,
    span_y: f64,
}

struct PieceIndex {
    bounds: IrregularBounds,
    bins: Vec<Vec<usize>>,
}

impl PieceIndex {
    fn new(bounds: IrregularBounds) -> Self {
        Self {
            bounds,
            bins: vec![Vec::new(); PIECE_INDEX_SIDE * PIECE_INDEX_SIDE],
        }
    }

    fn insert(&mut self, piece_index: usize, bounds: IrregularBounds) {
        let (min_x, max_x, min_y, max_y) = bin_range(bounds, self.bounds, PIECE_INDEX_SIDE);
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                self.bins[y * PIECE_INDEX_SIDE + x].push(piece_index);
            }
        }
    }

    fn remove(&mut self, piece_index: usize, bounds: IrregularBounds) {
        let (min_x, max_x, min_y, max_y) = bin_range(bounds, self.bounds, PIECE_INDEX_SIDE);
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                if let Some(position) = self.bins[y * PIECE_INDEX_SIDE + x]
                    .iter()
                    .position(|candidate| *candidate == piece_index)
                {
                    self.bins[y * PIECE_INDEX_SIDE + x].swap_remove(position);
                }
            }
        }
    }

    fn query_into(&self, bounds: IrregularBounds, scratch: &mut PieceQueryScratch) {
        let (min_x, max_x, min_y, max_y) = bin_range(bounds, self.bounds, PIECE_INDEX_SIDE);
        scratch.begin_query();
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                for piece_index in self.bins[y * PIECE_INDEX_SIDE + x].iter().copied() {
                    scratch.report(piece_index);
                }
            }
        }
        scratch.selected.sort_unstable();
    }
}

struct PieceQueryScratch {
    marks: Vec<u32>,
    generation: u32,
    selected: Vec<usize>,
}

impl PieceQueryScratch {
    fn new(piece_count: usize) -> Self {
        Self {
            marks: vec![0; piece_count],
            generation: 0,
            selected: Vec::with_capacity(piece_count),
        }
    }

    fn begin_query(&mut self) {
        self.selected.clear();
        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            self.marks.fill(0);
            self.generation = 1;
        }
    }

    fn report(&mut self, piece_index: usize) {
        if self.marks[piece_index] != self.generation {
            self.marks[piece_index] = self.generation;
            self.selected.push(piece_index);
        }
    }
}

/// The pose a cached proxy row was derived at, compared by bit pattern.
///
/// Bits rather than values, deliberately. The derivation runs the pose through
/// [`continuous_angle`], so two distinct `rotation_deg` bit patterns can produce
/// the same transform; keying on the raw bits can therefore only ever cause an
/// unnecessary recomputation, never a stale read. A key that compared canonical
/// angles would have to prove the canonicalisation is injective on every input
/// the search generates, which is a much stronger claim than this cache needs.
#[derive(Clone, Copy, PartialEq, Eq)]
struct ProxyRowPose {
    rotation_bits: u64,
    translate_x_bits: u64,
    translate_y_bits: u64,
    mirrored: bool,
}

impl ProxyRowPose {
    #[inline]
    fn of(placement: &RelaxedPlacement) -> Self {
        Self {
            rotation_bits: placement.rotation_deg.to_bits(),
            translate_x_bits: placement.translate_x.to_bits(),
            translate_y_bits: placement.translate_y.to_bits(),
            mirrored: placement.mirrored,
        }
    }
}

/// Dense per-piece proxy geometry, maintained one row at a time.
///
/// The confirmation collider ([`continuous_pair_collision`]) opens with a
/// broad-phase reject against the two operands' transformed surrogate extents,
/// and it used to derive both of them from scratch on every call — a walk over
/// every cell vertex of both shapes. Asking one piece about all of its
/// neighbours therefore re-derived the *same* extent for that piece once per
/// neighbour, and a whole-layout score re-derived every piece's extent `n - 1`
/// times.
///
/// This is the row storage that stops it. One entry per piece holds the pose
/// the extent was taken at and the extent itself; a lookup that finds the same
/// pose returns the stored extent, and a lookup that does not re-derives one
/// row. A sweep that moves one piece therefore invalidates exactly one row,
/// which is what delta scoring means for this quantity.
///
/// The cache is *self-invalidating*: it stores the pose alongside the extent
/// and checks it on every read, so no call site has to remember to evict, and
/// correctness does not depend on the search announcing a move. A candidate
/// pose that is scored and then rejected simply leaves a row that the next
/// reader recomputes.
struct ProxyRowCache {
    poses: Vec<Option<ProxyRowPose>>,
    bounds: Vec<IrregularBounds>,
}

impl ProxyRowCache {
    fn new(piece_count: usize) -> Self {
        Self {
            poses: vec![None; piece_count],
            bounds: vec![IrregularBounds::new(0.0, 0.0, 0.0, 0.0); piece_count],
        }
    }

    /// The transformed proxy extent of `placement`, derived once per pose.
    ///
    /// `shape` must be the same zero-degree confirmation surrogate the collider
    /// would have used, which is what makes the stored extent bit-identical to
    /// the one the deriving path computes.
    #[inline]
    fn bounds_for(
        &mut self,
        shape: &OrientedSurrogate,
        placement: &RelaxedPlacement,
    ) -> IrregularBounds {
        let transform = || {
            PoleTransform::new(
                placement.rotation_deg,
                placement.translate_x,
                placement.translate_y,
            )
        };
        let Some(slot) = self.poses.get_mut(placement.input_index) else {
            return transformed_surrogate_bounds(shape, transform());
        };
        let pose = ProxyRowPose::of(placement);
        if *slot == Some(pose) {
            return self.bounds[placement.input_index];
        }
        let bounds = transformed_surrogate_bounds(shape, transform());
        *slot = Some(pose);
        self.bounds[placement.input_index] = bounds;
        bounds
    }
}

/// The rotation half of a [`SurrogateKey`], memoised one slot per piece.
///
/// Deriving it is pure arithmetic, but the arithmetic is three `rem_euclid`
/// calls - three out-of-line `fmod`s, none of which the compiler can hoist -
/// and the proxy collider derives it for *two* poses on every pair question it
/// is asked, which is 52.0M questions on a mode-22 stream.
///
/// The poses it is asked about barely change. A candidate scan holds one
/// candidate pose fixed across every neighbour it queries, and a fixed piece's
/// pose changes only when a move is accepted, so one slot per piece answers
/// almost every question without touching `fmod`.
///
/// Keyed on the rotation's *bit pattern*, like [`ProxyRowPose`] and for the
/// same reason: the derivation canonicalises, so two distinct bit patterns can
/// share an answer, and comparing bits can therefore only cause an unnecessary
/// recomputation, never a stale read. The value is whatever the deriving
/// expression produced, so a hit and a miss are indistinguishable in the
/// result.
struct AngleKeyCache {
    entries: Vec<Option<(u64, i64)>>,
}

impl AngleKeyCache {
    fn new(piece_count: usize) -> Self {
        Self {
            entries: vec![None; piece_count],
        }
    }

    /// The rotation key of `rotation_deg`, from the slot when the piece was
    /// last asked about the same bits and from the deriving expression
    /// otherwise.
    ///
    /// `directional` is the lane's `uses_directional_pressure()`, which is
    /// constant for a lane; it selects the same branch the deriving path takes.
    #[inline(always)]
    fn rotation_key(&mut self, input_index: usize, rotation_deg: f64, directional: bool) -> i64 {
        let bits = rotation_deg.to_bits();
        if let Some(Some((stored_bits, key))) = self.entries.get(input_index).copied() {
            if stored_bits == bits {
                return key;
            }
        }
        let key = derive_rotation_key(rotation_deg, directional);
        if let Some(slot) = self.entries.get_mut(input_index) {
            *slot = Some((bits, key));
        }
        key
    }
}

/// The error a missing canonical orientation raises, verbatim.
///
/// A free function so a caller holding a catalogue borrow can raise it without
/// re-borrowing the whole lane.
fn missing_orientation_error(
    pieces: &[GeneralFastPiece<'_>],
    input_index: usize,
    key: SurrogateKey,
) -> GeneralFastError {
    GeneralPolygonError::from_message(format!(
        "relaxed surrogate catalog is missing canonical orientation {} for piece {}",
        angle_from_key(key.1),
        pieces[input_index].id
    ))
    .into()
}

/// The vector length the *proxy* tier measures with.
///
/// Every caller is a ranking or pruning question - a pole-pair separation, a
/// separating-axis normalisation - and none of them can reach a published
/// placement, which is why this knob exists at all and why it is confined to
/// this function's callers.
///
/// The default is the platform's `hypot`, and the default is what every
/// regression gate and every published number in this repository was measured
/// against.
///
/// # Why the flag, and what it costs
///
/// After the collider's precomputation, `hypot` *is* the proxy tier: a mode-22
/// stream runs 410M pole-pair separations and 83M axis normalisations through
/// it, and a measured 7.7 ns/call against 1.3 ns/call for `sqrt(x*x + y*y)`
/// says it is essentially all of what `pairPressure` costs. It is also the one
/// thing here that cannot be made faster *and* bit-identical: the platform's
/// `hypot` is correctly rounded, the naive form is not, and a 5M-sample probe
/// at the magnitudes this engine works at puts them one unit in the last place
/// apart on 16.9% of inputs. `libm`'s Rust port is not a way out either - it is
/// 3.7x faster than the platform call and disagrees with it on 13.9% of the
/// same inputs.
///
/// What that costs in *outcome* is a separate question from what it costs in
/// arithmetic, and it was measured rather than assumed. On the three pinned
/// regression streams the flagged build reproduces the unflagged one exactly -
/// mode 20 at `independentDepthMm` 206.869 and fingerprint `8a7737381238fa4d`,
/// the two mode-22 record replays at raw 159.09233022733062 and
/// 159.08263749731248 at `fa01012af1d559ae` and `145d0ed4b2f53d3f`, all
/// `exactValid` and `contractValid` - and so do the mode-26 ladder and the
/// mode-31 arm, failure-reason text included, while the mode-22 stream runs
/// 3.2 s against 3.7 s.
///
/// That was *evidence*, not a guarantee, and it has since been checked on the
/// corpus it asked for. The result is that it stays off permanently, not
/// provisionally.
///
/// # The corpus verdict: do not promote
///
/// Twenty-two streams - eight mode-20 runs over shapes-17 and triangle-20 at
/// two sheets and two seeds each, plus the four Mixed-61 gates, four mode-31
/// arms and six mode-26 ladder arms - were run under both builds and compared
/// as whole documents rather than as published fields, against a same-binary
/// determinism control that is clean on all 22.
///
/// The published outcome reproduces on 22 of 22. The whole document reproduces
/// on 2 of 22: under the flag the relaxed search takes a different path, with
/// last-place differences in `rawPenalty`, `weightedPressure` and
/// `weightedLoss` propagating into different accepted moves (32,317 against
/// 32,288 on one shapes-17 stream) and different evaluation counts. That is
/// the predicted mechanism, observed.
///
/// On three of the arms it reaches a reported layout: the coupled dynamic
/// separator's boundary projection treatment finishes at `finalDepthMm`
/// 179.931 under the flag against 179.810 without it, with a different
/// placement fingerprint. A 0.121 mm regression on a reported depth is not a
/// tie-break that happened to fall the other way; it is the flag choosing a
/// worse layout, and it disproves the outcome-neutrality that promotion would
/// have rested on.
///
/// So this stays a *measurement* instrument - the cheapest way to price the
/// proxy tier's length - and must not acquire a default, a settings knob, or a
/// coordinator that can reach it. The naive form additionally loses `hypot`'s
/// overflow and underflow guards; at millimetre magnitudes on a sheet that is
/// unreachable, but it is another reason this cannot be the default.
///
/// The route that remains open is a *certified* one: the platform call is
/// correctly rounded (verified exactly, in rational arithmetic, on 200,000
/// real pole-pair arguments with zero failures), so a faster length that
/// carries a proof of correct rounding is bit-identical by specification and
/// needs no corpus at all. A double-double fast path with one Newton
/// correction measures 3.08 ns/call against the platform's 8.04 with FMA
/// enabled, and disagrees with it on 0.558% of real inputs - which is the
/// population such a certificate has to detect and hand back to the platform
/// call. See the pole-loop chapter of `docs/next-generation-engine-plan.md`.
#[inline(always)]
fn proxy_hypot(x: f64, y: f64) -> f64 {
    #[cfg(feature = "fast-proxy-hypot")]
    {
        (x * x + y * y).sqrt()
    }
    #[cfg(not(feature = "fast-proxy-hypot"))]
    {
        x.hypot(y)
    }
}

/// The rotation half of a [`SurrogateKey`], derived.
fn derive_rotation_key(rotation_deg: f64, directional: bool) -> i64 {
    let angle = if directional {
        continuous_angle(rotation_deg)
    } else {
        canonical_angle(rotation_deg)
    };
    angle_key(angle)
}

impl CellIndex {
    fn new(cells: &[Triangle], bounds: IrregularBounds) -> Self {
        let words = cells.len().div_ceil(64).max(1);
        let mut bin_masks = vec![0_u64; CELL_INDEX_SIDE * CELL_INDEX_SIDE * words];
        for (cell_index, cell) in cells.iter().enumerate() {
            let (min_x, max_x, min_y, max_y) = cell_bin_range(cell.bounds, bounds);
            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    bin_masks[(y * CELL_INDEX_SIDE + x) * words + cell_index / 64] |=
                        1_u64 << (cell_index % 64);
                }
            }
        }
        let (span_x, span_y) = bin_spans(bounds);
        Self {
            bounds,
            bin_masks,
            words,
            span_x,
            span_y,
        }
    }

    /// Writes the covered cells' bitmask into `selected` and returns how many
    /// words of it are live.
    ///
    /// The bit set is the one the membership-list walk produced: a cell's bit is
    /// set exactly when the query rectangle covers a bin the cell was inserted
    /// into. Only the first `words` words are written, and only those are
    /// meaningful; the caller scans the same prefix.
    #[inline(always)]
    fn query_mask_into(
        &self,
        bounds: IrregularBounds,
        translate_x: f64,
        translate_y: f64,
        selected: &mut [u64; MAX_CELLS_PER_PIECE / 64],
    ) -> usize {
        let local = IrregularBounds::new(
            bounds.min_x - translate_x,
            bounds.min_y - translate_y,
            bounds.max_x - translate_x,
            bounds.max_y - translate_y,
        );
        let (min_x, max_x, min_y, max_y) =
            bin_range_within(local, self.bounds, self.span_x, self.span_y, CELL_INDEX_SIDE);
        // One word covers every surrogate the catalogue actually builds; the
        // general loop stays for the declared 512-cell ceiling.
        if self.words == 1 {
            let mut accumulated = 0_u64;
            for y in min_y..=max_y {
                let row = y * CELL_INDEX_SIDE;
                for x in min_x..=max_x {
                    accumulated |= self.bin_masks[row + x];
                }
            }
            selected[0] = accumulated;
            return 1;
        }
        let words = self.words;
        selected[..words].fill(0);
        for y in min_y..=max_y {
            let row = y * CELL_INDEX_SIDE;
            for x in min_x..=max_x {
                let base = (row + x) * words;
                for (word, mask) in selected[..words]
                    .iter_mut()
                    .zip(&self.bin_masks[base..base + words])
                {
                    *word |= *mask;
                }
            }
        }
        words
    }
}

/// One piece geometry at one canonical orientation, as the exploration tier
/// sees it: the exact expanded collision ring, its triangulation, the poles the
/// pressure proxy scores, and a bin index over the cells.
///
/// This is [`LegacyKernel`](crate::search::kernel::LegacyKernel)'s
/// [`ExplorationKernel::Shape`](crate::search::kernel::ExplorationKernel::Shape),
/// which is why it is `pub` at all. Every field stays private, so the type is
/// opaque outside this module and every consumer still goes through a function
/// here.
#[derive(Clone)]
pub struct OrientedSurrogate {
    collision: PolygonSet,
    cells: Vec<Triangle>,
    /// One [`CellAxes`] per entry of `cells`, in the same order.
    cell_axes: Vec<CellAxes>,
    poles: Vec<Pole>,
    bounds: IrregularBounds,
    cell_index: CellIndex,
    difficulty: f64,
    diameter: f64,
}

struct SurrogateCatalog {
    geometry_class_by_input: Vec<usize>,
    orientations: BTreeMap<SurrogateKey, OrientedSurrogate>,
    shared_pair_nfps: BTreeMap<PairNfpKey, Arc<PairNfp>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SurrogateCatalogMode {
    StructuredGrid,
    CurrentAssignment,
    ZeroDegreeOnly,
}

#[derive(Clone)]
struct RelaxedPlacement {
    input_index: usize,
    rotation_deg: f64,
    mirrored: bool,
    translate_x: f64,
    translate_y: f64,
}

#[derive(Clone)]
struct RelaxedState {
    placements: Vec<RelaxedPlacement>,
    strip_depth_mm: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GridInnerFit {
    min_x: i128,
    max_x: i128,
    min_y: i128,
    max_y: i128,
}

impl GridInnerFit {
    fn contains(self, x: i128, y: i128) -> bool {
        x >= self.min_x && x <= self.max_x && y >= self.min_y && y <= self.max_y
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct WorkCounters {
    oriented_surrogate_builds: usize,
    generated_cells: usize,
    ejection_chain_evaluations: usize,
    ejection_chain_accepts: usize,
    surrogate_evaluations: usize,
    piece_broad_phase_probes: usize,
    cell_index_probes: usize,
    sat_tests: usize,
    pair_nfp_builds: usize,
    pair_nfp_components: usize,
    shared_pair_nfp_entries: usize,
    shared_pair_nfp_components: usize,
    shared_pair_nfp_estimated_bytes: usize,
    shared_pair_nfp_adoptions: usize,
    directional_pair_evaluations: usize,
    directional_exact_confirmations: usize,
    directional_cache_hits: usize,
    directional_cache_misses: usize,
    directional_component_visits: usize,
    directional_intervals_produced: usize,
    directional_intervals_merged: usize,
    directional_over_budget_candidates: usize,
    directional_zero_penetration_inconsistencies: usize,
    directional_lane_rejections: usize,
    directional_relocations: usize,
    directional_rejected_contractions: usize,
    directional_containment_rejections: usize,
    directional_initial_pair_loss: GeneralRelaxedLossDistribution,
    directional_initial_boundary_loss: GeneralRelaxedLossDistribution,
    directional_accepted_pair_loss: GeneralRelaxedLossDistribution,
    directional_accepted_boundary_loss: GeneralRelaxedLossDistribution,
    axis_events: usize,
    axis_candidate_evaluations: usize,
    dynamic_hazard_queries: usize,
    dynamic_hazard_updates: usize,
    dynamic_pressure_evaluations: usize,
    dynamic_layout_loads: usize,
    dynamic_index_builds: usize,
    translation_evaluations: usize,
    rotation_evaluations: usize,
    retained_f64_confirmations: usize,
    confirmed_pair_additions: usize,
    confirmed_pair_removals: usize,
    accepted_moves: usize,
    angular_repair_successors: usize,
    angular_repair_improvements: usize,
    angular_repair_queries: usize,
}

impl WorkCounters {
    fn accumulate(&mut self, other: Self) {
        self.oriented_surrogate_builds = self
            .oriented_surrogate_builds
            .saturating_add(other.oriented_surrogate_builds);
        self.generated_cells = self.generated_cells.saturating_add(other.generated_cells);
        self.ejection_chain_evaluations = self
            .ejection_chain_evaluations
            .saturating_add(other.ejection_chain_evaluations);
        self.ejection_chain_accepts = self
            .ejection_chain_accepts
            .saturating_add(other.ejection_chain_accepts);
        self.surrogate_evaluations = self
            .surrogate_evaluations
            .saturating_add(other.surrogate_evaluations);
        self.piece_broad_phase_probes = self
            .piece_broad_phase_probes
            .saturating_add(other.piece_broad_phase_probes);
        self.cell_index_probes = self
            .cell_index_probes
            .saturating_add(other.cell_index_probes);
        self.sat_tests = self.sat_tests.saturating_add(other.sat_tests);
        self.pair_nfp_builds = self.pair_nfp_builds.saturating_add(other.pair_nfp_builds);
        self.pair_nfp_components = self
            .pair_nfp_components
            .saturating_add(other.pair_nfp_components);
        self.shared_pair_nfp_entries = self
            .shared_pair_nfp_entries
            .saturating_add(other.shared_pair_nfp_entries);
        self.shared_pair_nfp_components = self
            .shared_pair_nfp_components
            .saturating_add(other.shared_pair_nfp_components);
        self.shared_pair_nfp_estimated_bytes = self
            .shared_pair_nfp_estimated_bytes
            .saturating_add(other.shared_pair_nfp_estimated_bytes);
        self.shared_pair_nfp_adoptions = self
            .shared_pair_nfp_adoptions
            .saturating_add(other.shared_pair_nfp_adoptions);
        self.directional_pair_evaluations = self
            .directional_pair_evaluations
            .saturating_add(other.directional_pair_evaluations);
        self.directional_exact_confirmations = self
            .directional_exact_confirmations
            .saturating_add(other.directional_exact_confirmations);
        self.directional_cache_hits = self
            .directional_cache_hits
            .saturating_add(other.directional_cache_hits);
        self.directional_cache_misses = self
            .directional_cache_misses
            .saturating_add(other.directional_cache_misses);
        self.directional_component_visits = self
            .directional_component_visits
            .saturating_add(other.directional_component_visits);
        self.directional_intervals_produced = self
            .directional_intervals_produced
            .saturating_add(other.directional_intervals_produced);
        self.directional_intervals_merged = self
            .directional_intervals_merged
            .saturating_add(other.directional_intervals_merged);
        self.directional_over_budget_candidates = self
            .directional_over_budget_candidates
            .saturating_add(other.directional_over_budget_candidates);
        self.directional_zero_penetration_inconsistencies = self
            .directional_zero_penetration_inconsistencies
            .saturating_add(other.directional_zero_penetration_inconsistencies);
        self.directional_lane_rejections = self
            .directional_lane_rejections
            .saturating_add(other.directional_lane_rejections);
        self.directional_relocations = self
            .directional_relocations
            .saturating_add(other.directional_relocations);
        self.directional_rejected_contractions = self
            .directional_rejected_contractions
            .saturating_add(other.directional_rejected_contractions);
        self.directional_containment_rejections = self
            .directional_containment_rejections
            .saturating_add(other.directional_containment_rejections);
        self.directional_initial_pair_loss
            .merge(other.directional_initial_pair_loss);
        self.directional_initial_boundary_loss
            .merge(other.directional_initial_boundary_loss);
        self.directional_accepted_pair_loss
            .merge(other.directional_accepted_pair_loss);
        self.directional_accepted_boundary_loss
            .merge(other.directional_accepted_boundary_loss);
        self.axis_events = self.axis_events.saturating_add(other.axis_events);
        self.axis_candidate_evaluations = self
            .axis_candidate_evaluations
            .saturating_add(other.axis_candidate_evaluations);
        self.dynamic_hazard_queries = self
            .dynamic_hazard_queries
            .saturating_add(other.dynamic_hazard_queries);
        self.dynamic_hazard_updates = self
            .dynamic_hazard_updates
            .saturating_add(other.dynamic_hazard_updates);
        self.dynamic_pressure_evaluations = self
            .dynamic_pressure_evaluations
            .saturating_add(other.dynamic_pressure_evaluations);
        self.dynamic_layout_loads = self
            .dynamic_layout_loads
            .saturating_add(other.dynamic_layout_loads);
        self.dynamic_index_builds = self
            .dynamic_index_builds
            .saturating_add(other.dynamic_index_builds);
        self.translation_evaluations = self
            .translation_evaluations
            .saturating_add(other.translation_evaluations);
        self.rotation_evaluations = self
            .rotation_evaluations
            .saturating_add(other.rotation_evaluations);
        self.retained_f64_confirmations = self
            .retained_f64_confirmations
            .saturating_add(other.retained_f64_confirmations);
        self.confirmed_pair_additions = self
            .confirmed_pair_additions
            .saturating_add(other.confirmed_pair_additions);
        self.confirmed_pair_removals = self
            .confirmed_pair_removals
            .saturating_add(other.confirmed_pair_removals);
        self.accepted_moves = self.accepted_moves.saturating_add(other.accepted_moves);
        self.angular_repair_successors = self
            .angular_repair_successors
            .saturating_add(other.angular_repair_successors);
        self.angular_repair_improvements = self
            .angular_repair_improvements
            .saturating_add(other.angular_repair_improvements);
        self.angular_repair_queries = self
            .angular_repair_queries
            .saturating_add(other.angular_repair_queries);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PairEntry {
    raw_loss: f64,
    guided_weight: f64,
    normalization_scale: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct BoundaryEntry {
    violations: usize,
    raw_loss: f64,
}

#[derive(Clone, Debug, PartialEq)]
struct PairTracker {
    piece_count: usize,
    boundaries: Vec<BoundaryEntry>,
    pairs: Vec<PairEntry>,
    incident_raw_loss: Vec<f64>,
    boundary_violations: usize,
    boundary_loss: f64,
    collision_pairs: Vec<(usize, usize, f64)>,
    weighted_loss: f64,
}

impl PairTracker {
    fn feasible(&self) -> bool {
        self.boundary_violations == 0 && self.collision_pairs.is_empty()
    }

    fn common_loss(&self) -> f64 {
        self.boundary_loss
            + self
                .collision_pairs
                .iter()
                .map(|(_, _, penalty)| *penalty)
                .sum::<f64>()
    }

    fn pair(&self, first: usize, second: usize) -> PairEntry {
        self.pairs[pair_slot(self.piece_count, first, second)]
    }

    fn replace_pair(&mut self, first: usize, second: usize, raw_loss: f64, guided_weight: f64) {
        let slot = pair_slot(self.piece_count, first, second);
        let old = self.pair(first, second);
        let normalized_old = old.raw_loss / old.normalization_scale;
        let normalization_scale = 1.0;
        let normalized_new = raw_loss / normalization_scale;
        self.incident_raw_loss[first] =
            (self.incident_raw_loss[first] - normalized_old + normalized_new).max(0.0);
        self.incident_raw_loss[second] =
            (self.incident_raw_loss[second] - normalized_old + normalized_new).max(0.0);
        self.pairs[slot] = PairEntry {
            raw_loss,
            guided_weight,
            normalization_scale,
        };
    }

    fn replace_boundary(&mut self, index: usize, boundary: BoundaryEntry) {
        self.boundaries[index] = boundary;
    }
}

/// What one moved piece's query produces, and what the incremental tracker
/// installs.
///
/// A sweep asks one question per candidate pose — *if this piece went here,
/// what would its rows be?* — and this is the whole answer: the piece's own
/// boundary term, and the row it owns against every partner it collides with.
/// [`update_score_after_move`] consumes exactly this and nothing else, which is
/// what makes an accepted move cost the moved row rather than the layout.
///
/// It is named for what it is because the name was load-bearing and missing.
/// Sol's fourth finding asks for a moved-piece query returning
/// `Pruned | Complete<MovedRowDelta>`; the hazard index already answers in that
/// shape ([`GeneralHazardQuery`]), and this is the lane-level counterpart that
/// the shape was missing.
///
/// # Rows are keyed by the index-ordered pair
///
/// Every producer builds its keys through [`ordered_pair`] and sorts by
/// `(first, second)` before returning, so a row is named `(lower, higher)`
/// whichever of the two pieces was the one that moved. That is not a formatting
/// convention. It is the row-ownership decision, and the ledger chapter "Who
/// owns a row" is where it was forced rather than chosen: a tracker row has to
/// be a measurement of the *layout* if a sweep is ever to inherit one instead
/// of rescoring, and `(moving, fixed)` is a function of the path taken to a
/// layout rather than of the layout.
///
/// The key is index-ordered unconditionally, and it always was. What is *not*
/// unconditional is the order the two operands are presented to the proxy
/// kernel in — the proxy is not symmetric in them, and only
/// `canonical-pair-order` makes the asking order agree with the key. So this
/// type carries the half of the decision that is free, and the flag carries the
/// half that costs a trajectory. See [`canonical_pair_operands`].
///
/// The sortedness is load-bearing twice: [`update_score_after_move`] walks the
/// row with a single cursor against an ascending pair sequence, and
/// [`sorted_pair_difference_counts`] merges two of these rather than building
/// two `BTreeSet`s.
#[derive(Clone, Debug)]
struct MovedRowDelta {
    boundary_violations: usize,
    boundary_loss: f64,
    /// The moved piece's rows, sorted by index-ordered pair key, each pair once.
    collision_pairs: Vec<(usize, usize, f64)>,
    weighted_loss: f64,
    /// Whether `collision_pairs` is every colliding partner. See [`MovedRows`].
    rows: MovedRows,
}

/// Whether a [`MovedRowDelta`]'s row set is all of the moved piece's rows.
///
/// This is the `Pruned | Complete` distinction Sol's fourth finding asks a
/// moved-piece query to make, at the lane's level. It is a marker on the delta
/// rather than an enum wrapping one because an incomplete answer still carries
/// a usable boundary term and a usable loss — a scan that stopped early has
/// already established that the candidate is worse than the bound, which is the
/// only thing the caller wanted from it. What it does not carry is a row set a
/// tracker may install.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MovedRows {
    /// Every partner the candidate collides with is present.
    Complete,
    /// The partner scan stopped early: the running weighted loss had already
    /// passed the bound the caller supplied, so the remaining partners were
    /// never asked.
    ///
    /// Such a delta is never installed into the tracker, and the reason is an
    /// ordering identity rather than a check. The bound a scan prunes against
    /// is exactly the `weighted_loss` of the candidate it would have to beat,
    /// and both comparators — [`compare_score_objective`] and
    /// [`compare_move_score`] — order on `weighted_loss` first. So a pruned
    /// delta compares strictly worse than the thing it was measured against, at
    /// both sites that retain one: [`refine_candidate`] keeps a candidate only
    /// when it compares `!= Greater`, and [`report_diverse_sample`] either
    /// returns early or sorts the pruned sample past the truncation point. The
    /// `debug_assert` in [`update_score_after_move`] is what checks that the
    /// identity still holds rather than merely being argued for.
    PrunedAtBound,
    /// No partner scan ran at all: the lane's dynamic query budget was spent,
    /// or the pose was rejected before the scan.
    ///
    /// The row set is empty and the loss is infinite. This is deliberately
    /// *not* merged into `PrunedAtBound`, because the argument that keeps it
    /// out of the tracker is weaker: it relies on the incumbent it is compared
    /// against having a finite loss, not on an ordering identity, so it is
    /// documented rather than asserted. It arises only on the dynamic-hazard
    /// backend, which no default path binds.
    Unscanned,
}

/// The row-set state of a partner scan that ran, given whether it stopped at
/// the caller's bound.
#[inline(always)]
fn moved_rows(pruned: bool) -> MovedRows {
    if pruned {
        MovedRows::PrunedAtBound
    } else {
        MovedRows::Complete
    }
}

#[derive(Clone)]
struct LaneOutcome {
    state: RelaxedState,
    score: PairTracker,
    weights: BTreeMap<(usize, usize), f64>,
    counters: WorkCounters,
    selected_lane: usize,
    restart_disruptions: usize,
}

struct LaneBatch {
    outcomes: Vec<LaneOutcome>,
    counters: WorkCounters,
}

enum ExactLaneValidation {
    Infeasible,
    Accepted {
        placements: Vec<GeneralFastPlacement>,
        metrics: GeneralPlacementMetrics,
    },
    Rejected,
}

struct SelectedLane {
    outcome: LaneOutcome,
    validation: ExactLaneValidation,
}

#[derive(Clone)]
struct EjectionCandidate {
    replacements: Vec<(usize, RelaxedPlacement)>,
    score: PairTracker,
}

struct RepairExperimentOutcome {
    selected: Option<LaneOutcome>,
    counters: WorkCounters,
    control_loss: f64,
    rotation_loss: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CoupledSeparatorArm {
    Control,
    Treatment,
}

/// How a coupled-separator arm compares its incremental rollback tracker
/// against the complete rescore that authorises a rollback.
///
/// The two readings of one collision pair are the *same measurement* taken
/// from opposite sides. [`JaguaHazardIndex::collision_pressure`] sums an `f32`
/// pole-pair series over a freshly transformed scratch polygon for the moving
/// piece and the committed layout shape for the fixed one; swapping the roles
/// swaps which side comes from which pipeline and reverses the summation
/// order. The incremental tracker keeps whichever reading the *last moved*
/// piece produced, while a complete rescore always reads a pair from its
/// lower-indexed piece, so the two can differ in the low `f32` bits of a value
/// that is mathematically identical.
///
/// `Exact` demands bitwise equality anyway, which is how every arm outside the
/// mode-26 sheet clamp has always behaved and is what keeps those modes'
/// accepted states bit-identical. It is the default, and it is deliberately
/// *not* something a caller can turn off globally: the variant is chosen at
/// each arm invocation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum CoupledRollbackComparison {
    /// Bitwise equality on every field of the tracker. Any low-bit asymmetry
    /// aborts the contraction target.
    #[default]
    Exact,
    /// Structure exactly, magnitudes to within the pole-pressure rounding
    /// floor: piece counts, row counts, pair indices and violation counts must
    /// still match bit for bit, but two loss magnitudes that agree to within
    /// [`COUPLED_ROLLBACK_PRESSURE_ULP_BUDGET`] `f32` units in the last place
    /// are treated as the same reading.
    ///
    /// Only the mode-26 clamped-sheet ladder runs its arms this way. Tolerated
    /// disagreements are counted into the ladder diagnostics rather than
    /// swallowed.
    ToleratesPoleRounding,
}

impl CoupledRollbackComparison {
    /// The diagnostics label for a non-default comparison. `Exact` has no
    /// label, so an arm that never opted in serializes exactly as it did
    /// before the policy existed.
    fn label(self) -> Option<&'static str> {
        match self {
            Self::Exact => None,
            Self::ToleratesPoleRounding => Some("toleratesPoleRounding"),
        }
    }
}

#[cfg(feature = "jagua-experimental")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CoupledTerminalPolicy {
    None,
    ExactBoundaryProjection,
}

#[cfg(feature = "jagua-experimental")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CoupledRollbackRescorePolicy {
    StrictDerivedAgreement,
    CanonicalAuthoritativeRows,
}

impl CoupledSeparatorArm {
    #[cfg(feature = "jagua-experimental")]
    fn pressure_model(self) -> GeneralRelaxedPressureModel {
        GeneralRelaxedPressureModel::DynamicPoles
    }

    #[cfg(feature = "jagua-experimental")]
    fn refines_rotation(self) -> bool {
        self == Self::Treatment
    }

    fn label(self) -> &'static str {
        match self {
            Self::Control => "dynamicCoveragePolesTranslationOnly",
            Self::Treatment => "dynamicCoveragePolesRigidDescent",
        }
    }
}

#[cfg(feature = "jagua-experimental")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RawMinimumTransition {
    NoImprovement,
    MinorImprovement,
    SubstantialImprovement,
}

#[cfg(feature = "jagua-experimental")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CoupledRoundDisposition {
    AcceptFeasible,
    ContinueInfeasible(RawMinimumTransition),
}

#[cfg(feature = "jagua-experimental")]
#[derive(Clone)]
struct CoupledMinimumCheckpoint {
    state: RelaxedState,
    score: PairTracker,
}

#[cfg(feature = "jagua-experimental")]
struct CoupledTargetOutcome {
    diagnostics: GeneralCoupledSeparatorTargetDiagnostics,
    accepted: Option<GeneralFastResult>,
    work: CoupledSeparatorWork,
    minimum: Option<CoupledMinimumCheckpoint>,
    final_state: RelaxedState,
    exact_metrics: Option<GeneralPlacementMetrics>,
    independent_audit: Option<CoupledIndependentAuditOutcome>,
}

#[cfg(feature = "jagua-experimental")]
struct CoupledIndependentAuditOutcome {
    diagnostics: GeneralPrecompressionIndependentAuditDiagnostics,
    metrics: Option<GeneralPlacementMetrics>,
}

#[cfg(feature = "jagua-experimental")]
#[derive(Clone)]
struct CoupledFailedCheckpoint {
    incumbent: GeneralFastResult,
    target_ordinal: usize,
    target_depth_mm: f64,
    compression_split_mm: f64,
    target_seed: u64,
    compression_seed: u64,
    catalog: Arc<SurrogateCatalog>,
    hazard_catalog: Arc<JaguaHazardCatalog>,
    minimum: CoupledMinimumCheckpoint,
    attempt_diagnostics: GeneralCoupledSeparatorTargetDiagnostics,
}

#[cfg(feature = "jagua-experimental")]
#[derive(Clone)]
struct CoupledArmOutcome {
    diagnostics: GeneralCoupledSeparatorArmDiagnostics,
    checkpoint: Option<CoupledFailedCheckpoint>,
}

#[cfg(feature = "jagua-experimental")]
#[derive(Clone, Copy, Debug)]
struct ConflictRuinExactScore {
    total_overlap_area_mm2: f64,
    positive_overlap_pairs: usize,
    maximum_pair_area_mm2: f64,
    frontier_depth_mm: f64,
}

#[cfg(feature = "jagua-experimental")]
#[derive(Clone)]
struct ConflictRuinBeamState {
    state: RelaxedState,
    active: Vec<bool>,
    collisions: Vec<Option<PolygonSet>>,
    score: ConflictRuinExactScore,
}

#[cfg(feature = "jagua-experimental")]
struct ConflictRuinBuildOutcome {
    beam: Vec<ConflictRuinBeamState>,
    initial_score: ConflictRuinExactScore,
}

#[cfg(feature = "jagua-experimental")]
#[derive(Clone)]
struct ConflictRuinCandidate {
    placement: RelaxedPlacement,
    proxy_loss: f64,
}

#[cfg(feature = "jagua-experimental")]
struct PrecompressionExactParentCandidate {
    compressed: RelaxedState,
    metrics: GeneralPlacementMetrics,
    compressed_raw_loss: f64,
    frontier_depth_mm: f64,
    fingerprint: String,
}

#[cfg(feature = "jagua-experimental")]
struct PrecompressionInfeasibleChild {
    state: RelaxedState,
    fresh_raw_loss: f64,
    fresh_positive_pairs: usize,
    beam_ordinal: usize,
    fingerprint: String,
}

#[cfg(feature = "jagua-experimental")]
#[derive(Clone, Copy, Debug, Default)]
struct ConflictRuinWork {
    orientation_build_limit: usize,
    pair_intersection_limit: usize,
    parent_orientation_streams: usize,
    cheap_queries: usize,
    exact_finalists: usize,
    exact_pair_intersections: usize,
    required_current_finalists: usize,
    orientation_builds: usize,
    transformed_output_vertices: usize,
    feature_visits: usize,
    pre_dedup_contact_attempts: usize,
    deduplicated_proposals: usize,
    clipper_input_vertices: usize,
    clipper_output_vertices: usize,
    partials_retained: usize,
}

#[cfg(feature = "jagua-experimental")]
impl ConflictRuinWork {
    fn for_piece_count(piece_count: usize) -> Self {
        let projected_roots = piece_count.min(CONFLICT_RUIN_REMOVED_PIECES);
        let selector_pairs = projected_roots
            .saturating_mul(piece_count.saturating_sub(1))
            .saturating_sub(projected_roots.saturating_mul(projected_roots.saturating_sub(1)) / 2);
        Self {
            orientation_build_limit: piece_count
                .saturating_mul(2)
                .saturating_add(CONFLICT_RUIN_STREAM_CAP)
                .saturating_add(CONFLICT_RUIN_FINALIST_CAP),
            pair_intersection_limit: piece_count
                .saturating_mul(piece_count.saturating_sub(1))
                .saturating_div(2)
                .saturating_add(selector_pairs)
                .saturating_add(
                    CONFLICT_RUIN_FINALIST_CAP.saturating_mul(piece_count.saturating_sub(1)),
                ),
            ..Self::default()
        }
    }
}

#[cfg(feature = "jagua-experimental")]
struct ConflictRuinBoundaryProbe {
    root: usize,
    placement: RelaxedPlacement,
    blockers: Vec<(usize, f64)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CoordinateAxis {
    Horizontal,
    Vertical,
    ForwardDiagonal,
    BackwardDiagonal,
    Rotation,
}

/// One relaxed-search lane.
///
/// `K` is the [`ExplorationKernel`] the lane's proxy geometry runs on. It
/// defaults to [`LegacyKernel`], which is what every production route and every
/// existing `LaneSearch<'_>` mention resolves to, so a build that does not opt
/// into another kernel monomorphises to exactly one instantiation and the
/// generic parameter costs nothing.
///
/// The bound is `Shape = OrientedSurrogate` for now: PR3 opens the query seam,
/// while the catalogue that owns the oriented shapes is still concrete. See
/// [`crate::search::kernel`] for what that does and does not allow to be
/// swapped.
/// The lane search every production route runs: the legacy geometry kernel.
///
/// Constructor calls name this alias rather than `LaneSearch` so that the
/// kernel a lane runs on is written down at every construction site instead of
/// being inferred. Swapping a kernel in is then one alias, not an audit of
/// which lanes were left on the default.
type LegacyLaneSearch<'a> = LaneSearch<'a, LegacyKernel>;

struct LaneSearch<'a, K: ExplorationKernel<Shape = OrientedSurrogate> = LegacyKernel> {
    pieces: &'a [GeneralFastPiece<'a>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    catalog: Arc<SurrogateCatalog>,
    /// The geometric services this lane's proxy tier runs on.
    kernel: K,
    rng: SplitMix64,
    weights: BTreeMap<(usize, usize), f64>,
    counters: WorkCounters,
    allow_worsening_chain: bool,
    piece_query_scratch: PieceQueryScratch,
    /// Per-piece proxy extents, kept in step with the layout one row per move.
    proxy_rows: ProxyRowCache,
    /// Per-piece memo of the rotation half of a [`SurrogateKey`].
    angle_keys: AngleKeyCache,
    /// Reusable buffer the accepted-move merge writes its new collision list
    /// into. It is swapped with the incumbent list rather than copied, so after
    /// the first move of a lane neither list allocates again.
    collision_merge_scratch: Vec<(usize, usize, f64)>,
    /// Recycled row buffers for [`MovedRowDelta::collision_pairs`].
    ///
    /// The candidate scorer builds one `Vec` per call for about 1.8 rows, and
    /// the refinement loop then drops two of them per iteration — the loser of
    /// the paired probe and the incumbent it displaced. This is where those two
    /// buffers go instead of back to the allocator, and where the next scan
    /// takes its buffer from. Nothing but the buffer's *capacity* differs, and
    /// no path reads a capacity, so the flag is bit-identical by construction.
    ///
    /// Compiled out entirely when `relaxed-row-buffer-reuse` is off, so the
    /// default build has neither the field nor the `pop`.
    #[cfg(feature = "relaxed-row-buffer-reuse")]
    row_pool: Vec<Vec<(usize, usize, f64)>>,
    /// Keying buffer for `relaxed-scan-order-proxy`'s neighbour reordering.
    ///
    /// Unconditional, unlike [`Self::row_pool`], because the reordering is
    /// reached through a helper that both bodies of
    /// [`Self::score_placement`] call and only one of the two can name a lane
    /// field after the destructure. It is never written when the flag is off —
    /// an empty `Vec` allocates nothing — and the default build reproduces all
    /// four gates as whole documents with it in place.
    scan_order_scratch: Vec<(f64, usize)>,
    /// Whether the move the shadow-rescore audit is about to inspect was a
    /// *revert* — a dynamic candidate the objective judged worse than the
    /// incumbent, whose row is reinstalled out of the tracker rather than
    /// measured. Diagnostic only, and compiled out with the audit.
    #[cfg(feature = "shadow-rescore")]
    audit_move_was_revert: bool,
    /// The row [`Self::confirm_dynamic_replacement`] last produced, and the
    /// piece it was produced for. Diagnostic only: it lets the audit tell a row
    /// that was measured wrongly from a row that was measured rightly and then
    /// lost on the way into the tracker.
    #[cfg(feature = "shadow-rescore")]
    audit_last_confirmed_row: Option<(usize, Vec<(usize, usize, f64)>)>,
    pair_nfp_cache: BTreeMap<PairNfpKey, Arc<PairNfp>>,
    pair_nfp_cache_components: usize,
    #[cfg(feature = "jagua-experimental")]
    hazard_index: Option<JaguaHazardIndex>,
    #[cfg(feature = "jagua-experimental")]
    hazard_catalog: Option<Arc<JaguaHazardCatalog>>,
    dynamic_query_limit: Option<usize>,
    refine_rotation: bool,
    /// The lane's depth clock, when one is armed.
    ///
    /// This is the missing piece (a) the mode-26 anatomy identified:
    /// `RelaxedState.strip_depth_mm` is written in exactly five non-test
    /// places and every one of them is a *whole-pipeline* decision, so nothing
    /// owned the depth per sweep. This does. It is a lane field rather than a
    /// state field on purpose - a state can be cloned, projected and restored
    /// by paths that predate the schedule, and the floor has to survive all of
    /// them.
    ///
    /// Compiled out entirely when `compression-schedule` is off, so the
    /// default build has neither the field nor the branch that reads it.
    #[cfg(feature = "compression-schedule")]
    compression: Option<CompressionSchedule>,
}

type SurrogateKey = (usize, i64, bool);
type PairNfpKey = (usize, i64, bool, usize, i64, bool);

#[derive(Clone, PartialEq)]
struct ConvexNfp {
    points: Vec<IrregularPoint>,
    bounds: IrregularBounds,
}

#[derive(Clone, PartialEq)]
struct PairNfp {
    components: Vec<ConvexNfp>,
}

struct PairAxisIntervals {
    nfp_key: PairNfpKey,
    fixed_translate_x: f64,
    fixed_translate_y: f64,
    guided_weight: f64,
    normalization_scale: f64,
    intervals: Vec<(f64, f64)>,
}

#[derive(Clone, Copy, Debug)]
struct GridDirectionalPenetration {
    horizontal_grid: i64,
    vertical_grid: i64,
    horizontal_intervals: usize,
    vertical_intervals: usize,
}

impl GridDirectionalPenetration {
    fn penetration_mm(self) -> Option<f64> {
        let penetration = self.horizontal_grid.min(self.vertical_grid);
        (penetration > 0).then(|| from_grid(penetration as f64))
    }
}

pub fn improve_complete_layout(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    incumbent: &GeneralFastResult,
) -> Result<GeneralRelaxedOutcome, GeneralFastError> {
    improve_complete_layout_with_pinned_vacancy_parent(
        pieces,
        fast_settings,
        relaxed_settings,
        incumbent,
        None,
        None,
    )
}

/// `secondary_pinned_vacancy_parent` supplies the second-parent fixture (the
/// warm-start slot) that mode 23 (recombination) crosses with
/// `pinned_vacancy_parent`. Every other mode ignores it.
pub fn improve_complete_layout_with_pinned_vacancy_parent(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    incumbent: &GeneralFastResult,
    pinned_vacancy_parent: Option<&GeneralPersistentVacancyPinnedParent>,
    secondary_pinned_vacancy_parent: Option<&GeneralPersistentVacancyPinnedParent>,
) -> Result<GeneralRelaxedOutcome, GeneralFastError> {
    improve_complete_layout_under_rollback_comparison(
        pieces,
        fast_settings,
        relaxed_settings,
        incumbent,
        pinned_vacancy_parent,
        secondary_pinned_vacancy_parent,
        CoupledRollbackComparison::Exact,
    )
}

/// The relaxed entry point with the coupled separator's rollback comparison
/// policy made explicit.
///
/// Every public caller goes through
/// [`improve_complete_layout_with_pinned_vacancy_parent`], which pins
/// [`CoupledRollbackComparison::Exact`]; the policy is a parameter rather than
/// a setting precisely so that no configuration can widen it for a mode that
/// did not ask. The one caller that asks is the mode-26 ladder, whose rungs run
/// under their own sheet clamp.
pub(crate) fn improve_complete_layout_under_rollback_comparison(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    incumbent: &GeneralFastResult,
    pinned_vacancy_parent: Option<&GeneralPersistentVacancyPinnedParent>,
    secondary_pinned_vacancy_parent: Option<&GeneralPersistentVacancyPinnedParent>,
    rollback_comparison: CoupledRollbackComparison,
) -> Result<GeneralRelaxedOutcome, GeneralFastError> {
    validate_relaxed_settings(relaxed_settings)?;
    let mut diagnostics = GeneralRelaxedDiagnostics::default();
    if pieces.is_empty() {
        diagnostics.skipped_reason = Some("relaxed search requires at least one piece".to_owned());
        return Ok(relaxed_outcome(
            pieces,
            fast_settings,
            incumbent.clone(),
            diagnostics,
        ));
    }
    if pieces.iter().any(|piece| {
        piece
            .polygon
            .regions()
            .iter()
            .any(|region| !region.holes.is_empty())
    }) {
        diagnostics.skipped_reason =
            Some("relaxed search does not yet flatten hole topology".to_owned());
        if relaxed_settings.coupled_dynamic_separator {
            diagnostics.coupled_dynamic_separator = Some(run_coupled_dynamic_separator_experiment(
                pieces,
                fast_settings,
                relaxed_settings,
                incumbent,
                pinned_vacancy_parent,
                secondary_pinned_vacancy_parent,
                rollback_comparison,
            ));
        }
        return Ok(relaxed_outcome(
            pieces,
            fast_settings,
            incumbent.clone(),
            diagnostics,
        ));
    }

    let catalog_mode = if relaxed_settings.pressure_model
        == GeneralRelaxedPressureModel::DirectionalPenetration
    {
        SurrogateCatalogMode::CurrentAssignment
    } else if relaxed_settings.collision_backend == GeneralRelaxedCollisionBackend::RollbackTriangle
        || matches!(
            relaxed_settings.pressure_model,
            GeneralRelaxedPressureModel::StructuredTrianglePoles
        )
    {
        SurrogateCatalogMode::StructuredGrid
    } else {
        SurrogateCatalogMode::ZeroDegreeOnly
    };
    let (catalog, catalog_work) =
        match build_surrogate_catalog(pieces, fast_settings, catalog_mode, Some(incumbent)) {
            Ok(catalog) => catalog,
            Err(GeneralFastError::Geometry(error))
                if error.message().contains("relaxed surrogate") =>
            {
                diagnostics.skipped_reason = Some(error.to_string());
                return Ok(relaxed_outcome(
                    pieces,
                    fast_settings,
                    incumbent.clone(),
                    diagnostics,
                ));
            }
            Err(error) => return Err(error),
        };
    diagnostics.oriented_surrogate_builds = catalog_work.oriented_surrogate_builds;
    diagnostics.generated_cells = catalog_work.generated_cells;
    diagnostics.shared_pair_nfp_entries = catalog_work.shared_pair_nfp_entries;
    diagnostics.shared_pair_nfp_components = catalog_work.shared_pair_nfp_components;
    diagnostics.shared_pair_nfp_estimated_bytes = catalog_work.shared_pair_nfp_estimated_bytes;
    let mut protected = incumbent.clone();
    let mut working = initialize_complete_state(
        pieces,
        fast_settings,
        relaxed_settings.collision_backend,
        relaxed_settings.angle_seed_policy,
        relaxed_settings.pressure_model,
        incumbent,
    )?;
    let mut shrink_ratio = relaxed_settings.initial_shrink_ratio;
    let mut repair_successors_attempted = 0usize;
    // The incumbent the constructor handed over is the curve's origin: without
    // it a reader cannot tell a search that started at 206 mm from one that
    // started at 180 mm, and the first improvement's delta is unattributable.
    #[cfg(feature = "quality-trace")]
    if quality_trace::active()
        && !protected.placements.is_empty()
        && protected.unplaced_piece_ids.is_empty()
    {
        quality_trace::incumbent(
            protected.used_long_axis_depth_mm,
            protected.placements.len(),
            &general_placement_fingerprint(&protected.placements),
            "constructor",
        );
    }
    for epoch in 0..relaxed_settings.epochs {
        #[cfg(feature = "quality-trace")]
        let _trace_epoch = quality_trace::scope(
            format!("m0.epoch{epoch}"),
            relaxed_settings.seed,
            None,
        );
        diagnostics.epochs_attempted += 1;
        let incumbent_depth_before_mm = protected.used_long_axis_depth_mm;
        let protected_depth = protected
            .used_long_axis_depth_mm
            .max(working.strip_depth_mm);
        let target_depth = (protected.used_long_axis_depth_mm * (1.0 - shrink_ratio))
            .max(area_depth_lower_bound(pieces, fast_settings)?);
        let attempt_state = working.clone();
        let lane_result = if relaxed_settings.synchronize_lanes {
            run_synchronized_lanes(
                pieces,
                fast_settings,
                relaxed_settings,
                &attempt_state,
                target_depth,
                epoch,
                catalog.clone(),
            )
            .map(|lane| LaneBatch {
                counters: lane.counters,
                outcomes: vec![lane],
            })
        } else {
            run_independent_lanes(
                pieces,
                fast_settings,
                relaxed_settings,
                &attempt_state,
                target_depth,
                epoch,
                catalog.clone(),
            )
        };
        let batch = match lane_result {
            Ok(batch) => batch,
            Err(error) if is_directional_lane_unscorable(&error) => {
                diagnostics.directional_lane_rejections = diagnostics
                    .directional_lane_rejections
                    .saturating_add(relaxed_settings.lanes);
                diagnostics.skipped_reason = Some(error.to_string());
                return Ok(GeneralRelaxedOutcome {
                    result: protected,
                    diagnostics,
                });
            }
            Err(GeneralFastError::Geometry(error))
                if error.message().contains("relaxed surrogate") =>
            {
                diagnostics.skipped_reason = Some(error.to_string());
                return Ok(GeneralRelaxedOutcome {
                    result: protected,
                    diagnostics,
                });
            }
            Err(error) => return Err(error),
        };
        let mut selected =
            select_lane_for_publication(pieces, fast_settings, batch.outcomes, &mut diagnostics);
        selected.outcome.counters = batch.counters;
        let mut lane = selected.outcome;
        let mut exact_validation = selected.validation;
        if !lane.score.feasible()
            && repair_successors_attempted < relaxed_settings.angular_repair.successors
            && relaxed_settings.angular_repair.complete_query_budget > 0
        {
            let experiment = run_bounded_repair_experiment(
                pieces,
                fast_settings,
                relaxed_settings,
                &lane,
                epoch,
                catalog.clone(),
            )?;
            repair_successors_attempted = repair_successors_attempted.saturating_add(1);
            diagnostics.angular_repair_base_loss = Some(lane.score.common_loss());
            diagnostics.angular_repair_control_loss = Some(experiment.control_loss);
            diagnostics.angular_repair_rotation_loss = Some(experiment.rotation_loss);
            lane.counters.accumulate(experiment.counters);
            if let Some(selected) = experiment.selected {
                lane.state = selected.state;
                lane.score = selected.score;
                lane.weights = selected.weights;
                exact_validation =
                    validate_selected_lane(pieces, fast_settings, &lane, &mut diagnostics);
            }
        }
        diagnostics.ejection_chain_evaluations = diagnostics
            .ejection_chain_evaluations
            .saturating_add(lane.counters.ejection_chain_evaluations);
        diagnostics.ejection_chain_accepts = diagnostics
            .ejection_chain_accepts
            .saturating_add(lane.counters.ejection_chain_accepts);
        diagnostics.surrogate_evaluations = diagnostics
            .surrogate_evaluations
            .saturating_add(lane.counters.surrogate_evaluations);
        diagnostics.piece_broad_phase_probes = diagnostics
            .piece_broad_phase_probes
            .saturating_add(lane.counters.piece_broad_phase_probes);
        diagnostics.cell_index_probes = diagnostics
            .cell_index_probes
            .saturating_add(lane.counters.cell_index_probes);
        diagnostics.sat_tests = diagnostics
            .sat_tests
            .saturating_add(lane.counters.sat_tests);
        diagnostics.pair_nfp_builds = diagnostics
            .pair_nfp_builds
            .saturating_add(lane.counters.pair_nfp_builds);
        diagnostics.pair_nfp_components = diagnostics
            .pair_nfp_components
            .saturating_add(lane.counters.pair_nfp_components);
        diagnostics.shared_pair_nfp_adoptions = diagnostics
            .shared_pair_nfp_adoptions
            .saturating_add(lane.counters.shared_pair_nfp_adoptions);
        diagnostics.directional_pair_evaluations = diagnostics
            .directional_pair_evaluations
            .saturating_add(lane.counters.directional_pair_evaluations);
        diagnostics.directional_exact_confirmations = diagnostics
            .directional_exact_confirmations
            .saturating_add(lane.counters.directional_exact_confirmations);
        diagnostics.directional_cache_hits = diagnostics
            .directional_cache_hits
            .saturating_add(lane.counters.directional_cache_hits);
        diagnostics.directional_cache_misses = diagnostics
            .directional_cache_misses
            .saturating_add(lane.counters.directional_cache_misses);
        diagnostics.directional_component_visits = diagnostics
            .directional_component_visits
            .saturating_add(lane.counters.directional_component_visits);
        diagnostics.directional_intervals_produced = diagnostics
            .directional_intervals_produced
            .saturating_add(lane.counters.directional_intervals_produced);
        diagnostics.directional_intervals_merged = diagnostics
            .directional_intervals_merged
            .saturating_add(lane.counters.directional_intervals_merged);
        diagnostics.directional_over_budget_candidates = diagnostics
            .directional_over_budget_candidates
            .saturating_add(lane.counters.directional_over_budget_candidates);
        diagnostics.directional_zero_penetration_inconsistencies = diagnostics
            .directional_zero_penetration_inconsistencies
            .saturating_add(lane.counters.directional_zero_penetration_inconsistencies);
        diagnostics.directional_lane_rejections = diagnostics
            .directional_lane_rejections
            .saturating_add(lane.counters.directional_lane_rejections);
        diagnostics.directional_relocations = diagnostics
            .directional_relocations
            .saturating_add(lane.counters.directional_relocations);
        diagnostics.directional_rejected_contractions = diagnostics
            .directional_rejected_contractions
            .saturating_add(lane.counters.directional_rejected_contractions);
        diagnostics.directional_containment_rejections = diagnostics
            .directional_containment_rejections
            .saturating_add(lane.counters.directional_containment_rejections);
        diagnostics
            .directional_initial_pair_loss
            .merge(lane.counters.directional_initial_pair_loss);
        diagnostics
            .directional_initial_boundary_loss
            .merge(lane.counters.directional_initial_boundary_loss);
        diagnostics
            .directional_accepted_pair_loss
            .merge(lane.counters.directional_accepted_pair_loss);
        diagnostics
            .directional_accepted_boundary_loss
            .merge(lane.counters.directional_accepted_boundary_loss);
        diagnostics.axis_events = diagnostics
            .axis_events
            .saturating_add(lane.counters.axis_events);
        diagnostics.axis_candidate_evaluations = diagnostics
            .axis_candidate_evaluations
            .saturating_add(lane.counters.axis_candidate_evaluations);
        diagnostics.dynamic_hazard_queries = diagnostics
            .dynamic_hazard_queries
            .saturating_add(lane.counters.dynamic_hazard_queries);
        diagnostics.dynamic_hazard_updates = diagnostics
            .dynamic_hazard_updates
            .saturating_add(lane.counters.dynamic_hazard_updates);
        diagnostics.dynamic_pressure_evaluations = diagnostics
            .dynamic_pressure_evaluations
            .saturating_add(lane.counters.dynamic_pressure_evaluations);
        diagnostics.translation_evaluations = diagnostics
            .translation_evaluations
            .saturating_add(lane.counters.translation_evaluations);
        diagnostics.rotation_evaluations = diagnostics
            .rotation_evaluations
            .saturating_add(lane.counters.rotation_evaluations);
        diagnostics.retained_f64_confirmations = diagnostics
            .retained_f64_confirmations
            .saturating_add(lane.counters.retained_f64_confirmations);
        diagnostics.confirmed_pair_additions = diagnostics
            .confirmed_pair_additions
            .saturating_add(lane.counters.confirmed_pair_additions);
        diagnostics.confirmed_pair_removals = diagnostics
            .confirmed_pair_removals
            .saturating_add(lane.counters.confirmed_pair_removals);
        diagnostics.accepted_moves = diagnostics
            .accepted_moves
            .saturating_add(lane.counters.accepted_moves);
        diagnostics.angular_repair_successors = diagnostics
            .angular_repair_successors
            .saturating_add(lane.counters.angular_repair_successors);
        diagnostics.angular_repair_improvements = diagnostics
            .angular_repair_improvements
            .saturating_add(lane.counters.angular_repair_improvements);
        diagnostics.angular_repair_queries = diagnostics
            .angular_repair_queries
            .saturating_add(lane.counters.angular_repair_queries);
        let mut exact_valid = false;
        let mut exact_accepted = false;
        let mut retain_lane_state = false;
        if lane.score.feasible() {
            match exact_validation {
                ExactLaneValidation::Accepted {
                    placements,
                    metrics,
                } if placements.len() > protected.placements.len()
                    || (placements.len() == protected.placements.len()
                        && metrics.used_long_axis_depth_mm < protected.used_long_axis_depth_mm) =>
                {
                    exact_valid = true;
                    retain_lane_state = true;
                    protected.placements = placements;
                    protected.unplaced_piece_ids.clear();
                    protected.used_short_axis_span_mm = metrics.used_short_axis_span_mm;
                    protected.used_long_axis_depth_mm = metrics.used_long_axis_depth_mm;
                    protected.unused_short_axis_projection_mm =
                        metrics.unused_short_axis_projection_mm;
                    protected.occupied_envelope_area_mm2 = metrics.occupied_envelope_area_mm2;
                    working.strip_depth_mm = metrics.used_long_axis_depth_mm;
                    diagnostics.epochs_improved += 1;
                    exact_accepted = true;
                    shrink_ratio = relaxed_settings.initial_shrink_ratio;
                    // The public-incumbent improvement: one point on the curve.
                    #[cfg(feature = "quality-trace")]
                    if quality_trace::active() {
                        quality_trace::incumbent(
                            protected.used_long_axis_depth_mm,
                            protected.placements.len(),
                            &general_placement_fingerprint(&protected.placements),
                            "m0.relaxedEpoch",
                        );
                    }
                }
                ExactLaneValidation::Accepted { .. } => {
                    exact_valid = true;
                    retain_lane_state = true;
                    diagnostics.exact_valid_non_improvements =
                        diagnostics.exact_valid_non_improvements.saturating_add(1);
                    shrink_ratio = (shrink_ratio * 0.5).max(relaxed_settings.minimum_shrink_ratio);
                }
                ExactLaneValidation::Rejected | ExactLaneValidation::Infeasible => {
                    shrink_ratio = (shrink_ratio * 0.5).max(relaxed_settings.minimum_shrink_ratio);
                }
            }
        } else {
            shrink_ratio = (shrink_ratio * 0.75).max(relaxed_settings.minimum_shrink_ratio);
        }
        working = if retain_lane_state {
            lane.state.clone()
        } else {
            initialize_complete_state(
                pieces,
                fast_settings,
                relaxed_settings.collision_backend,
                relaxed_settings.angle_seed_policy,
                relaxed_settings.pressure_model,
                &protected,
            )?
        };
        diagnostics.epochs.push(GeneralRelaxedEpochDiagnostics {
            epoch,
            selected_lane: lane.selected_lane,
            restart_disruptions: lane.restart_disruptions,
            target_depth_mm: target_depth,
            weighted_loss: lane.score.weighted_loss,
            collision_pairs: lane.score.collision_pairs.len(),
            blocking_pairs: blocking_pair_diagnostics(pieces, &lane.score, &lane.weights),
            boundary_violations: lane.score.boundary_violations,
            boundary_piece_ids: lane
                .score
                .boundaries
                .iter()
                .enumerate()
                .filter(|(_, boundary)| boundary.violations > 0)
                .map(|(index, _)| pieces[index].id.to_owned())
                .collect(),
            surrogate_feasible: lane.score.feasible(),
            exact_valid,
            exact_accepted,
            translation_evaluations: lane.counters.translation_evaluations,
            rotation_evaluations: lane.counters.rotation_evaluations,
            complete_queries: lane.counters.dynamic_hazard_queries,
            retained_f64_confirmations: lane.counters.retained_f64_confirmations,
            accepted_moves: lane.counters.accepted_moves,
            incumbent_depth_before_mm,
            incumbent_depth_after_mm: protected.used_long_axis_depth_mm,
            incumbent_depth_delta_mm: incumbent_depth_before_mm - protected.used_long_axis_depth_mm,
        });
        if protected_depth <= protected.used_long_axis_depth_mm
            && shrink_ratio <= relaxed_settings.minimum_shrink_ratio
        {
            break;
        }
    }
    if relaxed_settings.coupled_dynamic_separator {
        diagnostics.coupled_dynamic_separator = Some(run_coupled_dynamic_separator_experiment(
            pieces,
            fast_settings,
            relaxed_settings,
            &protected,
            pinned_vacancy_parent,
            secondary_pinned_vacancy_parent,
            rollback_comparison,
        ));
    }
    Ok(relaxed_outcome(
        pieces,
        fast_settings,
        protected,
        diagnostics,
    ))
}

/// The single exit of the relaxed entry point, and therefore the single place
/// where a search mode's publication can become the engine's own result.
///
/// Every `return` in [`improve_complete_layout_under_rollback_comparison`] goes
/// through here, so no mode needs - or gets - its own copy of the adoption
/// rule. `legacy` is the result that entry point would have returned before
/// this function existed: the protected constructor/relaxed incumbent.
///
/// See [`adopt_published_layout`] for the rule. When no mode ran, that function
/// returns `legacy` untouched without measuring anything, so a run with the
/// persistent-vacancy arms off is byte-identical to the pre-adoption engine.
fn relaxed_outcome(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    legacy: GeneralFastResult,
    diagnostics: GeneralRelaxedDiagnostics,
) -> GeneralRelaxedOutcome {
    #[cfg(feature = "quality-trace")]
    let _trace = quality_trace::scope("publication".to_owned(), 0, None);
    let result = adopt_published_layout(pieces, fast_settings, &diagnostics, legacy);
    GeneralRelaxedOutcome {
        result,
        diagnostics,
    }
}

/// The placements a search mode published in its own right, if it published.
///
/// Every mode - 20 and 21, the 22-31 band, and any mode added later - reports
/// through the one `persistentVacancyPopulation` block and writes its published
/// layout into that block's `final_placements`. Reading the block rather than
/// each mode's own diagnostics is what makes the adoption rule uniform: a new
/// mode is routed the moment it fills that field, and no mode can be forgotten.
///
/// Deliberately *not* [`persistent_vacancy_reported_layout`]: that function
/// falls back to the parent layout so that a declining arm's report still
/// describes something. An empty `final_placements` is precisely a refusal -
/// the mode published nothing of its own - and a refusal must leave the legacy
/// result standing, so no fallback belongs here.
#[cfg(feature = "jagua-experimental")]
fn published_mode_placements(
    diagnostics: &GeneralRelaxedDiagnostics,
) -> Option<Vec<GeneralFastPlacement>> {
    let population = diagnostics
        .coupled_dynamic_separator
        .as_ref()?
        .persistent_vacancy_population
        .as_ref()?;
    (!population.final_placements.is_empty())
        .then(|| fast_placements_from_coupled_diagnostics(&population.final_placements))
}

/// Records why the adoption rule declined a mode's publication.
///
/// The review's second finding was that "every adoption rejection silently
/// returns legacy; production telemetry cannot distinguish incomplete,
/// invalid, envelope-only rejection, or non-improvement". Under the trace the
/// four refusals are distinct named events; without it this function has no
/// body and the rule is byte-identical to the one that shipped.
#[cfg(feature = "quality-trace")]
#[allow(dead_code)]
fn trace_publication_refusal(published_depth_mm: f64, legacy_depth_mm: f64, reason: &str) {
    if quality_trace::active() {
        quality_trace::publication(
            quality_trace::Disposition::Discarded,
            published_depth_mm,
            legacy_depth_mm,
            reason,
        );
    }
}

/// Records why the adoption rule declined a publication. Compiled out.
#[cfg(not(feature = "quality-trace"))]
#[allow(dead_code)]
#[inline(always)]
fn trace_publication_refusal(_published_depth_mm: f64, _legacy_depth_mm: f64, _reason: &str) {}

/// Adopts a mode's publication as the engine's result when, and only when, it
/// is both legal against the real request and strictly better than the legacy
/// incumbent.
///
/// The rule, in the order it is applied:
///
/// 1. A mode must have published a layout of its own. No mode run, or a mode
///    that refused, leaves `legacy` untouched.
/// 2. The publication must be *complete*: one placement per requested piece.
///    Neither validator enforces that on its own, and a partial layout would
///    measure shallower than the complete one it is being compared against.
/// 3. It must beat `legacy` on raw source depth - the untouched `f64` reading
///    that cannot round, measured the same way on both sides. A legacy result
///    that failed to place every piece is not a comparable reading and never
///    wins. Ties keep `legacy`, so adoption is a strict improvement.
/// 4. Publication authority stays where it already was: the composite exact
///    validator, re-run here against the *real* request rather than trusted
///    from the mode's own `exact_valid` flag. That flag is the mode's verdict
///    under the settings the mode searched in - a clamped sheet for a mode-26
///    rung - and mode 23 reports a layout together with a `false` verdict. Only
///    a layout that passes `validate_and_measure_placements` here, which is
///    `exactValid` and includes `contractValid`, is adopted.
///
/// The adopted result carries that validator's own measurements, so its depth,
/// span, projection and envelope area are in the same basis as the legacy
/// result's. Every other field - the constructor's evaluation counts and arm
/// outcomes - describes work that still happened and is carried over unchanged.
///
/// The diagnostics block is read, never written: it keeps recording what each
/// arm did whether or not the result moved.
#[cfg(feature = "jagua-experimental")]
fn adopt_published_layout(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    diagnostics: &GeneralRelaxedDiagnostics,
    legacy: GeneralFastResult,
) -> GeneralFastResult {
    let Some(published) = published_mode_placements(diagnostics) else {
        return legacy;
    };
    adopt_published_placements(pieces, fast_settings, published, legacy)
}

/// Steps 2-4 of the adoption rule, applied to placements a caller already has
/// in hand.
///
/// [`adopt_published_layout`] is step 1 - "did a mode publish anything?" - read
/// off the diagnostics DTO, and then this. The split exists so that the
/// portfolio coordinator, which holds an operator's typed outcome directly and
/// never reconstructs it from a DTO, publishes through the *same* comparator,
/// the same completeness check and the same composite validator as the
/// separator's own slot. A coordinator that re-implemented any of the three
/// would be inventing its own notion of validity, which is precisely the thing
/// this engine does not allow.
#[cfg(feature = "jagua-experimental")]
pub(super) fn adopt_published_placements(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    published: Vec<GeneralFastPlacement>,
    legacy: GeneralFastResult,
) -> GeneralFastResult {
    if published.len() != pieces.len() {
        trace_publication_refusal(f64::NAN, f64::NAN, "incompleteCardinality");
        return legacy;
    }
    let Ok(published_depth_mm) = coupled_raw_source_depth(pieces, &published, fast_settings) else {
        trace_publication_refusal(f64::NAN, f64::NAN, "publishedDepthUnmeasurable");
        return legacy;
    };
    let legacy_depth_mm = if legacy.unplaced_piece_ids.is_empty() {
        coupled_raw_source_depth(pieces, &legacy.placements, fast_settings).ok()
    } else {
        None
    };
    if legacy_depth_mm.is_some_and(|legacy_depth_mm| published_depth_mm >= legacy_depth_mm) {
        trace_publication_refusal(
            published_depth_mm,
            legacy_depth_mm.unwrap_or(f64::NAN),
            "notStrictlyBetterThanLegacy",
        );
        return legacy;
    }
    let Ok(metrics) = validate_and_measure_placements(pieces, &published, fast_settings) else {
        trace_publication_refusal(
            published_depth_mm,
            legacy_depth_mm.unwrap_or(f64::NAN),
            "compositeValidatorRejected",
        );
        return legacy;
    };
    #[cfg(feature = "quality-trace")]
    if quality_trace::active() {
        quality_trace::publication(
            quality_trace::Disposition::PublicIncumbent,
            published_depth_mm,
            legacy_depth_mm.unwrap_or(f64::NAN),
            "adopted",
        );
        quality_trace::incumbent(
            metrics.used_long_axis_depth_mm,
            published.len(),
            &general_placement_fingerprint(&published),
            "modeAdoption",
        );
    }
    GeneralFastResult {
        placements: published,
        unplaced_piece_ids: Vec::new(),
        used_short_axis_span_mm: metrics.used_short_axis_span_mm,
        used_long_axis_depth_mm: metrics.used_long_axis_depth_mm,
        unused_short_axis_projection_mm: metrics.unused_short_axis_projection_mm,
        occupied_envelope_area_mm2: metrics.occupied_envelope_area_mm2,
        ..legacy
    }
}

/// Without the experimental feature no search mode can run, so there is never a
/// publication to adopt and the legacy result is the only result.
#[cfg(not(feature = "jagua-experimental"))]
fn adopt_published_layout(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    diagnostics: &GeneralRelaxedDiagnostics,
    legacy: GeneralFastResult,
) -> GeneralFastResult {
    let _ = (pieces, fast_settings, diagnostics);
    legacy
}

fn run_bounded_repair_experiment<'a>(
    pieces: &'a [GeneralFastPiece<'a>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    base: &LaneOutcome,
    epoch: usize,
    catalog: Arc<SurrogateCatalog>,
) -> Result<RepairExperimentOutcome, GeneralFastError> {
    let total_budget = relaxed_settings.angular_repair.complete_query_budget;
    let control_budget = total_budget / 2;
    let rotation_budget = total_budget.saturating_sub(control_budget);
    let mut arm_settings = relaxed_settings;
    arm_settings.collision_backend = GeneralRelaxedCollisionBackend::DynamicHazard;
    arm_settings.angle_seed_policy = GeneralRelaxedAngleSeedPolicy::ContinuousUniform;
    arm_settings.pressure_model = GeneralRelaxedPressureModel::ContinuousTrianglePoles;
    arm_settings.angular_repair = GeneralAngularRepairSettings::disabled();

    let mut control = LegacyLaneSearch::new(
        pieces,
        fast_settings,
        arm_settings,
        derive_seed(relaxed_settings.seed, epoch, usize::MAX - 1),
        catalog.clone(),
    );
    control.weights = base.weights.clone();
    let control_outcome = control.run_repair_arm(
        base.state.clone(),
        false,
        relaxed_settings.angular_repair.neighborhood_size,
        control_budget,
        relaxed_settings.angular_repair.retained_confirmation_budget / 2,
    )?;

    let mut rotation = LegacyLaneSearch::new(
        pieces,
        fast_settings,
        arm_settings,
        derive_seed(relaxed_settings.seed, epoch, usize::MAX - 2),
        catalog,
    );
    rotation.weights = base.weights.clone();
    let rotation_outcome = rotation.run_repair_arm(
        base.state.clone(),
        true,
        relaxed_settings.angular_repair.neighborhood_size,
        rotation_budget,
        relaxed_settings
            .angular_repair
            .retained_confirmation_budget
            .saturating_sub(relaxed_settings.angular_repair.retained_confirmation_budget / 2),
    )?;

    let control_loss = control_outcome.score.common_loss();
    let rotation_loss = rotation_outcome.score.common_loss();
    let mut counters = WorkCounters::default();
    counters.accumulate(control_outcome.counters);
    counters.accumulate(rotation_outcome.counters);
    counters.angular_repair_successors = 1;
    counters.angular_repair_queries = counters.dynamic_hazard_queries;
    let best = if compare_lane_outcomes(0, &rotation_outcome, 1, &control_outcome) == Ordering::Less
    {
        rotation_outcome
    } else {
        control_outcome
    };
    let selected = if compare_chain_score(&best.score, &base.score) == Ordering::Less {
        counters.angular_repair_improvements = 1;
        Some(best)
    } else {
        None
    };
    Ok(RepairExperimentOutcome {
        selected,
        counters,
        control_loss,
        rotation_loss,
    })
}

fn run_coupled_dynamic_separator_experiment<'a>(
    pieces: &'a [GeneralFastPiece<'a>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    protected: &GeneralFastResult,
    pinned_vacancy_parent: Option<&GeneralPersistentVacancyPinnedParent>,
    secondary_pinned_vacancy_parent: Option<&GeneralPersistentVacancyPinnedParent>,
    rollback_comparison: CoupledRollbackComparison,
) -> GeneralCoupledSeparatorDiagnostics {
    let skipped = coupled_separator_configuration_error(relaxed_settings);
    if let Some(reason) = skipped {
        return GeneralCoupledSeparatorDiagnostics {
            seed_domain: COUPLED_SEPARATOR_SEED_DOMAIN,
            control: skipped_coupled_separator_arm(
                CoupledSeparatorArm::Control,
                protected,
                reason.clone(),
            ),
            treatment: skipped_coupled_separator_arm(
                CoupledSeparatorArm::Treatment,
                protected,
                reason,
            ),
            boundary_projection_treatment: None,
            conflict_ruin_recreate: None,
            precompression_frontier_vacancy: None,
            persistent_vacancy_population: None,
        };
    }

    #[cfg(feature = "jagua-experimental")]
    {
        let catalog = match build_surrogate_catalog(
            pieces,
            fast_settings,
            SurrogateCatalogMode::ZeroDegreeOnly,
            Some(protected),
        ) {
            Ok((catalog, _)) => catalog,
            Err(error) => {
                let reason = format!("confirmation catalog: {error}");
                return GeneralCoupledSeparatorDiagnostics {
                    seed_domain: COUPLED_SEPARATOR_SEED_DOMAIN,
                    control: skipped_coupled_separator_arm(
                        CoupledSeparatorArm::Control,
                        protected,
                        reason.clone(),
                    ),
                    treatment: skipped_coupled_separator_arm(
                        CoupledSeparatorArm::Treatment,
                        protected,
                        reason,
                    ),
                    boundary_projection_treatment: None,
                    conflict_ruin_recreate: None,
                    precompression_frontier_vacancy: None,
                    persistent_vacancy_population: None,
                };
            }
        };
        let control = {
            #[cfg(feature = "quality-trace")]
            let _trace = quality_trace::scope(
                "coupled.control".to_owned(),
                relaxed_settings.seed,
                None,
            );
            run_coupled_separator_arm(
                pieces,
                fast_settings,
                relaxed_settings,
                protected,
                CoupledSeparatorArm::Control,
                CoupledTerminalPolicy::None,
                rollback_comparison,
                catalog.clone(),
            )
        };
        let treatment = {
            #[cfg(feature = "quality-trace")]
            let _trace = quality_trace::scope(
                "coupled.treatment".to_owned(),
                relaxed_settings.seed,
                None,
            );
            run_coupled_separator_arm(
                pieces,
                fast_settings,
                relaxed_settings,
                protected,
                CoupledSeparatorArm::Treatment,
                CoupledTerminalPolicy::None,
                rollback_comparison,
                catalog.clone(),
            )
        };
        let boundary_projection_treatment = {
            #[cfg(feature = "quality-trace")]
            let _trace = quality_trace::scope(
                "coupled.boundaryProjection".to_owned(),
                relaxed_settings.seed,
                None,
            );
            run_coupled_separator_arm(
                pieces,
                fast_settings,
                relaxed_settings,
                protected,
                CoupledSeparatorArm::Treatment,
                CoupledTerminalPolicy::ExactBoundaryProjection,
                rollback_comparison,
                catalog,
            )
        };
        let conflict_ruin_recreate = Some(run_conflict_ruin_recreate_experiment(
            pieces,
            fast_settings,
            relaxed_settings,
            &control,
            &treatment,
        ));
        let precompression_frontier_vacancy =
            (relaxed_settings.precompression_frontier_vacancy_mode > 0).then(|| {
                run_precompression_frontier_vacancy_experiment(
                    pieces,
                    fast_settings,
                    relaxed_settings,
                    &boundary_projection_treatment,
                    relaxed_settings.precompression_frontier_vacancy_mode,
                )
            });
        let persistent_vacancy_population =
            (relaxed_settings.persistent_vacancy_mode > 0).then(|| {
                // A pinned fixture parent replaces only the parent-layout
                // source; the compiled-in frozen fingerprint, depth, and dual
                // validation checks still gate the arm.
                let pinned_arm =
                    pinned_vacancy_parent.map(|pinned| GeneralCoupledSeparatorArmDiagnostics {
                        final_placements: coupled_placement_diagnostics(&pinned.placements),
                        ..GeneralCoupledSeparatorArmDiagnostics::default()
                    });
                let parent_source = pinned_vacancy_parent.map(|pinned| {
                    format!("pinnedFixture:{}#{}", pinned.source, pinned.source_sha256)
                });
                let effective_parent = pinned_arm
                    .as_ref()
                    .unwrap_or(&boundary_projection_treatment.diagnostics);
                dispatch_persistent_vacancy_mode(
                    pieces,
                    fast_settings,
                    relaxed_settings,
                    effective_parent,
                    parent_source,
                    secondary_pinned_vacancy_parent,
                )
            });
        GeneralCoupledSeparatorDiagnostics {
            seed_domain: COUPLED_SEPARATOR_SEED_DOMAIN,
            control: control.diagnostics,
            treatment: treatment.diagnostics,
            boundary_projection_treatment: Some(boundary_projection_treatment.diagnostics),
            conflict_ruin_recreate,
            precompression_frontier_vacancy,
            persistent_vacancy_population,
        }
    }
    #[cfg(not(feature = "jagua-experimental"))]
    {
        let _ = (
            pieces,
            fast_settings,
            pinned_vacancy_parent,
            secondary_pinned_vacancy_parent,
            rollback_comparison,
        );
        let reason = "coupled dynamic separator requires the jagua-experimental feature".to_owned();
        GeneralCoupledSeparatorDiagnostics {
            seed_domain: COUPLED_SEPARATOR_SEED_DOMAIN,
            control: skipped_coupled_separator_arm(
                CoupledSeparatorArm::Control,
                protected,
                reason.clone(),
            ),
            treatment: skipped_coupled_separator_arm(
                CoupledSeparatorArm::Treatment,
                protected,
                reason,
            ),
            boundary_projection_treatment: None,
            conflict_ruin_recreate: None,
            precompression_frontier_vacancy: None,
            persistent_vacancy_population: None,
        }
    }
}

/// Runs one deep-operator (persistent-vacancy) mode against one parent layout.
///
/// This is the whole of what the coupled separator's mode slot used to do
/// inline, extracted verbatim so that it has exactly one implementation and
/// two callers: the separator's own slot, which is unchanged, and the
/// portfolio coordinator, which needs to invoke an operator against a parent
/// it chose from its archive rather than against the one arm the separator
/// happens to end on.
///
/// Nothing here decides validity or publication. The mode reports what it
/// found and `record_persistent_vacancy_contract_report` measures it; the
/// adoption rule lives in [`adopt_published_placements`] and is the only door
/// to a published result for either caller.
#[cfg(feature = "jagua-experimental")]
pub(super) fn dispatch_persistent_vacancy_mode(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    effective_parent: &GeneralCoupledSeparatorArmDiagnostics,
    parent_source: Option<String>,
    secondary_pinned_vacancy_parent: Option<&GeneralPersistentVacancyPinnedParent>,
) -> GeneralPersistentVacancyDiagnostics {
    // One scope per deep-operator dispatch, named by mode and
    // rooted at the parent it descends from, so every exact-valid
    // candidate the mode produces is attributed to both.
    #[cfg(feature = "quality-trace")]
    let _trace_mode = {
        let parent_fingerprint = quality_trace::active().then(|| {
            general_placement_fingerprint(&fast_placements_from_coupled_diagnostics(
                &effective_parent.final_placements,
            ))
        });
        quality_trace::scope(
            format!("mode{}", relaxed_settings.persistent_vacancy_mode),
            relaxed_settings.seed,
            parent_fingerprint.as_deref(),
        )
    };
    let mut population = match relaxed_settings.persistent_vacancy_mode {
        22 => run_alternation_fixpoint(
            pieces,
            fast_settings,
            relaxed_settings,
            effective_parent,
            parent_source,
        ),
        23 => run_recombination(
            pieces,
            fast_settings,
            relaxed_settings,
            effective_parent,
            parent_source,
            secondary_pinned_vacancy_parent,
        ),
        24 => persistent_vacancy::run_bounded_reinsertion(
            pieces,
            fast_settings,
            relaxed_settings,
            effective_parent,
            parent_source,
        ),
        26 => run_ladder_compression(
            pieces,
            fast_settings,
            relaxed_settings,
            effective_parent,
            parent_source,
        ),
        27 => run_micro_legalization_probe(pieces, fast_settings, effective_parent, parent_source),
        #[cfg(feature = "compression-schedule")]
        34 => run_compression_schedule(
            pieces,
            fast_settings,
            relaxed_settings,
            effective_parent,
            parent_source,
        ),
        // Modes 32 and 33 are modes 28 and 29 with the
        // orientation-perturbation candidate stream armed; nothing
        // else about the two pipelines differs, which is why they
        // are the same two entry points with one flag.
        mode @ (28 | 32) => persistent_vacancy::run_replacement_repair(
            pieces,
            fast_settings,
            relaxed_settings,
            effective_parent,
            parent_source,
            mode == 32,
        ),
        mode @ (29 | 33) => persistent_vacancy::run_joint_replacement_repair(
            pieces,
            fast_settings,
            relaxed_settings,
            effective_parent,
            parent_source,
            mode == 33,
        ),
        mode @ (30 | 31) => run_global_legalization_probe(
            pieces,
            fast_settings,
            relaxed_settings,
            effective_parent,
            parent_source,
            mode == 31,
        ),
        mode => persistent_vacancy::run_persistent_vacancy_population(
            pieces,
            fast_settings,
            relaxed_settings,
            effective_parent,
            parent_source,
            mode,
        ),
    };
    record_persistent_vacancy_contract_report(
        &mut population,
        pieces,
        fast_settings,
        effective_parent,
    );
    #[cfg(feature = "quality-trace")]
    quality_trace::mode_result(
        population.mode,
        population.exact_valid,
        population.independent_depth_mm,
        population.parent_independent_depth_mm,
        population.final_placement_fingerprint.as_deref(),
        population.failure_reason.as_deref(),
    );
    population
}

/// Builds a `GeneralFastResult` suitable as a relaxed-search incumbent from
/// placements that may or may not currently be exact-valid (the whole point
/// of this engine is to legalize temporarily infeasible complete states).
/// Only `.placements` and `.used_long_axis_depth_mm` drive the downstream
/// search; the remaining bookkeeping fields are unused by that path and are
/// left at their neutral defaults.
#[cfg(feature = "jagua-experimental")]
pub(super) fn general_fast_result_seed(
    placements: Vec<GeneralFastPlacement>,
    used_long_axis_depth_mm: f64,
) -> GeneralFastResult {
    GeneralFastResult {
        placements,
        unplaced_piece_ids: Vec::new(),
        exact_evaluations: 0,
        primary_exact_evaluations: 0,
        order_portfolio_exact_evaluations: 0,
        catalog_portfolio_exact_evaluations: 0,
        pairing_exact_evaluations: 0,
        beam_exact_evaluations: 0,
        tightening_exact_evaluations: 0,
        tightening_passes_attempted: 0,
        tightening_passes_improved: 0,
        catalog_candidate_placed_count: None,
        catalog_candidate_depth_mm: None,
        pairing_candidate_placed_count: None,
        pairing_candidate_depth_mm: None,
        beam_candidate_placed_count: None,
        beam_candidate_depth_mm: None,
        exploratory_exact_evaluations: 0,
        repair_exact_evaluations: 0,
        local_angle_refinement_exact_evaluations: 0,
        repair_targets_considered: 0,
        order_variants_attempted: 0,
        catalog_variants_attempted: 0,
        order_portfolio_failed: false,
        catalog_portfolio_failed: false,
        pairing_failed: false,
        beam_failed: false,
        exploratory_failed: false,
        repair_failed: false,
        used_short_axis_span_mm: 0.0,
        used_long_axis_depth_mm,
        unused_short_axis_projection_mm: 0.0,
        occupied_envelope_area_mm2: 0.0,
    }
}

#[cfg(feature = "jagua-experimental")]
pub(super) fn fast_placements_from_coupled_diagnostics(
    placements: &[GeneralCoupledSeparatorPlacementDiagnostics],
) -> Vec<GeneralFastPlacement> {
    placements
        .iter()
        .map(|placement| GeneralFastPlacement {
            piece_id: placement.piece_id.clone(),
            rotation_deg: placement.rotation_deg,
            mirrored: placement.mirrored,
            translate_short_axis: placement.translate_short_axis,
            translate_long_axis: placement.translate_long_axis,
        })
        .collect()
}

/// Counts pairwise exact polygon overlaps in a (possibly infeasible)
/// placement set. Used only for mode 23's cheap seam diagnostic; the
/// authoritative exact-valid gate remains `validate_and_measure_placements`.
#[cfg(feature = "jagua-experimental")]
fn count_exact_overlap_pairs(
    pieces: &[GeneralFastPiece<'_>],
    placements: &[GeneralFastPlacement],
) -> Result<usize, GeneralFastError> {
    let by_id = pieces
        .iter()
        .map(|piece| (piece.id, piece))
        .collect::<BTreeMap<_, _>>();
    let mut transformed = Vec::with_capacity(placements.len());
    for placement in placements {
        let piece = by_id.get(placement.piece_id.as_str()).ok_or_else(|| {
            GeneralFastError::InvalidInput(format!(
                "recombination hybrid references unknown piece {}",
                placement.piece_id
            ))
        })?;
        transformed.push(piece.polygon.transformed(
            placement.rotation_deg,
            placement.mirrored,
            placement.translate_short_axis,
            placement.translate_long_axis,
        )?);
    }
    let mut overlaps = 0usize;
    for first in 0..transformed.len() {
        for second in (first + 1)..transformed.len() {
            if polygons_overlap_exact(&transformed[first], &transformed[second])? {
                overlaps += 1;
            }
        }
    }
    Ok(overlaps)
}

/// Mode 22: alternation fixpoint. Alternates the legacy separator treatment
/// (the same pipeline mode 0 exercises, warm-started in-process from the
/// current state) with the persistent-vacancy descent (the same machinery
/// mode 11 exercises, targeting `current_best + rung`) for up to
/// `ALTERNATION_MAX_CYCLES` cycles, accepting only strict, exact-valid
/// improvements from either arm, until neither arm improves.
#[cfg(feature = "jagua-experimental")]
fn run_alternation_fixpoint(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    parent: &GeneralCoupledSeparatorArmDiagnostics,
    parent_source: Option<String>,
) -> GeneralPersistentVacancyDiagnostics {
    let mut diagnostics = GeneralPersistentVacancyDiagnostics {
        mode: 22,
        seed_domain: ALTERNATION_SEED_DOMAIN,
        parent_source,
        ..GeneralPersistentVacancyDiagnostics::default()
    };
    let Some(starting_target_depth_mm) = relaxed_settings.persistent_vacancy_target_depth_mm else {
        diagnostics.failure_reason =
            Some("persistent vacancy mode 22 requires an explicit target depth".to_owned());
        return diagnostics;
    };
    if !starting_target_depth_mm.is_finite() || starting_target_depth_mm <= 0.0 {
        diagnostics.failure_reason =
            Some("persistent vacancy target depth must be a positive finite value".to_owned());
        return diagnostics;
    }
    diagnostics.target_depth_mm = starting_target_depth_mm;
    if parent.final_placements.len() != pieces.len() {
        diagnostics.failure_reason =
            Some("persistent vacancy parent is not a complete exact-valid layout".to_owned());
        return diagnostics;
    }

    let parent_placements = fast_placements_from_coupled_diagnostics(&parent.final_placements);
    diagnostics.parent_fingerprint = Some(coupled_fast_placement_fingerprint(&parent_placements));
    if let Err(error) = validate_and_measure_placements(pieces, &parent_placements, fast_settings) {
        diagnostics.failure_reason = Some(format!("persistent vacancy parent validation: {error}"));
        return diagnostics;
    }
    let mut current_best =
        match coupled_independent_source_depth(pieces, &parent_placements, fast_settings) {
            Ok(depth) => depth,
            Err(error) => {
                diagnostics.failure_reason =
                    Some(format!("persistent vacancy parent depth: {error}"));
                return diagnostics;
            }
        };
    diagnostics.parent_independent_depth_mm = Some(current_best);
    diagnostics.initial_state_fingerprint = diagnostics.parent_fingerprint.clone();
    let mut current = general_fast_result_seed(parent_placements, current_best);

    diagnostics.attempted = true;
    // The coordinator's alternation *quantum*: a cap that can only shorten the
    // mode's own bound, never extend it, so no setting can buy this mode more
    // work than it has ever been allowed to do.
    let max_cycles = relaxed_settings
        .alternation_max_cycles
        .unwrap_or(ALTERNATION_MAX_CYCLES)
        .min(ALTERNATION_MAX_CYCLES);
    let mut cycle_rows = Vec::with_capacity(max_cycles);
    let mut cycles_run = 0usize;
    for cycle in 0..max_cycles {
        cycles_run = cycle + 1;
        let mut row = GeneralPersistentVacancyAlternationCycleDiagnostics {
            cycle,
            ..GeneralPersistentVacancyAlternationCycleDiagnostics::default()
        };

        // (a) legacy separator treatment, warm-started in-process from the
        // current state (the same pipeline mode 0 exercises via the
        // warm-start incumbent, just supplied directly instead of through a
        // fixture file).
        let mut separator_settings = relaxed_settings;
        separator_settings.persistent_vacancy_mode = 0;
        let separator_improved = match improve_complete_layout_with_pinned_vacancy_parent(
            pieces,
            fast_settings,
            separator_settings,
            &current,
            None,
            None,
        ) {
            Ok(outcome) => {
                let arm = outcome
                    .diagnostics
                    .coupled_dynamic_separator
                    .as_ref()
                    .and_then(|coupled| coupled.boundary_projection_treatment.as_ref());
                match arm {
                    Some(arm) => {
                        row.separator_attempted = true;
                        row.separator_depth_mm = Some(arm.final_depth_mm);
                        let candidate_placements =
                            fast_placements_from_coupled_diagnostics(&arm.final_placements);
                        match validate_and_measure_placements(
                            pieces,
                            &candidate_placements,
                            fast_settings,
                        ) {
                            Ok(_) => match coupled_independent_source_depth(
                                pieces,
                                &candidate_placements,
                                fast_settings,
                            ) {
                                Ok(depth) => {
                                    row.separator_independent_depth_mm = Some(depth);
                                    if grid_key(depth) < grid_key(current_best) {
                                        current_best = depth;
                                        current =
                                            general_fast_result_seed(candidate_placements, depth);
                                        true
                                    } else {
                                        false
                                    }
                                }
                                Err(error) => {
                                    row.separator_failure_reason = Some(error.to_string());
                                    false
                                }
                            },
                            Err(error) => {
                                row.separator_failure_reason = Some(error.to_string());
                                false
                            }
                        }
                    }
                    None => {
                        row.separator_failure_reason = Some(
                            "separator sub-step produced no boundary-projection arm".to_owned(),
                        );
                        false
                    }
                }
            }
            Err(error) => {
                row.separator_failure_reason = Some(error.to_string());
                false
            }
        };
        row.separator_improved = separator_improved;

        // (b) persistent-vacancy descent (mode 11 machinery) from the
        // (possibly separator-improved) current state, targeting
        // current_best plus the shared descent rung.
        let descent_target = current_best + ALTERNATION_DESCENT_TARGET_STEP_MM;
        row.descent_target_depth_mm = Some(descent_target);
        let mut descent_settings = relaxed_settings;
        descent_settings.persistent_vacancy_mode = 11;
        descent_settings.persistent_vacancy_target_depth_mm = Some(descent_target);
        let descent_parent = GeneralCoupledSeparatorArmDiagnostics {
            final_placements: coupled_placement_diagnostics(&current.placements),
            ..GeneralCoupledSeparatorArmDiagnostics::default()
        };
        let descent_diagnostics = persistent_vacancy::run_persistent_vacancy_population(
            pieces,
            fast_settings,
            descent_settings,
            &descent_parent,
            Some(format!("alternationCycle{cycle}")),
            11,
        );
        row.descent_attempted = descent_diagnostics.attempted;
        let descent_improved = if descent_diagnostics.exact_valid {
            match descent_diagnostics.independent_depth_mm {
                Some(depth) => {
                    row.descent_independent_depth_mm = Some(depth);
                    if grid_key(depth) < grid_key(current_best) {
                        let descent_placements = fast_placements_from_coupled_diagnostics(
                            &descent_diagnostics.final_placements,
                        );
                        match validate_and_measure_placements(
                            pieces,
                            &descent_placements,
                            fast_settings,
                        ) {
                            Ok(_) => {
                                current_best = depth;
                                current = general_fast_result_seed(descent_placements, depth);
                                true
                            }
                            Err(error) => {
                                row.descent_failure_reason = Some(error.to_string());
                                false
                            }
                        }
                    } else {
                        false
                    }
                }
                None => false,
            }
        } else {
            row.descent_failure_reason = descent_diagnostics.failure_reason.clone();
            false
        };
        row.descent_improved = descent_improved;

        cycle_rows.push(row);
        if !separator_improved && !descent_improved {
            break;
        }
    }

    diagnostics.exact_valid = true;
    diagnostics.independent_depth_mm = Some(current_best);
    diagnostics.final_placement_fingerprint =
        Some(coupled_fast_placement_fingerprint(&current.placements));
    diagnostics.final_placements = coupled_placement_diagnostics(&current.placements);
    diagnostics.alternation = Some(GeneralPersistentVacancyAlternationDiagnostics {
        cycles_run,
        cycles: cycle_rows,
    });
    diagnostics
}

/// A profiling-totals sample taken at a mode-26 region boundary.
///
/// `search-profiling` builds only; see [`GeneralPersistentVacancyLadderAnatomy`]
/// for why this is not an ordinary diagnostic.
#[cfg(all(feature = "jagua-experimental", feature = "search-profiling"))]
struct LadderAnatomySample {
    started: Instant,
    phases: Vec<profiling::PhaseSample>,
    counters: [u64; Counter::COUNT],
}

#[cfg(all(feature = "jagua-experimental", feature = "search-profiling"))]
impl LadderAnatomySample {
    /// Opens a region.
    fn open() -> Self {
        let snapshot = profiling::snapshot();
        Self {
            started: Instant::now(),
            phases: snapshot.phases,
            counters: profiling::counter_totals(),
        }
    }

    /// Closes a region into `anatomy`, filling its wall time and its phase and
    /// counter deltas. Only phases and counters that actually moved are
    /// recorded, so a region's map names the work it did rather than the whole
    /// declaration order.
    fn close(self, anatomy: &mut GeneralPersistentVacancyLadderAnatomy) {
        anatomy.wall_ms = self.started.elapsed().as_secs_f64() * 1000.0;
        let after = profiling::snapshot();
        for (before, now) in self.phases.iter().zip(after.phases.iter()) {
            let nanos = now.nanos.saturating_sub(before.nanos);
            let calls = now.calls.saturating_sub(before.calls);
            if nanos == 0 && calls == 0 {
                continue;
            }
            anatomy.phases.insert(
                now.phase.name().to_owned(),
                GeneralLadderPhaseDelta {
                    milliseconds: nanos as f64 / 1.0e6,
                    calls,
                },
            );
        }
        let after_counters = profiling::counter_totals();
        for (index, counter) in Counter::ALL.iter().enumerate() {
            let delta = after_counters[index].saturating_sub(self.counters[index]);
            if delta == 0 {
                continue;
            }
            anatomy.counters.insert(counter.name().to_owned(), delta);
        }
    }
}

/// Times one mode-26 region into `slot` when the `search-profiling` build is
/// in use, and calls `body` unchanged otherwise.
///
/// This is a statement macro rather than a function taking a closure because
/// several of the regions it wraps borrow `row` mutably inside the body while
/// writing a field of `row` outside it.
macro_rules! ladder_time {
    ($slot:expr, $body:block) => {{
        #[cfg(feature = "search-profiling")]
        let started = Instant::now();
        let value = $body;
        #[cfg(feature = "search-profiling")]
        {
            $slot = started.elapsed().as_secs_f64() * 1000.0;
        }
        value
    }};
}

/// Builds the mode-26 ladder of effective sheet long-axis bounds.
///
/// The rungs run from just below `parent_depth_mm` down to `final_bound_mm`
/// in at most `LADDER_COMPRESSION_STEPS` uniform decrements. A rung smaller
/// than the separator's own single-target contraction
/// (`COUPLED_SEPARATOR_CONTRACTION_RATIO` of the parent depth) asks the
/// pipeline for less than one contraction step and would only burn a full
/// pipeline run for nothing, so that ratio is the step floor: it is
/// scale-free, so the ladder is instance-agnostic, and it is an existing
/// constant rather than a new tuned literal. When the floor binds, the ladder
/// simply uses fewer rungs; the last rung is always exactly `final_bound_mm`.
#[cfg(feature = "jagua-experimental")]
fn ladder_compression_bounds(parent_depth_mm: f64, final_bound_mm: f64) -> (f64, Vec<f64>) {
    let span_mm = parent_depth_mm - final_bound_mm;
    let floor_mm = parent_depth_mm * COUPLED_SEPARATOR_CONTRACTION_RATIO;
    let step_mm = (span_mm / LADDER_COMPRESSION_STEPS as f64).max(floor_mm);
    let steps = ((span_mm / step_mm).ceil() as usize).clamp(1, LADDER_COMPRESSION_STEPS);
    let bounds = (0..steps)
        .map(|step| {
            if step + 1 == steps {
                final_bound_mm
            } else {
                (parent_depth_mm - step_mm * (step + 1) as f64).max(final_bound_mm)
            }
        })
        .collect();
    (step_mm, bounds)
}

/// Mode 26: clamped-sheet ladder compression.
///
/// Every measured negative in this experiment family shares one shape: the
/// separator treats depth as an *objective* while the real sheet leaves
/// unlimited depth-ward room, so legalizing a deep layout relaxes it back out
/// to a shallow shelf instead of compressing it. This mode removes that room
/// instead of penalizing its use. Given parent `P` and a final bound
/// `D_final`, it walks a ladder of bounds from `P`'s own measured depth down
/// to `D_final` and, at each rung `bound_k`, hands the ordinary mode-0
/// pipeline a sheet whose long axis *is* `bound_k`:
///
/// * `fast_settings.sheet_long_axis_mm = bound_k` is the geometric clamp. It
///   is the only place the long axis bounds anything: every acceptance in the
///   pipeline runs through `validate_and_measure_placements`, whose
///   `collision_fits_sheet` check rejects any pose past
///   `bound_k - collision_sheet_inset_mm`. Depth-ward relaxation therefore
///   becomes impossible rather than merely expensive. That check runs on the
///   collision polygon, which carries the conservative search allowance the
///   reported source measure does not, so the clamp is stricter than
///   `bound_k` by exactly that allowance and can never admit a state the
///   explicit re-measure below would reject.
/// * the warm-start incumbent's `used_long_axis_depth_mm` is set to `bound_k`
///   too, because that (not the sheet) is what seeds `RelaxedState`'s
///   `strip_depth_mm` and the separator's contraction targets. Handing the
///   pipeline the parent's own poses under that depth is precisely a state
///   whose protruding pieces carry positive boundary loss, which is what the
///   coupled separator and the boundary-projection terminal are built to work
///   off.
///
/// Out-of-sheet warm starts need no relaxation anywhere: nothing on the path
/// into the pipeline validates the incumbent, and `compress_state_at_split`
/// already produces out-of-strip poses by construction. Only *publication* is
/// gated, which is the intended asymmetry.
///
/// Chaining policy: each rung warm-starts from the previous rung's final
/// state, feasible or not - the arm's exact-accepted state when the rung
/// legalized something, otherwise the arm's terminal minimum-loss state, which
/// is the compressed-but-infeasible layout the rung reached and could not
/// legalize. Reverting to the last exact-valid state instead would discard the
/// partial compression a failed rung achieved and, since every rung is
/// deterministic, would replay the same failure at every remaining rung; that
/// policy was measured and the chain provably never moves under it. A rung
/// that produces no usable complete state at all leaves the chain untouched,
/// which the next rung records in its `warmStartSource`. Publication is
/// independent of the chain: the deepest exact-valid state seen over the whole
/// ladder wins, with the parent itself as the floor, so mode 26 can never
/// publish something worse than its parent.
///
/// Every rung is re-validated and re-measured against the *real* request, not
/// the clamped one, so the publication contract is exactly the requested one.
#[cfg(feature = "jagua-experimental")]
fn run_ladder_compression(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    parent: &GeneralCoupledSeparatorArmDiagnostics,
    parent_source: Option<String>,
) -> GeneralPersistentVacancyDiagnostics {
    let mut diagnostics = GeneralPersistentVacancyDiagnostics {
        mode: 26,
        seed_domain: LADDER_COMPRESSION_SEED_DOMAIN,
        parent_source,
        ..GeneralPersistentVacancyDiagnostics::default()
    };
    let Some(final_bound_mm) = relaxed_settings.persistent_vacancy_target_depth_mm else {
        diagnostics.failure_reason =
            Some("persistent vacancy mode 26 requires an explicit final bound".to_owned());
        return diagnostics;
    };
    if !final_bound_mm.is_finite() || final_bound_mm <= 0.0 {
        diagnostics.failure_reason = Some(
            "persistent vacancy mode 26 final bound must be a positive finite value".to_owned(),
        );
        return diagnostics;
    }
    diagnostics.target_depth_mm = final_bound_mm;
    if pieces.is_empty() {
        diagnostics.failure_reason =
            Some("persistent vacancy experiment requires at least one piece".to_owned());
        return diagnostics;
    }
    if parent.final_placements.len() != pieces.len() {
        diagnostics.failure_reason =
            Some("persistent vacancy parent is not a complete exact-valid layout".to_owned());
        return diagnostics;
    }

    let parent_placements = fast_placements_from_coupled_diagnostics(&parent.final_placements);
    diagnostics.parent_fingerprint = Some(coupled_fast_placement_fingerprint(&parent_placements));
    if let Err(error) = validate_and_measure_placements(pieces, &parent_placements, fast_settings) {
        diagnostics.failure_reason = Some(format!("persistent vacancy parent validation: {error}"));
        return diagnostics;
    }
    let parent_depth_mm =
        match coupled_independent_source_depth(pieces, &parent_placements, fast_settings) {
            Ok(depth) => depth,
            Err(error) => {
                diagnostics.failure_reason =
                    Some(format!("persistent vacancy parent depth: {error}"));
                return diagnostics;
            }
        };
    diagnostics.parent_independent_depth_mm = Some(parent_depth_mm);
    diagnostics.initial_state_fingerprint = diagnostics.parent_fingerprint.clone();
    if grid_key(final_bound_mm) >= grid_key(parent_depth_mm) {
        diagnostics.failure_reason = Some(
            "persistent vacancy mode 26 final bound must be below the parent depth".to_owned(),
        );
        return diagnostics;
    }

    diagnostics.attempted = true;
    #[cfg(feature = "search-profiling")]
    let ladder_region = LadderAnatomySample::open();
    let (step_mm, bounds) = ladder_compression_bounds(parent_depth_mm, final_bound_mm);
    let mut ladder = GeneralPersistentVacancyLadderCompressionDiagnostics {
        parent_depth_mm,
        final_bound_mm,
        step_mm,
        steps_planned: bounds.len(),
        ..GeneralPersistentVacancyLadderCompressionDiagnostics::default()
    };

    // Two carried states. `published` is the deepest exact-valid layout known
    // and is the ladder's answer; `chain` is the compression frontier, which
    // is generally infeasible. They start equal, at the parent.
    let mut published_placements = parent_placements.clone();
    let mut published_depth_mm = parent_depth_mm;
    let mut published_source = "parent".to_owned();
    let mut chain_placements = parent_placements;
    let mut chain_depth_mm = Some(parent_depth_mm);
    let mut chain_source = "parent".to_owned();

    let mut separator_settings = relaxed_settings;
    separator_settings.persistent_vacancy_mode = 0;
    for (step, bound_mm) in bounds.iter().copied().enumerate() {
        // The seed carries the warm-start poses at a strip depth one separator
        // contraction *above* the bound, so the arm's first contraction target
        // lands exactly on `bound_mm` instead of 0.1% below it: the rung asks
        // the pipeline for precisely its own bound and nothing more. This is
        // the same seed-depth headroom mode 23 uses so that a pipeline which
        // only accepts strict improvements can publish at all, sized here to
        // the separator's own single-target step rather than to the epoch
        // ladder, so the strip depth stays tight enough for the boundary
        // penalty to keep pulling protruding pieces inward.
        let seed_depth_mm = bound_mm / (1.0 - COUPLED_SEPARATOR_CONTRACTION_RATIO);
        let mut row = GeneralPersistentVacancyLadderStepDiagnostics {
            step,
            bound_mm,
            seed_depth_mm,
            ..GeneralPersistentVacancyLadderStepDiagnostics::default()
        };
        ladder.steps_run = step + 1;
        #[cfg(feature = "search-profiling")]
        let step_region = LadderAnatomySample::open();

        // The clamp: the search believes in a sheet exactly `bound_mm` deep.
        let step_settings = GeneralFastSettings {
            sheet_long_axis_mm: bound_mm,
            ..fast_settings
        };
        let chain_fingerprint = coupled_fast_placement_fingerprint(&chain_placements);
        let published_fingerprint = coupled_fast_placement_fingerprint(&published_placements);
        let mut warm_starts = vec![(
            "feasible",
            published_source.clone(),
            Some(published_depth_mm),
            published_placements.clone(),
        )];
        if chain_fingerprint != published_fingerprint {
            warm_starts.push((
                "compression",
                chain_source.clone(),
                chain_depth_mm,
                chain_placements.clone(),
            ));
        }

        for (role, warm_start_source, warm_start_depth_mm, warm_placements) in warm_starts {
            // A rung arm gets a small deterministic retry budget. Each attempt
            // salts the separator seed, so a rung that lost its single draw to
            // an aborted target or an unprojectable residue gets genuinely
            // different draws rather than a repeat, and an attempt that
            // publishes ends the loop.
            for attempt in 0..LADDER_COMPRESSION_RUNG_ATTEMPTS {
                let mut attempt_settings = separator_settings;
                if attempt > 0 {
                    // The first attempt keeps the rung's own seed, so the
                    // retry budget is a strict superset of the single-shot
                    // behaviour rather than a reshuffle of it: whatever the
                    // rung used to draw, it still draws first.
                    attempt_settings.seed = derive_seed(
                        separator_settings.seed,
                        attempt,
                        LADDER_COMPRESSION_ATTEMPT_SLOT,
                    );
                }
                let (mut arm_row, produced) = run_ladder_compression_arm(
                    pieces,
                    fast_settings,
                    step_settings,
                    attempt_settings,
                    bound_mm,
                    seed_depth_mm,
                    role,
                    warm_start_source.clone(),
                    warm_start_depth_mm,
                    warm_placements.clone(),
                );
                arm_row.attempt = attempt;
                row.attempts_run = row.attempts_run.saturating_add(1);
                let mut published_here = false;
                if let Some(product) = produced {
                    // Publication takes the arm's own exact-accepted state when
                    // it has one, and otherwise whatever the micro-legalization
                    // pass managed to project out of the rejected state.
                    let candidate = if product.exact_valid {
                        Some((product.placements.clone(), product.depth_mm, None))
                    } else {
                        product.legalized.clone().map(|(placements, depth_mm)| {
                            (placements, depth_mm, product.repair_tier)
                        })
                    };
                    if let Some((placements, depth_mm, tier)) = candidate {
                        let improved = grid_key(depth_mm) < grid_key(published_depth_mm);
                        if improved {
                            published_depth_mm = depth_mm;
                            published_placements = placements;
                            published_source = format!("step{step}");
                            ladder.published_step = Some(step);
                            ladder.published_bound_mm = Some(bound_mm);
                            row.improved_publication = true;
                            row.published_by_micro_legalization =
                                tier == Some(LADDER_REPAIR_TIER_MICRO);
                            row.published_repair_tier = tier.map(str::to_owned);
                        }
                        // A tier-four state that does not improve must not end
                        // the rung's retry loop. Without the tier this arm
                        // produced no candidate at all and the remaining
                        // attempts were still spent looking, so consuming them
                        // on a state that beats nothing would *remove* draws
                        // rather than add publications - the one way a strictly
                        // later tier could make the ladder worse.
                        published_here = improved || tier != Some(LADDER_REPAIR_TIER_GLOBAL);
                    }
                    // The compression frontier is the deepest state seen,
                    // feasible or not: that is the material the next, tighter
                    // rung works off. It always tracks the arm's own state, so
                    // a micro-legalized publication never blunts the frontier.
                    if chain_depth_mm
                        .is_none_or(|current| grid_key(product.depth_mm) < grid_key(current))
                    {
                        chain_depth_mm = Some(product.depth_mm);
                        chain_placements = product.placements;
                        chain_source = if arm_row.from_terminal {
                            format!("step{step}:terminal")
                        } else {
                            format!("step{step}")
                        };
                        row.chain_advanced = true;
                    }
                }
                row.rollback_disagreements_tolerated = row
                    .rollback_disagreements_tolerated
                    .saturating_add(arm_row.rollback_disagreements_tolerated);
                row.rollback_disagreement_max_pressure_ulps = row
                    .rollback_disagreement_max_pressure_ulps
                    .max(arm_row.rollback_disagreement_max_pressure_ulps);
                row.arms.push(arm_row);
                if published_here {
                    break;
                }
            }
        }

        row.published_depth_mm_after = published_depth_mm;
        row.chained_depth_mm_after = chain_depth_mm;
        #[cfg(feature = "search-profiling")]
        {
            step_region.close(&mut row.anatomy);
            // Everything the rung spent outside its own arms: the two
            // warm-start fingerprints, the warm-start clones, and the
            // publication bookkeeping between attempts.
            let arm_ms = row
                .arms
                .iter()
                .map(|arm| arm.anatomy.wall_ms)
                .sum::<f64>();
            row.anatomy.orchestration_ms = (row.anatomy.wall_ms - arm_ms).max(0.0);
        }
        ladder.steps.push(row);
    }

    #[cfg(feature = "search-profiling")]
    {
        ladder_region.close(&mut ladder.anatomy);
        let step_ms = ladder
            .steps
            .iter()
            .map(|step| step.anatomy.wall_ms)
            .sum::<f64>();
        ladder.anatomy.orchestration_ms = (ladder.anatomy.wall_ms - step_ms).max(0.0);
    }
    diagnostics.exact_valid = true;
    diagnostics.independent_depth_mm = Some(published_depth_mm);
    diagnostics.final_placement_fingerprint =
        Some(coupled_fast_placement_fingerprint(&published_placements));
    diagnostics.final_placements = coupled_placement_diagnostics(&published_placements);
    diagnostics.ladder_compression = Some(ladder);
    diagnostics
}

/// Mode 34: the compression schedule.
///
/// The mode-26 ladder and this mode ask for the same thing and pay for it
/// differently. A mode-26 rung hands the whole mode-0 pipeline a sheet whose
/// long axis *is* the rung's bound and lets it legalize from scratch: measured
/// at 32.25M candidate queries and 4.7-13.8 s per rung, to move one bound by
/// 0.159 mm, with 75.5% of the arm wall lost to a rollback comparison the rung
/// inherits from the coupled separator. This mode keeps one lane alive and
/// lowers the clamp underneath it, one canonical grid unit at a time, running
/// the lane's own violating-pair queue as the repair between steps.
///
/// The five things it adds to the relaxed lane are exactly the five the rung
/// anatomy found missing:
///
/// * **(a) a schedule that owns the depth.** [`CompressionSchedule`] lives on
///   the lane, and `move_sweep` writes its `depth_mm()` into the state at every
///   sweep entry. That one write reaches all eleven `boundary_penalty` call
///   sites and every candidate generator's sampling box, because all of them
///   already read the same scalar.
/// * **(b) a monotone floor in the proxy tier.** The schedule's `floor_mm` only
///   ever decreases and lives on the lane, not on the state, so no rollback, no
///   epoch acceptance and no rescore can restore a looser depth - the memory
///   `boundary_penalty` does not have.
/// * **(c) a deepest-confirmed slot.** `confirmed` below is the incumbent and
///   it is only ever written by an accepted exact confirmation; `state` is the
///   frontier and may be infeasible for as long as the schedule likes. That
///   asymmetry is the one property Sol's review asks the port to preserve.
/// * **(d) a repair ordered from the queue that already exists.** A step that
///   makes `k` pieces protrude puts exactly those `k` pieces into the next
///   sweep's active set, through `PairTracker::collision_pairs` and
///   `piece_is_active`, with no new selection logic. When a confirmation is
///   nonetheless refused, one `micro_legalize` pass - the only repair tier
///   cheap enough for this loop at 0.83 ms - is offered the layout.
/// * **(e) a rollback contract that survives a moving depth.** It does not
///   inherit the coupled separator's rollback rescore at all; see
///   [`CompressionSchedule`]'s own note for why, and what replaces it.
///
/// Publication is the same contract mode 26 uses: the deepest exact-valid
/// layout seen over the whole run wins, with the parent itself as the floor, so
/// this mode can never publish something worse than its parent. Every candidate
/// is validated against the *real* request; the schedule's clamp is a proxy-tier
/// scalar and never touches `fast_settings.sheet_long_axis_mm`.
#[cfg(all(feature = "jagua-experimental", feature = "compression-schedule"))]
fn run_compression_schedule(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    parent: &GeneralCoupledSeparatorArmDiagnostics,
    parent_source: Option<String>,
) -> GeneralPersistentVacancyDiagnostics {
    let mut diagnostics = GeneralPersistentVacancyDiagnostics {
        mode: 34,
        seed_domain: COMPRESSION_SCHEDULE_SEED_DOMAIN,
        parent_source,
        ..GeneralPersistentVacancyDiagnostics::default()
    };
    let Some(final_bound_mm) = relaxed_settings.persistent_vacancy_target_depth_mm else {
        diagnostics.failure_reason =
            Some("persistent vacancy mode 34 requires an explicit final bound".to_owned());
        return diagnostics;
    };
    if !final_bound_mm.is_finite() || final_bound_mm <= 0.0 {
        diagnostics.failure_reason = Some(
            "persistent vacancy mode 34 final bound must be a positive finite value".to_owned(),
        );
        return diagnostics;
    }
    diagnostics.target_depth_mm = final_bound_mm;
    if pieces.is_empty() {
        diagnostics.failure_reason =
            Some("compression schedule requires at least one piece".to_owned());
        return diagnostics;
    }
    if parent.final_placements.len() != pieces.len() {
        diagnostics.failure_reason =
            Some("compression schedule requires a complete parent layout".to_owned());
        return diagnostics;
    }
    let Some(schedule_settings) = relaxed_settings.compression_schedule else {
        diagnostics.failure_reason =
            Some("persistent vacancy mode 34 requires an armed compression schedule".to_owned());
        return diagnostics;
    };

    let parent_placements = fast_placements_from_coupled_diagnostics(&parent.final_placements);
    diagnostics.parent_fingerprint = Some(coupled_fast_placement_fingerprint(&parent_placements));
    diagnostics.initial_state_fingerprint = diagnostics.parent_fingerprint.clone();
    if let Err(error) = validate_and_measure_placements(pieces, &parent_placements, fast_settings) {
        diagnostics.failure_reason = Some(format!("compression schedule parent validation: {error}"));
        return diagnostics;
    }
    let parent_depth_mm =
        match coupled_independent_source_depth(pieces, &parent_placements, fast_settings) {
            Ok(depth) => depth,
            Err(error) => {
                diagnostics.failure_reason = Some(format!("compression schedule parent depth: {error}"));
                return diagnostics;
            }
        };
    diagnostics.parent_independent_depth_mm = Some(parent_depth_mm);
    if grid_key(final_bound_mm) >= grid_key(parent_depth_mm) {
        diagnostics.failure_reason =
            Some("persistent vacancy mode 34 final bound must be below the parent depth".to_owned());
        return diagnostics;
    }
    diagnostics.attempted = true;

    match drive_compression_schedule(
        pieces,
        fast_settings,
        relaxed_settings,
        schedule_settings,
        &parent_placements,
        parent_depth_mm,
        parent_depth_mm - final_bound_mm,
    ) {
        Ok((published_placements, published_depth_mm, report)) => {
            diagnostics.complete_states = 1;
            diagnostics.exact_valid = true;
            diagnostics.independent_depth_mm = Some(published_depth_mm);
            diagnostics.final_placement_fingerprint =
                Some(coupled_fast_placement_fingerprint(&published_placements));
            diagnostics.final_placements = coupled_placement_diagnostics(&published_placements);
            diagnostics.compression_schedule = Some(report);
        }
        Err(error) => {
            diagnostics.failure_reason = Some(format!("compression schedule: {error}"));
        }
    }
    diagnostics
}

/// The schedule's driver loop: step, repair, confirm.
///
/// Returns the deepest exact-valid layout it reached with its raw source depth,
/// plus the schedule's report. The parent is the floor of both, so the worst
/// this can return is the parent unchanged.
#[cfg(all(feature = "jagua-experimental", feature = "compression-schedule"))]
#[allow(clippy::type_complexity)]
fn drive_compression_schedule(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    schedule_settings: CompressionScheduleSettings,
    parent_placements: &[GeneralFastPlacement],
    parent_depth_mm: f64,
    requested_drop_mm: f64,
) -> Result<
    (
        Vec<GeneralFastPlacement>,
        f64,
        GeneralCompressionScheduleDiagnostics,
    ),
    GeneralFastError,
> {
    let incumbent = general_fast_result_seed(parent_placements.to_vec(), parent_depth_mm);
    let catalog_mode = if relaxed_settings.pressure_model
        == GeneralRelaxedPressureModel::DirectionalPenetration
    {
        SurrogateCatalogMode::CurrentAssignment
    } else if relaxed_settings.collision_backend == GeneralRelaxedCollisionBackend::RollbackTriangle
        || matches!(
            relaxed_settings.pressure_model,
            GeneralRelaxedPressureModel::StructuredTrianglePoles
        )
    {
        SurrogateCatalogMode::StructuredGrid
    } else {
        SurrogateCatalogMode::ZeroDegreeOnly
    };
    let (catalog, _) =
        build_surrogate_catalog(pieces, fast_settings, catalog_mode, Some(&incumbent))?;
    let mut state = initialize_complete_state(
        pieces,
        fast_settings,
        relaxed_settings.collision_backend,
        relaxed_settings.angle_seed_policy,
        relaxed_settings.pressure_model,
        &incumbent,
    )?;
    let mut search = LegacyLaneSearch::new(
        pieces,
        fast_settings,
        relaxed_settings,
        derive_seed(
            relaxed_settings.seed,
            0,
            COMPRESSION_SCHEDULE_SEED_DOMAIN as usize,
        ),
        catalog,
    );
    // The schedule walks the *strip* depth, which bounds the collision
    // polygons; the bound the caller supplied is a *source* depth, which
    // bounds the material. The two differ by the search envelope's expansion,
    // a constant on this request, so the schedule is given the same **drop**
    // rather than the same absolute number - which is what makes it a matched
    // arm against a mode-26 ladder asked for the same drop - and it starts at
    // the clamp the parent already occupies rather than at the number the
    // parent reports. See [`LaneSearch::tight_strip_depth`].
    let start_depth_mm = search.tight_strip_depth(&state)?;
    state.strip_depth_mm = start_depth_mm;
    let inset_mm = collision_sheet_inset_mm(fast_settings);
    let mut schedule = CompressionSchedule::new(
        schedule_settings,
        start_depth_mm,
        start_depth_mm - requested_drop_mm,
        inset_mm * 2.0,
    );
    // What one whole-layout validation costs the exact tier, so the schedule
    // can charge itself in the portfolio's currency without a counter.
    schedule.set_exact_pairs_per_confirmation(pieces.len() * pieces.len().saturating_sub(1) / 2);
    let steps_planned = schedule.steps_planned();
    let sweeps_per_step = schedule.sweeps_per_step();
    let repair_policy = schedule.repair_policy();
    let mut score = search.score_state(&state)?;
    // The proxy tier's opinion of the parent, recorded before anything moves.
    let parent_boundary_violations = score.boundary_violations;
    let parent_collision_pairs = score.collision_pairs.len();
    let parent_proxy_feasible = score.feasible();

    // The incumbent half of the asymmetry. It starts at the parent, so the
    // schedule's floor is the parent by construction.
    let mut confirmed_state = state.clone();
    let mut published_placements = parent_placements.to_vec();
    let mut published_depth_mm = parent_depth_mm;

    let mut rows = Vec::with_capacity(steps_planned.min(4_096));
    let mut confirmation_ms = 0.0;
    let mut repair_ms = 0.0;
    let mut global_sweep = 0usize;

    let unbounded_tail =
        schedule_settings.continue_past_bound && schedule_settings.work_cap_queries.is_some();
    while schedule.steps_taken() < steps_planned.max(1) || unbounded_tail {
        if !schedule.may_step(search.counters.surrogate_evaluations) {
            break;
        }
        let step = schedule.steps_taken();
        let queries_before = search.counters.surrogate_evaluations;
        schedule.step_down();
        // The step, taken here rather than left to the first sweep, so that the
        // residue below is the residue the *step* made rather than what one
        // sweep of repair left of it. `move_sweep` performs the same write, so
        // its schedule check finds the depth already in place and does nothing
        // - the write stays in the sweep because any other caller of a
        // schedule-armed lane needs it there.
        state.strip_depth_mm = schedule.depth_mm();
        search.refresh_boundary_rows(&state, &mut score)?;
        let before_violations = score.boundary_violations;
        let before_pairs = score.collision_pairs.len();
        let before_loss = score.boundary_loss;
        search.compression = Some(schedule);

        let repair_started = Instant::now();
        let mut sweeps_run = 0usize;
        for _ in 0..sweeps_per_step.max(1) {
            if score.feasible() {
                break;
            }
            search.move_sweep(&mut state, &mut score, global_sweep)?;
            global_sweep += 1;
            sweeps_run += 1;
            if score.feasible() {
                break;
            }
            update_weights(&mut search.weights, &score.collision_pairs);
            refresh_weighted_loss(&mut score, &search.weights);
        }
        repair_ms += repair_started.elapsed().as_secs_f64() * 1_000.0;
        schedule = search
            .compression
            .take()
            .expect("the lane keeps the schedule it was handed");

        let mut row = GeneralCompressionScheduleStepRow {
            step,
            depth_mm: schedule.depth_mm(),
            boundary_violations_before: before_violations,
            collision_pairs_before: before_pairs,
            boundary_loss_before: before_loss,
            boundary_violations_after: score.boundary_violations,
            collision_pairs_after: score.collision_pairs.len(),
            boundary_loss_after: score.boundary_loss,
            sweeps_run,
            candidate_queries: search
                .counters
                .surrogate_evaluations
                .saturating_sub(queries_before),
            proxy_feasible: score.feasible(),
            ..GeneralCompressionScheduleStepRow::default()
        };

        if schedule.due_for_confirmation(score.feasible()) {
            schedule.note_confirmation_attempt();
            let started = Instant::now();
            let placements = to_fast_placements(&state, pieces);
            let mut accepted = None;
            // Whether it was the *frontier itself* the validator accepted, as
            // opposed to something the repair pass made out of it. Only the
            // former may move the floor: the deepest-confirmed slot has to hold
            // a layout that is exact-valid *and* is the layout the lane is
            // holding, or a rollback would restore an infeasible state at a
            // depth the schedule believes was confirmed.
            let mut frontier_confirmed = false;
            match validate_and_measure_placements(pieces, &placements, fast_settings) {
                Ok(_) => {
                    accepted = Some(placements.clone());
                    frontier_confirmed = true;
                }
                Err(_) => {
                    schedule.note_refused();
                    row.confirmation_refused = true;
                    if repair_policy == CompressionRepairPolicy::MicroLegalizeOnReject {
                        let (_, repaired) = micro_legalize(pieces, &placements, fast_settings);
                        schedule.note_micro_legalization(repaired.is_some());
                        if let Some(repaired) = repaired {
                            row.micro_legalized = true;
                            accepted = Some(repaired);
                        }
                    }
                }
            }
            if let Some(accepted) = accepted {
                // The confirmation's own measurement, on the untouched source
                // rings: the only number this mode publishes on.
                if let Ok(raw_depth_mm) =
                    coupled_independent_source_depth(pieces, &accepted, fast_settings)
                {
                    row.raw_depth_mm = Some(raw_depth_mm);
                    if frontier_confirmed {
                        schedule.note_confirmed();
                        row.confirmed = true;
                        confirmed_state = state.clone();
                    }
                    if grid_key(raw_depth_mm) < grid_key(published_depth_mm) {
                        published_depth_mm = raw_depth_mm;
                        published_placements = accepted;
                    }
                }
            }
            confirmation_ms += started.elapsed().as_secs_f64() * 1_000.0;
        }

        if schedule.due_for_rollback() {
            // Both halves of the snapshot, restored in the same statement. The
            // depth the schedule returns to is its monotone floor, which is the
            // depth `confirmed_state` was confirmed at.
            schedule.rollback_to_floor();
            state = confirmed_state.clone();
            state.strip_depth_mm = schedule.depth_mm();
            search.weights.clear();
            score = search.score_state(&state)?;
            row.rolled_back = true;
        }

        rows.push(row);
    }

    // The last state the frontier reached never gets a scheduled confirmation
    // if the run ended between cadences, and it is the deepest one there is.
    if score.feasible() {
        let started = Instant::now();
        let placements = to_fast_placements(&state, pieces);
        if validate_and_measure_placements(pieces, &placements, fast_settings).is_ok() {
            if let Ok(raw_depth_mm) =
                coupled_independent_source_depth(pieces, &placements, fast_settings)
            {
                schedule.note_confirmation_attempt();
                schedule.note_confirmed();
                if grid_key(raw_depth_mm) < grid_key(published_depth_mm) {
                    published_depth_mm = raw_depth_mm;
                    published_placements = placements;
                }
            }
        }
        confirmation_ms += started.elapsed().as_secs_f64() * 1_000.0;
    }

    schedule.may_step(search.counters.surrogate_evaluations);
    let mut report = schedule.report();
    report.start_depth_mm = start_depth_mm;
    report.parent_boundary_violations = parent_boundary_violations;
    report.parent_collision_pairs = parent_collision_pairs;
    report.parent_proxy_feasible = parent_proxy_feasible;
    report.confirmation_ms = confirmation_ms;
    report.repair_ms = repair_ms;
    report.steps = rows;
    Ok((published_placements, published_depth_mm, report))
}

/// Mode 27: the standalone micro-legalization probe.
///
/// Takes the parent exactly as given, measures its residue against the real
/// request, and attempts the same repair pass mode 26 runs per rung. No bound,
/// no ladder, no search: this is the instrument for asking "how far is this
/// state from legal, and can projection close it?" of any state at all.
#[cfg(feature = "jagua-experimental")]
fn run_micro_legalization_probe(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    parent: &GeneralCoupledSeparatorArmDiagnostics,
    parent_source: Option<String>,
) -> GeneralPersistentVacancyDiagnostics {
    let mut diagnostics = GeneralPersistentVacancyDiagnostics {
        mode: 27,
        seed_domain: MICRO_LEGALIZATION_SEED_DOMAIN,
        parent_source,
        ..GeneralPersistentVacancyDiagnostics::default()
    };
    if pieces.is_empty() {
        diagnostics.failure_reason =
            Some("micro-legalization probe requires at least one piece".to_owned());
        return diagnostics;
    }
    if parent.final_placements.len() != pieces.len() {
        diagnostics.failure_reason =
            Some("micro-legalization probe requires a complete parent layout".to_owned());
        return diagnostics;
    }

    let parent_placements = fast_placements_from_coupled_diagnostics(&parent.final_placements);
    diagnostics.parent_fingerprint = Some(coupled_fast_placement_fingerprint(&parent_placements));
    diagnostics.initial_state_fingerprint = diagnostics.parent_fingerprint.clone();
    // Unlike every other mode, this one is *meant* to be pointed at states that
    // do not validate, so the parent is measured rather than gated on.
    diagnostics.parent_independent_depth_mm =
        coupled_independent_source_depth(pieces, &parent_placements, fast_settings).ok();
    diagnostics.attempted = true;

    let (micro_diagnostics, repaired) = micro_legalize(pieces, &parent_placements, fast_settings);
    diagnostics.micro_legalization = Some(micro_diagnostics);
    match repaired {
        Some(repaired) => {
            diagnostics.exact_valid = true;
            diagnostics.independent_depth_mm =
                coupled_independent_source_depth(pieces, &repaired, fast_settings).ok();
            diagnostics.final_placement_fingerprint =
                Some(coupled_fast_placement_fingerprint(&repaired));
            diagnostics.final_placements = coupled_placement_diagnostics(&repaired);
        }
        None => {
            diagnostics.failure_reason =
                Some("micro-legalization did not produce an exact-valid state".to_owned());
        }
    }
    diagnostics
}

/// Modes 30 and 31: the standalone global pressure-balanced legalization
/// probes, and the fourth mode-26 repair tier run on its own.
///
/// Mode 30 takes the parent exactly as given and solves it under the request's
/// own sheet - the direct global analogue of mode 27, and the instrument for
/// asking "how much displacement, distributed over the whole layout, does this
/// state actually need?". Mode 31 takes the same parent under an explicit depth
/// bound (CLI argument 45), which enters the program as a hard containment
/// constraint on every piece, and is the standalone form of the tier a mode-26
/// rung runs.
///
/// Like modes 27 to 29 both are deliberately pointed at states that do *not*
/// validate, so the parent is measured rather than gated on.
#[cfg(feature = "jagua-experimental")]
fn run_global_legalization_probe(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    parent: &GeneralCoupledSeparatorArmDiagnostics,
    parent_source: Option<String>,
    bounded: bool,
) -> GeneralPersistentVacancyDiagnostics {
    let mode = if bounded { 31 } else { 30 };
    let mut diagnostics = GeneralPersistentVacancyDiagnostics {
        mode,
        seed_domain: GLOBAL_LEGALIZATION_SEED_DOMAIN,
        parent_source,
        ..GeneralPersistentVacancyDiagnostics::default()
    };
    let bound_mm = if bounded {
        let Some(bound_mm) = relaxed_settings.persistent_vacancy_target_depth_mm else {
            diagnostics.failure_reason =
                Some("persistent vacancy mode 31 requires an explicit depth bound".to_owned());
            return diagnostics;
        };
        if !bound_mm.is_finite() || bound_mm <= 0.0 {
            diagnostics.failure_reason = Some(
                "persistent vacancy mode 31 depth bound must be a positive finite value".to_owned(),
            );
            return diagnostics;
        }
        diagnostics.target_depth_mm = bound_mm;
        Some(bound_mm)
    } else {
        None
    };
    if pieces.is_empty() {
        diagnostics.failure_reason =
            Some("global legalization requires at least one piece".to_owned());
        return diagnostics;
    }
    if parent.final_placements.len() != pieces.len() {
        diagnostics.failure_reason =
            Some("global legalization requires a complete parent layout".to_owned());
        return diagnostics;
    }

    let parent_placements = fast_placements_from_coupled_diagnostics(&parent.final_placements);
    diagnostics.parent_fingerprint = Some(coupled_fast_placement_fingerprint(&parent_placements));
    diagnostics.initial_state_fingerprint = diagnostics.parent_fingerprint.clone();
    diagnostics.parent_independent_depth_mm =
        coupled_independent_source_depth(pieces, &parent_placements, fast_settings).ok();
    diagnostics.attempted = true;

    let (global_diagnostics, repaired) =
        global_legalize(pieces, &parent_placements, fast_settings, bound_mm);
    diagnostics.cap_exhausted = global_diagnostics.cap_exhausted.clone();
    match repaired {
        Some(repaired) => {
            diagnostics.complete_states = 1;
            diagnostics.direct_insertions = global_diagnostics.moved_pieces;
            diagnostics.exact_valid = true;
            diagnostics.independent_depth_mm =
                coupled_independent_source_depth(pieces, &repaired, fast_settings).ok();
            diagnostics.final_placement_fingerprint =
                Some(coupled_fast_placement_fingerprint(&repaired));
            diagnostics.final_placements = coupled_placement_diagnostics(&repaired);
        }
        None => {
            diagnostics.publication_rejections = 1;
            diagnostics.failure_reason = Some(
                global_diagnostics
                    .skipped_reason
                    .clone()
                    .or_else(|| global_diagnostics.rejection_reason.clone())
                    .unwrap_or_else(|| {
                        "global legalization produced no exact-valid state".to_owned()
                    }),
            );
        }
    }
    diagnostics.global_legalization = Some(global_diagnostics);
    diagnostics
}

/// Runs one mode-26 rung from one warm start: the ordinary mode-0 pipeline
/// under the rung's clamped sheet, then the rung's own measurements of what
/// came back, always against the *real* request rather than the clamped one.
///
/// Returns the arm's diagnostics row plus, when the arm produced a usable
/// complete state, that state with its measured depth and exact validity.
#[allow(clippy::too_many_arguments)]
#[cfg(feature = "jagua-experimental")]
fn run_ladder_compression_arm(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    step_settings: GeneralFastSettings,
    separator_settings: GeneralRelaxedSettings,
    bound_mm: f64,
    seed_depth_mm: f64,
    role: &str,
    warm_start_source: String,
    warm_start_depth_mm: Option<f64>,
    warm_placements: Vec<GeneralFastPlacement>,
) -> (
    GeneralPersistentVacancyLadderArmDiagnostics,
    Option<LadderCompressionArmProduct>,
) {
    let mut row = GeneralPersistentVacancyLadderArmDiagnostics {
        role: role.to_owned(),
        warm_start_source,
        warm_start_depth_mm,
        ..GeneralPersistentVacancyLadderArmDiagnostics::default()
    };
    #[cfg(feature = "search-profiling")]
    let arm_region = LadderAnatomySample::open();
    let seed = general_fast_result_seed(warm_placements, seed_depth_mm);
    // The one place the tolerant rollback comparison is armed. A rung runs the
    // separator against a sheet that does not exist outside this ladder, so its
    // accepted states are this mode's alone to change; every other caller of
    // the pipeline keeps the bit-exact rule. See `CoupledRollbackComparison`
    // for why the exact rule was rejecting rollbacks that were correct.
    #[cfg(feature = "search-profiling")]
    let separator_started = Instant::now();
    let separator_outcome = improve_complete_layout_under_rollback_comparison(
        pieces,
        step_settings,
        separator_settings,
        &seed,
        None,
        None,
        CoupledRollbackComparison::ToleratesPoleRounding,
    );
    #[cfg(feature = "search-profiling")]
    {
        row.anatomy.separator_ms = separator_started.elapsed().as_secs_f64() * 1000.0;
    }
    let candidate = match separator_outcome {
        Ok(outcome) => {
            row.epochs_improved = outcome.diagnostics.epochs_improved;
            let arm = outcome
                .diagnostics
                .coupled_dynamic_separator
                .as_ref()
                .and_then(|coupled| coupled.boundary_projection_treatment.as_ref());
            match arm {
                Some(arm) => {
                    row.separator_attempted = true;
                    row.arm_final_depth_mm = Some(arm.final_depth_mm);
                    row.arm_targets_attempted = arm.targets_attempted;
                    row.arm_targets_accepted = arm.targets_accepted;
                    row.separator_skipped_reason = arm.skipped_reason.clone();
                    row.terminal_collision_pairs = arm.terminal_collision_pairs;
                    row.terminal_boundary_violations = arm.terminal_boundary_violations;
                    row.rollback_disagreements_tolerated =
                        arm.targets.iter().fold(0usize, |total, target| {
                            total.saturating_add(target.rollback_disagreements_tolerated)
                        });
                    row.rollback_disagreement_max_pressure_ulps = arm
                        .targets
                        .iter()
                        .map(|target| target.rollback_disagreement_max_pressure_ulps)
                        .max()
                        .unwrap_or(0);
                    row.aborted_by_rollback_disagreement = arm.targets.iter().any(|target| {
                        target
                            .failure_reason
                            .as_deref()
                            .is_some_and(|reason| reason.starts_with(ROLLBACK_DISAGREEMENT_ABORT))
                    });
                    row.rollback_comparison = arm.rollback_comparison.clone();
                    // The arm's exact-accepted state when it legalized
                    // something, otherwise its terminal minimum-loss state:
                    // the compressed-but-infeasible layout the arm reached and
                    // could not legalize is exactly the material a tighter
                    // rung is meant to keep working off.
                    let (placements, from_terminal) = if arm.targets_accepted > 0 {
                        (
                            fast_placements_from_coupled_diagnostics(&arm.final_placements),
                            false,
                        )
                    } else {
                        (
                            fast_placements_from_coupled_diagnostics(&arm.terminal_placements),
                            true,
                        )
                    };
                    if placements.len() == pieces.len() {
                        row.from_terminal = from_terminal;
                        Some(placements)
                    } else {
                        row.failure_reason = Some(
                            "clamped separator arm produced no usable complete state".to_owned(),
                        );
                        None
                    }
                }
                None => {
                    row.failure_reason = Some(
                        "clamped separator arm produced no boundary-projection arm".to_owned(),
                    );
                    None
                }
            }
        }
        Err(error) => {
            row.failure_reason = Some(error.to_string());
            None
        }
    };

    let Some(placements) = candidate else {
        #[cfg(feature = "search-profiling")]
        arm_region.close(&mut row.anatomy);
        return (row, None);
    };
    row.state_fingerprint = Some(coupled_fast_placement_fingerprint(&placements));
    let measured = ladder_time!(row.anatomy.depth_measure_ms, {
        coupled_independent_source_depth(pieces, &placements, fast_settings)
    });
    let depth_mm = match measured {
        Ok(depth) => {
            row.converged_depth_mm = Some(depth);
            row.bound_excess_mm = Some((depth - bound_mm).max(0.0));
            depth
        }
        Err(error) => {
            row.failure_reason = Some(error.to_string());
            #[cfg(feature = "search-profiling")]
            arm_region.close(&mut row.anatomy);
            return (row, None);
        }
    };
    row.overlap_pairs = ladder_time!(row.anatomy.overlap_count_ms, {
        count_exact_overlap_pairs(pieces, &placements).ok()
    });
    let validated = ladder_time!(row.anatomy.exact_validate_ms, {
        validate_and_measure_placements(pieces, &placements, fast_settings)
    });
    match validated {
        Ok(_) => row.exact_valid = true,
        Err(error) => row.exact_rejection_reason = Some(error.to_string()),
    }

    // The residue this mode keeps producing is a compressed state that misses
    // the contract by a handful of pairs, with no source overlap anywhere.
    //
    // Tier one treats that as a projection problem: where the deficits really
    // are at a rounding scale, the micro-legalizer nudges the offending pieces
    // onto the feasible side and the rung publishes. The pass validates its
    // own output against the real request, so anything it returns is
    // publishable.
    //
    // Tier two exists because most of this residue is *not* rounding-scale.
    // The measured terminal states miss by millimetres - miter-joined envelope
    // conflicts far from the material closest-approach point - and the
    // micro-legalizer correctly refuses them, because a millimetre of travel
    // is a search move and its model is translation-only. That residue needs
    // local *re-placement*, so the pieces incident to the conflict are ejected
    // and rebuilt by the construction insertion machinery under this rung's
    // own clamped sheet. It runs only when tier one produced nothing, and it
    // likewise validates its own output before returning it.
    //
    // Tier three exists because tier two ejects a *vertex cover*: it leaves one
    // endpoint of every conflict exactly where it was, so the re-placed piece
    // has to find room against occupancy that is itself part of the conflict.
    // The deep-frontier residue is multi-millimetre and sits in two- and
    // three-piece components where no such single-piece pose exists at all, and
    // tier two correctly refuses it. The joint pass ejects the whole component,
    // searches over insertion order, and can seed a coordinated exchange - and
    // like the other two it validates its own output before returning it. It
    // runs strictly after tier two has produced nothing, so it can only add
    // publications to rungs that were already failing.
    let mut legalized = None;
    let mut repair_tier = None;
    if !row.exact_valid {
        let (micro_diagnostics, repaired) = ladder_time!(row.anatomy.micro_legalization_ms, {
            micro_legalize(pieces, &placements, fast_settings)
        });
        if let Some(repaired) = repaired {
            match coupled_independent_source_depth(pieces, &repaired, fast_settings) {
                Ok(repaired_depth_mm) => {
                    row.micro_legalized_depth_mm = Some(repaired_depth_mm);
                    row.micro_legalized_fingerprint =
                        Some(coupled_fast_placement_fingerprint(&repaired));
                    legalized = Some((repaired, repaired_depth_mm));
                    repair_tier = Some(LADDER_REPAIR_TIER_MICRO);
                }
                Err(error) => {
                    row.failure_reason = Some(format!("micro-legalized state depth: {error}"));
                }
            }
        }
        row.micro_legalization = Some(micro_diagnostics);
    }
    if !row.exact_valid && legalized.is_none() {
        // Mode 26's repair tiers are protected legacy behaviour, so they stay
        // on the legacy candidate stream: the orientation perturbation is
        // reachable only through the modes that were built to measure it.
        let outcome = ladder_time!(row.anatomy.replacement_repair_ms, {
            persistent_vacancy::replacement_repair(
                pieces,
                &placements,
                fast_settings,
                bound_mm,
                false,
            )
        });
        if let Some(repaired) = &outcome.repaired {
            if let Some(repaired_depth_mm) = outcome.diagnostics.depth_mm {
                row.replacement_repaired_depth_mm = Some(repaired_depth_mm);
                row.replacement_repaired_fingerprint =
                    Some(coupled_fast_placement_fingerprint(repaired));
                legalized = Some((repaired.clone(), repaired_depth_mm));
                repair_tier = Some(LADDER_REPAIR_TIER_REPLACEMENT);
            }
        }
        row.replacement_repair = Some(outcome.diagnostics);
    }
    if !row.exact_valid && legalized.is_none() {
        let outcome = ladder_time!(row.anatomy.joint_replacement_ms, {
            persistent_vacancy::joint_replacement_repair(
                pieces,
                &placements,
                fast_settings,
                bound_mm,
                false,
            )
        });
        if let Some(repaired) = &outcome.repaired {
            if let Some(repaired_depth_mm) = outcome.diagnostics.depth_mm {
                row.joint_replaced_depth_mm = Some(repaired_depth_mm);
                row.joint_replaced_fingerprint = Some(coupled_fast_placement_fingerprint(repaired));
                legalized = Some((repaired.clone(), repaired_depth_mm));
                repair_tier = Some(LADDER_REPAIR_TIER_JOINT);
            }
        }
        row.joint_replacement = Some(outcome.diagnostics);
    }
    // Tier four. The three tiers above are all *local*: they move, eject or
    // re-place the pieces incident to the conflict and hold the rest of the
    // layout still. The per-component beam proved that on a deep frontier the
    // pieces of a violation component individually have no in-bound pose - so
    // the room the component needs is not inside it, and no amount of local
    // search will find it. The global pass makes every piece a variable, adds
    // the sheet and this rung's bound as hard constraints on all of them, and
    // asks for the minimum-norm correction of the whole linearized system at
    // once; pieces that violate nothing move to make room for pieces that do.
    // It runs strictly after the local tiers have produced nothing, so like
    // them it can only add publications to rungs that were already failing.
    if !row.exact_valid && legalized.is_none() {
        let (global_diagnostics, repaired) = ladder_time!(row.anatomy.global_legalization_ms, {
            global_legalize(pieces, &placements, fast_settings, Some(bound_mm))
        });
        if let Some(repaired) = repaired {
            match coupled_independent_source_depth(pieces, &repaired, fast_settings) {
                Ok(repaired_depth_mm) => {
                    row.global_legalized_depth_mm = Some(repaired_depth_mm);
                    row.global_legalized_fingerprint =
                        Some(coupled_fast_placement_fingerprint(&repaired));
                    legalized = Some((repaired, repaired_depth_mm));
                    repair_tier = Some(LADDER_REPAIR_TIER_GLOBAL);
                }
                Err(error) => {
                    row.failure_reason = Some(format!("globally legalized state depth: {error}"));
                }
            }
        }
        row.global_legalization = Some(global_diagnostics);
    }

    #[cfg(feature = "search-profiling")]
    arm_region.close(&mut row.anatomy);
    let exact_valid = row.exact_valid;
    (
        row,
        Some(LadderCompressionArmProduct {
            placements,
            depth_mm,
            exact_valid,
            legalized,
            repair_tier,
        }),
    )
}

/// What one mode-26 rung arm produced.
///
/// `placements` is the arm's own state and is generally *infeasible* - it is
/// the compression frontier the next rung works off. `legalized` is the
/// exact-valid state one of the two repair tiers recovered from it, when
/// either managed to, and is the arm's publication candidate whenever
/// `exact_valid` is false; `repair_tier` says which tier produced it.
#[cfg(feature = "jagua-experimental")]
struct LadderCompressionArmProduct {
    placements: Vec<GeneralFastPlacement>,
    depth_mm: f64,
    exact_valid: bool,
    legalized: Option<(Vec<GeneralFastPlacement>, f64)>,
    repair_tier: Option<&'static str>,
}

/// Mode 23: recombination. Crosses parent A (`pinned_vacancy_parent`) with
/// parent B (`secondary_pinned_vacancy_parent`, the warm-start slot) at a
/// scale-free cut fraction of parent A's own measured short-axis span, then
/// legalizes the resulting hybrid through the same separator treatment
/// mode 0 uses.
#[cfg(feature = "jagua-experimental")]
fn run_recombination(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    parent_a: &GeneralCoupledSeparatorArmDiagnostics,
    parent_a_source: Option<String>,
    parent_b: Option<&GeneralPersistentVacancyPinnedParent>,
) -> GeneralPersistentVacancyDiagnostics {
    let mut diagnostics = GeneralPersistentVacancyDiagnostics {
        mode: 23,
        seed_domain: RECOMBINATION_SEED_DOMAIN,
        parent_source: parent_a_source,
        ..GeneralPersistentVacancyDiagnostics::default()
    };
    let Some(cut_fraction) = relaxed_settings.persistent_vacancy_target_depth_mm else {
        diagnostics.failure_reason =
            Some("persistent vacancy mode 23 requires an explicit cut fraction".to_owned());
        return diagnostics;
    };
    if !(cut_fraction.is_finite() && cut_fraction > 0.0 && cut_fraction < 1.0) {
        diagnostics.failure_reason = Some(
            "persistent vacancy mode 23 cut fraction must be strictly between 0 and 1".to_owned(),
        );
        return diagnostics;
    }
    diagnostics.target_depth_mm = cut_fraction;
    let Some(parent_b) = parent_b else {
        diagnostics.failure_reason = Some(
            "persistent vacancy mode 23 requires a second parent fixture in the warm-start slot"
                .to_owned(),
        );
        return diagnostics;
    };
    if parent_a.final_placements.len() != pieces.len() {
        diagnostics.failure_reason =
            Some("persistent vacancy parent A is not a complete exact-valid layout".to_owned());
        return diagnostics;
    }
    if parent_b.placements.len() != pieces.len() {
        diagnostics.failure_reason =
            Some("persistent vacancy parent B is not a complete exact-valid layout".to_owned());
        return diagnostics;
    }

    let mut placements_a = BTreeMap::new();
    for placement in &parent_a.final_placements {
        if placements_a
            .insert(placement.piece_id.as_str(), placement)
            .is_some()
        {
            diagnostics.failure_reason = Some(format!(
                "persistent vacancy parent A has duplicate piece {}",
                placement.piece_id
            ));
            return diagnostics;
        }
    }
    let mut placements_b = BTreeMap::new();
    for placement in &parent_b.placements {
        if placements_b
            .insert(placement.piece_id.as_str(), placement)
            .is_some()
        {
            diagnostics.failure_reason = Some(format!(
                "persistent vacancy parent B has duplicate piece {}",
                placement.piece_id
            ));
            return diagnostics;
        }
    }
    let piece_ids = pieces.iter().map(|piece| piece.id).collect::<BTreeSet<_>>();
    let ids_a = placements_a.keys().copied().collect::<BTreeSet<_>>();
    let ids_b = placements_b.keys().copied().collect::<BTreeSet<_>>();
    if ids_a != piece_ids || ids_b != piece_ids {
        diagnostics.failure_reason = Some(
            "persistent vacancy mode 23 requires both parents to cover exactly the request's pieceId set"
                .to_owned(),
        );
        return diagnostics;
    }

    diagnostics.attempted = true;
    let short_axis_values = pieces
        .iter()
        .map(|piece| {
            placements_a
                .get(piece.id)
                .expect("validated piece id set")
                .translate_short_axis
        })
        .collect::<Vec<_>>();
    let min_short = short_axis_values
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    let max_short = short_axis_values
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    if !min_short.is_finite() || !max_short.is_finite() || max_short <= min_short {
        diagnostics.failure_reason = Some(
            "persistent vacancy mode 23 requires parent A to span a positive short-axis range"
                .to_owned(),
        );
        return diagnostics;
    }
    let threshold = min_short + cut_fraction * (max_short - min_short);

    let mut hybrid = Vec::with_capacity(pieces.len());
    let mut pieces_from_parent_a = 0usize;
    let mut pieces_from_parent_b = 0usize;
    for piece in pieces {
        let pose_a = placements_a.get(piece.id).expect("validated piece id set");
        let (rotation_deg, mirrored, translate_short_axis, translate_long_axis) =
            if pose_a.translate_short_axis < threshold {
                pieces_from_parent_a += 1;
                (
                    pose_a.rotation_deg,
                    pose_a.mirrored,
                    pose_a.translate_short_axis,
                    pose_a.translate_long_axis,
                )
            } else {
                pieces_from_parent_b += 1;
                let pose_b = placements_b.get(piece.id).expect("validated piece id set");
                (
                    pose_b.rotation_deg,
                    pose_b.mirrored,
                    pose_b.translate_short_axis,
                    pose_b.translate_long_axis,
                )
            };
        hybrid.push(GeneralFastPlacement {
            piece_id: piece.id.to_owned(),
            rotation_deg,
            mirrored,
            translate_short_axis,
            translate_long_axis,
        });
    }
    diagnostics.parent_fingerprint = Some(coupled_fast_placement_fingerprint(
        &fast_placements_from_coupled_diagnostics(&parent_a.final_placements),
    ));
    diagnostics.initial_state_fingerprint = Some(coupled_fast_placement_fingerprint(&hybrid));

    let hybrid_overlap_pairs = count_exact_overlap_pairs(pieces, &hybrid).ok();
    let hybrid_independent_depth_mm =
        match coupled_independent_source_depth(pieces, &hybrid, fast_settings) {
            Ok(depth) => depth,
            Err(error) => {
                diagnostics.failure_reason = Some(format!("recombination hybrid depth: {error}"));
                return diagnostics;
            }
        };
    // The legalization pipeline (both the epoch lane search and the coupled
    // separator's target loop) only ever accepts a *strict* depth decrease
    // at a fixed piece count, so a seed incumbent pinned exactly at the raw
    // hybrid's tight geometric bound can never be published even if a
    // seam-clearing rearrangement exists at that same footprint. Seeding a
    // looser nominal depth gives the strict-improvement search room to find
    // and publish that rearrangement. The headroom is derived, not tuned:
    // it is the depth the fixed epoch budget could theoretically contract
    // back down to the hybrid's own tight bound, using the run's own
    // existing epoch count and shrink ratio (no new absolute-mm literal).
    let epoch_contraction_reach =
        (1.0 - relaxed_settings.initial_shrink_ratio).powf(relaxed_settings.epochs as f64);
    let seed_depth_mm = if epoch_contraction_reach.is_finite() && epoch_contraction_reach > 0.0 {
        hybrid_independent_depth_mm / epoch_contraction_reach
    } else {
        hybrid_independent_depth_mm
    };
    let seed_incumbent = general_fast_result_seed(hybrid, seed_depth_mm);

    let mut legalize_settings = relaxed_settings;
    legalize_settings.persistent_vacancy_mode = 0;
    let legalized = match improve_complete_layout_with_pinned_vacancy_parent(
        pieces,
        fast_settings,
        legalize_settings,
        &seed_incumbent,
        None,
        None,
    ) {
        Ok(outcome) => outcome,
        Err(error) => {
            diagnostics.failure_reason = Some(format!("recombination legalization: {error}"));
            return diagnostics;
        }
    };
    let Some(legal_arm) = legalized
        .diagnostics
        .coupled_dynamic_separator
        .as_ref()
        .and_then(|coupled| coupled.boundary_projection_treatment.as_ref())
    else {
        diagnostics.failure_reason =
            Some("recombination legalization produced no boundary-projection arm".to_owned());
        return diagnostics;
    };
    let final_placements = fast_placements_from_coupled_diagnostics(&legal_arm.final_placements);
    let exact_valid =
        validate_and_measure_placements(pieces, &final_placements, fast_settings).is_ok();
    let independent_depth_mm =
        coupled_independent_source_depth(pieces, &final_placements, fast_settings).ok();

    diagnostics.exact_valid = exact_valid;
    diagnostics.independent_depth_mm = independent_depth_mm;
    diagnostics.final_placement_fingerprint =
        Some(coupled_fast_placement_fingerprint(&final_placements));
    diagnostics.final_placements = coupled_placement_diagnostics(&final_placements);
    diagnostics.recombination = Some(GeneralPersistentVacancyRecombinationDiagnostics {
        cut_fraction,
        short_axis_threshold_mm: threshold,
        pieces_from_parent_a,
        pieces_from_parent_b,
        hybrid_overlap_pairs,
        hybrid_independent_depth_mm,
        legalization_seed_depth_mm: seed_depth_mm,
        legalized_depth_mm: legal_arm.final_depth_mm,
    });
    diagnostics
}

fn coupled_separator_configuration_error(settings: GeneralRelaxedSettings) -> Option<String> {
    let angular_disabled = settings.angular_repair.neighborhood_size == 0
        && settings.angular_repair.successors == 0
        && settings.angular_repair.complete_query_budget == 0
        && settings.angular_repair.retained_confirmation_budget == 0
        && settings.angular_repair.early_stop_queries == 0;
    if settings.collision_backend != GeneralRelaxedCollisionBackend::RollbackTriangle
        || settings.angle_seed_policy != GeneralRelaxedAngleSeedPolicy::StructuredGrid
        || settings.pressure_model != GeneralRelaxedPressureModel::StructuredTrianglePoles
        || settings.lanes != COUPLED_SEPARATOR_WORKERS
        || settings.sweeps_per_epoch != COUPLED_SEPARATOR_ROUNDS
        || settings.global_samples_per_move != 10
        || settings.focused_samples_per_move != 10
        || settings.refinement_rounds != 5
        || settings.synchronize_lanes
        || !angular_disabled
    {
        return Some(
            "coupled dynamic separator requires the protected 8-lane, 40-sweep, 10/10-sample, 5-refinement structured route"
                .to_owned(),
        );
    }
    None
}

fn skipped_coupled_separator_arm(
    arm: CoupledSeparatorArm,
    protected: &GeneralFastResult,
    reason: String,
) -> GeneralCoupledSeparatorArmDiagnostics {
    GeneralCoupledSeparatorArmDiagnostics {
        pressure_model: arm.label().to_owned(),
        initial_depth_mm: protected.used_long_axis_depth_mm,
        final_depth_mm: protected.used_long_axis_depth_mm,
        final_placement_fingerprint: Some(coupled_fast_placement_fingerprint(
            &protected.placements,
        )),
        final_placements: coupled_placement_diagnostics(&protected.placements),
        skipped_reason: Some(reason),
        ..GeneralCoupledSeparatorArmDiagnostics::default()
    }
}

#[allow(clippy::too_many_arguments)]
#[cfg(feature = "jagua-experimental")]
fn run_coupled_separator_arm<'a>(
    pieces: &'a [GeneralFastPiece<'a>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    protected: &GeneralFastResult,
    arm: CoupledSeparatorArm,
    terminal_policy: CoupledTerminalPolicy,
    rollback_comparison: CoupledRollbackComparison,
    catalog: Arc<SurrogateCatalog>,
) -> CoupledArmOutcome {
    let mut diagnostics = GeneralCoupledSeparatorArmDiagnostics {
        pressure_model: arm.label().to_owned(),
        attempted: true,
        initial_depth_mm: protected.used_long_axis_depth_mm,
        final_depth_mm: protected.used_long_axis_depth_mm,
        independently_measured_final_depth_mm: coupled_independent_source_depth(
            pieces,
            &protected.placements,
            fast_settings,
        )
        .ok(),
        final_placement_fingerprint: Some(coupled_fast_placement_fingerprint(
            &protected.placements,
        )),
        final_placements: coupled_placement_diagnostics(&protected.placements),
        rollback_comparison: rollback_comparison.label().map(str::to_owned),
        ..GeneralCoupledSeparatorArmDiagnostics::default()
    };
    let experiment_seed = relaxed_settings.seed ^ COUPLED_SEPARATOR_SEED_DOMAIN;
    let mut incumbent = protected.clone();
    let mut checkpoint = None;
    let hazard_catalog = match JaguaHazardCatalog::new(pieces, fast_settings) {
        Ok(catalog) => Arc::new(catalog),
        Err(error) => {
            diagnostics.skipped_reason = Some(format!("dynamic hazard catalog: {error}"));
            return CoupledArmOutcome {
                diagnostics,
                checkpoint,
            };
        }
    };
    diagnostics.catalog_builds = 1;
    diagnostics.immutable_variant_builds = hazard_catalog.immutable_variant_count();

    for target_ordinal in 0..COUPLED_SEPARATOR_TARGETS {
        let target_seed = derive_seed(experiment_seed, target_ordinal, usize::MAX - 64);
        let compression_seed = derive_seed(target_seed, 0, usize::MAX - 63);
        let worker_seeds = (0..COUPLED_SEPARATOR_WORKERS)
            .map(|worker| derive_seed(target_seed, 0, worker))
            .collect::<Vec<_>>();
        let target_depth_mm =
            (incumbent.used_long_axis_depth_mm * (1.0 - COUPLED_SEPARATOR_CONTRACTION_RATIO)).max(
                area_depth_lower_bound(pieces, fast_settings)
                    .unwrap_or(incumbent.used_long_axis_depth_mm),
            );
        if target_depth_mm >= incumbent.used_long_axis_depth_mm {
            diagnostics.skipped_reason =
                Some("area lower bound prevents another contraction".to_owned());
            break;
        }
        diagnostics.targets_attempted = diagnostics.targets_attempted.saturating_add(1);
        let compression_split_mm = incumbent.used_long_axis_depth_mm * 0.5;
        let base_state = match initialize_complete_state(
            pieces,
            fast_settings,
            GeneralRelaxedCollisionBackend::DynamicHazard,
            GeneralRelaxedAngleSeedPolicy::ContinuousUniform,
            arm.pressure_model(),
            &incumbent,
        )
        .and_then(|state| {
            compress_state_at_split(&state, target_depth_mm, compression_split_mm, pieces)
        }) {
            Ok(state) => state,
            Err(error) => {
                let failure_reason = format!("target initialization: {error}");
                let incumbent_fingerprint =
                    coupled_fast_placement_fingerprint(&incumbent.placements);
                diagnostics
                    .targets
                    .push(GeneralCoupledSeparatorTargetDiagnostics {
                        ordinal: target_ordinal,
                        target_depth_mm,
                        compression_split_mm,
                        target_seed,
                        compression_seed,
                        worker_seeds,
                        initial_state_fingerprint: incumbent_fingerprint.clone(),
                        final_state_fingerprint: incumbent_fingerprint,
                        rounds: 0,
                        strikes: 0,
                        rollbacks: 0,
                        full_rescore_agreements: 0,
                        rollback_disagreements_tolerated: 0,
                        rollback_disagreement_max_pressure_ulps: 0,
                        initial_raw_loss: 0.0,
                        minimum_raw_loss: 0.0,
                        final_raw_loss: 0.0,
                        final_weighted_loss: 0.0,
                        feasible: false,
                        exact_valid: false,
                        exact_accepted: false,
                        exact_rejection_reason: None,
                        accepted_depth_mm: None,
                        boundary_projection: None,
                        cap_exhausted: None,
                        failure_reason: Some(failure_reason.clone()),
                    });
                diagnostics.skipped_reason = Some(failure_reason);
                break;
            }
        };

        let mut arm_settings = relaxed_settings;
        arm_settings.collision_backend = GeneralRelaxedCollisionBackend::DynamicHazard;
        arm_settings.angle_seed_policy = GeneralRelaxedAngleSeedPolicy::ContinuousUniform;
        arm_settings.pressure_model = arm.pressure_model();
        arm_settings.angular_repair = GeneralAngularRepairSettings::disabled();
        arm_settings.synchronize_lanes = true;
        arm_settings.sweeps_per_epoch = COUPLED_SEPARATOR_ROUNDS;

        let target = run_coupled_separator_target(
            pieces,
            fast_settings,
            arm_settings,
            &incumbent,
            base_state,
            target_ordinal,
            target_depth_mm,
            compression_split_mm,
            target_seed,
            compression_seed,
            worker_seeds,
            arm,
            CoupledRollbackRescorePolicy::StrictDerivedAgreement,
            rollback_comparison,
            false,
            catalog.clone(),
            hazard_catalog.clone(),
        );
        let CoupledTargetOutcome {
            diagnostics: mut target_diagnostics,
            mut accepted,
            work: counters,
            minimum,
            ..
        } = match target {
            Ok(outcome) => outcome,
            Err(error) => {
                diagnostics.skipped_reason = Some(format!("target {target_ordinal}: {error}"));
                break;
            }
        };
        diagnostics.worker_sweeps = diagnostics
            .worker_sweeps
            .saturating_add(counters.worker_sweeps);
        diagnostics.dynamic_queries = diagnostics
            .dynamic_queries
            .saturating_add(counters.dynamic_queries);
        diagnostics.pressure_evaluations = diagnostics
            .pressure_evaluations
            .saturating_add(counters.pressure_evaluations);
        diagnostics.retained_confirmations = diagnostics
            .retained_confirmations
            .saturating_add(counters.retained_confirmations);
        diagnostics.hazard_updates = diagnostics
            .hazard_updates
            .saturating_add(counters.hazard_updates);
        diagnostics.layout_loads = diagnostics
            .layout_loads
            .saturating_add(counters.layout_loads);
        diagnostics.index_builds = diagnostics
            .index_builds
            .saturating_add(counters.index_builds);
        diagnostics.worker_full_score_pair_visits = diagnostics
            .worker_full_score_pair_visits
            .saturating_add(counters.worker_full_score_pair_visits);
        diagnostics.auditor_full_score_pair_visits = diagnostics
            .auditor_full_score_pair_visits
            .saturating_add(counters.auditor_full_score_pair_visits);
        diagnostics.auditor_dynamic_queries = diagnostics
            .auditor_dynamic_queries
            .saturating_add(counters.auditor_dynamic_queries);
        diagnostics.auditor_pressure_evaluations = diagnostics
            .auditor_pressure_evaluations
            .saturating_add(counters.auditor_pressure_evaluations);
        diagnostics.auditor_layout_loads = diagnostics
            .auditor_layout_loads
            .saturating_add(counters.auditor_layout_loads);
        diagnostics.auditor_index_builds = diagnostics
            .auditor_index_builds
            .saturating_add(counters.auditor_index_builds);
        if accepted.is_none()
            && terminal_policy == CoupledTerminalPolicy::ExactBoundaryProjection
            && target_diagnostics.failure_reason.is_none()
            && target_diagnostics.cap_exhausted.is_none()
        {
            if let Some(minimum) = minimum.as_ref() {
                let projection_checkpoint = CoupledFailedCheckpoint {
                    incumbent: incumbent.clone(),
                    target_ordinal,
                    target_depth_mm,
                    compression_split_mm,
                    target_seed,
                    compression_seed,
                    catalog: catalog.clone(),
                    hazard_catalog: hazard_catalog.clone(),
                    minimum: minimum.clone(),
                    attempt_diagnostics: target_diagnostics.clone(),
                };
                let (projection_diagnostics, projected) =
                    try_exact_boundary_projection(&projection_checkpoint, pieces, fast_settings);
                target_diagnostics.boundary_projection = Some(projection_diagnostics);
                accepted = projected;
            }
        }
        let cap_exhausted = target_diagnostics.cap_exhausted.is_some();
        let target_failure = target_diagnostics.failure_reason.clone();
        diagnostics.targets.push(target_diagnostics.clone());
        if let Some(reason) = target_failure {
            diagnostics.skipped_reason = Some(format!("target {target_ordinal}: {reason}"));
            break;
        }
        if cap_exhausted {
            diagnostics.skipped_reason = Some(format!(
                "target {target_ordinal} crossed an atomic experiment cap"
            ));
            break;
        }
        let Some(accepted) = accepted else {
            if let Some(minimum) = minimum {
                checkpoint = Some(CoupledFailedCheckpoint {
                    incumbent: incumbent.clone(),
                    target_ordinal,
                    target_depth_mm,
                    compression_split_mm,
                    target_seed,
                    compression_seed,
                    catalog: catalog.clone(),
                    hazard_catalog: hazard_catalog.clone(),
                    minimum,
                    attempt_diagnostics: target_diagnostics,
                });
            }
            break;
        };
        diagnostics.targets_accepted = diagnostics.targets_accepted.saturating_add(1);
        diagnostics.final_depth_mm = accepted.used_long_axis_depth_mm;
        diagnostics.independently_measured_final_depth_mm =
            coupled_independent_source_depth(pieces, &accepted.placements, fast_settings).ok();
        diagnostics.final_placement_fingerprint =
            Some(coupled_fast_placement_fingerprint(&accepted.placements));
        diagnostics.final_placements = coupled_placement_diagnostics(&accepted.placements);
        incumbent = accepted;
    }
    // Record the terminal state of a failed last target. Pure bookkeeping: the
    // checkpoint is already computed above and nothing below reads these
    // fields back, so every arm's control flow, acceptance and reported depths
    // are unchanged.
    if let Some(checkpoint) = checkpoint.as_ref() {
        diagnostics.terminal_placements =
            relaxed_placement_diagnostics(&checkpoint.minimum.state, pieces);
        diagnostics.terminal_strip_depth_mm = Some(checkpoint.minimum.state.strip_depth_mm);
        diagnostics.terminal_collision_pairs = Some(checkpoint.minimum.score.collision_pairs.len());
        diagnostics.terminal_boundary_violations =
            Some(checkpoint.minimum.score.boundary_violations);
    }
    CoupledArmOutcome {
        diagnostics,
        checkpoint,
    }
}

/// Renders a relaxed state's poses in the shared placement-diagnostics shape.
#[cfg(feature = "jagua-experimental")]
fn relaxed_placement_diagnostics(
    state: &RelaxedState,
    pieces: &[GeneralFastPiece<'_>],
) -> Vec<GeneralCoupledSeparatorPlacementDiagnostics> {
    state
        .placements
        .iter()
        .filter_map(|placement| {
            pieces.get(placement.input_index).map(|piece| {
                GeneralCoupledSeparatorPlacementDiagnostics {
                    piece_id: piece.id.to_owned(),
                    rotation_deg: placement.rotation_deg,
                    mirrored: placement.mirrored,
                    translate_short_axis: placement.translate_x,
                    translate_long_axis: placement.translate_y,
                }
            })
        })
        .collect()
}

#[cfg(feature = "jagua-experimental")]
fn try_exact_boundary_projection(
    checkpoint: &CoupledFailedCheckpoint,
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
) -> (
    GeneralBoundaryProjectionDiagnostics,
    Option<GeneralFastResult>,
) {
    let mut diagnostics = GeneralBoundaryProjectionDiagnostics {
        attempted: true,
        ..GeneralBoundaryProjectionDiagnostics::default()
    };
    let minimum = &checkpoint.minimum;
    if minimum.score.boundary_loss <= 0.0 {
        diagnostics.rejection_reason =
            Some("terminal state has no positive boundary loss".to_owned());
        return (diagnostics, None);
    }
    if !minimum.score.collision_pairs.is_empty() {
        diagnostics.rejection_reason = Some(
            "terminal state is not boundary-only; exact projection is intentionally narrow"
                .to_owned(),
        );
        return (diagnostics, None);
    }
    let mut projected = minimum.state.clone();
    projected.strip_depth_mm = checkpoint.target_depth_mm;
    let projections = match select_all_exact_boundary_projections(
        &projected,
        &minimum.score,
        pieces,
        fast_settings,
    ) {
        Ok(projections) if !projections.is_empty() => projections,
        Ok(_) => {
            diagnostics.rejection_reason =
                Some("terminal state has no projectable boundary pieces".to_owned());
            return (diagnostics, None);
        }
        Err(reason) => {
            diagnostics.rejection_reason = Some(reason);
            return (diagnostics, None);
        }
    };
    let (root, root_placement) = &projections[0];
    diagnostics.root_piece_id = Some(pieces[*root].id.to_owned());
    diagnostics.root_boundary_loss = Some(minimum.score.boundaries[*root].raw_loss);
    diagnostics.projected_pose = Some(GeneralCoupledSeparatorPlacementDiagnostics {
        piece_id: pieces[*root].id.to_owned(),
        rotation_deg: root_placement.rotation_deg,
        mirrored: root_placement.mirrored,
        translate_short_axis: root_placement.translate_x,
        translate_long_axis: root_placement.translate_y,
    });
    diagnostics.projected_pieces = projections
        .iter()
        .map(
            |(piece_index, placement)| GeneralCoupledSeparatorPlacementDiagnostics {
                piece_id: pieces[*piece_index].id.to_owned(),
                rotation_deg: placement.rotation_deg,
                mirrored: placement.mirrored,
                translate_short_axis: placement.translate_x,
                translate_long_axis: placement.translate_y,
            },
        )
        .collect();
    for (piece_index, placement) in projections {
        projected.placements[piece_index] = placement;
    }
    diagnostics.state_fingerprint = Some(coupled_state_fingerprint(&projected));
    let placements = to_fast_placements(&projected, pieces);
    let metrics = match validate_and_measure_placements(pieces, &placements, fast_settings) {
        Ok(metrics) => metrics,
        Err(error) => {
            diagnostics.rejection_reason = Some(error.to_string());
            return (diagnostics, None);
        }
    };
    diagnostics.exact_valid = true;
    diagnostics.exact_depth_mm = Some(metrics.used_long_axis_depth_mm);
    if metrics.used_long_axis_depth_mm >= checkpoint.incumbent.used_long_axis_depth_mm {
        diagnostics.rejection_reason =
            Some("exact-valid projection did not improve the incumbent".to_owned());
        return (diagnostics, None);
    }

    diagnostics.exact_accepted = true;
    let mut result = checkpoint.incumbent.clone();
    result.placements = placements;
    result.unplaced_piece_ids.clear();
    result.used_short_axis_span_mm = metrics.used_short_axis_span_mm;
    result.used_long_axis_depth_mm = metrics.used_long_axis_depth_mm;
    result.unused_short_axis_projection_mm = metrics.unused_short_axis_projection_mm;
    result.occupied_envelope_area_mm2 = metrics.occupied_envelope_area_mm2;
    (diagnostics, Some(result))
}

#[cfg(feature = "jagua-experimental")]
fn run_precompression_frontier_vacancy_experiment<'a>(
    pieces: &'a [GeneralFastPiece<'a>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    boundary_projection: &CoupledArmOutcome,
    mode: usize,
) -> GeneralPrecompressionFrontierVacancyDiagnostics {
    let mut diagnostics = GeneralPrecompressionFrontierVacancyDiagnostics {
        mode,
        attempted: true,
        ..GeneralPrecompressionFrontierVacancyDiagnostics::default()
    };
    if mode == 3 {
        diagnostics.validation_counts = Some(GeneralPrecompressionValidationDiagnostics::default());
    }
    let Some(checkpoint) = boundary_projection.checkpoint.as_ref() else {
        diagnostics.skipped_reason =
            Some("exact-boundary arm retained no failed checkpoint".to_owned());
        return diagnostics;
    };
    diagnostics.target_depth_mm = Some(checkpoint.target_depth_mm);
    diagnostics.incumbent_strip_depth_mm = Some(checkpoint.incumbent.used_long_axis_depth_mm);
    diagnostics.checkpoint_fingerprint = Some(conflict_ruin_checkpoint_fingerprint(checkpoint));
    diagnostics.control = Some(checkpoint.attempt_diagnostics.clone());
    if checkpoint.attempt_diagnostics.failure_reason.is_some()
        || checkpoint.attempt_diagnostics.cap_exhausted.is_some()
    {
        diagnostics.skipped_reason = Some("failed target was not an uncapped outcome".to_owned());
        return diagnostics;
    }
    if checkpoint.target_depth_mm >= checkpoint.incumbent.used_long_axis_depth_mm {
        diagnostics.skipped_reason =
            Some("failed target does not contract the incumbent collision strip".to_owned());
        return diagnostics;
    }
    if let Err(error) =
        validate_and_measure_placements(pieces, &checkpoint.incumbent.placements, fast_settings)
    {
        diagnostics.skipped_reason = Some(format!("incumbent publication validation: {error}"));
        return diagnostics;
    }
    if let Some(counts) = diagnostics.validation_counts.as_mut() {
        counts.incumbent = 1;
        counts.total = 1;
    }

    let failed_score = match precompression_full_score(
        pieces,
        fast_settings,
        relaxed_settings,
        checkpoint,
        &checkpoint.minimum.state,
        &mut diagnostics,
    ) {
        Ok(score) => score,
        Err(reason) => {
            diagnostics.skipped_reason = Some(reason);
            return diagnostics;
        }
    };
    // The precompression frontier experiment is not on the mode-26 path (its
    // own CLI mode gates it), so it keeps the bit-exact rule.
    if let Some(disagreement) = raw_tracker_disagreement(
        &failed_score,
        &checkpoint.minimum.score,
        CoupledRollbackComparison::Exact,
        &mut RollbackComparisonTally::default(),
    ) {
        diagnostics.skipped_reason = Some(format!(
            "failed checkpoint disagrees with a complete rescore: {disagreement}"
        ));
        return diagnostics;
    }
    if failed_score.boundary_loss <= 0.0 || !failed_score.collision_pairs.is_empty() {
        diagnostics.skipped_reason =
            Some("failed checkpoint is not strictly boundary-only".to_owned());
        return diagnostics;
    }
    let projections = match select_all_exact_boundary_projections(
        &checkpoint.minimum.state,
        &failed_score,
        pieces,
        fast_settings,
    ) {
        Ok(projections) if projections.len() == CONFLICT_RUIN_REMOVED_PIECES => projections,
        Ok(projections) => {
            diagnostics.skipped_reason = Some(format!(
                "selector requires exactly {CONFLICT_RUIN_REMOVED_PIECES} boundary offenders, found {}",
                projections.len()
            ));
            return diagnostics;
        }
        Err(reason) => {
            diagnostics.skipped_reason = Some(format!("boundary selector: {reason}"));
            return diagnostics;
        }
    };
    let removal_order = projections
        .iter()
        .map(|(piece_index, _)| *piece_index)
        .collect::<Vec<_>>();
    diagnostics.selected_piece_ids = removal_order
        .iter()
        .map(|piece_index| pieces[*piece_index].id.to_owned())
        .collect();

    let parent_state = match initialize_complete_state(
        pieces,
        fast_settings,
        GeneralRelaxedCollisionBackend::DynamicHazard,
        GeneralRelaxedAngleSeedPolicy::ContinuousUniform,
        GeneralRelaxedPressureModel::DynamicPoles,
        &checkpoint.incumbent,
    ) {
        Ok(state) => state,
        Err(error) => {
            diagnostics.skipped_reason = Some(format!("incumbent state: {error}"));
            return diagnostics;
        }
    };
    let incumbent_fingerprint = coupled_state_fingerprint(&parent_state);
    diagnostics.incumbent_parent_fingerprint = Some(incumbent_fingerprint.clone());
    let rebuild_started = Instant::now();
    let rebuilt = build_conflict_ruin_states(
        &parent_state,
        parent_state.strip_depth_mm,
        &checkpoint.hazard_catalog,
        pieces,
        fast_settings,
        relaxed_settings.seed ^ PRECOMPRESSION_FRONTIER_SEED_DOMAIN,
        &removal_order,
        &mut diagnostics.rebuild,
    );
    diagnostics.rebuild.elapsed_ms = rebuild_started.elapsed().as_secs_f64() * 1_000.0;
    let rebuilt = match rebuilt {
        Ok(rebuilt) => rebuilt,
        Err(reason) => {
            diagnostics.rebuild.cap_exhausted = reason
                .strip_prefix("cap: ")
                .map(str::to_owned)
                .or_else(|| diagnostics.rebuild.cap_exhausted.clone());
            diagnostics.skipped_reason = Some(reason);
            return diagnostics;
        }
    };

    let mut exact_parent_candidates = Vec::new();
    let mut infeasible_candidates = Vec::new();
    for (beam_ordinal, child) in rebuilt
        .into_iter()
        .take(CONFLICT_RUIN_BEAM_WIDTH)
        .enumerate()
    {
        let state = child.state;
        let fingerprint = coupled_state_fingerprint(&state);
        let score = match precompression_full_score(
            pieces,
            fast_settings,
            relaxed_settings,
            checkpoint,
            &state,
            &mut diagnostics,
        ) {
            Ok(score) => score,
            Err(reason) => {
                diagnostics.skipped_reason = Some(reason);
                return diagnostics;
            }
        };
        let placements = to_fast_placements(&state, pieces);
        let publication = validate_and_measure_placements(pieces, &placements, fast_settings);
        if let Some(counts) = diagnostics.validation_counts.as_mut() {
            counts.rebuilt_children = counts.rebuilt_children.saturating_add(1);
            counts.total = counts.total.saturating_add(1);
        }
        diagnostics
            .rebuilt_children
            .push(GeneralPrecompressionFrontierChildDiagnostics {
                beam_ordinal,
                fingerprint: fingerprint.clone(),
                exact_overlap_area_mm2: child.score.total_overlap_area_mm2,
                exact_positive_overlap_pairs: child.score.positive_overlap_pairs,
                frontier_depth_mm: child.score.frontier_depth_mm,
                fresh_raw_loss: score.common_loss(),
                fresh_positive_pairs: score.collision_pairs.len(),
                fresh_feasible: score.feasible(),
                publication_valid: publication.is_ok(),
                publication_rejection_reason: publication.as_ref().err().map(ToString::to_string),
            });
        if fingerprint == incumbent_fingerprint {
            continue;
        }
        match publication {
            Ok(metrics) => {
                let compressed = match compress_state_at_split(
                    &state,
                    checkpoint.target_depth_mm,
                    checkpoint.compression_split_mm,
                    pieces,
                ) {
                    Ok(compressed) => compressed,
                    Err(error) => {
                        diagnostics.skipped_reason = Some(format!("parent compression: {error}"));
                        return diagnostics;
                    }
                };
                let compressed_score = match precompression_full_score(
                    pieces,
                    fast_settings,
                    relaxed_settings,
                    checkpoint,
                    &compressed,
                    &mut diagnostics,
                ) {
                    Ok(score) => score,
                    Err(reason) => {
                        diagnostics.skipped_reason = Some(reason);
                        return diagnostics;
                    }
                };
                exact_parent_candidates.push(PrecompressionExactParentCandidate {
                    compressed,
                    metrics,
                    compressed_raw_loss: compressed_score.common_loss(),
                    frontier_depth_mm: child.score.frontier_depth_mm,
                    fingerprint,
                });
            }
            Err(_) if mode == 3 && !score.feasible() => {
                infeasible_candidates.push(PrecompressionInfeasibleChild {
                    state,
                    fresh_raw_loss: score.common_loss(),
                    fresh_positive_pairs: score.collision_pairs.len(),
                    beam_ordinal,
                    fingerprint,
                });
            }
            Err(_) => {}
        }
    }
    if mode == 3 {
        diagnostics.rebuilt_child_record_hash = Some(precompression_child_record_hash(
            &diagnostics.rebuilt_children,
        ));
        if diagnostics.validation_counts.is_some_and(|counts| {
            counts.rebuilt_children > CONFLICT_RUIN_BEAM_WIDTH || counts.total > 7
        }) {
            diagnostics.skipped_reason =
                Some("cap: pre-compression validation budget exhausted".to_owned());
            return diagnostics;
        }
    }
    exact_parent_candidates.sort_by(|first, second| {
        first
            .compressed_raw_loss
            .total_cmp(&second.compressed_raw_loss)
            .then_with(|| {
                first
                    .metrics
                    .used_long_axis_depth_mm
                    .total_cmp(&second.metrics.used_long_axis_depth_mm)
            })
            .then_with(|| first.frontier_depth_mm.total_cmp(&second.frontier_depth_mm))
            .then_with(|| first.fingerprint.cmp(&second.fingerprint))
    });
    diagnostics.eligible_parent_fingerprints = exact_parent_candidates
        .iter()
        .map(|candidate| candidate.fingerprint.clone())
        .collect();
    if mode == 3 {
        infeasible_candidates.sort_by(|first, second| {
            first
                .fresh_raw_loss
                .total_cmp(&second.fresh_raw_loss)
                .then_with(|| first.fresh_positive_pairs.cmp(&second.fresh_positive_pairs))
                .then_with(|| first.beam_ordinal.cmp(&second.beam_ordinal))
                .then_with(|| first.fingerprint.cmp(&second.fingerprint))
        });
        let Some(selected) = infeasible_candidates.into_iter().next() else {
            diagnostics.skipped_reason = Some(
                "rebuild produced no distinct fresh-infeasible publication-invalid child"
                    .to_owned(),
            );
            return diagnostics;
        };
        diagnostics.selected_parent_fingerprint = Some(selected.fingerprint.clone());
        diagnostics.rebuild.selected_state_fingerprint = Some(selected.fingerprint.clone());
        run_precompression_infeasible_handoff(
            pieces,
            fast_settings,
            relaxed_settings,
            checkpoint,
            selected.state,
            &mut diagnostics,
        );
        return diagnostics;
    }
    let Some(selected) = exact_parent_candidates.into_iter().next() else {
        diagnostics.skipped_reason =
            Some("rebuild produced no distinct authoritative exact-valid parent".to_owned());
        return diagnostics;
    };
    diagnostics.selected_parent_fingerprint = Some(selected.fingerprint.clone());
    diagnostics.selected_parent_depth_mm = Some(selected.metrics.used_long_axis_depth_mm);
    diagnostics.selected_compressed_raw_loss = Some(selected.compressed_raw_loss);
    diagnostics.rebuild.selected_state_fingerprint = Some(selected.fingerprint.clone());
    if mode == 1 {
        diagnostics.skipped_reason =
            Some("mode one froze the direct exact-valid-parent candidate".to_owned());
        return diagnostics;
    }
    if mode != 2 {
        diagnostics.skipped_reason = Some("unsupported pre-compression mode".to_owned());
        return diagnostics;
    }
    run_precompression_stage_b(
        pieces,
        fast_settings,
        relaxed_settings,
        checkpoint,
        selected.compressed,
        &mut diagnostics,
    );
    diagnostics
}

#[cfg(feature = "jagua-experimental")]
fn precompression_child_record_hash(
    children: &[GeneralPrecompressionFrontierChildDiagnostics],
) -> String {
    let mut digest = Sha256::new();
    for child in children {
        digest.update(child.beam_ordinal.to_le_bytes());
        digest.update((child.fingerprint.len() as u64).to_le_bytes());
        digest.update(child.fingerprint.as_bytes());
        digest.update(child.exact_overlap_area_mm2.to_bits().to_le_bytes());
        digest.update(child.exact_positive_overlap_pairs.to_le_bytes());
        digest.update(child.frontier_depth_mm.to_bits().to_le_bytes());
        digest.update(child.fresh_raw_loss.to_bits().to_le_bytes());
        digest.update(child.fresh_positive_pairs.to_le_bytes());
        digest.update([u8::from(child.fresh_feasible)]);
        digest.update([u8::from(child.publication_valid)]);
        if let Some(reason) = &child.publication_rejection_reason {
            digest.update([1]);
            digest.update((reason.len() as u64).to_le_bytes());
            digest.update(reason.as_bytes());
        } else {
            digest.update([0]);
        }
    }
    format!("{:x}", digest.finalize())
}

#[cfg(feature = "jagua-experimental")]
fn run_precompression_infeasible_handoff<'a>(
    pieces: &'a [GeneralFastPiece<'a>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    checkpoint: &CoupledFailedCheckpoint,
    selected_state: RelaxedState,
    diagnostics: &mut GeneralPrecompressionFrontierVacancyDiagnostics,
) {
    let stage_a_target_seed = derive_seed(
        relaxed_settings.seed ^ PRECOMPRESSION_HANDOFF_SEED_DOMAIN,
        checkpoint.target_ordinal,
        usize::MAX - 72,
    );
    let stage_a_compression_seed = derive_seed(stage_a_target_seed, 0, usize::MAX - 71);
    let stage_a_worker_seeds = (0..COUPLED_SEPARATOR_WORKERS)
        .map(|worker| derive_seed(stage_a_target_seed, 0, worker))
        .collect::<Vec<_>>();
    diagnostics.stage_a_seed_domain = Some(PRECOMPRESSION_HANDOFF_SEED_DOMAIN);
    diagnostics.stage_a_target_seed = Some(stage_a_target_seed);
    diagnostics.stage_a_compression_seed = Some(stage_a_compression_seed);
    diagnostics.stage_a_worker_seeds = stage_a_worker_seeds.clone();

    let stage_a_initial_fingerprint = coupled_state_fingerprint(&selected_state);
    let stage_a_started = Instant::now();
    let stage_a = run_coupled_separator_target(
        pieces,
        fast_settings,
        precompression_arm_settings(relaxed_settings),
        &checkpoint.incumbent,
        selected_state,
        checkpoint
            .target_ordinal
            .saturating_add(COUPLED_SEPARATOR_TARGETS),
        checkpoint.incumbent.used_long_axis_depth_mm,
        checkpoint.compression_split_mm,
        stage_a_target_seed,
        stage_a_compression_seed,
        stage_a_worker_seeds,
        CoupledSeparatorArm::Treatment,
        CoupledRollbackRescorePolicy::CanonicalAuthoritativeRows,
        CoupledRollbackComparison::Exact,
        true,
        checkpoint.catalog.clone(),
        checkpoint.hazard_catalog.clone(),
    );
    let stage_a_elapsed_ms = stage_a_started.elapsed().as_secs_f64() * 1_000.0;
    let stage_a = match stage_a {
        Ok(outcome) => outcome,
        Err(error) => {
            diagnostics.stage_a = Some(GeneralConflictRuinArmDiagnostics {
                attempted: true,
                applied_rebuild: true,
                elapsed_ms: stage_a_elapsed_ms,
                initial_state_fingerprint: Some(stage_a_initial_fingerprint),
                failure_reason: Some(error.to_string()),
                ..GeneralConflictRuinArmDiagnostics::default()
            });
            return;
        }
    };
    diagnostics.stage_a = Some(precompression_target_arm_diagnostics(
        &stage_a,
        stage_a_initial_fingerprint,
        stage_a_elapsed_ms,
        true,
    ));
    if let Some(audit) = &stage_a.independent_audit {
        diagnostics.stage_a_independent_audit = Some(audit.diagnostics.clone());
        if let Some(counts) = diagnostics.validation_counts.as_mut() {
            counts.stage_a = audit.diagnostics.independent_audit_count;
        }
    } else if stage_a.exact_metrics.is_some() {
        if let Some(counts) = diagnostics.validation_counts.as_mut() {
            counts.stage_a = 1;
        }
    }
    if let Some(counts) = diagnostics.validation_counts.as_mut() {
        counts.total = counts
            .incumbent
            .saturating_add(counts.rebuilt_children)
            .saturating_add(counts.stage_a);
    }
    if stage_a.diagnostics.failure_reason.is_some() || stage_a.diagnostics.cap_exhausted.is_some() {
        diagnostics.skipped_reason = Some("Stage A ended with a failure or cap".to_owned());
        return;
    }
    if let Some(reason) = precompression_handoff_work_cap_reason(stage_a.work) {
        diagnostics.skipped_reason = Some(reason);
        return;
    }
    if diagnostics
        .validation_counts
        .is_some_and(|counts| counts.stage_a > 1 || counts.total > 7)
    {
        diagnostics.skipped_reason =
            Some("cap: pre-compression validation budget exhausted".to_owned());
        return;
    }
    let stage_a_metrics = stage_a.exact_metrics.or_else(|| {
        stage_a
            .independent_audit
            .as_ref()
            .and_then(|audit| audit.metrics)
    });
    let Some(stage_a_metrics) = stage_a_metrics else {
        diagnostics.skipped_reason = Some("Stage A did not produce exact-valid metrics".to_owned());
        return;
    };
    let stage_a_placements = to_fast_placements(&stage_a.final_state, pieces);
    let stage_a_placement_fingerprint = coupled_fast_placement_fingerprint(&stage_a_placements);
    if stage_a_placement_fingerprint
        == coupled_fast_placement_fingerprint(&checkpoint.incumbent.placements)
    {
        diagnostics.skipped_reason =
            Some("Stage A restored the original incumbent placement".to_owned());
        return;
    }
    diagnostics.selected_parent_depth_mm = Some(stage_a_metrics.used_long_axis_depth_mm);
    let stage_a_parent =
        fast_result_from_exact_state(&checkpoint.incumbent, stage_a_placements, stage_a_metrics);
    let stage_b_initial = match compress_state_at_split(
        &stage_a.final_state,
        checkpoint.target_depth_mm,
        checkpoint.compression_split_mm,
        pieces,
    ) {
        Ok(state) => state,
        Err(error) => {
            diagnostics.skipped_reason = Some(format!("Stage B compression: {error}"));
            return;
        }
    };
    let stage_b_initial_fingerprint = coupled_state_fingerprint(&stage_b_initial);
    let stage_b_started = Instant::now();
    let stage_b = run_coupled_separator_target(
        pieces,
        fast_settings,
        precompression_arm_settings(relaxed_settings),
        &stage_a_parent,
        stage_b_initial,
        checkpoint.target_ordinal,
        checkpoint.target_depth_mm,
        checkpoint.compression_split_mm,
        checkpoint.target_seed,
        checkpoint.compression_seed,
        checkpoint.attempt_diagnostics.worker_seeds.clone(),
        CoupledSeparatorArm::Treatment,
        CoupledRollbackRescorePolicy::CanonicalAuthoritativeRows,
        CoupledRollbackComparison::Exact,
        false,
        checkpoint.catalog.clone(),
        checkpoint.hazard_catalog.clone(),
    );
    let stage_b_elapsed_ms = stage_b_started.elapsed().as_secs_f64() * 1_000.0;
    let stage_b = match stage_b {
        Ok(outcome) => outcome,
        Err(error) => {
            diagnostics.treatment = GeneralConflictRuinArmDiagnostics {
                attempted: true,
                applied_rebuild: true,
                elapsed_ms: stage_b_elapsed_ms,
                initial_state_fingerprint: Some(stage_b_initial_fingerprint),
                failure_reason: Some(error.to_string()),
                ..GeneralConflictRuinArmDiagnostics::default()
            };
            return;
        }
    };
    diagnostics.treatment = precompression_target_arm_diagnostics(
        &stage_b,
        stage_b_initial_fingerprint,
        stage_b_elapsed_ms,
        true,
    );
    if stage_b.diagnostics.failure_reason.is_none()
        && stage_b.diagnostics.cap_exhausted.is_none()
        && stage_b.diagnostics.feasible
    {
        if let Some(counts) = diagnostics.validation_counts.as_mut() {
            counts.stage_b = 1;
        }
    }
    if let Some(counts) = diagnostics.validation_counts.as_mut() {
        counts.total = counts
            .incumbent
            .saturating_add(counts.rebuilt_children)
            .saturating_add(counts.stage_a)
            .saturating_add(counts.stage_b);
    }
    let mut aggregate_work = stage_a.work;
    aggregate_work.accumulate(stage_b.work);
    if let Some(reason) = precompression_handoff_work_cap_reason(aggregate_work) {
        diagnostics.skipped_reason = Some(reason);
        return;
    }
    if diagnostics
        .validation_counts
        .is_some_and(|counts| counts.stage_b > 1 || counts.total > 7)
    {
        diagnostics.skipped_reason =
            Some("cap: pre-compression validation budget exhausted".to_owned());
        return;
    }
    diagnostics.mechanism_passed = !checkpoint.attempt_diagnostics.feasible
        && !checkpoint.attempt_diagnostics.exact_valid
        && stage_b.diagnostics.exact_valid
        && stage_b.accepted.is_some();
    if !diagnostics.mechanism_passed {
        diagnostics.skipped_reason =
            Some("Stage B did not accept the failed contraction".to_owned());
    }
}

#[cfg(feature = "jagua-experimental")]
fn precompression_arm_settings(mut settings: GeneralRelaxedSettings) -> GeneralRelaxedSettings {
    settings.collision_backend = GeneralRelaxedCollisionBackend::DynamicHazard;
    settings.angle_seed_policy = GeneralRelaxedAngleSeedPolicy::ContinuousUniform;
    settings.pressure_model = GeneralRelaxedPressureModel::DynamicPoles;
    settings.angular_repair = GeneralAngularRepairSettings::disabled();
    settings.synchronize_lanes = true;
    settings.sweeps_per_epoch = COUPLED_SEPARATOR_ROUNDS;
    settings
}

#[cfg(feature = "jagua-experimental")]
fn fast_result_from_exact_state(
    base: &GeneralFastResult,
    placements: Vec<GeneralFastPlacement>,
    metrics: GeneralPlacementMetrics,
) -> GeneralFastResult {
    let mut result = base.clone();
    result.placements = placements;
    result.unplaced_piece_ids.clear();
    result.used_short_axis_span_mm = metrics.used_short_axis_span_mm;
    result.used_long_axis_depth_mm = metrics.used_long_axis_depth_mm;
    result.unused_short_axis_projection_mm = metrics.unused_short_axis_projection_mm;
    result.occupied_envelope_area_mm2 = metrics.occupied_envelope_area_mm2;
    result
}

#[cfg(feature = "jagua-experimental")]
fn precompression_handoff_work_cap_reason(work: CoupledSeparatorWork) -> Option<String> {
    let checks = [
        (work.dynamic_queries, 6_720_000, "dynamic-query"),
        (work.pressure_evaluations, 64_000_000, "pressure-evaluation"),
        (work.retained_confirmations, 39_040, "retained-confirmation"),
        (work.hazard_updates, 39_040, "hazard-update"),
        (work.layout_loads, 640, "layout-load"),
        (
            work.worker_full_score_pair_visits,
            1_171_200,
            "worker full-score pair-visit",
        ),
        (
            work.auditor_full_score_pair_visits,
            18_300,
            "auditor full-score pair-visit",
        ),
    ];
    checks.into_iter().find_map(|(actual, cap, label)| {
        (actual > cap).then(|| format!("cap: pre-compression handoff {label} budget exhausted"))
    })
}

#[cfg(feature = "jagua-experimental")]
fn precompression_target_arm_diagnostics(
    outcome: &CoupledTargetOutcome,
    initial_state_fingerprint: String,
    elapsed_ms: f64,
    applied_rebuild: bool,
) -> GeneralConflictRuinArmDiagnostics {
    let final_placements = outcome
        .accepted
        .as_ref()
        .map(|result| result.placements.clone())
        .unwrap_or_default();
    GeneralConflictRuinArmDiagnostics {
        attempted: true,
        applied_rebuild,
        elapsed_ms,
        initial_state_fingerprint: Some(initial_state_fingerprint),
        final_state_fingerprint: Some(outcome.diagnostics.final_state_fingerprint.clone()),
        exact_valid: outcome.diagnostics.exact_valid,
        accepted_depth_mm: outcome.diagnostics.accepted_depth_mm,
        final_placement_fingerprint: (!final_placements.is_empty())
            .then(|| coupled_fast_placement_fingerprint(&final_placements)),
        final_placements: coupled_placement_diagnostics(&final_placements),
        work: conflict_ruin_retry_work(outcome.work),
        target: Some(outcome.diagnostics.clone()),
        failure_reason: outcome
            .diagnostics
            .failure_reason
            .clone()
            .or_else(|| outcome.diagnostics.cap_exhausted.clone())
            .or_else(|| outcome.diagnostics.exact_rejection_reason.clone()),
    }
}

#[cfg(feature = "jagua-experimental")]
fn run_precompression_stage_b<'a>(
    pieces: &'a [GeneralFastPiece<'a>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    checkpoint: &CoupledFailedCheckpoint,
    initial_state: RelaxedState,
    diagnostics: &mut GeneralPrecompressionFrontierVacancyDiagnostics,
) {
    let initial_state_fingerprint = coupled_state_fingerprint(&initial_state);
    let started = Instant::now();
    let outcome = run_coupled_separator_target(
        pieces,
        fast_settings,
        precompression_arm_settings(relaxed_settings),
        &checkpoint.incumbent,
        initial_state,
        checkpoint.target_ordinal,
        checkpoint.target_depth_mm,
        checkpoint.compression_split_mm,
        checkpoint.target_seed,
        checkpoint.compression_seed,
        checkpoint.attempt_diagnostics.worker_seeds.clone(),
        CoupledSeparatorArm::Treatment,
        CoupledRollbackRescorePolicy::StrictDerivedAgreement,
        CoupledRollbackComparison::Exact,
        false,
        checkpoint.catalog.clone(),
        checkpoint.hazard_catalog.clone(),
    );
    let elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0;
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(error) => {
            diagnostics.treatment = GeneralConflictRuinArmDiagnostics {
                attempted: true,
                applied_rebuild: true,
                elapsed_ms,
                initial_state_fingerprint: Some(initial_state_fingerprint),
                failure_reason: Some(error.to_string()),
                ..GeneralConflictRuinArmDiagnostics::default()
            };
            return;
        }
    };
    diagnostics.treatment = precompression_target_arm_diagnostics(
        &outcome,
        initial_state_fingerprint,
        elapsed_ms,
        true,
    );
    diagnostics.mechanism_passed = outcome.accepted.is_some();
}

#[cfg(feature = "jagua-experimental")]
fn precompression_full_score<'a>(
    pieces: &'a [GeneralFastPiece<'a>],
    fast_settings: GeneralFastSettings,
    mut relaxed_settings: GeneralRelaxedSettings,
    checkpoint: &CoupledFailedCheckpoint,
    state: &RelaxedState,
    diagnostics: &mut GeneralPrecompressionFrontierVacancyDiagnostics,
) -> Result<PairTracker, String> {
    let pair_visits = pieces.len().saturating_mul(pieces.len().saturating_sub(1)) / 2;
    let next_scores = diagnostics.full_scores.saturating_add(1);
    let next_pair_visits = diagnostics
        .full_score_pair_visits
        .saturating_add(pair_visits);
    if next_scores > PRECOMPRESSION_FRONTIER_FULL_SCORE_CAP
        || next_pair_visits > PRECOMPRESSION_FRONTIER_PAIR_VISIT_CAP
    {
        return Err("cap: pre-compression full-score budget exhausted".to_owned());
    }
    diagnostics.full_scores = next_scores;
    diagnostics.full_score_pair_visits = next_pair_visits;
    relaxed_settings.collision_backend = GeneralRelaxedCollisionBackend::DynamicHazard;
    relaxed_settings.angle_seed_policy = GeneralRelaxedAngleSeedPolicy::ContinuousUniform;
    relaxed_settings.pressure_model = GeneralRelaxedPressureModel::DynamicPoles;
    relaxed_settings.angular_repair = GeneralAngularRepairSettings::disabled();
    relaxed_settings.synchronize_lanes = true;
    let mut search = LegacyLaneSearch::new(
        pieces,
        fast_settings,
        relaxed_settings,
        checkpoint.target_seed ^ PRECOMPRESSION_FRONTIER_SEED_DOMAIN,
        checkpoint.catalog.clone(),
    );
    search.hazard_catalog = Some(checkpoint.hazard_catalog.clone());
    search
        .prepare_dynamic_hazard(state)
        .map_err(|error| format!("pre-compression full-index rebuild: {error}"))?;
    search
        .score_state(state)
        .map_err(|error| format!("pre-compression full score: {error}"))
}

#[cfg(feature = "jagua-experimental")]
fn run_conflict_ruin_recreate_experiment<'a>(
    pieces: &'a [GeneralFastPiece<'a>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    control: &CoupledArmOutcome,
    treatment: &CoupledArmOutcome,
) -> GeneralConflictRuinDiagnostics {
    let mut diagnostics = GeneralConflictRuinDiagnostics {
        seed_domain: CONFLICT_RUIN_SEED_DOMAIN,
        ..GeneralConflictRuinDiagnostics::default()
    };
    let checkpoint =
        match select_conflict_ruin_checkpoint(control, treatment, pieces, fast_settings) {
            Ok(checkpoint) => checkpoint,
            Err(error) => {
                diagnostics.skipped_reason = Some(error.to_string());
                return diagnostics;
            }
        };
    let Some(checkpoint) = checkpoint else {
        diagnostics.skipped_reason =
            Some("no uncapped failed rigid-separator checkpoint was retained".to_owned());
        return diagnostics;
    };
    diagnostics.target_depth_mm = Some(checkpoint.target_depth_mm);
    diagnostics.checkpoint_fingerprint = Some(conflict_ruin_checkpoint_fingerprint(checkpoint));
    if checkpoint.attempt_diagnostics.failure_reason.is_some()
        || checkpoint.attempt_diagnostics.cap_exhausted.is_some()
    {
        diagnostics.skipped_reason =
            Some("attempt-zero checkpoint was failed or cap-exhausted".to_owned());
        return diagnostics;
    }
    if checkpoint.minimum.score.feasible() || checkpoint.minimum.score.common_loss() <= 0.0 {
        diagnostics.skipped_reason = Some(
            "attempt-zero checkpoint did not retain a strictly positive-loss state".to_owned(),
        );
        return diagnostics;
    }
    if let Err(reason) = validate_conflict_ruin_state(&checkpoint.minimum.state, pieces.len()) {
        diagnostics.skipped_reason = Some(reason);
        return diagnostics;
    }
    if checkpoint.minimum.score.boundary_loss > 0.0 {
        let probe = match probe_conflict_ruin_boundary_blockers(checkpoint, pieces, fast_settings) {
            Ok(probe) => probe,
            Err(reason) => {
                diagnostics.skipped_reason = Some(reason);
                return diagnostics;
            }
        };
        diagnostics.selector_mode = Some("boundaryBlockerProbe".to_owned());
        diagnostics.root_piece_id = Some(pieces[probe.root].id.to_owned());
        diagnostics.root_boundary_loss =
            Some(checkpoint.minimum.score.boundaries[probe.root].raw_loss);
        diagnostics.root_probe_pose = Some(GeneralCoupledSeparatorPlacementDiagnostics {
            piece_id: pieces[probe.root].id.to_owned(),
            rotation_deg: probe.placement.rotation_deg,
            mirrored: probe.placement.mirrored,
            translate_short_axis: probe.placement.translate_x,
            translate_long_axis: probe.placement.translate_y,
        });
        diagnostics.root_probe_blockers = probe
            .blockers
            .iter()
            .map(
                |(piece_index, pressure)| GeneralConflictRuinBlockerDiagnostics {
                    piece_id: pieces[*piece_index].id.to_owned(),
                    proxy_pressure: *pressure,
                },
            )
            .collect();
        let mut projected_state = checkpoint.minimum.state.clone();
        projected_state.placements[probe.root] = probe.placement.clone();
        diagnostics.root_probe_state_fingerprint =
            Some(coupled_state_fingerprint(&projected_state));

        let mut probe_settings = relaxed_settings;
        probe_settings.collision_backend = GeneralRelaxedCollisionBackend::DynamicHazard;
        probe_settings.angle_seed_policy = GeneralRelaxedAngleSeedPolicy::ContinuousUniform;
        probe_settings.pressure_model = GeneralRelaxedPressureModel::DynamicPoles;
        probe_settings.angular_repair = GeneralAngularRepairSettings::disabled();
        probe_settings.synchronize_lanes = true;
        let mut probe_search = LegacyLaneSearch::new(
            pieces,
            fast_settings,
            probe_settings,
            checkpoint.target_seed ^ CONFLICT_RUIN_SEED_DOMAIN,
            checkpoint.catalog.clone(),
        );
        probe_search.hazard_catalog = Some(checkpoint.hazard_catalog.clone());
        if let Err(error) = probe_search.prepare_dynamic_hazard(&projected_state) {
            diagnostics.skipped_reason =
                Some(format!("boundary projection full-index rebuild: {error}"));
            return diagnostics;
        }
        let tracker = match probe_search.score_state(&projected_state) {
            Ok(tracker) => tracker,
            Err(error) => {
                diagnostics.skipped_reason =
                    Some(format!("boundary projection full score: {error}"));
                return diagnostics;
            }
        };
        diagnostics.root_probe_tracker_loss = Some(tracker.common_loss());
        diagnostics.root_probe_tracker_boundary_loss = Some(tracker.boundary_loss);
        diagnostics.root_probe_tracker_positive_pairs = Some(tracker.collision_pairs.len());
        diagnostics.root_probe_tracker_feasible = Some(tracker.feasible());

        let placements = to_fast_placements(&projected_state, pieces);
        match validate_and_measure_placements(pieces, &placements, fast_settings) {
            Ok(metrics) => {
                diagnostics.root_probe_exact_valid = Some(true);
                diagnostics.root_probe_exact_depth_mm = Some(metrics.used_long_axis_depth_mm);
                diagnostics.root_probe_improves_incumbent = Some(
                    metrics.used_long_axis_depth_mm < checkpoint.incumbent.used_long_axis_depth_mm,
                );
            }
            Err(error) => {
                diagnostics.root_probe_exact_valid = Some(false);
                diagnostics.root_probe_improves_incumbent = Some(false);
                diagnostics.root_probe_exact_rejection_reason = Some(error.to_string());
            }
        }
        diagnostics.skipped_reason = Some(
            "boundary projection audited; publication remains disabled pending causal review"
                .to_owned(),
        );
        return diagnostics;
    }
    diagnostics.selector_mode = Some("positivePairConflict".to_owned());
    let removal_order = match select_conflict_ruin_neighborhood(checkpoint, pieces) {
        Ok(order) => order,
        Err(reason) => {
            diagnostics.skipped_reason = Some(reason);
            return diagnostics;
        }
    };
    let mut removed = removal_order.clone();
    removed.sort_unstable();
    diagnostics.removed_piece_ids = removed
        .iter()
        .map(|index| pieces[*index].id.to_owned())
        .collect();
    diagnostics.removal_order_piece_ids = removal_order
        .iter()
        .map(|index| pieces[*index].id.to_owned())
        .collect();
    diagnostics.attempted = true;

    let rebuild_started = Instant::now();
    let rebuilt = build_conflict_ruin_state(
        checkpoint,
        pieces,
        fast_settings,
        relaxed_settings.seed,
        &removal_order,
        &mut diagnostics.rebuild,
    );
    diagnostics.rebuild.elapsed_ms = rebuild_started.elapsed().as_secs_f64() * 1_000.0;
    let rebuilt = match rebuilt {
        Ok(rebuilt) => rebuilt,
        Err(reason) => {
            diagnostics.rebuild.cap_exhausted = reason
                .strip_prefix("cap: ")
                .map(str::to_owned)
                .or_else(|| diagnostics.rebuild.cap_exhausted.clone());
            diagnostics.skipped_reason = Some(reason);
            return diagnostics;
        }
    };
    diagnostics.rebuild.selected_state_fingerprint = Some(coupled_state_fingerprint(&rebuilt));

    let retry_seed = relaxed_settings.seed ^ CONFLICT_RUIN_RETRY_SEED_DOMAIN;
    let worker_seeds = (0..COUPLED_SEPARATOR_WORKERS)
        .map(|worker| derive_seed(retry_seed, checkpoint.target_ordinal, worker))
        .collect::<Vec<_>>();
    diagnostics.retry_control = run_conflict_ruin_retry(
        checkpoint,
        pieces,
        fast_settings,
        relaxed_settings,
        checkpoint.minimum.state.clone(),
        false,
        worker_seeds.clone(),
    );
    diagnostics.treatment = run_conflict_ruin_retry(
        checkpoint,
        pieces,
        fast_settings,
        relaxed_settings,
        rebuilt,
        true,
        worker_seeds,
    );
    diagnostics
}

#[cfg(feature = "jagua-experimental")]
fn select_conflict_ruin_checkpoint<'a>(
    control: &'a CoupledArmOutcome,
    treatment: &'a CoupledArmOutcome,
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
) -> Result<Option<&'a CoupledFailedCheckpoint>, GeneralFastError> {
    let mut checkpoints = [control, treatment]
        .into_iter()
        .filter_map(|outcome| outcome.checkpoint.as_ref())
        .map(|checkpoint| {
            Ok::<_, GeneralFastError>((
                checkpoint,
                coupled_independent_source_depth(
                    pieces,
                    &checkpoint.incumbent.placements,
                    fast_settings,
                )?,
                coupled_fast_placement_fingerprint(&checkpoint.incumbent.placements),
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    checkpoints.sort_by(|first, second| {
        first
            .1
            .total_cmp(&second.1)
            .then_with(|| first.2.cmp(&second.2))
    });
    Ok(checkpoints.first().map(|(checkpoint, _, _)| *checkpoint))
}

#[cfg(feature = "jagua-experimental")]
fn select_exact_boundary_projection(
    state: &RelaxedState,
    tracker: &PairTracker,
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
) -> Result<(usize, RelaxedPlacement), String> {
    let root = ordered_boundary_piece_indices(state, tracker, pieces)?
        .first()
        .copied()
        .ok_or_else(|| "boundary projection found no positive boundary row".to_owned())?;
    Ok((
        root,
        project_piece_into_exact_boundary(state, pieces, fast_settings, root)?,
    ))
}

#[cfg(feature = "jagua-experimental")]
fn select_all_exact_boundary_projections(
    state: &RelaxedState,
    tracker: &PairTracker,
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
) -> Result<Vec<(usize, RelaxedPlacement)>, String> {
    ordered_boundary_piece_indices(state, tracker, pieces)?
        .into_iter()
        .map(|root| {
            Ok((
                root,
                project_piece_into_exact_boundary(state, pieces, fast_settings, root)?,
            ))
        })
        .collect()
}

#[cfg(feature = "jagua-experimental")]
fn ordered_boundary_piece_indices(
    state: &RelaxedState,
    tracker: &PairTracker,
    pieces: &[GeneralFastPiece<'_>],
) -> Result<Vec<usize>, String> {
    let frontiers = state
        .placements
        .iter()
        .map(|placement| {
            conflict_ruin_material_frontier(pieces[placement.input_index], placement)
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut boundary_pieces = (0..pieces.len())
        .filter(|index| tracker.boundaries[*index].raw_loss > 0.0)
        .collect::<Vec<_>>();
    boundary_pieces.sort_by(|first, second| {
        tracker.boundaries[*second]
            .raw_loss
            .total_cmp(&tracker.boundaries[*first].raw_loss)
            .then_with(|| frontiers[*second].total_cmp(&frontiers[*first]))
            .then_with(|| {
                tracker.incident_raw_loss[*second].total_cmp(&tracker.incident_raw_loss[*first])
            })
            .then_with(|| pieces[*first].id.cmp(pieces[*second].id))
    });
    Ok(boundary_pieces)
}

#[cfg(feature = "jagua-experimental")]
fn project_piece_into_exact_boundary(
    state: &RelaxedState,
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    root: usize,
) -> Result<RelaxedPlacement, String> {
    let current = &state.placements[root];
    let local_collision = pieces[root]
        .polygon
        .transformed(current.rotation_deg, current.mirrored, 0.0, 0.0)
        .and_then(|polygon| polygon.offset(collision_expansion_mm(fast_settings)))
        .map_err(|error| format!("boundary projection geometry: {error}"))?;
    let bounds = local_collision
        .bounds()
        .ok_or_else(|| "boundary projection collision is empty".to_owned())?;
    let inset = collision_sheet_inset_mm(fast_settings);
    let minimum_x = grid_upper_bound_key(inset - bounds.min_x);
    let maximum_x = grid_lower_bound_key(fast_settings.sheet_short_axis_mm - inset - bounds.max_x);
    let minimum_y = grid_upper_bound_key(inset - bounds.min_y);
    let maximum_y = grid_lower_bound_key(state.strip_depth_mm - inset - bounds.max_y);
    if minimum_x > maximum_x || minimum_y > maximum_y {
        return Err("boundary projection has an empty canonical inner-fit rectangle".to_owned());
    }
    let current_x = grid_key(current.translate_x);
    let current_y = grid_key(current.translate_y);
    let placement = RelaxedPlacement {
        input_index: root,
        rotation_deg: current.rotation_deg,
        mirrored: current.mirrored,
        translate_x: from_grid(current_x.clamp(minimum_x, maximum_x) as f64),
        translate_y: from_grid(current_y.clamp(minimum_y, maximum_y) as f64),
    };
    let collision = pieces[root]
        .polygon
        .transformed(
            placement.rotation_deg,
            placement.mirrored,
            placement.translate_x,
            placement.translate_y,
        )
        .and_then(|polygon| polygon.offset(collision_expansion_mm(fast_settings)))
        .map_err(|error| format!("boundary projection placement: {error}"))?;
    if !collision.fits_rect(
        inset,
        inset,
        fast_settings.sheet_short_axis_mm - inset,
        state.strip_depth_mm - inset,
    ) {
        return Err("boundary projection is not exact-fit after canonical clamping".to_owned());
    }
    Ok(placement)
}

#[cfg(feature = "jagua-experimental")]
fn probe_conflict_ruin_boundary_blockers(
    checkpoint: &CoupledFailedCheckpoint,
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
) -> Result<ConflictRuinBoundaryProbe, String> {
    let state = &checkpoint.minimum.state;
    let (root, placement) =
        select_exact_boundary_projection(state, &checkpoint.minimum.score, pieces, fast_settings)?;
    let frontiers = state
        .placements
        .iter()
        .map(|placement| {
            conflict_ruin_material_frontier(pieces[placement.input_index], placement)
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let poses = state.placements.iter().map(hazard_pose).collect::<Vec<_>>();
    let mut active = vec![true; pieces.len()];
    active[root] = false;
    let mut index = JaguaHazardIndex::from_catalog_active(
        pieces,
        fast_settings,
        checkpoint.target_depth_mm,
        &poses,
        &active,
        &checkpoint.hazard_catalog,
    )
    .map_err(|error| format!("boundary-blocker probe index: {error}"))?;
    let pose = hazard_pose(&placement);
    let query = index
        .query_unplaced(root, pose)
        .map_err(|error| format!("boundary-blocker probe query: {error}"))?;
    let GeneralHazardQuery::Complete {
        boundary,
        colliding_piece_ids,
    } = query
    else {
        return Err("boundary-blocker probe unexpectedly pruned".to_owned());
    };
    if boundary {
        return Err("boundary-blocker probe remained outside the hazard envelope".to_owned());
    }
    let mut blockers = colliding_piece_ids
        .into_iter()
        .map(|fixed_piece_id| {
            Ok::<_, String>((
                fixed_piece_id,
                index
                    .collision_pressure(root, pose, fixed_piece_id)
                    .map_err(|error| format!("boundary-blocker probe pressure: {error}"))?,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    blockers.sort_by(|first, second| {
        second
            .1
            .total_cmp(&first.1)
            .then_with(|| frontiers[second.0].total_cmp(&frontiers[first.0]))
            .then_with(|| pieces[first.0].id.cmp(pieces[second.0].id))
    });
    Ok(ConflictRuinBoundaryProbe {
        root,
        placement,
        blockers,
    })
}

#[cfg(feature = "jagua-experimental")]
fn validate_conflict_ruin_state(state: &RelaxedState, piece_count: usize) -> Result<(), String> {
    if state.placements.len() != piece_count {
        return Err(format!(
            "checkpoint contains {} placements for {piece_count} pieces",
            state.placements.len()
        ));
    }
    let mut seen = vec![false; piece_count];
    for (state_index, placement) in state.placements.iter().enumerate() {
        if placement.input_index >= piece_count || seen[placement.input_index] {
            return Err("checkpoint contains an unknown or duplicate stable input ID".to_owned());
        }
        if placement.input_index != state_index {
            return Err("checkpoint stable input IDs are not stored at stable indices".to_owned());
        }
        seen[placement.input_index] = true;
    }
    if seen.iter().any(|present| !present) {
        return Err("checkpoint is missing a stable input ID".to_owned());
    }
    Ok(())
}

#[cfg(feature = "jagua-experimental")]
fn conflict_ruin_checkpoint_fingerprint(checkpoint: &CoupledFailedCheckpoint) -> String {
    let mut digest = Sha256::new();
    digest.update(b"conflict-ruin-reset-empty-weights-v1");
    digest.update(grid_key(checkpoint.target_depth_mm).to_le_bytes());
    digest.update(coupled_state_fingerprint(&checkpoint.minimum.state));
    digest.update(pair_tracker_fingerprint(&checkpoint.minimum.score));
    digest.update(coupled_fast_placement_fingerprint(
        &checkpoint.incumbent.placements,
    ));
    digest.update(checkpoint.target_seed.to_le_bytes());
    digest.update(checkpoint.compression_seed.to_le_bytes());
    format!("{:x}", digest.finalize())
}

#[cfg(feature = "jagua-experimental")]
fn pair_tracker_fingerprint(tracker: &PairTracker) -> String {
    let mut digest = Sha256::new();
    digest.update(tracker.piece_count.to_le_bytes());
    for boundary in &tracker.boundaries {
        digest.update(boundary.violations.to_le_bytes());
        digest.update(boundary.raw_loss.to_bits().to_le_bytes());
    }
    for pair in &tracker.pairs {
        digest.update(pair.raw_loss.to_bits().to_le_bytes());
        digest.update(pair.guided_weight.to_bits().to_le_bytes());
        digest.update(pair.normalization_scale.to_bits().to_le_bytes());
    }
    format!("{:x}", digest.finalize())
}

#[cfg(feature = "jagua-experimental")]
fn select_conflict_ruin_neighborhood(
    checkpoint: &CoupledFailedCheckpoint,
    pieces: &[GeneralFastPiece<'_>],
) -> Result<Vec<usize>, String> {
    let state = &checkpoint.minimum.state;
    let tracker = &checkpoint.minimum.score;
    let mut indices = (0..pieces.len()).collect::<Vec<_>>();
    indices.sort_by(|first, second| {
        tracker.incident_raw_loss[*second]
            .total_cmp(&tracker.incident_raw_loss[*first])
            .then_with(|| {
                tracker.boundaries[*second]
                    .raw_loss
                    .total_cmp(&tracker.boundaries[*first].raw_loss)
            })
            .then_with(|| {
                conflict_ruin_material_frontier(pieces[*second], &state.placements[*second])
                    .unwrap_or(f64::NEG_INFINITY)
                    .total_cmp(
                        &conflict_ruin_material_frontier(pieces[*first], &state.placements[*first])
                            .unwrap_or(f64::NEG_INFINITY),
                    )
            })
            .then_with(|| pieces[*first].id.cmp(pieces[*second].id))
    });
    let root = indices
        .into_iter()
        .find(|index| tracker.incident_raw_loss[*index] > 0.0)
        .ok_or_else(|| "checkpoint has no positive incident conflict loss".to_owned())?;
    let mut neighbors = tracker
        .collision_pairs
        .iter()
        .filter_map(|(first, second, loss)| {
            if *loss <= 0.0 {
                None
            } else if *first == root {
                Some((*second, *loss))
            } else if *second == root {
                Some((*first, *loss))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    neighbors.sort_by(|first, second| {
        second
            .1
            .total_cmp(&first.1)
            .then_with(|| {
                conflict_ruin_material_frontier(pieces[second.0], &state.placements[second.0])
                    .unwrap_or(f64::NEG_INFINITY)
                    .total_cmp(
                        &conflict_ruin_material_frontier(
                            pieces[first.0],
                            &state.placements[first.0],
                        )
                        .unwrap_or(f64::NEG_INFINITY),
                    )
            })
            .then_with(|| pieces[first.0].id.cmp(pieces[second.0].id))
    });
    neighbors.dedup_by_key(|(index, _)| *index);
    if neighbors.len() < CONFLICT_RUIN_REMOVED_PIECES - 1 {
        return Err("root has fewer than two positive conflict neighbors".to_owned());
    }
    let mut selected = vec![root, neighbors[0].0, neighbors[1].0];
    selected.sort_by(|first, second| {
        tracker.incident_raw_loss[*second]
            .total_cmp(&tracker.incident_raw_loss[*first])
            .then_with(|| pieces[*first].id.cmp(pieces[*second].id))
    });
    Ok(selected)
}

#[cfg(feature = "jagua-experimental")]
fn conflict_ruin_material_frontier(
    piece: GeneralFastPiece<'_>,
    placement: &RelaxedPlacement,
) -> Result<f64, GeneralFastError> {
    piece
        .polygon
        .transformed(
            placement.rotation_deg,
            placement.mirrored,
            placement.translate_x,
            placement.translate_y,
        )?
        .bounds()
        .map(|bounds| bounds.max_y)
        .ok_or_else(|| {
            GeneralPolygonError::from_message("conflict-ruin material contour is empty").into()
        })
}

#[cfg(feature = "jagua-experimental")]
fn build_conflict_ruin_state(
    checkpoint: &CoupledFailedCheckpoint,
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    seed: u64,
    removal_order: &[usize],
    diagnostics: &mut GeneralConflictRuinRebuildDiagnostics,
) -> Result<RelaxedState, String> {
    let mut work = ConflictRuinWork::for_piece_count(pieces.len());
    let outcome = build_conflict_ruin_state_inner(
        &checkpoint.minimum.state,
        checkpoint.target_depth_mm,
        &checkpoint.hazard_catalog,
        pieces,
        fast_settings,
        seed,
        removal_order,
        diagnostics,
        &mut work,
    );
    conflict_ruin_publish_work(diagnostics, work);
    let outcome = outcome?;
    let selected = outcome
        .beam
        .into_iter()
        .min_by(conflict_ruin_beam_order)
        .ok_or_else(|| "conflict rebuild retained no complete state".to_owned())?;
    let initial_fallback = ConflictRuinBeamState {
        state: checkpoint.minimum.state.clone(),
        active: vec![true; pieces.len()],
        collisions: vec![None; pieces.len()],
        score: outcome.initial_score,
    };
    let selected = if conflict_ruin_beam_order(&initial_fallback, &selected) == Ordering::Less {
        initial_fallback
    } else {
        selected
    };
    validate_complete_conflict_ruin_child(&selected, pieces.len())?;
    diagnostics.selected_exact_overlap_area_mm2 = Some(selected.score.total_overlap_area_mm2);
    diagnostics.selected_positive_overlap_pairs = Some(selected.score.positive_overlap_pairs);
    Ok(selected.state)
}

#[cfg(feature = "jagua-experimental")]
fn build_conflict_ruin_states(
    base_state: &RelaxedState,
    strip_depth_mm: f64,
    hazard_catalog: &Arc<JaguaHazardCatalog>,
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    seed: u64,
    removal_order: &[usize],
    diagnostics: &mut GeneralConflictRuinRebuildDiagnostics,
) -> Result<Vec<ConflictRuinBeamState>, String> {
    let mut work = ConflictRuinWork::for_piece_count(pieces.len());
    let outcome = build_conflict_ruin_state_inner(
        base_state,
        strip_depth_mm,
        hazard_catalog,
        pieces,
        fast_settings,
        seed,
        removal_order,
        diagnostics,
        &mut work,
    );
    conflict_ruin_publish_work(diagnostics, work);
    let outcome = outcome?;
    for child in &outcome.beam {
        validate_complete_conflict_ruin_child(child, pieces.len())?;
    }
    Ok(outcome.beam)
}

#[cfg(feature = "jagua-experimental")]
fn validate_complete_conflict_ruin_child(
    child: &ConflictRuinBeamState,
    piece_count: usize,
) -> Result<(), String> {
    if child.active.iter().any(|active| !active) {
        return Err("conflict rebuild did not restore every active bit".to_owned());
    }
    validate_conflict_ruin_state(&child.state, piece_count)
}

#[cfg(feature = "jagua-experimental")]
fn build_conflict_ruin_state_inner(
    base_state: &RelaxedState,
    strip_depth_mm: f64,
    hazard_catalog: &Arc<JaguaHazardCatalog>,
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    seed: u64,
    removal_order: &[usize],
    diagnostics: &mut GeneralConflictRuinRebuildDiagnostics,
    work: &mut ConflictRuinWork,
) -> Result<ConflictRuinBuildOutcome, String> {
    if removal_order.is_empty() || removal_order.len() > CONFLICT_RUIN_REMOVED_PIECES {
        return Err(format!(
            "conflict rebuild requires between one and {CONFLICT_RUIN_REMOVED_PIECES} removed pieces"
        ));
    }
    let mut collisions = vec![None; pieces.len()];
    for placement in &base_state.placements {
        collisions[placement.input_index] = Some(conflict_ruin_build_collision(
            pieces[placement.input_index],
            placement,
            fast_settings,
            work,
        )?);
    }
    let pair_count = pieces.len().saturating_mul(pieces.len().saturating_sub(1)) / 2;
    let mut initial_pair_areas = vec![0.0; pair_count];
    let mut initial_score = ConflictRuinExactScore {
        total_overlap_area_mm2: 0.0,
        positive_overlap_pairs: 0,
        maximum_pair_area_mm2: 0.0,
        frontier_depth_mm: conflict_ruin_state_frontier(
            pieces,
            base_state,
            &vec![true; pieces.len()],
        )?,
    };
    for first in 0..pieces.len() {
        for second in (first + 1)..pieces.len() {
            let area = conflict_ruin_intersection_area(
                collisions[first]
                    .as_ref()
                    .ok_or_else(|| format!("missing collision polygon for piece {first}"))?,
                collisions[second]
                    .as_ref()
                    .ok_or_else(|| format!("missing collision polygon for piece {second}"))?,
                work,
            )?;
            initial_pair_areas[pair_slot(pieces.len(), first, second)] = area;
            conflict_ruin_add_pair_area(&mut initial_score, area);
        }
    }
    diagnostics.initial_exact_overlap_area_mm2 = initial_score.total_overlap_area_mm2;
    diagnostics.initial_positive_overlap_pairs = initial_score.positive_overlap_pairs;

    let mut active = vec![true; pieces.len()];
    for piece_index in removal_order {
        if *piece_index >= pieces.len() || !active[*piece_index] {
            return Err(
                "conflict rebuild removal order contains an unknown or duplicate ID".to_owned(),
            );
        }
        active[*piece_index] = false;
        collisions[*piece_index] = None;
    }
    let survivor_score =
        conflict_ruin_score_active_pairs(pieces, base_state, &active, &initial_pair_areas)?;
    let mut beam = vec![ConflictRuinBeamState {
        state: base_state.clone(),
        active,
        collisions,
        score: survivor_score,
    }];

    for (layer, piece_index) in removal_order.iter().copied().enumerate() {
        let mut children = Vec::new();
        for (parent_ordinal, parent) in beam.iter().enumerate() {
            let poses = parent
                .state
                .placements
                .iter()
                .map(hazard_pose)
                .collect::<Vec<_>>();
            let mut index = JaguaHazardIndex::from_catalog_active(
                pieces,
                fast_settings,
                strip_depth_mm,
                &poses,
                &parent.active,
                hazard_catalog,
            )
            .map_err(|error| format!("partial hazard index: {error}"))?;
            let orientations = conflict_ruin_orientations(
                pieces[piece_index],
                &parent.state.placements[piece_index],
                derive_seed(
                    seed ^ CONFLICT_RUIN_ANGLE_SEED_DOMAIN,
                    layer,
                    parent_ordinal.saturating_mul(pieces.len()) + piece_index,
                ),
            );
            for (orientation_ordinal, (rotation_deg, mirrored)) in
                orientations.into_iter().enumerate()
            {
                work.parent_orientation_streams = work.parent_orientation_streams.saturating_add(1);
                if work.parent_orientation_streams > CONFLICT_RUIN_STREAM_CAP {
                    return Err("cap: parent-orientation stream budget exhausted".to_owned());
                }
                let orientation = RelaxedPlacement {
                    input_index: piece_index,
                    rotation_deg,
                    mirrored,
                    translate_x: 0.0,
                    translate_y: 0.0,
                };
                let local_collision = conflict_ruin_build_collision(
                    pieces[piece_index],
                    &orientation,
                    fast_settings,
                    work,
                )?;
                let position_seed = derive_seed(
                    seed ^ CONFLICT_RUIN_POSITION_SEED_DOMAIN,
                    layer.saturating_mul(CONFLICT_RUIN_BEAM_WIDTH) + parent_ordinal,
                    orientation_ordinal.saturating_mul(pieces.len()) + piece_index,
                );
                let proposals = conflict_ruin_positions(
                    &parent.state.placements[piece_index],
                    &orientation,
                    &local_collision,
                    parent,
                    fast_settings,
                    strip_depth_mm,
                    position_seed,
                    work,
                )?;
                let mut ranked = Vec::new();
                for placement in proposals {
                    work.cheap_queries = work.cheap_queries.saturating_add(1);
                    if work.cheap_queries > CONFLICT_RUIN_QUERY_CAP {
                        return Err("cap: cheap-query budget exhausted".to_owned());
                    }
                    let pose = hazard_pose(&placement);
                    let query = match index.query_unplaced(piece_index, pose) {
                        Ok(query) => query,
                        Err(error) if error.to_string().contains("query envelope") => continue,
                        Err(error) => return Err(format!("partial hazard query: {error}")),
                    };
                    let GeneralHazardQuery::Complete {
                        colliding_piece_ids,
                        ..
                    } = query
                    else {
                        return Err("partial unplaced query unexpectedly pruned".to_owned());
                    };
                    let mut proxy_loss = 0.0;
                    for fixed_piece_id in colliding_piece_ids {
                        if !parent.active[fixed_piece_id] {
                            return Err("inactive hazard leaked into a partial query".to_owned());
                        }
                        proxy_loss += index
                            .collision_pressure(piece_index, pose, fixed_piece_id)
                            .map_err(|error| format!("partial hazard pressure: {error}"))?;
                    }
                    ranked.push(ConflictRuinCandidate {
                        placement,
                        proxy_loss,
                    });
                }
                let required_current = ranked
                    .iter()
                    .find(|candidate| {
                        placement_key(&candidate.placement)
                            == placement_key(&parent.state.placements[piece_index])
                    })
                    .cloned();
                let mut finalists = conflict_ruin_diverse_finalists(
                    ranked,
                    fast_settings,
                    derive_seed(
                        seed ^ CONFLICT_RUIN_DIVERSITY_SEED_DOMAIN,
                        layer.saturating_mul(CONFLICT_RUIN_BEAM_WIDTH) + parent_ordinal,
                        orientation_ordinal.saturating_mul(pieces.len()) + piece_index,
                    ),
                );
                if let Some(required_current) = required_current {
                    if !finalists.iter().any(|candidate| {
                        placement_key(&candidate.placement)
                            == placement_key(&required_current.placement)
                    }) {
                        if finalists.len() == CONFLICT_RUIN_FINALISTS_PER_STREAM {
                            finalists.pop();
                        }
                        finalists.push(required_current);
                        work.required_current_finalists =
                            work.required_current_finalists.saturating_add(1);
                    }
                }
                for finalist in finalists {
                    work.exact_finalists = work.exact_finalists.saturating_add(1);
                    if work.exact_finalists > CONFLICT_RUIN_FINALIST_CAP {
                        return Err("cap: exact-finalist budget exhausted".to_owned());
                    }
                    children.push(conflict_ruin_exact_child(
                        parent,
                        pieces,
                        piece_index,
                        finalist.placement,
                        fast_settings,
                        work,
                    )?);
                }
            }
        }
        if children.is_empty() {
            return Err(format!(
                "conflict rebuild layer {layer} produced no exact-scored child"
            ));
        }
        children.sort_by(conflict_ruin_beam_order);
        let mut fingerprints = BTreeSet::new();
        children.retain(|child| fingerprints.insert(coupled_state_fingerprint(&child.state)));
        children.truncate(CONFLICT_RUIN_BEAM_WIDTH);
        work.partials_retained = work.partials_retained.saturating_add(children.len());
        beam = children;
    }
    Ok(ConflictRuinBuildOutcome {
        beam,
        initial_score,
    })
}

#[cfg(feature = "jagua-experimental")]
fn conflict_ruin_publish_work(
    diagnostics: &mut GeneralConflictRuinRebuildDiagnostics,
    work: ConflictRuinWork,
) {
    diagnostics.parent_orientation_streams = work.parent_orientation_streams;
    diagnostics.cheap_queries = work.cheap_queries;
    diagnostics.exact_finalists = work.exact_finalists;
    diagnostics.exact_pair_intersection_limit = work.pair_intersection_limit;
    diagnostics.exact_pair_intersections = work.exact_pair_intersections;
    diagnostics.required_current_finalists = work.required_current_finalists;
    diagnostics.orientation_build_limit = work.orientation_build_limit;
    diagnostics.orientation_builds = work.orientation_builds;
    diagnostics.transformed_output_vertices = work.transformed_output_vertices;
    diagnostics.feature_visits = work.feature_visits;
    diagnostics.pre_dedup_contact_attempts = work.pre_dedup_contact_attempts;
    diagnostics.deduplicated_proposals = work.deduplicated_proposals;
    diagnostics.clipper_input_vertices = work.clipper_input_vertices;
    diagnostics.clipper_output_vertices = work.clipper_output_vertices;
    diagnostics.partials_retained = work.partials_retained;
}

#[cfg(feature = "jagua-experimental")]
fn conflict_ruin_build_collision(
    piece: GeneralFastPiece<'_>,
    placement: &RelaxedPlacement,
    fast_settings: GeneralFastSettings,
    work: &mut ConflictRuinWork,
) -> Result<PolygonSet, String> {
    if work.orientation_builds >= work.orientation_build_limit {
        return Err("cap: transformed-orientation build budget exhausted".to_owned());
    }
    work.orientation_builds += 1;
    let collision = piece
        .polygon
        .transformed(
            placement.rotation_deg,
            placement.mirrored,
            placement.translate_x,
            placement.translate_y,
        )
        .and_then(|polygon| polygon.offset(collision_expansion_mm(fast_settings)))
        .map_err(|error| format!("conflict collision geometry: {error}"))?;
    work.transformed_output_vertices = work
        .transformed_output_vertices
        .saturating_add(collision.vertex_count());
    if work.transformed_output_vertices > CONFLICT_RUIN_TRANSFORMED_VERTEX_CAP {
        return Err("cap: transformed-output vertex budget exhausted".to_owned());
    }
    Ok(collision)
}

#[cfg(feature = "jagua-experimental")]
fn conflict_ruin_intersection_area(
    first: &PolygonSet,
    second: &PolygonSet,
    work: &mut ConflictRuinWork,
) -> Result<f64, String> {
    if work.exact_pair_intersections >= work.pair_intersection_limit {
        return Err("cap: exact pair-intersection budget exhausted".to_owned());
    }
    let input_vertices = first.vertex_count().saturating_add(second.vertex_count());
    if work.clipper_input_vertices.saturating_add(input_vertices)
        > CONFLICT_RUIN_CLIPPER_INPUT_VERTEX_CAP
    {
        return Err("cap: aggregate Clipper input-vertex budget exhausted".to_owned());
    }
    let result = first
        .intersection_area_with_complexity(second)
        .map_err(|error| format!("exact conflict intersection: {error}"))?;
    work.exact_pair_intersections += 1;
    work.clipper_input_vertices = work
        .clipper_input_vertices
        .saturating_add(result.input_vertices);
    work.clipper_output_vertices = work
        .clipper_output_vertices
        .saturating_add(result.output_vertices);
    if work.clipper_output_vertices > CONFLICT_RUIN_CLIPPER_OUTPUT_VERTEX_CAP {
        return Err("cap: aggregate Clipper output-vertex budget exhausted".to_owned());
    }
    Ok(result.area_mm2)
}

#[cfg(feature = "jagua-experimental")]
fn conflict_ruin_add_pair_area(score: &mut ConflictRuinExactScore, area_mm2: f64) {
    score.total_overlap_area_mm2 += area_mm2;
    if area_mm2 > 0.0 {
        score.positive_overlap_pairs = score.positive_overlap_pairs.saturating_add(1);
        score.maximum_pair_area_mm2 = score.maximum_pair_area_mm2.max(area_mm2);
    }
}

#[cfg(feature = "jagua-experimental")]
fn conflict_ruin_score_active_pairs(
    pieces: &[GeneralFastPiece<'_>],
    state: &RelaxedState,
    active: &[bool],
    pair_areas: &[f64],
) -> Result<ConflictRuinExactScore, String> {
    let mut score = ConflictRuinExactScore {
        total_overlap_area_mm2: 0.0,
        positive_overlap_pairs: 0,
        maximum_pair_area_mm2: 0.0,
        frontier_depth_mm: conflict_ruin_state_frontier(pieces, state, active)?,
    };
    for first in 0..pieces.len() {
        if !active[first] {
            continue;
        }
        for second in (first + 1)..pieces.len() {
            if active[second] {
                conflict_ruin_add_pair_area(
                    &mut score,
                    pair_areas[pair_slot(pieces.len(), first, second)],
                );
            }
        }
    }
    Ok(score)
}

#[cfg(feature = "jagua-experimental")]
fn conflict_ruin_state_frontier(
    pieces: &[GeneralFastPiece<'_>],
    state: &RelaxedState,
    active: &[bool],
) -> Result<f64, String> {
    state
        .placements
        .iter()
        .filter(|placement| active[placement.input_index])
        .map(|placement| {
            conflict_ruin_material_frontier(pieces[placement.input_index], placement)
                .map_err(|error| error.to_string())
        })
        .try_fold(f64::NEG_INFINITY, |frontier, candidate| {
            candidate.map(|candidate| frontier.max(candidate))
        })
}

#[cfg(feature = "jagua-experimental")]
fn conflict_ruin_orientations(
    piece: GeneralFastPiece<'_>,
    current: &RelaxedPlacement,
    seed: u64,
) -> Vec<(f64, bool)> {
    let mut orientations = Vec::with_capacity(CONFLICT_RUIN_ORIENTATIONS_PER_PARENT);
    let push = |orientations: &mut Vec<(f64, bool)>, angle: f64, mirrored: bool| {
        let angle = if piece.allow_rotation {
            continuous_angle(angle)
        } else {
            0.0
        };
        let mirrored = piece.allow_mirror && mirrored;
        let key = (angle_key(angle), mirrored);
        if !orientations
            .iter()
            .any(|(existing_angle, existing_mirror)| {
                (angle_key(*existing_angle), *existing_mirror) == key
            })
        {
            orientations.push((angle, mirrored));
        }
    };
    push(&mut orientations, current.rotation_deg, current.mirrored);
    let mirrors = if piece.allow_mirror {
        vec![current.mirrored, !current.mirrored]
    } else {
        vec![false]
    };
    for mirrored in mirrors.iter().copied() {
        for angle in [0.0, 90.0, 180.0, 270.0] {
            push(&mut orientations, angle, mirrored);
        }
    }
    if piece.allow_rotation {
        for mirrored in mirrors.iter().copied() {
            for region in piece.polygon.regions() {
                let points = region.outer.source_points();
                for index in 0..points.len() {
                    let first = points[index];
                    let second = points[(index + 1) % points.len()];
                    let delta_x = if mirrored {
                        first.x - second.x
                    } else {
                        second.x - first.x
                    };
                    let delta_y = second.y - first.y;
                    let edge_angle = delta_y.atan2(delta_x).to_degrees();
                    push(&mut orientations, -edge_angle, mirrored);
                    push(&mut orientations, 90.0 - edge_angle, mirrored);
                    if orientations.len() >= CONFLICT_RUIN_ORIENTATIONS_PER_PARENT {
                        break;
                    }
                }
                if orientations.len() >= CONFLICT_RUIN_ORIENTATIONS_PER_PARENT {
                    break;
                }
            }
            if orientations.len() >= CONFLICT_RUIN_ORIENTATIONS_PER_PARENT {
                break;
            }
        }
        let mut rng = SplitMix64::new(seed);
        let mut attempts = 0usize;
        while orientations.len() < CONFLICT_RUIN_ORIENTATIONS_PER_PARENT && attempts < 128 {
            let mirrored = piece.allow_mirror && rng.next_u64() & 1 == 1;
            push(&mut orientations, rng.range(0.0, 360.0), mirrored);
            attempts += 1;
        }
    }
    orientations.truncate(CONFLICT_RUIN_ORIENTATIONS_PER_PARENT);
    orientations
}

#[cfg(feature = "jagua-experimental")]
fn conflict_ruin_positions(
    current: &RelaxedPlacement,
    orientation: &RelaxedPlacement,
    local_collision: &PolygonSet,
    parent: &ConflictRuinBeamState,
    fast_settings: GeneralFastSettings,
    target_depth_mm: f64,
    seed: u64,
    work: &mut ConflictRuinWork,
) -> Result<Vec<RelaxedPlacement>, String> {
    let bounds = local_collision
        .bounds()
        .ok_or_else(|| "conflict orientation has empty collision geometry".to_owned())?;
    let inset = collision_sheet_inset_mm(fast_settings);
    let min_x = inset - bounds.min_x;
    let max_x = fast_settings.sheet_short_axis_mm - inset - bounds.max_x;
    let min_y = inset - bounds.min_y;
    let max_y = target_depth_mm - inset - bounds.max_y;
    if min_x > max_x || min_y > max_y {
        return Ok(Vec::new());
    }
    let current_x = current.translate_x.clamp(min_x, max_x);
    let current_y = current.translate_y.clamp(min_y, max_y);
    let center_x = (min_x + max_x) * 0.5;
    let center_y = (min_y + max_y) * 0.5;
    let mut categories = vec![Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()];
    categories[0].push((current_x, current_y));
    categories[1].extend([
        (min_x, min_y),
        (min_x, max_y),
        (max_x, min_y),
        (max_x, max_y),
        (min_x, center_y),
        (max_x, center_y),
        (center_x, min_y),
        (center_x, max_y),
    ]);
    for (fixed_index, fixed_collision) in parent.collisions.iter().enumerate() {
        if !parent.active[fixed_index] {
            continue;
        }
        let fixed_bounds = fixed_collision
            .as_ref()
            .and_then(PolygonSet::bounds)
            .ok_or_else(|| format!("active piece {fixed_index} has no collision bounds"))?;
        work.feature_visits = work.feature_visits.saturating_add(4);
        if work.feature_visits > CONFLICT_RUIN_FEATURE_VISIT_CAP {
            return Err("cap: source/fixed feature-visit budget exhausted".to_owned());
        }
        let left = (fixed_bounds.min_x - bounds.max_x).clamp(min_x, max_x);
        let right = (fixed_bounds.max_x - bounds.min_x).clamp(min_x, max_x);
        let below = (fixed_bounds.min_y - bounds.max_y).clamp(min_y, max_y);
        let above = (fixed_bounds.max_y - bounds.min_y).clamp(min_y, max_y);
        let contact_positions = [
            (left, current_y),
            (right, current_y),
            (current_x, below),
            (current_x, above),
            (left, below),
            (left, above),
            (right, below),
            (right, above),
        ];
        work.pre_dedup_contact_attempts = work
            .pre_dedup_contact_attempts
            .saturating_add(contact_positions.len());
        if work.pre_dedup_contact_attempts > CONFLICT_RUIN_CONTACT_ATTEMPT_CAP {
            return Err("cap: pre-dedup contact-attempt budget exhausted".to_owned());
        }
        categories[2].extend(contact_positions);
    }
    let width = (bounds.max_x - bounds.min_x).max(fast_settings.total_padding_mm);
    let height = (bounds.max_y - bounds.min_y).max(fast_settings.total_padding_mm);
    let mut focused_rng = SplitMix64::new(seed ^ 0xF0C5_5EED_0000_0001);
    for _ in 0..16 {
        categories[3].push((
            (current_x + focused_rng.range(-2.0 * width, 2.0 * width)).clamp(min_x, max_x),
            (current_y + focused_rng.range(-2.0 * height, 2.0 * height)).clamp(min_y, max_y),
        ));
    }
    let mut global_rng = SplitMix64::new(seed ^ 0x610B_A11E_0000_0001);
    for _ in 0..16 {
        categories[4].push((
            global_rng.range(min_x, max_x),
            global_rng.range(min_y, max_y),
        ));
    }
    let mut category_indices = vec![0usize; categories.len()];
    let mut deduplicated = BTreeSet::new();
    let mut placements = Vec::with_capacity(CONFLICT_RUIN_POSES_PER_STREAM);
    while placements.len() < CONFLICT_RUIN_POSES_PER_STREAM {
        let mut progressed = false;
        for category in 0..categories.len() {
            let Some((x, y)) = categories[category]
                .get(category_indices[category])
                .copied()
            else {
                continue;
            };
            category_indices[category] += 1;
            progressed = true;
            let placement = RelaxedPlacement {
                input_index: orientation.input_index,
                rotation_deg: orientation.rotation_deg,
                mirrored: orientation.mirrored,
                translate_x: snap_mm(x),
                translate_y: snap_mm(y),
            };
            if deduplicated.insert(placement_key(&placement)) {
                placements.push(placement);
                if placements.len() >= CONFLICT_RUIN_POSES_PER_STREAM {
                    break;
                }
            }
        }
        if !progressed {
            break;
        }
    }
    work.deduplicated_proposals = work.deduplicated_proposals.saturating_add(placements.len());
    if work.deduplicated_proposals > CONFLICT_RUIN_PROPOSAL_CAP {
        return Err("cap: deduplicated-proposal budget exhausted".to_owned());
    }
    Ok(placements)
}

#[cfg(feature = "jagua-experimental")]
fn conflict_ruin_diverse_finalists(
    mut candidates: Vec<ConflictRuinCandidate>,
    fast_settings: GeneralFastSettings,
    seed: u64,
) -> Vec<ConflictRuinCandidate> {
    candidates.sort_by(|first, second| {
        first
            .proxy_loss
            .total_cmp(&second.proxy_loss)
            .then_with(|| {
                conflict_ruin_diversity_key(&first.placement, seed)
                    .cmp(&conflict_ruin_diversity_key(&second.placement, seed))
            })
            .then_with(|| placement_key(&first.placement).cmp(&placement_key(&second.placement)))
    });
    candidates.truncate(CONFLICT_RUIN_FINALISTS_PER_STREAM.saturating_mul(4));
    let threshold = fast_settings
        .sheet_short_axis_mm
        .hypot(fast_settings.sheet_long_axis_mm)
        * 0.01;
    let mut selected = Vec::with_capacity(CONFLICT_RUIN_FINALISTS_PER_STREAM);
    for candidate in &candidates {
        if selected.iter().all(|selected: &ConflictRuinCandidate| {
            (selected.placement.translate_x - candidate.placement.translate_x)
                .hypot(selected.placement.translate_y - candidate.placement.translate_y)
                >= threshold
        }) {
            selected.push(candidate.clone());
            if selected.len() == CONFLICT_RUIN_FINALISTS_PER_STREAM {
                return selected;
            }
        }
    }
    for candidate in candidates {
        if selected.iter().any(|selected| {
            placement_key(&selected.placement) == placement_key(&candidate.placement)
        }) {
            continue;
        }
        selected.push(candidate);
        if selected.len() == CONFLICT_RUIN_FINALISTS_PER_STREAM {
            break;
        }
    }
    selected
}

#[cfg(feature = "jagua-experimental")]
fn conflict_ruin_diversity_key(placement: &RelaxedPlacement, seed: u64) -> u64 {
    let (_, angle, mirrored, x, y) = placement_key(placement);
    let mixed = seed
        ^ (angle as u64).rotate_left(7)
        ^ (x as u64).rotate_left(23)
        ^ (y as u64).rotate_left(41)
        ^ u64::from(mirrored);
    SplitMix64::new(mixed).next_u64()
}

#[cfg(feature = "jagua-experimental")]
fn conflict_ruin_exact_child(
    parent: &ConflictRuinBeamState,
    pieces: &[GeneralFastPiece<'_>],
    piece_index: usize,
    placement: RelaxedPlacement,
    fast_settings: GeneralFastSettings,
    work: &mut ConflictRuinWork,
) -> Result<ConflictRuinBeamState, String> {
    let collision =
        conflict_ruin_build_collision(pieces[piece_index], &placement, fast_settings, work)?;
    let inset = collision_sheet_inset_mm(fast_settings);
    if !collision.fits_rect(
        inset,
        inset,
        fast_settings.sheet_short_axis_mm - inset,
        parent.state.strip_depth_mm - inset,
    ) {
        return Err("exact finalist lies outside the target strip".to_owned());
    }
    let active_pair_count = parent.active.iter().filter(|active| **active).count();
    if work
        .exact_pair_intersections
        .saturating_add(active_pair_count)
        > work.pair_intersection_limit
    {
        return Err("cap: exact pair-intersection budget cannot fund a finalist".to_owned());
    }
    let candidate_vertices = collision.vertex_count();
    let required_input_vertices = parent
        .collisions
        .iter()
        .enumerate()
        .filter(|(index, _)| parent.active[*index])
        .map(|(_, fixed)| {
            candidate_vertices.saturating_add(fixed.as_ref().map_or(0, PolygonSet::vertex_count))
        })
        .sum::<usize>();
    if work
        .clipper_input_vertices
        .saturating_add(required_input_vertices)
        > CONFLICT_RUIN_CLIPPER_INPUT_VERTEX_CAP
    {
        return Err("cap: aggregate Clipper input budget cannot fund a finalist".to_owned());
    }
    let mut score = parent.score;
    for (fixed_index, fixed_collision) in parent.collisions.iter().enumerate() {
        if !parent.active[fixed_index] {
            continue;
        }
        let area = conflict_ruin_intersection_area(
            &collision,
            fixed_collision
                .as_ref()
                .ok_or_else(|| format!("active piece {fixed_index} has no collision polygon"))?,
            work,
        )?;
        conflict_ruin_add_pair_area(&mut score, area);
    }
    score.frontier_depth_mm = score.frontier_depth_mm.max(
        conflict_ruin_material_frontier(pieces[piece_index], &placement)
            .map_err(|error| error.to_string())?,
    );
    let mut child = parent.clone();
    child.state.placements[piece_index] = placement;
    child.active[piece_index] = true;
    child.collisions[piece_index] = Some(collision);
    child.score = score;
    Ok(child)
}

#[cfg(feature = "jagua-experimental")]
fn conflict_ruin_beam_order(
    first: &ConflictRuinBeamState,
    second: &ConflictRuinBeamState,
) -> Ordering {
    first
        .score
        .total_overlap_area_mm2
        .total_cmp(&second.score.total_overlap_area_mm2)
        .then_with(|| {
            first
                .score
                .positive_overlap_pairs
                .cmp(&second.score.positive_overlap_pairs)
        })
        .then_with(|| {
            first
                .score
                .maximum_pair_area_mm2
                .total_cmp(&second.score.maximum_pair_area_mm2)
        })
        .then_with(|| {
            first
                .score
                .frontier_depth_mm
                .total_cmp(&second.score.frontier_depth_mm)
        })
        .then_with(|| canonical_state_key(&first.state).cmp(&canonical_state_key(&second.state)))
}

#[cfg(feature = "jagua-experimental")]
fn run_conflict_ruin_retry<'a>(
    checkpoint: &CoupledFailedCheckpoint,
    pieces: &'a [GeneralFastPiece<'a>],
    fast_settings: GeneralFastSettings,
    mut relaxed_settings: GeneralRelaxedSettings,
    initial_state: RelaxedState,
    applied_rebuild: bool,
    worker_seeds: Vec<u64>,
) -> GeneralConflictRuinArmDiagnostics {
    let started = Instant::now();
    let mut diagnostics = GeneralConflictRuinArmDiagnostics {
        attempted: true,
        applied_rebuild,
        initial_state_fingerprint: Some(coupled_state_fingerprint(&initial_state)),
        ..GeneralConflictRuinArmDiagnostics::default()
    };
    relaxed_settings.collision_backend = GeneralRelaxedCollisionBackend::DynamicHazard;
    relaxed_settings.angle_seed_policy = GeneralRelaxedAngleSeedPolicy::ContinuousUniform;
    relaxed_settings.pressure_model = GeneralRelaxedPressureModel::DynamicPoles;
    relaxed_settings.angular_repair = GeneralAngularRepairSettings::disabled();
    relaxed_settings.synchronize_lanes = true;
    relaxed_settings.sweeps_per_epoch = COUPLED_SEPARATOR_ROUNDS;
    let outcome = run_coupled_separator_target(
        pieces,
        fast_settings,
        relaxed_settings,
        &checkpoint.incumbent,
        initial_state,
        checkpoint
            .target_ordinal
            .saturating_add(COUPLED_SEPARATOR_TARGETS),
        checkpoint.target_depth_mm,
        checkpoint.compression_split_mm,
        checkpoint.target_seed ^ CONFLICT_RUIN_SEED_DOMAIN,
        checkpoint.compression_seed ^ CONFLICT_RUIN_SEED_DOMAIN,
        worker_seeds,
        CoupledSeparatorArm::Treatment,
        CoupledRollbackRescorePolicy::StrictDerivedAgreement,
        // A mode-26 rung runs this side experiment too, but the ladder reads
        // only the boundary-projection arm, so widening the comparison here
        // would change nothing the ladder can see. It keeps the exact rule.
        CoupledRollbackComparison::Exact,
        false,
        checkpoint.catalog.clone(),
        checkpoint.hazard_catalog.clone(),
    );
    diagnostics.elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0;
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(error) => {
            diagnostics.failure_reason = Some(error.to_string());
            return diagnostics;
        }
    };
    diagnostics.final_state_fingerprint = Some(outcome.diagnostics.final_state_fingerprint.clone());
    diagnostics.exact_valid = outcome.diagnostics.exact_valid;
    diagnostics.accepted_depth_mm = outcome.diagnostics.accepted_depth_mm;
    diagnostics.failure_reason = outcome
        .diagnostics
        .failure_reason
        .clone()
        .or_else(|| outcome.diagnostics.cap_exhausted.clone());
    diagnostics.work = conflict_ruin_retry_work(outcome.work);
    if let Some(accepted) = outcome.accepted {
        diagnostics.final_placement_fingerprint =
            Some(coupled_fast_placement_fingerprint(&accepted.placements));
        diagnostics.final_placements = coupled_placement_diagnostics(&accepted.placements);
    }
    diagnostics.target = Some(outcome.diagnostics);
    diagnostics
}

#[cfg(feature = "jagua-experimental")]
fn conflict_ruin_retry_work(work: CoupledSeparatorWork) -> GeneralConflictRuinRetryWorkDiagnostics {
    GeneralConflictRuinRetryWorkDiagnostics {
        worker_sweeps: work.worker_sweeps,
        dynamic_queries: work.dynamic_queries,
        pressure_evaluations: work.pressure_evaluations,
        retained_confirmations: work.retained_confirmations,
        hazard_updates: work.hazard_updates,
        layout_loads: work.layout_loads,
        index_builds: work.index_builds,
        worker_full_score_pair_visits: work.worker_full_score_pair_visits,
        auditor_full_score_pair_visits: work.auditor_full_score_pair_visits,
        auditor_dynamic_queries: work.auditor_dynamic_queries,
        auditor_pressure_evaluations: work.auditor_pressure_evaluations,
        auditor_layout_loads: work.auditor_layout_loads,
        auditor_index_builds: work.auditor_index_builds,
    }
}

#[cfg(feature = "jagua-experimental")]
#[derive(Clone, Copy, Debug, Default)]
struct CoupledSeparatorWork {
    worker_sweeps: usize,
    dynamic_queries: usize,
    pressure_evaluations: usize,
    retained_confirmations: usize,
    hazard_updates: usize,
    layout_loads: usize,
    index_builds: usize,
    worker_full_score_pair_visits: usize,
    auditor_full_score_pair_visits: usize,
    auditor_dynamic_queries: usize,
    auditor_pressure_evaluations: usize,
    auditor_layout_loads: usize,
    auditor_index_builds: usize,
}

#[cfg(feature = "jagua-experimental")]
impl CoupledSeparatorWork {
    fn accumulate(&mut self, other: Self) {
        self.worker_sweeps = self.worker_sweeps.saturating_add(other.worker_sweeps);
        self.dynamic_queries = self.dynamic_queries.saturating_add(other.dynamic_queries);
        self.pressure_evaluations = self
            .pressure_evaluations
            .saturating_add(other.pressure_evaluations);
        self.retained_confirmations = self
            .retained_confirmations
            .saturating_add(other.retained_confirmations);
        self.hazard_updates = self.hazard_updates.saturating_add(other.hazard_updates);
        self.layout_loads = self.layout_loads.saturating_add(other.layout_loads);
        self.index_builds = self.index_builds.saturating_add(other.index_builds);
        self.worker_full_score_pair_visits = self
            .worker_full_score_pair_visits
            .saturating_add(other.worker_full_score_pair_visits);
        self.auditor_full_score_pair_visits = self
            .auditor_full_score_pair_visits
            .saturating_add(other.auditor_full_score_pair_visits);
        self.auditor_dynamic_queries = self
            .auditor_dynamic_queries
            .saturating_add(other.auditor_dynamic_queries);
        self.auditor_pressure_evaluations = self
            .auditor_pressure_evaluations
            .saturating_add(other.auditor_pressure_evaluations);
        self.auditor_layout_loads = self
            .auditor_layout_loads
            .saturating_add(other.auditor_layout_loads);
        self.auditor_index_builds = self
            .auditor_index_builds
            .saturating_add(other.auditor_index_builds);
    }
}

#[cfg(feature = "jagua-experimental")]
fn coupled_auditor_score(
    worker: &Mutex<LaneSearch<'_>>,
    state: &RelaxedState,
    weights: &BTreeMap<(usize, usize), f64>,
    pair_visits_per_score: usize,
) -> (Result<PairTracker, GeneralFastError>, CoupledSeparatorWork) {
    let _span = profiling::span(Phase::AuditorScore);
    let mut worker = match worker.lock() {
        Ok(worker) => worker,
        Err(_) => {
            return (
                Err(GeneralFastError::InvalidInput(
                    "coupled separator worker lock was poisoned".to_owned(),
                )),
                CoupledSeparatorWork::default(),
            );
        }
    };
    let saved_counters = worker.counters;
    worker.weights = weights.clone();
    let result = worker
        .prepare_dynamic_hazard(state)
        .and_then(|()| worker.score_state(state));
    let auditor_dynamic_queries = worker
        .counters
        .dynamic_hazard_queries
        .saturating_sub(saved_counters.dynamic_hazard_queries);
    let auditor_pressure_evaluations = worker
        .counters
        .dynamic_pressure_evaluations
        .saturating_sub(saved_counters.dynamic_pressure_evaluations);
    let auditor_layout_loads = worker
        .counters
        .dynamic_layout_loads
        .saturating_sub(saved_counters.dynamic_layout_loads);
    let auditor_index_builds = worker
        .counters
        .dynamic_index_builds
        .saturating_sub(saved_counters.dynamic_index_builds);
    worker.counters = saved_counters;
    (
        result,
        CoupledSeparatorWork {
            dynamic_queries: auditor_dynamic_queries,
            pressure_evaluations: auditor_pressure_evaluations,
            layout_loads: auditor_layout_loads,
            index_builds: auditor_index_builds,
            auditor_full_score_pair_visits: usize::from(auditor_layout_loads > 0)
                .saturating_mul(pair_visits_per_score),
            auditor_dynamic_queries,
            auditor_pressure_evaluations,
            auditor_layout_loads,
            auditor_index_builds,
            ..CoupledSeparatorWork::default()
        },
    )
}

#[cfg(feature = "jagua-experimental")]
fn run_coupled_separator_target<'a>(
    pieces: &'a [GeneralFastPiece<'a>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    incumbent: &GeneralFastResult,
    initial_state: RelaxedState,
    target_ordinal: usize,
    target_depth_mm: f64,
    compression_split_mm: f64,
    target_seed: u64,
    compression_seed: u64,
    worker_seeds: Vec<u64>,
    arm: CoupledSeparatorArm,
    rollback_rescore_policy: CoupledRollbackRescorePolicy,
    rollback_comparison: CoupledRollbackComparison,
    independent_final_audit: bool,
    catalog: Arc<SurrogateCatalog>,
    hazard_catalog: Arc<JaguaHazardCatalog>,
) -> Result<CoupledTargetOutcome, GeneralFastError> {
    let mut rollback_tally = RollbackComparisonTally::default();
    let pair_visits_per_score = pieces.len().saturating_mul(pieces.len().saturating_sub(1)) / 2;
    let workers = worker_seeds
        .iter()
        .map(|seed| {
            let mut worker = LegacyLaneSearch::new(
                pieces,
                fast_settings,
                relaxed_settings,
                *seed,
                catalog.clone(),
            );
            worker.dynamic_query_limit = Some(COUPLED_SEPARATOR_WORKER_QUERY_CAP);
            worker.hazard_catalog = Some(hazard_catalog.clone());
            worker.refine_rotation = arm.refines_rotation();
            Mutex::new(worker)
        })
        .collect::<Vec<_>>();
    let mut weights = BTreeMap::new();
    let initial_state_fingerprint = coupled_state_fingerprint(&initial_state);
    let mut auditor_work = CoupledSeparatorWork::default();
    let (initial_score, initial_audit_work) =
        coupled_auditor_score(&workers[0], &initial_state, &weights, pair_visits_per_score);
    auditor_work.accumulate(initial_audit_work);
    let initial_score = match initial_score {
        Ok(score) => score,
        Err(error) => {
            let (mut work, accounting_failure) =
                match coupled_separator_work(&workers, 0, pair_visits_per_score) {
                    Ok(work) => (work, None),
                    Err(accounting_error) => (
                        CoupledSeparatorWork::default(),
                        Some(format!("; work accounting: {accounting_error}")),
                    ),
                };
            work.accumulate(auditor_work);
            return Ok(CoupledTargetOutcome {
                diagnostics: GeneralCoupledSeparatorTargetDiagnostics {
                    ordinal: target_ordinal,
                    target_depth_mm,
                    compression_split_mm,
                    target_seed,
                    compression_seed,
                    worker_seeds,
                    initial_state_fingerprint: initial_state_fingerprint.clone(),
                    final_state_fingerprint: initial_state_fingerprint,
                    rounds: 0,
                    strikes: 0,
                    rollbacks: 0,
                    full_rescore_agreements: 0,
                    rollback_disagreements_tolerated: 0,
                    rollback_disagreement_max_pressure_ulps: 0,
                    initial_raw_loss: 0.0,
                    minimum_raw_loss: 0.0,
                    final_raw_loss: 0.0,
                    final_weighted_loss: 0.0,
                    feasible: false,
                    exact_valid: false,
                    exact_accepted: false,
                    exact_rejection_reason: None,
                    accepted_depth_mm: None,
                    boundary_projection: None,
                    cap_exhausted: None,
                    failure_reason: Some(format!(
                        "initial full score: {error}{}",
                        accounting_failure.unwrap_or_default()
                    )),
                },
                accepted: None,
                work,
                minimum: None,
                final_state: initial_state,
                exact_metrics: None,
                independent_audit: None,
            });
        }
    };
    let initial_raw_loss = initial_score.common_loss();
    let mut master = LaneOutcome {
        state: initial_state.clone(),
        score: initial_score.clone(),
        weights: weights.clone(),
        counters: WorkCounters::default(),
        selected_lane: 0,
        restart_disruptions: 0,
    };
    let mut minimum_raw_state = initial_state;
    let mut minimum_raw_score = initial_score;
    let mut no_improvement = 0usize;
    let mut strikes = 0usize;
    let mut strike_start_raw_loss = initial_raw_loss;
    let mut rollbacks = 0usize;
    let mut full_rescore_agreements = 0usize;
    let mut rounds = 0usize;
    let mut cap_exhausted = None;
    let mut failure_reason = None;
    for round in 0..COUPLED_SEPARATOR_ROUNDS {
        let ordinals = (0..COUPLED_SEPARATOR_WORKERS).collect::<Vec<_>>();
        let outcomes = map_slice_with_job_pool(&ordinals, |ordinal| {
            let mut worker = workers[*ordinal].lock().map_err(|_| {
                GeneralFastError::InvalidInput(
                    "coupled separator worker lock was poisoned".to_owned(),
                )
            })?;
            worker.weights = weights.clone();
            worker.run_sweep(master.state.clone(), round)
        });
        let mut selected = None::<(usize, LaneOutcome)>;
        for (ordinal, outcome) in outcomes.into_iter().enumerate() {
            let outcome = match outcome {
                Ok(outcome) => outcome,
                Err(error) => {
                    failure_reason = Some(format!("worker {ordinal}: {error}"));
                    break;
                }
            };
            if selected
                .as_ref()
                .is_none_or(|(selected_ordinal, selected_outcome)| {
                    compare_coupled_separator_outcomes(
                        ordinal,
                        &outcome,
                        *selected_ordinal,
                        selected_outcome,
                    ) == Ordering::Less
                })
            {
                selected = Some((ordinal, outcome));
            }
        }
        rounds = rounds.saturating_add(1);
        if failure_reason.is_some() {
            break;
        }
        match coupled_separator_cap_reason(&workers, rounds, pair_visits_per_score) {
            Ok(Some(reason)) => {
                cap_exhausted = Some(reason);
                break;
            }
            Ok(None) => {}
            Err(error) => {
                failure_reason = Some(format!("cap accounting: {error}"));
                break;
            }
        }
        let Some((selected_ordinal, mut selected)) = selected else {
            failure_reason = Some("no worker outcome was available".to_owned());
            break;
        };
        selected.selected_lane = selected_ordinal;
        selected.weights = weights.clone();
        let previous_minimum = minimum_raw_score.common_loss();
        let transition = match coupled_round_disposition(&selected.score, previous_minimum) {
            CoupledRoundDisposition::AcceptFeasible => {
                master = selected;
                break;
            }
            CoupledRoundDisposition::ContinueInfeasible(transition) => transition,
        };
        match transition {
            RawMinimumTransition::SubstantialImprovement => {
                minimum_raw_state = selected.state.clone();
                minimum_raw_score = selected.score.clone();
                no_improvement = 0;
            }
            RawMinimumTransition::MinorImprovement => {
                minimum_raw_state = selected.state.clone();
                minimum_raw_score = selected.score.clone();
            }
            RawMinimumTransition::NoImprovement => {
                no_improvement = no_improvement.saturating_add(1);
            }
        }
        apply_coupled_gls_update(&mut weights, &mut selected);
        master = selected;

        let mut reached_strike_limit = false;
        if no_improvement >= COUPLED_SEPARATOR_NO_IMPROVEMENT_LIMIT {
            no_improvement = 0;
            match rollback_rescore_policy {
                CoupledRollbackRescorePolicy::StrictDerivedAgreement => {
                    if minimum_raw_score.common_loss()
                        < strike_start_raw_loss * COUPLED_SEPARATOR_SUBSTANTIAL_RATIO
                    {
                        strikes = 0;
                    } else {
                        strikes = strikes.saturating_add(1);
                    }
                    strike_start_raw_loss = minimum_raw_score.common_loss();
                    let (restored_score, rollback_audit_work) = coupled_auditor_score(
                        &workers[0],
                        &minimum_raw_state,
                        &weights,
                        pair_visits_per_score,
                    );
                    auditor_work.accumulate(rollback_audit_work);
                    let restored_score = match restored_score {
                        Ok(score) => score,
                        Err(error) => {
                            failure_reason = Some(format!("rollback full score: {error}"));
                            break;
                        }
                    };
                    if let Some(disagreement) = raw_tracker_disagreement(
                        &restored_score,
                        &minimum_raw_score,
                        rollback_comparison,
                        &mut rollback_tally,
                    ) {
                        failure_reason =
                            Some(format!("{ROLLBACK_DISAGREEMENT_ABORT}: {disagreement}"));
                        break;
                    }
                    master = LaneOutcome {
                        state: minimum_raw_state.clone(),
                        score: restored_score,
                        weights: weights.clone(),
                        counters: WorkCounters::default(),
                        selected_lane: 0,
                        restart_disruptions: 0,
                    };
                    reached_strike_limit = strikes >= COUPLED_SEPARATOR_STRIKE_LIMIT;
                }
                CoupledRollbackRescorePolicy::CanonicalAuthoritativeRows => {
                    let (restored_score, rollback_audit_work) = coupled_auditor_score(
                        &workers[0],
                        &minimum_raw_state,
                        &weights,
                        pair_visits_per_score,
                    );
                    auditor_work.accumulate(rollback_audit_work);
                    let restored_score = match restored_score {
                        Ok(score) => score,
                        Err(error) => {
                            failure_reason = Some(format!("rollback full score: {error}"));
                            break;
                        }
                    };
                    if let Some(disagreement) = authoritative_raw_tracker_disagreement_under(
                        &restored_score,
                        &minimum_raw_score,
                        rollback_comparison,
                        &mut rollback_tally,
                    ) {
                        failure_reason =
                            Some(format!("{ROLLBACK_DISAGREEMENT_ABORT}: {disagreement}"));
                        break;
                    }
                    reached_strike_limit = install_canonical_coupled_rollback(
                        restored_score,
                        &minimum_raw_state,
                        &mut minimum_raw_score,
                        &mut master,
                        &weights,
                        &mut strikes,
                        &mut strike_start_raw_loss,
                    );
                }
            }
            full_rescore_agreements = full_rescore_agreements.saturating_add(1);
            rollbacks = rollbacks.saturating_add(1);
        }

        if reached_strike_limit {
            break;
        }
    }

    let mut work = match coupled_separator_work(&workers, rounds, pair_visits_per_score) {
        Ok(work) => work,
        Err(error) => {
            failure_reason.get_or_insert_with(|| format!("work accounting: {error}"));
            CoupledSeparatorWork::default()
        }
    };
    work.accumulate(auditor_work);
    if work.auditor_layout_loads > COUPLED_SEPARATOR_AUDITOR_FULL_SCORES {
        cap_exhausted = Some("auditor full-score cap".to_owned());
    }
    let mut independent_audit = None;
    if independent_final_audit
        && failure_reason.is_none()
        && cap_exhausted.is_none()
        && !master.score.feasible()
    {
        let mut audit_diagnostics = GeneralPrecompressionIndependentAuditDiagnostics {
            attempted: true,
            ..GeneralPrecompressionIndependentAuditDiagnostics::default()
        };
        let (fresh_score, fresh_audit_work) =
            coupled_auditor_score(&workers[0], &master.state, &weights, pair_visits_per_score);
        work.accumulate(fresh_audit_work);
        if work.auditor_layout_loads > COUPLED_SEPARATOR_AUDITOR_FULL_SCORES {
            cap_exhausted = Some("auditor full-score cap".to_owned());
            audit_diagnostics.rejection_reason = Some("auditor full-score cap".to_owned());
            independent_audit = Some(CoupledIndependentAuditOutcome {
                diagnostics: audit_diagnostics,
                metrics: None,
            });
        } else {
            match fresh_score {
                Err(error) => {
                    audit_diagnostics.rejection_reason = Some(format!("final full score: {error}"));
                    independent_audit = Some(CoupledIndependentAuditOutcome {
                        diagnostics: audit_diagnostics,
                        metrics: None,
                    });
                }
                Ok(fresh_score) => {
                    if let Some(disagreement) = coupled_tracker_disagreement(
                        &fresh_score,
                        &master.score,
                        rollback_rescore_policy,
                        rollback_comparison,
                        &mut rollback_tally,
                    ) {
                        audit_diagnostics.rejection_reason = Some(format!(
                            "final tracker disagrees with a complete rescore: {disagreement}"
                        ));
                        independent_audit = Some(CoupledIndependentAuditOutcome {
                            diagnostics: audit_diagnostics,
                            metrics: None,
                        });
                    } else {
                        audit_diagnostics.fresh_score_agreement = true;
                        audit_diagnostics.final_positive_pairs =
                            Some(fresh_score.collision_pairs.len());
                        audit_diagnostics.final_boundary_violations =
                            Some(fresh_score.boundary_violations);
                        audit_diagnostics.final_boundary_loss = Some(fresh_score.boundary_loss);
                        audit_diagnostics.positive_boundary_rows = fresh_score
                            .boundaries
                            .iter()
                            .enumerate()
                            .filter(|(_, boundary)| {
                                boundary.violations > 0 || boundary.raw_loss > 0.0
                            })
                            .map(|(piece_index, boundary)| {
                                GeneralPrecompressionBoundaryRowDiagnostics {
                                    piece_id: pieces[piece_index].id.to_owned(),
                                    violations: boundary.violations,
                                    raw_loss: boundary.raw_loss,
                                }
                            })
                            .collect();
                        let placements = to_fast_placements(&master.state, pieces);
                        audit_diagnostics.audited_placement_fingerprint =
                            Some(coupled_fast_placement_fingerprint(&placements));
                        if fresh_score.collision_pairs.is_empty() {
                            audit_diagnostics.independent_audit_count = 1;
                            match validate_and_measure_placements(
                                pieces,
                                &placements,
                                fast_settings,
                            ) {
                                Ok(metrics) => {
                                    audit_diagnostics.independent_audit_valid = true;
                                    audit_diagnostics.used_short_axis_span_mm =
                                        Some(metrics.used_short_axis_span_mm);
                                    audit_diagnostics.used_long_axis_depth_mm =
                                        Some(metrics.used_long_axis_depth_mm);
                                    audit_diagnostics.unused_short_axis_projection_mm =
                                        Some(metrics.unused_short_axis_projection_mm);
                                    audit_diagnostics.occupied_envelope_area_mm2 =
                                        Some(metrics.occupied_envelope_area_mm2);
                                    independent_audit = Some(CoupledIndependentAuditOutcome {
                                        diagnostics: audit_diagnostics,
                                        metrics: Some(metrics),
                                    });
                                }
                                Err(error) => {
                                    audit_diagnostics.rejection_reason = Some(error.to_string());
                                    independent_audit = Some(CoupledIndependentAuditOutcome {
                                        diagnostics: audit_diagnostics,
                                        metrics: None,
                                    });
                                }
                            }
                        } else {
                            audit_diagnostics.rejection_reason = Some(
                                "fresh final score retained positive collision pairs".to_owned(),
                            );
                            independent_audit = Some(CoupledIndependentAuditOutcome {
                                diagnostics: audit_diagnostics,
                                metrics: None,
                            });
                        }
                    }
                }
            }
        }
    }
    let mut exact_valid = false;
    let mut exact_accepted = false;
    let mut exact_rejection_reason = None;
    let mut accepted_depth_mm = None;
    let mut exact_metrics = None;
    let accepted = if failure_reason.is_none() && cap_exhausted.is_none() && master.score.feasible()
    {
        let placements = to_fast_placements(&master.state, pieces);
        match validate_and_measure_placements(pieces, &placements, fast_settings) {
            Ok(metrics) => {
                exact_valid = true;
                exact_metrics = Some(metrics);
                if metrics.used_long_axis_depth_mm < incumbent.used_long_axis_depth_mm {
                    exact_accepted = true;
                    accepted_depth_mm = Some(metrics.used_long_axis_depth_mm);
                    let mut result = incumbent.clone();
                    result.placements = placements;
                    result.unplaced_piece_ids.clear();
                    result.used_short_axis_span_mm = metrics.used_short_axis_span_mm;
                    result.used_long_axis_depth_mm = metrics.used_long_axis_depth_mm;
                    result.unused_short_axis_projection_mm =
                        metrics.unused_short_axis_projection_mm;
                    result.occupied_envelope_area_mm2 = metrics.occupied_envelope_area_mm2;
                    Some(result)
                } else {
                    exact_rejection_reason =
                        Some("exact-valid endpoint did not improve the incumbent".to_owned());
                    None
                }
            }
            Err(error) => {
                exact_rejection_reason = Some(error.to_string());
                None
            }
        }
    } else {
        None
    };
    let diagnostics = GeneralCoupledSeparatorTargetDiagnostics {
        ordinal: target_ordinal,
        target_depth_mm,
        compression_split_mm,
        target_seed,
        compression_seed,
        worker_seeds,
        initial_state_fingerprint,
        final_state_fingerprint: coupled_state_fingerprint(&master.state),
        rounds,
        strikes,
        rollbacks,
        full_rescore_agreements,
        rollback_disagreements_tolerated: rollback_tally.tolerated,
        rollback_disagreement_max_pressure_ulps: rollback_tally.max_pressure_ulps,
        initial_raw_loss,
        minimum_raw_loss: minimum_raw_score.common_loss(),
        final_raw_loss: master.score.common_loss(),
        final_weighted_loss: master.score.weighted_loss,
        feasible: master.score.feasible(),
        exact_valid,
        exact_accepted,
        exact_rejection_reason,
        accepted_depth_mm,
        boundary_projection: None,
        cap_exhausted,
        failure_reason,
    };
    Ok(CoupledTargetOutcome {
        diagnostics,
        accepted,
        work,
        minimum: Some(CoupledMinimumCheckpoint {
            state: minimum_raw_state,
            score: minimum_raw_score,
        }),
        final_state: master.state,
        exact_metrics,
        independent_audit,
    })
}

#[cfg(feature = "jagua-experimental")]
fn raw_minimum_transition(candidate: f64, retained: f64) -> RawMinimumTransition {
    if candidate >= retained {
        RawMinimumTransition::NoImprovement
    } else if candidate < retained * COUPLED_SEPARATOR_SUBSTANTIAL_RATIO {
        RawMinimumTransition::SubstantialImprovement
    } else {
        RawMinimumTransition::MinorImprovement
    }
}

#[cfg(feature = "jagua-experimental")]
fn coupled_round_disposition(
    score: &PairTracker,
    retained_raw_loss: f64,
) -> CoupledRoundDisposition {
    if score.feasible() {
        CoupledRoundDisposition::AcceptFeasible
    } else {
        CoupledRoundDisposition::ContinueInfeasible(raw_minimum_transition(
            score.common_loss(),
            retained_raw_loss,
        ))
    }
}

#[cfg(feature = "jagua-experimental")]
fn apply_coupled_gls_update(
    weights: &mut BTreeMap<(usize, usize), f64>,
    selected: &mut LaneOutcome,
) {
    update_weights(weights, &selected.score.collision_pairs);
    selected.weights = weights.clone();
    refresh_weighted_loss(&mut selected.score, weights);
}

#[cfg(feature = "jagua-experimental")]
fn compare_coupled_separator_outcomes(
    first_ordinal: usize,
    first: &LaneOutcome,
    second_ordinal: usize,
    second: &LaneOutcome,
) -> Ordering {
    first
        .score
        .weighted_loss
        .total_cmp(&second.score.weighted_loss)
        .then_with(|| {
            first
                .score
                .common_loss()
                .total_cmp(&second.score.common_loss())
        })
        .then_with(|| {
            first
                .score
                .boundary_loss
                .total_cmp(&second.score.boundary_loss)
        })
        .then_with(|| {
            first
                .score
                .boundary_violations
                .cmp(&second.score.boundary_violations)
        })
        .then_with(|| {
            first
                .score
                .collision_pairs
                .len()
                .cmp(&second.score.collision_pairs.len())
        })
        .then_with(|| canonical_state_key(&first.state).cmp(&canonical_state_key(&second.state)))
        .then_with(|| first_ordinal.cmp(&second_ordinal))
}

#[cfg(feature = "jagua-experimental")]
fn install_canonical_coupled_rollback(
    restored_score: PairTracker,
    minimum_raw_state: &RelaxedState,
    minimum_raw_score: &mut PairTracker,
    master: &mut LaneOutcome,
    weights: &BTreeMap<(usize, usize), f64>,
    strikes: &mut usize,
    strike_start_raw_loss: &mut f64,
) -> bool {
    let canonical_raw_loss = restored_score.common_loss();
    if canonical_raw_loss < *strike_start_raw_loss * COUPLED_SEPARATOR_SUBSTANTIAL_RATIO {
        *strikes = 0;
    } else {
        *strikes = strikes.saturating_add(1);
    }
    *strike_start_raw_loss = canonical_raw_loss;
    *minimum_raw_score = restored_score.clone();
    *master = LaneOutcome {
        state: minimum_raw_state.clone(),
        score: restored_score,
        weights: weights.clone(),
        counters: WorkCounters::default(),
        selected_lane: 0,
        restart_disruptions: 0,
    };
    *strikes >= COUPLED_SEPARATOR_STRIKE_LIMIT
}

#[cfg(feature = "jagua-experimental")]
fn raw_tracker_disagreement(
    first: &PairTracker,
    second: &PairTracker,
    comparison: CoupledRollbackComparison,
    tally: &mut RollbackComparisonTally,
) -> Option<String> {
    if let Some(disagreement) =
        authoritative_raw_tracker_disagreement_under(first, second, comparison, tally)
    {
        return Some(disagreement);
    }
    if first.incident_raw_loss.len() != second.incident_raw_loss.len()
        || first
            .incident_raw_loss
            .iter()
            .zip(&second.incident_raw_loss)
            .any(|(first, second)| !derived_losses_agree(*first, *second, comparison, tally))
    {
        let difference = first
            .incident_raw_loss
            .iter()
            .zip(&second.incident_raw_loss)
            .enumerate()
            .find(|(_, (first, second))| first != second)
            .map(|(index, (first, second))| {
                format!("incident loss {index}: {first:.17e} != {second:.17e}")
            })
            .unwrap_or_else(|| "incident loss vector length differs".to_owned());
        return Some(difference);
    }
    if !derived_losses_agree(first.boundary_loss, second.boundary_loss, comparison, tally) {
        return Some(format!(
            "boundary loss {:.17e} != {:.17e}",
            first.boundary_loss, second.boundary_loss
        ));
    }
    None
}

/// Whether two *derived* rollback losses - the per-piece incident sums and the
/// boundary total - may be treated as the same reading.
///
/// These are `f64` running sums whose last bit depends on accumulation order,
/// so one `f64` ulp is the right and only tolerance for them under either
/// policy. A sum of `f32`-valued pressure terms is not itself an `f32`, so
/// there is no `f32` rounding floor here to widen to; the call below is
/// [`RollbackMagnitude::NativeF64`] precisely to say so, and it also keeps the
/// gap visible in the tally.
#[cfg(feature = "jagua-experimental")]
fn derived_losses_agree(
    first: f64,
    second: f64,
    comparison: CoupledRollbackComparison,
    tally: &mut RollbackComparisonTally,
) -> bool {
    if equal_within_one_ulp(first, second) {
        return true;
    }
    rollback_losses_agree(
        first,
        second,
        RollbackMagnitude::NativeF64,
        comparison,
        tally,
    )
}

/// The authoritative comparison under the bit-exact rule, which is what every
/// arm outside the mode-26 clamp runs. Used by the tests that pin that rule.
#[cfg(all(test, feature = "jagua-experimental"))]
fn authoritative_raw_tracker_disagreement(
    first: &PairTracker,
    second: &PairTracker,
) -> Option<String> {
    authoritative_raw_tracker_disagreement_under(
        first,
        second,
        CoupledRollbackComparison::Exact,
        &mut RollbackComparisonTally::default(),
    )
}

/// The authoritative rollback comparison, under an explicit comparison policy.
///
/// The rows checked are the same ones `Exact` has always checked; `comparison`
/// only decides how two *loss magnitudes* are judged equal. Everything that
/// describes the shape of the state - piece count, row counts, pair indices,
/// boundary violation counts - is compared bit for bit under either policy, so
/// a tolerant arm can never accept a rollback that disagrees about which pairs
/// collide or how many boundaries are violated.
///
/// `tally` records what the tolerance actually did: how many magnitudes were
/// tolerated, and the widest `f32`-ulp gap seen across every magnitude
/// comparison (including one that failed), so the budget stays measurable
/// rather than assumed.
#[cfg(feature = "jagua-experimental")]
fn authoritative_raw_tracker_disagreement_under(
    first: &PairTracker,
    second: &PairTracker,
    comparison: CoupledRollbackComparison,
    tally: &mut RollbackComparisonTally,
) -> Option<String> {
    if first.piece_count != second.piece_count {
        return Some(format!(
            "piece count {} != {}",
            first.piece_count, second.piece_count
        ));
    }
    if first.boundaries.len() != second.boundaries.len() {
        return Some("boundary rows differ".to_owned());
    }
    for (first, second) in first.boundaries.iter().zip(&second.boundaries) {
        if first.violations != second.violations
            || !rollback_losses_agree(
                first.raw_loss,
                second.raw_loss,
                RollbackMagnitude::NativeF64,
                comparison,
                tally,
            )
        {
            return Some("boundary rows differ".to_owned());
        }
    }
    if first.boundary_violations != second.boundary_violations {
        return Some(format!(
            "boundary violation count {} != {}",
            first.boundary_violations, second.boundary_violations
        ));
    }
    if first.collision_pairs.len() != second.collision_pairs.len() {
        return Some("collision rows differ".to_owned());
    }
    for (first, second) in first.collision_pairs.iter().zip(&second.collision_pairs) {
        if first.0 != second.0
            || first.1 != second.1
            || !rollback_losses_agree(
                first.2,
                second.2,
                RollbackMagnitude::PairPressure,
                comparison,
                tally,
            )
        {
            return Some("collision rows differ".to_owned());
        }
    }
    if first.pairs.len() != second.pairs.len() {
        return Some("pair rows differ".to_owned());
    }
    for (first, second) in first.pairs.iter().zip(&second.pairs) {
        if first.normalization_scale != second.normalization_scale
            || !rollback_losses_agree(
                first.raw_loss,
                second.raw_loss,
                RollbackMagnitude::PairPressure,
                comparison,
                tally,
            )
        {
            return Some("pair rows differ".to_owned());
        }
    }
    None
}

/// What a tolerant rollback comparison did, for the ladder diagnostics.
#[cfg(feature = "jagua-experimental")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct RollbackComparisonTally {
    /// Magnitudes that differed bitwise but were accepted as the same reading.
    tolerated: usize,
    /// The widest `f32`-ulp gap observed across every magnitude that differed
    /// bitwise, tolerated or not. `0` means nothing ever differed.
    max_pressure_ulps: u32,
}

/// Where a rollback magnitude's low bits come from, which is what decides the
/// unit its disagreements may be measured in.
///
/// [`COUPLED_ROLLBACK_PRESSURE_ULP_BUDGET`] is denominated in `f32` ulps
/// because the rounding floor it absorbs is an `f32` one. Applying it to a
/// magnitude that is `f64` all the way down is not a loose version of the same
/// rule, it is a different and far weaker one: narrowing an `f64` to `f32`
/// discards about 29 bits, so 64 `f32` ulps spans on the order of 1e10 `f64`
/// ulps. Provenance therefore has to be carried to the comparison rather than
/// inferred there.
#[cfg(feature = "jagua-experimental")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RollbackMagnitude {
    /// A per-pair collision pressure - the pair rows and the collision rows.
    ///
    /// Under the dynamic-pole model these arms run,
    /// [`JaguaHazardIndex::collision_pressure`] ends in `f64::from(f32)`, so
    /// the value *is* an `f32` and narrowing back is lossless: the `f32` bit
    /// distance is the exact unit of a summation-order disagreement. The
    /// `f64`-native pole models (`pole_overlap_pressure`) produce genuine
    /// `f64`s through the same field, so the budget is additionally gated on
    /// [`pressure_rounding_floor_applies`] rather than on an assumption about
    /// which model is configured.
    PairPressure,
    /// A magnitude computed in `f64` throughout: the boundary penalty
    /// (`boundary_penalty` is pure `f64` area arithmetic and never touches a
    /// pole), and every running sum built from the terms above - the per-piece
    /// incident totals and the boundary total. A sum of `f32`-valued terms is
    /// not itself an `f32`, so none of these has an `f32` rounding floor to
    /// absorb.
    ///
    /// These keep the one-`f64`-ulp accumulation-order rule the engine has
    /// always applied to them.
    NativeF64,
}

/// Whether narrowing both readings to `f32` is lossless, i.e. whether an `f32`
/// ulp count is a real measurement of the gap between them rather than an
/// artefact of throwing away 29 bits.
///
/// True for anything that reached `f64` through `f64::from(some_f32)`, which is
/// what the dynamic-pole pressure path produces.
#[cfg(feature = "jagua-experimental")]
fn pressure_rounding_floor_applies(first: f64, second: f64) -> bool {
    is_exactly_representable_as_f32(first) && is_exactly_representable_as_f32(second)
}

#[cfg(feature = "jagua-experimental")]
fn is_exactly_representable_as_f32(value: f64) -> bool {
    let narrowed = value as f32;
    narrowed.is_finite() && f64::from(narrowed) == value
}

/// Whether two rollback loss magnitudes may be treated as the same reading.
///
/// `Exact` is untouched by the provenance scoping in every respect - it refuses
/// any bitwise difference, and it records the same gap it always did - so every
/// arm outside the mode-26 clamp stays bit-identical.
#[cfg(feature = "jagua-experimental")]
fn rollback_losses_agree(
    first: f64,
    second: f64,
    magnitude: RollbackMagnitude,
    comparison: CoupledRollbackComparison,
    tally: &mut RollbackComparisonTally,
) -> bool {
    if first == second {
        return true;
    }
    // Measured for every magnitude regardless of provenance: the diagnostic
    // promises "the widest `f32`-ulp gap any comparison saw, tolerated or not",
    // and a gap that is now refused is exactly the kind worth still seeing.
    let distance = pressure_ulp_distance(first, second);
    tally.max_pressure_ulps = tally.max_pressure_ulps.max(distance);
    match comparison {
        CoupledRollbackComparison::Exact => false,
        CoupledRollbackComparison::ToleratesPoleRounding => {
            let tolerated = match magnitude {
                RollbackMagnitude::PairPressure
                    if pressure_rounding_floor_applies(first, second) =>
                {
                    distance <= COUPLED_ROLLBACK_PRESSURE_ULP_BUDGET
                }
                RollbackMagnitude::PairPressure | RollbackMagnitude::NativeF64 => {
                    equal_within_one_ulp(first, second)
                }
            };
            if tolerated {
                tally.tolerated = tally.tolerated.saturating_add(1);
            }
            tolerated
        }
    }
}

/// The gap between two loss magnitudes in `f32` units in the last place.
///
/// The dynamic-pole pressure is an `f32` series widened to `f64`
/// ([`JaguaHazardIndex::collision_pressure`] ends in `f64::from(..)`), so
/// narrowing back is exact and the `f32` bit distance is the natural unit for
/// a summation-order disagreement. A magnitude that is genuinely `f64`-native
/// (a boundary penalty) and differs by an `f64` ulp collapses to the same
/// `f32`, so it reports `0` and is likewise inside any budget.
///
/// Values that are not both finite, or that differ in sign, are infinitely far
/// apart: those are real disagreements, not rounding.
#[cfg(feature = "jagua-experimental")]
fn pressure_ulp_distance(first: f64, second: f64) -> u32 {
    if first == second {
        return 0;
    }
    if !first.is_finite()
        || !second.is_finite()
        || first.is_sign_negative() != second.is_sign_negative()
    {
        return u32::MAX;
    }
    let (first, second) = (first as f32, second as f32);
    if !first.is_finite() || !second.is_finite() {
        return u32::MAX;
    }
    first.to_bits().abs_diff(second.to_bits())
}

#[cfg(feature = "jagua-experimental")]
fn coupled_tracker_disagreement(
    first: &PairTracker,
    second: &PairTracker,
    policy: CoupledRollbackRescorePolicy,
    comparison: CoupledRollbackComparison,
    tally: &mut RollbackComparisonTally,
) -> Option<String> {
    match policy {
        CoupledRollbackRescorePolicy::StrictDerivedAgreement => {
            raw_tracker_disagreement(first, second, comparison, tally)
        }
        CoupledRollbackRescorePolicy::CanonicalAuthoritativeRows => {
            authoritative_raw_tracker_disagreement_under(first, second, comparison, tally)
        }
    }
}

#[cfg(feature = "jagua-experimental")]
fn equal_within_one_ulp(first: f64, second: f64) -> bool {
    first == second
        || (first.is_finite()
            && second.is_finite()
            && first.is_sign_negative() == second.is_sign_negative()
            && first.to_bits().abs_diff(second.to_bits()) <= 1)
}

#[cfg(feature = "jagua-experimental")]
fn coupled_separator_cap_reason(
    workers: &[Mutex<LaneSearch<'_>>],
    rounds: usize,
    pair_visits_per_score: usize,
) -> Result<Option<String>, GeneralFastError> {
    let full_score_pair_visits = rounds.saturating_mul(pair_visits_per_score);
    if full_score_pair_visits > COUPLED_SEPARATOR_WORKER_FULL_SCORE_PAIR_VISIT_CAP {
        return Ok(Some("worker full-score pair-visit cap".to_owned()));
    }
    for (ordinal, worker) in workers.iter().enumerate() {
        let worker = worker.lock().map_err(|_| {
            GeneralFastError::InvalidInput("coupled separator worker lock was poisoned".to_owned())
        })?;
        let counters = worker.counters;
        let reason = if counters.dynamic_hazard_queries > COUPLED_SEPARATOR_WORKER_QUERY_CAP {
            Some("complete-query cap")
        } else if counters.dynamic_pressure_evaluations > COUPLED_SEPARATOR_WORKER_PRESSURE_CAP {
            Some("pressure-evaluation cap")
        } else if counters.retained_f64_confirmations > COUPLED_SEPARATOR_WORKER_CONFIRMATION_CAP {
            Some("retained-confirmation cap")
        } else if counters.dynamic_hazard_updates > COUPLED_SEPARATOR_WORKER_UPDATE_CAP {
            Some("hazard-update cap")
        } else if counters.dynamic_layout_loads > COUPLED_SEPARATOR_WORKER_LAYOUT_LOAD_CAP {
            Some("layout-load cap")
        } else if counters.dynamic_index_builds > 1 {
            Some("index-build cap")
        } else {
            None
        };
        if let Some(reason) = reason {
            return Ok(Some(format!("worker {ordinal} {reason}")));
        }
    }
    Ok(None)
}

#[cfg(feature = "jagua-experimental")]
fn coupled_separator_work(
    workers: &[Mutex<LaneSearch<'_>>],
    rounds: usize,
    pair_visits_per_score: usize,
) -> Result<CoupledSeparatorWork, GeneralFastError> {
    let mut work = CoupledSeparatorWork::default();
    for worker in workers {
        let worker = worker.lock().map_err(|_| {
            GeneralFastError::InvalidInput("coupled separator worker lock was poisoned".to_owned())
        })?;
        work.dynamic_queries = work
            .dynamic_queries
            .saturating_add(worker.counters.dynamic_hazard_queries);
        work.pressure_evaluations = work
            .pressure_evaluations
            .saturating_add(worker.counters.dynamic_pressure_evaluations);
        work.retained_confirmations = work
            .retained_confirmations
            .saturating_add(worker.counters.retained_f64_confirmations);
        work.hazard_updates = work
            .hazard_updates
            .saturating_add(worker.counters.dynamic_hazard_updates);
        work.layout_loads = work
            .layout_loads
            .saturating_add(worker.counters.dynamic_layout_loads);
        work.index_builds = work
            .index_builds
            .saturating_add(worker.counters.dynamic_index_builds);
    }
    work.worker_sweeps = rounds.saturating_mul(workers.len());
    work.worker_full_score_pair_visits = work.worker_sweeps.saturating_mul(pair_visits_per_score);
    Ok(work)
}

#[cfg(feature = "jagua-experimental")]
fn coupled_state_fingerprint(state: &RelaxedState) -> String {
    let mut digest = Sha256::new();
    digest.update(grid_key(state.strip_depth_mm).to_le_bytes());
    for (input_index, angle, mirrored, translate_x, translate_y) in canonical_state_key(state) {
        digest.update(input_index.to_le_bytes());
        digest.update(angle.to_le_bytes());
        digest.update([u8::from(mirrored)]);
        digest.update(translate_x.to_le_bytes());
        digest.update(translate_y.to_le_bytes());
    }
    format!("{:x}", digest.finalize())
}

pub(super) fn coupled_placement_diagnostics(
    placements: &[GeneralFastPlacement],
) -> Vec<GeneralCoupledSeparatorPlacementDiagnostics> {
    placements
        .iter()
        .map(|placement| GeneralCoupledSeparatorPlacementDiagnostics {
            piece_id: placement.piece_id.clone(),
            rotation_deg: placement.rotation_deg,
            mirrored: placement.mirrored,
            translate_short_axis: placement.translate_short_axis,
            translate_long_axis: placement.translate_long_axis,
        })
        .collect()
}

/// The canonical placement fingerprint the engine reports as
/// `finalPlacementFingerprint` and `parentFingerprint`.
///
/// Exposed so an external fixture that *claims* a fingerprint can be checked
/// against the placements it actually carries, without anyone reimplementing
/// the digest and drifting from it. It is order-independent (placements are
/// sorted by piece ID) and reads poses through the same angle/grid keys the
/// search compares them with, so it identifies a layout rather than a
/// serialization of one.
pub fn general_placement_fingerprint(placements: &[GeneralFastPlacement]) -> String {
    coupled_fast_placement_fingerprint(placements)
}

fn coupled_fast_placement_fingerprint(placements: &[GeneralFastPlacement]) -> String {
    let mut canonical = placements.iter().collect::<Vec<_>>();
    canonical.sort_by(|first, second| first.piece_id.cmp(&second.piece_id));
    let mut digest = Sha256::new();
    for placement in canonical {
        digest.update((placement.piece_id.len() as u64).to_le_bytes());
        digest.update(placement.piece_id.as_bytes());
        digest.update(angle_key(placement.rotation_deg).to_le_bytes());
        digest.update([u8::from(placement.mirrored)]);
        digest.update(grid_key(placement.translate_short_axis).to_le_bytes());
        digest.update(grid_key(placement.translate_long_axis).to_le_bytes());
    }
    format!("{:x}", digest.finalize())
}

#[cfg(feature = "jagua-experimental")]
fn coupled_independent_source_depth(
    pieces: &[GeneralFastPiece<'_>],
    placements: &[GeneralFastPlacement],
    settings: GeneralFastSettings,
) -> Result<f64, GeneralFastError> {
    let pieces_by_id = pieces
        .iter()
        .map(|piece| (piece.id, piece))
        .collect::<BTreeMap<_, _>>();
    let edge_clearance_mm = settings
        .sheet_edge_clearance_mm
        .unwrap_or(settings.total_padding_mm / 2.0);
    placements
        .iter()
        .map(|placement| {
            let piece = pieces_by_id
                .get(placement.piece_id.as_str())
                .ok_or_else(|| {
                    GeneralFastError::InvalidInput(format!(
                        "a coupled placement references unknown piece {}",
                        placement.piece_id
                    ))
                })?;
            let transformed = piece.polygon.transformed(
                placement.rotation_deg,
                placement.mirrored,
                placement.translate_short_axis,
                placement.translate_long_axis,
            )?;
            let bounds = transformed.bounds().ok_or_else(|| {
                GeneralFastError::InvalidInput(
                    "a coupled source polygon must be non-empty".to_owned(),
                )
            })?;
            Ok(bounds.max_y + edge_clearance_mm)
        })
        .collect::<Result<Vec<_>, GeneralFastError>>()?
        .into_iter()
        .max_by(f64::total_cmp)
        .ok_or_else(|| {
            GeneralFastError::InvalidInput(
                "coupled diagnostics must retain at least one placement".to_owned(),
            )
        })
}

/// The unsnapped counterpart of [`coupled_independent_source_depth`].
///
/// Same layout, same `max_y + edge clearance` definition, same edge clearance -
/// but measured on the untouched `f64` source rings rather than through
/// `PolygonSet::bounds`, which reads the canonical integer-grid path and rounds
/// to 0.001 mm. The two agree to within half a grid step; they can land on
/// opposite sides of a hard threshold, which is the reason both are reported.
#[cfg(feature = "jagua-experimental")]
pub(super) fn coupled_raw_source_depth(
    pieces: &[GeneralFastPiece<'_>],
    placements: &[GeneralFastPlacement],
    settings: GeneralFastSettings,
) -> Result<f64, GeneralFastError> {
    let pieces_by_id = pieces
        .iter()
        .map(|piece| (piece.id, piece))
        .collect::<BTreeMap<_, _>>();
    let edge_clearance_mm = settings
        .sheet_edge_clearance_mm
        .unwrap_or(settings.total_padding_mm / 2.0);
    let placements = placements
        .iter()
        .map(|placement| {
            let piece = pieces_by_id
                .get(placement.piece_id.as_str())
                .ok_or_else(|| {
                    GeneralFastError::InvalidInput(format!(
                        "a coupled placement references unknown piece {}",
                        placement.piece_id
                    ))
                })?;
            Ok(GeneralPlacement {
                piece_id: piece.id,
                polygon: piece.polygon,
                rotation_deg: placement.rotation_deg,
                mirrored: placement.mirrored,
                translate_x: placement.translate_short_axis,
                translate_y: placement.translate_long_axis,
            })
        })
        .collect::<Result<Vec<_>, GeneralFastError>>()?;
    raw_source_long_axis_depth_mm(&placements, edge_clearance_mm)
        .map_err(|error| GeneralFastError::InvalidInput(error.message().to_owned()))
}

/// The layout a persistent-vacancy arm's report is about: its published
/// placements when it published one, and otherwise the parent it was handed.
///
/// An arm that declined - the parent failed the composite check, the target was
/// unreachable - still reports `parent_fingerprint`, `target_depth_mm` and a
/// failure reason about that parent, so the parent is what the added
/// contract/raw-depth fields must describe for the report to be self-consistent.
#[cfg(feature = "jagua-experimental")]
fn persistent_vacancy_reported_layout(
    diagnostics: &GeneralPersistentVacancyDiagnostics,
    parent: &GeneralCoupledSeparatorArmDiagnostics,
) -> Vec<GeneralFastPlacement> {
    let subject = if diagnostics.final_placements.is_empty() {
        &parent.final_placements
    } else {
        &diagnostics.final_placements
    };
    fast_placements_from_coupled_diagnostics(subject)
}

/// Fills in the two reporting-only fields that separate contract validity from
/// search-envelope admissibility, and a depth that never rounds.
///
/// Computed once here, after the mode dispatch, rather than at each of the
/// dozen sites that set `exact_valid`: every mode reaches this point, so one
/// implementation covers all of them and no mode can forget to answer.
///
/// This changes no search behavior. It reads the layout the arm already
/// reported and adds two measurements of it.
#[cfg(feature = "jagua-experimental")]
fn record_persistent_vacancy_contract_report(
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    parent: &GeneralCoupledSeparatorArmDiagnostics,
) {
    let placements = persistent_vacancy_reported_layout(diagnostics, parent);
    if placements.len() != pieces.len() {
        return;
    }
    diagnostics.contract_valid =
        validate_placements_against_contract(pieces, &placements, fast_settings).is_ok();
    diagnostics.raw_source_depth_mm =
        coupled_raw_source_depth(pieces, &placements, fast_settings).ok();
}

fn run_independent_lanes<'a>(
    pieces: &'a [GeneralFastPiece<'a>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    initial_state: &RelaxedState,
    target_depth_mm: f64,
    epoch: usize,
    catalog: Arc<SurrogateCatalog>,
) -> Result<LaneBatch, GeneralFastError> {
    let lane_ordinals = (0..relaxed_settings.lanes).collect::<Vec<_>>();
    let lane_results = map_slice_with_job_pool(&lane_ordinals, |lane| {
        let seed = derive_seed(relaxed_settings.seed, epoch, *lane);
        let mut lane_state = initial_state.clone();
        let directional =
            relaxed_settings.pressure_model == GeneralRelaxedPressureModel::DirectionalPenetration;
        let disruption_count = if directional {
            0
        } else {
            lane_disruption_count(*lane)
        };
        if disruption_count > 0 {
            for disruption in 0..disruption_count {
                lane_state =
                    disrupt_state_legacy(lane_state, pieces, derive_seed(seed, disruption, *lane))?;
            }
        }
        let mut search = LegacyLaneSearch::new(
            pieces,
            fast_settings,
            relaxed_settings,
            seed,
            catalog.clone(),
        );
        let compressed = if directional {
            search
                .compress_directional_state(&lane_state, target_depth_mm)?
                .unwrap_or(lane_state)
        } else {
            compress_state_at_split(
                &lane_state,
                target_depth_mm,
                compression_split(seed, lane_state.strip_depth_mm, fast_settings),
                pieces,
            )?
        };
        search.run(compressed)
    });
    let mut outcomes = Vec::with_capacity(lane_results.len());
    let mut total = WorkCounters::default();
    let mut first_directional_rejection = None;
    for (lane_ordinal, lane_result) in lane_results.into_iter().enumerate() {
        let mut lane = match lane_result {
            Ok(lane) => lane,
            Err(error) if is_directional_lane_unscorable(&error) => {
                total.directional_lane_rejections =
                    total.directional_lane_rejections.saturating_add(1);
                if first_directional_rejection.is_none() {
                    first_directional_rejection = Some(error);
                }
                continue;
            }
            Err(error) => return Err(error),
        };
        total.accumulate(lane.counters);
        lane.selected_lane = lane_ordinal;
        lane.restart_disruptions = if relaxed_settings.pressure_model
            == GeneralRelaxedPressureModel::DirectionalPenetration
        {
            0
        } else {
            lane_disruption_count(lane_ordinal)
        };
        outcomes.push(lane);
    }
    if outcomes.is_empty() {
        if relaxed_settings.pressure_model == GeneralRelaxedPressureModel::DirectionalPenetration {
            return Err(first_directional_rejection
                .unwrap_or_else(|| directional_lane_unscorable_error("all lanes rejected")));
        }
        return Err(GeneralFastError::InvalidSettings(
            "relaxed search requires at least one lane".to_owned(),
        ));
    }
    Ok(LaneBatch {
        outcomes,
        counters: total,
    })
}

fn select_lane_for_publication(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    outcomes: Vec<LaneOutcome>,
    diagnostics: &mut GeneralRelaxedDiagnostics,
) -> SelectedLane {
    let mut outcomes = outcomes.into_iter().map(Some).collect::<Vec<_>>();
    let mut valid = Vec::<(usize, Vec<GeneralFastPlacement>, GeneralPlacementMetrics)>::new();
    for (index, outcome) in outcomes.iter().enumerate() {
        let outcome = outcome.as_ref().expect("lane outcomes are present");
        if !outcome.score.feasible() {
            continue;
        }
        diagnostics.surrogate_feasible_states =
            diagnostics.surrogate_feasible_states.saturating_add(1);
        let placements = to_fast_placements(&outcome.state, pieces);
        match validate_and_measure_placements(pieces, &placements, fast_settings) {
            Ok(metrics) => valid.push((index, placements, metrics)),
            Err(error) => {
                diagnostics.exact_rejected_states =
                    diagnostics.exact_rejected_states.saturating_add(1);
                diagnostics.exact_rejection_reasons.push(error.to_string());
            }
        }
    }
    if let Some((index, placements, metrics)) = valid.into_iter().min_by(
        |(first_index, _, first_metrics), (second_index, _, second_metrics)| {
            compare_exact_metrics(*first_metrics, *second_metrics).then_with(|| {
                let first = outcomes[*first_index]
                    .as_ref()
                    .expect("lane outcome is present");
                let second = outcomes[*second_index]
                    .as_ref()
                    .expect("lane outcome is present");
                canonical_state_key(&first.state)
                    .cmp(&canonical_state_key(&second.state))
                    .then_with(|| first.selected_lane.cmp(&second.selected_lane))
            })
        },
    ) {
        return SelectedLane {
            outcome: outcomes[index]
                .take()
                .expect("selected lane outcome is present"),
            validation: ExactLaneValidation::Accepted {
                placements,
                metrics,
            },
        };
    }
    let index = outcomes
        .iter()
        .enumerate()
        .filter_map(|(index, outcome)| outcome.as_ref().map(|outcome| (index, outcome)))
        .min_by(|(_, first), (_, second)| {
            compare_lane_outcomes(first.selected_lane, first, second.selected_lane, second)
        })
        .map(|(index, _)| index)
        .expect("relaxed search produces at least one lane");
    let outcome = outcomes[index]
        .take()
        .expect("selected lane outcome is present");
    let validation = if outcome.score.feasible() {
        ExactLaneValidation::Rejected
    } else {
        ExactLaneValidation::Infeasible
    };
    SelectedLane {
        outcome,
        validation,
    }
}

fn validate_selected_lane(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    lane: &LaneOutcome,
    diagnostics: &mut GeneralRelaxedDiagnostics,
) -> ExactLaneValidation {
    if !lane.score.feasible() {
        return ExactLaneValidation::Infeasible;
    }
    diagnostics.surrogate_feasible_states = diagnostics.surrogate_feasible_states.saturating_add(1);
    // The relaxed loop's proxy/exact boundary, matching the deep operators':
    // a lane state the surrogate tier called feasible, offered to the exact
    // validator.
    quality_trace::proxy_survivors(1);
    let placements = to_fast_placements(&lane.state, pieces);
    match validate_and_measure_placements(pieces, &placements, fast_settings) {
        Ok(metrics) => ExactLaneValidation::Accepted {
            placements,
            metrics,
        },
        Err(error) => {
            diagnostics.exact_rejected_states = diagnostics.exact_rejected_states.saturating_add(1);
            diagnostics.exact_rejection_reasons.push(error.to_string());
            ExactLaneValidation::Rejected
        }
    }
}

fn compare_exact_metrics(
    first: GeneralPlacementMetrics,
    second: GeneralPlacementMetrics,
) -> Ordering {
    first
        .used_long_axis_depth_mm
        .total_cmp(&second.used_long_axis_depth_mm)
        .then_with(|| {
            first
                .unused_short_axis_projection_mm
                .total_cmp(&second.unused_short_axis_projection_mm)
        })
        .then_with(|| {
            first
                .occupied_envelope_area_mm2
                .total_cmp(&second.occupied_envelope_area_mm2)
        })
}

fn lane_disruption_count(lane: usize) -> usize {
    if lane == 0 {
        0
    } else {
        1 + (lane - 1) % 3
    }
}

fn run_synchronized_lanes<'a>(
    pieces: &'a [GeneralFastPiece<'a>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    initial_state: &RelaxedState,
    target_depth_mm: f64,
    epoch: usize,
    catalog: Arc<SurrogateCatalog>,
) -> Result<LaneOutcome, GeneralFastError> {
    let lane_ordinals = (0..relaxed_settings.lanes).collect::<Vec<_>>();
    let workers = lane_ordinals
        .iter()
        .map(|lane| {
            Mutex::new(LegacyLaneSearch::new(
                pieces,
                fast_settings,
                relaxed_settings,
                derive_seed(relaxed_settings.seed, epoch, *lane),
                catalog.clone(),
            ))
        })
        .collect::<Vec<_>>();
    let mut master = None::<LaneOutcome>;
    let mut weights = BTreeMap::new();

    for sweep in 0..relaxed_settings.sweeps_per_epoch {
        let lane_results = map_slice_with_job_pool(&lane_ordinals, |lane| {
            let seed = derive_seed(relaxed_settings.seed, epoch, *lane);
            let state = if let Some(master) = &master {
                master.state.clone()
            } else {
                compress_state_at_split(
                    initial_state,
                    target_depth_mm,
                    compression_split(seed, initial_state.strip_depth_mm, fast_settings),
                    pieces,
                )?
            };
            let mut worker = workers[*lane].lock().map_err(|_| {
                GeneralFastError::InvalidInput("relaxed lane worker lock was poisoned".to_owned())
            })?;
            worker.weights = weights.clone();
            worker.run_sweep(state, sweep)
        });
        let mut best = None::<(usize, LaneOutcome)>;
        for (lane_ordinal, lane_result) in lane_results.into_iter().enumerate() {
            let lane = lane_result?;
            if best.as_ref().is_none_or(|(best_ordinal, best_lane)| {
                compare_lane_outcomes(lane_ordinal, &lane, *best_ordinal, best_lane)
                    == Ordering::Less
            }) {
                best = Some((lane_ordinal, lane));
            }
        }
        let Some((lane_ordinal, mut selected)) = best else {
            return Err(GeneralFastError::InvalidSettings(
                "relaxed search requires at least one lane".to_owned(),
            ));
        };
        selected.selected_lane = lane_ordinal;
        selected.restart_disruptions = 0;
        update_weights(&mut weights, &selected.score.collision_pairs);
        let feasible = selected.score.feasible();
        master = Some(selected);
        if feasible {
            break;
        }
    }

    let mut outcome = master.ok_or_else(|| {
        GeneralFastError::InvalidSettings("relaxed search requires at least one sweep".to_owned())
    })?;
    outcome.weights = weights;
    outcome.counters = workers.iter().try_fold(
        WorkCounters::default(),
        |mut total, worker| -> Result<_, GeneralFastError> {
            let worker = worker.lock().map_err(|_| {
                GeneralFastError::InvalidInput("relaxed lane worker lock was poisoned".to_owned())
            })?;
            total.ejection_chain_evaluations = total
                .ejection_chain_evaluations
                .saturating_add(worker.counters.ejection_chain_evaluations);
            total.ejection_chain_accepts = total
                .ejection_chain_accepts
                .saturating_add(worker.counters.ejection_chain_accepts);
            total.surrogate_evaluations = total
                .surrogate_evaluations
                .saturating_add(worker.counters.surrogate_evaluations);
            total.piece_broad_phase_probes = total
                .piece_broad_phase_probes
                .saturating_add(worker.counters.piece_broad_phase_probes);
            total.cell_index_probes = total
                .cell_index_probes
                .saturating_add(worker.counters.cell_index_probes);
            total.sat_tests = total.sat_tests.saturating_add(worker.counters.sat_tests);
            total.pair_nfp_builds = total
                .pair_nfp_builds
                .saturating_add(worker.counters.pair_nfp_builds);
            total.pair_nfp_components = total
                .pair_nfp_components
                .saturating_add(worker.counters.pair_nfp_components);
            total.shared_pair_nfp_adoptions = total
                .shared_pair_nfp_adoptions
                .saturating_add(worker.counters.shared_pair_nfp_adoptions);
            total.axis_events = total
                .axis_events
                .saturating_add(worker.counters.axis_events);
            total.axis_candidate_evaluations = total
                .axis_candidate_evaluations
                .saturating_add(worker.counters.axis_candidate_evaluations);
            total.dynamic_hazard_queries = total
                .dynamic_hazard_queries
                .saturating_add(worker.counters.dynamic_hazard_queries);
            total.dynamic_hazard_updates = total
                .dynamic_hazard_updates
                .saturating_add(worker.counters.dynamic_hazard_updates);
            total.dynamic_pressure_evaluations = total
                .dynamic_pressure_evaluations
                .saturating_add(worker.counters.dynamic_pressure_evaluations);
            total.translation_evaluations = total
                .translation_evaluations
                .saturating_add(worker.counters.translation_evaluations);
            total.rotation_evaluations = total
                .rotation_evaluations
                .saturating_add(worker.counters.rotation_evaluations);
            total.retained_f64_confirmations = total
                .retained_f64_confirmations
                .saturating_add(worker.counters.retained_f64_confirmations);
            total.confirmed_pair_additions = total
                .confirmed_pair_additions
                .saturating_add(worker.counters.confirmed_pair_additions);
            total.confirmed_pair_removals = total
                .confirmed_pair_removals
                .saturating_add(worker.counters.confirmed_pair_removals);
            total.accepted_moves = total
                .accepted_moves
                .saturating_add(worker.counters.accepted_moves);
            Ok(total)
        },
    )?;
    Ok(outcome)
}

impl<'a, K: ExplorationKernel<Shape = OrientedSurrogate> + Default> LaneSearch<'a, K> {
    fn new(
        pieces: &'a [GeneralFastPiece<'a>],
        fast_settings: GeneralFastSettings,
        relaxed_settings: GeneralRelaxedSettings,
        seed: u64,
        catalog: Arc<SurrogateCatalog>,
    ) -> Self {
        Self {
            pieces,
            fast_settings,
            relaxed_settings,
            catalog,
            kernel: K::default(),
            rng: SplitMix64::new(seed),
            weights: BTreeMap::new(),
            counters: WorkCounters::default(),
            allow_worsening_chain: false,
            piece_query_scratch: PieceQueryScratch::new(pieces.len()),
            proxy_rows: ProxyRowCache::new(pieces.len()),
            angle_keys: AngleKeyCache::new(pieces.len()),
            collision_merge_scratch: Vec::new(),
            #[cfg(feature = "relaxed-row-buffer-reuse")]
            row_pool: Vec::new(),
            scan_order_scratch: Vec::new(),
            #[cfg(feature = "shadow-rescore")]
            audit_move_was_revert: false,
            #[cfg(feature = "shadow-rescore")]
            audit_last_confirmed_row: None,
            pair_nfp_cache: BTreeMap::new(),
            pair_nfp_cache_components: 0,
            #[cfg(feature = "jagua-experimental")]
            hazard_index: None,
            #[cfg(feature = "jagua-experimental")]
            hazard_catalog: None,
            dynamic_query_limit: None,
            refine_rotation: false,
            #[cfg(feature = "compression-schedule")]
            compression: None,
        }
    }

    fn uses_dynamic_hazard(&self) -> bool {
        self.relaxed_settings.collision_backend == GeneralRelaxedCollisionBackend::DynamicHazard
    }

    fn uses_dynamic_pressure(&self) -> bool {
        self.uses_dynamic_hazard()
            && self.relaxed_settings.pressure_model == GeneralRelaxedPressureModel::DynamicPoles
    }

    fn uses_continuous_triangle_pressure(&self) -> bool {
        self.uses_dynamic_hazard()
            && self.relaxed_settings.pressure_model
                == GeneralRelaxedPressureModel::ContinuousTrianglePoles
    }

    fn uses_directional_pressure(&self) -> bool {
        self.relaxed_settings.collision_backend == GeneralRelaxedCollisionBackend::RollbackTriangle
            && self.relaxed_settings.pressure_model
                == GeneralRelaxedPressureModel::DirectionalPenetration
    }

    fn directional_inner_fit(
        &self,
        placement: &RelaxedPlacement,
        strip_depth_mm: f64,
    ) -> Result<Option<GridInnerFit>, GeneralFastError> {
        let shape = self.oriented(
            placement.input_index,
            placement.rotation_deg,
            placement.mirrored,
        )?;
        let coordinate = |value: f64, label: &str| {
            grid_coordinate(value).ok_or_else(|| {
                GeneralFastError::InvalidInput(format!(
                    "directional {label} is outside the canonical grid"
                ))
            })
        };
        let inset = coordinate(collision_sheet_inset_mm(self.fast_settings), "sheet inset")?;
        let sheet_short = coordinate(self.fast_settings.sheet_short_axis_mm, "sheet short axis")?;
        let strip_depth = coordinate(strip_depth_mm, "strip depth")?;
        let local_min_x = coordinate(shape.bounds.min_x, "local minimum x")?;
        let local_max_x = coordinate(shape.bounds.max_x, "local maximum x")?;
        let local_min_y = coordinate(shape.bounds.min_y, "local minimum y")?;
        let local_max_y = coordinate(shape.bounds.max_y, "local maximum y")?;
        let min_x = inset.checked_sub(local_min_x);
        let max_x = sheet_short
            .checked_sub(inset)
            .and_then(|value| value.checked_sub(local_max_x));
        let min_y = inset.checked_sub(local_min_y);
        let max_y = strip_depth
            .checked_sub(inset)
            .and_then(|value| value.checked_sub(local_max_y));
        let (Some(min_x), Some(max_x), Some(min_y), Some(max_y)) = (min_x, max_x, min_y, max_y)
        else {
            return Err(GeneralFastError::InvalidInput(
                "directional inner-fit arithmetic overflowed".to_owned(),
            ));
        };
        Ok((min_x <= max_x && min_y <= max_y).then_some(GridInnerFit {
            min_x,
            max_x,
            min_y,
            max_y,
        }))
    }

    fn directional_position(
        &self,
        placement: &RelaxedPlacement,
    ) -> Result<(i128, i128), GeneralFastError> {
        let x = grid_coordinate(placement.translate_x).ok_or_else(|| {
            GeneralFastError::InvalidInput(
                "directional horizontal placement is outside the canonical grid".to_owned(),
            )
        })?;
        let y = grid_coordinate(placement.translate_y).ok_or_else(|| {
            GeneralFastError::InvalidInput(
                "directional vertical placement is outside the canonical grid".to_owned(),
            )
        })?;
        Ok((x, y))
    }

    fn directional_contains(
        &self,
        placement: &RelaxedPlacement,
        strip_depth_mm: f64,
    ) -> Result<bool, GeneralFastError> {
        let Some(inner_fit) = self.directional_inner_fit(placement, strip_depth_mm)? else {
            return Ok(false);
        };
        let (x, y) = self.directional_position(placement)?;
        Ok(inner_fit.contains(x, y))
    }

    fn directional_relative_point(
        &self,
        fixed: &RelaxedPlacement,
        moving: &RelaxedPlacement,
    ) -> Result<IrregularPoint, GeneralFastError> {
        let relative_x = relative_grid_coordinate(fixed.translate_x, moving.translate_x)
            .ok_or_else(|| {
                GeneralFastError::InvalidInput(
                    "directional horizontal translation difference is outside the canonical grid"
                        .to_owned(),
                )
            })?;
        let relative_y = relative_grid_coordinate(fixed.translate_y, moving.translate_y)
            .ok_or_else(|| {
                GeneralFastError::InvalidInput(
                    "directional vertical translation difference is outside the canonical grid"
                        .to_owned(),
                )
            })?;
        Ok(IrregularPoint::new(
            from_grid(relative_x as f64),
            from_grid(relative_y as f64),
        ))
    }

    fn compress_directional_state(
        &mut self,
        state: &RelaxedState,
        target_depth_mm: f64,
    ) -> Result<Option<RelaxedState>, GeneralFastError> {
        let mut order = (0..state.placements.len()).collect::<Vec<_>>();
        order.sort_by(|first, second| {
            self.pieces[state.placements[*first].input_index]
                .id
                .cmp(self.pieces[state.placements[*second].input_index].id)
        });
        let mut replacements = Vec::new();
        for input_index in order {
            let placement = &state.placements[input_index];
            let Some(inner_fit) = self.directional_inner_fit(placement, target_depth_mm)? else {
                self.counters.directional_rejected_contractions = self
                    .counters
                    .directional_rejected_contractions
                    .saturating_add(1);
                return Ok(None);
            };
            let (x, y) = self.directional_position(placement)?;
            if inner_fit.contains(x, y) {
                continue;
            }
            let x = x.clamp(inner_fit.min_x, inner_fit.max_x);
            let y = y.clamp(inner_fit.min_y, inner_fit.max_y);
            replacements.push((input_index, x, y));
        }
        let mut compressed = state.clone();
        compressed.strip_depth_mm = target_depth_mm;
        for (input_index, x, y) in replacements.iter().copied() {
            compressed.placements[input_index].translate_x = from_grid(x as f64);
            compressed.placements[input_index].translate_y = from_grid(y as f64);
        }
        for placement in &compressed.placements {
            if !self.directional_contains(placement, target_depth_mm)? {
                self.counters.directional_containment_rejections = self
                    .counters
                    .directional_containment_rejections
                    .saturating_add(1);
                return Ok(None);
            }
        }
        self.counters.directional_relocations = self
            .counters
            .directional_relocations
            .saturating_add(replacements.len());
        Ok(Some(compressed))
    }

    fn seed_angle(&self, angle_deg: f64) -> f64 {
        match self.relaxed_settings.angle_seed_policy {
            GeneralRelaxedAngleSeedPolicy::CurrentOnly => continuous_angle(angle_deg),
            GeneralRelaxedAngleSeedPolicy::StructuredGrid => canonical_angle(angle_deg),
            GeneralRelaxedAngleSeedPolicy::ContinuousUniform => continuous_angle(angle_deg),
        }
    }

    fn dynamic_query_budget_exhausted(&self) -> bool {
        self.dynamic_query_limit
            .is_some_and(|limit| self.counters.dynamic_hazard_queries >= limit)
    }

    fn count_seed_evaluation(&mut self, current: &RelaxedPlacement, candidate: &RelaxedPlacement) {
        if angle_key(current.rotation_deg) != angle_key(candidate.rotation_deg)
            || current.mirrored != candidate.mirrored
        {
            self.counters.rotation_evaluations =
                self.counters.rotation_evaluations.saturating_add(1);
        } else {
            self.counters.translation_evaluations =
                self.counters.translation_evaluations.saturating_add(1);
        }
    }

    fn prepare_dynamic_hazard(&mut self, state: &RelaxedState) -> Result<(), GeneralFastError> {
        if !self.uses_dynamic_hazard() {
            return Ok(());
        }
        #[cfg(feature = "jagua-experimental")]
        {
            let poses = state.placements.iter().map(hazard_pose).collect::<Vec<_>>();
            self.counters.dynamic_layout_loads =
                self.counters.dynamic_layout_loads.saturating_add(1);
            if let Some(index) = self.hazard_index.as_mut() {
                index
                    .rebuild(state.strip_depth_mm, &poses)
                    .map_err(dynamic_hazard_error)?;
            } else {
                self.hazard_index = Some(if let Some(catalog) = &self.hazard_catalog {
                    JaguaHazardIndex::from_catalog(
                        self.pieces,
                        self.fast_settings,
                        state.strip_depth_mm,
                        &poses,
                        catalog,
                    )
                    .map_err(dynamic_hazard_error)?
                } else {
                    JaguaHazardIndex::new(
                        self.pieces,
                        self.fast_settings,
                        state.strip_depth_mm,
                        &poses,
                    )
                    .map_err(dynamic_hazard_error)?
                });
                self.counters.dynamic_index_builds =
                    self.counters.dynamic_index_builds.saturating_add(1);
            }
            return Ok(());
        }
        #[cfg(not(feature = "jagua-experimental"))]
        {
            let _ = state;
            Err(GeneralFastError::InvalidSettings(
                "dynamic hazard search requires the jagua-experimental feature".to_owned(),
            ))
        }
    }

    fn local_shape_bounds(
        &mut self,
        input_index: usize,
        rotation_deg: f64,
        mirrored: bool,
    ) -> Result<IrregularBounds, GeneralFastError> {
        if self.uses_dynamic_hazard() {
            #[cfg(feature = "jagua-experimental")]
            {
                return self
                    .hazard_index
                    .as_mut()
                    .expect("dynamic hazard index is prepared before search")
                    .pose_bounds(
                        input_index,
                        GeneralHazardPose {
                            rotation_deg: continuous_angle(rotation_deg),
                            mirrored,
                            translate_short_axis: 0.0,
                            translate_long_axis: 0.0,
                        },
                    )
                    .map_err(dynamic_hazard_error);
            }
            #[cfg(not(feature = "jagua-experimental"))]
            {
                return Err(GeneralFastError::InvalidSettings(
                    "dynamic hazard search requires the jagua-experimental feature".to_owned(),
                ));
            }
        }
        // `oriented` derives the rotation half of the key through
        // `derive_rotation_key`, which runs `rem_euclid` and a rounding step on
        // every call. `relaxed-cached-pose-bounds` asks [`AngleKeyCache`] for
        // the same `i64` instead, which is the memo the candidate scan has used
        // all along; on a hit it is a bits comparison, and on a miss it derives
        // exactly what `oriented` would have and stores it. The key, the
        // catalogue entry, the bounds and the missing-orientation error are the
        // same in both arms - only how many times `canonical_angle` runs
        // differs. This is the lane's most-called catalogue path: 4.45M calls
        // on the mode-20 gate-1 stream and 11.64M on mode-22 gate-2, nearly all
        // of them from `boundary_penalty`.
        #[cfg(not(feature = "relaxed-cached-pose-bounds"))]
        {
            Ok(self.oriented(input_index, rotation_deg, mirrored)?.bounds)
        }
        #[cfg(feature = "relaxed-cached-pose-bounds")]
        {
            let directional = self.uses_directional_pressure();
            let key = (
                self.catalog.geometry_class_by_input[input_index],
                self.angle_keys
                    .rotation_key(input_index, rotation_deg, directional),
                mirrored,
            );
            let bounds = self
                .catalog
                .orientations
                .get(&key)
                .map(|shape| shape.bounds);
            bounds.ok_or_else(|| self.missing_orientation(input_index, key))
        }
    }

    fn commit_dynamic_hazard(
        &mut self,
        placement: &RelaxedPlacement,
    ) -> Result<(), GeneralFastError> {
        if !self.uses_dynamic_hazard() {
            return Ok(());
        }
        #[cfg(feature = "jagua-experimental")]
        {
            self.hazard_index
                .as_mut()
                .expect("dynamic hazard index is prepared before search")
                .commit(placement.input_index, hazard_pose(placement))
                .map_err(dynamic_hazard_error)?;
            self.counters.dynamic_hazard_updates =
                self.counters.dynamic_hazard_updates.saturating_add(1);
            return Ok(());
        }
        #[cfg(not(feature = "jagua-experimental"))]
        {
            let _ = placement;
            Err(GeneralFastError::InvalidSettings(
                "dynamic hazard search requires the jagua-experimental feature".to_owned(),
            ))
        }
    }

    fn run(&mut self, mut state: RelaxedState) -> Result<LaneOutcome, GeneralFastError> {
        self.prepare_dynamic_hazard(&state)?;
        if self.uses_directional_pressure() {
            self.preflight_directional_assignment(&state)?;
        }
        let mut score = self.score_state(&state)?;
        for sweep in 0..self.relaxed_settings.sweeps_per_epoch {
            if score.feasible() {
                break;
            }
            self.move_sweep(&mut state, &mut score, sweep)?;
            if ENABLE_EJECTION_CHAIN
                && !score.feasible()
                && sweep == self.relaxed_settings.sweeps_per_epoch / 2
            {
                self.try_ejection_chain(&mut state, &mut score)?;
            }
            if !score.feasible() && sweep + 1 < self.relaxed_settings.sweeps_per_epoch {
                update_weights(&mut self.weights, &score.collision_pairs);
                refresh_weighted_loss(&mut score, &self.weights);
            }
        }
        Ok(LaneOutcome {
            state,
            score,
            weights: self.weights.clone(),
            counters: self.counters,
            selected_lane: 0,
            restart_disruptions: 0,
        })
    }

    fn preflight_directional_assignment(
        &mut self,
        state: &RelaxedState,
    ) -> Result<(), GeneralFastError> {
        let mut keys = Vec::with_capacity(
            state
                .placements
                .len()
                .saturating_mul(state.placements.len().saturating_sub(1)),
        );
        for fixed_index in 0..state.placements.len() {
            for moving_index in 0..state.placements.len() {
                if fixed_index == moving_index {
                    continue;
                }
                keys.push(self.pair_nfp_key(
                    &state.placements[fixed_index],
                    &state.placements[moving_index],
                )?);
            }
        }
        if !self.preflight_directional_pair_nfps(&keys, false)? {
            return Err(directional_lane_unscorable_error(
                "fixed-orientation translation cache budget",
            ));
        }
        Ok(())
    }

    fn run_repair_arm(
        &mut self,
        mut state: RelaxedState,
        allow_rotation: bool,
        neighborhood_size: usize,
        query_budget: usize,
        confirmation_budget: usize,
    ) -> Result<LaneOutcome, GeneralFastError> {
        self.prepare_dynamic_hazard(&state)?;
        let initial_score = self.score_state_dynamic(&state)?;
        let active = repair_active_indices(
            &state,
            &initial_score,
            self.pieces,
            &self.weights,
            neighborhood_size,
        );
        let reinsertion_budget = query_budget / 2;
        for (ordinal, input_index) in active.iter().copied().enumerate() {
            if self.counters.dynamic_hazard_queries >= reinsertion_budget
                || self.counters.retained_f64_confirmations >= confirmation_budget
            {
                break;
            }
            let ignored = active[(ordinal + 1)..]
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            let pieces_left = active.len().saturating_sub(ordinal).max(1);
            let piece_budget = reinsertion_budget
                .saturating_sub(self.counters.dynamic_hazard_queries)
                / pieces_left;
            let angles = if allow_rotation {
                repair_angles(self.pieces[input_index], &state.placements[input_index])
            } else {
                vec![state.placements[input_index].rotation_deg]
            };
            let replacement = self.search_repair_piece(
                &state,
                input_index,
                &angles,
                &ignored,
                piece_budget,
                true,
            )?;
            self.commit_dynamic_hazard(&replacement)?;
            if move_tie_key(&replacement) != move_tie_key(&state.placements[input_index]) {
                self.counters.accepted_moves = self.counters.accepted_moves.saturating_add(1);
            }
            state.placements[input_index] = replacement;
        }

        self.relaxed_settings.angle_seed_policy = GeneralRelaxedAngleSeedPolicy::CurrentOnly;
        self.relaxed_settings.global_samples_per_move = 2;
        self.relaxed_settings.focused_samples_per_move = 2;
        self.relaxed_settings.refinement_rounds = 1;
        self.dynamic_query_limit = Some(query_budget);
        let mut score = self.score_state_dynamic(&state)?;
        for sweep in 0..4 {
            if score.feasible() || self.dynamic_query_budget_exhausted() {
                break;
            }
            self.move_sweep(&mut state, &mut score, sweep)?;
            if !score.feasible() && !self.dynamic_query_budget_exhausted() {
                update_weights(&mut self.weights, &score.collision_pairs);
                refresh_weighted_loss(&mut score, &self.weights);
            }
        }
        self.dynamic_query_limit = None;
        score = self.score_state_dynamic(&state)?;
        Ok(LaneOutcome {
            state,
            score,
            weights: self.weights.clone(),
            counters: self.counters,
            selected_lane: 0,
            restart_disruptions: 0,
        })
    }

    fn search_repair_piece(
        &mut self,
        state: &RelaxedState,
        input_index: usize,
        angles: &[f64],
        ignored: &BTreeSet<usize>,
        query_budget: usize,
        allow_worsening: bool,
    ) -> Result<RelaxedPlacement, GeneralFastError> {
        let query_limit = self
            .counters
            .dynamic_hazard_queries
            .saturating_add(query_budget);
        let current = state.placements[input_index].clone();
        let current_score =
            self.confirm_repair_candidate(state, input_index, &current, ignored, false)?;
        let mut best = current.clone();
        let mut best_score = self.score_repair_candidate(state, input_index, &best, ignored)?;
        let contact_limit = self
            .counters
            .dynamic_hazard_queries
            .saturating_add(query_budget.saturating_mul(3) / 4);
        let mut contact_sets = angles
            .iter()
            .copied()
            .map(|angle| {
                self.repair_contact_candidates(
                    state,
                    input_index,
                    continuous_angle(angle),
                    current.mirrored,
                    ignored,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut contact_index = 0usize;
        while self.counters.dynamic_hazard_queries < contact_limit
            && contact_sets.iter().any(|candidates| !candidates.is_empty())
        {
            let set_index = contact_index % contact_sets.len();
            if contact_sets[set_index].is_empty() {
                contact_index = contact_index.saturating_add(1);
                continue;
            }
            let candidate = contact_sets[set_index].remove(0);
            let score = self.score_repair_candidate(state, input_index, &candidate, ignored)?;
            if angle_key(candidate.rotation_deg) != angle_key(current.rotation_deg) {
                self.counters.rotation_evaluations =
                    self.counters.rotation_evaluations.saturating_add(1);
            } else {
                self.counters.translation_evaluations =
                    self.counters.translation_evaluations.saturating_add(1);
            }
            if compare_move_score(&score, &candidate, &best_score, &best) == Ordering::Less {
                best = candidate;
                best_score = score;
            }
            contact_index = contact_index.saturating_add(1);
        }

        let best_bounds = self.local_shape_bounds(input_index, best.rotation_deg, best.mirrored)?;
        let mut step_x = ((best_bounds.max_x - best_bounds.min_x) * 0.25).max(0.001);
        let mut step_y = ((best_bounds.max_y - best_bounds.min_y) * 0.25).max(0.001);
        let mut horizontal = true;
        while self.counters.dynamic_hazard_queries.saturating_add(2) <= query_limit
            && (step_x >= 0.001 || step_y >= 0.001)
        {
            let offsets = if horizontal {
                [(step_x, 0.0), (-step_x, 0.0)]
            } else {
                [(0.0, step_y), (0.0, -step_y)]
            };
            let mut improved = false;
            for (offset_x, offset_y) in offsets {
                let candidate = RelaxedPlacement {
                    translate_x: snap_mm(best.translate_x + offset_x),
                    translate_y: snap_mm(best.translate_y + offset_y),
                    ..best.clone()
                };
                let score = self.score_repair_candidate(state, input_index, &candidate, ignored)?;
                self.counters.translation_evaluations =
                    self.counters.translation_evaluations.saturating_add(1);
                if compare_move_score(&score, &candidate, &best_score, &best) == Ordering::Less {
                    best = candidate;
                    best_score = score;
                    improved = true;
                }
            }
            if !improved {
                if horizontal {
                    step_x *= 0.5;
                } else {
                    step_y *= 0.5;
                }
            }
            horizontal = !horizontal;
        }
        let confirmed = self.confirm_repair_candidate(state, input_index, &best, ignored, true)?;
        if !allow_worsening
            && compare_score_objective(&confirmed, &current_score) == Ordering::Greater
        {
            Ok(current)
        } else {
            Ok(best)
        }
    }

    fn repair_contact_candidates(
        &mut self,
        state: &RelaxedState,
        input_index: usize,
        angle: f64,
        mirrored: bool,
        ignored: &BTreeSet<usize>,
    ) -> Result<Vec<RelaxedPlacement>, GeneralFastError> {
        let local = self.local_shape_bounds(input_index, angle, mirrored)?;
        let inset = collision_sheet_inset_mm(self.fast_settings);
        let min_x = inset - local.min_x;
        let max_x = self.fast_settings.sheet_short_axis_mm - inset - local.max_x;
        let min_y = inset - local.min_y;
        let max_y = state.strip_depth_mm - inset - local.max_y;
        if min_x > max_x || min_y > max_y {
            return Ok(Vec::new());
        }
        let current = &state.placements[input_index];
        let mut positions = vec![
            (
                current.translate_x.clamp(min_x, max_x),
                current.translate_y.clamp(min_y, max_y),
            ),
            (min_x, min_y),
            (max_x, min_y),
        ];
        for (fixed_index, fixed) in state.placements.iter().enumerate() {
            if fixed_index == input_index || ignored.contains(&fixed_index) {
                continue;
            }
            let bounds = self.placement_bounds(fixed)?;
            let left = bounds.min_x - local.max_x;
            let right = bounds.max_x - local.min_x;
            let below = bounds.min_y - local.max_y;
            let above = bounds.max_y - local.min_y;
            let align_left = bounds.min_x - local.min_x;
            let align_right = bounds.max_x - local.max_x;
            let align_bottom = bounds.min_y - local.min_y;
            let align_top = bounds.max_y - local.max_y;
            positions.extend([
                (left, align_bottom),
                (left, align_top),
                (right, align_bottom),
                (right, align_top),
                (align_left, below),
                (align_right, below),
                (align_left, above),
                (align_right, above),
            ]);
        }
        let mut unique = BTreeMap::new();
        for (x, y) in positions {
            if x < min_x || x > max_x || y < min_y || y > max_y {
                continue;
            }
            let x = snap_mm(x);
            let y = snap_mm(y);
            unique
                .entry((grid_key(y), grid_key(x)))
                .or_insert(RelaxedPlacement {
                    input_index,
                    rotation_deg: angle,
                    mirrored,
                    translate_x: x,
                    translate_y: y,
                });
        }
        Ok(unique.into_values().collect())
    }

    fn score_repair_candidate(
        &mut self,
        state: &RelaxedState,
        input_index: usize,
        candidate: &RelaxedPlacement,
        ignored: &BTreeSet<usize>,
    ) -> Result<MovedRowDelta, GeneralFastError> {
        #[cfg(feature = "jagua-experimental")]
        {
            let (boundary_violations, boundary_loss) =
                self.boundary_penalty(candidate, state.strip_depth_mm)?;
            if self.dynamic_query_budget_exhausted() {
                return Ok(MovedRowDelta {
                    boundary_violations,
                    boundary_loss,
                    collision_pairs: Vec::new(),
                    weighted_loss: f64::INFINITY,
                    rows: MovedRows::Unscanned,
                });
            }
            let query = self
                .hazard_index
                .as_mut()
                .expect("repair prepares the dynamic hazard index")
                .query(input_index, hazard_pose(candidate), None)
                .map_err(dynamic_hazard_error)?;
            self.counters.dynamic_hazard_queries =
                self.counters.dynamic_hazard_queries.saturating_add(1);
            let GeneralHazardQuery::Complete {
                colliding_piece_ids,
                ..
            } = query
            else {
                return Err(GeneralFastError::InvalidInput(
                    "repair requires complete hazard rows".to_owned(),
                ));
            };
            let mut collision_pairs = Vec::new();
            let mut weighted_loss = boundary_loss;
            for fixed_index in colliding_piece_ids {
                if ignored.contains(&fixed_index) {
                    continue;
                }
                let penalty =
                    self.rollback_pair_pressure(candidate, &state.placements[fixed_index])?;
                let pair = ordered_pair(input_index, fixed_index);
                collision_pairs.push((pair.0, pair.1, penalty));
                weighted_loss += self.pair_weight(pair.0, pair.1) * penalty;
                self.counters.dynamic_pressure_evaluations =
                    self.counters.dynamic_pressure_evaluations.saturating_add(1);
            }
            collision_pairs.sort_by_key(|(first, second, _)| (*first, *second));
            Ok(MovedRowDelta {
                boundary_violations,
                boundary_loss,
                collision_pairs,
                weighted_loss,
                rows: MovedRows::Complete,
            })
        }
        #[cfg(not(feature = "jagua-experimental"))]
        {
            let _ = (state, input_index, candidate, ignored);
            Err(GeneralFastError::InvalidSettings(
                "angular repair requires the jagua-experimental feature".to_owned(),
            ))
        }
    }

    fn confirm_repair_candidate(
        &mut self,
        state: &RelaxedState,
        input_index: usize,
        candidate: &RelaxedPlacement,
        ignored: &BTreeSet<usize>,
        retained: bool,
    ) -> Result<MovedRowDelta, GeneralFastError> {
        let (boundary_violations, boundary_loss) =
            self.boundary_penalty(candidate, state.strip_depth_mm)?;
        let mut collision_pairs = Vec::new();
        let mut weighted_loss = boundary_loss;
        for fixed_index in 0..state.placements.len() {
            if fixed_index == input_index || ignored.contains(&fixed_index) {
                continue;
            }
            let penalty =
                self.confirmed_pair_pressure(candidate, &state.placements[fixed_index])?;
            if penalty == 0.0 {
                continue;
            }
            let pair = ordered_pair(input_index, fixed_index);
            collision_pairs.push((pair.0, pair.1, penalty));
            weighted_loss += self.pair_weight(pair.0, pair.1) * penalty;
        }
        collision_pairs.sort_by_key(|(first, second, _)| (*first, *second));
        if retained {
            self.counters.retained_f64_confirmations =
                self.counters.retained_f64_confirmations.saturating_add(1);
        }
        Ok(MovedRowDelta {
            boundary_violations,
            boundary_loss,
            collision_pairs,
            weighted_loss,
            rows: MovedRows::Complete,
        })
    }

    fn run_sweep(
        &mut self,
        mut state: RelaxedState,
        sweep: usize,
    ) -> Result<LaneOutcome, GeneralFastError> {
        self.prepare_dynamic_hazard(&state)?;
        let mut score = self.score_state(&state)?;
        self.move_sweep(&mut state, &mut score, sweep)?;
        if ENABLE_EJECTION_CHAIN
            && !score.feasible()
            && sweep == self.relaxed_settings.sweeps_per_epoch / 2
        {
            self.try_ejection_chain(&mut state, &mut score)?;
        }
        Ok(LaneOutcome {
            state,
            score,
            weights: self.weights.clone(),
            counters: self.counters,
            selected_lane: 0,
            restart_disruptions: 0,
        })
    }

    fn move_sweep(
        &mut self,
        state: &mut RelaxedState,
        score: &mut PairTracker,
        sweep: usize,
    ) -> Result<(), GeneralFastError> {
        let _span = profiling::span(Phase::MoveSweep);
        self.apply_compression_schedule(state, score)?;
        if !score.feasible() {
            let mut forced = BTreeSet::new();
            let mut active = score
                .collision_pairs
                .iter()
                .flat_map(|(first, second, _)| [*first, *second])
                .collect::<BTreeSet<_>>();
            if !self.uses_directional_pressure() {
                for (index, placement) in state.placements.iter().enumerate() {
                    if self.boundary_penalty(placement, state.strip_depth_mm)?.0 > 0 {
                        active.insert(index);
                    }
                }
            }
            if active.is_empty() {
                forced.extend(legacy_forced_blockers(state, self.pieces, 4));
                active.extend(forced.iter().copied());
            }
            let mut order = active.into_iter().collect::<Vec<_>>();
            shuffle(&mut order, &mut self.rng);
            if sweep > 0 && sweep % 4 == 0 {
                for blocker in legacy_forced_blockers(&state, self.pieces, 2) {
                    if !order.contains(&blocker) {
                        order.push(blocker);
                    }
                    forced.insert(blocker);
                }
            }
            let mut piece_index = self.build_piece_index(state)?;
            for input_index in order {
                if self.dynamic_query_budget_exhausted() {
                    break;
                }
                if !forced.contains(&input_index)
                    && !self.piece_is_active(state, score, input_index)?
                {
                    continue;
                }
                let current = state.placements[input_index].clone();
                let old_boundary = self.boundary_penalty(&current, state.strip_depth_mm)?;
                let (mut replacement, replacement_score) =
                    self.search_piece(state, score, input_index, &piece_index)?;
                let mut replacement_score = if self.uses_dynamic_hazard() {
                    self.confirm_dynamic_replacement(
                        state,
                        input_index,
                        &replacement,
                        &replacement_score,
                    )?
                } else {
                    replacement_score
                };
                #[cfg(feature = "shadow-rescore")]
                {
                    self.audit_move_was_revert = false;
                }
                if self.uses_dynamic_hazard() {
                    let current_score = tracked_piece_score(score, input_index, &self.weights);
                    if compare_score_objective(&replacement_score, &current_score)
                        == Ordering::Greater
                    {
                        #[cfg(feature = "shadow-rescore")]
                        {
                            self.audit_move_was_revert = true;
                        }
                        replacement = current.clone();
                        replacement_score = current_score;
                    }
                }
                if move_tie_key(&replacement) != move_tie_key(&current) {
                    self.counters.accepted_moves = self.counters.accepted_moves.saturating_add(1);
                    if self.uses_directional_pressure() {
                        for (_, _, penalty) in &replacement_score.collision_pairs {
                            self.counters
                                .directional_accepted_pair_loss
                                .observe(*penalty);
                        }
                        self.counters
                            .directional_accepted_boundary_loss
                            .observe(replacement_score.boundary_loss);
                    }
                }
                let old_bounds = self.placement_bounds(&current)?;
                let replacement_bounds = self.placement_bounds(&replacement)?;
                piece_index.remove(input_index, old_bounds);
                piece_index.insert(input_index, replacement_bounds);
                self.commit_dynamic_hazard(&replacement)?;
                state.placements[input_index] = replacement;
                update_score_after_move(
                    score,
                    input_index,
                    old_boundary,
                    replacement_score,
                    &self.weights,
                    &mut self.collision_merge_scratch,
                );
                self.audit_incremental_score(state, score, input_index, &piece_index)?;
            }
        }
        Ok(())
    }

    /// Writes the lane's scheduled depth into the state, and brings the
    /// tracker's boundary rows onto it.
    ///
    /// This is the whole of the depth schedule's cost inside a sweep, and it is
    /// what the mode-26 anatomy priced: **one `f64` write per sweep and zero
    /// additional geometry.** `boundary_penalty` already takes the depth as a
    /// parameter at all eleven of its call sites and every one of them passes
    /// `state.strip_depth_mm`, so substituting the schedule's depth *into that
    /// scalar* reaches all eleven - the penalty itself, and the sampling boxes
    /// `random_candidate`, `random_directional_candidate`,
    /// `directional_inner_fit` and `repair_contact_candidates` derive from
    /// `strip_depth_mm - inset - local.max_y`.
    ///
    /// The refresh is the part that is *not* free, and it is bounded: the
    /// tracker's boundary rows were measured against the old depth, so a
    /// changed depth invalidates exactly `n` of them and nothing else. The pair
    /// rows are untouched - a pair's penetration does not depend on the sheet -
    /// so this is `n` calls at a measured 84.9 ns, about 5 µs for 61 pieces,
    /// against the 2,555 candidate queries an m26-class sweep costs. It runs
    /// only when the depth actually moved, which is once per *step* rather than
    /// once per sweep.
    ///
    /// Without it the tracker would report the old depth's feasibility and the
    /// sweep below would return immediately, which is the silent failure this
    /// function exists to prevent.
    #[cfg(feature = "compression-schedule")]
    fn apply_compression_schedule(
        &mut self,
        state: &mut RelaxedState,
        score: &mut PairTracker,
    ) -> Result<(), GeneralFastError> {
        let Some(schedule) = self.compression.as_mut() else {
            return Ok(());
        };
        schedule.note_sweep();
        let depth_mm = schedule.depth_mm();
        if depth_mm == state.strip_depth_mm {
            return Ok(());
        }
        state.strip_depth_mm = depth_mm;
        self.refresh_boundary_rows(state, score)
    }

    #[cfg(not(feature = "compression-schedule"))]
    #[inline(always)]
    fn apply_compression_schedule(
        &mut self,
        _state: &mut RelaxedState,
        _score: &mut PairTracker,
    ) -> Result<(), GeneralFastError> {
        Ok(())
    }

    /// The tightest strip depth this state occupies without a single boundary
    /// violation.
    ///
    /// The schedule has to start at the frontier the layout *actually* sits on,
    /// and that is not the layout's reported depth. A reported depth measures
    /// the material; `boundary_penalty` measures the **collision** polygon,
    /// which carries `collision_expansion_mm` - half the pair clearance, the
    /// safety margin and the search-offset allowance - and is compared against
    /// `strip_depth - inset`. Seeding the schedule with the material depth
    /// therefore over-clamps the very first step by the whole envelope
    /// (measured on the 159.079 parent: 12 protruding pieces and a boundary
    /// loss of 1.0e4 at what should have been a one-micron perturbation), and
    /// the schedule spends its budget chasing a residue that was never a
    /// residue of *its* step.
    ///
    /// This is the same quantity `boundary_penalty` would drive to zero, read
    /// directly off the bounds it reads: `max_y + inset` over the layout. It
    /// costs one `placement_bounds` call per piece.
    #[cfg(feature = "compression-schedule")]
    fn tight_strip_depth(&mut self, state: &RelaxedState) -> Result<f64, GeneralFastError> {
        let inset = collision_sheet_inset_mm(self.fast_settings);
        let mut deepest = f64::NEG_INFINITY;
        for index in 0..state.placements.len() {
            let bounds = self.placement_bounds(&state.placements[index])?;
            deepest = deepest.max(bounds.max_y);
        }
        Ok(if deepest.is_finite() {
            deepest + inset
        } else {
            state.strip_depth_mm
        })
    }

    /// Re-measures every piece's boundary row against the state's current
    /// strip depth, leaving the pair rows alone.
    ///
    /// Compiled only with the schedule, because it is the only thing that
    /// moves a depth under a live tracker.
    #[cfg(feature = "compression-schedule")]
    fn refresh_boundary_rows(
        &mut self,
        state: &RelaxedState,
        score: &mut PairTracker,
    ) -> Result<(), GeneralFastError> {
        let depth_mm = state.strip_depth_mm;
        let mut violations = 0usize;
        let mut loss = 0.0;
        for index in 0..state.placements.len() {
            let (piece_violations, piece_loss) =
                self.boundary_penalty(&state.placements[index], depth_mm)?;
            score.replace_boundary(
                index,
                BoundaryEntry {
                    violations: piece_violations,
                    raw_loss: piece_loss,
                },
            );
            violations = violations.saturating_add(piece_violations);
            loss += piece_loss;
        }
        score.boundary_violations = violations;
        score.boundary_loss = loss;
        refresh_weighted_loss(score, &self.weights);
        Ok(())
    }

    /// Compares the incrementally maintained score against a complete rescore
    /// of the same state.
    ///
    /// Compiled out entirely unless the `shadow-rescore` feature is on, in
    /// which case it runs after every accepted move. See
    /// [`crate::search::shadow_rescore`] for the agreement rule and how to read
    /// the report.
    ///
    /// The audit must not be able to change what the search does, so the lane's
    /// work counters are saved and restored around it: the rescore's probes and
    /// pressure evaluations feed deterministic quotas, and letting them land
    /// would make the audited run a different search from the audited-out one.
    /// The other state a rescore reaches is read-only or idempotent — the
    /// hazard index is only queried, and the proxy row cache re-derives exactly
    /// what it stored.
    ///
    /// One exception is worth stating rather than glossing: the *directional*
    /// backend's rescore fills the lane's pair-NFP cache, which is budgeted, so
    /// an audited directional lane can reach that budget earlier than an
    /// unaudited one and take the unscorable branch sooner. The audited streams
    /// are the dynamic-hazard ones, where this does not arise, and both pinned
    /// gates reproduce their fingerprints exactly under the audit — but a
    /// directional arm run under this feature is not guaranteed to be the same
    /// search, and its outcome should not be quoted as one.
    #[cfg(feature = "shadow-rescore")]
    fn audit_incremental_score(
        &mut self,
        state: &RelaxedState,
        incremental: &PairTracker,
        moved_index: usize,
        piece_index: &PieceIndex,
    ) -> Result<(), GeneralFastError> {
        let saved_counters = self.counters;
        let shadow = self.score_state(state);
        self.counters = saved_counters;
        let mut shadow = shadow?;
        // Same summation order in both paths: the complete score accumulates
        // the weighted total interleaved with its boundary walk, so recompute
        // it the way the delta does before comparing. What is under test is the
        // delta, not which of two orders `+` was applied in.
        refresh_weighted_loss(&mut shadow, &self.weights);
        match shadow_tracker_disagreement(&shadow, incremental) {
            ShadowAgreement::Rows(rendered) => {
                let detail = self
                    .render_structural_detail(state, &shadow, incremental, moved_index, piece_index);
                shadow_rescore::record_disagreement(rendered, detail);
            }
            ShadowAgreement::MagnitudeOnly {
                rendered,
                worst_pressure_ulps,
            } => shadow_rescore::record_magnitude_only(rendered, worst_pressure_ulps),
            ShadowAgreement::Agrees { derived_ulps } => {
                shadow_rescore::record_agreement(derived_ulps)
            }
        }
        Ok(())
    }

    /// Renders *why* one structural disagreement happened, for the audit's
    /// census.
    ///
    /// The verdict alone says a row count differs. What settles the *mechanism*
    /// is four further questions, and this asks all four of every pair the two
    /// trackers disagree about:
    ///
    /// * **Was the move a revert?** A reverted move reinstalls its row out of
    ///   the tracker instead of measuring one, which was a named suspect.
    /// * **Did a confirmation run?** `confirmedRowLen` is `-1` when the lane is
    ///   not on the dynamic-hazard backend, which rules the hazard index out of
    ///   a disagreement rather than leaving it a suspect.
    /// * **Did the broad phase offer the pair?** If the moved piece's own
    ///   `PieceIndex` query does not contain the partner, the row never had the
    ///   chance to record it and the defect is in the index; if it does, the
    ///   defect is downstream of it.
    /// * **Do the two proxy tiers answer the same asked either way round?**
    ///   Both tiers are asked as `(i, j)` and as `(j, i)`. An asymmetric answer
    ///   is a collider that is not a function of the unordered pair; a
    ///   symmetric answer that the tracker's row contradicts is a stale row.
    ///
    /// That last question is the one that identified this defect - see
    /// `canonical_pair_operands`.
    ///
    /// Diagnostic only, and it restores the lane's counters like its caller
    /// does, so an audited run stays the same search.
    #[cfg(feature = "shadow-rescore")]
    fn render_structural_detail(
        &mut self,
        state: &RelaxedState,
        shadow: &PairTracker,
        incremental: &PairTracker,
        moved_index: usize,
        piece_index: &PieceIndex,
    ) -> String {
        let saved_counters = self.counters;
        // What the moved piece's own broad phase would report right now. A pair
        // the complete score sees and the tracker does not is either a pair the
        // broad phase never offered the candidate scorer, or one it offered and
        // the row dropped; those are different defects and this tells them
        // apart.
        let moved_bounds = self.placement_bounds(&state.placements[moved_index]).ok();
        let broad_phase = moved_bounds.map(|bounds| {
            let mut scratch = PieceQueryScratch::new(state.placements.len());
            piece_index.query_into(bounds, &mut scratch);
            scratch.selected.iter().copied().collect::<BTreeSet<_>>()
        });
        let shadow_rows = shadow
            .collision_pairs
            .iter()
            .map(|(first, second, _)| (*first, *second))
            .collect::<BTreeSet<_>>();
        let incremental_rows = incremental
            .collision_pairs
            .iter()
            .map(|(first, second, _)| (*first, *second))
            .collect::<BTreeSet<_>>();
        let confirmed_row_len = match &self.audit_last_confirmed_row {
            Some((index, row)) if *index == moved_index => row.len() as i64,
            _ => -1,
        };
        let mut rendered = format!(
            "moved piece {moved_index} (inputIndex {}), revert={}, shadow rows {}, \
             tracker rows {}, confirmedRowLen {confirmed_row_len}",
            state.placements[moved_index].input_index,
            self.audit_move_was_revert,
            shadow.collision_pairs.len(),
            incremental.collision_pairs.len()
        );
        let differing = shadow_rows
            .symmetric_difference(&incremental_rows)
            .copied()
            .collect::<Vec<_>>();
        for (first, second) in differing {
            let side = if shadow_rows.contains(&(first, second)) {
                "shadow-only"
            } else {
                "tracker-only"
            };
            let forward = self
                .confirmed_pair_pressure(&state.placements[first], &state.placements[second])
                .unwrap_or(f64::NAN);
            let reverse = self
                .confirmed_pair_pressure(&state.placements[second], &state.placements[first])
                .unwrap_or(f64::NAN);
            let tracked = incremental.pair(first, second).raw_loss;
            let touches_moved = first == moved_index || second == moved_index;
            let confirmed = match &self.audit_last_confirmed_row {
                Some((index, row)) if *index == moved_index => {
                    match row.iter().find(|(a, b, _)| (*a, *b) == (first, second)) {
                        Some((_, _, penalty)) => format!("{penalty:.17e}"),
                        None => "absent".to_owned(),
                    }
                }
                _ => "no-confirm-for-this-move".to_owned(),
            };
            // The proxy collider actually in play on this backend, asked both
            // ways round. `confirmed_pair_pressure` above is the *other* proxy
            // tier (zero-degree cells transformed at query time); this one is
            // the rotated-surrogate tier a candidate scan and a whole-layout
            // score both run on, and it is the one whose verdict decides
            // whether a row exists at all.
            let proxy_forward = self
                .pair_collides(&state.placements[first], &state.placements[second])
                .unwrap_or(false);
            let proxy_reverse = self
                .pair_collides(&state.placements[second], &state.placements[first])
                .unwrap_or(false);
            let partner = if first == moved_index { second } else { first };
            let in_broad_phase = match &broad_phase {
                Some(selected) => format!("{}", selected.contains(&partner)),
                None => "unknown".to_owned(),
            };
            rendered.push_str(&format!(
                "; {side} pair ({first}, {second}) touchesMoved={touches_moved} \
                 remeasured(lower,higher)={forward:.17e} remeasured(higher,lower)={reverse:.17e} \
                 trackerPairRow={tracked:.17e} inConfirmedRow={confirmed} \
                 partnerInMovedBroadPhase={in_broad_phase} \
                 proxyCollides(lower,higher)={proxy_forward} \
                 proxyCollides(higher,lower)={proxy_reverse}"
            ));
        }
        self.counters = saved_counters;
        rendered
    }

    #[cfg(not(feature = "shadow-rescore"))]
    #[inline(always)]
    fn audit_incremental_score(
        &mut self,
        _state: &RelaxedState,
        _incremental: &PairTracker,
        _moved_index: usize,
        _piece_index: &PieceIndex,
    ) -> Result<(), GeneralFastError> {
        Ok(())
    }

    fn try_ejection_chain(
        &mut self,
        state: &mut RelaxedState,
        score: &mut PairTracker,
    ) -> Result<(), GeneralFastError> {
        let Some((first, second, _)) = score
            .collision_pairs
            .iter()
            .max_by(|first, second| {
                let first_pressure = first.2 * self.pair_weight(first.0, first.1);
                let second_pressure = second.2 * self.pair_weight(second.0, second.1);
                first_pressure.total_cmp(&second_pressure).then_with(|| {
                    ordered_pair(second.0, second.1).cmp(&ordered_pair(first.0, first.1))
                })
            })
            .copied()
        else {
            return Ok(());
        };

        let mut candidates = Vec::new();
        for root in [first, second] {
            let donors = self.chain_donors(state, root)?;
            for donor in donors.iter().copied() {
                let root_orientations = self.chain_orientations(
                    root,
                    &state.placements[root],
                    &state.placements[donor],
                );
                let donor_orientations = self.chain_orientations(
                    donor,
                    &state.placements[donor],
                    &state.placements[root],
                );
                for (root_angle, root_mirror) in root_orientations.iter().copied() {
                    for (donor_angle, donor_mirror) in donor_orientations.iter().copied() {
                        let replacements = vec![
                            (
                                root,
                                self.placement_in_slot(
                                    root,
                                    root_angle,
                                    root_mirror,
                                    &state.placements[donor],
                                )?,
                            ),
                            (
                                donor,
                                self.placement_in_slot(
                                    donor,
                                    donor_angle,
                                    donor_mirror,
                                    &state.placements[root],
                                )?,
                            ),
                        ];
                        self.report_ejection_candidate(
                            state,
                            score,
                            replacements,
                            &mut candidates,
                        )?;
                    }
                }
            }

            for donor_pair in donors.windows(2) {
                let first_donor = donor_pair[0];
                let second_donor = donor_pair[1];
                let root_orientation = self.chain_orientations(
                    root,
                    &state.placements[root],
                    &state.placements[first_donor],
                )[0];
                let first_orientation = self.chain_orientations(
                    first_donor,
                    &state.placements[first_donor],
                    &state.placements[second_donor],
                )[0];
                let second_orientation = self.chain_orientations(
                    second_donor,
                    &state.placements[second_donor],
                    &state.placements[root],
                )[0];
                let replacements = vec![
                    (
                        root,
                        self.placement_in_slot(
                            root,
                            root_orientation.0,
                            root_orientation.1,
                            &state.placements[first_donor],
                        )?,
                    ),
                    (
                        first_donor,
                        self.placement_in_slot(
                            first_donor,
                            first_orientation.0,
                            first_orientation.1,
                            &state.placements[second_donor],
                        )?,
                    ),
                    (
                        second_donor,
                        self.placement_in_slot(
                            second_donor,
                            second_orientation.0,
                            second_orientation.1,
                            &state.placements[root],
                        )?,
                    ),
                ];
                self.report_ejection_candidate(state, score, replacements, &mut candidates)?;
            }
        }

        if candidates.is_empty() {
            return Ok(());
        }
        candidates.sort_by(compare_ejection_candidates);
        candidates.dedup_by(|first, second| {
            ejection_candidate_key(first) == ejection_candidate_key(second)
        });
        let improving = compare_chain_score(&candidates[0].score, score) == Ordering::Less;
        let selected = if improving {
            0
        } else if self.allow_worsening_chain {
            let eligible = candidates
                .iter()
                .take(EJECTION_CHAIN_DIVERSITY)
                .take_while(|candidate| {
                    candidate.score.weighted_loss <= score.weighted_loss * 2.0
                        && candidate.score.collision_pairs.len()
                            <= score.collision_pairs.len().saturating_add(3)
                })
                .count();
            if eligible == 0 {
                return Ok(());
            }
            (self.rng.next_u64() as usize) % eligible
        } else {
            return Ok(());
        };
        let selected = candidates.swap_remove(selected);
        for (index, placement) in selected.replacements {
            state.placements[index] = placement;
        }
        *score = selected.score;
        self.counters.ejection_chain_accepts += 1;
        Ok(())
    }

    fn report_ejection_candidate(
        &mut self,
        state: &RelaxedState,
        score: &PairTracker,
        replacements: Vec<(usize, RelaxedPlacement)>,
        candidates: &mut Vec<EjectionCandidate>,
    ) -> Result<(), GeneralFastError> {
        self.counters.ejection_chain_evaluations += 1;
        let candidate_score = self.score_after_replacements(state, score, &replacements)?;
        candidates.push(EjectionCandidate {
            replacements,
            score: candidate_score,
        });
        Ok(())
    }

    fn chain_donors(
        &mut self,
        state: &RelaxedState,
        root: usize,
    ) -> Result<Vec<usize>, GeneralFastError> {
        let root_placement = &state.placements[root];
        let root_bounds = self
            .oriented(
                root_placement.input_index,
                root_placement.rotation_deg,
                root_placement.mirrored,
            )?
            .bounds;
        let root_width = (root_bounds.max_x - root_bounds.min_x).max(0.001);
        let root_height = (root_bounds.max_y - root_bounds.min_y).max(0.001);
        let root_area = root_width * root_height;
        let root_aspect = (root_width / root_height).ln().abs();
        let mut ranked = Vec::new();
        for (index, placement) in state.placements.iter().enumerate() {
            if index == root
                || same_piece_geometry(
                    self.pieces[root_placement.input_index],
                    self.pieces[placement.input_index],
                )
            {
                continue;
            }
            let bounds = self
                .oriented(
                    placement.input_index,
                    placement.rotation_deg,
                    placement.mirrored,
                )?
                .bounds;
            let width = (bounds.max_x - bounds.min_x).max(0.001);
            let height = (bounds.max_y - bounds.min_y).max(0.001);
            let area = width * height;
            let aspect = (width / height).ln().abs();
            let fit = (area / root_area).ln().abs() + (aspect - root_aspect).abs() * 0.5;
            let frontier = placement.translate_y + bounds.max_y;
            ranked.push((index, fit, frontier, self.pieces[index].id));
        }
        ranked.sort_by(|first, second| {
            first
                .1
                .total_cmp(&second.1)
                .then_with(|| second.2.total_cmp(&first.2))
                .then_with(|| first.3.cmp(second.3))
        });
        Ok(ranked
            .into_iter()
            .take(EJECTION_CHAIN_MAX_DONORS)
            .map(|(index, _, _, _)| index)
            .collect())
    }

    fn chain_orientations(
        &self,
        input_index: usize,
        current: &RelaxedPlacement,
        slot: &RelaxedPlacement,
    ) -> Vec<(f64, bool)> {
        let piece = self.pieces[input_index];
        let mut orientations = BTreeSet::new();
        orientations.insert((
            angle_key(if piece.allow_rotation {
                current.rotation_deg
            } else {
                0.0
            }),
            piece.allow_mirror && current.mirrored,
        ));
        orientations.insert((
            angle_key(if piece.allow_rotation {
                slot.rotation_deg
            } else {
                0.0
            }),
            piece.allow_mirror && slot.mirrored,
        ));
        orientations
            .into_iter()
            .map(|(angle, mirrored)| (angle_from_key(angle), mirrored))
            .collect()
    }

    fn placement_in_slot(
        &mut self,
        input_index: usize,
        rotation_deg: f64,
        mirrored: bool,
        slot: &RelaxedPlacement,
    ) -> Result<RelaxedPlacement, GeneralFastError> {
        let slot_bounds =
            self.local_shape_bounds(slot.input_index, slot.rotation_deg, slot.mirrored)?;
        let target_x = slot.translate_x + (slot_bounds.min_x + slot_bounds.max_x) * 0.5;
        let target_y = slot.translate_y + (slot_bounds.min_y + slot_bounds.max_y) * 0.5;
        let shape_bounds = self.local_shape_bounds(input_index, rotation_deg, mirrored)?;
        Ok(RelaxedPlacement {
            input_index,
            rotation_deg,
            mirrored,
            translate_x: snap_mm(target_x - (shape_bounds.min_x + shape_bounds.max_x) * 0.5),
            translate_y: snap_mm(target_y - (shape_bounds.min_y + shape_bounds.max_y) * 0.5),
        })
    }

    fn score_after_replacements(
        &mut self,
        state: &RelaxedState,
        base: &PairTracker,
        replacements: &[(usize, RelaxedPlacement)],
    ) -> Result<PairTracker, GeneralFastError> {
        let moved = replacements
            .iter()
            .map(|(index, _)| *index)
            .collect::<BTreeSet<_>>();
        let replacement_map = replacements
            .iter()
            .map(|(index, placement)| (*index, placement))
            .collect::<BTreeMap<_, _>>();
        let mut result = base.clone();
        for index in moved.iter().copied() {
            let old_boundary =
                self.boundary_penalty(&state.placements[index], state.strip_depth_mm)?;
            result.boundary_violations = result.boundary_violations.saturating_sub(old_boundary.0);
            result.boundary_loss = (result.boundary_loss - old_boundary.1).max(0.0);
            result.weighted_loss = (result.weighted_loss - old_boundary.1).max(0.0);
        }
        let mut removed_pair_loss = 0.0;
        result.collision_pairs.retain(|(first, second, penalty)| {
            let remove = moved.contains(first) || moved.contains(second);
            if remove {
                removed_pair_loss += self.pair_weight(*first, *second) * *penalty;
            }
            !remove
        });
        result.weighted_loss = (result.weighted_loss - removed_pair_loss).max(0.0);

        for index in moved.iter().copied() {
            let placement = replacement_map[&index];
            let (violations, loss) = self.boundary_penalty(placement, state.strip_depth_mm)?;
            result.replace_boundary(
                index,
                BoundaryEntry {
                    violations,
                    raw_loss: loss,
                },
            );
            result.boundary_violations = result.boundary_violations.saturating_add(violations);
            result.boundary_loss += loss;
            result.weighted_loss += loss;
            for fixed in 0..state.placements.len() {
                if fixed == index || moved.contains(&fixed) {
                    continue;
                }
                let penalty = self.pair_penalty(placement, &state.placements[fixed])?;
                if penalty > 0.0 {
                    let pair = ordered_pair(index, fixed);
                    result.collision_pairs.push((pair.0, pair.1, penalty));
                    result.weighted_loss += self.pair_weight(pair.0, pair.1) * penalty;
                }
            }
        }
        let moved = moved.into_iter().collect::<Vec<_>>();
        for first_position in 0..moved.len() {
            for second_position in (first_position + 1)..moved.len() {
                let first = moved[first_position];
                let second = moved[second_position];
                let penalty =
                    self.pair_penalty(replacement_map[&first], replacement_map[&second])?;
                if penalty > 0.0 {
                    let pair = ordered_pair(first, second);
                    result.collision_pairs.push((pair.0, pair.1, penalty));
                    result.weighted_loss += self.pair_weight(pair.0, pair.1) * penalty;
                }
            }
        }
        result
            .collision_pairs
            .sort_by_key(|(first, second, _)| (*first, *second));
        for index in moved.iter().copied() {
            for fixed in 0..result.piece_count {
                if fixed == index {
                    continue;
                }
                let pair = ordered_pair(index, fixed);
                let raw_loss = result
                    .collision_pairs
                    .iter()
                    .find(|(first, second, _)| (*first, *second) == pair)
                    .map(|(_, _, penalty)| *penalty)
                    .unwrap_or(0.0);
                result.replace_pair(pair.0, pair.1, raw_loss, self.pair_weight(pair.0, pair.1));
            }
        }
        Ok(result)
    }

    fn search_piece(
        &mut self,
        state: &RelaxedState,
        tracker: &PairTracker,
        input_index: usize,
        piece_index: &PieceIndex,
    ) -> Result<(RelaxedPlacement, MovedRowDelta), GeneralFastError> {
        let current = state.placements[input_index].clone();
        let current_bounds =
            self.local_shape_bounds(input_index, current.rotation_deg, current.mirrored)?;
        let unique_position_threshold = ((current_bounds.max_x - current_bounds.min_x)
            .min(current_bounds.max_y - current_bounds.min_y)
            * UNIQUE_SAMPLE_POSITION_RATIO)
            .max(0.001);
        let current_score =
            self.score_placement(state, input_index, &current, piece_index, None)?;
        if self.uses_directional_pressure() {
            let evaluation_budget =
                AXIS_MINIMIZATION_PASSES.saturating_mul(AXIS_RETAINED_CANDIDATES);
            return Ok(self
                .minimize_candidate_axes(
                    state,
                    tracker,
                    input_index,
                    current.clone(),
                    current_score.clone(),
                    piece_index,
                    evaluation_budget,
                )?
                .map(|(placement, score, _)| (placement, score))
                .unwrap_or((current, current_score)));
        }
        let mut starts = vec![(current.clone(), current_score)];
        let focused_radius_x = (current_bounds.max_x - current_bounds.min_x) * 1.5;
        let focused_radius_y = (current_bounds.max_y - current_bounds.min_y) * 1.5;
        for _ in 0..self.relaxed_settings.focused_samples_per_move {
            let candidate = self.random_candidate(
                &current,
                input_index,
                state.strip_depth_mm,
                Some((focused_radius_x, focused_radius_y)),
            )?;
            let score = self.score_placement(
                state,
                input_index,
                &candidate,
                piece_index,
                sample_upper_bound(&starts),
            )?;
            self.count_seed_evaluation(&current, &candidate);
            report_diverse_sample(&mut starts, candidate, score, unique_position_threshold);
        }
        for _ in 0..self.relaxed_settings.global_samples_per_move {
            let candidate =
                self.random_candidate(&current, input_index, state.strip_depth_mm, None)?;
            let score = self.score_placement(
                state,
                input_index,
                &candidate,
                piece_index,
                sample_upper_bound(&starts),
            )?;
            self.count_seed_evaluation(&current, &candidate);
            report_diverse_sample(&mut starts, candidate, score, unique_position_threshold);
        }

        let refinement_budget = self
            .relaxed_settings
            .refinement_rounds
            .saturating_mul(10)
            .saturating_mul(starts.len());
        let pre_refinement_budget = refinement_budget.saturating_mul(3) / 4;
        let per_start_budget = even_floor(pre_refinement_budget / starts.len().max(1));
        let mut refined = Vec::with_capacity(starts.len());
        let mut refinement_evaluations = 0usize;
        for (start, start_score) in starts {
            let (candidate, score, evaluations) = self.refine_candidate(
                state,
                input_index,
                start,
                start_score,
                piece_index,
                unique_position_threshold / UNIQUE_SAMPLE_POSITION_RATIO,
                PRE_REFINEMENT_INITIAL_RATIO,
                PRE_REFINEMENT_LIMIT_RATIO,
                5.0,
                1.0,
                per_start_budget,
            )?;
            refinement_evaluations = refinement_evaluations.saturating_add(evaluations);
            refined.push((candidate, score));
        }
        refined.sort_by(|(first, first_score), (second, second_score)| {
            compare_move_score(first_score, first, second_score, second)
        });
        let (best, best_score) = refined
            .into_iter()
            .next()
            .expect("the current placement always provides a refinement start");
        let final_budget = even_floor(refinement_budget.saturating_sub(refinement_evaluations));
        if ENABLE_NFP_AXIS_MINIMIZER {
            if let Some((best, best_score, axis_evaluations)) = self.minimize_candidate_axes(
                state,
                tracker,
                input_index,
                best.clone(),
                best_score.clone(),
                piece_index,
                final_budget,
            )? {
                let remaining = even_floor(final_budget.saturating_sub(axis_evaluations));
                let (best, best_score, _) = self.refine_candidate(
                    state,
                    input_index,
                    best,
                    best_score,
                    piece_index,
                    unique_position_threshold / UNIQUE_SAMPLE_POSITION_RATIO,
                    FINAL_REFINEMENT_INITIAL_RATIO,
                    FINAL_REFINEMENT_LIMIT_RATIO,
                    0.5,
                    0.05,
                    remaining,
                )?;
                return Ok((best, best_score));
            }
        }
        let (best, best_score, _) = self.refine_candidate(
            state,
            input_index,
            best,
            best_score,
            piece_index,
            unique_position_threshold / UNIQUE_SAMPLE_POSITION_RATIO,
            FINAL_REFINEMENT_INITIAL_RATIO,
            FINAL_REFINEMENT_LIMIT_RATIO,
            0.5,
            0.05,
            final_budget,
        )?;
        Ok((best, best_score))
    }

    fn piece_is_active(
        &mut self,
        state: &RelaxedState,
        score: &PairTracker,
        input_index: usize,
    ) -> Result<bool, GeneralFastError> {
        if score
            .collision_pairs
            .iter()
            .any(|(first, second, _)| *first == input_index || *second == input_index)
        {
            return Ok(true);
        }
        Ok(self
            .boundary_penalty(&state.placements[input_index], state.strip_depth_mm)?
            .0
            > 0)
    }

    fn refine_candidate(
        &mut self,
        state: &RelaxedState,
        input_index: usize,
        mut best: RelaxedPlacement,
        mut best_score: MovedRowDelta,
        piece_index: &PieceIndex,
        minimum_dimension: f64,
        initial_step_ratio: f64,
        limit_step_ratio: f64,
        initial_rotation_step_deg: f64,
        rotation_step_limit_deg: f64,
        evaluation_budget: usize,
    ) -> Result<(RelaxedPlacement, MovedRowDelta, usize), GeneralFastError> {
        let mut step_x = minimum_dimension * initial_step_ratio;
        let mut step_y = minimum_dimension * initial_step_ratio;
        let step_limit = (minimum_dimension * limit_step_ratio).max(0.001);
        let can_refine_rotation = self.refine_rotation
            && self.uses_dynamic_pressure()
            && self.pieces[input_index].allow_rotation;
        let mut rotation_step_deg = initial_rotation_step_deg;
        let mut axis = self.random_coordinate_axis(
            step_x,
            step_y,
            step_limit,
            rotation_step_deg,
            rotation_step_limit_deg,
            can_refine_rotation,
        );
        let mut evaluations = 0usize;
        while evaluations + 2 <= evaluation_budget
            && (step_x >= step_limit
                || step_y >= step_limit
                || (can_refine_rotation && rotation_step_deg >= rotation_step_limit_deg))
        {
            let offsets = coordinate_offsets(axis, step_x, step_y, rotation_step_deg);
            let first_candidate = RelaxedPlacement {
                input_index,
                rotation_deg: continuous_angle(best.rotation_deg + offsets[0].2),
                mirrored: best.mirrored,
                translate_x: snap_mm(best.translate_x + offsets[0].0),
                translate_y: snap_mm(best.translate_y + offsets[0].1),
            };
            let first_score = self.score_placement(
                state,
                input_index,
                &first_candidate,
                piece_index,
                Some(best_score.weighted_loss),
            )?;
            let second_candidate = RelaxedPlacement {
                input_index,
                rotation_deg: continuous_angle(best.rotation_deg + offsets[1].2),
                mirrored: best.mirrored,
                translate_x: snap_mm(best.translate_x + offsets[1].0),
                translate_y: snap_mm(best.translate_y + offsets[1].1),
            };
            let second_score = self.score_placement(
                state,
                input_index,
                &second_candidate,
                piece_index,
                Some(best_score.weighted_loss),
            )?;
            evaluations += 2;
            if axis == CoordinateAxis::Rotation {
                self.counters.rotation_evaluations =
                    self.counters.rotation_evaluations.saturating_add(2);
            } else {
                self.counters.translation_evaluations =
                    self.counters.translation_evaluations.saturating_add(2);
            }
            let selected = match compare_score_objective(&first_score, &second_score) {
                Ordering::Less => 0,
                Ordering::Greater => 1,
                Ordering::Equal => (self.rng.next_u64() as usize) & 1,
            };
            // The loop retires exactly two row buffers per iteration: the
            // candidate the probe did not select, and — when the selected one
            // wins — the incumbent it displaces. Both used to go back to the
            // allocator here, and under `relaxed-row-buffer-reuse` both go back
            // to the lane's pool instead, which is where the next two
            // `score_placement` calls take theirs from. The values, their order
            // and the two comparisons above are untouched; the discarded delta
            // is destructured at the same point it was dropped before.
            let (candidate, score, discarded) = if selected == 0 {
                (first_candidate, first_score, second_score)
            } else {
                (second_candidate, second_score, first_score)
            };
            self.recycle_row_buffer(discarded.collision_pairs);
            let comparison = compare_score_objective(&score, &best_score);
            if comparison != Ordering::Greater {
                best = candidate;
                let displaced = std::mem::replace(&mut best_score, score);
                self.recycle_row_buffer(displaced.collision_pairs);
            } else {
                self.recycle_row_buffer(score.collision_pairs);
            }
            let multiplier = if comparison == Ordering::Less {
                REFINEMENT_SUCCESS_MULTIPLIER
            } else {
                REFINEMENT_FAILURE_MULTIPLIER
            };
            apply_coordinate_multiplier(
                axis,
                &mut step_x,
                &mut step_y,
                &mut rotation_step_deg,
                multiplier,
            );
            if comparison != Ordering::Less
                && (step_x >= step_limit
                    || step_y >= step_limit
                    || (can_refine_rotation && rotation_step_deg >= rotation_step_limit_deg))
            {
                axis = self.random_coordinate_axis(
                    step_x,
                    step_y,
                    step_limit,
                    rotation_step_deg,
                    rotation_step_limit_deg,
                    can_refine_rotation,
                );
            }
        }
        Ok((best, best_score, evaluations))
    }

    fn minimize_candidate_axes(
        &mut self,
        state: &RelaxedState,
        tracker: &PairTracker,
        input_index: usize,
        mut best: RelaxedPlacement,
        mut best_score: MovedRowDelta,
        piece_index: &PieceIndex,
        evaluation_budget: usize,
    ) -> Result<Option<(RelaxedPlacement, MovedRowDelta, usize)>, GeneralFastError> {
        if evaluation_budget < 2 {
            return Ok(None);
        }
        let mut evaluations = 0usize;
        for pass in 0..AXIS_MINIMIZATION_PASSES {
            let axis = if pass % 2 == 0 {
                CoordinateAxis::Horizontal
            } else {
                CoordinateAxis::Vertical
            };
            let remaining = evaluation_budget.saturating_sub(evaluations);
            if remaining == 0 {
                break;
            }
            let Some(axis_values) = self.axis_minima(
                state,
                tracker,
                input_index,
                &best,
                axis,
                AXIS_RETAINED_CANDIDATES.min(remaining),
            )?
            else {
                return Ok(None);
            };
            let mut improved = false;
            for value in axis_values {
                let mut candidate = best.clone();
                match axis {
                    CoordinateAxis::Horizontal => candidate.translate_x = value,
                    CoordinateAxis::Vertical => candidate.translate_y = value,
                    CoordinateAxis::ForwardDiagonal
                    | CoordinateAxis::BackwardDiagonal
                    | CoordinateAxis::Rotation => {
                        unreachable!("axis minimization only uses cardinal axes")
                    }
                }
                if move_tie_key(&candidate) == move_tie_key(&best) {
                    continue;
                }
                let score = self.score_placement(
                    state,
                    input_index,
                    &candidate,
                    piece_index,
                    Some(best_score.weighted_loss),
                )?;
                evaluations = evaluations.saturating_add(1);
                self.counters.axis_candidate_evaluations =
                    self.counters.axis_candidate_evaluations.saturating_add(1);
                self.counters.translation_evaluations =
                    self.counters.translation_evaluations.saturating_add(1);
                if compare_score_objective(&score, &best_score) == Ordering::Less {
                    best = candidate;
                    best_score = score;
                    improved = true;
                }
                if evaluations >= evaluation_budget {
                    break;
                }
            }
            if !improved && pass >= 1 {
                break;
            }
        }
        Ok(Some((best, best_score, evaluations)))
    }

    fn axis_minima(
        &mut self,
        state: &RelaxedState,
        tracker: &PairTracker,
        input_index: usize,
        moving: &RelaxedPlacement,
        axis: CoordinateAxis,
        retained: usize,
    ) -> Result<Option<Vec<f64>>, GeneralFastError> {
        let moving_bounds =
            self.local_shape_bounds(moving.input_index, moving.rotation_deg, moving.mirrored)?;
        let moving_diameter = ((moving_bounds.max_x - moving_bounds.min_x).powi(2)
            + (moving_bounds.max_y - moving_bounds.min_y).powi(2))
        .sqrt();
        let inset = collision_sheet_inset_mm(self.fast_settings);
        let (minimum, maximum, current, direction) = match axis {
            CoordinateAxis::Horizontal => (
                inset - moving_bounds.min_x,
                self.fast_settings.sheet_short_axis_mm - inset - moving_bounds.max_x,
                moving.translate_x,
                (1.0, 0.0),
            ),
            CoordinateAxis::Vertical => (
                inset - moving_bounds.min_y,
                state.strip_depth_mm - inset - moving_bounds.max_y,
                moving.translate_y,
                (0.0, 1.0),
            ),
            CoordinateAxis::ForwardDiagonal
            | CoordinateAxis::BackwardDiagonal
            | CoordinateAxis::Rotation => {
                unreachable!("axis minimization only uses cardinal axes")
            }
        };
        if minimum > maximum {
            return Ok(Some(Vec::new()));
        }

        let mut pair_functions = Vec::with_capacity(state.placements.len().saturating_sub(1));
        let mut events = grid_neighbors_clamped(minimum, minimum, maximum);
        events.extend(grid_neighbors_clamped(maximum, minimum, maximum));
        events.extend(grid_neighbors_clamped(current, minimum, maximum));
        let mut logical_components = 0usize;
        for (fixed_index, fixed) in state.placements.iter().enumerate() {
            if fixed_index == input_index {
                continue;
            }
            let fixed_bounds =
                self.local_shape_bounds(fixed.input_index, fixed.rotation_deg, fixed.mirrored)?;
            let fixed_diameter = ((fixed_bounds.max_x - fixed_bounds.min_x).powi(2)
                + (fixed_bounds.max_y - fixed_bounds.min_y).powi(2))
            .sqrt();
            let relative = IrregularPoint::new(
                moving.translate_x - fixed.translate_x,
                moving.translate_y - fixed.translate_y,
            );
            let nfp_key = self.pair_nfp_key(fixed, moving)?;
            let Some(pair_nfp) = self.resolve_pair_nfp(fixed, moving)? else {
                return Ok(None);
            };
            logical_components = logical_components.saturating_add(pair_nfp.components.len());
            if logical_components > MAX_NFP_COMPONENTS_PER_MOVE {
                return Ok(None);
            }
            let mut intervals = Vec::new();
            for component in &pair_nfp.components {
                for point in &component.points {
                    let coordinate = if direction.0 == 1.0 {
                        fixed.translate_x + point.x
                    } else {
                        fixed.translate_y + point.y
                    };
                    if coordinate >= minimum && coordinate <= maximum {
                        events.extend(grid_neighbors_clamped(coordinate, minimum, maximum));
                    }
                }
                let orthogonal_outside = if direction.0 == 1.0 {
                    relative.y < component.bounds.min_y || relative.y > component.bounds.max_y
                } else {
                    relative.x < component.bounds.min_x || relative.x > component.bounds.max_x
                };
                if orthogonal_outside {
                    continue;
                }
                if let Some((start, end)) =
                    convex_line_interval(&component.points, relative, direction)
                {
                    let start = (current + start).max(minimum);
                    let end = (current + end).min(maximum);
                    if start <= end {
                        intervals.push((start, end));
                    }
                }
            }
            if events.len() > MAX_AXIS_EVENTS_PER_MOVE {
                return Ok(None);
            }
            merge_intervals(&mut intervals);
            if intervals.is_empty() {
                continue;
            }
            for (start, end) in intervals.iter().copied() {
                events.extend(grid_neighbors_clamped(start, minimum, maximum));
                events.extend(grid_neighbors_clamped(end, minimum, maximum));
                events.push(grid_predecessor_clamped(start, minimum, maximum));
                events.push(grid_successor_clamped(end, minimum, maximum));
                events.extend(grid_neighbors_clamped(
                    start + (end - start) * 0.5,
                    minimum,
                    maximum,
                ));
                if events.len() > MAX_AXIS_EVENTS_PER_MOVE {
                    return Ok(None);
                }
            }
            pair_functions.push(PairAxisIntervals {
                nfp_key,
                fixed_translate_x: fixed.translate_x,
                fixed_translate_y: fixed.translate_y,
                guided_weight: tracker.pair(input_index, fixed_index).guided_weight,
                normalization_scale: if self.uses_directional_pressure() {
                    1.0
                } else {
                    moving_diameter.max(fixed_diameter).max(0.001)
                },
                intervals,
            });
        }
        events.sort_by(f64::total_cmp);
        events.dedup_by(|first, second| grid_key(*first) == grid_key(*second));
        if events.len() > MAX_AXIS_EVENTS_PER_MOVE {
            return Ok(None);
        }
        self.counters.axis_events = self.counters.axis_events.saturating_add(events.len());
        let scored = events
            .iter()
            .copied()
            .map(|value| {
                let loss = pair_functions
                    .iter()
                    .map(|pair| {
                        pair.guided_weight * interval_penetration(value, &pair.intervals)
                            / pair.normalization_scale
                    })
                    .sum::<f64>();
                (value, loss)
            })
            .collect::<Vec<_>>();
        let mut minima = scored
            .iter()
            .enumerate()
            .filter(|(index, (_, loss))| {
                (*index == 0 || *loss <= scored[*index - 1].1)
                    && (*index + 1 == scored.len() || *loss <= scored[*index + 1].1)
            })
            .map(|(_, value)| *value)
            .collect::<Vec<_>>();
        let prefer_distance = self.uses_directional_pressure();
        minima.sort_by(|first, second| {
            compare_axis_candidate(first, second, current, prefer_distance)
        });
        minima.truncate(retained.saturating_mul(4).max(retained));
        for candidate in &mut minima {
            candidate.1 = pair_functions
                .iter()
                .map(|pair| {
                    let relative = match axis {
                        CoordinateAxis::Horizontal => IrregularPoint::new(
                            candidate.0 - pair.fixed_translate_x,
                            moving.translate_y - pair.fixed_translate_y,
                        ),
                        CoordinateAxis::Vertical => IrregularPoint::new(
                            moving.translate_x - pair.fixed_translate_x,
                            candidate.0 - pair.fixed_translate_y,
                        ),
                        CoordinateAxis::ForwardDiagonal
                        | CoordinateAxis::BackwardDiagonal
                        | CoordinateAxis::Rotation => {
                            unreachable!("axis minimization only uses cardinal axes")
                        }
                    };
                    pair.guided_weight * self.pair_directional_penetration(pair.nfp_key, relative)
                        / pair.normalization_scale
                })
                .sum::<f64>();
        }
        minima.sort_by(|first, second| {
            compare_axis_candidate(first, second, current, prefer_distance)
        });
        minima.truncate(retained);
        Ok(Some(minima.into_iter().map(|(value, _)| value).collect()))
    }

    fn pair_directional_penetration(&self, nfp_key: PairNfpKey, relative: IrregularPoint) -> f64 {
        let pair_nfp = self
            .pair_nfp_cache
            .get(&nfp_key)
            .expect("axis pair NFP is cached before scoring");
        let mut horizontal = Vec::new();
        let mut vertical = Vec::new();
        for component in &pair_nfp.components {
            if relative.y >= component.bounds.min_y && relative.y <= component.bounds.max_y {
                if let Some(interval) =
                    convex_line_interval(&component.points, relative, (1.0, 0.0))
                {
                    horizontal.push(interval);
                }
            }
            if relative.x >= component.bounds.min_x && relative.x <= component.bounds.max_x {
                if let Some(interval) =
                    convex_line_interval(&component.points, relative, (0.0, 1.0))
                {
                    vertical.push(interval);
                }
            }
        }
        merge_intervals(&mut horizontal);
        let horizontal = interval_penetration(0.0, &horizontal);
        if horizontal == 0.0 {
            return 0.0;
        }
        merge_intervals(&mut vertical);
        horizontal.min(interval_penetration(0.0, &vertical))
    }

    fn grid_directional_pair_penetration(
        &mut self,
        nfp_key: PairNfpKey,
        relative: IrregularPoint,
    ) -> GridDirectionalPenetration {
        let pair_nfp = self
            .pair_nfp_cache
            .get(&nfp_key)
            .expect("directional pair NFP is cached after preflight");
        let relative_x = grid_key(relative.x);
        let relative_y = grid_key(relative.y);
        let mut horizontal = Vec::new();
        let mut vertical = Vec::new();
        for component in &pair_nfp.components {
            if relative_y >= grid_lower_bound_key(component.bounds.min_y)
                && relative_y <= grid_upper_bound_key(component.bounds.max_y)
            {
                if let Some(interval) =
                    convex_line_interval(&component.points, relative, (1.0, 0.0))
                {
                    horizontal.push(grid_interval_bounds(interval));
                }
            }
            if relative_x >= grid_lower_bound_key(component.bounds.min_x)
                && relative_x <= grid_upper_bound_key(component.bounds.max_x)
            {
                if let Some(interval) =
                    convex_line_interval(&component.points, relative, (0.0, 1.0))
                {
                    vertical.push(grid_interval_bounds(interval));
                }
            }
        }
        let horizontal_intervals = horizontal.len();
        let vertical_intervals = vertical.len();
        let produced = horizontal_intervals.saturating_add(vertical_intervals);
        merge_grid_intervals(&mut horizontal);
        merge_grid_intervals(&mut vertical);
        let merged = horizontal.len().saturating_add(vertical.len());
        let horizontal = grid_interval_penetration(0, &horizontal);
        let vertical = grid_interval_penetration(0, &vertical);
        self.counters.directional_pair_evaluations =
            self.counters.directional_pair_evaluations.saturating_add(1);
        self.counters.directional_component_visits = self
            .counters
            .directional_component_visits
            .saturating_add(pair_nfp.components.len());
        self.counters.directional_intervals_produced = self
            .counters
            .directional_intervals_produced
            .saturating_add(produced);
        self.counters.directional_intervals_merged = self
            .counters
            .directional_intervals_merged
            .saturating_add(merged);
        if horizontal.min(vertical) == 0 {
            self.counters.directional_zero_penetration_inconsistencies = self
                .counters
                .directional_zero_penetration_inconsistencies
                .saturating_add(1);
        }
        GridDirectionalPenetration {
            horizontal_grid: horizontal,
            vertical_grid: vertical,
            horizontal_intervals,
            vertical_intervals,
        }
    }

    fn random_coordinate_axis(
        &mut self,
        step_x: f64,
        step_y: f64,
        step_limit: f64,
        rotation_step_deg: f64,
        rotation_step_limit_deg: f64,
        can_refine_rotation: bool,
    ) -> CoordinateAxis {
        // At most six candidates, chosen by index from a fixed-order list: a
        // stack array carries them exactly as the `Vec` did, without asking the
        // allocator once per refinement step.
        let mut axes = [CoordinateAxis::Horizontal; 6];
        let mut len = 0usize;
        let mut push = |axis| {
            axes[len] = axis;
            len += 1;
        };
        if step_x >= step_limit {
            push(CoordinateAxis::Horizontal);
        }
        if step_y >= step_limit {
            push(CoordinateAxis::Vertical);
        }
        if step_x >= step_limit || step_y >= step_limit {
            push(CoordinateAxis::ForwardDiagonal);
            push(CoordinateAxis::BackwardDiagonal);
        }
        if can_refine_rotation && rotation_step_deg >= rotation_step_limit_deg {
            push(CoordinateAxis::Rotation);
            push(CoordinateAxis::Rotation);
        }
        drop(push);
        axes[(self.rng.next_u64() as usize) % len]
    }

    fn random_candidate(
        &mut self,
        current: &RelaxedPlacement,
        input_index: usize,
        strip_depth_mm: f64,
        focused: Option<(f64, f64)>,
    ) -> Result<RelaxedPlacement, GeneralFastError> {
        if self.uses_directional_pressure() {
            return self.random_directional_candidate(current, strip_depth_mm, focused);
        }
        let piece = self.pieces[input_index];
        let rotation_deg = if self.relaxed_settings.angle_seed_policy
            == GeneralRelaxedAngleSeedPolicy::CurrentOnly
        {
            current.rotation_deg
        } else if piece.allow_rotation {
            if focused.is_some() {
                let sampled = current.rotation_deg + self.rng.range(-15.0, 15.0);
                self.seed_angle(sampled)
            } else {
                let sampled = self.rng.range(0.0, 360.0);
                self.seed_angle(sampled)
            }
        } else {
            current.rotation_deg
        };
        let mirrored = if piece.allow_mirror && focused.is_none() {
            self.rng.next_u64() & 1 == 1
        } else {
            current.mirrored
        };
        let bounds = self.local_shape_bounds(input_index, rotation_deg, mirrored)?;
        let inset = collision_sheet_inset_mm(self.fast_settings);
        let min_x = inset - bounds.min_x;
        let max_x = self.fast_settings.sheet_short_axis_mm - inset - bounds.max_x;
        let min_y = inset - bounds.min_y;
        let max_y = strip_depth_mm - inset - bounds.max_y;
        let (translate_x, translate_y) = if let Some((radius_x, radius_y)) = focused {
            (
                clamp_or_center(
                    current.translate_x + self.rng.range(-radius_x, radius_x),
                    min_x,
                    max_x,
                ),
                clamp_or_center(
                    current.translate_y + self.rng.range(-radius_y, radius_y),
                    min_y,
                    max_y,
                ),
            )
        } else {
            (
                sample_or_center(&mut self.rng, min_x, max_x),
                sample_or_center(&mut self.rng, min_y, max_y),
            )
        };
        Ok(RelaxedPlacement {
            input_index,
            rotation_deg,
            mirrored,
            translate_x: snap_mm(translate_x),
            translate_y: snap_mm(translate_y),
        })
    }

    fn random_directional_candidate(
        &mut self,
        current: &RelaxedPlacement,
        strip_depth_mm: f64,
        focused: Option<(f64, f64)>,
    ) -> Result<RelaxedPlacement, GeneralFastError> {
        let Some(mut inner_fit) = self.directional_inner_fit(current, strip_depth_mm)? else {
            return Err(directional_lane_unscorable_error(
                "fixed orientation has an empty inner-fit rectangle",
            ));
        };
        if let Some((radius_x, radius_y)) = focused {
            let (current_x, current_y) = self.directional_position(current)?;
            let radius_x = grid_coordinate(radius_x.abs()).ok_or_else(|| {
                GeneralFastError::InvalidInput(
                    "directional focus radius x is outside the canonical grid".to_owned(),
                )
            })?;
            let radius_y = grid_coordinate(radius_y.abs()).ok_or_else(|| {
                GeneralFastError::InvalidInput(
                    "directional focus radius y is outside the canonical grid".to_owned(),
                )
            })?;
            inner_fit.min_x = inner_fit.min_x.max(current_x.saturating_sub(radius_x));
            inner_fit.max_x = inner_fit.max_x.min(current_x.saturating_add(radius_x));
            inner_fit.min_y = inner_fit.min_y.max(current_y.saturating_sub(radius_y));
            inner_fit.max_y = inner_fit.max_y.min(current_y.saturating_add(radius_y));
        }
        let x = sample_grid_coordinate_with_rng(&mut self.rng, inner_fit.min_x, inner_fit.max_x)?;
        let y = sample_grid_coordinate_with_rng(&mut self.rng, inner_fit.min_y, inner_fit.max_y)?;
        Ok(RelaxedPlacement {
            input_index: current.input_index,
            rotation_deg: current.rotation_deg,
            mirrored: current.mirrored,
            translate_x: from_grid(x as f64),
            translate_y: from_grid(y as f64),
        })
    }

    fn score_state(&mut self, state: &RelaxedState) -> Result<PairTracker, GeneralFastError> {
        let _span = profiling::span(Phase::FullRescore);
        profiling::count(Counter::FullRescores, 1);
        if self.uses_directional_pressure() {
            return self.score_state_directional(state);
        }
        if self.uses_dynamic_hazard() {
            return self.score_state_dynamic(state);
        }
        let piece_count = state.placements.len();
        let mut collision_pairs = Vec::new();
        let mut boundaries = Vec::with_capacity(piece_count);
        // Every slot this vector declares is overwritten wholesale by the pair
        // loop below, guided weight included, so the separate `pair_weight`
        // sweep that used to run here answered `n * (n - 1) / 2` ordered-map
        // lookups whose results were then discarded.
        let mut pairs = vec![
            PairEntry {
                raw_loss: 0.0,
                guided_weight: 1.0,
                normalization_scale: 1.0,
            };
            piece_count.saturating_mul(piece_count.saturating_sub(1)) / 2
        ];
        // One shape resolution per piece rather than two per pair. The
        // catalogue is reached through a cloned handle so the resolved borrows
        // outlive the `&mut self` calls in the loop below; that is one refcount
        // bump per whole-layout score, against `n * (n - 1)` ordered-map
        // descents saved.
        let catalog = Arc::clone(&self.catalog);
        let mut shapes = Vec::with_capacity(piece_count);
        for placement in &state.placements {
            let key = self.surrogate_key(
                placement.input_index,
                placement.rotation_deg,
                placement.mirrored,
            );
            let Some(shape) = catalog.orientations.get(&key) else {
                return Err(self.missing_orientation(placement.input_index, key));
            };
            shapes.push(shape);
        }
        let mut incident_raw_loss = vec![0.0; piece_count];
        let mut boundary_violations = 0usize;
        let mut boundary_loss = 0.0;
        let mut weighted_loss = 0.0;
        for (index, placement) in state.placements.iter().enumerate() {
            let (violations, loss) = self.boundary_penalty(placement, state.strip_depth_mm)?;
            boundaries.push(BoundaryEntry {
                violations,
                raw_loss: loss,
            });
            boundary_violations = boundary_violations.saturating_add(violations);
            boundary_loss += loss;
            weighted_loss += loss;
            let first_shape = shapes[index];
            for second in (index + 1)..state.placements.len() {
                let penalty = self
                    .resolved_pair_row(
                        first_shape,
                        placement,
                        shapes[second],
                        &state.placements[second],
                    )
                    .penalty();
                let guided_weight = self.pair_weight(index, second);
                pairs[pair_slot(piece_count, index, second)] = PairEntry {
                    raw_loss: penalty,
                    guided_weight,
                    normalization_scale: 1.0,
                };
                if penalty > 0.0 {
                    incident_raw_loss[index] += penalty;
                    incident_raw_loss[second] += penalty;
                    collision_pairs.push((index, second, penalty));
                    weighted_loss += guided_weight * penalty;
                }
            }
        }
        Ok(PairTracker {
            piece_count,
            boundaries,
            pairs,
            incident_raw_loss,
            boundary_violations,
            boundary_loss,
            collision_pairs,
            weighted_loss,
        })
    }

    fn score_state_directional(
        &mut self,
        state: &RelaxedState,
    ) -> Result<PairTracker, GeneralFastError> {
        let piece_count = state.placements.len();
        let mut boundaries = Vec::with_capacity(piece_count);
        let boundary_violations = 0usize;
        let boundary_loss = 0.0;
        let mut colliding = Vec::new();
        for (index, placement) in state.placements.iter().enumerate() {
            if !self.directional_contains(placement, state.strip_depth_mm)? {
                self.counters.directional_containment_rejections = self
                    .counters
                    .directional_containment_rejections
                    .saturating_add(1);
                return Err(directional_lane_unscorable_error(
                    "initial state violates the canonical inner-fit rectangle",
                ));
            }
            boundaries.push(BoundaryEntry {
                violations: 0,
                raw_loss: 0.0,
            });
            self.counters.directional_initial_boundary_loss.observe(0.0);
            for second in (index + 1)..piece_count {
                if !self.pair_collides(placement, &state.placements[second])? {
                    continue;
                }
                let key = self.pair_nfp_key(placement, &state.placements[second])?;
                let relative =
                    self.directional_relative_point(placement, &state.placements[second])?;
                colliding.push((index, second, key, relative));
            }
        }
        let keys = colliding
            .iter()
            .map(|(_, _, key, _)| *key)
            .collect::<Vec<_>>();
        if !self.preflight_directional_pair_nfps(&keys, false)? {
            return Err(directional_lane_unscorable_error("cache budget"));
        }
        let mut pairs = vec![
            PairEntry {
                raw_loss: 0.0,
                guided_weight: 1.0,
                normalization_scale: 1.0,
            };
            piece_count.saturating_mul(piece_count.saturating_sub(1)) / 2
        ];
        for first in 0..piece_count {
            for second in (first + 1)..piece_count {
                pairs[pair_slot(piece_count, first, second)].guided_weight =
                    self.pair_weight(first, second);
            }
        }
        let mut incident_raw_loss = vec![0.0; piece_count];
        let mut collision_pairs = Vec::with_capacity(colliding.len());
        let mut weighted_loss = boundary_loss;
        for (first, second, key, relative) in colliding {
            let penetration = self.grid_directional_pair_penetration(key, relative);
            let Some(penalty) = penetration.penetration_mm() else {
                return Err(directional_lane_unscorable_error(&format!(
                    "SAT-positive pair {} / {} has zero grid penetration at ({}, {}) for key {:?}: horizontal={} across {} intervals, vertical={} across {} intervals",
                    self.pieces[first].id,
                    self.pieces[second].id,
                    relative.x,
                    relative.y,
                    key,
                    penetration.horizontal_grid,
                    penetration.horizontal_intervals,
                    penetration.vertical_grid,
                    penetration.vertical_intervals,
                )));
            };
            let guided_weight = self.pair_weight(first, second);
            pairs[pair_slot(piece_count, first, second)] = PairEntry {
                raw_loss: penalty,
                guided_weight,
                normalization_scale: 1.0,
            };
            incident_raw_loss[first] += penalty;
            incident_raw_loss[second] += penalty;
            collision_pairs.push((first, second, penalty));
            weighted_loss += guided_weight * penalty;
            self.counters.directional_initial_pair_loss.observe(penalty);
        }
        Ok(PairTracker {
            piece_count,
            boundaries,
            pairs,
            incident_raw_loss,
            boundary_violations,
            boundary_loss,
            collision_pairs,
            weighted_loss,
        })
    }

    /// The row buffer the next candidate scan fills.
    ///
    /// `Vec::new()` in the default build — literally what the scorer wrote
    /// before this lever existed — and a recycled buffer under
    /// `relaxed-row-buffer-reuse`. A recycled buffer is `clear`ed, so the scan
    /// pushes the same values in the same order into an empty vector either
    /// way; only its capacity differs, and no path reads one.
    #[inline(always)]
    fn take_row_buffer(&mut self) -> Vec<(usize, usize, f64)> {
        #[cfg(feature = "relaxed-row-buffer-reuse")]
        {
            match self.row_pool.pop() {
                Some(mut buffer) => {
                    buffer.clear();
                    buffer
                }
                None => Vec::new(),
            }
        }
        #[cfg(not(feature = "relaxed-row-buffer-reuse"))]
        {
            Vec::new()
        }
    }

    /// Hands a finished delta's rows back, instead of dropping them.
    ///
    /// The default build drops the vector here, at the same point the value
    /// would have gone out of scope anyway; the flag keeps it. A buffer that
    /// never allocated is not worth pooling, and the pool is capped so a lane
    /// cannot accumulate more spare rows than the refinement loop can hold
    /// live at once.
    #[inline(always)]
    fn recycle_row_buffer(&mut self, buffer: Vec<(usize, usize, f64)>) {
        #[cfg(feature = "relaxed-row-buffer-reuse")]
        {
            if buffer.capacity() > 0 && self.row_pool.len() < ROW_BUFFER_POOL_CAPACITY {
                self.row_pool.push(buffer);
            }
        }
        #[cfg(not(feature = "relaxed-row-buffer-reuse"))]
        {
            drop(buffer);
        }
    }

    fn score_placement(
        &mut self,
        state: &RelaxedState,
        input_index: usize,
        candidate: &RelaxedPlacement,
        piece_index: &PieceIndex,
        upper_bound: Option<f64>,
    ) -> Result<MovedRowDelta, GeneralFastError> {
        let _span = profiling::span(Phase::ScorePlacement);
        profiling::count(Counter::CandidateQueries, 1);
        if self.uses_directional_pressure() {
            return self.score_placement_directional(
                state,
                input_index,
                candidate,
                piece_index,
                upper_bound,
            );
        }
        if self.uses_dynamic_hazard() {
            return self.score_placement_dynamic(state, input_index, candidate, upper_bound);
        }
        self.counters.surrogate_evaluations += 1;
        let (boundary_violations, boundary_loss) =
            self.boundary_penalty(candidate, state.strip_depth_mm)?;
        let mut weighted_loss = boundary_loss;
        let mut collision_pairs = self.take_row_buffer();
        let mut pruned = false;
        // The scan runs over *disjoint field borrows* rather than `&mut self`.
        //
        // The candidate's shape is resolved once for the whole scan instead of
        // once per neighbour, and a colliding pair no longer resolves both
        // operands a second time to quantify its penalty: a scan of `k`
        // neighbours descended the ordered catalogue `2k + 2c` times for `c`
        // collisions and now descends it `k + 1`. Holding a resolved shape
        // across the loop means the catalogue stays borrowed, so the loop
        // cannot call `&mut self` methods - and taking a second `Arc` handle to
        // dodge that was measurably worse than the descents it saved, because
        // every lane bumps the *same* refcount and eight of them contend on one
        // cache line. Destructuring gives the same freedom for nothing.
        //
        // The `k + 1`st descent is the one `relaxed-scan-shape-reuse` removes:
        // the broad-phase probe needs the candidate's *bounds*, which live in
        // the very shape the scan is about to resolve, so the default body
        // descends for the bounds through `oriented` and then descends a second
        // time for the shape itself. Both bodies below run the same scan over
        // the same neighbours through [`scan_fixed_neighbors`]; they differ
        // only in how many times the candidate's own key is looked up.
        let failure: Option<GeneralFastError>;
        #[cfg(not(feature = "relaxed-scan-shape-reuse"))]
        {
            let probe_span = profiling::census::start(Phase::ScoreProbe);
            let shape_bounds = self
                .oriented(
                    candidate.input_index,
                    candidate.rotation_deg,
                    candidate.mirrored,
                )?
                .bounds;
            profiling::census::count(Counter::ScanCatalogDescents, 1);
            let candidate_bounds =
                translated_bounds(shape_bounds, candidate.translate_x, candidate.translate_y);
            piece_index.query_into(candidate_bounds, &mut self.piece_query_scratch);
            let mut fixed_indices = std::mem::take(&mut self.piece_query_scratch.selected);
            profiling::census::count(Counter::ScanNeighborsReturned, fixed_indices.len() as u64);
            order_scan_neighbors(
                &mut fixed_indices,
                &mut self.scan_order_scratch,
                state,
                candidate,
            );
            profiling::census::finish(Phase::ScoreProbe, probe_span);
            let LaneSearch {
                pieces,
                catalog,
                kernel,
                counters,
                angle_keys,
                weights,
                relaxed_settings,
                ..
            } = self;
            let directional = relaxed_settings.collision_backend
                == GeneralRelaxedCollisionBackend::RollbackTriangle
                && relaxed_settings.pressure_model
                    == GeneralRelaxedPressureModel::DirectionalPenetration;
            let candidate_key = (
                catalog.geometry_class_by_input[candidate.input_index],
                angle_keys.rotation_key(candidate.input_index, candidate.rotation_deg, directional),
                candidate.mirrored,
            );
            profiling::census::count(Counter::ScanCatalogDescents, 1);
            let scan_span = profiling::census::start(Phase::ScoreScan);
            failure = match catalog.orientations.get(&candidate_key) {
                None => Some(missing_orientation_error(
                    pieces,
                    candidate.input_index,
                    candidate_key,
                )),
                Some(candidate_shape) => scan_fixed_neighbors(
                    pieces,
                    catalog,
                    kernel,
                    counters,
                    angle_keys,
                    weights,
                    directional,
                    state,
                    input_index,
                    candidate,
                    candidate_shape,
                    &fixed_indices,
                    upper_bound,
                    &mut collision_pairs,
                    &mut weighted_loss,
                    &mut pruned,
                ),
            };
            profiling::census::finish(Phase::ScoreScan, scan_span);
            // Only on the path that does not raise: the borrowed buffer used to
            // be handed back after the early return above, so a raising scan
            // left the scratch empty and dropped the taken vector. That is
            // unobservable - the error ends the lane - but it is free to keep.
            if failure.is_none() {
                self.piece_query_scratch.selected = fixed_indices;
            }
        }
        #[cfg(feature = "relaxed-scan-shape-reuse")]
        {
            let LaneSearch {
                pieces,
                catalog,
                kernel,
                counters,
                angle_keys,
                weights,
                relaxed_settings,
                piece_query_scratch,
                scan_order_scratch,
                ..
            } = self;
            let directional = relaxed_settings.collision_backend
                == GeneralRelaxedCollisionBackend::RollbackTriangle
                && relaxed_settings.pressure_model
                    == GeneralRelaxedPressureModel::DirectionalPenetration;
            let candidate_key = (
                catalog.geometry_class_by_input[candidate.input_index],
                angle_keys.rotation_key(candidate.input_index, candidate.rotation_deg, directional),
                candidate.mirrored,
            );
            profiling::census::count(Counter::ScanCatalogDescents, 1);
            failure = match catalog.orientations.get(&candidate_key) {
                None => Some(missing_orientation_error(
                    pieces,
                    candidate.input_index,
                    candidate_key,
                )),
                Some(candidate_shape) => {
                    let probe_span = profiling::census::start(Phase::ScoreProbe);
                    let candidate_bounds = translated_bounds(
                        candidate_shape.bounds,
                        candidate.translate_x,
                        candidate.translate_y,
                    );
                    piece_index.query_into(candidate_bounds, piece_query_scratch);
                    profiling::census::count(
                        Counter::ScanNeighborsReturned,
                        piece_query_scratch.selected.len() as u64,
                    );
                    order_scan_neighbors(
                        &mut piece_query_scratch.selected,
                        scan_order_scratch,
                        state,
                        candidate,
                    );
                    profiling::census::finish(Phase::ScoreProbe, probe_span);
                    let scan_span = profiling::census::start(Phase::ScoreScan);
                    let outcome = scan_fixed_neighbors(
                        pieces,
                        catalog,
                        kernel,
                        counters,
                        angle_keys,
                        weights,
                        directional,
                        state,
                        input_index,
                        candidate,
                        candidate_shape,
                        &piece_query_scratch.selected,
                        upper_bound,
                        &mut collision_pairs,
                        &mut weighted_loss,
                        &mut pruned,
                    );
                    profiling::census::finish(Phase::ScoreScan, scan_span);
                    outcome
                }
            };
        }
        if let Some(error) = failure {
            return Err(error);
        }
        let finalize_span = profiling::census::start(Phase::ScoreFinalize);
        profiling::census::count(Counter::ScanCollisionRows, collision_pairs.len() as u64);
        if pruned {
            profiling::census::count(Counter::ScanUpperBoundCutoffs, 1);
        }
        collision_pairs.sort_by_key(|(first, second, _)| (*first, *second));
        profiling::census::finish(Phase::ScoreFinalize, finalize_span);
        Ok(MovedRowDelta {
            boundary_violations,
            boundary_loss,
            collision_pairs,
            weighted_loss,
            rows: moved_rows(pruned),
        })
    }

    fn score_placement_directional(
        &mut self,
        state: &RelaxedState,
        input_index: usize,
        candidate: &RelaxedPlacement,
        _piece_index: &PieceIndex,
        _upper_bound: Option<f64>,
    ) -> Result<MovedRowDelta, GeneralFastError> {
        self.counters.surrogate_evaluations = self.counters.surrogate_evaluations.saturating_add(1);
        if !self.directional_contains(candidate, state.strip_depth_mm)? {
            self.counters.directional_containment_rejections = self
                .counters
                .directional_containment_rejections
                .saturating_add(1);
            return Ok(MovedRowDelta {
                boundary_violations: 1,
                boundary_loss: 0.0,
                collision_pairs: Vec::new(),
                weighted_loss: f64::INFINITY,
                rows: MovedRows::Unscanned,
            });
        }
        let boundary_violations = 0;
        let boundary_loss = 0.0;
        let mut colliding = Vec::new();
        for fixed_index in 0..state.placements.len() {
            if fixed_index == input_index
                || !self.pair_collides(candidate, &state.placements[fixed_index])?
            {
                continue;
            }
            let fixed = &state.placements[fixed_index];
            let (canonical_first, canonical_second) = if input_index < fixed_index {
                (candidate, fixed)
            } else {
                (fixed, candidate)
            };
            let key = self.pair_nfp_key(canonical_first, canonical_second)?;
            let relative = self.directional_relative_point(canonical_first, canonical_second)?;
            colliding.push((fixed_index, key, relative));
        }
        colliding.sort_by_key(|(fixed, _, _)| *fixed);
        let keys = colliding.iter().map(|(_, key, _)| *key).collect::<Vec<_>>();
        if !self.preflight_directional_pair_nfps(&keys, true)? {
            return Ok(unscorable_directional_score(
                input_index,
                boundary_violations,
                boundary_loss,
                &colliding,
            ));
        }
        let mut weighted_loss = boundary_loss;
        let mut collision_pairs = Vec::with_capacity(colliding.len());
        for (fixed_index, key, relative) in colliding.iter().copied() {
            let Some(penalty) = self
                .grid_directional_pair_penetration(key, relative)
                .penetration_mm()
            else {
                return Ok(unscorable_directional_score(
                    input_index,
                    boundary_violations,
                    boundary_loss,
                    &colliding,
                ));
            };
            let pair = ordered_pair(input_index, fixed_index);
            collision_pairs.push((pair.0, pair.1, penalty));
            weighted_loss += self.pair_weight(pair.0, pair.1) * penalty;
        }
        collision_pairs.sort_by_key(|(first, second, _)| (*first, *second));
        Ok(MovedRowDelta {
            boundary_violations,
            boundary_loss,
            collision_pairs,
            weighted_loss,
            rows: MovedRows::Complete,
        })
    }

    fn score_state_dynamic(
        &mut self,
        state: &RelaxedState,
    ) -> Result<PairTracker, GeneralFastError> {
        #[cfg(feature = "jagua-experimental")]
        {
            let piece_count = state.placements.len();
            let mut collision_pairs = Vec::new();
            let mut boundaries = Vec::with_capacity(piece_count);
            let mut pairs = vec![
                PairEntry {
                    raw_loss: 0.0,
                    guided_weight: 1.0,
                    normalization_scale: 1.0,
                };
                piece_count.saturating_mul(piece_count.saturating_sub(1)) / 2
            ];
            let mut incident_raw_loss = vec![0.0; piece_count];
            let mut boundary_violations = 0usize;
            let mut boundary_loss = 0.0;
            let mut weighted_loss = 0.0;
            for (index, placement) in state.placements.iter().enumerate() {
                let (violations, loss) = self.boundary_penalty(placement, state.strip_depth_mm)?;
                boundaries.push(BoundaryEntry {
                    violations,
                    raw_loss: loss,
                });
                boundary_violations = boundary_violations.saturating_add(violations);
                boundary_loss += loss;
                weighted_loss += loss;
                for second in (index + 1)..piece_count {
                    let penalty =
                        self.confirmed_pair_pressure(placement, &state.placements[second])?;
                    if penalty == 0.0 {
                        continue;
                    }
                    let guided_weight = self.pair_weight(index, second);
                    pairs[pair_slot(piece_count, index, second)] = PairEntry {
                        raw_loss: penalty,
                        guided_weight,
                        normalization_scale: 1.0,
                    };
                    incident_raw_loss[index] += penalty;
                    incident_raw_loss[second] += penalty;
                    collision_pairs.push((index, second, penalty));
                    weighted_loss += guided_weight * penalty;
                }
            }
            return Ok(PairTracker {
                piece_count,
                boundaries,
                pairs,
                incident_raw_loss,
                boundary_violations,
                boundary_loss,
                collision_pairs,
                weighted_loss,
            });
        }
        #[cfg(not(feature = "jagua-experimental"))]
        {
            let _ = state;
            Err(GeneralFastError::InvalidSettings(
                "dynamic hazard search requires the jagua-experimental feature".to_owned(),
            ))
        }
    }

    fn score_placement_dynamic(
        &mut self,
        state: &RelaxedState,
        input_index: usize,
        candidate: &RelaxedPlacement,
        upper_bound: Option<f64>,
    ) -> Result<MovedRowDelta, GeneralFastError> {
        #[cfg(feature = "jagua-experimental")]
        {
            self.counters.surrogate_evaluations =
                self.counters.surrogate_evaluations.saturating_add(1);
            let (boundary_violations, boundary_loss) =
                self.boundary_penalty(candidate, state.strip_depth_mm)?;
            if self.dynamic_query_budget_exhausted() {
                return Ok(MovedRowDelta {
                    boundary_violations,
                    boundary_loss,
                    collision_pairs: Vec::new(),
                    weighted_loss: f64::INFINITY,
                    rows: MovedRows::Unscanned,
                });
            }
            let mut weighted_loss = boundary_loss;
            let mut collision_pairs = Vec::new();
            let mut pruned = false;
            let query = self
                .hazard_index
                .as_mut()
                .expect("dynamic hazard index is prepared before search")
                .query(input_index, hazard_pose(candidate), None)
                .map_err(dynamic_hazard_error)?;
            self.counters.dynamic_hazard_queries =
                self.counters.dynamic_hazard_queries.saturating_add(1);
            let GeneralHazardQuery::Complete {
                colliding_piece_ids,
                ..
            } = query
            else {
                return Err(GeneralFastError::InvalidInput(
                    "dynamic hazard placement scoring requires a complete query".to_owned(),
                ));
            };
            for fixed_index in colliding_piece_ids {
                let penalty = if self.uses_dynamic_pressure() {
                    self.hazard_index
                        .as_mut()
                        .expect("dynamic hazard index is prepared before search")
                        .collision_pressure(input_index, hazard_pose(candidate), fixed_index)
                        .map_err(dynamic_hazard_error)?
                } else {
                    self.rollback_pair_pressure(candidate, &state.placements[fixed_index])?
                };
                self.counters.dynamic_pressure_evaluations =
                    self.counters.dynamic_pressure_evaluations.saturating_add(1);
                let pair = ordered_pair(input_index, fixed_index);
                collision_pairs.push((pair.0, pair.1, penalty));
                weighted_loss += self.pair_weight(pair.0, pair.1) * penalty;
                if upper_bound.is_some_and(|upper_bound| weighted_loss > upper_bound) {
                    pruned = true;
                    break;
                }
            }
            collision_pairs.sort_by_key(|(first, second, _)| (*first, *second));
            return Ok(MovedRowDelta {
                boundary_violations,
                boundary_loss,
                collision_pairs,
                weighted_loss,
                rows: moved_rows(pruned),
            });
        }
        #[cfg(not(feature = "jagua-experimental"))]
        {
            let _ = (state, input_index, candidate, upper_bound);
            Err(GeneralFastError::InvalidSettings(
                "dynamic hazard search requires the jagua-experimental feature".to_owned(),
            ))
        }
    }

    fn confirm_dynamic_replacement(
        &mut self,
        state: &RelaxedState,
        input_index: usize,
        candidate: &RelaxedPlacement,
        search_score: &MovedRowDelta,
    ) -> Result<MovedRowDelta, GeneralFastError> {
        let (boundary_violations, boundary_loss) =
            self.boundary_penalty(candidate, state.strip_depth_mm)?;
        let mut collision_pairs = Vec::new();
        let mut weighted_loss = boundary_loss;
        for fixed_index in 0..state.placements.len() {
            if fixed_index == input_index {
                continue;
            }
            let penalty =
                self.confirmed_pair_pressure(candidate, &state.placements[fixed_index])?;
            if penalty == 0.0 {
                continue;
            }
            let pair = ordered_pair(input_index, fixed_index);
            collision_pairs.push((pair.0, pair.1, penalty));
            weighted_loss += self.pair_weight(pair.0, pair.1) * penalty;
        }
        collision_pairs.sort_by_key(|(first, second, _)| (*first, *second));
        // Both lists are sorted by `(first, second)` and carry each pair once,
        // so their symmetric difference is a linear merge. The two `BTreeSet`s
        // this replaces allocated a node per pair on every accepted move purely
        // to feed these two diagnostic counters.
        let (additions, removals) =
            sorted_pair_difference_counts(&collision_pairs, &search_score.collision_pairs);
        self.counters.retained_f64_confirmations =
            self.counters.retained_f64_confirmations.saturating_add(1);
        self.counters.confirmed_pair_additions = self
            .counters
            .confirmed_pair_additions
            .saturating_add(additions);
        self.counters.confirmed_pair_removals = self
            .counters
            .confirmed_pair_removals
            .saturating_add(removals);
        #[cfg(feature = "shadow-rescore")]
        {
            self.audit_last_confirmed_row = Some((input_index, collision_pairs.clone()));
        }
        Ok(MovedRowDelta {
            boundary_violations,
            boundary_loss,
            collision_pairs,
            weighted_loss,
            rows: MovedRows::Complete,
        })
    }

    fn confirmed_pair_pressure(
        &mut self,
        first: &RelaxedPlacement,
        second: &RelaxedPlacement,
    ) -> Result<f64, GeneralFastError> {
        // Deliberately *not* canonicalised here, even under
        // `canonical-pair-order`. This tier's verdict is already symmetric — it
        // transforms both operands into the world frame before testing, and the
        // audit measured both orders agreeing bit for bit on every pair it
        // disagreed about — so there is nothing for a swap to fix. Its
        // magnitude reaches the canonical rule through
        // [`Self::rollback_pair_pressure`], which resolves both shapes from the
        // catalogue and can therefore be asked either way round.
        //
        // Its `DynamicPoles` branch below could not be canonicalised in any
        // case: it asks the hazard index for one *explicit* pose against the
        // committed layout, so the first operand must be the piece whose pose
        // is being proposed. Swapping it would silently substitute that piece's
        // committed pose for its candidate one.
        let first_key = (
            self.catalog.geometry_class_by_input[first.input_index],
            angle_key(0.0),
            first.mirrored,
        );
        let second_key = (
            self.catalog.geometry_class_by_input[second.input_index],
            angle_key(0.0),
            second.mirrored,
        );
        let first_shape = self.catalog.orientations.get(&first_key).ok_or_else(|| {
            GeneralPolygonError::from_message("missing zero-degree confirmation surrogate")
        })?;
        let second_shape = self.catalog.orientations.get(&second_key).ok_or_else(|| {
            GeneralPolygonError::from_message("missing zero-degree confirmation surrogate")
        })?;
        // One row of proxy geometry per operand, derived once per pose rather
        // than once per pair. `bounds_for` returns exactly what the deriving
        // collider computed, so the verdict and both probe counts below are the
        // ones the uncached call produced.
        let first_bounds = self.proxy_rows.bounds_for(first_shape, first);
        let second_bounds = self.proxy_rows.bounds_for(second_shape, second);
        let (collides, cell_probes, sat_tests) = continuous_pair_collision(
            first_shape,
            first,
            first_bounds,
            second_shape,
            second,
            second_bounds,
        );
        self.counters.piece_broad_phase_probes =
            self.counters.piece_broad_phase_probes.saturating_add(1);
        self.counters.cell_index_probes =
            self.counters.cell_index_probes.saturating_add(cell_probes);
        self.counters.sat_tests = self.counters.sat_tests.saturating_add(sat_tests);
        if !collides {
            return Ok(0.0);
        }
        let pressure = match self.relaxed_settings.pressure_model {
            GeneralRelaxedPressureModel::StructuredTrianglePoles => {
                self.rollback_pair_pressure(first, second)?
            }
            GeneralRelaxedPressureModel::DirectionalPenetration => {
                return Err(GeneralFastError::InvalidSettings(
                    "directional penetration requires rollback candidate scoring".to_owned(),
                ));
            }
            GeneralRelaxedPressureModel::ContinuousTrianglePoles => {
                continuous_pole_overlap_pressure(
                    first_shape,
                    first.rotation_deg,
                    first.translate_x,
                    first.translate_y,
                    second_shape,
                    second.rotation_deg,
                    second.translate_x,
                    second.translate_y,
                )
            }
            GeneralRelaxedPressureModel::DynamicPoles => {
                #[cfg(feature = "jagua-experimental")]
                {
                    self.hazard_index
                        .as_mut()
                        .expect("dynamic hazard index is prepared before confirmation")
                        .collision_pressure(
                            first.input_index,
                            hazard_pose(first),
                            second.input_index,
                        )
                        .map_err(dynamic_hazard_error)?
                }
                #[cfg(not(feature = "jagua-experimental"))]
                {
                    return Err(GeneralFastError::InvalidSettings(
                        "dynamic pressure requires the jagua-experimental feature".to_owned(),
                    ));
                }
            }
        };
        self.counters.dynamic_pressure_evaluations =
            self.counters.dynamic_pressure_evaluations.saturating_add(1);
        Ok(pressure)
    }

    fn build_piece_index(&mut self, state: &RelaxedState) -> Result<PieceIndex, GeneralFastError> {
        let _span = profiling::span(Phase::PieceIndexBuild);
        let inset = collision_sheet_inset_mm(self.fast_settings);
        let mut index = PieceIndex::new(IrregularBounds::new(
            inset,
            inset,
            self.fast_settings.sheet_short_axis_mm - inset,
            state.strip_depth_mm - inset,
        ));
        for (piece_index, placement) in state.placements.iter().enumerate() {
            index.insert(piece_index, self.placement_bounds(placement)?);
        }
        Ok(index)
    }

    fn placement_bounds(
        &mut self,
        placement: &RelaxedPlacement,
    ) -> Result<IrregularBounds, GeneralFastError> {
        let shape_bounds = self.local_shape_bounds(
            placement.input_index,
            placement.rotation_deg,
            placement.mirrored,
        )?;
        Ok(translated_bounds(
            shape_bounds,
            placement.translate_x,
            placement.translate_y,
        ))
    }

    fn boundary_penalty(
        &mut self,
        placement: &RelaxedPlacement,
        strip_depth_mm: f64,
    ) -> Result<(usize, f64), GeneralFastError> {
        let _span = profiling::span(Phase::BoundaryPenalty);
        let bounds = self.placement_bounds(placement)?;
        let inset = collision_sheet_inset_mm(self.fast_settings);
        let overflow = [
            (inset - bounds.min_x).max(0.0),
            (bounds.max_x - (self.fast_settings.sheet_short_axis_mm - inset)).max(0.0),
            (inset - bounds.min_y).max(0.0),
            (bounds.max_y - (strip_depth_mm - inset)).max(0.0),
        ];
        let violations = overflow.iter().filter(|value| **value > 0.0).count();
        if violations == 0 {
            return Ok((0, 0.0));
        }
        let width = (bounds.max_x - bounds.min_x).max(0.0);
        let height = (bounds.max_y - bounds.min_y).max(0.0);
        let area = width * height;
        let inside_width = (bounds
            .max_x
            .min(self.fast_settings.sheet_short_axis_mm - inset)
            - bounds.min_x.max(inset))
        .max(0.0);
        let inside_height =
            (bounds.max_y.min(strip_depth_mm - inset) - bounds.min_y.max(inset)).max(0.0);
        let outside_area = (area - inside_width * inside_height).max(0.0) + area * 0.0001;
        Ok((violations, 2.0 * outside_area.sqrt() * area.sqrt()))
    }

    fn resolve_pair_nfp(
        &mut self,
        fixed: &RelaxedPlacement,
        moving: &RelaxedPlacement,
    ) -> Result<Option<&PairNfp>, GeneralFastError> {
        let key = self.pair_nfp_key(fixed, moving)?;
        if !self.pair_nfp_cache.contains_key(&key) {
            let component_count = self.pair_nfp_component_count(key)?;
            if component_count > MAX_NFP_COMPONENTS_PER_MOVE
                || self
                    .pair_nfp_cache_components
                    .saturating_add(component_count)
                    > MAX_LANE_NFP_COMPONENTS
            {
                return Ok(None);
            }
            self.build_pair_nfp(key)?;
        }
        Ok(self.pair_nfp_cache.get(&key).map(Arc::as_ref))
    }

    fn pair_nfp_component_count(&self, key: PairNfpKey) -> Result<usize, GeneralFastError> {
        let fixed = self
            .catalog
            .orientations
            .get(&(key.0, key.1, key.2))
            .ok_or_else(|| GeneralPolygonError::from_message("missing fixed NFP surrogate"))?;
        let moving = self
            .catalog
            .orientations
            .get(&(key.3, key.4, key.5))
            .ok_or_else(|| GeneralPolygonError::from_message("missing moving NFP surrogate"))?;
        Ok(fixed.cells.len().saturating_mul(moving.cells.len()))
    }

    fn build_pair_nfp(&mut self, key: PairNfpKey) -> Result<(), GeneralFastError> {
        if self.pair_nfp_cache.contains_key(&key) {
            return Ok(());
        }
        if let Some(shared) = self.catalog.shared_pair_nfps.get(&key).cloned() {
            self.pair_nfp_cache_components = self
                .pair_nfp_cache_components
                .saturating_add(shared.components.len());
            self.counters.shared_pair_nfp_adoptions =
                self.counters.shared_pair_nfp_adoptions.saturating_add(1);
            self.pair_nfp_cache.insert(key, shared);
            return Ok(());
        }
        let pair_nfp = Arc::new(build_pair_nfp_value(&self.catalog.orientations, key)?);
        self.pair_nfp_cache_components = self
            .pair_nfp_cache_components
            .saturating_add(pair_nfp.components.len());
        self.counters.pair_nfp_builds = self.counters.pair_nfp_builds.saturating_add(1);
        self.counters.pair_nfp_components = self
            .counters
            .pair_nfp_components
            .saturating_add(pair_nfp.components.len());
        self.pair_nfp_cache.insert(key, pair_nfp);
        Ok(())
    }

    fn preflight_directional_pair_nfps(
        &mut self,
        keys: &[PairNfpKey],
        enforce_candidate_limit: bool,
    ) -> Result<bool, GeneralFastError> {
        let mut keys = keys.to_vec();
        keys.sort_unstable();
        keys.dedup();
        let mut visits = 0usize;
        let mut allocations = 0usize;
        for key in keys.iter().copied() {
            let components = self.pair_nfp_component_count(key)?;
            visits = visits.saturating_add(components);
            if self.pair_nfp_cache.contains_key(&key) {
                self.counters.directional_cache_hits =
                    self.counters.directional_cache_hits.saturating_add(1);
            } else {
                self.counters.directional_cache_misses =
                    self.counters.directional_cache_misses.saturating_add(1);
                allocations = allocations.saturating_add(components);
            }
        }
        if !directional_nfp_preflight_fits(
            self.pair_nfp_cache_components,
            allocations,
            visits,
            enforce_candidate_limit,
            MAX_NFP_COMPONENTS_PER_MOVE,
            MAX_LANE_NFP_COMPONENTS,
        ) {
            self.counters.directional_over_budget_candidates = self
                .counters
                .directional_over_budget_candidates
                .saturating_add(1);
            return Ok(false);
        }
        for key in keys {
            self.build_pair_nfp(key)?;
        }
        Ok(true)
    }

    fn pair_nfp_key(
        &self,
        fixed: &RelaxedPlacement,
        moving: &RelaxedPlacement,
    ) -> Result<PairNfpKey, GeneralFastError> {
        let fixed_key =
            self.ensure_oriented(fixed.input_index, fixed.rotation_deg, fixed.mirrored)?;
        let moving_key =
            self.ensure_oriented(moving.input_index, moving.rotation_deg, moving.mirrored)?;
        Ok((
            fixed_key.0,
            fixed_key.1,
            fixed_key.2,
            moving_key.0,
            moving_key.1,
            moving_key.2,
        ))
    }

    fn pair_penalty(
        &mut self,
        first: &RelaxedPlacement,
        second: &RelaxedPlacement,
    ) -> Result<f64, GeneralFastError> {
        if !self.pair_collides(first, second)? {
            return Ok(0.0);
        }
        // Opened after the collision test so that the overwhelmingly common
        // non-colliding answer costs nothing: this phase is the quantification
        // of a collision that has already been reported.
        let _span = profiling::span(Phase::PairPressure);
        if self.uses_directional_pressure() {
            return Err(GeneralFastError::InvalidSettings(
                "directional penetration requires candidate-scoped scoring".to_owned(),
            ));
        }
        let first_key =
            self.memoised_surrogate_key(first.input_index, first.rotation_deg, first.mirrored);
        let second_key =
            self.memoised_surrogate_key(second.input_index, second.rotation_deg, second.mirrored);
        let Some(first_shape) = self.catalog.orientations.get(&first_key) else {
            return Err(self.missing_orientation(first.input_index, first_key));
        };
        let Some(second_shape) = self.catalog.orientations.get(&second_key) else {
            return Err(self.missing_orientation(second.input_index, second_key));
        };
        #[cfg(feature = "canonical-pair-order")]
        let (first_shape, first, second_shape, second) =
            canonical_pair_operands(first_shape, first, second_shape, second);
        Ok(self.kernel.pair_pressure(
            PosedShape::new(first_shape, first.translate_x, first.translate_y),
            PosedShape::new(second_shape, second.translate_x, second.translate_y),
        ))
    }

    fn pair_collides(
        &mut self,
        first: &RelaxedPlacement,
        second: &RelaxedPlacement,
    ) -> Result<bool, GeneralFastError> {
        let _span = profiling::span(Phase::PairCollide);
        profiling::count(Counter::NeighborTests, 1);
        self.counters.piece_broad_phase_probes += 1;
        // Resolve each pose once. The `ensure_oriented`/`get` pairing this
        // replaces asked the same ordered map for the same key twice per pose,
        // which on this call - the single hottest in every measured stream -
        // was four descents where two answer the question. The failure branch
        // still raises the identical error, for the same pose, in the same
        // order.
        let first_key =
            self.memoised_surrogate_key(first.input_index, first.rotation_deg, first.mirrored);
        let second_key =
            self.memoised_surrogate_key(second.input_index, second.rotation_deg, second.mirrored);
        let Some(first_shape) = self.catalog.orientations.get(&first_key) else {
            return Err(self.missing_orientation(first.input_index, first_key));
        };
        let Some(second_shape) = self.catalog.orientations.get(&second_key) else {
            return Err(self.missing_orientation(second.input_index, second_key));
        };
        if self.uses_directional_pressure() {
            let relative_x = relative_grid_coordinate(first.translate_x, second.translate_x)
                .ok_or_else(|| {
                    GeneralFastError::InvalidInput(
                        "directional horizontal translation is outside the canonical grid"
                            .to_owned(),
                    )
                })?;
            let relative_y = relative_grid_coordinate(first.translate_y, second.translate_y)
                .ok_or_else(|| {
                    GeneralFastError::InvalidInput(
                        "directional vertical translation is outside the canonical grid".to_owned(),
                    )
                })?;
            let mut sat_positive = false;
            'cells: for first_cell in &first_shape.cells {
                for second_cell in &second_shape.cells {
                    self.counters.sat_tests = self.counters.sat_tests.saturating_add(1);
                    let overlaps = triangles_overlap_on_grid(
                        *first_cell,
                        *second_cell,
                        relative_x,
                        relative_y,
                    )
                    .ok_or_else(|| {
                        GeneralFastError::InvalidInput(
                            "directional collision coordinates are outside the canonical grid"
                                .to_owned(),
                        )
                    })?;
                    if overlaps {
                        sat_positive = true;
                        break 'cells;
                    }
                }
            }
            if !sat_positive {
                return Ok(false);
            }
            let first_collision = first_shape
                .collision
                .translated(first.translate_x, first.translate_y)?;
            let second_collision = second_shape
                .collision
                .translated(second.translate_x, second.translate_y)?;
            let overlaps = polygons_overlap_exact(&first_collision, &second_collision)?;
            self.counters.directional_exact_confirmations = self
                .counters
                .directional_exact_confirmations
                .saturating_add(1);
            return Ok(overlaps);
        }
        // The default proxy verdict is the kernel's answer, not this
        // function's. `K` is `LegacyKernel` in every build that does not opt
        // into another kernel, and `LegacyKernel::pair_collides` is the loop
        // this branch used to run inline; the probe totals it reports are
        // folded back into the lane quotas below.
        Ok(kernel_pair_collides(
            &mut self.kernel,
            &mut self.counters,
            first_shape,
            first,
            second_shape,
            second,
        ))
    }

    /// The proxy collision verdict for two *already resolved* shapes.
    ///
    /// Same question, same counters, same span as [`Self::pair_collides`]; the
    /// difference is that the caller has already resolved both operands' shapes
    /// and does not want the catalogue consulted again. A whole-layout score
    /// asks about `n * (n - 1) / 2` pairs over `n` distinct shapes, so resolving
    /// per pair asked the ordered catalogue `n - 1` times for each answer it
    /// needed once.
    ///
    /// Restricted to the non-directional proxy: the directional backend answers
    /// a different question — a grid-relative SAT with an exact confirmation —
    /// and keeps its own path in [`Self::pair_collides`].
    fn resolved_pair_collides(
        &mut self,
        first_shape: &OrientedSurrogate,
        first: &RelaxedPlacement,
        second_shape: &OrientedSurrogate,
        second: &RelaxedPlacement,
    ) -> bool {
        resolved_pair_collides(
            &mut self.kernel,
            &mut self.counters,
            first_shape,
            first,
            second_shape,
            second,
        )
    }

    /// The proxy row for two already resolved shapes.
    ///
    /// The resolved counterpart of [`Self::pair_penalty`]'s non-directional
    /// branch, asked in the same order: collision first, magnitude only for a
    /// pair the proxy has already reported.
    fn resolved_pair_row(
        &mut self,
        first_shape: &OrientedSurrogate,
        first: &RelaxedPlacement,
        second_shape: &OrientedSurrogate,
        second: &RelaxedPlacement,
    ) -> PairRow {
        resolved_pair_row(
            &mut self.kernel,
            &mut self.counters,
            first_shape,
            first,
            second_shape,
            second,
        )
    }

    fn rollback_pair_pressure(
        &self,
        first: &RelaxedPlacement,
        second: &RelaxedPlacement,
    ) -> Result<f64, GeneralFastError> {
        #[cfg(feature = "canonical-pair-order")]
        let (first, second) = if first.input_index <= second.input_index {
            (first, second)
        } else {
            (second, first)
        };
        if self.uses_continuous_triangle_pressure() {
            let first_shape = self.oriented(first.input_index, 0.0, first.mirrored)?;
            let second_shape = self.oriented(second.input_index, 0.0, second.mirrored)?;
            return Ok(continuous_pole_overlap_pressure(
                first_shape,
                first.rotation_deg,
                first.translate_x,
                first.translate_y,
                second_shape,
                second.rotation_deg,
                second.translate_x,
                second.translate_y,
            ));
        }
        let first_shape =
            self.pressure_oriented(first.input_index, first.rotation_deg, first.mirrored)?;
        let second_shape =
            self.pressure_oriented(second.input_index, second.rotation_deg, second.mirrored)?;
        Ok(pole_overlap_pressure(
            first_shape,
            first.translate_x,
            first.translate_y,
            second_shape,
            second.translate_x,
            second.translate_y,
        ))
    }

    fn pressure_oriented(
        &self,
        input_index: usize,
        rotation_deg: f64,
        mirrored: bool,
    ) -> Result<&OrientedSurrogate, GeneralFastError> {
        self.oriented(input_index, rotation_deg, mirrored)
    }

    fn pair_weight(&self, first: usize, second: usize) -> f64 {
        self.weights
            .get(&ordered_pair(first, second))
            .copied()
            .unwrap_or(1.0)
    }

    fn oriented(
        &self,
        input_index: usize,
        rotation_deg: f64,
        mirrored: bool,
    ) -> Result<&OrientedSurrogate, GeneralFastError> {
        let key = self.surrogate_key(input_index, rotation_deg, mirrored);
        self.catalog
            .orientations
            .get(&key)
            .ok_or_else(|| self.missing_orientation(input_index, key))
    }

    /// The canonical surrogate key of a pose.
    ///
    /// This is pure arithmetic over the pose and the piece's geometry class -
    /// the catalog is never consulted - so it cannot fail. Splitting it out of
    /// [`Self::ensure_oriented`] is what lets the pair hot loop resolve a pose
    /// with a single ordered-map descent instead of a `contains_key` probe
    /// followed by a `get` probe for the same key.
    fn surrogate_key(&self, input_index: usize, rotation_deg: f64, mirrored: bool) -> SurrogateKey {
        (
            self.catalog.geometry_class_by_input[input_index],
            self.rotation_key(rotation_deg),
            mirrored,
        )
    }

    /// The rotation half of a [`Self::surrogate_key`].
    fn rotation_key(&self, rotation_deg: f64) -> i64 {
        derive_rotation_key(rotation_deg, self.uses_directional_pressure())
    }

    /// [`Self::surrogate_key`], answered from [`AngleKeyCache`] when the piece
    /// was last asked about the same rotation bits.
    ///
    /// The cached value is the one [`Self::rotation_key`] returned for those
    /// bits, so a hit and a miss are the same answer; only the three `fmod`s
    /// differ. `inline(always)` because it sits inside the proxy collider.
    #[inline(always)]
    fn memoised_surrogate_key(
        &mut self,
        input_index: usize,
        rotation_deg: f64,
        mirrored: bool,
    ) -> SurrogateKey {
        let directional = self.uses_directional_pressure();
        (
            self.catalog.geometry_class_by_input[input_index],
            self.angle_keys
                .rotation_key(input_index, rotation_deg, directional),
            mirrored,
        )
    }

    /// The error a missing canonical orientation raises, verbatim.
    fn missing_orientation(&self, input_index: usize, key: SurrogateKey) -> GeneralFastError {
        missing_orientation_error(self.pieces, input_index, key)
    }

    fn ensure_oriented(
        &self,
        input_index: usize,
        rotation_deg: f64,
        mirrored: bool,
    ) -> Result<SurrogateKey, GeneralFastError> {
        let key = self.surrogate_key(input_index, rotation_deg, mirrored);
        if !self.catalog.orientations.contains_key(&key) {
            return Err(self.missing_orientation(input_index, key));
        }
        Ok(key)
    }
}

fn directional_nfp_preflight_fits(
    current_components: usize,
    new_components: usize,
    candidate_visits: usize,
    enforce_candidate_limit: bool,
    candidate_limit: usize,
    lane_limit: usize,
) -> bool {
    (!enforce_candidate_limit || candidate_visits <= candidate_limit)
        && current_components.saturating_add(new_components) <= lane_limit
}

/// Builds one legacy oriented shape outside the catalogue.
///
/// The catalogue carries a per-job cell budget across every orientation it
/// builds. A caller that wants a single shape - the kernel parity harness,
/// which has to hand two kernels the same geometry - has no job to charge, so
/// this starts a fresh budget. Nothing in the search calls it, which is why it
/// is compiled only for that harness.
#[cfg(all(test, feature = "jagua-experimental"))]
pub(crate) fn oriented_surrogate_for_kernel(
    source: &PolygonSet,
    rotation_deg: f64,
    mirrored: bool,
    expansion_mm: f64,
) -> Result<OrientedSurrogate, GeneralFastError> {
    build_oriented_surrogate(
        source,
        rotation_deg,
        mirrored,
        expansion_mm,
        &mut WorkCounters::default(),
    )
}

/// Builds one [`OrientedSurrogate`]: the source ring transformed and expanded,
/// triangulated, poled, and indexed.
///
/// Extracted verbatim from the catalogue builder's inner loop so that the
/// kernel seam has one surrogate constructor rather than two. `counters`
/// carries the per-job cell budget across calls; every error message and every
/// budget check is the one the loop raised.
fn build_oriented_surrogate(
    source: &PolygonSet,
    rotation_deg: f64,
    mirrored: bool,
    expansion_mm: f64,
    counters: &mut WorkCounters,
) -> Result<OrientedSurrogate, GeneralFastError> {
    let polygon = source
        .transformed(rotation_deg, mirrored, 0.0, 0.0)?
        .offset(expansion_mm)?;
    if polygon
        .regions()
        .iter()
        .any(|region| !region.holes.is_empty())
    {
        return Err(GeneralPolygonError::from_message(
            "relaxed surrogate does not yet support offset holes",
        )
        .into());
    }
    let mut cells = Vec::new();
    for region in polygon.regions() {
        cells.extend(triangulate_ring(region.outer.points())?);
    }
    if cells.is_empty() || cells.len() > MAX_CELLS_PER_PIECE {
        return Err(GeneralPolygonError::from_message(format!(
            "relaxed surrogate cell count must be between 1 and {MAX_CELLS_PER_PIECE}"
        ))
        .into());
    }
    counters.oriented_surrogate_builds += 1;
    counters.generated_cells = counters.generated_cells.saturating_add(cells.len());
    if counters.generated_cells > MAX_CELLS_PER_JOB {
        return Err(GeneralPolygonError::from_message(format!(
            "relaxed surrogate job may contain at most {MAX_CELLS_PER_JOB} generated cells"
        ))
        .into());
    }
    let bounds = polygon
        .bounds()
        .ok_or_else(|| GeneralPolygonError::from_message("relaxed surrogate geometry is empty"))?;
    let hull_area_scale = ((bounds.max_x - bounds.min_x) * (bounds.max_y - bounds.min_y))
        .sqrt()
        .max(1.0);
    let poles = cells.iter().copied().map(triangle_pole).collect();
    let cell_axes = cells.iter().copied().map(CellAxes::new).collect();
    let cell_index = CellIndex::new(&cells, bounds);
    Ok(OrientedSurrogate {
        collision: polygon,
        cells,
        cell_axes,
        poles,
        bounds,
        cell_index,
        difficulty: hull_area_scale,
        diameter: (bounds.max_x - bounds.min_x).hypot(bounds.max_y - bounds.min_y),
    })
}

/// The legacy proxy collision verdict for two posed surrogates.
///
/// This is the body [`LaneSearch::pair_collides`] used to run inline, moved out
/// verbatim so that [`LegacyKernel`](crate::search::kernel::LegacyKernel) can
/// own it and an alternative kernel can replace it. The only change is that the
/// two quota counters are reported through [`KernelProbes`] instead of being
/// incremented on the lane directly; the caller folds the totals back, which is
/// the same arithmetic in the same order.
///
/// Broad phase is a translated-AABB reject, then a per-cell bin-mask query into
/// the second shape's cell index, then a strict triangle penetration test.
/// "Strict" matters: [`triangle_penetration`] returns `None` on exact contact,
/// so touching surrogates do not collide.
pub(crate) fn surrogate_pair_collides(
    first_shape: &OrientedSurrogate,
    first_translate_x: f64,
    first_translate_y: f64,
    second_shape: &OrientedSurrogate,
    second_translate_x: f64,
    second_translate_y: f64,
    probes: &mut KernelProbes,
) -> bool {
    let first_bounds = translated_bounds(first_shape.bounds, first_translate_x, first_translate_y);
    let second_bounds =
        translated_bounds(second_shape.bounds, second_translate_x, second_translate_y);
    if !bounds_overlap(first_bounds, second_bounds) {
        return false;
    }
    let relative_x = second_translate_x - first_translate_x;
    let relative_y = second_translate_y - first_translate_y;
    let mut cell_mask = [0_u64; MAX_CELLS_PER_PIECE / 64];
    for (first_ordinal, first_cell) in first_shape.cells.iter().enumerate() {
        let first_cell_bounds =
            translated_bounds(first_cell.bounds, first_translate_x, first_translate_y);
        probes.cell_index_probes += 1;
        // A cell that misses the whole second shape cannot meet any of its
        // cells: `second_shape.bounds` is the fold of the same ring points the
        // cells are triangulated from, so every `second_cell.bounds` is inside
        // it, and translating both by the same amount preserves that. Every
        // reported cell would therefore have failed its own extent test below,
        // so nothing downstream - not the narrow phase, not `sat_tests` - runs.
        // 39.5% of a mode-22 stream's cell probes end here.
        if !bounds_overlap(first_cell_bounds, second_bounds) {
            continue;
        }
        let words = second_shape.cell_index.query_mask_into(
            first_cell_bounds,
            second_translate_x,
            second_translate_y,
            &mut cell_mask,
        );
        for (word_index, word) in cell_mask[..words].iter_mut().enumerate() {
            while *word != 0 {
                let bit = word.trailing_zeros() as usize;
                *word &= *word - 1;
                let second_cell_index = word_index * 64 + bit;
                let second_cell = second_shape.cells[second_cell_index];
                let second_cell_bounds = translated_bounds(
                    second_cell.bounds,
                    second_translate_x,
                    second_translate_y,
                );
                if !bounds_overlap(first_cell_bounds, second_cell_bounds) {
                    continue;
                }
                probes.sat_tests += 1;
                if oriented_cells_penetrate(
                    &first_shape.cell_axes[first_ordinal],
                    second_cell,
                    relative_x,
                    relative_y,
                ) {
                    return true;
                }
            }
        }
    }
    false
}

pub(crate) fn pole_overlap_pressure(
    first_shape: &OrientedSurrogate,
    first_translate_x: f64,
    first_translate_y: f64,
    second_shape: &OrientedSurrogate,
    second_translate_x: f64,
    second_translate_y: f64,
) -> f64 {
    let epsilon =
        first_shape.diameter.max(second_shape.diameter) * OVERLAP_PROXY_EPSILON_DIAMETER_RATIO;
    let mut overlap_proxy = epsilon * epsilon;
    for first_pole in &first_shape.poles {
        let first_center = IrregularPoint::new(
            first_pole.center.x + first_translate_x,
            first_pole.center.y + first_translate_y,
        );
        for second_pole in &second_shape.poles {
            let second_center = IrregularPoint::new(
                second_pole.center.x + second_translate_x,
                second_pole.center.y + second_translate_y,
            );
            let distance = proxy_hypot(
                first_center.x - second_center.x,
                first_center.y - second_center.y,
            );
            let penetration = first_pole.radius + second_pole.radius - distance;
            let decayed = if penetration >= epsilon {
                penetration
            } else {
                epsilon * epsilon / (-penetration + 2.0 * epsilon)
            };
            overlap_proxy +=
                std::f64::consts::PI * decayed * first_pole.radius.min(second_pole.radius);
        }
    }
    overlap_proxy.sqrt() * (first_shape.difficulty * second_shape.difficulty).sqrt()
}

fn continuous_pole_overlap_pressure(
    first_shape: &OrientedSurrogate,
    first_rotation_deg: f64,
    first_translate_x: f64,
    first_translate_y: f64,
    second_shape: &OrientedSurrogate,
    second_rotation_deg: f64,
    second_translate_x: f64,
    second_translate_y: f64,
) -> f64 {
    let first_transform =
        PoleTransform::new(first_rotation_deg, first_translate_x, first_translate_y);
    let second_transform =
        PoleTransform::new(second_rotation_deg, second_translate_x, second_translate_y);
    let first_bounds = transformed_surrogate_bounds(first_shape, first_transform);
    let second_bounds = transformed_surrogate_bounds(second_shape, second_transform);
    let first_diameter =
        (first_bounds.max_x - first_bounds.min_x).hypot(first_bounds.max_y - first_bounds.min_y);
    let second_diameter = (second_bounds.max_x - second_bounds.min_x)
        .hypot(second_bounds.max_y - second_bounds.min_y);
    let epsilon = first_diameter.max(second_diameter) * OVERLAP_PROXY_EPSILON_DIAMETER_RATIO;
    let mut overlap_proxy = epsilon * epsilon;
    for first_pole in &first_shape.poles {
        let first_center = first_transform.point(first_pole.center);
        for second_pole in &second_shape.poles {
            let second_center = second_transform.point(second_pole.center);
            let distance = proxy_hypot(
                first_center.x - second_center.x,
                first_center.y - second_center.y,
            );
            let penetration = first_pole.radius + second_pole.radius - distance;
            let decayed = if penetration >= epsilon {
                penetration
            } else {
                epsilon * epsilon / (-penetration + 2.0 * epsilon)
            };
            overlap_proxy +=
                std::f64::consts::PI * decayed * first_pole.radius.min(second_pole.radius);
        }
    }
    let first_difficulty = ((first_bounds.max_x - first_bounds.min_x)
        * (first_bounds.max_y - first_bounds.min_y))
        .sqrt()
        .max(1.0);
    let second_difficulty = ((second_bounds.max_x - second_bounds.min_x)
        * (second_bounds.max_y - second_bounds.min_y))
        .sqrt()
        .max(1.0);
    overlap_proxy.sqrt() * (first_difficulty * second_difficulty).sqrt()
}

/// Reorders a candidate scan's neighbours cheapest-first, under
/// `relaxed-scan-order-proxy`.
///
/// **This is a class (B) lever: it changes which candidates the search visits.**
/// The broad-phase hands back its neighbours in ascending piece index, which is
/// an artefact of the bin walk and of [`PieceQueryScratch`]'s `sort_unstable`,
/// not a statement about which of them matter. Between 81.7% and 83.7% of scans
/// stop early on the caller's upper bound, so the index order decides *which*
/// rows land before the cutoff fires — hence `pruned`, hence [`MovedRows`],
/// hence what the tracker may install. Ordering the near neighbours first makes
/// the cutoff fire on fewer neighbours; it also changes the order the
/// `weighted_loss` sum is accumulated in, so the arms diverge in the low bits
/// of an `f64` even on scans that never prune. Neither arm is more correct than
/// the other, and no tie-break makes them agree.
///
/// The proxy is the squared distance between the two placements' translation
/// origins. It is deliberately the cheapest separation statistic available to
/// this loop: everything better — a bounds gap, an extent overlap — needs the
/// fixed operand's oriented shape, and resolving that for all 7.3 returned
/// neighbours in order to skip 3.1 of them is the cost the lever exists to
/// avoid. The origin is not the centroid, so the ordering is a heuristic on the
/// pose rather than on the geometry.
///
/// The keys are built once per neighbour into a lane scratch, so the comparator
/// does no arithmetic; the order is a strict total order — `total_cmp` on the
/// key, then the piece index — so it is deterministic despite the unstable
/// sort.
#[inline(always)]
fn order_scan_neighbors(
    fixed_indices: &mut [usize],
    scratch: &mut Vec<(f64, usize)>,
    state: &RelaxedState,
    candidate: &RelaxedPlacement,
) {
    #[cfg(feature = "relaxed-scan-order-proxy")]
    {
        if fixed_indices.len() < 2 {
            return;
        }
        scratch.clear();
        scratch.reserve(fixed_indices.len());
        for fixed_index in fixed_indices.iter().copied() {
            let fixed = &state.placements[fixed_index];
            let delta_x = fixed.translate_x - candidate.translate_x;
            let delta_y = fixed.translate_y - candidate.translate_y;
            scratch.push((delta_x * delta_x + delta_y * delta_y, fixed_index));
        }
        scratch.sort_unstable_by(|(left_key, left), (right_key, right)| {
            left_key.total_cmp(right_key).then_with(|| left.cmp(right))
        });
        for (slot, (_, fixed_index)) in fixed_indices.iter_mut().zip(scratch.iter()) {
            *slot = *fixed_index;
        }
    }
    #[cfg(not(feature = "relaxed-scan-order-proxy"))]
    {
        let _ = (fixed_indices, scratch, state, candidate);
    }
}

/// The generic scorer's neighbour scan, over the lane fields it actually needs.
///
/// This is the *whole* of what `score_placement` does per neighbour: the
/// catalogue descent for the fixed operand, the proxy row, the weighted
/// accumulation and the caller's upper-bound cutoff. It exists as one function
/// so that the two bodies of [`LaneSearch::score_placement`] — the default one
/// and the `relaxed-scan-shape-reuse` one — cannot drift: they differ in how
/// the candidate's own shape and the broad-phase probe are ordered, and in
/// nothing that touches an `f64`.
///
/// Returns the error a missing fixed orientation raises, or `None`. The scan is
/// left partial in that case, exactly as the inline loop left it.
#[allow(clippy::too_many_arguments)]
#[inline(always)]
fn scan_fixed_neighbors<K: ExplorationKernel<Shape = OrientedSurrogate>>(
    pieces: &[GeneralFastPiece<'_>],
    catalog: &SurrogateCatalog,
    kernel: &mut K,
    counters: &mut WorkCounters,
    angle_keys: &mut AngleKeyCache,
    weights: &BTreeMap<(usize, usize), f64>,
    directional: bool,
    state: &RelaxedState,
    input_index: usize,
    candidate: &RelaxedPlacement,
    candidate_shape: &OrientedSurrogate,
    fixed_indices: &[usize],
    upper_bound: Option<f64>,
    collision_pairs: &mut Vec<(usize, usize, f64)>,
    weighted_loss: &mut f64,
    pruned: &mut bool,
) -> Option<GeneralFastError> {
    for fixed_index in fixed_indices.iter().copied() {
        if fixed_index == input_index {
            continue;
        }
        profiling::census::count(Counter::ScanNeighborsVisited, 1);
        let fixed = &state.placements[fixed_index];
        let fixed_key = (
            catalog.geometry_class_by_input[fixed.input_index],
            angle_keys.rotation_key(fixed.input_index, fixed.rotation_deg, directional),
            fixed.mirrored,
        );
        profiling::census::count(Counter::ScanCatalogDescents, 1);
        let Some(fixed_shape) = catalog.orientations.get(&fixed_key) else {
            return Some(missing_orientation_error(
                pieces,
                fixed.input_index,
                fixed_key,
            ));
        };
        let penalty = resolved_pair_row(
            kernel,
            counters,
            candidate_shape,
            candidate,
            fixed_shape,
            fixed,
        )
        .penalty();
        if penalty > 0.0 {
            let pair = ordered_pair(input_index, fixed_index);
            collision_pairs.push((pair.0, pair.1, penalty));
            *weighted_loss += weights.get(&pair).copied().unwrap_or(1.0) * penalty;
            if upper_bound.is_some_and(|upper_bound| *weighted_loss > upper_bound) {
                *pruned = true;
                break;
            }
        }
    }
    None
}

/// The proxy collision verdict for two already resolved shapes, over the two
/// lane fields it actually needs.
///
/// Same span, same counters, same kernel call as the method that forwards to
/// it. It takes `kernel` and `counters` as disjoint borrows for the reason
/// [`kernel_pair_collides`] does: the shapes a resolved caller holds are
/// borrowed from the lane's catalogue, so a caller that keeps a shape alive
/// across the call cannot also hand over `&mut self`.
///
/// Only the split arm of [`resolved_pair_row`] asks the verdict as its own
/// step; the fused arm asks the kernel for the whole row at once and never
/// reaches here, which leaves this the split arm's alone and therefore dead in
/// a `fused-pair-query` build. It stays as written rather than being restated
/// inside the split body, because it is the shape the caller of a *verdict*
/// wants and the two arms are meant to differ in exactly one thing.
#[cfg_attr(feature = "fused-pair-query", allow(dead_code))]
#[inline(always)]
fn resolved_pair_collides<K: ExplorationKernel<Shape = OrientedSurrogate>>(
    kernel: &mut K,
    counters: &mut WorkCounters,
    first_shape: &OrientedSurrogate,
    first: &RelaxedPlacement,
    second_shape: &OrientedSurrogate,
    second: &RelaxedPlacement,
) -> bool {
    let _span = profiling::span(Phase::PairCollide);
    profiling::count(Counter::NeighborTests, 1);
    counters.piece_broad_phase_probes += 1;
    kernel_pair_collides(kernel, counters, first_shape, first, second_shape, second)
}

/// The proxy row for two already resolved shapes: collision first, magnitude
/// only for a pair the proxy has already reported.
///
/// Two bodies, one contract. The default one asks the kernel the two questions
/// separately, which is what every measured stream in this file was produced
/// by; `fused-pair-query` asks [`ExplorationKernel::pair_row`] once instead.
/// Both compute the same `f64` from the same operands in the same order — the
/// fused arm reproduces all four regression gates as whole documents — so the
/// feature is a *measurement* of what a second trait entry costs, not a change
/// of answer. See the PR6 entry in `docs/next-generation-engine-plan.md`.
#[cfg(not(feature = "fused-pair-query"))]
#[inline(always)]
fn resolved_pair_row<K: ExplorationKernel<Shape = OrientedSurrogate>>(
    kernel: &mut K,
    counters: &mut WorkCounters,
    first_shape: &OrientedSurrogate,
    first: &RelaxedPlacement,
    second_shape: &OrientedSurrogate,
    second: &RelaxedPlacement,
) -> PairRow {
    if !resolved_pair_collides(kernel, counters, first_shape, first, second_shape, second) {
        return PairRow::separated();
    }
    let _span = profiling::span(Phase::PairPressure);
    // The pole series is accumulated with the first operand outermost, so it is
    // order-dependent in its low bits for the same reason the collider is
    // order-dependent in its verdict. A row that is owned by the index-ordered
    // pair has to be quantified in that order too.
    #[cfg(feature = "canonical-pair-order")]
    let (first_shape, first, second_shape, second) =
        canonical_pair_operands(first_shape, first, second_shape, second);
    PairRow::colliding(kernel.pair_pressure(
        PosedShape::new(first_shape, first.translate_x, first.translate_y),
        PosedShape::new(second_shape, second.translate_x, second.translate_y),
    ))
}

/// [`resolved_pair_row`] over the fused kernel entry.
///
/// One [`ExplorationKernel::pair_row`] call carries both questions, so the
/// operands are presented once and the index-ordered swap is derived once
/// instead of twice. The two profiling phases move inside the kernel, where
/// they still wrap the same two pieces of geometric work — the split arm's
/// [`Phase::PairCollide`] span additionally encloses the lane's own counter
/// bumps, which is the one attribution difference between the arms and is worth
/// a few nanoseconds of nothing. The lane keeps those counters either way,
/// because they are the lane's quotas and not the kernel's.
#[cfg(feature = "fused-pair-query")]
#[inline(always)]
fn resolved_pair_row<K: ExplorationKernel<Shape = OrientedSurrogate>>(
    kernel: &mut K,
    counters: &mut WorkCounters,
    first_shape: &OrientedSurrogate,
    first: &RelaxedPlacement,
    second_shape: &OrientedSurrogate,
    second: &RelaxedPlacement,
) -> PairRow {
    profiling::count(Counter::NeighborTests, 1);
    counters.piece_broad_phase_probes += 1;
    #[cfg(feature = "canonical-pair-order")]
    let (first_shape, first, second_shape, second) =
        canonical_pair_operands(first_shape, first, second_shape, second);
    let mut probes = KernelProbes::default();
    let row = kernel.pair_row(
        PosedShape::new(first_shape, first.translate_x, first.translate_y),
        PosedShape::new(second_shape, second.translate_x, second.translate_y),
        &mut probes,
    );
    counters.cell_index_probes = counters
        .cell_index_probes
        .wrapping_add(probes.cell_index_probes);
    counters.sat_tests = counters.sat_tests.wrapping_add(probes.sat_tests);
    row
}

/// The two operands of a pair question, in the order the *pair* owns rather
/// than the order the caller happened to ask in.
///
/// The proxy tier is not a function of the unordered pair. Its narrow phase
/// tests the first operand's precomputed cell axes against the second operand's
/// points taken in a frame relative to the first, so swapping the operands
/// re-derives the same six separating axes through different subtractions and
/// projects them at a negated offset. The two answers agree except when a
/// contact is marginal, and there they can differ outright: on the
/// `pinned-fs-parent-164.0376` stream the pair `(33, 51)` collides asked as
/// `(33, 51)` and does not asked as `(51, 33)`.
///
/// That matters because two callers ask in two different orders. A candidate
/// scan asks `(moving, fixed)`, so its answer depends on which piece moved
/// last; a whole-layout score asks `(lower index, higher index)`, so its answer
/// depends only on the layout. Only the second can be a *measurement of the
/// layout*, which is what a tracker row has to be if a sweep is ever to inherit
/// one instead of rescoring — so the canonical owner of a row is the
/// index-ordered pair, and this is the function that enforces it.
///
/// Off by default, and it must stay off in anything that publishes a comparable
/// number: enforcing the rule changes the value a candidate scan computes for a
/// marginal pair, which moves the search's trajectory. See the row-ownership
/// entry in `docs/next-generation-engine-plan.md` for the measurement and the
/// price.
#[cfg(feature = "canonical-pair-order")]
#[inline(always)]
fn canonical_pair_operands<'a>(
    first_shape: &'a OrientedSurrogate,
    first: &'a RelaxedPlacement,
    second_shape: &'a OrientedSurrogate,
    second: &'a RelaxedPlacement,
) -> (
    &'a OrientedSurrogate,
    &'a RelaxedPlacement,
    &'a OrientedSurrogate,
    &'a RelaxedPlacement,
) {
    if first.input_index <= second.input_index {
        (first_shape, first, second_shape, second)
    } else {
        (second_shape, second, first_shape, first)
    }
}

/// Asks the kernel about one pair and folds its reported work into the lane
/// quotas.
///
/// The one place the kernel's proxy verdict is taken, so that a resolved caller
/// and a resolving one cannot drift in how they charge for it. `kernel` and
/// `counters` are passed as disjoint borrows rather than as `&mut self` because
/// the shapes a resolved caller holds are borrowed from the lane's catalogue.
///
/// `inline(always)`, matching every method on [`ExplorationKernel`]'s legacy
/// implementation, for the same reason PR3 gave: this sits on the hottest call
/// in every measured stream — about 22.8M invocations on mode 20 and 52.0M on
/// mode 22 — and factoring the body out of `pair_collides` must reproduce the
/// direct call it replaced rather than introduce one.
#[inline(always)]
fn kernel_pair_collides<K: ExplorationKernel<Shape = OrientedSurrogate>>(
    kernel: &mut K,
    counters: &mut WorkCounters,
    first_shape: &OrientedSurrogate,
    first: &RelaxedPlacement,
    second_shape: &OrientedSurrogate,
    second: &RelaxedPlacement,
) -> bool {
    #[cfg(feature = "canonical-pair-order")]
    let (first_shape, first, second_shape, second) = canonical_pair_operands(
        first_shape,
        first,
        second_shape,
        second,
    );
    let mut probes = KernelProbes::default();
    let collides = kernel.pair_collides(
        PosedShape::new(first_shape, first.translate_x, first.translate_y),
        PosedShape::new(second_shape, second.translate_x, second.translate_y),
        &mut probes,
    );
    counters.cell_index_probes = counters
        .cell_index_probes
        .wrapping_add(probes.cell_index_probes);
    counters.sat_tests = counters.sat_tests.wrapping_add(probes.sat_tests);
    collides
}

/// The confirmation collider: whether two continuously posed surrogates
/// overlap, with both broad-phase extents supplied by the caller.
///
/// The extents are the transformed cell-vertex extents of the two operands —
/// exactly what this function used to derive itself, on both operands, on every
/// call. The answer only ever depended on those extents and the cells, so
/// hoisting them to the caller changes nothing about the verdict or either
/// probe count; it only stops one piece's extent from being re-derived once per
/// neighbour it is asked about. [`ProxyRowCache`] is where they come from.
fn continuous_pair_collision(
    first_shape: &OrientedSurrogate,
    first: &RelaxedPlacement,
    first_bounds: IrregularBounds,
    second_shape: &OrientedSurrogate,
    second: &RelaxedPlacement,
    second_bounds: IrregularBounds,
) -> (bool, usize, usize) {
    if !bounds_overlap(first_bounds, second_bounds) {
        return (false, 0, 0);
    }
    let first_transform =
        PoleTransform::new(first.rotation_deg, first.translate_x, first.translate_y);
    let second_transform =
        PoleTransform::new(second.rotation_deg, second.translate_x, second.translate_y);
    let mut cell_probes = 0usize;
    let mut sat_tests = 0usize;
    for first_cell in first_shape.cells.iter().copied() {
        let first_cell = transform_triangle(first_cell, first_transform);
        for second_cell in second_shape.cells.iter().copied() {
            cell_probes = cell_probes.saturating_add(1);
            let second_cell = transform_triangle(second_cell, second_transform);
            if !bounds_overlap(first_cell.bounds, second_cell.bounds) {
                continue;
            }
            sat_tests = sat_tests.saturating_add(1);
            if triangle_penetration(first_cell, 0.0, 0.0, second_cell, 0.0, 0.0).is_some() {
                return (true, cell_probes, sat_tests);
            }
        }
    }
    (false, cell_probes, sat_tests)
}

fn transform_triangle(triangle: Triangle, transform: PoleTransform) -> Triangle {
    Triangle::new(triangle.points.map(|point| transform.point(point)))
}

#[derive(Clone, Copy)]
struct PoleTransform {
    sin: f64,
    cos: f64,
    translate_x: f64,
    translate_y: f64,
}

impl PoleTransform {
    fn new(rotation_deg: f64, translate_x: f64, translate_y: f64) -> Self {
        let (sin, cos) = continuous_angle(rotation_deg).to_radians().sin_cos();
        Self {
            sin,
            cos,
            translate_x,
            translate_y,
        }
    }

    fn point(self, point: IrregularPoint) -> IrregularPoint {
        IrregularPoint::new(
            point.x * self.cos - point.y * self.sin + self.translate_x,
            point.x * self.sin + point.y * self.cos + self.translate_y,
        )
    }
}

fn transformed_surrogate_bounds(
    shape: &OrientedSurrogate,
    transform: PoleTransform,
) -> IrregularBounds {
    let mut bounds = IrregularBounds::new(
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for point in shape
        .cells
        .iter()
        .flat_map(|triangle| triangle.points.iter().copied())
    {
        let point = transform.point(point);
        bounds.min_x = bounds.min_x.min(point.x);
        bounds.min_y = bounds.min_y.min(point.y);
        bounds.max_x = bounds.max_x.max(point.x);
        bounds.max_y = bounds.max_y.max(point.y);
    }
    bounds
}

fn build_pair_nfp_value(
    orientations: &BTreeMap<SurrogateKey, OrientedSurrogate>,
    key: PairNfpKey,
) -> Result<PairNfp, GeneralFastError> {
    let fixed_cells = &orientations
        .get(&(key.0, key.1, key.2))
        .ok_or_else(|| GeneralPolygonError::from_message("missing fixed NFP surrogate"))?
        .cells;
    let moving_cells = &orientations
        .get(&(key.3, key.4, key.5))
        .ok_or_else(|| GeneralPolygonError::from_message("missing moving NFP surrogate"))?
        .cells;
    let mut components = Vec::with_capacity(fixed_cells.len().saturating_mul(moving_cells.len()));
    for fixed_cell in fixed_cells.iter().copied() {
        for moving_cell in moving_cells.iter().copied() {
            let boundary =
                compute_relative_nfp_boundary_reference(&fixed_cell.points, &moving_cell.points)
                    .map_err(|message| {
                        GeneralPolygonError::from_message(format!(
                            "relaxed pair NFP construction failed: {message}"
                        ))
                    })?;
            let bounds = bounds_for_points(&boundary.points).ok_or_else(|| {
                GeneralPolygonError::from_message("relaxed pair NFP component is empty")
            })?;
            components.push(ConvexNfp {
                points: boundary.points,
                bounds,
            });
        }
    }
    Ok(PairNfp { components })
}

fn build_shared_pair_nfps(
    orientations: &BTreeMap<SurrogateKey, OrientedSurrogate>,
) -> Result<(BTreeMap<PairNfpKey, Arc<PairNfp>>, WorkCounters), GeneralFastError> {
    let orientation_keys = orientations.keys().copied().collect::<Vec<_>>();
    let mut keys = Vec::with_capacity(orientation_keys.len().saturating_pow(2));
    let mut component_count = 0usize;
    for fixed in orientation_keys.iter().copied() {
        for moving in orientation_keys.iter().copied() {
            let key = (fixed.0, fixed.1, fixed.2, moving.0, moving.1, moving.2);
            let fixed_cells = orientations
                .get(&fixed)
                .expect("shared NFP fixed orientation is present")
                .cells
                .len();
            let moving_cells = orientations
                .get(&moving)
                .expect("shared NFP moving orientation is present")
                .cells
                .len();
            component_count =
                component_count.saturating_add(fixed_cells.saturating_mul(moving_cells));
            keys.push(key);
        }
    }
    keys.sort_unstable();
    keys.dedup();
    let entry_bytes = std::mem::size_of::<PairNfpKey>()
        .saturating_add(std::mem::size_of::<Arc<PairNfp>>())
        .saturating_add(std::mem::size_of::<PairNfp>())
        .saturating_add(128);
    let component_bytes = std::mem::size_of::<ConvexNfp>().saturating_add(
        MAX_TRIANGLE_NFP_POINTS.saturating_mul(std::mem::size_of::<IrregularPoint>()),
    );
    let estimated_bytes = keys
        .len()
        .saturating_mul(entry_bytes)
        .saturating_add(component_count.saturating_mul(component_bytes));
    if component_count > MAX_SHARED_NFP_COMPONENTS
        || estimated_bytes > MAX_SHARED_NFP_ESTIMATED_BYTES
    {
        return Ok((BTreeMap::new(), WorkCounters::default()));
    }
    let mut table = BTreeMap::new();
    for key in keys {
        table.insert(key, Arc::new(build_pair_nfp_value(orientations, key)?));
    }
    let counters = WorkCounters {
        shared_pair_nfp_entries: table.len(),
        shared_pair_nfp_components: component_count,
        shared_pair_nfp_estimated_bytes: estimated_bytes,
        ..WorkCounters::default()
    };
    Ok((table, counters))
}

fn build_surrogate_catalog(
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    mode: SurrogateCatalogMode,
    assignment: Option<&GeneralFastResult>,
) -> Result<(Arc<SurrogateCatalog>, WorkCounters), GeneralFastError> {
    let mut catalog = BTreeMap::new();
    let mut counters = WorkCounters::default();
    let angle_count = (360.0 / SURROGATE_ANGLE_STEP_DEG).round() as usize;
    let mut representatives = Vec::<usize>::new();
    let mut geometry_class_by_input = Vec::with_capacity(pieces.len());
    for (input_index, piece) in pieces.iter().enumerate() {
        let geometry_class = representatives
            .iter()
            .position(|representative| pieces[*representative].polygon == piece.polygon)
            .unwrap_or_else(|| {
                representatives.push(input_index);
                representatives.len() - 1
            });
        geometry_class_by_input.push(geometry_class);
    }
    for (geometry_class, input_index) in representatives.into_iter().enumerate() {
        let piece = pieces[input_index];
        let class_allows_rotation = pieces.iter().enumerate().any(|(index, piece)| {
            geometry_class_by_input[index] == geometry_class && piece.allow_rotation
        });
        let class_allows_mirror = pieces.iter().enumerate().any(|(index, piece)| {
            geometry_class_by_input[index] == geometry_class && piece.allow_mirror
        });
        let mirrors: &[bool] = if class_allows_mirror {
            &[false, true]
        } else {
            &[false]
        };
        let mut poses = match mode {
            SurrogateCatalogMode::StructuredGrid => {
                let angles = if class_allows_rotation {
                    (0..angle_count)
                        .map(|index| index as f64 * SURROGATE_ANGLE_STEP_DEG)
                        .collect::<Vec<_>>()
                } else {
                    vec![0.0]
                };
                angles
                    .into_iter()
                    .flat_map(|angle| {
                        mirrors
                            .iter()
                            .copied()
                            .map(move |mirrored| (canonical_angle(angle), mirrored))
                    })
                    .collect::<Vec<_>>()
            }
            SurrogateCatalogMode::CurrentAssignment => {
                let placements = assignment
                    .expect("current-assignment catalog requires an incumbent")
                    .placements
                    .iter()
                    .map(|placement| (placement.piece_id.as_str(), placement))
                    .collect::<BTreeMap<_, _>>();
                pieces
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| geometry_class_by_input[*index] == geometry_class)
                    .map(|(_, piece)| {
                        placements
                            .get(piece.id)
                            .map(|placement| {
                                (continuous_angle(placement.rotation_deg), placement.mirrored)
                            })
                            .unwrap_or((0.0, false))
                    })
                    .collect::<Vec<_>>()
            }
            SurrogateCatalogMode::ZeroDegreeOnly => mirrors
                .iter()
                .copied()
                .map(|mirrored| (0.0, mirrored))
                .collect::<Vec<_>>(),
        };
        poses.sort_by_key(|(angle, mirrored)| (angle_key(*angle), *mirrored));
        poses.dedup_by_key(|(angle, mirrored)| (angle_key(*angle), *mirrored));
        for (angle, mirrored) in poses {
            let key = (geometry_class, angle_key(angle), mirrored);
            catalog.insert(
                key,
                build_oriented_surrogate(
                    piece.polygon,
                    angle_from_key(key.1),
                    mirrored,
                    collision_expansion_mm(settings),
                    &mut counters,
                )?,
            );
        }
    }
    let (shared_pair_nfps, shared_work) = if mode == SurrogateCatalogMode::CurrentAssignment {
        build_shared_pair_nfps(&catalog)?
    } else {
        (BTreeMap::new(), WorkCounters::default())
    };
    counters.accumulate(shared_work);
    Ok((
        Arc::new(SurrogateCatalog {
            geometry_class_by_input,
            orientations: catalog,
            shared_pair_nfps,
        }),
        counters,
    ))
}

fn initialize_complete_state(
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    collision_backend: GeneralRelaxedCollisionBackend,
    angle_seed_policy: GeneralRelaxedAngleSeedPolicy,
    pressure_model: GeneralRelaxedPressureModel,
    incumbent: &GeneralFastResult,
) -> Result<RelaxedState, GeneralFastError> {
    let by_id = incumbent
        .placements
        .iter()
        .map(|placement| (placement.piece_id.as_str(), placement))
        .collect::<BTreeMap<_, _>>();
    let inset = collision_sheet_inset_mm(settings);
    let mut shelf_y = incumbent.used_long_axis_depth_mm.max(inset);
    let mut placements = Vec::with_capacity(pieces.len());
    for (input_index, piece) in pieces.iter().enumerate() {
        if let Some(existing) = by_id.get(piece.id) {
            placements.push(RelaxedPlacement {
                input_index,
                rotation_deg: match (pressure_model, collision_backend, angle_seed_policy) {
                    (GeneralRelaxedPressureModel::DirectionalPenetration, _, _) => {
                        continuous_angle(existing.rotation_deg)
                    }
                    (
                        _,
                        GeneralRelaxedCollisionBackend::DynamicHazard,
                        GeneralRelaxedAngleSeedPolicy::ContinuousUniform,
                    ) => continuous_angle(existing.rotation_deg),
                    (
                        _,
                        GeneralRelaxedCollisionBackend::DynamicHazard,
                        GeneralRelaxedAngleSeedPolicy::CurrentOnly,
                    ) => continuous_angle(existing.rotation_deg),
                    _ => canonical_angle(existing.rotation_deg),
                },
                mirrored: existing.mirrored,
                translate_x: existing.translate_short_axis,
                translate_y: existing.translate_long_axis,
            });
            continue;
        }
        let collision = piece.polygon.offset(collision_expansion_mm(settings))?;
        let bounds = collision.bounds().ok_or_else(|| {
            GeneralPolygonError::from_message("cannot initialize empty relaxed geometry")
        })?;
        let translate_x = snap_mm(inset - bounds.min_x);
        let translate_y = snap_mm(shelf_y - bounds.min_y);
        shelf_y += bounds.max_y - bounds.min_y + settings.total_padding_mm;
        placements.push(RelaxedPlacement {
            input_index,
            rotation_deg: 0.0,
            mirrored: false,
            translate_x,
            translate_y,
        });
    }
    Ok(RelaxedState {
        placements,
        strip_depth_mm: shelf_y.max(incumbent.used_long_axis_depth_mm),
    })
}

fn disrupt_state_legacy(
    mut state: RelaxedState,
    pieces: &[GeneralFastPiece<'_>],
    seed: u64,
) -> Result<RelaxedState, GeneralFastError> {
    if state.placements.len() < 2 {
        return Ok(state);
    }
    let mut ranked = state
        .placements
        .iter()
        .enumerate()
        .map(|(state_index, placement)| {
            let bounds = pieces[placement.input_index]
                .polygon
                .bounds()
                .ok_or_else(|| {
                    GeneralPolygonError::from_message("cannot disrupt empty geometry")
                })?;
            let width = bounds.max_x - bounds.min_x;
            let height = bounds.max_y - bounds.min_y;
            Ok::<_, GeneralFastError>((state_index, width * height, width.hypot(height)))
        })
        .collect::<Result<Vec<_>, _>>()?;
    ranked.sort_by(|first, second| {
        second
            .1
            .total_cmp(&first.1)
            .then_with(|| second.2.total_cmp(&first.2))
            .then_with(|| first.0.cmp(&second.0))
    });
    let total_area = ranked.iter().map(|(_, area, _)| *area).sum::<f64>();
    let mut cumulative = 0.0;
    let mut large = Vec::new();
    for entry in ranked.iter().copied() {
        large.push(entry);
        cumulative += entry.1;
        if large.len() >= 2 && cumulative >= total_area * 0.75 {
            break;
        }
    }
    if large.len() < 2 {
        large = ranked;
    }

    let mut rng = SplitMix64::new(seed ^ 0x9FB2_1C65_1E98_DF25);
    let first_position = (rng.next_u64() as usize) % large.len();
    let first = large[first_position];
    let mut distinct = large
        .iter()
        .copied()
        .filter(|second| {
            second.0 != first.0
                && ((second.1 - first.1).abs() > first.1 * 0.01
                    || (second.2 - first.2).abs() > first.2 * 0.01)
        })
        .collect::<Vec<_>>();
    if distinct.is_empty() {
        distinct.extend(large.iter().copied().filter(|second| second.0 != first.0));
    }
    let second = distinct[(rng.next_u64() as usize) % distinct.len()];

    let first_old = state.placements[first.0].clone();
    let second_old = state.placements[second.0].clone();
    let first_piece = pieces[first_old.input_index];
    let second_piece = pieces[second_old.input_index];
    state.placements[first.0].translate_x = second_old.translate_x;
    state.placements[first.0].translate_y = second_old.translate_y;
    state.placements[first.0].rotation_deg = if first_piece.allow_rotation {
        second_old.rotation_deg
    } else {
        0.0
    };
    state.placements[first.0].mirrored = first_piece.allow_mirror && second_old.mirrored;
    state.placements[second.0].translate_x = first_old.translate_x;
    state.placements[second.0].translate_y = first_old.translate_y;
    state.placements[second.0].rotation_deg = if second_piece.allow_rotation {
        first_old.rotation_deg
    } else {
        0.0
    };
    state.placements[second.0].mirrored = second_piece.allow_mirror && first_old.mirrored;
    let mut moved = BTreeSet::new();
    relocate_contained_cluster(
        &mut state,
        pieces,
        first.0,
        second.0,
        first_old.translate_x - second_old.translate_x,
        first_old.translate_y - second_old.translate_y,
        &mut moved,
    )?;
    relocate_contained_cluster(
        &mut state,
        pieces,
        second.0,
        first.0,
        second_old.translate_x - first_old.translate_x,
        second_old.translate_y - first_old.translate_y,
        &mut moved,
    )?;
    Ok(state)
}

fn relocate_contained_cluster(
    state: &mut RelaxedState,
    pieces: &[GeneralFastPiece<'_>],
    container_index: usize,
    other_swapped_index: usize,
    delta_x: f64,
    delta_y: f64,
    already_moved: &mut BTreeSet<usize>,
) -> Result<(), GeneralFastError> {
    let container = &state.placements[container_index];
    let container_polygon = pieces[container.input_index].polygon.transformed(
        container.rotation_deg,
        container.mirrored,
        container.translate_x,
        container.translate_y,
    )?;
    let mut contained = Vec::new();
    for (index, placement) in state.placements.iter().enumerate() {
        if index == container_index
            || index == other_swapped_index
            || already_moved.contains(&index)
        {
            continue;
        }
        let polygon = pieces[placement.input_index].polygon.transformed(
            placement.rotation_deg,
            placement.mirrored,
            placement.translate_x,
            placement.translate_y,
        )?;
        let bounds = polygon.bounds().ok_or_else(|| {
            GeneralPolygonError::from_message("cannot relocate empty cluster geometry")
        })?;
        let point = IrregularPoint::new(
            (bounds.min_x + bounds.max_x) * 0.5,
            (bounds.min_y + bounds.max_y) * 0.5,
        );
        if container_polygon.contains_point(point) == PointInPolygonResult::IsInside {
            contained.push(index);
        }
    }
    for index in contained {
        state.placements[index].translate_x =
            snap_mm(state.placements[index].translate_x + delta_x);
        state.placements[index].translate_y =
            snap_mm(state.placements[index].translate_y + delta_y);
        already_moved.insert(index);
    }
    Ok(())
}

fn compression_split(seed: u64, strip_depth_mm: f64, settings: GeneralFastSettings) -> f64 {
    let inset = collision_sheet_inset_mm(settings);
    let mut rng = SplitMix64::new(seed ^ 0xD1B5_4A32_D192_ED03);
    rng.range(inset, (strip_depth_mm - inset).max(inset))
}

fn compress_state_at_split(
    state: &RelaxedState,
    target_depth_mm: f64,
    split_position_mm: f64,
    pieces: &[GeneralFastPiece<'_>],
) -> Result<RelaxedState, GeneralFastError> {
    let delta = (target_depth_mm - state.strip_depth_mm).min(0.0);
    let mut compressed = state.clone();
    compressed.strip_depth_mm = target_depth_mm;
    for placement in &mut compressed.placements {
        if !pieces[placement.input_index].allow_rotation {
            placement.rotation_deg = 0.0;
        }
        let bounds = pieces[placement.input_index]
            .polygon
            .transformed(placement.rotation_deg, placement.mirrored, 0.0, 0.0)?
            .bounds()
            .ok_or_else(|| {
                GeneralPolygonError::from_message(
                    "cannot compress relaxed state with empty source geometry",
                )
            })?;
        let centroid_y = placement.translate_y + (bounds.min_y + bounds.max_y) * 0.5;
        if centroid_y > split_position_mm {
            placement.translate_y = snap_mm(placement.translate_y + delta);
        }
    }
    Ok(compressed)
}

fn area_depth_lower_bound(
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
) -> Result<f64, GeneralFastError> {
    let expansion = collision_expansion_mm(settings);
    let area = pieces.iter().try_fold(0.0, |total, piece| {
        Ok::<_, GeneralFastError>(total + piece.polygon.offset(expansion)?.area_mm2())
    })?;
    Ok(area / collision_sheet_short_axis_mm(settings) + 2.0 * collision_sheet_inset_mm(settings))
}

fn to_fast_placements(
    state: &RelaxedState,
    pieces: &[GeneralFastPiece<'_>],
) -> Vec<GeneralFastPlacement> {
    state
        .placements
        .iter()
        .map(|placement| GeneralFastPlacement {
            piece_id: pieces[placement.input_index].id.to_owned(),
            rotation_deg: placement.rotation_deg,
            mirrored: placement.mirrored,
            translate_short_axis: placement.translate_x,
            translate_long_axis: placement.translate_y,
        })
        .collect()
}

fn triangulate_ring(points: &[IrregularPoint]) -> Result<Vec<Triangle>, GeneralFastError> {
    let mut indices = (0..points.len()).collect::<Vec<_>>();
    indices.retain(|index| {
        let previous = points[(*index + points.len() - 1) % points.len()];
        let current = points[*index];
        let next = points[(*index + 1) % points.len()];
        orientation(previous.x, previous.y, current.x, current.y, next.x, next.y) != 0
    });
    if indices.len() < 3 {
        return Err(GeneralPolygonError::from_message(
            "relaxed surrogate ring collapsed during triangulation",
        )
        .into());
    }
    let mut triangles = Vec::with_capacity(indices.len() - 2);
    while indices.len() > 3 {
        let mut ear = None;
        for position in 0..indices.len() {
            let previous = indices[(position + indices.len() - 1) % indices.len()];
            let current = indices[position];
            let next = indices[(position + 1) % indices.len()];
            let triangle_points = [points[previous], points[current], points[next]];
            if orientation(
                triangle_points[0].x,
                triangle_points[0].y,
                triangle_points[1].x,
                triangle_points[1].y,
                triangle_points[2].x,
                triangle_points[2].y,
            ) <= 0
            {
                continue;
            }
            if indices.iter().copied().any(|candidate| {
                candidate != previous
                    && candidate != current
                    && candidate != next
                    && point_in_triangle(points[candidate], triangle_points)
            }) {
                continue;
            }
            ear = Some((position, Triangle::new(triangle_points)));
            break;
        }
        let Some((position, triangle)) = ear else {
            return Err(GeneralPolygonError::from_message(
                "relaxed surrogate could not triangulate a canonical ring",
            )
            .into());
        };
        triangles.push(triangle);
        indices.remove(position);
    }
    triangles.push(Triangle::new([
        points[indices[0]],
        points[indices[1]],
        points[indices[2]],
    ]));
    Ok(triangles)
}

fn point_in_triangle(point: IrregularPoint, triangle: [IrregularPoint; 3]) -> bool {
    (0..3).all(|index| {
        let start = triangle[index];
        let end = triangle[(index + 1) % 3];
        orientation(start.x, start.y, end.x, end.y, point.x, point.y) >= 0
    })
}

/// Whether an oriented shape's cell, at a zero translation, penetrates a second
/// cell translated by `(second_x, second_y)`.
///
/// This is [`triangle_penetration`] with `first_x = first_y = 0.0` and with
/// everything that depends on the first triangle alone read out of
/// [`CellAxes`] instead of re-derived. Same axes, in the same order, from the
/// same bit patterns; the only thing dropped is the magnitude, which the proxy
/// collider has never read - both of `triangle_penetration`'s callers ask it
/// `.is_some()`.
///
/// The predicate the deriving path computes is "no edge of either triangle has
/// zero length, and every one of the six axes shows positive overlap", and it
/// short-circuits on the first edge that fails, in first-triangle-then-second
/// order. This reproduces that order exactly, so a degenerate cell answers
/// `false` at the same edge it used to answer `None` at.
///
/// `inline(always)` for the reason PR4 recorded: this is inside the hottest
/// call in every measured stream - 80.4M invocations on a mode-22 stream - and
/// a plain `#[inline]` on a factored hot helper cost that stage 0.9%.
#[inline(always)]
fn oriented_cells_penetrate(
    first: &CellAxes,
    second: Triangle,
    second_x: f64,
    second_y: f64,
) -> bool {
    let second_points = second
        .points
        .map(|point| IrregularPoint::new(point.x + second_x, point.y + second_y));
    for edge in &first.edges {
        if edge.degenerate {
            return false;
        }
        let (second_min, second_max) = project_triangle(&second_points, edge.axis_x, edge.axis_y);
        let overlap = edge.self_max.min(second_max) - edge.self_min.max(second_min);
        if overlap <= 0.0 {
            return false;
        }
    }
    for index in 0..3 {
        let edge_x = second_points[(index + 1) % 3].x - second_points[index].x;
        let edge_y = second_points[(index + 1) % 3].y - second_points[index].y;
        let length = proxy_hypot(edge_x, edge_y);
        if length == 0.0 {
            return false;
        }
        let axis_x = -edge_y / length;
        let axis_y = edge_x / length;
        let (first_min, first_max) = project_triangle(&first.points, axis_x, axis_y);
        let (second_min, second_max) = project_triangle(&second_points, axis_x, axis_y);
        let overlap = first_max.min(second_max) - first_min.max(second_min);
        if overlap <= 0.0 {
            return false;
        }
    }
    true
}

fn triangle_penetration(
    first: Triangle,
    first_x: f64,
    first_y: f64,
    second: Triangle,
    second_x: f64,
    second_y: f64,
) -> Option<f64> {
    let first_points = first
        .points
        .map(|point| IrregularPoint::new(point.x + first_x, point.y + first_y));
    let second_points = second
        .points
        .map(|point| IrregularPoint::new(point.x + second_x, point.y + second_y));
    let mut minimum = f64::INFINITY;
    for polygon in [&first_points, &second_points] {
        for index in 0..3 {
            let edge_x = polygon[(index + 1) % 3].x - polygon[index].x;
            let edge_y = polygon[(index + 1) % 3].y - polygon[index].y;
            let length = proxy_hypot(edge_x, edge_y);
            if length == 0.0 {
                return None;
            }
            let axis_x = -edge_y / length;
            let axis_y = edge_x / length;
            let (first_min, first_max) = project_triangle(&first_points, axis_x, axis_y);
            let (second_min, second_max) = project_triangle(&second_points, axis_x, axis_y);
            let overlap = first_max.min(second_max) - first_min.max(second_min);
            if overlap <= 0.0 {
                return None;
            }
            minimum = minimum.min(overlap);
        }
    }
    Some(minimum)
}

fn triangles_overlap_on_grid(
    first: Triangle,
    second: Triangle,
    relative_x: i128,
    relative_y: i128,
) -> Option<bool> {
    let first_points = grid_triangle_points(first, 0, 0)?;
    let second_points = grid_triangle_points(second, relative_x, relative_y)?;
    for polygon in [&first_points, &second_points] {
        for index in 0..3 {
            let edge_x = polygon[(index + 1) % 3].0 - polygon[index].0;
            let edge_y = polygon[(index + 1) % 3].1 - polygon[index].1;
            if edge_x == 0 && edge_y == 0 {
                return Some(false);
            }
            let axis = (-edge_y, edge_x);
            let (first_min, first_max) = project_grid_triangle(&first_points, axis);
            let (second_min, second_max) = project_grid_triangle(&second_points, axis);
            if first_max.min(second_max) <= first_min.max(second_min) {
                return Some(false);
            }
        }
    }
    Some(true)
}

fn grid_triangle_points(
    triangle: Triangle,
    translate_x: i128,
    translate_y: i128,
) -> Option<[(i128, i128); 3]> {
    let [first, second, third] = triangle.points;
    Some([
        (
            grid_coordinate(first.x)?.checked_add(translate_x)?,
            grid_coordinate(first.y)?.checked_add(translate_y)?,
        ),
        (
            grid_coordinate(second.x)?.checked_add(translate_x)?,
            grid_coordinate(second.y)?.checked_add(translate_y)?,
        ),
        (
            grid_coordinate(third.x)?.checked_add(translate_x)?,
            grid_coordinate(third.y)?.checked_add(translate_y)?,
        ),
    ])
}

fn grid_coordinate(value: f64) -> Option<i128> {
    to_grid_mm(value).map(|value| value as i128)
}

fn relative_grid_coordinate(first: f64, second: f64) -> Option<i128> {
    grid_coordinate(second)?.checked_sub(grid_coordinate(first)?)
}

fn project_grid_triangle(points: &[(i128, i128); 3], axis: (i128, i128)) -> (i128, i128) {
    let first = points[0].0 * axis.0 + points[0].1 * axis.1;
    points[1..].iter().fold((first, first), |bounds, point| {
        let projection = point.0 * axis.0 + point.1 * axis.1;
        (bounds.0.min(projection), bounds.1.max(projection))
    })
}

fn triangle_pole(triangle: Triangle) -> Pole {
    let [first, second, third] = triangle.points;
    let opposite_first = (second.x - third.x).hypot(second.y - third.y);
    let opposite_second = (first.x - third.x).hypot(first.y - third.y);
    let opposite_third = (first.x - second.x).hypot(first.y - second.y);
    let perimeter = opposite_first + opposite_second + opposite_third;
    let center = IrregularPoint::new(
        (opposite_first * first.x + opposite_second * second.x + opposite_third * third.x)
            / perimeter,
        (opposite_first * first.y + opposite_second * second.y + opposite_third * third.y)
            / perimeter,
    );
    let doubled_area = ((second.x - first.x) * (third.y - first.y)
        - (second.y - first.y) * (third.x - first.x))
        .abs();
    Pole {
        center,
        radius: doubled_area / perimeter,
    }
}

fn project_triangle(points: &[IrregularPoint; 3], axis_x: f64, axis_y: f64) -> (f64, f64) {
    points
        .iter()
        .map(|point| point.x * axis_x + point.y * axis_y)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), value| {
            (min.min(value), max.max(value))
        })
}

fn compare_lane_outcomes(
    first_ordinal: usize,
    first: &LaneOutcome,
    second_ordinal: usize,
    second: &LaneOutcome,
) -> Ordering {
    first
        .score
        .common_loss()
        .total_cmp(&second.score.common_loss())
        .then_with(|| {
            first
                .score
                .boundary_loss
                .total_cmp(&second.score.boundary_loss)
        })
        .then_with(|| {
            first
                .score
                .boundary_violations
                .cmp(&second.score.boundary_violations)
        })
        .then_with(|| {
            first
                .score
                .collision_pairs
                .len()
                .cmp(&second.score.collision_pairs.len())
        })
        .then_with(|| canonical_state_key(&first.state).cmp(&canonical_state_key(&second.state)))
        .then_with(|| first_ordinal.cmp(&second_ordinal))
}

fn compare_chain_score(first: &PairTracker, second: &PairTracker) -> Ordering {
    first
        .common_loss()
        .total_cmp(&second.common_loss())
        .then_with(|| first.weighted_loss.total_cmp(&second.weighted_loss))
        .then_with(|| first.boundary_violations.cmp(&second.boundary_violations))
        .then_with(|| {
            first
                .collision_pairs
                .len()
                .cmp(&second.collision_pairs.len())
        })
}

fn compare_ejection_candidates(first: &EjectionCandidate, second: &EjectionCandidate) -> Ordering {
    compare_chain_score(&first.score, &second.score)
        .then_with(|| ejection_candidate_key(first).cmp(&ejection_candidate_key(second)))
}

fn ejection_candidate_key(candidate: &EjectionCandidate) -> Vec<(usize, i64, bool, i64, i64)> {
    candidate
        .replacements
        .iter()
        .map(|(_, placement)| placement_key(placement))
        .collect()
}

fn same_piece_geometry(first: GeneralFastPiece<'_>, second: GeneralFastPiece<'_>) -> bool {
    let first_bounds = first
        .polygon
        .bounds()
        .expect("general pieces are non-empty");
    let second_bounds = second
        .polygon
        .bounds()
        .expect("general pieces are non-empty");
    let first_dimensions = [
        first_bounds.max_x - first_bounds.min_x,
        first_bounds.max_y - first_bounds.min_y,
    ];
    let second_dimensions = [
        second_bounds.max_x - second_bounds.min_x,
        second_bounds.max_y - second_bounds.min_y,
    ];
    let area_scale = first
        .polygon
        .area_mm2()
        .max(second.polygon.area_mm2())
        .max(1.0);
    (first.polygon.area_mm2() - second.polygon.area_mm2()).abs() <= area_scale * 0.001
        && (first_dimensions[0] - second_dimensions[0]).abs() <= 0.001
        && (first_dimensions[1] - second_dimensions[1]).abs() <= 0.001
}

/// The verdict of one shadow-rescore audit.
#[cfg(feature = "shadow-rescore")]
enum ShadowAgreement {
    /// Every row matched bit for bit. `derived_ulps` is the widest gap any
    /// running `f64` sum showed, in `f64` ulps; `0` means the whole tracker was
    /// bit-identical.
    Agrees { derived_ulps: u64 },
    /// The two trackers describe the same layout — same colliding pairs, same
    /// violation counts, same row *shape* — but at least one row's magnitude
    /// differs bitwise.
    MagnitudeOnly {
        rendered: String,
        worst_pressure_ulps: u64,
    },
    /// The two trackers disagree about the layout itself: a different set of
    /// colliding pairs, a different violation count, a different row count.
    Rows(String),
}

/// Compares a complete rescore against an incrementally maintained tracker.
///
/// The three verdicts are three different claims, and keeping them apart is the
/// point of the audit:
///
/// * **Structure** — which pairs collide, how many rows there are, how many
///   boundaries are violated. Nothing about evaluation order can change these,
///   so any difference is a delta that has lost track of the layout. This is
///   the class that must be empty.
/// * **Magnitude** — the `f64` loss on a row whose structure matched. The proxy
///   pressure kernels are not symmetric in their two operands: they accumulate
///   a pole-pair series with the first operand outermost, so reading a pair as
///   `(i, j)` and as `(j, i)` are two different summation orders over the same
///   terms. A candidate scorer always reads a pair as `(moving, fixed)` and a
///   complete score always reads it as `(lower index, higher index)`, so the
///   two paths legitimately differ in the low bits of a magnitude whenever the
///   moved piece is the higher-indexed one.
/// * **Derived sums** — the running `f64` totals. Order-dependent by
///   construction; see [`crate::search::shadow_rescore`].
///
/// `guided_weight` is deliberately not compared, for the same reason the
/// coupled rollback auditor does not compare it: it is not a measurement of the
/// layout but a copy of the guidance weight in force when the row was last
/// written, and a complete score writes it for every pair while the delta
/// writes it for the rows it touches. Every weight that reaches a score does so
/// through the weighted total, which *is* compared.
#[cfg(feature = "shadow-rescore")]
fn shadow_tracker_disagreement(shadow: &PairTracker, incremental: &PairTracker) -> ShadowAgreement {
    if shadow.piece_count != incremental.piece_count {
        return ShadowAgreement::Rows(format!(
            "piece count {} != {}",
            shadow.piece_count, incremental.piece_count
        ));
    }
    if shadow.boundaries.len() != incremental.boundaries.len() {
        return ShadowAgreement::Rows("boundary row count differs".to_owned());
    }
    if shadow.boundary_violations != incremental.boundary_violations {
        return ShadowAgreement::Rows(format!(
            "boundary violation count {} != {}",
            shadow.boundary_violations, incremental.boundary_violations
        ));
    }
    if shadow.collision_pairs.len() != incremental.collision_pairs.len() {
        return ShadowAgreement::Rows(format!(
            "collision row count {} != {}",
            shadow.collision_pairs.len(),
            incremental.collision_pairs.len()
        ));
    }
    if shadow.pairs.len() != incremental.pairs.len() {
        return ShadowAgreement::Rows("pair row count differs".to_owned());
    }
    if shadow.incident_raw_loss.len() != incremental.incident_raw_loss.len() {
        return ShadowAgreement::Rows("incident loss vector length differs".to_owned());
    }
    let mut magnitude = None::<(String, u64)>;
    let mut note_magnitude = |rendered: String, first: f64, second: f64| {
        let gap = shadow_rescore::derived_ulp_distance(first, second);
        match &mut magnitude {
            Some((_, worst)) => *worst = (*worst).max(gap),
            slot @ None => *slot = Some((rendered, gap)),
        }
    };
    for (index, (shadow, incremental)) in shadow
        .boundaries
        .iter()
        .zip(&incremental.boundaries)
        .enumerate()
    {
        if shadow.violations != incremental.violations {
            return ShadowAgreement::Rows(format!(
                "boundary row {index} violations {} != {}",
                shadow.violations, incremental.violations
            ));
        }
        if shadow.raw_loss != incremental.raw_loss {
            note_magnitude(
                format!(
                    "boundary row {index}: {:.17e} != {:.17e}",
                    shadow.raw_loss, incremental.raw_loss
                ),
                shadow.raw_loss,
                incremental.raw_loss,
            );
        }
    }
    for (index, (shadow, incremental)) in shadow
        .collision_pairs
        .iter()
        .zip(&incremental.collision_pairs)
        .enumerate()
    {
        if (shadow.0, shadow.1) != (incremental.0, incremental.1) {
            return ShadowAgreement::Rows(format!(
                "collision row {index}: pair ({}, {}) != ({}, {})",
                shadow.0, shadow.1, incremental.0, incremental.1
            ));
        }
        if shadow.2 != incremental.2 {
            note_magnitude(
                format!(
                    "collision row {index} pair ({}, {}): {:.17e} != {:.17e}",
                    shadow.0, shadow.1, shadow.2, incremental.2
                ),
                shadow.2,
                incremental.2,
            );
        }
    }
    for (slot, (shadow, incremental)) in shadow.pairs.iter().zip(&incremental.pairs).enumerate() {
        if shadow.normalization_scale != incremental.normalization_scale
            || (shadow.raw_loss > 0.0) != (incremental.raw_loss > 0.0)
        {
            return ShadowAgreement::Rows(format!(
                "pair row {slot}: {:.17e} != {:.17e}",
                shadow.raw_loss, incremental.raw_loss
            ));
        }
        if shadow.raw_loss != incremental.raw_loss {
            note_magnitude(
                format!(
                    "pair row {slot}: {:.17e} != {:.17e}",
                    shadow.raw_loss, incremental.raw_loss
                ),
                shadow.raw_loss,
                incremental.raw_loss,
            );
        }
    }
    if let Some((rendered, worst_pressure_ulps)) = magnitude {
        return ShadowAgreement::MagnitudeOnly {
            rendered,
            worst_pressure_ulps,
        };
    }
    let mut derived_ulps =
        shadow_rescore::derived_ulp_distance(shadow.boundary_loss, incremental.boundary_loss).max(
            shadow_rescore::derived_ulp_distance(shadow.weighted_loss, incremental.weighted_loss),
        );
    for (shadow, incremental) in shadow
        .incident_raw_loss
        .iter()
        .zip(&incremental.incident_raw_loss)
    {
        derived_ulps =
            derived_ulps.max(shadow_rescore::derived_ulp_distance(*shadow, *incremental));
    }
    ShadowAgreement::Agrees { derived_ulps }
}

/// Installs one accepted move into the incumbent score.
///
/// This is the delta the sweep runs on, and it is now bounded by the moved row
/// plus the incumbent's collision list rather than by the layout:
///
/// * **The moved row is walked once, as a merge.** The `fixed` loop visits
///   pairs in exactly `(first, second)` order — `(0, i), (1, i), ... (i - 1, i),
///   (i, i + 1), ... (i, n - 1)` is ascending — and `replacement.collision_pairs`
///   arrives sorted by the same key. One cursor over the row therefore answers
///   every "what is this pair's new loss" question, where a linear `find` per
///   piece used to rescan the whole row `n - 1` times.
/// * **The collision list is rebuilt by merge, not by sort.** The rows that
///   survive are the incumbent's minus the moved piece's, which is already
///   sorted, and the incoming row is sorted; merging them into the caller's
///   scratch and swapping produces exactly the order `sort_by_key` produced,
///   without the `O(m log m)` and without an allocation once the scratch has
///   grown.
///
/// The two running `f64` sums — the boundary total and the weighted total — are
/// deliberately *not* turned into subtract-and-add deltas. Their accumulation
/// order is observable in the last bit and the coupled rollback auditor compares
/// them against a from-scratch score, so the weighted total keeps being summed
/// over the whole ordered collision list exactly as it was.
fn update_score_after_move(
    score: &mut PairTracker,
    input_index: usize,
    old_boundary: (usize, f64),
    replacement: MovedRowDelta,
    weights: &BTreeMap<(usize, usize), f64>,
    merge_scratch: &mut Vec<(usize, usize, f64)>,
) {
    let _span = profiling::span(Phase::UpdateAfterMove);
    profiling::count(Counter::AcceptedMoves, 1);
    profiling::count(Counter::EffectivePieceMoves, 1);
    let tracked_boundary = score.boundaries[input_index];
    debug_assert_eq!(tracked_boundary.violations, old_boundary.0);
    debug_assert!((tracked_boundary.raw_loss - old_boundary.1).abs() <= f64::EPSILON);
    debug_assert!(
        replacement
            .collision_pairs
            .windows(2)
            .all(|window| (window[0].0, window[0].1) < (window[1].0, window[1].1)),
        "the moved row reaches the delta sorted by pair"
    );
    // Only a complete result may produce a tracker delta - the roadmap bullet
    // this type exists to satisfy. A bound-pruned row set is a partial
    // measurement of a candidate that had already lost, and installing one
    // would silently drop the partners the scan never reached. See
    // [`MovedRows::PrunedAtBound`] for why the comparators make this hold, and
    // why `Unscanned` is documented rather than asserted.
    debug_assert_ne!(
        replacement.rows,
        MovedRows::PrunedAtBound,
        "a bound-pruned row set must never be installed into the tracker"
    );
    score.replace_boundary(
        input_index,
        BoundaryEntry {
            violations: replacement.boundary_violations,
            raw_loss: replacement.boundary_loss,
        },
    );
    let mut row_cursor = 0usize;
    for fixed in 0..score.piece_count {
        if fixed == input_index {
            continue;
        }
        let pair = ordered_pair(input_index, fixed);
        while row_cursor < replacement.collision_pairs.len()
            && (
                replacement.collision_pairs[row_cursor].0,
                replacement.collision_pairs[row_cursor].1,
            ) < pair
        {
            row_cursor += 1;
        }
        let raw_loss = replacement
            .collision_pairs
            .get(row_cursor)
            .filter(|(first, second, _)| (*first, *second) == pair)
            .map(|(_, _, penalty)| *penalty)
            .unwrap_or(0.0);
        let guided_weight = weights.get(&pair).copied().unwrap_or(1.0);
        score.replace_pair(pair.0, pair.1, raw_loss, guided_weight);
    }
    score.boundary_violations = score
        .boundary_violations
        .saturating_sub(old_boundary.0)
        .saturating_add(replacement.boundary_violations);
    score.boundary_loss =
        (score.boundary_loss - old_boundary.1 + replacement.boundary_loss).max(0.0);
    merge_sorted_moved_row(
        &mut score.collision_pairs,
        input_index,
        &replacement.collision_pairs,
        merge_scratch,
    );
    score.weighted_loss = score.boundary_loss
        + score
            .collision_pairs
            .iter()
            .map(|(first, second, penalty)| {
                weights
                    .get(&ordered_pair(*first, *second))
                    .copied()
                    .unwrap_or(1.0)
                    * *penalty
            })
            .sum::<f64>();
}

/// Replaces `input_index`'s rows in a sorted collision list with `row`.
///
/// `collision_pairs` and `row` are both sorted by `(first, second)`; `row`
/// contains only pairs that mention `input_index`, and `collision_pairs` is the
/// incumbent list. Dropping the old rows preserves sortedness, and the two key
/// sets are disjoint by construction, so one linear merge reproduces the
/// previous `retain` + `extend` + `sort_by_key` order exactly rather than merely
/// up to ties — which matters, because the resulting list is compared
/// element-wise against a from-scratch score.
fn merge_sorted_moved_row(
    collision_pairs: &mut Vec<(usize, usize, f64)>,
    input_index: usize,
    row: &[(usize, usize, f64)],
    scratch: &mut Vec<(usize, usize, f64)>,
) {
    scratch.clear();
    scratch.reserve(collision_pairs.len() + row.len());
    let mut incoming = row.iter().copied().peekable();
    for retained in collision_pairs
        .iter()
        .copied()
        .filter(|(first, second, _)| *first != input_index && *second != input_index)
    {
        while incoming
            .peek()
            .is_some_and(|(first, second, _)| (*first, *second) < (retained.0, retained.1))
        {
            scratch.push(incoming.next().expect("peeked entry is present"));
        }
        scratch.push(retained);
    }
    scratch.extend(incoming);
    std::mem::swap(collision_pairs, scratch);
}

fn tracked_piece_score(
    score: &PairTracker,
    input_index: usize,
    weights: &BTreeMap<(usize, usize), f64>,
) -> MovedRowDelta {
    let boundary = score.boundaries[input_index];
    let collision_pairs = score
        .collision_pairs
        .iter()
        .filter(|(first, second, _)| *first == input_index || *second == input_index)
        .copied()
        .collect::<Vec<_>>();
    let weighted_loss = boundary.raw_loss
        + collision_pairs
            .iter()
            .map(|(first, second, penalty)| {
                weights
                    .get(&ordered_pair(*first, *second))
                    .copied()
                    .unwrap_or(1.0)
                    * *penalty
            })
            .sum::<f64>();
    MovedRowDelta {
        boundary_violations: boundary.violations,
        boundary_loss: boundary.raw_loss,
        collision_pairs,
        weighted_loss,
        rows: MovedRows::Complete,
    }
}

fn refresh_weighted_loss(score: &mut PairTracker, weights: &BTreeMap<(usize, usize), f64>) {
    for first in 0..score.piece_count {
        for second in (first + 1)..score.piece_count {
            let slot = pair_slot(score.piece_count, first, second);
            score.pairs[slot].guided_weight = weights.get(&(first, second)).copied().unwrap_or(1.0);
        }
    }
    score.weighted_loss = score.boundary_loss
        + score
            .collision_pairs
            .iter()
            .map(|(first, second, penalty)| {
                weights
                    .get(&ordered_pair(*first, *second))
                    .copied()
                    .unwrap_or(1.0)
                    * *penalty
            })
            .sum::<f64>();
}

fn compare_move_score(
    first_score: &MovedRowDelta,
    first: &RelaxedPlacement,
    second_score: &MovedRowDelta,
    second: &RelaxedPlacement,
) -> Ordering {
    first_score
        .weighted_loss
        .total_cmp(&second_score.weighted_loss)
        .then_with(|| {
            first_score
                .boundary_violations
                .cmp(&second_score.boundary_violations)
        })
        .then_with(|| {
            first_score
                .collision_pairs
                .len()
                .cmp(&second_score.collision_pairs.len())
        })
        .then_with(|| move_tie_key(first).cmp(&move_tie_key(second)))
}

fn compare_score_objective(first: &MovedRowDelta, second: &MovedRowDelta) -> Ordering {
    first
        .weighted_loss
        .total_cmp(&second.weighted_loss)
        .then_with(|| first.boundary_violations.cmp(&second.boundary_violations))
        .then_with(|| {
            first
                .collision_pairs
                .len()
                .cmp(&second.collision_pairs.len())
        })
}

fn unscorable_directional_score(
    input_index: usize,
    boundary_violations: usize,
    boundary_loss: f64,
    colliding: &[(usize, PairNfpKey, IrregularPoint)],
) -> MovedRowDelta {
    let mut collision_pairs = colliding
        .iter()
        .map(|(fixed_index, _, _)| {
            let pair = ordered_pair(input_index, *fixed_index);
            (pair.0, pair.1, 1.0)
        })
        .collect::<Vec<_>>();
    collision_pairs.sort_by_key(|(first, second, _)| (*first, *second));
    MovedRowDelta {
        boundary_violations,
        boundary_loss,
        collision_pairs,
        weighted_loss: f64::INFINITY,
        rows: MovedRows::Complete,
    }
}

fn coordinate_offsets(
    axis: CoordinateAxis,
    step_x: f64,
    step_y: f64,
    rotation_step_deg: f64,
) -> [(f64, f64, f64); 2] {
    match axis {
        CoordinateAxis::Horizontal => [(step_x, 0.0, 0.0), (-step_x, 0.0, 0.0)],
        CoordinateAxis::Vertical => [(0.0, step_y, 0.0), (0.0, -step_y, 0.0)],
        CoordinateAxis::ForwardDiagonal => [(step_x, step_y, 0.0), (-step_x, -step_y, 0.0)],
        CoordinateAxis::BackwardDiagonal => [(-step_x, step_y, 0.0), (step_x, -step_y, 0.0)],
        CoordinateAxis::Rotation => [
            (0.0, 0.0, rotation_step_deg),
            (0.0, 0.0, -rotation_step_deg),
        ],
    }
}

fn convex_line_interval(
    points: &[IrregularPoint],
    origin: IrregularPoint,
    direction: (f64, f64),
) -> Option<(f64, f64)> {
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for index in 0..points.len() {
        let start = points[index];
        let end = points[(index + 1) % points.len()];
        let edge = (end.x - start.x, end.y - start.y);
        let start_from_origin = (start.x - origin.x, start.y - origin.y);
        let denominator = cross_vectors(direction, edge);
        if denominator == 0.0 {
            if cross_vectors(start_from_origin, direction) == 0.0 {
                let first = dot_vectors(start_from_origin, direction);
                let second = dot_vectors((end.x - origin.x, end.y - origin.y), direction);
                minimum = minimum.min(first.min(second));
                maximum = maximum.max(first.max(second));
            }
            continue;
        }
        let segment_position = cross_vectors(start_from_origin, direction) / denominator;
        if !(0.0..=1.0).contains(&segment_position) {
            continue;
        }
        let line_position = cross_vectors(start_from_origin, edge) / denominator;
        minimum = minimum.min(line_position);
        maximum = maximum.max(line_position);
    }
    (minimum.is_finite() && maximum.is_finite()).then_some((minimum, maximum))
}

fn cross_vectors(first: (f64, f64), second: (f64, f64)) -> f64 {
    first.0 * second.1 - first.1 * second.0
}

fn dot_vectors(first: (f64, f64), second: (f64, f64)) -> f64 {
    first.0 * second.0 + first.1 * second.1
}

fn merge_intervals(intervals: &mut Vec<(f64, f64)>) {
    intervals.sort_by(|first, second| {
        first
            .0
            .total_cmp(&second.0)
            .then_with(|| first.1.total_cmp(&second.1))
    });
    let mut write = 0usize;
    for read in 0..intervals.len() {
        let current = intervals[read];
        if write > 0 && current.0 <= intervals[write - 1].1 {
            intervals[write - 1].1 = intervals[write - 1].1.max(current.1);
        } else {
            intervals[write] = current;
            write += 1;
        }
    }
    intervals.truncate(write);
}

fn interval_penetration(value: f64, intervals: &[(f64, f64)]) -> f64 {
    for (start, end) in intervals.iter().copied() {
        if value < start {
            break;
        }
        if value <= end {
            return (value - start).min(end - value).max(0.0);
        }
    }
    0.0
}

fn compare_axis_candidate(
    first: &(f64, f64),
    second: &(f64, f64),
    current: f64,
    prefer_distance: bool,
) -> Ordering {
    let by_loss = first.1.total_cmp(&second.1);
    let by_distance = (first.0 - current)
        .abs()
        .total_cmp(&(second.0 - current).abs());
    let by_coordinate = first.0.total_cmp(&second.0);
    if prefer_distance {
        by_loss.then(by_distance).then(by_coordinate)
    } else {
        by_loss.then(by_coordinate).then(by_distance)
    }
}

fn merge_grid_intervals(intervals: &mut Vec<(i64, i64)>) {
    for interval in intervals.iter_mut() {
        if interval.0 > interval.1 {
            *interval = (interval.1, interval.0);
        }
    }
    intervals.sort_unstable();
    let mut write = 0usize;
    for read in 0..intervals.len() {
        let current = intervals[read];
        if write > 0 && current.0 <= intervals[write - 1].1 {
            intervals[write - 1].1 = intervals[write - 1].1.max(current.1);
        } else {
            intervals[write] = current;
            write += 1;
        }
    }
    intervals.truncate(write);
}

fn grid_interval_penetration(value: i64, intervals: &[(i64, i64)]) -> i64 {
    for (start, end) in intervals.iter().copied() {
        if value < start {
            break;
        }
        if value <= end {
            return (value - start).min(end - value).max(0);
        }
    }
    0
}

fn grid_neighbors_clamped(value: f64, minimum: f64, maximum: f64) -> Vec<f64> {
    let value = value.clamp(minimum, maximum);
    let scaled = value * 1_000.0;
    [scaled.floor(), scaled.ceil()]
        .into_iter()
        .map(from_grid)
        .map(|value| value.clamp(minimum, maximum))
        .collect()
}

fn grid_predecessor_clamped(value: f64, minimum: f64, maximum: f64) -> f64 {
    from_grid((grid_lower_bound_key(value) - 1) as f64).clamp(minimum, maximum)
}

fn grid_successor_clamped(value: f64, minimum: f64, maximum: f64) -> f64 {
    from_grid((grid_upper_bound_key(value) + 1) as f64).clamp(minimum, maximum)
}

fn apply_coordinate_multiplier(
    axis: CoordinateAxis,
    step_x: &mut f64,
    step_y: &mut f64,
    rotation_step_deg: &mut f64,
    multiplier: f64,
) {
    match axis {
        CoordinateAxis::Horizontal => *step_x *= multiplier,
        CoordinateAxis::Vertical => *step_y *= multiplier,
        CoordinateAxis::ForwardDiagonal | CoordinateAxis::BackwardDiagonal => {
            let diagonal_multiplier = multiplier.sqrt();
            *step_x *= diagonal_multiplier;
            *step_y *= diagonal_multiplier;
        }
        CoordinateAxis::Rotation => *rotation_step_deg *= multiplier,
    }
}

fn even_floor(value: usize) -> usize {
    value - value % 2
}

fn move_tie_key(placement: &RelaxedPlacement) -> (i64, bool, i64, i64) {
    (
        angle_key(placement.rotation_deg),
        placement.mirrored,
        grid_key(placement.translate_x),
        grid_key(placement.translate_y),
    )
}

fn blocking_pair_diagnostics(
    pieces: &[GeneralFastPiece<'_>],
    score: &PairTracker,
    weights: &BTreeMap<(usize, usize), f64>,
) -> Vec<GeneralRelaxedPairDiagnostics> {
    let mut pairs = score
        .collision_pairs
        .iter()
        .map(|(first, second, penalty)| {
            let guided_weight = weights
                .get(&ordered_pair(*first, *second))
                .copied()
                .unwrap_or(1.0);
            GeneralRelaxedPairDiagnostics {
                first_piece_id: pieces[*first].id.to_owned(),
                second_piece_id: pieces[*second].id.to_owned(),
                raw_penalty: *penalty,
                guided_weight,
                weighted_pressure: *penalty * guided_weight,
            }
        })
        .collect::<Vec<_>>();
    pairs.sort_by(|first, second| {
        second
            .weighted_pressure
            .total_cmp(&first.weighted_pressure)
            .then_with(|| first.first_piece_id.cmp(&second.first_piece_id))
            .then_with(|| first.second_piece_id.cmp(&second.second_piece_id))
    });
    pairs.truncate(8);
    pairs
}

fn canonical_state_key(state: &RelaxedState) -> Vec<(usize, i64, bool, i64, i64)> {
    state.placements.iter().map(placement_key).collect()
}

fn report_diverse_sample(
    samples: &mut Vec<(RelaxedPlacement, MovedRowDelta)>,
    candidate: RelaxedPlacement,
    score: MovedRowDelta,
    position_threshold: f64,
) {
    let mut similar = [0usize; LOCAL_DESCENT_STARTS];
    let mut similar_count = 0usize;
    for (index, (placement, _)) in samples.iter().enumerate() {
        if placements_are_similar(placement, &candidate, position_threshold) {
            similar[similar_count] = index;
            similar_count += 1;
        }
    }
    if similar_count > 0
        && similar[..similar_count].iter().any(|index| {
            compare_move_score(&samples[*index].1, &samples[*index].0, &score, &candidate)
                != Ordering::Greater
        })
    {
        return;
    }
    for index in similar[..similar_count].iter().copied().rev() {
        samples.remove(index);
    }
    samples.push((candidate, score));
    samples.sort_by(|(first, first_score), (second, second_score)| {
        compare_move_score(first_score, first, second_score, second)
    });
    samples.truncate(LOCAL_DESCENT_STARTS);
}

fn sample_upper_bound(samples: &[(RelaxedPlacement, MovedRowDelta)]) -> Option<f64> {
    (samples.len() >= LOCAL_DESCENT_STARTS).then(|| {
        samples
            .last()
            .expect("sample capacity is non-empty")
            .1
            .weighted_loss
    })
}

fn placements_are_similar(
    first: &RelaxedPlacement,
    second: &RelaxedPlacement,
    position_threshold: f64,
) -> bool {
    (first.translate_x - second.translate_x).abs() < position_threshold
        && (first.translate_y - second.translate_y).abs() < position_threshold
        && angle_distance_deg(first.rotation_deg, second.rotation_deg) < UNIQUE_SAMPLE_ANGLE_DEG
        && first.mirrored == second.mirrored
}

fn angle_distance_deg(first: f64, second: f64) -> f64 {
    let difference = (first - second).rem_euclid(360.0);
    difference.min(360.0 - difference)
}

fn sample_or_center(rng: &mut SplitMix64, minimum: f64, maximum: f64) -> f64 {
    if minimum <= maximum {
        rng.range(minimum, maximum)
    } else {
        (minimum + maximum) * 0.5
    }
}

fn clamp_or_center(value: f64, minimum: f64, maximum: f64) -> f64 {
    if minimum <= maximum {
        value.clamp(minimum, maximum)
    } else {
        (minimum + maximum) * 0.5
    }
}

fn placement_key(placement: &RelaxedPlacement) -> (usize, i64, bool, i64, i64) {
    (
        placement.input_index,
        angle_key(placement.rotation_deg),
        placement.mirrored,
        grid_key(placement.translate_x),
        grid_key(placement.translate_y),
    )
}

/// Counts, without allocating, how many pairs appear only in `confirmed` and
/// how many only in `searched`.
///
/// Both slices must be sorted by `(first, second)` and free of duplicates,
/// which is what every producer of a [`MovedRowDelta`] guarantees: the pair
/// list is built from a deduplicated neighbour set and sorted before it is
/// returned. Under that precondition this is exactly the pair of
/// `BTreeSet::difference` counts it replaces.
fn sorted_pair_difference_counts(
    confirmed: &[(usize, usize, f64)],
    searched: &[(usize, usize, f64)],
) -> (usize, usize) {
    let (mut additions, mut removals) = (0usize, 0usize);
    let (mut left, mut right) = (0usize, 0usize);
    while left < confirmed.len() && right < searched.len() {
        let first = (confirmed[left].0, confirmed[left].1);
        let second = (searched[right].0, searched[right].1);
        match first.cmp(&second) {
            Ordering::Less => {
                additions += 1;
                left += 1;
            }
            Ordering::Greater => {
                removals += 1;
                right += 1;
            }
            Ordering::Equal => {
                left += 1;
                right += 1;
            }
        }
    }
    (
        additions + (confirmed.len() - left),
        removals + (searched.len() - right),
    )
}

fn update_weights(weights: &mut BTreeMap<(usize, usize), f64>, collisions: &[(usize, usize, f64)]) {
    let maximum = collisions
        .iter()
        .map(|(_, _, penalty)| *penalty)
        .max_by(f64::total_cmp)
        .unwrap_or(0.0);
    let active = collisions
        .iter()
        .map(|(first, second, _)| ordered_pair(*first, *second))
        .collect::<BTreeSet<_>>();
    for (pair, weight) in weights.iter_mut() {
        if !active.contains(pair) {
            *weight = (*weight * 0.95).max(1.0);
        }
    }
    if maximum > 0.0 {
        for (first, second, penalty) in collisions {
            let multiplier = 1.02 + 0.08 * (*penalty / maximum);
            *weights.entry(ordered_pair(*first, *second)).or_insert(1.0) *= multiplier;
        }
    }
}

fn repair_active_indices(
    state: &RelaxedState,
    score: &PairTracker,
    pieces: &[GeneralFastPiece<'_>],
    weights: &BTreeMap<(usize, usize), f64>,
    neighborhood_size: usize,
) -> Vec<usize> {
    let neighborhood_size = neighborhood_size.clamp(2, pieces.len());
    let mut selected = Vec::with_capacity(neighborhood_size);
    if let Some((first, second, _)) = score.collision_pairs.iter().max_by(|first, second| {
        let first_weighted = first.2
            * weights
                .get(&ordered_pair(first.0, first.1))
                .copied()
                .unwrap_or(1.0);
        let second_weighted = second.2
            * weights
                .get(&ordered_pair(second.0, second.1))
                .copied()
                .unwrap_or(1.0);
        first_weighted
            .total_cmp(&second_weighted)
            .then_with(|| ordered_pair(second.0, second.1).cmp(&ordered_pair(first.0, first.1)))
    }) {
        selected.push(*first);
        selected.push(*second);
    }
    for blocker in high_frontier_blockers(state, pieces, neighborhood_size) {
        if selected.len() >= neighborhood_size {
            break;
        }
        if !selected.contains(&blocker) {
            selected.push(blocker);
        }
    }
    selected.sort_by(|first, second| {
        pieces[*second]
            .polygon
            .area_mm2()
            .total_cmp(&pieces[*first].polygon.area_mm2())
            .then_with(|| pieces[*first].id.cmp(pieces[*second].id))
    });
    selected
}

fn repair_angles(piece: GeneralFastPiece<'_>, current: &RelaxedPlacement) -> Vec<f64> {
    if !piece.allow_rotation {
        return vec![current.rotation_deg];
    }
    let mut edges = piece
        .polygon
        .regions()
        .iter()
        .flat_map(|region| {
            let points = region.outer.points();
            (0..points.len()).map(move |index| {
                let mut start = points[index];
                let mut end = points[(index + 1) % points.len()];
                if current.mirrored {
                    start.x = -start.x;
                    end.x = -end.x;
                }
                let delta_x = end.x - start.x;
                let delta_y = end.y - start.y;
                (delta_x.hypot(delta_y), delta_y.atan2(delta_x).to_degrees())
            })
        })
        .filter(|(length, _)| *length > 0.001)
        .collect::<Vec<_>>();
    edges.sort_by(|first, second| {
        second
            .0
            .total_cmp(&first.0)
            .then_with(|| first.1.total_cmp(&second.1))
    });
    let mut directions = Vec::with_capacity(2);
    for (_, direction) in edges {
        if directions.iter().all(|selected: &f64| {
            let difference = (direction - *selected).to_radians().sin().abs();
            difference > 0.1
        }) {
            directions.push(direction);
        }
        if directions.len() == 2 {
            break;
        }
    }
    let mut angles = vec![continuous_angle(current.rotation_deg)];
    for direction in directions {
        angles.push(continuous_angle(-direction));
        angles.push(continuous_angle(90.0 - direction));
    }
    angles.sort_by_key(|angle| angle_key(*angle));
    angles.dedup_by_key(|angle| angle_key(*angle));
    if let Some(position) = angles
        .iter()
        .position(|angle| angle_key(*angle) == angle_key(current.rotation_deg))
    {
        angles.swap(0, position);
    }
    angles
}

fn high_frontier_blockers(
    state: &RelaxedState,
    pieces: &[GeneralFastPiece<'_>],
    count: usize,
) -> Vec<usize> {
    let mut ranked = state
        .placements
        .iter()
        .map(|placement| {
            (
                placement.input_index,
                transformed_source_max_y(pieces[placement.input_index], placement),
                pieces[placement.input_index].id,
            )
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|first, second| {
        second
            .1
            .total_cmp(&first.1)
            .then_with(|| first.2.cmp(second.2))
    });
    ranked
        .into_iter()
        .take(count)
        .map(|(index, _, _)| index)
        .collect()
}

fn legacy_forced_blockers(
    state: &RelaxedState,
    pieces: &[GeneralFastPiece<'_>],
    count: usize,
) -> Vec<usize> {
    let mut ranked = state
        .placements
        .iter()
        .map(|placement| {
            let bounds = pieces[placement.input_index]
                .polygon
                .bounds()
                .expect("general pieces are non-empty");
            (
                placement.input_index,
                placement.translate_y + bounds.max_y,
                pieces[placement.input_index].id,
            )
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|first, second| {
        second
            .1
            .total_cmp(&first.1)
            .then_with(|| first.2.cmp(second.2))
    });
    ranked
        .into_iter()
        .take(count)
        .map(|(index, _, _)| index)
        .collect()
}

fn transformed_source_max_y(piece: GeneralFastPiece<'_>, placement: &RelaxedPlacement) -> f64 {
    let radians = placement.rotation_deg.to_radians();
    let sine = radians.sin();
    let cosine = radians.cos();
    piece
        .polygon
        .regions()
        .iter()
        .flat_map(|region| std::iter::once(&region.outer).chain(region.holes.iter()))
        .flat_map(|ring| ring.points())
        .map(|point| {
            let local_x = if placement.mirrored {
                -point.x
            } else {
                point.x
            };
            placement.translate_y + local_x * sine + point.y * cosine
        })
        .max_by(f64::total_cmp)
        .expect("general pieces are non-empty")
}

fn validate_relaxed_settings(settings: GeneralRelaxedSettings) -> Result<(), GeneralFastError> {
    if settings.epochs == 0
        || settings.lanes == 0
        || settings.sweeps_per_epoch == 0
        || settings.global_samples_per_move == 0
        || settings.focused_samples_per_move == 0
        || settings.refinement_rounds == 0
        || !settings.initial_shrink_ratio.is_finite()
        || !settings.minimum_shrink_ratio.is_finite()
        || settings.initial_shrink_ratio <= 0.0
        || settings.initial_shrink_ratio >= 1.0
        || settings.minimum_shrink_ratio <= 0.0
        || settings.minimum_shrink_ratio > settings.initial_shrink_ratio
    {
        return Err(GeneralFastError::InvalidSettings(
            "relaxed-search quotas and shrink ratios must be positive and bounded".to_owned(),
        ));
    }
    if settings.collision_backend == GeneralRelaxedCollisionBackend::RollbackTriangle
        && !matches!(
            settings.pressure_model,
            GeneralRelaxedPressureModel::StructuredTrianglePoles
                | GeneralRelaxedPressureModel::DirectionalPenetration
        )
    {
        return Err(GeneralFastError::InvalidSettings(
            "the rollback triangle backend requires structured or directional pressure".to_owned(),
        ));
    }
    if settings.pressure_model == GeneralRelaxedPressureModel::DirectionalPenetration
        && settings.synchronize_lanes
    {
        return Err(GeneralFastError::InvalidSettings(
            "directional penetration does not support synchronized lanes".to_owned(),
        ));
    }
    if settings.collision_backend == GeneralRelaxedCollisionBackend::DynamicHazard
        && settings.pressure_model == GeneralRelaxedPressureModel::DirectionalPenetration
    {
        return Err(GeneralFastError::InvalidSettings(
            "directional penetration requires the rollback triangle backend".to_owned(),
        ));
    }
    #[cfg(not(feature = "jagua-experimental"))]
    if settings.collision_backend == GeneralRelaxedCollisionBackend::DynamicHazard {
        return Err(GeneralFastError::InvalidSettings(
            "dynamic hazard search requires the jagua-experimental feature".to_owned(),
        ));
    }
    Ok(())
}

fn cell_bin_range(
    cell_bounds: IrregularBounds,
    shape_bounds: IrregularBounds,
) -> (usize, usize, usize, usize) {
    bin_range(cell_bounds, shape_bounds, CELL_INDEX_SIDE)
}

fn bin_range(
    cell_bounds: IrregularBounds,
    shape_bounds: IrregularBounds,
    side: usize,
) -> (usize, usize, usize, usize) {
    let (span_x, span_y) = bin_spans(shape_bounds);
    bin_range_within(cell_bounds, shape_bounds, span_x, span_y, side)
}

/// The two bin spans of a shape's extent.
///
/// Split out of [`bin_range`] so an index that queries the same extent tens of
/// millions of times can derive them once. The expressions are the ones
/// [`bin_range`] evaluated inline, so the values are identical.
fn bin_spans(shape_bounds: IrregularBounds) -> (f64, f64) {
    (
        (shape_bounds.max_x - shape_bounds.min_x).max(0.001),
        (shape_bounds.max_y - shape_bounds.min_y).max(0.001),
    )
}

#[inline(always)]
fn bin_range_within(
    cell_bounds: IrregularBounds,
    shape_bounds: IrregularBounds,
    span_x: f64,
    span_y: f64,
    side: usize,
) -> (usize, usize, usize, usize) {
    let bin = |value: f64, min: f64, span: f64| {
        (((value - min) / span) * side as f64)
            .floor()
            .clamp(0.0, (side - 1) as f64) as usize
    };
    (
        bin(cell_bounds.min_x, shape_bounds.min_x, span_x),
        bin(cell_bounds.max_x, shape_bounds.min_x, span_x),
        bin(cell_bounds.min_y, shape_bounds.min_y, span_y),
        bin(cell_bounds.max_y, shape_bounds.min_y, span_y),
    )
}

fn translated_bounds(bounds: IrregularBounds, x: f64, y: f64) -> IrregularBounds {
    IrregularBounds::new(
        bounds.min_x + x,
        bounds.min_y + y,
        bounds.max_x + x,
        bounds.max_y + y,
    )
}

fn bounds_overlap(first: IrregularBounds, second: IrregularBounds) -> bool {
    first.min_x < second.max_x
        && first.max_x > second.min_x
        && first.min_y < second.max_y
        && first.max_y > second.min_y
}

fn point_angle_key(angle_deg: f64) -> i64 {
    (angle_deg.rem_euclid(360.0) * ANGLE_KEY_SCALE).round() as i64
}

fn angle_key(angle_deg: f64) -> i64 {
    point_angle_key(angle_deg)
}

fn angle_from_key(key: i64) -> f64 {
    key as f64 / ANGLE_KEY_SCALE
}

fn canonical_angle(angle_deg: f64) -> f64 {
    let normalized = angle_deg.rem_euclid(360.0);
    angle_from_key(angle_key(
        (normalized / SURROGATE_ANGLE_STEP_DEG).round() * SURROGATE_ANGLE_STEP_DEG,
    ))
}

fn continuous_angle(angle_deg: f64) -> f64 {
    angle_from_key(angle_key(angle_deg.rem_euclid(360.0)))
}

#[cfg(feature = "jagua-experimental")]
fn hazard_pose(placement: &RelaxedPlacement) -> GeneralHazardPose {
    GeneralHazardPose {
        rotation_deg: continuous_angle(placement.rotation_deg),
        mirrored: placement.mirrored,
        translate_short_axis: placement.translate_x,
        translate_long_axis: placement.translate_y,
    }
}

#[cfg(feature = "jagua-experimental")]
fn dynamic_hazard_error(
    error: crate::search::general_hazard::GeneralHazardError,
) -> GeneralFastError {
    GeneralFastError::InvalidInput(format!("dynamic hazard backend: {error}"))
}

fn directional_lane_unscorable_error(reason: &str) -> GeneralFastError {
    GeneralFastError::InvalidSettings(format!("{DIRECTIONAL_LANE_UNSCORABLE}: {reason}"))
}

fn is_directional_lane_unscorable(error: &GeneralFastError) -> bool {
    error.to_string().contains(DIRECTIONAL_LANE_UNSCORABLE)
}

fn snap_mm(value: f64) -> f64 {
    to_grid_mm(value).map(from_grid).unwrap_or(value)
}

fn grid_key(value: f64) -> i64 {
    to_grid_mm(value)
        .map(|value| value as i64)
        .unwrap_or(i64::MAX)
}

fn grid_lower_bound_key(value: f64) -> i64 {
    (value * 1_000.0).floor() as i64
}

fn grid_upper_bound_key(value: f64) -> i64 {
    (value * 1_000.0).ceil() as i64
}

fn grid_interval_bounds(interval: (f64, f64)) -> (i64, i64) {
    let start = interval.0.min(interval.1);
    let end = interval.0.max(interval.1);
    (grid_lower_bound_key(start), grid_upper_bound_key(end))
}

fn ordered_pair(first: usize, second: usize) -> (usize, usize) {
    if first < second {
        (first, second)
    } else {
        (second, first)
    }
}

fn pair_slot(piece_count: usize, first: usize, second: usize) -> usize {
    let (first, second) = ordered_pair(first, second);
    debug_assert!(first < second);
    first * (2 * piece_count - first - 1) / 2 + second - first - 1
}

fn derive_seed(seed: u64, epoch: usize, lane: usize) -> u64 {
    seed ^ (epoch as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (lane as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
}

fn sample_grid_coordinate_with_rng(
    rng: &mut SplitMix64,
    minimum: i128,
    maximum: i128,
) -> Result<i128, GeneralFastError> {
    let span = maximum
        .checked_sub(minimum)
        .and_then(|value| value.checked_add(1))
        .and_then(|value| u64::try_from(value).ok())
        .ok_or_else(|| {
            GeneralFastError::InvalidInput(
                "directional inner-fit interval is outside the sampling domain".to_owned(),
            )
        })?;
    Ok(minimum + i128::from(rng.next_u64() % span))
}

fn shuffle(values: &mut [usize], rng: &mut SplitMix64) {
    for index in (1..values.len()).rev() {
        let swap = (rng.next_u64() as usize) % (index + 1);
        values.swap(index, swap);
    }
}

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }

    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / ((1_u64 << 53) as f64))
    }

    fn range(&mut self, min: f64, max: f64) -> f64 {
        if min >= max {
            return min;
        }
        min + (max - min) * self.unit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "jagua-experimental")]
    use crate::geometry::general_polygon::PolygonRegion;
    use crate::geometry::general_polygon::PolygonSet;
    #[cfg(feature = "jagua-experimental")]
    use crate::parallel::JobPool;
    #[cfg(feature = "jagua-experimental")]
    use crate::search::general_fast::construct_short_side_first;

    fn point(x: f64, y: f64) -> IrregularPoint {
        IrregularPoint::new(x, y)
    }

    fn l_shape() -> PolygonSet {
        PolygonSet::from_outer(vec![
            point(0.0, 0.0),
            point(4.0, 0.0),
            point(4.0, 1.0),
            point(1.0, 1.0),
            point(1.0, 4.0),
            point(0.0, 4.0),
        ])
        .unwrap()
    }

    fn square(size: f64) -> PolygonSet {
        PolygonSet::from_outer(vec![
            point(0.0, 0.0),
            point(size, 0.0),
            point(size, size),
            point(0.0, size),
        ])
        .unwrap()
    }

    #[cfg(feature = "jagua-experimental")]
    fn holed_square() -> PolygonSet {
        PolygonSet::new(vec![PolygonRegion::new(
            vec![
                point(0.0, 0.0),
                point(40.0, 0.0),
                point(40.0, 40.0),
                point(0.0, 40.0),
            ],
            vec![vec![
                point(10.0, 10.0),
                point(10.0, 30.0),
                point(30.0, 30.0),
                point(30.0, 10.0),
            ]],
        )
        .unwrap()])
        .unwrap()
    }

    fn feasible_tracker(piece_count: usize) -> PairTracker {
        PairTracker {
            piece_count,
            boundaries: vec![
                BoundaryEntry {
                    violations: 0,
                    raw_loss: 0.0,
                };
                piece_count
            ],
            pairs: vec![
                PairEntry {
                    raw_loss: 0.0,
                    guided_weight: 1.0,
                    normalization_scale: 1.0,
                };
                piece_count.saturating_mul(piece_count.saturating_sub(1)) / 2
            ],
            incident_raw_loss: vec![0.0; piece_count],
            boundary_violations: 0,
            boundary_loss: 0.0,
            collision_pairs: Vec::new(),
            weighted_loss: 0.0,
        }
    }

    fn feasible_lane(lane: usize, translate_x: f64, translate_y: f64) -> LaneOutcome {
        LaneOutcome {
            state: RelaxedState {
                placements: vec![RelaxedPlacement {
                    input_index: 0,
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_x,
                    translate_y,
                }],
                strip_depth_mm: translate_y + 10.0,
            },
            score: feasible_tracker(1),
            weights: BTreeMap::new(),
            counters: WorkCounters::default(),
            selected_lane: lane,
            restart_disruptions: lane_disruption_count(lane),
        }
    }

    #[cfg(feature = "jagua-experimental")]
    fn coupled_test_settings(seed: u64) -> GeneralRelaxedSettings {
        let mut settings =
            GeneralRelaxedSettings::mixed_61_dynamic_hazard_probe(seed, COUPLED_SEPARATOR_WORKERS);
        settings.sweeps_per_epoch = COUPLED_SEPARATOR_ROUNDS;
        settings.global_samples_per_move = 10;
        settings.focused_samples_per_move = 10;
        settings.refinement_rounds = 5;
        settings.angle_seed_policy = GeneralRelaxedAngleSeedPolicy::ContinuousUniform;
        settings.pressure_model = GeneralRelaxedPressureModel::DynamicPoles;
        settings.synchronize_lanes = true;
        settings
    }

    #[cfg(feature = "jagua-experimental")]
    fn coupled_experiment_test_settings(seed: u64) -> GeneralRelaxedSettings {
        let mut settings = GeneralRelaxedSettings::mixed_61_probe(seed, COUPLED_SEPARATOR_WORKERS);
        settings.sweeps_per_epoch = COUPLED_SEPARATOR_ROUNDS;
        settings.global_samples_per_move = 10;
        settings.focused_samples_per_move = 10;
        settings.refinement_rounds = 5;
        settings.coupled_dynamic_separator = true;
        settings
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn alternation_requires_an_explicit_target_depth() {
        let polygons = [square(10.0), square(8.0)];
        let pieces = [
            GeneralFastPiece {
                id: "large",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "small",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let fast_settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let settings = coupled_experiment_test_settings(3);
        let constructed = JobPool::new(Some(1))
            .run_scoped(|| construct_short_side_first(&pieces, fast_settings))
            .unwrap();
        let parent = GeneralCoupledSeparatorArmDiagnostics {
            final_placements: coupled_placement_diagnostics(&constructed.placements),
            ..GeneralCoupledSeparatorArmDiagnostics::default()
        };
        let result = run_alternation_fixpoint(&pieces, fast_settings, settings, &parent, None);
        assert!(!result.attempted);
        assert!(result
            .failure_reason
            .unwrap()
            .contains("requires an explicit target depth"));
        assert!(result.alternation.is_none());
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn alternation_requires_a_complete_parent() {
        let polygons = [square(10.0), square(8.0)];
        let pieces = [
            GeneralFastPiece {
                id: "large",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "small",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let fast_settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let mut settings = coupled_experiment_test_settings(3);
        settings.persistent_vacancy_target_depth_mm = Some(50.0);
        let parent = GeneralCoupledSeparatorArmDiagnostics::default();
        let result = run_alternation_fixpoint(&pieces, fast_settings, settings, &parent, None);
        assert!(!result.attempted);
        assert!(result
            .failure_reason
            .unwrap()
            .contains("not a complete exact-valid layout"));
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn alternation_runs_to_a_fixpoint_and_reports_diagnostics() {
        let polygons = [square(10.0), square(8.0)];
        let pieces = [
            GeneralFastPiece {
                id: "large",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "small",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let fast_settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let mut settings = coupled_experiment_test_settings(5);
        settings.persistent_vacancy_target_depth_mm = Some(50.0);
        let constructed = JobPool::new(Some(1))
            .run_scoped(|| construct_short_side_first(&pieces, fast_settings))
            .unwrap();
        let parent = GeneralCoupledSeparatorArmDiagnostics {
            final_placements: coupled_placement_diagnostics(&constructed.placements),
            ..GeneralCoupledSeparatorArmDiagnostics::default()
        };
        let result = JobPool::new(Some(1)).run_scoped(|| {
            run_alternation_fixpoint(
                &pieces,
                fast_settings,
                settings,
                &parent,
                Some("test-parent".to_owned()),
            )
        });
        assert!(result.attempted);
        assert!(result.exact_valid);
        assert_eq!(result.mode, 22);
        assert_eq!(result.final_placements.len(), pieces.len());
        let alternation = result.alternation.clone().unwrap();
        assert!(alternation.cycles_run >= 1);
        assert!(alternation.cycles_run <= ALTERNATION_MAX_CYCLES);
        assert_eq!(alternation.cycles.len(), alternation.cycles_run);
        // The descent arm runs on any piece count; on this tiny synthetic
        // fixture it must either improve or report a clean non-improvement,
        // and the loop must still terminate at a joint fixpoint.
        let last = alternation.cycles.last().unwrap();
        assert!(!last.separator_improved && !last.descent_improved);
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn alternation_is_deterministic_across_repeated_runs() {
        let polygons = [square(10.0), square(8.0)];
        let pieces = [
            GeneralFastPiece {
                id: "large",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "small",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let fast_settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let mut settings = coupled_experiment_test_settings(9);
        settings.persistent_vacancy_target_depth_mm = Some(50.0);
        let constructed = JobPool::new(Some(1))
            .run_scoped(|| construct_short_side_first(&pieces, fast_settings))
            .unwrap();
        let parent = GeneralCoupledSeparatorArmDiagnostics {
            final_placements: coupled_placement_diagnostics(&constructed.placements),
            ..GeneralCoupledSeparatorArmDiagnostics::default()
        };
        let first = JobPool::new(Some(1)).run_scoped(|| {
            run_alternation_fixpoint(&pieces, fast_settings, settings, &parent, None)
        });
        let second = JobPool::new(Some(1)).run_scoped(|| {
            run_alternation_fixpoint(&pieces, fast_settings, settings, &parent, None)
        });
        assert_eq!(first, second);
    }

    /// Two squares in a 100x100 sheet, constructed short-side-first, wrapped
    /// as a mode-26 parent. Small enough that a whole ladder runs in a test.
    #[cfg(feature = "jagua-experimental")]
    fn two_piece_ladder_fixture() -> ([PolygonSet; 2], GeneralFastSettings) {
        (
            [square(10.0), square(8.0)],
            GeneralFastSettings::deterministic_test(100.0, 100.0),
        )
    }

    #[cfg(feature = "jagua-experimental")]
    fn two_piece_ladder_parent(
        polygons: &[PolygonSet; 2],
        fast_settings: GeneralFastSettings,
    ) -> GeneralCoupledSeparatorArmDiagnostics {
        let pieces = [
            GeneralFastPiece {
                id: "large",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "small",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let constructed = JobPool::new(Some(1))
            .run_scoped(|| construct_short_side_first(&pieces, fast_settings))
            .unwrap();
        GeneralCoupledSeparatorArmDiagnostics {
            final_placements: coupled_placement_diagnostics(&constructed.placements),
            ..GeneralCoupledSeparatorArmDiagnostics::default()
        }
    }

    /// The whole mode-34 path, end to end, on the same two-piece fixture the
    /// mode-26 ladder tests use.
    ///
    /// It pins the four contracts that make the schedule a *port* rather than a
    /// new operator: the parent is the publication floor, the schedule's own
    /// step is one canonical grid unit, the exact tier is asked at the cadence
    /// the schedule claims, and the work the schedule reports is the work unit
    /// the portfolio budgets in.
    #[test]
    #[cfg(all(feature = "jagua-experimental", feature = "compression-schedule"))]
    fn compression_schedule_publishes_no_worse_than_its_parent() {
        let (polygons, fast_settings) = two_piece_ladder_fixture();
        let pieces = [
            GeneralFastPiece {
                id: "large",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "small",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let parent = two_piece_ladder_parent(&polygons, fast_settings);
        let mut settings = coupled_experiment_test_settings(9);
        settings.persistent_vacancy_mode = 34;
        settings.persistent_vacancy_target_depth_mm = Some(1.0);
        settings.compression_schedule = Some(CompressionScheduleSettings {
            sweeps_per_step: 2,
            confirm_every: 2,
            rollback_after_steps: 8,
            work_cap_queries: Some(200_000),
            continue_past_bound: false,
            repair_policy: CompressionRepairPolicy::MicroLegalizeOnReject,
        });

        let population = JobPool::new(Some(1)).run_scoped(|| {
            run_compression_schedule(&pieces, fast_settings, settings, &parent, None)
        });
        assert!(population.attempted, "{:?}", population.failure_reason);
        assert!(population.exact_valid);
        let report = population
            .compression_schedule
            .expect("an attempted schedule reports");
        let parent_depth = population
            .parent_independent_depth_mm
            .expect("the parent is measured");
        let published = population
            .independent_depth_mm
            .expect("an exact-valid arm has a depth");
        assert!(
            published <= parent_depth,
            "the parent is the floor: {published} vs {parent_depth}"
        );
        assert_eq!(report.step_mm, 0.001, "the step is one grid unit");
        assert_eq!(report.steps_taken, report.steps.len());
        assert!(report.steps_taken > 0);
        // The frontier never sits looser than the deepest confirmed depth.
        assert!(report.final_depth_mm <= report.floor_depth_mm + f64::EPSILON);
        // Every depth the schedule names is on the canonical grid.
        for row in &report.steps {
            assert_eq!(row.depth_mm, snap_mm(row.depth_mm));
        }
        // The work unit is the portfolio's: queries plus five per pair test,
        // and one confirmation is one whole-layout validation.
        assert_eq!(
            report.exact_pair_tests,
            report.confirmations_attempted * pieces.len() * (pieces.len() - 1) / 2
        );
        assert_eq!(
            report.work_units,
            report.candidate_queries + 5 * report.exact_pair_tests
        );
    }

    /// The schedule stops on the budget it was given rather than on the step
    /// plan, and says which.
    #[test]
    #[cfg(all(feature = "jagua-experimental", feature = "compression-schedule"))]
    fn compression_schedule_stops_on_its_work_cap() {
        let (polygons, fast_settings) = two_piece_ladder_fixture();
        let pieces = [
            GeneralFastPiece {
                id: "large",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "small",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let parent = two_piece_ladder_parent(&polygons, fast_settings);
        let mut settings = coupled_experiment_test_settings(9);
        settings.persistent_vacancy_mode = 34;
        settings.persistent_vacancy_target_depth_mm = Some(1.0);
        settings.compression_schedule = Some(CompressionScheduleSettings {
            sweeps_per_step: 2,
            confirm_every: 2,
            rollback_after_steps: 0,
            work_cap_queries: Some(1),
            continue_past_bound: true,
            repair_policy: CompressionRepairPolicy::SweepsOnly,
        });
        let population = JobPool::new(Some(1)).run_scoped(|| {
            run_compression_schedule(&pieces, fast_settings, settings, &parent, None)
        });
        let report = population
            .compression_schedule
            .expect("an attempted schedule reports");
        assert_eq!(report.exit_cause, "workCap");
        assert!(report.steps_taken <= 1);
        // A schedule that spent nothing still publishes its parent.
        assert!(population.exact_valid);
        assert_eq!(
            population.independent_depth_mm,
            population.parent_independent_depth_mm
        );
    }

    /// Mode 34 refuses to run without a schedule, rather than silently running
    /// an unclamped lane.
    #[test]
    #[cfg(all(feature = "jagua-experimental", feature = "compression-schedule"))]
    fn compression_schedule_mode_requires_an_armed_schedule() {
        let (polygons, fast_settings) = two_piece_ladder_fixture();
        let pieces = [
            GeneralFastPiece {
                id: "large",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "small",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let parent = two_piece_ladder_parent(&polygons, fast_settings);
        let mut settings = coupled_experiment_test_settings(9);
        settings.persistent_vacancy_mode = 34;
        settings.persistent_vacancy_target_depth_mm = Some(1.0);
        settings.compression_schedule = None;
        let population = JobPool::new(Some(1)).run_scoped(|| {
            run_compression_schedule(&pieces, fast_settings, settings, &parent, None)
        });
        assert!(!population.attempted);
        assert!(population
            .failure_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("armed compression schedule")));
    }

    /// A lane with no schedule is the lane that was there before: the sweep's
    /// schedule hook must not touch the state or the tracker.
    #[test]
    #[cfg(feature = "compression-schedule")]
    fn an_unarmed_lane_leaves_the_depth_and_the_tracker_alone() {
        let polygons = [square(10.0)];
        let pieces = [GeneralFastPiece {
            id: "only",
            polygon: &polygons[0],
            allow_rotation: false,
            allow_mirror: false,
        }];
        let fast_settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let settings = GeneralRelaxedSettings::mixed_61_probe(3, 1);
        let (catalog, _) = build_surrogate_catalog(
            &pieces,
            fast_settings,
            SurrogateCatalogMode::StructuredGrid,
            None,
        )
        .expect("the catalogue builds for one square");
        let mut search = LegacyLaneSearch::new(&pieces, fast_settings, settings, 1, catalog);
        assert!(search.compression.is_none());
        let mut state = RelaxedState {
            placements: vec![RelaxedPlacement {
                input_index: 0,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: 10.0,
                translate_y: 10.0,
            }],
            strip_depth_mm: 50.0,
        };
        let mut score = search.score_state(&state).expect("the state scores");
        let before = score.clone();
        search
            .apply_compression_schedule(&mut state, &mut score)
            .expect("an unarmed lane cannot fail");
        assert_eq!(state.strip_depth_mm, 50.0);
        assert_eq!(score, before);
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn ladder_bounds_reach_the_requested_final_bound_in_bounded_steps() {
        // The ordinary case: the span is wide enough that the step floor does
        // not bind, so the ladder uses every rung and lands exactly on the
        // requested bound.
        let (step_mm, bounds) = ladder_compression_bounds(167.846, 164.0);
        assert_eq!(bounds.len(), LADDER_COMPRESSION_STEPS);
        assert!((step_mm - (167.846 - 164.0) / LADDER_COMPRESSION_STEPS as f64).abs() < 1e-12);
        assert!((bounds[bounds.len() - 1] - 164.0).abs() < 1e-12);
        for window in bounds.windows(2) {
            assert!(window[1] < window[0], "rungs must descend");
        }
        assert!(
            bounds[0] < 167.846,
            "the first rung must clamp below the parent"
        );

        // The floor case: a span narrower than one separator contraction
        // collapses to a single rung, still exactly at the requested bound.
        let (floor_step_mm, floor_bounds) = ladder_compression_bounds(100.0, 99.99);
        assert_eq!(floor_bounds, vec![99.99]);
        assert!((floor_step_mm - 100.0 * COUPLED_SEPARATOR_CONTRACTION_RATIO).abs() < 1e-12);

        // Scale-freedom: the same relative request produces the same relative
        // ladder at any instance size.
        let (small_step, small) = ladder_compression_bounds(10.0, 9.0);
        let (large_step, large) = ladder_compression_bounds(1000.0, 900.0);
        assert_eq!(small.len(), large.len());
        assert!((large_step / small_step - 100.0).abs() < 1e-9);
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn ladder_compression_requires_an_explicit_final_bound() {
        let (polygons, fast_settings) = two_piece_ladder_fixture();
        let pieces = [
            GeneralFastPiece {
                id: "large",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "small",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let settings = coupled_experiment_test_settings(3);
        let parent = two_piece_ladder_parent(&polygons, fast_settings);
        let result = run_ladder_compression(&pieces, fast_settings, settings, &parent, None);
        assert!(!result.attempted);
        assert!(result
            .failure_reason
            .unwrap()
            .contains("requires an explicit final bound"));
        assert!(result.ladder_compression.is_none());
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn ladder_compression_rejects_a_bound_at_or_above_the_parent_depth() {
        let (polygons, fast_settings) = two_piece_ladder_fixture();
        let pieces = [
            GeneralFastPiece {
                id: "large",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "small",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let mut settings = coupled_experiment_test_settings(3);
        // Far above any layout this fixture can produce, so the clamp would be
        // vacuous. The mode must say so instead of burning a ladder.
        settings.persistent_vacancy_target_depth_mm = Some(90.0);
        let parent = two_piece_ladder_parent(&polygons, fast_settings);
        let result = JobPool::new(Some(1))
            .run_scoped(|| run_ladder_compression(&pieces, fast_settings, settings, &parent, None));
        assert!(!result.attempted);
        assert!(result
            .failure_reason
            .unwrap()
            .contains("must be below the parent depth"));
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn ladder_compression_requires_a_complete_parent() {
        let (polygons, fast_settings) = two_piece_ladder_fixture();
        let pieces = [
            GeneralFastPiece {
                id: "large",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "small",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let mut settings = coupled_experiment_test_settings(3);
        settings.persistent_vacancy_target_depth_mm = Some(10.0);
        let parent = GeneralCoupledSeparatorArmDiagnostics::default();
        let result = run_ladder_compression(&pieces, fast_settings, settings, &parent, None);
        assert!(!result.attempted);
        assert!(result
            .failure_reason
            .unwrap()
            .contains("not a complete exact-valid layout"));
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn ladder_compression_walks_every_rung_and_never_publishes_worse_than_its_parent() {
        let (polygons, fast_settings) = two_piece_ladder_fixture();
        let pieces = [
            GeneralFastPiece {
                id: "large",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "small",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let mut settings = coupled_experiment_test_settings(5);
        // A bound the fixture cannot possibly honour: the largest piece alone
        // is deeper than this. The ladder must still run to completion, report
        // every rung, and fall back to publishing its own parent.
        settings.persistent_vacancy_target_depth_mm = Some(1.0);
        let parent = two_piece_ladder_parent(&polygons, fast_settings);
        let result = JobPool::new(Some(1)).run_scoped(|| {
            run_ladder_compression(
                &pieces,
                fast_settings,
                settings,
                &parent,
                Some("test-parent".to_owned()),
            )
        });
        assert!(result.attempted);
        assert_eq!(result.mode, 26);
        assert!(result.exact_valid, "the parent floor is always publishable");
        assert_eq!(result.final_placements.len(), pieces.len());
        let ladder = result.ladder_compression.clone().unwrap();
        assert_eq!(ladder.steps_run, ladder.steps_planned);
        assert_eq!(ladder.steps.len(), ladder.steps_run);
        assert!(ladder.steps_planned <= LADDER_COMPRESSION_STEPS);
        assert_eq!(ladder.final_bound_mm, 1.0);
        assert_eq!(
            ladder.published_step, None,
            "an impossible bound publishes nothing new"
        );
        assert_eq!(
            result.independent_depth_mm, result.parent_independent_depth_mm,
            "publication falls back to the parent"
        );
        for (index, step) in ladder.steps.iter().enumerate() {
            assert_eq!(step.step, index);
            assert!(!step.arms.is_empty(), "every rung runs at least one arm");
            assert!(step.bound_mm >= ladder.final_bound_mm);
            assert!(
                step.seed_depth_mm > step.bound_mm,
                "the seed carries headroom"
            );
            assert!(!step.improved_publication);
            assert_eq!(step.published_depth_mm_after, ladder.parent_depth_mm);
            assert_eq!(step.arms[0].role, "feasible");
        }
        // The clamp is real: nothing the ladder ever published sits past the
        // parent's own depth.
        assert!(result.independent_depth_mm.unwrap() <= ladder.parent_depth_mm);
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn ladder_compression_is_deterministic_across_repeated_runs() {
        let (polygons, fast_settings) = two_piece_ladder_fixture();
        let pieces = [
            GeneralFastPiece {
                id: "large",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "small",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let mut settings = coupled_experiment_test_settings(9);
        settings.persistent_vacancy_target_depth_mm = Some(20.0);
        let parent = two_piece_ladder_parent(&polygons, fast_settings);
        let first = JobPool::new(Some(1))
            .run_scoped(|| run_ladder_compression(&pieces, fast_settings, settings, &parent, None));
        let second = JobPool::new(Some(1))
            .run_scoped(|| run_ladder_compression(&pieces, fast_settings, settings, &parent, None));
        assert_eq!(first, second);
    }

    fn two_piece_recombination_fixture() -> (
        [PolygonSet; 2],
        GeneralFastSettings,
        GeneralCoupledSeparatorArmDiagnostics,
        GeneralPersistentVacancyPinnedParent,
    ) {
        let polygons = [square(10.0), square(8.0)];
        let fast_settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let parent_a = GeneralCoupledSeparatorArmDiagnostics {
            final_placements: vec![
                GeneralCoupledSeparatorPlacementDiagnostics {
                    piece_id: "large".to_owned(),
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_short_axis: 0.0,
                    translate_long_axis: 0.0,
                },
                GeneralCoupledSeparatorPlacementDiagnostics {
                    piece_id: "small".to_owned(),
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_short_axis: 30.0,
                    translate_long_axis: 0.0,
                },
            ],
            ..GeneralCoupledSeparatorArmDiagnostics::default()
        };
        let parent_b = GeneralPersistentVacancyPinnedParent {
            placements: vec![
                GeneralFastPlacement {
                    piece_id: "large".to_owned(),
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_short_axis: 20.0,
                    translate_long_axis: 15.0,
                },
                GeneralFastPlacement {
                    piece_id: "small".to_owned(),
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_short_axis: 0.0,
                    translate_long_axis: 15.0,
                },
            ],
            source: "test-parent-b".to_owned(),
            source_sha256: "deadbeef".to_owned(),
        };
        (polygons, fast_settings, parent_a, parent_b)
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn recombination_requires_a_secondary_parent() {
        let (polygons, fast_settings, parent_a, _) = two_piece_recombination_fixture();
        let pieces = [
            GeneralFastPiece {
                id: "large",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "small",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let mut settings = coupled_experiment_test_settings(11);
        settings.persistent_vacancy_target_depth_mm = Some(0.5);
        let result = run_recombination(&pieces, fast_settings, settings, &parent_a, None, None);
        assert!(!result.attempted);
        assert!(result
            .failure_reason
            .unwrap()
            .contains("second parent fixture"));
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn recombination_requires_cut_fraction_in_open_unit_interval() {
        let (polygons, fast_settings, parent_a, parent_b) = two_piece_recombination_fixture();
        let pieces = [
            GeneralFastPiece {
                id: "large",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "small",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let mut settings = coupled_experiment_test_settings(13);
        for bad_fraction in [0.0, 1.0, -0.1, 1.1, f64::NAN] {
            settings.persistent_vacancy_target_depth_mm = Some(bad_fraction);
            let result = run_recombination(
                &pieces,
                fast_settings,
                settings,
                &parent_a,
                None,
                Some(&parent_b),
            );
            assert!(!result.attempted);
            assert!(
                result
                    .failure_reason
                    .as_deref()
                    .is_some_and(|reason| reason.contains("cut fraction")),
                "fraction {bad_fraction} unexpectedly accepted"
            );
        }
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn recombination_requires_matching_piece_id_sets() {
        let (polygons, fast_settings, parent_a, _) = two_piece_recombination_fixture();
        let pieces = [
            GeneralFastPiece {
                id: "large",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "small",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let mismatched_parent_b = GeneralPersistentVacancyPinnedParent {
            placements: vec![
                GeneralFastPlacement {
                    piece_id: "large".to_owned(),
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_short_axis: 0.0,
                    translate_long_axis: 0.0,
                },
                GeneralFastPlacement {
                    piece_id: "other".to_owned(),
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_short_axis: 30.0,
                    translate_long_axis: 0.0,
                },
            ],
            source: "mismatched".to_owned(),
            source_sha256: "deadbeef".to_owned(),
        };
        let mut settings = coupled_experiment_test_settings(17);
        settings.persistent_vacancy_target_depth_mm = Some(0.5);
        let result = run_recombination(
            &pieces,
            fast_settings,
            settings,
            &parent_a,
            None,
            Some(&mismatched_parent_b),
        );
        assert!(!result.attempted);
        assert!(result.failure_reason.unwrap().contains("pieceId set"));
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn recombination_crosses_and_legalizes_two_parents() {
        let (polygons, fast_settings, parent_a, parent_b) = two_piece_recombination_fixture();
        let pieces = [
            GeneralFastPiece {
                id: "large",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "small",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let mut settings = coupled_experiment_test_settings(19);
        settings.persistent_vacancy_target_depth_mm = Some(0.5);
        let result = JobPool::new(Some(1)).run_scoped(|| {
            run_recombination(
                &pieces,
                fast_settings,
                settings,
                &parent_a,
                Some("parent-a".to_owned()),
                Some(&parent_b),
            )
        });
        assert!(result.attempted);
        assert_eq!(result.mode, 23);
        assert_eq!(result.final_placements.len(), pieces.len());
        let recombination = result.recombination.clone().unwrap();
        assert!((recombination.cut_fraction - 0.5).abs() < 1e-12);
        assert_eq!(
            recombination.pieces_from_parent_a + recombination.pieces_from_parent_b,
            2
        );
        // The large piece's short-axis anchor (0.0) is below the threshold
        // (15.0 = min 0.0 + 0.5 * span 30.0), so it keeps parent A's pose.
        assert_eq!(recombination.pieces_from_parent_a, 1);
        assert_eq!(recombination.pieces_from_parent_b, 1);
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn recombination_is_deterministic_across_repeated_runs() {
        let (polygons, fast_settings, parent_a, parent_b) = two_piece_recombination_fixture();
        let pieces = [
            GeneralFastPiece {
                id: "large",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "small",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let mut settings = coupled_experiment_test_settings(23);
        settings.persistent_vacancy_target_depth_mm = Some(0.5);
        let first = JobPool::new(Some(1)).run_scoped(|| {
            run_recombination(
                &pieces,
                fast_settings,
                settings,
                &parent_a,
                None,
                Some(&parent_b),
            )
        });
        let second = JobPool::new(Some(1)).run_scoped(|| {
            run_recombination(
                &pieces,
                fast_settings,
                settings,
                &parent_a,
                None,
                Some(&parent_b),
            )
        });
        assert_eq!(first, second);
    }

    /// Two squares whose parent poses both sit deep in the sheet. `long_axis`
    /// picks the large piece's long-axis anchor, which is what decides
    /// whether a given bound ejects one piece or both.
    #[cfg(feature = "jagua-experimental")]
    fn two_piece_bounded_reinsertion_fixture(
        large_long_axis: f64,
    ) -> (
        [PolygonSet; 2],
        GeneralFastSettings,
        GeneralCoupledSeparatorArmDiagnostics,
    ) {
        // `deterministic_test` leaves padding and edge clearance at zero, so
        // a piece's measured long-axis extent is exactly its transformed
        // `max_y`: the large square reaches `large_long_axis + 10`, the small
        // one reaches 48. Both poses are held a millimetre off the sheet
        // edge, since the collision offset an exact validation rebuilds adds
        // `search_offset_allowance_mm` in every direction.
        let polygons = [square(10.0), square(8.0)];
        let fast_settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let parent = GeneralCoupledSeparatorArmDiagnostics {
            final_placements: vec![
                GeneralCoupledSeparatorPlacementDiagnostics {
                    piece_id: "large".to_owned(),
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_short_axis: 1.0,
                    translate_long_axis: large_long_axis,
                },
                GeneralCoupledSeparatorPlacementDiagnostics {
                    piece_id: "small".to_owned(),
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_short_axis: 30.0,
                    translate_long_axis: 40.0,
                },
            ],
            ..GeneralCoupledSeparatorArmDiagnostics::default()
        };
        (polygons, fast_settings, parent)
    }

    #[cfg(feature = "jagua-experimental")]
    fn bounded_reinsertion_test_pieces(polygons: &[PolygonSet; 2]) -> [GeneralFastPiece<'_>; 2] {
        [
            GeneralFastPiece {
                id: "large",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "small",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ]
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn bounded_reinsertion_requires_a_positive_finite_bound() {
        let (polygons, fast_settings, parent) = two_piece_bounded_reinsertion_fixture(0.0);
        let pieces = bounded_reinsertion_test_pieces(&polygons);
        let mut settings = coupled_experiment_test_settings(29);

        settings.persistent_vacancy_target_depth_mm = None;
        let missing = persistent_vacancy::run_bounded_reinsertion(
            &pieces,
            fast_settings,
            settings,
            &parent,
            None,
        );
        assert!(!missing.attempted);
        assert_eq!(missing.mode, 24);
        assert!(missing
            .failure_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("requires an explicit depth bound")));
        assert!(missing.bounded_reinsertion.is_none());

        for bad_bound in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            settings.persistent_vacancy_target_depth_mm = Some(bad_bound);
            let result = persistent_vacancy::run_bounded_reinsertion(
                &pieces,
                fast_settings,
                settings,
                &parent,
                None,
            );
            assert!(!result.attempted, "bound {bad_bound} unexpectedly accepted");
            assert!(
                result
                    .failure_reason
                    .as_deref()
                    .is_some_and(|reason| reason.contains("positive finite value")),
                "bound {bad_bound} unexpectedly accepted"
            );
        }
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn bounded_reinsertion_requires_a_complete_parent() {
        let polygons = [square(10.0), square(8.0)];
        let pieces = bounded_reinsertion_test_pieces(&polygons);
        let fast_settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let mut settings = coupled_experiment_test_settings(31);
        settings.persistent_vacancy_target_depth_mm = Some(30.0);
        let result = persistent_vacancy::run_bounded_reinsertion(
            &pieces,
            fast_settings,
            settings,
            &GeneralCoupledSeparatorArmDiagnostics::default(),
            None,
        );
        assert!(!result.attempted);
        assert!(result
            .failure_reason
            .unwrap()
            .contains("not a complete exact-valid layout"));
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn bounded_reinsertion_ejects_only_the_pieces_past_the_bound() {
        // The large square ends at 11 mm and the small one at 48 mm, so a
        // 30 mm bound ejects exactly the small piece and pins the large one.
        let (polygons, fast_settings, parent) = two_piece_bounded_reinsertion_fixture(1.0);
        let pieces = bounded_reinsertion_test_pieces(&polygons);
        let mut settings = coupled_experiment_test_settings(37);
        settings.persistent_vacancy_target_depth_mm = Some(30.0);
        let result = JobPool::new(Some(1)).run_scoped(|| {
            persistent_vacancy::run_bounded_reinsertion(
                &pieces,
                fast_settings,
                settings,
                &parent,
                None,
            )
        });
        assert!(result.attempted, "{:?}", result.failure_reason);
        assert!(result.exact_valid, "{:?}", result.failure_reason);
        assert_eq!(result.mode, 24);
        assert_eq!(result.final_placements.len(), pieces.len());

        let bounded = result.bounded_reinsertion.clone().unwrap();
        assert_eq!(bounded.bound_mm, 30.0);
        assert_eq!(bounded.parent_depth_mm, 48.0);
        assert_eq!(bounded.kept_count, 1);
        assert_eq!(bounded.ejected_count, 1);
        assert_eq!(bounded.reinserted_count, 1);
        assert_eq!(bounded.pieces.len(), 1);
        assert_eq!(bounded.pieces[0].piece_id, "small");
        assert_eq!(bounded.pieces[0].parent_extent_mm, 48.0);
        assert!(bounded.pieces[0].reinserted);
        assert!(bounded.failed_piece_id.is_none());
        // The whole point of the mode: the published layout is inside the
        // bound, so it is strictly shallower than the parent it compressed.
        let final_depth_mm = bounded.final_depth_mm.unwrap();
        assert!(
            final_depth_mm <= 30.0,
            "final depth {final_depth_mm} exceeded the bound"
        );
        assert_eq!(result.independent_depth_mm, Some(final_depth_mm));
        assert!(final_depth_mm < bounded.parent_depth_mm);
        // The kept piece never moved.
        let large = result
            .final_placements
            .iter()
            .find(|placement| placement.piece_id == "large")
            .unwrap();
        assert_eq!(large.translate_short_axis, 1.0);
        assert_eq!(large.translate_long_axis, 1.0);
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn bounded_reinsertion_replaces_displaced_pieces_by_descending_area() {
        // Both squares now end past a 20 mm bound (the large one at 50 mm,
        // the small one at 48 mm), so both are ejected and the reinsertion
        // order is observable: the 100 mm2 square before the 64 mm2 one.
        let (polygons, fast_settings, parent) = two_piece_bounded_reinsertion_fixture(40.0);
        let pieces = bounded_reinsertion_test_pieces(&polygons);
        let mut settings = coupled_experiment_test_settings(41);
        settings.persistent_vacancy_target_depth_mm = Some(20.0);
        let result = JobPool::new(Some(1)).run_scoped(|| {
            persistent_vacancy::run_bounded_reinsertion(
                &pieces,
                fast_settings,
                settings,
                &parent,
                None,
            )
        });
        assert!(result.attempted, "{:?}", result.failure_reason);
        assert!(result.exact_valid, "{:?}", result.failure_reason);

        let bounded = result.bounded_reinsertion.clone().unwrap();
        assert_eq!(bounded.parent_depth_mm, 50.0);
        assert_eq!(bounded.kept_count, 0);
        assert_eq!(bounded.ejected_count, 2);
        assert_eq!(bounded.reinserted_count, 2);
        assert_eq!(
            bounded
                .pieces
                .iter()
                .map(|row| row.piece_id.as_str())
                .collect::<Vec<_>>(),
            vec!["large", "small"]
        );
        assert_eq!(
            result.initial_inactive_piece_ids,
            vec!["large".to_owned(), "small".to_owned()]
        );
        for row in &bounded.pieces {
            let placed = row.placed_extent_mm.unwrap();
            assert!(placed <= 20.0, "piece {} placed at {placed}", row.piece_id);
        }
        assert!(bounded.final_depth_mm.unwrap() <= 20.0);
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn bounded_reinsertion_fails_cleanly_under_an_impossible_bound() {
        // No pose of a 10 mm square fits a 5 mm strip, so the bound is
        // unreachable. The mode must report that, not exceed it.
        let (polygons, fast_settings, parent) = two_piece_bounded_reinsertion_fixture(40.0);
        let pieces = bounded_reinsertion_test_pieces(&polygons);
        let mut settings = coupled_experiment_test_settings(43);
        settings.persistent_vacancy_target_depth_mm = Some(5.0);
        let result = JobPool::new(Some(1)).run_scoped(|| {
            persistent_vacancy::run_bounded_reinsertion(
                &pieces,
                fast_settings,
                settings,
                &parent,
                None,
            )
        });
        assert!(result.attempted, "{:?}", result.failure_reason);
        assert!(!result.exact_valid);
        assert!(result.independent_depth_mm.is_none());
        assert!(result.final_placements.is_empty());
        assert!(result
            .failure_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("within the 5 mm bound")));

        let bounded = result.bounded_reinsertion.clone().unwrap();
        assert_eq!(bounded.ejected_count, 2);
        assert_eq!(bounded.reinserted_count, 0);
        assert_eq!(bounded.failed_piece_id.as_deref(), Some("large"));
        assert!(bounded.final_depth_mm.is_none());
        // The run stops at the first unplaceable piece rather than working
        // through the rest of the ejected set.
        assert_eq!(bounded.pieces.len(), 1);
        assert!(!bounded.pieces[0].reinserted);
        assert!(bounded.pieces[0].failure_reason.is_some());
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn bounded_reinsertion_is_deterministic_across_repeated_runs() {
        let (polygons, fast_settings, parent) = two_piece_bounded_reinsertion_fixture(1.0);
        let pieces = bounded_reinsertion_test_pieces(&polygons);
        let mut settings = coupled_experiment_test_settings(47);
        settings.persistent_vacancy_target_depth_mm = Some(30.0);
        let first = JobPool::new(Some(1)).run_scoped(|| {
            persistent_vacancy::run_bounded_reinsertion(
                &pieces,
                fast_settings,
                settings,
                &parent,
                None,
            )
        });
        let second = JobPool::new(Some(1)).run_scoped(|| {
            persistent_vacancy::run_bounded_reinsertion(
                &pieces,
                fast_settings,
                settings,
                &parent,
                None,
            )
        });
        assert_eq!(first, second);
    }

    #[test]
    fn rollback_backend_rejects_non_rollback_pressure_models() {
        for pressure_model in [
            GeneralRelaxedPressureModel::ContinuousTrianglePoles,
            GeneralRelaxedPressureModel::DynamicPoles,
        ] {
            let settings = GeneralRelaxedSettings {
                pressure_model,
                ..GeneralRelaxedSettings::mixed_61_probe(0, 1)
            };
            let error = validate_relaxed_settings(settings).unwrap_err();
            assert!(error
                .to_string()
                .contains("requires structured or directional pressure"));
        }
        let settings = GeneralRelaxedSettings {
            pressure_model: GeneralRelaxedPressureModel::DirectionalPenetration,
            ..GeneralRelaxedSettings::mixed_61_probe(0, 1)
        };
        assert!(validate_relaxed_settings(settings).is_ok());
    }

    #[test]
    fn ear_clipping_preserves_concave_area() {
        let polygon = l_shape();
        let cells = triangulate_ring(polygon.regions()[0].outer.points()).unwrap();
        let cell_area = cells
            .iter()
            .map(|cell| {
                let [a, b, c] = cell.points;
                ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)).abs() / 2.0
            })
            .sum::<f64>();
        assert_eq!(cells.len(), 4);
        assert!((cell_area - polygon.area_mm2()).abs() < 1e-9);
    }

    #[test]
    fn triangle_penetration_has_a_boolean_zero_boundary() {
        let first = Triangle::new([point(0.0, 0.0), point(2.0, 0.0), point(0.0, 2.0)]);
        let second = Triangle::new([point(0.0, 0.0), point(2.0, 0.0), point(0.0, 2.0)]);
        assert!(triangle_penetration(first, 0.0, 0.0, second, 1.0, 0.0).is_some());
        assert!(triangle_penetration(first, 0.0, 0.0, second, 2.0, 0.0).is_none());
    }

    /// The specialised proxy narrow phase answers the deriving path's question.
    ///
    /// [`oriented_cells_penetrate`] reads the first triangle's axes and its own
    /// projections out of a [`CellAxes`] instead of deriving them, which is only
    /// sound if the two agree on every input - including the ones that decide
    /// the verdict on a rounding bit. The sweep is deliberately dense around
    /// exact contact (`2.0` is the tangent offset for these cells) and includes
    /// a degenerate cell, whose zero-length edge the deriving path answers
    /// `None` to.
    #[test]
    fn specialised_cell_penetration_matches_the_deriving_path() {
        let cells = [
            Triangle::new([point(0.0, 0.0), point(2.0, 0.0), point(0.0, 2.0)]),
            Triangle::new([point(0.0, 0.0), point(3.5, 0.25), point(1.25, 2.75)]),
            Triangle::new([
                point(-1.5, -0.75),
                point(0.125, -1.875),
                point(0.375, 0.625),
            ]),
            // Collinear points: every edge is non-zero but the triangle has no
            // area, which is the boundary the strict test refuses.
            Triangle::new([point(0.0, 0.0), point(1.0, 1.0), point(2.0, 2.0)]),
            // A repeated vertex: edge 0 has length exactly zero.
            Triangle::new([point(1.0, 1.0), point(1.0, 1.0), point(2.5, 0.5)]),
        ];
        let mut some = 0_usize;
        let mut none = 0_usize;
        for first in cells {
            let axes = CellAxes::new(first);
            for second in cells {
                for step_x in -40_i32..=40 {
                    for step_y in -40_i32..=40 {
                        let second_x = f64::from(step_x) * 0.1;
                        let second_y = f64::from(step_y) * 0.1;
                        let derived =
                            triangle_penetration(first, 0.0, 0.0, second, second_x, second_y);
                        let specialised =
                            oriented_cells_penetrate(&axes, second, second_x, second_y);
                        assert_eq!(
                            derived.is_some(),
                            specialised,
                            "disagreement at ({second_x}, {second_y})"
                        );
                        if derived.is_some() {
                            some += 1;
                        } else {
                            none += 1;
                        }
                    }
                }
            }
        }
        // Both verdicts have to be represented or the sweep proves nothing.
        assert!(some > 0 && none > 0, "sweep produced {some} / {none}");
    }

    /// A cell's bit is reported exactly when the membership walk reported it.
    ///
    /// The bin masks replace a `Vec<Vec<usize>>` walk, and the claim is that
    /// the *set* is unchanged rather than merely that the answer is still
    /// conservative - a lost bit would silently turn a collision into a miss.
    #[test]
    fn cell_index_masks_reproduce_the_membership_walk() {
        let cells = triangulate_ring(l_shape().regions()[0].outer.points()).unwrap();
        let bounds = l_shape().bounds().unwrap();
        let index = CellIndex::new(&cells, bounds);
        let mut walked = vec![Vec::new(); CELL_INDEX_SIDE * CELL_INDEX_SIDE];
        for (cell_index, cell) in cells.iter().enumerate() {
            let (min_x, max_x, min_y, max_y) = cell_bin_range(cell.bounds, bounds);
            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    walked[y * CELL_INDEX_SIDE + x].push(cell_index);
                }
            }
        }
        let mut probed = 0_usize;
        for step_x in -12_i32..=12 {
            for step_y in -12_i32..=12 {
                let probe = translated_bounds(
                    cells[0].bounds,
                    f64::from(step_x) * 2.0,
                    f64::from(step_y) * 2.0,
                );
                for translate in [0.0_f64, 7.5, -3.25] {
                    let local = IrregularBounds::new(
                        probe.min_x - translate,
                        probe.min_y - translate,
                        probe.max_x - translate,
                        probe.max_y - translate,
                    );
                    let (min_x, max_x, min_y, max_y) = cell_bin_range(local, bounds);
                    let mut expected = [0_u64; MAX_CELLS_PER_PIECE / 64];
                    for y in min_y..=max_y {
                        for x in min_x..=max_x {
                            for cell_index in walked[y * CELL_INDEX_SIDE + x].iter().copied() {
                                expected[cell_index / 64] |= 1_u64 << (cell_index % 64);
                            }
                        }
                    }
                    let mut actual = [u64::MAX; MAX_CELLS_PER_PIECE / 64];
                    let words = index.query_mask_into(probe, translate, translate, &mut actual);
                    assert_eq!(words, 1);
                    assert_eq!(actual[..words], expected[..words]);
                    probed += 1;
                }
            }
        }
        assert_eq!(probed, 25 * 25 * 3);
    }

    /// A cell never leaves the extent the surrogate advertises.
    ///
    /// This is the premise the whole-shape early-out in
    /// [`surrogate_pair_collides`] rests on: a first cell that misses
    /// `second_shape.bounds` can skip the index query because no cell of the
    /// second shape can reach outside that extent.
    #[test]
    fn surrogate_cells_stay_inside_the_shape_extent() {
        let polygon = l_shape();
        let bounds = polygon.bounds().unwrap();
        let cells = triangulate_ring(polygon.regions()[0].outer.points()).unwrap();
        assert!(cells.len() >= 3);
        for cell in cells {
            assert!(cell.bounds.min_x >= bounds.min_x);
            assert!(cell.bounds.min_y >= bounds.min_y);
            assert!(cell.bounds.max_x <= bounds.max_x);
            assert!(cell.bounds.max_y <= bounds.max_y);
        }
    }

    #[test]
    fn canonical_grid_triangle_overlap_has_an_exact_contact_boundary() {
        let first = Triangle::new([point(0.0, 0.0), point(2.0, 0.0), point(0.0, 2.0)]);
        let second = Triangle::new([point(0.0, 0.0), point(2.0, 0.0), point(0.0, 2.0)]);
        assert_eq!(
            triangles_overlap_on_grid(first, second, 1_999, 0),
            Some(true)
        );
        assert_eq!(
            triangles_overlap_on_grid(first, second, 2_000, 0),
            Some(false)
        );
        assert_eq!(
            triangles_overlap_on_grid(first, second, 2_001, 0),
            Some(false)
        );
        let origin_relative = relative_grid_coordinate(0.0, 1.999);
        let shifted_relative = relative_grid_coordinate(1_000_000.0, 1_000_001.999);
        assert_eq!(shifted_relative, origin_relative);
        assert_eq!(
            triangles_overlap_on_grid(first, second, shifted_relative.unwrap(), 0),
            Some(true)
        );
        assert_eq!(relative_grid_coordinate(f64::INFINITY, 0.0), None);
    }

    #[test]
    fn convex_line_interval_returns_contact_coordinates() {
        let square = [
            point(0.0, 0.0),
            point(4.0, 0.0),
            point(4.0, 3.0),
            point(0.0, 3.0),
        ];
        assert_eq!(
            convex_line_interval(&square, point(-2.0, 1.0), (1.0, 0.0)),
            Some((2.0, 6.0))
        );
        assert_eq!(
            convex_line_interval(&square, point(2.0, -5.0), (0.0, 1.0)),
            Some((5.0, 8.0))
        );
        assert_eq!(
            convex_line_interval(&square, point(-2.0, 4.0), (1.0, 0.0)),
            None
        );
    }

    #[test]
    fn interval_union_drives_directional_penetration() {
        let mut intervals = vec![(5.0, 8.0), (0.0, 3.0), (2.0, 6.0), (10.0, 11.0)];
        merge_intervals(&mut intervals);
        assert_eq!(intervals, vec![(0.0, 8.0), (10.0, 11.0)]);
        assert_eq!(interval_penetration(4.0, &intervals), 4.0);
        assert_eq!(interval_penetration(9.0, &intervals), 0.0);
        assert_eq!(interval_penetration(10.25, &intervals), 0.25);
    }

    #[test]
    fn lane_seed_derivation_is_stable_and_distinct() {
        assert_eq!(derive_seed(7, 2, 3), derive_seed(7, 2, 3));
        assert_ne!(derive_seed(7, 2, 3), derive_seed(7, 2, 4));
    }

    /// Under `canonical-pair-order`, a pair question is a function of the
    /// *unordered* pair: asking `(a, b)` and asking `(b, a)` returns the same
    /// bits, verdict and magnitude alike.
    ///
    /// This is the row-ownership contract, and it is the one the default build
    /// does not keep. The proxy tier's narrow phase tests the first operand's
    /// precomputed cell axes against the second operand's points in a frame
    /// relative to the first, and its pressure accumulates a pole series with
    /// the first operand outermost, so both halves of the answer depend on
    /// which operand was named first. A candidate scan names the moving piece
    /// and a whole-layout score names the lower index, which is why the two
    /// have never agreed. See `canonical_pair_operands`.
    #[cfg(feature = "canonical-pair-order")]
    #[test]
    fn canonical_pair_order_makes_a_pair_question_order_free() {
        let first_polygon = square(10.0);
        let second_polygon = l_shape();
        let pieces = [
            GeneralFastPiece {
                id: "first",
                polygon: &first_polygon,
                allow_rotation: true,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "second",
                polygon: &second_polygon,
                allow_rotation: true,
                allow_mirror: false,
            },
        ];
        let fast_settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let (catalog, _) = build_surrogate_catalog(
            &pieces,
            fast_settings,
            SurrogateCatalogMode::ZeroDegreeOnly,
            None,
        )
        .unwrap();
        let shape = |index: usize| {
            catalog
                .orientations
                .get(&(catalog.geometry_class_by_input[index], angle_key(0.0), false))
                .expect("zero-degree surrogate")
        };
        let mut kernel = LegacyKernel::default();
        let mut counters = WorkCounters::default();
        // A ladder of offsets from deep overlap out to clear separation, so the
        // sweep crosses the contact boundary where an order-dependent verdict
        // would show up rather than only sampling the easy interior.
        for step in 0..40 {
            let offset = step as f64 * 0.25;
            let first = RelaxedPlacement {
                input_index: 0,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: 20.0,
                translate_y: 20.0,
            };
            let second = RelaxedPlacement {
                input_index: 1,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: 20.0 + offset,
                translate_y: 20.0 + offset,
            };
            let forward = resolved_pair_row(
                &mut kernel,
                &mut counters,
                shape(0),
                &first,
                shape(1),
                &second,
            )
            .penalty();
            let reverse = resolved_pair_row(
                &mut kernel,
                &mut counters,
                shape(1),
                &second,
                shape(0),
                &first,
            )
            .penalty();
            assert_eq!(
                forward.to_bits(),
                reverse.to_bits(),
                "pair question at offset {offset} answered {forward:.17e} as (0, 1) \
                 and {reverse:.17e} as (1, 0)"
            );
        }
    }

    /// A bound-pruned row set is marked as one, and a scan that ran to the end
    /// is marked complete — over the same scorer, same state, same candidate,
    /// with only the bound changing.
    ///
    /// This is the property [`update_score_after_move`]'s `debug_assert` rests
    /// on, and asserting it here is what stops that assert from being vacuous:
    /// if [`MovedRows::PrunedAtBound`] were never produced in the first place,
    /// "no pruned delta reaches the tracker" would hold for the wrong reason.
    /// A bound of zero prunes at the first colliding partner of a candidate
    /// that has some; no bound at all cannot prune.
    #[test]
    fn a_bound_pruned_row_set_is_marked_pruned_and_an_unbounded_one_complete() {
        let polygon = square(10.0);
        let pieces = [
            GeneralFastPiece {
                id: "a",
                polygon: &polygon,
                allow_rotation: false,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "b",
                polygon: &polygon,
                allow_rotation: false,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "c",
                polygon: &polygon,
                allow_rotation: false,
                allow_mirror: false,
            },
        ];
        let fast_settings = GeneralFastSettings::deterministic_test(200.0, 200.0);
        let (catalog, _) = build_surrogate_catalog(
            &pieces,
            fast_settings,
            SurrogateCatalogMode::ZeroDegreeOnly,
            None,
        )
        .unwrap();
        let relaxed_settings = GeneralRelaxedSettings::mixed_61_probe(0, 1);
        let mut search =
            LegacyLaneSearch::new(&pieces, fast_settings, relaxed_settings, 0, catalog);
        // Three squares stacked on one point: piece 0's scan has two colliding
        // partners, so a zero bound stops it after the first.
        let placement = |input_index: usize| RelaxedPlacement {
            input_index,
            rotation_deg: 0.0,
            mirrored: false,
            translate_x: 30.0,
            translate_y: 30.0,
        };
        let state = RelaxedState {
            placements: vec![placement(0), placement(1), placement(2)],
            strip_depth_mm: 200.0,
        };
        let piece_index = search.build_piece_index(&state).unwrap();
        let candidate = placement(0);

        let complete = search
            .score_placement(&state, 0, &candidate, &piece_index, None)
            .unwrap();
        assert_eq!(complete.rows, MovedRows::Complete);
        assert_eq!(
            complete.collision_pairs.len(),
            2,
            "an unbounded scan must see both partners"
        );

        let pruned = search
            .score_placement(&state, 0, &candidate, &piece_index, Some(0.0))
            .unwrap();
        assert_eq!(pruned.rows, MovedRows::PrunedAtBound);
        assert!(
            pruned.collision_pairs.len() < complete.collision_pairs.len(),
            "a pruned scan must have stopped short: {} against {}",
            pruned.collision_pairs.len(),
            complete.collision_pairs.len()
        );
    }

    /// The fused kernel entry answers exactly what the two split entries
    /// answer: same verdict, same magnitude bits, same reported probes.
    ///
    /// This is the equivalence the `fused-pair-query` arm rests on, and it is
    /// asserted here rather than inferred from the trait's default body,
    /// because the default body is what a *future* kernel is invited to
    /// override. A kernel that overrides [`ExplorationKernel::pair_row`] to
    /// share a traversal has to keep passing this, or the arm stops being a
    /// measurement of the seam and becomes a change of answer.
    ///
    /// The ladder walks from deep overlap out to clear separation so the sweep
    /// crosses the contact boundary, which is where a fused implementation that
    /// reordered its work would first disagree.
    #[test]
    fn the_fused_pair_row_reproduces_the_split_pair_query() {
        let first_polygon = square(10.0);
        let second_polygon = l_shape();
        let pieces = [
            GeneralFastPiece {
                id: "first",
                polygon: &first_polygon,
                allow_rotation: true,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "second",
                polygon: &second_polygon,
                allow_rotation: true,
                allow_mirror: false,
            },
        ];
        let fast_settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let (catalog, _) = build_surrogate_catalog(
            &pieces,
            fast_settings,
            SurrogateCatalogMode::ZeroDegreeOnly,
            None,
        )
        .unwrap();
        let shape = |index: usize| {
            catalog
                .orientations
                .get(&(catalog.geometry_class_by_input[index], angle_key(0.0), false))
                .expect("zero-degree surrogate")
        };
        let mut kernel = LegacyKernel::default();
        let mut separated = 0usize;
        let mut colliding = 0usize;
        for step in 0..40 {
            let offset = step as f64 * 0.75;
            let first = PosedShape::new(shape(0), 20.0, 20.0);
            let second = PosedShape::new(shape(1), 20.0 + offset, 20.0);

            let mut split_probes = KernelProbes::default();
            let split_collides = kernel.pair_collides(first, second, &mut split_probes);
            let split_pressure = split_collides.then(|| kernel.pair_pressure(first, second));

            let mut fused_probes = KernelProbes::default();
            let fused = kernel.pair_row(first, second, &mut fused_probes);

            assert_eq!(
                fused.collides(),
                split_collides,
                "verdict at offset {offset}"
            );
            assert_eq!(fused_probes, split_probes, "probes at offset {offset}");
            assert_eq!(
                fused.penalty().to_bits(),
                split_pressure.unwrap_or(0.0).to_bits(),
                "magnitude at offset {offset}: fused {:.17e} against split {:?}",
                fused.penalty(),
                split_pressure,
            );
            if split_collides {
                colliding += 1;
            } else {
                separated += 1;
            }
        }
        // A ladder that never crossed the boundary would pass vacuously.
        assert!(
            colliding > 0 && separated > 0,
            "the ladder must cross contact: {colliding} colliding, {separated} separated"
        );
    }

    /// The proxy row cache is only sound if a hit returns exactly what the
    /// deriving path would have computed and a pose change is a miss. Both
    /// halves are load-bearing: a hit that rounded differently would move a
    /// collision verdict, and a stale hit would answer about the wrong pose.
    #[test]
    fn proxy_row_cache_hits_and_misses_match_the_deriving_path() {
        let polygon = square(10.0);
        let pieces = [GeneralFastPiece {
            id: "first",
            polygon: &polygon,
            allow_rotation: true,
            allow_mirror: false,
        }];
        let fast_settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let (catalog, _) = build_surrogate_catalog(
            &pieces,
            fast_settings,
            SurrogateCatalogMode::ZeroDegreeOnly,
            None,
        )
        .unwrap();
        let shape = catalog
            .orientations
            .get(&(catalog.geometry_class_by_input[0], angle_key(0.0), false))
            .expect("zero-degree confirmation surrogate");

        let mut cache = ProxyRowCache::new(pieces.len());
        let poses = [
            RelaxedPlacement {
                input_index: 0,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: 12.0,
                translate_y: 30.0,
            },
            RelaxedPlacement {
                input_index: 0,
                rotation_deg: 37.5,
                mirrored: false,
                translate_x: 12.0,
                translate_y: 30.0,
            },
            RelaxedPlacement {
                input_index: 0,
                rotation_deg: 37.5,
                mirrored: false,
                translate_x: 12.000_001,
                translate_y: 30.0,
            },
        ];
        // Every pose, then every pose again in the same order, so the second
        // pass is served entirely from stored rows.
        for placement in poses.iter().chain(poses.iter()) {
            let derived = transformed_surrogate_bounds(
                shape,
                PoleTransform::new(
                    placement.rotation_deg,
                    placement.translate_x,
                    placement.translate_y,
                ),
            );
            let cached = cache.bounds_for(shape, placement);
            assert_eq!(cached.min_x.to_bits(), derived.min_x.to_bits());
            assert_eq!(cached.min_y.to_bits(), derived.min_y.to_bits());
            assert_eq!(cached.max_x.to_bits(), derived.max_x.to_bits());
            assert_eq!(cached.max_y.to_bits(), derived.max_y.to_bits());
        }

        // A repeated read of the pose the cache is holding is a hit, and a hit
        // is the same value.
        let held = cache.bounds_for(shape, &poses[2]);
        assert_eq!(
            held.min_x.to_bits(),
            cache.bounds_for(shape, &poses[2]).min_x.to_bits()
        );
        // A piece index the cache was not sized for still answers, by deriving.
        let unknown = RelaxedPlacement {
            input_index: pieces.len(),
            ..poses[0].clone()
        };
        let derived = transformed_surrogate_bounds(
            shape,
            PoleTransform::new(
                unknown.rotation_deg,
                unknown.translate_x,
                unknown.translate_y,
            ),
        );
        assert_eq!(
            cache.bounds_for(shape, &unknown).max_y.to_bits(),
            derived.max_y.to_bits()
        );
    }

    #[test]
    fn shared_pair_nfps_preserve_tracker_and_lane_budget_semantics() {
        let polygon = square(10.0);
        let pieces = [
            GeneralFastPiece {
                id: "first",
                polygon: &polygon,
                allow_rotation: false,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "second",
                polygon: &polygon,
                allow_rotation: false,
                allow_mirror: false,
            },
        ];
        let fast_settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let incumbent =
            crate::search::general_fast::construct_short_side_first(&pieces, fast_settings)
                .unwrap();
        let (shared_catalog, shared_work) = build_surrogate_catalog(
            &pieces,
            fast_settings,
            SurrogateCatalogMode::CurrentAssignment,
            Some(&incumbent),
        )
        .unwrap();
        assert_eq!(shared_work.shared_pair_nfp_entries, 1);
        assert_eq!(shared_work.shared_pair_nfp_components, 4);
        let cold_catalog = Arc::new(SurrogateCatalog {
            geometry_class_by_input: shared_catalog.geometry_class_by_input.clone(),
            orientations: shared_catalog.orientations.clone(),
            shared_pair_nfps: BTreeMap::new(),
        });
        let mut relaxed_settings = GeneralRelaxedSettings::mixed_61_probe(0, 1);
        relaxed_settings.angle_seed_policy = GeneralRelaxedAngleSeedPolicy::CurrentOnly;
        relaxed_settings.pressure_model = GeneralRelaxedPressureModel::DirectionalPenetration;
        let state = RelaxedState {
            placements: vec![
                RelaxedPlacement {
                    input_index: 0,
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_x: 20.0,
                    translate_y: 20.0,
                },
                RelaxedPlacement {
                    input_index: 1,
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_x: 20.0,
                    translate_y: 20.0,
                },
            ],
            strip_depth_mm: 100.0,
        };
        let mut cold = LegacyLaneSearch::new(&pieces, fast_settings, relaxed_settings, 0, cold_catalog);
        let mut shared = LegacyLaneSearch::new(
            &pieces,
            fast_settings,
            relaxed_settings,
            0,
            shared_catalog.clone(),
        );
        let cold_tracker = cold.score_state(&state).unwrap();
        let shared_tracker = shared.score_state(&state).unwrap();
        assert_eq!(shared_tracker, cold_tracker);
        assert_eq!(
            shared.pair_nfp_cache_components,
            cold.pair_nfp_cache_components
        );
        assert_eq!(cold.counters.pair_nfp_builds, 1);
        assert_eq!(shared.counters.pair_nfp_builds, 0);
        assert_eq!(shared.counters.shared_pair_nfp_adoptions, 1);
        let key = shared
            .pair_nfp_key(&state.placements[0], &state.placements[1])
            .unwrap();
        assert!(Arc::ptr_eq(
            shared.pair_nfp_cache.get(&key).unwrap(),
            shared_catalog.shared_pair_nfps.get(&key).unwrap(),
        ));
        let components = shared.pair_nfp_cache_components;
        assert!(!directional_nfp_preflight_fits(
            0,
            components,
            components,
            false,
            MAX_NFP_COMPONENTS_PER_MOVE,
            components - 1,
        ));
        assert!(directional_nfp_preflight_fits(
            0, components, components, true, components, components,
        ));
        assert_ne!(
            (key.0, key.1, false, key.3, key.4, true),
            (key.3, key.4, true, key.0, key.1, false),
        );
        assert_ne!(
            (key.0, key.1, key.2, key.3, key.4, key.5),
            (key.0, key.1 + 1, key.2, key.3, key.4, key.5),
        );
    }

    #[test]
    fn lane_disruptions_preserve_control_and_cycle_restart_depth() {
        assert_eq!(
            (0..8).map(lane_disruption_count).collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 1, 2, 3, 1]
        );
    }

    #[test]
    fn frontier_blockers_use_transformed_source_bounds() {
        let wide = PolygonSet::from_outer(vec![
            point(0.0, 0.0),
            point(100.0, 0.0),
            point(100.0, 10.0),
            point(0.0, 10.0),
        ])
        .unwrap();
        let square = square(20.0);
        let pieces = [
            GeneralFastPiece {
                id: "rotated-wide",
                polygon: &wide,
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "translated-square",
                polygon: &square,
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let state = RelaxedState {
            placements: vec![
                RelaxedPlacement {
                    input_index: 0,
                    rotation_deg: 90.0,
                    mirrored: false,
                    translate_x: 0.0,
                    translate_y: 0.0,
                },
                RelaxedPlacement {
                    input_index: 1,
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_x: 0.0,
                    translate_y: 50.0,
                },
            ],
            strip_depth_mm: 120.0,
        };

        assert_eq!(high_frontier_blockers(&state, &pieces, 2), vec![0, 1]);
    }

    #[test]
    fn publication_reducer_selects_the_shallower_exact_lane() {
        let polygon = square(10.0);
        let pieces = [GeneralFastPiece {
            id: "square",
            polygon: &polygon,
            allow_rotation: true,
            allow_mirror: true,
        }];
        let mut diagnostics = GeneralRelaxedDiagnostics::default();
        let selected = select_lane_for_publication(
            &pieces,
            GeneralFastSettings::deterministic_test(100.0, 100.0),
            vec![feasible_lane(0, 10.0, 30.0), feasible_lane(1, 10.0, 10.0)],
            &mut diagnostics,
        );
        assert_eq!(selected.outcome.selected_lane, 1);
        assert!(matches!(
            selected.validation,
            ExactLaneValidation::Accepted { .. }
        ));
        assert_eq!(diagnostics.surrogate_feasible_states, 2);
        assert_eq!(diagnostics.exact_rejected_states, 0);
    }

    #[test]
    fn publication_reducer_uses_the_canonical_key_for_equal_metrics() {
        let polygon = square(10.0);
        let pieces = [GeneralFastPiece {
            id: "square",
            polygon: &polygon,
            allow_rotation: true,
            allow_mirror: true,
        }];
        let mut diagnostics = GeneralRelaxedDiagnostics::default();
        let selected = select_lane_for_publication(
            &pieces,
            GeneralFastSettings::deterministic_test(100.0, 100.0),
            vec![feasible_lane(0, 10.0, 10.0), feasible_lane(1, 20.0, 10.0)],
            &mut diagnostics,
        );
        assert_eq!(selected.outcome.selected_lane, 0);
    }

    #[test]
    fn publication_reducer_skips_an_exact_rejection() {
        let polygon = square(10.0);
        let pieces = [GeneralFastPiece {
            id: "square",
            polygon: &polygon,
            allow_rotation: true,
            allow_mirror: true,
        }];
        let mut diagnostics = GeneralRelaxedDiagnostics::default();
        let selected = select_lane_for_publication(
            &pieces,
            GeneralFastSettings::deterministic_test(100.0, 100.0),
            vec![feasible_lane(0, -1.0, 1.0), feasible_lane(1, 20.0, 20.0)],
            &mut diagnostics,
        );
        assert_eq!(selected.outcome.selected_lane, 1);
        assert_eq!(diagnostics.exact_rejected_states, 1);
        assert!(matches!(
            selected.validation,
            ExactLaneValidation::Accepted { .. }
        ));
    }

    #[test]
    fn atomic_replacement_delta_matches_full_rescore() {
        let polygons = [square(10.0), square(8.0), square(6.0)];
        let pieces = [
            GeneralFastPiece {
                id: "large",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "medium",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "small",
                polygon: &polygons[2],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let fast_settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let relaxed_settings = GeneralRelaxedSettings::mixed_61_probe(0, 1);
        let (catalog, _) = build_surrogate_catalog(
            &pieces,
            fast_settings,
            SurrogateCatalogMode::StructuredGrid,
            None,
        )
        .unwrap();
        let mut search = LegacyLaneSearch::new(&pieces, fast_settings, relaxed_settings, 0, catalog);
        let state = RelaxedState {
            placements: vec![
                RelaxedPlacement {
                    input_index: 0,
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_x: 0.0,
                    translate_y: 0.0,
                },
                RelaxedPlacement {
                    input_index: 1,
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_x: 5.0,
                    translate_y: 0.0,
                },
                RelaxedPlacement {
                    input_index: 2,
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_x: 30.0,
                    translate_y: 0.0,
                },
            ],
            strip_depth_mm: 100.0,
        };
        let base = search.score_state(&state).unwrap();
        let replacements = vec![
            (
                0,
                RelaxedPlacement {
                    translate_x: 30.0,
                    ..state.placements[0].clone()
                },
            ),
            (
                2,
                RelaxedPlacement {
                    translate_x: 0.0,
                    ..state.placements[2].clone()
                },
            ),
        ];
        let incremental = search
            .score_after_replacements(&state, &base, &replacements)
            .unwrap();
        let mut replaced = state;
        for (index, placement) in replacements {
            replaced.placements[index] = placement;
        }
        let full = search.score_state(&replaced).unwrap();
        assert_eq!(incremental.boundary_violations, full.boundary_violations);
        assert_eq!(
            incremental
                .collision_pairs
                .iter()
                .map(|(first, second, _)| (*first, *second))
                .collect::<Vec<_>>(),
            full.collision_pairs
                .iter()
                .map(|(first, second, _)| (*first, *second))
                .collect::<Vec<_>>()
        );
        assert!((incremental.boundary_loss - full.boundary_loss).abs() < 1e-9);
        assert!((incremental.weighted_loss - full.weighted_loss).abs() < 1e-9);
        assert!((incremental.common_loss() - full.common_loss()).abs() < 1e-9);
        assert_eq!(incremental.boundaries.len(), full.boundaries.len());
        assert_eq!(incremental.pairs.len(), full.pairs.len());
        for index in 0..incremental.piece_count {
            assert_eq!(
                incremental.boundaries[index].violations,
                full.boundaries[index].violations
            );
            assert!(
                (incremental.boundaries[index].raw_loss - full.boundaries[index].raw_loss).abs()
                    < 1e-9
            );
            assert!(
                (incremental.incident_raw_loss[index] - full.incident_raw_loss[index]).abs() < 1e-9
            );
        }
        for first in 0..incremental.piece_count {
            for second in (first + 1)..incremental.piece_count {
                assert!(
                    (incremental.pair(first, second).raw_loss - full.pair(first, second).raw_loss)
                        .abs()
                        < 1e-9
                );
            }
        }
    }

    #[test]
    fn move_update_recomputes_weighted_loss_without_cancellation() {
        let mut score = PairTracker {
            piece_count: 3,
            boundaries: vec![
                BoundaryEntry {
                    violations: 0,
                    raw_loss: 0.0,
                };
                3
            ],
            pairs: vec![
                PairEntry {
                    raw_loss: 1.0e16,
                    guided_weight: 1.0,
                    normalization_scale: 1.0,
                },
                PairEntry {
                    raw_loss: 0.0,
                    guided_weight: 1.0,
                    normalization_scale: 1.0,
                },
                PairEntry {
                    raw_loss: 1.0,
                    guided_weight: 1.0,
                    normalization_scale: 1.0,
                },
            ],
            incident_raw_loss: vec![1.0e16, 1.0e16, 1.0],
            boundary_violations: 0,
            boundary_loss: 0.0,
            collision_pairs: vec![(0, 1, 1.0e16), (1, 2, 1.0)],
            weighted_loss: 1.0e16,
        };
        update_score_after_move(
            &mut score,
            0,
            (0, 0.0),
            MovedRowDelta {
                boundary_violations: 0,
                boundary_loss: 0.0,
                collision_pairs: Vec::new(),
                weighted_loss: 0.0,
                rows: MovedRows::Complete,
            },
            &BTreeMap::new(),
            &mut Vec::new(),
        );
        assert_eq!(score.collision_pairs, vec![(1, 2, 1.0)]);
        assert_eq!(score.weighted_loss, 1.0);
    }

    /// The accepted-move merge has to reproduce `retain` + `extend` + sort, not
    /// merely produce a sorted list: the sweep's collision list is compared
    /// element-wise against a from-scratch score, so a permutation of equal
    /// content would read as a real disagreement.
    #[test]
    fn moved_row_merge_reproduces_retain_extend_and_sort() {
        let incumbent = vec![
            (0usize, 3usize, 1.0f64),
            (1, 2, 2.0),
            (1, 4, 3.0),
            (2, 3, 4.0),
            (3, 5, 5.0),
        ];
        for input_index in 0..6usize {
            let neighbour = (input_index + 1) % 6;
            let single = ordered_pair(input_index, neighbour);
            for row in [
                Vec::new(),
                vec![(single.0, single.1, 9.0)],
                (0..6)
                    .filter(|fixed| *fixed != input_index)
                    .map(|fixed| {
                        let pair = ordered_pair(input_index, fixed);
                        (pair.0, pair.1, 7.0)
                    })
                    .collect::<Vec<_>>(),
            ] {
                let mut row = row;
                row.sort_by_key(|(first, second, _)| (*first, *second));
                let mut expected = incumbent.clone();
                expected
                    .retain(|(first, second, _)| *first != input_index && *second != input_index);
                expected.extend(row.iter().copied());
                expected.sort_by_key(|(first, second, _)| (*first, *second));

                let mut actual = incumbent.clone();
                let mut scratch = Vec::new();
                merge_sorted_moved_row(&mut actual, input_index, &row, &mut scratch);
                assert_eq!(actual, expected, "input index {input_index}");
            }
        }
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn coupled_raw_minimum_transition_matches_strict_thresholds() {
        assert_eq!(
            raw_minimum_transition(100.0, 100.0),
            RawMinimumTransition::NoImprovement
        );
        assert_eq!(
            raw_minimum_transition(99.0, 100.0),
            RawMinimumTransition::MinorImprovement
        );
        assert_eq!(
            raw_minimum_transition(98.0, 100.0),
            RawMinimumTransition::MinorImprovement
        );
        assert_eq!(
            raw_minimum_transition(97.999, 100.0),
            RawMinimumTransition::SubstantialImprovement
        );
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn coupled_canonical_tracker_agreement_uses_authoritative_rows() {
        let mut canonical = feasible_tracker(2);
        canonical.boundaries[0] = BoundaryEntry {
            violations: 1,
            raw_loss: 4.0,
        };
        canonical.boundary_violations = 1;
        canonical.boundary_loss = 4.0;
        canonical.pairs[0].raw_loss = 3.0;
        canonical.incident_raw_loss = vec![3.0, 3.0];
        canonical.collision_pairs = vec![(0, 1, 3.0)];
        canonical.weighted_loss = 7.0;

        let mut derived_drift = canonical.clone();
        derived_drift.boundary_loss = f64::from_bits(canonical.boundary_loss.to_bits() + 8);
        derived_drift.incident_raw_loss[0] =
            f64::from_bits(canonical.incident_raw_loss[0].to_bits() + 8);
        derived_drift.weighted_loss =
            f64::from_bits(canonical.weighted_loss.to_bits().saturating_add(8));
        assert!(authoritative_raw_tracker_disagreement(&canonical, &derived_drift).is_none());
        assert!(raw_tracker_disagreement(
            &canonical,
            &derived_drift,
            CoupledRollbackComparison::Exact,
            &mut RollbackComparisonTally::default(),
        )
        .is_some());

        let mut changed = canonical.clone();
        changed.boundaries[0].raw_loss = 4.5;
        assert_eq!(
            authoritative_raw_tracker_disagreement(&canonical, &changed).as_deref(),
            Some("boundary rows differ")
        );
        changed = canonical.clone();
        changed.pairs[0].raw_loss = 3.5;
        assert_eq!(
            authoritative_raw_tracker_disagreement(&canonical, &changed).as_deref(),
            Some("pair rows differ")
        );
        changed = canonical.clone();
        changed.pairs[0].normalization_scale = 2.0;
        assert_eq!(
            authoritative_raw_tracker_disagreement(&canonical, &changed).as_deref(),
            Some("pair rows differ")
        );
        changed = canonical.clone();
        changed.collision_pairs[0].2 = 3.5;
        assert_eq!(
            authoritative_raw_tracker_disagreement(&canonical, &changed).as_deref(),
            Some("collision rows differ")
        );
        changed = canonical.clone();
        changed.boundary_violations = 2;
        assert!(authoritative_raw_tracker_disagreement(&canonical, &changed)
            .as_deref()
            .is_some_and(|reason| reason.starts_with("boundary violation count")));
    }

    /// The tracker the incremental path builds when the *last moved* piece read
    /// a pair, next to the one a complete rescore builds reading the same pair
    /// from its lower-indexed side: one `f32` ulp apart in the pole pressure,
    /// with every derived sum carrying that difference forward.
    #[cfg(feature = "jagua-experimental")]
    fn pole_rounding_pair(ulps: u32) -> (PairTracker, PairTracker) {
        let pressure = f64::from(3.25f32);
        let rounded = f64::from(f32::from_bits(3.25f32.to_bits() + ulps));
        let mut canonical = feasible_tracker(2);
        canonical.pairs[0].raw_loss = pressure;
        canonical.incident_raw_loss = vec![pressure, pressure];
        canonical.collision_pairs = vec![(0, 1, pressure)];
        canonical.weighted_loss = pressure;

        let mut drifted = canonical.clone();
        drifted.pairs[0].raw_loss = rounded;
        drifted.incident_raw_loss = vec![rounded, rounded];
        drifted.collision_pairs = vec![(0, 1, rounded)];
        drifted.weighted_loss = rounded;
        (canonical, drifted)
    }

    /// The policy pin. The bit-exact rule is what every arm outside the mode-26
    /// clamp is judged by, and it rejects the pole-rounding asymmetry; the
    /// tolerant rule accepts it and says so in its tally.
    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn rollback_comparison_separates_pole_rounding_from_real_disagreement() {
        let (canonical, drifted) = pole_rounding_pair(1);

        let mut exact_tally = RollbackComparisonTally::default();
        assert_eq!(
            authoritative_raw_tracker_disagreement_under(
                &canonical,
                &drifted,
                CoupledRollbackComparison::Exact,
                &mut exact_tally,
            )
            .as_deref(),
            Some("collision rows differ"),
            "the exact rule must keep rejecting the low-bit asymmetry"
        );
        assert_eq!(exact_tally.tolerated, 0);
        assert_eq!(exact_tally.max_pressure_ulps, 1);

        let mut tolerant_tally = RollbackComparisonTally::default();
        assert!(
            authoritative_raw_tracker_disagreement_under(
                &canonical,
                &drifted,
                CoupledRollbackComparison::ToleratesPoleRounding,
                &mut tolerant_tally,
            )
            .is_none(),
            "the tolerant rule must accept one reading of one measurement"
        );
        assert!(tolerant_tally.tolerated > 0);
        assert_eq!(tolerant_tally.max_pressure_ulps, 1);

        // The strict-derived variant additionally compares the per-piece
        // incident *sums*, which are `f64` accumulations and so are held to one
        // `f64` ulp under both policies. One `f32` ulp is about 2^29 of those,
        // so the same drift that the pressure rows tolerate is refused here.
        // See `RollbackMagnitude`.
        assert_eq!(
            raw_tracker_disagreement(
                &canonical,
                &drifted,
                CoupledRollbackComparison::ToleratesPoleRounding,
                &mut RollbackComparisonTally::default(),
            )
            .as_deref()
            .map(|reason| reason.starts_with("incident loss 0")),
            Some(true),
        );
    }

    /// The scoping pin for the tolerant policy: the `f32`-denominated budget
    /// reaches pole pressures and nothing else.
    ///
    /// A boundary penalty is `f64` area arithmetic all the way down, so an
    /// `f32`-ulp budget spent on it would admit gaps of order 1e10 `f64` ulps -
    /// which is why provenance is a parameter of the comparison rather than a
    /// property inferred from the value.
    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn tolerant_rollback_budget_is_scoped_to_pole_pressure_magnitudes() {
        let tolerant = CoupledRollbackComparison::ToleratesPoleRounding;
        let pressure = f64::from(3.25f32);
        let one_f32_ulp_away = f64::from(f32::from_bits(3.25f32.to_bits() + 1));
        let one_f64_ulp_away = f64::from_bits(pressure.to_bits() + 1);

        // Pole pressure: the `f32` floor is real, so one `f32` ulp is rounding.
        let mut tally = RollbackComparisonTally::default();
        assert!(rollback_losses_agree(
            pressure,
            one_f32_ulp_away,
            RollbackMagnitude::PairPressure,
            tolerant,
            &mut tally,
        ));
        assert_eq!(tally.tolerated, 1);

        // The identical numbers, read as an `f64`-native magnitude: the same
        // gap is now some 2^29 `f64` ulps and is refused.
        let mut tally = RollbackComparisonTally::default();
        assert!(!rollback_losses_agree(
            pressure,
            one_f32_ulp_away,
            RollbackMagnitude::NativeF64,
            tolerant,
            &mut tally,
        ));
        assert_eq!(tally.tolerated, 0);
        assert_eq!(
            tally.max_pressure_ulps, 1,
            "a refused gap is still measured and reported"
        );

        // An `f64`-native magnitude keeps its accumulation-order allowance.
        let mut tally = RollbackComparisonTally::default();
        assert!(rollback_losses_agree(
            pressure,
            one_f64_ulp_away,
            RollbackMagnitude::NativeF64,
            tolerant,
            &mut tally,
        ));
        assert_eq!(tally.tolerated, 1);

        // A pair row carrying a genuinely `f64`-native pressure - what the
        // `pole_overlap_pressure` models produce - has no `f32` floor to widen
        // to, and falls back to the same one-`f64`-ulp rule.
        let native = 3.2500000000000004_f64;
        assert!(!is_exactly_representable_as_f32(native));
        let mut tally = RollbackComparisonTally::default();
        assert!(!rollback_losses_agree(
            native,
            native + 1e-7,
            RollbackMagnitude::PairPressure,
            tolerant,
            &mut tally,
        ));
        assert_eq!(tally.tolerated, 0);
        let mut tally = RollbackComparisonTally::default();
        assert!(rollback_losses_agree(
            native,
            f64::from_bits(native.to_bits() + 1),
            RollbackMagnitude::PairPressure,
            tolerant,
            &mut tally,
        ));

        // `Exact` is unmoved by any of this, for either provenance.
        for magnitude in [
            RollbackMagnitude::PairPressure,
            RollbackMagnitude::NativeF64,
        ] {
            let mut tally = RollbackComparisonTally::default();
            assert!(!rollback_losses_agree(
                pressure,
                one_f32_ulp_away,
                magnitude,
                CoupledRollbackComparison::Exact,
                &mut tally,
            ));
            assert_eq!(tally.tolerated, 0);
            assert_eq!(tally.max_pressure_ulps, 1);
        }
    }

    /// Tolerance is a rounding allowance, not a licence. Structure is still
    /// compared bit for bit, and a magnitude beyond the budget is still an
    /// abort.
    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn tolerant_rollback_comparison_still_rejects_structural_and_large_gaps() {
        let (canonical, _) = pole_rounding_pair(1);
        let tolerant = CoupledRollbackComparison::ToleratesPoleRounding;

        let mut changed = canonical.clone();
        changed.collision_pairs[0].1 = 2;
        assert_eq!(
            authoritative_raw_tracker_disagreement_under(
                &canonical,
                &changed,
                tolerant,
                &mut RollbackComparisonTally::default(),
            )
            .as_deref(),
            Some("collision rows differ"),
            "a different colliding pair is not rounding"
        );

        let mut changed = canonical.clone();
        changed.collision_pairs.push((0, 2, 1.0));
        assert_eq!(
            authoritative_raw_tracker_disagreement_under(
                &canonical,
                &changed,
                tolerant,
                &mut RollbackComparisonTally::default(),
            )
            .as_deref(),
            Some("collision rows differ")
        );

        let mut changed = canonical.clone();
        changed.boundary_violations = 2;
        assert!(authoritative_raw_tracker_disagreement_under(
            &canonical,
            &changed,
            tolerant,
            &mut RollbackComparisonTally::default(),
        )
        .as_deref()
        .is_some_and(|reason| reason.starts_with("boundary violation count")));

        let (canonical, far) = pole_rounding_pair(COUPLED_ROLLBACK_PRESSURE_ULP_BUDGET + 1);
        let mut tally = RollbackComparisonTally::default();
        assert_eq!(
            authoritative_raw_tracker_disagreement_under(&canonical, &far, tolerant, &mut tally)
                .as_deref(),
            Some("collision rows differ"),
            "a gap past the budget is a real disagreement"
        );
        assert_eq!(tally.tolerated, 0);
        assert_eq!(
            tally.max_pressure_ulps,
            COUPLED_ROLLBACK_PRESSURE_ULP_BUDGET + 1,
            "the observed gap is reported even when it is refused"
        );
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn pressure_ulp_distance_measures_the_f32_floor() {
        assert_eq!(pressure_ulp_distance(1.0, 1.0), 0);
        // An `f64`-native quantity one `f64` ulp apart collapses to one `f32`.
        assert_eq!(
            pressure_ulp_distance(1.0, f64::from_bits(1.0f64.to_bits() + 1)),
            0
        );
        assert_eq!(
            pressure_ulp_distance(
                f64::from(1.0f32),
                f64::from(f32::from_bits(1.0f32.to_bits() + 3))
            ),
            3
        );
        assert_eq!(pressure_ulp_distance(1.0, -1.0), u32::MAX);
        assert_eq!(pressure_ulp_distance(1.0, f64::NAN), u32::MAX);
        assert_eq!(pressure_ulp_distance(1.0, f64::INFINITY), u32::MAX);
    }

    /// Where each policy is actually used. The clamped mode-26 rung labels its
    /// arms `toleratesPoleRounding`; the ordinary relaxed entry point that
    /// every other mode goes through labels nothing at all, which is the
    /// serialized form of "bit-exact, as before".
    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn only_the_clamped_ladder_arm_runs_the_tolerant_comparison() {
        assert_eq!(
            CoupledRollbackComparison::default(),
            CoupledRollbackComparison::Exact
        );
        assert_eq!(CoupledRollbackComparison::Exact.label(), None);

        let (polygons, fast_settings) = two_piece_ladder_fixture();
        let pieces = [
            GeneralFastPiece {
                id: "large",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "small",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let mut settings = coupled_experiment_test_settings(9);
        settings.persistent_vacancy_target_depth_mm = Some(1.0);
        let parent = two_piece_ladder_parent(&polygons, fast_settings);

        let result = JobPool::new(Some(1))
            .run_scoped(|| run_ladder_compression(&pieces, fast_settings, settings, &parent, None));
        let ladder = result
            .ladder_compression
            .expect("the ladder runs on this fixture");
        let mut attempted_arms = 0usize;
        for step in &ladder.steps {
            for arm in &step.arms {
                if !arm.separator_attempted {
                    continue;
                }
                attempted_arms += 1;
                assert_eq!(
                    arm.rollback_comparison.as_deref(),
                    Some("toleratesPoleRounding"),
                    "every clamped rung arm opts in"
                );
            }
        }
        assert!(
            attempted_arms > 0,
            "the ladder must attempt at least one clamped arm"
        );

        // The same pieces through the entry point every other mode uses.
        let incumbent = JobPool::new(Some(1))
            .run_scoped(|| construct_short_side_first(&pieces, fast_settings))
            .unwrap();
        let mut exact_settings = settings;
        exact_settings.persistent_vacancy_mode = 0;
        exact_settings.persistent_vacancy_target_depth_mm = None;
        let coupled = JobPool::new(Some(1))
            .run_scoped(|| {
                improve_complete_layout(&pieces, fast_settings, exact_settings, &incumbent)
            })
            .expect("the relaxed entry point runs on this fixture")
            .diagnostics
            .coupled_dynamic_separator
            .expect("the coupled separator is armed");
        for arm in [
            Some(&coupled.control),
            Some(&coupled.treatment),
            coupled.boundary_projection_treatment.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            assert_eq!(
                arm.rollback_comparison, None,
                "the ordinary entry point never widens the comparison"
            );
            assert!(arm
                .targets
                .iter()
                .all(|target| target.rollback_disagreements_tolerated == 0));
        }
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn coupled_canonical_rollback_installs_rescore_before_strike_state() {
        let mut restored = feasible_tracker(2);
        restored.boundaries[0] = BoundaryEntry {
            violations: 1,
            raw_loss: 4.0,
        };
        restored.boundary_violations = 1;
        restored.boundary_loss = 4.0;
        restored.pairs[0].raw_loss = 3.0;
        restored.incident_raw_loss = vec![3.0, 3.0];
        restored.collision_pairs = vec![(0, 1, 3.0)];
        restored.weighted_loss = 7.0;
        let mut minimum = restored.clone();
        minimum.boundary_loss = 99.0;
        minimum.incident_raw_loss = vec![99.0, 99.0];
        minimum.weighted_loss = 102.0;
        let minimum_state = RelaxedState {
            placements: vec![
                RelaxedPlacement {
                    input_index: 0,
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_x: 1.0,
                    translate_y: 2.0,
                },
                RelaxedPlacement {
                    input_index: 1,
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_x: 3.0,
                    translate_y: 4.0,
                },
            ],
            strip_depth_mm: 10.0,
        };
        let mut master = LaneOutcome {
            state: RelaxedState {
                placements: Vec::new(),
                strip_depth_mm: 999.0,
            },
            score: feasible_tracker(0),
            weights: BTreeMap::new(),
            counters: WorkCounters::default(),
            selected_lane: 7,
            restart_disruptions: 3,
        };
        let weights = BTreeMap::new();
        let mut strikes = 2;
        let mut strike_start_raw_loss = 100.0;

        let reached_limit = install_canonical_coupled_rollback(
            restored.clone(),
            &minimum_state,
            &mut minimum,
            &mut master,
            &weights,
            &mut strikes,
            &mut strike_start_raw_loss,
        );

        assert!(!reached_limit);
        assert_eq!(strikes, 0);
        assert_eq!(strike_start_raw_loss, 7.0);
        assert_eq!(minimum, restored);
        assert_eq!(master.score, restored);
        assert_eq!(
            canonical_state_key(&master.state),
            canonical_state_key(&minimum_state)
        );
        assert_eq!(master.selected_lane, 0);
        assert_eq!(master.restart_disruptions, 0);
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn exact_boundary_projection_clamps_rotated_concave_geometry_on_every_side() {
        let polygon = l_shape();
        let pieces = [GeneralFastPiece {
            id: "concave",
            polygon: &polygon,
            allow_rotation: true,
            allow_mirror: true,
        }];
        let mut settings = GeneralFastSettings::deterministic_test(20.0, 20.0);
        settings.total_padding_mm = 2.0;
        let inset = collision_sheet_inset_mm(settings);

        for (translate_x, translate_y) in [(-100.0, 5.0), (100.0, 5.0), (5.0, -100.0), (5.0, 100.0)]
        {
            let mut state = RelaxedState {
                placements: vec![RelaxedPlacement {
                    input_index: 0,
                    rotation_deg: 33.125,
                    mirrored: true,
                    translate_x,
                    translate_y,
                }],
                strip_depth_mm: 20.0,
            };
            let projected = project_piece_into_exact_boundary(&state, &pieces, settings, 0)
                .expect("rotated concave contour fits the exact sheet");
            state.placements[0] = projected.clone();
            let collision = polygon
                .transformed(
                    projected.rotation_deg,
                    projected.mirrored,
                    projected.translate_x,
                    projected.translate_y,
                )
                .and_then(|geometry| geometry.offset(collision_expansion_mm(settings)))
                .expect("projected collision geometry");
            assert!(collision.fits_rect(inset, inset, 20.0 - inset, 20.0 - inset));
            let repeated = project_piece_into_exact_boundary(&state, &pieces, settings, 0)
                .expect("projection is idempotent");
            assert_eq!(projected.rotation_deg, repeated.rotation_deg);
            assert_eq!(projected.mirrored, repeated.mirrored);
            assert_eq!(projected.translate_x, repeated.translate_x);
            assert_eq!(projected.translate_y, repeated.translate_y);
        }
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn exact_boundary_projection_rejects_an_empty_inner_fit() {
        let polygon = square(30.0);
        let pieces = [GeneralFastPiece {
            id: "oversized",
            polygon: &polygon,
            allow_rotation: true,
            allow_mirror: true,
        }];
        let state = RelaxedState {
            placements: vec![RelaxedPlacement {
                input_index: 0,
                rotation_deg: 17.0,
                mirrored: false,
                translate_x: 0.0,
                translate_y: 0.0,
            }],
            strip_depth_mm: 20.0,
        };
        let result = project_piece_into_exact_boundary(
            &state,
            &pieces,
            GeneralFastSettings::deterministic_test(20.0, 20.0),
            0,
        );
        let Err(error) = result else {
            panic!("an oversized contour has no exact inner fit");
        };
        assert!(error.contains("empty canonical inner-fit rectangle"));
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn coupled_feasibility_precedes_a_single_gls_update() {
        let feasible = feasible_tracker(2);
        assert_eq!(
            coupled_round_disposition(&feasible, 0.0),
            CoupledRoundDisposition::AcceptFeasible
        );

        let mut colliding = feasible_lane(0, 0.0, 0.0);
        colliding.state.placements.push(RelaxedPlacement {
            input_index: 1,
            rotation_deg: 0.0,
            mirrored: false,
            translate_x: 0.0,
            translate_y: 0.0,
        });
        colliding.score = PairTracker {
            piece_count: 2,
            boundaries: vec![
                BoundaryEntry {
                    violations: 0,
                    raw_loss: 0.0,
                };
                2
            ],
            pairs: vec![PairEntry {
                raw_loss: 10.0,
                guided_weight: 1.0,
                normalization_scale: 1.0,
            }],
            incident_raw_loss: vec![10.0, 10.0],
            boundary_violations: 0,
            boundary_loss: 0.0,
            collision_pairs: vec![(0, 1, 10.0)],
            weighted_loss: 10.0,
        };
        assert_eq!(
            coupled_round_disposition(&colliding.score, 12.0),
            CoupledRoundDisposition::ContinueInfeasible(
                RawMinimumTransition::SubstantialImprovement
            )
        );
        let mut weights = BTreeMap::new();
        apply_coupled_gls_update(&mut weights, &mut colliding);
        assert_eq!(weights.get(&(0, 1)).copied(), Some(1.1));
        assert_eq!(colliding.score.weighted_loss, 11.0);
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn coupled_auditor_reuses_variants_and_restores_worker_accounting() {
        let polygons = [square(10.0), square(8.0)];
        let pieces = [
            GeneralFastPiece {
                id: "large",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "small",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let fast_settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let relaxed_settings = coupled_test_settings(7);
        let (catalog, _) = build_surrogate_catalog(
            &pieces,
            fast_settings,
            SurrogateCatalogMode::ZeroDegreeOnly,
            None,
        )
        .unwrap();
        let hazard_catalog = Arc::new(JaguaHazardCatalog::new(&pieces, fast_settings).unwrap());
        let mut search = LegacyLaneSearch::new(&pieces, fast_settings, relaxed_settings, 7, catalog);
        search.hazard_catalog = Some(hazard_catalog);
        let worker = Mutex::new(search);
        let state = RelaxedState {
            placements: vec![
                RelaxedPlacement {
                    input_index: 0,
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_x: 10.0,
                    translate_y: 10.0,
                },
                RelaxedPlacement {
                    input_index: 1,
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_x: 40.0,
                    translate_y: 10.0,
                },
            ],
            strip_depth_mm: 100.0,
        };
        let (initial_score, initial_work) =
            coupled_auditor_score(&worker, &state, &BTreeMap::new(), 1);
        assert!(initial_score.unwrap().feasible());
        assert_eq!(initial_work.auditor_layout_loads, 1);
        assert_eq!(initial_work.auditor_index_builds, 1);
        {
            let worker = worker.lock().unwrap();
            assert_eq!(worker.counters.dynamic_layout_loads, 0);
            assert_eq!(worker.counters.dynamic_index_builds, 0);
            assert!(worker.hazard_index.is_some());
        }

        let outcome = worker.lock().unwrap().run_sweep(state.clone(), 0).unwrap();
        assert!(outcome.score.feasible());
        {
            let worker = worker.lock().unwrap();
            assert_eq!(worker.counters.dynamic_layout_loads, 1);
            assert_eq!(worker.counters.dynamic_index_builds, 0);
        }
        let (restored_score, restored_work) =
            coupled_auditor_score(&worker, &state, &BTreeMap::new(), 1);
        assert!(restored_score.unwrap().feasible());
        assert_eq!(restored_work.auditor_layout_loads, 1);
        assert_eq!(restored_work.auditor_index_builds, 0);
        let worker = worker.lock().unwrap();
        assert_eq!(worker.counters.dynamic_layout_loads, 1);
        assert_eq!(worker.counters.dynamic_index_builds, 0);
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn coupled_target_failure_is_structured_and_keeps_work() {
        let polygon = square(10.0);
        let pieces = [GeneralFastPiece {
            id: "square",
            polygon: &polygon,
            allow_rotation: true,
            allow_mirror: true,
        }];
        let fast_settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let incumbent = construct_short_side_first(&pieces, fast_settings).unwrap();
        let relaxed_settings = coupled_test_settings(11);
        let (catalog, _) = build_surrogate_catalog(
            &pieces,
            fast_settings,
            SurrogateCatalogMode::ZeroDegreeOnly,
            Some(&incumbent),
        )
        .unwrap();
        let target_seed = 19;
        let worker_seeds = (0..COUPLED_SEPARATOR_WORKERS)
            .map(|worker| derive_seed(target_seed, 0, worker))
            .collect::<Vec<_>>();
        let invalid = RelaxedState {
            placements: vec![RelaxedPlacement {
                input_index: 0,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: f64::NAN,
                translate_y: 0.0,
            }],
            strip_depth_mm: 99.0,
        };
        let hazard_catalog = Arc::new(JaguaHazardCatalog::new(&pieces, fast_settings).unwrap());
        let outcome = run_coupled_separator_target(
            &pieces,
            fast_settings,
            relaxed_settings,
            &incumbent,
            invalid,
            0,
            99.0,
            50.0,
            target_seed,
            23,
            worker_seeds,
            CoupledSeparatorArm::Control,
            CoupledRollbackRescorePolicy::StrictDerivedAgreement,
            CoupledRollbackComparison::Exact,
            false,
            catalog,
            hazard_catalog,
        )
        .unwrap();
        assert!(outcome.accepted.is_none());
        assert!(outcome
            .diagnostics
            .failure_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("initial full score")));
        assert_eq!(outcome.diagnostics.rounds, 0);
        assert_eq!(outcome.work.layout_loads, 1);
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn coupled_pair_visit_cap_falls_back_atomically() {
        let reason = coupled_separator_cap_reason(
            &[],
            COUPLED_SEPARATOR_WORKER_FULL_SCORE_PAIR_VISIT_CAP + 1,
            1,
        )
        .unwrap();
        assert_eq!(reason.as_deref(), Some("worker full-score pair-visit cap"));
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn disabled_coupled_separator_replays_without_diagnostics() {
        let polygon = square(10.0);
        let pieces = [GeneralFastPiece {
            id: "square",
            polygon: &polygon,
            allow_rotation: true,
            allow_mirror: true,
        }];
        let fast_settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let incumbent = construct_short_side_first(&pieces, fast_settings).unwrap();
        let mut settings = GeneralRelaxedSettings::mixed_61_probe(29, 1);
        settings.epochs = 1;
        settings.sweeps_per_epoch = 1;
        settings.global_samples_per_move = 1;
        settings.focused_samples_per_move = 1;
        settings.refinement_rounds = 1;
        settings.coupled_dynamic_separator = false;
        let first = improve_complete_layout(&pieces, fast_settings, settings, &incumbent).unwrap();
        let second = improve_complete_layout(&pieces, fast_settings, settings, &incumbent).unwrap();
        assert_eq!(first.result, second.result);
        assert_eq!(first.diagnostics, second.diagnostics);
        assert!(first.diagnostics.coupled_dynamic_separator.is_none());
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn coupled_separator_comparator_applies_weighted_then_raw_order() {
        let mut weighted = feasible_lane(0, 0.0, 0.0);
        weighted.score.weighted_loss = 2.0;
        weighted.score.boundary_loss = 1.0;
        weighted.score.collision_pairs = vec![(0, 1, 1.0)];
        let mut raw = feasible_lane(1, 1.0, 0.0);
        raw.score.weighted_loss = 3.0;
        raw.score.boundary_loss = 0.1;
        raw.score.collision_pairs = vec![(0, 1, 0.1)];
        assert_eq!(
            compare_coupled_separator_outcomes(0, &weighted, 1, &raw),
            Ordering::Less
        );

        raw.score.weighted_loss = weighted.score.weighted_loss;
        assert_eq!(
            compare_coupled_separator_outcomes(0, &weighted, 1, &raw),
            Ordering::Greater
        );
        assert_eq!(
            compare_coupled_separator_outcomes(1, &raw, 0, &weighted),
            Ordering::Less
        );
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn coupled_separator_seed_table_is_shared_between_arms() {
        let seed = 17_u64 ^ COUPLED_SEPARATOR_SEED_DOMAIN;
        let first = (0..COUPLED_SEPARATOR_TARGETS)
            .map(|target| {
                let target_seed = derive_seed(seed, target, usize::MAX - 64);
                (0..COUPLED_SEPARATOR_WORKERS)
                    .map(|worker| derive_seed(target_seed, 0, worker))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let second = (0..COUPLED_SEPARATOR_TARGETS)
            .map(|target| {
                let target_seed = derive_seed(seed, target, usize::MAX - 64);
                (0..COUPLED_SEPARATOR_WORKERS)
                    .map(|worker| derive_seed(target_seed, 0, worker))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert_eq!(first, second);
        assert_ne!(first[0], first[1]);
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn coupled_separator_is_cross_schedule_deterministic_and_resets_targets() {
        let polygons = [square(10.0), square(8.0)];
        let pieces = [
            GeneralFastPiece {
                id: "large",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "small",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let fast_settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let protected = construct_short_side_first(&pieces, fast_settings).unwrap();
        let mut settings = GeneralRelaxedSettings::mixed_61_probe(31, COUPLED_SEPARATOR_WORKERS);
        settings.sweeps_per_epoch = COUPLED_SEPARATOR_ROUNDS;
        settings.global_samples_per_move = 10;
        settings.focused_samples_per_move = 10;
        settings.refinement_rounds = 5;
        let single = JobPool::new(Some(1)).run_scoped(|| {
            run_coupled_dynamic_separator_experiment(
                &pieces,
                fast_settings,
                settings,
                &protected,
                None,
                None,
                CoupledRollbackComparison::Exact,
            )
        });
        let parallel = JobPool::new(Some(4)).run_scoped(|| {
            run_coupled_dynamic_separator_experiment(
                &pieces,
                fast_settings,
                settings,
                &protected,
                None,
                None,
                CoupledRollbackComparison::Exact,
            )
        });
        assert_eq!(single, parallel);
        for arm in [&single.control, &single.treatment] {
            assert_eq!(arm.targets_attempted, arm.targets.len());
            assert_eq!(arm.catalog_builds, 1);
            assert_eq!(
                arm.index_builds,
                arm.targets_attempted * COUPLED_SEPARATOR_WORKERS
            );
            assert_eq!(arm.immutable_variant_builds, 4);
            for (ordinal, target) in arm.targets.iter().enumerate() {
                assert_eq!(target.ordinal, ordinal);
                assert_eq!(target.worker_seeds.len(), COUPLED_SEPARATOR_WORKERS);
                if ordinal > 0 {
                    assert_ne!(target.target_seed, arm.targets[ordinal - 1].target_seed);
                }
            }
        }
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn coupled_catalog_runs_supported_concave_shapes() {
        let polygons = [l_shape(), square(8.0)];
        let pieces = [
            GeneralFastPiece {
                id: "concave",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "square",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let fast_settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let protected = construct_short_side_first(&pieces, fast_settings).unwrap();
        let settings = coupled_experiment_test_settings(37);
        let result = run_coupled_dynamic_separator_experiment(
            &pieces,
            fast_settings,
            settings,
            &protected,
            None,
            None,
            CoupledRollbackComparison::Exact,
        );

        for arm in [&result.control, &result.treatment] {
            assert_eq!(arm.catalog_builds, 1);
            assert_eq!(arm.immutable_variant_builds, 4);
            assert!(!arm
                .skipped_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("dynamic hazard catalog")));
        }
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn coupled_catalog_reports_unsupported_holes_without_mutating_protected_result() {
        let polygon = holed_square();
        let pieces = [GeneralFastPiece {
            id: "holed",
            polygon: &polygon,
            allow_rotation: true,
            allow_mirror: true,
        }];
        let fast_settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let protected = construct_short_side_first(&pieces, fast_settings).unwrap();
        let result = run_coupled_dynamic_separator_experiment(
            &pieces,
            fast_settings,
            coupled_experiment_test_settings(41),
            &protected,
            None,
            None,
            CoupledRollbackComparison::Exact,
        );
        let protected_fingerprint = coupled_fast_placement_fingerprint(&protected.placements);

        for arm in [&result.control, &result.treatment] {
            assert!(!arm.attempted);
            assert_eq!(arm.catalog_builds, 0);
            assert_eq!(
                arm.final_placement_fingerprint.as_deref(),
                Some(protected_fingerprint.as_str())
            );
            assert_eq!(
                arm.final_placements,
                coupled_placement_diagnostics(&protected.placements)
            );
            assert!(arm
                .skipped_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("confirmation catalog")));
        }
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn public_relaxed_path_reports_coupled_hole_fallback() {
        let polygon = holed_square();
        let pieces = [GeneralFastPiece {
            id: "holed",
            polygon: &polygon,
            allow_rotation: true,
            allow_mirror: true,
        }];
        let fast_settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let protected = construct_short_side_first(&pieces, fast_settings).unwrap();
        let outcome = improve_complete_layout(
            &pieces,
            fast_settings,
            coupled_experiment_test_settings(43),
            &protected,
        )
        .unwrap();

        assert_eq!(outcome.result, protected);
        assert_eq!(
            outcome.diagnostics.skipped_reason.as_deref(),
            Some("relaxed search does not yet flatten hole topology")
        );
        let coupled = outcome
            .diagnostics
            .coupled_dynamic_separator
            .expect("coupled fallback diagnostics");
        let protected_fingerprint = coupled_fast_placement_fingerprint(&protected.placements);
        for arm in [&coupled.control, &coupled.treatment] {
            assert!(!arm.attempted);
            assert_eq!(
                arm.final_placement_fingerprint.as_deref(),
                Some(protected_fingerprint.as_str())
            );
            assert!(arm
                .skipped_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("confirmation catalog")));
        }
    }

    /// Two 10 mm squares on a bare 100x100 sheet: no padding, no safety margin,
    /// so a layout's raw source depth is just its deepest `max_y`. That makes
    /// every depth in the adoption tests below readable by hand.
    #[cfg(feature = "jagua-experimental")]
    fn adoption_settings() -> GeneralFastSettings {
        GeneralFastSettings::deterministic_test(100.0, 100.0)
    }

    /// Both squares side by side at `long_axis_mm`, so the layout's raw source
    /// depth is `long_axis_mm + 10`. The 5 mm offsets keep the collision
    /// polygons - the sources expanded by the search allowance - off the sheet
    /// edges, which the composite validator would otherwise reject.
    #[cfg(feature = "jagua-experimental")]
    fn adoption_layout(long_axis_mm: f64) -> Vec<GeneralFastPlacement> {
        ["a", "b"]
            .into_iter()
            .enumerate()
            .map(|(index, id)| GeneralFastPlacement {
                piece_id: id.to_owned(),
                rotation_deg: 0.0,
                mirrored: false,
                translate_short_axis: 5.0 + index as f64 * 20.0,
                translate_long_axis: long_axis_mm,
            })
            .collect()
    }

    /// The legacy incumbent every adoption test starts from: both squares at
    /// 20 mm, so 30 mm deep, and carrying a distinctive constructor counter so
    /// the tests can tell a carried-over field from a defaulted one.
    #[cfg(feature = "jagua-experimental")]
    fn adoption_legacy_result() -> GeneralFastResult {
        let mut legacy = general_fast_result_seed(adoption_layout(20.0), 30.0);
        legacy.exact_evaluations = 7;
        legacy.order_variants_attempted = 3;
        legacy
    }

    #[cfg(feature = "jagua-experimental")]
    fn adoption_diagnostics(
        population: Option<GeneralPersistentVacancyDiagnostics>,
    ) -> GeneralRelaxedDiagnostics {
        GeneralRelaxedDiagnostics {
            coupled_dynamic_separator: Some(GeneralCoupledSeparatorDiagnostics {
                seed_domain: COUPLED_SEPARATOR_SEED_DOMAIN,
                control: GeneralCoupledSeparatorArmDiagnostics::default(),
                treatment: GeneralCoupledSeparatorArmDiagnostics::default(),
                boundary_projection_treatment: None,
                conflict_ruin_recreate: None,
                precompression_frontier_vacancy: None,
                persistent_vacancy_population: population,
            }),
            ..GeneralRelaxedDiagnostics::default()
        }
    }

    /// A mode report that published `placements` and claims they are valid.
    /// The claim is deliberately unconditional: the adoption point must reach
    /// its own verdict through the exact validator, never through this flag.
    #[cfg(feature = "jagua-experimental")]
    fn adoption_publication(
        placements: &[GeneralFastPlacement],
    ) -> GeneralPersistentVacancyDiagnostics {
        GeneralPersistentVacancyDiagnostics {
            mode: 22,
            attempted: true,
            exact_valid: true,
            contract_valid: true,
            final_placements: coupled_placement_diagnostics(placements),
            ..GeneralPersistentVacancyDiagnostics::default()
        }
    }

    #[cfg(feature = "jagua-experimental")]
    fn adopt_for_test(diagnostics: &GeneralRelaxedDiagnostics) -> GeneralFastResult {
        let polygon = square(10.0);
        let pieces = ["a", "b"].map(|id| GeneralFastPiece {
            id,
            polygon: &polygon,
            allow_rotation: false,
            allow_mirror: false,
        });
        adopt_published_layout(
            &pieces,
            adoption_settings(),
            diagnostics,
            adoption_legacy_result(),
        )
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn improving_mode_publication_becomes_the_engine_result() {
        let published = adoption_layout(5.0);
        let adopted = adopt_for_test(&adoption_diagnostics(Some(adoption_publication(
            &published,
        ))));

        assert_eq!(adopted.placements, published);
        assert!(adopted.unplaced_piece_ids.is_empty());
        // Re-measured by the validator, not copied from the mode's report.
        assert!(adopted.used_long_axis_depth_mm < adoption_legacy_result().used_long_axis_depth_mm);
        assert!(adopted.occupied_envelope_area_mm2 > 0.0);
        // The constructor's own work still happened and is still reported.
        assert_eq!(adopted.exact_evaluations, 7);
        assert_eq!(adopted.order_variants_attempted, 3);
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn refusing_mode_keeps_the_legacy_result() {
        let refused = GeneralPersistentVacancyDiagnostics {
            mode: 22,
            attempted: true,
            failure_reason: Some("no exact-valid state".to_owned()),
            ..GeneralPersistentVacancyDiagnostics::default()
        };

        assert_eq!(
            adopt_for_test(&adoption_diagnostics(Some(refused))),
            adoption_legacy_result()
        );
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn deeper_mode_publication_keeps_the_legacy_result() {
        let published = adoption_layout(40.0);

        assert_eq!(
            adopt_for_test(&adoption_diagnostics(Some(adoption_publication(
                &published
            )))),
            adoption_legacy_result()
        );
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn equally_deep_mode_publication_keeps_the_legacy_result() {
        let published = adoption_layout(20.0)
            .into_iter()
            .map(|placement| GeneralFastPlacement {
                translate_short_axis: placement.translate_short_axis + 1.0,
                ..placement
            })
            .collect::<Vec<_>>();

        assert_eq!(
            adopt_for_test(&adoption_diagnostics(Some(adoption_publication(
                &published
            )))),
            adoption_legacy_result()
        );
    }

    /// The one case that decides who holds publication authority: a shallower
    /// layout whose pieces overlap, reported with `exactValid` set. Adopting it
    /// would mean the mode's own flag published; refusing it means the exact
    /// validator did.
    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn illegal_mode_publication_is_refused_despite_its_own_valid_flag() {
        let mut published = adoption_layout(5.0);
        published[1].translate_short_axis = 10.0;

        assert_eq!(
            adopt_for_test(&adoption_diagnostics(Some(adoption_publication(
                &published
            )))),
            adoption_legacy_result()
        );
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn incomplete_mode_publication_is_refused() {
        let mut published = adoption_layout(5.0);
        published.truncate(1);

        assert_eq!(
            adopt_for_test(&adoption_diagnostics(Some(adoption_publication(
                &published
            )))),
            adoption_legacy_result()
        );
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn absent_mode_leaves_the_result_untouched() {
        assert_eq!(
            adopt_for_test(&adoption_diagnostics(None)),
            adoption_legacy_result()
        );
        assert_eq!(
            adopt_for_test(&GeneralRelaxedDiagnostics::default()),
            adoption_legacy_result()
        );
    }

    /// The invariance the rollout depends on: arming the diagnostics arms
    /// without arming a publishing mode must leave the engine's result exactly
    /// where the legacy path put it.
    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn coupled_arms_without_a_publishing_mode_do_not_move_the_result() {
        let polygon = square(10.0);
        let pieces = [GeneralFastPiece {
            id: "square",
            polygon: &polygon,
            allow_rotation: true,
            allow_mirror: true,
        }];
        let fast_settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let incumbent = construct_short_side_first(&pieces, fast_settings).unwrap();
        let mut settings = coupled_experiment_test_settings(47);
        settings.persistent_vacancy_mode = 0;
        settings.coupled_dynamic_separator = false;
        let without_arms =
            improve_complete_layout(&pieces, fast_settings, settings, &incumbent).unwrap();
        settings.coupled_dynamic_separator = true;
        let with_arms =
            improve_complete_layout(&pieces, fast_settings, settings, &incumbent).unwrap();

        assert!(with_arms.diagnostics.coupled_dynamic_separator.is_some());
        assert!(without_arms.diagnostics.coupled_dynamic_separator.is_none());
        assert_eq!(with_arms.result, without_arms.result);
    }
}
