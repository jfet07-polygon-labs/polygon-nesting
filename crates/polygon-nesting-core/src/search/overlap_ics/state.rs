//! The ICS state: continuous poses, SoA transformed geometry, cached pair and
//! boundary rows, and a protected exact incumbent that the search cannot touch.
//!
//! Two things this file exists to keep apart, because the campaign's previous
//! failures came from mixing them:
//!
//! * **the current state is allowed to be infeasible, forever.** It carries no
//!   validity flag and no exact verdict. Nothing in the descent asks the exact
//!   geometry whether a move is legal.
//! * **`best_exact` is only ever written by `publish.rs`,** after both exact
//!   authorities have accepted. A failed publication attempt leaves it exactly
//!   as it was, and the deadline returns it.
//!
//! The pose is continuous `f64` and is **never** snapped. The only
//! quantization in the whole engine is `GridSet::of` at publication, which is
//! the arbitration in docs/overlap-ics-converged-spec.md.

use crate::geometry::general_polygon::PolygonSet;
use crate::search::general_fast::{GeneralFastPiece, GeneralFastPlacement, GeneralFastSettings};

use super::contact::{bounds, Contact};
use super::decomposition::{self, Cell, Decomposition};

/// A continuous rigid pose over the source material.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub tx_mm: f64,
    pub ty_mm: f64,
    /// Accumulates over the whole circle. There is no angle catalogue and no
    /// local window; Round 1 freezes only the mirror.
    pub theta_deg: f64,
    pub mirrored: bool,
}

impl Pose {
    pub fn rotation_deg(self) -> f64 {
        self.theta_deg
    }
}

/// The `(sin, cos)` the *publication* path will compute for this pose.
///
/// The angle coordinate is carried in **degrees**, and that is not a style
/// choice. `PolygonSet::transformed` and
/// `validation::general_polygon::placement_rotation` both derive their sine and
/// cosine from `rotation_deg.to_radians()`; if the search carried radians and
/// converted on the way out, the geometry Φ measures would differ from the
/// geometry the two exact authorities judge in the last bits, and an imported
/// pose set (S0) would not even reproduce its own depth. Degrees make the
/// identity exact by construction while leaving the coordinate continuous and
/// unbounded, which is all the spec asks of it - no catalogue, no window, no
/// 2.5° step.
///
/// Sol R2 §4 asks for `libm` trigonometry for cross-version determinism.
/// Identity with the publication transform is the stronger requirement and is
/// the one taken here; the determinism contract already carries the libm
/// implementation in its environment tuple.
#[inline]
pub fn pose_sin_cos(theta_deg: f64) -> (f64, f64) {
    theta_deg.to_radians().sin_cos()
}

#[inline]
pub fn apply_pose(point: [f64; 2], mirrored: bool, sin: f64, cos: f64, tx: f64, ty: f64) -> [f64; 2] {
    let mirror_x = if mirrored { -point[0] } else { point[0] };
    [
        mirror_x * cos - point[1] * sin + tx,
        mirror_x * sin + point[1] * cos + ty,
    ]
}

/// The request's own clearance contract, in the engine's units.
#[derive(Clone, Copy, Debug)]
pub struct Contract {
    pub sheet_short_axis_mm: f64,
    pub sheet_long_axis_mm: f64,
    pub total_padding_mm: f64,
    pub sheet_edge_clearance_mm: f64,
    pub flattening_sag_tolerance_mm: f64,
    pub clearance_safety_margin_mm: f64,
}

impl Contract {
    pub fn from_settings(settings: GeneralFastSettings) -> Self {
        Self {
            sheet_short_axis_mm: settings.sheet_short_axis_mm,
            sheet_long_axis_mm: settings.sheet_long_axis_mm,
            total_padding_mm: settings.total_padding_mm,
            sheet_edge_clearance_mm: settings
                .sheet_edge_clearance_mm
                .unwrap_or(settings.total_padding_mm / 2.0),
            flattening_sag_tolerance_mm: settings.flattening_sag_tolerance_mm,
            clearance_safety_margin_mm: settings.clearance_safety_margin_mm,
        }
    }

    /// `c_pair = total_padding + 2 * sag` — the material contract's own pair
    /// clearance, read off `validate_publication` rather than invented.
    pub fn pair_clearance_mm(&self) -> f64 {
        self.total_padding_mm + 2.0 * self.flattening_sag_tolerance_mm
    }

    /// The material contract's own sheet clearance.
    pub fn edge_clearance_mm(&self) -> f64 {
        self.sheet_edge_clearance_mm + self.flattening_sag_tolerance_mm
    }

    /// The round kernel's radius at **allowance zero**. The search-offset
    /// allowance is search-only and never part of publication legality, so it
    /// is not a field of this struct at all.
    pub fn expansion_mm(&self) -> f64 {
        self.total_padding_mm / 2.0 + self.clearance_safety_margin_mm
    }

