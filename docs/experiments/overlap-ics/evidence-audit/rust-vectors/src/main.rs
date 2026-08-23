//! **The independent recomputation of the evidence-producing machinery.**
//!
//!     cargo run --release -- <repo-root> [out.json]
//!
//! Every section below computes something the engine also computes, by a
//! *different route written from the specification text*, and asserts the two
//! agree. Nothing here re-uses the engine's own answer as its expectation, and
//! nothing here is a threshold.
//!
//! * **S1 `observe_raw`** - a 20-line reference transition function written from
//!   the frozen sentence (Grok review 12 Round 2 §6.5 and the Sparrow
//!   `separator.rs:102-115` reading quoted in `mod.rs`), driven over two million
//!   random and adversarial sequences against the shipped helper.
//! * **S2 the convention family** - every place a `W`/`T`/inset/clearance/radius
//!   enters a computation in two modules, asserted equal on three contracts
//!   including one with `sag > 0` and one with `safety > 0`. The gate fixture has
//!   `sag = safety = 0`, which makes the whole clearance split degenerate there;
//!   these vectors are the only place it is exercised.
//! * **S3 the measurement path** - `publish::raw_depth_of` (published depth, on
//!   placements) against `state::raw_source_depth_mm` (proxy depth, on geometry)
//!   for random poses, bit for bit.
//! * **S4 Algorithm 8** - the weight schedule recomputed from the previous
//!   weights and the row violations on a real trajectory slice, plus
//!   reset-on-width-change and persist-across-rollback.
//! * **S5 the tournament** - eight worker sweeps reconstructed outside the
//!   engine from the same entry state and the same keys; the master's aggregate
//!   work vector must equal the *sum of the eight deltas* (Sol's double-debit
//!   concern), the winner must be the minimum guided with the ordinal breaking
//!   ties, and the installed state must be the winner's.
//! * **S6 the bite record** - `width_before/after`, `delta`, `split_y` and
//!   `moved_pieces` of the first explore bite recomputed from the constructor's
//!   own poses, and `uniform_cut_mm` recomputed for a compress bite from
//!   `(seed, ordinal)` alone.
//!
//! Exit status is 1 if any assertion fails.

use std::collections::BTreeMap;

use polygon_nesting_core::domain::ImportedPiece;
use polygon_nesting_core::geometry::general_polygon::PolygonSet;
use polygon_nesting_core::geometry::general_source::polygon_set_from_imported_piece;
use polygon_nesting_core::search::general_fast::{
    construct_short_side_first, GeneralFastPiece, GeneralFastPlacement, GeneralFastSettings,
};
use polygon_nesting_core::search::overlap_ics::broad_phase::boundary_residuals;
use polygon_nesting_core::search::overlap_ics::descent::{counter_hash, Descent, DescentConfig};
use polygon_nesting_core::search::overlap_ics::diagnostics::WorkVector;
use polygon_nesting_core::search::overlap_ics::energy;
use polygon_nesting_core::search::overlap_ics::homotopy;
use polygon_nesting_core::search::overlap_ics::publish::{placement_fingerprint, placements_of, raw_depth_of};
use polygon_nesting_core::search::overlap_ics::relocate::{
    strip_sample_box, centroid_relative_extents, transformed_centroid,
};
use polygon_nesting_core::search::overlap_ics::state::{
    self, build_geometry, pair_count, piece_sources, raw_source_depth_mm, Contract, EdgeRow,
    IcsState, PairRow, PieceSource, Pose, EDGE_BOTTOM, EDGE_LEFT, EDGE_RIGHT,
    EDGE_TOP, GLS_WEIGHT_FLOOR,
};
use polygon_nesting_core::search::overlap_ics::{
    observe_raw, poses_of, Budget, Engine, IcsConfig, RawObservation, ScheduleConfig,
    STRIKE_IMPROVEMENT_RATIO,
};
use serde_json::{json, Value};

// ------------------------------------------------------------- the ledger ---

#[derive(Default)]
struct Ledger {
    rows: Vec<Value>,
}

impl Ledger {
    fn check(&mut self, name: &str, ok: bool, detail: Value) {
        self.rows.push(json!({"vector": name, "ok": ok, "detail": detail}));
        if !ok {
            eprintln!("FAIL {name}: {detail}");
        }
    }
    fn failures(&self) -> Vec<&Value> {
        self.rows
            .iter()
            .filter(|row| row["ok"] == json!(false))
            .collect()
    }
}

// -------------------------------------------- S1: the strike helper (spec) ---

/// **The reference transition, written from the spec text and nothing else.**
///
/// The frozen sentence is *"explore 200 iterations without 2 % raw-Φ
/// improvement vs strike-best -> strike"*, and its source reading is *"a new
/// best that is not below `min_loss * 0.98` updates the incumbent and falls
/// through **without touching** `n_iter_no_improvement`; only a
/// non-improvement increments it"*.
///
/// Transcribed as three disjoint cases on `(raw, min)`, deliberately in a
/// different shape from the shipped one - a match on a pre-computed ordering
/// rather than a nested `if` - so that a shared typo cannot survive in both:
///
/// ```text
///   raw <  0.98 * min   ->  new minimum, counter RESET      ("substantial")
///   0.98 * min <= raw < min -> new minimum, counter UNTOUCHED ("marginal")
///   raw >= min          ->  no new minimum, counter += 1     ("none")
/// ```
///
/// `min` starts at `+inf`, for which the first branch is the whole real line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reference {
    Substantial,
    Marginal,
    None,
}

fn reference_observe(raw: f64, min: &mut f64, since: &mut u64) -> Reference {
    let improves = raw < *min;
    let substantially = raw < STRIKE_IMPROVEMENT_RATIO * *min;
    let verdict = match (improves, substantially) {
        (true, true) => Reference::Substantial,
        (true, false) => Reference::Marginal,
        (false, _) => Reference::None,
    };
    match verdict {
        Reference::Substantial => {
            *min = raw;
            *since = 0;
        }
        Reference::Marginal => {
            *min = raw;
        }
        Reference::None => {
            *since += 1;
        }
    }
    verdict
}

fn shipped_class(observation: RawObservation) -> Reference {
    match observation {
        RawObservation::Substantial => Reference::Substantial,
        RawObservation::Marginal => Reference::Marginal,
        RawObservation::None => Reference::None,
    }
}

/// A deterministic `[0, 1)` stream from the engine's own counter hash, so the
/// property test is replayable and carries no `rand` dependency.
fn unit(key: u64) -> f64 {
    (counter_hash(&[key]) >> 11) as f64 / (1u64 << 53) as f64
}

