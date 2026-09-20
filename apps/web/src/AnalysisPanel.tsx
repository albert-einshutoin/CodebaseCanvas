import { useMemo, useState } from 'react';
import type { SystemGraph } from './graph';
import { candidateDescription, navigationLabel } from './canvasView';
import { sourcePosition } from './NodeDetailsPanel';
import { diagnosticPage, graphDiagnostics, type DiagnosticFilters } from './graphDiagnostics';

type Props = {
  graph: SystemGraph;
  generation: number;
  visibleIds: ReadonlySet<string>;
  onNavigate: (id: string, generation: number) => void;
};
const allFilters: DiagnosticFilters = { severity: '', code: '', skippedOnly: false };

/** The caller keys this panel by import generation; selection never resets its state. */
export function AnalysisPanel({ graph, generation, visibleIds, onNavigate }: Props) {
  const model = useMemo(() => graphDiagnostics(graph), [graph]);
  const nodes = useMemo(() => new Map(graph.nodes.map(n => [n.id, n])), [graph]);
  const [open, setOpen] = useState(false);
  const [filters, setFilters] = useState(allFilters);
  const [limit, setLimit] = useState(50);
  const page = useMemo(() => diagnosticPage(model.rows, filters, limit), [model, filters, limit]);
  const calls = graph.metadata.callAnalysis;
  function filter(next: DiagnosticFilters) { setFilters(next); setLimit(50); }

  return <section className="analysis-panel" aria-label="Analysis">
    <h2>Analysis snapshot</h2>
    <p>In file: {graph.nodes.length} nodes · {graph.edges.length} edges · {model.rows.length} diagnostics</p>
    <p>info: {model.counts.info} · warning: {model.counts.warning} · error: {model.counts.error}</p>
    {model.counts.error > 0 && <p className="error"><strong>Graph may be incomplete.</strong> Analyzer error diagnostics are recorded in this validated snapshot.</p>}
    <p>Analyzed at (UTC): <time dateTime={graph.metadata.analyzedAt}>{graph.metadata.analyzedAt}</time>.
      This is a snapshot; after source changes, re-run analysis and choose the graph again.</p>
    <p>Recorded call analysis — scope: <code>{calls.scope}</code>; mode: <code>{calls.mode}</code>.<br />
      Examined call sites: {calls.examinedCalls} · Emitted call sites: {calls.emittedCalls} · Skipped call sites: {calls.skippedCalls}</p>
    <p>These counters cover call sites in the recorded scope, not whole-repository coverage or relationship resolution.
      Emitted sites are not deduplicated calls edges. Zero calls or diagnostics does not guarantee complete analysis.</p>
    <p className="confidence-legend"><strong>confirmed</strong>: the stated static fact is confirmed. <strong>best_effort</strong>: limited heuristics or partial resolution.
      {' '}<strong>unsupported / unknown</strong>: unsupported relationships are not filled in by guessing.
      Confidence belongs to each evidence item, not to a diagnostic or a whole node; severity is shown as recorded.</p>
    <p><strong>Requests token / Requested by</strong>: injects confirms <code>requested_token</code>, not implementation selection after provider overrides, instance creation or runtime calls.</p>
    {!model.rows.length && <p>No analyzer diagnostics for this graph.</p>}
    <div className="analysis-actions">
      <button type="button" aria-expanded={open} aria-controls="diagnostic-list" onClick={() => setOpen(!open)}>{open ? 'Hide diagnostics' : 'Show diagnostics'}</button>
      {calls.skippedCalls > 0 && <button type="button" onClick={() => { filter({ ...allFilters, skippedOnly: true }); setOpen(true); }}>Show skipped-call diagnostics</button>}
    </div>
    <div id="diagnostic-list" hidden={!open}>
      <h3>Analyzer diagnostics</h3>
      <div className="diagnostic-filters">
        <label>Diagnostic severity <select value={filters.severity} onChange={e => filter({ ...filters, severity: e.currentTarget.value })}>
          <option value="">All severities</option>{(['info', 'warning', 'error'] as const).map(s => <option key={s}>{s}</option>)}
        </select></label>
        <label>Diagnostic code <select value={filters.code} onChange={e => filter({ ...filters, code: e.currentTarget.value })}>
          <option value="">All codes</option>{model.codes.map(code => <option key={code}>{code}</option>)}
        </select></label>
        <label><input type="checkbox" checked={filters.skippedOnly} onChange={e => filter({ ...filters, skippedOnly: e.currentTarget.checked })} />Skipped-call diagnostics only</label>
        <button type="button" onClick={() => filter(allFilters)}>Reset diagnostic filters</button>
      </div>
      {filters.skippedOnly && <p>Showing diagnostics with recorded skippedCount. Severity: {filters.severity || 'all'}; code: {filters.code || 'all'}. Each row identifies its method and reason; counts are call sites.</p>}
      <p role="status">{model.rows.length} total diagnostics · {page.total} match · {page.rows.length} displayed</p>
      {!!model.rows.length && !page.total && <p>No diagnostics match these filters.</p>}
      <ul className="diagnostic-rows">{page.rows.map(({ key, diagnostic: d }) => {
        const node = d.relatedNodeId === undefined ? undefined : nodes.get(d.relatedNodeId)!;
        return <li key={key}>
          <strong>{d.severity} / {d.code}</strong><p>{d.message}</p><p>{sourcePosition(d)}</p>
          {d.skippedCount !== undefined && <p>1 diagnostic · Skipped call sites: {d.skippedCount}</p>}
          {node ? <button type="button" onClick={() => onNavigate(node.id, generation)}>
            {navigationLabel(node, visibleIds)}: [{node.kind}] {node.name}
            <small>{candidateDescription(node, nodes)}</small><small>{node.id}</small>
          </button> : <p>{d.file ? 'File notice' : 'Global notice'} — no related node recorded.</p>}
        </li>;
      })}</ul>
      {page.rows.length < page.total && <button type="button" onClick={() => setLimit(current => current + 50)}>Show 50 more diagnostics</button>}
    </div>
  </section>;
}
