import { expect, test } from 'vitest';
import { browserDurations, completeRun, parsePeakRss, repeatStats } from './benchmark.metrics.ts';

test('RSS units are normalized and bad output is rejected', () => {
  expect(parsePeakRss('  1327104  maximum resident set size\n', 'darwin')).toBe(1327104);
  expect(parsePeakRss('Maximum resident set size (kbytes): 42\n', 'linux')).toBe(43008);
  for (const value of ['0 maximum resident set size', '-1 maximum resident set size', 'maximum resident set size']) {
    expect(() => parsePeakRss(value, 'darwin')).toThrow();
  }
});

test('missing, failed, timed-out, and mismatched trials never complete a run', () => {
  const trials = Array.from({ length: 6 }, () => ({ status: 'OK', normalizedSha256: 'same' }));
  const browser = Array.from({ length: 6 }, () => ({ status: 'OK', graphHash: 'graph' }));
  const profiles = [{ name: 'fixture', trials }, { name: 'real', trials }];
  const web = { fixture: browser, real: browser };
  const hashes = { fixture: 'graph', real: 'graph' };
  expect(completeRun(profiles, web, hashes)).toBe(true);
  expect(completeRun(profiles, { ...web, real: browser.slice(1) }, hashes)).toBe(false);
  expect(completeRun(profiles, { ...web, real: browser.map((item, index) => index === 2 ? { ...item, status: 'FAILED' } : item) }, hashes)).toBe(false);
  expect(completeRun([{ ...profiles[0], trials: trials.map((item, index) => index === 1 ? { ...item, status: 'TIMEOUT' } : item) }, profiles[1]], web, hashes)).toBe(false);
  expect(completeRun([{ ...profiles[0], trials: trials.map((item, index) => index === 1 ? { ...item, normalizedSha256: 'changed' } : item) }, profiles[1]], web, hashes)).toBe(false);
});

test('repeat statistics require five complete trials and keep the first trial separate', () => {
  expect(repeatStats([3, 5, 2, 4, 1])).toEqual({ median: 3, min: 1, max: 5 });
  expect(() => repeatStats([1, 2, 3, 4])).toThrow();
  expect(() => repeatStats([1, 2, 3, 4, NaN])).toThrow();
});

test('browser marks must all belong to the requested generation and be ordered', () => {
  const steps = ['import_start', 'read_start', 'read_end', 'json_start', 'json_end', 'validation_start', 'validation_end', 'canvas_start', 'layout_start', 'layout_end', 'render_end'];
  const marks = steps.map((step, index) => ({ name: `cbc-benchmark:2:${step}`, startTime: index * 2 }));
  expect(browserDurations(marks, 2).import_to_render_ms).toBe(20);
  expect(browserDurations(marks, 2).validation_ms).toBe(2);
  expect(() => browserDurations(marks, 1)).toThrow(/Missing/);
  expect(() => browserDurations(marks.filter(mark => !mark.name.endsWith('render_end')), 2)).toThrow(/Missing/);
  expect(() => browserDurations(marks.map(mark => mark.name.endsWith('layout_end') ? { ...mark, startTime: 0 } : mark), 2)).toThrow(/Out-of-order/);
});
