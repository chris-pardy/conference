import { act, render, screen } from '@testing-library/react'
import { afterEach, expect, test, vi } from 'vitest'
import { SessionProvider } from './SessionProvider'
import { type SessionContextValue, useSession } from './session'

afterEach(() => {
  vi.unstubAllGlobals()
})

const alice = { did: 'did:plc:alice', handle: 'alice.test', csrfToken: 'token' }
const refused = () => Response.json({ error: 'InvalidCsrfToken', message: 'stale' }, { status: 403 })

/** Signs Alice in, then has sign-out refused, and getSession answer with `after`. */
async function signOutRefused(
  after: () => Response,
): Promise<{ ok: boolean; ctx: SessionContextValue; sessionFetches: number }> {
  let signedIn = false
  let sessionFetches = 0
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: string) => {
      if (!input.includes('getSession')) return refused()
      if (!signedIn) return Response.json(alice)
      sessionFetches++
      return after()
    }),
  )
  let ctx!: SessionContextValue
  function Probe() {
    ctx = useSession()
    return <span>{ctx.session.kind}</span>
  }
  render(
    <SessionProvider>
      <Probe />
    </SessionProvider>,
  )
  await screen.findByText('signedIn')
  signedIn = true
  let ok = false
  await act(async () => {
    ok = await ctx.signOut()
  })
  return { ok, ctx, sessionFetches }
}

test('a refused sign-out is a failure when the appview cannot be asked', async () => {
  const { ok, ctx } = await signOutRefused(() => new Response(null, { status: 503 }))
  expect(ok).toBe(false)
  expect(ctx.session.kind).toBe('signedIn')
})

test('a refused sign-out is a failure when the same account is still signed in', async () => {
  const { ok } = await signOutRefused(() => Response.json(alice))
  expect(ok).toBe(false)
})

test('a refused sign-out asks for the session only once', async () => {
  const { sessionFetches } = await signOutRefused(() => Response.json(alice))
  expect(sessionFetches).toBe(1)
})

test('a refused sign-out succeeds when another account signed in elsewhere', async () => {
  const { ok, ctx } = await signOutRefused(() => Response.json({ ...alice, did: 'did:plc:bob', handle: 'bob.test' }))
  expect(ok).toBe(true)
  expect(ctx.session).toMatchObject({ kind: 'signedIn', user: { did: 'did:plc:bob' } })
})

test('a session check still in flight when sign-out completes does not sign the person back in', async () => {
  let answer: ((res: Response) => void) | undefined
  vi.stubGlobal(
    'fetch',
    vi.fn((input: string) => {
      if (!input.includes('getSession')) return Promise.resolve(new Response(null, { status: 200 }))
      if (answer === undefined && screen.queryByText('signedIn') === null) return Promise.resolve(Response.json(alice))
      return new Promise<Response>((resolve) => {
        answer = resolve
      })
    }),
  )
  let ctx!: SessionContextValue
  function Probe() {
    ctx = useSession()
    return <span>{ctx.session.kind}</span>
  }
  render(
    <SessionProvider>
      <Probe />
    </SessionProvider>,
  )
  await screen.findByText('signedIn')
  act(() => {
    window.dispatchEvent(new Event('online'))
  })
  expect(answer).toBeDefined()
  await act(async () => {
    expect(await ctx.signOut()).toBe(true)
  })
  expect(ctx.session.kind).toBe('signedOut')
  await act(async () => {
    answer?.(Response.json(alice))
  })
  expect(ctx.session.kind).toBe('signedOut')
})
