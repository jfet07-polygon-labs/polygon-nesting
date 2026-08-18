use super::*;

use std::mem::size_of;

// The skyline constructor's trapped-void evaluator. Its default build is a
// zero-sized forwarder to `trapped_void_cells` below; `fast-constructor-profile`
// swaps in the incremental bit-grid evaluator.
#[path = "construction_void_grid.rs"]
mod construction_void_grid;
use construction_void_grid::ConstructionVoidCache;

// The skyline constructor's exact-confirmation prefilter. Its default build is
// a zero-sized forwarder whose one query always answers "no information", so
// every pair reaches Clipper exactly as before; `fast-constructor-confirm`
// swaps in the grid-exact separation certificate.
#[path = "construction_confirm_shield.rs"]
mod construction_confirm_shield;
use construction_confirm_shield::ConfirmShields;

// The skyline constructor's inner overlap certificate - the prefilter that runs
// in the opposite direction to the shield above, and the only one that can
// reject a row before it builds anything. Its default build is a zero-sized
// forwarder whose one query always answers "no proof"; `fast-constructor-reject`
// swaps in the inscribed-disc certificate, and `constructor-census` compiles the
// same certificate in for measurement without letting it decide anything.
#[path = "construction_reject_certificate.rs"]
pub(crate) mod construction_reject_certificate;
use construction_reject_certificate::RejectCertificates;

const PERSISTENT_VACANCY_SEED_DOMAIN: u64 = 0x5650_4f50_3030_3031;
const TARGET_DEPTH_MM: f64 = 165.0;
const EXPECTED_PARENT_FINGERPRINT: &str =
    "b9335a72cdcdd8df29be21450818f4ab1766ea1ea0b16765ad3998942a2ea6c5";
const EXPECTED_PARENT_DEPTH_MM: f64 = 168.361;
const MAX_LAYERS: usize = 40;
const BEAM_WIDTH: usize = 8;
const SELECTED_PIECES_PER_PARENT: usize = 2;
const ORIENTATIONS_PER_PIECE: usize = 12;
const POSITIONS_PER_ORIENTATION: usize = 32;
const FINALISTS_PER_PIECE: usize = 8;
const MAX_INACTIVE_PIECES: usize = 32;
const MAX_SOURCE_FEATURES: usize = 512;
const MAX_COLLISION_VERTICES: usize = 512;
// Modes 7 and 8 revive one archived elite topology on deterministically
// detected stagnation. A revival may fire no earlier than layer
// ARCHIVE_STAGNATION_LAYERS and at least ARCHIVE_REVIVAL_COOLDOWN layers after
// the previous expanded revival, so at most
// 1 + (MAX_LAYERS - 1 - ARCHIVE_STAGNATION_LAYERS) / ARCHIVE_REVIVAL_COOLDOWN
// revival expansions exist. Mode 7 expands the revived state as an extra
// parent; the quota formulas below fund that lane explicitly on top of the
// ordinary 8-parent schedule. Mode 8 swaps the revived state into the
// comparator-worst entering slot and adds no work.
const ARCHIVE_STAGNATION_LAYERS: usize = 3;
const ARCHIVE_REVIVAL_COOLDOWN: usize = 3;
const MAX_ARCHIVE_REVIVALS: usize =
    1 + (MAX_LAYERS - 1 - ARCHIVE_STAGNATION_LAYERS) / ARCHIVE_REVIVAL_COOLDOWN;
// Mode 11 runs a translation-only exact settling prelude before the target
// initializer: SETTLE_SWEEPS bottom-up passes over every piece of the
// instance, each attempt exploring one orientation stream and
// exact-confirming candidate positions in ascending settle-key order until
// the first strictly lower valid pose. Each settle attempt may exact-confirm
// up to POSITIONS_PER_ORIENTATION candidate rows, so the finalist-row and
// pair ceilings carry an explicit settle term instead of the 8-per-slot
// population term. The resulting slot ceiling is SETTLE_SWEEPS per piece and
// therefore lives in `VacancyQuotas`.
const SETTLE_SWEEPS: usize = 3;
const SETTLE_PROBES_PER_ATTEMPT: usize = 64;
// Mode 13 rebuilds the layout from an external hint fixture: one guided
// insertion per piece, ranked by grid distance to the hint pose, with at most
// RECONSTRUCTION_ROWS_PER_PIECE exact confirmations per piece, plus one
// deferred retry pass over the pieces the first pass could not place.
const RECONSTRUCTION_PASSES_PER_PIECE: usize = 2;
const RECONSTRUCTION_ROWS_PER_PIECE: usize = 192;
// Mode 20 constructs complete layouts from scratch with a skyline beam:
// CONSTRUCTION_RESTARTS independent beam passes (one seeded insertion order
// each) keep CONSTRUCTION_BEAM_WIDTH partial layouts per rank. Every
// (restart, rank, parent) expansion funds one selected slot whose candidate
// poses come from synthetic hints planted at the CONSTRUCTION_HINT_STATIONS
// deepest skyline valleys under CONSTRUCTION_HINT_PRIORS orientation priors
// (the pinned fixture's pose and the unrotated catalog pose), plus the full
// orientation/position streams, exact-confirmed in landing-frontier order up
// to CONSTRUCTION_ROWS_PER_PIECE rows with the last CONSTRUCTION_SHELF_ROWS
// reserved for the upward shelf escape, collecting at most
// CONSTRUCTION_FINALISTS_PER_SLOT children. Beam pruning caps survivors per
// parent at CONSTRUCTION_BEAM_CHILDREN_PER_PARENT and bands the frontier key
// at CONSTRUCTION_FRONTIER_BAND_GRID so the trapped-void term stays active
// on frontier-raising commits.
const CONSTRUCTION_RESTARTS: usize = 8;

/// Coordinator-supplied diversity salt for the mode-20/25 skyline constructor.
///
/// Every field is `None` for every CLI invocation and for every mode the
/// separator dispatches on its own, in which case this struct is inert and the
/// constructor is exactly the one the regression gates pin. It exists so the
/// portfolio coordinator can draw *several* constructor tickets from one
/// process rather than one, which is what the ledger's cell-size sweep argues
/// for and what a single mode-20 call - best of its whole restart sweep, one
/// derived cell - cannot give.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct ConstructionSalt {
    /// `Some((first, count))` runs restart indices `first ..= first + count - 1`
    /// reduced modulo [`CONSTRUCTION_RESTARTS`], instead of all of them. A
    /// `count` of zero, or one at or above the restart count, is normalised
    /// back to the full sweep, so the window can only ever select a subset of
    /// the orders the constructor already has.
    restart_window: Option<(usize, usize)>,
    /// Overrides the `fast-constructor-profile` evaluator's derived cell
    /// divisor. The legacy raster has no derived cell and ignores it.
    void_cell_divisor: Option<f64>,
}

impl ConstructionSalt {
    /// Reads the salt a coordinator put in the relaxed settings.
    pub(super) fn from_settings(settings: GeneralRelaxedSettings) -> Self {
        Self {
            restart_window: settings.construction_restart_window,
            void_cell_divisor: settings.construction_void_cell_divisor,
        }
    }

    /// The restart indices this salt selects, in order.
    ///
    /// The unsalted answer is `0 .. CONSTRUCTION_RESTARTS`, which is the
    /// sequence the loop has always walked.
    fn restarts(self) -> Vec<usize> {
        match self.restart_window {
            Some((first, count)) if count >= 1 && count < CONSTRUCTION_RESTARTS => (0..count)
                .map(|step| first.wrapping_add(step) % CONSTRUCTION_RESTARTS)
                .collect(),
            _ => (0..CONSTRUCTION_RESTARTS).collect(),
        }
    }
}
const CONSTRUCTION_BEAM_WIDTH: usize = 6;
const CONSTRUCTION_HINT_STATIONS: usize = 3;
const CONSTRUCTION_HINT_PRIORS: usize = 2;
const CONSTRUCTION_ROWS_PER_PIECE: usize = 320;
const CONSTRUCTION_SHELF_ROWS: usize = 24;
const CONSTRUCTION_FINALISTS_PER_SLOT: usize = 4;
const CONSTRUCTION_BEAM_CHILDREN_PER_PARENT: usize = 2;
const CONSTRUCTION_FRONTIER_BAND_GRID: i64 = 500;
const CONSTRUCTION_SKYLINE_COLUMNS: usize = 64;
const CONSTRUCTION_SEED_DOMAIN: u64 = 0x534B_594C_3230_3330;
// Mode 25 is mode 20's constructor plus the off-beam best-ever expansion
// parent: every layer keeps a sidecar copy of the pool elite under
// `construction_elite_key` (the retention key with the frontier banding
// removed), and whenever that elite is *not* one of the retained beam states
// it funds exactly one extra bounded expansion as an additional parent. The
// elite is never given a beam slot - it competes for retention only through
// its children - so the beam width, the per-parent diversity quota and the
// retention rule are all unchanged. One extra parent per (restart, rank) is
// the entire additional cost, which is why the per-piece construction term
// funds `CONSTRUCTION_BEAM_WIDTH + CONSTRUCTION_BEST_EVER_PARENTS` expansions
// per restart and rank. Mode 20 leaves the sidecar permanently empty and is
// bit-identical to the pre-mode-25 constructor.
const CONSTRUCTION_BEST_EVER_PARENTS: usize = 1;
// Mode 24 reuses the construction insertion machinery from a different
// starting point (a partially ejected parent rather than an empty sheet), so
// it takes its own seed domain: identical piece/ordinal pairs in the two
// modes must not draw the same orientation and position streams.
const BOUNDED_REINSERTION_SEED_DOMAIN: u64 = 0x424E_4452_494E_3234;
// Mode 28 (conflict-targeted re-placement) drives the same insertion primitive
// from a third starting point - a *rejected* compressed state with its
// conflicting pieces removed - so it likewise takes its own seed domain.
const REPLACEMENT_REPAIR_SEED_DOMAIN: u64 = 0x5245_504C_4143_3238;
// Mode 29 (joint multi-piece re-placement) drives the same insertion primitive
// from a fourth starting point - a rejected compressed state with the *whole*
// violating component removed rather than a vertex cover of it - so it likewise
// takes its own seed domain.
const JOINT_REPLACEMENT_SEED_DOMAIN: u64 = 0x4A4F_494E_5452_3239;
// Ejection sets at or below this size get *every* insertion order enumerated,
// in lexicographic order of the base order's own positions. Four pieces is 24
// orders, which is the point where exhaustive enumeration is still cheaper than
// any selection rule that would have to justify itself.
const JOINT_REPLACEMENT_MAX_PERMUTED_PIECES: usize = 4;
// Total insertion orders one joint attempt may try. It is the factorial ceiling
// at `JOINT_REPLACEMENT_MAX_PERMUTED_PIECES`, and it is also what bounds the
// rotation family a larger ejection set falls back to, so the pass's cost is a
// fixed multiple of a single re-placement at every instance size.
const JOINT_REPLACEMENT_ORDER_CAP: usize = 24;
// Pose-swap seeding rounds, and the attempts one round may spend. The swap is a
// coordinated move - piece A into piece B's vacated pocket and B into A's -
// which no per-piece displacement cloud around a piece's *own* vacated pose can
// express, and which the translation-only tiers provably cannot reach. One
// round, attempted only after every plain order has failed.
const JOINT_REPLACEMENT_SWAP_ROUNDS: usize = 1;
const JOINT_REPLACEMENT_SWAP_ATTEMPT_CAP: usize = 24;
// Vacated poses of *other* ejected pieces seeded per piece, nearest first. The
// swap round makes the exchange the leading hypothesis for one chosen pair;
// this makes it a candidate for every piece in every order, at a cost that
// stays inside the anchor-local stream's existing per-piece row budget
// (the cloud is at most 179 poses against `ANCHOR_LOCAL_ROWS` = 192).
const JOINT_REPLACEMENT_PEER_POSES: usize = 3;
// Violation components one joint pass repairs, one at a time, re-surveying the
// whole layout between them. The pass used to pool every pair-bearing
// component into a single ejection set, so four independent two-piece conflicts
// refused on an ejection cap that none of them individually trips; independent
// conflicts are independent repairs and are run as such. The ceiling exists so
// a residue of arbitrarily many clusters is still a bounded pass - beyond it
// the state is a search problem, which is the same judgement the component and
// ejection limits already encode.
const JOINT_REPLACEMENT_COMPONENT_PASSES: usize = 8;
// The finalist-combination beam. The shared insertion primitive commits the
// first in-bound finalist for every piece, so a component whose first piece
// takes the shallowest pose and thereby boxes the second one out is refused
// with no backtracking - which is exactly the failure mode the joint tier's
// measured negative was made of. For components at or below
// `JOINT_REPLACEMENT_BEAM_MAX_PIECES` the pass therefore enumerates
// *combinations* of finalist ranks rather than the single greedy one: at
// `CONSTRUCTION_FINALISTS_PER_SLOT` = 4 ranks and three pieces that is 4^3 = 64
// combinations, of which the all-zeros one is the greedy commit itself.
const JOINT_REPLACEMENT_BEAM_MAX_PIECES: usize = 3;
const JOINT_REPLACEMENT_BEAM_COMBINATIONS: usize = 64;
// Anchor-local re-insertion. The shared re-placement primitive's candidate
// generator is a *top-frontier* drop constructor: it plants hints at skyline
// valleys, drops, and slides. That is the right instrument for building a
// layout upward from an empty sheet, and it structurally cannot see an
// INTERIOR pocket - not even the pocket a piece was just lifted out of, whose
// occupant provably fits because it was sitting there. So a piece that carries
// a recorded prior pose gets its candidates seeded at that pose as well: the
// vacated pose itself, a bounded displacement cloud around it, and the piece's
// other admitted orientations at the vacated translation.
//
// The cloud's magnitudes are scale-free. Two families are unioned, deduplicated
// on the placement grid and taken in ascending order:
//
// * dyadic fractions of the piece's own smaller bounding-box extent in the
//   orientation being seeded, which is the only intrinsic length a general
//   piece carries and the one that scales with the pocket it came out of;
// * small multiples of the caller's measured residue scale, when the caller
//   measured one - mode 28 passes the violation mass its ejection set carries,
//   which is exactly how far the conflict has to travel to clear.
//
// Displacement directions lead with the piece's own escape geometry - the
// projection's own answer, then the closest-approach witness of each violating
// pair the piece is incident to, then their joint sum - and are backed by the
// unaimed fan below. The aimed directions are what actually matters: at record
// density the feasible set around a conflicting pose is a sliver, and a net of
// axes and diagonals alone has 45-degree holes in it.
//
// So the cloud is at most
// (4 + 3) magnitudes * (1 + 4 + 1 + 16) directions
//   + 1 vacated + ANCHOR_LOCAL_PROJECTION_ITERATES projected
//   + JOINT_REPLACEMENT_PEER_POSES peer = 182 poses per
// piece, plus one vacated-translation pose per extra orientation prior; every
// one of them a charged confirmation row inside the existing per-piece cap.
const ANCHOR_LOCAL_EXTENT_FRACTIONS: [f64; 4] = [1.0 / 256.0, 1.0 / 64.0, 1.0 / 16.0, 1.0 / 4.0];
const ANCHOR_LOCAL_RESIDUE_MULTIPLES: [f64; 3] = [1.0, 2.0, 4.0];
const ANCHOR_LOCAL_MAGNITUDES: usize =
    ANCHOR_LOCAL_EXTENT_FRACTIONS.len() + ANCHOR_LOCAL_RESIDUE_MULTIPLES.len();
const ANCHOR_LOCAL_SEPARATION_DIRECTIONS: usize = 4;
// Iterates kept from the single-piece separating projection. The projection
// oscillates rather than settles when one piece has to satisfy several
// neighbours at once, so it is read as a trajectory of poses to try rather
// than as a solver with an answer; this is how many of those poses are worth
// charged confirmation rows.
const ANCHOR_LOCAL_PROJECTION_ITERATES: usize = 24;
// The anchor-local stream's own charged-row budget, held apart from the
// station stream's so that seeding a vacated pose can only ever *add*
// candidates to a slot, never displace the ones the stations would have found.
// Sized to cover the whole cloud, so nothing the primitive generates is
// silently dropped, and funded alongside the per-piece construction row cap by
// `bounded_reinsertion_fits_the_construction_budget`.
const ANCHOR_LOCAL_ROWS: usize = 192;
// The unaimed fallback fan: the eight construction probe directions, then the
// eight half-octants between them. Angular resolution is what this cloud is
// short of - the feasible set around a conflicting pose at record density is a
// sliver, so a displacement that is right in length and 22.5 degrees off in
// bearing lands outside it - and the half-octants halve that error for the
// price of one more magnitude ladder. Spelled out rather than derived from
// trigonometry so the pose stream does not depend on the platform's libm.
const ANCHOR_LOCAL_FAN_DIRECTIONS: [(f64, f64); 16] = [
    (1.0, 0.0),
    (-1.0, 0.0),
    (0.0, 1.0),
    (0.0, -1.0),
    (0.7071067811865476, 0.7071067811865476),
    (-0.7071067811865476, 0.7071067811865476),
    (0.7071067811865476, -0.7071067811865476),
    (-0.7071067811865476, -0.7071067811865476),
    (0.9238795325112867, 0.3826834323650898),
    (0.9238795325112867, -0.3826834323650898),
    (-0.9238795325112867, 0.3826834323650898),
    (-0.9238795325112867, -0.3826834323650898),
    (0.3826834323650898, 0.9238795325112867),
    (0.3826834323650898, -0.9238795325112867),
    (-0.3826834323650898, 0.9238795325112867),
    (-0.3826834323650898, -0.9238795325112867),
];
// Modes 32 and 33: orientation-perturbed re-insertion.
//
// The anchor-local stream above is a neighbourhood of exactly ONE pose - the
// vacated one - carried under the anchor's *own* orientation prior, and the
// only other orientations any re-insertion caller ever sees come from the
// station stream, which is anchored at a skyline valley and cannot reach an
// interior pocket. So a layout built from continuous fine angles admits no
// alternative orientation at re-insertion density at all: the ejected piece
// goes back at the angle it came out at, translated, or it does not go back.
// That was measured rather than argued (the pose-entry negative: zero rotation
// change and zero mirror flips across every legal state modes 28 and 29
// produced on the record basin), and it is the degree of freedom this stream
// adds.
//
// Two properties make it a *local* operator rather than a second constructor:
//
// * every variant is re-centred on the vacated footprint's own bounding-box
//   centre, so a ladder rung rotates the piece **in place**. Rotation is
//   applied about the source origin, so without re-centring a 5-degree rung on
//   a piece whose material sits 100 mm from that origin is an 8.7 mm
//   translation - a different pocket, not a different orientation;
// * every variant is searched over the *same* local translation neighbourhood
//   the vacated pose gets - the projection trajectory, the peers' pockets and
//   the aimed displacement cloud - rigidly carried along by the same
//   re-centring shift.
//
// The ladder itself is scale-free: an angle carries no length, so a geometric
// ladder is the general instrument. Ratio 5/2, topping out just under 5 degrees
// (above which "rotate in place" stops being a local repair and becomes a
// re-orientation the station stream already offers). Spelled out rather than
// computed so the pose stream does not depend on the platform's `powf`.
//
// The floor was originally 0.02 degrees, on the argument that a finer rung
// cannot move a vertex by even one pose-grid quantum. That argument was wrong
// by a wide margin and the stream's own accepted poses are what showed it: a
// rung `d` degrees moves a vertex sitting `r` from the rotation centre by
// `r * d * pi/180`, so on a hand-sized `r` of 100 mm the old floor is 0.035 mm
// - thirty-five 0.001 mm quanta, not one. The rung that stops being expressible
// on that radius is nearer 6e-4 degrees. And the campaign's accepted rungs pile
// up on the floor itself, which is the signature of a floor placed above the
// useful band rather than at its edge.
//
// So the ladder is extended downward by two rungs of the same ratio. 0.0032
// degrees still moves a 100 mm vertex by 0.0056 mm - five quanta, comfortably
// expressible - while being a sixth of the old floor, and both new rungs are
// exact on the 1e-6 degree angle key, so no rung collapses onto another and
// re-spends the same charged rows.
const ORIENTATION_PERTURBATION_LADDER_DEG: [f64; 9] = [
    0.0032, 0.008, 0.02, 0.05, 0.125, 0.3125, 0.78125, 1.953125, 4.8828125,
];
// Orientation variants one perturbed re-insertion seeds per piece: the ladder
// in both signs, the mirrored counterpart of the vacated orientation, and the
// ladder in both signs mirrored. A piece whose request forbids rotation or
// mirroring contributes only the families it is allowed, and duplicates on the
// angle grid are dropped, so this is a ceiling rather than a count.
const ORIENTATION_PERTURBATION_VARIANTS: usize =
    4 * ORIENTATION_PERTURBATION_LADDER_DEG.len() + 1;
// The orientation stream's own charged-row budget, held apart from both the
// station stream's and the anchor-local stream's for the same reason those two
// are held apart: an additive degree of freedom must never be able to spend the
// rows a legacy-reachable solution would have used.
//
// The stream is the anchor-local neighbourhood carried onto each variant, so
// covering it whole costs exactly the anchor-local budget once per variant -
// which is the same rule `ANCHOR_LOCAL_ROWS` follows ("sized to cover the whole
// cloud, so nothing the primitive generates is silently dropped"). It is
// derived rather than tuned, and it matters: at a budget that truncates the
// stream to its leading ranks the record basin's depth-setting piece produced
// no finalist at all, and at full coverage it produced two.
const ORIENTATION_PERTURBATION_ROWS: usize =
    ORIENTATION_PERTURBATION_VARIANTS * ANCHOR_LOCAL_ROWS;
// Bucket ordinals the anchor-local stream may consume for one piece: its whole
// cloud, plus one vacated-translation pose per orientation prior. The
// orientation stream starts its own ordinals above this so the coarse spatial
// de-duplication downstream can never collapse a rotated candidate onto an
// unrotated one that happens to share a 256-grid cell.
const ANCHOR_LOCAL_BUCKET_SPAN: usize = 1
    + ANCHOR_LOCAL_PROJECTION_ITERATES
    + JOINT_REPLACEMENT_PEER_POSES
    + ANCHOR_LOCAL_MAGNITUDES
        * (1 + ANCHOR_LOCAL_SEPARATION_DIRECTIONS + 1 + ANCHOR_LOCAL_FAN_DIRECTIONS.len())
    + CONSTRUCTION_HINT_PRIORS;
const ORIENTATION_PERTURBATION_BUCKET_BASE: usize =
    ORIENTATIONS_PER_PIECE + CONSTRUCTION_HINT_PRIORS + ANCHOR_LOCAL_BUCKET_SPAN;
const CONSTRUCTION_TRANSIENT_BYTES: usize = 192 * 1024;
// Child-scoring flood fills follow the reviewed-contract precedent of the
// uncharged LNS depth-key scans: the structural ceiling (`VacancyQuotas::
// construction_void_scan_cap`) is asserted in the quota test and the realized
// count is reported in the construction diagnostics (voidScans).
// Mode 14 alternates one settle sweep with one guillotine group-drop pass per
// compaction round: for a descending ladder of horizontal cuts - at most one
// cut per piece, since the ladder is built from the distinct lower bounds of
// the active pieces - every active piece above the cut translates downward as
// one rigid group, so pairs inside the group need no re-checking and mutually
// wedged clusters can harvest slack no single-piece move reaches.
const COMPACTION_ROUNDS: usize = 3;
const GROUP_DROP_PROBES_PER_CUT: usize = 64;
// Mode 15 runs a non-monotone lift/resettle/reinsert lifecycle: each round
// removes the frontier piece plus its nearest neighbors (an adaptive
// neighborhood schedule), lets the survivors resettle into the vacated
// space, reinserts the removed pieces with full orientation freedom, and
// accepts only rounds whose complete result strictly lowers the frontier,
// reverting to the snapshot otherwise. Two settle sweeps run per round (one
// before removal, one on the survivors) plus one initial sweep.
const LNS_ROUNDS: usize = 24;
const LNS_NEIGHBORHOOD_SCHEDULE: [usize; LNS_ROUNDS] = [
    4, 6, 8, 10, 12, 16, 20, 24, 4, 6, 8, 10, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56,
];
const LNS_SETTLE_SWEEPS: usize = 3 * LNS_ROUNDS + 1;
const LNS_SCHEDULE_TOTAL: usize =
    2 * (4 + 6 + 8 + 10 + 12 + 16 + 20 + 24) + (28 + 32 + 36 + 40 + 44 + 48 + 52 + 56);
const LNS_REINSERT_SLOTS: usize = LNS_SCHEDULE_TOTAL
    + LNS_ROUNDS * SEPARATION_RELOCATIONS_PER_ROUND
    + OPTIMIZER_CYCLES * OPTIMIZER_CANDIDATES_PER_PIECE * LNS_SCHEDULE_TOTAL;
// Mode 16 replaces greedy reinsertion with overlap-mediated separation:
// removed pieces return at their old poses (overlaps permitted), then a
// bounded deterministic descent moves one overlapping soft piece at a time
// along the compass ladder, accepting only strict decreases of the
// grid-quantized total exact overlap area, until overlap reaches zero or the
// move budget is exhausted. Only a zero-overlap endpoint may compete for
// acceptance.
const SEPARATION_MOVES_PER_ROUND: usize = 200;
// Mode-21 bridge selection probes every active piece once per round with an
// uncharged trapped-void flood fill (plus one baseline scan), counted in the
// LNS diagnostics and structurally bounded by the schedule
// (`VacancyQuotas::bridge_void_scan_cap`).
const SEPARATION_RELOCATIONS_PER_ROUND: usize = 12;
// Mode-17 endpoint optimizer: after a round's endpoint is feasible, up to
// OPTIMIZER_CYCLES steepest-descent passes re-place each lifted piece at the
// best of its top OPTIMIZER_CANDIDATES_PER_PIECE candidate poses under the
// full acceptance key, so the endpoint generator optimizes rather than
// merely places.
const OPTIMIZER_CYCLES: usize = 2;
const OPTIMIZER_CANDIDATES_PER_PIECE: usize = 3;
const SEPARATION_PROBES_PER_MOVE: usize = 96;
const ORDINARY_SELECTED_PIECE_SLOTS: usize = MAX_LAYERS * BEAM_WIDTH * SELECTED_PIECES_PER_PARENT;
const ARCHIVE_SELECTED_PIECE_SLOTS: usize = MAX_ARCHIVE_REVIVALS * SELECTED_PIECES_PER_PARENT;
const POPULATION_SELECTED_PIECE_SLOTS: usize =
    ORDINARY_SELECTED_PIECE_SLOTS + ARCHIVE_SELECTED_PIECE_SLOTS;
const POSITION_SOURCE_ATTEMPTS_PER_ORIENTATION: usize = 529;
const SEPARATION_COLLISION_BUILDS: usize =
    LNS_ROUNDS * (LNS_REINSERT_SLOTS / 2 + SEPARATION_MOVES_PER_ROUND * SEPARATION_PROBES_PER_MOVE);
// Full-state collision-build passes funded outside the per-slot lanes: the
// settle or compaction prelude, the target initializer, and the mode-14 exact
// re-anchor after the group drops. Each pass rebuilds one collision per piece.
const PRELUDE_COLLISION_BUILD_PASSES: usize = 3;
// Every publication audit runs the dual validator: two passes that each
// rebuild one collision per piece and re-check every active pair once.
const VALIDATOR_PASSES_PER_AUDIT: usize = 2;
const MAX_CLIPPER_OUTPUT_VERTICES: usize = 4_000_000;
const MAX_PARTIAL_AUDITS: usize = 41;
const MAX_COMPLETE_AUDITS: usize = 64;
const MAX_AUDITS: usize = MAX_PARTIAL_AUDITS + MAX_COMPLETE_AUDITS;
const MAX_RETAINED_BYTES: usize = 64 * 1024 * 1024;

/// Instance-scaled aggregate ceilings.
///
/// Every constant above is either per-piece, per-slot or per-round and is
/// therefore instance-independent; the aggregate ceilings below multiply those
/// rates by the piece count of the request under test, so the machinery funds
/// the same *per-piece* work on any instance. The formulas are asserted in
/// `aggregate_quota_formulas_match_the_reviewed_contract`, which additionally
/// pins the historical Mixed-61 values they reproduce at 61 pieces.
///
/// All products saturate: an instance large enough to overflow `usize` gets a
/// ceiling of `usize::MAX` rather than a wrapped (and far too small) budget.
// No `Default`: a zero-quota ledger would silently starve every lane, so the
// only way to obtain quotas is to state the instance's piece count.
#[derive(Clone, Copy, Debug)]
struct VacancyQuotas {
    piece_count: usize,
    /// Distinct guillotine cuts a single mode-14 group-drop pass may evaluate;
    /// the ladder is built from the distinct active lower bounds, so it can
    /// never exceed one cut per piece.
    group_drop_cuts: usize,
    settle_selected_piece_slots: usize,
    reconstruction_selected_piece_slots: usize,
    lns_settle_selected_piece_slots: usize,
    construction_selected_piece_slots: usize,
    construction_void_scan_cap: usize,
    bridge_void_scan_cap: usize,
    group_drop_pair_visits: usize,
    separation_pair_visits: usize,
    max_selected_piece_slots: usize,
    max_orientation_streams: usize,
    max_source_feature_visits: usize,
    max_position_source_attempts: usize,
    max_returned_positions: usize,
    max_hazard_queries: usize,
    max_proxy_pressure_visits: usize,
    max_exact_finalist_rows: usize,
    max_experimental_collision_builds: usize,
    max_experimental_pair_visits: usize,
    /// Collision rebuilds and pair re-checks charged by one publication audit.
    validator_collision_builds_per_audit: usize,
    validator_pair_visits_per_audit: usize,
    max_validator_collision_builds: usize,
    max_validator_pair_visits: usize,
    max_transformed_collision_vertices: usize,
    max_clipper_input_vertices: usize,
}

impl VacancyQuotas {
    fn for_piece_count(piece_count: usize) -> Self {
        let scale = |rate: usize| rate.saturating_mul(piece_count);
        // Pairs of distinct pieces in a complete state.
        let complete_pairs = piece_count
            .saturating_mul(piece_count.saturating_sub(1))
            .saturating_div(2);
        // Active pieces a single candidate row is checked against.
        let peers = piece_count.saturating_sub(1);

        let settle_selected_piece_slots = scale(SETTLE_SWEEPS);
        let reconstruction_selected_piece_slots = scale(RECONSTRUCTION_PASSES_PER_PIECE);
        let lns_settle_selected_piece_slots = scale(LNS_SETTLE_SWEEPS);
        // One expansion per (restart, rank, beam slot), plus the one off-beam
        // best-ever parent expansion mode 25 may add at every (restart, rank).
        let construction_selected_piece_slots = scale(
            CONSTRUCTION_RESTARTS * (CONSTRUCTION_BEAM_WIDTH + CONSTRUCTION_BEST_EVER_PARENTS),
        );

        let max_selected_piece_slots = POPULATION_SELECTED_PIECE_SLOTS
            .saturating_add(settle_selected_piece_slots)
            .saturating_add(reconstruction_selected_piece_slots)
            .saturating_add(lns_settle_selected_piece_slots)
            .saturating_add(LNS_REINSERT_SLOTS)
            .saturating_add(construction_selected_piece_slots);
        let max_orientation_streams =
            max_selected_piece_slots.saturating_mul(ORIENTATIONS_PER_PIECE);
        let max_returned_positions =
            max_orientation_streams.saturating_mul(POSITIONS_PER_ORIENTATION);
        // Modes 32 and 33 open a lane that did not exist before: the
        // orientation-perturbation stream's charged confirmation rows, and one
        // collision build per orientation variant per ejected piece per
        // component pass. It is funded by its own term rather than squeezed
        // into the construction term, for the same reason every other lane
        // here has one - and because a ceiling that only grows for a lane no
        // legacy mode can enter cannot change a legacy mode's behaviour. The
        // slot factor is the joint pass's own worst case (every insertion
        // order and every pose-swap attempt over an ejection set as large as
        // the whole layout), which strictly covers the second tier's.
        let orientation_perturbation_slots = JointReplacementBudget::slot_cap(piece_count);
        let orientation_perturbation_rows =
            orientation_perturbation_slots.saturating_mul(ORIENTATION_PERTURBATION_ROWS);
        // The rows themselves are already funded through `max_exact_finalist_rows`,
        // which the build ceiling adds wholesale; this term is the *extra*
        // build the stream takes before any row runs, one per variant per
        // ejected piece per component pass.
        let orientation_perturbation_builds = JOINT_REPLACEMENT_COMPONENT_PASSES
            .saturating_mul(piece_count)
            .saturating_mul(ORIENTATION_PERTURBATION_VARIANTS);
        let max_exact_finalist_rows = POPULATION_SELECTED_PIECE_SLOTS
            .saturating_mul(FINALISTS_PER_PIECE)
            .saturating_add(settle_selected_piece_slots.saturating_mul(SETTLE_PROBES_PER_ATTEMPT))
            .saturating_add(
                reconstruction_selected_piece_slots.saturating_mul(RECONSTRUCTION_ROWS_PER_PIECE),
            )
            .saturating_add(
                lns_settle_selected_piece_slots.saturating_mul(SETTLE_PROBES_PER_ATTEMPT),
            )
            .saturating_add(LNS_REINSERT_SLOTS.saturating_mul(RECONSTRUCTION_ROWS_PER_PIECE))
            .saturating_add(
                construction_selected_piece_slots.saturating_mul(CONSTRUCTION_ROWS_PER_PIECE),
            )
            .saturating_add(orientation_perturbation_rows);

        let group_drop_pair_visits = scale(COMPACTION_ROUNDS)
            .saturating_mul(GROUP_DROP_PROBES_PER_CUT)
            .saturating_mul(piece_count);
        let separation_pair_visits =
            scale(LNS_ROUNDS * SEPARATION_MOVES_PER_ROUND * SEPARATION_PROBES_PER_MOVE);

        let max_experimental_collision_builds = scale(PRELUDE_COLLISION_BUILD_PASSES)
            .saturating_add(max_orientation_streams)
            .saturating_add(max_exact_finalist_rows)
            .saturating_add(reconstruction_selected_piece_slots)
            .saturating_add(LNS_REINSERT_SLOTS)
            .saturating_add(
                construction_selected_piece_slots.saturating_mul(CONSTRUCTION_HINT_PRIORS),
            )
            .saturating_add(SEPARATION_COLLISION_BUILDS)
            .saturating_add(orientation_perturbation_builds);
        let max_experimental_pair_visits = complete_pairs
            .saturating_add(max_exact_finalist_rows.saturating_mul(peers))
            .saturating_add(group_drop_pair_visits)
            .saturating_add(separation_pair_visits);

        let validator_collision_builds_per_audit = scale(VALIDATOR_PASSES_PER_AUDIT);
        let validator_pair_visits_per_audit =
            complete_pairs.saturating_mul(VALIDATOR_PASSES_PER_AUDIT);
        let max_validator_collision_builds =
            validator_collision_builds_per_audit.saturating_mul(MAX_AUDITS);
        let max_validator_pair_visits = validator_pair_visits_per_audit.saturating_mul(MAX_AUDITS);

        Self {
            piece_count,
            group_drop_cuts: piece_count,
            settle_selected_piece_slots,
            reconstruction_selected_piece_slots,
            lns_settle_selected_piece_slots,
            construction_selected_piece_slots,
            construction_void_scan_cap: construction_selected_piece_slots
                .saturating_mul(CONSTRUCTION_FINALISTS_PER_SLOT)
                .saturating_add(CONSTRUCTION_RESTARTS),
            bridge_void_scan_cap: LNS_ROUNDS.saturating_mul(piece_count.saturating_add(1)),
            group_drop_pair_visits,
            separation_pair_visits,
            max_selected_piece_slots,
            max_orientation_streams,
            max_source_feature_visits: max_selected_piece_slots
                .saturating_mul(2)
                .saturating_mul(MAX_SOURCE_FEATURES),
            max_position_source_attempts: max_orientation_streams
                .saturating_mul(POSITION_SOURCE_ATTEMPTS_PER_ORIENTATION),
            max_returned_positions,
            max_hazard_queries: max_returned_positions,
            max_proxy_pressure_visits: max_returned_positions.saturating_mul(piece_count),
            max_exact_finalist_rows,
            max_experimental_collision_builds,
            max_experimental_pair_visits,
            validator_collision_builds_per_audit,
            validator_pair_visits_per_audit,
            max_validator_collision_builds,
            max_validator_pair_visits,
            max_transformed_collision_vertices: max_experimental_collision_builds
                .saturating_add(max_validator_collision_builds)
                .saturating_mul(MAX_COLLISION_VERTICES),
            max_clipper_input_vertices: max_experimental_pair_visits
                .saturating_add(max_validator_pair_visits)
                .saturating_mul(2 * MAX_COLLISION_VERTICES),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct VacancyTransition {
    inserted: usize,
    ejected: Vec<usize>,
}

#[derive(Clone)]
struct VacancyState {
    placements: Vec<RelaxedPlacement>,
    active: Vec<bool>,
    collisions: Vec<Option<Arc<PolygonSet>>>,
    last_transition: Option<VacancyTransition>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct VacancyStateIdentity {
    active_placements: Vec<(usize, i64, bool, i64, i64)>,
    inactive: Vec<usize>,
    last_transition: Option<VacancyTransition>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ContactEdge {
    first_id: String,
    second_id: String,
    axis: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ContactSignature {
    active_ids: Vec<String>,
    edges: Vec<ContactEdge>,
}

#[derive(Clone, Debug)]
struct PieceDifficulty {
    expanded_area_grid2: i128,
    hull_deficit_grid2: i128,
    minimum_side_grid: i64,
    material_area_grid2: i128,
}

#[derive(Clone)]
struct RankedProposal {
    placement: RelaxedPlacement,
    proxy_loss: f64,
    orientation_ordinal: usize,
    diversity_key: u64,
}

struct SelectedInactivePieces {
    indices: Vec<usize>,
    rotation_start_index: Option<usize>,
}

#[derive(Clone)]
struct EliteSnapshot {
    fingerprint: String,
    inactive_piece_count: usize,
    inactive_area_grid2: i128,
    inactive_difficulty_sequence: Vec<(i128, i128, i64, String)>,
    ejected_material_area_grid2: i128,
    ejected_piece_count: usize,
    active_frontier_grid: i64,
    identity: VacancyStateIdentity,
}

/// Bounded out-of-beam topology archive for modes 7 and 8.
///
/// The archive stores full clones of the best-ever area-first and count-first
/// elite states. It never occupies an ordinary beam slot; a revived state is
/// either expanded as one extra parent (mode 7) or swapped into the
/// comparator-worst entering slot (mode 8) on deterministically detected
/// stagnation layers only. Every decision derives from the run's own layer
/// history and semantic state identities; no wall clock, platform, or
/// population-ordinal information enters the schedule.
struct TopologyArchive {
    area: Option<(EliteSnapshot, VacancyState)>,
    count: Option<(EliteSnapshot, VacancyState)>,
    last_improvement_layer: usize,
    last_revival_layer: Option<usize>,
    revivals_expanded: usize,
    revivals_skipped: usize,
    revival_ordinal: usize,
    peak_bytes: usize,
    revival_children_generated: usize,
    revival_children_retained: usize,
}

enum RevivalDecision {
    NotStagnant,
    Skipped(&'static str),
    Revive {
        kind: &'static str,
        state: VacancyState,
        fingerprint: String,
    },
}

impl TopologyArchive {
    fn new() -> Self {
        Self {
            area: None,
            count: None,
            last_improvement_layer: 0,
            last_revival_layer: None,
            revivals_expanded: 0,
            revivals_skipped: 0,
            revival_ordinal: 0,
            peak_bytes: 0,
            revival_children_generated: 0,
            revival_children_retained: 0,
        }
    }

    fn bytes(&self) -> usize {
        [self.area.as_ref(), self.count.as_ref()]
            .into_iter()
            .flatten()
            .map(|(snapshot, state)| {
                size_of::<EliteSnapshot>()
                    .saturating_add(elite_snapshot_heap_bytes(snapshot))
                    .saturating_add(size_of::<VacancyState>())
                    .saturating_add(state_heap_bytes(state))
            })
            .sum()
    }

    fn charge_peak(&mut self) {
        self.peak_bytes = self.peak_bytes.max(self.bytes());
    }

    fn plan_revival(
        &self,
        layer: usize,
        population: &[VacancyState],
        pieces: &[GeneralFastPiece<'_>],
        difficulty: &[PieceDifficulty],
        mode: usize,
    ) -> RevivalDecision {
        if self.area.is_none() && self.count.is_none() {
            return RevivalDecision::NotStagnant;
        }
        if layer.saturating_sub(self.last_improvement_layer) < ARCHIVE_STAGNATION_LAYERS {
            return RevivalDecision::NotStagnant;
        }
        if let Some(last) = self.last_revival_layer {
            if layer.saturating_sub(last) < ARCHIVE_REVIVAL_COOLDOWN {
                return RevivalDecision::NotStagnant;
            }
        }
        if self.revivals_expanded >= MAX_ARCHIVE_REVIVALS {
            return RevivalDecision::Skipped("revivalBudgetExhausted");
        }
        if matches!(mode, 8 | 9 | 10 | 11 | 12 | 14 | 15 | 16 | 17 | 18 | 19)
            && population.len() < 2
        {
            return RevivalDecision::Skipped("populationTooSmall");
        }
        let candidates: [(&'static str, Option<&(EliteSnapshot, VacancyState)>); 2] =
            if self.revival_ordinal.is_multiple_of(2) {
                [("area", self.area.as_ref()), ("count", self.count.as_ref())]
            } else {
                [("count", self.count.as_ref()), ("area", self.area.as_ref())]
            };
        let mut last_reason = "archiveEmpty";
        for (kind, entry) in candidates {
            let Some((snapshot, state)) = entry else {
                continue;
            };
            if population
                .iter()
                .any(|member| same_state_identity(member, state))
            {
                last_reason = "inPopulation";
                continue;
            }
            if matches!(mode, 8 | 9 | 10 | 11 | 12 | 14 | 15 | 16 | 17 | 18 | 19) {
                let worst = population
                    .last()
                    .expect("a mode-8 revival population has at least two states");
                let better = if kind == "area" {
                    compare_states(state, worst, pieces, difficulty).is_lt()
                } else {
                    compare_count_states(state, worst, pieces, difficulty).is_lt()
                };
                if !better {
                    last_reason = "notBetterThanWorst";
                    continue;
                }
            }
            return RevivalDecision::Revive {
                kind,
                state: state.clone(),
                fingerprint: snapshot.fingerprint.clone(),
            };
        }
        RevivalDecision::Skipped(last_reason)
    }
}

fn elite_snapshot_heap_bytes(snapshot: &EliteSnapshot) -> usize {
    snapshot
        .fingerprint
        .capacity()
        .saturating_add(
            snapshot
                .inactive_difficulty_sequence
                .capacity()
                .saturating_mul(size_of::<(i128, i128, i64, String)>()),
        )
        .saturating_add(
            snapshot
                .inactive_difficulty_sequence
                .iter()
                .map(|(_, _, _, id)| id.capacity())
                .sum::<usize>(),
        )
        .saturating_add(
            snapshot
                .identity
                .active_placements
                .capacity()
                .saturating_mul(size_of::<(usize, i64, bool, i64, i64)>()),
        )
        .saturating_add(
            snapshot
                .identity
                .inactive
                .capacity()
                .saturating_mul(size_of::<usize>()),
        )
        .saturating_add(
            snapshot
                .identity
                .last_transition
                .as_ref()
                .map_or(0, |transition| {
                    transition
                        .ejected
                        .capacity()
                        .saturating_mul(size_of::<usize>())
                }),
        )
}

struct RunWork {
    diagnostics: GeneralPersistentVacancyWorkDiagnostics,
    quotas: VacancyQuotas,
    /// The constructor's exact-confirmation prefilter cache. It lives here
    /// rather than in a parameter because every function on the confirmation
    /// path already carries `&mut RunWork`, so no signature moves and the
    /// default build's generated code is unchanged — the forwarder is
    /// zero-sized and its one query is a constant `false`.
    confirm_shields: ConfirmShields,
    /// The constructor's inner overlap certificate cache, here for the same
    /// reason and with the same default-build cost, which is none.
    reject_certificates: RejectCertificates,
}

impl RunWork {
    fn new(piece_count: usize) -> Self {
        Self {
            diagnostics: GeneralPersistentVacancyWorkDiagnostics::default(),
            quotas: VacancyQuotas::for_piece_count(piece_count),
            confirm_shields: ConfirmShields::default(),
            reject_certificates: RejectCertificates::default(),
        }
    }

    fn cap(&self, reason: &str) -> String {
        format!("cap: {reason}")
    }

    fn charge_source_features(&mut self, amount: usize) -> Result<(), String> {
        self.diagnostics.source_feature_visits = self
            .diagnostics
            .source_feature_visits
            .saturating_add(amount);
        if self.diagnostics.source_feature_visits > self.quotas.max_source_feature_visits {
            return Err(self.cap("source-feature visit budget exhausted"));
        }
        Ok(())
    }

    fn charge_position_sources(&mut self, amount: usize) -> Result<(), String> {
        self.diagnostics.position_source_attempts = self
            .diagnostics
            .position_source_attempts
            .saturating_add(amount);
        if self.diagnostics.position_source_attempts > self.quotas.max_position_source_attempts {
            return Err(self.cap("position-source attempt budget exhausted"));
        }
        Ok(())
    }

    fn charge_experimental_pair(&mut self) -> Result<(), String> {
        self.diagnostics.experimental_pair_visits =
            self.diagnostics.experimental_pair_visits.saturating_add(1);
        if self.diagnostics.experimental_pair_visits > self.quotas.max_experimental_pair_visits {
            return Err(self.cap("experimental pair-visit budget exhausted"));
        }
        Ok(())
    }

    fn charge_validator_audit(&mut self, complete: bool) -> Result<(), String> {
        if complete {
            if self.diagnostics.complete_audits >= MAX_COMPLETE_AUDITS {
                return Err(self.cap("complete-audit budget exhausted"));
            }
            self.diagnostics.complete_audits += 1;
        } else {
            if self.diagnostics.partial_audits >= MAX_PARTIAL_AUDITS {
                return Err(self.cap("partial-audit budget exhausted"));
            }
            self.diagnostics.partial_audits += 1;
        }
        let collision_builds = self.quotas.validator_collision_builds_per_audit;
        let pair_visits = self.quotas.validator_pair_visits_per_audit;
        let collision_vertices = collision_builds.saturating_mul(MAX_COLLISION_VERTICES);
        let input_vertices = pair_visits.saturating_mul(2 * MAX_COLLISION_VERTICES);
        if self
            .diagnostics
            .validator_collision_builds
            .saturating_add(collision_builds)
            > self.quotas.max_validator_collision_builds
        {
            return Err(self.cap("validator collision-build budget exhausted"));
        }
        if self
            .diagnostics
            .validator_pair_visits
            .saturating_add(pair_visits)
            > self.quotas.max_validator_pair_visits
        {
            return Err(self.cap("validator pair-visit budget exhausted"));
        }
        if self
            .diagnostics
            .transformed_collision_vertices
            .saturating_add(collision_vertices)
            > self.quotas.max_transformed_collision_vertices
        {
            return Err(self.cap("transformed collision-vertex budget exhausted"));
        }
        if self
            .diagnostics
            .clipper_input_vertices
            .saturating_add(input_vertices)
            > self.quotas.max_clipper_input_vertices
        {
            return Err(self.cap("validator Clipper input-vertex budget exhausted"));
        }
        self.diagnostics.validator_collision_builds += collision_builds;
        self.diagnostics.validator_pair_visits += pair_visits;
        self.diagnostics.transformed_collision_vertices += collision_vertices;
        self.diagnostics.clipper_input_vertices += input_vertices;
        Ok(())
    }
}

pub(super) fn run_persistent_vacancy_population(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    parent: &GeneralCoupledSeparatorArmDiagnostics,
    parent_source: Option<String>,
    mode: usize,
) -> GeneralPersistentVacancyDiagnostics {
    let parent_is_pinned = parent_source.is_some();
    let mut diagnostics = GeneralPersistentVacancyDiagnostics {
        mode,
        seed_domain: PERSISTENT_VACANCY_SEED_DOMAIN,
        target_depth_mm: TARGET_DEPTH_MM,
        parent_source,
        ..GeneralPersistentVacancyDiagnostics::default()
    };
    let mut work = RunWork::new(pieces.len());
    match run_population(
        pieces,
        fast_settings,
        relaxed_settings.persistent_vacancy_target_depth_mm,
        parent,
        parent_is_pinned || relaxed_settings.persistent_vacancy_allow_unpinned_parent,
        mode,
        ConstructionSalt::from_settings(relaxed_settings),
        &mut diagnostics,
        &mut work,
    ) {
        Ok(Some((state, metrics))) => {
            diagnostics.exact_valid = true;
            diagnostics.independent_depth_mm = Some(metrics);
            let placements = fast_placements(&state, pieces, false);
            diagnostics.final_placement_fingerprint =
                Some(coupled_fast_placement_fingerprint(&placements));
            diagnostics.final_placements = coupled_placement_diagnostics(&placements);
        }
        Ok(None) => {
            diagnostics.failure_reason = Some(
                "persistent vacancy population exhausted its bounded layers without a complete state"
                    .to_owned(),
            );
        }
        Err(reason) => {
            diagnostics.cap_exhausted = reason.strip_prefix("cap: ").map(str::to_owned);
            diagnostics.failure_reason = Some(reason);
        }
    }
    diagnostics.work = work.diagnostics;
    diagnostics
}

/// Mode 24: bounded-depth reinsertion.
///
/// Compression by *ejection and reconstruction* under a hard bound, as
/// opposed to compression by overlap (deliberately overlapping a deep layout
/// and legalizing through the separator), which is a measured negative: it
/// always relaxes back to a worse depth.
///
/// Given a complete exact-valid parent and an absolute bound `D` (mm):
///
/// 1. Every placed piece's own extent along the depth (long) axis is measured
///    on the real transformed source polygon, using the same
///    `max_y + edge clearance` quantity `coupled_independent_source_depth`
///    maximizes over. A layout's depth is by definition the largest such
///    extent, so `D` is meaningful exactly when it is below the parent's.
/// 2. Every piece whose extent exceeds `D` is ejected; all others stay
///    pinned at their parent poses and form the fixed occupancy.
/// 3. The ejected pieces are reinserted one at a time by the construction
///    insertion machinery (`construct_candidate_poses`: skyline stations,
///    orientation priors, epsilon rungs, charged confirm rows, the
///    drop/slide/re-drop `construction_slide` contact walk). The bound is
///    enforced *geometrically* by handing that machinery a sheet whose long
///    axis is `D`, the same way mode 13 restricts reconstruction to its
///    target depth: `construction_confirm_row`'s `fits_rect` check then
///    rejects any out-of-bound pose before it is ever confirmed. Each
///    accepted pose is re-measured against `D` as an explicit contract check.
/// 4. If some piece has no in-bound pose the attempt fails cleanly for that
///    bound - reported, never exceeded. Otherwise the completed layout goes
///    through the standard exact validation and is reported in the ordinary
///    persistent-vacancy shape.
///
/// Reinsertion order is displaced-first by descending piece area (only
/// displaced pieces are reinserted, so that is the whole order), with the
/// `pieceId` breaking ties deterministically.
///
/// Budget: each reinserted piece costs exactly one `construct_candidate_poses`
/// call, i.e. one construction slot expansion charged to the shared
/// `VacancyQuotas` ledger, plus one collision build per kept piece to seed the
/// occupancy. A run therefore charges at most `piece_count` slot expansions,
/// which the existing per-piece construction term
/// (`construction_selected_piece_slots = CONSTRUCTION_RESTARTS *
/// CONSTRUCTION_BEAM_WIDTH * piece_count`) already funds many times over at
/// every piece count - so this mode needs no new aggregate term, and the
/// headroom is asserted in `bounded_reinsertion_fits_the_construction_budget`.
pub(super) fn run_bounded_reinsertion(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    parent: &GeneralCoupledSeparatorArmDiagnostics,
    parent_source: Option<String>,
) -> GeneralPersistentVacancyDiagnostics {
    let mut diagnostics = GeneralPersistentVacancyDiagnostics {
        mode: 24,
        seed_domain: BOUNDED_REINSERTION_SEED_DOMAIN,
        parent_source,
        ..GeneralPersistentVacancyDiagnostics::default()
    };
    let Some(bound_mm) = relaxed_settings.persistent_vacancy_target_depth_mm else {
        diagnostics.failure_reason =
            Some("persistent vacancy mode 24 requires an explicit depth bound".to_owned());
        return diagnostics;
    };
    if !bound_mm.is_finite() || bound_mm <= 0.0 {
        diagnostics.failure_reason = Some(
            "persistent vacancy mode 24 depth bound must be a positive finite value".to_owned(),
        );
        return diagnostics;
    }
    diagnostics.target_depth_mm = bound_mm;
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

    let mut work = RunWork::new(pieces.len());
    let mut bounded = GeneralPersistentVacancyBoundedReinsertionDiagnostics {
        bound_mm,
        ..GeneralPersistentVacancyBoundedReinsertionDiagnostics::default()
    };
    if let Err(reason) = bounded_reinsertion_inner(
        pieces,
        fast_settings,
        bound_mm,
        parent,
        &mut diagnostics,
        &mut bounded,
        &mut work,
    ) {
        diagnostics.cap_exhausted = reason.strip_prefix("cap: ").map(str::to_owned);
        diagnostics.failure_reason = Some(reason);
    }
    diagnostics.bounded_reinsertion = Some(bounded);
    diagnostics.work = work.diagnostics;
    diagnostics
}

/// One piece's extent along the depth (long) axis, measured on the real
/// transformed source polygon. This is the per-placement term of
/// `coupled_independent_source_depth`, which reports the maximum of exactly
/// this quantity over a layout - so a layout is within a bound precisely when
/// every piece is.
fn placement_long_axis_extent_mm(
    piece: GeneralFastPiece<'_>,
    placement: &RelaxedPlacement,
    settings: GeneralFastSettings,
) -> f64 {
    let edge_clearance_mm = settings
        .sheet_edge_clearance_mm
        .unwrap_or(settings.total_padding_mm / 2.0);
    transformed_source_max_y(piece, placement) + edge_clearance_mm
}

#[allow(clippy::too_many_arguments)]
fn bounded_reinsertion_inner(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    bound_mm: f64,
    parent: &GeneralCoupledSeparatorArmDiagnostics,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    bounded: &mut GeneralPersistentVacancyBoundedReinsertionDiagnostics,
    work: &mut RunWork,
) -> Result<(), String> {
    let parent_fast = diagnostic_fast_placements(&parent.final_placements);
    validate_and_measure_placements(pieces, &parent_fast, fast_settings)
        .map_err(|error| format!("persistent vacancy parent validation: {error}"))?;
    diagnostics.parent_fingerprint = Some(coupled_fast_placement_fingerprint(&parent_fast));
    let parent_depth_mm = coupled_independent_source_depth(pieces, &parent_fast, fast_settings)
        .map_err(|error| format!("persistent vacancy parent depth: {error}"))?;
    diagnostics.parent_independent_depth_mm = Some(parent_depth_mm);
    bounded.parent_depth_mm = parent_depth_mm;
    for piece in pieces {
        if piece.polygon.vertex_count() > MAX_SOURCE_FEATURES {
            return Err(format!(
                "piece {} exceeds the {MAX_SOURCE_FEATURES}-feature experiment cap",
                piece.id
            ));
        }
    }

    diagnostics.attempted = true;

    // The insertion sheet is clamped to the bound, so `fits_rect` inside
    // every charged confirm row rejects an out-of-bound pose before it can
    // be confirmed. That clamp runs on the collision polygon, which carries
    // the conservative offset allowance the reported source measure does
    // not, so it is stricter than the bound by exactly that allowance: it
    // can never admit a pose the explicit re-measure below would reject.
    let bound_settings = GeneralFastSettings {
        sheet_long_axis_mm: bound_mm,
        ..fast_settings
    };
    let anchor =
        relaxed_state_from_diagnostics_with_target(pieces, &parent.final_placements, bound_mm)?;
    let bound_grid = grid_key(bound_mm);

    // Mode 24's ejection rule: every piece whose own extent exceeds the bound,
    // and nothing else.
    let extents = (0..pieces.len())
        .map(|index| {
            placement_long_axis_extent_mm(pieces[index], &anchor.placements[index], fast_settings)
        })
        .collect::<Vec<_>>();
    let ejected = (0..pieces.len())
        .filter(|index| grid_key(extents[*index]) > bound_grid)
        .collect::<Vec<_>>();
    bounded.ejected_count = ejected.len();
    bounded.kept_count = pieces.len() - ejected.len();

    // Every piece mode 24 ejects protrudes past the bound, so each carries the
    // pose it protruded from. That pose is out of bound and will be rejected
    // by the clamped confirm row, but the cloud around it is exactly the set
    // of small local corrections that could bring the piece back inside
    // without giving up its neighbourhood. The bound overrun is the mode's
    // measured residue scale, so the cloud is sized from it.
    let overrun_mm = ejected
        .iter()
        .map(|index| extents[*index] - bound_mm)
        .fold(0.0f64, f64::max);
    let pass = replace_ejected_under_bound(
        pieces,
        fast_settings,
        bound_settings,
        bound_mm,
        &anchor,
        &ejected,
        None,
        None,
        BOUNDED_REINSERTION_SEED_DOMAIN,
        &AnchorLocalSeeding::with_residue_scale(Some(overrun_mm)),
        work,
    )?;
    diagnostics.initial_state_fingerprint = Some(pass.initial_state_fingerprint);
    diagnostics.initial_active_piece_ids = pass.initial_active_piece_ids;
    diagnostics.initial_inactive_piece_ids = pass.order_piece_ids;
    diagnostics.initial_inactive_order_hash = Some(pass.order_hash);
    for outcome in &pass.pieces {
        let mut row = GeneralPersistentVacancyBoundedReinsertionPieceRow {
            piece_id: outcome.piece_id.clone(),
            parent_extent_mm: extents[outcome.index],
            candidates_considered: outcome.candidates_considered,
            bound_rejections: outcome.bound_rejections,
            reinserted: outcome.placed_extent_mm.is_some(),
            placed_extent_mm: outcome.placed_extent_mm,
            anchor_local_candidates: outcome.anchor_local_candidates,
            anchor_local_finalists: outcome.anchor_local_finalists,
            ..GeneralPersistentVacancyBoundedReinsertionPieceRow::default()
        };
        if row.reinserted {
            bounded.reinserted_count = bounded.reinserted_count.saturating_add(1);
        } else {
            // A clean per-bound failure: the bound is reported as
            // unreachable for this piece rather than exceeded.
            row.failure_reason = Some(format!(
                "no exact-valid pose for piece {} within the {bound_mm} mm bound",
                outcome.piece_id
            ));
            bounded.failed_piece_id = Some(outcome.piece_id.clone());
        }
        bounded.pieces.push(row);
    }
    diagnostics.construction = Some(pass.construction);
    if let Some(failed) = &bounded.failed_piece_id {
        diagnostics.publication_rejections = diagnostics.publication_rejections.saturating_add(1);
        diagnostics.failure_reason = Some(format!(
            "bounded reinsertion could not place piece {failed} within the {bound_mm} mm bound"
        ));
        return Ok(());
    }
    let state = pass.state;
    diagnostics.direct_insertions = bounded.reinserted_count;
    diagnostics.complete_states = diagnostics.complete_states.saturating_add(1);

    let final_placements = fast_placements(&state, pieces, false);
    validate_and_measure_placements(pieces, &final_placements, fast_settings)
        .map_err(|error| format!("bounded reinsertion final validation: {error}"))?;
    let final_depth_mm = coupled_independent_source_depth(pieces, &final_placements, fast_settings)
        .map_err(|error| format!("bounded reinsertion final depth: {error}"))?;
    diagnostics.exact_valid = true;
    diagnostics.independent_depth_mm = Some(final_depth_mm);
    diagnostics.final_placement_fingerprint =
        Some(coupled_fast_placement_fingerprint(&final_placements));
    diagnostics.final_placements = coupled_placement_diagnostics(&final_placements);
    bounded.final_depth_mm = Some(final_depth_mm);
    Ok(())
}

/// Whether the shared re-placement primitive seeds candidate poses at the
/// piece's own vacated pose, and at what scale.
///
/// [`AnchorLocalSeeding::disabled`] reproduces the skyline-only generator
/// exactly, which is what the from-scratch constructor asks for: there is no
/// vacated pose on an empty sheet, and the anchor a construction carries is a
/// pose *prior*, not a pose the piece was just lifted out of.
#[derive(Clone, Debug, Default, PartialEq)]
struct AnchorLocalSeeding {
    /// Seed at the vacated pose in addition to - never instead of - the
    /// skyline stations.
    enabled: bool,
    /// The caller's measured local residue scale, in millimetres. `None` when
    /// the caller has no measure, in which case the cloud is sized from the
    /// piece's own extent alone.
    residue_scale_mm: Option<f64>,
    /// Per piece index, the unit directions that piece must travel to come
    /// apart from whatever it was in conflict with, in the caller's own
    /// deterministic order. Empty for a caller whose ejection is not driven by
    /// a pairwise conflict.
    ///
    /// These matter more than the magnitudes do. At record density the
    /// feasible set around a conflicting pose is a sliver whose width is the
    /// layout's slack, so a cloud that only samples the axes and the diagonals
    /// is a net with 45-degree holes in it and reliably misses; aimed along
    /// the pair's own closest-approach witness, the same magnitudes land.
    separating_directions: BTreeMap<usize, Vec<(f64, f64)>>,
    /// Per piece index, the translations a single-piece separating projection
    /// passed through on its way to pulling that piece clear of everything it
    /// violates - the displacements derived from the conflict rather than
    /// sampled near it.
    ///
    /// A single witness direction clears one pair; a piece wedged by several
    /// has to satisfy all of them at once, and the combination is not a
    /// direction anything local can guess. These are those combinations - the
    /// projection's whole trajectory, since a single movable piece makes the
    /// iteration oscillate rather than settle - seeded as exact poses *and*,
    /// through the final one, as the leading cloud direction.
    projected_displacements: BTreeMap<usize, Vec<(f64, f64)>>,
    /// Per piece index, absolute translations of poses *other* ejected pieces
    /// vacated in the same round, nearest first.
    ///
    /// A cloud around a piece's own vacated pose can only ever express "this
    /// piece moves a little". When two pieces are jointly over-compressed into
    /// each other, the move that legalizes them is often an exchange - each
    /// takes the pocket the other left - and the exchange is not in any
    /// single-piece neighbourhood at any radius the cloud can afford. Seeding
    /// the peers' vacated translations puts it in the candidate stream
    /// directly, at three poses per piece.
    ///
    /// Empty for every caller that ejects pieces one at a time, which is what
    /// keeps their candidate streams bit-identical.
    peer_poses: BTreeMap<usize, Vec<(f64, f64)>>,
    /// Per piece index, the orientation-perturbed variants of that piece's
    /// vacated pose - the continuous-angle ladder and, where the request allows
    /// it, the mirror flip - each carrying the local bounding box that
    /// re-centres it on the pocket the piece came out of.
    ///
    /// Empty for every caller but modes 32 and 33, which is what keeps every
    /// other caller's candidate stream bit-identical: an empty map emits no
    /// orientation candidate, spends no orientation row, and leaves the
    /// attribution block off the diagnostics entirely.
    orientation_variants: BTreeMap<usize, Vec<OrientationVariant>>,
}

/// One orientation-perturbed variant of a piece's vacated pose.
///
/// The bounding box is the piece's collision polygon at this orientation and
/// zero translation, which is what both clamps the variant into the
/// piece-feasible band and supplies the re-centring shift: matching this box's
/// centre to the vacated box's centre is what makes the rung a rotation of the
/// piece in place.
#[derive(Clone, Copy, Debug, PartialEq)]
struct OrientationVariant {
    rotation_deg: f64,
    mirrored: bool,
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
}

/// Where a construction finalist's seed came from. Carried out of
/// `construct_candidate_poses` so callers can attribute a placement without
/// re-deriving it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct CandidateProvenance {
    /// Seeded under the unrotated catalog orientation prior rather than the
    /// anchor's own orientation.
    zero_prior: bool,
    /// Seeded at the piece's vacated pose rather than at a skyline station.
    anchor_local: bool,
    /// The vacated pose itself - the single candidate whose feasibility is a
    /// matter of record rather than of search. Exactly one candidate per slot
    /// carries this, and only when anchor-local seeding is on.
    vacated: bool,
    /// Seeded by the orientation-perturbation stream: the vacated pose's own
    /// translation neighbourhood, carried onto a rotated or mirrored variant of
    /// the vacated orientation and re-centred on the vacated footprint.
    orientation_perturbed: bool,
}

impl AnchorLocalSeeding {
    /// Skyline stations only.
    fn disabled() -> Self {
        Self::default()
    }

    /// Anchor-local seeding with the caller's measured residue scale, when it
    /// is a usable positive length.
    fn with_residue_scale(residue_scale_mm: Option<f64>) -> Self {
        Self {
            enabled: true,
            residue_scale_mm: residue_scale_mm
                .filter(|scale| scale.is_finite() && *scale > 0.0)
                .map(snap_mm)
                .filter(|scale| *scale > 0.0),
            separating_directions: BTreeMap::new(),
            projected_displacements: BTreeMap::new(),
            peer_poses: BTreeMap::new(),
            orientation_variants: BTreeMap::new(),
        }
    }

    /// The orientation-perturbed variants seeded for one piece, in the fixed
    /// ladder order the builder produced. Empty for every caller that did not
    /// arm the stream.
    fn orientation_variants(&self, piece_index: usize) -> &[OrientationVariant] {
        if !self.enabled {
            return &[];
        }
        self.orientation_variants
            .get(&piece_index)
            .map_or(&[][..], Vec::as_slice)
    }

    /// The peer vacated translations seeded for one piece: finite, and never
    /// more than [`JOINT_REPLACEMENT_PEER_POSES`] of them.
    fn peer_poses(&self, piece_index: usize) -> Vec<(f64, f64)> {
        if !self.enabled {
            return Vec::new();
        }
        self.peer_poses
            .get(&piece_index)
            .map(|poses| {
                poses
                    .iter()
                    .copied()
                    .filter(|(x, y)| x.is_finite() && y.is_finite())
                    .take(JOINT_REPLACEMENT_PEER_POSES)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The piece's projected separating displacements: the usable, non-zero
    /// iterates of its projection trajectory, in the order the projection
    /// produced them.
    fn projected_displacements(&self, piece_index: usize) -> Vec<(f64, f64)> {
        self.projected_displacements
            .get(&piece_index)
            .map(|trajectory| {
                trajectory
                    .iter()
                    .copied()
                    .filter(|(x, y)| {
                        x.is_finite() && y.is_finite() && (grid_key(*x) != 0 || grid_key(*y) != 0)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The displacement directions for one piece: its own escape directions
    /// first, capped at [`ANCHOR_LOCAL_SEPARATION_DIRECTIONS`] and followed by
    /// their normalized sum when there is more than one - a piece wedged by
    /// two neighbours has to clear both at once - then the unaimed fan, which
    /// is the general fallback and the only source for a caller with no
    /// conflict geometry to offer.
    fn directions(&self, piece_index: usize) -> Vec<(f64, f64)> {
        let mut directions: Vec<(f64, f64)> = Vec::new();
        if !self.enabled {
            return directions;
        }
        let push = |direction: (f64, f64), directions: &mut Vec<(f64, f64)>| {
            let (x, y) = direction;
            if !x.is_finite() || !y.is_finite() {
                return;
            }
            let length = x.hypot(y);
            if length <= 0.0 {
                return;
            }
            let unit = (x / length, y / length);
            // Deduplicated on the placement grid so a repeated witness cannot
            // re-spend the whole magnitude ladder.
            if directions.iter().any(|kept| {
                grid_key(kept.0) == grid_key(unit.0) && grid_key(kept.1) == grid_key(unit.1)
            }) {
                return;
            }
            directions.push(unit);
        };
        // The trajectory's endpoint is the projection's own best answer to
        // "which way out", so it leads the magnitude ladder as well.
        if let Some(projected) = self.projected_displacements(piece_index).last().copied() {
            push(projected, &mut directions);
        }
        if let Some(separating) = self.separating_directions.get(&piece_index) {
            let mut sum = (0.0f64, 0.0f64);
            for direction in separating.iter().take(ANCHOR_LOCAL_SEPARATION_DIRECTIONS) {
                push(*direction, &mut directions);
                sum = (sum.0 + direction.0, sum.1 + direction.1);
            }
            if separating.len() > 1 {
                push(sum, &mut directions);
            }
        }
        for direction in ANCHOR_LOCAL_FAN_DIRECTIONS {
            push(direction, &mut directions);
        }
        directions
    }

    /// The cloud's displacement magnitudes for a piece whose bounding box in
    /// the seeded orientation is `width_mm` by `height_mm`: ascending, unique
    /// on the placement grid, and never longer than [`ANCHOR_LOCAL_MAGNITUDES`].
    fn magnitudes_mm(&self, width_mm: f64, height_mm: f64) -> Vec<f64> {
        if !self.enabled {
            return Vec::new();
        }
        let extent = width_mm.min(height_mm).max(0.0);
        let mut magnitudes = Vec::with_capacity(ANCHOR_LOCAL_MAGNITUDES);
        for fraction in ANCHOR_LOCAL_EXTENT_FRACTIONS {
            magnitudes.push(extent * fraction);
        }
        if let Some(scale) = self.residue_scale_mm {
            for multiple in ANCHOR_LOCAL_RESIDUE_MULTIPLES {
                magnitudes.push(scale * multiple);
            }
        }
        // Ascending on the placement grid, then deduplicated on it: a
        // magnitude that snaps onto one already in the list would only re-spend
        // charged confirmation rows on poses the primitive already tried.
        magnitudes = magnitudes.into_iter().map(snap_mm).collect();
        magnitudes.sort_by_key(|magnitude| grid_key(*magnitude));
        magnitudes.dedup_by_key(|magnitude| grid_key(*magnitude));
        magnitudes.retain(|magnitude| grid_key(*magnitude) > 0);
        magnitudes
    }
}

/// The orientation-perturbed variants of one ejected piece's vacated pose, in
/// the order the stream seeds them.
///
/// Rotation rungs lead, ascending in magnitude with the positive sign first;
/// the mirror family follows, so a row-budget cut truncates the mirror variants
/// before the rotation variants. The vacated orientation itself is never
/// re-emitted here - it is the anchor-local stream's own leading candidate, and
/// re-emitting it would spend an orientation row on a pose already tried.
/// Orientations the request forbids (`allow_rotation`, `allow_mirror`) are not
/// generated at all, so a fixed-orientation piece contributes nothing and a
/// non-mirrorable one contributes only the ladder.
///
/// One charged collision build per surviving variant, taken once per ejected
/// piece per pass rather than once per insertion order, which is why the
/// builder lives here and its output is carried on [`AnchorLocalSeeding`].
fn orientation_perturbation_variants(
    piece: GeneralFastPiece<'_>,
    vacated: &RelaxedPlacement,
    settings: GeneralFastSettings,
    work: &mut RunWork,
) -> Result<Vec<OrientationVariant>, String> {
    let vacated_key = (angle_key(vacated.rotation_deg), vacated.mirrored);
    let mut orientations: Vec<(f64, bool)> = Vec::with_capacity(ORIENTATION_PERTURBATION_VARIANTS);
    let push = |orientations: &mut Vec<(f64, bool)>, rotation_deg: f64, mirrored: bool| {
        let rotation_deg = continuous_angle(rotation_deg);
        let key = (angle_key(rotation_deg), mirrored);
        if key == vacated_key {
            return;
        }
        if orientations
            .iter()
            .any(|(angle, mirror)| (angle_key(*angle), *mirror) == key)
        {
            return;
        }
        orientations.push((rotation_deg, mirrored));
    };
    for mirrored in [vacated.mirrored, !vacated.mirrored] {
        if mirrored != vacated.mirrored {
            if !piece.allow_mirror {
                continue;
            }
            // The pure mirror flip: no rotation at all, which is the one
            // orientation change a fixed-orientation mirrorable piece can make.
            push(&mut orientations, vacated.rotation_deg, mirrored);
        }
        if !piece.allow_rotation {
            continue;
        }
        for delta_deg in ORIENTATION_PERTURBATION_LADDER_DEG {
            for signed_deg in [delta_deg, -delta_deg] {
                push(
                    &mut orientations,
                    vacated.rotation_deg + signed_deg,
                    mirrored,
                );
            }
        }
    }
    let mut variants = Vec::with_capacity(orientations.len());
    for (rotation_deg, mirrored) in orientations {
        let local = RelaxedPlacement {
            input_index: vacated.input_index,
            rotation_deg,
            mirrored,
            translate_x: 0.0,
            translate_y: 0.0,
        };
        let collision = build_collision(piece, &local, settings, work)?;
        // A variant whose collision polygon came back empty carries no
        // geometry to re-centre; dropping it is the same refusal the priors
        // make, except that a missing perturbation is never fatal.
        let Some(bounds) = collision.bounds() else {
            continue;
        };
        variants.push(OrientationVariant {
            rotation_deg,
            mirrored,
            min_x: bounds.min_x,
            max_x: bounds.max_x,
            min_y: bounds.min_y,
            max_y: bounds.max_y,
        });
    }
    Ok(variants)
}

/// Arms the orientation-perturbed stream on `anchor_local` for every ejected
/// piece, reading each piece's vacated pose off the anchor.
///
/// A variant build that trips a work cap is reported as a cap failure exactly
/// like any other charged build; a variant set that comes back empty simply
/// leaves that piece with the legacy stream, which is what a fixed-orientation
/// non-mirrorable piece gets.
fn arm_orientation_perturbation(
    pieces: &[GeneralFastPiece<'_>],
    anchor: &RelaxedState,
    settings: GeneralFastSettings,
    ejected_indices: &[usize],
    anchor_local: &mut AnchorLocalSeeding,
    work: &mut RunWork,
) -> Result<(), String> {
    for index in ejected_indices.iter().copied() {
        let variants = orientation_perturbation_variants(
            pieces[index],
            &anchor.placements[index],
            settings,
            work,
        )?;
        if !variants.is_empty() {
            anchor_local.orientation_variants.insert(index, variants);
        }
    }
    Ok(())
}

/// What the shared bounded re-placement primitive did to one ejected piece.
#[derive(Debug)]
struct ReplacedPieceOutcome {
    index: usize,
    piece_id: String,
    candidates_considered: usize,
    bound_rejections: usize,
    /// The extent of the pose that was accepted, or `None` when the piece had
    /// no in-bound pose at all.
    placed_extent_mm: Option<f64>,
    /// Anchor-local candidates seeded for this piece, and the exact-valid
    /// finalists they produced. Both zero when the piece had no recorded prior
    /// pose to seed from.
    anchor_local_candidates: usize,
    anchor_local_finalists: usize,
    /// The orientation-perturbed stream's own attribution for this piece.
    /// `None` for every caller that did not arm it, which is what keeps the
    /// legacy modes' diagnostics byte-identical.
    orientation: Option<GeneralOrientationSeedingRow>,
}

/// What one run of the shared bounded re-placement primitive produced.
struct BoundedReplacementPass {
    /// The layout after re-placement. Complete exactly when every ejected
    /// piece found a pose; otherwise the failed piece is still inactive.
    state: VacancyState,
    /// The state *as ejected*, before any piece was put back.
    initial_state_fingerprint: String,
    initial_active_piece_ids: Vec<String>,
    /// The ejected pieces in the order they were re-placed.
    order_piece_ids: Vec<String>,
    order_hash: String,
    /// One row per attempted piece, in re-placement order. Stops at the first
    /// piece that could not be placed.
    pieces: Vec<ReplacedPieceOutcome>,
    construction: GeneralPersistentVacancyConstructionDiagnostics,
}

impl BoundedReplacementPass {
    fn failed_piece_id(&self) -> Option<&str> {
        self.pieces
            .last()
            .filter(|row| row.placed_extent_mm.is_none())
            .map(|row| row.piece_id.as_str())
    }
}

/// Removes `ejected` from `anchor` and re-places those pieces, one at a time,
/// with the construction insertion machinery under a sheet clamped to
/// `bound_mm`.
///
/// This is the single insertion path shared by mode 24 (bounded-depth
/// reinsertion, which ejects the pieces that stick out past the bound) and
/// mode 28 (conflict-targeted re-placement, which ejects the pieces incident
/// to a clearance violation). The two modes differ only in *which* pieces they
/// hand over and in what they do with the result; the ejection, the fixed
/// occupancy, the re-placement order, and the per-pose bound contract are
/// identical, and live here so they cannot drift apart.
///
/// Re-placement order is descending piece area with `pieceId` breaking ties,
/// unless the caller hands one in through `order_override` - which mode 29 does
/// because *which order* is the variable it searches over. An override must be
/// a permutation of `ejected`; anything else is refused rather than silently
/// re-placing a different set.
///
/// Which *pose* each piece commits to is the second variable, and
/// `finalist_choice` is how a caller searches it: entry `i` selects the
/// `i`-th piece's rank among its own in-bound finalists, so the all-zeros
/// choice - and `None` - is the greedy shallowest-first commit this primitive
/// has always made. Mode 29's beam uses it to back out of a first-finalist
/// commit that boxed a later piece out; nothing else passes anything but
/// `None`, so every other caller's stream is bit-identical.
/// The bound is enforced twice: geometrically, because
/// `construction_confirm_row`'s `fits_rect` runs against the clamped sheet and
/// so refuses an out-of-bound pose before it is ever confirmed, and then again
/// explicitly, because every confirmed pose is re-measured against `bound_mm`
/// on the real transformed source polygon.
///
/// Every ejected piece here has a *recorded prior pose* - the pose it is being
/// lifted out of, which `anchor` still carries - so `anchor_local` is what both
/// callers use to ask for anchor-local seeding on top of the skyline stations.
/// A caller with no prior pose to offer passes
/// [`AnchorLocalSeeding::disabled`] and gets exactly the skyline-only
/// generator.
///
/// Budget: one charged collision build per kept piece plus one
/// `construct_candidate_poses` call per ejected piece. Ejecting *every* piece
/// is the worst case at both terms, which is what
/// `bounded_reinsertion_fits_the_construction_budget` asserts against the
/// existing per-piece construction quota - so neither caller needs a new
/// aggregate term. Anchor-local candidates carry their own per-piece row
/// budget on top of that cap, which the same assertion funds: seeding a
/// vacated pose must be able to *add* candidates without spending the rows the
/// stations would have used, or a piece that used to be re-placed from the
/// skyline stops being re-placed at all.
#[allow(clippy::too_many_arguments)]
fn replace_ejected_under_bound(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    bound_settings: GeneralFastSettings,
    bound_mm: f64,
    anchor: &RelaxedState,
    ejected: &[usize],
    order_override: Option<&[usize]>,
    finalist_choice: Option<&[usize]>,
    seed_domain: u64,
    anchor_local: &AnchorLocalSeeding,
    work: &mut RunWork,
) -> Result<BoundedReplacementPass, String> {
    let bound_grid = grid_key(bound_mm);
    let mut state = VacancyState {
        placements: anchor.placements.clone(),
        active: vec![true; pieces.len()],
        collisions: vec![None; pieces.len()],
        last_transition: None,
    };
    for index in ejected {
        state.active[*index] = false;
    }

    // Fixed occupancy: one charged collision build per kept piece.
    for index in 0..pieces.len() {
        if state.active[index] {
            let collision = build_collision(
                pieces[index],
                &state.placements[index],
                bound_settings,
                work,
            )?;
            state.collisions[index] = Some(Arc::new(collision));
        }
    }
    let initial_state_fingerprint = state_fingerprint(&state, pieces);
    let initial_active_piece_ids = active_ids(&state, pieces);

    // Displaced-first by descending piece area; `pieceId` breaks ties. Only
    // displaced pieces are re-placed, so this is the whole order.
    let order = match order_override {
        Some(order) => {
            let mut sorted = order.to_vec();
            sorted.sort_unstable();
            let mut expected = ejected.to_vec();
            expected.sort_unstable();
            if sorted != expected {
                return Err(
                    "re-placement order override is not a permutation of the ejection set"
                        .to_owned(),
                );
            }
            order.to_vec()
        }
        None => bounded_replacement_order(pieces, ejected),
    };
    let order_piece_ids = order
        .iter()
        .map(|index| pieces[*index].id.to_owned())
        .collect::<Vec<_>>();
    let order_hash = id_order_hash(&order, pieces);

    let replacement_seed = parent_seed_key(&state, pieces) ^ seed_domain ^ (bound_grid as u64);
    let mut construction = GeneralPersistentVacancyConstructionDiagnostics {
        hint_stations_per_slot: CONSTRUCTION_HINT_STATIONS,
        rows_per_piece_cap: CONSTRUCTION_ROWS_PER_PIECE,
        finalists_per_slot: CONSTRUCTION_FINALISTS_PER_SLOT,
        ..GeneralPersistentVacancyConstructionDiagnostics::default()
    };

    let mut rows = Vec::with_capacity(order.len());
    for (ordinal, index) in order.iter().copied().enumerate() {
        let mut row = ReplacedPieceOutcome {
            index,
            piece_id: pieces[index].id.to_owned(),
            candidates_considered: 0,
            bound_rejections: 0,
            placed_extent_mm: None,
            anchor_local_candidates: 0,
            anchor_local_finalists: 0,
            orientation: None,
        };
        let seeded_before = construction.anchor_local_candidates;
        let orientation_seeded_before = construction.orientation_candidates;
        let orientation_rows_before = construction.orientation_rows;
        let orientation_variants = anchor_local.orientation_variants(index).len();
        let finalists = construct_candidate_poses(
            pieces,
            bound_settings,
            anchor,
            &state,
            replacement_seed,
            ordinal,
            index,
            anchor_local,
            &mut construction,
            work,
        )?;
        row.candidates_considered = finalists.len();
        row.anchor_local_candidates = construction
            .anchor_local_candidates
            .saturating_sub(seeded_before);
        row.anchor_local_finalists = finalists
            .iter()
            .filter(|(_, _, provenance)| provenance.anchor_local)
            .count();
        // The orientation stream's attribution block exists only for a caller
        // that armed it, so that every other caller's diagnostics are the JSON
        // they were before the stream existed.
        if orientation_variants > 0 {
            row.orientation = Some(GeneralOrientationSeedingRow {
                variants: orientation_variants,
                candidates: construction
                    .orientation_candidates
                    .saturating_sub(orientation_seeded_before),
                rows: construction
                    .orientation_rows
                    .saturating_sub(orientation_rows_before),
                finalists: finalists
                    .iter()
                    .filter(|(_, _, provenance)| provenance.orientation_perturbed)
                    .count(),
                ..GeneralOrientationSeedingRow::default()
            });
        }
        // The finalists arrive ranked by the landing-frontier key, so the
        // first one still inside the bound is the shallowest confirmed pose,
        // and rank zero - the default - commits it. A caller searching poses
        // asks for a later in-bound rank instead; a rank the piece does not
        // have leaves `chosen` empty and fails the pass on this piece, exactly
        // as an empty finalist list does.
        let wanted_rank = finalist_choice
            .and_then(|choice| choice.get(ordinal).copied())
            .unwrap_or(0);
        let mut chosen = None;
        let mut in_bound_rank = 0usize;
        for (pose, collision, provenance) in finalists {
            let extent = placement_long_axis_extent_mm(pieces[index], &pose, fast_settings);
            if grid_key(extent) <= bound_grid {
                if in_bound_rank == wanted_rank {
                    chosen = Some((pose, collision, extent, provenance));
                    break;
                }
                in_bound_rank = in_bound_rank.saturating_add(1);
                continue;
            }
            row.bound_rejections = row.bound_rejections.saturating_add(1);
        }
        // The accepted-pose attribution: which of the four candidate families
        // the pose this piece committed to actually came from. This is the
        // measurement the orientation mechanism is judged on, so it is read off
        // the committed finalist's own provenance rather than reconstructed
        // from the pose - the contact walk translates a confirmed pose but
        // never rotates it, so the orientation the finalist carries is the
        // orientation it was seeded at.
        if let (Some(orientation), Some((pose, _, _, provenance))) =
            (row.orientation.as_mut(), chosen.as_ref())
        {
            if provenance.vacated {
                orientation.accepted_vacated = 1;
            } else if provenance.orientation_perturbed {
                orientation.accepted_orientation = 1;
            } else if provenance.anchor_local {
                orientation.accepted_anchor_local = 1;
            } else {
                orientation.accepted_station = 1;
            }
            let vacated_pose = &anchor.placements[index];
            orientation.accepted_rotation_deg = Some(pose.rotation_deg);
            orientation.accepted_rotation_delta_deg = Some(
                angle_from_key(angle_key(pose.rotation_deg) - angle_key(vacated_pose.rotation_deg)),
            );
            orientation.accepted_mirror_flipped = Some(pose.mirrored != vacated_pose.mirrored);
        }
        match chosen {
            Some((pose, collision, extent, _)) => {
                state.placements[index] = pose;
                state.active[index] = true;
                state.collisions[index] = Some(collision);
                row.placed_extent_mm = Some(extent);
                rows.push(row);
            }
            None => {
                // A clean per-bound failure: the bound is reported as
                // unreachable for this piece rather than exceeded, and the
                // remaining pieces are not attempted.
                rows.push(row);
                break;
            }
        }
    }

    Ok(BoundedReplacementPass {
        state,
        initial_state_fingerprint,
        initial_active_piece_ids,
        order_piece_ids,
        order_hash,
        pieces: rows,
        construction,
    })
}

/// Mode 28: conflict-targeted re-placement.
///
/// The repair class the micro-legalizer provably cannot reach. Mode 26's
/// clamped-sheet ladder routinely converges a deep layout past its requested
/// bound and is then rejected over a handful of clearance-violating *pairs*
/// whose deficits are millimetre-scale - miter-joined envelope conflicts far
/// from the material closest-approach point. Mode 27's micro-legalizer
/// correctly refuses those: its model is translation-only, and a millimetre of
/// travel is a search move, not a projection. The residue is nevertheless
/// tiny and *local*, so the instrument it actually calls for is local
/// re-placement:
///
/// 1. Survey the violation graph of the state against the real publication
///    contracts (the same survey mode 27 runs).
/// 2. Choose the **ejection set**: for each violating pair, the endpoint whose
///    removal clears more violation mass, with `pieceId` breaking ties; the
///    union over pairs. Because one endpoint of every violating pair is
///    ejected, the remaining layout provably carries no violating pair at all.
/// 3. Remove them. Any *boundary* residue left among the kept pieces is a
///    projection problem again, so the micro-legalizer is run on the remaining
///    sub-layout to clear it.
/// 4. Re-place the ejected pieces with the construction insertion machinery
///    under a sheet clamped to the bound - the same
///    [`replace_ejected_under_bound`] primitive mode 24 uses. Ejection vacates
///    space exactly where the conflicts sat, which is the whole hypothesis:
///    mode 24's negative was measured from a *converged exact-valid* layout,
///    where no free space existed anywhere; a compressed rejected state is a
///    different animal.
/// 5. Exact-validate the whole state against the real request and publish only
///    on success. Anything else fails cleanly, with the ejected set and the
///    per-piece outcomes reported.
///
/// The mode is deliberately pointed at states that do *not* validate, so - like
/// mode 27 - it measures its parent rather than gating on it.
///
/// # Mode 32
///
/// Mode 32 is this pipeline with the orientation-perturbation stream armed: the
/// same survey, the same vertex-cover ejection, the same kept-sub-layout
/// micro-legalization, the same insertion order, the same bound contract and
/// the same exact validator, with continuous-angle variants of each ejected
/// piece's vacated pose added to its candidate stream *behind* every pose mode
/// 28 could reach. It even shares mode 28's seed domain, so the legacy part of
/// the candidate stream is literally the same poses in the same order; mode 32
/// can therefore only find what mode 28 found, or find that and then more.
pub(super) fn run_replacement_repair(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    parent: &GeneralCoupledSeparatorArmDiagnostics,
    parent_source: Option<String>,
    orientation_perturbation: bool,
) -> GeneralPersistentVacancyDiagnostics {
    let mode = if orientation_perturbation { 32 } else { 28 };
    let mut diagnostics = GeneralPersistentVacancyDiagnostics {
        mode,
        seed_domain: REPLACEMENT_REPAIR_SEED_DOMAIN,
        parent_source,
        ..GeneralPersistentVacancyDiagnostics::default()
    };
    let Some(bound_mm) = relaxed_settings.persistent_vacancy_target_depth_mm else {
        diagnostics.failure_reason = Some(format!(
            "persistent vacancy mode {mode} requires an explicit depth bound"
        ));
        return diagnostics;
    };
    if !bound_mm.is_finite() || bound_mm <= 0.0 {
        diagnostics.failure_reason = Some(format!(
            "persistent vacancy mode {mode} depth bound must be a positive finite value"
        ));
        return diagnostics;
    }
    diagnostics.target_depth_mm = bound_mm;
    if pieces.is_empty() {
        diagnostics.failure_reason =
            Some("persistent vacancy experiment requires at least one piece".to_owned());
        return diagnostics;
    }
    if parent.final_placements.len() != pieces.len() {
        diagnostics.failure_reason =
            Some("conflict-targeted re-placement requires a complete parent layout".to_owned());
        return diagnostics;
    }

    let parent_placements = diagnostic_fast_placements(&parent.final_placements);
    diagnostics.parent_fingerprint = Some(coupled_fast_placement_fingerprint(&parent_placements));
    diagnostics.initial_state_fingerprint = diagnostics.parent_fingerprint.clone();
    // Like mode 27, this mode is *meant* to be pointed at states that do not
    // validate, so the parent is measured rather than gated on.
    diagnostics.parent_independent_depth_mm =
        coupled_independent_source_depth(pieces, &parent_placements, fast_settings).ok();
    diagnostics.attempted = true;

    let outcome = replacement_repair(
        pieces,
        &parent_placements,
        fast_settings,
        bound_mm,
        orientation_perturbation,
    );
    diagnostics.work = outcome.diagnostics.work;
    diagnostics.cap_exhausted = outcome.diagnostics.cap_exhausted.clone();
    match &outcome.repaired {
        Some(repaired) => {
            diagnostics.complete_states = 1;
            diagnostics.direct_insertions = outcome.diagnostics.replaced_count;
            diagnostics.exact_valid = true;
            diagnostics.independent_depth_mm = outcome.diagnostics.depth_mm;
            diagnostics.final_placement_fingerprint =
                Some(coupled_fast_placement_fingerprint(repaired));
            diagnostics.final_placements = coupled_placement_diagnostics(repaired);
        }
        None => {
            diagnostics.publication_rejections = 1;
            diagnostics.failure_reason = Some(
                outcome
                    .diagnostics
                    .skipped_reason
                    .clone()
                    .or_else(|| outcome.diagnostics.rejection_reason.clone())
                    .unwrap_or_else(|| {
                        "conflict-targeted re-placement produced no exact-valid state".to_owned()
                    }),
            );
        }
    }
    // The ejection set in slot order; its re-placement order and that order's
    // hash live in the pass's own block, where they are documented together.
    diagnostics.initial_inactive_piece_ids = outcome.diagnostics.ejected_piece_ids.clone();
    diagnostics.replacement_repair = Some(outcome.diagnostics);
    diagnostics
}

/// Mode 29: joint multi-piece re-placement.
///
/// The last residue class the compress-repair stack could not reach. Tier one
/// (mode 27's projection) handles rounding-scale and boundary-class residues;
/// tier two (mode 28) ejects a *vertex cover* of the violation graph and
/// re-places those pieces one at a time, which repairs an interior pocket up to
/// about half a millimetre of correction. Beyond that the measured terminal
/// states of a deep mode-26 frontier carry multi-millimetre deficits in two- and
/// three-piece components, and both tiers correctly refuse: no nearby feasible
/// *single*-piece pose exists, because the piece that would have to move is
/// wedged against a partner that tier two deliberately left in place.
///
/// The joint pass changes exactly that:
///
/// 1. Eject **every** piece of every component that carries a violating pair,
///    bounded by the same admission cap tier two uses. Both sides of each
///    conflict come out, so the vacated space is the conflict's whole
///    footprint.
/// 2. Re-place them **jointly**, searching over insertion order: every
///    permutation of the ejection set up to
///    [`JOINT_REPLACEMENT_MAX_PERMUTED_PIECES`] (rotations of the canonical
///    order above it), each piece drawing the full candidate stream - its own
///    vacated pose, the single-piece separating projection's trajectory, the
///    aimed displacement cloud, **the other ejected pieces' vacated poses**,
///    and the skyline stations. Order matters here in a way it does not for a
///    single ejection: the first piece placed fixes the occupancy the rest must
///    clear, so the same set can be infeasible in one order and feasible in
///    another. The first order in which every piece confirms under the clamp
///    wins.
/// 3. If no order succeeds, one round of pairwise **pose-swap seeding**: the
///    two pieces' vacated poses are exchanged in the anchor, so each one's
///    whole candidate cloud re-centres on the other's pocket. That is the
///    coordinated move - A into B's place and B into A's - that no translation
///    and no single-piece neighbourhood can express at any magnitude.
/// 4. Exact-validate the complete state against the real request and publish
///    only on success.
///
/// Like modes 27 and 28 it is deliberately pointed at states that do *not*
/// validate, so it measures its parent rather than gating on it.
///
/// # Mode 33
///
/// Mode 33 is this pipeline with the orientation-perturbation stream armed.
/// Every structural constant is unchanged - the component passes, the ejection
/// limit, the order enumeration, the pose-swap round, the finalist beam - and
/// the only difference is that each ejected piece's candidate stream carries
/// continuous-angle variants of its own vacated pose behind every pose mode 29
/// could reach. It shares mode 29's seed domain for the same reason mode 32
/// shares mode 28's.
pub(super) fn run_joint_replacement_repair(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    parent: &GeneralCoupledSeparatorArmDiagnostics,
    parent_source: Option<String>,
    orientation_perturbation: bool,
) -> GeneralPersistentVacancyDiagnostics {
    let mode = if orientation_perturbation { 33 } else { 29 };
    let mut diagnostics = GeneralPersistentVacancyDiagnostics {
        mode,
        seed_domain: JOINT_REPLACEMENT_SEED_DOMAIN,
        parent_source,
        ..GeneralPersistentVacancyDiagnostics::default()
    };
    let Some(bound_mm) = relaxed_settings.persistent_vacancy_target_depth_mm else {
        diagnostics.failure_reason = Some(format!(
            "persistent vacancy mode {mode} requires an explicit depth bound"
        ));
        return diagnostics;
    };
    if !bound_mm.is_finite() || bound_mm <= 0.0 {
        diagnostics.failure_reason = Some(format!(
            "persistent vacancy mode {mode} depth bound must be a positive finite value"
        ));
        return diagnostics;
    }
    diagnostics.target_depth_mm = bound_mm;
    if pieces.is_empty() {
        diagnostics.failure_reason =
            Some("persistent vacancy experiment requires at least one piece".to_owned());
        return diagnostics;
    }
    if parent.final_placements.len() != pieces.len() {
        diagnostics.failure_reason =
            Some("joint re-placement requires a complete parent layout".to_owned());
        return diagnostics;
    }

    let parent_placements = diagnostic_fast_placements(&parent.final_placements);
    diagnostics.parent_fingerprint = Some(coupled_fast_placement_fingerprint(&parent_placements));
    diagnostics.initial_state_fingerprint = diagnostics.parent_fingerprint.clone();
    diagnostics.parent_independent_depth_mm =
        coupled_independent_source_depth(pieces, &parent_placements, fast_settings).ok();
    diagnostics.attempted = true;

    let outcome = joint_replacement_repair(
        pieces,
        &parent_placements,
        fast_settings,
        bound_mm,
        orientation_perturbation,
    );
    diagnostics.work = outcome.diagnostics.work;
    diagnostics.cap_exhausted = outcome.diagnostics.cap_exhausted.clone();
    match &outcome.repaired {
        Some(repaired) => {
            diagnostics.complete_states = 1;
            diagnostics.direct_insertions = outcome.diagnostics.ejected_count;
            diagnostics.exact_valid = true;
            diagnostics.independent_depth_mm = outcome.diagnostics.depth_mm;
            diagnostics.final_placement_fingerprint =
                Some(coupled_fast_placement_fingerprint(repaired));
            diagnostics.final_placements = coupled_placement_diagnostics(repaired);
        }
        None => {
            diagnostics.publication_rejections = 1;
            diagnostics.failure_reason = Some(
                outcome
                    .diagnostics
                    .skipped_reason
                    .clone()
                    .or_else(|| outcome.diagnostics.rejection_reason.clone())
                    .unwrap_or_else(|| {
                        "joint re-placement produced no exact-valid state".to_owned()
                    }),
            );
        }
    }
    // The joint ejection set in slot order; the winning insertion order and its
    // hash live in the pass's own block, next to every order it tried.
    diagnostics.initial_inactive_piece_ids = outcome.diagnostics.ejected_piece_ids.clone();
    diagnostics.initial_inactive_order_hash = outcome.diagnostics.accepted_order_hash.clone();
    diagnostics.joint_replacement = Some(outcome.diagnostics);
    diagnostics
}

/// What one conflict-targeted re-placement attempt produced.
pub(crate) struct ReplacementRepairOutcome {
    pub diagnostics: GeneralReplacementRepairDiagnostics,
    /// The repaired layout, returned only after the authoritative validator
    /// accepted it against the real request.
    pub repaired: Option<Vec<GeneralFastPlacement>>,
}

/// Runs the conflict-targeted re-placement repair over `placements` under the
/// clamped bound `bound_mm`.
///
/// See [`run_replacement_repair`] for the mechanism. This is the callable pass,
/// used both by mode 28 standalone and by mode 26 as its second-tier repair.
/// Like the micro-legalizer, it never publishes on its own authority: a layout
/// comes back only after [`validate_and_measure_placements`] accepted it.
///
/// `orientation_perturbation` is mode 32's single degree of freedom: it arms
/// the continuous-angle candidate stream and changes nothing else. Every other
/// caller passes `false` and gets the stream it always got.
pub(crate) fn replacement_repair(
    pieces: &[GeneralFastPiece<'_>],
    placements: &[GeneralFastPlacement],
    fast_settings: GeneralFastSettings,
    bound_mm: f64,
    orientation_perturbation: bool,
) -> ReplacementRepairOutcome {
    let mut diagnostics = GeneralReplacementRepairDiagnostics {
        bound_mm,
        ..GeneralReplacementRepairDiagnostics::default()
    };
    let refuse = |diagnostics: GeneralReplacementRepairDiagnostics| ReplacementRepairOutcome {
        diagnostics,
        repaired: None,
    };
    if placements.len() != pieces.len() {
        diagnostics.skipped_reason =
            Some("conflict-targeted re-placement requires a complete layout".to_owned());
        return refuse(diagnostics);
    }
    if !bound_mm.is_finite() || bound_mm <= 0.0 {
        diagnostics.skipped_reason =
            Some("conflict-targeted re-placement requires a positive finite bound".to_owned());
        return refuse(diagnostics);
    }
    for piece in pieces {
        if piece.polygon.vertex_count() > MAX_SOURCE_FEATURES {
            diagnostics.skipped_reason = Some(format!(
                "piece {} exceeds the {MAX_SOURCE_FEATURES}-feature experiment cap",
                piece.id
            ));
            return refuse(diagnostics);
        }
    }

    let violations = match survey_layout_violations(pieces, placements, fast_settings) {
        Ok(violations) => violations,
        Err(error) => {
            diagnostics.skipped_reason = Some(format!("violation survey: {error}"));
            return refuse(diagnostics);
        }
    };
    diagnostics.violating_pairs = violations.pairs.len();
    diagnostics.boundary_pieces = violations.boundary_pieces.len();
    diagnostics.material_pairs = violations.material_pairs;
    diagnostics.collision_pairs = violations.collision_pairs;
    diagnostics.max_material_deficit_mm = violations.max_material_deficit_mm;
    diagnostics.max_envelope_push_mm = violations.max_envelope_push_mm;
    diagnostics.max_boundary_deficit_mm = violations.max_boundary_deficit_mm;
    diagnostics.component_count = violations.components.len();
    diagnostics.largest_component_pieces = violations.largest_component_pieces();
    let component_limit = micro_legalization_component_limit(pieces.len());
    let ejection_limit = replacement_ejection_limit(pieces.len());
    diagnostics.component_limit = component_limit;
    diagnostics.ejection_limit = ejection_limit;
    if violations.pairs.is_empty() {
        // A pure boundary residue is the micro-legalizer's job, and a state
        // with no residue at all does not need this pass. Either way there is
        // no conflict to target.
        diagnostics.skipped_reason =
            Some("no violating pair to target with a re-placement".to_owned());
        return refuse(diagnostics);
    }
    if diagnostics.largest_component_pieces > component_limit {
        diagnostics.skipped_reason = Some(format!(
            "violation component spans {} pieces, above the local-repair limit of {component_limit}",
            diagnostics.largest_component_pieces
        ));
        return refuse(diagnostics);
    }

    // The ejection set: for each violating pair, the endpoint whose removal
    // clears more violation mass. `pieceId` breaks ties, so the choice is a
    // function of the layout and the request alone.
    let incident_mass = violations.incident_mass(placements.len());
    let mut ejected = Vec::new();
    for pair in &violations.pairs {
        let (first, second) = (pair.first, pair.second);
        let chosen = match grid_key(incident_mass[first]).cmp(&grid_key(incident_mass[second])) {
            Ordering::Greater => first,
            Ordering::Less => second,
            Ordering::Equal => {
                if placements[first].piece_id <= placements[second].piece_id {
                    first
                } else {
                    second
                }
            }
        };
        if !ejected.contains(&chosen) {
            ejected.push(chosen);
        }
    }
    ejected.sort_unstable();
    diagnostics.ejected_count = ejected.len();
    if ejected.len() > ejection_limit {
        diagnostics.skipped_reason = Some(format!(
            "ejection set of {} pieces exceeds the local-repair limit of {ejection_limit}",
            ejected.len()
        ));
        return refuse(diagnostics);
    }
    diagnostics.ejected_piece_ids = ejected
        .iter()
        .map(|slot| placements[*slot].piece_id.clone())
        .collect();
    diagnostics.ejected_mass_mm = ejected.iter().map(|slot| incident_mass[*slot]).collect();

    // The remainder must now be free of violating pairs by construction. A
    // boundary residue may survive, and that *is* a projection problem, so the
    // micro-legalizer gets the sub-layout.
    let mut kept = Vec::with_capacity(placements.len() - ejected.len());
    for (slot, placement) in placements.iter().enumerate() {
        if !ejected.contains(&slot) {
            kept.push(placement.clone());
        }
    }
    let kept_violations = match survey_layout_violations(pieces, &kept, fast_settings) {
        Ok(violations) => violations,
        Err(error) => {
            diagnostics.skipped_reason = Some(format!("kept sub-layout survey: {error}"));
            return refuse(diagnostics);
        }
    };
    diagnostics.kept_violating_pairs = kept_violations.pairs.len();
    diagnostics.kept_boundary_pieces = kept_violations.boundary_pieces.len();
    if !kept_violations.pairs.is_empty() {
        // Cannot happen while the ejection set covers every violating pair,
        // but a silent claim is worse than a reported refusal.
        diagnostics.skipped_reason = Some(format!(
            "kept sub-layout still carries {} violating pairs after ejection",
            kept_violations.pairs.len()
        ));
        return refuse(diagnostics);
    }
    let mut base = placements.to_vec();
    if !kept_violations.boundary_pieces.is_empty() {
        // The micro-legalizer only returns a layout the authoritative
        // validator has already accepted, so a `Some` here is trustworthy.
        let (micro, repaired) = micro_legalize(pieces, &kept, fast_settings);
        diagnostics.kept_micro_legalization = Some(micro);
        match repaired {
            Some(repaired) => {
                let by_id = repaired
                    .iter()
                    .map(|placement| (placement.piece_id.as_str(), placement))
                    .collect::<BTreeMap<_, _>>();
                for placement in &mut base {
                    if let Some(moved) = by_id.get(placement.piece_id.as_str()) {
                        placement.translate_short_axis = moved.translate_short_axis;
                        placement.translate_long_axis = moved.translate_long_axis;
                    }
                }
            }
            None => {
                diagnostics.rejection_reason = Some(
                    "the kept sub-layout's boundary residue could not be micro-legalized"
                        .to_owned(),
                );
                return refuse(diagnostics);
            }
        }
    }

    diagnostics.attempted = true;
    let bound_settings = GeneralFastSettings {
        sheet_long_axis_mm: bound_mm,
        ..fast_settings
    };
    let anchor = match relaxed_state_from_fast_placements(pieces, &base, bound_mm) {
        Ok(anchor) => anchor,
        Err(error) => {
            diagnostics.rejection_reason = Some(error);
            return refuse(diagnostics);
        }
    };
    // Placement slots and piece indices need not agree, so the ejection set is
    // translated into piece indices before it reaches the insertion path.
    let by_id = pieces
        .iter()
        .enumerate()
        .map(|(index, piece)| (piece.id, index))
        .collect::<BTreeMap<_, _>>();
    let ejected_indices: Vec<usize> = match ejected
        .iter()
        .map(|slot| {
            by_id
                .get(placements[*slot].piece_id.as_str())
                .copied()
                .ok_or_else(|| format!("unknown piece {}", placements[*slot].piece_id))
        })
        .collect::<Result<Vec<usize>, String>>()
    {
        Ok(indices) => indices,
        Err(error) => {
            diagnostics.rejection_reason = Some(error);
            return refuse(diagnostics);
        }
    };

    // Each ejected piece carries the pose it was in conflict at, and the
    // conflict is millimetre-scale by construction, so the residue scale the
    // anchor-local cloud is sized from is the ejection set's own violation
    // mass: how far this conflict has to travel to clear. The directions come
    // from the same survey - each ejected piece's own escape witness against
    // every partner it violates, oriented away from that partner.
    let residue_scale_mm = diagnostics
        .ejected_mass_mm
        .iter()
        .copied()
        .fold(0.0f64, f64::max);
    let mut anchor_local = AnchorLocalSeeding::with_residue_scale(Some(residue_scale_mm));
    for (slot, index) in ejected.iter().copied().zip(ejected_indices.iter().copied()) {
        let mut directions = Vec::new();
        for pair in &violations.pairs {
            // `separation_direction` is the direction `first` travels; the
            // other endpoint comes apart along its negation.
            if pair.first == slot {
                directions.push(pair.separation_direction);
            } else if pair.second == slot {
                directions.push((-pair.separation_direction.0, -pair.separation_direction.1));
            }
        }
        if !directions.is_empty() {
            anchor_local.separating_directions.insert(index, directions);
        }
        // The projection is measured against the occupancy this piece is
        // actually re-placed into: the kept sub-layout it has to clear - after
        // whatever the micro-legalizer did to it - with the piece itself
        // appended so it is the single movable slot. The other ejected pieces
        // are gone, so their conflicting poses do not constrain it.
        let mut sub = base
            .iter()
            .enumerate()
            .filter(|(other, _)| !ejected.contains(other))
            .map(|(_, placement)| placement.clone())
            .collect::<Vec<_>>();
        sub.push(base[slot].clone());
        let projection_slot = sub.len() - 1;
        match separating_translation(
            pieces,
            &sub,
            bound_settings,
            projection_slot,
            ANCHOR_LOCAL_PROJECTION_ITERATES,
        ) {
            Ok((trajectory, converged)) => {
                if converged {
                    diagnostics.projections_converged =
                        diagnostics.projections_converged.saturating_add(1);
                }
                anchor_local
                    .projected_displacements
                    .insert(index, trajectory);
            }
            Err(_) => {
                // A projection that cannot be measured is a missing seed, not
                // a failed repair: the cloud and the stations still run.
                diagnostics.projection_failures = diagnostics.projection_failures.saturating_add(1);
            }
        }
    }
    diagnostics.projected_displacements_mm = ejected_indices
        .iter()
        .map(|index| {
            anchor_local
                .projected_displacements(*index)
                .last()
                .map_or(0.0, |(x, y)| x.hypot(*y))
        })
        .collect();
    let mut work = RunWork::new(pieces.len());
    // Mode 32's degree of freedom, armed once per pass: one charged collision
    // build per orientation variant per ejected piece, taken here rather than
    // inside the insertion primitive so a caller that runs the primitive
    // repeatedly pays for the geometry once.
    if orientation_perturbation {
        if let Err(reason) = arm_orientation_perturbation(
            pieces,
            &anchor,
            bound_settings,
            &ejected_indices,
            &mut anchor_local,
            &mut work,
        ) {
            diagnostics.work = work.diagnostics;
            diagnostics.cap_exhausted = reason.strip_prefix("cap: ").map(str::to_owned);
            diagnostics.rejection_reason = Some(reason);
            return refuse(diagnostics);
        }
    }
    let pass = replace_ejected_under_bound(
        pieces,
        fast_settings,
        bound_settings,
        bound_mm,
        &anchor,
        &ejected_indices,
        None,
        None,
        REPLACEMENT_REPAIR_SEED_DOMAIN,
        &anchor_local,
        &mut work,
    );
    diagnostics.work = work.diagnostics;
    let pass = match pass {
        Ok(pass) => pass,
        Err(reason) => {
            diagnostics.cap_exhausted = reason.strip_prefix("cap: ").map(str::to_owned);
            diagnostics.rejection_reason = Some(reason);
            return refuse(diagnostics);
        }
    };
    // `ejected_piece_ids` stays in slot order and stays aligned with
    // `ejected_mass_mm`; the re-placement order is `pieces`, anchored by the
    // order hash.
    diagnostics.ejected_order_hash = Some(pass.order_hash.clone());
    diagnostics.pieces = pass
        .pieces
        .iter()
        .map(|row| GeneralReplacementRepairPieceRow {
            piece_id: row.piece_id.clone(),
            candidates_considered: row.candidates_considered,
            bound_rejections: row.bound_rejections,
            replaced: row.placed_extent_mm.is_some(),
            placed_extent_mm: row.placed_extent_mm,
            anchor_local_candidates: row.anchor_local_candidates,
            anchor_local_finalists: row.anchor_local_finalists,
            orientation: row.orientation.clone(),
        })
        .collect();
    diagnostics.replaced_count = diagnostics.pieces.iter().filter(|row| row.replaced).count();
    if let Some(failed) = pass.failed_piece_id() {
        diagnostics.failed_piece_id = Some(failed.to_owned());
        diagnostics.rejection_reason = Some(format!(
            "no exact-valid pose for piece {failed} within the {bound_mm} mm bound"
        ));
        return refuse(diagnostics);
    }

    let repaired = fast_placements(&pass.state, pieces, false);
    match validate_and_measure_placements(pieces, &repaired, fast_settings) {
        Ok(_) => {}
        Err(error) => {
            diagnostics.rejection_reason = Some(error.to_string());
            return refuse(diagnostics);
        }
    }
    match coupled_independent_source_depth(pieces, &repaired, fast_settings) {
        Ok(depth_mm) => {
            diagnostics.exact_valid = true;
            diagnostics.depth_mm = Some(depth_mm);
            ReplacementRepairOutcome {
                diagnostics,
                repaired: Some(repaired),
            }
        }
        Err(error) => {
            diagnostics.rejection_reason = Some(format!("repaired state depth: {error}"));
            refuse(diagnostics)
        }
    }
}

/// The shared re-placement order: displaced-first by descending piece area,
/// with `pieceId` breaking ties. It is a function of the request alone, which
/// is what makes it usable both as [`replace_ejected_under_bound`]'s default
/// and as the base the joint pass permutes.
fn bounded_replacement_order(pieces: &[GeneralFastPiece<'_>], ejected: &[usize]) -> Vec<usize> {
    let mut order = ejected.to_vec();
    order.sort_by(|first, second| {
        doubled_area_grid2(pieces[*second].polygon.area_mm2())
            .cmp(&doubled_area_grid2(pieces[*first].polygon.area_mm2()))
            .then_with(|| pieces[*first].id.cmp(pieces[*second].id))
    });
    order
}

/// Advances `items` to the next lexicographically greater arrangement,
/// returning `false` when it is already the last one.
///
/// Spelled out rather than pulled from a crate so the enumeration order is part
/// of this file's contract: the identity arrangement comes first, so a joint
/// pass's *first* attempt is always the canonical single-piece order, and every
/// later attempt is a deterministic successor of it.
fn next_lexicographic_permutation(items: &mut [usize]) -> bool {
    if items.len() < 2 {
        return false;
    }
    let mut pivot = items.len() - 1;
    while pivot > 0 && items[pivot - 1] >= items[pivot] {
        pivot -= 1;
    }
    if pivot == 0 {
        return false;
    }
    let mut successor = items.len() - 1;
    while items[successor] <= items[pivot - 1] {
        successor -= 1;
    }
    items.swap(pivot - 1, successor);
    items[pivot..].reverse();
    true
}

/// The insertion orders one joint attempt enumerates over `base`, which is
/// already in the primitive's canonical order, plus whether the enumeration is
/// exhaustive.
///
/// Up to [`JOINT_REPLACEMENT_MAX_PERMUTED_PIECES`] the answer is every
/// permutation, in lexicographic order of `base`'s own positions - at four
/// pieces that is 24 orders, exactly [`JOINT_REPLACEMENT_ORDER_CAP`]. A larger
/// ejection set (which only an instance large enough to admit one can produce)
/// falls back to the rotations of the canonical order, so every piece gets a
/// turn at going in first while the cost stays inside the same ceiling.
fn joint_replacement_orders(base: &[usize]) -> (Vec<Vec<usize>>, bool) {
    let count = base.len();
    if count == 0 {
        return (Vec::new(), true);
    }
    if count <= JOINT_REPLACEMENT_MAX_PERMUTED_PIECES {
        let mut positions = (0..count).collect::<Vec<_>>();
        let mut orders = Vec::new();
        loop {
            orders.push(positions.iter().map(|position| base[*position]).collect());
            if orders.len() >= JOINT_REPLACEMENT_ORDER_CAP
                || !next_lexicographic_permutation(&mut positions)
            {
                break;
            }
        }
        return (orders, true);
    }
    let orders = (0..count.min(JOINT_REPLACEMENT_ORDER_CAP))
        .map(|shift| {
            (0..count)
                .map(|position| base[(position + shift) % count])
                .collect()
        })
        .collect();
    (orders, false)
}

/// What one joint multi-piece re-placement attempt produced.
pub(crate) struct JointReplacementOutcome {
    pub diagnostics: GeneralJointReplacementDiagnostics,
    /// The repaired layout, returned only after the authoritative validator
    /// accepted it against the real request.
    pub repaired: Option<Vec<GeneralFastPlacement>>,
}

/// Runs one insertion order of a joint re-placement and measures what came
/// back, always against the real request rather than the clamped one.
#[allow(clippy::too_many_arguments)]
fn joint_replacement_attempt(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    bound_settings: GeneralFastSettings,
    bound_mm: f64,
    anchor: &RelaxedState,
    ejected_indices: &[usize],
    order: &[usize],
    anchor_local: &AnchorLocalSeeding,
    ordinal: usize,
    component_pass: usize,
    swap_pair: Option<Vec<String>>,
    finalist_ranks: Option<&[usize]>,
    work: &mut RunWork,
) -> (
    GeneralJointReplacementOrderRow,
    Option<Vec<GeneralFastPlacement>>,
) {
    let mut row = GeneralJointReplacementOrderRow {
        ordinal,
        component_pass,
        order_piece_ids: order
            .iter()
            .map(|index| pieces[*index].id.to_owned())
            .collect(),
        swap_pair,
        finalist_ranks: finalist_ranks.map(<[usize]>::to_vec),
        ..GeneralJointReplacementOrderRow::default()
    };
    let pass = match replace_ejected_under_bound(
        pieces,
        fast_settings,
        bound_settings,
        bound_mm,
        anchor,
        ejected_indices,
        Some(order),
        finalist_ranks,
        JOINT_REPLACEMENT_SEED_DOMAIN,
        anchor_local,
        work,
    ) {
        Ok(pass) => pass,
        Err(reason) => {
            row.rejection_reason = Some(reason);
            return (row, None);
        }
    };
    row.order_hash = Some(pass.order_hash.clone());
    row.pieces = pass
        .pieces
        .iter()
        .map(|piece| GeneralReplacementRepairPieceRow {
            piece_id: piece.piece_id.clone(),
            candidates_considered: piece.candidates_considered,
            bound_rejections: piece.bound_rejections,
            replaced: piece.placed_extent_mm.is_some(),
            placed_extent_mm: piece.placed_extent_mm,
            anchor_local_candidates: piece.anchor_local_candidates,
            anchor_local_finalists: piece.anchor_local_finalists,
            orientation: piece.orientation.clone(),
        })
        .collect();
    row.replaced_count = row.pieces.iter().filter(|piece| piece.replaced).count();
    if let Some(failed) = pass.failed_piece_id() {
        row.failed_piece_id = Some(failed.to_owned());
        row.rejection_reason = Some(format!(
            "no exact-valid pose for piece {failed} within the {bound_mm} mm bound"
        ));
        return (row, None);
    }

    // The complete re-placed layout comes back whether or not the *whole*
    // request validates on it: a per-component pass legalizes one conflict
    // cluster at a time, so a state that still carries another cluster's
    // violation is real progress rather than a failure. `exactValid` on the row
    // stays the authoritative whole-request verdict, and it is the only thing a
    // single-cluster instance ever accepts on.
    let repaired = fast_placements(&pass.state, pieces, false);
    match validate_and_measure_placements(pieces, &repaired, fast_settings) {
        Ok(_) => match coupled_independent_source_depth(pieces, &repaired, fast_settings) {
            Ok(depth_mm) => {
                row.exact_valid = true;
                row.depth_mm = Some(depth_mm);
            }
            Err(error) => row.rejection_reason = Some(format!("repaired state depth: {error}")),
        },
        Err(error) => row.rejection_reason = Some(error.to_string()),
    }
    (row, Some(repaired))
}

/// The finalist-rank combinations the beam enumerates for a `pieces`-piece
/// component, in the order it tries them.
///
/// Odometer order over `CONSTRUCTION_FINALISTS_PER_SLOT` ranks per piece with
/// the *last* piece varying fastest, so the first combination is all-zeros -
/// the greedy shallowest-first commit the primitive has always made - and every
/// later one is a deterministic successor of it. Empty for a component the beam
/// does not run on, which is how the caller asks for the greedy pass alone.
fn finalist_rank_combinations(pieces: usize) -> Vec<Vec<usize>> {
    if pieces == 0 || pieces > JOINT_REPLACEMENT_BEAM_MAX_PIECES {
        return Vec::new();
    }
    let mut combinations = Vec::new();
    let mut ranks = vec![0usize; pieces];
    loop {
        combinations.push(ranks.clone());
        if combinations.len() >= JOINT_REPLACEMENT_BEAM_COMBINATIONS {
            break;
        }
        let mut position = pieces;
        loop {
            if position == 0 {
                return combinations;
            }
            position -= 1;
            ranks[position] += 1;
            if ranks[position] < CONSTRUCTION_FINALISTS_PER_SLOT {
                break;
            }
            ranks[position] = 0;
        }
    }
    combinations
}

/// The next violation component to repair: the pair-bearing component carrying
/// the most violation mass, with the lowest slot index breaking ties, skipping
/// any component whose exact slot set an earlier pass already failed on.
///
/// Boundary-only components are a projection problem rather than a
/// re-placement one and are never selected, which is exactly what
/// [`LayoutViolations::pair_components`] leaves out.
fn next_violation_component(
    violations: &LayoutViolations,
    refused: &BTreeSet<Vec<usize>>,
) -> Option<Vec<usize>> {
    let mut best: Option<((i64, usize), Vec<usize>)> = None;
    for slots in violations.pair_components() {
        if slots.is_empty() || refused.contains(&slots) {
            continue;
        }
        let mass = violations
            .pairs
            .iter()
            .filter(|pair| slots.contains(&pair.first) || slots.contains(&pair.second))
            .fold(0.0f64, |total, pair| total + pair.mass_mm);
        // Descending mass on the placement grid, ascending first slot: an
        // integer key, so the choice cannot depend on float comparison order.
        let key = (-grid_key(mass), slots[0]);
        if best.as_ref().is_none_or(|(current, _)| key < *current) {
            best = Some((key, slots));
        }
    }
    best.map(|(_, slots)| slots)
}

/// Whether a re-placed layout cleared the component it was aimed at without
/// making the rest of the residue worse.
///
/// This is the acceptance rule for a component pass that is *not* the last one:
/// no violating pair may remain incident to any piece the pass ejected, and the
/// boundary residue may not have grown. A pass that clears the whole layout is
/// never accepted on this rule - the authoritative validator is, which is what
/// keeps a single-cluster instance's behaviour exactly what it always was.
fn component_pass_cleared(
    before: &LayoutViolations,
    after: &LayoutViolations,
    ejected: &[usize],
) -> bool {
    if after.boundary_pieces.len() > before.boundary_pieces.len() {
        return false;
    }
    !after
        .pairs
        .iter()
        .any(|pair| ejected.contains(&pair.first) || ejected.contains(&pair.second))
}

/// Runs the joint multi-piece re-placement repair over `placements` under the
/// clamped bound `bound_mm`.
///
/// See [`run_joint_replacement_repair`] for the mechanism. This is the callable
/// pass, used both by mode 29 standalone and by mode 26 as its third-tier
/// repair. Like the other two tiers it never publishes on its own authority: a
/// layout comes back only after [`validate_and_measure_placements`] accepted it.
///
/// `orientation_perturbation` is mode 33's single degree of freedom, exactly as
/// it is mode 32's in the second tier: it arms the continuous-angle candidate
/// stream per component and changes nothing else.
pub(crate) fn joint_replacement_repair(
    pieces: &[GeneralFastPiece<'_>],
    placements: &[GeneralFastPlacement],
    fast_settings: GeneralFastSettings,
    bound_mm: f64,
    orientation_perturbation: bool,
) -> JointReplacementOutcome {
    let mut diagnostics = GeneralJointReplacementDiagnostics {
        bound_mm,
        ..GeneralJointReplacementDiagnostics::default()
    };
    let refuse = |diagnostics: GeneralJointReplacementDiagnostics| JointReplacementOutcome {
        diagnostics,
        repaired: None,
    };
    if placements.len() != pieces.len() {
        diagnostics.skipped_reason =
            Some("joint re-placement requires a complete layout".to_owned());
        return refuse(diagnostics);
    }
    if !bound_mm.is_finite() || bound_mm <= 0.0 {
        diagnostics.skipped_reason =
            Some("joint re-placement requires a positive finite bound".to_owned());
        return refuse(diagnostics);
    }
    for piece in pieces {
        if piece.polygon.vertex_count() > MAX_SOURCE_FEATURES {
            diagnostics.skipped_reason = Some(format!(
                "piece {} exceeds the {MAX_SOURCE_FEATURES}-feature experiment cap",
                piece.id
            ));
            return refuse(diagnostics);
        }
    }

    let violations = match survey_layout_violations(pieces, placements, fast_settings) {
        Ok(violations) => violations,
        Err(error) => {
            diagnostics.skipped_reason = Some(format!("violation survey: {error}"));
            return refuse(diagnostics);
        }
    };
    diagnostics.violating_pairs = violations.pairs.len();
    diagnostics.boundary_pieces = violations.boundary_pieces.len();
    diagnostics.material_pairs = violations.material_pairs;
    diagnostics.collision_pairs = violations.collision_pairs;
    diagnostics.max_material_deficit_mm = violations.max_material_deficit_mm;
    diagnostics.max_envelope_push_mm = violations.max_envelope_push_mm;
    diagnostics.max_boundary_deficit_mm = violations.max_boundary_deficit_mm;
    diagnostics.component_count = violations.components.len();
    diagnostics.largest_component_pieces = violations.largest_component_pieces();
    let component_limit = micro_legalization_component_limit(pieces.len());
    let ejection_limit = replacement_ejection_limit(pieces.len());
    diagnostics.component_limit = component_limit;
    diagnostics.ejection_limit = ejection_limit;
    if violations.pairs.is_empty() {
        diagnostics.skipped_reason =
            Some("no violating pair to target with a joint re-placement".to_owned());
        return refuse(diagnostics);
    }
    if diagnostics.largest_component_pieces > component_limit {
        diagnostics.skipped_reason = Some(format!(
            "violation component spans {} pieces, above the local-repair limit of {component_limit}",
            diagnostics.largest_component_pieces
        ));
        return refuse(diagnostics);
    }

    // The pass repairs the violation graph one connected component at a time,
    // re-surveying the whole layout between components. Independent conflicts
    // are independent repairs: pooling every pair-bearing component into one
    // ejection set made four separate two-piece conflicts refuse on an ejection
    // cap that none of them individually trips, which is what the measured
    // negative on this tier was largely made of.
    let bound_settings = GeneralFastSettings {
        sheet_long_axis_mm: bound_mm,
        ..fast_settings
    };
    let mut work = RunWork::new(pieces.len());
    let mut budget = JointReplacementBudget::for_piece_count(pieces.len());
    let mut working = placements.to_vec();
    let mut refused = BTreeSet::<Vec<usize>>::new();
    let mut ordinal = 0usize;
    let mut accepted_depth_mm = None::<f64>;
    let mut repaired_any = false;
    let mut orders_exhaustive = true;

    for component_pass in 0..JOINT_REPLACEMENT_COMPONENT_PASSES {
        let current = match survey_layout_violations(pieces, &working, fast_settings) {
            Ok(current) => current,
            Err(error) => {
                diagnostics.rejection_reason = Some(format!("working layout survey: {error}"));
                break;
            }
        };
        if current.pairs.is_empty() {
            break;
        }
        let Some(component) = next_violation_component(&current, &refused) else {
            break;
        };
        // Measured on *this* pass's survey, not the input one: a later
        // component's residue scale is what its own anchor-local cloud is
        // sized from, and on the first pass the two surveys are the same.
        let incident_mass = current.incident_mass(working.len());
        let mut row = GeneralJointReplacementComponentRow {
            pass: component_pass,
            piece_ids: component
                .iter()
                .map(|slot| working[*slot].piece_id.clone())
                .collect(),
            incident_mass_mm: component
                .iter()
                .map(|slot| incident_mass.get(*slot).copied().unwrap_or_default())
                .collect(),
            violating_pairs_before: current.pairs.len(),
            boundary_pieces_before: current.boundary_pieces.len(),
            ..GeneralJointReplacementComponentRow::default()
        };
        diagnostics.component_passes_run = diagnostics.component_passes_run.saturating_add(1);
        if component.len() < 2 {
            // A violating pair always contributes two slots, so this cannot
            // happen from a pair-bearing component; refusing it explicitly
            // keeps the pass from claiming to be joint while re-placing one
            // piece.
            row.skipped_reason = Some(
                "joint re-placement requires at least two pieces in the violating set".to_owned(),
            );
            orders_exhaustive = false;
            refused.insert(component);
            diagnostics.components_refused = diagnostics.components_refused.saturating_add(1);
            diagnostics.components.push(row);
            continue;
        }
        if component.len() > ejection_limit {
            row.skipped_reason = Some(format!(
                "joint ejection set of {} pieces exceeds the local-repair limit of {ejection_limit}",
                component.len()
            ));
            orders_exhaustive = false;
            refused.insert(component);
            diagnostics.components_refused = diagnostics.components_refused.saturating_add(1);
            diagnostics.components.push(row);
            continue;
        }
        for id in &row.piece_ids {
            diagnostics.ejected_piece_ids.push(id.clone());
        }
        for mass in &row.incident_mass_mm {
            diagnostics.ejected_mass_mm.push(*mass);
        }
        diagnostics.ejected_count = diagnostics.ejected_count.saturating_add(component.len());

        let outcome = repair_violation_component(
            pieces,
            fast_settings,
            bound_settings,
            bound_mm,
            &working,
            &current,
            &component,
            component_pass,
            &mut ordinal,
            &mut budget,
            orientation_perturbation,
            &mut work,
            &mut diagnostics,
            &mut row,
        );
        orders_exhaustive &= outcome.orders_exhaustive;
        match outcome.repaired {
            Some(repaired) => {
                working = repaired;
                accepted_depth_mm = outcome.depth_mm;
                repaired_any = true;
                row.repaired = true;
                diagnostics.components_repaired = diagnostics.components_repaired.saturating_add(1);
            }
            None => {
                refused.insert(component);
                diagnostics.components_refused = diagnostics.components_refused.saturating_add(1);
            }
        }
        let after = survey_layout_violations(pieces, &working, fast_settings).ok();
        row.violating_pairs_after = after
            .as_ref()
            .map_or(row.violating_pairs_before, |after| after.pairs.len());
        row.boundary_pieces_after = after.as_ref().map_or(row.boundary_pieces_before, |after| {
            after.boundary_pieces.len()
        });
        let capped = outcome.cap_exhausted.clone();
        diagnostics.components.push(row);
        if let Some(capped) = capped {
            diagnostics.cap_exhausted = Some(capped);
            break;
        }
    }
    diagnostics.work = work.diagnostics;
    // True only when *every* component pass enumerated all of its own set's
    // permutations, so a single-cluster instance reports exactly what it did.
    diagnostics.orders_exhaustive = orders_exhaustive && diagnostics.component_passes_run > 0;

    // Publication is the whole-request validator's call, exactly as before: a
    // component pass may accept partial progress, but only a layout the
    // authoritative validator accepts ever leaves this function.
    if repaired_any {
        if let Some(depth_mm) = accepted_depth_mm {
            diagnostics.exact_valid = true;
            diagnostics.depth_mm = Some(depth_mm);
            if let Some(winner) = diagnostics
                .orders
                .iter()
                .rev()
                .find(|row| row.exact_valid)
                .cloned()
            {
                diagnostics.accepted_order = Some(winner.ordinal);
                diagnostics.accepted_order_hash = winner.order_hash.clone();
                diagnostics.accepted_by_swap = winner.swap_pair.is_some();
                diagnostics.accepted_finalist_ranks = winner.finalist_ranks.clone();
            }
            return JointReplacementOutcome {
                diagnostics,
                repaired: Some(working),
            };
        }
    }
    if diagnostics.rejection_reason.is_none() {
        diagnostics.rejection_reason = Some(format!(
            "no insertion order re-placed the {} violation components inside the {bound_mm} mm bound",
            diagnostics.component_passes_run
        ));
    }
    refuse(diagnostics)
}

/// What one component pass produced.
struct ComponentRepairOutcome {
    /// The whole layout with this component re-placed, when the pass accepted
    /// one. `None` means the component was not repaired and the caller keeps
    /// the layout it had.
    repaired: Option<Vec<GeneralFastPlacement>>,
    /// The measured depth of that layout when the *whole request* validated on
    /// it. `None` on a partial repair, which is not a publication candidate.
    depth_mm: Option<f64>,
    /// Whether this component's insertion-order plan was every permutation of
    /// its ejection set rather than the bounded rotation family.
    orders_exhaustive: bool,
    cap_exhausted: Option<String>,
}

/// Repairs one connected violation component of `working` by ejecting all of
/// it and re-placing it under the clamped bound.
///
/// The enumeration is the tier's original one - every insertion order, then the
/// pose-swap round - followed by the finalist-combination beam. The beam runs
/// *last* deliberately: every state the tier used to publish it still publishes
/// by exactly the route it used to, and the beam only ever adds states nothing
/// before it could reach.
///
/// Acceptance is the whole-request validator whenever it passes, which is what
/// a single-component instance is decided on and is bit-identical to the
/// pooled pass. Only when other conflict clusters are still outstanding may a
/// pass instead accept partial progress - this component's pairs cleared, the
/// boundary residue no worse - because such a layout cannot validate as a whole
/// yet by construction.
#[allow(clippy::too_many_arguments)]
fn repair_violation_component(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    bound_settings: GeneralFastSettings,
    bound_mm: f64,
    working: &[GeneralFastPlacement],
    violations: &LayoutViolations,
    ejected: &[usize],
    component_pass: usize,
    ordinal: &mut usize,
    budget: &mut JointReplacementBudget,
    orientation_perturbation: bool,
    work: &mut RunWork,
    diagnostics: &mut GeneralJointReplacementDiagnostics,
    row: &mut GeneralJointReplacementComponentRow,
) -> ComponentRepairOutcome {
    let refuse = |cap_exhausted: Option<String>| ComponentRepairOutcome {
        repaired: None,
        depth_mm: None,
        orders_exhaustive: false,
        cap_exhausted,
    };
    // Every violating pair of *this* component has both endpoints in the
    // ejection set, so what survives among the kept pieces is the other
    // components' residue plus any boundary residue. A boundary residue is a
    // projection problem, so - exactly as in the second tier - the
    // micro-legalizer gets the sub-layout, but only once this is the last
    // cluster standing and the projection is not being asked to reason about a
    // conflict another pass still owns.
    let mut kept = Vec::with_capacity(working.len().saturating_sub(ejected.len()));
    for (slot, placement) in working.iter().enumerate() {
        if !ejected.contains(&slot) {
            kept.push(placement.clone());
        }
    }
    let kept_violations = match survey_layout_violations(pieces, &kept, fast_settings) {
        Ok(violations) => violations,
        Err(error) => {
            row.rejection_reason = Some(format!("kept sub-layout survey: {error}"));
            return refuse(None);
        }
    };
    row.kept_violating_pairs = kept_violations.pairs.len();
    row.kept_boundary_pieces = kept_violations.boundary_pieces.len();
    // The top-level kept-residue fields keep the pooled pass's meaning: they
    // describe the *first* component pass, which on a single-cluster instance
    // is the whole pass. Later passes are readable in their own rows.
    let first_pass = component_pass == 0;
    if first_pass {
        diagnostics.kept_violating_pairs = kept_violations.pairs.len();
        diagnostics.kept_boundary_pieces = kept_violations.boundary_pieces.len();
    }
    let other_clusters_remain = !kept_violations.pairs.is_empty();
    let mut base = working.to_vec();
    if kept_violations.pairs.is_empty() && !kept_violations.boundary_pieces.is_empty() {
        let (micro, repaired) = micro_legalize(pieces, &kept, fast_settings);
        row.kept_micro_legalization = Some(micro.clone());
        if first_pass {
            diagnostics.kept_micro_legalization = Some(micro);
        }
        match repaired {
            Some(repaired) => {
                let by_id = repaired
                    .iter()
                    .map(|placement| (placement.piece_id.as_str(), placement))
                    .collect::<BTreeMap<_, _>>();
                for placement in &mut base {
                    if let Some(moved) = by_id.get(placement.piece_id.as_str()) {
                        placement.translate_short_axis = moved.translate_short_axis;
                        placement.translate_long_axis = moved.translate_long_axis;
                    }
                }
            }
            None => {
                row.rejection_reason = Some(
                    "the kept sub-layout's boundary residue could not be micro-legalized"
                        .to_owned(),
                );
                return refuse(None);
            }
        }
    }

    diagnostics.attempted = true;
    let anchor = match relaxed_state_from_fast_placements(pieces, &base, bound_mm) {
        Ok(anchor) => anchor,
        Err(error) => {
            row.rejection_reason = Some(error);
            return refuse(None);
        }
    };
    let by_id = pieces
        .iter()
        .enumerate()
        .map(|(index, piece)| (piece.id, index))
        .collect::<BTreeMap<_, _>>();
    let ejected_indices: Vec<usize> = match ejected
        .iter()
        .map(|slot| {
            by_id
                .get(working[*slot].piece_id.as_str())
                .copied()
                .ok_or_else(|| format!("unknown piece {}", working[*slot].piece_id))
        })
        .collect::<Result<Vec<usize>, String>>()
    {
        Ok(indices) => indices,
        Err(error) => {
            row.rejection_reason = Some(error);
            return refuse(None);
        }
    };

    // Seeding is the second tier's, plus the peers. The residue scale, the
    // aimed separating directions and the single-piece projections are all
    // measured exactly as there; what is new is that each ejected piece also
    // gets the vacated translations of the *others* as candidate poses, which
    // is the only way a per-piece candidate stream can propose an exchange.
    let residue_scale_mm = row.incident_mass_mm.iter().copied().fold(0.0f64, f64::max);
    let mut anchor_local = AnchorLocalSeeding::with_residue_scale(Some(residue_scale_mm));
    for (slot, index) in ejected.iter().copied().zip(ejected_indices.iter().copied()) {
        let mut directions = Vec::new();
        for pair in &violations.pairs {
            if pair.first == slot {
                directions.push(pair.separation_direction);
            } else if pair.second == slot {
                directions.push((-pair.separation_direction.0, -pair.separation_direction.1));
            }
        }
        if !directions.is_empty() {
            anchor_local.separating_directions.insert(index, directions);
        }
        // Nearest peers first, on the placement grid, with the piece index
        // breaking exact ties: the pocket a piece is most likely to be able to
        // trade into is the one next door.
        let mut peers = ejected
            .iter()
            .copied()
            .zip(ejected_indices.iter().copied())
            .filter(|(other, _)| *other != slot)
            .map(|(other, other_index)| {
                let offset_x = base[other].translate_short_axis - base[slot].translate_short_axis;
                let offset_y = base[other].translate_long_axis - base[slot].translate_long_axis;
                let distance = grid_key(offset_x.hypot(offset_y));
                (
                    distance,
                    other_index,
                    (
                        base[other].translate_short_axis,
                        base[other].translate_long_axis,
                    ),
                )
            })
            .collect::<Vec<_>>();
        peers.sort_by_key(|(distance, other_index, _)| (*distance, *other_index));
        anchor_local.peer_poses.insert(
            index,
            peers
                .into_iter()
                .take(JOINT_REPLACEMENT_PEER_POSES)
                .map(|(_, _, pose)| pose)
                .collect(),
        );
        let mut sub = base
            .iter()
            .enumerate()
            .filter(|(other, _)| !ejected.contains(other))
            .map(|(_, placement)| placement.clone())
            .collect::<Vec<_>>();
        sub.push(base[slot].clone());
        let projection_slot = sub.len() - 1;
        match separating_translation(
            pieces,
            &sub,
            bound_settings,
            projection_slot,
            ANCHOR_LOCAL_PROJECTION_ITERATES,
        ) {
            Ok((trajectory, converged)) => {
                if converged {
                    diagnostics.projections_converged =
                        diagnostics.projections_converged.saturating_add(1);
                }
                anchor_local
                    .projected_displacements
                    .insert(index, trajectory);
            }
            Err(_) => {
                diagnostics.projection_failures = diagnostics.projection_failures.saturating_add(1);
            }
        }
    }
    // Aligned with `ejectedPieceIds`, which accumulates across component passes.
    diagnostics
        .projected_displacements_mm
        .extend(ejected_indices.iter().map(|index| {
            anchor_local
                .projected_displacements(*index)
                .last()
                .map_or(0.0, |(x, y)| x.hypot(*y))
        }));
    // Mode 33's degree of freedom, armed once per component: the orientation
    // variants are a function of the piece and its vacated pose alone, so they
    // are built here rather than inside the insertion primitive, which this
    // pass runs once per insertion order, once per swap and once per beam
    // combination.
    if orientation_perturbation {
        if let Err(reason) = arm_orientation_perturbation(
            pieces,
            &anchor,
            bound_settings,
            &ejected_indices,
            &mut anchor_local,
            work,
        ) {
            row.rejection_reason = Some(reason.clone());
            return refuse(reason.strip_prefix("cap: ").map(str::to_owned));
        }
    }

    let canonical = bounded_replacement_order(pieces, &ejected_indices);
    let (orders, exhaustive) = joint_replacement_orders(&canonical);
    let combinations = finalist_rank_combinations(canonical.len());
    row.orders_planned = orders.len();
    row.beam_combinations_planned = combinations.len().saturating_sub(1);
    diagnostics.orders_planned = diagnostics.orders_planned.saturating_add(orders.len());
    let swap_pairs_planned = canonical
        .len()
        .saturating_mul(canonical.len().saturating_sub(1))
        .saturating_div(2)
        .min(JOINT_REPLACEMENT_SWAP_ATTEMPT_CAP);
    diagnostics.swap_pairs_planned = diagnostics
        .swap_pairs_planned
        .saturating_add(swap_pairs_planned);

    // The attempt plan, in the order it is spent: every plain insertion order
    // at the greedy finalist commit, then the pose-swap round, then the
    // finalist-combination beam on the canonical order.
    enum Attempt<'a> {
        Order(&'a [usize]),
        Swap(usize, usize),
        Beam(&'a [usize]),
    }
    let mut plan = Vec::<Attempt<'_>>::new();
    for order in &orders {
        plan.push(Attempt::Order(order));
    }
    for _ in 0..JOINT_REPLACEMENT_SWAP_ROUNDS {
        let mut swaps = 0usize;
        for first_position in 0..canonical.len() {
            for second_position in (first_position + 1)..canonical.len() {
                if swaps >= JOINT_REPLACEMENT_SWAP_ATTEMPT_CAP {
                    break;
                }
                plan.push(Attempt::Swap(first_position, second_position));
                swaps += 1;
            }
        }
    }
    for ranks in combinations.iter().skip(1) {
        plan.push(Attempt::Beam(ranks));
    }

    let mut accepted = None::<(Vec<GeneralFastPlacement>, Option<f64>)>;
    let mut cap_exhausted = None::<String>;
    for attempt in &plan {
        if let Err(reason) = budget.charge(ejected_indices.len()) {
            cap_exhausted = Some(reason.to_owned());
            break;
        }
        let (attempt_anchor, order, swap_pair, ranks) = match attempt {
            Attempt::Order(order) => (anchor.clone(), *order, None, None),
            Attempt::Swap(first_position, second_position) => {
                // Exchanging two vacated poses in the anchor re-centres both
                // pieces' whole candidate clouds on each other's pocket, so the
                // exchange is proposed by the same confirmed-pose machinery as
                // everything else. Orientations stay with their own pieces: a
                // rotation is admissible per piece, so trading them could
                // propose a pose the request forbids.
                let first = canonical[*first_position];
                let second = canonical[*second_position];
                let mut swapped = anchor.clone();
                let held = (
                    swapped.placements[first].translate_x,
                    swapped.placements[first].translate_y,
                );
                swapped.placements[first].translate_x = swapped.placements[second].translate_x;
                swapped.placements[first].translate_y = swapped.placements[second].translate_y;
                swapped.placements[second].translate_x = held.0;
                swapped.placements[second].translate_y = held.1;
                (
                    swapped,
                    canonical.as_slice(),
                    Some(vec![
                        pieces[first].id.to_owned(),
                        pieces[second].id.to_owned(),
                    ]),
                    None,
                )
            }
            Attempt::Beam(ranks) => (anchor.clone(), canonical.as_slice(), None, Some(*ranks)),
        };
        match attempt {
            Attempt::Order(_) => {
                row.orders_tried = row.orders_tried.saturating_add(1);
                diagnostics.orders_tried = diagnostics.orders_tried.saturating_add(1);
            }
            Attempt::Swap(..) => {
                row.swap_attempts_tried = row.swap_attempts_tried.saturating_add(1);
                diagnostics.swap_attempts_tried = diagnostics.swap_attempts_tried.saturating_add(1);
                diagnostics.swap_rounds_run = diagnostics.swap_rounds_run.max(1);
            }
            Attempt::Beam(_) => {
                row.beam_combinations_tried = row.beam_combinations_tried.saturating_add(1);
                diagnostics.beam_combinations_tried =
                    diagnostics.beam_combinations_tried.saturating_add(1);
            }
        }
        let (attempt_row, produced) = joint_replacement_attempt(
            pieces,
            fast_settings,
            bound_settings,
            bound_mm,
            &attempt_anchor,
            &ejected_indices,
            order,
            &anchor_local,
            *ordinal,
            component_pass,
            swap_pair,
            ranks,
            work,
        );
        *ordinal += 1;
        let capped = attempt_row
            .rejection_reason
            .as_deref()
            .and_then(|reason| reason.strip_prefix("cap: "))
            .map(str::to_owned);
        let exact_valid = attempt_row.exact_valid;
        let depth_mm = attempt_row.depth_mm;
        let accept = |row: &mut GeneralJointReplacementComponentRow| {
            row.accepted_ordinal = Some(attempt_row.ordinal);
            row.accepted_order_hash = attempt_row.order_hash.clone();
            row.accepted_by_swap = attempt_row.swap_pair.is_some();
            row.accepted_finalist_ranks = attempt_row.finalist_ranks.clone();
        };
        if let Some(capped) = capped {
            diagnostics.orders.push(attempt_row);
            cap_exhausted = Some(capped);
            break;
        }
        let Some(candidate) = produced else {
            diagnostics.orders.push(attempt_row);
            continue;
        };
        if exact_valid {
            accept(row);
            diagnostics.orders.push(attempt_row);
            accepted = Some((candidate, depth_mm));
            break;
        }
        if !other_clusters_remain {
            diagnostics.orders.push(attempt_row);
            continue;
        }
        // Partial progress: this component's pairs are gone and the boundary
        // residue is no worse, but another cluster still keeps the layout from
        // validating. Measured on the same survey every tier is targeted at.
        let Ok(after) = survey_layout_violations(pieces, &candidate, fast_settings) else {
            diagnostics.orders.push(attempt_row);
            continue;
        };
        let cleared = component_pass_cleared(violations, &after, ejected);
        if cleared {
            accept(row);
        }
        diagnostics.orders.push(attempt_row);
        if cleared {
            accepted = Some((candidate, None));
            break;
        }
    }

    match accepted {
        Some((repaired, depth_mm)) => ComponentRepairOutcome {
            repaired: Some(repaired),
            depth_mm,
            orders_exhaustive: exhaustive,
            cap_exhausted,
        },
        None => {
            if row.rejection_reason.is_none() {
                row.rejection_reason = Some(format!(
                    "no insertion order re-placed the {}-piece violation set inside the {bound_mm} mm bound",
                    ejected_indices.len()
                ));
            }
            ComponentRepairOutcome {
                orders_exhaustive: exhaustive,
                ..refuse(cap_exhausted)
            }
        }
    }
}

/// The joint tier's own attempt and slot ledger.
///
/// Charged per re-placement attempt against the *same* product the single-set
/// pass was already funded for in
/// `bounded_reinsertion_fits_the_construction_budget`: at most
/// `JOINT_REPLACEMENT_ORDER_CAP` plain orders plus the pose-swap round, each
/// over an ejection set as large as the whole layout. The per-component loop
/// and the finalist beam therefore spend the tier's existing construction-slot
/// allowance differently rather than asking for a new one, and an instance too
/// small to fund the full plan stops on `capExhausted` instead of overrunning.
struct JointReplacementBudget {
    attempts_remaining: usize,
    slots_remaining: usize,
}

impl JointReplacementBudget {
    fn attempt_cap() -> usize {
        JOINT_REPLACEMENT_COMPONENT_PASSES.saturating_mul(
            JOINT_REPLACEMENT_ORDER_CAP
                .saturating_add(JOINT_REPLACEMENT_SWAP_ROUNDS * JOINT_REPLACEMENT_SWAP_ATTEMPT_CAP)
                .saturating_add(JOINT_REPLACEMENT_BEAM_COMBINATIONS),
        )
    }

    fn slot_cap(piece_count: usize) -> usize {
        JOINT_REPLACEMENT_ORDER_CAP
            .saturating_add(JOINT_REPLACEMENT_SWAP_ROUNDS * JOINT_REPLACEMENT_SWAP_ATTEMPT_CAP)
            .saturating_mul(piece_count)
    }

    fn for_piece_count(piece_count: usize) -> Self {
        Self {
            attempts_remaining: Self::attempt_cap(),
            slots_remaining: Self::slot_cap(piece_count),
        }
    }

    /// Charges one re-placement attempt over `ejected` pieces.
    fn charge(&mut self, ejected: usize) -> Result<(), &'static str> {
        if self.attempts_remaining == 0 {
            return Err("joint re-placement attempt budget exhausted");
        }
        if self.slots_remaining < ejected {
            return Err("joint re-placement slot budget exhausted");
        }
        self.attempts_remaining -= 1;
        self.slots_remaining -= ejected;
        Ok(())
    }
}

/// `relaxed_state_from_diagnostics_with_target` for a plain fast placement
/// list, which is what every repair pass carries.
fn relaxed_state_from_fast_placements(
    pieces: &[GeneralFastPiece<'_>],
    placements: &[GeneralFastPlacement],
    target_depth_mm: f64,
) -> Result<RelaxedState, String> {
    let by_id = pieces
        .iter()
        .enumerate()
        .map(|(index, piece)| (piece.id, index))
        .collect::<BTreeMap<_, _>>();
    let mut slots = vec![None; pieces.len()];
    for placement in placements {
        let index = *by_id
            .get(placement.piece_id.as_str())
            .ok_or_else(|| format!("unknown piece {}", placement.piece_id))?;
        if slots[index].is_some() {
            return Err(format!("duplicate piece {}", placement.piece_id));
        }
        slots[index] = Some(RelaxedPlacement {
            input_index: index,
            rotation_deg: placement.rotation_deg,
            mirrored: placement.mirrored,
            translate_x: placement.translate_short_axis,
            translate_y: placement.translate_long_axis,
        });
    }
    let placements = slots
        .into_iter()
        .enumerate()
        .map(|(index, placement)| {
            placement.ok_or_else(|| format!("layout is missing piece {}", pieces[index].id))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RelaxedState {
        placements,
        strip_depth_mm: target_depth_mm,
    })
}

/// `parent_is_admissible` is `parent_is_pinned` unless the caller opted into an
/// in-process parent through
/// `GeneralRelaxedSettings::persistent_vacancy_allow_unpinned_parent`. The
/// *reported* `parent_source` is unchanged either way, so a run that descended
/// from an in-process arm can never be mistaken for a replay of a fixture.
#[allow(clippy::too_many_arguments)]
fn run_population(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    target_override_mm: Option<f64>,
    parent: &GeneralCoupledSeparatorArmDiagnostics,
    parent_is_admissible: bool,
    mode: usize,
    construction_salt: ConstructionSalt,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
) -> Result<Option<(VacancyState, f64)>, String> {
    if !matches!(
        mode,
        1 | 2
            | 3
            | 4
            | 5
            | 6
            | 7
            | 8
            | 9
            | 10
            | 11
            | 12
            | 13
            | 14
            | 15
            | 16
            | 17
            | 18
            | 19
            | 20
            | 21
            | 25
    ) {
        return Err("persistent vacancy mode must be between 1 and 21, or 25".to_owned());
    }
    // Modes 1-8 are the frozen diagnostic screens: their 165.0 mm target and
    // b9335a72 parent identity are part of the pinned experiment contract.
    // Mode 9 is the descending-target contraction lane: it requires an
    // explicitly pinned exact-valid parent fixture plus an explicit target,
    // and skips only the frozen fingerprint/depth equality pins while keeping
    // full parent validation.
    let target_depth_mm = match (mode, target_override_mm) {
        (9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18 | 19 | 20 | 21 | 25, Some(target)) => {
            if !target.is_finite() || target <= 0.0 {
                return Err(
                    "persistent vacancy target depth must be a positive finite value".to_owned(),
                );
            }
            target
        }
        (9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18 | 19 | 20 | 21 | 25, None) => {
            return Err(
                "persistent vacancy modes 9-21 and 25 require an explicit target depth".to_owned(),
            );
        }
        (_, Some(_)) => {
            return Err(
                "persistent vacancy target depth overrides require modes 9-21 and 25".to_owned(),
            );
        }
        (_, None) => TARGET_DEPTH_MM,
    };
    if matches!(
        mode,
        9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18 | 19 | 20 | 21 | 25
    ) && !parent_is_admissible
    {
        return Err(
            "persistent vacancy modes 9-21 and 25 require a pinned parent fixture".to_owned(),
        );
    }
    diagnostics.target_depth_mm = target_depth_mm;
    if pieces.is_empty() {
        return Err("persistent vacancy experiment requires at least one piece".to_owned());
    }
    // Modes 20/25 build every pose themselves, so they are the lanes that
    // accept an anchor with no placements: each piece then falls back to its
    // catalog identity pose as the sole orientation prior. Every other lane
    // derives its starting layout from the parent and still requires a
    // complete one.
    let anchor_is_synthetic = matches!(mode, 20 | 25) && parent.final_placements.is_empty();
    if !anchor_is_synthetic && parent.final_placements.len() != pieces.len() {
        return Err("persistent vacancy parent is not a complete exact-valid layout".to_owned());
    }
    let parent_fast = diagnostic_fast_placements(&parent.final_placements);
    if !matches!(mode, 13 | 20 | 25) {
        validate_and_measure_placements(pieces, &parent_fast, fast_settings)
            .map_err(|error| format!("persistent vacancy parent validation: {error}"))?;
    }
    let parent_fingerprint = coupled_fast_placement_fingerprint(&parent_fast);
    diagnostics.parent_fingerprint = Some(parent_fingerprint.clone());
    if !matches!(
        mode,
        9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18 | 19 | 20 | 21 | 25
    ) && parent_fingerprint != EXPECTED_PARENT_FINGERPRINT
    {
        return Err(format!(
            "persistent vacancy parent fingerprint mismatch: expected {EXPECTED_PARENT_FINGERPRINT}, got {parent_fingerprint}"
        ));
    }
    if !matches!(mode, 13 | 20 | 25) {
        let parent_depth = coupled_independent_source_depth(pieces, &parent_fast, fast_settings)
            .map_err(|error| format!("persistent vacancy parent depth: {error}"))?;
        if matches!(mode, 9 | 10 | 11 | 12 | 14 | 15 | 16 | 17 | 18 | 19 | 21) {
            diagnostics.parent_independent_depth_mm = Some(parent_depth);
        }
        if !matches!(mode, 9 | 10 | 11 | 12 | 14 | 15 | 16 | 17 | 18 | 19 | 21)
            && grid_key(parent_depth) != grid_key(EXPECTED_PARENT_DEPTH_MM)
        {
            return Err(format!(
                "persistent vacancy parent depth mismatch: expected {EXPECTED_PARENT_DEPTH_MM}, got {parent_depth}"
            ));
        }
    }
    for piece in pieces {
        if piece.polygon.vertex_count() > MAX_SOURCE_FEATURES {
            return Err(format!(
                "piece {} exceeds the {MAX_SOURCE_FEATURES}-feature experiment cap",
                piece.id
            ));
        }
    }

    diagnostics.attempted = true;
    let target_settings = GeneralFastSettings {
        sheet_long_axis_mm: target_depth_mm,
        ..fast_settings
    };
    let mut baseline = if anchor_is_synthetic {
        identity_relaxed_state(pieces, target_depth_mm)
    } else {
        relaxed_state_from_diagnostics_with_target(
            pieces,
            &parent.final_placements,
            target_depth_mm,
        )?
    };
    if mode == 13 {
        let (state, independent) = reconstruct_from_hints(
            pieces,
            fast_settings,
            target_depth_mm,
            &baseline,
            diagnostics,
            work,
        )?;
        return Ok(Some((state, independent)));
    }
    if matches!(mode, 20 | 25) {
        let (state, independent) = construct_skyline_beam(
            pieces,
            fast_settings,
            target_depth_mm,
            &baseline,
            mode == 25,
            construction_salt,
            diagnostics,
            work,
        )?;
        return Ok(Some((state, independent)));
    }
    if matches!(mode, 11 | 12) {
        baseline = settle_baseline(pieces, fast_settings, baseline, diagnostics, work)?;
    }
    if mode == 14 {
        baseline = compact_baseline(pieces, fast_settings, baseline, diagnostics, work)?;
    }
    if mode == 18 {
        baseline = frontier_band_feasibility(pieces, fast_settings, baseline, diagnostics, work)?;
    }
    if matches!(mode, 15 | 16 | 17 | 19 | 21) {
        baseline = lift_resettle_reinsert(
            pieces,
            fast_settings,
            target_depth_mm,
            baseline,
            matches!(mode, 16 | 17 | 19 | 21),
            matches!(mode, 17 | 19 | 21),
            mode == 19,
            mode == 21,
            diagnostics,
            work,
        )?;
    }
    let (initial, difficulty, inactive_order) = initial_vacancy_state(
        pieces,
        target_settings,
        baseline,
        diagnostics,
        work,
        matches!(mode, 11 | 12 | 14 | 15 | 16 | 17 | 18 | 19 | 21),
    )?;
    diagnostics.initial_state_fingerprint = Some(state_fingerprint(&initial, pieces));
    diagnostics.initial_active_piece_ids = active_ids(&initial, pieces);
    diagnostics.initial_inactive_piece_ids = inactive_order
        .iter()
        .map(|index| pieces[*index].id.to_owned())
        .collect();
    diagnostics.initial_inactive_order_hash = Some(id_order_hash(&inactive_order, pieces));
    if inactive_order.is_empty() {
        // Modes 11/12 only: the settling prelude already pulled every piece
        // inside the target strip, so the settled state is a complete
        // candidate. It is counted before the audit, must still pass the
        // unchanged dual publication audit, and a non-cap audit failure is
        // recorded as a publication rejection before the arm fails.
        diagnostics.complete_states = diagnostics.complete_states.saturating_add(1);
        if let Err(reason) = audit_state(&initial, pieces, target_settings, true, work) {
            if !reason.starts_with("cap: ") {
                diagnostics.publication_rejections =
                    diagnostics.publication_rejections.saturating_add(1);
            }
            return Err(reason);
        }
        let placements = fast_placements(&initial, pieces, false);
        let independent = coupled_independent_source_depth(pieces, &placements, target_settings)
            .map_err(|error| format!("persistent vacancy settled depth: {error}"))?;
        return Ok(Some((initial, independent)));
    }
    audit_state(&initial, pieces, target_settings, false, work)?;

    let hazard_catalog = Arc::new(
        JaguaHazardCatalog::new(pieces, target_settings)
            .map_err(|error| format!("persistent vacancy hazard catalog: {error}"))?,
    );
    let baseline_placements = initial.placements.clone();
    let mut population = vec![initial];
    let mut best_ever_area: Option<EliteSnapshot> = None;
    let mut best_ever_count: Option<EliteSnapshot> = None;
    let mut retained_carryovers = BTreeSet::new();
    let mut archive = matches!(
        mode,
        7 | 8 | 9 | 10 | 11 | 12 | 14 | 15 | 16 | 17 | 18 | 19 | 21
    )
    .then(TopologyArchive::new);
    for layer in 0..MAX_LAYERS {
        // Modes 7/8 plan a revival before the entering-population hash so the
        // hash always reflects the population that is actually expanded
        // (mode 8 swaps the comparator-worst entering slot in place).
        let mut layer_archive = None;
        let mut revival_parent: Option<VacancyState> = None;
        if let Some(archive_state) = archive.as_mut() {
            let layers_since_improvement =
                layer.saturating_sub(archive_state.last_improvement_layer);
            let mut row = GeneralPersistentVacancyArchiveLayerDiagnostics {
                layers_since_improvement,
                ..GeneralPersistentVacancyArchiveLayerDiagnostics::default()
            };
            match archive_state.plan_revival(layer, &population, pieces, &difficulty, mode) {
                RevivalDecision::NotStagnant => {}
                RevivalDecision::Skipped(reason) => {
                    archive_state.revivals_skipped =
                        archive_state.revivals_skipped.saturating_add(1);
                    row.revival_attempted = true;
                    row.skipped_reason = Some(reason.to_owned());
                }
                RevivalDecision::Revive {
                    kind,
                    state,
                    fingerprint,
                } => {
                    archive_state.revivals_expanded =
                        archive_state.revivals_expanded.saturating_add(1);
                    archive_state.last_revival_layer = Some(layer);
                    archive_state.revival_ordinal = archive_state.revival_ordinal.saturating_add(1);
                    row.revival_attempted = true;
                    row.revival_expanded = true;
                    row.revival_kind = Some(kind.to_owned());
                    row.revived_state_fingerprint = Some(fingerprint);
                    if matches!(mode, 8 | 9 | 10 | 11 | 12 | 14 | 15 | 16 | 17 | 18 | 19) {
                        let replaced_index = population.len() - 1;
                        row.replaced_state_fingerprint =
                            Some(state_fingerprint(&population[replaced_index], pieces));
                        population[replaced_index] = state;
                    } else {
                        revival_parent = Some(state);
                    }
                }
            }
            layer_archive = Some(row);
        }
        let layer_entry_work = generation_work_snapshot(work.diagnostics);
        let entering_population_hash = population_hash(&population, pieces);
        let expanded_carryover_fingerprints = population
            .iter()
            .map(|state| state_fingerprint(state, pieces))
            .filter(|fingerprint| retained_carryovers.contains(fingerprint))
            .collect::<Vec<_>>();
        let carryover_states = if mode == 5 {
            distinct_elite_states(&population, pieces, &difficulty)
        } else {
            Vec::new()
        };
        let offered_carryover_fingerprints = carryover_states
            .iter()
            .map(|state| state_fingerprint(state, pieces))
            .collect::<Vec<_>>();
        let mut children = Vec::new();
        let mut selected_piece_ids = BTreeSet::new();
        let mut parent_selections = Vec::new();
        let direct_before = diagnostics.direct_insertions;
        let ejections_before = diagnostics.ejection_insertions;
        for parent_state in &population {
            expand_parent(
                parent_state,
                &baseline_placements,
                pieces,
                target_settings,
                &difficulty,
                &hazard_catalog,
                layer,
                mode,
                diagnostics,
                work,
                &mut selected_piece_ids,
                &mut parent_selections,
                &mut children,
            )?;
        }
        let ordinary_children_count = children.len();
        if let Some(revived_state) = &revival_parent {
            let revival_row_index = parent_selections.len();
            expand_parent(
                revived_state,
                &baseline_placements,
                pieces,
                target_settings,
                &difficulty,
                &hazard_catalog,
                layer,
                mode,
                diagnostics,
                work,
                &mut selected_piece_ids,
                &mut parent_selections,
                &mut children,
            )?;
            if let Some(row) = parent_selections.get_mut(revival_row_index) {
                row.revived = Some(true);
            }
        }
        let revival_child_fingerprints = children[ordinary_children_count..]
            .iter()
            .map(|state| state_fingerprint(state, pieces))
            .collect::<BTreeSet<_>>();
        if let (Some(archive_state), Some(row)) = (archive.as_mut(), layer_archive.as_mut()) {
            let generated = children.len().saturating_sub(ordinary_children_count);
            row.revival_children_generated = generated;
            archive_state.revival_children_generated = archive_state
                .revival_children_generated
                .saturating_add(generated);
        }
        if children.is_empty() {
            return Err(format!(
                "persistent vacancy layer {layer} produced no exact-valid child"
            ));
        }
        let selected_piece_ids = selected_piece_ids.into_iter().collect::<Vec<_>>();
        let ordinary_live_state_bytes = state_vec_bytes(&children);
        let carryover_live_state_bytes = state_vec_bytes(&carryover_states);
        let combined_pool_backing_bytes = children
            .len()
            .saturating_add(carryover_states.len())
            .saturating_mul(size_of::<VacancyState>());
        let mut largest_clone_bytes = 0usize;
        for state in children.iter().chain(&carryover_states) {
            let bytes = size_of::<VacancyState>().saturating_add(state_heap_bytes(state));
            largest_clone_bytes = largest_clone_bytes.max(bytes);
        }
        let retained_clone_bytes = largest_clone_bytes.saturating_mul(2);
        preflight_raw_live_memory(
            &population,
            ordinary_live_state_bytes,
            carryover_live_state_bytes,
            retained_clone_bytes,
            combined_pool_backing_bytes,
            archive.as_ref().map_or(0, TopologyArchive::bytes),
            &selected_piece_ids,
            &parent_selections,
            diagnostics,
            work,
        )?;
        // The ordinary child-order hash keeps its cross-mode meaning: it
        // covers exactly the ordinary parents' children. Mode-7 revival
        // children are merged only after that hash is taken.
        let mut revival_children = children.split_off(ordinary_children_count);
        children.sort_by(|first, second| compare_states(first, second, pieces, &difficulty));
        let before_dedup = children.len();
        children.dedup_by(|first, second| same_state_identity(first, second));
        diagnostics.deduplicated_states = diagnostics
            .deduplicated_states
            .saturating_add(before_dedup.saturating_sub(children.len()));
        let ordinary_child_order_hash = child_order_hash(&children, pieces);
        if !revival_children.is_empty() {
            children.append(&mut revival_children);
            children.sort_by(|first, second| compare_states(first, second, pieces, &difficulty));
            let before_merge_dedup = children.len();
            children.dedup_by(|first, second| same_state_identity(first, second));
            diagnostics.deduplicated_states = diagnostics
                .deduplicated_states
                .saturating_add(before_merge_dedup.saturating_sub(children.len()));
        }

        let complete_count = children
            .iter()
            .take_while(|state| state.active.iter().all(|active| *active))
            .count();
        let complete_candidate_order_hash = child_order_hash(&children[..complete_count], pieces);
        diagnostics.complete_states = diagnostics.complete_states.saturating_add(complete_count);
        let mut accepted_complete = None;
        for candidate in children.iter().take(complete_count) {
            match audit_state(candidate, pieces, target_settings, true, work) {
                Ok(_) => {
                    let placements = fast_placements(candidate, pieces, false);
                    let independent =
                        coupled_independent_source_depth(pieces, &placements, target_settings)
                            .map_err(|error| format!("persistent vacancy final depth: {error}"))?;
                    accepted_complete = Some((candidate.clone(), independent));
                    break;
                }
                Err(reason) if !reason.starts_with("cap: ") => {
                    diagnostics.publication_rejections =
                        diagnostics.publication_rejections.saturating_add(1);
                }
                Err(reason) => return Err(reason),
            }
        }
        children.retain(|state| state.active.iter().any(|active| !*active));
        if children.is_empty() && accepted_complete.is_none() {
            return Err(format!(
                "persistent vacancy layer {layer} retained only publication-invalid complete states"
            ));
        }

        let generated_children = children.len();
        let effective_carryover_fingerprints = if mode == 5 {
            let ordinary_partial_fingerprints = children
                .iter()
                .map(|state| state_fingerprint(state, pieces))
                .collect::<BTreeSet<_>>();
            carryover_states
                .iter()
                .map(|state| state_fingerprint(state, pieces))
                .filter(|fingerprint| !ordinary_partial_fingerprints.contains(fingerprint))
                .collect::<BTreeSet<_>>()
        } else {
            BTreeSet::new()
        };
        let pre_carryover_work =
            work_delta(generation_work_snapshot(work.diagnostics), layer_entry_work);
        if accepted_complete.is_none() {
            let (combined, carryover_deduplicated) =
                retention_pool(children, carryover_states, pieces, &difficulty, mode);
            children = combined;
            diagnostics.deduplicated_states = diagnostics
                .deduplicated_states
                .saturating_add(carryover_deduplicated);
        }
        let (next, distinct_signatures) = if let Some((candidate, _)) = &accepted_complete {
            (vec![candidate.clone()], 1)
        } else {
            retain_population(children, pieces, &difficulty, mode)
        };
        if next.is_empty() {
            return Err(format!(
                "persistent vacancy layer {layer} retained no state"
            ));
        }
        enforce_population_width(mode, accepted_complete.is_some(), next.len(), layer)?;
        diagnostics.distinct_signatures_retained = diagnostics
            .distinct_signatures_retained
            .saturating_add(distinct_signatures);
        if accepted_complete.is_none() {
            audit_state(&next[0], pieces, target_settings, false, work)?;
        }
        let retained_carryover_fingerprints = if mode == 5 {
            next.iter()
                .map(|state| state_fingerprint(state, pieces))
                .filter(|fingerprint| effective_carryover_fingerprints.contains(fingerprint))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        if let (Some(archive_state), Some(row)) = (archive.as_mut(), layer_archive.as_mut()) {
            if !revival_child_fingerprints.is_empty() {
                let retained = next
                    .iter()
                    .filter(|state| {
                        revival_child_fingerprints.contains(&state_fingerprint(state, pieces))
                    })
                    .count();
                row.revival_children_retained = retained;
                archive_state.revival_children_retained = archive_state
                    .revival_children_retained
                    .saturating_add(retained);
            }
        }
        let (area_elite, count_elite) = population_elites(&next, pieces, &difficulty);
        let area_snapshot = elite_snapshot(area_elite, pieces, &difficulty);
        let count_snapshot = elite_snapshot(count_elite, pieces, &difficulty);
        let area_improved = update_best_area(&mut best_ever_area, &area_snapshot);
        let count_improved = update_best_count(&mut best_ever_count, &count_snapshot);
        if let Some(archive_state) = archive.as_mut() {
            if area_improved {
                archive_state.area = Some((area_snapshot.clone(), area_elite.clone()));
            }
            if count_improved {
                archive_state.count = Some((count_snapshot.clone(), count_elite.clone()));
            }
            if area_improved || count_improved {
                archive_state.last_improvement_layer = layer;
            }
            archive_state.charge_peak();
            if let Some(row) = layer_archive.as_mut() {
                row.archived_area_updated = area_improved;
                row.archived_count_updated = count_improved;
            }
        }
        let best_ever_area_snapshot = best_ever_area
            .as_ref()
            .expect("the current area elite initializes best-ever history");
        let best_ever_count_snapshot = best_ever_count
            .as_ref()
            .expect("the current count elite initializes best-ever history");
        let best_identity = state_identity(&next[0]);
        let layer_diagnostics = GeneralPersistentVacancyLayerDiagnostics {
            layer,
            parents: population.len(),
            generated_children,
            retained_states: next.len(),
            distinct_contact_signatures: distinct_signatures,
            selected_piece_ids,
            parent_selections,
            direct_insertions: diagnostics.direct_insertions.saturating_sub(direct_before),
            ejection_insertions: diagnostics
                .ejection_insertions
                .saturating_sub(ejections_before),
            best_inactive_piece_count: best_identity.inactive.len(),
            best_inactive_piece_ids: best_identity
                .inactive
                .iter()
                .map(|index| pieces[*index].id.to_owned())
                .collect(),
            best_inactive_area_grid2: inactive_area(&next[0], &difficulty).to_string(),
            best_state_fingerprint: state_fingerprint(&next[0], pieces),
            elite: Some(GeneralPersistentVacancyEliteLayerDiagnostics {
                entering_population_hash,
                ordinary_child_order_hash,
                complete_candidate_order_hash,
                pre_carryover_work,
                area_elite_fingerprint: area_snapshot.fingerprint.clone(),
                area_elite_inactive_piece_count: area_snapshot.inactive_piece_count,
                area_elite_inactive_area_grid2: area_snapshot.inactive_area_grid2.to_string(),
                count_elite_fingerprint: count_snapshot.fingerprint.clone(),
                count_elite_inactive_piece_count: count_snapshot.inactive_piece_count,
                count_elite_inactive_area_grid2: count_snapshot.inactive_area_grid2.to_string(),
                best_ever_area_elite_fingerprint: best_ever_area_snapshot.fingerprint.clone(),
                best_ever_area_elite_inactive_piece_count: best_ever_area_snapshot
                    .inactive_piece_count,
                best_ever_area_elite_inactive_area_grid2: best_ever_area_snapshot
                    .inactive_area_grid2
                    .to_string(),
                best_ever_count_elite_fingerprint: best_ever_count_snapshot.fingerprint.clone(),
                best_ever_count_elite_inactive_piece_count: best_ever_count_snapshot
                    .inactive_piece_count,
                best_ever_count_elite_inactive_area_grid2: best_ever_count_snapshot
                    .inactive_area_grid2
                    .to_string(),
                offered_carryovers_distinct: offered_carryover_fingerprints.len() > 1,
                offered_carryover_fingerprints,
                retained_carryover_fingerprints: retained_carryover_fingerprints.clone(),
                expanded_carryover_fingerprints,
            }),
            archive: layer_archive,
        };
        preflight_live_memory(
            &population,
            ordinary_live_state_bytes,
            carryover_live_state_bytes,
            retained_clone_bytes,
            combined_pool_backing_bytes,
            archive.as_ref().map_or(0, TopologyArchive::bytes),
            diagnostics,
            &layer_diagnostics,
            work,
        )?;
        charge_retained_memory(
            &next,
            archive.as_ref().map_or(0, TopologyArchive::bytes),
            diagnostics,
            &layer_diagnostics,
            work,
        )?;
        diagnostics.layers.push(layer_diagnostics);
        diagnostics.layers_completed = layer + 1;
        if let Some(archive_state) = archive.as_ref() {
            diagnostics.archive = Some(GeneralPersistentVacancyArchiveDiagnostics {
                stagnation_threshold_layers: ARCHIVE_STAGNATION_LAYERS,
                revival_cooldown_layers: ARCHIVE_REVIVAL_COOLDOWN,
                max_revival_expansions: MAX_ARCHIVE_REVIVALS,
                revival_policy: if mode == 7 {
                    "extraParent".to_owned()
                } else {
                    "swapWorstEntering".to_owned()
                },
                revivals_expanded: archive_state.revivals_expanded,
                revivals_skipped: archive_state.revivals_skipped,
                revival_children_generated: archive_state.revival_children_generated,
                revival_children_retained: archive_state.revival_children_retained,
                archive_peak_bytes: archive_state.peak_bytes,
                final_archived_area_fingerprint: archive_state
                    .area
                    .as_ref()
                    .map(|(snapshot, _)| snapshot.fingerprint.clone()),
                final_archived_count_fingerprint: archive_state
                    .count
                    .as_ref()
                    .map(|(snapshot, _)| snapshot.fingerprint.clone()),
            });
        }
        if let Some(complete) = accepted_complete {
            return Ok(Some(complete));
        }
        retained_carryovers = retained_carryover_fingerprints.into_iter().collect();
        population = next;
    }
    Ok(None)
}

#[derive(Clone, Copy)]
struct SettleKey {
    max_y: i64,
    translate_y: i64,
    translate_x: i64,
}

fn settle_key_for(
    collision: &PolygonSet,
    placement: &RelaxedPlacement,
) -> Result<SettleKey, String> {
    let bounds = collision
        .bounds()
        .ok_or_else(|| "settle candidate has empty collision geometry".to_owned())?;
    Ok(SettleKey {
        max_y: grid_key(bounds.max_y),
        translate_y: grid_key(placement.translate_y),
        translate_x: grid_key(placement.translate_x),
    })
}

fn settle_key_less(first: SettleKey, second: SettleKey) -> bool {
    (first.max_y, first.translate_y, first.translate_x)
        < (second.max_y, second.translate_y, second.translate_x)
}

/// Mode-11 exact settling prelude: translation-only, bottom-up drop
/// compaction over every piece of the full exact-valid parent layout, before
/// any target deactivation. Each attempt keeps the piece's current
/// orientation and horizontal position and lowers the piece with a
/// decreasing step ladder (0.512 mm down to 0.001 mm), exact-confirming every
/// probe with full-sheet containment plus zero exact pair intersection
/// against every other piece. This is an endpoint-exact re-placement move,
/// not a swept-motion contract: near-tangent neighbors can form forbidden
/// bands thinner than one step, so a probe may land beyond a band no
/// continuous slide could cross. That matches every other placement operator
/// in this experiment, all of which relocate pieces discontinuously; validity
/// rests entirely on the per-probe exact gates and the final dual
/// publication audit, never on motion continuity.
const SETTLE_STEP_LADDER_MM: [f64; 10] = [
    0.512, 0.256, 0.128, 0.064, 0.032, 0.016, 0.008, 0.004, 0.002, 0.001,
];

fn settle_baseline(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    baseline: RelaxedState,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
) -> Result<RelaxedState, String> {
    let mut state = VacancyState {
        collisions: baseline
            .placements
            .iter()
            .enumerate()
            .map(|(index, placement)| {
                build_collision(pieces[index], placement, fast_settings, work)
                    .map(|collision| Some(Arc::new(collision)))
            })
            .collect::<Result<Vec<_>, _>>()?,
        placements: baseline.placements.clone(),
        active: vec![true; pieces.len()],
        last_transition: None,
    };
    let frontier = |state: &VacancyState| -> i64 {
        state
            .collisions
            .iter()
            .flatten()
            .filter_map(|collision| collision.bounds())
            .map(|bounds| grid_key(bounds.max_y))
            .max()
            .unwrap_or(i64::MIN)
    };
    let mut settle = GeneralPersistentVacancySettleDiagnostics {
        sweeps: SETTLE_SWEEPS,
        attempts: 0,
        accepted_moves: 0,
        exact_rows: 0,
        frontier_before_grid: frontier(&state),
        frontier_after_grid: 0,
    };
    let inset = collision_sheet_inset_mm(fast_settings);
    for _sweep in 0..SETTLE_SWEEPS {
        settle_sweep(
            &mut state,
            pieces,
            fast_settings,
            inset,
            false,
            &mut settle,
            work,
        )?;
    }
    settle.frontier_after_grid = frontier(&state);
    diagnostics.settle = Some(settle);
    Ok(RelaxedState {
        placements: state.placements,
        strip_depth_mm: baseline.strip_depth_mm,
    })
}

/// Mode-14 compaction prelude: alternates one per-piece settle sweep with
/// one guillotine group-drop pass per round, then exactly re-anchors the
/// state by rebuilding every collision from its placement and re-verifying
/// all pairs, failing closed on any disagreement. Group drops translate every
/// active piece above a horizontal cut downward as one rigid body, so pairs
/// inside the group are preserved by construction and only group-versus-rest
/// pairs plus containment need exact confirmation.
fn compact_baseline(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    baseline: RelaxedState,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
) -> Result<RelaxedState, String> {
    let mut state = VacancyState {
        collisions: baseline
            .placements
            .iter()
            .enumerate()
            .map(|(index, placement)| {
                build_collision(pieces[index], placement, fast_settings, work)
                    .map(|collision| Some(Arc::new(collision)))
            })
            .collect::<Result<Vec<_>, _>>()?,
        placements: baseline.placements.clone(),
        active: vec![true; pieces.len()],
        last_transition: None,
    };
    let frontier = |state: &VacancyState| -> i64 {
        state
            .collisions
            .iter()
            .flatten()
            .filter_map(|collision| collision.bounds())
            .map(|bounds| grid_key(bounds.max_y))
            .max()
            .unwrap_or(i64::MIN)
    };
    let mut settle = GeneralPersistentVacancySettleDiagnostics {
        sweeps: COMPACTION_ROUNDS,
        attempts: 0,
        accepted_moves: 0,
        exact_rows: 0,
        frontier_before_grid: frontier(&state),
        frontier_after_grid: 0,
    };
    let mut group_drop = GeneralPersistentVacancyGroupDropDiagnostics {
        rounds: COMPACTION_ROUNDS,
        cuts_evaluated: 0,
        probes: 0,
        accepted_drops: 0,
        frontier_before_grid: settle.frontier_before_grid,
        frontier_after_grid: 0,
    };
    let inset = collision_sheet_inset_mm(fast_settings);
    for _round in 0..COMPACTION_ROUNDS {
        settle_sweep(
            &mut state,
            pieces,
            fast_settings,
            inset,
            true,
            &mut settle,
            work,
        )?;
        group_drop_pass(&mut state, fast_settings, inset, &mut group_drop, work)?;
    }
    settle.frontier_after_grid = frontier(&state);
    group_drop.frontier_after_grid = settle.frontier_after_grid;
    diagnostics.settle = Some(settle);
    diagnostics.group_drop = Some(group_drop);
    // Exact re-anchor: incremental group translations accumulate f64 sums, so
    // every collision is rebuilt from its placement and every pair re-proved
    // before the compacted state is trusted.
    for index in 0..pieces.len() {
        let rebuilt =
            build_collision(pieces[index], &state.placements[index], fast_settings, work)?;
        if !rebuilt.fits_rect(
            inset,
            inset,
            fast_settings.sheet_short_axis_mm - inset,
            fast_settings.sheet_long_axis_mm - inset,
        ) {
            return Err(format!(
                "compaction re-anchor: piece {} escaped containment",
                pieces[index].id
            ));
        }
        state.collisions[index] = Some(Arc::new(rebuilt));
    }
    for first in 0..pieces.len() {
        for second in (first + 1)..pieces.len() {
            work.charge_experimental_pair()?;
            let a = state.collisions[first]
                .as_ref()
                .ok_or_else(|| "re-anchor missing collision".to_owned())?;
            let b = state.collisions[second]
                .as_ref()
                .ok_or_else(|| "re-anchor missing collision".to_owned())?;
            if exact_intersection_area(a, b, work)? > 0.0 {
                return Err(format!(
                    "compaction re-anchor: pieces {} and {} overlap after group drops",
                    pieces[first].id, pieces[second].id
                ));
            }
        }
    }
    Ok(RelaxedState {
        placements: state.placements,
        strip_depth_mm: baseline.strip_depth_mm,
    })
}

/// One guillotine pass: for each distinct active min-y cut in descending
/// order, the rigid group of all pieces at or above the cut slides downward
/// with the settle step ladder. A probe is legal when every group piece stays
/// inside the full-sheet inset rectangle and no translated group piece
/// exactly intersects any piece outside the group.
fn group_drop_pass(
    state: &mut VacancyState,
    settings: GeneralFastSettings,
    inset: f64,
    diagnostics: &mut GeneralPersistentVacancyGroupDropDiagnostics,
    work: &mut RunWork,
) -> Result<(), String> {
    let min_y_of = |state: &VacancyState, index: usize| -> Option<i64> {
        state.collisions[index]
            .as_ref()
            .and_then(|collision| collision.bounds())
            .map(|bounds| grid_key(bounds.min_y))
    };
    let mut cuts = (0..state.active.len())
        .filter(|index| state.active[*index])
        .filter_map(|index| min_y_of(state, index))
        .collect::<Vec<_>>();
    cuts.sort_unstable();
    cuts.dedup();
    cuts.reverse();
    cuts.truncate(work.quotas.group_drop_cuts);
    for cut in cuts {
        diagnostics.cuts_evaluated += 1;
        let group = (0..state.active.len())
            .filter(|index| state.active[*index])
            .filter(|index| min_y_of(state, *index).is_some_and(|min_y| min_y >= cut))
            .collect::<Vec<_>>();
        if group.is_empty() {
            continue;
        }
        let in_group = {
            let mut mask = vec![false; state.active.len()];
            for index in &group {
                mask[*index] = true;
            }
            mask
        };
        let mut probes = 0usize;
        'ladder: for step in SETTLE_STEP_LADDER_MM {
            loop {
                if probes >= GROUP_DROP_PROBES_PER_CUT {
                    break 'ladder;
                }
                probes += 1;
                diagnostics.probes += 1;
                let mut legal = true;
                let mut translated = Vec::with_capacity(group.len());
                for index in &group {
                    let collision = state.collisions[*index]
                        .as_ref()
                        .ok_or_else(|| "group drop missing collision".to_owned())?;
                    let bounds = collision
                        .bounds()
                        .ok_or_else(|| "group drop empty collision".to_owned())?;
                    if bounds.min_y - step < inset {
                        legal = false;
                        break;
                    }
                    let moved = collision
                        .translated(0.0, -step)
                        .map_err(|error| format!("group drop translation: {error}"))?;
                    translated.push((*index, moved));
                }
                if legal {
                    'pairs: for (_, moved) in &translated {
                        for fixed_index in 0..state.active.len() {
                            if !state.active[fixed_index] || in_group[fixed_index] {
                                continue;
                            }
                            work.charge_experimental_pair()?;
                            let fixed = state.collisions[fixed_index]
                                .as_ref()
                                .ok_or_else(|| "group drop missing fixed collision".to_owned())?;
                            if exact_intersection_area(moved, fixed, work)? > 0.0 {
                                legal = false;
                                break 'pairs;
                            }
                        }
                    }
                }
                if !legal {
                    break;
                }
                for (index, moved) in translated {
                    state.placements[index].translate_y -= step;
                    state.collisions[index] = Some(Arc::new(moved));
                }
                diagnostics.accepted_drops += 1;
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
/// Mode-15 non-monotone lifecycle: lift the frontier piece with its nearest
/// neighborhood, resettle the survivors into the vacated space, reinsert the
/// removed pieces with full orientation freedom, and accept only rounds
/// whose complete result strictly lowers the frontier, reverting to the
/// round snapshot otherwise. Every intermediate state is exact-valid
/// (removal cannot invalidate, every reinsertion passes the exact gates),
/// motion is deliberately non-monotone for the lifted pieces, and every
/// selection derives from geometry and stable identifiers only.
fn lift_resettle_reinsert(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    target_depth_mm: f64,
    baseline: RelaxedState,
    separation: bool,
    vacancy_transport: bool,
    band_ruin: bool,
    bridge_ruin: bool,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
) -> Result<RelaxedState, String> {
    // The lifecycle works at full-sheet settings so lifted pieces can park
    // high transiently inside a round (the essential non-monotone freedom);
    // the requested target only gates the downstream deactivation and the
    // dual publication audit. The target's grid value additionally salts the
    // walk seed, so a caller can restart a stalled parent onto a distinct
    // deterministic walk by micro-varying the target - a replayable
    // multi-start without any hidden state.
    let target_salt = grid_key(target_depth_mm) as u64;
    let work_settings = fast_settings;
    let mut state = VacancyState {
        collisions: baseline
            .placements
            .iter()
            .enumerate()
            .map(|(index, placement)| {
                build_collision(pieces[index], placement, work_settings, work)
                    .map(|collision| Some(Arc::new(collision)))
            })
            .collect::<Result<Vec<_>, _>>()?,
        placements: baseline.placements.clone(),
        active: vec![true; pieces.len()],
        last_transition: None,
    };
    let frontier = |state: &VacancyState| -> i64 {
        state
            .collisions
            .iter()
            .flatten()
            .filter_map(|collision| collision.bounds())
            .map(|bounds| grid_key(bounds.max_y))
            .max()
            .unwrap_or(i64::MIN)
    };
    // Lexicographic acceptance key: the frontier first, then the sum of all
    // piece frontiers. Plateau rounds that keep the frontier but thin the
    // top band are accepted, progressively draining the band until the
    // frontier itself can drop; the pair strictly decreases on every
    // accepted round, so the walk terminates.
    let depth_key = |state: &VacancyState| -> (i64, i128, i128) {
        let mut sum: i128 = 0;
        let mut max = i64::MIN;
        for collision in state.collisions.iter().flatten() {
            if let Some(bounds) = collision.bounds() {
                let key = grid_key(bounds.max_y);
                sum += i128::from(key);
                max = max.max(key);
            }
        }
        // Vacancy-transport signal: trapped-void cells become the middle key
        // so an endpoint that lifts a piece but drains a trapped void toward
        // the top-connected region reads as progress; the piece-centric keys
        // cannot see that trade.
        let voids = if vacancy_transport {
            i128::try_from(trapped_void_cells(state, fast_settings, max)).unwrap_or(i128::MAX)
        } else {
            0
        };
        (max, voids, sum)
    };
    let mut settle = GeneralPersistentVacancySettleDiagnostics {
        sweeps: LNS_SETTLE_SWEEPS,
        attempts: 0,
        accepted_moves: 0,
        exact_rows: 0,
        frontier_before_grid: frontier(&state),
        frontier_after_grid: 0,
    };
    let mut lns = GeneralPersistentVacancyLnsDiagnostics {
        rounds: LNS_ROUNDS,
        bridge_void_scans: 0,
        bridge_selections: 0,
        rounds_accepted: 0,
        rounds_reverted: 0,
        reinsertions: 0,
        reinsert_failures: 0,
        separation_moves: 0,
        separation_probes: 0,
        separation_zero_overlap: 0,
        separation_recruits: 0,
        separation_pair_moves: 0,
        separation_weight_bumps: 0,
        separation_relocations: 0,
        rounds_wandered: 0,
        optimizer_improvements: 0,
        frontier_before_grid: settle.frontier_before_grid,
        frontier_after_grid: 0,
    };
    let inset = collision_sheet_inset_mm(work_settings);
    let hazard_catalog = Arc::new(
        JaguaHazardCatalog::new(pieces, work_settings)
            .map_err(|error| format!("lns hazard catalog: {error}"))?,
    );
    let lns_seed = parent_seed_key(&state, pieces) ^ target_salt;
    settle_sweep(
        &mut state,
        pieces,
        work_settings,
        inset,
        true,
        &mut settle,
        work,
    )?;
    let mut recon = GeneralPersistentVacancyReconstructionDiagnostics::default();
    recon.rows_per_piece_cap = RECONSTRUCTION_ROWS_PER_PIECE;
    // Record-to-record travel: a round endpoint within the deterministic
    // tolerance of the entry key is kept as the wander state so later rounds
    // explore from it, while the best state ever seen is tracked separately
    // and returned, so the published result can never regress.
    const LNS_TOLERANCE_GRID: [i128; LNS_ROUNDS] = [
        0, 2_000, 4_000, 8_000, 0, 2_000, 4_000, 8_000, 0, 2_000, 4_000, 8_000, 0, 2_000, 4_000,
        8_000, 0, 2_000, 4_000, 8_000, 0, 2_000, 4_000, 8_000,
    ];
    const LNS_FRONTIER_TOLERANCE_GRID: [i64; LNS_ROUNDS] = [
        0, 500, 1_000, 2_000, 0, 500, 1_000, 2_000, 0, 500, 1_000, 2_000, 0, 500, 1_000, 2_000, 0,
        500, 1_000, 2_000, 0, 500, 1_000, 2_000,
    ];
    let mut best_state = state.clone();
    let mut best_key = depth_key(&state);
    // Tabu memory: wander endpoints whose semantic fingerprint was already
    // visited in this walk are reverted, breaking the deterministic limit
    // cycles that otherwise trap the record-to-record traversal.
    let mut visited = BTreeSet::new();
    visited.insert(state_fingerprint(&state, pieces));
    for (round, neighborhood) in LNS_NEIGHBORHOOD_SCHEDULE.into_iter().enumerate() {
        let snapshot = state.clone();
        let entry_key = depth_key(&state);
        // Bridge selection (mode 21): probe every active piece for the
        // vacancy its removal would reconnect (uncharged flood fills,
        // counted and structurally bounded like the acceptance-key scans)
        // and seed the ruin on the strongest free-space bridge instead of
        // the deepest frontier piece. Everything downstream - neighborhood,
        // budgets, streams, acceptance - is identical to the mode-17
        // control, so the arms differ only in removal selection.
        let mut bridge_piece = None;
        if bridge_ruin {
            let baseline_frontier = (0..pieces.len())
                .filter(|index| state.active[*index])
                .filter_map(|index| {
                    state.collisions[index]
                        .as_ref()
                        .and_then(|collision| collision.bounds())
                        .map(|bounds| grid_key(bounds.max_y))
                })
                .max()
                .unwrap_or(0);
            lns.bridge_void_scans = lns.bridge_void_scans.saturating_add(1);
            let baseline_voids = trapped_void_cells(&state, work_settings, baseline_frontier);
            if baseline_voids > 0 {
                let mut best: Option<(usize, usize)> = None;
                let actives = (0..pieces.len())
                    .filter(|index| state.active[*index])
                    .collect::<Vec<_>>();
                for index in actives {
                    state.active[index] = false;
                    lns.bridge_void_scans = lns.bridge_void_scans.saturating_add(1);
                    let voids_without =
                        trapped_void_cells(&state, work_settings, baseline_frontier);
                    state.active[index] = true;
                    let reconnected = baseline_voids.saturating_sub(voids_without);
                    let better = match &best {
                        None => reconnected > 0,
                        Some((best_reconnected, best_index)) => {
                            reconnected > *best_reconnected
                                || (reconnected == *best_reconnected
                                    && pieces[index].id < pieces[*best_index].id)
                        }
                    };
                    if better {
                        best = Some((reconnected, index));
                    }
                }
                if let Some((_, index)) = best {
                    lns.bridge_selections = lns.bridge_selections.saturating_add(1);
                    bridge_piece = Some(index);
                }
            }
        }
        // Frontier piece: the round-th deepest active piece (modulo four),
        // ties by stable ID, so consecutive rounds attack different members
        // of the frontier band instead of retrying one piece.
        let mut by_depth = (0..pieces.len())
            .filter(|index| state.active[*index])
            .filter_map(|index| {
                state.collisions[index]
                    .as_ref()
                    .and_then(|collision| collision.bounds())
                    .map(|bounds| (grid_key(bounds.max_y), index))
            })
            .collect::<Vec<_>>();
        by_depth.sort_by(|first, second| {
            second
                .0
                .cmp(&first.0)
                .then_with(|| pieces[first.1].id.cmp(pieces[second.1].id))
        });
        let Some(frontier_piece) = bridge_piece.or_else(|| {
            by_depth
                .get(round % 4)
                .or_else(|| by_depth.first())
                .map(|(_, index)| *index)
        }) else {
            break;
        };
        let frontier_center = state.collisions[frontier_piece]
            .as_ref()
            .and_then(|collision| collision.bounds())
            .map(|bounds| {
                (
                    grid_key((bounds.min_x + bounds.max_x) * 0.5),
                    grid_key((bounds.min_y + bounds.max_y) * 0.5),
                )
            })
            .ok_or_else(|| "frontier piece has no bounds".to_owned())?;
        let mut by_distance = (0..pieces.len())
            .filter(|index| state.active[*index] && *index != frontier_piece)
            .filter_map(|index| {
                state.collisions[index]
                    .as_ref()
                    .and_then(|collision| collision.bounds())
                    .map(|bounds| {
                        let center_x = grid_key((bounds.min_x + bounds.max_x) * 0.5);
                        let center_y = grid_key((bounds.min_y + bounds.max_y) * 0.5);
                        let distance = center_x
                            .abs_diff(frontier_center.0)
                            .saturating_add(center_y.abs_diff(frontier_center.1));
                        (distance, index)
                    })
            })
            .collect::<Vec<_>>();
        by_distance.sort_by(|first, second| {
            first
                .0
                .cmp(&second.0)
                .then_with(|| pieces[first.1].id.cmp(pieces[second.1].id))
        });
        let mut removed = vec![frontier_piece];
        if band_ruin {
            // Band ruin: remove the K deepest pieces as a set, regardless of
            // adjacency. The frontier band's tops sit atop different columns
            // spread across the width; spatial-neighborhood ruins never
            // remove them together, and the mode-18 certificate proves no
            // single one of them has a sub-frontier pose alone.
            removed = by_depth
                .iter()
                .take(neighborhood)
                .map(|(_, index)| *index)
                .collect();
        } else {
            removed.extend(
                by_distance
                    .into_iter()
                    .take(neighborhood.saturating_sub(1))
                    .map(|(_, index)| index),
            );
        }
        // Old poses are the reinsertion hints; removal itself cannot
        // invalidate the remaining exact-valid layout.
        let hints = RelaxedState {
            placements: state.placements.clone(),
            strip_depth_mm: work_settings.sheet_long_axis_mm,
        };
        for index in &removed {
            state.active[*index] = false;
            state.collisions[*index] = None;
        }
        settle_sweep(
            &mut state,
            pieces,
            work_settings,
            inset,
            true,
            &mut settle,
            work,
        )?;
        // Reinsert in descending material area, ties by stable ID, so large
        // pieces claim space first.
        let mut reinsert_order = removed.clone();
        reinsert_order.sort_by(|first, second| {
            let area = |index: usize| grid_key(pieces[index].polygon.area_mm2());
            area(*second)
                .cmp(&area(*first))
                .then_with(|| pieces[*first].id.cmp(pieces[*second].id))
        });
        let mut failed = false;
        if separation {
            failed = !overlap_mediated_reinsert(
                pieces,
                work_settings,
                &hints,
                &mut state,
                &removed,
                &hazard_catalog,
                round,
                lns_seed,
                &mut recon,
                &mut lns,
                work,
            )?;
        } else {
            for (slot, piece_index) in reinsert_order.into_iter().enumerate() {
                let mut screen = JaguaHazardIndex::from_catalog_active(
                    pieces,
                    work_settings,
                    work_settings.sheet_long_axis_mm,
                    &state.placements.iter().map(hazard_pose).collect::<Vec<_>>(),
                    &state.active,
                    &hazard_catalog,
                )
                .map_err(|error| format!("lns hazard screen index: {error}"))?;
                let placed = reconstruct_insert_piece(
                    pieces,
                    work_settings,
                    &hints,
                    &mut state,
                    lns_seed,
                    200 + round * 32 + slot,
                    piece_index,
                    true,
                    Some(&mut screen),
                    &mut recon,
                    work,
                )?;
                if placed {
                    lns.reinsertions += 1;
                } else {
                    failed = true;
                    lns.reinsert_failures += 1;
                    break;
                }
            }
        }
        if failed {
            state = snapshot;
            lns.rounds_reverted += 1;
            continue;
        }
        // Endpoint optimizer: steepest-descent re-placement of the lifted
        // pieces under the full acceptance key. Each pass removes one lifted
        // piece, evaluates its top candidate poses by the complete key, and
        // keeps the best strictly improving pose; passes repeat until no
        // piece improves or the cycle budget is exhausted.
        if vacancy_transport {
            for _cycle in 0..OPTIMIZER_CYCLES {
                let mut any_improved = false;
                for lifted in &removed {
                    let index = *lifted;
                    if !state.active[index] {
                        continue;
                    }
                    let entry = depth_key(&state);
                    let saved_placement = state.placements[index].clone();
                    let saved_collision = state.collisions[index].clone();
                    state.active[index] = false;
                    state.collisions[index] = None;
                    let mut screen = JaguaHazardIndex::from_catalog_active(
                        pieces,
                        work_settings,
                        work_settings.sheet_long_axis_mm,
                        &state.placements.iter().map(hazard_pose).collect::<Vec<_>>(),
                        &state.active,
                        &hazard_catalog,
                    )
                    .map_err(|error| format!("optimizer screen index: {error}"))?;
                    let mut best_pose: Option<((i64, i128, i128), RelaxedPlacement, PolygonSet)> =
                        None;
                    for attempt in 0..OPTIMIZER_CANDIDATES_PER_PIECE {
                        let placed = reconstruct_insert_piece(
                            pieces,
                            work_settings,
                            &hints,
                            &mut state,
                            lns_seed,
                            1_000 + round * 64 + attempt * 8,
                            index,
                            true,
                            Some(&mut screen),
                            &mut recon,
                            work,
                        )?;
                        if !placed {
                            break;
                        }
                        let key = depth_key(&state);
                        let placement = state.placements[index].clone();
                        let collision = state.collisions[index]
                            .clone()
                            .ok_or_else(|| "optimizer missing collision".to_owned())?;
                        if best_pose
                            .as_ref()
                            .is_none_or(|(best_key, _, _)| key < *best_key)
                        {
                            best_pose = Some((key, placement, Arc::unwrap_or_clone(collision)));
                        }
                        state.active[index] = false;
                        state.collisions[index] = None;
                    }
                    match best_pose {
                        Some((key, placement, collision)) if key < entry => {
                            state.placements[index] = placement;
                            state.collisions[index] = Some(Arc::new(collision));
                            state.active[index] = true;
                            any_improved = true;
                            lns.optimizer_improvements =
                                lns.optimizer_improvements.saturating_add(1);
                        }
                        _ => {
                            state.placements[index] = saved_placement;
                            state.collisions[index] = saved_collision;
                            state.active[index] = true;
                        }
                    }
                }
                if !any_improved {
                    break;
                }
            }
        }
        // Post-endpoint settle: shelved and separated pieces drop into the
        // voids the rearrangement drained toward the top-connected region
        // before the acceptance key is measured; without this, every shelf
        // landing reads as a frontier regression and vacancy transport is
        // invisible to the key.
        settle_sweep(
            &mut state,
            pieces,
            work_settings,
            inset,
            true,
            &mut settle,
            work,
        )?;
        let endpoint_key = depth_key(&state);
        if endpoint_key < best_key {
            best_state = state.clone();
            best_key = endpoint_key;
        }
        let tolerance = LNS_TOLERANCE_GRID[round];
        let frontier_tolerance = LNS_FRONTIER_TOLERANCE_GRID[round];
        // Trapped-void wander tolerance: up to 50 cells (about 200 mm2 at
        // the 2 mm raster) of transient void regression per tolerant round.
        let void_tolerance: i128 = if LNS_TOLERANCE_GRID[round] > 0 { 50 } else { 0 };
        let within_tolerance = endpoint_key.0 <= entry_key.0.saturating_add(frontier_tolerance)
            && endpoint_key.1 <= entry_key.1.saturating_add(void_tolerance)
            && endpoint_key.2 <= entry_key.2.saturating_add(tolerance);
        let fresh = visited.insert(state_fingerprint(&state, pieces));
        if endpoint_key < entry_key && fresh {
            lns.rounds_accepted += 1;
        } else if within_tolerance && fresh {
            lns.rounds_wandered = lns.rounds_wandered.saturating_add(1);
        } else {
            state = snapshot;
            lns.rounds_reverted += 1;
        }
    }
    state = best_state;
    settle.frontier_after_grid = frontier(&state);
    lns.frontier_after_grid = settle.frontier_after_grid;
    diagnostics.settle = Some(settle);
    diagnostics.lns = Some(lns);
    if recon.insertions > 0 || recon.exact_rows > 0 {
        diagnostics.reconstruction = Some(recon.clone());
    }
    // Exact re-anchor before the state is trusted, mirroring mode 14.
    for index in 0..pieces.len() {
        let rebuilt =
            build_collision(pieces[index], &state.placements[index], work_settings, work)?;
        if !rebuilt.fits_rect(
            inset,
            inset,
            work_settings.sheet_short_axis_mm - inset,
            work_settings.sheet_long_axis_mm - inset,
        ) {
            return Err(format!(
                "lns re-anchor: piece {} escaped containment",
                pieces[index].id
            ));
        }
        state.collisions[index] = Some(Arc::new(rebuilt));
    }
    for first in 0..pieces.len() {
        for second in (first + 1)..pieces.len() {
            work.charge_experimental_pair()?;
            let a = state.collisions[first]
                .as_ref()
                .ok_or_else(|| "lns re-anchor missing collision".to_owned())?;
            let b = state.collisions[second]
                .as_ref()
                .ok_or_else(|| "lns re-anchor missing collision".to_owned())?;
            if exact_intersection_area(a, b, work)? > 0.0 {
                return Err(format!(
                    "lns re-anchor: pieces {} and {} overlap",
                    pieces[first].id, pieces[second].id
                ));
            }
        }
    }
    Ok(RelaxedState {
        placements: state.placements,
        strip_depth_mm: baseline.strip_depth_mm,
    })
}

/// Mode-16 overlap-mediated reinsertion: removed pieces return at their old
/// poses with overlaps permitted, then a bounded deterministic descent moves
/// one overlapping soft piece at a time along the compass ladder, accepting
/// only strict decreases of the grid-quantized total exact overlap area.
/// Returns true only when total overlap reaches exactly zero, so every
/// competing endpoint is exact-valid; a nonzero residual reports failure and
/// the caller reverts the round snapshot.
#[allow(clippy::too_many_arguments)]
fn overlap_mediated_reinsert(
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    hints: &RelaxedState,
    state: &mut VacancyState,
    removed: &[usize],
    hazard_catalog: &Arc<JaguaHazardCatalog>,
    round: usize,
    lns_seed: u64,
    recon: &mut GeneralPersistentVacancyReconstructionDiagnostics,
    lns: &mut GeneralPersistentVacancyLnsDiagnostics,
    work: &mut RunWork,
) -> Result<bool, String> {
    const SEPARATION_DIRECTIONS: [(f64, f64); 8] = [
        (0.0, -1.0),
        (0.0, 1.0),
        (-1.0, 0.0),
        (1.0, 0.0),
        (0.7071067811865476, 0.7071067811865476),
        (-0.7071067811865476, 0.7071067811865476),
        (0.7071067811865476, -0.7071067811865476),
        (-0.7071067811865476, -0.7071067811865476),
    ];
    const SEPARATION_RADII_MM: [f64; 12] = [
        0.256, 0.512, 1.024, 2.048, 3.072, 4.096, 6.144, 8.192, 12.288, 16.384, 24.576, 32.768,
    ];
    let inset = collision_sheet_inset_mm(settings);
    // Soft pieces return at their hint poses, overlaps permitted.
    for index in removed {
        let placement = hints.placements[*index].clone();
        let collision = build_collision(pieces[*index], &placement, settings, work)?;
        state.placements[*index] = placement;
        state.active[*index] = true;
        state.collisions[*index] = Some(Arc::new(collision));
        lns.reinsertions += 1;
    }
    let quantized = |area: f64| -> i128 { (area * 1_000_000.0).round() as i128 };
    // Guided pair weights: pairs that stay overlapping when the descent has
    // no strictly improving move get their weight incremented, so later
    // moves may trade a low-weight overlap increase for a high-weight
    // decrease and cross ridges the unweighted objective cannot. Overlap
    // zero remains the only publication condition and is weight-independent.
    let mut pair_weights: BTreeMap<(usize, usize), i128> = BTreeMap::new();
    let weight_of = |weights: &BTreeMap<(usize, usize), i128>, a: usize, b: usize| -> i128 {
        let key = if a < b { (a, b) } else { (b, a) };
        *weights.get(&key).unwrap_or(&1)
    };
    let piece_overlap = |state: &VacancyState,
                         index: usize,
                         collision: &PolygonSet,
                         weights: &BTreeMap<(usize, usize), i128>,
                         work: &mut RunWork|
     -> Result<(i128, i128), String> {
        let mut weighted = 0i128;
        let mut raw = 0i128;
        for other in 0..pieces.len() {
            if other == index || !state.active[other] {
                continue;
            }
            work.charge_experimental_pair()?;
            let fixed = state.collisions[other]
                .as_ref()
                .ok_or_else(|| "separation missing collision".to_owned())?;
            let overlap = quantized(exact_intersection_area(collision, fixed, work)?);
            raw += overlap;
            weighted += overlap.saturating_mul(weight_of(weights, index, other));
        }
        Ok((weighted, raw))
    };
    let mut soft = removed.to_vec();
    soft.sort_by(|first, second| pieces[*first].id.cmp(pieces[*second].id));
    let mut relocations = 0usize;
    for _move in 0..SEPARATION_MOVES_PER_ROUND {
        let pair_moves_before = lns.separation_pair_moves;
        // Pick the soft piece with the largest current overlap, ties by ID.
        let mut worst: Option<(i128, usize)> = None;
        for index in &soft {
            let collision = state.collisions[*index]
                .as_ref()
                .ok_or_else(|| "separation missing soft collision".to_owned())?
                .clone();
            let (overlap, _raw) = piece_overlap(state, *index, &collision, &pair_weights, work)?;
            if overlap > 0 {
                let candidate = (overlap, *index);
                worst = Some(match worst {
                    None => candidate,
                    Some(current) => {
                        if candidate.0 > current.0
                            || (candidate.0 == current.0
                                && pieces[candidate.1].id < pieces[current.1].id)
                        {
                            candidate
                        } else {
                            current
                        }
                    }
                });
            }
        }
        let Some((current_overlap, index)) = worst else {
            lns.separation_zero_overlap = lns.separation_zero_overlap.saturating_add(1);
            return Ok(true);
        };
        // Best strict-improvement probe: rotational deltas first (they
        // resolve squeezed configurations translations cannot), then the
        // compass translation ladder, all under one probe budget.
        const SEPARATION_ROTATIONS_DEG: [f64; 8] = [-0.5, 0.5, -1.0, 1.0, -2.5, 2.5, -5.0, 5.0];
        let mut best: Option<(i128, RelaxedPlacement, PolygonSet)> = None;
        let mut probes = 0usize;
        let mut candidates_iter: Vec<(f64, f64, f64)> = SEPARATION_ROTATIONS_DEG
            .iter()
            .map(|delta| (0.0, 0.0, *delta))
            .collect();
        for radius in SEPARATION_RADII_MM {
            for (direction_x, direction_y) in SEPARATION_DIRECTIONS {
                candidates_iter.push((radius * direction_x, radius * direction_y, 0.0));
            }
        }
        'probe: for (offset_x, offset_y, rotation_delta) in candidates_iter {
            {
                if probes >= SEPARATION_PROBES_PER_MOVE {
                    break 'probe;
                }
                probes += 1;
                lns.separation_probes = lns.separation_probes.saturating_add(1);
                let mut candidate = state.placements[index].clone();
                candidate.translate_x += offset_x;
                candidate.translate_y += offset_y;
                candidate.rotation_deg += rotation_delta;
                let collision = build_collision(pieces[index], &candidate, settings, work)?;
                if !collision.fits_rect(
                    inset,
                    inset,
                    settings.sheet_short_axis_mm - inset,
                    settings.sheet_long_axis_mm - inset,
                ) {
                    continue;
                }
                let (overlap, _raw) = piece_overlap(state, index, &collision, &pair_weights, work)?;
                if overlap < current_overlap
                    && best
                        .as_ref()
                        .is_none_or(|(best_overlap, _, _)| overlap < *best_overlap)
                {
                    best = Some((overlap, candidate, collision));
                    if overlap == 0 {
                        break 'probe;
                    }
                }
            }
        }
        // Coordinated pair fallback: before recruiting, try moving the stuck
        // piece and its worst-overlap partner simultaneously in opposite
        // directions along their centroid axis - the move that resolves two
        // pieces squeezed between anchors, which no unilateral probe can.
        let mut best = best;
        if best.is_none() {
            let stuck_collision = state.collisions[index]
                .as_ref()
                .ok_or_else(|| "separation missing stuck collision".to_owned())?
                .clone();
            let mut worst_partner: Option<(i128, usize)> = None;
            for other in 0..pieces.len() {
                if other == index || !state.active[other] {
                    continue;
                }
                work.charge_experimental_pair()?;
                let fixed = state.collisions[other]
                    .as_ref()
                    .ok_or_else(|| "separation missing partner collision".to_owned())?;
                let overlap = quantized(exact_intersection_area(&stuck_collision, fixed, work)?);
                if overlap > 0 {
                    let candidate = (overlap, other);
                    worst_partner = Some(match worst_partner {
                        None => candidate,
                        Some(current) if candidate.0 > current.0 => candidate,
                        Some(current) => current,
                    });
                }
            }
            if let Some((_, partner)) = worst_partner {
                let center = |collision: &PolygonSet| {
                    collision.bounds().map(|bounds| {
                        (
                            (bounds.min_x + bounds.max_x) * 0.5,
                            (bounds.min_y + bounds.max_y) * 0.5,
                        )
                    })
                };
                let partner_collision = state.collisions[partner]
                    .as_ref()
                    .ok_or_else(|| "separation missing partner collision".to_owned())?
                    .clone();
                if let (Some(stuck_center), Some(partner_center)) =
                    (center(&stuck_collision), center(&partner_collision))
                {
                    let axis_x = stuck_center.0 - partner_center.0;
                    let axis_y = stuck_center.1 - partner_center.1;
                    let norm = (axis_x * axis_x + axis_y * axis_y).sqrt();
                    if norm > 1e-9 {
                        let unit = (axis_x / norm, axis_y / norm);
                        let pair_total = |a: &PolygonSet,
                                          b: &PolygonSet,
                                          work: &mut RunWork|
                         -> Result<i128, String> {
                            let mut total = 0i128;
                            for other in 0..pieces.len() {
                                if other == index || other == partner || !state.active[other] {
                                    continue;
                                }
                                work.charge_experimental_pair()?;
                                let fixed = state.collisions[other]
                                    .as_ref()
                                    .ok_or_else(|| "separation missing collision".to_owned())?;
                                total += quantized(exact_intersection_area(a, fixed, work)?);
                                work.charge_experimental_pair()?;
                                total += quantized(exact_intersection_area(b, fixed, work)?);
                            }
                            work.charge_experimental_pair()?;
                            total += quantized(exact_intersection_area(a, b, work)?);
                            Ok(total)
                        };
                        let entry_pair_total =
                            pair_total(&stuck_collision, &partner_collision, work)?;
                        'pair: for radius in SEPARATION_RADII_MM {
                            if probes >= SEPARATION_PROBES_PER_MOVE {
                                break;
                            }
                            probes += 1;
                            lns.separation_probes = lns.separation_probes.saturating_add(1);
                            let half = radius * 0.5;
                            let mut moved_a = state.placements[index].clone();
                            moved_a.translate_x += unit.0 * half;
                            moved_a.translate_y += unit.1 * half;
                            let mut moved_b = state.placements[partner].clone();
                            moved_b.translate_x -= unit.0 * half;
                            moved_b.translate_y -= unit.1 * half;
                            let collision_a =
                                build_collision(pieces[index], &moved_a, settings, work)?;
                            let collision_b =
                                build_collision(pieces[partner], &moved_b, settings, work)?;
                            let bounds_ok = |collision: &PolygonSet| {
                                collision.fits_rect(
                                    inset,
                                    inset,
                                    settings.sheet_short_axis_mm - inset,
                                    settings.sheet_long_axis_mm - inset,
                                )
                            };
                            if !bounds_ok(&collision_a) || !bounds_ok(&collision_b) {
                                continue;
                            }
                            let total = pair_total(&collision_a, &collision_b, work)?;
                            if total < entry_pair_total {
                                state.placements[index] = moved_a;
                                state.collisions[index] = Some(Arc::new(collision_a));
                                state.placements[partner] = moved_b;
                                state.collisions[partner] = Some(Arc::new(collision_b));
                                if !soft.contains(&partner) {
                                    soft.push(partner);
                                    soft.sort_by(|first, second| {
                                        pieces[*first].id.cmp(pieces[*second].id)
                                    });
                                }
                                lns.separation_pair_moves =
                                    lns.separation_pair_moves.saturating_add(1);
                                lns.separation_moves = lns.separation_moves.saturating_add(1);
                                best = None;
                                break 'pair;
                            }
                        }
                        if lns.separation_pair_moves > 0 {
                            // A committed pair move restarts the outer loop.
                        }
                    }
                }
            }
        }
        let committed_pair_move_this_iteration = false;
        let _ = committed_pair_move_this_iteration;
        let Some((_, placement, collision)) = best else {
            // A pair move may have just been committed; if so, resume the
            // outer descent from the updated state instead of recruiting.
            if lns.separation_pair_moves > pair_moves_before {
                continue;
            }
            // No strict soft-piece improvement anywhere on the ladder: recruit
            // the anchor contributing the largest exact overlap against the
            // stuck piece into the soft set (bilateral separation). If it is
            // already soft, the configuration is genuinely stuck.
            let stuck_collision = state.collisions[index]
                .as_ref()
                .ok_or_else(|| "separation missing stuck collision".to_owned())?
                .clone();
            let mut worst_anchor: Option<(i128, usize)> = None;
            for other in 0..pieces.len() {
                if other == index || !state.active[other] || soft.contains(&other) {
                    continue;
                }
                work.charge_experimental_pair()?;
                let fixed = state.collisions[other]
                    .as_ref()
                    .ok_or_else(|| "separation missing anchor collision".to_owned())?;
                let overlap = quantized(exact_intersection_area(&stuck_collision, fixed, work)?);
                if overlap > 0 {
                    let candidate = (overlap, other);
                    worst_anchor = Some(match worst_anchor {
                        None => candidate,
                        Some(current) => {
                            if candidate.0 > current.0
                                || (candidate.0 == current.0
                                    && pieces[candidate.1].id < pieces[current.1].id)
                            {
                                candidate
                            } else {
                                current
                            }
                        }
                    });
                }
            }
            // Global relocation escape: deactivate the stuck piece and
            // reinsert it anywhere through the depth-ranked, hazard-screened
            // generator. A successful relocation zeroes that piece's overlap
            // without touching any other pair, so total raw overlap strictly
            // decreases and the descent resumes with global progress.
            if relocations < SEPARATION_RELOCATIONS_PER_ROUND {
                let saved_placement = state.placements[index].clone();
                let saved_collision = state.collisions[index].clone();
                state.active[index] = false;
                state.collisions[index] = None;
                let mut screen = JaguaHazardIndex::from_catalog_active(
                    pieces,
                    settings,
                    settings.sheet_long_axis_mm,
                    &state.placements.iter().map(hazard_pose).collect::<Vec<_>>(),
                    &state.active,
                    hazard_catalog,
                )
                .map_err(|error| format!("separation relocation index: {error}"))?;
                let placed = reconstruct_insert_piece(
                    pieces,
                    settings,
                    hints,
                    state,
                    lns_seed,
                    400 + round * SEPARATION_RELOCATIONS_PER_ROUND + relocations,
                    index,
                    true,
                    Some(&mut screen),
                    recon,
                    work,
                )?;
                relocations += 1;
                if placed {
                    lns.separation_relocations = lns.separation_relocations.saturating_add(1);
                    continue;
                }
                state.placements[index] = saved_placement;
                state.collisions[index] = saved_collision;
                state.active[index] = true;
            }
            let Some((_, recruit)) = worst_anchor else {
                // Weight escalation: no anchor to recruit; increment the
                // weights of every currently overlapping pair touching the
                // stuck piece and retry, allowing ridge-crossing trades. Cap
                // escalations through the shared move budget.
                let stuck = state.collisions[index]
                    .as_ref()
                    .ok_or_else(|| "separation missing stuck collision".to_owned())?
                    .clone();
                let mut bumped = false;
                for other in 0..pieces.len() {
                    if other == index || !state.active[other] {
                        continue;
                    }
                    work.charge_experimental_pair()?;
                    let fixed = state.collisions[other]
                        .as_ref()
                        .ok_or_else(|| "separation missing collision".to_owned())?;
                    if exact_intersection_area(&stuck, fixed, work)? > 0.0 {
                        let key = if index < other {
                            (index, other)
                        } else {
                            (other, index)
                        };
                        *pair_weights.entry(key).or_insert(1) += 1;
                        bumped = true;
                    }
                }
                if bumped {
                    lns.separation_weight_bumps = lns.separation_weight_bumps.saturating_add(1);
                    if lns.separation_weight_bumps <= 40 {
                        continue;
                    }
                }
                return Ok(false);
            };
            soft.push(recruit);
            soft.sort_by(|first, second| pieces[*first].id.cmp(pieces[*second].id));
            lns.separation_recruits = lns.separation_recruits.saturating_add(1);
            continue;
        };
        state.placements[index] = placement;
        state.collisions[index] = Some(Arc::new(collision));
        lns.separation_moves = lns.separation_moves.saturating_add(1);
    }
    // Move budget exhausted; check the residual.
    for index in &soft {
        let collision = state.collisions[*index]
            .as_ref()
            .ok_or_else(|| "separation missing soft collision".to_owned())?
            .clone();
        let (_weighted, raw) = piece_overlap(state, *index, &collision, &pair_weights, work)?;
        if raw > 0 {
            return Ok(false);
        }
    }
    lns.separation_zero_overlap = lns.separation_zero_overlap.saturating_add(1);
    Ok(true)
}

/// Deterministic trapped-void raster for the mode-17 vacancy-transport
/// acceptance signal. The strip up to the current frontier is rasterized at
/// a fixed cell size; a cell is free when its center lies inside no active
/// expanded collision, and free cells flood-fill four-connected from the
/// above-frontier band. The returned value counts free cells NOT connected
/// to that band - the trapped voids whose drainage upward is exactly the
/// slack routing the piece-centric keys cannot see. Guidance only: validity
/// still rests entirely on the exact gates.
fn trapped_void_cells(
    state: &VacancyState,
    settings: GeneralFastSettings,
    frontier_grid: i64,
) -> usize {
    let void_span = profiling::deep::start(Phase::VacancyProxyRank);
    const CELL_MM: f64 = 2.0;
    let width = settings.sheet_short_axis_mm;
    let depth = (frontier_grid as f64) / 1000.0 + 2.0 * CELL_MM;
    let columns = (width / CELL_MM).ceil() as usize;
    let rows = (depth / CELL_MM).ceil() as usize;
    if columns == 0 || rows == 0 {
        profiling::deep::finish(Phase::VacancyProxyRank, void_span);
        return 0;
    }
    let actives = state
        .collisions
        .iter()
        .enumerate()
        .filter(|(index, _)| state.active[*index])
        .filter_map(|(_, collision)| collision.as_ref())
        .map(|collision| (collision.bounds(), collision))
        .collect::<Vec<_>>();
    let mut free = vec![true; columns * rows];
    for row in 0..rows {
        let y = (row as f64 + 0.5) * CELL_MM;
        for column in 0..columns {
            let x = (column as f64 + 0.5) * CELL_MM;
            for (bounds, collision) in &actives {
                if let Some(bounds) = bounds {
                    if x < bounds.min_x || x > bounds.max_x || y < bounds.min_y || y > bounds.max_y
                    {
                        continue;
                    }
                }
                if !matches!(
                    collision.contains_point(IrregularPoint::new(x, y)),
                    PointInPolygonResult::IsOutside
                ) {
                    free[row * columns + column] = false;
                    break;
                }
            }
        }
    }
    // Flood-fill four-connected from the top row (the above-frontier band).
    let mut reachable = vec![false; columns * rows];
    let mut stack = Vec::new();
    let top = rows - 1;
    for column in 0..columns {
        let cell = top * columns + column;
        if free[cell] {
            reachable[cell] = true;
            stack.push(cell);
        }
    }
    while let Some(cell) = stack.pop() {
        let row = cell / columns;
        let column = cell % columns;
        let mut push = |candidate: usize| {
            if free[candidate] && !reachable[candidate] {
                reachable[candidate] = true;
                stack.push(candidate);
            }
        };
        if column > 0 {
            push(cell - 1);
        }
        if column + 1 < columns {
            push(cell + 1);
        }
        if row > 0 {
            push(cell - columns);
        }
        if row + 1 < rows {
            push(cell + columns);
        }
    }
    let trapped = free
        .iter()
        .zip(reachable.iter())
        .filter(|(is_free, is_reachable)| **is_free && !**is_reachable)
        .count();
    profiling::deep::finish(Phase::VacancyProxyRank, void_span);
    trapped
}

/// Mode-18 frontier-band feasibility diagnostic: for each of the deepest
/// FRONTIER_BAND_PIECES pieces, remove the piece and sweep a deterministic
/// lattice of candidate poses (all conflict-ruin orientations crossed with an
/// 8 mm translation lattice over the sub-frontier strip), hazard-screening
/// each pose and exactly confirming survivors, searching for ANY exact-valid
/// pose whose collision frontier lies strictly below the current global
/// frontier. The result converts the open search question into a measured
/// fact: either a sub-frontier pose exists that the search misses, or the
/// incumbent is certified one-piece locally optimal at this resolution.
fn frontier_band_feasibility(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    baseline: RelaxedState,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
) -> Result<RelaxedState, String> {
    const FRONTIER_BAND_PIECES: usize = 5;
    const LATTICE_MM: f64 = 8.0;
    let settings = fast_settings;
    let mut state = VacancyState {
        collisions: baseline
            .placements
            .iter()
            .enumerate()
            .map(|(index, placement)| {
                build_collision(pieces[index], placement, settings, work)
                    .map(|collision| Some(Arc::new(collision)))
            })
            .collect::<Result<Vec<_>, _>>()?,
        placements: baseline.placements.clone(),
        active: vec![true; pieces.len()],
        last_transition: None,
    };
    let hazard_catalog = Arc::new(
        JaguaHazardCatalog::new(pieces, settings)
            .map_err(|error| format!("feasibility hazard catalog: {error}"))?,
    );
    let frontier_grid = state
        .collisions
        .iter()
        .flatten()
        .filter_map(|collision| collision.bounds())
        .map(|bounds| grid_key(bounds.max_y))
        .max()
        .unwrap_or(0);
    let mut by_depth = (0..pieces.len())
        .filter_map(|index| {
            state.collisions[index]
                .as_ref()
                .and_then(|collision| collision.bounds())
                .map(|bounds| (grid_key(bounds.max_y), index))
        })
        .collect::<Vec<_>>();
    by_depth.sort_by(|first, second| {
        second
            .0
            .cmp(&first.0)
            .then_with(|| pieces[first.1].id.cmp(pieces[second.1].id))
    });
    let inset = collision_sheet_inset_mm(settings);
    let seed = parent_seed_key(&state, pieces);
    let mut rows = Vec::new();
    for (piece_depth, index) in by_depth.into_iter().take(FRONTIER_BAND_PIECES) {
        let saved_placement = state.placements[index].clone();
        let saved_collision = state.collisions[index].clone();
        state.active[index] = false;
        state.collisions[index] = None;
        let mut screen = JaguaHazardIndex::from_catalog_active(
            pieces,
            settings,
            settings.sheet_long_axis_mm,
            &state.placements.iter().map(hazard_pose).collect::<Vec<_>>(),
            &state.active,
            &hazard_catalog,
        )
        .map_err(|error| format!("feasibility screen index: {error}"))?;
        let orientations = conflict_ruin_orientations(
            pieces[index],
            &saved_placement,
            derive_seed(seed, 0, index),
        );
        let mut screened = 0usize;
        let mut confirmed = 0usize;
        let mut best_sub_frontier: Option<(i64, RelaxedPlacement)> = None;
        for (rotation_deg, mirrored) in orientations {
            let orientation = RelaxedPlacement {
                input_index: index,
                rotation_deg,
                mirrored,
                translate_x: 0.0,
                translate_y: 0.0,
            };
            let local = build_collision(pieces[index], &orientation, settings, work)?;
            let Some(local_bounds) = local.bounds() else {
                continue;
            };
            let min_x = inset - local_bounds.min_x;
            let max_x = settings.sheet_short_axis_mm - inset - local_bounds.max_x;
            let min_y = inset - local_bounds.min_y;
            // The pose frontier must land strictly below the global frontier.
            let max_y = (frontier_grid as f64) / 1000.0 - 0.001 - local_bounds.max_y;
            if min_x > max_x || min_y > max_y {
                continue;
            }
            let steps_x = ((max_x - min_x) / LATTICE_MM).floor() as usize + 1;
            let steps_y = ((max_y - min_y) / LATTICE_MM).floor() as usize + 1;
            for step_y in 0..steps_y {
                for step_x in 0..steps_x {
                    let candidate = RelaxedPlacement {
                        input_index: index,
                        rotation_deg,
                        mirrored,
                        translate_x: min_x + step_x as f64 * LATTICE_MM,
                        translate_y: min_y + step_y as f64 * LATTICE_MM,
                    };
                    screened += 1;
                    work.diagnostics.hazard_queries =
                        work.diagnostics.hazard_queries.saturating_add(1);
                    if work.diagnostics.hazard_queries > work.quotas.max_hazard_queries {
                        return Err(work.cap("hazard-query budget exhausted"));
                    }
                    match screen.query_unplaced(index, hazard_pose(&candidate)) {
                        Ok(GeneralHazardQuery::Complete {
                            boundary,
                            colliding_piece_ids,
                        }) => {
                            if boundary || !colliding_piece_ids.is_empty() {
                                continue;
                            }
                        }
                        Ok(_) => {}
                        Err(error) if error.to_string().contains("query envelope") => continue,
                        Err(error) => {
                            return Err(format!("feasibility screen: {error}"));
                        }
                    }
                    work.diagnostics.exact_finalist_rows =
                        work.diagnostics.exact_finalist_rows.saturating_add(1);
                    if work.diagnostics.exact_finalist_rows > work.quotas.max_exact_finalist_rows {
                        return Err(work.cap("exact-finalist row budget exhausted"));
                    }
                    let collision = build_collision(pieces[index], &candidate, settings, work)?;
                    if !collision.fits_rect(
                        inset,
                        inset,
                        settings.sheet_short_axis_mm - inset,
                        settings.sheet_long_axis_mm - inset,
                    ) {
                        continue;
                    }
                    let Some(bounds) = collision.bounds() else {
                        continue;
                    };
                    let pose_frontier = grid_key(bounds.max_y);
                    if pose_frontier >= frontier_grid {
                        continue;
                    }
                    let mut overlapping = false;
                    for other in 0..pieces.len() {
                        if other == index || !state.active[other] {
                            continue;
                        }
                        work.charge_experimental_pair()?;
                        let fixed = state.collisions[other]
                            .as_ref()
                            .ok_or_else(|| "feasibility missing collision".to_owned())?;
                        if exact_intersection_area(&collision, fixed, work)? > 0.0 {
                            overlapping = true;
                            break;
                        }
                    }
                    if overlapping {
                        continue;
                    }
                    confirmed += 1;
                    if best_sub_frontier
                        .as_ref()
                        .is_none_or(|(best, _)| pose_frontier < *best)
                    {
                        best_sub_frontier = Some((pose_frontier, candidate.clone()));
                    }
                }
            }
        }
        rows.push(GeneralPersistentVacancyFeasibilityRow {
            piece_id: pieces[index].id.to_owned(),
            piece_frontier_grid: piece_depth,
            lattice_poses_screened: screened,
            exact_valid_sub_frontier_poses: confirmed,
            best_sub_frontier_grid: best_sub_frontier.as_ref().map(|(depth, _)| *depth),
        });
        state.placements[index] = saved_placement;
        state.collisions[index] = saved_collision;
        state.active[index] = true;
    }
    diagnostics.frontier_feasibility = Some(rows);
    Ok(baseline)
}

fn settle_sweep(
    state: &mut VacancyState,
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    inset: f64,
    diagonal: bool,
    settle: &mut GeneralPersistentVacancySettleDiagnostics,
    work: &mut RunWork,
) -> Result<(), String> {
    let mut order = (0..pieces.len()).collect::<Vec<_>>();
    order.sort_by_key(|index| {
        let min_y = state.collisions[*index]
            .as_ref()
            .and_then(|collision| collision.bounds())
            .map(|bounds| grid_key(bounds.min_y))
            .unwrap_or(i64::MAX);
        (min_y, pieces[*index].id)
    });
    for piece_index in order {
        settle.attempts += 1;
        work.diagnostics.selected_piece_slots =
            work.diagnostics.selected_piece_slots.saturating_add(1);
        if work.diagnostics.selected_piece_slots > work.quotas.max_selected_piece_slots {
            return Err(work.cap("selected-piece slot budget exhausted"));
        }
        work.charge_source_features(pieces[piece_index].polygon.vertex_count().saturating_mul(2))?;
        work.diagnostics.orientation_streams =
            work.diagnostics.orientation_streams.saturating_add(1);
        if work.diagnostics.orientation_streams > work.quotas.max_orientation_streams {
            return Err(work.cap("orientation-stream budget exhausted"));
        }
        let mut temp = state.clone();
        temp.active[piece_index] = false;
        temp.collisions[piece_index] = None;
        // FIX 3: the settle phase owns a full collision state plus one
        // temporary clone per attempt; charge that live set against the
        // retained-memory gate exactly like the population phases.
        let live_bytes = state_slice_bytes(std::slice::from_ref(state))
            .saturating_add(state_slice_bytes(std::slice::from_ref(&temp)))
            .saturating_add(2usize.saturating_mul(size_of::<VacancyState>()));
        work.diagnostics.total_retained_peak_bytes =
            work.diagnostics.total_retained_peak_bytes.max(live_bytes);
        if live_bytes > MAX_RETAINED_BYTES {
            return Err(work.cap("settle live-state memory budget exhausted"));
        }
        let mut best_placement = state.placements[piece_index].clone();
        let mut best_collision: Option<Arc<PolygonSet>> = None;
        let mut probes = 0usize;
        // Every accepted probe strictly lowers the piece, so the compaction
        // is monotone. The plain settle keeps the single downward ladder; a
        // purely lateral phase was tried and rejected. Diagonal settling
        // (mode 14) additionally probes the two 45-degree descent
        // directions, which can slide a piece along a slope that blocks the
        // straight drop; acceptance still requires strict descent, so no
        // lateral drift without depth progress is possible.
        const DESCENT_DIRECTIONS: [(f64, f64); 3] = [
            (0.0, -1.0),
            (-0.7071067811865476, -0.7071067811865476),
            (0.7071067811865476, -0.7071067811865476),
        ];
        let directions: &[(f64, f64)] = if diagonal {
            &DESCENT_DIRECTIONS
        } else {
            &DESCENT_DIRECTIONS[..1]
        };
        'ladder: for (direction_x, direction_y) in directions.iter().copied() {
            for step in SETTLE_STEP_LADDER_MM {
                loop {
                    if probes >= SETTLE_PROBES_PER_ATTEMPT {
                        break 'ladder;
                    }
                    let mut candidate = best_placement.clone();
                    candidate.translate_x += step * direction_x;
                    candidate.translate_y += step * direction_y;
                    probes += 1;
                    settle.exact_rows += 1;
                    work.diagnostics.exact_finalist_rows =
                        work.diagnostics.exact_finalist_rows.saturating_add(1);
                    if work.diagnostics.exact_finalist_rows > work.quotas.max_exact_finalist_rows {
                        return Err(work.cap("exact-finalist row budget exhausted"));
                    }
                    let collision =
                        build_collision(pieces[piece_index], &candidate, settings, work)?;
                    if !collision.fits_rect(
                        inset,
                        inset,
                        settings.sheet_short_axis_mm - inset,
                        settings.sheet_long_axis_mm - inset,
                    ) {
                        break;
                    }
                    let descended =
                        grid_key(candidate.translate_y) < grid_key(best_placement.translate_y);
                    if !descended {
                        break;
                    }
                    let mut overlapping = false;
                    for fixed_index in 0..pieces.len() {
                        if fixed_index == piece_index || !temp.active[fixed_index] {
                            continue;
                        }
                        work.charge_experimental_pair()?;
                        let fixed = temp.collisions[fixed_index].as_ref().ok_or_else(|| {
                            format!("active piece {fixed_index} has no collision")
                        })?;
                        if exact_intersection_area(&collision, fixed, work)? > 0.0 {
                            overlapping = true;
                            break;
                        }
                    }
                    if overlapping {
                        break;
                    }
                    best_placement = candidate;
                    best_collision = Some(Arc::new(collision));
                }
            }
        }
        if let Some(collision) = best_collision {
            state.placements[piece_index] = best_placement;
            state.collisions[piece_index] = Some(collision);
            settle.accepted_moves += 1;
        }
    }
    Ok(())
}

/// Mode-13 guided reconstruction: rebuilds the layout from an external hint
/// fixture under the engine's own exact contract. Pieces are inserted in
/// ascending hint-depth order; each insertion ranks displacement probes,
/// generator candidates, and upward shelf fallbacks by canonical-grid L1
/// distance from the hint pose and exact-confirms them in order until the
/// first pose with full-strip containment and zero exact pair intersection
/// against every already-placed piece. Pieces whose pockets are closed
/// during the first pass are deferred and retried after every other piece
/// has settled; the deferred pass completes every retry before failing so
/// the diagnostics record the true unplaced set. The hints are never
/// trusted: the completed state must pass the unchanged dual publication
/// audit. Like the rest of the engine, candidate generation quantizes
/// platform trigonometry onto the canonical grid, so replay identity is
/// promised only on the recorded machine/toolchain identity.
/// Mode-20 skyline beam constructor: builds complete exact-valid layouts
/// from scratch, using the pinned parent fixture only as a deterministic
/// seed anchor and per-piece orientation prior (mode-13-style: never
/// validated, never trusted). Each restart runs one seeded insertion order
/// through a beam of partial layouts; every expansion plants synthetic hints
/// at the deepest skyline valleys and exact-confirms candidates in
/// landing-frontier order through the unchanged collision machinery. Only
/// complete candidates that pass the unchanged dual publication gates under
/// the target settings may publish.
///
/// Mode 20 (`best_ever_parent = false`) and mode 25 (`best_ever_parent =
/// true`) share this constructor; see `CONSTRUCTION_BEST_EVER_PARENTS` for the
/// off-beam best-ever parent mechanism mode 25 adds.
#[allow(clippy::too_many_arguments)]
fn construct_skyline_beam(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    target_depth_mm: f64,
    anchor: &RelaxedState,
    best_ever_parent: bool,
    construction_salt: ConstructionSalt,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
) -> Result<(VacancyState, f64), String> {
    let mut construction = GeneralPersistentVacancyConstructionDiagnostics {
        restarts: CONSTRUCTION_RESTARTS,
        beam_width: CONSTRUCTION_BEAM_WIDTH,
        hint_stations_per_slot: CONSTRUCTION_HINT_STATIONS,
        rows_per_piece_cap: CONSTRUCTION_ROWS_PER_PIECE,
        finalists_per_slot: CONSTRUCTION_FINALISTS_PER_SLOT,
        best_ever_parent_enabled: best_ever_parent,
        ..GeneralPersistentVacancyConstructionDiagnostics::default()
    };
    let result = construct_skyline_beam_inner(
        pieces,
        fast_settings,
        target_depth_mm,
        anchor,
        best_ever_parent,
        construction_salt,
        diagnostics,
        &mut construction,
        work,
    );
    diagnostics.construction = Some(construction);
    result
}

#[allow(clippy::too_many_arguments)]
fn construct_skyline_beam_inner(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    target_depth_mm: f64,
    anchor: &RelaxedState,
    best_ever_parent: bool,
    construction_salt: ConstructionSalt,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    construction: &mut GeneralPersistentVacancyConstructionDiagnostics,
    work: &mut RunWork,
) -> Result<(VacancyState, f64), String> {
    // Construction inserts at the full-sheet settings so the upward shelf
    // escape always admits a pose; only the publication audit runs under the
    // target settings (the lift/resettle precedent).
    let work_settings = fast_settings;
    let target_settings = GeneralFastSettings {
        sheet_long_axis_mm: target_depth_mm,
        ..fast_settings
    };
    let anchor_state = VacancyState {
        placements: anchor.placements.clone(),
        active: vec![true; pieces.len()],
        collisions: vec![None; pieces.len()],
        last_transition: None,
    };
    let construction_seed = parent_seed_key(&anchor_state, pieces)
        ^ CONSTRUCTION_SEED_DOMAIN
        ^ (grid_key(target_depth_mm) as u64);
    const ORDER_NAMES: [&str; CONSTRUCTION_RESTARTS] = [
        "padded-bbox-area",
        "max-dimension",
        "semi-perimeter",
        "banded-area-shuffle",
        "height",
        "width",
        "vertex-count",
        "padded-bbox-area-reshuffled",
    ];
    // The trapped-void evaluator. Off the `fast-constructor-profile` profile
    // this is a zero-sized forwarder to `trapped_void_cells` and the
    // constructor behaves exactly as before; on it, occupancy is carried
    // incrementally down the beam. See `construction_void_grid`.
    let mut voids =
        ConstructionVoidCache::new(pieces, work_settings, construction_salt.void_cell_divisor);
    let mut best: Option<(i64, usize, VacancyState, f64)> = None;
    for restart in construction_salt.restarts() {
        // One trace scope per construction restart. The restarts are the
        // constructor's own basin generators - eight different insertion
        // orders over the same pieces - so this is the granularity at which
        // "which operator produced the layout" has an answer for mode 20.
        #[cfg(feature = "quality-trace")]
        let _trace_restart = crate::quality_trace::scope(
            format!("mode20.restart{restart}.{}", ORDER_NAMES[restart]),
            construction_seed,
            None,
        );
        let order_seed = derive_seed(construction_seed, restart, 0);
        let order = construction_order(pieces, work_settings, restart, order_seed)?;
        let mut row = GeneralPersistentVacancyConstructionRestartRow {
            order: ORDER_NAMES[restart].to_owned(),
            ..GeneralPersistentVacancyConstructionRestartRow::default()
        };
        let mut beam = vec![VacancyState {
            placements: anchor.placements.clone(),
            active: vec![false; pieces.len()],
            collisions: vec![None; pieces.len()],
            last_transition: None,
        }];
        // Off-beam best-ever expansion parent (mode 25 only): a full sidecar
        // copy of the pool elite of the previous layer, kept live as an extra
        // expansion parent whenever the retention step did not keep it.
        let mut sidecar: Option<VacancyState> = None;
        let mut starved = None;
        for (rank, piece_index) in order.iter().copied().enumerate() {
            let mut children: Vec<(ConstructionChildKey, usize, VacancyState)> = Vec::new();
            let mut children_bytes = 0usize;
            let mut seen_children = BTreeSet::new();
            // The sidecar earns its extra expansion only while it is absent
            // from the beam: an elite the beam already carries is expanded by
            // its own slot, and reserving a beam slot for an elite is the
            // measured-negative variant this mechanism deliberately avoids.
            let sidecar_parent = sidecar.as_ref().filter(|elite| {
                !beam
                    .iter()
                    .any(|retained| same_state_identity(retained, elite))
            });
            let beam_slots = beam.len();
            let parent_slots = beam_slots.saturating_add(usize::from(sidecar_parent.is_some()));
            for slot in 0..parent_slots {
                let (parent_state, ordinal) = match sidecar_parent {
                    Some(elite) if slot == beam_slots => {
                        (elite, best_ever_parent_ordinal(pieces.len(), restart, rank))
                    }
                    _ => (
                        &beam[slot],
                        (restart * pieces.len() + rank) * CONSTRUCTION_BEAM_WIDTH + slot,
                    ),
                };
                let live_bytes = state_slice_bytes(&beam)
                    .saturating_add(sidecar.as_ref().map_or(0, state_heap_bytes))
                    .saturating_add(children_bytes)
                    .saturating_add(2usize.saturating_mul(size_of::<VacancyState>()))
                    .saturating_add(CONSTRUCTION_TRANSIENT_BYTES)
                    .saturating_add(voids.retained_bytes());
                work.diagnostics.total_retained_peak_bytes =
                    work.diagnostics.total_retained_peak_bytes.max(live_bytes);
                if live_bytes > MAX_RETAINED_BYTES {
                    return Err(work.cap("construction live-state memory budget exhausted"));
                }
                if slot == beam_slots {
                    construction.best_ever_parent_expansions =
                        construction.best_ever_parent_expansions.saturating_add(1);
                    row.best_ever_parent_layers = row.best_ever_parent_layers.saturating_add(1);
                }
                // Every child of this slot is this parent plus one piece, so
                // the evaluator resolves the parent's occupancy once here and
                // each child costs one piece's raster. No-op off the profile.
                voids.begin_parent(parent_state);
                // A from-scratch construction has no vacated pose to seed from:
                // its anchor is a pose *prior*, not a pocket a piece was just
                // lifted out of. Skyline stations only, bit-identically.
                let finalists = construct_candidate_poses(
                    pieces,
                    work_settings,
                    anchor,
                    parent_state,
                    construction_seed,
                    ordinal,
                    piece_index,
                    &AnchorLocalSeeding::disabled(),
                    construction,
                    work,
                )?;
                for (candidate, collision, provenance) in finalists {
                    let mut child = parent_state.clone();
                    child.placements[piece_index] = candidate;
                    child.active[piece_index] = true;
                    child.collisions[piece_index] = Some(collision);
                    child.last_transition = None;
                    construction.children_generated =
                        construction.children_generated.saturating_add(1);
                    if provenance.zero_prior {
                        construction.zero_prior_finalists =
                            construction.zero_prior_finalists.saturating_add(1);
                    } else {
                        construction.fixture_prior_finalists =
                            construction.fixture_prior_finalists.saturating_add(1);
                    }
                    let identity = state_identity(&child);
                    if !seen_children.insert(identity) {
                        construction.children_deduplicated =
                            construction.children_deduplicated.saturating_add(1);
                        continue;
                    }
                    let key = construction_child_key(
                        &child,
                        work_settings,
                        construction,
                        piece_index,
                        &mut voids,
                    );
                    children_bytes = children_bytes.saturating_add(state_heap_bytes(&child));
                    children.push((key, slot, child));
                }
            }
            if children.is_empty() {
                starved = Some(format!(
                    "no exact-valid children at rank {rank} for piece {}",
                    pieces[piece_index].id
                ));
                break;
            }
            children.sort_by(|first, second| first.0.cmp(&second.0).then(first.1.cmp(&second.1)));
            // Sidecar refresh (mode 25): the strict minimum of this layer's
            // whole child pool - the sidecar's own children included - under
            // the elite comparator. Children are deduplicated by identity and
            // the identity anchors the comparator, so the minimum is unique
            // and the refresh is a strict comparator improvement over every
            // other partial the layer produced.
            let elite = best_ever_parent
                .then(|| {
                    children
                        .iter()
                        .min_by(|first, second| {
                            construction_elite_key(&first.0).cmp(&construction_elite_key(&second.0))
                        })
                        .map(|(_, _, child)| child.clone())
                })
                .flatten();
            // Diversity quota: at most CONSTRUCTION_BEAM_CHILDREN_PER_PARENT
            // survivors per parent, backfilled from the remaining children in
            // key order when the quota-constrained pool runs short.
            let mut per_parent = vec![0usize; parent_slots];
            let mut next = Vec::with_capacity(CONSTRUCTION_BEAM_WIDTH);
            let mut leftovers = Vec::new();
            for (_, slot, child) in children {
                if next.len() == CONSTRUCTION_BEAM_WIDTH {
                    break;
                }
                if per_parent[slot] < CONSTRUCTION_BEAM_CHILDREN_PER_PARENT {
                    per_parent[slot] += 1;
                    if slot == beam_slots {
                        construction.best_ever_parent_children_retained = construction
                            .best_ever_parent_children_retained
                            .saturating_add(1);
                    }
                    next.push(child);
                } else {
                    leftovers.push((slot, child));
                }
            }
            for (slot, child) in leftovers {
                if next.len() == CONSTRUCTION_BEAM_WIDTH {
                    break;
                }
                if slot == beam_slots {
                    construction.best_ever_parent_children_retained = construction
                        .best_ever_parent_children_retained
                        .saturating_add(1);
                }
                next.push(child);
            }
            beam = next;
            if best_ever_parent {
                sidecar = elite;
            }
        }
        if let Some(reason) = starved {
            row.rejection = Some(reason);
            construction.restart_rows.push(row);
            continue;
        }
        let candidate = beam
            .first()
            .ok_or_else(|| "construction beam emptied without starvation".to_owned())?;
        row.complete = candidate.active.iter().all(|active| *active);
        if !row.complete {
            row.rejection = Some("constructed state is not complete".to_owned());
            construction.restart_rows.push(row);
            continue;
        }
        construction.complete_candidates = construction.complete_candidates.saturating_add(1);
        let frontier_grid = candidate
            .collisions
            .iter()
            .enumerate()
            .filter(|(index, _)| candidate.active[*index])
            .filter_map(|(_, collision)| collision.as_ref())
            .filter_map(|collision| collision.bounds())
            .map(|bounds| grid_key(bounds.max_y))
            .max()
            .unwrap_or(0);
        row.frontier_grid = Some(frontier_grid);
        construction.void_scans = construction.void_scans.saturating_add(1);
        row.trapped_void_cells =
            Some(voids.state_trapped_cells(candidate, work_settings, frontier_grid));
        diagnostics.complete_states = diagnostics.complete_states.saturating_add(1);
        construction.audited_candidates = construction.audited_candidates.saturating_add(1);
        match audit_state(candidate, pieces, target_settings, true, work) {
            Err(reason) if reason.starts_with("cap: ") => return Err(reason),
            Err(reason) => {
                diagnostics.publication_rejections =
                    diagnostics.publication_rejections.saturating_add(1);
                row.rejection = Some(reason);
                construction.restart_rows.push(row);
                continue;
            }
            Ok(_) => {}
        }
        let placements = fast_placements(candidate, pieces, false);
        let independent = coupled_independent_source_depth(pieces, &placements, target_settings)
            .map_err(|error| format!("persistent vacancy constructed depth: {error}"))?;
        row.independent_depth_mm = Some(independent);
        construction.restart_rows.push(row);
        let key = (grid_key(independent), restart);
        if best
            .as_ref()
            .map(|(depth, ordinal, _, _)| key < (*depth, *ordinal))
            .unwrap_or(true)
        {
            best = Some((key.0, restart, candidate.clone(), independent));
        }
    }
    match best {
        Some((_, restart, state, independent)) => {
            construction.published_restart_ordinal = Some(restart);
            Ok((state, independent))
        }
        None => Err(
            "skyline construction produced no publishable layout within the target depth"
                .to_owned(),
        ),
    }
}

type ConstructionChildKey = (i64, usize, i64, i128, VacancyStateIdentity);

/// Child acceptance key: banded resulting frontier first (so the trapped-void
/// term stays active across frontier-raising commits inside the same band),
/// then the trapped-void flood-fill count, then the exact frontier, then the
/// summed per-piece frontiers (compactness), then the full placement identity
/// as the deterministic tie anchor.
fn construction_child_key(
    child: &VacancyState,
    settings: GeneralFastSettings,
    construction: &mut GeneralPersistentVacancyConstructionDiagnostics,
    inserted: usize,
    voids: &mut ConstructionVoidCache,
) -> ConstructionChildKey {
    let mut frontier_grid = 0i64;
    let mut frontier_sum = 0i128;
    for (index, collision) in child.collisions.iter().enumerate() {
        if !child.active[index] {
            continue;
        }
        if let Some(bounds) = collision.as_ref().and_then(|collision| collision.bounds()) {
            let piece_frontier = grid_key(bounds.max_y);
            frontier_grid = frontier_grid.max(piece_frontier);
            frontier_sum += i128::from(piece_frontier);
        }
    }
    construction.void_scans = construction.void_scans.saturating_add(1);
    let voids = voids.child_trapped_cells(child, settings, inserted, frontier_grid);
    (
        frontier_grid.div_euclid(CONSTRUCTION_FRONTIER_BAND_GRID),
        voids,
        frontier_grid,
        frontier_sum,
        state_identity(child),
    )
}

/// Elite comparator for the off-beam best-ever expansion parent: the same
/// terms as the retention key, reordered so the *exact* landing frontier leads
/// instead of its `CONSTRUCTION_FRONTIER_BAND_GRID` band. The banding is
/// deliberate - it keeps the trapped-void term live across frontier-raising
/// commits - but it is also exactly what lets a strictly shallower partial be
/// out-ranked and pruned. This comparator names that partial: the globally
/// best-ever state under the quantity the search actually minimizes (depth),
/// with voids, compactness and the identity anchor as the deterministic tail.
/// It is derived from the already-computed child key, so naming the elite
/// costs no additional geometry.
///
/// The comparator is only meaningful between states of the same construction
/// rank: the constructor's beam is rank-synchronous (every state at layer `r`
/// holds exactly the first `r` pieces of that restart's insertion order), a
/// partial from an earlier rank is missing pieces the later layers insert and
/// can never be a legal parent there, and every term of the key grows with the
/// number of placed pieces. "Best-ever" is therefore scoped to the frontier
/// rank and refreshed once per layer.
fn construction_elite_key(key: &ConstructionChildKey) -> (i64, usize, i128, &VacancyStateIdentity) {
    (key.2, key.1, key.3, &key.4)
}

/// Ordinal domain of the off-beam best-ever parent expansion. The ordinary
/// beam expansions consume `[0, CONSTRUCTION_RESTARTS * pieces *
/// CONSTRUCTION_BEAM_WIDTH)`; the one sidecar expansion of each
/// `(restart, rank)` is placed strictly above that range, so it never draws
/// the same seeded orientation and position streams as a beam slot and mode
/// 20's ordinals are left exactly as they are.
fn best_ever_parent_ordinal(piece_count: usize, restart: usize, rank: usize) -> usize {
    CONSTRUCTION_RESTARTS
        .saturating_mul(piece_count)
        .saturating_mul(CONSTRUCTION_BEAM_WIDTH)
        .saturating_add(restart.saturating_mul(piece_count).saturating_add(rank))
}

/// Seeded insertion-order portfolio: one deterministic descending key per
/// restart over uncharged source-polygon bounds, with seeded tie-noise that
/// permutes identical clones (and, for the banded restart, the whole band).
fn construction_order(
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    restart: usize,
    order_seed: u64,
) -> Result<Vec<usize>, String> {
    let pad = grid_key(settings.total_padding_mm).max(0) as i128;
    let mut dimensions = Vec::with_capacity(pieces.len());
    let mut max_area = 0i128;
    for (index, piece) in pieces.iter().enumerate() {
        let bounds = piece
            .polygon
            .bounds()
            .ok_or_else(|| format!("piece {} has empty geometry", piece.id))?;
        let width = grid_key(bounds.max_x - bounds.min_x).max(0) as i128;
        let height = grid_key(bounds.max_y - bounds.min_y).max(0) as i128;
        let padded_area = (width + pad) * (height + pad);
        max_area = max_area.max(padded_area);
        dimensions.push((index, width, height, padded_area));
    }
    let mut rows = Vec::with_capacity(pieces.len());
    for (index, width, height, padded_area) in dimensions {
        let primary = match restart {
            0 => padded_area,
            1 => width.max(height),
            2 => width + height,
            3 => (padded_area * 4) / (max_area + 1),
            4 => height,
            5 => width,
            // Interlock-carriers first: source vertex count is the cheap
            // deterministic proxy for non-convexity, so the stars reach the
            // floor while the drop-settle can still nest them into each
            // other.
            6 => pieces[index].polygon.vertex_count() as i128,
            // Same key as restart 0 under a different seeded tie-noise
            // permutation of the identical clones.
            _ => padded_area,
        };
        rows.push((primary, derive_seed(order_seed, 0, index), index));
    }
    rows.sort_by(|first, second| {
        second
            .0
            .cmp(&first.0)
            .then(first.1.cmp(&second.1))
            .then_with(|| pieces[first.2].id.cmp(pieces[second.2].id))
            .then(first.2.cmp(&second.2))
    });
    Ok(rows.into_iter().map(|(_, _, index)| index).collect())
}

/// Width-aware bounding-box skyline over CONSTRUCTION_SKYLINE_COLUMNS
/// columns: each station is the center of one of the
/// CONSTRUCTION_HINT_STATIONS lowest sliding windows wide enough for the
/// requesting piece (window top = max column top inside the window, ties by
/// window start, pairwise start spacing of at least 8 columns), paired with
/// that window's top. This is the classical lowest-fitting skyline
/// position: a station is only proposed where the piece actually fits
/// laterally. On an empty state it degenerates to the sheet floor.
fn skyline_hint_stations(
    state: &VacancyState,
    settings: GeneralFastSettings,
    required_width_mm: f64,
) -> Vec<(f64, f64)> {
    let inset = collision_sheet_inset_mm(settings);
    let usable = settings.sheet_short_axis_mm - 2.0 * inset;
    let column_width = usable / CONSTRUCTION_SKYLINE_COLUMNS as f64;
    let last_column = CONSTRUCTION_SKYLINE_COLUMNS - 1;
    let mut tops = vec![inset; CONSTRUCTION_SKYLINE_COLUMNS];
    let column_of = |x: f64| -> usize {
        (((x - inset) / column_width).floor().max(0.0) as usize).min(last_column)
    };
    for (index, collision) in state.collisions.iter().enumerate() {
        if !state.active[index] {
            continue;
        }
        let Some(collision) = collision.as_ref() else {
            continue;
        };
        // Real-polygon profile instead of the bounding box: every boundary
        // vertex raises its own column, and every edge raises the columns
        // whose centers it crosses at the interpolated height. The station
        // tops then sit on the true material profile, which is at or below
        // the box top everywhere - so the ranked candidates start closer to
        // any interlock pocket and the drop ladder finishes the descent.
        for region in collision.regions() {
            let points = region.outer.source_points();
            for index in 0..points.len() {
                let first = points[index];
                let second = points[(index + 1) % points.len()];
                let first_column = column_of(first.x);
                tops[first_column] = tops[first_column].max(first.y);
                let (low_x, high_x) = if first.x <= second.x {
                    (first.x, second.x)
                } else {
                    (second.x, first.x)
                };
                let low_column = column_of(low_x);
                let high_column = column_of(high_x);
                if high_column > low_column && (second.x - first.x).abs() > f64::EPSILON {
                    for column in low_column..=high_column {
                        let center = inset + (column as f64 + 0.5) * column_width;
                        if center < low_x || center > high_x {
                            continue;
                        }
                        let t = (center - first.x) / (second.x - first.x);
                        let y = first.y + t * (second.y - first.y);
                        tops[column] = tops[column].max(y);
                    }
                }
            }
        }
    }
    let window = ((required_width_mm / column_width).ceil().max(1.0) as usize)
        .min(CONSTRUCTION_SKYLINE_COLUMNS);
    let mut ranked = Vec::with_capacity(CONSTRUCTION_SKYLINE_COLUMNS - window + 1);
    for start in 0..=(CONSTRUCTION_SKYLINE_COLUMNS - window) {
        let top = tops[start..start + window]
            .iter()
            .fold(f64::MIN, |acc, value| acc.max(*value));
        ranked.push((grid_key(top), start));
    }
    ranked.sort();
    let mut stations = Vec::with_capacity(CONSTRUCTION_HINT_STATIONS);
    for (top_key, start) in ranked {
        if stations
            .iter()
            .any(|(existing, _)| start.abs_diff(*existing) < 8)
        {
            continue;
        }
        stations.push((start, (top_key as f64) / 1_000.0));
        if stations.len() == CONSTRUCTION_HINT_STATIONS {
            break;
        }
    }
    stations
        .into_iter()
        .map(|(start, top)| {
            (
                inset + (start as f64 + window as f64 * 0.5) * column_width,
                top,
            )
        })
        .collect()
}

/// Non-mutating expansion sibling of reconstruct_insert_piece: generates up
/// to CONSTRUCTION_FINALISTS_PER_SLOT exact-valid poses for one piece
/// against one beam parent. Candidates come from synthetic station hints
/// under both orientation priors (97-pose displacement cloud each), the full
/// orientation/position streams anchored at station zero, and the upward
/// shelf ladder; all are ranked by the landing-frontier key and confirmed at
/// the full-sheet settings.
///
/// When `anchor_local` is enabled the anchor pose is additionally read as a
/// *vacated* pose and seeded directly - see [`AnchorLocalSeeding`]. Those
/// candidates lead, in the order the primitive defines (the vacated pose, the
/// projection's trajectory, then the cloud by ascending displacement), because
/// the whole point is the interior pocket a top-frontier generator cannot
/// reach. They are otherwise ordinary candidates: the same charged
/// confirmation row, the same contact walk, the same finalist cap. Only their
/// row budget is separate, so that leading cannot cost the station stream the
/// rows it would have spent.
///
/// Returns (pose, collision, provenance).
#[allow(clippy::too_many_arguments)]
fn construct_candidate_poses(
    pieces: &[GeneralFastPiece<'_>],
    work_settings: GeneralFastSettings,
    anchor: &RelaxedState,
    parent: &VacancyState,
    construction_seed: u64,
    ordinal: usize,
    piece_index: usize,
    anchor_local: &AnchorLocalSeeding,
    construction: &mut GeneralPersistentVacancyConstructionDiagnostics,
    work: &mut RunWork,
) -> Result<Vec<(RelaxedPlacement, Arc<PolygonSet>, CandidateProvenance)>, String> {
    let proposal_span = profiling::deep::start(Phase::VacancyProposals);
    // The expansion parent is fixed for every confirmation row this call
    // generates, so its separation certificates are derived once here and
    // reused by all of them. A no-op off the profile.
    work.confirm_shields.begin_parent(&parent.collisions);
    work.reject_certificates.begin_parent(&parent.collisions);
    construction.slots = construction.slots.saturating_add(1);
    work.diagnostics.selected_piece_slots = work.diagnostics.selected_piece_slots.saturating_add(1);
    if work.diagnostics.selected_piece_slots > work.quotas.max_selected_piece_slots {
        return Err(work.cap("selected-piece slot budget exhausted"));
    }
    work.charge_source_features(pieces[piece_index].polygon.vertex_count().saturating_mul(2))?;
    let inset = collision_sheet_inset_mm(work_settings);
    let frontier_y = parent
        .collisions
        .iter()
        .enumerate()
        .filter(|(index, _)| parent.active[*index])
        .filter_map(|(_, collision)| collision.as_ref())
        .filter_map(|collision| collision.bounds())
        .map(|bounds| bounds.max_y)
        .fold(0.0f64, f64::max);
    let anchor_pose = &anchor.placements[piece_index];
    let mut priors = vec![(anchor_pose.rotation_deg, anchor_pose.mirrored)];
    if (angle_key(anchor_pose.rotation_deg), anchor_pose.mirrored) != (angle_key(0.0), false) {
        priors.push((0.0, false));
    }
    let mut candidates = Vec::new();
    let mut shelf_candidates = Vec::new();
    let mut anchor_local_candidates = Vec::new();
    let mut orientation_candidates: Vec<RelaxedPlacement> = Vec::new();
    let mut station_zero_hint: Option<RelaxedPlacement> = None;
    for (prior_index, (rotation_deg, mirrored)) in priors.iter().copied().enumerate() {
        let zero_prior = prior_index > 0;
        let prior_orientation = RelaxedPlacement {
            input_index: piece_index,
            rotation_deg,
            mirrored,
            translate_x: 0.0,
            translate_y: 0.0,
        };
        // One hint-orientation collision build per prior, funded by the
        // standalone CONSTRUCTION_HINT_PRIORS * CONSTRUCTION_SELECTED_PIECE_SLOTS
        // term of the experimental-build ceiling.
        let prior_local =
            build_collision(pieces[piece_index], &prior_orientation, work_settings, work)?;
        let prior_bounds = prior_local
            .bounds()
            .ok_or_else(|| "construction prior orientation has empty geometry".to_owned())?;
        let prior_center_x = (prior_bounds.min_x + prior_bounds.max_x) * 0.5;
        // Clamp every synthetic translation into the piece-feasible band so
        // a station near the sheet edge cannot strand a wide piece off the
        // strip (the vacancy position generator applies the same clamp to
        // its own baseline).
        let feasible_min_x = inset - prior_bounds.min_x;
        let feasible_max_x = work_settings.sheet_short_axis_mm - inset - prior_bounds.max_x;
        if feasible_min_x > feasible_max_x {
            continue;
        }
        // Anchor-local seeding. The anchor pose is a pose this piece actually
        // occupied, so under the anchor's own orientation prior it is the one
        // candidate whose feasibility is a matter of record rather than of
        // search - and it sits wherever the piece sat, interior pockets
        // included. Each cloud member gets its own bucket ordinal so the
        // coarse spatial de-duplication downstream, which is sized for
        // station hints, cannot collapse the fine end of the cloud.
        if anchor_local.enabled {
            let vacated = RelaxedPlacement {
                input_index: piece_index,
                rotation_deg,
                mirrored,
                translate_x: snap_mm(
                    anchor_pose
                        .translate_x
                        .clamp(feasible_min_x, feasible_max_x),
                ),
                translate_y: snap_mm(anchor_pose.translate_y),
            };
            let base_ordinal = ORIENTATIONS_PER_PIECE + CONSTRUCTION_HINT_PRIORS;
            anchor_local_candidates.push((
                base_ordinal + anchor_local_candidates.len(),
                zero_prior,
                vacated.clone(),
            ));
            if !zero_prior {
                // The projected poses: the vacated pose plus each translation
                // the single-piece separating projection passed through. These
                // are the anchor-local candidates that are *solved* rather
                // than sampled, so they go in right behind the vacated pose
                // itself.
                for (shift_x, shift_y) in anchor_local.projected_displacements(piece_index) {
                    let projected = RelaxedPlacement {
                        translate_x: snap_mm(
                            (vacated.translate_x + shift_x).clamp(feasible_min_x, feasible_max_x),
                        ),
                        translate_y: snap_mm(vacated.translate_y + shift_y),
                        ..vacated.clone()
                    };
                    anchor_local_candidates.push((
                        base_ordinal + anchor_local_candidates.len(),
                        zero_prior,
                        projected,
                    ));
                }
                // The peers' vacated translations: the poses the *other*
                // pieces of this round's ejection set were lifted out of.
                // They are absolute positions rather than displacements, so
                // they go in as their own candidates rather than through the
                // cloud, and they are the only way a candidate stream that is
                // otherwise a neighbourhood of one pose can propose an
                // exchange.
                for (peer_x, peer_y) in anchor_local.peer_poses(piece_index) {
                    let peer = RelaxedPlacement {
                        translate_x: snap_mm(peer_x.clamp(feasible_min_x, feasible_max_x)),
                        translate_y: snap_mm(peer_y),
                        ..vacated.clone()
                    };
                    anchor_local_candidates.push((
                        base_ordinal + anchor_local_candidates.len(),
                        zero_prior,
                        peer,
                    ));
                }
                // The displacement cloud rides the anchor orientation only;
                // the remaining priors contribute the vacated translation
                // under their own orientation, which is item three of the
                // primitive and not a second cloud.
                let directions = anchor_local.directions(piece_index);
                for magnitude in anchor_local.magnitudes_mm(
                    prior_bounds.max_x - prior_bounds.min_x,
                    prior_bounds.max_y - prior_bounds.min_y,
                ) {
                    for (direction_x, direction_y) in directions.iter().copied() {
                        let probe = RelaxedPlacement {
                            translate_x: snap_mm(
                                (vacated.translate_x + magnitude * direction_x)
                                    .clamp(feasible_min_x, feasible_max_x),
                            ),
                            translate_y: snap_mm(vacated.translate_y + magnitude * direction_y),
                            ..vacated.clone()
                        };
                        anchor_local_candidates.push((
                            base_ordinal + anchor_local_candidates.len(),
                            zero_prior,
                            probe,
                        ));
                    }
                }
                // The orientation-perturbation stream (modes 32 and 33). Every
                // candidate above shares one rotation and one mirror state -
                // the ones the piece was lifted out of - so the whole
                // anchor-local cloud is a *translation* neighbourhood, and a
                // layout built from continuous fine angles has no other
                // orientation stream that can reach an interior pocket. This
                // adds one: each variant of the vacated orientation gets the
                // neighbourhood just built, rigidly carried onto it by the
                // shift that keeps the piece's own bounding-box centre where
                // the vacated footprint's centre was. Nothing else changes -
                // the same clamps, the same snap grid, the same charged
                // confirmation row, the same contact walk, the same finalist
                // cap.
                let variants = anchor_local.orientation_variants(piece_index);
                if !variants.is_empty() {
                    // What has been pushed under this prior is exactly the
                    // vacated pose's local translation neighbourhood: the
                    // vacated pose, the projection trajectory, the peers'
                    // pockets, then the aimed cloud.
                    let neighbourhood = anchor_local_candidates
                        .iter()
                        .map(|(_, _, candidate)| (candidate.translate_x, candidate.translate_y))
                        .collect::<Vec<(f64, f64)>>();
                    let vacated_center_x =
                        (prior_bounds.min_x + prior_bounds.max_x) * 0.5 + vacated.translate_x;
                    let vacated_center_y =
                        (prior_bounds.min_y + prior_bounds.max_y) * 0.5 + vacated.translate_y;
                    let mut recentred = Vec::with_capacity(variants.len());
                    for variant in variants {
                        // The same piece-feasible band the priors are clamped
                        // into, re-derived for this variant's own extent: a
                        // rotation changes the width the strip has to hold.
                        let variant_min_x = inset - variant.min_x;
                        let variant_max_x =
                            work_settings.sheet_short_axis_mm - inset - variant.max_x;
                        if variant_min_x > variant_max_x {
                            continue;
                        }
                        recentred.push((
                            variant,
                            variant_min_x,
                            variant_max_x,
                            vacated_center_x
                                - (variant.min_x + variant.max_x) * 0.5
                                - vacated.translate_x,
                            vacated_center_y
                                - (variant.min_y + variant.max_y) * 0.5
                                - vacated.translate_y,
                        ));
                    }
                    // Rank-major: every variant's re-centred pose before any
                    // variant's first displacement, so a row-budget cut
                    // truncates the shared neighbourhood uniformly instead of
                    // spending the whole budget on the first rung.
                    for (translate_x, translate_y) in neighbourhood {
                        for (variant, variant_min_x, variant_max_x, shift_x, shift_y) in &recentred
                        {
                            orientation_candidates.push(RelaxedPlacement {
                                input_index: piece_index,
                                rotation_deg: variant.rotation_deg,
                                mirrored: variant.mirrored,
                                translate_x: snap_mm(
                                    (translate_x + shift_x).clamp(*variant_min_x, *variant_max_x),
                                ),
                                translate_y: snap_mm(translate_y + shift_y),
                            });
                        }
                    }
                }
            }
        }
        let stations = skyline_hint_stations(
            parent,
            work_settings,
            prior_bounds.max_x - prior_bounds.min_x,
        );
        if stations.is_empty() {
            continue;
        }
        let bucket_ordinal = ORIENTATIONS_PER_PIECE + prior_index;
        for (station_index, (station_x, station_top)) in stations.iter().copied().enumerate() {
            let hint = RelaxedPlacement {
                input_index: piece_index,
                rotation_deg,
                mirrored,
                translate_x: snap_mm(
                    (station_x - prior_center_x).clamp(feasible_min_x, feasible_max_x),
                ),
                translate_y: snap_mm(station_top - prior_bounds.min_y + 0.6),
            };
            if station_index == 0 && station_zero_hint.is_none() {
                station_zero_hint = Some(hint.clone());
            }
            let landing = |probe: &RelaxedPlacement| -> u64 {
                grid_key(prior_bounds.max_y + probe.translate_y)
                    .max(0)
                    .unsigned_abs()
            };
            // Vertical contact ladder at the station: several epsilon
            // offsets above the valley top so the ranked confirmation can
            // settle on the lowest valid clearance instead of a single
            // fixed hover.
            for epsilon in [0.05f64, 0.3, 1.2, 2.4] {
                let mut probe = hint.clone();
                probe.translate_y = snap_mm(station_top - prior_bounds.min_y + epsilon);
                candidates.push((landing(&probe), bucket_ordinal, zero_prior, probe));
            }
            candidates.push((landing(&hint), bucket_ordinal, zero_prior, hint.clone()));
            for radius in CONSTRUCTION_PROBE_RADII_MM {
                for (direction_x, direction_y) in CONSTRUCTION_PROBE_DIRECTIONS {
                    let mut probe = hint.clone();
                    probe.translate_x += radius * direction_x;
                    probe.translate_y += radius * direction_y;
                    probe.translate_x =
                        snap_mm(probe.translate_x.clamp(feasible_min_x, feasible_max_x));
                    candidates.push((landing(&probe), bucket_ordinal, zero_prior, probe));
                }
            }
        }
        // Interleaved escape ladders: the station-local ladder stacks in the
        // lowest valley from its own top upward (filling valleys instead of
        // ratcheting the global frontier), while the global-frontier ladder
        // is the guaranteed-empty escape; interleaving keeps the global rung
        // inside the reserved shelf rows even when the valley ladder is
        // fully congested.
        const STATION_LADDER_RUNGS_MM: [f64; 8] = [0.05, 0.3, 0.6, 1.2, 1.8, 2.4, 3.6, 4.8];
        for (step, rung) in STATION_LADDER_RUNGS_MM.into_iter().enumerate() {
            for lateral in [0.0f64, -2.0, 2.0, -6.0, 6.0] {
                let probe = RelaxedPlacement {
                    input_index: piece_index,
                    rotation_deg,
                    mirrored,
                    translate_x: snap_mm(
                        (stations[0].0 - prior_center_x + lateral)
                            .clamp(feasible_min_x, feasible_max_x),
                    ),
                    translate_y: snap_mm(stations[0].1 - prior_bounds.min_y + rung),
                };
                shelf_candidates.push((bucket_ordinal, zero_prior, probe));
            }
            if step < 4 {
                for lateral in [0.0f64, -2.0, 2.0, -6.0, 6.0] {
                    let probe = RelaxedPlacement {
                        input_index: piece_index,
                        rotation_deg,
                        mirrored,
                        translate_x: snap_mm(
                            (stations[0].0 - prior_center_x + lateral)
                                .clamp(feasible_min_x, feasible_max_x),
                        ),
                        translate_y: snap_mm(
                            frontier_y - prior_bounds.min_y + 0.6 * (step as f64 + 1.0),
                        ),
                    };
                    shelf_candidates.push((bucket_ordinal, zero_prior, probe));
                }
            }
        }
    }
    // The orientation and position streams are anchored at the station-zero
    // hint, so a piece too wide for any skyline window has neither. That is
    // fatal only when the stations are the sole source of candidates:
    // anchor-local seeding needs no station, because the vacated pose is its
    // own anchor.
    let station_zero_hint = match station_zero_hint {
        Some(hint) => Some(hint),
        None if !anchor_local_candidates.is_empty() => None,
        None => return Err("construction produced no station-zero hint".to_owned()),
    };
    if let Some(station_zero_hint) = station_zero_hint {
        let angle_seed = derive_seed(
            construction_seed ^ CONFLICT_RUIN_ANGLE_SEED_DOMAIN,
            ordinal,
            piece_index,
        );
        let orientations =
            conflict_ruin_orientations(pieces[piece_index], &station_zero_hint, angle_seed);
        for (orientation_ordinal, (rotation_deg, mirrored)) in orientations.into_iter().enumerate()
        {
            work.diagnostics.orientation_streams =
                work.diagnostics.orientation_streams.saturating_add(1);
            if work.diagnostics.orientation_streams > work.quotas.max_orientation_streams {
                return Err(work.cap("orientation-stream budget exhausted"));
            }
            let orientation = RelaxedPlacement {
                input_index: piece_index,
                rotation_deg,
                mirrored,
                translate_x: 0.0,
                translate_y: 0.0,
            };
            let local_collision =
                build_collision(pieces[piece_index], &orientation, work_settings, work)?;
            let local_max_y = local_collision
                .bounds()
                .ok_or_else(|| "construction orientation has empty geometry".to_owned())?
                .max_y;
            let position_seed = derive_seed(
                construction_seed ^ CONFLICT_RUIN_POSITION_SEED_DOMAIN,
                ordinal
                    .saturating_mul(ORIENTATIONS_PER_PIECE)
                    .saturating_add(orientation_ordinal),
                piece_index,
            );
            let proposals = vacancy_positions(
                &station_zero_hint,
                &orientation,
                &local_collision,
                parent,
                work_settings,
                position_seed,
                work,
            )?;
            for placement in proposals {
                let key = grid_key(local_max_y + placement.translate_y)
                    .max(0)
                    .unsigned_abs();
                candidates.push((key, orientation_ordinal, false, placement));
            }
        }
    }
    candidates.sort_by(|first, second| {
        first
            .0
            .cmp(&second.0)
            .then_with(|| first.1.cmp(&second.1))
            .then_with(|| placement_key(&first.3).cmp(&placement_key(&second.3)))
    });
    let local_row_cap = CONSTRUCTION_ROWS_PER_PIECE - CONSTRUCTION_SHELF_ROWS;
    let mut rows = 0usize;
    // Anchor-local candidates get their own charged-row budget rather than the
    // station stream's. Leading with them and charging them to the shared cap
    // would let a cloud that confirms nothing spend the rows the stations
    // needed - which is exactly how a piece that used to be re-placed from the
    // skyline stops being re-placed at all. The stream is additive by
    // construction, so its budget is too.
    let mut anchor_rows = 0usize;
    // The orientation stream's budget is held apart from *both* of the others,
    // for the same reason: an additive degree of freedom must never be able to
    // spend the rows a legacy-reachable solution would have used.
    let mut orientation_rows = 0usize;
    let mut tried_buckets = BTreeSet::new();
    let mut finalists = Vec::with_capacity(CONSTRUCTION_FINALISTS_PER_SLOT);
    construction.anchor_local_candidates = construction
        .anchor_local_candidates
        .saturating_add(anchor_local_candidates.len());
    construction.orientation_candidates = construction
        .orientation_candidates
        .saturating_add(orientation_candidates.len());
    // Anchor-local candidates lead. They are the only ones that can reach an
    // interior pocket, they are already ordered closest-displacement first,
    // and they are bounded by the cloud's own size, so leading costs the
    // station stream at most that many of its charged rows.
    //
    // The orientation-perturbed candidates come next: after every pose the
    // legacy stream can reach, so anything modes 28 and 29 would have found is
    // still found first and in the same finalist rank, and before the station
    // stream, because they are anchor-local poses too.
    let ranked = anchor_local_candidates
        .into_iter()
        .enumerate()
        .map(|(index, (bucket_ordinal, zero_prior, candidate))| {
            (
                false,
                bucket_ordinal,
                CandidateProvenance {
                    zero_prior,
                    anchor_local: true,
                    // The first candidate pushed under the anchor's own
                    // orientation prior is the vacated pose itself.
                    vacated: index == 0,
                    orientation_perturbed: false,
                },
                candidate,
            )
        })
        .chain(orientation_candidates.into_iter().enumerate().map(
            |(index, candidate)| {
                (
                    false,
                    ORIENTATION_PERTURBATION_BUCKET_BASE + index,
                    CandidateProvenance {
                        zero_prior: false,
                        anchor_local: false,
                        vacated: false,
                        orientation_perturbed: true,
                    },
                    candidate,
                )
            },
        ))
        .chain(
            candidates
                .into_iter()
                .map(|(_, bucket_ordinal, zero_prior, candidate)| {
                    (
                        false,
                        bucket_ordinal,
                        CandidateProvenance {
                            zero_prior,
                            anchor_local: false,
                            vacated: false,
                            orientation_perturbed: false,
                        },
                        candidate,
                    )
                }),
        )
        .chain(
            shelf_candidates
                .into_iter()
                .map(|(bucket_ordinal, zero_prior, candidate)| {
                    (
                        true,
                        bucket_ordinal,
                        CandidateProvenance {
                            zero_prior,
                            anchor_local: false,
                            vacated: false,
                            orientation_perturbed: false,
                        },
                        candidate,
                    )
                }),
        );
    #[cfg(feature = "constructor-census")]
    crate::constructor_census::slot_begin();
    for (is_shelf, bucket_ordinal, provenance, candidate) in ranked {
        if finalists.len() == CONSTRUCTION_FINALISTS_PER_SLOT || rows >= CONSTRUCTION_ROWS_PER_PIECE
        {
            break;
        }
        if provenance.anchor_local {
            if anchor_rows >= ANCHOR_LOCAL_ROWS {
                continue;
            }
        } else if provenance.orientation_perturbed {
            if orientation_rows >= ORIENTATION_PERTURBATION_ROWS {
                continue;
            }
        } else if !is_shelf && rows >= local_row_cap {
            continue;
        }
        let bucket = (
            bucket_ordinal,
            grid_key(candidate.translate_x).div_euclid(256),
            grid_key(candidate.translate_y).div_euclid(256),
        );
        if !tried_buckets.insert(bucket) {
            continue;
        }
        if provenance.anchor_local {
            anchor_rows += 1;
            construction.anchor_local_rows = construction.anchor_local_rows.saturating_add(1);
        } else if provenance.orientation_perturbed {
            orientation_rows += 1;
            construction.orientation_rows = construction.orientation_rows.saturating_add(1);
        } else {
            rows += 1;
        }
        let Some(collision) = ({
            #[cfg(feature = "constructor-census")]
            let _census = crate::constructor_census::site(
                crate::constructor_census::Site::Candidate,
            );
            construction_confirm_row(
                pieces,
                work_settings,
                parent,
                piece_index,
                &candidate,
                inset,
                construction,
                work,
            )?
        }) else {
            continue;
        };
        // Multi-directional contact walk (the bounded NFP surrogate): the
        // confirmed pose alternates gravity, tangential, and diagonal
        // contact pushes along the REAL polygons, walking the contact
        // boundary into notches no single axis push reaches. Each push
        // starts from an already-valid pose, so every charged row keeps the
        // high yield that separates this family from speculative-row
        // variants; the walk stops when a full cycle moves nothing or the
        // per-slot row cap is reached.
        let mut walk_pose = candidate.clone();
        let mut walk_collision = collision;
        for _cycle in 0..2 {
            let entry = placement_key(&walk_pose);
            for direction in [
                (0.0, -1.0),
                (-1.0, 0.0),
                (-0.7071067811865476, -0.7071067811865476),
            ] {
                if rows >= CONSTRUCTION_ROWS_PER_PIECE {
                    break;
                }
                let (pushed_pose, pushed_collision) = construction_slide(
                    pieces,
                    work_settings,
                    parent,
                    piece_index,
                    walk_pose,
                    walk_collision,
                    direction,
                    inset,
                    &mut rows,
                    construction,
                    work,
                )?;
                walk_pose = pushed_pose;
                walk_collision = pushed_collision;
            }
            if placement_key(&walk_pose) == entry {
                break;
            }
        }
        if rows < CONSTRUCTION_ROWS_PER_PIECE {
            let (final_pose, final_collision) = construction_slide(
                pieces,
                work_settings,
                parent,
                piece_index,
                walk_pose,
                walk_collision,
                (0.0, -1.0),
                inset,
                &mut rows,
                construction,
                work,
            )?;
            walk_pose = final_pose;
            walk_collision = final_collision;
        }
        if is_shelf {
            construction.shelf_finalists = construction.shelf_finalists.saturating_add(1);
        }
        if provenance.anchor_local {
            construction.anchor_local_finalists =
                construction.anchor_local_finalists.saturating_add(1);
        }
        if provenance.orientation_perturbed {
            construction.orientation_finalists =
                construction.orientation_finalists.saturating_add(1);
        }
        finalists.push((walk_pose, Arc::new(walk_collision), provenance));
    }
    #[cfg(feature = "constructor-census")]
    crate::constructor_census::slot_end();
    profiling::deep::finish(Phase::VacancyProposals, proposal_span);
    Ok(finalists)
}

/// Maximal-contact push: translates an already-valid pose along one axis
/// direction with the geometric ladder plus two bisection refinements,
/// stopping at the first exact contact, and returns the furthest valid
/// (pose, collision). Every attempt is a charged confirmation row.
#[allow(clippy::too_many_arguments)]
fn construction_slide(
    pieces: &[GeneralFastPiece<'_>],
    work_settings: GeneralFastSettings,
    parent: &VacancyState,
    piece_index: usize,
    start_pose: RelaxedPlacement,
    start_collision: PolygonSet,
    direction: (f64, f64),
    inset: f64,
    rows: &mut usize,
    construction: &mut GeneralPersistentVacancyConstructionDiagnostics,
    work: &mut RunWork,
) -> Result<(RelaxedPlacement, PolygonSet), String> {
    let mut settled_pose = start_pose.clone();
    let mut settled_collision = start_collision;
    let mut last_valid = 0.0f64;
    let mut first_invalid = None;
    for delta in CONSTRUCTION_DROP_LADDER_MM {
        let mut probe = start_pose.clone();
        probe.translate_x = snap_mm(start_pose.translate_x + delta * direction.0);
        probe.translate_y = snap_mm(start_pose.translate_y + delta * direction.1);
        *rows += 1;
        let confirmed = {
            #[cfg(feature = "constructor-census")]
            let _census = crate::constructor_census::site(
                crate::constructor_census::Site::SlideLadder,
            );
            construction_confirm_row(
                pieces,
                work_settings,
                parent,
                piece_index,
                &probe,
                inset,
                construction,
                work,
            )?
        };
        match confirmed {
            Some(pushed) => {
                settled_pose = probe;
                settled_collision = pushed;
                last_valid = delta;
            }
            None => {
                first_invalid = Some(delta);
                break;
            }
        }
    }
    if let Some(invalid) = first_invalid {
        let mut low = last_valid;
        let mut high = invalid;
        for _ in 0..2 {
            let mid = (low + high) * 0.5;
            let mut probe = start_pose.clone();
            probe.translate_x = snap_mm(start_pose.translate_x + mid * direction.0);
            probe.translate_y = snap_mm(start_pose.translate_y + mid * direction.1);
            *rows += 1;
            let confirmed = {
                #[cfg(feature = "constructor-census")]
                let _census = crate::constructor_census::site(
                    crate::constructor_census::Site::SlideBisect,
                );
                construction_confirm_row(
                    pieces,
                    work_settings,
                    parent,
                    piece_index,
                    &probe,
                    inset,
                    construction,
                    work,
                )?
            };
            match confirmed {
                Some(pushed) => {
                    settled_pose = probe;
                    settled_collision = pushed;
                    low = mid;
                }
                None => {
                    high = mid;
                }
            }
        }
    }
    Ok((settled_pose, settled_collision))
}

/// One exact confirmation row: charges the finalist-row budget, builds the
/// pose collision, and checks full-sheet containment plus zero exact
/// overlap against the parent's active pieces. Returns the collision when
/// the pose is exact-valid.
#[allow(clippy::too_many_arguments)]
fn construction_confirm_row(
    pieces: &[GeneralFastPiece<'_>],
    work_settings: GeneralFastSettings,
    parent: &VacancyState,
    piece_index: usize,
    candidate: &RelaxedPlacement,
    inset: f64,
    construction: &mut GeneralPersistentVacancyConstructionDiagnostics,
    work: &mut RunWork,
) -> Result<Option<PolygonSet>, String> {
    let started = profiling::deep::start(Phase::VacancyExactRows);
    #[cfg(feature = "constructor-census")]
    crate::constructor_census::row_started();
    construction.exact_rows = construction.exact_rows.saturating_add(1);
    work.diagnostics.exact_finalist_rows = work.diagnostics.exact_finalist_rows.saturating_add(1);
    if work.diagnostics.exact_finalist_rows > work.quotas.max_exact_finalist_rows {
        return Err(work.cap("exact-finalist row budget exhausted"));
    }
    // The inner certificate, before anything is built. It answers "provably
    // overlapping" or "no information"; a proof is the verdict the exact tier
    // below would have returned, so acting on one substitutes a decision rather
    // than making a different one. Off both flags this is a zero-sized call
    // that returns `None`.
    work.reject_certificates.begin_candidate(
        pieces[piece_index].polygon,
        piece_index,
        candidate.rotation_deg,
        candidate.mirrored,
        candidate.translate_x,
        candidate.translate_y,
        collision_expansion_mm(work_settings),
    );
    #[cfg(feature = "constructor-census")]
    crate::constructor_census::row_certificate(
        [
            work.reject_certificates
                .proven_overlap(&parent.active, 1)
                .is_some(),
            work.reject_certificates
                .proven_overlap(&parent.active, 2)
                .is_some(),
            work.reject_certificates
                .proven_overlap(&parent.active, 4)
                .is_some(),
            work.reject_certificates
                .proven_overlap(&parent.active, construction_reject_certificate::COVER_DISCS)
                .is_some(),
        ],
        work.reject_certificates.proven_overlap_without_inflation(
            &parent.active,
            construction_reject_certificate::COVER_DISCS,
        ),
        work.reject_certificates
            .signed_pressure(&parent.active, construction_reject_certificate::COVER_DISCS),
    );
    #[cfg(feature = "fast-constructor-reject")]
    if work
        .reject_certificates
        .proven_overlap(&parent.active, construction_reject_certificate::REJECT_DISCS)
        .is_some()
    {
        debug_assert!(
            certified_row_really_overlaps(pieces, work_settings, parent, piece_index, candidate),
            "the constructor's inner overlap certificate disagreed with the exact tier"
        );
        #[cfg(feature = "constructor-census")]
        crate::constructor_census::row_rejected_by_overlap();
        profiling::deep::finish(Phase::VacancyExactRows, started);
        return Ok(None);
    }
    profiling::deep::count(Counter::CollisionPolygonBuilds, 1);
    let build_started = profiling::deep::start(Phase::CollisionPolygonBuild);
    let collision = build_collision(pieces[piece_index], candidate, work_settings, work)?;
    profiling::deep::finish(Phase::CollisionPolygonBuild, build_started);
    if !collision.fits_rect(
        inset,
        inset,
        work_settings.sheet_short_axis_mm - inset,
        work_settings.sheet_long_axis_mm - inset,
    ) {
        #[cfg(feature = "constructor-census")]
        crate::constructor_census::row_rejected_by_containment();
        profiling::deep::finish(Phase::VacancyExactRows, started);
        return Ok(None);
    }
    let pairs_started = profiling::deep::start(Phase::ExactOverlapTest);
    profiling::deep::count(Counter::ExactPairTests, 1);
    // One separation certificate per confirmation row, against the parent's
    // pre-derived ones. A no-op off the profile, and inert for a degenerate
    // polygon, in which case every pair below takes the exact route.
    work.confirm_shields.begin_candidate(&collision);
    for fixed_index in 0..pieces.len() {
        if !parent.active[fixed_index] {
            continue;
        }
        work.charge_experimental_pair()?;
        let fixed = parent.collisions[fixed_index]
            .as_ref()
            .ok_or_else(|| format!("active piece {fixed_index} has no collision"))?;
        // The prefilter answers "provably separated" or "no information", so a
        // skip here is the exact query's own answer, reached without running
        // it. The debug build proves that claim rather than asserting it: it
        // runs the query anyway and requires the areas to agree.
        if work.confirm_shields.separated(fixed_index) {
            debug_assert_eq!(
                collision
                    .intersection_area_mm2(fixed)
                    .expect("a certified-separated pair is a valid pair query"),
                0.0,
                "the constructor's separation certificate disagreed with the exact tier"
            );
            continue;
        }
        if exact_intersection_area(&collision, fixed, work)? > 0.0 {
            #[cfg(feature = "constructor-census")]
            crate::constructor_census::row_rejected_by_overlap();
            profiling::deep::finish(Phase::ExactOverlapTest, pairs_started);
            profiling::deep::finish(Phase::VacancyExactRows, started);
            return Ok(None);
        }
    }
    #[cfg(feature = "constructor-census")]
    crate::constructor_census::row_accepted();
    profiling::deep::finish(Phase::ExactOverlapTest, pairs_started);
    profiling::deep::finish(Phase::VacancyExactRows, started);
    Ok(Some(collision))
}

/// The empirical half of the inner certificate's soundness claim.
///
/// Called from the `debug_assert` on the reject path, so it runs on **every**
/// row the certificate rejects in a debug build and on none of them in a
/// release one: it builds the collision polygon the certificate exists to avoid
/// building and requires a positive exact intersection area against some active
/// piece. A certificate that were ever wrong would fire here rather than
/// silently changing what the constructor accepts.
///
/// It deliberately does not go through [`build_collision`] or
/// [`exact_intersection_area`], because those charge quotas: the assertion must
/// not move the budgets the arm it is checking runs under.
#[cfg(feature = "fast-constructor-reject")]
fn certified_row_really_overlaps(
    pieces: &[GeneralFastPiece<'_>],
    work_settings: GeneralFastSettings,
    parent: &VacancyState,
    piece_index: usize,
    candidate: &RelaxedPlacement,
) -> bool {
    let Ok(collision) = LEGACY.exact_authority().collision_polygon(
        pieces[piece_index].polygon,
        KernelPose {
            rotation_deg: candidate.rotation_deg,
            mirrored: candidate.mirrored,
            translate_x: candidate.translate_x,
            translate_y: candidate.translate_y,
        },
        collision_expansion_mm(work_settings),
    ) else {
        return false;
    };
    (0..pieces.len()).any(|fixed_index| {
        parent.active[fixed_index]
            && parent.collisions[fixed_index].as_ref().is_some_and(|fixed| {
                collision
                    .intersection_area_mm2(fixed)
                    .is_ok_and(|area| area > 0.0)
            })
    })
}

pub(super) const CONSTRUCTION_DROP_LADDER_MM: [f64; 6] = [0.4, 0.8, 1.6, 3.2, 6.4, 12.8];

const CONSTRUCTION_PROBE_RADII_MM: [f64; 12] = [
    0.128, 0.256, 0.384, 0.512, 0.768, 1.024, 1.536, 2.048, 3.072, 4.096, 6.144, 8.192,
];
const CONSTRUCTION_PROBE_DIRECTIONS: [(f64, f64); 8] = [
    (1.0, 0.0),
    (-1.0, 0.0),
    (0.0, 1.0),
    (0.0, -1.0),
    (0.7071067811865476, 0.7071067811865476),
    (-0.7071067811865476, 0.7071067811865476),
    (0.7071067811865476, -0.7071067811865476),
    (-0.7071067811865476, -0.7071067811865476),
];

fn reconstruct_from_hints(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    target_depth_mm: f64,
    hints: &RelaxedState,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
) -> Result<(VacancyState, f64), String> {
    let target_settings = GeneralFastSettings {
        sheet_long_axis_mm: target_depth_mm,
        ..fast_settings
    };
    let mut state = VacancyState {
        placements: hints.placements.clone(),
        active: vec![false; pieces.len()],
        collisions: vec![None; pieces.len()],
        last_transition: None,
    };
    let hint_state = VacancyState {
        placements: hints.placements.clone(),
        active: vec![true; pieces.len()],
        collisions: vec![None; pieces.len()],
        last_transition: None,
    };
    let reconstruction_seed = parent_seed_key(&hint_state, pieces);
    let mut order = (0..pieces.len()).collect::<Vec<_>>();
    order.sort_by_key(|index| {
        (
            grid_key(hints.placements[*index].translate_y),
            pieces[*index].id,
        )
    });
    let mut recon = GeneralPersistentVacancyReconstructionDiagnostics {
        insertions: 0,
        exact_rows: 0,
        rows_per_piece_cap: RECONSTRUCTION_ROWS_PER_PIECE,
        deferred_first_pass: 0,
        failed_piece_id: None,
        failed_piece_count: 0,
    };
    let mut deferred = Vec::new();
    for (ordinal, piece_index) in order.into_iter().enumerate() {
        let placed = reconstruct_insert_piece(
            pieces,
            target_settings,
            hints,
            &mut state,
            reconstruction_seed,
            ordinal,
            piece_index,
            false,
            None,
            &mut recon,
            work,
        )?;
        if !placed {
            deferred.push(piece_index);
            recon.deferred_first_pass = recon.deferred_first_pass.saturating_add(1);
        }
    }
    // Deferred second pass: pieces whose hint pockets were closed during the
    // first pass retry after every other piece has settled, when the shelf
    // region and any reopened pockets are maximally available.
    let mut still_failed = Vec::new();
    for (retry_ordinal, piece_index) in deferred.into_iter().enumerate() {
        let placed = reconstruct_insert_piece(
            pieces,
            target_settings,
            hints,
            &mut state,
            reconstruction_seed,
            // The deferred pass continues the first pass's ordinal stream, so
            // its seeds never collide with the one-ordinal-per-piece prefix.
            pieces.len() + retry_ordinal,
            piece_index,
            false,
            None,
            &mut recon,
            work,
        )?;
        if !placed {
            still_failed.push(pieces[piece_index].id.to_owned());
        }
    }
    if let Some(first_failed) = still_failed.first() {
        recon.failed_piece_id = Some(first_failed.clone());
        recon.failed_piece_count = still_failed.len();
        diagnostics.reconstruction = Some(recon.clone());
        return Err(format!(
            "seeded reconstruction left {} pieces without an exact-valid pose after the deferred pass, first {}",
            still_failed.len(),
            first_failed
        ));
    }
    diagnostics.reconstruction = Some(recon.clone());
    diagnostics.complete_states = diagnostics.complete_states.saturating_add(1);
    if let Err(reason) = audit_state(&state, pieces, target_settings, true, work) {
        if !reason.starts_with("cap: ") {
            diagnostics.publication_rejections =
                diagnostics.publication_rejections.saturating_add(1);
        }
        return Err(reason);
    }
    let placements = fast_placements(&state, pieces, false);
    let independent = coupled_independent_source_depth(pieces, &placements, target_settings)
        .map_err(|error| format!("persistent vacancy reconstructed depth: {error}"))?;
    Ok((state, independent))
}

#[allow(clippy::too_many_arguments)]
fn reconstruct_insert_piece(
    pieces: &[GeneralFastPiece<'_>],
    target_settings: GeneralFastSettings,
    hints: &RelaxedState,
    state: &mut VacancyState,
    reconstruction_seed: u64,
    ordinal: usize,
    piece_index: usize,
    rank_by_depth: bool,
    hazard_screen: Option<&mut JaguaHazardIndex>,
    recon: &mut GeneralPersistentVacancyReconstructionDiagnostics,
    work: &mut RunWork,
) -> Result<bool, String> {
    work.diagnostics.selected_piece_slots = work.diagnostics.selected_piece_slots.saturating_add(1);
    if work.diagnostics.selected_piece_slots > work.quotas.max_selected_piece_slots {
        return Err(work.cap("selected-piece slot budget exhausted"));
    }
    work.charge_source_features(pieces[piece_index].polygon.vertex_count().saturating_mul(2))?;
    // Conservative fixed bound for the per-attempt transient buffers
    // (candidate rows, shelf poses, ranked vector, bucket set); they are
    // structurally bounded far below this figure.
    const RECONSTRUCTION_TRANSIENT_BYTES: usize = 96 * 1024;
    let live_bytes = state_slice_bytes(std::slice::from_ref(state))
        .saturating_add(2usize.saturating_mul(size_of::<VacancyState>()))
        .saturating_add(RECONSTRUCTION_TRANSIENT_BYTES);
    work.diagnostics.total_retained_peak_bytes =
        work.diagnostics.total_retained_peak_bytes.max(live_bytes);
    if live_bytes > MAX_RETAINED_BYTES {
        return Err(work.cap("reconstruction live-state memory budget exhausted"));
    }
    let inset = collision_sheet_inset_mm(target_settings);
    let hint = &hints.placements[piece_index];
    let hint_x = grid_key(hint.translate_x);
    let hint_y = grid_key(hint.translate_y);
    let angle_seed = derive_seed(
        reconstruction_seed ^ CONFLICT_RUIN_ANGLE_SEED_DOMAIN,
        ordinal,
        piece_index,
    );
    let orientations = conflict_ruin_orientations(pieces[piece_index], hint, angle_seed);
    let mut candidates = Vec::new();
    // Deterministic displacement probes around the hint at the hint
    // orientation: the reconstruction usually needs a sub-millimetre shift
    // away from neighbors that sit at the hint contract's tighter
    // separation, and the general position generator's position cap crowds
    // those poses out.
    const PROBE_RADII_MM: [f64; 12] = [
        0.128, 0.256, 0.384, 0.512, 0.768, 1.024, 1.536, 2.048, 3.072, 4.096, 6.144, 8.192,
    ];
    const PROBE_DIRECTIONS: [(f64, f64); 8] = [
        (1.0, 0.0),
        (-1.0, 0.0),
        (0.0, 1.0),
        (0.0, -1.0),
        (0.7071067811865476, 0.7071067811865476),
        (-0.7071067811865476, 0.7071067811865476),
        (0.7071067811865476, -0.7071067811865476),
        (-0.7071067811865476, -0.7071067811865476),
    ];
    let hint_orientation = RelaxedPlacement {
        input_index: piece_index,
        rotation_deg: hint.rotation_deg,
        mirrored: hint.mirrored,
        translate_x: 0.0,
        translate_y: 0.0,
    };
    let hint_local = build_collision(
        pieces[piece_index],
        &hint_orientation,
        target_settings,
        work,
    )?;
    let hint_local_bounds = hint_local
        .bounds()
        .ok_or_else(|| "reconstruction hint orientation has empty geometry".to_owned())?;
    let hint_local_min_y = hint_local_bounds.min_y;
    let hint_local_max_y = hint_local_bounds.max_y;
    let probe_key = |probe: &RelaxedPlacement| -> u64 {
        if rank_by_depth {
            grid_key(hint_local_max_y + probe.translate_y)
                .max(0)
                .unsigned_abs()
        } else {
            grid_key(probe.translate_x)
                .abs_diff(hint_x)
                .saturating_add(grid_key(probe.translate_y).abs_diff(hint_y))
        }
    };
    candidates.push((probe_key(hint), 0usize, hint.clone()));
    for radius in PROBE_RADII_MM {
        for (direction_x, direction_y) in PROBE_DIRECTIONS {
            let mut probe = hint.clone();
            probe.translate_x += radius * direction_x;
            probe.translate_y += radius * direction_y;
            candidates.push((probe_key(&probe), 0usize, probe));
        }
    }
    // Upward shelf fallback: the region above the current frontier is empty
    // during bottom-up reconstruction, so a piece whose hint pocket is
    // laterally closed under the tighter engine contract can escape upward;
    // the later settling ladder recompacts the layout. Shelf poses anchor
    // the piece's hint-orientation material bottom just above the frontier.
    let frontier_y = state
        .collisions
        .iter()
        .flatten()
        .filter_map(|collision| collision.bounds())
        .map(|bounds| bounds.max_y)
        .fold(0.0f64, f64::max);
    let mut shelf_candidates = Vec::new();
    for step in 1..=12u32 {
        for lateral in [0.0f64, -4.0, 4.0, -8.0, 8.0] {
            let mut probe = hint.clone();
            probe.translate_x += lateral;
            probe.translate_y = frontier_y - hint_local_min_y + 0.6 * f64::from(step);
            shelf_candidates.push(probe);
        }
    }
    for (orientation_ordinal, (rotation_deg, mirrored)) in orientations.into_iter().enumerate() {
        work.diagnostics.orientation_streams =
            work.diagnostics.orientation_streams.saturating_add(1);
        if work.diagnostics.orientation_streams > work.quotas.max_orientation_streams {
            return Err(work.cap("orientation-stream budget exhausted"));
        }
        let orientation = RelaxedPlacement {
            input_index: piece_index,
            rotation_deg,
            mirrored,
            translate_x: 0.0,
            translate_y: 0.0,
        };
        let local_collision =
            build_collision(pieces[piece_index], &orientation, target_settings, work)?;
        let position_seed = derive_seed(
            reconstruction_seed ^ CONFLICT_RUIN_POSITION_SEED_DOMAIN,
            ordinal
                .saturating_mul(ORIENTATIONS_PER_PIECE)
                .saturating_add(orientation_ordinal),
            piece_index,
        );
        let proposals = vacancy_positions(
            hint,
            &orientation,
            &local_collision,
            state,
            target_settings,
            position_seed,
            work,
        )?;
        let local_max_y = local_collision
            .bounds()
            .ok_or_else(|| "reconstruction orientation has empty geometry".to_owned())?
            .max_y;
        for placement in proposals {
            let key = if rank_by_depth {
                // Lowest-fit: rank by the approximate landing frontier so a
                // lifted piece claims the deepest pocket anywhere on the
                // sheet rather than returning near its old pose.
                grid_key(local_max_y + placement.translate_y)
                    .max(0)
                    .unsigned_abs()
            } else {
                grid_key(placement.translate_x)
                    .abs_diff(hint_x)
                    .saturating_add(grid_key(placement.translate_y).abs_diff(hint_y))
            };
            candidates.push((key, orientation_ordinal, placement));
        }
    }
    candidates.sort_by(|first, second| {
        first
            .0
            .cmp(&second.0)
            .then_with(|| first.1.cmp(&second.1))
            .then_with(|| placement_key(&first.2).cmp(&placement_key(&second.2)))
    });
    // The last RECONSTRUCTION_SHELF_ROWS of the per-piece budget are
    // reserved for the shelf fallback so local congestion can never starve
    // it.
    const RECONSTRUCTION_SHELF_ROWS: usize = 60;
    let local_row_cap = RECONSTRUCTION_ROWS_PER_PIECE - RECONSTRUCTION_SHELF_ROWS;
    let mut rows = 0usize;
    let mut tried_buckets = BTreeSet::new();
    let ranked = candidates
        .into_iter()
        .map(|(_, orientation_ordinal, candidate)| (false, orientation_ordinal, candidate))
        .chain(
            shelf_candidates
                .into_iter()
                .map(|candidate| (true, 0usize, candidate)),
        )
        .collect::<Vec<_>>();
    let mut hazard_screen = hazard_screen;
    for (is_shelf, orientation_ordinal, candidate) in ranked {
        if rows >= RECONSTRUCTION_ROWS_PER_PIECE {
            break;
        }
        if !is_shelf && rows >= local_row_cap {
            continue;
        }
        if let Some(index) = hazard_screen.as_deref_mut() {
            work.diagnostics.hazard_queries = work.diagnostics.hazard_queries.saturating_add(1);
            if work.diagnostics.hazard_queries > work.quotas.max_hazard_queries {
                return Err(work.cap("hazard-query budget exhausted"));
            }
            match index.query_unplaced(piece_index, hazard_pose(&candidate)) {
                Ok(GeneralHazardQuery::Complete {
                    boundary,
                    colliding_piece_ids,
                }) => {
                    if boundary || !colliding_piece_ids.is_empty() {
                        continue;
                    }
                }
                Ok(_) => {}
                Err(error) if error.to_string().contains("query envelope") => continue,
                Err(error) => {
                    return Err(format!("reconstruction hazard screen: {error}"));
                }
            }
        }
        let bucket = (
            orientation_ordinal,
            grid_key(candidate.translate_x).div_euclid(256),
            grid_key(candidate.translate_y).div_euclid(256),
        );
        if !tried_buckets.insert(bucket) {
            continue;
        }
        rows += 1;
        recon.exact_rows += 1;
        work.diagnostics.exact_finalist_rows =
            work.diagnostics.exact_finalist_rows.saturating_add(1);
        if work.diagnostics.exact_finalist_rows > work.quotas.max_exact_finalist_rows {
            return Err(work.cap("exact-finalist row budget exhausted"));
        }
        let collision = build_collision(pieces[piece_index], &candidate, target_settings, work)?;
        if !collision.fits_rect(
            inset,
            inset,
            target_settings.sheet_short_axis_mm - inset,
            target_settings.sheet_long_axis_mm - inset,
        ) {
            continue;
        }
        let mut overlapping = false;
        for fixed_index in 0..pieces.len() {
            if !state.active[fixed_index] {
                continue;
            }
            work.charge_experimental_pair()?;
            let fixed = state.collisions[fixed_index]
                .as_ref()
                .ok_or_else(|| format!("active piece {fixed_index} has no collision"))?;
            if exact_intersection_area(&collision, fixed, work)? > 0.0 {
                overlapping = true;
                break;
            }
        }
        if overlapping {
            continue;
        }
        state.placements[piece_index] = candidate;
        state.active[piece_index] = true;
        state.collisions[piece_index] = Some(Arc::new(collision));
        recon.insertions += 1;
        return Ok(true);
    }
    Ok(false)
}

fn initial_vacancy_state(
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    baseline: RelaxedState,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
    allow_complete: bool,
) -> Result<(VacancyState, Vec<PieceDifficulty>, Vec<usize>), String> {
    let mut collisions = Vec::with_capacity(pieces.len());
    let mut difficulty = Vec::with_capacity(pieces.len());
    for placement in &baseline.placements {
        let collision = build_collision(pieces[placement.input_index], placement, settings, work)?;
        difficulty.push(piece_difficulty(pieces[placement.input_index], &collision)?);
        collisions.push(Some(Arc::new(collision)));
    }
    let inset = collision_sheet_inset_mm(settings);
    let mut active = vec![true; pieces.len()];
    let mut inactive_order = Vec::new();
    for index in 0..pieces.len() {
        let collision = collisions[index]
            .as_ref()
            .ok_or_else(|| format!("missing initializer collision for piece {index}"))?;
        if !collision.fits_rect(
            inset,
            inset,
            settings.sheet_short_axis_mm - inset,
            settings.sheet_long_axis_mm - inset,
        ) {
            let overflow = boundary_overflow_grid(collision, settings)?;
            if overflow <= 0 {
                return Err(format!(
                    "piece {} failed target containment without positive grid overflow",
                    pieces[index].id
                ));
            }
            active[index] = false;
            inactive_order.push((index, overflow));
        }
    }
    inactive_order.sort_by(|(first, first_overflow), (second, second_overflow)| {
        second_overflow
            .cmp(first_overflow)
            .then_with(|| pieces[*first].id.cmp(pieces[*second].id))
    });
    let inactive_order = inactive_order
        .into_iter()
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if inactive_order.is_empty() && !allow_complete {
        return Err("target initializer removed no boundary offender".to_owned());
    }
    if inactive_order.len() > MAX_INACTIVE_PIECES
        || pieces.len().saturating_sub(inactive_order.len()) * 2 < pieces.len()
    {
        return Err(format!(
            "target initializer retained {} active and {} inactive pieces",
            pieces.len().saturating_sub(inactive_order.len()),
            inactive_order.len()
        ));
    }
    for index in &inactive_order {
        collisions[*index] = None;
    }
    let state = VacancyState {
        placements: baseline.placements,
        active,
        collisions,
        last_transition: None,
    };
    verify_exact_active_pairs(&state, work)?;
    diagnostics.direct_insertions = 0;
    Ok((state, difficulty, inactive_order))
}

#[allow(clippy::too_many_arguments)]
fn expand_parent(
    parent: &VacancyState,
    baseline: &[RelaxedPlacement],
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    difficulty: &[PieceDifficulty],
    hazard_catalog: &Arc<JaguaHazardCatalog>,
    layer: usize,
    mode: usize,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
    selected_piece_ids: &mut BTreeSet<String>,
    parent_selections: &mut Vec<GeneralPersistentVacancyParentSelectionDiagnostics>,
    children: &mut Vec<VacancyState>,
) -> Result<(), String> {
    let mut index = build_active_hazard_index(parent, pieces, settings, hazard_catalog)?;
    let parent_seed = parent_seed_key(parent, pieces);
    let transition_seed = derive_seed(PERSISTENT_VACANCY_SEED_DOMAIN ^ parent_seed, layer, 0);
    let mut selection = selected_inactive_pieces(parent, pieces, difficulty, layer, mode);
    // Mode 10 replaces the odd-layer coverage-insertion slot with a blocker
    // relocation slot driven by slot zero's observed ejection sets.
    let relocation_layer = matches!(mode, 10 | 12) && !layer.is_multiple_of(2);
    if relocation_layer {
        selection.indices.truncate(1);
    }
    let hardest_piece_id = selection
        .indices
        .first()
        .map(|index| pieces[*index].id.to_owned())
        .ok_or_else(|| "persistent vacancy parent has no inactive piece".to_owned())?;
    let coverage_piece_id = selection
        .rotation_start_index
        .and_then(|_| selection.indices.get(1))
        .map(|index| pieces[*index].id.to_owned());
    let stable_inactive = stable_inactive_order(parent, pieces);
    let mut selection_diagnostics = GeneralPersistentVacancyParentSelectionDiagnostics {
        parent_state_fingerprint: state_fingerprint(parent, pieces),
        inactive_order_hash: id_order_hash(&stable_inactive, pieces),
        scheduler_family: scheduler_family(mode).to_owned(),
        hardest_piece_id,
        rotation_start_index: selection.rotation_start_index,
        coverage_piece_id,
        transition_seed,
        revived: None,
        relocated_piece_id: None,
        slots: Vec::with_capacity(selection.indices.len()),
    };
    let children_before_slot_zero = children.len();
    for (selected_ordinal, piece_index) in selection.indices.into_iter().enumerate() {
        expand_selected_piece(
            parent,
            &baseline[piece_index],
            pieces,
            settings,
            &mut index,
            transition_seed,
            selected_ordinal,
            piece_index,
            diagnostics,
            work,
            selected_piece_ids,
            &mut selection_diagnostics,
            children,
        )?;
    }
    if relocation_layer {
        let relocated =
            select_relocation_piece(parent, pieces, &children[children_before_slot_zero..]);
        if let Some(relocated_index) = relocated {
            selection_diagnostics.relocated_piece_id = Some(pieces[relocated_index].id.to_owned());
            let mut temp = parent.clone();
            temp.active[relocated_index] = false;
            temp.collisions[relocated_index] = None;
            let mut temp_index =
                build_active_hazard_index(&temp, pieces, settings, hazard_catalog)?;
            expand_selected_piece(
                &temp,
                &parent.placements[relocated_index],
                pieces,
                settings,
                &mut temp_index,
                transition_seed,
                1,
                relocated_index,
                diagnostics,
                work,
                selected_piece_ids,
                &mut selection_diagnostics,
                children,
            )?;
        }
    }
    parent_selections.push(selection_diagnostics);
    Ok(())
}

fn build_active_hazard_index(
    parent: &VacancyState,
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    hazard_catalog: &Arc<JaguaHazardCatalog>,
) -> Result<JaguaHazardIndex, String> {
    let poses = parent
        .placements
        .iter()
        .map(hazard_pose)
        .collect::<Vec<_>>();
    JaguaHazardIndex::from_catalog_active(
        pieces,
        settings,
        settings.sheet_long_axis_mm,
        &poses,
        &parent.active,
        hazard_catalog,
    )
    .map_err(|error| format!("persistent vacancy partial hazard index: {error}"))
}

/// Chooses the active piece a mode-10 relocation slot moves: the piece most
/// often named as an ejected blocker by slot zero's children, ties broken by
/// stable ID; when slot zero produced no ejection children, the active piece
/// whose expanded collision reaches deepest into the strip.
fn select_relocation_piece(
    parent: &VacancyState,
    pieces: &[GeneralFastPiece<'_>],
    slot_zero_children: &[VacancyState],
) -> Option<usize> {
    let mut blocker_counts: BTreeMap<usize, usize> = BTreeMap::new();
    for child in slot_zero_children {
        if let Some(transition) = &child.last_transition {
            for blocker in &transition.ejected {
                *blocker_counts.entry(*blocker).or_insert(0) += 1;
            }
        }
    }
    if let Some(best) = blocker_counts
        .iter()
        .max_by(|(first_index, first_count), (second_index, second_count)| {
            first_count
                .cmp(second_count)
                .then_with(|| pieces[**second_index].id.cmp(pieces[**first_index].id))
        })
        .map(|(index, _)| *index)
    {
        return Some(best);
    }
    (0..parent.active.len())
        .filter(|index| parent.active[*index])
        .filter_map(|index| {
            parent.collisions[index]
                .as_ref()
                .and_then(|collision| collision.bounds())
                .map(|bounds| (index, grid_key(bounds.max_y)))
        })
        .max_by(|(first_index, first_max), (second_index, second_max)| {
            first_max
                .cmp(second_max)
                .then_with(|| pieces[*second_index].id.cmp(pieces[*first_index].id))
        })
        .map(|(index, _)| index)
}

#[allow(clippy::too_many_arguments)]
fn expand_selected_piece(
    parent: &VacancyState,
    hint: &RelaxedPlacement,
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    index: &mut JaguaHazardIndex,
    transition_seed: u64,
    selected_ordinal: usize,
    piece_index: usize,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
    selected_piece_ids: &mut BTreeSet<String>,
    selection_diagnostics: &mut GeneralPersistentVacancyParentSelectionDiagnostics,
    children: &mut Vec<VacancyState>,
) -> Result<(), String> {
    selected_piece_ids.insert(pieces[piece_index].id.to_owned());
    work.diagnostics.selected_piece_slots = work.diagnostics.selected_piece_slots.saturating_add(1);
    if work.diagnostics.selected_piece_slots > work.quotas.max_selected_piece_slots {
        return Err(work.cap("selected-piece slot budget exhausted"));
    }
    work.charge_source_features(pieces[piece_index].polygon.vertex_count().saturating_mul(2))?;
    let angle_seed = derive_seed(
        transition_seed ^ CONFLICT_RUIN_ANGLE_SEED_DOMAIN,
        selected_ordinal,
        piece_index,
    );
    let orientations = conflict_ruin_orientations(pieces[piece_index], hint, angle_seed);
    let diversity_seed = derive_seed(
        transition_seed ^ CONFLICT_RUIN_DIVERSITY_SEED_DOMAIN,
        selected_ordinal,
        piece_index,
    );
    selection_diagnostics
        .slots
        .push(GeneralPersistentVacancySelectionSlotDiagnostics {
            selected_ordinal,
            piece_id: pieces[piece_index].id.to_owned(),
            angle_seed,
            diversity_seed,
        });
    let mut merged = Vec::new();
    for (orientation_ordinal, (rotation_deg, mirrored)) in orientations.into_iter().enumerate() {
        work.diagnostics.orientation_streams =
            work.diagnostics.orientation_streams.saturating_add(1);
        if work.diagnostics.orientation_streams > work.quotas.max_orientation_streams {
            return Err(work.cap("orientation-stream budget exhausted"));
        }
        let orientation = RelaxedPlacement {
            input_index: piece_index,
            rotation_deg,
            mirrored,
            translate_x: 0.0,
            translate_y: 0.0,
        };
        let local_collision = build_collision(pieces[piece_index], &orientation, settings, work)?;
        let position_seed = derive_seed(
            transition_seed ^ CONFLICT_RUIN_POSITION_SEED_DOMAIN,
            selected_ordinal
                .saturating_mul(ORIENTATIONS_PER_PIECE)
                .saturating_add(orientation_ordinal),
            piece_index,
        );
        let proposals = vacancy_positions(
            hint,
            &orientation,
            &local_collision,
            parent,
            settings,
            position_seed,
            work,
        )?;
        let mut ranked = Vec::new();
        for placement in proposals {
            work.diagnostics.hazard_queries = work.diagnostics.hazard_queries.saturating_add(1);
            if work.diagnostics.hazard_queries > work.quotas.max_hazard_queries {
                return Err(work.cap("hazard-query budget exhausted"));
            }
            let pose = hazard_pose(&placement);
            let query = match index.query_unplaced(piece_index, pose) {
                Ok(query) => query,
                Err(error) if error.to_string().contains("query envelope") => continue,
                Err(error) => return Err(format!("persistent vacancy hazard query: {error}")),
            };
            let GeneralHazardQuery::Complete {
                boundary,
                colliding_piece_ids,
            } = query
            else {
                return Err("persistent vacancy unplaced query unexpectedly pruned".to_owned());
            };
            if boundary {
                continue;
            }
            let mut proxy_loss = 0.0;
            for fixed_piece_id in colliding_piece_ids {
                if !parent.active[fixed_piece_id] {
                    return Err("inactive hazard leaked into vacancy query".to_owned());
                }
                work.diagnostics.proxy_pressure_visits =
                    work.diagnostics.proxy_pressure_visits.saturating_add(1);
                if work.diagnostics.proxy_pressure_visits > work.quotas.max_proxy_pressure_visits {
                    return Err(work.cap("proxy-pressure visit budget exhausted"));
                }
                proxy_loss += index
                    .collision_pressure(piece_index, pose, fixed_piece_id)
                    .map_err(|error| format!("persistent vacancy pressure: {error}"))?;
            }
            ranked.push(RankedProposal {
                diversity_key: conflict_ruin_diversity_key(&placement, diversity_seed),
                placement,
                proxy_loss,
                orientation_ordinal,
            });
        }
        ranked.sort_by(compare_proposals);
        ranked.truncate(2);
        merged.extend(ranked);
    }
    merged.sort_by(compare_proposals);
    let mut placement_keys = BTreeSet::new();
    merged.retain(|proposal| placement_keys.insert(placement_key(&proposal.placement)));
    merged.truncate(FINALISTS_PER_PIECE);
    for finalist in merged {
        work.diagnostics.exact_finalist_rows =
            work.diagnostics.exact_finalist_rows.saturating_add(1);
        if work.diagnostics.exact_finalist_rows > work.quotas.max_exact_finalist_rows {
            return Err(work.cap("exact-finalist row budget exhausted"));
        }
        if let Some(child) = exact_vacancy_child(
            parent,
            pieces,
            piece_index,
            finalist.placement,
            settings,
            diagnostics,
            work,
        )? {
            children.push(child);
        }
    }
    Ok(())
}

fn compare_proposals(first: &RankedProposal, second: &RankedProposal) -> Ordering {
    first
        .proxy_loss
        .total_cmp(&second.proxy_loss)
        .then_with(|| first.orientation_ordinal.cmp(&second.orientation_ordinal))
        .then_with(|| first.diversity_key.cmp(&second.diversity_key))
        .then_with(|| placement_key(&first.placement).cmp(&placement_key(&second.placement)))
}

fn exact_vacancy_child(
    parent: &VacancyState,
    pieces: &[GeneralFastPiece<'_>],
    piece_index: usize,
    placement: RelaxedPlacement,
    settings: GeneralFastSettings,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
) -> Result<Option<VacancyState>, String> {
    let child_span = profiling::deep::start(Phase::VacancyExactRows);
    let collision = Arc::new(build_collision(
        pieces[piece_index],
        &placement,
        settings,
        work,
    )?);
    let inset = collision_sheet_inset_mm(settings);
    if !collision.fits_rect(
        inset,
        inset,
        settings.sheet_short_axis_mm - inset,
        settings.sheet_long_axis_mm - inset,
    ) {
        profiling::deep::finish(Phase::VacancyExactRows, child_span);
        return Ok(None);
    }
    let mut blockers = Vec::new();
    for fixed_index in 0..pieces.len() {
        if !parent.active[fixed_index] {
            continue;
        }
        work.charge_experimental_pair()?;
        let fixed = parent.collisions[fixed_index]
            .as_ref()
            .ok_or_else(|| format!("active piece {fixed_index} has no collision"))?;
        if exact_intersection_area(&collision, fixed, work)? > 0.0 {
            blockers.push(fixed_index);
            if blockers.len() > 2 {
                profiling::deep::finish(Phase::VacancyExactRows, child_span);
                return Ok(None);
            }
        }
    }
    blockers.sort_by(|first, second| pieces[*first].id.cmp(pieces[*second].id));
    if let Some(previous) = &parent.last_transition {
        if previous.ejected.contains(&piece_index) && blockers.contains(&previous.inserted) {
            diagnostics.immediate_reversals_rejected =
                diagnostics.immediate_reversals_rejected.saturating_add(1);
            profiling::deep::finish(Phase::VacancyExactRows, child_span);
            return Ok(None);
        }
    }
    let inactive_before = parent.active.iter().filter(|active| !**active).count();
    let inactive_after = inactive_before
        .saturating_sub(1)
        .saturating_add(blockers.len());
    if inactive_after > MAX_INACTIVE_PIECES {
        profiling::deep::finish(Phase::VacancyExactRows, child_span);
        return Ok(None);
    }
    let mut child = parent.clone();
    for blocker in &blockers {
        child.active[*blocker] = false;
        child.collisions[*blocker] = None;
    }
    child.placements[piece_index] = placement;
    child.active[piece_index] = true;
    child.collisions[piece_index] = Some(collision);
    child.last_transition = Some(VacancyTransition {
        inserted: piece_index,
        ejected: blockers.clone(),
    });
    if blockers.is_empty() {
        diagnostics.direct_insertions = diagnostics.direct_insertions.saturating_add(1);
    } else {
        diagnostics.ejection_insertions = diagnostics.ejection_insertions.saturating_add(1);
    }
    profiling::deep::finish(Phase::VacancyExactRows, child_span);
    Ok(Some(child))
}

fn vacancy_positions(
    baseline: &RelaxedPlacement,
    orientation: &RelaxedPlacement,
    local_collision: &PolygonSet,
    parent: &VacancyState,
    settings: GeneralFastSettings,
    seed: u64,
    work: &mut RunWork,
) -> Result<Vec<RelaxedPlacement>, String> {
    let bounds = local_collision
        .bounds()
        .ok_or_else(|| "vacancy orientation has empty collision geometry".to_owned())?;
    let inset = collision_sheet_inset_mm(settings);
    let min_x = inset - bounds.min_x;
    let max_x = settings.sheet_short_axis_mm - inset - bounds.max_x;
    let min_y = inset - bounds.min_y;
    let max_y = settings.sheet_long_axis_mm - inset - bounds.max_y;
    if min_x > max_x || min_y > max_y {
        return Ok(Vec::new());
    }
    let baseline_x = baseline.translate_x.clamp(min_x, max_x);
    let baseline_y = baseline.translate_y.clamp(min_y, max_y);
    let center_x = (min_x + max_x) * 0.5;
    let center_y = (min_y + max_y) * 0.5;
    let mut categories = vec![Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()];
    categories[0].push((baseline_x, baseline_y));
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
            .and_then(|collision| collision.bounds())
            .ok_or_else(|| format!("active piece {fixed_index} has no collision bounds"))?;
        let left = (fixed_bounds.min_x - bounds.max_x).clamp(min_x, max_x);
        let right = (fixed_bounds.max_x - bounds.min_x).clamp(min_x, max_x);
        let below = (fixed_bounds.min_y - bounds.max_y).clamp(min_y, max_y);
        let above = (fixed_bounds.max_y - bounds.min_y).clamp(min_y, max_y);
        categories[2].extend([
            (left, baseline_y),
            (right, baseline_y),
            (baseline_x, below),
            (baseline_x, above),
            (left, below),
            (left, above),
            (right, below),
            (right, above),
        ]);
    }
    let width = (bounds.max_x - bounds.min_x).max(settings.total_padding_mm);
    let height = (bounds.max_y - bounds.min_y).max(settings.total_padding_mm);
    let mut focused_rng = SplitMix64::new(seed ^ 0xF0C5_5EED_0000_0001);
    for _ in 0..16 {
        categories[3].push((
            (baseline_x + focused_rng.range(-2.0 * width, 2.0 * width)).clamp(min_x, max_x),
            (baseline_y + focused_rng.range(-2.0 * height, 2.0 * height)).clamp(min_y, max_y),
        ));
    }
    let mut global_rng = SplitMix64::new(seed ^ 0x610B_A11E_0000_0001);
    for _ in 0..16 {
        categories[4].push((
            global_rng.range(min_x, max_x),
            global_rng.range(min_y, max_y),
        ));
    }
    work.charge_position_sources(categories.iter().map(Vec::len).sum())?;
    let mut category_indices = vec![0usize; categories.len()];
    let mut keys = BTreeSet::new();
    let mut placements = Vec::with_capacity(POSITIONS_PER_ORIENTATION);
    while placements.len() < POSITIONS_PER_ORIENTATION {
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
            if keys.insert(placement_key(&placement)) {
                placements.push(placement);
                if placements.len() == POSITIONS_PER_ORIENTATION {
                    break;
                }
            }
        }
        if !progressed {
            break;
        }
    }
    work.diagnostics.returned_positions = work
        .diagnostics
        .returned_positions
        .saturating_add(placements.len());
    if work.diagnostics.returned_positions > work.quotas.max_returned_positions {
        return Err(work.cap("returned-position budget exhausted"));
    }
    Ok(placements)
}

fn retention_pool(
    mut ordinary: Vec<VacancyState>,
    mut carryovers: Vec<VacancyState>,
    pieces: &[GeneralFastPiece<'_>],
    difficulty: &[PieceDifficulty],
    mode: usize,
) -> (Vec<VacancyState>, usize) {
    if mode != 5 {
        return (ordinary, 0);
    }
    ordinary.append(&mut carryovers);
    ordinary.sort_by(|first, second| compare_states(first, second, pieces, difficulty));
    let before_dedup = ordinary.len();
    ordinary.dedup_by(|first, second| same_state_identity(first, second));
    let deduplicated = before_dedup.saturating_sub(ordinary.len());
    (ordinary, deduplicated)
}

fn enforce_population_width(
    mode: usize,
    terminal_complete: bool,
    retained: usize,
    layer: usize,
) -> Result<(), String> {
    if !terminal_complete && matches!(mode, 5 | 6) && retained != BEAM_WIDTH {
        return Err(format!(
            "persistent vacancy layer {layer} changed dual-objective width: expected {BEAM_WIDTH}, got {retained}"
        ));
    }
    Ok(())
}

fn retain_population(
    sorted: Vec<VacancyState>,
    pieces: &[GeneralFastPiece<'_>],
    difficulty: &[PieceDifficulty],
    mode: usize,
) -> (Vec<VacancyState>, usize) {
    if matches!(
        mode,
        1 | 3 | 7 | 8 | 9 | 10 | 11 | 12 | 14 | 15 | 16 | 17 | 18 | 19
    ) {
        let retained = sorted.into_iter().take(BEAM_WIDTH).collect::<Vec<_>>();
        let signatures = retained
            .iter()
            .map(|state| contact_signature(state, pieces))
            .collect::<BTreeSet<_>>()
            .len();
        return (retained, signatures);
    }
    if matches!(mode, 5 | 6) {
        let mut retained = Vec::with_capacity(BEAM_WIDTH.min(sorted.len()));
        if let Some(area_elite) = sorted.first() {
            retained.push(area_elite.clone());
        }
        if let Some(count_elite) = sorted
            .iter()
            .min_by(|first, second| compare_count_states(first, second, pieces, difficulty))
        {
            if retained
                .iter()
                .all(|state| !same_state_identity(state, count_elite))
            {
                retained.push(count_elite.clone());
            }
        }
        for state in sorted {
            if retained
                .iter()
                .any(|selected| same_state_identity(selected, &state))
            {
                continue;
            }
            retained.push(state);
            if retained.len() == BEAM_WIDTH {
                break;
            }
        }
        let signatures = retained
            .iter()
            .map(|state| contact_signature(state, pieces))
            .collect::<BTreeSet<_>>()
            .len();
        return (retained, signatures);
    }
    let mut signatures = BTreeSet::new();
    let mut selected_indices = BTreeSet::new();
    let mut retained = Vec::new();
    for (index, state) in sorted.iter().enumerate() {
        let signature = contact_signature(state, pieces);
        if signatures.insert(signature) {
            selected_indices.insert(index);
            retained.push(state.clone());
            if retained.len() == BEAM_WIDTH {
                return (retained, signatures.len());
            }
        }
    }
    for (index, state) in sorted.into_iter().enumerate() {
        if selected_indices.contains(&index) {
            continue;
        }
        retained.push(state);
        if retained.len() == BEAM_WIDTH {
            break;
        }
    }
    (retained, signatures.len())
}

fn audit_state(
    state: &VacancyState,
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    complete: bool,
    work: &mut RunWork,
) -> Result<GeneralPlacementMetrics, String> {
    validate_state_structure(state, pieces.len())?;
    if complete != state.active.iter().all(|active| *active) {
        return Err("audit completeness does not match the active set".to_owned());
    }
    // The proxy/exact boundary for the deep operators: a state the proxy tier
    // has already called feasible, now offered to the exact validator. This is
    // the survivor count the frontier trace reports; it is not a hot site (the
    // audit budget caps it) and it is compiled out with the feature.
    crate::quality_trace::proxy_survivors(1);
    work.charge_validator_audit(complete)?;
    let active_pieces = pieces
        .iter()
        .enumerate()
        .filter(|(index, _)| state.active[*index])
        .map(|(_, piece)| *piece)
        .collect::<Vec<_>>();
    let placements = fast_placements(state, pieces, true);
    if active_pieces.len() != placements.len() {
        return Err("filtered audit piece and placement counts disagree".to_owned());
    }
    validate_and_measure_placements(&active_pieces, &placements, settings)
        .map_err(|error| format!("persistent vacancy dual audit: {error}"))
}

fn validate_state_structure(state: &VacancyState, piece_count: usize) -> Result<(), String> {
    if state.placements.len() != piece_count
        || state.active.len() != piece_count
        || state.collisions.len() != piece_count
    {
        return Err("vacancy state vectors do not match the piece count".to_owned());
    }
    let mut seen = vec![false; piece_count];
    for (slot, placement) in state.placements.iter().enumerate() {
        if placement.input_index >= piece_count
            || seen[placement.input_index]
            || placement.input_index != slot
        {
            return Err(
                "vacancy state has an unknown, duplicate, or misplaced stable ID".to_owned(),
            );
        }
        seen[placement.input_index] = true;
        if state.active[slot] != state.collisions[slot].is_some() {
            return Err("vacancy active bits and collision slots disagree".to_owned());
        }
    }
    if seen.iter().any(|present| !*present) {
        return Err("vacancy state is missing a stable ID".to_owned());
    }
    Ok(())
}

fn verify_exact_active_pairs(state: &VacancyState, work: &mut RunWork) -> Result<(), String> {
    for first in 0..state.active.len() {
        if !state.active[first] {
            continue;
        }
        for second in (first + 1)..state.active.len() {
            if !state.active[second] {
                continue;
            }
            work.charge_experimental_pair()?;
            let first_collision = state.collisions[first]
                .as_ref()
                .ok_or_else(|| format!("active piece {first} has no collision"))?;
            let second_collision = state.collisions[second]
                .as_ref()
                .ok_or_else(|| format!("active piece {second} has no collision"))?;
            if exact_intersection_area(first_collision, second_collision, work)? > 0.0 {
                return Err(format!(
                    "initializer active pieces {first} and {second} overlap"
                ));
            }
        }
    }
    Ok(())
}

fn build_collision(
    piece: GeneralFastPiece<'_>,
    placement: &RelaxedPlacement,
    settings: GeneralFastSettings,
    work: &mut RunWork,
) -> Result<PolygonSet, String> {
    work.diagnostics.experimental_collision_builds = work
        .diagnostics
        .experimental_collision_builds
        .saturating_add(1);
    if work.diagnostics.experimental_collision_builds
        > work.quotas.max_experimental_collision_builds
    {
        return Err(work.cap("experimental collision-build budget exhausted"));
    }
    // The build itself is the exact tier, reached by naming the legacy kernel:
    // this polygon is what the deep operators' exact confirmation rows and the
    // publication validator both measure, so no generic substitution may
    // reroute it. The budget bookkeeping around it stays here, because it is
    // this operator's quota, not the kernel's.
    let collision = LEGACY
        .exact_authority()
        .collision_polygon(
            piece.polygon,
            KernelPose {
                rotation_deg: placement.rotation_deg,
                mirrored: placement.mirrored,
                translate_x: placement.translate_x,
                translate_y: placement.translate_y,
            },
            collision_expansion_mm(settings),
        )
        .map_err(|error| format!("persistent vacancy collision geometry: {error}"))?;
    if collision.vertex_count() > MAX_COLLISION_VERTICES {
        return Err(format!(
            "piece {} collision exceeds the {MAX_COLLISION_VERTICES}-vertex experiment cap",
            piece.id
        ));
    }
    work.diagnostics.transformed_collision_vertices = work
        .diagnostics
        .transformed_collision_vertices
        .saturating_add(collision.vertex_count());
    if work.diagnostics.transformed_collision_vertices
        > work.quotas.max_transformed_collision_vertices
    {
        return Err(work.cap("transformed collision-vertex budget exhausted"));
    }
    Ok(collision)
}

fn exact_intersection_area(
    first: &PolygonSet,
    second: &PolygonSet,
    work: &mut RunWork,
) -> Result<f64, String> {
    if bounds_are_disjoint(first, second)? {
        #[cfg(feature = "constructor-census")]
        crate::constructor_census::pair(first, second, false, false);
        return Ok(0.0);
    }
    let input_vertices = first.vertex_count().saturating_add(second.vertex_count());
    if work
        .diagnostics
        .clipper_input_vertices
        .saturating_add(input_vertices)
        > work.quotas.max_clipper_input_vertices
    {
        return Err(work.cap("Clipper input-vertex budget exhausted"));
    }
    let result = first
        .intersection_area_with_complexity(second)
        .map_err(|error| format!("persistent vacancy exact intersection: {error}"))?;
    let next_output = work
        .diagnostics
        .clipper_output_vertices
        .saturating_add(result.output_vertices);
    if next_output > MAX_CLIPPER_OUTPUT_VERTICES {
        return Err(work.cap("Clipper output-vertex budget exhausted"));
    }
    work.diagnostics.clipper_input_vertices = work
        .diagnostics
        .clipper_input_vertices
        .saturating_add(result.input_vertices);
    work.diagnostics.clipper_output_vertices = next_output;
    #[cfg(feature = "constructor-census")]
    crate::constructor_census::pair(first, second, true, result.area_mm2 > 0.0);
    Ok(result.area_mm2)
}

fn bounds_are_disjoint(first: &PolygonSet, second: &PolygonSet) -> Result<bool, String> {
    let first = first
        .bounds()
        .ok_or_else(|| "first exact polygon has no bounds".to_owned())?;
    let second = second
        .bounds()
        .ok_or_else(|| "second exact polygon has no bounds".to_owned())?;
    Ok(grid_key(first.max_x) <= grid_key(second.min_x)
        || grid_key(second.max_x) <= grid_key(first.min_x)
        || grid_key(first.max_y) <= grid_key(second.min_y)
        || grid_key(second.max_y) <= grid_key(first.min_y))
}

/// Anchor of last resort for the from-scratch constructor: every piece sits at
/// its unrotated catalog pose at the strip origin. It carries no positional
/// information - it only gives the construction lane a well-defined identity
/// prior per piece when no parent layout was supplied.
fn identity_relaxed_state(pieces: &[GeneralFastPiece<'_>], target_depth_mm: f64) -> RelaxedState {
    RelaxedState {
        placements: (0..pieces.len())
            .map(|index| RelaxedPlacement {
                input_index: index,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: 0.0,
                translate_y: 0.0,
            })
            .collect(),
        strip_depth_mm: target_depth_mm,
    }
}

fn relaxed_state_from_diagnostics_with_target(
    pieces: &[GeneralFastPiece<'_>],
    placements: &[GeneralCoupledSeparatorPlacementDiagnostics],
    target_depth_mm: f64,
) -> Result<RelaxedState, String> {
    let by_id = pieces
        .iter()
        .enumerate()
        .map(|(index, piece)| (piece.id, index))
        .collect::<BTreeMap<_, _>>();
    let mut slots = vec![None; pieces.len()];
    for placement in placements {
        let index = *by_id
            .get(placement.piece_id.as_str())
            .ok_or_else(|| format!("unknown parent piece {}", placement.piece_id))?;
        if slots[index].is_some() {
            return Err(format!("duplicate parent piece {}", placement.piece_id));
        }
        slots[index] = Some(RelaxedPlacement {
            input_index: index,
            rotation_deg: placement.rotation_deg,
            mirrored: placement.mirrored,
            translate_x: placement.translate_short_axis,
            translate_y: placement.translate_long_axis,
        });
    }
    let placements = slots
        .into_iter()
        .enumerate()
        .map(|(index, placement)| {
            placement.ok_or_else(|| format!("parent is missing piece {}", pieces[index].id))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RelaxedState {
        placements,
        strip_depth_mm: target_depth_mm,
    })
}

fn diagnostic_fast_placements(
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

fn fast_placements(
    state: &VacancyState,
    pieces: &[GeneralFastPiece<'_>],
    active_only: bool,
) -> Vec<GeneralFastPlacement> {
    state
        .placements
        .iter()
        .filter(|placement| !active_only || state.active[placement.input_index])
        .map(|placement| GeneralFastPlacement {
            piece_id: pieces[placement.input_index].id.to_owned(),
            rotation_deg: placement.rotation_deg,
            mirrored: placement.mirrored,
            translate_short_axis: placement.translate_x,
            translate_long_axis: placement.translate_y,
        })
        .collect()
}

fn piece_difficulty(
    piece: GeneralFastPiece<'_>,
    collision: &PolygonSet,
) -> Result<PieceDifficulty, String> {
    let bounds = collision
        .bounds()
        .ok_or_else(|| format!("piece {} collision has no bounds", piece.id))?;
    let points = collision
        .regions()
        .iter()
        .flat_map(|region| {
            std::iter::once(region.outer.points())
                .chain(region.holes.iter().map(|hole| hole.points()))
        })
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    let hull = crate::geometry::convex::compute_convex_hull(&points);
    let hull_area = polygon_area_mm2(&hull.points);
    let expanded_area = collision.area_mm2();
    Ok(PieceDifficulty {
        expanded_area_grid2: doubled_area_grid2(expanded_area),
        hull_deficit_grid2: doubled_area_grid2((hull_area - expanded_area).max(0.0)),
        minimum_side_grid: grid_key((bounds.max_x - bounds.min_x).min(bounds.max_y - bounds.min_y)),
        material_area_grid2: doubled_area_grid2(piece.polygon.area_mm2()),
    })
}

fn polygon_area_mm2(points: &[IrregularPoint]) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .map(|(first, second)| first.x * second.y - second.x * first.y)
        .sum::<f64>()
        .abs()
        / 2.0
}

fn doubled_area_grid2(area_mm2: f64) -> i128 {
    (area_mm2 * 2_000_000.0).round() as i128
}

fn boundary_overflow_grid(
    collision: &PolygonSet,
    settings: GeneralFastSettings,
) -> Result<i64, String> {
    let bounds = collision
        .bounds()
        .ok_or_else(|| "boundary overflow requires non-empty geometry".to_owned())?;
    let inset = collision_sheet_inset_mm(settings);
    let min_x = grid_key(inset);
    let min_y = grid_key(inset);
    let max_x = grid_key(settings.sheet_short_axis_mm - inset);
    let max_y = grid_key(settings.sheet_long_axis_mm - inset);
    Ok([
        min_x.saturating_sub(grid_key(bounds.min_x)),
        min_y.saturating_sub(grid_key(bounds.min_y)),
        grid_key(bounds.max_x).saturating_sub(max_x),
        grid_key(bounds.max_y).saturating_sub(max_y),
    ]
    .into_iter()
    .max()
    .unwrap_or(0))
}

fn selected_inactive_pieces(
    state: &VacancyState,
    pieces: &[GeneralFastPiece<'_>],
    difficulty: &[PieceDifficulty],
    layer: usize,
    mode: usize,
) -> SelectedInactivePieces {
    let mut inactive = (0..state.active.len())
        .filter(|index| !state.active[*index])
        .collect::<Vec<_>>();
    inactive.sort_by(|first, second| {
        difficulty[*second]
            .expanded_area_grid2
            .cmp(&difficulty[*first].expanded_area_grid2)
            .then_with(|| {
                difficulty[*second]
                    .hull_deficit_grid2
                    .cmp(&difficulty[*first].hull_deficit_grid2)
            })
            .then_with(|| {
                difficulty[*second]
                    .minimum_side_grid
                    .cmp(&difficulty[*first].minimum_side_grid)
            })
            .then_with(|| pieces[*first].id.cmp(pieces[*second].id))
    });
    if !matches!(
        mode,
        3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 14 | 15 | 16 | 17 | 18 | 19
    ) || inactive.len() <= 1
    {
        inactive.truncate(SELECTED_PIECES_PER_PARENT);
        return SelectedInactivePieces {
            indices: inactive,
            rotation_start_index: None,
        };
    }
    let hardest = inactive[0];
    let stable = stable_inactive_order(state, pieces);
    let start = layer % stable.len();
    let coverage = (0..stable.len())
        .map(|offset| stable[(start + offset) % stable.len()])
        .find(|index| *index != hardest)
        .expect("more than one inactive piece has a non-hard coverage slot");
    SelectedInactivePieces {
        indices: vec![hardest, coverage],
        rotation_start_index: Some(start),
    }
}

fn stable_inactive_order(state: &VacancyState, pieces: &[GeneralFastPiece<'_>]) -> Vec<usize> {
    let mut inactive = (0..state.active.len())
        .filter(|index| !state.active[*index])
        .collect::<Vec<_>>();
    inactive.sort_by(|first, second| pieces[*first].id.cmp(pieces[*second].id));
    inactive
}

fn scheduler_family(mode: usize) -> &'static str {
    if matches!(
        mode,
        3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 14 | 15 | 16 | 17 | 18 | 19
    ) {
        "hardPlusStatelessRotation"
    } else {
        "twoHardest"
    }
}

fn compare_states(
    first: &VacancyState,
    second: &VacancyState,
    pieces: &[GeneralFastPiece<'_>],
    difficulty: &[PieceDifficulty],
) -> Ordering {
    inactive_area(first, difficulty)
        .cmp(&inactive_area(second, difficulty))
        .then_with(|| {
            inactive_difficulty_sequence(first, pieces, difficulty)
                .cmp(&inactive_difficulty_sequence(second, pieces, difficulty))
        })
        .then_with(|| {
            first
                .active
                .iter()
                .filter(|active| !**active)
                .count()
                .cmp(&second.active.iter().filter(|active| !**active).count())
        })
        .then_with(|| {
            ejected_material_area(first, difficulty).cmp(&ejected_material_area(second, difficulty))
        })
        .then_with(|| {
            first
                .last_transition
                .as_ref()
                .map_or(0, |transition| transition.ejected.len())
                .cmp(
                    &second
                        .last_transition
                        .as_ref()
                        .map_or(0, |transition| transition.ejected.len()),
                )
        })
        .then_with(|| {
            active_frontier_grid(first, pieces).cmp(&active_frontier_grid(second, pieces))
        })
        .then_with(|| compare_state_identity(first, second))
}

fn compare_count_states(
    first: &VacancyState,
    second: &VacancyState,
    pieces: &[GeneralFastPiece<'_>],
    difficulty: &[PieceDifficulty],
) -> Ordering {
    inactive_piece_count(first)
        .cmp(&inactive_piece_count(second))
        .then_with(|| inactive_area(first, difficulty).cmp(&inactive_area(second, difficulty)))
        .then_with(|| {
            inactive_difficulty_sequence(first, pieces, difficulty)
                .cmp(&inactive_difficulty_sequence(second, pieces, difficulty))
        })
        .then_with(|| {
            ejected_material_area(first, difficulty).cmp(&ejected_material_area(second, difficulty))
        })
        .then_with(|| ejected_piece_count(first).cmp(&ejected_piece_count(second)))
        .then_with(|| {
            active_frontier_grid(first, pieces).cmp(&active_frontier_grid(second, pieces))
        })
        .then_with(|| compare_state_identity(first, second))
}

fn inactive_piece_count(state: &VacancyState) -> usize {
    state.active.iter().filter(|active| !**active).count()
}

fn ejected_piece_count(state: &VacancyState) -> usize {
    state
        .last_transition
        .as_ref()
        .map_or(0, |transition| transition.ejected.len())
}

fn inactive_area(state: &VacancyState, difficulty: &[PieceDifficulty]) -> i128 {
    state
        .active
        .iter()
        .enumerate()
        .filter(|(_, active)| !**active)
        .map(|(index, _)| difficulty[index].expanded_area_grid2)
        .sum()
}

fn inactive_difficulty_sequence(
    state: &VacancyState,
    pieces: &[GeneralFastPiece<'_>],
    difficulty: &[PieceDifficulty],
) -> Vec<(i128, i128, i64, String)> {
    let mut inactive = (0..state.active.len())
        .filter(|index| !state.active[*index])
        .collect::<Vec<_>>();
    inactive.sort_by(|first, second| {
        difficulty[*second]
            .expanded_area_grid2
            .cmp(&difficulty[*first].expanded_area_grid2)
            .then_with(|| {
                difficulty[*second]
                    .hull_deficit_grid2
                    .cmp(&difficulty[*first].hull_deficit_grid2)
            })
            .then_with(|| {
                difficulty[*second]
                    .minimum_side_grid
                    .cmp(&difficulty[*first].minimum_side_grid)
            })
            .then_with(|| pieces[*first].id.cmp(pieces[*second].id))
    });
    inactive
        .into_iter()
        .map(|index| {
            (
                difficulty[index].expanded_area_grid2,
                difficulty[index].hull_deficit_grid2,
                difficulty[index].minimum_side_grid,
                pieces[index].id.to_owned(),
            )
        })
        .collect()
}

fn ejected_material_area(state: &VacancyState, difficulty: &[PieceDifficulty]) -> i128 {
    state.last_transition.as_ref().map_or(0, |transition| {
        transition
            .ejected
            .iter()
            .map(|index| difficulty[*index].material_area_grid2)
            .sum()
    })
}

fn active_frontier_grid(state: &VacancyState, pieces: &[GeneralFastPiece<'_>]) -> i64 {
    state
        .placements
        .iter()
        .filter(|placement| state.active[placement.input_index])
        .filter_map(|placement| {
            pieces[placement.input_index]
                .polygon
                .transformed(
                    placement.rotation_deg,
                    placement.mirrored,
                    placement.translate_x,
                    placement.translate_y,
                )
                .ok()
                .and_then(|polygon| polygon.bounds())
                .map(|bounds| grid_key(bounds.max_y))
        })
        .max()
        .unwrap_or(i64::MIN)
}

fn state_identity(state: &VacancyState) -> VacancyStateIdentity {
    VacancyStateIdentity {
        active_placements: state
            .placements
            .iter()
            .filter(|placement| state.active[placement.input_index])
            .map(placement_key)
            .collect(),
        inactive: (0..state.active.len())
            .filter(|index| !state.active[*index])
            .collect(),
        last_transition: state.last_transition.clone(),
    }
}

fn compare_state_identity(first: &VacancyState, second: &VacancyState) -> Ordering {
    first
        .placements
        .iter()
        .filter(|placement| first.active[placement.input_index])
        .map(placement_key)
        .cmp(
            second
                .placements
                .iter()
                .filter(|placement| second.active[placement.input_index])
                .map(placement_key),
        )
        .then_with(|| {
            first
                .active
                .iter()
                .enumerate()
                .filter(|(_, active)| !**active)
                .map(|(index, _)| index)
                .cmp(
                    second
                        .active
                        .iter()
                        .enumerate()
                        .filter(|(_, active)| !**active)
                        .map(|(index, _)| index),
                )
        })
        .then_with(|| first.last_transition.cmp(&second.last_transition))
}

fn same_state_identity(first: &VacancyState, second: &VacancyState) -> bool {
    compare_state_identity(first, second).is_eq()
}

fn state_fingerprint(state: &VacancyState, pieces: &[GeneralFastPiece<'_>]) -> String {
    state_digest(state, pieces)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn population_hash(population: &[VacancyState], pieces: &[GeneralFastPiece<'_>]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"persistent-vacancy-population-v1\0");
    digest.update((population.len() as u32).to_be_bytes());
    for state in population {
        digest.update(state_digest(state, pieces));
    }
    format!("{:x}", digest.finalize())
}

fn child_order_hash(children: &[VacancyState], pieces: &[GeneralFastPiece<'_>]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"persistent-vacancy-child-order-v1\0");
    digest.update((children.len() as u32).to_be_bytes());
    for state in children {
        digest.update([u8::from(state.active.iter().all(|active| *active))]);
        digest.update(state_digest(state, pieces));
    }
    format!("{:x}", digest.finalize())
}

fn population_elites<'a>(
    population: &'a [VacancyState],
    pieces: &[GeneralFastPiece<'_>],
    difficulty: &[PieceDifficulty],
) -> (&'a VacancyState, &'a VacancyState) {
    let area = population
        .iter()
        .min_by(|first, second| compare_states(first, second, pieces, difficulty))
        .expect("an elite population is non-empty");
    let count = population
        .iter()
        .min_by(|first, second| compare_count_states(first, second, pieces, difficulty))
        .expect("an elite population is non-empty");
    (area, count)
}

fn distinct_elite_states(
    population: &[VacancyState],
    pieces: &[GeneralFastPiece<'_>],
    difficulty: &[PieceDifficulty],
) -> Vec<VacancyState> {
    let (area, count) = population_elites(population, pieces, difficulty);
    let mut elites = vec![area.clone()];
    if !same_state_identity(area, count) {
        elites.push(count.clone());
    }
    elites
}

fn elite_snapshot(
    state: &VacancyState,
    pieces: &[GeneralFastPiece<'_>],
    difficulty: &[PieceDifficulty],
) -> EliteSnapshot {
    EliteSnapshot {
        fingerprint: state_fingerprint(state, pieces),
        inactive_piece_count: inactive_piece_count(state),
        inactive_area_grid2: inactive_area(state, difficulty),
        inactive_difficulty_sequence: inactive_difficulty_sequence(state, pieces, difficulty),
        ejected_material_area_grid2: ejected_material_area(state, difficulty),
        ejected_piece_count: ejected_piece_count(state),
        active_frontier_grid: active_frontier_grid(state, pieces),
        identity: state_identity(state),
    }
}

fn compare_area_snapshots(first: &EliteSnapshot, second: &EliteSnapshot) -> Ordering {
    first
        .inactive_area_grid2
        .cmp(&second.inactive_area_grid2)
        .then_with(|| {
            first
                .inactive_difficulty_sequence
                .cmp(&second.inactive_difficulty_sequence)
        })
        .then_with(|| first.inactive_piece_count.cmp(&second.inactive_piece_count))
        .then_with(|| {
            first
                .ejected_material_area_grid2
                .cmp(&second.ejected_material_area_grid2)
        })
        .then_with(|| first.ejected_piece_count.cmp(&second.ejected_piece_count))
        .then_with(|| first.active_frontier_grid.cmp(&second.active_frontier_grid))
        .then_with(|| first.identity.cmp(&second.identity))
}

fn compare_count_snapshots(first: &EliteSnapshot, second: &EliteSnapshot) -> Ordering {
    first
        .inactive_piece_count
        .cmp(&second.inactive_piece_count)
        .then_with(|| first.inactive_area_grid2.cmp(&second.inactive_area_grid2))
        .then_with(|| {
            first
                .inactive_difficulty_sequence
                .cmp(&second.inactive_difficulty_sequence)
        })
        .then_with(|| {
            first
                .ejected_material_area_grid2
                .cmp(&second.ejected_material_area_grid2)
        })
        .then_with(|| first.ejected_piece_count.cmp(&second.ejected_piece_count))
        .then_with(|| first.active_frontier_grid.cmp(&second.active_frontier_grid))
        .then_with(|| first.identity.cmp(&second.identity))
}

fn update_best_area(best: &mut Option<EliteSnapshot>, candidate: &EliteSnapshot) -> bool {
    if best.as_ref().map_or(true, |current| {
        compare_area_snapshots(candidate, current).is_lt()
    }) {
        *best = Some(candidate.clone());
        return true;
    }
    false
}

fn update_best_count(best: &mut Option<EliteSnapshot>, candidate: &EliteSnapshot) -> bool {
    if best.as_ref().map_or(true, |current| {
        compare_count_snapshots(candidate, current).is_lt()
    }) {
        *best = Some(candidate.clone());
        return true;
    }
    false
}

fn parent_seed_key(state: &VacancyState, pieces: &[GeneralFastPiece<'_>]) -> u64 {
    let digest = state_digest(state, pieces);
    u64::from_be_bytes(digest[..8].try_into().expect("SHA-256 has eight bytes"))
}

fn state_digest(state: &VacancyState, pieces: &[GeneralFastPiece<'_>]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"persistent-vacancy-state-v1\0");
    let active_placement_count = state
        .placements
        .iter()
        .filter(|placement| state.active[placement.input_index])
        .count();
    digest.update((active_placement_count as u32).to_be_bytes());
    for (index, angle, mirrored, x, y) in state
        .placements
        .iter()
        .filter(|placement| state.active[placement.input_index])
        .map(placement_key)
    {
        update_framed_id(&mut digest, pieces[index].id);
        digest.update(angle.to_be_bytes());
        digest.update([u8::from(mirrored)]);
        digest.update(x.to_be_bytes());
        digest.update(y.to_be_bytes());
    }
    let inactive_count = inactive_piece_count(state);
    digest.update((inactive_count as u32).to_be_bytes());
    for index in (0..state.active.len()).filter(|index| !state.active[*index]) {
        update_framed_id(&mut digest, pieces[index].id);
    }
    match &state.last_transition {
        None => digest.update([0]),
        Some(transition) => {
            digest.update([1]);
            update_framed_id(&mut digest, pieces[transition.inserted].id);
            digest.update((transition.ejected.len() as u32).to_be_bytes());
            for index in &transition.ejected {
                update_framed_id(&mut digest, pieces[*index].id);
            }
        }
    }
    digest.finalize().into()
}

fn update_framed_id(digest: &mut Sha256, id: &str) {
    digest.update((id.len() as u32).to_be_bytes());
    digest.update(id.as_bytes());
}

fn active_ids(state: &VacancyState, pieces: &[GeneralFastPiece<'_>]) -> Vec<String> {
    (0..state.active.len())
        .filter(|index| state.active[*index])
        .map(|index| pieces[index].id.to_owned())
        .collect()
}

fn id_order_hash(indices: &[usize], pieces: &[GeneralFastPiece<'_>]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"persistent-vacancy-inactive-order-v1\0");
    digest.update((indices.len() as u32).to_be_bytes());
    for index in indices {
        update_framed_id(&mut digest, pieces[*index].id);
    }
    format!("{:x}", digest.finalize())
}

fn contact_signature(state: &VacancyState, pieces: &[GeneralFastPiece<'_>]) -> ContactSignature {
    let active_ids = active_ids(state, pieces);
    let mut edges = Vec::new();
    for first in 0..state.active.len() {
        if !state.active[first] {
            continue;
        }
        for second in (first + 1)..state.active.len() {
            if !state.active[second] {
                continue;
            }
            let Some(first_bounds) = state.collisions[first]
                .as_ref()
                .and_then(|collision| collision.bounds())
            else {
                continue;
            };
            let Some(second_bounds) = state.collisions[second]
                .as_ref()
                .and_then(|collision| collision.bounds())
            else {
                continue;
            };
            let x_contact = (grid_key(first_bounds.max_x) == grid_key(second_bounds.min_x)
                || grid_key(second_bounds.max_x) == grid_key(first_bounds.min_x))
                && grid_key(first_bounds.max_y).min(grid_key(second_bounds.max_y))
                    > grid_key(first_bounds.min_y).max(grid_key(second_bounds.min_y));
            let y_contact = (grid_key(first_bounds.max_y) == grid_key(second_bounds.min_y)
                || grid_key(second_bounds.max_y) == grid_key(first_bounds.min_y))
                && grid_key(first_bounds.max_x).min(grid_key(second_bounds.max_x))
                    > grid_key(first_bounds.min_x).max(grid_key(second_bounds.min_x));
            let axis = match (x_contact, y_contact) {
                (true, false) => Some(0),
                (false, true) => Some(1),
                _ => None,
            };
            if let Some(axis) = axis {
                let (first_id, second_id) = if pieces[first].id <= pieces[second].id {
                    (pieces[first].id, pieces[second].id)
                } else {
                    (pieces[second].id, pieces[first].id)
                };
                edges.push(ContactEdge {
                    first_id: first_id.to_owned(),
                    second_id: second_id.to_owned(),
                    axis,
                });
            }
        }
    }
    edges.sort();
    ContactSignature { active_ids, edges }
}

#[cfg(test)]
fn contact_signature_hash(signature: &ContactSignature) -> String {
    let mut digest = Sha256::new();
    digest.update(b"persistent-vacancy-contact-v1\0");
    digest.update((signature.active_ids.len() as u32).to_be_bytes());
    for id in &signature.active_ids {
        update_framed_id(&mut digest, id);
    }
    digest.update((signature.edges.len() as u32).to_be_bytes());
    for edge in &signature.edges {
        update_framed_id(&mut digest, &edge.first_id);
        update_framed_id(&mut digest, &edge.second_id);
        digest.update([edge.axis]);
    }
    format!("{:x}", digest.finalize())
}

fn charge_retained_memory(
    population: &[VacancyState],
    archive_bytes: usize,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    pending_layer: &GeneralPersistentVacancyLayerDiagnostics,
    work: &mut RunWork,
) -> Result<(), String> {
    diagnostics.layers.reserve(1);
    let legacy_state_bytes = legacy_state_slice_bytes(population);
    let state_bytes = state_slice_bytes(population)
        .saturating_add(population.len().saturating_mul(size_of::<VacancyState>()));
    let diagnostic_bytes = persistent_diagnostic_bytes(diagnostics)
        .saturating_add(layer_diagnostic_heap_bytes(pending_layer));
    let total_bytes = state_bytes
        .saturating_add(diagnostic_bytes)
        .saturating_add(archive_bytes);
    work.diagnostics.retained_peak_bytes =
        work.diagnostics.retained_peak_bytes.max(legacy_state_bytes);
    work.diagnostics.selector_diagnostic_peak_bytes = work
        .diagnostics
        .selector_diagnostic_peak_bytes
        .max(diagnostic_bytes);
    work.diagnostics.total_retained_peak_bytes =
        work.diagnostics.total_retained_peak_bytes.max(total_bytes);
    if total_bytes > MAX_RETAINED_BYTES {
        return Err(work.cap("retained-memory budget exhausted"));
    }
    Ok(())
}

fn preflight_live_memory(
    entering_population: &Vec<VacancyState>,
    ordinary_live_state_bytes: usize,
    carryover_live_state_bytes: usize,
    retained_clone_bytes: usize,
    combined_pool_backing_bytes: usize,
    archive_bytes: usize,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    pending_layer: &GeneralPersistentVacancyLayerDiagnostics,
    work: &mut RunWork,
) -> Result<(), String> {
    diagnostics.layers.reserve(1);
    let diagnostic_bytes = persistent_diagnostic_bytes(diagnostics)
        .saturating_add(layer_diagnostic_heap_bytes(pending_layer));
    let total_bytes = state_vec_bytes(entering_population)
        .saturating_add(ordinary_live_state_bytes)
        .saturating_add(carryover_live_state_bytes)
        .saturating_add(retained_clone_bytes)
        .saturating_add(combined_pool_backing_bytes)
        .saturating_add(archive_bytes)
        .saturating_add(diagnostic_bytes);
    work.diagnostics.selector_diagnostic_peak_bytes = work
        .diagnostics
        .selector_diagnostic_peak_bytes
        .max(diagnostic_bytes);
    work.diagnostics.total_retained_peak_bytes =
        work.diagnostics.total_retained_peak_bytes.max(total_bytes);
    if total_bytes > MAX_RETAINED_BYTES {
        return Err(work.cap("live-pool memory budget exhausted"));
    }
    Ok(())
}

fn preflight_raw_live_memory(
    entering_population: &Vec<VacancyState>,
    ordinary_live_state_bytes: usize,
    carryover_live_state_bytes: usize,
    retained_clone_bytes: usize,
    combined_pool_backing_bytes: usize,
    archive_bytes: usize,
    selected_piece_ids: &[String],
    parent_selections: &[GeneralPersistentVacancyParentSelectionDiagnostics],
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
) -> Result<(), String> {
    const ELITE_DIAGNOSTIC_HEAP_UPPER_BOUND: usize = 8 * 1024;

    diagnostics.layers.reserve(1);
    let pending_selector_bytes = selected_piece_ids
        .len()
        .saturating_mul(size_of::<String>())
        .saturating_add(
            selected_piece_ids
                .iter()
                .map(String::capacity)
                .sum::<usize>(),
        )
        .saturating_add(
            parent_selections
                .len()
                .saturating_mul(size_of::<GeneralPersistentVacancyParentSelectionDiagnostics>()),
        )
        .saturating_add(
            parent_selections
                .iter()
                .map(parent_selection_heap_bytes)
                .sum::<usize>(),
        )
        .saturating_add(ELITE_DIAGNOSTIC_HEAP_UPPER_BOUND);
    let diagnostic_bytes =
        persistent_diagnostic_bytes(diagnostics).saturating_add(pending_selector_bytes);
    let total_bytes = state_vec_bytes(entering_population)
        .saturating_add(ordinary_live_state_bytes)
        .saturating_add(carryover_live_state_bytes)
        .saturating_add(retained_clone_bytes)
        .saturating_add(combined_pool_backing_bytes)
        .saturating_add(archive_bytes)
        .saturating_add(diagnostic_bytes);
    work.diagnostics.selector_diagnostic_peak_bytes = work
        .diagnostics
        .selector_diagnostic_peak_bytes
        .max(diagnostic_bytes);
    work.diagnostics.total_retained_peak_bytes =
        work.diagnostics.total_retained_peak_bytes.max(total_bytes);
    if total_bytes > MAX_RETAINED_BYTES {
        return Err(work.cap("pre-deduplication live-pool memory budget exhausted"));
    }
    Ok(())
}

fn state_vec_bytes(states: &Vec<VacancyState>) -> usize {
    states
        .capacity()
        .saturating_mul(size_of::<VacancyState>())
        .saturating_add(state_slice_bytes(states))
}

fn state_slice_bytes(states: &[VacancyState]) -> usize {
    states.iter().map(state_heap_bytes).sum()
}

fn legacy_state_slice_bytes(states: &[VacancyState]) -> usize {
    states.iter().map(legacy_state_heap_bytes).sum()
}

fn legacy_state_heap_bytes(state: &VacancyState) -> usize {
    state.placements.capacity() * size_of::<RelaxedPlacement>()
        + state.active.capacity() * size_of::<bool>()
        + state.collisions.capacity() * size_of::<Option<Arc<PolygonSet>>>()
        + state
            .collisions
            .iter()
            .filter_map(Option::as_ref)
            .map(|collision| {
                collision.vertex_count() * size_of::<IrregularPoint>() + size_of::<PolygonSet>()
            })
            .sum::<usize>()
}

fn state_heap_bytes(state: &VacancyState) -> usize {
    legacy_state_heap_bytes(state)
        + state.last_transition.as_ref().map_or(0, |transition| {
            transition.ejected.capacity() * size_of::<usize>()
        })
}

fn generation_work_snapshot(
    mut diagnostics: GeneralPersistentVacancyWorkDiagnostics,
) -> GeneralPersistentVacancyWorkDiagnostics {
    diagnostics.retained_peak_bytes = 0;
    diagnostics.selector_diagnostic_peak_bytes = 0;
    diagnostics.total_retained_peak_bytes = 0;
    diagnostics
}

fn work_delta(
    after: GeneralPersistentVacancyWorkDiagnostics,
    before: GeneralPersistentVacancyWorkDiagnostics,
) -> GeneralPersistentVacancyWorkDiagnostics {
    GeneralPersistentVacancyWorkDiagnostics {
        selected_piece_slots: after
            .selected_piece_slots
            .saturating_sub(before.selected_piece_slots),
        orientation_streams: after
            .orientation_streams
            .saturating_sub(before.orientation_streams),
        source_feature_visits: after
            .source_feature_visits
            .saturating_sub(before.source_feature_visits),
        position_source_attempts: after
            .position_source_attempts
            .saturating_sub(before.position_source_attempts),
        returned_positions: after
            .returned_positions
            .saturating_sub(before.returned_positions),
        hazard_queries: after.hazard_queries.saturating_sub(before.hazard_queries),
        proxy_pressure_visits: after
            .proxy_pressure_visits
            .saturating_sub(before.proxy_pressure_visits),
        exact_finalist_rows: after
            .exact_finalist_rows
            .saturating_sub(before.exact_finalist_rows),
        experimental_collision_builds: after
            .experimental_collision_builds
            .saturating_sub(before.experimental_collision_builds),
        validator_collision_builds: after
            .validator_collision_builds
            .saturating_sub(before.validator_collision_builds),
        experimental_pair_visits: after
            .experimental_pair_visits
            .saturating_sub(before.experimental_pair_visits),
        validator_pair_visits: after
            .validator_pair_visits
            .saturating_sub(before.validator_pair_visits),
        transformed_collision_vertices: after
            .transformed_collision_vertices
            .saturating_sub(before.transformed_collision_vertices),
        clipper_input_vertices: after
            .clipper_input_vertices
            .saturating_sub(before.clipper_input_vertices),
        clipper_output_vertices: after
            .clipper_output_vertices
            .saturating_sub(before.clipper_output_vertices),
        partial_audits: after.partial_audits.saturating_sub(before.partial_audits),
        complete_audits: after.complete_audits.saturating_sub(before.complete_audits),
        retained_peak_bytes: 0,
        selector_diagnostic_peak_bytes: 0,
        total_retained_peak_bytes: 0,
    }
}

fn persistent_diagnostic_bytes(diagnostics: &GeneralPersistentVacancyDiagnostics) -> usize {
    option_string_bytes(&diagnostics.parent_fingerprint)
        .saturating_add(option_string_bytes(&diagnostics.initial_state_fingerprint))
        .saturating_add(string_vec_bytes(
            &diagnostics.initial_active_piece_ids,
            diagnostics.initial_active_piece_ids.capacity(),
        ))
        .saturating_add(string_vec_bytes(
            &diagnostics.initial_inactive_piece_ids,
            diagnostics.initial_inactive_piece_ids.capacity(),
        ))
        .saturating_add(option_string_bytes(
            &diagnostics.initial_inactive_order_hash,
        ))
        .saturating_add(option_string_bytes(
            &diagnostics.final_placement_fingerprint,
        ))
        .saturating_add(
            diagnostics.final_placements.capacity()
                * size_of::<GeneralCoupledSeparatorPlacementDiagnostics>(),
        )
        .saturating_add(
            diagnostics
                .final_placements
                .iter()
                .map(|placement| placement.piece_id.capacity())
                .sum::<usize>(),
        )
        .saturating_add(
            diagnostics.layers.capacity() * size_of::<GeneralPersistentVacancyLayerDiagnostics>(),
        )
        .saturating_add(
            diagnostics
                .layers
                .iter()
                .map(layer_diagnostic_heap_bytes)
                .sum::<usize>(),
        )
        .saturating_add(option_string_bytes(&diagnostics.cap_exhausted))
        .saturating_add(option_string_bytes(&diagnostics.failure_reason))
        .saturating_add(option_string_bytes(&diagnostics.parent_source))
        .saturating_add(diagnostics.archive.as_ref().map_or(0, |archive| {
            archive
                .revival_policy
                .capacity()
                .saturating_add(option_string_bytes(
                    &archive.final_archived_area_fingerprint,
                ))
                .saturating_add(option_string_bytes(
                    &archive.final_archived_count_fingerprint,
                ))
        }))
}

fn layer_diagnostic_heap_bytes(layer: &GeneralPersistentVacancyLayerDiagnostics) -> usize {
    string_vec_bytes(
        &layer.selected_piece_ids,
        layer.selected_piece_ids.capacity(),
    )
    .saturating_add(
        layer.parent_selections.capacity()
            * size_of::<GeneralPersistentVacancyParentSelectionDiagnostics>(),
    )
    .saturating_add(
        layer
            .parent_selections
            .iter()
            .map(parent_selection_heap_bytes)
            .sum::<usize>(),
    )
    .saturating_add(string_vec_bytes(
        &layer.best_inactive_piece_ids,
        layer.best_inactive_piece_ids.capacity(),
    ))
    .saturating_add(layer.best_inactive_area_grid2.capacity())
    .saturating_add(layer.best_state_fingerprint.capacity())
    .saturating_add(
        layer
            .elite
            .as_ref()
            .map_or(0, elite_layer_diagnostic_heap_bytes),
    )
    .saturating_add(
        layer
            .archive
            .as_ref()
            .map_or(0, archive_layer_diagnostic_heap_bytes),
    )
}

fn archive_layer_diagnostic_heap_bytes(
    archive: &GeneralPersistentVacancyArchiveLayerDiagnostics,
) -> usize {
    // Heap buffers only: the inline struct storage is already covered by the
    // containing layer row's capacity term.
    (archive.revival_kind.as_ref().map_or(0, String::capacity))
        .saturating_add(
            archive
                .revived_state_fingerprint
                .as_ref()
                .map_or(0, String::capacity),
        )
        .saturating_add(
            archive
                .replaced_state_fingerprint
                .as_ref()
                .map_or(0, String::capacity),
        )
        .saturating_add(archive.skipped_reason.as_ref().map_or(0, String::capacity))
}

fn elite_layer_diagnostic_heap_bytes(
    elite: &GeneralPersistentVacancyEliteLayerDiagnostics,
) -> usize {
    elite
        .entering_population_hash
        .capacity()
        .saturating_add(elite.ordinary_child_order_hash.capacity())
        .saturating_add(elite.complete_candidate_order_hash.capacity())
        .saturating_add(elite.area_elite_fingerprint.capacity())
        .saturating_add(elite.area_elite_inactive_area_grid2.capacity())
        .saturating_add(elite.count_elite_fingerprint.capacity())
        .saturating_add(elite.count_elite_inactive_area_grid2.capacity())
        .saturating_add(elite.best_ever_area_elite_fingerprint.capacity())
        .saturating_add(elite.best_ever_area_elite_inactive_area_grid2.capacity())
        .saturating_add(elite.best_ever_count_elite_fingerprint.capacity())
        .saturating_add(elite.best_ever_count_elite_inactive_area_grid2.capacity())
        .saturating_add(string_vec_bytes(
            &elite.offered_carryover_fingerprints,
            elite.offered_carryover_fingerprints.capacity(),
        ))
        .saturating_add(string_vec_bytes(
            &elite.retained_carryover_fingerprints,
            elite.retained_carryover_fingerprints.capacity(),
        ))
        .saturating_add(string_vec_bytes(
            &elite.expanded_carryover_fingerprints,
            elite.expanded_carryover_fingerprints.capacity(),
        ))
}

fn parent_selection_heap_bytes(
    selection: &GeneralPersistentVacancyParentSelectionDiagnostics,
) -> usize {
    selection
        .parent_state_fingerprint
        .capacity()
        .saturating_add(selection.inactive_order_hash.capacity())
        .saturating_add(selection.scheduler_family.capacity())
        .saturating_add(selection.hardest_piece_id.capacity())
        .saturating_add(
            selection
                .coverage_piece_id
                .as_ref()
                .map_or(0, String::capacity),
        )
        .saturating_add(
            selection
                .relocated_piece_id
                .as_ref()
                .map_or(0, String::capacity),
        )
        .saturating_add(
            selection.slots.capacity()
                * size_of::<GeneralPersistentVacancySelectionSlotDiagnostics>(),
        )
        .saturating_add(
            selection
                .slots
                .iter()
                .map(|slot| slot.piece_id.capacity())
                .sum::<usize>(),
        )
}

fn string_vec_bytes(strings: &[String], capacity: usize) -> usize {
    capacity
        .saturating_mul(size_of::<String>())
        .saturating_add(strings.iter().map(String::capacity).sum::<usize>())
}

fn option_string_bytes(value: &Option<String>) -> usize {
    value.as_ref().map_or(0, String::capacity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::IrregularPoint;
    use crate::search::general_micro_legalization::{
        global_legalization_worst_case_collision_builds, global_legalization_worst_case_pair_visits,
    };

    fn square(size: f64) -> PolygonSet {
        PolygonSet::from_outer(vec![
            IrregularPoint::new(0.0, 0.0),
            IrregularPoint::new(size, 0.0),
            IrregularPoint::new(size, size),
            IrregularPoint::new(0.0, size),
        ])
        .unwrap()
    }

    fn state_with_two_squares(second_x: f64, second_y: f64) -> (Vec<PolygonSet>, VacancyState) {
        let polygons = vec![square(10.0), square(10.0)];
        let placements = vec![
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
                translate_x: second_x,
                translate_y: second_y,
            },
        ];
        let collisions = vec![
            Some(Arc::new(polygons[0].clone())),
            Some(Arc::new(
                polygons[1]
                    .transformed(0.0, false, second_x, second_y)
                    .unwrap(),
            )),
        ];
        (
            polygons,
            VacancyState {
                placements,
                active: vec![true, true],
                collisions,
                last_transition: None,
            },
        )
    }

    fn selector_ids(ids: &[&str], layer: usize, mode: usize) -> Vec<String> {
        let polygons = ids.iter().map(|_| square(10.0)).collect::<Vec<_>>();
        let pieces = ids
            .iter()
            .enumerate()
            .map(|(index, id)| GeneralFastPiece {
                id,
                polygon: &polygons[index],
                allow_rotation: true,
                allow_mirror: true,
            })
            .collect::<Vec<_>>();
        let placements = ids
            .iter()
            .enumerate()
            .map(|(index, _)| RelaxedPlacement {
                input_index: index,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: 0.0,
                translate_y: 0.0,
            })
            .collect::<Vec<_>>();
        let state = VacancyState {
            placements,
            active: vec![false; ids.len()],
            collisions: vec![None; ids.len()],
            last_transition: None,
        };
        let difficulty = ids
            .iter()
            .map(|id| {
                let rank = match *id {
                    "b" => 100,
                    "d" => 80,
                    "c" => 60,
                    _ => 40,
                };
                PieceDifficulty {
                    expanded_area_grid2: rank,
                    hull_deficit_grid2: rank,
                    minimum_side_grid: rank as i64,
                    material_area_grid2: rank,
                }
            })
            .collect::<Vec<_>>();
        selected_inactive_pieces(&state, &pieces, &difficulty, layer, mode)
            .indices
            .into_iter()
            .map(|index| pieces[index].id.to_owned())
            .collect()
    }

    fn state_with_active_mask(active: Vec<bool>) -> VacancyState {
        let placements = active
            .iter()
            .enumerate()
            .map(|(index, _)| RelaxedPlacement {
                input_index: index,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: index as f64 * 20.0,
                translate_y: 0.0,
            })
            .collect::<Vec<_>>();
        VacancyState {
            collisions: vec![None; active.len()],
            placements,
            active,
            last_transition: None,
        }
    }

    fn test_difficulties(areas: &[i128]) -> Vec<PieceDifficulty> {
        areas
            .iter()
            .map(|area| PieceDifficulty {
                expanded_area_grid2: *area,
                hull_deficit_grid2: *area,
                minimum_side_grid: *area as i64,
                material_area_grid2: *area,
            })
            .collect()
    }

    #[test]
    fn semantic_identity_ignores_diagnostic_history_and_inactive_pose() {
        let (_, mut first) = state_with_two_squares(10.0, 0.0);
        first.active[1] = false;
        first.collisions[1] = None;
        let mut second = first.clone();
        second.placements[1].translate_x = 999.0;
        assert_eq!(state_identity(&first), state_identity(&second));
    }

    #[test]
    fn last_transition_remains_part_of_semantic_identity() {
        let (_, mut first) = state_with_two_squares(10.0, 0.0);
        let mut second = first.clone();
        first.last_transition = Some(VacancyTransition {
            inserted: 0,
            ejected: vec![1],
        });
        second.last_transition = Some(VacancyTransition {
            inserted: 1,
            ejected: vec![0],
        });
        assert_ne!(state_identity(&first), state_identity(&second));
    }

    #[test]
    fn contact_signature_distinguishes_axis_and_ignores_corner() {
        let (polygons, x_state) = state_with_two_squares(10.0, 0.0);
        let pieces = [
            GeneralFastPiece {
                id: "a",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "b",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let (_, y_state) = state_with_two_squares(0.0, 10.0);
        let (_, corner_state) = state_with_two_squares(10.0, 10.0);
        assert_eq!(contact_signature(&x_state, &pieces).edges[0].axis, 0);
        assert_eq!(contact_signature(&y_state, &pieces).edges[0].axis, 1);
        assert!(contact_signature(&corner_state, &pieces).edges.is_empty());
        assert_ne!(
            contact_signature_hash(&contact_signature(&x_state, &pieces)),
            contact_signature_hash(&contact_signature(&y_state, &pieces))
        );
    }

    #[test]
    fn shared_state_seed_does_not_depend_on_population_ordinal() {
        let (polygons, state) = state_with_two_squares(10.0, 0.0);
        let pieces = [
            GeneralFastPiece {
                id: "a",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "b",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let seed = parent_seed_key(&state, &pieces);
        assert_eq!(
            derive_seed(PERSISTENT_VACANCY_SEED_DOMAIN ^ seed, 4, 0),
            derive_seed(PERSISTENT_VACANCY_SEED_DOMAIN ^ seed, 4, 0)
        );
    }

    #[test]
    fn selector_families_share_streams_across_retention_modes() {
        let ids = ["d", "a", "c", "b"];
        assert_eq!(selector_ids(&ids, 2, 1), selector_ids(&ids, 2, 2));
        assert_eq!(selector_ids(&ids, 2, 3), selector_ids(&ids, 2, 4));
        assert_eq!(selector_ids(&ids, 2, 3), selector_ids(&ids, 2, 5));
        assert_eq!(selector_ids(&ids, 2, 3), selector_ids(&ids, 2, 6));
    }

    #[test]
    fn stateless_rotation_is_stable_under_input_storage_permutation() {
        let first = selector_ids(&["d", "a", "c", "b"], 2, 3);
        let second = selector_ids(&["b", "c", "a", "d"], 2, 3);
        assert_eq!(first, second);
    }

    #[test]
    fn stateless_rotation_singleton_has_only_the_hard_slot() {
        assert_eq!(selector_ids(&["b"], 7, 3), vec!["b"]);
    }

    #[test]
    fn stateless_rotation_skips_hard_slot_and_covers_a_fixed_set() {
        let ids = ["d", "a", "c", "b"];
        let mut coverage = BTreeSet::new();
        for layer in 0..ids.len() {
            let selected = selector_ids(&ids, layer, 3);
            assert_eq!(selected[0], "b");
            assert_ne!(selected[1], "b");
            coverage.insert(selected[1].clone());
        }
        assert_eq!(
            coverage,
            BTreeSet::from(["a".to_owned(), "c".to_owned(), "d".to_owned()])
        );
    }

    #[test]
    fn selector_diagnostics_are_accounted_separately_from_state_memory() {
        let mut diagnostics = GeneralPersistentVacancyDiagnostics {
            initial_active_piece_ids: vec!["a".to_owned()],
            ..GeneralPersistentVacancyDiagnostics::default()
        };
        let pending = GeneralPersistentVacancyLayerDiagnostics {
            selected_piece_ids: vec!["a".to_owned()],
            parent_selections: vec![GeneralPersistentVacancyParentSelectionDiagnostics {
                parent_state_fingerprint: "state".to_owned(),
                inactive_order_hash: "inactive".to_owned(),
                scheduler_family: "twoHardest".to_owned(),
                hardest_piece_id: "a".to_owned(),
                slots: vec![GeneralPersistentVacancySelectionSlotDiagnostics {
                    piece_id: "a".to_owned(),
                    ..GeneralPersistentVacancySelectionSlotDiagnostics::default()
                }],
                ..GeneralPersistentVacancyParentSelectionDiagnostics::default()
            }],
            ..GeneralPersistentVacancyLayerDiagnostics::default()
        };
        let mut work = RunWork::new(2);
        charge_retained_memory(&[], 0, &mut diagnostics, &pending, &mut work).unwrap();
        assert_eq!(work.diagnostics.retained_peak_bytes, 0);
        assert!(work.diagnostics.selector_diagnostic_peak_bytes > 0);
        assert_eq!(
            work.diagnostics.total_retained_peak_bytes,
            work.diagnostics.selector_diagnostic_peak_bytes
        );
    }

    #[test]
    fn count_comparator_prefers_fewer_inactive_pieces_over_lower_area() {
        let polygons = (0..3).map(|_| square(10.0)).collect::<Vec<_>>();
        let ids = ["a", "b", "c"];
        let pieces = ids
            .iter()
            .enumerate()
            .map(|(index, id)| GeneralFastPiece {
                id,
                polygon: &polygons[index],
                allow_rotation: true,
                allow_mirror: true,
            })
            .collect::<Vec<_>>();
        let difficulty = test_difficulties(&[1, 1, 100]);
        let low_area = state_with_active_mask(vec![false, false, true]);
        let low_count = state_with_active_mask(vec![true, true, false]);
        assert!(compare_states(&low_area, &low_count, &pieces, &difficulty).is_lt());
        assert!(compare_count_states(&low_count, &low_area, &pieces, &difficulty).is_lt());
    }

    #[test]
    fn dual_objective_retention_reserves_both_elites_and_keeps_width() {
        let polygons = (0..10).map(|_| square(10.0)).collect::<Vec<_>>();
        let ids = (0..10)
            .map(|index| format!("p{index:02}"))
            .collect::<Vec<_>>();
        let pieces = ids
            .iter()
            .enumerate()
            .map(|(index, id)| GeneralFastPiece {
                id,
                polygon: &polygons[index],
                allow_rotation: true,
                allow_mirror: true,
            })
            .collect::<Vec<_>>();
        let difficulty = test_difficulties(&[1, 2, 4, 8, 16, 32, 64, 128, 256, 1_000]);
        let area_elite = state_with_active_mask(vec![
            false, false, true, true, true, true, true, true, true, true,
        ]);
        let count_elite = state_with_active_mask(vec![
            true, true, true, true, true, true, true, true, true, false,
        ]);
        let mut states = vec![area_elite.clone(), count_elite.clone()];
        for first in 1..9 {
            let mut active = vec![true; 10];
            active[first] = false;
            active[(first + 1) % 9] = false;
            states.push(state_with_active_mask(active));
        }
        states.sort_by(|first, second| compare_states(first, second, &pieces, &difficulty));
        let (retained, _) = retain_population(states, &pieces, &difficulty, 6);
        assert_eq!(retained.len(), BEAM_WIDTH);
        let identities = retained.iter().map(state_identity).collect::<BTreeSet<_>>();
        assert!(identities.contains(&state_identity(&area_elite)));
        assert!(identities.contains(&state_identity(&count_elite)));
        assert_eq!(identities.len(), retained.len());
    }

    #[test]
    fn carryover_pool_changes_mode_five_but_not_mode_six() {
        let polygons = (0..3).map(|_| square(10.0)).collect::<Vec<_>>();
        let ids = ["a", "b", "c"];
        let pieces = ids
            .iter()
            .enumerate()
            .map(|(index, id)| GeneralFastPiece {
                id,
                polygon: &polygons[index],
                allow_rotation: true,
                allow_mirror: true,
            })
            .collect::<Vec<_>>();
        let difficulty = test_difficulties(&[1, 10, 100]);
        let ordinary = state_with_active_mask(vec![true, false, false]);
        let carryover = state_with_active_mask(vec![false, true, true]);
        let (mode_six, _) = retention_pool(
            vec![ordinary.clone()],
            vec![carryover.clone()],
            &pieces,
            &difficulty,
            6,
        );
        let (mode_five, _) = retention_pool(
            vec![ordinary],
            vec![carryover.clone()],
            &pieces,
            &difficulty,
            5,
        );
        assert_eq!(mode_six.len(), 1);
        assert_eq!(mode_five.len(), 2);
        assert!(mode_five
            .iter()
            .any(|state| state_identity(state) == state_identity(&carryover)));
    }

    #[test]
    fn population_and_child_order_hashes_are_deterministic_and_domain_separated() {
        let polygons = vec![square(10.0), square(10.0)];
        let pieces = [
            GeneralFastPiece {
                id: "a",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "b",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let first = state_with_active_mask(vec![true, false]);
        let second = state_with_active_mask(vec![true, true]);
        let population = vec![first.clone(), second.clone()];
        assert_eq!(
            population_hash(&population, &pieces),
            population_hash(&population, &pieces)
        );
        assert_eq!(
            child_order_hash(&population, &pieces),
            child_order_hash(&population, &pieces)
        );
        assert_ne!(
            population_hash(&population, &pieces),
            child_order_hash(&population, &pieces)
        );
        assert_ne!(
            child_order_hash(&population, &pieces),
            child_order_hash(&[second, first], &pieces)
        );
    }

    #[test]
    fn live_pool_memory_counts_a_duplicate_carryover_before_deduplication() {
        let (_, state) = state_with_two_squares(10.0, 0.0);
        let entering = vec![state.clone()];
        let pending = GeneralPersistentVacancyLayerDiagnostics::default();
        let mut without_diagnostics = GeneralPersistentVacancyDiagnostics::default();
        let mut without_work = RunWork::new(2);
        preflight_live_memory(
            &entering,
            0,
            0,
            0,
            0,
            0,
            &mut without_diagnostics,
            &pending,
            &mut without_work,
        )
        .unwrap();
        let mut with_diagnostics = GeneralPersistentVacancyDiagnostics::default();
        let mut with_work = RunWork::new(2);
        preflight_live_memory(
            &entering,
            0,
            state_vec_bytes(&vec![state]),
            0,
            0,
            0,
            &mut with_diagnostics,
            &pending,
            &mut with_work,
        )
        .unwrap();
        assert!(
            with_work.diagnostics.total_retained_peak_bytes
                > without_work.diagnostics.total_retained_peak_bytes
        );
    }

    #[test]
    fn preserved_state_receives_a_distinct_later_layer_stream() {
        let polygons = (0..4).map(|_| square(10.0)).collect::<Vec<_>>();
        let ids = ["a", "b", "c", "d"];
        let pieces = ids
            .iter()
            .enumerate()
            .map(|(index, id)| GeneralFastPiece {
                id,
                polygon: &polygons[index],
                allow_rotation: true,
                allow_mirror: true,
            })
            .collect::<Vec<_>>();
        let state = state_with_active_mask(vec![false; 4]);
        let difficulty = test_difficulties(&[1, 100, 10, 20]);
        let first = selected_inactive_pieces(&state, &pieces, &difficulty, 0, 5);
        let second = selected_inactive_pieces(&state, &pieces, &difficulty, 1, 5);
        assert_ne!(first.indices, second.indices);
        let seed = parent_seed_key(&state, &pieces);
        assert_ne!(
            derive_seed(PERSISTENT_VACANCY_SEED_DOMAIN ^ seed, 0, 0),
            derive_seed(PERSISTENT_VACANCY_SEED_DOMAIN ^ seed, 1, 0)
        );
    }

    #[test]
    fn shared_population_has_identical_layer_local_work_evidence() {
        let polygons = vec![square(10.0), square(10.0)];
        let pieces = [
            GeneralFastPiece {
                id: "a",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "b",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let population = vec![state_with_active_mask(vec![true, false])];
        let before = GeneralPersistentVacancyWorkDiagnostics {
            selected_piece_slots: 10,
            hazard_queries: 20,
            ..GeneralPersistentVacancyWorkDiagnostics::default()
        };
        let after = GeneralPersistentVacancyWorkDiagnostics {
            selected_piece_slots: 12,
            hazard_queries: 27,
            retained_peak_bytes: 999,
            ..GeneralPersistentVacancyWorkDiagnostics::default()
        };
        let first_hash = population_hash(&population, &pieces);
        let second_hash = population_hash(&population.clone(), &pieces);
        let first_delta = work_delta(
            generation_work_snapshot(after),
            generation_work_snapshot(before),
        );
        let second_delta = work_delta(
            generation_work_snapshot(after),
            generation_work_snapshot(before),
        );
        assert_eq!(first_hash, second_hash);
        assert_eq!(first_delta, second_delta);
        assert_eq!(first_delta.selected_piece_slots, 2);
        assert_eq!(first_delta.hazard_queries, 7);
        assert_eq!(first_delta.retained_peak_bytes, 0);
    }

    #[test]
    fn best_ever_elites_are_monotonic_under_their_own_comparators() {
        let polygons = (0..3).map(|_| square(10.0)).collect::<Vec<_>>();
        let ids = ["a", "b", "c"];
        let pieces = ids
            .iter()
            .enumerate()
            .map(|(index, id)| GeneralFastPiece {
                id,
                polygon: &polygons[index],
                allow_rotation: true,
                allow_mirror: true,
            })
            .collect::<Vec<_>>();
        let difficulty = test_difficulties(&[1, 1, 100]);
        let lower_area = elite_snapshot(
            &state_with_active_mask(vec![false, false, true]),
            &pieces,
            &difficulty,
        );
        let lower_count = elite_snapshot(
            &state_with_active_mask(vec![true, true, false]),
            &pieces,
            &difficulty,
        );
        let mut best_area = None;
        update_best_area(&mut best_area, &lower_count);
        update_best_area(&mut best_area, &lower_area);
        update_best_area(&mut best_area, &lower_count);
        assert_eq!(best_area.unwrap().fingerprint, lower_area.fingerprint);
        let mut best_count = None;
        update_best_count(&mut best_count, &lower_area);
        update_best_count(&mut best_count, &lower_count);
        update_best_count(&mut best_count, &lower_area);
        assert_eq!(best_count.unwrap().fingerprint, lower_count.fingerprint);
    }

    #[test]
    fn raw_live_pool_cap_failure_is_atomic() {
        let mut diagnostics = GeneralPersistentVacancyDiagnostics::default();
        let mut work = RunWork::new(2);
        let result = preflight_raw_live_memory(
            &Vec::new(),
            MAX_RETAINED_BYTES,
            0,
            0,
            0,
            0,
            &[],
            &[],
            &mut diagnostics,
            &mut work,
        );
        assert_eq!(
            result.unwrap_err(),
            "cap: pre-deduplication live-pool memory budget exhausted"
        );
        assert!(diagnostics.layers.is_empty());
    }

    #[test]
    fn dual_objective_modes_reject_nonterminal_width_changes() {
        assert!(enforce_population_width(6, false, BEAM_WIDTH - 1, 4).is_err());
        assert!(enforce_population_width(5, false, BEAM_WIDTH, 4).is_ok());
        assert!(enforce_population_width(6, true, 1, 4).is_ok());
        assert!(enforce_population_width(3, false, BEAM_WIDTH - 1, 4).is_ok());
    }

    #[test]
    fn mirrored_source_budget_funds_both_traversals() {
        for piece_count in [1usize, 2, 17, 20, 61, 400] {
            let quotas = VacancyQuotas::for_piece_count(piece_count);
            assert_eq!(
                quotas.max_source_feature_visits,
                quotas.max_selected_piece_slots * 2 * MAX_SOURCE_FEATURES,
                "piece count {piece_count}"
            );
        }
    }

    #[test]
    fn best_ever_parent_expansion_fits_the_construction_budget() {
        // Mode 25 spends at most one extra expansion per (restart, rank), so
        // its worst case is CONSTRUCTION_RESTARTS * (CONSTRUCTION_BEAM_WIDTH +
        // CONSTRUCTION_BEST_EVER_PARENTS) expansions per piece - exactly what
        // the generalized construction term funds, on any instance.
        for piece_count in [1usize, 2, 3, 17, 20, 61, 137, 400] {
            let quotas = VacancyQuotas::for_piece_count(piece_count);
            let beam_slots = CONSTRUCTION_RESTARTS * CONSTRUCTION_BEAM_WIDTH * piece_count;
            let sidecar_slots =
                CONSTRUCTION_RESTARTS * CONSTRUCTION_BEST_EVER_PARENTS * piece_count;
            assert_eq!(
                quotas.construction_selected_piece_slots,
                beam_slots + sidecar_slots,
                "piece count {piece_count}"
            );
            assert!(
                beam_slots + sidecar_slots <= quotas.max_selected_piece_slots,
                "piece count {piece_count}"
            );
            // Every expansion may burn the full construction row cap and one
            // hint-prior collision build per prior, and every child it keeps
            // costs one trapped-void flood fill.
            assert!(
                (beam_slots + sidecar_slots).saturating_mul(CONSTRUCTION_ROWS_PER_PIECE)
                    <= quotas.max_exact_finalist_rows,
                "piece count {piece_count}"
            );
            assert!(
                (beam_slots + sidecar_slots).saturating_mul(CONSTRUCTION_HINT_PRIORS)
                    <= quotas.max_experimental_collision_builds,
                "piece count {piece_count}"
            );
            assert!(
                (beam_slots + sidecar_slots)
                    .saturating_mul(CONSTRUCTION_FINALISTS_PER_SLOT)
                    .saturating_add(CONSTRUCTION_RESTARTS)
                    <= quotas.construction_void_scan_cap,
                "piece count {piece_count}"
            );
        }
    }

    #[test]
    fn best_ever_parent_ordinals_never_collide_with_beam_ordinals() {
        // The sidecar expansion must not draw a beam slot's seeded
        // orientation and position streams, and mode 20's ordinals must stay
        // exactly where they are.
        for piece_count in [1usize, 2, 3, 17, 61] {
            let mut beam_ordinals = BTreeSet::new();
            let mut sidecar_ordinals = BTreeSet::new();
            for restart in 0..CONSTRUCTION_RESTARTS {
                for rank in 0..piece_count {
                    for slot in 0..CONSTRUCTION_BEAM_WIDTH {
                        beam_ordinals.insert(
                            (restart * piece_count + rank) * CONSTRUCTION_BEAM_WIDTH + slot,
                        );
                    }
                    assert!(
                        sidecar_ordinals.insert(best_ever_parent_ordinal(
                            piece_count,
                            restart,
                            rank
                        )),
                        "piece count {piece_count}"
                    );
                }
            }
            assert!(
                beam_ordinals.is_disjoint(&sidecar_ordinals),
                "piece count {piece_count}"
            );
            assert_eq!(
                sidecar_ordinals.len(),
                CONSTRUCTION_RESTARTS * piece_count,
                "piece count {piece_count}"
            );
        }
    }

    #[test]
    fn construction_elite_key_leads_with_the_exact_frontier() {
        // The retention key bands the frontier and then prefers the fewer
        // trapped voids; the elite comparator drops the banding, so the
        // strictly shallower partial leads even when it carries more voids.
        // That disagreement is exactly when the sidecar is off-beam.
        let shallow_state = state_with_active_mask(vec![true, false]);
        let deep_state = state_with_active_mask(vec![false, true]);
        let shallow: ConstructionChildKey = (0, 9, 100, 100, state_identity(&shallow_state));
        let deep: ConstructionChildKey = (0, 2, 400, 400, state_identity(&deep_state));
        assert!(
            shallow.0 == deep.0 && shallow.2 < deep.2,
            "the two children must share a frontier band"
        );
        assert!(deep < shallow, "retention prefers the fewer-void child");
        assert!(
            construction_elite_key(&shallow) < construction_elite_key(&deep),
            "the elite comparator prefers the strictly shallower child"
        );
        // Inside one band the elite comparator still falls back to the void
        // count only after the exact frontier ties.
        let tied: ConstructionChildKey = (0, 1, 100, 100, state_identity(&deep_state));
        assert!(construction_elite_key(&tied) < construction_elite_key(&shallow));
    }

    #[test]
    fn bounded_reinsertion_fits_the_construction_budget() {
        // Modes 24 and 28 share `replace_ejected_under_bound`, which charges
        // one construction slot expansion per re-placed piece and one
        // collision build per kept piece. A run ejects at most every piece, so
        // its worst case is `piece_count` slot expansions and `piece_count`
        // seeding builds - both strictly funded by the existing per-piece
        // construction term, which is why neither mode needs a new aggregate
        // quota term and the frozen ceilings are untouched. Mode 28 ejects a
        // vertex cover of the violation graph, so it can never eject more than
        // `piece_count` pieces however the admission limit is sized, and each
        // of its calls gets its own `RunWork` ledger - so a mode-26 rung
        // running the repair per arm never accumulates against a shared budget
        // either.
        for piece_count in [1usize, 2, 3, 17, 20, 61, 137, 400] {
            let quotas = VacancyQuotas::for_piece_count(piece_count);
            let worst_case_slots = piece_count;
            assert!(
                replacement_ejection_limit(piece_count).min(piece_count) <= worst_case_slots,
                "piece count {piece_count}"
            );
            assert!(
                worst_case_slots <= quotas.construction_selected_piece_slots,
                "piece count {piece_count}"
            );
            assert!(
                worst_case_slots <= quotas.max_selected_piece_slots,
                "piece count {piece_count}"
            );
            // Each of those slots may burn the full construction row cap plus
            // the anchor-local stream's own budget, and each row is one
            // collision build plus one pair visit per peer.
            // Modes 32 and 33 add the orientation stream's own row budget on
            // top of those two, plus one charged collision build per
            // orientation variant per ejected piece - taken once per pass by
            // `arm_orientation_perturbation`, not once per insertion order.
            // The stream's rows carry their own quota term, which is why the
            // ceiling still covers the worst case with them included.
            let rows_per_slot =
                CONSTRUCTION_ROWS_PER_PIECE + ANCHOR_LOCAL_ROWS + ORIENTATION_PERTURBATION_ROWS;
            let orientation_builds =
                worst_case_slots.saturating_mul(ORIENTATION_PERTURBATION_VARIANTS);
            assert!(
                worst_case_slots.saturating_mul(rows_per_slot) <= quotas.max_exact_finalist_rows,
                "piece count {piece_count}"
            );
            assert!(
                piece_count
                    .saturating_add(worst_case_slots.saturating_mul(rows_per_slot))
                    .saturating_add(orientation_builds)
                    <= quotas.max_experimental_collision_builds,
                "piece count {piece_count}"
            );
            // The orientation budget is derived from the stream it has to
            // cover - the anchor-local neighbourhood once per variant - rather
            // than being a tuned number.
            assert_eq!(
                ORIENTATION_PERTURBATION_ROWS,
                ORIENTATION_PERTURBATION_VARIANTS * ANCHOR_LOCAL_ROWS,
                "piece count {piece_count}"
            );

            // Mode 29 drives the *same* primitive once per attempted insertion
            // order, against one shared ledger. Its slot budget is exactly the
            // single-set pass's old worst case - at most
            // `JOINT_REPLACEMENT_ORDER_CAP` plain orders plus
            // `JOINT_REPLACEMENT_SWAP_ROUNDS * JOINT_REPLACEMENT_SWAP_ATTEMPT_CAP`
            // pose-swap attempts, each over an ejection set as large as the
            // whole layout - and the per-component loop and the finalist beam
            // spend *that* allowance differently rather than asking for a new
            // one. `JointReplacementBudget` enforces it at runtime, so an
            // instance too small to fund the full plan stops on `capExhausted`
            // instead of overrunning, and the joint tier still needs no new
            // aggregate quota term.
            let joint_attempts = JointReplacementBudget::attempt_cap();
            let joint_slots = JointReplacementBudget::slot_cap(piece_count);
            assert_eq!(
                joint_slots,
                (JOINT_REPLACEMENT_ORDER_CAP
                    + JOINT_REPLACEMENT_SWAP_ROUNDS * JOINT_REPLACEMENT_SWAP_ATTEMPT_CAP)
                    .saturating_mul(worst_case_slots),
                "piece count {piece_count}"
            );
            // The beam is exactly `CONSTRUCTION_FINALISTS_PER_SLOT` ranks per
            // piece over a component of at most
            // `JOINT_REPLACEMENT_BEAM_MAX_PIECES`, so its combination ceiling
            // is that power and nothing tunable drifts away from it.
            assert_eq!(
                JOINT_REPLACEMENT_BEAM_COMBINATIONS,
                CONSTRUCTION_FINALISTS_PER_SLOT.pow(JOINT_REPLACEMENT_BEAM_MAX_PIECES as u32),
                "piece count {piece_count}"
            );
            assert_eq!(
                finalist_rank_combinations(JOINT_REPLACEMENT_BEAM_MAX_PIECES).len(),
                JOINT_REPLACEMENT_BEAM_COMBINATIONS,
                "piece count {piece_count}"
            );
            assert!(
                finalist_rank_combinations(JOINT_REPLACEMENT_BEAM_MAX_PIECES + 1).is_empty(),
                "piece count {piece_count}"
            );
            assert!(
                joint_slots <= quotas.construction_selected_piece_slots,
                "piece count {piece_count}"
            );
            assert!(
                joint_slots <= quotas.max_selected_piece_slots,
                "piece count {piece_count}"
            );
            // The peer poses ride the anchor-local stream's existing per-piece
            // budget rather than adding a term: the cloud tops out at 179
            // poses, and three peers keep it under `ANCHOR_LOCAL_ROWS`.
            assert!(
                179 + JOINT_REPLACEMENT_PEER_POSES <= ANCHOR_LOCAL_ROWS,
                "piece count {piece_count}"
            );
            // The anchor-local stream can never consume a bucket ordinal the
            // orientation stream would then reuse, which is what keeps a
            // rotated candidate from being de-duplicated onto an unrotated one.
            assert!(
                179 + JOINT_REPLACEMENT_PEER_POSES + CONSTRUCTION_HINT_PRIORS
                    <= ANCHOR_LOCAL_BUCKET_SPAN,
                "piece count {piece_count}"
            );
            assert!(
                joint_slots.saturating_mul(rows_per_slot) <= quotas.max_exact_finalist_rows,
                "piece count {piece_count}"
            );
            // Each attempted order also rebuilds one collision per kept piece.
            // The orientation variants are built once per component pass, so
            // mode 33's build term is the component ceiling times the ejection
            // worst case times the variant ceiling.
            let joint_orientation_builds = JOINT_REPLACEMENT_COMPONENT_PASSES
                .saturating_mul(worst_case_slots)
                .saturating_mul(ORIENTATION_PERTURBATION_VARIANTS);
            assert!(
                joint_attempts
                    .saturating_mul(piece_count)
                    .saturating_add(joint_slots.saturating_mul(rows_per_slot))
                    .saturating_add(joint_orientation_builds)
                    <= quotas.max_experimental_collision_builds,
                "piece count {piece_count}"
            );

            // Modes 30 and 31 and the mode-26 fourth repair tier share
            // `global_legalize`, which places nothing: it builds one collision
            // envelope per piece per margin escalation and then only
            // *measures*, charging at most a fixed number of exact pair probes
            // per pair per re-linearization round against its own
            // `GlobalLegalizationBudget`. Both worst cases are strictly funded
            // by the experimental terms already reviewed, so the global tier
            // needs no new aggregate quota term either, and an instance whose
            // geometry somehow outran the plan stops on `capExhausted` rather
            // than overrunning a ceiling.
            assert!(
                global_legalization_worst_case_pair_visits(piece_count)
                    <= quotas.max_experimental_pair_visits,
                "piece count {piece_count}"
            );
            assert!(
                global_legalization_worst_case_collision_builds(piece_count)
                    <= quotas.max_experimental_collision_builds,
                "piece count {piece_count}"
            );
        }
    }

    #[test]
    fn aggregate_quota_formulas_match_the_reviewed_contract() {
        // Instance-independent rates. The ordinary 8-parent, 40-layer
        // schedule funds 640 selected-piece slots; the archive revival lane of
        // modes 7/8 adds at most 13 expansions of 2 slots each, so every
        // downstream ceiling carries the ordinary term plus the revival-lane
        // term. None of these terms scales with the piece count.
        assert_eq!(MAX_ARCHIVE_REVIVALS, 13);
        assert_eq!(ORDINARY_SELECTED_PIECE_SLOTS, 640);
        assert_eq!(ARCHIVE_SELECTED_PIECE_SLOTS, 26);
        assert_eq!(POPULATION_SELECTED_PIECE_SLOTS, 640 + 26);
        assert_eq!(SETTLE_SWEEPS, 3);
        assert_eq!(SETTLE_PROBES_PER_ATTEMPT, 64);
        assert_eq!(RECONSTRUCTION_PASSES_PER_PIECE, 2);
        assert_eq!(RECONSTRUCTION_ROWS_PER_PIECE, 192);
        assert_eq!(LNS_SETTLE_SWEEPS, 73);
        assert_eq!(SEPARATION_RELOCATIONS_PER_ROUND, 12);
        assert_eq!(LNS_SCHEDULE_TOTAL, 536);
        assert_eq!(LNS_REINSERT_SLOTS, 536 + 24 * 12 + 2 * 3 * 536);
        assert_eq!(LNS_REINSERT_SLOTS, 4_040);
        assert_eq!(LNS_ROUNDS, 24);
        assert_eq!(CONSTRUCTION_RESTARTS, 8);
        assert_eq!(CONSTRUCTION_BEAM_WIDTH, 6);
        // Mode 25's off-beam best-ever parent is one extra expansion per
        // (restart, rank): the construction term funds 7, not 6, expansions
        // per restart and rank.
        assert_eq!(CONSTRUCTION_BEST_EVER_PARENTS, 1);
        assert_eq!(CONSTRUCTION_ROWS_PER_PIECE, 320);
        assert_eq!(CONSTRUCTION_HINT_PRIORS, 2);
        assert_eq!(CONSTRUCTION_FINALISTS_PER_SLOT, 4);
        assert_eq!(COMPACTION_ROUNDS, 3);
        assert_eq!(GROUP_DROP_PROBES_PER_CUT, 64);
        assert_eq!(SEPARATION_MOVES_PER_ROUND, 200);
        assert_eq!(SEPARATION_PROBES_PER_MOVE, 96);
        assert_eq!(SEPARATION_COLLISION_BUILDS, 24 * (4_040 / 2 + 200 * 96));
        assert_eq!(PRELUDE_COLLISION_BUILD_PASSES, 3);
        assert_eq!(VALIDATOR_PASSES_PER_AUDIT, 2);
        assert_eq!(MAX_AUDITS, 41 + 64);
        assert!(CONSTRUCTION_RESTARTS <= MAX_COMPLETE_AUDITS);
        assert!(CONSTRUCTION_SHELF_ROWS < CONSTRUCTION_ROWS_PER_PIECE);
        assert!(CONSTRUCTION_BEAM_CHILDREN_PER_PARENT <= CONSTRUCTION_BEAM_WIDTH);

        // Every aggregate ceiling is a per-piece (or per-pair) rate times the
        // piece count of the instance under test. The reference arithmetic
        // below is written out independently of
        // `VacancyQuotas::for_piece_count` so this test verifies the formulas
        // rather than restating the implementation.
        for pieces in [1usize, 2, 3, 17, 20, 61, 137, 400] {
            let quotas = VacancyQuotas::for_piece_count(pieces);
            let population = 640 + 26;
            let settle = 3 * pieces;
            let reconstruction = 2 * pieces;
            let lns_settle = 73 * pieces;
            let construction = 8 * (6 + 1) * pieces;
            let reinsert = 4_040;
            let slots = population + settle + reconstruction + lns_settle + reinsert + construction;
            let streams = slots * 12;
            let positions = streams * 32;
            // The orientation-perturbation lane of modes 32 and 33: the joint
            // pass's own slot worst case (24 insertion orders plus 24
            // pose-swap attempts, each over an ejection set as large as the
            // whole layout) times the stream's per-slot row budget, which is
            // itself the anchor-local budget once per orientation variant.
            let orientation_slots = (24 + 24) * pieces;
            let orientation_rows = orientation_slots * (37 * 192);
            let orientation_builds = 8 * pieces * 37;
            assert_eq!(ORIENTATION_PERTURBATION_VARIANTS, 37);
            assert_eq!(ORIENTATION_PERTURBATION_ROWS, 37 * 192);
            assert_eq!(JOINT_REPLACEMENT_COMPONENT_PASSES, 8);
            let rows = population * 8
                + settle * 64
                + reconstruction * 192
                + lns_settle * 64
                + reinsert * 192
                + construction * 320
                + orientation_rows;
            // Distinct pairs of a complete state, and the peers one candidate
            // row is exact-checked against.
            let complete_pairs = pieces * (pieces - 1) / 2;
            let peers = pieces - 1;
            let group_drop_pairs = 3 * pieces * 64 * pieces;
            let separation_pairs = 24 * 200 * 96 * pieces;
            let experimental_builds = 3 * pieces
                + streams
                + rows
                + reconstruction
                + reinsert
                + 2 * construction
                + 24 * (4_040 / 2 + 200 * 96)
                + orientation_builds;
            let experimental_pairs =
                complete_pairs + rows * peers + group_drop_pairs + separation_pairs;
            let validator_builds_per_audit = 2 * pieces;
            let validator_pairs_per_audit = 2 * complete_pairs;

            assert_eq!(quotas.piece_count, pieces, "piece count {pieces}");
            assert_eq!(quotas.group_drop_cuts, pieces, "piece count {pieces}");
            assert_eq!(
                quotas.settle_selected_piece_slots, settle,
                "piece count {pieces}"
            );
            assert_eq!(
                quotas.reconstruction_selected_piece_slots, reconstruction,
                "piece count {pieces}"
            );
            assert_eq!(
                quotas.lns_settle_selected_piece_slots, lns_settle,
                "piece count {pieces}"
            );
            assert_eq!(
                quotas.construction_selected_piece_slots, construction,
                "piece count {pieces}"
            );
            assert_eq!(
                quotas.construction_void_scan_cap,
                construction * 4 + 8,
                "piece count {pieces}"
            );
            assert_eq!(
                quotas.bridge_void_scan_cap,
                24 * (pieces + 1),
                "piece count {pieces}"
            );
            assert_eq!(
                quotas.group_drop_pair_visits, group_drop_pairs,
                "piece count {pieces}"
            );
            assert_eq!(
                quotas.separation_pair_visits, separation_pairs,
                "piece count {pieces}"
            );
            assert_eq!(
                quotas.max_selected_piece_slots, slots,
                "piece count {pieces}"
            );
            assert_eq!(
                quotas.max_orientation_streams, streams,
                "piece count {pieces}"
            );
            assert_eq!(
                quotas.max_source_feature_visits,
                slots * 2 * 512,
                "piece count {pieces}"
            );
            assert_eq!(
                quotas.max_position_source_attempts,
                streams * 529,
                "piece count {pieces}"
            );
            assert_eq!(
                quotas.max_returned_positions, positions,
                "piece count {pieces}"
            );
            assert_eq!(quotas.max_hazard_queries, positions, "piece count {pieces}");
            assert_eq!(
                quotas.max_proxy_pressure_visits,
                positions * pieces,
                "piece count {pieces}"
            );
            assert_eq!(quotas.max_exact_finalist_rows, rows, "piece count {pieces}");
            assert_eq!(
                quotas.max_experimental_collision_builds, experimental_builds,
                "piece count {pieces}"
            );
            assert_eq!(
                quotas.max_experimental_pair_visits, experimental_pairs,
                "piece count {pieces}"
            );
            assert_eq!(
                quotas.validator_collision_builds_per_audit, validator_builds_per_audit,
                "piece count {pieces}"
            );
            assert_eq!(
                quotas.validator_pair_visits_per_audit, validator_pairs_per_audit,
                "piece count {pieces}"
            );
            // The validator ceilings fund exactly MAX_AUDITS publications on
            // any instance.
            assert_eq!(
                quotas.max_validator_collision_builds,
                validator_builds_per_audit * 105,
                "piece count {pieces}"
            );
            assert_eq!(
                quotas.max_validator_pair_visits,
                validator_pairs_per_audit * 105,
                "piece count {pieces}"
            );
            assert_eq!(
                quotas.max_transformed_collision_vertices,
                (experimental_builds + validator_builds_per_audit * 105) * 512,
                "piece count {pieces}"
            );
            assert_eq!(
                quotas.max_clipper_input_vertices,
                2 * 512 * (experimental_pairs + validator_pairs_per_audit * 105),
                "piece count {pieces}"
            );
        }

        // Historical Mixed-61 ceilings. These are the exact literals the
        // reviewed contract was certified against, before the machinery was
        // generalized to any instance; they are asserted here - and nowhere in
        // engine code - to prove the formulas above reproduce the frozen
        // 61-piece budgets bit for bit.
        //
        // One term has moved since that certification: the construction lane
        // now funds CONSTRUCTION_BEAM_WIDTH + CONSTRUCTION_BEST_EVER_PARENTS
        // expansions per (restart, rank) instead of CONSTRUCTION_BEAM_WIDTH,
        // so mode 25's off-beam best-ever parent is charged to the same ledger
        // as every ordinary beam expansion. At 61 pieces that is 8 * 7 * 61 =
        // 3_416 construction slots in place of 8 * 6 * 61 = 2_928, and every
        // ceiling derived from it grows by the same 488 slots. Raising a
        // ceiling can only turn a `cap:` failure into a completed run, so no
        // run that already completed - mode 20's included - changes behavior.
        let mixed61 = VacancyQuotas::for_piece_count(61);
        assert_eq!(mixed61.settle_selected_piece_slots, 183);
        assert_eq!(mixed61.reconstruction_selected_piece_slots, 122);
        assert_eq!(mixed61.lns_settle_selected_piece_slots, 73 * 61);
        assert_eq!(mixed61.construction_selected_piece_slots, 3_416);
        assert_eq!(
            CONSTRUCTION_HINT_PRIORS * mixed61.construction_selected_piece_slots,
            6_832
        );
        assert_eq!(mixed61.construction_void_scan_cap, 8 * 61 * 7 * 4 + 8);
        assert_eq!(mixed61.group_drop_cuts, 61);
        assert_eq!(mixed61.group_drop_pair_visits, 3 * 61 * 64 * 61);
        assert_eq!(mixed61.bridge_void_scan_cap, 24 * 62);
        assert_eq!(mixed61.separation_pair_visits, 24 * 200 * 96 * 61);
        assert_eq!(
            mixed61.max_selected_piece_slots,
            640 + 26 + 183 + 122 + 73 * 61 + 4_040 + 3_416
        );
        assert_eq!(
            mixed61.max_orientation_streams,
            (640 + 26 + 183 + 122 + 73 * 61 + 4_040 + 3_416) * 12
        );
        assert_eq!(
            mixed61.max_position_source_attempts,
            (640 + 26 + 183 + 122 + 73 * 61 + 4_040 + 3_416) * 12 * 529
        );
        assert_eq!(
            mixed61.max_returned_positions,
            (640 + 26 + 183 + 122 + 73 * 61 + 4_040 + 3_416) * 12 * 32
        );
        assert_eq!(
            mixed61.max_hazard_queries,
            (640 + 26 + 183 + 122 + 73 * 61 + 4_040 + 3_416) * 12 * 32
        );
        assert_eq!(
            mixed61.max_proxy_pressure_visits,
            (640 + 26 + 183 + 122 + 73 * 61 + 4_040 + 3_416) * 12 * 32 * 61
        );
        // The orientation-perturbation lane (modes 32/33): 48 joint attempt
        // slots per piece times the stream's per-slot row budget of one
        // anchor-local budget per orientation variant. 37 variants is the
        // nine-rung ladder in both signs, mirrored, plus the pure mirror flip.
        assert_eq!(
            mixed61.max_exact_finalist_rows,
            (640 + 26) * 8
                + 183 * 64
                + 122 * 192
                + 73 * 61 * 64
                + 4_040 * 192
                + 3_416 * 320
                + 48 * 61 * (37 * 192)
        );
        assert_eq!(
            mixed61.max_experimental_collision_builds,
            3 * 61
                + (640 + 26 + 183 + 122 + 73 * 61 + 4_040 + 3_416) * 12
                + ((640 + 26) * 8
                    + 183 * 64
                    + 122 * 192
                    + 73 * 61 * 64
                    + 4_040 * 192
                    + 3_416 * 320
                    + 48 * 61 * (37 * 192))
                + 122
                + 4_040
                + 2 * 3_416
                + 24 * (4_040 / 2 + 200 * 96)
                + 8 * 61 * 37
        );
        assert_eq!(
            mixed61.max_experimental_pair_visits,
            1_830
                + ((640 + 26) * 8
                    + 183 * 64
                    + 122 * 192
                    + 73 * 61 * 64
                    + 4_040 * 192
                    + 3_416 * 320
                    + 48 * 61 * (37 * 192))
                    * 60
                + 3 * 61 * 64 * 61
                + 24 * 200 * 96 * 61
        );
        assert_eq!(mixed61.validator_collision_builds_per_audit, 122);
        assert_eq!(mixed61.validator_pair_visits_per_audit, 3_660);
        assert_eq!(mixed61.max_validator_collision_builds, 12_810);
        assert_eq!(mixed61.max_validator_pair_visits, 384_300);
        assert_eq!(
            mixed61.max_transformed_collision_vertices,
            (mixed61.max_experimental_collision_builds + 12_810) * 512
        );
        assert_eq!(
            mixed61.max_clipper_input_vertices,
            2 * 512 * (mixed61.max_experimental_pair_visits + 384_300)
        );
    }

    fn archived_entry(state: &VacancyState, fingerprint: &str) -> (EliteSnapshot, VacancyState) {
        (
            EliteSnapshot {
                fingerprint: fingerprint.to_owned(),
                inactive_piece_count: state.active.iter().filter(|active| !**active).count(),
                inactive_area_grid2: 0,
                inactive_difficulty_sequence: Vec::new(),
                ejected_material_area_grid2: 0,
                ejected_piece_count: 0,
                active_frontier_grid: 0,
                identity: state_identity(state),
            },
            state.clone(),
        )
    }

    fn archive_test_pieces_and_difficulty(
        polygons: &[PolygonSet],
    ) -> (Vec<GeneralFastPiece<'_>>, Vec<PieceDifficulty>) {
        let pieces = polygons
            .iter()
            .enumerate()
            .map(|(index, polygon)| GeneralFastPiece {
                id: ["a", "b", "c", "d"][index],
                polygon,
                allow_rotation: true,
                allow_mirror: true,
            })
            .collect::<Vec<_>>();
        let difficulty = polygons
            .iter()
            .map(|_| PieceDifficulty {
                expanded_area_grid2: 100,
                hull_deficit_grid2: 100,
                minimum_side_grid: 100,
                material_area_grid2: 100,
            })
            .collect::<Vec<_>>();
        (pieces, difficulty)
    }

    #[test]
    fn archive_revival_schedule_is_deterministic_and_bounded() {
        let polygons = vec![square(10.0), square(10.0), square(10.0)];
        let (pieces, difficulty) = archive_test_pieces_and_difficulty(&polygons);
        let population = vec![state_with_active_mask(vec![true, true, false])];
        let archived = state_with_active_mask(vec![true, false, true]);
        let mut archive = TopologyArchive::new();

        // An empty archive never plans a revival.
        assert!(matches!(
            archive.plan_revival(10, &population, &pieces, &difficulty, 7),
            RevivalDecision::NotStagnant
        ));

        archive.area = Some(archived_entry(&archived, "area-fp"));
        // Below the stagnation threshold nothing fires.
        assert!(matches!(
            archive.plan_revival(
                ARCHIVE_STAGNATION_LAYERS - 1,
                &population,
                &pieces,
                &difficulty,
                7
            ),
            RevivalDecision::NotStagnant
        ));
        // At the threshold the area elite is revived.
        match archive.plan_revival(
            ARCHIVE_STAGNATION_LAYERS,
            &population,
            &pieces,
            &difficulty,
            7,
        ) {
            RevivalDecision::Revive {
                kind, fingerprint, ..
            } => {
                assert_eq!(kind, "area");
                assert_eq!(fingerprint, "area-fp");
            }
            _ => panic!("expected a revival at the stagnation threshold"),
        }
        // Cooldown suppresses the next firing until it elapses.
        archive.revivals_expanded = 1;
        archive.revival_ordinal = 1;
        archive.last_revival_layer = Some(ARCHIVE_STAGNATION_LAYERS);
        assert!(matches!(
            archive.plan_revival(
                ARCHIVE_STAGNATION_LAYERS + ARCHIVE_REVIVAL_COOLDOWN - 1,
                &population,
                &pieces,
                &difficulty,
                7
            ),
            RevivalDecision::NotStagnant
        ));
        assert!(matches!(
            archive.plan_revival(
                ARCHIVE_STAGNATION_LAYERS + ARCHIVE_REVIVAL_COOLDOWN,
                &population,
                &pieces,
                &difficulty,
                7
            ),
            RevivalDecision::Revive { .. }
        ));
        // The expansion budget rejects further revivals explicitly.
        archive.revivals_expanded = MAX_ARCHIVE_REVIVALS;
        assert!(matches!(
            archive.plan_revival(30, &population, &pieces, &difficulty, 7),
            RevivalDecision::Skipped("revivalBudgetExhausted")
        ));
    }

    #[test]
    fn archive_revival_alternates_between_area_and_count() {
        let polygons = vec![square(10.0), square(10.0), square(10.0)];
        let (pieces, difficulty) = archive_test_pieces_and_difficulty(&polygons);
        let population = vec![state_with_active_mask(vec![true, true, false])];
        let area_state = state_with_active_mask(vec![true, false, true]);
        let count_state = state_with_active_mask(vec![false, true, true]);
        let mut archive = TopologyArchive::new();
        archive.area = Some(archived_entry(&area_state, "area-fp"));
        archive.count = Some(archived_entry(&count_state, "count-fp"));

        match archive.plan_revival(10, &population, &pieces, &difficulty, 7) {
            RevivalDecision::Revive { kind, .. } => assert_eq!(kind, "area"),
            _ => panic!("expected an even-ordinal area revival"),
        }
        archive.revival_ordinal = 1;
        match archive.plan_revival(10, &population, &pieces, &difficulty, 7) {
            RevivalDecision::Revive { kind, .. } => assert_eq!(kind, "count"),
            _ => panic!("expected an odd-ordinal count revival"),
        }
        // A candidate whose identity is already in the population falls
        // through to the other elite.
        archive.revival_ordinal = 0;
        let population = vec![area_state.clone()];
        match archive.plan_revival(10, &population, &pieces, &difficulty, 7) {
            RevivalDecision::Revive { kind, .. } => assert_eq!(kind, "count"),
            _ => panic!("expected fallthrough to the count elite"),
        }
        // Both candidates in the population produce an explicit skip.
        let population = vec![area_state, count_state];
        assert!(matches!(
            archive.plan_revival(10, &population, &pieces, &difficulty, 7),
            RevivalDecision::Skipped("inPopulation")
        ));
    }

    #[test]
    fn mode_eight_revival_requires_strict_improvement_over_the_worst_slot() {
        let polygons = vec![square(10.0), square(10.0), square(10.0)];
        let (pieces, difficulty) = archive_test_pieces_and_difficulty(&polygons);
        // Two inactive pieces make the archived state worse under the
        // area-first comparator than both population states (one inactive).
        let worse_archived = state_with_active_mask(vec![false, false, true]);
        let mut archive = TopologyArchive::new();
        archive.area = Some(archived_entry(&worse_archived, "area-fp"));
        let population = vec![
            state_with_active_mask(vec![true, true, false]),
            state_with_active_mask(vec![true, false, true]),
        ];
        assert!(matches!(
            archive.plan_revival(10, &population, &pieces, &difficulty, 8),
            RevivalDecision::Skipped("notBetterThanWorst")
        ));
        // A single-state population cannot be swapped.
        let single = vec![state_with_active_mask(vec![true, true, false])];
        assert!(matches!(
            archive.plan_revival(10, &single, &pieces, &difficulty, 8),
            RevivalDecision::Skipped("populationTooSmall")
        ));
        // A strictly better archived state is swapped in under mode 8.
        let better_archived = state_with_active_mask(vec![true, true, true]);
        archive.area = Some(archived_entry(&better_archived, "better-fp"));
        assert!(matches!(
            archive.plan_revival(10, &population, &pieces, &difficulty, 8),
            RevivalDecision::Revive { kind: "area", .. }
        ));
    }

    #[test]
    fn modes_seven_and_eight_reuse_the_rotating_scheduler_and_area_retention() {
        assert_eq!(scheduler_family(7), "hardPlusStatelessRotation");
        assert_eq!(scheduler_family(8), "hardPlusStatelessRotation");
        assert_eq!(selector_ids(&["a", "b", "c", "d"], 0, 7), vec!["b", "a"]);
        assert_eq!(selector_ids(&["a", "b", "c", "d"], 1, 7), vec!["b", "c"]);
        assert_eq!(selector_ids(&["a", "b", "c", "d"], 1, 8), vec!["b", "c"]);

        let (_, first) = state_with_two_squares(20.0, 0.0);
        let (_, second) = state_with_two_squares(25.0, 0.0);
        let (_, third) = state_with_two_squares(30.0, 0.0);
        let polygons = vec![square(10.0), square(10.0)];
        let polygons = polygons[..2].to_vec();
        let (pieces, difficulty) = archive_test_pieces_and_difficulty(&polygons);
        let sorted = vec![first, second, third];
        let (mode3, signatures3) = retain_population(sorted.clone(), &pieces, &difficulty, 3);
        let (mode7, signatures7) = retain_population(sorted.clone(), &pieces, &difficulty, 7);
        let (mode8, signatures8) = retain_population(sorted, &pieces, &difficulty, 8);
        assert_eq!(mode3.len(), mode7.len());
        assert_eq!(signatures3, signatures7);
        assert_eq!(signatures3, signatures8);
        for (left, right) in mode3.iter().zip(mode7.iter()) {
            assert!(same_state_identity(left, right));
        }
        for (left, right) in mode3.iter().zip(mode8.iter()) {
            assert!(same_state_identity(left, right));
        }
    }

    #[test]
    fn archive_bytes_charge_grows_with_archived_states() {
        let mut archive = TopologyArchive::new();
        assert_eq!(archive.bytes(), 0);
        let (_, state) = state_with_two_squares(20.0, 0.0);
        archive.area = Some(archived_entry(&state, "area-fp"));
        let with_area = archive.bytes();
        assert!(with_area > 0);
        archive.count = Some(archived_entry(&state, "count-fp"));
        assert!(archive.bytes() > with_area);
        archive.charge_peak();
        assert_eq!(archive.peak_bytes, archive.bytes());
    }

    #[test]
    fn pinned_parent_fixture_reproduces_the_frozen_fingerprint() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/mixed-61/persistent-vacancy-parent-b9335a72.json"
        );
        let bytes = std::fs::read(path).expect("the pinned parent fixture is committed");
        let fixture: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            fixture["requestSha256"],
            "dfd2ceecf02efe3475e3344dfefbfb2a2a5bd8a673008b449f5689507c933ba1"
        );
        assert_eq!(fixture["reportedDepthMm"], 168.625);
        assert_eq!(fixture["independentDepthMm"], 168.361);
        let placements = fixture["placements"]
            .as_array()
            .unwrap()
            .iter()
            .map(|placement| GeneralFastPlacement {
                piece_id: placement["pieceId"].as_str().unwrap().to_owned(),
                rotation_deg: placement["rotationDeg"].as_f64().unwrap(),
                mirrored: placement["mirrored"].as_bool().unwrap(),
                translate_short_axis: placement["translateShortAxis"].as_f64().unwrap(),
                translate_long_axis: placement["translateLongAxis"].as_f64().unwrap(),
            })
            .collect::<Vec<_>>();
        assert_eq!(placements.len(), 61);
        assert_eq!(
            coupled_fast_placement_fingerprint(&placements),
            EXPECTED_PARENT_FINGERPRINT
        );
        assert_eq!(
            fixture["expectedPlacementFingerprint"],
            EXPECTED_PARENT_FINGERPRINT
        );
    }

    #[test]
    fn descent_modes_enforce_target_and_pinned_parent_requirements() {
        let polygons = vec![square(10.0)];
        let pieces = vec![GeneralFastPiece {
            id: "a",
            polygon: &polygons[0],
            allow_rotation: true,
            allow_mirror: true,
        }];
        let fast = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let mut relaxed = GeneralRelaxedSettings::mixed_61_probe(0, 1);
        let parent = GeneralCoupledSeparatorArmDiagnostics::default();

        // Mode 9 without an explicit target is rejected before any work.
        let result =
            run_persistent_vacancy_population(&pieces, fast, relaxed, &parent, Some("f".into()), 9);
        assert!(result
            .failure_reason
            .unwrap()
            .contains("require an explicit target depth"));

        // Mode 9 with a target but no pinned parent fixture is rejected.
        relaxed.persistent_vacancy_target_depth_mm = Some(90.0);
        let result = run_persistent_vacancy_population(&pieces, fast, relaxed, &parent, None, 9);
        assert!(result
            .failure_reason
            .unwrap()
            .contains("require a pinned parent fixture"));

        // Mode 20 enforces the same target and fixture requirements as the
        // rest of the descent lane.
        let mut without_target = relaxed;
        without_target.persistent_vacancy_target_depth_mm = None;
        let result = run_persistent_vacancy_population(
            &pieces,
            fast,
            without_target,
            &parent,
            Some("f".into()),
            20,
        );
        assert!(result
            .failure_reason
            .unwrap()
            .contains("require an explicit target depth"));
        let result = run_persistent_vacancy_population(&pieces, fast, relaxed, &parent, None, 20);
        assert!(result
            .failure_reason
            .unwrap()
            .contains("require a pinned parent fixture"));

        // The opt-in admits an in-process parent, and only that: it moves the
        // gate, it does not remove it, and it is off in the default settings.
        assert!(!GeneralRelaxedSettings::mixed_61_probe(0, 1)
            .persistent_vacancy_allow_unpinned_parent);
        let mut unpinned = relaxed;
        unpinned.persistent_vacancy_allow_unpinned_parent = true;
        let result = run_persistent_vacancy_population(&pieces, fast, unpinned, &parent, None, 20);
        assert!(!result
            .failure_reason
            .unwrap_or_default()
            .contains("require a pinned parent fixture"));
        // The report still describes the parent honestly: no fixture was read,
        // so nothing may claim one was.
        assert!(result.parent_source.is_none());

        // Frozen modes reject target overrides outright.
        let result =
            run_persistent_vacancy_population(&pieces, fast, relaxed, &parent, Some("f".into()), 3);
        assert!(result
            .failure_reason
            .unwrap()
            .contains("target depth overrides require modes 9-21"));

        // Non-finite and non-positive targets fail closed.
        relaxed.persistent_vacancy_target_depth_mm = Some(f64::NAN);
        let result = run_persistent_vacancy_population(
            &pieces,
            fast,
            relaxed,
            &parent,
            Some("f".into()),
            11,
        );
        assert!(result
            .failure_reason
            .unwrap()
            .contains("positive finite value"));
    }

    #[test]
    fn construction_order_is_deterministic_and_ranks_area_descending() {
        let polygons = vec![square(10.0), square(20.0), square(15.0), square(5.0)];
        let pieces = polygons
            .iter()
            .enumerate()
            .map(|(index, polygon)| GeneralFastPiece {
                id: ["a", "b", "c", "d"][index],
                polygon,
                allow_rotation: true,
                allow_mirror: true,
            })
            .collect::<Vec<_>>();
        let fast = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let first = construction_order(&pieces, fast, 0, 7).unwrap();
        let second = construction_order(&pieces, fast, 0, 7).unwrap();
        assert_eq!(first, second);
        assert_eq!(first, vec![1, 2, 0, 3]);
        for restart in 0..CONSTRUCTION_RESTARTS {
            let mut order = construction_order(&pieces, fast, restart, 7).unwrap();
            order.sort_unstable();
            assert_eq!(order, vec![0, 1, 2, 3]);
        }
    }

    #[test]
    fn construction_diagnostics_stay_absent_from_legacy_serializations() {
        // "construction" is also a substring of "reconstruction", so this
        // guards both optional lanes staying skipped on legacy-mode output.
        let serialized =
            serde_json::to_string(&GeneralPersistentVacancyDiagnostics::default()).unwrap();
        assert!(!serialized.contains("construction"));
        // The mode-24 block is optional on the same terms.
        assert!(!serialized.contains("boundedReinsertion"));
    }

    #[test]
    fn settle_key_orders_by_frontier_then_translation() {
        let low = SettleKey {
            max_y: 10,
            translate_y: 5,
            translate_x: 5,
        };
        let high = SettleKey {
            max_y: 11,
            translate_y: 0,
            translate_x: 0,
        };
        assert!(settle_key_less(low, high));
        assert!(!settle_key_less(high, low));
        let same_frontier_lower_y = SettleKey {
            max_y: 10,
            translate_y: 4,
            translate_x: 9,
        };
        assert!(settle_key_less(same_frontier_lower_y, low));
    }

    #[test]
    fn settle_baseline_drops_a_floating_square_onto_the_floor() {
        let polygons = vec![square(10.0), square(10.0)];
        let pieces = polygons
            .iter()
            .enumerate()
            .map(|(index, polygon)| GeneralFastPiece {
                id: ["a", "b"][index],
                polygon,
                allow_rotation: true,
                allow_mirror: true,
            })
            .collect::<Vec<_>>();
        let fast = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let baseline = RelaxedState {
            placements: vec![
                RelaxedPlacement {
                    input_index: 0,
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_x: 20.0,
                    translate_y: 0.1,
                },
                RelaxedPlacement {
                    input_index: 1,
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_x: 20.0,
                    translate_y: 40.0,
                },
            ],
            strip_depth_mm: 100.0,
        };
        let mut diagnostics = GeneralPersistentVacancyDiagnostics::default();
        let mut work = RunWork::new(2);
        let settled =
            settle_baseline(&pieces, fast, baseline, &mut diagnostics, &mut work).unwrap();
        let settle = diagnostics.settle.expect("settle diagnostics recorded");
        let ys = settled
            .placements
            .iter()
            .map(|placement| placement.translate_y)
            .collect::<Vec<_>>();
        assert!(settle.accepted_moves >= 1, "settle: {settle:?} ys: {ys:?}");
        assert!(settle.frontier_after_grid < settle.frontier_before_grid);
        // Down-only settling drops the floating square toward the first
        // square; the exact pair gate keeps the result overlap-free and the
        // expanded-collision allowance retains a tiny gap above it.
        assert!(settled.placements[1].translate_y < 40.0);
        assert!(
            settled.placements[1].translate_y >= settled.placements[0].translate_y + 10.0 - 1e-9
        );
    }

    /// A settings block with a real clearance contract, matching how the
    /// benchmark driver configures the engine for the exact-clearance fixture.
    fn replacement_settings(short_axis_mm: f64, long_axis_mm: f64) -> GeneralFastSettings {
        let mut settings = GeneralFastSettings::deterministic_test(short_axis_mm, long_axis_mm);
        settings.total_padding_mm = 5.0;
        settings.sheet_edge_clearance_mm = Some(5.0);
        settings.clearance_safety_margin_mm = 0.0;
        settings.flattening_sag_tolerance_mm = 0.0;
        settings.search_offset_allowance_mm = 0.0005;
        settings
    }

    fn replacement_placement(id: &str, x_mm: f64, y_mm: f64) -> GeneralFastPlacement {
        GeneralFastPlacement {
            piece_id: id.to_owned(),
            rotation_deg: 0.0,
            mirrored: false,
            translate_short_axis: x_mm,
            translate_long_axis: y_mm,
        }
    }

    fn replacement_pieces<'a>(
        ids: &'a [&'a str],
        polygons: &'a [PolygonSet],
    ) -> Vec<GeneralFastPiece<'a>> {
        ids.iter()
            .enumerate()
            .map(|(index, id)| GeneralFastPiece {
                id,
                polygon: &polygons[index],
                allow_rotation: false,
                allow_mirror: false,
            })
            .collect()
    }

    #[test]
    fn replacement_repair_ejects_the_conflict_and_rebuilds_it() {
        // The mechanism end to end on a residue the micro-legalizer refuses:
        // `b` sits 1 mm from `a` under a 5 mm contract, a 4 mm deficit, which
        // is a search move rather than a projection. There is open sheet above
        // the pair, so ejecting one endpoint and re-placing it must succeed.
        let ids = ["a", "b"];
        let polygons = vec![square(20.0), square(20.0)];
        let pieces = replacement_pieces(&ids, &polygons);
        let settings = replacement_settings(200.0, 300.0);
        let placements = vec![
            replacement_placement("a", 20.0, 20.0),
            replacement_placement("b", 41.0, 20.0),
        ];
        assert!(validate_and_measure_placements(&pieces, &placements, settings).is_err());

        let outcome = replacement_repair(&pieces, &placements, settings, 200.0, false);
        let diagnostics = &outcome.diagnostics;
        assert!(diagnostics.attempted, "{diagnostics:?}");
        assert_eq!(diagnostics.violating_pairs, 1, "{diagnostics:?}");
        assert_eq!(diagnostics.ejected_count, 1, "{diagnostics:?}");
        assert_eq!(diagnostics.kept_violating_pairs, 0, "{diagnostics:?}");
        assert_eq!(diagnostics.replaced_count, 1, "{diagnostics:?}");
        assert!(diagnostics.failed_piece_id.is_none(), "{diagnostics:?}");
        assert!(diagnostics.exact_valid, "{diagnostics:?}");

        let repaired = outcome.repaired.expect("a repaired layout");
        // The pass never publishes on its own authority: what it returns has
        // already been through the authoritative validator.
        validate_and_measure_placements(&pieces, &repaired, settings)
            .expect("the repaired layout validates");
        // Every re-placed pose honours the clamp.
        for placement in &repaired {
            assert!(placement.translate_long_axis <= 200.0);
        }
    }

    #[test]
    fn replacement_repair_is_deterministic() {
        let ids = ["a", "b"];
        let polygons = vec![square(20.0), square(20.0)];
        let pieces = replacement_pieces(&ids, &polygons);
        let settings = replacement_settings(200.0, 300.0);
        let placements = vec![
            replacement_placement("a", 20.0, 20.0),
            replacement_placement("b", 41.0, 20.0),
        ];

        let first = replacement_repair(&pieces, &placements, settings, 200.0, false);
        let second = replacement_repair(&pieces, &placements, settings, 200.0, false);
        assert_eq!(first.diagnostics, second.diagnostics);
        assert_eq!(first.repaired, second.repaired);
    }

    #[test]
    fn replacement_repair_ejects_the_heavier_endpoint_of_a_pair() {
        // `b` is in conflict with both `a` and `c`, so it carries twice the
        // incident mass either neighbour does and is the piece whose removal
        // clears the most violation. Ejecting it covers both pairs at once,
        // which is the whole point of scoring the choice by mass.
        let ids = ["a", "b", "c"];
        let polygons = vec![square(20.0), square(20.0), square(20.0)];
        let pieces = replacement_pieces(&ids, &polygons);
        let settings = replacement_settings(200.0, 300.0);
        let placements = vec![
            replacement_placement("a", 20.0, 20.0),
            replacement_placement("b", 41.0, 20.0),
            replacement_placement("c", 62.0, 20.0),
        ];

        let outcome = replacement_repair(&pieces, &placements, settings, 200.0, false);
        let diagnostics = &outcome.diagnostics;
        assert_eq!(diagnostics.violating_pairs, 2, "{diagnostics:?}");
        assert_eq!(diagnostics.ejected_count, 1, "{diagnostics:?}");
        assert_eq!(diagnostics.ejected_piece_ids, vec!["b".to_owned()]);
        assert_eq!(diagnostics.kept_violating_pairs, 0, "{diagnostics:?}");
    }

    #[test]
    fn replacement_repair_refuses_a_layout_with_no_violating_pair() {
        let ids = ["a", "b"];
        let polygons = vec![square(20.0), square(20.0)];
        let pieces = replacement_pieces(&ids, &polygons);
        let settings = replacement_settings(200.0, 300.0);
        let placements = vec![
            replacement_placement("a", 20.0, 20.0),
            replacement_placement("b", 60.0, 20.0),
        ];
        assert!(validate_and_measure_placements(&pieces, &placements, settings).is_ok());

        let outcome = replacement_repair(&pieces, &placements, settings, 200.0, false);
        assert!(outcome.repaired.is_none());
        assert!(!outcome.diagnostics.attempted);
        assert!(
            outcome
                .diagnostics
                .skipped_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("no violating pair")),
            "{:?}",
            outcome.diagnostics
        );
    }

    #[test]
    fn replacement_repair_fails_cleanly_when_a_piece_cannot_be_re_placed() {
        // The same conflict, but the sheet is clamped so tightly that the
        // ejected piece has nowhere legal to land. The pass must report the
        // failure per piece rather than publish anything.
        let ids = ["a", "b"];
        let polygons = vec![square(20.0), square(20.0)];
        let pieces = replacement_pieces(&ids, &polygons);
        let settings = replacement_settings(50.0, 300.0);
        let placements = vec![
            replacement_placement("a", 5.0, 5.0),
            replacement_placement("b", 26.0, 5.0),
        ];

        let outcome = replacement_repair(&pieces, &placements, settings, 30.0, false);
        assert!(outcome.repaired.is_none(), "{:?}", outcome.diagnostics);
        assert!(!outcome.diagnostics.exact_valid);
        assert_eq!(outcome.diagnostics.replaced_count, 0);
        assert!(
            outcome.diagnostics.failed_piece_id.is_some(),
            "{:?}",
            outcome.diagnostics
        );
    }

    #[test]
    fn replacement_repair_refuses_an_oversized_violation_component() {
        // A residue that spans more of the layout than a local repair may
        // touch is a search problem, and is refused rather than attempted.
        let count = 8;
        let ids = (0..count)
            .map(|index| format!("p{index}"))
            .collect::<Vec<_>>();
        let id_refs = ids.iter().map(String::as_str).collect::<Vec<_>>();
        let polygons = (0..count).map(|_| square(20.0)).collect::<Vec<_>>();
        let pieces = replacement_pieces(&id_refs, &polygons);
        let settings = replacement_settings(400.0, 300.0);
        // A chain of pieces each 1 mm from the next: one component spanning
        // every piece, against a limit of `max(4, 8 / 8) = 4`.
        let placements = (0..count)
            .map(|index| replacement_placement(&ids[index], 20.0 + 21.0 * index as f64, 20.0))
            .collect::<Vec<_>>();

        let outcome = replacement_repair(&pieces, &placements, settings, 200.0, false);
        let diagnostics = &outcome.diagnostics;
        assert!(outcome.repaired.is_none());
        assert!(!diagnostics.attempted, "{diagnostics:?}");
        assert_eq!(diagnostics.component_limit, 4, "{diagnostics:?}");
        assert_eq!(
            diagnostics.largest_component_pieces, count,
            "{diagnostics:?}"
        );
        assert!(
            diagnostics
                .skipped_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("local-repair limit")),
            "{diagnostics:?}"
        );
    }

    /// A rectangle `width_mm` across the short axis by `height_mm` along the
    /// long one, with its lower-left corner at the origin - the shape the joint
    /// re-placement tests need, because a *square* pair can always be separated
    /// along whichever axis has room and so never forces a joint move.
    fn rect(width_mm: f64, height_mm: f64) -> PolygonSet {
        PolygonSet::from_outer(vec![
            IrregularPoint::new(0.0, 0.0),
            IrregularPoint::new(width_mm, 0.0),
            IrregularPoint::new(width_mm, height_mm),
            IrregularPoint::new(0.0, height_mm),
        ])
        .unwrap()
    }

    /// The interlocked two-piece residue both single-piece tiers refuse.
    ///
    /// `a` is 20 across by 40 along; `b` is 40 across by 20 along; the sheet is
    /// 55 across, so `b` spans essentially the whole width and the two can only
    /// ever be stacked, never placed side by side. `a` sits across the middle
    /// of the strip with `b` overlapping it.
    ///
    /// That is the shape of the residue this tier exists for: eject either
    /// piece alone and the *other* one is still parked across the middle, so
    /// neither of the two bands it leaves - below it and above it - is 40 long
    /// enough to take `a`, and the width leaves no room to go around. Eject
    /// both and the strip is empty, so the pair simply re-stacks.
    fn interlocked_pair() -> (Vec<PolygonSet>, Vec<GeneralFastPlacement>) {
        (
            vec![rect(20.0, 40.0), rect(40.0, 20.0)],
            vec![
                replacement_placement("a", 5.0, 20.0),
                replacement_placement("b", 5.0, 30.0),
            ],
        )
    }

    #[test]
    fn joint_replacement_repairs_a_pair_single_piece_replacement_cannot_solve() {
        let ids = ["a", "b"];
        let (polygons, placements) = interlocked_pair();
        let pieces = replacement_pieces(&ids, &polygons);
        let settings = replacement_settings(55.0, 300.0);
        assert!(validate_and_measure_placements(&pieces, &placements, settings).is_err());

        // Tier two ejects a vertex cover - one endpoint of the single violating
        // pair - and provably cannot re-place it: the piece left behind is the
        // thing blocking every band.
        let single = replacement_repair(&pieces, &placements, settings, 80.0, false);
        assert!(single.repaired.is_none(), "{:?}", single.diagnostics);
        assert_eq!(
            single.diagnostics.ejected_count, 1,
            "{:?}",
            single.diagnostics
        );
        assert_eq!(
            single.diagnostics.failed_piece_id.as_deref(),
            Some("a"),
            "{:?}",
            single.diagnostics
        );

        // Tier three ejects the whole component and re-places it jointly.
        let joint = joint_replacement_repair(&pieces, &placements, settings, 80.0, false);
        let diagnostics = &joint.diagnostics;
        assert!(diagnostics.attempted, "{diagnostics:?}");
        assert_eq!(diagnostics.violating_pairs, 1, "{diagnostics:?}");
        assert_eq!(diagnostics.ejected_count, 2, "{diagnostics:?}");
        assert_eq!(
            diagnostics.ejected_piece_ids,
            vec!["a".to_owned(), "b".to_owned()],
            "{diagnostics:?}"
        );
        assert_eq!(diagnostics.kept_violating_pairs, 0, "{diagnostics:?}");
        // Two pieces is two orders, and the enumeration is exhaustive.
        assert_eq!(diagnostics.orders_planned, 2, "{diagnostics:?}");
        assert!(diagnostics.orders_exhaustive, "{diagnostics:?}");
        assert!(diagnostics.orders_tried >= 1, "{diagnostics:?}");
        assert!(diagnostics.exact_valid, "{diagnostics:?}");
        assert!(diagnostics.accepted_order.is_some(), "{diagnostics:?}");

        let repaired = joint.repaired.expect("a repaired layout");
        // The pass never publishes on its own authority.
        validate_and_measure_placements(&pieces, &repaired, settings)
            .expect("the jointly repaired layout validates");
        for placement in &repaired {
            assert!(placement.translate_long_axis <= 80.0, "{placement:?}");
        }
    }

    #[test]
    fn joint_replacement_ejects_the_whole_component_not_a_vertex_cover() {
        // The two tiers are pointed at the same residue and disagree only about
        // how much of it to lift out. That difference is the mechanism.
        let ids = ["a", "b"];
        let (polygons, placements) = interlocked_pair();
        let pieces = replacement_pieces(&ids, &polygons);
        let settings = replacement_settings(55.0, 300.0);

        let single = replacement_repair(&pieces, &placements, settings, 80.0, false);
        let joint = joint_replacement_repair(&pieces, &placements, settings, 80.0, false);
        assert_eq!(single.diagnostics.ejected_count, 1);
        assert_eq!(joint.diagnostics.ejected_count, 2);
        assert_eq!(
            joint.diagnostics.largest_component_pieces, 2,
            "{:?}",
            joint.diagnostics
        );
    }

    #[test]
    fn joint_replacement_is_deterministic() {
        let ids = ["a", "b"];
        let (polygons, placements) = interlocked_pair();
        let pieces = replacement_pieces(&ids, &polygons);
        let settings = replacement_settings(55.0, 300.0);

        let first = joint_replacement_repair(&pieces, &placements, settings, 80.0, false);
        let second = joint_replacement_repair(&pieces, &placements, settings, 80.0, false);
        assert_eq!(first.diagnostics, second.diagnostics);
        assert_eq!(first.repaired, second.repaired);
    }

    fn rotatable_replacement_pieces<'a>(
        ids: &'a [&'a str],
        polygons: &'a [PolygonSet],
    ) -> Vec<GeneralFastPiece<'a>> {
        ids.iter()
            .enumerate()
            .map(|(index, id)| GeneralFastPiece {
                id,
                polygon: &polygons[index],
                allow_rotation: true,
                allow_mirror: true,
            })
            .collect()
    }

    #[test]
    fn orientation_ladder_is_geometric_and_scale_free() {
        // An angle carries no length, so the ladder is a request-independent
        // constant set rather than anything derived from an instance. What has
        // to hold is that it is a *ladder*: strictly ascending, one fixed
        // ratio, spanning the band between "finer than the placement grid can
        // express" and "no longer a local repair".
        let ladder = ORIENTATION_PERTURBATION_LADDER_DEG;
        assert!(ladder[0] > 0.0, "{ladder:?}");
        for window in ladder.windows(2) {
            assert!(window[1] > window[0], "{ladder:?}");
            let ratio = window[1] / window[0];
            assert!((ratio - 2.5).abs() < 1e-12, "{ladder:?} ratio {ratio}");
        }
        assert!(*ladder.last().expect("a last rung") < 5.0, "{ladder:?}");
        // Every rung is representable on the angle grid, so no two rungs can
        // collapse onto one another and re-spend the same charged rows.
        let mut keys = ladder.iter().map(|rung| angle_key(*rung)).collect::<Vec<_>>();
        keys.dedup();
        assert_eq!(keys.len(), ladder.len(), "{ladder:?}");
        // The floor's own justification, stated as arithmetic rather than as a
        // comment: a rung `d` moves a vertex at radius `r` by `r * d * pi/180`,
        // and the finest rung has to clear one 0.001 mm pose-grid quantum on a
        // hand-sized radius or the pose stream emits angles the grid rounds
        // away. 100 mm is the hand-sized radius the ladder is argued on.
        const HAND_SIZED_RADIUS_MM: f64 = 100.0;
        const POSE_GRID_QUANTUM_MM: f64 = 0.001;
        let finest_travel_mm = HAND_SIZED_RADIUS_MM * ladder[0].to_radians();
        assert!(
            finest_travel_mm > POSE_GRID_QUANTUM_MM,
            "{ladder:?} finest rung travels {finest_travel_mm} mm"
        );
        assert_eq!(
            ORIENTATION_PERTURBATION_VARIANTS,
            4 * ladder.len() + 1,
            "{ladder:?}"
        );
    }

    #[test]
    fn orientation_variants_follow_the_ladder_and_the_request_freedoms() {
        let polygons = vec![rect(20.0, 40.0)];
        let settings = replacement_settings(200.0, 300.0);
        let vacated = RelaxedPlacement {
            input_index: 0,
            rotation_deg: 17.5,
            mirrored: false,
            translate_x: 30.0,
            translate_y: 40.0,
        };

        let free = GeneralFastPiece {
            id: "a",
            polygon: &polygons[0],
            allow_rotation: true,
            allow_mirror: true,
        };
        let mut work = RunWork::new(1);
        let variants = orientation_perturbation_variants(free, &vacated, settings, &mut work)
            .expect("variants build");
        assert_eq!(
            variants.len(),
            ORIENTATION_PERTURBATION_VARIANTS,
            "{variants:?}"
        );
        // The vacated orientation is never re-emitted: it is the anchor-local
        // stream's own leading candidate.
        assert!(
            !variants.iter().any(|variant| {
                (angle_key(variant.rotation_deg), variant.mirrored)
                    == (angle_key(vacated.rotation_deg), vacated.mirrored)
            }),
            "{variants:?}"
        );
        // Rotation rungs lead, ascending in magnitude with the positive sign
        // first; the mirror family follows, so a budget cut truncates the
        // mirror variants before the rotation variants.
        let rotation_family = 2 * ORIENTATION_PERTURBATION_LADDER_DEG.len();
        for (index, variant) in variants.iter().enumerate() {
            assert_eq!(
                variant.mirrored,
                index >= rotation_family,
                "{index} {variant:?}"
            );
            assert!(variant.max_x > variant.min_x, "{variant:?}");
            assert!(variant.max_y > variant.min_y, "{variant:?}");
        }
        assert_eq!(
            variants[0].rotation_deg,
            continuous_angle(vacated.rotation_deg + ORIENTATION_PERTURBATION_LADDER_DEG[0]),
            "{variants:?}"
        );
        assert_eq!(
            variants[1].rotation_deg,
            continuous_angle(vacated.rotation_deg - ORIENTATION_PERTURBATION_LADDER_DEG[0]),
            "{variants:?}"
        );

        // A piece the request pins gets only the freedoms it actually has.
        let mirror_only = GeneralFastPiece {
            allow_rotation: false,
            ..free
        };
        let mut work = RunWork::new(1);
        let variants =
            orientation_perturbation_variants(mirror_only, &vacated, settings, &mut work)
                .expect("variants build");
        assert_eq!(variants.len(), 1, "{variants:?}");
        assert!(variants[0].mirrored, "{variants:?}");

        let pinned = GeneralFastPiece {
            allow_rotation: false,
            allow_mirror: false,
            ..free
        };
        let mut work = RunWork::new(1);
        let variants = orientation_perturbation_variants(pinned, &vacated, settings, &mut work)
            .expect("variants build");
        assert!(variants.is_empty(), "{variants:?}");
    }

    #[test]
    fn orientation_perturbation_is_a_no_op_on_a_pinned_instance() {
        // `replacement_pieces` pins both orientation freedoms, so modes 32 and
        // 33 have no variant to seed and must reproduce modes 28 and 29 to the
        // last field - including leaving the attribution block off entirely.
        let ids = ["a", "b"];
        let (polygons, placements) = interlocked_pair();
        let pieces = replacement_pieces(&ids, &polygons);
        let settings = replacement_settings(55.0, 300.0);

        let legacy = replacement_repair(&pieces, &placements, settings, 80.0, false);
        let perturbed = replacement_repair(&pieces, &placements, settings, 80.0, true);
        assert_eq!(legacy.diagnostics, perturbed.diagnostics);
        assert_eq!(legacy.repaired, perturbed.repaired);
        assert!(
            perturbed
                .diagnostics
                .pieces
                .iter()
                .all(|row| row.orientation.is_none()),
            "{:?}",
            perturbed.diagnostics
        );

        let legacy = joint_replacement_repair(&pieces, &placements, settings, 80.0, false);
        let perturbed = joint_replacement_repair(&pieces, &placements, settings, 80.0, true);
        assert_eq!(legacy.diagnostics, perturbed.diagnostics);
        assert_eq!(legacy.repaired, perturbed.repaired);
    }

    /// The two-square residue of `replacement_repair_ejects_the_conflict_and_rebuilds_it`
    /// with both orientation freedoms granted, which is what arms the stream.
    fn rotatable_conflict() -> (Vec<PolygonSet>, Vec<GeneralFastPlacement>) {
        (
            vec![square(20.0), square(20.0)],
            vec![
                replacement_placement("a", 20.0, 20.0),
                replacement_placement("b", 41.0, 20.0),
            ],
        )
    }

    #[test]
    fn orientation_perturbation_is_additive_and_attributes_the_accepted_pose() {
        let ids = ["a", "b"];
        let (polygons, placements) = rotatable_conflict();
        let pieces = rotatable_replacement_pieces(&ids, &polygons);
        let settings = replacement_settings(200.0, 300.0);
        assert!(validate_and_measure_placements(&pieces, &placements, settings).is_err());

        let legacy = replacement_repair(&pieces, &placements, settings, 200.0, false);
        let perturbed = replacement_repair(&pieces, &placements, settings, 200.0, true);
        assert!(legacy.repaired.is_some(), "{:?}", legacy.diagnostics);

        for (row, legacy_row) in perturbed
            .diagnostics
            .pieces
            .iter()
            .zip(legacy.diagnostics.pieces.iter())
        {
            let orientation = row.orientation.as_ref().expect("an armed attribution block");
            assert_eq!(
                orientation.variants, ORIENTATION_PERTURBATION_VARIANTS,
                "{orientation:?}"
            );
            assert!(orientation.candidates > 0, "{orientation:?}");
            assert!(
                orientation.rows <= ORIENTATION_PERTURBATION_ROWS,
                "{orientation:?}"
            );
            // Legacy-first: the orientation stream is ranked behind every
            // anchor-local candidate, so it cannot displace an anchor-local
            // finalist mode 28 would have found.
            assert_eq!(
                row.anchor_local_finalists, legacy_row.anchor_local_finalists,
                "{row:?} vs {legacy_row:?}"
            );
            // The accepted-pose attribution is exclusive and totals one on a
            // piece that found a pose, zero on a piece that did not.
            let accepted = orientation.accepted_vacated
                + orientation.accepted_anchor_local
                + orientation.accepted_orientation
                + orientation.accepted_station;
            assert_eq!(accepted, usize::from(row.replaced), "{row:?}");
            assert_eq!(
                orientation.accepted_rotation_deg.is_some(),
                row.replaced,
                "{row:?}"
            );
            if orientation.accepted_orientation == 0 && row.replaced {
                // Anything the legacy stream reached keeps the vacated
                // orientation exactly, which is the property the whole
                // pose-entry negative measured.
                assert_eq!(
                    orientation.accepted_rotation_delta_deg,
                    Some(0.0),
                    "{orientation:?}"
                );
                assert_eq!(
                    orientation.accepted_mirror_flipped,
                    Some(false),
                    "{orientation:?}"
                );
            }
        }
    }

    #[test]
    fn orientation_perturbation_is_deterministic() {
        let ids = ["a", "b"];
        let (polygons, placements) = rotatable_conflict();
        let pieces = rotatable_replacement_pieces(&ids, &polygons);
        let settings = replacement_settings(200.0, 300.0);

        let first = replacement_repair(&pieces, &placements, settings, 200.0, true);
        let second = replacement_repair(&pieces, &placements, settings, 200.0, true);
        assert_eq!(first.diagnostics, second.diagnostics);
        assert_eq!(first.repaired, second.repaired);

        let first = joint_replacement_repair(&pieces, &placements, settings, 200.0, true);
        let second = joint_replacement_repair(&pieces, &placements, settings, 200.0, true);
        assert_eq!(first.diagnostics, second.diagnostics);
        assert_eq!(first.repaired, second.repaired);
    }

    #[test]
    fn joint_replacement_fails_cleanly_when_no_order_or_swap_works() {
        // The same interlocked pair under a clamp too short to stack them. No
        // insertion order can succeed, so the swap round runs, and then the
        // finalist-combination beam spends every non-greedy rank pair before
        // the pass reports the exhausted search rather than publishing
        // anything. The attempt plan's shape is the contract here: the plain
        // orders come first and the swap after them, exactly where they always
        // were, so a state this tier used to publish still publishes by the
        // route it used to; the beam only ever runs after both.
        let ids = ["a", "b"];
        let (polygons, placements) = interlocked_pair();
        let pieces = replacement_pieces(&ids, &polygons);
        let settings = replacement_settings(55.0, 300.0);

        let outcome = joint_replacement_repair(&pieces, &placements, settings, 55.0, false);
        let diagnostics = &outcome.diagnostics;
        assert!(outcome.repaired.is_none(), "{diagnostics:?}");
        assert!(!diagnostics.exact_valid, "{diagnostics:?}");
        assert!(diagnostics.attempted, "{diagnostics:?}");
        assert_eq!(diagnostics.orders_tried, 2, "{diagnostics:?}");
        // Every plain order failed, so the one available exchange was tried.
        assert_eq!(diagnostics.swap_rounds_run, 1, "{diagnostics:?}");
        assert_eq!(diagnostics.swap_pairs_planned, 1, "{diagnostics:?}");
        assert_eq!(diagnostics.swap_attempts_tried, 1, "{diagnostics:?}");
        // One component, ejecting both pieces, so the beam's plan is every
        // rank pair but the greedy one: 4 * 4 - 1 = 15.
        assert_eq!(diagnostics.component_passes_run, 1, "{diagnostics:?}");
        assert_eq!(diagnostics.components.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics.components_repaired, 0, "{diagnostics:?}");
        assert_eq!(diagnostics.beam_combinations_tried, 15, "{diagnostics:?}");
        assert_eq!(diagnostics.orders.len(), 2 + 1 + 15, "{diagnostics:?}");
        assert!(
            diagnostics.orders[..3]
                .iter()
                .all(|row| row.finalist_ranks.is_none()),
            "{diagnostics:?}"
        );
        assert_eq!(
            diagnostics.orders[2].swap_pair,
            Some(vec!["a".to_owned(), "b".to_owned()]),
            "{diagnostics:?}"
        );
        assert_eq!(
            diagnostics.orders[3].finalist_ranks,
            Some(vec![0, 1]),
            "{diagnostics:?}"
        );
        assert_eq!(
            diagnostics.orders[17].finalist_ranks,
            Some(vec![3, 3]),
            "{diagnostics:?}"
        );
        assert!(diagnostics.cap_exhausted.is_none(), "{diagnostics:?}");
        assert!(
            diagnostics
                .rejection_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("no insertion order")),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn joint_replacement_repairs_independent_components_one_at_a_time() {
        // Two independent two-piece conflicts on one sheet. The pooled pass
        // ejected all four at once and refused on an ejection cap that neither
        // conflict individually trips; the per-component pass repairs them in
        // sequence and re-surveys between them.
        let ids = ["a", "b", "c", "d"];
        let polygons = vec![square(20.0), square(20.0), square(20.0), square(20.0)];
        let pieces = replacement_pieces(&ids, &polygons);
        let settings = replacement_settings(400.0, 400.0);
        // Two overlapping pairs, far enough apart that their violation graphs
        // never touch.
        let placements = vec![
            replacement_placement("a", 40.0, 40.0),
            replacement_placement("b", 45.0, 40.0),
            replacement_placement("c", 300.0, 300.0),
            replacement_placement("d", 305.0, 300.0),
        ];
        let violations =
            survey_layout_violations(&pieces, &placements, settings).expect("a surveyable layout");
        assert_eq!(violations.pair_components().len(), 2, "{violations:?}");

        let outcome = joint_replacement_repair(&pieces, &placements, settings, 400.0, false);
        let diagnostics = &outcome.diagnostics;
        assert_eq!(diagnostics.component_passes_run, 2, "{diagnostics:?}");
        assert_eq!(diagnostics.components.len(), 2, "{diagnostics:?}");
        assert!(
            diagnostics
                .components
                .iter()
                .all(|row| row.piece_ids.len() == 2),
            "{diagnostics:?}"
        );
        assert_eq!(diagnostics.components_repaired, 2, "{diagnostics:?}");
        assert_eq!(diagnostics.ejected_count, 4, "{diagnostics:?}");
        assert!(diagnostics.exact_valid, "{diagnostics:?}");
        let repaired = outcome.repaired.expect("a repaired layout");
        validate_and_measure_placements(&pieces, &repaired, settings)
            .expect("the published layout validates against the real request");
    }

    #[test]
    fn joint_replacement_refuses_a_layout_with_no_violating_pair() {
        let ids = ["a", "b"];
        let polygons = vec![square(20.0), square(20.0)];
        let pieces = replacement_pieces(&ids, &polygons);
        let settings = replacement_settings(200.0, 300.0);
        let placements = vec![
            replacement_placement("a", 20.0, 20.0),
            replacement_placement("b", 60.0, 20.0),
        ];
        assert!(validate_and_measure_placements(&pieces, &placements, settings).is_ok());

        let outcome = joint_replacement_repair(&pieces, &placements, settings, 200.0, false);
        assert!(outcome.repaired.is_none());
        assert!(!outcome.diagnostics.attempted);
        assert!(
            outcome
                .diagnostics
                .skipped_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("no violating pair")),
            "{:?}",
            outcome.diagnostics
        );
    }

    #[test]
    fn joint_replacement_refuses_an_oversized_violation_component() {
        // The joint tier admits on exactly the terms the single-piece tier
        // does: a component larger than a local repair may touch is a search
        // problem for both.
        let count = 8;
        let ids = (0..count)
            .map(|index| format!("p{index}"))
            .collect::<Vec<_>>();
        let id_refs = ids.iter().map(String::as_str).collect::<Vec<_>>();
        let polygons = (0..count).map(|_| square(20.0)).collect::<Vec<_>>();
        let pieces = replacement_pieces(&id_refs, &polygons);
        let settings = replacement_settings(400.0, 300.0);
        let placements = (0..count)
            .map(|index| replacement_placement(&ids[index], 20.0 + 21.0 * index as f64, 20.0))
            .collect::<Vec<_>>();

        let outcome = joint_replacement_repair(&pieces, &placements, settings, 200.0, false);
        let diagnostics = &outcome.diagnostics;
        assert!(outcome.repaired.is_none());
        assert!(!diagnostics.attempted, "{diagnostics:?}");
        assert_eq!(diagnostics.component_limit, 4, "{diagnostics:?}");
        assert!(
            diagnostics
                .skipped_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("local-repair limit")),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn joint_replacement_orders_enumerate_lexicographically_and_stay_bounded() {
        // Small sets get every permutation, identity first - so the joint
        // pass's own first attempt is the canonical single-piece order.
        let (orders, exhaustive) = joint_replacement_orders(&[7, 3, 5]);
        assert!(exhaustive);
        assert_eq!(
            orders,
            vec![
                vec![7, 3, 5],
                vec![7, 5, 3],
                vec![3, 7, 5],
                vec![3, 5, 7],
                vec![5, 7, 3],
                vec![5, 3, 7],
            ]
        );
        let (orders, exhaustive) = joint_replacement_orders(&[0, 1, 2, 3]);
        assert!(exhaustive);
        assert_eq!(orders.len(), JOINT_REPLACEMENT_ORDER_CAP);
        assert_eq!(orders[0], vec![0, 1, 2, 3]);
        assert_eq!(orders[JOINT_REPLACEMENT_ORDER_CAP - 1], vec![3, 2, 1, 0]);
        // Every order is a permutation of the set, and no two repeat.
        let distinct = orders.iter().cloned().collect::<BTreeSet<_>>();
        assert_eq!(distinct.len(), orders.len());
        for order in &orders {
            let mut sorted = order.clone();
            sorted.sort_unstable();
            assert_eq!(sorted, vec![0, 1, 2, 3]);
        }

        // Above the permuted ceiling the plan is the rotation family: every
        // piece gets a turn at going in first, and the count stays bounded.
        let (orders, exhaustive) = joint_replacement_orders(&[10, 20, 30, 40, 50]);
        assert!(!exhaustive);
        assert_eq!(orders.len(), 5);
        assert_eq!(orders[0], vec![10, 20, 30, 40, 50]);
        assert_eq!(orders[1], vec![20, 30, 40, 50, 10]);
        assert_eq!(orders[4], vec![50, 10, 20, 30, 40]);
        let big = (0..64).collect::<Vec<usize>>();
        let (orders, exhaustive) = joint_replacement_orders(&big);
        assert!(!exhaustive);
        assert_eq!(orders.len(), JOINT_REPLACEMENT_ORDER_CAP);
    }

    #[test]
    fn joint_replacement_refuses_an_order_that_is_not_a_permutation() {
        // The primitive is shared, and an override that quietly re-placed a
        // different set would corrupt a layout rather than fail a repair.
        let ids = ["a", "b"];
        let (polygons, placements) = interlocked_pair();
        let pieces = replacement_pieces(&ids, &polygons);
        let settings = replacement_settings(55.0, 300.0);
        let bound_settings = GeneralFastSettings {
            sheet_long_axis_mm: 80.0,
            ..settings
        };
        let anchor =
            relaxed_state_from_fast_placements(&pieces, &placements, 80.0).expect("an anchor");
        let mut work = RunWork::new(pieces.len());
        let outcome = replace_ejected_under_bound(
            &pieces,
            settings,
            bound_settings,
            80.0,
            &anchor,
            &[0, 1],
            Some(&[0, 0]),
            None,
            JOINT_REPLACEMENT_SEED_DOMAIN,
            &AnchorLocalSeeding::disabled(),
            &mut work,
        )
        .err()
        .expect("a non-permutation override is refused");
        assert!(outcome.contains("permutation"), "{outcome}");
    }

    /// A layout with one piece sealed inside a genuine interior pocket: a
    /// three-by-three grid of squares on a sheet cut to fit it exactly, so the
    /// centre cell is bounded on all four sides by pieces and the strip has no
    /// floor, no shoulder and - under the clamp - no shelf. `nudge_mm`
    /// displaces the centre piece into its right-hand neighbour, which is the
    /// layout's only violation.
    ///
    /// This is the miniature of the residue class the whole primitive exists
    /// for. A top-frontier generator plants its hints at skyline valleys, and
    /// the skyline over the centre column is the *top* row's upper edge, so no
    /// station it can produce is anywhere near the pocket.
    fn interior_pocket_layout(nudge_mm: f64) -> (Vec<PolygonSet>, Vec<GeneralFastPlacement>, f64) {
        let polygons = (0..9).map(|_| square(20.0)).collect::<Vec<_>>();
        // A 25.02 mm pitch under a 5 mm contract leaves 5.02 mm of clearance
        // in both axes: legal, and record-tight - the centre cell's feasible
        // translations are a 0.02 mm square. A generator that samples poses
        // has to *hit* that square; a generator seeded at the pose the piece
        // was lifted out of starts a nudge away from it. (Nine pieces is few
        // enough that the position stream does hit it too, so this fixture
        // proves the seeding reaches the pocket, not that nothing else can.)
        let placements = POCKET_IDS
            .iter()
            .enumerate()
            .map(|(slot, id)| {
                let nudge = if slot == POCKET_SLOT { nudge_mm } else { 0.0 };
                replacement_placement(
                    id,
                    20.0 + POCKET_PITCH_MM * (slot % 3) as f64 + nudge,
                    20.0 + POCKET_PITCH_MM * (slot / 3) as f64,
                )
            })
            .collect::<Vec<_>>();
        (polygons, placements, POCKET_SHEET_SHORT_AXIS_MM)
    }

    /// The pocket sheet is cut to the grid plus the sheet edge clearance on
    /// every side, in both axes. Used as the clamp too, so there is no shelf
    /// above the frontier either.
    const POCKET_PITCH_MM: f64 = 25.02;
    // The trailing hundredth keeps the outermost pieces off the canonical
    // grid's boundary rather than exactly on it.
    const POCKET_SHEET_SHORT_AXIS_MM: f64 = 20.0 + 2.0 * POCKET_PITCH_MM + 20.0 + 5.01;
    const POCKET_SLOT: usize = 4;

    const POCKET_IDS: [&str; 9] = [
        "r0-c0", "r0-c1", "r0-c2", "r1-c0", "pocket", "r1-c2", "r2-c0", "r2-c1", "r2-c2",
    ];

    /// Runs the shared re-placement primitive over the pocket layout with
    /// anchor-local seeding either armed or disabled, and returns the pass.
    fn pocket_pass(
        pieces: &[GeneralFastPiece<'_>],
        placements: &[GeneralFastPlacement],
        settings: GeneralFastSettings,
        bound_mm: f64,
        anchor_local: &AnchorLocalSeeding,
    ) -> BoundedReplacementPass {
        let bound_settings = GeneralFastSettings {
            sheet_long_axis_mm: bound_mm,
            ..settings
        };
        let anchor = relaxed_state_from_fast_placements(pieces, placements, bound_mm)
            .expect("an anchor state");
        let mut work = RunWork::new(pieces.len());
        replace_ejected_under_bound(
            pieces,
            settings,
            bound_settings,
            bound_mm,
            &anchor,
            &[POCKET_SLOT],
            None,
            None,
            REPLACEMENT_REPAIR_SEED_DOMAIN,
            anchor_local,
            &mut work,
        )
        .expect("a completed pass")
    }

    #[test]
    fn anchor_local_seeding_reaches_the_pocket_a_piece_was_lifted_out_of() {
        // The nudge control, in miniature. `pocket` is displaced 1 mm into its
        // right-hand neighbour and then ejected; every other piece is left
        // exactly where it was, so the pose it was nudged out of provably
        // still fits, and at this pitch it is one of a 0.02 mm square of poses
        // that do.
        let (polygons, placements, bound_mm) = interior_pocket_layout(1.0);
        let pieces = replacement_pieces(&POCKET_IDS, &polygons);
        let settings = replacement_settings(POCKET_SHEET_SHORT_AXIS_MM, 300.0);
        let vacated = &placements[POCKET_SLOT];

        // The control arm: exactly today's skyline-only generator, which is
        // also the identity check that keeps the from-scratch constructor
        // bit-identical - it must seed nothing anchor-locally.
        let skyline_only = pocket_pass(
            &pieces,
            &placements,
            settings,
            bound_mm,
            &AnchorLocalSeeding::disabled(),
        );
        let control = &skyline_only.pieces[0];
        assert_eq!(control.anchor_local_candidates, 0, "{control:?}");
        assert_eq!(control.anchor_local_finalists, 0, "{control:?}");

        // The treatment arm: the same ejection, the same occupancy, the same
        // confirmation machinery, seeded at the vacated pose as well. The
        // conflict's own residue is the 1 mm the nudge cost the pair.
        let pass = pocket_pass(
            &pieces,
            &placements,
            settings,
            bound_mm,
            &AnchorLocalSeeding::with_residue_scale(Some(1.0)),
        );
        let row = &pass.pieces[0];
        assert!(row.anchor_local_candidates > 0, "{row:?}");
        assert!(
            row.anchor_local_finalists > 0,
            "the pocket must be reachable from the anchor: {row:?}"
        );
        assert!(row.placed_extent_mm.is_some(), "{row:?}");

        // And it went back into the pocket rather than somewhere else: the
        // placed pose is within the nudge of the pose it was lifted out of.
        let repaired = fast_placements(&pass.state, &pieces, false);
        let placed = repaired
            .iter()
            .find(|placement| placement.piece_id == "pocket")
            .expect("the re-placed piece");
        assert!(
            (placed.translate_short_axis - vacated.translate_short_axis).abs() <= 1.0
                && (placed.translate_long_axis - vacated.translate_long_axis).abs() <= 1.0,
            "{placed:?} is not anchor-local to {vacated:?}"
        );

        // What it placed is a real layout, not just a confirmed row.
        validate_and_measure_placements(&pieces, &repaired, settings)
            .expect("the re-placed layout validates");
        let depth_mm = coupled_independent_source_depth(&pieces, &repaired, settings)
            .expect("a measurable depth");
        assert!(depth_mm <= bound_mm, "{depth_mm} exceeds {bound_mm}");
    }

    #[test]
    fn anchor_local_seeding_is_deterministic() {
        let (polygons, placements, bound_mm) = interior_pocket_layout(1.0);
        let pieces = replacement_pieces(&POCKET_IDS, &polygons);
        let settings = replacement_settings(POCKET_SHEET_SHORT_AXIS_MM, 300.0);
        let seeding = AnchorLocalSeeding::with_residue_scale(Some(1.0));

        let first = pocket_pass(&pieces, &placements, settings, bound_mm, &seeding);
        let second = pocket_pass(&pieces, &placements, settings, bound_mm, &seeding);
        assert_eq!(first.order_hash, second.order_hash);
        assert_eq!(
            fast_placements(&first.state, &pieces, false),
            fast_placements(&second.state, &pieces, false)
        );
        assert_eq!(
            first.pieces[0].anchor_local_candidates,
            second.pieces[0].anchor_local_candidates
        );
        assert_eq!(
            first.pieces[0].anchor_local_finalists,
            second.pieces[0].anchor_local_finalists
        );
    }

    #[test]
    fn anchor_local_cloud_stays_inside_its_declared_bounds() {
        // Disabled is the identity: no magnitudes, so no cloud at all.
        let disabled = AnchorLocalSeeding::disabled();
        assert!(disabled.magnitudes_mm(40.0, 90.0).is_empty());
        assert!(disabled.directions(0).is_empty());

        // Magnitudes: derived from the smaller extent, ascending, unique on
        // the placement grid, and never more than the declared count.
        let mut seeding = AnchorLocalSeeding::with_residue_scale(Some(0.25));
        let magnitudes = seeding.magnitudes_mm(40.0, 90.0);
        assert!(
            magnitudes.len() <= ANCHOR_LOCAL_MAGNITUDES,
            "{magnitudes:?}"
        );
        assert!(magnitudes.iter().all(|magnitude| *magnitude > 0.0));
        assert!(
            magnitudes.windows(2).all(|pair| pair[0] < pair[1]),
            "{magnitudes:?}"
        );
        // Scale-free: doubling the piece doubles every extent-derived rung.
        let doubled = seeding.magnitudes_mm(80.0, 180.0);
        for fraction in ANCHOR_LOCAL_EXTENT_FRACTIONS {
            assert!(
                doubled
                    .iter()
                    .any(|magnitude| grid_key(*magnitude) == grid_key(snap_mm(80.0 * fraction))),
                "{doubled:?} is missing {fraction}"
            );
        }
        // With no measured residue the cloud is sized from the piece alone.
        let extent_only = AnchorLocalSeeding {
            enabled: true,
            ..AnchorLocalSeeding::default()
        };
        assert_eq!(
            extent_only.magnitudes_mm(40.0, 90.0).len(),
            ANCHOR_LOCAL_EXTENT_FRACTIONS.len()
        );

        // Directions: aimed ones first, then the fan, all unit, all distinct,
        // and capped.
        seeding
            .separating_directions
            .insert(3, vec![(1.0, 0.0), (0.0, 2.0), (1.0, 0.0)]);
        seeding.projected_displacements.insert(3, vec![(0.0, -4.0)]);
        let directions = seeding.directions(3);
        assert_eq!(directions[0], (0.0, -1.0), "{directions:?}");
        assert_eq!(directions[1], (1.0, 0.0), "{directions:?}");
        assert!(
            directions.len()
                <= 1 + ANCHOR_LOCAL_SEPARATION_DIRECTIONS + 1 + ANCHOR_LOCAL_FAN_DIRECTIONS.len(),
            "{directions:?}"
        );
        for (index, direction) in directions.iter().enumerate() {
            assert!(
                (direction.0.hypot(direction.1) - 1.0).abs() < 1e-9,
                "{direction:?}"
            );
            assert!(
                !directions[..index]
                    .iter()
                    .any(|kept| grid_key(kept.0) == grid_key(direction.0)
                        && grid_key(kept.1) == grid_key(direction.1)),
                "{directions:?} repeats at {index}"
            );
        }

        // The whole cloud is bounded by the product of the two, plus the
        // vacated pose and the projection trajectory.
        let cloud = magnitudes.len() * directions.len()
            + 1
            + seeding.projected_displacements(3).len()
            + CONSTRUCTION_HINT_PRIORS;
        assert!(
            cloud
                <= ANCHOR_LOCAL_MAGNITUDES
                    * (1 + ANCHOR_LOCAL_SEPARATION_DIRECTIONS
                        + 1
                        + ANCHOR_LOCAL_FAN_DIRECTIONS.len())
                    + 1
                    + ANCHOR_LOCAL_PROJECTION_ITERATES
                    + CONSTRUCTION_HINT_PRIORS,
            "{cloud}"
        );
    }

    #[test]
    fn separating_projection_aims_at_the_pocket_it_was_pushed_out_of() {
        // The projection is the aimed input the cloud rides: it measures the
        // conflict rather than sampling around it, so a piece nudged a
        // millimetre into its neighbour must come back with a translation of
        // about that size pointing the other way.
        let (polygons, placements, _) = interior_pocket_layout(1.0);
        let pieces = replacement_pieces(&POCKET_IDS, &polygons);
        let settings = replacement_settings(POCKET_SHEET_SHORT_AXIS_MM, 300.0);
        let kept = placements
            .iter()
            .enumerate()
            .filter(|(slot, _)| *slot != POCKET_SLOT)
            .map(|(_, placement)| placement.clone())
            .chain(std::iter::once(placements[POCKET_SLOT].clone()))
            .collect::<Vec<_>>();

        let (trajectory, _) = separating_translation(
            &pieces,
            &kept,
            settings,
            kept.len() - 1,
            ANCHOR_LOCAL_PROJECTION_ITERATES,
        )
        .expect("a measurable projection");
        assert!(!trajectory.is_empty());
        assert!(
            trajectory.len() <= ANCHOR_LOCAL_PROJECTION_ITERATES,
            "{trajectory:?}"
        );
        let (shift_x, shift_y) = trajectory[trajectory.len() - 1];
        assert!(shift_x < 0.0, "the projection must aim back: {shift_x}");
        assert!(
            shift_y.abs() < 0.5,
            "and stay in the pocket's row: {shift_y}"
        );
    }
}
