---
status: design-review
impact: cross-cutting
depends-on: [conference-space, space-sync, block-actions, conference-feed, groups, places]
branch:
tests-commit:
---

# Plans

## Summary

Any attendee can post an unofficial plan, such as "Late-night noodles at
Wok2Walk, 23:00, 6 spots". They choose who sees it, and others join with
one tap. This is "let's go do X", the moment the product optimizes for
(see the vision doc): a small group, a time, a place, and no approval
needed. It's item 2 of the November 1 demo: create it, pick who sees it,
others join, chat and show up.

The plan's chat is the [`chat`](chat.md) feature's. Who a plan can be shown
to is built on [`groups`](groups.md), and where it happens on
[`places`](places.md).

## Experience

**Posting a plan:**

1. An attendee taps "New plan".
2. They give it a title ("Bitterballen after the talks"), a start time, and
   optionally an end time, a note and a number of **spots** ("table for 6").
3. They pick a place, in one of three ways:
   - a **venue place** from the conference's list: rooms from the program,
     and areas the organizer adds, such as "the crash pad" or "the
     hack-station"
   - an **outside place** found by search ("Wok2Walk")
   - **free text** ("by the bikes on the bridge")
4. They choose who can see it:
   - the whole conference
   - picked people
   - a group: their connections, the people going to a plan or session, or
     one of their own lists ("cool cats I've met")
5. They choose whether attendance is **optional** (each person decides if
   their name shows) or **private for everyone**, and who may invite more
   people: nobody, anyone going, or a group.
6. They post it. It appears for everyone in the audience straight away.

**Finding plans.** People in the audience see a plan:

- as a card in the conference feed
- in a Plans list of upcoming plans, soonest first
- next to sessions in their Going/schedule view, once they've joined
- on the plan's own page
- as a card posted or embedded in a group chat

**Joining:**

- One tap for **going**, another for **interested**, and one to back out.
- Everyone who can see the plan sees a count: "5 going, 2 interested".
  With spots it reads "5 of 6 going". Spots aren't enforced yet, so a
  plan can go over.
- Each person's attendance is **public** by default: their name shows on the
  plan. They can switch theirs to private, and they still count. If the
  host made attendance private for everyone, nobody is named.
- The host always sees everyone who's coming, private or not.

**Inviting.** Whoever the host allows can add people or a group to the
audience. Invited people see the plan marked "invited you".

**Changes.** The host can:

- edit the time, place or details: people going see the plan marked
  "changed"
- cancel it: the card shows it's cancelled until its time passes
- widen or narrow who can see it
- change who may invite

Changes and invitations show only in the app: on the card, and in the feed
as "changed" or "invited you". There are no push notifications yet.

**Featuring.** An organizer can feature a plan, which shows "Featured by
the organizers":

- A plan open to the whole conference goes wide: it appears wherever
  official things appear (pins, schedule, Today).
- A plan with a narrower audience keeps it, and only gets the badge.
- The host can remove the feature ("we're keeping it small").

**After the plan.** Once it's over (at its end time, or a few hours after the
start if there's none) it drops out of the feed, the Plans list and pins.
Plans you went to stay in your history, with their page and chat.

## Data

- **The plan** is a `community.lexicon.calendar.event` in the host's repo,
  written into the conference space. It holds `name`, `description`,
  `startsAt`, optional `endsAt`, `status` (`scheduled`, `rescheduled`,
  `cancelled`) and `locations`.
  - It hangs off the conference node with a `childOf` claim, as in the vision's
    tree model.
- **Location** uses the open `locations` union:
  - a venue place: our own `app.eventside.location.place` (the place record's
    AT-URI and its name), plus a copy of the place's address or geo so other
    calendar apps still show something
  - an outside place: `community.lexicon.location.fsq` (Foursquare place ID,
    lat/long, name)
  - free text: `community.lexicon.location.address` with the conference's
    country and `name` set to the text
- **Plan settings** live in an eventside sidecar record next to the event:
  the audience (whole conference, picked DIDs, or group references), spots,
  the attendance policy and who may invite.
- **RSVPs** are `community.lexicon.calendar.rsvp` records (`going`,
  `interested`, `notgoing`) in each attendee's repo in the space. Whether an
  attendee's own attendance is public is a field in an eventside sidecar,
  or an extra field on the RSVP; the architecture analysis decides which.
- **Invitations** are records by the inviter that add people or groups to
  the audience. The appview honors them only if the host's settings allow
  that inviter.
- **Featuring** is an `includes` record in the organizer's repo, as in the
  vision. Removing it is the host's sidecar refusing it, or the organizer
  deleting it.