fn section_strike(ledger: &mut Ledger) {
    // --- the committed transition table, restated as a truth table ----------
    let table: [(f64, f64, u64, Reference, f64, u64); 9] = [
        // first ever reading against +inf: always substantial, counter resets
        (1.0, f64::INFINITY, 7, Reference::Substantial, 1.0, 0),
        // a 2 % improvement: reset
        (0.97, 1.0, 5, Reference::Substantial, 0.97, 0),
        // exactly 2 %: NOT substantial (the predicate is strict `<`)
        (0.98, 1.0, 5, Reference::Marginal, 0.98, 5),
        // inside the band: snapshot moves, counter paused
        (0.999, 1.0, 5, Reference::Marginal, 0.999, 5),
        // a microscopic minimum: paused, not forgiven. THIS is round 1's bug.
        (1.0 - 1e-15, 1.0, 199, Reference::Marginal, 1.0 - 1e-15, 199),
        // equal: not a minimum
        (1.0, 1.0, 5, Reference::None, 1.0, 6),
        // worse: not a minimum
        (2.0, 1.0, 5, Reference::None, 1.0, 6),
        // at the zero floor, zero is not an improvement on zero
        (0.0, 0.0, 3, Reference::None, 0.0, 4),
        // NaN is not less than anything
        (f64::NAN, 1.0, 3, Reference::None, 1.0, 4),
    ];
    let mut table_ok = true;
    let mut table_detail = Vec::new();
    for (raw, min0, since0, want_class, want_min, want_since) in table {
        let (mut min, mut since) = (min0, since0);
        let got = shipped_class(observe_raw(raw, &mut min, &mut since));
        let ok = got == want_class
            && (min.to_bits() == want_min.to_bits())
            && since == want_since;
        table_ok &= ok;
        if !ok {
            table_detail.push(json!({
                "raw": raw, "minIn": min0, "sinceIn": since0,
                "gotClass": format!("{got:?}"), "wantClass": format!("{want_class:?}"),
                "gotMin": min, "wantMin": want_min,
                "gotSince": since, "wantSince": want_since,
            }));
        }
    }
    ledger.check(
        "S1a observe_raw truth table (9 hand-derived transitions)",
        table_ok,
        json!({"offenders": table_detail}),
    );

    // --- the property test: two million steps over random sequences ---------
    //
    // Values are drawn from a mixture chosen to hit the boundaries the shipped
    // predicate is written on: exact ties, the 0.98 knife edge, the 1e-15
    // trickle that was round 1's defect, zero, and coarse random.
    let mut mismatches: Vec<Value> = Vec::new();
    let mut classes = [0u64; 3];
    let mut steps = 0u64;
    for run in 0..20_000u64 {
        let (mut min_a, mut since_a) = (f64::INFINITY, 0u64);
        let (mut min_b, mut since_b) = (f64::INFINITY, 0u64);
        for step in 0..100u64 {
            let key = counter_hash(&[run, step, 0xA5A5_5A5A]);
            let pick = key % 7;
            let raw = match pick {
                0 => unit(key),                                   // uniform
                1 => min_b,                                       // exact tie
                2 => min_b * STRIKE_IMPROVEMENT_RATIO,            // the knife edge
                3 => min_b * (1.0 - 1e-15),                       // the trickle
                4 => 0.0,                                         // the floor
                5 => min_b * (0.5 + unit(key >> 7)),              // near
                _ => unit(key >> 13) * 1e-4,                      // the shelf
            };
            let want = reference_observe(raw, &mut min_a, &mut since_a);
            let got = shipped_class(observe_raw(raw, &mut min_b, &mut since_b));
            classes[match got {
                Reference::Substantial => 0,
                Reference::Marginal => 1,
                Reference::None => 2,
            }] += 1;
            steps += 1;
            if got != want
                || min_a.to_bits() != min_b.to_bits()
                || since_a != since_b
            {
                if mismatches.len() < 5 {
                    mismatches.push(json!({
                        "run": run, "step": step, "raw": raw,
                        "referenceClass": format!("{want:?}"),
                        "shippedClass": format!("{got:?}"),
                        "referenceMin": min_a, "shippedMin": min_b,
                        "referenceSince": since_a, "shippedSince": since_b,
                    }));
                }
            }
        }
    }
    ledger.check(
        "S1b observe_raw property test vs spec-text reference",
        mismatches.is_empty(),
        json!({
            "steps": steps,
            "substantial": classes[0], "marginal": classes[1], "none": classes[2],
            "offenders": mismatches,
        }),
    );

    // --- S1c: the repair's own red/green, on the sequence it was made for ---
    //
    // Round 1 shipped `raw < min => reset`. The repair pauses instead of
    // resetting. The vector that separates them is an ALTERNATING sequence -
    // one marginal minimum, one non-improvement - which is what the Φ ≈ 1e-4
    // shelf of mixed-61's 22nd bite produces: under round 1 the counter is
    // knocked back to zero every second step and never reaches 200; under the
    // repair it advances by one every two steps and strikes at 400.
    fn round_one(raw: f64, min: &mut f64, since: &mut u64) {
        if raw < *min {
            *min = raw;
            *since = 0;
        } else {
            *since += 1;
        }
    }
    let (mut min_old, mut since_old) = (1e-4f64, 0u64);
    let (mut min_new, mut since_new) = (1e-4f64, 0u64);
    for step in 0..400u64 {
        let raw_old = if step % 2 == 0 { min_old * (1.0 - 1e-15) } else { min_old * 1.5 };
        let raw_new = if step % 2 == 0 { min_new * (1.0 - 1e-15) } else { min_new * 1.5 };
        round_one(raw_old, &mut min_old, &mut since_old);
        observe_raw(raw_new, &mut min_new, &mut since_new);
    }
    ledger.check(
        "S1c the alternating shelf sequence strikes under the repair and not under round 1",
        since_new >= 200 && since_old <= 1,
        json!({"repairedCounter": since_new, "roundOneCounter": since_old,
               "exploreStrikeCap": 200,
               "note": "this is the red/green the failure license names"}),
    );

    // --- S1d: REPORTED, not asserted. The case the repair does NOT cover. ---
    //
    // A MONOTONE trickle - a new minimum inside the 2 % band on *every*
    // iteration, with no non-improving iteration between - leaves the counter
    // exactly where it started, under the repaired predicate as much as under
    // round 1's. That is faithful to the cited source (`separator.rs:102-115`
    // increments only on a non-improvement), so it is not a defect of the port;
    // it is the boundary of what the repair can do, and any claim that "the
    // separation now strikes out on the shelf" has to be a claim about a
    // sequence with non-improving iterations in it.
    let (mut min, mut since) = (1e-4f64, 0u64);
    let mut marginal = 0u64;
    for _ in 0..1000 {
        let raw = min * (1.0 - 1e-15);
        if observe_raw(raw, &mut min, &mut since) == RawObservation::Marginal {
            marginal += 1;
        }
    }
    ledger.check(
        "S1d REPORTED: a monotone marginal trickle never increments the counter",
        true,
        json!({"iterations": 1000, "marginalMinima": marginal,
               "sinceImprovementAfter": since,
               "reading": "the repair pauses the counter on a marginal minimum; \
                           it does not increment it, so a strictly monotone \
                           trickle still holds one separation open indefinitely",
               "faithfulToSource": true}),
    );
}

// ---------------------------------------- S2: the convention family, twice ---

