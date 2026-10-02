import { fireEvent, screen, within } from '@testing-library/react'
import { afterEach, expect, test, vi } from 'vitest'
import { block, blocksOfType, byteSlice, card, renderCard } from './test-support'

afterEach(() => {
  vi.restoreAllMocks()
})

/** True when `a` comes before `b` in the document. */
function before(a: Node, b: Node) {
  return (a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0
}

test('TC-4: a card renders its blocks in order', () => {
  renderCard(
    card([
      block('header', { text: 'Welkom in Amsterdam' }),
      block('divider'),
      block('section', {
        blocks: [block('richText', { text: 'Registration opens at 08:30 in the Grote Zaal foyer.' })],
      }),
      block('context', { text: 'Posted by the organizers' }),
    ]),
  )

  const header = screen.getByRole('heading', { name: 'Welkom in Amsterdam' })
  const divider = screen.getByRole('separator')
  const text = screen.getByText('Registration opens at 08:30 in the Grote Zaal foyer.')
  const context = screen.getByText('Posted by the organizers')

  expect(before(header, divider)).toBe(true)
  expect(before(divider, text)).toBe(true)
  expect(before(text, context)).toBe(true)
})

test('TC-5: rich text shows links, mentions and emphasis', () => {
  // Multi-byte characters before the facets, so byte offsets differ from string offsets.
  const text = 'Koffie ☕ eerst — ask @alice.test about the venue page. Entry is free for speakers.'
  renderCard(
    card([
      block('richText', {
        text,
        facets: [
          {
            index: byteSlice(text, '@alice.test'),
            features: [{ $type: 'app.eventside.block.defs#mention', did: 'did:plc:alice' }],
          },
          {
            index: byteSlice(text, 'venue page'),
            features: [{ $type: 'app.eventside.block.defs#link', uri: 'https://atmosphereconf.org/venue' }],
          },
          {
            index: byteSlice(text, 'free for speakers'),
            features: [{ $type: 'app.eventside.block.defs#bold' }],
          },
        ],
      }),
    ]),
  )

  const mention = screen.getByRole('link', { name: '@alice.test' })
  expect(mention.getAttribute('href')).toContain('did:plc:alice')

  const link = screen.getByRole('link', { name: 'venue page' })
  expect(link.getAttribute('href')).toBe('https://atmosphereconf.org/venue')

  const bold = screen.getByText('free for speakers')
  expect(bold.closest('strong, b'), 'the phrase should be bold').not.toBeNull()

  // The unfaceted text is still there, whole.
  expect(document.body.textContent).toContain(text)
})

test('TC-7: an image shows with its alt text, and the alt text replaces it when it fails', () => {
  const alt = 'The Zuiderkerk tower seen from the canal'
  renderCard(card([block('image', { url: 'https://atmosphereconf.org/img/zuiderkerk.jpg', alt })]))

  const img = screen.getByRole('img', { name: alt })
  expect(img.getAttribute('src')).toBe('https://atmosphereconf.org/img/zuiderkerk.jpg')
  // Before the image fails, the alt text is only the image's name.
  expect(screen.queryByText(alt)).toBeNull()

  fireEvent.error(img)

  expect(screen.getByText(alt)).toBeTruthy()
})

test('TC-8: a block type the app does not know is skipped', () => {
  const errors = vi.spyOn(console, 'error')
  const { container } = renderCard(
    card([
      block('header', { text: 'Welkom in Amsterdam' }),
      { $type: 'app.eventside.block.future#hologram', text: 'Hologram of the keynote', size: 3 },
      block('section', { blocks: [block('richText', { text: 'Doors open at 09:00.' })] }),
    ]),
  )

  expect(screen.getByRole('heading', { name: 'Welkom in Amsterdam' })).toBeTruthy()
  expect(screen.getByText('Doors open at 09:00.')).toBeTruthy()
  expect(container.textContent).not.toMatch(/hologram/i)
  expect(container.querySelector('[data-block="hologram"]')).toBeNull()
  expect(container.textContent).not.toMatch(/unavailable|unknown|update/i)
  expect(errors).not.toHaveBeenCalled()
})

test('TC-9: reserved block types are skipped for now', () => {
  const errors = vi.spyOn(console, 'error')
  const module = {
    wasm: { cid: 'bafkreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenxquvyku', did: 'did:plc:organizer' },
    exports: ['render'],
  }
  const { container } = renderCard(
    card([
      block('header', { text: 'Toys' }),
      block('custom', { id: 'dice', module, sources: [] }),
      block('canvas', { id: 'doodle', module, width: 320, height: 200 }),
      block('context', { text: 'More toys on day 3' }),
    ]),
  )

  expect(screen.getByRole('heading', { name: 'Toys' })).toBeTruthy()
  expect(screen.getByText('More toys on day 3')).toBeTruthy()
  expect(blocksOfType(container, 'custom')).toHaveLength(0)
  expect(blocksOfType(container, 'canvas')).toHaveLength(0)
  expect(container.querySelector('canvas')).toBeNull()
  expect(within(container).queryByText(/unavailable|unknown|update/i)).toBeNull()
  expect(errors).not.toHaveBeenCalled()
})
