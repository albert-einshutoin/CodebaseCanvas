import { describe, expect, it } from 'vitest';
import expected from '../../../examples/nestjs-sample/expected-graph.json';
import { SystemGraphSchema } from './graph';
import { graphToCytoscapeElements } from './graphCanvasAdapter';

const graph = SystemGraphSchema.parse(expected);

describe('graphToCytoscapeElements', () => {
  it('converts visible nodes with kind labels and display parents', () => {
    const elements = graphToCytoscapeElements(graph);
    const nodes = elements.filter(element => element.group === 'nodes');
    const byId = new Map(graph.nodes.map(node => [node.id, node]));

    expect(nodes).toHaveLength(graph.nodes.filter(node => node.kind !== 'method').length);
    expect(nodes.every(node => byId.get(node.data.id as string)?.kind !== 'method')).toBe(true);
    expect(nodes.find(node => node.data.kind === 'controller')?.data.label).toContain('[controller]');
    expect(nodes.find(node => node.data.kind === 'endpoint')?.data.parent).toBeDefined();
  });

  it('converts only edges between visible nodes and preserves edge kinds', () => {
    const elements = graphToCytoscapeElements(graph);
    const nodes = elements.filter(element => element.group === 'nodes');
    const edges = elements.filter(element => element.group === 'edges');
    const visibleIds = new Set(nodes.map(node => node.data.id));
    const expectedEdges = graph.edges.filter(edge => visibleIds.has(edge.from) && visibleIds.has(edge.to));

    expect(edges).toHaveLength(expectedEdges.length);
    expect(edges.map(edge => edge.data.kind)).toEqual(expectedEdges.map(edge => edge.kind));
    expect(edges.every(edge => edge.data.source && edge.data.target)).toBe(true);
  });

  it('returns no elements for an empty graph', () => {
    const emptyGraph = SystemGraphSchema.parse({
      ...graph,
      metadata: { ...graph.metadata, callAnalysis: { ...graph.metadata.callAnalysis, examinedCalls: 0, emittedCalls: 0, skippedCalls: 0 } },
      nodes: [],
      edges: [],
      diagnostics: [],
    });

    expect(graphToCytoscapeElements(emptyGraph)).toEqual([]);
  });
});

it('preserves the exact fixture projection without mutating the source graph', () => {
  const before = structuredClone(graph);
  const elements = graphToCytoscapeElements(graph);
  const nodes = elements.filter(e => e.group === 'nodes');
  const edges = elements.filter(e => e.group === 'edges');
  const visible = graph.nodes.filter(n => n.kind !== 'method');
  const ids = new Set(visible.map(n => n.id));
  expect(nodes).toHaveLength(40);
  expect(nodes.map(n => n.data.id)).toEqual(visible.map(n => n.id));
  expect(edges.map(e => [e.data.id, e.data.source, e.data.kind, e.data.target])).toEqual(
    graph.edges.filter(e => ids.has(e.from) && ids.has(e.to)).map(e => [e.id, e.from, e.kind, e.to]),
  );
  for (const node of nodes) {
    const original = visible.find(n => n.id === node.data.id)!;
    expect(node.data.parent).toBe(original.parentId);
    expect(node.data.label).toContain(`[${original.kind}]`);
    if (node.data.parent) expect(ids.has(node.data.parent)).toBe(true);
  }
  const users = nodes.filter(n => n.data.label === '[service]\nUsersService');
  expect(users).toHaveLength(1);
  expect(users[0].data.parent).toBeUndefined();
  expect(edges.filter(e => e.data.kind === 'contains' && e.data.target === users[0].data.id)).toHaveLength(2);
  const endpoints = visible.filter(n => n.kind === 'endpoint' && n.metadata?.httpMethod === 'GET' && n.metadata.path === '/users');
  expect(endpoints).toHaveLength(2);
  expect(new Set(endpoints.map(n => n.id)).size).toBe(2);
  expect(edges.filter(e => e.data.kind === 'calls')).toHaveLength(0);
  expect(graph).toEqual(before);
});

it('labels injects as Requests token while preserving its kind and canonical endpoints', () => {
  const edges = graphToCytoscapeElements(graph).filter(e => e.group === 'edges' && e.data.kind === 'injects');
  expect(edges).toHaveLength(5);
  for (const edge of edges) {
    const original = graph.edges.find(e => e.id === edge.data.id)!;
    expect(edge.data.label).toBe('Requests token');
    expect(edge.data.kind).toBe('injects');
    expect([edge.data.source, edge.data.target]).toEqual([original.from, original.to]);
    expect(original.metadata?.semantics).toBe('requested_token');
  }
});
