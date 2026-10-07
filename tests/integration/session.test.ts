import { startVivarium } from '@vivarium-dev/client'
import { expect, test } from '@vivarium-dev/client/vitest'
import { getSession, signIn, sleep, storedTokens } from '../support/auth.ts'
import { SIGN_IN_SCOPES } from '../support/scopes.ts'
import { freePort, spawnServer, tempDatabase } from '../support/server.ts'

// Renewal settings that make every run refresh: access tokens live an hour,
// so a two-hour skew always counts as "about to expire".
const RENEW_EVERY_SECOND = { TOKEN_RENEW_INTERVAL: '1s', TOKEN_REFRESH_SKEW: '2h' }

/** Env for a server that can be restarted with the same client ID and database. */
async function restartable(extra: Record<string, string> = {}) {
  const port = String(await freePort())
  return { PORT: port, PUBLIC_URL: `http://127.0.0.1:${port}`, DATABASE_URL: tempDatabase().databaseUrl, ...extra }
}

/** Polls until `check` holds, or fails after `ms`. */
async function eventually(check: () => Promise<boolean> | boolean, ms: number, what: string) {
  const until = Date.now() + ms
  while (Date.now() < until) {
    if (await check()) return
    await sleep(250)
  }
  throw new Error(`timed out after ${ms}ms waiting until ${what}`)
}

test('TC-11: a session survives a backend restart, and can still be renewed', async ({ viv }) => {
  const ana = await viv.createAccount(viv.handle('ana'))
  const env = await restartable()
  let server = await spawnServer(env)
  let flow: Awaited<ReturnType<typeof signIn>>
  try {
    flow = await signIn(server.url, ana.handle)
    expect((await getSession(server.url, flow.jar)).status).toBe(200)
  } finally {
    await server.stop()
  }

  server = await spawnServer({ ...env, ...RENEW_EVERY_SECOND })
  try {
    const { status, body } = await getSession(server.url, flow.jar)
    expect(status).toBe(200)
    expect(body.did).toBe(ana.did)

    const before = storedTokens(server.databasePath as string, ana.did)?.accessToken
    expect(before).toBeTypeOf('string')
    await eventually(
      () => storedTokens(server.databasePath as string, ana.did)?.accessToken !== before,
      10_000,
      'the restarted backend renews the session',
    )
    expect((await getSession(server.url, flow.jar)).status).toBe(200)
  } finally {
    await server.stop()
  }
})

test('TC-12: tokens are renewed in the background', async ({ viv }) => {
  const ana = await viv.createAccount(viv.handle('ana'))
  const server = await spawnServer(RENEW_EVERY_SECOND)
  try {
    const flow = await signIn(server.url, ana.handle)
    const first = storedTokens(server.databasePath as string, ana.did)
    expect(first).toBeDefined()

    // Ana does nothing; the renewer runs on its own.
    await eventually(
      () => storedTokens(server.databasePath as string, ana.did)?.accessToken !== first?.accessToken,
      10_000,
      'the access token is renewed',
    )
    const renewed = storedTokens(server.databasePath as string, ana.did)
    expect(renewed?.refreshToken).not.toBe(first?.refreshToken)
    expect((await getSession(server.url, flow.jar)).status).toBe(200)
  } finally {
    await server.stop()
  }
})

test('TC-13: two renewals at once don’t sign anyone out', async ({ viv }) => {
  const ana = await viv.createAccount(viv.handle('ana'))
  const env = await restartable(RENEW_EVERY_SECOND)
  const first = await spawnServer(env)
  // A second instance on the same database, with the same client ID.
  const second = await spawnServer({ ...env, PORT: '0' })
  try {
    const flow = await signIn(first.url, ana.handle)
    const start = storedTokens(first.databasePath as string, ana.did)
    expect(start).toBeDefined()

    // Several renewal runs on both instances.
    await sleep(6_000)

    const now = storedTokens(first.databasePath as string, ana.did)
    expect(now, 'the session should still hold tokens').toBeDefined()
    expect(now?.accessToken).not.toBe(start?.accessToken)
    for (const server of [first, second]) {
      const { status, body } = await getSession(server.url, flow.jar)
      expect(status, `getSession on ${server.url}`).toBe(200)
      expect(body.did).toBe(ana.did)
    }
  } finally {
    await Promise.all([first.stop(), second.stop()])
  }
})

