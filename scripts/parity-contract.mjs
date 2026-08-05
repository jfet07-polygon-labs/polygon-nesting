import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { copyFileSync, cpSync, existsSync, lstatSync, mkdirSync, readFileSync, readdirSync, rmSync } from 'node:fs';
import { join, resolve, sep } from 'node:path';

import {
  ACCEPTED_OLD_REVISION,
  CANONICAL_ROW_IDS,
  NORMALIZATION_VERSION,
  PARITY_TARGETS,
  SOURCE_CONTRACT,
  semanticDigest,
} from './parity/verify-parity-bundle.mjs';

export const PARITY_CONTRACT = Object.freeze({
  archiveName: 'old-new-parity-bundle.tar.gz',
  archiveSha256Name: 'old-new-parity-bundle.tar.gz.sha256',
});

const SHA256_PATTERN = /^[a-f0-9]{64}$/;
const ROOT_FILES = Object.freeze(['aggregate-metadata.json', 'bundle-manifest.json', 'SHA256SUMS', 'targets']);
const TARGET_MANIFEST_EXCLUSIONS = new Set(['bundle-manifest.json', 'SHA256SUMS']);
const PROJECTOR_SOURCES = Object.freeze({
  adapter: 'crates/polygon-nesting-napi/src/bin/parity-desktop-request-adapter.rs',
  outcomeProjector: 'crates/polygon-nesting-napi/src/bin/parity-project-engine-outcome.rs',
  eventsProjector: 'crates/polygon-nesting-napi/src/bin/parity-project-engine-events.rs',
});
const EXECUTABLES = Object.freeze({
  adapter: 'parity-desktop-request-adapter',
  cli: 'polygon-nesting',
  outcomeProjector: 'parity-project-engine-outcome',
  eventsProjector: 'parity-project-engine-events',
});
const CLI_TRANSPORT_FILENAMES = Object.freeze([
  'adapted-request.json', 'adapter-stderr.txt', 'adapter-process.json', 'result.json', 'events.ndjson', 'stderr.txt',
  'process.json', 'projected-result.json', 'projected-events.ndjson', 'outcome-projector-process.json', 'events-projector-process.json',
]);

function fail(message) {
  throw new Error(`trusted parity aggregate ${message}`);
}

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

function exactKeys(value, keys, label) {
  if (!value || typeof value !== 'object' || Array.isArray(value) || Object.keys(value).sort().join(',') !== [...keys].sort().join(',')) {
    fail(`${label} schema is not accepted`);
  }
}

function safeRelativePath(value, label) {
  if (typeof value !== 'string' || !value || value.includes('\\') || value.startsWith('/') || /^[A-Za-z]:/.test(value) || value.split('/').some((part) => !part || part === '.' || part === '..')) {
    fail(`${label} path is unsafe`);
  }
  return value;
}

function regularPath(root, relativePath, label) {
  safeRelativePath(relativePath, label);
  const path = resolve(root, relativePath);
  if (!path.startsWith(`${resolve(root)}${sep}`) || !existsSync(path)) fail(`${label} is missing: ${relativePath}`);
  const stats = lstatSync(path);
  if (!stats.isFile() || stats.isSymbolicLink() || stats.nlink !== 1) fail(`${label} must be a regular non-linked file: ${relativePath}`);
  return path;
}

function directoryPath(root, relativePath, label) {
  safeRelativePath(relativePath, label);
  const path = resolve(root, relativePath);
  if (!path.startsWith(`${resolve(root)}${sep}`) || !existsSync(path)) fail(`${label} is missing: ${relativePath}`);
  const stats = lstatSync(path);
  if (!stats.isDirectory() || stats.isSymbolicLink()) fail(`${label} must be a non-symlink directory: ${relativePath}`);
  return path;
}

function readJson(root, relativePath, label) {
  try {
    return JSON.parse(readFileSync(regularPath(root, relativePath, label), 'utf8'));
  } catch (error) {
    if (error.message.startsWith('trusted parity aggregate')) throw error;
    fail(`${label} is not valid JSON`);
  }
}

