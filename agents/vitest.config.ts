import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    // Seeded, deterministic, no network (CLAUDE.md section 2). The engine is stubbed at the
    // HTTP boundary only; strategy logic is tested against that stub, never against a live node.
    environment: 'node',
    include: ['test/**/*.test.ts'],
    restoreMocks: true,
    sequence: { shuffle: false },
    testTimeout: 10_000,
  },
});
