import cytoscape from 'cytoscape';
import { expect, it } from 'vitest';
import expected from '../../../examples/nestjs-sample/expected-graph.json';
import { SystemGraphSchema } from './graph';
import { initialCanvasState, transitionCanvasState } from './canvasState';
import { defaultKinds, projectView, searchNodes, searchIndex, directNeighborhood } from './canvasView';
import { graphToCytoscapeElements } from './graphCanvasAdapter';
import { updateCanvasElements } from './canvasLayout';

const graph = SystemGraphSchema.parse(expected);
const id = (name: string) => graph.nodes.find(n => n.name === name)!.id;

it('searches the full immutable snapshot literally, stably and with a 50-result limit', () => {
  const before = structuredClone(graph);
  const index = searchIndex(graph);
  expect(searchNodes(index, '  aUtHsErViCe ').nodes.map(n => n.id)).toContain(id('AuthService'));
  for (const node of graph.nodes) {
    expect(searchNodes(index, node.name).total).toBeGreaterThan(0);
    if (node.qualifiedName) expect(searchNodes(index, node.qualifiedName).nodes.map(n => n.id)).toContain(node.id);
    if (node.kind === 'endpoint') expect(searchNodes(index, String(node.metadata!.path)).nodes.map(n => n.id)).toContain(node.id);
  }
  expect(searchNodes(index, 'Same').nodes.filter(n => n.name === 'Same')).toHaveLength(2);
  expect(searchNodes(index, '.*').total).toBe(0);
  expect(searchNodes(index, '  ')).toMatchObject({ total: 0, nodes: [], empty: true });
  const repeated = searchIndex(SystemGraphSchema.parse({ ...graph, nodes: [...graph.nodes].reverse(), edges: [...graph.edges].reverse() }));
  expect(searchNodes(repeated, 'a')).toEqual(searchNodes(index, 'a'));
  expect(graph).toEqual(before);
});

it('projects Services with canonical ancestor frames and independently expected edge tuples', () => {
  const view = projectView(graph, new Set(['service']), null);
  const services = graph.nodes.filter(n => n.kind === 'service');
  const expectedIds = new Set(services.map(n => n.id));
  for (const n of services) if (n.parentId) expectedIds.add(n.parentId);
  expect(view.visibleIds).toEqual(expectedIds);
  expect(view.matchedIds).toEqual(new Set(services.map(n => n.id)));
  const elements = graphToCytoscapeElements(graph, null, view.visibleIds);
  expect(elements.filter(e => e.group === 'edges').map(e => [e.data.source, e.data.kind, e.data.target])).toEqual(graph.edges.filter(e => expectedIds.has(e.from) && expectedIds.has(e.to)).map(e => [e.from, e.kind, e.to]));
  expect(elements.find(e => e.data.id === id('UsersService'))!.data.parent).toBeUndefined();
  expect(projectView(graph, new Set(), null).visibleIds.size).toBe(0);
  expect(projectView(graph, defaultKinds(), null).visibleIds.size).toBe(40);
});

it('reveals only the target kind, preserves query, fixes filtered selection and rejects old navigation', () => {
  let state = initialCanvasState(3);
  state = transitionCanvasState(graph, state, { type: 'query', query: 'Auth' });
  state = transitionCanvasState(graph, state, { type: 'kinds', kinds: new Set(['service']) });
  const endpoint = graph.nodes.find(n => n.kind === 'endpoint')!;
  state = transitionCanvasState(graph, state, { type: 'navigate', id: endpoint.id, generation: 3 });
  expect(state.enabledKinds).toEqual(new Set(['service', 'endpoint']));
  expect(state.query).toBe('Auth');
  state = transitionCanvasState(graph, state, { type: 'neighbors', generation: 3 });
  expect(state.neighborhoodAnchorId).toBe(endpoint.id);
  const selected = state;
  state = transitionCanvasState(graph, state, { type: 'query', query: 'next' });
  expect(state.focusRequest).toBe(selected.focusRequest);
  state = transitionCanvasState(graph, state, { type: 'kinds', kinds: new Set(['service']) });
  expect(state.selectedNodeId).toBeNull();
  expect(state.focusRequest).toBeNull();
  expect(state.neighborhoodAnchorId).toBeNull();
  const reset = transitionCanvasState(graph, state, { type: 'reset', generation: 4 });
  expect(reset).toEqual(initialCanvasState(4));
  expect(transitionCanvasState(graph, reset, { type: 'navigate', id: endpoint.id, generation: 3 })).toBe(reset);
  expect(transitionCanvasState(graph, reset, { type: 'neighbors', generation: 3 })).toBe(reset);
});

