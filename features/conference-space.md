---
status: design-review
impact: cross-cutting
depends-on: [attendee-sign-in]
branch:
tests-commit:
---

# Conference space

## Summary

A conference is an atproto permissioned space owned by an **organization
account**, an identity separate from any one person. Attendees are its
members. A conference is public on the outside by default, or invite-only,
and members-only on the inside. Every later feature hangs off it: the
program, plans, chat, groups, feeds and event profiles.

For the November 1 demo, conferences are set up and administered by a
script. There are no admin screens in the app.

## Experience

**Finding a conference (public):**

1. Someone opens a conference's link. The link is the AT-URI of a public
   `community.lexicon.calendar.event` record, or a URL for one.
2. They see its public page: name, dates, city, description and branding.
   Other atproto calendar apps can show the same event.
3. The page shows how to get in, depending on which join methods are on:
   - enter an invite code
   - "Request to join"
   - "Join", if the conference is open
   - "Sign in", if they're on the attendee list

**Finding a conference (invite-only).** For events such as a destination
wedding, nothing is public. Someone with an invite link or code sees a
sign-in and join prompt. Anyone else sees nothing.

**Joining.** The attendee signs in (see
[`attendee-sign-in`](attendee-sign-in.md)) and is admitted by any of the
methods the organizer has turned on:

- **Invite link or code:** one shared code ("atmosphere27"), or personal
  codes.
- **Attendee list:** the organizer imports a list from their ticketing
  tool. People on it get in when they sign in, matched by atproto handle or
  by verified email. Email matching asks the person's PDS for their email
  (the OAuth `transition:email` scope) at join time.
- **Request and approve:** the attendee asks, and an owner or staff member
  approves.
- **Open:** anyone signed in can join.

Once admitted, they see the inside of the conference.

**Inside.** Only members can open the conference's inside. Members read
only their own records, and the appview serves everything else, as
[`space-sync`](space-sync.md) already decided.

**Leaving.** An attendee can leave a conference. What they wrote stays in
their repo, but the appview stops serving it to others. They can rejoin
through any method that admits them.

**Organizers.** Each organization is its own atproto account. Its owners
sign in as themselves, and the appview acts as the organization for them,
so nobody shares a password, and owners can be added or removed. One
organization can run several conferences.

**Roles:**

- **Owner:** full control, including adding and removing owners.
- **Staff:** the same admin powers, except managing owners.
- **Speaker:** an attendee linked to their sessions, usually set by
  `program-import`.
- **Attendee.**

**Administration for November 1.** A script or CLI does all of it:

- create the organization account and its conferences
- add owners and staff
- choose the join methods and the public or invite-only setting
- import attendee lists, and issue codes
- approve or deny requests
- remove or ban members, and change roles

A banned person is removed and can't rejoin by any method.

**Done.** Two differently branded conferences, run by different
organizations, are seeded by the script.

- One attendee joins each by a different method.
- They see the public page before joining and the inside after.
- A non-member can't see the inside.
- Removing or banning someone cuts off their access.

## Data

- **The organization** is an atproto account (a `did:plc`, on our PDS or
  any PDS). The appview holds a session for it and writes as it, on behalf of
  owners who are signed in as themselves.
- **The conference's entry point** (public conferences) is a public
  `community.lexicon.calendar.event` record in the organization's repo. It
  can be one that already exists, such as one published through another
  calendar app. An eventside sidecar record links it to the space and holds
  the conference's settings: join methods, public or invite-only, and theme
  (for [`event-branding`](event-branding.md)).
- **Invite-only conferences** have no public event record. Their entry point
  is an invite link that names the space.
