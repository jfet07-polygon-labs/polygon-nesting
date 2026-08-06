# Migration provenance, parity gates, and desktop cutover

## Source and baseline

The accepted source revision is `e4f3608878611c002f343473fab72adc7d155f87` from `min-plane-dfx`. The frozen extraction-baseline commit is `5c72d8fca8e078b0a6e7d5f2515a8a0953475481`. The source worktree was clean when this document was prepared.

The recorded preparation toolchain is `rustc 1.95.0 (59807616e 2026-04-14)`, `cargo 1.95.0 (f2d3ce0bd 2026-03-21)`, Node `v24.16.0`, and pnpm `11.8.0` on `aarch64-apple-darwin`. The source-baseline decision explicitly did not claim a historical performance speedup. It accepted existing correctness and quality evidence; a new performance claim requires independently recorded benchmark evidence.

Protocol-vector provenance is machine-readable in `tests/vectors/protocol/provenance.json`. Its `sourceArtifact` paths identify artifacts from the frozen baseline, not paths that resolve in the accepted-source tree at `e4f3608878611c002f343473fab72adc7d155f87`. The baseline commit above is the provenance anchor for those paths. Its imported source-artifact hashes are:

| Artifact | SHA-256 |
| --- | --- |
| `request-v1.json` source artifact | `69103cfdc60e38e9efa028bf33f320c36eecc0b0417a839953990cdd1cc4f6f2` |
| `event-v1.json` source artifact | `865993c508b4e85fa2d7b62deb19529424eeed4dc88c3d36a207ebf2f6d52464` |
| `state-snapshot-event-v1.json` source artifact | `68cabe898cb09dd2210cbf93c6e427016235bd3175158e7021ece2fa827e481c` |

The source baseline commit records `docs/artifacts/polygon-nesting-extraction-baseline/source.json`, `migration-corpus.json`, `native-vectors.sha256`, `source-fixtures.sha256`, `legal-and-addon.sha256`, `package-manifest.json`, `gates/results.json`, gate logs, and a complete `SHA256SUMS` manifest. These are provenance records in source history, not runtime dependencies of this repository. The baseline commands are reproducible from the accepted source with `cargo fmt --manifest-path crates/irregular-nesting-native/Cargo.toml -- --check`, `cargo clippy --manifest-path crates/irregular-nesting-native/Cargo.toml --all-targets -- -D warnings`, `cargo test --release --manifest-path crates/irregular-nesting-native/Cargo.toml`, and its separately recorded Node, Electron, quality, capacity, fixture-export, and hash gates. The release candidate must rerun its corresponding standalone commands and record their exact output and exit status.

## Source ownership map

| Source owner | Standalone owner | Status |
| --- | --- | --- |
| `crates/irregular-nesting-native/src/{archive,capacity,caches,canonical_grid,checkpoints,clipper,domain,geometry,js_number,nfp_ifp,result,search,short_side,trace,transforms,validation}/**` | `crates/polygon-nesting-core/src/` with the same module paths | Imported deterministic implementation |
| `src/lib.rs` | `crates/polygon-nesting-core/src/lib.rs` | Core exports and typed boundary |
| `boundary/parallel.rs` | `crates/polygon-nesting-core/src/parallel.rs` | Job-owned Rayon pool |
| `boundary/run_job.rs` | `crates/polygon-nesting-core/src/job.rs` | Typed job execution and outcome projection |
| `boundary/request.rs`, `boundary/result.rs`, `boundary/error.rs` | `crates/polygon-nesting-protocol/src/{request,result,error}.rs` and core projections | Versioned neutral protocol and typed errors |
| `boundary/events.rs` | `crates/polygon-nesting-core/src/events.rs` and `crates/polygon-nesting-napi/src/events.rs` | Core semantic sequencing and N-API callback bridge |
| `boundary/job.rs` | `crates/polygon-nesting-napi/src/job.rs` | AsyncTask, invocation registry, and desktop lifecycle |
| `boundary/diagnostics.rs` | `crates/polygon-nesting-napi/src/diagnostics.rs` | N-API diagnostic glue |
| `boundary/mod.rs` | Split across core, protocol, and N-API modules above | No standalone boundary directory |
| Native addon build metadata | `crates/polygon-nesting-napi/build.rs` and `packages/polygon-nesting/scripts/build-native.mjs` | Four-target addon build and staging |
| No source equivalent | `crates/polygon-nesting-cli/**`, `Dockerfile`, and CLI image scripts | New CLI and OCI adapter |
| Electron renderer, preload, main process, SQLite, DXF dialogs, IPC, worker supervision, TypeScript polygon algorithm, rectangle nesting | Excluded | Application ownership remains outside this repository |

All source Rust integration tests under `crates/irregular-nesting-native/tests/*.rs` map to the identically named core integration test in `crates/polygon-nesting-core/tests/`. `clipper_offset_vectors.rs` retains its same-name mapping and reads `clipper-offset-pending.json`. All source vectors under `crates/irregular-nesting-native/tests/vectors/` map to `tests/vectors/core/` with the same filename. `no_pool_global_rayon_containment.rs` remains a separate integration-test process. The standalone suite adds protocol, core control, core event, typed job-service, N-API compatibility and lifecycle, and CLI contract tests for the new seams.

The release corpus is standalone: `tests/vectors/core/`, `tests/vectors/protocol/`, and `tests/fixtures/` contain no runtime dependency on the source checkout.

