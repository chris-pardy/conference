import { lexicons } from '../lexicon/lexicons'

/** Why a card failed: the JSON pointer of the offending value, and a reason code shared with the Rust validator. */
export interface CardError {
  path: string
  reason: string
  message: string
}

export type CardValidation = { ok: true } | { ok: false; error: CardError }

export const CARD_NSID = 'app.gather.block.card'

// @atproto/lexicon reports one error as a message: "Record/blocks/0 must have
// the property "id"". These turn the rest of the message into a reason code;
// crates/blocks/src/lexicon.rs produces the same codes.
const REASONS: [RegExp, string][] = [
  [/^must not have fewer than \d+ elements$/, 'min-length'],
  [/^must not have more than \d+ elements$/, 'max-length'],
  [/^must not be shorter than \d+ (characters|graphemes)$/, 'min-length'],
  [/^must not be longer than \d+ (characters|graphemes)$/, 'max-length'],
  [/^can not be less than -?\d+$/, 'minimum'],
  [/^can not be greater than -?\d+$/, 'maximum'],
  [/^must be one of \(/, 'enum'],
  [/^\$type must be one of /, 'union'],
  [/^must be (a string|an integer|a boolean|an array|an object|a CID|a byte array)$/, 'type'],
  [/^must be (an? valid|a uri|a cid string|a well-formed)/, 'format'],
  [/^must be /, 'const'],
]

/** "blocks/0/id" as a JSON pointer: "/blocks/0/id". */
function pointer(segments: string[]): string {
  return segments.map((s) => `/${s.replaceAll('~', '~0').replaceAll('/', '~1')}`).join('')
}

function toCardError(message: string): CardError {
  const match = /^(?:Record|Object|Value)((?:\/\S+)?) (.+)$/.exec(message)
  if (!match) return { path: '', reason: 'invalid', message }
  const segments = match[1].split('/').filter(Boolean)
  const rest = match[2]
  const human = `${pointer(segments) || '/'} ${rest}`

  const required = /^must have the property "(.+)"$/.exec(rest)
  if (required) return { path: pointer([...segments, required[1]]), reason: 'required', message: human }
  if (rest === 'must be an object which includes the "$type" property') {
    return { path: pointer([...segments, '$type']), reason: 'required', message: human }
  }
  const reason = REASONS.find(([re]) => re.test(rest))?.[1] ?? 'invalid'
  return { path: pointer(segments), reason, message: human }
}

const DEFS_PREFIX = 'app.gather.block.defs#'

type Node = Record<string, unknown>
const asArray = (v: unknown): unknown[] => (Array.isArray(v) ? v : [])

/**
 * Rules the lexicon can't express, checked after it passes (the same rules,
 * in the same order, as crates/blocks/src/lexicon.rs `check_card`):
 * - block ids are unique within each form: the card, each sheet and each
 *   list template, since a submit sends `{id: value}` for its form
 * - a select's option values are unique
 * - source names are unique, and `$item` (a list's element) is reserved
 * Sources are checked first, then blocks, depth first in document order.
 */
function checkCard(card: Node): CardError | null {
  const visit = (blocks: unknown[], path: string, ids: Set<string>): CardError | null => {
    for (const [i, raw] of blocks.entries()) {
      const block = raw as Node
      const at = `${path}/${i}`
      if (typeof block?.$type !== 'string' || !block.$type.startsWith(DEFS_PREFIX)) continue
      const type = block.$type.slice(DEFS_PREFIX.length)
      if (typeof block.id === 'string') {
        if (ids.has(block.id)) {
          return {
            path: `${at}/id`,
            reason: 'duplicate',
            message: `${at}/id "${block.id}" is already used in this form`,
          }
        }
        ids.add(block.id)
      }
      if (type === 'select') {
        const values = new Set<string>()
        for (const [j, option] of asArray(block.options).entries()) {
          const value = (option as Node).value as string
          if (values.has(value)) {
            return {
              path: `${at}/options/${j}/value`,
              reason: 'duplicate',
              message: `${at}/options/${j}/value "${value}" is already an option`,
            }
          }
          values.add(value)
        }
      }
      const nested =
        type === 'section' || type === 'stack'
          ? visit(asArray(block.blocks), `${at}/blocks`, ids)
          : type === 'columns'
            ? asArray(block.columns).reduce<CardError | null>(
                (err, column, j) => err ?? visit(asArray((column as Node).blocks), `${at}/columns/${j}/blocks`, ids),
                null,
              )
            : type === 'list'
              ? visit(asArray(block.template), `${at}/template`, new Set())
              : type === 'button' && block.opens
                ? visit(asArray((block.opens as Node).blocks), `${at}/opens/blocks`, new Set())
                : null
      if (nested) return nested
    }
    return null
  }
  const names = new Set<string>()
  for (const [i, source] of asArray(card.sources).entries()) {
    const name = (source as Node).name as string
    const at = `/sources/${i}/name`
    if (name === '$item') {
      return { path: at, reason: 'reserved', message: `${at} "$item" is reserved for list elements` }
    }
    if (names.has(name)) return { path: at, reason: 'duplicate', message: `${at} "${name}" is already a source` }
    names.add(name)
  }
  return visit(asArray(card.blocks), '/blocks', new Set())
}

/** Validates an `app.gather.block.card` record against the lexicons. */
export function validateCard(record: unknown): CardValidation {
  if (typeof record !== 'object' || record === null || Array.isArray(record)) {
    return { ok: false, error: { path: '', reason: 'type', message: 'a card must be an object' } }
  }
  const $type = (record as { $type?: unknown }).$type
  if ($type !== CARD_NSID) {
    return {
      ok: false,
      error: { path: '/$type', reason: 'const', message: `/$type must be ${CARD_NSID}` },
    }
  }
  const result = lexicons.validate(CARD_NSID, record)
  if (!result.success) return { ok: false, error: toCardError(result.error.message) }
  const error = checkCard(record as Node)
  return error ? { ok: false, error } : { ok: true }
}
