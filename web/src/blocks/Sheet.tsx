import { type KeyboardEvent, useEffect, useId, useMemo, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { CardContext, FormContext, useCard } from './context'
import { FormStore } from './forms'
import { Blocks } from './render'

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), textarea:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])'

/**
 * A modal bottom sheet of blocks, opened by a button. Its blocks render on
 * the sheet surface (whatever surface the card is on) and form their own
 * form, so a submit inside sends the sheet's inputs. Focus stays inside
 * while it's open, and the page behind doesn't scroll. Escape, the close
 * button or the backdrop closes it.
 */
export function Sheet({ title, blocks, onClose }: { title: string; blocks: unknown; onClose: () => void }) {
  const card = useCard()
  const scope = useMemo(() => ({ ...card, surface: 'sheet' as const, compactPrimary: undefined }), [card])
  const [form] = useState(() => new FormStore())
  const titleId = useId()
  const dialog = useRef<HTMLDivElement>(null)
  const close = useRef<HTMLButtonElement>(null)

  useEffect(() => {
    close.current?.focus()
    const { overflow } = document.body.style
    document.body.style.overflow = 'hidden'
    return () => {
      document.body.style.overflow = overflow
    }
  }, [])

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.stopPropagation()
      onClose()
      return
    }
    if (e.key !== 'Tab' || !dialog.current) return
    const focusable = [...dialog.current.querySelectorAll<HTMLElement>(FOCUSABLE)]
    if (focusable.length === 0) return
    const first = focusable[0]
    const last = focusable[focusable.length - 1]
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault()
      last.focus()
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault()
      first.focus()
    }
  }

  return createPortal(
    <div className="g-sheet-layer">
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
