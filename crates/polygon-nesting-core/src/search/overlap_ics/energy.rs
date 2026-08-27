//! Φ: the raw squared-hinge penalty, the continuous guided weights, and the
//! fixed-order fold that makes the incremental cache honest.
//!
//! ```text
//! v_ij   = max over cell pairs of [c_pair - s(A_a,B_b)]_+
//! Phi_raw    = sum over pairs of v_ij^2   +  sum over piece-edges of v_ie^2
//! Phi_guided = sum over pairs of w_ij v_ij^2 + sum over piece-edges of w_ie v_ie^2
//! ```
//!
//! `w` is an `f64` row scalar at or above 1, driven by [`gls_update`] - the
//! published Algorithm 8 schedule of arXiv:2509.13329, all rows, every sweep.
//! The two totals are folded together and only the raw one is ever compared
//! against a stall; neither is ever reported as quality.
//!
//! **The fixed-order fold is not an optimization, it is the defence.** A move
//! recomputes only the moving piece's `n-1` rows - but the totals are then
//! re-folded over *all* cached row scalars in pair-ID order, which is 1,830
//! additions on mixed-61 and not 1,830 geometry queries. Sol R2 §2 asks for
//! exactly this, to stop this engine inheriting the incremental-tracker drift
//! defect the relaxed lane had. `incremental_rows_match_cold_rebuild` in the
//! module's tests is the enforcement.

use super::broad_phase::{boundary_residuals, pair_is_near};
use super::contact::{box_gap, convex_cell_gap, convex_cell_gap_cached, Contact};
use super::diagnostics::WorkVector;
use super::state::{
    pair_index, Contract, EdgeRow, Geometry, IcsState, PairRow, EDGE_BOTTOM, EDGE_LEFT, EDGE_RIGHT,
    EDGE_TOP, GLS_WEIGHT_FLOOR,
};

/// The two folded totals plus the largest single residual.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Totals {
    pub raw: f64,
    pub guided: f64,
    /// `max_g`: the largest violation of any row, in millimetres. This is the
    /// number the publication band is measured against, never Φ itself.
    pub max_violation_mm: f64,
}

/// Measures one piece-pair row from the current geometry.
/// The contact a zero row carries: the same value `measure_pair` returns when
/// the broad phase rejects, written once so the near set's unlink and the
/// measurement agree by construction.
const EMPTY_CONTACT: Contact = Contact {
    signed_gap_mm: f64::INFINITY,
    normal: [0.0, 0.0],
    witness_a: [0.0, 0.0],
    witness_b: [0.0, 0.0],
};

pub fn measure_pair(
    geometry: &mut Geometry,
    first: usize,
    second: usize,
    clearance_mm: f64,
    work: &mut WorkVector,
) -> (f64, Contact) {
    work.pair_row_probes += 1;
    let empty = EMPTY_CONTACT;
    if !pair_is_near(
        geometry.piece_bounds[first],
        geometry.piece_bounds[second],
        clearance_mm,
    ) {
        work.broad_phase_rejects += 1;
        return (0.0, empty);
    }
    let (first_start, first_end) = geometry.piece_cells[first];
    let (second_start, second_end) = geometry.piece_cells[second];
    // The axes of every cell that can matter, computed once for this pose and
    // then reused by every cell pair and every neighbour this candidate is
    // measured against.
    for cell in first_start..first_end {
        geometry.ensure_cell_axes(cell);
    }
    for cell in second_start..second_end {
        geometry.ensure_cell_axes(cell);
    }
    let mut worst = 0.0f64;
    let mut worst_contact = empty;
    for a in first_start..first_end {
        for b in second_start..second_end {
            // The cell-level box proof, for the nonconvex pieces whose cells
            // are triangles: most triangle pairs of two adjacent decagons
            // cannot reach the clearance and never become a query.
            work.cell_pair_box_tests += 1;
            if box_gap(geometry.cell_bounds[a], geometry.cell_bounds[b]) >= clearance_mm {
                continue;
            }
            work.convex_cell_gap_queries += 1;
            let (a_axes, a_own) = geometry.cell_axes_slice(a);
            let (b_axes, b_own) = geometry.cell_axes_slice(b);
            let contact = convex_cell_gap_cached(
                geometry.cell_slice(a),
                a_axes,
                a_own,
                geometry.cell_slice(b),
                b_axes,
                b_own,
            );
            let violation = clearance_mm - contact.signed_gap_mm;
            if violation > worst {
                worst = violation;
                worst_contact = contact;
            }
        }
    }
    if worst <= 0.0 {
        (0.0, empty)
    } else {
        (worst, worst_contact)
    }
}

