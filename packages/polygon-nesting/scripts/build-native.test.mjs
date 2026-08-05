import assert from 'node:assert/strict'
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { tmpdir } from 'node:os'
import { dirname, resolve } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'
import target from '../npm/target.cjs'
const PACKAGE_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const REPOSITORY_ROOT = resolve(PACKAGE_ROOT, '..', '..')
const buildNative = await import('./build-native.mjs')

function npmPackInvocation(packageRoot, platform = process.platform) {
  return platform === 'win32'
    ? {
        command: 'cmd.exe',
        args: ['/d', '/s', '/c', 'npm pack --dry-run --json'],
        options: { cwd: packageRoot, encoding: 'utf8' }
      }
    : {
        command: 'npm',
        args: ['pack', '--dry-run', '--json'],
        options: { cwd: packageRoot, encoding: 'utf8' }
      }
}

const TARGETS = [
  {
    platform: 'linux',
    arch: 'x64',
    cargoTarget: 'x86_64-unknown-linux-gnu',
    libraryFileName: 'libpolygon_nesting_napi.so'
  },
  {
    platform: 'win32',
    arch: 'x64',
    cargoTarget: 'x86_64-pc-windows-msvc',
    libraryFileName: 'polygon_nesting_napi.dll'
  },
  {
    platform: 'darwin',
    arch: 'arm64',
    cargoTarget: 'aarch64-apple-darwin',
    libraryFileName: 'libpolygon_nesting_napi.dylib'
  },
  {
    platform: 'darwin',
    arch: 'x64',
    cargoTarget: 'x86_64-apple-darwin',
    libraryFileName: 'libpolygon_nesting_napi.dylib'
  }
]

test('maps every supported deployment platform to its Cargo target', () => {
  for (const nativeTarget of TARGETS) {
    assert.deepEqual(target.resolveNativeTarget(nativeTarget.platform, nativeTarget.arch), nativeTarget)
  }
})

test('uses desktop-compatible platform and architecture addon filenames', () => {
  for (const nativeTarget of TARGETS) {
    assert.equal(
      target.stagedAddonFileName(nativeTarget.platform, nativeTarget.arch),
      `irregular-nesting-native.${nativeTarget.platform}-${nativeTarget.arch}.node`
    )
  }
})

test('resolves only the closed mapping by Cargo target triple', () => {
  for (const nativeTarget of TARGETS) {
    assert.deepEqual(target.resolveNativeTargetByCargoTarget(nativeTarget.cargoTarget), nativeTarget)
  }
  assert.throws(
    () => target.resolveNativeTargetByCargoTarget('aarch64-unknown-linux-gnu'),
    /unsupported Cargo target "aarch64-unknown-linux-gnu"/
  )
})

test('discovers release and development artifacts below target triples', () => {
  for (const nativeTarget of TARGETS) {
    assert.equal(
      target.artifactPathForTarget(REPOSITORY_ROOT, nativeTarget.platform, nativeTarget.arch, 'release'),
      resolve(REPOSITORY_ROOT, 'target', nativeTarget.cargoTarget, 'release', nativeTarget.libraryFileName)
    )
    assert.equal(
      target.artifactPathForTarget(REPOSITORY_ROOT, nativeTarget.platform, nativeTarget.arch, 'dev'),
      resolve(REPOSITORY_ROOT, 'target', nativeTarget.cargoTarget, 'debug', nativeTarget.libraryFileName)
    )
  }
})

test('passes explicit Cargo build arguments for every deployment target', () => {
  for (const nativeTarget of TARGETS) {
    assert.deepEqual(target.cargoBuildArgsForTarget(
      nativeTarget.platform,
      nativeTarget.arch,
      'release',
      resolve(REPOSITORY_ROOT, 'crates', 'polygon-nesting-napi', 'Cargo.toml')
    ), [
      'build',
      '--locked',
      '--manifest-path',
      resolve(REPOSITORY_ROOT, 'crates', 'polygon-nesting-napi', 'Cargo.toml'),
      '--release',
      '--target',
      nativeTarget.cargoTarget
    ])
  }
})

