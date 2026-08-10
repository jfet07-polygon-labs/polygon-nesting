#!/usr/bin/env node
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { existsSync, lstatSync, mkdtempSync, readFileSync, readdirSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, relative, resolve, sep } from 'node:path'
import { fileURLToPath } from 'node:url'
import target from '../packages/polygon-nesting/npm/target.cjs'
import { validatePackageContents } from '../packages/polygon-nesting/scripts/build-native.mjs'
import { PACKAGE_DESTINATIONS, renderReleaseNotes, stable } from './assemble-release-candidate.mjs'

const SHA = /^[a-f0-9]{64}$/
const DIGEST = /^sha256:[a-f0-9]{64}$/
const hash = (path) => createHash('sha256').update(readFileSync(path)).digest('hex')
const committedHash = (trustedSourceRoot, sourceCommit, path) => createHash('sha256').update(execFileSync(
  'git',
  ['-C', trustedSourceRoot, 'cat-file', 'blob', `${sourceCommit}:${path}`],
  { env: { ...process.env, GIT_NO_REPLACE_OBJECTS: '1' } }
)).digest('hex')

function equal(actual, expected, label) {
  if (actual !== expected) throw new Error(`${label} must be ${JSON.stringify(expected)}, received ${JSON.stringify(actual)}`)
}

function json(path, label) {
  try {
    return JSON.parse(readFileSync(path, 'utf8'))
  } catch (error) {
    throw new Error(`${label} is not valid JSON`, { cause: error })
  }
}

function exactKeys(value, keys, label) {
  if (!value || typeof value !== 'object' || Array.isArray(value) || JSON.stringify(Object.keys(value).sort()) !== JSON.stringify([...keys].sort())) {
    throw new Error(`${label} schema is not accepted`)
  }
}

const legal = (trustedSourceRoot, sourceCommit) => ({
  'LICENSES/clipper2-ts-BSL-1.0.txt': committedHash(trustedSourceRoot, sourceCommit, 'packages/polygon-nesting/LICENSES/clipper2-ts-BSL-1.0.txt'),
  NOTICE: committedHash(trustedSourceRoot, sourceCommit, 'packages/polygon-nesting/NOTICE')
})

function assertFile(path, label) {
  if (!existsSync(path) || !lstatSync(path).isFile() || lstatSync(path).isSymbolicLink()) throw new Error(`${label} must be a regular file`)
}

