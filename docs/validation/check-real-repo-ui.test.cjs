const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const { mkdtempSync, rmSync, writeFileSync } = require('node:fs');
const { tmpdir } = require('node:os');
const { join } = require('node:path');
const { test } = require('node:test');
const { compareSummaries, comparableSummary } = require('./check-real-repo-ui.cjs');

test('UI evidence mismatch names fields and exits nonzero', () => {
  const expected = { canvas: { nodes: '105' }, selections: [{ contextSha256: 'fixed' }], requestsAfterImport: [], consoleErrors: [] };
  const changed = { canvas: { nodes: '104' }, selections: [{ contextSha256: 'other' }], requestsAfterImport: [{ url: '/upload' }], consoleErrors: ['broken'] };
  assert.deepEqual(compareSummaries(expected, expected), []);
  assert.deepEqual(compareSummaries(expected, changed), [
    'summary.canvas.nodes', 'summary.selections[0].contextSha256', 'summary.requestsAfterImport.length', 'summary.consoleErrors.length',
  ]);
  const root = mkdtempSync(join(tmpdir(), 'issue30-ui-summary-test-'));
  try {
    const expectedPath = join(root, 'expected.json'), changedPath = join(root, 'changed.json');
    writeFileSync(expectedPath, JSON.stringify(expected));
    writeFileSync(changedPath, JSON.stringify(changed));
    const result = spawnSync(process.execPath, [join(__dirname, 'check-real-repo-ui.cjs'), '--compare', expectedPath, changedPath], { encoding: 'utf8' });
    assert.equal(result.status, 1);
    assert.match(result.stderr, /summary\.canvas\.nodes/);
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('fresh snapshot ignores only time-dependent hashes', () => {
  const expected = { graphSha256: 'old', analyzedAt: 'old time', canvas: { nodes: '105' }, selections: [{ contextSha256: 'old context', contextHasScope: true }] };
  const fresh = { graphSha256: 'new', analyzedAt: 'new time', canvas: { nodes: '105' }, selections: [{ contextSha256: 'new context', contextHasScope: true }] };
  assert.deepEqual(compareSummaries(comparableSummary(expected, true), comparableSummary(fresh, true)), []);
  fresh.canvas.nodes = '104';
  assert.deepEqual(compareSummaries(comparableSummary(expected, true), comparableSummary(fresh, true)), ['summary.canvas.nodes']);
});
