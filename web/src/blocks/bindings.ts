import { useCallback, useContext, useSyncExternalStore } from 'react'
import { ItemContext, useCard } from './context'
import type { SourceRef, SourceState } from './SourceResolver'

export interface Binding {
  source: string
  path?: string
}

/** A block as the renderer sees it: its literal properties, plus `bind`. */
export type BlockData = {
  $type?: string
  id?: string
  bind?: Record<string, Binding | undefined>
  [prop: string]: unknown
}

const UNAVAILABLE: SourceState = { state: 'unavailable' }

/**
 * Reads a JSON pointer (RFC 6901) from a value. The empty pointer is the
 * whole value; a pointer that leads nowhere gives `found: false`.
 */
export function readPointer(value: unknown, pointer = ''): { found: boolean; value?: unknown } {
  if (pointer === '') return { found: true, value }
  if (!pointer.startsWith('/')) return { found: false }
  let current = value
  for (const raw of pointer.slice(1).split('/')) {
    const segment = raw.replaceAll('~1', '/').replaceAll('~0', '~')
    if (Array.isArray(current) && /^(0|[1-9]\d*)$/.test(segment) && Number(segment) < current.length) {
      current = current[Number(segment)]
    } else if (
      current !== null &&
      typeof current === 'object' &&
      !Array.isArray(current) &&
      Object.hasOwn(current, segment)
    ) {
      current = (current as Record<string, unknown>)[segment]
    } else {
      return { found: false }
    }
  }
  return current === undefined ? { found: false } : { found: true, value: current }
}

/** Subscribes to sources by key, re-rendering when any of them changes, and returns their states. */
export function useSourceStates(sources: readonly (readonly [string, SourceRef])[]): SourceState[] {
  const { store } = useCard()
  const keys = sources.map(([key]) => key)
  const joined = keys.join('\n')
  // biome-ignore lint/correctness/useExhaustiveDependencies: `joined` stands for `sources`
  const subscribe = useCallback((listener: () => void) => store.subscribe(sources, listener), [store, joined])
  useSyncExternalStore(subscribe, () => store.version(keys))
  return keys.map((key) => store.get(key))
}

/**
 * The value of each named property of a block: its binding's value when it
 * has one, else its literal. A binding to a source the card doesn't declare,
 * or to a path that isn't there, is unavailable.
 */
export function useBound(block: BlockData, props: readonly string[]): Record<string, SourceState> {
  const { store } = useCard()
  const item = useContext(ItemContext)

  const bound = props.flatMap((prop) => {
    const binding = block.bind?.[prop]
    return binding && typeof binding.source === 'string' ? [[prop, binding] as const] : []
  })
  const watched = bound.flatMap(([, binding]) => {
    if (binding.source === '$item') return []
    const named = store.named(binding.source)
    return named ? [named] : []
  })
  const states = useSourceStates(watched)
  const stateOf = new Map(watched.map(([key], i) => [key, states[i]]))

  const result: Record<string, SourceState> = {}
  for (const prop of props) result[prop] = { state: 'ready', value: block[prop] }
  for (const [prop, binding] of bound) {
    let source: SourceState
    if (binding.source === '$item') {
      source = item ? { state: 'ready', value: item.item } : UNAVAILABLE
    } else {
      const named = store.named(binding.source)
      source = named ? (stateOf.get(named[0]) ?? UNAVAILABLE) : UNAVAILABLE
    }
    if (source.state !== 'ready') {
      result[prop] = source
      continue
    }
    const read = readPointer(source.value, binding.path ?? '')
    result[prop] = read.found ? { state: 'ready', value: read.value } : UNAVAILABLE
  }
  return result
}

/** The overall state of some bound values: loading wins over unavailable, which wins over ready. */
export function settle(states: Record<string, SourceState>): 'loading' | 'unavailable' | 'ready' {
  const all = Object.values(states)
  if (all.some((s) => s.state === 'loading')) return 'loading'
  if (all.some((s) => s.state === 'unavailable')) return 'unavailable'
  return 'ready'
}

/** A ready value as display text: strings and numbers show, anything else doesn't. */
export function text(state: SourceState | undefined): string | undefined {
  if (state?.state !== 'ready') return undefined
  const { value } = state
  if (typeof value === 'string') return value
  if (typeof value === 'number' && Number.isFinite(value)) return String(value)
  if (typeof value === 'boolean') return String(value)
  return undefined
}

/** A ready value as a number, from a number or a numeric string. */
export function numeric(state: SourceState | undefined): number | undefined {
  if (state?.state !== 'ready') return undefined
  const n = typeof state.value === 'string' && state.value.trim() !== '' ? Number(state.value) : state.value
  return typeof n === 'number' && Number.isFinite(n) ? n : undefined
}
