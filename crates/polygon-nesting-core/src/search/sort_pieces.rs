//! The user-owned initial ordering boundary shared by the rectangular and
//! irregular nesting algorithms.
//!
//! TS source: `src/workers/algorithm/sortPiecesForNesting.ts` (18 lines).
//! **Governance note** (quoted verbatim per this task's instructions, do not
//! paraphrase away): `AGENTS.md:46` -- *"`src/workers/algorithm/sortPiecesForNesting.ts`
//! is the user-owned initial ordering boundary. Do not change its behavior
//! unless the user explicitly asks for algorithm work."* Also
//! `docs/architecture/algorithm-boundary.md:15` -- *"`sortPiecesForNesting`
//! is the user-owned initial ordering boundary. It may contain user-provided
//! ordering logic, but it must not place pieces, score placements, split
//! free rectangles, or produce history."* This is a mechanical, zero-
//! interpretation port: descending `longestEdge`, then `area`, then
//! `imbalance`, stable on ties.
//!
//! # Seam: a provisional `PreparedPiece`/`RectWith` mirror
//!
//! TS's `sortPiecesForNesting` operates on `PreparedPiece`
//! (`src/shared/domain/nesting.ts:120-138`, the *rectangular*-algorithm
//! prepared-piece shape, sharing this one boundary function with the
//! irregular algorithm per `search-scoring.md` §2.1 -- "shares the exact
//! same function/behavior" with `computeNesting.ts:66`). That type is not
//! part of `crate::domain` (which is scoped to the irregular-only trusted
//! carriers, see `domain::mod`'s "Scope" doc) and no other module in this
//! crate has ported it yet. Per this task's instructions ("define your
//! inputs against `crate::domain` types... note the seam for later wiring,
//! do NOT block"), [`PreparedPiece`] and [`RectWith`] below are a **minimal,
//! provisional mirror** of `nesting.ts`'s real schema types -- field names
//! and shapes match 1:1 so a future `crate::domain`/boundary module's real
//! `PreparedPiece` (if one lands) can replace this local copy without
//! changing [`sort_pieces_for_nesting`]'s signature in spirit. `crate::domain::Rect`
//! (the `x, y, width, height` carrier already ported for `ImportedPiece::real_bounds`)
//! is reused for `RectWith`'s base fields rather than re-declaring a third
//! copy of that shape.

use std::cmp::Ordering;

use crate::domain::{PieceId, Rect};

/// TS: `src/shared/domain/geometry.ts:36-49` `class RectWith`. A rectangle
/// extended with the three pre-computed sort keys this module's comparator
/// reads. TS's schema constrains `longestEdge`/`area`/`imbalance` to exact
/// integers (`search-scoring.md` §3.1: "all three sort keys are exact
/// integers... removes float-tie/epsilon concerns from this comparator
/// entirely"); mirrored here as `f64` per this crate's established
/// number-fidelity convention (every TS `number` is `f64`, refinements
/// enforced only at the TS decode boundary -- see `domain::transform`'s
/// `IrregularTransformCandidate::index` doc for the canonical statement of
/// this rule).
#[derive(Clone, Debug, PartialEq)]
pub struct RectWith {
    pub rect: Rect,
    pub longest_edge: f64,
    pub area: f64,
    pub imbalance: f64,
}

/// TS: `src/shared/domain/nesting.ts:120-138` `class PreparedPiece`. See
/// this module's top doc for why this is a provisional local mirror rather
/// than a `crate::domain` type.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedPiece {
    pub id: PieceId,
    pub source_piece_id: PieceId,
    pub interchangeability_key: Option<String>,
    pub real_bounds: Rect,
    pub padded_bounds: RectWith,
    pub padding: f64,
    pub allow_rotation: bool,
    pub allow_mirror: bool,
    pub cut_row_ref: Option<CutRowRef>,
}

/// TS: `nesting.ts:133-137`'s inline `cutRowRef` struct literal.
#[derive(Clone, Debug, PartialEq)]
pub struct CutRowRef {
    pub reference: String,
    pub customer_name: String,
    pub csv_row_id: String,
}