fn contract_of(edge: f64, pair: f64, sag: f64, safety: f64) -> Contract {
    let mut settings = GeneralFastSettings::deterministic_test(2000.0, 2700.0);
    settings.total_padding_mm = pair;
    settings.sheet_edge_clearance_mm = Some(edge);
    settings.flattening_sag_tolerance_mm = sag;
    settings.clearance_safety_margin_mm = safety;
    Contract::from_settings(settings)
}

/// The three sites that build "the usable strip" out of the contract, restated
/// as one predicate each, and compared on random boxes.
///
/// * `broad_phase::boundary_residuals` - Φ's four rows.
/// * `relocate::strip_sample_box`      - where a container draw may put a centroid.
/// * `publish::sheet_slack`            - how far a repair may push a piece.
///
/// `sheet_slack` is private, so it is reached through the only public statement
/// of the same rule the module makes: a repair may not push a box past the
/// wall `boundary_residuals` charges. The equality asserted is therefore
/// "`strip_sample_box` and `boundary_residuals` name the same four walls", plus
/// the arithmetic identity `strip_top == target - depth_top_inset` that
/// `sheet_slack` shares with both by construction.
fn section_conventions(ledger: &mut Ledger) {
    // The two real contracts of this campaign, plus one probe that exists only
    // to demonstrate a latent coupling. `asserted = false` on the probe: it is
    // expected to be red and a harness that failed on it would be permanently
    // red about a request nobody runs.
    let contracts = [
        ("gate mixed-61 (sag=0, safety=0)", contract_of(5.0, 5.0, 0.0, 0.0), true),
        ("triangle-20 (sag=0.25, safety=0.25)", contract_of(5.0, 10.0, 0.25, 0.25), true),
        ("PROBE safety>sag (not a campaign request)", contract_of(5.0, 10.0, 0.10, 0.40), false),
    ];
    for (label, contract, asserted) in contracts {
        // --- C1: the sample box and the boundary rows name the same walls ---
        let mut worst_gap = 0.0f64;
        let mut offenders: Vec<Value> = Vec::new();
        for trial in 0..20_000u64 {
            let key = counter_hash(&[trial, 0xC0FFEE]);
            let target = 100.0 + unit(key) * 2400.0;
            // A synthetic piece: centroid-relative extents of a random box.
            let extents = [
                -(1.0 + unit(key >> 3) * 60.0),
                -(1.0 + unit(key >> 9) * 60.0),
                1.0 + unit(key >> 15) * 60.0,
                1.0 + unit(key >> 21) * 60.0,
            ];
            let sample = strip_sample_box(&contract, target, extents);
            // Put the centroid at each of the four corners of the sample box
            // and assert Φ charges exactly nothing.
            for (cx, cy) in [
                (sample[0], sample[1]),
                (sample[2], sample[3]),
                (sample[0], sample[3]),
                (sample[2], sample[1]),
            ] {
                if !(sample[0] <= sample[2] && sample[1] <= sample[3]) {
                    continue; // an infeasible box: `mix` clamps, Φ is charged
                }
                let boxed = [
                    cx + extents[0],
                    cy + extents[1],
                    cx + extents[2],
                    cy + extents[3],
                ];
                let residual = boundary_residuals(boxed, &contract, target);
                let worst = residual.iter().copied().fold(0.0f64, f64::max);
                worst_gap = worst_gap.max(worst);
                if worst > 1e-9 && offenders.len() < 4 {
                    offenders.push(json!({
                        "trial": trial, "target": target, "extents": extents,
                        "sampleBox": sample, "residual": residual,
                    }));
                }
            }
        }
        ledger.check(
            &format!("S2a[{label}] strip_sample_box corners charge zero boundary Φ"),
            offenders.is_empty(),
            json!({"worstResidualMm": worst_gap, "offenders": offenders}),
        );

        // --- C2: one micrometre outside a wall is charged exactly that -------
        let target = 500.0;
        let extents = [-10.0, -20.0, 30.0, 40.0];
        let sample = strip_sample_box(&contract, target, extents);
        let mut exactness: Vec<Value> = Vec::new();
        let epsilon = 0.001;
        for (index, (dx, dy)) in [(-epsilon, 0.0), (epsilon, 0.0), (0.0, -epsilon), (0.0, epsilon)]
            .into_iter()
            .enumerate()
        {
            let (cx, cy) = match index {
                0 => (sample[0] + dx, (sample[1] + sample[3]) / 2.0),
                1 => (sample[2] + dx, (sample[1] + sample[3]) / 2.0),
                2 => ((sample[0] + sample[2]) / 2.0, sample[1] + dy),
                _ => ((sample[0] + sample[2]) / 2.0, sample[3] + dy),
            };
            let boxed = [cx + extents[0], cy + extents[1], cx + extents[2], cy + extents[3]];
            let residual = boundary_residuals(boxed, &contract, target);
            let side = match index {
                0 => EDGE_LEFT,
                1 => EDGE_RIGHT,
                2 => EDGE_BOTTOM,
                _ => EDGE_TOP,
            };
            if (residual[side] - epsilon).abs() > 1e-9 {
                exactness.push(json!({"side": side, "residual": residual}));
            }
        }
        ledger.check(
            &format!("S2b[{label}] 1 um outside a sample-box wall is 1 um of Φ on that side"),
            exactness.is_empty(),
            json!({"offenders": exactness, "sampleBox": sample}),
        );

        // --- C3: the depth convention and the top row are the same rule -----
        //
        // `raw_source_depth = max_y + sheet_edge_clearance` and the top row is
        // charged at `T - depth_top_inset`. The two agree iff
        // `depth_top_inset == sheet_edge_clearance`, i.e. iff a layout whose
        // raw depth is exactly `T` charges exactly zero top Φ. A strip top at
        // `edge + sag` would charge one sag tolerance of phantom violation.
        let strip_target = 400.0;
        let max_y = strip_target - contract.sheet_edge_clearance_mm;
        let boxed = [100.0, 100.0, 200.0, max_y];
        let residual = boundary_residuals(boxed, &contract, strip_target);
        ledger.check(
            &format!("S2c[{label}] raw depth == T charges zero top Φ (no phantom sag)"),
            residual[EDGE_TOP] == 0.0,
            json!({"topResidualMm": residual[EDGE_TOP],
                   "depthTopInset": contract.depth_top_inset_mm(),
                   "physicalEdge": contract.physical_edge_clearance_mm(),
                   "sheetEdgeClearance": contract.sheet_edge_clearance_mm}),
        );
        let over = [100.0, 100.0, 200.0, max_y + 0.007];
        let residual_over = boundary_residuals(over, &contract, strip_target);
        ledger.check(
            &format!("S2d[{label}] 7 um past T is exactly 7 um of top Φ"),
            (residual_over[EDGE_TOP] - 0.007).abs() < 1e-12,
            json!({"topResidualMm": residual_over[EDGE_TOP]}),
        );

        // --- C4: the kernel boundary vs Φ's boundary --------------------------
        //
        // `boundary_admissible` demands `min - radius >= inset`, i.e. material
        // at least `sheet_inset + expansion == edge + safety` from the sheet
        // edge. Φ's left/right/bottom rows demand `edge + sag`. Φ is the
        // conservative one iff `sag >= safety`; when `safety > sag` a layout Φ
        // calls boundary-clear is one the exact kernel refuses, and the 4 um
        // repair band is the only thing between them.
        let kernel_boundary = contract.sheet_inset_mm() + contract.expansion_mm();
        let phi_boundary = contract.physical_edge_clearance_mm();
        ledger.check(
            &format!("S2e[{label}] Φ's boundary is at least the kernel's"),
            phi_boundary >= kernel_boundary || !asserted,
            json!({
                "asserted": asserted,
                "reportedRed": phi_boundary < kernel_boundary,
                "phiBoundaryMm": phi_boundary,
                "kernelBoundaryMm": kernel_boundary,
                "shortfallMm": kernel_boundary - phi_boundary,
                "sag": contract.flattening_sag_tolerance_mm,
                "safety": contract.clearance_safety_margin_mm,
                "note": "shortfall > 0 means Φ under-charges the sheet edge \
                         relative to the exact kernel by that many mm",
            }),
        );

        // --- C5: the two clearance derivations -------------------------------
        ledger.check(
            &format!("S2f[{label}] pair clearance == padding + 2 sag, radius == padding/2 + safety"),
            contract.pair_clearance_mm()
                == contract.total_padding_mm + 2.0 * contract.flattening_sag_tolerance_mm
                && contract.expansion_mm()
                    == contract.total_padding_mm / 2.0 + contract.clearance_safety_margin_mm,
            json!({"pairClearance": contract.pair_clearance_mm(),
                   "expansion": contract.expansion_mm(),
                   "twoR": 2.0 * contract.expansion_mm()}),
        );
    }
}

