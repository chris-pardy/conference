import { createContext } from 'react'
import type { CardRef } from './ActionContext'

export type SourceState = { state: 'loading' } | { state: 'ready'; value: unknown } | { state: 'unavailable' }

export interface Observable<T> {
  subscribe(next: (value: T) => void): { unsubscribe(): void }
}

/** A `#sourceRef` from the card's lexicon. */
export type SourceRef = { $type: string; [field: string]: unknown }

/**
 * The seam between the renderer and wherever data comes from. Anything
 * missing, forbidden or failing is `unavailable`: the reason never reaches
 * the card.
 */
export interface SourceResolver {
  watch(card: CardRef, name: string, ref: SourceRef): Observable<SourceState>
}

export const SourceResolverContext = createContext<SourceResolver | null>(null)

export const PROFILE_SOURCE = 'app.gather.block.defs#profileSource'

/** A source that never answers, is forbidden, or fails. The last two show as unavailable. */
export type FixtureMarker = { readonly fixture: 'loading' | 'forbidden' | 'failing' }

class Marker implements FixtureMarker {
  constructor(readonly fixture: FixtureMarker['fixture']) {}
}

const LOADING = new Marker('loading')
const FORBIDDEN = new Marker('forbidden')
const FAILING = new Marker('failing')

function stateOf(fixture: unknown, known: boolean): SourceState {
  if (!known || fixture === FORBIDDEN || fixture === FAILING) return { state: 'unavailable' }
  if (fixture === LOADING) return { state: 'loading' }
  return { state: 'ready', value: fixture }
}

/**
 * Answers sources by name from fixed values, for the gallery and tests.
 * A name it doesn't know is a missing source. Profiles are keyed by DID.
 */
export class FixtureResolver implements SourceResolver {
  static loading(): FixtureMarker {
    return LOADING
  }
  static forbidden(): FixtureMarker {
    return FORBIDDEN
  }
  static failing(): FixtureMarker {
    return FAILING
  }

  /**
   * From JSON fixtures, where `{"$fixture": "loading" | "forbidden" | "failing"}`
   * stands for a marker.
   */
  static fromJson(sources: Record<string, unknown> = {}, profiles: Record<string, unknown> = {}): FixtureResolver {
    const named: Record<string, Marker> = { loading: LOADING, forbidden: FORBIDDEN, failing: FAILING }
    const markers = (values: Record<string, unknown>) =>
      Object.fromEntries(
        Object.entries(values).map(([name, value]) => {
          const fixture = (value as { $fixture?: unknown } | null)?.$fixture
          return [name, typeof fixture === 'string' && Object.hasOwn(named, fixture) ? named[fixture] : value]
        }),
      )
    return new FixtureResolver(markers(sources), { profiles: markers(profiles) })
  }

  private readonly sources: Map<string, unknown>
  private readonly profiles: Map<string, unknown>
  private readonly listeners = new Map<string, Set<(state: SourceState) => void>>()

  constructor(sources: Record<string, unknown>, options: { profiles?: Record<string, unknown> } = {}) {
    this.sources = new Map(Object.entries(sources))
    this.profiles = new Map(Object.entries(options.profiles ?? {}))
  }

  /** Changes a source's value; watchers see it. */
  set(name: string, value: unknown): void {
    this.sources.set(name, value)
    const state = stateOf(value, true)
    for (const listener of this.listeners.get(name) ?? []) listener(state)
  }

  watch(_card: CardRef, name: string, ref: SourceRef): Observable<SourceState> {
    return {
      subscribe: (next) => {
        if (ref.$type === PROFILE_SOURCE) {
          const did = String(ref.did)
          next(stateOf(this.profiles.get(did), this.profiles.has(did)))
          return { unsubscribe() {} }
        }
        next(stateOf(this.sources.get(name), this.sources.has(name)))
        const set = this.listeners.get(name) ?? new Set()
        this.listeners.set(name, set)
        set.add(next)
        return { unsubscribe: () => set.delete(next) }
      },
    }
  }
}
