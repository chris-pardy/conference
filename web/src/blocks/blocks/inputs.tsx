import { useContext, useEffect, useId, useRef, useState } from 'react'
import { ActionContext } from '../ActionContext'
import { type BlockData, settle, text, useBound } from '../bindings'
import { FormContext, ItemContext, useCard } from '../context'
import { useField } from '../forms'
import { blockAttrs, Placeholder } from '../frame'
import { Sheet } from '../Sheet'

type Props = { block: BlockData }

const VARIANTS = new Set(['default', 'primary', 'danger'])
const graphemes = new Intl.Segmenter(undefined, { granularity: 'grapheme' })

/** A block's variant, or "default" for one the vocabulary doesn't have. */
export function variantOf(block: BlockData): string {
  return typeof block.variant === 'string' && VARIANTS.has(block.variant) ? block.variant : 'default'
}

const str = (v: unknown) => (typeof v === 'string' ? v : undefined)

/** Hands an intent to the card's host. */
function useSend() {
  const { cardRef } = useCard()
  const host = useContext(ActionContext)
  const item = useContext(ItemContext)
  return (blockId: string, actionId: string, value: unknown) => {
    // Inside a list item, the intent says which item it came from: the path of
    // keys from the outermost list in.
    host?.onAction({ card: cardRef, blockId, actionId, value, ...(item ? { item: item.path } : {}) })
  }
}

export function Button({ block }: Props) {
  const { surface, compactPrimary } = useCard()
  const send = useSend()
  const bound = useBound(block, ['label', 'value'])
  const [open, setOpen] = useState(false)
  const opener = useRef<HTMLButtonElement>(null)
  const lastLabel = useRef<string | undefined>(undefined)
  const sheet = block.opens as { title?: unknown; blocks?: unknown } | undefined
  // A sheet an edit removes closes for good: restoring it doesn't reopen it.
  useEffect(() => {
    if (!sheet && open) setOpen(false)
  }, [sheet, open])
  const isOpen = open && sheet !== undefined
  const variant = variantOf(block)
  // A compact card keeps only its one primary button.
  if (surface === 'compact' && block !== compactPrimary) return null
  const state = settle(bound)
  const label = text(bound.label)
  // The lexicon types a button's value as a string, so a bound value must be text too.
  const value = text(bound.value)
  const badValue = block.bind?.value !== undefined && value === undefined
  const ready = state === 'ready' && !!label && !badValue
  if (ready) lastLabel.current = label
  // While its sheet is open, a button whose data drops out keeps its last
  // label, so the sheet (and what was typed in it) stays.
  const shown = ready ? label : isOpen ? lastLabel.current : undefined
  if (!shown) {
    return <Placeholder type="button" block={block} state={state === 'loading' ? 'loading' : 'unavailable'} />
  }
  const id = str(block.id)
  const action = str(block.action)
  return (
    <div {...blockAttrs('button', block)} data-variant={variant} className="g-block g-button-block">
      <button
        ref={opener}
        type="button"
        className={`g-button g-button--${variant}`}
        aria-haspopup={sheet ? 'dialog' : undefined}
        aria-expanded={sheet ? isOpen : undefined}
        onClick={() => {
          if (sheet) setOpen(true)
          else if (id && action) send(id, action, value)
        }}
      >
        {shown}
      </button>
      {sheet && isOpen && (
        <Sheet
          title={str(sheet.title) ?? shown}
          blocks={sheet.blocks}
          onClose={() => setOpen(false)}
          returnFocus={opener.current}
        />
      )}
    </div>
  )
}

export function ButtonGroup({ block }: Props) {
  const { surface } = useCard()
  const send = useSend()
  if (surface === 'compact') return null
  const options = Array.isArray(block.buttons) ? (block.buttons as { label?: unknown; value?: unknown }[]) : []
  const id = str(block.id)
  const action = str(block.action)
  return (
    <fieldset {...blockAttrs('buttonGroup', block)} className="g-block g-button-group">
      {str(block.label) && <legend className="g-label">{str(block.label)}</legend>}
      <div className="g-button-group__row">
        {options.map((option, i) => (
          <button
            // biome-ignore lint/suspicious/noArrayIndexKey: options are positional
            key={i}
            type="button"
            className="g-button g-button--default"
            onClick={() => id && action && send(id, action, option.value)}
          >
            {str(option.label)}
          </button>
        ))}
      </div>
    </fieldset>
  )
}

