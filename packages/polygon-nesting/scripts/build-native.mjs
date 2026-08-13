#!/usr/bin/env node
import { execFileSync, spawnSync } from 'node:child_process'
import { copyFileSync, existsSync, mkdirSync, rmSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import target from '../npm/target.cjs'

const {
  artifactPathForTarget,
  cargoBuildArgsForTarget,
  cargoTargetDirectoryForWorkspace,
  resolveNativeTarget,
  resolveNativeTargetByCargoTarget,
  stagedAddonFileName
} = target
const PACKAGE_ROOT = dirname(dirname(fileURLToPath(import.meta.url)))
const WORKSPACE_ROOT = dirname(dirname(PACKAGE_ROOT))
const PROBE_TIMEOUT_MS = 120_000
const WORKER_PROBE_OUTPUT = 'terminal-barrier-ok\nterminal-frames=1\nregistry-reusable=true\nworker-cleanup-ok\nworker-token-reuse-ok\n'
const REQUIRED_PACKAGE_FILES = Object.freeze([
  'LICENSES/clipper2-ts-BSL-1.0.txt',
  'NOTICE',
  'npm/index.cjs',
  'npm/target.cjs',
  'package.json',
  'schemas/index.json',
  'schemas/cli/benchmark-report-v1.schema.json',
  'schemas/cli/engine-event-v1.schema.json',
  'schemas/cli/engine-outcome-v1.schema.json',
  'schemas/cli/engine-request-v1.schema.json',
  'schemas/cli/polygon-input-v1.schema.json',
  'schemas/napi/desktop-request-v1.schema.json',
  'schemas/napi/job-event-v3.schema.json',
  'schemas/napi/job-result-v3.schema.json',
  'schemas/napi/last-job-diagnostics-v1.schema.json',
  'schemas/napi/native-capability-v3.schema.json'
])
const STAGED_ADDON_FILES = Object.freeze(
  Object.values(target.NATIVE_TARGETS).map(({ platform, arch }) => `npm/${stagedAddonFileName(platform, arch)}`)
)
const PUBLISHED_STAGED_ADDON_FILES = Object.freeze(
  Object.values(target.PUBLISHED_NATIVE_TARGETS).map(({ platform, arch }) => `npm/${stagedAddonFileName(platform, arch)}`)
)

function parseArgs(argv) {
  let profile = 'release'
  let specifiedProfile
  let cargoTarget
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index]
    let name
    let value
    if (arg === '--release') {
      name = '--profile'
      value = 'release'
    } else if (arg === '--profile' || arg === '--target') {
      name = arg
      value = argv[index + 1]
      if (value === undefined || value.startsWith('--')) {
        throw new Error(`${name} requires a value`)
      }
      index += 1
    } else if (arg.startsWith('--profile=') || arg.startsWith('--target=')) {
      [name, value] = arg.split('=', 2)
      if (value === '') throw new Error(`${name} must not be empty`)
    } else {
      throw new Error(`unknown option "${arg}"`)
    }

    if (name === '--profile') {
      if (!['release', 'dev'].includes(value)) {
        throw new Error(`unsupported profile "${value}"`)
      }
      if (specifiedProfile !== undefined && specifiedProfile !== value) {
        throw new Error(`conflicting --profile values "${specifiedProfile}" and "${value}"`)
      }
      specifiedProfile = value
      profile = value
    } else {
      if (cargoTarget !== undefined && cargoTarget !== value) {
        throw new Error(`conflicting --target values "${cargoTarget}" and "${value}"`)
      }
      cargoTarget = value
    }
  }
  return { profile, cargoTarget }
}

function assertSuccessfulProcess(result, operation) {
  if (result?.error !== undefined) {
    throw new Error(`[build-native] ${operation} failed to start: ${result.error.message}`)
  }
  if (result?.signal != null) {
    throw new Error(`[build-native] ${operation} terminated by ${result.signal}`)
  }
  if (result?.status !== 0) {
    throw new Error(`[build-native] ${operation} exited with status ${String(result?.status)}`)
  }
}

function verifyDarwinAddon({ addonPath, execute = spawnSync, nodeExecutable = process.execPath }) {
  const signing = execute('codesign', ['--force', '--sign', '-', addonPath], { stdio: 'inherit' })
  assertSuccessfulProcess(signing, 'codesign sign')

  const strict = execute('codesign', ['--verify', '--strict', addonPath], { encoding: 'utf8' })
  assertSuccessfulProcess(strict, 'codesign strict verification')

  const signature = execute('codesign', ['-dvvv', addonPath], { encoding: 'utf8' })
  assertSuccessfulProcess(signature, 'codesign signature inspection')
  if (/linker-signed/.test(signature.stderr || signature.stdout || '')) {
    throw new Error(`[build-native] staged addon remains linker-signed: ${addonPath}`)
  }

  const load = execute(
    nodeExecutable,
    ['-e', `require(${JSON.stringify(addonPath)}); process.stdout.write('loaded')`],
    { encoding: 'utf8' }
  )
  assertSuccessfulProcess(load, 'child-process addon load')
  if (load.stdout !== 'loaded') {
    throw new Error(`[build-native] child process could not load staged addon: ${addonPath}`)
  }
}

