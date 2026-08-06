import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

const REPOSITORY_ROOT = dirname(dirname(dirname(fileURLToPath(import.meta.url))))
const ci = readFileSync(join(REPOSITORY_ROOT, '.github', 'workflows', 'ci.yml'), 'utf8')
const parity = readFileSync(join(REPOSITORY_ROOT, '.github', 'workflows', 'standalone-parity.yml'), 'utf8')
const release = readFileSync(join(REPOSITORY_ROOT, '.github', 'workflows', 'release.yml'), 'utf8')
const rustCacheAction = readFileSync(join(REPOSITORY_ROOT, '.github', 'actions', 'setup-rust-cache', 'action.yml'), 'utf8')

function remoteActionReferences(workflow) {
  return [...workflow.matchAll(/^\s*uses:\s+([^\s]+)\s*$/gm)]
    .map(([, reference]) => reference)
    .filter((reference) => !reference.startsWith('./'))
}

function assertRemoteActionsPinned(workflow) {
  for (const reference of remoteActionReferences(workflow)) {
    assert.match(reference, /^[^@\s]+@[a-f0-9]{40}$/i, `remote action must use a full commit SHA: ${reference}`)
  }
}

function cachedCargoPaths(action) {
  const [, block] = action.match(/path:\s*\|\n((?:\s+[^\n]+\n)+?)\s+key:/) ?? []
  assert.notEqual(block, undefined, 'Cargo cache paths are required')
  return block.trim().split('\n').map((path) => path.trim())
}

function restorePrefixes(action) {
  const [, block] = action.match(/restore-keys:\s*\|\n((?: {10}[^\n]+\n?)+)/) ?? []
  assert.notEqual(block, undefined, 'Cargo cache restore prefix is required')
  return block.trim().split('\n').map((prefix) => prefix.trim())
}

function assertCargoCacheContract(action) {
  assert.deepEqual(cachedCargoPaths(action), [
    '~/.cargo/registry/index',
    '~/.cargo/registry/cache',
    '~/.cargo/git/db'
  ])
  assert.deepEqual(restorePrefixes(action), [
    'rust-cache-v1-${{ runner.os }}-${{ runner.arch }}-rust-1.95.0-'
  ])
  assert.match(action, /key:\s*rust-cache-v1-\$\{\{ runner\.os \}\}-\$\{\{ runner\.arch \}\}-rust-1\.95\.0-\$\{\{ hashFiles\('Cargo\.lock'\) \}\}/)
  assert.match(action, /node scripts\/ensure-sccache\.mjs/)
  assert.match(action, /SCCACHE_VERSION:\s*0\.10\.0/)
  assert.match(action, /RUSTC_WRAPPER=sccache/)
  assert.match(action, /SCCACHE_GHA_ENABLED=true/)
  assert.match(action, /CARGO_INCREMENTAL=0/)
}

function workflowJobs(workflow) {
  const [, jobs] = workflow.match(/^jobs:\n([\s\S]*)/m) ?? []
  assert.notEqual(jobs, undefined, 'workflow must define jobs')
  return [...jobs.matchAll(/^  ([\w-]+):\n([\s\S]*?)(?=^  [\w-]+:\n|(?![\s\S]))/gm)]
    .map(([, name, job]) => ({ name, job }))
}

function workflowJob(workflow, name) {
  const job = workflowJobs(workflow).find((candidate) => candidate.name === name)?.job
  assert.notEqual(job, undefined, `workflow must define the ${name} job`)
  return job
}

function jobEnvironment(job) {
  const [, environment] = job.match(/^    env:\n((?:^      [^\n]*\n)*)/m) ?? []
  return environment ?? ''
}

