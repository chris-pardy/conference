import { spawn } from 'node:child_process'
import { expect, test } from '@vivarium-dev/client/vitest'
import { getSession, open, signIn } from '../support/auth.ts'
import { freePort, SERVER_BIN, spawnServer, tempDatabase } from '../support/server.ts'

/** Env for instances that share one client ID and database, as in a rolling deploy. */
async function shared(extra: Record<string, string> = {}) {
  const port = String(await freePort())
  return { PORT: port, PUBLIC_URL: `http://127.0.0.1:${port}`, DATABASE_URL: tempDatabase().databaseUrl, ...extra }
}

test('TC-18: a sign-in pushed before the scopes grew is finished, then asked to sign in again', async ({ viv }) => {
  const ana = await viv.createAccount(viv.handle('ana'))
  const env = await shared()
  let server = await spawnServer(env)
  let flow: Awaited<ReturnType<typeof signIn>>
  try {
    flow = await signIn(server.url, ana.handle, { stopAtCallback: true })
  } finally {
    await server.stop()
  }
  expect(flow.callbackUrl).toBeDefined()

  // The callback reaches an instance that asks for more than the request was pushed with.
  server = await spawnServer({ ...env, OAUTH_SCOPES: 'atproto transition:generic' })
  try {
    const { location } = await open(flow.jar, flow.callbackUrl as string)
    expect(location).toBeDefined()
    expect(new URL(location as string).searchParams.get('error')).toBeNull()

    const expired = await getSession(server.url, flow.jar)
    expect(expired.status).toBe(401)
    expect(expired.body.error).toBe('SessionExpired')
  } finally {
    await server.stop()
  }
})

test('TC-25: client metadata is served only at a URL that is its client ID', async () => {
  const server = await spawnServer()
  try {
    const metadata = (query: string) => fetch(`${server.url}/oauth-client-metadata.json${query}`)
    for (const query of ['', '?scope=atproto%20transition%3Ageneric']) {
      const res = await metadata(query)
      expect(res.status, query).toBe(200)
      expect((await res.json()).client_id).toBe(`${server.url}/oauth-client-metadata.json${query}`)
    }
    for (const query of [
      '?scope=atproto',
      '?scope=atproto+transition:generic',
      '?scope=atproto%20transition%3Ageneric&x=1',
      '?scope=atproto%20transition%3Ageneric&scope=atproto',
    ]) {
      expect((await metadata(query)).status, query).toBe(404)
    }
  } finally {
    await server.stop()
  }
})

/** Starts the backend with a bad setting and resolves with its exit code. */
function exitCode(env: Record<string, string>): Promise<number | null> {
  return new Promise((done, reject) => {
    const child = spawn(SERVER_BIN, [], {
      env: { ...process.env, PORT: '0', DATABASE_URL: tempDatabase().databaseUrl, ...env },
      stdio: 'ignore',
    })
    const timer = setTimeout(() => {
      child.kill()
      reject(new Error(`the backend didn't exit with ${JSON.stringify(env)}`))
    }, 10_000)
    child.on('exit', (code) => {
      clearTimeout(timer)
      done(code)
    })
  })
}

test('TC-27: a misconfigured backend exits with the configuration error code', async () => {
  const settings: Record<string, string>[] = [
    { DATABASE_URL: 'mysql://eventside@127.0.0.1:9/eventside' },
    { DATABASE_URL: 'sqlite::memory:' },
    { OAUTH_SIGNING_KEY: '{"kty":"EC"}' },
    { PUBLIC_URL: 'https://app.example/eventside' },
    { PUBLIC_URL: 'app.example' },
  ]
  for (const env of settings) {
    expect(await exitCode(env), JSON.stringify(env)).toBe(2)
  }
})
