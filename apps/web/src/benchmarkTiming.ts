const enabled = typeof window !== 'undefined' && new URLSearchParams(window.location.search).get('cbc-benchmark') === '1';
let activeGeneration = 0;
let marked: string[] = [];

export function beginBenchmark(generation: number): void {
  if (!enabled) return;
  for (const name of marked) performance.clearMarks(name);
  marked = [];
  activeGeneration = generation;
  benchmarkMark(generation, 'import_start');
}

export function benchmarkMark(generation: number, step: string): void {
  if (!enabled || generation !== activeGeneration) return;
  const name = `cbc-benchmark:${generation}:${step}`;
  performance.mark(name);
  marked.push(name);
}

export function benchmarkEnabled(generation: number): boolean {
  return enabled && generation === activeGeneration;
}