function assertNoRunnerContextInJobEnvironment(workflow) {
  for (const { name, job } of workflowJobs(workflow)) {
    assert.doesNotMatch(
      jobEnvironment(job),
      /\$\{\{\s*runner\./,
      `runner context is not allowed in jobs.${name}.env`
    )
  }
}

function targetDirectoryPreparation(job) {
  const marker = '      - name: Prepare fresh Cargo target directory\n'
  const markerIndex = job.indexOf(marker)
  assert.notEqual(markerIndex, -1, 'Rust-producing job must prepare a fresh Cargo target directory')
  const scriptMarker = '        run: |\n'
  const scriptStart = job.indexOf(scriptMarker, markerIndex)
  assert.notEqual(scriptStart, -1, 'target-directory preparation must use a shell run block')
  const bodyStart = scriptStart + scriptMarker.length
  const nextStep = job.indexOf('\n      - ', bodyStart)
  return job.slice(bodyStart, nextStep === -1 ? job.length : nextStep)
    .replace(/^ {10}/gm, '')
}

function targetDirectoryAssignments(job) {
  return [...targetDirectoryPreparation(job).matchAll(/^CARGO_TARGET_DIR="([^"]+)"$/gm)]
    .map(([, directory]) => directory)
}

function assertFreshTargetDirectory(job, expectedRoot, compilePattern) {
  assert.doesNotMatch(jobEnvironment(job), /^      CARGO_TARGET_DIR:/m)
  const preparation = targetDirectoryPreparation(job)
  assert.equal(
    preparation.split('\n', 1)[0],
    `CARGO_TARGET_DIR="${expectedRoot}"`,
    'target-directory preparation must use the exact runner-temp root'
  )
  assert.match(preparation, /rm -rf "\$CARGO_TARGET_DIR"/)
  assert.match(preparation, /mkdir -p "\$CARGO_TARGET_DIR"/)
  assert.match(preparation, /test -z "\$\(find "\$CARGO_TARGET_DIR" -mindepth 1 -print -quit\)"/)
  assert.match(preparation, /printf 'CARGO_TARGET_DIR=%s\\n' "\$CARGO_TARGET_DIR" >> "\$GITHUB_ENV"/)
  const preparationIndex = job.indexOf('      - name: Prepare fresh Cargo target directory')
  const compileIndex = job.search(compilePattern)
  assert.ok(compileIndex > preparationIndex, 'fresh target-directory preparation must precede compilation')
}

function matrixValues(job, property) {
  return [...job.matchAll(new RegExp(`^          - ${property}: (.+)$`, 'gm'))]
    .map(([, value]) => value)
}

function assertCiTriggerContract(workflow) {
  const [, triggerBlock] = workflow.match(/^on:\n([\s\S]*?)^concurrency:/m) ?? []
  assert.equal(triggerBlock, [
    '  push:',
    '    branches:',
    '      - main',
    '  pull_request:',
    '  workflow_dispatch:',
    '    inputs:',
    '      parity_source_run_id:',
    '        description: Explicit jfet07-polygon-labs/polygon-nesting run ID containing the trusted aggregate parity bundle',
    '        required: false',
    '        type: string',
    '',
    ''
  ].join('\n'), 'CI trigger contract must be exact and unrestricted')
}

function assertDistinctCargoTargetRoots(ciWorkflow, parityWorkflow) {
  assertNoRunnerContextInJobEnvironment(ciWorkflow)
  assertNoRunnerContextInJobEnvironment(parityWorkflow)
  const quality = workflowJob(ciWorkflow, 'quality')
  const native = workflowJob(ciWorkflow, 'native')
  const parityJob = workflowJob(parityWorkflow, 'parity')
  assertFreshTargetDirectory(quality, '$RUNNER_TEMP/cargo-target-quality', /cargo clippy --workspace/)
  assertFreshTargetDirectory(native, '$RUNNER_TEMP/cargo-target-native-${TARGET_KEY}', /npm run build:release/)
  assertFreshTargetDirectory(parityJob, '$RUNNER_TEMP/cargo-target-parity-${{ matrix.key }}', /cargo build --locked --release/)

  assert.deepEqual(targetDirectoryAssignments(quality), [
    '$RUNNER_TEMP/cargo-target-quality'
  ], 'quality job Cargo target root must be exact')
  assert.deepEqual(targetDirectoryAssignments(native), [
    '$RUNNER_TEMP/cargo-target-native-${TARGET_KEY}'
  ], 'native job Cargo target root must be exact')
  assert.deepEqual(targetDirectoryAssignments(parityJob), [
    '$RUNNER_TEMP/cargo-target-parity-${{ matrix.key }}'
  ], 'parity job Cargo target root must be exact')

  const nativeRoots = matrixValues(native, 'target-key').map((key) => `cargo-target-native-${key}`)
  const parityRoots = matrixValues(parityJob, 'key').map((key) => `cargo-target-parity-${key}`)
  assert.deepEqual(nativeRoots, [
    'cargo-target-native-linux-x64',
    'cargo-target-native-win32-x64',
    'cargo-target-native-darwin-arm64',
    'cargo-target-native-darwin-x64'
  ])
  assert.deepEqual(parityRoots, [
    'cargo-target-parity-linux-x64',
    'cargo-target-parity-win32-x64',
    'cargo-target-parity-darwin-arm64',
    'cargo-target-parity-darwin-x64'
  ])
  const allRoots = ['cargo-target-quality', ...nativeRoots, ...parityRoots]
  assert.equal(new Set(allRoots).size, allRoots.length, 'Cargo target roots must be distinct')
}

function assertPinnedDockerBaseImages(dockerfile) {
  const images = [...dockerfile.matchAll(/^FROM\s+([^\s]+)(?:\s+AS\s+\w+)?$/gm)]
    .map(([, image]) => image)
  assert.equal(images.length, 2, 'Dockerfile must define exactly two base images')
  for (const image of images) assert.match(image, /^[^@\s]+@sha256:[a-f0-9]{64}$/i, `Docker base image requires a digest: ${image}`)
  assert.deepEqual(images, [
    'rust:1.95.0-bookworm@sha256:6258907abe69656e41cd992e0b705cdcfabcbbe3db374f92ed2d47121282d4a1',
    'debian:bookworm-slim@sha256:7b140f374b289a7c2befc338f42ebe6441b7ea838a042bbd5acbfca6ec875818'
  ])
}

function assertReleaseRegistryDigest(workflow) {
  const [, image] = workflow.match(/docker run --detach --publish 5000:5000 --name release-registry\s+(registry:[^\s]+)/) ?? []
  assert.equal(image, 'registry:2.8.3@sha256:a3d8aaa63ed8681a604f1dea0aa03f100d5895b6a58ace528858a7b332415373', 'release registry digest must be exact')
  assert.match(image, /^registry:2\.8\.3@sha256:[a-f0-9]{64}$/i, 'release registry digest must be immutable')
}

function assertExactElectronRuntime(workflow) {
  assert.match(
    workflow,
    /npx --yes --package=electron@39\.2\.7 electron -e/,
    'Electron runtime must use the exact tested version'
  )
}

function assertPrivateCiImagesPinned(workflow) {
  const images = workflow.match(/ghcr\.io\/[^\s"']+/g) ?? []
  for (const image of images) {
    assert.match(image, /@sha256:[a-f0-9]{64}$/i, `GHCR CI image must use a digest: ${image}`)
  }
}

test('CI only pushes main, tests workflow contracts, and preserves manual runs from cancellation', () => {
  assert.match(ci, /push:\n\s+branches:\n\s+- main/)
  assert.match(ci, /pull_request:/)
  assert.match(ci, /workflow_dispatch:/)
  assert.match(ci, /concurrency:\n\s+group:.*github\.event_name/s)
  assert.match(ci, /cancel-in-progress:\s*\$\{\{ github\.event_name != 'workflow_dispatch' \}\}/)
  assert.match(ci, /tests\/ci\/\*\.test\.mjs/)
})

test('Rust-producing CI jobs export fresh runner-temp target roots before compilation and use the shared safe cache action', () => {
  for (const workflow of [ci, parity]) {
    assert.match(workflow, /\.\/\.github\/actions\/setup-rust-cache/)
    assert.match(workflow, /--locked/)
  }
  assertDistinctCargoTargetRoots(ci, parity)
  assert.match(parity, /\$CARGO_TARGET_DIR\/\$\{\{ matrix\.target \}\}\/release\/polygon-nesting/)
  assert.match(parity, /\$CARGO_TARGET_DIR\/\$\{\{ matrix\.target \}\}\/release\/parity-desktop-request-adapter/)
})

test('workflow contracts reject runner context in job-level environment', () => {
  assertNoRunnerContextInJobEnvironment(ci)
  assertNoRunnerContextInJobEnvironment(parity)
  assert.throws(
    () => assertNoRunnerContextInJobEnvironment(
      ci.replace(
        '      TARGET_KEY: ${{ matrix.target-key }}\n',
        '      TARGET_KEY: ${{ matrix.target-key }}\n      BAD_RUNNER_PATH: ${{ runner.temp }}\n'
      )
    ),
    /runner context/
  )
})

test('Rust cache allowlist and restore prefix bind the complete dependency identity', () => {
  assertCargoCacheContract(rustCacheAction)
})

test('workflow contract rejects mutable action references, broad cache keys, and cached executable paths', () => {
  assert.throws(
    () => assertRemoteActionsPinned('uses: actions/checkout@stable\n'),
    /full commit SHA/
  )
  assert.throws(
    () => assertCargoCacheContract(rustCacheAction.replace(
      '~/.cargo/git/db',
      '~/.cargo/git/db\n          ~/.cargo/bin'
    )),
    /deep-equal|key/
  )
  assert.throws(
    () => assertCargoCacheContract(rustCacheAction.replace(
      'rust-cache-v1-${{ runner.os }}-${{ runner.arch }}-rust-1.95.0-',
      'rust-cache-v1-'
    )),
    /deepEqual|key/
  )
})

test('every remote action in workflows and the composite is pinned by a full commit SHA', () => {
  for (const workflow of [ci, parity, release, rustCacheAction]) assertRemoteActionsPinned(workflow)
  assert.match(release, /Publication remains disabled/)
  assert.match(release, /NODE_AUTH_TOKEN/)
})

test('CI trigger and Cargo target roots have the exact per-job contract', () => {
  assertCiTriggerContract(ci)
  assertDistinctCargoTargetRoots(ci, parity)
})

test('image and runtime contracts require immutable references', () => {
  const dockerfile = readFileSync(join(REPOSITORY_ROOT, 'Dockerfile'), 'utf8')
  assertPinnedDockerBaseImages(dockerfile)
  assertReleaseRegistryDigest(release)
  assertExactElectronRuntime(ci)
  assertPrivateCiImagesPinned(`${ci}\n${parity}\n${release}`)
})

test('workflow contracts reject mutable triggers, target roots, and image references', () => {
  const dockerfile = readFileSync(join(REPOSITORY_ROOT, 'Dockerfile'), 'utf8')
  assert.throws(
    () => assertCiTriggerContract(ci.replace('  pull_request:\n', '  pull_request:\n    branches:\n      - main\n')),
    /trigger/
  )
  assert.throws(
    () => assertCiTriggerContract(ci.replace('      - main\n  pull_request:', '      - main\n      - development\n  pull_request:')),
    /trigger/
  )
  assert.throws(
    () => assertDistinctCargoTargetRoots(
      ci.replace('cargo-target-native-${TARGET_KEY}', 'cargo-target-quality'),
      parity
    ),
    /distinct|target root|runner-temp root/
  )
  assert.throws(
    () => assertPinnedDockerBaseImages(dockerfile.replace(/@sha256:[a-f0-9]{64}/, '')),
    /digest/
  )
  assert.throws(
    () => assertReleaseRegistryDigest(release.replace(/@sha256:[a-f0-9]{64}/, '')),
    /registry digest/
  )
  assert.throws(
    () => assertExactElectronRuntime(ci.replace('electron@39.2.7', 'electron@latest')),
    /Electron runtime/
  )
  assert.throws(
    () => assertPrivateCiImagesPinned('container: ghcr.io/jfet97/polygon-nesting-ci:latest'),
    /GHCR CI image/
  )
})
