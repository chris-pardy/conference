import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Link } from 'react-router'
import { ActionContext, type ActionHost, type ActionIntent } from '../blocks/ActionContext'
import { BlockCard, type Card as CardUi } from '../blocks/BlockCard'
import {
  type Observable,
  PROFILE_SOURCE,
  type SourceRef,
  type SourceResolver,
  SourceResolverContext,
  type SourceState,
} from '../blocks/SourceResolver'
import { type CardView, cards, profile, type ServerAction, STD_MODULE } from './api'
import { type Action, type CardModule, loadModule, type ReduceOutput } from './wasm'

const VIEW_SOURCE = 'app.eventside.block.defs#viewSource'
/** The view a card binds to for its own data: what its module's view returns as `data`. */
export const CARD_DATA = 'app.eventside.card.data'

/**
 * Answers a card's sources: its own data (a viewSource of
 * `app.eventside.card.data`), and people's profiles. Anything else is
 * unavailable.
 */
export class CardDataResolver implements SourceResolver {
  private readonly listeners = new Set<(state: SourceState) => void>()

  constructor(private data: unknown) {}

  set(data: unknown): void {
    this.data = data
    for (const listener of this.listeners) listener({ state: 'ready', value: data })
  }

  watch(_card: unknown, _name: string, ref: SourceRef): Observable<SourceState> {
    return {
      subscribe: (next) => {
        if (ref.$type === PROFILE_SOURCE) {
          let live = true
          next({ state: 'loading' })
          profile(String(ref.did)).then((p) => {
            if (live) next(p ? { state: 'ready', value: p } : { state: 'unavailable' })
          })
          return {
            unsubscribe() {
              live = false
            },
          }
        }
        if (ref.$type === VIEW_SOURCE && ref.view === CARD_DATA) {
          next({ state: 'ready', value: this.data })
          this.listeners.add(next)
          return { unsubscribe: () => this.listeners.delete(next) }
        }
        next({ state: 'unavailable' })
        return { unsubscribe() {} }
      },
    }
  }
}

// Actions the server's reducer emits arrive on the event's stream; the page
// hands them here, and each card picks up its own.
type ServerActionsListener = (actions: ServerAction[]) => void
const listeners = new Map<string, Set<ServerActionsListener>>()

export const serverActions = {
  deliver(cardId: string, actions: ServerAction[]): void {
    for (const listener of listeners.get(cardId) ?? []) listener(actions)
  },
  listen(cardId: string, listener: ServerActionsListener): () => void {
    let set = listeners.get(cardId)
    if (!set) {
      set = new Set()
      listeners.set(cardId, set)
    }
    set.add(listener)
    return () => set.delete(listener)
  },
}

export interface Viewer {
  did: string
  handle: string
}

interface LiveCardProps {
  card: CardView
  viewer: Viewer
  /** The server answered a dispatch with the card as it is now. */
  onUpdate(card: CardView): void
  onRemove?(): void
  /** May the viewer delete this card (its author, or the event's host)? */
  canRemove?: boolean
  /**
   * A preview: no server. The same module runs here twice, as the client
   * and as the server, so the preview behaves as the posted card will.
   */
  preview?: boolean
}

type ModuleState = { kind: 'loading' } | { kind: 'ready'; module: CardModule } | { kind: 'failed' }

/** One line of the card's action log. */
interface LogEntry {
  n: number
  /** Where the action went or came from. */
  where: 'tap' | 'client' | 'to server' | 'from server' | 'server (preview)' | 'effect'
  type: string
  payload: unknown
}

const LOG_SIZE = 20
let logCounter = 0

/**
 * Each card's local state and action log on this device, kept across page
 * changes and reloads: local state belongs to the device, not to the page.
 */
const deviceStore = {
  memory: new Map<string, { local: unknown; log: LogEntry[] }>(),
  get(key: string | null) {
    if (!key) return undefined
    let found = this.memory.get(key)
    if (!found) {
      try {
        const saved = localStorage.getItem(`card:${key}`)
        if (saved) found = { local: JSON.parse(saved), log: [] }
      } catch {
        // No storage (private window, blocked): memory only.
      }
      if (found) this.memory.set(key, found)
    }
    return found
  },
  set(key: string | null, patch: Partial<{ local: unknown; log: LogEntry[] }>) {
    if (!key) return
    const next = { local: null, log: [], ...this.memory.get(key), ...patch }
    this.memory.set(key, next)
    if ('local' in patch) {
      try {
        localStorage.setItem(`card:${key}`, JSON.stringify(next.local))
      } catch {
        // Memory only.
      }
    }
  },
}

