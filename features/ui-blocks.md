---
status: test-cases
impact: cross-cutting
depends-on: [ci-pipeline]
branch:
tests-commit:
---

# UI blocks

## Summary

This is a block-kit style vocabulary of UI building blocks, plus a themed
React renderer that turns a **card** (a tree of blocks bound to data) into
UI. Conference experiences are built from it: chat, polls, Q&A/AMA,
announcements, wifi info and toys.

Every later part of the configurable UI renders through it.

**The split.** This file started as all of "configurable UI blocks". Design
review round 1 split it into six features, and this one is now the **core**:

- the lexicons for cards, blocks and bindings
- the renderer
- the static vocabulary
- the theme contract
- field and list bindings, resolved through a pluggable source resolver
- the surfaces
- the block gallery

Interaction, offline and sandboxed JS moved to sibling features. The
decisions recorded under Options considered still apply across the whole
family.

### Where this sits

| Slug | What it is | Depends on |
|------|-----------|-----------|
| [`attendee-sign-in`](attendee-sign-in.md) | OAuth backend-for-frontend in the appview, with `space:` scopes, plus a test helper | ci-pipeline |
| [`space-sync`](space-sync.md) | The appview's identity and access to spaces: credentials, write notifications, a per-repo index | ci-pipeline, attendee-sign-in |
| **`ui-blocks`** | This feature: lexicons, renderer, vocabulary, theme, bindings, surfaces, gallery | ci-pipeline |
| [`block-actions`](block-actions.md) | Actions, isomorphic wasm middleware, ingest validation, built-in views, live updates | ui-blocks, space-sync, attendee-sign-in |
| [`block-offline`](block-offline.md) | Offline queue, optimistic overlay, replay and reconciliation | block-actions |
| [`block-sandbox`](block-sandbox.md) | Custom JS blocks and computed bindings | block-actions |
| `conference-space` | A conference as a permissioned space: the organizer owns it and attendees are members | (not specced yet) |
| `conference-feed` | The main timeline and scoped sub-feeds (per session or room), ordering, pinning | (not specced yet) |
| `feed-templates`, `block-editor`, `feed-apps` | The three authoring modes | (not specced yet) |
| `announcements`, `wifi-info`, `polls`, `qa`, `session-chat`, `toys` | The experiences | (not specced yet) |

## Experience

**Attendees** see cards made of blocks, on four surfaces:

- in the feed
- as compact home or pinned cards
- in sheets opened from a button
- as ephemeral responses

Every card uses the organizer's white-label theme, through semantic
variants only.

**Bound data.** Blocks can show data bound from sources: a field of a
record, or a list that repeats a block template over a collection. While a
source loads, the block shows a skeleton. If it's missing, forbidden or
failing, the block shows a quiet "unavailable", never saying why, and the
rest of the card still renders.

**Interactive blocks.** Buttons, selects and text inputs render, validate
their own local input (e.g. required, max length), and hand an **action
intent** to whoever is hosting the card. In this feature, the gallery shows
the intent. [`block-actions`](block-actions.md) sends intents to the
appview.

**Old clients.** An older client that doesn't know a newer block type skips
it silently.

**The block gallery** (`/dev/blocks`) renders sample cards covering every
vocabulary block and every surface. Their sources are fixtures, and the
gallery shows each action intent a card emits. It's the reference for
authors, and the target for Playwright and the demo video. Once
`block-actions` lands, it switches to live sources.

### Vocabulary (v1)

- **Layout and text:** section, header, divider, context (small print), rich
  text, image, columns/stack.
- **Inputs:** button (it can open a sheet), button group, text input,
  single select, multi select, submit.
- **Data display:** list (repeats over a collection), progress/result bar,
  stat/number, badge.
- **Conference-aware:** person (a DID shown as avatar and name), session
  reference, room/location, time/countdown, copyable value (e.g. a wifi
  password), QR code.
- **Custom and canvas:** reserved in the lexicon. Rendered by
  [`block-sandbox`](block-sandbox.md): `custom` runs a wasm module that
  returns blocks, and `canvas` draws commands and reports taps. Until then,
  the renderer treats both as unknown blocks and skips them.
- **Declarative animation:** an `animate` property (from, to, duration,
  easing, repeat) is reserved in the lexicon, and `block-sandbox` runs
  it.

Blocks take semantic variants only: primary/danger buttons, emphasis, and
info/warning/success tones.

## Data

- **Cards** are records in a space. The lexicons are defined here. Who
  writes cards is up to later features (feed, templates, editor, apps).
- **Rich text** is plain text plus atproto richtext facets, the same model
  as `app.bsky` posts.
