import { defineConfig } from 'vitest/config';
export default defineConfig({
  test: {
    server: { deps: { inline: ['yixi-lit-design'] } },
    environment: 'jsdom',
    include: ['src/**/*.test.ts'],
    setupFiles: ['./src/test/Setup.ts'],
  },
});
