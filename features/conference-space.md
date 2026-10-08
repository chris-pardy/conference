---
status: implementing
impact: cross-cutting
depends-on: [attendee-sign-in]
branch: feature/conference-space
tests-commit: c6e7d413c8c84d283f6767b85f4834b10ff5b6c7
---

# Conference space

## Summary

A conference is an atproto permissioned space whose **authority is the
organization**, an identity separate from any one person, with our server
as its space host. Attendees are its
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

**Inside.** Only members can open the conference's inside, through
eventside or any other app the organizer allows.

**Leaving.** An attendee can leave a conference. What they wrote stays in
their repo, but the appview stops serving it to others. They can rejoin
through any method that admits them.

**Organizers.** Each organization has its own DID, which our server hosts
spaces for. Its admins (owners and staff) sign in as themselves, so nobody
shares a password, and admins can be added or removed. Each conference has
one **super admin**, the person whose account publishes the conference's
records. One organization can run several conferences.

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

- **The organization** is a space authority DID (by default a `did:plc` our
  server mints) whose DID document names our server as its space host.
- **The conference's entry point** (public conferences) is a public
  `community.lexicon.calendar.event` record in the super admin's repo. It
  can be one that already exists, such as one published through another
  calendar app. An eventside sidecar record links it to the space and holds
  the conference's settings: join methods, public or invite-only, and theme
  (for [`event-branding`](event-branding.md)).
- **Invite-only conferences** have no public event record. Their entry point
  is an invite link that names the space.
