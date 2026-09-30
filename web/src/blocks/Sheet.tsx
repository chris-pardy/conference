import { useEffect, useId, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { FormContext } from './context'
import { FormStore } from './forms'
import { Blocks } from './render'

/**
 * A modal bottom sheet of blocks, opened by a button. It is its own form, so
 * a submit inside sends the sheet's inputs. Escape, the close button or the
 * backdrop closes it.
 */
export function Sheet({ title, blocks, onClose }: { title: string; blocks: unknown; onClose: () => void }) {
  const [form] = useState(() => new FormStore())
  const titleId = useId()
  const close = useRef<HTMLButtonElement>(null)
  const latestClose = useRef(onClose)
  latestClose.current = onClose

  useEffect(() => {
    close.current?.focus()
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') latestClose.current()
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [])

  return createPortal(
    <div className="g-sheet-layer">
      <div className="g-sheet-backdrop" onClick={onClose} aria-hidden="true" />
      <div role="dialog" aria-modal="true" aria-labelledby={titleId} data-surface="sheet" className="g-sheet">
        <div className="g-sheet__head">
          <h2 id={titleId} className="g-sheet__title">
            {title}
          </h2>
          <button ref={close} type="button" className="g-button g-button--default g-sheet__close" onClick={onClose}>
            Close
          </button>
        </div>
        <FormContext value={form}>
          <div className="g-sheet__body">
            <Blocks blocks={blocks} />
          </div>
        </FormContext>
      </div>
    </div>,
    document.body,
  )
}
