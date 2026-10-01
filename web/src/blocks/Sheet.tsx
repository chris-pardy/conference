import {
  createContext,
  type KeyboardEvent,
  type RefObject,
  useContext,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
} from 'react'
import { createPortal } from 'react-dom'
import { CardContext, FormContext, useCard } from './context'
import { FormStore } from './forms'
import { Blocks } from './render'

/**
 * The page stays unscrollable while any sheet is open. Sheets can open from
 * inside sheets and can all unmount at once, in any order, so the lock is
 * counted: the first sheet saves the page's overflow, the last restores it.
 */
let openSheets = 0
let savedOverflow = ''

function lockScroll(): () => void {
  if (openSheets++ === 0) {
    savedOverflow = document.body.style.overflow
    document.body.style.overflow = 'hidden'
  }
  return () => {
    if (--openSheets === 0) document.body.style.overflow = savedOverflow
  }
}

/** The dialog of the sheet a block is inside, if any. */
const EnclosingSheet = createContext<RefObject<HTMLDivElement | null> | null>(null)

/** Open sheets, innermost last: Escape closes only the innermost. */
const openStack: symbol[] = []

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
  const cardRoot = useRef(card.root)
  cardRoot.current = card.root
  const enclosing = useRef(useContext(EnclosingSheet))
  const scope = useMemo(() => ({ ...card, surface: 'sheet' as const, compactPrimary: undefined }), [card])
  const [form] = useState(() => new FormStore())
  const titleId = useId()
  const layer = useRef<HTMLDivElement>(null)
  const dialog = useRef<HTMLDivElement>(null)
  const close = useRef<HTMLButtonElement>(null)
  const latestClose = useRef(onClose)
  latestClose.current = onClose

  useEffect(() => {
    close.current?.focus()
    const unlockScroll = lockScroll()
    // Everything behind the sheet is inert: not focusable, not read out.
    const behind = [...document.body.children].filter((el) => el !== layer.current && !el.hasAttribute('inert'))
    for (const el of behind) el.setAttribute('inert', '')
    // Escape inside a sheet is handled by the sheet itself (below). If focus
    // has dropped out of every sheet (a click on text, a focused block
    // removed), the innermost sheet still closes, and the host never sees the
    // key: this listener runs first, in the capture phase, and stops it.
    const me = Symbol('sheet')
    openStack.push(me)
    const onEscape = (e: globalThis.KeyboardEvent) => {
      if (e.key !== 'Escape' || openStack[openStack.length - 1] !== me) return
      if (e.target instanceof Element && e.target.closest('.g-sheet')) return
      e.preventDefault()
      e.stopPropagation()
      latestClose.current()
    }
    document.addEventListener('keydown', onEscape, true)
    return () => {
      document.removeEventListener('keydown', onEscape, true)
      openStack.splice(openStack.indexOf(me), 1)
      unlockScroll()
      for (const el of behind) el.removeAttribute('inert')
      // Only now: browsers won't focus an element that's still inert.
      // If an edit removed the opener, focus goes to the sheet this one was
      // opened from (the card behind it is still inert), else to the card.
      // Leave focus alone if it's already somewhere live outside any sheet
      // (an outer sheet closing in the same commit just gave it back).
      const active = document.activeElement
      if (active && active !== document.body && active.isConnected && !active.closest('.g-sheet')) return
      const target = returnTo.current?.isConnected
        ? returnTo.current
        : (enclosing.current?.current ?? cardRoot.current?.current)
      target?.focus()
    }
  }, [])

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Escape') {
      // Closes this sheet only: not an outer sheet, nor the host around the card.
      e.preventDefault()
      e.stopPropagation()
      onClose()
      return
    }
    if (e.key !== 'Tab' || !dialog.current) return
    // Events bubble through portals to an outer sheet; only the innermost traps.
    e.stopPropagation()
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
        // Focusable itself, so a click on its text keeps focus inside it.
        tabIndex={-1}
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
            <EnclosingSheet value={dialog}>
              <div className="g-sheet__body">
                <Blocks blocks={blocks} />
              </div>
            </EnclosingSheet>
          </FormContext>
        </CardContext>
      </div>
    </div>,
    document.body,
  )
}
