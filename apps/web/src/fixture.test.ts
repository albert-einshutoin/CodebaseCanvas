import { expect, it } from 'vitest';
import expected from '../../../examples/nestjs-sample/expected-graph.json';
import { SystemGraphSchema } from './graph';

const graph = SystemGraphSchema.parse(expected);
const sources = import.meta.glob<string>('../../../examples/nestjs-sample/{src/**/*.ts,prisma/*.prisma}', {query: '?raw', import: 'default', eager: true});

it('accepts the hand-authored fixture with the production validator', () => {
  expect(graph).toEqual(expected);
  expect([graph.nodes.length, graph.edges.length, graph.diagnostics.length]).toEqual([57, 90, 15]);
  expect(graph.metadata.callAnalysis).toMatchObject({examinedCalls: 9, emittedCalls: 2, skippedCalls: 7});
});

it('keeps reviewed identity, membership and token boundaries', () => {
  const service = graph.nodes.find(n => n.name === 'UsersService')!;
  expect(service.parentId).toBeUndefined();
  expect(graph.edges.filter(e => e.kind === 'contains' && e.to === service.id)).toHaveLength(2);
  const finds = graph.nodes.filter(n => n.kind === 'method' && n.name === 'find' && n.parentId === service.id);
  expect(new Set(finds.map(n => n.id)).size).toBe(2);
  const endpoints = graph.nodes.filter(n => n.kind === 'endpoint' && n.metadata?.path === '/users');
  expect(new Set(endpoints.map(n => n.metadata?.controllerMethodId)).size).toBe(2);
  expect(graph.nodes.filter(n => n.name === 'Same')).toHaveLength(2);
  expect(graph.nodes.filter(n => n.kind === 'external_dependency' && n.name === 'map')).toHaveLength(2);
  const override = graph.nodes.find(n => n.name === 'OverrideController')!;
  expect(graph.edges.some(e => e.kind === 'injects' && e.from === override.id && e.to === service.id)).toBe(true);
  const calls = graph.edges.filter(e => e.kind === 'calls');
  expect(calls).toHaveLength(2);
  const byId = new Map(graph.nodes.map(n => [n.id, n]));
  expect(calls.map(e => [byId.get(e.from)?.name, byId.get(e.to)?.name]).sort()).toEqual([['choose', 'find'], ['login', 'normalize']]);
  expect(calls.every(e => byId.get(e.from)?.parentId === byId.get(e.to)?.parentId)).toBe(true);
});

it('anchors every source location within the fixture', () => {
  const locations = [...graph.nodes, ...graph.diagnostics, ...graph.nodes.flatMap(n => n.evidence), ...graph.edges.flatMap(e => e.evidence)];
  for (const item of locations) {
    if (!item.file) continue;
    const source = sources[`../../../examples/nestjs-sample/${item.file}`];
    expect(source, item.file).toBeDefined();
    const lines = source.split('\n');
    expect(item.line, item.file).toBeGreaterThan(0);
    expect(item.line, item.file).toBeLessThan(lines.length);
    expect(lines[item.line! - 1].trim(), `${item.file}:${item.line}`).not.toBe('');
  }
});
