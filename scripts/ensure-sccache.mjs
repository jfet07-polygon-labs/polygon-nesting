#!/usr/bin/env node
import { spawnSync } from 'node:child_process'

const SCCACHE_VERSION = '0.10.0'

function assertSuccessfulProcess(result, operation) {
  if (result?.error !== undefined) {
    throw new Error(`${operation} failed to start: ${result.error.message}`)
  }
  if (result?.signal != null) {
    throw new Error(`${operation} terminated by ${result.signal}`)
  }
  if (result?.status !== 0) {
    throw new Error(`${operation} exited with status ${String(result?.status)}`)
  }
}

function isExpectedSccacheVersion(result) {
  if (result?.error !== undefined || result?.signal != null || result?.status !== 0) return false
  return new RegExp(`^sccache ${SCCACHE_VERSION.replaceAll('.', '\\.')}(?:\\s|$)`, 'm')
    .test(`${result.stdout || ''}${result.stderr || ''}`)
}

function ensureSccache({
  bootstrapTargetDirectory,
  environment = process.env,
  run = spawnSync
} = {}) {
  if (environment.SCCACHE_VERSION !== undefined && environment.SCCACHE_VERSION !== SCCACHE_VERSION) {
    throw new Error(`SCCACHE_VERSION must be ${SCCACHE_VERSION}`)
  }

  const version = () => run('sccache', ['--version'], { encoding: 'utf8', env: environment })
  if (isExpectedSccacheVersion(version())) return
  if (bootstrapTargetDirectory === undefined) {
    throw new Error(`expected sccache ${SCCACHE_VERSION}; SCCACHE_BOOTSTRAP_TARGET_DIR is required to reinstall it`)
  }

  const installation = run('cargo', [
    'install', '--locked', '--version', SCCACHE_VERSION, '--force', 'sccache'
  ], {
    env: { ...environment, CARGO_TARGET_DIR: bootstrapTargetDirectory },
    stdio: 'inherit'
  })
  assertSuccessfulProcess(installation, 'cargo install sccache')
  if (!isExpectedSccacheVersion(version())) {
    throw new Error(`expected sccache ${SCCACHE_VERSION} after installation`)
  }
}

function main() {
  ensureSccache({
    bootstrapTargetDirectory: process.env.SCCACHE_BOOTSTRAP_TARGET_DIR
  })
}

if (import.meta.url === new URL(process.argv[1], 'file:').href) main()

export { SCCACHE_VERSION, ensureSccache }
