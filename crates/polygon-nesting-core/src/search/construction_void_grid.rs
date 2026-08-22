//! Trapped-void evaluation for the skyline beam constructor, redesigned.
//!
//! The constructor's child acceptance key carries a trapped-void term
//! (`construction_child_key`), and that term is the single most expensive
//! thing mode 20 does: 66.6% of the profiled leaf time of the mode-20 anchor
//! stream, 20.1 s over 11,281 calls, 1.78 ms each. The legacy evaluator
//! ([`super::trapped_void_cells`]) earns that cost honestly - it rasterises
//! the *whole* strip on every call, allocates three vectors per call, and asks
//! a Clipper point-in-polygon question per cell per active collision - but
//! none of that work is *necessary*, and this module is the demonstration.
//!
//! Four changes, each of which is a separate factor:
//!
//! * **Incremental occupancy.** A constructor child is its parent plus exactly
//!   one placed piece, and a piece's occupancy never depends on the pieces
//!   around it. So a child's grid is its parent's grid with one piece's raster
//!   OR-ed in. Parent grids are kept in a small FIFO of retained grids keyed by
//!   state identity, which the previous rank's children populate, so the
//!   expansion of a beam slot normally starts from a cache hit rather than
//!   from a rebuild. The measured effect: the rasteriser sees one piece per
//!   call instead of ~31.
//! * **Scanline rasterisation.** A ring is filled by intersecting the row's
//!   centre line with its edges and filling between sorted crossings, which is
//!   `O(rows x edges)` instead of `O(cells x edges)`. The even-odd rule over
//!   outer ring and holes together is exactly "in material" for the properly
//!   nested, non-overlapping regions a Clipper `PolyTree` produces, and spans
//!   are filled closed - with on-scanline edges and vertices added explicitly -
//!   to reproduce the legacy rule that `IsOn` is occupied.
//! * **Bit-grid flood fill.** Occupancy, freedom and reachability are `u64`
//!   words with one row per word-aligned stride. Horizontal closure of a row
//!   is a Kogge-Stone occluded fill (six shift/mask steps per word plus a
//!   cross-word carry) in each direction; vertical propagation is one
//!   `free & reach` per word. The legacy per-cell `Vec<bool>` stack is gone.
//! * **Scale-derived resolution.** The cell is a fixed fraction of the
//!   *narrowest piece in the request*, not a hard-coded 2 mm. A 2 mm cell is a
//!   different instrument for a 30 mm part than for a 3000 mm one, and the
//!   quantity the key wants - "is this void a channel material could be routed
//!   through" - is a question about piece scale. The cell is coarsened, and
//!   only coarsened, when the derived grid would exceed the evaluator's cell
//!   budget. See [`VOID_CELLS_PER_MIN_PIECE_EXTENT`] for what the sweep found
//!   about the choice, which is that there is nothing to find.
//!
//! Everything here is **guidance**, exactly as the legacy evaluator is: the
//! value feeds a sort key inside the constructor's beam and never reaches a
//! validity decision. Publication still rests entirely on the unchanged exact
//! gates.
//!
//! # Why this is opt-in
//!
//! The evaluator is a different instrument from the legacy raster and does not
//! claim to be the same one: the resolution is derived rather than fixed, and a
//! scanline fill and a per-cell Clipper query can disagree about a cell centre
//! that lands within rounding of an edge. It therefore ships behind
//! `fast-constructor-profile`, **off by default**, under the standing rule that
//! protected legacy stays bit-identical and a new profile owes per-seed
//! determinism, unchanged exact-valid publication, and a quality gate measured
//! as descendant depth under a fixed downstream work budget - never as the
//! constructor's own immediate depth, which the ledger repeatedly shows is an
//! invalid proxy.
//!
//! What it does claim, and what was measured, is that on the pinned mode-20
//! anchor stream it reproduces the legacy constructor's published endpoint -
//! 206.869 mm at fingerprint `8a7737381238fa4d`, every restart row's depth
//! included - and that at five matched cell sizes from 1.875 mm to 2.5 mm the
//! two evaluators produce the *same* endpoint fingerprint as each other. The
//! residue is a handful of `trappedVoidCells` diagnostic counts.
//!
//! With the feature off, every entry point below forwards to
//! [`super::trapped_void_cells`] verbatim and no new state exists.

use super::*;

