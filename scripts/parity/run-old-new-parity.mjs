#!/usr/bin/env node
import { createHash, randomUUID } from 'node:crypto';
import { createRequire } from 'node:module';
import { copyFile, cp, mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { dirname, extname, join, resolve } from 'node:path';

import {
  ACCEPTED_OLD_REVISION,
  CANONICAL_ROW_IDS,
  NORMALIZATION_VERSION,
  ParityValidationError,
  SOURCE_CONTRACT,
  compareParityRows,
  parityTarget,
  verifyParityDirectory,
} from './verify-parity-bundle.mjs';
import { PARITY_TARGET_LAYOUT } from './assemble-parity-aggregate.mjs';
import { copyCommittedProjectorSources } from './projector-source-evidence.mjs';

const EXECUTABLE_VERSION = 1;
const SOURCE_CONTRACT_VERSION = 1;
const SOURCE_ROOT = fileURLToPath(new URL('../..', import.meta.url));

function fail(message) {
  throw new ParityValidationError(message);
}

function argument(name) {
  const index = process.argv.indexOf(name);
  if (index === -1 || !process.argv[index + 1]) fail(`${name} is required`);
  return process.argv[index + 1];
}

function optionalArgument(name) {
  const index = process.argv.indexOf(name);
  return index === -1 ? null : argument(name);
}

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

async function writeNewFile(path, content) {
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, content, { flag: 'wx' });
}

