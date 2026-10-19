// The client for groups, feeds and cards. Everything lives on our server;
// atproto is only how people sign in.

import { api } from '../api'
import type { Card as CardUi } from '../blocks/BlockCard'

export interface GroupSummary {
  id: string
  name: string
  description: string
  role: string
  memberCount: number
  feedCount: number
}

export interface Group {
  id: string
  name: string
  description: string
  invite: string
  role: string
  memberCount: number
  members: string[]
}

export interface Feed {
  id: string
  name: string
  groupId: string
}

/** A card as the server sends it down: its card-kit UI, and its data as this viewer sees it. */
export interface CardView {
  id: string
  feedId: string
  author: string
  version: number
  createdAt: number
  ui: CardUi
  reducers: Record<string, string>
  data: Record<string, unknown>
  /** Where to fetch the card's wasm module, if it has one. */
  module: string | null
}

/** An action, as the generic payload sent up for any block on any card. */
export interface ActionPayload {
  cardId: string
  blockId: string
  actionId: string
  value: unknown
  item?: unknown
}

/** An update, as the generic payload sent down a group's stream. */
export type StreamPayload =
  | { type: 'hello' }
  | { type: 'resync' }
  | { type: 'card'; feedId: string; card: CardView }
  | { type: 'cardRemoved'; feedId: string; cardId: string }
  | { type: 'feed'; feed: Feed }

export class ApiError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message)
  }
}

const X = '/xrpc/app.eventside.'

async function get<T>(method: string, params: Record<string, string> = {}): Promise<T> {
  const query = new URLSearchParams(params).toString()
  const res = await api(`${X}${method}${query ? `?${query}` : ''}`)
  return answer<T>(res)
}

async function post<T>(method: string, body: unknown): Promise<T> {
  const res = await api(`${X}${method}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })
  return answer<T>(res)
}

async function answer<T>(res: Response): Promise<T> {
  const body = await res.json().catch(() => ({}))
  if (!res.ok) throw new ApiError(res.status, body.message ?? `HTTP ${res.status}`)
  return body as T
}

export const groups = {
  list: () => get<{ groups: GroupSummary[] }>('group.list').then((r) => r.groups),
  get: (id: string) => get<{ group: Group; feeds: Feed[] }>('group.get', { id }),
  create: (name: string, description: string) => post<{ id: string }>('group.create', { name, description }),
  join: (invite: string) => post<{ id: string }>('group.join', { invite }),
  /** The group's live updates. */
  subscribe(id: string, onPayload: (payload: StreamPayload) => void): () => void {
    const source = new EventSource(`${X}group.subscribe?${new URLSearchParams({ id })}`)
    source.onmessage = (event) => {
      try {
        onPayload(JSON.parse(event.data))
      } catch {
        // A payload we can't read is skipped.
      }
    }
    // The browser reconnects by itself; on reconnect, catch up on what was missed.
    let dropped = false
    source.onerror = () => {
      dropped = true
    }
    source.onopen = () => {
      if (dropped) onPayload({ type: 'resync' })
      dropped = false
    }
    return () => source.close()
  },
}

export const feeds = {
  create: (groupId: string, name: string) => post<Feed>('feed.create', { groupId, name }),
  cards: (id: string) => get<{ cards: CardView[] }>('feed.getCards', { id }).then((r) => r.cards),
}

export const cards = {
  create: (feedId: string, ui: unknown, reducers: Record<string, string>, module?: string) =>
    post<{ card: CardView }>('card.create', { feedId, ui, reducers, module }).then((r) => r.card),
  update: (cardId: string, ui: unknown) => post<{ card: CardView }>('card.update', { cardId, ui }).then((r) => r.card),
  remove: (cardId: string) => post<object>('card.delete', { cardId }),
  act: (payload: ActionPayload) => post<{ card: CardView }>('card.action', payload).then((r) => r.card),
}

export interface Profile {
  did: string
  handle: string
  displayName?: string | null
  avatar?: string | null
}

const profiles = new Map<string, Promise<Profile | null>>()

export function profile(did: string): Promise<Profile | null> {
  let found = profiles.get(did)
  if (!found) {
    found = get<Profile>('actor.getProfile', { did }).catch(() => null)
    profiles.set(did, found)
  }
  return found
}
