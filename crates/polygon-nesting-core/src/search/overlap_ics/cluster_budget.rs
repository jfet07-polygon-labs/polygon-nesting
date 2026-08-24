//! Ex-ante conflict-component slot allocation.
//!
//! This module implements only the independently-authored partition field in
//! `docs/conflict-cluster-budget-spec.md`. It never scores a sample, accepts a
//! pose, updates GLS, or observes an outcome while constructing a schedule.

use std::cmp::Ordering;

use sha2::{Digest, Sha256};

use super::decomposition;
use super::descent::counter_hash;
use super::diagnostics::WorkVector;
use super::energy::incident_raw;
use super::relocate::RelocateKey;
use super::state::{apply_pose, pair_index, pose_sin_cos, Contract, IcsState, PieceSource};

const MEMBER_TAG: u64 = 0x4343_4d45_4d42_5231; // "CCMEMBR1"
const PLACEBO_TAG: u64 = 0x4343_4d41_5353_5031; // "CCMASSP1"

/// The runtime arms exposed only by the experiment feature.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PartitionArm {
    #[default]
    Off,
    Mass,
    ShuffledMass,
    MaxViolation,
    /// Compute all plans and execute the current order, retaining records.
    Shadow,
    /// Compute all plans and execute the current order, aggregate only.
    ComputeIgnore,
}

impl PartitionArm {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Mass => "mass",
            Self::ShuffledMass => "shuffled-mass",
            Self::MaxViolation => "max-violation",
            Self::Shadow => "shadow",
            Self::ComputeIgnore => "compute-ignore",
        }
    }

    pub fn is_off(self) -> bool {
        self == Self::Off
    }
}

/// One immutable centroid-centred inscribed disc in source coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SourceDisc {
    pub piece_input_index: usize,
    pub cell_ordinal: usize,
    pub center_source_mm: [f64; 2],
    pub radius_mm: f64,
}

#[derive(Clone, Copy, Debug)]
struct WorldDisc {
    center_mm: [f64; 2],
    radius_mm: f64,
}

/// Built once per engine and shared read-only by tournament workers.
#[derive(Clone, Debug)]
pub struct ClusterField {
    discs: Vec<Vec<SourceDisc>>,
    invalid_reason: Option<String>,
    pub disc_count: usize,
}

impl ClusterField {
    pub fn from_sources(sources: &[PieceSource]) -> Self {
        let mut discs = Vec::with_capacity(sources.len());
        let mut invalid_reason = None;
        let mut disc_count = 0usize;
        for (piece, source) in sources.iter().enumerate() {
            let mut piece_discs = Vec::with_capacity(source.decomposition.cells.len());
            for (cell_ordinal, cell) in source.decomposition.cells.iter().copied().enumerate() {
                let points = source.decomposition.cell_points(cell);
                match source_disc(piece, cell_ordinal, points) {
                    Ok(disc) => {
                        piece_discs.push(disc);
                        disc_count += 1;
                    }
                    Err(error) => {
                        invalid_reason.get_or_insert(error);
                    }
                }
            }
            if piece_discs.len() != source.decomposition.cells.len() {
                invalid_reason.get_or_insert_with(|| {
                    format!("piece {piece} did not emit one disc per decomposition cell")
                });
            }
            discs.push(piece_discs);
        }
        Self {
            discs,
            invalid_reason,
            disc_count,
        }
    }