/// TS: `sortPiecesForNesting.ts:14-18` `sortPiecesForNesting`.
///
/// TS: `pieces.toSorted(order)`, where `order` is `Order.mapInput(paddedBoundsOrder,
/// piece => piece.paddedBounds)` and `paddedBoundsOrder` is
/// `Order.combineAll([flip(longestEdge), flip(area), flip(imbalance)])` --
/// i.e. descending `longestEdge`, then descending `area`, then descending
/// `imbalance` (`combineAll` keeps the "and then" chain explicit; `flip`
/// makes each numeric comparison descending, so larger candidates sort
/// first).
///
/// `Array.prototype.toSorted` is spec-guaranteed stable (ECMA-262, stable
/// since ES2019): equal-key pieces retain their original relative order, an
/// implicit fourth tie-break TS never states explicitly (original request
/// order). `slice::sort_by` is Rust's stable sort (`sort_unstable_by` is
/// **not** stable and must never be substituted here, per
/// `search-scoring.md` §12 item 1).
///
/// Since `RectWith`'s three sort keys are exact integers at the TS schema
/// boundary (see [`RectWith`]'s doc), plain `f64` `<`/`>` comparison here
/// needs no `Order.Number` NaN/`-0` handling (`js_number`'s `Order.Number`
/// equivalent, `search::score_grid::cmp_number`, is deliberately not used --
/// this comparator's inputs are integers by construction, and pulling in
/// that dependency here would suggest a NaN-handling concern this function
/// genuinely does not have).
pub fn sort_pieces_for_nesting(pieces: &[PreparedPiece]) -> Vec<PreparedPiece> {
    let mut sorted: Vec<PreparedPiece> = pieces.to_vec();
    sorted.sort_by(|first, second| {
        descending(
            first.padded_bounds.longest_edge,
            second.padded_bounds.longest_edge,
        )
        .then_with(|| descending(first.padded_bounds.area, second.padded_bounds.area))
        .then_with(|| {
            descending(
                first.padded_bounds.imbalance,
                second.padded_bounds.imbalance,
            )
        })
    });
    sorted
}

fn descending(a: f64, b: f64) -> Ordering {
    // Exact-integer inputs (see this module's top doc): plain `<`/`>` is
    // sufficient, then flipped for descending order.
    if a < b {
        Ordering::Greater
    } else if a > b {
        Ordering::Less
    } else {
        Ordering::Equal
    }
}

/// Canonical public-request order used before the stable priority sort.
pub(crate) fn compare_canonical_request_ties(
    first: &PreparedPiece,
    second: &PreparedPiece,
) -> Ordering {
    descending(first.real_bounds.width, second.real_bounds.width)
        .then_with(|| descending(first.real_bounds.height, second.real_bounds.height))
        .then_with(|| compare_natural_piece_ids(first.id.as_str(), second.id.as_str()))
}

