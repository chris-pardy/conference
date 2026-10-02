---
status: design-review
impact: cross-cutting
depends-on: [ci-pipeline]
branch:
tests-commit:
---

# Attendee sign-in

## Summary

Attendees and organizers sign in with their atproto account. The appview
acts as a confidential OAuth client, a backend-for-frontend (BFF): it holds
the DPoP-bound tokens, and the PWA only ever has a session cookie. That
lets the appview act on a person's behalf once later features ask for more
access.

**Identity only.** The session asks for the `atproto` scope and nothing
else: it proves who the person is (their DID and handle) and grants no
repo, blob or space access. Features that need more, such as writing into a
space in [`block-actions`](block-actions.md) or an organizer's `read` grant
in [`space-sync`](space-sync.md), add their scopes themselves and send the
person back through consent when they first need it (decided after the
brainstorm, when the build was picked up).

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

**Organizers** sign in the same way, with the same identity-only scope.
The space `read` grant that [`space-sync`](space-sync.md) needs is that
feature's to ask for.

## Data

- **Sessions** live in the appview's database: DID, DPoP key, access and
  refresh tokens, and scopes. The PWA holds only an `HttpOnly`, `Secure`,
  `SameSite=Lax` cookie, and state-changing XRPC calls require a CSRF
  token.
- **Scopes:** `atproto` only, for everyone. The session records the
  scopes it was granted, so later features can tell when they need to ask
  for more.
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

### Which scopes

- **`atproto` only, identity** (chosen when the build was picked up): the
  smallest consent screen, and nothing here needs more. Each later feature
  adds the scopes it uses.
- The brainstormed set (`space:` `read_self` and `create` on
  `app.eventside.*` for attendees, plus `read` and `manage` for
  organizers): asks for access before any feature uses it.

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
- Any scope beyond `atproto`: `space:`, `repo:`, `blob:` and
  `transition:` grants belong to the features that use them.

## Open questions

- Session storage in the appview (SQLite, or something else) is the first
  database in the project. Architecture analysis should decide this for
  everyone.
- How the test helper signs in: automate vivarium's pick-an-account consent
  page, or have the appview accept a test-only token exchange. Vivarium has
  no OAuth helper in `@vivarium/client`.
- How a later feature asks for more scopes: a step-up sign-in that keeps
  the page, and what the PWA shows while it happens. This feature should
  provide the mechanism even though nothing uses it yet.

## Architecture analysis

**Existing code touched:**

- `crates/server`: today it's one `/health` route on axum with an
  `AppState` holding the atproto URL and an HTTP client. This feature adds
  the OAuth routes, client metadata and JWKS, session storage, cookie and
  CSRF middleware, and the server's first configuration beyond `PORT` and
  `ATPROTO_URL` (public URL, signing key, database path, cookie secret).
- `Cargo.toml` and `Cargo.lock`: the first database and crypto
  dependencies (DPoP and `private_key_jwt` need ES256 JWTs).
- `web/src/main.tsx` and `web/src/App.tsx`: a signed-in state for the shell,
  the sign-in and callback routes, and a "who am I" call. The app shell is
  still the `/health` status page.
- `web/vite.config.ts`: the dev and preview proxies, and the service
  worker's `navigateFallbackDenylist`, must let the OAuth endpoints
  (`/oauth/*`, client metadata, JWKS) reach the server rather than the
  cached app shell.
- `tests/support/server.ts` and `server-setup.ts`: `spawnServer` has to
  pass the new configuration, and gains the sign-in test helper.
- `e2e/` and `playwright.config.ts`: the first browser flow that leaves the
  app (to vivarium's consent page) and comes back, so the backend's public
  URL has to match the origin the browser uses.

**Features affected:**

- [`space-sync`](space-sync.md): it planned to get the organizer's space
  `read` grant from this feature's session. Now it has to ask for that
  scope itself, using the step-up mechanism this feature provides. It still
  reuses the client keys this feature publishes for its client
  attestations.
- [`block-actions`](block-actions.md): its `submitAction` relies on this
  feature's session cookie and CSRF token, unchanged. It planned to write
  into the space with scopes this feature granted, and now has to step up
  to `space:` `create` on `app.eventside.*` itself.
- [`block-offline`](block-offline.md): it re-asks for sign-in when a session
  expires with actions queued, so it depends on how expiry is reported to
  the PWA.
- [`ui-blocks`](ui-blocks.md) (complete): no behavior change. Its feature
  table describes this feature as carrying `space:` scopes, which is now
  out of date.
- Every later feature: its tests sign in with this feature's test helper,
  and its data lives in the database this feature introduces.

**New shared surfaces:**

- The appview's database, its migrations, and the decision on what it is.
- The session: a cookie, a CSRF token, a server-side extractor giving the
  signed-in DID to any route, and an authenticated client that makes
  DPoP-bound calls to the person's PDS.
- A step-up: a way for a feature to require more scopes, and to send the
  person back through consent without losing their page.
- The appview's client metadata and JWKS, whose keys `space-sync` reuses.
- The PWA's signed-in state (who's signed in, sign in, sign out, expired).
- The test helper that signs in a vivarium account without the consent
  screen.

**Verdict: cross-cutting.** This is auth, the first database and the first
real configuration of the server, and every later feature builds on its
session, its test helper or both. Narrowing the scopes to `atproto` also
moves work into `space-sync` and `block-actions`, which planned on
inheriting space grants from here and will now need the step-up mechanism
this feature has to design.

## Design review

## Test cases

## Review log
