import { createHash, randomUUID } from 'node:crypto'
import { readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs'

export const QUALITY_GOLDEN_VERSION = 1

const CONTINUOUS_REGRESSION_LIMIT = 0.005
const CONTINUOUS_IMPROVEMENT_THRESHOLD = 0.0025
const COUNT_REGRESSION_LIMIT = 1

export const QUALITY_METRICS = Object.freeze([
  { name: 'collisionBoundsAreaMm2', direction: 'min', kind: 'continuous' },
  { name: 'collisionBoundsSpanMm', direction: 'min', kind: 'continuous' },
  { name: 'collisionBoundsWorstNormalizedSheetConsumption', direction: 'min', kind: 'continuous' },
  { name: 'collisionBoundsNormalizedSpanSum', direction: 'min', kind: 'continuous' },
  { name: 'occupiedHullWasteRatio', direction: 'min', kind: 'continuous' },
  { name: 'freeMaterialRegionCount', direction: 'min', kind: 'count' },
  { name: 'freeMaterialHoleCount', direction: 'min', kind: 'count' },
  { name: 'freeMaterialSliverMetric', direction: 'min', kind: 'continuous' },
  { name: 'largestNetFreeMaterialRegionAreaMm2', direction: 'max', kind: 'continuous' },
  { name: 'sharedCollisionBoundaryLengthMm', direction: 'max', kind: 'continuous' },
  { name: 'sharedCollisionBoundaryContactUnits', direction: 'max', kind: 'continuous' },
  { name: 'sharedCollisionBoundaryContactBand', direction: 'max', kind: 'count' },
  { name: 'nearCompleteStructuralContactCount', direction: 'max', kind: 'count' },
  { name: 'dominantNearCompleteStructuralContactCount', direction: 'max', kind: 'count' }
])

function finiteNumber(value, label) {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    throw new Error(`${label} must be a finite number`)
  }
  return Number(value.toFixed(9))
}

function normalizedValue(value) {
  if (typeof value === 'number') return finiteNumber(value, 'layout fingerprint number')
  if (Array.isArray(value)) return value.map(normalizedValue)
  if (value !== null && typeof value === 'object') {
    return Object.fromEntries(Object.keys(value).sort().map((key) => [key, normalizedValue(value[key])]))
  }
  return value
}

function compareText(left, right) {
  return left < right ? -1 : left > right ? 1 : 0
}

export function layoutFingerprint(result) {
  const placed = result.placedCollisionGeometries.map((entry) => normalizedValue(entry))
    .sort((left, right) => {
      const leftId = left.placement?.pieceId ?? left.collisionGeometry?.sourcePieceId ?? ''
      const rightId = right.placement?.pieceId ?? right.collisionGeometry?.sourcePieceId ?? ''
      return compareText(leftId, rightId) || compareText(JSON.stringify(left), JSON.stringify(right))
    })
  const unplaced = [...(result.unplacedPieceIds ?? result.score?.unplacedSourcePieceIds ?? [])].sort()
  const identity = normalizedValue({
    placed,
    unplaced,
    sortedPieceIds: result.sortedPieceIds,
    placementOrder: result.score?.placementOrder,
    freeMaterialSnapshot: result.score?.freeMaterialSnapshot,
    portfolio: result.portfolio === undefined
      ? undefined
      : {
          source: result.portfolio.source,
          status: result.portfolio.status,
          terminationReason: result.portfolio.terminationReason,
          placements: result.portfolio.placements,
          unplacedPieceIds: result.portfolio.unplacedPieceIds
        }
  })
  return createHash('sha256').update(JSON.stringify(identity)).digest('hex')
}

export function requestFingerprint(requestText) {
  return createHash('sha256').update(requestText).digest('hex')
}

function sortedUniqueIds(ids, label) {
  if (!Array.isArray(ids) || ids.some((id) => typeof id !== 'string')) {
    throw new Error(`${label} must be an array of string IDs`)
  }
  const sorted = [...ids].sort(compareText)
  if (new Set(sorted).size !== sorted.length) throw new Error(`${label} contains duplicate IDs`)
  return sorted
}

