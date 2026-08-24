//! Minimum-conflict binary close: one cold binary energy and one deterministic
//! source/sink cut at explore-bite injection.
//!
//! This module is compiled only by the experiment's feature. It owns no live
//! engine state and charges none of its measurement work to the legacy search
//! currency. A decision either proves every literal in the frozen term table,
//! graph cut and cold selected state, or is visibly invalid; there is no
//! centre fallback here.

use std::collections::VecDeque;

use sha2::{Digest, Sha256};

use crate::domain::IrregularPoint;
use crate::geometry::general_polygon::PolygonSet;

use super::diagnostics::WorkVector;
use super::energy;
use super::homotopy;
use super::relocate::transformed_centroid;
use super::state::{
    build_geometry, pair_count, pair_index, Contract, EdgeRow, IcsState, PairRow, PieceSource, Pose,
};

const TERM_DOMAIN: &[u8] = b"minimum-conflict-binary-close/terms/v1";
const GRAPH_DOMAIN: &[u8] = b"minimum-conflict-binary-close/graph/v1";
const LABEL_DOMAIN: &[u8] = b"minimum-conflict-binary-close/labels/v1";
const RESIDUAL_DOMAIN: &[u8] = b"minimum-conflict-binary-close/residual-reachability/v1";
const POSE_DOMAIN: &[u8] = b"minimum-conflict-binary-close/poses/v1";
const ROW_DOMAIN: &[u8] = b"minimum-conflict-binary-close/rows/v1";

/// Runtime arm of the feature. `Centre` remains the literal historical call.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BinaryCloseArm {
    #[default]
    Centre,
    MinCut,
    ComputeIgnore,
}

impl BinaryCloseArm {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Centre => "centre",
            Self::MinCut => "mincut",
            Self::ComputeIgnore => "compute-ignore",
        }
    }
}

/// Bit-exact cached pair row. Keeping the complete cold row beside the scalar
/// term makes installation verification cover the contact field and reset GLS
/// weight as well as the violation used by the binary energy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PairRowBits {
    pub violation_mm: u64,
    pub weight: u64,
    pub signed_gap_mm: u64,
    pub normal: [u64; 2],
    pub witness_a: [u64; 2],
    pub witness_b: [u64; 2],
}

/// Bit-exact cached boundary row, including its reset GLS weight and witness.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EdgeRowBits {
    pub violation_mm: u64,
    pub weight: u64,
    pub witness: [u64; 2],
}

/// Four cold pair rows and their squared raw costs. The first array coordinate
/// is piece `first`'s state and the second is piece `second`'s state.
#[derive(Clone, Debug, PartialEq)]
pub struct PairTerm {
    pub pair_id: usize,
    pub first: usize,
    pub second: usize,
    pub violations_mm: [[f64; 2]; 2],
    pub costs: [[f64; 2]; 2],
    pub row_bits: [[PairRowBits; 2]; 2],
    pub finite_nonnegative: bool,
    pub zero_diagonal: bool,
    pub submodular: bool,
}

/// Both states of one piece's four boundary rows, in `L,R,B,T` order.
#[derive(Clone, Debug, PartialEq)]
pub struct UnaryTerm {
    pub piece: usize,
    pub violations_mm: [[f64; 4]; 2],
    pub row_costs: [[f64; 4]; 2],
    pub row_bits: [[EdgeRowBits; 4]; 2],
    pub sums: [f64; 2],
    pub finite_nonnegative: bool,
}

/// One canonical graph edge, including zero-capacity edges which the residual
/// solver is allowed not to store.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GraphEdge {
    pub from: usize,
    pub to: usize,
    pub capacity: f64,
}

/// Complete evidence for one attempted treatment or compute-ignore bite.
#[derive(Clone, Debug, PartialEq)]
pub struct BinaryCloseDecision {
    pub request_seed: u64,
    pub explore_bite_ordinal: u64,
    pub depth_before_mm: f64,
    pub target_depth_mm: f64,
    pub delta_mm: f64,
    pub pose_state_bits_valid: bool,
    pub parent_proxy_pair_legal: bool,
    pub parent_pose_digest_sha256: [u8; 32],
    pub pair_terms: Vec<PairTerm>,
    pub unary_terms: Vec<UnaryTerm>,
    pub all_finite_nonnegative: bool,
    pub all_zero_diagonal: bool,
    pub all_submodular: bool,
    pub graph_edges: Vec<GraphEdge>,
    pub residual_source_reachable: Vec<bool>,
    pub labels: Vec<bool>,
    pub centre_labels: Vec<bool>,
    pub hamming_disagreement: usize,
    pub moved_pieces: usize,
    pub centre_moved_pieces: usize,
    pub term_table_digest_sha256: [u8; 32],
    pub graph_digest_sha256: [u8; 32],
    pub residual_digest_sha256: [u8; 32],
    pub label_digest_sha256: [u8; 32],
    pub installed_pose_digest_sha256: [u8; 32],
    pub installed_row_digest_sha256: [u8; 32],
    pub selected_cut_capacity: f64,
    pub selected_table_energy: f64,
    pub cold_raw_phi: f64,
    pub cut_table_bits_equal: bool,
    pub table_cold_bits_equal: bool,
    pub installed_rows_match_table: bool,
    pub selected_totals_finite_nonnegative: bool,
    pub field_work: WorkVector,
    pub valid: bool,
    pub invalid_reason: Option<String>,
}

