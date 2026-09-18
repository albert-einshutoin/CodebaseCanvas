import cytoscape from 'cytoscape';
import type { SystemGraph } from './graph';
import { directMethods } from './canvasState';

export function graphToCytoscapeElements(
  graph: SystemGraph,
  expandedOwnerId: string | null = null,
  candidateIds?: ReadonlySet<string>,
): cytoscape.ElementDefinition[] {
  const methodIds = new Set(directMethods(graph, expandedOwnerId).map(node => node.id));
  const byId = new Map(graph.nodes.map(node => [node.id, node]));
  const included = new Set(graph.nodes.filter(node =>
    (node.kind !== 'method' || methodIds.has(node.id)) && (!candidateIds || candidateIds.has(node.id)),
  ).map(node => node.id));
  // A restricted projection keeps canonical ancestors; it never assigns new parents.
  for (const id of included) {
    const parentId = byId.get(id)?.parentId;
    if (parentId && byId.has(parentId)) included.add(parentId);
  }
  const visibleNodes = graph.nodes.filter(node => included.has(node.id));
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
        label: edge.kind === 'injects' ? 'Requests token' : edge.kind,
      },
    }));

  return [...nodes, ...edges];
}