test('rejects unsupported targets before Cargo executes', () => {
  let cargoCalled = false
  assert.throws(
    () => buildNative.buildNative({ platform: 'linux', arch: 'arm64', execute: () => { cargoCalled = true } }),
    /unsupported native addon target "linux-arm64"/
  )
  assert.equal(cargoCalled, false)
})

test('builds from the standalone workspace and stages the mapped addon', () => {
  const workspaceRoot = mkdtempSync(resolve(tmpdir(), 'polygon-nesting-package-'))
  try {
    const nativeTarget = TARGETS[0]
    const packageRoot = resolve(workspaceRoot, 'packages', 'polygon-nesting')
    const sourcePath = target.artifactPathForTarget(workspaceRoot, nativeTarget.platform, nativeTarget.arch, 'release')
    const copied = []
    const commands = []
    buildNative.buildNative({
      packageRoot,
      workspaceRoot,
      platform: nativeTarget.platform,
      arch: nativeTarget.arch,
      execute(command, args, options) {
        commands.push({ command, args, options })
        mkdirSync(dirname(sourcePath), { recursive: true })
        writeFileSync(sourcePath, 'native addon')
      },
      fileSystem: {
        copyFile(source, destination) {
          copied.push({ source, destination })
        },
        exists(path) {
          return existsSync(path)
        },
        makeDirectory() {},
        remove(path) {
          rmSync(path, { force: true })
        },
        readFile(path) {
          return readFileSync(path)
        }
      }
    })
    assert.equal(commands.length, 1)
    assert.equal(commands[0].command, 'cargo')
    assert.deepEqual(commands[0].args, [
      'build',
      '--locked',
      '--manifest-path',
      resolve(workspaceRoot, 'crates', 'polygon-nesting-napi', 'Cargo.toml'),
      '--release',
      '--target',
      nativeTarget.cargoTarget
    ])
    assert.equal(commands[0].options.cwd, workspaceRoot)
    assert.equal(commands[0].options.stdio, 'inherit')
    assert.equal(commands[0].options.env.CARGO_TARGET_DIR, resolve(workspaceRoot, 'target'))
    assert.deepEqual(copied, [
      {
        source: sourcePath,
        destination: resolve(
          packageRoot,
          'npm',
          'irregular-nesting-native.linux-x64.node'
        )
      }
    ])
  } finally {
    rmSync(workspaceRoot, { force: true, recursive: true })
  }
})

test('removes stale artifacts before Cargo and leaves no staged addon after failed output', () => {
  const workspaceRoot = mkdtempSync(resolve(tmpdir(), 'polygon-nesting-stale-'))
  const packageRoot = resolve(workspaceRoot, 'packages', 'polygon-nesting')
  const nativeTarget = TARGETS[0]
  const sourcePath = target.artifactPathForTarget(workspaceRoot, nativeTarget.platform, nativeTarget.arch, 'release')
  const stagedPath = resolve(packageRoot, 'npm', target.stagedAddonFileName(nativeTarget.platform, nativeTarget.arch))
  try {
    mkdirSync(dirname(sourcePath), { recursive: true })
    mkdirSync(dirname(stagedPath), { recursive: true })
    writeFileSync(sourcePath, 'stale source')
    writeFileSync(stagedPath, 'stale staged addon')
    assert.throws(
      () => buildNative.buildNative({
        packageRoot,
        workspaceRoot,
        platform: nativeTarget.platform,
        arch: nativeTarget.arch,
        execute() {}
      }),
      /expected build artifact not found/
    )
    assert.equal(existsSync(sourcePath), false)
    assert.equal(existsSync(stagedPath), false)

    writeFileSync(stagedPath, 'stale staged addon')
    assert.throws(
      () => buildNative.buildNative({
        packageRoot,
        workspaceRoot,
        platform: nativeTarget.platform,
        arch: nativeTarget.arch,
        execute() { throw new Error('cargo failed') }
      }),
      /cargo failed/
    )
    assert.equal(existsSync(stagedPath), false)
  } finally {
    rmSync(workspaceRoot, { force: true, recursive: true })
  }
})

