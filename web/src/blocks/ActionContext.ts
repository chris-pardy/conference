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
   * Sent from inside a list item: the keys of that item and of every list
   * item it's inside, outermost first (one key for a single list). Absent
   * outside lists.
   */
  item?: unknown[]
}

export interface ActionHost {
  onAction(intent: ActionIntent): void
}

export const ActionContext = createContext<ActionHost | null>(null)
