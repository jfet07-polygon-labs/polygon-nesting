import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import {
  linkSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync
} from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import test from 'node:test'

import {
  assemblePackage,
  determinePublicationAction,
  validateArtifactInventory,
  validateSourceRun,
  verifyDelivery
} from './publish-native-package.mjs'

const PACKAGE_NAME = '@jfet07-polygon-labs/polygon-nesting'
const VERSION = '0.1.0'
const SOURCE_REVISION = 'e6d62e6c751329ccd77292bbf2d175805a260032'
const RUSTC_VERSION = 'rustc 1.95.0 (59807616e 2026-04-14)'
const CARGO_VERSION = 'cargo 1.95.0 (f2d3ce0bd 2026-03-21)'
const TARGETS = Object.freeze({
  'linux-x64': Object.freeze({
    platform: 'linux',
    arch: 'x64',
    cargoTarget: 'x86_64-unknown-linux-gnu',
    addonName: 'irregular-nesting-native.linux-x64.node'
  }),
  'win32-x64': Object.freeze({
    platform: 'win32',
    arch: 'x64',
    cargoTarget: 'x86_64-pc-windows-msvc',
    addonName: 'irregular-nesting-native.win32-x64.node'
  }),
  'darwin-arm64': Object.freeze({
    platform: 'darwin',
    arch: 'arm64',
    cargoTarget: 'aarch64-apple-darwin',
    addonName: 'irregular-nesting-native.darwin-arm64.node'
  }),
  'darwin-x64': Object.freeze({
    platform: 'darwin',
    arch: 'x64',
    cargoTarget: 'x86_64-apple-darwin',
    addonName: 'irregular-nesting-native.darwin-x64.node'
  })
})

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex')
}

function executeWithStubbedHostLoad(command, args, options) {
  return command === process.execPath
    ? 'host-load-ok\n'
    : execFileSync(command, args, options)
}

function makeContracts() {
  return Object.fromEntries(Object.entries(TARGETS).map(([targetKey, target], index) => {
    const bytes = Buffer.from(`fixture-addon-${targetKey}`)
    return [targetKey, {
      ...target,
      artifactName: `native-build-${targetKey}`,
      sha256: sha256(bytes),
      cargoLockSha256: index === 1 ? 'b'.repeat(64) : 'a'.repeat(64),
      napiManifestSha256: index === 1 ? 'd'.repeat(64) : 'c'.repeat(64),
      bytes
    }]
  }))
}

