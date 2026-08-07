# Polygon Nesting Engine

`polygon-nesting` is a standalone Rust implementation of deterministic irregular polygon nesting. It has one typed core and two delivery adapters:

- an Electron N-API addon distributed as `@jfet07-polygon-labs/polygon-nesting`;
- a Linux amd64 OCI image containing the one-shot `polygon-nesting` CLI.

The engine supports the `compact` and `compact-short-side` profiles. Archive-ineligible requests are returned as typed outcomes. The engine does not implement rectangle nesting, Electron application state, storage, HTTP, Azure resource management, or customer credentials.

## Architecture

```text
polygon-nesting-protocol
        ^
        |
polygon-nesting-core
     ^          ^
     |          |
polygon-nesting-cli  polygon-nesting-napi
     |                      |
 OCI image          @jfet07-polygon-labs/polygon-nesting
```

`protocol` owns versioned request, outcome, error, and semantic event data. `core` owns deterministic computation, job-local Rayon pools, caches, cancellation checkpoints, and event ordering. The CLI owns command parsing, deadline shortening, artifact-path safety, signal handling, atomic writes, and exit mapping. N-API owns desktop compatibility conversion, adapter validation, error projection, invocation registration, callback acknowledgement, and environment cleanup. The core has no dependency on N-API, Node, Electron, libuv, CLI parsing, Azure SDKs, HTTP servers, or application persistence.

See [architecture](docs/architecture.md), [protocol compatibility](docs/protocol-compatibility.md), [N-API compatibility](docs/napi-compatibility.md), [CLI contract](docs/cli-contract.md), [Azure Container Jobs contract](docs/azure-container-job-contract.md), and [migration and release gates](docs/migration-from-min-plane-dfx.md).

## CLI

```sh
polygon-nesting run \
  --input /work/request.json \
  --output /work/result.json \
  [--events /work/events.ndjson] \
  [--deadline-ms MILLISECONDS]
```

The CLI reads one protocol v1 request and writes one protocol outcome. It exits with `0` for success, `1` for internal failure, `2` for malformed input or invocation, `3` for a typed domain failure, `4` for cancellation or deadline, and `5` for artifact-write failure.

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

The package publishes prebuilt addons only for Linux x64, Windows x64, macOS arm64, and macOS x64. The loader retains the desktop alias `irregular-nesting-native` and selects `irregular-nesting-native.<platform>-<arch>.node`. Unsupported targets fail before Cargo is invoked.

## OCI image

The OCI image supports only `linux/amd64` and runs as the non-root `polygon` user. It accepts one request per process and has no Azure credentials or storage SDK. A consuming backend owns durable storage, execution orchestration, and outcome handling.

## Provenance and release state

The initial source is `min-plane-dfx` commit `e4f3608878611c002f343473fab72adc7d155f87`. The translated Clipper2 material is identified in [NOTICE](NOTICE) and its complete BSL 1.0 text is in [LICENSES/clipper2-ts-BSL-1.0.txt](LICENSES/clipper2-ts-BSL-1.0.txt). Current legal hashes and release-candidate evidence requirements are documented in [migration-from-min-plane-dfx.md](docs/migration-from-min-plane-dfx.md).

No package publication, OCI publication, or Azure deployment is claimed by this repository state. The embedded source engine must remain in the desktop application until the documented same-target parity, release-candidate, registry-delivery, Electron loading, and packaged-app gates all pass.