it('retains expansion when its kind is off and clears hidden method anchor on Hide', () => {
  const method = graph.nodes.find(n => n.kind === 'method')!;
  let state = transitionCanvasState(graph, initialCanvasState(1), { type: 'navigate', id: method.id, generation: 1 });
  state = transitionCanvasState(graph, state, { type: 'neighbors', generation: 1 });
  const clear = transitionCanvasState(graph, state, { type: 'clear-neighbors' });
  expect(clear).toEqual({ ...state, neighborhoodAnchorId: null });
  const hidden = transitionCanvasState(graph, state, { type: 'hide' });
  expect(hidden.neighborhoodAnchorId).toBeNull();
  const off = transitionCanvasState(graph, state, { type: 'kinds', kinds: new Set(['endpoint']) });
  expect(off.expandedOwnerId).toBe(method.parentId);
  expect(projectView(graph, off.enabledKinds, off.expandedOwnerId).visibleIds.has(method.id)).toBe(false);
  const on = transitionCanvasState(graph, off, { type: 'kinds', kinds: defaultKinds() });
  expect(projectView(graph, on.enabledKinds, on.expandedOwnerId).visibleIds.has(method.id)).toBe(true);
});

it('depth one uses incident visible edges only, with ancestors as context, without a second traversal', () => {
  const view = projectView(graph, defaultKinds(), null);
  const anchor = id('AuthService');
  const focus = directNeighborhood(graph, view.visibleIds, anchor)!;
  const edges = graph.edges.filter(e => (e.from === anchor || e.to === anchor) && view.visibleIds.has(e.from) && view.visibleIds.has(e.to));
  expect(focus.edgeIds).toEqual(new Set(edges.map(e => e.id)));
  expect(focus.nodeIds).toEqual(new Set([anchor, ...edges.flatMap(e => [e.from, e.to])]));
  expect([...focus.nodeIds].every(n => graph.nodes.find(x => x.id === n)!.kind !== 'method')).toBe(true);
  expect(directNeighborhood(graph, new Set(), anchor)).toBeNull();
});

it('real Cytoscape retains surviving children and canonical edges when filters change', () => {
  const cy = cytoscape({ headless: true, elements: graphToCytoscapeElements(graph) });
  const endpoints = projectView(graph, new Set(['endpoint']), null);
  const endpoint = graph.nodes.find(n => n.kind === 'endpoint')!;
  const retained = cy.getElementById(endpoint.id)[0];
  updateCanvasElements(cy, graphToCytoscapeElements(graph, null, endpoints.visibleIds));
  expect(new Set(cy.nodes().map(n => n.id()))).toEqual(endpoints.visibleIds);
  expect(cy.getElementById(endpoint.id)[0]).toBe(retained);
  expect(cy.getElementById(endpoint.id).parent()[0].id()).toBe(endpoint.parentId);
  updateCanvasElements(cy, []);
  expect(cy.elements()).toHaveLength(0);
  updateCanvasElements(cy, graphToCytoscapeElements(graph));
  expect(cy.nodes()).toHaveLength(40); expect(cy.edges()).toHaveLength(56);
  cy.destroy();
});

import { canonicalId, type GraphEdge, type GraphNode } from './graph';
import { applyViewStyle, focusStyle } from './canvasFocus';
import { CanvasControls } from './CanvasControls';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { candidateDescription, navigationLabel } from './canvasView';

function independentGraph() {
  const evidence = [{ source: 'ast' as const, confidence: 'confirmed' as const, file: 'src/example.ts' }];
  const nodes: GraphNode[] = ['Frame', 'OtherFrame', 'Anchor', 'Neighbor', 'DepthTwo', 'Sibling'].map(name => ({
    id: canonicalId('class', 'src/example.ts', name), name, kind: name.endsWith('Frame') ? 'module' : 'service', file: 'src/example.ts', evidence,
  }));
  const [frame, otherFrame, anchor, neighbor, depthTwo, sibling] = nodes;
  anchor.parentId = frame.id; neighbor.parentId = otherFrame.id; sibling.parentId = otherFrame.id;
  const edge = (from: GraphNode, kind: GraphEdge['kind'], to: GraphNode): GraphEdge => ({
    id: canonicalId('edge', from.id, kind, to.id), from: from.id, to: to.id, kind, evidence,
    ...(kind === 'injects' ? { metadata: { semantics: 'requested_token' } } : {}),
  });
  const edges = [edge(frame, 'contains', anchor), edge(otherFrame, 'contains', neighbor), edge(otherFrame, 'contains', sibling),
    edge(anchor, 'imports', anchor), edge(anchor, 'imports', neighbor), edge(neighbor, 'injects', anchor),
    edge(neighbor, 'imports', depthTwo)];
  return SystemGraphSchema.parse({ schemaVersion: '0.1', metadata: { analyzerVersion: 'independent-test', analyzedAt: '2026-09-19T00:00:00Z', callAnalysis: { scope: 'parsed_named_class_methods', mode: 'same_class_only', examinedCalls: 0, emittedCalls: 0, skippedCalls: 0 } }, nodes, edges, diagnostics: [] });
}

