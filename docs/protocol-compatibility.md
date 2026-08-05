# Protocol compatibility

## Version 1

The public protocol version is `1`. Every `EngineRequest` includes `version: 1`. `decode_request` rejects another version before execution. `encode_outcome` writes a stable envelope:

```json
{"version":1,"outcome":{}}
```

Events are encoded as one `SequencedEngineEvent` per JSON value. The protocol defines `portfolio-progress` and `state-snapshot` events only. Terminal acknowledgement is an adapter transport concern, not a protocol event.

The protocol is defined by `polygon-nesting-protocol`, not by Electron IPC or a UI request type. `EngineRequest`, `EngineOutcome`, `EngineError`, diagnostics, and semantic events are its public contract. Canonical examples are in `tests/vectors/protocol/`.

## Input and output rules

Serde accepts unknown request object fields for forward tolerance. Re-encoding emits only known, supported fields, so callers must not expect unknown fields to round-trip. Required fields, defaults, optional-field omission, enum spelling, and camelCase field names are part of version 1 behavior.

Validation is mandatory at the protocol boundary. Requests require version 1, a positive finite `timeoutMs`, positive JavaScript-safe-integer sheet dimensions, at least one uniquely identified prepared piece, finite and safe numeric fields where required, and internally consistent optimizer settings. The adapters may do outer transport validation, but they do not replace core protocol validation.

Every result field represented by `ExactDecimalString` is a canonical base-10 integer string, never a JSON number. Those fields are `sheetDoubledAreaGrid2`, `minimumDoubledCollisionAreaSumGrid2`, `minimumCollisionAreaPressurePpm`, `maximumSingletonSpanPressurePpm`, `placedDoubledMaterialAreaGrid2`, `totalEnclosedCavityDoubledAreaGrid2`, `envelopeAreaGrid2`, `totalDoubledAreaGrid2`, `exactHullGapDoubledAreaGrid2`, `exactHullDoubledAreaGrid2`, `hullGapDoubledAreaGrid2`, `occupiedHullDoubledAreaGrid2`, `cohesionDeficitNumerator`, `cohesionDeficitDenominator`, `intrinsicEnvelopeAreaGrid2`, `productionEnvelopeAreaGrid2`, `largestOccupiedHullGapDoubledAreaGrid2`, and `collisionMaterialDoubledAreaGrid2`.

A canonical decimal is `0`, or an optional leading minus sign followed by nonzero-leading ASCII digits. The outcome encoder additionally checks the five capacity field names `maximumSingletonSpanPressurePpm`, `minimumCollisionAreaPressurePpm`, `minimumDoubledCollisionAreaSumGrid2`, `placedDoubledMaterialAreaGrid2`, and `sheetDoubledAreaGrid2` across the emitted value tree. Noncanonical strings and numeric encodings are rejected during outcome encoding.

`ExecutionDiagnostics` contains worker counts, elapsed time, and counters. It is intentionally non-semantic. Parity also preserves the presence, but not the value, of these timing-only fields wherever they occur: `runtimeMs`, `elapsedMs`, `preflightRuntimeMs`, `completeArchiveRuntimeMs`, `prefixTerminalizationMs`, `coldSearchMs`, `topologyMeasurementMs`, `contactMeasurementMs`, `serializedTraceBytes`, and `peakRssDeltaBytes`. `elapsedMs` can also occur in a `portfolio-progress` event.

## Eligibility and errors

The supported engine profiles are `compact` and `compact-short-side`. Requests that disable the shared archive, choose short-side-fill placement, or activate the GA path produce an `archive-ineligible` typed outcome. They are not silently emulated.

Errors are typed, application-neutral values. They do not contain Electron callback categories, Node object identities, desktop job identifiers, filesystem paths, or persistence metadata.

## Compatibility policy

Version 1 supports additive request and result fields only when old readers can safely ignore them and old writers preserve their established omission semantics. A change to field meaning, validation, enum spelling, ordering, canonical-number representation, result projection, error category, or semantic-event order is breaking and requires a new protocol version plus vectors and parity evidence.

A new version must retain a documented decoder and compatibility story for every supported prior version. It must not claim compatibility merely because JSON parses. Compatibility requires preserved validation, canonical encoding, typed outcomes, and semantic ordering.
