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

test('protected release publisher owns GHCR, GitHub Packages, and npmjs delivery', () => {
  assert.equal(existsSync(legacyWorkflow), false, 'the untrusted manual native package publisher must be removed')
  assert.equal(existsSync(legacyScript), false, 'the fixed-run package publisher script must be removed')
  assert.equal(existsSync(legacyScriptTest), false, 'the fixed-run package publisher tests must be removed')
  assert.match(publisher, /^on:\n  workflow_run:/m)
  assert.match(publisher, /^  packages: write$/m)
  assert.match(publisher, /^    environment: publish$/m)
  assert.match(publisher, /github\.event\.workflow_run\.head_branch == 'main'/)
})

test('dual npm publication consumes only verified candidate tarballs and is rerunnable', () => {
  assert.match(publisher, /GITHUB_NPM_TARBALL=.*resolve\('release-candidate',githubPackage\.tarball\.fileName\)/)
  assert.match(publisher, /PUBLIC_NPM_TARBALL=.*resolve\('release-candidate',publicPackage\.tarball\.fileName\)/)
  assert.doesNotMatch(publisher, /NPM_TARBALL=.*join\('release-candidate'/)
  assert.match(publisher, /npm publish "\$GITHUB_NPM_TARBALL" --ignore-scripts --registry https:\/\/npm\.pkg\.github\.com/)
  assert.match(publisher, /npm publish "\$PUBLIC_NPM_TARBALL" --ignore-scripts --registry https:\/\/registry\.npmjs\.org/)
  assert.match(publisher, /npm view "@jfet07-polygon-labs\/polygon-nesting@0\.1\.2" --json --registry https:\/\/npm\.pkg\.github\.com/)
  assert.match(publisher, /npm view "@jfet97\/polygon-nesting@0\.1\.2" --json --registry https:\/\/registry\.npmjs\.org/)
  assert.match(publisher, /refusing to replace an existing npm version with different bytes/)
  assert.match(publisher, /if: steps\.github-npm-state\.outputs\.action == 'publish'/)
  assert.match(publisher, /if: steps\.public-npm-state\.outputs\.action == 'publish'/)
  assert.match(publisher, /published npm package bytes differ from the verified release tarball/)
  assert.match(publisher, /npm install --ignore-scripts --no-audit --no-fund --save-exact "@jfet07-polygon-labs\/polygon-nesting@0\.1\.2" --registry https:\/\/npm\.pkg\.github\.com/)
  assert.match(publisher, /npm install --ignore-scripts --no-audit --no-fund --save-exact "@jfet97\/polygon-nesting@0\.1\.2" --registry https:\/\/registry\.npmjs\.org/)
  assert.match(publisher, /load\('@jfet07-polygon-labs\/polygon-nesting'\)/)
  assert.match(publisher, /load\('@jfet97\/polygon-nesting'\)/)
  assert.doesNotMatch(publisher, /cargo build|npm run build:release/)
})

test('registry credentials are isolated to their destination commands', () => {
  const githubTokenUses = [...publisher.matchAll(/NODE_AUTH_TOKEN: \$\{\{ github\.token \}\}/g)]
  const npmTokenUses = [...publisher.matchAll(/NPM_TOKEN: \$\{\{ secrets\.NPM_TOKEN \}\}/g)]
  assert.ok(githubTokenUses.length >= 3)
  assert.equal(npmTokenUses.length, 1)
  assert.match(publisher, /printf '%s\\n' '\/\/registry\.npmjs\.org\/:_authToken=\$\{NPM_TOKEN\}' > "\$RUNNER_TEMP\/npmjs-publish\.npmrc"/)
  assert.match(publisher, /NPM_CONFIG_USERCONFIG="\$RUNNER_TEMP\/npmjs-publish\.npmrc" npm publish "\$PUBLIC_NPM_TARBALL"/)
  assert.doesNotMatch(publisher, /NODE_AUTH_TOKEN: \$\{\{ secrets\.NPM_TOKEN \}\}/)
  assert.doesNotMatch(publisher, /NPM_TOKEN: \$\{\{ github\.token \}\}/)
})

test('selected release source uses authenticated checkout without preserving credentials', () => {
  assert.doesNotMatch(publisher, /http\.extraheader=AUTHORIZATION: bearer/)
  assert.match(
    publisher,
    /- name: Checkout selected release source\n\s+uses: actions\/checkout@[a-f0-9]{40}\n\s+with:\n\s+ref: \$\{\{ steps\.source\.outputs\.source_commit \}\}\n\s+persist-credentials: false/
  )
})

test('all immutable destinations are inspected before any artifact is published', () => {
  const inspectGitHubNpm = publisher.indexOf('- name: Inspect immutable GitHub Packages version')
  const inspectPublicNpm = publisher.indexOf('- name: Inspect immutable npmjs version')
  const inspectOci = publisher.indexOf('- name: Inspect immutable runtime tag')
  const firstPublish = Math.min(
    publisher.indexOf('- name: Publish exact verified GitHub Packages tarball'),
    publisher.indexOf('- name: Publish exact verified npmjs tarball'),
    publisher.indexOf('- name: Publish exact verified runtime image')
  )
  assert.ok(inspectGitHubNpm >= 0 && inspectPublicNpm >= 0 && inspectOci >= 0 && firstPublish >= 0)
  assert.ok(inspectGitHubNpm < firstPublish)
  assert.ok(inspectPublicNpm < firstPublish)
  assert.ok(inspectOci < firstPublish)
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
