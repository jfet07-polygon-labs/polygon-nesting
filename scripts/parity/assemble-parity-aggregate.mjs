#!/usr/bin/env node
import { createHash } from 'node:crypto';
import { cp, lstat, mkdir, readdir, readFile, rm, writeFile } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import { join, relative, resolve, sep } from 'node:path';

import {
  ACCEPTED_OLD_REVISION,
  CANONICAL_ROW_IDS,
  NORMALIZATION_VERSION,
  PARITY_TARGETS,
  ParityValidationError,
  SOURCE_CONTRACT,
  semanticDigest,
} from './verify-parity-bundle.mjs';

export const PARITY_AGGREGATE_CONTRACT = Object.freeze({
  version: 1,
  artifactName: 'old-new-parity-bundle',
  archiveName: 'old-new-parity-bundle.tar.gz',
  digestName: 'old-new-parity-bundle.tar.gz.sha256',
  aggregateMetadata: 'aggregate-metadata.json',
  bundleManifest: 'bundle-manifest.json',
  checksums: 'SHA256SUMS',
  targetsDirectory: 'targets',
});

export const PARITY_TARGET_LAYOUT = Object.freeze(PARITY_TARGETS.map(({ key, target, artifact }) => Object.freeze({ key, target, artifact })));

function fail(message) {
  throw new ParityValidationError(`parity aggregate ${message}`);
}

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

function canonicalJson(value) {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`;
  if (value && typeof value === 'object') {
    return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(',')}}`;
  }
  return JSON.stringify(value);
}

function relativePath(root, path) {
  const pathValue = relative(root, path);
  if (!pathValue || pathValue.startsWith('..') || pathValue.split(sep).includes('..')) fail('path escapes aggregate staging root');
  return pathValue.split(sep).join('/');
}

async function assertNonLinkedTree(root) {
  const rootStats = await lstat(root);
  if (!rootStats.isDirectory() || rootStats.isSymbolicLink()) fail('target artifact must be a non-symlink directory');
  const entries = await readdir(root, { withFileTypes: true });
  for (const entry of entries) {
    const path = join(root, entry.name);
    const entryStats = await lstat(path);
    if (entryStats.isSymbolicLink()) fail(`target artifact has a symlink: ${entry.name}`);
    if (entry.isDirectory()) {
      await assertNonLinkedTree(path);
      continue;
    }
    if (!entry.isFile()) fail(`target artifact has an unsupported entry: ${entry.name}`);
    if (entryStats.nlink !== 1) fail(`target artifact has a hardlinked file: ${entry.name}`);
  }
}

async function readJson(path, label) {
  let contents;
  try {
    contents = await readFile(path, 'utf8');
  } catch {
    fail(`${label} is required`);
  }
  try {
    return JSON.parse(contents);
  } catch {
    fail(`${label} must contain valid JSON`);
  }
}

async function assertComparisonSet(comparisons, label, target, root, evidenceSide) {
  const expected = CANONICAL_ROW_IDS.flatMap((rowId) => SOURCE_CONTRACT.rawFilenames.map((filename) => `${rowId}/${filename}`));
  if (!Array.isArray(comparisons) || comparisons.length !== expected.length) fail(`${label} must contain every canonical raw comparison for ${target.key}`);
  for (const [index, comparison] of comparisons.entries()) {
    const expectedPath = expected[index];
    if (!comparison || `${comparison.rowId}/${comparison.filename}` !== expectedPath || !/^[a-f0-9]{64}$/.test(comparison.oldRawSha256 ?? '') || !/^[a-f0-9]{64}$/.test(comparison.newRawSha256 ?? '')) {
      fail(`${label} canonical comparison order differs for ${target.key}`);
    }
    const oldBytes = await readFile(join(root, 'old', 'raw', comparison.rowId, comparison.filename));
    const newBytes = await readFile(join(root, evidenceSide, 'raw', comparison.rowId, comparison.filename));
    if (comparison.oldRawSha256 !== sha256(oldBytes) || comparison.newRawSha256 !== sha256(newBytes)) fail(`${label} comparison digest differs from evidence for ${target.key}`);
    if (comparison.filename === 'result.json' || comparison.filename === 'events.ndjson') {
      const format = comparison.filename === 'events.ndjson' ? 'ndjson' : 'json';
      const oldSemantic = semanticDigest(oldBytes, format);
      if (comparison.semanticSha256 !== oldSemantic || semanticDigest(newBytes, format) !== oldSemantic) fail(`${label} semantic digest differs from evidence for ${target.key}`);
    } else if ('semanticSha256' in comparison) {
      fail(`${label} has an unexpected semantic digest for ${target.key}`);
    }
  }
}

