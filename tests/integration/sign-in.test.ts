import { createServer, type IncomingMessage } from 'node:http'
import type { AddressInfo } from 'node:net'
import { expect, test } from '@vivarium-dev/client/vitest'
import { inject } from 'vitest'
import {
  CookieJar,
  getSession,
  giveProfile,
  open,
  sessionCookie,
  signIn,
  signOut,
  signUp,
  storedTokens,
  tokenActive,
} from '../support/auth.ts'
import { SIGN_IN_SCOPES } from '../support/scopes.ts'
import { spawnServer } from '../support/server.ts'

const serverUrl = () => inject('serverUrl')

/** The path (and query) a redirect landed on, relative to the server. */
function landedOn(location: string | undefined): string {
  expect(location, 'the appview should redirect the browser').toBeTypeOf('string')
  const url = new URL(location as string)
  expect(url.origin).toBe(serverUrl())
  return url.pathname + url.search
}

/** Asserts a redirect to the sign-in page with an error, and returns the error code. */
function signInError(location: string | undefined, base = serverUrl()): string {
  expect(location, 'the appview should redirect to its sign-in page').toBeTypeOf('string')
  const url = new URL(location as string)
  expect(url.origin).toBe(base)
  expect(url.pathname).toBe('/signin')
  const error = url.searchParams.get('error')
  expect(error, 'the sign-in page should be told what went wrong').toBeTruthy()
  return error as string
}

test('TC-2: signing in returns to the page it started from', async ({ viv }) => {
  const ana = await viv.createAccount(viv.handle('ana'))
  const flow = await signIn(serverUrl(), ana.handle, { returnTo: '/dev/blocks?card=3' })
  expect(landedOn(flow.location)).toBe('/dev/blocks?card=3')
})

test('TC-4: an unknown handle is reported, not sent anywhere', async ({ viv }) => {
  const flow = await signIn(serverUrl(), viv.handle('nobody'))
  expect(flow.callbackUrl, 'the browser should never reach an authorization server').toBeUndefined()
  expect(signInError(flow.location)).toBe('handle_not_found')
  expect((await getSession(serverUrl(), flow.jar)).status).toBe(401)
})

test('TC-5: declining consent leaves the person signed out', async ({ viv }) => {
  const ana = await viv.createAccount(viv.handle('ana'))
  const flow = await signIn(serverUrl(), ana.handle, { decision: 'deny' })
  expect(signInError(flow.location)).toBe('access_denied')
  expect(sessionCookie(serverUrl(), flow.jar)).toBeUndefined()
  expect((await getSession(serverUrl(), flow.jar)).body.error).toBe('AuthRequired')
})

test('TC-6: picking a different account at the PDS is refused', async ({ viv }) => {
  const ana = await viv.createAccount(viv.handle('ana'))
  const bram = await viv.createAccount(viv.handle('bram'))
  const flow = await signIn(serverUrl(), ana.handle, { account: bram.handle })
  expect(signInError(flow.location)).toBe('account_mismatch')
  expect(sessionCookie(serverUrl(), flow.jar)).toBeUndefined()
  expect((await getSession(serverUrl(), flow.jar)).status).toBe(401)
})

