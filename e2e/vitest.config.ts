import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    environment: 'node',
    include: ['test/**/*.test.ts'],
    // The harness boots anvil, deploys, migrates, and runs scenarios. It is slow by nature;
    // it is never flaky, and a retry would hide that distinction.
    retry: 0,
    testTimeout: 300_000,
    hookTimeout: 120_000,
  },
});