const PROJECTOR_SOURCE_PATHS = Object.freeze({
  adapter: 'crates/polygon-nesting-napi/src/bin/parity-desktop-request-adapter.rs',
  outcomeProjector: 'crates/polygon-nesting-napi/src/bin/parity-project-engine-outcome.rs',
  eventsProjector: 'crates/polygon-nesting-napi/src/bin/parity-project-engine-events.rs',
});

async function assertTrustedRevision(root, expectedRevision) {
  const child = spawn('git', ['-C', root, 'rev-parse', 'HEAD'], { stdio: ['ignore', 'pipe', 'ignore'] });
  let output = '';
  child.stdout.on('data', (chunk) => { output += chunk; });
  const status = await new Promise((resolvePromise) => child.on('close', (code) => resolvePromise(code)));
  if (status !== 0 || output.trim() !== expectedRevision) fail('trusted source root revision does not match aggregate revision');
}

async function readCommittedSource(root, revision, sourcePath) {
  const child = spawn('git', ['-C', root, 'cat-file', 'blob', `${revision}:${sourcePath}`], { stdio: ['ignore', 'pipe', 'ignore'] });
  const chunks = [];
  child.stdout.on('data', (chunk) => chunks.push(chunk));
  const status = await new Promise((resolvePromise) => child.on('close', (code) => resolvePromise(code)));
  if (status !== 0) fail(`trusted projector source is not committed: ${sourcePath}`);
  return Buffer.concat(chunks);
}

async function assertTrustedSource(root, sourcePath, expectedRevision) {
  if (!sourcePath || sourcePath !== PROJECTOR_SOURCE_PATHS.adapter && sourcePath !== PROJECTOR_SOURCE_PATHS.outcomeProjector && sourcePath !== PROJECTOR_SOURCE_PATHS.eventsProjector) {
    fail('trusted projector source path is not accepted');
  }
  if (!/^[a-f0-9]{40}$/.test(expectedRevision ?? '')) fail('trusted source revision is not accepted');
  const trustedRoot = resolve(root);
  const trustedPath = resolve(trustedRoot, sourcePath);
  if (!trustedPath.startsWith(`${trustedRoot}${sep}`)) fail('trusted projector source path escapes checkout');
  let stats;
  try {
    stats = await lstat(trustedPath);
  } catch {
    fail(`trusted projector source is missing: ${sourcePath}`);
  }
  if (!stats.isFile() || stats.isSymbolicLink() || stats.nlink !== 1) fail(`trusted projector source is not a regular file: ${sourcePath}`);
  return readCommittedSource(trustedRoot, expectedRevision, sourcePath);
}

