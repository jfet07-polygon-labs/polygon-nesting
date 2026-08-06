import assert from 'node:assert/strict'
import { dirname, join } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

const REPOSITORY_ROOT = dirname(dirname(dirname(fileURLToPath(import.meta.url))))
const { ensureSccache } = await import('../../scripts/ensure-sccache.mjs')

function result({ status = 0, stdout = '', stderr = '', error } = {}) {
  return { status, stdout, stderr, error, signal: null }
}

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

test('workflow invokes the runtime sccache verifier at the pinned version', async () => {
  const action = await import('node:fs/promises').then(({ readFile }) => readFile(join(REPOSITORY_ROOT, '.github', 'actions', 'setup-rust-cache', 'action.yml'), 'utf8'))
  assert.match(action, /node scripts\/ensure-sccache\.mjs/)
  assert.match(action, /SCCACHE_VERSION:\s*0\.10\.0/)
  assert.doesNotMatch(action, /if ! command -v sccache/)
})
