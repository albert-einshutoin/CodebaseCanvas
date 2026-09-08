import { describe, expect, it } from 'vitest';
import cases from '../../../contracts/cases.json';
import { SystemGraphSchema, canonicalId, parseGraphText, readGraphFile } from './graph';

describe('shared wire contract', () => {
  it('accepts and preserves the complete graph', () => {
    expect(SystemGraphSchema.parse(cases.graph)).toEqual(cases.graph);
  });
  for (const item of cases.valid) {
    it(`accepts ${item.name}`, () => expect(SystemGraphSchema.parse(item.graph)).toEqual(item.graph));
  }
  for (const item of cases.invalid) {
    it(`rejects ${item.name}`, () => {
      const graph = structuredClone(cases.graph);
      if (!item.path.length) {
        expect(SystemGraphSchema.safeParse(item.value).success).toBe(false);
        return;
      }
      let target: any = graph;
      for (const key of item.path.slice(0, -1)) target = target[key];
      target[item.path.at(-1)!] = item.value;
      expect(SystemGraphSchema.safeParse(graph).success).toBe(false);
    });
  }
  for (const raw of cases.validJson) {
    it('preserves open metadata keys and values', () => {
      const graph = JSON.parse(raw);
      const parsed = SystemGraphSchema.parse(graph);
      expect(parsed).toEqual(graph);
      expect(JSON.stringify(parsed.nodes[0].metadata)).toBe(JSON.stringify(graph.nodes[0].metadata));
      expect(Object.prototype.hasOwnProperty.call(parsed.nodes[0].metadata, '__proto__')).toBe(true);
    });
  }
  for (const raw of cases.invalidJson) {
    it('rejects invalid raw JSON values', () => expect(SystemGraphSchema.safeParse(JSON.parse(raw)).success).toBe(false));
  }
  for (const vector of cases.ids) {
    it(`encodes ${vector.expected}`, () => {
      expect(canonicalId(vector.tag, ...vector.parts)).toBe(vector.expected);
    });
  }
});

describe('local graph import', () => {
  it('reads and validates a graph file in memory', async () => {
    const result = await readGraphFile({ text: async () => JSON.stringify(cases.graph) });

    expect(result).toEqual({ ok: true, graph: cases.graph });
  });

  it('reports malformed JSON without exposing a parser dump', () => {
    expect(parseGraphText('{')).toEqual({
      ok: false,
      message: 'Unable to open this graph.',
      details: ['The selected file is not valid JSON.'],
    });
  });

  it('reports schema and semantic validation errors', () => {
    const future = structuredClone(cases.graph);
    future.schemaVersion = '0.2';
    expect(parseGraphText(JSON.stringify(future))).toMatchObject({
      ok: false,
      details: ['Unsupported schema version (expected 0.1).'],
    });

    const dangling = structuredClone(cases.graph);
    dangling.edges[0].to = 'class:missing';
    expect(parseGraphText(JSON.stringify(dangling))).toMatchObject({
      ok: false,
      details: ['Dangling edge: ' + dangling.edges[0].id],
    });
  });
});
