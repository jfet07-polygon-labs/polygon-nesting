# Azure Container Jobs integration contract

The `polygon-nesting` OCI image runs one polygon nesting request per Azure Container Job execution. It is a temporary compute worker, not an HTTP service and not an Azure resource manager.

## Responsibilities

The consuming backend owns:

1. creating a durable run record;
2. validating authorization and choosing the OCI image version;
3. placing one protocol v1 `request.json` file in durable storage or a mounted work volume;
4. starting one Container Job execution with allocated CPU and memory;
5. making `/work` available to the container and collecting the output artifacts after exit;
6. mapping the documented CLI exit status and artifacts into backend job state.

The image owns only deterministic engine execution. It has no Azure credentials, storage SDK, customer account configuration, HTTP listener, or application database dependency.

## File contract

The container entrypoint is equivalent to:

```sh
polygon-nesting run \
  --input /work/request.json \
  --output /work/result.json \
  --events /work/events.ndjson
```

`request.json` is a single protocol v1 `EngineRequest`. `result.json` is one versioned outcome envelope. `events.ndjson` is optional and is ordered semantic event data suitable for compact progress capture. A backend that needs final-result-only jobs omits `--events`.

The backend must use a durable shared or transferred storage mechanism for all three paths. The container filesystem disappears when its execution ends.

## Execution behavior

Start one container execution for each nesting job. Do not multiplex customer jobs through one running container. The core creates the job-owned Rayon pool using the CPU allocation provided to that execution. `SIGTERM` and `SIGINT` request cooperative cancellation, so the backend should allow its Container Job termination grace period before forcing termination.

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