impl BinaryCloseDecision {
    /// Builds and validates the complete binary decision without touching the
    /// supplied parent or any trajectory counter.
    pub fn build(
        sources: &[PieceSource],
        contract: &Contract,
        parent: &IcsState,
        request_seed: u64,
        explore_bite_ordinal: u64,
        depth_before_mm: f64,
    ) -> Self {
        let target_depth_mm = homotopy::explore_width_mm(depth_before_mm);
        let delta_mm = target_depth_mm - depth_before_mm;
        let count = parent.poses.len();
        let mut state_one_poses = parent.poses.clone();
        for pose in &mut state_one_poses {
            pose.ty_mm += delta_mm;
        }
        let pose_state_bits_valid = parent
            .poses
            .iter()
            .zip(&state_one_poses)
            .all(|(zero, one)| {
                zero.tx_mm.to_bits() == one.tx_mm.to_bits()
                    && zero.theta_deg.to_bits() == one.theta_deg.to_bits()
                    && zero.mirrored == one.mirrored
                    && one.ty_mm.to_bits() == (zero.ty_mm + delta_mm).to_bits()
            });
        let geometry_zero = build_geometry(sources, &parent.poses);
        let geometry_one = build_geometry(sources, &state_one_poses);
        let mut field_work = WorkVector {
            pose_transforms: (2 * count) as u64,
            ..WorkVector::default()
        };
        let clearance = contract.pair_clearance_mm();
        let mut pair_terms = Vec::with_capacity(pair_count(count));
        for first in 0..count {
            for second in (first + 1)..count {
                let mut violations_mm = [[0.0; 2]; 2];
                let mut row_bits = [[PairRowBits::default(); 2]; 2];
                for first_state in 0..2 {
                    for second_state in 0..2 {
                        let first_geometry = if first_state == 0 {
                            &geometry_zero
                        } else {
                            &geometry_one
                        };
                        let second_geometry = if second_state == 0 {
                            &geometry_zero
                        } else {
                            &geometry_one
                        };
                        let (violation_mm, contact) = energy::measure_pair_cross(
                            first_geometry,
                            first,
                            second_geometry,
                            second,
                            clearance,
                            &mut field_work,
                        );
                        violations_mm[first_state][second_state] = violation_mm;
                        row_bits[first_state][second_state] = pair_row_bits(&PairRow {
                            violation_mm,
                            contact,
                            ..PairRow::default()
                        });
                    }
                }
                let costs = square_pair_rows(violations_mm);
                let (finite_nonnegative, zero_diagonal, submodular) =
                    validate_pair_costs(violations_mm, costs);
                pair_terms.push(PairTerm {
                    pair_id: pair_index(count, first, second),
                    first,
                    second,
                    violations_mm,
                    costs,
                    row_bits,
                    finite_nonnegative,
                    zero_diagonal,
                    submodular,
                });
            }
        }

        let mut unary_terms = Vec::with_capacity(count);
        for piece in 0..count {
            let rows_zero = energy::measure_edges(
                &geometry_zero,
                piece,
                contract,
                target_depth_mm,
                [EdgeRow::default(); 4],
            );
            let rows_one = energy::measure_edges(
                &geometry_one,
                piece,
                contract,
                target_depth_mm,
                [EdgeRow::default(); 4],
            );
            let violations_mm = [
                rows_zero.map(|row| row.violation_mm),
                rows_one.map(|row| row.violation_mm),
            ];
            let row_bits = [
                rows_zero.map(|row| edge_row_bits(&row)),
                rows_one.map(|row| edge_row_bits(&row)),
            ];
            let mut row_costs = [[0.0; 4]; 2];
            let mut sums = [0.0; 2];
            let mut finite_nonnegative = true;
            for state in 0..2 {
                for edge in 0..4 {
                    let violation = violations_mm[state][edge];
                    let cost = violation * violation;
                    row_costs[state][edge] = cost;
                    sums[state] += cost;
                    finite_nonnegative &= finite_nonnegative_value(violation)
                        && finite_nonnegative_value(cost)
                        && finite_nonnegative_value(sums[state]);
                }
            }
            unary_terms.push(UnaryTerm {
                piece,
                violations_mm,
                row_costs,
                row_bits,
                sums,
                finite_nonnegative,
            });
        }

        let all_finite_nonnegative = pose_state_bits_valid
            && parent.poses.iter().all(pose_is_finite)
            && state_one_poses.iter().all(pose_is_finite)
            && valid_depth_transition(depth_before_mm, target_depth_mm, delta_mm)
            && pair_terms.iter().all(|term| term.finite_nonnegative)
            && unary_terms.iter().all(|term| term.finite_nonnegative)
            && graph_capacity_sum_is_finite(&pair_terms, &unary_terms);
        let all_zero_diagonal = pair_terms.iter().all(|term| term.zero_diagonal);
        let all_submodular = pair_terms.iter().all(|term| term.submodular);
        let parent_proxy_pair_legal = parent.pair_rows.iter().all(|row| row.violation_mm == 0.0);
        let parent_pose_digest_sha256 = pose_digest(&parent.poses);
        let term_table_digest_sha256 = term_digest(&pair_terms, &unary_terms);
        let centre_split = homotopy::centre_cut_mm(depth_before_mm);
        let centre_labels = sources
            .iter()
            .zip(&parent.poses)
            .map(|(source, pose)| transformed_centroid(source, *pose)[1] > centre_split)
            .collect::<Vec<_>>();
        let centre_moved_pieces = centre_labels.iter().filter(|label| **label).count();

        let source_node = count;
        let sink_node = count + 1;
        let pregraph_valid = all_finite_nonnegative
            && all_zero_diagonal
            && all_submodular
            && parent_proxy_pair_legal;
        // §2 is literal: no graph exists until every operand and pair sum has
        // passed the frozen domain checks.
        let mut graph_edges = Vec::new();
        if pregraph_valid {
            for unary in &unary_terms {
                graph_edges.push(GraphEdge {
                    from: source_node,
                    to: unary.piece,
                    capacity: unary.sums[1],
                });
                graph_edges.push(GraphEdge {
                    from: unary.piece,
                    to: sink_node,
                    capacity: unary.sums[0],
                });
            }
            for pair in &pair_terms {
                graph_edges.push(GraphEdge {
                    from: pair.first,
                    to: pair.second,
                    capacity: pair.costs[0][1],
                });
                graph_edges.push(GraphEdge {
                    from: pair.second,
                    to: pair.first,
                    capacity: pair.costs[1][0],
                });
            }
        }
        let graph_digest_sha256 = graph_digest(count + 2, &graph_edges);
        let residual_source_reachable = if pregraph_valid {
            solve_cut(count + 2, source_node, sink_node, &graph_edges)
                .map(|reachable| reachable[..count].to_vec())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        // The frozen convention is 0 on the residual source side and 1 on the
        // sink side. Reachability itself is therefore the complement of the
        // Boolean state we install.
        let labels = residual_source_reachable
            .iter()
            .map(|side| !*side)
            .collect::<Vec<_>>();
        let residual_digest_sha256 =
            bool_vector_digest(RESIDUAL_DOMAIN, &residual_source_reachable);
        let label_digest_sha256 = label_digest(&labels);
        let hamming_disagreement = labels
            .iter()
            .zip(&centre_labels)
            .filter(|(left, right)| left != right)
            .count();
        let moved_pieces = labels.iter().filter(|label| **label).count();

        let mut installed_pose_digest_sha256 = [0u8; 32];
        let mut installed_row_digest_sha256 = [0u8; 32];
        let mut selected_cut_capacity = f64::NAN;
        let mut selected_table_energy = f64::NAN;
        let mut cold_raw_phi = f64::NAN;
        let mut cut_table_bits_equal = false;
        let mut table_cold_bits_equal = false;
        let mut installed_rows_match_table = false;
        let mut selected_totals_finite_nonnegative = false;
        if labels.len() == count {
            let selected_cut = checked_cut_energy_for_labels(&pair_terms, &unary_terms, &labels);
            let selected_table =
                checked_table_energy_for_labels(&pair_terms, &unary_terms, &labels);
            if let Some(value) = selected_cut {
                selected_cut_capacity = value;
            }
            if let Some(value) = selected_table {
                selected_table_energy = value;
            }
            let selected_poses = apply_labels(&parent.poses, &labels, delta_mm);
            installed_pose_digest_sha256 = pose_digest(&selected_poses);
            let mut selected_state = IcsState {
                geometry: build_geometry(sources, &selected_poses),
                poses: selected_poses,
                pair_rows: vec![PairRow::default(); pair_count(count)],
                edge_rows: vec![[EdgeRow::default(); 4]; count],
                target_depth_mm,
            };
            field_work.pose_transforms += count as u64;
            energy::rebuild_all(&mut selected_state, contract, &mut field_work);
            cold_raw_phi = energy::fold(&selected_state).raw;
            installed_row_digest_sha256 = row_digest(&selected_state);
            installed_rows_match_table =
                selected_rows_match(&selected_state, &pair_terms, &unary_terms, &labels);
            selected_totals_finite_nonnegative = selected_cut.is_some()
                && selected_table.is_some()
                && finite_nonnegative_value(cold_raw_phi);
            cut_table_bits_equal = selected_totals_finite_nonnegative
                && selected_cut_capacity.to_bits() == selected_table_energy.to_bits();
            table_cold_bits_equal = selected_totals_finite_nonnegative
                && selected_table_energy.to_bits() == cold_raw_phi.to_bits();
        }

        let valid = pregraph_valid
            && labels.len() == count
            && selected_totals_finite_nonnegative
            && cut_table_bits_equal
            && table_cold_bits_equal
            && installed_rows_match_table;
        let invalid_reason = (!valid).then(|| {
            if !pose_state_bits_valid {
                "pose state bits changed outside ty".to_owned()
            } else if !parent_proxy_pair_legal {
                "parent proxy pair rows are not all legal".to_owned()
            } else if !all_finite_nonnegative {
                "a term is non-finite or negative".to_owned()
            } else if !all_zero_diagonal {
                "a pair has a nonzero diagonal cost".to_owned()
            } else if !all_submodular {
                "a pair is nonsubmodular".to_owned()
            } else if labels.len() != count {
                "the deterministic max-flow did not return a complete labeling".to_owned()
            } else if !selected_totals_finite_nonnegative {
                "a selected canonical energy or cold raw Phi is non-finite or negative".to_owned()
            } else if !cut_table_bits_equal {
                "selected cut and table energy bits differ".to_owned()
            } else if !table_cold_bits_equal {
                "table energy and cold raw Phi bits differ".to_owned()
            } else {
                "installed row bits differ from the selected table".to_owned()
            }
        });

        Self {
            request_seed,
            explore_bite_ordinal,
            depth_before_mm,
            target_depth_mm,
            delta_mm,
            pose_state_bits_valid,
            parent_proxy_pair_legal,
            parent_pose_digest_sha256,
            pair_terms,
            unary_terms,
            all_finite_nonnegative,
            all_zero_diagonal,
            all_submodular,
            graph_edges,
            residual_source_reachable,
            labels,
            centre_labels,
            hamming_disagreement,
            moved_pieces,
            centre_moved_pieces,
            term_table_digest_sha256,
            graph_digest_sha256,
            residual_digest_sha256,
            label_digest_sha256,
            installed_pose_digest_sha256,
            installed_row_digest_sha256,
            selected_cut_capacity,
            selected_table_energy,
            cold_raw_phi,
            cut_table_bits_equal,
            table_cold_bits_equal,
            installed_rows_match_table,
            selected_totals_finite_nonnegative,
            field_work,
            valid,
            invalid_reason,
        }
    }

    /// Applies the proven labels to the live pose array. Callers must check
    /// `valid`; an invalid decision has no installable labeling.
    pub fn install(&self, poses: &mut [Pose]) {
        assert!(
            self.valid,
            "an invalid binary-close decision cannot install"
        );
        assert_eq!(poses.len(), self.labels.len());
        for (pose, moved) in poses.iter_mut().zip(&self.labels) {
            if *moved {
                pose.ty_mm += self.delta_mm;
            }
        }
    }

    /// Confirms that the live engine's ordinary cold refresh installed exactly
    /// the state already proved inside this decision. This is checked before
    /// separation begins; a mismatch invalidates and aborts the bite.
    pub fn verify_live_install(&mut self, state: &IcsState) -> bool {
        let matches = state.target_depth_mm.to_bits() == self.target_depth_mm.to_bits()
            && pose_digest(&state.poses) == self.installed_pose_digest_sha256
            && row_digest(state) == self.installed_row_digest_sha256
            && selected_rows_match(state, &self.pair_terms, &self.unary_terms, &self.labels)
            && energy::fold(state).raw.to_bits() == self.cold_raw_phi.to_bits();
        if !matches {
            self.valid = false;
            self.invalid_reason =
                Some("the live cold refresh differs from the proved selected state".to_owned());
        }
        matches
    }
}

/// The per-call trace. A second `run_cutclose` on the same engine gets a fresh
/// trace while the engine's global explore ordinal continues.
#[derive(Clone, Debug, PartialEq)]
pub struct BinaryCloseTrace {
    pub arm: BinaryCloseArm,
    pub decisions: Vec<BinaryCloseDecision>,
    pub invalid_decisions: u64,
}

impl BinaryCloseTrace {
    pub fn new(arm: BinaryCloseArm) -> Self {
        Self {
            arm,
            decisions: Vec::new(),
            invalid_decisions: 0,
        }
    }

