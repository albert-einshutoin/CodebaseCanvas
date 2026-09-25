import { useEffect, useMemo, useRef } from 'react';
import cytoscape from 'cytoscape';
import { graphToCytoscapeElements } from './graphCanvasAdapter';
import { applyViewStyle, focusStyle } from './canvasFocus';
import type { Neighborhood, ViewProjection } from './canvasView';
import type { CanvasState } from './canvasState';
import type { SystemGraph } from './graph';
import { layoutCanvas, updateCanvasElements, fitPadding } from './canvasLayout';
import { benchmarkEnabled, benchmarkMark } from './benchmarkTiming';

type GraphCanvasProps = {
  graph: SystemGraph;
  view: ViewProjection;
  neighborhood: Neighborhood;
  onSelect: (nodeId: string | null) => void;
  selectedNodeId: string | null;
  expandedOwnerId: string | null;
  generation: number;
  focusRequest: CanvasState['focusRequest'];
};

const style: cytoscape.StylesheetJson = [
  {
    selector: 'node',
    style: {
      label: 'data(label)',
      'background-color': '#64748b',
      color: '#0f172a',
      'font-size': '14px',
      'font-weight': 600,
      'text-wrap': 'wrap',
      'text-max-width': '145px',
      'text-valign': 'center',
      'text-halign': 'center',
      shape: 'roundrectangle',
      width: 155,
      height: 64,
      'text-overflow-wrap': 'anywhere',
      padding: '12px',
      'border-width': 1,
      'border-color': '#334155',
    },
  },
  {
    selector: 'node[kind = "module"]',
    style: { shape: 'roundrectangle', 'background-color': '#cbd5e1' },
  },
  {
    selector: 'node[kind = "controller"]',
    style: { shape: 'hexagon', 'background-color': '#bfdbfe' },
  },
  {
    selector: 'node[kind = "service"]',
    style: { shape: 'ellipse', 'background-color': '#bbf7d0' },
  },
  {
    selector: 'node[kind = "repository"]',
    style: { shape: 'barrel', 'background-color': '#fde68a' },
  },
  {
    selector: 'node[kind = "endpoint"]',
    style: { shape: 'diamond', 'background-color': '#fbcfe8' },
  },
  {
    selector: 'node[kind = "database_model"]',
    style: { shape: 'barrel', 'background-color': '#ddd6fe' },
  },
  {
    selector: ':parent',
    style: {
      shape: 'roundrectangle',
      'font-size': '18px',
      'background-color': '#e2e8f0',
      'background-opacity': 0.7,
      'border-width': 2,
      'border-color': '#94a3b8',
      padding: '28px',
      'text-valign': 'top',
      'text-margin-y': -8,
    },
  },
  { selector: 'node[kind = "module"]:parent', style: { 'font-size': '24px', 'border-width': 3, 'border-color': '#475569' } },
  {
    selector: 'node:selected',
    style: { 'border-width': 4, 'border-color': '#0f172a' },
  },
  {
    selector: 'edge',
    style: {
      label: '',
      color: '#334155',
      'font-size': '8px',
      'curve-style': 'bezier',
      'line-color': '#94a3b8',
      'target-arrow-color': '#64748b',
      'target-arrow-shape': 'triangle',
      'text-background-color': '#f8fafc',
      'text-background-opacity': 1,
      'text-background-padding': '2px',
    },
  },
  { selector: 'edge.inspected', style: { label: 'data(label)', 'line-color': '#475569', 'width': 2 } },
  ...focusStyle,
];