/// Measures one pair whose two pieces come from independently transformed
/// versions of the same layout.
///
/// The minimum-conflict binary-close experiment needs the four combinations
/// `(0,0)`, `(0,1)`, `(1,0)`, `(1,1)` without installing any of them in the
/// live engine. This is the existing pair-row authority with only the two
/// geometry operands split; broad phase, cell order, contact field and strict
/// maximum are otherwise identical to [`measure_pair`]. Its work vector is
/// owned by the diagnostic decision and is never added to the trajectory's
/// legacy currency.
#[cfg(feature = "minimum-conflict-binary-close")]
pub fn measure_pair_cross(
    first_geometry: &Geometry,
    first: usize,
    second_geometry: &Geometry,
    second: usize,
    clearance_mm: f64,
    work: &mut WorkVector,
) -> (f64, Contact) {
    work.pair_row_probes += 1;
    let empty = Contact {
        signed_gap_mm: f64::INFINITY,
        normal: [0.0, 0.0],
        witness_a: [0.0, 0.0],
        witness_b: [0.0, 0.0],
    };
    if !pair_is_near(
        first_geometry.piece_bounds[first],
        second_geometry.piece_bounds[second],
        clearance_mm,
    ) {
        work.broad_phase_rejects += 1;
        return (0.0, empty);
    }
    let (first_start, first_end) = first_geometry.piece_cells[first];
    let (second_start, second_end) = second_geometry.piece_cells[second];
    let mut worst = 0.0f64;
    let mut worst_contact = empty;
    for a in first_start..first_end {
        for b in second_start..second_end {
            if box_gap(
                first_geometry.cell_bounds[a],
                second_geometry.cell_bounds[b],
            ) >= clearance_mm
            {
                continue;
            }
            work.convex_cell_gap_queries += 1;
            let contact =
                convex_cell_gap(first_geometry.cell_slice(a), second_geometry.cell_slice(b));
            let violation = clearance_mm - contact.signed_gap_mm;
            if violation > worst {
                worst = violation;
                worst_contact = contact;
            }
        }
    }
    if worst <= 0.0 {
        (0.0, empty)
    } else {
        (worst, worst_contact)
    }
}

/// Measures the four boundary rows of one piece.
pub fn measure_edges(
    geometry: &Geometry,
    piece: usize,
    contract: &Contract,
    target_depth_mm: f64,
    previous: [EdgeRow; 4],
) -> [EdgeRow; 4] {
    let residuals = boundary_residuals(geometry.piece_bounds[piece], contract, target_depth_mm);
    let ring = geometry.ring_slice(piece);
    let mut rows = [EdgeRow::default(); 4];
    for edge in 0..4 {
        rows[edge].weight = previous[edge].weight;
        rows[edge].violation_mm = residuals[edge];
        if residuals[edge] <= 0.0 {
            continue;
        }
        // The witness is the extremal material vertex on that side, scanned in
        // ring order with a strict comparison so ties take the lowest index.
        let mut best = ring[0];
        for point in &ring[1..] {
            let replace = match edge {
                EDGE_LEFT => point[0] < best[0],
                EDGE_RIGHT => point[0] > best[0],
                EDGE_BOTTOM => point[1] < best[1],
                _ => point[1] > best[1],
            };
            if replace {
                best = *point;
            }
        }
        rows[edge].witness = best;
    }
    rows
}

/// Rebuilds **every** row from the geometry: the cold reconstruction the
/// incremental cache is tested against.
pub fn rebuild_all(state: &mut IcsState, contract: &Contract, work: &mut WorkVector) {
    let count = state.poses.len();
    let clearance = contract.pair_clearance_mm();
    for near in &mut state.near {
        near.clear();
    }
    for first in 0..count {
        for second in (first + 1)..count {
            let index = pair_index(count, first, second);
            let (violation, contact) =
                measure_pair(&mut state.geometry, first, second, clearance, work);
            state.pair_rows[index].violation_mm = violation;
            state.pair_rows[index].contact = contact;
            // The index is built in ascending `second` for each `first` and in
            // ascending `first` for each `second`, so both lists come out
            // sorted without a sort.
            if violation > 0.0 {
                state.near[first].push(second as u32);
                state.near[second].push(first as u32);
            }
        }
    }
    for near in &mut state.near {
        near.sort_unstable();
    }
    for piece in 0..count {
        state.edge_rows[piece] = measure_edges(
            &state.geometry,
            piece,
            contract,
            state.target_depth_mm,
            state.edge_rows[piece],
        );
    }
}

