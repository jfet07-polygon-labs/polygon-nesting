#!/usr/bin/env node
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import {
  copyFileSync,
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  realpathSync,
  writeFileSync
} from 'node:fs'
import { gunzipSync } from 'node:zlib'
import { basename, dirname, join, relative, resolve, sep } from 'node:path'
import { fileURLToPath } from 'node:url'

const PACKAGE_NAME = '@jfet07-polygon-labs/polygon-nesting'
const PACKAGE_VERSION = '0.1.0'
const REGISTRY = 'https://npm.pkg.github.com'
const SOURCE_REPOSITORY = 'jfet07-polygon-labs/polygon-nesting'
const SOURCE_RUN_ID = 31109349775
const SOURCE_WORKFLOW_PATH = '.github/workflows/ci.yml'
const SOURCE_WORKFLOW_NAME = 'CI'
const SOURCE_RUN_EVENT = 'pull_request'
const SOURCE_RUN_STATUS = 'completed'
const SOURCE_RUN_CONCLUSION = 'success'
const SOURCE_HEAD_SHA = '92d51ba49c496ccd818646e9504bd042b2f73187'
const SOURCE_REVISION = 'e6d62e6c751329ccd77292bbf2d175805a260032'
const RUSTC_VERSION = 'rustc 1.95.0 (59807616e 2026-04-14)'
const CARGO_VERSION = 'cargo 1.95.0 (f2d3ce0bd 2026-03-21)'
const SHA1 = /^[a-f0-9]{40}$/
const PACKAGE_FILE_HASHES = Object.freeze({
  'LICENSES/clipper2-ts-BSL-1.0.txt': 'ea056d2c64294936b226f7360c265e77c52adc4ba171ee61029357f101f439cf',
  NOTICE: '1fa11aadfd5f98d734cbaced1fa10d525fd85565c560044734db4ce752037c1d',
  'npm/index.cjs': '1352a24bdaa1031335f700333e5470c240de72d13b10fba6e8d56d85c0079098',
  'npm/target.cjs': '8ffe08bfacb0df8076037951a6a6e1ea8bc223bdc9d3403ea2a60abeb7d997b4',
  'package.json': '9dd59f387cf56267a490d025cef2aef0f2de27e1b33e6b425bd4584e7184cf1a'
})
const BASE_PACKAGE_FILES = Object.freeze([
  'LICENSES/clipper2-ts-BSL-1.0.txt',
  'NOTICE',
  'npm/index.cjs',
  'npm/target.cjs',
  'package.json'
])
const TARGET_CONTRACTS = Object.freeze({
  'linux-x64': Object.freeze({
    artifactName: 'native-build-linux-x64',
    platform: 'linux',
    arch: 'x64',
    cargoTarget: 'x86_64-unknown-linux-gnu',
    addonName: 'irregular-nesting-native.linux-x64.node',
    sha256: '383d89b9a118e547edb80b7169ae1c374370d8e105dd928add5c767c60dac004',
    cargoLockSha256: 'df251a33c90fd1c81e332fb8bc1190c9fa69b75ec12ce8310e0b1172ceaa91ff',
    napiManifestSha256: '7e9d9af5ef6c2b99d4770d3890aaf1708637a123cf06b34b61b54a3ea76de4dd'
  }),
  'win32-x64': Object.freeze({
    artifactName: 'native-build-win32-x64',
    platform: 'win32',
    arch: 'x64',
    cargoTarget: 'x86_64-pc-windows-msvc',
    addonName: 'irregular-nesting-native.win32-x64.node',
    sha256: '107dd43c45480143fca40208324752044afbf2f575ab5b9685d2e76de32ba4a8',
    cargoLockSha256: '314942668602017b11271b5429071ffd852e13ecfb89941cd5900f3eac2e73a8',
    napiManifestSha256: '947802fbcaa4a6e109de64370ee668484df706a573af4919c999402dbfbe3980'
  }),
  'darwin-arm64': Object.freeze({
    artifactName: 'native-build-darwin-arm64',
    platform: 'darwin',
    arch: 'arm64',
    cargoTarget: 'aarch64-apple-darwin',
    addonName: 'irregular-nesting-native.darwin-arm64.node',
    sha256: '546c01e5b11ee6b46f69ca0604848de5855f3efb8383835d867229de751e27f2',
    cargoLockSha256: 'df251a33c90fd1c81e332fb8bc1190c9fa69b75ec12ce8310e0b1172ceaa91ff',
    napiManifestSha256: '7e9d9af5ef6c2b99d4770d3890aaf1708637a123cf06b34b61b54a3ea76de4dd'
  }),
  'darwin-x64': Object.freeze({
    artifactName: 'native-build-darwin-x64',
    platform: 'darwin',
    arch: 'x64',
    cargoTarget: 'x86_64-apple-darwin',
    addonName: 'irregular-nesting-native.darwin-x64.node',
    sha256: '7bda27790b76006b7af7ffa0172aa9c67d394c680c4b582f84a102c2e47459d4',
    cargoLockSha256: 'df251a33c90fd1c81e332fb8bc1190c9fa69b75ec12ce8310e0b1172ceaa91ff',
    napiManifestSha256: '7e9d9af5ef6c2b99d4770d3890aaf1708637a123cf06b34b61b54a3ea76de4dd'
  })
})