/// Cells across the narrowest piece in the request.
///
/// The trapped-void term asks whether a void is a channel material can be
/// routed through, so the length scale that matters is the smallest piece: a
/// gap narrower than that can never be filled no matter how it is shaped. A
/// dimensionless divisor makes the whole signal **scale covariant** - scale a
/// request by `k` and every cell, count and ranking is unchanged - which the
/// legacy 2 mm cell emphatically is not: it is 1/15th of the narrowest piece
/// on the pinned Mixed-61 stream, a third of a 6 mm part and a thousandth of a
/// 2 m one.
///
/// The *value* 15 is a calibration, and it is worth being exact about why it
/// is not an optimisation. A sweep of the cell over 18 sizes from 1.2 mm to
/// 5.0 mm on the pinned mode-20 stream, each endpoint given the identical
/// short descent, found no signal in the choice at all: the descendant depth
/// is bimodal (twelve of eighteen land on a 179-181 mm plateau, six find a
/// 169.5-174.3 mm basin) and the two neighbours of the shipped 2.0 mm cell,
/// 1.95 mm and 2.05 mm, both land on the plateau while 1.4 mm beats 2.0 mm.
/// The cell is a lottery ticket, not a tuned parameter. Given that, the only
/// defensible choice is the one that costs nothing to make: 15 reproduces the
/// shipped grid exactly on the stream whose quality is pinned, so this profile
/// is a pure speed change there, and it scales with the geometry everywhere
/// else. A portfolio that wants the 169.5 mm basin should draw several tickets
/// - which this evaluator makes affordable - rather than trust one.
#[cfg(feature = "fast-constructor-profile")]
const VOID_CELLS_PER_MIN_PIECE_EXTENT: f64 = 15.0;

/// The evaluator's per-grid cell budget.
///
/// This is the one quantity here that is a *work* bound rather than a geometry
/// derivation, and it only ever coarsens the cell: a request whose narrowest
/// piece is a thousandth of its sheet would otherwise ask for a grid nobody
/// wants to allocate. At this budget a grid word array is at most 256 KiB.
#[cfg(feature = "fast-constructor-profile")]
const VOID_MAX_GRID_CELLS: f64 = 2_097_152.0;

/// Retained occupancy grids.
///
/// A rank generates at most `CONSTRUCTION_BEAM_WIDTH + 1` parents times
/// `CONSTRUCTION_FINALISTS_PER_SLOT` children, and the next rank expands at
/// most `CONSTRUCTION_BEAM_WIDTH + 1` of them, so a FIFO that holds one full
/// rank of children turns every parent lookup at the next rank into a hit.
#[cfg(feature = "fast-constructor-profile")]
const VOID_GRID_CACHE_SLOTS: usize = 64;

/// The grid resolution, in millimetres, for a request whose narrowest piece
/// has extent `min_piece_extent_mm` on a `width_mm x envelope_mm` strip.
///
/// Homogeneous of degree one in its arguments - scale all three by `k` and the
/// result scales by `k` - which is the property that makes the void signal a
/// statement about the request's own geometry rather than about millimetres.
#[cfg(feature = "fast-constructor-profile")]
fn derived_cell_mm(
    min_piece_extent_mm: f64,
    width_mm: f64,
    envelope_mm: f64,
    cells_per_min_piece_extent: f64,
) -> f64 {
    let piece_cell = min_piece_extent_mm / cells_per_min_piece_extent;
    let budget_cell = ((width_mm.max(0.0) * envelope_mm.max(0.0)) / VOID_MAX_GRID_CELLS).sqrt();
    // The budget only ever coarsens: a cell finer than it would blow the grid,
    // while a cell coarser than the piece scale is what the caller asked for by
    // bringing pieces that large.
    match (
        piece_cell.is_finite() && piece_cell > 0.0,
        budget_cell.is_finite() && budget_cell > 0.0,
    ) {
        (true, true) => piece_cell.max(budget_cell),
        (true, false) => piece_cell,
        (false, true) => budget_cell,
        (false, false) => f64::NAN,
    }
}

/// One retained occupancy grid.
#[cfg(feature = "fast-constructor-profile")]
struct CachedGrid {
    identity: VacancyStateIdentity,
    rows: usize,
    words: Vec<u64>,
}

/// The constructor's trapped-void evaluator.
///
/// Compiled out to a zero-sized forwarder when `fast-constructor-profile` is
/// off; see the module documentation.
#[cfg(feature = "fast-constructor-profile")]
pub(super) struct ConstructionVoidCache {
    cell_mm: f64,
    columns: usize,
    words_per_row: usize,
    tail_mask: u64,
    max_rows: usize,
    parent: Vec<u64>,
    parent_rows: usize,
    occupied: Vec<u64>,
    free: Vec<u64>,
    reach: Vec<u64>,
    row_scratch: Vec<u64>,
    crossings: ScanlineCrossings,
    cache: Vec<CachedGrid>,
    next_slot: usize,
}