/// Rebuilds only the rows one moved piece touches: its `n-1` pair rows and its
/// four boundary rows.
pub fn rebuild_piece_rows(
    state: &mut IcsState,
    contract: &Contract,
    piece: usize,
    work: &mut WorkVector,
) {
    let count = state.poses.len();
    let clearance = contract.pair_clearance_mm();
    // Zero every row this piece currently owns, and unlink it from the other
    // end. A row that was zero before and is zero now is never touched at all,
    // which is the whole point: it is the reaching that costs, not the value.
    let previous = std::mem::take(&mut state.near[piece]);
    for &other in &previous {
        let other = other as usize;
        let (first, second) = if other < piece {
            (other, piece)
        } else {
            (piece, other)
        };
        let index = pair_index(count, first, second);
        state.pair_rows[index].violation_mm = 0.0;
        state.pair_rows[index].contact = EMPTY_CONTACT;
        if let Ok(at) = state.near[other].binary_search(&(piece as u32)) {
            state.near[other].remove(at);
        }
    }
    let mut current = previous;
    current.clear();
    for other in 0..count {
        if other == piece {
            continue;
        }
        let (first, second) = if other < piece {
            (other, piece)
        } else {
            (piece, other)
        };
        let (violation, contact) = measure_pair(&mut state.geometry, first, second, clearance, work);
        if violation > 0.0 {
            let index = pair_index(count, first, second);
            state.pair_rows[index].violation_mm = violation;
            state.pair_rows[index].contact = contact;
            current.push(other as u32);
            let insert = state.near[other]
                .binary_search(&(piece as u32))
                .unwrap_or_else(|at| at);
            state.near[other].insert(insert, piece as u32);
        }
    }
    state.near[piece] = current;
    state.edge_rows[piece] = measure_edges(
        &state.geometry,
        piece,
        contract,
        state.target_depth_mm,
        state.edge_rows[piece],
    );
}

/// The fixed-order scalar fold over every cached row.
///
/// Pair rows in pair-ID order first, then boundary rows in piece order, then
/// the four edges in `L, R, B, T` order. No reassociation, no parallel
/// reduction, no `sum()` over an unordered iterator.
pub fn fold(state: &IcsState) -> Totals {
    let mut totals = Totals::default();
    for row in &state.pair_rows {
        let violation = row.violation_mm;
        if violation <= 0.0 {
            continue;
        }
        let square = violation * violation;
        totals.raw += square;
        totals.guided += row.weight * square;
        if violation > totals.max_violation_mm {
            totals.max_violation_mm = violation;
        }
    }
    for rows in &state.edge_rows {
        for row in rows {
            let violation = row.violation_mm;
            if violation <= 0.0 {
                continue;
            }
            let square = violation * violation;
            totals.raw += square;
            totals.guided += row.weight * square;
            if violation > totals.max_violation_mm {
                totals.max_violation_mm = violation;
            }
        }
    }
    totals
}

/// The raw **and** guided energy incident on one piece: its `n-1` pair rows and
/// its four boundary rows, folded in the same fixed order.
///
/// This pair is a relocate's whole objective. Grok review 12 Round 2 §6.3 makes
/// the sample score lexicographic - "incident Φ = 0 beats any positive; else min
/// incident **weighted** Φ" - which is Algorithm 5/6's `Clear < Collision{loss}`
/// on our field, and it needs both halves of the fold at once. Computing them
/// together also means one pass over the same `n+3` rows instead of two.
pub fn incident_totals(state: &IcsState, piece: usize) -> (f64, f64) {
    let count = state.poses.len();
    let mut raw = 0.0;
    let mut guided = 0.0;
    // `near[piece]` is ascending and holds exactly the non-zero rows, so this
    // visits the same rows in the same order the `0..count` walk did once its
    // `violation_mm > 0.0` test had thrown the rest away. The sum is therefore
    // identical bit for bit, not merely equal.
    for &other in &state.near[piece] {
        let other = other as usize;
        let (first, second) = if other < piece {
            (other, piece)
        } else {
            (piece, other)
        };
        let row = &state.pair_rows[pair_index(count, first, second)];
        if row.violation_mm > 0.0 {
            let square = row.violation_mm * row.violation_mm;
            raw += square;
            guided += row.weight * square;
        }
    }
    for row in &state.edge_rows[piece] {
        if row.violation_mm > 0.0 {
            let square = row.violation_mm * row.violation_mm;
            raw += square;
            guided += row.weight * square;
        }
    }
    (raw, guided)
}