test('TC-14: an unreachable authorization server doesn’t sign anyone out', async () => {
  // A box of its own, so stopping it leaves the shared one alone.
  const box = await startVivarium({ upstream: false, attach: false })
  let stopped = false
  const server = await spawnServer({ ATPROTO_URL: box.url, ...RENEW_EVERY_SECOND })
  try {
    const ana = await box.createAccount('ana-tc14.vivarium.test')
    const flow = await signIn(server.url, ana.handle)
    expect((await getSession(server.url, flow.jar)).status).toBe(200)

    await box.stop()
    stopped = true
    // Several renewal runs that can't reach the authorization server.
    await sleep(5_000)

    const { status, body } = await getSession(server.url, flow.jar)
    expect(status).toBe(200)
    expect(body.did).toBe(ana.did)
  } finally {
    await server.stop()
    if (!stopped) await box.stop()
  }
})

test('TC-16: an idle session expires, and remembers who it was', async ({ viv }) => {
  const ana = await viv.createAccount(viv.handle('ana'))
  const server = await spawnServer({ SESSION_IDLE_TIMEOUT: '2s' })
  try {
    const flow = await signIn(server.url, ana.handle)
    expect((await getSession(server.url, flow.jar)).status).toBe(200)

    await sleep(3_500)
    const { status, body } = await getSession(server.url, flow.jar)
    expect(status).toBe(401)
    expect(body.error).toBe('SessionExpired')
    // Enough to sign the same person back in without retyping their handle.
    expect(body.handle).toBe(ana.handle)
  } finally {
    await server.stop()
  }
})

test('TC-17: a revoked account is noticed without the person doing anything', async ({ viv }) => {
  const ana = await viv.createAccount(viv.handle('ana'))
  const server = await spawnServer(RENEW_EVERY_SECOND)
  try {
    const flow = await signIn(server.url, ana.handle)
    expect((await getSession(server.url, flow.jar)).status).toBe(200)

    await viv.deleteAccount(ana.did)
    // The renewer finds out on its own; the session holds no tokens any more.
    await eventually(
      () => storedTokens(server.databasePath as string, ana.did) === undefined,
      10_000,
      'the renewer ends the session',
    )
    const { status, body } = await getSession(server.url, flow.jar)
    expect(status).toBe(401)
    expect(body.error).toBe('SessionExpired')
  } finally {
    await server.stop()
  }
})

test('TC-18: growing the sign-in scopes asks people to sign in again', async ({ viv }) => {
  const ana = await viv.createAccount(viv.handle('ana'))
  const env = await restartable()
  let server = await spawnServer(env)
  let jar: Awaited<ReturnType<typeof signIn>>['jar']
  try {
    jar = (await signIn(server.url, ana.handle)).jar
    expect((await getSession(server.url, jar)).body.scopes).toEqual(SIGN_IN_SCOPES)
  } finally {
    await server.stop()
  }

  server = await spawnServer({ ...env, OAUTH_SCOPES: 'atproto transition:generic' })
  try {
    const expired = await getSession(server.url, jar)
    expect(expired.status).toBe(401)
    expect(expired.body.error).toBe('SessionExpired')

    await signIn(server.url, ana.handle, { jar })
    const { status, body } = await getSession(server.url, jar)
    expect(status).toBe(200)
    expect([...body.scopes].sort()).toEqual(['atproto', 'transition:generic'])
  } finally {
    await server.stop()
  }
})
