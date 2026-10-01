import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { expect, test } from 'vitest'
import { validateCard } from './validate'

// Cases whose expected results were recorded from validateCard: the
// @atproto/lexicon check plus our card rules (unique ids per form, unique
// option values). The Rust validator (crates/blocks/tests/parity.rs) must
// give the same answers, so the PWA and the appview accept exactly the same
// cards. This test catches drift, e.g. after a lexicon library upgrade.
const cases: { name: string; card: unknown; expect: { ok: true } | { path: string; reason: string } }[] = JSON.parse(
  readFileSync(join(process.cwd(), 'tests/fixtures/cards/parity.json'), 'utf8'),
)

test('TC-2: the frontend validator matches the recorded lexicon results', () => {
  expect(cases.length).toBeGreaterThan(100)
  for (const { name, card, expect: expected } of cases) {
    const result = validateCard(card)
    const got = result.ok ? { ok: true } : { path: result.error.path, reason: result.error.reason }
    expect(got, name).toEqual(expected)
  }
})
