'use strict'

const path = require('node:path')

const NATIVE_TARGETS = Object.freeze({
  'linux-x64': Object.freeze({
    platform: 'linux',
    arch: 'x64',
    cargoTarget: 'x86_64-unknown-linux-gnu',
    libraryFileName: 'libpolygon_nesting_napi.so'
  }),
  'win32-x64': Object.freeze({
    platform: 'win32',
    arch: 'x64',
    cargoTarget: 'x86_64-pc-windows-msvc',
    libraryFileName: 'polygon_nesting_napi.dll'
  }),
  'darwin-arm64': Object.freeze({
    platform: 'darwin',
    arch: 'arm64',
    cargoTarget: 'aarch64-apple-darwin',
    libraryFileName: 'libpolygon_nesting_napi.dylib'
  }),
  'darwin-x64': Object.freeze({
    platform: 'darwin',
    arch: 'x64',
    cargoTarget: 'x86_64-apple-darwin',
    libraryFileName: 'libpolygon_nesting_napi.dylib'
  })
})

function resolveNativeTarget(platform, arch) {
  const target = NATIVE_TARGETS[`${platform}-${arch}`]
  if (target === undefined) {
    throw new Error(`unsupported native addon target "${platform}-${arch}"`)
  }
  return target
}

function resolveNativeTargetByCargoTarget(cargoTarget) {
  const target = Object.values(NATIVE_TARGETS).find(
    (candidate) => candidate.cargoTarget === cargoTarget
  )
  if (target === undefined) {
    throw new Error(`unsupported Cargo target "${cargoTarget}"`)
  }
  return target
}

function stagedAddonFileName(platform, arch) {
  resolveNativeTarget(platform, arch)
  return `irregular-nesting-native.${platform}-${arch}.node`
}

function cargoBuildArgsForTarget(platform, arch, profile, manifestPath) {
  const { cargoTarget } = resolveNativeTarget(platform, arch)
  const profileArgs = profile === 'release' ? ['--release'] : profile === 'dev' ? [] : ['--profile', profile]
  const manifestArgs = manifestPath === undefined ? [] : ['--manifest-path', manifestPath]
  return ['build', '--locked', ...manifestArgs, ...profileArgs, '--target', cargoTarget]
}

function artifactPathForTarget(workspaceRoot, platform, arch, profile) {
  const { cargoTarget, libraryFileName } = resolveNativeTarget(platform, arch)
  const targetProfile = profile === 'dev' ? 'debug' : profile
  return path.join(workspaceRoot, 'target', cargoTarget, targetProfile, libraryFileName)
}

module.exports = {
  NATIVE_TARGETS,
  artifactPathForTarget,
  cargoBuildArgsForTarget,
  resolveNativeTarget,
  resolveNativeTargetByCargoTarget,
  stagedAddonFileName
}
