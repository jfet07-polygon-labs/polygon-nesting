import assert from 'node:assert/strict'
import { existsSync, readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

const REPOSITORY_ROOT = dirname(dirname(dirname(fileURLToPath(import.meta.url))))
const ci = readFileSync(join(REPOSITORY_ROOT, '.github', 'workflows', 'ci.yml'), 'utf8')
const parity = readFileSync(join(REPOSITORY_ROOT, '.github', 'workflows', 'standalone-parity.yml'), 'utf8')
const release = readFileSync(join(REPOSITORY_ROOT, '.github', 'workflows', 'release.yml'), 'utf8')
const runtimePublicationRequestPath = join(REPOSITORY_ROOT, '.github', 'workflows', 'request-runtime-image-publication.yml')
const runtimePublicationRequest = existsSync(runtimePublicationRequestPath) ? readFileSync(runtimePublicationRequestPath, 'utf8') : ''
const runtimePublicationPath = join(REPOSITORY_ROOT, '.github', 'workflows', 'publish-runtime-image.yml')
const runtimePublication = existsSync(runtimePublicationPath) ? readFileSync(runtimePublicationPath, 'utf8') : ''
const rustCacheAction = readFileSync(join(REPOSITORY_ROOT, '.github', 'actions', 'setup-rust-cache', 'action.yml'), 'utf8')
const CI_IMAGE_REFERENCE = 'ghcr.io/jfet07-polygon-labs/polygon-nesting-ci@sha256:66a7ca95c13074714135ad840465834e60797ddd45d13d130e0e5c6077b950f3'

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

function assertCiImageContainer(job) {
  assert.match(job, new RegExp(`^    container: ${CI_IMAGE_REFERENCE.replaceAll('.', '\\.')}$`, 'm'))
}

function assertNoCiImageContainer(job) {
  assert.doesNotMatch(job, /^    container:\s+ghcr\.io\/[^\s]+@sha256:[a-f0-9]{64}$/m)
}

function assertGitSafeDirectory(job) {
  assert.match(job, /git config --global --add safe\.directory "\$GITHUB_WORKSPACE"/)
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

function matrixIncludeItems(job) {
  const [, include] = job.match(/^    strategy:\n      fail-fast: false\n      matrix:\n        include:\n([\s\S]*?)(?=^    [\w-]+:|(?![\s\S]))/m) ?? []
  assert.notEqual(include, undefined, 'job must define a matrix include list')
  return include.trimEnd().split(/^          - /m).filter(Boolean).map((item) => Object.fromEntries(
    item.split('\n').filter(Boolean).map((line, index) => {
      const [, key, value] = line.match(index === 0 ? /^([^:]+): (.+)$/ : /^            ([^:]+): (.+)$/) ?? []
      assert.notEqual(key, undefined, `matrix item is malformed: ${line}`)
      return [key, value]
    })
  ))
}

function assertCiTriggerContract(workflow) {
  const [, triggerBlock] = workflow.match(/^on:\n([\s\S]*?)^concurrency:/m) ?? []
  assert.equal(triggerBlock, [
    '  push:',
    '    branches:',
    '      - main',
    '  pull_request:',
    '',
    ''
  ].join('\n'), 'CI trigger contract must be exact and unrestricted')
}

function assertReleaseTriggerContract(workflow) {
  const [, triggerBlock] = workflow.match(/^on:\n([\s\S]*?)^permissions:/m) ?? []
  assert.equal(triggerBlock, [
    '  workflow_dispatch:',
    '    inputs:',
    '      ci_run_id:',
    '        description: Successful main CI run ID containing exact release evidence',
    '        required: false',
    '        type: string',
    '  push:',
    '    tags:',
    "      - 'v*'",
    '',
    ''
  ].join('\n'), 'release triggers must remain limited to manual dispatch and version tags')
  assert.match(workflow, /path: 'Cargo\.toml'/)
  assert.match(workflow, /tagName !== `v\$\{version\}`/)
}

function assertStandaloneParityTriggerContract(workflow) {
  const [, triggerBlock] = workflow.match(/^on:\n([\s\S]*?)^permissions:/m) ?? []
  assert.equal(triggerBlock, [
    '  workflow_call:',
    '    inputs:',
    '      source-run:',
    '        description: Source jfet07-polygon-labs/min-plane-dxf workflow run ID',
    '        required: true',
    '        type: string',
    '      parity-bundle-path:',
    '        description: Fixed output path for trusted parity input',
    '        required: true',
    '        type: string',
    '  workflow_dispatch:',
    '    inputs:',
    '      source-run:',
    '        description: Source jfet07-polygon-labs/min-plane-dxf workflow run ID',
    '        required: true',
    '        type: string',
    '',
    ''
  ].join('\n'), 'standalone parity triggers must remain limited to workflow calls and manual dispatch')
}

function assertCiRunnerContract(workflow) {
  const native = workflowJob(workflow, 'native')
  assert.match(workflowJob(workflow, 'quality'), /^    runs-on: blacksmith-8vcpu-ubuntu-2404$/m)
  assert.match(workflowJob(workflow, 'oci-evidence'), /^    runs-on: blacksmith-2vcpu-ubuntu-2404$/m)
  assert.doesNotMatch(workflow, /^  parity-release-gate:$/m)
  assert.deepEqual(matrixIncludeItems(native), [
    {
      'target-key': 'linux-x64',
      runner: 'blacksmith-2vcpu-ubuntu-2404',
      platform: 'linux',
      arch: 'x64',
      'cargo-target': 'x86_64-unknown-linux-gnu'
    },
    {
      'target-key': 'darwin-arm64',
      runner: 'macos-15',
      platform: 'darwin',
      arch: 'arm64',
      'cargo-target': 'aarch64-apple-darwin'
    }
  ])
  assert.doesNotMatch(workflow, /darwin-x64|x86_64-apple-darwin|macos-15-intel/)
  assert.match(native, /^    if: github\.event_name == 'push'$/m)
}

function assertReleaseRunnerContract(workflow) {
  for (const jobName of ['authorize-source', 'resolve-ci', 'candidate', 'oci']) {
    assert.match(workflowJob(workflow, jobName), /^    runs-on: blacksmith-2vcpu-ubuntu-2404$/m)
  }
}

function assertDistinctCargoTargetRoots(ciWorkflow, parityWorkflow) {
  assertNoRunnerContextInJobEnvironment(ciWorkflow)
  assertNoRunnerContextInJobEnvironment(parityWorkflow)
  const quality = workflowJob(ciWorkflow, 'quality')
  const native = workflowJob(ciWorkflow, 'native')
  const parityJob = workflowJob(parityWorkflow, 'parity')
  const parityLinuxJob = workflowJob(parityWorkflow, 'parity-linux')
  assertFreshTargetDirectory(quality, '$RUNNER_TEMP/cargo-target-quality', /cargo clippy --workspace/)
  assertFreshTargetDirectory(native, '$RUNNER_TEMP/cargo-target-native-${TARGET_KEY}', /npm run build:release/)
  assertFreshTargetDirectory(parityJob, '$RUNNER_TEMP/cargo-target-parity-${{ matrix.key }}', /cargo build --locked --release/)
  assertFreshTargetDirectory(parityLinuxJob, '$RUNNER_TEMP/cargo-target-parity-${{ matrix.key }}', /cargo build --locked --release/)

  assert.deepEqual(targetDirectoryAssignments(quality), [
    '$RUNNER_TEMP/cargo-target-quality'
  ], 'quality job Cargo target root must be exact')
  assert.deepEqual(targetDirectoryAssignments(native), [
    '$RUNNER_TEMP/cargo-target-native-${TARGET_KEY}'
  ], 'native job Cargo target root must be exact')
  assert.deepEqual(targetDirectoryAssignments(parityJob), [
    '$RUNNER_TEMP/cargo-target-parity-${{ matrix.key }}'
  ], 'parity job Cargo target root must be exact')
  assert.deepEqual(targetDirectoryAssignments(parityLinuxJob), [
    '$RUNNER_TEMP/cargo-target-parity-${{ matrix.key }}'
  ], 'Linux parity job Cargo target root must be exact')

  const nativeRoots = matrixValues(native, 'target-key').map((key) => `cargo-target-native-${key}`)
  const parityRoots = [
    ...matrixValues(parityLinuxJob, 'key'),
    ...matrixValues(parityJob, 'key')
  ].map((key) => `cargo-target-parity-${key}`)
  assert.deepEqual(nativeRoots, [
    'cargo-target-native-linux-x64',
    'cargo-target-native-darwin-arm64'
  ])
  assert.deepEqual(parityRoots, [
    'cargo-target-parity-linux-x64',
    'cargo-target-parity-darwin-arm64'
  ])
  const allRoots = ['cargo-target-quality', ...nativeRoots, ...parityRoots]
  assert.equal(new Set(allRoots).size, allRoots.length, 'Cargo target roots must be distinct')
}

function assertPinnedDockerBaseImages(dockerfile) {
  const images = [...dockerfile.matchAll(/^FROM\s+([^\s]+)(?:\s+AS\s+\w+)?$/gm)]
    .map(([, image]) => image)
  const externalImages = images.filter((image) => image.includes('@sha256:'))
  assert.equal(externalImages.length, 2, 'Dockerfile must define exactly two external base images with digests')
  for (const image of externalImages) assert.match(image, /^[^@\s]+@sha256:[a-f0-9]{64}$/i, `Docker base image requires a digest: ${image}`)
  assert.deepEqual(externalImages, [
    'rust:1.95.0-bookworm@sha256:6258907abe69656e41cd992e0b705cdcfabcbbe3db374f92ed2d47121282d4a1',
    'debian:bookworm-slim@sha256:7b140f374b289a7c2befc338f42ebe6441b7ea838a042bbd5acbfca6ec875818'
  ])
}

function assertCiRegistryDigest(workflow) {
  const [, image] = workflow.match(/docker run --detach --publish 5000:5000 --name ci-registry\s+(registry:[^\s]+)/) ?? []
  assert.equal(image, 'registry:2.8.3@sha256:a3d8aaa63ed8681a604f1dea0aa03f100d5895b6a58ace528858a7b332415373', 'CI registry digest must be exact')
  assert.match(image, /^registry:2\.8\.3@sha256:[a-f0-9]{64}$/i, 'CI registry digest must be immutable')
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

function assertRuntimeSourceValidation(job) {
  assert.match(job, /run\.path !== '\.github\/workflows\/release\.yml'/)
  assert.match(job, /run\.status !== 'completed'/)
  assert.match(job, /run\.conclusion !== 'success'/)
  assert.match(job, /run\.repository\?\.full_name/)
  assert.match(job, /run\.head_sha/)
  assert.match(job, /inventory\.total_count !== expected\.length/)
  assert.match(job, /npm-release-candidate-\$\{run\.head_sha\}/)
  assert.match(job, /oci-release-candidate-\$\{run\.head_sha\}/)
  assert.match(job, /artifact\.expired !== false/)
}

function assertRuntimePublicationSecurityContract(workflow) {
  assert.deepEqual(
    workflowJobs(workflow).map(({ name }) => name),
    ['publish'],
    'runtime publication must define only the publish job'
  )
  const publish = workflowJob(workflow, 'publish')
  assert.match(
    publish,
    /^    if: \$\{\{ github\.event\.workflow_run\.conclusion == 'success' && github\.event\.workflow_run\.head_branch == 'main' \}\}$/m
  )
  assert.match(publish, /^    environment: publish$/m)
  assert.match(
    publish,
    /^          if \(comparison\.base_commit\?\.sha !== sourceCommit \|\| \(comparison\.status !== 'ahead' && comparison\.status !== 'identical'\)\) \{$/m,
    'runtime publication must execute the exact ancestry rejection condition'
  )
}

test('CI runs quality on pull requests and produces release evidence only on main pushes', () => {
  assert.match(ci, /push:\n\s+branches:\n\s+- main/)
  assert.match(ci, /pull_request:/)
  assert.doesNotMatch(ci, /workflow_dispatch:/)
  assert.match(ci, /concurrency:\n\s+group:.*github\.event_name/s)
  assert.match(ci, /cancel-in-progress: true/)
  assert.match(ci, /tests\/ci\/\*\.test\.mjs/)
  assert.match(workflowJob(ci, 'native'), /^    if: github\.event_name == 'push'$/m)
  assert.match(workflowJob(ci, 'oci-evidence'), /^    if: github\.event_name == 'push'$/m)
})

test('compatible Linux jobs use the immutable CI image and native jobs stay outside it', () => {
  assertCiImageContainer(workflowJob(ci, 'quality'))
  assertCiImageContainer(workflowJob(parity, 'parity-linux'))
  assertNoCiImageContainer(workflowJob(ci, 'oci-evidence'))
  assertNoCiImageContainer(workflowJob(ci, 'native'))
  assertNoCiImageContainer(workflowJob(parity, 'parity'))
  assert.match(workflowJob(parity, 'parity-linux'), /key: linux-x64[\s\S]*target: x86_64-unknown-linux-gnu/)
  assert.doesNotMatch(`${ci}\n${parity}`, /blacksmith-2vcpu-windows|x86_64-pc-windows-msvc/)
  assert.doesNotMatch(workflowJob(parity, 'parity-linux'), /actions\/setup-node|dtolnay\/rust-toolchain/)
  assert.match(workflowJob(parity, 'parity-linux'), /PATH: \/opt\/node-v24\.19\.0\/bin:/)
})

test('CI image container jobs trust the checked-out workspace before invoking Git', () => {
  assertGitSafeDirectory(workflowJob(ci, 'quality'))
  assertGitSafeDirectory(workflowJob(parity, 'parity-linux'))
})

test('Rust-producing CI jobs export fresh runner-temp target roots before compilation and use the shared safe cache action', () => {
  for (const workflow of [ci, parity]) {
    assert.match(workflow, /\.\/\.github\/actions\/setup-rust-cache/)
    assert.match(workflow, /--locked/)
  }
  assertDistinctCargoTargetRoots(ci, parity)
  assert.match(parity, /\$CARGO_TARGET_DIR\/\$\{\{ matrix\.target \}\}\/release\/polygon-nesting/)
  assert.match(parity, /\$CARGO_TARGET_DIR\/\$\{\{ matrix\.target \}\}\/release\/parity-desktop-request-adapter/)
  assert.doesNotMatch(parity, /aggregate:|require-all-targets:|assemble-parity-aggregate\.mjs/)
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
  for (const workflow of [ci, parity, release, runtimePublicationRequest, runtimePublication, rustCacheAction]) assertRemoteActionsPinned(workflow)
  assert.doesNotMatch(release, /publication-placeholder|Publication remains disabled/)
})

test('CI, release, and standalone parity trigger, runner, and Cargo target-root contracts are exact', () => {
  assertCiTriggerContract(ci)
  assertReleaseTriggerContract(release)
  assertStandaloneParityTriggerContract(parity)
  assertCiRunnerContract(ci)
  assertReleaseRunnerContract(release)
  assertDistinctCargoTargetRoots(ci, parity)
})

test('image and runtime contracts require immutable references', () => {
  const dockerfile = readFileSync(join(REPOSITORY_ROOT, 'Dockerfile'), 'utf8')
  assertPinnedDockerBaseImages(dockerfile)
  assertCiRegistryDigest(ci)
  assertExactElectronRuntime(ci)
  assertPrivateCiImagesPinned(`${ci}\n${parity}\n${release}`)
})

test('main CI builds runtime once, release reuses it, and publication consumes only a selected archived release', () => {
  assert.match(ci, /docker buildx build[\s\S]*--target runtime/)
  assert.match(ci, /--output type=oci,dest=oci-image\.tar/)
  assert.match(ci, /name: oci-build-\$\{\{ github\.sha \}\}/)
  assert.doesNotMatch(release, /docker buildx build|smoke-cli-image\.sh/)
  assert.match(release, /name: oci-build-\$\{\{ needs\.authorize-source\.outputs\.source-commit \}\}/)
  assert.doesNotMatch(release, /publication-placeholder|Publication remains disabled/)
  assert.notEqual(runtimePublicationRequest, '', 'runtime publication request workflow must exist')
  assert.notEqual(runtimePublication, '', 'runtime publication workflow must exist')
  const [, requestTriggerBlock] = runtimePublicationRequest.match(/^on:\n([\s\S]*?)^permissions:/m) ?? []
  assert.equal(requestTriggerBlock, [
    '  workflow_dispatch:',
    '    inputs:',
    '      release_run_id:',
    '        description: Successful completed release.yml run containing the immutable runtime OCI archive',
    '        required: true',
    '        type: string',
    '',
    ''
  ].join('\n'))
  assert.equal(runtimePublicationRequest.match(/^permissions:\n([\s\S]*?)^jobs:/m)?.[1], '  contents: read\n\n')
  assert.doesNotMatch(runtimePublicationRequest, /packages: write|environment: publish/)
  assert.match(runtimePublicationRequest, /release_run_id must be a positive integer/)
  assert.match(runtimePublicationRequest, /runtime-image-publication-request/)
  assert.match(runtimePublicationRequest, /retention-days: 1/)

  const [, triggerBlock] = runtimePublication.match(/^on:\n([\s\S]*?)^permissions:/m) ?? []
  assert.equal(triggerBlock, [
    '  workflow_run:',
    '    workflows:',
    '      - Request runtime image publication',
    '    types:',
    '      - completed',
    '',
    ''
  ].join('\n'))
  assert.doesNotMatch(runtimePublication, /workflow_dispatch|workflow_call/)
  assert.equal(runtimePublication.match(/^permissions:\n([\s\S]*?)^concurrency:/m)?.[1], '  contents: read\n  actions: read\n  packages: write\n\n')
  assertRuntimePublicationSecurityContract(runtimePublication)
  const publish = workflowJob(runtimePublication, 'publish')
  assertRuntimeSourceValidation(publish)
  assert.match(publish, /^    if: \$\{\{ github\.event\.workflow_run\.conclusion == 'success' && github\.event\.workflow_run\.head_branch == 'main' \}\}$/m)
  assert.match(publish, /run-id: \$\{\{ github\.event\.workflow_run\.id \}\}/)
  assert.match(publish, /name: runtime-image-publication-request/)
  assert.match(publish, /find publication-request -type f \| wc -l/)
  assert.match(publish, /test -f publication-request\/release-run-id\.txt/)
  assert.match(publish, /RELEASE_RUN_ID=.*release-run-id\.txt/)
  assert.match(publish, /compare\/\$source_commit\.\.\.main/)
  assert.match(publish, /comparison\.status !== 'ahead' && comparison\.status !== 'identical'/)
  assert.match(publish, /comparison\.base_commit\?\.sha !== sourceCommit/)
  assert.match(publish, /^    environment: publish$/m)
  assert.match(publish, /^    runs-on: blacksmith-2vcpu-ubuntu-2404$/m)
  assert.match(publish, /ref: \$\{\{ github\.event\.repository\.default_branch \}\}/)
  assert.match(publish, /gh api "repos\/\$GITHUB_REPOSITORY\/actions\/runs\/\$RELEASE_RUN_ID"/)
  assert.match(publish, /actions\/download-artifact@d3f86a106a0bac45b974a628896c90dbdf5c8093/)
  assert.match(publish, /name: oci-release-candidate-\$\{\{ steps\.source\.outputs\.source_commit \}\}/)
  assert.match(publish, /git archive --format=tar HEAD \| tar -xf -/)
  assert.match(publish, /test -f "\$verifier_root\/scripts\/smoke-cli-image\.sh"/)
  assert.match(publish, /node "\$RUNNER_TEMP\/trusted-publication-verifier\/scripts\/verify-runtime-publication\.mjs"/)
  assert.doesNotMatch(publish, /node scripts\/verify-runtime-publication\.mjs/)
  assert.match(publish, /--manifest-digest oci-candidate\/manifest-digest\.txt/)
  assert.match(publish, /MANIFEST_DIGEST=.*readFileSync\('runtime-verification\.json'/)
  assert.match(publish, /skopeo inspect --format/)
  assert.match(publish, /refusing to move existing tag to a different digest/)
  assert.match(publish, /skopeo copy oci-archive:oci-candidate\/oci-image\.tar/)
  assert.match(publish, /docker login ghcr\.io/)
  assert.match(publish, /registry-url: https:\/\/npm\.pkg\.github\.com/)
  assert.match(publish, /npm publish "\$GITHUB_NPM_TARBALL" --ignore-scripts --registry https:\/\/npm\.pkg\.github\.com/)
  assert.match(publish, /npm publish "\$PUBLIC_NPM_TARBALL" --ignore-scripts --registry https:\/\/registry\.npmjs\.org/)
  assert.match(publish, /npm view "@jfet07-polygon-labs\/polygon-nesting@\$RELEASE_VERSION" --json --registry https:\/\/npm\.pkg\.github\.com/)
  assert.match(publish, /npm view "@jfet97\/polygon-nesting@\$RELEASE_VERSION" --json --registry https:\/\/registry\.npmjs\.org/)
  assert.match(publish, /RELEASE_VERSION=.*release\.npmPackages\[0\]\.version/)
  assert.match(publish, /IMAGE_TAG=%s:%s/)
  assert.doesNotMatch(publish, /\(\?:\\\+\[0-9A-Za-z.-\]\+\)\?/, 'release versions used as OCI tags must reject build metadata')
  assert.match(publish, /NPM_TOKEN: \$\{\{ secrets\.NPM_TOKEN \}\}/)
  assert.match(publish, /\/\/registry\.npmjs\.org\/:_authToken=\$\{NPM_TOKEN\}/)
  assert.match(publish, /NPM_CONFIG_USERCONFIG="\$RUNNER_TEMP\/npmjs-publish\.npmrc" npm publish/)
  assert.doesNotMatch(publish, /NODE_AUTH_TOKEN: \$\{\{ secrets\.NPM_TOKEN \}\}/)
  assert.match(publish, /docker pull "\$IMAGE_REF"/)
  assert.match(publish, /run: >-\n\s+"\$RUNNER_TEMP\/trusted-publication-verifier\/scripts\/smoke-cli-image\.sh"\n\s+"\$IMAGE_REF"\n\s+"\$RELEASE_VERSION"/)
  assert.doesNotMatch(publish, /run: scripts\/smoke-cli-image\.sh "\$IMAGE_REF"/)
  assert.match(publish, /publication-evidence\.json/)
  for (const field of ['sourceRunId', 'sourceCommit', 'archiveSha256', 'manifestDigest', 'tag', 'immutableImageReference', 'postPublicationDigest', 'npmPackages', '@jfet07-polygon-labs/polygon-nesting', '@jfet97/polygon-nesting', 'https://npm.pkg.github.com', 'https://registry.npmjs.org', 'actor', 'repository', 'workflowRunId', 'timestamp', 'smoke']) {
    assert.match(publish, new RegExp(field), `publication evidence must include ${field}`)
  }
  assert.doesNotMatch(publish, /docker build(?:x)? build|cargo build|npm run build/, 'runtime publication must not rebuild source')
})

test('runtime publication rejects missing provenance and mutable image tags', () => {
  assert.notEqual(runtimePublication, '', 'runtime publication workflow must exist')
  const publish = workflowJob(runtimePublication, 'publish')
  assert.match(publish, /different digest|different existing digest|must not move|refuse/i)
  assert.match(publish, /post.*digest|POST_PUBLICATION_DIGEST|postPublicationDigest/i)
  assert.doesNotMatch(publish, /--tag .*:latest/, 'runtime publication must not use a mutable latest tag')
})

test('workflow contracts reject mutable triggers, target roots, and image references', () => {
  const dockerfile = readFileSync(join(REPOSITORY_ROOT, 'Dockerfile'), 'utf8')
  const runtimePublish = workflowJob(runtimePublication, 'publish')
  assert.throws(
    () => assertCiTriggerContract(ci.replace('  pull_request:\n', '  pull_request:\n    branches:\n      - main\n')),
    /trigger/
  )
  assert.throws(
    () => assertCiTriggerContract(ci.replace('      - main\n  pull_request:', '      - main\n      - development\n  pull_request:')),
    /trigger/
  )
  assert.throws(
    () => assertReleaseTriggerContract(release.replace('  push:\n', '  pull_request:\n  push:\n')),
    /release triggers/
  )
  assert.throws(
    () => assertReleaseTriggerContract(release.replace("      - 'v*'", "      - 'v*'\n      - latest")),
    /release triggers/
  )
  assert.throws(
    () => assertRuntimeSourceValidation(runtimePublish.replace("run.path !== '.github/workflows/release.yml'", "run.path !== '.github/workflows/other.yml'")),
    /release\\.yml/
  )
  assert.throws(
    () => assertRuntimePublicationSecurityContract(`${runtimePublication}\n  bypass:\n    runs-on: ubuntu-latest\n    steps:\n      - run: docker push ghcr.io/example/bypass\n`),
    /only the publish job/
  )
  assert.throws(
    () => assertRuntimePublicationSecurityContract(runtimePublication.replace(
      "if (comparison.base_commit?.sha !== sourceCommit || (comparison.status !== 'ahead' && comparison.status !== 'identical')) {",
      "if (false && (comparison.base_commit?.sha !== sourceCommit || (comparison.status !== 'ahead' && comparison.status !== 'identical'))) {"
    )),
    /ancestry rejection condition/
  )
  assert.throws(
    () => assertStandaloneParityTriggerContract(parity.replace('  workflow_dispatch:\n', '  pull_request:\n  workflow_dispatch:\n')),
    /standalone parity triggers/
  )
  assert.throws(
    () => assertStandaloneParityTriggerContract(parity.replace('  workflow_call:\n', '  push:\n  workflow_call:\n')),
    /standalone parity triggers/
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
    () => assertCiRegistryDigest(ci.replace('registry:2.8.3@sha256:a3d8aaa63ed8681a604f1dea0aa03f100d5895b6a58ace528858a7b332415373', 'registry:2.8.3')),
    /registry digest/
  )
  assert.throws(
    () => assertExactElectronRuntime(ci.replaceAll('electron@39.2.7', 'electron@latest')),
    /Electron runtime/
  )
  assert.throws(
    () => assertCiRunnerContract(ci.replace('blacksmith-8vcpu-ubuntu-2404', 'ubuntu-latest')),
    /runs-on|deepEqual/
  )
  assert.throws(
    () => assertCiRunnerContract(ci.replace(
      '    runs-on: ${{ matrix.runner }}',
      "          - platform: linux\n            target-key: linux-duplicate\n            runner: blacksmith-2vcpu-ubuntu-2404\n            arch: x64\n            cargo-target: x86_64-unknown-linux-gnu\n    runs-on: ${{ matrix.runner }}"
    )),
    /deep-equal/
  )
  assert.throws(
    () => assertReleaseRunnerContract(release.replace('runs-on: blacksmith-2vcpu-ubuntu-2404', 'runs-on: ubuntu-latest')),
    /runs-on/
  )
  assert.throws(
    () => assertPrivateCiImagesPinned('container: ghcr.io/jfet97/polygon-nesting-ci:latest'),
    /GHCR CI image/
  )
})