test('removes staged addon after every Darwin post-copy verification failure', () => {
  const failureStages = [
    ['sign', /codesign sign/],
    ['strict', /codesign strict verification/],
    ['metadata', /linker-signed/],
    ['child-load', /child-process addon load/]
  ]
  for (const [failureStage, expectedError] of failureStages) {
    const workspaceRoot = mkdtempSync(resolve(tmpdir(), 'polygon-nesting-darwin-failure-'))
    const packageRoot = resolve(workspaceRoot, 'packages', 'polygon-nesting')
    const nativeTarget = TARGETS.find(({ platform, arch }) => platform === 'darwin' && arch === 'arm64')
    const sourcePath = target.artifactPathForTarget(workspaceRoot, nativeTarget.platform, nativeTarget.arch, 'release')
    const stagedPath = resolve(packageRoot, 'npm', target.stagedAddonFileName(nativeTarget.platform, nativeTarget.arch))
    try {
      assert.throws(
        () => buildNative.buildNative({
          packageRoot,
          workspaceRoot,
          platform: nativeTarget.platform,
          arch: nativeTarget.arch,
          execute() {
            mkdirSync(dirname(sourcePath), { recursive: true })
            writeFileSync(sourcePath, 'new native addon')
          },
          verifyExecute(_command, args) {
            if (failureStage === 'sign' && args[0] === '--force') {
              return { signal: null, status: 1, stderr: 'sign failed' }
            }
            if (failureStage === 'strict' && args[0] === '--verify') {
              return { signal: null, status: 1, stderr: 'strict failed' }
            }
            if (failureStage === 'metadata' && args[0] === '-dvvv') {
              return { signal: null, status: 0, stderr: 'Signature=linker-signed' }
            }
            if (failureStage === 'child-load' && args[0] === '-e') {
              return { signal: null, status: 1, stderr: 'load failed', stdout: '' }
            }
            return { signal: null, status: 0, stderr: '', stdout: 'loaded' }
          }
        }),
        expectedError
      )
      assert.equal(existsSync(stagedPath), false)
    } finally {
      rmSync(workspaceRoot, { force: true, recursive: true })
    }
  }
})

test('accepts --release as the release-profile shorthand and rejects conflicts before Cargo', () => {
  assert.deepEqual(buildNative.parseArgs(['--release']), {
    profile: 'release',
    cargoTarget: undefined
  })
  let cargoCalled = false
  assert.throws(
    () => buildNative.runCli(['--release', '--profile', 'dev'], {
      execute: () => { cargoCalled = true }
    }),
    /conflicting --profile values "release" and "dev"/
  )
  assert.equal(cargoCalled, false)
})

test('supports --release as an exact shorthand for the release profile', () => {
  assert.deepEqual(buildNative.parseArgs(['--release']), buildNative.parseArgs(['--profile', 'release']))

  const nativeTarget = TARGETS[0]
  const execute = (calls) => (command, args, options) => {
    calls.push({ command, args, options })
    mkdirSync(dirname(target.artifactPathForTarget(REPOSITORY_ROOT, nativeTarget.platform, nativeTarget.arch, 'release')), { recursive: true })
  }
  const shorthandCalls = []
  const explicitCalls = []
  const fileSystem = {
    remove() {},
    exists() { return true },
    makeDirectory() {},
    copyFile() {}
  }
  buildNative.runCli(['--release'], {
    platform: nativeTarget.platform,
    arch: nativeTarget.arch,
    workspaceRoot: REPOSITORY_ROOT,
    execute: execute(shorthandCalls),
    fileSystem
  })
  buildNative.runCli(['--profile', 'release'], {
    platform: nativeTarget.platform,
    arch: nativeTarget.arch,
    workspaceRoot: REPOSITORY_ROOT,
    execute: execute(explicitCalls),
    fileSystem
  })
  assert.deepEqual(shorthandCalls, explicitCalls)
})

