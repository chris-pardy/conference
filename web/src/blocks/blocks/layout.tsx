import { useState } from 'react'
import { type BlockData, settle, text, useBound } from '../bindings'
import { useCard } from '../context'
import { blockAttrs, Placeholder } from '../frame'
import { Blocks } from '../render'
import { RichText, safeImageSrc } from '../richtext'

type Props = { block: BlockData }

export function Section({ block }: Props) {
  const title = typeof block.title === 'string' ? block.title : undefined
  return (
    <div {...blockAttrs('section', block)} className="g-block g-section">
      {title && <h3 className="g-section__title">{title}</h3>}
      <Blocks blocks={block.blocks} />
    </div>
  )
}

export function Header({ block }: Props) {
  const bound = useBound(block, ['text'])
  const state = settle(bound)
  const value = text(bound.text)
  if (state !== 'ready' || value === undefined) {
    return <Placeholder type="header" block={block} state={state === 'loading' ? 'loading' : 'unavailable'} />
  }
  return (
    <h2 {...blockAttrs('header', block)} className="g-block g-header">
      {value}
    </h2>
  )
}

export function Divider({ block }: Props) {
  return <hr {...blockAttrs('divider', block)} className="g-block g-divider" />
}

export function Context({ block }: Props) {
  const bound = useBound(block, ['text'])
  const state = settle(bound)
  const value = text(bound.text)
  if (state !== 'ready' || value === undefined) {
    return <Placeholder type="context" block={block} state={state === 'loading' ? 'loading' : 'unavailable'} />
  }
  return (
    <p {...blockAttrs('context', block)} className="g-block g-context">
      {value}
    </p>
  )
}

export function RichTextBlock({ block }: Props) {
  const bound = useBound(block, ['text'])
  const state = settle(bound)
  const value = text(bound.text)
  if (state !== 'ready' || value === undefined) {
    return <Placeholder type="richText" block={block} state={state === 'loading' ? 'loading' : 'unavailable'} />
  }
  // Facets index the literal text, so a bound text shows plain.
  const facets = block.bind?.text ? undefined : block.facets
  return (
    <p {...blockAttrs('richText', block)} className="g-block g-richtext">
      <RichText text={value} facets={facets} />
    </p>
  )
}

export function Image({ block }: Props) {
  const { surface } = useCard()
  const bound = useBound(block, ['url', 'alt'])
  // Failure belongs to a URL: a new URL (say, a corrected binding) gets a fresh try.
  const [failedUrl, setFailedUrl] = useState<string | null>(null)
  if (surface === 'compact') return null
  const state = settle(bound)
  const url = safeImageSrc(text(bound.url))
  const alt = text(bound.alt) ?? ''
  // Missing data is the same quiet placeholder as any block's; the alt text
  // stands in only for an image that can't be shown.
  if (state !== 'ready') return <Placeholder type="image" block={block} state={state} />
  const ratio = block.aspectRatio as { width?: number; height?: number } | undefined
  const aspectRatio = ratio?.width && ratio.height ? `${ratio.width} / ${ratio.height}` : undefined
  if (!url || failedUrl === url) {
    return (
      <div {...blockAttrs('image', block)} className="g-block g-image g-image--failed" style={{ aspectRatio }}>
        <span>{alt || 'Unavailable'}</span>
      </div>
    )
  }
  return (
    <div {...blockAttrs('image', block)} className="g-block g-image" style={{ aspectRatio }}>
      <img src={url} alt={alt} loading="lazy" onError={() => setFailedUrl(url)} />
    </div>
  )
}

export function Stack({ block }: Props) {
  return (
    <div {...blockAttrs('stack', block)} className="g-block g-stack">
      <Blocks blocks={block.blocks} />
    </div>
  )
}

/** The first block id anywhere in some blocks (ids are unique across a card), or null. */
function firstId(blocks: unknown): string | null {
  for (const block of Array.isArray(blocks) ? (blocks as BlockData[]) : []) {
    if (typeof block?.id === 'string' && block.id !== '') return block.id
    const nested = firstId(block?.blocks) ?? firstId(block?.template)
    if (nested) return nested
    for (const column of Array.isArray(block?.columns) ? (block.columns as { blocks?: unknown }[]) : []) {
      const inColumn = firstId(column?.blocks)
      if (inColumn) return inColumn
    }
  }
  return null
}

export function Columns({ block }: Props) {
  const columns = Array.isArray(block.columns) ? (block.columns as { blocks?: unknown }[]) : []
  // A column's identity is the first block id in it, else its position, so a
  // column added before another doesn't remount it and lose what was typed.
  const seen = new Set<string>()
  const keys = columns.map((column, i) => {
    const id = firstId(column?.blocks)
    const key = id !== null && !seen.has(`#${id}`) ? `#${id}` : `@${i}`
    seen.add(key)
    return key
  })
  return (
    <div
      {...blockAttrs('columns', block)}
      className="g-block g-columns"
      style={{ gridTemplateColumns: `repeat(${Math.max(columns.length, 1)}, minmax(0, 1fr))` }}
    >
      {columns.map((column, i) => (
        <div key={keys[i]} className="g-column">
          <Blocks blocks={column?.blocks} />
        </div>
      ))}
    </div>
  )
}
