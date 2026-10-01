import { useContext, useEffect, useRef } from 'react'
import { FormContext } from './context'

export interface Field {
  id: string
  value(): unknown
  /** Checks the input, showing why when it's invalid. */
  validate(): boolean
  clear(): void
  /** Moves focus to the input, so assistive tech reads it and its error. */
  focus(): void
}

/** The inputs of one form, in the order they mounted (document order). */
export class FormStore {
  private readonly fields = new Set<Field>()

  register(field: Field): () => void {
    this.fields.add(field)
    return () => {
      this.fields.delete(field)
    }
  }

  /**
   * Validates every input, so each shows its own error, and says whether all
   * passed. On a failure, focus goes to the first invalid input, whose error
   * a screen reader then announces through its description.
   */
  validate(): boolean {
    let first: Field | undefined
    for (const field of this.fields) {
      if (!field.validate()) first ??= field
    }
    first?.focus()
    return first === undefined
  }

  values(): Record<string, unknown> {
    const values: Record<string, unknown> = {}
    for (const field of this.fields) {
      const value = field.value()
      if (value !== undefined) values[field.id] = value
    }
    return values
  }

  clear(): void {
    for (const field of this.fields) field.clear()
  }
}

/**
 * Registers an input with its form, reading the latest callbacks at submit
 * time. An input that isn't shown (`enabled: false`) stays out of the form.
 */
export function useField(field: Field, enabled = true): void {
  const form = useContext(FormContext)
  const latest = useRef(field)
  latest.current = field
  const id = field.id
  useEffect(() => {
    if (!form || !enabled) return
    return form.register({
      id,
      value: () => latest.current.value(),
      validate: () => latest.current.validate(),
      clear: () => latest.current.clear(),
      focus: () => latest.current.focus(),
    })
  }, [form, id, enabled])
}
