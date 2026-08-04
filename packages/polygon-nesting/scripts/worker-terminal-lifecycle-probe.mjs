#!/usr/bin/env node
/**
 * Exercises terminal delivery through the public addon API in a real Worker.
 * The caller supplies any outer harness deadline. This fixture deliberately
 * has no timeout because terminal acknowledgement is a strict production barrier.
 */

import { existsSync, readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { Worker, isMainThread, parentPort, workerData } from 'node:worker_threads'

const SCRIPT_PATH = fileURLToPath(import.meta.url)
const PACKAGE_ROOT = dirname(dirname(SCRIPT_PATH))
const REPOSITORY_ROOT = dirname(dirname(PACKAGE_ROOT))
const ADDON_ENTRY_PATH = join(PACKAGE_ROOT, 'npm', 'index.cjs')
const REQUEST_FIXTURE_PATH = join(
  REPOSITORY_ROOT,
  'tests',
  'fixtures',
  'mixed-61',
  '300x300-compact',
  'request.json'
)

const CONTROL = {
  startWorker: 0,
  terminalEntered: 1,
  releaseTerminal: 2,
  callbackReturned: 3,
  promiseSettled: 4
}

function assert(condition, message) {
  if (!condition) throw new Error(`assertion failed: ${message}`)
}

function loadAddon() {
  assert(existsSync(ADDON_ENTRY_PATH), `native addon entry missing: ${ADDON_ENTRY_PATH}`)
  const require = createRequire(import.meta.url)
  const addon = require(ADDON_ENTRY_PATH)
  const capability = addon.nativeCapability()
  assert(capability.apiVersion === 3, `expected apiVersion 3, received ${capability.apiVersion}`)
  return addon
}

function lifecycleRequest() {
  const fixture = JSON.parse(readFileSync(REQUEST_FIXTURE_PATH, 'utf8'))
  const pieces = fixture.pieces.slice(0, 1)
  const sourcePieceIds = new Set(pieces.map((piece) => piece.sourcePieceId))
  const sourcePieces = fixture.sourcePieces.filter((piece) => sourcePieceIds.has(piece.id))
  assert(sourcePieces.length === sourcePieceIds.size, 'fixture has source geometry for every piece')
  return { ...fixture, jobId: 'native-worker-terminal-lifecycle', pieces, sourcePieces }
}

function waitForWorkerMessage(worker, expectedKind) {
  return new Promise((resolve, reject) => {
    const cleanup = () => {
      worker.off('message', onMessage)
      worker.off('error', onError)
      worker.off('exit', onExit)
    }
    const onMessage = (message) => {
      cleanup()
      if (message?.kind === expectedKind) return resolve(message)
      if (message?.kind === 'native-promise-rejected') {
        return reject(new Error(`native promise rejected before ${expectedKind}: ${message.message}`))
      }
      reject(new Error(`expected worker message ${expectedKind}, received ${String(message?.kind)}`))
    }
    const onError = (error) => {
      cleanup()
      reject(error)
    }
    const onExit = (code) => {
      cleanup()
      reject(new Error(`worker exited before ${expectedKind}: ${code}`))
    }
    worker.once('message', onMessage)
    worker.once('error', onError)
    worker.once('exit', onExit)
  })
}

async function runTerminalBarrier({ report = true } = {}) {
  const controlBuffer = new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT * 5)
  const control = new Int32Array(controlBuffer)
  const worker = new Worker(SCRIPT_PATH, { workerData: { controlBuffer, request: lifecycleRequest() } })
  try {
    const entered = waitForWorkerMessage(worker, 'terminal-entered')
    Atomics.store(control, CONTROL.startWorker, 1)
    Atomics.notify(control, CONTROL.startWorker)
    const terminal = await entered
    assert(terminal.frames === 1, 'exactly one terminal frame reached the callback')
    assert(Atomics.load(control, CONTROL.callbackReturned) === 0, 'terminal callback remains blocked')
    assert(Atomics.load(control, CONTROL.promiseSettled) === 0, 'native promise remains pending')

    const settled = waitForWorkerMessage(worker, 'promise-settled')
    Atomics.store(control, CONTROL.releaseTerminal, 1)
    Atomics.notify(control, CONTROL.releaseTerminal)
    const completion = await settled
    assert(completion.frames === 1, 'exactly one terminal frame was delivered')
    assert(completion.registryReusable === true, 'worker registry lease was released after completion')
    assert(completion.diagnosticsObserved === true, 'worker-local diagnostics were available')
    assert(Atomics.load(control, CONTROL.callbackReturned) === 1, 'terminal callback returned')
    assert(Atomics.load(control, CONTROL.promiseSettled) === 1, 'native promise settled after callback return')
    if (report) {
      process.stdout.write('terminal-barrier-ok\nterminal-frames=1\nregistry-reusable=true\n')
    }
  } finally {
    await worker.terminate()
  }
}

