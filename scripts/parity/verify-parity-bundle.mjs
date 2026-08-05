import { createHash } from 'node:crypto';
import { lstat, readdir, readFile } from 'node:fs/promises';
import { resolve, sep } from 'node:path';

export const SOURCE_CONTRACT = Object.freeze({
  repository: 'jfet97/min-plane-dfx',
  workflow: '.github/workflows/capture-old-rust-parity.yml',
  workflowName: 'Capture accepted old Rust parity',
  ref: 'refs/heads/main',
  workflowSupportRevision: '93fed7f533ed119e9573d0e6c2ebd2b2f4815a10',
  acceptedEngineRevision: '5c72d8fca8e078b0a6e7d5f2515a8a0953475481',
  sourceProvenanceRevision: 'e4f3608878611c002f343473fab72adc7d155f87',
  artifactPrefix: 'old-rust-parity-capture-',
  archivePrefix: 'old-rust-parity-capture-',
  archiveSuffix: '.tar.gz',
  archiveDigestSuffix: '.tar.gz.sha256',
  captureMetadata: 'capture-metadata.json',
  bundleManifest: 'bundle-manifest.json',
  bundleManifestVersion: 1,
  rawFilenames: ['request.json', 'result.json', 'events.ndjson', 'stderr.txt', 'process.json'],
});

export const ACCEPTED_OLD_REVISION = SOURCE_CONTRACT.acceptedEngineRevision;
export const SOURCE_REPOSITORY = SOURCE_CONTRACT.repository;
export const SOURCE_WORKFLOW = SOURCE_CONTRACT.workflow;
export const TARGET = 'aarch64-apple-darwin';
export const RUST_VERSION = '1.95.0';
export const NORMALIZATION_VERSION = '1';
export const ACCEPTED_NATIVE_DEPENDENCIES = Object.freeze([
  ['autocfg', '1.5.1'], ['bitflags', '2.13.1'], ['block-buffer', '0.10.4'], ['cfg-if', '1.0.4'], ['convert_case', '0.11.0'], ['cpufeatures', '0.2.17'], ['crossbeam-deque', '0.8.7'], ['crossbeam-epoch', '0.9.20'], ['crossbeam-utils', '0.8.22'], ['crypto-common', '0.1.7'], ['ctor', '1.0.11'], ['digest', '0.10.7'], ['either', '1.17.0'], ['futures-channel', '0.3.33'], ['futures-core', '0.3.33'], ['futures-executor', '0.3.33'], ['futures-io', '0.3.33'], ['futures-macro', '0.3.33'], ['futures-sink', '0.3.33'], ['futures-task', '0.3.33'], ['futures-util', '0.3.33'], ['futures', '0.3.33'], ['generic-array', '0.14.7'], ['irregular-nesting-native', '0.1.0'], ['itoa', '1.0.18'], ['libc', '0.2.189'], ['libloading', '0.9.0'], ['libm', '0.2.16'], ['memchr', '2.8.3'], ['napi-build', '2.4.0'], ['napi-derive-backend', '6.1.0'], ['napi-derive', '3.6.1'], ['napi-sys', '3.3.0'], ['napi', '3.12.0'], ['nohash-hasher', '0.2.0'], ['num-bigint', '0.4.8'], ['num-integer', '0.1.46'], ['num-traits', '0.2.19'], ['pin-project-lite', '0.2.17'], ['proc-macro2', '1.0.107'], ['quote', '1.0.47'], ['rayon-core', '1.13.0'], ['rayon', '1.12.0'], ['robust', '1.2.0'], ['rustc-hash', '2.1.3'], ['ryu-js', '1.0.3'], ['semver', '1.0.28'], ['serde', '1.0.229'], ['serde_core', '1.0.229'], ['serde_derive', '1.0.229'], ['serde_json', '1.0.151'], ['sha2', '0.10.9'], ['slab', '0.4.12'], ['syn', '2.0.119'], ['syn', '3.0.3'], ['typenum', '1.20.1'], ['unicode-ident', '1.0.24'], ['unicode-segmentation', '1.13.3'], ['version_check', '0.9.5'], ['windows-link', '0.2.1'], ['zmij', '1.0.23'],
].map(([name, version]) => Object.freeze({ name, version })));
export const ACCEPTED_NATIVE_DEPENDENCIES_SHA256 = '8925ac904fa2eb41a3f82907d530578a5174509eef0470712193cc4d45a3d0c8';

