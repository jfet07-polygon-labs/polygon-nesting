const fs = require('node:fs');
const path = require('node:path');

const addonPath = process.argv[2];
if (!addonPath) {
  throw new Error('usage: node preexecution_diagnostics.cjs <addon.node>');
}

const addon = require(addonPath);
const request = JSON.parse(
  fs.readFileSync(
    path.join(
      __dirname,
      '../../../tests/fixtures/mixed-61/300x300-compact/request.json',
    ),
    'utf8',
  ),
);
request.pieces = request.pieces.slice(0, 1);
request.sourcePieces = request.sourcePieces.slice(0, 1);
request.options.timeoutMs = 1000;
request.options.historyMode = 'stream';

function terminalOnlyCallback(json) {
  const frame = JSON.parse(json);
  if (frame.kind !== 'terminal' && !Number.isInteger(frame.ordinal)) {
    throw new Error('invalid core event frame');
  }
}

(async () => {
  const success = JSON.parse(
    await addon.runIrregularJob(
      JSON.stringify(request),
      'preexecution-diagnostics-success',
      terminalOnlyCallback,
      true,
    ),
  );
  if (success.ok !== true) {
    throw new Error(`expected successful job, got ${JSON.stringify(success)}`);
  }
  const previous = addon.getLastJobDiagnostics();

  const malformed = JSON.parse(
    await addon.runIrregularJob(
      '{not json',
      'preexecution-diagnostics-malformed',
      terminalOnlyCallback,
      true,
    ),
  );
  if (malformed.error?.operation !== 'decodeRequestJson') {
    throw new Error(`expected decode failure, got ${JSON.stringify(malformed)}`);
  }
  const diagnostics = JSON.parse(addon.getLastJobDiagnostics());
  if (diagnostics.threadCountRequested !== 1 || diagnostics.threadCountUsed !== 1) {
    throw new Error(`expected no-pool diagnostics, got ${JSON.stringify(diagnostics)}`);
  }
  if (!(diagnostics.wallClockMs >= 0)) {
    throw new Error(`expected measured nonnegative elapsed time, got ${diagnostics.wallClockMs}`);
  }
  if (addon.getLastJobDiagnostics() === previous) {
    throw new Error('malformed completion retained the successful job diagnostics');
  }
  console.log('pre-execution diagnostics regression passed');
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