#[cfg(feature = "fast-constructor-profile")]
impl ConstructionVoidCache {
    /// Derives the grid resolution from the request's own geometry.
    ///
    /// `divisor_override` replaces [`VOID_CELLS_PER_MIN_PIECE_EXTENT`] for this
    /// one evaluator when a coordinator salted it; `None`, which is every CLI
    /// invocation, keeps the calibrated divisor. A non-finite or non-positive
    /// override is ignored rather than propagated into the cell, because a
    /// coordinator's salt table must not be able to produce a `NaN` grid.
    pub(super) fn new(
        pieces: &[GeneralFastPiece<'_>],
        settings: GeneralFastSettings,
        divisor_override: Option<f64>,
    ) -> Self {
        let divisor = divisor_override
            .filter(|value| value.is_finite() && *value > 0.0)
            .unwrap_or(VOID_CELLS_PER_MIN_PIECE_EXTENT);
        let mut min_extent = f64::INFINITY;
        for piece in pieces {
            if let Some(bounds) = piece.polygon.bounds() {
                let extent = (bounds.max_x - bounds.min_x).min(bounds.max_y - bounds.min_y);
                if extent.is_finite() && extent > 0.0 {
                    min_extent = min_extent.min(extent);
                }
            }
        }
        let width = settings.sheet_short_axis_mm;
        let cell_mm = derived_cell_mm(min_extent, width, settings.sheet_long_axis_mm, divisor);
        let columns = if cell_mm.is_finite() && cell_mm > 0.0 && width.is_finite() && width > 0.0 {
            let columns = (width / cell_mm).ceil();
            if columns.is_finite() && columns > 0.0 {
                columns as usize
            } else {
                0
            }
        } else {
            0
        };
        let words_per_row = columns.div_ceil(64);
        let used_in_tail = columns - 64 * words_per_row.saturating_sub(1);
        let tail_mask = if columns == 0 {
            0
        } else if used_in_tail == 64 {
            u64::MAX
        } else {
            (1u64 << used_in_tail) - 1
        };
        // A grid never exceeds the same cell budget the resolution was derived
        // against, so a frontier past the strip envelope cannot make the
        // evaluator allocate without bound.
        let max_rows = if columns == 0 {
            0
        } else {
            ((VOID_MAX_GRID_CELLS / columns as f64).floor() as usize).max(1)
        };
        Self {
            cell_mm,
            columns,
            words_per_row,
            tail_mask,
            max_rows,
            parent: Vec::new(),
            parent_rows: 0,
            occupied: Vec::new(),
            free: Vec::new(),
            reach: Vec::new(),
            row_scratch: Vec::new(),
            crossings: ScanlineCrossings::default(),
            cache: Vec::new(),
            next_slot: 0,
        }
    }

    /// Rows covering the strip up to `frontier_grid`, plus the two-cell
    /// above-frontier band the flood fill floods from. Same formula the legacy
    /// evaluator uses, at the derived cell.
    fn rows_for(&self, frontier_grid: i64) -> usize {
        let depth = (frontier_grid.max(0) as f64) / 1000.0 + 2.0 * self.cell_mm;
        let rows = (depth / self.cell_mm).ceil();
        if !rows.is_finite() || rows <= 0.0 {
            0
        } else {
            (rows as usize).min(self.max_rows)
        }
    }

    /// Installs the occupancy of the state whose children are about to be
    /// keyed: a cache hit when the previous rank keyed this state as a child,
    /// a full rebuild otherwise.
    pub(super) fn begin_parent(&mut self, state: &VacancyState) {
        if self.columns == 0 {
            return;
        }
        let identity = state_identity(state);
        if let Some(slot) = self
            .cache
            .iter()
            .position(|entry| entry.identity == identity)
        {
            self.parent_rows = self.cache[slot].rows;
            self.parent.clear();
            self.parent.extend_from_slice(&self.cache[slot].words);
            return;
        }
        let frontier = state
            .collisions
            .iter()
            .enumerate()
            .filter(|(index, _)| state.active[*index])
            .filter_map(|(_, collision)| collision.as_ref())
            .filter_map(|collision| collision.bounds())
            .map(|bounds| grid_key(bounds.max_y))
            .max()
            .unwrap_or(0);
        let rows = self.rows_for(frontier);
        self.parent_rows = rows;
        self.parent.clear();
        self.parent.resize(rows * self.words_per_row, 0);
        for (index, collision) in state.collisions.iter().enumerate() {
            if !state.active[index] {
                continue;
            }
            let Some(collision) = collision.as_ref() else {
                continue;
            };
            rasterize(
                self.cell_mm,
                self.columns,
                self.words_per_row,
                rows,
                collision,
                &mut self.crossings,
                &mut self.parent,
            );
        }
    }

    /// The trapped-void count of `child`, which is the state passed to the
    /// most recent [`Self::begin_parent`] plus the piece at `inserted`.
    pub(super) fn child_trapped_cells(
        &mut self,
        child: &VacancyState,
        settings: GeneralFastSettings,
        inserted: usize,
        frontier_grid: i64,
    ) -> usize {
        let _ = settings;
        // Same phase as the legacy evaluator, so the two profiles are read off
        // the same row of the same table.
        let void_span = profiling::deep::start(Phase::VacancyProxyRank);
        if self.columns == 0 {
            profiling::deep::finish(Phase::VacancyProxyRank, void_span);
            return 0;
        }
        let rows = self.rows_for(frontier_grid);
        if rows == 0 {
            profiling::deep::finish(Phase::VacancyProxyRank, void_span);
            return 0;
        }
        let total = rows * self.words_per_row;
        self.occupied.clear();
        self.occupied.resize(total, 0);
        // A child's frontier is its parent's or the inserted piece's, so the
        // parent's rows are never more than the child's and the rows above the
        // parent's own frontier band are free by construction. Zero-extension
        // is therefore exact, not an approximation.
        let shared = (self.parent_rows.min(rows) * self.words_per_row).min(self.parent.len());
        self.occupied[..shared].copy_from_slice(&self.parent[..shared]);
        if let Some(collision) = child.collisions.get(inserted).and_then(Option::as_ref) {
            rasterize(
                self.cell_mm,
                self.columns,
                self.words_per_row,
                rows,
                collision,
                &mut self.crossings,
                &mut self.occupied,
            );
        }
        let trapped = self.flood_and_count(rows);
        self.store(state_identity(child), rows);
        profiling::deep::finish(Phase::VacancyProxyRank, void_span);
        trapped
    }

    /// The trapped-void count of an arbitrary state, rebuilt from scratch.
    /// Used by the constructor's per-restart diagnostic row, which is not on
    /// an incremental lineage.
    pub(super) fn state_trapped_cells(
        &mut self,
        state: &VacancyState,
        settings: GeneralFastSettings,
        frontier_grid: i64,
    ) -> usize {
        let _ = settings;
        let void_span = profiling::deep::start(Phase::VacancyProxyRank);
        if self.columns == 0 {
            profiling::deep::finish(Phase::VacancyProxyRank, void_span);
            return 0;
        }
        let rows = self.rows_for(frontier_grid);
        if rows == 0 {
            profiling::deep::finish(Phase::VacancyProxyRank, void_span);
            return 0;
        }
        self.occupied.clear();
        self.occupied.resize(rows * self.words_per_row, 0);
        for (index, collision) in state.collisions.iter().enumerate() {
            if !state.active[index] {
                continue;
            }
            let Some(collision) = collision.as_ref() else {
                continue;
            };
            rasterize(
                self.cell_mm,
                self.columns,
                self.words_per_row,
                rows,
                collision,
                &mut self.crossings,
                &mut self.occupied,
            );
        }
        let trapped = self.flood_and_count(rows);
        profiling::deep::finish(Phase::VacancyProxyRank, void_span);
        trapped
    }

    /// Heap the evaluator is holding live, for the constructor's retained-byte
    /// ceiling.
    pub(super) fn retained_bytes(&self) -> usize {
        let word = size_of::<u64>();
        let grids = self
            .cache
            .iter()
            .map(|entry| {
                entry.words.capacity() * word
                    + entry.identity.active_placements.capacity()
                        * size_of::<(usize, i64, bool, i64, i64)>()
                    + entry.identity.inactive.capacity() * size_of::<usize>()
                    + size_of::<CachedGrid>()
            })
            .sum::<usize>();
        grids
            + (self.parent.capacity()
                + self.occupied.capacity()
                + self.free.capacity()
                + self.reach.capacity()
                + self.row_scratch.capacity())
                * word
            + self.crossings.interior.capacity() * size_of::<f64>()
            + self.crossings.boundary.capacity() * size_of::<(f64, f64)>()
    }

    fn store(&mut self, identity: VacancyStateIdentity, rows: usize) {
        let total = rows * self.words_per_row;
        if self.cache.len() < VOID_GRID_CACHE_SLOTS {
            self.cache.push(CachedGrid {
                identity,
                rows,
                words: self.occupied[..total].to_vec(),
            });
            return;
        }
        let slot = self.next_slot;
        self.next_slot = (slot + 1) % VOID_GRID_CACHE_SLOTS;
        let entry = &mut self.cache[slot];
        entry.identity = identity;
        entry.rows = rows;
        entry.words.clear();
        entry.words.extend_from_slice(&self.occupied[..total]);
    }

    /// Four-connected flood fill from the above-frontier band, then the count
    /// of free cells the fill did not reach - the trapped voids.
    fn flood_and_count(&mut self, rows: usize) -> usize {
        let Self {
            occupied,
            free,
            reach,
            row_scratch,
            words_per_row,
            tail_mask,
            ..
        } = self;
        let stride = *words_per_row;
        let total = rows * stride;
        free.clear();
        free.resize(total, 0);
        reach.clear();
        reach.resize(total, 0);
        row_scratch.clear();
        row_scratch.resize(stride, 0);
        for row in 0..rows {
            let base = row * stride;
            for word in 0..stride {
                let mut value = !occupied[base + word];
                if word + 1 == stride {
                    value &= *tail_mask;
                }
                free[base + word] = value;
            }
        }
        let top = (rows - 1) * stride;
        reach[top..top + stride].copy_from_slice(&free[top..top + stride]);
        loop {
            let mut changed = false;
            for row in (0..rows.saturating_sub(1)).rev() {
                changed |= propagate(free, reach, row_scratch, row, row + 1, stride);
            }
            for row in 1..rows {
                changed |= propagate(free, reach, row_scratch, row, row - 1, stride);
            }
            if !changed {
                break;
            }
        }
        let mut trapped = 0usize;
        for index in 0..total {
            trapped += (free[index] & !reach[index]).count_ones() as usize;
        }
        trapped
    }
}

/// Adds `row`'s inflow from `from_row` and re-closes the row horizontally.
/// Returns whether the row gained anything; every row is horizontally closed
/// on entry, so a row with no vertical inflow cannot change.
#[cfg(feature = "fast-constructor-profile")]
fn propagate(
    free: &[u64],
    reach: &mut [u64],
    scratch: &mut [u64],
    row: usize,
    from_row: usize,
    stride: usize,
) -> bool {
    let base = row * stride;
    let source = from_row * stride;
    let mut gained = false;
    for word in 0..stride {
        let value = reach[base + word] | (free[base + word] & reach[source + word]);
        gained |= value != reach[base + word];
        scratch[word] = value;
    }
    if !gained {
        return false;
    }
    reach[base..base + stride].copy_from_slice(&scratch[..stride]);
    let (free_row, reach_row) = (&free[base..base + stride], &mut reach[base..base + stride]);
    spread_row(free_row, reach_row);
    true
}

/// Horizontal closure of one row: a Kogge-Stone occluded fill in each
/// direction, with the cross-word carry threaded through the sweep. One
/// rightward sweep followed by one leftward sweep closes every maximal free
/// run that contains a set bit, so the row needs no iteration of its own.
#[cfg(feature = "fast-constructor-profile")]
fn spread_row(free: &[u64], reach: &mut [u64]) {
    let mut carry = false;
    for index in 0..free.len() {
        let passable = free[index];
        let mut filled = reach[index];
        if carry {
            filled |= passable & 1;
        }
        let mut through = passable;
        filled |= through & (filled << 1);
        through &= through << 1;
        filled |= through & (filled << 2);
        through &= through << 2;
        filled |= through & (filled << 4);
        through &= through << 4;
        filled |= through & (filled << 8);
        through &= through << 8;
        filled |= through & (filled << 16);
        through &= through << 16;
        filled |= through & (filled << 32);
        reach[index] = filled;
        carry = filled >> 63 != 0;
    }
    let mut carry = false;
    for index in (0..free.len()).rev() {
        let passable = free[index];
        let mut filled = reach[index];
        if carry {
            filled |= passable & (1u64 << 63);
        }
        let mut through = passable;
        filled |= through & (filled >> 1);
        through &= through >> 1;
        filled |= through & (filled >> 2);
        through &= through >> 2;
        filled |= through & (filled >> 4);
        through &= through >> 4;
        filled |= through & (filled >> 8);
        through &= through >> 8;
        filled |= through & (filled >> 16);
        through &= through >> 16;
        filled |= through & (filled >> 32);
        reach[index] = filled;
        carry = filled & 1 != 0;
    }
}

/// Scanline-fills one collision polygon into the occupancy grid.
///
/// A cell is occupied when its centre lies in the polygon's material *or on its
/// boundary*, which is the legacy evaluator's rule: `contains_point` returning
/// anything but `IsOutside` marks the cell. Interior cells come from the
/// even-odd rule over the region's outer ring and its holes taken together;
/// boundary cells come from two extra sources that even-odd alone drops, and
/// both matter on this engine's geometry because axis-aligned parts at
/// integral translations put edges exactly on cell centres:
///
/// * spans are filled **closed**, so a centre exactly on a left or right
///   crossing is occupied;
/// * an edge lying exactly along the scanline, and a vertex exactly on it,
///   are filled directly, because the half-open crossing test deliberately
///   yields nothing for either.
///
/// Crossings use the half-open `(y1 <= y) != (y2 <= y)` test, so a vertex the
/// ring passes through is counted once, a vertex it merely touches not at all,
/// and the divisor is never zero.
#[cfg(feature = "fast-constructor-profile")]
fn rasterize(
    cell_mm: f64,
    columns: usize,
    words_per_row: usize,
    rows: usize,
    polygon: &PolygonSet,
    crossings: &mut ScanlineCrossings,
    grid: &mut [u64],
) {
    for region in polygon.regions() {
        let bounds = region.outer.bounds();
        if !bounds.min_y.is_finite() || !bounds.max_y.is_finite() {
            continue;
        }
        // Rows whose centre can lie inside this region.
        let first = ((bounds.min_y / cell_mm) - 0.5).floor();
        let last = ((bounds.max_y / cell_mm) - 0.5).ceil();
        if !first.is_finite() || !last.is_finite() {
            continue;
        }
        let first = if first < 0.0 { 0usize } else { first as usize };
        let last = if last < 0.0 {
            continue;
        } else {
            (last as usize).min(rows.saturating_sub(1))
        };
        for row in first..=last.max(first) {
            if row >= rows {
                break;
            }
            let y = (row as f64 + 0.5) * cell_mm;
            crossings.interior.clear();
            crossings.boundary.clear();
            push_crossings(region.outer.points(), y, crossings);
            for hole in &region.holes {
                push_crossings(hole.points(), y, crossings);
            }
            if crossings.interior.len() < 2 && crossings.boundary.is_empty() {
                continue;
            }
            crossings
                .interior
                .sort_by(|first, second| first.total_cmp(second));
            let base = row * words_per_row;
            let row_words = &mut grid[base..base + words_per_row];
            for span in crossings.interior.chunks_exact(2) {
                fill_closed_span(row_words, cell_mm, columns, span[0], span[1]);
            }
            for (low, high) in &crossings.boundary {
                fill_closed_span(row_words, cell_mm, columns, *low, *high);
            }
        }
    }
}

/// Sets every cell of `row_words` whose centre lies in the closed interval
/// `[low, high]`.
#[cfg(feature = "fast-constructor-profile")]
fn fill_closed_span(row_words: &mut [u64], cell_mm: f64, columns: usize, low: f64, high: f64) {
    let start = ((low / cell_mm) - 0.5).ceil();
    let end = ((high / cell_mm) - 0.5).floor() + 1.0;
    if !start.is_finite() || !end.is_finite() {
        return;
    }
    set_bits(
        row_words,
        clamp_column(start, columns),
        clamp_column(end, columns),
    );
}

#[cfg(feature = "fast-constructor-profile")]
fn clamp_column(value: f64, columns: usize) -> usize {
    if value <= 0.0 {
        0
    } else if value >= columns as f64 {
        columns
    } else {
        value as usize
    }
}

/// One scanline's worth of reusable crossing buffers: the even-odd crossings
/// that bound interior spans, and the closed spans an on-scanline edge or
/// vertex contributes directly.
#[cfg(feature = "fast-constructor-profile")]
#[derive(Default)]
struct ScanlineCrossings {
    interior: Vec<f64>,
    boundary: Vec<(f64, f64)>,
}

#[cfg(feature = "fast-constructor-profile")]
fn push_crossings(points: &[IrregularPoint], y: f64, out: &mut ScanlineCrossings) {
    if points.len() < 3 {
        return;
    }
    let mut previous = points[points.len() - 1];
    for current in points {
        if previous.y == y && current.y == y {
            // A horizontal edge on the scanline: every cell centre along it is
            // `IsOn`, and the crossing test below yields nothing for it.
            out.boundary
                .push((previous.x.min(current.x), previous.x.max(current.x)));
        } else if (previous.y <= y) != (current.y <= y) {
            out.interior.push(
                previous.x + (y - previous.y) * (current.x - previous.x) / (current.y - previous.y),
            );
        }
        if current.y == y {
            // A vertex on the scanline. When the ring passes through it the
            // crossing above already covers the cell; when it merely touches,
            // this is the only thing that marks it, and the legacy rule marks
            // it.
            out.boundary.push((current.x, current.x));
        }
        previous = *current;
    }
}

/// Sets bits `[start, end)` of one row's word slice.
#[cfg(feature = "fast-constructor-profile")]
fn set_bits(words: &mut [u64], start: usize, end: usize) {
    if start >= end {
        return;
    }
    let first_word = start / 64;
    let last_word = (end - 1) / 64;
    let head = u64::MAX << (start % 64);
    let tail = u64::MAX >> (63 - ((end - 1) % 64));
    if first_word == last_word {
        words[first_word] |= head & tail;
        return;
    }
    words[first_word] |= head;
    for word in words.iter_mut().take(last_word).skip(first_word + 1) {
        *word = u64::MAX;
    }
    words[last_word] |= tail;
}

/// The forwarder compiled when `fast-constructor-profile` is off: a zero-sized
/// value whose entry points are the legacy evaluator, unchanged.
#[cfg(not(feature = "fast-constructor-profile"))]
pub(super) struct ConstructionVoidCache;

#[cfg(not(feature = "fast-constructor-profile"))]
impl ConstructionVoidCache {
    pub(super) fn new(
        pieces: &[GeneralFastPiece<'_>],
        settings: GeneralFastSettings,
        divisor_override: Option<f64>,
    ) -> Self {
        // The legacy raster's cell is a fixed grid step, not a derived one, so
        // there is nothing here for a divisor salt to move.
        let _ = (pieces, settings, divisor_override);
        Self
    }

    pub(super) fn begin_parent(&mut self, state: &VacancyState) {
        let _ = state;
    }

    pub(super) fn child_trapped_cells(
        &mut self,
        child: &VacancyState,
        settings: GeneralFastSettings,
        inserted: usize,
        frontier_grid: i64,
    ) -> usize {
        let _ = inserted;
        trapped_void_cells(child, settings, frontier_grid)
    }

    pub(super) fn state_trapped_cells(
        &mut self,
        state: &VacancyState,
        settings: GeneralFastSettings,
        frontier_grid: i64,
    ) -> usize {
        trapped_void_cells(state, settings, frontier_grid)
    }

    pub(super) fn retained_bytes(&self) -> usize {
        0
    }
}

#[cfg(all(test, feature = "fast-constructor-profile"))]
mod tests {
    use super::*;

