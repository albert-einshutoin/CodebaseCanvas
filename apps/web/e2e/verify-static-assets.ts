import { lstatSync, readFileSync, readdirSync } from 'node:fs';
import { isDeepStrictEqual } from 'node:util';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const dist = resolve(dirname(fileURLToPath(import.meta.url)), '../dist');
const config = JSON.parse(readFileSync(join(dist, 'wrangler.json'), 'utf8')) as Record<string, unknown>;
const assets = config.assets as Record<string, unknown> | undefined;

function fail(message: string): never { throw new Error(`Static assets: ${message}`); }
function regular(path: string) {
  if (!lstatSync(path).isFile()) fail(`not a regular file: ${path}`);
}

if (config.name !== 'codebasecanvas' || config.compatibility_date !== '2026-09-24') fail('generated deployment identity changed');
if (config.send_metrics !== false) fail('Wrangler telemetry is enabled');
if (!assets || Object.keys(assets).sort().join(',') !== 'directory,not_found_handling'
  || assets.directory !== '.' || assets.not_found_handling !== 'single-page-application') fail('generated assets config changed');
const allowedMetadata = new Set([
  'configPath', 'userConfigPath', 'topLevelName', 'name', 'compatibility_date',
  'jsx_factory', 'jsx_fragment', 'assets', 'dev', 'python_modules', 'send_metrics',
]);
const generatedEmptyGroups: Record<string, unknown> = {
  vars: {},
  durable_objects: { bindings: [] },
  queues: { producers: [], consumers: [] },
  logfwdr: { bindings: [] },
};
export function assertNoWorkerFeatures(candidate: Record<string, unknown>) {
  if ('main' in candidate || 'route' in candidate || 'routes' in candidate) fail('Worker entrypoint or route is present');
  for (const [key, value] of Object.entries(candidate)) {
    if (allowedMetadata.has(key)) continue;
    const empty = key in generatedEmptyGroups ? generatedEmptyGroups[key]
      : Array.isArray(value) ? [] : {};
    if (!isDeepStrictEqual(value, empty)) fail(`unexpected Worker feature or binding: ${key}`);
  }
}
assertNoWorkerFeatures(config);
if (!lstatSync(dist).isDirectory()) fail('asset directory is not a directory');
const rootEntries = readdirSync(dist).sort();
if (rootEntries.join(',') !== '.assetsignore,_headers,assets,index.html,wrangler.json') fail(`unexpected asset-directory entries: ${rootEntries.join(', ')}`);
for (const name of rootEntries.filter(name => name !== 'assets')) regular(join(dist, name));
const ignored = readFileSync(join(dist, '.assetsignore'), 'utf8').trim().split(/\r?\n/).sort();
if (ignored.join(',') !== '.dev.vars,wrangler.json') fail('generated deployment config is not excluded from upload');
const headers = readFileSync(join(dist, '_headers'), 'utf8');
for (const line of ['/*', 'X-Content-Type-Options: nosniff', 'Referrer-Policy: no-referrer', 'X-Frame-Options: DENY']) {
  if (!headers.includes(line)) fail(`missing static header: ${line}`);
}
const html = readFileSync(join(dist, 'index.html'), 'utf8');
const referenced = new Set([...html.matchAll(/(?:src|href)="(\/assets\/[\w-]+\.(?:js|css))"/g)].map(match => match[1].slice('/assets/'.length)));
if (![...referenced].some(name => name.endsWith('.js')) || ![...referenced].some(name => name.endsWith('.css'))) fail('index.html lacks absolute JS/CSS asset URLs');
const assetsDir = join(dist, 'assets');
if (!lstatSync(assetsDir).isDirectory()) fail('assets is not a directory');
const generated = readdirSync(assetsDir).sort();
if (generated.length !== referenced.size || generated.some(name => !referenced.has(name) || !/^index-[\w-]+\.(js|css)$/.test(name))) fail('client assets differ from the current HTML references');
for (const name of generated) regular(join(assetsDir, name));
console.log(`Verified generated config and client assets: ${generated.join(', ')}`);
