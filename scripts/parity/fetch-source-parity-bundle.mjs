#!/usr/bin/env node
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { mkdir, readdir, readFile, rm, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { gunzipSync } from 'node:zlib';
import { spawnSync } from 'node:child_process';

import {
  ACCEPTED_OLD_REVISION,
  ParityValidationError,
  SOURCE_CONTRACT,
  SOURCE_REPOSITORY,
  SOURCE_WORKFLOW,
  parityTarget,
  verifyParityDirectory,
} from './verify-parity-bundle.mjs';

function fail(message) {
  throw new ParityValidationError(message);
}

function argument(name) {
  const index = process.argv.indexOf(name);
  if (index === -1 || !process.argv[index + 1]) fail(`${name} is required`);
  return process.argv[index + 1];
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, { encoding: 'utf8', ...options });
  if (result.status !== 0) fail(`${command} ${args[0] ?? ''} failed: ${result.stderr || result.stdout}`);
  return result.stdout;
}

function tarText(bytes, label) {
  const terminator = bytes.indexOf(0);
  try {
    return new TextDecoder('utf-8', { fatal: true }).decode(terminator === -1 ? bytes : bytes.subarray(0, terminator));
  } catch {
    fail(`archive ${label} is not valid UTF-8`);
  }
}

function tarSize(bytes) {
  const value = tarText(bytes, 'size').trim();
  if (!/^[0-7]*$/.test(value)) fail('archive member size is invalid');
  return Number.parseInt(value || '0', 8);
}

function assertSafeMemberPath(path) {
  const normalized = path.replace(/^(\.\/)+/, '');
  if (normalized && !normalized.startsWith('/') && !normalized.startsWith('\\') && !normalized.includes('\\') && !/^[A-Za-z]:/.test(normalized) && !normalized.split('/').some((part) => part === '' || part === '.' || part === '..')) return;
  if (normalized === '') return;
  fail('archive contains an unsafe path');
}

export function assertSafeArchive(archive) {
  let bytes;
  try {
    bytes = gunzipSync(readFileSync(archive));
  } catch {
    fail('archive is not a readable gzip tar stream');
  }
  let offset = 0;
  let members = 0;
  while (offset + 512 <= bytes.length) {
    const header = bytes.subarray(offset, offset + 512);
    if (header.every((byte) => byte === 0)) break;
    const type = header[156];
    if (type === 0x32) fail('archive contains a symlink');
    if (type === 0x31) fail('archive contains a hardlink');
    if (type !== 0 && type !== 0x30 && type !== 0x35) fail('archive contains an unsupported member type');
    const name = tarText(header.subarray(0, 100), 'member name');
    const prefix = tarText(header.subarray(345, 500), 'member prefix');
    const path = prefix ? `${prefix}/${name}` : name;
    assertSafeMemberPath(path);
    const size = tarSize(header.subarray(124, 136));
    if (!Number.isSafeInteger(size) || size < 0) fail('archive member size is invalid');
    offset += 512 + Math.ceil(size / 512) * 512;
    if (offset > bytes.length) fail('archive member body is truncated');
    members += 1;
  }
  if (members === 0 || offset + 1024 > bytes.length || !bytes.subarray(offset, offset + 1024).every((byte) => byte === 0)) fail('archive is missing the tar end marker');
}

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

async function main() {
  const sourceRun = argument('--source-run');
  const target = parityTarget(argument('--target'));
  const output = resolve(argument('--output'));
  const downloaded = join(output, 'downloaded');
  await rm(output, { recursive: true, force: true });
  await mkdir(downloaded, { recursive: true });

  const runMetadata = JSON.parse(run('gh', ['run', 'view', sourceRun, '--repo', SOURCE_REPOSITORY, '--json', 'headSha,headBranch,workflowName']));
  if (runMetadata.headSha !== SOURCE_CONTRACT.workflowSupportRevision || runMetadata.headBranch !== 'main' || runMetadata.workflowName !== SOURCE_CONTRACT.workflowName) {
    fail('source workflow run is not for the accepted workflow, revision, and allowed ref');
  }
  run('gh', ['run', 'download', sourceRun, '--repo', SOURCE_REPOSITORY, '--name', target.artifact, '--dir', downloaded]);
  const archiveName = `${SOURCE_CONTRACT.archivePrefix}${target.target}${SOURCE_CONTRACT.archiveSuffix}`;
  const digestName = `${SOURCE_CONTRACT.archivePrefix}${target.target}${SOURCE_CONTRACT.archiveDigestSuffix}`;
  const files = await readdir(downloaded);
  if (files.length !== 2 || !files.includes(archiveName) || !files.includes(digestName)) {
    fail('trusted artifact does not contain the exact target archive and digest sibling');
  }
  const archive = join(downloaded, archiveName);
  const archiveSidecar = await readFile(join(downloaded, digestName), 'utf8');
  const sidecarMatch = new RegExp(`^([a-f0-9]{64})  ${archiveName.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\n$`).exec(archiveSidecar);
  const expectedArchiveSha256 = sidecarMatch?.[1];
  if (!expectedArchiveSha256 || sha256(await readFile(archive)) !== expectedArchiveSha256) {
    fail('target archive digest sibling does not match');
  }
  run('gh', ['attestation', 'verify', archive, '--repo', SOURCE_REPOSITORY, '--signer-workflow', `https://github.com/${SOURCE_REPOSITORY}/${SOURCE_WORKFLOW}`]);
  assertSafeArchive(archive);
  run('tar', ['-xzf', archive, '--no-same-owner', '-C', output]);
  const provenance = {
    repository: SOURCE_REPOSITORY,
    workflow: SOURCE_WORKFLOW,
    ref: 'refs/heads/main',
    sha: SOURCE_CONTRACT.workflowSupportRevision,
    artifact: target.artifact,
  };
  const verified = await verifyParityDirectory(output, { source: provenance, provenance, target: target.target });
  const archiveSha256 = sha256(await readFile(archive));
  const evidence = {
    schemaVersion: 1,
    sourceRun,
    sourceRepository: SOURCE_REPOSITORY,
    sourceWorkflow: SOURCE_WORKFLOW,
    sourceArtifact: target.artifact,
    acceptedOldRevision: ACCEPTED_OLD_REVISION,
    target: target.target,
    archiveSha256,
    expectedArchiveSha256,
    verification: 'gh attestation verify succeeded before extraction',
    captureMetadata: verified.captureMetadata,
    bundleManifest: verified.bundleManifest,
    rawChecksums: verified.rawChecksums,
  };
  await writeFile(join(output, 'source-provenance-evidence.json'), `${JSON.stringify(evidence, null, 2)}\n`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    process.stderr.write(`parity source fetch failed: ${error.message}\n`);
    process.exitCode = 1;
  });
}
