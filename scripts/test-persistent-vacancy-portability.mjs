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

const runBenchmarkSource = (benchmarkArguments, label) => {
  const result = spawnSync(executable, benchmarkArguments, {
    cwd: root,
    encoding: 'utf8',
    maxBuffer: 8 * 1024 * 1024,
  })
  if (result.error) throw result.error
  if (result.status !== 0) {
    throw new Error(`${label} failed (${result.status}): ${result.stderr}`)
  }
  return result.stdout
}

const runBenchmark = (benchmarkArguments, label) => {
  return JSON.parse(runBenchmarkSource(benchmarkArguments, label))
}

const runModeSource = (mode) => {
  const benchmarkArguments = [...arguments_]
  benchmarkArguments[benchmarkArguments.length - 2] = String(mode)
  return runBenchmarkSource(benchmarkArguments, `persistent-vacancy mode ${mode}`)
}

const runMode = (mode) => {
  return JSON.parse(runModeSource(mode))
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
const losslessPopulation = (source) =>
  parseLosslessJson(source).relaxedDiagnostics?.coupledDynamicSeparator
    ?.persistentVacancyPopulation

const adjacentUnsafeIntegers = parseLosslessJson(
  '{"first":9007199254740992,"second":9007199254740993}',
)
if (
  losslessCanonicalJson(adjacentUnsafeIntegers.first) ===
  losslessCanonicalJson(adjacentUnsafeIntegers.second)
) {
  throw new Error('lossless JSON replay aliases adjacent unsafe integers')
}

const assertBoundedPartialTerminal = (candidate, label) => {
  if (!candidate?.attempted) {
    throw new Error(`${label} did not execute: ${candidate?.failureReason ?? 'missing diagnostics'}`)
  }
  if (candidate.capExhausted !== null) {
    throw new Error(`${label} exhausted a cap: ${candidate.capExhausted}`)
  }
  if (
    candidate.exactValid !== false ||
    candidate.layersCompleted !== 40 ||
    candidate.layers.length !== 40 ||
    candidate.failureReason !==
      'persistent vacancy population exhausted its bounded layers without a complete state' ||
    candidate.publicationRejections !== 0 ||
    candidate.work.partialAudits !== 41 ||
    candidate.work.completeAudits !== 0 ||
    candidate.layers.some((layer) => layer.retainedStates !== 8)
  ) {
    throw new Error(`${label} did not reach the expected exact-valid bounded partial terminal`)
  }
}

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

const output = runMode(3)
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
assertBoundedPartialTerminal(population, 'persistent-vacancy mode 3')
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
const trajectory = structuredClone(population)
delete trajectory.work.retainedPeakBytes
delete trajectory.work.selectorDiagnosticPeakBytes
delete trajectory.work.totalRetainedPeakBytes
const populationSha256 = createHash('sha256')
  .update(JSON.stringify(canonical(trajectory)))
  .digest('hex')
if (populationSha256 !== '6f074367e6c665f5d93d4f1de0a1e7911a4a3557f312b423107c96a2fe9d46f2') {
  throw new Error(`persistent-vacancy trajectory changed: ${populationSha256}`)
}

const mode8 =
  runMode(8).relaxedDiagnostics?.coupledDynamicSeparator?.persistentVacancyPopulation
const mode9 =
  runMode(9).relaxedDiagnostics?.coupledDynamicSeparator?.persistentVacancyPopulation
const mode10 =
  runMode(10).relaxedDiagnostics?.coupledDynamicSeparator?.persistentVacancyPopulation
const mode14Source = runModeSource(14)
const mode15Source = runModeSource(15)
const mode14ReplaySource = runModeSource(14)
const mode15ReplaySource = runModeSource(15)
const mode14 =
  JSON.parse(mode14Source).relaxedDiagnostics?.coupledDynamicSeparator
    ?.persistentVacancyPopulation
const mode15 =
  JSON.parse(mode15Source).relaxedDiagnostics?.coupledDynamicSeparator
    ?.persistentVacancyPopulation
const mode14Replay =
  JSON.parse(mode14ReplaySource).relaxedDiagnostics?.coupledDynamicSeparator
    ?.persistentVacancyPopulation
const mode15Replay =
  JSON.parse(mode15ReplaySource).relaxedDiagnostics?.coupledDynamicSeparator
    ?.persistentVacancyPopulation
assertBoundedPartialTerminal(mode8, 'persistent-vacancy mode 8')
assertBoundedPartialTerminal(mode9, 'persistent-vacancy mode 9')
assertBoundedPartialTerminal(mode10, 'persistent-vacancy mode 10')
for (const [mode, candidate] of [
  [14, mode14],
  [15, mode15],
]) {
  if (
    !candidate?.attempted ||
    candidate.capExhausted !== null ||
    candidate.layersCompleted !== 40 ||
    candidate.layers.length !== 40 ||
    candidate.work.partialAudits !== 58 ||
    candidate.work.completeAudits !== 0 ||
    candidate.repairExpedition?.depths.length !== 16 ||
    candidate.repairExpedition?.completeEndpoint !== false
  ) {
    throw new Error(`persistent-vacancy mode ${mode} did not reach its bounded repair terminal`)
  }
}
const populationHistory = (candidate) =>
  candidate.layers.map((layer) => ({
    enteringPopulationHash: layer.elite.enteringPopulationHash,
    ordinaryChildOrderHash: layer.elite.ordinaryChildOrderHash,
    bestStateFingerprint: layer.bestStateFingerprint,
    bestInactivePieceCount: layer.bestInactivePieceCount,
    bestInactiveAreaGrid2: layer.bestInactiveAreaGrid2,
    bestEverAreaEliteFingerprint: layer.elite.bestEverAreaEliteFingerprint,
    bestEverCountEliteFingerprint: layer.elite.bestEverCountEliteFingerprint,
  }))
if (JSON.stringify(populationHistory(population)) !== JSON.stringify(populationHistory(mode8))) {
  throw new Error('macro compute-but-discard control changed the mode-3 population history')
}
if (
  mode8.layers.some(
    (layer) =>
      layer.macroExpansion.admittedChildren !== 0 ||
      layer.macroExpansion.retainedChildFingerprints.length !== 0,
  )
) {
  throw new Error('macro control admitted or retained a treatment child')
}
const mode9RetainedNovel = mode9.layers.reduce(
  (total, layer) => total + layer.macroExpansion.retainedChildFingerprints.length,
  0,
)
if (mode9RetainedNovel === 0) {
  throw new Error('macro treatment retained no novel depth-two child')
}
const mode8ByEntering = new Map(
  mode8.layers.map((layer) => [layer.elite.enteringPopulationHash, layer]),
)
let sharedEnteringPopulations = 0
for (const treatmentLayer of mode9.layers) {
  const controlLayer = mode8ByEntering.get(treatmentLayer.elite.enteringPopulationHash)
  if (!controlLayer) continue
  sharedEnteringPopulations += 1
  const controlMacro = structuredClone(controlLayer.macroExpansion)
  const treatmentMacro = structuredClone(treatmentLayer.macroExpansion)
  delete controlMacro.admittedChildren
  delete treatmentMacro.admittedChildren
  delete controlMacro.retainedChildFingerprints
  delete treatmentMacro.retainedChildFingerprints
  if (
    controlLayer.elite.ordinaryChildOrderHash !==
      treatmentLayer.elite.ordinaryChildOrderHash ||
    controlLayer.elite.completeCandidateOrderHash !==
      treatmentLayer.elite.completeCandidateOrderHash ||
    JSON.stringify(controlLayer.elite.preCarryoverWork) !==
      JSON.stringify(treatmentLayer.elite.preCarryoverWork) ||
    JSON.stringify(controlMacro) !== JSON.stringify(treatmentMacro)
  ) {
    throw new Error('macro control and treatment differ before admission at a shared population')
  }
}
if (sharedEnteringPopulations === 0) {
  throw new Error('macro control and treatment share no entering population')
}
const controlBest = mode8.layers.at(-1).elite
const treatmentBest = mode9.layers.at(-1).elite
if (
  treatmentBest.bestEverAreaEliteInactivePieceCount >
    controlBest.bestEverAreaEliteInactivePieceCount ||
  BigInt(treatmentBest.bestEverAreaEliteInactiveAreaGrid2) >=
    BigInt(controlBest.bestEverAreaEliteInactiveAreaGrid2)
) {
  throw new Error('macro treatment did not preserve count and strictly improve inactive area')
}
const preservedBest = mode10.layers.at(-1).elite
const mode10Trajectory = structuredClone(mode10)
delete mode10Trajectory.work.retainedPeakBytes
delete mode10Trajectory.work.selectorDiagnosticPeakBytes
delete mode10Trajectory.work.totalRetainedPeakBytes
delete mode10Trajectory.preExpeditionWork
delete mode10Trajectory.preExpeditionBehaviorHash
for (const layer of mode10Trajectory.layers) delete layer.retainedPopulationHash
const mode10TrajectorySha256 = createHash('sha256')
  .update(JSON.stringify(canonical(mode10Trajectory)))
  .digest('hex')
if (mode10TrajectorySha256 !== '1edb02e2fcacfa5c3d749cb228eee735744171f5c25993c09daa9cd8054b7709') {
  throw new Error(`preserved-best trajectory changed: ${mode10TrajectorySha256}`)
}
const expectedPreExpeditionBehaviorHash =
  'a0043940706bda90b41b8901445503702a9dcd5fe13d048cbb7f7d00200ae403'
if (
  mode10.preExpeditionBehaviorHash !== expectedPreExpeditionBehaviorHash ||
  mode14.preExpeditionBehaviorHash !== mode10.preExpeditionBehaviorHash ||
  mode15.preExpeditionBehaviorHash !== mode10.preExpeditionBehaviorHash
) {
  throw new Error('repair modes changed the protected pre-expedition behavior')
}
const exactMode14 = losslessPopulation(mode14Source)
const exactMode15 = losslessPopulation(mode15Source)
const exactMode14Replay = losslessPopulation(mode14ReplaySource)
const exactMode15Replay = losslessPopulation(mode15ReplaySource)
if (
  losslessCanonicalJson(exactMode14) !== losslessCanonicalJson(exactMode14Replay) ||
  losslessCanonicalJson(exactMode15) !== losslessCanonicalJson(exactMode15Replay)
) {
  throw new Error('repair mode replay changed, including its deterministic memory peaks')
}
const exactLegacyHashes = {
  mode14Population: exactSha256(exactMode14),
  mode14Repair: exactSha256(exactMode14.repairExpedition),
  mode15Population: exactSha256(exactMode15),
  mode15Repair: exactSha256(exactMode15.repairExpedition),
}
const expectedLegacyHashes = {
  mode14Population: '0f07d554b86b1f38376871f2a6933e48d44ce806ce0d62679025d148d05dacab',
  mode14Repair: '79c9fee76ffba4fb9c8a3489c191acd742b8d63cbb14a5708d3258818358f618',
  mode15Population: '216445040ff4982d3ce5605a610c830f08ba243de9b09e543b15c0734c77490d',
  mode15Repair: '2350b92068d9aa71575db53aa25bd6b04984bd551d02e1ecc7e292692feec86d',
}
if (JSON.stringify(exactLegacyHashes) !== JSON.stringify(expectedLegacyHashes)) {
  throw new Error(`repair legacy replay oracle changed: ${JSON.stringify(exactLegacyHashes)}`)
}
const controlDepthZero = mode14.repairExpedition.depths[0]
const treatmentDepthZero = mode15.repairExpedition.depths[0]
for (const key of [
  'expandedParents',
  'generatedChildren',
  'deduplicatedChildren',
  'transposedChildren',
  'completeCandidates',
  'directInsertions',
  'ejectionInsertions',
]) {
  if (controlDepthZero[key] !== treatmentDepthZero[key]) {
    throw new Error(`repair modes diverged before queue admission at ${key}`)
  }
}
if (JSON.stringify(controlDepthZero.work) !== JSON.stringify(treatmentDepthZero.work)) {
  throw new Error('repair modes changed generator work before queue admission')
}
if (
  JSON.stringify(controlDepthZero.expansions) !== JSON.stringify(treatmentDepthZero.expansions)
) {
  throw new Error('repair modes changed the root expansion before queue admission')
}
if (controlDepthZero.frontierHash === treatmentDepthZero.frontierHash) {
  throw new Error('repair scheduler treatment never crossed its augmented-queue boundary')
}
const generatorProjection = (expansion) => ({
  parentStateFingerprint: expansion.parentStateFingerprint,
  selectedPieceId: expansion.selectedPieceId,
  transitionSeed: expansion.transitionSeed,
  angleSeed: expansion.angleSeed,
  diversitySeed: expansion.diversitySeed,
  proposalOrderHash: expansion.proposalOrderHash,
  exactRowOrderHash: expansion.exactRowOrderHash,
  generatedChildOrderHash: expansion.generatedChildOrderHash,
  work: expansion.work,
})
const sharedGeneratorRows = new Map()
for (const [arm, candidate] of [
  ['control', mode14],
  ['treatment', mode15],
]) {
  for (const depth of candidate.repairExpedition.depths) {
    for (const expansion of depth.expansions) {
      const key = `${expansion.parentStateFingerprint}\0${expansion.selectedPieceId}`
      const entry = sharedGeneratorRows.get(key) ?? { arms: new Set(), projections: new Set() }
      entry.arms.add(arm)
      entry.projections.add(JSON.stringify(generatorProjection(expansion)))
      sharedGeneratorRows.set(key, entry)
    }
  }
}
let sharedSemanticHeadRows = 0
for (const entry of sharedGeneratorRows.values()) {
  if (entry.arms.size !== 2) continue
  sharedSemanticHeadRows += 1
  if (entry.projections.size !== 1) {
    throw new Error('shared repair semantic-state/head pair changed its generator stream')
  }
}
if (sharedSemanticHeadRows === 0) {
  throw new Error('repair arms share no semantic-state/head generator row')
}
const controlRepair = mode14.repairExpedition
const treatmentRepair = mode15.repairExpedition
if (
  controlRepair.rootStateFingerprint !== treatmentRepair.rootStateFingerprint ||
  controlRepair.rootInactivePieceCount !== 10 ||
  treatmentRepair.rootInactivePieceCount !== 10 ||
  treatmentRepair.endpointInactivePieceCount > controlRepair.endpointInactivePieceCount ||
  BigInt(treatmentRepair.endpointInactiveAreaGrid2) >=
    BigInt(controlRepair.endpointInactiveAreaGrid2) ||
  !treatmentRepair.endpointParetoImprovesRoot
) {
  throw new Error('displaced-first repair did not strictly beat its global-hardest control')
}
if (
  controlRepair.endpointInactivePieceCount !== 10 ||
  controlRepair.endpointInactiveAreaGrid2 !== '50292855011' ||
  treatmentRepair.endpointInactivePieceCount !== 10 ||
  treatmentRepair.endpointInactiveAreaGrid2 !== '45454946952'
) {
  throw new Error('repair endpoint oracle changed')
}
const firstPreservedParentLayer = mode10.layers.findIndex(
  (layer) =>
    layer.macroExpansion.parentOrigin === 'bestEverArea' &&
    layer.macroExpansion.preservedParentAbsentFromOrdinary === true,
)
if (firstPreservedParentLayer <= 0) {
  throw new Error('preserved-best macro treatment never expanded an absent incumbent')
}
const mode9ByEntering = new Map(
  mode9.layers.map((layer) => [layer.elite.enteringPopulationHash, layer]),
)
for (const treatmentLayer of mode10.layers.slice(0, firstPreservedParentLayer)) {
  const controlLayer = mode9ByEntering.get(treatmentLayer.elite.enteringPopulationHash)
  if (!controlLayer) {
    throw new Error('mode 10 diverged before its first absent-preserved-parent expansion')
  }
  const controlMacro = structuredClone(controlLayer.macroExpansion)
  const treatmentMacro = structuredClone(treatmentLayer.macroExpansion)
  delete treatmentMacro.parentOrigin
  delete treatmentMacro.preservedParentAbsentFromOrdinary
  if (
    controlLayer.elite.ordinaryChildOrderHash !==
      treatmentLayer.elite.ordinaryChildOrderHash ||
    controlLayer.elite.completeCandidateOrderHash !==
      treatmentLayer.elite.completeCandidateOrderHash ||
    JSON.stringify(controlLayer.elite.preCarryoverWork) !==
      JSON.stringify(treatmentLayer.elite.preCarryoverWork) ||
    JSON.stringify(controlMacro) !== JSON.stringify(treatmentMacro) ||
    controlLayer.bestStateFingerprint !== treatmentLayer.bestStateFingerprint
  ) {
    throw new Error('mode 10 changed the mode-9 stream before its active treatment layer')
  }
}
const firstTreatmentLayer = mode10.layers[firstPreservedParentLayer]
const firstTreatmentControl = mode9ByEntering.get(
  firstTreatmentLayer.elite.enteringPopulationHash,
)
if (
  !firstTreatmentControl ||
  firstTreatmentControl.elite.ordinaryChildOrderHash !==
    firstTreatmentLayer.elite.ordinaryChildOrderHash
) {
  throw new Error('mode 10 changed the ordinary stream at its first active treatment layer')
}
for (const key of [
  'selectedPieceSlots',
  'orientationStreams',
  'exactFinalistRows',
  'partialAudits',
  'completeAudits',
]) {
  if (mode10.work[key] !== mode9.work[key]) {
    throw new Error(`preserved-best macro treatment changed fixed work counter ${key}`)
  }
}
if (
  preservedBest.bestEverAreaEliteInactivePieceCount !== 11 ||
  preservedBest.bestEverAreaEliteInactiveAreaGrid2 !== '47975977789'
) {
  throw new Error(
    `preserved-best endpoint changed: ${preservedBest.bestEverAreaEliteInactivePieceCount}/${preservedBest.bestEverAreaEliteInactiveAreaGrid2}`,
  )
}
if (
  preservedBest.bestEverAreaEliteInactivePieceCount >
    treatmentBest.bestEverAreaEliteInactivePieceCount ||
  BigInt(preservedBest.bestEverAreaEliteInactiveAreaGrid2) >=
    BigInt(treatmentBest.bestEverAreaEliteInactiveAreaGrid2)
) {
  throw new Error('preserved-best macro treatment did not preserve count and improve mode 9')
}
process.stdout.write(
  `persistent-vacancy portability: ok (${populationSha256}; ${mode9RetainedNovel} retained macro states; first preserved-parent layer ${firstPreservedParentLayer})\n`,
)
