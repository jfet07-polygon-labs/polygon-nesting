import assert from 'node:assert/strict'
import { existsSync, readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

const REPOSITORY_ROOT = dirname(dirname(dirname(fileURLToPath(import.meta.url))))
const DOCKERFILE_PATH = join(REPOSITORY_ROOT, 'Dockerfile')
const LEGACY_DOCKERFILE_PATH = join(REPOSITORY_ROOT, '.github', 'ci', 'Dockerfile')
const WORKFLOW_PATH = join(REPOSITORY_ROOT, '.github', 'workflows', 'ci-image.yml')
const QUALITY_WORKFLOW_PATH = join(REPOSITORY_ROOT, '.github', 'workflows', 'ci.yml')
const PARITY_WORKFLOW_PATH = join(REPOSITORY_ROOT, '.github', 'workflows', 'standalone-parity.yml')

const RUST_BASE_IMAGE = 'rust:1.95.0-bookworm@sha256:6258907abe69656e41cd992e0b705cdcfabcbbe3db374f92ed2d47121282d4a1'
const DEBIAN_BASE_IMAGE = 'debian:bookworm-slim@sha256:7b140f374b289a7c2befc338f42ebe6441b7ea838a042bbd5acbfca6ec875818'
const CI_IMAGE = 'ghcr.io/jfet07-polygon-labs/polygon-nesting-ci'
const CI_IMAGE_TAG = 'ci-v1.0.0'
const CI_IMAGE_REFERENCE = `${CI_IMAGE}@sha256:66a7ca95c13074714135ad840465834e60797ddd45d13d130e0e5c6077b950f3`
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

function dockerStages(dockerfile) {
  const matches = [...dockerfile.matchAll(/^FROM\s+([^\s]+)(?:\s+AS\s+(\w+))?\s*$/gm)]
  return matches.map((match, index) => {
    const bodyStart = match.index + match[0].length
    const bodyEnd = matches[index + 1]?.index ?? dockerfile.length
    return { image: match[1], name: match[2], body: dockerfile.slice(bodyStart, bodyEnd) }
  })
}

function stage(dockerfile, name) {
  const selected = dockerStages(dockerfile).find((candidate) => candidate.name === name)
  assert.ok(selected, `Dockerfile must define a ${name} stage`)
  return selected.body
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
  const stages = dockerStages(dockerfile)
  const externalImages = stages.filter(({ image }) => image.includes('@sha256:')).map(({ image }) => image)
  assert.deepEqual(externalImages, [RUST_BASE_IMAGE, DEBIAN_BASE_IMAGE], 'all external stages must use reviewed immutable bases')
  for (const image of externalImages) assert.match(image, /^[^@\s]+@sha256:[a-f0-9]{64}$/i, `Docker base image requires a digest: ${image}`)
  assert.deepEqual(stages.map(({ name }) => name), ['base', 'ci', 'builder', 'runtime'])
  assert.match(dockerfile, /^ARG TARGETPLATFORM$/m)
  assert.match(stage(dockerfile, 'base'), /RUN test "\$TARGETPLATFORM" = "linux\/amd64"/)

  const ci = stage(dockerfile, 'ci')
  assert.match(ci, /org\.opencontainers\.image\.source="https:\/\/github\.com\/jfet07-polygon-labs\/polygon-nesting"/)
  assert.match(ci, /org\.opencontainers\.image\.version="ci-v1\.0\.0"/)
  assert.match(ci, /org\.opencontainers\.image\.title="polygon-nesting-ci"/)
  assert.doesNotMatch(ci, /jfet97/)
  for (const artifact of [NODE_22, NODE_24, SCCACHE, GITHUB_CLI]) assertCheckedDownload(ci, artifact)
  assert.match(ci, /rustup component add --toolchain 1\.95\.0-x86_64-unknown-linux-gnu rustfmt clippy/)
  assert.ok(ci.includes("rustfmt --version | grep -Eq '^rustfmt 1\\.9\\.0-stable '"), 'rustfmt must be the reviewed Rust 1.95 component release')
  assert.match(ci, /rustup target add --toolchain 1\.95\.0-x86_64-unknown-linux-gnu x86_64-unknown-linux-gnu/)
  assert.match(ci, /ENV NODE_22_HOME=\/opt\/node-v22\.22\.0/)
  assert.match(ci, /NODE_24_HOME=\/opt\/node-v24\.19\.0/)
  assert.match(ci, /PATH="\/opt\/node-v22\.22\.0\/bin:/)
  assert.match(ci, /\/etc\/profile\.d\/ci-image-path\.sh/, 'Bash login shells must preserve the CI tool path')
  assert.match(ci, /install -m 0755 \/tmp\/sccache-v0\.10\.0-x86_64-unknown-linux-musl\/sccache \/usr\/local\/bin\/sccache/)
  assert.match(ci, /install -m 0755 \/tmp\/gh_2\.97\.0_linux_amd64\/bin\/gh \/usr\/local\/bin\/gh/)
  for (const packageName of ['bash', 'ca-certificates', 'coreutils', 'git', 'gzip', 'python3', 'tar', 'build-essential', 'binutils', 'libc6-dev', 'libssl-dev', 'pkg-config']) {
    assert.match(ci, new RegExp(`\\b${packageName.replace('-', '\\-')}\\b`), `CI image must install ${packageName}`)
  }
  assert.match(ci, /^USER root$/m)
  assert.match(ci, /^CMD \["sleep", "infinity"\]$/m)
  assert.doesNotMatch(ci, /^ENTRYPOINT\s/m)
  assert.doesNotMatch(ci, /^(?:COPY|ADD)\s/im, 'CI image must not copy build context material')
  assert.doesNotMatch(ci, /cargo (?:build|fetch|install)\b/, 'CI image must not bake application Cargo outputs')
  assert.doesNotMatch(ci, /\b(?:CARGO_TARGET_DIR|SCCACHE_GHA_ENABLED|ACTIONS_[A-Z_]+|GITHUB_[A-Z_]+|AZP_[A-Z_]+)\b/, 'CI image must not bake CI cache configuration')
  assert.doesNotMatch(ci, /\b(?:TOKEN|PASSWORD|SECRET|CREDENTIAL)\b/i, 'CI image must not bake credentials')

  const builder = stage(dockerfile, 'builder')
  assert.match(builder, /WORKDIR \/workspace/)
  assert.match(builder, /COPY Cargo\.toml Cargo\.lock \.\//)
  assert.match(builder, /COPY crates \.\/crates/)
  assert.match(builder, /RUN cargo build --release --locked -p polygon-nesting-cli/)

  const runtime = stage(dockerfile, 'runtime')
  assert.match(runtime, /ARG ENGINE_VERSION/)
  assert.match(runtime, /ARG SOURCE_COMMIT/)
  assert.match(runtime, /test "\$ENGINE_VERSION" = "0\.1\.2"/)
  assert.match(runtime, /test "\$SOURCE_COMMIT" != "unknown"/)
  assert.match(runtime, /org\.opencontainers\.image\.source="https:\/\/github\.com\/jfet07-polygon-labs\/polygon-nesting"/)
  assert.match(runtime, /org\.opencontainers\.image\.licenses="NOASSERTION"/)
  assert.match(runtime, /USER polygon/)
  assert.match(runtime, /ENTRYPOINT \["\/usr\/local\/bin\/polygon-nesting"\]/)
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
  assert.match(workflow, /action=reuse/)
  assert.match(workflow, /docker buildx imagetools inspect "\$CI_IMAGE:\$CI_IMAGE_TAG"/)
  assert.match(workflow, /if: steps\.state\.outputs\.action == 'publish'/)
  assert.match(workflow, /--platform linux\/amd64/)
  assert.match(workflow, /--cache-from type=gha,scope=ci-image-linux-amd64/)
  assert.match(workflow, /--cache-to type=gha,mode=max,scope=ci-image-linux-amd64/)
  assert.match(workflow, /--metadata-file build-metadata\.json/)
  assert.match(workflow, /\["containerimage\.digest"\]/)
  assert.match(workflow, /printf 'digest=%s\\n' "\$manifest_digest" >> "\$GITHUB_OUTPUT"/)
  assert.match(workflow, /steps\.result\.outputs\.digest/)
  assert.match(workflow, /BUILT_DIGEST: \$\{\{ steps\.build\.outputs\.digest \}\}/)
  assert.match(workflow, /EXISTING_DIGEST: \$\{\{ steps\.state\.outputs\.existing_digest \}\}/)
  assert.match(workflow, /CI image manifest digest:/)
  assert.match(workflow, /--target ci/)
  assert.match(workflow, /--metadata-file build-metadata\.json[\s\S]*\n\s*\./)
  assert.doesNotMatch(workflow, /\.github\/ci(?:\s|$)/m, 'CI image must build from repository root')

  const publish = workflowJob(workflow, 'publish')
  assert.match(publish, /^    if: github\.ref == 'refs\/heads\/main'$/m, 'publisher must reject manually selected non-main refs')
  assert.match(publish, /^    runs-on: blacksmith-2vcpu-ubuntu-2404$/m)
  assert.doesNotMatch(publish, /^\s+container:/m, 'publisher must run directly on the hosted Ubuntu runner')
  assert.doesNotMatch(workflow, /^\s+container:/m, 'workflow must not use a job container to publish its own image')
  assert.match(publish, /docker buildx build/)
  assert.match(publish, /--target ci/)
  assert.match(publish, /^\s+\.$/m, 'Docker build context must be the repository root')

  const smoke = workflowJob(workflow, 'smoke-pushed-digest')
  assert.match(smoke, /^    needs: publish$/m)
  assert.match(smoke, /^    runs-on: blacksmith-2vcpu-ubuntu-2404$/m)
  assert.match(smoke, /MANIFEST_DIGEST: \$\{\{ needs\.publish\.outputs\.digest \}\}/)
  assert.match(smoke, /docker login ghcr\.io --username "\$GITHUB_ACTOR" --password-stdin/)
  assert.match(smoke, /IMAGE_REF="\$\{CI_IMAGE\}@\$\{MANIFEST_DIGEST\}"/)
  assert.match(smoke, /docker pull "\$IMAGE_REF"/)
  assert.match(smoke, /docker image inspect "\$IMAGE_REF" --format '\{\{json \.Config\.Entrypoint\}\}' \| grep -Fx null/)
  assert.match(smoke, /docker run --rm "\$IMAGE_REF" bash -lc/)
  assert.doesNotMatch(smoke, /:\$\{CI_IMAGE_TAG\}/, 'smoke must use the pushed manifest digest rather than a tag')
}

function assertCiImageContainers(workflow) {
  const references = [...workflow.matchAll(/^\s+container:\s+(ghcr\.io\/[^\s]+@sha256:[a-f0-9]{64})\s*$/gm)].map(([, reference]) => reference)
  assert.ok(references.length > 0, 'Linux CI jobs must use the immutable CI image')
  assert.deepEqual([...new Set(references)], [CI_IMAGE_REFERENCE], 'all CI containers must use one literal immutable digest')
}

test('consolidated Dockerfile defines reviewed CI and runtime targets', () => {
  assert.equal(existsSync(LEGACY_DOCKERFILE_PATH), false, 'the legacy CI Dockerfile must be absent')
  assertDockerfileContract(loadRequiredText(DOCKERFILE_PATH, 'root Dockerfile'))
})

test('CI image publisher builds the consolidated root ci target', () => {
  assertWorkflowContract(loadRequiredText(WORKFLOW_PATH, '.github/workflows/ci-image.yml'))
})

test('Linux-only CI jobs consume the immutable image while native and Docker jobs stay native', () => {
  const qualityWorkflow = loadRequiredText(QUALITY_WORKFLOW_PATH, '.github/workflows/ci.yml')
  const parityWorkflow = loadRequiredText(PARITY_WORKFLOW_PATH, '.github/workflows/standalone-parity.yml')
  assertCiImageContainers(qualityWorkflow)
  assertCiImageContainers(parityWorkflow)
  assert.doesNotMatch(workflowJob(qualityWorkflow, 'oci-evidence'), /^\s+container:/m)
  assert.doesNotMatch(workflowJob(qualityWorkflow, 'native'), /^\s+container:/m)
  assert.doesNotMatch(workflowJob(parityWorkflow, 'parity'), /^\s+container:/m)
})

test('CI image contracts reject mutable images, downloads, and unsafe image contents', () => {
  const dockerfile = loadRequiredText(DOCKERFILE_PATH, 'root Dockerfile')
  assert.throws(
    () => assertDockerfileContract(dockerfile.replace(/@sha256:[a-f0-9]{64}/, '')),
    /immutable Rust base|digest|external stages/
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
    () => assertDockerfileContract(`${dockerfile.replace('FROM base AS ci', 'FROM base AS ci\nCOPY . /workspace')}`),
    /copy build context/
  )
  assert.throws(
    () => assertDockerfileContract(`${dockerfile.replace('FROM base AS ci', 'FROM base AS ci\nENV SCCACHE_GHA_ENABLED=true')}`),
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
