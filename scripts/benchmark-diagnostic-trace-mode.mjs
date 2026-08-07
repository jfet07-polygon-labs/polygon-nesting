#!/usr/bin/env node

import { execFile } from 'node:child_process';
import { existsSync } from 'node:fs';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';

import { normalizeSemanticJson } from './parity/verify-parity-bundle.mjs';

const execFileAsync = promisify(execFile);
const REPOSITORY_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const DEFAULT_INPUT = join(REPOSITORY_ROOT, 'tests/fixtures/cli/request-v1.json');
const TRACE_FIELDS = Object.freeze([
  'capacityTrace',
  'intrinsicAnytimeSchedulerTrace',
  'focusedCompleteReconstructionTrace',
  'intrinsicShortSideObserverTrace',
  'intrinsicShortSidePairFoldTrace',
]);

function fail(message) {
  throw new Error(`diagnostic trace benchmark: ${message}`);
}

function canonicalJson(value) {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`;
  if (value && typeof value === 'object') {
    return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(',')}}`;
  }
  return JSON.stringify(value);
}

function removeDiagnosticTraceFields(value) {
  if (Array.isArray(value)) return value.map(removeDiagnosticTraceFields);
  if (!value || typeof value !== 'object') return value;
  return Object.fromEntries(
    Object.entries(value)
      .filter(([key]) => !TRACE_FIELDS.includes(key))
      .map(([key, nested]) => [key, removeDiagnosticTraceFields(nested)]),
  );
}

function normalizedSemanticValue(value) {
  return normalizeSemanticJson(removeDiagnosticTraceFields(value));
}

function median(values) {
  const sorted = [...values].sort((left, right) => left - right);
  if (sorted.length === 0) fail('cannot calculate a median without samples');
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 0
    ? (sorted[middle - 1] + sorted[middle]) / 2
    : sorted[middle];
}

function minimum(values) {
  if (values.length === 0) fail('cannot calculate a minimum without samples');
  return Math.min(...values);
}

function positiveInteger(value, label) {
  if (!Number.isSafeInteger(value) || value < 1) fail(`${label} must be a positive safe integer`);
  return value;
}

export function diagnosticTraceModeOrder(iterations) {
  positiveInteger(iterations, 'iterations');
  const order = [];
  for (let index = 0; index < iterations; index += 1) {
    order.push(...(index % 2 === 0 ? ['full', 'off'] : ['off', 'full']));
  }
  return order;
}

export { TRACE_FIELDS };

async function runCli({ cliPath, inputPath, outputPath }) {
  const started = process.hrtime.bigint();
  let result;
  try {
    result = await execFileAsync(
      cliPath,
      ['run', '--input', inputPath, '--output', outputPath],
      { cwd: REPOSITORY_ROOT, encoding: 'buffer', maxBuffer: 16 * 1024 * 1024 },
    );
  } catch (error) {
    const runtimeMs = Number(process.hrtime.bigint() - started) / 1_000_000;
    return {
      exitCode: error.code ?? 1,
      stderr: Buffer.isBuffer(error.stderr) ? error.stderr : Buffer.from(String(error.stderr ?? error.message ?? '')),
      resultBytes: null,
      runtimeMs,
    };
  }
  return {
    exitCode: 0,
    stderr: Buffer.isBuffer(result.stderr) ? result.stderr : Buffer.from(result.stderr ?? ''),
    resultBytes: await readFile(outputPath),
    runtimeMs: Number(process.hrtime.bigint() - started) / 1_000_000,
  };
}

function defaultCliPath() {
  const configured = process.env.POLYGON_NESTING_CLI;
  if (configured) return resolve(configured);
  for (const candidate of [
    join(REPOSITORY_ROOT, 'target/release/polygon-nesting'),
    join(REPOSITORY_ROOT, 'target/debug/polygon-nesting'),
  ]) {
    if (existsSync(candidate)) return candidate;
  }
  return join(REPOSITORY_ROOT, 'target/release/polygon-nesting');
}

function parseRequest(inputBytes, inputPath) {
  let request;
  try {
    request = JSON.parse(inputBytes.toString('utf8'));
  } catch (error) {
    fail(`input ${inputPath} is not valid JSON: ${error.message}`);
  }
  if (!request || typeof request !== 'object' || Array.isArray(request)) {
    fail(`input ${inputPath} must contain one request object`);
  }
  return request;
}

