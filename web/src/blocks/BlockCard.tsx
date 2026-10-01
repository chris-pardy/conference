import { useContext, useMemo, useRef, useState } from 'react'
import type { Main as CardRecord } from '../lexicon/types/app/gather/block/card'
import type { CardRef } from './ActionContext'
import { variantOf } from './blocks/inputs'
import { CardContext, FormContext } from './context'
import { FormStore } from './forms'
import { Blocks, blockType } from './render'
import { type SourceRef, SourceResolverContext } from './SourceResolver'
import { SourceStore } from './sources'
import './blocks.css'

/** Where a card is shown. */
export type Surface = 'feed' | 'compact' | 'sheet' | 'ephemeral'

/** An `app.gather.block.card` record. */
export type Card = CardRecord

export interface BlockCardProps {
  cardRef: CardRef
  card: Card
  surface: Surface
  /** Called when an ephemeral card is dismissed. */
  onDismiss?: () => void
}

/** How many blocks a compact card shows, besides its primary button. */
const COMPACT_BLOCKS = 3
/** Blocks a compact card leaves out: media and inputs. */
const NOT_COMPACT = new Set(['image', 'qr', 'textInput', 'select', 'submit', 'buttonGroup', 'button'])

/** A layout block's children: what's rendered inside it on the card itself (not in lists or sheets). */
function children(block: unknown): unknown[] {
  const b = block as { blocks?: unknown; columns?: unknown }
  switch (blockType(block)) {
    case 'section':
    case 'stack':
      return Array.isArray(b.blocks) ? b.blocks : []
    case 'columns':
      return Array.isArray(b.columns) ? b.columns.flatMap((c) => (Array.isArray(c?.blocks) ? c.blocks : [])) : []
    default:
      return []
  }
}

/** Every block in the tree, in document order. */
function* walk(blocks: unknown[]): Generator<unknown> {
  for (const block of blocks) {
    yield block
    yield* walk(children(block))
  }
}

/**
 * What a compact card shows: its first few top-level blocks, leaving out
 * media and inputs (nested ones hide themselves), and the first primary
 * button anywhere in the card, which is the only button it keeps.
 */
function compactLayout(blocks: unknown[]): { blocks: unknown[]; primary: unknown } {
  const shown = blocks
    .filter((b) => {
      const type = blockType(b)
      return type !== null && !NOT_COMPACT.has(type)
    })
    .slice(0, COMPACT_BLOCKS)
  const primary = [...walk(blocks)].find((b) => blockType(b) === 'button' && variantOf(b as never) === 'primary')
  const inside = primary !== undefined && [...walk(shown)].includes(primary)
  return { blocks: primary && !inside ? [...shown, primary] : shown, primary }
}

/**
 * Renders a card for a surface. Sources come from the SourceResolver in
 * context, and interactive blocks hand their intents to the ActionContext.
 */
export function BlockCard({ cardRef, card, surface, onDismiss }: BlockCardProps) {
  const resolver = useContext(SourceResolverContext)
  const [dismissed, setDismissed] = useState(false)
  const [form] = useState(() => new FormStore())

  // Keyed on the sources' content, so a host that re-fetches or re-parses the
  // same card keeps its store, and bound blocks don't drop back to loading.
  const sourcesKey = JSON.stringify(Array.isArray(card.sources) ? card.sources : [])
  const refs = useMemo(() => {
    const map = new Map<string, SourceRef>()
    for (const source of JSON.parse(sourcesKey) as { name?: unknown; ref?: SourceRef }[]) {
      const ref = source?.ref
      if (typeof source?.name === 'string' && ref && typeof ref.$type === 'string') map.set(source.name, ref)
    }
    return map
  }, [sourcesKey])
  const { uri, cid } = cardRef
  // The store outlives cid changes (an edited card is the same card): it
  // reads the current ref when it watches. When it must be replaced (a new
  // resolver or new sources), the new one starts from the old one's values.
  const current = useRef({ uri, cid })
  current.current = { uri, cid }
  const previous = useRef<{ uri: string; store: SourceStore } | null>(null)
  const store = useMemo(() => {
    const before = previous.current?.uri === uri ? previous.current.store : undefined
    return new SourceStore(resolver, () => current.current, refs, before)
  }, [resolver, uri, refs])
  previous.current = { uri, store }

  const all = Array.isArray(card.blocks) ? (card.blocks as unknown[]) : []
  const compact = useMemo(() => (surface === 'compact' ? compactLayout(all) : null), [surface, all])
  const scope = useMemo(
    () => ({
      cardRef: { uri, cid },
      store,
      surface,
      timeZone: typeof card.timeZone === 'string' ? card.timeZone : undefined,
      compactPrimary: compact?.primary,
    }),
    [uri, cid, store, surface, card.timeZone, compact],
  )

  if (dismissed) return null
  const blocks = compact ? compact.blocks : all

  return (
    <CardContext value={scope}>
      <FormContext value={form}>
        <article className={`g-card g-card--${surface}`} data-card={uri} data-surface={surface}>
          {surface === 'ephemeral' && (
            <div className="g-card__ephemeral">
              <span className="g-card__only-you">Only you can see this</span>
              <button
                type="button"
                className="g-button g-button--default g-card__dismiss"
                onClick={() => {
                  setDismissed(true)
                  onDismiss?.()
                }}
              >
                Dismiss
              </button>
            </div>
          )}
          <Blocks blocks={blocks} />
        </article>
      </FormContext>
    </CardContext>
  )
}
