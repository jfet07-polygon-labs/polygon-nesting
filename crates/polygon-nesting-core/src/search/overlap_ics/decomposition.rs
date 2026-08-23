//! Deterministic source-material decomposition into convex cells.
//!
//! Two rules, both from the converged spec (Sol R2 §4, Grok R2 §4):
//!
//! * **holes are an explicit error**, never silently filled. Round 1 supports
//!   the simple outer-ring population mixed-61, shapes-17 and triangle-20 are
//!   made of; a constrained triangulation of a region with holes is a
//!   transfer-round requirement and a wrong answer here would be invisible.
//! * **a convex piece is one cell.** Only a nonconvex ring is triangulated.
//!   The reason is in Grok R2 §1.1: the maximum triangle-cell penetration
//!   *under*-estimates the whole piece's minimum translation vector, so
//!   decomposing a convex piece would weaken Φ for no gain. On mixed-61 this
//!   is 52 of 61 pieces; on triangle-20 it is all 20.
//!
//! The ear clip is our own, deliberately: `general_relaxed`'s private clipper
//! is not imported (Sol review 14 §1, Grok R2 §1.1 both say so), and neither is
//! any offset or Clipper path. The input is [`PolygonRing::source_points`] -
//! the untouched `f64` ring, not the canonical integer grid - because Φ is a
//! statement about material and the publication judge is the only thing in this
//! engine that is allowed to see the 1 µm grid.

use crate::geometry::general_polygon::PolygonSet;

/// One convex cell of a piece, as a contiguous vertex range in the piece's
/// packed source-point array.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub start: usize,
    pub len: usize,
}

/// A piece's decomposition, in its own source frame.
#[derive(Clone, Debug)]
pub struct Decomposition {
    /// Every cell's vertices, packed back to back, counter-clockwise.
    pub points: Vec<[f64; 2]>,
    pub cells: Vec<Cell>,
    /// The outer ring itself, counter-clockwise, in source coordinates. This is
    /// what the depth and the boundary residuals are measured on, and what the
    /// publication path re-transforms.
    pub ring: Vec<[f64; 2]>,
    /// `true` when the ring is convex and therefore *is* the single cell.
    pub convex: bool,
}

impl Decomposition {
    pub fn cell_points(&self, cell: Cell) -> &[[f64; 2]] {
        &self.points[cell.start..cell.start + cell.len]
    }
}

/// The counter-clockwise source ring of a single-region, hole-free
/// [`PolygonSet`].
///
/// Errors - never warnings, never repairs - on holes, on multiple regions, and
/// on a non-finite or degenerate ring.
pub fn source_ring(polygon: &PolygonSet) -> Result<Vec<[f64; 2]>, String> {
    let regions = polygon.regions();
    if regions.len() != 1 {
        return Err(format!(
            "the overlap-ICS decomposition supports exactly one material region in Round 1, not {}",
            regions.len()
        ));
    }
    let region = &regions[0];
    if !region.holes.is_empty() {
        return Err(
            "the overlap-ICS decomposition refuses a region with holes in Round 1; it does not fill them".to_owned(),
        );
    }
    let mut ring = Vec::with_capacity(region.outer.source_points().len());
    for point in region.outer.source_points() {
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err(
                "the overlap-ICS decomposition refuses a non-finite source vertex".to_owned(),
            );
        }
        ring.push([point.x, point.y]);
    }
    if ring.len() < 3 {
        return Err(
            "a decomposable ring needs at least three vertices".to_owned(),
        );
    }
    if signed_area(&ring) < 0.0 {
        ring.reverse();
    }
    if signed_area(&ring) <= 0.0 {
        return Err(
            "a decomposable ring must enclose positive area".to_owned(),
        );
    }
    Ok(ring)
}

/// Twice the signed area of a ring: positive counter-clockwise.
///
/// Summed in index order and never reassociated, so it is a fixed-order fold
/// like every other scalar in this module.
pub fn signed_area(ring: &[[f64; 2]]) -> f64 {
    let mut total = 0.0;
    for index in 0..ring.len() {
        let first = ring[index];
        let second = ring[(index + 1) % ring.len()];
        total += first[0] * second[1] - second[0] * first[1];
    }
    total / 2.0
}

/// Whether a counter-clockwise ring is convex, with collinear vertices allowed.
pub fn is_convex(ring: &[[f64; 2]]) -> bool {
    let count = ring.len();
    if count < 3 {
        return false;
    }
    for index in 0..count {
        let a = ring[index];
        let b = ring[(index + 1) % count];
        let c = ring[(index + 2) % count];
        if cross(a, b, c) < 0.0 {
            return false;
        }
    }
    true
}

