import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'

const paths = process.argv.slice(2)
if (paths.length !== 3) {
  throw new Error('usage: node scripts/check-blocker-probe-evidence.mjs MODE20.json MODE21.json MODE22.json')
}

const sha256 = (value) => createHash('sha256').update(value).digest('hex')
const PARENT_ROW_LIMIT = 48
const TRANSIENT_MEMORY_RESERVATION_BYTES = 16 * 1024 * 1024
const RETAINED_MEMORY_RESERVATION_BYTES = 2 * 1024 * 1024
const EXPECTED = new Map([
  [
    20,
    {
      baseMode: 16,
      baseProjectionSha256: '3f4cf4496d5fe3e7470ce407a6b4e3f5546b8d4c8bbe5f7afb3b4f8c1766c846',
      probeSha256: '55740ff0f97353a404d93a7adf736701149b1edf6c86922b6c0af2d7a8c3578b',
    },
  ],
  [
    21,
    {
      baseMode: 17,
      baseProjectionSha256: '8f91c7fe755e1fac1dc237dda09f53a58fd538ff5decbc7df7a693f09cab135a',
      probeSha256: '8246f7613428b369723078a7c95f8cafe666c5f54708d02059b53f0cb1c1e642',
    },
  ],
  [
    22,
    {
      baseMode: 18,
      baseProjectionSha256: 'd1b00626b54ce8587c5322be46706d22898d4b4a11e4ff10defd2cb0e6754769',
      probeSha256: '0ac0c432476911670b19e68994928674fc7e333515b0592a8c44ac5aefc98310',
    },
  ],
])
const canonicalize = (value) => {
  if (Array.isArray(value)) return value.map(canonicalize)
  if (value !== null && typeof value === 'object') {
    return Object.fromEntries(
      Object.keys(value)
        .sort()
        .map((key) => [key, canonicalize(value[key])]),
    )
  }
  return value
}
const canonicalSha256 = (value) => sha256(`${JSON.stringify(canonicalize(value), null, 2)}\n`)
const compareSignals = (first, second) => {
  const firstMinimum = first.minimumEjectedPieceCount ?? Number.POSITIVE_INFINITY
  const secondMinimum = second.minimumEjectedPieceCount ?? Number.POSITIVE_INFINITY
  if (firstMinimum !== secondMinimum) {
    return firstMinimum < secondMinimum ? 'firstBetter' : 'secondBetter'
  }
  if (first.distinctChildrenAtMinimum !== second.distinctChildrenAtMinimum) {
    return first.distinctChildrenAtMinimum > second.distinctChildrenAtMinimum
      ? 'firstBetter'
      : 'secondBetter'
  }
  return 'equal'
}
const reverseOrdering = (ordering) => {
  if (ordering === 'firstBetter') return 'secondBetter'
  if (ordering === 'secondBetter') return 'firstBetter'
  return ordering
}
const framedRowKey = (parent, first, second) => {
  const hash = createHash('sha256')
  hash.update('persistent-vacancy-blocker-probe-row-v1\0')
  for (const value of [parent, first, second]) {
    const length = Buffer.alloc(4)
    length.writeUInt32BE(Buffer.byteLength(value))
    hash.update(length)
    hash.update(value)
  }
  return hash.digest('hex')
}

const runs = paths
  .map((path) => {
    const source = readFileSync(path)
    const output = JSON.parse(source)
    const population =
      output.relaxedDiagnostics?.coupledDynamicSeparator?.persistentVacancyPopulation
    const probe = population?.blockerBurdenProbe
    if (!population || !probe) {
      throw new Error(`${path}: blocker-burden diagnostics are missing`)
    }
    return {
      path,
      outputSha256: sha256(source),
      mode: population.mode,
      probe,
    }
  })
  .sort((first, second) => first.mode - second.mode)

if (runs.map(({ mode }) => mode).join(',') !== '20,21,22') {
  throw new Error('the evidence set must contain modes 20, 21, and 22 exactly once')
}