/// The guided energy incident on one piece.
pub fn incident_guided(state: &IcsState, piece: usize) -> f64 {
    incident_totals(state, piece).1
}

/// The **raw** energy incident on one piece: the `loss > 0` test that decides
/// whether a piece is in the sweep's colliding set at all.
pub fn incident_raw(state: &IcsState, piece: usize) -> f64 {
    incident_totals(state, piece).0
}

/// The negative gradient of the incident guided energy at one piece:
/// `(force_x, force_y, torque)`, with the torque `tau = (p - c) x (w v n)`
/// about the piece's transformed **centroid**.
///
/// **This is a probe of the field, not a move of the member.** Nothing in
/// `relocate.rs`, `descent.rs` or `disrupt.rs` calls it and no acceptance path
/// can reach it: `CutCloseRelocate` samples and coordinate-descends, and the
/// gradient proposal it replaced is deleted. What survives is the *claim* the
/// numeric-soundness corpus makes about Φ itself - that a step along Φ's own
/// negative gradient reduces a violation measured by a completely independent
/// scorer, on whole transformed rings, by ray-cast containment, in a linear
/// rather than a squared metric. That claim is a fact about the field and it
/// stays in the regression floor whatever the search does with it
/// (docs/cutclose-relocate-spec.md, "The gate": the soundness populations keep
/// their literal thresholds). `corpus::gradient_probe_step` is the only caller,
/// and it lives in the corpus for the same reason `homotopy::compressed` stays
/// a corpus factory rather than a live start.
pub fn incident_gradient(state: &IcsState, piece: usize) -> [f64; 3] {
    let count = state.poses.len();
    let centre = state.geometry.centroids[piece];
    let mut force = [0.0f64, 0.0];
    let mut torque = 0.0f64;
    for other in 0..count {
        if other == piece {
            continue;
        }
        let (first, second) = if other < piece {
            (other, piece)
        } else {
            (piece, other)
        };
        let row = &state.pair_rows[pair_index(count, first, second)];
        if row.violation_mm <= 0.0 {
            continue;
        }
        let contact = if piece == first {
            row.contact
        } else {
            row.contact.reversed()
        };
        let scale = 2.0 * row.weight * row.violation_mm;
        force[0] += scale * contact.normal[0];
        force[1] += scale * contact.normal[1];
        let arm = [
            contact.witness_a[0] - centre[0],
            contact.witness_a[1] - centre[1],
        ];
        torque += scale * (arm[0] * contact.normal[1] - arm[1] * contact.normal[0]);
    }
    for edge in 0..4 {
        let row = state.edge_rows[piece][edge];
        if row.violation_mm <= 0.0 {
            continue;
        }
        let normal = super::state::EDGE_NORMALS[edge];
        let scale = 2.0 * row.weight * row.violation_mm;
        force[0] += scale * normal[0];
        force[1] += scale * normal[1];
        let arm = [row.witness[0] - centre[0], row.witness[1] - centre[1]];
        torque += scale * (arm[0] * normal[1] - arm[1] * normal[0]);
    }
    [force[0], force[1], torque]
}

/// Algorithm 8's published multipliers, read off Sparrow `consts.rs`
/// (`GLS_WEIGHT_MIN_INC_RATIO`, `GLS_WEIGHT_MAX_INC_RATIO`, `GLS_WEIGHT_DECAY`,
/// rev `14f4868f`) and frozen before any wall number exists. They are the
/// paper's, not ours, and changing them is a forbidden rescue
/// (docs/cutclose-relocate-spec.md, "The gate").
pub const GLS_WEIGHT_MIN_INC_RATIO: f64 = 1.2;
pub const GLS_WEIGHT_MAX_INC_RATIO: f64 = 2.0;
pub const GLS_WEIGHT_DECAY: f64 = 0.95;

/// The weight cap, `2^20`. Grok review 12 Round 2 M24 takes it as a harmless
/// knob from Sol: it bounds a pathological row that stays active for tens of
/// thousands of sweeps, and it is far above anything a healthy schedule reaches
/// (`1.2^k > 2^20` needs `k > 79` consecutive active sweeps at the *minimum*
/// ratio).
pub const GLS_WEIGHT_CAP: f64 = 1_048_576.0;

