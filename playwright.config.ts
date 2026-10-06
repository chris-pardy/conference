import { mkdtempSync } from 'node:fs'
import { createServer } from 'node:net'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
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
// The backend's database, fresh for each run and shared by every worker.
process.env.E2E_DATABASE_PATH ??= join(mkdtempSync(join(tmpdir(), 'eventside-e2e-')), 'eventside.db')
const VIVARIUM_URL = process.env.VIVARIUM_URL
if (!VIVARIUM_URL) {
  throw new Error('Playwright needs the run vivarium in VIVARIUM_URL: use `pnpm test:e2e`')
}
const SERVER_PORT = process.env.E2E_SERVER_PORT
const PREVIEW_PORT = process.env.E2E_PREVIEW_PORT
// One origin for the app, the cookie and the OAuth client ID: vivarium accepts
// client IDs on 127.0.0.1, so the browser uses that rather than localhost.
const APP_ORIGIN = `http://127.0.0.1:${PREVIEW_PORT}`

export default defineConfig({
  testDir: './e2e',
  forbidOnly: !!process.env.CI,
  reporter: [['list'], ['html', { open: 'never' }]],
  use: {
    ...devices['Pixel 7'],
    baseURL: APP_ORIGIN,
    serviceWorkers: 'block',
    trace: 'retain-on-failure',
    video: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
  projects: [{ name: 'chromium', use: { browserName: 'chromium' } }],
  webServer: [
    {
      command: 'target/debug/conference-server',
      env: {
        PORT: SERVER_PORT,
        ATPROTO_URL: VIVARIUM_URL,
        PUBLIC_URL: APP_ORIGIN,
        DATABASE_URL: `sqlite://${process.env.E2E_DATABASE_PATH}?mode=rwc`,
        SIGNUP_PDS_URL: VIVARIUM_URL,
        ALLOW_PRIVATE_NETWORK: 'true',
        // Short enough for a test to wait out; no other test idles this long.
        SESSION_IDLE_TIMEOUT: '10s',
      },
      url: `http://127.0.0.1:${SERVER_PORT}/health`,
      reuseExistingServer: false,
    },
    {
      command: `pnpm --filter @conference/web preview --host 127.0.0.1 --port ${PREVIEW_PORT} --strictPort`,
      env: { SERVER_URL: `http://127.0.0.1:${SERVER_PORT}` },
      url: APP_ORIGIN,
      reuseExistingServer: false,
    },
  ],
})
