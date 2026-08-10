import assert from 'node:assert/strict'
import test from 'node:test'

import {
  classifyNpmPublication,
  isExactNpmDuplicateVersionError,
  waitForNpmPublication
} from './npm-registry-publication.mjs'

const EXPECTED = {
  shasum: '0123456789abcdef0123456789abcdef01234567',
  integrity: 'sha512-expected'
}

const MATCHING_METADATA = {
  name: '@scope/package',
  version: '1.2.3',
  dist: { ...EXPECTED }
}

function npmError(code, message = code) {
  return Object.assign(new Error(message), { code })
}

test('polling retries E404 until matching metadata becomes visible', async () => {
  const responses = [npmError('E404'), MATCHING_METADATA]
  let attempts = 0
  const result = await waitForNpmPublication({
    expected: EXPECTED,
    maxAttempts: 3,
    poll: async () => {
      attempts += 1
      const response = responses.shift()
      if (response instanceof Error) throw response
      return response
    },
    sleep: async () => {}
  })

  assert.equal(result, 'skip')
  assert.equal(attempts, 2)
})

test('matching visible metadata is classified as skip', () => {
  assert.equal(classifyNpmPublication(MATCHING_METADATA, EXPECTED), 'skip')
})

test('polling fails immediately when visible metadata has different bytes', async () => {
  let attempts = 0
  await assert.rejects(
    waitForNpmPublication({
      expected: EXPECTED,
      maxAttempts: 3,
      poll: async () => {
        attempts += 1
        return {
          ...MATCHING_METADATA,
          dist: { ...EXPECTED, integrity: 'sha512-different' }
        }
      },
      sleep: async () => {}
    }),
    /refusing to accept an npm version with different bytes/
  )
  assert.equal(attempts, 1)
})

test('polling times out after the configured number of E404 responses', async () => {
  let attempts = 0
  await assert.rejects(
    waitForNpmPublication({
      expected: EXPECTED,
      maxAttempts: 3,
      poll: async () => {
        attempts += 1
        throw npmError('E404')
      },
      sleep: async () => {}
    }),
    /npm publication was not visible after 3 attempts/
  )
  assert.equal(attempts, 3)
})

test('polling fails immediately on errors other than E404', async () => {
  const forbidden = npmError('E403', 'authentication failed')
  let attempts = 0
  await assert.rejects(
    waitForNpmPublication({
      expected: EXPECTED,
      maxAttempts: 3,
      poll: async () => {
        attempts += 1
        throw forbidden
      },
      sleep: async () => {}
    }),
    (error) => error === forbidden
  )
  assert.equal(attempts, 1)
})

test('exact npm duplicate-version E403 is recoverable', () => {
  const error = {
    stderr: [
      'npm error code E403',
      'npm error 403 403 Forbidden - PUT https://registry.npmjs.org/@scope%2fpackage - You cannot publish over the previously published versions: 1.2.3.'
    ].join('\n')
  }

  assert.equal(isExactNpmDuplicateVersionError(error), true)
})

test('unrelated npm E403 is not recoverable', () => {
  const error = {
    stderr: [
      'npm error code E403',
      'npm error 403 403 Forbidden - PUT https://registry.npmjs.org/@scope%2fpackage - Access token expired or revoked.'
    ].join('\n')
  }

  assert.equal(isExactNpmDuplicateVersionError(error), false)
})
