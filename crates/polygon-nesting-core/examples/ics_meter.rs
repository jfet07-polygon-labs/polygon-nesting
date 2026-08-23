//! **The currency calibration and its reject check, as a runnable cell.**
//!
//! docs/economics-round-spec.md, funded change 3: *"B/E/R/D from timing-only
//! microbenchmarks on all three fixtures, conservative rounding; REJECT the
//! currency if wall-prediction error >10 % on any transfer fixture."* This is
//! the runnable half of that sentence. The evidence agent runs it; nothing in
//! the engine calls it, and it cannot change a trajectory because it never
//! runs one - it reads documents another process wrote.
//!
//!     cargo build --release --features overlap-ics,ics-profile \
//!         --example overlap_ics_benchmark
//!     # ... one `--cell=cutclose` document per fixture ...
//!     cargo run --release --features overlap-ics --example ics_meter -- \
//!         --out=/tmp/currency.json \
//!         --cell=mixed-61=/tmp/mixed61.json \
//!         --cell=shapes-17=/tmp/shapes17.json \
//!         --cell=triangle-20=/tmp/triangle20.json
//!
//! Exit status is the verdict, read directly and never through a pipe:
//!
//! * `0` - the check ran and the currency **transfers**: no fixture pair is
//!   off by more than 10 %.
//! * `1` - the check ran and the currency is **rejected**. That is a result,
//!   not a script failure: the document says which pair rejected it and by how
//!   much.
//! * `2` - the check could not run at all. The input is missing a fixture, or
//!   was produced by a build without `ics-profile` and therefore carries no
//!   nanoseconds.
//!
//! The arithmetic is **not** here. Every number this prints comes from
//! `search::overlap_ics_meter::currency`, which has its own unit vectors, so
//! this file cannot pass by agreeing with a duplicate of the rule.

use std::collections::BTreeMap;

use polygon_nesting_core::search::overlap_ics_meter::currency::{
    calibrate, calibrate_prime, timings_from_rows, transfer_check, transfer_check_prime,
    BiteProfileRow, CellWall, Currency, FixtureCell, FixtureCellPrime, FixtureTimingInput,
    WorkTerms, WorkTermsPrime,
};
use serde_json::{json, Value};

/// The three fixtures the spec names. All three, or the calibration refuses.
const FIXTURES: [&str; 3] = ["mixed-61", "shapes-17", "triangle-20"];

fn u64_at(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or_default()
}

