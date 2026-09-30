import { useContext, useEffect, useRef } from 'react'
import { FormContext } from './context'

export interface Field {
  id: string
  value(): unknown
  /** Checks the input, showing why when it's invalid. */
  validate(): boolean
  clear(): void
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

  /** Validates every input, so each shows its own error, and says whether all passed. */
  validate(): boolean {
    let ok = true
    for (const field of this.fields) ok = field.validate() && ok
    return ok
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

/** Registers an input with its form, reading the latest callbacks at submit time. */
export function useField(field: Field): void {
  const form = useContext(FormContext)
  const latest = useRef(field)
  latest.current = field
  const id = field.id
  useEffect(() => {
    if (!form) return
    return form.register({
      id,
      value: () => latest.current.value(),
      validate: () => latest.current.validate(),
      clear: () => latest.current.clear(),
    })
  }, [form, id])
}
