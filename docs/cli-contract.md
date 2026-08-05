# Polygon nesting CLI contract

`polygon-nesting` is a Linux command-line adapter for one deterministic polygon nesting job. One process accepts one request and exits after writing its artifacts. The CLI does not host HTTP, use Node, manage Azure resources, or run multiple unrelated jobs.

## Run

```sh
polygon-nesting run \
  --input /work/request.json \
  --output /work/result.json \
  [--events /work/events.ndjson] \
  [--deadline-ms MILLISECONDS]
```

`--input` and `--output` are required. `--events` is optional. The three artifact paths must name distinct files, including through relative-path normalization and existing symlinks. This prevents a result or event artifact from replacing the input or another artifact. Omitting `--events` produces only the final result and does not create an event artifact.

`--deadline-ms` must be a positive finite number. It only shortens the decoded request timeout: core execution receives the smaller of the request `timeoutMs` and `--deadline-ms`. Core execution owns deadline detection and cancellation semantics.

The input file is one protocol v1 `EngineRequest` JSON document. The adapter decodes it with `polygon_nesting_protocol::decode_request` before calling `polygon_nesting_core::run`. The output is exactly the protocol `encode_outcome` envelope. The adapter writes result and event artifacts through adjacent temporary files, flushes and synchronizes them, then renames them. Consumers must treat a nonzero process exit as an unsuccessful job even if an earlier artifact exists.

When requested, the event artifact is newline-delimited JSON. Every line is one protocol `encode_event` value. Core execution owns the zero-based ordinals, and the CLI preserves their emitted order without appending a terminal transport event.

`SIGINT` and `SIGTERM` request cooperative cancellation through the core `CancellationControl`. SIGTERM uses the same first-writer cancellation control and maps to `CancelReason::Cancelled`.

## Exit statuses

| Status | Meaning |
| --- | --- |
| `0` | Successful outcome written. |
| `1` | Internal failure. A sanitized typed internal-failure outcome is written when the output artifact is available. |
| `2` | Malformed command invocation, unreadable input, malformed JSON, invalid deadline, request validation failure, or unsafe artifact path alias. A sanitized typed malformed-input outcome is written when the output artifact is available. |
| `3` | Typed domain failure written, including archive-ineligible requests and non-cancellation engine failures. |
| `4` | Typed cancellation or deadline outcome written. |
| `5` | Result or requested event artifact could not be encoded or written. |

For nonzero statuses, stderr contains only a stable category message prefixed with `polygon-nesting:`. It excludes panic payloads, source locations, and raw parser details.

## Engine information

`polygon-nesting --info` prints the versioned engine capability record as JSON:

```json
{"name":"polygon-nesting","version":"0.1.0"}
```

The exact version is the built crate version. `--info` cannot be combined with `run`.