    /// Reference four-connected flood fill over an explicit boolean grid, in
    /// the shape of the legacy evaluator's stack walk. The bit-grid fill has
    /// to agree with it on every input.
    fn reference_trapped(free: &[bool], columns: usize, rows: usize) -> usize {
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
            let push = |candidate: usize, reachable: &mut Vec<bool>, stack: &mut Vec<usize>| {
                if free[candidate] && !reachable[candidate] {
                    reachable[candidate] = true;
                    stack.push(candidate);
                }
            };
            if column > 0 {
                push(cell - 1, &mut reachable, &mut stack);
            }
            if column + 1 < columns {
                push(cell + 1, &mut reachable, &mut stack);
            }
            if row > 0 {
                push(cell - columns, &mut reachable, &mut stack);
            }
            if row + 1 < rows {
                push(cell + columns, &mut reachable, &mut stack);
            }
        }
        free.iter()
            .zip(reachable.iter())
            .filter(|(is_free, is_reachable)| **is_free && !**is_reachable)
            .count()
    }

    fn evaluator(columns: usize) -> ConstructionVoidCache {
        let words_per_row = columns.div_ceil(64);
        let used_in_tail = columns - 64 * words_per_row.saturating_sub(1);
        ConstructionVoidCache {
            cell_mm: 1.0,
            columns,
            words_per_row,
            tail_mask: if used_in_tail == 64 {
                u64::MAX
            } else {
                (1u64 << used_in_tail) - 1
            },
            max_rows: 1 << 20,
            parent: Vec::new(),
            parent_rows: 0,
            occupied: Vec::new(),
            free: Vec::new(),
            reach: Vec::new(),
            row_scratch: Vec::new(),
            crossings: ScanlineCrossings::default(),
            cache: Vec::new(),
            next_slot: 0,
        }
    }

