import { describe, expect, it } from 'vitest';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import fixture from '../../../examples/nestjs-sample/expected-graph.json';
import { SystemGraphSchema, type Diagnostic } from './graph';
import { graphDiagnostics, diagnosticPage } from './graphDiagnostics';
import { AnalysisPanel } from './AnalysisPanel';
import { nodeDetails } from './nodeDetails';

const graph = SystemGraphSchema.parse(fixture);
const valid = (diagnostics: Diagnostic[]) => SystemGraphSchema.parse({ ...graph,
  edges: graph.edges.filter(e => e.kind !== 'calls'), diagnostics,
  metadata: { ...graph.metadata, callAnalysis: { ...graph.metadata.callAnalysis, examinedCalls: 0, emittedCalls: 0, skippedCalls: 0 } },
});
const render = (g = graph) => renderToStaticMarkup(createElement(AnalysisPanel, {
  graph: g, generation: 1, visibleIds: new Set<string>(), onNavigate: () => {},
}));

describe('whole snapshot diagnostics', () => {
  it('retains every fixture field and separates global counters from local details', () => {
    const model = graphDiagnostics(graph);
    expect(model.counts).toEqual({ info: 0, warning: 15, error: 0 });
    expect(model.rows).toHaveLength(15);
    const reasons = Object.fromEntries(model.codes.filter(code => code.startsWith('unsupported_call_')).map(code => [code,
      model.rows.filter(r => r.diagnostic.code === code).reduce((n, r) => n + (r.diagnostic.skippedCount ?? 0), 0)]));
    expect(reasons).toEqual({ unsupported_call_injected_receiver: 5, unsupported_call_computed_target: 1, unsupported_call_nested_function: 1 });
    expect(model.rows.map(r => r.diagnostic)).toEqual(expect.arrayContaining(graph.diagnostics));
    expect(model.rows.filter(r => r.diagnostic.skippedCount !== undefined).reduce((n, r) => n + r.diagnostic.skippedCount!, 0)).toBe(7);
    expect(graph.metadata.callAnalysis).toMatchObject({ examinedCalls: 9, emittedCalls: 2, skippedCalls: 7 });
    for (const [name, count] of [['UsersService', 3], ['choose', 2]] as const) {
      expect(nodeDetails(graph, graph.nodes.find(n => n.name === name)!.id)!.skipped).toBe(count);
    }
    const html = render();
    for (const text of ['parsed_named_class_methods', 'same_class_only', 'confirmed', 'best_effort', 'unsupported / unknown', graph.metadata.analyzedAt, 'requested_token']) expect(html).toContain(text);
  });
  it('sorts copied tuples deterministically and retains exact ordinary duplicates with distinct keys', () => {
    const d: Diagnostic = { code: 'NEW', severity: 'info', message: 'same' };
    const g = valid([d, d, { ...d, message: 'other' }]);
    const before = JSON.stringify(g);
    const model = graphDiagnostics(g);
    expect(model.rows).toHaveLength(3);
    expect(new Set(model.rows.map(r => r.key)).size).toBe(3);
    expect(graphDiagnostics(SystemGraphSchema.parse({ ...g, diagnostics: [...g.diagnostics].reverse() }))).toEqual(model);
    expect(JSON.stringify(g)).toBe(before);
  });
  it('separates total, matching and displayed counts and reaches the final row', () => {
    const g = valid(Array.from({ length: 123 }, (_, i) => ({ code: i % 2 ? 'ODD' : 'EVEN', severity: 'info', message: String(i).padStart(3, '0') })));
    const model = graphDiagnostics(g);
    const filters = { severity: '', code: '', skippedOnly: false };
    expect(diagnosticPage(model.rows, filters, 50).rows).toHaveLength(50);
    expect(diagnosticPage(model.rows, filters, 100).rows).toHaveLength(100);
    expect(diagnosticPage(model.rows, filters, 150).rows).toHaveLength(123);
    expect(diagnosticPage(model.rows, { ...filters, code: 'EVEN' }, 50).total).toBe(62);
    expect(diagnosticPage(model.rows, { ...filters, severity: 'error' }, 50).total).toBe(0);
    expect(model.rows).toHaveLength(123);
  });
  it('recognizes skipped sites by presence, not code prefix or row count', () => {
    const copy = structuredClone(graph);
    const d = copy.diagnostics.find(d => d.skippedCount !== undefined)!;
    d.code = 'FUTURE_REASON'; d.skippedCount! += 2;
    copy.metadata.callAnalysis.examinedCalls += 2; copy.metadata.callAnalysis.skippedCalls += 2;
    const g = SystemGraphSchema.parse(copy);
    const rows = diagnosticPage(graphDiagnostics(g).rows, { severity: '', code: '', skippedOnly: true }, 50).rows;
    expect(rows.some(r => r.diagnostic.code === 'FUTURE_REASON')).toBe(true);
    expect(rows.reduce((n, r) => n + r.diagnostic.skippedCount!, 0)).toBe(9);
    expect(rows.length).toBeLessThan(9);
  });
  it('preserves error severity, optional positions and untrusted text', () => {
    const g = valid([
      { severity: 'error', code: 'PARSE_NEW', message: '<script>日本語</script>', file: 'src/未解析.ts' },
      { severity: 'warning', code: 'UNKNOWN', message: 'global notice' },
      { severity: 'info', code: 'INFO', message: 'information' },
    ]);
    expect(graphDiagnostics(g).counts).toEqual({ info: 1, warning: 1, error: 1 });
    const html = render(g);
    expect(html).toContain('Graph may be incomplete');
    expect(html).toContain('Examined call sites: 0');
    expect(html).toContain('&lt;script&gt;日本語&lt;/script&gt;');
    expect(html).not.toContain('<script>');
    expect(html).toContain('src/未解析.ts</p>');
    expect(html).not.toContain('src/未解析.ts:1');
    expect(html).toContain('Global notice — no related node recorded.');
    expect(html).not.toContain('Go to node:');
    expect(render(valid([]))).toContain('No analyzer diagnostics for this graph.');
    expect(render(valid([]))).toContain('does not guarantee complete analysis');
  });
  it('does not reinterpret emitted sites as deduplicated edges', () => {
    const copy = structuredClone(graph);
    copy.metadata.callAnalysis.emittedCalls += 3; copy.metadata.callAnalysis.examinedCalls += 3;
    const g = SystemGraphSchema.parse(copy);
    expect(render(g)).toContain('Emitted call sites: 5');
    expect(g.edges.filter(e => e.kind === 'calls')).toHaveLength(2);
  });
  it.each([
    { code: 'BAD', severity: 'error', message: 'bad', relatedNodeId: 'missing' },
    { code: 'BAD', severity: 'error', message: 'bad', file: '../private.ts' },
    { code: 'BAD', severity: 'error', message: 'bad', line: 1 },
  ])('keeps import rejection for malformed diagnostics', d => {
    expect(SystemGraphSchema.safeParse({ ...graph, diagnostics: [...graph.diagnostics, d] }).success).toBe(false);
  });
  it('rejects inconsistent skipped totals and duplicate method/reason groups', () => {
    expect(SystemGraphSchema.safeParse({ ...graph, diagnostics: graph.diagnostics.filter(d => d.skippedCount === undefined) }).success).toBe(false);
    expect(SystemGraphSchema.safeParse({ ...graph, diagnostics: [...graph.diagnostics, graph.diagnostics.find(d => d.skippedCount !== undefined)!] }).success).toBe(false);
  });
});

it('renders every diagnostic field with distinct method identities and does not substitute definition positions', () => {
  const methods = graph.nodes.filter(n => n.kind === 'method' && n.name === 'find');
  const g = valid(methods.flatMap(n => [
    { code: 'NEW_REASON', severity: 'info' as const, message: 'same name', relatedNodeId: n.id },
    { code: 'NEW_REASON', severity: 'warning' as const, message: 'own location', relatedNodeId: n.id, file: 'src/diagnostic.ts', line: 42 },
  ]));
  const html = render(g);
  for (const n of methods) expect(html).toContain(n.id);
  expect(html).toContain('instance'); expect(html).toContain('static');
  expect(html).toContain('Source location not recorded</p>');
  expect(html).toContain('src/diagnostic.ts:42</p>');
  expect(graphDiagnostics(g).rows.map(r => r.diagnostic)).toEqual(expect.arrayContaining(g.diagnostics));
});