test('rejects conflicting and repeated release options before Cargo', () => {
  for (const argv of [
    ['--release', '--profile', 'dev'],
    ['--profile', 'dev', '--release']
  ]) {
    let cargoCalled = false
    assert.throws(
      () => buildNative.runCli(argv, { execute: () => { cargoCalled = true } }),
      /conflicting/
    )
    assert.equal(cargoCalled, false)
  }
  assert.deepEqual(buildNative.parseArgs(['--release', '--release']), { profile: 'release', cargoTarget: undefined })
})

test('rejects unknown, missing, empty, and conflicting build options before Cargo', () => {
  const invalidArguments = [
    ['--taret', 'x86_64-unknown-linux-gnu'],
    ['--target'],
    ['--target='],
    ['--profile'],
    ['--profile='],
    ['--target', 'x86_64-unknown-linux-gnu', '--target', 'aarch64-apple-darwin'],
    ['--profile', 'release', '--profile', 'dev']
  ]
  for (const argv of invalidArguments) {
    let cargoCalled = false
    assert.throws(
      () => buildNative.runCli(argv, { execute: () => { cargoCalled = true } }),
      /unknown option|requires a value|must not be empty|conflicting/
    )
    assert.equal(cargoCalled, false)
  }
})

test('signs Darwin addons and verifies the signature and child-process loading', () => {
  const calls = []
  buildNative.verifyDarwinAddon({
    addonPath: '/tmp/addon.node',
    execute(command, args, options) {
      calls.push({ command, args, options })
      if (args[0] === '-dvvv') {
        return { status: 0, stderr: 'Signature=adhoc\n' }
      }
      return { signal: null, status: 0, stderr: '', stdout: 'loaded' }
    },
    nodeExecutable: 'node'
  })
  assert.deepEqual(calls, [
    {
      command: 'codesign',
      args: ['--force', '--sign', '-', '/tmp/addon.node'],
      options: { stdio: 'inherit' }
    },
    {
      command: 'codesign',
      args: ['--verify', '--strict', '/tmp/addon.node'],
      options: { encoding: 'utf8' }
    },
    {
      command: 'codesign',
      args: ['-dvvv', '/tmp/addon.node'],
      options: { encoding: 'utf8' }
    },
    {
      command: 'node',
      args: ['-e', "require(\"/tmp/addon.node\"); process.stdout.write('loaded')"],
      options: { encoding: 'utf8' }
    }
  ])
})

test('Darwin builds, verifies, inspects, and loads the real staged addon', {
  skip: process.platform !== 'darwin'
}, () => {
  const nativeTarget = target.resolveNativeTarget(process.platform, process.arch)
  const build = spawnSync(process.execPath, [
    'scripts/build-native.mjs',
    '--profile',
    'release',
    '--target',
    nativeTarget.cargoTarget
  ], { cwd: PACKAGE_ROOT, encoding: 'utf8', timeout: 300_000 })
  assert.equal(build.error, undefined, build.stderr || build.stdout)
  assert.equal(build.signal, null, build.stderr || build.stdout)
  assert.equal(build.status, 0, build.stderr || build.stdout)

  const stagedPath = resolve(PACKAGE_ROOT, 'npm', target.stagedAddonFileName(process.platform, process.arch))
  const strict = spawnSync('codesign', ['--verify', '--strict', stagedPath], { encoding: 'utf8' })
  assert.equal(strict.status, 0, strict.stderr || strict.stdout)
  const metadata = spawnSync('codesign', ['-dvvv', stagedPath], { encoding: 'utf8' })
  assert.equal(metadata.status, 0, metadata.stderr || metadata.stdout)
  assert.doesNotMatch(metadata.stderr, /linker-signed/)
  const load = spawnSync(process.execPath, [
    '-e',
    "const addon = require('./npm/index.cjs'); const capability = addon.nativeCapability(); if (capability.apiVersion !== 3) process.exit(1); process.stdout.write(capability.targetTriple)"
  ], { cwd: PACKAGE_ROOT, encoding: 'utf8' })
  assert.equal(load.status, 0, load.stderr || load.stdout)
  assert.equal(load.stdout, nativeTarget.cargoTarget)
})