    fn count_from_bools(free: &[bool], columns: usize, rows: usize) -> usize {
        let mut evaluator = evaluator(columns);
        let stride = evaluator.words_per_row;
        evaluator.occupied = vec![0u64; rows * stride];
        for row in 0..rows {
            for column in 0..columns {
                if !free[row * columns + column] {
                    evaluator.occupied[row * stride + column / 64] |= 1u64 << (column % 64);
                }
            }
        }
        evaluator.flood_and_count(rows)
    }

    #[test]
    fn bit_flood_fill_agrees_with_the_reference_walk_on_a_pseudorandom_corpus() {
        // A xorshift corpus rather than a hand-drawn one: the interesting
        // disagreements are cross-word carries and snaking channels, and those
        // are easier to hit by volume than by drawing.
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for case in 0..200 {
            let columns = 1 + (next() as usize % 200);
            let rows = 1 + (next() as usize % 40);
            let density = next() % 100;
            let free = (0..columns * rows)
                .map(|_| next() % 100 >= density)
                .collect::<Vec<_>>();
            assert_eq!(
                count_from_bools(&free, columns, rows),
                reference_trapped(&free, columns, rows),
                "case {case}: columns {columns} rows {rows} density {density}"
            );
        }
    }

    #[test]
    fn a_sealed_pocket_is_trapped_and_an_open_channel_is_not() {
        // Row 0 is the sheet floor; the top row is the above-frontier band.
        // `#` is occupied, `.` free. The left pocket is sealed, the right one
        // drains through a one-cell channel.
        let plan = [
            "........................",
            "..####..........####....",
            "..#..#..........#..#....",
            "..#..#..........#.,#....",
            "..####..........##.##...",
            "........................",
        ];
        let rows = plan.len();
        let columns = plan[0].len();
        // The plan is written top row first; the grid's row 0 is the bottom.
        let mut free = vec![false; columns * rows];
        for (index, line) in plan.iter().rev().enumerate() {
            for (column, glyph) in line.chars().enumerate() {
                free[index * columns + column] = glyph != '#';
            }
        }
        assert_eq!(
            count_from_bools(&free, columns, rows),
            reference_trapped(&free, columns, rows)
        );
        // The sealed 2x2 pocket on the left is the only trapped region.
        assert_eq!(count_from_bools(&free, columns, rows), 4);
    }