/// **Algorithm 8, on our `v`: every pair row and every boundary row, every
/// sweep.**
///
/// ```text
/// v == 0 : w <- max(1, 0.95 w)
/// v >  0 : w <- min(2^20, w * (1.2 + 0.8 * v / v_max))
/// ```
///
/// Source of the schedule: arXiv:2509.13329 Algorithm 8, as read at Sparrow
/// `quantify/tracker.rs::update_weights` (rev `14f4868f`), which runs it on
/// every pair *and* every container-item entry of its tracker after every
/// separator iteration. Our differences, stated rather than hidden:
///
/// * their `loss` is a pole-based overlap-**area proxy** (Algorithms 3-4) in
///   `f32`; ours is the source-ring signed-gap violation `v` in `f64`, and
///   `guided = w v^2`. We do not port the proxy.
/// * their container leak is a bbox-area term; ours is the four boundary rows,
///   which are already part of Φ, so "outside the strip" is weighted by the
///   same schedule as a pair overlap rather than by a second rule.
///
/// What this **replaces** is the previous round's `guided_update`: one integer
/// increment on the single lexicographically-first maximum-utility row, and
/// only on a stalled sweep. Grok review 12 Round 1 §1.5 measured that as the
/// wrong dialect; Round 2 M4 and Sol review 17 Round 2 §1 both signed the
/// published one. There is now no second schedule to leak.
///
/// `v_max` is the largest violation over **all** rows, so the ratio is the
/// row's share of the worst pressure in the layout. With no active row the
/// whole pass is a decay, which is the correct behaviour at `Φ = 0`: nothing to
/// punish, and every weight relaxes one step toward the floor.
///
/// Returns how many rows were **active** (the `v > 0` branch), which is the
/// number a diagnostics line reports beside `weightUpdates`.
pub fn gls_update(state: &mut IcsState) -> u64 {
    let mut max_violation = 0.0f64;
    for row in &state.pair_rows {
        if row.violation_mm > max_violation {
            max_violation = row.violation_mm;
        }
    }
    for rows in &state.edge_rows {
        for row in rows {
            if row.violation_mm > max_violation {
                max_violation = row.violation_mm;
            }
        }
    }
    let span = GLS_WEIGHT_MAX_INC_RATIO - GLS_WEIGHT_MIN_INC_RATIO;
    let mut active = 0u64;
    // The closure is the whole schedule, written once so a pair row and a
    // boundary row cannot drift into two dialects the way `penalty` and a
    // hypothetical `weight` would have.
    let mut update = |violation: f64, weight: &mut f64| {
        if violation <= 0.0 {
            *weight = (*weight * GLS_WEIGHT_DECAY).max(GLS_WEIGHT_FLOOR);
            return;
        }
        active += 1;
        let share = if max_violation > 0.0 {
            violation / max_violation
        } else {
            0.0
        };
        let ratio = GLS_WEIGHT_MIN_INC_RATIO + span * share;
        *weight = (*weight * ratio).min(GLS_WEIGHT_CAP);
    };
    for row in &mut state.pair_rows {
        update(row.violation_mm, &mut row.weight);
    }
    for rows in &mut state.edge_rows {
        for row in rows {
            update(row.violation_mm, &mut row.weight);
        }
    }
    active
}

/// The piece carrying the most incident guided energy: the jump's target and
/// the sweep's first visit. Stable tie by input index.
pub fn highest_pressure_piece(state: &IcsState) -> usize {
    let mut best = 0usize;
    let mut best_energy = f64::NEG_INFINITY;
    for piece in 0..state.poses.len() {
        let energy = incident_guided(state, piece);
        if energy > best_energy {
            best_energy = energy;
            best = piece;
        }
    }
    best
}

