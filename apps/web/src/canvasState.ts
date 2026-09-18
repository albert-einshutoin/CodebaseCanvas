import type { SystemGraph } from './graph';

const ownerKinds = new Set(['module', 'controller', 'service', 'repository', 'class']);

export function directMethods(graph: SystemGraph, ownerId: string | null) {
  const owner = graph.nodes.find(node => node.id === ownerId);
  if (!owner || !ownerKinds.has(owner.kind)) return [];
  const owned = new Set(graph.edges.filter(edge => edge.kind === 'contains' && edge.from === ownerId).map(edge => edge.to));
  return graph.nodes.filter(node => node.kind === 'method' && node.parentId === ownerId && owned.has(node.id));
}

export type CanvasState = {
  selectedNodeId: string | null;
  expandedOwnerId: string | null;
  generation: number;
};
export type CanvasAction =
  | { type: 'select'; id: string | null }
  | { type: 'show'; id: string }
  | { type: 'hide' }
  | { type: 'reset'; generation: number };

export function initialCanvasState(generation: number): CanvasState {
  return { selectedNodeId: null, expandedOwnerId: null, generation };
}

export function transitionCanvasState(graph: SystemGraph, state: CanvasState, action: CanvasAction): CanvasState {
  if (action.type === 'reset') return initialCanvasState(action.generation);
  if (action.type === 'select') return { ...state, selectedNodeId: action.id };
  if (action.type === 'show' && !directMethods(graph, action.id).length) return state;
  const expandedOwnerId = action.type === 'show' ? action.id : null;
  const hiddenSelection = state.expandedOwnerId !== expandedOwnerId
    && directMethods(graph, state.expandedOwnerId).some(node => node.id === state.selectedNodeId);
  return { ...state, expandedOwnerId, selectedNodeId: hiddenSelection ? state.expandedOwnerId : state.selectedNodeId };
}
