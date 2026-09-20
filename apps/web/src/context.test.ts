import { expect, it } from 'vitest';
import fixture from '../../../examples/nestjs-sample/expected-graph.json';
import { SystemGraphSchema } from './graph';
import { collectContext } from './context';
import { generateContextMarkdown } from './contextMarkdown';
const graph = SystemGraphSchema.parse(fixture);
const users = graph.nodes.find(n => n.name === 'UsersService')!;
it('collects only the dedicated token paths and preserves identities without mutation', () => {
  const before = JSON.stringify(graph);
  const c = collectContext(graph, users.id);
  const paths = c.sections.find(s => s.key === 'tokenEndpoints')!.items;
  const expected = graph.edges.filter(e => e.kind === 'injects' && e.to === users.id)
    .flatMap(di => graph.nodes.find(n => n.id === di.from)!.kind === 'controller'
      ? graph.edges.filter(e => e.kind === 'exposes' && e.from === di.from).map(e => [di.id, e.id]) : []);
  expect(paths.map(p => p.edges.map(e => e.id)).sort()).toEqual(expected.sort());
  expect(paths.length).toBeGreaterThan(0);
  expect(c.sections.find(s => s.key === 'handlerCalls')!.items).toEqual([]);
  expect(JSON.stringify(graph)).toBe(before);
});
it('renders a deterministic bounded snapshot and rejects absent or invalid selection/input', () => {
  const before = JSON.stringify(graph);
  const reordered = structuredClone(graph);
  reordered.nodes.reverse(); reordered.edges.reverse(); reordered.diagnostics.reverse();
  for (const n of reordered.nodes) n.evidence.reverse();
  for (const e of reordered.edges) e.evidence.reverse();
  const text = generateContextMarkdown(graph, users.id).markdown;
  expect(generateContextMarkdown(reordered, users.id).markdown).toBe(text);
  expect(text).toContain(graph.metadata.analyzedAt);
  expect(text).toContain('Declared on controllers requesting this token');
  expect(text).toContain('best_effort');
  expect(Array.from(text).length).toBeLessThanOrEqual(32000);
  expect(JSON.stringify(graph)).toBe(before);
  expect(() => generateContextMarkdown(graph, null)).toThrow();
  expect(() => generateContextMarkdown(graph, 'missing')).toThrow();
  expect(() => generateContextMarkdown({}, users.id)).toThrow();
});