function writeJson(path, value) {
  mkdirSync(dirname(path), { recursive: true })
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`)
}

function makeFixture(t) {
  const root = mkdtempSync(join(tmpdir(), 'polygon-publish-test-'))
  const artifactsRoot = join(root, 'artifacts')
  const packageRoot = join(root, 'package-source')
  const outputDirectory = join(root, 'output')
  const contracts = makeContracts()
  mkdirSync(join(packageRoot, 'npm'), { recursive: true })
  mkdirSync(join(packageRoot, 'LICENSES'), { recursive: true })
  writeJson(join(packageRoot, 'package.json'), {
    name: PACKAGE_NAME,
    version: VERSION,
    private: false,
    publishConfig: { registry: 'https://npm.pkg.github.com' },
    repository: {
      type: 'git',
      url: 'git+https://github.com/jfet07-polygon-labs/polygon-nesting.git'
    },
    main: 'npm/index.cjs',
    exports: './npm/index.cjs',
    files: ['npm/index.cjs', 'npm/target.cjs', 'npm/*.node', 'NOTICE', 'LICENSES/**'],
    scripts: { build: 'exit 99', prepack: 'exit 98', install: 'exit 97' }
  })
  writeFileSync(join(packageRoot, 'npm/index.cjs'), "'use strict'\nmodule.exports = require('./irregular-nesting-native.' + process.platform + '-' + process.arch + '.node')\n")
  writeFileSync(join(packageRoot, 'npm/target.cjs'), "'use strict'\nmodule.exports = {}\n")
  writeFileSync(join(packageRoot, 'NOTICE'), 'fixture notice\n')
  writeFileSync(join(packageRoot, 'LICENSES/clipper2-ts-BSL-1.0.txt'), 'fixture license\n')

  for (const [targetKey, contract] of Object.entries(contracts)) {
    const artifactRoot = join(artifactsRoot, contract.artifactName)
    mkdirSync(artifactRoot, { recursive: true })
    writeFileSync(join(artifactRoot, contract.addonName), contract.bytes)
    writeFileSync(join(artifactRoot, `${contract.addonName}.sha256`), `${contract.sha256}  ${contract.addonName}\n`)
    writeJson(join(artifactRoot, 'target.json'), {
      schemaVersion: 2,
      targetKey,
      platform: contract.platform,
      arch: contract.arch,
      cargoTarget: contract.cargoTarget,
      rustc: RUSTC_VERSION,
      cargo: CARGO_VERSION,
      profile: 'release',
      features: [],
      sourceRevision: SOURCE_REVISION,
      nativeDependency: {
        cargoLockSha256: contract.cargoLockSha256,
        napiManifestSha256: contract.napiManifestSha256
      }
    })
  }
  t.after(() => rmSync(root, { recursive: true, force: true }))
  return {
    artifactsRoot,
    contracts,
    packageFileHashes: Object.fromEntries([
      'LICENSES/clipper2-ts-BSL-1.0.txt',
      'NOTICE',
      'npm/index.cjs',
      'npm/target.cjs',
      'package.json'
    ].map((path) => [path, sha256(readFileSync(join(packageRoot, path)))])),
    outputDirectory,
    packageRoot,
    root
  }
}

function assemblyOptions(fixture, overrides = {}) {
  return {
    artifactsRoot: fixture.artifactsRoot,
    contracts: fixture.contracts,
    packageFileHashes: fixture.packageFileHashes,
    outputDirectory: fixture.outputDirectory,
    packageRoot: fixture.packageRoot,
    sourceRevision: SOURCE_REVISION,
    ...overrides
  }
}

test('accepts only the fixed successful CI pull-request run', () => {
  const run = {
    id: 31109349775,
    name: 'CI',
    path: '.github/workflows/ci.yml',
    event: 'pull_request',
    status: 'completed',
    conclusion: 'success',
    head_sha: '92d51ba49c496ccd818646e9504bd042b2f73187'
  }
  assert.doesNotThrow(() => validateSourceRun(run))
  for (const [field, value] of Object.entries({
    id: 1,
    name: 'Other',
    path: '.github/workflows/other.yml',
    event: 'workflow_dispatch',
    status: 'in_progress',
    conclusion: 'failure',
    head_sha: '0'.repeat(40)
  })) {
    assert.throws(() => validateSourceRun({ ...run, [field]: value }), new RegExp(field === 'head_sha' ? 'head SHA' : field))
  }
})

test('requires the exact unexpired four-artifact inventory with no extras', () => {
  const artifacts = Object.keys(TARGETS).map((targetKey, index) => ({
    id: index + 1,
    name: `native-build-${targetKey}`,
    expired: false
  }))
  assert.doesNotThrow(() => validateArtifactInventory({ total_count: 4, artifacts }))
  assert.throws(() => validateArtifactInventory({ total_count: 5, artifacts: [...artifacts, { id: 9, name: 'extra', expired: false }] }), /exact artifact set/)
  assert.throws(() => validateArtifactInventory({ total_count: 4, artifacts: artifacts.map((artifact, index) => index === 0 ? { ...artifact, expired: true } : artifact) }), /expired/)
})

test('publishes when the exact registry version is absent', () => {
  const manifest = {
    package: { name: PACKAGE_NAME, version: VERSION },
    tarball: { shasum: 'a'.repeat(40), integrity: `sha512-${Buffer.alloc(64).toString('base64')}` }
  }
  assert.equal(determinePublicationAction({ manifest, registryMetadata: null }), 'publish')
})

test('skips publishing when the exact registry version matches local bytes', () => {
  const manifest = {
    package: { name: PACKAGE_NAME, version: VERSION },
    tarball: { shasum: 'a'.repeat(40), integrity: `sha512-${Buffer.alloc(64).toString('base64')}` }
  }
  assert.equal(determinePublicationAction({
    manifest,
    registryMetadata: { dist: { shasum: manifest.tarball.shasum, integrity: manifest.tarball.integrity } }
  }), 'skip')
})

test('rejects an existing exact registry version with different bytes', () => {
  const manifest = {
    package: { name: PACKAGE_NAME, version: VERSION },
    tarball: { shasum: 'a'.repeat(40), integrity: `sha512-${Buffer.alloc(64).toString('base64')}` }
  }
  assert.throws(() => determinePublicationAction({
    manifest,
    registryMetadata: { dist: { shasum: 'b'.repeat(40), integrity: manifest.tarball.integrity } }
  }), /registry dist shasum/)
})

test('post-publish rerun changes from publish to matching-version skip', () => {
  const manifest = {
    package: { name: PACKAGE_NAME, version: VERSION },
    tarball: { shasum: 'a'.repeat(40), integrity: `sha512-${Buffer.alloc(64).toString('base64')}` }
  }
  assert.equal(determinePublicationAction({ manifest, registryMetadata: null }), 'publish')
  assert.equal(determinePublicationAction({
    manifest,
    registryMetadata: { dist: { shasum: manifest.tarball.shasum, integrity: manifest.tarball.integrity } }
  }), 'skip')
})

test('assembles one exact tarball while accepting target-specific dependency hashes', async (t) => {
  const fixture = makeFixture(t)
  const invocations = []
  const execute = (command, args, options) => {
    invocations.push({ command, args, options })
    if (command === process.execPath) return 'host-load-ok\n'
    return execFileSync(command, args, options)
  }
  const manifest = await assemblePackage(assemblyOptions(fixture, { execute }))
  const packCalls = invocations.filter(({ command, args }) => command === 'npm' && args[0] === 'pack')
  assert.equal(packCalls.length, 1)
  assert.deepEqual(packCalls[0].args.slice(0, 3), ['pack', '--json', '--ignore-scripts'])
  assert.equal(manifest.package.name, PACKAGE_NAME)
  assert.equal(manifest.package.version, VERSION)
  assert.match(manifest.tarball.fileName, /^jfet07-polygon-labs-polygon-nesting-0\.1\.0\.tgz$/)
  assert.equal(manifest.tarball.path, join(fixture.outputDirectory, manifest.tarball.fileName))
  assert.equal(manifest.tarball.sha256, sha256(readFileSync(manifest.tarball.path)))
  assert.equal(manifest.artifacts['win32-x64'].cargoLockSha256, 'b'.repeat(64))
  assert.equal(manifest.artifacts['linux-x64'].cargoLockSha256, 'a'.repeat(64))
  assert.deepEqual(manifest.packedFiles.map(({ path }) => path).sort(), [
    'LICENSES/clipper2-ts-BSL-1.0.txt',
    'NOTICE',
    'npm/index.cjs',
    'npm/irregular-nesting-native.darwin-arm64.node',
    'npm/irregular-nesting-native.darwin-x64.node',
    'npm/irregular-nesting-native.linux-x64.node',
    'npm/irregular-nesting-native.win32-x64.node',
    'npm/target.cjs',
    'package.json'
  ])
  assert.deepEqual(JSON.parse(readFileSync(join(fixture.outputDirectory, 'publication-manifest.json'), 'utf8')), manifest)
})

test('repeated assembly produces identical publication tarball identities', async (t) => {
  const fixture = makeFixture(t)
  const first = await assemblePackage(assemblyOptions(fixture, { execute: executeWithStubbedHostLoad }))
  const second = await assemblePackage(assemblyOptions(fixture, {
    execute: executeWithStubbedHostLoad,
    outputDirectory: join(fixture.root, 'repeat-output')
  }))
  assert.deepEqual(second.tarball, {
    ...first.tarball,
    path: join(fixture.root, 'repeat-output', first.tarball.fileName)
  })
})

test('rejects sidecar drift and symbolic-link artifacts before packing', async (t) => {
  const fixture = makeFixture(t)
  const linux = fixture.contracts['linux-x64']
  const artifactRoot = join(fixture.artifactsRoot, linux.artifactName)
  writeFileSync(join(artifactRoot, `${linux.addonName}.sha256`), `${linux.sha256} *${linux.addonName}\n`)
  await assert.rejects(assemblePackage(assemblyOptions(fixture, { execute: execFileSync })), /sidecar text/)

  writeFileSync(join(artifactRoot, `${linux.addonName}.sha256`), `${linux.sha256}  ${linux.addonName}\n`)
  rmSync(join(artifactRoot, linux.addonName))
  symlinkSync(join(fixture.root, 'outside.node'), join(artifactRoot, linux.addonName))
  writeFileSync(join(fixture.root, 'outside.node'), linux.bytes)
  await assert.rejects(assemblePackage(assemblyOptions(fixture, { execute: execFileSync })), /regular file/)

  rmSync(join(artifactRoot, linux.addonName))
  linkSync(join(fixture.root, 'outside.node'), join(artifactRoot, linux.addonName))
  await assert.rejects(assemblePackage(assemblyOptions(fixture, { execute: execFileSync })), /without links/)
})

test('rejects source files reached through symlinked package directories', async (t) => {
  const fixture = makeFixture(t)
  const outsideNpm = join(fixture.root, 'outside-npm')
  mkdirSync(outsideNpm)
  for (const fileName of ['index.cjs', 'target.cjs']) {
    writeFileSync(join(outsideNpm, fileName), readFileSync(join(fixture.packageRoot, 'npm', fileName)))
  }
  rmSync(join(fixture.packageRoot, 'npm'), { recursive: true })
  symlinkSync(outsideNpm, join(fixture.packageRoot, 'npm'))
  await assert.rejects(
    assemblePackage(assemblyOptions(fixture, { execute: execFileSync })),
    /symlinked path component|outside package source/
  )
})

test('rejects forged suffixes on exact Rust and Cargo toolchain identities', async (t) => {
  const rustcFixture = makeFixture(t)
  const rustcTargetPath = join(rustcFixture.artifactsRoot, 'native-build-linux-x64', 'target.json')
  const rustcTarget = JSON.parse(readFileSync(rustcTargetPath, 'utf8'))
  writeJson(rustcTargetPath, { ...rustcTarget, rustc: `${RUSTC_VERSION} forged` })
  await assert.rejects(
    assemblePackage(assemblyOptions(rustcFixture, { execute: executeWithStubbedHostLoad })),
    /rustc/
  )

  const cargoFixture = makeFixture(t)
  const cargoTargetPath = join(cargoFixture.artifactsRoot, 'native-build-linux-x64', 'target.json')
  const cargoTarget = JSON.parse(readFileSync(cargoTargetPath, 'utf8'))
  writeJson(cargoTargetPath, { ...cargoTarget, cargo: `${CARGO_VERSION} forged` })
  await assert.rejects(
    assemblePackage(assemblyOptions(cargoFixture, { execute: executeWithStubbedHostLoad })),
    /cargo/
  )
})

test('rejects target metadata and package identity drift before packing', async (t) => {
  const fixture = makeFixture(t)
  const targetPath = join(fixture.artifactsRoot, 'native-build-linux-x64', 'target.json')
  const target = JSON.parse(readFileSync(targetPath, 'utf8'))
  writeJson(targetPath, { ...target, profile: 'dev' })
  await assert.rejects(assemblePackage(assemblyOptions(fixture, { execute: execFileSync })), /profile/)

  writeJson(targetPath, target)
  const packagePath = join(fixture.packageRoot, 'package.json')
  const packageManifest = JSON.parse(readFileSync(packagePath, 'utf8'))
  writeJson(packagePath, { ...packageManifest, name: '@wrong/package' })
  await assert.rejects(assemblePackage(assemblyOptions(fixture, { execute: execFileSync })), /package source hash package\.json/)

  writeJson(packagePath, { ...packageManifest, repository: 'github:wrong/repository' })
  await assert.rejects(assemblePackage(assemblyOptions(fixture, { execute: execFileSync })), /package source hash package\.json/)
})

test('rejects fixed package-file drift and physical output aliases before packing', async (t) => {
  const fixture = makeFixture(t)
  writeFileSync(join(fixture.packageRoot, 'NOTICE'), 'mutated notice\n')
  await assert.rejects(
    assemblePackage(assemblyOptions(fixture, { execute: execFileSync })),
    /package source hash NOTICE/
  )

  writeFileSync(join(fixture.packageRoot, 'NOTICE'), 'fixture notice\n')
  writeFileSync(join(fixture.packageRoot, 'npm/index.cjs'), "'use strict'\nmodule.exports = {}\n")
  await assert.rejects(
    assemblePackage(assemblyOptions(fixture, { execute: execFileSync })),
    /package source hash npm\/index\.cjs/
  )

  writeFileSync(join(fixture.packageRoot, 'npm/index.cjs'), "'use strict'\nmodule.exports = require('./irregular-nesting-native.' + process.platform + '-' + process.arch + '.node')\n")
  const alias = join(fixture.root, 'root-alias')
  symlinkSync(fixture.root, alias)
  await assert.rejects(
    assemblePackage(assemblyOptions(fixture, {
      execute: execFileSync,
      outputDirectory: join(alias, 'package-source', 'nested-output')
    })),
    /physically disjoint/
  )
})

test('allows a symlinked temporary parent when the physical output remains disjoint', async (t) => {
  const fixture = makeFixture(t)
  const outputDirectory = `/tmp/polygon-publish-output-${process.pid}-${Date.now()}`
  t.after(() => rmSync(outputDirectory, { recursive: true, force: true }))
  await assemblePackage(assemblyOptions(fixture, {
    execute: executeWithStubbedHostLoad,
    outputDirectory
  }))
})

test('verifies registry metadata and the exact installed package identity', () => {
  const manifest = {
    package: { name: PACKAGE_NAME, version: VERSION },
    tarball: { shasum: 'a'.repeat(40), integrity: `sha512-${Buffer.alloc(64).toString('base64')}` }
  }
  assert.doesNotThrow(() => verifyDelivery({
    installedPackage: { name: PACKAGE_NAME, version: VERSION },
    manifest,
    registryMetadata: { dist: { shasum: manifest.tarball.shasum, integrity: manifest.tarball.integrity } }
  }))
  assert.throws(() => verifyDelivery({
    installedPackage: { name: PACKAGE_NAME, version: VERSION },
    manifest,
    registryMetadata: { dist: { shasum: 'b'.repeat(40), integrity: manifest.tarball.integrity } }
  }), /shasum/)
  assert.throws(() => verifyDelivery({
    installedPackage: { name: '@wrong/package', version: VERSION },
    manifest,
    registryMetadata: { dist: { shasum: manifest.tarball.shasum, integrity: manifest.tarball.integrity } }
  }), /installed package name/)
})