- **Who sees what** is enforced by the appview. Members can read only their
  own records ([`space-sync`](space-sync.md)), so the appview serves a plan
  and its RSVPs only to the audience. The attendee list it serves names only
  public attendance, except to the host.

## Options considered

### Splitting

- **Plans and chat as two features, brainstormed separately** (chosen).
  Chat is a stream on any node: the conference, a session or a plan.
- One feature with a chat only for plans: chat would be redesigned when it
  generalizes.
- One combined feature: a big build and a big review.

### Who a plan can be shown to

- **All of these: the whole conference, picked people, my connections, and
  groups** (chosen).

### What "show up" means

- **Nothing extra: the RSVP and the plan card's time and place** (chosen).
- A check-in ("I'm here") that records attendance: deferred.
- A live "on my way / here" status: more moving parts.

### What a plan holds

- **Title, time, place, note and spots, with places picked from a list**
  (chosen). A place can also be found by search or typed.
- Title, time, free-text place and note only: quickest, but no structured
  places.
- Adding spots alone, without a place list.

### Where places come from

- **The venue's list (rooms from the program, plus organizer areas), plus
  outside search** (chosen). Bookable breakout rooms come later, as more
  kinds of place in the same list.
- Organizer-added places only: rooms would be entered twice.
- Rooms only, with everything else free text.

### Outside place search

- **Foursquare** (proposed): `community.lexicon.location.fsq` already exists,
  and its data can be stored in records.
- Google Places: its terms allow storing only the `place_id`, not the name
  or coordinates. The user named both, so `places` makes the final choice.

### Spots

- **A spot count that's shown but not enforced** (chosen by the user): "punt
  on not overfilling". Enforcing it (refusing or waitlisting RSVPs past the
  limit) comes later.

### What a group is

