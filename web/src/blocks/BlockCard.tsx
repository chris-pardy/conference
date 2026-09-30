import { useContext, useMemo, useState } from 'react'
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

/** The first few blocks, without media or inputs, then the first primary button. */
function compactBlocks(blocks: unknown[]): unknown[] {
  const shown = blocks.filter((b) => {
    const type = blockType(b)
    return type !== null && !NOT_COMPACT.has(type)
  })
  const primary = blocks.find((b) => blockType(b) === 'button' && variantOf(b as never) === 'primary')
  return [...shown.slice(0, COMPACT_BLOCKS), ...(primary ? [primary] : [])]
}

/**
 * Renders a card for a surface. Sources come from the SourceResolver in
 * context, and interactive blocks hand their intents to the ActionContext.
 */
export function BlockCard({ cardRef, card, surface, onDismiss }: BlockCardProps) {
  const resolver = useContext(SourceResolverContext)
  const [dismissed, setDismissed] = useState(false)
  const [form] = useState(() => new FormStore())

  const refs = useMemo(() => {
    const map = new Map<string, SourceRef>()
    for (const source of Array.isArray(card.sources) ? card.sources : []) {
      const ref = source?.ref as SourceRef | undefined
      if (typeof source?.name === 'string' && ref && typeof ref.$type === 'string') map.set(source.name, ref)
    }
    return map
  }, [card.sources])
  const { uri, cid } = cardRef
  const store = useMemo(() => new SourceStore(resolver, { uri, cid }, refs), [resolver, uri, cid, refs])
  const scope = useMemo(
    () => ({
      cardRef: { uri, cid },
      store,
      surface,
      timeZone: typeof card.timeZone === 'string' ? card.timeZone : undefined,
    }),
    [uri, cid, store, surface, card.timeZone],
  )

  if (dismissed) return null
  const blocks = Array.isArray(card.blocks) ? (card.blocks as unknown[]) : []

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
          <Blocks blocks={surface === 'compact' ? compactBlocks(blocks) : blocks} />
        </article>
      </FormContext>
    </CardContext>
  )
}
