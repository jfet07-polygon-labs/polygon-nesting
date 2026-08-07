const fs = require('node:fs');
const path = require('node:path');

const addonPath = process.argv[2];
if (!addonPath) {
  throw new Error('usage: node nonterminal_callback_throw.cjs <addon.node>');
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
request.options.diagnosticTraceMode = 'full';

const frames = [];
addon
  .runIrregularJob(
    JSON.stringify(request),
    'nonterminal-callback-throw-regression',
    (json) => {
      const frame = JSON.parse(json);
      frames.push(frame);
      if (frame.kind !== 'terminal') {
        throw new Error('nonterminal callback throw');
      }
    },
    true,
  )
  .then((json) => {
    const response = JSON.parse(json);
    const terminals = frames.filter((frame) => frame.kind === 'terminal');
    if (terminals.length !== 1) {
      throw new Error(`expected exactly one terminal frame, got ${terminals.length}`);
    }
    if (response.ok !== false) {
      throw new Error(`expected failure envelope, got ${json}`);
    }
    if (response.error.category !== 'worker_protocol_error') {
      throw new Error(`unexpected category: ${response.error.category}`);
    }
    if (response.error.operation !== 'nativeEventDelivery') {
      throw new Error(`unexpected operation: ${response.error.operation}`);
    }
    if (response.error.context.napiStatus !== 'PendingException') {
      throw new Error(`unexpected napi status: ${response.error.context.napiStatus}`);
    }
    const diagnostics = JSON.parse(addon.getLastJobDiagnostics());
    if (!diagnostics || !Number.isInteger(diagnostics.threadCountRequested)) {
      throw new Error('completion did not publish execution diagnostics');
    }
    console.log('nonterminal callback throw regression passed');
  })
  .catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