    pub fn sheet_inset_mm(&self) -> f64 {
        self.sheet_edge_clearance_mm - self.total_padding_mm / 2.0
    }
}

/// One piece's immutable source-frame data.
#[derive(Clone, Debug)]
pub struct PieceSource {
    pub id: String,
    pub decomposition: Decomposition,
    pub centroid: [f64; 2],
    /// The largest source-vertex radius from the centroid: the `R_i` of the
    /// SE(2) metric in the descent.
    pub max_radius_mm: f64,
    pub area_mm2: f64,
    pub min_width_mm: f64,
}

impl PieceSource {
    pub fn of(id: &str, polygon: &PolygonSet) -> Result<Self, String> {
        let decomposition =
            decomposition::decompose(polygon).map_err(|error| format!("piece {id}: {error}"))?;
        let centroid = decomposition::centroid(&decomposition.ring);
        let mut max_radius: f64 = 0.0;
        for point in &decomposition.ring {
            max_radius = max_radius.max(libm::hypot(
                point[0] - centroid[0],
                point[1] - centroid[1],
            ));
        }
        Ok(Self {
            id: id.to_owned(),
            area_mm2: decomposition::signed_area(&decomposition.ring),
            min_width_mm: decomposition::minimum_width(&decomposition.ring),
            decomposition,
            centroid,
            max_radius_mm: max_radius,
        })
    }
}

/// The `n(n-1)/2` pair rows, in the fixed pair order `(0,1), (0,2), ..., (n-2,n-1)`.
#[inline]
pub fn pair_index(count: usize, first: usize, second: usize) -> usize {
    debug_assert!(first < second && second < count);
    first * count - first * (first + 1) / 2 + (second - first - 1)
}

pub fn pair_count(count: usize) -> usize {
    count * count.saturating_sub(1) / 2
}

/// One cached piece-pair row: the maximum cell violation and the witness that
/// realizes it.
#[derive(Clone, Copy, Debug)]
pub struct PairRow {
    /// `v_ij = max over cell pairs of [c_pair - s_ab]_+`.
    pub violation_mm: f64,
    /// The guided integer weight `w = 1 + p`.
    pub penalty: u32,
    /// The active cell's contact, oriented from the **lower-index** piece.
    pub contact: Contact,
}

impl Default for PairRow {
    fn default() -> Self {
        Self {
            violation_mm: 0.0,
            penalty: 0,
            contact: Contact {
                signed_gap_mm: f64::INFINITY,
                normal: [0.0, 0.0],
                witness_a: [0.0, 0.0],
                witness_b: [0.0, 0.0],
            },
        }
    }
}

/// One of the four boundary rows of a piece: left, right, bottom, top.
#[derive(Clone, Copy, Debug, Default)]
pub struct EdgeRow {
    pub violation_mm: f64,
    pub penalty: u32,
    pub witness: [f64; 2],
}

pub const EDGE_LEFT: usize = 0;
pub const EDGE_RIGHT: usize = 1;
pub const EDGE_BOTTOM: usize = 2;
pub const EDGE_TOP: usize = 3;

/// The inward unit normal of each boundary: the direction the piece must move.
pub const EDGE_NORMALS: [[f64; 2]; 4] = [[1.0, 0.0], [-1.0, 0.0], [0.0, 1.0], [0.0, -1.0]];

/// The transformed geometry of the whole layout, structure of arrays.
#[derive(Clone, Debug)]
pub struct Geometry {
    /// Every cell's transformed vertices, packed piece by piece.
    pub cell_points: Vec<[f64; 2]>,
    /// Every cell, as a range into `cell_points`.
    pub cells: Vec<Cell>,
    pub cell_bounds: Vec<[f64; 4]>,
    /// The cell range `[start, end)` of each piece.
    pub piece_cells: Vec<(usize, usize)>,
    /// Every transformed outer ring, packed piece by piece.
    pub ring_points: Vec<[f64; 2]>,
    pub piece_rings: Vec<(usize, usize)>,
    pub piece_bounds: Vec<[f64; 4]>,
    pub centroids: Vec<[f64; 2]>,
}

impl Geometry {
    pub fn cell_slice(&self, cell: usize) -> &[[f64; 2]] {
        let range = self.cells[cell];
        &self.cell_points[range.start..range.start + range.len]
    }

    pub fn ring_slice(&self, piece: usize) -> &[[f64; 2]] {
        let (start, end) = self.piece_rings[piece];
        &self.ring_points[start..end]
    }
}

