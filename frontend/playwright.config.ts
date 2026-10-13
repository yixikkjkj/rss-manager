import { defineConfig } from '@playwright/test';
import { existsSync } from 'node:fs';

export default defineConfig({
  testDir: './e2e',
  workers: 1,
  fullyParallel: false,
  timeout: 30000,
  use: {
    baseURL: 'http://127.0.0.1:39183',
    headless: true,
    channel:
      process.platform === 'win32' &&
      existsSync('C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe')
        ? 'msedge'
        : undefined,
    trace: 'retain-on-failure',
  },
  webServer: {
    command: 'node ../scripts/e2e-server.cjs',
    url: 'http://127.0.0.1:39183/healthz',
    reuseExistingServer: false,
    timeout: 30000,
  },
});