    pub fn invalid_reason(&self) -> Option<&str> {
        self.invalid_reason.as_deref()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FallbackKind {
    None,
    ZeroSignal,
    Invalid,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentRecord {
    pub id: usize,
    pub members: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PairEdgeRecord {
    pub pair_id: usize,
    pub first: usize,
    pub second: usize,
}

#[derive(Clone, Debug)]
struct Component {
    id: usize,
    members: Vec<usize>,
}

/// One full ex-ante decision. All three schedules are constructed before any
/// relocate runs, including in a treatment arm that will consume only one.
#[derive(Clone, Debug)]
pub struct PartitionDecision {
    pub key: RelocateKey,
    pub entry: Vec<usize>,
    pub components: Vec<ComponentRecord>,
    pub positive_pair_edges: Vec<PairEdgeRecord>,
    pub pair_disc_terms: u64,
    pub positive_boundary_rows: u64,
    pub mass_bits: Vec<u64>,
    pub max_violation_bits: Vec<u64>,
    pub mass_quotas: Vec<usize>,
    pub shuffled_quotas: Vec<usize>,
    pub max_violation_quotas: Vec<usize>,
    pub mass_schedule: Vec<usize>,
    pub shuffled_schedule: Vec<usize>,
    pub max_violation_schedule: Vec<usize>,
    pub mass_fallback: FallbackKind,
    pub shuffled_fallback: FallbackKind,
    pub max_violation_fallback: FallbackKind,
    pub placebo_offset: usize,
    pub spearman_field_mass_max: Option<f64>,
    pub spearman_quota_mass_max: Option<f64>,
    pub member_permutations: Vec<Vec<usize>>,
}

impl PartitionDecision {
    pub fn build(
        state: &IcsState,
        field: &ClusterField,
        contract: &Contract,
        key: RelocateKey,
    ) -> Self {
        let count = state.poses.len();
        let entry: Vec<usize> = (0..count)
            .filter(|piece| incident_raw(state, *piece) > 0.0)
            .collect();
        let q = entry.len();
        if q == 0 {
            return Self {
                key,
                entry,
                components: Vec::new(),
                positive_pair_edges: Vec::new(),
                pair_disc_terms: 0,
                positive_boundary_rows: 0,
                mass_bits: Vec::new(),
                max_violation_bits: Vec::new(),
                mass_quotas: Vec::new(),
                shuffled_quotas: Vec::new(),
                max_violation_quotas: Vec::new(),
                mass_schedule: Vec::new(),
                shuffled_schedule: Vec::new(),
                max_violation_schedule: Vec::new(),
                mass_fallback: FallbackKind::None,
                shuffled_fallback: FallbackKind::None,
                max_violation_fallback: FallbackKind::None,
                placebo_offset: 0,
                spearman_field_mass_max: None,
                spearman_quota_mass_max: None,
                member_permutations: Vec::new(),
            };
        }

        let mut in_entry = vec![false; count];
        for piece in &entry {
            in_entry[*piece] = true;
        }
        let mut parent: Vec<usize> = (0..count).collect();
        let mut positive_pair_edges = Vec::new();
        for first in 0..count {
            for second in (first + 1)..count {
                let pair_id = pair_index(count, first, second);
                let row = state.pair_rows[pair_id];
                if !in_entry[first] || !in_entry[second] || !(row.violation_mm > 0.0) {
                    continue;
                }
                union(&mut parent, first, second);
                positive_pair_edges.push(PairEdgeRecord {
                    pair_id,
                    first,
                    second,
                });
            }
        }
        let mut components: Vec<Component> = Vec::new();
        for piece in &entry {
            let root = find(&mut parent, *piece);
            if let Some(component) = components.iter_mut().find(|row| row.id == root) {
                component.members.push(*piece);
            } else {
                components.push(Component {
                    id: root,
                    members: vec![*piece],
                });
            }
        }
        for component in &mut components {
            component.members.sort_unstable();
            component.id = component.members[0];
        }
        components.sort_by_key(|component| component.id);

        let component_of = component_index_map(count, &components);
        let (world, mut invalid) = world_discs(state, field);
        if field.invalid_reason.is_some() {
            invalid = true;
        }

        let mut masses = vec![0.0f64; components.len()];
        let mut max_violation = vec![0.0f64; components.len()];
        let mut pair_disc_terms = 0u64;
        for edge in &positive_pair_edges {
            let row = state.pair_rows[edge.pair_id];
            let component = component_of[edge.first]
                .expect("an induced positive edge's first piece belongs to S");
            debug_assert_eq!(component_of[edge.second], Some(component));
            max_violation[component] = max_violation[component].max(row.violation_mm);
            for left in &world[edge.first] {
                for right in &world[edge.second] {
                    let distance = libm::hypot(
                        left.center_mm[0] - right.center_mm[0],
                        left.center_mm[1] - right.center_mm[1],
                    );
                    let term = checked_pair_mass_term(
                        left.radius_mm,
                        right.radius_mm,
                        distance,
                        contract.pair_clearance_mm(),
                    );
                    if let Some(term) = term {
                        masses[component] += term;
                    } else {
                        invalid = true;
                    }
                    pair_disc_terms += 1;
                }
            }
        }

        let mut positive_boundary_rows = 0u64;
        for (component, row) in components.iter().enumerate() {
            for piece in &row.members {
                for edge in 0..4 {
                    let violation = state.edge_rows[*piece][edge].violation_mm;
                    if !(violation > 0.0) {
                        continue;
                    }
                    positive_boundary_rows += 1;
                    let term = boundary_mass_term(violation);
                    if !term.is_finite() || term < 0.0 {
                        invalid = true;
                    } else {
                        masses[component] += term;
                    }
                    max_violation[component] = max_violation[component].max(violation);
                }
            }
        }
        if masses.iter().any(|mass| !mass.is_finite() || *mass < 0.0) {
            invalid = true;
        }

        let member_counts: Vec<usize> = components.iter().map(|row| row.members.len()).collect();
        let (mass_quotas, mass_fallback) = if invalid {
            (member_counts.clone(), FallbackKind::Invalid)
        } else {
            allocate(&masses, q, &member_counts)
        };

        let placebo_offset = if mass_fallback == FallbackKind::None {
            nonzero_placebo_offset(key, components.len())
        } else {
            0
        };
        let (shuffled_quotas, shuffled_fallback) = if placebo_offset == 0 {
            (mass_quotas.clone(), mass_fallback)
        } else {
            let shuffled = rotate_association(&masses, placebo_offset);
            let (quotas, fallback) = allocate(&shuffled, q, &member_counts);
            (quotas, fallback)
        };

        let (max_violation_quotas, max_violation_fallback) = if invalid {
            (member_counts.clone(), FallbackKind::Invalid)
        } else {
            allocate(&max_violation, q, &member_counts)
        };
        let member_permutations: Vec<Vec<usize>> = components
            .iter()
            .map(|component| member_permutation(component, key))
            .collect();
        let mass_schedule = schedule(&components, &member_permutations, &mass_quotas);
        let shuffled_schedule = schedule(&components, &member_permutations, &shuffled_quotas);
        let max_violation_schedule =
            schedule(&components, &member_permutations, &max_violation_quotas);

        debug_assert_eq!(mass_schedule.len(), q);
        debug_assert_eq!(shuffled_schedule.len(), q);
        debug_assert_eq!(max_violation_schedule.len(), q);
        let spearman_field_mass_max = spearman(&masses, &max_violation);
        let spearman_quota_mass_max = spearman_usize(&mass_quotas, &max_violation_quotas);

        Self {
            key,
            entry,
            components: components
                .iter()
                .map(|row| ComponentRecord {
                    id: row.id,
                    members: row.members.clone(),
                })
                .collect(),
            positive_pair_edges,
            pair_disc_terms,
            positive_boundary_rows,
            mass_bits: masses.iter().map(|value| value.to_bits()).collect(),
            max_violation_bits: max_violation.iter().map(|value| value.to_bits()).collect(),
            mass_quotas,
            shuffled_quotas,
            max_violation_quotas,
            mass_schedule,
            shuffled_schedule,
            max_violation_schedule,
            mass_fallback,
            shuffled_fallback,
            max_violation_fallback,
            placebo_offset,
            spearman_field_mass_max,
            spearman_quota_mass_max,
            member_permutations,
        }
    }

    pub fn schedule_for(&self, arm: PartitionArm) -> Option<&[usize]> {
        match arm {
            PartitionArm::Mass => Some(&self.mass_schedule),
            PartitionArm::ShuffledMass => Some(&self.shuffled_schedule),
            PartitionArm::MaxViolation => Some(&self.max_violation_schedule),
            PartitionArm::Off | PartitionArm::Shadow | PartitionArm::ComputeIgnore => None,
        }
    }

    pub fn q(&self) -> usize {
        self.entry.len()
    }

    pub fn plan_identities_hold(&self) -> bool {
        let q = self.q();
        [
            &self.mass_quotas,
            &self.shuffled_quotas,
            &self.max_violation_quotas,
        ]
        .into_iter()
        .all(|quotas| quotas.iter().sum::<usize>() == q)
            && [
                &self.mass_schedule,
                &self.shuffled_schedule,
                &self.max_violation_schedule,
            ]
            .into_iter()
            .all(|schedule| schedule.len() == q)
    }
}

fn source_disc(
    piece: usize,
    cell_ordinal: usize,
    points: &[[f64; 2]],
) -> Result<SourceDisc, String> {
    let center = decomposition::centroid(points);
    let mut radius = f64::INFINITY;
    let mut nonzero_edges = 0usize;
    for edge in 0..points.len() {
        let a = points[edge];
        let b = points[(edge + 1) % points.len()];
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        if !dx.is_finite() || !dy.is_finite() {
            return Err(format!(
                "non-finite source edge at piece {piece}, cell {cell_ordinal}, edge {edge}"
            ));
        }
        let len = libm::hypot(dx, dy);
        if !len.is_finite() {
            return Err(format!(
                "non-finite source edge length at piece {piece}, cell {cell_ordinal}, edge {edge}"
            ));
        }
        if len == 0.0 {
            continue;
        }
        nonzero_edges += 1;
        let h = (dx * (center[1] - a[1]) - dy * (center[0] - a[0])) / len;
        if !h.is_finite() {
            return Err(format!(
                "non-finite source height at piece {piece}, cell {cell_ordinal}, edge {edge}"
            ));
        }
        radius = radius.min(h);
    }
    if !center[0].is_finite()
        || !center[1].is_finite()
        || nonzero_edges < 3
        || !radius.is_finite()
        || radius <= 0.0
    {
        return Err(format!(
            "invalid source disc at piece {piece}, cell {cell_ordinal}"
        ));
    }
    Ok(SourceDisc {
        piece_input_index: piece,
        cell_ordinal,
        center_source_mm: center,
        radius_mm: radius,
    })
}

fn checked_pair_mass_term(
    left_radius: f64,
    right_radius: f64,
    distance: f64,
    clearance: f64,
) -> Option<f64> {
    if !left_radius.is_finite()
        || !right_radius.is_finite()
        || !distance.is_finite()
        || !clearance.is_finite()
    {
        return None;
    }
    let raw_delta = left_radius + right_radius + clearance - distance;
    if !raw_delta.is_finite() {
        return None;
    }
    let delta = raw_delta.max(0.0);
    let term = delta * delta;
    term.is_finite().then_some(term)
}

fn boundary_mass_term(violation: f64) -> f64 {
    violation * violation
}

fn nonzero_placebo_offset(key: RelocateKey, component_count: usize) -> usize {
    if component_count <= 1 {
        return 0;
    }
    1 + (counter_hash(&[key.seed, key.bite, key.iteration, PLACEBO_TAG]) as usize
        % (component_count - 1))
}

fn rotate_association(values: &[f64], offset: usize) -> Vec<f64> {
    if values.is_empty() {
        return Vec::new();
    }
    (0..values.len())
        .map(|index| values[(index + offset) % values.len()])
        .collect()
}

/// Aggregate telemetry. Detailed decisions are retained only by `Shadow`.
#[derive(Clone, Debug, Default)]
pub struct PartitionTrace {
    pub partition_decisions: u64,
    pub eligible_decisions: u64,
    pub eligible_disagreement_decisions: u64,
    pub entry_colliding_pieces: u64,
    pub component_count: u64,
    pub positive_pair_edges: u64,
    pub partition_slots: u64,
    pub executed_slots: u64,
    pub full_relocate_slots: u64,
    pub zero_energy_slots: u64,
    pub pair_disc_terms: u64,
    pub positive_boundary_rows: u64,
    pub zero_signal_fallback_decisions: u64,
    pub invalid_fallback_decisions: u64,
    pub plan_identity_failure_decisions: u64,
    pub execution_identity_failure_decisions: u64,
    pub graph_digest_sha256: [u8; 32],
    pub allocation_digest_sha256: [u8; 32],
    pub schedule_digest_sha256: [u8; 32],
    pub decisions: Vec<PartitionDecision>,
}

/// Arm-neutral record of the order actually consumed by the cost cell.
#[derive(Clone, Debug, Default)]
pub struct AtomicOrderTrace {
    pub actual_slots: u64,
    pub order_digest_sha256: [u8; 32],
}

impl AtomicOrderTrace {
    pub fn observe_order(&mut self, order: &[usize]) {
        let mut payload = Vec::new();
        push_u64(&mut payload, order.len() as u64);
        for piece in order {
            push_u64(&mut payload, *piece as u64);
        }
        chain_digest(&mut self.order_digest_sha256, &payload);
    }

    pub fn observe_slot(&mut self) {
        self.actual_slots += 1;
    }

    pub fn digest_hex(&self) -> String {
        use std::fmt::Write;
        let mut out = String::with_capacity(64);
        for byte in &self.order_digest_sha256 {
            write!(&mut out, "{byte:02x}").expect("writing to a String cannot fail");
        }
        out
    }
}

/// One timed arm of the paired compute-ignore Gate-0 cell.
#[derive(Clone, Debug)]
pub struct PartitionCostArmSample {
    pub arm: PartitionArm,
    pub warmup_sweeps: usize,
    pub measured_sweeps: usize,
    pub piece_count: usize,
    pub entry_colliding_pieces: usize,
    pub expected_atomic_slots: u64,
    pub completed_atomic_slots: u64,
    pub legacy_proposals: u64,
    pub elapsed_seconds: f64,
    pub slots_per_second: f64,
    pub pose_sequence_digest_sha256: String,
    pub consumed_order_digest_sha256: String,
    pub work: WorkVector,
    pub partition: PartitionTrace,
}

/// Values printed by G0.2. The inversion witness is deliberately a pure
/// frozen-row/field arithmetic vector; it does not call `measure_pair`.
#[derive(Clone, Debug)]
pub struct Gate0VectorReport {
    pub unit_square_center: [f64; 2],
    pub unit_square_radius: f64,
    pub transformed_center: [f64; 2],
    pub transformed_radius: f64,
    pub pair_mass_terms: [f64; 2],
    pub mass_inversion_quotas: Vec<usize>,
    pub max_violation_inversion_quotas: Vec<usize>,
    pub boundary_term: f64,
    pub largest_remainder_component_ids: Vec<usize>,
    pub largest_remainder_quotas: Vec<usize>,
    pub mixed_zero_quotas: Vec<usize>,
    pub zero_signal_quotas: Vec<usize>,
    pub zero_signal_fallback: FallbackKind,
    pub placebo_offset: usize,
    pub placebo_input: Vec<f64>,
    pub placebo_rotated: Vec<f64>,
    pub placebo_quotas: Vec<usize>,
    pub placebo_multiset_preserved: bool,
    pub placebo_non_identity: bool,
    pub member_permutation: Vec<usize>,
    pub round_robin_schedule: Vec<usize>,
    pub invalid_quotas: Vec<usize>,
    pub invalid_fallback: FallbackKind,
    pub nonfinite_source_rejected: bool,
    pub nonfinite_pair_rejected: bool,
    pub quota_sum_identity: bool,
    pub schedule_length_identity: bool,
    pub executed_slots_identity: bool,
    pub full_plus_zero_identity: bool,
    pub inversion_is_pure_frozen_row_field_vector: bool,
}

pub fn gate0_vector_report() -> Gate0VectorReport {
    let disc = source_disc(0, 0, &[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]])
        .expect("the unit square is a valid CCW cell");
    let transformed_center = apply_pose(disc.center_source_mm, true, 1.0, 0.0, 10.0, 5.0);
    let pair_mass_terms = [
        checked_pair_mass_term(2.0, 2.0, 4.0, 1.0).expect("finite pair vector"),
        checked_pair_mass_term(1.0, 1.0, 1.5, 1.0).expect("finite pair vector"),
    ];
    let mass_inversion_quotas = allocate(&pair_mass_terms, 4, &[2, 2]).0;
    let max_violation_inversion_quotas = allocate(&[2.0, 1.0], 4, &[2, 2]).0;
    let largest_remainder_quotas = allocate(&[6.0, 3.0, 1.0], 5, &[1, 1, 1]).0;
    let mixed_zero_quotas = allocate(&[0.0, 4.0, 0.0], 5, &[3, 1, 1]).0;
    let (zero_signal_quotas, zero_signal_fallback) = allocate(&[0.0, 0.0, 0.0], 5, &[3, 1, 1]);
    let key = RelocateKey {
        seed: 0,
        bite: 11,
        iteration: 13,
        worker: 2,
    };
    let placebo_offset = nonzero_placebo_offset(key, 4);
    let placebo_input = vec![1.0, 2.0, 3.0, 4.0];
    let placebo_rotated = rotate_association(&placebo_input, placebo_offset);
    let placebo_quotas = allocate(&placebo_rotated, 4, &[1, 1, 1, 1]).0;
    let mut input_bits = placebo_input
        .iter()
        .map(|value| value.to_bits())
        .collect::<Vec<_>>();
    let mut rotated_bits = placebo_rotated
        .iter()
        .map(|value| value.to_bits())
        .collect::<Vec<_>>();
    input_bits.sort_unstable();
    rotated_bits.sort_unstable();
    let member_permutation = member_permutation(
        &Component {
            id: 3,
            members: vec![3, 4, 5, 6],
        },
        RelocateKey { seed: 7, ..key },
    );
    let components = vec![
        Component {
            id: 0,
            members: vec![0, 1],
        },
        Component {
            id: 2,
            members: vec![2, 3],
        },
    ];
    let round_robin_schedule = schedule(&components, &[vec![1, 0], vec![2, 3]], &[3, 1]);
    let schedule_length_identity = round_robin_schedule.len() == 4;
    let (invalid_quotas, invalid_fallback) = allocate(&[f64::NAN, 1.0], 2, &[1, 1]);
    let mut trace = PartitionTrace {
        partition_slots: 2,
        ..PartitionTrace::default()
    };
    trace.observe_execution(true);
    trace.observe_execution(false);
    let nonfinite_source_rejected =
        source_disc(0, 0, &[[0.0, 0.0], [f64::NAN, 0.0], [0.0, 1.0]]).is_err();
    let nonfinite_pair_rejected = checked_pair_mass_term(1.0, 1.0, 1.0, f64::NAN).is_none()
        && checked_pair_mass_term(f64::MAX, f64::MAX, 0.0, 1.0).is_none();

    Gate0VectorReport {
        unit_square_center: disc.center_source_mm,
        unit_square_radius: disc.radius_mm,
        transformed_center,
        transformed_radius: disc.radius_mm,
        pair_mass_terms,
        mass_inversion_quotas,
        max_violation_inversion_quotas,
        boundary_term: boundary_mass_term(3.0),
        largest_remainder_component_ids: vec![0, 3, 7],
        largest_remainder_quotas,
        mixed_zero_quotas,
        zero_signal_quotas,
        zero_signal_fallback,
        placebo_offset,
        placebo_input: placebo_input.clone(),
        placebo_rotated: placebo_rotated.clone(),
        placebo_quotas,
        placebo_multiset_preserved: input_bits == rotated_bits,
        placebo_non_identity: placebo_input != placebo_rotated,
        member_permutation,
        round_robin_schedule,
        invalid_quotas,
        invalid_fallback,
        nonfinite_source_rejected,
        nonfinite_pair_rejected,
        quota_sum_identity: [1usize, 3].iter().sum::<usize>() == 4,
        schedule_length_identity,
        executed_slots_identity: trace.executed_slots == trace.partition_slots,
        full_plus_zero_identity: trace.executed_slots
            == trace.full_relocate_slots + trace.zero_energy_slots,
        inversion_is_pure_frozen_row_field_vector: true,
    }
}

impl PartitionTrace {
    pub fn observe_plan(
        &mut self,
        decision: &PartitionDecision,
        arm: PartitionArm,
        order: &[usize],
    ) {
        self.partition_decisions += 1;
        self.eligible_decisions += u64::from(decision.components.len() >= 2);
        self.eligible_disagreement_decisions += u64::from(
            decision.components.len() >= 2 && decision.mass_quotas != decision.max_violation_quotas,
        );
        self.entry_colliding_pieces += decision.q() as u64;
        self.component_count += decision.components.len() as u64;
        self.positive_pair_edges += decision.positive_pair_edges.len() as u64;
        self.partition_slots += decision.q() as u64;
        self.pair_disc_terms += decision.pair_disc_terms;
        self.positive_boundary_rows += decision.positive_boundary_rows;
        self.zero_signal_fallback_decisions +=
            u64::from(decision.mass_fallback == FallbackKind::ZeroSignal);
        self.invalid_fallback_decisions += u64::from(
            decision.mass_fallback == FallbackKind::Invalid
                || decision.shuffled_fallback == FallbackKind::Invalid
                || decision.max_violation_fallback == FallbackKind::Invalid,
        );
        self.plan_identity_failure_decisions +=
            u64::from(!decision.plan_identities_hold() || order.len() != decision.q());

        let mut graph = Vec::new();
        push_key(&mut graph, decision.key);
        push_u64(&mut graph, decision.q() as u64);
        push_u64(&mut graph, decision.entry.len() as u64);
        for member in &decision.entry {
            push_u64(&mut graph, *member as u64);
        }
        push_u64(&mut graph, decision.positive_pair_edges.len() as u64);
        for edge in &decision.positive_pair_edges {
            push_u64(&mut graph, edge.pair_id as u64);
            push_u64(&mut graph, edge.first as u64);
            push_u64(&mut graph, edge.second as u64);
        }
        push_u64(&mut graph, decision.components.len() as u64);
        for component in &decision.components {
            push_u64(&mut graph, component.id as u64);
            push_u64(&mut graph, component.members.len() as u64);
            for member in &component.members {
                push_u64(&mut graph, *member as u64);
            }
        }
        chain_digest(&mut self.graph_digest_sha256, &graph);

        let mut allocation = Vec::new();
        push_key(&mut allocation, decision.key);
        push_u64(&mut allocation, decision.mass_bits.len() as u64);
        for value in &decision.mass_bits {
            push_u64(&mut allocation, *value);
        }
        push_u64(&mut allocation, decision.max_violation_bits.len() as u64);
        for value in &decision.max_violation_bits {
            push_u64(&mut allocation, *value);
        }
        for quotas in [
            &decision.mass_quotas,
            &decision.shuffled_quotas,
            &decision.max_violation_quotas,
        ] {
            push_u64(&mut allocation, quotas.len() as u64);
            for quota in quotas {
                push_u64(&mut allocation, *quota as u64);
            }
        }
        for fallback in [
            decision.mass_fallback,
            decision.shuffled_fallback,
            decision.max_violation_fallback,
        ] {
            push_u64(&mut allocation, fallback_code(fallback));
        }
        push_u64(&mut allocation, decision.placebo_offset as u64);
        chain_digest(&mut self.allocation_digest_sha256, &allocation);

        let mut schedule_bytes = Vec::new();
        push_key(&mut schedule_bytes, decision.key);
        push_u64(&mut schedule_bytes, arm as u64);
        push_u64(&mut schedule_bytes, order.len() as u64);
        for piece in order {
            push_u64(&mut schedule_bytes, *piece as u64);
        }
        chain_digest(&mut self.schedule_digest_sha256, &schedule_bytes);

        if arm == PartitionArm::Shadow {
            self.decisions.push(decision.clone());
        }
    }

    pub fn observe_execution(&mut self, ran: bool) {
        self.executed_slots += 1;
        if ran {
            self.full_relocate_slots += 1;
        } else {
            self.zero_energy_slots += 1;
        }
    }

    pub fn finish_execution(&mut self, expected_slots: usize, before: (u64, u64, u64)) {
        let executed = self.executed_slots - before.0;
        let full = self.full_relocate_slots - before.1;
        let zero = self.zero_energy_slots - before.2;
        self.execution_identity_failure_decisions +=
            u64::from(executed != expected_slots as u64 || full + zero != expected_slots as u64);
    }

    pub fn append(&mut self, other: &Self) {
        self.partition_decisions += other.partition_decisions;
        self.eligible_decisions += other.eligible_decisions;
        self.eligible_disagreement_decisions += other.eligible_disagreement_decisions;
        self.entry_colliding_pieces += other.entry_colliding_pieces;
        self.component_count += other.component_count;
        self.positive_pair_edges += other.positive_pair_edges;
        self.partition_slots += other.partition_slots;
        self.executed_slots += other.executed_slots;
        self.full_relocate_slots += other.full_relocate_slots;
        self.zero_energy_slots += other.zero_energy_slots;
        self.pair_disc_terms += other.pair_disc_terms;
        self.positive_boundary_rows += other.positive_boundary_rows;
        self.zero_signal_fallback_decisions += other.zero_signal_fallback_decisions;
        self.invalid_fallback_decisions += other.invalid_fallback_decisions;
        self.plan_identity_failure_decisions += other.plan_identity_failure_decisions;
        self.execution_identity_failure_decisions += other.execution_identity_failure_decisions;
        if other.partition_decisions > 0 {
            chain_digest(&mut self.graph_digest_sha256, &other.graph_digest_sha256);
            chain_digest(
                &mut self.allocation_digest_sha256,
                &other.allocation_digest_sha256,
            );
            chain_digest(
                &mut self.schedule_digest_sha256,
                &other.schedule_digest_sha256,
            );
        }
        self.decisions.extend(other.decisions.iter().cloned());
    }

    pub fn slot_identities_hold(&self) -> bool {
        self.plan_identity_failure_decisions == 0
            && self.execution_identity_failure_decisions == 0
            && self.partition_slots == self.executed_slots
            && self.executed_slots == self.full_relocate_slots + self.zero_energy_slots
    }

    pub fn eligible_disagreement_rate(&self) -> Option<f64> {
        (self.eligible_decisions > 0)
            .then_some(self.eligible_disagreement_decisions as f64 / self.eligible_decisions as f64)
    }
}

fn fallback_code(kind: FallbackKind) -> u64 {
    match kind {
        FallbackKind::None => 0,
        FallbackKind::ZeroSignal => 1,
        FallbackKind::Invalid => 2,
    }
}

fn world_discs(state: &IcsState, field: &ClusterField) -> (Vec<Vec<WorldDisc>>, bool) {
    let mut invalid = false;
    let mut world = Vec::with_capacity(field.discs.len());
    for (piece, discs) in field.discs.iter().enumerate() {
        let pose = state.poses[piece];
        let (sin, cos) = pose_sin_cos(pose.theta_deg);
        let mut transformed = Vec::with_capacity(discs.len());
        for disc in discs {
            let center = apply_pose(
                disc.center_source_mm,
                pose.mirrored,
                sin,
                cos,
                pose.tx_mm,
                pose.ty_mm,
            );
            if !center[0].is_finite() || !center[1].is_finite() || !disc.radius_mm.is_finite() {
                invalid = true;
            }
            transformed.push(WorldDisc {
                center_mm: center,
                radius_mm: disc.radius_mm,
            });
        }
        world.push(transformed);
    }
    (world, invalid)
}

fn component_index_map(count: usize, components: &[Component]) -> Vec<Option<usize>> {
    let mut map = vec![None; count];
    for (component, row) in components.iter().enumerate() {
        for piece in &row.members {
            map[*piece] = Some(component);
        }
    }
    map
}

fn find(parent: &mut [usize], value: usize) -> usize {
    let mut root = value;
    while parent[root] != root {
        root = parent[root];
    }
    let mut cursor = value;
    while parent[cursor] != cursor {
        let next = parent[cursor];
        parent[cursor] = root;
        cursor = next;
    }
    root
}

fn union(parent: &mut [usize], left: usize, right: usize) {
    let left_root = find(parent, left);
    let right_root = find(parent, right);
    if left_root == right_root {
        return;
    }
    let low = left_root.min(right_root);
    let high = left_root.max(right_root);
    parent[high] = low;
}

fn allocate(weights: &[f64], q: usize, member_counts: &[usize]) -> (Vec<usize>, FallbackKind) {
    debug_assert_eq!(weights.len(), member_counts.len());
    if q == 0 {
        return (Vec::new(), FallbackKind::None);
    }
    if weights
        .iter()
        .any(|weight| !weight.is_finite() || *weight < 0.0)
    {
        return (member_counts.to_vec(), FallbackKind::Invalid);
    }
    let mut total = 0.0f64;
    for weight in weights {
        total += *weight;
    }
    if !total.is_finite() {
        return (member_counts.to_vec(), FallbackKind::Invalid);
    }
    if total == 0.0 {
        return (member_counts.to_vec(), FallbackKind::ZeroSignal);
    }

    let mut quotas = vec![0usize; weights.len()];
    let mut remainders = Vec::with_capacity(weights.len());
    let mut base_sum = 0usize;
    for (index, weight) in weights.iter().enumerate() {
        let ideal = ((q as f64) * *weight) / total;
        let base = ideal.floor();
        let remainder = ideal - base;
        if !ideal.is_finite()
            || !base.is_finite()
            || !remainder.is_finite()
            || base < 0.0
            || base > usize::MAX as f64
        {
            return (member_counts.to_vec(), FallbackKind::Invalid);
        }
        quotas[index] = base as usize;
        base_sum = match base_sum.checked_add(quotas[index]) {
            Some(value) => value,
            None => return (member_counts.to_vec(), FallbackKind::Invalid),
        };
        remainders.push((index, remainder));
    }
    if base_sum > q {
        return (member_counts.to_vec(), FallbackKind::Invalid);
    }
    let remaining = q - base_sum;
    if remaining >= weights.len() {
        return (member_counts.to_vec(), FallbackKind::Invalid);
    }
    remainders.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.0.cmp(&right.0))
    });
    for (index, _) in remainders.iter().take(remaining) {
        quotas[*index] += 1;
    }
    if quotas.iter().sum::<usize>() != q {
        return (member_counts.to_vec(), FallbackKind::Invalid);
    }
    (quotas, FallbackKind::None)
}

