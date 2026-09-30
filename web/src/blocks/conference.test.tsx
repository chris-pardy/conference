import { act, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, expect, test, vi } from 'vitest'
import { FixtureResolver } from './SourceResolver'
import { block, blocksOfType, card, renderCard } from './test-support'

afterEach(() => {
  vi.useRealTimers()
})

test('TC-26: a person block shows who someone is', () => {
  const resolver = new FixtureResolver(
    {},
    {
      profiles: {
        'did:plc:alice': {
          did: 'did:plc:alice',
          handle: 'alice.test',
          displayName: 'Alice de Vries',
          avatar: 'https://cdn.atmosphereconf.org/avatars/alice.jpg',
        },
        'did:plc:bob': { did: 'did:plc:bob', handle: 'bob.test' },
        'did:plc:carol': FixtureResolver.loading(),
        // did:plc:dave has no profile at all.
      },
    },
  )
  const { container } = renderCard(
    card([
      block('person', { did: 'did:plc:alice' }),
      block('person', { did: 'did:plc:bob' }),
      block('person', { did: 'did:plc:carol' }),
      block('person', { did: 'did:plc:dave' }),
    ]),
    { resolver },
  )

  const [alice, bob, carol, dave] = blocksOfType(container, 'person')

  expect(within(alice).getByText('Alice de Vries')).toBeTruthy()
  expect(alice.querySelector('img')?.getAttribute('src')).toBe('https://cdn.atmosphereconf.org/avatars/alice.jpg')

  expect(within(bob).getByText(/bob\.test/)).toBeTruthy()
  expect(bob.textContent).not.toMatch(/undefined|null/)

  expect(carol.dataset.state).toBe('loading')
  expect(carol.matches('[aria-busy="true"]') || carol.querySelector('[aria-busy="true"]') !== null).toBe(true)

  expect(dave.dataset.state).toBe('unavailable')
  expect(dave.textContent).toMatch(/unavailable/i)
  expect(dave.textContent).not.toContain('did:plc:dave')
})

test('TC-27: session and room blocks show their details on a 24-hour clock', () => {
  const { container } = renderCard(
    card([
      block('sessionRef', {
        title: 'Lexicons in Practice',
        start: '2027-04-30T14:30:00+02:00',
        end: '2027-04-30T15:15:00+02:00',
        room: 'Grote Zaal',
      }),
      block('room', { name: 'Zaal B', detail: '1st floor (Trap)' }),
    ]),
  )

  const [session] = blocksOfType(container, 'sessionRef')
  expect(within(session).getByText('Lexicons in Practice')).toBeTruthy()
  expect(session.textContent).toMatch(/14:30/)
  expect(session.textContent).toMatch(/15:15/)
  expect(session.textContent).toContain('Grote Zaal')
  expect(session.textContent).not.toMatch(/\b(am|pm)\b/i)

  const [room] = blocksOfType(container, 'room')
  expect(room.textContent).toContain('Zaal B')
  expect(room.textContent).toContain('1st floor (Trap)')
})

test('TC-28: a countdown counts down, then says now, then ended', () => {
  vi.useFakeTimers()
  vi.setSystemTime(new Date('2027-04-30T18:58:00+02:00'))

  const { container } = renderCard(
    card([
      block('time', {
        mode: 'countdown',
        label: 'Bitterballen after the talks',
        at: '2027-04-30T19:00:00+02:00',
        end: '2027-04-30T20:00:00+02:00',
      }),
    ]),
  )
  const [countdown] = blocksOfType(container, 'time')
  expect(within(countdown).getByText('Bitterballen after the talks')).toBeTruthy()
  expect(countdown.textContent).toMatch(/\b2\s*min|\b0?2:00\b/i)

  act(() => vi.advanceTimersByTime(60_000))
  expect(countdown.textContent).toMatch(/\b1\s*min|\b0?1:00\b/i)

  act(() => vi.advanceTimersByTime(61_000))
  expect(countdown.textContent).toMatch(/\bnow\b/i)
  expect(countdown.textContent).not.toMatch(/ended/i)

  act(() => vi.advanceTimersByTime(60 * 60_000))
  expect(countdown.textContent).toMatch(/\bended\b/i)
  expect(countdown.textContent).not.toMatch(/\bnow\b/i)
})

test('TC-29: a copyable value copies to the clipboard', async () => {
  const user = userEvent.setup()
  renderCard(
    card([
      block('context', { text: 'Wifi: Atmosphere-Gast' }),
      block('copyable', { label: 'Wifi password', value: 'stroopwafel-2027' }),
    ]),
  )

  await user.click(screen.getByRole('button', { name: /copy/i }))

  expect(await navigator.clipboard.readText()).toBe('stroopwafel-2027')
  expect(screen.getByText(/copied/i)).toBeTruthy()
  // Briefly: it goes away again by itself.
  await waitFor(() => expect(screen.queryByText(/copied/i)).toBeNull(), { timeout: 5_000 })
}, 10_000)
