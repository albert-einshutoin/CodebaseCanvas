import type { SystemGraph } from './graph';
import { defaultKinds, directMethods, projectView, type FilterKind } from './canvasView';
export { directMethods } from './canvasView';

export type CanvasState = {
  query: string;
  enabledKinds: ReadonlySet<FilterKind>;
  neighborhoodAnchorId: string | null;
  selectedNodeId: string | null;
  expandedOwnerId: string | null;
  generation: number;
  focusRequest: { id: string; generation: number; sequence: number } | null;
  focusSequence: number;
};
export type CanvasAction =
  | { type: 'query'; query: string }
  | { type: 'kinds'; kinds: ReadonlySet<FilterKind> }
  | { type: 'neighbors'; generation: number }
  | { type: 'clear-neighbors' }
  | { type: 'select'; id: string | null }
  | { type: 'navigate'; id: string; generation: number }
  | { type: 'show'; id: string }
  | { type: 'hide' }
  | { type: 'reset'; generation: number };

export function initialCanvasState(generation: number): CanvasState {
  return { query: '', enabledKinds: defaultKinds(), neighborhoodAnchorId: null,
    selectedNodeId: null, expandedOwnerId: null, generation, focusRequest: null, focusSequence: 0 };
}

function reconcileView(graph: SystemGraph, state: CanvasState): CanvasState {
  const { visibleIds } = projectView(graph, state.enabledKinds, state.expandedOwnerId);
  return { ...state,
    selectedNodeId: state.selectedNodeId && visibleIds.has(state.selectedNodeId) ? state.selectedNodeId : null,
    neighborhoodAnchorId: state.neighborhoodAnchorId && visibleIds.has(state.neighborhoodAnchorId) ? state.neighborhoodAnchorId : null,
    focusRequest: state.focusRequest && visibleIds.has(state.focusRequest.id) ? state.focusRequest : null,
  };
}

export function transitionCanvasState(graph: SystemGraph, state: CanvasState, action: CanvasAction): CanvasState {
  if (action.type === 'reset') return initialCanvasState(action.generation);
  if (action.type === 'query') return { ...state, query: action.query };
  if (action.type === 'kinds') return reconcileView(graph, { ...state, enabledKinds: new Set(action.kinds) });
  if (action.type === 'clear-neighbors') return { ...state, neighborhoodAnchorId: null };
  if (action.type === 'neighbors') return action.generation === state.generation
    ? reconcileView(graph, { ...state, neighborhoodAnchorId: state.selectedNodeId }) : state;
  if (action.type === 'select') return { ...state, selectedNodeId: action.id, focusRequest: null };
  if (action.type === 'navigate') {
    if (action.generation !== state.generation) return state;
    const node = graph.nodes.find(n => n.id === action.id);
    if (!node) return state;
    if (node.kind === 'method' && !directMethods(graph, node.parentId ?? null).some(n => n.id === node.id)) return state;
    const requiredNode = node.kind === 'method' ? graph.nodes.find(n => n.id === node.parentId)! : node;
    let enabledKinds = state.enabledKinds;
    // Ancestor-only frames are already visible; navigating to them needs no reveal.
    if (!projectView(graph, state.enabledKinds, state.expandedOwnerId).visibleIds.has(node.id) && requiredNode.kind !== 'method') enabledKinds = new Set([...state.enabledKinds, requiredNode.kind]);
    const sequence = state.focusSequence + 1;
    return { ...state, enabledKinds, neighborhoodAnchorId: null, selectedNodeId: node.id,
      expandedOwnerId: node.kind === 'method' ? node.parentId! : state.expandedOwnerId,
      focusSequence: sequence, focusRequest: { id: node.id, generation: state.generation, sequence } };
  }
  if (action.type === 'show' && !directMethods(graph, action.id).length) return state;
  const expandedOwnerId = action.type === 'show' ? action.id : null;
  const hiddenSelection = state.expandedOwnerId !== expandedOwnerId
    && directMethods(graph, state.expandedOwnerId).some(node => node.id === state.selectedNodeId);
  return reconcileView(graph, { ...state, focusRequest: null, expandedOwnerId, selectedNodeId: hiddenSelection ? state.expandedOwnerId : state.selectedNodeId });
}
