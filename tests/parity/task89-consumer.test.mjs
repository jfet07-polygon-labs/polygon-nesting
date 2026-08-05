import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtemp, readFile, rm, unlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { gzipSync } from 'node:zlib';
import { join } from 'node:path';
import test from 'node:test';

import { createTask112ParityFixture } from './helpers/task112-parity-fixture.mjs';

async function consumer() {
  return import(new URL('../../scripts/stage-parity-aggregate.mjs', import.meta.url));
}

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

function octal(value, length) {
  return `${value.toString(8).padStart(length - 1, '0')}\0`;
}

function ustarMember(path, type = '0', body = Buffer.alloc(0)) {
  const header = Buffer.alloc(512);
  header.write(path);
  header.write(octal(body.length, 12), 124);
  header[156] = type.charCodeAt(0);
  header.write('ustar\0', 257);
  header.fill(32, 148, 156);
  const checksum = header.reduce((sum, byte) => sum + byte, 0);
  header.write(octal(checksum, 8), 148);
  return Buffer.concat([header, body, Buffer.alloc((512 - body.length % 512) % 512)]);
}

async function maliciousArchive(root, members, suffix = '') {
  const archivePath = join(root, 'old-new-parity-bundle.tar.gz');
  const bytes = gzipSync(Buffer.concat([...members, Buffer.alloc(1024), Buffer.from(suffix)]));
  await writeFile(archivePath, bytes);
  const digestPath = `${archivePath}.sha256`;
  await writeFile(digestPath, `${sha256(bytes)}  old-new-parity-bundle.tar.gz\n`);
  return { archivePath, digestPath };
}

test('accepts and stages every Task112 aggregate target only after complete validation', async (t) => {
  const fixture = await createTask112ParityFixture();
  const output = await mkdtemp(join(tmpdir(), 'task89-stage-'));
  t.after(async () => {
    await fixture.cleanup();
    await rm(output, { recursive: true, force: true });
  });
  const { stageParityAggregateArchive } = await consumer();

  for (const target of fixture.targets) {
    const artifactDirectory = join(output, target.key);
    stageParityAggregateArchive({
      archivePath: fixture.archivePath,
      digestPath: fixture.digestPath,
      artifactDirectory,
      cargoTarget: target.target,
      sourceCommit: fixture.sourceRevision,
      targetKey: target.key,
      trustedSourceRoot: fixture.trustedSourceRoot,
    });
    assert.deepEqual(
      await readFile(join(artifactDirectory, 'parity-bundle', 'parity.json')),
      await readFile(join(fixture.targetDirectories[target.target], 'parity.json')),
    );
  }
});