export function GraphCanvas({ graph, onSelect, selectedNodeId, expandedOwnerId, generation, focusRequest, view, neighborhood }: GraphCanvasProps) {
  const visibleKey = useMemo(() => JSON.stringify([...view.visibleIds].sort()), [view]);
  const containerRef = useRef<HTMLDivElement>(null);
  const cyRef = useRef<cytoscape.Core | null>(null);
  const onSelectRef = useRef(onSelect);
  const syncing = useRef(false);
  const previousGraph = useRef<{ graph: SystemGraph; generation: number } | null>(null);
  const renderListener = useRef<cytoscape.EventHandler | null>(null);
  onSelectRef.current = onSelect;

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const cy = cytoscape({
      container,
      style,
      layout: { name: 'grid', fit: false },
      boxSelectionEnabled: false,
    });
    cyRef.current = cy;

    const handleNodeSelect: cytoscape.EventHandler = event => {
      if (syncing.current) return;
      // Modifier keys can add selections even with Cytoscape's single selection mode.
      cy.nodes(':selected').not(event.target).unselect();
      onSelectRef.current(event.target.id());
    };

    const handleNodeUnselect: cytoscape.EventHandler = event => {
      if (syncing.current) return;
      if (!event.target.cy().nodes(':selected').length) onSelectRef.current(null);
    };
    const handleBackgroundTap: cytoscape.EventHandler = event => {
      if (event.target !== cy) return;
      cy.elements().unselect();
      onSelectRef.current(null);
    };
    const inspectEdge: cytoscape.EventHandler = event => event.target.addClass('inspected');
    const clearEdge: cytoscape.EventHandler = event => {
      if (!event.target.connectedNodes().some((node: cytoscape.NodeSingular) => node.selected())) event.target.removeClass('inspected');
    };
    cy.on('mouseover', 'edge', inspectEdge);
    cy.on('mouseout', 'edge', clearEdge);
    cy.on('select', 'node', handleNodeSelect);
    cy.on('unselect', 'node', handleNodeUnselect);
    cy.on('tap', handleBackgroundTap);

    const resizeObserver = new ResizeObserver(() => cy.resize());
    resizeObserver.observe(container);

    return () => {
      resizeObserver.disconnect();
      cy.removeListener('select', 'node', handleNodeSelect);
      cy.removeListener('unselect', 'node', handleNodeUnselect);
      cy.removeListener('tap', handleBackgroundTap);
      if (renderListener.current) cy.removeListener('render', renderListener.current);
      renderListener.current = null;
      cy.destroy();
      previousGraph.current = null;
      cyRef.current = null;
    };
  }, []);

  useEffect(() => {
    const cy = cyRef.current;
    if (!cy) return;

    const initial = previousGraph.current?.graph !== graph || previousGraph.current.generation !== generation;
    if (initial && renderListener.current) {
      cy.removeListener('render', renderListener.current);
      renderListener.current = null;
    }
    if (initial && benchmarkEnabled(generation)) {
      const onRender: cytoscape.EventHandler = () => {
        const container = containerRef.current;
        if (container?.dataset.graphGeneration !== String(generation) || container.dataset.layoutReady !== 'true' || !cy.nodes().length) return;
        benchmarkMark(generation, 'render_end');
        cy.removeListener('render', onRender);
        renderListener.current = null;
      };
      cy.on('render', onRender);
      renderListener.current = onRender;
      benchmarkMark(generation, 'canvas_start');
    }
    if (containerRef.current) containerRef.current.dataset.layoutReady = 'false';
    syncing.current = true;
    try {
      if (initial) { cy.elements().remove(); cy.reset(); }
      const changed = updateCanvasElements(cy, graphToCytoscapeElements(graph, expandedOwnerId, view.visibleIds));
      if (initial || changed) {
        if (initial) benchmarkMark(generation, 'layout_start');
        layoutCanvas(cy, initial);
        if (initial) benchmarkMark(generation, 'layout_end');
      }
      previousGraph.current = { graph, generation };
    } finally { syncing.current = false; }
    // A kind toggle may change frame styling without changing the element set.
  }, [graph, generation, visibleKey]);

  useEffect(() => {
    const cy = cyRef.current;
    if (!cy) return;
    syncing.current = true;
    try {
      cy.nodes(':selected').not(cy.getElementById(selectedNodeId ?? '')).unselect();
      if (selectedNodeId) cy.getElementById(selectedNodeId).select();
      cy.edges().removeClass('inspected');
      cy.nodes(':selected').connectedEdges().addClass('inspected');
    } finally { syncing.current = false; }
  }, [selectedNodeId, expandedOwnerId, graph, generation, view]);

  useEffect(() => {
    const cy = cyRef.current;
    if (cy) applyViewStyle(cy, view, neighborhood);
  }, [view, neighborhood, graph, generation]);

  // Structural layout and controlled selection effects above finish synchronously first.
  useEffect(() => {
    const cy = cyRef.current;
    if (!cy || !focusRequest || focusRequest.generation !== generation) return;
    const target = cy.getElementById(focusRequest.id);
    if (target.length) cy.center(target);
  }, [focusRequest, generation]);

  // Read-only E2E seam: report the rendered Cytoscape state after its synchronous effects.
  useEffect(() => {
    const cy = cyRef.current;
    const container = containerRef.current;
    if (!cy || !container) return;
    container.dataset.graphGeneration = String(generation);
    container.dataset.graphNodes = String(cy.nodes().length);
    container.dataset.graphEdges = String(cy.edges().length);
    container.dataset.selectedNodes = cy.nodes(':selected').map(node => node.id()).join(',');
    container.dataset.focusNeighbors = String(cy.nodes('.focus-neighbor').length);
    container.dataset.focusDimmed = String(cy.elements('.focus-dim').length);
    container.dataset.focusNeighborIds = JSON.stringify(cy.nodes('.focus-neighbor').map(node => node.id()).sort());
    container.dataset.focusContextIds = JSON.stringify(cy.nodes('.focus-context').map(node => node.id()).sort());
    container.dataset.focusDimmedNodeIds = JSON.stringify(cy.nodes('.focus-dim').map(node => node.id()).sort());
    container.dataset.focusEdgeIds = JSON.stringify(cy.edges().not('.focus-dim').map(edge => edge.id()).sort());
    container.dataset.parentFrameIds = JSON.stringify(cy.nodes(':parent').map(node => node.id()).sort());
    container.dataset.ancestorFrameIds = JSON.stringify(cy.nodes('.ancestor-frame').map(node => node.id()).sort());
    container.dataset.layoutReady = 'true';
  }, [graph, generation, view, neighborhood, selectedNodeId, expandedOwnerId, focusRequest]);

  function zoomBy(factor: number) {
    const cy = cyRef.current;
    if (!cy) return;
    cy.zoom(Math.min(cy.maxZoom(), Math.max(cy.minZoom(), cy.zoom() * factor)));
  }

  function fitGraph() {
    const cy = cyRef.current;
    if (!cy) return;
    if (cy.elements().length) cy.fit(cy.elements(), fitPadding);
    else cy.reset();
  }

  return (
    <section className="canvas-shell" aria-label="SystemGraph canvas">
      <div className="canvas-toolbar" aria-label="Canvas controls">
        <button type="button" aria-label="Zoom in" onClick={() => zoomBy(1.2)}>＋</button>
        <button type="button" aria-label="Zoom out" onClick={() => zoomBy(1 / 1.2)}>－</button>
        <button type="button" aria-label="Fit graph to screen" onClick={fitGraph}>Fit</button>
      </div>
      <div ref={containerRef} className="canvas-viewport" role="application" aria-label="Interactive code graph" />
      {!view.visibleIds.size && <p className="canvas-empty" role="status">{graph.nodes.length ? 'No nodes match these filters. Reset kind filters to restore the view.' : 'This graph has no nodes to display.'}</p>}
    </section>
  );
}
