import assert from 'node:assert/strict'
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'

import {
  readReleaseVersion,
  synchronizeReleaseVersion,
  validateReleaseVersion
} from './release-version.mjs'

function fixture(t, { cargoVersion = '1.2.3', npmVersion = '0.0.0', lockVersion = '0.0.0' } = {}) {
  const root = mkdtempSync(join(tmpdir(), 'polygon-release-version-'))
  mkdirSync(join(root, 'packages/polygon-nesting'), { recursive: true })
  writeFileSync(join(root, 'Cargo.toml'), `[workspace]\nmembers = []\n\n[workspace.package]\nversion = "${cargoVersion}"\nedition = "2024"\n`)
  writeFileSync(join(root, 'packages/polygon-nesting/package.json'), `${JSON.stringify({ name: '@scope/package', version: npmVersion }, null, 2)}\n`)
  writeFileSync(join(root, 'Cargo.lock'), `version = 4\n\n${['cli', 'core', 'napi', 'protocol'].map((name) => `[[package]]\nname = "polygon-nesting-${name}"\nversion = "${lockVersion}"\n`).join('\n')}\n[[package]]\nname = "dependency"\nversion = "9.9.9"\n`)
  t.after(() => rmSync(root, { recursive: true, force: true }))
  return root
}

test('reads the canonical release version from workspace package metadata', (t) => {
  const root = fixture(t)
  assert.equal(readReleaseVersion(root), '1.2.3')
})

test('rejects build metadata because release versions are OCI tag identities', (t) => {
  const root = fixture(t, { cargoVersion: '1.2.3+build.1' })
  assert.throws(() => readReleaseVersion(root), /missing or invalid/)
})

test('rejects npm and workspace lock metadata that drift from the canonical version', (t) => {
  const root = fixture(t)
  assert.throws(() => validateReleaseVersion(root), /package metadata version/)
})

test('synchronizes generated npm and workspace lock metadata from Cargo.toml', (t) => {
  const root = fixture(t)
  synchronizeReleaseVersion(root)
  assert.equal(JSON.parse(readFileSync(join(root, 'packages/polygon-nesting/package.json'), 'utf8')).version, '1.2.3')
  assert.match(readFileSync(join(root, 'Cargo.lock'), 'utf8'), /name = "polygon-nesting-core"\nversion = "1\.2\.3"/)
  assert.match(readFileSync(join(root, 'Cargo.lock'), 'utf8'), /name = "dependency"\nversion = "9\.9\.9"/)
  assert.equal(validateReleaseVersion(root), '1.2.3')
})
