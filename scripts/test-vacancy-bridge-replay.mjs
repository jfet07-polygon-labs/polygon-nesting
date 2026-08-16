import { join, resolve } from 'node:path'
import { existsSync } from 'node:fs'
import { spawnSync } from 'node:child_process'

const root = resolve(import.meta.dirname, '..')
const fixture = join(
  root,
  'tests',
  'fixtures',
  'mixed-61',
  'persistent-vacancy-parent-b9335a72.json',
)
const request = join(root, 'tests', 'fixtures', 'mixed-61', 'mixed61-request.json')
const target = process.env.CARGO_TARGET_DIR
  ? resolve(process.env.CARGO_TARGET_DIR)
  : join(root, 'target')
const executable = process.env.POLYGON_NESTING_BENCHMARK
  ? resolve(process.env.POLYGON_NESTING_BENCHMARK)
  : join(
      target,
      'release',
      'examples',
      `general_request_benchmark${process.platform === 'win32' ? '.exe' : ''}`,
    )
const expectedCanonicalLegalCandidates = 0
const expectedCanonicalCandidateOrderHash =
  'ec918d6c93b9336fc9ec029c691dbb28b10271bc3313611c6a725948ebe4b106'

if (!existsSync(executable)) {
  throw new Error(`release benchmark executable is missing: ${executable}`)
}

const argumentsForMode = (mode) => [
  request,
  '1',
  '4',
  '0',
  '0',
  '0',
  '0',
  '1',
  '0',
  '0',
  '1',
  '1',
  '0',
  '16',
  '4',
  '8',
  '0',
  '0',
  '5',
  '5',
  '24',
  '8',
  '40',
  '10',
  '10',
  '5',
  '0',
  '0.005',
  '0.001',
  '1',
  '6',
  '0',
  '0',
  '0',
  'structured',
  '0',
  '10',
  '1',
  '0',
  '0',
  '0',
  '0',
  String(mode),
  fixture,
]

const runMode = (mode, ordinal) => {
  const result = spawnSync(executable, argumentsForMode(mode), {
    cwd: root,
    encoding: 'utf8',
    maxBuffer: 16 * 1024 * 1024,
  })
  if (result.error) throw result.error
  if (result.status !== 0) {
    throw new Error(
      `vacancy-bridge replay ${ordinal} (mode ${mode}) failed (${result.status}): ${result.stderr}`,
    )
  }
  return result.stdout
}

const losslessNumber = Symbol('losslessNumber')
const parseLosslessJson = (source) =>
  JSON.parse(source, (_key, value, context) =>
    typeof value === 'number' ? { [losslessNumber]: context.source } : value,
  )
const losslessCanonicalJson = (value) => {
  if (value && typeof value === 'object' && losslessNumber in value) {
    return value[losslessNumber]
  }
  if (Array.isArray(value)) {
    return `[${value.map(losslessCanonicalJson).join(',')}]`
  }
  if (value && typeof value === 'object') {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${losslessCanonicalJson(value[key])}`)
      .join(',')}}`
  }
  return JSON.stringify(value)
}
const exactNumber = (value) =>
  value && typeof value === 'object' && losslessNumber in value
    ? Number(value[losslessNumber])
    : value

const timingFields = new Set([
  'elapsedMs',
  'engineElapsedMs',
  'firstQuartileElapsedMs',
  'interquartileRangeElapsedMs',
  'maxElapsedMs',
  'medianElapsedMs',
  'minElapsedMs',
  'thirdQuartileElapsedMs',
  'wallSeconds',
  'maximumResidentSetBytes',
  'phaseElapsedMs',
])

const withoutTimingFields = (value) => {
  if (value && typeof value === 'object' && losslessNumber in value) return value
  if (Array.isArray(value)) return value.map(withoutTimingFields)
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value)
        .filter(([key]) => !timingFields.has(key))
        .map(([key, nested]) => [key, withoutTimingFields(nested)]),
    )
  }
  return value
}

const modeDependentPopulationKeys = new Set([
  // mode 17 continues its restart screen while mode 26 terminates at the
  // bridge generator; these aggregate fields belong to that mode-owned tail.
  'ejectionInsertions',
  'failureReason',
  'immediateReversalsRejected',
  'repairRestartScreen',
  'work',
])

const protectedProjection = (value, path = []) => {
  if (value && typeof value === 'object' && losslessNumber in value) return value
  if (Array.isArray(value)) return value.map((nested) => protectedProjection(nested, path))
  if (value && typeof value === 'object') {
    const inPersistentVacancyPopulation =
      path.at(-1) === 'persistentVacancyPopulation'
    return Object.fromEntries(
      Object.entries(value)
        .filter(([key]) => key !== 'vacancyBridgeRelocation')
        .filter(([key]) => key !== 'persistentVacancyMode')
        .filter(([key]) => !(inPersistentVacancyPopulation && key === 'mode'))
        .filter(
          ([key]) =>
            !(inPersistentVacancyPopulation && modeDependentPopulationKeys.has(key)),
        )
        .map(([key, nested]) => [
          key,
          protectedProjection(nested, [...path, key]),
        ]),
    )
  }
  return value
}

