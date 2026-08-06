import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { execFile } from 'node:child_process';
import { mkdtemp, mkdir, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { promisify } from 'node:util';

import {
  PROJECTOR_SOURCES,
  copyCommittedProjectorSources,
} from '../../scripts/parity/projector-source-evidence.mjs';

const execFileAsync = promisify(execFile);

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

async function git(arguments_, cwd) {
  return execFileAsync('git', arguments_, { cwd });
}

test('copies committed projector bytes instead of platform-normalized worktree bytes', async () => {
  const sourceRoot = await mkdtemp(join(tmpdir(), 'projector-source-root-'));
  const evidenceRoot = await mkdtemp(join(tmpdir(), 'projector-source-evidence-'));
  const committedSources = new Map();

  for (const source of Object.values(PROJECTOR_SOURCES)) {
    const bytes = Buffer.from(`fn ${source.label.replaceAll('-', '_')}() {}\n`);
    committedSources.set(source.path, bytes);
    await mkdir(dirname(join(sourceRoot, source.path)), { recursive: true });
    await writeFile(join(sourceRoot, source.path), bytes);
  }

  await git(['init'], sourceRoot);
  await git(['config', 'user.email', 'parity@example.test'], sourceRoot);
  await git(['config', 'user.name', 'Parity Test'], sourceRoot);
  await git(['add', '.'], sourceRoot);
  await git(['commit', '-m', 'trusted projector sources'], sourceRoot);
  const { stdout } = await git(['rev-parse', 'HEAD'], sourceRoot);
  const revision = stdout.trim();

  for (const source of Object.values(PROJECTOR_SOURCES)) {
    await writeFile(
      join(sourceRoot, source.path),
      committedSources.get(source.path).toString('utf8').replaceAll('\n', '\r\n'),
    );
  }

  const identities = await copyCommittedProjectorSources({
    evidenceRoot,
    sourceRevision: revision,
    sourceRoot,
  });

  for (const [key, source] of Object.entries(PROJECTOR_SOURCES)) {
    const expected = committedSources.get(source.path);
    assert.deepEqual(await readFile(join(evidenceRoot, 'source', source.path)), expected);
    assert.equal(identities[key].sha256, sha256(expected));
    assert.equal(identities[key].revision, revision);
  }
});

test('ignores replacement refs when copying committed projector bytes', async () => {
  const sourceRoot = await mkdtemp(join(tmpdir(), 'projector-source-replacement-root-'));
  const evidenceRoot = await mkdtemp(join(tmpdir(), 'projector-source-replacement-evidence-'));
  const committedSources = new Map();

  for (const source of Object.values(PROJECTOR_SOURCES)) {
    const bytes = Buffer.from(`fn committed_${source.label.replaceAll('-', '_')}() {}\n`);
    committedSources.set(source.path, bytes);
    await mkdir(dirname(join(sourceRoot, source.path)), { recursive: true });
    await writeFile(join(sourceRoot, source.path), bytes);
  }

  await git(['init'], sourceRoot);
  await git(['config', 'user.email', 'parity@example.test'], sourceRoot);
  await git(['config', 'user.name', 'Parity Test'], sourceRoot);
  await git(['add', '.'], sourceRoot);
  await git(['commit', '-m', 'trusted projector sources'], sourceRoot);
  const { stdout: trustedStdout } = await git(['rev-parse', 'HEAD'], sourceRoot);
  const trustedRevision = trustedStdout.trim();

  for (const source of Object.values(PROJECTOR_SOURCES)) {
    await writeFile(join(sourceRoot, source.path), `fn replacement_${source.label.replaceAll('-', '_')}() {}\n`);
  }
  await git(['add', '.'], sourceRoot);
  await git(['commit', '-m', 'replacement projector sources'], sourceRoot);
  const { stdout: replacementStdout } = await git(['rev-parse', 'HEAD'], sourceRoot);
  await git(['replace', trustedRevision, replacementStdout.trim()], sourceRoot);

  const identities = await copyCommittedProjectorSources({
    evidenceRoot,
    sourceRevision: trustedRevision,
    sourceRoot,
  });

  for (const [key, source] of Object.entries(PROJECTOR_SOURCES)) {
    const expected = committedSources.get(source.path);
    assert.deepEqual(await readFile(join(evidenceRoot, 'source', source.path)), expected);
    assert.equal(identities[key].sha256, sha256(expected));
  }
});