function hash(algorithm, bytes) {
  return createHash(algorithm).update(bytes).digest(algorithm === 'sha512' ? 'base64' : 'hex')
}

function assertEqual(actual, expected, label) {
  if (actual !== expected) {
    throw new Error(`${label} must be ${JSON.stringify(expected)}, received ${JSON.stringify(actual)}`)
  }
}

function readJson(path, label) {
  try {
    return JSON.parse(readFileSync(path, 'utf8'))
  } catch (error) {
    throw new Error(`${label} is not valid JSON`, { cause: error })
  }
}

function assertPlainFile(path, label) {
  if (!existsSync(path)) throw new Error(`${label} must exist as a regular file`)
  const status = lstatSync(path)
  if (!status.isFile() || status.isSymbolicLink() || status.nlink !== 1) {
    throw new Error(`${label} must be a regular file without links`)
  }
}

function assertPlainDirectory(path, label) {
  if (!existsSync(path)) throw new Error(`${label} must exist as a directory`)
  const status = lstatSync(path)
  if (!status.isDirectory() || status.isSymbolicLink()) throw new Error(`${label} must be a regular directory`)
}

function assertContainedPlainFile(root, relativePath, label) {
  const segments = relativePath.split('/')
  if (segments.length === 0 || segments.some((segment) => !segment || segment === '.' || segment === '..')) {
    throw new Error(`${label} has an unsafe relative path`)
  }
  let current = root
  for (let index = 0; index < segments.length; index += 1) {
    current = join(current, segments[index])
    if (!existsSync(current)) throw new Error(`${label} must exist`)
    const status = lstatSync(current)
    if (status.isSymbolicLink()) throw new Error(`${label} uses a symlinked path component`)
    if (index < segments.length - 1 && !status.isDirectory()) {
      throw new Error(`${label} parent must be a directory`)
    }
  }
  assertPlainFile(current, label)
  const physicalRoot = realpathSync(root)
  const physicalFile = realpathSync(current)
  if (!physicalFile.startsWith(`${physicalRoot}${sep}`)) throw new Error(`${label} is outside package source`)
  return current
}

function normalizedEntries(path) {
  return readdirSync(path).slice().sort()
}

function expectedPackageFiles(contracts) {
  return [
    ...BASE_PACKAGE_FILES,
    ...Object.values(contracts).map(({ addonName }) => `npm/${addonName}`)
  ].sort()
}

function validatePackageManifest(manifest) {
  assertEqual(manifest.name, PACKAGE_NAME, 'package name')
  assertEqual(manifest.version, PACKAGE_VERSION, 'package version')
  assertEqual(manifest.private, false, 'package private flag')
  assertEqual(manifest.publishConfig?.registry, REGISTRY, 'package registry')
  assertEqual(JSON.stringify(manifest.repository), JSON.stringify({
    type: 'git',
    url: 'git+https://github.com/jfet07-polygon-labs/polygon-nesting.git'
  }), 'package repository')
  assertEqual(manifest.main, 'npm/index.cjs', 'package main')
  assertEqual(manifest.exports, './npm/index.cjs', 'package exports')
  assertEqual(JSON.stringify(manifest.files), JSON.stringify([
    'npm/index.cjs',
    'npm/target.cjs',
    'npm/*.node',
    'NOTICE',
    'LICENSES/**'
  ]), 'package files allowlist')
}