    pub fn push(&mut self, decision: BinaryCloseDecision) {
        self.invalid_decisions += u64::from(!decision.valid);
        self.decisions.push(decision);
    }
}

impl Default for BinaryCloseTrace {
    fn default() -> Self {
        Self::new(BinaryCloseArm::Centre)
    }
}

/// Printed synthetic arithmetic/graph vectors for Gate 0. The real-geometry
/// half is a complete [`BinaryCloseDecision`] emitted beside this report by
/// the benchmark driver.
#[derive(Clone, Debug, PartialEq)]
pub struct Gate0VectorReport {
    pub expected_labels: Vec<bool>,
    pub solver_labels: Vec<bool>,
    pub exhaustive_energies: Vec<f64>,
    pub unique_nontrivial_minimum: bool,
    pub every_label_cut_energy_identity: bool,
    pub accepts_zero_diagonal_submodular: bool,
    pub rejects_nonfinite: bool,
    pub rejects_negative: bool,
    pub rejects_nonzero_diagonal: bool,
    pub rejects_nonsubmodular: bool,
    pub rejects_aggregate_overflow: bool,
    pub rejects_nonnegative_delta: bool,
    pub all_zero_labels: Vec<bool>,
    pub all_one_labels: Vec<bool>,
    pub tie_labels_first: Vec<bool>,
    pub tie_labels_second: Vec<bool>,
    pub graph_digest_sha256: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PoseStateBits {
    pub piece: usize,
    pub zero: [u64; 3],
    pub one: [u64; 3],
    pub mirrored: bool,
}

/// The deterministic real-geometry vector paired with
/// [`Gate0VectorReport`]. It exercises common translation, cold table rebuild,
/// and an independent incremental rebuild of the selected state.
#[derive(Clone, Debug, PartialEq)]
pub struct GeometryGate0VectorReport {
    pub pose_states: Vec<PoseStateBits>,
    pub decision: BinaryCloseDecision,
    pub incremental_pose_digest_sha256: [u8; 32],
    pub incremental_row_digest_sha256: [u8; 32],
    pub incremental_raw_phi: f64,
    pub incremental_matches_cold: bool,
}

/// Produces the fixed hand-computable four-node asymmetric vector used by both
/// the printed Gate-0 record and the unit tests.
pub fn gate0_vector_report() -> Gate0VectorReport {
    let unaries = [[0.0, 8.0], [7.0, 0.0], [0.0, 6.0], [5.0, 0.0]];
    let directed = [
        (0, 1, 1.0),
        (1, 0, 9.0),
        (1, 2, 2.0),
        (2, 1, 7.0),
        (2, 3, 1.0),
        (3, 2, 8.0),
    ];
    let edges = vector_graph(&unaries, &directed);
    let reachable = solve_cut(6, 4, 5, &edges).expect("the frozen vector is valid");
    let solver_labels = reachable[..4].iter().map(|side| !*side).collect::<Vec<_>>();
    let exhaustive_energies = (0..16)
        .map(|mask| {
            let labels = (0..4)
                .map(|piece| mask & (1 << piece) != 0)
                .collect::<Vec<_>>();
            vector_energy(&labels, &unaries, &directed)
        })
        .collect::<Vec<_>>();
    let mut ranked = exhaustive_energies
        .iter()
        .copied()
        .enumerate()
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| left.1.total_cmp(&right.1));
    let expected_labels = [false, true, true, true].to_vec();
    let expected_mask = expected_labels
        .iter()
        .enumerate()
        .fold(0usize, |mask, (piece, label)| {
            mask | (usize::from(*label) << piece)
        });
    let unique_nontrivial_minimum = ranked[0].0 == expected_mask
        && ranked[0].1 < ranked[1].1
        && expected_labels.iter().any(|label| *label)
        && expected_labels.iter().any(|label| !*label)
        && solver_labels == expected_labels;
    let every_label_cut_energy_identity = (0..16).all(|mask| {
        let labels = (0..4)
            .map(|piece| mask & (1 << piece) != 0)
            .collect::<Vec<_>>();
        vector_cut_capacity(&labels, &unaries, &directed).to_bits()
            == vector_energy(&labels, &unaries, &directed).to_bits()
    });
    let all_zero_edges = vector_graph(&[[0.0, 3.0], [0.0, 4.0]], &[]);
    let all_one_edges = vector_graph(&[[3.0, 0.0], [4.0, 0.0]], &[]);
    let tie_edges = vector_graph(&[[0.0, 0.0], [0.0, 0.0]], &[]);
    let labels_of =
        |reachable: Vec<bool>| reachable[..2].iter().map(|side| !*side).collect::<Vec<_>>();
    let all_zero_labels = labels_of(solve_cut(4, 2, 3, &all_zero_edges).unwrap());
    let all_one_labels = labels_of(solve_cut(4, 2, 3, &all_one_edges).unwrap());
    let tie_labels_first = labels_of(solve_cut(4, 2, 3, &tie_edges).unwrap());
    let tie_labels_second = labels_of(solve_cut(4, 2, 3, &tie_edges).unwrap());
    let accepted = validate_cost_vector([[0.0, 2.0], [3.0, 0.0]]);

