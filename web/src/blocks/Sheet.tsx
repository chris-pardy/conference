import { type KeyboardEvent, useEffect, useId, useMemo, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { CardContext, FormContext, useCard } from './context'
import { FormStore } from './forms'
import { Blocks } from './render'

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), textarea:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])'

/**
 * The elements Tab visits, in order, as a browser does: a radio group is one
 * stop, at its checked radio, or its first when none is checked.
 */
function tabStops(root: HTMLElement): HTMLElement[] {
  const all = [...root.querySelectorAll<HTMLElement>(FOCUSABLE)]
  const groups = new Map<string, HTMLInputElement[]>()
  for (const el of all) {
    if (el instanceof HTMLInputElement && el.type === 'radio' && el.name) {
      groups.set(el.name, [...(groups.get(el.name) ?? []), el])
    }
  }
  return all.filter((el) => {
    if (!(el instanceof HTMLInputElement && el.type === 'radio' && el.name)) return true
    const group = groups.get(el.name) ?? []
    return el === (group.find((r) => r.checked) ?? group[0])
  })
}

/**
 * A modal bottom sheet of blocks, opened by a button. Its blocks render on
 * the sheet surface (whatever surface the card is on) and form their own
 * form, so a submit inside sends the sheet's inputs. Focus stays inside
 * while it's open, and the page behind doesn't scroll. Escape, the close
 * button or the backdrop closes it.
 */
export function Sheet({
  title,
  blocks,
  onClose,
  returnFocus,
}: {
  title: string
  blocks: unknown
  onClose: () => void
  /** Where focus goes when the sheet closes, once the page behind is interactive again. */
  returnFocus?: HTMLElement | null
}) {
  const returnTo = useRef(returnFocus)
  returnTo.current = returnFocus
  const card = useCard()
  const scope = useMemo(() => ({ ...card, surface: 'sheet' as const, compactPrimary: undefined }), [card])
  const [form] = useState(() => new FormStore())
  const titleId = useId()
  const layer = useRef<HTMLDivElement>(null)
  const dialog = useRef<HTMLDivElement>(null)
  const close = useRef<HTMLButtonElement>(null)

  useEffect(() => {
    close.current?.focus()
    const { overflow } = document.body.style
    document.body.style.overflow = 'hidden'
    // Everything behind the sheet is inert: not focusable, not read out.
    const behind = [...document.body.children].filter((el) => el !== layer.current && !el.hasAttribute('inert'))
    for (const el of behind) el.setAttribute('inert', '')
    return () => {
      document.body.style.overflow = overflow
      for (const el of behind) el.removeAttribute('inert')
      // Only now: browsers won't focus an element that's still inert.
      returnTo.current?.focus()
    }
  }, [])

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.stopPropagation()
      onClose()
      return
    }
    if (e.key !== 'Tab' || !dialog.current) return
    // Tab moves between the sheet's own stops and wraps, so focus never leaves it.
    const stops = tabStops(dialog.current)
    if (stops.length === 0) return
    e.preventDefault()
    const active = document.activeElement
    // From a radio, start at its group's stop.
    const at = stops.findIndex(
      (el) =>
        el === active ||
        (active instanceof HTMLInputElement &&
          active.type === 'radio' &&
          el instanceof HTMLInputElement &&
          el.name === active.name),
    )
    const step = e.shiftKey ? -1 : 1
    const next = at === -1 ? (e.shiftKey ? stops.length - 1 : 0) : (at + step + stops.length) % stops.length
    stops[next].focus()
  }

  return createPortal(
    <div ref={layer} className="g-sheet-layer">
      <div className="g-sheet-backdrop" onClick={onClose} aria-hidden="true" />
      <div
        ref={dialog}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        data-surface="sheet"
        className="g-sheet"
        onKeyDown={onKeyDown}
      >
        <div className="g-sheet__head">
          <h2 id={titleId} className="g-sheet__title">
            {title}
          </h2>
          <button ref={close} type="button" className="g-button g-button--default g-sheet__close" onClick={onClose}>
            Close
          </button>
        </div>
        <CardContext value={scope}>
          <FormContext value={form}>
            <div className="g-sheet__body">
              <Blocks blocks={blocks} />
            </div>
          </FormContext>
        </CardContext>
      </div>
    </div>,
    document.body,
  )
}