function validateSourceRun(run) {
  assertEqual(run?.id, SOURCE_RUN_ID, 'run id')
  assertEqual(run?.name, SOURCE_WORKFLOW_NAME, 'run name')
  assertEqual(run?.path, SOURCE_WORKFLOW_PATH, 'run path')
  assertEqual(run?.event, SOURCE_RUN_EVENT, 'run event')
  assertEqual(run?.status, SOURCE_RUN_STATUS, 'run status')
  assertEqual(run?.conclusion, SOURCE_RUN_CONCLUSION, 'run conclusion')
  assertEqual(run?.head_sha, SOURCE_HEAD_SHA, 'run head SHA')
  return run
}

function validateArtifactInventory(inventory) {
  const expected = Object.values(TARGET_CONTRACTS).map(({ artifactName }) => artifactName).sort()
  if (!Number.isSafeInteger(inventory?.total_count)) throw new Error('artifact total_count is invalid')
  if (!Array.isArray(inventory?.artifacts)) throw new Error('artifact inventory is invalid')
  const actual = inventory.artifacts.map(({ name }) => name).sort()
  if (inventory.total_count !== expected.length || JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`source run must contain the exact artifact set ${JSON.stringify(expected)}`)
  }
  if (new Set(actual).size !== actual.length) throw new Error('source run artifact names must be unique')
  for (const artifact of inventory.artifacts) {
    if (!Number.isSafeInteger(artifact.id) || artifact.id <= 0) throw new Error(`artifact id is invalid: ${artifact.name}`)
    if (artifact.expired !== false) throw new Error(`artifact is expired: ${artifact.name}`)
  }
  return inventory
}

async function fetchJson(url, token) {
  const response = await fetch(url, {
    headers: {
      Accept: 'application/vnd.github+json',
      Authorization: `Bearer ${token}`,
      'X-GitHub-Api-Version': '2022-11-28'
    },
    redirect: 'error'
  })
  if (!response.ok) throw new Error(`GitHub API request failed with status ${response.status}`)
  return response.json()
}

async function verifyFixedSourceRun({ token = process.env.GITHUB_TOKEN } = {}) {
  if (typeof token !== 'string' || token.length === 0) throw new Error('GITHUB_TOKEN is required')
  const base = `https://api.github.com/repos/${SOURCE_REPOSITORY}/actions/runs/${SOURCE_RUN_ID}`
  const run = validateSourceRun(await fetchJson(base, token))
  const artifacts = validateArtifactInventory(await fetchJson(`${base}/artifacts?per_page=100`, token))
  return { run, artifacts }
}

function validateTargetMetadata(metadata, targetKey, contract, sourceRevision) {
  const expectedKeys = [
    'arch',
    'cargo',
    'cargoTarget',
    'features',
    'nativeDependency',
    'platform',
    'profile',
    'rustc',
    'schemaVersion',
    'sourceRevision',
    'targetKey'
  ]
  assertEqual(JSON.stringify(Object.keys(metadata).sort()), JSON.stringify(expectedKeys), `${targetKey} metadata keys`)
  assertEqual(metadata.schemaVersion, 2, `${targetKey} schemaVersion`)
  assertEqual(metadata.targetKey, targetKey, `${targetKey} targetKey`)
  assertEqual(metadata.platform, contract.platform, `${targetKey} platform`)
  assertEqual(metadata.arch, contract.arch, `${targetKey} arch`)
  assertEqual(metadata.cargoTarget, contract.cargoTarget, `${targetKey} cargoTarget`)
  assertEqual(metadata.profile, 'release', `${targetKey} profile`)
  assertEqual(JSON.stringify(metadata.features), '[]', `${targetKey} features`)
  assertEqual(metadata.sourceRevision, sourceRevision, `${targetKey} sourceRevision`)
  assertEqual(metadata.rustc, RUSTC_VERSION, `${targetKey} rustc`)
  assertEqual(metadata.cargo, CARGO_VERSION, `${targetKey} cargo`)
  assertEqual(
    JSON.stringify(Object.keys(metadata.nativeDependency ?? {}).sort()),
    JSON.stringify(['cargoLockSha256', 'napiManifestSha256']),
    `${targetKey} nativeDependency keys`
  )
  assertEqual(metadata.nativeDependency.cargoLockSha256, contract.cargoLockSha256, `${targetKey} Cargo.lock SHA-256`)
  assertEqual(metadata.nativeDependency.napiManifestSha256, contract.napiManifestSha256, `${targetKey} N-API manifest SHA-256`)
}