import { canonicalId, type GraphNode, type GraphEdge, type SystemGraph } from './graph';
import { additionalContextNodes } from './context';
import { codePoints } from './contextMarkdown';
import { nodeDetails } from './nodeDetails';
import { initialCanvasState, transitionCanvasState } from './canvasState';
import { projectView } from './canvasView';
const evidence = [{ source: 'ast' as const, confidence: 'confirmed' as const, file: 'a.ts', line: 1 }];
const declaration = (name: string, kind: GraphNode['kind'] = 'class'): GraphNode => ({
  id: canonicalId('class', 'a.ts', name), kind, name, file: 'a.ts', evidence: structuredClone(evidence),
});
const method = (owner: GraphNode, name: string): GraphNode => ({
  id: canonicalId('method', owner.id, 'instance', name), kind: 'method', name, parentId: owner.id, file: 'a.ts', evidence: structuredClone(evidence),
});
const edge = (a: GraphNode, kind: GraphEdge['kind'], b: GraphNode): GraphEdge => ({
  id: canonicalId('edge', a.id, kind, b.id), from: a.id, to: b.id, kind, evidence: structuredClone(evidence),
  ...(kind === 'injects' ? { metadata: { semantics: 'requested_token' } } : {}),
});
const emptyGraph = (): SystemGraph => ({ schemaVersion: '0.1', metadata: { analyzerVersion: 'test', analyzedAt: '2026-09-20T01:02:03Z', callAnalysis: { scope: 'parsed_named_class_methods', mode: 'same_class_only', examinedCalls: 0, emittedCalls: 0, skippedCalls: 0 } }, nodes: [], edges: [], diagnostics: [] });
function endpoint(owner: GraphNode, handler: GraphNode): GraphNode {
  return { id: canonicalId('endpoint', 'GET', '/same', handler.id), kind: 'endpoint', name: 'GET /same', parentId: owner.id, evidence: structuredClone(evidence), metadata: { httpMethod: 'GET', path: '/same', controllerMethodId: handler.id } };
}
function independentPaths() {
  const g = emptyGraph(); const controller = declaration('C', 'controller'), token = declaration('T', 'service'), extra = declaration('Extra');
  const handler = method(controller, 'handle'), callee = method(controller, 'called'), third = method(controller, 'third'), tokenMethod = method(token, 'notControllerCall');
  const route = endpoint(controller, handler);
  g.nodes = [controller, token, extra, handler, callee, third, tokenMethod, route];
  g.edges = [edge(controller, 'contains', handler), edge(controller, 'contains', callee), edge(controller, 'contains', third), edge(token, 'contains', tokenMethod),
    edge(controller, 'injects', token), edge(token, 'injects', extra), edge(controller, 'exposes', route), edge(route, 'depends_on', handler),
    edge(handler, 'calls', callee), edge(callee, 'calls', third), edge(third, 'calls', handler), edge(tokenMethod, 'calls', tokenMethod)];
  g.metadata.callAnalysis.examinedCalls = g.metadata.callAnalysis.emittedCalls = 4;
  return { g: SystemGraphSchema.parse(g), controller, token, extra, handler, callee, third, route, tokenMethod };
}
it('retains all original direct edges/directions for every fixture node and keeps methods lexical', () => {
  for (const n of graph.nodes) {
    const c = collectContext(graph, n.id);
    expect(c.sections[0].items.flatMap(i => i.edges.map(e => e.id)).sort()).toEqual(graph.edges.filter(e => e.from === n.id).map(e => e.id).sort());
    expect(c.sections[1].items.flatMap(i => i.edges.map(e => e.id)).sort()).toEqual(graph.edges.filter(e => e.to === n.id).map(e => e.id).sort());
    expect(c.sections[2].items.map(i => i.nodes[0].id).sort()).toEqual(nodeDetails(graph, n.id)!.methods.map(m => m.id).sort());
    const output = generateContextMarkdown(graph, n.id);
    expect(output.markdown).toContain('## Component');
    expect(output.markdown).toContain('## Analysis scope / unknown');
  }
  const c = collectContext(graph, users.id);
  expect(c.sections[1].items.filter(i => i.edges[0].kind === 'contains')).toHaveLength(2);
  expect(c.sections[2].items.filter(i => i.nodes[0].name === 'find')).toHaveLength(2);
  const routes = c.sections.find(s => s.key === 'tokenEndpoints')!.items.filter(i => i.nodes[0].metadata?.path === '/users');
  expect(new Set(routes.map(i => i.nodes[0].id)).size).toBeGreaterThanOrEqual(2);
});
it('uses successful independent endpoint paths, never token implementation calls or a third hop/cycle', () => {
  const { g, route, handler, callee, third, token, controller, tokenMethod, extra } = independentPaths();
  const c = collectContext(g, route.id);
  expect(c.sections.find(s => s.key === 'handlerCalls')!.items.map(i => ({ nodes: i.nodes.map(n => n.id), edges: i.edges.map(e => e.id) }))).toEqual([
    { nodes: [callee.id, handler.id], edges: [canonicalId('edge', route.id, 'depends_on', handler.id), canonicalId('edge', handler.id, 'calls', callee.id)] },
  ]);
  expect(c.sections.find(s => s.key === 'controllerTokens')!.items.map(i => i.nodes.map(n => n.id))).toEqual([[token.id, controller.id]]);
  const ids = new Set(c.sections.flatMap(s => s.items.flatMap(i => i.nodes.map(n => n.id))));
  for (const forbidden of [third, tokenMethod, extra]) expect(ids.has(forbidden.id)).toBe(false);
  const service = collectContext(g, token.id);
  expect(service.sections.find(s => s.key === 'handlerCalls')!.items).toEqual([]);
  expect(service.sections.find(s => s.key === 'controllerTokens')!.items).toEqual([]);
  // The real fixture has injected-receiver unknowns, not the independent success path above.
  for (const n of graph.nodes.filter(n => n.kind === 'endpoint')) expect(collectContext(graph, n.id).sections.find(s => s.key === 'handlerCalls')!.items).toEqual([]);
});
it('keeps external subpaths, equal names, memberships and missing optional fields distinct', () => {
  const external = graph.nodes.filter(n => n.kind === 'external_dependency' && n.name === 'map');
  expect(external).toHaveLength(2);
  expect(new Set(external.map(n => generateContextMarkdown(graph, n.id).markdown)).size).toBe(2);
  const copy = structuredClone(graph); copy.metadata.rootName = 'DO_NOT_EXPORT_ROOT';
  const db = copy.nodes.find(n => n.kind === 'database_model')!;
  delete db.line; delete db.qualifiedName; db.metadata = { secret: 'DO_NOT_EXPORT_METADATA' };
  const text = generateContextMarkdown(copy, db.id).markdown;
  expect(text).not.toContain('DO_NOT_EXPORT');
  expect(text).not.toContain('undefined');
  expect(text).not.toContain('## Outgoing relationships');
  expect(text).toContain('Zero calls/diagnostics never guarantees complete analysis');
});
it.each([49, 50, 51])('applies fixed section limit to %i actual candidates and evidence/diagnostics', count => {
  const g = emptyGraph(); const selected = declaration('S');
  g.nodes = [selected, ...Array.from({ length: count }, (_, i) => declaration(String(i).padStart(2, '0')))];
  g.edges = g.nodes.slice(1).map(n => edge(selected, 'imports', n));
  g.diagnostics = Array.from({ length: count }, () => ({ code: 'normal', severity: 'warning' as const, message: 'duplicate', relatedNodeId: selected.id }));
  selected.evidence = Array.from({ length: count }, () => ({ ...evidence[0] }));
  const c = collectContext(SystemGraphSchema.parse(g), selected.id);
  expect(c.sections[0].items).toHaveLength(count);
  const output = generateContextMarkdown(g, selected.id);
  const outgoing = output.omissions.find(o => o.section === 'Outgoing relationships')!;
  expect(outgoing.items).toBe(Math.max(count - 50, 0));
  expect(outgoing.included).toBe(Math.min(count, 50));
  expect(output.omissions.find(o => o.section.startsWith('Relevant diagnostics'))!.items).toBe(Math.max(count - 50, 0));
  expect(output.omissions.find(o => o.section === 'Node declaration evidence')!.candidates).toBeGreaterThanOrEqual(count);
  for (const o of output.omissions) expect(o.included + o.items + o.nodes + o.characters).toBe(o.candidates);
});
it.each([199, 200, 201])('checks the canonical distinct-node admission boundary with %i valid candidate IDs', count => {
  const g = emptyGraph(); const s = declaration('S');
  g.nodes = [s, ...Array.from({ length: count }, (_, i) => declaration(`N${i}`))];
  g.edges = g.nodes.slice(1).map(n => edge(s, 'imports', n));
  const candidates = collectContext(SystemGraphSchema.parse(g), s.id).sections[0].items;
  expect(candidates).toHaveLength(count);
  const admitted = new Set([s.id]);
  for (const item of candidates) {
    const added = additionalContextNodes(admitted, item.nodes.map(n => n.id));
    if (added) for (const id of added) admitted.add(id);
  }
  expect(admitted.size - 1).toBe(Math.min(count, 200));
  expect(additionalContextNodes(admitted, [s.id, candidates[0].nodes[0].id])).toEqual([]);
});
it('counts a two-hop intermediate atomically and does not leave rejected candidate IDs in the budget', () => {
  const { g, route } = independentPaths();
  const path = collectContext(g, route.id).sections.find(s => s.key === 'controllerTokens')!.items[0];
  expect(path.nodes).toHaveLength(2);
  const ids = path.nodes.map(n => n.id);
  const admitted = new Set([route.id, ...Array.from({ length: 199 }, (_, i) => `previous${i}`)]);
  expect(additionalContextNodes(admitted, ids)).toBeUndefined();
  expect(ids.some(id => admitted.has(id))).toBe(false);
  admitted.delete('previous198');
  expect(additionalContextNodes(admitted, [...ids, ...ids])).toEqual(ids);
  // A rejected character-budget attempt likewise never commits this tentative result.
  expect(admitted.size).toBe(199);
});
it('retains unknown totals when methods and diagnostic rows are omitted, including skippedCount > 1', () => {
  const g = emptyGraph(); const s = declaration('S');
  const methods = Array.from({ length: 51 }, (_, i) => method(s, `m${String(i).padStart(2, '0')}`));
  g.nodes = [s, ...methods]; g.edges = methods.map(m => edge(s, 'contains', m));
  g.diagnostics = methods.map(m => ({ code: 'FUTURE_REASON', severity: 'warning' as const, message: 'Unknown', file: 'a.ts', line: 1, relatedNodeId: m.id, skippedCount: 3 }));
  g.diagnostics.push({ code: 'ERROR', severity: 'error', message: 'Other failure', file: 'a.ts' } as typeof g.diagnostics[number]);
  g.metadata.callAnalysis.examinedCalls = g.metadata.callAnalysis.skippedCalls = 153;
  const out = generateContextMarkdown(g, s.id);
  expect(out.markdown).toContain('Graph may be incomplete');
  expect(out.markdown).toContain('skipped=153');
  expect(out.markdown).toMatch(/unlisted methods=[1-9]/);
  expect(out.markdown).toMatch(/Known diagnostics on unlisted candidate methods=[1-9]/);
  expect(out.markdown).toContain('not part of included-method totals');
  expect(out.markdown).toContain('not node-attributed, not added to call counts');
  expect(out.omissions.some(o => o.items > 0 || o.characters > 0)).toBe(true);
});
it('counts the fully escaped Unicode output, keeps required notices, and never emits graph data as Markdown syntax', () => {
  const g = emptyGraph();
  const hostile = '日本語😀e\u0301\n```\n# heading <img src=x> [link](file:///private) ';
  const s = declaration(hostile.repeat(20));
  s.qualifiedName = hostile.repeat(20);
  g.nodes = [s, ...Array.from({ length: 65 }, (_, i) => declaration(`${i}${hostile.repeat(20)}`))];
  g.edges = g.nodes.slice(1).map(n => edge(s, 'imports', n));
  for (const n of g.nodes) n.evidence = Array.from({ length: 55 }, (_, i) => ({ ...evidence[0], file: 'long😀'.repeat(100) + '.ts', confidence: i % 2 ? 'confirmed' : 'best_effort' }));
  g.diagnostics = Array.from({ length: 65 }, (_, i) => ({ code: `C${i}`, severity: 'error' as const, message: hostile.repeat(100), relatedNodeId: s.id }));
  const before = JSON.stringify(g);
  const output = generateContextMarkdown(g, s.id);
  expect(output.shortenedFields).toBeGreaterThan(0);
  expect(output.omissions.some(o => o.characters > 0)).toBe(true);
  expect(output.omissions.some(o => o.items > 0)).toBe(true);
  expect(codePoints(output.markdown)).toBeLessThanOrEqual(32000);
  expect(codePoints(output.markdown)).toBeGreaterThan(15000);
  expect(output.markdown.length).toBeGreaterThan(codePoints(output.markdown));
  expect(output.markdown).toContain('Graph may be incomplete');
  expect(output.markdown).toContain('best_effort');
  expect(output.markdown).toContain('## Truncation');
  expect(output.markdown).not.toContain('\n# heading');
  expect(output.markdown).not.toContain('```');
  expect(output.markdown).not.toContain('<img');
  expect(output.markdown).not.toContain('[link](');
  expect(output.markdown).toContain('[field truncated; prefix only]');
  expect(Array.from(output.markdown).some(c => c.length === 1 && /[\uD800-\uDFFF]/u.test(c))).toBe(false);
  expect(JSON.stringify(g)).toBe(before);
});
it('is invariant to metadata/evidence/diagnostic ordering and all Canvas presentation state', () => {
  const reordered = structuredClone(graph);
  for (const n of reordered.nodes) {
    n.evidence.reverse(); if (n.metadata) n.metadata = Object.fromEntries(Object.entries(n.metadata).reverse());
  }
  for (const e of reordered.edges) { e.evidence.reverse(); if (e.metadata) e.metadata = Object.fromEntries(Object.entries(e.metadata).reverse()); }
  reordered.nodes.reverse(); reordered.edges.reverse(); reordered.diagnostics.reverse();
  for (const n of graph.nodes) expect(generateContextMarkdown(reordered, n.id).markdown).toBe(generateContextMarkdown(graph, n.id).markdown);
  const before = generateContextMarkdown(graph, users.id).markdown;
  let state = initialCanvasState(3);
  state = transitionCanvasState(graph, state, { type: 'select', id: users.id });
  state = transitionCanvasState(graph, state, { type: 'show', id: users.id });
  projectView(graph, new Set(['service']), state.expandedOwnerId);
  expect(generateContextMarkdown(graph, users.id).markdown).toBe(before);
});
it('keeps the existing invalid-path/ref import boundary at export as well', () => {
  for (const change of [
    (g: SystemGraph) => { g.nodes[0].file = '/private/a.ts'; },
    (g: SystemGraph) => { g.edges[0].to = 'missing'; },
    (g: SystemGraph) => { g.diagnostics.push({ code: 'bad', severity: 'error', message: 'bad', line: 1 }); },
  ]) {
    const g = structuredClone(graph); change(g);
    expect(SystemGraphSchema.safeParse(g).success).toBe(false);
    expect(() => generateContextMarkdown(g, users.id)).toThrow();
  }
});
