# Polygon nesting CLI contract

`polygon-nesting` is a Linux command-line adapter for one deterministic polygon nesting job. One process accepts one request and exits after writing its artifacts. The CLI does not host HTTP, use Node, manage Azure resources, or run multiple unrelated jobs.

## Run

```sh
polygon-nesting run \
  --input /work/request.json \
  --result-file /work/result.json \
  [--events /work/events.ndjson] \
  [--report-file /work/report.json] \
  [--best-known-utilization-percent PERCENT] \
  [--deadline-ms MILLISECONDS]
```

`--input` and `--result-file` are required. `--events` and `--report-file` are optional. Every artifact path must name a distinct file, including through relative-path normalization and existing symlinks. This prevents a result, event, or report artifact from replacing the input or another artifact. `--best-known-utilization-percent` accepts a finite value from 0 through 100 and requires `--report-file`.

## Run raw DXFs

```sh
polygon-nesting run-dxf \
  --input-dir /work/dxfs \
  --sheet WIDTHxHEIGHT \
  --request-file /work/request.json \
  --result-file /work/result.json \
  [--padding MILLIMETRES] \
  [--profile compact|compact-short-side] \
  [--allow-mirror true|false] \
  [--timeout-ms MILLISECONDS] \
  [--events /work/events.ndjson] \
  [--report-file /work/report.json] \
  [--best-known-utilization-percent PERCENT] \
  [--deadline-ms MILLISECONDS]
```

`run-dxf` is an additive convenience adapter. It sorts all regular `.dxf` files in `--input-dir` by filename, converts each file into one quantity-one piece, atomically writes the generated protocol request to `--request-file`, then executes that request through the same path as `run`. The input unit and sheet unit are millimetres. Defaults are padding `10`, profile `compact`, mirroring enabled, and timeout `300000` ms. Padding must be an even integer so prepared bounds and collision clearance describe the same total expansion. `--request-file`, `--result-file`, and `--events` must be distinct under the same alias-safety rules as `run`, must not alias a source DXF, and must be outside `--input-dir`.

The importer supports `LINE`, `ARC`, `CIRCLE`, and `ELLIPSE` in the model-space `ENTITIES` section and preserves their source order. It normalizes every source geometry to its exact analytic minimum X/Y bounds, keeps arcs analytic, converts circles to two half arcs, and retains analytic ellipse metadata on adaptively sampled chords. It accepts at most 1000 DXF files and rejects files with no usable supported geometry. It intentionally does not expand block `INSERT` entities or read Configurator CSVs: quantities, order/customer metadata, block expansion, and application-specific prefiltering remain application responsibilities. `run --input` remains the canonical production integration for callers such as Configurator that already build an `EngineRequest`.

## Run polygon coordinates

```sh
polygon-nesting run-polygons \
  --polygons-file /work/polygons.json \
  --sheet WIDTHxHEIGHT \
  --request-file /work/request.json \
  --result-file /work/result.json \
  [--padding MILLIMETRES] \
  [--profile compact|compact-short-side] \
  [--allow-mirror true|false] \
  [--timeout-ms MILLISECONDS] \
  [--events /work/events.ndjson] \
  [--report-file /work/report.json] \
  [--best-known-utilization-percent PERCENT] \
  [--deadline-ms MILLISECONDS]
```

`run-polygons` is another additive convenience adapter. It reads [polygon input v1](polygon-input-v1.md), sorts unique definitions by ID, expands their quantities, normalizes coordinates to each polygon's minimum X/Y, atomically writes the generated protocol request, and executes it through the same path as `run`. Its units, defaults, padding rule, and artifact alias protections match `run-dxf`; generated artifacts must not alias `--polygons-file`. The input accepts at most 1000 definitions and 10000 expanded instances. The complete protocol `run --input` boundary remains appropriate for curves, holes, multiple contours, or custom engine settings.

`--deadline-ms` must be a positive finite number. It only shortens the decoded request timeout: core execution receives the smaller of the request `timeoutMs` and `--deadline-ms`. Core execution owns deadline detection and cancellation semantics.

