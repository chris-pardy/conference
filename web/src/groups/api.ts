// The client for events, feeds and cards (see DEMO.md). Everything lives on
// our server; atproto is only how people sign in.

import { api } from '../api'
import type { Card as CardUi } from '../blocks/BlockCard'

export type Rsvp = 'going' | 'maybe' | 'no'

/** An event as it appears in lists. */
export interface EventSummary {
  id: string
  title: string
  description: string
  /** Epoch ms. */
  startsAt: number
  endsAt: number | null
  timeZone: string
  location: string
  /** The host's DID. */
  host: string
  parent: { id: string; title: string } | null
  going: number
  maybe: number
  rsvp: Rsvp | null
  /** Whether the viewer is going, so can see the event's feeds, people and plans. */
  canView: boolean
  subEventCount: number
}

export interface EventDetail extends EventSummary {
  /** DIDs of people going, at most 24. */
  attendees: string[]
}

export interface Feed {
  id: string
  name: string
  eventId: string
}

/** A card as the server sends it down. */
export interface CardView {
  id: string
  feedId: string
  author: string
  version: number
  createdAt: number
  /** The card-kit UI as authored: the view's `base`. */
  ui: CardUi
  config: unknown
  /** The shared state, which only the server reducer changes. */
  state: unknown
  /** Platform context for this viewer: `{viewer, event?}`. */
  context: CardContext
  /** Where to fetch the card's wasm module. */
  module: string | null
  eventId: string | null
}

export interface CardContext {
  viewer?: { did: string; handle: string }
  event?: {
    id: string
    title: string
    description: string
    startsAt: number
    endsAt: number | null
    timeZone: string
    location: string
    host: string
    going: number
    maybe: number
    rsvp: Rsvp | null
    url: string
  }
  [key: string]: unknown
}

/** An action as the client sends it to the server: the server stamps the rest of `meta`. */
export interface OutgoingAction {
  type: string
  payload?: unknown
  meta?: { blockId?: string; item?: unknown }
}

/** An action the server's reducer emitted, sent down to clients. */
export interface ServerAction {
  type: string
  payload?: unknown
  to?: string
  meta?: Record<string, unknown>
}

/** An update, as the generic payload sent down an event's stream. */
export type StreamPayload =
  | { type: 'hello' }
  | { type: 'resync' }
  | { type: 'card'; feedId: string; card: CardView }
  | { type: 'cardRemoved'; feedId: string; cardId: string }
  | { type: 'feed'; feed: Feed }
  | { type: 'actions'; cardId: string; actions: ServerAction[] }
  | { type: 'event'; event: EventDetail }
  | { type: 'subEvent'; event: EventSummary }

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

export interface NewEvent {
  title: string
  description?: string
  startsAt: number
  endsAt?: number | null
  timeZone?: string
  location?: string
  /** Makes it a sub-event (a plan) of this event. */
  parentId?: string
  /** The parent's feed to post the event card in. */
  feedId?: string
}

export const events = {
  list: () => get<{ events: EventSummary[] }>('event.list').then((r) => r.events),
  get: (id: string) => get<{ event: EventDetail; feeds: Feed[]; subEvents: EventSummary[] }>('event.get', { id }),
  create: (event: NewEvent) => post<{ event: EventSummary }>('event.create', event).then((r) => r.event),
  rsvp: (id: string, status: Rsvp) => post<{ event: EventDetail }>('event.rsvp', { id, status }).then((r) => r.event),
  /** The event's live updates. Needs the viewer to be going. */
  subscribe(id: string, onPayload: (payload: StreamPayload) => void): () => void {
    const source = new EventSource(`${X}event.subscribe?${new URLSearchParams({ id })}`)
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
  create: (eventId: string, name: string) => post<Feed>('feed.create', { eventId, name }),
  cards: (id: string) => get<{ cards: CardView[] }>('feed.getCards', { id }).then((r) => r.cards),
}

/** The default module, for cards without their own: generic reducers named in `config`. */
export const STD_MODULE = `${X}card.getModule?hash=std`

export const cards = {
  create: (feedId: string, ui: unknown, config: unknown, module?: string) =>
    post<{ card: CardView }>('card.create', { feedId, ui, config, module }).then((r) => r.card),
  remove: (cardId: string) => post<object>('card.delete', { cardId }),
  dispatch: (cardId: string, actions: OutgoingAction[]) =>
    post<{ card: CardView }>('card.dispatch', { cardId, actions }).then((r) => r.card),
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