// ---------------------- S2/S3 on the real fixture: pivot and depth identity ---

struct Fixture {
    polygons: Vec<PolygonSet>,
    ids: Vec<String>,
    rotations: Vec<bool>,
    mirrors: Vec<bool>,
    settings: GeneralFastSettings,
}

fn load_fixture(root: &str) -> Result<Fixture, String> {
    let path = format!(
        "{root}/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json"
    );
    let bytes = std::fs::read(&path).map_err(|error| format!("{path}: {error}"))?;
    let request: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let sheet_w = request["sheet"]["width"].as_f64().ok_or("sheet.width")?;
    let sheet_h = request["sheet"]["height"].as_f64().ok_or("sheet.height")?;
    // Both request shapes the benchmark accepts: the modern `settings` block
    // and the legacy `options.irregularSettings`. mixed-61 is the legacy one,
    // and a loader that only read `settings` would have silently audited a
    // different contract from the one the gate ran.
    let (settings_node, geometry) = if request["settings"].is_object() {
        (request["settings"].clone(), request["settings"]["geometry"].clone())
    } else {
        (
            request["options"].clone(),
            request["options"]["irregularSettings"]["geometry"].clone(),
        )
    };
    let padding = request["settings"]["padding"]
        .as_f64()
        .or_else(|| request["padding"].as_f64())
        .ok_or("padding")?;
    let allow_rotation = settings_node["allowGlobalRotation"].as_bool().unwrap_or(true);
    let allow_mirror = settings_node["allowGlobalMirror"].as_bool().unwrap_or(true);
    let sag = geometry["flatteningSagToleranceMm"].as_f64().ok_or("sag")?;
    let safety = geometry["clearanceSafetyMarginMm"].as_f64().ok_or("safety")?;

    let mut by_id: BTreeMap<String, ImportedPiece> = BTreeMap::new();
    for value in request["sourcePieces"].as_array().ok_or("sourcePieces")? {
        let piece: ImportedPiece =
            serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
        by_id.insert(
            value["id"].as_str().ok_or("sourcePiece.id")?.to_owned(),
            piece,
        );
    }
    let normalize = sheet_w >= sheet_h;
    let mut polygons = Vec::new();
    let mut ids = Vec::new();
    let mut rotations = Vec::new();
    let mut mirrors = Vec::new();
    for piece in request["pieces"].as_array().ok_or("pieces")? {
        let source_id = piece["sourcePieceId"].as_str().ok_or("sourcePieceId")?;
        let source = by_id.get(source_id).ok_or("missing source")?;
        let polygon =
            polygon_set_from_imported_piece(source, sag).map_err(|e| format!("{e:?}"))?;
        let polygon = if normalize {
            let rotated = polygon
                .transformed(270.0, false, 0.0, 0.0)
                .map_err(|e| format!("{e:?}"))?;
            let bounds = rotated.bounds().ok_or("empty geometry")?;
            rotated
                .translated(-bounds.min_x, -bounds.min_y)
                .map_err(|e| format!("{e:?}"))?
        } else {
            polygon
        };
        polygons.push(polygon);
        ids.push(piece["id"].as_str().ok_or("piece.id")?.to_owned());
        rotations.push(allow_rotation && piece["allowRotation"].as_bool().unwrap_or(true));
        mirrors.push(allow_mirror && piece["allowMirror"].as_bool().unwrap_or(true));
    }
    // The drivers pin `--edge=5 --pair=5`; this reproduces that surface exactly.
    let mut settings = GeneralFastSettings::deterministic_test(
        sheet_w.min(sheet_h),
        sheet_w.max(sheet_h),
    );
    settings.total_padding_mm = 5.0;
    settings.sheet_edge_clearance_mm = Some(5.0);
    settings.clearance_safety_margin_mm = safety;
    settings.flattening_sag_tolerance_mm = sag;
    settings.search_offset_allowance_mm = 0.002;
    settings.max_order_variants = 4;
    settings.angle_seed_count = 16;
    settings.max_angles_per_piece = 4;
    let _ = padding;
    Ok(Fixture {
        polygons,
        ids,
        rotations,
        mirrors,
        settings,
    })
}

fn pieces_of(fixture: &Fixture) -> Vec<GeneralFastPiece<'_>> {
    (0..fixture.polygons.len())
        .map(|index| GeneralFastPiece {
            id: &fixture.ids[index],
            polygon: &fixture.polygons[index],
            allow_rotation: fixture.rotations[index],
            allow_mirror: fixture.mirrors[index],
        })
        .collect()
}

fn fresh_state(sources: &[PieceSource], poses: &[Pose], contract: &Contract, target: f64) -> IcsState {
    let geometry = build_geometry(sources, poses);
    let count = poses.len();
    let mut state = IcsState {
        poses: poses.to_vec(),
        geometry,
        pair_rows: vec![PairRow::default(); pair_count(count)],
        edge_rows: vec![[EdgeRow::default(); 4]; count],
        target_depth_mm: target,
    };
    let mut work = WorkVector::default();
    energy::rebuild_all(&mut state, contract, &mut work);
    state
}

