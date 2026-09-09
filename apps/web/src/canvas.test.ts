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
