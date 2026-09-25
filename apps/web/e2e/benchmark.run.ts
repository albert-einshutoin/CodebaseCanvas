import { spawn, execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { cpSync, existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, relative } from 'node:path';
import { performance } from 'node:perf_hooks';
import { fileURLToPath } from 'node:url';
import { SystemGraphSchema } from '../src/graph.ts';
import { binaryFromBuild, copyFixture } from './prepare.ts';
import { completeRun, parsePeakRss, repeatStats } from './benchmark.metrics.ts';

const checkout = join(dirname(fileURLToPath(import.meta.url)), '../../..');
const realCommit = 'c1c2cc4e448b279ff083272df1ac50d20c3304fa';
const realUrl = 'https://github.com/lujakob/nestjs-realworld-example-app';
const excluded = new Set(['.git', '.codebasecanvas', 'node_modules', 'dist', 'build', 'coverage', '.next', 'generated', '__generated__']);
const temporary = mkdtempSync(join(tmpdir(), 'cbc-issue27-'));
const pilot = process.env.CBC_BENCH_PILOT === '1';
const trials = pilot ? 1 : 6;
const output = join(checkout, `docs/benchmark/issue27-${pilot ? 'pilot' : 'raw'}.json`);
const sha = (bytes: Buffer | string) => createHash('sha256').update(bytes).digest('hex');
const measurementFiles = [
  'package.json', 'apps/web/playwright.benchmark.config.ts',
  'apps/web/e2e/benchmark.run.ts', 'apps/web/e2e/benchmark.metrics.ts', 'apps/web/e2e/performance.bench.ts',
  'apps/web/src/benchmarkTiming.ts', 'apps/web/src/graph.ts', 'apps/web/src/main.tsx', 'apps/web/src/Canvas.tsx',
];
const command = (program: string, args: string[], cwd = checkout, env = process.env) => execFileSync(program, args, { cwd, env, encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 });

function files(root: string): string[] {
  const found: string[] = [];
  function visit(dir: string) {
    for (const name of readdirSync(dir).sort()) {
      if (excluded.has(name)) continue;
      const full = join(dir, name);
      const stat = lstatSync(full);
      if (stat.isSymbolicLink()) throw new Error(`Input symlink unsupported: ${relative(root, full)}`);
      if (stat.isDirectory()) visit(full);
      else if (stat.isFile()) found.push(relative(root, full));
      else throw new Error(`Input special file unsupported: ${relative(root, full)}`);
    }
  }
  visit(root);
  return found;
}

function inputManifest(root: string) {
  const selected = files(root);
  const sources = selected.filter(path => /\.tsx?$/.test(path) && !path.endsWith('.d.ts'));
  const sourceLoc = sources.reduce((sum, path) => {
    const text = readFileSync(join(root, path), 'utf8');
    return sum + (text.length === 0 ? 0 : text.split('\n').length - (text.endsWith('\n') ? 1 : 0));
  }, 0);
  const manifest = selected.map(path => `${path}\0${sha(readFileSync(join(root, path)))}`).join('\n');
  return { fileCount: selected.length, tsFiles: sources.length, sourceLoc, manifestSha256: sha(manifest) };
}

function acquireReal(): { root: string; origin: string; commit: string; clean: boolean } {
  const root = process.env.CBC_BENCH_REAL_SOURCE ?? join(temporary, 'real-clone');
  if (!process.env.CBC_BENCH_REAL_SOURCE) {
    mkdirSync(root);
    command('git', ['init', '-q', root]);
    command('git', ['remote', 'add', 'origin', realUrl], root);
    command('git', ['-c', 'protocol.file.allow=never', 'fetch', '--no-tags', '--depth=1', 'origin', realCommit], root,
      { ...process.env, GIT_LFS_SKIP_SMUDGE: '1' });
    command('git', ['-c', 'core.hooksPath=/dev/null', '-c', 'filter.lfs.smudge=cat', '-c', 'filter.lfs.required=false', 'checkout', '--detach', 'FETCH_HEAD'], root,
      { ...process.env, GIT_LFS_SKIP_SMUDGE: '1' });
  }
  const commit = command('git', ['rev-parse', 'HEAD'], root).trim();
  const origin = command('git', ['remote', 'get-url', 'origin'], root).trim();
  const clean = !command('git', ['status', '--porcelain=v1'], root).trim();
  if (commit !== realCommit || origin !== realUrl || !clean) throw new Error('Fixed real repository identity or clean state mismatch');
  if (command('git', ['ls-files', '--stage'], root).split('\n').some(line => line.startsWith('160000 '))) throw new Error('Submodules are unsupported');
  return { root, origin, commit, clean };
}

async function analyze(binary: string, root: string, snapshot: string, trial: number) {
  const graphPath = join(root, '.codebasecanvas/graph.json');
  const previousInode = existsSync(graphPath) ? lstatSync(graphPath).ino : null;
  const started = performance.now();
  const args = process.platform === 'darwin' ? ['-l', binary, 'analyze', root] : ['-v', binary, 'analyze', root];
  const timed = process.platform === 'darwin' ? '/usr/bin/time' : '/usr/bin/time';
  const child = spawn(timed, args, { detached: true, stdio: ['ignore', 'pipe', 'pipe'] });
  let stdout = '', stderr = '', timedOut = false;
  child.stdout.setEncoding('utf8').on('data', chunk => { stdout += chunk; });
  child.stderr.setEncoding('utf8').on('data', chunk => { stderr += chunk; });
  const timer = setTimeout(() => { timedOut = true; try { process.kill(-child.pid!, 'SIGKILL'); } catch { /* exited */ } }, 120_000);
  const { code, signal, spawnError } = await new Promise<{code: number | null; signal: string | null; spawnError?: string}>((resolve) => {
    child.once('error', error => resolve({ code: null, signal: null, spawnError: error.message }));
    child.once('close', (code, signal) => resolve({ code, signal }));
  }).finally(() => clearTimeout(timer));
  const wallMs = performance.now() - started;
  if (timedOut) return { trial, status: 'TIMEOUT', reason: 'Analyzer exceeded 120s' };
  if (spawnError) return { trial, status: 'FAILED', reason: `Analyzer spawn failed: ${spawnError}` };
  if (code !== 0 || signal) return { trial, status: 'FAILED', reason: `Analyzer exit ${code}, signal ${signal}` };
  let peakRssBytes: number;
  try { peakRssBytes = parsePeakRss(stderr, process.platform === 'darwin' ? 'darwin' : 'linux'); }
  catch { return { trial, status: 'FAILED', reason: 'Peak RSS missing or invalid' }; }
  if (!existsSync(graphPath) || !lstatSync(graphPath).isFile()) return { trial, status: 'FAILED', reason: 'Graph output missing or invalid' };
  if (previousInode !== null && lstatSync(graphPath).ino === previousInode) return { trial, status: 'FAILED', reason: 'Graph output was not replaced' };
  const bytes = readFileSync(graphPath);
  let graph: unknown;
  try { graph = JSON.parse(bytes.toString('utf8')); }
  catch { return { trial, status: 'FAILED', reason: 'Graph output invalid JSON' }; }
  const parsed = SystemGraphSchema.safeParse(graph);
  if (!parsed.success) return { trial, status: 'FAILED', reason: 'Graph output violates SystemGraph contract' };
  const g = parsed.data;
  const ts = stdout.match(/Read (\d+) TypeScript\/TSX source files/);
  const prisma = stdout.match(/Analyzed (\d+) selected Prisma schemas/);
  if (!ts || !prisma) return { trial, status: 'FAILED', reason: 'CLI source counts missing' };
  writeFileSync(snapshot, bytes);
  const kinds = (items: {kind: string}[]) => Object.fromEntries([...new Set(items.map(item => item.kind))].sort().map(kind => [kind, items.filter(item => item.kind === kind).length]));
  const codes = Object.fromEntries([...new Set(g.diagnostics.map(item => item.code))].sort().map(code => [code, g.diagnostics.filter(item => item.code === code).length]));
  const normalized = { ...g, metadata: { ...g.metadata, analyzedAt: '<timestamp>' } };
  return { trial, status: 'OK', wallMs, peakRssBytes, peakRssMiB: peakRssBytes / 1048576,
    graphSha256: sha(bytes), normalizedSha256: sha(JSON.stringify(normalized)), graphBytes: bytes.length,
    tsFiles: Number(ts[1]), prismaSchemas: Number(prisma[1]), nodes: g.nodes.length, edges: g.edges.length,
    diagnostics: g.diagnostics.length, diagnosticSeverities: Object.fromEntries(['info', 'warning', 'error'].map(severity => [severity, g.diagnostics.filter(d => d.severity === severity).length])),
    nodeKinds: kinds(g.nodes), edgeKinds: kinds(g.edges), diagnosticCodes: codes,
    callAnalysis: g.metadata.callAnalysis, analyzerVersion: g.metadata.analyzerVersion, analyzedAt: g.metadata.analyzedAt,
    partial: g.diagnostics.some(item => item.severity === 'error') };
}

async function main() {
  if (!['darwin', 'linux'].includes(process.platform)) throw new Error('Only macOS and Linux RSS formats are supported');
  const real = acquireReal();
  const roots = [join(temporary, 'fixture'), join(temporary, 'real-input')];
  roots.forEach(root => mkdirSync(root));
  copyFixture(join(checkout, 'examples/nestjs-sample'), roots[0]);
  cpSync(real.root, roots[1], { recursive: true, filter: source => {
    if (source === real.root) return true;
    if (excluded.has(source.split('/').at(-1)!)) return false;
    const stat = lstatSync(source);
    if (stat.isSymbolicLink() || (!stat.isDirectory() && !stat.isFile())) throw new Error('Unsupported real input file type');
    return true;
  } });
  const profiles = roots.map((root, index) => ({ name: index ? 'real' : 'fixture', root, input: inputManifest(root), trials: [] as Awaited<ReturnType<typeof analyze>>[] }));
  const buildLog = command('cargo', ['build', '--release', '--locked', '-p', 'codebasecanvas-analyzer', '--bin', 'codebasecanvas', '--message-format=json']);
  const binary = binaryFromBuild(buildLog);
  if (binary !== join(checkout, 'target/release/codebasecanvas')) throw new Error('Cargo binary must be the current worktree release artifact');
  const binarySha256 = sha(readFileSync(binary));
  const measurementSourceSha256 = sha(measurementFiles.map(path => `${path}\0${sha(readFileSync(join(checkout, path)))}`).join('\n'));
  const result = { base: '6e5b974dc01bead04f93cff06c8ecab15613d8d1', real: { origin: real.origin, commit: real.commit, clean: real.clean },
    binarySha256, measurementSourceSha256, measurementFiles, startedAtUtc: new Date().toISOString(),
    profiles: [] as { name: string; input: ReturnType<typeof inputManifest>; trials: Awaited<ReturnType<typeof analyze>>[]; consistent: boolean; summary: unknown }[],
    web: {} as Record<string, { status: string; trial: number; graphHash?: string; reason?: string }[]>, status: (pilot ? 'PILOT' : 'OK') as 'PILOT' | 'OK' | 'FAILED', finishedAtUtc: '' };
  for (const profile of profiles) {
    const snapshot = join(temporary, `${profile.name}-first-success.json`);
    for (let trial = 0; trial < trials; trial++) {
      const candidate = join(temporary, `${profile.name}-${trial}.json`);
      const outcome = await analyze(binary, profile.root, candidate, trial);
      profile.trials.push(outcome);
      if (outcome.status === 'OK' && !existsSync(snapshot)) cpSync(candidate, snapshot);
      console.log(`${profile.name} Analyzer ${trial}: ${outcome.status}`);
    }
    const success = profile.trials.filter(t => t.status === 'OK');
    const consistent = success.every(t => t.normalizedSha256 === success[0]?.normalizedSha256);
    const summary = success.length === 6 && consistent && profile.trials[0].status === 'OK'
      ? { wallMs: repeatStats(profile.trials.slice(1).map(t => t.wallMs!)), maxPeakRssMiB: Math.max(...profile.trials.map(t => t.peakRssMiB!)) }
      : null;
    result.profiles.push({ name: profile.name, input: profile.input, trials: profile.trials, consistent, summary });
  }
  const webProfiles = profiles.flatMap(profile => {
    const first = profile.trials.find(t => t.status === 'OK');
    if (!first) return [];
    const graphPath = join(temporary, `${profile.name}-first-success.json`);
    return [{ name: profile.name, graphPath, graphHash: first.graphSha256!, graphBytes: first.graphBytes!, nodes: first.nodes!, edges: first.edges! }];
  });
  if (webProfiles.length) {
    command('pnpm', ['web:build']);
    const webOutput = join(temporary, 'browser.json');
    const input = join(temporary, 'browser-input.json');
    writeFileSync(input, JSON.stringify({ profiles: webProfiles, output: webOutput, trials }));
    let browserFailure: string | undefined;
    try { command('pnpm', ['--filter', '@codebasecanvas/web', 'exec', 'playwright', 'test', '--config', 'playwright.benchmark.config.ts'], checkout,
      { ...process.env, CBC_BENCH_INPUT: input }); }
    catch { browserFailure = 'Browser benchmark exited nonzero'; }
    if (existsSync(webOutput)) {
      try {
        const parsed: unknown = JSON.parse(readFileSync(webOutput, 'utf8'));
        if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) throw new Error('Invalid browser output shape');
        result.web = parsed as typeof result.web;
      }
      catch { browserFailure = 'Browser output invalid JSON'; }
    } else browserFailure = 'Browser output missing';
    if (browserFailure) {
      console.error(browserFailure);
      for (const profile of webProfiles) {
        const recorded = Array.isArray(result.web[profile.name]) ? result.web[profile.name] : [];
        result.web[profile.name] = [...recorded, { status: 'FAILED', trial: -1, reason: browserFailure }];
      }
    }
  }
  const hashes = Object.fromEntries(webProfiles.map(profile => [profile.name, profile.graphHash]));
  if (!pilot && !completeRun(result.profiles, result.web, hashes)) result.status = 'FAILED';
  result.finishedAtUtc = new Date().toISOString();
  mkdirSync(dirname(output), { recursive: true });
  writeFileSync(output, JSON.stringify(result, null, 2));
  console.log(`Raw result: ${output}`);
  if (result.status === 'FAILED') throw new Error('Benchmark trials incomplete; see raw result');
}

try { await main(); }
finally { rmSync(temporary, { recursive: true, force: true }); }
