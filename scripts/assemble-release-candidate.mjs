#!/usr/bin/env node
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import target from '../packages/polygon-nesting/npm/target.cjs'
import { validatePackageContents } from '../packages/polygon-nesting/scripts/build-native.mjs'

const ROOT = dirname(dirname(fileURLToPath(import.meta.url)))
const PACKAGE_ROOT = join(ROOT, 'packages/polygon-nesting')
const hash = (path) => createHash('sha256').update(readFileSync(path)).digest('hex')
const stable = (value) => Array.isArray(value) ? value.map(stable) : value && typeof value === 'object' ? Object.fromEntries(Object.entries(value).sort(([a], [b]) => a.localeCompare(b)).map(([key, child]) => [key, stable(child)])) : value
const writeJson = (path, value) => writeFileSync(path, `${JSON.stringify(stable(value), null, 2)}\n`)
function json(path, label) { try { return JSON.parse(readFileSync(path, 'utf8')) } catch (error) { throw new Error(`${label} is not valid JSON: ${path}`, { cause: error }) } }
function equal(actual, expected, label) { if (actual !== expected) throw new Error(`${label} must be ${JSON.stringify(expected)}, received ${JSON.stringify(actual)}`) }
function exactKeys(value, keys, label) { if (!value || typeof value !== 'object' || Array.isArray(value) || JSON.stringify(Object.keys(value).sort()) !== JSON.stringify([...keys].sort())) throw new Error(`${label} schema is not accepted`) }
function committedHash(trustedSourceRoot, sourceCommit, path) { return createHash('sha256').update(execFileSync('git', ['-C', trustedSourceRoot, 'cat-file', 'blob', `${sourceCommit}:${path}`], { env: { ...process.env, GIT_NO_REPLACE_OBJECTS: '1' } })).digest('hex') }

export function validateTargetArtifact({ artifactsRoot, sourceCommit, targetKey, nativeTarget, trustedSourceRoot }) {
  const directory = join(artifactsRoot, targetKey)
  const metadataPath = join(directory, 'target.json')
  if (!existsSync(metadataPath)) throw new Error(`target metadata is missing for ${targetKey}`)
  const metadata = json(metadataPath, 'target metadata')
  exactKeys(metadata, ['arch', 'cargo', 'cargoTarget', 'features', 'nativeDependency', 'platform', 'profile', 'rustc', 'schemaVersion', 'sourceRevision', 'targetKey'], 'target metadata')
  for (const [key, value] of Object.entries({ schemaVersion: 2, targetKey, platform: nativeTarget.platform, arch: nativeTarget.arch, cargoTarget: nativeTarget.cargoTarget, profile: 'release', sourceRevision: sourceCommit })) equal(metadata[key], value, `target metadata ${key}`)
  equal(JSON.stringify(metadata.features), '[]', 'target metadata features')
  if (typeof metadata.rustc !== 'string' || !metadata.rustc.startsWith('rustc 1.95.0') || typeof metadata.cargo !== 'string' || !metadata.cargo.startsWith('cargo 1.95.0')) throw new Error(`target metadata toolchain identity is not pinned for ${targetKey}`)
  exactKeys(metadata.nativeDependency, ['cargoLockSha256', 'napiManifestSha256'], 'target metadata native dependency')
  if (!/^[a-f0-9]{64}$/.test(metadata.nativeDependency.cargoLockSha256) || !/^[a-f0-9]{64}$/.test(metadata.nativeDependency.napiManifestSha256)) throw new Error(`target metadata native dependency hash is invalid for ${targetKey}`)
  equal(metadata.nativeDependency.cargoLockSha256, committedHash(trustedSourceRoot, sourceCommit, 'Cargo.lock'), `target metadata Cargo.lock identity for ${targetKey}`)
  equal(metadata.nativeDependency.napiManifestSha256, committedHash(trustedSourceRoot, sourceCommit, 'crates/polygon-nesting-napi/Cargo.toml'), `target metadata N-API manifest identity for ${targetKey}`)
  const addonName = target.stagedAddonFileName(nativeTarget.platform, nativeTarget.arch)
  const addonPath = join(directory, addonName)
  const checksum = join(directory, `${addonName}.sha256`)
  if (!existsSync(addonPath) || !existsSync(checksum)) throw new Error(`native addon or checksum is missing for ${targetKey}`)
  equal(readFileSync(checksum, 'utf8'), `${hash(addonPath)}  ${addonName}\n`, `native addon checksum for ${targetKey}`)
  return { targetKey, platform: nativeTarget.platform, arch: nativeTarget.arch, cargoTarget: nativeTarget.cargoTarget, addonName, addonPath, sha256: hash(addonPath), buildMetadata: stable(metadata) }
}

