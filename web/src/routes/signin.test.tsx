import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { expect, test } from 'vitest'
import SignIn from './signin'

function alertFor(error: string): string | null {
  render(
    <MemoryRouter initialEntries={[`/signin?${new URLSearchParams({ error })}`]}>
      <SignIn />
    </MemoryRouter>,
  )
  return screen.getByRole('alert').textContent
}

test('a known error code shows its message', () => {
  expect(alertFor('access_denied')).toBe('Sign-in was cancelled.')
})

test.each(['__proto__', 'constructor', 'toString', 'hasOwnProperty'])(
  'an error code naming an inherited key (%s) shows the fallback message',
  (error) => {
    expect(alertFor(error)).toBe('Sign-in didn’t work. Please try again.')
  },
)