    Gate0VectorReport {
        expected_labels,
        solver_labels,
        exhaustive_energies,
        unique_nontrivial_minimum,
        every_label_cut_energy_identity,
        accepts_zero_diagonal_submodular: accepted == (true, true, true),
        rejects_nonfinite: !validate_cost_vector([[0.0, f64::NAN], [1.0, 0.0]]).0,
        rejects_negative: !validate_cost_vector([[0.0, -1.0], [1.0, 0.0]]).0,
        rejects_nonzero_diagonal: !validate_cost_vector([[0.25, 2.0], [3.0, 0.0]]).1,
        rejects_nonsubmodular: !validate_cost_vector([[2.0, 1.0], [1.0, 2.0]]).2,
        rejects_aggregate_overflow: !validate_cost_vector([[0.0, f64::MAX], [f64::MAX, 0.0]]).0,
        rejects_nonnegative_delta: !valid_depth_transition(1.0, 1.0, 0.0)
            && !valid_depth_transition(1.0, 1.5, 0.5),
        all_zero_labels,
        all_one_labels,
        tie_labels_first,
        tie_labels_second,
        graph_digest_sha256: graph_digest(6, &edges),
    }
}

pub fn geometry_gate0_vector_report() -> GeometryGate0VectorReport {
    let square = PolygonSet::from_outer(vec![
        IrregularPoint::new(0.0, 0.0),
        IrregularPoint::new(10.0, 0.0),
        IrregularPoint::new(10.0, 10.0),
        IrregularPoint::new(0.0, 10.0),
    ])
    .expect("the frozen square is valid");
    let sources = (0..5)
        .map(|piece| PieceSource::of(&format!("p{piece}"), &square).expect("source"))
        .collect::<Vec<_>>();
    let poses = vec![
        Pose {
            tx_mm: 20.0,
            ty_mm: 1.0,
            theta_deg: 0.0,
            mirrored: false,
        },
        Pose {
            tx_mm: 20.0,
            ty_mm: 13.011,
            theta_deg: 0.0,
            mirrored: false,
        },
        Pose {
            tx_mm: 60.0,
            ty_mm: 1.0,
            theta_deg: 0.0,
            mirrored: false,
        },
        Pose {
            tx_mm: 72.0,
            ty_mm: 1.0,
            theta_deg: 0.0,
            mirrored: false,
        },
        Pose {
            tx_mm: 66.0,
            ty_mm: 13.0,
            theta_deg: 0.0,
            mirrored: false,
        },
    ];
    let contract = Contract {
        sheet_short_axis_mm: 100.0,
        sheet_long_axis_mm: 200.0,
        total_padding_mm: 2.0,
        sheet_edge_clearance_mm: 1.0,
        flattening_sag_tolerance_mm: 0.0,
        clearance_safety_margin_mm: 0.0,
    };
    let count = poses.len();
    let mut parent = IcsState {
        geometry: build_geometry(&sources, &poses),
        poses,
        pair_rows: vec![PairRow::default(); pair_count(count)],
        edge_rows: vec![[EdgeRow::default(); 4]; count],
        target_depth_mm: 24.011,
    };
    let mut work = WorkVector::default();
    energy::rebuild_all(&mut parent, &contract, &mut work);
    let decision = BinaryCloseDecision::build(&sources, &contract, &parent, 17, 22, 24.011);
    let pose_states = parent
        .poses
        .iter()
        .enumerate()
        .map(|(piece, pose)| PoseStateBits {
            piece,
            zero: [
                pose.tx_mm.to_bits(),
                pose.ty_mm.to_bits(),
                pose.theta_deg.to_bits(),
            ],
            one: [
                pose.tx_mm.to_bits(),
                (pose.ty_mm + decision.delta_mm).to_bits(),
                pose.theta_deg.to_bits(),
            ],
            mirrored: pose.mirrored,
        })
        .collect::<Vec<_>>();

    let mut incremental = parent;
    if decision.valid {
        decision.install(&mut incremental.poses);
    }
    incremental.target_depth_mm = decision.target_depth_mm;
    let mut incremental_work = WorkVector::default();
    for piece in 0..count {
        super::state::transform_piece(
            &sources,
            &mut incremental.geometry,
            &incremental.poses,
            piece,
        );
        energy::rebuild_piece_rows(&mut incremental, &contract, piece, &mut incremental_work);
    }
    let incremental_pose_digest_sha256 = pose_digest(&incremental.poses);
    let incremental_row_digest_sha256 = row_digest(&incremental);
    let incremental_raw_phi = energy::fold(&incremental).raw;
    let incremental_matches_cold = decision.valid
        && incremental_pose_digest_sha256 == decision.installed_pose_digest_sha256
        && incremental_row_digest_sha256 == decision.installed_row_digest_sha256
        && incremental_raw_phi.to_bits() == decision.cold_raw_phi.to_bits()
        && selected_rows_match(
            &incremental,
            &decision.pair_terms,
            &decision.unary_terms,
            &decision.labels,
        );

    GeometryGate0VectorReport {
        pose_states,
        decision,
        incremental_pose_digest_sha256,
        incremental_row_digest_sha256,
        incremental_raw_phi,
        incremental_matches_cold,
    }
}

fn square_pair_rows(violations: [[f64; 2]; 2]) -> [[f64; 2]; 2] {
    let mut costs = [[0.0; 2]; 2];
    for first in 0..2 {
        for second in 0..2 {
            costs[first][second] = violations[first][second] * violations[first][second];
        }
    }
    costs
}

fn validate_pair_costs(violations_mm: [[f64; 2]; 2], costs: [[f64; 2]; 2]) -> (bool, bool, bool) {
    let diagonal_sum = costs[0][0] + costs[1][1];
    let crossing_sum = costs[0][1] + costs[1][0];
    let finite_nonnegative = violations_mm
        .iter()
        .flatten()
        .chain(costs.iter().flatten())
        .copied()
        .all(finite_nonnegative_value)
        && finite_nonnegative_value(diagonal_sum)
        && finite_nonnegative_value(crossing_sum);
    let zero_diagonal = costs[0][0] == 0.0 && costs[1][1] == 0.0;
    let submodular = finite_nonnegative && diagonal_sum <= crossing_sum;
    (finite_nonnegative, zero_diagonal, submodular)
}

fn validate_cost_vector(costs: [[f64; 2]; 2]) -> (bool, bool, bool) {
    validate_pair_costs(costs.map(|row| row.map(|cost| cost.sqrt())), costs)
}

fn pose_is_finite(pose: &Pose) -> bool {
    pose.tx_mm.is_finite() && pose.ty_mm.is_finite() && pose.theta_deg.is_finite()
}

fn pair_row_bits(row: &PairRow) -> PairRowBits {
    PairRowBits {
        violation_mm: row.violation_mm.to_bits(),
        weight: row.weight.to_bits(),
        signed_gap_mm: row.contact.signed_gap_mm.to_bits(),
        normal: row.contact.normal.map(f64::to_bits),
        witness_a: row.contact.witness_a.map(f64::to_bits),
        witness_b: row.contact.witness_b.map(f64::to_bits),
    }
}

fn edge_row_bits(row: &EdgeRow) -> EdgeRowBits {
    EdgeRowBits {
        violation_mm: row.violation_mm.to_bits(),
        weight: row.weight.to_bits(),
        witness: row.witness.map(f64::to_bits),
    }
}

fn checked_add(total: &mut f64, value: f64) -> bool {
    if !finite_nonnegative_value(value) {
        return false;
    }
    *total += value;
    finite_nonnegative_value(*total)
}

fn graph_capacity_sum_is_finite(pairs: &[PairTerm], unaries: &[UnaryTerm]) -> bool {
    let mut total = 0.0;
    unaries.iter().all(|unary| {
        checked_add(&mut total, unary.sums[1])
            && checked_add(&mut total, unary.sums[0])
    }) && pairs.iter().all(|pair| {
        checked_add(&mut total, pair.costs[0][1])
            && checked_add(&mut total, pair.costs[1][0])
    })
}

fn finite_nonnegative_value(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}

fn valid_depth_transition(depth_before_mm: f64, target_depth_mm: f64, delta_mm: f64) -> bool {
    finite_nonnegative_value(depth_before_mm)
        && finite_nonnegative_value(target_depth_mm)
        && delta_mm.is_finite()
        && delta_mm < 0.0
        && target_depth_mm < depth_before_mm
        && (target_depth_mm - depth_before_mm).to_bits() == delta_mm.to_bits()
}

fn apply_labels(parent: &[Pose], labels: &[bool], delta_mm: f64) -> Vec<Pose> {
    parent
        .iter()
        .zip(labels)
        .map(|(pose, moved)| {
            let mut selected = *pose;
            if *moved {
                selected.ty_mm += delta_mm;
            }
            selected
        })
        .collect()
}

/// Folds in the authority's canonical order: all pair rows by pair ID, then
/// every piece's four boundary rows. It is deliberately expanded past the
/// unary sums so its additions are bit-identical to `energy::fold`.
fn checked_table_energy_for_labels(
    pairs: &[PairTerm],
    unaries: &[UnaryTerm],
    labels: &[bool],
) -> Option<f64> {
    let mut total = 0.0;
    for pair in pairs {
        if !checked_add(
            &mut total,
            pair.costs[usize::from(labels[pair.first])][usize::from(labels[pair.second])],
        ) {
            return None;
        }
    }
    for unary in unaries {
        let state = usize::from(labels[unary.piece]);
        for edge in 0..4 {
            if !checked_add(&mut total, unary.row_costs[state][edge]) {
                return None;
            }
        }
    }
    Some(total)
}

/// Recomputes the selected graph cut independently of table indexing. Pair
/// capacities are chosen by the directed crossing, then terminal capacities
/// are expanded back into their four authoritative boundary rows so the fold
/// order stays bit-identical to raw Phi.
fn checked_cut_energy_for_labels(
    pairs: &[PairTerm],
    unaries: &[UnaryTerm],
    labels: &[bool],
) -> Option<f64> {
    let mut total = 0.0;
    for pair in pairs {
        let capacity = match (labels[pair.first], labels[pair.second]) {
            (false, true) => pair.costs[0][1],
            (true, false) => pair.costs[1][0],
            _ => 0.0,
        };
        if !checked_add(&mut total, capacity) {
            return None;
        }
    }
    for unary in unaries {
        let state = usize::from(labels[unary.piece]);
        for edge in 0..4 {
            if !checked_add(&mut total, unary.row_costs[state][edge]) {
                return None;
            }
        }
    }
    Some(total)
}

fn selected_rows_match(
    state: &IcsState,
    pairs: &[PairTerm],
    unaries: &[UnaryTerm],
    labels: &[bool],
) -> bool {
    pairs.iter().all(|pair| {
        pair_row_bits(&state.pair_rows[pair.pair_id])
            == pair.row_bits[usize::from(labels[pair.first])][usize::from(labels[pair.second])]
    }) && unaries.iter().all(|unary| {
        let selected = usize::from(labels[unary.piece]);
        (0..4).all(|edge| {
            edge_row_bits(&state.edge_rows[unary.piece][edge]) == unary.row_bits[selected][edge]
        })
    })
}

#[derive(Clone, Copy, Debug)]
struct ResidualEdge {
    to: usize,
    reverse: usize,
    capacity: f64,
}

struct DeterministicFlow {
    adjacency: Vec<Vec<ResidualEdge>>,
    level: Vec<usize>,
    cursor: Vec<usize>,
}

impl DeterministicFlow {
    fn new(nodes: usize) -> Self {
        Self {
            adjacency: vec![Vec::new(); nodes],
            level: vec![usize::MAX; nodes],
            cursor: vec![0; nodes],
        }
    }

