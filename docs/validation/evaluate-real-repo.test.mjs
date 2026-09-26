import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { evaluate, makeResult, verifyInputFiles } from './evaluate-real-repo.mjs';

const id = name => `class:${Buffer.from(name).toString('hex')}`;
const base = () => ({
  version: 'test',
  items: [
    { itemId: 'MODULE-1', family: 'module', scored: true, status: 'supported', file: 'src/a.ts', line: 1,
      expectedRelation: { from: id('A'), kind: 'depends_on', to: id('B'), evidence: { source: 'nestjs', confidence: 'confirmed', file: 'src/a.ts', line: 1 } } },
    { itemId: 'DI-1', family: 'di', status: 'unsupported', file: 'src/a.ts', line: 2,
      expectedDiagnostic: { code: 'unsupported_di_external', scope: id('A'), file: 'src/a.ts', line: 2 } },
    { itemId: 'ENDPOINT-1', family: 'endpoint', status: 'supported', file: 'src/a.ts', line: 3,
      expectedRelations: [{ from: id('C'), kind: 'exposes', to: id('E'), evidence: { source: 'nestjs', confidence: 'confirmed', file: 'src/a.ts', line: 3 } }] },
  ],
});
const freeze = ledger => ({ ledgerSha256: createHash('sha256').update(JSON.stringify(ledger)).digest('hex') });
const graph = () => ({ nodes: [], edges: [], diagnostics: [], metadata: { callAnalysis: { examinedCalls: 0, emittedCalls: 0, skippedCalls: 0 } } });
const edge = (from, kind, to, line = 1) => ({ from, kind, to, evidence: [{ source: 'nestjs', confidence: 'confirmed', file: 'src/a.ts', line }] });

test('missing true target and itemless owner false relations both count', () => {
  const ledger = base(); const actual = graph();
  ledger.items.push(
    { itemId: 'DECLARATION-M', family: 'declaration', expectedNode: { id: id('M'), kind: 'module' } },
    { itemId: 'DECLARATION-D', family: 'declaration', expectedNode: { id: id('D'), kind: 'service' } },
    { itemId: 'DECLARATION-C', family: 'declaration', expectedNode: { id: id('C2'), kind: 'controller' } },
  );
  actual.edges = [edge(id('A'), 'depends_on', id('X')), edge(id('M'), 'depends_on', id('X')),
    edge(id('D'), 'injects', id('X')), edge(id('C2'), 'exposes', id('E2')), edge(id('C'), 'exposes', id('E'), 3)];
  actual.diagnostics = [{ code: 'unsupported_di_external', relatedNodeId: id('A'), file: 'src/a.ts', line: 2 }];
  const result = evaluate(JSON.stringify(ledger), freeze(ledger), actual);
  assert.equal(result.families.module.missing, 1);
  assert.equal(result.families.module.false, 2);
  assert.equal(result.families.di.false, 1);
  assert.equal(result.families.endpoint.false, 1);
  assert.equal(result.families.module.verdict, 'EXCEEDED');
});

test('classification change cannot hide missing after freeze', () => {
  const ledger = base(); const saved = freeze(ledger);
  ledger.items[0].status = 'unsupported';
  assert.throws(() => evaluate(JSON.stringify(ledger), saved, graph()), /freeze hash mismatch/);
});

test('duplicate and wrong-scope diagnostics are preserved', () => {
  const ledger = base(); const actual = graph();
  actual.diagnostics = [
    { code: 'unsupported_di_external', relatedNodeId: id('X'), file: 'src/a.ts', line: 2 },
    { code: 'unsupported_di_external', relatedNodeId: id('X'), file: 'src/a.ts', line: 2 },
  ];
  const result = evaluate(JSON.stringify(ledger), freeze(ledger), actual);
  assert.equal(result.families.di.silentOmission, 1);
  assert.equal(result.diagnosticDuplicates.length, 1);
  assert.equal(result.diagnosticWrongScope.length, 1);
});

test('zero denominator, unassessed items, and error diagnostics are never success', () => {
  const ledger = { version: 'test', items: [{ itemId: 'DI-2', family: 'di', status: 'undecided', file: 'src/a.ts', line: 1 }] };
  const result = evaluate(JSON.stringify(ledger), freeze(ledger), graph());
  assert.equal(result.families.module.missingRate, null);
  assert.equal(result.families.module.verdict, 'NOT_ASSESSABLE');
  assert.equal(result.families.di.verdict, 'NOT_ASSESSABLE');
  const withError = graph();
  withError.metadata.callAnalysis = { scope: 'parsed_named_class_methods', mode: 'same_class_only', examinedCalls: 0, emittedCalls: 0, skippedCalls: 0 };
  withError.diagnostics = [{ code: 'TS_PARSE_ERROR', severity: 'error', file: 'src/a.ts', line: 1 }];
  const errorResult = evaluate(JSON.stringify(ledger), freeze(ledger), withError);
  assert.equal(errorResult.technicalVerdict, 'EXCEEDED');
  assert.deepEqual(errorResult.errorDiagnostics, [{ code: 'TS_PARSE_ERROR', file: 'src/a.ts', line: 1 }]);
});

