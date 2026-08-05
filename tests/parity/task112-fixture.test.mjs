import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import { test } from 'node:test';

import { PARITY_TARGET_LAYOUT } from '../../scripts/parity/assemble-parity-aggregate.mjs';
import { CANONICAL_ROW_IDS, SOURCE_CONTRACT } from '../../scripts/parity/verify-parity-bundle.mjs';
import { createTask112ParityFixture } from './helpers/task112-parity-fixture.mjs';

async function fileExists(path) {
  try {
    await readFile(path);
    return true;
  } catch {
    return false;
  }
}

test('Task112 fixture generator assembles the complete v1 aggregate through the production assembler', async (t) => {
  const fixture = await createTask112ParityFixture();
  t.after(fixture.cleanup);

  assert.equal(await fileExists(fixture.archivePath), true);
  assert.equal(await fileExists(fixture.digestPath), true);
  assert.match(await readFile(fixture.digestPath, 'utf8'), /^[a-f0-9]{64}  old-new-parity-bundle\.tar\.gz\n$/);
  assert.deepEqual(fixture.targets.map(({ target }) => target), PARITY_TARGET_LAYOUT.map(({ target }) => target));
  assert.deepEqual((await readdir(`${fixture.aggregateDirectory}/targets`)).sort(), PARITY_TARGET_LAYOUT.map(({ target }) => target).sort());

  for (const target of PARITY_TARGET_LAYOUT) {
    const root = fixture.targetDirectories[target.target];
    const parity = JSON.parse(await readFile(`${root}/parity.json`, 'utf8'));
    assert.equal(parity.version, 1);
    assert.equal(parity.napiComparisons.length, CANONICAL_ROW_IDS.length * SOURCE_CONTRACT.rawFilenames.length);
    assert.equal(parity.cliComparisons.length, CANONICAL_ROW_IDS.length * SOURCE_CONTRACT.rawFilenames.length);
    assert.equal(await fileExists(`${root}/bundle-manifest.json`), true);
    assert.equal(await fileExists(`${root}/SHA256SUMS`), true);
    assert.equal(await fileExists(`${root}/source-provenance.json`), true);
    for (const sourcePath of [
      'crates/polygon-nesting-napi/src/bin/parity-desktop-request-adapter.rs',
      'crates/polygon-nesting-napi/src/bin/parity-project-engine-outcome.rs',
      'crates/polygon-nesting-napi/src/bin/parity-project-engine-events.rs',
    ]) assert.equal(await fileExists(`${root}/source/${sourcePath}`), true);
    for (const executable of ['parity-desktop-request-adapter', 'polygon-nesting', 'parity-project-engine-outcome', 'parity-project-engine-events']) {
      assert.equal(await fileExists(`${root}/executables/${executable}`), true);
    }
    for (const rowId of CANONICAL_ROW_IDS) {
      for (const side of ['old', 'new', 'projected']) {
        assert.deepEqual((await readdir(`${root}/${side}/raw/${rowId}`)).sort(), [...SOURCE_CONTRACT.rawFilenames].sort());
      }
      const oldResult = JSON.parse(await readFile(`${root}/old/raw/${rowId}/result.json`, 'utf8'));
      const newResult = JSON.parse(await readFile(`${root}/new/raw/${rowId}/result.json`, 'utf8'));
      assert.notEqual(oldResult.runtimeMs, newResult.runtimeMs);
      const cliFiles = await readdir(`${root}/cli/raw/${rowId}`);
      assert.equal(cliFiles.length, 11);
    }
  }
});

test('Task112 fixture generator produces byte-identical aggregate transports for unchanged input', async (t) => {
  const fixture = await createTask112ParityFixture();
  const equivalentFixture = await createTask112ParityFixture();
  t.after(fixture.cleanup);
  t.after(equivalentFixture.cleanup);

  assert.deepEqual(await readFile(fixture.archivePath), await readFile(fixture.repeat.archivePath));
  assert.deepEqual(await readFile(fixture.digestPath), await readFile(fixture.repeat.digestPath));
  assert.equal(fixture.sourceRevision, equivalentFixture.sourceRevision);
  assert.deepEqual(await readFile(fixture.archivePath), await readFile(equivalentFixture.archivePath));
});
