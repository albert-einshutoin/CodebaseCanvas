import assert from 'node:assert/strict';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { binaryFromBuild, readGeneratedGraph, requireSuccess } from './prepare.ts';

test('build and CLI failures stop before browser execution', () => {
  assert.throws(() => requireSuccess('Rust build', 1), /Rust build exited 1/);
  assert.throws(() => requireSuccess('CLI analyze', 2), /CLI analyze exited 2/);
  assert.throws(() => binaryFromBuild('{"reason":"build-finished","success":true}'), /No current binary/);
});

test('only the expected newly generated regular JSON file is accepted', () => {
  const root = mkdtempSync(join(tmpdir(), 'canvas-e2e-helper-'));
  try {
    const expected = join(root, 'fresh', '.codebasecanvas', 'graph.json');
    const stale = join(root, 'stale.json');
    writeFileSync(stale, JSON.stringify({ nodes: [], edges: [], diagnostics: [], metadata: { analyzedAt: '2026-09-24T00:00:00Z' } }));
    assert.throws(() => readGeneratedGraph(expected), /Generated graph is missing/);
    writeFileSync(join(root, 'invalid.json'), '{');
    assert.throws(() => readGeneratedGraph(join(root, 'invalid.json')), /not valid JSON/);
    assert.throws(() => readGeneratedGraph(root), /not a regular file/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
