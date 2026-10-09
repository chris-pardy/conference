---
status: implementing
impact: cross-cutting
depends-on: [attendee-sign-in]
branch: feature/conference-space
tests-commit: c0d4ee617c4e542b87e4191bdf6b83b992726f62
---

# Conference space

## Summary

A conference is **one private space**. Its authority is a DID belonging to
an organization (or, later, a person), and eventside hosts the space.
Attendees are members: they can write to the space but not read it.
Eventside reads all of it and serves it through [`feeds`](feeds.md), and so
can any other app the organizer allows. A conference is public on the
outside by default, or invite-only.

This is a rewrite. The first design (rounds 1–5, built to review round 19)
kept running into who may override whom once access was spread across
several repos. After the move to feeds, one private space with eventside
deciding visibility made most of that unnecessary. See "Previous design".

For the November 1 demo there is **one demo event**, set up and
administered by a CLI. There are no admin screens in the app.

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

**Finding a conference (invite-only).** Nothing is public. Someone with an
invite link or code sees a sign-in and join prompt. Anyone else sees
nothing.

**Joining.** The attendee signs in (see
[`attendee-sign-in`](attendee-sign-in.md)) and is admitted by any of the
methods the organizer has turned on:

- **Invite link or code:** one shared code ("atmosphere27"), or personal
  codes.
- **Attendee list:** the organizer imports a list from their ticketing
  tool. People on it get in when they sign in. They're matched either:
  - by atproto handle, resolved when the list is imported; or
  - by verified email, using the OAuth `transition:email` scope at join
    time.
- **Request and approve:** the attendee asks, and an owner or staff member
  approves.
- **Open:** anyone signed in can join.

Once admitted, they land on the conference's main feed. The main feed is
created from the conference's template when the conference is set up.

**Inside.** Only members see the inside, through eventside or any other app
the organizer allows. Attendees never read the space directly; eventside
serves them what they may see.

**Leaving.** An attendee can leave. What they wrote stays in their repo,
but eventside stops serving it. They can rejoin through any method that
admits them.

**Organizers.** Admins sign in as themselves; nobody shares a password.

- **Owner:** full control, including adding and removing owners.
- **Staff:** the same powers, except managing owners. Staff can invite
  people with full rights, but can't undo an owner's decision about a
  member.
- **Speaker:** an attendee linked to their sessions, usually set by
  `program-import`.
- **Attendee.**

When a personal account is the authority (after Nov 1), that account is
always an owner and can't be removed.

**Admin decisions** (admit, remove, ban, change a role) are made through
eventside:

- The admin's role is checked when they act, and the decision is signed
  and kept from then on.
- A later decision about the same person replaces an earlier one, except
  that staff can't replace an owner's decision.
- A former admin's decisions stand after they leave the role.
- A banned person is removed and can't rejoin by any method.

**Apps.** The organizer chooses:

- which other apps may read the conference's space;
- which outside card providers and feed generators [`feeds`](feeds.md)
  may call.

Eventside is always allowed. We can't stop an organizer from allowing a
careless app; that's their decision.

**Administration for November 1.** A CLI does all of it:

- create the organization and its conference
- add owners and staff
- choose the join methods, and public or invite-only
- import attendee lists, and issue codes
- approve or deny requests
- remove or ban members, and change roles
- choose the allowed apps

**Done.** The CLI seeds the demo event, and then:

- An attendee joins. They see the public page before, and the main feed
  after.
- A non-member can't see inside.
- Removing or banning someone cuts off their access.
- An allowed app can read the space, and one that isn't allowed can't.

## Data

- **The authority** is the organization's DID. For Nov 1, eventside mints
  one for the demo event's organization. Its DID document:
  - names eventside as its space host;
  - carries one or more `#eventside_attest` keys, whose private halves
    eventside holds. Several keys allow rotation.
- **The space:** one per conference, e.g.
  `at://<org did>/space/app.eventside.private/<conference>`.
  - Members can write, but not read.
  - Eventside, and the apps the organizer allows, read everything.
  - Everything [`feeds`](feeds.md) stores lives here: feeds, posts,
    pins, labels, and attendees' private actions.
- **The entry point.**
  - **Public conferences:** a public `community.lexicon.calendar.event`
    record in the authority's repo. An eventside sidecar record next to
    it holds the join methods, public or invite-only, the theme, and the
    template.
  - **Invite-only conferences:** no public record. The entry point is an
    invite link that names the space.
- **Membership and roles** are records in the authority's repo in the space,
  written by eventside:
  - one record per person, with their role, who decided and when;
  - a separate record per ban;
  - each signed by eventside with an `#eventside_attest` key, in the
    badge.blue attestation format. That's an inline signature over the
    record's CID, with the repository bound in, as designed in the
    previous round 5. It lets the record move unchanged into the repo of
    the person who holds the role later;
  - deleting a record withdraws it; a new decision replaces it.

  The space host's member list mirrors these records, since it's what
  lets members write.
- **The allowed apps** are a record in the authority's repo in the space.
- **Attendee lists, codes, requests and emails** are kept in eventside's
  database, where attendees can't read them. Emails are used only for
  matching, and never served.
- **Decision history** (every admit, removal, ban and role change, and who
  made it) is kept in eventside's database.

## Options considered

### Who the authority is

- **A DID per organization or person, with eventside as space host or as
  managing delegate** (chosen).
  - It can be an organization's own DID or a person's.
  - For Nov 1 there's one demo event, with one minted organization DID
    (`did:plc` or `did:web`), hosted by eventside.
  - Personal DIDs and the delegated mode come later.
- **Eventside's own `did:web` for every conference:** simplest, but no
  conference could ever move to another host.
- **A per-organization `did:web` under our domain:** cheaper than PLC, but
  still tied to us.

### Hosting