function validateArtifactDirectories({ artifactsRoot, contracts = TARGET_CONTRACTS, sourceRevision = SOURCE_REVISION }) {
  assertPlainDirectory(artifactsRoot, 'artifacts root')
  const expectedDirectories = Object.values(contracts).map(({ artifactName }) => artifactName).sort()
  assertEqual(JSON.stringify(normalizedEntries(artifactsRoot)), JSON.stringify(expectedDirectories), 'downloaded artifact directories')
  const records = {}
  for (const [targetKey, contract] of Object.entries(contracts)) {
    const artifactRoot = join(artifactsRoot, contract.artifactName)
    assertPlainDirectory(artifactRoot, `${targetKey} artifact directory`)
    const sidecarName = `${contract.addonName}.sha256`
    assertEqual(
      JSON.stringify(normalizedEntries(artifactRoot)),
      JSON.stringify([contract.addonName, sidecarName, 'target.json'].sort()),
      `${targetKey} artifact file closure`
    )
    const addonPath = join(artifactRoot, contract.addonName)
    const sidecarPath = join(artifactRoot, sidecarName)
    const metadataPath = join(artifactRoot, 'target.json')
    assertPlainFile(addonPath, `${targetKey} addon`)
    assertPlainFile(sidecarPath, `${targetKey} checksum sidecar`)
    assertPlainFile(metadataPath, `${targetKey} metadata`)
    const addonBytes = readFileSync(addonPath)
    const addonSha256 = hash('sha256', addonBytes)
    assertEqual(addonSha256, contract.sha256, `${targetKey} addon SHA-256`)
    assertEqual(readFileSync(sidecarPath, 'utf8'), `${contract.sha256}  ${contract.addonName}\n`, `${targetKey} sidecar text`)
    const metadata = readJson(metadataPath, `${targetKey} metadata`)
    validateTargetMetadata(metadata, targetKey, contract, sourceRevision)
    records[targetKey] = {
      addonName: contract.addonName,
      arch: contract.arch,
      cargo: CARGO_VERSION,
      cargoLockSha256: contract.cargoLockSha256,
      cargoTarget: contract.cargoTarget,
      napiManifestSha256: contract.napiManifestSha256,
      platform: contract.platform,
      rustc: RUSTC_VERSION,
      sha256: addonSha256
    }
  }
  return records
}

function safeTarPath(name) {
  const normalized = name.replaceAll('\\', '/')
  if (normalized.startsWith('/') || normalized.includes('\0')) throw new Error(`tarball contains unsafe path ${JSON.stringify(name)}`)
  const segments = normalized.split('/').filter((segment) => segment !== '')
  if (segments.includes('.') || segments.includes('..')) throw new Error(`tarball contains unsafe path ${JSON.stringify(name)}`)
  return segments.join('/')
}

function tarNumber(block, start, length, label) {
  const text = block.subarray(start, start + length).toString('ascii').replace(/\0.*$/, '').trim()
  if (!/^[0-7]+$/.test(text)) throw new Error(`tarball ${label} is invalid`)
  return Number.parseInt(text, 8)
}

