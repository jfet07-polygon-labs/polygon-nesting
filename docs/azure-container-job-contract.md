# Azure Container Jobs integration contract

The `polygon-nesting` OCI image runs one polygon nesting request per Azure Container Job execution. It is a temporary compute worker, not an HTTP service and not an Azure resource manager.

## Responsibilities

The consuming backend owns:

1. creating a durable run record;
2. validating authorization and choosing the OCI image version;
3. placing one protocol v1 `request.json` file in durable storage backed by the platform-managed Azure Files `/work` mount;
4. starting one Container Job execution with allocated CPU and memory;
5. mounting `/work` in the container and collecting the output artifacts after exit;
6. mapping the documented CLI exit status and artifacts into backend job state.

The image owns only deterministic engine execution. It has no Azure credentials, storage SDK, customer account configuration, HTTP listener, or application database dependency.

## File contract

The image entrypoint is `/usr/local/bin/polygon-nesting`. Each Container Job execution must supply the `run` subcommand and its arguments, for example:

```sh
IMAGE='ghcr.io/jfet07-polygon-labs/polygon-nesting@sha256:<verified-linux-amd64-manifest-digest>'
docker run --rm --volume "$PWD:/work" "$IMAGE" run \
  --input /work/request.json \
  --output /work/result.json \
  --events /work/events.ndjson
```

Azure Container Jobs must use the verified runtime image by immutable digest, not a mutable tag. Running the image without the `run` invocation is malformed and exits with the documented status `2`.

`request.json` is a single protocol v1 `EngineRequest`. Production jobs that do not need state snapshots or detailed algorithm traces should set both controls explicitly:

```json
{
  "historyMode": "off",
  "diagnosticTraceMode": "off"
}
```

`diagnosticTraceMode` omission defaults to `full`. Off mode removes only the five detailed trace fields; it does not change semantic results or typed failure behavior. `result.json` is one versioned outcome envelope. `events.ndjson` is optional and is ordered semantic event data suitable for compact progress capture. A backend that needs final-result-only jobs omits `--events`.

The backend must use the platform-managed Azure Files `/work` mount for all three paths. It must mount a per-run Azure Files directory at `/work`, or namespace the three artifact paths by the durable run ID, so concurrent executions cannot overwrite one another. The container filesystem disappears when its execution ends.

## Execution behavior

Start one container execution for each nesting job. Do not multiplex customer jobs through one running container. The core creates a job-owned Rayon pool. It first honors a positive `MIN_PLANE_IRREGULAR_NATIVE_THREADS` value; otherwise it uses one fewer than the OS-visible logical CPU count, clamped to one. Allocate CPU and memory for that execution accordingly. `SIGTERM` and `SIGINT` request cooperative cancellation, so the backend should allow its Container Job termination grace period before forcing termination.

A nonzero exit uses the stable statuses in [the CLI contract](cli-contract.md): `1` is an internal failure, `2` is malformed input or invocation, `3` is a typed domain failure, `4` is cancellation or deadline, and `5` is an output artifact failure. A process exit status alone is not a substitute for collecting `result.json` when the documented outcome was written.

## Image scope and platform

The Dockerfile rejects every target platform except `linux/amd64`. `scripts/smoke-cli-image.sh` verifies fixture execution, image architecture, the configured non-root user, OCI labels, and copied legal-file hashes. No Linux arm64 support is promised.

Release builds must pass nonempty `ENGINE_VERSION=0.1.0` and a non-`unknown` `SOURCE_COMMIT` build argument. Build and smoke verification are separate commands:

```sh
revision=$(git rev-parse HEAD)
docker buildx build --load --platform linux/amd64 \
  --build-arg ENGINE_VERSION=0.1.0 \
  --build-arg SOURCE_COMMIT="$revision" \
  --tag polygon-nesting:smoke .
./scripts/smoke-cli-image.sh polygon-nesting:smoke
```

The image labels identify the exact version, source repository, revision, and `NOASSERTION` license expression. The runtime image copies only the executable, CA certificate bundle, NOTICE, and translated Clipper2 license material from the build context, then runs as the `polygon` non-root user.