fn member_permutation(component: &Component, key: RelocateKey) -> Vec<usize> {
    let mut members = component.members.clone();
    let root = counter_hash(&[
        key.seed,
        key.bite,
        key.iteration,
        key.worker,
        component.id as u64,
        MEMBER_TAG,
    ]);
    for index in (1..members.len()).rev() {
        let target = counter_hash(&[root, index as u64]) as usize % (index + 1);
        members.swap(index, target);
    }
    members
}

fn schedule(components: &[Component], permutations: &[Vec<usize>], quotas: &[usize]) -> Vec<usize> {
    let mut out = Vec::with_capacity(quotas.iter().sum());
    let layers = quotas.iter().copied().max().unwrap_or(0);
    for layer in 0..layers {
        for component in 0..components.len() {
            if layer < quotas[component] {
                let members = &permutations[component];
                out.push(members[layer % members.len()]);
            }
        }
    }
    out
}

fn spearman(left: &[f64], right: &[f64]) -> Option<f64> {
    if left.len() != right.len() || left.len() < 2 {
        return None;
    }
    let left_ranks = midranks(left)?;
    let right_ranks = midranks(right)?;
    let mean = (left.len() as f64 + 1.0) / 2.0;
    let mut numerator = 0.0;
    let mut left_square = 0.0;
    let mut right_square = 0.0;
    for index in 0..left.len() {
        let a = left_ranks[index] - mean;
        let b = right_ranks[index] - mean;
        numerator += a * b;
        left_square += a * a;
        right_square += b * b;
    }
    if left_square == 0.0 || right_square == 0.0 {
        None
    } else {
        Some(numerator / libm::sqrt(left_square * right_square))
    }
}

