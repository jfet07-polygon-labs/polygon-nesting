# N-API compatibility and package contract

## Package

The desktop package is `@jfet07-polygon-labs/polygon-nesting` at the canonical workspace version from `Cargo.toml`, published only to `https://npm.pkg.github.com` when publication authorization and immutable source evidence exist. A release candidate consumes two `native-build-*` artifacts from a successful `main` push CI run for the exact source commit and does not rebuild Rust. The same CI run supplies the already smoke-tested OCI archive. The current repository's protocol, canonical 18-row semantic matrix, native, CLI, N-API, and OCI contract suites are authoritative release gates; old-engine migration parity remains an optional manual diagnostic. Node 18 or newer is required. The package contents allowlist is `package.json`, the CommonJS loader, target map, matching `.node` artifacts, versioned schemas, `NOTICE`, and the Clipper2 license text. It must not ship Rust source or a Cargo `target/` directory.

## Published JSON Schemas

The package exports its schema manifest as `@jfet07-polygon-labs/polygon-nesting/schemas` and individual files below `@jfet07-polygon-labs/polygon-nesting/schemas/*`. The manifest covers the N-API desktop request, resolved job envelope, callback event, native capability, and last-job diagnostics boundaries, plus the CLI EngineRequest, outcome envelope, event, polygon input, and benchmark report boundaries. Schema filenames and their embedded `$id` values are versioned independently from the npm release.

The schemas describe serialized JSON values. `runIrregularJob`, `engineInfoJson`, and `getLastJobDiagnostics` still accept or return strings where documented; callers parse those strings before schema validation. Required nested result, event, cache-telemetry, and snapshot structures are part of the schemas and are checked against canonical serializer output in the Rust suite. Runtime Rust validation remains authoritative for semantic and cross-field rules that JSON Schema cannot express, including unique IDs, aggregate polygon limits, simple-ring geometry, and optimizer membership constraints. Detailed observer-only trace objects remain additive protocol diagnostics and are intentionally open within their named trace boundary.

The loader preserves the desktop binary naming contract:

```text
irregular-nesting-native.<platform>-<arch>.node
```

Local/manual source builds support exactly:

| Platform | Architecture | Cargo target | Native library |
| --- | --- | --- | --- |
| Linux | x64 | `x86_64-unknown-linux-gnu` | `libpolygon_nesting_napi.so` |
| Windows | x64 | `x86_64-pc-windows-msvc` | `polygon_nesting_napi.dll` |
| macOS | arm64 | `aarch64-apple-darwin` | `libpolygon_nesting_napi.dylib` |

The published package contains exactly two addons: Linux x64 and macOS arm64. Windows x64 remains available for local/manual builds, but hosted CI does not build it and the registry package does not contain it.

Unsupported platform and architecture pairs fail before Cargo is invoked. A Darwin addon staged on Darwin is subject to the package build and signing checks. Plain Node and Electron-as-Node addon loading are release gates for each published target.

## Addon API version 3

The addon exports `engineInfoJson`, `nativeCapability`, `getLastJobDiagnostics`, `runIrregularJob`, and `cancelIrregularJob`.

`nativeCapability()` returns API version `3`, the crate version, the compiled target triple, and profiles `compact` and `compact-short-side`. Its stable capability shape is:

```json
{"apiVersion":3,"crateVersion":"<workspace-version>","targetTriple":"<compiled-target>","profiles":["compact","compact-short-side"]}
```

`engineInfoJson()` returns the typed engine name and crate version as a JSON string. `getLastJobDiagnostics()` returns a JSON string containing either an object or `null`; callers parse that string.

## Desktop request adapter

`runIrregularJob` starts an asynchronous task from the established desktop request JSON, an opaque invocation token, an event callback, and the snapshot-delivery option. Its resolved value is a JSON result or error envelope string. The adapter requires request `version: 1`, a nonblank `jobId`, `options.workerMode: "irregular-convex-v2"`, and `options.historyMode` set to `stream`, `final`, or `off`. `options.diagnosticTraceMode` accepts exactly `"full"` and `"off"`; omission defaults to `"full"` and any other value is rejected as a desktop revalidation error. A supplied `strategyRunId` must be non-empty. `cancelIrregularJob(invocationToken, reason)` returns whether it found a registered invocation and accepts only `cancelled` or `timeout`; other reason strings return `false`.

Omitted `allowGlobalMirror` and per-piece `allowMirror` default to `true`. Unknown desktop fields are tolerated. Desktop-only identifiers remain in the adapter; the core receives a neutral `EngineRequest` and repeats protocol validation. The adapter maps the desktop option to the top-level protocol `diagnosticTraceMode` field.

`diagnosticTraceMode: "off"` removes only the five detailed trace fields documented by the protocol contract. It does not renumber core semantic frames. Snapshot suppression still consumes each core ordinal, and completion still appends exactly one terminal frame at the next ordinal.

The adapter projects protocol failures into established desktop error categories. Archive-ineligible and legacy GA-windowed requests are returned as `not_implemented` routing failures so the caller can select the TypeScript backend. They are not emulated by the standalone core.

## Lifecycle and events

Cancellation is keyed by opaque invocation-token identity. The adapter cleans up the exact registered token, contains panics, and maintains environment cleanup hooks.

Core semantic frames retain their core-owned ordinals. When snapshot callbacks are disabled, the adapter suppresses delivery but does not renumber future frames. After core completion it sends exactly one `{"kind":"terminal","ordinal":N}` frame, where `N` follows the last consumed core ordinal, and waits for callback acknowledgement. The first callback transport failure is retained separately from the core outcome. Environment cleanup releases a terminal waiter with the N-API closing status.

The desktop application should consume a released package version or an immutable release-candidate tarball through its existing `irregular-nesting-native` resolution key. The final registry alias is `"irregular-nesting-native": "npm:@jfet07-polygon-labs/polygon-nesting@<workspace-version>"`. Access to the private GitHub Packages registry requires the configured `NODE_AUTH_TOKEN`; no token is committed. The desktop must not depend at runtime on a repository-relative Rust path or `workspace:*` package.