async function assertTargetParity(parity, target, root, trustedSourceRoot, sourceRevision) {
  if (!parity || Object.keys(parity).sort().join(',') !== 'acceptedEngineRevision,cliComparisons,comparisons,executableIdentities,napiComparisons,normalizationVersion,sourceArtifact,sourceContractVersion,sourceRevision,target,targetKey,version') fail(`parity.json schema is not accepted for ${target.key}`);
  if (parity.version !== 1) fail(`parity.json version is not accepted for ${target.key}`);
  if (parity.targetKey !== target.key) fail(`parity.json target key does not match artifact directory for ${target.key}`);
  if (parity.target !== target.target || parity.sourceArtifact !== target.artifact) fail(`parity.json target identity does not match artifact directory for ${target.key}`);
  if (parity.acceptedEngineRevision !== ACCEPTED_OLD_REVISION || parity.normalizationVersion !== NORMALIZATION_VERSION || parity.sourceContractVersion !== 1 || !/^[a-f0-9]{40}$/.test(parity.sourceRevision ?? '')) fail(`parity.json revision or normalization is not accepted for ${target.key}`);
  const labels = ['adapter', 'cli', 'outcomeProjector', 'eventsProjector'];
  const expectedLabels = {
    adapter: 'parity-desktop-request-adapter',
    cli: 'polygon-nesting',
    outcomeProjector: 'parity-project-engine-outcome',
    eventsProjector: 'parity-project-engine-events',
  };
  if (!parity.executableIdentities || Object.keys(parity.executableIdentities).sort().join(',') !== labels.sort().join(',')) fail(`parity.json executable identities are incomplete for ${target.key}`);
  for (const label of labels) {
    const identity = parity.executableIdentities[label];
    if (!identity || identity.label !== expectedLabels[label] || identity.version !== 1 || identity.evidencePath !== `executables/${expectedLabels[label]}` || !/^[a-f0-9]{64}$/.test(identity.sha256 ?? '')) fail(`parity.json executable identity is invalid for ${target.key}`);
    let executableBytes;
    try {
      executableBytes = await readFile(join(root, identity.evidencePath));
    } catch {
      fail(`parity.json executable evidence is missing for ${target.key}`);
    }
    if (sha256(executableBytes) !== identity.sha256) fail(`parity.json executable hash differs for ${target.key}`);
    if (label === 'cli') continue;
    if (typeof identity.sourcePath !== 'string' || !/^[A-Za-z0-9._/-]+\.rs$/.test(identity.sourcePath) || !/^[a-f0-9]{64}$/.test(identity.sourceSha256 ?? '') || identity.sourceVersion !== 1 || identity.sourceRevision !== parity.sourceRevision) fail(`parity.json projector source identity is invalid for ${target.key}`);
    const sourcePath = resolve(root, 'source', identity.sourcePath);
    if (!sourcePath.startsWith(`${resolve(root, 'source')}${sep}`)) fail(`parity.json projector source path escapes evidence for ${target.key}`);
    let sourceBytes;
    try {
      sourceBytes = await readFile(sourcePath);
    } catch {
      fail(`parity.json projector source evidence is missing for ${target.key}`);
    }
    if (sha256(sourceBytes) !== identity.sourceSha256) fail(`parity.json projector source hash differs for ${target.key}`);
    const trustedSourceBytes = await assertTrustedSource(trustedSourceRoot, identity.sourcePath, sourceRevision);
    if (!sourceBytes.equals(trustedSourceBytes)) fail(`parity.json projector source differs from trusted checkout for ${target.key}`);
  }
  await assertComparisonSet(parity.napiComparisons, 'N-API comparisons', target, root, 'new');
  await assertComparisonSet(parity.cliComparisons, 'CLI comparisons', target, root, 'projected');
  if (JSON.stringify(parity.comparisons) !== JSON.stringify(parity.napiComparisons)) fail(`parity.json legacy comparisons must equal N-API comparisons for ${target.key}`);
}

