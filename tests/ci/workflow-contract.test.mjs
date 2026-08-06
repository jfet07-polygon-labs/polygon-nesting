import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

const REPOSITORY_ROOT = dirname(dirname(dirname(fileURLToPath(import.meta.url))))
const ci = readFileSync(join(REPOSITORY_ROOT, '.github', 'workflows', 'ci.yml'), 'utf8')
const parity = readFileSync(join(REPOSITORY_ROOT, '.github', 'workflows', 'standalone-parity.yml'), 'utf8')
const release = readFileSync(join(REPOSITORY_ROOT, '.github', 'workflows', 'release.yml'), 'utf8')
const rustCacheAction = readFileSync(join(REPOSITORY_ROOT, '.github', 'actions', 'setup-rust-cache', 'action.yml'), 'utf8')

function remoteActionReferences(workflow) {
  return [...workflow.matchAll(/^\s*uses:\s+([^\s]+)\s*$/gm)]
    .map(([, reference]) => reference)
    .filter((reference) => !reference.startsWith('./'))
}

function assertRemoteActionsPinned(workflow) {
  for (const reference of remoteActionReferences(workflow)) {
    assert.match(reference, /^[^@\s]+@[a-f0-9]{40}$/i, `remote action must use a full commit SHA: ${reference}`)
  }
}

function cachedCargoPaths(action) {
  const [, block] = action.match(/path:\s*\|\n((?:\s+[^\n]+\n)+?)\s+key:/) ?? []
  assert.notEqual(block, undefined, 'Cargo cache paths are required')
  return block.trim().split('\n').map((path) => path.trim())
}

function restorePrefixes(action) {
  const [, block] = action.match(/restore-keys:\s*\|\n((?: {10}[^\n]+\n?)+)/) ?? []
  assert.notEqual(block, undefined, 'Cargo cache restore prefix is required')
  return block.trim().split('\n').map((prefix) => prefix.trim())
}

function assertCargoCacheContract(action) {
  assert.deepEqual(cachedCargoPaths(action), [
    '~/.cargo/registry/index',
    '~/.cargo/registry/cache',
    '~/.cargo/git/db'
  ])
  assert.deepEqual(restorePrefixes(action), [
    'rust-cache-v1-${{ runner.os }}-${{ runner.arch }}-rust-1.95.0-'
  ])
  assert.match(action, /key:\s*rust-cache-v1-\$\{\{ runner\.os \}\}-\$\{\{ runner\.arch \}\}-rust-1\.95\.0-\$\{\{ hashFiles\('Cargo\.lock'\) \}\}/)
  assert.match(action, /node scripts\/ensure-sccache\.mjs/)
  assert.match(action, /SCCACHE_VERSION:\s*0\.10\.0/)
  assert.match(action, /RUSTC_WRAPPER=sccache/)
  assert.match(action, /SCCACHE_GHA_ENABLED=true/)
  assert.match(action, /CARGO_INCREMENTAL=0/)
}

test('CI only pushes main, tests workflow contracts, and preserves manual runs from cancellation', () => {
  assert.match(ci, /push:\n\s+branches:\n\s+- main/)
  assert.match(ci, /pull_request:/)
  assert.match(ci, /workflow_dispatch:/)
  assert.match(ci, /concurrency:\n\s+group:.*github\.event_name/s)
  assert.match(ci, /cancel-in-progress:\s*\$\{\{ github\.event_name != 'workflow_dispatch' \}\}/)
  assert.match(ci, /tests\/ci\/\*\.test\.mjs/)
})

test('Rust-producing CI jobs compile from empty runner-temp target roots and use the shared safe cache action', () => {
  for (const workflow of [ci, parity]) {
    assert.match(workflow, /\.\/\.github\/actions\/setup-rust-cache/)
    assert.match(workflow, /CARGO_TARGET_DIR:\s*\$\{\{ runner\.temp \}\}/)
    assert.match(workflow, /rm -rf "\$CARGO_TARGET_DIR"/)
    assert.match(workflow, /mkdir -p "\$CARGO_TARGET_DIR"/)
    assert.match(workflow, /test -z "\$\(find "\$CARGO_TARGET_DIR" -mindepth 1 -print -quit\)"/)
    assert.match(workflow, /--locked/)
  }
  assert.match(parity, /\$CARGO_TARGET_DIR\/\$\{\{ matrix\.target \}\}\/release\/polygon-nesting/)
  assert.match(parity, /\$CARGO_TARGET_DIR\/\$\{\{ matrix\.target \}\}\/release\/parity-desktop-request-adapter/)
})

test('Rust cache allowlist and restore prefix bind the complete dependency identity', () => {
  assertCargoCacheContract(rustCacheAction)
})

test('workflow contract rejects mutable action references, broad cache keys, and cached executable paths', () => {
  assert.throws(
    () => assertRemoteActionsPinned('uses: actions/checkout@stable\n'),
    /full commit SHA/
  )
  assert.throws(
    () => assertCargoCacheContract(rustCacheAction.replace(
      '~/.cargo/git/db',
      '~/.cargo/git/db\n          ~/.cargo/bin'
    )),
    /deep-equal|key/
  )
  assert.throws(
    () => assertCargoCacheContract(rustCacheAction.replace(
      'rust-cache-v1-${{ runner.os }}-${{ runner.arch }}-rust-1.95.0-',
      'rust-cache-v1-'
    )),
    /deepEqual|key/
  )
})

test('every remote action in workflows and the composite is pinned by a full commit SHA', () => {
  for (const workflow of [ci, parity, release, rustCacheAction]) assertRemoteActionsPinned(workflow)
  assert.match(release, /Publication remains disabled/)
  assert.match(release, /NODE_AUTH_TOKEN/)
})
