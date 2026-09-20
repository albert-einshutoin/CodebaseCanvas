import { useEffect, useRef, useState } from 'react';
import type { SystemGraph } from './graph';
import { generateContextMarkdown } from './contextMarkdown';
import { createCopyController, type CopyState } from './contextClipboard';

/** Parent keys this control by import generation and selection; no Canvas actions are dispatched. */
export function CopyContext({ graph, nodeId }: { graph: SystemGraph; nodeId: string }) {
  const [state, setState] = useState<CopyState>({ kind: 'idle' });
  const controller = useRef<ReturnType<typeof createCopyController> | null>(null);
  const name = graph.nodes.find(n => n.id === nodeId)?.name ?? 'component';
  useEffect(() => () => { controller.current?.dispose(); }, []);
  function copy() {
    if (!controller.current) controller.current = createCopyController(
      () => generateContextMarkdown(graph, nodeId).markdown,
      typeof navigator !== 'undefined' && navigator.clipboard?.writeText
        ? text => navigator.clipboard.writeText(text) : undefined,
      setState,
    );
    void controller.current.copy();
  }
  return <section className="copy-context" aria-label="Copy component context">
    <button type="button" onClick={copy} disabled={state.kind === 'pending'} aria-label={`Copy Context for ${name}`}>Copy Context</button>
    <p role="status" aria-live="polite">
      {state.kind === 'pending' && `Copying Context for ${name}…`}
      {state.kind === 'copied' && `Copied Context for ${name}.`}
      {state.kind === 'busy' && 'Another clipboard write is still pending. Wait, then copy again.'}
      {state.kind === 'generationError' && 'Context generation failed. Nothing was copied.'}
      {state.kind === 'copyError' && `Copy failed for ${name}. Select the generated Markdown below and copy it manually.`}
    </p>
    {state.kind === 'copyError' && <details open><summary>Generated Markdown for manual copy</summary>
      <label>Context for {name}<textarea readOnly value={state.markdown} rows={14} onFocus={event => event.currentTarget.select()} /></label>
    </details>}
  </section>;
}
