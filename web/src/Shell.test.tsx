import { fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter, Route, Routes } from 'react-router'
import { afterEach, expect, test, vi } from 'vitest'
import Shell from './Shell'

afterEach(() => {
  vi.unstubAllGlobals()
})

test('a failed sign-out from the expired banner is reported', async () => {
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: string) => {
      if (input.includes('getSession'))
        return Response.json({ error: 'SessionExpired', handle: 'alice.test', did: 'did:plc:alice' }, { status: 401 })
      return new Response(null, { status: 503 })
    }),
  )
  render(
    <MemoryRouter>
      <Routes>
        <Route element={<Shell />}>
          <Route path="/" element={<p>home</p>} />
        </Route>
      </Routes>
    </MemoryRouter>,
  )
  fireEvent.click(await screen.findByRole('button', { name: 'Not you? Sign out' }))
  expect(await screen.findByText(/Couldn’t sign out/)).toBeTruthy()
  expect(screen.getByText('Your session has expired.')).toBeTruthy()
})

function renderShellAt(path: string) {
  render(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        <Route element={<Shell />}>
          <Route path="*" element={<p>page</p>} />
        </Route>
      </Routes>
    </MemoryRouter>,
  )
}

test('the account initial is a whole character, even an emoji', async () => {
  vi.stubGlobal(
    'fetch',
    vi.fn(async () =>
      Response.json({
        did: 'did:plc:ana',
        handle: 'ana.test',
        displayName: '🦋 Ana',
        csrfToken: 't',
        scopes: ['atproto'],
      }),
    ),
  )
  renderShellAt('/')
  expect((await screen.findByText('🦋')).textContent).toBe('🦋')
})

test('signing in comes back to the same place on the page', async () => {
  vi.stubGlobal(
    'fetch',
    vi.fn(async () => Response.json({ error: 'AuthRequired' }, { status: 401 })),
  )
  renderShellAt('/schedule?day=2#talk-3')
  const link = await screen.findByRole('link', { name: 'Sign in' })
  const href = new URL(link.getAttribute('href') ?? '', 'http://app.test')
  expect(href.searchParams.get('return_to')).toBe('/schedule?day=2#talk-3')
})
