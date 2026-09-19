import { nodeIdentityDetails, type GraphNode, type SystemGraph } from './graph';

export const kindOptions = [
  ['module', 'Module'], ['controller', 'Controller'], ['service', 'Service'],
  ['repository', 'Repository'], ['class', 'Class'], ['interface', 'Interface'],
  ['endpoint', 'Endpoint'], ['database_model', 'Database'], ['external_dependency', 'External dependency'],
] as const;
export type FilterKind = typeof kindOptions[number][0];
export const defaultKinds = (): ReadonlySet<FilterKind> => new Set(kindOptions.map(([kind]) => kind));
const compare = (a: string, b: string) => a < b ? -1 : a > b ? 1 : 0;
const ownerKinds = new Set(['module', 'controller', 'service', 'repository', 'class']);

export function directMethods(graph: SystemGraph, ownerId: string | null) {
  const owner = graph.nodes.find(node => node.id === ownerId);
  if (!owner || !ownerKinds.has(owner.kind)) return [];
  const owned = new Set(graph.edges.filter(edge => edge.kind === 'contains' && edge.from === ownerId).map(edge => edge.to));
  return graph.nodes.filter(node => node.kind === 'method' && node.parentId === ownerId && owned.has(node.id));
}

export function searchIndex(graph: SystemGraph) {
  return [...graph.nodes].sort((a, b) => compare(a.kind, b.kind) || compare(a.name, b.name) || compare(a.id, b.id)).map(node => {
    const values: unknown[] = [node.name, node.qualifiedName];
    if (node.kind === 'endpoint') {
      values.push(node.metadata?.path);
      if (typeof node.metadata?.httpMethod === 'string' && typeof node.metadata?.path === 'string') values.push(`${node.metadata.httpMethod} ${node.metadata.path}`);
    }
    if (node.kind === 'external_dependency') values.push(nodeIdentityDetails(node).specifier, node.metadata?.package, node.metadata?.symbol);
    return { node, values: values.filter((v): v is string => typeof v === 'string').map(v => v.toLowerCase()) };
  });
}
export function searchNodes(index: ReturnType<typeof searchIndex>, query: string) {
  const term = query.trim().toLowerCase();
  const matches = term ? index.filter(row => row.values.some(value => value.includes(term))) : [];
  return { empty: !term, total: matches.length, nodes: matches.slice(0, 50).map(row => row.node) };
}

export function projectView(graph: SystemGraph, enabledKinds: ReadonlySet<FilterKind>, expandedOwnerId: string | null) {
  const matchedIds = new Set(graph.nodes.filter(n => n.kind !== 'method' && enabledKinds.has(n.kind)).map(n => n.id));
  if (expandedOwnerId && matchedIds.has(expandedOwnerId)) for (const method of directMethods(graph, expandedOwnerId)) matchedIds.add(method.id);
  const visibleIds = new Set(matchedIds);
  const nodes = new Map(graph.nodes.map(n => [n.id, n]));
  // Validated parent chains are acyclic. Added frames never add siblings or methods.
  for (const id of visibleIds) {
    const parent = nodes.get(id)?.parentId;
    if (parent) visibleIds.add(parent);
  }
  return { matchedIds, visibleIds, ancestorIds: new Set([...visibleIds].filter(id => !matchedIds.has(id))) };
}
export type ViewProjection = ReturnType<typeof projectView>;

export function directNeighborhood(graph: SystemGraph, visibleIds: ReadonlySet<string>, anchorId: string | null) {
  if (!anchorId || !visibleIds.has(anchorId)) return null;
  const nodeIds = new Set([anchorId]);
  const edgeIds = new Set<string>();
  for (const edge of graph.edges) {
    if ((edge.from === anchorId || edge.to === anchorId) && visibleIds.has(edge.from) && visibleIds.has(edge.to)) {
      nodeIds.add(edge.from); nodeIds.add(edge.to); edgeIds.add(edge.id);
    }
  }
  const nodes = new Map(graph.nodes.map(n => [n.id, n]));
  const contextIds = new Set<string>();
  for (const id of nodeIds) {
    let parent = nodes.get(id)?.parentId;
    while (parent) { if (!nodeIds.has(parent)) contextIds.add(parent); parent = nodes.get(parent)?.parentId; }
  }
  return { nodeIds, edgeIds, contextIds };
}
export type Neighborhood = ReturnType<typeof directNeighborhood>;

export function navigationLabel(node: GraphNode, visibleIds: ReadonlySet<string>) {
  return visibleIds.has(node.id) ? 'Go to node' : node.kind === 'method' ? 'Reveal & go (show owner kind and expand method)' : 'Reveal & go (show this kind)';
}

export function candidateDescription(node: GraphNode, nodes: ReadonlyMap<string, GraphNode>) {
  const identity = nodeIdentityDetails(node);
  const owner = node.parentId ? nodes.get(node.parentId) : undefined;
  const handlerId = node.metadata?.controllerMethodId;
  const handler = typeof handlerId === 'string' ? nodes.get(handlerId) : undefined;
  return [node.file, owner && `owner: ${owner.name}`, identity.methodKind, identity.specifier,
    handler && `handler: ${handler.qualifiedName ?? handler.name}`, node.qualifiedName].filter(Boolean).join(' · ');
}