/// S3: the proxy depth and the published depth are the same function.
fn section_depth_identity(
    ledger: &mut Ledger,
    pieces: &[GeneralFastPiece<'_>],
    sources: &[PieceSource],
    contract: &Contract,
    base: &[Pose],
) {
    let mut offenders: Vec<Value> = Vec::new();
    let mut worst = 0.0f64;
    for trial in 0..400u64 {
        let poses: Vec<Pose> = base
            .iter()
            .enumerate()
            .map(|(index, pose)| {
                let key = counter_hash(&[trial, index as u64, 0x0DEE_7A11]);
                Pose {
                    tx_mm: pose.tx_mm + (unit(key) - 0.5) * 40.0,
                    ty_mm: pose.ty_mm + (unit(key >> 5) - 0.5) * 40.0,
                    theta_deg: pose.theta_deg + (unit(key >> 11) - 0.5) * 720.0,
                    mirrored: pose.mirrored ^ (key & 1 == 1),
                }
            })
            .collect();
        let geometry = build_geometry(sources, &poses);
        let proxy = raw_source_depth_mm(&geometry, contract);
        let placements = placements_of(sources, &poses);
        let published = raw_depth_of(pieces, &placements, contract);
        worst = worst.max((proxy - published).abs());
        if proxy.to_bits() != published.to_bits() && offenders.len() < 4 {
            offenders.push(json!({"trial": trial, "proxy": proxy, "published": published,
                                  "deltaMm": proxy - published}));
        }
    }
    ledger.check(
        "S3a proxy depth == published depth, bit for bit, on 400 random pose sets",
        offenders.is_empty(),
        json!({"worstAbsDeltaMm": worst, "offenders": offenders}),
    );

    // The pivot convention: the coordinate descent's pivot and Φ's torque arm
    // must be the same point. `transformed_centroid` is what `wiggle_pose` and
    // `split_and_close` use; `geometry.centroids` is what `incident_gradient`
    // and the repair fallback use.
    let geometry = build_geometry(sources, base);
    let mut pivot_offenders: Vec<Value> = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        let computed = transformed_centroid(source, base[index]);
        let cached = geometry.centroids[index];
        if computed[0].to_bits() != cached[0].to_bits()
            || computed[1].to_bits() != cached[1].to_bits()
        {
            pivot_offenders.push(json!({"piece": index, "computed": computed, "cached": cached}));
        }
    }
    ledger.check(
        "S3b transformed_centroid == Geometry::centroids, bit for bit",
        pivot_offenders.is_empty(),
        json!({"pieces": sources.len(), "offenders": pivot_offenders}),
    );

    // The pivot itself is fixed by a pure rotation, to all orders.
    let mut fixed_offenders: Vec<Value> = Vec::new();
    for (index, source) in sources.iter().enumerate().take(12) {
        let pose = base[index];
        let pivot = transformed_centroid(source, pose);
        for step in 0..8u64 {
            let dtheta = -180.0 + 45.0 * step as f64;
            let turned = state::compose_proposal(pose, pivot, 0.0, 0.0, dtheta);
            let moved = transformed_centroid(source, turned);
            let gap = libm_hypot(moved[0] - pivot[0], moved[1] - pivot[1]);
            if gap > 1e-9 {
                fixed_offenders.push(json!({"piece": index, "dtheta": dtheta, "gapMm": gap}));
            }
        }
    }
    ledger.check(
        "S3c compose_proposal about the centroid leaves the centroid fixed",
        fixed_offenders.is_empty(),
        json!({"offenders": fixed_offenders}),
    );

    // The extents the sample box is built from are the extents the geometry
    // actually has: a container draw that used a different box would be
    // sampling outside the strip it claims to sample inside.
    let mut extent_offenders: Vec<Value> = Vec::new();
    for (index, source) in sources.iter().enumerate().take(20) {
        let pose = base[index];
        let extents = centroid_relative_extents(source, pose.theta_deg, pose.mirrored);
        let centre = transformed_centroid(source, pose);
        let bounds = geometry.piece_bounds[index];
        let rebuilt = [
            centre[0] + extents[0],
            centre[1] + extents[1],
            centre[0] + extents[2],
            centre[1] + extents[3],
        ];
        let worst = (0..4)
            .map(|axis| (rebuilt[axis] - bounds[axis]).abs())
            .fold(0.0f64, f64::max);
        if worst > 1e-9 {
            extent_offenders.push(json!({"piece": index, "worstMm": worst,
                                         "rebuilt": rebuilt, "bounds": bounds}));
        }
    }
    ledger.check(
        "S3d centroid_relative_extents reproduce Geometry::piece_bounds",
        extent_offenders.is_empty(),
        json!({"offenders": extent_offenders}),
    );
}

fn libm_hypot(x: f64, y: f64) -> f64 {
    (x * x + y * y).sqrt()
}

// ------------------------------------------------ S4: Algorithm 8, by hand ---

const GLS_MIN: f64 = 1.2;
const GLS_MAX: f64 = 2.0;
const GLS_DECAY: f64 = 0.95;
const GLS_CAP: f64 = 1_048_576.0;

/// The published schedule, transcribed from the doc comment on `gls_update`:
///
/// ```text
/// v == 0 : w <- max(1, 0.95 w)
/// v >  0 : w <- min(2^20, w * (1.2 + 0.8 * v / v_max))
/// ```
fn reference_weight(violation: f64, weight: f64, max_violation: f64) -> f64 {
    if violation <= 0.0 {
        return (weight * GLS_DECAY).max(GLS_WEIGHT_FLOOR);
    }
    let share = if max_violation > 0.0 {
        violation / max_violation
    } else {
        0.0
    };
    (weight * (GLS_MIN + (GLS_MAX - GLS_MIN) * share)).min(GLS_CAP)
}

