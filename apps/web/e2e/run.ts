import { spawnSync } from 'node:child_process';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { binaryFromBuild, copyFixture, readGeneratedGraph, requireSuccess } from './prepare.ts';

const checkout = join(dirname(fileURLToPath(import.meta.url)), '../../..');
const fixture = join(checkout, 'examples/nestjs-sample');
const temporary = mkdtempSync(join(tmpdir(), 'canvas-current-e2e-'));

function command(label: string, program: string, args: string[], capture = false, env = process.env) {
  const result = spawnSync(program, args, {
    cwd: checkout, env, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024,
    stdio: capture ? ['inherit', 'pipe', 'pipe'] : 'inherit',
  });
  if (capture && result.stderr) process.stderr.write(result.stderr);
  if (result.error) throw new Error(`${label}: ${result.error.message}`);
  requireSuccess(label, result.status);
  return result.stdout ?? '';
}

try {
  const commit = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: checkout, encoding: 'utf8' }).trim();
  const dirty = execFileSync('git', ['status', '--porcelain=v1'], { cwd: checkout, encoding: 'utf8' }).trim().length > 0;
  console.log(`E2E checkout: ${commit} (${dirty ? 'uncommitted changes present' : 'clean'})`);
  const build = command('Rust build', 'cargo', ['build', '--locked', '-p', 'codebasecanvas-analyzer', '--bin', 'codebasecanvas', '--message-format=json'], true);
  const binary = binaryFromBuild(build);
  console.log(`E2E binary from current Cargo build: ${binary}`);
  copyFixture(fixture, temporary);
  command('CLI analyze', binary, ['analyze', temporary]);
  const graphPath = join(temporary, '.codebasecanvas', 'graph.json');
  const { graph, sha256 } = readGeneratedGraph(graphPath);
  console.log(`E2E CLI exit: 0; graph: ${graphPath}; sha256=${sha256}; nodes=${graph.nodes.length}; edges=${graph.edges.length}; diagnostics=${graph.diagnostics.length}; analyzedAt=${graph.metadata.analyzedAt}`);
  command('Preparation boundary tests', process.execPath, ['--experimental-strip-types', '--test', 'apps/web/e2e/prepare.node.ts']);
  command('Current Web build', 'pnpm', ['--filter', '@codebasecanvas/web', 'build']);
  command('Browser E2E', 'pnpm', ['--filter', '@codebasecanvas/web', 'exec', 'playwright', 'test', '--config', 'playwright.config.ts'], false,
    { ...process.env, CBC_E2E_GRAPH_PATH: graphPath });
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
