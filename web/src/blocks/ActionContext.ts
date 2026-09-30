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
}

export interface ActionHost {
  onAction(intent: ActionIntent): void
}

export const ActionContext = createContext<ActionHost | null>(null)
