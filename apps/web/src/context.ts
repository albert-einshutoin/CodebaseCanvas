import type { GraphEdge, GraphNode, SystemGraph } from './graph';
import { directMethods } from './canvasState';
import { relationshipLabel } from './nodeDetails';

export const compare = (a: string, b: string) => a < b ? -1 : a > b ? 1 : 0;
export function compareContextNodes(a: GraphNode, b: GraphNode) {
  return compare(a.kind, b.kind) || (a.kind === 'endpoint' && b.kind === 'endpoint'
    ? compare(String(a.metadata?.httpMethod), String(b.metadata?.httpMethod)) || compare(String(a.metadata?.path), String(b.metadata?.path))
    : compare(a.name, b.name)) || compare(a.id, b.id);
}
export type ContextItem = { nodes: GraphNode[]; edges: GraphEdge[]; label: string };
export const contextSections = {
  outgoing: 'Outgoing relationships', incoming: 'Incoming relationships', methods: 'Direct methods',
  tokenEndpoints: 'Declared on controllers requesting this token (not proven execution paths)',
  handlerCalls: 'Endpoint handler static calls', controllerTokens: 'Endpoint controller requested tokens',
} as const;
export type ContextSectionKey = keyof typeof contextSections;

/** Only these selection-rooted paths are allowed. Collected nodes never become new roots. */
export function collectContext(graph: SystemGraph, id: string | null) {
  const nodes = new Map(graph.nodes.map(n => [n.id, n]));
  const selected = id === null ? undefined : nodes.get(id);
  if (!selected) throw new Error('Select an existing component.');
  const outgoing = graph.edges.filter(e => e.from === id);
  const incoming = graph.edges.filter(e => e.to === id);
  const rows: Record<ContextSectionKey, ContextItem[]> = {
    outgoing: outgoing.map(e => ({ nodes: [nodes.get(e.to)!], edges: [e], label: relationshipLabel(e, selected, nodes.get(e.to)!, false) })),
    incoming: incoming.map(e => ({ nodes: [nodes.get(e.from)!], edges: [e], label: relationshipLabel(e, nodes.get(e.from)!, selected, true) })),
    methods: directMethods(graph, id).map(n => ({ nodes: [n], edges: [], label: 'Direct method' })),
    tokenEndpoints: [], handlerCalls: [], controllerTokens: [],
  };
  for (const di of incoming.filter(e => e.kind === 'injects' && nodes.get(e.from)?.kind === 'controller')) {
    for (const route of graph.edges.filter(e => e.kind === 'exposes' && e.from === di.from)) {
      rows.tokenEndpoints.push({ nodes: [nodes.get(route.to)!, nodes.get(di.from)!], edges: [di, route], label: 'Declared route via requesting controller' });
    }
  }
  if (selected.kind === 'endpoint') {
    for (const handler of outgoing.filter(e => e.kind === 'depends_on')) {
      for (const call of graph.edges.filter(e => e.kind === 'calls' && e.from === handler.to)) {
        rows.handlerCalls.push({ nodes: [nodes.get(call.to)!, nodes.get(handler.to)!], edges: [handler, call], label: 'Static call target via declared handler' });
      }
    }
    for (const route of incoming.filter(e => e.kind === 'exposes')) {
      for (const di of graph.edges.filter(e => e.kind === 'injects' && e.from === route.from)) {
        rows.controllerTokens.push({ nodes: [nodes.get(di.to)!, nodes.get(route.from)!], edges: [route, di], label: 'Requests token via declaring controller' });
      }
    }
  }
  const sections = (Object.keys(contextSections) as ContextSectionKey[]).map(key => ({ key, title: contextSections[key], items: rows[key].sort((a, b) =>
    compareContextNodes(a.nodes[0], b.nodes[0]) || compare(a.edges.map(e => e.id).join('\n'), b.edges.map(e => e.id).join('\n'))
      || compare(a.nodes.map(n => n.id).join('\n'), b.nodes.map(n => n.id).join('\n'))) }));
  return { selected, sections };
}

/** Return a tentative admission; callers commit IDs only after the whole item fits. */
export function additionalContextNodes(included: ReadonlySet<string>, ids: string[]): string[] | undefined {
  const added = [...new Set(ids)].filter(id => !included.has(id));
  return included.size - 1 + added.length <= 200 ? added : undefined;
}