- **Source references** come in two kinds:
  - a record, or a collection in the card's space
  - a view: a quasi-record computed by the appview (the built-in views
    arrive in `block-actions`)

  The renderer resolves them through a **source resolver** interface. This
  feature ships a fixture resolver, and `block-actions` ships the live
  appview one.
- **Privacy model** (applies to the whole family): attendees hold only a
  `read_self` space grant, so they can read their own records and nothing
  else of anyone's. Only our appview, which has its own did:web and client
  attestation, and is the only client on the space's app allow-list, gets
  space credentials that read every repo. Other members' data reaches
  attendees only as aggregates or views the appview chooses to serve. See
  [`space-sync`](space-sync.md).

## Options considered

### Splitting the feature

- **Split into a family, starting with ui-blocks** (chosen): the block
  vocabulary, data format and interaction model are the hardest decisions
  to change later, and everything renders through them.
- **One big "configurable UI" feature**: too large to spec, test or review
  as a unit.
- **Start with conference-space**: also foundational, but ui-blocks can be
  proven in a test space first.
- **Round 1 of the design review split ui-blocks itself into six**
  (chosen): attendee-sign-in, space-sync, ui-blocks (core), block-actions,
  block-offline and block-sandbox. The alternatives were three pieces
  (sign-in, sync, and everything else) or one. Six keeps each piece
  testable and reviewable, and lets the core land right after ci-pipeline.

### Who authors experiences

- **All three: templates, a full block editor, and third-party apps**
  (chosen by the user). Each is its own later feature.
- Offered separately: templates only (recommended at the time), organizer
  block editor only, or third-party apps only.

### Feed shape (for `conference-feed`)

- **One conference timeline with scoped sub-feeds** (chosen), with pinning
  as a feature: either explicit pins, or items kept at the top of the feed.
- A single timeline for everything: drowns in a busy conference.
- A dashboard of pinned cards: covered by pinning, not a separate model.

### Where items live

- **A conference permissioned space** (chosen): attendees-only reads,
  organizer and apps in the writer set, attendee contributions under the
  space's policy.
- Public records in the organizer's repo: would make chat and votes public.
- Backend database: loses the atproto story.

### Live data

- **Bindings to data sources** (chosen): sources are atproto records, or
  quasi-records computed by the appview (e.g. counts).
- Snapshots re-published on change (Slack style): a write per update, and
  all live logic stays with the owner.
- Snapshots plus a few live primitives: a halfway house.

### How expressive bindings are

- **Built-in field references and repeating over collections, plus
  sandboxed JS for anything more** (chosen).
- Field references only: variable-length lists would need rewrites.
- A bespoke expression language: a language to design and version.
  Sandboxed JS covers this need instead.

### Viewer-relative data

- **Viewer-scoped sources** (chosen): "you voted B", enforced as the viewer.
- The same view for everyone: no per-person state.

### Interactions

- **The client sends an action, proxied through our appview, where
  middleware sees it on the way in, then it's written to the space**
  (chosen).
- An HTTP callback to each owner: not recorded in atproto, and every owner
  needs a server.
- Both paths, chosen per action: two paths to build and secure.

### Middleware power

- **Full middleware** (chosen): accept, reject with a message, rewrite, and
  ephemeral blocks.
- Validate/reject, then react: less power.
- Observe only: can't stop bad actions.

### Where middleware runs, and offline behavior

- **Isomorphic sandboxed JS on both the PWA and the appview; offline
  actions are queued, and the server's result wins** (chosen).
- Fail visibly with retry, nothing queued: simpler, but a worse experience
  on conference wifi.
- Middleware hosting in this feature: this feature ships the runtime on
  both sides and the built-in middleware. Installing templates and
  third-party middleware belongs to `feed-templates` and `feed-apps`.

### Sandboxed JS scope

- **Both custom blocks and computed bindings, in v1** (chosen).
- Custom blocks only, in a later feature: the recommendation at the time.
- Computed bindings only.

### Sandbox reach

- **Only its bindings and actions** (chosen): no network, no storage,
  budgeted.
- Plus fetch to allowlisted hosts: weakens privacy.

### Rich text

- **atproto richtext facets** (chosen): consistent with the ecosystem, and
  mentions resolve to DIDs.
- A Markdown subset: ambiguous parsing, and no native DID mentions.

### Vocabulary size

- **Layout + text, inputs, data display and conference-aware blocks, all in
  v1** (chosen).

### Unknown block types

- **Skip silently** (chosen).
- Required fallback text with an "update the app" notice: the
  recommendation at the time.
- Pin a vocabulary version: refuses whole items.

### Styling

- **Semantic variants only** (chosen): keeps every card on-brand under
  white-labeling.
- Semantic plus limited overrides.
- Free styling: breaks white-labeling.

### Surfaces

- **Feed items, ephemeral responses, modals/sheets, and home/pinned cards**
  (chosen): all four in v1.

### Bad or slow sources