export const CANONICAL_ROW_IDS = Object.freeze([
  'triangle-20-2000x2700-compact', 'triangle-20-2000x2700-short-side', 'triangle-20-600x400-compact', 'triangle-20-600x400-short-side', 'triangle-20-300x300-compact', 'triangle-20-300x300-short-side',
  'mixed-61-2000x2700-compact', 'mixed-61-2000x2700-short-side', 'mixed-61-600x400-compact', 'mixed-61-600x400-short-side', 'mixed-61-300x300-compact', 'mixed-61-300x300-short-side',
  'shapes-17-2000x2700-compact', 'shapes-17-2000x2700-short-side', 'shapes-17-600x400-compact', 'shapes-17-600x400-short-side', 'shapes-17-300x300-compact', 'shapes-17-300x300-short-side',
]);

export const PARITY_TARGETS = Object.freeze([
  Object.freeze({ key: 'linux-x64', runner: 'ubuntu-24.04', target: 'x86_64-unknown-linux-gnu', profile: 'release', features: [], rustVersion: RUST_VERSION, artifact: 'old-rust-parity-capture-x86_64-unknown-linux-gnu' }),
  Object.freeze({ key: 'win32-x64', runner: 'windows-latest', target: 'x86_64-pc-windows-msvc', profile: 'release', features: [], rustVersion: RUST_VERSION, artifact: 'old-rust-parity-capture-x86_64-pc-windows-msvc' }),
  Object.freeze({ key: 'darwin-arm64', runner: 'macos-15', target: 'aarch64-apple-darwin', profile: 'release', features: [], rustVersion: RUST_VERSION, artifact: 'old-rust-parity-capture-aarch64-apple-darwin' }),
  Object.freeze({ key: 'darwin-x64', runner: 'macos-15-intel', target: 'x86_64-apple-darwin', profile: 'release', features: [], rustVersion: RUST_VERSION, artifact: 'old-rust-parity-capture-x86_64-apple-darwin' }),
]);

export function parityTarget(target) {
  const match = PARITY_TARGETS.find((entry) => entry.target === target);
  if (!match) fail(`target is not in the required parity matrix: ${target}`);
  return match;
}

const RAW_FILENAMES = SOURCE_CONTRACT.rawFilenames;
const MEASUREMENT_FIELDS = new Set([
  'runtimeMs',
  'elapsedMs',
  'preflightRuntimeMs',
  'completeArchiveRuntimeMs',
  'prefixTerminalizationMs',
  'coldSearchMs',
  'topologyMeasurementMs',
  'contactMeasurementMs',
  'serializedTraceBytes',
  'peakRssDeltaBytes',
]);

export class ParityValidationError extends Error {
  constructor(message) {
    super(message);
    this.name = 'ParityValidationError';
  }
}

function fail(message) {
  throw new ParityValidationError(message);
}

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

function required(value, message) {
  if (!value) fail(message);
  return value;
}

async function readJson(path, label) {
  let bytes;
  try {
    bytes = await readFile(path);
  } catch {
    fail(`${label} is required`);
  }
  try {
    return JSON.parse(bytes);
  } catch {
    fail(`${label} must contain valid JSON`);
  }
}

