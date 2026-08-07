import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { execFile } from 'node:child_process';
import { mkdtemp, mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { promisify } from 'node:util';

import {
  PARITY_AGGREGATE_CONTRACT,
  PARITY_TARGET_LAYOUT,
  assembleParityAggregate,
} from '../../scripts/parity/assemble-parity-aggregate.mjs';
import {
  ACCEPTED_OLD_REVISION,
  CANONICAL_ROW_IDS,
  NORMALIZATION_VERSION,
  SOURCE_CONTRACT,
  semanticDigest,
} from '../../scripts/parity/verify-parity-bundle.mjs';

const TRUSTED_SOURCE_ROOT = process.cwd();
const TRUSTED_REVISION = 'b1733423aad1f1d020f2dbfc30b7a8c9162373ae';
const execFileAsync = promisify(execFile);

async function git(arguments_, cwd) {
  return execFileAsync('git', arguments_, { cwd });
}

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

async function targetArtifact(root, target) {
  const directory = join(root, `old-new-parity-target-${target.key}`);
  const comparisons = [];
  for (const rowId of CANONICAL_ROW_IDS) {
    for (const filename of SOURCE_CONTRACT.rawFilenames) {
      const bytes = Buffer.from(filename === 'result.json' ? '{"result":"ok","runtimeMs":1}\n' : filename === 'events.ndjson' ? '{"kind":"progress","elapsedMs":2}\n' : filename === 'request.json' ? '{"desktop":true,"options":{"diagnosticTraceMode":"full"}}\n' : filename === 'process.json' ? `{"rowId":"${rowId}"}\n` : '');
      for (const side of ['old', 'new', 'projected']) {
        await mkdir(join(directory, side, 'raw', rowId), { recursive: true });
        await writeFile(join(directory, side, 'raw', rowId, filename), bytes);
      }
      comparisons.push({ rowId, filename, oldRawSha256: sha256(bytes), newRawSha256: sha256(bytes), ...((filename === 'result.json' || filename === 'events.ndjson') ? { semanticSha256: semanticDigest(bytes, filename === 'events.ndjson' ? 'ndjson' : 'json') } : {}) });
    }
    const cliFiles = ['adapted-request.json', 'adapter-stderr.txt', 'adapter-process.json', 'result.json', 'events.ndjson', 'stderr.txt', 'process.json', 'projected-result.json', 'projected-events.ndjson', 'outcome-projector-process.json', 'events-projector-process.json'];
    await mkdir(join(directory, 'cli', 'raw', rowId), { recursive: true });
    for (const filename of cliFiles) await writeFile(join(directory, 'cli', 'raw', rowId, filename), `${filename}\n`);
  }
  const projectorSources = { adapter: 'crates/polygon-nesting-napi/src/bin/parity-desktop-request-adapter.rs', outcomeProjector: 'crates/polygon-nesting-napi/src/bin/parity-project-engine-outcome.rs', eventsProjector: 'crates/polygon-nesting-napi/src/bin/parity-project-engine-events.rs' };
  const sourceIdentities = {};
  for (const [key, sourcePath] of Object.entries(projectorSources)) {
    const sourceBytes = await readFile(new URL(`../../${sourcePath}`, import.meta.url));
    await mkdir(join(directory, 'source', ...sourcePath.split('/').slice(0, -1)), { recursive: true });
    await writeFile(join(directory, 'source', sourcePath), sourceBytes);
    sourceIdentities[key] = { sourcePath, sourceSha256: sha256(sourceBytes), sourceVersion: 1, sourceRevision: TRUSTED_REVISION };
  }
  const executableIdentities = {};
  for (const [key, label] of [['adapter', 'parity-desktop-request-adapter'], ['cli', 'polygon-nesting'], ['outcomeProjector', 'parity-project-engine-outcome'], ['eventsProjector', 'parity-project-engine-events']]) {
    const bytes = Buffer.from(`binary ${label}\n`);
    await mkdir(join(directory, 'executables'), { recursive: true });
    await writeFile(join(directory, 'executables', label), bytes);
    executableIdentities[key] = { label, evidencePath: `executables/${label}`, version: 1, sha256: sha256(bytes), ...(sourceIdentities[key] ?? {}) };
  }
  await writeFile(join(directory, 'parity.json'), `${JSON.stringify({ version: 1, targetKey: target.key, target: target.target, acceptedEngineRevision: ACCEPTED_OLD_REVISION, sourceArtifact: target.artifact, normalizationVersion: NORMALIZATION_VERSION, sourceContractVersion: 1, sourceRevision: TRUSTED_REVISION, executableIdentities, comparisons, napiComparisons: comparisons, cliComparisons: comparisons })}\n`);
  const files = [];
  async function collect(path, relativePath = '') {
    for (const entry of await readdir(path, { withFileTypes: true })) {
      const nextPath = join(path, entry.name);
      const nextRelative = relativePath ? `${relativePath}/${entry.name}` : entry.name;
      if (entry.isDirectory()) await collect(nextPath, nextRelative);
      else files.push(nextRelative);
    }
  }
  await collect(directory);
  const entries = await Promise.all(files.map(async (path) => {
    const bytes = await readFile(join(directory, path));
    return { path, sha256: sha256(bytes), size: bytes.length };
  }));
  await writeFile(join(directory, 'bundle-manifest.json'), `${JSON.stringify({ version: 1, files: entries })}\n`);
  await writeFile(join(directory, 'SHA256SUMS'), `${entries.map(({ path, sha256: digest }) => `${digest}  ${path}`).join('\n')}\n`);
}

async function refreshTargetIntegrity(directory) {
  const files = [];
  async function collect(path, relativePath = '') {
    for (const entry of await readdir(path, { withFileTypes: true })) {
      const nextPath = join(path, entry.name);
      const nextRelative = relativePath ? `${relativePath}/${entry.name}` : entry.name;
      if (entry.isDirectory()) await collect(nextPath, nextRelative);
      else if (nextRelative !== 'bundle-manifest.json' && nextRelative !== 'SHA256SUMS') files.push(nextRelative);
    }
  }
  await collect(directory);
  const entries = await Promise.all(files.sort().map(async (path) => {
    const bytes = await readFile(join(directory, path));
    return { path, sha256: sha256(bytes), size: bytes.length };
  }));
  await writeFile(join(directory, 'bundle-manifest.json'), `${JSON.stringify({ version: 1, files: entries })}\n`);
  await writeFile(join(directory, 'SHA256SUMS'), `${entries.map(({ path, sha256: digest }) => `${digest}  ${path}`).join('\n')}\n`);
}

async function trustedRepository() {
  const root = await mkdtemp(join(tmpdir(), 'trusted-projector-source-'));
  const paths = [
    'crates/polygon-nesting-napi/src/bin/parity-desktop-request-adapter.rs',
    'crates/polygon-nesting-napi/src/bin/parity-project-engine-outcome.rs',
    'crates/polygon-nesting-napi/src/bin/parity-project-engine-events.rs',
  ];
  for (const sourcePath of paths) {
    await mkdir(join(root, ...sourcePath.split('/').slice(0, -1)), { recursive: true });
    await writeFile(join(root, sourcePath), await readFile(new URL(`../../${sourcePath}`, import.meta.url)));
  }
  await git(['init'], root);
  await git(['config', 'user.email', 'parity@example.test'], root);
  await git(['config', 'user.name', 'Parity Test'], root);
  await git(['add', '.'], root);
  await git(['commit', '-m', 'trusted projector sources'], root);
  const { stdout } = await git(['rev-parse', 'HEAD'], root);
  return { root, revision: stdout.trim() };
}

async function aggregateFixture() {
  const root = await mkdtemp(join(tmpdir(), 'parity-aggregate-'));
  const input = join(root, 'input');
  for (const target of PARITY_TARGET_LAYOUT) await targetArtifact(input, target);
  const trusted = await trustedRepository();
  for (const target of PARITY_TARGET_LAYOUT) {
    const directory = join(input, `old-new-parity-target-${target.key}`);
    const parityPath = join(directory, 'parity.json');
    const parity = JSON.parse(await readFile(parityPath));
    parity.sourceRevision = trusted.revision;
    for (const key of ['adapter', 'outcomeProjector', 'eventsProjector']) {
      parity.executableIdentities[key].sourceRevision = trusted.revision;
    }
    await writeFile(parityPath, `${JSON.stringify(parity)}\n`);
    await refreshTargetIntegrity(directory);
  }
  return { root, input, output: join(root, 'output'), trusted };
}

test('defines the immutable aggregate artifact names and target layout', () => {
  assert.deepEqual(PARITY_AGGREGATE_CONTRACT, {
    version: 1,
    artifactName: 'old-new-parity-bundle',
    archiveName: 'old-new-parity-bundle.tar.gz',
    digestName: 'old-new-parity-bundle.tar.gz.sha256',
    aggregateMetadata: 'aggregate-metadata.json',
    bundleManifest: 'bundle-manifest.json',
    checksums: 'SHA256SUMS',
    targetsDirectory: 'targets',
  });
  assert.equal(typeof assembleParityAggregate, 'function');
});

test('rejects a trusted checkout whose revision differs from aggregate revision', async () => {
  const fixture = await aggregateFixture();
  await assert.rejects(
    () => assembleParityAggregate({
      inputDirectory: fixture.input,
      outputDirectory: fixture.output,
      sourceRevision: '0'.repeat(40),
      trustedSourceRoot: TRUSTED_SOURCE_ROOT,
    }),
    /trusted source root revision does not match aggregate revision/,
  );
});

test('rejects a per-target evidence manifest whose hash differs from its raw bytes', async () => {
  const fixture = await aggregateFixture();
  await writeFile(join(fixture.input, 'old-new-parity-target-linux-x64', 'new', 'raw', CANONICAL_ROW_IDS[0], 'result.json'), '{"ok":false}\n');
  await assert.rejects(
    () => assembleParityAggregate({
      inputDirectory: fixture.input,
      outputDirectory: fixture.output,
      sourceRevision: fixture.trusted.revision,
      trustedSourceRoot: fixture.trusted.root,
    }),
    /manifest hash differs/,
  );
});

test('rejects an unlisted aggregate input root file', async () => {
  const fixture = await aggregateFixture();
  await writeFile(join(fixture.input, 'UNLISTED.txt'), 'untrusted');
  await assert.rejects(
    () => assembleParityAggregate({
      inputDirectory: fixture.input,
      outputDirectory: fixture.output,
      sourceRevision: fixture.trusted.revision,
      trustedSourceRoot: fixture.trusted.root,
    }),
    /unlisted root entry/,
  );
});

test('rejects unmanifested files in a target artifact', async () => {
  const fixture = await aggregateFixture();
  await writeFile(join(fixture.input, 'old-new-parity-target-linux-x64', 'unmanifested.txt'), 'untrusted');
  await assert.rejects(
    () => assembleParityAggregate({
      inputDirectory: fixture.input,
      outputDirectory: fixture.output,
      sourceRevision: fixture.trusted.revision,
      trustedSourceRoot: fixture.trusted.root,
    }),
    /manifest does not enumerate/,
  );
});

test('rejects dirty trusted worktree projector bytes despite matching HEAD', async () => {
  const fixture = await aggregateFixture();
  const trusted = await trustedRepository();
  for (const target of PARITY_TARGET_LAYOUT) {
    const directory = join(fixture.input, `old-new-parity-target-${target.key}`);
    const parityPath = join(directory, 'parity.json');
    const parity = JSON.parse(await readFile(parityPath));
    parity.sourceRevision = trusted.revision;
    for (const key of ['adapter', 'outcomeProjector', 'eventsProjector']) {
      parity.executableIdentities[key].sourceRevision = trusted.revision;
    }
    await writeFile(parityPath, `${JSON.stringify(parity)}\n`);
    await refreshTargetIntegrity(directory);
  }
  const sourcePath = 'crates/polygon-nesting-napi/src/bin/parity-desktop-request-adapter.rs';
  const dirty = Buffer.from('fn dirty_worktree_source() {}\n');
  await writeFile(join(trusted.root, sourcePath), dirty);
  for (const target of PARITY_TARGET_LAYOUT) {
    const directory = join(fixture.input, `old-new-parity-target-${target.key}`);
    const parityPath = join(directory, 'parity.json');
    const parity = JSON.parse(await readFile(parityPath));
    await writeFile(join(directory, 'source', sourcePath), dirty);
    parity.executableIdentities.adapter.sourceSha256 = sha256(dirty);
    await writeFile(parityPath, `${JSON.stringify(parity)}\n`);
    await refreshTargetIntegrity(directory);
  }
  await assert.rejects(
    () => assembleParityAggregate({
      inputDirectory: fixture.input,
      outputDirectory: fixture.output,
      sourceRevision: trusted.revision,
      trustedSourceRoot: trusted.root,
    }),
    /differs from trusted checkout/,
  );
});

test('rejects rehashed archived projector source that differs from trusted checkout', async () => {
  const fixture = await aggregateFixture();
  const directory = join(fixture.input, 'old-new-parity-target-linux-x64');
  const parityPath = join(directory, 'parity.json');
  const parity = JSON.parse(await readFile(parityPath));
  const identity = parity.executableIdentities.adapter;
  const sourcePath = join(directory, 'source', identity.sourcePath);
  const mutated = Buffer.from('fn forged_source() {}\n');
  await writeFile(sourcePath, mutated);
  identity.sourceSha256 = sha256(mutated);
  await writeFile(parityPath, `${JSON.stringify(parity)}\n`);
  await refreshTargetIntegrity(directory);
  await assert.rejects(
    () => assembleParityAggregate({
      inputDirectory: fixture.input,
      outputDirectory: fixture.output,
      sourceRevision: fixture.trusted.revision,
      trustedSourceRoot: fixture.trusted.root,
    }),
    /differs from trusted checkout/,
  );
});

test('writes the exact archive and digest sidecar from four verified target artifacts', async () => {
  const fixture = await aggregateFixture();
  const result = await assembleParityAggregate({
    inputDirectory: fixture.input,
    outputDirectory: fixture.output,
    sourceRevision: fixture.trusted.revision,
      trustedSourceRoot: fixture.trusted.root,
  });
  assert.equal(result.archivePath, join(fixture.output, PARITY_AGGREGATE_CONTRACT.archiveName));
  assert.equal(
    await readFile(result.digestPath, 'utf8'),
    `${sha256(await readFile(result.archivePath))}  ${PARITY_AGGREGATE_CONTRACT.archiveName}\n`,
  );
  const { stdout } = await import('node:child_process').then(({ execFile }) => new Promise((resolve, reject) => execFile('tar', ['-tzf', result.archivePath], (error, stdout) => error ? reject(error) : resolve({ stdout }))));
  for (const { target } of PARITY_TARGET_LAYOUT) assert.match(stdout, new RegExp(`targets/${target}/`));
});

test('produces byte-identical aggregate archives from unchanged verified target artifacts', async () => {
  const fixture = await aggregateFixture();
  const first = await assembleParityAggregate({
    inputDirectory: fixture.input,
    outputDirectory: join(fixture.root, 'first'),
    sourceRevision: fixture.trusted.revision,
      trustedSourceRoot: fixture.trusted.root,
  });
  const second = await assembleParityAggregate({
    inputDirectory: fixture.input,
    outputDirectory: join(fixture.root, 'second'),
    sourceRevision: fixture.trusted.revision,
      trustedSourceRoot: fixture.trusted.root,
  });
  assert.deepEqual(await readFile(first.archivePath), await readFile(second.archivePath));
});
