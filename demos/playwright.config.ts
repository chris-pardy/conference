import { resolve } from 'node:path'
import { defineConfig } from '@playwright/test'
import base from '../playwright.config.ts'

// Demo recordings: the e2e setup (backend + built PWA + the run's vivarium)
// with video always on. Not part of the test suite and not run in CI.
// Run: node scripts/with-vivarium.ts playwright test -c demos/playwright.config.ts
export default defineConfig({
  ...base,
  testDir: '.',
  testMatch: '*.demo.ts',
  outputDir: '../test-results/demos',
  reporter: 'list',
  // The servers' commands are relative to the repo root, not this directory.
  // A demo can idle longer than e2e's ten-second sessions (groups.demo.ts
  // keeps a second phone open), so the idle timeout comes from DEMO_IDLE_TIMEOUT.
  webServer: [base.webServer ?? []].flat().map((server) => ({
    ...server,
    cwd: resolve(import.meta.dirname, '..'),
    env:
      server.env?.SESSION_IDLE_TIMEOUT && process.env.DEMO_IDLE_TIMEOUT
        ? { ...server.env, SESSION_IDLE_TIMEOUT: process.env.DEMO_IDLE_TIMEOUT }
        : server.env,
  })),
  use: { ...base.use, video: { mode: 'on', size: { width: 412, height: 915 } } },
})