function validatePackageContents(files, { requireAllTargets = false } = {}) {
  const names = new Set(files)
  const allowed = new Set([...REQUIRED_PACKAGE_FILES, ...STAGED_ADDON_FILES])
  for (const required of REQUIRED_PACKAGE_FILES) {
    if (!names.has(required)) throw new Error(`package is missing required file "${required}"`)
  }
  for (const name of names) {
    if (!allowed.has(name)) throw new Error(`package contains unsupported staged addon or file "${name}"`)
  }
  const stagedAddons = STAGED_ADDON_FILES.filter((name) => names.has(name))
  if (stagedAddons.length === 0) throw new Error('package requires a nonempty staged addon subset')
  if (requireAllTargets) {
    if (PUBLISHED_STAGED_ADDON_FILES.some((name) => !names.has(name))) {
      throw new Error('release candidate requires both published staged addons')
    }
    if (stagedAddons.length !== PUBLISHED_STAGED_ADDON_FILES.length) {
      throw new Error('release candidate must contain only the two published staged addons')
    }
  }
  return stagedAddons
}

function runWorkerTerminalLifecycleProbe({
  spawn = spawnSync,
  packageRoot = PACKAGE_ROOT,
  nodeExecutable = process.execPath
} = {}) {
  const probe = spawn(nodeExecutable, ['scripts/worker-terminal-lifecycle-probe.mjs'], {
    cwd: packageRoot,
    encoding: 'utf8',
    timeout: PROBE_TIMEOUT_MS
  })
  if (probe.error !== undefined) {
    throw new Error(`[build-native] worker lifecycle probe failed to start: ${probe.error.message}`)
  }
  if (probe.signal != null) {
    throw new Error(`[build-native] worker lifecycle probe timed out or was terminated by ${probe.signal}`)
  }
  if (probe.status !== 0) {
    throw new Error(`[build-native] worker lifecycle probe exited with status ${String(probe.status)}`)
  }
  if (probe.stdout !== WORKER_PROBE_OUTPUT) {
    throw new Error(`[build-native] worker lifecycle probe output was incomplete: ${probe.stderr || probe.stdout}`)
  }
  return probe
}

function buildNative({
  packageRoot = PACKAGE_ROOT,
  workspaceRoot = WORKSPACE_ROOT,
  platform = process.platform,
  arch = process.arch,
  profile = 'release',
  cargoTarget,
  cargoTargetDirectory = cargoTargetDirectoryForWorkspace(workspaceRoot),
  execute = execFileSync,
  verifyExecute = spawnSync,
  fileSystem = {
    copyFile: copyFileSync,
    exists: existsSync,
    makeDirectory: mkdirSync,
    remove: rmSync
  }
} = {}) {
  const nativeTarget = cargoTarget === undefined
    ? resolveNativeTarget(platform, arch)
    : resolveNativeTargetByCargoTarget(cargoTarget)
  const targetDirectory = cargoTargetDirectoryForWorkspace(workspaceRoot, {
    CARGO_TARGET_DIR: cargoTargetDirectory
  })
  const manifestPath = join(workspaceRoot, 'crates', 'polygon-nesting-napi', 'Cargo.toml')
  const sourcePath = artifactPathForTarget(
    workspaceRoot,
    nativeTarget.platform,
    nativeTarget.arch,
    profile,
    targetDirectory
  )
  const npmDirectory = join(packageRoot, 'npm')
  const addonPath = join(
    npmDirectory,
    stagedAddonFileName(nativeTarget.platform, nativeTarget.arch)
  )
  fileSystem.remove(sourcePath, { force: true })
  fileSystem.remove(addonPath, { force: true })

  const args = [
    ...cargoBuildArgsForTarget(
      nativeTarget.platform,
      nativeTarget.arch,
      profile,
      manifestPath
    )
  ]
  execute('cargo', args, {
    cwd: workspaceRoot,
    env: { ...process.env, CARGO_TARGET_DIR: targetDirectory },
    stdio: 'inherit'
  })
  if (!fileSystem.exists(sourcePath)) {
    throw new Error(`[build-native] expected build artifact not found at ${sourcePath}`)
  }

  fileSystem.makeDirectory(npmDirectory, { recursive: true })
  fileSystem.copyFile(sourcePath, addonPath)

  if (platform === 'darwin' && nativeTarget.platform === 'darwin') {
    try {
      verifyDarwinAddon({ addonPath, execute: verifyExecute })
    } catch (error) {
      fileSystem.remove(addonPath, { force: true })
      throw error
    }
  }
  return addonPath
}

function runCli(argv, options) {
  return buildNative({ ...options, ...parseArgs(argv) })
}

function main() {
  runCli(process.argv.slice(2))
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main()
}

export {
  buildNative,
  parseArgs,
  runCli,
  runWorkerTerminalLifecycleProbe,
  validatePackageContents,
  verifyDarwinAddon
}
