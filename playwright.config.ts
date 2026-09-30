import { createServer } from 'node:net'
import { defineConfig, devices } from '@playwright/test'

// The backend and the built PWA, as a phone sees them. Run through
// `pnpm test:e2e` so the backend is pointed at the run's sealed vivarium.

/** A port nothing is listening on right now. */
function freePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const server = createServer()
    server.once('error', reject)
    server.listen(0, '127.0.0.1', () => {
      const { port } = server.address() as { port: number }
      server.close(() => resolve(port))
    })
  })
}

// Chosen once in the main process; the workers that load this config again
// inherit them through the environment, so every process agrees.
process.env.E2E_SERVER_PORT ??= String(await freePort())
process.env.E2E_PREVIEW_PORT ??= String(await freePort())
const SERVER_PORT = process.env.E2E_SERVER_PORT
const PREVIEW_PORT = process.env.E2E_PREVIEW_PORT

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
      env: { PORT: SERVER_PORT, ATPROTO_URL: process.env.VIVARIUM_URL ?? '' },
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
