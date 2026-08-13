# Polygon Nesting Engine

`polygon-nesting` is a standalone Rust implementation of deterministic irregular polygon nesting. It has one typed core and two delivery adapters:

- an Electron N-API addon distributed through GitHub Packages as `@jfet07-polygon-labs/polygon-nesting` and through npmjs as `@jfet97/polygon-nesting`;
- a Linux amd64 OCI image containing the one-shot `polygon-nesting` CLI.

The engine supports the `compact` and `compact-short-side` profiles. Archive-ineligible requests are returned as typed outcomes. The engine does not implement rectangle nesting, Electron application state, storage, HTTP, Azure resource management, or customer credentials.

## Architecture

```text
                polygon-nesting-protocol
                  ^        ^       ^
                  |        |       |
polygon-nesting-dxf        |       polygon-nesting-napi
          ^                |                 |
          |        polygon-nesting-core      +-- npm packages
          |                ^
          +--- polygon-nesting-cli
                       |
                   OCI image
```

`protocol` owns versioned request, outcome, error, and semantic event data. `core` owns deterministic computation, job-local Rayon pools, caches, cancellation checkpoints, and event ordering. `dxf` converts a deterministic directory of raw DXF files into a protocol request. The CLI owns command parsing, deadline shortening, artifact-path safety, signal handling, atomic writes, and exit mapping. N-API owns desktop compatibility conversion, adapter validation, error projection, invocation registration, callback acknowledgement, and environment cleanup. The core has no dependency on DXF parsing, N-API, Node, Electron, libuv, CLI parsing, Azure SDKs, HTTP servers, or application persistence.

See [architecture](docs/architecture.md), [protocol compatibility](docs/protocol-compatibility.md), [N-API compatibility](docs/napi-compatibility.md), [CLI contract](docs/cli-contract.md), [polygon input v1](docs/polygon-input-v1.md), [Azure Container Jobs contract](docs/azure-container-job-contract.md), and [migration and release gates](docs/migration-from-min-plane-dfx.md).

## CLI

```sh
polygon-nesting run \
  --input /work/request.json \
  --result-file /work/result.json \
  [--events /work/events.ndjson] \
  [--deadline-ms MILLISECONDS]
```

The CLI reads one protocol v1 request and writes one protocol outcome. It exits with `0` for success, `1` for internal failure, `2` for malformed input or invocation, `3` for a typed domain failure, `4` for cancellation or deadline, and `5` for artifact-write failure.

For standalone tests that begin with raw DXFs, the same image can construct and preserve the exact request before running it:

```sh
polygon-nesting run-dxf \
  --input-dir /work/dxfs \
  --sheet 2000x2700 \
  --padding 10 \
  --profile compact \
  --allow-mirror false \
  --request-file /work/request.json \
  --result-file /work/result.json
```

Each regular `.dxf` file is one quantity-one piece. Files are sorted by filename, dimensions are millimetres, and model-space `LINE`, `ARC`, `CIRCLE`, and `ELLIPSE` entities retain the curve metadata consumed by the engine. CSV/customer semantics remain application-owned; applications such as Configurator that already construct an `EngineRequest` continue to use `run`.

For standalone tests that already have polygon coordinates, the image can instead construct the same request shape from a versioned polygon document:

```sh
polygon-nesting run-polygons \
  --polygons-file /work/polygons.json \
  --sheet 2000x2700 \
  --padding 10 \
  --profile compact \
  --allow-mirror false \
  --request-file /work/request.json \
  --result-file /work/result.json \
  --report-file /work/report.json
```

The [polygon input v1 contract](docs/polygon-input-v1.md) accepts ordered millimetre coordinates, per-polygon quantities, and rotation/mirror permissions. It is an additive convenience boundary; `run` remains the complete protocol boundary and `run-dxf` remains available unchanged.

For comparative runs, every CLI command accepts `--report-file` and an optional `--best-known-utilization-percent`. The report adds instance descriptors, engine runtime, worker counts, completion, area utilization, and occupied-envelope density without changing `result.json`. Versioned JSON Schemas for both CLI and N-API boundaries ship under the npm package's `schemas` export and in the OCI image at `/usr/share/doc/polygon-nesting/schemas`.

## Request trace controls

Protocol requests accept `diagnosticTraceMode: "full"` or `"off"`; omission defaults to `"full"`. Off mode omits only the detailed `capacityTrace`, `intrinsicAnytimeSchedulerTrace`, `focusedCompleteReconstructionTrace`, `intrinsicShortSideObserverTrace`, and `intrinsicShortSidePairFoldTrace` fields. It preserves semantic results, typed failures, cancellation, counters, worker diagnostics, and event ordinals. `historyMode` independently controls state snapshots.

Production and Azure workers that need neither snapshots nor detailed traces should set both fields explicitly:

```json
{
  "historyMode": "off",
  "diagnosticTraceMode": "off"
}
```

Parity and contract requests set `diagnosticTraceMode: "full"` explicitly so their trace-bearing evidence does not depend on the omission default.

The reproducible benchmark alternates the modes and reports runtime samples, minimum and median runtime, and result bytes:

```sh
node scripts/benchmark-diagnostic-trace-mode.mjs \
  --cli target/release/polygon-nesting \
  --input tests/fixtures/cli/request-v1.json \
  --iterations 5
```

It requires semantic equivalence after the documented normalization and smaller Off result bytes. It does not enforce a speed threshold.

## N-API package

The same prebuilt addon payload is published in two packages at the canonical workspace version from `Cargo.toml`: `@jfet07-polygon-labs/polygon-nesting` on GitHub Packages and `@jfet97/polygon-nesting` on npmjs. Run `node scripts/release-version.mjs --sync` after changing that single version source; CI rejects unsynchronized npm or lock metadata. Both packages include only Linux x64 and macOS arm64 addons. Windows x64 remains available for local and manual builds but is not published. The loader retains the desktop alias `irregular-nesting-native` and selects `irregular-nesting-native.<platform>-<arch>.node`. Unsupported targets fail before Cargo is invoked.

## OCI image

The OCI image supports only `linux/amd64` and runs as the non-root `polygon` user. It accepts an existing request with `run`, a directory of quantity-one DXFs with `run-dxf`, or a versioned polygon-coordinate document with `run-polygons`, and has no Azure credentials or storage SDK. A consuming backend owns durable storage, customer metadata, execution orchestration, and outcome handling.

## Provenance and release state

The initial source is `min-plane-dfx` commit `e4f3608878611c002f343473fab72adc7d155f87`. The translated Clipper2 material is identified in [NOTICE](NOTICE) and its complete BSL 1.0 text is in [LICENSES/clipper2-ts-BSL-1.0.txt](LICENSES/clipper2-ts-BSL-1.0.txt). Current legal hashes and release-candidate evidence requirements are documented in [migration-from-min-plane-dfx.md](docs/migration-from-min-plane-dfx.md).

The protected publication workflow delivers the verified package payload to GitHub Packages and npmjs and the verified runtime image to GHCR. No Azure deployment is claimed by this repository state. The embedded source engine must remain in the desktop application until the documented same-target parity, release-candidate, registry-delivery, Electron loading, and packaged-app gates all pass.
