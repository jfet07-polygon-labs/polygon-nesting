#!/usr/bin/env node
import { readFileSync, writeFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = dirname(dirname(fileURLToPath(import.meta.url)))
const SEMVER = /^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?$/
const WORKSPACE_PACKAGE = /^polygon-nesting-(?:cli|core|napi|protocol)$/

function paths(root) {
  return {
    cargoManifest: join(root, 'Cargo.toml'),
    cargoLock: join(root, 'Cargo.lock'),
    npmManifest: join(root, 'packages/polygon-nesting/package.json')
  }
}

export function readReleaseVersion(root = ROOT) {
  const manifest = readFileSync(paths(resolve(root)).cargoManifest, 'utf8')
  const workspacePackage = manifest.match(/\[workspace\.package\]([\s\S]*?)(?=\n\[|$)/)?.[1]
  const version = workspacePackage?.match(/^version\s*=\s*"([^"]+)"\s*$/m)?.[1]
  if (!version || !SEMVER.test(version)) throw new Error('Cargo.toml workspace package version is missing or invalid')
  return version
}

function workspaceLockPackages(lock) {
  return [...lock.matchAll(/\[\[package\]\]\n([\s\S]*?)(?=\n\[\[package\]\]|$)/g)].map((match) => {
    const body = match[1]
    return {
      name: body.match(/^name = "([^"]+)"$/m)?.[1],
      version: body.match(/^version = "([^"]+)"$/m)?.[1]
    }
  }).filter(({ name }) => WORKSPACE_PACKAGE.test(name ?? ''))
}

export function validateReleaseVersion(root = ROOT) {
  const resolvedRoot = resolve(root)
  const version = readReleaseVersion(resolvedRoot)
  const releasePaths = paths(resolvedRoot)
  const npmManifest = JSON.parse(readFileSync(releasePaths.npmManifest, 'utf8'))
  if (npmManifest.version !== version) throw new Error(`npm package metadata version must be ${version}`)
  const workspacePackages = workspaceLockPackages(readFileSync(releasePaths.cargoLock, 'utf8'))
  if (workspacePackages.length !== 4 || workspacePackages.some((item) => item.version !== version)) {
    throw new Error(`Cargo.lock workspace package metadata version must be ${version}`)
  }
  return version
}

export function synchronizeReleaseVersion(root = ROOT) {
  const resolvedRoot = resolve(root)
  const version = readReleaseVersion(resolvedRoot)
  const releasePaths = paths(resolvedRoot)
  const npmManifest = JSON.parse(readFileSync(releasePaths.npmManifest, 'utf8'))
  npmManifest.version = version
  writeFileSync(releasePaths.npmManifest, `${JSON.stringify(npmManifest, null, 2)}\n`)
  const lock = readFileSync(releasePaths.cargoLock, 'utf8').replace(
    /(\[\[package\]\]\nname = "polygon-nesting-(?:cli|core|napi|protocol)"\nversion = ")[^"]+("\n)/g,
    `$1${version}$2`
  )
  writeFileSync(releasePaths.cargoLock, lock)
  return validateReleaseVersion(resolvedRoot)
}

function main(argv) {
  if (argv.length === 0) {
    process.stdout.write(readReleaseVersion())
    return
  }
  if (argv.length === 1 && argv[0] === '--check') {
    process.stdout.write(validateReleaseVersion())
    return
  }
  if (argv.length === 1 && argv[0] === '--sync') {
    process.stdout.write(synchronizeReleaseVersion())
    return
  }
  throw new Error('usage: release-version.mjs [--check|--sync]')
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    main(process.argv.slice(2))
  } catch (error) {
    console.error(`[release-version] ${error.message}`)
    process.exitCode = 1
  }
}
