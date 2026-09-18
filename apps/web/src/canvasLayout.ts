import cytoscape from 'cytoscape';
import fcose from 'cytoscape-fcose';

cytoscape.use(fcose);

export const fitPadding = 40;

// animate:false finishes synchronously; there are no deferred fit/selection callbacks.
export function layoutCanvas(cy: cytoscape.Core, initial: boolean) {
  if (!cy.nodes().length) return;
  const options: fcose.FcoseLayoutOptions = {
    name: 'fcose', quality: 'proof', animate: false, fit: false,
    randomize: initial, nodeDimensionsIncludeLabels: true,
    packComponents: false, tile: true,
    idealEdgeLength: 100, nodeRepulsion: 8000,
    tilingPaddingVertical: 35, tilingPaddingHorizontal: 35,
    numIter: 1000,
  };
  cy.layout(options).run();
  if (initial) cy.fit(cy.elements(), fitPadding);
}

export function updateCanvasElements(cy: cytoscape.Core, elements: cytoscape.ElementDefinition[]) {
  const ids = new Set(elements.map(element => element.data.id));
  const added = elements.filter(element => !cy.getElementById(element.data.id!).length);
  // Read the owner position before it changes from a leaf to a compound node.
  const positions = new Map(cy.nodes().map(node => [node.id(), { ...node.position() }]));
  cy.batch(() => {
    cy.elements().filter(element => !ids.has(element.id())).remove();
    cy.add(added.map((element, index) => ({
      ...element,
      position: element.group === 'nodes' && element.data.parent
        ? { x: (positions.get(element.data.parent)?.x ?? 0) + 30 * (index + 1), y: (positions.get(element.data.parent)?.y ?? 0) + 50 }
        : undefined,
    })));
  });
}
