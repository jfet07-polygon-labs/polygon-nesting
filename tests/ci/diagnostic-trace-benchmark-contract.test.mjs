import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { test } from 'node:test';

import {
  TRACE_FIELDS,
  benchmarkDiagnosticTraceMode,
  diagnosticTraceModeOrder,
} from '../../scripts/benchmark-diagnostic-trace-mode.mjs';

test('benchmark alternates Full and Off, compares only documented fields, and reports deterministic summaries', async () => {
  const inputPath = new URL('../../tests/vectors/protocol/request-v1.json', import.meta.url);
  const requests = [];
  const report = await benchmarkDiagnosticTraceMode({
    inputPath,
    iterations: 3,
    run: async ({ mode, request }) => {
      requests.push({ mode, request });
      const result = {
        version: 1,
        outcome: {
          status: 'success',
          result: mode === 'full'
            ? {
                placements: [{ pieceId: 'piece', runtimeMs: 10 }],
                capacityTrace: { steps: [1, 2, 3] },
                intrinsicAnytimeSchedulerTrace: { decisions: ['keep'] },
                focusedCompleteReconstructionTrace: { complete: true },
                intrinsicShortSideObserverTrace: {},
                intrinsicShortSidePairFoldTrace: {},
              }
            : {
                placements: [{ pieceId: 'piece', runtimeMs: 20 }],
              },
        },
      };
      return {
        exitCode: 0,
        stderr: Buffer.alloc(0),
        resultBytes: Buffer.from(JSON.stringify(result)),
        runtimeMs: mode === 'full' ? 10 : 5,
      };
    },
  });

  assert.deepEqual(diagnosticTraceModeOrder(3), ['full', 'off', 'off', 'full', 'full', 'off']);
  assert.deepEqual(requests.map(({ mode }) => mode), report.order);
  for (const { request } of requests) {
    assert.equal(request.historyMode, 'off');
    assert.ok(['full', 'off'].includes(request.diagnosticTraceMode));
  }
  assert.deepEqual(report.summary.full.runtimeSamplesMs, [10, 10, 10]);
  assert.equal(report.summary.full.minimumRuntimeMs, 10);
  assert.equal(report.summary.full.medianRuntimeMs, 10);
  assert.deepEqual(report.summary.off.runtimeSamplesMs, [5, 5, 5]);
  assert.equal(report.summary.off.minimumRuntimeMs, 5);
  assert.equal(report.summary.off.medianRuntimeMs, 5);
  assert.equal(report.semanticEquivalent, true);
  assert.equal(report.offResultBytesSmaller, true);
  assert.equal(Object.hasOwn(report, 'speedThreshold'), false);
  assert.deepEqual(TRACE_FIELDS, [
    'capacityTrace',
    'intrinsicAnytimeSchedulerTrace',
    'focusedCompleteReconstructionTrace',
    'intrinsicShortSideObserverTrace',
    'intrinsicShortSidePairFoldTrace',
  ]);
  assert.ok((await readFile(inputPath)).includes(Buffer.from('diagnosticTraceMode')));
});
