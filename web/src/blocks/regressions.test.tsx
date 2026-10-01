import { readFileSync } from 'node:fs'
import { join } from 'node:path'
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
import { parseDatetime } from './time'

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
    item: 'at://did:plc:bob/q/2',
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

test('TC-27: a time with no offset is wall-clock time in the card zone, whatever the device zone', () => {
  // Tokyo is never the test machine's zone here, so the device can't line up by chance.
  const { container } = renderCard(
    card(
      [
        block('sessionRef', { title: 'Keynote', start: '2027-04-30T19:00:00', end: '2027-04-30T19:45:00' }),
        block('time', { label: 'Doors', at: '2027-04-30T08:30:00' }),
      ],
      { timeZone: 'Asia/Tokyo' },
    ),
  )
  expect(blocksOfType(container, 'sessionRef')[0].textContent).toMatch(/19:00–19:45/)
  expect(blocksOfType(container, 'time')[0].textContent).toMatch(/08:30/)
})

test('TC-27: a wall-clock time just after a daylight-saving change still lands right', () => {
  // Europe/Amsterdam moves to summer time at 02:00 on 28 March 2027.
  const { container } = renderCard(
    card([block('time', { label: 'Early train', at: '2027-03-28T03:15:00' })], { timeZone: 'Europe/Amsterdam' }),
  )
  expect(blocksOfType(container, 'time')[0].textContent).toMatch(/03:15/)
})

test('TC-13: a source reloading keeps what was typed in its blocks', async () => {
  const user = userEvent.setup()
  const resolver = new FixtureResolver({ wifi: { label: 'Report a problem' } })
  renderCard(
    card(
      [
        block('button', {
          id: 'report',
          bind: { label: binding('wifi', '/label') },
          opens: { title: 'Report', blocks: [block('textInput', { id: 'problem', label: 'Problem' })] },
        }),
      ],
      { sources: [recordSource('wifi')] },
    ),
    { resolver },
  )
  await user.click(screen.getByRole('button', { name: 'Report a problem' }))
  await user.type(screen.getByRole('textbox', { name: 'Problem' }), 'Is there a bike rack?')

  act(() => resolver.set('wifi', FixtureResolver.loading()))
  act(() => resolver.set('wifi', { label: 'Report a problem' }))

  expect((screen.getByRole('textbox', { name: 'Problem' }) as HTMLInputElement).value).toBe('Is there a bike rack?')
})

