import { describe, it, expect } from 'vitest';
import fixture from '../../../examples/nestjs-sample/expected-graph.json';
import { SystemGraphSchema } from './graph';
import { nodeDetails, sortedEvidence } from './nodeDetails';

const graph = SystemGraphSchema.parse(fixture);
const node = (name: string) => graph.nodes.find(n => n.name === name)!;
describe('canonical node details', () => {
  it('keeps shared memberships, hidden methods and scoped skipped sites independent of projection', () => {
    const d = nodeDetails(graph, node('UsersService').id)!;
    expect(d.memberships.map(n => n.name)).toEqual(['AuthModule', 'UsersModule']);
    expect(d.methods).toHaveLength(4);
    expect(d.node.parentId).toBeUndefined();
    expect(d.skipped).toBe(3);
    expect(nodeDetails(graph, node('choose').id)!.skipped).toBe(2);
    expect(d.outgoing.some(r => r.label === 'Requests token')).toBe(true);
    expect(d.incoming.some(r => r.label === 'Requested by')).toBe(true);
  });
  it('is stable under array reordering without mutating the graph', () => {
    const before = JSON.stringify(graph);
    const reordered = structuredClone(graph);
    reordered.nodes.reverse(); reordered.edges.reverse(); reordered.diagnostics.reverse();
    for (const n of reordered.nodes) n.evidence.reverse();
    for (const e of reordered.edges) e.evidence.reverse();
    for (const n of graph.nodes) expect(nodeDetails(reordered, n.id)).toEqual(nodeDetails(graph, n.id));
    expect(JSON.stringify(graph)).toBe(before);
    expect(nodeDetails(graph, 'missing')).toBeUndefined();
  });
  it('retains endpoint handler identity, mixed evidence, exact external subpaths and absent fields', () => {
    const endpoints = graph.nodes.filter(n => n.kind === 'endpoint' && n.metadata?.path === '/users' && n.metadata.httpMethod === 'GET');
    expect(endpoints).toHaveLength(2);
    expect(new Set(endpoints.map(n => nodeDetails(graph, n.id)!.handler!.id)).size).toBe(2);
    expect(nodeDetails(graph, node('TokenRepository').id)!.node.evidence.map(e => e.confidence)).toContain('best_effort');
    expect(nodeDetails(graph, node('TokenRepository').id)!.node.evidence.map(e => e.confidence)).toContain('confirmed');
    expect(graph.nodes.filter(n => n.kind === 'external_dependency' && n.name === 'map').map(n => nodeDetails(graph, n.id)!.specifier).sort()).toEqual(['rxjs', 'rxjs/operators']);
    expect(nodeDetails(graph, graph.nodes.find(n => n.kind === 'database_model')!.id)!.fields).toEqual([]);
  });
});

it('preserves self-loop directions, multiple kinds and separately scoped file diagnostics', () => {
  const n = node('UsersService');
  const g = SystemGraphSchema.parse({ ...graph, edges: [...graph.edges, ...['imports', 'injects'].map(kind => ({
    id: canonicalId('edge', n.id, kind, n.id), from: n.id, to: n.id, kind, evidence: n.evidence,
    ...(kind === 'injects' ? { metadata: { semantics: 'requested_token' } } : {}),
  }))], diagnostics: [...graph.diagnostics, { code: 'file_notice', severity: 'warning', message: 'File notice', file: n.file }] });
  const d = nodeDetails(g, n.id)!;
  expect(d.incoming.filter(r => r.other.id === n.id).map(r => r.edge.kind)).toEqual(['imports', 'injects']);
  expect(d.outgoing.filter(r => r.other.id === n.id).map(r => r.edge.kind)).toEqual(['imports', 'injects']);
  expect(d.fileDiagnostics).toHaveLength(1);
  expect(d.diagnostics.some(d => d.code === 'file_notice')).toBe(false);
  expect(d.skipped).toBe(3);
  expect(nodeDetails(g, node('AuthModule').id)!.skipped).toBe(0);
});

import { canonicalId } from './graph';
import { renderToStaticMarkup } from 'react-dom/server';
import { createElement } from 'react';
import { NodeDetails, sourcePosition } from './NodeDetailsPanel';

