import { useEffect, useMemo, useRef, useState } from 'react'
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
import { type CardView, cards, profile } from './api'
import { type CardModule, loadModule, type ModuleOutput } from './wasm'

const VIEW_SOURCE = 'app.eventside.block.defs#viewSource'
/** The view a card binds to for its own data, as the server computes it for the viewer. */
export const CARD_DATA = 'app.eventside.card.data'

/**
 * Answers a card's sources: its own data from the server (a viewSource of
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

export interface Viewer {
  did: string
  handle: string
}

interface LiveCardProps {
  card: CardView
  viewer: Viewer
  /** The server answered an action with the card as it is now. */
  onUpdate(card: CardView): void
  onRemove?(): void
  /** A preview: actions run through the module but never reach the server. */
  preview?: boolean
}

type ModuleState = { kind: 'none' } | { kind: 'loading' } | { kind: 'ready'; module: CardModule } | { kind: 'failed' }

/**
 * A card from the server, live. Its blocks bind to the card's data, which
 * the server pushes down as people act. When the card carries a wasm
 * module, each action runs through the module first, on this device: it can
 * redraw the card at once, and decides whether the action goes to the server.
 */
export function LiveCard({ card, viewer, onUpdate, onRemove, preview }: LiveCardProps) {
  // One resolver per card: new data is pushed into it, so bound blocks update in place.
  // biome-ignore lint/correctness/useExhaustiveDependencies: a new card id is a new resolver
  const resolver = useMemo(() => new CardDataResolver(card.data), [card.id])
  useEffect(() => resolver.set(card.data), [resolver, card.data])

  const [moduleState, setModuleState] = useState<ModuleState>(card.module ? { kind: 'loading' } : { kind: 'none' })
  const [moduleUi, setModuleUi] = useState<CardUi | null>(null)
  const local = useRef<unknown>(null)
  const latest = useRef({ card, moduleUi })
  latest.current = { card, moduleUi }
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [showSource, setShowSource] = useState(false)
  const [lastRun, setLastRun] = useState<number | null>(null)

  const apply = (out: ModuleOutput) => {
    if (out.local !== undefined) local.current = out.local
    if (out.ui && typeof out.ui === 'object') setModuleUi(out.ui)
  }

  const run = (module: CardModule, event: Parameters<CardModule['update']>[0]['event']): ModuleOutput => {
    const { card, moduleUi } = latest.current
    const started = performance.now()
    const out = module.update({
      event,
      ui: moduleUi ?? card.ui,
      base: card.ui,
      data: card.data,
      local: local.current,
      viewer,
    })
    setLastRun(performance.now() - started)
    apply(out)
    return out
  }

  // Load the module, and let it draw the card.
  // biome-ignore lint/correctness/useExhaustiveDependencies: runs once per module URL
  useEffect(() => {
    if (!card.module) {
      setModuleState({ kind: 'none' })
      setModuleUi(null)
      return
    }
    let live = true
    setModuleState({ kind: 'loading' })
    loadModule(card.module)
      .then((module) => {
        if (!live) return
        setModuleState({ kind: 'ready', module })
        run(module, { type: 'init' })
      })
      .catch((err) => {
        console.error(err)
        if (live) setModuleState({ kind: 'failed' })
      })
    return () => {
      live = false
    }
  }, [card.module])

  // The server's data changed: the module may want to redraw.
  // biome-ignore lint/correctness/useExhaustiveDependencies: reacts to new data only
  useEffect(() => {
    if (moduleState.kind === 'ready') run(moduleState.module, { type: 'data' })
  }, [card.data])

  // biome-ignore lint/correctness/useExhaustiveDependencies: run reads the latest card through a ref
  const host = useMemo<ActionHost>(
    () => ({
      onAction(intent: ActionIntent) {
        let actionId = intent.actionId
        let value = intent.value
        const current = moduleState
        if (current.kind === 'ready') {
          let out: ModuleOutput
          try {
            out = run(current.module, {
              type: 'action',
              blockId: intent.blockId,
              actionId,
              value,
              ...(intent.item ? { item: intent.item } : {}),
            })
          } catch (err) {
            console.error(err)
            setError('This card’s module failed.')
            return
          }
          const forward = out.forward ?? true
          if (forward === false) return
          if (typeof forward === 'object') {
            actionId = forward.actionId ?? actionId
            if ('value' in forward) value = forward.value
          }
        }
        if (preview) return
        setBusy(true)
        setError(null)
        cards
          .act({ cardId: card.id, blockId: intent.blockId, actionId, value, item: intent.item })
          .then(onUpdate)
          .catch((err: Error) => setError(err.message))
          .finally(() => setBusy(false))
      },
    }),
    [card.id, moduleState, onUpdate, preview],
  )

  const cardRef = useMemo(
    () => ({ uri: `eventside:card/${card.id}`, cid: String(card.version) }),
    [card.id, card.version],
  )
  const shown = moduleUi ?? card.ui
  const mine = card.author === viewer.did

  return (
    <div className="live-card" data-card-id={card.id} data-busy={busy || undefined}>
      <SourceResolverContext value={resolver}>
        <ActionContext value={host}>
          <BlockCard cardRef={cardRef} card={shown} surface="feed" />
        </ActionContext>
      </SourceResolverContext>
      <div className="live-card__meta">
        {card.module && (
          <span
            className={`live-card__chip live-card__chip--${moduleState.kind}`}
            title="This card carries a wasm module that answers actions on your device"
          >
            wasm
            {moduleState.kind === 'ready' && lastRun !== null && ` · ${lastRun < 1 ? '<1' : lastRun.toFixed(0)}ms`}
            {moduleState.kind === 'loading' && ' · loading'}
            {moduleState.kind === 'failed' && ' · failed'}
          </span>
        )}
        {busy && <span className="live-card__sync">sending…</span>}
        {error && (
          <span className="live-card__error" role="alert">
            {error}
          </span>
        )}
        <span className="live-card__spacer" />
        <button type="button" className="live-card__link" onClick={() => setShowSource((s) => !s)}>
          {showSource ? 'Hide source' : 'Source'}
        </button>
        {mine && onRemove && (
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
            <summary>Card kit UI{moduleUi ? ' (as drawn by the module)' : ''}</summary>
            <pre>{JSON.stringify(shown, null, 2)}</pre>
          </details>
          <details open>
            <summary>Data from the server</summary>
            <pre>{JSON.stringify(card.data, null, 2)}</pre>
          </details>
          {Object.keys(card.reducers).length > 0 && (
            <details>
              <summary>Reducers</summary>
              <pre>{JSON.stringify(card.reducers, null, 2)}</pre>
            </details>
          )}
          {moduleUi && local.current !== null && (
            <details>
              <summary>Module state (this device only)</summary>
              <pre>{JSON.stringify(local.current, null, 2)}</pre>
            </details>
          )}
        </div>
      )}
    </div>
  )
}
