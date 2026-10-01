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
  /** Last ready values carried over from the store this one replaced. */
  private readonly seeded = new Map<string, SourceState>()

  /**
   * `card` is read when a source is watched, so the card's current ref (say,
   * a new cid after an edit) is used without replacing the store. A store
   * that replaces another because the card's sources changed starts from the
   * other's ready values for sources that didn't change, so bound blocks keep
   * showing data instead of dropping back to skeletons. A new resolver is a
   * new authority (another viewer), so then nothing carries over.
   */
  constructor(
    private readonly resolver: SourceResolver | null,
    private readonly card: () => CardRef,
    private readonly refs: Map<string, SourceRef>,
    previous?: SourceStore,
  ) {
    // The resolver answers as one viewer, so a different resolver is a
    // different authority: nothing carries over to it, or one person's data
    // could show to another.
    if (previous?.resolver !== resolver) return
    for (const [key, entry] of previous.entries) {
      if (entry.state.state !== 'ready') continue
      const name = key.startsWith('source:') ? key.slice('source:'.length) : null
      const same = name === null || JSON.stringify(previous?.refs.get(name)) === JSON.stringify(refs.get(name))
      if (same) this.seeded.set(key, entry.state)
    }
  }

  /** The key and ref for a named card source, or null when the card doesn't declare it. */
  named(name: string): [string, SourceRef] | null {
    const ref = this.refs.get(name)
    return ref ? [`source:${name}`, ref] : null
  }

  /** The key and ref for a DID's profile. */
  profile(did: string): [string, SourceRef] {
    return [`profile:${did}`, { $type: 'app.eventside.block.defs#profileSource', did }]
  }

  get(key: string): SourceState {
    return this.entries.get(key)?.state ?? this.seeded.get(key) ?? LOADING
  }

  /** A snapshot that changes whenever any of the keys changes. */
  version(keys: readonly string[]): string {
    return keys.map((key) => this.entries.get(key)?.version ?? 0).join(',')
  }

  subscribe(sources: readonly (readonly [string, SourceRef])[], listener: () => void): () => void {
    for (const [key, ref] of sources) {
      let entry = this.entries.get(key)
      if (!entry) {
        entry = { state: this.seeded.get(key) ?? LOADING, version: 0, listeners: new Set() }
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
        if (entry.listeners.size > 0) continue
        // Let go a moment later: a block whose bindings changed unsubscribes
        // and subscribes again in the same commit, and must find the source
        // still watched, with its value, instead of loading it again.
        queueMicrotask(() => {
          if (entry.listeners.size > 0 || this.entries.get(key) !== entry) return
          entry.unsubscribe?.()
          this.entries.delete(key)
        })
      }
    }
  }

  private start(key: string, ref: SourceRef, entry: Entry) {
    const update = (state: SourceState) => {
      // Once a source has answered, reloading keeps its last value on screen
      // (stale while revalidating), so blocks don't unmount and lose what
      // someone typed into them. Only the first load shows a skeleton.
      if (state.state === 'loading' && entry.state.state === 'ready') return
      // A real answer replaces a carried-over value for good.
      this.seeded.delete(key)
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
      // Lookups the renderer makes itself (a person's profile) have no source
      // name: "" can't collide with one, since card source names can't be empty.
      const name = key.startsWith('source:') ? key.slice('source:'.length) : ''
      const subscription = this.resolver.watch(this.card(), name, ref).subscribe((state) => {
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
