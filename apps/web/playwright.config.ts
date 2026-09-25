import { defineConfig } from '@playwright/test';

const origin = 'http://127.0.0.1:4186';

export default defineConfig({
  testDir: './e2e',
  testMatch: '*.e2e.ts',
  workers: 1,
  retries: 0,
  reporter: 'list',
  use: {
    baseURL: origin,
    browserName: 'chromium',
    serviceWorkers: 'block',
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
  webServer: {
    command: 'pnpm exec vite preview --host 127.0.0.1 --port 4186 --strictPort',
    url: origin,
    reuseExistingServer: false,
    timeout: 30_000,
  },
});