- **The space** is a permissioned space owned by the organization:
  - one space per conference
  - the conference's sessions, plans, chat and everything else inside it
    are records in that space
  - which apps can read it is the organizer's choice (a curated allow-list
    or open), and every member can read everything in it through those apps
    (revised in design review round 2; this replaced "only the appview reads,
    members get `read_self`")
- **Membership and roles** are the space's member list, plus a role for each
  member, recorded by the organization.
- **Attendee lists, codes, requests and bans** are kept by the organization,
  where attendees can't read them. Whether that's records in the
  organization's repo inside the space, or rows in the appview's database,
  is for the architecture analysis.
- **Emails** from list imports, or from `transition:email`, are used only for
  matching and are never served to other members.

## Options considered

### The organizer identity

- **Its own account, with the appview acting for owners** (chosen). Owners
  sign in as themselves, and nobody shares a login.
- An organization account whose credentials the owners share: the shared
  login the vision rules out.
- A person's account owning the conference: the event would be tied to one
  person.

### How attendees get in

- **Any combination of invite code, attendee list, request and approve, and
  open** (chosen).
- One method per conference: simpler to explain and test.

### Matching an attendee list

- **By handle or by email, whichever the row has** (chosen).
- By email only, through `transition:email`: ticketing exports have emails,
  but it adds a scope at join time.
- By handle only: ticketing tools rarely have handles.

### Roles for the MVP

- **Owner, staff, speaker, attendee** (chosen).
- Sponsor: deferred. It was demo item 4's "sponsor data sharing", which now
  waits for a later feature.

### Administration

- **A script or CLI for everything, for November 1** (chosen).
- Admin screens in the PWA: a stronger demo of the organizer side, but more
  to build.
- Scripted setup plus a small Members screen in the PWA (approve, remove,
  ban, change role).

### What non-members see

- **It depends on the conference. Public by default, with the entry point a
  public community calendar event; invite-only conferences show nothing**
  (chosen).
- Always a public page.
- Never anything until you're in.

### Leaving

- **Allowed, and the attendee's records stay theirs** (chosen).
- Allowed, and their records are deleted.
- Not in the MVP: membership would end only when an owner removed it.

## Out of scope

- Admin screens in the app.
- The sponsor role, and sponsor data sharing.
- Billing, tiers and counting active participants.
- Per-conference onboarding (code of conduct, event profile):
  `event-profile`.
- Configuring branding: [`event-branding`](event-branding.md).
- Signing up on our own PDS at registration.
- Group, plan and ballot spaces themselves: this feature sets the model
  (one space per audience), and `groups`, `plans` and `polls` build them.

## Open questions

- Should a public `community.lexicon.calendar.rsvp` of "going" to the
  conference's event, written from any calendar app, count as a way to join
  (when the conference is open), or as a request?
- When the conference attaches to an event record that someone else
  published, is that record's author the organization, or can an
  organization adopt an event it didn't write?
- How does the appview get and keep the organization account's session? An
  owner signs in as the organization once, as `space-sync`'s "organizers
  connect once" assumes, or the script provisions it.
- Where do attendee lists, codes, requests and bans live: in the space as
  organization records, or in the appview's database?
- How does an organization's owner list change hands safely, e.g. stopping
  the last owner from removing themselves?

## Architecture analysis

**Existing code touched:**

- None on `main` yet works with spaces: `crates/server` is a skeleton, and
  ui-blocks shipped with fixture sources only.
- `attendee-sign-in` (on its branch) adds the appview's sessions, OAuth
  client and scope list in `crates/server/src/{auth,oauth,config}.rs`.
  Conference-space builds on them in two ways:
  - acting as the organization account needs a session the appview keeps
    for an account nobody is browsing as
  - matching by email needs `transition:email`, asked for at join time
    (step-up), which sign-in's design deferred
- `lexicons/`: new eventside records (the conference sidecar, roles,
  membership-related records) and the vendored
  `community.lexicon.calendar.event`, shared with [`plans`](plans.md).
- `scripts/` and `tests/support/`: the admin CLI is new. Test support gains a
  "seed a conference" helper that later features' tests will use in place
  of creating raw spaces.

**Features affected:**

- [`space-sync`](space-sync.md) (`analysis`): it assumes "organizers
  connect a space once by signing in with a `read` grant" and "tests
  create spaces directly". Conference-space settles who connects (the
  organization account, through the appview) and how spaces are created
  and configured: app allow-list, `read_self`, and member policy. It also
  makes membership changes (leave, remove, ban) something space-sync has
  to act on: a former member's records stop being served.
- [`attendee-sign-in`](attendee-sign-in.md) (being built): it needs an
  appview-held session for an organization account, and step-up for
  `transition:email` at join time. Its design says "no step-up for now",
  so this either adds step-up or asks every attendee for the email scope
  at sign-in.
- [`ui-blocks`](ui-blocks.md) (complete): test support creates a space
  directly with a member-list policy. Its tests don't change, but the
  "real conferences create their spaces in conference-space" handoff it
  recorded happens here.
- [`block-actions`](block-actions.md) (`analysis`): its writes and views are
  scoped to a space. Space roles (owner, staff) are what later card
  authoring checks against.
- [`plans`](plans.md) (`design-review`): plans hang off the conference node
  with `childOf`, featuring needs the owner and staff roles, and "whole
  conference" means the space's members.
- Not yet specced: `program-import` (sessions and speakers, which sets the
  speaker role), `event-branding` (the theme on the sidecar),
  `conference-feed`, `groups`, `places`, `chat`, `connections` and
  `event-profile`. All of them read membership and roles from here, and
  scope their records to the conference's space.

**New shared surfaces:**

- The **conference model**: a public `calendar.event` (or an invite-only
  link) as the entry point, a sidecar linking it to its space, and one space
  per conference.
- **Membership and roles**: every feature's permission checks ask "is this
  DID a member, and with what role?".
- **Acting as the organization**: the appview's path for writing as an
  organization on behalf of an owner.
- **The admin CLI and the test seeding helper.**

**Verdict: cross-cutting.** Conference-space sets the foundations every
later feature builds on. It adds lexicons and record shapes: the conference
sidecar, roles and the join-related records. It defines the membership and
role checks, and the appview's organization identity. It changes what
`space-sync` (who connects, membership changes) and `attendee-sign-in`
(an organization session, and the email scope or step-up) have to provide.
It needs a design review.

## Design review

### Design (revised in round 2)

A first draft was critiqued by a fresh reviewer, and this version folds in
what held up. The changes are listed under "Critique folded in" at the end.

#### What vivarium supports

