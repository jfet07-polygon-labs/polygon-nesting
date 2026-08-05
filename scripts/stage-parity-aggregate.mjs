#!/usr/bin/env node
import { createHash } from 'node:crypto';
import { copyFileSync, existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { gunzipSync } from 'node:zlib';
import { basename, dirname, join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';
import { PARITY_CONTRACT, stageTrustedAggregateParityTarget, verifyParityAggregate } from './parity-contract.mjs';

function fail(message) {
  throw new Error(`trusted parity aggregate ${message}`);
}

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

function requireRegularFile(path, description) {
  if (!existsSync(path)) fail(`${description} is missing: ${path}`);
  const stats = lstatSync(path);
  if (!stats.isFile() || stats.isSymbolicLink() || stats.nlink !== 1) fail(`${description} must be a regular non-linked file: ${path}`);
}

function archivePath(value) {
  if (!value || value.includes('\\') || value.startsWith('/') || /^[A-Za-z]:/.test(value) || value.split('/').some((part) => part === '..')) fail('archive contains an unsafe path');
  const clean = value.replace(/^\.\//, '');
  if (!clean || clean.split('/').some((part) => !part || part === '.')) return null;
  return clean;
}

function tarString(header, offset, length) {
  const end = header.indexOf(0, offset);
  return header.subarray(offset, end === -1 || end > offset + length ? offset + length : end).toString('utf8');
}

function tarSize(header) {
  const field = header.subarray(124, 136);
  const text = field.toString('ascii').replace(/[\0 ]+$/g, '');
  if (!/^[0-7]+$/.test(text)) fail('archive has a malformed octal size');
  const size = Number.parseInt(text, 8);
  if (!Number.isSafeInteger(size) || size < 0) fail('archive has an invalid size');
  return size;
}

function isZeroBlock(bytes) {
  return bytes.every((byte) => byte === 0);
}

function verifyTarChecksum(header) {
  const encoded = header.subarray(148, 156).toString('ascii').replace(/[\0 ]+$/g, '');
  if (!/^[0-7]+$/.test(encoded)) fail('archive has a malformed header checksum');
  const actual = Number.parseInt(encoded, 8);
  let expected = 0;
  for (let index = 0; index < 512; index += 1) expected += index >= 148 && index < 156 ? 32 : header[index];
  if (actual !== expected) fail('archive header checksum is invalid');
}

/** Parses a gzip USTAR stream before materializing any member. */
function parseSafeArchive(archivePath_) {
  let bytes;
  try {
    bytes = gunzipSync(readFileSync(archivePath_));
  } catch {
    fail('archive is not a valid gzip stream');
  }
  const entries = [];
  const paths = new Set();
  let offset = 0;
  let ended = false;
  let pendingLongName = null;
  while (offset < bytes.length) {
    if (offset + 512 > bytes.length) fail('archive has a truncated header');
    const header = bytes.subarray(offset, offset + 512);
    if (isZeroBlock(header)) {
      if (offset + 1024 > bytes.length || !isZeroBlock(bytes.subarray(offset + 512, offset + 1024))) fail('archive is missing terminal zero blocks');
      if (!isZeroBlock(bytes.subarray(offset + 1024))) fail('archive has data after terminal zero blocks');
      ended = true;
      break;
    }
    if (!['ustar\0', 'ustar '].includes(header.subarray(257, 263).toString('ascii'))) fail('archive is not a USTAR stream');
    verifyTarChecksum(header);
    const type = String.fromCharCode(header[156] || 0);
    const prefix = tarString(header, 345, 155);
    const name = tarString(header, 0, 100);
    const size = tarSize(header);
    const bodyStart = offset + 512;
    const bodyEnd = bodyStart + size;
    const next = bodyStart + Math.ceil(size / 512) * 512;
    if (bodyEnd > bytes.length || next > bytes.length) fail('archive has a truncated body or padding');
    if (type === 'L') {
      if (pendingLongName !== null || bytes.subarray(bodyStart, bodyEnd).at(-1) !== 0) fail('archive has an invalid GNU long-name entry');
      pendingLongName = bytes.subarray(bodyStart, bodyEnd - 1).toString('utf8');
      offset = next;
      continue;
    }
    if (type !== '\0' && type !== '0' && type !== '5') fail('archive contains a link or unsupported entry');
    const path = archivePath(pendingLongName ?? (prefix ? `${prefix}/${name}` : name));
    pendingLongName = null;
    if (path && paths.has(path)) fail('archive has duplicate paths');
    if (path) {
      paths.add(path);
      if (type === '5' && size !== 0) fail('archive directory has a body');
      entries.push({ path, type, bytes: bytes.subarray(bodyStart, bodyEnd) });
    }
    offset = next;
  }
  if (!ended) fail('archive is missing terminal zero blocks');
  return entries;
}

function extractSafeArchive(archivePath_, destination) {
  for (const entry of parseSafeArchive(archivePath_)) {
    const path = resolve(destination, entry.path);
    if (!path.startsWith(`${resolve(destination)}/`)) fail('archive path escapes extraction root');
    if (entry.type === '5') mkdirSync(path, { recursive: true });
    else {
      mkdirSync(dirname(path), { recursive: true });
      writeFileSync(path, entry.bytes, { flag: 'wx' });
    }
  }
}

function verifyArchiveDigest({ archivePath, digestPath }) {
  if (basename(archivePath) !== PARITY_CONTRACT.archiveName) fail('archive name is not accepted');
  if (basename(digestPath) !== PARITY_CONTRACT.archiveSha256Name) fail('archive digest sidecar name is not accepted');
  requireRegularFile(archivePath, 'archive');
  requireRegularFile(digestPath, 'archive digest sidecar');
  const expected = /^([a-f0-9]{64})  old-new-parity-bundle\.tar\.gz\n$/.exec(readFileSync(digestPath, 'utf8'));
  if (!expected) fail('archive digest sidecar format is invalid');
  if (sha256(readFileSync(archivePath)) !== expected[1]) fail('archive SHA-256 does not match its sidecar');
}

export function extractVerifiedParityAggregate({ archivePath, digestPath, sourceCommit, trustedSourceRoot }) {
  verifyArchiveDigest({ archivePath, digestPath });
  const extractionDirectory = mkdtempSync(join(tmpdir(), 'polygon-parity-aggregate-'));
  try {
    extractSafeArchive(archivePath, extractionDirectory);
    return { aggregateDirectory: extractionDirectory, verified: verifyParityAggregate({ aggregateDirectory: extractionDirectory, sourceCommit, trustedSourceRoot }), cleanup: () => rmSync(extractionDirectory, { force: true, recursive: true }) };
  } catch (error) {
    rmSync(extractionDirectory, { force: true, recursive: true });
    throw error;
  }
}

export function stageParityAggregateArchive({ archivePath, artifactDirectory, cargoTarget, digestPath, sourceCommit, targetKey, trustedSourceRoot }) {
  if (typeof trustedSourceRoot !== 'string' || !trustedSourceRoot) fail('trusted source root is required');
  verifyArchiveDigest({ archivePath, digestPath });
  const extractionDirectory = mkdtempSync(join(tmpdir(), 'polygon-parity-aggregate-'));
  try {
    extractSafeArchive(archivePath, extractionDirectory);
    stageTrustedAggregateParityTarget({ aggregateDirectory: extractionDirectory, artifactDirectory, cargoTarget, sourceCommit, targetKey, trustedSourceRoot });
    copyFileSync(archivePath, join(artifactDirectory, PARITY_CONTRACT.archiveName));
    copyFileSync(digestPath, join(artifactDirectory, PARITY_CONTRACT.archiveSha256Name));
  } finally {
    rmSync(extractionDirectory, { force: true, recursive: true });
  }
}

function parseArgs(argv) {
  const options = {};
  for (let index = 0; index < argv.length; index += 2) {
    const name = argv[index];
    const value = argv[index + 1];
    if (!['--archive', '--artifact-directory', '--cargo-target', '--digest', '--source-commit', '--target', '--trusted-source-root'].includes(name) || value === undefined) throw new Error(`expected a parity aggregate option and value, received ${name ?? '<end>'}`);
    if (name === '--archive') options.archivePath = value;
    if (name === '--artifact-directory') options.artifactDirectory = value;
    if (name === '--cargo-target') options.cargoTarget = value;
    if (name === '--digest') options.digestPath = value;
    if (name === '--source-commit') options.sourceCommit = value;
    if (name === '--target') options.targetKey = value;
    if (name === '--trusted-source-root') options.trustedSourceRoot = value;
  }
  for (const name of ['archivePath', 'artifactDirectory', 'cargoTarget', 'digestPath', 'sourceCommit', 'targetKey', 'trustedSourceRoot']) if (!options[name]) throw new Error(`${name} is required`);
  return options;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    stageParityAggregateArchive(parseArgs(process.argv.slice(2)));
  } catch (error) {
    console.error(`[stage-parity-aggregate] ${error.message}`);
    process.exitCode = 1;
  }
}