function assertSourceIdentity(identity, label, expectedTarget = TARGET) {
  required(identity, `${label} is required`);
  const target = parityTarget(expectedTarget);
  if (identity.repository !== SOURCE_REPOSITORY) fail(`${label} repository is not accepted`);
  if (identity.workflow !== SOURCE_WORKFLOW) fail(`${label} workflow is not accepted`);
  if (identity.artifact !== target.artifact) fail(`${label} artifact is not accepted`);
  if (identity.sha !== SOURCE_CONTRACT.workflowSupportRevision) fail(`${label} sha is not the accepted workflow support revision`);
  if (identity.ref !== 'refs/heads/main') fail(`${label} ref is not allowed`);
}

function assertExactKeys(value, keys, label) {
  if (!value || typeof value !== 'object' || Array.isArray(value) || Object.keys(value).sort().join(',') !== [...keys].sort().join(',')) fail(`${label} schema is not accepted`);
}

function assertCaptureMetadata(metadata, expectedTarget, expectedArtifact) {
  assertExactKeys(metadata, ['version', 'acceptedEngineRevision', 'sourceProvenanceRevision', 'target', 'toolchain', 'artifactName', 'build', 'rustc', 'cargo', 'sourceCargoLockSha256', 'nativeDependencies', 'addon', 'workflow', 'corpus', 'raw'], 'capture metadata');
  assertExactKeys(metadata.build, ['profile', 'features'], 'capture metadata build');
  assertExactKeys(metadata.rustc, ['identity', 'verbose'], 'capture metadata rustc');
  assertExactKeys(metadata.cargo, ['identity'], 'capture metadata cargo');
  assertExactKeys(metadata.nativeDependencies, ['entries', 'sha256'], 'capture metadata native dependencies');
  assertExactKeys(metadata.addon, ['historicalSha256', 'freshSha256'], 'capture metadata addon');
  assertExactKeys(metadata.workflow, ['repository', 'ref', 'sha', 'runId', 'runAttempt'], 'capture metadata workflow');
  assertExactKeys(metadata.corpus, ['manifestName', 'manifestSha256', 'sha256SumsSha256', 'rowIds'], 'capture metadata corpus');
  assertExactKeys(metadata.raw, ['version', 'files'], 'capture metadata raw');
  if (metadata.version !== 1) fail('capture metadata version is not accepted');
  if (metadata.acceptedEngineRevision !== ACCEPTED_OLD_REVISION) fail('capture metadata engine revision is not accepted');
  if (metadata.sourceProvenanceRevision !== SOURCE_CONTRACT.sourceProvenanceRevision) {
    fail('capture metadata source provenance revision is not accepted');
  }
  if (metadata.target !== expectedTarget) fail('capture metadata target is not accepted');
  if (metadata.artifactName !== expectedArtifact) fail('capture metadata artifact is not accepted');
  if (metadata.toolchain !== RUST_VERSION || !metadata.rustc?.identity?.includes(RUST_VERSION) || !metadata.rustc?.verbose?.includes(RUST_VERSION) || !metadata.cargo?.identity?.includes(RUST_VERSION)) {
    fail('capture metadata Rust and Cargo versions must be 1.95.0');
  }
  if (metadata.build?.profile !== 'release') fail('capture metadata profile is not release');
  if (!Array.isArray(metadata.build?.features) || metadata.build.features.length !== 0) fail('capture metadata features are not accepted');
  if (!/^[a-f0-9]{64}$/.test(metadata.sourceCargoLockSha256 ?? '')) fail('capture metadata lock hash is required');
  if (!Array.isArray(metadata.nativeDependencies?.entries) || metadata.nativeDependencies.entries.length === 0 || !/^[a-f0-9]{64}$/.test(metadata.nativeDependencies?.sha256 ?? '')) {
    fail('capture metadata dependency identity is required');
  }
  if (!metadata.nativeDependencies.entries.every((entry, index, entries) => {
    assertExactKeys(entry, ['name', 'version'], 'capture metadata dependency entry');
    return typeof entry.name === 'string' && entry.name && typeof entry.version === 'string' && entry.version && (index === 0 || `${entries[index - 1].name}@${entries[index - 1].version}` < `${entry.name}@${entry.version}`);
  })) fail('capture metadata dependency entries are invalid');
  if (JSON.stringify(metadata.nativeDependencies.entries) !== JSON.stringify(ACCEPTED_NATIVE_DEPENDENCIES) || metadata.nativeDependencies.sha256 !== ACCEPTED_NATIVE_DEPENDENCIES_SHA256) fail('capture metadata dependency identity is not accepted');
  if (sha256(`${JSON.stringify(metadata.nativeDependencies.entries)}\n`) !== metadata.nativeDependencies.sha256) fail('capture metadata dependency digest differs from canonical entries');
  if (!/^[a-f0-9]{64}$/.test(metadata.addon?.historicalSha256 ?? '') || !/^[a-f0-9]{64}$/.test(metadata.addon?.freshSha256 ?? '')) {
    fail('capture metadata addon identities are required');
  }
  if (metadata.addon.historicalSha256 !== '9fc447f80a820c60676eee62706694c7f7ac79092a66ac131ac50b4f216dec9b') {
    fail('capture metadata historical addon identity is not accepted');
  }
  if (metadata.workflow.repository !== SOURCE_REPOSITORY || metadata.workflow.ref !== SOURCE_CONTRACT.ref || metadata.workflow.sha !== SOURCE_CONTRACT.workflowSupportRevision || typeof metadata.workflow.runId !== 'string' || metadata.workflow.runId.length === 0 || typeof metadata.workflow.runAttempt !== 'string' || metadata.workflow.runAttempt.length === 0) fail('capture metadata workflow identity is not accepted');
  if (metadata.corpus?.manifestName !== 'migration-corpus.json' || !/^[a-f0-9]{64}$/.test(metadata.corpus?.manifestSha256 ?? '') || !/^[a-f0-9]{64}$/.test(metadata.corpus?.sha256SumsSha256 ?? '')) {
    fail('capture metadata corpus identity is required');
  }
  if (JSON.stringify(metadata.corpus.rowIds) !== JSON.stringify(CANONICAL_ROW_IDS)) fail('capture metadata corpus rows are not in trusted order');
  if (metadata.raw?.version !== 1 || !Array.isArray(metadata.raw?.files)) fail('capture metadata raw evidence is required');
}