- **Per-block placeholder states** (chosen): a skeleton while loading, and
  "unavailable" (never why) when missing or forbidden. Crashing or overtime
  sandbox code gets the same treatment.
- Hide the block: authors can't tell why things vanish.
- Hide the whole item: one bad binding hides everything.

### Proving it done

- **A block gallery page** (chosen): live sources in a vivarium test space,
  plus demo middleware.
- Component tests only: nothing visible until the feed ships.

### Privacy inside a space (design review round 1)

- **Members get `read_self`; only the appview reads everything and serves
  aggregates** (chosen by the user): the space's app allow-list admits only
  our appview's attested client, so only it can obtain space credentials.
- Accept that all members can read everything: rules out secret ballots and
  anonymous Q&A.
- Actions written into an appview-owned repo: the attendee would no longer
  author their own record.
- A separate private space per sensitive interaction: many more spaces to
  manage.

### What custom blocks can see (design review round 1)

- **Declared, non-viewer-scoped sources only** (chosen): the card must pass
  sources explicitly, and never `mine` or other viewer-scoped fields. The
  WebRTC leak is an accepted risk.
- Anything, gated by a trust flag on the card's author.

## Out of scope

- Sending actions, middleware, views and live updates
  ([`block-actions`](block-actions.md)).
- The offline queue and optimistic updates
  ([`block-offline`](block-offline.md)).
- Custom JS blocks and computed bindings
  ([`block-sandbox`](block-sandbox.md)).
- Sign-in, and the appview's access to spaces.
- The feed: timeline, sub-feeds, ordering, pinning (`conference-feed`).
- Creating and administering conference spaces (`conference-space`).
- Organizer block editing, templates, and third-party apps.
- Any specific experience.
- Per-organizer theme configuration (a future `theming` feature). This
  feature ships the token contract and one default theme.

## Open questions

None for this feature. The earlier questions moved with their topics:

- the JS engine goes to `block-actions` and `block-sandbox`
- quasi-record addressing goes to `block-actions`
- notifying people about offline rejections goes to `block-offline`
- the space dependency is answered by the split

## Architecture analysis

**Existing code touched:** none: no code yet. This feature builds on the
skeleton that [`ci-pipeline`](ci-pipeline.md) will create (`ready`, not
built):

- `crates/server`, which becomes the appview
- `web/`, the React + Vite PWA
- the vitest integration harness (vivarium plus the server)
- the Playwright config

**Features affected:**

- [`ci-pipeline`](ci-pipeline.md) (`ready`). Its approved design still
  holds, but ui-blocks leans on it in four ways:
  - **Playwright blocks service workers globally** (`serviceWorkers:
    'block'`), yet the offline queue tests need the offline behavior. Either
    the queue works without a service worker (in-page IndexedDB plus
    `context.setOffline`), or those specs opt back into service workers.
    The design has to pick.
  - **An embedded JS engine** in `crates/server`, and possibly a wasm build
    of it for the PWA, adds build time and dependencies to the `build` and
    `test` jobs and to the Rust cache.
  - **The placeholder `/health` page** becomes one route among several (the
    gallery), which brings client-side routing into `web/`.
  - **Integration tests need an authenticated attendee** writing to a
    space. That's the first use of vivarium OAuth and permissioned spaces,
    through `viv`.
- **Planned features** (`conference-space`, `conference-feed`,
  `feed-templates`, `block-editor`, `feed-apps` and the experiences) aren't
  specced yet. Every one of them consumes the surfaces below, so changing
  them later ripples into all of them.

**New shared surfaces:**

- **Lexicons:**
  - block documents (the vocabulary)
  - bindings and source references
  - action records
  - the middleware request and response shapes
  - the quasi-record addressing scheme
- **The React renderer:** a block-document component, one component per
  block type, the four surfaces (feed item, ephemeral, sheet, compact
  card), and placeholder states.
- **A theme token contract:** the semantic variants map to white-label
  tokens. No theming system exists yet. The design decides whether
  ui-blocks defines the token set or a separate `theming` feature comes
  first. The earlier "Program" design direction (see project memory) could
  seed it.
- **The sandbox runtime:** one JS module format for custom blocks, computed
  bindings and middleware. It runs:
  - in the PWA, in an isolated worker or frame
  - in the Rust appview, in an embedded engine (QuickJS via `rquickjs`,
    Boa, or V8 via `deno_core`)

  It needs identical semantics in both places, with time and memory
  budgets.
- **Appview APIs:**
  - submit an action, which runs middleware and writes the action to the
    space
  - resolve a source, viewer-scoped
  - subscribe to source changes
- **The offline action queue and reconciliation:** the local result
  against the server's result.
- **The block gallery route,** plus the test-space fixtures that later
  features' tests will reuse.

