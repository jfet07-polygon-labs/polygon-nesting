import assert from 'node:assert/strict'
import { existsSync, readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

const ROOT = dirname(dirname(dirname(fileURLToPath(import.meta.url))))
const legacyWorkflow = join(ROOT, '.github/workflows/publish-native-package.yml')
const legacyScript = join(ROOT, 'scripts/publish-native-package.mjs')
const legacyScriptTest = join(ROOT, 'scripts/publish-native-package.test.mjs')
const publisher = readFileSync(join(ROOT, '.github/workflows/publish-runtime-image.yml'), 'utf8')
const npmrc = readFileSync(join(ROOT, '.npmrc'), 'utf8')
const migration = readFileSync(join(ROOT, 'docs/migration-from-min-plane-dfx.md'), 'utf8')

test('protected release publisher owns both GHCR and GitHub npm delivery', () => {
  assert.equal(existsSync(legacyWorkflow), false, 'the untrusted manual native package publisher must be removed')
  assert.equal(existsSync(legacyScript), false, 'the fixed-run package publisher script must be removed')
  assert.equal(existsSync(legacyScriptTest), false, 'the fixed-run package publisher tests must be removed')
  assert.match(publisher, /^on:\n  workflow_run:/m)
  assert.match(publisher, /^  packages: write$/m)
  assert.match(publisher, /^    environment: publish$/m)
  assert.match(publisher, /github\.event\.workflow_run\.head_branch == 'main'/)
})

test('npm publication consumes only the verified candidate tarball and is rerunnable', () => {
  assert.match(publisher, /NPM_TARBALL=.*resolve\('release-candidate',release\.tarball\.fileName\)/)
  assert.doesNotMatch(publisher, /NPM_TARBALL=.*join\('release-candidate',release\.tarball\.fileName\)/)
  assert.match(publisher, /npm publish "\$NPM_TARBALL" --ignore-scripts --registry https:\/\/npm\.pkg\.github\.com/)
  assert.match(publisher, /npm view "@jfet07-polygon-labs\/polygon-nesting@0\.1\.1" --json/)
  assert.match(publisher, /refusing to replace an existing npm version with different bytes/)
  assert.match(publisher, /if: steps\.npm-state\.outputs\.action == 'publish'/)
  assert.match(publisher, /published npm package bytes differ from the verified release tarball/)
  assert.match(publisher, /npm install --ignore-scripts --no-audit --no-fund --save-exact "@jfet07-polygon-labs\/polygon-nesting@0\.1\.1"/)
  assert.match(publisher, /readFileSync\(join\(process\.env\.DELIVERY_ROOT, 'node_modules\/\@jfet07-polygon-labs\/polygon-nesting\/package\.json'\), 'utf8'\)/)
  assert.match(publisher, /load\('@jfet07-polygon-labs\/polygon-nesting'\)/)
  assert.doesNotMatch(publisher, /load\('@jfet07-polygon-labs\/polygon-nesting\/package\.json'\)/)
  assert.doesNotMatch(publisher, /cargo build|npm run build:release/)
})

test('selected release source uses authenticated checkout without preserving credentials', () => {
  assert.doesNotMatch(publisher, /http\.extraheader=AUTHORIZATION: bearer/)
  assert.match(
    publisher,
    /- name: Checkout selected release source\n\s+uses: actions\/checkout@[a-f0-9]{40}\n\s+with:\n\s+ref: \$\{\{ steps\.source\.outputs\.source_commit \}\}\n\s+persist-credentials: false/
  )
})

test('both immutable destinations are inspected before either artifact is published', () => {
  const inspectNpm = publisher.indexOf('- name: Inspect immutable npm package version')
  const inspectOci = publisher.indexOf('- name: Inspect immutable runtime tag')
  const publishNpm = publisher.indexOf('- name: Publish exact verified npm tarball')
  const publishOci = publisher.indexOf('- name: Publish exact verified runtime image')
  assert.ok(inspectNpm >= 0 && inspectOci >= 0 && publishNpm >= 0 && publishOci >= 0)
  assert.ok(inspectNpm < publishNpm)
  assert.ok(inspectNpm < publishOci)
  assert.ok(inspectOci < publishNpm)
  assert.ok(inspectOci < publishOci)
})

test('repository npm configuration binds the organization scope without a committed credential', () => {
  assert.equal(npmrc, [
    '@jfet07-polygon-labs:registry=https://npm.pkg.github.com',
    '//npm.pkg.github.com/:_authToken=${NODE_AUTH_TOKEN}',
    ''
  ].join('\n'))
  assert.doesNotMatch(npmrc, /_authToken=(?!\$\{NODE_AUTH_TOKEN\})[^\n]+/)
})

test('migration docs define current-repository release gates without legacy parity', () => {
  assert.match(migration, /current repository/i)
  assert.match(migration, /two published native targets/i)
  assert.doesNotMatch(migration, /macOS x64|darwin-x64|x86_64-apple-darwin/i)
  assert.match(migration, /Windows.*local\/manual/i)
  assert.doesNotMatch(migration, /standard future release path remains parity-bound/i)
  assert.doesNotMatch(migration, /run `31109349775`/)
})
