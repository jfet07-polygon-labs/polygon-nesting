'use strict'

const path = require('node:path')
const { stagedAddonFileName } = require('./target.cjs')

const binaryName = stagedAddonFileName(process.platform, process.arch)
const binaryPath = path.join(__dirname, binaryName)

try {
  module.exports = require(binaryPath)
} catch (cause) {
  const causeMessage = cause && cause.message ? cause.message : String(cause)
  throw new Error(
    `polygon-nesting: failed to load native addon "${binaryName}" from ${binaryPath}.\n` +
      'For development, run `npm run build:release` from packages/polygon-nesting.\n' +
      'For packaged execution, include the matching unpacked .node file.\n' +
      `Original error: ${causeMessage}`,
    { cause }
  )
}
