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

grep -Fqx 'ARG TARGETPLATFORM' "$dockerfile"
grep -Fqx 'RUN test "$TARGETPLATFORM" = "linux/amd64"' "$dockerfile"
grep -Fqx 'ARG ENGINE_VERSION' "$dockerfile"
grep -Fqx 'ARG SOURCE_COMMIT' "$dockerfile"
grep -Fq 'test "$ENGINE_VERSION" = "0.1.0"' "$dockerfile"
grep -Fq 'test "$SOURCE_COMMIT" != "unknown"' "$dockerfile"
grep -Fq 'org.opencontainers.image.source="https://github.com/jfet97/polygon-nesting"' "$dockerfile"
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
