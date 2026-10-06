---
status: design-review
impact: cross-cutting
depends-on: [attendee-sign-in]
branch:
tests-commit:
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

**Admins, the super admin, and organization records.**

- An organization's **admins** (owners and staff) are DIDs, kept as host
  state. They're members of each of the organization's conferences.
- Each conference has exactly **one super admin**: the one DID that writes
  all of the conference's organization records. There's no tiebreak to
  design, because there's only ever one author:
  - the **public calendar event** and its `app.eventside.conference`
    sidecar, in the super admin's public repo (public conferences), or inside
    the space (invite-only)
  - **role** and **rules** records, in the super admin's repo inside the
    conference space
- The super admin is set when the conference is created
  (`conference create --super-admin <handle>`), and is recorded in host
  state and in the sidecar. Readers trust role and rules records **only**
  from that DID. Records from anyone else, including other admins, have no
  effect.
- The authority DID itself can be the super admin, once it has a repo
  (bring your own account, or our reference PDS later).
- **Admin sessions:** each admin connects once through
  `eventside admin connect <handle>`, an OAuth sign-in with
  `ADMIN_SCOPES`. That gives a cookieless session of kind `admin` that never
  idles out. Other admins act through the CLI, which writes as the super
  admin using the super admin's session. Changing a conference's super admin
  (and re-issuing its records) is out of scope for now.
