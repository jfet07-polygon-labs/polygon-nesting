import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
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
const executable = join(
  target,
  'release',
  'examples',
  `general_request_benchmark${process.platform === 'win32' ? '.exe' : ''}`,
)
const arguments_ = [
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
  '3',
  fixture,
]

const missingFixture = spawnSync(executable, arguments_.slice(0, -1), {
  cwd: root,
  encoding: 'utf8',
  maxBuffer: 4 * 1024 * 1024,
})
if (missingFixture.status === 0) {
  throw new Error('persistent-vacancy mode succeeded without a frozen parent fixture')
}
if (!missingFixture.stderr.includes('parent fixture path is required')) {
  throw new Error(`unexpected missing-fixture failure: ${missingFixture.stderr}`)
}

const wrongFixtureArguments = [...arguments_]
wrongFixtureArguments[wrongFixtureArguments.length - 1] = request
const wrongFixture = spawnSync(executable, wrongFixtureArguments, {
  cwd: root,
  encoding: 'utf8',
  maxBuffer: 4 * 1024 * 1024,
})
if (wrongFixture.status === 0) {
  throw new Error('persistent-vacancy mode accepted a parent fixture with the wrong hash')
}
if (!wrongFixture.stderr.includes('parent fixture hash mismatch')) {
  throw new Error(`unexpected wrong-fixture failure: ${wrongFixture.stderr}`)
}

const settingsMismatchArguments = [...arguments_]
settingsMismatchArguments[16] = '2699'
const settingsMismatch = spawnSync(executable, settingsMismatchArguments, {
  cwd: root,
  encoding: 'utf8',
  maxBuffer: 4 * 1024 * 1024,
})
if (settingsMismatch.status === 0) {
  throw new Error('persistent-vacancy mode accepted mismatched effective geometry settings')
}
if (!settingsMismatch.stderr.includes('parent fixture settings mismatch')) {
  throw new Error(`unexpected settings-mismatch failure: ${settingsMismatch.stderr}`)
}

const disabledWithFixtureArguments = [...arguments_]
disabledWithFixtureArguments[disabledWithFixtureArguments.length - 2] = '0'
const disabledWithFixture = spawnSync(executable, disabledWithFixtureArguments, {
  cwd: root,
  encoding: 'utf8',
  maxBuffer: 4 * 1024 * 1024,
})
if (disabledWithFixture.status === 0) {
  throw new Error('disabled persistent-vacancy mode accepted a parent fixture')
}
if (!disabledWithFixture.stderr.includes('accepted only when persistent vacancy mode is nonzero')) {
  throw new Error(`unexpected disabled-with-fixture failure: ${disabledWithFixture.stderr}`)
}

const modeZeroArguments = arguments_.slice(0, -2)
modeZeroArguments.push('0')
const modeZero = spawnSync(executable, modeZeroArguments, {
  cwd: root,
  encoding: 'utf8',
  maxBuffer: 4 * 1024 * 1024,
})
if (modeZero.error) throw modeZero.error
if (modeZero.status !== 0) {
  throw new Error(`mode-zero compatibility run failed (${modeZero.status}): ${modeZero.stderr}`)
}
const modeZeroOutput = JSON.parse(modeZero.stdout)
if (Object.hasOwn(modeZeroOutput, 'persistentVacancyParentFixture')) {
  throw new Error('mode-zero output gained persistentVacancyParentFixture')
}

const result = spawnSync(executable, arguments_, {
  cwd: root,
  encoding: 'utf8',
  maxBuffer: 4 * 1024 * 1024,
})
if (result.error) throw result.error
if (result.status !== 0) {
  throw new Error(`persistent-vacancy benchmark failed (${result.status}): ${result.stderr}`)
}
const output = JSON.parse(result.stdout)
const fixtureSha256 = createHash('sha256').update(readFileSync(fixture)).digest('hex')
if (fixtureSha256 !== '18e0b052997d1251573fa35679c9fcf1d5e796acf771ec48f320ce4e9bf0081d') {
  throw new Error(`persistent-vacancy fixture hash changed: ${fixtureSha256}`)
}
if (output.persistentVacancyParentFixture?.sha256 !== fixtureSha256) {
  throw new Error('benchmark did not report the frozen parent fixture hash')
}
const population =
  output.relaxedDiagnostics?.coupledDynamicSeparator?.persistentVacancyPopulation
if (!population?.attempted) {
  throw new Error(
    `persistent-vacancy arm was not attempted: ${population?.failureReason ?? 'missing diagnostics'}`,
  )
}
if (
  population.parentFingerprint !==
  'b9335a72cdcdd8df29be21450818f4ab1766ea1ea0b16765ad3998942a2ea6c5'
) {
  throw new Error(`persistent-vacancy parent changed: ${population.parentFingerprint}`)
}
const canonical = (value) => {
  if (Array.isArray(value)) return value.map(canonical)
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.keys(value)
        .sort()
        .map((key) => [key, canonical(value[key])]),
    )
  }
  return value
}
const populationSha256 = createHash('sha256')
  .update(JSON.stringify(canonical(population)))
  .digest('hex')
if (populationSha256 !== 'eeddb6241d98ac94cbf378a5f03cfa0173b87755feb8ccac4235cb46689b6efa') {
  throw new Error(`persistent-vacancy trajectory changed: ${populationSha256}`)
}
process.stdout.write(`persistent-vacancy portability: ok (${populationSha256})\n`)
