import { screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { expect, test } from 'vitest'
import { block, blocksOfType, CARD_REF, card, renderCard } from './test-support'

const everything = () =>
  card([
    block('header', { text: 'Wifi & koffie' }),
    block('image', { url: 'https://atmosphereconf.org/img/foyer.jpg', alt: 'The foyer coffee bar' }),
    block('richText', { text: 'Coffee is free all day in the foyer.' }),
    block('textInput', { id: 'wish', label: 'Your coffee wish' }),
    block('button', { id: 'join', label: 'Join the queue', action: 'join', variant: 'primary' }),
  ])

test('TC-31: a card fits the feed surface', () => {
  const { container } = renderCard(everything(), { surface: 'feed' })

  expect(container.querySelector('[data-surface="feed"]')).not.toBeNull()
  expect(screen.getByRole('heading', { name: 'Wifi & koffie' })).toBeTruthy()
  expect(screen.getByRole('img', { name: 'The foyer coffee bar' })).toBeTruthy()
  expect(screen.getByText('Coffee is free all day in the foyer.')).toBeTruthy()
  expect(screen.getByRole('textbox', { name: 'Your coffee wish' })).toBeTruthy()
  expect(screen.getByRole('button', { name: 'Join the queue' })).toBeTruthy()
  expect(screen.queryByText(/only you/i)).toBeNull()
})

test('TC-31: a card fits the compact surface', () => {
  const { container } = renderCard(everything(), { surface: 'compact' })

  expect(container.querySelector('[data-surface="compact"]')).not.toBeNull()
  expect(screen.getByRole('heading', { name: 'Wifi & koffie' })).toBeTruthy()
  expect(screen.getByRole('button', { name: 'Join the queue' })).toBeTruthy()
  expect(container.querySelector('img')).toBeNull()
  expect(screen.queryByRole('textbox')).toBeNull()
  expect(blocksOfType(container, 'textInput')).toHaveLength(0)
})

test('TC-31: a card fits the ephemeral surface, marked only you, and can be dismissed', async () => {
  const user = userEvent.setup()
  const { container } = renderCard(everything(), { surface: 'ephemeral' })

  expect(container.querySelector('[data-surface="ephemeral"]')).not.toBeNull()
  expect(screen.getByRole('heading', { name: 'Wifi & koffie' })).toBeTruthy()
  expect(screen.getByRole('img', { name: 'The foyer coffee bar' })).toBeTruthy()
  expect(screen.getByRole('textbox', { name: 'Your coffee wish' })).toBeTruthy()
  expect(screen.getByRole('button', { name: 'Join the queue' })).toBeTruthy()
  expect(screen.getByText(/only you/i)).toBeTruthy()

  await user.click(screen.getByRole('button', { name: /dismiss/i }))

  expect(screen.queryByRole('heading', { name: 'Wifi & koffie' })).toBeNull()
  expect(container.querySelector('[data-surface="ephemeral"]')).toBeNull()
})

test('TC-32: a button opens a sheet of blocks', async () => {
  const user = userEvent.setup()
  const { onAction } = renderCard(
    card([
      block('header', { text: 'Wifi' }),
      block('button', {
        id: 'details',
        label: 'Details',
        opens: {
          title: 'Wifi details',
          blocks: [
            block('section', { blocks: [block('richText', { text: 'Network: Atmosphere-Gast, on every floor.' })] }),
            block('textInput', { id: 'problem', label: 'Report a problem' }),
            block('submit', { id: 'report', label: 'Send', action: 'report' }),
          ],
        },
      }),
    ]),
  )

  expect(screen.queryByRole('dialog')).toBeNull()
  const opener = screen.getByRole('button', { name: 'Details' })
  expect(opener.getAttribute('aria-haspopup')).toBe('dialog')

  await user.click(opener)

  const sheet = screen.getByRole('dialog', { name: 'Wifi details' })
  expect(sheet.closest('[data-surface="sheet"]') ?? sheet.querySelector('[data-surface="sheet"]')).not.toBeNull()
  expect(within(sheet).getByText('Network: Atmosphere-Gast, on every floor.')).toBeTruthy()

  await user.type(within(sheet).getByRole('textbox', { name: 'Report a problem' }), 'No signal in Zaal B')
  await user.click(within(sheet).getByRole('button', { name: 'Send' }))
  expect(onAction).toHaveBeenCalledTimes(1)
  expect(onAction).toHaveBeenCalledWith({
    card: CARD_REF,
    blockId: 'report',
    actionId: 'report',
    value: { problem: 'No signal in Zaal B' },
  })

  await user.click(within(sheet).getByRole('button', { name: /close/i }))
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(screen.getByRole('heading', { name: 'Wifi' })).toBeTruthy()
  expect(screen.getByRole('button', { name: 'Details' })).toBeTruthy()
})