async function assertTargetManifest(root, target) {
  const manifest = await readJson(join(root, PARITY_AGGREGATE_CONTRACT.bundleManifest), `${target.key}/bundle-manifest.json`);
  if (manifest.version !== PARITY_AGGREGATE_CONTRACT.version || !Array.isArray(manifest.files) || manifest.files.length === 0) {
    fail(`bundle manifest schema is not accepted for ${target.key}`);
  }
  const entries = new Map();
  for (const entry of manifest.files) {
    if (!entry || typeof entry.path !== 'string' || !/^[a-f0-9]{64}$/.test(entry.sha256) || !Number.isSafeInteger(entry.size) || entry.size < 0) {
      fail(`bundle manifest entry is invalid for ${target.key}`);
    }
    if (entry.path.startsWith('/') || entry.path.split('/').includes('..') || entries.has(entry.path)) {
      fail(`bundle manifest path is invalid for ${target.key}`);
    }
    const path = resolve(root, entry.path);
    if (!path.startsWith(`${resolve(root)}${sep}`)) fail(`bundle manifest path escapes target artifact for ${target.key}`);
    const bytes = await readFile(path);
    if (bytes.length !== entry.size || sha256(bytes) !== entry.sha256) fail(`manifest hash differs for ${target.key}/${entry.path}`);
    entries.set(entry.path, entry.sha256);
  }
  const checksums = await readFile(join(root, PARITY_AGGREGATE_CONTRACT.checksums), 'utf8');
  const checksumEntries = new Map();
  for (const line of checksums.trim().split('\n')) {
    const match = /^([a-f0-9]{64})  (.+)$/.exec(line);
    if (!match || checksumEntries.has(match[2])) fail(`SHA256SUMS entry is invalid for ${target.key}`);
    checksumEntries.set(match[2], match[1]);
  }
  if (checksumEntries.size !== entries.size) fail(`SHA256SUMS does not match bundle manifest for ${target.key}`);
  for (const [path, digest] of entries) {
    if (checksumEntries.get(path) !== digest) fail(`SHA256SUMS does not match bundle manifest for ${target.key}`);
  }
  const actualFiles = (await collectFiles(root))
    .filter((path) => ![PARITY_AGGREGATE_CONTRACT.bundleManifest, PARITY_AGGREGATE_CONTRACT.checksums].includes(path));
  if (JSON.stringify(actualFiles) !== JSON.stringify([...entries.keys()].sort())) {
    fail(`bundle manifest does not enumerate every target evidence file for ${target.key}`);
  }
}

async function collectFiles(root) {
  const files = [];
  async function visit(path) {
    const entries = await readdir(path, { withFileTypes: true });
    for (const entry of entries.sort((left, right) => left.name.localeCompare(right.name))) {
      const entryPath = join(path, entry.name);
      const entryStats = await lstat(entryPath);
      if (entryStats.isSymbolicLink()) fail(`aggregate staging has a symlink: ${relativePath(root, entryPath)}`);
      if (entry.isDirectory()) {
        await visit(entryPath);
      } else if (entry.isFile()) {
        if (entryStats.nlink !== 1) fail(`aggregate staging has a hardlinked file: ${relativePath(root, entryPath)}`);
        files.push(relativePath(root, entryPath));
      } else {
        fail(`aggregate staging has an unsupported entry: ${relativePath(root, entryPath)}`);
      }
    }
  }
  await visit(root);
  return files.sort();
}

async function writeManifestAndChecksums(root, metadata) {
  const metadataPath = join(root, PARITY_AGGREGATE_CONTRACT.aggregateMetadata);
  await writeFile(metadataPath, `${canonicalJson(metadata)}\n`);
  const evidenceFiles = await collectFiles(root);
  const files = [];
  for (const path of evidenceFiles) {
    const bytes = await readFile(join(root, path));
    files.push({ path, sha256: sha256(bytes), size: bytes.length });
  }
  const manifest = { version: PARITY_AGGREGATE_CONTRACT.version, files };
  await writeFile(join(root, PARITY_AGGREGATE_CONTRACT.bundleManifest), `${canonicalJson(manifest)}\n`);
  const checksumFiles = [...await collectFiles(root)].filter((path) => path !== PARITY_AGGREGATE_CONTRACT.checksums);
  const checksums = await Promise.all(checksumFiles.map(async (path) => `${sha256(await readFile(join(root, path)))}  ${path}`));
  await writeFile(join(root, PARITY_AGGREGATE_CONTRACT.checksums), `${checksums.join('\n')}\n`);
}

function run(command, arguments_, options) {
  return new Promise((resolvePromise, reject) => {
    const child = spawn(command, arguments_, options);
    let stderr = '';
    child.stderr.on('data', (chunk) => { stderr += chunk; });
    child.on('error', reject);
    child.on('close', (code, signal) => {
      if (code === 0 && !signal) resolvePromise();
      else reject(new Error(`${command} failed with ${signal ?? code}: ${stderr.trim()}`));
    });
  });
}

async function archive(stagingDirectory, archivePath) {
  await run('tar', [
    '--sort=name', '--owner=0', '--group=0', '--numeric-owner', '--mtime=UTC 1970-01-01',
    '--use-compress-program=gzip -n', '-C', stagingDirectory, '-cf', archivePath, '.',
  ], { stdio: ['ignore', 'ignore', 'pipe'] });
}