export function validateReleaseMetadata(release, trustedSourceRoot) {
  exactKeys(release, ['legalHashes', 'nativeArtifacts', 'npmPackages', 'releaseNotes', 'schemaVersion', 'sourceCommit'], 'release metadata')
  equal(release.schemaVersion, 4, 'release schemaVersion')
  if (!/^[a-f0-9]{40}$/.test(release.sourceCommit ?? '')) throw new Error('release sourceCommit is invalid')
  equal(release.nativeArtifacts?.length, 2, 'native artifact count')
  equal(release.npmPackages?.length, 2, 'npm package count')
  const targets = Object.entries(target.PUBLISHED_NATIVE_TARGETS)
  equal(JSON.stringify(release.nativeArtifacts.map(({ targetKey }) => targetKey).sort()), JSON.stringify(targets.map(([key]) => key).sort()), 'native target keys')
  for (const artifact of release.nativeArtifacts) {
    exactKeys(artifact, ['addonName', 'arch', 'buildMetadata', 'cargoTarget', 'platform', 'sha256', 'targetKey'], `${artifact.targetKey} native artifact`)
    const expected = target.PUBLISHED_NATIVE_TARGETS[artifact.targetKey]
    equal(artifact.cargoTarget, expected.cargoTarget, `${artifact.targetKey} cargo target`)
    equal(artifact.platform, expected.platform, `${artifact.targetKey} platform`)
    equal(artifact.arch, expected.arch, `${artifact.targetKey} arch`)
    equal(artifact.addonName, target.stagedAddonFileName(expected.platform, expected.arch), `${artifact.targetKey} addon name`)
    if (!SHA.test(artifact.sha256)) throw new Error(`${artifact.targetKey} addon hash is invalid`)
    exactKeys(artifact.buildMetadata, ['arch', 'cargo', 'cargoTarget', 'features', 'nativeDependency', 'platform', 'profile', 'rustc', 'schemaVersion', 'sourceRevision', 'targetKey'], `${artifact.targetKey} build metadata`)
    equal(artifact.buildMetadata.sourceRevision, release.sourceCommit, `${artifact.targetKey} build source revision`)
    equal(artifact.buildMetadata.schemaVersion, 2, `${artifact.targetKey} build metadata version`)
    if (!String(artifact.buildMetadata.rustc).startsWith('rustc 1.95.0') || !String(artifact.buildMetadata.cargo).startsWith('cargo 1.95.0')) throw new Error(`${artifact.targetKey} build toolchain is not pinned`)
    if (!SHA.test(artifact.buildMetadata.nativeDependency?.cargoLockSha256) || !SHA.test(artifact.buildMetadata.nativeDependency?.napiManifestSha256)) throw new Error(`${artifact.targetKey} native dependency provenance is invalid`)
    equal(artifact.buildMetadata.nativeDependency.cargoLockSha256, committedHash(trustedSourceRoot, release.sourceCommit, 'Cargo.lock'), `${artifact.targetKey} Cargo.lock identity`)
    equal(artifact.buildMetadata.nativeDependency.napiManifestSha256, committedHash(trustedSourceRoot, release.sourceCommit, 'crates/polygon-nesting-napi/Cargo.toml'), `${artifact.targetKey} N-API manifest identity`)
  }
  for (const [index, packageRecord] of release.npmPackages.entries()) {
    const destination = PACKAGE_DESTINATIONS[index]
    exactKeys(packageRecord, ['key', 'name', 'packManifest', 'packedFiles', 'registry', 'tarball', 'version'], `${destination.key} npm package`)
    equal(packageRecord.key, destination.key, `${destination.key} npm package key`)
    equal(packageRecord.name, destination.name, `${destination.key} npm package name`)
    equal(packageRecord.version, '0.1.3', `${destination.key} npm package version`)
    equal(packageRecord.registry, destination.registry, `${destination.key} npm package registry`)
    exactKeys(packageRecord.packManifest, ['fileName', 'sha256'], `${destination.key} pack manifest`)
    exactKeys(packageRecord.tarball, ['fileName', 'sha256'], `${destination.key} tarball`)
    if (!SHA.test(packageRecord.packManifest.sha256) || !SHA.test(packageRecord.tarball.sha256)) throw new Error(`${destination.key} npm package hash is invalid`)
  }
  if (release.npmPackages[0].tarball.sha256 === release.npmPackages[1].tarball.sha256) throw new Error('dual npm tarballs must have independent hashes')
  for (const [name, value] of Object.entries(legal(trustedSourceRoot, release.sourceCommit))) equal(release.legalHashes?.[name], value, `legal hash ${name}`)
}

function walk(root, current = root, results = []) {
  for (const entry of readdirSync(current, { withFileTypes: true })) {
    const path = join(current, entry.name)
    if (entry.isDirectory()) walk(root, path, results)
    else if (entry.isFile() && !lstatSync(path).isSymbolicLink()) results.push(relative(root, path).split(sep).join('/'))
    else throw new Error('tarball contains non-regular file')
  }
  return results
}