**Findings that affect the design:**

- **Writing to a space as an attendee needs that attendee's OAuth session**
  with a `space:` scope (vivarium enforces it). The appview has to be the
  OAuth client (a backend-for-frontend holding DPoP-bound tokens) to write
  actions on the attendee's behalf. No sign-in feature exists. The design
  must decide whether sign-in is part of ui-blocks, or a new
  `attendee-sign-in` feature that ui-blocks depends on.
- **Vivarium spaces must be created explicitly**
  (`com.atproto.simplespace.createSpace`), and it tracks a proposal's
  lexicons, so expect churn. The gallery's test space and the
  `conference-space` feature both depend on this. Keeping ui-blocks'
  records independent of the space's membership model avoids a hard
  dependency on `conference-space`.
- **Whose repo action records go in:** space writes go through the writer's
  own space credentials, so either the attendee's repo or the appview's.
  That decides whether the action authored by the attendee is literally
  true, and how the space's writer set must be configured.
- **"Isomorphic" middleware** means one JS module that behaves the same
  under the browser's engine and under an embedded Rust engine. Two
  engines risk subtle differences (Date, Intl, float formatting, missing
  APIs). One engine on both sides (e.g. QuickJS natively and as wasm)
  removes that risk, at some cost in PWA performance.
- **Quasi-records need a live change feed.** Either the appview pushes
  source updates (WebSocket/SSE), or clients re-resolve on space write
  notifications. That's a design choice with cost and latency trade-offs.

**Verdict:** cross-cutting. ui-blocks introduces the lexicons, the
renderer, the theme contract, the sandbox runtime and the appview APIs that
every planned configurable-UI feature is built on. It also establishes
auth, space writes and client-side routing for the first time. It leans on
`ci-pipeline`'s test harness in ways that its design needs to accommodate:
service workers in Playwright, and the build cost of an embedded JS engine.

**Addendum (design review round 1):** the critique found that vivarium's
space write check never checks which app is writing, that a space
credential reads every repo in the space, and that sources spanning members
need an appview-side indexer. Those findings drove the split into six
features, and moved the auth, sync and action concerns out of this one.

## Design review

NSIDs use `app.gather.*` as a **placeholder** namespace until the user
picks the real one.

### Approach

A **card** is a record holding three things:

- a tree of blocks
- named source references
- an optional list of middleware module references (reserved here, used
  by `block-actions`)

`<BlockCard>` renders a card for a surface:

1. It walks the block tree.
2. It resolves each binding against sources obtained from a
   `SourceResolver` found through React context.
3. It renders each known block type through a registry, and skips unknown
   types (they render `null`).

Bindings are JSON pointers into a source's value. A `list` block repeats
its template for each element of a bound array, with the element exposed
as the reserved source `$item`. Keys come from a declared path, so
reordering doesn't remount blocks.

`SourceResolver` is the seam between this feature and the rest of the
family:

```ts
interface SourceResolver {
  // one observable per source; the renderer subscribes while mounted
  watch(card: CardRef, name: string, ref: SourceRef):
    Observable<{ state: 'loading' } | { state: 'ready', value: unknown } | { state: 'unavailable' }>
}
```

The gallery and unit tests use `FixtureResolver`, which maps a source name
to a value or a state, including delayed and failing sources. The live
resolver comes with `block-actions`. Anything forbidden, missing or
erroring maps to `unavailable`, and the reason is never surfaced.

**Interactive blocks** hold their own local input state, run declared
local validation (required, min/max length, max selections), and on submit
call `onAction(intent)`, which they get from context:

```ts
type ActionIntent = { card: CardRef, blockId: string, actionId: string, value: unknown }
```

The gallery's host logs intents on screen. Sending them is `block-actions`'
job.

**Surfaces** are the same tree with a layout variant:

- `feed`: full width
- `compact`: truncated to the first N blocks, with media and inputs hidden
  except for the primary button
- `sheet`: a modal bottom sheet, opened by `button.opens`
- `ephemeral`: an inline, dismissible card with an "only you" marker

**Theme:** `web/src/theme/tokens.css` defines the semantic custom
properties. Blocks use only those tokens. The default theme is the
"Program"/"Signal" direction.

**Conference-aware blocks without their backing features:**

- **person** takes a DID and resolves its display name and avatar through
  the resolver (a `profile` source kind). The fixture resolver answers in
  tests.
- **session reference** and **room** take inline title, time and room data,
  plus an optional URI for a future schedule feature to deep-link.
- **time/countdown** ticks on the client clock and switches to "now" or
  "ended" labels.
- **QR code** is generated on the client.
- **copyable value** uses the clipboard API, and shows "Copied".

All times use a 24-hour clock.

### Components

