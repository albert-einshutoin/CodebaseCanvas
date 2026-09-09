import { useEffect, useRef } from 'react';
import cytoscape from 'cytoscape';
import { graphToCytoscapeElements } from './graphCanvasAdapter';
import type { SystemGraph } from './graph';

type GraphCanvasProps = {
  graph: SystemGraph;
  onSelect: (nodeId: string | null) => void;
};

const fitPadding = 32;

const style: cytoscape.StylesheetJson = [
  {
    selector: 'node',
    style: {
      label: 'data(label)',
      'background-color': '#64748b',
      color: '#0f172a',
      'font-size': '10px',
      'font-weight': 600,
      'text-wrap': 'wrap',
      'text-max-width': '140px',
      'text-valign': 'center',
      'text-halign': 'center',
      shape: 'roundrectangle',
      width: 'label',
      height: 'label',
      padding: '12px',
      'border-width': 1,
      'border-color': '#334155',
    },
  },
  {
    selector: ':parent',
    style: {
      'background-color': '#e2e8f0',
      'background-opacity': 0.7,
      'border-width': 2,
      'border-color': '#94a3b8',
      padding: '18px',
      'text-valign': 'top',
      'text-margin-y': 6,
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
    selector: 'node:selected',
    style: { 'border-width': 4, 'border-color': '#0f172a' },
  },
  {
    selector: 'edge',
    style: {
      label: 'data(label)',
      color: '#475569',
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
];

export function GraphCanvas({ graph, onSelect }: GraphCanvasProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const cyRef = useRef<cytoscape.Core | null>(null);
  const onSelectRef = useRef(onSelect);
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
      onSelectRef.current(event.target.id());
    };

    const handleNodeUnselect: cytoscape.EventHandler = event => {
      if (!event.target.cy().nodes(':selected').length) onSelectRef.current(null);
    };
    const handleBackgroundTap: cytoscape.EventHandler = event => {
      if (event.target !== cy) return;
      cy.elements().unselect();
      onSelectRef.current(null);
    };
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
      cy.destroy();
      cyRef.current = null;
    };
  }, []);

  useEffect(() => {
    const cy = cyRef.current;
    if (!cy) return;

    const elements = graphToCytoscapeElements(graph);
    cy.elements().remove();
    if (!elements.length) {
      cy.reset();
      onSelectRef.current(null);
      return;
    }

    cy.add(elements);
    cy.layout({ name: 'breadthfirst', directed: true, fit: false, animate: false, padding: fitPadding }).run();
    cy.fit(cy.elements(), fitPadding);
    onSelectRef.current(null);
  }, [graph]);

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
      {!graph.nodes.length && <p className="canvas-empty" role="status">This graph has no nodes to display.</p>}
    </section>
  );
}
