import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';

const origin = 'http://127.0.0.1:4186';
const checkout = join(dirname(fileURLToPath(import.meta.url)), '../../..');
const builtHtml = readFileSync(join(checkout, 'apps/web/dist/index.html'), 'utf8');
const builtAssets = new Set([...builtHtml.matchAll(/(?:src|href)="(\/assets\/[\w.-]+\.(?:js|css))"/g)].map(match => match[1]));
if (builtAssets.size < 2) throw new Error('Current Web build did not declare its JS and CSS assets');
const oracle = JSON.parse(readFileSync(join(checkout, 'examples/nestjs-sample/expected-graph.json'), 'utf8')) as GeneratedGraph;
const graphPath = process.env.CBC_E2E_GRAPH_PATH;
if (!graphPath) throw new Error('CBC_E2E_GRAPH_PATH must name this run\'s CLI output');

const authId = 'class:7372632f617574682f617574682e736572766963652e7473:4175746853657276696365';
const tokenId = 'class:7372632f617574682f746f6b656e2e7265706f7369746f72792e7473:546f6b656e5265706f7369746f7279';
const overrideId = 'class:7372632f72656772657373696f6e732f6f766572726964652e6d6f64756c652e7473:4f76657272696465436f6e74726f6c6c6572';
const usersId = 'class:7372632f75736572732f75736572732e736572766963652e7473:557365727353657276696365';

type GeneratedGraph = {
  schemaVersion: string;
  metadata: { analyzedAt: string; analyzerVersion: string; callAnalysis: { examinedCalls: number; emittedCalls: number; skippedCalls: number } };
  nodes: { id: string; name: string; kind: string; file?: string; parentId?: string }[];
  edges: { id: string; from: string; to: string; kind: string; metadata?: { semantics?: string } }[];
  diagnostics: unknown[];
};

function generated(): GeneratedGraph {
  return JSON.parse(readFileSync(graphPath!, 'utf8')) as GeneratedGraph;
}

async function guardNetwork(page: Page, documentPaths: readonly string[] = ['/']) {
  const rejected: string[] = [];
  await page.route('**/*', async route => {
    const request = route.request();
    const url = new URL(request.url());
    const type = request.resourceType();
    const allowed = request.method() === 'GET' && url.origin === origin && !url.search
      && ((documentPaths.includes(url.pathname) && type === 'document')
        || (builtAssets.has(url.pathname) && type === (url.pathname.endsWith('.js') ? 'script' : 'stylesheet')));
    if (allowed) await route.continue();
    else { rejected.push(`${request.method()} ${request.url()}`); await route.abort(); }
  });
  await page.routeWebSocket('**/*', socket => {
    rejected.push(`WebSocket ${socket.url()}`);
    socket.close();
  });
  return rejected;
}

test('static SPA loads at root and a direct deep path, then reloads without retaining the local graph', async ({ page }) => {
  const rejected = await guardNetwork(page, ['/', '/review']);
  const assetResponses: { url: string; type: string }[] = [];
  page.on('response', response => {
    if (builtAssets.has(new URL(response.url()).pathname)) assetResponses.push({ url: response.url(), type: response.headers()['content-type'] ?? '' });
  });
  const root = await page.goto('/');
  expect(root?.status()).toBe(200);
  expect(root?.headers()['content-type']).toContain('text/html');
  await expect(page.locator('#graph-file')).toBeVisible();
  const deep = await page.goto('/review');
  expect(deep?.status()).toBe(200);
  expect(deep?.headers()['content-type']).toContain('text/html');
  await expect(page.locator('#graph-file')).toBeVisible();
  await page.reload();
  await expect(page.locator('#graph-file')).toBeVisible();
  await importGraph(page);
  await expect(page.locator('.canvas-viewport')).toBeVisible();
  await page.reload();
  await expect(page.locator('#graph-file')).toBeVisible();
  await expect(page.locator('.canvas-viewport')).toHaveCount(0);
  for (const asset of builtAssets) {
    expect(assetResponses.some(response => new URL(response.url).pathname === asset
      && response.type.includes(asset.endsWith('.js') ? 'javascript' : 'text/css'))).toBe(true);
  }
  expect(rejected).toEqual([]);
});

async function importGraph(page: Page, path = graphPath!) {
  await page.locator('#graph-file').setInputFiles(path);
  await expect(page.getByRole('heading', { name: 'Graph loaded and validated' })).toBeVisible();
}

async function selectSearchResult(page: Page, query: string, id: string) {
  await page.getByRole('searchbox', { name: 'Search symbols' }).fill(query);
  await page.getByRole('list', { name: 'Search results' }).getByRole('button').filter({ hasText: id }).click();
  await expect(page.locator('.canvas-viewport')).toHaveAttribute('data-selected-nodes', id);
  await expect(page.locator('aside[aria-label="Node details"]')).toContainText(id);
}