```
lexicons/app/gather/block/defs.json    block union, binding, sourceRef, moduleRef, variants
lexicons/app/gather/block/card.json    the card record
crates/blocks/                         serde types + lexicon validation for cards (Rust),
                                       used later by the appview; no runtime yet
web/src/blocks/
  BlockCard.tsx                        entry: <BlockCard card surface />
  registry.ts                          type -> component; unknown -> null
  blocks/*.tsx                         one component per vocabulary block
  bindings.ts                          JSON pointer, $item scope, list keys
  SourceResolver.ts                    interface + FixtureResolver
  ActionContext.ts                     onAction(intent) context
  surfaces/*.tsx                       feed, compact, sheet, ephemeral wrappers
web/src/theme/tokens.css               semantic token contract + default theme
web/src/routes/dev/blocks.tsx          gallery at /dev/blocks (react-router)
web/src/blocks/gallery/*.card.json     sample cards (valid against the lexicon) + fixture sources
```

### Data (lexicons)

- **`app.gather.block.defs`:**
  - The **block union**, with each v1 type as a def: `section`, `header`,
    `divider`, `context`, `richText`, `image`, `stack`, `columns`,
    `button`, `buttonGroup`, `textInput`, `select`, `submit`, `list`,
    `progress`, `stat`, `badge`, `person`, `sessionRef`, `room`, `time`,
    `copyable`, `qr`, `custom`, `canvas` (the last two reserved for
    `block-sandbox`).
  - Every block has an optional `id`, which interactive blocks require.
  - Every bindable property is typed as a literal, a `#binding`
    (`{source, path}`), or a `#computed` (reserved for `block-sandbox`).
  - `#sourceRef` is a union of `#recordSource` (a space record reference),
    `#collectionSource` (an nsid plus an optional filter),
    `#profileSource` (a DID) and `#viewSource` (an nsid plus params).
  - `#spaceRecordRef` holds a space URI, an author DID, a collection, an
    rkey and a CID. Space record URIs have more segments than a standard
    at-uri, so `strongRef` doesn't fit.
  - `#moduleRef` (reserved) holds these fields:
    - `wasm`: a blob CID plus the uploader's DID
    - `script`: optional, for modules built on the shared JS runtime
    - `exports`
  - `#animate` (reserved): a target property, `from`, `to`, `duration`,
    `easing` and `repeat`.
- **`app.gather.block.card`** (a record in a space) has these fields:
  `blocks`, `sources` (a map from name to `#sourceRef`), `middleware`
  (a list of `#moduleRef`, reserved), `fallbackText` (optional), and
  `createdAt`.
- **Unknown fields and types:** open unions, so older clients skip newer
  block types.
- **Who writes and reads:** nothing is written by this feature. The gallery
  cards are static JSON validated against the lexicon.

### Interfaces other features will use

- `<BlockCard card surface />`, `SourceResolver`, `ActionContext` /
  `ActionIntent`, and the block registry. Adding a block type means a
  lexicon def, a component, and a gallery card.
- The lexicons, and their generated TS and Rust types.
- `tokens.css`, the semantic token contract.
- The gallery's fixture format, which later features reuse for their own
  cards.

### Impact on existing features

- **[`ci-pipeline`](ci-pipeline.md)** (`ready`) keeps its approved design,
  with two small additions:
  - `react-router` for `/dev/blocks`, with the placeholder health page
    staying at `/`
  - lexicon codegen (`@atproto/lex-cli`) wired into `check:build`, so
    generated types never drift
- **The five new sibling features** build on these interfaces.

### Alternatives

- **Resolve sources straight from the space in this feature:** needs
  sign-in and space-sync first, which would delay the core. The resolver
  seam lets the core land now.
- **A standard `strongRef` for records:** doesn't fit space URIs.
- **Snapshots with no bindings in the core,** adding bindings later: the
  user chose bindings, and the lexicon shape has to include them from day
  one.

### Risks

- **Lexicon churn:** the lexicon is designed before its main consumers
  (actions, feed, editor) exist. The reserved `#computed`, `#moduleRef`,
  `middleware` and `#viewSource` shapes reduce that, but may still change
  before those features ship.
- **The namespace is a placeholder** and gets baked into records.
- **Vocabulary size:** 24 block types is a lot of components and gallery
  cards for one feature. It's still static UI, so it's low risk.

### Round 2

**User feedback:**

- Keep `app.gather.*` as a placeholder until a real domain is bought.
- The PWA sandbox should use a wasm interpreter, and then the question was
  raised: why not let a custom block just run a wasm module?

**Decision:** wasm is the only sandbox unit for middleware, computed
bindings and custom blocks.

- **Hosts:** `wasmtime` on the appview, and the browser's native
  WebAssembly in a Worker on the PWA.
- **ABI:** an Extism-style ABI, JSON in and out: `handle`, `compute`,
  `render`, `event`, `tick`.