test('TC-7: who is signed in, as the app sees it', async ({ viv }) => {
  const ana = await viv.createAccount(viv.handle('ana'))
  await giveProfile(viv.url, ana, 'Ana de Vries')
  const flow = await signIn(serverUrl(), ana.handle)

  const { status, body } = await getSession(serverUrl(), flow.jar)
  expect(status).toBe(200)
  expect(body).toMatchObject({ did: ana.did, handle: ana.handle, displayName: 'Ana de Vries', scopes: SIGN_IN_SCOPES })
  expect(body.avatar).toMatch(/^https?:\/\//)
  expect(body.csrfToken).toBeTypeOf('string')
  expect(body.csrfToken.length).toBeGreaterThanOrEqual(16)

  // Signed out, unknown and malformed cookies all mean "sign in", never "expired".
  const nobody = new CookieJar()
  const unknown = new CookieJar()
  unknown.set(serverUrl(), 'session', 'a'.repeat(43))
  const malformed = new CookieJar()
  malformed.set(serverUrl(), 'session', '%%not-a-session%%')
  for (const jar of [nobody, unknown, malformed]) {
    const answer = await getSession(serverUrl(), jar)
    expect(answer.status).toBe(401)
    expect(answer.body.error).toBe('AuthRequired')
  }
})

test('TC-8: a newcomer creates an account and comes back signed in', async ({ viv }) => {
  const handle = viv.handle('newcomer')
  const flow = await signUp(serverUrl(), handle, { returnTo: '/' })
  expect(landedOn(flow.location)).toBe('/')

  const { status, body } = await getSession(serverUrl(), flow.jar)
  expect(status).toBe(200)
  expect(body.handle).toBe(handle)
  expect(body.did).toMatch(/^did:/)
  viv.track(body.did)
})

test('TC-9: creating an account asks the sign-up PDS for its sign-up screen', async () => {
  // A stand-in PDS and authorization server that records the pushed request.
  const pushed: URLSearchParams[] = []
  const stub = createServer(async (req, res) => {
    const base = `http://127.0.0.1:${(stub.address() as AddressInfo).port}`
    const json = (body: unknown, status = 200) => {
      res.writeHead(status, { 'content-type': 'application/json' })
      res.end(JSON.stringify(body))
    }
    if (req.url === '/.well-known/oauth-protected-resource') {
      return json({ resource: base, authorization_servers: [base] })
    }
    if (req.url === '/.well-known/oauth-authorization-server') {
      return json({
        issuer: base,
        authorization_endpoint: `${base}/oauth/authorize`,
        token_endpoint: `${base}/oauth/token`,
        pushed_authorization_request_endpoint: `${base}/oauth/par`,
        require_pushed_authorization_requests: true,
        response_types_supported: ['code'],
        grant_types_supported: ['authorization_code', 'refresh_token'],
        code_challenge_methods_supported: ['S256'],
        token_endpoint_auth_methods_supported: ['none', 'private_key_jwt'],
        token_endpoint_auth_signing_alg_values_supported: ['ES256'],
        dpop_signing_alg_values_supported: ['ES256'],
        scopes_supported: ['atproto', 'transition:generic'],
        authorization_response_iss_parameter_supported: true,
        client_id_metadata_document_supported: true,
        prompt_values_supported: ['none', 'login', 'consent', 'select_account', 'create'],
      })
    }
    if (req.url === '/oauth/par' && req.method === 'POST') {
      pushed.push(new URLSearchParams(await body(req)))
      res.setHeader('dpop-nonce', 'stub-nonce')
      return json({ request_uri: 'urn:ietf:params:oauth:request_uri:req-stub', expires_in: 300 }, 201)
    }
    json({ error: 'not_found' }, 404)
  })
  await new Promise<void>((done) => stub.listen(0, '127.0.0.1', done))
  const stubUrl = `http://127.0.0.1:${(stub.address() as AddressInfo).port}`
  const server = await spawnServer({ SIGNUP_PDS_URL: stubUrl })
  try {
    const start = await new CookieJar().fetch(`${server.url}/oauth/signup`)
    expect(start.status).toBeGreaterThanOrEqual(300)
    expect(start.status).toBeLessThan(400)
    const authorize = new URL(start.headers.get('location') ?? '', server.url)
    expect(authorize.origin + authorize.pathname).toBe(`${stubUrl}/oauth/authorize`)
    expect(authorize.searchParams.get('request_uri')).toBe('urn:ietf:params:oauth:request_uri:req-stub')

    expect(pushed).toHaveLength(1)
    const par = pushed[0]
    expect(par.get('prompt')).toBe('create')
    expect(par.has('login_hint')).toBe(false)
    // The client ID is the metadata document for the default scope list.
    expect(par.get('scope')).toBe(SIGN_IN_SCOPES.join(' '))
    const clientId = par.get('client_id') as string
    expect(clientId.startsWith(`${server.url}/oauth-client-metadata.json`)).toBe(true)
    const metadata = await (await fetch(clientId)).json()
    expect(metadata).toMatchObject({ client_id: clientId, scope: SIGN_IN_SCOPES.join(' ') })
    expect(par.get('code_challenge_method')).toBe('S256')
    expect(par.get('client_assertion_type')).toBe('urn:ietf:params:oauth:client-assertion-type:jwt-bearer')
    expect(par.get('client_assertion')).toBeTruthy()
  } finally {
    await server.stop()
    stub.close()
  }
})

function body(req: IncomingMessage): Promise<string> {
  return new Promise((done) => {
    let data = ''
    req.on('data', (chunk) => {
      data += chunk
    })
    req.on('end', () => done(data))
  })
}

test('TC-15: signing out ends the session and revokes its tokens', async ({ viv }) => {
  const server = await spawnServer()
  try {
    const ana = await viv.createAccount(viv.handle('ana'))
    const flow = await signIn(server.url, ana.handle)
    const { body } = await getSession(server.url, flow.jar)
    expect(body.did).toBe(ana.did)
    const tokens = storedTokens(server.databasePath as string, ana.did)
    expect(tokens, 'the session should hold tokens').toBeDefined()
    const clientId = `${server.url}/oauth-client-metadata.json`
    expect(await tokenActive(viv.url, tokens?.refreshToken as string, clientId)).toBe(true)

    const oldCookie = sessionCookie(server.url, flow.jar)
    const res = await signOut(server.url, flow.jar, body.csrfToken)
    expect(res.ok).toBe(true)
    expect(sessionCookie(server.url, flow.jar), 'signing out should clear the cookie').toBeUndefined()

    // The old cookie means "sign in", not "expired".
    const stale = new CookieJar()
    stale.set(server.url, 'session', oldCookie as string)
    const after = await getSession(server.url, stale)
    expect(after.status).toBe(401)
    expect(after.body.error).toBe('AuthRequired')

    expect(await tokenActive(viv.url, tokens?.refreshToken as string, clientId)).toBe(false)
  } finally {
    await server.stop()
  }
})

test('TC-19: a sign-in started in someone else’s browser can’t be finished in another', async ({ viv }) => {
  const mallory = await viv.createAccount(viv.handle('mallory'))
  const malloryFlow = await signIn(serverUrl(), mallory.handle, { stopAtCallback: true })
  expect(malloryFlow.callbackUrl).toBeTypeOf('string')

  // Ana's browser opens Mallory's callback link.
  const anaJar = new CookieJar()
  const { location } = await open(anaJar, malloryFlow.callbackUrl as string)
  signInError(location)
  expect(sessionCookie(serverUrl(), anaJar)).toBeUndefined()
  expect((await getSession(serverUrl(), anaJar)).status).toBe(401)
})

test('TC-20: signing in replaces whatever session the browser had', async ({ viv }) => {
  const ana = await viv.createAccount(viv.handle('ana'))
  const jar = new CookieJar()

  // A planted cookie is never adopted.
  jar.set(serverUrl(), 'session', 'planted-by-someone-else-0123456789abcdef')
  await signIn(serverUrl(), ana.handle, { jar })
  const first = sessionCookie(serverUrl(), jar)
  expect(first).toBeTypeOf('string')
  expect(first).not.toBe('planted-by-someone-else-0123456789abcdef')

  // Signing in again issues a new session and retires the old one.
  await signIn(serverUrl(), ana.handle, { jar })
  const second = sessionCookie(serverUrl(), jar)
  expect(second).toBeTypeOf('string')
  expect(second).not.toBe(first)
  expect((await getSession(serverUrl(), jar)).body.did).toBe(ana.did)

  for (const old of ['planted-by-someone-else-0123456789abcdef', first as string]) {
    const stale = new CookieJar()
    stale.set(serverUrl(), 'session', old)
    expect((await getSession(serverUrl(), stale)).status).toBe(401)
  }
})

test('TC-21: a callback link only works once', async ({ viv }) => {
  const ana = await viv.createAccount(viv.handle('ana'))
  const flow = await signIn(serverUrl(), ana.handle)
  expect((await getSession(serverUrl(), flow.jar)).status).toBe(200)
  const cookie = sessionCookie(serverUrl(), flow.jar)

  // Again in the same browser: an error, and the existing session is untouched.
  const again = await open(flow.jar, flow.callbackUrl as string)
  signInError(again.location)
  expect(sessionCookie(serverUrl(), flow.jar)).toBe(cookie)

  // In a fresh browser: an error, and no session.
  const fresh = new CookieJar()
  const replay = await open(fresh, flow.callbackUrl as string)
  signInError(replay.location)
  expect(sessionCookie(serverUrl(), fresh)).toBeUndefined()
})

test('TC-22: sign-in never redirects off-site', async ({ viv }) => {
  const ana = await viv.createAccount(viv.handle('ana'))
  const offsite = [
    'https://evil.example/',
    '//evil.example',
    '/\\evil.example',
    '/\t/evil.example',
    'javascript:alert(1)',
    'http://127.0.0.1:9/elsewhere',
  ]
  for (const returnTo of offsite) {
    const flow = await signIn(serverUrl(), ana.handle, { returnTo })
    expect(landedOn(flow.location), `return_to ${JSON.stringify(returnTo)}`).toBe('/')
  }

  // Encoded forms may survive as harmless paths, but must never leave the site,
  // even if something decodes them once more.
  for (const returnTo of ['%2F%2Fevil.example', '/%2F/evil.example', '/%5Cevil.example', '/%2F%2Fevil.example']) {
    const flow = await signIn(serverUrl(), ana.handle, { returnTo })
    const path = landedOn(flow.location)
    const decodedOnce = new URL(decodeURIComponent(path), serverUrl())
    expect(decodedOnce.origin, `return_to ${JSON.stringify(returnTo)}`).toBe(serverUrl())
  }

  // The same check applies where the sign-in page is told where to go back to.
  const failed = await signIn(serverUrl(), viv.handle('nobody'), { returnTo: '//evil.example' })
  signInError(failed.location)
  const echoed = new URL(failed.location as string).searchParams.get('return_to')
  expect([null, '/']).toContain(echoed)
})

test('TC-23: changes without the CSRF token are refused', async ({ viv }) => {
  const ana = await viv.createAccount(viv.handle('ana'))
  const flow = await signIn(serverUrl(), ana.handle)
  const { body } = await getSession(serverUrl(), flow.jar)
  const cookie = sessionCookie(serverUrl(), flow.jar)

  for (const token of [undefined, 'not-the-token', '']) {
    const res = await signOut(serverUrl(), flow.jar, token)
    expect(res.status, `sign-out with token ${JSON.stringify(token)}`).toBe(403)
  }
  expect(sessionCookie(serverUrl(), flow.jar)).toBe(cookie)
  const still = await getSession(serverUrl(), flow.jar)
  expect(still.status).toBe(200)
  expect(still.body.did).toBe(ana.did)
  expect(still.body.csrfToken).toBe(body.csrfToken)
})

test('TC-24: private addresses are refused outside dev', async ({ viv }) => {
  const ana = await viv.createAccount(viv.handle('ana'))
  const server = await spawnServer({ ALLOW_PRIVATE_NETWORK: 'false' })
  try {
    const flow = await signIn(server.url, ana.handle)
    expect(flow.callbackUrl, 'the browser should never be sent to a loopback PDS').toBeUndefined()
    signInError(flow.location, server.url)
    expect(sessionCookie(server.url, flow.jar)).toBeUndefined()
  } finally {
    await server.stop()
  }
})