function readTarFiles(tarballBytes) {
  const archive = gunzipSync(tarballBytes)
  const files = new Map()
  let offset = 0
  while (offset + 512 <= archive.length) {
    const header = archive.subarray(offset, offset + 512)
    if (header.every((byte) => byte === 0)) break
    const storedChecksum = tarNumber(header, 148, 8, 'header checksum')
    const checksumHeader = Buffer.from(header)
    checksumHeader.fill(0x20, 148, 156)
    const computedChecksum = checksumHeader.reduce((sum, byte) => sum + byte, 0)
    if (computedChecksum !== storedChecksum) throw new Error('tarball header checksum is invalid')
    const name = header.subarray(0, 100).toString('utf8').replace(/\0.*$/, '')
    const prefix = header.subarray(345, 500).toString('utf8').replace(/\0.*$/, '')
    const path = safeTarPath(prefix ? `${prefix}/${name}` : name)
    const size = tarNumber(header, 124, 12, 'entry size')
    const type = String.fromCharCode(header[156] || 0)
    const dataStart = offset + 512
    const dataEnd = dataStart + size
    if (dataEnd > archive.length) throw new Error('tarball entry exceeds archive bounds')
    if (type === '0' || type === '\0') {
      if (!path.startsWith('package/')) throw new Error(`tarball file is outside package root: ${path}`)
      const packagePath = path.slice('package/'.length)
      if (!packagePath || files.has(packagePath)) throw new Error(`tarball file path is invalid or duplicated: ${path}`)
      files.set(packagePath, Buffer.from(archive.subarray(dataStart, dataEnd)))
    } else if (type !== '5') {
      throw new Error(`tarball contains unsupported link or entry type ${JSON.stringify(type)} at ${path}`)
    }
    offset = dataStart + Math.ceil(size / 512) * 512
  }
  return files
}

function copyContainedFile(sourceRoot, relativePath, destination, label) {
  const source = assertContainedPlainFile(sourceRoot, relativePath, label)
  mkdirSync(dirname(destination), { recursive: true })
  copyFileSync(source, destination)
}

function parsePackOutput(stdout) {
  let parsed
  try {
    parsed = JSON.parse(stdout)
  } catch (error) {
    throw new Error('npm pack output is not valid JSON', { cause: error })
  }
  if (!Array.isArray(parsed) || parsed.length !== 1) throw new Error('npm pack must produce exactly one tarball record')
  return parsed[0]
}

