import type { CardRef } from './ActionContext'
import type { SourceRef, SourceResolver, SourceState } from './SourceResolver'

const LOADING: SourceState = { state: 'loading' }
const UNAVAILABLE: SourceState = { state: 'unavailable' }

interface Entry {
  state: SourceState
  version: number
  listeners: Set<() => void>
  unsubscribe?: () => void
}

/**
 * One card's view of its sources: it watches each source once, however many
 * blocks bind to it, only while something is subscribed, and lets blocks
 * re-render through useSyncExternalStore when a source changes.
 */
export class SourceStore {
  private readonly entries = new Map<string, Entry>()

  constructor(
    private readonly resolver: SourceResolver | null,
    private readonly card: CardRef,
    private readonly refs: Map<string, SourceRef>,
  ) {}

  /** The key and ref for a named card source, or null when the card doesn't declare it. */
  named(name: string): [string, SourceRef] | null {
    const ref = this.refs.get(name)
    return ref ? [`source:${name}`, ref] : null
  }

  /** The key and ref for a DID's profile. */
  profile(did: string): [string, SourceRef] {
    return [`profile:${did}`, { $type: 'app.gather.block.defs#profileSource', did }]
  }

  get(key: string): SourceState {
    return this.entries.get(key)?.state ?? LOADING
  }

  /** A snapshot that changes whenever any of the keys changes. */
  version(keys: readonly string[]): string {
    return keys.map((key) => this.entries.get(key)?.version ?? 0).join(',')
  }

  subscribe(sources: readonly (readonly [string, SourceRef])[], listener: () => void): () => void {
    for (const [key, ref] of sources) {
      let entry = this.entries.get(key)
      if (!entry) {
        entry = { state: LOADING, version: 0, listeners: new Set() }
        this.entries.set(key, entry)
      }
      entry.listeners.add(listener)
      if (!entry.unsubscribe) this.start(key, ref, entry)
    }
    return () => {
      for (const [key] of sources) {
        const entry = this.entries.get(key)
        if (!entry) continue
        entry.listeners.delete(listener)
        if (entry.listeners.size === 0) {
          entry.unsubscribe?.()
          this.entries.delete(key)
        }
      }
    }
  }

  private start(key: string, ref: SourceRef, entry: Entry) {
    const update = (state: SourceState) => {
      // Once a source has answered, reloading keeps its last value on screen
      // (stale while revalidating), so blocks don't unmount and lose what
      // someone typed into them. Only the first load shows a skeleton.
      if (state.state === 'loading' && entry.state.state === 'ready') return
      entry.state = state
      entry.version++
      for (const listener of [...entry.listeners]) listener()
    }
    if (!this.resolver) {
      update(UNAVAILABLE)
      entry.unsubscribe = () => {}
      return
    }
    try {
      const name = key.startsWith('source:') ? key.slice('source:'.length) : key
      const subscription = this.resolver.watch(this.card, name, ref).subscribe((state) => {
        // A resolver may answer after its block unmounted.
        if (this.entries.get(key) === entry) update(state)
      })
      entry.unsubscribe = () => subscription.unsubscribe()
    } catch {
      update(UNAVAILABLE)
      entry.unsubscribe = () => {}
    }
  }
}
