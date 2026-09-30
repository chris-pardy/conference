import { createContext } from 'react'
import type { CardRef } from './ActionContext'

export type SourceState = { state: 'loading' } | { state: 'ready'; value: unknown } | { state: 'unavailable' }

export interface Observable<T> {
  subscribe(next: (value: T) => void): { unsubscribe(): void }
}

/** A `#sourceRef` from the card's lexicon. */
export type SourceRef = { $type: string; [field: string]: unknown }

/** The seam between the renderer and wherever data comes from. */
export interface SourceResolver {
  watch(card: CardRef, name: string, ref: SourceRef): Observable<SourceState>
}

export const SourceResolverContext = createContext<SourceResolver | null>(null)

/** A source that never answers, is forbidden, or fails. The last two show as unavailable. */
export type FixtureMarker = { readonly fixture: 'loading' | 'forbidden' | 'failing' }

/**
 * Answers sources by name from fixed values, for the gallery and tests.
 * A name it doesn't know is a missing source. Profiles are keyed by DID.
 */
export class FixtureResolver implements SourceResolver {
  static loading(): FixtureMarker {
    throw new Error('not implemented')
  }
  static forbidden(): FixtureMarker {
    throw new Error('not implemented')
  }
  static failing(): FixtureMarker {
    throw new Error('not implemented')
  }

  constructor(_sources: Record<string, unknown>, _options: { profiles?: Record<string, unknown> } = {}) {
    throw new Error('not implemented')
  }

  /** Changes a source's value; watchers see it. */
  set(_name: string, _value: unknown): void {
    throw new Error('not implemented')
  }

  watch(_card: CardRef, _name: string, _ref: SourceRef): Observable<SourceState> {
    throw new Error('not implemented')
  }
}