/// The full ICS state.
#[derive(Clone, Debug)]
pub struct IcsState {
    pub poses: Vec<Pose>,
    pub geometry: Geometry,
    pub pair_rows: Vec<PairRow>,
    pub edge_rows: Vec<[EdgeRow; 4]>,
    /// The locked strip depth this state is being descended into.
    pub target_depth_mm: f64,
}

/// The protected exact incumbent: the only quality series this engine reports.
#[derive(Clone, Debug)]
pub struct ExactIncumbent {
    pub placements: Vec<GeneralFastPlacement>,
    pub raw_source_depth_mm: f64,
    /// `true` when this incumbent is still the constructor's own layout. A
    /// constructor fingerprint is never a "child".
    pub from_constructor: bool,
    pub placement_fingerprint: String,
}

/// Builds the immutable per-piece source data for a piece set, in input order.
pub fn piece_sources(pieces: &[GeneralFastPiece<'_>]) -> Result<Vec<PieceSource>, String> {
    pieces
        .iter()
        .map(|piece| PieceSource::of(piece.id, piece.polygon))
        .collect()
}

/// Transforms one piece into the SoA arrays. Mirrored poses write their cells
/// in reverse order so every cell stays counter-clockwise, which is what the
/// SAT normals in `contact.rs` assume.
pub fn transform_piece(sources: &[PieceSource], geometry: &mut Geometry, poses: &[Pose], piece: usize) {
    let source = &sources[piece];
    let pose = poses[piece];
    let (sin, cos) = pose_sin_cos(pose.theta_deg);
    let (cell_start, cell_end) = geometry.piece_cells[piece];
    for cell in cell_start..cell_end {
        let range = geometry.cells[cell];
        let local = source.decomposition.cells[cell - cell_start];
        for offset in 0..range.len {
            let source_index = if pose.mirrored {
                local.start + local.len - 1 - offset
            } else {
                local.start + offset
            };
            geometry.cell_points[range.start + offset] = apply_pose(
                source.decomposition.points[source_index],
                pose.mirrored,
                sin,
                cos,
                pose.tx_mm,
                pose.ty_mm,
            );
        }
        geometry.cell_bounds[cell] = bounds(
            &geometry.cell_points[range.start..range.start + range.len],
        );
    }
    let (ring_start, ring_end) = geometry.piece_rings[piece];
    let ring = &source.decomposition.ring;
    for offset in 0..(ring_end - ring_start) {
        let source_index = if pose.mirrored {
            ring.len() - 1 - offset
        } else {
            offset
        };
        geometry.ring_points[ring_start + offset] = apply_pose(
            ring[source_index],
            pose.mirrored,
            sin,
            cos,
            pose.tx_mm,
            pose.ty_mm,
        );
    }
    geometry.piece_bounds[piece] = bounds(&geometry.ring_points[ring_start..ring_end]);
    geometry.centroids[piece] = apply_pose(
        source.centroid,
        pose.mirrored,
        sin,
        cos,
        pose.tx_mm,
        pose.ty_mm,
    );
}

/// Allocates the SoA arrays for a piece set. Every later transform writes in
/// place; the descent allocates nothing.
pub fn build_geometry(sources: &[PieceSource], poses: &[Pose]) -> Geometry {
    let mut cells = Vec::new();
    let mut piece_cells = Vec::with_capacity(sources.len());
    let mut piece_rings = Vec::with_capacity(sources.len());
    let mut cell_total = 0usize;
    let mut ring_total = 0usize;
    for source in sources {
        let start = cells.len();
        for cell in &source.decomposition.cells {
            cells.push(Cell {
                start: cell_total,
                len: cell.len,
            });
            cell_total += cell.len;
        }
        piece_cells.push((start, cells.len()));
        piece_rings.push((ring_total, ring_total + source.decomposition.ring.len()));
        ring_total += source.decomposition.ring.len();
    }
    let cell_count = cells.len();
    let mut geometry = Geometry {
        cell_points: vec![[0.0; 2]; cell_total],
        cells,
        cell_bounds: vec![[0.0; 4]; cell_count],
        piece_cells,
        ring_points: vec![[0.0; 2]; ring_total],
        piece_rings,
        piece_bounds: vec![[0.0; 4]; sources.len()],
        centroids: vec![[0.0; 2]; sources.len()],
    };
    for piece in 0..sources.len() {
        transform_piece(sources, &mut geometry, poses, piece);
    }
    geometry
}

/// The layout's raw source depth in the engine's published convention:
/// `max source y + sheet edge clearance`, measured on the transformed `f64`
/// rings and never on the canonical grid.
pub fn raw_source_depth_mm(geometry: &Geometry, contract: &Contract) -> f64 {
    let mut deepest = f64::NEG_INFINITY;
    for point in &geometry.ring_points {
        deepest = deepest.max(point[1]);
    }
    deepest + contract.sheet_edge_clearance_mm
}