test('rejects failed Darwin signing and strict verification before loading', () => {
  for (const failure of [
    { stage: 'sign', result: { signal: null, status: 1, stderr: 'signing failed' } },
    { stage: 'strict', result: { signal: null, status: 1, stderr: 'strict verification failed' } },
    { stage: 'sign', result: { error: new Error('codesign unavailable'), signal: null, status: null } },
    { stage: 'strict', result: { signal: 'SIGTERM', status: null, stderr: '' } }
  ]) {
    const calls = []
    assert.throws(
      () => buildNative.verifyDarwinAddon({
        addonPath: '/tmp/addon.node',
        execute(command, args) {
          calls.push({ command, args })
          if (failure.stage === 'sign' && args[0] === '--force') return failure.result
          if (failure.stage === 'strict' && args[0] === '--verify') return failure.result
          return { signal: null, status: 0, stderr: '', stdout: 'loaded' }
        }
      }),
      /codesign sign|codesign strict verification/
    )
    assert.equal(calls.some(({ args }) => args[0] === '-dvvv'), false)
  }
})

test('rejects linker-signed Darwin addons', () => {
  assert.throws(
    () => buildNative.verifyDarwinAddon({
      addonPath: '/tmp/addon.node',
      execute(_command, args) {
        if (args[0] === '-dvvv') {
          return { status: 0, stderr: 'Signature=linker-signed\n' }
        }
        return { status: 0, stderr: '', stdout: '' }
      }
    }),
    /linker-signed/
  )
})

test('loader selects the staged host addon and preserves the native API exports', {
  skip: !existsSync(resolve(PACKAGE_ROOT, 'npm', target.stagedAddonFileName(process.platform, process.arch)))
}, async () => {
  const { default: native } = await import('../npm/index.cjs')
  const capability = native.nativeCapability()
  assert.equal(capability.apiVersion, 3)
  assert.equal(capability.targetTriple, target.resolveNativeTarget(process.platform, process.arch).cargoTarget)
  assert.equal(typeof native.runIrregularJob, 'function')
  assert.equal(typeof native.cancelIrregularJob, 'function')
  assert.equal(typeof native.getLastJobDiagnostics, 'function')
})

test('worker terminal lifecycle probe verifies the cross-thread barrier contract', {
  skip: !existsSync(resolve(PACKAGE_ROOT, 'npm', target.stagedAddonFileName(process.platform, process.arch)))
}, () => {
  buildNative.runWorkerTerminalLifecycleProbe()
})

test('package legal material has authoritative bytes and pinned LF endings', () => {
  const sha256 = (path) => createHash('sha256').update(readFileSync(path)).digest('hex')
  const legalFiles = [
    ['NOTICE', '1fa11aadfd5f98d734cbaced1fa10d525fd85565c560044734db4ce752037c1d'],
    ['LICENSES/clipper2-ts-BSL-1.0.txt', 'ea056d2c64294936b226f7360c265e77c52adc4ba171ee61029357f101f439cf']
  ]
  for (const [relativePath, expectedHash] of legalFiles) {
    const packagePath = resolve(PACKAGE_ROOT, relativePath)
    const authoritativePath = resolve(REPOSITORY_ROOT, relativePath)
    assert.equal(sha256(packagePath), expectedHash)
    assert.deepEqual(readFileSync(packagePath), readFileSync(authoritativePath))
  }
  const attributes = readFileSync(resolve(REPOSITORY_ROOT, '.gitattributes'), 'utf8')
  assert.match(attributes, /^packages\/polygon-nesting\/NOTICE text eol=lf$/m)
  assert.match(
    attributes,
    /^packages\/polygon-nesting\/LICENSES\/clipper2-ts-BSL-1\.0\.txt text eol=lf$/m
  )
})

