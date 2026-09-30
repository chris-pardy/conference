import { defineConfig, devices } from '@playwright/test'

// The backend and the built PWA, as a phone sees them. Run through
// `pnpm test:e2e` so the backend is pointed at the run's sealed vivarium.
const SERVER_PORT = 3100
const PREVIEW_PORT = 4173

export default defineConfig({
  testDir: './e2e',
  forbidOnly: !!process.env.CI,
  reporter: [['list'], ['html', { open: 'never' }]],
  use: {
    ...devices['Pixel 7'],
    baseURL: `http://localhost:${PREVIEW_PORT}`,
    serviceWorkers: 'block',
    trace: 'retain-on-failure',
    video: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
  projects: [{ name: 'chromium', use: { browserName: 'chromium' } }],
  webServer: [
    {
      command: 'target/debug/conference-server',
      env: { PORT: String(SERVER_PORT), ATPROTO_URL: process.env.VIVARIUM_URL ?? '' },
      url: `http://127.0.0.1:${SERVER_PORT}/health`,
      reuseExistingServer: false,
    },
    {
      command: `pnpm --filter @conference/web preview --port ${PREVIEW_PORT} --strictPort`,
      env: { SERVER_URL: `http://127.0.0.1:${SERVER_PORT}` },
      url: `http://localhost:${PREVIEW_PORT}`,
      reuseExistingServer: false,
    },
  ],
})
