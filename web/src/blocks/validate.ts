import { lexicons } from '../lexicon/lexicons'
import { MAX_DEPTH } from './limits'

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

/** A JSON pointer (RFC 6901): empty, or "/" segments where `~` only appears as `~0` or `~1`. */
function checkPointer(value: unknown, at: string): CardError | null {
  if (typeof value !== 'string' || value === '' || (value.startsWith('/') && !/~(?![01])/.test(value))) return null
  return {
    path: at,
    reason: 'format',
    message: `${at} "${value}" must be a JSON pointer (empty, or starting with "/", with ~ only as ~0 or ~1)`,
  }
}

/**
 * Rules the lexicon can't express, checked after it passes (the same rules,
 * in the same order, as crates/blocks/src/lexicon.rs `check_card`):
 * - block ids are unique across the whole card, sheets and list templates
 *   included, so an intent's blockId (plus `item` in a list) names one block
 * - a select's option values are unique, and so are a button group's
 * - source names are unique, and `$item` (a list's element) is reserved
 * - a button has exactly one of `action` and `opens`
 * - a binding names a declared source, or `$item` inside a list template
 *   (or a sheet opened from one), and its `path` is a JSON pointer
 * - a list's `key` is a JSON pointer
 * Sources are checked first, then blocks, depth first in document order.
 * Within a block: its id, its options, its button rule, its bindings (in
 * `bind` order, then a list's `items`), a list's `key`, then the blocks
 * inside it. A binding's source is checked before its path.
 */

function checkCard(card: Node): CardError | null {
  const declared = new Set(asArray(card.sources).map((s) => (s as Node).name))
  const checkBinding = (binding: unknown, at: string, inList: boolean): CardError | null => {
    const source = (binding as Node | null)?.source
    if (typeof source !== 'string') return null
    if (!(source === '$item' ? inList : declared.has(source))) {
      const why = source === '$item' ? 'is only defined inside a list template' : 'is not a source of this card'
      return { path: `${at}/source`, reason: 'unknown-source', message: `${at}/source "${source}" ${why}` }
    }
    return checkPointer((binding as Node).path, `${at}/path`)
  }
  const visit = (blocks: unknown[], path: string, ids: Set<string>, inList: boolean): CardError | null => {
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
            message: `${at}/id "${block.id}" is already used on this card`,
          }
        }
        ids.add(block.id)
      }
      const choices = type === 'select' ? 'options' : type === 'buttonGroup' ? 'buttons' : null
      if (choices) {
        const values = new Set<string>()
        for (const [j, option] of asArray(block[choices]).entries()) {
          const value = (option as Node).value as string
          if (values.has(value)) {
            return {
              path: `${at}/${choices}/${j}/value`,
              reason: 'duplicate',
              message: `${at}/${choices}/${j}/value "${value}" is already a choice`,
            }
          }
          values.add(value)
        }
      }
      if (type === 'button') {
        const hasAction = block.action !== undefined
        const hasSheet = block.opens !== undefined
        if (!hasAction && !hasSheet) {
          return { path: `${at}/action`, reason: 'required', message: `${at} needs an action or a sheet to open` }
        }
        if (hasAction && hasSheet) {
          return { path: `${at}/opens`, reason: 'exclusive', message: `${at} has both an action and a sheet` }
        }
      }
      for (const [prop, binding] of Object.entries((block.bind as Node | undefined) ?? {})) {
        const error = checkBinding(binding, `${at}/bind${pointer([prop])}`, inList)
        if (error) return error
      }
      if (type === 'list') {
        const error = checkBinding(block.items, `${at}/items`, inList) ?? checkPointer(block.key, `${at}/key`)
        if (error) return error
      }
      const nested =
        type === 'section' || type === 'stack'
          ? visit(asArray(block.blocks), `${at}/blocks`, ids, inList)
          : type === 'columns'
            ? asArray(block.columns).reduce<CardError | null>(
                (err, column, j) =>
                  err ?? visit(asArray((column as Node).blocks), `${at}/columns/${j}/blocks`, ids, inList),
                null,
              )
            : type === 'list'
              ? visit(asArray(block.template), `${at}/template`, ids, true)
              : type === 'button' && block.opens
                ? visit(asArray((block.opens as Node).blocks), `${at}/opens/blocks`, ids, inList)
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
  return visit(asArray(card.blocks), '/blocks', new Set(), false)
}

/**
 * The first block nested deeper than MAX_DEPTH, in document order. It walks
 * with its own stack, not recursion, and runs before the lexicon check
 * (which recurses), so no card can overflow the call stack. Children are a
 * block's `blocks`, its columns' `blocks`, its `template`, then its sheet's
 * `blocks`, as in crates/blocks/src/lexicon.rs `check_depth`.
 */
