import { expect, it } from 'vitest';
import expected from '../../../examples/nestjs-sample/expected-graph.json';
import { canonicalId, SystemGraphSchema } from './graph';
import { directMethods, initialCanvasState, transitionCanvasState } from './canvasState';
import { graphToCytoscapeElements } from './graphCanvasAdapter';

const graph = SystemGraphSchema.parse(expected);
const owners = graph.nodes.filter(n => n.kind === 'controller');
const a = owners[0].id;
const b = owners[1].id;

it('expands only the explicitly requested owner and retains all canonical visible edges', () => {
  const before = structuredClone(graph);
  for (const owner of graph.nodes.filter(n => directMethods(graph, n.id).length)) {
    const methods = directMethods(graph, owner.id);
    const elements = graphToCytoscapeElements(graph, owner.id);
    const ids = new Set([...graph.nodes.filter(n => n.kind !== 'method'), ...methods].map(n => n.id));
    expect(elements.filter(e => e.group === 'nodes').map(e => e.data.id)).toEqual(graph.nodes.filter(n => ids.has(n.id)).map(n => n.id));
    expect(elements.filter(e => e.group === 'edges').map(e => [e.data.id, e.data.source, e.data.kind, e.data.target])).toEqual(graph.edges.filter(e => ids.has(e.from) && ids.has(e.to)).map(e => [e.id, e.from, e.kind, e.to]));
    expect(elements.filter(e => e.data.kind === 'method').map(e => e.data.parent)).toEqual(methods.map(() => owner.id));
  }
  expect(graph).toEqual(before);
});

it('uses parent and contains identity, not equal method or class names', () => {
  const sameClasses = graph.nodes.filter(n => n.kind === 'class' && n.name === 'Same');
  expect(sameClasses).toHaveLength(2);
  const methods = sameClasses.map(owner => ({
    id: canonicalId('method', owner.id, 'instance', 'sameMethod'), kind: 'method' as const,
    name: 'sameMethod', parentId: owner.id, file: owner.file, line: owner.line, evidence: owner.evidence,
  }));
  const sameNames = SystemGraphSchema.parse({ ...graph, nodes: [...graph.nodes, ...methods], edges: [...graph.edges, ...methods.map(method => ({
    id: canonicalId('edge', method.parentId, 'contains', method.id), kind: 'contains', from: method.parentId, to: method.id, evidence: method.evidence,
  }))] });
  for (const owner of sameClasses) {
    expect(directMethods(sameNames, owner.id).map(n => n.id)).toEqual([canonicalId('method', owner.id, 'instance', 'sameMethod')]);
  }
  const broken = { ...graph, edges: graph.edges.filter(e => !(e.kind === 'contains' && e.from === a)) };
  expect(directMethods(broken, a)).toEqual([]);
  expect(directMethods(graph, graph.nodes.find(n => n.kind === 'interface')!.id)).toEqual([]);
});

it('selection is independent of expansion; explicit switching and collapse repair hidden selection', () => {
  let state = transitionCanvasState(graph, initialCanvasState(1), { type: 'select', id: a });
  expect(state.expandedOwnerId).toBeNull();
  state = transitionCanvasState(graph, state, { type: 'show', id: a });
  state = transitionCanvasState(graph, state, { type: 'select', id: b });
  expect(state.expandedOwnerId).toBe(a);
  state = transitionCanvasState(graph, state, { type: 'select', id: directMethods(graph, a)[0].id });
  state = transitionCanvasState(graph, state, { type: 'show', id: b });
  expect(state.selectedNodeId).toBe(a);
  expect(state.expandedOwnerId).toBe(b);
  state = transitionCanvasState(graph, state, { type: 'select', id: directMethods(graph, b)[0].id });
  state = transitionCanvasState(graph, state, { type: 'hide' });
  expect(state.selectedNodeId).toBe(b);
  expect(graphToCytoscapeElements(graph, state.expandedOwnerId)).toEqual(graphToCytoscapeElements(graph));
});

it('collapse preserves unrelated selection and graph generation resets even equal IDs', () => {
  let state = transitionCanvasState(graph, initialCanvasState(1), { type: 'show', id: a });
  state = transitionCanvasState(graph, state, { type: 'select', id: b });
  state = transitionCanvasState(graph, state, { type: 'hide' });
  expect(state.selectedNodeId).toBe(b);
  expect(transitionCanvasState(graph, state, { type: 'reset', generation: 2 })).toEqual(initialCanvasState(2));
  expect(transitionCanvasState(graph, state, { type: 'show', id: 'absent' })).toBe(state);
});

it('retains ancestors without inventing membership for shared providers', () => {
  const endpoint = graph.nodes.find(n => n.kind === 'endpoint')!;
  const elements = graphToCytoscapeElements(graph, null, new Set([endpoint.id]));
  const expectedIds = [endpoint.id];
  let parent = endpoint.parentId;
  while (parent) { expectedIds.push(parent); parent = graph.nodes.find(n => n.id === parent)!.parentId; }
  expect(new Set(elements.filter(e => e.group === 'nodes').map(e => e.data.id))).toEqual(new Set(expectedIds));
  const shared = graph.nodes.find(n => n.name === 'UsersService')!;
  const membership = graph.edges.find(e => e.kind === 'contains' && e.to === shared.id)!;
  const restricted = graphToCytoscapeElements(graph, null, new Set([shared.id, membership.from]));
  expect(restricted.find(e => e.data.id === shared.id)!.data.parent).toBeUndefined();
  expect(restricted.filter(e => e.data.kind === 'contains').map(e => e.data.id)).toEqual([membership.id]);
  expect(restricted.filter(e => e.group === 'edges').every(e => [shared.id, membership.from].includes(e.data.source!) && [shared.id, membership.from].includes(e.data.target!))).toBe(true);
});

it('explicit navigation expands only the method owner, focuses repeatedly and rejects stale/missing targets', () => {
  let state = transitionCanvasState(graph, initialCanvasState(4), { type: 'show', id: a });
  const method = directMethods(graph, b)[0];
  state = transitionCanvasState(graph, state, { type: 'navigate', id: method.id, generation: 4 });
  expect(state.expandedOwnerId).toBe(b);
  expect(state.selectedNodeId).toBe(method.id);
  expect(state.focusRequest).toEqual({ id: method.id, generation: 4, sequence: 1 });
  const repeated = transitionCanvasState(graph, state, { type: 'navigate', id: method.id, generation: 4 });
  expect(repeated.focusRequest?.sequence).toBe(2);
  const visible = transitionCanvasState(graph, repeated, { type: 'navigate', id: a, generation: 4 });
  expect(visible.expandedOwnerId).toBe(b);
  expect(transitionCanvasState(graph, state, { type: 'navigate', id: a, generation: 3 })).toBe(state);
  expect(transitionCanvasState(graph, state, { type: 'navigate', id: 'missing', generation: 4 })).toBe(state);
  const closed = transitionCanvasState(graph, state, { type: 'select', id: null });
  expect(closed.expandedOwnerId).toBe(b);
  expect(closed.focusRequest).toBeNull();
  expect(transitionCanvasState(graph, state, { type: 'hide' }).selectedNodeId).toBe(b);
  expect(transitionCanvasState(graph, state, { type: 'reset', generation: 5 })).toEqual(initialCanvasState(5));
});
