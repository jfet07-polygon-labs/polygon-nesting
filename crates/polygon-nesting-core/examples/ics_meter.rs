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
    calibrate, timings_from_rows, transfer_check, BiteProfileRow, CellWall, Currency, FixtureCell,
    WorkTerms,
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
                sources.push(json!({
                    "fixture": fixture,
                    "path": path,
                    "bites": cell_rows.len(),
                    "searchSeconds": seconds,
                }));
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

    let document = json!({
        "experiment": "overlap-ics",
        "battery": "economics-round-currency",
        "spec": "docs/economics-round-spec.md, funded change 3",
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
    });
    let accepted = check.accepted;
    fail(document, out, i32::from(!accepted));
}