fn compare_natural_piece_ids(first: &str, second: &str) -> Ordering {
    let first_bytes = first.as_bytes();
    let second_bytes = second.as_bytes();
    let mut first_index = 0;
    let mut second_index = 0;

    while first_index < first_bytes.len() && second_index < second_bytes.len() {
        if first_bytes[first_index].is_ascii_digit() && second_bytes[second_index].is_ascii_digit()
        {
            let first_start = first_index;
            let second_start = second_index;
            while first_index < first_bytes.len() && first_bytes[first_index].is_ascii_digit() {
                first_index += 1;
            }
            while second_index < second_bytes.len() && second_bytes[second_index].is_ascii_digit() {
                second_index += 1;
            }

            let mut first_significant = first_start;
            let mut second_significant = second_start;
            while first_significant + 1 < first_index && first_bytes[first_significant] == b'0' {
                first_significant += 1;
            }
            while second_significant + 1 < second_index && second_bytes[second_significant] == b'0'
            {
                second_significant += 1;
            }

            let first_number = &first_bytes[first_significant..first_index];
            let second_number = &second_bytes[second_significant..second_index];
            let number_order = first_number
                .len()
                .cmp(&second_number.len())
                .then_with(|| first_number.cmp(second_number));
            if number_order != Ordering::Equal {
                return number_order;
            }

            let run_length_order = (first_index - first_start).cmp(&(second_index - second_start));
            if run_length_order != Ordering::Equal {
                return run_length_order;
            }
            continue;
        }

        let byte_order = first_bytes[first_index].cmp(&second_bytes[second_index]);
        if byte_order != Ordering::Equal {
            return byte_order;
        }
        first_index += 1;
        second_index += 1;
    }

    first_bytes.len().cmp(&second_bytes.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece(id: &str, longest_edge: f64, area: f64, imbalance: f64) -> PreparedPiece {
        PreparedPiece {
            id: PieceId::new(id),
            source_piece_id: PieceId::new(id),
            interchangeability_key: None,
            real_bounds: Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
            padded_bounds: RectWith {
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                },
                longest_edge,
                area,
                imbalance,
            },
            padding: 0.0,
            allow_rotation: true,
            allow_mirror: true,
            cut_row_ref: None,
        }
    }

    #[test]
    fn empty_input_returns_empty_output() {
        assert_eq!(sort_pieces_for_nesting(&[]), Vec::<PreparedPiece>::new());
    }

    #[test]
    fn equal_priority_pieces_preserve_original_relative_order() {
        let pieces = vec![
            piece("a", 100.0, 5000.0, 40.0),
            piece("b", 100.0, 5000.0, 40.0),
            piece("c", 100.0, 5000.0, 40.0),
        ];
        let sorted = sort_pieces_for_nesting(&pieces);
        let ids: Vec<&str> = sorted.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn natural_piece_id_order_handles_numeric_runs_without_integer_parsing() {
        let mut ids = ["piece-10", "piece-02", "piece-2", "piece-1"];
        ids.sort_by(|first, second| compare_natural_piece_ids(first, second));
        assert_eq!(ids, ["piece-1", "piece-2", "piece-02", "piece-10"]);
    }

    #[test]
    fn canonical_request_ties_prefer_width_then_height_then_natural_id() {
        let mut wide_10 = piece("piece-10", 100.0, 5000.0, 60.0);
        wide_10.real_bounds.width = 100.0;
        wide_10.real_bounds.height = 40.0;
        let mut tall_1 = piece("piece-1", 100.0, 5000.0, 60.0);
        tall_1.real_bounds.width = 40.0;
        tall_1.real_bounds.height = 100.0;
        let mut wide_2 = piece("piece-2", 100.0, 5000.0, 60.0);
        wide_2.real_bounds.width = 100.0;
        wide_2.real_bounds.height = 40.0;
        let mut pieces = [wide_10, tall_1, wide_2];

        pieces.sort_by(compare_canonical_request_ties);

        let ids: Vec<&str> = pieces.iter().map(|piece| piece.id.as_str()).collect();
        assert_eq!(ids, ["piece-2", "piece-10", "piece-1"]);
    }

    #[test]
    fn sorts_descending_by_longest_edge_then_area_then_imbalance() {
        let pieces = vec![
            piece("small", 50.0, 2500.0, 0.0),
            piece("large", 200.0, 40000.0, 0.0),
            piece("medium", 100.0, 10000.0, 0.0),
        ];
        let sorted = sort_pieces_for_nesting(&pieces);
        let ids: Vec<&str> = sorted.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, vec!["large", "medium", "small"]);
    }

    #[test]
    fn ties_on_longest_edge_break_by_area_then_imbalance() {
        let pieces = vec![
            piece("edge-tie-low-area", 100.0, 4000.0, 0.0),
            piece("edge-tie-high-area", 100.0, 9000.0, 0.0),
            piece("edge-area-tie-low-imbalance", 100.0, 9000.0, 10.0),
            piece("edge-area-tie-high-imbalance", 100.0, 9000.0, 90.0),
        ];
        let sorted = sort_pieces_for_nesting(&pieces);
        let ids: Vec<&str> = sorted.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "edge-area-tie-high-imbalance",
                "edge-area-tie-low-imbalance",
                "edge-tie-high-area",
                "edge-tie-low-area",
            ]
        );
    }
}
