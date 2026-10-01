import { createContext } from 'react'

export interface CardRef {
  uri: string
  cid?: string
}

/** What an interactive block hands its host. Sending it is block-actions' job. */
export interface ActionIntent {
  card: CardRef
  blockId: string
  actionId: string
  value: unknown
  /**
   * Sent from inside a list item: a step for that item and every list item
   * it's inside, outermost first. A step is `{key}` (the list's `key` value)
   * or `{index}` (its position, when it has no unique key). Absent outside
   * lists.
   */
  item?: ({ key: unknown } | { index: number })[]
}

export interface ActionHost {
  onAction(intent: ActionIntent): void
}

export const ActionContext = createContext<ActionHost | null>(null)
