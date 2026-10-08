---
status: analysis
impact:
depends-on: [ui-blocks, space-sync, attendee-sign-in]
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
- How people join a conference, and the list of apps an organizer allows.
  These belong to the `conference-space` rewrite.
- Named, long-lived groups (opensocial.group) and personal-account
  hosting.
- Specific experiences (polls, Q&A, mark-safe, plans, chat). They're
  cards and feeds built on this feature, specced separately.

## Open questions

- How an outside-service algorithm authenticates to eventside, and what
  it may see. Its skeleton is filtered afterwards, but it still has to
  read posts to rank them.
- How much the [`space-sync`](space-sync.md) and
  [`block-actions`](block-actions.md) specs change now that there's one
  private space per conference, written by attendees and readable by
  eventside. For the architecture analysis.

## Architecture analysis

## Design review

## Test cases

## Review log
