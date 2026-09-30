import { act, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { expect, test } from 'vitest'
import { FixtureResolver } from './SourceResolver'
import { binding, block, blocksOfType, card, collectionSource, renderCard } from './test-support'

const QUESTIONS = [
  { id: 'q1', text: 'Is there a bike rack?', votes: 12 },
  { id: 'q2', text: 'Will the talks be recorded?', votes: 30 },
  { id: 'q3', text: 'Where do the canal tours leave from?', votes: 7 },
]

/** A card listing the `questions` source: a section per question, with its text and vote count. */
function questionsCard(extraTemplate: unknown[] = [], empty?: string) {
  return card(
    [
      block('header', { text: 'Vragen' }),
      block('list', {
        items: binding('questions'),
        key: '/id',
        empty,
        template: [
          block('section', {
            blocks: [
              block('richText', { bind: { text: binding('$item', '/text') } }),
              block('stat', { label: 'votes', bind: { value: binding('$item', '/votes') } }),
              ...extraTemplate,
            ],
          }),
        ],
      }),
    ],
    { sources: [collectionSource('questions')] },
  )
}

test('TC-10: a progress bar, stat and badge show their values', () => {
  const { container } = renderCard(
    card([
      block('progress', { label: 'Bitterballen', value: 62 }),
      block('stat', { value: '412', label: 'attendees' }),
      block('badge', { text: 'Open', tone: 'success' }),
    ]),
  )

  const bar = screen.getByRole('progressbar', { name: /Bitterballen/ })
  expect(bar.getAttribute('aria-valuenow')).toBe('62')
  expect(within(blocksOfType(container, 'progress')[0]).getByText(/62\s*%/)).toBeTruthy()

  const [stat] = blocksOfType(container, 'stat')
  expect(within(stat).getByText('412')).toBeTruthy()
  expect(within(stat).getByText('attendees')).toBeTruthy()

  const [badge] = blocksOfType(container, 'badge')
  expect(badge.textContent).toBe('Open')
  expect(badge.dataset.tone).toBe('success')
})

test('TC-11: a list repeats its template for each item', () => {
  const resolver = new FixtureResolver({ questions: QUESTIONS })
  const { container } = renderCard(questionsCard(), { resolver })

  const [list] = blocksOfType(container, 'list')
  const sections = blocksOfType(list, 'section')
  expect(sections).toHaveLength(3)
  QUESTIONS.forEach((q, i) => {
    expect(within(sections[i]).getByText(q.text)).toBeTruthy()
    expect(within(sections[i]).getByText(String(q.votes))).toBeTruthy()
  })
})

test('TC-12: an empty list shows its empty state', () => {
  const resolver = new FixtureResolver({ questions: [] })
  const { container } = renderCard(questionsCard([], 'Nog geen vragen: ask the first one'), { resolver })

  const [list] = blocksOfType(container, 'list')
  expect(within(list).getByText('Nog geen vragen: ask the first one')).toBeTruthy()
  expect(blocksOfType(list, 'section')).toHaveLength(0)
  expect(list.dataset.state).toBe('empty')
})

test('TC-13: a reordered list keeps each item state', async () => {
  const user = userEvent.setup()
  const resolver = new FixtureResolver({ questions: QUESTIONS })
  const reply = block('textInput', { id: 'reply', label: 'Your answer' })
  const { container } = renderCard(questionsCard([reply]), { resolver })

  const third = blocksOfType(container, 'section')[2]
  const input = within(third).getByRole('textbox', { name: 'Your answer' }) as HTMLInputElement
  await user.type(input, 'Behind Centraal')

  act(() => resolver.set('questions', [QUESTIONS[2], QUESTIONS[0], QUESTIONS[1]]))

  const first = blocksOfType(container, 'section')[0]
  expect(within(first).getByText('Where do the canal tours leave from?')).toBeTruthy()
  const moved = within(first).getByRole('textbox', { name: 'Your answer' }) as HTMLInputElement
  expect(moved.value).toBe('Behind Centraal')
  expect(moved).toBe(input)
  // The others are untouched.
  for (const other of blocksOfType(container, 'section').slice(1)) {
    expect((within(other).getByRole('textbox', { name: 'Your answer' }) as HTMLInputElement).value).toBe('')
  }
})