For `run`, the input file is one protocol v1 `EngineRequest` JSON document. The adapter decodes it with `polygon_nesting_protocol::decode_request` before calling `polygon_nesting_core::run`. Every command writes exactly the protocol `encode_outcome` envelope. On Unix, each write uses the persistent `.polygon-nesting-staging` root in the destination parent. The root is opened with `O_NOFOLLOW` and must be a directory owned by the effective user with exact mode `0700`. Each artifact is written in a fresh exclusive per-job directory inside the retained root, synchronized, and renamed into the retained destination directory handle. Per-job cleanup is relative to the retained root and preserves a public replacement of the root path. This boundary excludes malicious mutation by the same user after validation. Consumers must treat a nonzero process exit as an unsuccessful job even if an earlier artifact exists.

The protocol request accepts `diagnosticTraceMode: "full"` or `"off"`; omission is equivalent to `"full"`. Off mode omits only `capacityTrace`, `intrinsicAnytimeSchedulerTrace`, `focusedCompleteReconstructionTrace`, `intrinsicShortSideObserverTrace`, and `intrinsicShortSidePairFoldTrace`. It remains independent of `historyMode`: a request that intentionally disables both can include:

```json
{
  "historyMode": "off",
  "diagnosticTraceMode": "off"
}
```

When requested, the event artifact is newline-delimited JSON. Every line is one protocol `encode_event` value. Core execution owns the zero-based ordinals, and the CLI preserves their emitted order without appending a terminal transport event.

## Benchmark report

`--report-file` writes a separate versioned benchmark report without changing the protocol result bytes. It describes the instance (part and source counts, sheet, profile, boundary size, effective transform permissions, and convex/concave/curved-or-unknown counts) and the run (engine time, workers, completion, placed counts, polygon area, sheet utilization, occupied-envelope density, and an optional gap to a supplied best-known utilization). The report is built from the exact in-memory request and outcome used by the run; it never reopens mutable artifact paths to reconstruct a sample.

Polygon area and vertex fields are emitted only when every relevant source is one connected closed line polygon without curve metadata. This keeps curved DXF diagnostics honest instead of treating sampled chords as exact source area. The report is one run sample; benchmark harnesses should aggregate repeated invocations into medians and spread statistics externally. Its schema is `schemas/cli/benchmark-report-v1.schema.json` in both the npm package and the OCI image documentation directory.

`SIGINT` and `SIGTERM` request cooperative cancellation through the core `CancellationControl`. SIGTERM uses the same first-writer cancellation control and maps to `CancelReason::Cancelled`.

## Exit statuses

| Status | Meaning |
| --- | --- |
| `0` | Successful outcome written. |
| `1` | Internal failure. A sanitized typed internal-failure outcome is written when a safe output artifact is available, including signal-handler registration failure. The outer process panic guard exits `1` without an artifact. |
| `2` | Malformed command invocation, unreadable input, malformed JSON or DXF, invalid polygon coordinates, invalid deadline, request validation failure, or unsafe artifact path alias. When the runner can recover exactly one `run`, `run-dxf`, or `run-polygons` command and one safe unique output path, it writes a sanitized typed malformed-input outcome. An invocation that combines `--info` with any command follows the same safe-artifact rule. |
| `3` | Typed domain failure written, including archive-ineligible requests and non-cancellation engine failures. |
| `4` | Typed cancellation or deadline outcome written. |
| `5` | The generated request, result, requested event artifact, or benchmark report could not be encoded or written. If the event artifact fails after the result is written, the CLI attempts to replace it with an I/O-failure outcome. |

When the event artifact write fails after the result artifact was written, the CLI attempts to replace the result with a sanitized `io_failure` envelope whose operation is `write-events`. The process exits `5` regardless. If the replacement cannot be written, the original result artifact remains unchanged.

For nonzero statuses, stderr contains only a stable category message prefixed with `polygon-nesting:`. It excludes panic payloads, source locations, and raw parser details.

## Engine information

`polygon-nesting --info` prints the versioned engine capability record as JSON:

```json
{"name":"polygon-nesting","version":"<workspace-version>"}
```

The exact version is the built crate version. `--info` cannot be combined with `run`, `run-dxf`, or `run-polygons`.
