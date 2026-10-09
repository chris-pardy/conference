---
status: analysis
impact:
depends-on: [ci-pipeline, attendee-sign-in, conference-space]
branch:
tests-commit:
---

# Space sync

## Summary

> **Superseded (2026-10-09).** This feature was folded into
> [`conference-space`](conference-space.md) in its design review round 1.
> The salvaged space host already holds eventside's credentials,
> syncer registration, backfill, the records index and the ingest hook.
> Nothing depends on this file any more; it's kept as a record.

This is the appview's own access to permissioned spaces. The appview:

- has an identity of its own
- is the only client allowed to hold space credentials
- follows every member's repo in a space
- keeps an index that views and the feed are computed from

This is what enforces the privacy model: members read only their own
records, and see other people's data only through what the appview chooses
to serve.

Split out of [`ui-blocks`](ui-blocks.md) in design review round 1, where
the critique found this subsystem hidden inside "resolve as the viewer".
[`feeds`](feeds.md) needs it too.

## Experience

Nobody sees this directly. What changes is visible to attendees through
later features:

- Seconds after someone writes to the space, the appview has indexed the
  record, and views built on it update.
- Attendees can't read other members' raw records, only what the appview
  serves.
- Records written behind the appview's back (directly to the PDS) are
  indexed like any others. Features decide whether to trust them. For
  example, `block-actions` re-validates them.

**Organizers** connect a space to the appview once, by signing in with a
`read` grant (see [`attendee-sign-in`](attendee-sign-in.md)). The appview
keeps its access alive from then on.

## Data

- **The appview's identity** is a `did:web` for the appview host. Its DID
  document names the service endpoint that receives `notifyWrite`
  callbacks.
- **Space configuration** (set by whoever creates the space, later
  `conference-space`):
  - the app allow-list admits only the appview's OAuth client, verified
    through client attestation, so only the appview can obtain space
    credentials
  - members get only `read_self`
- **Credentials:**
  - A delegation token comes from the organizer's session with a `read`
    grant.
  - Exchanging it, together with the appview's client attestation, yields a
    space credential valid for about 2 hours.
  - The credential is refreshed before it expires, and refresh failures are
    surfaced to the organizer.
- **Notifications:** `registerNotify`, renewed every 24 hours. Each
  notification triggers a `listRepoOps` pull for that repo from its stored
  cursor.
- **The index** lives in the appview's database: one row per record, keyed
  by space, author, collection and rkey, with its value, CID and when it
  was indexed. There's a cursor per repo. `space.listRepos` finds members
  who are new since the last sync.
- **The read API** is internal to the appview, and used by features that
  compute views. It is never exposed raw to clients.

## Options considered

### Privacy model

- **Members hold `read_self`; the appview is the only full reader, through
  the app allow-list and attestation** (chosen by the user). Verified in
  vivarium:
  - user tokens can read only their own repo
  - `getSpaceCredential` enforces the allow-list
- All members read everything: no secret ballots or anonymous Q&A.

### How changes arrive

- **Write notifications plus a `listRepoOps` pull per repo** (chosen): the
  spec-intended path, incremental and cursor-based.
- Polling every repo: simple, but slow and costly as membership grows.

## Out of scope

- Creating spaces, managing members and setting policy
  (`conference-space`). Tests create spaces directly.
- Any view or aggregate: those belong to the features that need them
  (`block-actions`, [`feeds`](feeds.md)).
- Writing on a user's behalf: `attendee-sign-in` sessions do that.

## Open questions

- What happens when the organizer's delegating session is revoked: the
  space goes stale, and who gets told?
- How far back to backfill on first sync: everything, or from space
  creation (the same thing for new spaces).
- Rate limits and batching for large conferences (thousands of member
  repos).

## Architecture analysis

## Design review

## Test cases

## Review log