async function assertBundleManifest(root, bundle) {
  if (bundle.version !== SOURCE_CONTRACT.bundleManifestVersion || !Array.isArray(bundle.files)) {
    fail('bundle manifest version or files is invalid');
  }
  const files = new Map();
  for (const entry of bundle.files) {
    if (!entry || typeof entry.path !== 'string' || !/^[a-f0-9]{64}$/.test(entry.sha256) || !Number.isSafeInteger(entry.size) || entry.size < 0) {
      fail('bundle manifest file entry is invalid');
    }
    if (entry.path.startsWith('/') || entry.path.split('/').includes('..') || files.has(entry.path)) fail('bundle manifest path is invalid');
    const path = await assertRegularFile(root, entry.path);
    const bytes = await readFile(path);
    if (bytes.length !== entry.size || sha256(bytes) !== entry.sha256) fail(`bundle manifest hash differs for ${entry.path}`);
    files.set(entry.path, entry);
  }
  return files;
}

async function assertExactBundleTree(root, manifestFiles) {
  const actual = [];
  async function visit(directory) {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const path = resolve(directory, entry.name);
      const stats = await lstat(path);
      const relativePath = path.slice(`${resolve(root)}${sep}`.length).split(sep).join('/');
      if (stats.isSymbolicLink() || (entry.isFile() && stats.nlink !== 1)) fail(`bundle contains a linked entry: ${relativePath}`);
      if (entry.isDirectory()) await visit(path);
      else if (entry.isFile()) actual.push(relativePath);
      else fail(`bundle contains an unsupported entry: ${relativePath}`);
    }
  }
  await visit(root);
  const expected = [...new Set([SOURCE_CONTRACT.bundleManifest, 'SHA256SUMS', ...manifestFiles.keys()])].sort();
  if (JSON.stringify(actual.sort()) !== JSON.stringify(expected)) fail('bundle contains unlisted or missing files');
}