it('does not traverse context frames, depth two, or duplicate self/parallel-edge neighbors', () => {
  const g = independentGraph();
  const [frame, otherFrame, anchor, neighbor] = g.nodes;
  const view = projectView(g, new Set(['service']), null);
  const focus = directNeighborhood(g, view.visibleIds, anchor.id)!;
  expect(focus.nodeIds).toEqual(new Set([anchor.id, frame.id, neighbor.id]));
  expect(focus.contextIds).toEqual(new Set([otherFrame.id]));
  expect(focus.edgeIds).toEqual(new Set([g.edges[0].id, g.edges[3].id, g.edges[4].id, g.edges[5].id]));
});

it('applies and clears dim without changing elements, viewport, selection labels, or child opacity', () => {
  const g = independentGraph();
  const view = projectView(g, new Set(['service']), null);
  const cy = cytoscape({ headless: true, styleEnabled: true, elements: graphToCytoscapeElements(g, null, view.visibleIds), style: [
    { selector: 'node', style: { opacity: 1 } }, ...focusStyle,
  ] });
  const [, otherFrame, anchor, neighbor, depthTwo, sibling] = g.nodes;
  cy.zoom(1.7); cy.pan({ x: 24, y: 52 });
  const ids = cy.elements().map(e => e.id());
  const positions = cy.nodes().map(n => ({ ...n.position() }));
  cy.getElementById(anchor.id).select();
  cy.getElementById(g.edges[4].id).addClass('inspected');
  let layouts = 0; cy.on('layoutstart', () => { layouts++; });
  applyViewStyle(cy, view, directNeighborhood(g, view.visibleIds, anchor.id));
  expect(cy.getElementById(otherFrame.id).hasClass('focus-context')).toBe(true);
  expect(cy.getElementById(neighbor.id).effectiveOpacity()).toBe(1);
  expect(cy.getElementById(sibling.id).hasClass('focus-dim')).toBe(true);
  expect(cy.getElementById(depthTwo.id).hasClass('focus-dim')).toBe(true);
  applyViewStyle(cy, view, null);
  expect(cy.elements('.focus-dim, .focus-context, .focus-neighbor')).toHaveLength(0);
  expect(cy.getElementById(anchor.id).selected()).toBe(true);
  expect(cy.getElementById(g.edges[4].id).hasClass('inspected')).toBe(true);
  expect(cy.elements().map(e => e.id())).toEqual(ids);
  expect(cy.nodes().map(n => n.position())).toEqual(positions);
  expect(cy.zoom()).toBe(1.7); expect(cy.pan()).toEqual({ x: 24, y: 52 }); expect(layouts).toBe(0);
  cy.destroy();
});

it('all accepted edge kinds participate in both directions, without showing hidden method edges', () => {
  const view = projectView(graph, defaultKinds(), null);
  const observed = new Set<string>();
  for (const owner of graph.nodes.filter(n => ['controller', 'service', 'class'].includes(n.kind))) {
    const expanded = projectView(graph, defaultKinds(), owner.id);
    for (const edge of graph.edges.filter(e => expanded.visibleIds.has(e.from) && expanded.visibleIds.has(e.to))) {
      observed.add(edge.kind);
      for (const anchor of [edge.from, edge.to]) expect(directNeighborhood(graph, expanded.visibleIds, anchor)!.edgeIds.has(edge.id)).toBe(true);
    }
  }
  expect(observed).toEqual(new Set(['contains', 'imports', 'injects', 'calls', 'exposes', 'depends_on']));
  expect([...directNeighborhood(graph, view.visibleIds, id('UsersService'))!.nodeIds].some(id => graph.nodes.find(n => n.id === id)?.kind === 'method')).toBe(false);
});

