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

The source baseline commit records `docs/artifacts/polygon-nesting-extraction-baseline/source.json`, `migration-corpus.json`, `native-vectors.sha256`, `source-fixtures.sha256`, `legal-and-addon.sha256`, `package-manifest.json`, `gates/results.json`, gate logs, and a complete `SHA256SUMS` manifest. These are provenance records in source history, not runtime dependencies of this repository. The historical baseline commands are reproducible from the accepted source with `cargo fmt --manifest-path crates/irregular-nesting-native/Cargo.toml -- --check`, `cargo clippy --manifest-path crates/irregular-nesting-native/Cargo.toml --all-targets -- -D warnings`, `cargo test --release --manifest-path crates/irregular-nesting-native/Cargo.toml`, and its separately recorded Node, Electron, quality, capacity, fixture-export, and hash gates. Current releases use the successful `main` CI evidence described below instead of rerunning those historical baseline commands.

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
| Native addon build metadata | `crates/polygon-nesting-napi/build.rs` and `packages/polygon-nesting/scripts/build-native.mjs` | Three-target local/manual addon build support and two-target publication staging |
| Configurator DXF conversion seam | `crates/polygon-nesting-dxf/**` | Optional raw-DXF-to-protocol adapter with real Shapes-17 and Mixed-61 corpus tests |
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

## Current repository release publication

The current repository is the authoritative source for `@jfet07-polygon-labs/polygon-nesting` on GitHub Packages, `@jfet97/polygon-nesting` on npmjs, and the Azure Jobs runtime image on GHCR. Both NPM packages and the OCI image use the canonical workspace version from `Cargo.toml`; `scripts/release-version.mjs` synchronizes and verifies ecosystem metadata. Pull requests run quality checks only. A successful push to `main` runs quality once, builds and loads the two published native targets (Linux x64 and macOS arm64), and builds and smoke-tests the Linux amd64 OCI archive once. The release workflow accepts only the exact unexpired evidence from that successful `main` CI run. Windows x64 remains supported for local/manual source builds, but hosted CI does not build or publish it.

The release candidate is assembled without rebuilding native code or the OCI image and without repeating quality or smoke tests. It creates two deterministic NPM tarballs from the same native payload and records independent hashes and manifests for both destinations. Offline verification requires every non-`package.json` payload byte to match and permits only the approved package name and registry differences between manifests. The protected default-branch publisher verifies the candidate again and preflights GitHub Packages, npmjs, and GHCR before publishing any artifact. GitHub Packages uses `github.token`; npmjs commands alone use `secrets.NPM_TOKEN` and an explicit `https://registry.npmjs.org` registry. The publisher then copies the exact OCI archive to GHCR. Existing version or tag bytes must match exactly; publication refuses to replace immutable identities with different content. Publication evidence records both package destinations.

The required current-repository gates include protocol and canonical JSON vectors, the canonical 18-row semantic and quality matrix, exact-grid numeric vectors, all core tests, thread-count equality at 1, 2, 4, and 8 workers, no-global-pool containment, N-API Node and Electron-as-Node loading, terminal acknowledgement and cleanup lifecycle tests, CLI artifact and exit-code tests, package allowlist and legal-hash checks, Full versus Off diagnostic trace equivalence, and a non-root Linux amd64 image smoke test. The matrix's accepted metrics and controlled promotion policy are documented in `docs/canonical-quality-golden.md`.

Protocol and contract requests set `diagnosticTraceMode: "full"` explicitly. Production and Azure requests that do not need diagnostic traces set both `historyMode: "off"` and `diagnosticTraceMode: "off"`. The benchmark alternates both modes, removes only the five documented trace fields plus documented timing values for semantic comparison, reports runtime samples/minimum/median and result bytes, and requires that Off produce smaller result bytes for a trace-producing fixture.

## Optional migration parity

The standalone old-versus-new parity workflow remains available as a manual migration diagnostic for Linux x64 and macOS arm64. It compares the accepted old `min-plane-dxf` outputs with the current core, N-API adapter, and CLI while normalizing documented timing and worker diagnostics. Windows parity remains available through the local/manual scripts, but no hosted Windows job is defined. Old-versus-new parity is no longer a CI, release-candidate, NPM publication, or OCI publication prerequisite because the port is complete and this repository's contract suite is now authoritative.

## Desktop cutover and removal rule

1. Assemble and verify the two immutable NPM tarballs containing the same two published native target addons, and one immutable Linux amd64 OCI image digest.
2. Install the exact candidate tarball under the existing `irregular-nesting-native` dependency key without changing the desktop loader unless a verified incompatibility requires it.
3. Run same-target old-versus-new corpus parity, package resolution, Node and Electron loading, lifecycle, legal, quality, and packaged-app gates for every supported desktop target.
4. Publish the exact verified artifacts only after authorization, then verify registry delivery by installing the published package and pulling the OCI image by digest.
5. Replace the desktop dependency with the version-pinned published alias and retain a rollback path to the accepted external package version.
6. Remove `crates/irregular-nesting-native`, obsolete staging scripts, and old CI only after every prior gate passes and the evidence records remain available.

The embedded Rust engine must not be removed merely because the standalone repository compiles, a local addon loads, or a candidate exists. All parity, release-candidate, registry-delivery, desktop integration, packaged-app, and rollback gates are mandatory first.
