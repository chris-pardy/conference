import { afterEach, expect, test, vi } from 'vitest'
import { fetchSession } from './session'

afterEach(() => {
  vi.unstubAllGlobals()
})

const answer = (res: Response) =>
  vi.stubGlobal(
    'fetch',
    vi.fn(async () => res),
  )

test('only the appview’s own refusals read as signed out or expired', async () => {
  answer(Response.json({ error: 'AuthRequired', message: 'Sign in to continue.' }, { status: 401 }))
  expect(await fetchSession()).toEqual({ kind: 'signedOut' })

  answer(Response.json({ error: 'SessionExpired', handle: 'alice.test', did: 'did:plc:alice' }, { status: 401 }))
  expect(await fetchSession()).toEqual({ kind: 'expired', handle: 'alice.test', did: 'did:plc:alice' })
})

test('a proxy’s refusal reads as unavailable, not signed out', async () => {
  for (const status of [400, 401, 403, 404, 408, 429, 503]) {
    answer(new Response('busy', { status }))
    expect(await fetchSession(), String(status)).toEqual({ kind: 'unavailable' })
  }
})