- **Hosted by eventside, for Nov 1** (chosen). The delegated mode (the
  space on the authority's own PDS, with eventside as its `managingApp`)
  needs only one `createSpace` per conference from the authority's
  session, so it's workable later.
- Both modes for Nov 1.
- Delegated only: would drop the space host already written on the old
  branch.

### How admin decisions work

- **Checked when made, latest wins** (chosen). Staff can't replace an
  owner's decision. A former admin's decisions stand. No crawling, no
  per-admin repos.
- Any current admin's latest decision wins: staff could undo an owner's
  ban.
- Signed decisions in each admin's own repo (the previous round 5): the
  model that kept failing review.

### Where membership and roles live

- **Signed records written by eventside** (chosen), in the authority's repo
  in the space for now, and eventually in the repo of the person who holds
  the role. Allowed apps can see who's a member and who organizes.
- Eventside's database only: other apps couldn't tell who organizes.

### Signing

- **badge.blue attestations now** (chosen), with `#eventside_attest` keys.
- The repo commit signature alone: enough while the records sit in the
  authority's repo, but not once they move to members' repos.

### Carried over from the first design

- **Join methods** (chosen): any combination of invite code, attendee list,
  request and approve, and open. The alternative was one method per
  conference.
- **Matching a list** (chosen): by handle or by email, whichever the row
  has. Handles must resolve at import.
- **Roles** (chosen): owner, staff, speaker, attendee. Sponsor is deferred.
- **Administration** (chosen): a CLI for everything on Nov 1, rather than
  admin screens.
- **What non-members see** (chosen): public by default, through a community
  calendar event. An invite-only conference shows nothing.
- **Leaving** (chosen): allowed, and the attendee's records stay theirs.

## Out of scope

- Admin screens in the app.
- Personal DIDs as authority, and the delegated hosting mode.
- Moving a conference to another host.
- The sponsor role, and sponsor data sharing.
- Billing, tiers and counting active participants.
- Per-conference onboarding (code of conduct, event profile):
  `event-profile`.
- Configuring branding: [`event-branding`](event-branding.md).
- Feeds, posts and moderation: [`feeds`](feeds.md).
- Named, long-lived groups (opensocial.group).
- RSVPs from other calendar apps as a way to join.

## Open questions

None. The organization's DID is a `did:plc`; see the architecture
analysis.

## Previous design

The first design was specced through design round 5 and built to review
round 19, then paused on 2026-10-08.

- Its full text, with its test cases and review log, is on the archived
  branch `archive/conference-space-v1` (at `eacd33e`).
- Its spec as approved on `main` is in commit `a393962`.

What it tried, and why it was replaced:

- The organization was the authority, eventside its space host, and every
  member could read the whole space through allowed apps.
- Roles and rules were records crawled from the super admin. Later
  (round 5), each admin signed their decisions into their own repo.
- Review rounds 11–19 kept finding problems with admin precedence, role
  changes and withdrawing decisions by deleting records.

What carries over to the new build, wherever it still fits:

- the space host (`crates/server/src/spacehost/`);
- the join flows;
- the CLI;
- the attestation code (`attest.rs`);
- the test helpers.

The build starts on a new branch with new tests.

## Architecture analysis

**Existing code touched:**

- **On `main`:** only `attendee-sign-in` exists: `crates/server/src/auth/`,
  `oauth.rs`, `identity.rs` and `keys.rs`, plus the PWA shell. Joining
  needs more sign-in scopes: `space:` writes on `app.eventside.*`, and
  `transition:email` for list matching.
- **On `archive/conference-space-v1`** (to salvage from, not merge):
  - `crates/server/src/spacehost/`: about 6,100 lines.
    - `authority.rs` mints an organization's `did:plc`, with our host as
      `#atproto_space_host`, the operator's recovery key first among the
      rotation keys, and rotation of the `#eventside_attest` keys. That
      settles the open question: the demo DID is a **`did:plc`**, because
      minting and key rotation are already written and tested against
      vivarium.
    - `credential.rs` handles space credentials (vivarium 0.0.2's wire
      format).
    - `notify.rs` handles the writer set and write notifications.
    - `sync.rs` reads writers' ops from their PDSes.
    - `index.rs` is the record index.
    - `attest.rs` signs and verifies badge.blue attestations.
  - `crates/server/src/conference/`: the conference model and XRPC, the
    join flows, `admin.rs` (acting for admins) and the 2,150-line admin
    CLI.
  - `tests/support/` (`conference.ts`, `other-app.ts`, `attestation.ts`,
    `server-setup.ts`) and five migrations.
  - What gets dropped: the admin and intake spaces, crawling from the
    super admin, per-admin repos for decisions, and the
    withdrawn-decision tables (migrations 0004 and 0005).
- **New work the old branch never had: the authority's own repo in the
  space.**
  - Membership, role, allowed-apps and feed records now live in the
    authority's repo inside the space. A minted `did:plc` has no PDS, so
    our host has to keep that repo itself and serve it through the space
    read endpoints (`getRecord`, `listRecords`, `listRepoOps`,
    `getLatestCommit`), as a writer in its own space. That's a small part
    of a PDS, limited to one repo per space.
  - The public calendar event can't live there, because it has to be
    public. It stays in an owner's own repo, as in the first design.
    Hosting a public repo for the organization would be the alternative.

**Features affected:**

- [`feeds`](feeds.md) (`ready`) depends on this feature for:
  - the private space;
  - the `#members` and `#organizers` audiences;
  - the allowed-apps list, which covers outside providers and generators
    too;
  - the template that seeds the main feed;
  - the authority's repo, where eventside writes feed records.
- [`space-sync`](space-sync.md) (`analysis`):
  - The old branch built most of space-sync inside its space host:
    eventside's own credentials, write notifications, pulling ops from
    writers' PDSes, and a per-repo index.
  - Speccing space-sync separately would duplicate that, or split one
    subsystem across two features.
  - The design review should decide whether space-sync folds into this
    feature or is cut down to what's left.
- [`block-actions`](block-actions.md) (`analysis`):
  - Its actions are written into this space.
  - Its ingest rule ("does this record count") now comes from here: the
    author was a member when they wrote it.
- [`plans`](plans.md) (`design-review`): plans hung off a conference node
  and the first design's notion of organizers. It now uses the
  `#organizers` audience and the conference's feeds.
- [`attendee-sign-in`](attendee-sign-in.md) (`complete`): gains scopes, and
  existing sessions are asked to sign in again.
- `event-branding`, `program-import`, `groups`, `connections`,
  `event-profile` (not specced) read membership and roles from here, and
  scope their records to this space.

**New shared surfaces:**

- Membership and role records, and their attestation format.
- The membership check other features call: is this person a member, and
  in which role, now and at a given time.
- The allowed-apps record.
- The authority's repo in the space, which our host writes on the
  authority's behalf.
- The conference's sidecar record (join methods, visibility, theme,
  template).