test('rejects a mismatched archive sidecar before parsing or extraction', async (t) => {
  const root = await mkdtemp(join(tmpdir(), 'task89-sidecar-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const { archivePath, digestPath } = await maliciousArchive(root, [ustarMember('safe')]);
  await writeFile(digestPath, `${'0'.repeat(64)}  old-new-parity-bundle.tar.gz\n`);
  const { stageParityAggregateArchive } = await consumer();
  assert.throws(() => stageParityAggregateArchive({ archivePath, digestPath, artifactDirectory: join(root, 'out'), cargoTarget: 'x86_64-unknown-linux-gnu', sourceCommit: '0'.repeat(40), targetKey: 'linux-x64', trustedSourceRoot: root }), /SHA-256/);
});

test('rejects a recomputed archive sidecar when a USTAR header checksum is invalid', async (t) => {
  const root = await mkdtemp(join(tmpdir(), 'task89-ustar-checksum-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const member = ustarMember('safe');
  member[0] ^= 1;
  const { archivePath, digestPath } = await maliciousArchive(root, [member]);
  const { stageParityAggregateArchive } = await consumer();
  assert.throws(() => stageParityAggregateArchive({ archivePath, digestPath, artifactDirectory: join(root, 'out'), cargoTarget: 'x86_64-unknown-linux-gnu', sourceCommit: '0'.repeat(40), targetKey: 'linux-x64', trustedSourceRoot: root }), /header checksum/);
});

test('rejects unsafe archive names, links, and truncated USTAR bodies before staging', async (t) => {
  const root = await mkdtemp(join(tmpdir(), 'task89-archive-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const { stageParityAggregateArchive } = await consumer();
  for (const member of [ustarMember('../ traversal with spaces'), ustarMember('C:/absolute'), ustarMember('C:relative'), ustarMember('linked', '2'), ustarMember('hardlinked', '1')]) {
    const { archivePath, digestPath } = await maliciousArchive(root, [member]);
    assert.throws(() => stageParityAggregateArchive({ archivePath, digestPath, artifactDirectory: join(root, 'out'), cargoTarget: 'x86_64-unknown-linux-gnu', sourceCommit: '0'.repeat(40), targetKey: 'linux-x64', trustedSourceRoot: root }), /unsafe path|link or unsupported/);
  }
  const truncated = gzipSync(ustarMember('truncated').subarray(0, 600));
  const archivePath = join(root, 'old-new-parity-bundle.tar.gz');
  const digestPath = `${archivePath}.sha256`;
  await writeFile(archivePath, truncated);
  await writeFile(digestPath, `${sha256(truncated)}  old-new-parity-bundle.tar.gz\n`);
  assert.throws(() => stageParityAggregateArchive({ archivePath, digestPath, artifactDirectory: join(root, 'out'), cargoTarget: 'x86_64-unknown-linux-gnu', sourceCommit: '0'.repeat(40), targetKey: 'linux-x64', trustedSourceRoot: root }), /truncated|terminal zero/);
});

async function rejectsFixtureMutation(t, name, mutate, expected) {
  const fixture = await createTask112ParityFixture();
  t.after(() => fixture.cleanup());
  const { verifyParityAggregate } = await import(new URL('../../scripts/parity-contract.mjs', import.meta.url));
  await mutate(fixture);
  assert.throws(() => verifyParityAggregate({ aggregateDirectory: fixture.aggregateDirectory, sourceCommit: fixture.sourceRevision, trustedSourceRoot: fixture.trustedSourceRoot }), expected, name);
}

test('rejects target manifest closure drift before selecting any target', async (t) => {
  await rejectsFixtureMutation(t, 'target closure', (fixture) => unlink(join(fixture.aggregateDirectory, 'targets', fixture.targets[0].target, 'cli', 'raw', 'mixed-61-2000x2700-compact', 'stderr.txt')), /bundle manifest|CLI transport|raw evidence/);
});

test('rejects a coherent target swap independently of target selection', async (t) => {
  await rejectsFixtureMutation(t, 'coherent swap', async (fixture) => {
    const target = fixture.targets[0].target;
    await writeFile(join(fixture.aggregateDirectory, 'targets', target, 'parity.json'), await readFile(join(fixture.aggregateDirectory, 'targets', fixture.targets[1].target, 'parity.json')));
  }, /bundle manifest|parity/);
});

test('rejects aggregate source revision drift independently', async (t) => {
  await rejectsFixtureMutation(t, 'aggregate source drift', (fixture) => writeFile(join(fixture.aggregateDirectory, 'aggregate-metadata.json'), '{"acceptedEngineRevision":"5c72d8fca8e078b0a6e7d5f2515a8a0953475481","sourceRevision":"0000000000000000000000000000000000000000","targets":[],"version":1}\n'), /aggregate metadata/);
});

test('rejects rehashed projector-source drift from the committed git blob', async (t) => {
  await rejectsFixtureMutation(t, 'projector source drift', (fixture) => writeFile(join(fixture.aggregateDirectory, 'targets', fixture.targets[0].target, 'source', 'crates/polygon-nesting-napi/src/bin/parity-desktop-request-adapter.rs'), 'drift\n'), /bundle manifest|projector source/);
});

test('rejects stale semantic comparison evidence after package rehash', async (t) => {
  await rejectsFixtureMutation(t, 'stale semantic comparison', (fixture) => writeFile(join(fixture.aggregateDirectory, 'targets', fixture.targets[0].target, 'new', 'raw', 'mixed-61-2000x2700-compact', 'result.json'), '{"result":"drift"}\n'), /bundle manifest|comparison/);
});

test('rejects root checksum drift and target checksum drift independently', async (t) => {
  await rejectsFixtureMutation(t, 'root checksum drift', (fixture) => writeFile(join(fixture.aggregateDirectory, 'SHA256SUMS'), '0'.repeat(64) + '  aggregate-metadata.json\n'), /SHA256SUMS/);
  await rejectsFixtureMutation(t, 'target checksum drift', (fixture) => writeFile(join(fixture.aggregateDirectory, 'targets', fixture.targets[0].target, 'SHA256SUMS'), '0'.repeat(64) + '  parity.json\n'), /SHA256SUMS/);
});