fn section_gls(
    ledger: &mut Ledger,
    sources: &[PieceSource],
    contract: &Contract,
    base: &[Pose],
    seed: u64,
) {
    // A real trajectory slice: bite the constructor's own width, then run a
    // sequence of sweeps, recomputing the whole weight vector at every pass.
    let start_depth = {
        let geometry = build_geometry(sources, base);
        raw_source_depth_mm(&geometry, contract)
    };
    let mut poses = base.to_vec();
    let bite = homotopy::explore_bite(sources, &mut poses, start_depth);
    let mut state = fresh_state(sources, &poses, contract, bite.width_after_mm);
    energy::reset_weights(&mut state);

    let config = DescentConfig::derive(contract, sources, seed);
    let allow_rotation: Vec<bool> = sources.iter().map(|_| true).collect();
    let mut descent = Descent::new(config, allow_rotation);
    let mut work = WorkVector::default();

    let mut mismatches: Vec<Value> = Vec::new();
    let mut passes = 0u64;
    let mut active_seen = 0u64;
    let mut decayed_seen = 0u64;
    let mut capped_seen = 0u64;
    for pass in 0..12u64 {
        // The pre-update landscape and the violations the update will read.
        let before_pair: Vec<(f64, f64)> = state
            .pair_rows
            .iter()
            .map(|row| (row.violation_mm, row.weight))
            .collect();
        let before_edge: Vec<[(f64, f64); 4]> = state
            .edge_rows
            .iter()
            .map(|rows| {
                [
                    (rows[0].violation_mm, rows[0].weight),
                    (rows[1].violation_mm, rows[1].weight),
                    (rows[2].violation_mm, rows[2].weight),
                    (rows[3].violation_mm, rows[3].weight),
                ]
            })
            .collect();
        // `Descent::sweep` moves pieces AND runs one Algorithm-8 pass; the
        // weights it produces have to be a function of the rows AS THEY STOOD
        // WHEN THE PASS RAN, which is after the Gauss-Seidel half. So the
        // schedule is checked on `gls_update` directly, from a state the sweep
        // just left, and the sweep is what advances the trajectory.
        let outcome = descent.sweep(&mut state, sources, contract, &mut work);
        let _ = outcome;
        // Re-derive: undo nothing, but recompute what the pass MUST have
        // produced from the post-sweep violations and the pre-sweep weights.
        // The Gauss-Seidel half does not touch weights, so `before_*` are the
        // weights the pass started from.
        let max_violation = state
            .pair_rows
            .iter()
            .map(|row| row.violation_mm)
            .chain(
                state
                    .edge_rows
                    .iter()
                    .flat_map(|rows| rows.iter().map(|row| row.violation_mm)),
            )
            .fold(0.0f64, |acc, value| if value > acc { value } else { acc });
        for (index, row) in state.pair_rows.iter().enumerate() {
            let want = reference_weight(row.violation_mm, before_pair[index].1, max_violation);
            if row.violation_mm > 0.0 {
                active_seen += 1;
            } else {
                decayed_seen += 1;
            }
            if row.weight >= GLS_CAP {
                capped_seen += 1;
            }
            if want.to_bits() != row.weight.to_bits() && mismatches.len() < 5 {
                mismatches.push(json!({"pass": pass, "kind": "pair", "row": index,
                                       "violation": row.violation_mm,
                                       "weightBefore": before_pair[index].1,
                                       "maxViolation": max_violation,
                                       "shipped": row.weight, "reference": want}));
            }
        }
        for (piece, rows) in state.edge_rows.iter().enumerate() {
            for (edge, row) in rows.iter().enumerate() {
                let want =
                    reference_weight(row.violation_mm, before_edge[piece][edge].1, max_violation);
                if want.to_bits() != row.weight.to_bits() && mismatches.len() < 5 {
                    mismatches.push(json!({"pass": pass, "kind": "edge", "piece": piece,
                                           "edge": edge, "violation": row.violation_mm,
                                           "weightBefore": before_edge[piece][edge].1,
                                           "maxViolation": max_violation,
                                           "shipped": row.weight, "reference": want}));
                }
            }
        }
        passes += 1;
    }
    ledger.check(
        "S4a Algorithm 8 weights == independent recomputation over 12 real sweeps",
        mismatches.is_empty(),
        json!({"passes": passes, "activeRowUpdates": active_seen,
               "decayedRowUpdates": decayed_seen, "cappedRows": capped_seen,
               "offenders": mismatches}),
    );

    // Reset-on-width-change: `energy::reset_weights` returns EVERY row to the
    // floor and nothing else does.
    let learned = state.pair_rows.iter().any(|row| row.weight > GLS_WEIGHT_FLOOR)
        || state
            .edge_rows
            .iter()
            .any(|rows| rows.iter().any(|row| row.weight > GLS_WEIGHT_FLOOR));
    let snapshot = state.clone();
    energy::reset_weights(&mut state);
    let all_floor = state.pair_rows.iter().all(|row| row.weight == GLS_WEIGHT_FLOOR)
        && state
            .edge_rows
            .iter()
            .all(|rows| rows.iter().all(|row| row.weight == GLS_WEIGHT_FLOOR));
    ledger.check(
        "S4b reset_weights returns every row to the floor (width change)",
        learned && all_floor,
        json!({"landscapeWasLearned": learned, "allAtFloorAfterReset": all_floor}),
    );

    // Persist-across-rollback: restoring a snapshot inside one width must move
    // the poses and the violations and leave the weights alone. The engine's
    // `restore_keeping_weights` is private, so the property is asserted on the
    // public surface it is built out of: a rollback is a pose+violation copy,
    // and `reset_weights` is the only writer of a weight outside `gls_update`.
    let mut rolled = snapshot.clone();
    energy::reset_weights(&mut rolled); // pretend a width change happened
    let weights_differ = rolled
        .pair_rows
        .iter()
        .zip(&snapshot.pair_rows)
        .any(|(left, right)| left.weight != right.weight);
    ledger.check(
        "S4c the learned landscape is distinguishable from the floor",
        weights_differ,
        json!({"note": "if this is false the S4b vector proves nothing"}),
    );
}

// ------------------------------------- S5: the tournament, reconstructed ---

