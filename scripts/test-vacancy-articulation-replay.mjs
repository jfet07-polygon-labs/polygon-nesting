import { createHash } from 'node:crypto'
import { existsSync, readFileSync } from 'node:fs'
import { join, resolve } from 'node:path'
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
const expectedPopulationSha256 =
  '8f91c7fe755e1fac1dc237dda09f53a58fd538ff5decbc7df7a693f09cab135a'
const fixtureSha256 = createHash('sha256').update(readFileSync(fixture)).digest('hex')
const expectedFixtureSha256 =
  '18e0b052997d1251573fa35679c9fcf1d5e796acf771ec48f320ce4e9bf0081d'

if (!existsSync(executable)) {
  throw new Error(
    `release benchmark executable is missing: ${executable}; build it before running this oracle`,
  )
}
if (fixtureSha256 !== expectedFixtureSha256) {
  throw new Error(`persistent-vacancy fixture hash changed: ${fixtureSha256}`)
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

const canonicalReplaySuffix = ['structured', '0', '10', '1', '0', '0', '0', '0']
const canonicalArguments = argumentsForMode(17)
if (
  JSON.stringify(canonicalArguments.slice(-10, -2)) !==
    JSON.stringify(canonicalReplaySuffix) ||
  canonicalArguments.at(-2) !== '17' ||
  canonicalArguments.at(-1) !== fixture
) {
  throw new Error(
    'vacancy-articulation replay argv drifted from the canonical structured/0/10/1/0/0/0/0/mode/fixture command',
  )
}

const runMode = (mode, ordinal) => {
  const result = spawnSync(executable, argumentsForMode(mode), {
    cwd: root,
    encoding: 'utf8',
    maxBuffer: 16 * 1024 * 1024,
  })
  if (result.error) throw result.error
  if (result.status !== 0) {
    throw new Error(
      `vacancy-articulation replay ${ordinal} (mode ${mode}) failed (${result.status}): ${result.stderr}`,
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
const exactSha256 = (value) =>
  createHash('sha256').update(losslessCanonicalJson(value)).digest('hex')

// keep this allowlist deliberately explicit. if a new timing or OS measurement
// field is introduced, add its exact serialized name here and review the oracle
// change together with the protocol change.
const explicitlyAllowedReplayFields = new Set([
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

const withoutExplicitlyAllowedReplayFields = (value) => {
  if (value && typeof value === 'object' && losslessNumber in value) {
    return value
  }
  if (Array.isArray(value)) {
    return value.map(withoutExplicitlyAllowedReplayFields)
  }
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value)
        .filter(([key]) => !explicitlyAllowedReplayFields.has(key))
        .map(([key, nested]) => [key, withoutExplicitlyAllowedReplayFields(nested)]),
    )
  }
  return value
}

const parsedMode25Output = (source, ordinal) => {
  const output = parseLosslessJson(source)
  const population =
    output.relaxedDiagnostics?.coupledDynamicSeparator?.persistentVacancyPopulation
  if (!population) {
    throw new Error(`mode 25 replay ${ordinal} did not return persistent-vacancy population diagnostics`)
  }
  const populationMode =
    population.mode && typeof population.mode === 'object' && losslessNumber in population.mode
      ? Number(population.mode[losslessNumber])
      : population.mode
  if (populationMode !== 25) {
    throw new Error(`mode 25 replay ${ordinal} reported population mode ${populationMode}`)
  }
  return { output, population }
}

const normalizedPopulation = (source, mode) => {
  const output = parseLosslessJson(source)
  const population =
    output.relaxedDiagnostics?.coupledDynamicSeparator?.persistentVacancyPopulation
  if (!population) {
    throw new Error(`mode ${mode} did not return persistent-vacancy population diagnostics`)
  }
  const populationMode =
    population.mode && typeof population.mode === 'object' && losslessNumber in population.mode
      ? Number(population.mode[losslessNumber])
      : population.mode
  if (populationMode !== mode) {
    throw new Error(`mode ${mode} reported population mode ${populationMode}`)
  }
  delete population.vacancyArticulationProbe
  population.mode = 17
  if (population.repairRestartScreen?.armFamily) {
    if (population.repairRestartScreen.armFamily === 'continuedStateRebuiltQueueArticulationProbe') {
      population.repairRestartScreen.armFamily = 'continuedStateRebuiltQueue'
    }
  }
  return population
}

const modes = [17, 25, 25, 17]
const sources = modes.map((mode, index) => runMode(mode, index + 1))
const projections = sources.map((source, index) => normalizedPopulation(source, modes[index]))
const hashes = projections.map(exactSha256)
if (hashes.some((hash) => hash !== expectedPopulationSha256)) {
  throw new Error(`canonical mode-17 projection changed: ${JSON.stringify(hashes)}`)
}
if (losslessCanonicalJson(projections[0]) !== losslessCanonicalJson(projections[3])) {
  throw new Error('mode-17 replay changed after the articulation probe sequence')
}
if (losslessCanonicalJson(projections[1]) !== losslessCanonicalJson(projections[2])) {
  throw new Error('mode-25 replay changed after normalization')
}
if (losslessCanonicalJson(projections[0]) !== losslessCanonicalJson(projections[1])) {
  throw new Error('mode-25 changed the mode-17 population outside its additive sidecar')
}

const mode25Runs = [
  parsedMode25Output(sources[1], 2),
  parsedMode25Output(sources[2], 3),
]
const completeMode25Populations = mode25Runs.map(({ population }) => population)
const completeMode25PopulationHashes = completeMode25Populations.map(exactSha256)
if (losslessCanonicalJson(completeMode25Populations[0]) !== losslessCanonicalJson(completeMode25Populations[1])) {
  throw new Error(
    `complete mode-25 population or vacancy-articulation sidecar changed: ${JSON.stringify(completeMode25PopulationHashes)}`,
  )
}

const comparableMode25Outputs = mode25Runs.map(({ output }) =>
  withoutExplicitlyAllowedReplayFields(output),
)
if (losslessCanonicalJson(comparableMode25Outputs[0]) !== losslessCanonicalJson(comparableMode25Outputs[1])) {
  throw new Error('mode-25 output changed outside the explicit timing/OS replay allowlist')
}

process.stdout.write(
  `vacancy-articulation replay: ok (${modes.join(',')} -> ${expectedPopulationSha256}; mode25=${completeMode25PopulationHashes[0]})\n`,
)
