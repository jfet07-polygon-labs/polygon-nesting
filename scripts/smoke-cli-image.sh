#!/usr/bin/env sh
set -eu

image=${1:-polygon-nesting:smoke}
repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
expected_version=${2:-$(node "$repository_root/scripts/release-version.mjs" --check)}
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
host_uid=$(id -u)
host_gid=$(id -g)
case "$host_uid:$host_gid" in
  *[!0-9:]*|:*|*:|*:*:*|0*:*) exit 1 ;;
esac

test "$(docker image inspect --format '{{index .Config.Labels "org.opencontainers.image.version"}}' "$image")" = "$expected_version"
test "$(docker image inspect --format '{{index .Config.Labels "org.opencontainers.image.source"}}' "$image")" = https://github.com/jfet07-polygon-labs/polygon-nesting
revision=$(docker image inspect --format '{{index .Config.Labels "org.opencontainers.image.revision"}}' "$image")
test -n "$revision"
test "$revision" != unknown
test "$(docker image inspect --format '{{index .Config.Labels "org.opencontainers.image.licenses"}}' "$image")" = NOASSERTION

test "$(docker run --rm --platform linux/amd64 --entrypoint sha256sum "$image" /usr/share/doc/polygon-nesting/NOTICE | cut -d ' ' -f 1)" = 1fa11aadfd5f98d734cbaced1fa10d525fd85565c560044734db4ce752037c1d
test "$(docker run --rm --platform linux/amd64 --entrypoint sha256sum "$image" /usr/share/doc/polygon-nesting/LICENSES/clipper2-ts-BSL-1.0.txt | cut -d ' ' -f 1)" = ea056d2c64294936b226f7360c265e77c52adc4ba171ee61029357f101f439cf
docker run --rm --platform linux/amd64 --entrypoint test "$image" -f /usr/share/doc/polygon-nesting/schemas/cli/benchmark-report-v1.schema.json

cp "$repository_root/tests/fixtures/cli/request-v1.json" "$workspace/request.json"
chmod 777 "$workspace"

docker run --rm \
  --platform linux/amd64 \
  --user "$host_uid:$host_gid" \
  --mount "type=bind,src=$workspace,dst=/work" \
  "$image" run \
  --input /work/request.json \
  --result-file /work/result.json \
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

mkdir "$workspace/dxfs"
cp "$repository_root/tests/fixtures/dxf/shapes-17/3268390_1.dxf" "$workspace/dxfs/part.dxf"
docker run --rm \
  --platform linux/amd64 \
  --user "$host_uid:$host_gid" \
  --mount "type=bind,src=$workspace,dst=/work" \
  "$image" run-dxf \
  --input-dir /work/dxfs \
  --sheet 2000x2700 \
  --allow-mirror false \
  --request-file /work/dxf-request.json \
  --result-file /work/dxf-result.json

test -s "$workspace/dxf-request.json"
test -s "$workspace/dxf-result.json"
python3 - "$workspace/dxf-request.json" "$workspace/dxf-result.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as request_file:
    request = json.load(request_file)
assert len(request["pieces"]) == 1
assert request["pieces"][0]["allowMirror"] is False
with open(sys.argv[2], encoding="utf-8") as result_file:
    assert json.load(result_file)["outcome"]["status"] == "success"
PY

cp "$repository_root/tests/fixtures/cli/polygons-v1.json" "$workspace/polygons.json"
docker run --rm \
  --platform linux/amd64 \
  --user "$host_uid:$host_gid" \
  --mount "type=bind,src=$workspace,dst=/work" \
  "$image" run-polygons \
  --polygons-file /work/polygons.json \
  --sheet 2000x2700 \
  --allow-mirror false \
  --request-file /work/polygon-request.json \
  --result-file /work/polygon-result.json \
  --report-file /work/polygon-report.json \
  --best-known-utilization-percent 1

test -s "$workspace/polygon-request.json"
test -s "$workspace/polygon-result.json"
test -s "$workspace/polygon-report.json"
python3 - "$workspace/polygon-request.json" "$workspace/polygon-result.json" "$workspace/polygon-report.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as request_file:
    request = json.load(request_file)
assert [piece["id"] for piece in request["pieces"]] == ["rectangle#1", "rectangle#2", "triangle#1"]
assert request["pieces"][2]["allowMirror"] is False
with open(sys.argv[2], encoding="utf-8") as result_file:
    assert json.load(result_file)["outcome"]["status"] == "success"
with open(sys.argv[3], encoding="utf-8") as report_file:
    report = json.load(report_file)
assert report["version"] == 1
assert report["instance"]["partCount"] == 3
assert report["run"]["bestKnownSheetUtilizationPercent"] == 1
PY

printf '{' > "$workspace/malformed.json"
if docker run --rm --platform linux/amd64 --user "$host_uid:$host_gid" --mount "type=bind,src=$workspace,dst=/work" "$image" run --input /work/malformed.json --result-file /work/malformed-result.json; then
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
if docker run --rm --platform linux/amd64 --user "$host_uid:$host_gid" --mount "type=bind,src=$workspace,dst=/work" "$image" run --input /work/archive-ineligible.json --result-file /work/archive-result.json; then
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