/// One `--cell=<fixture>=<path>` document, reduced to the rows the harness
/// reads. Everything else in the document is ignored on purpose: this cell has
/// no opinion about depths.
fn read_cell(fixture: &str, path: &str) -> Result<(Vec<BiteProfileRow>, CellWall, f64), String> {
    let text = std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
    let document: Value =
        serde_json::from_str(&text).map_err(|error| format!("{path}: {error}"))?;
    let search_seconds = document
        .pointer("/wall/searchSeconds")
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("{path}: no `wall.searchSeconds`"))?;
    let bites = document
        .pointer("/outcome/bites")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{path}: no `outcome.bites`"))?;
    let mut rows = Vec::with_capacity(bites.len());
    for bite in bites {
        let Some(profile) = bite.get("profile") else {
            return Err(format!("{path}: a bite has no `profile`"));
        };
        if !profile
            .get("measured")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err(format!(
                "{path}: the document carries no nanoseconds. Build the cell with \
                 `--features overlap-ics,ics-profile`."
            ));
        }
        rows.push(BiteProfileRow {
            fixture: fixture.to_owned(),
            iterations: u64_at(profile, "iterations"),
            barrier_to_barrier_ns: u64_at(profile, "barrierToBarrierNs"),
            prep_ns: u64_at(profile, "prepNs"),
            dispatch_ns: u64_at(profile, "dispatchNs"),
            sweep_critical_ns: u64_at(profile, "sweepCriticalNs"),
            sweep_total_ns: u64_at(profile, "sweepTotalNs"),
            merge_gls_ns: u64_at(profile, "mergeGlsNs"),
            exact_ns: u64_at(profile, "exactNs"),
            band_fold_ns: u64_at(profile, "bandFoldNs"),
            snapshot_ns: u64_at(profile, "snapshotNs"),
            residual_ns: u64_at(profile, "residualNs"),
            sample_evaluations: u64_at(profile, "sampleEvaluations"),
            exact_calls: u64_at(profile, "exactCalls"),
            repair_rows: u64_at(profile, "repairRows"),
            disruption_moves: u64_at(profile, "disruptionMoves"),
            // **`U'`'s per-bite counter (rider (i)).** It is a sibling of
            // `profile`, not a field of it, because it is not a phase timer and
            // never was: `BiteRecord::published` is the trajectory's own
            // publication record, emitted as a bool per bite by every build the
            // campaign has ever run, at no clock cost and inside the whole-
            // document two-process bit comparison. `published_bites.py` proves
            // the vector bit-identical across two processes on all three
            // fixed-work cells **before** `P` is fitted; nothing here would be
            // more instrumented for having been re-counted a second time
            // somewhere else.
            published_bites: u64::from(
                bite.get("published")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            ),
        });
    }
    if rows.is_empty() {
        return Err(format!("{path}: no bites, so nothing to price"));
    }
    let wall = CellWall {
        fixture: fixture.to_owned(),
        // The seconds the driver measured around the phases, in the same unit
        // the barriers are in.
        search_ns: (search_seconds * 1e9) as u64,
    };
    Ok((rows, wall, search_seconds))
}

fn terms_of(rows: &[BiteProfileRow]) -> WorkTerms {
    let mut terms = WorkTerms::default();
    for row in rows {
        terms.add(&WorkTerms {
            sample_evaluations: row.sample_evaluations,
            master_batches: row.iterations,
            actual_publication_attempt_calls: row.exact_calls,
            repair_rows: row.repair_rows,
            disruption_moves: row.disruption_moves,
        });
    }
    terms
}

/// `U'`'s counted terms for one fixture. `repair_rows` has no field to go in.
fn terms_prime_of(rows: &[BiteProfileRow]) -> WorkTermsPrime {
    let mut terms = WorkTermsPrime::default();
    for row in rows {
        terms.add(&WorkTermsPrime {
            sample_evaluations: row.sample_evaluations,
            master_batches: row.iterations,
            exact_checkpoint_calls: row.exact_calls,
            published_bites: row.published_bites,
            disruption_moves: row.disruption_moves,
        });
    }
    terms
}

/// One fixture's summed timings, as `calibrate_prime` reads them. Every field
/// is a sum over that fixture's bites except `search_ns`, which is the driver's
/// own wall around the phases and is the only number here the engine did not
/// produce.
fn timing_input_of(fixture: &str, rows: &[BiteProfileRow], wall: &CellWall) -> FixtureTimingInput {
    let sum = |pick: fn(&BiteProfileRow) -> u64| -> u64 {
        rows.iter().fold(0u64, |acc, row| acc.saturating_add(pick(row)))
    };
    // The per-batch overhead `B` prices: everything in the barrier that is not
    // a worker sweep and not the exact region.
    let overhead = sum(|row| {
        row.prep_ns
            .saturating_add(row.dispatch_ns)
            .saturating_add(row.merge_gls_ns)
            .saturating_add(row.band_fold_ns)
            .saturating_add(row.snapshot_ns)
            .saturating_add(row.residual_ns)
    });
    FixtureTimingInput {
        fixture: fixture.to_owned(),
        sweep_critical_ns: sum(|row| row.sweep_critical_ns),
        batch_overhead_ns: overhead,
        exact_ns: sum(|row| row.exact_ns),
        barrier_to_barrier_ns: sum(|row| row.barrier_to_barrier_ns),
        search_ns: wall.search_ns,
        sample_evaluations: sum(|row| row.sample_evaluations),
        iterations: sum(|row| row.iterations),
        exact_checkpoint_calls: sum(|row| row.exact_calls),
        published_bites: sum(|row| row.published_bites),
        disruption_moves: sum(|row| row.disruption_moves),
    }
}