    #[test]
    fn the_derived_resolution_is_scale_covariant() {
        // The whole point of a dimensionless divisor: a request expressed in a
        // different unit, or a machine cut at a different size, produces the
        // same grid in piece units. A fixed 2 mm cell fails this by
        // construction.
        for scale in [1e-3, 0.25, 1.0, 3.0, 250.0, 1e4] {
            let base = derived_cell_mm(30.0, 2000.0, 2700.0, VOID_CELLS_PER_MIN_PIECE_EXTENT);
            let scaled = derived_cell_mm(
                30.0 * scale,
                2000.0 * scale,
                2700.0 * scale,
                VOID_CELLS_PER_MIN_PIECE_EXTENT,
            );
            let relative = (scaled - base * scale).abs() / (base * scale);
            assert!(
                relative < 1e-12,
                "scale {scale}: {scaled} against {}",
                base * scale
            );
        }
    }

    #[test]
    fn the_derived_resolution_reproduces_the_shipped_grid_on_the_pinned_stream() {
        // Mixed-61: narrowest source piece 30 mm, strip 2000 mm x 2700 mm.
        // The calibration exists so that this profile's first delivery is a
        // speed change with an unchanged constructor endpoint; if this
        // assertion ever moves, the mode-20 quality evidence moves with it.
        assert_eq!(
            derived_cell_mm(30.0, 2000.0, 2700.0, VOID_CELLS_PER_MIN_PIECE_EXTENT),
            2.0
        );
    }

