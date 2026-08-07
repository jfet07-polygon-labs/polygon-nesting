#!/usr/bin/env sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)

dockerfile=$repository_root/Dockerfile
dockerignore=$repository_root/.dockerignore
smoke=$repository_root/scripts/smoke-cli-image.sh

for pattern in \
  .git \
  .env\* \
  .npmrc \
  .cargo/credentials \
  .cargo/credentials.toml \
  id_rsa \
  id_ed25519 \
  '*.pem' \
  '*.key' \
  '*.crt' \
  '*.node' \
  '*.tgz' \
  target \
  '**/target' \
  .vscode \
  .idea
do
  grep -Fqx "$pattern" "$dockerignore"
done

test ! -e "$repository_root/.github/ci/Dockerfile"
grep -Fqx 'FROM rust:1.95.0-bookworm@sha256:6258907abe69656e41cd992e0b705cdcfabcbbe3db374f92ed2d47121282d4a1 AS base' "$dockerfile"
grep -Fqx 'FROM base AS ci' "$dockerfile"
grep -Fqx 'FROM base AS builder' "$dockerfile"
grep -Fqx 'FROM debian:bookworm-slim@sha256:7b140f374b289a7c2befc338f42ebe6441b7ea838a042bbd5acbfca6ec875818 AS runtime' "$dockerfile"
grep -Fqx 'ARG TARGETPLATFORM' "$dockerfile"
grep -Fqx 'RUN test "$TARGETPLATFORM" = "linux/amd64"' "$dockerfile"
grep -Fq 'RUN cargo build --release --locked -p polygon-nesting-cli' "$dockerfile"
grep -Fqx 'ARG ENGINE_VERSION' "$dockerfile"
grep -Fqx 'ARG SOURCE_COMMIT' "$dockerfile"
grep -Fq 'test "$ENGINE_VERSION" = "0.1.1"' "$dockerfile"
grep -Fq 'test "$SOURCE_COMMIT" != "unknown"' "$dockerfile"
test "$(grep -Fc 'org.opencontainers.image.source="https://github.com/jfet07-polygon-labs/polygon-nesting"' "$dockerfile")" = 2
grep -Fq 'org.opencontainers.image.licenses="NOASSERTION"' "$dockerfile"
grep -Fqx 'USER polygon' "$dockerfile"
grep -Fq 'docker image inspect --format' "$smoke"
grep -Fq -- '--entrypoint id' "$smoke"
grep -Fq 'sha256sum' "$smoke"
grep -Fqx 'host_uid=$(id -u)' "$smoke"
grep -Fqx 'host_gid=$(id -g)' "$smoke"
grep -Fqx 'case "$host_uid:$host_gid" in' "$smoke"
grep -Fqx '  *[!0-9:]*|:*|*:|*:*:*|0*:*) exit 1 ;;' "$smoke"
test "$(grep -Fc -- '--user "$host_uid:$host_gid"' "$smoke")" = 3
