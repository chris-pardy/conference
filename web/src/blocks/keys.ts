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
 * React keys that follow content across card edits. An item holding any id
 * it held before keeps its old key, so adding a block, column or container
 * anywhere doesn't remount the ones that were there (and lose what was typed
 * in them). Items with no ids fall back to `fallback`, e.g. type and place.
 */
export function useStableKeys<T>(
  items: readonly T[],
  idsOf: (item: T) => string[],
  fallback: (item: T, index: number) => string,
): string[] {
  const previous = useRef(new Map<string, string>())
  const used = new Set<string>()
  const next = new Map<string, string>()
  const keys = items.map((item, i) => {
    const ids = idsOf(item)
    let key = ids.map((id) => previous.current.get(id)).find((k) => k !== undefined && !used.has(k))
    key ??= ids.length > 0 ? `#${ids[0]}` : fallback(item, i)
    if (used.has(key)) key = `${key}@${i}`
    used.add(key)
    for (const id of ids) next.set(id, key)
    return key
  })
  // Rendering the same items again gives the same keys, so this is safe to
  // repeat (StrictMode renders twice).
  previous.current = next
  return keys
}