function rowIds(rawChecksums) {
  const discovered = [...new Set(Object.keys(rawChecksums).map((path) => /^old\/raw\/([^/]+)\//.exec(path)?.[1]).filter(Boolean))].sort();
  if (JSON.stringify(discovered) !== JSON.stringify([...CANONICAL_ROW_IDS].sort())) fail('old-side checksum rows do not equal the canonical corpus');
  return [...CANONICAL_ROW_IDS];
}

function canonicalJson(value) {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`;
  if (value && typeof value === 'object') return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(',')}}`;
  return JSON.stringify(value);
}

function validateSourceProvenanceEvidence(bytes, verified, target) {
  let evidence;
  try { evidence = JSON.parse(bytes); } catch { fail('source provenance evidence is not valid JSON'); }
  const keys = ['acceptedOldRevision', 'archiveSha256', 'bundleManifest', 'captureMetadata', 'expectedArchiveSha256', 'rawChecksums', 'schemaVersion', 'sourceArtifact', 'sourceRepository', 'sourceRun', 'sourceWorkflow', 'target', 'verification'];
  if (!evidence || typeof evidence !== 'object' || Array.isArray(evidence) || JSON.stringify(Object.keys(evidence).sort()) !== JSON.stringify(keys)) fail('source provenance evidence schema is not accepted');
  if (evidence.schemaVersion !== 1 || typeof evidence.sourceRun !== 'string' || !/^[1-9][0-9]*$/.test(evidence.sourceRun) || evidence.sourceRun !== verified.captureMetadata?.workflow?.runId || typeof verified.captureMetadata?.workflow?.runId !== 'string' || !/^[1-9][0-9]*$/.test(verified.captureMetadata.workflow.runId) || evidence.sourceRepository !== SOURCE_CONTRACT.repository || evidence.sourceWorkflow !== SOURCE_CONTRACT.workflow || evidence.sourceArtifact !== target.artifact || evidence.acceptedOldRevision !== ACCEPTED_OLD_REVISION || evidence.target !== target.target || evidence.verification !== 'gh attestation verify succeeded before extraction' || !/^[a-f0-9]{64}$/.test(evidence.archiveSha256) || evidence.archiveSha256 !== evidence.expectedArchiveSha256) fail('source provenance evidence identity is not accepted');
  if (canonicalJson(evidence.captureMetadata) !== canonicalJson(verified.captureMetadata) || canonicalJson(evidence.bundleManifest) !== canonicalJson(verified.bundleManifest) || canonicalJson(evidence.rawChecksums) !== canonicalJson(verified.rawChecksums)) fail('source provenance evidence differs from the verified old-side bundle');
  return evidence;
}

async function executableIdentity(path, label, evidenceRoot, source = null) {
  const bytes = await readFile(path);
  const evidencePath = `executables/${label}`;
  await writeNewFile(join(evidenceRoot, evidencePath), bytes);
  return {
    label,
    evidencePath,
    version: EXECUTABLE_VERSION,
    sha256: sha256(bytes),
    ...(source ? {
      sourcePath: source.path,
      sourceSha256: source.sha256,
      sourceVersion: SOURCE_CONTRACT_VERSION,
      sourceRevision: source.revision,
    } : {}),
  };
}

function explicitFullDiagnosticTraceMode(request, rowId) {
  let parsed;
  try {
    parsed = JSON.parse(request);
  } catch (error) {
    fail(`parity request for ${rowId} is not valid JSON: ${error.message}`);
  }
  if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
    fail(`parity request for ${rowId} must be an object`);
  }
  if (!parsed.options || typeof parsed.options !== 'object' || Array.isArray(parsed.options)) {
    fail(`parity request for ${rowId} must include an options object`);
  }
  if (parsed.options.diagnosticTraceMode === 'full') return request;
  parsed.options.diagnosticTraceMode = 'full';
  return `${JSON.stringify(parsed)}\n`;
}

function runProcess(command, arguments_, input) {
  return new Promise((resolvePromise, reject) => {
    const child = spawn(command, arguments_, { stdio: ['pipe', 'pipe', 'pipe'] });
    const stdout = [];
    const stderr = [];
    child.stdout.on('data', (chunk) => stdout.push(chunk));
    child.stderr.on('data', (chunk) => stderr.push(chunk));
    child.on('error', reject);
    child.on('close', (code, signal) => resolvePromise({
      stdout: Buffer.concat(stdout),
      stderr: Buffer.concat(stderr),
      exitCode: code,
      signal,
    }));
    child.stdin.end(input);
  });
}

function assertSuccessfulProcess(process, label) {
  if (process.exitCode !== 0 || process.signal) fail(`${label} transport failed with ${process.signal ?? process.exitCode}`);
}

function processEvidence(process, target, inputs = {}) {
  return Buffer.from(`${canonicalJson({
    version: 1,
    target,
    exitCode: process.exitCode,
    signal: process.signal,
    stdoutSha256: sha256(process.stdout),
    stderrSha256: sha256(process.stderr),
    ...inputs,
  })}\n`);
}

async function captureNapiRow(addon, oldRoot, newRoot, rowId, target) {
  const oldRow = join(oldRoot, 'old', 'raw', rowId);
  const newRow = join(newRoot, 'new', 'raw', rowId);
  const request = await readFile(join(oldRow, 'request.json'), 'utf8');
  const parityRequest = explicitFullDiagnosticTraceMode(request, rowId);
  const events = [];
  const run = addon.runIrregularJob ?? addon.run_irregular_job;
  if (typeof run !== 'function') fail('standalone N-API addon does not export runIrregularJob');
  const invocationToken = `parity-${rowId}-${randomUUID()}`;
  let result;
  let stderr = '';
  try {
    result = await run(parityRequest, invocationToken, (event) => events.push(event), false);
  } catch (error) {
    stderr = `${error instanceof Error ? error.stack ?? error.message : String(error)}\n`;
    result = JSON.stringify({ ok: false, error: { category: 'parity_adapter_failure', message: stderr } });
  }
  const resultBytes = Buffer.from(`${result}\n`);
  const eventBytes = Buffer.from(events.length === 0 ? '' : `${events.join('\n')}\n`);
  const requestBytes = Buffer.from(parityRequest);
  await writeNewFile(join(newRow, 'request.json'), requestBytes);
  await writeNewFile(join(newRow, 'result.json'), resultBytes);
  await writeNewFile(join(newRow, 'events.ndjson'), eventBytes);
  await writeNewFile(join(newRow, 'stderr.txt'), stderr);
  await writeNewFile(join(newRow, 'process.json'), `${canonicalJson({
    version: 1,
    target,
    adapter: 'standalone-napi',
    requestSha256: sha256(requestBytes),
    resultSha256: sha256(resultBytes),
    eventsSha256: sha256(eventBytes),
  })}\n`);
  return requestBytes;
}

function assertNoJobIdLeak(bytes, invocationToken, rowId, label) {
  if (bytes.includes(invocationToken)) fail(`${label} leaks the generated jobId for ${rowId}`);
}

async function captureCliRow({ oldRoot, newRoot, rowId, target, adapter, cli, outcomeProjector, eventsProjector }) {
  const oldRow = join(oldRoot, 'old', 'raw', rowId);
  const cliRow = join(newRoot, 'cli', 'raw', rowId);
  const request = await readFile(join(oldRow, 'request.json'));
  const parityRequest = Buffer.from(explicitFullDiagnosticTraceMode(request.toString('utf8'), rowId));
  const adapted = await runProcess(adapter, [], parityRequest);
  assertSuccessfulProcess(adapted, `desktop request adapter for ${rowId}`);
  if (adapted.stderr.length !== 0) fail(`desktop request adapter wrote stderr for ${rowId}`);
  const adaptedRequest = adapted.stdout;
  await writeNewFile(join(cliRow, 'adapted-request.json'), adaptedRequest);
  await writeNewFile(join(cliRow, 'adapter-stderr.txt'), adapted.stderr);
  await writeNewFile(join(cliRow, 'adapter-process.json'), processEvidence(adapted, target, { requestSha256: sha256(parityRequest), adaptedRequestSha256: sha256(adaptedRequest) }));

  const resultPath = join(cliRow, 'result.json');
  const eventsPath = join(cliRow, 'events.ndjson');
  const cliRun = await runProcess(cli, ['run', '--input', join(cliRow, 'adapted-request.json'), '--result-file', resultPath, '--events', eventsPath], Buffer.alloc(0));
  assertSuccessfulProcess(cliRun, `neutral CLI for ${rowId}`);
  const result = await readFile(resultPath);
  const events = await readFile(eventsPath);
  await writeNewFile(join(cliRow, 'stderr.txt'), cliRun.stderr);
  await writeNewFile(join(cliRow, 'process.json'), processEvidence(cliRun, target, {
    adaptedRequestSha256: sha256(adaptedRequest), resultSha256: sha256(result), eventsSha256: sha256(events),
  }));

  const projectedResult = await runProcess(outcomeProjector, [], result);
  assertSuccessfulProcess(projectedResult, `outcome projector for ${rowId}`);
  if (projectedResult.stderr.length !== 0) fail(`outcome projector wrote stderr for ${rowId}`);
  const projectedEvents = await runProcess(eventsProjector, ['--outcome', resultPath], events);
  assertSuccessfulProcess(projectedEvents, `event projector for ${rowId}`);
  if (projectedEvents.stderr.length !== 0) fail(`event projector wrote stderr for ${rowId}`);
  const invocationToken = `parity-${rowId}`;
  assertNoJobIdLeak(projectedResult.stdout, invocationToken, rowId, 'projected result');
  assertNoJobIdLeak(projectedEvents.stdout, invocationToken, rowId, 'projected events');
  await writeNewFile(join(cliRow, 'projected-result.json'), projectedResult.stdout);
  await writeNewFile(join(cliRow, 'projected-events.ndjson'), projectedEvents.stdout);
  await writeNewFile(join(cliRow, 'outcome-projector-process.json'), processEvidence(projectedResult, target, { resultSha256: sha256(result) }));
  await writeNewFile(join(cliRow, 'events-projector-process.json'), processEvidence(projectedEvents, target, {
    outcomeSha256: sha256(result),
    eventsSha256: sha256(events),
    eventsProjectorSha256: sha256(await readFile(eventsProjector)),
  }));

  const desktopRow = join(newRoot, 'projected', 'raw', rowId);
  await mkdir(desktopRow, { recursive: true });
  await writeNewFile(join(desktopRow, 'request.json'), parityRequest);
  await writeNewFile(join(desktopRow, 'result.json'), projectedResult.stdout);
  await writeNewFile(join(desktopRow, 'events.ndjson'), projectedEvents.stdout);
  await writeNewFile(join(desktopRow, 'stderr.txt'), cliRun.stderr);
  await writeNewFile(join(desktopRow, 'process.json'), await readFile(join(cliRow, 'process.json')));
}

async function loadAddon(addonPath) {
  if (extname(addonPath) === '.node' || extname(addonPath) === '.cjs') return createRequire(import.meta.url)(addonPath);
  return import(pathToFileURL(addonPath).href);
}

function oneOfArguments(names) {
  const supplied = names.filter((name) => process.argv.includes(name));
  if (supplied.length !== 1) fail(`exactly one of ${names.join(', ')} is required`);
  return resolve(argument(supplied[0]));
}

async function copyOldEvidence(oldRoot, newRoot) {
  await mkdir(newRoot, { recursive: true });
  for (const entry of await readdir(oldRoot, { withFileTypes: true })) {
    await cp(join(oldRoot, entry.name), join(newRoot, entry.name), {
      recursive: entry.isDirectory(),
      dereference: false,
      errorOnExist: true,
      force: false,
    });
  }
}

async function collectFiles(root) {
  const files = [];
  async function visit(directory) {
    for (const entry of (await readdir(directory, { withFileTypes: true })).sort((left, right) => left.name.localeCompare(right.name))) {
      const path = join(directory, entry.name);
      if (entry.isDirectory()) await visit(path);
      else if (entry.isFile()) files.push(path.slice(root.length + 1).split('\\').join('/'));
      else fail(`target evidence has an unsupported filesystem entry: ${path}`);
    }
  }
  await visit(root);
  return files.sort();
}

async function writeTargetManifest(root) {
  const excluded = new Set(['bundle-manifest.json', 'SHA256SUMS']);
  const files = [];
  for (const path of await collectFiles(root)) {
    if (excluded.has(path)) continue;
    const bytes = await readFile(join(root, path));
    files.push({ path, sha256: sha256(bytes), size: bytes.length });
  }
  await writeFile(join(root, 'bundle-manifest.json'), `${canonicalJson({ version: 1, files })}\n`);
  const sums = await Promise.all(files.map(async ({ path }) => `${sha256(await readFile(join(root, path)))}  ${path}`));
  await writeFile(join(root, 'SHA256SUMS'), `${sums.join('\n')}\n`);
}

async function main() {
  const oldRoot = resolve(argument('--old-root'));
  const newRoot = resolve(argument('--new-root'));
  const addonPath = oneOfArguments(['--addon', '--package']);
  const evidencePath = resolve(argument('--evidence'));
  const sourceProvenanceEvidencePath = resolve(argument('--source-provenance-evidence'));
  const target = parityTarget(argument('--target'));
  const sourceRevision = optionalArgument('--source-revision') ?? SOURCE_CONTRACT.workflowSupportRevision;
  if (!/^[a-f0-9]{40}$/.test(sourceRevision)) fail('standalone source revision must be a full SHA');
  const targetKey = optionalArgument('--target-key') ?? PARITY_TARGET_LAYOUT.find((entry) => entry.target === target.target)?.key;
  if (!targetKey) fail('target key is not in the required parity matrix');
  const adapter = optionalArgument('--adapter');
  const cli = optionalArgument('--cli');
  const outcomeProjector = optionalArgument('--outcome-projector');
  const eventsProjector = optionalArgument('--events-projector');
  const cliArguments = [adapter, cli, outcomeProjector, eventsProjector];
  if (cliArguments.some(Boolean) && cliArguments.some((path) => !path)) fail('adapter, CLI, and both projectors must be supplied together');
  const identity = { repository: SOURCE_CONTRACT.repository, workflow: SOURCE_CONTRACT.workflow, ref: SOURCE_CONTRACT.ref, sha: SOURCE_CONTRACT.workflowSupportRevision, artifact: target.artifact };
  const verified = await verifyParityDirectory(oldRoot, { source: identity, provenance: identity, target: target.target });
  const sourceProvenanceEvidence = await readFile(sourceProvenanceEvidencePath, 'utf8');
  validateSourceProvenanceEvidence(sourceProvenanceEvidence, verified, target);
  await copyOldEvidence(oldRoot, newRoot);
  await copyFile(sourceProvenanceEvidencePath, join(newRoot, 'source-provenance-evidence.json'));
  await writeFile(join(newRoot, 'source-provenance.json'), `${canonicalJson({ sourceRevision, sourceVersion: 1, trustedSourceRootKind: 'committed-git-source-at-workflow-sha' })}\n`, { flag: 'wx' });
  const addon = await loadAddon(addonPath);
  const rows = rowIds(verified.rawChecksums);
  for (const rowId of rows) await captureNapiRow(addon, oldRoot, newRoot, rowId, target.target);
  const napiComparisons = await compareParityRows(oldRoot, newRoot, rows);
  let cliComparisons = [];
  let executableIdentities = {};
  if (adapter) {
    const projectorSources = await copyCommittedProjectorSources({
      evidenceRoot: newRoot,
      sourceRevision,
      sourceRoot: SOURCE_ROOT,
    });
    executableIdentities = {
      adapter: await executableIdentity(adapter, 'parity-desktop-request-adapter', newRoot, projectorSources.adapter),
      cli: await executableIdentity(cli, 'polygon-nesting', newRoot),
      outcomeProjector: await executableIdentity(outcomeProjector, 'parity-project-engine-outcome', newRoot, projectorSources.outcomeProjector),
      eventsProjector: await executableIdentity(eventsProjector, 'parity-project-engine-events', newRoot, projectorSources.eventsProjector),
    };
    for (const rowId of rows) await captureCliRow({ oldRoot, newRoot, rowId, target: target.target, adapter, cli, outcomeProjector, eventsProjector });
    cliComparisons = await compareParityRows(oldRoot, newRoot, rows, { newSide: 'projected' });
  }
  const evidence = {
    version: 1,
    targetKey,
    target: target.target,
    acceptedEngineRevision: ACCEPTED_OLD_REVISION,
    sourceArtifact: target.artifact,
    normalizationVersion: NORMALIZATION_VERSION,
    sourceContractVersion: SOURCE_CONTRACT_VERSION,
    sourceRevision,
    executableIdentities,
    comparisons: napiComparisons,
    napiComparisons,
    cliComparisons,
  };
  await mkdir(dirname(evidencePath), { recursive: true });
  await writeFile(evidencePath, `${canonicalJson(evidence)}\n`, { flag: 'wx' });
  if (resolve(dirname(evidencePath)) !== newRoot) fail('parity.json must be written at the target evidence root');
  await writeTargetManifest(newRoot);
}

main().catch((error) => {
  process.stderr.write(`standalone parity comparison failed: ${error.message}\n`);
  process.exitCode = 1;
});