- **Budgets:** fuel is instrumented at load time, so budgets behave the
  same on both sides. Memory is capped. A stuck Worker can be killed.
- **JS authoring:** a JS module is a script plus a shared, prebuilt QuickJS
  runtime compiled to wasm, cached by CID.
- **What custom blocks draw:** they return standard blocks, plus a `canvas`
  block with draw commands and tap coordinates.
- **Animation:** `tick(dt)` at up to 30fps under a per-frame budget, plus a
  declarative `animate` that the host runs without calling the module every
  frame.

Both sandbox risks from round 1 go away:

- wasm has no network imports, so there's no WebRTC leak
- loops are fuel-bounded and run in a killable Worker, so nothing freezes
  the page

**What changed:**

- **Here:** `canvas`, the `#moduleRef` shape (wasm plus an optional script)
  and `#animate` are reserved in the lexicon.
- **`block-actions`:** wasm instead of QuickJS.
- **`block-sandbox`:** rewritten around wasm (no iframe), with the canvas
  block and animation.

**Round 2 outcome:** the user approved the core design as written above.

### Round 1

The first draft covered the whole family in one feature. A subagent
critique checked it against vivarium's space implementation and found:

- **Critical:**
  - Space writes don't check which app is writing, so attendees can skip
    the middleware.
  - A space credential reads every repo, so "viewer-scoped" wasn't access
    control.
  - Collection sources and views need an unmentioned appview indexer:
    credentials, delegation, write notifications, and per-repo cursors.
- **High:**
  - A middleware rewrite could put words in the attendee's mouth.
  - Custom blocks can leak data through actions and WebRTC.
  - The iframe watchdog can't recover a frozen frame in Chrome either.
  - Idempotency had no mechanism.
  - Offline `now` was ambiguous.
- **Medium:**
  - Offline scope was overstated.
  - Blob fetches are unauthenticated and blobs are stored per uploader.
  - quickjs-ng vs QuickJS parity.
  - Space URIs don't fit `strongRef`.
  - Sign-in had no test helper.

**User decisions:**

- Split into six features, as proposed.
- Privacy: members hold only `read_self`. Our appview, with its own did:web,
  is the only reader of all repos, and exposes aggregates over XRPC.
  Verified against vivarium:
  - user tokens can read only their own repo
  - `getSpaceCredential` enforces the space's app allow-list through
    client attestation
- Custom blocks see declared, non-viewer-scoped sources only.

**What changed:**

- This file was narrowed to the core.
- Five sibling feature files were written, carrying the rest of the
  original brainstorm and the critique's fixes.
- The core design above replaces the whole-family draft, which is kept
  below for reference.

<details><summary>Round 1 draft (whole family, superseded)</summary>


NSIDs below use `app.gather.*` as a **placeholder** namespace until the user
picks the real one.

### Approach

A **card** is the unit of configurable UI. It holds three things:

- a tree of blocks
- named data sources the blocks bind to
- optional middleware modules

Cards are records in a permissioned space. Later features embed or
reference them: feed items, pinned cards, template output.

The React renderer turns a card into UI for a surface: feed item, compact
card, sheet or ephemeral response. It resolves the card's sources as the
viewer, keeps them live, and routes interactions into actions.

**Two sandboxes, split by job:**

- **Middleware and computed bindings** are pure, deterministic functions,
  so they must behave identically on the PWA and the appview. They run in
  **QuickJS** everywhere:
  - `rquickjs` embedded in the Rust appview
  - `quickjs-emscripten` (wasm) in a Web Worker on the PWA

  One engine means identical semantics. Hosts inject `now`, the viewer and
  the resolved inputs. `Date.now`, `Math.random`, timers and I/O are
  removed, and interrupt and memory limits enforce budgets.
- **Custom blocks** draw UI, so they only ever run on the client, never on
  the server. They run in a `sandbox="allow-scripts"` iframe on an opaque
  origin, with a CSP of `default-src 'none'`. They talk only over
  `postMessage`: bindings in, actions and a size hint out. A heartbeat
  watchdog tears down a frame that stops responding.

Module code is stored as **blobs referenced by CID**, so the client and
server provably run the same bytes.

**Actions** flow through the appview:

1. The client calls `submitAction`.
2. The appview loads the card and runs the middleware chain: built-in
   validation, then the card's modules in order.
3. If the chain accepts, the appview writes the (possibly rewritten) action
   record into the space as the attendee, using the attendee's OAuth
   session (the appview is a backend-for-frontend OAuth client).
4. It returns `{accepted | rejected, message?, ephemeral?, uri?}`.

Every action carries a client-generated `clientId`, so resubmitting the
same action is idempotent.

**Offline:**

- The same middleware chain runs in the PWA's worker, and its result is
  shown immediately.
