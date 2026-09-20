import { nodeIdentityDetails, type Diagnostic, type Evidence, type GraphNode, type SystemGraph } from './graph';
import { CopyContext } from './CopyContext';
import { navigationLabel } from './canvasView';
import type { NodeDetailsModel } from './nodeDetails';

export function sourcePosition(value: { file?: string; line?: number; endLine?: number }) {
  if (!value.file) return 'Source location not recorded';
  return `${value.file}${value.line === undefined ? '' : `:${value.line}${value.endLine === undefined ? '' : `–${value.endLine}`}`}`;
}
function EvidenceList({ items, label }: { items: Evidence[]; label: string }) {
  return <details><summary>{label} ({items.length})</summary><ul>{items.map((e, i) => <li key={i}>
    <strong>{e.confidence}</strong> — {e.source}<br />{sourcePosition(e)}
  </li>)}</ul></details>;
}
function Diagnostics({ items }: { items: Diagnostic[] }) {
  return items.length ? <ul>{items.map((d, i) => <li key={i}>
    <strong>{d.severity} / {d.code}</strong><p>{d.message}</p>
    <p>{sourcePosition(d)}</p>{d.relatedNodeId && <p>relatedNodeId: <code>{d.relatedNodeId}</code></p>}
    {d.skippedCount !== undefined && <p>Skipped call sites: {d.skippedCount}</p>}
  </li>)}</ul> : <p>No diagnostics recorded in this snapshot for this scope.</p>;
}

type Props = { details: NodeDetailsModel | undefined; graph: SystemGraph; visibleIds: ReadonlySet<string>; onNavigate: (id: string) => void; onClose: () => void };
export function NodeDetails({ details: d, graph, visibleIds, onNavigate, onClose }: Props) {
  function target(n: GraphNode) {
    const kind = nodeIdentityDetails(n).methodKind;
    return <button type="button" onClick={() => onNavigate(n.id)}>
      {navigationLabel(n, visibleIds)}: {n.name}{kind ? ` (${kind})` : ''} [{n.kind}]
      <small>{n.id}</small>
    </button>;
  }
  function targets(nodes: GraphNode[]) {
    return nodes.length ? <ul>{nodes.map(n => <li key={n.id}>{target(n)}</li>)}</ul> : <p>Not recorded in this snapshot.</p>;
  }
  function relations(title: string, rows: NodeDetailsModel['incoming']) {
    const kinds = [...new Set(rows.map(r => r.edge.kind))];
    return <section><h3>{title}</h3>{!rows.length && <p>Not recorded in this snapshot.</p>}{kinds.map(kind =>
      <details key={kind} open><summary>{kind} ({rows.filter(r => r.edge.kind === kind).length})</summary>
        <ul>{rows.filter(r => r.edge.kind === kind).map(r => <li key={r.edge.id}>
          <strong>{r.label}</strong>{target(r.other)}
          <details><summary>Canonical edge direction / ID</summary><p><code>{r.edge.from}</code> → {r.edge.kind} → <code>{r.edge.to}</code></p><p>{r.edge.id}</p></details>
          <EvidenceList items={r.edge.evidence} label="Edge evidence" />
        </li>)}</ul>
      </details>)}</section>;
  }
  return <aside className="node-details" aria-label="Node details">
    {!d ? <p>Select a node to inspect its definition, direct relationships and recorded evidence.</p> : <>
      <button type="button" onClick={onClose}>Close details / clear selection</button>
      <CopyContext graph={graph} nodeId={d.node.id} />
      <h2>{d.node.name}</h2><p>Kind: {d.node.kind}{d.methodKind && ` (${d.methodKind})`}</p>
      {d.node.qualifiedName && <p>{d.node.qualifiedName}</p>}
      <p>{sourcePosition(d.node)}</p><details><summary>Canonical node ID</summary><code>{d.node.id}</code></details>
      <p>Analyzed at (UTC): <time dateTime={graph.metadata.analyzedAt}>{graph.metadata.analyzedAt}</time></p>
      <p>Analyzer: {graph.metadata.analyzerVersion}</p>
      <p>This is a snapshot. After source changes, re-run analysis and choose the graph file again.</p>
      <EvidenceList items={d.node.evidence} label="Node evidence" />
      <p>confirmed describes a static fact, not runtime behavior. best_effort evidence remains separate.</p>
      {['module', 'controller', 'service', 'repository', 'class'].includes(d.node.kind) && <>
        <h3>Module memberships</h3>{targets(d.memberships)}
        <h3>Direct methods ({d.methods.length})</h3>{targets(d.methods)}
      </>}
      {d.owner && <section><h3>Owning declaration</h3>{target(d.owner)}</section>}
      {d.node.kind === 'endpoint' && <section><h3>Declared endpoint</h3>
        <p>{typeof d.node.metadata?.httpMethod === 'string' ? d.node.metadata.httpMethod : ''} {typeof d.node.metadata?.path === 'string' ? d.node.metadata.path : ''}</p>
        <h4>Controller</h4>{d.controller && target(d.controller)}<h4>Handler</h4>{d.handler && target(d.handler)}
      </section>}
      {d.node.kind === 'database_model' && <section><h3>Model: {d.node.name}</h3><h4>Recorded fields</h4>
        {d.fields.length ? <ul>{d.fields.map((f, i) => <li key={i}>{f.name}{f.type && `: ${f.type}`}</li>)}</ul> : <p>No supported fields recorded in this snapshot.</p>}
      </section>}
      {d.specifier && <section><h3>External symbol</h3><p>Import specifier: {d.specifier}</p><p>Symbol: {d.node.name}</p><p>External identity does not prove a runtime class or resolved implementation.</p></section>}
      <p>Requests token / Requested by describe requested tokens, not resolved implementations or runtime calls.</p>
      {relations('Outgoing relationships', d.outgoing)}{relations('Incoming relationships', d.incoming)}
      <section><h3>Related unknown / diagnostics</h3>
        <h4>Selected node ({d.diagnostics.length} diagnostics)</h4><Diagnostics items={d.diagnostics} />
        {d.relatedDiagnostics.map(group => <details key={group.node.id}><summary>{d.handler ? 'Declared handler' : 'Direct method'}: {group.node.name} ({group.diagnostics.length} diagnostics)</summary>
          {target(group.node)}<Diagnostics items={group.diagnostics} />
        </details>)}
        <p>Selected method scope skipped call sites: {d.skipped} (selected method, direct methods, or declared handler only).</p>
        <details><summary>Same source file notices — not attributed to this node ({d.fileDiagnostics.length})</summary><Diagnostics items={d.fileDiagnostics} /></details>
        <p>Zero recorded diagnostics or skipped sites does not mean complete analysis.</p>
      </section>
      <section><h3>Graph-wide recorded call analysis</h3>
        <p>Scope: {graph.metadata.callAnalysis.scope}; mode: {graph.metadata.callAnalysis.mode}</p>
        <p>Examined: {graph.metadata.callAnalysis.examinedCalls} / emitted: {graph.metadata.callAnalysis.emittedCalls} / skipped: {graph.metadata.callAnalysis.skippedCalls}</p>
        <p>These are graph-wide recorded call sites, not this node's coverage or the number of edges. Per-node examined counts are not recorded.</p>
      </section>
    </>}
  </aside>;
}
