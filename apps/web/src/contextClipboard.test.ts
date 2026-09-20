import { expect, it, vi } from 'vitest';
import { createCopyController } from './contextClipboard';
const deferred = () => {
  let resolve!: () => void, reject!: () => void;
  const promise = new Promise<void>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
};
it('writes exact generated text, publishes success only after resolution, and permits recopy', async () => {
  const d = deferred(); const update = vi.fn(); const write = vi.fn(() => d.promise);
  const c = createCopyController(() => '# exact 日本語 😀', write, update, { pending: false });
  const pending = c.copy();
  expect(write).toHaveBeenCalledExactlyOnceWith('# exact 日本語 😀');
  expect(update.mock.calls).toEqual([[{ kind: 'pending' }]]);
  await c.copy(); expect(write).toHaveBeenCalledTimes(1);
  d.resolve(); await pending;
  expect(update).toHaveBeenLastCalledWith({ kind: 'copied' });
  await c.copy(); expect(write).toHaveBeenCalledTimes(2);
});
it.each(['reject', 'absent', 'throw'])('retains the same generated text on clipboard %s without raw errors', async kind => {
  const update = vi.fn();
  const c = createCopyController(() => '# snapshot', kind === 'absent' ? undefined : () => {
    if (kind === 'throw') throw new Error('/private/error');
    return Promise.reject(new Error('permission'));
  }, update, { pending: false });
  await c.copy();
  expect(update).toHaveBeenLastCalledWith({ kind: 'copyError', markdown: '# snapshot' });
  expect(JSON.stringify(update.mock.calls)).not.toContain('/private');
});
it.each([() => { throw new Error('bad graph'); }, () => ''])('separates generation failures from clipboard failures', async generate => {
  const write = vi.fn(); const update = vi.fn();
  await createCopyController(generate, write, update, { pending: false }).copy();
  expect(write).not.toHaveBeenCalled();
  expect(update.mock.calls).toEqual([[{ kind: 'generationError' }]]);
});
it.each(['resolve', 'reject'])('ignores stale %s across selection/remount/same-ID snapshot changes and serializes OS writes', async outcome => {
  const coordinator = { pending: false }; const old = deferred(); const previous = vi.fn(); const next = vi.fn();
  const a = createCopyController(() => 'old snapshot', () => old.promise, previous, coordinator);
  const run = a.copy(); a.dispose();
  const write = vi.fn(async () => {});
  const b = createCopyController(() => 'same ID, new snapshot', write, next, coordinator);
  await b.copy(); expect(next).toHaveBeenLastCalledWith({ kind: 'busy' }); expect(write).not.toHaveBeenCalled();
  if (outcome === 'resolve') old.resolve(); else old.reject(); await run;
  expect(previous.mock.calls).toEqual([[{ kind: 'pending' }]]);
  await b.copy(); expect(write).toHaveBeenCalledExactlyOnceWith('same ID, new snapshot');
  expect(next).toHaveBeenLastCalledWith({ kind: 'copied' });
  await a.copy(); expect(previous).toHaveBeenCalledTimes(1);
});
