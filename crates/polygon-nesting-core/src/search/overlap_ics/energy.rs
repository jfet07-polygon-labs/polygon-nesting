//! Φ: the raw squared-hinge penalty, the guided integer weights, and the
//! fixed-order fold that makes the incremental cache honest.
//!
//! ```text
//! v_ij   = max over cell pairs of [c_pair - s(A_a,B_b)]_+
//! Phi_raw    = sum over pairs of v_ij^2   +  sum over piece-edges of v_ie^2
//! Phi_guided = sum over pairs of w_ij v_ij^2 + sum over piece-edges of w_ie v_ie^2
//! ```
//!
//! `w = 1 + p`, `p` an integer incremented by the guided update in
//! `descent.rs`. The two are stored separately and only the raw one is ever
//! compared against a stall; neither is ever reported as quality.
//!
//! **The fixed-order fold is not an optimization, it is the defence.** A move
//! recomputes only the moving piece's `n-1` rows - but the totals are then
//! re-folded over *all* cached row scalars in pair-ID order, which is 1,830
//! additions on mixed-61 and not 1,830 geometry queries. Sol R2 §2 asks for
//! exactly this, to stop this engine inheriting the incremental-tracker drift
//! defect the relaxed lane had. `incremental_rows_match_cold_rebuild` in the
//! module's tests is the enforcement.

use super::broad_phase::{boundary_residuals, pair_is_near};
use super::contact::{box_gap, convex_cell_gap, Contact};
use super::diagnostics::WorkVector;
use super::state::{
    pair_index, Contract, EdgeRow, Geometry, IcsState, PairRow, EDGE_BOTTOM, EDGE_LEFT, EDGE_RIGHT,
    EDGE_TOP,
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
pub fn measure_pair(
    geometry: &Geometry,
    first: usize,
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
        geometry.piece_bounds[first],
        geometry.piece_bounds[second],
        clearance_mm,
    ) {
        work.broad_phase_rejects += 1;
        return (0.0, empty);
    }
    let (first_start, first_end) = geometry.piece_cells[first];
    let (second_start, second_end) = geometry.piece_cells[second];
    let mut worst = 0.0f64;
    let mut worst_contact = empty;
    for a in first_start..first_end {
        for b in second_start..second_end {
            // The cell-level box proof, for the nonconvex pieces whose cells
            // are triangles: most triangle pairs of two adjacent decagons
            // cannot reach the clearance and never become a query.
            if box_gap(geometry.cell_bounds[a], geometry.cell_bounds[b]) >= clearance_mm {
                continue;
            }
            work.convex_cell_gap_queries += 1;
            let contact = convex_cell_gap(geometry.cell_slice(a), geometry.cell_slice(b));
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
        rows[edge].penalty = previous[edge].penalty;
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
    for first in 0..count {
        for second in (first + 1)..count {
            let index = pair_index(count, first, second);
            let (violation, contact) =
                measure_pair(&state.geometry, first, second, clearance, work);
            state.pair_rows[index].violation_mm = violation;
            state.pair_rows[index].contact = contact;
        }
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
    for other in 0..count {
        if other == piece {
            continue;
        }
        let (first, second) = if other < piece {
            (other, piece)
        } else {
            (piece, other)
        };
        let index = pair_index(count, first, second);
        let (violation, contact) = measure_pair(&state.geometry, first, second, clearance, work);
        state.pair_rows[index].violation_mm = violation;
        state.pair_rows[index].contact = contact;
    }
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
        totals.guided += (1 + row.penalty) as f64 * square;
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
            totals.guided += (1 + row.penalty) as f64 * square;
            if violation > totals.max_violation_mm {
                totals.max_violation_mm = violation;
            }
        }
    }
    totals
}

/// The guided energy incident on one piece: its `n-1` pair rows and its four
/// boundary rows, folded in the same fixed order.
///
/// This is what a backtracking step is accepted against - not the global total
/// - because a one-piece move can only change these rows and re-folding 1,830
/// scalars per ladder rung would be the ladder's cost rather than the
/// geometry's.
pub fn incident_guided(state: &IcsState, piece: usize) -> f64 {
    let count = state.poses.len();
    let mut total = 0.0;
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
        if row.violation_mm > 0.0 {
            total += (1 + row.penalty) as f64 * row.violation_mm * row.violation_mm;
        }
    }
    for row in &state.edge_rows[piece] {
        if row.violation_mm > 0.0 {
            total += (1 + row.penalty) as f64 * row.violation_mm * row.violation_mm;
        }
    }
    total
}

/// The negative gradient of the incident guided energy at one piece:
/// `(force_x, force_y, torque)`.
///
/// The torque of a contact is Sol review 14 §1's `tau = (p - c) x (w v n)`,
/// which is exactly `d/dtheta` of the same squared hinge - the witness moves
/// with the piece and the normal does not.
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
        let scale = 2.0 * (1 + row.penalty) as f64 * row.violation_mm;
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
        let scale = 2.0 * (1 + row.penalty) as f64 * row.violation_mm;
        force[0] += scale * normal[0];
        force[1] += scale * normal[1];
        let arm = [row.witness[0] - centre[0], row.witness[1] - centre[1]];
        torque += scale * (arm[0] * normal[1] - arm[1] * normal[0]);
    }
    [force[0], force[1], torque]
}

/// The lexicographically first maximum-utility row, `u = v / (1 + p)`, and the
/// increment of its integer penalty. This is the guided local search update:
/// it changes the landscape while allowing raw overlap to worsen temporarily.
pub fn guided_update(state: &mut IcsState) -> Option<(usize, usize)> {
    let mut best_utility = 0.0f64;
    let mut best: Option<(usize, usize)> = None;
    for (index, row) in state.pair_rows.iter().enumerate() {
        if row.violation_mm <= 0.0 {
            continue;
        }
        let utility = row.violation_mm / (1 + row.penalty) as f64;
        if utility > best_utility {
            best_utility = utility;
            best = Some((0, index));
        }
    }
    for (piece, rows) in state.edge_rows.iter().enumerate() {
        for (edge, row) in rows.iter().enumerate() {
            if row.violation_mm <= 0.0 {
                continue;
            }
            let utility = row.violation_mm / (1 + row.penalty) as f64;
            if utility > best_utility {
                best_utility = utility;
                best = Some((1, piece * 4 + edge));
            }
        }
    }
    match best {
        Some((0, index)) => {
            state.pair_rows[index].penalty = state.pair_rows[index].penalty.saturating_add(1);
            Some((0, index))
        }
        Some((1, slot)) => {
            let row = &mut state.edge_rows[slot / 4][slot % 4];
            row.penalty = row.penalty.saturating_add(1);
            Some((1, slot))
        }
        _ => None,
    }
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

/// Zeroes every guided penalty. Used when a new strip target is locked so the
/// weights of a previous strip cannot leak into a new landscape.
pub fn clear_penalties(state: &mut IcsState) {
    for row in &mut state.pair_rows {
        row.penalty = 0;
    }
    for rows in &mut state.edge_rows {
        for row in rows {
            row.penalty = 0;
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
    pub max_penalty: u32,
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
    let mut out = RowCensus::default();
    for row in &state.pair_rows {
        out.max_penalty = out.max_penalty.max(row.penalty);
        if row.violation_mm > 0.0 {
            out.active_pairs += 1;
            out.max_pair_violation_mm = out.max_pair_violation_mm.max(row.violation_mm);
        }
    }
    for rows in &state.edge_rows {
        let mut active = [false; 4];
        for (edge, row) in rows.iter().enumerate() {
            out.max_penalty = out.max_penalty.max(row.penalty);
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
