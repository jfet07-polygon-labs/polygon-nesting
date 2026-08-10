import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { cpSync, mkdtempSync, mkdirSync, readFileSync, rmSync, symlinkSync, unlinkSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import test from 'node:test'
import { fileURLToPath, pathToFileURL } from 'node:url'

const REPOSITORY_ROOT = dirname(dirname(fileURLToPath(import.meta.url)))
const TARGETS = [
  ['linux-x64', 'linux', 'x64', 'x86_64-unknown-linux-gnu'],
  ['darwin-arm64', 'darwin', 'arm64', 'aarch64-apple-darwin']
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
    version: '0.1.2',
    publishConfig: { registry: 'https://npm.pkg.github.com' },
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

function makeFixture(t) {
  const root = mkdtempSync(join(tmpdir(), 'polygon-release-ci-'))
  const artifactsRoot = join(root, 'artifacts')
  const packageRoot = makePackageFixture(root)
  const outputDirectory = join(root, 'candidate')
  const sourceCommit = execFileSync('git', ['stash', 'create'], { cwd: REPOSITORY_ROOT, encoding: 'utf8' }).trim()
    || execFileSync('git', ['rev-parse', 'HEAD'], { cwd: REPOSITORY_ROOT, encoding: 'utf8' }).trim()
  for (const [targetKey, platform, arch, cargoTarget] of TARGETS) {
    const directory = join(artifactsRoot, targetKey)
    const addonName = `irregular-nesting-native.${targetKey}.node`
    const bytes = Buffer.from(`native-${targetKey}`)
    mkdirSync(directory, { recursive: true })
    writeFileSync(join(directory, addonName), bytes)
    writeFileSync(join(directory, `${addonName}.sha256`), `${sha256(bytes)}  ${addonName}\n`)
    writeJson(join(directory, 'target.json'), {
      schemaVersion: 2,
      targetKey,
      platform,
      arch,
      cargoTarget,
      rustc: 'rustc 1.95.0 fixture',
      cargo: 'cargo 1.95.0 fixture',
      profile: 'release',
      features: [],
      sourceRevision: sourceCommit,
      nativeDependency: {
        cargoLockSha256: sha256(readFileSync(join(REPOSITORY_ROOT, 'Cargo.lock'))),
        napiManifestSha256: sha256(readFileSync(join(REPOSITORY_ROOT, 'crates/polygon-nesting-napi/Cargo.toml')))
      }
    })
  }
  t.after(() => rmSync(root, { force: true, recursive: true }))
  return { artifactsRoot, outputDirectory, packageRoot, root, sourceCommit, trustedSourceRoot: REPOSITORY_ROOT }
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

test('assembles both published native targets and verifies an offline candidate', async (t) => {
  const fixture = makeFixture(t)
  const { assembleReleaseCandidate } = await loadModule('assemble-release-candidate.mjs')
  const { verifyReleaseCandidate } = await loadModule('verify-release-candidate.mjs')
  const release = await assembleReleaseCandidate(fixture)
  assert.equal(release.schemaVersion, 4)
  assert.equal(release.nativeArtifacts.length, 2)
  assert.equal('parityAggregate' in release, false)
  assert.deepEqual(release.npmPackages.map(({ name, registry, version }) => ({ name, registry, version })), [
    { name: '@jfet07-polygon-labs/polygon-nesting', registry: 'https://npm.pkg.github.com', version: '0.1.2' },
    { name: '@jfet97/polygon-nesting', registry: 'https://registry.npmjs.org', version: '0.1.2' }
  ])
  assert.notEqual(release.npmPackages[0].tarball.sha256, release.npmPackages[1].tarball.sha256)
  await verifyReleaseCandidate({ candidateDirectory: fixture.outputDirectory, trustedSourceRoot: fixture.trustedSourceRoot })
  const repeated = await assembleReleaseCandidate({ ...fixture, outputDirectory: join(fixture.root, 'candidate-repeat') })
  assert.deepEqual(
    repeated.npmPackages.map(({ packManifest, tarball }) => ({ packManifest: packManifest.sha256, tarball: tarball.sha256 })),
    release.npmPackages.map(({ packManifest, tarball }) => ({ packManifest: packManifest.sha256, tarball: tarball.sha256 }))
  )
})

test('dual package verifier accepts only manifest identity and registry differences', async (t) => {
  const fixture = makeFixture(t)
  const { assembleReleaseCandidate } = await loadModule('assemble-release-candidate.mjs')
  const { comparePackagePayloads, validatePackageManifestPair } = await loadModule('verify-release-candidate.mjs')
  const release = await assembleReleaseCandidate(fixture)
  const [githubPackage, publicPackage] = release.npmPackages
  const payloads = comparePackagePayloads({
    candidateDirectory: fixture.outputDirectory,
    packageRecords: release.npmPackages
  })
  assert.deepEqual(payloads.githubPayload, payloads.publicPayload)
  assert.doesNotThrow(() => validatePackageManifestPair(
    payloads.githubManifest,
    payloads.publicManifest
  ))
  assert.throws(() => validatePackageManifestPair(
    payloads.githubManifest,
    { ...payloads.publicManifest, main: 'npm/other.cjs' }
  ), /approved identity and registry differences/)
  assert.notEqual(githubPackage.packManifest.sha256, publicPackage.packManifest.sha256)
})

test('dual package verifier rejects archive members outside package directory', async (t) => {
  const fixture = makeFixture(t)
  const { assembleReleaseCandidate } = await loadModule('assemble-release-candidate.mjs')
  const { comparePackagePayloads } = await loadModule('verify-release-candidate.mjs')
  const release = await assembleReleaseCandidate(fixture)
  const publicTarball = join(fixture.outputDirectory, release.npmPackages[1].tarball.fileName)
  const extraction = join(fixture.root, 'public-tarball')
  mkdirSync(extraction)
  execFileSync('tar', ['-xzf', publicTarball, '-C', extraction])
  writeFileSync(join(extraction, 'unexpected.txt'), 'not part of the npm package\n')
  execFileSync('tar', ['-czf', publicTarball, '-C', extraction, '.'])
  assert.throws(() => comparePackagePayloads({
    candidateDirectory: fixture.outputDirectory,
    packageRecords: release.npmPackages
  }), /outside package directory/)
})

test('release metadata resolves legal evidence from the selected source commit', async (t) => {
  const fixture = makeFixture(t)
  const { assembleReleaseCandidate } = await loadModule('assemble-release-candidate.mjs')
  const { validateReleaseMetadata } = await loadModule('verify-release-candidate.mjs')
  const release = await assembleReleaseCandidate(fixture)
  const trustedSourceRoot = join(fixture.root, 'trusted-source')
  const notice = Buffer.from('selected source notice\n')
  const license = Buffer.from('selected source license\n')
  mkdirSync(join(trustedSourceRoot, 'crates/polygon-nesting-napi'), { recursive: true })
  mkdirSync(join(trustedSourceRoot, 'packages/polygon-nesting/LICENSES'), { recursive: true })
  cpSync(join(REPOSITORY_ROOT, 'Cargo.lock'), join(trustedSourceRoot, 'Cargo.lock'))
  cpSync(join(REPOSITORY_ROOT, 'crates/polygon-nesting-napi/Cargo.toml'), join(trustedSourceRoot, 'crates/polygon-nesting-napi/Cargo.toml'))
  writeFileSync(join(trustedSourceRoot, 'packages/polygon-nesting/NOTICE'), notice)
  writeFileSync(join(trustedSourceRoot, 'packages/polygon-nesting/LICENSES/clipper2-ts-BSL-1.0.txt'), license)
  execFileSync('git', ['init'], { cwd: trustedSourceRoot })
  execFileSync('git', ['add', '.'], { cwd: trustedSourceRoot })
  execFileSync('git', ['-c', 'user.name=Release Test', '-c', 'user.email=release-test@example.com', 'commit', '-m', 'fixture'], { cwd: trustedSourceRoot })
  release.sourceCommit = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: trustedSourceRoot, encoding: 'utf8' }).trim()
  release.legalHashes = {
    'LICENSES/clipper2-ts-BSL-1.0.txt': sha256(license),
    NOTICE: sha256(notice)
  }
  for (const artifact of release.nativeArtifacts) artifact.buildMetadata.sourceRevision = release.sourceCommit
  validateReleaseMetadata(release, trustedSourceRoot)
})

test('candidate assembly rejects a source revision that differs from native metadata', async (t) => {
  const fixture = makeFixture(t)
  const { assembleReleaseCandidate } = await loadModule('assemble-release-candidate.mjs')
  await assert.rejects(
    assembleReleaseCandidate({ ...fixture, sourceCommit: '0123456789abcdef0123456789abcdef01234567' }),
    /sourceRevision/
  )
})

test('candidate assembly rejects target metadata swapped between Rust triples', async (t) => {
  const fixture = makeFixture(t)
  const metadataPath = join(fixture.artifactsRoot, 'linux-x64', 'target.json')
  const metadata = JSON.parse(readFileSync(metadataPath, 'utf8'))
  metadata.cargoTarget = 'aarch64-apple-darwin'
  writeJson(metadataPath, metadata)
  const { assembleReleaseCandidate } = await loadModule('assemble-release-candidate.mjs')
  await assert.rejects(assembleReleaseCandidate(fixture), /cargoTarget/)
})

test('candidate assembly rejects a mutated native addon with an unchanged checksum', async (t) => {
  const fixture = makeFixture(t)
  writeFileSync(join(fixture.artifactsRoot, 'linux-x64', 'irregular-nesting-native.linux-x64.node'), 'mutation')
  const { assembleReleaseCandidate } = await loadModule('assemble-release-candidate.mjs')
  await assert.rejects(assembleReleaseCandidate(fixture), /checksum/)
})

test('offline verification rejects a mutated candidate manifest', async (t) => {
  const fixture = makeFixture(t)
  const { assembleReleaseCandidate } = await loadModule('assemble-release-candidate.mjs')
  const { verifyReleaseCandidate } = await loadModule('verify-release-candidate.mjs')
  const release = await assembleReleaseCandidate(fixture)
  const manifest = join(fixture.outputDirectory, release.npmPackages[0].packManifest.fileName)
  writeFileSync(manifest, `${readFileSync(manifest, 'utf8')} `)
  await assert.rejects(verifyReleaseCandidate({ candidateDirectory: fixture.outputDirectory, trustedSourceRoot: fixture.trustedSourceRoot }), /pack manifest SHA-256/)
})

test('offline verification recomputes OCI archive and runtime publication evidence', async (t) => {
  const fixture = makeFixture(t)
  const { assembleReleaseCandidate } = await loadModule('assemble-release-candidate.mjs')
  const { verifyReleaseCandidate } = await loadModule('verify-release-candidate.mjs')
  const { verifyRuntimePublication } = await loadModule('verify-runtime-publication.mjs')
  const release = await assembleReleaseCandidate(fixture)
  const labels = { 'org.opencontainers.image.title': 'polygon-nesting', 'org.opencontainers.image.licenses': 'NOASSERTION', 'org.opencontainers.image.source': 'https://github.com/jfet07-polygon-labs/polygon-nesting', 'org.opencontainers.image.version': '0.1.2', 'org.opencontainers.image.revision': release.sourceCommit }
  const oci = makeOciArchive(fixture.root, labels)
  const evidencePath = join(fixture.root, 'oci-evidence.json')
  const manifestDigestPath = join(fixture.root, 'manifest-digest.txt')
  writeFileSync(manifestDigestPath, `${oci.manifestDigest}\n`)
  writeJson(evidencePath, {
    schemaVersion: 1,
    manifestDigest: oci.manifestDigest,
    archiveSha256: sha256(readFileSync(oci.archivePath)),
    immutableImageReference: `127.0.0.1:5000/polygon-nesting@${oci.manifestDigest}`,
    platform: 'linux/amd64',
    sourceCommit: release.sourceCommit,
    nonRootSmoke: true,
    labels,
    legalHashes: release.legalHashes
  })
  await verifyReleaseCandidate({ candidateDirectory: fixture.outputDirectory, trustedSourceRoot: fixture.trustedSourceRoot, ociArchivePath: oci.archivePath, ociEvidencePath: evidencePath })
  assert.deepEqual(await verifyRuntimePublication({
    candidateDirectory: fixture.outputDirectory,
    trustedSourceRoot: fixture.trustedSourceRoot,
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
  await assert.rejects(verifyRuntimePublication({ candidateDirectory: fixture.outputDirectory, trustedSourceRoot: fixture.trustedSourceRoot, ociArchivePath: oci.archivePath, ociEvidencePath: evidenceLink, manifestDigestPath, sourceCommit: release.sourceCommit, sourceRunId: '123' }), /OCI evidence must be a regular file/)
  unlinkSync(evidenceLink)
  await assert.rejects(verifyRuntimePublication({ candidateDirectory: fixture.outputDirectory, trustedSourceRoot: fixture.trustedSourceRoot, ociArchivePath: oci.archivePath, ociEvidencePath: evidencePath, manifestDigestPath, sourceCommit: '0'.repeat(40), sourceRunId: '123' }), /release sourceCommit/)
  writeFileSync(oci.archivePath, Buffer.concat([readFileSync(oci.archivePath), Buffer.from('mutation')]))
  await assert.rejects(verifyRuntimePublication({ candidateDirectory: fixture.outputDirectory, trustedSourceRoot: fixture.trustedSourceRoot, ociArchivePath: oci.archivePath, ociEvidencePath: evidencePath, manifestDigestPath, sourceCommit: release.sourceCommit, sourceRunId: '123' }), /OCI archive SHA-256/)
})

test('normal PR quality checks run current repository release contracts', () => {
  const ci = readFileSync(join(REPOSITORY_ROOT, '.github/workflows/ci.yml'), 'utf8')
  assert.match(ci, /node --test tests\/ci\/\*\.test\.mjs tests\/parity\/\*\.test\.mjs scripts\/release-ci\.test\.mjs/)
  assert.match(ci, /node scripts\/run-current-canonical-matrix\.mjs/)
  assert.doesNotMatch(ci, /publish-native-package\.test\.mjs/)
  assert.doesNotMatch(ci, /^\s*node --test scripts\/release-ci\.test\.mjs$/m)
})

test('main CI is the sole build evidence producer and excludes hosted Windows and macOS x64', () => {
  const ci = readFileSync(join(REPOSITORY_ROOT, '.github/workflows/ci.yml'), 'utf8')
  assert.doesNotMatch(ci, /workflow_dispatch|win32-x64|windows-2025|darwin-x64|x86_64-apple-darwin|macos-15-intel/)
  assert.match(ci, /if: github\.event_name == 'push'/)
  assert.match(ci, /name: native-build-\$\{\{ matrix\.target-key \}\}/)
  assert.match(ci, /name: oci-build-\$\{\{ github\.sha \}\}/)
  assert.match(ci, /--output type=oci,dest=oci-image\.tar/)
  assert.match(ci, /smoke-cli-image\.sh/)
})

test('release reuses exact successful main CI evidence without quality, compilation, or image rebuilds', () => {
  const ci = readFileSync(join(REPOSITORY_ROOT, '.github/workflows/ci.yml'), 'utf8')
  const release = readFileSync(join(REPOSITORY_ROOT, '.github/workflows/release.yml'), 'utf8')
  const assembler = readFileSync(join(REPOSITORY_ROOT, 'scripts/assemble-release-candidate.mjs'), 'utf8')
  assert.doesNotMatch(release, /Task112|parity-bound|release-native-|externally generated parity/)
  assert.match(release, /run\.event !== 'push'/)
  assert.match(release, /run\.head_branch !== 'main'/)
  assert.match(release, /pattern: native-build-\*/)
  assert.match(release, /name: oci-build-\$\{\{ needs\.authorize-source\.outputs\.source-commit \}\}/)
  assert.match(release, /downloaded-native-artifacts\/native-build-\$target/)
  assert.match(release, /for target in linux-x64 darwin-arm64/)
  assert.doesNotMatch(release, /darwin-x64|x86_64-apple-darwin|macos-15-intel/)
  assert.match(release, /--trusted-source-root "\$GITHUB_WORKSPACE"/)
  assert.match(ci, /node --test packages\/polygon-nesting\/scripts\/build-native\.test\.mjs/)
  assert.doesNotMatch(release, /cargo (build|test|clippy)|npm (run build:release|test)|docker buildx build|smoke-cli-image\.sh/)
  assert.doesNotMatch(assembler, /execute\('npm', \['test'\]/)
})