test('vendored Clipper headers resolve to authoritative license bytes', () => {
  const authoritative = readFileSync(resolve(REPOSITORY_ROOT, 'LICENSES', 'clipper2-ts-BSL-1.0.txt'))
  for (const sourceFile of ['core.rs', 'engine.rs', 'offset.rs']) {
    const sourcePath = resolve(REPOSITORY_ROOT, 'crates', 'polygon-nesting-core', 'src', 'clipper', sourceFile)
    const header = readFileSync(sourcePath, 'utf8')
    const [, relativeLicensePath] = header.match(/Complete license text: (.+)/) ?? []
    assert.notEqual(relativeLicensePath, undefined)
    const licensePath = resolve(dirname(sourcePath), relativeLicensePath.trim())
    assert.equal(existsSync(licensePath), true)
    assert.deepEqual(readFileSync(licensePath), authoritative)
  }
})

test('package manifest publishes only the addon loader, binaries, and notices', () => {
  const manifest = JSON.parse(readFileSync(resolve(PACKAGE_ROOT, 'package.json'), 'utf8'))
  assert.equal(manifest.name, '@jfet97/polygon-nesting')
  assert.equal(manifest.private, false)
  assert.equal(manifest.publishConfig.registry, 'https://npm.pkg.github.com')
  assert.equal(manifest.main, 'npm/index.cjs')
  assert.equal(manifest.exports, './npm/index.cjs')
  assert.deepEqual(manifest.files, [
    'npm/index.cjs',
    'npm/target.cjs',
    'npm/*.node',
    'NOTICE',
    'LICENSES/**'
  ])
})

test('validates local package subsets and complete four-target release candidates', () => {
  const baseFiles = [
    'LICENSES/clipper2-ts-BSL-1.0.txt',
    'NOTICE',
    'npm/index.cjs',
    'npm/target.cjs',
    'package.json'
  ]
  const stagedFiles = TARGETS.map(({ platform, arch }) => `npm/irregular-nesting-native.${platform}-${arch}.node`)
  assert.doesNotThrow(() => buildNative.validatePackageContents([...baseFiles, stagedFiles[0]]))
  assert.throws(
    () => buildNative.validatePackageContents([...baseFiles, 'npm/unknown.node']),
    /unsupported staged addon/
  )
  assert.throws(
    () => buildNative.validatePackageContents([...baseFiles, stagedFiles[0]], { requireAllTargets: true }),
    /requires all four supported staged addons/
  )
  assert.doesNotThrow(() => buildNative.validatePackageContents([...baseFiles, ...stagedFiles], { requireAllTargets: true }))
})

test('bounds worker lifecycle probe execution and reports spawn failures', () => {
  assert.throws(
    () => buildNative.runWorkerTerminalLifecycleProbe({
      spawn: () => ({ error: new Error('timeout'), signal: 'SIGTERM', status: null, stderr: '', stdout: '' })
    }),
    /worker lifecycle probe failed to start/
  )
  assert.throws(
    () => buildNative.runWorkerTerminalLifecycleProbe({
      spawn: () => ({ signal: 'SIGTERM', status: null, stderr: '', stdout: '' })
    }),
    /worker lifecycle probe timed out or was terminated/
  )
})

test('uses an explicit command interpreter to run npm pack on Windows', () => {
  assert.deepEqual(npmPackInvocation(PACKAGE_ROOT, 'win32'), {
    command: 'cmd.exe',
    args: ['/d', '/s', '/c', 'npm pack --dry-run --json'],
    options: { cwd: PACKAGE_ROOT, encoding: 'utf8' }
  })
  assert.deepEqual(npmPackInvocation(PACKAGE_ROOT, 'linux'), {
    command: 'npm',
    args: ['pack', '--dry-run', '--json'],
    options: { cwd: PACKAGE_ROOT, encoding: 'utf8' }
  })
})

test('npm pack dry-run contains the allowlist without source or target leakage', () => {
  const { command, args, options } = npmPackInvocation(PACKAGE_ROOT)
  const packed = spawnSync(command, args, options)
  assert.equal(packed.status, 0, packed.stderr || packed.stdout)
  const [{ files }] = JSON.parse(packed.stdout)
  const names = files.map(({ path }) => path).sort()
  const stagedAddons = buildNative.validatePackageContents(names)
  assert.equal(stagedAddons.length >= 1, true)
  assert.equal(names.some((name) => name.startsWith('src/') || name.startsWith('target/')), false)
})
