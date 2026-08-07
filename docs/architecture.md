# Architecture and deterministic core contract

## Dependency direction

```text
protocol <- core <- cli
                 <- napi
```

`polygon-nesting-protocol` depends only on `serde` and `serde_json`. `polygon-nesting-core` depends on the protocol and Rust algorithm libraries. `polygon-nesting-cli` and `polygon-nesting-napi` depend on core and protocol. `scripts/verify-dependency-direction.sh` is the executable dependency-direction check.

The protocol is application-neutral. The core must not depend on N-API, `napi-derive`, Node, Electron, libuv, CLI argument parsing, Azure SDKs, HTTP, Blob Storage, or application databases. CLI and N-API must not make algorithm, cache-ownership, or core-validation decisions.

## Deterministic core

`polygon_nesting_core::run(request, control, sink)` is the typed execution boundary. It validates the protocol request, rejects archive-ineligible requests as typed outcomes, constructs job-local geometry and free-material caches, creates one job-owned Rayon pool, runs inside that pool, projects a typed outcome, and clears the caches before returning.

The core preserves the exact-grid and numerical authorities used by the imported engine, including canonical-grid integer authorities, robust predicates, Clipper2 ownership, canonical JSON, and V8-parity number handling. Semantic output is the typed result, typed error, and ordered semantic event stream. Requested and actual worker counts, cache telemetry, and timing measurements are non-semantic equality inputs. Parity preserves whether timing fields exist while normalizing their values.

Each invocation owns its cache stores, coordinator, and Rayon pool. Unrelated jobs must use separate process executions or separate core invocations. The engine does not use the global Rayon pool for nesting work.

## Trace and history controls

`EngineRequest.diagnosticTraceMode` defaults to `full` and accepts only `full` or `off`. Off mode keeps the scalar bookkeeping required for decisions, counters, cancellation, hashes, and endpoint selection, but does not retain or project `capacityTrace`, `intrinsicAnytimeSchedulerTrace`, `focusedCompleteReconstructionTrace`, `intrinsicShortSideObserverTrace`, or `intrinsicShortSidePairFoldTrace`. The mode changes diagnostic serialization only; it does not change placements, scores, failures, or semantic event order.

`historyMode` is a separate control for state snapshots. `historyMode: "off"` and `diagnosticTraceMode: "off"` may be selected together by production workers, while parity and diagnostic requests use explicit `diagnosticTraceMode: "full"` when trace output is part of the evidence.

## Cancellation

`CancellationControl` is an atomic first-writer-wins state machine. Its terminal states are `Cancelled` and `Deadline`; the first accepted terminal reason is retained. Core checkpoints translate that one reason into the existing NFP/IFP control path. A deadline is set by the core when the request timeout expires. CLI signals use `Cancelled` through the same control. N-API cancellation uses the same control for an invocation token.

Cancellation is cooperative. A caller must allow the running core job to reach a checkpoint. A cancellation or deadline outcome is typed and is distinct from an internal failure.

## Semantic event order

The core owns semantic events. `EventSequencer` starts at ordinal `0`, emits strictly contiguous increasing ordinals, and has no terminal event variant. The semantic events are `portfolio-progress` and `state-snapshot`.

Adapters must preserve the core ordinal sequence. A CLI event file contains only protocol semantic events in emitted order. The N-API adapter may suppress snapshot delivery when snapshots are disabled, but it consumes the ordinal and does not renumber later semantic frames. N-API then appends one adapter-owned `terminal` frame at the next ordinal and waits for its acknowledgement. Callback-delivery failures remain transport failures and do not alter core semantics.

## Boundaries

```text
versioned JSON -> protocol decode -> typed core job -> typed outcome/events
                                                |             |
                                                |             +-> CLI JSON and NDJSON files
                                                +-> N-API desktop conversion and callbacks
```

JSON decoding and encoding belong in adapters and the protocol boundary, not in algorithm modules. Desktop-only routing identifiers, callback lifecycle, and addon cleanup belong in N-API. Artifact paths, signals, and exit statuses belong in CLI. Storage, authorization, Azure execution creation, and frontend status handling belong in the consuming backend.
