import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

const [requestArgument, outputArgument] = process.argv.slice(2);
if (!requestArgument || !outputArgument) {
  throw new Error("usage: node mixed61-to-sparrow.mjs REQUEST.json OUTPUT.json");
}

const requestPath = resolve(requestArgument);
const outputPath = resolve(outputArgument);
const requestBytes = readFileSync(requestPath);
const request = JSON.parse(requestBytes.toString("utf8"));
const sourceById = new Map(request.sourcePieces.map((piece) => [piece.id, piece]));

const samePoint = (first, second) =>
  Math.abs(first[0] - second[0]) <= 1e-9 &&
  Math.abs(first[1] - second[1]) <= 1e-9;

const isReflectionEquivalentByRotation = (points) => {
  const reflected = points.map(([x, y]) => [-x, y]).reverse();
  const firstEdge = [points[1][0] - points[0][0], points[1][1] - points[0][1]];
  for (let shift = 0; shift < reflected.length; shift += 1) {
    const next = (shift + 1) % reflected.length;
    const reflectedEdge = [
      reflected[next][0] - reflected[shift][0],
      reflected[next][1] - reflected[shift][1],
    ];
    if (Math.abs(Math.hypot(...firstEdge) - Math.hypot(...reflectedEdge)) > 1e-7) {
      continue;
    }
    const angle =
      Math.atan2(reflectedEdge[1], reflectedEdge[0]) -
      Math.atan2(firstEdge[1], firstEdge[0]);
    const cosine = Math.cos(angle);
    const sine = Math.sin(angle);
    const matches = points.every(([x, y], index) => {
      const localX = x - points[0][0];
      const localY = y - points[0][1];
      const expected = reflected[(shift + index) % reflected.length];
      const transformed = [
        reflected[shift][0] + localX * cosine - localY * sine,
        reflected[shift][1] + localX * sine + localY * cosine,
      ];
      return (
        Math.abs(transformed[0] - expected[0]) <= 1e-7 &&
        Math.abs(transformed[1] - expected[1]) <= 1e-7
      );
    });
    if (matches) {
      return true;
    }
  }
  return false;
};

const asymmetricPieceIds = [];

const items = request.pieces.map((piece, index) => {
  const source = sourceById.get(piece.sourcePieceId);
  if (!source) {
    throw new Error(`missing source piece ${piece.sourcePieceId}`);
  }
  const segments = source.geometry?.segments;
  if (!Array.isArray(segments) || segments.length < 3) {
    throw new Error(`source piece ${piece.sourcePieceId} has no polygon ring`);
  }
  if (segments.some((segment) => segment.kind !== "line")) {
    throw new Error(`source piece ${piece.sourcePieceId} is not line-only`);
  }
  const points = segments.map((segment) => [segment.x1, segment.y1]);
  for (let segmentIndex = 0; segmentIndex < segments.length; segmentIndex += 1) {
    const segment = segments[segmentIndex];
    const next = points[(segmentIndex + 1) % points.length];
    if (!samePoint([segment.x2, segment.y2], next)) {
      throw new Error(`source piece ${piece.sourcePieceId} is not one ordered closed ring`);
    }
  }
  if (!isReflectionEquivalentByRotation(points)) {
    asymmetricPieceIds.push(piece.id);
  }
  points.push([...points[0]]);
  return {
    id: index,
    demand: 1,
    dxf: piece.id,
    shape: {
      type: "simple_polygon",
      data: points,
    },
  };
});

const output = {
  name: "mixed61-polygon-nesting",
  items,
  strip_height: Math.min(request.sheet.width, request.sheet.height),
};

mkdirSync(dirname(outputPath), { recursive: true });
writeFileSync(outputPath, `${JSON.stringify(output, null, 2)}\n`);
process.stdout.write(
  `${JSON.stringify({
    requestPath,
    requestSha256: createHash("sha256").update(requestBytes).digest("hex"),
    outputPath,
    itemCount: items.length,
    stripHeightMm: output.strip_height,
    rotations: "continuous",
    mirrorSemantics: asymmetricPieceIds.length === 0
      ? "not encoded; every input polygon passed reflection-equivalence-by-rotation audit"
      : "not encoded; use a mirror-disabled engine control",
    asymmetricPieceIds,
  })}\n`,
);
