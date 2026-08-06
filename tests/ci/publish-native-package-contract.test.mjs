import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

const ROOT = dirname(dirname(dirname(fileURLToPath(import.meta.url))))
const workflow = readFileSync(join(ROOT, '.github/workflows/publish-native-package.yml'), 'utf8')
const npmrc = readFileSync(join(ROOT, '.npmrc'), 'utf8')
const migration = readFileSync(join(ROOT, 'docs/migration-from-min-plane-dfx.md'), 'utf8')

function remoteActionReferences(value) {
  return [...value.matchAll(/^\s*uses:\s+([^\s]+)\s*$/gm)]
    .map(([, reference]) => reference)
    .filter((reference) => !reference.startsWith('./'))
}

test('publication is a fixed manual workflow with minimal permissions and one Blacksmith job', () => {
  assert.match(workflow, /^name: Publish native package$/m)
  assert.match(workflow, /^on:\n  workflow_dispatch:\n$/m)
  assert.doesNotMatch(workflow, /inputs:/)
  assert.match(workflow, /^permissions:\n  actions: read\n  contents: read\n  packages: write\n$/m)
  assert.equal((workflow.match(/^  publish:\n/gm) ?? []).length, 1)
  assert.match(workflow, /^    runs-on: blacksmith-2vcpu-ubuntu-2404$/m)
  assert.doesNotMatch(workflow, /^    environment:/m)
})

test('publication pins the fixed source run and exact artifact names without rebuilding', () => {
  assert.match(workflow, /SOURCE_RUN_ID: "31109349775"/)
  assert.match(workflow, /SOURCE_HEAD_SHA: 92d51ba49c496ccd818646e9504bd042b2f73187/)
  for (const target of ['linux-x64', 'win32-x64', 'darwin-arm64', 'darwin-x64']) {
    assert.match(workflow, new RegExp(`name: native-build-${target}`))
  }
  assert.doesNotMatch(workflow, /cargo|rust-toolchain|build:release|assemble-release-candidate|release\.yml/i)
})

test('publication pins every action and publishes only the generated tarball path', () => {
  for (const reference of remoteActionReferences(workflow)) {
    assert.match(reference, /^[^@\s]+@[a-f0-9]{40}$/i)
  }
  assert.match(workflow, /NODE_AUTH_TOKEN: \$\{\{ secrets\.GITHUB_TOKEN \}\}/)
  assert.match(workflow, /manifest\.tarball\.path/)
  assert.match(workflow, /npm publish "\$TARBALL_PATH" --ignore-scripts --registry https:\/\/npm\.pkg\.github\.com/)
  assert.doesNotMatch(workflow, /npm publish\s+(?:\.|packages\/polygon-nesting|"?\$STAGING)/)
})

test('publication safely resumes after an exact version was already published', () => {
  const decisionStep = workflow.indexOf('id: publication-state')
  const publishStep = workflow.indexOf('npm publish "$TARBALL_PATH"')
  const deliveryStep = workflow.indexOf('Verify registry delivery and exact installation')
  assert.ok(decisionStep >= 0)
  assert.ok(decisionStep < publishStep)
  assert.ok(publishStep < deliveryStep)
  assert.match(workflow, /npm view "@jfet07-polygon-labs\/polygon-nesting@0\.1\.0" --json/)
  assert.match(workflow, /grep -q 'E404'/)
  assert.match(workflow, /publication-decision/)
  assert.match(workflow, /if: steps\.publication-state\.outputs\.action == 'publish'/)
  assert.doesNotMatch(workflow, /- name: Verify registry delivery and exact installation\n\s+if:/)
})

test('repository npm configuration binds the organization scope without a committed credential', () => {
  assert.equal(npmrc, [
    '@jfet07-polygon-labs:registry=https://npm.pkg.github.com',
    '//npm.pkg.github.com/:_authToken=${NODE_AUTH_TOKEN}',
    ''
  ].join('\n'))
  assert.doesNotMatch(npmrc, /_authToken=(?!\$\{NODE_AUTH_TOKEN\})[^\n]+/)
})

test('publication records and verifies tarball metadata before exact delivery installation', () => {
  assert.match(workflow, /publication-manifest\.json/)
  assert.match(workflow, /npm view "@jfet07-polygon-labs\/polygon-nesting@0\.1\.0" --json/)
  assert.match(workflow, /\(cd "\$DELIVERY_ROOT" && npm install --ignore-scripts --no-audit --no-fund --save-exact "@jfet07-polygon-labs\/polygon-nesting@0\.1\.0"\)/)
  assert.match(workflow, /node scripts\/publish-native-package\.mjs verify-delivery/)
  assert.match(workflow, /load\('@jfet07-polygon-labs\/polygon-nesting'\)/)
})

test('migration docs distinguish the authorized fast cutover from future parity-bound releases', () => {
  assert.match(migration, /one-time authorized fast package cutover/i)
  assert.match(migration, /run `31109349775`/)
  assert.match(migration, /does not run new parity/i)
  assert.match(migration, /standard future release path/i)
  assert.match(migration, /does not authorize removal of the embedded Rust engine/i)
})