export function TextInput({ block }: Props) {
  const { surface } = useCard()
  const [value, setValue] = useState('')
  const [error, setError] = useState<string | null>(null)
  const inputId = useId()
  const errorId = useId()
  const id = str(block.id) ?? ''
  const required = block.required === true
  const min = typeof block.minLength === 'number' ? block.minLength : undefined
  const max = typeof block.maxLength === 'number' ? block.maxLength : undefined

  // Length counts characters as people see them (graphemes: an emoji family is one), as atproto's maxGraphemes does.
  const length = (s: string) => [...graphemes.segment(s)].length
  const check = (v: string): string | null => {
    if (v.trim() === '') return required ? 'This field is required.' : null
    if (max !== undefined && length(v) > max) return `Use at most ${max} characters (now ${length(v)}).`
    if (min !== undefined && length(v) < min) return `Use at least ${min} characters.`
    return null
  }
  useField(
    {
      id,
      // Whitespace alone counts as empty, as in validation.
      value: () => (value.trim() === '' ? undefined : value),
      validate: () => {
        const problem = check(value)
        setError(problem)
        return problem === null
      },
      clear: () => {
        setValue('')
        setError(null)
      },
      target: () => document.getElementById(inputId),
    },
    surface !== 'compact',
  )
  if (surface === 'compact') return null

  const Field = block.multiline === true ? 'textarea' : 'input'
  return (
    <div {...blockAttrs('textInput', block)} className="g-block g-field">
      <label className="g-label" htmlFor={inputId}>
        {str(block.label)}
      </label>
      <Field
        id={inputId}
        className="g-input"
        value={value}
        placeholder={str(block.placeholder)}
        aria-required={required || undefined}
        aria-invalid={error ? true : undefined}
        aria-describedby={error ? errorId : undefined}
        onChange={(e) => {
          setValue(e.target.value)
          if (error) setError(null)
        }}
      />
      {max !== undefined && (
        <span className="g-hint" aria-hidden="true">
          {length(value)}/{max}
        </span>
      )}
      {error && (
        <span id={errorId} className="g-error">
          {error}
        </span>
      )}
    </div>
  )
}

export function Select({ block }: Props) {
  const { surface } = useCard()
  const multiple = block.multiple === true
  const [chosen, setChosen] = useState<string[]>([])
  const [error, setError] = useState<string | null>(null)
  const name = useId()
  const labelId = useId()
  const errorId = useId()
  const id = str(block.id) ?? ''
  const options = (
    Array.isArray(block.options) ? (block.options as { label?: unknown; value?: unknown }[]) : []
  ).filter((o) => typeof o.value === 'string') as { label?: unknown; value: string }[]
  const limit = multiple && typeof block.maxSelections === 'number' ? block.maxSelections : undefined
  const required = block.required === true
  // Only choices that are still options count: an edit to the card may have removed some.
  const current = options.map((o) => o.value).filter((v) => chosen.includes(v))

  useField(
    {
      id,
      // Selections are sent in the options' order.
      // Nothing chosen is left out, as for an empty text input.
      value: () => (current.length === 0 ? undefined : multiple ? current : current[0]),
      validate: () => {
        const problem =
          required && current.length === 0
            ? 'Choose an option.'
            : limit !== undefined && current.length > limit
              ? `Choose at most ${limit} (now ${current.length}).`
              : null
        setError(problem)
        return problem === null
      },
      clear: () => {
        setChosen([])
        setError(null)
      },
      target: () => {
        const choices = [...document.getElementsByName(name)] as HTMLInputElement[]
        return choices.find((c) => c.checked && !c.disabled) ?? choices.find((c) => !c.disabled) ?? null
      },
    },
    surface !== 'compact',
  )
  if (surface === 'compact') return null

  const full = limit !== undefined && current.length >= limit
  const toggle = (value: string) => {
    setError(null)
    if (!multiple) return setChosen([value])
    setChosen(current.includes(value) ? current.filter((v) => v !== value) : full ? current : [...current, value])
  }
  const described = [limit !== undefined ? `${labelId}-hint` : null, error ? errorId : null].filter(Boolean).join(' ')

  const choices = options.map((option) => {
    const checked = current.includes(option.value)
    return (
      <label key={option.value} className="g-choice">
        <input
          type={multiple ? 'checkbox' : 'radio'}
          name={name}
          value={option.value}
          checked={checked}
          disabled={multiple && full && !checked}
          onChange={() => toggle(option.value)}
        />
        <span>{str(option.label)}</span>
      </label>
    )
  })
  const extras = (
    <>
      {limit !== undefined && (
        <span id={`${labelId}-hint`} className="g-hint">
          Choose up to {limit}.
        </span>
      )}
      {error && (
        <span id={errorId} className="g-error">
          {error}
        </span>
      )}
    </>
  )

  if (multiple) {
    return (
      <fieldset {...blockAttrs('select', block)} className="g-block g-select" aria-describedby={described || undefined}>
        <legend className="g-label">{str(block.label)}</legend>
        {choices}
        {extras}
      </fieldset>
    )
  }
  return (
    <div
      {...blockAttrs('select', block)}
      role="radiogroup"
      aria-labelledby={labelId}
      aria-required={required || undefined}
      aria-describedby={described || undefined}
      className="g-block g-select"
    >
      <span id={labelId} className="g-label">
        {str(block.label)}
      </span>
      {choices}
      {extras}
    </div>
  )
}

export function Submit({ block }: Props) {
  const { surface } = useCard()
  const form = useContext(FormContext)
  const send = useSend()
  if (surface === 'compact') return null
  const variant = variantOf(block)
  const id = str(block.id)
  const action = str(block.action)
  return (
    <div {...blockAttrs('submit', block)} data-variant={variant} className="g-block g-button-block">
      <button
        type="button"
        className={`g-button g-button--${variant}`}
        onClick={() => {
          if (!form || !id || !action) return
          if (!form.validate()) return
          send(id, action, form.values())
          form.clear()
        }}
      >
        {str(block.label)}
      </button>
    </div>
  )
}
