#!/usr/bin/env node
import { execFileSync } from 'node:child_process'
import { readFileSync, writeFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { setTimeout as defaultSleep } from 'node:timers/promises'

const DUPLICATE_VERSION_CODE = /^(?:npm (?:ERR!|error) )?code E403$/m
const DUPLICATE_VERSION_DETAIL = /^(?:npm (?:ERR!|error) )?403 403 Forbidden - PUT https:\/\/registry\.npmjs\.org\/\S+ - You cannot publish over the previously published versions: \S+\.$/m

export function classifyNpmPublication(metadata, expected) {
  if (metadata?.dist?.shasum === expected.shasum && metadata?.dist?.integrity === expected.integrity) return 'skip'
  throw new Error('refusing to accept an npm version with different bytes')
}

export function isExactNpmDuplicateVersionError(error) {
  const stderr = Buffer.isBuffer(error?.stderr) ? error.stderr.toString('utf8') : error?.stderr
  return typeof stderr === 'string'
    && DUPLICATE_VERSION_CODE.test(stderr)
    && DUPLICATE_VERSION_DETAIL.test(stderr)
}

export async function waitForNpmPublication({
  expected,
  maxAttempts,
  poll,
  pollIntervalMs = 1_000,
  sleep = defaultSleep
}) {
  for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
    try {
      return classifyNpmPublication(await poll(), expected)
    } catch (error) {
      if (error?.code !== 'E404') throw error
      if (attempt === maxAttempts) throw new Error(`npm publication was not visible after ${maxAttempts} attempts`, { cause: error })
      await sleep(pollIntervalMs)
    }
  }
}

function option(argv, name) {
  const index = argv.indexOf(name)
  if (index === -1 || !argv[index + 1]) throw new Error(`missing ${name}`)
  return argv[index + 1]
}

async function main(argv) {
  const command = argv[0]
  if (command === 'is-duplicate') {
    const stderr = readFileSync(option(argv, '--stderr'), 'utf8')
    if (!isExactNpmDuplicateVersionError({ stderr })) process.exitCode = 1
    return
  }
  if (command === 'wait') {
    const packageSpec = option(argv, '--package')
    const registry = option(argv, '--registry')
    const expected = JSON.parse(readFileSync(option(argv, '--expected'), 'utf8'))
    const output = option(argv, '--output')
    const maxAttempts = Number(option(argv, '--attempts'))
    const pollIntervalMs = Number(option(argv, '--interval-ms'))
    if (!Number.isSafeInteger(maxAttempts) || maxAttempts < 1 || !Number.isSafeInteger(pollIntervalMs) || pollIntervalMs < 0) {
      throw new Error('npm publication polling bounds are invalid')
    }
    let metadata
    await waitForNpmPublication({
      expected,
      maxAttempts,
      pollIntervalMs,
      poll: async () => {
        try {
          metadata = JSON.parse(execFileSync('npm', ['view', packageSpec, '--json', '--registry', registry], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }))
          return metadata
        } catch (error) {
          const stderr = Buffer.isBuffer(error.stderr) ? error.stderr.toString('utf8') : String(error.stderr ?? '')
          if (/\bE404\b/.test(stderr)) throw Object.assign(new Error('npm version is not visible'), { code: 'E404', cause: error })
          throw error
        }
      }
    })
    writeFileSync(output, `${JSON.stringify(metadata, null, 2)}\n`)
    return
  }
  throw new Error('usage: npm-registry-publication.mjs is-duplicate --stderr <path> | wait --package <spec> --registry <url> --expected <path> --output <path> --attempts <n> --interval-ms <n>')
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2)).catch((error) => {
    console.error(`[npm-registry-publication] ${error.message}`)
    process.exitCode = 1
  })
}