export async function assembleParityAggregate({ inputDirectory, outputDirectory, sourceRevision, trustedSourceRoot }) {
  if (!/^[a-f0-9]{40}$/.test(sourceRevision ?? '')) fail('standalone source revision must be a full SHA');
  if (typeof trustedSourceRoot !== 'string' || !trustedSourceRoot) fail('trusted source root is required');
  const trustedRoot = resolve(trustedSourceRoot);
  let trustedStats;
  try {
    trustedStats = await lstat(trustedRoot);
  } catch {
    fail('trusted source root is required');
  }
  if (!trustedStats.isDirectory() || trustedStats.isSymbolicLink()) fail('trusted source root must be a non-symlink directory');
  await assertTrustedRevision(trustedRoot, sourceRevision);
  const inputRoot = resolve(inputDirectory);
  const outputRoot = resolve(outputDirectory);
  await assertNonLinkedTree(inputRoot);
  const inputEntries = await readdir(inputRoot, { withFileTypes: true });
  if (inputEntries.some((entry) => !entry.isDirectory() || entry.isSymbolicLink())) fail('input artifacts contain an unlisted root entry');
  const actualKeys = inputEntries
    .map((entry) => entry.name)
    .sort();
  const expectedKeys = PARITY_TARGET_LAYOUT.map(({ key }) => `old-new-parity-target-${key}`).sort();
  if (JSON.stringify(actualKeys) !== JSON.stringify(expectedKeys)) fail(`input artifacts must contain exactly the ${PARITY_TARGET_LAYOUT.length} required target directories`);

  await rm(outputRoot, { recursive: true, force: true });
  const stagingDirectory = join(outputRoot, 'staging');
  await mkdir(join(stagingDirectory, PARITY_AGGREGATE_CONTRACT.targetsDirectory), { recursive: true });
  for (const target of PARITY_TARGET_LAYOUT) {
    const source = join(inputRoot, `old-new-parity-target-${target.key}`);
    await assertNonLinkedTree(source);
    await assertTargetManifest(source, target);
    const parity = await readJson(join(source, 'parity.json'), `${target.key}/parity.json`);
    await assertTargetParity(parity, target, source, trustedRoot, sourceRevision);
    if (parity.sourceRevision !== sourceRevision) fail(`parity.json source revision does not match aggregate revision for ${target.key}`);
    await cp(source, join(stagingDirectory, PARITY_AGGREGATE_CONTRACT.targetsDirectory, target.target), {
      recursive: true,
      dereference: false,
      errorOnExist: true,
      force: false,
    });
  }
  const metadata = {
    version: PARITY_AGGREGATE_CONTRACT.version,
    acceptedEngineRevision: ACCEPTED_OLD_REVISION,
    sourceRevision,
    targets: PARITY_TARGET_LAYOUT,
  };
  await writeManifestAndChecksums(stagingDirectory, metadata);
  const archivePath = join(outputRoot, PARITY_AGGREGATE_CONTRACT.archiveName);
  await archive(stagingDirectory, archivePath);
  const digest = sha256(await readFile(archivePath));
  const digestPath = join(outputRoot, PARITY_AGGREGATE_CONTRACT.digestName);
  await writeFile(digestPath, `${digest}  ${PARITY_AGGREGATE_CONTRACT.archiveName}\n`);
  return { archivePath, digestPath, digest };
}

async function main() {
  const arguments_ = process.argv.slice(2);
  function argument(name) {
    const index = arguments_.indexOf(name);
    if (index === -1 || !arguments_[index + 1]) fail(`${name} is required`);
    return arguments_[index + 1];
  }
  const inputDirectory = argument('--input');
  const outputDirectory = argument('--output');
  const sourceRevision = argument('--source-revision');
  const trustedSourceRoot = argument('--trusted-source-root');
  const result = await assembleParityAggregate({ inputDirectory, outputDirectory, sourceRevision, trustedSourceRoot });
  process.stdout.write(`${result.archivePath}\n`);
}

if (import.meta.url === new URL(process.argv[1], 'file:').href) {
  main().catch((error) => {
    process.stderr.write(`standalone parity aggregate failed: ${error.message}\n`);
    process.exitCode = 1;
  });
}
