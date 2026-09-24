import { useMemo, useRef, useState, type ChangeEvent } from 'react';
import { createRoot } from 'react-dom/client';
import './style.css';
import { AnalysisPanel } from './AnalysisPanel';
import { NodeDetails } from './NodeDetailsPanel';
import { nodeDetails } from './nodeDetails';
import { CanvasControls } from './CanvasControls';
import { projectView, directNeighborhood } from './canvasView';
import { GraphCanvas } from './Canvas';
import { directMethods, initialCanvasState, transitionCanvasState, type CanvasAction } from './canvasState';
import { readGraphFile, type SystemGraph } from './graph';

type ImportState =
  | { kind: 'idle' }
  | { kind: 'loading'; fileName: string }
  | { kind: 'loaded'; fileName: string; graph: SystemGraph }
  | { kind: 'error'; fileName: string; message: string; details: string[] };

function App() {
  const [state, setState] = useState<ImportState>({ kind: 'idle' });
  const [canvasState, setCanvasState] = useState(() => initialCanvasState(0));
  const importSequence = useRef(0);

  async function handleFileChange(event: ChangeEvent<HTMLInputElement>) {
    const input = event.currentTarget;
    const file = input.files?.[0];
    if (!file) return;

    const sequence = ++importSequence.current;
    setState({ kind: 'loading', fileName: file.name });
    setCanvasState(initialCanvasState(sequence));
    input.value = '';
    const result = await readGraphFile(file);
    if (sequence !== importSequence.current) return;
    setState(result.ok
      ? { kind: 'loaded', fileName: file.name, graph: result.graph }
      : { kind: 'error', fileName: file.name, message: result.message, details: result.details });
  }

  const graph = state.kind === 'loaded' ? state.graph : undefined;
  const view = useMemo(() => graph ? projectView(graph, canvasState.enabledKinds, canvasState.expandedOwnerId) : undefined, [graph, canvasState.enabledKinds, canvasState.expandedOwnerId]);
  const neighborhood = useMemo(() => graph && view ? directNeighborhood(graph, view.visibleIds, canvasState.neighborhoodAnchorId) : null, [graph, view, canvasState.neighborhoodAnchorId]);
  const details = useMemo(() => graph ? nodeDetails(graph, canvasState.selectedNodeId) : undefined, [graph, canvasState.selectedNodeId]);

  const selectedNode = state.kind === 'loaded'
    ? state.graph.nodes.find(node => node.id === canvasState.selectedNodeId)
    : undefined;

  const methods = state.kind === 'loaded' ? directMethods(state.graph, canvasState.selectedNodeId) : [];
  const expandedOwner = state.kind === 'loaded' ? state.graph.nodes.find(node => node.id === canvasState.expandedOwnerId) : undefined;
  function changeCanvas(action: CanvasAction) {
    if (state.kind === 'loaded') setCanvasState(current => transitionCanvasState(state.graph, current, action));
  }

  return (
    <main className={state.kind === 'loaded' ? 'wide' : undefined}>
      <p>開発プレビュー</p>
      <h1>CodebaseCanvas</h1>
      <h2>Open a local CodebaseCanvas graph</h2>
      <ol>
        <li>
          Generate a graph locally with the CLI:
          <code>codebasecanvas analyze &lt;repo&gt;</code>
          → <code>&lt;repo&gt;/.codebasecanvas/graph.json</code>.
          <a href="https://github.com/albert-einshutoin/CodebaseCanvas#セットアップ"> See setup instructions.</a>
        </li>
        <li>
          <label htmlFor="graph-file">Choose graph.json</label>
          <input id="graph-file" type="file" accept=".json,application/json" onChange={event => void handleFileChange(event)} />
        </li>
      </ol>
      <p>Your file stays in this browser and is not uploaded.</p>
      <p>This graph is a snapshot: re-run the CLI and choose the file again after source changes.</p>

      {state.kind === 'loading' && <p role="status">Reading {state.fileName}…</p>}
      {state.kind === 'error' && (
        <section className="message error" role="alert">
          <h2>{state.message}</h2>
          <ul>{state.details.map(detail => <li key={detail}>{detail}</li>)}</ul>
        </section>
      )}
      {state.kind === 'loaded' && (
        <>
          <section className="message success" role="status">
            <h2>Graph loaded and validated</h2>
            <p>{state.fileName} is kept in browser memory.</p>
            <dl>
              <dt>Schema</dt><dd>{state.graph.schemaVersion}</dd>
            </dl>
          </section>
          <AnalysisPanel key={canvasState.generation} graph={state.graph} generation={canvasState.generation} visibleIds={view!.visibleIds}
            onNavigate={(id, generation) => changeCanvas({ type: 'navigate', id, generation })} />
          <CanvasControls graph={state.graph} state={canvasState} view={view!} neighborhood={neighborhood} onChange={changeCanvas} />
          <p className="selection-status" aria-live="polite">
            {selectedNode ? `Selected: [${selectedNode.kind}] ${selectedNode.name}` : 'Select a node to see its kind and name.'}
          </p>
          <div className="method-controls">
            <span>{methods.length} directly owned methods in this snapshot</span>
            <button type="button" disabled={!methods.length} aria-expanded={!!selectedNode && selectedNode.id === canvasState.expandedOwnerId}
              onClick={() => selectedNode && changeCanvas({ type: 'show', id: selectedNode.id })}>Show methods</button>
            {expandedOwner && <span className="expanded-owner">{expandedOwner.kind !== 'method' && canvasState.enabledKinds.has(expandedOwner.kind) ? 'Methods shown' : 'Expansion target hidden by kind filter'}: {expandedOwner.name}
              <button type="button" aria-expanded={true} aria-label={`Hide methods of ${expandedOwner.name}`} onClick={() => changeCanvas({ type: 'hide' })}>Hide methods</button>
            </span>}
          </div>
          <p className="canvas-legend">Frames follow declared parents. Shared / outside nodes stay outside Modules. Arrows show relations, not execution order. Select a node or hover an edge to read its relation.</p>
          <div className="graph-workspace">
            <GraphCanvas view={view!} neighborhood={neighborhood} graph={state.graph} {...canvasState} onSelect={id => changeCanvas({ type: 'select', id })} />
            <NodeDetails key={JSON.stringify([canvasState.generation, canvasState.selectedNodeId])} graph={state.graph} details={details} visibleIds={view!.visibleIds}
              onClose={() => changeCanvas({ type: 'select', id: null })}
              onNavigate={id => changeCanvas({ type: 'navigate', id, generation: canvasState.generation })} />
          </div>
        </>
      )}
    </main>
  );
}

createRoot(document.getElementById('root')!).render(<App />);