- The action goes into an **IndexedDB queue** in the page. No service
  worker is used, so it's compatible with `ci-pipeline`'s
  `serviceWorkers: 'block'` Playwright setting.
- Pending actions are overlaid on the cached sources through each view's
  **optimistic reducer**, e.g. tally +1.
- On reconnect, the queue replays in order. The server's result replaces
  the optimistic one. If they differ, the card reverts and shows the
  server's message.

**Sources** come in two kinds:

- **Record sources** are a record or a collection in the space.
- **View sources** are quasi-records: named views the appview computes,
  such as a tally of actions on this card.

Both resolve through the appview as the viewer. v1 ships two built-in
views, `actionTally` and `actionList`, which cover polls, votes and Q&A.
Plugin-defined views come with `feed-apps`.

Clients stay live over one **WebSocket per session**. The appview follows
the space's write notifications and `listRepoOps`, recomputes the affected
views, and pushes new values for the sources each client has subscribed to.

### Components

```
lexicons/app/gather/block/*.json       lexicons (below); TS types via @atproto/lex-cli
crates/blocks/                         shared Rust crate
  src/lexicon.rs                       serde types + validation of block documents
  src/sandbox.rs                       rquickjs host: budgets, deterministic globals
  src/middleware.rs                    chain runner, built-in validation middleware
  src/views/{tally,list}.rs            built-in views + their optimistic-reducer specs
crates/server/                         (from ci-pipeline) becomes the appview
  src/xrpc/block.rs                    submitAction, resolveSources, subscribeSources (WS)
  src/space.rs                         space reads/writes, write-notification follower
  src/oauth.rs                         BFF session (from attendee-sign-in, see Impact)
web/src/blocks/
  BlockCard.tsx                        <BlockCard card surface> entry point
  registry.ts                          type -> component; unknown types -> null
  blocks/*.tsx                         one component per block type
  bindings.ts                          JSON-pointer resolution, list repetition, $item scope
  sources.ts                           resolve + WS subscription + cache
  sandbox/worker.ts                    quickjs-emscripten worker (middleware, computed)
  sandbox/CustomFrame.tsx              iframe host for custom blocks + watchdog
  actions/queue.ts                     IndexedDB queue, replay, reconcile
  theme/tokens.css                     semantic token contract + default theme
web/src/routes/dev/blocks.tsx          block gallery (/dev/blocks)
tests/support/blocks/                  seed space, gallery cards, demo middleware modules
```

### Data (lexicons)

- **`app.gather.block.defs`** defines the block union: every v1 block type
  from the vocabulary, plus `custom`. It also defines `binding`, `computed`,
  `sourceRef`, `moduleRef` and `variant`.
  - Every block has an optional `id`. Interactive blocks require one.
  - Any bindable property accepts either a literal, a
    `binding {source, path}` (JSON pointer), or a
    `computed {module, export, inputs[]}`.
  - A `list` block has `items` (a binding to an array), a `key` path and a
    `template` of blocks, which reach the current element through the
    reserved source `$item`.
  - A `button` may carry `opens`, a nested block document that's shown as
    a sheet.
- **`app.gather.block.card`** (a record in the space) has these fields:
  - `blocks`
  - `sources` (a map from name to `sourceRef`)
  - `middleware` (a list of `moduleRef`s)
  - `fallbackText` (optional; unused by v1 clients, kept for notifications
    and search)
  - `createdAt`
- **`sourceRef`** is one of:
  - `{record: at-uri}`
  - `{collection: nsid, filter?}`, scoped to the card's space
  - `{view: nsid, params}`
- **`moduleRef`** is a blob with the `application/javascript` MIME type,
  referenced by CID, plus its `exports`.
- **`app.gather.block.action`** (a record in the space, in the attendee's
  space repo) has these fields: `card` (strongRef), `blockId`, `actionId`,
  `value` (unknown, validated by middleware), `clientId`, `createdAt`.
