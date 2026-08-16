import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'

const paths = process.argv.slice(2)
if (paths.length !== 3) {
  throw new Error('usage: node scripts/check-blocker-probe-evidence.mjs MODE20.json MODE21.json MODE22.json')
}

const sha256 = (value) => createHash('sha256').update(value).digest('hex')
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
  if (
    !probe.attempted ||
    probe.failureReason !== undefined ||
    probe.capExhausted !== undefined ||
    !probe.baseProjectionMatched ||
    probe.visitedParentRows !== probe.parentRowLimit ||
    probe.rows.length !== probe.parentRowLimit
  ) {
    throw new Error(`${path}: blocker-burden probe did not reach its valid bounded terminal`)
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
    const nonTied =
      row.signalOrdering === 'firstBetter' || row.signalOrdering === 'secondBetter'
    if (
      row.signalOrdering !== undefined &&
      row.signalOrdering !== 'equal' &&
      !nonTied
    ) {
      throw new Error(`${path}: invalid signal ordering at parent ordinal ${row.parentOrdinal}`)
    }
    if (nonTied) rawNonTiedPairs += 1
    if (row.comparatorOpposed) {
      if (!nonTied) {
        throw new Error(`${path}: opposing row has no valid non-tied signal ordering`)
      }
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
    if (row.comparatorOpposed) {
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
