import assert from 'node:assert/strict';
import { chmod, link, lstat, mkdtemp, mkdir, readFile, readdir, rm, symlink, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { tmpdir } from 'node:os';
import { delimiter, join, posix, win32 } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { gzipSync } from 'node:zlib';
import test from 'node:test';

import {
  ParityValidationError,
  verifyParityDirectory,
  normalizeSemanticJson,
  verifyDependencyIdentity,
  compareParityRows,
  CANONICAL_ROW_IDS,
  ACCEPTED_NATIVE_DEPENDENCIES,
  ACCEPTED_NATIVE_DEPENDENCIES_SHA256,
  PARITY_TARGETS,
  SOURCE_CONTRACT,
} from '../../scripts/parity/verify-parity-bundle.mjs';
import * as sourceParityBundle from '../../scripts/parity/fetch-source-parity-bundle.mjs';

const {
  assertSafeArchive,
  attestationVerificationArgs,
  publicationStagingTemplate,
  requireDisjointDestinations,
} = sourceParityBundle;

const SOURCE = {
  repository: SOURCE_CONTRACT.repository,
  workflow: SOURCE_CONTRACT.workflow,
  ref: SOURCE_CONTRACT.ref,
  sha: SOURCE_CONTRACT.workflowSupportRevision,
  artifact: 'old-rust-parity-capture-aarch64-apple-darwin',
};

async function fixture({ withBuildIdentity = false } = {}) {
  const root = await mkdtemp(join(tmpdir(), 'parity-consumer-'));
  const fixtureRows = withBuildIdentity ? CANONICAL_ROW_IDS : ['desktop-01'];
  for (const rowId of fixtureRows) {
    const row = join(root, 'old', 'raw', rowId);
    await mkdir(row, { recursive: true });
    await writeFile(join(row, 'request.json'), '{"desktop":true,"options":{"diagnosticTraceMode":"full"}}\n');
    await writeFile(join(row, 'result.json'), '{"result":"ok","runtimeMs":1}\n');
    await writeFile(join(row, 'events.ndjson'), '{"kind":"portfolio-progress","elapsedMs":2}\n');
    await writeFile(join(row, 'stderr.txt'), '');
    await writeFile(join(row, 'process.json'), JSON.stringify({ rowId }));
  }
  const rawPaths = [
    'request.json', 'result.json', 'events.ndjson', 'stderr.txt', 'process.json',
  ];
  const files = (await Promise.all(fixtureRows.flatMap((rowId) => rawPaths.map(async (filename) => {
    const path = `old/raw/${rowId}/${filename}`;
    const bytes = await readFile(join(root, path));
    return { path, sha256: createHash('sha256').update(bytes).digest('hex'), size: bytes.length };
  })))).flat();
  await writeFile(join(root, SOURCE_CONTRACT.captureMetadata), JSON.stringify({
    version: 1,
    acceptedEngineRevision: SOURCE_CONTRACT.acceptedEngineRevision,
    sourceProvenanceRevision: SOURCE_CONTRACT.sourceProvenanceRevision,
    target: 'aarch64-apple-darwin',
    toolchain: '1.95.0',
    artifactName: SOURCE.artifact,
    build: { profile: 'release', features: [] },
    rustc: { identity: 'rustc 1.95.0', verbose: 'release: 1.95.0' },
    cargo: { identity: 'cargo 1.95.0' },
    sourceCargoLockSha256: 'c'.repeat(64),
    nativeDependencies: {
      entries: ACCEPTED_NATIVE_DEPENDENCIES,
      sha256: ACCEPTED_NATIVE_DEPENDENCIES_SHA256,
    },
    addon: {
      historicalSha256: '9fc447f80a820c60676eee62706694c7f7ac79092a66ac131ac50b4f216dec9b',
      freshSha256: 'e'.repeat(64),
    },
    workflow: { repository: SOURCE.repository, ref: SOURCE.ref, sha: SOURCE.sha, runId: '1', runAttempt: '1' },
    corpus: { manifestName: 'migration-corpus.json', manifestSha256: 'a'.repeat(64), sha256SumsSha256: 'b'.repeat(64), rowIds: fixtureRows },
    raw: {
      version: 1,
      files: files.map(({ path, sha256, size }) => ({ path: path.slice('old/raw/'.length), sha256, size })),
    },
  }, null, 2));
  const metadataBytes = await readFile(join(root, SOURCE_CONTRACT.captureMetadata));
  files.push({
    path: SOURCE_CONTRACT.captureMetadata,
    sha256: createHash('sha256').update(metadataBytes).digest('hex'),
    size: metadataBytes.length,
  });
  await writeFile(join(root, SOURCE_CONTRACT.bundleManifest), JSON.stringify({ version: 1, files }, null, 2));
  await writeFile(join(root, 'SHA256SUMS'), `${files.map(({ path, sha256 }) => `${sha256}  ${path}`).join('\n')}\n`);
  return root;
}

const execFileAsync = promisify(execFile);

function expectFailure(action, message) {
  return assert.rejects(action, (error) => error instanceof ParityValidationError && error.message.includes(message));
}

async function writeTarMember(archive, name, type = '0') {
  const header = Buffer.alloc(512);
  Buffer.from(name).copy(header, 0);
  header.write('0000644\0', 100, 'ascii');
  header.write('0000000\0', 108, 'ascii');
  header.write('0000000\0', 116, 'ascii');
  header.write('00000000000\0', 124, 'ascii');
  header.write('00000000000\0', 136, 'ascii');
  header.fill(0x20, 148, 156);
  header.write(type, 156, 'ascii');
  header.write('ustar\0', 257, 'ascii');
  header.write('00', 263, 'ascii');
  const checksum = header.reduce((total, byte) => total + byte, 0);
  header.write(`${checksum.toString(8).padStart(6, '0')}\0 `, 148, 'ascii');
  await writeFile(archive, gzipSync(Buffer.concat([header, Buffer.alloc(1024)])));
}

test('pins the complete Task109 canonical dependency output', () => {
  assert.equal(ACCEPTED_NATIVE_DEPENDENCIES.length, 61);
  assert.equal(ACCEPTED_NATIVE_DEPENDENCIES_SHA256, '8925ac904fa2eb41a3f82907d530578a5174509eef0470712193cc4d45a3d0c8');
  assert.equal(
    createHash('sha256').update(`${JSON.stringify(ACCEPTED_NATIVE_DEPENDENCIES)}\n`).digest('hex'),
    ACCEPTED_NATIVE_DEPENDENCIES_SHA256,
  );
});

test('centralizes the exact attested old-side archive contract', () => {
  assert.deepEqual(SOURCE_CONTRACT, {
    repository: 'jfet07-polygon-labs/min-plane-dxf',
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
});

test('uses the documented signer-workflow identity format for attestation verification', () => {
  assert.deepEqual(attestationVerificationArgs('/tmp/capture.tar.gz'), [
    'attestation',
    'verify',
    '/tmp/capture.tar.gz',
    '--repo',
    'jfet07-polygon-labs/min-plane-dxf',
    '--signer-workflow',
    'jfet07-polygon-labs/min-plane-dxf/.github/workflows/capture-old-rust-parity.yml',
  ]);
});

test('optional parity workflow preserves the transferred source identity without gating CI', async () => {
  const standalone = await readFile(new URL('../../.github/workflows/standalone-parity.yml', import.meta.url), 'utf8');
  const ci = await readFile(new URL('../../.github/workflows/ci.yml', import.meta.url), 'utf8');
  assert.match(standalone, /Source jfet07-polygon-labs\/min-plane-dxf workflow run ID/);
  assert.match(standalone, /old-new-parity-target-\$\{\{ matrix\.key \}\}/);
  assert.doesNotMatch(standalone, /old-new-parity-bundle|blacksmith-2vcpu-windows|x86_64-pc-windows-msvc/);
  assert.doesNotMatch(standalone, /jfet97\/min-plane-dxf/);
  assert.doesNotMatch(ci, /old-new-parity-bundle|standalone-parity\.yml|jfet97\/polygon-nesting/);
});

test('forces Windows tar to treat native paths as local and preserves its destination', () => {
  assert.deepEqual(
    sourceParityBundle.extractionArgs(
      'C:\\actions\\temp\\capture.tar.gz',
      'D:\\a\\polygon-nesting\\artifacts\\polygon-source-parity-stage-123',
      'win32',
    ),
    [
      '--force-local',
      '-xzf',
      'C:\\actions\\temp\\capture.tar.gz',
      '--no-same-owner',
      '-C',
      'D:/a/polygon-nesting/artifacts/polygon-source-parity-stage-123',
    ],
  );
  assert.deepEqual(sourceParityBundle.extractionArgs('/tmp/capture.tar.gz', '/tmp/staging', 'linux'), [
    '-xzf',
    '/tmp/capture.tar.gz',
    '--no-same-owner',
    '-C',
    '/tmp/staging',
  ]);
  assert.deepEqual(sourceParityBundle.extractionArgs('/tmp/capture.tar.gz', '/tmp/staging', 'darwin'), [
    '-xzf',
    '/tmp/capture.tar.gz',
    '--no-same-owner',
    '-C',
    '/tmp/staging',
  ]);
});

test('stages final publication beside its output on every supported platform', () => {
  const windowsOutput = 'D:\\a\\polygon-nesting\\polygon-nesting\\artifacts\\trusted-parity-bundle\\x86_64-pc-windows-msvc';
  const windowsStaging = publicationStagingTemplate(windowsOutput, 'win32');
  assert.equal(windowsStaging, 'D:\\a\\polygon-nesting\\polygon-nesting\\artifacts\\trusted-parity-bundle\\polygon-source-parity-stage-');
  assert.equal(win32.parse(windowsStaging).root, win32.parse(windowsOutput).root);

  for (const platform of ['linux', 'darwin']) {
    const output = '/workspace/artifacts/trusted-parity-bundle/x86_64-unknown-linux-gnu';
    assert.equal(
      publicationStagingTemplate(output, platform),
      posix.join(posix.dirname(output), 'polygon-source-parity-stage-'),
    );
  }
});

test('cleans a newly created output parent after extraction failure without deleting an existing parent', async () => {
  const root = await mkdtemp(join(tmpdir(), 'parity-output-parent-cleanup-'));
  const source = await fixture({ withBuildIdentity: true });
  const archiveName = `${SOURCE_CONTRACT.archivePrefix}aarch64-apple-darwin${SOURCE_CONTRACT.archiveSuffix}`;
  const digestName = `${SOURCE_CONTRACT.archivePrefix}aarch64-apple-darwin${SOURCE_CONTRACT.archiveDigestSuffix}`;
  const archive = join(root, archiveName);
  const digest = join(root, digestName);
  const bin = join(root, 'bin');
  const fakeGh = join(bin, 'fake-gh.mjs');
  await mkdir(bin);
  await execFileAsync('tar', ['--format=ustar', '-czf', archive, '-C', source, '.']);
  await writeFile(digest, `${createHash('sha256').update(await readFile(archive)).digest('hex')}  ${archiveName}\n`);
  await writeFile(fakeGh, `import { copyFileSync } from 'node:fs';
const args = process.argv.slice(2);
if (args[0] === 'run' && args[1] === 'view') process.stdout.write('${JSON.stringify({ headSha: SOURCE_CONTRACT.workflowSupportRevision, headBranch: 'main', workflowName: SOURCE_CONTRACT.workflowName })}');
else if (args[0] === 'run' && args[1] === 'download') {
  const directory = args[args.indexOf('--dir') + 1];
  copyFileSync(process.env.PARITY_ARCHIVE, directory + '/' + process.env.PARITY_ARCHIVE_NAME);
  copyFileSync(process.env.PARITY_DIGEST, directory + '/' + process.env.PARITY_DIGEST_NAME);
}`);
  const suffix = process.platform === 'win32' ? '.cmd' : '';
  await writeFile(join(bin, `gh${suffix}`), process.platform === 'win32'
    ? `@echo off\r\n"${process.execPath}" "${fakeGh}" %*\r\n`
    : `#!/bin/sh\nexec "${process.execPath}" "${fakeGh}" "$@"\n`);
  await writeFile(join(bin, `tar${suffix}`), process.platform === 'win32'
    ? '@if not "%PARITY_CONCURRENT_FILE%"=="" echo concurrent> "%PARITY_CONCURRENT_FILE%"\r\n@echo extraction failed 1>&2\r\nexit /b 1\r\n'
    : '#!/bin/sh\nif [ -n "$PARITY_CONCURRENT_FILE" ]; then printf "%s" concurrent > "$PARITY_CONCURRENT_FILE"; fi\nprintf "%s\\n" extraction-failed >&2\nexit 1\n');
  if (process.platform !== 'win32') {
    await chmod(join(bin, 'gh'), 0o755);
    await chmod(join(bin, 'tar'), 0o755);
  }
  const environment = {
    ...process.env,
    PARITY_ARCHIVE: archive,
    PARITY_ARCHIVE_NAME: archiveName,
    PARITY_DIGEST: digest,
    PARITY_DIGEST_NAME: digestName,
    PATH: `${bin}${delimiter}${process.env.PATH}`,
  };
  const run = (output, provenanceOutput, overrides = {}) => execFileAsync(process.execPath, [
    'scripts/parity/fetch-source-parity-bundle.mjs',
    '--source-run', '1',
    '--target', 'aarch64-apple-darwin',
    '--output', output,
    '--provenance-output', provenanceOutput,
  ], { cwd: process.cwd(), env: { ...environment, ...overrides } });
  const createdParent = join(root, 'created');
  await assert.rejects(run(join(createdParent, 'nested', 'output'), join(root, 'provenance', 'created.json')), /tar -xzf failed/);
  await assert.rejects(lstat(createdParent), { code: 'ENOENT' });

  const concurrentParent = join(root, 'concurrent');
  const concurrentFile = join(concurrentParent, 'nested', 'other-publisher');
  await assert.rejects(
    run(join(concurrentParent, 'nested', 'output'), join(root, 'provenance', 'concurrent.json'), { PARITY_CONCURRENT_FILE: concurrentFile }),
    /tar -xzf failed/,
  );
  assert.equal(await readFile(concurrentFile, 'utf8'), 'concurrent');

  const existingParent = join(root, 'existing');
  await mkdir(existingParent);
  await assert.rejects(run(join(existingParent, 'output'), join(root, 'provenance', 'existing.json')), /tar -xzf failed/);
  assert.ok((await lstat(existingParent)).isDirectory());
  await rm(source, { recursive: true, force: true });
  await rm(root, { recursive: true, force: true });
});

test('defines the trusted canonical 18-row order independently of bundle metadata', () => {
  assert.equal(CANONICAL_ROW_IDS.length, 18);
  assert.deepEqual(CANONICAL_ROW_IDS.slice(0, 6), [
    'triangle-20-2000x2700-compact', 'triangle-20-2000x2700-short-side', 'triangle-20-600x400-compact',
    'triangle-20-600x400-short-side', 'triangle-20-300x300-compact', 'triangle-20-300x300-short-side',
  ]);
});

function standaloneParityMatrixItems(workflow, jobName) {
  const job = workflowJob(workflow, jobName);
  const [, include] = job.match(/^      matrix:\n        include:\n([\s\S]*?)(?=^    runs-on:)/m) ?? [];
  assert.notEqual(include, undefined, `${jobName} must define a matrix include list`);
  return include.trimEnd().split(/^          - /m).filter(Boolean).map((item) => Object.fromEntries(
    item.split('\n').filter(Boolean).map((line, index) => {
      const [, key, value] = line.match(index === 0 ? /^([^:]+): (.+)$/ : /^            ([^:]+): (.+)$/) ?? [];
      assert.notEqual(key, undefined, `matrix item is malformed: ${line}`);
      return [key, value];
    }),
  ));
}

function assertStandaloneParityMatrixContract(workflow) {
  const actual = [
    ...standaloneParityMatrixItems(workflow, 'parity-linux'),
    ...standaloneParityMatrixItems(workflow, 'parity'),
  ];
  assert.deepEqual(actual, PARITY_TARGETS
    .filter(({ key }) => key !== 'win32-x64')
    .map(({ key, runner, target }) => ({
      key,
      runner,
      target,
      'executable-suffix': "''",
    })));
  assert.doesNotMatch(workflow, /win32-x64|x86_64-pc-windows-msvc|blacksmith-2vcpu-windows|darwin-x64|x86_64-apple-darwin|macos-15-intel/);
}

function workflowJob(workflow, name) {
  const [, job] = workflow.match(new RegExp(
    `^  ${name}:\\n([\\s\\S]*?)(?=^  [\\w-]+:\\n|(?![\\s\\S]))`,
    'm',
  )) ?? [];
  assert.notEqual(job, undefined, `workflow must define the ${name} job`);
  return job;
}

test('defines the three exact local parity target runner pairs', () => {
  assert.deepEqual(PARITY_TARGETS.map(({ runner, target }) => ({ runner, target })), [
    { runner: 'blacksmith-2vcpu-ubuntu-2404', target: 'x86_64-unknown-linux-gnu' },
    { runner: 'blacksmith-2vcpu-windows-2025', target: 'x86_64-pc-windows-msvc' },
    { runner: 'blacksmith-6vcpu-macos-15', target: 'aarch64-apple-darwin' },
  ]);
  assert.ok(PARITY_TARGETS.every(({ profile, features, rustVersion }) => profile === 'release' && features.length === 0 && rustVersion === '1.95.0'));
});

test('workflow uses both hosted parity runners and uploads each matrix result', async () => {
  const workflow = await readFile(new URL('../../.github/workflows/standalone-parity.yml', import.meta.url), 'utf8');
  assertStandaloneParityMatrixContract(workflow);
  for (const { runner, target } of PARITY_TARGETS.filter(({ key }) => key !== 'win32-x64')) {
    assert.match(workflow, new RegExp(runner.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')));
    assert.match(workflow, new RegExp(target.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')));
  }
  assert.match(workflow, /strategy:/);
  assert.match(workflow, /actions\/checkout@11bd71901bbe5b1630ceea73d27597364c9af683/);
  assert.match(workflow, /actions\/setup-node@49933ea5288caeca8642d1e84afbd3f7d6820020/);
  assert.match(workflow, /dtolnay\/rust-toolchain@4360b52568e2003a75bf9bc1d59f33a8e3fc893c/);
  assert.match(workflow, /old-new-parity-target-\$\{\{ matrix\.key \}\}/);
  assert.match(workflow, /--provenance-output "\$RUNNER_TEMP\/standalone-parity-provenance\/\$\{\{ matrix\.key \}\}\.json"/);
  assert.match(workflow, /--source-provenance-evidence "\$RUNNER_TEMP\/standalone-parity-provenance\/\$\{\{ matrix\.key \}\}\.json"/);
  assert.doesNotMatch(
    workflow,
    /assemble-parity-aggregate|old-new-parity-bundle\.tar\.gz|actions\/attest-build-provenance|id-token: write|attestations: write/,
  );
  assert.doesNotMatch(workflow, /jfet97\/min-plane-dfx/);
});

test('standalone parity matrix rejects swapped target runner assignments', async () => {
  const workflow = await readFile(new URL('../../.github/workflows/standalone-parity.yml', import.meta.url), 'utf8');
  const swappedRunners = workflow
    .replace('runner: blacksmith-2vcpu-ubuntu-2404', 'runner: temporary-runner')
    .replace('runner: blacksmith-6vcpu-macos-15', 'runner: blacksmith-2vcpu-ubuntu-2404')
    .replace('runner: temporary-runner', 'runner: blacksmith-6vcpu-macos-15');
  assert.throws(() => assertStandaloneParityMatrixContract(swappedRunners), /strictly deep-equal/);
  const addedTarget = workflow.replace(
    '    runs-on: ${{ matrix.runner }}',
    "          - target: x86_64-unknown-linux-musl\n            key: linux-musl-x64\n            runner: blacksmith-2vcpu-ubuntu-2404\n            executable-suffix: ''\n    runs-on: ${{ matrix.runner }}",
  );
  assert.throws(() => assertStandaloneParityMatrixContract(addedTarget), /strictly deep-equal/);
});

test('requires disjoint source-parity output and provenance destinations before publication', () => {
  const root = join(tmpdir(), 'parity-fetch-destination-test');
  const output = join(root, 'output');
  const provenance = join(root, 'provenance.json');
  assert.doesNotThrow(() => requireDisjointDestinations(output, provenance));

  for (const [candidateOutput, candidateProvenance] of [
    [output, output],
    [output, join(output, 'provenance.json')],
    [join(root, 'output', 'bundle'), join(root, 'output')],
  ]) {
    assert.throws(
      () => requireDisjointDestinations(candidateOutput, candidateProvenance),
      (error) => error instanceof ParityValidationError && error.message === 'output and provenance output must be disjoint',
    );
  }
});

test('source-parity fetch rejects overlapping destinations without creating output or temporary state', async () => {
  const root = await mkdtemp(join(tmpdir(), 'parity-fetch-overlap-'));
  const temp = join(root, 'temp');
  await mkdir(temp);

  for (const [output, provenanceOutput] of [
    [join(root, 'same'), join(root, 'same')],
    [join(root, 'output'), join(root, 'output', 'provenance.json')],
    [join(root, 'provenance', 'output'), join(root, 'provenance')],
  ]) {
    await assert.rejects(
      execFileAsync(process.execPath, [
        'scripts/parity/fetch-source-parity-bundle.mjs',
        '--source-run', '1',
        '--target', 'aarch64-apple-darwin',
        '--output', output,
        '--provenance-output', provenanceOutput,
      ], { cwd: process.cwd(), env: { ...process.env, TMPDIR: temp } }),
      /output and provenance output must be disjoint/,
    );
    await assert.rejects(lstat(output), { code: 'ENOENT' });
    await assert.rejects(lstat(provenanceOutput), { code: 'ENOENT' });
    assert.deepEqual(await readdir(temp), []);
  }

  await rm(root, { recursive: true, force: true });
});

test('source-parity fetch rejects physically nested destinations through existing symlink parents', async () => {
  const root = await mkdtemp(join(tmpdir(), 'parity-fetch-symlink-overlap-'));
  const temp = join(root, 'temp');
  const physical = join(root, 'physical');
  const alias = join(root, 'alias');
  await mkdir(temp);
  await mkdir(physical);
  await symlink(physical, alias, process.platform === 'win32' ? 'junction' : 'dir');

  for (const [output, provenanceOutput] of [
    [join(alias, 'output'), join(physical, 'output', 'provenance.json')],
    [join(physical, 'provenance'), join(alias, 'provenance', 'output')],
  ]) {
    await assert.rejects(
      execFileAsync(process.execPath, [
        'scripts/parity/fetch-source-parity-bundle.mjs',
        '--source-run', '1',
        '--target', 'aarch64-apple-darwin',
        '--output', output,
        '--provenance-output', provenanceOutput,
      ], { cwd: process.cwd(), env: { ...process.env, TMPDIR: temp } }),
      /output and provenance output must be disjoint/,
    );
    await assert.rejects(lstat(output), { code: 'ENOENT' });
    await assert.rejects(lstat(provenanceOutput), { code: 'ENOENT' });
    assert.deepEqual(await readdir(temp), []);
  }

  await rm(root, { recursive: true, force: true });
});

test('rejects a source bundle without independent provenance', async () => {
  const root = await fixture();
  await expectFailure(() => verifyParityDirectory(root, { source: SOURCE, provenance: null }), 'provenance attestation is required');
});

test('rejects fabricated source identity and self asserted metadata', async () => {
  const root = await fixture();
  await expectFailure(() => verifyParityDirectory(root, {
    source: { ...SOURCE, repository: 'evil/repository' },
    provenance: { repository: 'evil/repository', workflow: SOURCE.workflow, ref: SOURCE.ref, sha: SOURCE.sha, artifact: SOURCE.artifact },
  }), 'repository');
});

test('rejects a bundle with missing immutable raw files', async () => {
  const root = await fixture({ withBuildIdentity: true });
  await rm(join(root, 'old', 'raw', CANONICAL_ROW_IDS[0], 'result.json'));
  await expectFailure(() => verifyParityDirectory(root, {
    source: SOURCE,
    provenance: SOURCE,
  }), 'raw file is missing');
});

test('normalizes only approved measurements while preserving field presence', () => {
  const normalized = normalizeSemanticJson({ runtimeMs: 1, threadCount: 4, nested: { elapsedMs: 2 } });
  assert.deepEqual(normalized, { runtimeMs: '<measurement>', threadCount: 4, nested: { elapsedMs: '<measurement>' } });
});

test('requires a lock hash before accepting a complete checksummed raw capture', async () => {
  const root = await fixture({ withBuildIdentity: true });
  const metadataPath = join(root, SOURCE_CONTRACT.captureMetadata);
  const metadata = JSON.parse(await readFile(metadataPath));
  delete metadata.sourceCargoLockSha256;
  await writeFile(metadataPath, JSON.stringify(metadata));
  await expectFailure(() => verifyParityDirectory(root, { source: SOURCE, provenance: SOURCE }), 'schema is not accepted');
});

test('rejects additional properties in the exact capture metadata contract', async () => {
  const root = await fixture({ withBuildIdentity: true });
  const metadataPath = join(root, SOURCE_CONTRACT.captureMetadata);
  const metadata = JSON.parse(await readFile(metadataPath));
  metadata.untrusted = true;
  await writeFile(metadataPath, JSON.stringify(metadata));
  await expectFailure(() => verifyParityDirectory(root, { source: SOURCE, provenance: SOURCE }), 'schema is not accepted');
});

test('rejects nested metadata additions and workflow identity drift', async () => {
  const cases = [
    ['build', (metadata) => { metadata.build.untrusted = true; }, 'schema is not accepted'],
    ['repository', (metadata) => { metadata.workflow.repository = 'untrusted/repository'; }, 'workflow identity is not accepted'],
    ['ref', (metadata) => { metadata.workflow.ref = 'refs/heads/untrusted'; }, 'workflow identity is not accepted'],
  ];
  for (const [, mutate, expected] of cases) {
    const root = await fixture({ withBuildIdentity: true });
    const metadataPath = join(root, SOURCE_CONTRACT.captureMetadata);
    const metadata = JSON.parse(await readFile(metadataPath));
    mutate(metadata);
    await writeFile(metadataPath, JSON.stringify(metadata));
    await expectFailure(() => verifyParityDirectory(root, { source: SOURCE, provenance: SOURCE }), expected);
  }
});

test('rejects noncanonical dependency metadata entries and digests', async () => {
  const cases = [
    (metadata) => { metadata.nativeDependencies.entries[0].version = '99.0.0'; },
    (metadata) => { metadata.nativeDependencies.entries = [{ name: 'z', version: '1' }, { name: 'a', version: '1' }]; },
    (metadata) => { metadata.nativeDependencies.sha256 = '0'.repeat(64); },
  ];
  for (const mutate of cases) {
    const root = await fixture({ withBuildIdentity: true });
    const metadataPath = join(root, SOURCE_CONTRACT.captureMetadata);
    const metadata = JSON.parse(await readFile(metadataPath));
    mutate(metadata);
    await writeFile(metadataPath, JSON.stringify(metadata));
    await expectFailure(() => verifyParityDirectory(root, { source: SOURCE, provenance: SOURCE }), 'dependency');
  }
});

test('rejects arbitrary canonical dependency entries with a recomputed digest', async () => {
  const root = await fixture({ withBuildIdentity: true });
  const metadataPath = join(root, SOURCE_CONTRACT.captureMetadata);
  const metadata = JSON.parse(await readFile(metadataPath));
  metadata.nativeDependencies.entries = [{ name: 'arbitrary', version: '999' }];
  metadata.nativeDependencies.sha256 = createHash('sha256')
    .update(`${JSON.stringify(metadata.nativeDependencies.entries)}\n`)
    .digest('hex');
  await writeFile(metadataPath, JSON.stringify(metadata));
  await expectFailure(() => verifyParityDirectory(root, { source: SOURCE, provenance: SOURCE }), 'dependency identity is not accepted');
});

test('accepts a complete checksummed capture with exact build identity', async () => {
  const root = await fixture({ withBuildIdentity: true });
  const verified = await verifyParityDirectory(root, { source: SOURCE, provenance: SOURCE });
  assert.equal(verified.captureMetadata.acceptedEngineRevision, SOURCE_CONTRACT.acceptedEngineRevision);
  assert.equal(verified.normalizationVersion, '1');
});

test('rejects a changed raw file after checksums were recorded', async () => {
  const root = await fixture({ withBuildIdentity: true });
  await writeFile(join(root, 'old', 'raw', CANONICAL_ROW_IDS[0], 'result.json'), '{"result":"tampered"}\n');
  await expectFailure(() => verifyParityDirectory(root, { source: SOURCE, provenance: SOURCE }), 'bundle manifest hash differs');
});

test('compares semantic result and ordered event records without normalizing thread count', async () => {
  const oldRoot = await fixture();
  const newRoot = await mkdtemp(join(tmpdir(), 'new-parity-consumer-'));
  const newRow = join(newRoot, 'new', 'raw', 'desktop-01');
  await mkdir(newRow, { recursive: true });
  await writeFile(join(newRow, 'result.json'), '{"result":"ok","runtimeMs":900}\n');
  await writeFile(join(newRow, 'events.ndjson'), '{"kind":"portfolio-progress","elapsedMs":999}\n');
  await writeFile(join(newRow, 'request.json'), '{"desktop":true,"options":{"diagnosticTraceMode":"full"}}\n');
  await writeFile(join(newRow, 'stderr.txt'), '');
  await writeFile(join(newRow, 'process.json'), JSON.stringify({ rowId: 'desktop-01' }));
  const evidence = await compareParityRows(oldRoot, newRoot, ['desktop-01']);
  assert.equal(evidence.length, 5);
  assert.deepEqual(evidence.map(({ filename }) => filename), ['request.json', 'result.json', 'events.ndjson', 'stderr.txt', 'process.json']);
  await writeFile(join(newRow, 'result.json'), '{"result":"ok","threadCount":99,"runtimeMs":900}\n');
  await expectFailure(() => compareParityRows(oldRoot, newRoot, ['desktop-01']), 'semantic mismatch');
});

test('production parity command fails when captured standalone output differs semantically', async () => {
  const oldRoot = await fixture({ withBuildIdentity: true });
  const newRoot = await mkdtemp(join(tmpdir(), 'new-parity-command-'));
  const addon = join(newRoot, 'fake-addon.mjs');
  await writeFile(addon, `export async function runIrregularJob(_request, _token, onEvent) { onEvent('{"kind":"portfolio-progress","elapsedMs":5}'); return '{"result":"different"}'; }`);
  const verified = await verifyParityDirectory(oldRoot, { source: SOURCE, provenance: SOURCE, target: 'aarch64-apple-darwin' });
  const provenance = join(newRoot, 'old-capture-provenance.json');
  await writeFile(provenance, `${JSON.stringify({ schemaVersion: 1, sourceRun: '1', sourceRepository: SOURCE.repository, sourceWorkflow: SOURCE.workflow, sourceArtifact: SOURCE.artifact, acceptedOldRevision: '5c72d8fca8e078b0a6e7d5f2515a8a0953475481', target: 'aarch64-apple-darwin', archiveSha256: 'a'.repeat(64), expectedArchiveSha256: 'a'.repeat(64), verification: 'gh attestation verify succeeded before extraction', captureMetadata: verified.captureMetadata, bundleManifest: verified.bundleManifest, rawChecksums: verified.rawChecksums })}\n`);
  await assert.rejects(
    execFileAsync(process.execPath, [
      'scripts/parity/run-old-new-parity.mjs', '--old-root', oldRoot, '--new-root', newRoot,
      '--addon', addon, '--target', 'aarch64-apple-darwin', '--evidence', join(newRoot, 'evidence.json'), '--source-provenance-evidence', provenance,
    ], { cwd: process.cwd() }),
    /semantic mismatch/,
  );
});

test('rejects dependency identities that differ from the accepted old pair', () => {
  assert.throws(() => verifyDependencyIdentity({ 'napi-derive': '3.6.2' }, { 'napi-derive': '3.6.1' }), /napi-derive/);
});

test('pins the executed N-API derive dependency to the accepted version', async () => {
  const cargoToml = await readFile(new URL('../../crates/polygon-nesting-napi/Cargo.toml', import.meta.url), 'utf8');
  assert.match(cargoToml, /^napi-derive = "=3\.6\.1"$/m);
});

test('accepts canonical directory members with trailing slashes', async () => {
  const root = await mkdtemp(join(tmpdir(), 'parity-directory-archive-'));
  const archive = join(root, 'safe.tar.gz');
  await writeTarMember(archive, './old/raw/', '5');
  assert.doesNotThrow(() => assertSafeArchive(archive));
});

test('rejects symlink archive members before extraction', async () => {
  const root = await mkdtemp(join(tmpdir(), 'parity-archive-'));
  await writeFile(join(root, 'payload.json'), '{}');
  await symlink('payload.json', join(root, 'linked.json'));
  const archive = join(root, 'unsafe.tar.gz');
  await execFileAsync('tar', ['-czf', archive, '-C', root, 'linked.json']);
  assert.throws(() => assertSafeArchive(archive), /symlink/);
});

test('rejects hardlink archive members before extraction', async () => {
  const root = await mkdtemp(join(tmpdir(), 'parity-hardlink-archive-'));
  await writeFile(join(root, 'payload.json'), '{}');
  await link(join(root, 'payload.json'), join(root, 'linked.json'));
  const archive = join(root, 'unsafe.tar.gz');
  await execFileAsync('tar', ['-czf', archive, '-C', root, 'payload.json', 'linked.json']);
  assert.throws(() => assertSafeArchive(archive), /hardlink/);
});

test('rejects traversal archive members whose names contain spaces', async () => {
  const root = await mkdtemp(join(tmpdir(), 'parity-spaced-traversal-archive-'));
  const archive = join(root, 'unsafe.tar.gz');
  await writeTarMember(archive, '../outside payload');
  assert.throws(() => assertSafeArchive(archive), /unsafe path/);
});

test('rejects Win32 drive-designator archive members', async () => {
  const root = await mkdtemp(join(tmpdir(), 'parity-win32-drive-archive-'));
  const archive = join(root, 'unsafe.tar.gz');
  await writeTarMember(archive, 'C:/outside payload');
  assert.throws(() => assertSafeArchive(archive), /unsafe path/);
});

test('rejects Win32 drive-relative archive members', async () => {
  const root = await mkdtemp(join(tmpdir(), 'parity-win32-drive-relative-archive-'));
  const archive = join(root, 'unsafe.tar.gz');
  await writeTarMember(archive, 'C:outside payload');
  assert.throws(() => assertSafeArchive(archive), /unsafe path/);
});

test('rejects symlinked capture metadata before parsing it', async () => {
  const root = await fixture({ withBuildIdentity: true });
  const metadata = join(root, SOURCE_CONTRACT.captureMetadata);
  const external = join(root, 'external-capture-metadata.json');
  await writeFile(external, await readFile(metadata));
  await rm(metadata);
  await symlink(external, metadata);
  await expectFailure(() => verifyParityDirectory(root, { source: SOURCE, provenance: SOURCE }), 'regular non-symlink');
});

test('rejects symlink raw files', async () => {
  const root = await fixture({ withBuildIdentity: true });
  const path = join(root, 'old', 'raw', CANONICAL_ROW_IDS[0], 'stderr.txt');
  await rm(path);
  await symlink('request.json', path);
  await expectFailure(() => verifyParityDirectory(root, { source: SOURCE, provenance: SOURCE }), 'regular non-symlink');
});