function extractTarball(tarball, execute) {
  const extraction = mkdtempSync(join(tmpdir(), 'polygon-release-'))
  const listing = execute('tar', ['-tzf', tarball], { encoding: 'utf8' }).split(/\r?\n/).filter(Boolean)
  if (listing.some((entry) => entry.startsWith('/') || entry.split('/').includes('..'))) {
    rmSync(extraction, { recursive: true, force: true })
    throw new Error('npm tarball contains an unsafe path')
  }
  const archivePaths = listing
    .map((entry) => entry.replace(/^\.\//, '').replace(/\/$/, ''))
    .filter((entry) => entry && entry !== '.')
  if (!archivePaths.includes('package/package.json') || archivePaths.some((entry) => entry !== 'package' && !entry.startsWith('package/'))) {
    rmSync(extraction, { recursive: true, force: true })
    throw new Error('npm tarball contains an entry outside package directory')
  }
  execute('tar', ['-xzf', tarball, '-C', extraction])
  return { extraction, packageRoot: join(extraction, 'package') }
}

export function validatePackageManifestPair(githubManifest, publicManifest) {
  equal(githubManifest.name, PACKAGE_DESTINATIONS[0].name, 'GitHub Packages manifest name')
  equal(publicManifest.name, PACKAGE_DESTINATIONS[1].name, 'npmjs manifest name')
  equal(githubManifest.version, '0.1.3', 'GitHub Packages manifest version')
  equal(publicManifest.version, '0.1.3', 'npmjs manifest version')
  equal(githubManifest.publishConfig?.registry, PACKAGE_DESTINATIONS[0].registry, 'GitHub Packages manifest registry')
  equal(publicManifest.publishConfig?.registry, PACKAGE_DESTINATIONS[1].registry, 'npmjs manifest registry')
  const normalizedPublic = structuredClone(publicManifest)
  normalizedPublic.name = githubManifest.name
  normalizedPublic.publishConfig = { ...normalizedPublic.publishConfig, registry: githubManifest.publishConfig.registry }
  if (JSON.stringify(stable(normalizedPublic)) !== JSON.stringify(stable(githubManifest))) {
    throw new Error('dual package manifests differ beyond approved identity and registry differences')
  }
}

export function comparePackagePayloads({ candidateDirectory, packageRecords, execute = execFileSync }) {
  if (!Array.isArray(packageRecords) || packageRecords.length !== 2) throw new Error('dual package comparison requires exactly two package records')
  const extracted = packageRecords.map((record) => {
    const tarball = join(resolve(candidateDirectory), record.tarball.fileName)
    assertFile(tarball, `${record.key} tarball`)
    return extractTarball(tarball, execute)
  })
  try {
    const manifests = extracted.map(({ packageRoot }) => json(join(packageRoot, 'package.json'), 'package manifest'))
    validatePackageManifestPair(manifests[0], manifests[1])
    const fileLists = extracted.map(({ packageRoot }) => walk(packageRoot).filter((path) => path !== 'package.json').sort())
    equal(JSON.stringify(fileLists[0]), JSON.stringify(fileLists[1]), 'dual package non-manifest file closure')
    const payloads = fileLists.map((files, index) => Object.fromEntries(files.map((path) => [path, readFileSync(join(extracted[index].packageRoot, path))])))
    for (const path of fileLists[0]) {
      if (!payloads[0][path].equals(payloads[1][path])) throw new Error(`dual package payload bytes differ: ${path}`)
    }
    const digestPayload = (payload) => Object.fromEntries(Object.entries(payload).map(([path, bytes]) => [path, createHash('sha256').update(bytes).digest('hex')]))
    return {
      githubManifest: manifests[0],
      publicManifest: manifests[1],
      githubPayload: digestPayload(payloads[0]),
      publicPayload: digestPayload(payloads[1])
    }
  } finally {
    for (const { extraction } of extracted) rmSync(extraction, { recursive: true, force: true })
  }
}

function verifyNpmPackageRecord({ root, packageRecord, nativeArtifacts, execute }) {
  const manifestPath = join(root, packageRecord.packManifest.fileName)
  const tarballPath = join(root, packageRecord.tarball.fileName)
  assertFile(manifestPath, `${packageRecord.key} pack manifest`)
  assertFile(tarballPath, `${packageRecord.key} tarball`)
  equal(hash(manifestPath), packageRecord.packManifest.sha256, `${packageRecord.key} pack manifest SHA-256`)
  equal(hash(tarballPath), packageRecord.tarball.sha256, `${packageRecord.key} tarball SHA-256`)
  const pack = json(manifestPath, `${packageRecord.key} pack manifest`)
  equal(pack.name, packageRecord.name, `${packageRecord.key} npm pack name`)
  equal(pack.version, packageRecord.version, `${packageRecord.key} npm pack version`)
  equal(pack.filename, packageRecord.tarball.fileName, `${packageRecord.key} npm pack filename`)
  equal(pack.shasum, createHash('sha1').update(readFileSync(tarballPath)).digest('hex'), `${packageRecord.key} npm pack shasum`)
  equal(pack.integrity, `sha512-${createHash('sha512').update(readFileSync(tarballPath)).digest('base64')}`, `${packageRecord.key} npm pack integrity`)
  if (!Array.isArray(pack.files) || new Set(pack.files.map((entry) => entry.path)).size !== pack.files.length) throw new Error(`${packageRecord.key} npm pack files are not unique`)
  validatePackageContents(pack.files.map(({ path }) => path).sort(), { requireAllTargets: true })

  const { extraction, packageRoot } = extractTarball(tarballPath, execute)
  try {
    equal(JSON.stringify(walk(packageRoot).sort()), JSON.stringify(pack.files.map(({ path }) => path).sort()), `${packageRecord.key} tarball manifest closure`)
    if (!Array.isArray(packageRecord.packedFiles) || JSON.stringify(packageRecord.packedFiles.map((entry) => entry.path).sort()) !== JSON.stringify(pack.files.map((entry) => entry.path).sort())) {
      throw new Error(`${packageRecord.key} release packed file closure is not accepted`)
    }
    for (const entry of pack.files) {
      if (typeof entry.path !== 'string' || !Number.isSafeInteger(entry.size) || entry.size < 0) throw new Error(`${packageRecord.key} npm pack file record is invalid`)
      const recorded = packageRecord.packedFiles.find((candidate) => candidate.path === entry.path)
      if (!recorded || recorded.size !== entry.size || recorded.mode !== entry.mode || !SHA.test(recorded.sha256)) throw new Error(`${packageRecord.key} release packed file record is invalid: ${entry.path}`)
      const bytes = readFileSync(join(packageRoot, entry.path))
      equal(bytes.length, entry.size, `${packageRecord.key} npm pack file size ${entry.path}`)
      equal(createHash('sha256').update(bytes).digest('hex'), recorded.sha256, `${packageRecord.key} npm pack file digest ${entry.path}`)
    }
    for (const item of nativeArtifacts) equal(hash(join(packageRoot, 'npm', item.addonName)), item.sha256, `${packageRecord.key} ${item.targetKey} tarball addon`)
  } finally {
    rmSync(extraction, { recursive: true, force: true })
  }
}

function descriptorBlob(root, descriptor, label, mediaTypes) {
  if (!descriptor || !mediaTypes.includes(descriptor.mediaType) || !DIGEST.test(descriptor.digest ?? '') || !Number.isSafeInteger(descriptor.size) || descriptor.size < 0) throw new Error(`${label} descriptor is invalid`)
  const path = join(root, 'blobs', 'sha256', descriptor.digest.slice(7))
  assertFile(path, `${label} blob`)
  equal(readFileSync(path).length, descriptor.size, `${label} blob size`)
  equal(`sha256:${hash(path)}`, descriptor.digest, `${label} blob digest`)
  return path
}

function verifyOciArchive({ archivePath, evidence, execute = execFileSync }) {
  assertFile(archivePath, 'OCI archive')
  equal(hash(archivePath), evidence.archiveSha256, 'OCI archive SHA-256')
  const root = mkdtempSync(join(tmpdir(), 'polygon-oci-'))
  try {
    const listing = execute('tar', ['-tf', archivePath], { encoding: 'utf8' }).split(/\r?\n/).filter(Boolean)
    if (listing.some((entry) => entry.startsWith('/') || entry.split('/').includes('..'))) throw new Error('OCI archive contains an unsafe path')
    execute('tar', ['-xf', archivePath, '--no-same-owner', '-C', root])
    const index = json(join(root, 'index.json'), 'OCI index')
    const descriptor = index.manifests?.[0]
    if (!descriptor || index.schemaVersion !== 2 || index.manifests.length !== 1 || descriptor.platform?.os !== 'linux' || descriptor.platform?.architecture !== 'amd64' || descriptor.mediaType !== 'application/vnd.oci.image.manifest.v1+json') throw new Error('OCI index is invalid')
    const manifestPath = descriptorBlob(root, descriptor, 'OCI manifest', ['application/vnd.oci.image.manifest.v1+json'])
    equal(descriptor.digest, evidence.manifestDigest, 'OCI evidence manifest digest')
    const manifest = json(manifestPath, 'OCI manifest')
    if (manifest.schemaVersion !== 2 || manifest.mediaType !== 'application/vnd.oci.image.manifest.v1+json' || !Array.isArray(manifest.layers)) throw new Error('OCI manifest is invalid')
    const configPath = descriptorBlob(root, manifest.config, 'OCI config', ['application/vnd.oci.image.config.v1+json'])
    const config = json(configPath, 'OCI config')
    if (config.architecture !== 'amd64' || config.os !== 'linux') throw new Error('OCI config platform is invalid')
    if (typeof config.config?.User !== 'string' || !config.config.User || config.config.User === 'root' || config.config.User === '0') throw new Error('OCI config must run as non-root')
    if (JSON.stringify(config.config?.Labels ?? {}) !== JSON.stringify(evidence.labels ?? {})) throw new Error('OCI config labels differ from evidence')
    const expected = new Set(['index.json', 'oci-layout', `blobs/sha256/${descriptor.digest.slice(7)}`, `blobs/sha256/${manifest.config.digest.slice(7)}`])
    for (const layer of manifest.layers) {
      const layerPath = descriptorBlob(root, layer, 'OCI layer', ['application/vnd.oci.image.layer.v1.tar+gzip', 'application/vnd.oci.image.layer.v1.tar'])
      expected.add(relative(root, layerPath).split(sep).join('/'))
    }
    const actual = new Set(walk(root).sort())
    if (JSON.stringify([...actual].sort()) !== JSON.stringify([...expected].sort())) throw new Error('OCI archive closure is not accepted')
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
}

export async function verifyReleaseCandidate({ candidateDirectory, trustedSourceRoot, ociArchivePath, ociEvidencePath, execute = execFileSync }) {
  const root = resolve(candidateDirectory)
  const release = json(join(root, 'release.json'), 'release metadata')
  if (!trustedSourceRoot) throw new Error('trustedSourceRoot is required')
  validateReleaseMetadata(release, trustedSourceRoot)
  const notes = join(root, release.releaseNotes?.fileName ?? '')
  assertFile(notes, 'release notes')
  equal(hash(notes), release.releaseNotes?.sha256, 'release notes SHA-256')
  equal(readFileSync(notes, 'utf8'), renderReleaseNotes(release), 'release notes')
  for (const packageRecord of release.npmPackages) verifyNpmPackageRecord({ root, packageRecord, nativeArtifacts: release.nativeArtifacts, execute })
  comparePackagePayloads({ candidateDirectory: root, packageRecords: release.npmPackages, execute })

  if (ociArchivePath || ociEvidencePath) {
    if (!ociArchivePath || !ociEvidencePath) throw new Error('OCI evidence and archive are required together')
    const evidence = json(ociEvidencePath, 'OCI evidence')
    exactKeys(evidence, ['archiveSha256', 'immutableImageReference', 'labels', 'legalHashes', 'manifestDigest', 'nonRootSmoke', 'platform', 'schemaVersion', 'sourceCommit'], 'OCI evidence')
    equal(evidence.schemaVersion, 1, 'OCI evidence version')
    equal(evidence.platform, 'linux/amd64', 'OCI platform')
    equal(evidence.nonRootSmoke, true, 'OCI non-root smoke')
    equal(evidence.sourceCommit, release.sourceCommit, 'OCI source commit')
    equal(evidence.immutableImageReference, `127.0.0.1:5000/polygon-nesting@${evidence.manifestDigest}`, 'OCI immutable reference')
    for (const [key, value] of Object.entries({
      'org.opencontainers.image.title': 'polygon-nesting',
      'org.opencontainers.image.source': 'https://github.com/jfet07-polygon-labs/polygon-nesting',
      'org.opencontainers.image.version': '0.1.3',
      'org.opencontainers.image.revision': release.sourceCommit,
      'org.opencontainers.image.licenses': 'NOASSERTION'
    })) equal(evidence.labels?.[key], value, `OCI label ${key}`)
    equal(JSON.stringify(evidence.legalHashes), JSON.stringify(release.legalHashes), 'OCI legal hashes')
    verifyOciArchive({ archivePath: ociArchivePath, evidence, execute })
  }
  return release
}

function dryRun() {
  equal(Object.keys(target.PUBLISHED_NATIVE_TARGETS).length, 2, 'published native target count')
  equal(PACKAGE_DESTINATIONS.length, 2, 'published npm destination count')
  return true
}

function parseArgs(argv) {
  const options = {}
  for (let index = 0; index < argv.length; index += 1) {
    const key = argv[index]
    if (key === '--dry-run') {
      options.dryRun = true
      continue
    }
    const value = argv[++index]
    if (!value || !['--candidate', '--trusted-source-root', '--oci-archive', '--oci-evidence'].includes(key)) throw new Error(`unknown or incomplete option: ${key}`)
    options[{
      '--candidate': 'candidateDirectory',
      '--trusted-source-root': 'trustedSourceRoot',
      '--oci-archive': 'ociArchivePath',
      '--oci-evidence': 'ociEvidencePath'
    }[key]] = value
  }
  return options
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const options = parseArgs(process.argv.slice(2))
  ;(options.dryRun ? Promise.resolve(dryRun()) : verifyReleaseCandidate(options)).catch((error) => {
    console.error(`[verify-release-candidate] ${error.message}`)
    process.exitCode = 1
  })
}

export { dryRun, parseArgs, verifyOciArchive }