- The admin CLI.

**Verdict:** cross-cutting.

- It sets the space and identity model that every later feature stores
  its data in.
- It adds lexicons, a database schema and sign-in scopes.
- It overlaps `space-sync`.

It needs a design review.

## Design review

**Approach.**

- **The authority** is the organization's own account on a PDS that
  supports spaces. In tests that's vivarium's PDS (`createAccount`).
  - The PDS mints its `did:plc`, and its repo is a normal repo.
  - At setup, one PLC operation adds three entries to its DID document:
    - `#atproto_space_host`, naming eventside;
    - `#atproto_space`, the key eventside signs space credentials with;
    - `#eventside_attest`, the key eventside signs decisions with.
  - The operation needs the account's email token, which vivarium's mail
    catcher supplies in tests.
  - Eventside holds a session for the organization account and writes as
    it.
  - Because the authority's repo is an ordinary one:
    - the authority's records in the space are read the way any writer's
      are, from its PDS;
    - the public calendar event and its sidecar live in the
      organization's public repo, as the brainstorm says;
    - the same path serves personal DIDs later.
  - The old branch minted a PDS-less `did:plc` instead. That would have
    meant our host keeping a repo for it (MST, commits, `listRepoOps` in
    vivarium's format) and answering as its PDS. This approach drops that
    work.
- **The space** is `app.eventside.private`, with one rkey per conference.
  Eventside is the space host.
  - **Writers:** the host's member list, mirrored from the decisions. The
    authority is always a writer.
  - **Credentials** (a change from the old `credential.rs`):
    - A member's delegation only ever gets `read_self`.
    - Eventside's own client reads everything.
    - Another app reads everything only if:
      - it's on the allowed-apps record for `read`; **and**
      - the delegation comes from an owner or a staff member.
    - So attendees never read other people's records, through eventside
      or any other app. The organizer decides which apps read the
      conference, and does so through their own delegation.
- **Admins act as themselves.** Each admin connects once from the CLI
  (`eventside admin connect`, over OAuth, as on the old branch).
  - A command run `--as <handle>` acts under that admin's session, so
    the actor's identity is verified, not claimed.
  - The organization account's session is what eventside writes with.
- **Decisions.** Every admin action, and every join or leave, goes
  through `decide(conference, subject, action, actor)`. The steps run in
  one database transaction, with `BEGIN IMMEDIATE` so the CLI process
  and the server serialize:
  1. Read the subject's state **from the `decisions` log**:
     - the last effective decision about them;
     - its kind: `admit`, `remove`, `ban` or `role`;
     - its rank: `owner`, `staff` or `self`.
  2. Check the actor's role now, and apply the precedence table below.
  3. Append the decision to the log, with a `seq` from the database.
     Order is by `seq`, never by clock.
  4. Write an **outbox** entry. A worker in the server applies it, and
     replays any unapplied entries on start:
     - write or delete the signed record in the authority's repo;
     - update the host's member list;
     - on removal or ban, revoke the person's outstanding credentials at
       writers' PDSes, using the old `notify.rs`.

  The log is what decides. The records in the repo are a signed,
  readable copy of it.
- **Precedence.** Ranks are owner > staff > self. A system actor such as
  `program-import` acts at staff rank.

  | Actor | Subject's last decision | Action | Allowed? |
  |---|---|---|---|
  | anyone | none, or a lapsed admission | admit (any method) | yes, unless banned |
  | self | any admission | leave | yes, always |
  | self | removal by staff or self | rejoin by a method | yes |
  | self | removal by an owner | rejoin by a method | no; it needs a staff or owner admit |
  | self | ban (any rank) | rejoin | no |
  | staff | admission by an owner | remove, ban, change role | yes: staff may **tighten** |
  | staff | removal or ban by an owner | admit, unban | no: staff can't **loosen** an owner's decision |
  | staff | anything about an owner or staff member | any | only owners act on admins |
  | owner | any | any | yes |
  | owner | the last owner | remove, demote, leave | no: there's always one owner |

  - **Unban** restores eligibility to join, not membership.
  - **A ban** can be placed on someone who has never joined.
  - **An owner demoted to staff** keeps owner rank on the decisions they
    made as an owner. Those decisions still need an owner to loosen
    them, even the person who made them.
  - **Role changes** are decisions too. Staff can make an attendee a
    speaker, even one an owner admitted.
- **A former admin's decisions stand.** Decisions keep the rank they
  were made with, and are never re-checked against anyone's current
  role.
- **What leaving and removal do to content.** This settles a conflict
  between the brainstorm and [`feeds`](feeds.md):
  - **Leave, remove or ban:** eventside stops serving that person's
    records while they're not a member. The feeds filter adds "the
    author is a member now".
  - **Rejoin:** their records written while a member come back.
  - **Records written while not a member** are never served.
  - Feeds' acceptance decisions are kept either way. Only serving
    depends on current membership.
  - "When written" is the time eventside received or indexed the record,
    never the record's `createdAt`, which its author controls.
- **Signing.**
  - Records are signed with the old `attest.rs` (badge.blue inline
    attestation, with `repository` = the authority, and the log `seq` in
    the attestation).
  - A record counts for readers only if it verifies against an
    `#eventside_attest` key listed in the DID document now.
  - **Re-signing** for a key rotation is driven from the decisions log,
    never from what's in the repo, so a record forged with a leaked key
    isn't laundered. Re-signing changes only the key and `signedAt`. The
    decision, its rank and its time stay the same, and nothing is
    re-checked.
  - For Nov 1, keys are only added. Removing one comes later.
- **Joining** keeps the first design's flows from `conference/` on the
  old branch, and each one ends in `decide`.
- **Every request checks membership.** The BFF checks it on each request,
  never from a cached session, so a removal takes effect at once.
- **Allowed apps** are one signed record per conference,
  `app.eventside.conference.apps`. Owners and staff can change it.
- **"Conference created"** is an outbox entry.
  [`feeds`](feeds.md)' worker consumes it and writes the main feed
  record, using the authority write API below.

**Components.**

- **`crates/server/src/spacehost/`**, salvaged:
  - `credential.rs`, with the new read rule;
  - `notify.rs` and `sync.rs`, reading every writer, the authority
    included, from its PDS;
  - `attest.rs`;
  - `authority.rs`, reworked: no minting; it connects the organization
    account and adds keys by PLC operation;
  - `index.rs`, cut down to a records index.
- **`crates/server/src/conference/`:**
  - `decide.rs`: the precedence table, the log, the outbox;
  - `join.rs`;
  - `membership.rs`;
  - `apps.rs`;
  - `authority_repo.rs`: put and delete records as the authority, used
    by `decide` and by feeds;
  - `cli.rs`;
  - `api.rs`.
- **A fresh migration:**
  - authorities and their sessions;
  - conferences;
  - `decisions`, with `seq`;
  - the outbox;
  - codes, lists, requests and email HMACs;
  - the records index.
- **`space-sync` is folded in.** Its parts land here:
  - eventside's credentials;
  - registering as a syncer;
  - backfill;
  - the records index;
  - the ingest hook that feeds uses.

  Write notifications for records written behind eventside's back are
  kept, but feeds and this feature mostly index on write.

**Data** (each record has `$type` and `createdAt`; DIDs are `did` format):

- **`app.eventside.conference.member`** (authority's repo, in the space;
  rkey = the subject's DID): `{subject, role, method, decidedBy,
  decidedRank, seq, decidedAt, signatures}`.
- **`app.eventside.conference.ban`:** `{subject, decidedBy, decidedRank,
  seq, decidedAt, signatures}`.
- **`app.eventside.conference.apps`:** `{apps: [{client?: uri, service?:
  did, uses: [read|cardProvider|feedGenerator]}], signatures}`.
- **`app.eventside.conference.sidecar`** (the organization's public repo,
  public conferences only): `{event, space, methods, theme, template}`.
  Invite-only conferences keep their settings in eventside's database,
  so nothing public names them. Their invite link carries the space and
  the code.
- **Eventside's database:** `decisions` (authoritative), the outbox,
  codes, lists, requests, email HMACs, admin and organization sessions,
  and the records index.
- **Audiences for feeds:**
  - `#organizers` = owners and staff;
  - `#members` = everyone admitted.
  - Speakers are members with a role, and aren't organizers.

**Interfaces.**

- **XRPC for the PWA:**
  - `app.eventside.conference.get`
  - `join`, which returns `joined`, `pending`, `emailNeeded` or `refused`
  - `leave`
  - `getMembership`
- **Rust, for other features:**
  - `membership::is_member(conf, did, at: Received)` and
    `membership::is_member_now`;
  - `membership::role`;
  - `decide`;
  - `apps::allowed(conf, id, use)`;
  - `authority_repo::put` and `delete`;
  - outbox events (`conference.created`).
- **The space host:** credentials (with the new read rule),
  notifications, revocation, and the read endpoints.
- **The CLI:**
  - `org connect` (the organization account, plus the PLC update)
  - `admin connect`
  - `conference create`
  - `admin add|remove`
  - `join set`
  - `list import`
  - `code create`
  - `requests …`
  - `member add|remove|ban|unban|role`
  - `apps allow|disallow`

**Edits to [`feeds`](feeds.md)** (it's `ready`, so these need the user's
approval):

- The private space's authority is the organization's DID, not
  eventside's `did:web`.
- Feed records count only from the authority's repo, and eventside
  writes them through `authority_repo`.
- `depends-on` drops `space-sync`.
- The visibility filter adds "the author is a member now".
- Moderator scopes: a person who becomes a moderator is asked to sign in
  again the next time they try a moderator action.

**Impact on existing features.**

- **`feeds`:** as above.
- **`space-sync`:** superseded and folded in.
- **`block-actions`:** its ingest rule is `is_member(at: Received)`, and
  it serves only current members' actions.
- **`attendee-sign-in`:**
  - adds `space:` scopes;
  - adds `transition:email`, asked for only when needed;
  - existing sessions are asked to sign in again.
- **`plans`:** uses `#organizers`.

**Alternatives.**

- **Our host keeps a PDS-less authority's repo** (the first draft):
  - It needs a hand-written MST, commits and `listRepoOps` in vivarium's
    format, and a `#atproto_pds` pointing at us.
  - The public event would then have to live in an owner's repo.
  - Too much to get right for Nov 1.
- **Precedence read from repo records:** a removal deletes the record, so
  the next decision has nothing to compare against.
- **Members reading through allowed apps:** it would expose ballots and
  mark-safe responses to every attendee.
- **Decisions attributed by the operator** (`--as` with no session):
  simpler, but the role check would only be a claim.
- **Signed decisions in each admin's own repo** (the first design):
  rejected.

**Risks.**

- **The demo network.** bsky.social doesn't implement
  `com.atproto.space.*`. The demo's organization account is on vlpds
  (round 1).
- **Schedule.**
  - Nov 1 is 24 days away, and feeds comes after this.
  - Proposed Nov 1 slice:
    - the organization account;
    - a public conference;
    - shared code, open joining, and a list matched by handle;
    - the CLI;
    - precedence from the log;
    - adding keys.
  - Proposed fast-follow:
    - email matching;
    - request and approve;
    - personal codes;
    - invite-only;
    - removing keys.
  - This slices methods you chose; it doesn't drop them. It's your call.
- **Outbox lag.** Records reach the repo shortly after the decision
  commits. Readers that only see the repo lag slightly. Eventside itself
  reads the log.

### Round 1

The draft was critiqued before being presented. Folded in:

- **Precedence comes from the decisions log, ordered by `seq`.** A
  removal deletes the repo record, so reading precedence from records
  lost owner removals.
- **A full precedence table:**
  - staff may tighten an owner's decision, but not loosen it;
  - only owners act on admins;
  - there's always at least one owner;
  - the rules for unban, pre-emptive bans and demoted owners are spelled
    out.
- **Members only ever hold `read_self`.** Other apps read with an
  owner's or staff member's delegation. The old credential rule let
  every attendee read everything through an allowed app.
- **The authority is an organization account on a PDS that supports
  spaces**, rather than a host-kept repo for a PDS-less DID. That
  removes the repo code, and puts the public event back in the
  authority's repo, as the brainstorm says.
- **Invite-only settings stay out of public repos.**
- **Admins act under their own connected sessions.**
- **Leaving, removal and bans stop the person's records being served**
  until they rejoin. "When written" means when eventside received the
  record.
- **`decide` runs in one transaction**, with an outbox that's replayed on
  start. Credentials are revoked, and membership is checked on every
  request.
- **For feeds:** an authority write API, `#organizers` defined, the
  "created" event through the outbox, and edits to `feeds.md` listed.
- **Re-signing** comes from the log, and only key removal is deferred.
- **`space-sync` is folded in.**
- **Records get `$type` and `createdAt`.**
- **Staff can manage allowed apps**, as the brainstorm's roles say.
- **A Nov 1 slice** is proposed.
- The done criterion still says attendees see the main feed. That needs
  feeds, so before feeds lands, the test for it checks that the main
  feed record exists.

**The user's feedback:**

- The demo organization's account is on **vlpds**.
- Precedence, admin sign-in and everything else: approved.
  - That includes the five edits to `feeds`, folding in `space-sync`,
    and the Nov 1 slice.

### Round 1 (approved)

- **The demo organization's account is on vlpds,** the PDS that supports
  spaces for the demo.
- **The five `feeds` edits** are applied in `feeds.md`, under "Changes
  from conference-space", and add feeds' TC-43.
- **`space-sync` is marked superseded.**
- **The Nov 1 slice** is accepted:
  - **In:** organization account, public conference, shared code, open
    joining, attendee list by handle, the CLI, precedence, adding keys.
  - **Fast-follow:** email matching, request and approve, personal
    codes, invite-only, removing keys.

  The test cases cover the slice. The fast-follow methods get their own
  feature.

### Round 2: hosting through the managing-app policy (during the build, 2026-10-09)

**What the red tests found.** Vivarium 0.0.3 treats itself as the space
host for any account it hosts, and ignores `#atproto_space_host` in the
DID document (`resolveSpaceHost` returns its own URL for a local
account). A real PDS that supports spaces, like vlpds, will very likely
do the same. Eventside can't be the space host for an organization that
has an ordinary account.

**The user's decision:** "Do we need to be a space host? That was to let
us create spaces at will. With a reduced list of spaces we can use
simple-spaces and just be the managing app policy." So eventside is
**not** a space host.

**What changes:**

- **The space lives on the organization's own PDS.** At conference
  creation, eventside calls `com.atproto.simplespace.createSpace` with the
  organization's session, with policy `managingAppPolicy {managingApp:
  <eventside did>#eventside_access}`. That's one space per conference, so
  creating it under the authority's session is fine.
- **Eventside's DID** is the server's `did:web`. Its DID document lists:
  - an `#eventside_access` service, which answers
    `com.atproto.simplespace.checkUserAccess`;
  - the `#eventside_attest` keys.
- **`checkUserAccess(space, user, access, clientId)`** is where the access
  rules live. The PDS calls it with a service JWT from the authority, and
  treats an unreachable eventside, or any non-2xx answer, as a denial.
  Eventside answers from the decisions log:
  - **write:** the user is a member now.
  - **read** with eventside's own client: the user is a member. They get
    only their own records; eventside serves everything else itself.
  - **read** with another client: the client is on the allowed-apps
    record for `read`, **and** the user is an owner or staff member.
  - **Anyone else** is denied.
- **No PLC update.** The organization's DID document is untouched.
  Signatures use the `#eventside_attest` keys in eventside's own DID
  document. A reader checks that the signing DID is the space's
  `managingApp`. Rotation adds keys there.
- **Removal takes effect** at the next `checkUserAccess`, plus the BFF's
  per-request membership check. Revoking credentials at writers' PDSes is
  no longer eventside's job.
- **Dropped from the salvage:**
  - the space host (`spacehost/credential.rs` and `notify.rs`, the writer
    set, minting the organization's DID);
  - the host-side read endpoints.
- **Kept:**
  - `attest.rs`;
  - `sync.rs`, which reads writers' records with eventside's own read
    access;
  - the index;
  - the join flows and the CLI.
- **`org connect`** is now an OAuth connection to the organization's
  account, with no PLC token.
- **Delegated mode** moves into scope. "Hosted by eventside" is out of
  scope.

**Horizon** (the user, 2026-10-09): "let's plan this approach medium
term. We could use a custom PDS (event host?) long term."

- **Medium term:** the managing-app policy on the organization's own PDS,
  as above. It works on any PDS that supports simple-spaces, and keeps
  the number of spaces small (one per conference).
- **Long term:** an **event host**, a PDS we run for organizations and
  events. It would host their repos and spaces natively, apply
  eventside's rules without a `checkUserAccess` round trip, and create
  spaces at will (per plan, per group, per poll). That's what being a
  space host was meant to give us.
- **Keeping the move open:**
  - Keep the access rules behind one interface: today eventside answers
    `checkUserAccess`, and later the event host calls the same code
    in-process.
  - Keep membership and role records signed and self-describing, so
    they move unchanged.
  - Avoid depending on anything that only works because the space sits
    on someone else's PDS.

  The event host would be its own feature, specced when it's needed.

**Test cases this changes** (approved by the user with the red tests,
2026-10-09):

- **TC-1** becomes "Creating a conference makes eventside its managing
  app".
- **TC-37:** signatures verify against a key in eventside's DID
  document, and eventside is the space's managing app.

### Red-test approval (2026-10-09)

The user approved the red tests ("approve let's go"), and with them:

- **The rewordings** of TC-1, TC-30 and TC-37 above.
- **Changes to frozen [`attendee-sign-in`](attendee-sign-in.md) tests**,
  committed with this feature's red tests:
  - TC-7 and TC-18 (`sign-in.test.ts`, `session.test.ts`) asserted the
    granted scopes were exactly `['atproto']`. They now accept `atproto`
    plus this feature's `app.eventside.*` space scopes
    (`tests/support/scopes.ts`).
  - TC-9 still asserts the exact client ID, now computed the way sign-in
    builds it for a scope list other than `atproto`.
- **TC-41, dating records.** A record counts by the revision its PDS
  assigned it, checked against membership at that revision, not by when
  eventside received it. (Vivarium stores a removed member's write, and
  sync finds it after they rejoin; dating by receipt would count it.)
  This replaces "the time eventside received or indexed the record" in
  the design.
- **TC-18:** vivarium words the two refusals differently
  (`UserNotAuthorized`, `SpaceNotFound`), so the test checks only that
  both are refused.
- **TC-24:** vivarium still stores a removed member's write, so the test
  checks that the space never takes it in.
- **Contract details:** the space is created with `appAccess: #open`, so
  the PDS always asks eventside; the organization's OAuth grant includes
  space create and write scopes; the CLI gains `records list`.

### Blocked: frozen TC-18 can't pass (2026-10-09)

The implementation passes every other test (73 of 74 integration tests,
all e2e, unit, Rust and tooling suites), but TC-18 fails whatever the
code does:

- The test asks for a credential for a space that doesn't exist. The
  helper `requestCredential` (`tests/support/other-app.ts`) first gets a
  delegation token from Mallory's PDS, and `delegationToken` throws on any
  answer other than 200.
- The organization's account is on the same vivarium, and vivarium's
  `getDelegationToken` checks that the space exists, so it answers
  `400 SpaceNotFound` before eventside is ever asked:
  `Error: getDelegationToken answered 400: {"error":"SpaceNotFound",…}`.
- The red-test approval expected that refusal from `getSpaceCredential`,
  one step later.

**Fix, approved by the user ("apply your fix", 2026-10-09):** `requestCredential` returns the
delegation-token refusal as its answer (status and body) instead of
throwing, so TC-18's "refused, no credential" check sees it. The test
itself doesn't change.


These cover the Nov 1 slice (design review round 1). Email matching,
request and approve, personal codes, invite-only and key removal are a
fast-follow feature.

The scenario is AtmosphereConf 2027 in Amsterdam:

- The organization "Atmosphere" has its own account on a PDS that
  supports spaces. In tests that's vivarium's PDS; in the demo, vlpds.
- **Olga** and **Kees** are owners. **Pim** and **Lotte** are staff.
- **Ana**, **Bram**, **Joost** and **Ruud** are attendees.
- **Mallory** isn't a member.

### The organization and its conference

#### TC-1: Creating a conference makes eventside its managing app

*(Reworded in design round 2; approved 2026-10-09.)*

- **Given** the organization's account on its PDS, connected with the CLI
- **When** Olga creates the conference
- **Then** its space exists on the organization's own PDS
- **And** the space's policy names eventside as its managing app, so the
  PDS asks eventside who may read and write

#### TC-2: A public conference publishes an event other calendar apps can read

- **When** Olga creates the public conference "AtmosphereConf 2027"
- **Then** a `community.lexicon.calendar.event` with its name, dates and
  city is in the organization's public repo
- **And** another calendar app can read it, along with a link to the
  conference

#### TC-3: Creating a conference sets up its main feed

- **When** Olga creates the conference
- **Then** the conference's main feed record exists in the organization's
  repo in the space

#### TC-4: Admins act only as themselves

- **Given** Pim has connected and Lotte hasn't
- **When** the CLI is asked to act as Lotte, or as Mallory
- **Then** it refuses, and nothing is decided
- **And** acting as Pim works, and the decision records Pim

#### TC-5: There's always an owner

- **Given** Olga is the only owner
- **When** anyone tries to remove Olga, make her staff, or have her leave
- **Then** it's refused
- **And** once Kees is an owner too, Olga can step down

### Finding a conference

#### TC-6: A non-member sees the public page and how to get in

- **Given** the conference accepts a shared code and has an attendee list
- **When** Mallory opens its link
- **Then** she sees its name, dates, city, description and branding
- **And** a way to enter a code, and "Sign in" for people on the list
- **And** nothing from inside it

#### TC-7: The public page works by DID or by handle

- **When** the conference's link names the organization by DID, or by
  handle
- **Then** both open the same page

### Joining

#### TC-8: Ana joins with a shared invite code

- **Given** the shared code "atmosphere27"
- **When** Ana signs in and enters it
- **Then** she's a member, and lands inside the conference

#### TC-9: A wrong code doesn't admit anyone

- **When** Mallory enters "atmosphere26"
- **Then** she's refused, and isn't a member

#### TC-10: Expired and used-up shared codes stop working

- **Given** one code that expired yesterday, and one limited to two uses
  that has been used twice
- **When** Joost tries each
- **Then** both are refused

#### TC-11: Being on the attendee list by handle admits you on sign-in

- **Given** Olga imported a list with Ana's handle
- **When** Ana signs in and opens the conference
- **Then** she's a member, without entering a code

#### TC-12: A handle on the list stays with the account it first named

- **Given** Bram's handle was on the imported list
- **When** Bram moves that handle to a new account, and someone else takes
  it over
- **Then** the account the handle named at import is the one admitted

#### TC-13: A handle that doesn't resolve is refused at import

- **When** Olga imports a list with a handle that doesn't resolve
- **Then** the import reports that row, and imports the rest

#### TC-14: An open conference admits anyone signed in

- **Given** Olga turns on open joining
- **When** Joost signs in and joins
- **Then** he's a member

#### TC-15: A conference with no way in for you refuses

- **Given** the conference only takes its attendee list
- **When** Mallory, who isn't on it, tries to join
- **Then** she's refused

#### TC-16: Too many join attempts are slowed down

- **When** Mallory tries one wrong code after another
- **Then** after a few attempts she's told to wait, even for a right
  code

### Inside, and which apps can read

#### TC-17: A member writes to the space but can't read other members' records

- **Given** Ana and Bram are members, and Bram has written records in the
  space
- **When** Ana writes a record in the space
- **Then** it's written
- **And** whatever Ana asks for, directly or through eventside, she gets
  none of Bram's raw records, only her own

#### TC-18: Non-members can't see inside

- **When** Mallory, or a visitor who isn't signed in, asks for anything
  inside the conference
- **Then** they get nothing, the same as for a conference that doesn't
  exist

#### TC-19: An allowed app reads the space with an organizer's delegation

- **Given** Olga has allowed another app to read the conference
- **When** that app uses Pim's delegation
- **Then** it can read every member's records in the space

#### TC-20: An allowed app can't read others' records for an attendee

- **Given** the same allowed app
- **When** it uses Ana's delegation
- **Then** it can read only Ana's own records

#### TC-21: An app that isn't allowed can't read at all

- **When** an app Olga hasn't allowed asks to read the space, with any
  delegation
- **Then** it's refused

#### TC-22: Staff can manage which apps may read

- **When** Pim allows an app, and later disallows it
- **Then** the app can read with an organizer's delegation, and afterwards
  can't

### Leaving, removal and bans

#### TC-23: Ana leaves, and can come back

- **Given** Ana joined with the shared code
- **When** she leaves
- **Then** she's no longer a member, and can't see inside
- **And** she can rejoin with the code

#### TC-24: A removed member loses access at once

- **Given** Ruud is a member, and is signed in
- **When** Pim removes him
- **Then** Ruud's next request inside the conference is refused
- **And** he can't write to the space any more

#### TC-25: A banned person can't get back in by any method

- **Given** Bram is a member, and is also on the attendee list
- **When** Olga bans him
- **Then** he's removed
- **And** the code, the list and open joining all refuse him

#### TC-26: Someone can be banned before they join

- **When** Olga bans Mallory, who has never joined
- **Then** Mallory can't join by any method

#### TC-27: Lifting a ban lets someone join again, without joining them

- **Given** Bram is banned
- **When** Olga lifts the ban
- **Then** Bram isn't a member
- **And** he can join again with the code

### Who can undo whom

#### TC-28: Staff can tighten an owner's admission

- **Given** Olga admitted Joost
- **When** Pim removes Joost, or bans him
- **Then** it takes effect

#### TC-29: Staff can't loosen an owner's removal or ban

- **Given** Olga removed Ana and banned Bram
- **When** Pim tries to admit Ana, or to lift Bram's ban
- **Then** both are refused, and nothing changes
- **And** Kees, an owner, can do either

#### TC-30: Rejoining after a removal depends on who removed you

- **Given** Pim removed Ruud, and Olga removed Joost
- **When** each tries to rejoin with the shared code
- **Then** Ruud gets back in
- **And** Joost is refused until an owner admits him *(reworded during
  the build to match the precedence table; approved 2026-10-09)*

#### TC-31: Only owners act on admins

- **When** Pim tries to remove Lotte, ban Kees, or make himself an owner
- **Then** each is refused
- **And** Olga can remove Lotte

#### TC-32: An owner demoted to staff keeps the weight of their owner decisions

- **Given** Kees banned Bram while he was an owner
- **When** Olga makes Kees staff
- **Then** Bram's ban still needs an owner to lift it, and Kees can't

#### TC-33: A former admin's decisions stand

- **Given** Pim admitted Joost while he was staff
- **When** Olga removes Pim as an admin
- **Then** Joost is still a member
- **And** Pim can no longer decide anything

#### TC-34: Between staff, the latest decision stands

- **Given** Ana joined with the code
- **When** Lotte removes her, and then Pim admits her
- **Then** Ana is a member

#### TC-35: Decisions made at the same moment are applied in order

- **When** the CLI removes Ana while the server handles her leaving and
  rejoining at the same time
- **Then** the result is the same as some order of those decisions, one
  after another
- **And** the records in the space match the decision that came last

#### TC-36: Staff can make an attendee a speaker

- **Given** Olga admitted Joost
- **When** Pim makes Joost a speaker
- **Then** Joost is a speaker

### Signed records

#### TC-37: Another app can verify who's a member and who organizes

- **Given** an allowed app with Olga's delegation
- **When** it reads the membership and role records in the space
- **Then** it finds Ana as an attendee and Pim as staff
- **And** each record's signature verifies against a key in eventside's
  DID document, and eventside is the space's managing app *(reworded in
  design round 2; approved 2026-10-09)*

#### TC-38: Records that aren't properly signed don't count

- **When** Mallory's app writes a record claiming she's an owner into
  its own repo, and a record in the organization's repo is altered so
  its signature no longer matches
- **Then** neither counts: Mallory isn't a member, and the altered record
  is ignored

#### TC-39: A new signing key is used, and older records still verify

- **When** the operator adds a second signing key, and Pim then admits
  Joost
- **Then** Joost's record is signed with the new key
- **And** earlier records still verify with the first one

#### TC-40: A decision interrupted by a crash is completed on restart

- **Given** the server stops after Pim admits Joost but before the record
  is written
- **When** the server starts again
- **Then** Joost's record appears in the space, and he can write to it

#### TC-41: A record written while not a member is never counted

- **Given** Ruud was removed
- **When** he writes a record into the space from another client, and is
  admitted again later
- **Then** the record he wrote while removed is never counted
- **And** records he writes after rejoining are

### Regressions

#### TC-42: Signing in still works, with the new permissions

- **When** Ana signs in
- **Then** she's asked for permission to write to conferences, and the
  sign-in cases of [`attendee-sign-in`](attendee-sign-in.md) still pass
- **And** someone signed in before this change is asked to sign in again
  before joining

## Review log

### Round 1

Reviewed `9aa0932`. `pnpm check` green; frozen files unchanged apart from
the approved `other-app.ts` fix.

1. **[major]** `decide.rs`: staff could undo an owner's removal by banning
   and then unbanning. **Fix:** a ban keeps the higher of the removal's
   and the ban's rank, and unban restores the removal in force before
   the ban; unit test for the sequence.
2. **[major]** `outbox.rs`: the lease is per process, so the background
   loop and `kick()` drain at once and a stale put can land after a
   delete. **Fix:** one drain per process behind a mutex, woken by
   `kick()`; the lease token is per drain.
3. **[major]** The folded-in `space-sync` parts (records index, sync and
   backfill, ingest hook) weren't built; `is_member_at` and
   `attest::verify` had no callers. **Fix:** build the index, sync and the
   ingest hook the design names, and have `records list` read the index.
4. **[major]** Dating by PDS rev lets a self-hosted PDS backdate a record,
   and clock skew can drop a genuine one. **Declined for now:** dating by
   rev is the rule the user approved at the red-test gate (2026-10-09).
   Changing it needs their decision, so it's raised in the PR.
5. **[minor]** `checkUserAccess` accepts a service JWT with no `lxm`.
   **Fix:** require it.
6. **[minor]** `checkUserAccess` fetches the authority's DID document on
   every call. **Fix:** cache with a short TTL, refetch once on a verify
   failure.
7. **[minor]** `conference create` isn't atomic or safe to retry. **Fix:**
   record the conference as pending first, and finish it on rerun.
8. **[minor]** `join_attempts` rows are never pruned. **Fix:** prune
   outside the window.
9. **[nit]** Every decision reads the whole log to count owners. **Fix**
   if cheap.

Rework (`a0c90ca`): findings 1, 2, 3, 5, 6, 7, 8 and 9 fixed as described;
4 left as approved. `pnpm check` green. The records index, sync,
`notifyWrite` registration and the `sync::on_ingest` hook for feeds are in
`conference/sync.rs`.

### Round 2

Reviewed `162128a`. `pnpm check` green; frozen files unchanged apart from
the approved `other-app.ts` fix.

1. **[major]** The public event's address has no `country`, which
   `community.lexicon.location.address` requires. **Fix:** leave the
   address out unless `--country` is given; don't repeat the city as
   `name`.
2. **[major]** The outbox lease can expire during one slow entry, so a
   second drain can apply the same subject and an older state can land
   last. **Fix:** after writing, re-check that the subject's latest `seq`
   is the one applied, and re-apply if not; renew the lease while
   applying.
3. **[minor]** `attest::keys()` generates a key and writes on every call,
   including every anonymous `did.json`. **Fix:** ensure the key at start
   only; `keys()` only reads.
4. **[minor]** A forged service JWT forces a DID refetch every time.
   **Fix:** refetch at most once per DID per interval.
5. **[minor]** A withdrawn signed record re-put into the org's repo
   counts again. **Fix:** a signed record counts only if its `seq` is the
   subject's current decision and its rkey is the subject.
6. **[minor]** `authorized()` has no rule for the authority, which the
   design says is always a writer. **Fix:** allow the authority.
7. **[minor]** Backfill reads every admitted repo every 30 s. **Fix:**
   rely on `notifyWrite`, backfill stale repos on a long interval, with
   bounded concurrency.
8. **[minor]** `--starts`/`--ends` are stored as typed, not normalized,
   and `ends < starts` isn't refused. **Fix:** store RFC 3339 and refuse.
9. **[nit]** An outbox comment claims ordering across drains that
   doesn't hold. **Fix:** correct the comment.

Rework (`ff5a34f`): findings 2–9 fixed as described. `pnpm check` green.
Finding 1 **partly fixed**: the city is no longer repeated as `name`, and
`country` is set when `--country` is given. **The rest is declined for
now:** frozen TC-2 checks that the event's locations contain the city,
and the frozen `createConference` helper never passes `--country`, so
leaving the address out would fail TC-2. Making the address always valid
needs the user's approval of a test change (e.g. the helper passing
`--country NL`); it's raised in the PR.

