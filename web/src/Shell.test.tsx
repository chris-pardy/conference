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