fn fail(document: Value, out: Option<&str>, status: i32) -> ! {
    let text = serde_json::to_string_pretty(&document).unwrap_or_else(|_| "{}".to_owned());
    if let Some(path) = out {
        let _ = std::fs::write(path, format!("{text}\n"));
    }
    println!("{text}");
    std::process::exit(status)
}

fn main() {
    let mut out: Option<String> = None;
    let mut cells: BTreeMap<String, String> = BTreeMap::new();
    for argument in std::env::args().skip(1) {
        if let Some(path) = argument.strip_prefix("--out=") {
            out = Some(path.to_owned());
        } else if let Some(pair) = argument.strip_prefix("--cell=") {
            match pair.split_once('=') {
                Some((fixture, path)) => {
                    cells.insert(fixture.to_owned(), path.to_owned());
                }
                None => fail(
                    json!({"error": format!("`--cell={pair}` is not `<fixture>=<path>`")}),
                    None,
                    2,
                ),
            }
        } else {
            fail(
                json!({"error": format!("unknown argument `{argument}`")}),
                None,
                2,
            );
        }
    }
    let out = out.as_deref();

    let mut rows = Vec::new();
    let mut walls = Vec::new();
    let mut fixture_cells = Vec::new();
    let mut prime_cells = Vec::new();
    let mut prime_inputs = Vec::new();
    let mut sources = Vec::new();
    for fixture in FIXTURES {
        let Some(path) = cells.get(fixture) else {
            fail(
                json!({
                    "experiment": "overlap-ics",
                    "battery": "economics-round-currency",
                    "error": format!("no `--cell={fixture}=<path>`; the spec calibrates on all three fixtures"),
                }),
                out,
                2,
            );
        };
        match read_cell(fixture, path) {
            Ok((cell_rows, wall, seconds)) => {
                fixture_cells.push(FixtureCell {
                    fixture: fixture.to_owned(),
                    terms: terms_of(&cell_rows),
                    seconds,
                });
                let prime_terms = terms_prime_of(&cell_rows);
                sources.push(json!({
                    "fixture": fixture,
                    "path": path,
                    "bites": cell_rows.len(),
                    "searchSeconds": seconds,
                    // Rider (i)'s counter, per cell, beside the bite count that
                    // bounds it.
                    "publishedBites": prime_terms.published_bites,
                }));
                prime_cells.push(FixtureCellPrime {
                    fixture: fixture.to_owned(),
                    terms: prime_terms,
                    seconds,
                });
                prime_inputs.push(timing_input_of(fixture, &cell_rows, &wall));
                rows.extend(cell_rows);
                walls.push(wall);
            }
            Err(error) => fail(
                json!({
                    "experiment": "overlap-ics",
                    "battery": "economics-round-currency",
                    "error": error,
                }),
                out,
                2,
            ),
        }
    }

    // `U0` is the currency Wave 1 could honestly write, and its transfer error
    // is the number that says whether the other four terms are needed at all.
    // It is reported beside `U1`, never instead of it.
    let u0 = transfer_check(&Currency::U0, &fixture_cells);

    let report = match timings_from_rows(&rows, &walls) {
        Ok(report) => report,
        Err(error) => fail(
            json!({
                "experiment": "overlap-ics",
                "battery": "economics-round-currency",
                "cellSources": sources,
                "u0": u0.ok(),
                "error": error,
            }),
            out,
            2,
        ),
    };
    let calibration = match calibrate(&report.timings, &FIXTURES) {
        Ok(calibration) => calibration,
        Err(error) => fail(
            json!({
                "experiment": "overlap-ics",
                "battery": "economics-round-currency",
                "cellSources": sources,
                "harness": report,
                "u0": u0.ok(),
                "error": error,
            }),
            out,
            2,
        ),
    };
    let check = match transfer_check(&calibration.currency, &fixture_cells) {
        Ok(check) => check,
        Err(error) => fail(
            json!({
                "experiment": "overlap-ics",
                "battery": "economics-round-currency",
                "cellSources": sources,
                "calibration": calibration,
                "error": error,
            }),
            out,
            2,
        ),
    };

    // ------------------------------------------------------------ `U'` --
    //
    // docs/currency-amendment.md, three signatures. The amended currency is
    // calibrated and checked **beside** the signed one, never instead of it:
    // `U` was rejected by a committed measurement and has to stay exactly the
    // thing that was rejected, so its two documents above do not move by one
    // field. The exit status carries `U'`'s verdict, because `U'` is the
    // currency this wave was funded to measure, and `EXIT_MEANS` says so in
    // the document rather than leaving a reader to infer it.
    let prime_calibration = match calibrate_prime(&prime_inputs, &FIXTURES) {
        Ok(calibration) => calibration,
        Err(error) => fail(
            json!({
                "experiment": "overlap-ics",
                "battery": "economics-round-currency",
                "cellSources": sources,
                "harness": report,
                "calibration": calibration,
                "u1": check,
                "u0": u0.ok(),
                "primeTimingInputs": prime_inputs,
                "error": format!("U' calibration: {error}"),
            }),
            out,
            2,
        ),
    };
    let prime_check = match transfer_check_prime(&prime_calibration.currency, &prime_cells) {
        Ok(prime_check) => prime_check,
        Err(error) => fail(
            json!({
                "experiment": "overlap-ics",
                "battery": "economics-round-currency",
                "cellSources": sources,
                "calibration": calibration,
                "u1": check,
                "u0": u0.ok(),
                "calibrationPrime": prime_calibration,
                "error": format!("U' transfer check: {error}"),
            }),
            out,
            2,
        ),
    };

    let document = json!({
        "experiment": "overlap-ics",
        "battery": "economics-round-currency",
        "spec": "docs/economics-round-spec.md, funded change 3",
        "amendment": "docs/currency-amendment.md: U' = sample_evaluations + B*master_batches \
                      + E*exact_checkpoint_calls + P*published_bites + D*disruption_moves; R is \
                      DROPPED absolutely; same derivation, same >10% reject rule verbatim, \
                      still a stop.",
        "cellSources": sources,
        "cells": fixture_cells,
        "harness": report,
        "calibration": calibration,
        "u1": check,
        // The comparison the round is entitled to see: how far a currency that
        // is `sample_evaluations` alone transfers.
        "u0": u0.as_ref().ok(),
        "summary": calibration.currency.summary(),
        "CURRENCY_ACCEPTED": check.accepted,
        "WORST_RELATIVE_ERROR": check.worst_relative_error,

        "primeTimingInputs": prime_inputs,
        "cellsPrime": prime_cells,
        "calibrationPrime": prime_calibration,
        "u2": prime_check,
        "summaryPrime": prime_calibration.currency.summary(),
        "CURRENCY_PRIME_ACCEPTED": prime_check.accepted,
        "WORST_RELATIVE_ERROR_PRIME": prime_check.worst_relative_error,
        "EXIT_MEANS": "0 iff U' (the amended currency) transfers within 10% on every ordered \
                       fixture pair; 1 iff it is REJECTED by the amendment's own rule; 2 iff \
                       the check could not run. U1's verdict is CURRENCY_ACCEPTED and is \
                       reported, not exited on.",
    });
    let accepted = prime_check.accepted;
    fail(document, out, i32::from(!accepted));
}
