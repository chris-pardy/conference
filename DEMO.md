# Demo: events, sub-events and redux cards

A demo branch. Everything lives in our database; atproto is only sign-in.

## Events

- The top level is **events**. An event is a group of people with event
  details: title, description, start, end, time zone, location, host.
- Anyone signed in can see a **top-level** event and RSVP to it:
  `going`, `maybe` or `no`.
- Saying **going** opens the event: its feeds, cards, attendees and
  sub-events. Hosts are always going.
- A **sub-event** is an event created inside another event (the core
  feature: "make plans inside the conference"). It is a full event with its
  own page, RSVPs and feeds, but only people who can view the parent
  (going to it) can see it or RSVP. Sub-events can nest.
- Creating a sub-event posts an **event card** in one of the parent's feeds
  (the first feed unless one is named). The card shows the sub-event live
  (going count, your RSVP) and RSVPs from the card.

## Cards are redux

`(state, action) -> (state', actions[])`, isomorphic: the same wasm module
runs in the browser (`env: "client"`) and on the server (`env: "server"`).

- **Shared state** lives on the server, one JSON value per card. Only the
  server reducer changes it. Everyone in the event receives it.
- **Local state** lives on one device. Only the client reducer changes it.
- A tap on a block becomes an action
  `{type: <block action id>, payload: <value>, meta: {origin: "ui", blockId, item?}}`
  reduced on the client first.
- Actions the **client** reducer emits are sent to the server
  (`card.dispatch`), where the server stamps `meta` (id, actor, at,
  origin: "client") and reduces them with `env: "server"`.
- Actions the **server** reducer emits:
  - `type` starting with `@`: a **platform effect**, run by the server as
    the actor. Today: `@rsvp {status}` RSVPs the actor to the card's linked
    event.
  - anything else goes **down** to clients (`origin: "server"`), to
    everyone or, with `to: <did>`, to one person, who reduce it with
    `env: "client"`.
- `@init` is reduced once on the server when a card is created (initial
  shared state, from `state: null`) and once per device when a card loads
  (initial local state).
- The **view** runs on the client after any state change and turns
  `{shared, local, context, viewer, config, base}` into the card-kit UI and
  the data its bindings read.

### Wasm ABI (v2)

Exports: `memory`, `alloc(len: i32) -> i32`, `reduce(ptr: i32, len: i32) -> i64`,
`view(ptr: i32, len: i32) -> i64`. JSON in, JSON out, the output packed as
`(ptr << 32) | len`. No imports: no clock, RNG, network or DOM; the host
passes `seed` (u32) and `now` (epoch ms).

`reduce` input:

```jsonc
{
  "env": "client" | "server",
  "state": null,     // client: this device's local state; server: the shared state
  "shared": null,    // the shared state (on the server, the same as state)
  "action": {
    "type": "roll",
    "payload": null,
    "meta": {
      "id": "…",                  // server: a unique action id
      "actor": { "did": "…", "handle": "…", "displayName": "…" },
      "at": 0,                    // epoch ms
      "origin": "ui" | "client" | "server" | "platform",
      "blockId": "…", "item": [ { "key": "…" } ]  // ui actions from blocks
    }
  },
  "config": {},      // the card's authored config
  "base": {},        // the card's authored card-kit UI
  "context": {},     // see Context (client: this viewer's; server: {})
  "viewer": { "did": "…", "handle": "…" },  // client only
  "seed": 0, "now": 0
}
```

`reduce` output: `{ "state"?: any, "actions"?: [{ "type", "payload"?, "to"? }] }`.
A missing `state` means unchanged.

`view` input: `{ "shared", "local", "config", "base", "context", "viewer", "now" }`.
Output: `{ "ui"?: card, "data"?: any }`. `ui` defaults to `base`; `data`
is what the card's `data` source (a viewSource of `app.eventside.card.data`)
binds to.

### Context

Computed by the server for each viewer:

