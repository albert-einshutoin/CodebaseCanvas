import { createHash } from 'node:crypto';
import { cpSync, lstatSync, readFileSync, statSync } from 'node:fs';
import { extname, isAbsolute, join } from 'node:path';

export function requireSuccess(label: string, status: number | null): void {
  if (status !== 0) throw new Error(`${label} exited ${status ?? 'without an exit code'}`);
}

export function binaryFromBuild(output: string): string {
  const artifacts = output.split('\n').flatMap(line => {
    try { return [JSON.parse(line) as { reason?: string; target?: { name?: string; kind?: string[] }; executable?: string }]; }
    catch { return []; }
  });
  const binary = artifacts.find(item => item.reason === 'compiler-artifact' && item.target?.name === 'codebasecanvas'
    && item.target.kind?.includes('bin') && item.executable)?.executable;
  if (!binary || !isAbsolute(binary) || !statSync(binary).isFile()) throw new Error('No current binary in Cargo build output');
  return binary;
}

export function copyFixture(fixture: string, destination: string): void {
  const filter = (source: string) => {
    const entry = lstatSync(source);
    return entry.isDirectory() || (entry.isFile() && ['.ts', '.tsx', '.prisma'].includes(extname(source)));
  };
  cpSync(join(fixture, 'src'), join(destination, 'src'), { recursive: true, filter });
  cpSync(join(fixture, 'prisma'), join(destination, 'prisma'), { recursive: true, filter });
  cpSync(join(fixture, 'tsconfig.json'), join(destination, 'tsconfig.json'));
}

export function readGeneratedGraph(path: string) {
  let file;
  try { file = lstatSync(path); }
  catch { throw new Error(`Generated graph is missing: ${path}`); }
  if (!file.isFile()) throw new Error(`Generated graph is not a regular file: ${path}`);
  const bytes = readFileSync(path);
  let graph: unknown;
  try { graph = JSON.parse(bytes.toString('utf8')); }
  catch { throw new Error(`Generated graph is not valid JSON: ${path}`); }
  if (!graph || typeof graph !== 'object' || !('nodes' in graph) || !Array.isArray(graph.nodes)
    || !('edges' in graph) || !Array.isArray(graph.edges) || !('diagnostics' in graph) || !Array.isArray(graph.diagnostics)
    || !('metadata' in graph) || !graph.metadata || typeof graph.metadata !== 'object'
    || !('analyzedAt' in graph.metadata) || typeof graph.metadata.analyzedAt !== 'string') {
    throw new Error(`Generated graph lacks required summary fields: ${path}`);
  }
  return { bytes, graph: graph as { nodes: unknown[]; edges: unknown[]; diagnostics: unknown[]; metadata: { analyzedAt: string } },
    sha256: createHash('sha256').update(bytes).digest('hex') };
}
