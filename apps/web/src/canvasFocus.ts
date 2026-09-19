import type cytoscape from 'cytoscape';
import type { Neighborhood, ViewProjection } from './canvasView';

export const focusStyle: cytoscape.StylesheetJson = [
  { selector: 'node.ancestor-frame', style: { 'border-style': 'dashed' } },
  { selector: 'node.focus-dim:childless', style: { opacity: 0.18 } },
  { selector: 'edge.focus-dim', style: { opacity: 0.12 } },
  // Compound opacity propagates to children. Change only the frame's own paint.
  { selector: 'node.focus-dim:parent', style: { 'background-opacity': 0.06, 'border-opacity': 0.18, 'text-opacity': 0.22 } },
  { selector: 'node.focus-context', style: { 'background-opacity': 0.08, 'border-opacity': 0.5, 'text-opacity': 0.7 } },
  { selector: 'node.focus-neighbor', style: { 'border-color': '#2563eb', 'border-width': 3 } },
  { selector: 'node.focus-neighbor:selected', style: { 'border-color': '#0f172a', 'border-width': 4 } },
];

export function applyViewStyle(cy: cytoscape.Core, view: ViewProjection, neighborhood: Neighborhood) {
  cy.batch(() => {
    cy.elements().removeClass('ancestor-frame focus-dim focus-context focus-neighbor');
    cy.nodes().filter(n => view.ancestorIds.has(n.id())).addClass('ancestor-frame');
    if (!neighborhood) return;
    cy.nodes().forEach(node => {
      node.addClass(neighborhood.nodeIds.has(node.id()) ? 'focus-neighbor' : neighborhood.contextIds.has(node.id()) ? 'focus-context' : 'focus-dim');
    });
    cy.edges().filter(edge => !neighborhood.edgeIds.has(edge.id())).addClass('focus-dim');
  });
}
