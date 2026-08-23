//! **Disruption: swap two large pieces, and carry whoever was inside them.**
//!
//! Algorithm 12's fail path of arXiv:2509.13329, as read at Sparrow
//! `optimizer/explore.rs::disrupt_solution` and `practically_contained_items`
//! (rev `14f4868f`), implemented on our poses and our source rings. No source
//! text is copied.
//!
//! **This is not the routine move and it is not a stall handler.** It fires on
//! a *failed separation* inside exploration - the loop persists at the same `W`,
//! draws a least-infeasible snapshot from its pool, disrupts, and separates
//! again. The one disruption in Sparrow's own 10 s mixed-61 log did not produce
//! their result; it is the escape hatch, not the engine. The old strip/ball
//! topology jump this replaces fired on a *stalled sweep*, which is a different
//! trigger for a different purpose and is gone.
//!
//! Three decisions here are arbitrations rather than readings, and each is
//! marked at its site:
//!
//! 1. **followers are found by a guaranteed interior witness**, not by an area
//!    centroid (arbitration 1 of docs/cutclose-relocate-spec.md). Their test is
//!    "this piece's pole of inaccessibility is inside the swapped shape"; a POI
//!    is interior by construction and a nonconvex piece's area centroid is not.
//! 2. **the distinctness test is `AND`**, not `OR`: the second piece is
//!    preferred only when both its area and its diameter differ from the first
//!    by more than 1 %, with any distinct piece as the fallback. Sol review 17
//!    Round 2 §1 checked the recorded binary against Grok's round-1 reading and
//!    the source says `AND`.
//! 3. **followers are capped** at `n`, so a defect in the containment test can
//!    move at most the layout and never more (Grok review 12 Round 1 §2.1).

use super::descent::counter_hash;
use super::diagnostics::WorkVector;
use super::energy::rebuild_all;
use super::state::{
    apply_pose, pose_sin_cos, transform_piece, Contract, IcsState, PieceSource, Pose,
};

/// The cumulative convex-hull-area percentile that defines a "large" piece.
/// Sparrow `config.rs`'s `large_item_ch_area_cutoff_percentile`, 0.75.
pub const LARGE_ITEM_CH_AREA_CUTOFF: f64 = 0.75;

/// The relative tolerance of the distinctness test, 1 %.
pub const DISTINCTNESS_TOLERANCE: f64 = 0.01;

/// A domain tag so a disruption draw cannot collide with a sample stream.
const DISRUPT_STREAM_TAG: u64 = 0x4453_5250_5F53_5750; // "DSRP_SWP"

/// What one disruption did.
#[derive(Clone, Debug, PartialEq)]
pub struct DisruptOutcome {
    /// `false` when the layout has fewer than two pieces, or every candidate
    /// pair was rejected. Nothing was moved.
    pub fired: bool,
    /// The two swapped pieces, in the order they were chosen.
    pub swapped: Option<(usize, usize)>,
    /// `true` when the second piece came from the `AND` distinctness filter and
    /// `false` when it came from the any-distinct fallback. Reported because
    /// "the fallback fires every time" is a fact about the request, not a bug.
    pub distinct: bool,
    /// The followers that were carried, and by which swapped piece's map.
    pub followers: Vec<usize>,
    /// Followers dropped because the cap was reached.
    pub followers_capped: usize,
}

impl DisruptOutcome {
    fn idle() -> Self {
        Self {
            fired: false,
            swapped: None,
            distinct: false,
            followers: Vec::new(),
            followers_capped: 0,
        }
    }
}

/// The set of "large" pieces: those whose convex-hull area is at or above the
/// cutoff.
///
/// The cutoff is found by walking the pieces in descending hull area and
/// accumulating until the running sum passes `0.75` of the total; the hull area
/// of the piece that *caused* the excess is the cutoff. That is their
/// construction exactly, including the detail that makes it a percentile of
/// **area** rather than of count. Ties in hull area are broken by input index,
/// so the set is a pure function of the request.
pub fn large_pieces(sources: &[PieceSource]) -> Vec<usize> {
    if sources.is_empty() {
        return Vec::new();
    }
    let mut total = 0.0f64;
    for source in sources {
        total += source.convex_hull_area_mm2;
    }
    let threshold = total * LARGE_ITEM_CH_AREA_CUTOFF;
    let mut ordered: Vec<usize> = (0..sources.len()).collect();
    ordered.sort_by(|left, right| {
        sources[*right]
            .convex_hull_area_mm2
            .partial_cmp(&sources[*left].convex_hull_area_mm2)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(left.cmp(right))
    });
    let mut cumulative = 0.0f64;
    let mut cutoff = 0.0f64;
    for piece in &ordered {
        cumulative += sources[*piece].convex_hull_area_mm2;
        if cumulative > threshold {
            cutoff = sources[*piece].convex_hull_area_mm2;
            break;
        }
    }
    (0..sources.len())
        .filter(|piece| sources[*piece].convex_hull_area_mm2 >= cutoff)
        .collect()
}