```jsonc
{
  "viewer": { "did": "…", "handle": "…" },
  "event": {          // only on an event card: the linked event
    "id": "…", "title": "…", "description": "…",
    "startsAt": 0, "endsAt": 0, "timeZone": "Europe/Amsterdam",
    "location": "…", "host": "did:…",
    "going": 3, "maybe": 1, "rsvp": "going" | "maybe" | "no" | null,
    "url": "/e/<id>"
  }
}
```

### Modules (`cards/`)

- `std`: the default for any card without its own module. `config` maps
  action types to reducers: `vote`, `append`, `count`, `leaderboard`,
  `upvote:<append type>`, `none`. Client: forwards every ui action to the
  server. Server: folds it into shared state. View: per-viewer data
  (`mine`, `voted`, percents, sorted lists), zero-filled for every action
  the UI declares, in the shapes below.
  - vote `{total, mine, counts, percent, options[{value,count,percent,mine}]}`
  - append `{total, items[{id,did,handle,at,value,votes,voted,mine,…form fields}]}`
    (most upvoted, then newest; at most 50)
  - leaderboard `{total, mine, items[{id,did,handle,rank,score,value,mine}]}`
  - count `{total, mine}`
- `dice`: rolls are local. Posting a roll sends it to the server, which
  keeps the posted rolls and, when someone takes the lowest roll, tells
  everyone (`lowest`).
- `quiz`: answers are checked on the device for instant feedback.
  Submitting sends the answers; the server re-scores them with the same
  code (no cheating), updates the leaderboard and tells the player
  (`scored`, to them only).
- `event`: the event card. Renders `context.event`; an RSVP tap is
  reduced on the client (pending), forwarded, and the server reducer emits
  `@rsvp`.

## API (`/xrpc/app.eventside.*`)

All need a signed-in session. Hidden and unknown ids are both `NotFound`.

| Method | | |
|---|---|---|
| `event.list` | GET | `{events: EventSummary[]}`: top-level events, and sub-events of events you're going to, by start |
| `event.get?id=` | GET | `{event: EventDetail, feeds: Feed[], subEvents: EventSummary[]}`; feeds and subEvents empty unless you can view |
| `event.create` | POST | `{title, description?, startsAt, endsAt?, timeZone?, location?, parentId?, feedId?}` → `{event: EventSummary}` |
| `event.rsvp` | POST | `{id, status}` → `{event: EventDetail}` |
| `event.subscribe?id=` | GET | SSE, needs going. Payloads below |
| `feed.create` | POST | `{eventId, name}` → `Feed` |
| `feed.getCards?id=` | GET | `{cards: CardView[]}` |
| `card.create` | POST | `{feedId, ui, config?, module? (base64)}` → `{card}` |
| `card.delete` | POST | `{cardId}` (author or host) |
| `card.dispatch` | POST | `{cardId, actions: [{type, payload?, meta?: {blockId?, item?}}]}` → `{card}` |
| `card.getModule?hash=` | GET | the wasm bytes, immutable |
| `actor.getProfile?did=` | GET | `{did, handle, displayName, avatar}` |

```ts
EventSummary = { id, title, description, startsAt, endsAt, timeZone, location,
  host, parent: { id, title } | null, going, maybe,
  rsvp: 'going' | 'maybe' | 'no' | null, canView, subEventCount }
EventDetail = EventSummary & { attendees: string[] /* dids going, ≤ 24 */ }
Feed = { id, name, eventId }
CardView = { id, feedId, author, version, createdAt, ui, config, state,
  context, module /* URL */, eventId: string | null }
```

Stream payloads: `hello`, `resync`, `{type: "card", feedId, card}`,
`{type: "cardRemoved", feedId, cardId}`, `{type: "feed", feed}`,
`{type: "actions", cardId, actions}` (server-emitted, already filtered for
this viewer), `{type: "event", event: EventDetail}` (this event changed),
`{type: "subEvent", event: EventSummary}` (a sub-event was created or changed).
