import { spawn } from 'node:child_process'
import { existsSync } from 'node:fs'
import { expect, inject, test } from 'vitest'
import { freePort, SERVER_BIN, spawnServer, tempDatabase } from '../support/server.ts'

test('TC-25: the appview publishes its client metadata and keys', async () => {
  const serverUrl = inject('serverUrl')
  const metadataRes = await fetch(`${serverUrl}/oauth-client-metadata.json`)
  expect(metadataRes.status).toBe(200)
  expect(metadataRes.headers.get('content-type')).toMatch(/application\/json/)
  const metadata = await metadataRes.json()
  expect(metadata).toMatchObject({
    client_id: `${serverUrl}/oauth-client-metadata.json`,
    redirect_uris: [`${serverUrl}/oauth/callback`],
    scope: 'atproto',
    grant_types: expect.arrayContaining(['authorization_code', 'refresh_token']),
    response_types: ['code'],
    token_endpoint_auth_method: 'private_key_jwt',
    token_endpoint_auth_signing_alg: 'ES256',
    dpop_bound_access_tokens: true,
    application_type: 'web',
    jwks_uri: `${serverUrl}/oauth/jwks.json`,
  })

  const jwksRes = await fetch(`${serverUrl}/oauth/jwks.json`)
  expect(jwksRes.status).toBe(200)
  const { keys } = await jwksRes.json()
  expect(keys.length).toBeGreaterThanOrEqual(1)
  for (const key of keys) {
    expect(key).toMatchObject({ kty: 'EC', crv: 'P-256' })
    expect(key.kid).toBeTypeOf('string')
    expect(key, 'only public keys are published').not.toHaveProperty('d')
  }
})

test('TC-26: the signing key stays the same across restarts', async () => {
  const port = String(await freePort())
  const env = { PORT: port, PUBLIC_URL: `http://127.0.0.1:${port}`, DATABASE_URL: tempDatabase().databaseUrl }
  const jwks = async () => {
    const server = await spawnServer(env)
    try {
      const res = await fetch(`${server.url}/oauth/jwks.json`)
      expect(res.status).toBe(200)
      return await res.json()
    } finally {
      await server.stop()
    }
  }
  const first = await jwks()
  const second = await jwks()
  expect(first.keys?.length).toBeGreaterThanOrEqual(1)
  expect(second).toEqual(first)
})

/** Runs the server to completion (or until it announces itself), capturing output. */
function startOnce(databaseUrl: string): Promise<{ listening: boolean; code: number | null; output: string }> {
  return new Promise((done) => {
    const child = spawn(SERVER_BIN, [], {
      env: { ...process.env, PORT: '0', ATPROTO_URL: 'http://127.0.0.1:9', DATABASE_URL: databaseUrl },
      stdio: ['ignore', 'pipe', 'pipe'],
    })
    let output = ''
    const timer = setTimeout(() => {
      child.kill()
      done({ listening: false, code: null, output })
    }, 25_000)
    const collect = (d: Buffer) => {
      output += d
      if (/listening on http/.test(output)) {
        clearTimeout(timer)
        child.kill()
        done({ listening: true, code: null, output })
      }
    }
    child.stdout.on('data', collect)
    child.stderr.on('data', collect)
    child.on('exit', (code) => {
      clearTimeout(timer)
      done({ listening: /listening on http/.test(output), code, output })
    })
  })
}

test('TC-27: the backend picks its database from its URL', { timeout: 60_000 }, async () => {
  const sqlite = tempDatabase()
  const onSqlite = await startOnce(sqlite.databaseUrl)
  expect(onSqlite.listening, onSqlite.output).toBe(true)
  expect(existsSync(sqlite.databasePath), 'the SQLite database file should be created').toBe(true)

  // Nothing listens on the discard port: the backend tries Postgres, and says so.
  const onPostgres = await startOnce('postgres://eventside@127.0.0.1:9/eventside')
  expect(onPostgres.listening).toBe(false)
  expect(onPostgres.code, 'it should exit rather than hang').not.toBeNull()
  expect(onPostgres.code).not.toBe(0)
  expect(onPostgres.output).toMatch(/postgres/i)

  const unsupported = await startOnce('mysql://eventside@127.0.0.1:9/eventside')
  expect(unsupported.listening).toBe(false)
  expect(unsupported.code).not.toBe(0)
  expect(unsupported.output).toMatch(/sqlite/i)
  expect(unsupported.output).toMatch(/postgres/i)
})

test('TC-28: the backend starts without atproto, on a fresh database', async () => {
  const server = await spawnServer({ ATPROTO_URL: 'http://127.0.0.1:9' })
  try {
    const res = await fetch(`${server.url}/health`)
    expect(res.status).toBe(200)
    expect(await res.json()).toEqual({ status: 'up', atproto: 'unreachable' })
    // Startup set up the fresh database and the signing key without atproto.
    expect(existsSync(server.databasePath as string)).toBe(true)
    expect((await fetch(`${server.url}/oauth/jwks.json`)).status).toBe(200)
  } finally {
    await server.stop()
  }
})
