#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import {
  assertQualityGolden,
  extractQualityRow,
  makeQualityGolden,
  promoteQualityGolden,
  readQualityGolden
} from './canonical-quality.mjs'

const ROOT = dirname(dirname(fileURLToPath(import.meta.url)))
const QUALITY_GOLDEN = join(ROOT, 'tests', 'fixtures', 'canonical-quality-golden.json')

export const CURRENT_CANONICAL_ROW_IDS = Object.freeze([
  'triangle-20-2000x2700-compact', 'triangle-20-2000x2700-short-side', 'triangle-20-600x400-compact', 'triangle-20-600x400-short-side', 'triangle-20-300x300-compact', 'triangle-20-300x300-short-side',
  'mixed-61-2000x2700-compact', 'mixed-61-2000x2700-short-side', 'mixed-61-600x400-compact', 'mixed-61-600x400-short-side', 'mixed-61-300x300-compact', 'mixed-61-300x300-short-side',
  'shapes-17-2000x2700-compact', 'shapes-17-2000x2700-short-side', 'shapes-17-600x400-compact', 'shapes-17-600x400-short-side', 'shapes-17-300x300-compact', 'shapes-17-300x300-short-side'
])

function fail(message) {
  throw new Error(message)
}

function argument(name) {
  const index = process.argv.indexOf(name)
  if (index < 0 || !process.argv[index + 1]) fail(`${name} is required`)
  return resolve(process.argv[index + 1])
}

function run(command, args, options, label) {
  const result = spawnSync(command, args, {
    encoding: 'utf8',
    maxBuffer: 128 * 1024 * 1024,
    timeout: 10 * 60 * 1000,
    ...options
  })
  if (result.error) fail(`${label} failed to start: ${result.error.message}`)
  if (result.status !== 0) fail(`${label} exited ${result.status}: ${result.stderr}`)
  return result
}

function fixturePath(rowId) {
  const family = ['triangle-20', 'mixed-61', 'shapes-17'].find((candidate) => rowId.startsWith(`${candidate}-`))
  if (!family) fail(`canonical row family is invalid: ${rowId}`)
  return join(ROOT, 'tests', 'fixtures', family, rowId.slice(family.length + 1), 'request.json')
}

export function runCurrentCanonicalMatrix({ adapter, cli, updateGolden = false }) {
  const directory = mkdtempSync(join(tmpdir(), 'polygon-current-matrix-'))
  const qualityRows = {}
  try {
    for (const [ordinal, rowId] of CURRENT_CANONICAL_ROW_IDS.entries()) {
      const desktopRequest = JSON.parse(readFileSync(fixturePath(rowId), 'utf8'))
      desktopRequest.options.diagnosticTraceMode = 'full'
      const adapted = run(adapter, [], { input: `${JSON.stringify(desktopRequest)}\n` }, `adapter for ${rowId}`)
      if (adapted.stderr) fail(`adapter for ${rowId} wrote stderr: ${adapted.stderr}`)
      const input = join(directory, `${ordinal}-request.json`)
      const output = join(directory, `${ordinal}-result.json`)
      const events = join(directory, `${ordinal}-events.ndjson`)
      writeFileSync(input, adapted.stdout)
      run(cli, ['run', '--input', input, '--result-file', output, '--events', events], {}, `CLI for ${rowId}`)
      const outcome = JSON.parse(readFileSync(output, 'utf8'))
      if (outcome.version !== 1 || outcome.outcome?.status !== 'success') {
        fail(`CLI for ${rowId} did not produce a versioned success outcome`)
      }
      const frames = readFileSync(events, 'utf8').trim().split('\n').filter(Boolean).map((line) => JSON.parse(line))
      if (frames.length === 0 || frames.some((frame, index) => frame.ordinal !== index)) {
        fail(`CLI for ${rowId} did not produce ordered semantic events`)
      }
      qualityRows[rowId] = extractQualityRow(rowId, outcome.outcome.result)
      process.stdout.write(`${rowId}: ${qualityRows[rowId].placedCount} placed, ${qualityRows[rowId].unplacedCount} unplaced\n`)
    }
    const accepted = readQualityGolden(QUALITY_GOLDEN)
    const candidate = makeQualityGolden(qualityRows)
    if (updateGolden) {
      const evaluation = promoteQualityGolden(QUALITY_GOLDEN, accepted, candidate)
      process.stdout.write(`golden promoted with ${evaluation.improvements.length} improvements and ${evaluation.slightRegressions.length} slight regressions\n`)
    } else {
      assertQualityGolden(accepted, candidate)
      process.stdout.write('canonical quality golden: ok\n')
    }
  } finally {
    rmSync(directory, { force: true, recursive: true })
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    runCurrentCanonicalMatrix({
      adapter: argument('--adapter'),
      cli: argument('--cli'),
      updateGolden: process.argv.includes('--update-golden')
    })
  } catch (error) {
    console.error(`[run-current-canonical-matrix] ${error.message}`)
    process.exitCode = 1
  }
}
