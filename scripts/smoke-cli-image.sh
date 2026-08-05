#!/usr/bin/env sh
set -eu

image=${1:-polygon-nesting:smoke}
repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
workspace=$(mktemp -d)
cleanup() {
  rm -rf "$workspace"
}
trap cleanup EXIT

operating_system=$(docker image inspect --format '{{.Os}}' "$image")
test "$operating_system" = linux
architecture=$(docker image inspect --format '{{.Architecture}}' "$image")
test "$architecture" = amd64
configured_user=$(docker image inspect --format '{{.Config.User}}' "$image")
test -n "$configured_user"
test "$(docker run --rm --platform linux/amd64 --entrypoint id "$image" -u)" != 0

test "$(docker image inspect --format '{{index .Config.Labels "org.opencontainers.image.version"}}' "$image")" = 0.1.0
test "$(docker image inspect --format '{{index .Config.Labels "org.opencontainers.image.source"}}' "$image")" = https://github.com/jfet97/polygon-nesting
revision=$(docker image inspect --format '{{index .Config.Labels "org.opencontainers.image.revision"}}' "$image")
test -n "$revision"
test "$revision" != unknown
test "$(docker image inspect --format '{{index .Config.Labels "org.opencontainers.image.licenses"}}' "$image")" = NOASSERTION

test "$(docker run --rm --platform linux/amd64 --entrypoint sha256sum "$image" /usr/share/doc/polygon-nesting/NOTICE | cut -d ' ' -f 1)" = 1fa11aadfd5f98d734cbaced1fa10d525fd85565c560044734db4ce752037c1d
test "$(docker run --rm --platform linux/amd64 --entrypoint sha256sum "$image" /usr/share/doc/polygon-nesting/LICENSES/clipper2-ts-BSL-1.0.txt | cut -d ' ' -f 1)" = ea056d2c64294936b226f7360c265e77c52adc4ba171ee61029357f101f439cf

cp "$repository_root/tests/fixtures/cli/request-v1.json" "$workspace/request.json"
chmod 777 "$workspace"

docker run --rm \
  --platform linux/amd64 \
  --mount "type=bind,src=$workspace,dst=/work" \
  "$image" run \
  --input /work/request.json \
  --output /work/result.json \
  --events /work/events.ndjson

test -s "$workspace/result.json"
test -s "$workspace/events.ndjson"

python3 - "$workspace/result.json" "$workspace/events.ndjson" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as result_file:
    assert json.load(result_file)["version"] == 1
with open(sys.argv[2], encoding="utf-8") as events_file:
    events = [json.loads(line) for line in events_file if line.strip()]
assert events
assert [event["ordinal"] for event in events] == list(range(len(events)))
PY

printf '{' > "$workspace/malformed.json"
if docker run --rm --platform linux/amd64 --mount "type=bind,src=$workspace,dst=/work" "$image" run --input /work/malformed.json --output /work/malformed-result.json; then
  exit 1
else
  test "$?" = 2
fi
python3 - "$workspace/malformed-result.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as result_file:
    assert json.load(result_file)["outcome"]["error"]["category"] == "malformed_input"
PY

python3 - "$workspace/request.json" "$workspace/archive-ineligible.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as input_file:
    request = json.load(input_file)
request["settings"]["optimizer"]["intrinsicSharedArchiveEnabled"] = False
with open(sys.argv[2], "w", encoding="utf-8") as output_file:
    json.dump(request, output_file)
PY
if docker run --rm --platform linux/amd64 --mount "type=bind,src=$workspace,dst=/work" "$image" run --input /work/archive-ineligible.json --output /work/archive-result.json; then
  exit 1
else
  test "$?" = 3
fi
python3 - "$workspace/archive-result.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as result_file:
    assert json.load(result_file)["outcome"]["status"] == "archive-ineligible"
PY
