---
status: design-review
impact: cross-cutting
depends-on: [ci-pipeline]
branch:
tests-commit:
---

# UI blocks

## Summary

This is a block-kit style vocabulary of UI building blocks, plus a themed
React renderer. Conference experiences are built from it: chat, polls,
Q&A/AMA, announcements, wifi info and toys.

Blocks can bind to live data from the conference space. Attendee
interactions become actions that pass through middleware. That middleware
is sandboxed JS that runs both on the PWA and on our appview, so
interactions work offline.

Every later part of the configurable UI builds on this feature: the
conference feed, templates, the block editor, third-party feed apps and
each experience.

### Where this sits

"Configurable UI" was split into a family of features. This one comes
first, because everything else renders through it:

| Slug | What it is |
|------|-----------|
| `conference-space` | A conference as a permissioned space: the organizer owns it and attendees are members |
| **`ui-blocks`** | This feature: the vocabulary, renderer, bindings, actions and middleware contract |
| `conference-feed` | The main timeline and scoped sub-feeds (per session or room), ordering, pinning, live updates |
| `feed-templates` | App-shipped experience templates that organizers configure |
| `block-editor` | Organizers compose custom cards from blocks |
| `feed-apps` | Third-party apps post blocks and supply middleware: install, trust, auth |
| `announcements`, `wifi-info`, `polls`, `qa`, `session-chat`, `toys` | The experiences, built on the above |

Organizers get all three authoring modes: templates, a full block editor,
and third-party apps.

## Experience

**Attendees** see cards made of blocks: in the feed, as pinned or home
cards, in sheets opened from a button, and as ephemeral responses to their
own actions. Every card uses the organizer's white-label theme.

**Reading live data.** A block can show live data. Examples:

- the poll bar shows current percentages
- the Q&A list reorders as upvotes arrive
- the poll says "you voted B"

Data a viewer isn't allowed to read never appears. While data loads, the
block shows a skeleton. If the data is missing, forbidden or failing, the
block shows a quiet "unavailable" without saying why, and the rest of the
card still renders.

**Acting.** An attendee taps a button, picks an option, or types and
submits. Middleware then does one of these:

- accepts it straight away (optimistically, on the device)
- rejects it with a message ("Voting has closed")
- shows an ephemeral response only they see ("Thanks! Here's the result")

**Offline.** With no connection, the action still runs through the local
middleware and the card updates optimistically. The action is queued and
sent when the connection returns. If the server then disagrees (say the
poll closed in the meantime), the card reverts and shows the server's
message.

**Custom blocks.** A custom block, such as a toy, renders its own UI in an
isolated frame. It sees only the data it's bound to, and it can only act
through the same action path.

**Old clients.** An older app that doesn't know a newer block type skips
it silently.

**Organizers and developers** get a block gallery page. It renders sample
cards covering every block type and every surface, with live bindings to
records in a test space and demo middleware. It's the reference for
authors, and the target for the Playwright tests and the demo video.

### Vocabulary (v1)

- **Layout and text:** section, header, divider, context (small print), rich
  text, image, columns/stack.
- **Inputs:** button, button group, text input, single select, multi select,
  submit.
- **Data display:** list (repeats a block template over a collection),
  progress/result bar, stat/number, badge.
- **Conference-aware:** person (a DID shown as avatar and name), session
  reference, room/location, time/countdown, copyable value (e.g. a wifi
  password), QR code.
- **Custom:** a sandboxed JS block.

Blocks take semantic variants only: primary/danger buttons, emphasis, and
info/warning/success tones. The theme decides colors and fonts.

## Data

- **Block documents** are the blocks that make up a card. They're records
  in the conference space, whose shape is defined by this feature's
  lexicons. Who writes them is decided by later features (feed, templates,
  editor, apps).
- **Rich text** is plain text plus atproto richtext facets, the same model
  as `app.bsky` posts.
- **Data sources** come in two kinds:
  - atproto records in the conference space
  - **quasi-records**: record-shaped views that the appview computes, such
    as vote tallies
- **Bindings:** a block property can be bound to a field of a source, and
  a list block can repeat over a collection. Sources resolve as the viewer,
  so viewer-scoped values such as `myVote` are possible and permissions are
  enforced as that viewer.
- **Computed bindings:** sandboxed JS can derive values from the resolved
  sources.
- **Actions:** each one records the attendee's DID, the card, the block, an
  action id and a value.
  - The client sends it to our appview, which runs middleware and then
    writes the accepted action to the conference space as a record.
  - The attendee authors the action, and the space's policy governs who
    reads it.
  - The offline queue lives on the device until it's sent.
- **Middleware** is a sandboxed JS module.
  - It receives an action and the viewer context.
  - It returns one of: accept, reject with a message, a rewritten action,
    or ephemeral blocks.
  - The same module runs on the PWA (optimistically) and on the appview
    (authoritatively).
- **Sandbox limits** (custom blocks, computed bindings and middleware):
  - the code sees only its inputs: resolved bindings, or the action and
    context
  - it can only emit actions
  - it has no network and no storage
  - it runs under time and memory budgets

## Options considered

### Splitting the feature

- **Split into a family, starting with ui-blocks** (chosen): the block
  vocabulary, data format and interaction model are the hardest decisions
  to change later, and everything renders through them.
- **One big "configurable UI" feature**: too large to spec, test or review
  as a unit.
- **Start with conference-space**: also foundational, but ui-blocks can be
  proven in a test space first.

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

## Out of scope

- The feed: timeline, sub-feeds, ordering, pinning (`conference-feed`).
- The conference space and membership (`conference-space`). The gallery
  uses a test space.
- Organizer block editing (`block-editor`).
- Experience templates and how organizers configure them
  (`feed-templates`).
- Installing, trusting and authenticating third-party apps and their
  middleware (`feed-apps`).
- Any specific experience: polls, Q&A, chat, wifi, announcements, toys.
- Free-form styling.

## Open questions

- How the Rust appview runs sandboxed JS middleware (an embedded engine
  such as QuickJS, Boa or V8 isolates, or a JS sidecar), and how exactly
  the same module runs in the PWA (a worker, an iframe, or QuickJS-wasm).
  For architecture analysis and design review.
- How quasi-records are addressed and served. Probably a URI scheme and an
  appview endpoint, plus how clients subscribe to their changes.
- How an attendee sees a server rejection of a queued offline action beyond
  the card itself, e.g. whether a notification is needed when the card
  isn't on screen.
- Whether `ui-blocks` needs `conference-space` first, or can define its
  records against a generic space and leave membership to that feature.

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

## Design review

## Test cases

## Review log