const population = (output, mode, ordinal) => {
  const value = output.relaxedDiagnostics?.coupledDynamicSeparator?.persistentVacancyPopulation
  if (!value) throw new Error(`mode ${mode} replay ${ordinal} omitted population diagnostics`)
  const populationMode = exactNumber(value.mode)
  if (populationMode !== mode) {
    throw new Error(`mode ${mode} replay ${ordinal} reported population mode ${populationMode}`)
  }
  return value
}

const modes = [17, 26, 26, 17]
const outputs = modes.map((mode, index) =>
  parseLosslessJson(runMode(mode, index + 1)),
)
const populations = outputs.map((output, index) => population(output, modes[index], index + 1))
const normalizedOutputs = outputs.map(withoutTimingFields)
if (
  losslessCanonicalJson(normalizedOutputs[0]) !==
  losslessCanonicalJson(normalizedOutputs[3])
) {
  throw new Error('mode 17 output changed after the mode 26 probe sequence')
}
if (
  losslessCanonicalJson(normalizedOutputs[1]) !==
  losslessCanonicalJson(normalizedOutputs[2])
) {
  throw new Error('mode 26 output changed across deterministic replay')
}
if (
  losslessCanonicalJson(protectedProjection(normalizedOutputs[0])) !==
  losslessCanonicalJson(protectedProjection(normalizedOutputs[1]))
) {
  throw new Error('mode 26 changed the protected mode 17 projection')
}

for (const [index, probe] of [populations[1], populations[2]].entries()) {
  const bridge = probe.vacancyBridgeRelocation
  if (!bridge || !bridge.attempted) {
    throw new Error(`mode 26 replay ${index + 2} omitted bridge diagnostics`)
  }
  if (exactNumber(bridge.generatedCandidates) > exactNumber(bridge.candidateCap)) {
    throw new Error('mode 26 candidate cap was exceeded')
  }
  if (exactNumber(bridge.legalCandidates) > exactNumber(bridge.generatedCandidates)) {
    throw new Error('mode 26 legal count exceeded generated count')
  }
  if (
    exactNumber(bridge.topologyWork.topologyCalls) !==
    exactNumber(bridge.legalCandidates) + 1
  ) {
    throw new Error('mode 26 topology ledger did not account for source plus legal candidates')
  }
  if (
    exactNumber(bridge.topologyWork.topologyInputVertices) >
      exactNumber(bridge.topologyWork.topologyInputVertexCap) ||
    exactNumber(bridge.topologyWork.topologyOutputVertices) >
      exactNumber(bridge.topologyWork.topologyOutputVertexCap)
  ) {
    throw new Error('mode 26 topology ledger exceeded its cumulative cap')
  }
  if (bridge.failureReason) throw new Error(`mode 26 replay failed: ${bridge.failureReason}`)
  if (bridge.candidateOrderHash !== expectedCanonicalCandidateOrderHash) {
    throw new Error(
      `mode 26 canonical candidate-order hash changed: ${bridge.candidateOrderHash}`,
    )
  }
  if (exactNumber(bridge.legalCandidates) !== expectedCanonicalLegalCandidates) {
    throw new Error(
      `mode 26 canonical legal-candidate count changed: ${exactNumber(bridge.legalCandidates)}`,
    )
  }
  if (!['generatorInconclusive', 'pairedContinuationComplete'].includes(bridge.terminalStatus)) {
    throw new Error(`mode 26 reached an unknown terminal: ${bridge.terminalStatus}`)
  }
  if (bridge.terminalStatus === 'generatorInconclusive') {
    if (
      !bridge.inconclusive ||
      bridge.control ||
      bridge.treatment ||
      exactNumber(bridge.legalCandidates) !== 0
    ) {
      throw new Error('mode 26 inconclusive terminal is not generator-level')
    }
  } else if (
    bridge.inconclusive ||
    !bridge.control ||
    !bridge.treatment ||
    !bridge.causalComparabilityPassed ||
    !bridge.matchedContinuationWork
  ) {
    throw new Error('mode 26 paired terminal lacks independent comparable arms')
  }
}

process.stdout.write(
  `vacancy-bridge replay: ok (${modes.join(',')}; terminal=${populations[1].vacancyBridgeRelocation.terminalStatus}; legal=${exactNumber(populations[1].vacancyBridgeRelocation.legalCandidates)})\n`,
)
