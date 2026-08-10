import assert from 'node:assert/strict'
import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { test } from 'node:test'
import {
  QUALITY_METRICS,
  assertQualityGolden,
  evaluateQualityPromotion,
  layoutFingerprint,
  makeQualityGolden,
  readQualityGolden,
  writeQualityGolden
} from './canonical-quality.mjs'

function row(overrides = {}) {
  return {
    placedCount: 17,
    unplacedCount: 0,
    layoutFingerprint: 'a'.repeat(64),
    metrics: Object.fromEntries(QUALITY_METRICS.map(({ name }) => [name, 100])),
    ...overrides
  }
}

function golden(qualityRow = row()) {
  return makeQualityGolden({ fixture: qualityRow })
}

test('exact quality golden accepts the unchanged matrix and rejects silent layout changes', () => {
  const accepted = golden()
  assert.doesNotThrow(() => assertQualityGolden(accepted, structuredClone(accepted)))
  assert.throws(
    () => assertQualityGolden(accepted, golden(row({ layoutFingerprint: 'b'.repeat(64) }))),
    /--update-golden/
  )
})

test('promotion requires a material improvement', () => {
  const accepted = golden()
  const candidate = structuredClone(accepted)
  candidate.rows.fixture.layoutFingerprint = 'b'.repeat(64)
  const evaluation = evaluateQualityPromotion(accepted, candidate)
  assert.equal(evaluation.accepted, false)
  assert.match(evaluation.errors.join('\n'), /requires at least one material quality improvement/)
})

test('promotion allows a material improvement with only slight regressions', () => {
  const accepted = golden()
  const candidate = structuredClone(accepted)
  candidate.rows.fixture.layoutFingerprint = 'b'.repeat(64)
  candidate.rows.fixture.metrics.collisionBoundsAreaMm2 = 99.7
  candidate.rows.fixture.metrics.collisionBoundsSpanMm = 100.4
  candidate.rows.fixture.metrics.freeMaterialHoleCount = 101
  const evaluation = evaluateQualityPromotion(accepted, candidate)
  assert.equal(evaluation.accepted, true)
  assert.equal(evaluation.improvements.length, 1)
  assert.equal(evaluation.slightRegressions.length, 2)
})

test('promotion rejects lost placements and material regressions', () => {
  const accepted = golden()
  const candidate = structuredClone(accepted)
  candidate.rows.fixture.placedCount = 16
  candidate.rows.fixture.unplacedCount = 1
  candidate.rows.fixture.metrics.collisionBoundsAreaMm2 = 101
  candidate.rows.fixture.metrics.collisionBoundsSpanMm = 99
  const evaluation = evaluateQualityPromotion(accepted, candidate)
  assert.equal(evaluation.accepted, false)
  assert.match(evaluation.errors.join('\n'), /placement count regressed/)
  assert.match(evaluation.errors.join('\n'), /exceeds the slight-regression limit/)
})

test('zero baselines cannot regress silently', () => {
  const accepted = golden(row({
    metrics: Object.fromEntries(QUALITY_METRICS.map(({ name }) => [name, 0]))
  }))
  const candidate = structuredClone(accepted)
  candidate.rows.fixture.metrics.occupiedHullWasteRatio = 0.000001
  candidate.rows.fixture.metrics.sharedCollisionBoundaryLengthMm = 1
  const evaluation = evaluateQualityPromotion(accepted, candidate)
  assert.equal(evaluation.accepted, false)
  assert.match(evaluation.errors.join('\n'), /occupiedHullWasteRatio/)
})

test('layout fingerprint covers ordering, portfolio placements, and free-material identity', () => {
  const result = {
    placedCollisionGeometries: [],
    unplacedPieceIds: [],
    sortedPieceIds: ['piece-a'],
    score: { placementOrder: ['piece-a'], unplacedSourcePieceIds: [], freeMaterialSnapshot: { regions: [] } },
    portfolio: {
      source: 'shared-archive',
      status: 'completed',
      terminationReason: 'shared_archive_completed',
      placements: [{ pieceId: 'piece-a', transform: { translateX: 0 } }],
      unplacedPieceIds: []
    }
  }
  const accepted = layoutFingerprint(result)
  for (const mutate of [
    (candidate) => candidate.sortedPieceIds.push('piece-b'),
    (candidate) => candidate.score.placementOrder.push('piece-b'),
    (candidate) => { candidate.score.freeMaterialSnapshot.regions = [{ area: 1 }] },
    (candidate) => { candidate.portfolio.placements[0].transform.translateX = 1 }
  ]) {
    const candidate = structuredClone(result)
    mutate(candidate)
    assert.notEqual(layoutFingerprint(candidate), accepted)
  }
})

test('golden reader rejects malformed rows', () => {
  const directory = mkdtempSync(join(tmpdir(), 'canonical-quality-schema-'))
  try {
    const path = join(directory, 'golden.json')
    writeFileSync(path, JSON.stringify({ version: 1, rows: { fixture: row({ layoutFingerprint: 'invalid' }) } }))
    assert.throws(() => readQualityGolden(path), /invalid layout fingerprint/)
  } finally {
    rmSync(directory, { force: true, recursive: true })
  }
})

test('golden writes are atomic and remove a failed temporary file', () => {
  const directory = mkdtempSync(join(tmpdir(), 'canonical-quality-atomic-'))
  try {
    const path = join(directory, 'golden.json')
    assert.throws(() => writeQualityGolden(path, golden(), {
      renameSync: () => { throw new Error('interrupted rename') }
    }), /interrupted rename/)
    assert.deepEqual(readdirSync(directory), [])
    writeQualityGolden(path, golden())
    assert.deepEqual(JSON.parse(readFileSync(path, 'utf8')), golden())
  } finally {
    rmSync(directory, { force: true, recursive: true })
  }
})

test('promotion refuses a changed canonical row set', () => {
  const accepted = golden()
  const candidate = makeQualityGolden({ ...accepted.rows, extra: row() })
  const evaluation = evaluateQualityPromotion(accepted, candidate)
  assert.equal(evaluation.accepted, false)
  assert.match(evaluation.errors.join('\n'), /row set must not change/)
})
