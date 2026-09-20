import type { SystemGraph } from './graph';
import { sortedDiagnostics } from './nodeDetails';

/** Full validated snapshot only; no Canvas projection or local details scope. */
export function graphDiagnostics(graph: SystemGraph) {
  const counts = { info: 0, warning: 0, error: 0 };
  for (const diagnostic of graph.diagnostics) counts[diagnostic.severity]++;
  // Sorted occurrence index is a display key, not a wire ID. Exact duplicates remain rows.
  const rows = sortedDiagnostics(graph.diagnostics).map((diagnostic, key) => ({ key, diagnostic }));
  const codes = [...new Set(graph.diagnostics.map(d => d.code))].sort();
  return { counts, rows, codes };
}
export type DiagnosticFilters = { severity: string; code: string; skippedOnly: boolean };
export function diagnosticPage(rows: ReturnType<typeof graphDiagnostics>['rows'], filters: DiagnosticFilters, limit: number) {
  const matches = rows.filter(({ diagnostic: d }) => (!filters.severity || d.severity === filters.severity)
    && (!filters.code || d.code === filters.code) && (!filters.skippedOnly || d.skippedCount !== undefined));
  return { total: matches.length, rows: matches.slice(0, limit) };
}