async function runWorkerCleanup() {
  const controlBuffer = new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT * 5)
  const control = new Int32Array(controlBuffer)
  const worker = new Worker(SCRIPT_PATH, { workerData: { controlBuffer, request: lifecycleRequest() } })
  const entered = waitForWorkerMessage(worker, 'terminal-entered')
  Atomics.store(control, CONTROL.startWorker, 1)
  Atomics.notify(control, CONTROL.startWorker)
  const terminal = await entered
  assert(terminal.frames === 1, 'cleanup worker reached exactly one terminal frame')
  assert(Atomics.load(control, CONTROL.callbackReturned) === 0, 'cleanup worker terminal callback remains blocked')
  const exitCode = await worker.terminate()
  assert(Number.isInteger(exitCode), 'cleanup worker exits without deadlock')
  process.stdout.write('worker-cleanup-ok\n')
}

function runWorker() {
  const control = new Int32Array(workerData.controlBuffer)
  Atomics.wait(control, CONTROL.startWorker, 0)
  const addon = loadAddon()
  const invocationToken = 'native-worker-terminal-lifecycle-invocation-token'
  let terminalFrames = 0
  const promise = addon.runIrregularJob(
    JSON.stringify(workerData.request),
    invocationToken,
    (json) => {
      if (JSON.parse(json).kind !== 'terminal') return
      terminalFrames += 1
      Atomics.store(control, CONTROL.terminalEntered, 1)
      parentPort.postMessage({ kind: 'terminal-entered', frames: terminalFrames })
      Atomics.wait(control, CONTROL.releaseTerminal, 0)
      Atomics.store(control, CONTROL.callbackReturned, 1)
    },
    false
  )
  promise.then(
    () => {
      const diagnostics = JSON.parse(addon.getLastJobDiagnostics())
      Atomics.store(control, CONTROL.promiseSettled, 1)
      parentPort.postMessage({
        kind: 'promise-settled',
        frames: terminalFrames,
        registryReusable: addon.cancelIrregularJob(invocationToken, 'cancelled') === false,
        diagnosticsObserved: diagnostics !== null
      })
    },
    (error) => parentPort.postMessage({ kind: 'native-promise-rejected', message: String(error) })
  )
}

if (isMainThread) {
  /*
   * Addon diagnostics are process-local to the Worker environment and vanish
   * with forced Worker termination. A fresh Worker reusing the same token is
   * the observable cross-environment proof that cleanup removed the lease.
   */
  runTerminalBarrier()
    .then(runWorkerCleanup)
    .then(() => runTerminalBarrier({ report: false }))
    .then(() => process.stdout.write('worker-token-reuse-ok\n'))
    .catch((error) => {
    process.stderr.write(`[worker-terminal-lifecycle] FAILED: ${error.stack ?? error.message}\n`)
    process.exitCode = 1
  })
} else {
  runWorker()
}
