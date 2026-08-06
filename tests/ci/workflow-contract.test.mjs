import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

const REPOSITORY_ROOT = dirname(dirname(dirname(fileURLToPath(import.meta.url))))
const ci = readFileSync(join(REPOSITORY_ROOT, '.github', 'workflows', 'ci.yml'), 'utf8')
const parity = readFileSync(join(REPOSITORY_ROOT, '.github', 'workflows', 'standalone-parity.yml'), 'utf8')
const release = readFileSync(join(REPOSITORY_ROOT, '.github', 'workflows', 'release.yml'), 'utf8')

const ACTION_PINS = [
  'actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683',
  'actions/setup-node@49933ea5288caeca8642d1e84afbd3f7d6820020',
  'dtolnay/rust-toolchain@4360b52568e2003a75bf9bc1d59f33a8e3fc893c',
  'actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02',
  'actions/download-artifact@d3f86a106a0bac45b974a628896c90dbdf5c8093',
  'actions/attest-build-provenance@96b4a1ef7235a096b17240c259729fdd70c83d45',
]

function assertPinnedActions(workflow) {
  assert.doesNotMatch(workflow, /uses:\s+[^\s@]+@(v\d+|main|master|latest)\b/)
  for (const pin of ACTION_PINS) {
    if (workflow.includes(pin)) assert.match(workflow, new RegExp(pin))
  }
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

test('Rust cache only stores immutable Cargo dependencies and enables cross-platform sccache', () => {
  const action = readFileSync(join(REPOSITORY_ROOT, '.github', 'actions', 'setup-rust-cache', 'action.yml'), 'utf8')
  assert.match(action, /path:\s*\|[\s\S]*\.cargo\/registry\/index[\s\S]*\.cargo\/registry\/cache[\s\S]*\.cargo\/git\/db/)
  assert.doesNotMatch(action, /(^|\n)\s*-?\s*target\//)
  assert.doesNotMatch(action, /\.node/)
  assert.doesNotMatch(action, /parity|evidence|artifact/i)
  assert.match(action, /RUSTC_WRAPPER=sccache/)
  assert.match(action, /SCCACHE_GHA_ENABLED=true/)
  assert.match(action, /CARGO_INCREMENTAL=0/)
  assert.match(action, /rust-cache-v1-/)
  assert.match(action, /runner\.os/)
  assert.match(action, /runner\.arch/)
  assert.match(action, /1\.95\.0/)
  assert.match(action, /hashFiles\('Cargo\.lock'\)/)
})

test('all workflow actions are pinned by reviewed commit SHA without changing release publication posture', () => {
  for (const workflow of [ci, parity, release]) assertPinnedActions(workflow)
  assert.match(release, /Publication remains disabled/)
  assert.match(release, /NODE_AUTH_TOKEN/)
})
