import { act, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { expect, test } from 'vitest'
import { FixtureResolver } from './SourceResolver'
import { binding, block, blocksOfType, card, recordSource, renderCard } from './test-support'

const UNAVAILABLE = /unavailable/i

test('TC-14: a bound value shows the source field', () => {
  const resolver = new FixtureResolver({ stats: { total: 412, checkedIn: 377 } })
  const { container } = renderCard(
    card([block('stat', { label: 'attendees', bind: { value: binding('stats', '/total') } })], {
      sources: [recordSource('stats')],
    }),
    { resolver },
  )

  const [stat] = blocksOfType(container, 'stat')
  expect(within(stat).getByText('412')).toBeTruthy()
  expect(within(stat).getByText('attendees')).toBeTruthy()
})

test('TC-15: a bound value updates when its source changes, without re-mounting the card', async () => {
  const user = userEvent.setup()
  const resolver = new FixtureResolver({ stats: { total: 412 } })
  const { container } = renderCard(
    card(
      [
        block('header', { text: 'Aanwezig' }),
        block('stat', { label: 'attendees', bind: { value: binding('stats', '/total') } }),
        block('textInput', { id: 'note', label: 'Note' }),
      ],
      { sources: [recordSource('stats')] },
    ),
    { resolver },
  )
  const heading = screen.getByRole('heading', { name: 'Aanwezig' })
  const input = screen.getByRole('textbox', { name: 'Note' }) as HTMLInputElement
  await user.type(input, 'half full')

  act(() => resolver.set('stats', { total: 413 }))

  const [stat] = blocksOfType(container, 'stat')
  expect(within(stat).getByText('413')).toBeTruthy()
  expect(within(stat).queryByText('412')).toBeNull()
  expect(screen.getByRole('heading', { name: 'Aanwezig' })).toBe(heading)
  expect(screen.getByRole('textbox', { name: 'Note' })).toBe(input)
  expect(input.value).toBe('half full')
})

test('TC-16: a loading source shows a skeleton, and the rest of the card shows', () => {
  const resolver = new FixtureResolver({ slow: FixtureResolver.loading() })
  const { container } = renderCard(
    card(
      [
        block('header', { text: 'Wachtrij bij de garderobe' }),
        block('stat', { label: 'people waiting', bind: { value: binding('slow', '/count') } }),
        block('context', { text: 'Updated every minute' }),
      ],
      { sources: [recordSource('slow')] },
    ),
    { resolver },
  )

  const [stat] = blocksOfType(container, 'stat')
  expect(stat.dataset.state).toBe('loading')
  const busy = stat.matches('[aria-busy="true"]') ? stat : stat.querySelector('[aria-busy="true"]')
  expect(busy, 'the stat should be marked busy for assistive tech').not.toBeNull()
  expect(stat.textContent).not.toMatch(UNAVAILABLE)

  expect(screen.getByRole('heading', { name: 'Wachtrij bij de garderobe' })).toBeTruthy()
  expect(screen.getByText('Updated every minute')).toBeTruthy()
  expect(container.querySelectorAll('[data-state="loading"]')).toHaveLength(1)
})

test('TC-17: a missing, forbidden or failing source shows "unavailable", never why', () => {
  const resolver = new FixtureResolver({
    // `gone` is declared by the card but absent here: missing.
    secret: FixtureResolver.forbidden(),
    broken: FixtureResolver.failing(),
  })
  const { container } = renderCard(
    card(
      [
        block('header', { text: 'Stemmen' }),
        block('stat', { label: 'gone', bind: { value: binding('gone', '/total') } }),
        block('stat', { label: 'secret', bind: { value: binding('secret', '/total') } }),
        block('stat', { label: 'broken', bind: { value: binding('broken', '/total') } }),
      ],
      { sources: [recordSource('gone'), recordSource('secret'), recordSource('broken')] },
    ),
    { resolver },
  )

  const stats = blocksOfType(container, 'stat')
  expect(stats).toHaveLength(3)
  for (const stat of stats) {
    expect(stat.dataset.state).toBe('unavailable')
    expect(stat.textContent).toMatch(UNAVAILABLE)
  }
  // The same placeholder for all three: nothing tells them apart.
  const placeholder = (el: HTMLElement) => (el.textContent ?? '').replace(/gone|secret|broken/, '')
  expect(new Set(stats.map(placeholder)).size).toBe(1)
  expect(container.textContent).not.toMatch(/forbidden|denied|permission|error|fail|missing|not found|404|403/i)

  expect(screen.getByRole('heading', { name: 'Stemmen' })).toBeTruthy()
})

test('TC-18: a binding to a field that does not exist shows "unavailable"', () => {
  const resolver = new FixtureResolver({ stats: { total: 412 } })
  const { container } = renderCard(
    card(
      [
        block('header', { text: 'Aanwezig' }),
        block('stat', { label: 'speakers', bind: { value: binding('stats', '/speakers/count') } }),
        block('stat', { label: 'attendees', bind: { value: binding('stats', '/total') } }),
      ],
      { sources: [recordSource('stats')] },
    ),
    { resolver },
  )

  const [missing, present] = blocksOfType(container, 'stat')
  expect(missing.dataset.state).toBe('unavailable')
  expect(missing.textContent).toMatch(UNAVAILABLE)
  expect(within(present).getByText('412')).toBeTruthy()
  expect(screen.getByRole('heading', { name: 'Aanwezig' })).toBeTruthy()
})
