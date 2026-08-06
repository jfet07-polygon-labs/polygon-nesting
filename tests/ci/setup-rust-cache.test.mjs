import assert from 'node:assert/strict'
import { dirname, join } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

const REPOSITORY_ROOT = dirname(dirname(dirname(fileURLToPath(import.meta.url))))
const { ensureSccache, isMainModule } = await import('../../scripts/ensure-sccache.mjs')

function result({ status = 0, stdout = '', stderr = '', error } = {}) {
  return { status, stdout, stderr, error, signal: null }
}

test('recognizes a Windows script path as the invoked main module', () => {
  assert.equal(
    isMainModule(
      'file:///D:/a/polygon-nesting/polygon-nesting/scripts/ensure-sccache.mjs',
      String.raw`D:\a\polygon-nesting\polygon-nesting\scripts\ensure-sccache.mjs`,
      'win32'
    ),
    true
  )
})

test('reinstalls pinned sccache when a preinstalled PATH executable has the wrong version', () => {
  const calls = []
  ensureSccache({
    bootstrapTargetDirectory: '/tmp/sccache-bootstrap',
    run(command, args, options) {
      calls.push({ command, args, options })
      if (command === 'sccache') {
        return result({ stdout: calls.filter(({ command: name }) => name === 'sccache').length === 1 ? 'sccache 0.9.0\n' : 'sccache 0.10.0\n' })
      }
      return result()
    }
  })
  assert.deepEqual(calls.map(({ command, args }) => [command, args]), [
    ['sccache', ['--version']],
    ['cargo', ['install', '--locked', '--version', '0.10.0', '--force', 'sccache']],
    ['sccache', ['--version']]
  ])
  assert.equal(calls[1].options.env.CARGO_TARGET_DIR, '/tmp/sccache-bootstrap')
})

test('rejects a mismatched sccache version after forced installation', () => {
  assert.throws(
    () => ensureSccache({
      bootstrapTargetDirectory: '/tmp/sccache-bootstrap',
      run(command) {
        return command === 'sccache' ? result({ stdout: 'sccache 0.9.0\n' }) : result()
      }
    }),
    /expected sccache 0\.10\.0/
  )
})

test('keeps an already installed exact sccache version without invoking Cargo', () => {
  const calls = []
  ensureSccache({
    run(command, args) {
      calls.push([command, args])
      return result({ stdout: 'sccache 0.10.0\n' })
    }
  })
  assert.deepEqual(calls, [['sccache', ['--version']]])
})

const CACHE_SERVICE_V2_ACTION_SHA = '5a3ec84eff668545956fd18022155c47e93e2684'
const RUNTIME_EXPORTER_ACTION_SHA = '60a0d83039c74a4aee543508d2ffcb1c3799cdea'
const OBSOLETE_CACHE_ACTION_SHA = '0c45773b623bea8c8e75f6c82b208c3cf94ea4f9'

function assertCacheServiceV2Action(action) {
  assert.match(
    action,
    new RegExp(`actions/cache@${CACHE_SERVICE_V2_ACTION_SHA}`),
    'actions/cache must pin the reviewed v4.2.3 cache service v2 commit'
  )
  assert.doesNotMatch(action, new RegExp(`actions/cache@${OBSOLETE_CACHE_ACTION_SHA}`))
}

function assertSccacheRuntimeCredentials(action) {
  assert.match(
    action,
    new RegExp(`actions/github-script@${RUNTIME_EXPORTER_ACTION_SHA}`),
    'runtime credentials must use the reviewed GitHub Script action commit'
  )
  assert.match(action, /const cacheUrl = process\.env\.ACTIONS_RESULTS_URL \|\| process\.env\.ACTIONS_CACHE_URL/)
  assert.match(action, /const runtimeToken = process\.env\.ACTIONS_RUNTIME_TOKEN/)
  assert.match(action, /if \(!cacheUrl \|\| !runtimeToken\) \{\n\s+core\.setFailed\('GitHub Actions cache runtime credentials are unavailable'\)\n\s+return\n\s+\}/)
  assert.match(action, /core\.exportVariable\('ACTIONS_RESULTS_URL', cacheUrl\)/)
  assert.match(action, /core\.exportVariable\('ACTIONS_RUNTIME_TOKEN', runtimeToken\)/)
  assert.match(action, /core\.exportVariable\('ACTIONS_CACHE_SERVICE_V2', process\.env\.ACTIONS_CACHE_SERVICE_V2 \|\| ''\)/)
  assert.match(action, /RUSTC_WRAPPER=sccache/)
  assert.match(action, /SCCACHE_GHA_ENABLED=true/)
  assert.ok(
    action.indexOf(`actions/github-script@${RUNTIME_EXPORTER_ACTION_SHA}`) < action.indexOf('- name: Configure sccache'),
    'runtime credentials must be exported before ordinary Rust run steps inherit sccache configuration'
  )
  assert.doesNotMatch(
    action,
    /(?:console\.\w+|core\.(?:debug|info|notice|warning|error))\([^)]*ACTIONS_(?:RESULTS_URL|CACHE_URL|RUNTIME_TOKEN)/,
    'runtime cache credentials must not be printed'
  )
}

test('workflow invokes the runtime sccache verifier at the pinned version', async () => {
  const action = await import('node:fs/promises').then(({ readFile }) => readFile(join(REPOSITORY_ROOT, '.github', 'actions', 'setup-rust-cache', 'action.yml'), 'utf8'))
  assert.match(action, /node scripts\/ensure-sccache\.mjs/)
  assert.match(action, /SCCACHE_VERSION:\s*0\.10\.0/)
  assert.doesNotMatch(action, /if ! command -v sccache/)
})

test('workflow pins the reviewed cache service v2 action identity', async () => {
  const action = await import('node:fs/promises').then(({ readFile }) => readFile(join(REPOSITORY_ROOT, '.github', 'actions', 'setup-rust-cache', 'action.yml'), 'utf8'))
  assertCacheServiceV2Action(action)
  assert.throws(
    () => assertCacheServiceV2Action(action.replace(CACHE_SERVICE_V2_ACTION_SHA, OBSOLETE_CACHE_ACTION_SHA)),
    /cache service v2/
  )
})

test('workflow exports non-secret GitHub Actions cache credentials for sccache 0.10.0', async () => {
  const action = await import('node:fs/promises').then(({ readFile }) => readFile(join(REPOSITORY_ROOT, '.github', 'actions', 'setup-rust-cache', 'action.yml'), 'utf8'))
  assertSccacheRuntimeCredentials(action)
  assert.throws(
    () => assertSccacheRuntimeCredentials(action.replace("core.exportVariable('ACTIONS_RUNTIME_TOKEN', runtimeToken)", '')),
    /ACTIONS_RUNTIME_TOKEN/
  )
})