function parseChecksums(contents) {
  const entries = new Map();
  for (const line of contents.trim().split('\n')) {
    if (!line) continue;
    const match = /^([a-f0-9]{64})  (.+)$/.exec(line);
    if (!match) fail('SHA256SUMS has an invalid entry');
    const [, digest, path] = match;
    if (path.startsWith('/') || path.split('/').includes('..')) fail('SHA256SUMS path escapes the bundle');
    if (entries.has(path)) fail('SHA256SUMS has a duplicate path');
    entries.set(path, digest);
  }
  return entries;
}

async function assertRegularFile(root, relativePath) {
  const path = resolve(root, relativePath);
  if (!path.startsWith(`${resolve(root)}${sep}`)) fail(`raw path escapes bundle: ${relativePath}`);
  let stats;
  try {
    stats = await lstat(path);
  } catch {
    fail(`raw file is missing: ${relativePath}`);
  }
  if (!stats.isFile() || stats.isSymbolicLink()) fail(`raw file must be a regular non-symlink: ${relativePath}`);
  return path;
}

async function assertRawTree(root, rows, checksums) {
  for (const rowId of rows) {
    if (typeof rowId !== 'string' || !/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(rowId)) fail('manifest row id is invalid');
    for (const filename of RAW_FILENAMES) {
      const path = `old/raw/${rowId}/${filename}`;
      const absolute = await assertRegularFile(root, path);
      const expected = checksums.get(path);
      if (!expected) fail(`SHA256SUMS is missing ${path}`);
      if (sha256(await readFile(absolute)) !== expected) fail(`raw hash differs for ${path}`);
    }
  }
}

export function verifyDependencyIdentity(actual, accepted) {
  for (const [name, version] of Object.entries(accepted)) {
    if (actual[name] !== version) fail(`dependency identity differs for ${name}`);
  }
}

export function normalizeSemanticJson(value) {
  if (Array.isArray(value)) return value.map(normalizeSemanticJson);
  if (!value || typeof value !== 'object') return value;
  return Object.fromEntries(Object.entries(value).map(([key, nested]) => [
    key,
    MEASUREMENT_FIELDS.has(key) ? '<measurement>' : normalizeSemanticJson(nested),
  ]));
}

function canonicalJson(value) {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`;
  if (value && typeof value === 'object') {
    return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(',')}}`;
  }
  return JSON.stringify(value);
}

export function semanticDigest(bytes, format = 'json') {
  const lines = format === 'ndjson' ? bytes.toString('utf8').split('\n').filter(Boolean) : [bytes.toString('utf8')];
  const normalized = lines.map((line) => canonicalJson(normalizeSemanticJson(JSON.parse(line)))).join('\n');
  return sha256(normalized);
}

