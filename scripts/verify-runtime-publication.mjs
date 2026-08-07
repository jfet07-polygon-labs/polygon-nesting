#!/usr/bin/env node
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { lstatSync, readFileSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import { verifyReleaseCandidate } from './verify-release-candidate.mjs'

const SHA = /^[a-f0-9]{40}$/
const DIGEST = /^sha256:[a-f0-9]{64}$/
const RUN_ID = /^[1-9][0-9]*$/
const EVIDENCE_KEYS = [
  'archiveSha256',
  'immutableImageReference',
  'labels',
  'legalHashes',
  'manifestDigest',
  'nonRootSmoke',
  'platform',
  'schemaVersion',
  'sourceCommit'
]
const ROOT = resolve(join(fileURLToPath(new URL('.', import.meta.url)), '..'))

function sha256(path) {
  return createHash('sha256').update(readFileSync(path)).digest('hex')
}

function readJson(path, label) {
  try {
    return JSON.parse(readFileSync(path, 'utf8'))
  } catch (error) {
    throw new Error(`${label} is not valid JSON`, { cause: error })
  }
}

function assertRegularFile(path, label) {
  let stats
  try {
    stats = lstatSync(path)
  } catch (error) {
    throw new Error(`${label} must be a regular file`, { cause: error })
  }
  if (!stats.isFile() || stats.isSymbolicLink()) throw new Error(`${label} must be a regular file`)
}

function equal(actual, expected, label) {
  if (actual !== expected) throw new Error(`${label} must be ${JSON.stringify(expected)}, received ${JSON.stringify(actual)}`)
}

function exactKeys(value, expected, label) {
  if (!value || typeof value !== 'object' || Array.isArray(value) || JSON.stringify(Object.keys(value).sort()) !== JSON.stringify(expected.slice().sort())) {
    throw new Error(`${label} schema is not accepted`)
  }
}

export async function verifyRuntimePublication({
  candidateDirectory,
  trustedSourceRoot = ROOT,
  ociArchivePath,
  ociEvidencePath,
  manifestDigestPath,
  sourceCommit,
  sourceRunId,
  execute = execFileSync
}) {
  if (!SHA.test(sourceCommit ?? '')) throw new Error('sourceCommit must be a full lowercase commit ID')
  if (!RUN_ID.test(String(sourceRunId ?? ''))) throw new Error('sourceRunId must be a positive integer')
  if (!candidateDirectory || !trustedSourceRoot || !ociArchivePath || !ociEvidencePath || !manifestDigestPath) {
    throw new Error('candidateDirectory, trustedSourceRoot, ociArchivePath, ociEvidencePath, and manifestDigestPath are required')
  }

  const releasePath = join(resolve(candidateDirectory), 'release.json')
  assertRegularFile(releasePath, 'release metadata')
  assertRegularFile(ociArchivePath, 'OCI archive')
  assertRegularFile(ociEvidencePath, 'OCI evidence')
  assertRegularFile(manifestDigestPath, 'manifest digest sidecar')
  const release = readJson(releasePath, 'release metadata')
  equal(release.sourceCommit, sourceCommit, 'release sourceCommit')
  const evidence = readJson(ociEvidencePath, 'OCI evidence')
  exactKeys(evidence, EVIDENCE_KEYS, 'OCI evidence')
  equal(evidence.sourceCommit, sourceCommit, 'OCI evidence sourceCommit')
  equal(evidence.platform, 'linux/amd64', 'OCI platform')
  if (!DIGEST.test(evidence.manifestDigest ?? '')) throw new Error('OCI manifest digest is invalid')
  if (!/^[a-f0-9]{64}$/.test(evidence.archiveSha256 ?? '')) throw new Error('OCI archive SHA-256 is invalid')
  equal(readFileSync(manifestDigestPath, 'utf8'), `${evidence.manifestDigest}\n`, 'manifest digest sidecar')
  equal(sha256(ociArchivePath), evidence.archiveSha256, 'OCI archive SHA-256')

  const verifiedRelease = await verifyReleaseCandidate({
    candidateDirectory,
    trustedSourceRoot,
    ociArchivePath,
    ociEvidencePath,
    execute
  })
  equal(verifiedRelease.sourceCommit, sourceCommit, 'verified release sourceCommit')

  return {
    archiveSha256: evidence.archiveSha256,
    manifestDigest: evidence.manifestDigest,
    immutableImageReference: evidence.immutableImageReference,
    sourceCommit: evidence.sourceCommit,
    sourceRunId: String(sourceRunId)
  }
}

function parseArgs(argv) {
  const options = {}
  const names = new Map([
    ['--candidate', 'candidateDirectory'],
    ['--trusted-source-root', 'trustedSourceRoot'],
    ['--oci-archive', 'ociArchivePath'],
    ['--oci-evidence', 'ociEvidencePath'],
    ['--manifest-digest', 'manifestDigestPath'],
    ['--source-commit', 'sourceCommit'],
    ['--source-run-id', 'sourceRunId']
  ])
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index]
    const name = names.get(key)
    const value = argv[index + 1]
    if (!name || !value || value.startsWith('--') || options[name] !== undefined) throw new Error(`unknown or incomplete option: ${key ?? ''}`)
    options[name] = value
  }
  if (Object.keys(options).length !== names.size) throw new Error('all runtime publication verification options are required')
  return options
}

async function main(argv) {
  const result = await verifyRuntimePublication(parseArgs(argv))
  process.stdout.write(`${JSON.stringify(result)}\n`)
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2)).catch((error) => {
    console.error(`[verify-runtime-publication] ${error.message}`)
    process.exitCode = 1
  })
}

export { exactKeys, parseArgs, sha256 }
