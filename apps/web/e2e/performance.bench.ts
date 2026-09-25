import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { test, expect } from '@playwright/test';
import { browserDurations } from './benchmark.metrics';

type Input = { profiles: { name: string; graphPath: string; graphHash: string; graphBytes: number; nodes: number; edges: number }[]; output: string; trials: number };
const inputPath = process.env.CBC_BENCH_INPUT;
if (!inputPath) throw new Error('CBC_BENCH_INPUT is required');
const input = JSON.parse(readFileSync(inputPath, 'utf8')) as Input;

test('production File input and matching Cytoscape render', async ({ browser }) => {
  test.setTimeout(13 * 60_000);
  const result: Record<string, unknown[]> = {};
  const save = () => writeFileSync(input.output, JSON.stringify(result, null, 2));
  for (const profile of input.profiles) {
    result[profile.name] = [];
    for (let trial = 0; trial < input.trials; trial++) {
      const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1, serviceWorkers: 'block' });
      const page = await context.newPage();
      let timer: ReturnType<typeof setTimeout> | undefined;
      try {
        const graphHash = createHash('sha256').update(readFileSync(profile.graphPath)).digest('hex');
        if (graphHash !== profile.graphHash) throw new Error('Graph file/hash mismatch');
        await page.goto('http://127.0.0.1:4187/?cbc-benchmark=1');
        const measured = async () => {
          await page.locator('#graph-file').setInputFiles(profile.graphPath);
          await page.waitForFunction(() => performance.getEntriesByName('cbc-benchmark:1:render_end', 'mark').length === 1, undefined, { timeout: 60_000 });
          await expect(page.getByRole('heading', { name: 'Graph loaded and validated' })).toBeVisible();
          const canvas = page.locator('.canvas-viewport');
          await expect(canvas).toHaveAttribute('data-graph-generation', '1');
          await expect(canvas).toHaveAttribute('data-layout-ready', 'true');
          const display = await canvas.evaluate(element => ({
            nodes: Number(element.getAttribute('data-graph-nodes')),
            edges: Number(element.getAttribute('data-graph-edges')),
            parentFrames: (JSON.parse(element.getAttribute('data-parent-frame-ids') ?? '[]') as string[]).length,
          }));
          if (!Number.isSafeInteger(display.nodes) || display.nodes < 1 || !Number.isSafeInteger(display.edges) || !Number.isSafeInteger(display.parentFrames)) throw new Error('Invalid displayed counts');
          const marks = await page.evaluate(() => performance.getEntriesByType('mark').map(entry => ({ name: entry.name, startTime: entry.startTime })));
          return { status: 'OK', trial, graphHash: profile.graphHash, graphBytes: profile.graphBytes, nodes: profile.nodes, edges: profile.edges,
            display, timings: browserDurations(marks, 1) };
        };
        const value = await Promise.race([measured(), new Promise<never>((_, reject) => { timer = setTimeout(() => reject(new Error('Web import/render timeout 60s')), 60_000); })]);
        result[profile.name].push(value);
      } catch (error) {
        result[profile.name].push({ status: 'FAILED', trial, graphHash: profile.graphHash, reason: String(error) });
      } finally {
        if (timer) clearTimeout(timer);
        await context.close();
        save();
      }
    }
  }
  for (const profile of input.profiles) expect(result[profile.name].every(value => (value as {status: string}).status === 'OK')).toBe(true);
});
