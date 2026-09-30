import { expect, test } from 'vitest'
import { block, blocksOfType, card, renderCard } from './test-support'

// The colors each variant maps to are checked in a real browser (e2e/blocks.spec.ts).

test('TC-34: variants are exposed by meaning, and unsupported ones are ignored', () => {
  const { container } = renderCard(
    card([
      block('button', { id: 'go', label: 'Ik kom!', action: 'going', variant: 'primary' }),
      block('button', { id: 'leave', label: 'Afmelden', action: 'leave', variant: 'danger' }),
      block('button', { id: 'odd', label: 'Glitter', action: 'glitter', variant: 'rainbow' }),
      block('badge', { text: 'Tip', tone: 'info' }),
      block('badge', { text: 'Druk', tone: 'warning' }),
      block('badge', { text: 'Open', tone: 'success' }),
      block('badge', { text: 'Sparkly', tone: 'sparkly' }),
    ]),
  )

  expect(blocksOfType(container, 'button').map((b) => b.dataset.variant)).toEqual(['primary', 'danger', 'default'])
  expect(blocksOfType(container, 'badge').map((b) => b.dataset.tone)).toEqual(['info', 'warning', 'success', 'neutral'])
  expect(container.textContent).toContain('Glitter')
  expect(container.textContent).toContain('Sparkly')
})