function checkDepth(card: Node): CardError | null {
  const stack: [unknown, string, number][] = []
  const push = (list: unknown, path: string, depth: number) => {
    if (!Array.isArray(list)) return
    for (let i = list.length - 1; i >= 0; i--) stack.push([list[i], `${path}/${i}`, depth])
  }
  push(card.blocks, '/blocks', 1)
  while (stack.length > 0) {
    const [raw, at, depth] = stack.pop() as [unknown, string, number]
    if (raw === null || typeof raw !== 'object' || Array.isArray(raw)) continue
    if (depth > MAX_DEPTH) {
      return { path: at, reason: 'max-depth', message: `${at} is nested more than ${MAX_DEPTH} blocks deep` }
    }
    const block = raw as Node
    const children: [unknown, string][] = [[block.blocks, `${at}/blocks`]]
    asArray(block.columns).forEach((column, j) => {
      children.push([(column as Node | null)?.blocks, `${at}/columns/${j}/blocks`])
    })
    children.push([block.template, `${at}/template`])
    children.push([(block.opens as Node | null | undefined)?.blocks, `${at}/opens/blocks`])
    for (let k = children.length - 1; k >= 0; k--) push(children[k][0], children[k][1], depth + 1)
  }
  return null
}

/**
 * The first lexicon `cid` that isn't ASCII. CIDs are ASCII, but the base58
 * decoder inside @atproto/lexicon accepts characters above U+00FF, which
 * the Rust validator rightly doesn't; checking this first, the same way on
 * both sides, keeps them agreeing. It looks only where the lexicon declares
 * a CID (a record source's `record.cid`, a module's `wasm.cid` and
 * `script.cid`), through blocks this version knows, in document order:
 * sources, then middleware, then blocks. As `check_cids` in
 * crates/blocks/src/lexicon.rs. (Runs after the depth check, so recursion
 * is bounded.)
 */
function checkCids(card: Node): CardError | null {
  const bad = (value: unknown, at: string): CardError | null =>
    typeof value === 'string' && [...value].some((c) => c.charCodeAt(0) > 0x7f)
      ? { path: at, reason: 'format', message: `${at} must be a cid string (ASCII)` }
      : null
  const cidOf = (value: unknown) => (value as Node | null | undefined)?.cid
  const module = (ref: unknown, at: string) =>
    bad(cidOf((ref as Node | null)?.wasm), `${at}/wasm/cid`) ??
    bad(cidOf((ref as Node | null)?.script), `${at}/script/cid`)
  for (const [i, source] of asArray(card.sources).entries()) {
    const ref = (source as Node | null)?.ref as Node | null | undefined
    if (ref?.$type === `${DEFS_PREFIX}recordSource`) {
      const error = bad((ref.record as Node | null)?.cid, `/sources/${i}/ref/record/cid`)
      if (error) return error
    }
  }
  for (const [i, ref] of asArray(card.middleware).entries()) {
    const error = module(ref, `/middleware/${i}`)
    if (error) return error
  }
  const visit = (blocks: unknown, path: string): CardError | null => {
    for (const [i, raw] of asArray(blocks).entries()) {
      const block = raw as Node | null
      const at = `${path}/${i}`
      const type =
        typeof block?.$type === 'string' && block.$type.startsWith(DEFS_PREFIX)
          ? block.$type.slice(DEFS_PREFIX.length)
          : null
      if (!block || !type) continue
      const error =
        type === 'custom' || type === 'canvas'
          ? module(block.module, `${at}/module`)
          : type === 'section' || type === 'stack'
            ? visit(block.blocks, `${at}/blocks`)
            : type === 'columns'
              ? asArray(block.columns).reduce<CardError | null>(
                  (err, column, j) => err ?? visit((column as Node | null)?.blocks, `${at}/columns/${j}/blocks`),
                  null,
                )
              : type === 'list'
                ? visit(block.template, `${at}/template`)
                : type === 'button'
                  ? visit((block.opens as Node | null | undefined)?.blocks, `${at}/opens/blocks`)
                  : null
      if (error) return error
    }
    return null
  }
  return visit(card.blocks, '/blocks')
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
  const early = checkDepth(record as Node) ?? checkCids(record as Node)
  if (early) return { ok: false, error: early }
  const result = lexicons.validate(CARD_NSID, record)
  if (!result.success) return { ok: false, error: toCardError(result.error.message) }
  const error = checkCard(record as Node)
  return error ? { ok: false, error } : { ok: true }
}