/// Their `approx_eq!(area, epsilon = area * 0.01)` pair, negated and joined by
/// `AND`: the second piece must differ from the first in **both** area and
/// diameter by more than 1 % of the first's.
pub fn is_distinct_enough(first: &PieceSource, second: &PieceSource) -> bool {
    let area_gap = (second.area_mm2 - first.area_mm2).abs();
    let diameter_gap = (second.diameter_mm - first.diameter_mm).abs();
    area_gap > first.area_mm2.abs() * DISTINCTNESS_TOLERANCE
        && diameter_gap > first.diameter_mm.abs() * DISTINCTNESS_TOLERANCE
}

/// A piece's interior witness, transformed by its current pose.
///
/// This is the point the follower test asks about, and it is inside the
/// material by construction ([`super::decomposition::interior_witness`]).
pub fn transformed_witness(source: &PieceSource, pose: Pose) -> [f64; 2] {
    let (sin, cos) = pose_sin_cos(pose.theta_deg);
    apply_pose(
        source.interior_witness,
        pose.mirrored,
        sin,
        cos,
        pose.tx_mm,
        pose.ty_mm,
    )
}

/// Whether a point is inside a transformed outer ring, by the even-odd crossing
/// rule.
///
/// Deterministic and boundary-consistent: the half-open `(a.y <= y) != (b.y <=
/// y)` test counts each crossing exactly once, so a point on a horizontal ray
/// through a vertex is not double counted. A point exactly on an edge may fall
/// either way; that is acceptable here because the answer only decides whether
/// a piece is carried along by a disruption, and a disruption is already a
/// deliberate perturbation.
pub fn point_in_ring(point: [f64; 2], ring: &[[f64; 2]]) -> bool {
    let mut inside = false;
    let count = ring.len();
    for index in 0..count {
        let a = ring[index];
        let b = ring[(index + 1) % count];
        if (a[1] <= point[1]) != (b[1] <= point[1]) {
            let denominator = b[1] - a[1];
            if denominator != 0.0 {
                let crossing = a[0] + (point[1] - a[1]) / denominator * (b[0] - a[0]);
                if crossing > point[0] {
                    inside = !inside;
                }
            }
        }
    }
    inside
}

/// The rigid map that takes the *old* pose of a swapped piece to its *new* one,
/// applied to another piece's pose.
///
/// Their `dt_new.compose().inverse().transform(&dt_old.compose())` composed onto
/// a follower is exactly "put the follower where it would be if the whole
/// neighbourhood had been carried by the swap". Written on our poses: the
/// rotation adds, and the translation is the follower's position expressed
/// relative to the old pose's frame and re-planted in the new one.
///
/// Mirror is frozen - it is not a Sparrow move and the converter never encoded
/// one - so a follower keeps its own mirror flag and the map is the
/// mirror-agnostic rigid one.
pub fn carry(follower: Pose, from: Pose, to: Pose) -> Pose {
    let dtheta = to.theta_deg - from.theta_deg;
    let (sin, cos) = pose_sin_cos(dtheta);
    let arm = [follower.tx_mm - from.tx_mm, follower.ty_mm - from.ty_mm];
    Pose {
        tx_mm: to.tx_mm + (arm[0] * cos - arm[1] * sin),
        ty_mm: to.ty_mm + (arm[0] * sin + arm[1] * cos),
        theta_deg: follower.theta_deg + dtheta,
        mirrored: follower.mirrored,
    }
}

/// Maps a swapped angle onto the receiving piece's allowed set.
///
/// `sample/uniform_sampler.rs::convert_sample_to_closest_feasible`: a
/// continuously rotatable piece keeps the sampled angle; a frozen piece keeps
/// its own. This engine's rotation surface is exactly those two cases -
/// `allow_rotation` is a per-piece boolean and there is no discrete catalogue -
/// so the map is total.
#[inline]
pub fn closest_feasible_angle(allow_rotation: bool, sampled_deg: f64, own_deg: f64) -> f64 {
    if allow_rotation {
        sampled_deg
    } else {
        own_deg
    }
}

