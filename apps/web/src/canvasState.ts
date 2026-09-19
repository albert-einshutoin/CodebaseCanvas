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
  focusRequest: { id: string; generation: number; sequence: number } | null;
  focusSequence: number;
};
export type CanvasAction =
  | { type: 'select'; id: string | null }
  | { type: 'navigate'; id: string; generation: number }
  | { type: 'show'; id: string }
  | { type: 'hide' }
  | { type: 'reset'; generation: number };

export function initialCanvasState(generation: number): CanvasState {
  return { selectedNodeId: null, expandedOwnerId: null, generation, focusRequest: null, focusSequence: 0 };
}

export function transitionCanvasState(graph: SystemGraph, state: CanvasState, action: CanvasAction): CanvasState {
  if (action.type === 'reset') return initialCanvasState(action.generation);
  if (action.type === 'select') return { ...state, selectedNodeId: action.id, focusRequest: null };
  if (action.type === 'navigate') {
    if (action.generation !== state.generation) return state;
    const node = graph.nodes.find(n => n.id === action.id);
    if (!node) return state;
    if (node.kind === 'method' && !directMethods(graph, node.parentId ?? null).some(n => n.id === node.id)) return state;
    const sequence = state.focusSequence + 1;
    return { ...state, selectedNodeId: node.id,
      expandedOwnerId: node.kind === 'method' ? node.parentId! : state.expandedOwnerId,
      focusSequence: sequence, focusRequest: { id: node.id, generation: state.generation, sequence } };
  }
  if (action.type === 'show' && !directMethods(graph, action.id).length) return state;
  const expandedOwnerId = action.type === 'show' ? action.id : null;
  const hiddenSelection = state.expandedOwnerId !== expandedOwnerId
    && directMethods(graph, state.expandedOwnerId).some(node => node.id === state.selectedNodeId);
  return { ...state, focusRequest: null, expandedOwnerId, selectedNodeId: hiddenSelection ? state.expandedOwnerId : state.selectedNodeId };
}