it('renders escaped text, optional absence, UTC provenance and guarded database fields', () => {
  const db = graph.nodes.find(n => n.kind === 'database_model')!;
  const copy = structuredClone(graph);
  const n = copy.nodes.find(n => n.id === db.id)!;
  delete n.line; delete n.qualifiedName;
  n.metadata = { fields: [null, { name: '<script>alert(1)</script>', type: 'String' }, { name: 'x', type: {} }, 2], unused: { bad: '<img src=x>' } };
  copy.metadata.analyzedAt = '2026-09-19T01:02:03Z';
  const valid = SystemGraphSchema.parse(copy);
  const html = renderToStaticMarkup(createElement(NodeDetails, { graph: valid, details: nodeDetails(valid, n.id), expandedOwnerId: null, onNavigate: () => {}, onClose: () => {} }));
  expect(html).toContain('&lt;script&gt;alert(1)&lt;/script&gt;');
  expect(html).not.toContain('<script>');
  expect(html).not.toContain('[object Object]');
  expect(html).toContain('2026-09-19T01:02:03Z');
  expect(html).not.toContain(graph.metadata.analyzedAt);
  expect(sourcePosition({ file: 'src/a.ts' })).toBe('src/a.ts');
  expect(nodeDetails(valid, db.id)!.fields).toHaveLength(2);
  for (const fields of [null, {}, 'unknown', [null, 1, {}]]) {
    n.metadata = { fields };
    expect(nodeDetails(SystemGraphSchema.parse(copy), n.id)!.fields).toEqual([]);
  }
});

it('labels static/instance methods and rejects invalid source/evidence/endpoint input before details', () => {
  const d = nodeDetails(graph, node('UsersService').id)!;
  const methods = d.methods.filter(n => n.name === 'find');
  expect(methods).toHaveLength(2);
  expect(methods.map(n => nodeDetails(graph, n.id)!.methodKind).sort()).toEqual(['instance', 'static']);
  const html = renderToStaticMarkup(createElement(NodeDetails, { graph, details: d, expandedOwnerId: null, onNavigate: () => {}, onClose: () => {} }));
  expect(html).toContain('Show method on Canvas');
  expect(html).toContain('(static)'); expect(html).toContain('(instance)');
  for (const mutate of [
    (g: typeof graph) => { g.nodes[0].evidence = []; },
    (g: typeof graph) => { g.nodes[0].evidence[0].file = '/Users/private/source.ts'; },
    (g: typeof graph) => { g.nodes.find(n => n.kind === 'endpoint')!.metadata = { path: '/users' }; },
  ]) {
    const g = structuredClone(graph); mutate(g); expect(SystemGraphSchema.safeParse(g).success).toBe(false);
  }
});

it.each([
  ['calls', 'Static call target', 'Statically called by'],
  ['depends_on', 'Declared handler', 'Handler for endpoint'],
  ['injects', 'Requests token', 'Requested by'],
])('pairs %s labels with the exact edge direction and navigation target in each section', (kind, outgoingLabel, incomingLabel) => {
  const before = JSON.stringify(graph);
  const edges = graph.edges.filter(edge => edge.kind === kind);
  expect(edges.length).toBeGreaterThan(0);
  for (const edge of edges) {
    const moduleImport = kind === 'depends_on' && graph.nodes.find(n => n.id === edge.from)!.kind === 'module';
    for (const incoming of [false, true]) {
      const selectedId = incoming ? edge.to : edge.from;
      const otherId = incoming ? edge.from : edge.to;
      const label = moduleImport ? 'Module import' : incoming ? incomingLabel : outgoingLabel;
      const d = nodeDetails(graph, selectedId)!;
      const row = (incoming ? d.incoming : d.outgoing).find(row => row.edge.id === edge.id)!;
      expect(row).toBeDefined();
      expect({ label: row.label, otherId: row.other.id, edge: row.edge }).toEqual({
        label, otherId, edge: { ...edge, evidence: sortedEvidence(edge.evidence) },
      });
      const html = renderToStaticMarkup(createElement(NodeDetails, {
        graph, details: d, expandedOwnerId: null, onNavigate: () => {}, onClose: () => {},
      }));
      const section = html.split(`<section><h3>${incoming ? 'Incoming' : 'Outgoing'} relationships</h3>`)[1]?.split('</section>')[0];
      expect(section).toBeDefined();
      // Restrict the label/button assertion to the row containing this canonical edge ID.
      const renderedRow = section!.split('<li><strong>').slice(1).find(part => part.includes(`<p>${edge.id}</p>`));
      expect(renderedRow).toBeDefined();
      expect(renderedRow!.split('</button>')[0]).toContain(`${label}</strong><button`);
      expect(renderedRow!.split('</button>')[0]).toContain(`<small>${otherId}</small>`);
    }
  }
  expect(JSON.stringify(graph)).toBe(before);
});