export async function verifyParityDirectory(root, { source, provenance, target = TARGET }) {
  parityTarget(target);
  assertSourceIdentity(source, 'requested source', target);
  assertSourceIdentity(provenance, 'provenance attestation', target);
  const targetContract = parityTarget(target);
  const captureMetadataPath = await assertRegularFile(root, SOURCE_CONTRACT.captureMetadata);
  const bundleManifestPath = await assertRegularFile(root, SOURCE_CONTRACT.bundleManifest);
  const checksumPath = await assertRegularFile(root, 'SHA256SUMS');
  const captureMetadata = await readJson(captureMetadataPath, SOURCE_CONTRACT.captureMetadata);
  assertCaptureMetadata(captureMetadata, target, targetContract.artifact);
  const bundleManifest = await readJson(bundleManifestPath, SOURCE_CONTRACT.bundleManifest);
  const manifestFiles = await assertBundleManifest(root, bundleManifest);
  await assertExactBundleTree(root, manifestFiles);
  const rawFiles = captureMetadata.raw.files;
  if (!Array.isArray(rawFiles) || rawFiles.length !== CANONICAL_ROW_IDS.length * SOURCE_CONTRACT.rawFilenames.length) fail('capture metadata raw file identities are incomplete');
  const metadataRaw = new Map();
  for (const entry of rawFiles) {
    if (!entry || typeof entry.path !== 'string' || !/^[^/]+\/(?:request|result|events|stderr|process)\.(?:json|ndjson|txt)$/.test(entry.path) || !/^[a-f0-9]{64}$/.test(entry.sha256 ?? '') || !Number.isSafeInteger(entry.size) || entry.size < 0 || metadataRaw.has(entry.path)) fail('capture metadata raw file identity is invalid');
    metadataRaw.set(entry.path, entry);
  }
  const manifestRaw = [...manifestFiles.values()].filter(({ path }) => path.startsWith('old/raw/'));
  if (manifestRaw.length !== metadataRaw.size || manifestRaw.some((entry) => {
    const metadataEntry = metadataRaw.get(entry.path.slice('old/raw/'.length));
    return !metadataEntry || metadataEntry.sha256 !== entry.sha256 || metadataEntry.size !== entry.size;
  })) fail('capture metadata raw file identities differ from the bundle manifest');
  const rows = [...new Set([...manifestFiles.keys()].map((path) => /^old\/raw\/([^/]+)\//.exec(path)?.[1]).filter(Boolean))].sort();
  const expectedRows = [...CANONICAL_ROW_IDS].sort();
  if (JSON.stringify(rows) !== JSON.stringify(expectedRows)) fail('bundle raw rows do not match the trusted canonical 18-row set');
  let checksumContents;
  try {
    checksumContents = await readFile(checksumPath, 'utf8');
  } catch {
    fail('SHA256SUMS is required');
  }
  const checksums = parseChecksums(checksumContents);
  if (checksums.size !== manifestFiles.size || [...manifestFiles].some(([path, entry]) => checksums.get(path) !== entry.sha256)) fail('SHA256SUMS does not exactly match the bundle manifest');
  await assertRawTree(root, rows, checksums);
  return {
    captureMetadata,
    bundleManifest,
    rawChecksums: Object.fromEntries([...checksums].filter(([path]) => path.startsWith('old/raw/'))),
    normalizationVersion: NORMALIZATION_VERSION,
  };
}

export async function compareParityRows(oldRoot, newRoot, rowIds, { newSide = 'new' } = {}) {
  if (!['new', 'projected'].includes(newSide)) fail('parity comparison new side is not accepted');
  const evidence = [];
  for (const rowId of rowIds) {
    for (const filename of RAW_FILENAMES) {
      const oldBytes = await readFile(await assertRegularFile(oldRoot, `old/raw/${rowId}/${filename}`));
      const newBytes = await readFile(await assertRegularFile(newRoot, `${newSide}/raw/${rowId}/${filename}`));
      const comparison = filename === 'events.ndjson' ? 'ndjson' : filename === 'result.json' ? 'json' : null;
      const oldDigest = comparison ? semanticDigest(oldBytes, comparison) : null;
      const newDigest = comparison ? semanticDigest(newBytes, comparison) : null;
      if (oldDigest !== newDigest) fail(`semantic mismatch for ${rowId}/${filename}`);
      evidence.push({
        rowId,
        filename,
        oldRawSha256: sha256(oldBytes),
        newRawSha256: sha256(newBytes),
        ...(oldDigest ? { semanticSha256: oldDigest } : {}),
      });
    }
  }
  return evidence;
}