- **One concept covering every kind of group, as its own feature
  ([`groups`](groups.md))** (chosen): derived groups (my connections,
  going to a node, chat members) and lists (an organizer's or an
  attendee's own).
- Groups defined inside plans: they'd be pulled out again for chat,
  sponsors and announcements.
- For the MVP, groups must cover my connections, going to a plan or
  session, and my own lists. Organizer lists and chat members come later.

### Whether the audience follows changes

- **It follows changes** (settled by choosing live groups): membership is
  checked whenever the appview serves the plan.
- A snapshot taken when the plan is posted: simpler, but late connections
  would miss out.

### RSVP states

- **Going and interested** (chosen): both are standard community RSVP
  statuses.
- Going only.

### Who sees who's going

- **Everyone sees a count. Each person chooses public or private, with
  public the default, and the host can make it private for everyone on that
  plan. The host always sees everyone** (chosen by the user).
- Anyone who can see the plan sees everyone.
- Only people going see the list.
- Only the host sees it.
- A policy set by the conference organizer, or by both organizer and host:
  the user chose a per-plan host setting.

### Who can invite

- **The host chooses: nobody, anyone going, or a group** (chosen by the
  user).
- Anyone going, always.
- The host only.

### Notifications

- **In the app only for the MVP** (chosen). Web push comes later, with chat.
- Web push now: permissions, a service worker and iOS limits.

### Featuring

- **In the MVP. A whole-conference plan goes wide, a plan with a narrower
  audience only gets the badge, and the host can remove it** (chosen by the
  user).
- Badge only, with the audience never widened.
- Later: it touches billing (official nodes) and the organizer admin.
- On consent, also offered: only the organizer can unfeature, or the host
  must accept first.

### Past plans

- **They drop out of lists, except the ones you went to** (chosen).
- Staying listed, greyed out, until the conference ends.

## Out of scope

- Enforcing spots, and waitlists.
- Push notifications.
- Booking rooms, and bookable breakout rooms.
- Checking in, and recording who actually showed up.
- The plan's chat ([`chat`](chat.md)).
- Organizer-defined groups and chat-member groups ([`groups`](groups.md)
  after the MVP).
- Plans that aren't part of a conference.
- Showing plans in other calendar apps: they live in a permissioned space.

## Open questions

- Is a free-text place stored as `location.address` with only a country and
  a name, or does that misuse the type? The alternative is an
  `app.eventside.location.text` union member.
- Where does an attendee's public/private attendance choice live: a field
  on the RSVP, or a sidecar? Extra fields on a community record are allowed,
  but other apps ignore them.
- How is a host's "remove the feature" recorded? An `includes` record lives
  in the organizer's repo, which the host can't delete.
- Does featuring a whole-conference plan make it an "official" node for
  billing (vision: active participants)?
- Does an edit by the host send everyone going a "changed" marker for any
  edit, or only for time and place?

## Architecture analysis

**Existing code touched:**

- `lexicons/`: today it holds only `app/eventside/block/{defs,card}.json`.
  Plans adds the first eventside records outside blocks, and the first
  community lexicons the repo depends on.
  - New eventside lexicons: the plan sidecar, `app.eventside.location.place`,
    the invitation record, and the attendance-visibility field or record.
  - Vendored community lexicons, used as they are: `calendar.event`,
    `calendar.rsvp`, `location.{address,fsq,geo}`.
  - The vision's `childOf`/`includes` tree records, in whichever namespace
    `conference-space` settles on.
- `lexicons/app/eventside/block/defs.json`: the conference-aware blocks
  (`#sessionRef`, `#room`, `#time`, `#person`) are where a plan card would
  live. A plan either renders through `#sessionRef` stretched to cover
  plans, or needs a new `#planRef` block and a `#place` that supersedes or
  generalizes `#room`. The same applies to `web/src/blocks/blocks/conference.tsx`
  and the Rust mirror in `crates/blocks/src/lexicon.rs`.
- `crates/server`: new XRPC endpoints for creating, editing, joining,
  inviting and listing plans, and audience-filtered views over the
  space-sync index. These are the first endpoints that serve another
  member's records under a per-viewer audience check.
- `web/src/routes`: today it has only `dev/`. Plans adds the first attendee
  screens: the composer, the plan page and the Plans list.

**Features affected:**

- [`ui-blocks`](ui-blocks.md) (complete): its conference-aware vocabulary
  gains a plan card, or `#sessionRef` is widened to cover one. `#room` either
  becomes a reference to a `places` record or sits next to a new `#place`
  block. A vocabulary change has to follow its "old clients skip unknown
  blocks" rule and keep its gallery tests green.
- [`block-actions`](block-actions.md) (`analysis`): its write path makes every
  interaction an `app.eventside.block.action` record that middleware checks.
  An RSVP here is a `community.lexicon.calendar.rsvp`, a different
  collection. Either block-actions grows to write typed records (an action
  whose accepted result is a community record), or plans writes RSVPs
  through its own endpoint. That changes block-actions' scope, and its
  ingest re-validation has to cover RSVPs and invitations written straight
  to the PDS.
- [`space-sync`](space-sync.md) (`analysis`): plans is the first consumer
  that needs the index to serve records filtered by audience, and the first
  to need community-namespace collections indexed. Its read API has to
  support "records in collection X that point at node Y".
- [`attendee-sign-in`](attendee-sign-in.md) (being built): writing plans,
  RSVPs and invitations into the space as the attendee needs a `space:`
  write scope added to the scope list. That's the mechanism its design
  reserved for later features, and existing sessions will be asked to sign
  in again.
- `conference-space` (not specced): plans hang off its conference node with
  `childOf`, and featuring needs its notion of who counts as an organizer
  (co-owners).
- `conference-feed` (not specced): it has to carry plan items with the
  reasons "changed", "invited you" and "featured", pins for featured
  whole-conference plans, and the rule that past plans drop out.
- [`groups`](groups.md) and [`places`](places.md) (not specced): plans
  defines how it references them (a group URI in an audience, a place URI
  in a location), which sets their public shape before they're
  brainstormed.
- `chat` (not specced): it embeds plan cards, and each plan has a chat.
- `program-import` (not specced): sessions are also `calendar.event` nodes,
  and the Going/schedule view mixes them with plans. Rooms it imports become
  `places`.
- `connections` (not specced): the source of the "my connections" group.

**New shared surfaces:**

- The **node model in practice**: `calendar.event` plus sidecar, `childOf`
  and `includes`. Sessions, plans and later meetups all share it.
- **RSVP handling**: the community RSVP record, the public/private
  attendance choice, and the counts and lists the appview serves. Sessions
  will reuse it.
- **Audience-filtered serving**: the appview check "may this viewer see
  this node?", which feeds, chat and Going all rely on.
- **The location union**, including `app.eventside.location.place`.
- **Invitations** as audience grants, which chat invites may reuse.

**Verdict: cross-cutting.** Plans adds lexicons and record shapes: the first
community lexicons, the plan sidecar, and the location, invitation and
featuring records. It sets the node, RSVP and audience-filtering model that
sessions, feeds, chat and groups all build on. It changes what two specced
features have to provide: block-actions' write path and space-sync's read
API. It also extends a built one, ui-blocks' conference vocabulary, and adds
a scope to attendee-sign-in. It needs a design review.

## Design review

## Test cases

## Review log