These facts were read from the `@vivarium-dev/cli` binary, first from 0.0.1
and then again from **0.0.2** (PR #6), which catches up with the upstream
permissioned-data proposal. The 0.0.2 facts:

- **Space URIs** have the form `at://{ownerDid}/space/{type}/{skey}`. The
  owner must be an account on the PDS hosting the space, so the appview's
  `did:web` can't own one.
- **`com.atproto.simplespace.*` is owner-only**, and OAuth sessions need
  `manage=update` for member changes:
  - `createSpace {spaceType, skey?, readPolicy, writePolicy, appAccess}`,
    with every field required; the owner is the caller
  - `updateSpace`, `deleteSpace`
  - `putMember {space, did, read, write}`, an upsert (it replaces 0.0.1's
    `addMember`)
  - `removeMember {space, did}`
  - `listMembers`, which returns `{did, read, write}` entries and is
    owner-session only
  - `getSpace`
- **Policies** are `#publicPolicy`, `#memberListPolicy` and
  `#managingAppPolicy`, set separately for reading and writing. An unknown
  policy denies.
- **App access** is `#open` or `#allowList`. Under `#allowList`,
  `getSpaceCredential` needs a client attestation whose `iss` is on the
  list.
- There are **no invites, join requests, roles or bans**.
- **What the PDS enforces:**
  - the owner always passes
  - writes into a user's own space repo are accepted whatever their
    membership
  - but the space host records and fans out a write only if its author has
    `write` under the write policy, so a non-member's writes never reach
    `listRepos`
  - `removeMember` is a plain delete: credentials already issued (now 10
    minutes, at most 1 hour) keep working until they expire
  - `notifyCredentialRevoked` exists for revoking issued credentials, but
    only the space's manager can send it
- **OAuth scopes** (unchanged from 0.0.1):
  - `space:{type}?authority=…&action=…&collection=…&manage=…`
  - `authority` defaults to `self`
  - a missing `action` means every action except `read_self`
  - every requested scope must appear verbatim in the client metadata
  - a delegation token (for `space-sync`) needs `action=read`
- **Space credentials changed** in 0.0.2: they're HTTP-message-signature
  bound to a P-256 `did:key` (not DPoP), last 10 minutes, and are needed for
  `registerNotify` and `listRepos`. That's `space-sync`'s concern, recorded
  under Impact.
- **Email:** `transition:email` and `account:email` are accepted.
  `getSession` returns the email whatever the scopes, so tests can't prove
  the scope is needed.
- **Vivarium is one PDS.** An organization or attendees on other PDSes go
  untested.
- **No space lexicons ship** with vivarium's npm packages. Field names come
  from its handler code.

#### Approach

**Access control lives in the space, not in our appview.** Eventside is one
app among several that may read a conference's records, so the rules any
app has to respect are expressed in atproto itself:

- **which space a record is written into** decides who can read it: every
  member with `read` can read everything in a space, through any app on the
  space's app list that they use
- **the space's policies and member list** (`read` and `write` flags) decide
  who is in
- **role and rules records** in the space say who may do what inside it
  (see "Roles and rules")

There is no privacy *inside* a space beyond that. Anything that needs a
narrower audience goes in its own space. Our appview still serves views and
aggregates, but that's a convenience, not the guarantee.

**One space per audience.** Each space type is its own OAuth consent
boundary, so people see what an app wants to read or write:

| Space type | Audience | Owner | Policies | Specced in |
|---|---|---|---|---|
| `app.eventside.conference` | everyone admitted to the conference | the organization | read and write: `#memberListPolicy` | this feature |
| `app.eventside.group` | a group: an organizer's list (speakers, volunteers), a plan's audience, a derived group (going to X, my connections), or an attendee's personal list ("cool cats I've met") | the organization, except personal lists, which the attendee owns | explicit lists: `#memberListPolicy`; derived groups: `#managingAppPolicy` pointing at eventside | `groups` |
| `app.eventside.ballot` | an anonymous poll's voters: they can write their ballot, and only the organization can read | always the organization | write: the voters (synced `putMember {read: false, write: true}` or `#managingAppPolicy`; `#publicPolicy` at a public event); read: the owner only | `polls` (later) |

A plan, a chat or a card is written into the space of its audience: the
conference space for the whole conference, or a group's space for a
group. A plan for picked people gets a small group space of its own
(`plans` and `groups` settle the details).

**Derived groups are the one exception to "rules live in the space".**
Their policy is `#managingAppPolicy`, so the PDS asks eventside's
`checkUserAccess` (with the asking app's client ID, for reads). That puts
the decision back in our code, makes derived groups unavailable to every app
while eventside is down (failures and the 5-second timeout mean "denied"),
and needs the appview's `did:web` to publish a service endpoint for it. To
keep it inspectable, the group space carries a record describing the
derivation (e.g. "going to `at://…`"), so other apps can see what the rule
is. `groups` designs it.

**Who owns which spaces** (decided in round 2: split by kind):

- **The organization owns** the conference space, organizer lists, plan
  audiences, derived groups and every ballot space. Eventside creates them
  with the organization's session, so `ORG_SCOPES` covers
  `app.eventside.group` and `app.eventside.ballot` (`manage=create`,
  `manage=update`, `action=read`, and writes). The conference's app-access
  setting applies to all of them, except ballots (below).
- **Attendees own their personal lists.** A "cool cats I've met" list is an
  `app.eventside.group` space on the attendee's own PDS, so it's private from
  the organizer. That adds `space:app.eventside.group?manage=create` and
  `?manage=update` (with the default `authority=self`, so only their own
  spaces) to `LOGIN_SCOPES`, which every attendee is asked for. Their PDS has
  to host spaces, and the organizer's app-access setting doesn't reach these
  spaces: they're allow-listed to eventside by default, and changing that is
  the attendee's choice (a later feature). `groups` designs the details.

**Anonymous polls** use the ballot pattern. "Anonymous" means **hidden from
other participants**, not from the organization or eventside: the ballot
space's owner can see who voted what, because a ballot is a record in the
voter's repo. So ballot spaces are always owned by the organization and
allow-listed to eventside alone, whatever the conference's app-access
setting. Readers count one ballot per DID. In small polls, comparing tallies
over time can reveal votes; polls should say so. The rest is for `polls`: voters are write-only members
(`putMember {read: false, write: true}`), or anyone can write under
`#publicPolicy`. No voter, and no voter's app, can read anyone else's
ballot. The owner's app tallies the ballots and publishes only the result,
as a record in the poll's audience space. This replaces the earlier
"members hold `read_self`, only our appview reads" model for anonymous
votes and Q&A (see Impact).

**Which apps can read.** Each conference's sidecar records the
organizer's choice, applied to all of the conference's spaces:

- **curated** (the default): `#allowList`, starting with eventside's bare
  client ID; `eventside admin apps add|remove <client-id>` edits it. Until
  the organizer adds another app, only eventside can read, so other apps get
  nothing by default. That's deliberate: opening up is the organizer's
  decision.
- **open**: `#open`, so any app a member authorizes can read everything in
  the conference's spaces, including an invite-only conference's event and
  sidecar inside the space. The CLI warns before switching.

Switching mode calls `updateSpace` on every space the organization owns for
that conference.

**The conference space's member list is the source of truth for access,
and membership records make it visible.** The PDS uses the member list to
decide who gets credentials and whose writes it records. But only the owner
can call `listMembers`, and a removed member stays in `listRepos`, so other
apps can't see the list or its history. The organization therefore also
writes **membership records** into the space:
`app.eventside.conference.member {subject, since, until?}`, one per
membership period. Any reader honors them: a record is shown only if its
author had a membership period covering the moment the space host recorded
it (its `spaceRev`).

Both are written on every change, in this order:

1. `putMember {read: true, write: true}` or `removeMember`, as the
   organization. If it fails, the join or removal fails and is retried.
   Nothing is reported as done until the PDS has it.
2. The membership record: created on join, given its `until` on leave,
   removal or ban.
3. The appview's `members` and `member_periods` tables, an **index** of the
   two, so our own checks need no network round trip.

`eventside admin members reconcile` (also run at startup) rebuilds the
index from `listMembers` and the membership records, and repairs a missing
record or `until`.

Codes, attendee lists, requests and bans stay in the appview's database:
they're secrets, or decisions about who to add, not rules other apps need.
A ban is enforced by never calling `putMember` for that DID again.

**Removal and other apps.** `removeMember` stops new credentials, but a
credential another app already holds lasts up to 10 minutes. We can't send
`notifyCredentialRevoked` for credentials we didn't issue, so that window is
accepted and documented. Our own appview stops serving a removed member at
once.

**Roles and rules** are records the organization writes into the
conference space, so every member's app sees the same rules:

- `app.eventside.conference.role {subject: did, role: owner | staff |
  speaker}`, one record per person with a role. Attendees have none.
  Removing or banning someone deletes their role record.
- `app.eventside.conference.rules`, one record (rkey `self`), which says
  for each record type who may write it in this space, e.g.:
  - cards and announcements: `owner`, `staff`
  - plans, chat messages, RSVPs: any member
  - roles and rules: only the organization's own repo

  Spaces can't restrict writes by collection, so these rules are enforced
  by readers: an app that honors them ignores records that break them, as
  our appview does at ingest. The rules record makes that explicit and the
  same for every app. Reader rules:
  - roles, rules and membership records count only from the space owner's
    repo
  - a record is judged by the rules, roles and membership in force when the
    space host recorded it (its `spaceRev`), never by its own `createdAt`,
    which the author sets; later rule changes don't retroactively hide it
  - the rules and membership lexicons are published with the others
  - finer rules, such as `block-actions`' middleware (one vote each, closing
    times), aren't in the rules record. They're eventside's own, and other
    apps may count actions eventside rejected. A card can say which
    middleware applies; that's for `block-actions` to decide.

Owners and staff of the organization get a role record in each of its
conferences and are added as members.

**No in-process caches or broadcasts.** The admin CLI is a separate
process sharing the database. So every check reads the database directly,
and other features (`space-sync`, `groups`, feeds) query membership when
they need it rather than subscribing to events. A CLI removal takes effect
on the next request.

**The organization session is a separate kind of session.**
`attendee-sign-in` changes in these ways:

- **Pending requests get a purpose:** `login`, `org` or `email`. `/oauth/login`
  only ever starts `login`, so `org` and `email` can't be started from the
  browser's ordinary sign-in.
- **`org` requests** are started by `eventside admin org connect <handle>`,
  which prints the authorization URL. They're pushed under the client ID for
  `ORG_SCOPES`. Their callback:
  - stores a session row with `kind = organization` and **no cookie**
  - shows a "connected, you can close this tab" page
  - `CurrentUser` refuses `organization` rows, so nobody can act as the
    organization in the PWA
- **`outdated()` is per kind.** Organization sessions are compared with
  `ORG_SCOPES`, and attendee sessions with `LOGIN_SCOPES`. An organization
  missing a newly added `ORG_SCOPES` entry is marked `reconnect needed`,
  which the CLI reports. It isn't ended silently.
- **Organization sessions never idle out.** The renewer keeps them alive.
  Only a refused refresh ends one, and that's reported the same way.

**`ORG_SCOPES`:**

- `atproto`
- `space:app.eventside.conference?manage=create` and
  `space:app.eventside.conference?manage=update` (no `manage=delete`: no
  command needs it, and an appview compromise shouldn't be able to delete
  conferences)
- `space:app.eventside.conference?action=read`, for `space-sync`'s
  delegation
- `space:app.eventside.conference?action=create&action=update&action=delete`
  for the organization's own records in the space (invite-only conferences)
- `repo:community.lexicon.calendar.event` and `repo:app.eventside.conference`,
  for the public event and sidecar
- `blob:*/*`, which `event-branding` will need for logos (included now so
  organizations don't have to reconnect)

**Sharing the organization's login.** Connecting needs someone who can sign
in as the organization once, at its PDS. For the MVP, that's the operator
running the script, who created the account. That's a deliberate departure
from "nobody shares a password". After `org connect`, nobody needs the
login again day to day, and owners act through the appview. A proper
handover (the PDS's own account recovery, or an organization's own
delegation) is out of scope.

**The appview's client ID on allow-lists** is always the bare
`…/oauth-client-metadata.json`, the `atproto`-only client ID. Space client
attestations are always signed as that client ID. So a change to
`LOGIN_SCOPES` or `ORG_SCOPES` never locks the appview out of existing
spaces. A unit test checks the attestation's `iss`.

**A conference is one space, entered through a calendar event.**

- The space is `at://{orgDid}/space/app.eventside.conference/{skey}`,
  created with `readPolicy` and `writePolicy` both `#memberListPolicy`, and
  `appAccess` per the organizer's choice (curated `#allowList`, starting with
  eventside's bare client ID, or `#open`).
- **Public conferences:**
  - The organization's public repo holds a `community.lexicon.calendar.event`
    and an `app.eventside.conference` sidecar with the same rkey.
  - The sidecar holds the space URI, `visibility: public`, `appAccess`
    (`curated` or `open`), the join methods that are on (as flags only: no
    codes, no lists) and the theme reference.
  - `conference create --event <at-uri>` attaches to an existing event,
    such as one published through another calendar app, as long as it's in
    the organization's repo. Without `--event`, it writes a new one.
- **Invite-only conferences** write the event and sidecar inside the space,
  in the organization's repo, so nothing is public. The appview reads them
  with the organization's own session (its own repo), so it needs no space
  credential.
- **Identity:**
  - The **conference node** is the event's AT-URI. Plans' `childOf` and
    feeds point at it, and public links use it.
  - The **space URI** is the access boundary every membership check uses.
  - `conferences` maps between the two, and `getConference` accepts either.

**Joining.** `app.eventside.conference.join {conference?, code?}` follows
this order:

1. **Banned** (by DID) → `refused`.
2. **Already a member** → `joined`.
3. **On the attendee list or pre-assigned a role**, by DID → `joined`, with
   the role.
4. **A valid code** → `joined`. A personal code is bound to the DID that
   first uses it, and works again for that DID, e.g. after leaving.
5. **Open** → `joined`.
6. **The list has email rows** and the person's verified email isn't known
   → `emailNeeded`, with `canRequest` set when requests are on. The PWA
   offers "Verify my email" or "Request to join".
7. **Requests are on** → `pending`.
8. Otherwise → `refused`.

`conference` can be left out when `code` is given: a code identifies its
conference. That's how an invite-only conference's link works. The link is
`/join/{code}`, and a code typed by itself works the same way.

**Matching the attendee list:**

- Handle rows are resolved to DIDs when they're imported. A handle that
  doesn't resolve yet is kept and retried at join time, and bound to the
  DID it first resolves to. A handle transferred later doesn't carry the
  place on the list with it.
- Email rows are stored as `HMAC-SHA256(server key, lowercased email)`, never
  in plain text.
- **The email step.** `emailNeeded` → the PWA starts an `email` sign-in, with
  `LOGIN_SCOPES` plus `transition:email`, under that list's client ID. The
  callback:
  - requires `emailConfirmed === true`
  - HMACs the email and matches it
  - doesn't store the email
  - retries the join
- **The grant keeps `transition:email` afterwards.** OAuth has no partial
  revoke, and revoking the whole grant would sign the person out. This is
  documented, and the appview never calls for the email again.

**Leaving, removing, banning:**

- **Leave or remove:** `removeMember` runs first, then the membership record
  gets its `until`, then the index row ends. If the PDS call fails, the
  request fails and is retried, and nothing changes until it succeeds.
- **Ban:** remove, plus a ban row keyed by DID.
- **What a former member sees:** nothing from any conference endpoint, from
  the next request on.
- **Rejoining:** whatever the person wrote while they weren't a member
  stays unserved. `space-sync` records the membership periods and serves a
  record only if it was written while its author was a member.

**Abuse limits on `join`:**

- 10 attempts a minute per DID, and 30 per IP.
- Personal codes carry at least 80 bits of randomness.
- Shared codes can have an expiry and a usage cap (`codes issue --shared
  --expires … --max-uses …`).

**Organizations, owners and roles:**

- **Owners and staff belong to the organization:** the `org_members` table
  (`owner` or `staff`). They're also admitted as members of each of the
  organization's conferences, with a role record there.
- **Speakers** get a role record in their conference; attendees have none.
  The appview indexes role records into `members.role`.
- **Pre-assigned roles:** `program-import` (or the CLI) can give a DID or
  handle a role, e.g. `speaker`, before the person joins. That works like
  an attendee-list row: it admits them, with the role, when they join.
- **The CLI's operator is trusted.** It runs on the server host with the
  database. The owner/staff difference only matters to future admin
  screens. `org owner remove` still refuses to remove the last owner, as a
  safety check, not a security boundary.

**Admin CLI** (a subcommand of the server binary, sharing its database
and organization sessions):

- `org create-record` and `org connect <handle>`
- `org owner add|remove`, `org staff add|remove`
- `conference create [--event <at-uri>] [--invite-only]`, which creates the
  space and writes the sidecar (and the event, if not attached)
- `join set --code --list --request --open`
- `codes issue [--personal <did|handle>…] [--shared --expires --max-uses]`
- `list import <csv>` (columns: handle, email, role?)
- `requests list|approve|deny`
- `member remove|ban|role` (`role` writes or deletes the role record)
- `members reconcile`
- `apps add|remove <client-id>`, `apps open|curate`
- `rules set <file>` (writes the rules record; `conference create` writes a
  default)

**Deferred:** whether a public `community.lexicon.calendar.rsvp` of
"going" to the conference's event should count as a way to join. RSVPs
from other apps are ignored for now.

#### Components

- **`crates/server/src/conference/`** (new):
  - `mod.rs`: the `Conference` model, and loading by event or space URI
  - `membership.rs`: membership changes (PDS first, then the index), the
    `members`, `org_members` and pre-assigned role tables, role records,
    and the role checks
  - `rules.rs`: the rules record (default, parsing) and
    `may_write(space, did, collection)` for ingest
  - `join.rs`: the join order, codes, list matching and rate limits
  - `routes.rs`: XRPC
  - `admin.rs`: the CLI
- **`crates/server/src/auth/`** (from `attendee-sign-in`, changed):
  - `oauth_requests.purpose`
  - `sessions.kind`
  - `ORG_SCOPES`
  - `outdated()` per kind
  - no idle timeout for organization sessions
  - `CurrentUser` refuses organization sessions
  - the `email` callback path
- **`crates/server/src/space.rs`** (new, shared):
  - a client for `simplespace.*` and `space.*` writes
  - space URI parsing
  - client attestation, always as the bare client ID
  - `space-sync` adds the read and notify side
- **PWA:**
  - `routes/c/$actor/$rkey.tsx`: the public conference page, where
    `$actor` is a DID or a handle (links use the DID)
  - `routes/join/$code.tsx`
  - the join panel: code entry, request, "verify my email", and the states
    pending, refused and banned (the reason isn't shown)
  - a minimal shell for members
- **Test support:**
  - `tests/support/conference.ts`: `seedConference({visibility, join,
    owners, list?, codes?})` creates the organization account through
    vivarium, connects it through the real OAuth flow, and runs the CLI
    commands
  - `joinAs(page, handle, conference, {code?})`

#### Data

| Item | Where | Written by | Read by |
|---|---|---|---|
| `community.lexicon.calendar.event` | org repo: public, or inside the space if invite-only | the appview as the org (or already existing) | anyone if public; the appview |
| `app.eventside.conference` sidecar: `space`, `visibility`, `appAccess` (`curated` or `open`), `join {code, list, request, open}`, `theme?` | next to the event, same rkey | the appview as the org | anyone if public; any app |
| The space, its policies, app access and member list (**the source of truth**) | the org's PDS | the appview as the org | the PDS; every app on the app list |
| `app.eventside.conference.role {subject, role}` | org repo, inside the space | the appview as the org | every member's apps |
| `app.eventside.conference.member {subject, since, until?}` | org repo, inside the space | the appview as the org | every member's apps (whose records to show) |
| `app.eventside.conference.rules` (rkey `self`): per collection, who may write | org repo, inside the space | the appview as the org | every member's apps (enforced at ingest) |
| `conferences`: space URI, event URI, org DID, settings | appview DB | CLI | everything (no cache) |
| `org_members`: org DID, DID, `owner` or `staff` | appview DB | CLI | role checks |
| `members`: space, DID, role (indexed from role records), joined via, joined and left at | appview DB, an index of the space's member list | join, leave, CLI, reconcile | every feature's checks |
| `member_periods`: space, DID, from, to | appview DB | join, leave, CLI | `space-sync`'s serving filter |
| `preassigned`: space, DID or handle, role | appview DB | CLI, `program-import` | join |
| `invite_codes`: hashed code, space, personal DID or handle?, expiry, max uses, uses | appview DB | CLI | join |
| `attendee_list`: space, DID or unresolved handle, email HMAC | appview DB | CLI import | join |
| `join_requests`, `bans` (by DID) | appview DB | join, CLI | join, CLI |
| `sessions.kind`, `oauth_requests.purpose` | appview DB (sign-in tables) | sign-in | sign-in, renewer |

#### Interfaces

- **XRPC:**
  - `app.eventside.conference.getConference {uri}`: `uri` is the event URI
    or the space URI. It returns the public view, plus `viewer {member,
    role?, pending?, banned?}` and the join methods open to this viewer. An
    invite-only conference returns not-found to anyone who isn't a member.
  - `join {conference?, code?}` returns `{status: joined | pending |
    emailNeeded | refused, canRequest?}`.
  - `leave {conference}`
  - `listMyConferences`
  - Writes need the session cookie and the CSRF token.
- **Rust, for later features** (all reading the database, no caches):
  - `membership::role(&db, &space, &did) -> Option<Role>`, where `Role` is
    owner, staff, speaker or attendee
  - `require_member(space)` → `CurrentMember {did, role}`
  - `members(&db, &space)`
  - `was_member_at(&db, &space, &did, time)`, for `space-sync`
  - `org_client(&org_did)`, a PDS client acting as the organization, for
    `program-import` and `event-branding`
  - `rules::may_write(&db, &space, &did, collection)`, for ingest in
    `space-sync` and `block-actions`
  - `space::create(owner_client, type, policies, app_access)` and
    `space::put_member` / `remove_member`, reused by `groups` and `polls`
    for their own space types
  - `preassign(&space, actor, role)`, for `program-import`
- **Tests:** `seedConference` and `joinAs`, plus the `eventside admin` CLI
  through `spawnServer`'s binary.

#### Impact on existing features

- **[`attendee-sign-in`](attendee-sign-in.md)** (being built; this design
  lands after it merges):
  - pending requests get a `purpose`, sessions get a `kind`, and
    `outdated()` and idle handling are per kind
  - the organization callback is cookieless, and `CurrentUser` refuses
    organization rows
  - the `email` callback path
  - its existing tests keep passing: `login` behaves exactly as now
- **[`space-sync`](space-sync.md)** (`analysis`): the dependency is
  reversed, so **space-sync depends on conference-space**. Its premise
  changes: the appview is no longer "the only client allowed to hold space
  credentials", and members no longer get only `read_self`. It's one reader
  among several, and it honors membership and rules records. Its inputs from
  here:
  - the list of spaces, from `conferences`
  - the organization session, which holds `action=read` for delegation
  - attestation as the bare client ID
  - the serving filter `was_member_at`, and `rules::may_write` at ingest
  - "tests create spaces directly" becomes `seedConference`
  - it syncs every eventside space type, not only conference spaces
  - vivarium 0.0.2 changes its credential flow: HTTP message signatures
    with a P-256 `did:key`, 10-minute credentials, `registerNotify` and
    `listRepos` needing a credential, and `repoRev`/`spaceRev` in
    notifications and `listRepos`. Its design should be written against
    0.0.2
- **[`ui-blocks`](ui-blocks.md)** (complete): no code change. Its fixture's
  `ats://…` space strings are opaque test data in frozen files. Its chosen
  privacy model ("members hold `read_self`; only our appview reads
  everything and serves aggregates") **no longer holds** once other apps can
  read: within a space, every member's apps can read everything. Its
  feature file should record that this was superseded here.
- **The privacy model for anonymous votes and Q&A** (chosen in ui-blocks and
  relied on by `block-actions`): replaced by ballot spaces. Votes or
  questions that must stay anonymous are written into an
  `app.eventside.ballot` space where voters are write-only, and only the
  aggregate is published. `block-actions`' design must use that pattern for
  anonymous actions (its `actionTally` view still works, computed from the
  ballot space by the owner's app).
- **[`block-actions`](block-actions.md)** and **[`plans`](plans.md)**:
  - attendee writes into an organization's space need
    `space:{type}?authority=*` scopes in `LOGIN_SCOPES`, with explicit
    `action=create&action=update&action=delete` and a `collection=` per
    record type (a missing `action` would include read)
  - they gate with `require_member`, and ingest checks `rules::may_write`
  - "whole conference" means the conference space; narrower audiences
    write into a group space (`groups`)
  - plans' `childOf` points at the event URI
  - **`plans` needs revising:** "who sees what is enforced by the appview"
    no longer holds; a picked-people audience needs its own space; and its
    per-RSVP "public or private attendance" means nothing when every member
    can read every RSVP in the space (it becomes "which space the RSVP is
    written into", or is dropped)
  - **`block-actions`:** its accepted-actions table is eventside's
    judgement, not a rule other apps see
- **Not yet specced:**
  - `program-import` uses `org_client` and `preassign`
  - `event-branding` writes the sidecar's `theme`, with `blob:` already in
    `ORG_SCOPES`
  - `groups` owns `app.eventside.group` spaces, including derived groups
    under `#managingAppPolicy` (the appview answers `checkUserAccess`)
  - `polls` (later) owns `app.eventside.ballot` spaces
  - `chat` and feeds use `require_member`, and write into the audience's
    space

#### Alternatives

- **Our appview as the only reader, with its database as the truth**
  (round 1): one allow-listed app, `read_self` for members, and every rule
  in our code. Superseded in round 2: other apps must be able to read, so
  the rules have to live in the space.
- **`#managingAppPolicy` for the conference space**: always current, but
  the decision would live in our code, and every policy check would depend
  on our appview being up. Kept for derived groups only, where membership
  can't be listed ahead of time. (Neither policy lets other apps list
  members; that's what membership records are for.)
- **One space for everything, with visibility by convention:** other apps
  wouldn't honor it. One space per audience puts the boundary in the
  protocol.
- **A space per share** (every plan and chat its own space, even
  whole-conference ones): uniform, but many spaces and member lists to keep
  in step for no gain in privacy.
- **Only the PDS member list, with no index:** a network round trip for
  every check.
- **An in-process `MembershipChanged` broadcast and caches:** wrong across
  the CLI's process boundary (critique #2).
- **The CLI calling admin endpoints on the running server:** this keeps
  caches correct, but needs an admin auth path. Not needed without caches.
- **Codes, lists and requests as records:** they hold secrets, and only the
  appview reads them.
- **Roles in the appview's database only** (round 1): other apps couldn't
  see them.
- **Organization sign-in through `LOGIN_SCOPES`:** every attendee would be
  asked for `manage=` scopes.
- **App passwords for the organization:** they skip OAuth scope checks
  entirely.
- **`transition:email` for everyone at sign-in:** every consent screen would
  ask for an email.
- **`account:email` instead of `transition:email`:** newer, but less widely
  deployed. We use `transition:email` and revisit later.

#### Risks

- **Vivarium (0.0.2) and upstream spaces are alpha**, with no published
  space lexicons. `space.rs` isolates the calls.
- **Rules are enforced by readers, not the PDS.** Spaces can't restrict
  writes by collection, so a member's app can write a record the rules
  forbid. Apps that honor the rules record ignore it; one that doesn't will
  show it. Accepted: the rules record makes the expected behavior explicit.
- **Other apps keep reading up to 10 minutes after removal**, with
  credentials we didn't issue and can't revoke.
- **Open app access is the organizer's call.** Anything in a conference
  space is then readable by any app a member trusts. The admin CLI warns
  when switching to open.
- **Single-PDS tests:** an organization or attendees on other PDSes, and
  attestation across hosts, go untested.
- **Removal:** our appview stops serving a removed member at once, and
  their later writes are never recorded by the space host (no `write`) or
  served by us. Tested at both layers.
- **`authority=*` attendee scopes are broad** (any organization's space of
  that type). This is acceptable because the appview ignores writes from
  non-members, and the scopes name their actions and collections.
- **Email in production:** vivarium returns it regardless of scope. A unit
  test checks that the `email` sign-in asks for `transition:email`.
- **The organization session dying** blocks admin writes and `space-sync`.
  The CLI reports it, and the operator re-runs `org connect`.
- **The index drifting** from the space's member list (e.g. a CLI crash
  between the PDS call and the index write): `members reconcile`, which also
  runs at startup.

#### Critique folded in

A fresh reviewer critiqued the first draft. Changes:

1. **Organization sessions:**
   - pending requests get a purpose
   - the organization callback is cookieless, and `CurrentUser` refuses
     organization sessions
   - `outdated()` and idle handling are per kind
   - the changes needed in `attendee-sign-in` are listed explicitly
2. **The CLI process boundary:** no in-process caches or broadcasts.
   Everything reads the database.
3. **Frozen fixture:** dropped the plan to edit ui-blocks'
   `test-support.tsx`.
4. **Dependency direction:** `space-sync` now depends on this feature, not
   the reverse.
5. **The allow-list and attestation** are pinned to the bare client ID.
6. **Source of truth:** the database is the truth, and the PDS list is a
   retried, reconcilable copy.
7. **Gaps against the description:**
   - organization-level owners and staff
   - a trusted operator
   - the shared organization login as a stated departure
   - pre-assigned roles for speakers
   - `--event` to attach an existing event
   - the RSVP question deferred
   - code-to-conference lookup for invite links
   - membership periods for rejoining
8. **Security:**
   - rate limits, 80-bit personal codes, shared-code expiry and caps
   - `emailConfirmed` required, HMAC'd emails, the email grant documented
   - handles resolved to DIDs
   - no `manage=delete`, and `repo:` and `blob:` scopes added
   - explicit actions and collections for attendee scopes
9. **Smaller points:**
   - the event URI as the node and the space URI as the boundary
   - DID-based routes
   - the single-PDS testing risk
   - `transition:email`
   - the email step offered alongside "request" rather than forced

### Round 1

**Feedback:** the user said vivarium had a new release that catches up with
upstream spaces, to be adopted on a branch (PR #6, vivarium 0.0.2). They
also asked about a "group host" design in progress. No such design was
found under that name; related work found:

- the upstream proposal's space authority/space host split, with
  invites and roles out of scope
- the group-as-its-own-DID pattern (Newbold's community spaces, Ellich's
  opensocial.community, the Certified Group Service, the Arbiter), which
  matches our organization account
- membership handshake records (an invite by the group, a confirm by the
  member) as a portable alternative to roles in the database

The user is looking for the group-host design. It will be checked against
this one when found.

**Changes:**

- "What vivarium supports" rewritten for 0.0.2.
- `addMember` → `putMember {read: true, write: true}`. The portable member
  list now also gates which writes the space host records.
- `createSpace` with explicit `readPolicy`, `writePolicy` and `appAccess`.
- `space-sync`'s impact notes 0.0.2's new credential flow.
- Nothing else in the approach changed: the appview's database stays the
  source of truth, because removal still doesn't revoke issued credentials
  and only the appview holds them.

### Round 2

**Feedback:** the design was approved in outline, with two changes:

1. Assume eventside isn't the only app reading these records, and bake clear
   access-control rules into the space itself.
2. Consider a new space for each kind of share (event, group, and so on).

Asked and answered in this round:

- **App access:** curated allow-list or open, at the organizer's
  discretion.
- **Granularity:** one space per audience, which is effectively one space
  per group. Also needed: a place to record the results of an anonymous poll
  at a public event.
- **Derived audiences** (going to X, my connections): `#managingAppPolicy`.
- **Roles and posting rules:** records in the space.

**After the critique, two more decisions:**

- **Space ownership, split by kind:** organizer lists, plan audiences,
  derived groups and ballots are org-owned; personal lists are attendee-owned
  spaces (attendees get `manage=` scopes for groups only, on their own
  spaces).
- **"Anonymous" means hidden from other participants**, not from the
  organization or eventside. Ballot spaces are always org-owned and
  allow-listed to eventside alone.

**Changes:**

- New principle: access control lives in the space (which space, its
  policies and member flags, and role and rules records). No privacy inside
  a space. Our appview's views are a convenience, not the guarantee.
- Space types: `app.eventside.conference` (this feature),
  `app.eventside.group` (`groups`, including derived groups under
  `#managingAppPolicy`), `app.eventside.ballot` (anonymous polls: voters
  write-only, the owner reads, the result is published).
- App access per conference: curated `#allowList` (default) or `#open`.
- The space's member list is now the source of truth: PDS first, then the
  database index; `reconcile` rebuilds the index. The outbox is gone.
- Roles and a rules record are written into the space by the organization;
  `rules::may_write` enforces them at ingest.
- Impact: ui-blocks' and block-actions' "members `read_self`, only the
  appview reads" privacy model is superseded; anonymous votes and Q&A move
  to ballot spaces.
- From the critique: membership records (`app.eventside.conference.member`)
  so other apps can see who was a member when; reader rules judged by
  `spaceRev`, owner-repo only; derived groups called out as the exception;
  the curated default means only eventside reads until the organizer adds
  apps; impacts on `plans`, `space-sync` and `block-actions` spelled out;
  round 1 contradictions removed.

## Test cases

## Review log
