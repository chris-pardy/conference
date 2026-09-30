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
  webServer: [base.webServer ?? []].flat().map((server) => ({ ...server, cwd: resolve(import.meta.dirname, '..') })),
  use: { ...base.use, video: { mode: 'on', size: { width: 412, height: 915 } } },
})