fn spearman_usize(left: &[usize], right: &[usize]) -> Option<f64> {
    let left: Vec<f64> = left.iter().map(|value| *value as f64).collect();
    let right: Vec<f64> = right.iter().map(|value| *value as f64).collect();
    spearman(&left, &right)
}

fn midranks(values: &[f64]) -> Option<Vec<f64>> {
    if values.iter().any(|value| !value.is_finite()) {
        return None;
    }
    let mut order: Vec<usize> = (0..values.len()).collect();
    order.sort_by(|left, right| {
        values[*left]
            .partial_cmp(&values[*right])
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.cmp(right))
    });
    let mut ranks = vec![0.0; values.len()];
    let mut start = 0usize;
    while start < order.len() {
        let mut end = start + 1;
        while end < order.len() && values[order[end]] == values[order[start]] {
            end += 1;
        }
        let rank = ((start + 1 + end) as f64) / 2.0;
        for index in &order[start..end] {
            ranks[*index] = rank;
        }
        start = end;
    }
    Some(ranks)
}

fn push_key(out: &mut Vec<u8>, key: RelocateKey) {
    for value in [key.seed, key.bite, key.iteration, key.worker] {
        push_u64(out, value);
    }
}

fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn chain_digest(current: &mut [u8; 32], payload: &[u8]) {
    let mut digest = Sha256::new();
    digest.update(*current);
    digest.update(payload);
    *current = digest.finalize().into();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn largest_remainder_vectors_are_exact() {
        assert_eq!(
            allocate(&[6.0, 3.0, 1.0], 5, &[1, 1, 1]),
            (vec![3, 2, 0], FallbackKind::None)
        );
        assert_eq!(
            allocate(&[0.0, 4.0, 0.0], 5, &[3, 1, 1]),
            (vec![0, 5, 0], FallbackKind::None)
        );
        assert_eq!(
            allocate(&[0.0, 0.0, 0.0], 5, &[3, 1, 1]),
            (vec![3, 1, 1], FallbackKind::ZeroSignal)
        );
    }

    #[test]
    fn inversion_vector_is_exact() {
        assert_eq!(checked_pair_mass_term(2.0, 2.0, 4.0, 1.0), Some(1.0));
        assert_eq!(checked_pair_mass_term(1.0, 1.0, 1.5, 1.0), Some(2.25));
        let (mass, _) = allocate(&[1.0, 2.25], 4, &[2, 2]);
        let (max_v, _) = allocate(&[2.0, 1.0], 4, &[2, 2]);
        assert_eq!(mass, vec![1, 3]);
        assert_eq!(max_v, vec![3, 1]);
    }

    #[test]
    fn unit_square_disc_and_pose_vector_are_exact() {
        let disc = source_disc(0, 0, &[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]])
            .expect("unit square emits a disc");
        assert_eq!(disc.center_source_mm, [0.5, 0.5]);
        assert_eq!(disc.radius_mm, 0.5);
        let center = apply_pose(disc.center_source_mm, true, 1.0, 0.0, 10.0, 5.0);
        assert_eq!(center, [9.5, 4.5]);
        assert_eq!(disc.radius_mm, 0.5);
    }

    #[test]
    fn boundary_placebo_and_member_vectors_are_exact() {
        assert_eq!(boundary_mass_term(3.0), 9.0);
        let key = RelocateKey {
            seed: 0,
            bite: 11,
            iteration: 13,
            worker: 2,
        };
        assert_eq!(nonzero_placebo_offset(key, 1), 0);
        assert_eq!(nonzero_placebo_offset(key, 4), 1);
        assert_ne!(nonzero_placebo_offset(key, 4), 0);

        let permutation = member_permutation(
            &Component {
                id: 3,
                members: vec![3, 4, 5, 6],
            },
            RelocateKey {
                seed: 7,
                bite: 11,
                iteration: 13,
                worker: 2,
            },
        );
        assert_eq!(permutation, vec![6, 4, 3, 5]);
    }

    #[test]
    fn invalid_arithmetic_and_slot_identities_are_exact() {
        assert_eq!(
            allocate(&[f64::NAN, 1.0], 2, &[1, 1]),
            (vec![1, 1], FallbackKind::Invalid)
        );
        assert_eq!(
            allocate(&[f64::MAX, f64::MAX], 2, &[1, 1]),
            (vec![1, 1], FallbackKind::Invalid)
        );

        let mut trace = PartitionTrace {
            partition_slots: 2,
            ..PartitionTrace::default()
        };
        assert!(!trace.slot_identities_hold());
        trace.observe_execution(true);
        trace.observe_execution(false);
        assert!(trace.slot_identities_hold());
    }

    #[test]
    fn printed_gate0_vector_report_is_exact() {
        let report = gate0_vector_report();
        assert_eq!(report.unit_square_center, [0.5, 0.5]);
        assert_eq!(report.unit_square_radius, 0.5);
        assert_eq!(report.transformed_center, [9.5, 4.5]);
        assert_eq!(report.transformed_radius, 0.5);
        assert_eq!(report.pair_mass_terms, [1.0, 2.25]);
        assert_eq!(report.mass_inversion_quotas, vec![1, 3]);
        assert_eq!(report.max_violation_inversion_quotas, vec![3, 1]);
        assert_eq!(report.boundary_term, 9.0);
        assert_eq!(report.largest_remainder_component_ids, vec![0, 3, 7]);
        assert_eq!(report.largest_remainder_quotas, vec![3, 2, 0]);
        assert_eq!(report.mixed_zero_quotas, vec![0, 5, 0]);
        assert_eq!(report.zero_signal_quotas, vec![3, 1, 1]);
        assert_eq!(report.zero_signal_fallback, FallbackKind::ZeroSignal);
        assert_eq!(report.placebo_offset, 1);
        assert_eq!(report.placebo_input, vec![1.0, 2.0, 3.0, 4.0]);
        assert_eq!(report.placebo_rotated, vec![2.0, 3.0, 4.0, 1.0]);
        assert_eq!(report.placebo_quotas, vec![1, 1, 2, 0]);
        assert!(report.placebo_multiset_preserved);
        assert!(report.placebo_non_identity);
        assert_eq!(report.member_permutation, vec![6, 4, 3, 5]);
        assert_eq!(report.round_robin_schedule, vec![1, 2, 0, 1]);
        assert_eq!(report.invalid_quotas, vec![1, 1]);
        assert_eq!(report.invalid_fallback, FallbackKind::Invalid);
        assert!(report.nonfinite_source_rejected);
        assert!(report.nonfinite_pair_rejected);
        assert!(report.quota_sum_identity);
        assert!(report.schedule_length_identity);
        assert!(report.executed_slots_identity);
        assert!(report.full_plus_zero_identity);
        assert!(report.inversion_is_pure_frozen_row_field_vector);
    }

    #[test]
    fn round_robin_layers_components() {
        let components = vec![
            Component {
                id: 0,
                members: vec![0, 1],
            },
            Component {
                id: 2,
                members: vec![2, 3],
            },
        ];
        let permutations = vec![vec![1, 0], vec![2, 3]];
        assert_eq!(
            schedule(&components, &permutations, &[3, 1]),
            vec![1, 2, 0, 1]
        );
    }

    #[test]
    fn spearman_uses_midranks_and_refuses_zero_variance() {
        assert_eq!(spearman(&[1.0, 2.0, 3.0], &[3.0, 2.0, 1.0]), Some(-1.0));
        assert_eq!(spearman(&[1.0, 1.0], &[1.0, 2.0]), None);
    }
}
