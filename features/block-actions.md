---
status: analysis
impact:
depends-on: [ui-blocks, space-sync, attendee-sign-in]
branch:
tests-commit:
---

# Block actions

## Summary

This makes UI blocks interactive and live. An attendee's interaction with
a card becomes an **action**. The action:

1. passes through **middleware**: sandboxed JS that can accept, reject,
   rewrite the value, or answer with ephemeral blocks
2. is written into the space as the attendee
3. updates live **views** such as vote tallies and question lists, which
   bound blocks show in real time

Split out of [`ui-blocks`](ui-blocks.md) in design review round 1. It
carries that brainstorm's choices on actions, middleware and live data,
plus the critique's fixes.

## Experience

**Acting.** An attendee taps a button, picks an option, or types and
submits. Within a moment:

- the card shows the accepted state
- or it shows the rejection message ("Voting has closed", "One vote each")
- or an ephemeral card only they see appears ("Thanks! 62% agree with you")

Tapping twice, or retrying after a network blip, never counts twice.

**Watching.** Bound data updates live while the card is on screen:

- tally bars move
- the Q&A list reorders by upvotes
- "you voted B" appears

**Going around the app.** An attendee who writes action records directly to
their PDS, bypassing the app, gains nothing. Such actions are checked by
the same middleware when the appview indexes them. Actions that would be
rejected never count in any view.

**Gallery.** The block gallery switches to live sources in a vivarium test
space, with demo middleware:

- one vote each
- reject empty text
- closes at a given time
- answer with an ephemeral card

## Data

- **`app.gather.block.action`** is a record in the attendee's repo in the
  space. Fields: `card` (a space record reference), `blockId`, `actionId`,
  `value` and `createdAt`. The record key is a client-generated TID
  (`clientId`). The PDS refuses to create an rkey that already exists, so
  retries are idempotent. The record is the attendee's **receipt**. Views
  count only actions the appview accepted.
- **The accepted-actions table** lives in the appview. It's keyed by
  action URI, and stores the middleware result (including ephemeral blocks,
  so a replay returns the same answer). It's filled in two ways:
  - by `submitAction`
  - by ingest re-validation of actions that arrive through
    [`space-sync`](space-sync.md) without having gone through
    `submitAction`
- **Middleware** is ES-module JS, stored as blobs referenced by the card
  (`#moduleRef`: CID, uploader DID, exports).
  - It's called as `handle(action, ctx)`, where `ctx` holds `viewer`,
    `now`, `sources` (resolved for the viewer), and `side` (`client` or
    `server`).
  - It returns either `{accept: true, value?, ephemeral?}` or
    `{accept: false, message}`.
  - A rewrite may change only `value`. The card, block, action id,
    collection and author are fixed.
  - The chain runs built-in validation first, then the card's modules in
    order, and stops at the first rejection.
- **The server decides on its own clock:** `now` is always the appview's
  time, never the action's `createdAt`.
- **Engine:** QuickJS on both sides, and the same **quickjs-ng** build:
  - `rquickjs` on the appview
  - `@jitl/quickjs-ng-*` wasm in a Web Worker on the PWA

  Both are pinned to the same upstream version, and a parity test runs the
  same modules through both. `Date` is replaced, `Math.random` is removed,
  and there's no I/O and no timers. Interrupt and memory limits enforce
  time and memory budgets.
- **Views** are quasi-records that the appview computes from the index plus
  the accepted-actions table.
  - `app.gather.block.view.actionTally{card, blockId}` returns
    `{total, options: [{value, count, percent}], mine?}`.
  - `app.gather.block.view.actionList{card, blockId, sort}` returns
    `{items: [{uri, author, value, score, mine}]}`.
  - `mine` only ever holds the viewer's own data.
  - Each view ships an optimistic reducer, for `block-offline`.
- **XRPC:**
  - `submitAction`, which needs the session cookie and a CSRF token
  - `resolveSources`
  - `subscribeSources`, a WebSocket that pushes new values for the sources
    a client has subscribed to
- **The live `SourceResolver`** for [`ui-blocks`](ui-blocks.md) is built on
  those three endpoints.

## Options considered

The main choices were made in the ui-blocks brainstorm (see its Options
considered): actions proxied through the appview, full middleware, and
isomorphic sandboxed JS. Round 1 of the design review added these:

### Enforcement point

- **Validate at submit and again at ingest; views count only accepted
  actions** (chosen): space writes can't be restricted to our app, so
  ingest is the only enforcement that can't be bypassed.
- Validate at submit only: bypassable by a direct PDS write.

### Idempotency

- **rkey = clientId, plus stored results** (chosen): the PDS enforces
  uniqueness, and replays return the same answer.
- A server-side dedupe table only: races with direct writes.

### Engine parity

- **quickjs-ng on both sides, pinned, with a parity test** (chosen).
- The browser's JS engine on the client: Intl, Date and float drift break
  "isomorphic".

## Out of scope

- The offline queue and optimistic updates
  ([`block-offline`](block-offline.md)).
- Custom blocks and computed bindings
  ([`block-sandbox`](block-sandbox.md)).
- Installing third-party middleware and plugin-defined views
  (`feed-apps`).
- Any specific experience (polls, Q&A). Those are templates over this.

## Open questions

- Blobs are fetchable without auth by anyone who knows the CID. Is that
  acceptable for middleware code, given that organizers shouldn't put
  secrets in modules? If not, the appview has to serve modules itself.
- Reusing a module across cards by different writers means re-uploading
  it, because blobs are stored per uploader. Is that fine for v1?
- Whether ingest re-validation should also delete or flag the rejected
  receipt, or just ignore it.

## Architecture analysis

## Design review

## Test cases

## Review log
