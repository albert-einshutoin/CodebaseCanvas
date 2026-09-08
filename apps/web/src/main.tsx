import { useRef, useState, type ChangeEvent } from 'react';
import { createRoot } from 'react-dom/client';
import './style.css';
import { readGraphFile, type SystemGraph } from './graph';

type ImportState =
  | { kind: 'idle' }
  | { kind: 'loading'; fileName: string }
  | { kind: 'loaded'; fileName: string; graph: SystemGraph }
  | { kind: 'error'; fileName: string; message: string; details: string[] };

function App() {
  const [state, setState] = useState<ImportState>({ kind: 'idle' });
  const importSequence = useRef(0);

  async function handleFileChange(event: ChangeEvent<HTMLInputElement>) {
    const input = event.currentTarget;
    const file = input.files?.[0];
    if (!file) return;

    const sequence = ++importSequence.current;
    setState({ kind: 'loading', fileName: file.name });
    input.value = '';
    const result = await readGraphFile(file);
    if (sequence !== importSequence.current) return;
    setState(result.ok
      ? { kind: 'loaded', fileName: file.name, graph: result.graph }
      : { kind: 'error', fileName: file.name, message: result.message, details: result.details });
  }

  return (
    <main>
      <p>開発プレビュー</p>
      <h1>CodebaseCanvas</h1>
      <h2>Open a local CodebaseCanvas graph</h2>
      <ol>
        <li>Generate a graph locally with the CLI (see setup instructions).</li>
        <li>
          <label htmlFor="graph-file">Choose graph.json</label>
          <input id="graph-file" type="file" accept=".json,application/json" onChange={event => void handleFileChange(event)} />
        </li>
      </ol>
      <p>Your file stays in this browser and is not uploaded.</p>
      <p>CLI graph generation is not implemented yet. This graph is a snapshot: re-run the CLI and choose the file again after source changes.</p>

      {state.kind === 'loading' && <p role="status">Reading {state.fileName}…</p>}
      {state.kind === 'error' && (
        <section className="message error" role="alert">
          <h2>{state.message}</h2>
          <ul>{state.details.map(detail => <li key={detail}>{detail}</li>)}</ul>
        </section>
      )}
      {state.kind === 'loaded' && (
        <section className="message success" role="status">
          <h2>Graph loaded and validated</h2>
          <p>{state.fileName} is kept in browser memory. Rendering is not implemented yet.</p>
          <dl>
            <dt>Schema</dt><dd>{state.graph.schemaVersion}</dd>
            <dt>Nodes</dt><dd>{state.graph.nodes.length}</dd>
            <dt>Edges</dt><dd>{state.graph.edges.length}</dd>
            <dt>Diagnostics</dt><dd>{state.graph.diagnostics.length}</dd>
          </dl>
        </section>
      )}
    </main>
  );
}

createRoot(document.getElementById('root')!).render(<App />);
