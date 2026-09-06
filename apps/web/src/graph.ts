import { z } from 'zod';

const integer = z.number().int().min(0).max(Number.MAX_SAFE_INTEGER);
const scalarString = z.string().refine(s => Array.from(s).every(c => c.length !== 1 || c.charCodeAt(0) < 0xd800 || c.charCodeAt(0) > 0xdfff), 'Invalid Unicode scalar string');
const text = scalarString.min(1);
type JsonValue = z.infer<ReturnType<typeof z.json>>;
// Validate in place: z.record would silently discard an own __proto__ key.
const metadataSchema = z.custom<Record<string, JsonValue>>((value: unknown) => {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return false;
  const pending: [unknown, number][] = [[value, -1]];
  while (pending.length) {
    const [v, depth] = pending.pop()!;
    if (depth > 32) return false;
    if (v === null || typeof v === 'boolean') continue;
    if (typeof v === 'number') { if (!Number.isFinite(v) || (Number.isInteger(v) && !Number.isSafeInteger(v))) return false; continue; }
    if (typeof v === 'string') { if (!scalarString.safeParse(v).success) return false; continue; }
    if (Array.isArray(v)) { for (const item of v) pending.push([item, depth + 1]); continue; }
    if (typeof v !== 'object' || (Object.getPrototypeOf(v) !== Object.prototype && Object.getPrototypeOf(v) !== null)) return false;
    for (const [key, item] of Object.entries(v)) {
      if (!scalarString.safeParse(key).success) return false;
      pending.push([item, depth + 1]);
    }
  }
  return true;
}, 'Expected JSON metadata with valid Unicode and maximum value depth 32');
export function isRepositoryPath(path: string): boolean {
  return path.length > 0 && !/[\\:\u0000-\u001f\u007f-\u009f]/u.test(path)
    && path.split('/').every(part => part !== '' && part !== '.' && part !== '..');
}
const file = text.refine(isRepositoryPath, 'Expected a normalized repository-relative path');
const position = { file: file.optional(), line: integer.min(1).optional(), endLine: integer.min(1).optional() };
function validPosition(p: {file?: string; line?: number; endLine?: number}): boolean {
  return (p.line === undefined || p.file !== undefined)
    && (p.endLine === undefined || (p.line !== undefined && p.endLine >= p.line));
}
function timestamp(value: string): boolean {
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/.test(value)) return false;
  const date = new Date(value);
  return Number.isFinite(date.valueOf()) && date.toISOString() === value.replace('Z', '.000Z');
}
export const EvidenceSchema = z.strictObject({
  source: z.enum(['ast', 'nestjs', 'prisma', 'resolver']),
  ...position, file,
  confidence: z.enum(['confirmed', 'best_effort']),
}).refine(validPosition, 'Invalid source range');
const nodeKinds = ['module', 'controller', 'service', 'repository', 'class', 'interface', 'method', 'endpoint', 'database_model', 'external_dependency'] as const;
const edgeKinds = ['contains', 'imports', 'injects', 'calls', 'exposes', 'reads', 'writes', 'depends_on'] as const;
export const GraphNodeSchema = z.strictObject({
  id: text, kind: z.enum(nodeKinds), name: text, qualifiedName: text.optional(), ...position,
  parentId: text.optional(), evidence: z.array(EvidenceSchema).min(1),
  metadata: metadataSchema.optional(),
}).refine(validPosition, 'Invalid source range');
export const GraphEdgeSchema = z.strictObject({
  id: text, from: text, to: text, kind: z.enum(edgeKinds), evidence: z.array(EvidenceSchema).min(1),
  metadata: metadataSchema.optional(),
});
export const DiagnosticSchema = z.strictObject({
  code: text, severity: z.enum(['info', 'warning', 'error']), message: text,
  file: file.optional(), line: integer.min(1).optional(), relatedNodeId: text.optional(), skippedCount: integer.min(1).optional(),
}).refine(validPosition, 'Invalid diagnostic location');
const WireGraphSchema = z.strictObject({
  schemaVersion: z.literal('0.1'),
  metadata: z.strictObject({
    analyzerVersion: text, analyzedAt: text.refine(timestamp, 'Expected a UTC second-resolution timestamp'), rootName: text.optional(),
    callAnalysis: z.strictObject({
      scope: z.literal('parsed_named_class_methods'), mode: z.literal('same_class_only'),
      examinedCalls: integer, emittedCalls: integer, skippedCalls: integer,
    }),
  }),
  nodes: z.array(GraphNodeSchema), edges: z.array(GraphEdgeSchema), diagnostics: z.array(DiagnosticSchema),
});
export type GraphNode = z.infer<typeof GraphNodeSchema>;
export type GraphEdge = z.infer<typeof GraphEdgeSchema>;
export type Evidence = z.infer<typeof EvidenceSchema>;
export type Diagnostic = z.infer<typeof DiagnosticSchema>;
export type SystemGraph = z.infer<typeof WireGraphSchema>;
export type GraphMetadata = SystemGraph['metadata'];

