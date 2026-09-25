export type Mark = { name: string; startTime: number };

const pairs = {
  read_decode_ms: ['read_start', 'read_end'],
  json_parse_ms: ['json_start', 'json_end'],
  validation_ms: ['validation_start', 'validation_end'],
  layout_ms: ['layout_start', 'layout_end'],
  initial_canvas_ms: ['canvas_start', 'render_end'],
  import_to_render_ms: ['import_start', 'render_end'],
} as const;

export function browserDurations(marks: Mark[], generation: number): Record<keyof typeof pairs, number> {
  const prefix = `cbc-benchmark:${generation}:`;
  const current = new Map(marks.filter(mark => mark.name.startsWith(prefix)).map(mark => [mark.name.slice(prefix.length), mark.startTime]));
  const ordered = ['import_start', 'read_start', 'read_end', 'json_start', 'json_end', 'validation_start', 'validation_end', 'canvas_start', 'layout_start', 'layout_end', 'render_end'];
  if (ordered.some(step => !Number.isFinite(current.get(step)))) throw new Error('Missing browser timing mark');
  for (let i = 1; i < ordered.length; i++) {
    if (current.get(ordered[i])! < current.get(ordered[i - 1])!) throw new Error('Out-of-order browser timing marks');
  }
  return Object.fromEntries(Object.entries(pairs).map(([key, [start, end]]) => [key, current.get(end)! - current.get(start)!])) as Record<keyof typeof pairs, number>;
}

export function parsePeakRss(stderr: string, platform: 'darwin' | 'linux'): number {
  const pattern = platform === 'darwin' ? /^\s*(\d+)\s+maximum resident set size\s*$/m : /^\s*Maximum resident set size \(kbytes\):\s*(\d+)\s*$/m;
  const match = stderr.match(pattern);
  if (!match) throw new Error('Peak RSS missing or unparseable');
  const value = Number(match[1]) * (platform === 'linux' ? 1024 : 1);
  if (!Number.isSafeInteger(value) || value <= 0) throw new Error('Invalid peak RSS');
  return value;
}

export function repeatStats(values: number[]): { median: number; min: number; max: number } {
  if (values.length !== 5 || values.some(value => !Number.isFinite(value) || value < 0)) throw new Error('Five valid repeat values required');
  const sorted = [...values].sort((a, b) => a - b);
  return { median: sorted[2], min: sorted[0], max: sorted[4] };
}

export function completeRun(profiles: { name: string; trials: { status: string; normalizedSha256?: string }[] }[],
  web: Record<string, { status: string; graphHash?: string }[]>, graphHashes: Record<string, string>): boolean {
  return profiles.length === 2 && new Set(profiles.map(profile => profile.name)).size === 2
    && ['fixture', 'real'].every(name => profiles.some(profile => profile.name === name)) && profiles.every(profile => {
    const normalized = profile.trials[0]?.normalizedSha256;
    const browser = web[profile.name];
    return profile.trials.length === 6 && !!normalized
      && profile.trials.every(trial => trial.status === 'OK' && trial.normalizedSha256 === normalized)
      && Array.isArray(browser) && browser.length === 6 && !!graphHashes[profile.name]
      && browser.every(trial => trial?.status === 'OK' && trial.graphHash === graphHashes[profile.name]);
  });
}