test('current CLI graph loads into Cytoscape and supports search, evidence, focus, and real clipboard', async ({ page, context }) => {
  const rejected = await guardNetwork(page);
  await context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin });
  await page.goto('/');
  await importGraph(page);
  const graph = generated();
  expect(graph.schemaVersion).toBe('0.1');
  expect(graph.nodes).toHaveLength(57);
  expect(graph.edges).toHaveLength(90);
  expect(graph.diagnostics).toHaveLength(15);
  expect(graph.metadata.callAnalysis).toEqual({ scope: 'parsed_named_class_methods', mode: 'same_class_only', examinedCalls: 9, emittedCalls: 2, skippedCalls: 7 });
  expect(graph.nodes).toEqual(expect.arrayContaining([
    expect.objectContaining({ id: authId, kind: 'service', name: 'AuthService', file: 'src/auth/auth.service.ts' }),
    expect.objectContaining({ id: tokenId, kind: 'repository', name: 'TokenRepository', file: 'src/auth/token.repository.ts' }),
  ]));
  expect(graph.edges).toEqual(expect.arrayContaining([expect.objectContaining({ from: authId, to: tokenId, kind: 'injects', metadata: expect.objectContaining({ semantics: 'requested_token' }) })]));
  const analysis = page.getByRole('region', { name: 'Analysis' });
  await expect(analysis).toContainText('57 nodes · 90 edges · 15 diagnostics');
  await expect(analysis).toContainText('Examined call sites: 9 · Emitted call sites: 2 · Skipped call sites: 7');
  await expect(analysis.locator('time')).toHaveAttribute('datetime', graph.metadata.analyzedAt);
  const canvas = page.locator('.canvas-viewport');
  await expect(canvas).toHaveAttribute('data-graph-generation', '1');
  await expect(canvas).toHaveAttribute('data-layout-ready', 'true');
  await expect(canvas).toHaveAttribute('data-graph-nodes', /[1-9]\d*/);
  await expect(canvas).toHaveAttribute('data-graph-edges', /[1-9]\d*/);
  await expect(canvas.locator('canvas').first()).toBeVisible();
  expect((await canvas.boundingBox())?.width).toBeGreaterThan(100);
  await selectSearchResult(page, 'AuthService', authId);
  const details = page.locator('aside[aria-label="Node details"]');
  await expect(details.getByRole('heading', { name: 'AuthService' })).toBeVisible();
  await expect(details).toContainText('src/auth/auth.service.ts');
  await expect(details).toContainText(`Analyzed at (UTC): ${graph.metadata.analyzedAt}`);
  await expect(details).toContainText(`Analyzer: ${graph.metadata.analyzerVersion}`);
  await expect(details).toContainText('confirmed describes a static fact, not runtime behavior');
  const token = details.getByRole('listitem').filter({ hasText: 'Requests token' }).filter({ hasText: 'TokenRepository' }).first();
  await expect(token).toBeVisible();
  await token.getByText('Canonical edge direction / ID').click();
  await expect(token).toContainText(authId);
  await expect(token).toContainText(tokenId);
  await token.getByText('Edge evidence').click();
  await expect(token).toContainText('confirmed');
  await expect(details.getByRole('heading', { name: 'Related unknown / diagnostics' })).toBeVisible();
  await details.getByText(/Direct method: login \(1 diagnostics\)/).click();
  await expect(details).toContainText('unsupported_call_injected_receiver');
  await expect(details).toContainText('Skipped call sites: 1');
  await expect(details).toContainText('Examined: 9 / emitted: 2 / skipped: 7');
  const nodesBefore = await canvas.getAttribute('data-graph-nodes');
  const edgesBefore = await canvas.getAttribute('data-graph-edges');
  const databaseFilter = page.getByRole('checkbox', { name: 'Database' });
  await databaseFilter.uncheck();
  const controls = page.getByRole('region', { name: 'Dependency focus controls' });
  await controls.getByRole('button', { name: 'Focus neighbors' }).click();
  await expect(controls).toContainText('Focus: [service] AuthService');
  await expect(canvas).toHaveAttribute('data-focus-neighbors', /[1-9]\d*/);
  await expect(canvas).toHaveAttribute('data-focus-dimmed', /[1-9]\d*/);
  const neighborIds = JSON.parse((await canvas.getAttribute('data-focus-neighbor-ids'))!) as string[];
  const contextIds = JSON.parse((await canvas.getAttribute('data-focus-context-ids'))!) as string[];
  const dimmedIds = JSON.parse((await canvas.getAttribute('data-focus-dimmed-node-ids'))!) as string[];
  const focusEdgeIds = JSON.parse((await canvas.getAttribute('data-focus-edge-ids'))!) as string[];
  const parentFrameIds = JSON.parse((await canvas.getAttribute('data-parent-frame-ids'))!) as string[];
  const ancestorFrameIds = JSON.parse((await canvas.getAttribute('data-ancestor-frame-ids'))!) as string[];
  const authModuleId = oracle.nodes.find(node => node.name === 'AuthModule')!.id;
  const requestedEdgeId = oracle.edges.find(edge => edge.from === authId && edge.to === tokenId && edge.kind === 'injects')!.id;
  const parentEdgeId = oracle.edges.find(edge => edge.from === authModuleId && edge.to === authId && edge.kind === 'contains')!.id;
  const visible = new Set(oracle.nodes.filter(node => node.kind !== 'method' && node.kind !== 'database_model').map(node => node.id));
  const expectedEdges = oracle.edges.filter(edge => visible.has(edge.from) && visible.has(edge.to)
    && (edge.from === authId || edge.to === authId));
  const expectedNeighbors = new Set([authId, ...expectedEdges.flatMap(edge => [edge.from, edge.to])]);
  const sorted = (ids: Iterable<string>) => [...ids].sort();
  expect(expectedNeighbors.has(tokenId)).toBe(true);
  expect(expectedNeighbors.has(authModuleId)).toBe(true);
  expect(expectedEdges.map(edge => edge.id)).toContain(requestedEdgeId);
  expect(expectedEdges.map(edge => edge.id)).toContain(parentEdgeId);
  expect(neighborIds).toEqual(sorted(expectedNeighbors));
  expect(focusEdgeIds).toEqual(sorted(expectedEdges.map(edge => edge.id)));
  expect(contextIds).toEqual([]);
  expect(ancestorFrameIds).toEqual([]);
  expect(parentFrameIds).toEqual(sorted(new Set(oracle.nodes.filter(node => visible.has(node.id) && node.parentId).map(node => node.parentId!))));
  expect(dimmedIds).toEqual(sorted([...visible].filter(id => !expectedNeighbors.has(id))));
  expect(dimmedIds).toContain(usersId);
  await controls.getByRole('button', { name: 'Clear focus' }).click();
  await expect(canvas).toHaveAttribute('data-focus-dimmed', '0');
  await expect(canvas).toHaveAttribute('data-selected-nodes', authId);
  await expect(databaseFilter).not.toBeChecked();
  expect(Number(await canvas.getAttribute('data-graph-nodes'))).toBeLessThanOrEqual(Number(nodesBefore));
  expect(Number(await canvas.getAttribute('data-graph-edges'))).toBeLessThanOrEqual(Number(edgesBefore));
  await expect(page.getByRole('searchbox', { name: 'Search symbols' })).toHaveValue('AuthService');
  await page.evaluate(() => navigator.clipboard.writeText('E2E_SENTINEL'));
  await details.getByRole('button', { name: 'Copy Context for AuthService' }).click();
  await expect(details).toContainText('Copied Context for AuthService.');
  const copied = await page.evaluate(() => navigator.clipboard.readText());
  expect(copied).not.toBe('E2E_SENTINEL');
  expect(copied).toContain('# CodebaseCanvas Context');
  expect(copied).toContain('AuthService');
  expect(copied).toContain(authId);
  expect(copied).toContain('TokenRepository');
  expect(copied).toContain(graph.metadata.analyzedAt);
  expect(copied).toContain(graph.metadata.analyzerVersion.replaceAll('.', '\\.'));
  expect(copied).toContain('Analysis scope / unknown');
  expect(copied).toMatch(/unsupported.*call.*injected.*receiver/);
  expect(copied).toContain('Selected node + included methods: diagnostics=1; skipped call sites=1');
  expect(copied).toContain('confirmed means a stated static fact');
  expect(Array.from(copied).length).toBeLessThanOrEqual(32_000);
  expect(rejected).toEqual([]);
});