### Round 3

Reviewed `4aee0d5`. `pnpm check` green; frozen files unchanged apart from
the approved `other-app.ts` fix. Precedence, `checkUserAccess`, OAuth
connect and signature checks held up.

1. **[major]** The join-attempt limit is check-then-act: 40 concurrent
   wrong codes gave 39 code checks and one 429. **Fix:** count and record
   the attempt inside the decision transaction, before checking the code.
2. **[minor]** One failing conference can starve the outbox for the
   others (`LIMIT` before the per-conference filter). **Fix:** pick due
   entries per conference in SQL; back off a failed conference's later
   entries too; correct the comment.
3. **[minor]** Each `notifyWrite` spawns its own unbounded sync. **Fix:**
   coalesce per (space, writer) with a bounded worker; single-flight the
   credential fetch.
4. **[minor]** A listed attendee still has to press Join (TC-11 says
   they're a member on opening), and refusals mention a code when there's
   no code method. **Fix:** join without a code on open when `list` is a
   method; word refusals by method.
5. **[nit]** `join set` changes the database before writing the sidecar.
   **Fix:** write the sidecar first.

Rework (`84cadad`): all five fixed. `pnpm check` green. The join limit is
checked and recorded inside the decision transaction, with a Rust test
firing 40 wrong codes at once; only attempts that carry a code count, so
the page's code-less list join on open doesn't use up the limit.

### Round 4

Reviewed `d4b2a3b`. `pnpm check` green; frozen files unchanged apart from
the approved `other-app.ts` fix.

1. **[major]** The page's automatic list join readmits a listed attendee
   right after they leave (and readmits someone staff removed) just by
   opening the page. **Fix:** join on open only someone with no decision
   in this conference yet; an explicit Join still rejoins.
2. **[major]** Signed member, ban and apps records don't name their
   space, so a signed record copied into another conference space of the
   same organization still verifies for outside readers. **Fix:** add a
   required `space` field (the space URI) to the three records, and
   require it to match where the record was read. This adds a field to
   the record shapes in the design; it's called out in the PR.
3. **[minor]** A code-less join is refused with 429 after wrong-code
   attempts. **Fix:** apply the limit only to attempts with a code.
4. **[minor]** The credential single-flight is one global lock, so an
   unreachable organization stalls every conference. **Fix:** lock per
   space, with a short backoff after a failure.
5. **[nit]** `parse_iso` accepts impossible dates like 31 February.
   **Fix:** check the day against the month.

Rework (`580e9c1`): all five fixed, with Rust tests. `pnpm check` green.
`join` gains an `onOpen` flag: the page's automatic list join admits only
someone the conference has never decided about. Member, ban and apps
records carry a required, signed `space`, and verification requires it
to match the space the record was read from.
