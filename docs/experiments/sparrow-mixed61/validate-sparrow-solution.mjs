#!/usr/bin/env node

import fs from "node:fs";

const [instancePath, solutionPath, requestedSeparationText = "5"] = process.argv.slice(2);
if (!instancePath || !solutionPath) {
  throw new Error("usage: validate-sparrow-solution.mjs INSTANCE SOLUTION [SEPARATION_MM]");
}

const instance = JSON.parse(fs.readFileSync(instancePath, "utf8"));
const document = JSON.parse(fs.readFileSync(solutionPath, "utf8"));
const solution = document.solution ?? document;
const requestedSeparation = Number(requestedSeparationText);
const items = new Map(instance.items.map((item) => [item.id, item]));

function transformRing(item, transformation) {
  const input = item.shape.data;
  const count = input.length > 1
    && input[0][0] === input.at(-1)[0]
    && input[0][1] === input.at(-1)[1]
    ? input.length - 1
    : input.length;
  const radians = transformation.rotation * Math.PI / 180;
  const cos = Math.cos(radians);
  const sin = Math.sin(radians);
  const [tx, ty] = transformation.translation;
  return input.slice(0, count).map(([x, y]) => ({
    x: cos * x - sin * y + tx,
    y: sin * x + cos * y + ty,
  }));
}

function orient(a, b, c) {
  return (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
}

function pointSegmentDistanceSquared(point, a, b) {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const denominator = dx * dx + dy * dy;
  if (denominator === 0) {
    return (point.x - a.x) ** 2 + (point.y - a.y) ** 2;
  }
  const t = Math.max(0, Math.min(1, ((point.x - a.x) * dx + (point.y - a.y) * dy) / denominator));
  const qx = a.x + t * dx;
  const qy = a.y + t * dy;
  return (point.x - qx) ** 2 + (point.y - qy) ** 2;
}

function segmentsIntersect(a, b, c, d) {
  const epsilon = 1e-10;
  const abC = orient(a, b, c);
  const abD = orient(a, b, d);
  const cdA = orient(c, d, a);
  const cdB = orient(c, d, b);
  if (((abC > epsilon && abD < -epsilon) || (abC < -epsilon && abD > epsilon))
    && ((cdA > epsilon && cdB < -epsilon) || (cdA < -epsilon && cdB > epsilon))) {
    return true;
  }
  return false;
}

function segmentDistanceSquared(a, b, c, d) {
  if (segmentsIntersect(a, b, c, d)) {
    return 0;
  }
  return Math.min(
    pointSegmentDistanceSquared(a, c, d),
    pointSegmentDistanceSquared(b, c, d),
    pointSegmentDistanceSquared(c, a, b),
    pointSegmentDistanceSquared(d, a, b),
  );
}

function pointInPolygon(point, polygon) {
  let inside = false;
  for (let i = 0, j = polygon.length - 1; i < polygon.length; j = i++) {
    const a = polygon[i];
    const b = polygon[j];
    const crosses = (a.y > point.y) !== (b.y > point.y)
      && point.x < ((b.x - a.x) * (point.y - a.y)) / (b.y - a.y) + a.x;
    if (crosses) inside = !inside;
  }
  return inside;
}

function polygonDistanceSquared(a, b) {
  if (pointInPolygon(a[0], b) || pointInPolygon(b[0], a)) {
    return 0;
  }
  let minimum = Number.POSITIVE_INFINITY;
  for (let i = 0; i < a.length; i += 1) {
    const a0 = a[i];
    const a1 = a[(i + 1) % a.length];
    for (let j = 0; j < b.length; j += 1) {
      const b0 = b[j];
      const b1 = b[(j + 1) % b.length];
      minimum = Math.min(minimum, segmentDistanceSquared(a0, a1, b0, b1));
    }
  }
  return minimum;
}

const placements = solution.layout.placed_items.map((placement) => {
  const item = items.get(placement.item_id);
  if (!item) throw new Error(`unknown item id ${placement.item_id}`);
  return { id: placement.item_id, ring: transformRing(item, placement.transformation) };
});

const expectedPlacementCount = instance.items.reduce((sum, item) => sum + item.demand, 0);
if (placements.length !== expectedPlacementCount) {
  throw new Error(`placement count mismatch: ${placements.length}`);
}

const actualCountByItemId = new Map();
for (const placement of placements) {
  actualCountByItemId.set(placement.id, (actualCountByItemId.get(placement.id) ?? 0) + 1);
}
const itemMultiplicityMismatches = instance.items.flatMap((item) => {
  const actual = actualCountByItemId.get(item.id) ?? 0;
  return actual === item.demand ? [] : [{ itemId: item.id, expected: item.demand, actual }];
});

let minimumPairDistanceSquared = Number.POSITIVE_INFINITY;
let minimumPair = null;
for (let i = 0; i < placements.length; i += 1) {
  for (let j = i + 1; j < placements.length; j += 1) {
    const distanceSquared = polygonDistanceSquared(placements[i].ring, placements[j].ring);
    if (distanceSquared < minimumPairDistanceSquared) {
      minimumPairDistanceSquared = distanceSquared;
      minimumPair = [placements[i].id, placements[j].id];
    }
  }
}

const coordinates = placements.flatMap((placement) => placement.ring);
const minimumX = Math.min(...coordinates.map((point) => point.x));
const maximumX = Math.max(...coordinates.map((point) => point.x));
const minimumY = Math.min(...coordinates.map((point) => point.y));
const maximumY = Math.max(...coordinates.map((point) => point.y));
const minimumPairDistance = Math.sqrt(minimumPairDistanceSquared);
const minimumBoundaryDistance = Math.min(
  minimumX,
  solution.strip_width - maximumX,
  minimumY,
  instance.strip_height - maximumY,
);

const tolerance = 2e-3;
const validation = {
  placementCount: placements.length,
  expectedPlacementCount,
  itemMultiplicityValid: itemMultiplicityMismatches.length === 0,
  itemMultiplicityMismatches,
  reportedStripWidth: solution.strip_width,
  occupiedBounds: { minimumX, maximumX, minimumY, maximumY },
  minimumPairDistance,
  minimumPair,
  minimumBoundaryDistance,
  requestedSeparation,
  valid: itemMultiplicityMismatches.length === 0
    && minimumPairDistance + tolerance >= requestedSeparation
    && minimumBoundaryDistance + tolerance >= requestedSeparation,
  outputPrecisionTolerance: tolerance,
};

console.log(JSON.stringify(validation, null, 2));
if (!validation.valid) process.exitCode = 1;
