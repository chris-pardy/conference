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
  return result.success ? { ok: true } : { ok: false, error: toCardError(result.error.message) }
}