- **Middleware contract:**
  - It receives `handle(action, ctx)`, where `ctx` holds `viewer` (DID),
    `now`, `sources` (the card's sources resolved for the viewer) and
    `side` (`client` or `server`).
  - It returns either `{accept: true, action?, ephemeral?}` or
    `{accept: false, message}`.
  - The chain stops at the first rejection, and a rewritten action carries
    on down the chain.
- **Views:**
  - `app.gather.block.view.actionTally{card, blockId}` returns
    `{total, options: [{value, count, percent}], mine?}`.
  - `app.gather.block.view.actionList{card, blockId, sort}` returns
    `{items: [{uri, author, value, score, mine}]}`.
  - Each view has a client-side optimistic reducer, fixed in code for the
    built-ins.

**Who writes and reads:**

- **Cards and module blobs** are written by whoever holds writer rights in
  the space. In the gallery, that's the test organizer. Later, it's
  feed/templates/apps.
- **Actions** are written by attendees, through the appview.
- **Reads** are made by the appview on the viewer's behalf. Views run
  server-side over actions the appview can read, and never return more
  about other people than their fields expose. `mine` only ever carries the
  viewer's own data.

### Interfaces other features will use

- **`<BlockCard card={uri|inline} surface="feed|compact|sheet|ephemeral" />`:**
  the only way other features render blocks.
- **The block registry**, for adding a block type. Adding one means a
  lexicon change, a component, and a gallery card.
- **XRPC:**
  - `app.gather.block.submitAction`
  - `app.gather.block.resolveSources`
  - `app.gather.block.subscribeSources` (WebSocket)
- **The sandbox module format** (ES module exports) and the middleware and
  computed-binding signatures. Templates and apps write to this.
- **The view registry in `crates/blocks`,** for adding a view and its
  optimistic reducer. `feed-apps` will extend it with plugin-defined views.
- **`theme/tokens.css`:** semantic tokens (`--g-color-{bg,surface,text,muted,
  border,primary,on-primary,danger,info,warning,success}`,
  `--g-font-{display,body,mono}`, `--g-radius-*`, `--g-space-*`). The
  default theme is the "Program"/"Signal" direction (Archivo Narrow,
  Archivo, JetBrains Mono). Configuring themes per organizer belongs to a
  future `theming` feature.
- **`tests/support/blocks`:** creates a space, seeds cards, and signs in
  test attendees. Later features' tests reuse it.

### Impact on existing features

- **[`ci-pipeline`](ci-pipeline.md)** (`ready`) doesn't need any changes to
  its approved design:
  - The offline queue lives in the page, so Playwright keeps blocking
    service workers. Offline tests use `context.setOffline`.
  - `rquickjs` compiles QuickJS from C, which needs the `cc` toolchain.
    That's already on GitHub runners, and `rust-cache` covers it.
  - `quickjs-emscripten` is a prebuilt wasm npm package, with no emsdk in
    CI.
  - The placeholder `/health` page moves to `/`, and `react-router` is
    added for `/dev/blocks`.
- **Attendee sign-in is a new dependency.** Writing actions as an attendee
  needs an OAuth session with a `space:` scope, held by the appview. That's
  a foundational feature of its own (`attendee-sign-in`), and ui-blocks
  depends on it, with the gallery's tests signing in through vivarium's
  OAuth. The alternative is folding a minimal sign-in into ui-blocks (see
  Alternatives).
- **No new dependency on `conference-space`.** ui-blocks only needs *a*
  space whose member policy lets its test attendees write. The test support
  code creates one directly (`simplespace.createSpace`, member-list
  policy). `conference-space` later decides how real conferences create
  and administer theirs.

### Alternatives

- **Browser JS for client middleware, QuickJS on the server:** faster on
  the PWA, but two engines drift (Intl, Date, float printing), which breaks
  "isomorphic".
- **V8 via `deno_core` on the server:** a faster engine, but a very large
  dependency and still a different engine from the client's.
- **Custom blocks in QuickJS with a virtual UI tree:** would make them
  isomorphic, but no one needs a server-rendered toy, and it means
  inventing a UI protocol.
- **A service worker queue with Background Sync:** sends actions even with
  the app closed, but clashes with `ci-pipeline`'s Playwright setting,
  isn't supported in Safari, and is harder to test. It can be added later.
- **Clients re-resolving on space write notifications themselves:** no
  appview socket, but every client would need space sync access, and views
  would have to be computed on the client.
- **Minimal sign-in folded into ui-blocks:** one feature fewer, but auth is
  foundational and every feature reuses it. Its scope, session and token
  storage deserve their own spec.
- **Modules inline as strings in the card record:** simpler, but records
  get bloated, and there's no content-addressed identity shared across
  cards.

### Risks

- **QuickJS in wasm** costs PWA memory and speed. Budgets have to be tuned
  so phones don't jank. Middleware and computed bindings are small, so
  this should be acceptable, but it hasn't been measured.
- **Iframe sandbox limits:** a same-process frame can still hog the main
  thread on Safari. The watchdog can only recover after the fact.
- **Proposal churn:** spaces follow proposal 0016, so vivarium's lexicons
  may change under us. The space client is isolated in `space.rs`.
- **Deterministic sources:** optimistic reducers exist only for the
  built-in views. A card bound to a record source shows no optimistic
  change offline, only after sync.
- **Scope:** this is a large feature. Every vocabulary block, both
  sandboxes, offline behavior, live views and four surfaces make for a big
  test plan and a long build.
- **The namespace is a placeholder** until chosen, and it will be baked
  into every record.


</details>

## Test cases

## Review log
