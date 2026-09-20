import { nodeIdentityDetails, type Diagnostic, type Evidence, type GraphEdge, type GraphNode, type SystemGraph } from './graph';
import { directMethods } from './canvasState';

const compare = (a: string, b: string) => a < b ? -1 : a > b ? 1 : 0;
const byNode = (a: GraphNode, b: GraphNode) => compare(a.kind, b.kind) || compare(a.name, b.name) || compare(a.id, b.id);
export const sortedEvidence = (items: Evidence[]) => [...items].sort((a, b) => compare(
  JSON.stringify([a.file, a.line, a.endLine, a.source, a.confidence]),
  JSON.stringify([b.file, b.line, b.endLine, b.source, b.confidence]),
));
export const sortedDiagnostics = (items: Diagnostic[]) => [...items].sort((a, b) => compare(
  JSON.stringify([a.relatedNodeId, a.file, a.line, a.code, a.severity, a.message, a.skippedCount]),
  JSON.stringify([b.relatedNodeId, b.file, b.line, b.code, b.severity, b.message, b.skippedCount]),
));
const detailNode = (node: GraphNode) => ({ ...node, evidence: sortedEvidence(node.evidence) });

export function relationshipLabel(edge: GraphEdge, from: GraphNode, to: GraphNode, incoming: boolean) {
  switch (edge.kind) {
    case 'injects': return incoming ? 'Requested by' : 'Requests token';
    case 'contains': return to.kind === 'method' ? 'Lexical ownership' : 'Module registration / membership';
    case 'depends_on': return from.kind === 'endpoint' ? (incoming ? 'Handler for endpoint' : 'Declared handler') : 'Module import';
    case 'calls': return incoming ? 'Statically called by' : 'Static call target';
    case 'imports': return 'Import binding use';
    default: return edge.kind;
  }
}

/** Bounded, one-hop details of a validated snapshot, independent of Canvas projection. */
export function nodeDetails(graph: SystemGraph, id: string | null) {
  const nodes = new Map(graph.nodes.map(n => [n.id, n]));
  const selected = id === null ? undefined : nodes.get(id);
  if (!selected) return;
  const node = detailNode(selected);
  const methods = directMethods(graph, id).sort(byNode).map(detailNode);
  const incomingEdges = graph.edges.filter(e => e.to === id);
  const outgoingEdges = graph.edges.filter(e => e.from === id);
  const relationships = (edges: GraphEdge[], incoming: boolean) => edges.map(edge => ({
    edge: { ...edge, evidence: sortedEvidence(edge.evidence) },
    other: detailNode(nodes.get(incoming ? edge.from : edge.to)!),
    label: relationshipLabel(edge, nodes.get(edge.from)!, nodes.get(edge.to)!, incoming),
  })).sort((a, b) => compare(a.edge.kind, b.edge.kind) || byNode(a.other, b.other) || compare(a.edge.id, b.edge.id));
  const memberships = incomingEdges.filter(e => e.kind === 'contains' && nodes.get(e.from)?.kind === 'module' && node.kind !== 'method')
    .map(e => detailNode(nodes.get(e.from)!)).sort(byNode);
  const owner = node.kind === 'method' && node.parentId && directMethods(graph, node.parentId).some(n => n.id === id)
    ? nodes.get(node.parentId) : undefined;
  const handler = node.kind === 'endpoint' ? nodes.get(outgoingEdges.find(e => e.kind === 'depends_on')?.to ?? '') : undefined;
  const controller = node.kind === 'endpoint' ? nodes.get(incomingEdges.find(e => e.kind === 'exposes')?.from ?? '') : undefined;
  const related = [...methods, ...(handler ? [handler] : [])].sort(byNode);
  const diagnostics = sortedDiagnostics(graph.diagnostics.filter(d => d.relatedNodeId === id));
  const relatedDiagnostics = related.map(n => ({ node: detailNode(n), diagnostics: sortedDiagnostics(graph.diagnostics.filter(d => d.relatedNodeId === n.id)) }));
  const methodIds = new Set(node.kind === 'method' ? [node.id] : related.map(n => n.id));
  const skipped = graph.diagnostics.filter(d => d.relatedNodeId && methodIds.has(d.relatedNodeId)).reduce((sum, d) => sum + (d.skippedCount ?? 0), 0);
  // File notices have no node attribution and never contribute to selected-method counts.
  const fileDiagnostics = sortedDiagnostics(graph.diagnostics.filter(d => d.relatedNodeId === undefined && node.file !== undefined && d.file === node.file));
  const rawFields = node.kind === 'database_model' ? node.metadata?.fields : undefined;
  const fields = Array.isArray(rawFields) ? rawFields.flatMap(value => {
    if (typeof value === 'string') return [{ name: value, type: undefined }];
    if (value && typeof value === 'object' && !Array.isArray(value) && typeof value.name === 'string') {
      return [{ name: value.name, type: typeof value.type === 'string' ? value.type : undefined }];
    }
    return [];
  }).sort((a, b) => compare(a.name, b.name) || compare(a.type ?? '', b.type ?? '')) : [];
  return { node, methods, memberships, owner: owner && detailNode(owner), handler: handler && detailNode(handler), controller: controller && detailNode(controller),
    incoming: relationships(incomingEdges, true), outgoing: relationships(outgoingEdges, false), diagnostics, relatedDiagnostics, fileDiagnostics, skipped, fields,
    ...nodeIdentityDetails(node) };
}
export type NodeDetailsModel = NonNullable<ReturnType<typeof nodeDetails>>;
