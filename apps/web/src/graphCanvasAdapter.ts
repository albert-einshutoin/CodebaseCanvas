import cytoscape from 'cytoscape';
import type { SystemGraph } from './graph';

export function graphToCytoscapeElements(graph: SystemGraph): cytoscape.ElementDefinition[] {
  const visibleNodes = graph.nodes.filter(node => node.kind !== 'method');
  const visibleIds = new Set(visibleNodes.map(node => node.id));
  const nodes: cytoscape.NodeDefinition[] = visibleNodes.map(node => ({
    group: 'nodes',
    data: {
      id: node.id,
      label: `[${node.kind}]\n${node.name}`,
      kind: node.kind,
      parent: node.parentId && visibleIds.has(node.parentId) ? node.parentId : undefined,
      parentId: node.parentId,
      file: node.file,
      line: node.line,
    },
  }));
  const edges: cytoscape.EdgeDefinition[] = graph.edges
    .filter(edge => visibleIds.has(edge.from) && visibleIds.has(edge.to))
    .map(edge => ({
      group: 'edges',
      data: {
        id: edge.id,
        source: edge.from,
        target: edge.to,
        kind: edge.kind,
        label: edge.kind,
      },
    }));

  return [...nodes, ...edges];
}