test('TC-29: copy falls back when there is no Clipboard API, and says when it cannot copy', async () => {
  const clipboard = Object.getOwnPropertyDescriptor(navigator, 'clipboard')
  Object.defineProperty(navigator, 'clipboard', { value: undefined, configurable: true })
  const execCommand = vi.fn(() => true)
  Object.defineProperty(document, 'execCommand', { value: execCommand, configurable: true })
  try {
    renderCard(card([block('copyable', { label: 'Wifi password', value: 'stroopwafel-2027' })]))
    fireEvent.click(screen.getByRole('button', { name: /copy/i }))
    expect(await screen.findByText('Copied')).toBeTruthy()
    expect(execCommand).toHaveBeenCalledWith('copy')
    expect(window.getSelection()?.toString()).toBe('stroopwafel-2027')

    execCommand.mockReturnValue(false)
    fireEvent.click(screen.getByRole('button', { name: /copy/i }))
    expect(await screen.findByText(/couldn't copy/i)).toBeTruthy()
  } finally {
    if (clipboard) Object.defineProperty(navigator, 'clipboard', clipboard)
    else Reflect.deleteProperty(navigator, 'clipboard')
    Reflect.deleteProperty(document, 'execCommand')
  }
})

test('TC-7: an image over plain http is not loaded', () => {
  const { container } = renderCard(card([block('image', { url: 'http://tracker.example/px.gif', alt: 'Venue' })]))
  expect(container.querySelector('img')).toBeNull()
  expect(screen.getByText('Venue')).toBeTruthy()
})

test('TC-10: a progress bar past its maximum announces a value within range', () => {
  renderCard(card([block('progress', { label: 'Hangers', value: 150, max: 100 })]))
  expect(screen.getByRole('progressbar', { name: 'Hangers' }).getAttribute('aria-valuenow')).toBe('100')
})

test('TC-10: a progress bar bound to a max of zero or less is unavailable', () => {
  const resolver = new FixtureResolver({ hangers: { left: 3, total: -5 } })
  const { container } = renderCard(
    card(
      [
        block('progress', {
          label: 'Hangers',
          bind: { value: binding('hangers', '/left'), max: binding('hangers', '/total') },
        }),
      ],
      { sources: [recordSource('hangers')] },
    ),
    { resolver },
  )
  expect(blocksOfType(container, 'progress')[0].dataset.state).toBe('unavailable')
})

test('TC-13: an edited card (new cid) keeps what was typed in its list items and open sheet', async () => {
  const user = userEvent.setup()
  const resolver = new FixtureResolver({ qs: [{ id: 'q1', text: 'Recorded?' }], wifi: { label: 'Report' } })
  const record = card(
    [
      block('list', {
        items: binding('qs'),
        key: '/id',
        template: [block('textInput', { id: 'answer', label: 'Answer' })],
      }),
      block('button', {
        id: 'report',
        bind: { label: binding('wifi', '/label') },
        opens: { title: 'Report', blocks: [block('textInput', { id: 'problem', label: 'Problem' })] },
      }),
    ],
    { sources: [recordSource('qs'), recordSource('wifi')] },
  )
  const view = (cid: string) => (
    <SourceResolverContext value={resolver}>
      <ActionContext value={{ onAction() {} }}>
        <BlockCard cardRef={{ ...CARD_REF, cid }} card={record} surface="feed" />
      </ActionContext>
    </SourceResolverContext>
  )
  const { rerender } = render(view('bafyreia'))
  await user.type(screen.getByRole('textbox', { name: 'Answer' }), 'hallo')
  await user.click(screen.getByRole('button', { name: 'Report' }))
  await user.type(screen.getByRole('textbox', { name: 'Problem' }), 'in sheet')

  rerender(view('bafyreib'))

  expect((screen.getByRole('textbox', { name: 'Answer' }) as HTMLInputElement).value).toBe('hallo')
  expect((screen.getByRole('textbox', { name: 'Problem' }) as HTMLInputElement).value).toBe('in sheet')
})

test('TC-27: every datetime validation accepts renders as a time', () => {
  const cases: { name: string; card: { createdAt: string }; expect: { ok?: boolean } }[] = JSON.parse(
    readFileSync(join(process.cwd(), 'tests/fixtures/cards/parity.json'), 'utf8'),
  )
  const valid = cases
    .filter((c) => c.name.startsWith('createdAt ') && c.expect.ok)
    .map((c) => c.card.createdAt)
    // The lenient check's month class lets "0," through, which names no month.
    .filter((dt) => !dt.includes('-0,-'))
  expect(valid.length).toBeGreaterThan(20)
  for (const dt of valid) {
    const { container, unmount } = renderCard(
      card([block('time', { label: 'At', at: dt })], { timeZone: 'Europe/Amsterdam' }),
    )
    expect(blocksOfType(container, 'time')[0].dataset.state, dt).toBeUndefined()
    unmount()
  }
  expect(parseDatetime('20270430T190000Z')?.toISOString()).toBe('2027-04-30T19:00:00.000Z')
  expect(parseDatetime('2027-04-30T19:00:00,5Z')?.toISOString()).toBe('2027-04-30T19:00:00.500Z')
  expect(parseDatetime('2027-04-30T1900', 'Europe/Amsterdam')?.toISOString()).toBe('2027-04-30T17:00:00.000Z')
  expect(parseDatetime('2027-04-30T19:00:00+0530')?.toISOString()).toBe('2027-04-30T13:30:00.000Z')
})

test('TC-5: a facet with both a mention and a link makes one link, not nested ones', () => {
  const text = 'ask @alice.test'
  const { container } = renderCard(
    card([
      block('richText', {
        text,
        facets: [
          {
            index: { byteStart: 4, byteEnd: 15 },
            features: [
              { $type: 'app.gather.block.defs#mention', did: 'did:plc:alice' },
              { $type: 'app.gather.block.defs#link', uri: 'https://example.com/alice' },
              { $type: 'app.gather.block.defs#bold' },
            ],
          },
        ],
      }),
    ]),
  )
  expect(container.querySelectorAll('a')).toHaveLength(1)
  expect(container.querySelector('a a')).toBeNull()
  expect(container.querySelector('a')?.getAttribute('href')).toContain('did:plc:alice')
  expect(container.querySelector('strong')?.textContent).toBe('@alice.test')
})

test('TC-21: when an edited card gains an input, typed text stays with its own input', async () => {
  const user = userEvent.setup()
  const view = (blocks: unknown[]) => (
    <SourceResolverContext value={new FixtureResolver({})}>
      <ActionContext value={{ onAction() {} }}>
        <BlockCard cardRef={CARD_REF} card={card(blocks)} surface="feed" />
      </ActionContext>
    </SourceResolverContext>
  )
  const question = block('textInput', { id: 'question', label: 'Your question' })
  const ask = block('submit', { id: 'ask', label: 'Ask', action: 'ask' })
  const { rerender } = render(view([question, ask]))
  await user.type(screen.getByRole('textbox', { name: 'Your question' }), 'Is there a bike rack?')

  rerender(view([block('textInput', { id: 'name', label: 'Your name' }), question, ask]))

  expect((screen.getByRole('textbox', { name: 'Your question' }) as HTMLInputElement).value).toBe(
    'Is there a bike rack?',
  )
  expect((screen.getByRole('textbox', { name: 'Your name' }) as HTMLInputElement).value).toBe('')
})

test('TC-17: a new resolver (another viewer) never shows the previous viewer’s data', () => {
  const record = card([block('stat', { label: 'room', bind: { value: binding('mine', '/room') } })], {
    sources: [recordSource('mine')],
  })
  const view = (resolver: FixtureResolver) => (
    <SourceResolverContext value={resolver}>
      <ActionContext value={{ onAction() {} }}>
        <BlockCard cardRef={CARD_REF} card={record} surface="feed" />
      </ActionContext>
    </SourceResolverContext>
  )
  const { container, rerender } = render(view(new FixtureResolver({ mine: { room: 'Hotel V, kamer 412' } })))
  expect(container.textContent).toContain('kamer 412')

  rerender(view(new FixtureResolver({ mine: FixtureResolver.loading() })))
  expect(container.textContent).not.toContain('kamer 412')
  expect(blocksOfType(container, 'stat')[0].dataset.state).toBe('loading')
})

test('TC-32: a sheet ending in a single select keeps Tab inside, with the page behind inert', async () => {
  const user = userEvent.setup()
  const { container } = renderCard(
    card([
      block('button', {
        id: 'pick',
        label: 'Kies',
        opens: {
          title: 'Track',
          blocks: [
            block('select', {
              id: 'track',
              label: 'Track',
              options: [
                { label: 'A', value: 'a' },
                { label: 'B', value: 'b' },
                { label: 'C', value: 'c' },
              ],
            }),
          ],
        },
      }),
    ]),
  )
  await user.click(screen.getByRole('button', { name: 'Kies' }))
  const sheet = screen.getByRole('dialog')
  expect(container.hasAttribute('inert')).toBe(true)
  for (let i = 0; i < 6; i++) {
    await user.tab()
    expect(sheet.contains(document.activeElement), `tab ${i + 1}`).toBe(true)
  }
  for (let i = 0; i < 4; i++) {
    await user.tab({ shift: true })
    expect(sheet.contains(document.activeElement), `shift-tab ${i + 1}`).toBe(true)
  }
  await user.keyboard('{Escape}')
  expect(container.hasAttribute('inert')).toBe(false)
})

test('TC-17: an image bound to an unavailable source shows the same placeholder as other blocks', () => {
  const resolver = new FixtureResolver({ plan: FixtureResolver.forbidden() })
  const { container } = renderCard(
    card([block('image', { alt: 'Floor plan', bind: { url: binding('plan', '/url') } })], {
      sources: [recordSource('plan')],
    }),
    { resolver },
  )
  const [image] = blocksOfType(container, 'image')
  expect(image.dataset.state).toBe('unavailable')
  expect(image.textContent).toMatch(/unavailable/i)
  expect(image.textContent).not.toContain('Floor plan')
})

test('TC-22: lengths count characters as people see them', async () => {
  const user = userEvent.setup()
  const { onAction } = renderCard(
    card([
      block('textInput', { id: 'mood', label: 'Mood', maxLength: 3 }),
      block('submit', { id: 's', label: 'Send', action: 'mood' }),
    ]),
  )
  await user.click(screen.getByRole('textbox', { name: 'Mood' }))
  await user.paste('👨‍👩‍👧🇳🇱👍🏽')
  await user.click(screen.getByRole('button', { name: 'Send' }))
  expect(onAction).toHaveBeenCalledWith({
    card: CARD_REF,
    blockId: 's',
    actionId: 'mood',
    value: { mood: '👨‍👩‍👧🇳🇱👍🏽' },
  })
})

test('TC-19: a button’s bound value is sent as text, and a non-text value makes it unavailable', async () => {
  const user = userEvent.setup()
  const resolver = new FixtureResolver({ v: { n: 7, o: { a: 1 } } })
  const { container, onAction } = renderCard(
    card(
      [
        block('button', { id: 'num', label: 'Seven', action: 'pick', bind: { value: binding('v', '/n') } }),
        block('button', { id: 'obj', label: 'Object', action: 'pick', bind: { value: binding('v', '/o') } }),
      ],
      { sources: [recordSource('v')] },
    ),
    { resolver },
  )
  await user.click(screen.getByRole('button', { name: 'Seven' }))
  expect(onAction).toHaveBeenCalledWith({ card: CARD_REF, blockId: 'num', actionId: 'pick', value: '7' })
  expect(blocksOfType(container, 'button')[1].dataset.state).toBe('unavailable')
})

/** Renders a card from a list of blocks, and re-renders it with others, as a host does after an edit. */
function editable(initial: unknown[], resolver: SourceResolver = new FixtureResolver({}), sources: unknown[] = []) {
  const view = (blocks: unknown[]) => (
    <SourceResolverContext value={resolver}>
      <ActionContext value={{ onAction() {} }}>
        <BlockCard cardRef={CARD_REF} card={card(blocks, { sources })} surface="feed" />
      </ActionContext>
    </SourceResolverContext>
  )
  const result = render(view(initial))
  return { ...result, edit: (blocks: unknown[]) => result.rerender(view(blocks)) }
}

test('TC-21: a header added above a section of inputs keeps what was typed in them', async () => {
  const user = userEvent.setup()
  const form = block('section', {
    blocks: [
      block('textInput', { id: 'q', label: 'Your question' }),
      block('submit', { id: 's', label: 'Ask', action: 'ask' }),
    ],
  })
  const { edit } = editable([form])
  await user.type(screen.getByRole('textbox', { name: 'Your question' }), 'Is there a bike rack?')
  edit([block('header', { text: 'Vragen' }), form])
  expect((screen.getByRole('textbox', { name: 'Your question' }) as HTMLInputElement).value).toBe(
    'Is there a bike rack?',
  )
})

test('TC-13: a header added above a list keeps what was typed in its items', async () => {
  const user = userEvent.setup()
  const resolver = new FixtureResolver({ qs: [{ id: 'q1', text: 'Recorded?' }] })
  const list = block('list', {
    items: binding('qs'),
    key: '/id',
    template: [block('textInput', { id: 'a', label: 'Answer' })],
  })
  const { edit } = editable([list], resolver, [recordSource('qs')])
  await user.type(screen.getByRole('textbox', { name: 'Answer' }), 'Ja')
  edit([block('header', { text: 'Vragen' }), list])
  expect((screen.getByRole('textbox', { name: 'Answer' }) as HTMLInputElement).value).toBe('Ja')
})

/** A resolver that answers later, as a live one does, and counts its watches. */
class SlowResolver implements SourceResolver {
  watches = 0
  constructor(private readonly values: Record<string, unknown>) {}
  watch(_card: unknown, name: string): Observable<SourceState> {
    this.watches++
    return {
      subscribe: (next) => {
        next({ state: 'loading' })
        const timer = setTimeout(() => next({ state: 'ready', value: this.values[name] }), 5)
        return { unsubscribe: () => clearTimeout(timer) }
      },
    }
  }
}

test('TC-15: an edit that adds a binding to an already-loaded source keeps the block and its open sheet', async () => {
  const user = userEvent.setup()
  const resolver = new SlowResolver({ session: { label: 'Report', value: 'wifi' } })
  const sheet = { title: 'Report', blocks: [block('textInput', { id: 'p', label: 'Problem' })] }
  const before = block('button', { id: 'r', bind: { label: binding('session', '/label') }, opens: sheet })
  const { edit } = editable([before], resolver, [recordSource('session')])
  await user.click(await screen.findByRole('button', { name: 'Report' }))
  await user.type(screen.getByRole('textbox', { name: 'Problem' }), 'Geen signaal')
  expect(resolver.watches).toBe(1)

  edit([
    block('button', {
      id: 'r',
      bind: { label: binding('session', '/label'), value: binding('session', '/value') },
      opens: sheet,
    }),
  ])
  await act(async () => {})

  expect(resolver.watches).toBe(1)
  expect((screen.getByRole('textbox', { name: 'Problem' }) as HTMLInputElement).value).toBe('Geen signaal')
})

test('TC-15: an edit that removes a binding keeps the source loaded', async () => {
  const resolver = new SlowResolver({ stats: { total: 412, label: 'attendees' } })
  const stat = (bind: Record<string, unknown>) => block('stat', { label: 'attendees', value: '?', bind })
  const { container, edit } = editable(
    [stat({ value: binding('stats', '/total'), label: binding('stats', '/label') })],
    resolver,
    [recordSource('stats')],
  )
  await screen.findByText('412')
  edit([stat({ value: binding('stats', '/total') })])
  await act(async () => {})
  expect(blocksOfType(container, 'stat')[0].dataset.state).toBeUndefined()
  expect(resolver.watches).toBe(1)
})

test('TC-22: a failed submit moves focus to the first invalid input', async () => {
  const user = userEvent.setup()
  renderCard(
    card([
      block('textInput', { id: 'name', label: 'Name' }),
      block('textInput', { id: 'q', label: 'Question', required: true }),
      block('select', { id: 't', label: 'Track', required: true, options: [{ label: 'A', value: 'a' }] }),
      block('submit', { id: 's', label: 'Send', action: 'send' }),
    ]),
  )
  await user.click(screen.getByRole('button', { name: 'Send' }))
  expect(document.activeElement).toBe(screen.getByRole('textbox', { name: 'Question' }))
})

test('TC-27: years before 100 are not read as the 1900s', () => {
  expect(parseDatetime('0099-06-15T19:00:00Z')?.toISOString()).toBe('0099-06-15T19:00:00.000Z')
  expect(parseDatetime('0050-01-01T12:00:00', 'Europe/Amsterdam')?.getUTCFullYear()).toBe(50)
})

test('TC-23: an edit that removes a chosen option drops the choice', async () => {
  const user = userEvent.setup()
  const snacks = (values: string[]) =>
    block('select', {
      id: 'snack',
      label: 'Snack',
      required: true,
      options: values.map((v) => ({ label: v, value: v })),
    })
  const send = block('submit', { id: 's', label: 'Send', action: 'vote' })
  const host = { onAction: vi.fn() }
  const view = (blocks: unknown[]) => (
    <SourceResolverContext value={new FixtureResolver({})}>
      <ActionContext value={host}>
        <BlockCard cardRef={CARD_REF} card={card(blocks)} surface="feed" />
      </ActionContext>
    </SourceResolverContext>
  )
  const { rerender } = render(view([snacks(['Stroopwafel', 'Bitterballen', 'Kaas']), send]))
  await user.click(screen.getByRole('radio', { name: 'Kaas' }))

  rerender(view([snacks(['Stroopwafel', 'Bitterballen']), send]))
  await user.click(screen.getByRole('button', { name: 'Send' }))
  expect(host.onAction).not.toHaveBeenCalled()
  expect(screen.getByRole('radiogroup', { name: 'Snack' }).getAttribute('aria-describedby')).toBeTruthy()
})

test('TC-24: a multi select edit that removes a choice frees its place under the limit', async () => {
  const user = userEvent.setup()
  const talks = (values: string[]) =>
    block('select', {
      id: 't',
      label: 'Talks',
      multiple: true,
      maxSelections: 2,
      options: values.map((v) => ({ label: v, value: v })),
    })
  const view = (blocks: unknown[]) => (
    <SourceResolverContext value={new FixtureResolver({})}>
      <ActionContext value={{ onAction() {} }}>
        <BlockCard cardRef={CARD_REF} card={card(blocks)} surface="feed" />
      </ActionContext>
    </SourceResolverContext>
  )
  const { rerender } = render(view([talks(['A', 'B', 'C', 'D'])]))
  await user.click(screen.getByRole('checkbox', { name: 'A' }))
  await user.click(screen.getByRole('checkbox', { name: 'D' }))
  rerender(view([talks(['A', 'B', 'C'])]))
  expect((screen.getByRole('checkbox', { name: 'B' }) as HTMLInputElement).disabled).toBe(false)
})

test('TC-21: a submit inside a list item says which item it came from', async () => {
  const user = userEvent.setup()
  const resolver = new FixtureResolver({
    qs: [
      { id: 'q1', text: 'Recorded?' },
      { id: 'q2', text: 'Bike rack?' },
    ],
  })
  const { container, onAction } = renderCard(
    card(
      [
        block('list', {
          items: binding('qs'),
          key: '/id',
          template: [
            block('richText', { bind: { text: binding('$item', '/text') } }),
            block('textInput', { id: 'answer', label: 'Answer' }),
            block('submit', { id: 'reply', label: 'Reply', action: 'answer' }),
          ],
        }),
      ],
      { sources: [recordSource('qs')] },
    ),
    { resolver },
  )
  const second = container.querySelectorAll<HTMLElement>('.g-list__item')[1]
  await user.type(within(second).getByRole('textbox', { name: 'Answer' }), 'Ja')
  await user.click(within(second).getByRole('button', { name: 'Reply' }))
  expect(onAction).toHaveBeenCalledWith({
    card: CARD_REF,
    blockId: 'reply',
    actionId: 'answer',
    value: { answer: 'Ja' },
    item: 'q2',
  })
})

test('TC-22: after an edit adds an input above, a failed submit focuses the first one on screen', async () => {
  const user = userEvent.setup()
  const field = (id: string, label: string) => block('textInput', { id, label, required: true })
  const send = block('submit', { id: 's', label: 'Send', action: 'send' })
  const view = (blocks: unknown[]) => (
    <SourceResolverContext value={new FixtureResolver({})}>
      <ActionContext value={{ onAction() {} }}>
        <BlockCard cardRef={CARD_REF} card={card(blocks)} surface="feed" />
      </ActionContext>
    </SourceResolverContext>
  )
  const { rerender } = render(view([field('second', 'Second'), send]))
  rerender(view([field('first', 'First'), field('second', 'Second'), send]))
  await user.click(screen.getByRole('button', { name: 'Send' }))
  expect(document.activeElement).toBe(screen.getByRole('textbox', { name: 'First' }))
})

test('TC-24: an edit that lowers the limit below what was chosen blocks the submit and says why', async () => {
  const user = userEvent.setup()
  const talks = (max: number) =>
    block('select', {
      id: 'talks',
      label: 'Talks',
      multiple: true,
      maxSelections: max,
      options: ['a', 'b', 'c'].map((v) => ({ label: v.toUpperCase(), value: v })),
    })
  const send = block('submit', { id: 's', label: 'Send', action: 'save' })
  const host = { onAction: vi.fn() }
  const view = (blocks: unknown[]) => (
    <SourceResolverContext value={new FixtureResolver({})}>
      <ActionContext value={host}>
        <BlockCard cardRef={CARD_REF} card={card(blocks)} surface="feed" />
      </ActionContext>
    </SourceResolverContext>
  )
  const { rerender } = render(view([talks(3), send]))
  for (const name of ['A', 'B', 'C']) await user.click(screen.getByRole('checkbox', { name }))
  rerender(view([talks(1), send]))
  await user.click(screen.getByRole('button', { name: 'Send' }))
  expect(host.onAction).not.toHaveBeenCalled()
  expect(screen.getByText(/at most 1/i)).toBeTruthy()

  await user.click(screen.getByRole('checkbox', { name: 'B' }))
  await user.click(screen.getByRole('checkbox', { name: 'C' }))
  await user.click(screen.getByRole('button', { name: 'Send' }))
  expect(host.onAction).toHaveBeenCalledWith({
    card: CARD_REF,
    blockId: 's',
    actionId: 'save',
    value: { talks: ['a'] },
  })
})

test('TC-19: list items sharing a key are told apart by position', async () => {
  const user = userEvent.setup()
  const resolver = new FixtureResolver({
    qs: [
      { id: 'q1', text: 'First' },
      { id: 'q1', text: 'Second' },
      { id: 'q2', text: 'Third' },
    ],
  })
  const { onAction } = renderCard(
    card(
      [
        block('list', {
          items: binding('qs'),
          key: '/id',
          template: [block('button', { id: 'up', action: 'upvote', bind: { label: binding('$item', '/text') } })],
        }),
      ],
      { sources: [recordSource('qs')] },
    ),
    { resolver },
  )
  for (const name of ['First', 'Second', 'Third']) await user.click(screen.getByRole('button', { name }))
  expect(onAction.mock.calls.map(([intent]) => intent.item)).toEqual([0, 1, 'q2'])
})

test('TC-31: a different card in the same place starts fresh', async () => {
  const user = userEvent.setup()
  const ask = (text: string) =>
    card([
      block('header', { text }),
      block('textInput', { id: 'question', label: 'Question' }),
      block('submit', { id: 'send', label: 'Send', action: 'ask' }),
    ])
  const view = (uri: string, record: ReturnType<typeof card>, surface: 'feed' | 'ephemeral') => (
    <SourceResolverContext value={new FixtureResolver({})}>
      <ActionContext value={{ onAction() {} }}>
        <BlockCard cardRef={{ uri }} card={record} surface={surface} />
      </ActionContext>
    </SourceResolverContext>
  )
  // Text typed on one card doesn't appear on the next.
  const { rerender } = render(view('at://x/card/a', ask('Keynote'), 'feed'))
  await user.type(screen.getByRole('textbox', { name: 'Question' }), 'private note for keynote')
  rerender(view('at://x/card/b', ask('Workshop'), 'feed'))
  expect((screen.getByRole('textbox', { name: 'Question' }) as HTMLInputElement).value).toBe('')

  // Dismissing one ephemeral card doesn't hide the next.
  rerender(view('at://x/card/c', ask('Received'), 'ephemeral'))
  await user.click(screen.getByRole('button', { name: /dismiss/i }))
  expect(screen.queryByRole('heading', { name: 'Received' })).toBeNull()
  rerender(view('at://x/card/d', ask('Also received'), 'ephemeral'))
  expect(screen.getByRole('heading', { name: 'Also received' })).toBeTruthy()
})

test('TC-27: year 0 without an offset is read in the card zone correctly', () => {
  expect(parseDatetime('0000-06-01T12:00', 'Europe/Amsterdam')?.getUTCFullYear()).toBe(0)
  expect(parseDatetime('1950-06-01T12:00:00', 'Europe/Amsterdam')?.toISOString()).toBe('1950-06-01T11:00:00.000Z')
})
