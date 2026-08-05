import { execFile } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtemp, mkdir, readdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';

import {
  PARITY_AGGREGATE_CONTRACT,
  PARITY_TARGET_LAYOUT,
  assembleParityAggregate,
} from '../../../scripts/parity/assemble-parity-aggregate.mjs';
import {
  ACCEPTED_OLD_REVISION,
  CANONICAL_ROW_IDS,
  NORMALIZATION_VERSION,
  SOURCE_CONTRACT,
  semanticDigest,
} from '../../../scripts/parity/verify-parity-bundle.mjs';

const execFileAsync = promisify(execFile);
const REPOSITORY_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../../..');
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
  'adapted-request.json',
  'adapter-stderr.txt',
  'adapter-process.json',
  'result.json',
  'events.ndjson',
  'stderr.txt',
  'process.json',
  'projected-result.json',
  'projected-events.ndjson',
  'outcome-projector-process.json',
  'events-projector-process.json',
]);

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

async function git(arguments_, cwd, environment = {}) {
  return execFileAsync('git', arguments_, { cwd, env: { ...process.env, ...environment } });
}

async function writeJson(path, value) {
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, `${canonicalJson(value)}\n`);
}

async function committedProjectorSources() {
  const sources = {};
  for (const [key, sourcePath] of Object.entries(PROJECTOR_SOURCES)) {
    const { stdout } = await git(['show', `HEAD:${sourcePath}`], REPOSITORY_ROOT);
    sources[key] = Buffer.from(stdout);
  }
  return sources;
}

async function createTrustedProjectorRepository(root) {
  const trustedSourceRoot = join(root, 'trusted-projectors');
  const sources = await committedProjectorSources();
  for (const [key, sourcePath] of Object.entries(PROJECTOR_SOURCES)) {
    const path = join(trustedSourceRoot, sourcePath);
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, sources[key]);
  }
  for (const sourcePath of ['Cargo.lock', 'crates/polygon-nesting-napi/Cargo.toml']) {
    const { stdout } = await git(['show', `HEAD:${sourcePath}`], REPOSITORY_ROOT);
    const path = join(trustedSourceRoot, sourcePath);
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, stdout);
  }
  await git(['init'], trustedSourceRoot);
  await git(['config', 'user.email', 'parity-fixture@example.test'], trustedSourceRoot);
  await git(['config', 'user.name', 'Task112 Parity Fixture'], trustedSourceRoot);
  await git(['add', '.'], trustedSourceRoot);
  await git(['commit', '-m', 'trusted projector sources'], trustedSourceRoot, {
    GIT_AUTHOR_DATE: '2000-01-01T00:00:00Z',
    GIT_COMMITTER_DATE: '2000-01-01T00:00:00Z',
  });
  const { stdout } = await git(['rev-parse', 'HEAD'], trustedSourceRoot);
  return { sources, trustedSourceRoot, sourceRevision: stdout.trim() };
}

function rawBytes(filename, rowId, side) {
  if (filename === 'request.json') return Buffer.from(`${canonicalJson({ request: { desktop: true, rowId } })}\n`);
  if (filename === 'result.json') return Buffer.from(`${canonicalJson({ result: { placements: [rowId], status: 'ok' }, runtimeMs: side === 'old' ? 1 : side === 'new' ? 9 : 17 })}\n`);
  if (filename === 'events.ndjson') return Buffer.from(`${canonicalJson({ elapsedMs: side === 'old' ? 2 : side === 'new' ? 10 : 18, event: 'completed', rowId })}\n`);
  if (filename === 'stderr.txt') return Buffer.from('');
  return Buffer.from(`${canonicalJson({ exitCode: 0, rowId })}\n`);
}

async function collectEvidenceFiles(root, relativePath = '') {
  const files = [];
  for (const entry of (await readdir(root, { withFileTypes: true })).sort((left, right) => left.name.localeCompare(right.name))) {
    const nextRelative = relativePath ? `${relativePath}/${entry.name}` : entry.name;
    const path = join(root, entry.name);
    if (entry.isDirectory()) files.push(...await collectEvidenceFiles(path, nextRelative));
    else files.push(nextRelative);
  }
  return files;
}

async function writeTargetIntegrity(root) {
  const files = (await collectEvidenceFiles(root)).filter((path) => path !== 'bundle-manifest.json' && path !== 'SHA256SUMS').sort();
  const entries = await Promise.all(files.map(async (path) => {
    const bytes = await readFile(join(root, path));
    return { path, sha256: sha256(bytes), size: bytes.length };
  }));
  await writeJson(join(root, 'bundle-manifest.json'), { version: 1, files: entries });
  await writeFile(join(root, 'SHA256SUMS'), `${entries.map(({ path, sha256: digest }) => `${digest}  ${path}`).join('\n')}\n`);
}