fn cross(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

/// The full decomposition of one piece: one cell when convex, a deterministic
/// ear clip otherwise.
pub fn decompose(polygon: &PolygonSet) -> Result<Decomposition, String> {
    let ring = source_ring(polygon)?;
    if is_convex(&ring) {
        return Ok(Decomposition {
            points: ring.clone(),
            cells: vec![Cell {
                start: 0,
                len: ring.len(),
            }],
            ring,
            convex: true,
        });
    }
    let triangles = ear_clip(&ring)?;
    let mut points = Vec::with_capacity(triangles.len() * 3);
    let mut cells = Vec::with_capacity(triangles.len());
    for triangle in &triangles {
        cells.push(Cell {
            start: points.len(),
            len: 3,
        });
        for index in triangle {
            points.push(ring[*index]);
        }
    }
    Ok(Decomposition {
        points,
        cells,
        ring,
        convex: false,
    })
}

/// A deterministic ear clip of a counter-clockwise simple ring.
///
/// The scan is the plain one and the tie rule is the whole point: indices are
/// visited in increasing order from the position after the last clip, and the
/// **first** admissible ear wins. No angle heuristic, no area heuristic, no
/// randomness - the same ring produces the same triangles on every run, which
/// is what makes Φ replayable.
pub fn ear_clip(ring: &[[f64; 2]]) -> Result<Vec<[usize; 3]>, String> {
    let count = ring.len();
    if count < 3 {
        return Err(
            "an ear clip needs at least three vertices".to_owned(),
        );
    }
    let mut remaining: Vec<usize> = (0..count).collect();
    let mut triangles = Vec::with_capacity(count.saturating_sub(2));
    let mut cursor = 0usize;
    let mut misses = 0usize;
    while remaining.len() > 3 {
        let size = remaining.len();
        let previous = remaining[(cursor + size - 1) % size];
        let current = remaining[cursor % size];
        let next = remaining[(cursor + 1) % size];
        if is_ear(ring, &remaining, previous, current, next) {
            triangles.push([previous, current, next]);
            remaining.remove(cursor % size);
            if cursor >= remaining.len() {
                cursor = 0;
            }
            misses = 0;
        } else {
            cursor = (cursor + 1) % size;
            misses += 1;
            if misses > size {
                return Err(
                    "the overlap-ICS ear clip found no admissible ear; the ring is not simple".to_owned(),
                );
            }
        }
    }
    triangles.push([remaining[0], remaining[1], remaining[2]]);
    triangles.retain(|triangle| {
        cross(ring[triangle[0]], ring[triangle[1]], ring[triangle[2]]) > 0.0
    });
    if triangles.is_empty() {
        return Err(
            "the overlap-ICS ear clip produced no positive-area triangle".to_owned(),
        );
    }
    Ok(triangles)
}

fn is_ear(
    ring: &[[f64; 2]],
    remaining: &[usize],
    previous: usize,
    current: usize,
    next: usize,
) -> bool {
    let a = ring[previous];
    let b = ring[current];
    let c = ring[next];
    if cross(a, b, c) <= 0.0 {
        return false;
    }
    for index in remaining {
        if *index == previous || *index == current || *index == next {
            continue;
        }
        if point_in_triangle(ring[*index], a, b, c) {
            return false;
        }
    }
    true
}

fn point_in_triangle(point: [f64; 2], a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> bool {
    cross(a, b, point) > 0.0 && cross(b, c, point) > 0.0 && cross(c, a, point) > 0.0
}

/// The polygon's minimum width over all rotations: the smallest distance
/// between two parallel supporting lines.
///
/// This is a *lower* bound on how much of the strip's depth the piece must
/// occupy whatever angle it is placed at, and that is why the strip's own lower
/// scale `L` is allowed to use it. `min(bbox width, bbox height)` would be an
/// over-estimate of the same quantity and therefore an unsafe bound.
///
/// Rotating calipers over the ring's own hull would need the hull; on a convex
/// ring this loop *is* the calipers, and on a nonconvex ring taking the maximum
/// vertex distance from each edge line still returns a width no larger than the
/// hull's, so the bound stays safe.
pub fn minimum_width(ring: &[[f64; 2]]) -> f64 {
    let count = ring.len();
    let mut best = f64::INFINITY;
    for index in 0..count {
        let a = ring[index];
        let b = ring[(index + 1) % count];
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let length = libm::hypot(dx, dy);
        if length <= 0.0 {
            continue;
        }
        let mut widest: f64 = 0.0;
        for point in ring {
            let distance = ((point[0] - a[0]) * dy - (point[1] - a[1]) * dx) / length;
            widest = widest.max(distance.abs());
        }
        best = best.min(widest);
    }
    if best.is_finite() {
        best
    } else {
        0.0
    }
}

/// A **guaranteed interior witness** of the material: the area centroid of the
/// first positive-area cell of the decomposition.
///
/// Arbitration 1 of docs/cutclose-relocate-spec.md. Sparrow's disruption
/// follower test asks whether a piece's *pole of inaccessibility* lies inside a
/// swapped shape (`optimizer/explore.rs::practically_contained_items`, rev
/// `14f4868f`); a POI is by construction interior. Grok review 12 Round 1 §2.1
/// proposed the piece's area centroid instead and flagged the gap himself: the
/// area centroid of a nonconvex ring can lie **outside** the material, so a
/// centroid-in-ring follower test both misses real followers and moves pieces
/// that are not inside anything.
///
/// We do not have a POI and we are not porting one. What we do have is the
/// deterministic ear clip above, and a cell of it is convex with positive area
/// by construction ([`ear_clip`] retains only positive-area triangles), so the
/// cell's own area centroid is strictly inside that cell and therefore strictly
/// inside the material. "First" is the ear clip's own emission order, which is
/// a pure function of the ring - so this witness is as replayable as Φ is.
///
/// For a convex piece the single cell *is* the ring and this is the ring
/// centroid, which is interior because the ring is convex.
pub fn interior_witness(decomposition: &Decomposition) -> [f64; 2] {
    for cell in &decomposition.cells {
        let points = decomposition.cell_points(*cell);
        if signed_area(points) > 0.0 {
            return centroid(points);
        }
    }
    // Unreachable for a decomposition this module produced: `decompose` errors
    // rather than return a cell set with no positive-area cell. The fallback is
    // the ring centroid so that a future decomposer cannot make this panic.
    centroid(&decomposition.ring)
}

/// The counter-clockwise convex hull of a ring, by monotone chain.
///
/// Deterministic: points are ordered by `(x, y)` with a total order that breaks
/// `f64` ties by index, and the turn test is the same `cross` the ear clip uses.
/// Collinear points are dropped (`<= 0.0`), so the hull is the minimal vertex
/// set.
pub fn convex_hull(ring: &[[f64; 2]]) -> Vec<[f64; 2]> {
    if ring.len() < 3 {
        return ring.to_vec();
    }
    let mut ordered: Vec<[f64; 2]> = ring.to_vec();
    ordered.sort_by(|left, right| {
        left[0]
            .partial_cmp(&right[0])
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                left[1]
                    .partial_cmp(&right[1])
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });
    ordered.dedup_by(|left, right| left[0] == right[0] && left[1] == right[1]);
    if ordered.len() < 3 {
        return ordered;
    }
    let mut hull: Vec<[f64; 2]> = Vec::with_capacity(ordered.len() * 2);
    for point in ordered.iter() {
        while hull.len() >= 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], *point) <= 0.0 {
            hull.pop();
        }
        hull.push(*point);
    }
    let lower = hull.len() + 1;
    for point in ordered.iter().rev().skip(1) {
        while hull.len() >= lower && cross(hull[hull.len() - 2], hull[hull.len() - 1], *point) <= 0.0
        {
            hull.pop();
        }
        hull.push(*point);
    }
    hull.pop();
    hull
}