- **The space** is a permissioned space with the organization as authority,
  hosted by our server:
  - one space per conference
  - the conference's sessions, plans, chat and everything else inside it
    are records in that space
  - which apps can read it is the organizer's choice (a curated allow-list
    or open), and every member can read everything in it through those apps
    (revised in design review round 2; this replaced "only the appview reads,
    members get `read_self`")
- **Membership** is our host's member list. **Roles and rules** are records
  the super admin writes into the space.
- **Attendee lists, codes, requests and bans** are kept by our server, where
  attendees can't read them.
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

None. Each one was settled in the design review:

- **RSVPs from other calendar apps as a way to join:** deferred. They're
  ignored for now.
- **Attaching an existing event:** allowed only for an event in the
  organization's own repo (`conference create --event`).
- **Acting for the organization:** the super admin (and other admins)
  connect once with `eventside admin connect`. That's a cookieless session
  our server uses to write the conference's records (revised in round 3).
- **Where lists, codes, requests and bans live:** in the appview's database,
  because they're secrets or decisions about whom to admit. Membership,
  roles and rules live in the space.
- **The last owner:** the CLI refuses to remove the last owner. The operator
  is trusted, so this is a safety check, not a security boundary.

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

### Design (revised in round 3)

Round 3 makes **our server the space host**. Earlier rounds had the
organization's PDS host the space and our appview manage it through the
organization's OAuth session. Their decisions on joining, privacy within a
space, and space types still stand. The rest is rewritten here, and the
round entries below record what changed when.

#### What the protocol and vivarium support

From the current permissioned-data proposal (0016), vivarium 0.0.2 (which
matches it: HTTP-message-signature credentials) and a review of vivarium's
source. The sibling `public-spaces` host tracks an older alpha format (DPoP
credentials) that vivarium 0.0.2 rejects, so we target the 0.0.2 format.

- **A space's authority** is the DID at the root of its URI
  (`at://{authority}/space/{type}/{skey}`). Its DID document names:
  - the **space host**: service `#atproto_space_host` (type
    `AtprotoSpaceHost`), falling back to `#atproto_pds`
  - the **credential key**: verification method `#atproto_space`, falling
    back to `#atproto`. Vivarium honors a JWT header `kid` of
    `#atproto_space`, so every credential and service JWT we sign carries
    it. For **minted** DIDs we also publish the same key as `#atproto`, which
    means service-auth from that DID is accepted anywhere; for a
    bring-your-own `did:plc`, `#atproto` is its PDS's key and is left alone.
- **The space host must implement:**
  - `com.atproto.space.getSpaceCredential`: takes a single-use delegation
    JWT from a user (`aud` = `{authority}#atproto_space_host`), an HTTP
    message signature by the app's P-256 `did:key`, and, under an app
    allow-list, a client attestation. It checks the user's read access and
    the app's access, then returns a credential signed with the authority's
    key: `{iss: authority, sub: space, cnf: {kid: app did:key}, iat, exp,
    jti}`, 10 minutes by default, 1 hour at most.
  - `listRepos` (the writer set, with `repoRev`, `hash` and `spaceRev`),
    `registerNotify` and `unregisterNotify`, all credential-authenticated
  - receiving `notifyWrite` from writers' PDSes: service-auth with
    `iss` = the writer (which must equal the body's `repo`),
    `aud` = `{authority}#atproto_space_host`,
    `lxm` = `com.atproto.space.notifyWrite`. Check the write policy (a
    non-member gets a deliberate 403, which makes vivarium drop it rather
    than retry; it times out after 5 seconds), ignore stale or future
    revisions, assign a `spaceRev`, and forward to registered syncers
  - sending `notifySpaceDeleted`, and `notifyCredentialRevoked` to repo
    hosts
  - The `simplespace.*` management methods are a PDS requirement, not a host
    one, so a host like ours uses its own admin.
- **Vivarium 0.0.2 as an attendee's PDS works with a host elsewhere:**
  - it accepts writes into a space whose authority isn't one of its accounts
  - it sends `notifyWrite` to the authority's `#atproto_space_host`, with
    retries
  - it verifies our credentials against the authority's DID document
  - it issues delegation tokens for foreign spaces (with an OAuth `read`
    grant)
  - it accepts our revocation and deletion notifications
- **Vivarium's PLC accepts any valid operation,** so our server can create a
  `did:plc` with custom services and keys. Vivarium resolves `did:web` over
  plain http only for loopback or declared app hosts.
- **One catch:** if the authority DID *is* a vivarium account, vivarium acts
  as the host itself and refuses writes. Repointing an existing vivarium
  account needs a vivarium change. Minted authorities don't.
- **Attendee scopes** must name the authority:
  `space:{type}?authority={did}` or `authority=*`. The default is `self`.

#### Approach

**Our server is the space host for every eventside space.** Each
organization has a **space authority DID** whose DID document points
`#atproto_space_host` at our server and publishes an `#atproto_space` key
that our server holds. Attendees' records still live in per-space repos on
their own PDSes. Our server is where access is decided:

- it issues every credential, so app access and read access are enforced by
  the protocol at our door
- it receives every write notification, so the write policy decides whose
  records enter the space
- membership, policies and app access are our own state, not a copy kept in
  step with someone else's PDS

**The authority DID:**

- **Default: we mint one.** `eventside admin org create` makes a new
  `did:plc`. Our server generates its rotation key and its signing key (as
  both `#atproto_space` and `#atproto`), and registers a genesis operation
  with one service, `#atproto_space_host` → our `PUBLIC_URL`. There's no
  `#atproto_pds`.
- **Bring your own (documented, not automated yet):**
  - an existing `did:plc`: the owner adds our `#atproto_space_host`
    service and the `#atproto_space` key we generate for them, with a PLC
    operation they sign
  - a `did:web` (e.g. `did:web:atmosphereconf.org`): they add the same two
    entries to their `did.json`
  - `eventside admin org adopt <did>` checks the document points at us with
    a key we hold, then treats it like a minted one
  - An existing account on a PDS that hosts spaces itself (vivarium today)
    needs that PDS to defer to the declared host. That's a vivarium change,
    deferred.
- **Later:** we expect to run a small reference PDS. A minted authority
  could then also get an `#atproto_pds` with us, and its own repo (for
  posting to Bluesky from the conference, for example). Until then, a
  minted authority has no repo.

**Our server holds the authority's keys** (rotation and signing),
encrypted at rest under a key from the `AUTHORITY_KEY_SECRET` environment
variable (a KMS later). That's custody, and it's stated to organizations.
The genesis operation lists an **operator-held recovery rotation key ahead
of ours**, so a breach of our server is recoverable within PLC's 72-hour
window. With "bring your own", the organization keeps its rotation
key, and we only hold the space key they published.

**Permissions are records in an admin space, crawled from the super admin.**
Every permission is a record that an admin writes, so any app (or another
host) could rebuild them. Our database is only an index of those records.

- **The only state we must keep** for an organization is its authority's
  keys and its **super admin's DID**. Everything else can be rebuilt by
  crawling from there (`eventside admin reindex <org>`).
- **The admin space:** each organization has one,
  `at://{authority}/space/app.eventside.admin/self`, hosted by us. Its
  members are the organization's admins (read and write). Other apps can be
  allow-listed into it like any space.
- **The crawl:**
  1. From the authority, find its super admin (our stored state).
  2. Read the super admin's repo in the admin space (with a credential we
     mint for ourselves). Their records name the organization's **admins**
     (`app.eventside.admin.admin {subject, role: owner | staff}`), its
     **spaces** and their **policies and app access**, and any membership
     decisions they made themselves.
  3. Read each admin's repo in the admin space. Their records count within
     their role.
  4. Index the result. Every host check (credentials, write intake, the
     host API) and every feature's membership check reads the index.
- **Records in the admin space** (lexicons published with the others):
  - `app.eventside.admin.admin {subject, role}`: super admin only
  - `app.eventside.admin.space {space, type, readPolicy, writePolicy,
    appAccess, allowList?, join?}`: one per space (conference, group,
    ballot), with the conference's join methods. Super admin only; owners
    may write them for spaces they create (`groups`, `polls`)
  - `app.eventside.admin.member {space, subject, read, write, since,
    until?, via}`: a membership an admin decided (an approved request, a
    pre-assigned speaker, an email match, a removal). Any admin within their
    role. Automatic admissions aren't written here (see "The intake space")
  - `app.eventside.admin.ban {space, subject}`: owners and the super admin
  - `app.eventside.admin.code {space, codeHash, subject?, expires?,
    maxUses?}` and `app.eventside.admin.listEntry {space, did? (with the
    handle it was resolved from) | emailHmac?, role?}`: owners and the super
    admin. Codes are hashed, and
    emails are HMAC'd with a server key, so neither is readable even inside
    the admin space.
- **Precedence, so no tiebreak is needed** (superseded in round 5: see
  "Signed decisions" below, which checks precedence when a decision is
  signed, not when it is read):
  - Admins write only to their own repos, so nobody can edit anyone else's
    records.
  - **The super admin's records always win.** No other admin's record can
    contradict them. For example, a staff member can't admit someone the
    super admin banned, or change a space's policy.
  - **Membership decisions rank by who made them** (revised in the
    simplification of 2026-10-07): the super admin, then owners, then
    staff. A decision (admitting, removing, denying) stands against later
    ones of a lower rank that contradict it, so staff can't override an
    owner's or the super admin's decision, nor owners the super admin's.
    Within a rank, the most recent record (by commit `repoRev`) stands. A
    ban beats any admission but the super admin's. Staff can still invite:
    the people they admit get full membership.
  - A record counts only while its author is an admin, and only within
    their role. When the super admin removes an admin, `--keep-admissions`
    first writes her own admissions for everyone who's a member only on
    their say-so (TC-58).
- **Who writes what, in practice:**
  - The **super admin** connects once (`eventside admin connect`). Our
    server writes the organization-wide records (admins, spaces, policies)
    as the super admin, and email-matched admissions (the one automatic
    case that can't be derived; see below).
  - **Other admins** connect the same way. A CLI action taken by them (an
    approval, a removal, a ban) is written to *their* repo, under their
    session.
- **Latency:** a permission change takes effect when it reaches the index.
  Our server indexes its own writes as soon as the PDS accepts them, and the
  `notifyWrite` from that PDS confirms it. If an admin's PDS is unreachable,
  the action fails and is retried. Nothing is reported as done before the
  record exists.
- **What stays in the database only,** because it's transient or secret:
  - rate-limit counters, replay caches, and issued credentials (for
    revocation)
  - the HMAC key and the authority keys

**The intake space: joining and leaving as the person's own records.**
Each conference has an intake space,
`at://{authority}/space/app.eventside.intake/{conference skey}`, hosted by
us:

- **Write policy `#publicPolicy`:** anyone signed in can write to it.
- **Read policy:** a member list of the organization's admins (read only),
  kept in step with the admin space's `admin` records, and the same app
  access as the admin space. So our host (as the authority) and the admins,
  through their allowed apps, can read it. Attendees can't read each other's
  join records.
- **Joining:** `join` writes `app.eventside.intake.join {code?}` into the
  person's own repo in the intake space. The code is in plain text, which
  only our host and the admins can read. **Leaving** writes
  `app.eventside.intake.leave {}`.
- **Automatic admissions are derived, not written** (round 5: joins are
  signed when written; see "Signed decisions"). The indexer admits
  someone whose join record (judged by its commit `repoRev`) matches a rule
  in the admin space: a valid `code` record (with expiry and uses counted
  in commit order), a `listEntry` by DID, or the space being open.
  So a registration rush is the attendees' own writes, not writes to the
  super admin's PDS.
- **Requests** are join records that no rule admits. They're pending until
  an admin writes a `member` record for them in the admin space, or denies
  them with a `ban` (or an explicit `app.eventside.admin.deny {space,
  subject}`, which doesn't block future requests).
- **Email matches can't be derived,** because only our host saw the verified
  email at sign-in. When the email step matches a `listEntry`, our server
  writes a `member` record as the super admin.
- **Who can see it:** our host and the organization's admins. So an admin's
  app can derive every membership itself, from the admin space plus the
  intake spaces, the same way our indexer does. Other members' apps use our
  host API, which publishes every membership with its periods.
- **Scope:** attendees need
  `space:app.eventside.intake?authority=*&action=create&action=delete&collection=app.eventside.intake.join&collection=app.eventside.intake.leave`
  in `LOGIN_SCOPES`, one more line on everyone's consent screen. This
  feature adds it.
- **Abuse:** anyone can write, so intake notifications are rate-limited per
  DID before we fetch anything, as on the other host endpoints.

**Admins, the super admin, and the conference's own records.**

- An organization's **admins** are the subjects of the super admin's
  `admin` records. They're members of each of the organization's
  conferences.
- Each conference has exactly **one super admin** (set by
  `conference create --super-admin <handle>`, and the organization's
  super admin by default). That DID writes the conference's public-facing
  records:
  - the **public calendar event** and its `app.eventside.conference`
    sidecar, in the super admin's public repo (public conferences), or inside
    the conference space (invite-only)
  - **role** and **rules** records, in the super admin's repo inside the
    conference space, where every member's apps can read them. Readers
    trust them **only** from that DID.
- The authority DID itself can be the super admin, once it has a repo
  (bring your own account, or our reference PDS later).
- **Admin sessions:** each admin connects once, through
  `eventside admin connect <handle>`. That's an OAuth sign-in with
  `ADMIN_SCOPES`, giving a cookieless session of kind `admin` that never
  idles out.
- **`ADMIN_SCOPES`**, written out:
  - `atproto`
  - `repo:community.lexicon.calendar.event` and
    `repo:app.eventside.conference`, for the public records
  - `space:app.eventside.conference?authority=*&action=create&action=update&action=delete&collection=app.eventside.conference&collection=app.eventside.conference.role&collection=app.eventside.conference.rules&collection=community.lexicon.calendar.event`
  - `space:app.eventside.admin?authority=*` with `read`, `create`,
    `update`, `delete` and the `app.eventside.admin.*` collections
  - `blob:*/*`, for `event-branding`
  - When `ADMIN_SCOPES` grows, admins are told to reconnect (as in
    round 1's "reconnect needed"). A real PDS may cap refresh-token
    lifetimes; the CLI reports an expired admin session the same way.
- **Bootstrapping:** `org create --super-admin <handle>` mints the
  authority, creates the admin space, and stores the super admin. The
  super admin connects, and the CLI writes their first records (themselves
  as owner, the admin space's own record).

**Membership, as the index sees it.**

- A member is someone with a current `member` record in the admin space,
  and no ban that wins over it.
- **Other apps see membership** in one of two ways: admins' apps read the
  admin space, and any credential holder for a conference space can call
  our host API, which publishes members with their periods, admins, and
  space policies (derived from the records, so nothing new).
- **Which records count.** A host sees one `notifyWrite` per repo state,
  not per record, and a removed member's own PDS keeps accepting their
  writes. When they rejoin, their next accepted `repoRev` carries every
  commit they made while out. So the host can't refuse individual records.
  Readers judge each record by its **commit's `repoRev`** (a TID timestamp)
  against the author's membership periods (`since`, `until`, and the last
  `repoRev` accepted inside the period). A record counts only if its commit
  falls inside a period. Our appview applies this at ingest, and other apps
  are expected to.
- **Removal takes effect at once, everywhere:**
  - we stop accepting the person's write notifications
  - we stop issuing credentials delegated by them
  - we revoke the ones already issued. We track each credential's `jti` and
    who delegated it, and send `notifyCredentialRevoked` once per distinct
    writer PDS, with the service JWT's `aud` set to one writer's DID on that
    PDS and up to 100 `jti`s per call. Vivarium keeps revocations for about
    an hour, so revocations from the last hour are also sent to any writer
    PDS that first appears within that hour. Credentials last 10 minutes,
    so revocation is a backstop, not the only guarantee.

**Abuse limits on the host endpoints.** `getSpaceCredential` and
`notifyWrite` are public, and verifying them means resolving DIDs. So:

- read the unverified `iss` (delegation) or `repo` (notifyWrite) first, and
  refuse non-members before resolving or verifying anything
- cache DID documents
- limit per IP and per DID
- keep the delegation and attestation replay caches bounded
- limit `listRepos` per credential key (`cnf.kid`), as the proposal suggests

**App access** is part of each space's `space` record: curated (an
allow-list of client IDs, starting with eventside's bare client ID) or
open. We enforce it in `getSpaceCredential`, so it holds for every app.
`eventside admin apps add|remove|open|curate` rewrites the record. Switching
to open warns first.

**One space per audience, all hosted by us.** The types from round 2 stand:

| Space type | Audience | Authority | Policies | Specced in |
|---|---|---|---|---|
| `app.eventside.conference` | everyone admitted | the organization's authority | read and write: members | this feature |
| `app.eventside.group` | organizer lists, plan audiences, derived groups (going to X, my connections), personal lists | the organization's authority, except personal lists (see `groups`) | members; derived groups are computed by us at check time | `groups` |
| `app.eventside.ballot` | an anonymous poll's voters: they write, only admins and eventside read | the organization's authority | write: voters; read: admins | `polls` (later) |

Because we're the host, **derived groups need no managing-app policy**. We
compute membership ourselves when checking a credential or a write, and the
group publishes a description of its rule. Round 2's "exception" goes away.
Personal lists that must stay private from the organizer are for `groups`
to settle: under the organization's authority, the organizer's admins could
be given read access.

**Rules enforced by readers** (from round 2, unchanged in substance):

- The rules record says, per record type, who may write it: cards and
  announcements only admins, plans, chat and RSVPs any member.
- The protocol can't restrict writes by collection, so readers apply the
  rules: only role and rules records from the super admin count, and a
  record is judged by its commit's `repoRev` against membership periods.
- Our appview applies them at ingest. Other apps are expected to.
- Finer rules (block-actions middleware) are eventside's own.

**The conference entry point:**

- **Public conferences:** a `community.lexicon.calendar.event` plus the
  `app.eventside.conference` sidecar (space URI, `visibility`, join flags,
  `appAccess` mode, super admin, theme), in the super admin's public repo.
  `conference create --event <at-uri>` adopts an existing event, if the
  super admin wrote it. Links use the super admin's DID (or handle).
- **Invite-only conferences:** the same records, written inside the space
  by the super admin. The entry point is `/join/{code}`.
- **Identity:** the event URI is the conference node (plans' `childOf`),
  and the space URI is the access boundary. `getConference` accepts either.

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

- Handle rows are resolved to DIDs when they're imported, and stored by
  DID. A handle transferred later doesn't carry the place on the list with
  it. A handle that doesn't resolve is reported in the import's output and
  skipped, with nothing stored about its row; the operator imports it again
  once it resolves (simplification of 2026-10-07).
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

- **Leave or remove:** the member row gets its end time, and the person's
  credentials are revoked. Done in one database transaction plus outgoing
  revocations, retried until delivered.
- **Ban:** remove, plus a ban row keyed by DID.
- **Rejoining** starts a new membership period. Records committed in between
  don't count, because readers judge each record's commit `repoRev` against
  the membership periods (see "Which records count").

**Abuse limits on `join`:**

- 10 attempts a minute per DID, and 30 per IP.
- Personal codes carry at least 80 bits of randomness.
- Shared codes can have an expiry and a usage cap (`codes issue --shared
  --expires … --max-uses …`).

**Admin CLI** (a subcommand of the server binary):

- `org create --super-admin <handle>` (mints the authority and creates the
  admin space) and `org adopt <did>`
- `admin connect <handle>`, and `org admin add|remove <handle> --role
  owner|staff` (super admin only)
- `reindex <org>`, which rebuilds the index by crawling from the super
  admin
- every action runs as the admin named by `--as <handle>` (default: the
  super admin) and is written to that admin's repo
- `conference create --super-admin <handle> [--event <at-uri>]
  [--invite-only]`
- `join set`, `codes issue`, `list import <csv>`, `requests
  list|approve|deny`
- `member remove|ban|role` (`role` writes or deletes the role record as an
  admin)
- `apps add|remove|open|curate`, `rules set <file>`

**Deferred:** whether a public `community.lexicon.calendar.rsvp` of
"going" to the conference's event should count as a way to join. RSVPs
from other apps are ignored for now.

#### Signed decisions (round 5)

Round 5 replaces how readers decide which permission records count. Up to
round 4 (and the 2026-10-07 simplification), a reader replayed history: a
record counted only while its author held the right role, and a decision
was weighed by its author's role *now*. So changing an admin's role quietly
changed other people's memberships, which is what rounds 11 to 15 kept
finding. The new rule:

> **A decision or action is checked when it's taken, and once valid, it
> stays valid.**

**Check, then sign.** Every permission record and every join is written
through our server: the CLI acts through the admin's session, and the PWA
through the attendee's. Before writing, the server checks the action
against the state at that moment: is the author an admin, may their rank do
this, does it contradict a standing decision of a higher rank, is the code
still valid and not used up. If it passes, the server signs the record and
writes it with the signature inside. If it fails, nothing is written and the
command says why.

- **The keys:** one or more dedicated verification methods in the
  authority's DID document, `#eventside_attest` and then
  `#eventside_attest_2` and so on. A DID document holds one key per
  fragment, so several keys means several fragments. Its private half is held by our server and kept
  separate from `#atproto_space` (credentials), so a leaked credential key
  can't forge decisions and the other way round. Whoever manages the DID
  document is in charge: any app they give the private key to can attest,
  and they take that power away by rotating the key out.
- **The signature follows the badge.blue attestation spec** (Nick
  Gerakines' "ATProtocol Attestation Specification",
  tangled.org/strings/ngerakines.me/3m3fy2xuahc22). That keeps our records
  readable by its tooling, such as the `atproto-attestation` Rust crate, and
  lets us move to remote attestations in a separate repo later without
  changing what readers check.
  - A record carries a `signatures` array. Each entry is an inline
    attestation:
    `{$type: "app.eventside.attest.signature", key:
    "did:plc:…#eventside_attest", space, seq, role, signedAt, signature:
    {$bytes}}`.
  - **What's signed:**
    1. Copy the entry and delete `signature`. That copy is `$sig`, so
       `key`, `space`, `seq`, `role` and `signedAt` are all signed.
    2. Add `repository`: the DID of the repo holding the record.
    3. Remove `signatures` from the record and add the `$sig` object.
    4. Encode as canonical DAG-CBOR, take the SHA-256, and build a CIDv1
       (dag-cbor, sha2-256).
    5. Sign the 36 CID bytes with low-S ECDSA (P-256).
  - **Verifying:** rebuild `$sig` the same way, with the repo the record was
    read from, and check the signature against `key`. The key's DID must be
    the space's authority, and its fragment must start with
    `eventside_attest`.
  - Because `repository` and `space` are inside the signed CID, a signed
    record copied into another repo or another space doesn't verify. A copy
    within the same repo and space repeats the same `seq`, and readers
    count each `seq` once.
  - **Later: remote attestations.** Once we host a repo for the authority
    (the reference PDS), a `signatures` entry can be a `strongRef` to a
    proof record `{cid}` in the authority's repo. The CID is built the same
    way. Readers accept either form.
- **`role`** is the author's role when the server signed: `superAdmin`,
  `owner`, `staff`, or `self` for a person's own join or leave.
- **`signedAt`** comes from our server's clock, strictly increasing per
  authority (signing is serialized per authority). It is the record's time
  for every rule below. Commit `repoRev`, first-seen times and the 60-second
  slack are no longer used for permission records.

**What readers do:** our indexer, an admin's app, and any app checking our
host API.

1. Find the records: the admin space's writers, and the intake space's.
2. Verify each signature against the authority's DID document. A record
   without a valid signature doesn't count.
3. For each person in each space, the **latest signed decision stands**:
   an admission, removal, ban, deny, or their own signed join or leave.

There's no precedence at read time. Precedence was enforced when signing:
the server refused to sign a staff admission that contradicts an owner's
ban. The ranking the server applies is round 3's, but against the **role
recorded in the standing decision's signature**, not the author's role now:

- A decision may contradict the standing decision only if the actor's rank
  now is at least the rank recorded on it. Ranks: super admin, then owners,
  then staff.
- The person themselves may always leave. Their own join is signed only if
  they aren't banned and a rule admits them (see joining), and it then
  stands over an earlier removal or denial, as before.
- Owners and the super admin ban. Staff admit, approve requests, deny,
  remove, and **issue codes** (the user's "staff can invite people with full
  rights"). The people they admit are full members.

**Consequences:**

- **Removing or demoting an admin changes nothing already decided.** Their
  admissions, removals, bans, codes and list rows keep standing. From then
  on the server won't sign for them, or signs at their new rank. TC-53
  flips.
- **Undoing a former admin's admissions** is an explicit action:
  `org admin undo <handle> --admissions [--conference <space>]` writes a
  signed removal, by the acting owner or super admin, for everyone whose
  standing decision is that admin's admission. People with another standing
  way in (their own signed join by code, say) aren't touched. It replaces
  `--keep-admissions`.
- **Codes and list rows outlive their author too.** A code is valid until
  it expires, is used up, or an admin revokes it with a signed `codeRevoke`
  (at the code author's rank or higher).
- **Admin records are never deleted.** Removing an admin writes a signed
  `admin {subject, role: "none"}`, so the crawl still finds the former
  admin's repo, whose decisions still stand.
- **Unsigned records don't count.** An admin record written from another
  app, or by hand, has no effect. Our server could only have signed it after
  checking it.

**Joins are signed too.** When the PWA joins, our server checks the join
(code valid and not used up, DID on the list, conference open, verified
email matched) and writes `app.eventside.intake.join {code?, via, role?,
signatures}` into the person's intake repo, signed with `role: "self"`. `via` is
`code`, `list`, `email`, `open` or `request`, and only `request` leaves
them pending. So:

- Code uses are counted when signing, not by replaying commits. Expiry is
  checked against `signedAt`.
- **Email matches no longer need the super admin's PDS.** Round 4's
  `member` record written as the super admin becomes a signed join with
  `via: "email"`. Joining never touches an admin's PDS (TC-54).
- A join written by another app without our signature is a request at most:
  it's pending until an admin decides.
- **Leaving** is a signed `leave`, written when the person leaves through
  eventside. An unsigned leave from another app doesn't count, like any
  unsigned record.

**Records are the source of truth, signatures included** (the user's
choice in round 5). A deleted record no longer counts. A record changed by
editing counts as edited if its new version is signed. So withdrawing a
decision, or re-signing one, is done by deleting or editing the record, and
there's no separate log. `reindex` rebuilds everything from the records, as
in round 4 (TC-50).

**Roles and rules** in the conference space (`role`, `rules`, the
conference sidecar) are signed the same way. Readers trust a signature by
the authority, not "written by the super admin's DID". They're still written
to the conference's super admin's repo, so TC-35 keeps its meaning.

**Key rotation, through multiple keys.** A signature names its key by
fragment, and a record may carry several signatures.

1. **Add** a new key (`#eventside_attest_2`) to the DID document. New
   records are signed with it.
2. **Re-sign** standing records by editing them, adding a signature by the
   new key next to the old one.
3. **Remove** the old key once nothing standing depends on it alone.

A record counts if any of its signatures verifies against a key the DID
document lists now.

- **A compromised key:** remove it straight away. Every signature that
  depends only on it stops verifying. Decisions worth keeping are re-signed
  by editing their records through the admin's session, and forged records
  are deleted. Records we can't edit any more, such as a disconnected
  former admin's, stop counting, which is accepted (the user, round 5).

**Content records aren't signed.** Plans, chat, RSVPs and other members'
records are still judged by their commit `repoRev` against membership
periods (see "Which records count"). The periods now come from signed
decisions.

**Refinements after the critique of round 5** (these replace the parts of
the text above they contradict):

- **Order by a sequence number, not by time.** Every signed entry carries
  a per-authority `seq` inside the signed payload. Readers order by `seq`.
  `signedAt` is informational, and readers reject one more than five
  minutes in the future. There is **one signer per authority**, our host,
  for the MVP. "Any app with the key can attest" stays true of the key, but
  a second signer would need its own sequence and a merge rule, which is
  left for later.
- **Sign, write, then commit,** so a failed write never stands (TC-55) and
  two admins can't race:
  1. In one database transaction (`BEGIN IMMEDIATE`, since the CLI is a
     separate process): check, take the next `seq`, and log the entry as
     *pending*.
  2. Write the record without holding the lock.
  3. Mark the entry *committed* on success, or *void* on failure.

  A pending entry blocks a conflicting signing (another decision about the
  same person, or a use of the same code) and counts for nothing until it's
  committed.
- **The signing journal** is operational state, not a log of record: the
  per-authority `seq` counter and the pending entries. It's added to Data
  and Components, with the `#eventside_attest*` keys and the `signatures`
  field. If it's
  lost, the counter restarts above the highest `seq` found in the records.
- **Each person can have more than one way in.** Membership comes from
  replaying that person's committed entries in `seq` order, not from their
  latest entry alone. An admin's admission and the person's own signed join
  are separate grounds, and a removal or ban ends every ground at or below
  its rank. `org admin undo --admissions` therefore removes a person only
  when the departed admin's admission is their only remaining ground. Ana
  joined with a code before Pim admitted her, so she stays (as in TC-53's
  test).
- **Edges:**
  - **A super-admin handover** (`conference super-admin`): the new super
    admin has the `superAdmin` rank, so they can override decisions their
    predecessor recorded at that rank. For roles and rules, the latest
    signed record (by `seq`) stands.
  - **A demoted admin** can't lift their own earlier decisions above their
    new rank (TC-60).
  - **Revoking codes:** `codeRevoke` needs the code author's rank or
    higher. Undoing an admin's admissions leaves their codes alone, and
    `org admin undo --codes` revokes those.
  - **Code use limits** count distinct DIDs. Rejoining with the same code
    doesn't use it up again, and a personal code works again for the DID
    it's bound to (TC-14).
- **Tests that go:** `--keep-admissions` and its tests, the round 13 and 14
  TC-28 regressions in `conference-review.test.ts`, and the TC-58 test in
  `conference-admins-keep.test.ts` (added after the freeze, not frozen),
  which TC-58's rewording replaces. TC-53's frozen test needs the approved
  amendment. TC-50's frozen test should pass as it stands.
- **Where the signature lives: (A), signed records in the admins' repos**
  (the user's choice). A host-signed log as the source of truth, with
  repos as a mirror, was considered and rejected. Deleting and editing
  records is how decisions are withdrawn or re-signed.

#### Hosting from a person's own account (deferred)

Small events that use a person's own DID as their authority were raised in
round 5 and deferred by the user ("don't worry about the personal accounts
for now"). The sketch, for when it comes back:
- **Setup:** one email-confirmed PLC update (`requestPlcOperationSignature`,
  `signPlcOperation`, `submitPlcOperation`, with `identity:*`) adds our
  `#atproto_space_host` and our `#atproto_space` and `#eventside_attest`
  keys. The person's own keys are untouched.
- **Vivarium** would need to defer to a declared host for its own accounts,
  and accept OAuth `identity:*` on those endpoints.
- **The user's preferred direction (2026-10-07):** revisit it with
  **records plus access delegates** rather than a DID-document change. The
  person publishes a record that delegates to our server, and our signatures
  point to that delegation. Two problems to solve then: the space host
  (without `#atproto_space_host`, spaces fall back to the person's PDS),
  and what deleting the delegation record means for signatures that rely
  on it.

#### Components

- **`crates/server/src/spacehost/`** (new): the host.
  - `credential.rs`: delegation verification (replay cache), RFC 9421 HTTP
    message signature verification (ecdsa-p256-sha256), client attestation,
    and minting with the authority key. It also tracks issued `jti`s.
  - `notify.rs`: `notifyWrite` intake (service-auth verification, write
    policy, `spaceRev`), forwarding to syncers, `registerNotify`, and the
    outgoing `notifySpaceDeleted` and `notifyCredentialRevoked`
  - `repos.rs`: `listRepos`
  - `api.rs`: our host API (membership, admins, policies, for credential
    holders)
  - `index.rs`: the crawler and indexer. It follows the admin space from the
    super admin (former admins' repos too), counts only records whose
    signature verifies, replays each person's signed entries in `seq` order
    (round 5), and keeps the index up to date from `notifyWrite` (our own
    admin space included). `reindex` rebuilds it from scratch.
  - `attest.rs` (round 5): the badge.blue inline attestations (signing,
    verifying, re-signing with a new key), and check-then-sign: the
    signing journal's `BEGIN IMMEDIATE` transaction that checks an action
    against the index, takes the authority's next `seq` and logs a pending
    entry, then commits or voids it once the record is written.
  - `authority.rs`: minting the `did:plc` (genesis operation, with
    `#eventside_attest`), adding and removing attestation keys with PLC
    operations signed by our rotation key, adopting a DID, key custody,
    DID-document checks
- **`crates/server/src/conference/`** (new): the conference model, join
  (unchanged), membership and roles on top of the host, rules, and the admin
  CLI.
- **`crates/server/src/auth/`** (from `attendee-sign-in`, changed): pending
  requests get a `purpose` (`login`, `admin` or `email`), sessions get a
  `kind` (`attendee` or `admin`), `ADMIN_SCOPES`, `outdated()` per kind, no
  idle timeout for admin sessions, `CurrentUser` refuses admin sessions, and
  the email callback.
- **PWA:** the public page (`/c/$actor/$rkey`), `/join/$code`, the join
  panel, and a minimal members' shell. Unchanged from round 2.
- **Test support:** `seedConference` mints an authority on vivarium's PLC
  with our server as host, makes an admin account, connects it through the
  real OAuth flow, and runs the CLI. `joinAs` is unchanged.

#### Data

**Kept by us, and needed to rebuild everything else:**

| Item | Where |
|---|---|
| Each authority's DID, its encrypted rotation and signing keys, and its super admin's DID | host DB |
| Each authority's attestation keys (`attest_keys`): fragment, public key, the private key encrypted at rest; a removed key keeps its row, without its private half, so its fragment isn't reused | host DB |
| The HMAC key for emails, and the server's encryption key | server config |

**Records, the source of truth:**

| Item | Where | Written by | Read by |
|---|---|---|---|
| Authority DID document (`#atproto_space_host`, `#atproto_space` key, `#eventside_attest` and `#eventside_attest_2`… keys) | PLC, or the organization's `did.json` | our server (minted) or the organization (adopted) | everyone |
| `app.eventside.admin.admin {subject, role: owner \| staff \| none, since}`, `.space`, `.member`, `.ban`, `.deny`, `.code`, `.codeRevoke {space, codeHash}`, `.listEntry`, each with `signatures` | admins' repos in the organization's admin space | the super admin and admins (our server, under their sessions, checked and signed) | admins' apps; our host (crawled) |
| `app.eventside.intake.join {code?, via, role?, signatures}` (`via`: `code`, `list`, `role`, `email`, `open` or `request`), `.leave {signatures}` | each person's own repo in the conference's intake space | the person (through eventside, checked and signed with `role: "self"`) | our host and the admins |
| `community.lexicon.calendar.event` + `app.eventside.conference` sidecar (space, visibility, join flags, app access mode, super admin, theme, `signatures`) | the super admin's public repo (public) or inside the conference space (invite-only) | our server as the super admin | anyone, or members |
| `app.eventside.conference.role {subject, role, since?, assignedBy?, via?, signatures}`, `app.eventside.conference.rules {rules, signatures}` | the conference super admin's repo inside the conference space | our server as the conference super admin, signed for the deciding admin | members' apps, trusted only when signed, from the conference super admin's repo |

**The `signatures` field** (round 5): an array of badge.blue inline
attestations, `{$type: "app.eventside.attest.signature", key:
"did:plc:…#eventside_attest…", space, seq, role, signedAt, signature:
{$bytes}}`. What's signed is the record without `signatures`, plus `$sig`
(the entry without `signature`, plus `repository`), as canonical DAG-CBOR,
by its CIDv1 (dag-cbor, sha2-256): low-S P-256 over the 36 CID bytes. A
record counts if any entry verifies against a key the DID document lists
now, for the repo it's read from and the space it's in, with `signedAt` no
more than five minutes ahead. Each `seq` counts once: within a repo and
space, the latest-written record claiming a `seq` is that decision's version
that stands. No lexicon JSON is published for the admin records yet, so
none is added for `app.eventside.attest.signature`.

**The index and operational state** (rebuildable or disposable):

| Item | Where | Built from or used by |
|---|---|---|
| `admins`, `spaces` (policies, app access), `members` (periods, last accepted `repoRev`), `bans`, `codes`, `list_entries`, `conferences` | host DB | crawled from the admin space; read by every check |
| `writers` (`repoRev`, `hash`, `spaceRev`), the space-wide `spaceRev` sequence | host DB | `notifyWrite` intake; `listRepos` |
| rate-limit counters, delegation and attestation replay caches, `credentials_issued` | host DB | credential checks; revocation |
| `sessions.kind`, `oauth_requests.purpose` | sign-in tables | sign-in, renewer |
| the signing journal: `signing_counters` (each authority's next `seq` and last `signedAt`) and `signing_journal` (each signing's entry: `pending`, then `committed` or `void`, with the conference, person and code it's about) | host DB | check-then-sign. A pending entry blocks another decision about the same person for up to two minutes; a code's pending and committed uses count against its limits. Lost, the counter restarts above the highest `seq` in the records |

#### Interfaces

- **The protocol host endpoints:** `getSpaceCredential`, `listRepos`,
  `registerNotify`, `unregisterNotify`, and receiving `notifyWrite`, at our
  `PUBLIC_URL`, in vivarium 0.0.2's wire format.
- **The admin space's lexicons** (`app.eventside.admin.*`), published, so
  any app an organization allows into its admin space can read its
  permissions directly.
- **Signatures** (round 5): the `signatures` field above, verifiable by any
  app against the authority's DID document (`#eventside_attest*`
  verification methods), with the badge.blue procedure.
- **Admin CLI additions** (round 5): `org admin undo <handle> --org <did>
  (--admissions | --codes) [--conference <space>]`, and `org keys add
  --org <did>` (prints the new key's ID), `org keys resign --org <did>`
  and `org keys remove <fragment> --org <did>`. `--keep-admissions` and the
  "who a role change affected" reports are gone.
- **Our host API** (credential-authenticated, `app.eventside.space.*`):
  `getSpace` (policies, app access, the authority's admins with periods)
  and `listMembers` (members with `read`, `write` and their periods). These
  are views derived from the admin space, for apps that can read a
  conference space but not the admin space.
- **Conference XRPC** (cookie and CSRF): `getConference`, `join`, `leave`,
  `listMyConferences`. Unchanged.
- **Rust, for later features:**
  - `membership::role`, `require_member`, `members`, `was_member_at`
  - `rules::may_write`
  - `spacehost::create_space(authority, type, policies, app_access)` and
    `spacehost::set_member` / `end_member`, for `groups` and `polls`
  - `spacehost::mint_for_self(space)`, so our own appview reads the space
    like any other app
  - `admin_client(admin_did)`, for writing as an admin (`program-import`,
    `event-branding`)

#### Impact on existing features

- **[`attendee-sign-in`](attendee-sign-in.md)** (complete): the session
  `kind` and request `purpose`, `ADMIN_SCOPES`, per-kind `outdated()` and
  idle handling, and the email path. Attendee behavior is unchanged, and its
  tests keep passing. `LOGIN_SCOPES` gains attendee space scopes
  (`authority=*`, explicit actions and collections) when the first feature
  writes into a space.
- **[`space-sync`](space-sync.md)** (`analysis`): changes the most. With
  our server as the host, write notifications arrive here directly, and our
  appview mints its own credentials. Space-sync becomes "fetch the new ops
  from the writer's PDS (`listRepoOps` with a self-minted credential) and
  index them". No organization delegation, no attestation round trip, no
  `registerNotify` to a remote host. Its design should start from this.
- **[`ui-blocks`](ui-blocks.md)** (complete): no code change. Its "members
  `read_self`, only our appview reads" privacy model was superseded in
  round 2.
- **[`block-actions`](block-actions.md)**: anonymous actions use ballot
  spaces; ingest checks `rules::may_write`; the accepted-actions table is
  eventside's judgement.
- **[`plans`](plans.md)**: a plan is written into its audience's space; a
  picked-people plan needs its own group space; per-RSVP "private
  attendance" means nothing within a space.
- **`groups`** (not specced): its spaces are hosted by us under the
  organization's authority, and derived groups are computed by the host.
  Personal-list privacy is its call.
- **Vivarium:** no change is needed for the tests. Adopting an existing
  vivarium account as an authority needs vivarium to defer to the declared
  host (deferred).

#### Alternatives

- **The organization's PDS as host, managed through its OAuth session**
  (rounds 1 and 2): `putMember` round trips, a mirror to reconcile, removal
  that couldn't revoke credentials, and an owner-only member list other
  apps couldn't see.
- **`#atproto_pds` pointing at us** (the fallback some alpha clients use
  instead of `#atproto_space_host`): that would make us the authority's
  PDS of record. Left out until we run the reference PDS.
- **Permissions as host state** (round 3's first draft): simpler, but only
  we could see or rebuild them. Records in an admin space, crawled from the
  super admin, make the host a cache.
- **Host-state roles and rules** (served only by our API): no repo needed,
  but they wouldn't be records.
- **Records from any admin:** two admins' records could conflict, and
  nothing would decide between them. One super admin per conference makes
  every organization record single-author.
- **The organization as a real account now** (vivarium deferring to the
  declared host for its own accounts): single author too, but it brings
  back an organization OAuth session and depends on the organization's PDS.
  It becomes possible later, through our reference PDS.
- **The `public-spaces` host:** it tracks the older DPoP format, and it's
  public-only.
- **Making the organization's existing account DID the default
  authority:** needs its PDS's cooperation, and breaks on vivarium today.
  Offered as "bring your own".

#### Risks

- **Key custody:** for minted authorities we hold the rotation key, so a
  breach of our server could take over organizations' DIDs. Keys are
  encrypted at rest. A later "export your rotation key" or "add your own
  rotation key" is worth planning.
- **Real-network discovery:** some current alpha PDSes and clients locate
  the host through `#atproto_pds`, which we omit. They may not find us
  until we run the reference PDS. Vivarium and the proposal use
  `#atproto_space_host`.
- **Alpha churn:** the credential and notify formats changed twice in a
  month. The host code is isolated in `spacehost/`, and the formats are
  pinned to vivarium's.
- **Rust implementations to build carefully:** RFC 9421 verification,
  ES256 JWT minting and verification, PLC genesis operations.
- **Rules are enforced by readers.** Other apps may ignore them.
- **Admin actions are PDS writes** on the acting admin's PDS. If it's down,
  those actions fail until it's back. Automatic joins don't depend on it:
  they're the attendee's own write to the intake space.
- **An attendee's PDS being down** means they can't join or leave until it's
  back.
- **Admin sessions are powerful:** the super admin's session writes every
  automatic admission. Scopes are limited to the admin collections.
- **Single-PDS tests** (vivarium): attendees on other PDSes go untested.
- **The real network for November 1:** attendees and admins need PDSes that
  implement permissioned repos. The demo assumes vivarium (or the spaces
  alpha PDSes). Mainstream PDSes don't host space repos yet.
- **A vivarium bug to file:** `notifySpaceDeleted` checks `iss` = authority
  only in its fallback branch.

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

**Approved** by the user on 2026-10-05.

### Round 3

**Feedback:** make our server the space host. The organization is a
`did:plc` or `did:web` whose document points at our server as its space
host. Missing vivarium support can be added there. The sibling
`public-spaces` project built a custom host.

Researched: the proposal's host requirements; vivarium 0.0.2 as attendees'
PDS for a space hosted elsewhere (works with no changes); `public-spaces`
(an older DPoP wire format, so we target vivarium 0.0.2's).

Decided with the user:

- **The authority DID:** we mint a dedicated `did:plc` by default. Repointing
  an existing DID, or adding entries to a `did:web`, is documented but not
  automated yet.
- **The DID document:** only `#atproto_space_host` and the `#atproto_space`
  key. `#atproto_pds` is a fallback, and we leave it out until we run a
  small reference PDS (expected, e.g. so people can post to Bluesky from
  their conference).
- **The organization's records** are written by an admin (into the space,
  or publicly). An admin can be the authority DID itself once it has a
  repo. Long-term we'll offer a PDS.

**Changes:**

- Our server implements the space host: credentials, write intake,
  `listRepos`, notify registrations, and revocation.
- Membership, policies, app access and admins become host state. There are
  no `putMember` round trips or reconcile, and the round 2 membership records
  are dropped (the host API publishes membership).
- Removal now revokes issued credentials.
- The organization's OAuth session becomes an admin's session. Admins write
  the public event, sidecar, roles and rules.
- Derived groups are computed by the host, with no managing-app policy.
- `space-sync` shrinks to fetching and indexing, since notifications arrive
  at our host.
- Status back to `design-review`. The approved test cases need revising
  for the admin model, host-enforced app access and removal (no reconcile).

**Critique, and the user's call on organization records:** a fresh reviewer
found that the first round 3 edit had mangled the file (a duplicated
description and round 2 design), now repaired, and these problems:

- **Admin-written records could conflict.** The user chose **one super
  admin per conference** as the single writer of its event, sidecar, roles
  and rules. There are no tiebreaks, and no "current admin" rule.
- **The host can't refuse individual records** (one `notifyWrite` per repo
  state, and a removed member's PDS keeps their writes). Readers judge each
  record's commit `repoRev` against membership periods, which the host API
  publishes.
- Also folded in:
  - revocation addressing (one call per writer PDS, `aud` a writer DID on
    it, at most 100 `jti`s)
  - `kid: #atproto_space` on everything we sign, and `#atproto` only for
    minted DIDs
  - the `notifyWrite` wire values (a deliberate 403 for non-members)
  - an operator recovery key ahead of ours in the PLC genesis operation,
    and the key-encryption source
  - abuse limits on the public host endpoints
  - `ADMIN_SCOPES` written out
  - the real-network assumption for November 1
  - stale description text fixed

### Round 4

**Feedback:**

- Membership, policies, app access and admins should be written into an
  **admin space** that we host, so we can crawl out from the super admin's
  account (the only state we need) to find every permission.
- Other admins write to their own repos, and can't edit the super admin's
  records.

**Changes:**

- New `app.eventside.admin` space per organization, hosted by us. Records:
  `admin`, `space` (policies, app access, join methods), `member`, `ban`,
  `code`, `listEntry`.
- **The state we keep** is reduced to each authority's keys and its super
  admin's DID. Everything else is an index rebuilt by crawling from the
  super admin (`reindex`).
- **Precedence:** each admin writes only to their own repo; the super
  admin's records always win; among other admins, a ban beats an admission,
  and otherwise the latest commit stands; a record counts only while its
  author is an admin, and within their role.
- Automatic admissions are written as the super admin. CLI actions are
  written as the acting admin.
- What stays database-only: pending join requests, rate limits, replay
  caches, issued credentials, and keys.
- Role and rules records for members stay in the conference space,
  written by the conference's super admin.
- Follow-up in the same round: an **intake space** per conference (anyone
  writes; our host and, at the user's request, the admins read). Join and leave are the person's own
  records there. Automatic admissions (code, list, open) are derived from
  them, not written. Requests are join records awaiting an admin's
  `member` (or `deny`) record. Email-matched admissions are still written
  by the super admin. Attendees get an intake write scope in
  `LOGIN_SCOPES`.

**Round 4 approved** by the user on 2026-10-06.

### Round 5

Opened 2026-10-07, during the build, after review round 15 came back with
the same class of finding as rounds 11 to 14: changing an admin's role
quietly changed other people's memberships.

**Feedback:**

- "A decision or action taken by anyone should be verified as valid at the
  time it was taken and should stay valid." Could signed records do it?
- A space can't hold a key, so the signing key goes in the authority's DID
  document. That's acceptable: "any app with the private key part can
  attest records. Whomever is managing the org DID document is in charge."
- Small events that use a person's own DID, not an organization's, need a
  way to publish a signing key too. Later in the round the user deferred
  this: "don't worry about the personal accounts for now".

- Later in the round:
  - Signature option (A): signed records in the admins' repos. "Don't worry
    about deleted or re-signed, we can delete and edit records to do that
    if we need to." So there's no decision log.
  - A former admin being able to delete their own bans is "part of the
    design".
  - Allow multiple attestation keys, and rotate keys that way.
  - Follow badge.blue's attestation design, so attestations can move to a
    separate repo (remote attestations) later.

**Changes** (see "Signed decisions" above):

- Every permission record, join and leave is checked by our server when
  it's taken, then signed with the authority's `#eventside_attest` key.
  Readers verify signatures, and for each person the latest signed
  decision stands. Ranking is applied at signing, against the rank
  recorded on the standing decision.
- Role periods, read-time precedence, first-seen clamping, the 60-second
  slack, `--keep-admissions`, and the "who a role change affected" reports
  all go.
- A former admin's decisions keep standing. `org admin undo --admissions`
  undoes their admissions explicitly.
- Unsigned admin records don't count. An unsigned join is a request at
  most.
- Email matches become signed joins, so the super admin's PDS is out of
  the join path entirely.
- Staff can issue codes.
- Removing an admin writes `admin {role: "none"}` rather than deleting
  their record, so the crawl still reaches the decisions that stand.
- Signed records in admins' repos are the source of truth. Deleting or
  editing a record withdraws or changes it, and there's no decision log.
- Hosting from a person's own DID is sketched and deferred.
- **Alternatives considered:**
  - a key published as a record in the admin space or the person's repo:
    rejected, because deleting the record breaks every signature
  - receipts in a repo the authority owns: deferred until we run the
    reference PDS; same rule for readers

**Test-case changes proposed** (to be approved after the design):

- **TC-53**, reworded: *A former admin's decisions keep standing.* Given Pim,
  staff, admitted Bram. When Olga removes Pim as an admin, Bram is still a
  member, and the CLI refuses any further decision as Pim. (Amends a frozen
  test.)
- **TC-58**, reworded: *An owner can undo a former admin's admissions.*
  Given Pim admitted Bram, Ana joined with a code, and Pim was removed. When
  Olga undoes Pim's admissions, Bram is no longer a member, and Ana still
  is.
- **TC-59 (new):** *An admin record without our signature doesn't count.*
  Pim writes a `member` record admitting Mallory from another app. Mallory
  isn't a member, and our host API doesn't list her.
- **TC-60 (new):** *A decision keeps its rank after its author is demoted.*
  Kees, an owner, banned Bram, then was made staff. Pim, staff, can't admit
  Bram.
- **TC-61 (new):** *Another app can check a decision for itself.* An app
  reading the admin space verifies Olga's admission of Bram against
  Atmosphere's DID document. The same record copied into Mallory's repo
  fails.
- **TC-63 (new):** *Rotating the signing key keeps earlier decisions.*
  (With round 5's multiple keys: adding a key and re-signing keeps them;
  removing a key drops records signed only by it.)
  After the operator adds a new signing key, Bram (admitted before) is still
  a member, and a new admission is signed with the new key.
- **TC-64 (new):** *A join written by another app is a request.* Ana writes
  a join with a valid code from another app, without our signature. With
  requests on she's pending; otherwise she isn't a member.
- **TC-65 (new):** *Staff can issue codes.* Pim issues a shared code, and
  Bram joins with it.
- Unchanged in meaning: TC-13, TC-14, TC-18, TC-50, TC-51, TC-52 and TC-54.
  Their checks move to signing time. The frozen tests may need no change,
  since they act through the CLI and the PWA, which sign.

**Round 5 approved** by the user on 2026-10-07 ("approved, let's
build"), including the test-case changes it proposed, written out under
"Test cases".

### Build notes

Written at the start of the build (2026-10-06). Nothing landed on `main`
after the spec was approved, so these check the design against the merged
`attendee-sign-in` code and vivarium 0.0.2's source.

**The sign-in code, as merged:**

- `crates/server/src/auth/` (`cookies`, `pds`, `renew`, `return_to`,
  `routes`, `session`) and `oauth.rs`, `config.rs` at the crate root are as
  the design assumes.
- `LOGIN_SCOPES` (`config.rs`) is `["atproto"]`. `OAUTH_SCOPES` overrides
  it, and that one list is `state.config.scopes`, which `session::outdated`
  compares every session against. Per-kind `outdated()` means admin sessions
  compare against `ADMIN_SCOPES` instead. The tests override that list with
  `ADMIN_OAUTH_SCOPES`, named after `OAUTH_SCOPES`.
- `oauth_requests` already has a `kind` column (`login` or `signup`),
  documented as "diagnostic only". The design's `purpose` can be that
  column, but the callback then has to branch on it.
- `sessions` has no `kind`. The renewer (`auth/renew.rs`) ends idle sessions
  by `last_seen_at`, and `refresh_leased` checks `outdated()`. Both need
  the admin exemption, not only `session::lookup`.
- `CurrentUser` finds sessions by cookie, and admin sessions never get one,
  so refusing them there is a backstop.
- **Client IDs:** `client_id()` is the bare metadata URL only for the scope
  list `atproto`. Any other list gets `…/oauth-client-metadata.json?scope=…`,
  and the metadata route serves every well-formed list. `ADMIN_SCOPES` and
  the email step's list need no route change. Once `LOGIN_SCOPES` gains the
  intake scope, eventside's own attendee client ID isn't the bare URL any
  more. So an app allow-list "starting with eventside's bare client ID" has
  to match eventside by its metadata URL whatever the query.
- `scope_problem` accepts `space:…?authority=*&action=…` scopes. The longest
  list (`ADMIN_SCOPES`) stays far below its 2048-byte limit.

**Vivarium 0.0.2, as its source behaves:**

- Accounts sign with **secp256k1** (ES256K). Delegation tokens and
  `notifyWrite` service JWTs from members' PDSes are ES256K. Our host has to
  verify ES256K as well as ES256 (the `k256` crate isn't a dependency yet).
- Delegation tokens: `typ: atproto-space-delegation+jwt`, `kid: #atproto`,
  `aud: {authority}#atproto_space_host`, 60 s, single use. A PDS issues one
  for a session, or for an OAuth grant with a `read` action on that space
  (`read_self` isn't enough).
- `getSpaceCredential`: body `{space, clientAttestation?}`,
  `Authorization: Bearer <delegation>`, and signature label `atproto-space`
  over `("authorization");keyid="did:key:…"`. The signature is
  ecdsa-p256-sha256, 64-byte r‖s. Vivarium's refusals are 400
  `AppNotAuthorized` and `UserNotAuthorized`, and 401 `JwtReplayed`. The
  tests expect the same names from our host.
- Client attestation: `typ: atproto-client-attestation+jwt`, `iss = sub =`
  the client ID, `aud: {authority}#atproto_space_host`, at most 60 s,
  single use. It's signed by a key in the client's `jwks` (or `jwks_uri`),
  chosen by `kid`.
- Using a credential: `Authorization: Atproto-Space <credential>`, an
  `atproto-space-audience` header, and a signature over
  `("authorization" "atproto-space-audience")`. The audience is the
  authority's DID for host methods (`listRepos`, and our `app.eventside.space.*`
  API in the tests), and the repo's DID for reads at a writer's PDS. Vivarium
  checks a credential's `kid` header against the authority's DID document,
  and honors `#atproto_space`.
- `notifyWrite`: vivarium retries 408, 425, 429, 5xx and network errors for
  24 h with backoff, drops anything else, and times out after 5 s. A sealed
  box still reaches our host, because loopback and `--app-host` URLs bypass
  its upstream gate.
- `listRepos` answers `{cursor, repos: [{did, repoRev, hash: {$bytes}, spaceRev}]}`,
  and its cursor is an exclusive `spaceRev`.
- `notifyCredentialRevoked`: `{space, credentials: [1–100 jtis]}`. The
  service JWT has `iss` = the authority, `aud` = a DID hosted on that PDS,
  and `lxm`. Revocations are kept about 3,610 s. A revoked credential is
  refused with 401 `CredentialRevoked`.
- Minting on vivarium's PLC: `POST {PLC_URL}/{did}` with a signed genesis
  operation, checked by `@did-plc/lib`. A DID minted there counts as local,
  so vivarium reads its document without the upstream gate. It serves the
  document with `Multikey` methods (`{did}#atproto_space`) and services as
  `#atproto_space_host`. `/{did}/data` has the rotation keys.
- A writer's PDS finds us through `#atproto_space_host`, falling back to
  `#atproto_pds`. It accepts writes into spaces whose authority isn't one of
  its accounts.
- **No unconfirmed emails:** an account's email is set only by
  `com.atproto.server.createAccount {email}`, and `emailConfirmed` is `true`
  whenever there is one. `getSession` returns it whatever the grant's scopes.
  So TC-19 can't be automated against vivarium 0.0.2, and is manual (see
  TC-19).
- **No way to take one PDS down:** every test account shares one vivarium.
  TC-54 and TC-55 deactivate the admin's account instead
  (`com.atproto.server.deactivateAccount`), so their PDS refuses their repo
  (`RepoDeactivated`), and then reactivate it.
- `RepoNotFound` answers 400, not 404.

**The surfaces the tests pin.** Nothing in the design spelled these out, so
the red tests chose them, and the implementation follows them:

- **CLI:** `conference-server admin <command>`. It runs with the server's
  environment (`DATABASE_URL`, `PUBLIC_URL`, `ATPROTO_URL`, and so on) while
  the server runs. It exits non-zero with the reason on stderr when it
  refuses or fails, and with `--json` it prints one JSON object as its last
  line of stdout. The commands:
  - `org create --super-admin <handle> --recovery-key <did:key>` → `{did}`
  - `org show --org <did>` → `{admins: [{did, role, connected}]}`
  - `org admin add <handle> --org <did> [--role owner|staff]`, and
    `org admin remove <handle> --org <did> [--keep-admissions]` (TC-58)
  - `connect <handle>`: it prints a URL to open, and when the browser
    finishes, the page says "Connected, you can close this tab" and the CLI
    exits 0. `ADMIN_OAUTH_SCOPES` overrides `ADMIN_SCOPES`. An outdated grant
    gets a message containing "reconnect" and `connect <handle>`.
  - `reindex --org <did>`
  - `conference create --org <did> (--name … --starts … --ends … --city …
    [--description …] | --event <at-uri>) [--invite-only] [--theme <json>]
    [--super-admin <handle>]` → `{space, intake, event?}`. The theme is
    tokens without the `--g-` prefix, e.g. `{"color-primary": "#b0306a"}`.
  - `join set --conference <space> --methods code,request,list,open`
  - `codes issue --conference <space> (--shared <code> | --personal)
    [--expires <iso>] [--max-uses <n>]` → `{codes: […]}`. Codes are unique
    across conferences, since a code identifies its conference.
  - `list import <csv> --conference <space>`, with columns `handle,email,role`.
    Handles that don't resolve are listed in its output (`unresolved` with
    `--json`) and skipped.
  - `requests list --conference <space>` → `{requests: [{did}]}`, and
    `requests approve|deny <handle> --conference <space>`
  - `member add|remove|ban <handle> --conference <space>`, and
    `member role <handle> --role owner|staff|speaker|none --conference <space>`.
    `member add` (an admin admitting someone with no pending request) is
    new: TC-29, 51, 52 and 53 need it.
  - `apps add|remove <client-id> (--conference <space> | --org <did>)`.
    `--org` means the admin space, whose app access the intake spaces share.
    `apps open --conference <space>` warns with "any app" and exits non-zero
    unless given `--yes`. `apps curate` switches back to the list.
  - every action takes `--as <handle>`, defaulting to the super admin
  - `AUTHORITY_KEY_SECRET` has to be optional for tests and the e2e
    server. Without it, the server generates and stores a key, as it does
    for `OAUTH_SIGNING_KEY`.
- **Conference XRPC** (cookie; POSTs also need CSRF):
  - `getConference?conference=<event at-uri | space uri>`. It answers
    signed out for public conferences, and 404 for an invite-only one to
    non-members. The body is `{space, name, startsAt, endsAt, locations?,
    description?, theme?, viewer?: {member, role?, request?}}`, and never a
    denial reason.
  - `join {conference?, code?, request?}` → `{status: joined | pending |
    refused | emailNeeded, role?, canRequest?, verifyUrl?}`. An invalid,
    expired or used-up code is a 400 `InvalidCode`, whatever other methods
    are on, and creates nothing. `request: true` asks to join without the
    email step. Past the rate limit, it's a 429 `RateLimitExceeded`. A
    refused join writes no intake record.
  - `leave {conference}`, and `listMyConferences`
  - **New: `listRecords?conference=&collection=`**, the records members are
    served. It applies membership periods and the rules, and answers 403 to
    non-members. TC-25, 27, 29, 34 and 35 need it. It's an addition to the
    design's "Unchanged" XRPC list, so it needs approval.
- **Host API:** `app.eventside.space.listMembers` → `{members: [{did, read,
  write, periods: [{since, until?}]}]}`, past members included. And
  `app.eventside.space.getSpace` → `{readPolicy, writePolicy, appAccess,
  admins}`. Both take a credential, with the authority as audience.
- **Records:**
  - the sidecar `app.eventside.conference` has `space` and `superAdmin`
  - `app.eventside.conference.role` is `{subject, role}`, one per person
    with a role
  - `app.eventside.conference.rules` (rkey `self`) is `{rules: [{collection,
    writers: admins | members}]}`. It covers `app.eventside.block.card` and
    `app.eventside.conference.announcement` for admins, and
    `community.lexicon.calendar.event` (plans) and `app.eventside.chat.message`
    (chat, reserved for `chat`) for members.
  - `app.eventside.intake.join` is `{code?}`
- **PWA:**
  - routes: `/c/{actor}/{rkey}`, `/join/{code}`, and `/conferences` (the
    list of your conferences)
  - non-members see a code field labelled "code" (or an "Enter a code"
    button that shows it), "Join", and "Request to join"
  - members see a "Leave" button, which is how the tests know they're
    inside
  - the email step offers "Verify my email" and "Request to join"
  - a pending request says "pending", and a denial says the person "wasn't
    admitted"
  - a conference's theme sets `--g-color-primary` (and the other tokens) on
    `:root`

**Implementation refinements (2026-10-06).** Where the design left room,
the build chose:

- **Space settings are append-only snapshots.** Each change to a space's
  join methods, app access or policies adds a snapshot, and a join is
  judged by the settings in force at the join record's commit, not at the
  time we happen to process it.
- **Record timing comes from commit revs.** When a record entered or left a
  space is read from the writer's `com.atproto.space.listRepoOps` commit
  revs, not from `createdAt` or our clock.
- **Admin member records are decisions.** `app.eventside.admin.member`
  records are append-only decisions with TID rkeys. A removal is a decision
  with `until`; decisions rank by who made them (see "Precedence" and
  TC-52).
- **Codes are HMAC'd with the server key.** Join codes are stored only as
  HMACs under the server's secret (`AUTHORITY_KEY_SECRET`, or the generated
  one), like emails.
- **Pre-assigned roles admit people.** A role record from the conference's
  super admin (`app.eventside.conference.role`) is itself a way in: the
  person it names is admitted when they join, like a list entry.
- **Role and rules writes always go through the super admin's session.**
  Whoever runs `member role` or changes the rules, the CLI writes the record
  into the super admin's repo with the super admin's connected session, so
  only those records count (TC-35, TC-51).
- **Reindex reads writer sets and previously read repos.** `reindex` reads
  every repo the space's writer set names, plus every repo we've read from
  that space before, so records from people who have since left are still
  re-derived.
- **Rate limits.** `join` is limited per person (10 a minute) and per IP
  (30 a minute). The per-IP limits skip loopback addresses only when
  `PUBLIC_URL` is `http` (local development, which every test and the e2e
  server share). Behind a reverse proxy, `TRUSTED_PROXIES` names it, and a
  request through it counts against the address it last forwarded for.
- **Revocation on any lost access.** For each membership, role, ban or app
  access change, the host computes who can read before and after, and
  revokes the credentials of everyone who lost access, whatever the cause.
- **Join panel.** The code field sits behind an "Enter a code" button, with
  "Join" and "Request to join" beside it (the tests accept either form).
- **Left for later:**
  - Lexicon JSON for the new records (`app.eventside.conference.*`,
    `app.eventside.intake.*`, `app.eventside.admin.*`, `app.eventside.space.*`)
    isn't in `lexicons/` yet.
  - `org adopt` (bringing an existing account in as an organization) isn't
    implemented; `org create` mints a new authority.
- **Sign-in scopes.** `LOGIN_SCOPES` now includes the intake scope, so the
  attendee client ID carries `?scope=…`. attendee-sign-in's TC-7, TC-9 and
  TC-18 were amended to check the default list (`tests/support/scopes.ts`),
  as the user decided.
- **Vivarium.** E2e TC-8 needs vivarium's authorize pages to wrap the long
  client ID. That fix is on vivarium's `fix/wrap-long-client-id` branch,
  unreleased. CI pins `@vivarium-dev/cli` 0.0.2 from npm, so e2e TC-8 will
  fail in CI until a vivarium release with the fix is published and pinned
  here. Locally, `VIVARIUM_BIN` pointing at a build of that branch passes.

**Review round 1 design notes (2026-10-06).** The round-1 fixes go a
little past the approved design in these places, to raise at the PR:

- **Kept state grows.** The design says the only state to keep is the
  authorities' keys and super admins. Two more are needed to rebuild
  everything: the host's **writer set** (`space_writers`, from accepted
  write notifications), because the people who wrote join and leave records
  into an intake space can't be found from the super admin's repo; and the
  **first-seen times** of records (`space_record_seen`, next point).
  `reindex` keeps both, and rebuilds everything else, the `conferences`
  table included.
- **Joins count from when we first saw them.** A join or leave counts from
  its commit's revision, but never earlier than when our host first saw it.
  A writer's PDS picks its own revisions, so without this a self-hosted PDS
  could backdate a join past a code's expiry, back into an earlier open
  period, or ahead of others for a code's last use.
- **Role records name who assigned them.** `member role` and `list import`
  write role records with `assignedBy` (the acting admin), and imported ones
  with `via: "list"`. A role counts, and admits its subject, only while that
  admin could assign it: owner and staff roles by owners and the super
  admin, other roles by any admin. A role from the list admits only while
  the `list` method is on. `member role` refuses staff giving or taking
  owner and staff roles, and removing an admin deletes the roles they
  assigned. `rules set` needs an owner.
- **A conference's super admin must be an admin.** `conference create
  --super-admin` refuses anyone who isn't, since their writes into the space
  are taken only from members. A create that fails partway is undone.
- **Invite-only conferences stay hidden.** `listRecords`, `leave`, and a
  `join` naming the conference that no way in admits all answer 404
  (`ConferenceNotFound`) to non-members, as `getConference` does. A join by
  code alone still answers `InvalidCode`.
- **Admins can't leave** (400 `AdminCannotLeave`): they're members because
  they're admins.
- **App access changes revoke.** Taking an app off a space's list (or
  switching an open space back to its list) revokes that app's credentials,
  and our host re-checks a credential's user and app on every use.
  `registerNotify` registrations end with the access they were made with.
- **`AUTHORITY_KEY_SECRET` is required when `PUBLIC_URL` is `https`.** It
  stays optional for `http` (development and tests).
- **Write notifications revoke too.** A record that arrives by
  `notifyWrite` (an admin's ban from another app, a leave) revokes the
  credentials of whoever it took access from, like a CLI change.

**Review round 2 design notes (2026-10-06).** The round-2 fixes, to raise
at the PR alongside round 1's:

- **Admins' memberships are member decisions too.** `org admin add` writes a
  `member` record (by the super admin, `via: "admin"`) for each conference
  the new admin isn't already in, `conference create` writes one for each
  admin, and `org admin remove` ends each with a removal record. So an
  admin's period starts when they became an admin, keeps that start through
  a change of role, and ends (rather than vanishing) when they're removed:
  what they wrote while an admin stays listed. Removing an admin who was
  also an attendee ends that membership too; they can join again.
- **A conference's super admin must stay an admin.** `org admin remove`
  refuses the super admin of any conference, and their role and rules
  records count only while they're an admin.
- **Admins can't be banned or removed from a conference.** The index ignores
  bans whose subject is an admin or the super admin, and `member ban` and
  `member remove` refuse admins. `member remove` of someone with the owner
  or staff role needs an owner, as `member role` does.
- **Conference-space records can't be backdated far.** A record counts from
  its commit, but never more than 60 seconds before our host first saw it
  (an intake record, as before, never before). The slack keeps an ordinary
  write notification's delay from moving a record into a later period.
- **List handles are bound when they first resolve.** *(Removed in the simplification of 2026-10-07: list handles resolve at import, so there are no handle-only rows to bind.)*
- **Revocations are retried.** A revocation not yet delivered to every
  writer's PDS stays in an outbox (`space_credentials.revocation_sent_at`,
  migration 0004) that the server sends again every 30 seconds until it's
  delivered or the credential expires.
- **Reindex keeps what syncs alongside it wrote,** and never moves a repo's
  read position backwards.

**Review round 3 design notes (2026-10-06).** The round-3 fixes, to raise
at the PR alongside rounds 1 and 2:

- **The super admin's latest decision stands.** In a person's timeline, a
  later admission or removal by another admin that contradicts the super
  admin's latest decision about them (admitting, removing or denying) is
  ignored, until she decides again or the person acts themselves (joins,
  asks again, leaves). Email matches and admin periods are nobody's verdict,
  and end her say like the person's own join. The CLI reports such an
  ignored decision as a failure. (Generalized to ranks in the
  simplification of 2026-10-07: owners' decisions stand against staff the
  same way.)
- **The super admin is always an owner.** `org admin add` refuses to make
  her staff, and the index treats her as an owner whatever an `admin`
  record says. `org admin remove` of her (allowed once there's another
  owner, TC-8) only takes her off the list of admins: she stays the super
  admin, an owner, and a member of every conference.
- **A conference's super admin can be changed.** This was out of scope in
  the design, but round 2's refusal to remove a conference's super admin
  made it a trap. New: `conference super-admin <handle> --conference
  <space>` (the organization's super admin only; the new one must be a
  connected admin, and the old one connected too). The new super admin
  re-issues the role and rules records (and an invite-only conference's
  in-space event and sidecar) in their own repo, a settings snapshot names
  them, a public conference's sidecar is updated to name them, and the old
  super admin's copies are deleted. Roles keep the time they took effect
  (`since`). A public conference's event stays in the old super admin's
  public repo, where it was published.
- **Admin-only collections are judged at the record's time.** Admin
  periods are marked by the super admin's `member` records with
  `via: "orgAdmin"` (written by `org admin add` for a new admin, and by
  `conference create` for each admin) and `via: "orgAdminRemoved"` (by
  `org admin remove`, for every conference). A role counts from its
  record's `since` (written by every command that gives a role), else from
  when its record was dated. Limits: a role record holds only its current
  role, so once a role is changed or removed, what its holder wrote under
  the earlier role (and wasn't an admin for) is no longer shown; and a
  current admin with no `orgAdmin` mark counts from their `admin` record.
- **List roles come with late matches.** A list row's role is given when
  the row matches only at join time (a verified email; handles now match
  at import): a role record from the conference's super admin,
  `assignedBy` the importing owner, `via: "list"`, as an import's.
- **One record-judging rule.** `conference::rules::{may_write,
  record_counts}` is what `listRecords` serves by, for later features to
  judge ingest by. It dates a record by its commit, clamped to 60 seconds
  before our host first saw it. That first-seen time isn't published, so
  another app judging by commit revisions alone disagrees only on records
  whose commit claims to be more than a minute older than when it reached
  us.
- **Per-IP rate limits have their own map,** so a flood of addresses can't
  evict a person's own limit.

**Review round 4 design notes (2026-10-06).** The round-4 fixes, to raise
at the PR alongside rounds 1 to 3:

- **Only the super admin's records are neutral.** A `member` record with
  `via: "email"`, `"orgAdmin"` or `"orgAdminRemoved"` ends the super
  admin's say (round 3) only when it's hers; another admin's is their own
  decision, judged like any other.
- **An overridden ban still ended the membership then.** A ban the super
  admin overrides by admitting the person later now lasts from the ban
  until her admission: the period before it ends at the ban, joins and
  other admins' admissions in between don't count, and her admission
  starts a new period.
- **A conference's super admin must be an owner.** `conference create
  --super-admin` and `conference super-admin` refuse staff, and `org admin
  add --role staff` refuses to demote a conference's super admin. The
  index caps them too: role records count only as far as their role
  allows (no owner or staff roles from staff), and a staff one's rules
  don't count (the default rules apply).
- **Handing a public conference over again.** Its event and sidecar stay
  in the repo that first published them (named by the settings' `event`),
  and the sidecar is updated with that account's session (skipped with a
  note if it isn't connected). Everything is read before anything is
  written, and running `conference super-admin` again finishes a handover
  cut short: it updates the sidecar and the conference row if they're
  stale, and deletes every earlier super admin's leftover role, rules,
  event and sidecar records.
- **`member remove` and `ban` take a role away only once the person is
  out**, so a removal the super admin's decision overrides leaves it be.
- **A late-resolving list handle is bound however the person gets in.** *(Removed in the simplification of 2026-10-07: list handles resolve at import, so there are no handle-only rows to bind.)*
- **Revocation delivery per PDS** moves to another writer only when the
  refusal is about the writer (`RepoNotFound`, `AccountDeactivated` and
  the like, or a 403 "not hosted here"), tries at most 3 writers, starts
  with the one the PDS last took, and leaves a PDS that doesn't take
  revocations (404, 501, `MethodNotImplemented`) alone for 10 minutes.

**Review round 5 design notes (2026-10-06).** The round-5 fixes, to raise
at the PR alongside rounds 1 to 4:

- **A handover moves only the super admin's own records**: the roles, the
  rules (`self`), the sidecar (`self`) and, for an invite-only conference,
  the event that sidecar names. Every other calendar event in the space is
  a plan and stays where it is, the super admins' own included, and the
  cleanup of earlier super admins' records spares them too.
- **Revocations work through a PDS's writers in turn.** Up to 10 writers
  per PDS per pass (a writer's refusal is a cheap 4xx); when every one
  tried is refused as gone, the next pass starts after them, wrapping
  around, so a run of gone accounts can't keep a PDS from hearing of
  revocations. The cursor isn't persisted: after a restart it starts over,
  and the walk takes at most a few passes again.
- **Binding a late list handle is best-effort.** *(Removed in the simplification of 2026-10-07: list handles resolve at import, so there are no handle-only rows to bind.)*
- **Only the super admin's own admission lifts another owner's ban.** Her
  admin marks (`orgAdmin`, written by `org admin add` for every conference)
  and email matches don't, as they're neutral in the timeline too. `org
  admin add` doesn't refuse a banned person: admins can't be banned while
  they're admins, and once removed as an admin the ban stands again.
- **A role record counts only within its writer's own powers too**: naming
  an owner in `assignedBy` doesn't lift a staff conference super admin's
  cap.
- **A code alone is looked up by its HMAC.** The index keeps each code
  record's HMAC in a column (migration 0005), so a code-only join loads
  only the organizations with that code, and a wrong one loads none.
  `reindex` fills the column in for an index made before.

**Review round 6 design notes (2026-10-06).** The round-6 fixes, to raise
at the PR alongside rounds 1 to 5:

- **Binding a late list handle writes the role first.** *(Removed in the simplification of 2026-10-07: list handles resolve at import, so there are no handle-only rows to bind.)*
- **Intake spaces keep only joins and leaves.** Anyone can write to an
  intake space, so the index keeps only `app.eventside.intake.join` and
  `.leave` records there, of at most 4 KiB each and at most 1,000 per
  repo per intake space (a repo at the limit gets no new records; its
  existing ones still update). Everything else a writer puts there is
  dropped when read, by sync and by `reindex`, and the index reads intake
  spaces by those two collections only.
- **An admin period suspends a ban rather than lifting it.** Someone
  banned by an owner and later made an admin is a member from the super
  admin's `orgAdmin` mark until her `orgAdminRemoved` mark (a ban during
  the period too), so their admin-era records and `listMembers` period
  stay; the ban holds again from the removal.
- **Revocations skip writers a PDS refused as gone.** Instead of a rotation
  cursor per PDS, a writer a PDS refuses as gone is skipped there for an
  hour (and forgotten as the writer it last took); the last writer taken is
  remembered per PDS and space. Passes, concurrent or in other spaces,
  can't throw each other off. If every writer on a PDS is gone, it's tried
  again once the hour is up. Not persisted, as before.
- **`listRecords` and `listMembers` are paged.** `limit` (default 100, at
  most 500) and an opaque `cursor`: for `listRecords`, the last record's
  place in its order (when it counts from, revision, author, key); for
  `listMembers`, the last DID. A response has `cursor` only when there's
  more. `listRecords` still reads a collection's rows to judge and order
  them, but parses and sends only the page's values. The PWA reads the
  first page of announcements.
- **Codes are looked up by HMAC only in admin spaces.** The `code_hmac`
  column is set only for code records in an admin space, the lookup reads
  only those, and `codes issue` checks uniqueness through the same lookup
  instead of deriving every organization.
- **Migrations folded.** The review migrations (0003 to 0005) are folded
  into `0002_conference_space.sql`, since nothing has been deployed. A
  database made by an earlier build of this branch fails the migration
  checksum and has to be deleted (the worktree's git-ignored `data/` was).

**Review round 7 design notes (2026-10-07).** The round-7 fixes, to raise
at the PR alongside rounds 1 to 6:

- **A read that stores nothing changes nothing.** `sync_repo` says whether
  it changed the index; only then is the organization's generation bumped
  (so its view re-derived), and only then does a write notification compare
  access before and after for revocations (a read that failed partway still
  does). Writing a record's same revision again still counts as a change:
  a sync racing another (the CLI's and a write notification's) may have
  stored it before bumping, and the caller must not read a stale view.
- **Intake reads are bounded.** A read of one repo in an intake space stops
  after 1,000 ops or 4 MiB, and the next read goes on from there (ops come
  oldest first, and how far it read is kept). The per-repo record limit is
  checked against the repo's records read once per sync, not a count per
  op.
- **A ban of a marked admin is part of their history.** Bans are skipped
  only for admins the super admin's `orgAdmin` marks don't cover (her, and
  admins made before marks); for everyone else the timeline suspends a ban
  during an admin period and holds it outside one, whether or not they're
  still an admin. A current admin is never in `banned`.
- **A handle row's list role claims the row.** *(Removed in the simplification of 2026-10-07: list handles resolve at import, so there are no handle-only rows to bind.)*
- **Paging is checked after membership.** `listRecords` parses its cursor
  and limit only for members, so an invite-only conference stays a 404 to
  others. `limit` on `listRecords` and `listMembers` is parsed by us, so a
  non-number is an XRPC `InvalidRequest`.
- **Gone writers are skipped for half a credential's life** (5 minutes, not
  an hour), and a full memo of them forgets the expired (then the soonest to
  expire) rather than everyone.

**Review round 8 design notes (2026-10-07).** The round-8 fixes, to raise
at the PR alongside rounds 1 to 7:

- **Only what the view is derived from re-derives it.** One predicate,
  `index::derives_from(space, collection)` (everything in the admin space,
  joins and leaves in an intake space, roles and rules in a conference
  space), decides what `index::records` reads, what reindex rebuilds from,
  and which stored writes bump the generation and run the revocation
  comparison. Members' own records (plans, later chat) are still stored,
  but cost no re-derive.
- **The generation is bumped before a repo is marked read.** A sync running
  alongside that finds nothing new then never answers from a view older
  than what was stored. If the bump fails, the repo isn't marked read and
  the next sync reads it again.
- **A capped intake read goes on from the last commit read whole.** Every op
  in a commit has the commit's revision, so a read cut short at the caps
  marks the repo read only through the commit before the last one it saw.
  A single commit too big for the caps is skipped rather than read again
  for ever. Reindex reads intake repos uncapped, since it replaces what was
  there with what it read (bounded by the page loop, and the stored
  records by the per-repo limit).
- **A list row held by a role's claim is bound before the role changes.** *(Removed in the simplification of 2026-10-07: list handles resolve at import, so there are no handle-only rows to bind.)*
- **Memos make room a tenth at a time.** The revocation memos (gone
  writers, resting PDSes, last writer per PDS) share one helper: a full memo
  forgets the expired, then the tenth soonest to expire (least recently
  remembered, for the last-writer memo), never everyone at once.

**Review round 9 design notes (2026-10-07).** The round-9 fixes, to raise
at the PR alongside rounds 1 to 8:

- **Every role write goes through one helper, `conference::put_role`.** *(Removed in the simplification of 2026-10-07: list handles resolve at import, so there are no handle-only rows to bind.)* Role records are written directly again.
- **A removal or ban that takes a role checks first.** It resolves the
  conference super admin's session before the decision is written, so a
  super admin who isn't connected fails the command with nothing written.
  (Corrected in round 10: that check proves only that the conference super
  admin has a session, not that their PDS is up. With it down, the decision
  was written and the role kept; round 10 undoes the decision instead.)
- **Reindex reads intake repos capped, like a sync.** A read cut short at
  the caps deletes only the records up to where it stopped (`rev <=` its
  last whole commit) and keeps the rest as they were, and sets the repo's
  `synced_rev` to that point even if a sync had read further, so the next
  sync reads on from there (re-reading what's already stored changes
  nothing). This replaces round 8's uncapped reindex read, which an
  anonymous intake writer could make buffer up to 1000 pages of 4 MB.

**Review round 10 design notes (2026-10-07).** The round-10 fixes, to
raise at the PR alongside rounds 1 to 9:

- **`list import` doesn't replace a role it didn't give.** A row with a
  role is imported, but its role is written only when the person has no
  role or already has a list role, and isn't an admin. Otherwise their
  role stays, and the command lists them ("Kept the role they already
  had"). A list role admits only while the list does, so replacing an
  owner-given role could push someone out of a conference without a list,
  or demote them. The import ends with the same revocation pass as other
  commands.
- **A list row's claim moves only onto a role from the row's importer.** *(Removed in the simplification of 2026-10-07: list handles resolve at import, so there are no handle-only rows to bind.)*
- **A removal or ban whose role can't be taken is undone.** If deleting
  the role fails after the decision is written (say the conference super
  admin's PDS is down), the decision record is deleted again and the
  command fails with "nothing was changed". If the undo fails too, the
  error says the person is out but still holds their role, and to run the
  command again.

**Simplification (2026-10-07).** After round 10, the user chose to
simplify the two pieces of machinery later rounds kept finding holes in,
then review again (rounds 11 to 15). What changed:

- **List handles resolve at import.** `list import` resolves every handle
  row before writing anything, and stores it by DID (with the handle, for
  people reading the record). A row whose handle doesn't resolve is listed
  in the command's output ("Skipped these handles…", `unresolved` with
  `--json`) and skipped: nothing about it is stored, and the operator
  imports it again once it resolves. A bad handle or role in any row
  refuses the whole import with nothing written. Gone with it: handle-only
  rows, binding at join time (`onBehalfOf` entries), list claims on role
  records (`listHandle`), `put_role`, `keep_list_claim`, and the round 4 to
  10 notes about them. A `listEntry` counts only by its `did` (or its
  `emailHmac`), and only while the owner who wrote it is one. The email
  step still gives a matched row's role, as before.
- **Membership decisions rank by who made them.** The super admin, then
  owners, then staff. In a person's timeline, a decision stands against
  later ones of a lower rank that contradict it, until someone of its rank
  or higher decides again, or the person acts themselves (joins, asks
  again, leaves). Within a rank, the latest decision stands. That's round
  3's rule for the super admin, applied to owners too, and denials count
  from every admin, not only the super admin. Bans are unchanged (owners
  and the super admin ban; only the super admin's admission lifts another
  owner's ban). Staff can still invite: the people they admit (by
  `member add` or by approving a request) are full members. The CLI reports an overridden decision as a failure:
  "a decision about them by an owner or the super admin stands, and
  {admin} can't override it".
- **A former admin's decisions stop counting (TC-53), unless kept
  (TC-58).** `org admin remove <handle> --keep-admissions` works out who's
  a member now only on the departing admin's say-so (the organization as
  it would be without their `admin` record, compared with now), writes
  the remover's own `member` record for each (`via: "kept"`, `keptThrough`
  the departing admin), and then removes the admin. Those people are then
  members by the super admin's decision, which only she can override. The
  roles the departing admin assigned still go, as before. (Round 12: also on
  `org admin add --role staff` of an owner, and the people a plain removal
  takes out are named.)

**Review round 11 design notes (2026-10-07).** The round-11 fixes, to
raise at the PR alongside the simplification:

- **An overridden decision isn't left on record.** Decisions rank by their
  authors' roles now, so a refused one would take effect, unannounced, if
  its author were promoted or the overriding admin demoted. `member
  add|remove` and `requests approve` delete their own record again when a
  higher rank's decision (or a ban) overrides it, and fail saying nothing
  was changed. Adding or approving someone already banned is refused
  before anything is written.
- **A kept admission keeps the period it began.** `--keep-admissions`
  writes each kept `member` record (`via: "kept"`) with `since` the start
  of the membership it keeps, and the index dates the super admin's kept
  admission from that `since` (never after the record), so what the person
  wrote before the keep still counts.
- **`list import` checks every row before writing anything, and adds
  only what's new.** A bad role refuses the import even on a row whose
  handle doesn't resolve. A resolver that can't answer right now (not "no
  such handle") refuses the import with nothing written, rather than
  skipping the row as unresolved. A row the same owner already imported
  (same DID, email and role), or one repeated in the file, isn't written
  again, and the same list role isn't rewritten (it keeps its `since`).
  So importing the list again, as the command suggests for skipped rows,
  adds only what's new.

**Review round 12 design notes (2026-10-07).** The round-12 fixes:

- **Codes and list rows are an owner's records,** and count only while
  they're one (as the design's "a record counts only while its author is
  an admin" always implied). So removing an owner, or making them staff,
  can take out everyone who got in only by their code or list. Both `org
  admin remove` and `org admin add --role staff` (of an owner) now work out
  who that is before writing anything, and either keep them with
  `--keep-admissions` (now on both) or name them in the output, with how to
  let them back in. A plain removal still goes ahead (TC-53).
- **A command that keeps admissions and then fails takes them back.** The
  kept `member` records are deleted again if the admin's `orgAdminRemoved`
  marks, their `admin` record or the demotion can't be written, and the
  error names any that couldn't be. Deleting the roles a removed admin gave
  comes after their `admin` record is gone, when those roles count no
  more, so a failure there is reported rather than undoing anything.
- **Only the super admin can lift another owner's ban,** by `member add`,
  as before round 11: the refusal before writing is for everyone else.
- **A list role is taken over by a later import from another owner,** with
  `assignedBy` the new importer, so it doesn't go when the first owner
  does. Only the same role from the same owner is left alone.

**Review round 13 design notes (2026-10-07).** The round-13 fixes, and
one question for the PR:

- **A removed or demoted owner's bans and removals stop counting too,**
  which can let people back in. TC-53 makes that the rule ("a former
  admin's decisions stop counting"), so they aren't re-issued
  automatically, which would also raise them to the super admin's rank.
  Instead `org admin remove` and a demotion name everyone it lets back in
  or un-bans (`letBackIn` with `--json`), with how to keep them out.
  **For the PR:** whether an owner's bans should outlive their role is a
  product decision worth confirming.
- **A demotion finds every conference's writer before writing anything,**
  as removal does, so a conference super admin who isn't connected fails it
  with nothing changed.
- **A list role taken over by another owner's import keeps the time it took
  effect** when the role is the same. A different role doesn't replace
  another owner's list-given owner or staff role: it's kept and reported,
  as admin-given roles are. The first row for a person in a file decides
  their role.

**Review round 14 design notes (2026-10-07).**

- **Every change to an admin's role is checked for who it takes out or lets
  in,** not only removals and demotions: promoting staff to owner re-ranks
  their past decisions (a decision weighs by its author's role now), and
  re-adding a removed admin makes their old decisions, codes, list rows
  and bans count again. `org admin add` works out both before writing
  anything, keeps the people it would take out with `--keep-admissions`,
  and names them otherwise, along with anyone it lets back in. Ranking a
  decision by its author's role when it was made would avoid the re-ranking,
  but needs role history the `admin` record doesn't keep; worth raising
  with the PR's question about bans.
- **The let-back-in report says which kind:** members again at once, or
  only able to join again.
- **Removing an owner hands their list roles on.** A list role the owner
  gave that another current owner's list also gives (the same person and
  role) is rewritten as that owner's, keeping its time, instead of being
  deleted.

**Review round 15 design notes (2026-10-07).**

- **The super admin's admission that lifts a ban admits from then.** Her
  kept admission is dated from the start of the period it keeps, which can
  be before an owner's ban it lifts, so the lift itself now opens a period.
- **A list's roles count only while the owner who imported it is one,** as
  its rows do (a staff admin's list role no longer admits). Demoting an
  owner hands their list roles to another owner whose list gives the same
  person the same role, as removal does.
- **The reports of an admin change say what changed, not why:** "aren't
  members any more after this change to {who}", "are members again now",
  "aren't banned any more", with the super admin's commands as the remedy.
  The kept record names the admin change in `keptThrough`.

**Red-test gate (2026-10-06):** the user approved the red tests and the
surfaces they pin (see Build notes), with these contract changes:

- TC-52 now reads "Pim, a staff admin, and Kees, an owner", matching the
  design (only owners and the super admin can ban).
- TC-10 uses the super admin's DID and handle, since a minted authority has
  no handle.
- TC-19 is manual (vivarium can't make an unconfirmed email).
- TC-54 and TC-55 simulate an unreachable PDS by deactivating the account.
- A new conference route, `app.eventside.conference.listRecords
  {conference, collection}`, serves members the conference's records with
  membership periods and the rules applied (403 for non-members).
- CLI additions: `org show`, and `member add` (an admin admitting someone
  directly).

## Test cases

The cast:

- **Atmosphere** is an organization: a space authority DID our server
  minted. **Olga** is its super admin (and an owner), and **Pim** is staff.
  Both have connected their accounts as admins.
- **Ana and Bram** are attendees with atproto accounts.
- **Mallory** is a signed-in stranger.
- **AtmosphereConf** (Amsterdam, 29 April–2 May) is Atmosphere's public
  conference.
- **"Sanne & Joost's wedding"** is an invite-only conference run by a second
  organization, **Bruiloft** (super admin: Sanne), with a different theme.

"The CLI" is the admin command line. "Another app" is a second atproto
client on the conference's app list, used in tests to read the space the
way any other app would.

### Organizations and conferences

### ~~TC-1: Connecting an organization leaves no way to act as it in the browser~~

Dropped in design review round 3: the organization has no session any more; see TC-43.

### ~~TC-2: A connected organization stays connected while idle~~

Dropped in design review round 3: admins, not the organization, connect; see TC-44.

### ~~TC-3: An organization missing newly required permissions is reported, not dropped~~

Dropped in design review round 3: see TC-45, for admins.

### TC-4: Creating a public conference publishes an event other calendar apps can read

- **Given** Olga has connected as Atmosphere's super admin
- **When** the operator creates AtmosphereConf as a public conference
- **Then** a public calendar event for it exists in Olga's repository, with
  its name, dates and city, plus a settings record naming its space and Olga
  as its super admin
- **And** anyone can open its public page by that event's link

### TC-5: A conference can adopt an event the organization already published

- **Given** Olga already published a calendar event for AtmosphereConf
  through another calendar app
- **When** the operator creates the conference from that event
- **Then** the public page shows that event, and no second event is created

### TC-6: A conference can't adopt someone else's event

- **Given** an event published by Mallory's account
- **When** the operator tries to create an Atmosphere conference from it
- **Then** the CLI refuses, saying the event must be in the super admin's
  own repository

### TC-7: An invite-only conference publishes nothing

- **Given** Sanne has connected as Bruiloft's super admin
- **When** the operator creates "Sanne & Joost's wedding" as invite-only
- **Then** no public event or settings record for it exists in Sanne's
  public repository
- **And** opening it by its address without being a member shows nothing
  about it

### TC-8: The last owner can't be removed

- **Given** Olga is Atmosphere's only owner
- **When** the operator tries to remove her as owner
- **Then** the CLI refuses
- **And** after a second owner is added, removing Olga works

### Finding a conference

### TC-9: A non-member sees the public page and how to get in

- **Given** AtmosphereConf is public, with invite codes and requests turned
  on
- **When** Mallory opens its public page
- **Then** she sees its name, dates, city and description
- **And** she's offered "Enter a code" and "Request to join"
- **And** nothing from inside the conference is shown

### TC-10: The public page works by DID or by handle

- **Given** AtmosphereConf's public page link uses its super admin's DID
- **When** someone opens the same page using the super admin's handle in
  place of the DID
- **Then** they see the same conference

### Joining

### TC-11: Ana joins with a shared invite code

- **Given** AtmosphereConf has the shared code "atmosphere27"
- **When** Ana signs in, opens the public page and enters the code
- **Then** she's a member and sees the inside of the conference
- **And** her join is a record in her own repository, which Olga's
  repository didn't have to change for
- **And** another app asking our server who the members are sees her, with
  read and write access, from now

### TC-12: A wrong code doesn't admit anyone

- **When** Mallory enters a code that doesn't exist
- **Then** she's told the code isn't valid, and isn't a member

### TC-13: Expired and used-up shared codes stop working

- **Given** one shared code expired yesterday, and another was limited to
  two uses and has been used twice
- **When** Bram enters either code
- **Then** he's told the code isn't valid, and isn't a member

### TC-14: A personal code belongs to the first person who uses it

- **Given** a personal code issued for AtmosphereConf
- **When** Ana uses it, leaves, and uses it again
- **Then** she's admitted both times
- **And** when Bram tries the same code, it's refused

### TC-15: An invite link opens an invite-only conference

- **Given** Ana has the invite link for "Sanne & Joost's wedding"
- **When** she opens it and signs in
- **Then** she joins the wedding and sees its inside, with Bruiloft's theme
- **And** she never had to know or type the conference's address

### TC-16: Being on the attendee list by handle admits you on sign-in

- **Given** Atmosphere imported an attendee list that includes Ana's handle
- **When** Ana signs in and opens AtmosphereConf
- **Then** she joins with one tap, without a code

### TC-17: A handle on the list stays with the account it first named

- **Given** the list was imported with the handle `ana.example`, which then
  belonged to Ana
- **And** Ana later changed handles, and Mallory took `ana.example`
- **When** Mallory tries to join AtmosphereConf
- **Then** she isn't admitted from the list

### TC-18: Being on the list by email asks for a verified email, once

- **Given** the attendee list has Bram's email, and requests are turned on
- **When** Bram, signed in, tries to join
- **Then** he's offered "Verify my email" and "Request to join"
- **When** he chooses to verify and approves sharing his email at his PDS
- **Then** he's a member
- **And** his email isn't stored or shown anywhere in the app

### TC-19: An unconfirmed email doesn't match (manual)

- **Given** the attendee list has an email that Mallory's account uses but
  hasn't confirmed
- **When** Mallory verifies her email to join
- **Then** she isn't admitted from the list

Manual: vivarium 0.0.2 reports every email it holds as confirmed
(`emailConfirmed: true`), and has no way to give an account an unconfirmed
one. Check it against a PDS that does, or automate it once vivarium can.

### TC-20: Requesting to join, then being approved

- **Given** AtmosphereConf has requests turned on and no other way in for
  Bram
- **When** Bram taps "Request to join"
- **Then** he sees that his request is pending, and isn't a member
- **When** the operator approves his request
- **Then** the next time he opens the conference, he's a member

### TC-21: A denied request

- **Given** Bram's request is pending
- **When** the operator denies it
- **Then** he isn't a member, and sees he wasn't admitted, with no reason
  given

### TC-22: An open conference admits anyone signed in

- **Given** AtmosphereConf is open
- **When** Mallory taps "Join"
- **Then** she's a member

### TC-23: A conference with no way in for you refuses

- **Given** AtmosphereConf only admits people on its attendee list
- **When** Mallory tries to join
- **Then** she's told she can't join, and nothing is created for her

### TC-24: Too many join attempts are slowed down

- **Given** Mallory has tried 10 wrong codes in the last minute
- **When** she tries another
- **Then** she's told to wait, and the code isn't checked

### Inside, leaving, removal and bans

### TC-25: Non-members can't see inside

- **Given** Ana is a member of AtmosphereConf and Mallory isn't
- **When** Mallory asks for anything inside the conference
- **Then** she gets nothing
- **And** the space's own access rules refuse her too

### TC-26: Ana leaves, and can come back

- **Given** Ana is a member
- **When** she leaves AtmosphereConf
- **Then** she no longer sees its inside
- **And** another app asking our server sees her membership ended now
- **And** she can rejoin with any method that admits her, which starts a new
  membership period

### TC-27: A removed member loses access at once

- **Given** Bram is a member
- **When** the operator removes him
- **Then** his very next request for anything inside the conference gets
  nothing
- **And** another app asking our server sees his membership ended
- **And** an app that already held access through Bram can no longer read
  anyone's records in the space

### TC-28: A banned person can't get back in by any method

- **Given** Bram is banned from AtmosphereConf
- **When** he tries a valid shared code, is on the attendee list, and asks
  to join
- **Then** every attempt is refused
- **And** he never reappears in the space's member list

### TC-29: Records written while not a member are never shown

- **Given** Bram wrote a record into the conference space while he was
  removed, and was later let back in
- **When** the conference's records are listed for members
- **Then** that record isn't included, but his records from while he was a
  member are

### Roles and rules

### TC-30: Owners and staff are members with a role others can see

- **Given** Olga is Atmosphere's owner and Pim is its staff
- **When** AtmosphereConf is created
- **Then** both are members
- **And** another app reading the space sees Olga as owner and Pim as staff,
  from records Olga wrote

### TC-31: A speaker assigned before joining gets the role on joining

- **Given** Ana's handle is assigned the speaker role for AtmosphereConf
  before she has joined
- **When** she joins
- **Then** she's admitted without a code, as a speaker
- **And** another app reading the space sees her as a speaker

### TC-32: Changing and removing roles

- **Given** Ana is a speaker
- **When** the operator makes her staff, and later removes her from the
  conference
- **Then** another app first sees her as staff, then sees no role for her

### TC-33: The conference's rules say who may post what

- **When** AtmosphereConf is created
- **Then** another app reading the space finds its rules: cards and
  announcements only by owners and staff, plans and chat by any member
- **And** the rules can only come from Atmosphere

### TC-34: Records that break the rules aren't shown

- **Given** Ana, an attendee with no role, wrote an announcement into the
  space
- **When** the conference's records are listed for members
- **Then** her announcement isn't included

### TC-35: Roles and rules from anyone but the super admin are ignored

- **Given** Ana, a member, and Pim, a staff admin, each wrote a record into
  the conference space claiming new roles and new rules
- **When** roles and rules are read
- **Then** none of their records has any effect

### Which apps can read

### TC-36: By default, only eventside can read the space

- **Given** AtmosphereConf has the default (curated) app setting
- **When** another app, signed in as Ana, a member, asks to read the space
- **Then** it's refused
- **When** the operator adds that app to the conference's list
- **Then** it can read the space, as Ana

### TC-37: An open conference lets any app a member uses read

- **Given** the operator switched AtmosphereConf to open, after the CLI
  warned what that means
- **When** another app, not on any list, signed in as Ana, asks to read the
  space
- **Then** it can read it
- **And** it still can't read anything as Mallory, who isn't a member

### Keeping things in step

### ~~TC-38: Reconciling repairs a membership left half-done~~

Dropped in design review round 3: there is no copy to reconcile; the index is rebuilt from records (TC-50).

### ~~TC-39: A failed membership change isn't reported as done~~

Dropped in design review round 3: joining no longer writes to the organization's PDS; see TC-54 and TC-55.

### Two conferences, one person

### TC-40: Ana is in two differently branded conferences with one account

- **Given** Ana is a member of AtmosphereConf and of "Sanne & Joost's
  wedding"
- **When** she opens her list of conferences
- **Then** she sees both, each with its own name and theme
- **And** opening each shows its own inside and nothing from the other

### Our server as the space host

### TC-43: Connecting as an admin leaves no way to act as an admin in the browser

- **Given** Olga is Atmosphere's super admin
- **When** she runs the CLI's connect command and completes sign-in in a
  browser
- **Then** the browser shows "Connected, you can close this tab"
- **And** the browser isn't signed in to the app as Olga
- **And** the CLI reports Olga as a connected admin

### TC-44: A connected admin stays connected while idle

- **Given** Olga connected and hasn't used the app for longer than the
  attendee idle timeout
- **When** the operator runs a CLI command as Olga
- **Then** it works without connecting again

### TC-45: An admin missing newly required permissions is told to reconnect

- **Given** Pim connected before the admin permissions grew
- **When** the operator runs a CLI command as Pim
- **Then** the CLI says Pim needs to reconnect, naming the command
- **And** nothing Pim already wrote stops counting

### TC-46: Creating an organization gives it an identity that points at our server

- **When** the operator creates Atmosphere with Olga as super admin
- **Then** Atmosphere's DID document names our server as its space host and
  publishes a space key
- **And** the operator's recovery key comes before our server's in its
  rotation keys
- **And** Atmosphere's admin space exists, with Olga as owner

### TC-47: Another app reads a member's records with access from our server

- **Given** AtmosphereConf allows another app, and Ana has written a plan
  into the conference space
- **When** that app, signed in as Bram (a member), asks our server for
  access and reads the space
- **Then** it lists Ana among the space's writers and reads her plan from
  her own PDS

### TC-48: A non-member's writes don't enter the space

- **Given** Mallory isn't a member of AtmosphereConf
- **When** she writes a record into the conference space from her own PDS
- **Then** our server refuses her PDS's write notice
- **And** she doesn't appear among the space's writers

### TC-49: An app can't get access to a space for someone who isn't a member

- **When** another app, signed in as Mallory, asks our server for access to
  the AtmosphereConf space
- **Then** it's refused

### TC-50: Every permission can be rebuilt from the super admin

- **Given** AtmosphereConf has admins, codes, an attendee list, members who
  joined each way, a removal and a ban
- **When** the operator wipes the server's index and runs reindex
- **Then** every member, admin, ban, code, policy and app setting is the
  same as before

### TC-51: Staff can't override the super admin

- **Given** Olga banned Bram from AtmosphereConf
- **When** Pim, a staff admin, admits Bram, and also tries to switch the
  conference to open
- **Then** Bram is still banned, and the conference isn't open

### TC-52: Staff can't override an owner's decision

(Reworded 2026-10-07 with the user's approval; see the review log.)

- **Given** Pim and Lotte, staff admins, and Kees, an owner
- **When** Pim admits Bram and Kees then bans him
- **Then** Bram is banned
- **When** Kees removes Ana and Pim then admits her
- **Then** Ana still isn't a member
- **When** Kees admits her again
- **Then** she's a member
- **Given** Joost joined with a code, so no admin decided about him
- **When** Lotte removes Joost and Pim then admits him again
- **Then** he's a member: between staff, the latest decision stands

### TC-53: A former admin's decisions keep standing

(Reworded in design review round 5, approved 2026-10-07. Before: "A former
admin's decisions stop counting".)

- **Given** Pim, a staff admin, admitted Bram, and Ana joined with a code
  and was also admitted by Pim
- **When** Olga removes Pim as an admin
- **Then** Bram and Ana are still members
- **And** the CLI refuses any further decision made as Pim

### TC-54: Joining doesn't depend on the super admin's PDS

- **Given** Olga's PDS is unreachable, and AtmosphereConf has the shared
  code "atmosphere27"
- **When** Ana joins with the code
- **Then** she's a member

### TC-55: An admin action fails while that admin's PDS is down

- **Given** Pim's PDS is unreachable
- **When** the operator approves Bram's request as Pim
- **Then** the CLI reports that it failed, and Bram isn't a member
- **When** Pim's PDS is back and the approval is retried
- **Then** Bram is a member

### TC-56: Admins can read join records, attendees can't

- **Given** Ana and Bram joined AtmosphereConf with codes
- **When** an app signed in as Pim reads the conference's intake
- **Then** it sees both join records, with their codes
- **When** an app signed in as Ana tries the same
- **Then** it's refused

### TC-57: A denied request can be made again; a ban can't

- **Given** Bram's request was denied
- **When** he asks to join again
- **Then** his new request is pending
- **And** if he had been banned instead, it would be refused

### TC-58: An owner can undo a former admin's admissions

(Reworded in design review round 5, approved 2026-10-07. Before: "An owner
can keep the people a departing admin let in".)

- **Given** Pim, a staff admin, admitted Bram and Ana, Ana had also joined
  with a code, and Olga then removed Pim as an admin
- **When** Olga undoes Pim's admissions
- **Then** Bram is no longer a member
- **And** Ana still is, through her code

### Signed decisions

### TC-59: An admin record without our signature doesn't count

- **Given** Pim is a staff admin
- **When** Pim writes a record admitting Mallory into the admin space from
  another app
- **Then** Mallory isn't a member, and our host doesn't list her

### TC-60: A decision keeps its rank after its author is demoted

- **Given** Kees, an owner, banned Bram, and Kees was then made staff
- **When** Pim, a staff admin, admits Bram
- **Then** it's refused, and Bram is still banned
- **And** Kees, now staff, can't lift the ban either

### TC-61: Another app can check a decision for itself

- **Given** Olga admitted Bram
- **When** another app reads Olga's admission from the admin space
- **Then** its signature verifies against Atmosphere's DID document
- **And** the same record copied into Mallory's repository doesn't verify

### ~~TC-62: Deleting a decision doesn't undo it~~

Dropped in design review round 5: records are the source of truth, so
deleting a decision withdraws it.

### TC-63: Rotating the signing key keeps earlier decisions

- **Given** Olga admitted Bram
- **When** the operator adds a new signing key and re-signs the standing
  records
- **Then** Bram is still a member, and a new admission is signed with the
  new key
- **When** the operator then removes the old key
- **Then** Bram is still a member
- **And** a record signed only with the old key no longer counts

### TC-64: A join written by another app is only a request

- **Given** AtmosphereConf has the shared code "atmosphere27" and requests
  turned on
- **When** Ana writes a join record with that code from another app,
  without our signature
- **Then** her request is pending, and she isn't a member
- **And** if requests were off, she'd simply not be a member

### TC-65: Staff can issue codes

- **When** Pim, a staff admin, issues a shared code
- **Then** Bram joins AtmosphereConf with it

### Regressions

### TC-41: Signing in still works the same for attendees

- **Given** the organization sign-in has been added
- **When** Ana signs in, signs out, and her session expires as before
- **Then** every attendee sign-in case behaves exactly as it did

### TC-42: The block gallery still works signed out

- **When** someone opens the home page and the block gallery without signing
  in
- **Then** both work as before

## Review log

### Blocked during implementation (2026-10-06)

The implementation is complete in the build worktree, but uncommitted:
every conference-space test passes, and only four frozen
**attendee-sign-in** tests fail. They contradict this feature's approved
design, and a human has to decide.

- Joining writes `app.eventside.intake.join` into the attendee's repo in the
  intake space. Vivarium refuses that write unless the attendee's grant
  includes a `space:app.eventside.intake?…` scope. So `LOGIN_SCOPES` must
  include it, as the design says ("This feature adds it").
- attendee-sign-in's frozen tests assume the default scope list is exactly
  `atproto`:
  - `tests/integration/sign-in.test.ts`: TC-7 (`getSession` scopes equal
    `['atproto']`) and TC-9 (the sign-up PAR uses the bare client ID and
    `scope=atproto`)
  - `tests/integration/session.test.ts`: TC-18 (the initial scopes are
    `['atproto']`)
  - `e2e/sign-in.spec.ts`: TC-8. With a grown scope list, the client ID
    carries `?scope=…`, and vivarium's sign-up page doesn't wrap it, so
    "Continue" can't be clicked.
- attendee-sign-in's own design anticipated this ("When a later feature
  needs more, it adds its scopes to that list"). Its tests pinned the
  `atproto`-only default.

**Decision (2026-10-06):** the user approved amending attendee-sign-in's
three frozen integration assertions (TC-7, TC-9, TC-18) from "exactly
`atproto`" to "the default sign-in scope list", which now includes the
intake scope. Vivarium's sign-up page is to be fixed to wrap long client
IDs, so e2e TC-8 passes unchanged.

### Round 1

Reviewer: a fresh subagent following `adversarial-review`. Verdict: not
clean (0 blocking, 9 major, 5 minor, 3 nits). After the rework: `pnpm check`
green (lint, build, Rust 56 + unit 122 + integration 82 + tooling 33 + e2e 35),
and `scripts/check-tests-unchanged.sh` reports the frozen tests unchanged.

1. **[major] `member role` had no authorization, and roles admitted their
   subject whoever gave them.** Staff could make anyone owner; the role
   record admitted its subject, outlived the admin who gave it (defeating
   TC-53 by that path), and `list import` roles admitted even with `list`
   off.
   **Fixed:** role records carry `assignedBy` (and `via: "list"` from
   imports). A role counts and admits only while that admin may assign it
   (owner and staff roles by owners), and a list role only while `list` is
   on. `member role` refuses staff giving or taking owner and staff roles;
   removing an admin deletes the roles they gave; `rules set` needs an
   owner. Rust tests in `index.rs`, and `conference-review.test.ts` (TC-51,
   TC-53 regressions). See "Review round 1 design notes".
2. **[major] Changing app access didn't revoke.**
   **Fixed:** a change to a space's app access revokes the credentials of
   every app that lost it (by `client_id`, and unattested ones when a list
   is in force), and `credential::check` re-checks the user's and the app's
   access on every use. Covered by the TC-36 regression test.
3. **[major] Access changes arriving by `notifyWrite` never revoked.**
   **Fixed:** after syncing a notified repo, the host diffs readers and app
   access before and after, and revokes the difference, as the CLI does.
4. **[major] Admissions were judged by a PDS-chosen revision.**
   **Fixed:** an intake record counts from its commit's revision, but never
   earlier than when our host first saw it (`space_record_seen`, kept across
   reindex). Codes' expiry, settings snapshots and code-use order all use
   that time. Rust test `a_join_counts_from_when_it_was_first_seen_not_before`.
5. **[major] Rate limits were charged before verification, and lost behind
   a proxy.**
   **Fixed:** the per-DID `getSpaceCredential` limit and the `notify:{repo}`
   limit are counted only after the delegation or service auth verifies. The
   per-IP limits exempt loopback only when `PUBLIC_URL` is `http`, and
   `TRUSTED_PROXIES` takes the client's address from `X-Forwarded-For` when
   the peer is a trusted proxy (unit test `client_ip`).
6. **[major] Reindex emptied the index first and swallowed failures.**
   **Fixed:** reindex reads every repo first, from scratch, then replaces the
   organization's index in one transaction. A repo it can't read keeps its
   old records, and the command then exits non-zero naming it. If the super
   admin's repo can't be read, nothing changes.
7. **[major] A non-admin could be a conference's super admin.**
   **Fixed:** `conference create --super-admin` refuses anyone who isn't an
   admin of the organization (TC-4 regression test).
8. **[major] Reindex didn't rebuild `conferences`; the writer set is kept
   state.**
   **Fixed:** reindex rebuilds each conference's row from the super admin's
   settings and the event and sidecar (public, or inside the space), keeping
   a row whose event can't be read. The TC-50 regression test wipes
   `space_records`, `space_repos` and `conferences` for real and checks the
   rebuild. The writer set and first-seen times are recorded as required
   kept state in "Review round 1 design notes".
9. **[major] Every request re-derived the whole organization.**
   **Fixed:** the derived organization is cached per authority, keyed by an
   `index_generations` counter in the database that every index write bumps
   (the CLI's too), so a request costs one indexed lookup until the records
   change.
10. **[minor] Membership was checked before the delegation's signature.**
    **Fixed:** the delegation is verified before membership is checked or
    the per-DID limit is counted, so someone without a user's token learns
    nothing about them.
11. **[minor] Invite-only conferences were revealed by `listRecords` and
    `join`.**
    **Fixed:** to non-members, `listRecords`, `leave`, and a `join` naming
    the conference that nothing admits answer 404 like `getConference`
    (TC-7 regression test).
12. **[minor] Without `AUTHORITY_KEY_SECRET`, the sealing secret sat beside
    the sealed keys.**
    **Fixed:** the server refuses to start with an `https` `PUBLIC_URL` and
    no `AUTHORITY_KEY_SECRET`; it stays optional for `http`.
13. **[minor] `registerNotify` registrations survived revocation.**
    **Fixed:** registrations record the credential's delegator and client
    ID, and revoking either drops them.
14. **[minor] `conference create` wasn't atomic.**
    **Fixed:** everything is validated before the first write, and if a
    write fails, the ones before it are deleted again (public event and
    sidecar, and the settings record that makes the space exist), so a retry
    starts clean. The `conferences` row is saved before success is reported.
15. **[nit] Admins could "leave".**
    **Fixed:** `leave` answers 400 `AdminCannotLeave` for admins (TC-26
    regression test).
16. **[nit] `list import` split on commas.**
    **Fixed:** an RFC 4180 reader handles quoted fields (unit test).
17. **[nit] Two ideas of a conference's super admin.**
    **Fixed:** `Conference::super_admin()` is the one answer (its settings'
    super admin, else the organization's), used by the index, `listRecords`,
    reindex and the CLI.

### Round 2

Reviewer: a fresh subagent following `adversarial-review`. Verdict: not
clean (0 blocking, 5 major, 5 minor, 1 nit). After the rework: `pnpm check`
green (lint, build, Rust 62 + unit 122 + integration 85 + tooling 33 + e2e 35),
and `scripts/check-tests-unchanged.sh` reports the frozen tests unchanged.

1. **[major] Admin membership periods came from the current `admin` record
   alone.** Promoting Pim moved his start forward; removing him erased his
   period, so his past records vanished and `listMembers` dropped him.
   **Fixed:** admins' memberships are `member` decisions by the super admin,
   written when they're added (and for each admin when a conference is
   created) and ended by a removal record when they're removed. Rust test
   `an_admins_membership_keeps_its_start_and_its_end`, and
   `conference-review.test.ts` (TC-29, round 2). See "Review round 2 design
   notes".
2. **[major] A conference's super admin could be removed as an admin and
   still assign roles.**
   **Fixed:** `org admin remove` refuses a conference's super admin (give it
   another first), and the index and `listRecords` count their role and
   rules records only while they're an admin. Rust test
   `a_conference_super_admin_counts_only_while_an_admin`, integration TC-4
   (round 2).
3. **[major] Conference-space records were dated by the writer's PDS alone.**
   **Fixed:** `listRecords` dates a record by its commit, but never more than
   60 seconds before our host first saw it (`index::conference_us`), so a
   removed self-hosted member can't date a record back into their earlier
   period. Not the full clamp intake records get: there an ordinary
   notification delay could put a record written as a member after a quick
   removal (TC-29 writes, removes and re-admits within a second). Rust test
   `a_conference_record_cant_be_dated_long_before_it_was_seen`.
4. **[major] Reindex overwrote syncs that landed while it read, and reset
   `synced_rev`.**
   **Fixed:** the replacing transaction deletes only records first seen
   before reindex started (a concurrent sync's are newer, and kept, and
   `apply` never replaces a newer revision), and `synced_rev` only moves
   forward. The reindex command already revokes against the rebuilt index.
5. **[major] List handles unresolved at import weren't bound, cost a lookup
   each per join, and were admitted by the super admin's authority.**
   **Fixed:** a joiner's own handle is resolved (and checked back to their
   DID), two lookups whatever the list's length; a matching handle-only
   entry is bound with an entry carrying their DID, written as the super
   admin `onBehalfOf` the importing owner, which counts only while that
   owner is one. Bound handles are skipped from then on, and the join is an
   ordinary list join. Rust test
   `a_handle_on_the_list_is_bound_once_and_only_for_its_importer`.
6. **[minor] An undelivered revocation was never retried.**
   **Fixed:** revoked credentials carry `revocation_sent_at`, set once every
   writer's PDS has them; a background loop resends the rest every 30
   seconds until delivered or expired.
7. **[minor] Any owner could ban the super admin or another admin.**
   **Fixed:** the index ignores bans of admins and the super admin, and
   `member ban` refuses an admin. Rust test `admins_cant_be_banned`,
   integration TC-51 (round 2).
8. **[minor] `member remove` deleted roles without a role check.**
   **Fixed:** removing someone with the owner or staff role needs an owner,
   and admins can't be removed from a conference (remove them as admins).
   Integration TC-51 (round 2).
9. **[minor] The rate-limit map wasn't bounded, and IPv6 was keyed per
   address.**
   **Fixed:** at the cap, a new key evicts the one used longest ago, so the
   map never grows past it; per-IP limits count IPv6 by its /64. Rust test
   `rate_limits_stay_bounded_and_count_ipv6_by_its_64`.
10. **[minor] Removal could race credential issuance.**
    **Fixed:** after recording a credential, the host re-checks the user's
    and app's access against a fresh index, and revokes and refuses it if
    either was lost; any revocation after the row exists finds it.
11. **[nit] `conference create` didn't undo its later writes.**
    **Fixed:** the in-space event and sidecar, the admins' member and role
    records, and the rules are all pushed onto `undo`.

### Round 3

Reviewer: a fresh subagent following `adversarial-review`. Verdict: not
clean (0 blocking, 6 major, 2 minor, 1 nit). After the rework: `pnpm check`
green (lint, build, Rust 72 + unit 122 + integration 90 + tooling 33 + e2e 35),
and `scripts/check-tests-unchanged.sh` reports the frozen tests unchanged.

1. **[major] Another admin could undo the super admin's removal or
   denial.** Olga removing Bram and Pim then admitting him left Bram in.
   **Fixed:** the super admin's latest decision about a person stands
   against other admins' later contradicting decisions, until she decides
   again or the person joins, asks again or leaves (`index.rs` timeline,
   `By`). `member add`/`requests approve`/`member remove` report it when
   their decision is overridden. Rust test
   `the_super_admins_latest_decision_stands_against_other_admins`,
   integration TC-51 (round 3). See "Review round 3 design notes".
2. **[major] `org admin add <super admin> --role staff` demoted her.**
   **Fixed:** `org admin add` refuses to make the organization's super admin
   staff, and `admin_role` and the index's `role_of` answer owner for her
   first. `org admin remove` of her (which frozen TC-8 allows once there's
   another owner) only deletes her `admin` record. `org show` reports her
   as owner. Rust test
   `the_super_admin_is_always_an_owner`, integration TC-51 (round 3).
3. **[major] A conference's super admin could never be removed.**
   **Fixed:** new `conference super-admin <handle> --conference <space>`
   hands a conference to another admin: their re-issued role and rules
   records (and an invite-only one's event and sidecar), a settings
   snapshot naming them, the public sidecar updated, the old copies
   deleted. `org admin remove`'s refusal names the command. Integration
   TC-4 (round 3) hands a conference over, removes the old super admin, and
   checks a speaker keeps her role and the public page still works.
4. **[major] Admin-only collections were judged by current admin and role
   state.**
   **Fixed:** judged at the record's dated time: admin periods from the
   super admin's `orgAdmin`/`orgAdminRemoved` member records, roles from
   their `since` (else their dated time). Limits recorded in the design
   notes. Rust test `admin_and_role_periods_are_judged_at_the_time`,
   integration TC-34 (round 3).
5. **[major] A list row's role was lost when it matched only at join time.**
   **Fixed:** binding a late-resolving handle and matching a verified email
   both write the row's role record (as the conference's super admin,
   `assignedBy` the importer, `via: "list"`) before the admission.
   Integration TC-31 (round 3) imports a handle before its account exists.
6. **[major] Revocations to a PDS always went to one writer.**
   **Fixed:** each PDS's writers are tried in turn until one is accepted,
   and a 4xx refusal moves on at once instead of retrying the same writer.
   **Declined in part:** per-PDS delivery tracking. `revocation_sent_at`
   means "every writer's PDS has it", which is what it records; resending
   to a PDS that already has a revocation is harmless (revoking twice is a
   no-op), and the resend stops once every PDS has it or it expires.
7. **[minor] The record-judging rule lived inline in `listRecords`.**
   **Fixed:** `conference::rules::{may_write, record_counts}`, used by
   `listRecords`, with the first-seen dependency documented there and in
   the design notes. The other membership helpers the design lists for
   later features are already methods on `index::Conference` (`is_member`,
   `was_member_at`, `role_of`, `role_at`, `was_admin_at`); renaming them
   is left to the features that use them.
8. **[minor] A per-IP flood could evict per-person rate limits.**
   **Fixed:** per-IP limits live in their own map. The Rust test
   `rate_limits_stay_bounded_and_count_ipv6_by_its_64` floods it and checks
   a person's limit still holds.
9. **[nit] Wrong refusal name, and `listRecords` sorted by claimed rev.**
   **Fixed:** the post-insert re-check answers `AppNotAuthorized` when only
   the app lost access; `listRecords` sorts by each record's dated time.

### Round 4

Reviewer: a fresh subagent following `adversarial-review`. Verdict: not
clean (0 blocking, 4 major, 3 minor, 1 nit). After the rework: `pnpm check`
green (lint, build, Rust 77 + unit 122 + integration 95 + tooling 33 + e2e 35),
and `scripts/check-tests-unchanged.sh` reports the frozen tests unchanged.

1. **[major] Staff could pass a decision off as neutral.** A staff `member`
   record with `via: "email"` or `"orgAdmin"` readmitted someone the super
   admin removed, and `"orgAdminRemoved"` removed someone she admitted.
   **Fixed:** those vias are neutral only in the super admin's records;
   anyone else's are their own decisions. Rust test
   `only_the_super_admins_admin_and_email_records_are_neutral`.
2. **[major] `member remove` stripped the role of someone it didn't
   remove.** The role was deleted before the reload found the super
   admin's decision overriding the removal. **Fixed:** the role is deleted
   only once the reloaded index shows the person out (also for `ban`).
   Integration TC-51 (round 4).
3. **[major] A second handover of a public conference failed midway.** It
   looked for the sidecar in the current super admin's repo rather than
   the publisher's, after writing the copies and a snapshot, and a retry
   said "already its super admin". **Fixed:** the sidecar is read and
   written in the repo the settings' `event` names, with that account's
   session (skipped with a note if not connected), before anything is
   written; a rerun finishes a cut-short handover (sidecar, conference row,
   earlier super admins' leftover records). Integration TC-4 (round 4)
   hands a public conference over twice and reruns it.
4. **[major] A list row's role was lost when a code let the person in
   first.** **Fixed:** with the list on, a joiner not already in by role or
   list has their own handle checked against unbound rows and bound (with
   the role) whatever else admits them, members included. Integration
   TC-31 (round 4).
5. **[minor] An overridden ban was dropped from history.** **Fixed:** it
   ends the membership at the ban and lasts until the super admin's
   admission, which starts a new period. Rust test
   `a_ban_the_super_admin_overrode_still_ended_the_membership_then`.
6. **[minor] A staff conference super admin had owner powers over roles
   and rules.** **Fixed:** a conference's super admin must be an owner
   (`conference create --super-admin`, `conference super-admin`, and no
   demotion to staff while one), and the index caps a staff one to staff
   roles and ignores their rules. Rust test
   `a_staff_conference_super_admin_has_only_staff_powers`, integration
   TC-4 (round 4).
7. **[minor] Revocation fallback hammered a PDS that rejects everything.**
   **Fixed:** only writer-specific refusals move on to another writer, at
   most 3 per PDS, the last accepted writer first, and a PDS answering
   404/501/`MethodNotImplemented` rests for 10 minutes. Writers' DID
   documents were already cached (`did_document`, with a TTL), so resolving
   them each pass costs no lookups. Rust tests
   `only_a_refusal_of_the_writer_moves_on_to_another`,
   `a_pds_hears_from_a_few_writers_the_one_it_last_took_first`.
8. **[nit] The last-owner check ignored the super admin without an admin
   record.** **Fixed:** she counts among the owners. Integration TC-8
   (round 4).

### Round 5

Reviewer: a fresh subagent following `adversarial-review`. Verdict: not
clean (0 blocking, 3 major, 4 minor, 0 nit). After the rework: `pnpm check`
green (lint, build, Rust 80 + unit 122 + integration 100 + tooling 33 + e2e
35), and `scripts/check-tests-unchanged.sh` reports the frozen tests
unchanged.

1. **[major] A handover moved and deleted the super admins' plans.** It
   copied and deleted every calendar event of the old (and every earlier)
   super admin's in the space, but members' plans are calendar events too.
   **Fixed:** only roles, the `self` rules and sidecar and, for an
   invite-only conference, the event its sidecar names are copied and
   tidied away. Integration TC-4 (round 5), public and invite-only.
2. **[major] A PDS whose first writers were gone never got revocations.**
   At most 3 writers, always the lowest DIDs unless one had been accepted
   in this process. **Fixed:** up to 10 writers a pass, and when all of
   them are refused as gone the next pass starts after them (wrapping
   around); the one last accepted still goes first. Not persisted: after a
   restart the walk starts over and again reaches every writer within a
   few passes. Rust test
   `a_pds_whose_first_writers_are_gone_hears_from_the_later_ones`.
3. **[major] Binding a late list handle made code, open and member joins
   depend on the super admin's PDS.** **Fixed:** when the binding fails and
   the person is a member or a code or open conference admits them, they're
   let in and it's tried again on a later join; it fails the join only when
   the list is the only way in. Integration TC-54 (round 5).
4. **[minor] Someone let in by a role didn't get their list handle bound.**
   **Fixed:** binding is skipped only when they're on the list by DID.
   Integration TC-17 (round 5).
5. **[minor] `org admin add`'s mark lifted another owner's ban for good.**
   **Fixed:** only the super admin's own admissions lift a ban, not her
   admin marks or email matches. `org admin add` still accepts a banned
   person: admins can't be banned while admins, and the ban stands again
   once they aren't. Rust test `only_the_super_admins_own_admission_lifts_a_ban`.
6. **[minor] A staff conference super admin escaped the cap by naming an
   owner in `assignedBy`.** **Fixed:** a role counts only if both the named
   decider and the writer may assign it. Rust test
   `a_staff_conference_super_admin_cant_borrow_an_owners_powers`.
7. **[minor] A code-only join derived every organization.** **Fixed:** code
   records' HMACs are a column of the index (migration 0005, kept by every
   write and by `reindex`), so only organizations with that code are
   loaded. Integration TC-11 (round 5) covers two organizations, a wrong
   code, and an index from before the column after `reindex`.

### Round 6

Reviewer: a fresh subagent following `adversarial-review`. Verdict: not
clean (0 blocking, 2 major, 3 minor, 2 nit). After the rework: `pnpm check`
green (lint, build, Rust 85 + unit 122 + integration 101 + tooling 33 + e2e
35), and `scripts/check-tests-unchanged.sh` reports the frozen tests
unchanged.

1. **[major] A list role that failed to be written was never given.**
   Binding wrote the bound `listEntry` first, then the role as the
   conference's super admin; if that failed, later joins saw the person on
   the list by DID and never tried again. **Fixed:** the role is written
   first and the binding last. Integration TC-31 (round 6), with the
   conference super admin's PDS down.
2. **[major] Intake spaces stored anything anyone wrote.** Every
   collection, of any size, was kept and re-derived on each load.
   **Fixed:** only joins and leaves of at most 4 KiB, at most 1,000 per
   repo per intake space, are kept (sync and `reindex`), and the index
   reads intake spaces by those collections only. Rust test
   `an_intake_space_keeps_only_joins_and_leaves`.
3. **[minor] A banned person's admin-era membership vanished after their
   removal as an admin** (a regression from round 5 #5). **Fixed:** the
   super admin's admin marks bound a membership period that no ban cuts
   short; the ban holds again from the removal mark. Rust test
   `a_banned_persons_admin_period_ends_rather_than_vanishing`.
4. **[minor] The revocation writer rotation was keyed by PDS only.** Passes
   for different spaces (or concurrent ones) moved each other's offsets,
   and a gone last writer stayed first. **Fixed:** the rotation is
   replaced by remembering writers a PDS refused as gone (skipped there for
   an hour, and dropped as its last writer); the last writer is kept per
   PDS and space. Rust tests in `notify`.
5. **[minor] `listRecords` and `listMembers` weren't paged.** **Fixed:**
   `limit` (default 100, at most 500) and `cursor` on both; the frozen
   tests' data is well under a page. Rust tests
   `records_are_paged_newest_first_by_a_cursor` and
   `members_are_paged_by_did`.
6. **[nit] `codes issue` derived every organization; the code lookup read
   any space.** **Fixed:** `codes issue` uses the HMAC lookup, and the
   column is set and read only for admin-space code records. Rust test
   `only_an_admin_spaces_code_records_name_a_code`.
7. **[nit] Migrations 0003 to 0005 amended 0002 in the same PR, and 0005
   had no backfill.** **Fixed:** folded into 0002 (see "Review round 6
   design notes"); the worktree's local databases were deleted.

### Round 7

Reviewer: a fresh subagent following `adversarial-review`. Verdict: not
clean (0 blocking, 2 major, 2 minor, 2 nit). After the rework: `pnpm check`
green (lint, build, Rust 90 + unit 122 + integration 104 + tooling 33 + e2e
35), and `scripts/check-tests-unchanged.sh` reports the frozen tests
unchanged.

1. **[major] Junk in an intake space still cost a full re-derive.** It was
   no longer stored, but was still read unbounded, bumped the
   organization's generation and ran the revocation comparison, and the
   record limit was a `COUNT(*)` per op. **Fixed:** a sync that stores
   nothing doesn't bump or compare, intake reads stop after 1,000 ops or
   4 MiB (the next read goes on from there), and the limit is read once
   per sync. See "Review round 7 design notes". Rust test
   `only_a_write_that_changes_the_index_counts_as_a_change`; integration
   TC-48 (round 7).
2. **[major] A ban of a current admin vanished from their history.**
   Someone banned and later made an admin counted as a member during the
   ban, and a join written during it was honoured. **Fixed:** bans are
   skipped only for admins with no `orgAdmin` marks; a current admin just
   isn't listed as banned. Rust test
   `a_ban_holds_outside_an_admin_period_whether_or_not_they_are_still_an_admin`
   (fails on the old code).
3. **[minor] `listRecords` checked its cursor before membership,** so a bad
   cursor told a non-member an invite-only conference exists. **Fixed:**
   the cursor and limit are checked after. Integration TC-7 (round 7).
4. **[minor] A list role given without its binding left the row open.**
   The join answered failed though the role let the person in, and the
   unbound row could then admit the handle's next holder too. **Fixed:**
   the role names the row (`listHandle`), which makes its holder on the
   list and the row no one else's, and a failed binding is judged against
   the reloaded index. Rust test
   `a_handle_rows_list_role_claims_the_row_for_its_holder`; integration
   TC-31 (round 7).
5. **[nit] Gone writers were skipped for an hour, and a full memo was
   cleared wholesale.** **Fixed:** 5 minutes (half a credential's life),
   and the expired are evicted first. Rust test
   `a_full_gone_memo_forgets_the_expired_not_everyone`.
6. **[nit] A non-numeric `limit` got axum's plain-text 400.** **Fixed:**
   `limit` is parsed by us into an XRPC `InvalidRequest`, on `listRecords`
   and `listMembers`. Rust test
   `a_limit_that_isnt_a_number_is_an_xrpc_invalid_request`; integration
   TC-7 (round 7).

### Round 8

Reviewer: a fresh subagent following `adversarial-review`. Verdict: not
clean (0 blocking, 2 major, 2 minor, 2 nit). After the rework: `pnpm check`
green (lint, build, Rust 93 + unit 122 + integration 107 + tooling 33 + e2e
35), and `scripts/check-tests-unchanged.sh` reports the frozen tests
unchanged.

1. **[major] Every stored write in a conference space re-derived the
   view.** A member's plan bumped the generation and ran the revocation
   comparison, though no one's access depends on it. **Fixed:** one
   predicate, `index::derives_from`, decides what the index reads, what
   reindex rebuilds from and which writes count as a change; the rest are
   stored without a bump. See "Review round 8 design notes". Rust tests
   `the_index_derives_from_admin_records_joins_leaves_roles_and_rules_only`
   and `reindex_reads_only_what_the_index_derives_from`; integration TC-32
   (round 8), which fails on the old code.
2. **[major] A role change dropped a list row's claim.** `member role
   --role staff` rewrote the role without `listHandle`, and `--role none`
   deleted it, so the row was open to the handle's next holder again, and
   in a list-only conference the person was no longer on the list.
   **Fixed:** before the role is changed or taken (by `member role`, or by
   `member remove`/`ban`), a row held only by the claim is bound with the
   list entry a join writes, so it no longer depends on the role. Rust test
   `a_handle_rows_list_role_claims_the_row_for_its_holder` (extended);
   integration TC-17 (round 8) for `staff` and `none`, which fail on the
   old code.
3. **[minor] Reindex read intake repos capped, then deleted the rest.**
   **Fixed:** reindex reads uncapped (the cap is for write notifications).
4. **[minor] A repo was marked read before the view was bumped,** so a sync
   alongside could answer from a stale view. **Fixed:** the bump comes
   first; a failed bump leaves the repo unread for the next sync.
5. **[nit] A capped read could stop mid-commit and skip the rest of it.**
   **Fixed:** a read cut short is marked read only through the last commit
   it read whole (one commit bigger than the caps is skipped). Rust test
   `a_read_cut_short_goes_on_from_the_last_commit_read_whole`.
6. **[nit] Two memos still cleared wholesale; the gone memo's eviction was
   a pass per insert.** **Fixed:** one helper for all three: the expired,
   then the tenth soonest to expire, go. Rust test
   `a_full_gone_memo_forgets_the_expired_not_everyone` (extended).

### Round 9

Reviewer: a fresh subagent following `adversarial-review`. Verdict: not
clean (0 blocking, 2 major, 1 minor, 1 nit). After the rework: `pnpm check`
green (lint, build, Rust 93 + unit 122 + integration 111 + tooling 33 + e2e
35), and `scripts/check-tests-unchanged.sh` reports the frozen tests
unchanged.

1. **[major] Two more role writes dropped a list row's claim.** `org admin
   add` wrote `{subject, role}` in every conference, and a re-import of
   someone under a new handle wrote their list role without `listHandle`,
   so the old handle's row reopened to its next holder. **Fixed:** every
   role write goes through `conference::put_role`, which carries the claim
   onto a list role and binds the row before any other role; see "Review
   round 9 design notes". Integration TC-17 (round 9) for `org admin add`
   and for a re-import, which fail on the old code.
2. **[major] Reindex read intake repos uncapped**, so any anonymous intake
   writer could make it buffer up to ~4 GB per repo. **Fixed:** reindex
   reads them capped like a sync; a read cut short keeps the records past
   where it stopped and sets `synced_rev` there, so later syncs read on and
   nothing is lost (round 8's concern). Integration TC-48 (round 9), which
   fails on the old code (reindex read past the cap), and fails too on a
   capped read that deletes the rest (the join past the cap is gone).
3. **[minor] `member remove`/`ban` wrote the decision before binding the
   row,** so without the organization's super admin the command failed
   with the person already removed and their role still there. **Fixed:**
   the conference super admin's session is resolved and the row bound
   before anything is written; a failure says nothing was changed.
   (`member role` already bound before writing.) Integration TC-17
   (round 9), which fails on the old code.
4. **[nit] Rewriting a version at the same revision counts as a change.**
   **Declined:** that's what keeps two syncs racing over the same ops
   correct. With `<` (or `<` plus a CID check), the sync that loses the
   race to store a record reports no change and returns at once, before
   the winner has bumped the generation, so its caller (a join waiting for
   its own write) answers from the old view: the round 7 TC-29 failure.
   Bumping before `synced_rev` (round 8) doesn't help, since the loser
   never gets as far as either. The extra bump only happens when ops are
   read twice (a commit cut short, or overlapping syncs), and costs a
   re-derive, not correctness.

### Round 10

Reviewer: a fresh subagent following `adversarial-review`. Verdict: not
clean (0 blocking, 2 major, 1 minor, 1 nit). After the rework: `pnpm check`
green (lint, build, Rust 93 + unit 122 + integration 114 + tooling 33 + e2e
35), and `scripts/check-tests-unchanged.sh` reports the frozen
tests unchanged. No reviewer has seen the round 10 fixes.

1. **[major] `list import` replaced any role with a list role.** It
   overwrote the role record whatever it held. A list role admits only
   while `list` is a join method, so importing someone an owner had given a
   role, into a conference without a list, pushed them out. It also
   silently demoted owner- and staff-given roles, replaced an org admin's
   own conference role, and never ran the revocation pass. **Fixed:** the
   list's role is written only when the person has no role or already has
   a list role, and isn't an admin; otherwise their role stays and the
   command lists them. The import ends with `after_change`. See "Review
   round 10 design notes". Integration TC-32 (round 10), which fails on the
   old code.
2. **[major] Round 9's `put_role` moved a list row's claim onto another
   owner's role.** When Sem re-imported Zoe, the claim from Kees's row
   moved onto Sem's list role. `org admin remove sem` deletes the roles Sem
   assigned (and they stop counting anyway), so Kees's row reopened to the
   handle's next holder: TC-17 again. **Fixed:** the claim is carried only
   onto a list role assigned by the row's own importer. Any other role is
   written after the row is bound. `org admin remove` also binds the row
   of every role it's about to delete, before writing anything.
   Integration TC-17 (round 10), which fails on the old code.
3. **[minor] A removal or ban could be left half done.** `decide`'s
   up-front check proved only that the conference super admin had a
   session, not that their PDS was up. With it down, the removal was
   written, deleting the role failed, and the person kept the role, which
   let them back in. **Fixed:** when the role can't be deleted, the
   decision record is deleted again, and the command fails saying nothing
   was changed. If that undo fails too, the error says exactly what state
   is left. Round 9's design note is corrected. Integration TC-55
   (round 10), which fails on the old code.
4. **[nit] `conference_super_admin` ignored its `org` parameter.**
   **Fixed:** the parameter is gone.

### Blocked after round 10

The pipeline allows ten review rounds. None came back clean, so this needs
a human decision before shipping.

| Round | Blocking | Major | Minor | Nit |
|-------|----------|-------|-------|-----|
| 1 | 0 | 9 | 5 | 3 |
| 2 | 0 | 5 | 5 | 1 |
| 3 | 0 | 6 | 2 | 1 |
| 4 | 0 | 4 | 3 | 1 |
| 5 | 0 | 3 | 4 | 0 |
| 6 | 0 | 2 | 3 | 2 |
| 7 | 0 | 2 | 2 | 2 |
| 8 | 0 | 2 | 2 | 2 |
| 9 | 0 | 2 | 1 | 1 |
| 10 | 0 | 2 | 1 | 1 |

**What the rounds found:**
- No round found anything blocking. The majors fell from 9 to 2 and have
  stayed at 2 for five rounds.
- Later rounds mostly found holes in two pieces of machinery: admin
  precedence (whose decision stands, and roles that count only while their
  assigner may assign them), and handle-only list rows with their claims
  and bindings. A previous round's fix often opened the hole:
  - Round 5's three majors were regressions from round 4's fixes.
  - Round 9 #2 reopened round 7's intake abuse through round 8's uncapped
    reindex.
  - Round 10 #2 reopened TC-17 through round 9's `put_role`.
  - Rounds 7, 8, 9 and 10 each found another way for a list row's claim to
    be lost.
- The other areas have held since they were fixed: the space host
  protocol, credentials, revocation, and the intake caps.

**Current state:** all frozen tests and every review regression test pass
(Rust 93, unit 122, integration 114, tooling 33, e2e 35), and the frozen
tests are unchanged. `pnpm check` is green with the round 10
fixes against the locally built vivarium. CI still needs vivarium 0.0.3
for attendee-sign-in's e2e TC-8.

**The choices:**
- **(a) Ship as is,** with the round 10 fixes unreviewed.
- **(b) Run more review rounds.** Recent rounds suggest each will find one
  or two more holes in the same machinery.
- **(c) Simplify the model before shipping.** For example, require list
  handles to resolve at import, which drops handle-only rows and the whole
  claim and binding machinery. And/or drop cross-admin precedence, so only
  owners or the super admin write membership decisions. Either would
  remove most of the code that later rounds found holes in.

**Decision (the user, 2026-10-07):** (b) and (c) together. Up to five more
review rounds (through round 15), after simplifying:
- **List handles must resolve at import.** A row whose handle doesn't
  resolve is reported and skipped, and the operator re-imports it later.
  Handle-only rows, late binding and list claims go away.
- **Staff can't override an owner's or the super admin's membership
  decision,** but staff can invite people, and the people they invite get
  full membership rights.

Frozen TC-52's last step (Pim, staff, re-admits Ana after Kees, an owner,
removed her, and she's a member) contradicts the second point. The
replacement wording needs the user's approval before the build resumes.

**Approved (the user, 2026-10-07):** TC-52 is reworded as above, and TC-58 is
new. The `frozen-tests` check now warns rather than fails on changed frozen
files (main, 30c53bf), so the TC-52 change lands as its own commit,
`test(conference-space): TC-52 staff can't override an owner (approved)`,
changing only TC-52's test in `tests/integration/conference-admins.test.ts`.
TC-58 is a new test, shown red before it's implemented. TC-53 is unchanged.
`org admin remove <handle> --keep-admissions` writes the remover's own
`member` records for everyone admitted only on the departing admin's say-so,
then removes them. The build then resumes with the simplification above and
review rounds 11 to 15.

### Round 11

Reviewer: a fresh subagent following `adversarial-review`, the first to see
the simplification. Verdict: not clean (0 blocking, 2 major, 2 minor, 1
nit). It confirmed the TC-52 change matches its approved wording and is the
only changed frozen file. After the rework: `pnpm check` green (lint, build,
Rust 94 + unit 122 + integration 108 + tooling 33 + e2e 35), and
`check:frozen` warns only about `tests/integration/conference-admins.test.ts`
(the approved TC-52 change).

1. **[major] A decision reported as overridden stayed on record,** and
   counted later if its author was promoted or the overriding owner
   demoted (ranks use authors' current roles). **Fixed:** `decide` deletes
   its record again and fails with "nothing was changed"; a banned person
   is refused before anything is written. Integration TC-52 (review round
   11), which fails on the old code.
2. **[major] `--keep-admissions` restarted the kept membership at the
   keep,** so what the person wrote before it stopped being shown.
   **Fixed:** the kept record's `since` is the start of the kept period, and
   the index honours it for the super admin's `via: "kept"` records. Rust
   test `a_kept_admission_keeps_the_period_it_began`; integration TC-58
   (review round 11), which fails on the old code.
3. **[minor] An unresolved row skipped the role check,** so a bad role
   didn't refuse the import. **Fixed:** every cell is checked before the
   handle is resolved. Integration TC-16 (review round 11).
4. **[minor] Re-importing duplicated every row and reset list roles'
   `since`, and a resolver hiccup read as "doesn't resolve".** **Fixed:**
   rows the same owner already imported aren't written again (their role
   is still given if it's missing), the same list role isn't rewritten,
   and a resolver error other than "no such handle" refuses the import.
   Integration TC-16 (review round 11).
5. **[nit] The keep summary counted (conference, person) pairs and showed
   DIDs.** **Fixed:** it counts people, and shows handles by conference.

### Round 12

Reviewer: a fresh subagent following `adversarial-review`. Verdict: not
clean (0 blocking, 3 major, 1 minor, 2 nit). It confirmed the TC-52 change
is still the only changed frozen file. After the rework: `pnpm check`
green (lint, build, Rust 94 + unit 122 + integration 111 + tooling 33 + e2e
35), and `check:frozen` warns only about
`tests/integration/conference-admins.test.ts`.

1. **[major] Round 11's banned pre-check stopped the super admin lifting
   another owner's ban.** **Fixed:** the check before writing applies to
   everyone but her. Integration TC-51 (review round 12).
2. **[major] Round 11's "same list role isn't rewritten" kept another
   owner's role,** which then went when that owner did. **Fixed:** only the
   same role from the same owner is left alone; otherwise the importer
   takes it over. Integration TC-32 (review round 12).
3. **[major] Removing or demoting an owner silently took out everyone who
   got in by their code or list.** **Fixed, as far as the frozen tests
   allow:** a plain removal still goes ahead (TC-53 expects it), but both
   removal and demotion now name the people it takes out, and both take
   `--keep-admissions`. Requiring a choice when the count isn't zero would
   break TC-53's plain removal. Integration TC-53 (review round 12).
4. **[minor] A failed `--keep-admissions` run left its kept records.**
   **Fixed:** they're deleted again when the removal or demotion fails
   before the admin record goes, and the error names any that couldn't
   be. Role tidy-up after the admin record is gone only reports failures.
   Not covered by an integration test: every write involved is the super
   admin's, so her PDS can't fail the later writes without failing the
   first.
5. **[nit] `list import` printed raw DIDs for kept roles.** **Fixed:**
   handle and DID.
6. **[nit] A row repeated in one file rewrote its role each time.**
   **Fixed:** roles given in the run are tracked.

### Round 13

Reviewer: a fresh subagent following `adversarial-review`. Verdict: not
clean (0 blocking, 1 major, 2 minor, 1 nit). It confirmed the TC-52 change
is still the only changed frozen file, and found no regressions from the
round-12 fixes. After the rework: `pnpm check` green (lint, build, Rust 94 +
unit 122 + integration 113 + tooling 33 + e2e 35), and `check:frozen` warns
only about `tests/integration/conference-admins.test.ts`.

1. **[major] Removing or demoting an owner silently lifted their bans and
   removals,** letting people back in. **Fixed by reporting, re-issuing
   declined:** TC-53 (approved) says a former admin's decisions stop
   counting, and a ban is one; re-issuing them as the super admin's would
   contradict it and raise them to her rank. Both commands now name the
   people let back in or un-banned, with how to keep them out, and the
   design notes flag the question for the PR. Integration TC-28 (review
   round 13).
2. **[minor] A list role taken over by another owner reset its `since`,
   and a different role replaced another owner's list-given staff role.**
   **Fixed:** the same role keeps its time; a different one leaves an owner
   or staff list role alone and reports it. Integration TC-34 (review
   round 13).
3. **[minor] A demotion resolved conference writers after writing the
   admin record.** **Fixed:** all are resolved first. Not covered by an
   integration test: the helpers can't make a conference super admin who
   isn't connected.
4. **[nit] With two rows for one person in a file, which role won depended
   on what was stored.** **Fixed:** the first row decides.

### Round 14

Reviewer: a fresh subagent following `adversarial-review`. Verdict: not
clean (0 blocking, 2 major, 2 minor, 1 nit). It confirmed the TC-52 change
is still the only changed frozen file. After the rework: `pnpm check`
green (lint, build, Rust 94 + unit 122 + integration 116 + tooling 33 + e2e
35), and `check:frozen` warns only about
`tests/integration/conference-admins.test.ts`.

1. **[major] Promoting staff to owner silently re-ranked their decisions,**
   which could take someone out or let them in. **Fixed:** every `org admin
   add` works out who it takes out (kept with `--keep-admissions`, named
   otherwise) and who it lets in (named). Integration TC-52 (review round
   14).
2. **[major] Re-adding a removed owner silently revived their bans,
   removals, codes and list rows.** **Fixed the same way:** the re-add
   names everyone it takes out. Counting only decisions made while their
   author was an admin was considered; it would also need the role at the
   time, which isn't recorded, so it's raised for the PR with round 13's
   question. Integration TC-28 (review round 14).
3. **[minor] Removing the owner whose import last took over a list role
   deleted it,** though another owner's list still gave it. **Fixed:** such
   a role is handed to that owner, keeping its time. Integration TC-32
   (review round 14).
4. **[minor] attendee-sign-in's frozen e2e TC-1 can flake under load,**
   checking the avatar's `naturalWidth` straight after it's visible.
   **Declined here:** it's another feature's frozen test, unchanged by this
   branch, and passed in every run of this build's `pnpm check`. Noted for
   the PR as a known flake to fix on `main` through attendee-sign-in.
5. **[nit] The let-back-in report didn't say who was a member again at
   once.** **Fixed:** it reports members again and those only able to join
   again separately (`member` in the JSON).

### Round 15

Reviewer: a fresh subagent following `adversarial-review`. Verdict: not
clean (0 blocking, 1 major, 3 minor, 1 nit). It confirmed the TC-52 change
is still the only changed frozen file. After the rework: `pnpm check`
green (lint, build, Rust 95 + unit 122 + integration 118 + tooling 33 + e2e
35), and `check:frozen` warns only about
`tests/integration/conference-admins.test.ts`. No reviewer has seen the
round 15 fixes.

1. **[major] Re-adding an owner with `--keep-admissions` reported keeping
   someone their old ban took out, but didn't.** The kept admission was
   dated before the ban it lifted, and the lift opened no period.
   **Fixed:** the super admin's ban-lifting admission opens a period at the
   lift. Integration TC-58 (review round 15).
2. **[minor] The admin-change reports blamed the wrong admin for
   promotions and re-adds, and their remedies didn't work for staff.**
   **Fixed:** they say what changed, and name the super admin's commands;
   `keptFrom` became `keptThrough`.
3. **[minor] Demoting an owner dropped list roles another owner's list
   also gives.** **Fixed:** the round-14 handover is shared with demotion.
   Integration TC-32 (review round 15).
4. **[minor] A demoted owner's list-given speaker roles kept admitting.**
   **Fixed:** a list role counts only while its importer is an owner. Rust
   test `a_list_role_counts_only_while_its_importer_is_an_owner`;
   integration TC-32 (review round 15).
5. **[nit] The handover resolved the conference super admin twice.**
   **Fixed.**

### Blocked after round 15

The decision after round 10 allowed review rounds through 15. Round 15
wasn't clean, so this needs a human decision again.

| Round | Blocking | Major | Minor | Nit |
|-------|----------|-------|-------|-----|
| 11 | 0 | 2 | 2 | 1 |
| 12 | 0 | 3 | 1 | 2 |
| 13 | 0 | 1 | 2 | 1 |
| 14 | 0 | 2 | 2 | 1 |
| 15 | 0 | 1 | 3 | 1 |

**What keeps recurring.** The simplification worked for list matching:
no round since has found a hole in handle resolution or list rows by DID.
Almost every major since round 12 is one family: **changing an admin's
role silently changes other people's memberships.** It comes from two
rules together:

- a record counts only while its author is an admin of the right role:
  decisions, codes, list rows, bans and list roles;
- a decision weighs by its author's role now, not when it was made.

So removing, demoting, promoting or re-adding an admin re-derives everyone
they ever decided about. Rounds 12 to 15 each found another direction
(taken out, let back in, re-ranked, revived) or another record kind. The
fixes made `org admin add|remove` diff the organization before and after,
report both directions, and keep the lost with `--keep-admissions`. Each
fix exposed the next edge, as with the list claims before.

**Current state:** every frozen test and review regression passes, and
`pnpm check` is green against the locally built vivarium. CI still needs
vivarium 0.0.3 for attendee-sign-in's e2e TC-8.

**The choices:**
- **(a) Ship as is,** with the round 15 fixes unreviewed. The admin-change
  reports name everyone affected, so nothing changes unannounced.
- **(b) Decide the rule, then review once more.** For example:
  - judge each record by its author's role *when it was written*, kept as
    role periods (like the existing `orgAdmin` marks), so later role
    changes re-rank nothing;
  - or make codes, list rows and bans belong to the conference once
    written, outliving their author's role.

  Either removes the re-derivation that later rounds keep finding edges
  in, but it changes what TC-53 and TC-58 mean.
- **(c) Narrow the feature:** only the super admin and owners decide
  memberships (staff only invite by approving requests), and changing an
  admin's role takes effect only for new records.

**Decision (the user, 2026-10-07):** revisit the approach instead. A
decision or action is checked when it's taken and stays valid, using signed
records. The feature goes back to design review (round 5, above). The build
resumes after the user approves the design and the test-case changes.

**Build resumes after design review round 5** (approved 2026-10-07). The
approved test-case changes are TC-53 (frozen, amended), TC-58 (reworded,
not frozen) and the new TC-59 to TC-65 (TC-62 dropped). Review rounds 16
and 17 follow, as proposed to the user in round 5. If round 17 isn't clean,
set `status: blocked`.

**Round 5 red tests approved** by the user on 2026-10-07 ("approve, let's
build"):
- `27c3ec4` amends frozen TC-53.
- `acdcbc5` adds TC-58 to TC-65.

The freeze can't be re-pointed, so `acdcbc5` is a second freeze, enforced
by hand: implementation and review also run
`scripts/check-tests-unchanged.sh acdcbc5`, and any change it reports is
blocking.

The non-frozen review tests "TC-53: a role a former admin gave stops
admitting (review round 1)" and "TC-32: demoting an owner hands their list
roles on, or they stop counting (review round 15)" contradict round 5. The
implementation deletes or rewrites them.

Then implementation, with review rounds 16 and 17.

### Implementation of design round 5

Signed decisions are built as the round 5 design and its refinements say.
Choices the design left open, and what changed in the tests:

- **Signing** is implemented in `spacehost/attest.rs` (DAG-CBOR in
  `crypto.rs`) rather than with the `atproto-attestation` crate: the crate
  brings its own atproto stack for what is a small encoder and one
  signature, and this one is checked against `tests/support/attestation.ts`
  by TC-61. A unit test checks the encoder makes the bytes the authority's
  `did:plc` was made from.
- **Readers verify against our copy of the DID document's keys**
  (`attest_keys`), which we write with every PLC operation that changes
  them, so a key's removal reaches the index (and its cache) at once.
- **A code's pending uses count against its limits** instead of blocking:
  in a registration rush, blocking every join on a shared code behind
  another's pending one would refuse people for no reason. A pending
  decision about the same person still blocks.
- **Within a repo and space, the latest-written record claiming a `seq` is
  the version that stands**, and counts only if it verifies. TC-63's
  frozen test needs this as written: vivarium's space `listRecords`
  returns `rkey`, not `uri`, so `dirkBefore.uri` is `undefined`, and the
  test puts Dirk's old-key admission back as a copy at rkey `undefined`
  beside the re-signed original rather than over it. Under this rule the
  copy is the decision's latest version, so it no longer counts, which is
  what the test expects. **Flagged for the user**: the test's intent
  (replace the record) and what it does (add a copy) differ; both pass.
- **Roles and rules** count only when signed *and* in the conference super
  admin's repo, so a role deleted after a handover can't be revived by a
  stale signed copy left in the previous super admin's repo.
- **`via: "role"`** joins: someone with a pre-assigned role (TC-31) joins
  with `via: "role"`, beside the design's `code`, `list`, `email`, `open`
  and `request`.
- **Ranks at signing**: a decision agreeing with the standing one keeps the
  higher rank; a person's own join (not a request) stands over an earlier
  removal or denial; a leave ends every ground but an admin's.
- **Migration**: the round 5 tables are `0003_signed_decisions.sql`, so a
  database made with the branch's earlier 0002 still migrates.

Tests: the frozen tests are unchanged (the TC-52 and TC-53 amendments are
the approved ones), and `acdcbc5` is unchanged. Non-frozen review tests
that encoded removed behavior were rewritten:

- "TC-53: a role a former admin gave stops admitting (review round 1)"
  becomes "…keeps admitting (review round 1, rewritten for design round
  5)".
- "TC-32: demoting an owner hands their list roles on, or they stop
  counting (review round 15)" becomes "demoting an owner leaves the list
  roles they gave standing (…rewritten for design round 5)".
- "TC-32: removing the owner whose list last gave a role hands it to
  another owner… (review round 14)" is left as it is: the hand-over is
  gone, but its assertion still holds, because the role stands.

`conference-review.test.ts` is also in `acdcbc5` (which took out the round
13 and 14 TC-28 regressions), so `scripts/check-tests-unchanged.sh acdcbc5`
reports it. Its only changes are the two rewrites above, which the user's
approval of the round 5 red tests records ("The implementation deletes or
rewrites them"). The round 5 tests themselves (`conference-signed.test.ts`,
`conference-admins-keep.test.ts`, `tests/support/attestation.ts`) are
unchanged.
- The Rust unit tests of the old precedence (role periods, first-seen
  clamping, kept admissions, overridden bans) are replaced by tests of
  signed records: unsigned records, grounds and ranks, a demoted author's
  ban, unsigned joins as requests, copies and versions of a `seq`, code
  limits by distinct DIDs, and signature checks.

### Round 16

Reviewer: a fresh subagent following `adversarial-review`. Verdict: not
clean (0 blocking, 1 major, 4 minor, 1 nit). It confirmed `c6e7d41`'s check
reports only the approved TC-52 and TC-53 changes, and `acdcbc5`'s only the
two recorded rewrites in `conference-review.test.ts`, and that `pnpm check`
was green.

1. **[major] A signed record moved into another collection still
   verified**, because the collection is in its path, not in what's
   signed: staff could turn their signed deny or removal into a ban.
   **Fixed:** a signed record counts only in the collection its signed
   `$type` names. Rust test
   `a_signed_record_moved_into_another_collection_doesnt_count`.
2. **[minor] Staff could let someone an owner removed back in by giving
   them a role**, which admits on joining. **Fixed:** for someone who isn't
   a member, a role is checked as an admission (`member add`) against the
   standing decision and any ban, inside the signing transaction.
   Integration "TC-52: staff can't let someone an owner removed back in
   with a role (review round 16)".
3. **[minor] A decision whose read-back failed was marked committed**, so
   the next check didn't see it. **Fixed:** its journal entry stays
   pending (blocking another decision about the person) until it lapses,
   by which time the write notification has normally brought it in; the
   same for joins and leaves.
4. **[minor] `org keys remove` silently dropped records that hadn't been
   re-signed.** **Fixed:** it refuses while any record verifies only by
   that key, listing them, unless `--force` (which then reports how many
   stopped counting). Integration "TC-63: a key that standing records
   depend on alone isn't removed without --force (review round 16)".
5. **[minor] Our copy of the keys was never reconciled with the DID
   document.** **Fixed in part:** `reindex` now removes any key the DID
   document no longer lists (one rotated out with the operator's recovery
   key, say). Declined: refreshing on a timer as well. The operator who
   rotates a key out behind our back runs `reindex`, as after any repair,
   and checking PLC on every derive would put the network in every
   membership check.
6. **[nit] Journal entries for code uses are never pruned.** Left: they're
   what keeps a code's limits while records are written, and one small row
   per code join is cheap.

The round 16 regressions are in a new file,
`tests/integration/conference-review-signed.test.ts`, since
`conference-review.test.ts` is under the `acdcbc5` freeze.

### Round 17

Reviewer: a fresh subagent following `adversarial-review`. Verdict: not
clean (0 blocking, 1 major, 4 minor, 1 nit). Both freeze checks report only
the approved changes, and `pnpm check` is green. Not reworked: round 17 was
the last round allowed (see "Blocked after round 17").

1. **[major] A former admin's edits and deletions in the admin space never
   reach the live index**, because `notifyWrite` takes admin-space writes
   only from current admins; `reindex` (which crawls former admins) does
   apply them, so the two disagree. Kees bans Bram, is removed as an admin,
   then deletes his ban: Bram is refused until a `reindex`, then joins.
   Suggested fix: accept admin-space notices from everyone `named_admins`
   lists (only verified records count anyway).
2. **[minor] Staff can still let someone an owner removed back in by
   issuing them a code**: a person's own join stands over any removal, and
   staff may issue codes. Round 16 closed the role path only. Suggested fix:
   give a code join the rank of the code that admitted it.
3. **[minor] A person who left can rejoin, with no rule admitting them now,
   by deleting their own signed leave** ("records are the source of
   truth"). Suggested: a journal tombstone for committed leaves, or accept.
4. **[minor] After a failed read-back (round 16 #3), the journal entry
   stays pending for good**: nothing commits it when the write notification
   indexes the record, so decisions about the person get "try again" for
   the full two minutes, the row is never pruned, and a code use stops
   counting after two minutes.
5. **[minor] One signing with a fast clock poisons `signedAt`** for every
   later one (it's forced strictly increasing), and readers ignore
   signatures more than five minutes ahead, so later decisions are
   reported done but don't count until real time catches up. Since `seq`
   orders, `signedAt` could just be `now`.
6. **[nit] No way to revoke a single code** (`codes revoke`); only `org
   admin undo --codes` revokes, all of one admin's codes at once.

### Blocked after round 17

Round 5's plan allowed review rounds 16 and 17, and round 17 isn't clean,
so this needs a human decision.

| Round | Blocking | Major | Minor | Nit |
|-------|----------|-------|-------|-----|
| 16 | 0 | 1 | 4 | 1 |
| 17 | 0 | 1 | 4 | 1 |

**What recurs.** The signed-decisions design removed the family that
rounds 11–15 kept finding (a role change re-deriving other people's
memberships): no round since has found one. What the two rounds found
instead is mostly at the edges of "records are the source of truth" and of
check-then-sign:

- **Ways around a standing decision through another kind of record:** a
  signed record moved into another collection (round 16, fixed), a role
  that admits (round 16, fixed), and a code that admits (round 17 #2).
  Each "way in" a lower rank can create is a way around a higher rank's
  removal, unless it carries its signer's rank.
- **Withdrawal by deletion:** a former admin's deletions don't reach the
  live index (round 17 #1), and a person can undo their own leave by
  deleting it (round 17 #3).
- **The journal's edges:** a decision committed but not yet indexed (round
  16 #3) and its fix's never-finished pending entry (round 17 #4), and
  `signedAt` monotonicity (round 17 #5).

**Current state:** every frozen test, the round 5 tests and every review
regression pass; `pnpm check` is green (warning only on the approved
`conference-admins.test.ts`); `scripts/check-tests-unchanged.sh acdcbc5`
reports only `conference-review.test.ts`, whose two rewrites the user's
approval records. Also open for the user: TC-63's frozen test reads
`dirkBefore.uri`, which vivarium's space `listRecords` doesn't return, so
it adds an old-key copy instead of replacing the record (see
"Implementation of design round 5").

**The choices:**
- **(a) Fix round 17's findings without another review**, then ship. The
  fixes are local: accept former admins' notices (#1), rank code joins by
  their code (#2), commit pending entries on indexing and date `signedAt`
  by `now` (#4, #5), `codes revoke` (#6); decide #3.
- **(b) Fix them, then review once more** (round 18).
- **(c) Ship as is**, recording #2 and #3 as accepted gaps.

**Decision (the user, 2026-10-08):** "good to go to round 20", agreeing to
the plan proposed after round 17:
- **Rework:** fix round 17 #1 (accept write notices from former admins in
  the admin space, so their edits and deletions reach the live index), #2
  (a join by code counts at the code issuer's rank), #4 (resolve the
  pending journal entry after a failed read-back) and #5 (don't carry a
  fast clock forward into later `signedAt`s).
- **Accepted by design:** #3, rejoining by deleting your own leave. A later
  ban or removal still stands over it.
- **Approved test fix:** TC-63's test in `conference-signed.test.ts`
  (acdcbc5) gets one fix. It builds Dirk's record URI from the `rkey` that
  `listRecords` returns, instead of reading `dirkBefore.uri`, so the old-key
  copy replaces the original record. Commit it on its own:
  `test(conference-space): TC-63 build the record URI from its rkey
  (approved)`.
- **Review rounds 18 to 20.** If round 20 isn't clean, set
  `status: blocked`.
