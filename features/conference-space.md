---
status: design-review
impact: cross-cutting
depends-on: [attendee-sign-in, space-sync]
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
  - the app allow-list admits only the appview, and members get
    `read_self` (from `space-sync`)
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
- Sub-gatherings with their own spaces: one space per conference.

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

## Test cases

## Review log
