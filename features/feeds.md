---
status: ready
impact: cross-cutting
depends-on: [ui-blocks, attendee-sign-in, conference-space, space-sync]
branch:
tests-commit:
---

# Feeds

## Summary

The app is a series of feeds holding rich cards. Every screen an attendee
sees is a feed, and eventside builds each feed for each viewer. A feed
shows the posts that target it, ordered by its algorithm, which can pin
some posts to the top. Its cards render differently depending on who is
looking.

This replaces the planned `conference-feed` ("one conference timeline with
scoped sub-feeds", chosen in the [`ui-blocks`](ui-blocks.md) brainstorm).
It came out of going back to first principles after the
[`conference-space`](conference-space.md) build stalled on its access
model (see that file's "Back to the drawing board"). That access model is
now simplified to **one private space per conference**, with eventside
deciding visibility.

## Experience

**Attendees.**

- When an attendee opens a conference, they land on its **main feed**.
  Organizer posts, featured plans and computed cards like "your next
  session" mix together there. Some cards stay on top because of their
  type, for example the wifi information.
- Each session, room, plan and group has its own feed, which is its chat.
  Opening one shows only posts aimed at it.
- A card can look different for each viewer. A poll shows its results once
  you've voted. A "mark yourself safe" check shows "You're marked safe"
  after you respond, and organizers see a tally.
- An urgent post, such as a mark-safe check, can reach several feeds at
  once and can be pushed to phones. How fresh a feed stays (live,
  refreshed on open, or pushed) depends on the feed.
- Offline, every feed opens to its last state, with a note saying it's
  offline. Posts made offline are queued (see
  [`block-offline`](block-offline.md)).

**Posting.**

- A post names the feeds it targets. A feed accepts or refuses each post
  according to who may post there: only organizers in Announcements,
  anyone in a session chat. A post a feed refuses doesn't appear in that
  feed, but still appears in the other feeds that accepted it.
- A post can narrow who sees it, using the same kinds of audience feeds
  use.

**Moderators.** Organizers moderate by default; a plan's or group's
creator moderates its feed. A moderator can:

- pin and unpin a post, overriding the algorithm's placement;
- hide a post from their feed, leaving it in any other feeds;
- bar an author from posting to their feed.

**Organizers** configure feeds, normally from a template. Most feeds are
algorithmic and need no hand pinning.

## Data

- **One private space per conference**, hosted by eventside, e.g.
  `at://did:web:eventside.app/space/app.eventside.private/<conference>`.
  - Attendees can write to it but not read it. Eventside reads all of it,
    and so can any other app the organizer allows.
  - Everything in this feature lives there, organizer posts included:
    feed records, posts, pins, hides, bars and labels. Attendees' private
    actions (poll votes, RSVPs, mark-safe responses) live there too.
  - Nothing a conference posts is public on the network. Eventside
    serves it all.
- **Feed record:**
  - algorithm;
  - audience: all members, a group, a plan's attendees, organizers, or a
    list of people;
  - who may post;
  - moderators;
  - freshness: live, refresh, or push for urgent posts.
- **Post record:**
  - its target feeds;
  - an optional narrower audience;
  - its content: a card, as defined in [`ui-blocks`](ui-blocks.md).
- **Moderation records:** pins, hides and author bars, written by a
  feed's moderators. They follow Bluesky's pin and label records, at least
  as a guide.
- **Feed entries.** An algorithm returns a skeleton, a list of entries. An
  entry is either:
  - the AT-URI of a post or other item; or
  - a **computed card**, which names a `getCardData` call that eventside
    renders for the viewer.
- **Algorithms.** Every algorithm implements one skeleton API, modeled on
  Bluesky's `getFeedSkeleton`. An implementation can be:
  - built into eventside;
  - declarative rules in the feed record (pin by type, hide after a time,
    order by);
  - a wasm module on the [`block-sandbox`](block-sandbox.md) runtime;
  - an outside service the feed record points at.
- **Visibility.**
  - Eventside filters each skeleton for the viewer after the algorithm
    runs, so no algorithm, an outside one included, can show a viewer
    something they may not see.
  - A viewer sees a post when they're in the audience of at least one
    feed that accepted it, and also in the post's own audience if it has
    one.
  - Computed cards apply their own rules in `getCardData`.
- **On the device:** each feed's last rendered state, for offline use.

## Options considered

### What the app is built around

- **Feeds** (chosen): every screen is a feed of cards, built per viewer.
  Polls, announcements and plans are kinds of cards, not separate
  subsystems.
- **Nested groups as the model** (explored, set aside): every event a
  group, each with its own space, admins and membership. That led back
  into the transitive-admin and per-space-creation problems that stalled
  conference-space. Groups may still sit on top of feeds later, using the
  opensocial.group proposal for named, long-lived groups.

### Where a conference's data lives

- **One private space per conference** (chosen). Attendees write to it,
  and eventside reads it and manages visibility. The organizer may let
  other apps read it. We can't truly prevent a careless organizer from
  granting read access to a bad app (the "evilside.app" problem), so that
  risk is the organizer's.
- **A space per feed or item**, with space permissions doing the
  filtering: a feed could then be a plain list of AT-URIs. But creating
  hundreds of spaces needs the authority's session each time, and keeping
  the permissions right was the rabbit hole.
- **A single global private space for all conferences:** mixes every
  organizer's data, and each organizer can't choose which apps read
  theirs.
- **Polls under the poster's DID**, each with its own response space:
  same per-space cost.

### Which feeds exist

- **A main feed plus automatic scoped feeds** (chosen): every session,
  room, plan and group gets a feed, and organizers can add named ones.
- Main feed only, with organizers making the rest by hand.
- Nothing by default; the organizer or a template defines everything.

### How a post gets into a feed

- **The post names its feeds, and each feed accepts or refuses it**
  (chosen). Feed policy controls who may post, e.g. organizers only in
  Announcements.
- The post names its feeds, and anyone who can see a feed may post to it,
  moderated afterwards.
- The feed owner writes an entry record for each post, like a curated
  list. Attendees couldn't post directly.

### Where posts live

- **All in the private space** (chosen), organizer posts included.
- Public posts in the author's public repo and members-only posts in the
  private space.
- Organizer posts public, attendee posts private.

### Pinning and ordering

- **Algorithms place posts themselves, and moderators can also pin**
  (chosen). Most feeds are algorithmic: a wifi information post stays on
  top of the main feed because of its type.
- Only organizers pin.
- The post author flags it as pinned.

### How a feed's algorithm is defined

- **One skeleton API, with built-in, declarative, wasm or outside-service
  implementations** (chosen, external services included). The Nov 1
  demo needs built-in and outside-service algorithms.
- Declarative rules only.
- Built-in kinds plus settings only.
- Wasm only.

### Who sees a post

- **Both feed and post audiences** (chosen). A post can narrow its
  audience, using the same audience kinds as feeds.
- Feed audiences only.
- Per-post audiences only, with feeds only doing the ordering.

### Freshness

- **Per feed** (chosen): each feed says whether it's live, refreshed on
  open, or pushes urgent posts.
- Always live, plus push for urgent posts.
- Pull to refresh, plus push.

### Moderation

- **Pin and unpin, hide from this feed, and bar an author from this
  feed** (chosen), modeled on Bluesky's pin and label records.

## Out of scope

- Unread counts and badges.
- Declarative and wasm algorithms for the Nov 1 demo. The API allows
  them, but only built-in and outside-service algorithms are built now.
- Live updates, Web Push and the offline cache (design review round 5:
  deferred past the Nov 1 demo). This feature's feeds refresh when
  opened. The design for the three stays in this file for a follow-up
  feature to build.
- How people join a conference, and the list of apps an organizer allows.
  These belong to the `conference-space` rewrite.
- Named, long-lived groups (opensocial.group) and personal-account
  hosting.
- Specific experiences (polls, Q&A, mark-safe, plans, chat). They're
  cards and feeds built on this feature, specced separately.

## Open questions

None. The two raised in brainstorming were settled in the architecture
analysis and the design review:

- Outside generators authenticate with a service JWT, and see only the
  stream of their feed's posts (rounds 3–5).
- What changes in `space-sync` and `block-actions` is listed under
  "Impact on existing features".

## Architecture analysis

**Existing code touched:**

- `web/src/Shell.tsx` and `web/src/routes/`: the app has only the shell and
  sign-in today. Feeds become its main screens: a conference's main feed
  at its root, and a route per scoped feed.
- `web/src/blocks/`: posts render through `<BlockCard surface="feed">`,
  and computed cards through the same renderer. A `SourceResolver` (the
  seam in `SourceResolver.ts`) has to resolve sources against the
  conference's private space for the viewer. `block-actions` plans the
  live one; feeds needs at least a read-only one first.
- `lexicons/app/eventside/`: only `auth` and `block` exist. New lexicons
  go under `app.eventside.feed.*`.
- `crates/server/src/`: only auth, identity and OAuth exist. Feed
  assembly, per-viewer filtering, the skeleton API, `getCardData`, push
  and the outside-service client are all new.
- The parked `feature/conference-space` branch (at `eacd33e`) has a space
  host (`crates/server/src/spacehost/`). Feeds needs the private space it
  would host, so whatever the `conference-space` rewrite keeps of it is
  this feature's foundation.

**Features affected:**

- [`conference-space`](conference-space.md) (`ready` on `main`, paused on
  its branch): its access model is replaced by one private space per
  conference, hosted under eventside's `did:web`, which attendees write to
  and eventside reads, with an organizer-chosen list of other apps that
  may read it. Its roles (who organizes, who's a member) become feed
  audiences and moderators. It needs a rewrite before feeds can be built,
  so feeds depends on it.
- [`space-sync`](space-sync.md) (`analysis`): its model mostly holds:
  members can only `read_self`, the appview reads everything and serves
  views. Two things change. The space's authority is eventside itself, so
  the appview doesn't need a delegation token from an organizer's session
  to get credentials. And the app allow-list is the organizer's to
  extend, not fixed to the appview. Its index is what feeds are assembled
  from.
- [`block-actions`](block-actions.md) (`analysis`): actions already live
  in the attendee's repo in the space, which fits. Its views
  (`actionTally`, `actionList`) and computed cards overlap: either views
  stay as sources inside cards, or `getCardData` subsumes them. Its live
  `subscribeSources` socket is the natural carrier for live feeds.
- [`block-offline`](block-offline.md) (`analysis`): posting offline goes
  through its queue, and cached feeds sit beside its optimistic overlay.
- [`block-sandbox`](block-sandbox.md) (`analysis`): wasm feed algorithms
  would run on its runtime later (not for Nov 1).
- [`ui-blocks`](ui-blocks.md) (`complete`): unchanged code, but its "Feed
  shape" decision (one timeline with scoped sub-feeds) is generalized into
  feeds, and the `feed` surface gets its real host.
- [`attendee-sign-in`](attendee-sign-in.md) (`complete`): posting as an
  attendee needs a `space:` write scope on `app.eventside.feed.*` added to
  the sign-in scope list, which asks existing sessions to sign in again.
- [`plans`](plans.md) (`design-review`): plans become feeds (each plan's
  chat), plan items are posts, and its "changed", "invited you" and
  "featured" reasons and "past plans drop out" become algorithm rules. Its
  audiences (groups, connections) have to use feed audience kinds.

**New shared surfaces:**

- Lexicons: the feed record, the post record, pin, hide and author-bar
  records, and the audience kinds shared by feeds and posts.
- The skeleton API that every algorithm implements, built-in or outside.
- `getFeed` (hydrated, filtered per viewer) and `getCardData`, which every
  experience (polls, Q&A, mark-safe, plans, chat) builds on.
- The per-viewer visibility check, which other features will call to
  decide what a viewer may see.
- The app's navigation: every screen is a feed.

**Verdict:** cross-cutting. It adds lexicons and XRPC endpoints that every
experience builds on, sets the app's navigation, changes the premise of
`conference-space` and `space-sync`, and reshapes `plans` and
`block-actions`. It needs a design review, and it depends on the
`conference-space` rewrite for the private space it reads from.

## Design review

**Approach.**

- **Eventside is the feed host.** Every record in this feature lives in
  the conference's private space, in its author's repo there.
- **Every write goes through eventside.** Sign-in is a backend-for-frontend
  ([`attendee-sign-in`](attendee-sign-in.md)): the PWA holds only a cookie,
  and eventside holds the OAuth tokens. So when an attendee posts,
  eventside writes the record with their session. That lets it:
  - decide acceptance at write time and tell the composer straight away
    ("posted to Keynote; Announcements is organizers only");
  - index the record as it writes it, so it shows without waiting for
    [`space-sync`](space-sync.md);
  - refuse records the author may not write: feed records, pins, labels,
    `urgent`.
- **Records written behind eventside's back** (straight to a PDS) are
  checked the same way when space-sync indexes them.
  - A post whose target feed isn't indexed yet stays pending until the
    feed record arrives, rather than being refused.
- **Decisions are kept, not recomputed.** Each acceptance decision is
  stored with the time it was made:
  - A later role change or bar doesn't retract earlier posts. That follows
    the principle from the conference-space redesign: a decision is
    checked when it's made and stays valid.
  - A re-index or backfill keeps the stored decisions rather than
    recomputing them against today's roles.
  - Other apps the organizer allows have to trust eventside's decisions.
    They can read the records but can't reproduce the decisions exactly.
  - To take a post down, a moderator hides it, or the author deletes it.
- **`getFeed` builds one page of one feed for one viewer:**
  1. **Read the feed record.** If the viewer isn't in its audience, the
     answer is "not found", the same as for a feed that doesn't exist.
  2. **Get a skeleton from the feed's algorithm.** A built-in algorithm
     runs in-process. An outside feed generator is called over XRPC,
     and sees only that feed's own posts (see "Outside feed
     generators").
  3. **Filter for the viewer.** A post stays if both hold:
     - it was accepted into at least one feed whose audience includes the
       viewer;
     - it either has no audience of its own, or the viewer is in that
       audience too.

     Hidden, deleted and unknown entries are dropped.
  4. **Apply the moderators' pins**, on the first page only:
     - a pin counts only if the post was accepted into this feed and
       passed step 3;
     - pinned posts don't appear again on later pages.
  5. **Hydrate** the posts and computed cards, through the visibility
     rules below.
- **Short pages.** Filtering can leave a page short. Eventside asks the
  algorithm for more until the page is full or a budget runs out.

**Visibility of card data.** Every card is resolved by eventside, since
attendees can't read the space. That's the main privacy boundary, so this
feature defines the read-only `SourceResolver` and its rule. A source
resolves only if it is:

- (a) a post the viewer can see under the filter above;
- (b) a record in the viewer's own repo; or
- (c) a record by eventside or an organizer.

Beyond that:

- A card written by an attendee can't use `collectionSource`.
- Anything else is `unavailable`, and the reason never reaches the card
  (the [`ui-blocks`](ui-blocks.md) contract).
- `getCardData` providers run the same check on every URI in their
  parameters. "Missing" and "forbidden" give the same answer.
- [`block-actions`](block-actions.md) views (tallies, lists) are reached
  only through cards that pass this check.

**Who may write what** (eventside checks at write time, and again at
ingest for direct writes):

- **Feed records** count only from eventside's own repo. Eventside writes
  them:
  - the main feed, from the conference's template;
  - a scoped feed whenever a session, room, plan or group appears. Its
    rkey comes from the subject, so creating it again is harmless.
  - An organizer edits a feed through eventside, which rewrites the
    record.
- **Pins and labels** count only from the feed's moderators, and that's
  decided when they're written, like acceptance.
- **`urgent`** counts only on a post by an organizer or a moderator of a
  target feed.
- **`pinKinds`** lifts only posts by organizers or moderators. An
  attendee's post with `kind: wifi` isn't pinned.
- **Scopes.** Attendees' sessions get `space:` `create` and `delete` on
  `app.eventside.feed.post` only. Pins and labels are written with
  moderators' sessions, which add those collections. Feed records are
  written by eventside as the authority.

**Components.**

- **Server** (`crates/server/src/feeds/`):
  - `write`: posts, pins and labels, each checked as it's written.
  - `acceptance`: decisions, and the ingest hook for direct writes.
  - `audience`: resolvers by kind. An unknown kind matches nobody.
  - `sources`: the read-only source resolver and its visibility rule.
  - `algorithms`: the built-in algorithms, the outside-generator client,
    and the per-feed post stream that feeds generators.
  - `assemble`: `getFeed`.
  - `cards`: built-in `getCardData` providers, plus the client for
    outside providers, with its cache.
  - `provision`: the automatic feed records.
  - `live`: change notices, and Web Push.
  - A migration for decisions, pins, labels and push subscriptions.
- **Built-in algorithms:**
  - `reverseChronological`: newest first, with parameters `pinKinds` and
    `expireAfter`.
  - `chat`: oldest first.
  - `main`:
    - pinned kinds first;
    - then organizer posts and featured plans, newest first;
    - interleaved with computed cards such as "your next session" and
      the weather.
  - `cards`: a list of card entries, taken from the feed record's
    parameters, e.g. the weather feed's "now", "next hours", "next days"
    and "alerts" cards.
- **A reference weather provider** (`crates/weather-cards`): a small
  outside service. It implements `getCardData` from Open-Meteo, which is
  free and needs no key, and runs on its own `did:web`. It proves the
  outside path end to end, and tests run it against a fake forecast.
- **PWA** (`web/src/feeds/`):
  - `FeedScreen`, which renders entries with `<BlockCard surface="feed">`;
  - feed navigation from `listFeeds`;
  - a composer that posts to one or more feeds and shows where the post
    was accepted;
  - moderator actions;
  - an IndexedDB cache of each feed's last page, cleared on sign-out;
  - a service worker for push.
- **Routes:**
  - `/c/<conference>` opens the main feed.
  - `/c/<conference>/f/<feed rkey>` opens a scoped feed.

**Data** (lexicons under `app.eventside.feed.*`):

- **Card body.** `app.eventside.block.card` is a record type, so the
  card's body is split out as an object def, `app.eventside.block.defs#cardBody`,
  that a post can embed.
  - A card's `CardRef` is then the post's URI.
  - `block-actions` accepts `feed.post` as a container for cards.
- **`#audience`** is a list matching the union of:
  - `#members`;
  - `#organizers`;
  - `#group{uri}`;
  - `#attendees{of}`;
  - `#people{dids}`.

  Kinds are resolved by the features that own them. Until `groups`,
  `plans` and `program-import` provide theirs, `#group` and `#attendees`
  match nobody.
- **`app.eventside.feed.feed`** (eventside's repo):
  - `name`, `kind` and `subject?`;
  - `algorithm`: `#builtin{name, params}`, `#service{did,
    sendViewer}`, or the reserved `#declarative` and `#wasm`. A card
    entry's provider can be outside too, with `sendViewer` set per
    entry.
  - `audience`, `posters` and `moderators`;
  - `freshness`: `live`, `refresh` or `push`.
- **`app.eventside.feed.post`:**
  - `feeds`, `kind`, `audience?`, `card` (a `#cardBody`), `urgent?` and
    `createdAt`.
- **`app.eventside.feed.pin`** `{feed, subject, createdAt}`, by a
  moderator. Bluesky keeps a pin on the profile record; here a pin is per
  feed, so it's a record of its own.
- **`app.eventside.feed.label`** `{feed, uri, val, neg?, cts}`, by a
  moderator. It's shaped after `com.atproto.label.defs#label`, scoped to
  one feed:
  - `hide` on a post hides it from this feed.
  - `bar` on a DID refuses that author's future posts here.
  - `neg` undoes either one.
- **Decisions** (eventside's database): `(record, feed, outcome, reason,
  decidedAt)`, for posts, pins and labels.
- **The post view** that clients get leaves out `feeds` and `audience`.
  It shows only the target feeds the viewer can see, and never the
  audience. Otherwise a post would reveal that a private plan's feed
  exists, or who is in its audience.

**Interfaces** (XRPC; the PWA calls them with the session cookie):

- **Writes:**
  - `app.eventside.feed.createPost` returns the per-feed decisions.
  - `deletePost`.
  - `pin` and `label`, for moderators.
- **Reads:**
  - `getFeed{feed, cursor?}` returns the feed's view (name, kind,
    freshness, and whether the viewer may post or moderate), plus
    entries `[{post, pinned?} | {card}]`.
  - `listFeeds{conference}`.
  - `getCardData{provider, params}`.
- **Outside card providers** implement the same
  `app.eventside.feed.getCardData`. A provider is named as `did#fragment`,
  and its DID document lists an `#eventside_cards` service endpoint.
  - **Request:** eventside calls `{provider, params}` with a service JWT.
    The params come from the feed record, e.g. the venue's latitude and
    longitude, and the units.
  - **Response:** `{card, data, expiresAt}`.
    - `card` is a `#cardBody`, a template built from the block vocabulary
      (the "card kit").
    - Its blocks bind to `data` through a new `#inlineSource`, so a
      provider can send the same template every time and only the data
      changes.
  - **What a provider never gets:** no posts, no space access, and no
    other attendees' data. The viewer's DID goes in the JWT's `sub` only
    if the feed record says `sendViewer`. Weather doesn't need it.
  - **What eventside checks on the card:**
    - it must pass the ui-blocks validator;
    - no middleware, actions, wasm or sources other than `#inlineSource`;
    - buttons may only open links.
  - **Images.** Viewers never load a provider's image URLs directly,
    which would give it every viewer's IP address. Eventside rewrites
    them to its own image proxy, which fetches each image once and caches
    it. A Bluesky post card's avatar and photos come through the proxy.
    Glyphs use a new `icon` block, from a fixed set that includes weather
    glyphs.
  - **Caching.** Eventside caches each answer until `expiresAt`, shared by
    every viewer when there's no `sub`. One forecast call then serves the
    whole conference.
  - **Failures.** A slow or failing provider is cut off at a timeout.
    Eventside serves the last cached answer, marked stale, or
    `unavailable`.
  - **Allow-list.** Eventside calls only providers on the organizer's
    allow-list.
- **Outside feed generators** (`#service`):
  - **What they get.** A generator is given the posts accepted into the
    feeds that name it, and nothing else. It gets them by opening
    `app.eventside.feed.subscribePosts{feed, cursor?}`, a WebSocket
    authenticated with its service JWT. Eventside accepts the
    subscription only if the feed record's `#service` names that DID and
    the DID is on the organizer's allow-list. The stream:
    - replays from the cursor, or from the start for backfill;
    - sends `#post{uri, cid, author, record, acceptedAt}`, where `record`
      is the full post minus `feeds` and `audience`. Nothing says where
      else it was posted, or who may see it;
    - sends `#remove{uri}` when a post is deleted or hidden from this
      feed.
  - **Restricted posts are streamed too, without their restriction.** A
    post that narrows its own audience ("only my friends") reaches the
    generator like any other, and the generator can rank it. Eventside
    strips it from the feed for every viewer outside that audience. Posts
    in other feeds, poll votes, RSVPs and every other record in the space
    are never streamed.
  - **Card data.** A post's card arrives as its template, with no data
    filled in.
  - **Per viewer.** A feed record with `sendViewer` puts the viewer's DID
    in the `sub` of the `getSkeleton` call, so a generator can make each
    viewer's feed different. That tells the generator who reads the feed
    and when, so it's off unless the feed asks for it.
  - **What it returns.** `app.eventside.feed.getSkeleton{feed, cursor?,
    limit?}` (modeled on `app.bsky.feed.getFeedSkeleton`) returns
    `{entries, cursor?}`. Entries can be anything:
    - post URIs, from this feed or elsewhere;
    - card entries naming allow-listed providers. A public Bluesky post
      is one of these: a card entry whose provider renders it as a card,
      the same way weather does. Eventside has no Bluesky-specific
      renderer.

    Eventside filters them like any skeleton: a viewer sees only what
    the visibility rules allow. Anything else is dropped.
  - **Failures.** A slow or failing generator is cut off at a timeout,
    and the feed falls back to newest first with a quiet note.
- **Bsky Buzz**, the example generator for Nov 1 (`crates/bsky-buzz`):
  - it mixes the conference's public Bluesky conversation (posts with
    its hashtag, and posts by speakers) with the posts streamed from its
    feed;
  - it is also a card provider: each Bluesky post becomes a card entry
    naming Bsky Buzz, and its `getCardData` returns the post as a card
    template plus data;
  - it ranks them by recent activity;
  - it runs as a separate service on its own `did:web`.

  Tests run it against vivarium's Jetstream.
- **Live:**
  - `subscribe` is a WebSocket saying "feed X changed".
  - `registerPush`.

**Impact on existing features.**

- **`conference-space`:** rewritten around one private space per
  conference, with eventside's `did:web` as its authority. It provides:
  - the `#members` and `#organizers` resolvers;
  - the organizer's list of allowed apps;
  - the template that seeds the main feed.

  The rewrite has to confirm that the space host on the parked branch can
  hold the authority's own repo, and that the `did:web` document points at
  it.
- **`space-sync`:** its index stays. Writes through eventside are indexed
  as they're written, so the demo doesn't need the notify path. The
  organizer's delegation token goes, since eventside is the authority. It
  gains the ingest hook.
- **`block-actions`:** treats `feed.post` as a container for cards. Its
  views are reached through the source visibility rule.
- **`attendee-sign-in`:** adds `space:` scopes on `feed.post` for
  everyone, and on `feed.pin` and `feed.label` for moderators. Everyone is
  asked to sign in again, so this should ship before attendees install the
  app, not during the conference.
- **`ui-blocks`:** `#cardBody` is split out of the card record, with no
  change in behavior. `feed` surface cards get a real host.
- **`plans`:**
  - A plan's chat is its feed.
  - Its "changed", "invited you" and "featured" reasons become behavior
    of the `main` and chat algorithms.
  - Its audiences use `#audience`.

**Alternatives.**

- **Decide acceptance at read time**, from current roles: a role change
  would retroactively hide or reveal old posts.
- **Implicit feeds**, with no record per subject: other allowed apps
  wouldn't see them, and organizers couldn't override one feed.
- **Writes straight from the PWA** (the first draft said "no proxy"):
  impossible with a backend-for-frontend. Writing through eventside also
  gives immediate feedback.
- **Outside services reading the space themselves:** they'd see secret
  ballots and mark-safe responses, and they'd need space credentials.
- **Outside services ordering a filtered candidate list** (round 1):
  candidates were sent with every request, and covered whatever the
  viewer could see.
- **Generators get no post data at all** (round 2): they could then only
  supply cards. Round 3 settled on a stream of the feed's own
  unrestricted posts instead.
- **Providers returning finished blocks with values baked in:** simpler,
  but there'd be no template to reuse, and the card would have to be sent
  again whenever the data changed.
- **Two record types for hides and bars:** the label shape covers both,
  plus undoing them.

**Risks.**

- **Schedule.**
  - Nov 1 is 24 days away. Feeds needs the `conference-space` rewrite,
    which isn't specced yet, and `space-sync` (in `analysis`). Both need
    tests and a build.
  - Proposed demo slice: the main feed, chat feeds, posting with
    decisions, pins and hides, built-in algorithms, a weather card on the
    main feed, a weather feed from the outside provider, and one feed
    driven by the Bsky Buzz generator, whose Bluesky posts arrive as
    cards. Weather and Bsky Buzz each run as their own service. Feeds
    refresh on open, and records are indexed as eventside writes them.
  - Proposed to defer: live sockets, Web Push and the offline cache.
- **Web Push** works on iOS only for an installed PWA, and needs VAPID
  keys and a service worker.
- **Trusting eventside's decisions.** Other allowed apps have to take
  eventside's acceptance decisions on trust. If that matters later, the
  decisions could be published as records.

### Round 1

The draft was critiqued before being presented. Folded in:

- **Card sources.** The source visibility rule was added. Without it, a
  card's sources could have exposed other attendees' votes, RSVPs and
  responses.
- **Writes go through eventside**, since sign-in is a backend-for-frontend:
  - posts are decided as they're written;
  - the composer gets the decision straight away;
  - records are indexed as they're written.
- **Trust rules** for feed records, pins, labels, `urgent` and `pinKinds`,
  with scopes narrowed to match.
- **Outside services** only order a filtered candidate list. The viewer's
  DID is sent only if the feed opts in, and only allow-listed services are
  called.
- **The post view** hides `feeds` and `audience`. `getCardData` checks
  every URI in its parameters.
- **Pending posts** when the target feed isn't indexed yet. Stored
  decisions are kept, never recomputed.
- **Smaller points:**
  - the `#cardBody` split;
  - pin paging;
  - checking that the space host can hold the authority's own repo;
  - idempotent feed rkeys;
  - clearing the offline cache on sign-out;
  - timing the new sign-in;
  - audience kinds that match nobody until their features exist.
- **The demo slice** now keeps the outside-service algorithm, which was
  chosen for Nov 1. It defers live sockets, Web Push and the offline
  cache instead.

**The user's feedback:**

- Outside algorithms must not be able to consume post data. The use
  cases for outside services should be thought through.
- Add a weather card for November 1. It should use `getCardData` and a
  structured card kit, so a weather feed can be built from it.

### Round 2

- **Outside services supply cards, not orderings.** An outside provider
  implements `getCardData` and gets no posts or space access, nor the
  viewer's identity unless the feed opts in. The `#service` algorithm is
  reserved.
- **Use cases considered:**
  - the public Bluesky conversation (deferred, as outside skeleton
    sources);
  - recommendations from the service's own data (needs `sendViewer`);
  - sponsor slots;
  - live operations: weather, transit, room capacity;
  - other events' listings.
- **Card kit.**
  - Providers return a `#cardBody` template plus `data`, bound through a
    new `#inlineSource`.
  - A new `icon` block replaces remote images, which would leak viewers'
    IP addresses.
  - Both are additions to [`ui-blocks`](ui-blocks.md).
- **Weather for Nov 1:**
  - a reference `weather-cards` provider, on Open-Meteo;
  - a weather card on the main feed;
  - a weather feed built with the `cards` built-in algorithm.
- **Caching:** answers are cached until they expire, and shared when no
  viewer identity was sent.

**The user's feedback:**

- An outside feed generator can return any data. Eventside filters the
  response by the visibility rules in the records it has access to.
- A generator is given access to the records posted to its feed. Posts
  with a restricted audience are filtered out.
- A generator can be per user, and it should have the feed's posts
  streamed to it.

### Round 3

- **The `#service` algorithm is back.** A generator gets a
  `subscribePosts` stream of the posts accepted into its feed, minus any
  post with its own audience. It can be per viewer with `sendViewer`,
  which is off by default.
- **Its skeleton can name anything:** posts from anywhere, or allow-listed
  card providers. Eventside's per-viewer filter is the only guard on what
  it returns.
- **Outside card providers stay**, with weather as the Nov 1 example.
- **The demo slice** adds one feed driven by an outside generator, with a
  fake generator in tests.

**The user's feedback:**

- The example outside generator is "Bsky Buzz".
- A generator gets the full data of every post made to its feed, except
  the cross-posting information, and generates a feed list from it.
- If a post says "only my friends", it's still in the feed, but it's
  stripped out for most people.

### Round 4

- **The stream carries every post accepted into the feed,** restricted
  ones included, with each post's audience but without its `feeds`
  field. The visibility filter strips restricted posts for viewers
  outside their audience.
- **Skeletons can name public Bluesky posts,** and eventside renders them
  as Bluesky post cards. That moves public records from deferred into
  Nov 1.
- **Bsky Buzz** is the reference generator. It mixes the conference's
  Bluesky conversation with the feed's own posts.

**The user's feedback:**

- A Bluesky post is a custom card, the same as weather.
- The feed generator shouldn't get post restrictions.
- The weather card is a service.
- "I think we're good": approved, with these changes.

### Round 5 (approved)

- **Bluesky posts are card entries** that Bsky Buzz serves through
  `getCardData`. Eventside renders nothing Bluesky-specific.
- **Images in outside cards** go through eventside's image proxy, instead
  of being refused, so Bluesky avatars and photos can show without
  exposing viewers.
- **The stream leaves out `audience`** as well as `feeds`. Restricted
  posts are still streamed, and filtered per viewer.
- **Weather** is the outside `weather-cards` service.
- **Taken as agreed** (the round 2 questions, settled by the approval):
  - the `#cardBody`, `#inlineSource` and `icon` additions to
    [`ui-blocks`](ui-blocks.md) are built as part of feeds;
  - the Nov 1 slice defers live updates, push and the offline cache.

## Test cases

The scenario is AtmosphereConf 2027 in Amsterdam:

- **Olga** organizes the conference, and **Pim** is on the staff.
- **Ana**, **Bram** and **Femke** are attendees, and **Mallory** isn't
  one.
- **Kees** moderates the "Grote Zaal" session chat, but isn't an
  organizer.

Each conference is set up by whatever the `conference-space` rewrite
provides. These cases assume only that a conference has members and
organizers, and an organizer's list of allowed apps.

### Feeds

#### TC-1: An attendee lands on the main feed

- **Given** Olga has posted "Welcome to Amsterdam" and then "Doors open
  09:00"
- **When** Ana opens the conference
- **Then** she sees its main feed, with "Doors open 09:00" above
  "Welcome to Amsterdam"

#### TC-2: Each session, room, plan and group gets its own feed

- **Given** the conference has a main feed only
- **When** the session "Lexicons in practice", the room "Zaal B", a plan
  and a group are added
- **Then** each of them has a feed of its own, scoped to it
- **And** adding the same session again doesn't create a second feed

#### TC-3: An organizer adds a named feed

- **When** Olga adds a feed called "Announcements" that only organizers
  may post to
- **Then** members see "Announcements" among the conference's feeds

#### TC-4: A feed only lists for the people who can see it

- **Given** the "Speakers" feed's audience is a list of the speakers,
  and Ana isn't one
- **When** Ana lists the conference's feeds
- **Then** "Speakers" isn't among them
- **And** opening it directly answers "not found", the same as a feed
  that doesn't exist

#### TC-5: Someone who isn't a member can't read a feed

- **When** Mallory, or a visitor who isn't signed in, opens the main feed
- **Then** they get "not found", and no posts

#### TC-6: An empty feed says so

- **Given** nobody has posted in the "Zaal C" feed
- **When** Ana opens it
- **Then** she sees an empty state, not an error

### Posting

#### TC-7: An attendee posts in a session chat

- **Given** Bram posted "Slides?" in the "Grote Zaal" chat
- **When** Ana posts "Are the slides online?" there
- **Then** both posts are in the chat, oldest first
- **And** they show who posted them

#### TC-8: One post goes to several feeds, and each feed decides

- **When** Ana posts "Lost a blue scarf" to the "Grote Zaal" chat and to
  "Announcements"
- **Then** the composer tells her it was posted to "Grote Zaal" and
  refused by "Announcements", which only organizers can post to
- **And** the post appears in "Grote Zaal" only

#### TC-9: An organizer's announcement reaches several feeds

- **When** Olga posts "Keynote moved to Zaal B" to the main feed,
  "Announcements" and the "Grote Zaal" chat
- **Then** it appears in all three

#### TC-10: A decision stands after a role change

- **Given** Pim posted "Lunch is served" to "Announcements" while he was
  staff
- **When** Olga removes Pim from the staff
- **Then** "Lunch is served" is still in "Announcements"
- **And** Pim's next post to "Announcements" is refused

#### TC-11: A post written behind eventside's back is checked the same way

- **When** Ana writes posts straight into the space from another client,
  one targeting "Announcements" and one targeting the "Grote Zaal" chat
- **Then** the "Grote Zaal" post appears in that chat
- **And** "Announcements" doesn't show hers

#### TC-12: A post that arrives before its feed waits for it

- **Given** a post targets the feed of a plan that eventside hasn't seen
  yet
- **When** the plan's feed arrives
- **Then** the post is checked against it, and appears if it's accepted

#### TC-13: Deleting a post removes it everywhere

- **Given** Olga's "Keynote moved to Zaal B" is in three feeds
- **When** she deletes it
- **Then** it's gone from all three

### Who sees what

#### TC-14: A post can narrow its audience

- **Given** the "Grote Zaal" chat is open to all members
- **When** Ana posts "Meet at the garderobe?" there, visible only to
  Bram and Femke
- **Then** Bram and Femke see it in the chat
- **And** nobody else does, organizers included

#### TC-15: A post doesn't reveal where else it went, or who may see it

- **Given** Ana posted to the "Grote Zaal" chat and to a private plan's
  feed, visible only to Bram and Femke
- **When** Bram reads the post in the "Grote Zaal" chat
- **Then** nothing in what he's served names the plan's feed, or the
  post's audience

#### TC-16: A card can't show someone else's private data

- **Given** Femke voted in a poll, and that vote is in her repo in the
  space
- **When** Ana posts a card whose source points at Femke's vote
- **Then** everyone except Femke sees that part of the card as
  "unavailable", and the rest of the card renders
- **And** it looks the same as a source that doesn't exist

#### TC-17: An attendee's card can't list a collection

- **When** Ana posts a card with a source that lists a whole collection
- **Then** that source is "unavailable" for every viewer

#### TC-18: Card data for something you can't see looks missing

- **When** Mallory asks for a card's data, with parameters that name a
  post she can't see
- **Then** she gets "unavailable", exactly as for a post that doesn't
  exist

### Ordering and moderation

#### TC-19: Wifi information stays on top of the main feed

- **Given** the main feed keeps `wifi` posts on top, and Olga posted the
  wifi details ("Atmosphere-Gast") on day 1
- **When** Ana opens the main feed on day 2, after many newer posts
- **Then** the wifi post is first
- **And** a post of Ana's marked as `wifi` isn't lifted

#### TC-20: Past plans drop out

- **Given** the main feed features "Bitterballen after the talks, 19:00"
- **When** Ana opens the main feed after it's over
- **Then** the plan is no longer in it

#### TC-21: A moderator pins a post

- **When** Kees pins Bram's "Slides are at lexicons.example" in the
  "Grote Zaal" chat
- **Then** it shows first in that chat, marked as pinned
- **And** it appears only once as Ana scrolls through every page
- **And** after Kees unpins it, it's back in its usual place

#### TC-22: A pin from someone who isn't a moderator does nothing

- **When** Ana pins her own post in the "Grote Zaal" chat
- **Then** the post isn't lifted, for her or anyone else

#### TC-23: A moderator hides a post from one feed

- **Given** Bram posted to the "Grote Zaal" chat and the main feed
- **When** Kees hides the post in the "Grote Zaal" chat
- **Then** it's gone from that chat and still in the main feed
- **And** after Kees undoes the hide, it's back in the chat

#### TC-24: A moderator bars an author from a feed

- **Given** Bram already has posts in the "Grote Zaal" chat
- **When** Kees bars Bram from the chat
- **Then** Bram's new posts there are refused, and his earlier ones stay
- **And** after Kees lifts the bar, Bram can post there again

#### TC-25: Only organizers and moderators can make a post urgent

- **When** Ana posts to the "Grote Zaal" chat marked as urgent
- **Then** the post appears there, but not as urgent
- **And** the same post from Olga is urgent

#### TC-26: Attendees can't define feeds or moderate

- **When** Ana writes a feed record, a pin or a hide straight into the
  space, for a feed she doesn't moderate
- **Then** no feed changes, for anyone

### Outside card providers

#### TC-27: The weather card on the main feed

- **Given** Olga has allowed the weather service, and the main feed
  includes its card for the venue
- **When** Ana opens the main feed
- **Then** she sees the current weather in Amsterdam, with a weather icon
  and the temperature, as a card

#### TC-28: A weather feed

- **Given** Olga has added a "Weather" feed made of weather cards
- **When** Ana opens it
- **Then** she sees cards for now, the next hours, the next days and any
  alerts

#### TC-29: One forecast serves everyone, and nobody is identified

- **Given** the weather card is still fresh
- **When** Ana, Bram and Femke each open the main feed
- **Then** the weather service is asked once
- **And** it isn't told who's looking

#### TC-30: A failing provider doesn't break the feed

- **Given** the weather service has answered before, and then goes down
- **When** Ana opens the main feed
- **Then** she sees the last weather card, marked as out of date, and the
  rest of the feed
- **And** if there was never an answer, the card is "unavailable"

#### TC-31: Only providers the organizer allowed are called

- **Given** a feed names a card provider that isn't on Olga's list of
  allowed apps
- **When** Ana opens the feed
- **Then** the provider is never contacted, and the card is
  "unavailable"

#### TC-32: A provider's card can only show

- **When** a provider answers with a card that has actions, middleware,
  or sources other than its own data
- **Then** the card is "unavailable"

#### TC-33: Viewers never load images from a provider

- **Given** a provider's card includes an image
- **When** Ana opens the feed
- **Then** the image shows
- **And** her browser fetched it from eventside, not the provider

### Outside feed generators

#### TC-34: A generator receives its feed's posts and nothing else

- **Given** the "Buzz" feed is driven by Bsky Buzz, which Olga allowed
- **When** Ana posts to "Buzz" and the "Grote Zaal" chat, Femke posts
  only in the "Grote Zaal" chat, and Femke votes in a poll
- **Then** Bsky Buzz is sent Ana's post, without its list of feeds or
  its audience
- **And** it isn't sent Femke's post or her vote
- **And** when Ana deletes her post, or a moderator hides it from
  "Buzz", Bsky Buzz is told it's gone
- **And** after reconnecting from where it left off, Bsky Buzz misses
  nothing

#### TC-35: Only the feed's own allowed generator can subscribe

- **When** a service that "Buzz" doesn't name, or one that isn't on
  Olga's list, asks for the posts of "Buzz"
- **Then** it's refused

#### TC-36: A restricted post a generator ranks is shown only to its audience

- **Given** Ana posted to "Buzz", visible only to Bram
- **When** Bsky Buzz puts that post in its feed
- **Then** Bram sees it in "Buzz", and Femke doesn't

#### TC-37: Whatever a generator returns is filtered per viewer

- **When** Bsky Buzz returns a post from a private plan Femke isn't in,
  a record eventside can't show, and a card from a provider Olga hasn't
  allowed
- **Then** Femke's "Buzz" shows none of them

#### TC-38: A generator learns who's reading only if the feed allows it

- **Given** one feed driven by Bsky Buzz sends the viewer, and another
  doesn't
- **When** Ana opens each
- **Then** Bsky Buzz is told it's Ana only for the first
- **And** it can give her a feed of her own there

#### TC-39: A failing generator falls back to newest first

- **Given** Bsky Buzz is down
- **When** Ana opens "Buzz"
- **Then** she sees the feed's posts newest first, with a note that the
  usual ordering is unavailable

#### TC-40: Bsky Buzz mixes in the public conversation

- **Given** people on Bluesky are posting with the conference's hashtag
- **When** Ana opens "Buzz"
- **Then** she sees those Bluesky posts as cards, with their authors'
  avatars, mixed with the posts made to "Buzz"

### Regressions

#### TC-41: Signing in still works, with the new permission

- **When** Ana signs in
- **Then** she's asked for permission to post to conferences, and the
  sign-in cases of [`attendee-sign-in`](attendee-sign-in.md) still pass
- **And** someone signed in before this change is asked to sign in again
  before they can post

#### TC-42: Existing cards render as before

- **When** the [`ui-blocks`](ui-blocks.md) gallery and card tests run
  after the card body is split out and the `icon` block and inline data
  are added
- **Then** they pass unchanged, and the gallery shows the `icon` block

## Review log