const actor = (viewer: Viewer) => ({ did: viewer.did, handle: viewer.handle })

/**
 * A card from the server, live, as a little redux store on each side.
 *
 * - Shared state comes from the server; only the server's reducer changes it.
 * - Local state lives on this device; only the client reducer changes it.
 * - A tap becomes an action, reduced here first (`env: "client"`). Whatever
 *   the client reducer emits goes to the server, which reduces it with the
 *   same module (`env: "server"`), and may emit actions back down to us.
 * - The module's view turns shared + local state into the card-kit UI and
 *   the data its bindings read, after every change.
 */
export function LiveCard({ card, viewer, onUpdate, onRemove, canRemove, preview }: LiveCardProps) {
  const moduleUrl = card.module ?? STD_MODULE
  // One resolver per card: new data is pushed into it, so bound blocks update in place.
  // biome-ignore lint/correctness/useExhaustiveDependencies: a new card id is a new resolver
  const resolver = useMemo(() => new CardDataResolver({}), [card.id])

  // Where this card's local state is kept on this device (a preview keeps none).
  const storeKey = preview ? null : `${viewer.did}|${card.id}`
  const [moduleState, setModuleState] = useState<ModuleState>({ kind: 'loading' })
  const [local, setLocalState] = useState<unknown>(() => deviceStore.get(storeKey)?.local ?? null)
  // In a preview, the shared state the in-browser "server" keeps.
  const [previewShared, setPreviewShared] = useState<unknown>(null)
  const shared = preview ? previewShared : card.state

  // The latest of everything, for callbacks that outlive a render.
  const latest = useRef({ card, local, shared })
  latest.current = { card, local, shared }

  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [showSource, setShowSource] = useState(false)
  // How long the module took last time, in ms: shown on the card's chip.
  const lastRun = useRef<number | null>(null)
  const [log, setLog] = useState<LogEntry[]>(() => deviceStore.get(storeKey)?.log ?? [])

  const record = useCallback(
    (where: LogEntry['where'], type: string, payload: unknown) => {
      setLog((l) => {
        const next = [{ n: ++logCounter, where, type, payload }, ...l].slice(0, LOG_SIZE)
        deviceStore.set(storeKey, { log: next })
        return next
      })
    },
    [storeKey],
  )

  const setLocal = (next: unknown) => {
    latest.current.local = next
    deviceStore.set(storeKey, { local: next })
    setLocalState(next)
  }
  const setShared = (next: unknown) => {
    latest.current.shared = next
    setPreviewShared(next)
  }

  const timed = <T,>(f: () => T): T => {
    const started = performance.now()
    try {
      return f()
    } finally {
      lastRun.current = performance.now() - started
    }
  }

  /** Reduces an action on this device, and returns what it emitted. */
  const reduceClient = (module: CardModule, action: Action): Action[] => {
    const { card, local, shared } = latest.current
    const out: ReduceOutput = timed(() =>
      module.reduce({
        env: 'client',
        state: local,
        shared,
        action,
        config: card.config,
        base: card.ui,
        context: card.context,
        viewer,
      }),
    )
    if (out.state !== undefined) setLocal(out.state)
    return Array.isArray(out.actions) ? out.actions : []
  }

  /**
   * The preview's stand-in for the server: the same module, reducing with
   * `env: "server"` in the browser. Its emitted actions come back to this
   * device as the server's would; effects (`@…`) need the real server.
   */
  const reduceServerHere = (module: CardModule, action: Action) => {
    const { card, shared } = latest.current
    const stamped: Action = {
      ...action,
      meta: {
        ...action.meta,
        id: `preview-${++logCounter}`,
        actor: actor(viewer),
        at: Date.now(),
        origin: action.type === '@init' ? 'platform' : 'client',
      },
    }
    const out: ReduceOutput = module.reduce({
      env: 'server',
      state: shared,
      shared,
      action: stamped,
      config: card.config,
      base: card.ui,
      context: {},
    })
    if (out.state !== undefined) setShared(out.state)
    for (const emitted of out.actions ?? []) {
      if (emitted.type.startsWith('@')) {
        record('effect', emitted.type, emitted.payload)
        continue
      }
      if (emitted.to && emitted.to !== viewer.did) continue
      record('from server', emitted.type, emitted.payload)
      reduceClient(module, { ...emitted, meta: { ...emitted.meta, origin: 'server' } })
    }
  }

  const send = (module: CardModule, emitted: Action[], from?: ActionIntent) => {
    if (emitted.length === 0) return
    for (const a of emitted) record(preview ? 'server (preview)' : 'to server', a.type, a.payload)
    if (preview) {
      for (const a of emitted) reduceServerHere(module, a)
      return
    }
    setBusy(true)
    setError(null)
    cards
      .dispatch(
        card.id,
        emitted.map((a) => ({
          type: a.type,
          payload: a.payload ?? null,
          meta: {
            blockId: a.meta?.blockId ?? from?.blockId,
            item: a.meta?.item ?? from?.item,
          },
        })),
      )
      .then(onUpdate)
      .catch((err: Error) => setError(err.message))
      .finally(() => setBusy(false))
  }

  // Load the module, then start this device's local state (and, in a
  // preview, the stand-in server's shared state).
  // biome-ignore lint/correctness/useExhaustiveDependencies: runs once per module and card
  useEffect(() => {
    let live = true
    setModuleState({ kind: 'loading' })
    loadModule(moduleUrl)
      .then((module) => {
        if (!live) return
        try {
          if (preview) reduceServerHere(module, { type: '@init', payload: null })
          // A card this device has seen before picks up where it left off.
          const saved = deviceStore.get(storeKey)
          if (saved) {
            setLocal(saved.local)
          } else {
            setLocal(null)
            const init: Action = { type: '@init', payload: null, meta: { origin: 'platform' } }
            const emitted = reduceClient(module, init)
            send(module, emitted)
          }
          setModuleState({ kind: 'ready', module })
        } catch (err) {
          console.error(err)
          setModuleState({ kind: 'failed' })
        }
      })
      .catch((err) => {
        console.error(err)
        if (live) setModuleState({ kind: 'failed' })
      })
    return () => {
      live = false
    }
  }, [moduleUrl, card.id, preview, storeKey])

  // Actions the server emitted for this card, reduced on this device.
  // biome-ignore lint/correctness/useExhaustiveDependencies: reduceClient reads the latest through a ref
  useEffect(() => {
    if (moduleState.kind !== 'ready' || preview) return
    const module = moduleState.module
    return serverActions.listen(card.id, (actions) => {
      for (const action of actions) {
        if (action.to && action.to !== viewer.did) continue
        record('from server', action.type, action.payload)
        try {
          send(module, reduceClient(module, { ...action, meta: { ...action.meta, origin: 'server' } }))
        } catch (err) {
          console.error(err)
          setError('This card’s module failed.')
        }
      }
    })
  }, [card.id, moduleState, preview, viewer.did])

  // The view: shared + local state (and the platform's context) in, card-kit UI and bound data out.
  const view = useMemo(() => {
    const fallback = { ui: card.ui, data: { ...(shared as object), viewer } }
    if (moduleState.kind !== 'ready') return { ...fallback, failed: false }
    try {
      const started = performance.now()
      const out = moduleState.module.view({
        shared,
        local,
        config: card.config,
        base: card.ui,
        context: card.context,
        viewer,
      })
      lastRun.current = performance.now() - started
      return {
        ui: (out.ui && typeof out.ui === 'object' ? out.ui : card.ui) as CardUi,
        data: out.data ?? fallback.data,
        failed: false,
      }
    } catch (err) {
      console.error(err)
      return { ...fallback, failed: true }
    }
  }, [moduleState, shared, local, card.config, card.ui, card.context, viewer])

  useEffect(() => resolver.set(view.data), [resolver, view.data])

  // biome-ignore lint/correctness/useExhaustiveDependencies: send and reduceClient read the latest through a ref
  const host = useMemo<ActionHost>(
    () => ({
      onAction(intent: ActionIntent) {
        if (moduleState.kind !== 'ready') return
        const module = moduleState.module
        const action: Action = {
          type: intent.actionId,
          payload: intent.value ?? null,
          meta: { origin: 'ui', blockId: intent.blockId, ...(intent.item ? { item: intent.item } : {}) },
        }
        record('tap', action.type, action.payload)
        let emitted: Action[]
        try {
          emitted = reduceClient(module, action)
        } catch (err) {
          console.error(err)
          setError('This card’s module failed.')
          return
        }
        if (emitted.length === 0) record('client', action.type, 'kept on this device')
        send(module, emitted, intent)
      },
    }),
    [card.id, moduleState, onUpdate, preview],
  )

  const cardRef = useMemo(
    () => ({ uri: `eventside:card/${card.id}`, cid: String(card.version) }),
    [card.id, card.version],
  )
  // The server names the standard module by its alias.
  const custom = card.module !== null && !card.module.includes('hash=std')

  return (
    <div className="live-card" data-card-id={card.id} data-busy={busy || undefined}>
      <SourceResolverContext value={resolver}>
        <ActionContext value={host}>
          <BlockCard cardRef={cardRef} card={view.ui} surface="feed" />
        </ActionContext>
      </SourceResolverContext>
      <div className="live-card__meta">
        <span
          className={`live-card__chip live-card__chip--${view.failed ? 'failed' : moduleState.kind}`}
          title={
            custom
              ? 'This card carries its own wasm module, run here and on the server'
              : 'This card runs the standard wasm module, here and on the server'
          }
        >
          {custom ? 'wasm' : 'std'}
          {moduleState.kind === 'ready' &&
            lastRun.current !== null &&
            ` · ${lastRun.current < 1 ? '<1' : lastRun.current.toFixed(0)}ms`}
          {moduleState.kind === 'loading' && ' · loading'}
          {(moduleState.kind === 'failed' || view.failed) && ' · failed'}
        </span>
        {busy && <span className="live-card__sync">sending…</span>}
        {error && (
          <span className="live-card__error" role="alert">
            {error}
          </span>
        )}
        <span className="live-card__spacer" />
        {card.eventId && card.context.event && (
          <Link className="live-card__open" to={`/e/${card.eventId}`}>
            Open plan →
          </Link>
        )}
        <button type="button" className="live-card__link" onClick={() => setShowSource((s) => !s)}>
          {showSource ? 'Hide source' : 'Source'}
        </button>
        {canRemove && onRemove && (
          <button
            type="button"
            className="live-card__link"
            onClick={() => {
              if (confirm('Delete this card for everyone?')) onRemove()
            }}
          >
            Delete
          </button>
        )}
      </div>
      {showSource && (
        <div className="live-card__source">
          <details open>
            <summary>Actions</summary>
            {log.length === 0 ? (
              <p className="gx-muted">Tap something on the card.</p>
            ) : (
              <ol className="live-card__log">
                {log.map((e) => (
                  <li key={e.n}>
                    <span className={`live-card__where live-card__where--${e.where.replace(/[^a-z]+/g, '-')}`}>
                      {e.where}
                    </span>
                    <code>{e.type}</code>
                    {e.payload !== null && e.payload !== undefined && (
                      <code className="live-card__payload">{JSON.stringify(e.payload)}</code>
                    )}
                  </li>
                ))}
              </ol>
            )}
          </details>
          <details open>
            <summary>Shared state {preview ? '(the in-browser server)' : '(from the server)'}</summary>
            <pre>{JSON.stringify(shared, null, 2)}</pre>
          </details>
          <details>
            <summary>Local state (this device only)</summary>
            <pre>{JSON.stringify(local, null, 2)}</pre>
          </details>
          <details>
            <summary>Context</summary>
            <pre>{JSON.stringify(card.context, null, 2)}</pre>
          </details>
          <details>
            <summary>Config</summary>
            <pre>{JSON.stringify(card.config, null, 2)}</pre>
          </details>
          <details>
            <summary>Card kit UI{view.ui !== card.ui ? ' (as drawn by the view)' : ''}</summary>
            <pre>{JSON.stringify(view.ui, null, 2)}</pre>
          </details>
        </div>
      )}
    </div>
  )
}