/// Every piece in descending incident guided energy, ties by input index.
pub fn sweep_order(state: &IcsState, out: &mut Vec<usize>) {
    out.clear();
    let count = state.poses.len();
    let mut keyed: Vec<(f64, usize)> = (0..count)
        .map(|piece| (incident_guided(state, piece), piece))
        .collect();
    keyed.sort_by(|left, right| {
        right
            .0
            .partial_cmp(&left.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(left.1.cmp(&right.1))
    });
    out.extend(keyed.into_iter().map(|(_, piece)| piece));
}

/// Rebuilds every row for a state whose default `PairRow` slots have never
/// been filled, and returns the fold.
pub fn cold_totals(state: &mut IcsState, contract: &Contract, work: &mut WorkVector) -> Totals {
    rebuild_all(state, contract, work);
    fold(state)
}

/// Returns every guided weight to the floor.
///
/// **Called on a successful width change and, only under the experiment
/// feature, after a pool restore before disruption.** Grok review 12 Round 2
/// §6.4: weights persist across a rollback *inside* a width - Sparrow's
/// `tracker.rs::restore_but_keep_weights` is explicit about it, and a schedule
/// that forgot the pressure it had learned on every failed separation would be
/// a different algorithm - and reset when the landscape itself changes. Their
/// tracker is rebuilt by `change_strip_width`; their exploration-pool restore
/// also rebuilds it, which is the independently gated seam in
/// `docs/pool-retry-tracker-rebase-spec.md`. The schedule agent owns the call
/// sites; this module owns the meaning.
pub fn reset_weights(state: &mut IcsState) {
    for row in &mut state.pair_rows {
        row.weight = GLS_WEIGHT_FLOOR;
    }
    for rows in &mut state.edge_rows {
        for row in rows {
            row.weight = GLS_WEIGHT_FLOOR;
        }
    }
}

/// The pair row's slot, exposed for the tests and the corpus driver.
pub fn pair_row(state: &IcsState, first: usize, second: usize) -> PairRow {
    state.pair_rows[pair_index(state.poses.len(), first, second)]
}

/// A census of the rows that are still active: how many pairs and how many
/// boundaries carry a violation, and the worst of each.
///
/// Diagnostic. It exists because "Φ fell from 266 to 47" is not a readable
/// statement about a layout - twenty pairs at 1.5 mm and one pair at 6.8 mm are
/// the same number and completely different failures - and Gate 0 has to be
/// able to tell those apart in the evidence rather than in a debugger.
/// **The four boundary sides are counted apart.** Grok review 10 §B.4 and Sol
/// review 15 §B.5: with one `activeEdgeRows` number, the previous round's
/// README could claim "a single global translation would legalize it" and the
/// verification round could only answer "that is a reading, not a
/// measurement" - because two rows on *opposite* sides mean no such
/// translation exists, and nothing in the evidence said which sides they were
/// on. It says now.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RowCensus {
    pub active_pairs: usize,
    pub active_edges: usize,
    pub max_pair_violation_mm: f64,
    pub max_edge_violation_mm: f64,
    /// The largest guided weight on any row.
    ///
    /// **Keeps the field name `max_penalty` and changes its type to `f64`.**
    /// The evidence document's `maxGuidedPenalty` therefore keeps pointing at
    /// "the biggest number the guided schedule has put on a row", which is the
    /// only thing anybody read it for; what changed underneath is that the
    /// schedule is now Algorithm 8's continuous multiplier rather than an
    /// integer increment, so the value is `w` itself and no longer `1 + p`.
    pub max_penalty: f64,
    /// Active row counts, `[left, right, bottom, top]`.
    pub active_edges_by_side: [usize; 4],
    /// The worst violation on each side, `[left, right, bottom, top]`.
    pub max_edge_violation_by_side_mm: [f64; 4],
    /// Pieces carrying an active row on two **opposite** sides at once
    /// (left+right, or bottom+top). A nonzero count is the shape no single
    /// rigid translation can fix.
    pub pieces_squeezed_on_opposite_sides: usize,
}

pub fn census(state: &IcsState) -> RowCensus {
    let mut out = RowCensus {
        max_penalty: GLS_WEIGHT_FLOOR,
        ..RowCensus::default()
    };
    for row in &state.pair_rows {
        out.max_penalty = out.max_penalty.max(row.weight);
        if row.violation_mm > 0.0 {
            out.active_pairs += 1;
            out.max_pair_violation_mm = out.max_pair_violation_mm.max(row.violation_mm);
        }
    }
    for rows in &state.edge_rows {
        let mut active = [false; 4];
        for (edge, row) in rows.iter().enumerate() {
            out.max_penalty = out.max_penalty.max(row.weight);
            if row.violation_mm > 0.0 {
                active[edge] = true;
                out.active_edges += 1;
                out.active_edges_by_side[edge] += 1;
                out.max_edge_violation_mm = out.max_edge_violation_mm.max(row.violation_mm);
                out.max_edge_violation_by_side_mm[edge] =
                    out.max_edge_violation_by_side_mm[edge].max(row.violation_mm);
            }
        }
        if (active[EDGE_LEFT] && active[EDGE_RIGHT]) || (active[EDGE_BOTTOM] && active[EDGE_TOP]) {
            out.pieces_squeezed_on_opposite_sides += 1;
        }
    }
    out
}