function legal(packageRoot) { return { 'LICENSES/clipper2-ts-BSL-1.0.txt': hash(join(packageRoot, 'LICENSES/clipper2-ts-BSL-1.0.txt')), NOTICE: hash(join(packageRoot, 'NOTICE')) } }
export function renderReleaseNotes(release) { return ['# Polygon Nesting 0.1.1 Release Candidate', '', `Source commit: \`${release.sourceCommit}\``, `Package SHA-256: \`${release.tarball.sha256}\``, '', '## Native artifacts', ...release.nativeArtifacts.map((item) => `- \`${item.targetKey}\`: \`${item.cargoTarget}\`, addon \`${item.sha256}\``), '', 'The candidate is built from the current repository after the complete Rust, protocol, canonical matrix, native package, CLI, N-API, and OCI contract suites pass.', ''].join('\n') }
export async function assembleReleaseCandidate({ artifactsRoot, outputDirectory, packageRoot = PACKAGE_ROOT, sourceCommit, trustedSourceRoot, execute = execFileSync }) {
  if (!/^[a-f0-9]{40}$/.test(sourceCommit ?? '')) throw new Error('sourceCommit must be a full lowercase commit ID')
  if (!trustedSourceRoot) throw new Error('trustedSourceRoot is required')
  const artifacts = Object.entries(target.PUBLISHED_NATIVE_TARGETS).map(([targetKey, nativeTarget]) => validateTargetArtifact({ artifactsRoot, sourceCommit, targetKey, nativeTarget, trustedSourceRoot })).sort((a, b) => a.targetKey.localeCompare(b.targetKey))
  const npm = join(packageRoot, 'npm')
  mkdirSync(npm, { recursive: true })
  for (const name of readdirSync(npm)) if (name.endsWith('.node')) rmSync(join(npm, name), { force: true })
  for (const artifact of artifacts) copyFileSync(artifact.addonPath, join(npm, artifact.addonName))
  rmSync(outputDirectory, { recursive: true, force: true })
  mkdirSync(outputDirectory, { recursive: true })
  const records = JSON.parse(execute('npm', ['pack', '--json', '--pack-destination', resolve(outputDirectory)], { cwd: packageRoot, encoding: 'utf8' }))
  if (!Array.isArray(records) || records.length !== 1) throw new Error('npm pack must return exactly one record')
  const record = records[0]
  validatePackageContents(record.files.map(({ path }) => path).sort(), { requireAllTargets: true })
  writeJson(join(outputDirectory, 'npm-pack-manifest.json'), record)
  const packedFiles = record.files.map(({ path, size, mode }) => ({ path, size, mode, sha256: hash(join(packageRoot, path)) })).sort((left, right) => left.path.localeCompare(right.path))
  const release = { schemaVersion: 3, package: { name: record.name, version: record.version }, sourceCommit, legalHashes: legal(packageRoot), packedFiles, nativeArtifacts: artifacts.map(({ addonPath, ...artifact }) => artifact), packManifest: { fileName: 'npm-pack-manifest.json', sha256: hash(join(outputDirectory, 'npm-pack-manifest.json')) }, tarball: { fileName: record.filename, sha256: hash(join(outputDirectory, record.filename)) } }
  writeFileSync(join(outputDirectory, 'RELEASE_NOTES.md'), renderReleaseNotes(release))
  release.releaseNotes = { fileName: 'RELEASE_NOTES.md', sha256: hash(join(outputDirectory, 'RELEASE_NOTES.md')) }
  writeJson(join(outputDirectory, 'release.json'), release)
  return release
}
function parseArgs(argv) { const options = {}; for (let i = 0; i < argv.length; i += 2) { const key = argv[i]; const value = argv[i + 1]; if (!value || !['--artifacts', '--output', '--package-root', '--source-commit', '--trusted-source-root'].includes(key)) throw new Error(`unknown or incomplete option: ${key}`); options[{ '--artifacts': 'artifactsRoot', '--output': 'outputDirectory', '--package-root': 'packageRoot', '--source-commit': 'sourceCommit', '--trusted-source-root': 'trustedSourceRoot' }[key]] = value } options.sourceCommit ??= process.env.GITHUB_SHA; return options }
if (process.argv[1] === fileURLToPath(import.meta.url)) assembleReleaseCandidate(parseArgs(process.argv.slice(2))).catch((error) => { console.error(`[assemble-release-candidate] ${error.message}`); process.exitCode = 1 })
export { parseArgs, stable, writeJson }