test('family totals derive from item ledger, including unsupported source units', () => {
  const ledger = base(); const actual = graph();
  actual.edges = [edge(id('A'), 'depends_on', id('B')), edge(id('C'), 'exposes', id('E'), 3)];
  actual.diagnostics = [{ code: 'unsupported_di_external', relatedNodeId: id('A'), file: 'src/a.ts', line: 2 }];
  const result = evaluate(JSON.stringify(ledger), freeze(ledger), actual);
  assert.deepEqual([result.families.module.sourceItems, result.families.module.supportedExpected, result.families.module.correct, result.families.module.missing], [1, 1, 1, 0]);
  assert.deepEqual([result.families.di.sourceItems, result.families.di.unsupported, result.families.di.diagnosed, result.families.di.unresolved], [1, 1, 1, 1]);
  assert.deepEqual(result.families.di.diagnosedItems, [{ itemId: 'DI-1', code: 'unsupported_di_external', file: 'src/a.ts', line: 2, relatedNodeId: id('A') }]);
  assert.equal(result.families.di.verdict, 'EXCEEDED');
});

test('call unknown aggregation keeps reason, representative line, and site count', () => {
  const method = `method:${Buffer.from(id('C')).toString('hex')}:696e7374616e6365:666f6f`;
  const ledger = { version: 'test', criteria: {}, items: [
    { itemId: 'CALL-1', family: 'call', file: 'src/a.ts', line: 5, status: 'unsupported', expectedUnknown: { scope: method, code: 'unsupported_call_injected_receiver', file: 'src/a.ts', line: 5 } },
    { itemId: 'CALL-2', family: 'call', file: 'src/a.ts', line: 8, status: 'unsupported', expectedUnknown: { scope: method, code: 'unsupported_call_injected_receiver', file: 'src/a.ts', line: 8 } },
  ], expectedCallUnknownGroups: [{ scope: method, code: 'unsupported_call_injected_receiver', file: 'src/a.ts', representativeLine: 5, skippedCount: 2, itemIds: ['CALL-1', 'CALL-2'] }] };
  const actual = graph(); actual.metadata.callAnalysis = { examinedCalls: 2, emittedCalls: 0, skippedCalls: 2 };
  actual.diagnostics = [{ code: 'unsupported_call_injected_receiver', relatedNodeId: method, file: 'src/a.ts', line: 8, skippedCount: 2 }];
  const result = evaluate(JSON.stringify(ledger), freeze(ledger), actual);
  assert.equal(result.calls.unknownMismatches.length, 1);
  assert.equal(result.calls.unknownMismatches[0].expectedLine, 5);
});

test('unexpected declaration and method-origin call both fail', () => {
  const ledger = base(); const actual = graph();
  actual.nodes = [{ id: id('Phantom'), kind: 'interface' }, { id: 'method:phantom', kind: 'method' }];
  actual.edges = [edge('method:phantom', 'calls', 'method:elsewhere')];
  const result = evaluate(JSON.stringify(ledger), freeze(ledger), actual);
  assert.deepEqual(result.unexpectedNodes.map(n => n.id), [id('Phantom'), 'method:phantom']);
  assert.equal(result.calls.falseEdges.length, 1);
  assert.equal(result.technicalVerdict, 'EXCEEDED');
});

test('node and edge evidence defects stay visible', () => {
  const ledger = base();
  ledger.items.push({ itemId: 'DECLARATION-1', family: 'declaration', status: 'supported', expectedNode: {
    id: id('A'), kind: 'module', name: 'A', file: 'src/a.ts', line: 1,
    evidence: { source: 'ast', confidence: 'confirmed', file: 'src/a.ts', line: 1 },
  } });
  const actual = graph();
  actual.nodes = [{ id: id('A'), kind: 'module', name: 'A', file: 'src/a.ts', line: 1, evidence: [] }];
  actual.edges = [edge(id('A'), 'depends_on', id('B'), 9), edge(id('C'), 'exposes', id('E'), 3)];
  actual.diagnostics = [{ code: 'unsupported_di_external', relatedNodeId: id('A'), file: 'src/a.ts', line: 2 }];
  const result = evaluate(JSON.stringify(ledger), freeze(ledger), actual);
  assert.deepEqual(result.provenanceErrors.map(e => e.entity), ['node', 'edge']);
  assert.equal(result.technicalVerdict, 'EXCEEDED');
});