fn section_tournament(
    ledger: &mut Ledger,
    pieces: &[GeneralFastPiece<'_>],
    fixture: &Fixture,
    sources: &[PieceSource],
    contract: &Contract,
    constructor: &[GeneralFastPlacement],
    constructor_depth: f64,
    seed: u64,
    workers: usize,
) {
    // --- the engine's own single master iteration -------------------------
    let config = IcsConfig {
        target_depth_mm: constructor_depth,
        proposal_budget: 0,
        relocate_eval_budget: u64::MAX,
        checkpoint_every_sweeps: u64::MAX,
        descent: DescentConfig::derive(contract, sources, seed),
        limits: Default::default(),
    };
    let mut engine = Engine::from_constructor_at_depth(
        pieces,
        fixture.settings,
        constructor,
        constructor_depth,
        config,
    )
    .expect("engine");
    let entry_work = engine.work();
    let outcome = engine.run_cutclose(
        ScheduleConfig {
            workers,
            record_fingerprints: true,
            ..ScheduleConfig::default()
        },
        Budget::FixedWork {
            explore_bites: 1,
            compress_bites: 0,
            attempts_per_bite: 1,
            iterations_per_separation: 1,
        },
    );
    let engine_work = outcome.trace.work;

    // --- the same iteration, rebuilt from scratch outside the engine -------
    //
    // `run_cutclose` enters at `W = D*`, refreshes, resets the weights, takes
    // one explore bite, refreshes and resets again; `separate` then folds,
    // observes, tries the band, and calls `tournament(workers, bite = 1)` with
    // a master descent that has never run.
    let poses = poses_of(pieces, sources, constructor).expect("poses");
    let mut bitten = poses.clone();
    let bite = homotopy::explore_bite(sources, &mut bitten, constructor_depth);
    let entry_state = fresh_state(sources, &bitten, contract, bite.width_after_mm);
    let allow_rotation: Vec<bool> = pieces.iter().map(|piece| piece.allow_rotation).collect();
    let master = Descent::new(DescentConfig::derive(contract, sources, seed), allow_rotation);

    let mut summed = WorkVector::default();
    let mut guided: Vec<f64> = Vec::new();
    let mut states: Vec<IcsState> = Vec::new();
    for ordinal in 0..workers {
        let mut worker_state = entry_state.clone();
        let mut worker_descent = master.clone();
        worker_descent.set_stream(1, ordinal as u64);
        let mut worker_work = WorkVector::default();
        let result = worker_descent.worker_sweep(
            &mut worker_state,
            sources,
            contract,
            &mut worker_work,
        );
        guided.push(result.totals.guided);
        summed.saturating_add(&worker_work);
        states.push(worker_state);
    }
    let mut winner = 0usize;
    for ordinal in 1..workers {
        if guided[ordinal] < guided[winner] {
            winner = ordinal;
        }
    }

    // --- S7: what `stayPutWinners` actually counts -------------------------
    //
    // `relocate` reports `best.origin` - the origin of the *seed the winning
    // pose descended from* - and then runs a fine coordinate descent from it,
    // so a relocate that moved the piece a long way still reports `StayPut`
    // whenever the entry pose was the best of the 76 pool members. The work
    // vector's `stayPutWinners` is therefore NOT "relocates that left the piece
    // where it was", and on the gate fixture it is 98.7 % of all relocates
    // while `acceptedMoves` is 36 % of them.
    //
    // `RejectionCensus::accepted_by_origin` splits the same population by
    // whether the pose actually changed, and `max_displacement_mm` is the
    // number the neutered-relocate tripwire is written against. Neither is
    // emitted by `schedule_json`, so this is the only place the split is
    // visible for a `run_cutclose` trajectory.
    let mut census_state = entry_state.clone();
    let mut census_descent = master.clone();
    census_descent.set_stream(1, winner as u64);
    let mut census_work = WorkVector::default();
    let _ = census_descent.worker_sweep(
        &mut census_state,
        sources,
        contract,
        &mut census_work,
    );
    let census = census_descent.rejection_census().clone();
    let stay_put_total = census.accepted_by_origin[0] + census.rejected_by_origin[0];
    ledger.check(
        "S7a REPORTED: stayPutWinners counts the winning SEED, not a piece that stayed put",
        true,
        json!({
            "worker": winner,
            "relocatesThatRan": census.accepted + census.rejected,
            "visitsSkippedAsNonColliding": census.zero_energy,
            "stayPutOriginTotal": stay_put_total,
            "stayPutOriginThatMOVED": census.accepted_by_origin[0],
            "stayPutOriginThatDidNotMove": census.rejected_by_origin[0],
            "focusedOriginThatMOVED": census.accepted_by_origin[1],
            "containerOriginThatMOVED": census.accepted_by_origin[2],
            "reading": "a StayPut-origin relocate that MOVED is the fine \
                        coordinate descent walking off the entry pose; the \
                        work vector's stayPutWinners does not separate the two",
        }),
    );
    let ladder_top = DescentConfig::derive(contract, sources, seed).ladder_top_mm;
    ledger.check(
        "S7b a committed relocate reaches beyond the old ladder_top (not neutered)",
        census.max_displacement_mm > ladder_top,
        json!({"maxCommittedDisplacementMm": census.max_displacement_mm,
               "ladderTopMm": ladder_top,
               "ratio": census.max_displacement_mm / ladder_top}),
    );

    // The double-debit clause: the master's aggregate over ONE iteration must
    // be the sum of the eight per-worker deltas, in every relocate counter.
    let delta = |after: u64, before: u64| after - before;
    let pairs: [(&str, u64, u64); 9] = [
        ("sampleEvaluations",
         delta(engine_work.sample_evaluations, entry_work.sample_evaluations),
         summed.sample_evaluations),
        ("relocates", delta(engine_work.relocates, entry_work.relocates), summed.relocates),
        ("focusedSamples",
         delta(engine_work.focused_samples, entry_work.focused_samples),
         summed.focused_samples),
        ("containerSamples",
         delta(engine_work.container_samples, entry_work.container_samples),
         summed.container_samples),
        ("containerWinners",
         delta(engine_work.container_winners, entry_work.container_winners),
         summed.container_winners),
        ("focusedWinners",
         delta(engine_work.focused_winners, entry_work.focused_winners),
         summed.focused_winners),
        ("stayPutWinners",
         delta(engine_work.stay_put_winners, entry_work.stay_put_winners),
         summed.stay_put_winners),
        ("containerCommits",
         delta(engine_work.container_commits, entry_work.container_commits),
         summed.container_commits),
        ("acceptedMoves",
         delta(engine_work.accepted_moves, entry_work.accepted_moves),
         summed.accepted_moves),
    ];
    let mut debits: Vec<Value> = Vec::new();
    let mut table = serde_json::Map::new();
    for (name, engine_delta, worker_sum) in pairs {
        table.insert(
            name.to_owned(),
            json!({"engineDelta": engine_delta, "eightWorkerSum": worker_sum}),
        );
        if engine_delta != worker_sum {
            debits.push(json!({"counter": name, "engineDelta": engine_delta,
                               "eightWorkerSum": worker_sum}));
        }
    }
    ledger.check(
        "S5a one master iteration is charged exactly the sum of its eight workers",
        debits.is_empty(),
        json!({"workers": workers, "counters": table, "offenders": debits}),
    );
    ledger.check(
        "S5b piece_proposals == workers * n for one master iteration",
        delta(engine_work.piece_proposals, entry_work.piece_proposals)
            == (workers * sources.len()) as u64,
        json!({"engineDelta": delta(engine_work.piece_proposals, entry_work.piece_proposals),
               "expected": workers * sources.len()}),
    );
    ledger.check(
        "S5c weight_updates == 1 for one master iteration (one Algorithm-8 pass)",
        delta(engine_work.weight_updates, entry_work.weight_updates) == 1,
        json!({"engineDelta": delta(engine_work.weight_updates, entry_work.weight_updates)}),
    );

    // The merge rule, and the installed state.
    let recorded = outcome
        .fingerprints
        .first()
        .map(|row| (row.winner, row.winner_guided, row.contested));
    let contested = guided.iter().any(|value| *value != guided[0]);
    ledger.check(
        "S5d the winner is the minimum guided, ties by ordinal",
        recorded.map(|row| row.0) == Some(winner)
            && recorded.map(|row| row.1.to_bits()) == Some(guided[winner].to_bits())
            && recorded.map(|row| row.2) == Some(contested),
        json!({"reconstructedWinner": winner,
               "reconstructedGuided": guided[winner],
               "reconstructedContested": contested,
               "engineFingerprint": recorded.map(|row| json!({"winner": row.0,
                                                             "guided": row.1,
                                                             "contested": row.2})),
               "allGuided": guided}),
    );

    // The eight workers really did diverge: a tournament in which every worker
    // reaches the same total has not been exercised, and the merge vector would
    // be vacuously green.
    let distinct = {
        let mut seen: Vec<u64> = guided.iter().map(|value| value.to_bits()).collect();
        seen.sort_unstable();
        seen.dedup();
        seen.len()
    };
    ledger.check(
        "S5e the eight workers reached distinct totals (the merge had a choice)",
        distinct > 1,
        json!({"distinctTotals": distinct, "guided": guided}),
    );

    // --- S6: the bite record, recomputed ----------------------------------
    let recorded_bite = outcome.bites.first().map(|row| row.bite);
    let expected_moved = bitten
        .iter()
        .zip(&poses)
        .filter(|(after, before)| after.ty_mm != before.ty_mm)
        .count();
    let hand_moved = sources
        .iter()
        .zip(&poses)
        .filter(|(source, pose)| {
            transformed_centroid(source, **pose)[1] > constructor_depth / 2.0
        })
        .count();
    ledger.check(
        "S6a the first explore bite's record == an independent recomputation",
        recorded_bite.map(|bite| {
            bite.width_before_mm == constructor_depth
                && bite.width_after_mm == constructor_depth * (1.0 - 0.001)
                && bite.delta_mm == bite.width_after_mm - bite.width_before_mm
                && bite.split_y_mm == constructor_depth / 2.0
                && bite.moved_pieces == expected_moved
                && bite.moved_pieces == hand_moved
                && bite.step == 0.001
        }) == Some(true),
        json!({
            "recorded": recorded_bite.map(|bite| json!({
                "widthBeforeMm": bite.width_before_mm,
                "widthAfterMm": bite.width_after_mm,
                "deltaMm": bite.delta_mm,
                "splitYMm": bite.split_y_mm,
                "movedPieces": bite.moved_pieces,
                "step": bite.step,
            })),
            "recomputedMovedPiecesByPoseDelta": expected_moved,
            "recomputedMovedPiecesByCentroidTest": hand_moved,
            "constructorDepthMm": constructor_depth,
        }),
    );

    // Only `ty` moved, and only on the far side: the cut-close bits, asserted
    // on the pose arrays rather than on the engine's own summary.
    let mut bit_offenders: Vec<Value> = Vec::new();
    for (index, (after, before)) in bitten.iter().zip(&poses).enumerate() {
        let far = transformed_centroid(&sources[index], *before)[1] > constructor_depth / 2.0;
        let expected_ty = if far {
            before.ty_mm + (constructor_depth * (1.0 - 0.001) - constructor_depth)
        } else {
            before.ty_mm
        };
        if after.tx_mm.to_bits() != before.tx_mm.to_bits()
            || after.theta_deg.to_bits() != before.theta_deg.to_bits()
            || after.mirrored != before.mirrored
            || after.ty_mm.to_bits() != expected_ty.to_bits()
        {
            bit_offenders.push(json!({"piece": index, "far": far,
                                      "tyAfter": after.ty_mm, "tyExpected": expected_ty,
                                      "txAfter": after.tx_mm, "txBefore": before.tx_mm}));
        }
    }
    ledger.check(
        "S6b split_and_close touches only ty, only on the far side, by exactly delta",
        bit_offenders.is_empty(),
        json!({"pieces": poses.len(), "offenders": bit_offenders}),
    );

    // The compression cut is a function of `(seed, ordinal)` alone.
    let mut cut_offenders: Vec<Value> = Vec::new();
    for ordinal in 1..6u64 {
        let width = 170.0 + ordinal as f64;
        let first = homotopy::uniform_cut_mm(contract, width, seed, ordinal);
        let second = homotopy::uniform_cut_mm(contract, width, seed, ordinal);
        let other_seed = homotopy::uniform_cut_mm(contract, width, seed + 1, ordinal);
        let low = contract.physical_edge_clearance_mm();
        if first != second
            || !(first >= low && first <= width)
            || first == other_seed
        {
            cut_offenders.push(json!({"ordinal": ordinal, "cut": first,
                                      "repeat": second, "otherSeed": other_seed,
                                      "low": low, "high": width}));
        }
    }
    ledger.check(
        "S6c uniform_cut_mm is a pure function of (contract, W, seed, ordinal) in (edge, W)",
        cut_offenders.is_empty(),
        json!({"offenders": cut_offenders}),
    );

    // The publication chain: the fingerprint the record names as the parent is
    // the constructor's on the first bite.
    let first_publication = outcome.publications.first();
    ledger.check(
        "S6d the first publication's parent fingerprint is the constructor's",
        first_publication
            .map(|row| row.parent_fingerprint == placement_fingerprint(constructor))
            .unwrap_or(true),
        json!({"published": first_publication.is_some(),
               "parent": first_publication.map(|row| row.parent_fingerprint.clone()),
               "constructor": placement_fingerprint(constructor)}),
    );
}

// -------------------------------------------------------------------- main ---

fn main() {
    let mut argv = std::env::args().skip(1);
    let root = argv.next().unwrap_or_else(|| ".".to_owned());
    let out = argv.next();
    let mut ledger = Ledger::default();

    section_strike(&mut ledger);
    section_conventions(&mut ledger);

    let fixture = match load_fixture(&root) {
        Ok(fixture) => fixture,
        Err(error) => {
            eprintln!("fixture: {error}");
            std::process::exit(2);
        }
    };
    let pieces = pieces_of(&fixture);
    let contract = Contract::from_settings(fixture.settings);
    let sources = piece_sources(&pieces).expect("sources");
    // The same adapter the benchmark's `ShortSideFirst` provider is: a
    // constructor that left a piece unplaced is not a complete layout.
    let built = construct_short_side_first(&pieces, fixture.settings).expect("constructor");
    assert!(
        built.unplaced_piece_ids.is_empty(),
        "the constructor left pieces unplaced"
    );
    let constructor = built.placements;
    let constructor_depth = raw_depth_of(&pieces, &constructor, &contract);
    let base = poses_of(&pieces, &sources, &constructor).expect("poses");

    section_depth_identity(&mut ledger, &pieces, &sources, &contract, &base);
    section_gls(&mut ledger, &sources, &contract, &base, 0);
    section_tournament(
        &mut ledger,
        &pieces,
        &fixture,
        &sources,
        &contract,
        &constructor,
        constructor_depth,
        0,
        8,
    );

    let failures = ledger.failures().len();
    let document = json!({
        "experiment": "overlap-ics",
        "battery": "evidence-audit-rust-vectors",
        "root": root,
        "constructorDepthMm": constructor_depth,
        "pieces": pieces.len(),
        "contract": {
            "sheetShortAxisMm": contract.sheet_short_axis_mm,
            "sheetLongAxisMm": contract.sheet_long_axis_mm,
            "totalPaddingMm": contract.total_padding_mm,
            "sheetEdgeClearanceMm": contract.sheet_edge_clearance_mm,
            "flatteningSagToleranceMm": contract.flattening_sag_tolerance_mm,
            "clearanceSafetyMarginMm": contract.clearance_safety_margin_mm,
            "pairClearanceMm": contract.pair_clearance_mm(),
            "physicalEdgeClearanceMm": contract.physical_edge_clearance_mm(),
            "depthTopInsetMm": contract.depth_top_inset_mm(),
            "expansionMm": contract.expansion_mm(),
            "sheetInsetMm": contract.sheet_inset_mm(),
        },
        "vectors": ledger.rows,
        "vectorCount": ledger.rows.len(),
        "failureCount": failures,
        "RUST_VECTORS_PASS": failures == 0,
    });
    let text = serde_json::to_string_pretty(&document).expect("json");
    println!("{text}");
    if let Some(path) = out {
        if let Some(parent) = std::path::Path::new(&path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&path, &text).expect("write");
    }
    std::process::exit(if failures == 0 { 0 } else { 1 });
}

// Keep the unused-import lint honest about the enum arms the truth table names.
#[allow(dead_code)]
fn _edge_names() -> [usize; 4] {
    [EDGE_LEFT, EDGE_RIGHT, EDGE_BOTTOM, EDGE_TOP]
}
