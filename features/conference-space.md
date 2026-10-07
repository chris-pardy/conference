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
    maxUses?}` and `app.eventside.admin.listEntry {space, did? | handle?,
    emailHmac?, role?}`: owners and the super admin. Codes are hashed, and
    emails are HMAC'd with a server key, so neither is readable even inside
    the admin space.
- **Precedence, so no tiebreak is needed:**
  - Admins write only to their own repos, so nobody can edit anyone else's
    records.
  - **The super admin's records always win.** No other admin's record can
    contradict them. For example, a staff member can't admit someone the
    super admin banned, or change a space's policy.
  - Among other admins, a ban beats an admission, and otherwise the most
    recent record (by commit `repoRev`) for the same space and person
    stands.
  - A record counts only while its author is an admin, and only within
    their role.
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
- **Automatic admissions are derived, not written.** The indexer admits
  someone whose join record (judged by its commit `repoRev`) matches a rule
  in the admin space: a valid `code` record (with expiry and uses counted
  in commit order), a `listEntry` by DID or handle, or the space being open.
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
    super admin, applies the precedence rules, and keeps the index up to date
    from `notifyWrite` (our own admin space included). `reindex` rebuilds it
    from scratch.
  - `authority.rs`: minting the `did:plc` (genesis operation), adopting a
    DID, key custody, DID-document checks
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
| The HMAC key for emails, and the server's encryption key | server config |

**Records, the source of truth:**

| Item | Where | Written by | Read by |
|---|---|---|---|
| Authority DID document (`#atproto_space_host`, `#atproto_space` key) | PLC, or the organization's `did.json` | our server (minted) or the organization (adopted) | everyone |
| `app.eventside.admin.admin`, `.space`, `.member`, `.ban`, `.deny`, `.code`, `.listEntry` | admins' repos in the organization's admin space | the super admin and admins (our server, under their sessions) | admins' apps; our host (crawled) |
| `app.eventside.intake.join {code?}`, `.leave` | each person's own repo in the conference's intake space | the person (through eventside) | our host and the admins |
| `community.lexicon.calendar.event` + `app.eventside.conference` sidecar (space, visibility, join flags, app access mode, super admin, theme) | the super admin's public repo (public) or inside the conference space (invite-only) | our server as the super admin | anyone, or members |
| `app.eventside.conference.role {subject, role}`, `app.eventside.conference.rules` | the super admin's repo inside the conference space | our server as the super admin | members' apps, trusted only from the super admin |

**The index and operational state** (rebuildable or disposable):

| Item | Where | Built from or used by |
|---|---|---|
| `admins`, `spaces` (policies, app access), `members` (periods, last accepted `repoRev`), `bans`, `codes`, `list_entries`, `conferences` | host DB | crawled from the admin space; read by every check |
| `writers` (`repoRev`, `hash`, `spaceRev`), the space-wide `spaceRev` sequence | host DB | `notifyWrite` intake; `listRepos` |
| rate-limit counters, delegation and attestation replay caches, `credentials_issued` | host DB | credential checks; revocation |
| `sessions.kind`, `oauth_requests.purpose` | sign-in tables | sign-in, renewer |

#### Interfaces

- **The protocol host endpoints:** `getSpaceCredential`, `listRepos`,
  `registerNotify`, `unregisterNotify`, and receiving `notifyWrite`, at our
  `PUBLIC_URL`, in vivarium 0.0.2's wire format.
- **The admin space's lexicons** (`app.eventside.admin.*`), published, so
  any app an organization allows into its admin space can read its
  permissions directly.
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
  - `org admin add|remove <handle> --org <did> [--role owner|staff]`
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
  - `list import <csv> --conference <space>`, with columns `handle,email,role`
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
  with `until`; the latest decision about a person stands (bans aside, see
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

### TC-52: Between other admins, a ban wins and otherwise the latest decision stands

- **Given** Pim, a staff admin, and Kees, an owner
- **When** Pim admits Bram and Kees then bans him
- **Then** Bram is banned
- **When** Kees removes Ana and Pim later admits her again
- **Then** Ana is a member

### TC-53: A former admin's decisions stop counting

- **Given** Pim admitted Bram while he was staff
- **When** Olga removes Pim as an admin
- **Then** Bram is no longer a member, unless another rule admits him

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
