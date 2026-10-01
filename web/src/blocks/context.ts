import { createContext, useContext } from 'react'
import type { CardRef } from './ActionContext'
import type { Surface } from './BlockCard'
import type { FormStore } from './forms'
import type { SourceStore } from './sources'

/** What every block in a card shares. */
export interface CardScope {
  cardRef: CardRef
  store: SourceStore
  surface: Surface
  /** IANA zone for the card's times; undefined means the viewer's. */
  timeZone?: string
  /** On a compact card, the one button it keeps, wherever that button is in the tree. */
  compactPrimary?: unknown
}

export const CardContext = createContext<CardScope | null>(null)

export function useCard(): CardScope {
  const scope = useContext(CardContext)
  if (!scope) throw new Error('blocks render inside a <BlockCard>')
  return scope
}

/** One step of an item path: an item's key (the list's `key` value), or its position when it has no unique key. */
export type ItemStep = { key: unknown } | { index: number }

/**
 * The element a list is repeating over, seen by bindings as the source
 * `$item`, and its path: a step for this item and every list item it's
 * inside, outermost first. Intents sent from inside it carry the path as
 * `item`.
 */
export const ItemContext = createContext<{ item: unknown; path: ItemStep[] } | null>(null)

/** The inputs a submit block sends: the nearest list item, sheet or card. */
export const FormContext = createContext<FormStore | null>(null)

/** Where a mention of a DID links to. Hosts can route mentions in-app; the default opens the profile on Bluesky. */
export const ProfileLinkContext = createContext<(did: string) => string>((did) => `https://bsky.app/profile/${did}`)