test('provider override preserves requested token, unknown receiver, and no inferred runtime implementation', async ({ page, context }) => {
  const rejected = await guardNetwork(page);
  await context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin });
  await page.goto('/');
  await importGraph(page);
  await selectSearchResult(page, 'OverrideController', overrideId);
  const details = page.locator('aside[aria-label="Node details"]');
  await expect(details).toContainText('Requests token / Requested by describe requested tokens, not resolved implementations');
  const requested = details.getByRole('listitem').filter({ hasText: 'Requests token' }).filter({ hasText: 'UsersService' }).first();
  await expect(requested).toContainText(usersId);
  await expect(requested).not.toContainText('MockUsersService');
  await expect(details).toContainText('Related unknown / diagnostics');
  await details.getByText(/Direct method: list \(1 diagnostics\)/).click();
  await expect(details).toContainText('unsupported_call_injected_receiver');
  await details.getByRole('button', { name: 'Copy Context for OverrideController' }).click();
  await expect(details).toContainText('Copied Context for OverrideController.');
  const copied = await page.evaluate(() => navigator.clipboard.readText());
  expect(copied).toContain('OverrideController');
  expect(copied).toContain('UsersService');
  expect(copied).toMatch(/unsupported.*call.*injected.*receiver/);
  expect(copied).toContain('not provider implementations, overrides, instances or runtime calls');
  const graph = generated();
  const mockId = graph.nodes.find(n => n.name === 'MockUsersService')?.id;
  const handler = graph.nodes.find(n => n.name === 'list' && n.file === 'src/regressions/override.module.ts');
  expect(mockId).toBeTruthy();
  expect(handler).toBeTruthy();
  expect(graph.edges.filter(e => e.kind === 'calls' && e.from === handler!.id && e.to === mockId)).toEqual([]);
  const endpointId = oracle.nodes.find(n => n.kind === 'endpoint' && n.name === 'GET /override')?.id;
  expect(endpointId).toBeTruthy();
  await selectSearchResult(page, 'GET /override', endpointId!);
  const endpointDetails = page.locator('aside[aria-label="Node details"]');
  await expect(endpointDetails).toContainText('Handler');
  await expect(endpointDetails).toContainText('list');
  await endpointDetails.getByRole('button', { name: 'Copy Context for GET /override' }).click();
  await expect(endpointDetails).toContainText('Copied Context for GET /override.');
  const endpointContext = await page.evaluate(() => navigator.clipboard.readText());
  expect(endpointContext).toContain('## Endpoint controller requested tokens');
  expect(endpointContext).toContain('UsersService');
  expect(endpointContext).not.toContain('## Endpoint handler static calls');
  expect(endpointContext).toMatch(/unsupported.*call.*injected.*receiver/);
  expect(rejected).toEqual([]);
});