    fn add_edge(&mut self, from: usize, to: usize, capacity: f64) {
        if capacity == 0.0 {
            return;
        }
        let reverse_at_to = self.adjacency[to].len();
        let reverse_at_from = self.adjacency[from].len();
        self.adjacency[from].push(ResidualEdge {
            to,
            reverse: reverse_at_to,
            capacity,
        });
        self.adjacency[to].push(ResidualEdge {
            to: from,
            reverse: reverse_at_from,
            capacity: 0.0,
        });
    }

    fn build_levels(&mut self, source: usize, sink: usize) -> bool {
        self.level.fill(usize::MAX);
        self.level[source] = 0;
        let mut queue = VecDeque::new();
        queue.push_back(source);
        while let Some(node) = queue.pop_front() {
            for edge in &self.adjacency[node] {
                if edge.capacity > 0.0 && self.level[edge.to] == usize::MAX {
                    self.level[edge.to] = self.level[node] + 1;
                    queue.push_back(edge.to);
                }
            }
        }
        self.level[sink] != usize::MAX
    }

    fn send(&mut self, node: usize, sink: usize, limit: f64) -> f64 {
        if node == sink {
            return limit;
        }
        while self.cursor[node] < self.adjacency[node].len() {
            let edge_index = self.cursor[node];
            let edge = self.adjacency[node][edge_index];
            if edge.capacity > 0.0 && self.level[edge.to] == self.level[node] + 1 {
                let sent = self.send(edge.to, sink, limit.min(edge.capacity));
                if sent > 0.0 {
                    self.adjacency[node][edge_index].capacity -= sent;
                    self.adjacency[edge.to][edge.reverse].capacity += sent;
                    return sent;
                }
            }
            self.cursor[node] += 1;
        }
        0.0
    }

