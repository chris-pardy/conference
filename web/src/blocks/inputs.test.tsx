import { screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { expect, test } from 'vitest'
import { block, CARD_REF, card, renderCard } from './test-support'

const option = (label: string, value: string) => ({ label, value })

test('TC-19: a button sends an action intent', async () => {
  const user = userEvent.setup()
  const { onAction } = renderCard(
    card([
      block('header', { text: 'Borrel op vrijdag' }),
      block('button', { id: 'rsvp', label: 'Ik kom!', action: 'going', value: 'yes', variant: 'primary' }),
    ]),
  )

  await user.click(screen.getByRole('button', { name: 'Ik kom!' }))

  expect(onAction).toHaveBeenCalledTimes(1)
  expect(onAction).toHaveBeenCalledWith({ card: CARD_REF, blockId: 'rsvp', actionId: 'going', value: 'yes' })
})

test('TC-20: a button group sends the chosen button intent', async () => {
  const user = userEvent.setup()
  const { onAction } = renderCard(
    card([
      block('buttonGroup', {
        id: 'snack',
        label: 'Favourite snack',
        action: 'vote',
        buttons: [option('Stroopwafel', 'stroopwafel'), option('Bitterballen', 'bitterballen'), option('Kaas', 'kaas')],
      }),
    ]),
  )

  await user.click(screen.getByRole('button', { name: 'Kaas' }))

  expect(onAction).toHaveBeenCalledTimes(1)
  expect(onAction).toHaveBeenCalledWith({ card: CARD_REF, blockId: 'snack', actionId: 'vote', value: 'kaas' })
})

test('TC-21: a text input submits its text, then clears', async () => {
  const user = userEvent.setup()
  const { onAction } = renderCard(
    card([
      block('textInput', { id: 'question', label: 'Your question', placeholder: 'Ask the organizers' }),
      block('submit', { id: 'ask', label: 'Ask', action: 'ask', variant: 'primary' }),
    ]),
  )

  const input = screen.getByRole('textbox', { name: 'Your question' }) as HTMLInputElement
  await user.type(input, 'Is there a bike rack?')
  await user.click(screen.getByRole('button', { name: 'Ask' }))

  expect(onAction).toHaveBeenCalledTimes(1)
  expect(onAction).toHaveBeenCalledWith({
    card: CARD_REF,
    blockId: 'ask',
    actionId: 'ask',
    value: { question: 'Is there a bike rack?' },
  })
  expect(input.value).toBe('')
})

test('TC-22: a required input blocks an empty or too long submit, and says why', async () => {
  const user = userEvent.setup()
  const { onAction } = renderCard(
    card([
      block('textInput', { id: 'question', label: 'Your question', required: true, maxLength: 280 }),
      block('submit', { id: 'ask', label: 'Ask', action: 'ask' }),
    ]),
  )
  const input = screen.getByRole('textbox', { name: 'Your question' }) as HTMLInputElement

  await user.click(screen.getByRole('button', { name: 'Ask' }))
  expect(onAction).not.toHaveBeenCalled()
  expect(input.getAttribute('aria-invalid')).toBe('true')
  expect(screen.getByRole('textbox', { name: 'Your question', description: /required/i })).toBe(input)

  await user.click(input)
  await user.paste('a'.repeat(281))
  expect(input.value, 'the input keeps all 281 characters so it can say why they are too many').toHaveLength(281)
  await user.click(screen.getByRole('button', { name: 'Ask' }))
  expect(onAction).not.toHaveBeenCalled()
  expect(input.getAttribute('aria-invalid')).toBe('true')
  expect(screen.getByRole('textbox', { name: 'Your question', description: /280/ })).toBe(input)
})

test('TC-23: a single select sends one choice', async () => {
  const user = userEvent.setup()
  const { onAction } = renderCard(
    card([
      block('select', {
        id: 'track',
        label: 'Which track?',
        options: [option('Protocol', 'protocol'), option('Apps', 'apps'), option('Community', 'community')],
      }),
      block('submit', { id: 'pick', label: 'Kies', action: 'pick' }),
    ]),
  )

  const group = screen.getByRole('radiogroup', { name: 'Which track?' })
  await user.click(within(group).getByRole('radio', { name: 'Protocol' }))
  await user.click(within(group).getByRole('radio', { name: 'Apps' }))
  await user.click(screen.getByRole('button', { name: 'Kies' }))

  expect(onAction).toHaveBeenCalledTimes(1)
  expect(onAction).toHaveBeenCalledWith({ card: CARD_REF, blockId: 'pick', actionId: 'pick', value: { track: 'apps' } })
})

test('TC-24: a multi select respects its limit', async () => {
  const user = userEvent.setup()
  const { onAction } = renderCard(
    card([
      block('select', {
        id: 'talks',
        label: 'Talks to save',
        multiple: true,
        maxSelections: 2,
        options: [
          option('Lexicons in Practice', 'lexicons'),
          option('Spaces Deep Dive', 'spaces'),
          option('Running a PDS', 'pds'),
          option('Feeds at Scale', 'feeds'),
          option('OAuth for Apps', 'oauth'),
        ],
      }),
      block('submit', { id: 'save', label: 'Save', action: 'save' }),
    ]),
  )

  const group = screen.getByRole('group', { name: 'Talks to save' })
  const box = (name: string) => within(group).getByRole('checkbox', { name }) as HTMLInputElement
  await user.click(box('Lexicons in Practice'))
  await user.click(box('Running a PDS'))
  await user.click(box('Feeds at Scale'))

  expect(box('Feeds at Scale').checked).toBe(false)
  expect(within(group).getByText(/(at most|up to|max(imum)?( of)?)\s*2/i)).toBeTruthy()

  await user.click(screen.getByRole('button', { name: 'Save' }))
  expect(onAction).toHaveBeenCalledTimes(1)
  expect(onAction).toHaveBeenCalledWith({
    card: CARD_REF,
    blockId: 'save',
    actionId: 'save',
    value: { talks: ['lexicons', 'pds'] },
  })
})

test('TC-25: inputs are usable with a keyboard and a screen reader', async () => {
  const user = userEvent.setup()
  const { onAction } = renderCard(
    card([
      block('button', { id: 'rsvp', label: 'Ik kom!', action: 'going', value: 'yes', variant: 'primary' }),
      block('textInput', { id: 'question', label: 'Your question' }),
      block('select', {
        id: 'track',
        label: 'Which track?',
        options: [option('Protocol', 'protocol'), option('Apps', 'apps')],
      }),
      block('select', {
        id: 'talks',
        label: 'Talks to save',
        multiple: true,
        maxSelections: 2,
        options: [option('Lexicons in Practice', 'lexicons'), option('Spaces Deep Dive', 'spaces')],
      }),
      block('submit', { id: 'send', label: 'Send', action: 'send' }),
    ]),
  )

  // Every control has an accessible name.
  const rsvp = screen.getByRole('button', { name: 'Ik kom!' })
  const question = screen.getByRole('textbox', { name: 'Your question' })
  const track = within(screen.getByRole('radiogroup', { name: 'Which track?' }))
  const radios = [track.getByRole('radio', { name: 'Protocol' }), track.getByRole('radio', { name: 'Apps' })]
  const talks = within(screen.getByRole('group', { name: 'Talks to save' }))
  const boxes = [
    talks.getByRole('checkbox', { name: 'Lexicons in Practice' }),
    talks.getByRole('checkbox', { name: 'Spaces Deep Dive' }),
  ]
  const send = screen.getByRole('button', { name: 'Send' })

  // Tab reaches every control, in order.
  const reached: Element[] = []
  for (let i = 0; i < 12; i++) {
    await user.tab()
    if (document.activeElement && !reached.includes(document.activeElement)) reached.push(document.activeElement)
  }
  expect(reached).toContain(rsvp)
  expect(reached).toContain(question)
  expect(
    radios.some((radio) => reached.includes(radio)),
    'a radio should be reachable',
  ).toBe(true)
  for (const checkbox of boxes) expect(reached).toContain(checkbox)
  expect(reached).toContain(send)
  const order = [rsvp, question, radios.find((r) => reached.includes(r)), boxes[0], send].map((el) =>
    reached.indexOf(el as Element),
  )
  expect(order).toEqual([...order].sort((a, b) => a - b))

  // And operates each one.
  rsvp.focus()
  await user.keyboard('{Enter}')
  expect(onAction).toHaveBeenLastCalledWith({ card: CARD_REF, blockId: 'rsvp', actionId: 'going', value: 'yes' })

  question.focus()
  await user.keyboard('Waar is de garderobe?')
  radios[1].focus()
  await user.keyboard(' ')
  boxes[0].focus()
  await user.keyboard(' ')
  send.focus()
  await user.keyboard('{Enter}')

  expect(onAction).toHaveBeenCalledTimes(2)
  expect(onAction).toHaveBeenLastCalledWith({
    card: CARD_REF,
    blockId: 'send',
    actionId: 'send',
    value: { question: 'Waar is de garderobe?', track: 'apps', talks: ['lexicons'] },
  })
})