test('unsupported schema and dangling edge are rejected, and a fresh import clears prior state', async ({ page, context }, testInfo) => {
  const rejected = await guardNetwork(page);
  await context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin });
  await page.goto('/');
  const graph = generated();
  const badVersion = testInfo.outputPath('unsupported.json');
  writeFileSync(badVersion, JSON.stringify({ ...graph, schemaVersion: '9.9' }));
  await page.locator('#graph-file').setInputFiles(badVersion);
  await expect(page.getByRole('alert')).toContainText('Unsupported schema version');
  await expect(page.locator('.canvas-viewport')).toHaveCount(0);
  await expect(page.getByRole('button', { name: /Copy Context/ })).toHaveCount(0);
  await importGraph(page);
  await selectSearchResult(page, 'AuthService', authId);
  await page.getByRole('button', { name: 'Copy Context for AuthService' }).click();
  await expect(page.locator('aside[aria-label="Node details"]')).toContainText('Copied Context for AuthService.');
  const firstGeneration = await page.locator('.canvas-viewport').getAttribute('data-graph-generation');
  const badEdge = testInfo.outputPath('dangling.json');
  writeFileSync(badEdge, JSON.stringify({ ...graph, edges: [{ ...graph.edges[0], to: 'class:00:00' }, ...graph.edges.slice(1)] }));
  await page.locator('#graph-file').setInputFiles(badEdge);
  await expect(page.getByRole('alert')).toContainText('Dangling edge');
  await expect(page.locator('.canvas-viewport')).toHaveCount(0);
  await expect(page.locator('aside[aria-label="Node details"]')).toHaveCount(0);
  await expect(page.getByRole('button', { name: /Copy Context/ })).toHaveCount(0);
  await expect(page.getByText('Copied Context for AuthService.')).toHaveCount(0);
  await importGraph(page);
  await expect(page.locator('.canvas-viewport')).toHaveAttribute('data-graph-generation', String(Number(firstGeneration) + 2));
  await expect(page.locator('.canvas-viewport')).toHaveAttribute('data-selected-nodes', '');
  await expect(page.locator('aside[aria-label="Node details"]')).toContainText('Select a node to inspect');
  expect(rejected).toEqual([]);
});

test('network guard detects a forbidden request before it can leave the page', async ({ page }) => {
  const rejected = await guardNetwork(page);
  await page.goto('/');
  await page.evaluate(async () => { try { await fetch('/upload', { method: 'POST', body: 'negative-control' }); } catch { /* blocked by the route */ } });
  await page.evaluate(async path => { try { await fetch(path); } catch { /* asset-shaped fetch is also blocked */ } }, [...builtAssets][0]);
  expect(rejected).toEqual(['POST http://127.0.0.1:4186/upload', `GET ${origin}${[...builtAssets][0]}`]);
});
