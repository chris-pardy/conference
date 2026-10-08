---
status: design-review
impact: cross-cutting
depends-on: [attendee-sign-in]
branch:
tests-commit:
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

## Test cases

## Review log
