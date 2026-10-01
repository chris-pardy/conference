import { useRef } from 'react'
import type { BlockData } from './bindings'

/**
 * Every block id in some blocks and inside them (sections, stacks, columns,
 * list templates, sheets), in document order. Ids are unique across a card.
 */
export function idsIn(blocks: unknown): string[] {
  const ids: string[] = []
  const walk = (list: unknown) => {
    for (const block of Array.isArray(list) ? (list as BlockData[]) : []) {
      if (typeof block?.id === 'string' && block.id !== '') ids.push(block.id)
      walk(block?.blocks)
      walk(block?.template)
      walk((block?.opens as { blocks?: unknown } | undefined)?.blocks)
      for (const column of Array.isArray(block?.columns) ? (block.columns as { blocks?: unknown }[]) : []) {
        walk(column?.blocks)
      }
    }
  }
  walk(blocks)
  return ids
}

/**
 * React keys that follow content across card edits, so adding, removing or
 * moving a block, column or container doesn't remount the ones that stayed
 * (and lose what was typed in them).
 *
 * Each previous key goes to the item of the same kind that kept the most of
 * the ids it had: a key moving to another kind would remount anyway, so it
 * isn't worth taking from the item that stayed. Items left over get a key
 * from their first id, or from `fallback` (e.g. type and place) if they hold
 * none.
 */
export function useStableKeys<T>(
  items: readonly T[],
  idsOf: (item: T) => string[],
  kindOf: (item: T) => string,
  fallback: (item: T, index: number) => string,
): string[] {
  const previous = useRef(new Map<string, { key: string; kind: string }>())
  const ids = items.map(idsOf)
  const kinds = items.map(kindOf)

  // How many of its old ids each item kept, per previous key of its kind.
  const claims: { item: number; key: string; kept: number }[] = []
  ids.forEach((list, item) => {
    const kept = new Map<string, number>()
    for (const id of list) {
      const was = previous.current.get(id)
      if (was && was.kind === kinds[item]) kept.set(was.key, (kept.get(was.key) ?? 0) + 1)
    }
    for (const [key, n] of kept) claims.push({ item, key, kept: n })
  })
  // Most ids kept first; ties go to the earlier item.
  claims.sort((a, b) => b.kept - a.kept || a.item - b.item)
  const keys: (string | undefined)[] = items.map(() => undefined)
  const used = new Set<string>()
  for (const { item, key } of claims) {
    if (keys[item] !== undefined || used.has(key)) continue
    keys[item] = key
    used.add(key)
  }
  const result = keys.map((assigned, i) => {
    if (assigned !== undefined) return assigned
    let key = ids[i].length > 0 ? `#${ids[i][0]}` : fallback(items[i], i)
    if (used.has(key)) key = `${key}@${i}`
    used.add(key)
    return key
  })

  // The first item holding an id keeps it (ids should be unique, but an
  // invalid card mustn't make keys swap on every render).
  const next = new Map<string, { key: string; kind: string }>()
  result.forEach((key, i) => {
    for (const id of ids[i]) if (!next.has(id)) next.set(id, { key, kind: kinds[i] })
  })
  // Rendering the same items again gives the same keys, so this is safe to
  // repeat (StrictMode renders twice).
  previous.current = next
  return result
}