function summarizeRuns(runs) {
  const runtimeSamplesMs = runs.map(({ runtimeMs }) => runtimeMs);
  const resultBytes = runs.map(({ resultBytes }) => resultBytes);
  return {
    runtimeSamplesMs,
    minimumRuntimeMs: minimum(runtimeSamplesMs),
    medianRuntimeMs: median(runtimeSamplesMs),
    resultBytes,
    minimumResultBytes: minimum(resultBytes),
    medianResultBytes: median(resultBytes),
  };
}

export async function benchmarkDiagnosticTraceMode({
  cliPath = defaultCliPath(),
  inputPath = DEFAULT_INPUT,
  iterations = 5,
  run = runCli,
} = {}) {
  positiveInteger(iterations, 'iterations');
  const resolvedInputPath = inputPath instanceof URL ? fileURLToPath(inputPath) : resolve(inputPath);
  const requestTemplate = parseRequest(await readFile(resolvedInputPath), resolvedInputPath);
  const order = diagnosticTraceModeOrder(iterations);
  const temporaryRoot = await mkdtemp(join(tmpdir(), 'polygon-nesting-diagnostic-trace-'));
  const runs = [];
  const normalizedByMode = new Map();

  try {
    for (const [index, mode] of order.entries()) {
      const request = JSON.parse(JSON.stringify(requestTemplate));
      request.historyMode = 'off';
      request.diagnosticTraceMode = mode;
      const requestPath = join(temporaryRoot, `${index}-${mode}-request.json`);
      const outputPath = join(temporaryRoot, `${index}-${mode}-result.json`);
      await writeFile(requestPath, `${canonicalJson(request)}\n`);

      const execution = await run({
        cliPath: resolve(cliPath),
        inputPath: requestPath,
        outputPath,
        mode,
        request,
      });
      if (execution.exitCode !== 0) {
        fail(`${mode} run ${index} exited with ${execution.exitCode}: ${execution.stderr?.toString('utf8') ?? ''}`);
      }
      if (!Buffer.isBuffer(execution.resultBytes)) fail(`${mode} run ${index} did not return result bytes`);
      let result;
      try {
        result = JSON.parse(execution.resultBytes.toString('utf8'));
      } catch (error) {
        fail(`${mode} run ${index} returned invalid result JSON: ${error.message}`);
      }
      const normalized = canonicalJson(normalizedSemanticValue(result));
      const previous = normalizedByMode.get(mode);
      if (previous !== undefined && previous !== normalized) {
        fail(`${mode} results are not deterministic across repeated runs`);
      }
      normalizedByMode.set(mode, normalized);
      runs.push({
        index,
        mode,
        runtimeMs: execution.runtimeMs,
        resultBytes: execution.resultBytes.length,
      });
    }
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }

  const fullNormalized = normalizedByMode.get('full');
  const offNormalized = normalizedByMode.get('off');
  if (fullNormalized === undefined || offNormalized === undefined) fail('both Full and Off modes must run');
  if (fullNormalized !== offNormalized) fail('Full and Off results differ after trace removal and documented timing normalization');

  const fullRuns = runs.filter(({ mode }) => mode === 'full');
  const offRuns = runs.filter(({ mode }) => mode === 'off');
  const summary = {
    full: summarizeRuns(fullRuns),
    off: summarizeRuns(offRuns),
  };
  const offResultBytesSmaller = summary.off.minimumResultBytes < summary.full.minimumResultBytes;
  if (!offResultBytesSmaller) fail('Off result bytes are not smaller for the trace-producing fixture');

  return {
    version: 1,
    cli: resolve(cliPath),
    input: resolvedInputPath,
    iterations,
    order,
    runs,
    semanticEquivalent: true,
    offResultBytesSmaller,
    summary,
  };
}

function argument(name) {
  const index = process.argv.indexOf(name);
  if (index === -1 || !process.argv[index + 1]) fail(`${name} is required`);
  return process.argv[index + 1];
}

async function main() {
  const cliPath = process.argv.includes('--cli') ? argument('--cli') : defaultCliPath();
  const inputPath = process.argv.includes('--input') ? argument('--input') : DEFAULT_INPUT;
  const iterations = process.argv.includes('--iterations') ? Number(argument('--iterations')) : 5;
  const report = await benchmarkDiagnosticTraceMode({ cliPath, inputPath, iterations });
  process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch((error) => {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  });
}
