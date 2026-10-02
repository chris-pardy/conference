---
status: analysis
impact:
depends-on: [ci-pipeline]
branch:
tests-commit:
---

# Attendee sign-in

## Summary

Attendees and organizers sign in with their atproto account. The appview
acts as a confidential OAuth client, a backend-for-frontend (BFF): it holds
the DPoP-bound tokens, and the PWA only ever has a session cookie. That
lets the appview act on a person's behalf. For example, it can write
action records into a space as the attendee.

This feature also ships a test helper, so every later feature's tests can
sign in a vivarium account without driving the consent screen.

Split out of [`ui-blocks`](ui-blocks.md) in design review round 1: writing
to a space as an attendee needs an OAuth session with a `space:` scope, and
nothing provided one.

## Experience

**Signing in:**

1. On the PWA, the attendee taps "Sign in" and enters their handle.
2. They're sent to their PDS's consent screen.
3. They come back signed in, and see their avatar and handle.

**Staying signed in:**

- Signing out ends the session on the appview and revokes its tokens.
- The session survives app restarts.
- An expired session asks the person to sign in again, keeping them on the
  page they were on.

**Organizers** sign in the same way. Their session carries a space `read`
grant, which [`space-sync`](space-sync.md) uses to delegate the appview's
access to spaces they own.

## Data

- **Sessions** live in the appview's database: DID, DPoP key, access and
  refresh tokens, and scopes. The PWA holds only an `HttpOnly`, `Secure`,
  `SameSite=Lax` cookie, and state-changing XRPC calls require a CSRF
  token.
- **Scopes:**
  - Attendees get `space:` with `read_self`, plus `create` on the
    `app.eventside.*` collections.
  - Organizers additionally get space `read` and `manage`.
- **Client metadata and signing keys:** the appview publishes its client
  metadata document and JWKS. The same keys sign the space client
  attestations that `space-sync` uses.
- **No atproto records** are written by this feature.

## Options considered

### Where tokens live

- **The appview holds them (BFF)** (chosen): the appview has to act as the
  user for space writes and ingest, the tokens never touch the browser, and
  it's a confidential client with attestation keys.
- A browser-held public OAuth client: the appview couldn't act for the user
  without the tokens being forwarded, and public clients can't do space
  client attestation.

### Where it's specced

- **Its own feature** (chosen in ui-blocks design review round 1): auth is
  foundational, and every feature reuses it.
- Folded minimally into ui-blocks: rejected.

## Out of scope

- Accounts, sign-up, and creating handles. People bring an existing
  atproto account.
- Roles and permissions beyond OAuth scopes. Who organizes which conference
  is `conference-space`.
- Native app sign-in (Capacitor deep links), for a later mobile feature.

## Open questions

- Session storage in the appview (SQLite, or something else) is the first
  database in the project. Architecture analysis should decide this for
  everyone.
- How the test helper signs in: automate vivarium's pick-an-account consent
  page, or have the appview accept a test-only token exchange. Vivarium has
  no OAuth helper in `@vivarium/client`.
- The exact scope strings for the `app.eventside.*` namespace.

## Architecture analysis

## Design review

## Test cases

## Review log
