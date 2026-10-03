import { afterEach, expect, test, vi } from 'vitest'
import { api, connectSession } from './api'
import type { Session } from './auth/session'

afterEach(() => {
  vi.unstubAllGlobals()
})

const user = (did: string, csrfToken: string) => ({ did, handle: 'alice.test', csrfToken })
const refused = () => Response.json({ error: 'InvalidCsrfToken', message: 'stale' }, { status: 403 })

function connect(did: string, csrfToken: string) {
  const changed = vi.fn<(session: Session) => void>()
  connectSession({ csrfToken, did, expired: () => {}, changed })
  return changed
}

test('a stale CSRF token is renewed and the request retried once', async () => {
  const changed = connect('did:plc:alice', 'old')
  const sent: (string | null)[] = []
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: string, init: RequestInit) => {
      if (input.includes('getSession')) return Response.json(user('did:plc:alice', 'new'))
      const token = new Headers(init.headers).get('X-CSRF-Token')
      sent.push(token)
      return token === 'new' ? Response.json({}) : refused()
    }),
  )

  const res = await api('/oauth/logout', { method: 'POST' })

  expect(res.status).toBe(200)
  expect(sent).toEqual(['old', 'new'])
  expect(changed).toHaveBeenCalledWith(expect.objectContaining({ kind: 'signedIn' }))
})

test('a stale CSRF token is not retried for a different account', async () => {
  const changed = connect('did:plc:alice', 'old')
  const sent: (string | null)[] = []
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: string, init: RequestInit) => {
      if (input.includes('getSession')) return Response.json(user('did:plc:bob', 'new'))
      sent.push(new Headers(init.headers).get('X-CSRF-Token'))
      return refused()
    }),
  )

  const res = await api('/oauth/logout', { method: 'POST' })

  expect(res.status).toBe(403)
  expect(sent).toEqual(['old'])
  expect(changed).toHaveBeenCalledWith({ kind: 'signedIn', user: user('did:plc:bob', 'new') })
})