## Legal provenance

The imported Clipper2 translation is declared in `NOTICE`; the complete Boost Software License 1.0 text is in `LICENSES/clipper2-ts-BSL-1.0.txt`. Exact SHA-256 values are:

| File | SHA-256 |
| --- | --- |
| `NOTICE` | `1fa11aadfd5f98d734cbaced1fa10d525fd85565c560044734db4ce752037c1d` |
| `LICENSES/clipper2-ts-BSL-1.0.txt` | `ea056d2c64294936b226f7360c265e77c52adc4ba171ee61029357f101f439cf` |
| `packages/polygon-nesting/NOTICE` | `1fa11aadfd5f98d734cbaced1fa10d525fd85565c560044734db4ce752037c1d` |
| `packages/polygon-nesting/LICENSES/clipper2-ts-BSL-1.0.txt` | `ea056d2c64294936b226f7360c265e77c52adc4ba171ee61029357f101f439cf` |

The OCI license label is `NOASSERTION`. It is not a repository-wide license assertion.

## Authorized fixed-run package cutover

The one-time authorized fast package cutover publishes `@jfet07-polygon-labs/polygon-nesting@0.1.0` from the four ordinary `native-build-*` artifacts in CI run `31109349775`. The dedicated manual workflow pins the run identity, complete artifact inventory, target metadata, native dependency hashes, addon hashes, wrapper hashes, and final package allowlist. It does not rebuild Rust and does not run new parity. This narrow authorization exists to make the fixed package available for downstream cutover validation. It does not authorize OCI publication, a GitHub release, or desktop dependency replacement. It does not authorize removal of the embedded Rust engine.

The workflow assembles the same immutable tarball on every attempt. If version `0.1.0` is absent, it publishes that explicit tarball. If the version already exists, a rerun skips publication only when the registry shasum and integrity exactly match the freshly assembled tarball. Every attempt then installs and loads the exact registry version in a fresh project. Any existing version with different bytes is a terminal mismatch.

## Standard future release-candidate evidence

The standard future release path remains parity-bound. A release candidate must record, under `docs/release-evidence/<version>/`, a source record, parity record, release record, and `SHA256SUMS`. Required immutable facts are the standalone engine release commit and tag, the accepted source commit, Rust and Cargo identities, target triple, build profile and features, complete fixture and vector hashes, frozen-request hashes, old-engine and new-engine semantic hashes, four addon hashes, NPM tarball hash and manifest, OCI digest and labels, and each gate command with its result. `SHA256SUMS` must cover every candidate evidence file other than itself, fixture, vector, staged addon, package tarball, and legal file named by the candidate.

Outside the one-time package authorization above, absence of release-candidate evidence is a release blocker, not a waived check. Standard publishing must consume the verified tarball and image digest without rebuilding. The authorized destinations are GitHub Packages for `@jfet07-polygon-labs/polygon-nesting` and `ghcr.io/jfet97/polygon-nesting` for the OCI image. An OCI image must resolve every authorized tag to the verified digest, and a private GitHub release must attach its checksums and evidence.

## Parity gates

For every frozen request, the extracted core, N-API adapter, and CLI must exactly match the accepted old-Rust semantic outcome and ordered semantic events on the same target, Rust toolchain, Cargo version, feature set, build profile, and relevant native dependencies. Parity preserves the presence but normalizes the values of `runtimeMs`, `elapsedMs`, `preflightRuntimeMs`, `completeArchiveRuntimeMs`, `prefixTerminalizationMs`, `coldSearchMs`, `topologyMeasurementMs`, `contactMeasurementMs`, `serializedTraceBytes`, and `peakRssDeltaBytes`; it normalizes the same `elapsedMs` field in `portfolio-progress` events. Worker and cache diagnostics are also non-semantic. Cross-target equality is not a substitute for same-target parity.

The required gates include protocol and canonical JSON vectors, exact-grid numeric vectors, all core tests, thread-count equality at 1, 2, 4, and 8 workers, no-global-pool containment, N-API Node and Electron-as-Node loading, terminal acknowledgement and cleanup lifecycle tests, CLI artifact and exit-code tests, package allowlist and legal-hash checks, and a non-root Linux amd64 image smoke test.

A release candidate must also prove that the standalone suite succeeds without the source checkout. TypeScript-versus-Rust comparison remains diagnostic only. It cannot relax accepted-old-Rust versus extracted-Rust parity.

## Desktop cutover and removal rule

1. Assemble and verify one immutable NPM tarball containing the four target addons, and one immutable Linux amd64 OCI image digest.
2. Install the exact candidate tarball under the existing `irregular-nesting-native` dependency key without changing the desktop loader unless a verified incompatibility requires it.
3. Run same-target old-versus-new corpus parity, package resolution, Node and Electron loading, lifecycle, legal, quality, and packaged-app gates for every supported desktop target.
4. Publish the exact verified artifacts only after authorization, then verify registry delivery by installing the published package and pulling the OCI image by digest.
5. Replace the desktop dependency with the version-pinned published alias and retain a rollback path to the accepted external package version.
6. Remove `crates/irregular-nesting-native`, obsolete staging scripts, and old CI only after every prior gate passes and the evidence records remain available.

The embedded Rust engine must not be removed merely because the standalone repository compiles, a local addon loads, or a candidate exists. All parity, release-candidate, registry-delivery, desktop integration, packaged-app, and rollback gates are mandatory first.
