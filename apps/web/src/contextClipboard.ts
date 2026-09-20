export type CopyState =
  | { kind: 'idle' | 'pending' | 'copied' | 'busy' | 'generationError' }
  | { kind: 'copyError'; markdown: string };

/** Shared across detail remounts: an OS write cannot be cancelled by changing selection. */
export const clipboardCoordinator = { pending: false };
export function createCopyController(
  generate: () => string,
  write: ((markdown: string) => Promise<void>) | undefined,
  update: (state: CopyState) => void,
  coordinator = clipboardCoordinator,
) {
  let disposed = false;
  let running = false;
  const publish = (state: CopyState) => { if (!disposed) update(state); };
  return {
    dispose() { disposed = true; },
    async copy() {
      if (disposed || running) return;
      if (coordinator.pending) { publish({ kind: 'busy' }); return; }
      let markdown: string;
      try {
        markdown = generate();
        if (!markdown) throw new Error('Empty context');
      } catch {
        publish({ kind: 'generationError' }); return;
      }
      running = true;
      coordinator.pending = true;
      publish({ kind: 'pending' });
      try {
        if (!write) throw new Error('Clipboard unavailable');
        await write(markdown);
        publish({ kind: 'copied' });
      } catch {
        publish({ kind: 'copyError', markdown });
      } finally {
        running = false;
        coordinator.pending = false;
      }
    },
  };
}
