import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { cpSync, mkdtempSync, mkdirSync, readFileSync, rmSync, symlinkSync, unlinkSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import test from 'node:test'
import { fileURLToPath, pathToFileURL } from 'node:url'

import { createTask112ParityFixture } from '../tests/parity/helpers/task112-parity-fixture.mjs'

const REPOSITORY_ROOT = dirname(dirname(fileURLToPath(import.meta.url)))
const TARGETS = [
  ['linux-x64', 'linux', 'x64', 'x86_64-unknown-linux-gnu'],
  ['win32-x64', 'win32', 'x64', 'x86_64-pc-windows-msvc'],
  ['darwin-arm64', 'darwin', 'arm64', 'aarch64-apple-darwin'],
  ['darwin-x64', 'darwin', 'x64', 'x86_64-apple-darwin']
]

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex')
}

function writeJson(path, value) {
  mkdirSync(dirname(path), { recursive: true })
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`)
}

async function loadModule(name) {
  return import(pathToFileURL(resolve(REPOSITORY_ROOT, 'scripts', name)).href)
}

function makePackageFixture(root) {
  const packageRoot = join(root, 'package')
  mkdirSync(join(packageRoot, 'npm'), { recursive: true })
  mkdirSync(join(packageRoot, 'LICENSES'), { recursive: true })
  for (const relative of ['npm/index.cjs', 'npm/target.cjs', 'NOTICE', 'LICENSES/clipper2-ts-BSL-1.0.txt']) {
    cpSync(join(REPOSITORY_ROOT, 'packages/polygon-nesting', relative), join(packageRoot, relative))
  }
  writeJson(join(packageRoot, 'package.json'), {
    name: '@jfet07-polygon-labs/polygon-nesting',
    version: '0.1.0',
    main: 'npm/index.cjs',
    files: ['npm/index.cjs', 'npm/target.cjs', 'npm/*.node', 'NOTICE', 'LICENSES/**'],
    scripts: { test: 'node package-check.mjs' }
  })
  writeFileSync(join(packageRoot, 'package-check.mjs'), [
    "import assert from 'node:assert/strict'",
    "import { existsSync } from 'node:fs'",
    `for (const name of ${JSON.stringify(TARGETS.map(([key]) => `npm/irregular-nesting-native.${key}.node`))}) assert.equal(existsSync(name), true)`
  ].join('\n'))
  return packageRoot
}

async function makeFixture(t) {
  const parity = await createTask112ParityFixture()
  const root = mkdtempSync(join(tmpdir(), 'polygon-release-ci-'))
  const artifactsRoot = join(root, 'artifacts')
  const packageRoot = makePackageFixture(root)
  const outputDirectory = join(root, 'candidate')
  const { stageParityAggregateArchive } = await loadModule('stage-parity-aggregate.mjs')
  for (const [targetKey, platform, arch, cargoTarget] of TARGETS) {
    const directory = join(artifactsRoot, targetKey)
    const addonName = `irregular-nesting-native.${targetKey}.node`
    const bytes = Buffer.from(`native-${targetKey}`)
    mkdirSync(directory, { recursive: true })
    writeFileSync(join(directory, addonName), bytes)
    writeFileSync(join(directory, `${addonName}.sha256`), `${sha256(bytes)}  ${addonName}\n`)
    writeJson(join(directory, 'target.json'), {
      schemaVersion: 2, targetKey, platform, arch, cargoTarget,
      rustc: 'rustc 1.95.0 fixture', cargo: 'cargo 1.95.0 fixture', profile: 'release', features: [],
      sourceRevision: parity.sourceRevision,
      nativeDependency: { cargoLockSha256: sha256(readFileSync(join(parity.trustedSourceRoot, 'Cargo.lock'))), napiManifestSha256: sha256(readFileSync(join(parity.trustedSourceRoot, 'crates/polygon-nesting-napi/Cargo.toml'))) }
    })
    stageParityAggregateArchive({
      archivePath: parity.archivePath,
      digestPath: parity.digestPath,
      artifactDirectory: directory,
      cargoTarget,
      sourceCommit: parity.sourceRevision,
      targetKey,
      trustedSourceRoot: parity.trustedSourceRoot
    })
  }
  t.after(() => { rmSync(root, { force: true, recursive: true }); parity.cleanup() })
  return { artifactsRoot, outputDirectory, packageRoot, parity, root }
}

function makeOciArchive(root, labels) {
  const layout = join(root, 'oci-layout')
  const config = Buffer.from(JSON.stringify({ architecture: 'amd64', os: 'linux', config: { User: 'polygon', Labels: labels } }))
  const configDigest = sha256(config)
  const manifest = Buffer.from(JSON.stringify({ config: { digest: `sha256:${configDigest}`, mediaType: 'application/vnd.oci.image.config.v1+json', size: config.length }, layers: [], mediaType: 'application/vnd.oci.image.manifest.v1+json', schemaVersion: 2 }))
  const manifestDigest = sha256(manifest)
  mkdirSync(join(layout, 'blobs', 'sha256'), { recursive: true })
  writeFileSync(join(layout, 'oci-layout'), '{"imageLayoutVersion":"1.0.0"}\n')
  writeJson(join(layout, 'index.json'), { manifests: [{ digest: `sha256:${manifestDigest}`, mediaType: 'application/vnd.oci.image.manifest.v1+json', platform: { architecture: 'amd64', os: 'linux' }, size: manifest.length }], schemaVersion: 2 })
  writeFileSync(join(layout, 'blobs', 'sha256', configDigest), config)
  writeFileSync(join(layout, 'blobs', 'sha256', manifestDigest), manifest)
  const archivePath = join(root, 'oci-image.tar')
  execFileSync('tar', ['-cf', archivePath, '-C', layout, '.'])
  return { archivePath, manifestDigest: `sha256:${manifestDigest}` }
}

test('stages all four Task112 targets then assembles and verifies an offline candidate', async (t) => {
  const fixture = await makeFixture(t)
  const { assembleReleaseCandidate } = await loadModule('assemble-release-candidate.mjs')
  const { verifyReleaseCandidate } = await loadModule('verify-release-candidate.mjs')
  const release = await assembleReleaseCandidate({ ...fixture, sourceCommit: fixture.parity.sourceRevision, trustedSourceRoot: fixture.parity.trustedSourceRoot })
  assert.equal(release.nativeArtifacts.length, 4)
  assert.equal(release.parityAggregate.sha256, sha256(readFileSync(fixture.parity.archivePath)))
  await verifyReleaseCandidate({ candidateDirectory: fixture.outputDirectory, trustedSourceRoot: fixture.parity.trustedSourceRoot })
})

test('candidate assembly rejects a source revision that drifts from the Task112 aggregate', async (t) => {
  const fixture = await makeFixture(t)
  const { assembleReleaseCandidate } = await loadModule('assemble-release-candidate.mjs')
  await assert.rejects(
    assembleReleaseCandidate({ ...fixture, sourceCommit: '0123456789abcdef0123456789abcdef01234567', trustedSourceRoot: fixture.parity.trustedSourceRoot }),
    /sourceRevision|candidate source revision|aggregate metadata/
  )
})

test('candidate assembly rejects a target bundle swapped between Rust triples', async (t) => {
  const fixture = await makeFixture(t)
  const left = join(fixture.artifactsRoot, 'linux-x64', 'parity-bundle')
  const right = join(fixture.artifactsRoot, 'darwin-x64', 'parity-bundle')
  rmSync(left, { recursive: true }); cpSync(right, left, { recursive: true })
  const { assembleReleaseCandidate } = await loadModule('assemble-release-candidate.mjs')
  await assert.rejects(assembleReleaseCandidate({ ...fixture, sourceCommit: fixture.parity.sourceRevision, trustedSourceRoot: fixture.parity.trustedSourceRoot }), /target|parity/i)
})

test('candidate assembly rejects a mutated aggregate archive even with an unchanged sidecar', async (t) => {
  const fixture = await makeFixture(t)
  const archive = join(fixture.artifactsRoot, 'linux-x64', 'old-new-parity-bundle.tar.gz')
  const bytes = readFileSync(archive)
  writeFileSync(archive, Buffer.concat([bytes, Buffer.from('mutation')]))
  const { assembleReleaseCandidate } = await loadModule('assemble-release-candidate.mjs')
  await assert.rejects(assembleReleaseCandidate({ ...fixture, sourceCommit: fixture.parity.sourceRevision, trustedSourceRoot: fixture.parity.trustedSourceRoot }), /SHA-256/)
})

test('offline verification rejects a mutated candidate manifest', async (t) => {
  const fixture = await makeFixture(t)
  const { assembleReleaseCandidate } = await loadModule('assemble-release-candidate.mjs')
  const { verifyReleaseCandidate } = await loadModule('verify-release-candidate.mjs')
  await assembleReleaseCandidate({ ...fixture, sourceCommit: fixture.parity.sourceRevision, trustedSourceRoot: fixture.parity.trustedSourceRoot })
  const manifest = join(fixture.outputDirectory, 'npm-pack-manifest.json')
  writeFileSync(manifest, `${readFileSync(manifest, 'utf8')} `)
  await assert.rejects(verifyReleaseCandidate({ candidateDirectory: fixture.outputDirectory, trustedSourceRoot: fixture.parity.trustedSourceRoot }), /pack manifest SHA-256/)
})

test('offline verification recomputes OCI archive evidence after Task112 parity verification', async (t) => {
  const fixture = await makeFixture(t)
  const { assembleReleaseCandidate } = await loadModule('assemble-release-candidate.mjs')
  const { verifyReleaseCandidate } = await loadModule('verify-release-candidate.mjs')
  const { verifyRuntimePublication } = await loadModule('verify-runtime-publication.mjs')
  const release = await assembleReleaseCandidate({ ...fixture, sourceCommit: fixture.parity.sourceRevision, trustedSourceRoot: fixture.parity.trustedSourceRoot })
  const labels = { 'org.opencontainers.image.title': 'polygon-nesting', 'org.opencontainers.image.licenses': 'NOASSERTION', 'org.opencontainers.image.source': 'https://github.com/jfet97/polygon-nesting', 'org.opencontainers.image.version': '0.1.0', 'org.opencontainers.image.revision': release.sourceCommit }
  const oci = makeOciArchive(fixture.root, labels)
  const evidencePath = join(fixture.root, 'oci-evidence.json')
  const manifestDigestPath = join(fixture.root, 'manifest-digest.txt')
  writeFileSync(manifestDigestPath, `${oci.manifestDigest}\n`)
  writeJson(evidencePath, {
    schemaVersion: 1, manifestDigest: oci.manifestDigest, archiveSha256: sha256(readFileSync(oci.archivePath)),
    immutableImageReference: `127.0.0.1:5000/polygon-nesting@${oci.manifestDigest}`, platform: 'linux/amd64', sourceCommit: release.sourceCommit, nonRootSmoke: true,
    labels, legalHashes: release.legalHashes
  })
  await verifyReleaseCandidate({ candidateDirectory: fixture.outputDirectory, trustedSourceRoot: fixture.parity.trustedSourceRoot, ociArchivePath: oci.archivePath, ociEvidencePath: evidencePath })
  assert.deepEqual(await verifyRuntimePublication({
    candidateDirectory: fixture.outputDirectory,
    trustedSourceRoot: fixture.parity.trustedSourceRoot,
    ociArchivePath: oci.archivePath,
    ociEvidencePath: evidencePath,
    manifestDigestPath,
    sourceCommit: release.sourceCommit,
    sourceRunId: '123'
  }), {
    archiveSha256: sha256(readFileSync(oci.archivePath)),
    manifestDigest: oci.manifestDigest,
    immutableImageReference: `127.0.0.1:5000/polygon-nesting@${oci.manifestDigest}`,
    sourceCommit: release.sourceCommit,
    sourceRunId: '123'
  })
  const evidenceLink = join(fixture.root, 'oci-evidence-link.json')
  symlinkSync(evidencePath, evidenceLink)
  await assert.rejects(verifyRuntimePublication({
    candidateDirectory: fixture.outputDirectory,
    trustedSourceRoot: fixture.parity.trustedSourceRoot,
    ociArchivePath: oci.archivePath,
    ociEvidencePath: evidenceLink,
    manifestDigestPath,
    sourceCommit: release.sourceCommit,
    sourceRunId: '123'
  }), /OCI evidence must be a regular file/)
  unlinkSync(evidenceLink)
  await assert.rejects(verifyRuntimePublication({
    candidateDirectory: fixture.outputDirectory,
    trustedSourceRoot: fixture.parity.trustedSourceRoot,
    ociArchivePath: oci.archivePath,
    ociEvidencePath: evidencePath,
    manifestDigestPath,
    sourceCommit: '0'.repeat(40),
    sourceRunId: '123'
  }), /release sourceCommit/)
  writeFileSync(oci.archivePath, Buffer.concat([readFileSync(oci.archivePath), Buffer.from('mutation')]))
  await assert.rejects(verifyRuntimePublication({
    candidateDirectory: fixture.outputDirectory,
    trustedSourceRoot: fixture.parity.trustedSourceRoot,
    ociArchivePath: oci.archivePath,
    ociEvidencePath: evidencePath,
    manifestDigestPath,
    sourceCommit: release.sourceCommit,
    sourceRunId: '123'
  }), /OCI archive SHA-256/)
})

test('normal PR quality checks run the complete parity and release contract suite', () => {
  const ci = readFileSync(join(REPOSITORY_ROOT, '.github/workflows/ci.yml'), 'utf8')
  assert.match(ci, /node --test tests\/ci\/\*\.test\.mjs tests\/parity\/\*\.test\.mjs scripts\/release-ci\.test\.mjs scripts\/publish-native-package\.test\.mjs/)
  assert.doesNotMatch(ci, /^\s*node --test scripts\/release-ci\.test\.mjs$/m)
})

test('CI and release workflows use the Task112 aggregate trust chain', async () => {
  const ci = readFileSync(join(REPOSITORY_ROOT, '.github/workflows/ci.yml'), 'utf8')
  const release = readFileSync(join(REPOSITORY_ROOT, '.github/workflows/release.yml'), 'utf8')
  for (const workflow of [ci, release]) {
    assert.match(workflow, /--trusted-source-root/)
    assert.doesNotMatch(workflow, /trusted-raw-parity-bundle|externally-asserted-equal-output-hashes|evidenceSha256/)
  }
  assert.match(ci, /run\.event!=='workflow_call'&&run\.event!=='workflow_dispatch'/)
  assert.match(release, /Normalize four release-native artifact directories/)
  assert.match(release, /downloaded-native-artifacts\/release-native-\$target/)
  assert.match(release, /--trusted-source-root \"\$GITHUB_WORKSPACE\"/)
})
