/** Why a card failed: the JSON pointer of the offending value, and a reason code shared with the Rust validator. */
export interface CardError {
  path: string
  reason: string
  message: string
}

export type CardValidation = { ok: true } | { ok: false; error: CardError }

/** Validates an `app.gather.block.card` record against the lexicons. */
export function validateCard(_record: unknown): CardValidation {
  throw new Error('not implemented')
}
