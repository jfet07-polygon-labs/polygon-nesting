import assert from 'node:assert/strict'
import { existsSync, readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

const REPOSITORY_ROOT = dirname(dirname(dirname(fileURLToPath(import.meta.url))))
const DOCKERFILE_PATH = join(REPOSITORY_ROOT, '.github', 'ci', 'Dockerfile')
const WORKFLOW_PATH = join(REPOSITORY_ROOT, '.github', 'workflows', 'ci-image.yml')
const QUALITY_WORKFLOW_PATH = join(REPOSITORY_ROOT, '.github', 'workflows', 'ci.yml')

const RUST_BASE_IMAGE = 'rust:1.95.0-bookworm@sha256:6258907abe69656e41cd992e0b705cdcfabcbbe3db374f92ed2d47121282d4a1'
const CI_IMAGE = 'ghcr.io/jfet07-polygon-labs/polygon-nesting-ci'
const CI_IMAGE_TAG = 'ci-v1.0.0'
const NODE_22 = {
  version: '22.22.0',
  archive: 'node-v22.22.0-linux-x64.tar.xz',
  checksum: '9aa8e9d2298ab68c600bd6fb86a6c13bce11a4eca1ba9b39d79fa021755d7c37'
}
const NODE_24 = {
  version: '24.19.0',
  archive: 'node-v24.19.0-linux-x64.tar.xz',
  checksum: '14b342e71204f811bde6153be8e04b62aef63c236fef92b55f9c83154b409647'
}
const SCCACHE = {
  version: '0.10.0',
  archive: 'sccache-v0.10.0-x86_64-unknown-linux-musl.tar.gz',
  checksum: '1fbb35e135660d04a2d5e42b59c7874d39b3deb17de56330b25b713ec59f849b'
}
const GITHUB_CLI = {
  version: '2.97.0',
  archive: 'gh_2.97.0_linux_amd64.tar.gz',
  checksum: 'a2c9b8497e1f85b1ad0dfcb78b5a622e098801b8e461e459e88e1ee12f018112'
}

function loadRequiredText(path, description) {
  assert.ok(existsSync(path), `${description} must exist`)
  return readFileSync(path, 'utf8')
}

function remoteActionReferences(workflow) {
  return [...workflow.matchAll(/^\s*-?\s*uses:\s+([^\s]+)\s*$/gm)]
    .map(([, reference]) => reference)
    .filter((reference) => !reference.startsWith('./'))
}

function assertRemoteActionsPinned(workflow) {
  for (const reference of remoteActionReferences(workflow)) {
    assert.match(reference, /^[^@\s]+@[a-f0-9]{40}$/i, `remote action must use a full commit SHA: ${reference}`)
  }
}

function assertCheckedDownload(dockerfile, { archive, checksum, version }) {
  assert.match(dockerfile, new RegExp(`v?${version.replaceAll('.', '\\.')}`), `download must pin version ${version}`)
  assert.match(dockerfile, new RegExp(`https://[^\\s]+/${archive.replaceAll('.', '\\.')}`), `download URL must name ${archive}`)
  assert.match(
    dockerfile,
    new RegExp(`printf '%s\\\\n' '${checksum}  /tmp/${archive.replaceAll('.', '\\.')}' \\| sha256sum --check --strict`),
    `${archive} must be verified by its exact SHA-256 checksum`
  )
}

function assertDockerfileContract(dockerfile) {
  const images = [...dockerfile.matchAll(/^FROM\s+([^\s]+)$/gm)].map(([, image]) => image)
  assert.deepEqual(images, [RUST_BASE_IMAGE], 'CI image must use the reviewed immutable Rust base')
  for (const image of images) assert.match(image, /^[^@\s]+@sha256:[a-f0-9]{64}$/i, `Docker base image requires a digest: ${image}`)
  assert.match(dockerfile, /^ARG TARGETPLATFORM$/m)
  assert.match(dockerfile, /RUN test "\$TARGETPLATFORM" = "linux\/amd64"/)
  assert.match(dockerfile, /org\.opencontainers\.image\.source="https:\/\/github\.com\/jfet07-polygon-labs\/polygon-nesting"/)
  assert.match(dockerfile, /org\.opencontainers\.image\.version="ci-v1\.0\.0"/)
  assert.match(dockerfile, /org\.opencontainers\.image\.title="polygon-nesting-ci"/)
  assert.doesNotMatch(dockerfile, /jfet97/)

  for (const artifact of [NODE_22, NODE_24, SCCACHE, GITHUB_CLI]) assertCheckedDownload(dockerfile, artifact)
  assert.match(dockerfile, /rustup component add --toolchain 1\.95\.0-x86_64-unknown-linux-gnu rustfmt clippy/)
  assert.ok(dockerfile.includes("rustfmt --version | grep -Eq '^rustfmt 1\\.9\\.0-stable '"), 'rustfmt must be the reviewed Rust 1.95 component release')
  assert.match(dockerfile, /rustup target add --toolchain 1\.95\.0-x86_64-unknown-linux-gnu x86_64-unknown-linux-gnu/)
  assert.match(dockerfile, /ENV NODE_22_HOME=\/opt\/node-v22\.22\.0/)
  assert.match(dockerfile, /NODE_24_HOME=\/opt\/node-v24\.19\.0/)
  assert.match(dockerfile, /PATH="\/opt\/node-v22\.22\.0\/bin:/)
  assert.match(dockerfile, /\/etc\/profile\.d\/ci-image-path\.sh/, 'Bash login shells must preserve the CI tool path')
  assert.match(dockerfile, /install -m 0755 \/tmp\/sccache-v0\.10\.0-x86_64-unknown-linux-musl\/sccache \/usr\/local\/bin\/sccache/)
  assert.match(dockerfile, /install -m 0755 \/tmp\/gh_2\.97\.0_linux_amd64\/bin\/gh \/usr\/local\/bin\/gh/)

  for (const packageName of ['bash', 'ca-certificates', 'coreutils', 'git', 'gzip', 'python3', 'tar', 'build-essential', 'binutils', 'libc6-dev', 'libssl-dev', 'pkg-config']) {
    assert.match(dockerfile, new RegExp(`\\b${packageName.replace('-', '\\-')}\\b`), `CI image must install ${packageName}`)
  }
  assert.match(dockerfile, /^USER root$/m)
  assert.match(dockerfile, /^CMD \["sleep", "infinity"\]$/m)
  assert.doesNotMatch(dockerfile, /^ENTRYPOINT\s/m)

  assert.doesNotMatch(dockerfile, /^(?:COPY|ADD)\s/im, 'CI image must not copy build context material')
  assert.doesNotMatch(dockerfile, /cargo (?:build|fetch|install)\b/, 'CI image must not bake application Cargo outputs')
  assert.doesNotMatch(dockerfile, /\b(?:CARGO_TARGET_DIR|SCCACHE_GHA_ENABLED|ACTIONS_[A-Z_]+|GITHUB_[A-Z_]+|AZP_[A-Z_]+)\b/, 'CI image must not bake CI cache configuration')
  assert.doesNotMatch(dockerfile, /\b(?:TOKEN|PASSWORD|SECRET|CREDENTIAL)\b/i, 'CI image must not bake credentials')
}

function assertWorkflowTriggers(workflow) {
  const [, triggerBlock] = workflow.match(/^on:\n([\s\S]*?)^permissions:/m) ?? []
  assert.equal(triggerBlock, '  workflow_dispatch:\n\n', 'CI image publisher must be manual-only')
}

function workflowJob(workflow, name) {
  const [, job] = workflow.match(new RegExp(
    `^  ${name}:\\n([\\s\\S]*?)(?=^  [\\w-]+:\\n|(?![\\s\\S]))`,
    'm'
  )) ?? []
  assert.notEqual(job, undefined, `workflow must define the ${name} job`)
  return job
}

function assertWorkflowContract(workflow) {
  assertWorkflowTriggers(workflow)
  const [, permissionBlock] = workflow.match(/^permissions:\n([\s\S]*?)^concurrency:/m) ?? []
  assert.equal(permissionBlock, '  contents: read\n  packages: write\n\n', 'publisher permissions must be exact')
  assert.match(workflow, /^concurrency:\n  group: ci-image-publish\n  cancel-in-progress: false$/m)
  assertRemoteActionsPinned(workflow)
  assert.deepEqual(remoteActionReferences(workflow), [
    'actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683',
    'docker/setup-buildx-action@e468171a9de216ec08956ac3ada2f0791b6bd435'
  ], 'publisher must use only reviewed SHA-pinned actions')

  assert.match(workflow, new RegExp(`CI_IMAGE: ${CI_IMAGE}`))
  assert.match(workflow, new RegExp(`CI_IMAGE_TAG: ${CI_IMAGE_TAG}`))
  assert.match(workflow, /printf '%s' "\$GITHUB_TOKEN" \| docker login ghcr\.io --username "\$GITHUB_ACTOR" --password-stdin/)
  assert.match(workflow, /gh api --paginate --slurp "\/orgs\/jfet07-polygon-labs\/packages\/container\/polygon-nesting-ci\/versions\?per_page=100"/)
  assert.match(workflow, /CI image tag ci-v1\.0\.0 already exists and will not be retagged/)
  assert.match(workflow, /--platform linux\/amd64/)
  assert.match(workflow, /--cache-from type=gha,scope=ci-image-linux-amd64/)
  assert.match(workflow, /--cache-to type=gha,mode=max,scope=ci-image-linux-amd64/)
  assert.match(workflow, /--metadata-file build-metadata\.json/)
  assert.match(workflow, /\["containerimage\.digest"\]/)
  assert.match(workflow, /printf 'digest=%s\\n' "\$manifest_digest" >> "\$GITHUB_OUTPUT"/)
  assert.match(workflow, /steps\.build\.outputs\.digest/)
  assert.match(workflow, /Published CI image manifest digest:/)

  const publish = workflowJob(workflow, 'publish')
  assert.match(publish, /^    if: github\.ref == 'refs\/heads\/main'$/m, 'publisher must reject manually selected non-main refs')
  assert.match(publish, /^    runs-on: blacksmith-2vcpu-ubuntu-2404$/m)
  assert.doesNotMatch(publish, /^\s+container:/m, 'publisher must run directly on the hosted Ubuntu runner')
  assert.doesNotMatch(workflow, /^\s+container:/m, 'workflow must not use a job container to publish its own image')
  assert.match(publish, /docker buildx build/)
  assert.match(publish, /\.github\/ci$/m, 'Docker build context must contain only CI image inputs')

  const smoke = workflowJob(workflow, 'smoke-pushed-digest')
  assert.match(smoke, /^    needs: publish$/m)
  assert.match(smoke, /^    runs-on: blacksmith-2vcpu-ubuntu-2404$/m)
  assert.match(smoke, /MANIFEST_DIGEST: \$\{\{ needs\.publish\.outputs\.digest \}\}/)
  assert.match(smoke, /IMAGE_REF="\$\{CI_IMAGE\}@\$\{MANIFEST_DIGEST\}"/)
  assert.match(smoke, /docker pull "\$IMAGE_REF"/)
  assert.match(smoke, /docker image inspect "\$IMAGE_REF" --format '\{\{json \.Config\.Entrypoint\}\}' \| grep -Fx null/)
  assert.match(smoke, /docker run --rm "\$IMAGE_REF" bash -lc/)
  assert.doesNotMatch(smoke, /:\$\{CI_IMAGE_TAG\}/, 'smoke must use the pushed manifest digest rather than a tag')

  for (const command of [
    'test "$(id -u)" = "0"',
    'test "$(uname -m)" = "x86_64"',
    'rustc --version',
    'cargo --version',
    'rustfmt --version',
    'cargo clippy --version',
    'rustup target list --installed',
    'test "$(node --version)" = "v22.22.0"',
    'test "$(/opt/node-v24.19.0/bin/node --version)" = "v24.19.0"',
    'sccache --version',
    'gh attestation --help',
    'git --version',
    'python3 --version',
    'bash --version',
    'groupadd --help',
    'useradd --help',
    'sha256sum --version',
    'tar --version',
    'gzip --version',
    'cc --version',
    'c++ --version',
    'ld --version',
    'pkg-config --version',
    '/workspace',
    '/usr/local/bin/polygon-nesting',
    '/release-candidate',
    '/parity-input',
    '/native-artifacts'
  ]) assert.ok(smoke.includes(command), `digest smoke must verify ${command}`)
}

test('portable CI image Dockerfile has pinned tools and excludes application material', () => {
  assertDockerfileContract(loadRequiredText(DOCKERFILE_PATH, '.github/ci/Dockerfile'))
})

test('CI image publisher has exact authority, immutable publication, and digest smoke contracts', () => {
  assertWorkflowContract(loadRequiredText(WORKFLOW_PATH, '.github/workflows/ci-image.yml'))
})

test('quality CI remains independent from the unpublished CI image', () => {
  const qualityWorkflow = loadRequiredText(QUALITY_WORKFLOW_PATH, '.github/workflows/ci.yml')
  assert.doesNotMatch(qualityWorkflow, /polygon-nesting-ci/)
  assert.doesNotMatch(qualityWorkflow, /container:\s*ghcr\.io\/jfet07-polygon-labs\/polygon-nesting-ci/)
})

test('CI image contracts reject mutable images, downloads, and unsafe image contents', () => {
  const dockerfile = loadRequiredText(DOCKERFILE_PATH, '.github/ci/Dockerfile')
  assert.throws(
    () => assertDockerfileContract(dockerfile.replace(/@sha256:[a-f0-9]{64}/, '')),
    /immutable Rust base|digest/
  )
  assert.throws(
    () => assertDockerfileContract(dockerfile.replace(NODE_22.checksum, '0'.repeat(64))),
    /SHA-256/
  )
  assert.throws(
    () => assertDockerfileContract(dockerfile.replace('sha256sum --check --strict', 'sha256sum')),
    /SHA-256/
  )
  assert.throws(
    () => assertDockerfileContract(dockerfile.replace('CMD ["sleep", "infinity"]', 'ENTRYPOINT ["sleep", "infinity"]')),
    /ENTRYPOINT|CMD/
  )
  assert.throws(
    () => assertDockerfileContract(dockerfile.replace('USER root', 'USER polygon')),
    /USER root/
  )
  assert.throws(
    () => assertDockerfileContract(`${dockerfile}\nCOPY . /workspace\n`),
    /copy build context/
  )
  assert.throws(
    () => assertDockerfileContract(`${dockerfile}\nENV SCCACHE_GHA_ENABLED=true\n`),
    /cache configuration/
  )
})

test('CI image workflow contracts reject widened authority and tag-based smoke tests', () => {
  const workflow = loadRequiredText(WORKFLOW_PATH, '.github/workflows/ci-image.yml')
  assert.throws(
    () => assertWorkflowContract(workflow.replace('  workflow_dispatch:\n', '  workflow_dispatch:\n  push:\n    branches:\n      - main\n')),
    /manual-only/
  )
  assert.throws(
    () => assertWorkflowContract(workflow.replace('  packages: write', '  packages: write\n  id-token: write')),
    /permissions/
  )
  assert.throws(
    () => assertWorkflowContract(workflow.replace("    if: github.ref == 'refs/heads/main'\n", '')),
    /main ref/
  )
  assert.throws(
    () => assertWorkflowContract(workflow.replace("github.ref == 'refs/heads/main'", "github.ref != 'refs/heads/main'")),
    /main ref/
  )
  assert.throws(
    () => assertWorkflowContract(workflow.replace('runs-on: blacksmith-2vcpu-ubuntu-2404', 'runs-on: ubuntu-latest')),
    /runs-on/
  )
  assert.throws(
    () => assertWorkflowContract(workflow.replace('actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683', 'actions/checkout@v5')),
    /full commit SHA/
  )
  assert.throws(
    () => assertWorkflowContract(workflow.replace('--cache-to type=gha,mode=max,scope=ci-image-linux-amd64', '--cache-to type=local,dest=/tmp/cache')),
    /cache-to/
  )
  assert.throws(
    () => assertWorkflowContract(workflow.replace('IMAGE_REF="${CI_IMAGE}@${MANIFEST_DIGEST}"', 'IMAGE_REF="${CI_IMAGE}:${CI_IMAGE_TAG}"')),
    /IMAGE_REF|tag/
  )
  assert.throws(
    () => assertWorkflowContract(workflow.replace(CI_IMAGE, 'ghcr.io/jfet97/polygon-nesting-ci')),
    /CI_IMAGE|source/
  )
})