export function extractQualityRow(rowId, result, { expectedPieceIds, requestFingerprint: fixtureFingerprint }) {
  const score = result.score
  if (score === null || typeof score !== 'object') throw new Error(`${rowId} has no layout score`)
  const expected = sortedUniqueIds(expectedPieceIds, `${rowId}.expectedPieceIds`)
  const placed = sortedUniqueIds(
    result.placedCollisionGeometries.map((entry) => entry.placement?.pieceId),
    `${rowId}.placedPieceIds`
  )
  const unplaced = sortedUniqueIds(result.unplacedPieceIds, `${rowId}.unplacedPieceIds`)
  const scoreUnplaced = sortedUniqueIds(score.unplacedSourcePieceIds, `${rowId}.score.unplacedSourcePieceIds`)
  const portfolioUnplaced = sortedUniqueIds(
    result.portfolio?.unplacedPieceIds,
    `${rowId}.portfolio.unplacedPieceIds`
  )
  if (JSON.stringify(unplaced) !== JSON.stringify(scoreUnplaced) ||
    JSON.stringify(unplaced) !== JSON.stringify(portfolioUnplaced)) {
    throw new Error(`${rowId} reports inconsistent unplaced piece IDs`)
  }
  if (placed.some((id) => unplaced.includes(id)) ||
    JSON.stringify([...placed, ...unplaced].sort(compareText)) !== JSON.stringify(expected)) {
    throw new Error(`${rowId} placed/unplaced IDs do not partition the request pieces`)
  }
  const metrics = Object.fromEntries(QUALITY_METRICS.map(({ name }) => [
    name,
    finiteNumber(score[name], `${rowId}.${name}`)
  ]))
  const scoreUnplacedCount = finiteNumber(score.unplacedCount, `${rowId}.score.unplacedCount`)
  if (!Number.isInteger(scoreUnplacedCount) || scoreUnplacedCount !== unplaced.length) {
    throw new Error(`${rowId}.score.unplacedCount does not match the unplaced IDs`)
  }
  return {
    placedCount: placed.length,
    unplacedCount: unplaced.length,
    layoutFingerprint: layoutFingerprint(result),
    requestFingerprint: fixtureFingerprint,
    metrics
  }
}

export function makeQualityGolden(rows) {
  return { version: QUALITY_GOLDEN_VERSION, rows }
}

function validateQualityGolden(golden, label) {
  if (golden.version !== QUALITY_GOLDEN_VERSION || golden.rows === null || typeof golden.rows !== 'object') {
    throw new Error(`unsupported canonical quality golden at ${label}`)
  }
  for (const [rowId, row] of Object.entries(golden.rows)) {
    if (!Number.isInteger(row.placedCount) || !Number.isInteger(row.unplacedCount) ||
      row.placedCount < 0 || row.unplacedCount < 0) {
      throw new Error(`${label}: ${rowId} has invalid placement counts`)
    }
    if (typeof row.layoutFingerprint !== 'string' || !/^[a-f0-9]{64}$/.test(row.layoutFingerprint)) {
      throw new Error(`${label}: ${rowId} has an invalid layout fingerprint`)
    }
    if (typeof row.requestFingerprint !== 'string' || !/^[a-f0-9]{64}$/.test(row.requestFingerprint)) {
      throw new Error(`${label}: ${rowId} has an invalid request fingerprint`)
    }
    for (const { name } of QUALITY_METRICS) finiteNumber(row.metrics?.[name], `${label}: ${rowId}.${name}`)
  }
}

export function readQualityGolden(path) {
  const golden = JSON.parse(readFileSync(path, 'utf8'))
  validateQualityGolden(golden, path)
  return golden
}

export function writeQualityGolden(path, golden, operations = {}) {
  validateQualityGolden(golden, path)
  const write = operations.writeFileSync ?? writeFileSync
  const rename = operations.renameSync ?? renameSync
  const remove = operations.rmSync ?? rmSync
  const temporaryPath = `${path}.${process.pid}.${randomUUID()}.tmp`
  try {
    write(temporaryPath, `${JSON.stringify(golden, null, 2)}\n`)
    rename(temporaryPath, path)
  } finally {
    remove(temporaryPath, { force: true })
  }
}

function exactDifferences(golden, candidate) {
  const differences = []
  const rowIds = [...new Set([...Object.keys(golden.rows), ...Object.keys(candidate.rows)])].sort()
  for (const rowId of rowIds) {
    const before = golden.rows[rowId]
    const after = candidate.rows[rowId]
    if (before === undefined || after === undefined) {
      differences.push(`${rowId}: row ${before === undefined ? 'added' : 'removed'}`)
      continue
    }
    for (const field of ['placedCount', 'unplacedCount', 'layoutFingerprint', 'requestFingerprint']) {
      if (before[field] !== after[field]) differences.push(`${rowId}.${field}: ${before[field]} -> ${after[field]}`)
    }
    for (const { name } of QUALITY_METRICS) {
      if (before.metrics[name] !== after.metrics[name]) {
        differences.push(`${rowId}.${name}: ${before.metrics[name]} -> ${after.metrics[name]}`)
      }
    }
  }
  return differences
}