const ownerByRowKey = new Map()
const summaries = []
for (const run of runs) {
  const { mode, path, probe } = run
  const expected = EXPECTED.get(mode)
  if (
    !probe.attempted ||
    probe.failureReason != null ||
    probe.capExhausted != null ||
    probe.baseMode !== expected.baseMode ||
    probe.expectedBaseProjectionSha256 !== expected.baseProjectionSha256 ||
    probe.actualBaseProjectionSha256 !== expected.baseProjectionSha256 ||
    !probe.baseProjectionMatched ||
    probe.parentRowLimit !== PARENT_ROW_LIMIT ||
    probe.visitedParentRows !== PARENT_ROW_LIMIT ||
    probe.rows.length !== PARENT_ROW_LIMIT ||
    probe.transientMemoryReservationBytes !== TRANSIENT_MEMORY_RESERVATION_BYTES ||
    probe.retainedMemoryReservationBytes !== RETAINED_MEMORY_RESERVATION_BYTES
  ) {
    throw new Error(`${path}: blocker-burden probe did not reach its valid bounded terminal`)
  }
  const probeSha256 = canonicalSha256(probe)
  if (probeSha256 !== expected.probeSha256) {
    throw new Error(`${path}: canonical probe hash mismatch`)
  }

  const localKeys = new Set()
  let keyedPairs = 0
  let rawNonTiedPairs = 0
  let rawComparatorOpposingPairs = 0
  let ownedNonTiedPairs = 0
  let ownedComparatorOpposingPairs = 0
  let duplicatePairs = 0
  const duplicateRowKeys = []
  for (const row of probe.rows) {
    if (!row.rowKeySha256) {
      if (
        row.firstSiblingAugmentedIdentityHash !== undefined ||
        row.secondSiblingAugmentedIdentityHash !== undefined
      ) {
        throw new Error(`${path}: partially identified row ${row.parentOrdinal}`)
      }
      continue
    }
    const expectedKey = framedRowKey(
      row.parentAugmentedIdentityHash,
      row.firstSiblingAugmentedIdentityHash,
      row.secondSiblingAugmentedIdentityHash,
    )
    if (row.rowKeySha256 !== expectedKey) {
      throw new Error(`${path}: row-key mismatch at parent ordinal ${row.parentOrdinal}`)
    }
    if (localKeys.has(row.rowKeySha256)) {
      throw new Error(`${path}: duplicate row key within mode ${mode}: ${row.rowKeySha256}`)
    }
    localKeys.add(row.rowKeySha256)
    keyedPairs += 1
    if (!row.firstSignal || !row.secondSignal) {
      throw new Error(`${path}: keyed row has no complete signal pair at parent ordinal ${row.parentOrdinal}`)
    }
    const signalOrdering = compareSignals(row.firstSignal, row.secondSignal)
    if (row.signalOrdering !== signalOrdering) {
      throw new Error(`${path}: signal ordering mismatch at parent ordinal ${row.parentOrdinal}`)
    }
    if (!['firstBetter', 'secondBetter', 'equal'].includes(row.comparatorOrdering)) {
      throw new Error(`${path}: invalid comparator ordering at parent ordinal ${row.parentOrdinal}`)
    }
    const nonTied = signalOrdering !== 'equal'
    const comparatorOpposed = nonTied && reverseOrdering(signalOrdering) === row.comparatorOrdering
    if (row.comparatorOpposed !== comparatorOpposed) {
      throw new Error(`${path}: comparator-opposition mismatch at parent ordinal ${row.parentOrdinal}`)
    }
    const expectedInvalidation = !nonTied
      ? 'equalSignal'
      : comparatorOpposed
        ? undefined
        : 'agreesWithExistingComparator'
    if (row.invalidationReason !== expectedInvalidation) {
      throw new Error(`${path}: invalidation-reason mismatch at parent ordinal ${row.parentOrdinal}`)
    }
    if (nonTied) rawNonTiedPairs += 1
    if (comparatorOpposed) {
      rawComparatorOpposingPairs += 1
    }
    const owner = ownerByRowKey.get(row.rowKeySha256)
    if (owner !== undefined) {
      duplicatePairs += 1
      duplicateRowKeys.push({ rowKeySha256: row.rowKeySha256, ownerMode: owner })
      continue
    }
    ownerByRowKey.set(row.rowKeySha256, mode)
    if (nonTied) {
      ownedNonTiedPairs += 1
    }
    if (comparatorOpposed) {
      ownedComparatorOpposingPairs += 1
    }
  }
  if (
    keyedPairs !== probe.preselectedPairs ||
    rawNonTiedPairs !== probe.nonTiedPairs ||
    rawComparatorOpposingPairs !== probe.comparatorOpposingPairs
  ) {
    throw new Error(
      `${path}: aggregate mismatch: keyed ${keyedPairs}/${probe.preselectedPairs}, non-tied ${rawNonTiedPairs}/${probe.nonTiedPairs}, opposing ${rawComparatorOpposingPairs}/${probe.comparatorOpposingPairs}`,
    )
  }
  summaries.push({
    mode,
    path,
    outputSha256: run.outputSha256,
    probeSha256,
    visitedParentRows: probe.visitedParentRows,
    preselectedPairs: probe.preselectedPairs,
    rawNonTiedPairs,
    rawComparatorOpposingPairs,
    ownedNonTiedPairs,
    ownedComparatorOpposingPairs,
    duplicatePairs,
    duplicateRowKeys,
    independentGatePassed: ownedComparatorOpposingPairs >= 30,
  })
}

const report = {
  schemaVersion: 1,
  assignment: 'lowestModeOrdinalOwnsRepeatedRowKey',
  requiredOwnedComparatorOpposingPairsPerMode: 30,
  stageAPassed: summaries.every(({ independentGatePassed }) => independentGatePassed),
  uniquePreselectedRowKeys: ownerByRowKey.size,
  trajectories: summaries,
}
process.stdout.write(`${JSON.stringify(report, null, 2)}\n`)