it('search distinguishes static/instance, equal routes and exact external subpaths with guarded metadata', () => {
  const before = structuredClone(graph);
  const nodes = new Map(graph.nodes.map(n => [n.id, n]));
  const methods = searchNodes(searchIndex(graph), 'find').nodes.filter(n => n.kind === 'method' && n.parentId === id('UsersService') && n.name === 'find');
  expect(methods).toHaveLength(2);
  expect(methods.map(n => candidateDescription(n, nodes)).some(text => text.includes('static'))).toBe(true);
  expect(methods.map(n => candidateDescription(n, nodes)).some(text => text.includes('instance'))).toBe(true);
  const endpoints = graph.nodes.filter(n => n.kind === 'endpoint');
  const path = endpoints.find(n => endpoints.filter(other => other.metadata!.path === n.metadata!.path).length > 1)!.metadata!.path as string;
  const matches = searchNodes(searchIndex(graph), path).nodes.filter(n => n.kind === 'endpoint');
  expect(new Set(matches.map(n => n.id)).size).toBe(matches.length);
  expect(matches.every(n => candidateDescription(n, nodes).includes('handler:'))).toBe(true);
  const g = independentGraph();
  const external: GraphNode = { id: canonicalId('external', '@example/pkg/subpath', 'Symbol'), kind: 'external_dependency', name: 'Symbol', evidence: g.nodes[0].evidence,
    metadata: { package: 'ExplicitPackage', symbol: 'ExplicitSymbol', unrelated: 'DoNotSearch', other: { text: 'DoNotSerialize' } } };
  g.nodes.push(external);
  const valid = SystemGraphSchema.parse(g);
  for (const query of ['@EXAMPLE/PKG/SUBPATH', 'ExplicitPackage', 'ExplicitSymbol']) expect(searchNodes(searchIndex(valid), query).nodes).toEqual([external]);
  for (const query of ['DoNotSearch', 'DoNotSerialize']) expect(searchNodes(searchIndex(valid), query).total).toBe(0);
  external.metadata = { package: ['DoNotSearch'], symbol: { text: 'DoNotSerialize' } };
  expect(searchNodes(searchIndex(SystemGraphSchema.parse(g)), 'DoNot')).toMatchObject({ total: 0 });
  expect(graph).toEqual(before);
});

it('renders bounded candidates, explicit empty states and reveal wording with text escaping', () => {
  const g = independentGraph();
  g.nodes = Array.from({ length: 60 }, (_, i) => ({ ...g.nodes[4], id: canonicalId('class', 'src/example.ts', `match${i}`), name: `match${i}`, qualifiedName: '<img src=x>' }));
  g.edges = [];
  const valid = SystemGraphSchema.parse(g);
  const state = { ...initialCanvasState(1), query: 'match', enabledKinds: new Set<never>() };
  const view = projectView(valid, state.enabledKinds, null);
  const html = renderToStaticMarkup(createElement(CanvasControls, { graph: valid, state, view, neighborhood: null, onChange: () => {} }));
  expect(html).toContain('60 matches'); expect(html).toContain('showing 50 (limit 50)'); expect(html).toContain('Refine your search');
  expect(html).toContain('Reveal &amp; go'); expect(html).toContain('&lt;img src=x&gt;'); expect(html).not.toContain('<img');
  expect(html).not.toContain('combobox');
  expect(navigationLabel(graph.nodes.find(n => n.kind === 'method')!, view.visibleIds)).toContain('expand method');
  expect(searchNodes(searchIndex(valid), 'missing')).toMatchObject({ total: 0, empty: false });
});

it('keeps a pinned anchor through ordinary selection and recomputes expansion without changing the snapshot', () => {
  const before = structuredClone(graph);
  let state = initialCanvasState(7);
  state = transitionCanvasState(graph, state, { type: 'kinds', kinds: new Set(['service']) });
  state = transitionCanvasState(graph, state, { type: 'query', query: 'AuthService' });
  state = transitionCanvasState(graph, state, { type: 'navigate', id: id('AuthService'), generation: 7 });
  state = transitionCanvasState(graph, state, { type: 'neighbors', generation: 7 });
  state = transitionCanvasState(graph, state, { type: 'select', id: id('UsersService') });
  expect(state.neighborhoodAnchorId).toBe(id('AuthService'));
  state = transitionCanvasState(graph, state, { type: 'show', id: id('AuthService') });
  const view = projectView(graph, state.enabledKinds, state.expandedOwnerId);
  const focus = directNeighborhood(graph, view.visibleIds, state.neighborhoodAnchorId)!;
  const methods = graph.nodes.filter(n => n.kind === 'method' && n.parentId === id('AuthService'));
  expect(methods.every(n => focus.nodeIds.has(n.id))).toBe(true);
  const clear = transitionCanvasState(graph, state, { type: 'clear-neighbors' });
  expect(clear).toEqual({ ...state, neighborhoodAnchorId: null });
  expect(graph).toEqual(before);
});
