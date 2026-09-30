import { existsSync, readdirSync, readFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { expect, test } from 'vitest'
import { validateCard } from './validate'

// Plain paths: under jsdom the global URL isn't the one node:fs accepts.
const HERE = dirname(fileURLToPath(import.meta.url))
const GALLERY = join(HERE, 'gallery')
const INVALID = resolve(HERE, '../../../tests/fixtures/cards/invalid')

function jsonFiles(dir: string, suffix: string) {
  const names = existsSync(dir) ? readdirSync(dir).filter((name) => name.endsWith(suffix)) : []
  return names.sort().map((name) => ({ name, json: JSON.parse(readFileSync(join(dir, name), 'utf8')) }))
}

test('TC-1: every gallery card is a valid card record (frontend)', () => {
  const cards = jsonFiles(GALLERY, '.card.json')
  expect(cards.length, 'the gallery should ship sample cards in web/src/blocks/gallery/*.card.json').toBeGreaterThan(0)
  for (const { name, json } of cards) {
    expect(validateCard(json.card), name).toEqual({ ok: true })
  }
})

test('TC-2: invalid cards are rejected (frontend), for the shared reason', () => {
  const fixtures = jsonFiles(INVALID, '.json')
  expect(fixtures.length).toBe(3)
  for (const { name, json } of fixtures) {
    const result = validateCard(json.card)
    expect(result.ok, `${name}: ${json.description}`).toBe(false)
    if (!result.ok) {
      expect({ path: result.error.path, reason: result.error.reason }, name).toEqual(json.error)
      expect(result.error.message.length, `${name} should explain itself`).toBeGreaterThan(0)
    }
  }
})