    #[test]
    fn the_cell_budget_coarsens_a_grid_that_would_be_too_fine() {
        // A 0.1 mm part on a 2 m x 2.7 m strip asks for a 6.7 nm... a 0.0067 mm
        // cell, which is 8e10 cells. The budget must take over, and it must
        // never refine a cell the piece scale already made coarse.
        let tiny = derived_cell_mm(0.1, 2000.0, 2700.0, VOID_CELLS_PER_MIN_PIECE_EXTENT);
        assert!(tiny > 0.1 / VOID_CELLS_PER_MIN_PIECE_EXTENT);
        assert!((2000.0 / tiny).ceil() * (2700.0 / tiny).ceil() <= VOID_MAX_GRID_CELLS * 1.01);
        assert_eq!(
            derived_cell_mm(3000.0, 2000.0, 2700.0, VOID_CELLS_PER_MIN_PIECE_EXTENT),
            200.0
        );
    }

    #[test]
    fn set_bits_writes_exactly_the_requested_range() {
        for start in 0..70usize {
            for length in 0..70usize {
                let mut words = [0u64; 4];
                set_bits(&mut words, start, start + length);
                for bit in 0..256usize {
                    let set = words[bit / 64] >> (bit % 64) & 1 == 1;
                    assert_eq!(
                        set,
                        bit >= start && bit < start + length,
                        "start {start} length {length} bit {bit}"
                    );
                }
            }
        }
    }
}
