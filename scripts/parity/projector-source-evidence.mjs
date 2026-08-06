import { createHash } from 'node:crypto';
import { execFile } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { promisify } from 'node:util';

const execFileAsync = promisify(execFile);

export const PROJECTOR_SOURCES = Object.freeze({
  adapter: Object.freeze({ label: 'parity-desktop-request-adapter', path: 'crates/polygon-nesting-napi/src/bin/parity-desktop-request-adapter.rs' }),
  outcomeProjector: Object.freeze({ label: 'parity-project-engine-outcome', path: 'crates/polygon-nesting-napi/src/bin/parity-project-engine-outcome.rs' }),
  eventsProjector: Object.freeze({ label: 'parity-project-engine-events', path: 'crates/polygon-nesting-napi/src/bin/parity-project-engine-events.rs' }),
});

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

async function committedBytes(sourceRoot, sourceRevision, sourcePath) {
  const { stdout } = await execFileAsync(
    'git',
    ['-C', sourceRoot, 'cat-file', 'blob', `${sourceRevision}:${sourcePath}`],
    {
      encoding: 'buffer',
      env: { ...process.env, GIT_NO_REPLACE_OBJECTS: '1' },
      maxBuffer: 1024 * 1024,
    },
  );
  return stdout;
}

export async function copyCommittedProjectorSources({ evidenceRoot, sourceRevision, sourceRoot }) {
  if (!/^[a-f0-9]{40}$/.test(sourceRevision ?? '')) throw new Error('standalone source revision must be a full SHA');
  const trustedRoot = resolve(sourceRoot);
  const sources = {};
  for (const [key, source] of Object.entries(PROJECTOR_SOURCES)) {
    const bytes = await committedBytes(trustedRoot, sourceRevision, source.path);
    const destination = join(evidenceRoot, 'source', source.path);
    await mkdir(dirname(destination), { recursive: true });
    await writeFile(destination, bytes, { flag: 'wx' });
    sources[key] = { ...source, sha256: sha256(bytes), revision: sourceRevision };
  }
  return sources;
}
