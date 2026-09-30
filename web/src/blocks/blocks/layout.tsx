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
  const [failed, setFailed] = useState(false)
  if (surface === 'compact') return null
  const state = settle(bound)
  const url = safeImageSrc(text(bound.url))
  const alt = text(bound.alt) ?? ''
  if (state === 'loading') return <Placeholder type="image" block={block} state="loading" />
  const ratio = block.aspectRatio as { width?: number; height?: number } | undefined
  const aspectRatio = ratio?.width && ratio.height ? `${ratio.width} / ${ratio.height}` : undefined
  if (!url || failed) {
    return (
      <div {...blockAttrs('image', block)} className="g-block g-image g-image--failed" style={{ aspectRatio }}>
        <span>{alt || 'Unavailable'}</span>
      </div>
    )
  }
  return (
    <div {...blockAttrs('image', block)} className="g-block g-image" style={{ aspectRatio }}>
      <img src={url} alt={alt} loading="lazy" onError={() => setFailed(true)} />
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

export function Columns({ block }: Props) {
  const columns = Array.isArray(block.columns) ? (block.columns as { blocks?: unknown }[]) : []
  return (
    <div
      {...blockAttrs('columns', block)}
      className="g-block g-columns"
      style={{ gridTemplateColumns: `repeat(${Math.max(columns.length, 1)}, minmax(0, 1fr))` }}
    >
      {columns.map((column, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: columns are positional
        <div key={i} className="g-column">
          <Blocks blocks={column?.blocks} />
        </div>
      ))}
    </div>
  )
}
