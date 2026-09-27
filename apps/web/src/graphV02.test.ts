import { describe, expect, it } from 'vitest';
import cases from '../../../contracts/v02-repository-request.json';
import { SystemGraphSchema, parseGraphText, readGraphFile } from './graph';
import { SystemGraphV02Schema } from './graphV02';

describe('shared experimental repository request contract', () => {
  for (const item of cases.valid) {
    it(`accepts ${item.name} without changing the input`, () => {
      const input = structuredClone(item.graph);
      expect(SystemGraphV02Schema.parse(input)).toEqual(item.graph);
      expect(input).toEqual(item.graph);
      expect(SystemGraphSchema.safeParse(input).success).toBe(false);
    });
  }
  for (const item of cases.invalid) {
    it(`rejects ${item.name}`, () => {
      expect(SystemGraphV02Schema.safeParse(item.graph).success).toBe(false);
    });
  }
  for (const item of cases.invalidJson) {
    it(`rejects ${item.name}`, () => {
      expect(SystemGraphV02Schema.safeParse(JSON.parse(item.raw)).success).toBe(false);
    });
  }
  for (const item of cases.validJson) {
    it(`preserves ${item.name}`, () => {
      const input = JSON.parse(item.raw);
      const parsed = SystemGraphV02Schema.parse(input);
      expect(parsed.nodes[0].metadata).toEqual(input.nodes[0].metadata);
      expect(Object.prototype.hasOwnProperty.call(parsed.nodes[0].metadata, '__proto__')).toBe(true);
    });
  }

  it('keeps the production File entry point on v0.1', async () => {
    const text = JSON.stringify(cases.valid[1].graph);
    expect(parseGraphText(text)).toMatchObject({ ok: false });
    expect(await readGraphFile(new Blob([text]))).toMatchObject({ ok: false });
    const mixed = { ...cases.valid[1].graph, schemaVersion: '0.1' };
    expect(SystemGraphSchema.safeParse(mixed).success).toBe(false);
  });
});