/** Components use lowercase UTF-8 hex; delimiters and nested IDs cannot collide. */
export function canonicalId(tag: string, ...parts: string[]): string {
  return `${tag}:${parts.map(part => [...new TextEncoder().encode(part)].map(b => b.toString(16).padStart(2, '0')).join('')).join(':')}`;
}
function idParts(id: string): [string, string[]] | undefined {
  const [tag, ...hex] = id.split(':');
  if (!tag || !hex.length || hex.some(p => !/^(?:[0-9a-f]{2})+$/.test(p))) return;
  try {
    return [tag, hex.map(p => new TextDecoder('utf-8', {fatal: true, ignoreBOM: true}).decode(Uint8Array.from(p.match(/../g)!, b => parseInt(b, 16))))];
  } catch { return; }
}
const classLike = (kind: string) => ['module', 'controller', 'service', 'repository', 'class'].includes(kind);
const declaration = (kind: string) => classLike(kind) || kind === 'interface' || kind === 'method';
const route = (path: string) => path.startsWith('/') && (path === '/' || (!path.endsWith('/') && path.slice(1).split('/').every(p => p !== '' && p !== '.' && p !== '..')))
  && !/[\\?#\s\u0085\u0000-\u001f\u007f]/u.test(path);

function semanticError(g: SystemGraph): string | undefined {
  const nodes = new Map(g.nodes.map(n => [n.id, n]));
  if (nodes.size !== g.nodes.length) return 'Duplicate node ID';
  const edgeIds = new Set<string>();
  const owners = new Map<string, string[]>(), members = new Map<string, string[]>(), exposing = new Map<string, string[]>(), handlers = new Map<string, string[]>();
  const add = (map: Map<string, string[]>, key: string, value: string) => {
    const values = map.get(key);
    if (values) values.push(value); else map.set(key, [value]);
  };
  for (const e of g.edges) {
    const a = nodes.get(e.from), b = nodes.get(e.to);
    if (!a || !b) return `Dangling edge: ${e.id}`;
    if (edgeIds.has(e.id) || e.id !== canonicalId('edge', e.from, e.kind, e.to)) return `Invalid or duplicate edge ID: ${e.id}`;
    edgeIds.add(e.id);
    let valid = false;
    switch (e.kind) {
      case 'contains':
        if (a.kind === 'module' && classLike(b.kind) && b.kind !== 'module') { add(members, b.id, a.id); valid = true; }
        else if (classLike(a.kind) && b.kind === 'method') { add(owners, b.id, a.id); valid = true; }
        break;
      case 'imports': valid = declaration(a.kind) && (declaration(b.kind) || b.kind === 'external_dependency'); break;
      case 'injects': valid = classLike(a.kind) && (classLike(b.kind) || b.kind === 'external_dependency') && e.metadata?.semantics === 'requested_token'; break;
      case 'calls': valid = a.kind === 'method' && b.kind === 'method' && a.parentId !== undefined && a.parentId === b.parentId; break;
      case 'exposes': valid = a.kind === 'controller' && b.kind === 'endpoint'; if (valid) add(exposing, b.id, a.id); break;
      case 'depends_on':
        valid = a.kind === 'module' && b.kind === 'module';
        if (a.kind === 'endpoint' && b.kind === 'method') { add(handlers, a.id, b.id); valid = true; }
        break;
    }
    if (!valid) return `Unsupported edge semantics: ${e.id}`;
  }
  for (const n of g.nodes) {
    const identity = idParts(n.id);
    if (!identity) return `Invalid node ID: ${n.id}`;
    const [tag, p] = identity;
    if (classLike(n.kind) || n.kind === 'interface' || n.kind === 'database_model') {
      if (tag !== (classLike(n.kind) ? 'class' : n.kind) || p.length < 2 || p[0] !== n.file || p.at(-1) !== n.name || (n.kind === 'database_model' && p.length !== 2)) return `Declaration identity mismatch: ${n.id}`;
    } else if (n.kind === 'method') {
      if (tag !== 'method' || p.length !== 3 || p[0] !== n.parentId || !['instance','static'].includes(p[1]) || p[2] !== n.name) return `Method identity mismatch: ${n.id}`;
    } else if (n.kind === 'external_dependency') {
      if (tag !== 'external' || p.length !== 2 || p[1] !== n.name) return `External identity mismatch: ${n.id}`;
    } else {
      const m = n.metadata;
      if (!m || typeof m.httpMethod !== 'string' || !['GET','POST','PUT','PATCH','DELETE'].includes(m.httpMethod) || typeof m.path !== 'string' || !route(m.path) || typeof m.controllerMethodId !== 'string'
        || n.id !== canonicalId('endpoint', m.httpMethod, m.path, m.controllerMethodId)) return `Endpoint metadata/identity mismatch: ${n.id}`;
      const controllers = exposing.get(n.id) ?? [], methods = handlers.get(n.id) ?? [];
      if (controllers.length !== 1 || methods.length !== 1 || methods[0] !== m.controllerMethodId || nodes.get(methods[0])?.parentId !== controllers[0] || n.parentId !== controllers[0]) return `Endpoint handler mismatch: ${n.id}`;
    }
    const parents = n.kind === 'method' ? owners.get(n.id) ?? [] : members.get(n.id) ?? [];
    if (n.kind === 'method' && (parents.length !== 1 || n.parentId !== parents[0])) return `Invalid lexical owner: ${n.id}`;
    if (classLike(n.kind) && n.parentId !== (parents.length === 1 ? parents[0] : undefined)) return `Invalid module membership parent: ${n.id}`;
    if (!classLike(n.kind) && n.kind !== 'method' && n.kind !== 'endpoint' && n.parentId !== undefined) return `Unexpected parent: ${n.id}`;
    const seen = new Set([n.id]);
    let parent = n.parentId;
    while (parent !== undefined) {
      if (seen.has(parent) || !nodes.has(parent)) return `Cyclic or dangling parent: ${n.id}`;
      seen.add(parent); parent = nodes.get(parent)!.parentId;
    }
  }
  let skipped = 0;
  const groups = new Set<string>();
  for (const d of g.diagnostics) {
    if (d.relatedNodeId !== undefined && !nodes.has(d.relatedNodeId)) return 'Dangling diagnostic reference';
    if (d.skippedCount !== undefined) {
      const group = canonicalId('diagnostic', d.relatedNodeId ?? '', d.code);
      if (!d.file || !d.line || !d.relatedNodeId || nodes.get(d.relatedNodeId)?.kind !== 'method' || groups.has(group)) return 'Invalid skipped-call diagnostic';
      groups.add(group); skipped += d.skippedCount;
      if (!Number.isSafeInteger(skipped)) return 'Skipped count overflow';
    }
  }
  const c = g.metadata.callAnalysis;
  if (!Number.isSafeInteger(c.emittedCalls + c.skippedCalls) || c.examinedCalls !== c.emittedCalls + c.skippedCalls || skipped !== c.skippedCalls
    || c.emittedCalls < g.edges.filter(e => e.kind === 'calls').length || (c.emittedCalls > 0 && !g.edges.some(e => e.kind === 'calls'))) return 'Call coverage mismatch';
}
export const SystemGraphSchema = WireGraphSchema.superRefine((graph, ctx) => {
  const error = semanticError(graph);
  if (error) ctx.addIssue({ code: 'custom', message: error });
});