- **`ADMIN_SCOPES`**, written out:
  - `atproto`
  - `repo:community.lexicon.calendar.event` and
    `repo:app.eventside.conference`, for the public records
  - `space:app.eventside.conference?authority=*&action=create&action=update&action=delete&collection=app.eventside.conference&collection=app.eventside.conference.role&collection=app.eventside.conference.rules&collection=community.lexicon.calendar.event`
  - `blob:*/*`, for `event-branding`
  - When `ADMIN_SCOPES` grows, admins are told to reconnect (as in
    round 1's "reconnect needed"). A real PDS may cap refresh-token
    lifetimes; the CLI reports an expired admin session the same way.
- **Bootstrapping:** `org create --owner <handle>` records the first owner.
  The CLI refuses to remove the last owner.

**Membership is host state.**

- Joining puts a member row in our database, with `read: true, write:
  true` and a start time. Leaving, removal and bans give it an end time.
- That's the member list our credential endpoint and write-notification
  intake check. Nothing is copied to another server.
- **Other apps see membership** through our host API: members with their
  periods, and admins, for credential holders. The membership records of
  round 2 aren't needed, because the host is the authority on membership.
- **Which records count.** A host sees one `notifyWrite` per repo state,
  not per record, and a removed member's own PDS keeps accepting their
  writes. When they rejoin, their next accepted `repoRev` carries every
  commit they made while out. So the host can't refuse individual records.
  Readers judge each record by its **commit's `repoRev`** (a TID timestamp)
  against the author's membership periods, which the host API publishes as
  `{since, until, lastAcceptedRepoRev}`. A record counts only if its commit
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

**App access is host state too:** per conference, curated (an allow-list
of client IDs, starting with eventside's bare client ID) or open. We
enforce it in `getSpaceCredential`, so it holds for every app.
`eventside admin apps add|remove|open|curate` edits it. Switching to open
warns first.

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

- `org create --owner <handle>` (mints the authority) and `org adopt <did>`
- `org admin add|remove <handle> --role owner|staff`, and
  `admin connect <handle>`
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

| Item | Where | Written by | Read by |
|---|---|---|---|
| Authority DID document (`#atproto_space_host`, `#atproto_space`/`#atproto` key) | PLC (or the organization's `did.json`) | our server (minted) or the organization (adopted) | everyone |
| `authorities`: DID, encrypted rotation and signing keys, minted or adopted | host DB | CLI | the host |
| `spaces`: URI, type, read and write policy, app access, allow-list | host DB | CLI, `groups`, `polls` | the host |
| `admins`: authority, DID, `owner` or `staff`, since, until | host DB | CLI | the host; the host API |
| `conferences`: space URI, event URI, super admin DID | host DB (also in the sidecar) | CLI | everything |
| `members`: space, DID, `read`, `write`, joined via, since, until, last accepted `repoRev` | host DB, the source of truth | join, leave, CLI, `notifyWrite` | the host; the host API; every feature's checks |
| `writers`: space, DID, `repoRev`, `hash`, `spaceRev`; plus a space-wide `spaceRev` sequence | host DB | `notifyWrite` intake | `listRepos`; `space-sync` |
| `notify_registrations`, `credentials_issued` (`jti`, delegating DID, app key, expiry), delegation replay cache | host DB | the host | the host |
| `community.lexicon.calendar.event` + `app.eventside.conference` sidecar | the super admin's public repo (public) or inside the space (invite-only) | our server as the super admin | anyone, or members |
| `app.eventside.conference.role {subject, role}`, `app.eventside.conference.rules` | the super admin's repo inside the space | our server as the super admin | members' apps, trusted only from the super admin |
| `invite_codes`, `attendee_list`, `join_requests`, `bans`, `preassigned` | host DB | CLI, join | join |
| `sessions.kind`, `oauth_requests.purpose` | sign-in tables | sign-in | sign-in, renewer |

#### Interfaces

- **The protocol host endpoints:** `getSpaceCredential`, `listRepos`,
  `registerNotify`, `unregisterNotify`, and receiving `notifyWrite`, at our
  `PUBLIC_URL`, in vivarium 0.0.2's wire format.
- **Our host API** (credential-authenticated, `app.eventside.space.*`):
  `getSpace` (policies, app access, the authority's admins with periods)
  and `listMembers` (members with `read`, `write` and their periods).
  Published lexicons, so other apps can apply the reader rules.
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

## Test cases

The cast:

- **Atmosphere** is an organization account. Its operator has connected it,
  and Olga is its owner.
- **Ana and Bram** are attendees with atproto accounts.
- **Mallory** is a signed-in stranger.
- **AtmosphereConf** (Amsterdam, 29 April–2 May) is Atmosphere's public
  conference.
- **"Sanne & Joost's wedding"** is an invite-only conference run by a second
  organization, **Bruiloft**, with a different theme.

"The CLI" is the admin command line. "Another app" is a second atproto
client on the conference's app list, used in tests to read the space the
way any other app would.

### Organizations and conferences

### TC-1: Connecting an organization leaves no way to act as it in the browser

- **Given** the operator can sign in as Atmosphere at its PDS
- **When** they run the CLI's connect command and complete sign-in in a
  browser
- **Then** the browser shows "Connected, you can close this tab"
- **And** the browser isn't signed in to the app as Atmosphere
- **And** the CLI reports Atmosphere as connected

### TC-2: A connected organization stays connected while idle

- **Given** Atmosphere was connected and nobody has used the app as it for
  longer than the attendee idle timeout
- **When** the operator runs any CLI command for Atmosphere
- **Then** it works without connecting again

### TC-3: An organization missing newly required permissions is reported, not dropped

- **Given** Atmosphere was connected before the organization permissions
  grew
- **When** the operator runs a CLI command for Atmosphere
- **Then** the CLI says Atmosphere needs to reconnect, naming the command
- **And** nothing it already set up stops working

### TC-4: Creating a public conference publishes an event other calendar apps can read

- **Given** Atmosphere is connected
- **When** the operator creates AtmosphereConf as a public conference
- **Then** a public calendar event for it exists in Atmosphere's repository,
  with its name, dates and city
- **And** anyone can open its public page by that event's link

### TC-5: A conference can adopt an event the organization already published

- **Given** Atmosphere already published a calendar event for AtmosphereConf
  through another calendar app
- **When** the operator creates the conference from that event
- **Then** the public page shows that event, and no second event is created

### TC-6: A conference can't adopt someone else's event

- **Given** an event published by Mallory's account
- **When** the operator tries to create an Atmosphere conference from it
- **Then** the CLI refuses, saying the event must be in Atmosphere's own
  repository

### TC-7: An invite-only conference publishes nothing

- **Given** Bruiloft is connected
- **When** the operator creates "Sanne & Joost's wedding" as invite-only
- **Then** no public event or settings record for it exists in Bruiloft's
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

- **Given** AtmosphereConf's public page link uses Atmosphere's DID
- **When** someone opens the same page using Atmosphere's handle in place of
  the DID
- **Then** they see the same conference

### Joining

### TC-11: Ana joins with a shared invite code

- **Given** AtmosphereConf has the shared code "atmosphere27"
- **When** Ana signs in, opens the public page and enters the code
- **Then** she's a member and sees the inside of the conference
- **And** the space's member list includes her, with read and write access
- **And** a membership record for her, starting now, is in the space

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

### TC-19: An unconfirmed email doesn't match

- **Given** the attendee list has an email that Mallory's account uses but
  hasn't confirmed
- **When** Mallory verifies her email to join
- **Then** she isn't admitted from the list

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
- **And** she's gone from the space's member list, and her membership record
  has an end time
- **And** she can rejoin with any method that admits her, which starts a new
  membership record

### TC-27: A removed member loses access at once

- **Given** Bram is a member
- **When** the operator removes him
- **Then** his very next request for anything inside the conference gets
  nothing
- **And** he's gone from the space's member list, and his membership record
  has an end time

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
- **And** another app reading the space sees Olga as owner and Pim as staff

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

### TC-35: Roles, rules and memberships from anyone but the organization are ignored

- **Given** Mallory, a member, wrote a record into the space claiming she's
  an owner, and another claiming new rules
- **When** roles and rules are read
- **Then** neither of her records has any effect

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

### TC-38: Reconciling repairs a membership left half-done

- **Given** Bram was added to the space's member list, but the app stopped
  before recording it
- **When** the server starts, or the operator runs reconcile
- **Then** Bram is a member everywhere, with a membership record

### TC-39: A failed membership change isn't reported as done

- **Given** Atmosphere's PDS is unreachable
- **When** Ana tries to join with a valid code
- **Then** she's told to try again, and isn't shown as a member
- **When** the PDS is back and she tries again
- **Then** she joins

### Two conferences, one person

### TC-40: Ana is in two differently branded conferences with one account

- **Given** Ana is a member of AtmosphereConf and of "Sanne & Joost's
  wedding"
- **When** she opens her list of conferences
- **Then** she sees both, each with its own name and theme
- **And** opening each shows its own inside and nothing from the other

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