/// **One disruption.**
///
/// 1. Pick the first piece uniformly from the large set.
/// 2. Pick the second uniformly from the large pieces that differ from the
///    first in **both** area and diameter by more than 1 %; if there are none,
///    from any piece that is not the first.
/// 3. Swap their poses, mapping each angle through the receiving piece's own
///    allowed set.
/// 4. Move every piece whose transformed interior witness lies inside a
///    swapped piece's ring **at its new pose** into the space that piece
///    vacated, by the rigid map that takes its new frame back to its old one -
///    capped at `n` moved pieces.
///
/// **Step 4's direction is the source's, and it is not the obvious one.**
/// `optimizer/explore.rs::disrupt_solution` calls `practically_contained_items`
/// *after* both `move_item` calls, so the containment question is asked of the
/// layout the swap produced; and its map is
/// `dt_new.compose().inverse().transform(&dt_old.compose())`, which in
/// `jagua_rs`'s convention (`a.transform(&b)` is `b . a`) is `T_old . T_new^-1`
/// - new frame to old frame. Their comment says why: "the huge item will create
/// a large empty space and many of the items which previously surrounded the
/// smaller one will be contained by the huge one", and those items are sent to
/// the empty space. Carrying the *old* footprint's occupants forward instead
/// would move the few pieces that were inside a large piece's own material into
/// the small hole it swapped into, which is the opposite operator.
///
/// The two follower blocks run in sequence, as theirs do: the second reads the
/// layout the first left behind, so a piece the first block moved into the
/// second's new footprint is carried again. That is a consequence of their
/// ordering rather than a rule of ours, and reproducing it is cheaper than
/// inventing a conflict rule.
///
/// Every cache is rebuilt before returning, so the caller receives a state it
/// can separate immediately. Weights are **not** touched: they are the
/// landscape, and a disruption is a move inside it.
pub fn disrupt(
    state: &mut IcsState,
    sources: &[PieceSource],
    contract: &Contract,
    allow_rotation: &[bool],
    seed: u64,
    bite: u64,
    attempt: u64,
    work: &mut WorkVector,
) -> DisruptOutcome {
    let count = state.poses.len();
    if count < 2 {
        return DisruptOutcome::idle();
    }
    let large = large_pieces(sources);
    let root = counter_hash(&[seed, bite, attempt, DISRUPT_STREAM_TAG]);
    let pool: &[usize] = if large.is_empty() { &[] } else { &large };
    let first = if pool.is_empty() {
        (counter_hash(&[root, 0]) % count as u64) as usize
    } else {
        pool[(counter_hash(&[root, 0]) % pool.len() as u64) as usize]
    };
    let distinct_candidates: Vec<usize> = large
        .iter()
        .copied()
        .filter(|piece| *piece != first && is_distinct_enough(&sources[first], &sources[*piece]))
        .collect();
    let (second, distinct) = if !distinct_candidates.is_empty() {
        (
            distinct_candidates
                [(counter_hash(&[root, 1]) % distinct_candidates.len() as u64) as usize],
            true,
        )
    } else {
        let fallback: Vec<usize> = (0..count).filter(|piece| *piece != first).collect();
        (
            fallback[(counter_hash(&[root, 2]) % fallback.len() as u64) as usize],
            false,
        )
    };

    let first_old = state.poses[first];
    let second_old = state.poses[second];
    let first_new = Pose {
        tx_mm: second_old.tx_mm,
        ty_mm: second_old.ty_mm,
        theta_deg: closest_feasible_angle(
            allow_rotation[first],
            second_old.theta_deg,
            first_old.theta_deg,
        ),
        mirrored: first_old.mirrored,
    };
    let second_new = Pose {
        tx_mm: first_old.tx_mm,
        ty_mm: first_old.ty_mm,
        theta_deg: closest_feasible_angle(
            allow_rotation[second],
            first_old.theta_deg,
            second_old.theta_deg,
        ),
        mirrored: second_old.mirrored,
    };

    // The swap first, because the containment question is asked of the layout
    // the swap produced.
    let mut moved: Vec<usize> = vec![first, second];
    state.poses[first] = first_new;
    state.poses[second] = second_new;
    transform_piece(sources, &mut state.geometry, &state.poses, first);
    transform_piece(sources, &mut state.geometry, &state.poses, second);
    work.pose_transforms += 2;

    let mut followers: Vec<usize> = Vec::new();
    let mut capped = 0usize;
    for (host, from, to) in [
        (first, first_new, first_old),
        (second, second_new, second_old),
    ] {
        // Collected here, inside the loop, so the second host sees what the
        // first host's block left behind.
        for follower in witnesses_inside(state, sources, host, &[first, second]) {
            // The cap: a defect in the containment test can move at most the
            // layout, never more than it. A piece both hosts claim counts once.
            if !moved.contains(&follower) {
                if moved.len() >= count {
                    capped += 1;
                    continue;
                }
                moved.push(follower);
                followers.push(follower);
            }
            state.poses[follower] = carry(state.poses[follower], from, to);
            transform_piece(sources, &mut state.geometry, &state.poses, follower);
            work.pose_transforms += 1;
        }
    }

    rebuild_all(state, contract, work);
    work.disruptions += 1;
    work.disruption_moves += moved.len() as u64;
    DisruptOutcome {
        fired: true,
        swapped: Some((first, second)),
        distinct,
        followers,
        followers_capped: capped,
    }
}

/// Every piece whose transformed interior witness lies inside `host`'s
/// transformed outer ring, in input order.
fn witnesses_inside(
    state: &IcsState,
    sources: &[PieceSource],
    host: usize,
    exclude: &[usize],
) -> Vec<usize> {
    let ring = state.geometry.ring_slice(host).to_vec();
    let host_bounds = state.geometry.piece_bounds[host];
    let mut out = Vec::new();
    for piece in 0..state.poses.len() {
        if piece == host || exclude.contains(&piece) {
            continue;
        }
        let witness = transformed_witness(&sources[piece], state.poses[piece]);
        if witness[0] < host_bounds[0]
            || witness[0] > host_bounds[2]
            || witness[1] < host_bounds[1]
            || witness[1] > host_bounds[3]
        {
            continue;
        }
        if point_in_ring(witness, &ring) {
            out.push(piece);
        }
    }
    out
}