async function assemblePackage({
  artifactsRoot,
  contracts = TARGET_CONTRACTS,
  execute = execFileSync,
  packageFileHashes = PACKAGE_FILE_HASHES,
  outputDirectory,
  packageRoot,
  sourceRevision = SOURCE_REVISION
}) {
  const resolvedArtifacts = resolve(artifactsRoot)
  const resolvedOutput = resolve(outputDirectory)
  const resolvedPackage = resolve(packageRoot)
  if (existsSync(resolvedOutput)) throw new Error('publication output directory must not already exist')
  assertPlainDirectory(resolvedArtifacts, 'artifacts root')
  assertPlainDirectory(resolvedPackage, 'package source')
  const physicalOutputParent = realpathSync(dirname(resolvedOutput))
  assertPlainDirectory(physicalOutputParent, 'publication output parent')
  const physicalArtifacts = realpathSync(resolvedArtifacts)
  const physicalPackage = realpathSync(resolvedPackage)
  const physicalOutput = join(physicalOutputParent, basename(resolvedOutput))
  for (const [left, right, label] of [
    [physicalArtifacts, physicalOutput, 'artifacts and output'],
    [physicalPackage, physicalOutput, 'package source and output']
  ]) {
    if (left === right || left.startsWith(`${right}${sep}`) || right.startsWith(`${left}${sep}`)) {
      throw new Error(`${label} directories must be physically disjoint`)
    }
  }
  const artifactRecords = validateArtifactDirectories({ artifactsRoot: resolvedArtifacts, contracts, sourceRevision })
  assertEqual(
    JSON.stringify(Object.keys(packageFileHashes).sort()),
    JSON.stringify(BASE_PACKAGE_FILES.slice().sort()),
    'package source hash keys'
  )
  const packageFileRecords = {}
  for (const [path, expectedHash] of Object.entries(packageFileHashes)) {
    const sourcePath = assertContainedPlainFile(resolvedPackage, path, `package source ${path}`)
    const actualHash = hash('sha256', readFileSync(sourcePath))
    assertEqual(actualHash, expectedHash, `package source hash ${path}`)
    packageFileRecords[path] = actualHash
  }
  const packageManifest = readJson(join(resolvedPackage, 'package.json'), 'package manifest')
  validatePackageManifest(packageManifest)

  mkdirSync(resolvedOutput, { recursive: false })
  const staging = join(resolvedOutput, 'package-staging')
  mkdirSync(staging)
  for (const path of BASE_PACKAGE_FILES) {
    copyContainedFile(resolvedPackage, path, join(staging, path), `package source ${path}`)
  }
  for (const contract of Object.values(contracts)) {
    const artifactRoot = join(resolvedArtifacts, contract.artifactName)
    copyContainedFile(
      artifactRoot,
      contract.addonName,
      join(staging, 'npm', contract.addonName),
      `artifact ${contract.addonName}`
    )
  }

  const stagedFiles = []
  const walk = (root, directory = root) => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const path = join(directory, entry.name)
      if (entry.isDirectory()) walk(root, path)
      else {
        assertPlainFile(path, `staged package entry ${relative(root, path)}`)
        stagedFiles.push(relative(root, path).split(sep).join('/'))
      }
    }
  }
  walk(staging)
  assertEqual(JSON.stringify(stagedFiles.sort()), JSON.stringify(expectedPackageFiles(contracts)), 'staged package allowlist')

  const hostLoad = execute(process.execPath, [
    '-e',
    `require(${JSON.stringify(join(staging, 'npm', 'index.cjs'))}); process.stdout.write('host-load-ok\\n')`
  ], { encoding: 'utf8' })
  assertEqual(hostLoad, 'host-load-ok\n', 'host Node addon load')

  const pack = parsePackOutput(execute('npm', [
    'pack',
    '--json',
    '--ignore-scripts',
    '--pack-destination',
    resolvedOutput,
    staging
  ], { encoding: 'utf8' }))
  assertEqual(pack.name, PACKAGE_NAME, 'npm pack name')
  assertEqual(pack.version, PACKAGE_VERSION, 'npm pack version')
  const tarballName = basename(pack.filename ?? '')
  if (!tarballName.endsWith('.tgz') || tarballName !== pack.filename) throw new Error('npm pack filename is invalid')
  const tarballPath = join(resolvedOutput, tarballName)
  assertPlainFile(tarballPath, 'npm tarball')
  const tarballBytes = readFileSync(tarballPath)
  const tarFiles = readTarFiles(tarballBytes)
  const expectedFiles = expectedPackageFiles(contracts)
  assertEqual(JSON.stringify([...tarFiles.keys()].sort()), JSON.stringify(expectedFiles), 'npm tarball allowlist')
  assertEqual(JSON.stringify((pack.files ?? []).map(({ path }) => path).sort()), JSON.stringify(expectedFiles), 'npm pack file manifest')
  assertEqual(pack.shasum, hash('sha1', tarballBytes), 'npm pack shasum')
  assertEqual(pack.integrity, `sha512-${hash('sha512', tarballBytes)}`, 'npm pack integrity')

  const packedFiles = [...tarFiles.entries()].map(([path, bytes]) => ({
    path,
    sha256: hash('sha256', bytes),
    size: bytes.length
  })).sort((left, right) => left.path.localeCompare(right.path))
  for (const contract of Object.values(contracts)) {
    assertEqual(
      packedFiles.find(({ path }) => path === `npm/${contract.addonName}`)?.sha256,
      contract.sha256,
      `packed addon ${contract.addonName}`
    )
  }
  const manifest = {
    schemaVersion: 1,
    package: {
      name: PACKAGE_NAME,
      registry: REGISTRY,
      repository: SOURCE_REPOSITORY,
      version: PACKAGE_VERSION
    },
    sourceRun: {
      event: SOURCE_RUN_EVENT,
      headSha: SOURCE_HEAD_SHA,
      id: SOURCE_RUN_ID,
      name: SOURCE_WORKFLOW_NAME,
      path: SOURCE_WORKFLOW_PATH,
      sourceRevision,
      status: SOURCE_RUN_STATUS,
      conclusion: SOURCE_RUN_CONCLUSION
    },
    artifacts: artifactRecords,
    packageFiles: packageFileRecords,
    tarball: {
      fileName: tarballName,
      path: tarballPath,
      integrity: pack.integrity,
      sha256: hash('sha256', tarballBytes),
      shasum: pack.shasum,
      size: tarballBytes.length
    },
    packedFiles
  }
  writeFileSync(join(resolvedOutput, 'publication-manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`)
  return manifest
}

function validatePublicationManifest(manifest) {
  assertEqual(manifest?.package?.name, PACKAGE_NAME, 'manifest package name')
  assertEqual(manifest?.package?.version, PACKAGE_VERSION, 'manifest package version')
  if (!SHA1.test(manifest?.tarball?.shasum ?? '')) throw new Error('manifest tarball shasum is invalid')
  if (!/^sha512-[A-Za-z0-9+/]+={0,2}$/.test(manifest?.tarball?.integrity ?? '')) throw new Error('manifest tarball integrity is invalid')
}

function determinePublicationAction({ manifest, registryMetadata }) {
  validatePublicationManifest(manifest)
  if (registryMetadata === null) return 'publish'
  assertEqual(registryMetadata?.dist?.shasum, manifest.tarball.shasum, 'registry dist shasum')
  assertEqual(registryMetadata?.dist?.integrity, manifest.tarball.integrity, 'registry dist integrity')
  return 'skip'
}

function verifyDelivery({ installedPackage, manifest, registryMetadata }) {
  determinePublicationAction({ manifest, registryMetadata })
  assertEqual(installedPackage?.name, PACKAGE_NAME, 'installed package name')
  assertEqual(installedPackage?.version, PACKAGE_VERSION, 'installed package version')
  return true
}

function parseArgs(argv, allowed) {
  const options = {}
  for (let index = 0; index < argv.length; index += 2) {
    const name = argv[index]
    const value = argv[index + 1]
    if (!allowed.includes(name) || value === undefined || value.startsWith('--')) {
      throw new Error(`unknown or incomplete option: ${name ?? ''}`)
    }
    if (options[name] !== undefined) throw new Error(`duplicate option: ${name}`)
    options[name] = value
  }
  return options
}

async function main(argv) {
  const [command, ...rest] = argv
  if (command === 'verify-source-run') {
    if (rest.length !== 0) throw new Error('verify-source-run accepts no options')
    await verifyFixedSourceRun()
    process.stdout.write('fixed-source-run-ok\n')
    return
  }
  if (command === 'assemble') {
    const options = parseArgs(rest, ['--artifacts-root', '--output', '--package-root'])
    if (Object.keys(options).length !== 3) throw new Error('assemble requires --artifacts-root, --output, and --package-root')
    const manifest = await assemblePackage({
      artifactsRoot: options['--artifacts-root'],
      outputDirectory: options['--output'],
      packageRoot: options['--package-root']
    })
    process.stdout.write(`${JSON.stringify({ manifest: join(resolve(options['--output']), 'publication-manifest.json'), tarball: join(resolve(options['--output']), manifest.tarball.fileName) })}\n`)
    return
  }
  if (command === 'publication-decision') {
    const options = parseArgs(rest, ['--manifest', '--registry-metadata'])
    if (Object.keys(options).length !== 2) throw new Error('publication-decision requires both metadata paths')
    const action = determinePublicationAction({
      manifest: readJson(options['--manifest'], 'publication manifest'),
      registryMetadata: readJson(options['--registry-metadata'], 'registry metadata')
    })
    process.stdout.write(`${action}\n`)
    return
  }
  if (command === 'verify-delivery') {
    const options = parseArgs(rest, ['--manifest', '--registry-metadata', '--installed-package'])
    if (Object.keys(options).length !== 3) throw new Error('verify-delivery requires all metadata paths')
    verifyDelivery({
      installedPackage: readJson(options['--installed-package'], 'installed package manifest'),
      manifest: readJson(options['--manifest'], 'publication manifest'),
      registryMetadata: readJson(options['--registry-metadata'], 'registry metadata')
    })
    process.stdout.write('registry-delivery-ok\n')
    return
  }
  throw new Error(`unknown command: ${command ?? ''}`)
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2)).catch((error) => {
    console.error(`[publish-native-package] ${error.message}`)
    process.exitCode = 1
  })
}

export {
  PACKAGE_NAME,
  PACKAGE_VERSION,
  SOURCE_REPOSITORY,
  SOURCE_REVISION,
  TARGET_CONTRACTS,
  assemblePackage,
  determinePublicationAction,
  validateArtifactDirectories,
  validateArtifactInventory,
  validateSourceRun,
  verifyDelivery,
  verifyFixedSourceRun
}