async function writeTargetInput(inputDirectory, target, trusted) {
  const root = join(inputDirectory, `old-new-parity-target-${target.key}`);
  const comparisons = [];
  const cliComparisons = [];
  for (const rowId of CANONICAL_ROW_IDS) {
    for (const filename of SOURCE_CONTRACT.rawFilenames) {
      const oldBytes = rawBytes(filename, rowId, 'old');
      const newBytes = rawBytes(filename, rowId, 'new');
      const projectedBytes = rawBytes(filename, rowId, 'projected');
      for (const [side, bytes] of [['old', oldBytes], ['new', newBytes], ['projected', projectedBytes]]) {
        const path = join(root, side, 'raw', rowId, filename);
        await mkdir(dirname(path), { recursive: true });
        await writeFile(path, bytes);
      }
      const comparison = {
        rowId,
        filename,
        oldRawSha256: sha256(oldBytes),
        newRawSha256: sha256(newBytes),
        ...((filename === 'result.json' || filename === 'events.ndjson') ? {
          semanticSha256: semanticDigest(oldBytes, filename === 'events.ndjson' ? 'ndjson' : 'json'),
        } : {}),
      };
      comparisons.push(comparison);
      cliComparisons.push({ ...comparison, newRawSha256: sha256(projectedBytes) });
    }
    for (const filename of CLI_TRANSPORT_FILENAMES) {
      const path = join(root, 'cli', 'raw', rowId, filename);
      await mkdir(dirname(path), { recursive: true });
      await writeFile(path, `${canonicalJson({ filename, rowId, transportVersion: 1 })}\n`);
    }
  }

  const executableIdentities = {};
  for (const [key, label] of Object.entries(EXECUTABLES)) {
    const bytes = Buffer.from(`Task112 fixture executable ${target.target} ${label}\n`);
    await mkdir(join(root, 'executables'), { recursive: true });
    await writeFile(join(root, 'executables', label), bytes);
    executableIdentities[key] = { label, evidencePath: `executables/${label}`, sha256: sha256(bytes), version: 1 };
  }
  for (const [key, sourcePath] of Object.entries(PROJECTOR_SOURCES)) {
    const bytes = trusted.sources[key];
    const path = join(root, 'source', sourcePath);
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, bytes);
    Object.assign(executableIdentities[key], {
      sourcePath,
      sourceRevision: trusted.sourceRevision,
      sourceSha256: sha256(bytes),
      sourceVersion: 1,
    });
  }

  await writeJson(join(root, 'source-provenance.json'), {
    sourceRevision: trusted.sourceRevision,
    sourceVersion: 1,
    trustedSourceRootKind: 'isolated-temporary-git-repository',
  });
  await writeJson(join(root, 'parity.json'), {
    acceptedEngineRevision: ACCEPTED_OLD_REVISION,
    cliComparisons,
    comparisons,
    executableIdentities,
    napiComparisons: comparisons,
    normalizationVersion: NORMALIZATION_VERSION,
    sourceArtifact: target.artifact,
    sourceContractVersion: 1,
    sourceRevision: trusted.sourceRevision,
    target: target.target,
    targetKey: target.key,
    version: 1,
  });
  await writeTargetIntegrity(root);
}

/** Builds Task112 v1 target evidence and assembles it with the local production assembler. */
export async function createTask112ParityFixture() {
  const root = await mkdtemp(join(tmpdir(), 'task112-parity-fixture-'));
  const inputDirectory = join(root, 'input');
  const trusted = await createTrustedProjectorRepository(root);
  for (const target of PARITY_TARGET_LAYOUT) await writeTargetInput(inputDirectory, target, trusted);

  const outputDirectory = join(root, 'aggregate');
  const repeatOutputDirectory = join(root, 'aggregate-repeat');
  const first = await assembleParityAggregate({
    inputDirectory,
    outputDirectory,
    sourceRevision: trusted.sourceRevision,
    trustedSourceRoot: trusted.trustedSourceRoot,
  });
  const repeat = await assembleParityAggregate({
    inputDirectory,
    outputDirectory: repeatOutputDirectory,
    sourceRevision: trusted.sourceRevision,
    trustedSourceRoot: trusted.trustedSourceRoot,
  });
  const targetDirectories = Object.fromEntries(PARITY_TARGET_LAYOUT.map((target) => [
    target.target,
    join(outputDirectory, 'staging', PARITY_AGGREGATE_CONTRACT.targetsDirectory, target.target),
  ]));
  return {
    ...first,
    root,
    inputDirectory,
    outputDirectory,
    aggregateDirectory: join(outputDirectory, 'staging'),
    sourceRevision: trusted.sourceRevision,
    targetDirectories,
    targets: PARITY_TARGET_LAYOUT,
    trustedSourceRoot: trusted.trustedSourceRoot,
    repeat,
    cleanup: () => rm(root, { recursive: true, force: true }),
  };
}