    fn max_flow(&mut self, source: usize, sink: usize) {
        while self.build_levels(source, sink) {
            self.cursor.fill(0);
            loop {
                let sent = self.send(source, sink, f64::INFINITY);
                if sent == 0.0 {
                    break;
                }
            }
        }
    }

    fn source_reachable(&self, source: usize) -> Vec<bool> {
        let mut reachable = vec![false; self.adjacency.len()];
        reachable[source] = true;
        let mut queue = VecDeque::new();
        queue.push_back(source);
        while let Some(node) = queue.pop_front() {
            for edge in &self.adjacency[node] {
                if edge.capacity > 0.0 && !reachable[edge.to] {
                    reachable[edge.to] = true;
                    queue.push_back(edge.to);
                }
            }
        }
        reachable
    }
}

fn solve_cut(nodes: usize, source: usize, sink: usize, edges: &[GraphEdge]) -> Option<Vec<bool>> {
    if source >= nodes
        || sink >= nodes
        || source == sink
        || edges.iter().any(|edge| {
            edge.from >= nodes || edge.to >= nodes || !finite_nonnegative_value(edge.capacity)
        })
    {
        return None;
    }
    let mut flow = DeterministicFlow::new(nodes);
    for edge in edges {
        flow.add_edge(edge.from, edge.to, edge.capacity);
    }
    flow.max_flow(source, sink);
    Some(flow.source_reachable(source))
}

fn vector_graph(unaries: &[[f64; 2]], directed: &[(usize, usize, f64)]) -> Vec<GraphEdge> {
    let count = unaries.len();
    let source = count;
    let sink = count + 1;
    let mut edges = Vec::new();
    for (piece, unary) in unaries.iter().enumerate() {
        edges.push(GraphEdge {
            from: source,
            to: piece,
            capacity: unary[1],
        });
        edges.push(GraphEdge {
            from: piece,
            to: sink,
            capacity: unary[0],
        });
    }
    edges.extend(directed.iter().map(|(from, to, capacity)| GraphEdge {
        from: *from,
        to: *to,
        capacity: *capacity,
    }));
    edges
}

fn vector_cut_capacity(
    labels: &[bool],
    unaries: &[[f64; 2]],
    directed: &[(usize, usize, f64)],
) -> f64 {
    let mut total = 0.0;
    for (piece, unary) in unaries.iter().enumerate() {
        total += unary[usize::from(labels[piece])];
    }
    for (from, to, capacity) in directed {
        if !labels[*from] && labels[*to] {
            total += capacity;
        }
    }
    total
}

fn vector_energy(labels: &[bool], unaries: &[[f64; 2]], directed: &[(usize, usize, f64)]) -> f64 {
    let mut total = 0.0;
    for (piece, unary) in unaries.iter().enumerate() {
        total += unary[usize::from(labels[piece])];
    }
    for pair in directed.chunks_exact(2) {
        let (first, second, c01) = pair[0];
        let (reverse_first, reverse_second, c10) = pair[1];
        assert_eq!((first, second), (reverse_second, reverse_first));
        total += match (labels[first], labels[second]) {
            (false, true) => c01,
            (true, false) => c10,
            _ => 0.0,
        };
    }
    total
}

fn digest_start(domain: &[u8]) -> Sha256 {
    let mut digest = Sha256::new();
    push_bytes(&mut digest, domain);
    digest
}

fn push_bytes(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

fn push_usize(digest: &mut Sha256, value: usize) {
    digest.update((value as u64).to_le_bytes());
}

fn push_f64(digest: &mut Sha256, value: f64) {
    digest.update(value.to_bits().to_le_bytes());
}

fn pose_digest(poses: &[Pose]) -> [u8; 32] {
    let mut digest = digest_start(POSE_DOMAIN);
    push_usize(&mut digest, poses.len());
    for (piece, pose) in poses.iter().enumerate() {
        push_usize(&mut digest, piece);
        push_f64(&mut digest, pose.tx_mm);
        push_f64(&mut digest, pose.ty_mm);
        push_f64(&mut digest, pose.theta_deg);
        digest.update([u8::from(pose.mirrored)]);
    }
    digest.finalize().into()
}

fn term_digest(pairs: &[PairTerm], unaries: &[UnaryTerm]) -> [u8; 32] {
    let mut digest = digest_start(TERM_DOMAIN);
    push_usize(&mut digest, pairs.len());
    for pair in pairs {
        push_usize(&mut digest, pair.pair_id);
        push_usize(&mut digest, pair.first);
        push_usize(&mut digest, pair.second);
        for first in 0..2 {
            for second in 0..2 {
                push_f64(&mut digest, pair.violations_mm[first][second]);
                push_f64(&mut digest, pair.costs[first][second]);
                push_pair_row_bits(&mut digest, &pair.row_bits[first][second]);
            }
        }
        digest.update([
            u8::from(pair.finite_nonnegative),
            u8::from(pair.zero_diagonal),
            u8::from(pair.submodular),
        ]);
    }
    push_usize(&mut digest, unaries.len());
    for unary in unaries {
        push_usize(&mut digest, unary.piece);
        for state in 0..2 {
            for edge in 0..4 {
                push_f64(&mut digest, unary.violations_mm[state][edge]);
                push_f64(&mut digest, unary.row_costs[state][edge]);
                push_edge_row_bits(&mut digest, &unary.row_bits[state][edge]);
            }
            push_f64(&mut digest, unary.sums[state]);
        }
        digest.update([u8::from(unary.finite_nonnegative)]);
    }
    digest.finalize().into()
}

fn graph_digest(nodes: usize, edges: &[GraphEdge]) -> [u8; 32] {
    let mut digest = digest_start(GRAPH_DOMAIN);
    push_usize(&mut digest, nodes);
    push_usize(&mut digest, edges.len());
    for edge in edges {
        push_usize(&mut digest, edge.from);
        push_usize(&mut digest, edge.to);
        push_f64(&mut digest, edge.capacity);
    }
    digest.finalize().into()
}

fn label_digest(labels: &[bool]) -> [u8; 32] {
    bool_vector_digest(LABEL_DOMAIN, labels)
}

fn bool_vector_digest(domain: &[u8], values: &[bool]) -> [u8; 32] {
    let mut digest = digest_start(domain);
    push_usize(&mut digest, values.len());
    for (piece, value) in values.iter().enumerate() {
        push_usize(&mut digest, piece);
        digest.update([u8::from(*value)]);
    }
    digest.finalize().into()
}

fn row_digest(state: &IcsState) -> [u8; 32] {
    let mut digest = digest_start(ROW_DOMAIN);
    push_usize(&mut digest, state.pair_rows.len());
    for (pair, row) in state.pair_rows.iter().enumerate() {
        push_usize(&mut digest, pair);
        push_pair_row_bits(&mut digest, &pair_row_bits(row));
    }
    push_usize(&mut digest, state.edge_rows.len());
    for (piece, rows) in state.edge_rows.iter().enumerate() {
        push_usize(&mut digest, piece);
        for (edge, row) in rows.iter().enumerate() {
            push_usize(&mut digest, edge);
            push_edge_row_bits(&mut digest, &edge_row_bits(row));
        }
    }
    digest.finalize().into()
}

fn push_pair_row_bits(digest: &mut Sha256, row: &PairRowBits) {
    digest.update(row.violation_mm.to_le_bytes());
    digest.update(row.weight.to_le_bytes());
    digest.update(row.signed_gap_mm.to_le_bytes());
    for value in row.normal {
        digest.update(value.to_le_bytes());
    }
    for value in row.witness_a {
        digest.update(value.to_le_bytes());
    }
    for value in row.witness_b {
        digest.update(value.to_le_bytes());
    }
}

fn push_edge_row_bits(digest: &mut Sha256, row: &EdgeRowBits) {
    digest.update(row.violation_mm.to_le_bytes());
    digest.update(row.weight.to_le_bytes());
    for value in row.witness {
        digest.update(value.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph_of(unaries: &[[f64; 2]], directed: &[(usize, usize, f64)]) -> Vec<GraphEdge> {
        let count = unaries.len();
        let source = count;
        let sink = count + 1;
        let mut edges = Vec::new();
        for (piece, unary) in unaries.iter().enumerate() {
            edges.push(GraphEdge {
                from: source,
                to: piece,
                capacity: unary[1],
            });
            edges.push(GraphEdge {
                from: piece,
                to: sink,
                capacity: unary[0],
            });
        }
        edges.extend(directed.iter().map(|(from, to, capacity)| GraphEdge {
            from: *from,
            to: *to,
            capacity: *capacity,
        }));
        edges
    }

    fn cut_capacity(
        labels: &[bool],
        unaries: &[[f64; 2]],
        directed: &[(usize, usize, f64)],
    ) -> f64 {
        let mut total = 0.0;
        for (piece, unary) in unaries.iter().enumerate() {
            total += unary[usize::from(labels[piece])];
        }
        for (from, to, capacity) in directed {
            if !labels[*from] && labels[*to] {
                total += capacity;
            }
        }
        total
    }

    #[test]
    fn asymmetric_graph_matches_exhaustive_unique_nontrivial_cut() {
        let report = gate0_vector_report();
        assert!(report.unique_nontrivial_minimum);
        assert!(report.every_label_cut_energy_identity);
        assert_eq!(report.solver_labels, report.expected_labels);
        let unaries = [[0.0, 8.0], [7.0, 0.0], [0.0, 6.0], [5.0, 0.0]];
        let directed = [
            (0, 1, 1.0),
            (1, 0, 9.0),
            (1, 2, 2.0),
            (2, 1, 7.0),
            (2, 3, 1.0),
            (3, 2, 8.0),
        ];
        let edges = graph_of(&unaries, &directed);
        let reachable = solve_cut(6, 4, 5, &edges).expect("valid graph");
        let labels = reachable[..4].iter().map(|side| !*side).collect::<Vec<_>>();
        let mut exhaustive = (0..16)
            .map(|mask| {
                let candidate = (0..4)
                    .map(|piece| mask & (1 << piece) != 0)
                    .collect::<Vec<_>>();
                (cut_capacity(&candidate, &unaries, &directed), candidate)
            })
            .collect::<Vec<_>>();
        exhaustive.sort_by(|left, right| left.0.total_cmp(&right.0));
        assert!(
            exhaustive[0].0 < exhaustive[1].0,
            "the vector must be unique"
        );
        assert_eq!(labels, exhaustive[0].1);
        assert_ne!(labels, vec![false; 4]);
        assert_ne!(labels, vec![true; 4]);
    }

    #[test]
    fn directed_cut_capacity_is_the_binary_energy_for_every_labeling() {
        let unaries = [[1.0, 4.0], [3.0, 2.0], [5.0, 0.5]];
        let directed = [(0, 1, 7.0), (1, 0, 2.0), (0, 2, 1.5), (2, 0, 6.0)];
        for mask in 0..8 {
            let labels = (0..3)
                .map(|piece| mask & (1 << piece) != 0)
                .collect::<Vec<_>>();
            let graph_value = cut_capacity(&labels, &unaries, &directed);
            let mut energy = 0.0;
            for (piece, unary) in unaries.iter().enumerate() {
                energy += unary[usize::from(labels[piece])];
            }
            energy += if !labels[0] && labels[1] { 7.0 } else { 0.0 };
            energy += if labels[0] && !labels[1] { 2.0 } else { 0.0 };
            energy += if !labels[0] && labels[2] { 1.5 } else { 0.0 };
            energy += if labels[0] && !labels[2] { 6.0 } else { 0.0 };
            assert_eq!(graph_value.to_bits(), energy.to_bits());
        }
    }

    #[test]
    fn pair_literal_validation_accepts_only_the_frozen_domain() {
        assert_eq!(
            validate_cost_vector([[0.0, 2.0], [3.0, 0.0]]),
            (true, true, true)
        );
        assert!(!validate_cost_vector([[0.0, f64::NAN], [1.0, 0.0]]).0);
        assert!(!validate_cost_vector([[0.0, -1.0], [1.0, 0.0]]).0);
        assert!(!validate_cost_vector([[0.25, 2.0], [3.0, 0.0]]).1);
        assert!(!validate_cost_vector([[2.0, 1.0], [1.0, 2.0]]).2);
        assert!(!validate_cost_vector([[0.0, f64::MAX], [f64::MAX, 0.0]]).0);
        assert!(!valid_depth_transition(1.0, 1.0, 0.0));
        assert!(!valid_depth_transition(1.0, 1.5, 0.5));
    }

    #[test]
    fn pose_labels_change_only_selected_ty_bits() {
        let parent = vec![
            Pose {
                tx_mm: 1.25,
                ty_mm: 8.0,
                theta_deg: -17.0,
                mirrored: false,
            },
            Pose {
                tx_mm: -2.0,
                ty_mm: 9.5,
                theta_deg: 31.0,
                mirrored: true,
            },
        ];
        let delta = -0.125;
        let selected = apply_labels(&parent, &[false, true], delta);
        assert_eq!(selected[0], parent[0]);
        assert_eq!(selected[1].tx_mm.to_bits(), parent[1].tx_mm.to_bits());
        assert_eq!(
            selected[1].theta_deg.to_bits(),
            parent[1].theta_deg.to_bits()
        );
        assert_eq!(selected[1].mirrored, parent[1].mirrored);
        assert_eq!(
            selected[1].ty_mm.to_bits(),
            (parent[1].ty_mm + delta).to_bits()
        );
    }

    #[test]
    fn residual_reachability_is_stable_for_trivial_optima_and_ties() {
        let all_zero = graph_of(&[[0.0, 3.0], [0.0, 4.0]], &[]);
        assert_eq!(
            solve_cut(4, 2, 3, &all_zero).unwrap()[..2]
                .iter()
                .map(|side| !*side)
                .collect::<Vec<_>>(),
            [false, false]
        );
        let all_one = graph_of(&[[3.0, 0.0], [4.0, 0.0]], &[]);
        assert_eq!(
            solve_cut(4, 2, 3, &all_one).unwrap()[..2]
                .iter()
                .map(|side| !*side)
                .collect::<Vec<_>>(),
            [true, true]
        );
        let tie = graph_of(&[[0.0, 0.0], [0.0, 0.0]], &[]);
        let first = solve_cut(4, 2, 3, &tie).unwrap();
        let second = solve_cut(4, 2, 3, &tie).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first[..2].iter().map(|side| !*side).collect::<Vec<_>>(),
            [true, true]
        );
    }

    #[test]
    fn common_translation_and_incremental_selected_state_match_the_cold_table() {
        let report = geometry_gate0_vector_report();
        let decision = &report.decision;
        assert!(decision.valid, "{:?}", decision.invalid_reason);
        assert_eq!(decision.labels, [false, true, false, false, false]);
        assert!(decision.labels.iter().any(|label| *label));
        assert!(decision.labels.iter().any(|label| !*label));
        assert!(decision.pair_terms.iter().any(|pair| {
            pair.violations_mm[usize::from(decision.labels[pair.first])]
                [usize::from(decision.labels[pair.second])]
                > 0.0
        }));
        assert!(decision.unary_terms.iter().any(|unary| {
            unary.violations_mm[usize::from(decision.labels[unary.piece])]
                .iter()
                .any(|violation| *violation > 0.0)
        }));
        for pair in &decision.pair_terms {
            assert_eq!(pair.costs[0][0].to_bits(), 0.0f64.to_bits());
            assert_eq!(pair.costs[1][1].to_bits(), 0.0f64.to_bits());
        }
        assert!(report.incremental_matches_cold);
        assert_eq!(
            report.incremental_raw_phi.to_bits(),
            decision.cold_raw_phi.to_bits()
        );
        assert_eq!(
            report.incremental_pose_digest_sha256,
            decision.installed_pose_digest_sha256
        );
        assert_eq!(
            report.incremental_row_digest_sha256,
            decision.installed_row_digest_sha256
        );
    }
}
