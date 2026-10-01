import { act, fireEvent, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import jsQR from 'jsqr'
import { afterEach, expect, test, vi } from 'vitest'
import { ActionContext } from './ActionContext'
import { BlockCard } from './BlockCard'
import {
  FixtureResolver,
  type Observable,
  type SourceRef,
  type SourceResolver,
  SourceResolverContext,
  type SourceState,
} from './SourceResolver'
import { binding, block, blocksOfType, CARD_REF, card, recordSource, renderCard } from './test-support'

// Cases found in adversarial review, beyond the frozen tests.

afterEach(() => {
  vi.useRealTimers()
})

test('TC-32: a sheet opened from a compact card shows its inputs', async () => {
  const user = userEvent.setup()
  const { onAction } = renderCard(
    card([
      block('header', { text: 'Wifi' }),
      block('button', {
        id: 'details',
        label: 'Details',
        variant: 'primary',
        opens: {
          title: 'Wifi details',
          blocks: [
            block('image', { url: 'https://atmosphereconf.org/img/map.png', alt: 'Floor plan' }),
            block('textInput', { id: 'problem', label: 'Report a problem' }),
            block('submit', { id: 'report', label: 'Send', action: 'report' }),
          ],
        },
      }),
    ]),
    { surface: 'compact' },
  )

  await user.click(screen.getByRole('button', { name: 'Details' }))
  const sheet = screen.getByRole('dialog', { name: 'Wifi details' })
  expect(within(sheet).getByRole('img', { name: 'Floor plan' })).toBeTruthy()
  await user.type(within(sheet).getByRole('textbox', { name: 'Report a problem' }), 'Geen signaal')
  await user.click(within(sheet).getByRole('button', { name: 'Send' }))
  expect(onAction).toHaveBeenCalledWith({
    card: CARD_REF,
    blockId: 'report',
    actionId: 'report',
    value: { problem: 'Geen signaal' },
  })
})

test('TC-32: focus stays inside an open sheet, and Escape closes it', async () => {
  const user = userEvent.setup()
  renderCard(
    card([
      block('button', { id: 'after', label: 'Behind the sheet', action: 'x' }),
      block('button', {
        id: 'details',
        label: 'Details',
        opens: { title: 'Details', blocks: [block('textInput', { id: 'n', label: 'Note' })] },
      }),
    ]),
  )
  await user.click(screen.getByRole('button', { name: 'Details' }))
  const sheet = screen.getByRole('dialog')
  for (let i = 0; i < 5; i++) {
    await user.tab()
    expect(sheet.contains(document.activeElement), 'focus should stay in the sheet').toBe(true)
  }
  await user.keyboard('{Escape}')
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Details' }))
})

test('TC-31: a compact card keeps exactly one primary button, even when buttons are nested', () => {
  renderCard(
    card([
      block('stack', {
        blocks: [
          block('header', { text: 'Keukenhof' }),
          block('button', { id: 'a', label: 'Join', action: 'join', variant: 'primary' }),
        ],
      }),
      block('button', { id: 'b', label: 'Also join', action: 'join', variant: 'primary' }),
    ]),
    { surface: 'compact' },
  )
  expect(screen.getAllByRole('button').map((b) => b.textContent)).toEqual(['Join'])
})

test('TC-22: an optional input holding only spaces is not sent', async () => {
  const user = userEvent.setup()
  const { onAction } = renderCard(
    card([
      block('textInput', { id: 'q', label: 'Question', minLength: 5 }),
      block('submit', { id: 's', label: 'Ask', action: 'ask' }),
    ]),
  )
  await user.type(screen.getByRole('textbox', { name: 'Question' }), '   ')
  await user.click(screen.getByRole('button', { name: 'Ask' }))
  expect(onAction).toHaveBeenCalledWith({ card: CARD_REF, blockId: 's', actionId: 'ask', value: {} })
})

test('TC-7: an image that failed tries again when its bound URL changes', () => {
  const resolver = new FixtureResolver({ img: { url: 'https://atmosphereconf.org/a.png' } })
  const { container } = renderCard(
    card([block('image', { alt: 'Venue', bind: { url: binding('img', '/url') } })], { sources: [recordSource('img')] }),
    { resolver },
  )
  fireEvent.error(screen.getByRole('img', { name: 'Venue' }))
  expect(container.querySelector('img')).toBeNull()

  act(() => resolver.set('img', { url: 'https://atmosphereconf.org/b.png' }))
  expect(screen.getByRole('img', { name: 'Venue' }).getAttribute('src')).toBe('https://atmosphereconf.org/b.png')
})

test('TC-28: a countdown stops ticking once it has ended', () => {
  vi.useFakeTimers()
  vi.setSystemTime(new Date('2027-04-30T21:00:00+02:00'))
  renderCard(
    card([
      block('time', {
        mode: 'countdown',
        at: '2027-04-30T19:00:00+02:00',
        end: '2027-04-30T20:00:00+02:00',
      }),
    ]),
  )
  expect(screen.getByText(/ended/)).toBeTruthy()
  expect(vi.getTimerCount()).toBe(0)
})

test('TC-28: a countdown far away re-renders once a minute, not every second', () => {
  vi.useFakeTimers()
  vi.setSystemTime(new Date('2027-04-30T12:00:00+02:00'))
  const { container } = renderCard(card([block('time', { mode: 'countdown', at: '2027-04-30T19:00:00+02:00' })]))
  const [time] = blocksOfType(container, 'time')
  expect(time.textContent).toMatch(/in 7 h/)
  act(() => vi.advanceTimersByTime(30_000))
  expect(time.textContent).toMatch(/in 7 h/)
  act(() => vi.advanceTimersByTime(60_000))
  expect(time.textContent).toMatch(/in 6 h 59 min/)
})

/** A resolver that counts watches and answers only when told to. */
class CountingResolver implements SourceResolver {
  watches = 0
  watch(_card: unknown, _name: string, _ref: SourceRef): Observable<SourceState> {
    this.watches++
    return {
      subscribe: (next) => {
        next({ state: 'ready', value: { total: 412 } })
        return { unsubscribe() {} }
      },
    }
  }
}

test('TC-15: a host that re-parses the same card keeps its sources watched', () => {
  const resolver = new CountingResolver()
  const record = () =>
    card([block('stat', { label: 'attendees', bind: { value: binding('stats', '/total') } })], {
      sources: [recordSource('stats')],
    })
  const view = (r: ReturnType<typeof record>) => (
    <SourceResolverContext value={resolver}>
      <ActionContext value={{ onAction() {} }}>
        <BlockCard cardRef={CARD_REF} card={r} surface="feed" />
      </ActionContext>
    </SourceResolverContext>
  )
  const { rerender } = render(view(record()))
  expect(resolver.watches).toBe(1)
  rerender(view(record()))
  expect(resolver.watches).toBe(1)
  expect(screen.getByText('412')).toBeTruthy()
})

test('TC-19: a button in a list sends its own bound value', async () => {
  const user = userEvent.setup()
  const resolver = new FixtureResolver({
    questions: [
      { uri: 'at://did:plc:alice/q/1', text: 'Recorded?' },
      { uri: 'at://did:plc:bob/q/2', text: 'Bike rack?' },
    ],
  })
  const { onAction } = renderCard(
    card(
      [
        block('list', {
          items: binding('questions'),
          key: '/uri',
          template: [
            block('button', {
              id: 'up',
              action: 'upvote',
              bind: { label: binding('$item', '/text'), value: binding('$item', '/uri') },
            }),
          ],
        }),
      ],
      { sources: [recordSource('questions')] },
    ),
    { resolver },
  )
  await user.click(screen.getByRole('button', { name: 'Bike rack?' }))
  expect(onAction).toHaveBeenCalledWith({
    card: CARD_REF,
    blockId: 'up',
    actionId: 'upvote',
    value: 'at://did:plc:bob/q/2',
  })
})

/** Decodes a rendered QR block by drawing its SVG modules as pixels. */
function decodeQr(root: HTMLElement): string | undefined {
  const svg = root.querySelector('svg') as SVGSVGElement
  const [x0, y0, size] = (svg.getAttribute('viewBox') ?? '').split(' ').map(Number)
  const d = svg.querySelector('path')?.getAttribute('d') ?? ''
  const scale = 4
  const px = size * scale
  const data = new Uint8ClampedArray(px * px * 4).fill(255)
  for (const [, c, r] of d.matchAll(/M(\d+) (\d+)/g)) {
    for (let dy = 0; dy < scale; dy++) {
      for (let dx = 0; dx < scale; dx++) {
        const i = (((Number(r) - y0) * scale + dy) * px + (Number(c) - x0) * scale + dx) * 4
        data[i] = 0
        data[i + 1] = 0
        data[i + 2] = 0
      }
    }
  }
  return jsQR(data, px, px)?.data
}

test('TC-30: a QR code encodes non-ASCII values as UTF-8', () => {
  const values = ['WIFI:S:Atmosphère-Gast;T:WPA;P:geheim;;', 'Café de Jaren 🍻']
  const { container } = renderCard(card(values.map((value) => block('qr', { value }))))
  expect(blocksOfType(container, 'qr').map(decodeQr)).toEqual(values)
})

test('TC-28: a countdown with a far-off end never sets a timer past what setTimeout can hold', () => {
  vi.useFakeTimers()
  vi.setSystemTime(new Date('2027-04-30T18:00:00+02:00'))
  const spy = vi.spyOn(globalThis, 'setTimeout')
  renderCard(
    card([block('time', { mode: 'countdown', at: '2027-04-30T17:00:00+02:00', end: '2027-06-01T00:00:00+02:00' })]),
  )
  act(() => vi.advanceTimersByTime(1000))
  const delays = spy.mock.calls.map(([, delay]) => delay ?? 0)
  expect(delays.length).toBeLessThan(5)
  for (const delay of delays) expect(delay).toBeLessThanOrEqual(2 ** 31 - 1)
  spy.mockRestore()
})

test('TC-15: a block that failed on bad data recovers when its source changes', () => {
  const resolver = new FixtureResolver({
    stats: {
      get total() {
        throw new Error('bad data')
      },
    },
  })
  const errors = vi.spyOn(console, 'error').mockImplementation(() => {})
  const { container } = renderCard(
    card(
      [
        block('header', { text: 'Aanwezig' }),
        block('stat', { label: 'attendees', bind: { value: binding('stats', '/total') } }),
      ],
      { sources: [recordSource('stats')] },
    ),
    { resolver },
  )
  expect(blocksOfType(container, 'stat')[0].dataset.state).toBe('unavailable')

  act(() => resolver.set('stats', { total: 413 }))
  const [stat] = blocksOfType(container, 'stat')
  expect(stat.dataset.state).toBeUndefined()
  expect(within(stat).getByText('413')).toBeTruthy()
  errors.mockRestore()
})
