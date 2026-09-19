import { useMemo } from 'react';
import type { SystemGraph } from './graph';
import type { CanvasAction, CanvasState } from './canvasState';
import { candidateDescription, defaultKinds, kindOptions, navigationLabel, searchIndex, searchNodes, type Neighborhood, type ViewProjection } from './canvasView';

type Props = { graph: SystemGraph; state: CanvasState; view: ViewProjection; neighborhood: Neighborhood; onChange: (action: CanvasAction) => void };
export function CanvasControls({ graph, state, view, neighborhood, onChange }: Props) {
  const index = useMemo(() => searchIndex(graph), [graph]);
  const nodes = useMemo(() => new Map(graph.nodes.map(n => [n.id, n])), [graph]);
  const results = useMemo(() => searchNodes(index, state.query), [index, state.query]);
  const anchor = nodes.get(state.neighborhoodAnchorId ?? '');
  return <><section className="view-controls" aria-label="Search, filters and dependency focus">
    <label htmlFor="symbol-search">Search symbols</label>
    <input id="symbol-search" type="search" value={state.query} placeholder="Name, qualified name, endpoint path or external specifier"
      onChange={event => onChange({ type: 'query', query: event.currentTarget.value })} />
    <p role="status">{results.empty ? 'Search the full snapshot, including hidden nodes and methods.'
      : `${results.total} matches · showing ${results.nodes.length} (limit 50).${results.total > 50 ? ' Refine your search to see other matches.' : ''}`}</p>
    {!!results.nodes.length && <ul className="search-results" aria-label="Search results">{results.nodes.map(node => <li key={node.id}>
      <button type="button" onClick={() => onChange({ type: 'navigate', id: node.id, generation: state.generation })}>
        {navigationLabel(node, view.visibleIds)}: [{node.kind}] {node.name}
        <small>{candidateDescription(node, nodes)}</small><small>{node.id}</small>
      </button>
    </li>)}</ul>}
    <fieldset><legend>Visible node kinds</legend><div className="kind-options">{kindOptions.map(([kind, label]) => <label key={kind}>
      <input type="checkbox" checked={state.enabledKinds.has(kind)} onChange={event => {
        const kinds = new Set(state.enabledKinds);
        if (event.currentTarget.checked) kinds.add(kind); else kinds.delete(kind);
        onChange({ type: 'kinds', kinds });
      }} />{label}
    </label>)}</div>
      <button type="button" onClick={() => onChange({ type: 'kinds', kinds: defaultKinds() })}>Reset kind filters</button>
      <button type="button" onClick={() => onChange({ type: 'kinds', kinds: new Set() })}>Hide all kinds</button>
    </fieldset>
    <p>{view.matchedIds.size} matching / expanded nodes + {view.ancestorIds.size} ancestor frames = {view.visibleIds.size} displayed nodes.
      Required parent frames remain even when their kind is off; they do not reveal siblings or methods.</p>
    <p>Depth 1, incoming and outgoing static relationships. Other elements are dimmed; this is not an execution path or recursive dependency view.</p>
  </section>
    <div className="neighborhood-controls" role="region" aria-label="Dependency focus controls">
      <button type="button" disabled={!state.selectedNodeId} onClick={() => onChange({ type: 'neighbors', generation: state.generation })}>Focus neighbors</button>
      {anchor && neighborhood && <><strong>Focus: [{anchor.kind}] {anchor.name}</strong><span>{neighborhood.nodeIds.size - 1} direct neighbors · {neighborhood.edgeIds.size} direct edges · {neighborhood.contextIds.size} context frames</span>
        <button type="button" onClick={() => onChange({ type: 'clear-neighbors' })}>Clear focus</button></>}
    </div>
  </>;
}