/// The area of the ring's convex hull: the "large item" measure of Sparrow's
/// disruption (`optimizer/explore.rs::disrupt_solution`, which reads
/// `surrogate().convex_hull_area`).
pub fn convex_hull_area(ring: &[[f64; 2]]) -> f64 {
    signed_area(&convex_hull(ring))
}

/// The ring's diameter: the largest distance between any two of its points.
///
/// The two farthest points are always hull vertices, so this is the hull's own
/// pairwise maximum. `jagua_rs`'s `SPolygon::calculate_diameter` (which Sparrow
/// reads through `shape.diameter`) uses the same fact; the loop is ours.
pub fn diameter(ring: &[[f64; 2]]) -> f64 {
    let hull = convex_hull(ring);
    let mut worst = 0.0f64;
    for (index, first) in hull.iter().enumerate() {
        for second in &hull[index + 1..] {
            worst = worst.max(libm::hypot(second[0] - first[0], second[1] - first[1]));
        }
    }
    worst
}

/// The axis-aligned bounds of a ring in its own source frame,
/// `[min x, min y, max x, max y]`.
pub fn ring_bounds(ring: &[[f64; 2]]) -> [f64; 4] {
    let mut out = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for point in ring {
        out[0] = out[0].min(point[0]);
        out[1] = out[1].min(point[1]);
        out[2] = out[2].max(point[0]);
        out[3] = out[3].max(point[1]);
    }
    out
}

/// The area centroid of a counter-clockwise ring, in source coordinates.
pub fn centroid(ring: &[[f64; 2]]) -> [f64; 2] {
    let count = ring.len();
    let mut area2 = 0.0;
    let mut cx = 0.0;
    let mut cy = 0.0;
    for index in 0..count {
        let first = ring[index];
        let second = ring[(index + 1) % count];
        let cross = first[0] * second[1] - second[0] * first[1];
        area2 += cross;
        cx += (first[0] + second[0]) * cross;
        cy += (first[1] + second[1]) * cross;
    }
    if area2 == 0.0 {
        let mut sx = 0.0;
        let mut sy = 0.0;
        for point in ring {
            sx += point[0];
            sy += point[1];
        }
        return [sx / count as f64, sy / count as f64];
    }
    [cx / (3.0 * area2), cy / (3.0 * area2)]
}
