import type { BlockData } from './bindings'

/** The attributes on every rendered block's root element. */
export function blockAttrs(type: string, block: BlockData) {
  return {
    'data-block': type,
    'data-block-id': typeof block.id === 'string' && block.id !== '' ? block.id : undefined,
  }
}

/**
 * A block whose data isn't ready: a skeleton while loading, and the same
 * quiet "Unavailable" whatever the reason (missing, forbidden, failing).
 */
export function Placeholder({
  type,
  block,
  state,
}: {
  type: string
  block: BlockData
  state: 'loading' | 'unavailable'
}) {
  if (state === 'loading') {
    return (
      <div {...blockAttrs(type, block)} data-state="loading" aria-busy="true" className="g-block g-skeleton">
        <span className="g-skeleton__bar" />
        <span className="g-visually-hidden">Loading</span>
      </div>
    )
  }
  return (
    <div {...blockAttrs(type, block)} data-state="unavailable" className="g-block g-unavailable">
      Unavailable
    </div>
  )
}