export function assertQualityGolden(golden, candidate) {
  const differences = exactDifferences(golden, candidate)
  if (differences.length > 0) {
    throw new Error(`canonical quality differs from the accepted golden:\n${differences.join('\n')}\nRun with --update-golden only after reviewing the quality trade-off.`)
  }
}

function signedImprovement(before, after, direction) {
  return direction === 'min' ? before - after : after - before
}

function relativeChangeMagnitude(before, delta) {
  if (before === 0) return delta === 0 ? 0 : Number.POSITIVE_INFINITY
  return Math.abs(delta) / Math.abs(before)
}

export function evaluateQualityPromotion(golden, candidate) {
  const errors = []
  const improvements = []
  const slightRegressions = []
  let tradeoffBalance = 0
  const goldenRows = Object.keys(golden.rows).sort()
  const candidateRows = Object.keys(candidate.rows).sort()
  if (JSON.stringify(goldenRows) !== JSON.stringify(candidateRows)) {
    errors.push('canonical row set must not change during golden promotion')
    return { accepted: false, errors, improvements, slightRegressions }
  }

  for (const rowId of goldenRows) {
    const before = golden.rows[rowId]
    const after = candidate.rows[rowId]
    if (after.requestFingerprint !== before.requestFingerprint) {
      errors.push(`${rowId}: canonical request fixture changed`)
    }
    if (after.unplacedCount > before.unplacedCount || after.placedCount < before.placedCount) {
      errors.push(`${rowId}: placement count regressed (${before.placedCount}/${before.unplacedCount} -> ${after.placedCount}/${after.unplacedCount})`)
    } else if (after.unplacedCount < before.unplacedCount || after.placedCount > before.placedCount) {
      improvements.push(`${rowId}: placement count improved`)
      tradeoffBalance += Math.max(
        before.unplacedCount - after.unplacedCount,
        after.placedCount - before.placedCount
      )
    }

    for (const metric of QUALITY_METRICS) {
      const beforeValue = before.metrics[metric.name]
      const afterValue = after.metrics[metric.name]
      const improvement = signedImprovement(beforeValue, afterValue, metric.direction)
      if (improvement === 0) continue
      if (improvement > 0) {
        const material = metric.kind === 'count'
          ? improvement >= 1
          : relativeChangeMagnitude(beforeValue, improvement) >= CONTINUOUS_IMPROVEMENT_THRESHOLD
        tradeoffBalance += metric.kind === 'count'
          ? improvement
          : beforeValue === 0 ? 1 : improvement / Math.abs(beforeValue) / CONTINUOUS_IMPROVEMENT_THRESHOLD
        if (material) improvements.push(`${rowId}.${metric.name}: ${beforeValue} -> ${afterValue}`)
        continue
      }

      const regression = -improvement
      const withinLimit = metric.kind === 'count'
        ? regression <= COUNT_REGRESSION_LIMIT
        : relativeChangeMagnitude(beforeValue, regression) <= CONTINUOUS_REGRESSION_LIMIT
      const message = `${rowId}.${metric.name}: ${beforeValue} -> ${afterValue}`
      tradeoffBalance -= metric.kind === 'count'
        ? regression
        : beforeValue === 0 ? Number.POSITIVE_INFINITY : regression / Math.abs(beforeValue) / CONTINUOUS_IMPROVEMENT_THRESHOLD
      if (withinLimit) slightRegressions.push(message)
      else errors.push(`${message} exceeds the slight-regression limit`)
    }
  }

  if (improvements.length === 0) errors.push('golden promotion requires at least one material quality improvement')
  if (!(tradeoffBalance > 0)) errors.push('aggregate normalized regressions outweigh the measured improvements')
  return { accepted: errors.length === 0, errors, improvements, slightRegressions, tradeoffBalance }
}

export function promoteQualityGolden(path, golden, candidate) {
  const evaluation = evaluateQualityPromotion(golden, candidate)
  if (!evaluation.accepted) {
    throw new Error(`canonical quality golden promotion refused:\n${evaluation.errors.join('\n')}`)
  }
  writeQualityGolden(path, candidate)
  return evaluation
}