function collectFiles(root, relativePath = '') {
  const files = [];
  for (const entry of readdirSync(root, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
    const next = relativePath ? `${relativePath}/${entry.name}` : entry.name;
    const path = join(root, entry.name);
    const stats = lstatSync(path);
    if (stats.isSymbolicLink()) fail(`archive extraction has a symlink: ${next}`);
    if (entry.isDirectory()) files.push(...collectFiles(path, next));
    else if (entry.isFile() && stats.nlink === 1) files.push(next);
    else fail(`archive extraction has an unsupported entry: ${next}`);
  }
  return files.sort();
}

function readChecksums(root, relativePath, label) {
  const contents = readFileSync(regularPath(root, relativePath, label), 'utf8');
  if (!contents.endsWith('\n') || contents.length === 0) fail(`${label} format is invalid`);
  const entries = new Map();
  for (const line of contents.slice(0, -1).split('\n')) {
    const match = /^([a-f0-9]{64})  (.+)$/.exec(line);
    if (!match || entries.has(match[2])) fail(`${label} entry is invalid`);
    safeRelativePath(match[2], label);
    entries.set(match[2], match[1]);
  }
  return entries;
}

function verifyManifest(root, manifestPath, checksumPath, exclusions, label, checksumIncludesManifest = false) {
  const manifest = readJson(root, manifestPath, label);
  exactKeys(manifest, ['files', 'version'], label);
  if (manifest.version !== 1 || !Array.isArray(manifest.files)) fail(`${label} version or files are invalid`);
  const paths = new Set();
  for (const entry of manifest.files) {
    exactKeys(entry, ['path', 'sha256', 'size'], `${label} entry`);
    safeRelativePath(entry.path, label);
    if (!SHA256_PATTERN.test(entry.sha256) || !Number.isSafeInteger(entry.size) || entry.size < 0 || paths.has(entry.path)) fail(`${label} entry is invalid`);
    const bytes = readFileSync(regularPath(root, entry.path, label));
    if (bytes.length !== entry.size || sha256(bytes) !== entry.sha256) fail(`${label} hash differs for ${entry.path}`);
    paths.add(entry.path);
  }
  const actual = collectFiles(root).filter((path) => !exclusions.has(path));
  if (JSON.stringify([...paths].sort()) !== JSON.stringify(actual)) fail(`${label} does not enumerate every file`);
  const checksums = readChecksums(root, checksumPath, `${label} SHA256SUMS`);
  const checksumPaths = new Set(paths);
  if (checksumIncludesManifest) checksumPaths.add(manifestPath);
  if (checksums.size !== checksumPaths.size || JSON.stringify([...checksums.keys()].sort()) !== JSON.stringify([...checksumPaths].sort())) fail(`${label} SHA256SUMS closure differs`);
  for (const path of checksumPaths) if (checksums.get(path) !== sha256(readFileSync(regularPath(root, path, label)))) fail(`${label} SHA256SUMS hash differs for ${path}`);
  return manifest;
}

function verifyAggregateMetadata(root, sourceCommit) {
  const metadata = readJson(root, 'aggregate-metadata.json', 'aggregate metadata');
  exactKeys(metadata, ['acceptedEngineRevision', 'sourceRevision', 'targets', 'version'], 'aggregate metadata');
  if (metadata.version !== 1 || metadata.acceptedEngineRevision !== ACCEPTED_OLD_REVISION || metadata.sourceRevision !== sourceCommit || !Array.isArray(metadata.targets)) {
    fail('aggregate metadata values are not accepted');
  }
  const expected = PARITY_TARGETS.map(({ key, target, artifact }) => ({ key, target, artifact }));
  if (metadata.targets.length !== expected.length || metadata.targets.some((mapping, index) => {
    exactKeys(mapping, ['artifact', 'key', 'target'], 'aggregate metadata target mapping');
    return mapping.key !== expected[index].key || mapping.target !== expected[index].target || mapping.artifact !== expected[index].artifact;
  })) fail('aggregate metadata target mapping is not accepted');
  const targets = directoryPath(root, 'targets', 'aggregate targets');
  if (JSON.stringify(readdirSync(targets).sort()) !== JSON.stringify(expected.map(({ target }) => target).sort())) fail('aggregate target layout is not accepted');
  return metadata;
}

function verifySource(key, identity, root, target, trustedSourceRoot, sourceRevision) {
  const expectedLabel = EXECUTABLES[key];
  const expectedKeys = ['evidencePath', 'label', 'sha256', 'version'];
  if (key !== 'cli') expectedKeys.push('sourcePath', 'sourceRevision', 'sourceSha256', 'sourceVersion');
  exactKeys(identity, expectedKeys, `executable identity for ${target.key}`);
  if (identity.version !== 1 || identity.label !== expectedLabel || identity.evidencePath !== `executables/${expectedLabel}` || !SHA256_PATTERN.test(identity.sha256)) fail(`executable identity is invalid for ${target.key}`);
  if (sha256(readFileSync(regularPath(root, identity.evidencePath, 'executable evidence'))) !== identity.sha256) fail(`executable evidence hash differs for ${target.key}`);
  if (key === 'cli') return;
  if (identity.sourcePath !== PROJECTOR_SOURCES[key] || identity.sourceRevision !== sourceRevision || identity.sourceVersion !== 1 || !SHA256_PATTERN.test(identity.sourceSha256)) fail(`projector source identity is invalid for ${target.key}`);
  const archived = readFileSync(regularPath(root, `source/${identity.sourcePath}`, 'projector source evidence'));
  if (sha256(archived) !== identity.sourceSha256) fail(`projector source hash differs for ${target.key}`);
  let committed;
  try {
    committed = execFileSync('git', ['-C', trustedSourceRoot, 'cat-file', 'blob', `${sourceRevision}:${identity.sourcePath}`], { env: { ...process.env, GIT_NO_REPLACE_OBJECTS: '1' } });
  } catch {
    fail(`trusted projector source is not committed: ${identity.sourcePath}`);
  }
  if (!archived.equals(committed)) fail(`projector source differs from committed candidate source for ${target.key}`);
}

function verifyComparisons(comparisons, evidenceSide, target) {
  const expected = CANONICAL_ROW_IDS.flatMap((rowId) => SOURCE_CONTRACT.rawFilenames.map((filename) => ({ rowId, filename })));
  if (!Array.isArray(comparisons) || comparisons.length !== expected.length) fail(`comparison rows are incomplete for ${target.key}`);
  for (const [index, comparison] of comparisons.entries()) {
    const { rowId, filename } = expected[index];
    const keys = filename === 'result.json' || filename === 'events.ndjson'
      ? ['filename', 'newRawSha256', 'oldRawSha256', 'rowId', 'semanticSha256']
      : ['filename', 'newRawSha256', 'oldRawSha256', 'rowId'];
    exactKeys(comparison, keys, `comparison for ${target.key}`);
    if (comparison.rowId !== rowId || comparison.filename !== filename || !SHA256_PATTERN.test(comparison.oldRawSha256) || !SHA256_PATTERN.test(comparison.newRawSha256)) fail(`comparison ordering or hash is invalid for ${target.key}`);
    const oldBytes = readFileSync(regularPath(target.root, `old/raw/${rowId}/${filename}`, 'old raw evidence'));
    const newBytes = readFileSync(regularPath(target.root, `${evidenceSide}/raw/${rowId}/${filename}`, 'new raw evidence'));
    if (sha256(oldBytes) !== comparison.oldRawSha256 || sha256(newBytes) !== comparison.newRawSha256) fail(`comparison raw evidence differs for ${target.key}`);
    if (keys.includes('semanticSha256')) {
      const format = filename === 'events.ndjson' ? 'ndjson' : 'json';
      const semantic = semanticDigest(oldBytes, format);
      if (comparison.semanticSha256 !== semantic || semanticDigest(newBytes, format) !== semantic) fail(`comparison semantic evidence differs for ${target.key}`);
    }
  }
}

function verifyTarget(root, target, sourceRevision, trustedSourceRoot) {
  verifyManifest(root, 'bundle-manifest.json', 'SHA256SUMS', TARGET_MANIFEST_EXCLUSIONS, `${target.key} bundle manifest`);
  const parity = readJson(root, 'parity.json', `${target.key} parity.json`);
  exactKeys(parity, ['acceptedEngineRevision', 'cliComparisons', 'comparisons', 'executableIdentities', 'napiComparisons', 'normalizationVersion', 'sourceArtifact', 'sourceContractVersion', 'sourceRevision', 'target', 'targetKey', 'version'], `${target.key} parity.json`);
  if (parity.version !== 1 || parity.targetKey !== target.key || parity.target !== target.target || parity.sourceArtifact !== target.artifact || parity.acceptedEngineRevision !== ACCEPTED_OLD_REVISION || parity.normalizationVersion !== NORMALIZATION_VERSION || parity.sourceContractVersion !== 1 || parity.sourceRevision !== sourceRevision) fail(`parity identity is invalid for ${target.key}`);
  const provenance = readJson(root, 'source-provenance.json', `${target.key} source provenance`);
  exactKeys(provenance, ['sourceRevision', 'sourceVersion', 'trustedSourceRootKind'], `${target.key} source provenance`);
  if (provenance.sourceRevision !== sourceRevision || provenance.sourceVersion !== 1 || typeof provenance.trustedSourceRootKind !== 'string' || !provenance.trustedSourceRootKind) fail(`source provenance is invalid for ${target.key}`);
  exactKeys(parity.executableIdentities, Object.keys(EXECUTABLES), `${target.key} executable identities`);
  for (const key of Object.keys(EXECUTABLES)) verifySource(key, parity.executableIdentities[key], root, target, trustedSourceRoot, sourceRevision);
  const targetContext = { root, key: target.key };
  verifyComparisons(parity.napiComparisons, 'new', targetContext);
  verifyComparisons(parity.cliComparisons, 'projected', targetContext);
  if (JSON.stringify(parity.comparisons) !== JSON.stringify(parity.napiComparisons)) fail(`comparisons must equal napiComparisons for ${target.key}`);
  for (const rowId of CANONICAL_ROW_IDS) {
    for (const filename of CLI_TRANSPORT_FILENAMES) regularPath(root, `cli/raw/${rowId}/${filename}`, 'CLI transport evidence');
    for (const filename of SOURCE_CONTRACT.rawFilenames) regularPath(root, `old/raw/${rowId}/${filename}`, 'old-side source provenance evidence');
  }
  return parity;
}

/** Verifies the complete Task112 aggregate before exposing a selected target. */
export function verifyParityAggregate({ aggregateDirectory, sourceCommit, trustedSourceRoot }) {
  if (!/^[a-f0-9]{40}$/.test(sourceCommit ?? '')) fail('candidate source revision must be a full SHA');
  const root = resolve(aggregateDirectory);
  if (!lstatSync(root).isDirectory()) fail('aggregate directory is not a directory');
  if (JSON.stringify(readdirSync(root).sort()) !== JSON.stringify([...ROOT_FILES].sort())) fail('aggregate root closure is not accepted');
  const metadata = verifyAggregateMetadata(root, sourceCommit);
  verifyManifest(root, 'bundle-manifest.json', 'SHA256SUMS', new Set(['bundle-manifest.json', 'SHA256SUMS']), 'aggregate bundle manifest', true);
  if (typeof trustedSourceRoot !== 'string' || !trustedSourceRoot) fail('trusted source root is required');
  const trustedStats = lstatSync(trustedSourceRoot);
  if (!trustedStats.isDirectory() || trustedStats.isSymbolicLink()) fail('trusted source root must be a non-symlink directory');
  const targets = new Map();
  for (const target of PARITY_TARGETS) {
    const targetRoot = directoryPath(root, `targets/${target.target}`, `target ${target.key}`);
    targets.set(target.target, { root: targetRoot, parity: verifyTarget(targetRoot, target, sourceCommit, trustedSourceRoot) });
  }
  return Object.freeze({ metadata, targets });
}

/* This legacy seam remains solely for the pre-Task89 candidate assembler, which is migrated in the next stage. */
export function validateTrustedParityBundle({ bundleDirectory, cargoTarget, parity, sourceCommit, targetKey }) {
  const root = resolve(bundleDirectory);
  const manifest = readJson(root, 'manifest.json', 'legacy manifest');
  const provenance = readJson(root, 'provenance.json', 'legacy provenance');
  if (provenance.repository !== 'jfet97/min-plane-dfx' || provenance.signerWorkflow !== '.github/workflows/capture-old-rust-parity.yml' || provenance.ref !== 'refs/heads/main' || provenance.sha !== ACCEPTED_OLD_REVISION || !/^[1-9][0-9]*$/.test(String(provenance.runId)) || provenance.artifactName !== PARITY_CONTRACT.archiveName.replace('.tar.gz', '') || provenance.archiveName !== PARITY_CONTRACT.archiveName || !SHA256_PATTERN.test(provenance.archiveSha256 ?? '')) fail('legacy provenance is not accepted');
  if (manifest.targetKey !== targetKey || manifest.targetTriple !== cargoTarget || manifest.old?.sourceRevision !== ACCEPTED_OLD_REVISION || manifest.new?.sourceRevision !== sourceCommit || !Array.isArray(manifest.rows) || manifest.rows.length === 0) fail('legacy manifest is not accepted');
  const checksums = readChecksums(root, 'SHA256SUMS', 'legacy SHA256SUMS');
  const identities = {};
  for (const side of ['old', 'new']) {
    const corpus = [];
    const inputs = [];
    const outputs = [];
    for (const row of [...manifest.rows].sort()) {
      if (typeof row !== 'string' || !/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(row)) fail('legacy manifest row is invalid');
      corpus.push(`${row}\n`);
      for (const filename of ['request.json', 'result.json', 'events.ndjson', 'stderr.txt', 'metadata.json']) {
        const path = `./${side}/raw/${row}/${filename}`;
        const normalized = path.slice(2);
        const bytes = readFileSync(regularPath(root, normalized, 'legacy raw evidence'));
        if (checksums.get(normalized) !== sha256(bytes)) fail(`legacy SHA256SUMS does not match ${normalized}`);
        if (filename === 'request.json') inputs.push(bytes);
        if (filename === 'result.json') outputs.push(Buffer.from(JSON.stringify(JSON.parse(bytes.toString('utf8'))) + '\n'));
        if (filename === 'events.ndjson') outputs.push(Buffer.from(bytes.toString('utf8').split(/\r?\n/).filter(Boolean).map((line) => JSON.stringify(JSON.parse(line))).join('\n') + '\n'));
      }
    }
    identities[side] = {
      corpusSha256: sha256(Buffer.concat(corpus.map((value) => Buffer.from(value)))),
      inputSha256: sha256(Buffer.concat(inputs)),
      outputSha256: sha256(Buffer.concat(outputs)),
    };
  }
  if (identities.old.corpusSha256 !== identities.new.corpusSha256 || identities.old.inputSha256 !== identities.new.inputSha256 || identities.old.outputSha256 !== identities.new.outputSha256) fail('legacy raw evidence differs between old and new');
  for (const side of ['old', 'new']) for (const [name, digest] of Object.entries(identities[side])) if (parity?.[side]?.[name] !== digest) fail(`legacy parity metadata ${side}.${name} differs from raw evidence`);
  return { manifest, provenance, ...identities };
}

export function stageTrustedAggregateParityTarget({ aggregateDirectory, artifactDirectory, cargoTarget, sourceCommit, targetKey, trustedSourceRoot }) {
  const verified = verifyParityAggregate({ aggregateDirectory, sourceCommit, trustedSourceRoot });
  const target = PARITY_TARGETS.find((entry) => entry.target === cargoTarget);
  if (!target || target.key !== targetKey || !verified.targets.has(cargoTarget)) fail('selected target identity is not accepted');
  const source = verified.targets.get(cargoTarget).root;
  const destination = resolve(artifactDirectory);
  mkdirSync(destination, { recursive: true });
  rmSync(join(destination, 'parity-bundle'), { force: true, recursive: true });
  rmSync(join(destination, 'parity.json'), { force: true });
  cpSync(source, join(destination, 'parity-bundle'), { recursive: true, dereference: false });
  copyFileSync(join(source, 'parity.json'), join(destination, 'parity.json'));
}
