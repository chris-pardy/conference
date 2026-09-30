import { act, render, screen } from '@testing-library/react'
import { afterEach, expect, test, vi } from 'vitest'
import App from './App'

afterEach(() => {
  vi.unstubAllGlobals()
})

test('TC-6: the placeholder page shows backend health', async () => {
  let answer!: (res: Response) => void
  const fetchMock = vi.fn(() => new Promise<Response>((resolve) => (answer = resolve)))
  vi.stubGlobal('fetch', fetchMock)

  render(<App />)

  // Before the answer arrives: a loading state.
  expect(screen.getByText(/checking/i)).toBeTruthy()
  expect(fetchMock).toHaveBeenCalledWith(expect.stringMatching(/\/health$/), expect.anything())

  await act(async () => {
    answer(Response.json({ status: 'up', atproto: 'reachable' }))
  })

  expect(await screen.findByText(/backend.*up/i)).toBeTruthy()
  expect(await screen.findByText(/atproto.*reachable/i)).toBeTruthy()
  expect(screen.queryByText(/checking/i)).toBeNull()
})
