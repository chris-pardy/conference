import { useId, useState } from 'react'
import { type BlockData, numeric, readPointer, settle, text, useBound } from '../bindings'
import { FormContext, ItemContext } from '../context'
import { FormStore } from '../forms'
import { blockAttrs, Placeholder } from '../frame'
import { Blocks } from '../render'

type Props = { block: BlockData }

export function List({ block }: Props) {
  // The list's items are a binding, even though the lexicon names it `items` rather than putting it in `bind`.
  const bound = useBound({ ...block, bind: { items: block.items as never } }, ['items'])
  const items = bound.items
  if (items.state === 'loading') return <Placeholder type="list" block={block} state="loading" />
  if (items.state === 'unavailable' || !Array.isArray(items.value)) {
    return <Placeholder type="list" block={block} state="unavailable" />
  }
  if (items.value.length === 0) {
    return (
      <div {...blockAttrs('list', block)} data-state="empty" className="g-block g-list g-list--empty">
        {typeof block.empty === 'string' ? block.empty : 'Nothing here yet.'}
      </div>
    )
  }
  const keyPath = typeof block.key === 'string' ? block.key : undefined
  const seen = new Set<string>()
  return (
    <ul {...blockAttrs('list', block)} className="g-block g-list">
      {items.value.map((item, i) => {
        // Items keep their identity by key, so reordering moves them with their state.
        const read = keyPath !== undefined ? readPointer(item, keyPath) : { found: false }
        let key = read.found ? `k:${JSON.stringify(read.value)}` : `i:${i}`
        if (seen.has(key)) key = `${key}#${i}`
        seen.add(key)
        // Intents from inside the item name it by its key, else its position.
        const itemKey = read.found ? (read as { value: unknown }).value : i
        return <ListItem key={key} item={item} itemKey={itemKey} template={block.template} />
      })}
    </ul>
  )
}

/** One element of a list: the template, seeing the element as `$item`, as its own form. */
function ListItem({ item, itemKey, template }: { item: unknown; itemKey: unknown; template: unknown }) {
  const [form] = useState(() => new FormStore())
  return (
    <li className="g-list__item">
      <ItemContext value={{ item, key: itemKey }}>
        <FormContext value={form}>
          <Blocks blocks={template} />
        </FormContext>
      </ItemContext>
    </li>
  )
}

export function Progress({ block }: Props) {
  const bound = useBound(block, ['label', 'value', 'max'])
  const labelId = useId()
  const state = settle(bound)
  const value = numeric(bound.value)
  if (state !== 'ready' || value === undefined) {
    return <Placeholder type="progress" block={block} state={state === 'loading' ? 'loading' : 'unavailable'} />
  }
  const max = numeric(bound.max) ?? 100
  // The lexicon's minimum only holds for a literal max; a bound one can be anything.
  if (max <= 0) return <Placeholder type="progress" block={block} state="unavailable" />
  const percent = Math.round(Math.min(Math.max(value / max, 0), 1) * 100)
  const label = text(bound.label)
  return (
    <div {...blockAttrs('progress', block)} className="g-block g-progress">
      <div className="g-progress__head">
        {label && (
          <span id={labelId} className="g-progress__label">
            {label}
          </span>
        )}
        <span className="g-progress__value">{percent}%</span>
      </div>
      <div
        role="progressbar"
        aria-labelledby={label ? labelId : undefined}
        aria-valuemin={0}
        aria-valuemax={max}
        aria-valuenow={Math.min(Math.max(value, 0), max)}
        aria-valuetext={`${percent}%`}
        className="g-progress__track"
      >
        <div className="g-progress__fill" style={{ width: `${percent}%` }} />
      </div>
    </div>
  )
}

export function Stat({ block }: Props) {
  const bound = useBound(block, ['value', 'label'])
  const state = settle(bound)
  const value = text(bound.value)
  if (state !== 'ready' || value === undefined) {
    return <Placeholder type="stat" block={block} state={state === 'loading' ? 'loading' : 'unavailable'} />
  }
  const label = text(bound.label)
  return (
    <div {...blockAttrs('stat', block)} className="g-block g-stat">
      <span className="g-stat__value">{value}</span>
      {label && (
        <>
          {' '}
          <span className="g-stat__label">{label}</span>
        </>
      )}
    </div>
  )
}

const TONES = new Set(['neutral', 'info', 'warning', 'success', 'danger'])

export function Badge({ block }: Props) {
  const bound = useBound(block, ['text'])
  const state = settle(bound)
  const value = text(bound.text)
  if (state !== 'ready' || value === undefined) {
    return <Placeholder type="badge" block={block} state={state === 'loading' ? 'loading' : 'unavailable'} />
  }
  const tone = typeof block.tone === 'string' && TONES.has(block.tone) ? block.tone : 'neutral'
  return (
    <span {...blockAttrs('badge', block)} data-tone={tone} className={`g-block g-badge g-badge--${tone}`}>
      {value}
    </span>
  )
}