test('call target, skipped group duplicate and unexpected scope fail separately', () => {
  const method = 'method:owner'; const target = 'method:target';
  const ledger = { version: 'test', items: [
    { itemId: 'METHOD-1', family: 'method', status: 'supported', expectedNode: { id: method, kind: 'method', name: 'owner', file: 'src/a.ts', line: 1, evidence: { source: 'ast', confidence: 'confirmed', file: 'src/a.ts', line: 1 } } },
    { itemId: 'CALL-1', family: 'call', status: 'supported', expectedRelation: { from: method, kind: 'calls', to: target, evidence: { source: 'ast', confidence: 'confirmed', file: 'src/a.ts', line: 2 } } },
    { itemId: 'CALL-2', family: 'call', status: 'unsupported', expectedUnknown: { scope: method, code: 'unsupported_call_unknown_receiver' } },
  ], expectedCallUnknownGroups: [{ scope: method, code: 'unsupported_call_unknown_receiver', file: 'src/a.ts', representativeLine: 3, skippedCount: 1 }] };
  const actual = graph();
  actual.nodes = [{ id: method, kind: 'method', name: 'owner', file: 'src/a.ts', line: 1, evidence: [{ source: 'ast', confidence: 'confirmed', file: 'src/a.ts', line: 1 }] }];
  actual.edges = [{ from: method, kind: 'calls', to: 'method:wrong' }];
  actual.metadata.callAnalysis = { scope: 'parsed_named_class_methods', mode: 'same_class_only', examinedCalls: 2, emittedCalls: 1, skippedCalls: 1 };
  actual.diagnostics = [
    { code: 'unsupported_call_unknown_receiver', relatedNodeId: method, file: 'src/a.ts', line: 3, skippedCount: 1 },
    { code: 'unsupported_call_unknown_receiver', relatedNodeId: method, file: 'src/a.ts', line: 3, skippedCount: 1 },
    { code: 'unsupported_call_unknown_receiver', relatedNodeId: 'method:other', file: 'src/a.ts', line: 4, skippedCount: 1 },
  ];
  const result = evaluate(JSON.stringify(ledger), freeze(ledger), actual);
  assert.equal(result.calls.missingEdges.length, 1);
  assert.equal(result.calls.falseEdges.length, 1);
  assert.equal(result.calls.duplicateGroups.length, 1);
  assert.ok(result.calls.unknownMismatches.some(m => m.reason === 'unexpected group'));
  assert.equal(result.technicalVerdict, 'EXCEEDED');
});

test('input inventory covers analyzer root, manifest and source bytes', () => {
  const root = mkdtempSync(join(tmpdir(), 'issue30-input-test-'));
  try {
    mkdirSync(join(root, 'src'));
    const source = 'export class A {}\n'; const sourceSha = createHash('sha256').update(source).digest('hex');
    const ledger = { sourceInventory: [{ file: 'src/a.ts', sha256: sourceSha }] };
    const manifestSha = createHash('sha256').update(`${sourceSha}  src/a.ts\n`).digest('hex');
    const frozen = { inputSourceManifestSha256: manifestSha };
    assert.throws(() => verifyInputFiles(ledger, frozen, root), /inventory mismatch/);
    writeFileSync(join(root, 'src/a.ts'), source);
    writeFileSync(join(root, 'extra.ts'), '');
    assert.throws(() => verifyInputFiles(ledger, frozen, root), /inventory mismatch/);
    rmSync(join(root, 'extra.ts'));
    writeFileSync(join(root, 'schema.prisma'), '');
    assert.throws(() => verifyInputFiles(ledger, frozen, root), /inventory mismatch/);
    rmSync(join(root, 'schema.prisma'));
    assert.throws(() => verifyInputFiles(ledger, { inputSourceManifestSha256: 'bad' }, root), /manifest freeze mismatch/);
    writeFileSync(join(root, 'src/a.ts'), 'changed\n');
    assert.throws(() => verifyInputFiles(ledger, frozen, root), /source hash mismatch/);
    writeFileSync(join(root, 'src/a.ts'), source);
    assert.equal(verifyInputFiles(ledger, frozen, root).sourceFiles, 1);
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('saved result binds graph bytes, metadata and ledger freeze deterministically', () => {
  const ledger = { version: 'test', items: [] }; const saved = freeze(ledger);
  const actual = graph(); actual.metadata = { analyzerVersion: '0.1.0', analyzedAt: '2026-09-26T00:00:00Z', rootName: 'input', callAnalysis: { scope: 'parsed_named_class_methods', mode: 'same_class_only', examinedCalls: 0, emittedCalls: 0, skippedCalls: 0 } };
  const graphText = JSON.stringify(actual); const verification = { sourceFiles: 1, manifestSha256: 'manifest' };
  const result = makeResult(JSON.stringify(ledger), { ...saved, inputSourceManifestSha256: 'manifest' }, actual, graphText, verification);
  assert.deepEqual(result, makeResult(JSON.stringify(ledger), { ...saved, inputSourceManifestSha256: 'manifest' }, actual, graphText, verification));
  assert.equal(result.provenance.graphSha256, createHash('sha256').update(graphText).digest('hex'));
  assert.equal(result.provenance.ledgerSha256, saved.ledgerSha256);
  assert.equal(result.provenance.analyzedAt, '2026-09-26T00:00:00Z');
});
