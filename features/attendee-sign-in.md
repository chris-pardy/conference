---
status: implementing
impact: cross-cutting
depends-on: [ci-pipeline]
branch: feature/attendee-sign-in
tests-commit: 24a32a9cafbe056c159318ef49b0c4542a640736
---

# Attendee sign-in

## Summary

Attendees and organizers sign in with their atproto account, or create one
if they don't have one yet. The appview
acts as a confidential OAuth client, a backend-for-frontend (BFF): it holds
the DPoP-bound tokens, and the PWA only ever has a session cookie. That
lets the appview act on a person's behalf once later features ask for more
access.

**Identity only.** The session asks for the `atproto` scope and nothing
else: it proves who the person is (their DID and handle) and grants no
repo, blob or space access. The scopes asked for at sign-in are one list
in code. When a later feature needs more, such as writing into a space in
[`block-actions`](block-actions.md), it adds its scopes to that list, and
everyone signing in from then on grants them. People whose sessions
predate the change are asked to sign in again. There's no step-up (asking
for more scopes mid-session) for now. That comes later, for conferences
with write-heavy plugins. (Decided after the brainstorm, when the build was
picked up, and in design review round 3.)

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

**Signing up** (for people without an atproto account):

1. Next to "Sign in", the attendee taps "Create account".
2. They're sent to the appview's sign-up PDS, set in its configuration
   (`bsky.social` in production, vivarium in tests), with OAuth
   `prompt=create`, and create their account and handle there.
3. They come back signed in, exactly as if they'd signed in.

Signing up only creates the atproto account. Joining a conference (code of
conduct, display name, pronouns and the like) is per-conference onboarding,
a later feature built on this one.

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

### What "sign up" means

- **Creating an atproto account, at one configured PDS** (chosen when the
  build was picked up): newcomers need a way in, and atproto OAuth's
  `prompt=create` does it without the appview handling passwords.
- Letting people pick a PDS to sign up on: more choice, but more UI and a
  decision most attendees can't make. A later feature can add it.
- A sign-up step in the app (name, code of conduct): it's per conference,
  so it belongs to a conference onboarding feature, not to sign-in.

### Where it's specced

- **Its own feature** (chosen in ui-blocks design review round 1): auth is
  foundational, and every feature reuses it.
- Folded minimally into ui-blocks: rejected.

## Out of scope

- Per-conference onboarding: code of conduct, display name, pronouns and
  other details a conference asks for when someone joins it.
- Choosing which PDS to sign up on, and running a PDS of our own.
- Account management: changing handles, deleting accounts. That's the PDS's
  job.
- Roles and permissions beyond OAuth scopes. Who organizes which conference
  is `conference-space`.
- Native app sign-in (Capacitor deep links), for a later mobile feature.
- Any scope beyond `atproto`: `space:`, `repo:`, `blob:` and
  `transition:` grants belong to the features that use them.

## Open questions

None. All were resolved in design review:

- **Session storage:** SQLite or Postgres, through sqlx `Any` (rounds 1–2).
- **How the test helper signs in:** it runs the real flow against
  vivarium's no-JavaScript consent form (round 1).
- **How later features get more scopes:** they add them to the sign-in
  scope list, and there's no step-up for now (round 3).

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
  `read` grant from this feature's session. Now it has to add that scope
  to the sign-in list itself. It still reuses the client keys this feature
  publishes for its client attestations.
- [`block-actions`](block-actions.md): its `submitAction` relies on this
  feature's session cookie and CSRF token, unchanged. It planned to write
  into the space with scopes this feature granted, and now has to add
  `space:` `create` on `app.eventside.*` to the sign-in list itself.
- [`block-offline`](block-offline.md): it re-asks for sign-in when a session
  expires with actions queued, so it depends on how expiry is reported to
  the PWA.
- [`ui-blocks`](ui-blocks.md) (complete): no behavior change. Its feature
  table describes this feature as carrying `space:` scopes, which is now
  out of date.
- Per-conference onboarding (not specced yet): it runs after sign-in or
  sign-up and keys everything on the signed-in DID this feature provides.
- Every later feature: its tests sign in with this feature's test helper,
  and its data lives in the database this feature introduces.

**New shared surfaces:**

- The appview's database, its migrations, and the decision on what it is.
- The session: a cookie, a CSRF token, a server-side extractor giving the
  signed-in DID to any route, and an authenticated client that makes
  DPoP-bound calls to the person's PDS.
- The sign-in scope list, which later features extend, and the re-sign-in
  that follows when it grows.
- Background token renewal, which keeps every live session's tokens fresh
  for whatever later features do with them.
- The appview's client metadata and JWKS, whose keys `space-sync` reuses.
- The PWA's signed-in state (who's signed in, sign in, sign out, expired).
- The test helper that signs in a vivarium account without the consent
  screen.

**Verdict: cross-cutting.** This is auth, the first database and the first
real configuration of the server, and every later feature builds on its
session, its test helper or both. Narrowing the scopes to `atproto` also
moves work into `space-sync` and `block-actions`, which planned on
inheriting space grants from here and will now add their own scopes to the
sign-in list.

## Design review

### Approach

The appview is a **confidential atproto OAuth client**. Its client ID is
`<PUBLIC_URL>/oauth-client-metadata.json`, and it authenticates to
authorization servers with `private_key_jwt`, signed by an ES256 key whose
public half it serves at `<PUBLIC_URL>/oauth/jwks.json`. `PUBLIC_URL` is the
origin the browser uses: the PWA and the appview share it, so the session
cookie is first-party and the callback lands on the same origin. In dev and
tests the Vite server proxies the OAuth paths to the appview, as it already
does for `/xrpc/`.

**Scopes.** `LOGIN_SCOPES` in `oauth/scopes.rs` is the one list of scopes
asked for at sign-in. Today it's just `atproto`, and later features add
theirs to it. The client metadata's `scope` and every PAR use it.
`OAUTH_SCOPES` overrides it from the environment, which tests use to
simulate a feature adding a scope.

**Signing in** is a top-level navigation, not a fetch:

1. The PWA sends the browser to `GET /oauth/login?handle=…&return_to=…`.
2. The appview resolves the handle to a DID, the DID to its PDS, and the PDS
   to its authorization server (protected-resource metadata, then
   authorization-server metadata). The spec's checks apply: the metadata's
   `issuer` is the URL it was fetched from, and the PDS lists that server.
3. It makes a PKCE verifier, a `state`, a fresh DPoP key and a random
   **pre-auth cookie** (`HttpOnly`, ten minutes, `SameSite=Lax`). It sends
   a pushed authorization request (PAR) with `LOGIN_SCOPES` and the handle
   as `login_hint`. It then stores the pending request: state, verifier,
   DPoP key, issuer, expected DID, `return_to`, and the pre-auth cookie's
   hash.
4. It redirects the browser to the authorization server.
5. The browser returns to `GET /oauth/callback?code&state&iss`. The appview
   consumes the pending request (single use). It rejects the callback if
   the browser's pre-auth cookie doesn't match, which stops an attacker
   from handing someone a callback URL that signs them into the attacker's
   account (login CSRF). Then it:
   - checks `iss`
   - exchanges the code for DPoP-bound tokens
   - checks that `sub` is the expected DID and the granted scope includes
     every scope in `LOGIN_SCOPES`
   - resolves that DID's PDS again and checks that its authorization server
     is the issuer that answered
6. It deletes any session the browser already had and creates a new one
   with a new ID, so a planted ID can never be promoted (session fixation).
   It sets the cookie and redirects to `return_to`.

`return_to` is parsed against `PUBLIC_URL` and kept only if the origin
matches. That rejects `//host`, `/\host`, encoded variants and control
characters, and anything rejected becomes `/`. The same check applies
wherever `return_to` is echoed back, including `/signin?error=`.

**Signing up** is the same flow with three differences. It starts at
`GET /oauth/signup?return_to=…`, and the authorization server is discovered
from the configured `SIGNUP_PDS_URL` instead of a handle. The PAR adds
`prompt=create` and has no `login_hint`. There's no expected DID, so the
step 5 check that the DID's PDS uses this issuer is what keeps the result
honest.

**Failures** (unknown handle, consent denied, an expired or mismatched
pending request, a mismatched issuer or DID) redirect to
`/signin?error=<code>&return_to=…`, and the PWA shows a message for each
code.

**Fetching what DID documents point at.** Handles and DID documents (a
`did:web` especially) can point the appview at any URL. So, outside dev,
the appview refuses private, loopback and link-local addresses for PDS,
metadata and `did:web` fetches. `ALLOW_PRIVATE_NETWORK=true` turns that
off for dev and tests, where everything is vivarium on `localhost`.

**Sessions.** The cookie holds a random 256-bit ID, and the database keeps
only its SHA-256 hash. Its `Max-Age` is `SESSION_IDLE_TIMEOUT` (default 30
days), renewed at most once an hour on use, so the session survives the
browser or the installed PWA closing. On HTTPS it's `__Host-session`,
`Secure`, `HttpOnly` and `SameSite=Lax`. On loopback HTTP (dev and tests)
it's `session` without `Secure`.

Each session has its own CSRF token. Every state-changing request (any
non-`GET` under `/xrpc/`, and `/oauth/logout`) must send it in an
`X-CSRF-Token` header. One middleware enforces this, so later features get
it without doing anything.

**How a session ends:**

| How | What happens | The next request |
|---|---|---|
| Signing out | The row is deleted and the cookie cleared | `401 AuthRequired`. The PWA shows the signed-out state, not "expired". |
| Idle past `SESSION_IDLE_TIMEOUT` | The session is marked ended | `401 SessionExpired` |
| The authorization server answers a refresh with `invalid_grant` (found by the renewer or on demand) | The session is marked ended | `401 SessionExpired` |
| `LOGIN_SCOPES` has grown since the session was granted | The session is marked ended the next time it's used | `401 SessionExpired`. The banner signs them in again, now with the new scopes. |

An ended session isn't deleted straight away. Its tokens and keys are
wiped, but the row keeps the DID and handle until the cookie's own
lifetime would have run out. That's what lets the "expired" banner sign
the same person back in (`login_hint`) and return them to the page they
were on. A sweeper deletes these rows and stale pending requests later.

Network errors and `5xx` from the authorization server never end a
session. They surface as `503` to the feature that asked.

**Tokens are renewed in the background.** A renewer task runs every
`TOKEN_RENEW_INTERVAL` (default 5 minutes). It refreshes every live session
whose access token expires before the next run plus `TOKEN_REFRESH_SKEW`
(default 60 s). That means:

- tokens are fresh whenever a feature needs them
- a live session's refresh token never lapses at the authorization server
  for lack of use
- a revoked grant or a deleted account is noticed within one interval, and
  the session ends

Sessions past their idle timeout aren't renewed: the renewer ends them.
`pds_client()` still refreshes on demand as a fallback, for example right
after a restart. `getSession` only reads the row, so opening the app never
calls the authorization server. The renewer works in batches with bounded
concurrency (8 at a time), and backs off on network errors and `5xx`.

**One refresh at a time.** Refresh tokens are single-use. A second refresh
with the same token gets `invalid_grant`, which would wrongly end the
session. So every refresh, whether from the renewer or on demand, first
takes a 30-second lease on the row:

```sql
UPDATE sessions SET refresh_lease_until = $now + 30000
WHERE id_hash = $1 AND (refresh_lease_until IS NULL OR refresh_lease_until < $now)
```

Only the caller that wins the lease talks to the authorization server.
Others wait and re-read the row. An `invalid_grant` ends the session only
if the row still holds the refresh token that was sent. This works the same
on SQLite and on several Postgres-backed instances. DPoP nonces are cached
per server in memory.

**Sign out** is `POST /oauth/logout`, with CSRF. It revokes the refresh
token at the authorization server (best effort, logged on failure), deletes
the session row, and clears the cookie.

**Who's signed in.** The query `app.eventside.auth.getSession` returns
`{did, handle, displayName?, avatar?, scopes, csrfToken}`. The handle comes
from the DID document and is checked in both directions. The display name
and avatar come from the person's public `app.bsky.actor.profile` record,
read unauthenticated from their PDS at sign-in and stored on the session.
They don't change until the next sign-in. A missing profile is fine: the
PWA falls back to the handle and an initial.

**Startup never depends on atproto.** It runs migrations, and loads or
generates the signing key. Discovery happens per sign-in. The existing
`server.test.ts` starts the server with an unreachable `ATPROTO_URL`, and
that keeps working.

### Components

**Server (`crates/server`):**

- `config.rs`:

  | Setting | Notes |
  |---|---|
  | `PUBLIC_URL` | Default `http://127.0.0.1:<bound port>` |
  | `DATABASE_URL` | `sqlite://…` or `postgres://…` (see Data). Default `sqlite://<data dir>/eventside.db?mode=rwc` |
  | `OAUTH_SIGNING_KEY` | An ES256 private JWK. Production sets it. Without it, the server generates a key once and keeps it in the database (insert-if-absent, so several instances agree), so local sessions survive restarts. |
  | `OAUTH_SCOPES` | Overrides `LOGIN_SCOPES`. Tests and operations only. |
  | `SIGNUP_PDS_URL` | Where "Create account" sends people |
  | `PLC_URL`, `HANDLE_RESOLVER_URL` | Identity. Both default to `ATPROTO_URL`, so dev and tests point everything at vivarium. |
  | `ALLOW_PRIVATE_NETWORK` | On in dev and tests |
  | `SESSION_IDLE_TIMEOUT`, `TOKEN_RENEW_INTERVAL`, `TOKEN_REFRESH_SKEW` | Session and renewal timing |

- `db.rs` and `migrations/`: the database pool and embedded migrations,
  for either backend (see Data).
- `identity.rs`: handle to DID (via `com.atproto.identity.resolveHandle` on
  the resolver), DID to document (PLC or `did:web`), and the PDS and handle
  from the document, with the bidirectional handle check and the
  private-network guard.
- `oauth/`: client metadata and JWKS, discovery, PAR, the callback, refresh
  and revocation, DPoP (including the nonce retry), client assertions,
  `LOGIN_SCOPES`, and the background renewer. Built on `atproto-oauth` (see
  Alternatives).
- `auth/`: the session store, the cookies, the CSRF middleware, the
  `CurrentUser` extractor, and the login, signup,
  callback and logout routes.
- `xrpc/auth.rs`: `app.eventside.auth.getSession`.

**Lexicons:** `lexicons/app/eventside/auth/getSession.json`, a query with the
output above and the errors `AuthRequired` and `SessionExpired`. It's
generated into `web/src/lexicon` like the block lexicons.

**PWA (`web/src`):**

- `auth/SessionProvider.tsx` and `useSession()`: loads `getSession` once,
  and exposes `signedOut | signedIn {user} | expired {handle}`.
- `api.ts`: the fetch wrapper every later feature uses. It adds
  `X-CSRF-Token`, and turns `SessionExpired` into the expired state.
- `routes/signin.tsx`: a handle field, "Sign in", "Create account", and the
  error messages.
- `Shell.tsx`, a layout route in `main.tsx` that wraps every page:
  - It holds the `SessionProvider`, the header and its account control.
    Signed out, the control shows "Sign in". Signed in, it shows the avatar
    and handle with a "Sign out" menu.
  - It shows the expired banner, whose button signs the same person back
    in and returns to the same page.
  - `App` itself is untouched, so its existing tests (which stub `fetch`
    and render `App` without a provider) keep passing.
- `web/vite.config.ts`: proxy `^/oauth/` and
  `^/oauth-client-metadata\.json$`, and add both to the service worker's
  `navigateFallbackDenylist`.

**Test support:**

- `tests/support/server.ts`: `spawnServer` takes
  `{databaseUrl?, port?, publicUrl?, env?}`. It makes a temp SQLite file
  when no `databaseUrl` is given. A test restarts a server by stopping it
  and spawning again with the same database and port, so the client ID
  stays the same. It always sets `ALLOW_PRIVATE_NETWORK`, plus a short
  `TOKEN_RENEW_INTERVAL` (1 s) for the tests that need it.
- `tests/support/auth.ts`:
  - `signIn(serverUrl, handle, {scope?})` returns
    `{cookie, csrfToken, did, handle}`. It runs the real flow over HTTP with
    a cookie jar (the pre-auth cookie included): it follows `/oauth/login`
    to vivarium, reads the `request_uri` and `client_id` out of the consent
    form, posts the account, and follows the redirect back to the callback.
  - `signUp(serverUrl, handle)` does the same through `/oauth/signup`.
    Vivarium creates the account on the fly, and the helper returns the new
    DID so the test can `viv.track()` it for cleanup.
- `e2e/support/auth.ts`: `signInAs(page, handle)` runs `signIn` against the
  preview origin and adds the cookie to the browser context. UI tests that
  are about signing in drive the real screens instead.
- `playwright.config.ts`:
  - The preview runs with `--host 127.0.0.1`, and `baseURL` becomes
    `http://127.0.0.1:<preview port>`. Vivarium accepts client IDs on
    `127.0.0.1`, and the cookie, the client ID and the callback all have to
    share one origin.
  - The server's `webServer` entry gets
    `PUBLIC_URL=http://127.0.0.1:<preview port>`, a temp `DATABASE_URL`,
    and the test settings above.

**How the hard cases get tested:**

- **Background renewal:** with a 1 s interval and a large skew, a
  session's access token changes in the test's database with no request
  made.
- **Refresh refused:** delete the account in vivarium (`viv.deleteAccount`).
  The renewer gets `invalid_grant`, and `getSession` then answers
  `SessionExpired` without the person doing anything.
- **Scopes grew:** sign in, restart on the same database and port with
  `OAUTH_SCOPES="atproto transition:generic"`, and see `SessionExpired`.
  Signing in again grants both.
- **Idle expiry:** `SESSION_IDLE_TIMEOUT=2s`.
- **Restart survival:** stop and respawn on the same database and port,
  with the same cookie.
- **Sign-out revocation:** the test owns the temp database. It reads the
  refresh token before signing out, then shows that vivarium refuses it
  afterwards (by trying a refresh at its token endpoint, or by
  introspection). Which of the two vivarium supports gets settled when the
  tests are written. If neither works, revocation is covered by a Rust
  test against a stub server.

### Data

The appview's own database, **SQLite or Postgres**, chosen by the scheme of
`DATABASE_URL`. SQLite suits a single box or a small event. Postgres suits
a hosted appview with more than one instance. Nothing is written to
atproto.

**Supporting both.** sqlx's `Any` driver gives one pool type, so feature
code doesn't care which backend it's on. Two rules keep the SQL portable:

- **Schema:** one set of migrations, using only types both backends share.
  `TEXT` covers IDs, JWKs and tokens, and `BIGINT` covers epoch-millisecond
  timestamps. There's no `BLOB`/`BYTEA`, no native booleans and no JSON
  column types.
- **Queries:** `$1`-style placeholders, which SQLite also accepts, and no
  backend-specific SQL. Upserts use `INSERT … ON CONFLICT`, which both
  support.

A feature that someday needs something backend-specific adds a
per-backend migration directory then, not now.

| Table | Columns | Written by | Read by |
|---|---|---|---|
| `oauth_requests` | `state` (key), `kind` (login, signup), PKCE verifier, DPoP private key, issuer, expected DID, pre-auth cookie hash, `return_to`, `expires_at` | `/oauth/login` and `/oauth/signup` | `/oauth/callback`, which deletes the row |
| `sessions` | `id_hash` (key), DID, handle, display name, avatar URL, DPoP private key, issuer, access and refresh tokens, `token_expires_at`, `refresh_lease_until`, scopes, CSRF token, `created_at`, `last_seen_at`, `ended_at` | callback, renewer, on-demand refresh | every authenticated request |
| `client_keys` | `kid`, private JWK, `created_at` | startup, only when `OAUTH_SIGNING_KEY` is unset | client assertions, JWKS, and later `space-sync` |

Expired rows in `oauth_requests` and idle `sessions` are swept on a timer.
The schema is the first in the project, so the `migrations/` convention it
sets is the one later features follow.

### Interfaces

For later features:

- **Rust:**
  - `CurrentUser`: an axum extractor with `did`, `handle` and `scopes`.
    It rejects with `AuthRequired` or `SessionExpired`.
  - `LOGIN_SCOPES`: a feature that needs a scope adds it here.
  - `user.pds_client().await?`: an HTTP client for the person's PDS that
    adds DPoP-bound auth and refreshes as it goes. `block-actions` writes
    through it.
  - `ClientKeys`: the signing keys, for `space-sync`'s client attestation.
  - `AppState::db()`: the pool.
  - The CSRF middleware covers every non-`GET` XRPC route.
- **HTTP:** `/oauth/login` (`handle`, `scope`, `return_to`),
  `/oauth/signup` (`return_to`), `/oauth/callback`, `POST /oauth/logout`,
  `/oauth-client-metadata.json`, `/oauth/jwks.json`, and
  `app.eventside.auth.getSession`.
- **PWA:** `useSession()`, `api.ts`, and the `Shell` layout.
- **Tests:** `signIn`, `signUp` and `signInAs`.

### Impact on existing features

- [`ci-pipeline`](ci-pipeline.md) (complete): the CI workflow is unchanged,
  and the new tests run under the same sealed vivarium. Its test plumbing
  does change:
  - `spawnServer` gains options and a temp database. Existing calls keep
    working.
  - Playwright moves to a `127.0.0.1` origin, and its server entry gains
    `PUBLIC_URL` and `DATABASE_URL`.
  - The existing e2e specs use relative URLs through `baseURL`, so they're
    unaffected.
- [`ui-blocks`](ui-blocks.md) (complete): no behavior change. The
  `/dev/blocks` gallery stays public, and it now sits inside the `Shell`
  layout with the header.
- [`space-sync`](space-sync.md): it needs an organizer's space `read`
  grant. Adding that to `LOGIN_SCOPES` would ask every attendee for it, so
  space-sync has to choose between that and bringing step-up forward for
  organizers. It reuses `ClientKeys` for attestation. Its spec needs
  updating when it's analyzed.
- [`block-actions`](block-actions.md): its `submitAction` uses `CurrentUser`
  and the CSRF middleware as planned. It adds `space:` `create` on
  `app.eventside.*` to `LOGIN_SCOPES`, so existing sessions sign in again
  once when it ships. It writes with `pds_client()`, whose tokens the
  renewer keeps fresh.
- [`block-offline`](block-offline.md): `SessionExpired` and the expired
  state in `useSession()` are what it reacts to when the queue can't be
  sent.

### Alternatives

- **A test-only token exchange:** not needed. Vivarium's consent page is a
  plain form, so the helper runs the real flow, and the appview has no test
  backdoor.
- **Stateless signed session cookies:** the tokens have to live server-side
  anyway, and revocation needs a row to delete.
- **SQLite only, or Postgres only** (asked for both during design review):
  SQLite alone can't scale past one instance, and Postgres alone makes
  every dev box and test run need a database server.
- **A storage trait with a hand-written implementation per backend:** that
  doubles every query in every later feature. Portable SQL through `Any` is
  one implementation.
- **rusqlite on `spawn_blocking`:** works, but sqlx gives an async pool and
  embedded migrations with less glue.
- **Discovering the sign-up PDS per conference, or letting people choose:**
  rejected in the feature description. One configured PDS for now.
- **A separate auth service:** another deployable for a single client.
- **Step-up (asking for more scopes mid-session):** drafted in round 1 and
  removed in round 3. Nothing needs it yet. It returns later, for
  conferences with write-heavy plugins.
- **Refreshing only on demand:** dropped in round 3. Refresh tokens could
  lapse unused, and a revoked grant wasn't noticed until a feature next
  wrote something.

**The OAuth library.** We read the source of each candidate:

- **`atproto-oauth` 0.14, used as a toolkit** (chosen):
  - `oauth_init_with_prompt` takes authorization-server metadata directly,
    with `prompt` and `login_hint`. That covers sign-up, and a later
    step-up.
  - It signs `private_key_jwt` client assertions for PAR, token and
    refresh, and has DPoP nonce-retry middleware.
  - It doesn't enforce https, and it's on rustls.
  - What we write ourselves: revocation (one POST), handle resolution
    through `resolveHandle` and the PLC URL (its resolver does DNS and
    https only), and the session store.
- **`jacquard-oauth` 0.12:** the most complete client, but its prompt enum
  has no `create`, it can't take extra parameters, it ignores the scopes
  passed to `start_auth`, and it treats `http://` service URLs as handles.
- **`atrium-oauth` 0.1:** the same `https://` assumption, with no way
  around it (its internals are private), and no `create` prompt. Its
  default features also pull in native-tls.
- **Hand-rolling** on `p256`: roughly 600–900 lines. It's the fallback if
  `atproto-oauth` gets in the way, since we keep the protocol pieces behind
  our own `oauth/` module either way.

### Risks

- **Cookies on plain-HTTP origins.** Dev and e2e run on `http://localhost`,
  where `Secure` cookies work in Chromium (the only Playwright project) but
  not every browser. Rule: `Secure` and the `__Host-` prefix when
  `PUBLIC_URL` is HTTPS, a plain `HttpOnly` cookie on loopback HTTP.
- **Sign-up is only partly testable.** Vivarium ignores `prompt=create`, so
  tests prove the flow and the post-sign-up checks, but not a real PDS's
  sign-up screen. A Rust unit test pins that the PAR carries
  `prompt=create`.
- **sqlx `Any` is the lowest common denominator.** It decodes a smaller set
  of types than the native drivers do, and portable SQL takes discipline in
  every later feature. **Postgres isn't tested yet** (decided in round 2).
  The suite runs on SQLite only, so SQL that breaks on Postgres can slip
  through until a Postgres CI job is added, which should happen before
  anyone deploys on Postgres.
- **Renewal load grows with live sessions.** Every live session refreshes
  about once per access-token lifetime. That's trivial for one conference,
  but a large multi-conference appview would want a cap or a longer
  interval. Both are configuration.
- **Growing `LOGIN_SCOPES` signs everyone out once**, and puts the new
  scopes on everyone's consent screen, organizer-only ones included. That's
  accepted until step-up exists.
- **Tokens at rest.** The database holds refresh tokens and DPoP keys.
  It's exactly as sensitive as `OAUTH_SIGNING_KEY`, and production has to
  protect it the same way. There's no field encryption in v1.
- **Production topology** (whether the appview serves the PWA build or sits
  behind a shared reverse proxy) isn't decided yet. The design only needs
  one origin for both, which either option gives.
- **`atproto-oauth` has had no release in six months**, and its newer
  RustCrypto versions (`sha2` 0.11, `rand` 0.10) may duplicate ours. It
  sits behind our `oauth/` module, so hand-rolling stays a contained
  fallback.

### Round 1

**User feedback while the draft was being written:**

- Ask only for the `atproto` identity scope. Later features add their own
  scopes through a step-up.
- Add sign-up. Settled in Q&A: it means creating an atproto account at one
  configured PDS. Per-conference onboarding (code of conduct, name,
  pronouns) is a separate, later feature.
- The backend should run on either SQLite or Postgres.

**Critique (a fresh reviewer), and what changed:**

- **Login CSRF and session fixation:** added the pre-auth cookie bound to
  the pending request, and a new session ID on every sign-in.
- **The e2e origin:** Playwright moves to `127.0.0.1`, with `PUBLIC_URL`
  and `DATABASE_URL` on the server. The ci-pipeline impact now says so.
- **The OAuth library was undecided:** chose `atproto-oauth`, after
  reading the source of all three candidates.
- **Surviving restarts:** the cookie gets a `Max-Age`, and `spawnServer`
  can restart on the same database and port.
- **`SessionExpired` contradictions:** signing out now gives
  `AuthRequired`. Expiry keeps the row (DID and handle only, tokens wiped)
  so the banner can sign the same person back in.
- **Step-up scopes:** they must be declared in the client metadata, so
  `OAUTH_EXTRA_SCOPES` was added and checked on `/oauth/login`.
- **`App.test.tsx` would break:** the provider and header moved into a
  `Shell` layout outside `App`. Startup no longer touches atproto.
- **Testability:**
  - expiry through `viv.deleteAccount` and a 2 s idle timeout
  - refresh timing through `TOKEN_REFRESH_SKEW`
  - a stated plan for observing revocation
- **Refresh failures:** only `invalid_grant` ends a session, and
  `getSession` no longer calls the authorization server.
- **SSRF:** private addresses are refused outside dev, and the spec's
  metadata checks are spelled out.
- **`return_to`:** parsed against `PUBLIC_URL` and compared by origin.
- **Step-up edge cases:** a mismatch, a missing session, races, and
  replacing the DPoP key.
- **Database defaults:** an explicit data directory, `mode=rwc`, and temp
  databases in every test server.
- **Simplifications:** dropped `OptionalUser`. The PWA's `ScopeRequired`
  handling moved to `block-actions`.

### Round 2

**User feedback:**

- Asked whether a Rust adapter covers both SQLite and Postgres. The
  options were sqlx `Any`, SeaORM/sea-query, and Diesel's
  `MultiConnection`. The user chose **sqlx `Any`** with portable SQL, as
  drafted.
- **No Postgres CI job for now.** Recorded as an accepted risk under Risks.

**What changed:** only the Risks entry on sqlx `Any`.

### Round 3

**User feedback:**

- No step-up. Scopes are added to the sign-in scope list when the features
  that need them ship. Step-up will come later, for conferences with
  write-heavy plugins.
- Renew tokens regularly on the backend, not only when the person does
  something.

**What changed:**

- Step-up, `OAUTH_EXTRA_SCOPES`, `require_scopes` and `ScopeRequired` are
  gone.
- Added `LOGIN_SCOPES` (overridable with `OAUTH_SCOPES`). A session missing
  a scope that's now in the list ends with `SessionExpired`, and the person
  signs in again.
- Added the background renewer, with a per-row refresh lease so that two
  refreshes never spend the same single-use token, even across instances.
- Updated the tests for renewal, refresh refused and scopes grown, and the
  impact on `space-sync` (organizer scopes) and `block-actions`.

### Build notes

Where the build refined the approved design:

- **The OAuth protocol is hand-rolled** in `crates/server/src/oauth.rs` and
  `keys.rs` (ES256 JWS, DPoP with nonce retry, PAR, token, refresh and
  revoke), not built on `atproto-oauth`. This is the fallback the design
  named. On reading the crate:
  - Its token and refresh calls parse the body without checking the HTTP
    status. That loses the `invalid_grant` vs. `5xx` distinction the
    renewer depends on.
  - Its `oauth_refresh` rediscovers the authorization server from the DID
    document on every refresh.
- **The client ID carries the scope list once it grows past `atproto`**
  (`…/oauth-client-metadata.json?scope=…`). Authorization servers cache
  client metadata. Vivarium caches it for 60 s and ignores cache headers,
  and production servers cache too. Without a new client ID, a server keeps
  refusing scopes that the cached metadata doesn't list. With `atproto`
  alone, the ID is the plain metadata URL.
- **Every grant is used as the client ID it was issued to.** A pending
  request (`oauth_requests.client_id`) records the client ID its PAR was
  pushed as, and the callback redeems the code as that client, whichever
  instance answers. A session (`sessions.client_id`) keeps that client ID,
  and every refresh and revocation, from any instance, is made as it. So
  instances on different scope lists, sharing one database during a
  rolling deploy, never use each other's grants as the wrong client. The
  callback checks the grant against the scopes that client ID names (the
  ones the request was pushed with), not the answering instance's list. If
  that instance asks for more, `session::outdated` sends the person back
  through sign-in on their next request to it.
- **The metadata route serves any well-formed scope list.**
  `/oauth-client-metadata.json` is `atproto` alone; `?scope=S` is served
  for any list of RFC 6749 scope tokens that includes `atproto`, has no
  duplicates and is at most 2 KB, and describes the client ID that is that
  exact URL. Only the exact spelling a client ID uses is served: the raw
  query must be `scope=` and the list percent-encoded as the client ID
  encodes it. `?scope=atproto`, malformed lists, other encodings (`+`,
  lowercase hex, unencoded `:`) and extra or repeated parameters are
  `404`, so a served document's `client_id` is always the URL it was
  fetched from. The server
  refuses to start (exit 2) if `OAUTH_SCOPES` fails that check, so its own
  list is always one the route serves. A grant from
  another instance's scope list may make the authorization server fetch
  that client's metadata again, from any instance, so an allow-list of this
  instance's own list (or one compiled into the binary) would break
  rolling deploys. Serving any list grants nothing: the client is
  confidential, and every PAR, token and revocation request as any of these
  client IDs needs an assertion signed with this client's key.
- **A session ends for a missing scope, and nothing else about its grant.**
  `session::outdated` is true only when the granted scopes lack one this
  instance asks for at sign-in. A grant with more scopes, or issued to
  another client ID, stays live and is refreshed as its own client.
- **`getSession` renews the session cookie on every answer**, not only when
  it moves `last_seen_at` (at most hourly). Other routes move
  `last_seen_at` without renewing the cookie, so renewing only on a touch
  could let a heavily used PWA's cookie run out while its row was live.
- **SQLite runs in WAL mode** (`PRAGMA journal_mode = WAL` on connect, kept
  in the file). In the default rollback mode, a reader of the file outside
  the server (the tests' `storedTokens`) failed with `database is locked`
  while the renewer was writing.
- **The schema is one migration, `0001_auth.sql`.** The branch's three
  migrations were squashed before merging, since they reach `main`
  together. A local `data/` database created from this branch before
  round 9 has the old migration checksums and has to be deleted.
- **The session cookie lives for the idle timeout plus 30 days** (the
  tombstone), not just the idle timeout. An idle session has to still send
  its cookie for the PWA to hear `SessionExpired`. An ended row is kept for
  the idle timeout plus 30 days after it ended, which is never before its
  cookie runs out.
- **Bad settings exit 2, before anything starts.** `Config::from_env`
  checks `PUBLIC_URL` (a bare `http` or `https` origin with no path,
  query, fragment or credentials, since the client ID, redirect URI,
  cookies and same-origin checks are built on it; it's normalized as the
  browser serializes an origin, so `https://App.Example:443` becomes
  `https://app.example` and a non-ASCII host becomes punycode, and the
  normalized value is the one used everywhere), `DATABASE_URL`
  (a supported scheme, and not in-memory SQLite) and `OAUTH_SIGNING_KEY`
  (a P-256 private JWK). A database that can't be reached or migrated
  still exits 1.
- **The PWA reads only the appview's own refusals as signed out.**
  `fetchSession` is signed out for a 401 `AuthRequired` and expired for a
  401 `SessionExpired`; any other status (a proxy's 429 or 408, say) is
  `unavailable`. `api()` shows the signed-out state when a request is
  refused with `AuthRequired`, as after a sign-out in another window.
- **The default database** is `sqlite://data/eventside.db?mode=rwc`,
  relative to the working directory. `data/` is git-ignored.
- **In-memory SQLite is refused.** sqlx shares an in-memory database
  (`sqlite::memory:`, `mode=memory`, `vfs=memdb`) between the pool's
  connections, but it vanishes when they all close, which the pool does to
  idle ones, silently dropping every session and the generated signing key.
  The server refuses it at startup. `file:` URIs are refused too, since
  SQLite reads their own parameters; a plain path names any real file.
- **Signing out of an ended or unknown session needs no CSRF token**, since
  it holds no tokens, and clears the cookie, but only a cookie the request
  carried. A cross-site form POST carries no cookie (`SameSite=Lax`), so it
  gets a `200` with no `Set-Cookie` and can't strand a live session the
  person could no longer reach to sign out. A live session always needs
  its CSRF token.
- **TC-24's IP rules are covered by a Rust unit test.** Vivarium serves
  plain `http://localhost`, so the frozen integration test is refused by
  the `https` check before the private-address guard runs. The guard's
  rules (loopback, private, link-local and the rest, refused unless
  `ALLOW_PRIVATE_NETWORK`) are covered by
  `net::tests::private_addresses_are_refused_unless_allowed`.
- **Only the request that ends a session revokes its grant.** `session::end`
  and the renewer's `session::end_revoking` wipe the row first and revoke
  only if they were the one to end it, so requests racing on a just-idled
  session revoke once, and a failed wipe revokes nothing.
- **The generated signing key is a single row.** `client_keys` has a
  `slot` primary key that is always `1`, next to the `kid`, private JWK and
  `created_at`. Every instance tries to insert its own new key with
  `ON CONFLICT (slot) DO NOTHING` and then reads `slot = 1`, so instances
  starting together on a fresh database all use the one key that was
  stored, and a restart never switches to another.
- **Where the design's interfaces ended up.** Later features use these
  names:
  - `CurrentUser` is `auth::CurrentUser` (`crates/server/src/auth/mod.rs`),
    with `did`, `handle` and `scopes` as designed.
  - `LOGIN_SCOPES` is `config::LOGIN_SCOPES` (`crates/server/src/config.rs`).
    `OAUTH_SCOPES` overrides it at run time.
  - `user.pds_client().await?` is `user.pds_client(&state).await?`, taking
    the `AppState`, and returns an `auth::pds::PdsClient`.
  - `ClientKeys` is `state.oauth.key`, one `keys::EcKey` (the configured
    `OAUTH_SIGNING_KEY` or the generated row), with its `kid` set.
  - `AppState::db()` is the field `state.db` (an `sqlx::AnyPool`, through
    `AppState`'s `Deref` to `Inner`).
  - The CSRF middleware is `auth::require_csrf`, layered over the router in
    `lib.rs`.
  - The design's `oauth/` and `xrpc/auth.rs` modules are the single files
    `crates/server/src/oauth.rs` and `crates/server/src/auth/routes.rs`.
  - Test helpers: `signIn` and `signUp` are in `tests/support/auth.ts`,
    `signInAs` in `e2e/support/auth.ts`.

## Test cases

Ana and Bram are attendees with vivarium accounts. Mallory is an attacker
with an account of her own. Unless a case says otherwise, the sign-in
scopes are the default (`atproto` only).

### Signing in

### TC-1: Ana signs in with her handle

- **Given** Ana has an atproto account with a profile (display name and
  avatar) and isn't signed in
- **When** she taps "Sign in", enters her handle, and picks her account on
  her PDS's consent screen
- **Then** she's back in the app, signed in
- **And** the header shows her avatar and handle

### TC-2: Signing in returns Ana to the page she started from

- **Given** Ana is on the block gallery page and isn't signed in
- **When** she signs in from there
- **Then** she lands back on the block gallery page, signed in

### TC-3: An account without a profile still signs in

- **Given** Bram has an atproto account with no profile record
- **When** he signs in
- **Then** the header shows his handle and an initial in place of an avatar

### TC-4: An unknown handle is reported, not sent anywhere

- **Given** Ana isn't signed in
- **When** she enters a handle that doesn't exist
- **Then** she stays on the sign-in page with a message that the handle
  couldn't be found
- **And** she isn't signed in

### TC-5: Declining consent leaves Ana signed out

- **Given** Ana has started signing in
- **When** her PDS sends her back with consent declined
- **Then** she sees the sign-in page with a message that sign-in was
  cancelled
- **And** she isn't signed in

### TC-6: Picking a different account at the PDS is refused

- **Given** Ana entered her own handle to sign in
- **When** she picks Bram's account on the consent screen instead
- **Then** sign-in fails with a message that the account didn't match
- **And** nobody is signed in

### TC-7: Who's signed in, as the app sees it

- **Given** Ana is signed in
- **When** the app asks who's signed in
- **Then** it gets her DID, her handle, her display name and avatar, the
  scopes she granted (just `atproto`), and a CSRF token
- **And** when nobody is signed in, or the cookie is unknown or malformed,
  the answer is "sign-in required", never "session expired"

### Signing up

### TC-8: A newcomer creates an account and comes back signed in

- **Given** Bram has no atproto account
- **When** he taps "Create account" and creates the account on the sign-up
  PDS
- **Then** he's back in the app, signed in with his new handle

### TC-9: Creating an account asks the PDS for its sign-up screen

- **Given** the appview's sign-up PDS is configured
- **When** someone taps "Create account"
- **Then** the authorization request goes to that PDS's authorization
  server, asks it to create an account, and names no existing account

### Staying signed in

### TC-10: Ana stays signed in when she closes and reopens the app

- **Given** Ana is signed in
- **When** she closes the app and opens it again later
- **Then** she's still signed in

### TC-11: Ana stays signed in across a backend restart

- **Given** Ana is signed in
- **When** the backend restarts
- **Then** she's still signed in
- **And** the backend can still renew her tokens

### TC-12: Tokens are renewed in the background

- **Given** Ana is signed in, and her access token will expire before the
  next renewal run
- **When** a renewal run happens and Ana does nothing
- **Then** her session has a new access token, and she's still signed in

### TC-13: Two renewals at once don't sign Ana out

- **Given** two backend instances share one database and Ana is signed in
- **When** both try to renew her tokens over several renewal runs
- **Then** she's still signed in, and her tokens keep being renewed

### TC-14: An unreachable authorization server doesn't sign anyone out

- **Given** Ana is signed in
- **When** her PDS's authorization server can't be reached during a
  renewal run
- **Then** she's still signed in

### Signing out and expiry

### TC-15: Ana signs out

- **Given** Ana is signed in
- **When** she signs out from the account menu
- **Then** the header shows "Sign in", with no "session expired" banner
- **And** her old cookie no longer signs anyone in
- **And** her tokens are revoked at her PDS's authorization server

### TC-16: An idle session expires, and Ana gets back to her page

- **Given** Ana signed in and then went idle past the idle timeout
- **When** she opens the block gallery page
- **Then** she sees a banner that her session expired
- **And** when she taps its sign-in button, she signs in as herself
  without retyping her handle, and lands back on the block gallery page

### TC-17: A revoked account is noticed without Ana doing anything

- **Given** Ana is signed in
- **When** her account is deleted at her PDS, and a renewal run happens
- **Then** her session has ended before she does anything
- **And** the next time the app asks, it hears "session expired"

### TC-18: Growing the sign-in scopes asks people to sign in again

- **Given** Ana signed in when the sign-in scopes were `atproto` only
- **When** the backend restarts asking for `atproto transition:generic`
- **Then** the app hears "session expired" for Ana's session
- **And** when she signs in again, her session has both scopes

### Security

### TC-19: A sign-in started in someone else's browser can't be finished in Ana's

- **Given** Mallory starts signing in with her own account and approves it
- **When** Ana's browser opens Mallory's callback link
- **Then** sign-in fails with an error
- **And** Ana isn't signed in as Mallory

### TC-20: Signing in replaces whatever session the browser had

- **Given** Ana's browser carries a session cookie from before
- **When** she signs in
- **Then** her new session cookie is different from the old one
- **And** the old cookie no longer signs anyone in

### TC-21: A callback link only works once

- **Given** Ana has just signed in through a callback
- **When** the same callback link is opened again
- **Then** it fails with an error and creates no session

### TC-22: Sign-in never redirects off-site

- **Given** a sign-in link whose return address is another site
  (`https://evil.example`, `//evil.example`, `/\evil.example`, or an
  encoded form of one of them)
- **When** Ana signs in with it
- **Then** she lands on the app's home page

### TC-23: Changes without the CSRF token are refused

- **Given** Ana is signed in
- **When** a request to sign her out arrives with her cookie but without
  her CSRF token, or with the wrong one
- **Then** it's refused
- **And** she's still signed in

### TC-24: Private addresses are refused outside dev

- **Given** the backend runs with the private-network guard on
- **When** Ana tries to sign in with an account whose PDS is on a loopback
  address
- **Then** sign-in fails with an error, and nothing is fetched from that
  address

### The appview as a client

### TC-25: The appview publishes its client metadata and keys

- **Given** the backend is running
- **When** anyone fetches its client metadata and its key set
- **Then** the metadata names the client ID, the callback address, the
  sign-in scopes and `private_key_jwt` authentication
- **And** the key set holds only public keys

### TC-26: The signing key stays the same across restarts

- **Given** the backend generated its own signing key
- **When** it restarts on the same database
- **Then** it publishes the same key

### TC-27: The backend picks its database from its URL

- **Given** a database URL
- **When** the backend starts
- **Then** a `sqlite:` URL runs on SQLite and a `postgres:` URL selects
  Postgres
- **And** any other scheme stops startup with a message naming the
  supported ones

### Regressions

### TC-28: The backend starts without atproto, on a fresh database

- **Given** a fresh database and an unreachable atproto service
- **When** the backend starts
- **Then** it serves its health check, reporting atproto as unreachable

### TC-29: The home page and block gallery work signed out

- **Given** nobody is signed in
- **When** someone opens the home page and the block gallery
- **Then** the home page shows the backend and atproto status
- **And** the gallery renders its cards
- **And** both pages show the header with "Sign in"

### TC-30: The installed app doesn't swallow sign-in

- **Given** the PWA is installed and its service worker is active
- **When** Ana starts signing in
- **Then** the sign-in request reaches the backend, not the cached app
  shell, and the flow completes

## Review log

### Round 1

The reviewer found 0 blocking, 0 major, 7 minor and 2 nits. `pnpm check`
was green and the frozen files unchanged. Every finding was fixed.

1. **[minor] An expired session could never be signed out, so its cookie
   stuck for 30 days.**
   - Fixed: the CSRF layer now lets `/oauth/logout` through for an ended
     session, which holds no tokens.
   - The expired banner has a "Not you? Sign out" button.
2. **[minor] Sessions ended by idle timeout or grown scopes weren't
   revoked.**
   - Fixed: `session::end` now revokes the grant in the background before
     wiping it.
   - `session::wipe` (no revoke) is kept only for grants the server has
     already refused.
3. **[minor] The renewer refreshed sessions granted under an older scope
   list, using the new client ID.**
   - Fixed: a refresh first checks the scopes, and ends and revokes the
     session if any are missing. That covers the renewer and on-demand
     refreshes alike.
4. **[minor] `pds_client()` spun for 10 s on a session with no refresh
   token.**
   - Fixed: once the access token is stale and there's no refresh token,
     the session ends and the client returns `SessionExpired`.
5. **[minor] A `handle.invalid` session couldn't be signed back in.**
   - Fixed: `SessionExpired` now carries the DID too, and the PWA uses it
     when the handle didn't verify.
   - `/oauth/login` accepts a DID, skipping handle resolution.
6. **[minor] A refresh could outlast its 30 s lease.**
   - Fixed: the lease is now 90 s, and each refresh is cut off at 60 s.
7. **[minor] `discover` didn't check the protected-resource `resource`
   field.**
   - Fixed: it must equal the PDS URL.
8. **[nit] `iss` was checked after `error` on the callback.**
   - Fixed: it's checked first (RFC 9207). Vivarium sends `iss` on denials,
     and TC-5 still passes.
9. **[nit] Sign-out failures were silent, and a network error went
   unhandled.**
   - Fixed: `signOut` catches errors and reports whether it worked, and the
     account menu shows a message when it didn't.

### Round 2

The reviewer found 0 blocking, 1 major, 5 minor and 3 nits. `pnpm check`
was green, and every round 1 fix was confirmed. All findings were fixed;
one suggestion within finding 1 (per-IP rate limiting) was declined.

1. **[major] Unbounded response bodies from attacker-chosen hosts.**
   - Fixed: `net::read_capped` and `net::read_json` cap every body read
     from another service at 64 KiB. They check `Content-Length` first,
     then stop reading in chunks once past the cap. They cover DID
     documents, `resolveHandle`, PDS and authorization-server metadata,
     profiles, and PAR, token and revocation responses.
   - Declined: per-IP rate limiting on `/oauth/login` and `/oauth/signup`.
     The appview binds 127.0.0.1 and runs behind whatever serves its
     public origin, so the peer address is the proxy's. A per-IP limit
     belongs at that edge, which the production topology (an open risk in
     the design) will decide. Pending requests are swept on every renewal
     run.
2. **[minor] A sign-out during a refresh left the new grant unrevoked.**
   - Fixed: when the save after a refresh matches no row, the new grant is
     revoked. That covers a session signed out, ended or replaced
     mid-refresh. A failed save revokes it too.
3. **[minor] No backoff for unreachable authorization servers.**
   - Fixed: an issuer whose metadata fetch fails, or whose refresh is
     `Unavailable`, is left alone for 30 s. `cached_auth_server` fails fast
     until then, so one run costs at most one timeout per issuer, not one
     per session.
4. **[minor] DIDs typed at `/oauth/login` went into the PLC URL
   unvalidated.**
   - Fixed: `identity::is_valid_did` requires `did:plc:` with 24 base32
     characters, or a host-level `did:web:`.
   - It's checked at login (a bad DID is `handle_not_found`), in
     `resolve_did`, and on `resolveHandle` results and callback `sub`
     values.
5. **[minor] Most error codes had no message.**
   - Fixed: the sign-in page now has a message for each of the 11 codes.
     The generic fallback covers only codes it doesn't know.
6. **[minor] A refresh's `sub` wasn't checked.**
   - Fixed: a refresh that comes back for a different DID revokes the new
     grant, wipes the session, and ends it.
7. **[nit] `displayName` wasn't limited.**
   - Fixed: it's truncated to 64 characters and 640 bytes on a character
     boundary, inside the lexicon's limits.
8. **[nit] `touch`'s comment promised cookie renewal that only `getSession`
   does.**
   - Fixed: the comment now says so. The PWA calls `getSession` on every
     load.
9. **[nit] `PdsClient` didn't share DPoP nonces.**
   - Fixed: it reads and updates the OAuth client's per-origin nonce cache.

### Round 3

The reviewer found 0 blocking, 0 major, 3 minor and 4 nits. `pnpm check`
was green. The reviewer confirmed the earlier fixes, and two suspected
races turned out not to happen. Every finding was fixed.

1. **[minor] The issuer backoff also blocked revocation, and every other
   session on the same issuer.**
   - Fixed: `cached_auth_server` no longer checks the backoff. Only the
     renewer does: it skips sessions on a backing-off issuer.
   - Revocation, the callback and `pds_client()` always try.
   - Failed metadata fetches and `Unavailable` refreshes still mark the
     issuer down.
2. **[minor] Temporary resolver failures were treated as final.**
   - Fixed: only a 400 or 404 from `resolveHandle` means "no such handle",
     so a 429 or 5xx is `Unresolvable`.
   - `resolve_did` uses `handle.invalid` only when the handle definitely
     doesn't point back. A resolver that can't answer fails the
     resolution, so the callback reports `resolution_failed` and stores
     nothing.
3. **[minor] The PWA showed the person as signed out whenever `getSession`
   failed.**
   - Fixed: network errors and 5xx answers read as a new `unavailable`
     state. The header shows neither control, and an already-known state is
     kept.
   - The provider asks again on `online` and when the app becomes visible.
   - The "never throws" comment now matches what the code does.
4. **[nit] Token responses weren't checked for `token_type` or `sub`.**
   - Fixed: every token response must be `DPoP` and carry `sub`. A refresh
     must come back for the session's own DID, and a missing `sub` counts
     as a mismatch.
5. **[nit] Issuers were normalized inconsistently.**
   - Fixed: an issuer is compared exactly everywhere: metadata `issuer`,
     the stored issuer, and the callback's `iss`. A trailing slash is
     refused rather than trimmed, since an atproto issuer is a bare origin.
6. **[nit] State-changing routes looked the session up twice.**
   - Fixed: the CSRF layer passes the session it checked to `CurrentUser`
     through request extensions.
7. **[nit] `serde_json` was listed under both dependencies and
   dev-dependencies.**
   - Fixed: the dev-dependency entry is gone.

### Round 4

The reviewer found 0 blocking, 0 major, 3 minor and 2 nits. `pnpm check`
was green. The reviewer confirmed the earlier fixes, and a scratch test
showed that grants ended by grown scopes are revoked. Every finding was
fixed.

1. **[minor] Sign-out waited on the authorization server before deleting
   the session.**
   - Fixed: `logout` deletes the row (and so clears the cookie) first.
   - It then revokes in a spawned task, and waits at most 3 s so the grant
     is usually gone by the time sign-out returns.
   - Dropping the request can no longer leave the person signed in.
2. **[minor] `pds_client()` re-refreshed whenever the server's tokens
   didn't outlive the skew.**
   - Fixed: the skew now only decides when to start a refresh. After a
     refresh, its own or another instance's, the client uses the token as
     long as it hasn't expired.
3. **[minor] One shared pre-auth cookie broke concurrent sign-ins.**
   - Fixed: each sign-in's cookie is named after the start of its `state`
     (`oauth_preauth_<id>`).
   - A callback reads and clears only its own, so two tabs, or a stray
     callback link, can't disturb another sign-in.
4. **[nit] The renewer loaded every due session at once.**
   - Fixed: it pages through due sessions 500 at a time (keyset on
     `id_hash`), finishing each batch before loading the next.
5. **[nit] `touch` ran more often than the design's "at most once an
   hour".**
   - Fixed: it's capped at once an hour. Only idle timeouts shorter than
     10 hours (as in tests) touch more often, every tenth of the timeout.

### Round 5

The reviewer found 0 blocking, 0 major, 3 minor and 2 nits. `pnpm check`
was green. Every finding was fixed after the review, and `pnpm check` is
green again. No reviewer has seen these fixes yet.

1. **[minor] The guarded client honored `HTTP(S)_PROXY`, so a proxy would
   bypass the private-network guard.**
   - Fixed: both clients use `.no_proxy()`.
   - An egress proxy, if one is ever needed, has to be configured
     explicitly and enforce the address policy itself.
2. **[minor] The DPoP nonce cache could grow without bound, keyed by
   attacker-chosen origins.**
   - Fixed: nonces over 512 bytes are ignored, and the cache starts over
     when it reaches 1024 origins. Nonces are only an optimization.
3. **[minor] A failed session insert left the person signed out, with the
   new grant unrevoked.**
   - Fixed: the callback now creates the new session before retiring the
     browser's old one, and revokes the new grant if the insert fails.
4. **[nit] IPv6 forms that embed an IPv4 address passed as public, and the
   DNS filter had no test.**
   - Fixed: NAT64 (`64:ff9b::/96`) and 6to4 (`2002::/16`) are judged by
     their embedded IPv4 address, and IPv4-compatible `::a.b.c.d` is
     refused.
   - The filter is now a function with its own unit test.
5. **[nit] `pds_endpoint` accepted any `…#atproto_pds` service.**
   - Fixed: it accepts only `#atproto_pds` or `<did>#atproto_pds`, with
     `type: AtprotoPersonalDataServer`.

### Blocked after round 5

The pipeline allows five review rounds. None came back clean, so this needs
a human decision before shipping.

**What the rounds found:**
- No round found anything blocking, and only round 2 found a major issue
  (unbounded response bodies, now fixed).
- Every finding was fixed except one suggestion, declined in round 2 with a
  reason that hasn't been disputed.
- The findings don't recur. Each round went one layer deeper into new edge
  cases, mostly around the network boundary (SSRF, resource limits) and
  races between renewal, sign-out and re-sign-in. The earlier fixes held
  in every later round.
- Two findings were follow-ups on earlier fixes: round 3 found the round 2
  backoff blocked revocation, and round 4 found the round 1 lease could be
  outlived by the PDS client's own refresh loop.

**Current state:** all 30 test cases pass, the frozen tests are unchanged,
and `pnpm check` is green with the round 5 fixes.

**The choices:**
- Accept the branch as it stands and ship it.
- Run a sixth round to review the round 5 fixes.

**Decision (2026-10-02):** the user raised the review limit to 10 rounds
(PR #5), so the review loop resumes at round 6.

### Round 6

The reviewer found 0 blocking, 0 major, 4 minor and 2 nits. `pnpm check`
was green. Every finding was fixed after the review, and `pnpm check` is
green again. No reviewer has seen these fixes yet.

1. **[minor] `TOKEN_RENEW_INTERVAL=0` panicked the renewal task, and huge
   durations could overflow.**
   - Fixed: a zero `TOKEN_RENEW_INTERVAL` or `SESSION_IDLE_TIMEOUT` is a
     startup error (exit 2).
   - `parse_duration` uses `checked_mul` and refuses anything over ten
     years, so the idle timeout plus the tombstone period can't overflow.
2. **[minor] The expired banner on `/signin` sent people back to the
   sign-in page.**
   - Fixed: the banner uses `useReturnTo()`, like the account control, so
     on `/signin` it keeps the page's own `return_to`.
3. **[minor] Sign-in through `vite dev` failed, because `PUBLIC_URL`
   defaulted to the backend's own address.**
   - Fixed: `pnpm dev:server` starts the backend with
     `PUBLIC_URL=http://127.0.0.1:5173`, and `pnpm dev:web` starts Vite.
   - Vite's dev server is pinned to `127.0.0.1:5173` (`strictPort`), with
     a comment saying why.
4. **[minor] The CSRF layer covered only routes registered before it.**
   - Fixed: `routes()` holds every route, and `router()` wraps the finished
     set in the CSRF check, so a feature adding a route there gets it.
   - A Rust test adds a dummy non-GET `/xrpc/` route and checks it
     returns 401 without a session, 403 without the right token, and 200
     with it.
5. **[nit] Sign-out looked the session up again.**
   - Fixed: it uses the session the CSRF layer checked, and looks up only
     for an ended session, which the layer lets through.
6. **[nit] The renewal query scanned every live session.**
   - Fixed: idle sessions and due tokens are now two queries, each paged
     by its own column on its own index, `(ended_at, last_seen_at, id_hash)`
     and `(ended_at, token_expires_at, id_hash)`.
   - SQLite plans both as index range searches with no temp B-tree.
   - The indexes are a new migration, `0002_renewal_indexes.sql`, rather
     than an edit to `0001_auth.sql`: a database that already ran 0001 (a
     local run of this branch) would refuse its changed checksum.

### Round 7

The reviewer found 0 blocking, 0 major, 3 minor and 4 nits. `pnpm check`
was green. Every finding was fixed after the review, and `pnpm check` is
green again. No reviewer has seen these fixes yet.

1. **[minor] One renewal run could refresh the same session twice.**
   - Fixed: a refresh moves the row's `token_expires_at`, possibly to a key
     still ahead of the cursor and due. The run keeps the `id_hash` of every
     session it has handled and skips one it meets again; the cursor still
     moves past it, so the run ends.
2. **[minor] Revocations from the renewer escaped its concurrency limit.**
   - Fixed: the renewer (idle expiry, and a grant missing a scope) uses a
     new `session::end_revoking`, which wipes the session and then awaits
     the revocation, for at most 15 seconds, while holding its permit.
   - Request-path callers keep `session::end`'s background revocation.
3. **[minor] A stale CSRF token, after a sign-out and sign-in in another
   window, failed every write until a reload.**
   - Fixed: on `403 InvalidCsrfToken`, `api()` fetches the session again
     and hands it to the `SessionProvider`, so the shell shows who is
     signed in now. It retries once with the new token if it's the same
     account, and doesn't for a different one.
   - Sign-out still refused after that means another account signed in
     elsewhere and the shell now shows it, so it's not reported as a
     connection problem.
   - Two unit tests cover the retry and the different-account case.
4. **[nit] `expires_in` from the authorization server was unbounded.**
   - Fixed: `TokenSet::expires_at` clamps it to 30 seconds–1 day (5
     minutes when missing) with saturating addition. Sign-in and renewal
     both use it, and a unit test covers the bounds.
5. **[nit] `currentPath()` was dead code.**
   - Fixed: deleted.
6. **[nit] The Rust CSRF test read the environment and left its temp
   directory behind.**
   - Fixed: it builds `Config` explicitly, and a guard removes the
     directory when the test ends, pass or fail.
7. **[nit] `oauth_requests.kind` was written but never read.**
   - Fixed: a comment where it's written says it's diagnostic only; the
     callback treats both flows alike.

### Round 8

The reviewer found 0 blocking, 0 major, 4 minor and 3 nits. `pnpm check`
was green. Every finding was fixed after the review, and `pnpm check` is
green again. No reviewer has seen these fixes yet.

1. **[minor] Sign-out reported success on any 403, even when the appview
   couldn't be asked.**
   - Fixed: on a 403 that `api()` couldn't resolve, `signOut` asks for the
     session itself. It succeeds only if that shows the session over
     (signed out, expired, or another account), and fails if the appview
     can't be asked or the same account is still signed in.
   - Three unit tests in `SessionProvider.test.tsx` cover the cases.
2. **[minor] Refresh and revocation used the current client ID, not the
   one the grant was issued to.**
   - Fixed: a session stores its client ID (new migration
     `0003_session_client_id.sql`; NULL means the current one).
     `session::outdated` ends a session issued to another client ID, as it
     does one whose scopes grew, so a shrunk or reordered scope list no
     longer leads to a refresh as the wrong client. Revocation is skipped
     for such a grant, since the server would refuse it. Recorded in the
     build notes.
   - A Rust test covers the check.
3. **[minor] Ended rows were swept 30 days after ending, while their
   cookie could live the idle timeout plus 30 days.**
   - Fixed: the sweeper keeps both ended and idle rows until the idle
     timeout plus the tombstone has passed since `ended_at` or
     `last_seen_at`, never before the cookie runs out. A Rust test covers
     a row ended 40 days ago (kept) and 61 days ago (gone).
4. **[minor] Ending idle sessions ignored the issuer backoff, and a
   hanging revocation endpoint could stall renewal.**
   - Fixed: `end_revoking` wipes without revoking when the issuer is
     backing off, and marks the issuer down when a revocation times out.
     `session::revoke` marks it down when the revocation POST is
     unavailable. A hanging issuer now costs one timeout per backoff
     period, not one per session.
5. **[nit] The CSRF retry would re-send a spent stream body.**
   - Fixed: `api()` doesn't retry when the body is a `ReadableStream`.
6. **[nit] The run's `handled` set grew with every session it touched.**
   - Fixed in part: only the refresh job remembers sessions, since an
     ended session can't come back into the idle query. The refresh set
     stays: it lasts one run, at roughly 100 bytes per due session, which
     at conference scale is well under a megabyte.
7. **[nit] An issuer with a path was accepted.**
   - Fixed: `auth_server` refuses an issuer that isn't a bare origin
     (its URL's origin must serialize to the issuer exactly), with a unit
     test.

### Round 9

The reviewer found 0 blocking, 1 major, 1 minor and 3 nits. `pnpm check`
was green. Every finding was fixed after the review, and `pnpm check` is
green again. No reviewer has seen these fixes yet.

1. **[major] Round 8's client ID check ended healthy sessions across
   instances, and a callback could redeem a code as the wrong client.**
   - Fixed: `oauth_requests` records the client ID each PAR was pushed as,
     and the callback exchanges the code (and revokes an unused grant) as
     it. Sessions store it too, and `exchange_code`, `refresh` and
     `revoke` take the client ID, so every call for a row is made as that
     row's client from any instance. The skipped revocation for "another
     client" is gone.
   - `session::outdated` checks only for a missing scope again, so a
     superset grant or another client ID no longer ends a session.
   - `/oauth-client-metadata.json?scope=S` serves metadata for any
     well-formed scope list containing `atproto`, at its exact client ID,
     and `404`s anything else (this instance's own list is always served).
     Why not an allow-list is in the build notes.
   - Rust tests cover the metadata rule, `outdated`, and a session issued
     to another client ID being refreshed and revoked as that client,
     against a stand-in authorization server that records the client ID.
2. **[minor] The cookie was renewed only when `getSession` itself moved
   `last_seen_at`.**
   - Fixed: `getSession` renews the cookie on every successful answer, and
     `touch` no longer reports whether it wrote. A Rust test checks two
     answers in a row both set the cookie.
3. **[nit] The expired banner's "Not you? Sign out" ignored a failure.**
   - Fixed: a `useSignOut` hook and `SignOutFailed` alert, shared with the
     account menu. A unit test in `Shell.test.tsx` covers the banner.
4. **[nit] A refused sign-out fetched the session twice.**
   - Fixed: `api()` keeps the session it fetched while handling an
     `InvalidCsrfToken` refusal, by response, behind `sessionLearned(res)`.
     `signOut` uses it, and only asks itself when `api()` didn't. A unit
     test counts one fetch.
5. **[nit] The first schema shipped as three migrations, with NULL
   handling no deployment would need.**
   - Fixed: squashed into `0001_auth.sql`, with `client_id NOT NULL` on
     `sessions` and `oauth_requests`, and the NULL handling removed. The
     worktree's local `data/` database was deleted; see the build notes.
   - Also while running the gate: TC-11 failed every time, on this round's
     code and on round 8's, with `database is locked` when the test read
     the SQLite file during a renewal. The server now puts SQLite in WAL
     mode, so readers don't wait on writers (build notes).

### Round 10

The reviewer found 0 blocking, 0 major, 3 minor and 2 nits. `pnpm check`
was green. Every finding was fixed after the review, and `pnpm check` is
green again. No reviewer has seen these fixes yet.

1. **[minor] `OAUTH_SCOPES` was never validated, and the metadata route
   served this instance's own list however it was configured.**
   - Fixed: the server refuses to start (exit 2) unless the list is
     distinct, valid scope names including `atproto` (the same
     `well_formed_scope` rule the metadata route uses). The route's "own
     list is always served" exception and its unit test are gone. A unit
     test in `config.rs` covers the check.
2. **[minor] `DATABASE_URL=sqlite::memory:` started, but each pooled
   connection was its own empty database.**
   - Fixed: in-memory and temporary SQLite URLs (`:memory:`, an empty
     path, `file:` URIs, `mode=memory`) are refused at startup, replacing
     the `:memory:` special case. A unit test in `db.rs` covers them.
3. **[minor] Nothing tested that a code is redeemed as the pending
   request's client ID.**
   - Fixed: a Rust test stores a pending request pushed as another scope
     list's client ID and runs the callback against the recording
     authorization server. It checks the token request and the unused
     grant's revocation were both made as that client ID. It fails if
     `complete` uses the instance's own client ID.
4. **[nit] Signing out with a cookie that names no session got
   `401 AuthRequired`, so the dead cookie wasn't cleared.**
   - Fixed: the CSRF layer lets `/oauth/logout` through for an unknown
     session as well as an ended one. Nothing is revoked or deleted, and
     the handler clears the cookie. A live session still needs its CSRF
     token. A Rust test covers both.
5. **[nit] `session::end` spawned a revocation even when another request
   had already ended the session.**
   - Fixed: `wipe` reports whether it ended the session, and `end` spawns
     the revocation only then. A Rust test ends the same session twice and
     sees one revocation.

**Decision (2026-10-05):** the user approved an 11th review round to check the round 10 fixes.

### Round 11

The reviewer found 0 blocking, 0 major, 2 minor and 2 nits, so the round
isn't clean. `pnpm check` was green. Every finding was fixed after the
review, and `pnpm check` is green again. No reviewer has seen the round 11
fixes yet.

1. **[minor] `session::end_revoking` revoked a grant even when it didn't
   end the session.** It ignored `wipe`'s result, so it revoked after
   another request had already ended the session, and also when `wipe`
   failed, leaving a live row on a revoked grant. Round 10 #5 fixed only
   `end`.
   - Fixed: `end_revoking` returns `wipe`'s error, and returns without
     revoking when `wipe` reports the session was already ended. A Rust
     test runs `end_revoking`, `end` and `end_revoking` on one session and
     sees one revocation.
2. **[minor] The stored handle was the DID document's raw `alsoKnownAs`
   text, lowercased, not the normalized handle that was checked.** With
   `at://@ana.test`, `resolve_handle` checked `ana.test` but `@ana.test`
   went into the session, the header, `SessionExpired` and the banner's
   `login_hint`.
   - Fixed: `claimed_handle` returns the entry run through
     `normalize_handle`, so the stored handle is the one checked. An entry
     that can't be a handle gives `handle.invalid`. A unit test in
     `identity.rs` covers it.
3. **[nit] The reason given for refusing in-memory SQLite was wrong, and
   `vfs=memdb` got through.** sqlx 0.8.6 shares an in-memory database
   between the pool's connections. The real hazard is that it vanishes
   when they all close, taking every session and the signing key with it.
   - Fixed: the comment, the startup error and the build note give the
     real reason. `vfs=memdb` is refused, and query parameters are
     percent-decoded before they're checked, as sqlx does. `file:` URIs
     are still refused: SQLite reads their own parameters (`file::memory:`
     and the like), and a plain path names any real file. The `db.rs`
     unit test covers the new cases.
4. **[nit] The scope list was validated only in `Config::from_env`, and a
   list over 2048 bytes failed with a message that didn't mention
   length.** `OAuthClient::metadata` assumed startup had checked the list,
   but a `Config` built directly (as tests do) could give a client whose
   own client ID `metadata` refused.
   - Fixed: `OAuthClient::new` returns an error for a list `metadata`
     wouldn't serve, and `AppState::build` passes it on. A new
     `scope_problem` names the rule that failed (length, token syntax,
     missing `atproto`, a duplicate), and both errors use it. Unit tests
     in `oauth.rs` and `config.rs` cover it.

**Decision (2026-10-05):** the user allowed up to 15 review rounds for this build.

### Round 12

The reviewer found 0 blocking, 0 major, 3 minor and 3 nits, so the round
isn't clean. `pnpm check` was green. Every finding was fixed after the
review, and `pnpm check` is green again. No reviewer has seen the round 12
fixes yet.

1. **[minor] The callback checked the grant against the answering
   instance's scope list, not the one the request was pushed with.**
   During a rolling deploy, a sign-in pushed by an `atproto` instance and
   answered by one asking for more was revoked and sent to
   `scope_missing`, though the person granted everything asked.
   - Fixed: `oauth::client_scopes` recovers the scope list from the
     pending request's client ID, and the callback checks the grant
     against it. If the answering instance asks for more,
     `session::outdated` sends the person back through sign-in on their
     next request. No schema change was needed. A Rust unit test covers
     `client_scopes`, and an integration test pushes on one instance,
     answers the callback on another with a grown list, and sees no error
     and then `SessionExpired`.
2. **[minor] `api()` ignored a 401 `AuthRequired`.** After a sign-out in
   another window, a refused request left the shell showing the person
   signed in.
   - Fixed: `api()` reports `AuthRequired` to the `SessionProvider` as
     signed out. A unit test sits next to the CSRF retry tests.
3. **[minor] `PUBLIC_URL` wasn't checked.** A value with a path published
   a client ID and redirect URI the routes don't serve, and a scheme-less
   one started with non-`Secure` cookies and an unusable client ID.
   - Fixed: `Config::from_env` requires a bare `http` or `https` origin
     (the rule `bare_origin` uses) and exits 2 otherwise. A trailing slash
     is still trimmed. Unit and integration tests cover it.
4. **[nit] The metadata route served documents whose `client_id` wasn't
   the fetched URL** (`+` for spaces, extra or repeated parameters).
   - Fixed: the route reads the raw query and serves only the exact
     spelling `client_id` produces. The bare URL and the canonical
     `?scope=…` URL still answer 200. Unit and integration tests cover
     both sides.
5. **[nit] Config errors found in `AppState::build` exited 1, others 2.**
   - Fixed: `DATABASE_URL` (scheme, in-memory SQLite) and
     `OAUTH_SIGNING_KEY` are checked in `Config::from_env`, so they exit 2.
     `db::connect` still runs the same check for a `Config` built
     directly. An integration test checks the exit codes.
6. **[nit] `fetchSession` read any 4xx other than `SessionExpired` as
   signed out**, so a proxy's 429 or 408 showed "Sign in" to a signed-in
   person.
   - Fixed: only a 401 `AuthRequired` is signed out. Every other status is
     `unavailable`. A unit test covers both.

### Round 13

The reviewer found 0 blocking, 0 major, 1 minor and 3 nits, so the round
isn't clean. `pnpm check` was green. Every finding was fixed after the
review, and `pnpm check` is green again. No reviewer has seen the round 13
fixes yet.

1. **[minor] A cross-site form POST to `/oauth/logout` cleared the
   person's session cookie while the session stayed live.** The POST
   carries no `SameSite=Lax` cookie, so the CSRF layer saw no session and
   let it through (round 10 #4), and the handler always cleared the
   cookie. The grant stayed live and kept being refreshed, out of the
   person's reach.
   - Fixed: `logout` sends the clearing `Set-Cookie` only when the
     request carried a session cookie. With none, it answers `200` with no
     `Set-Cookie`. Live sessions still need the CSRF token, and an ended or
     unknown cookie that was sent is still cleared. The Rust test
     `signing_out_a_session_that_is_gone_clears_its_cookie` now also posts
     with no cookie and no token and checks there's no `Set-Cookie`.
2. **[nit] A `PUBLIC_URL` with an uppercase or non-ASCII host was refused**
   with a message about path, query or port.
   - Fixed: `public_url` accepts any `http` or `https` URL with no path,
     query, fragment or credentials and returns its origin as the browser
     serializes it (lowercase, punycode, no default port). `Config` holds
     the normalized value, so the client ID, redirect URI and cookies all
     use it. The config unit test covers the normalization and the new
     refusals.
3. **[nit] TC-24 passes without reaching the private-address guard**,
   since vivarium is plain `http://localhost` and the `https` check
   refuses first. The test is frozen.
   - Fixed: the build notes record that the Rust unit test
     `private_addresses_are_refused_unless_allowed` covers the IP rules.
4. **[nit] The callback fell back to this instance's scope list when the
   client ID had none.** The fallback couldn't run, and if it had, it would
   have brought back the round 11/12 bug.
   - Fixed: a client ID with no recoverable scope list revokes the grant,
     logs, and fails the sign-in with `server_error`.

### Round 14

The reviewer found 0 blocking, 0 major, 3 minor and 1 nit, so the round
isn't clean. `pnpm check` was green. Every finding was fixed after the
review, and `pnpm check` is green again. No reviewer has seen the round 14
fixes yet.

1. **[minor] `/signin?error=__proto__` unmounted the whole app.** The
   message lookup indexed a plain object with the URL's `error`, so
   inherited keys matched: `__proto__` rendered `Object.prototype` as a
   React child and threw, and `constructor` or `toString` showed an empty
   alert.
   - Fixed: the lookup counts only the object's own keys
     (`Object.hasOwn`) and falls back otherwise. The new unit test
     `web/src/routes/signin.test.tsx` checks a known code and four
     inherited keys. No other lookup in `web/src` indexes an object
     literal with a URL- or server-provided string unguarded: the block
     registry, fixture markers and bindings already use `Object.hasOwn`.
2. **[minor] Instances generating the signing key at once could each use
   a different one.** The insert's `ON CONFLICT (kid)` used a random
   `kid`, so it never conflicted, and an instance could read back its own
   key before an older one landed.
   - Fixed: `client_keys` holds one row, keyed by a `slot` that is always
     `1`. Every instance inserts with `ON CONFLICT (slot) DO NOTHING` and
     reads that row back. The `kid` is still stored in the row. The Rust
     test `instances_generating_the_signing_key_at_once_agree_on_it` races
     four calls through two pools on a fresh database and checks they agree
     and one row exists. It failed against the old code.
3. **[minor] The design's interfaces for later features don't exist under
   those names**, and the build notes didn't say so.
   - Fixed: a build-notes entry maps each one (`CurrentUser`,
     `LOGIN_SCOPES`, `pds_client`, `ClientKeys`, `AppState::db()`, the CSRF
     middleware, the module paths and the test helpers) to the real one.
4. **[nit] A session check in flight during sign-out could sign the
   header back in** with a dead CSRF token when it answered late.
   - Fixed: `SessionProvider` keeps a generation count, bumped whenever
     sign-out, a change or an expiry sets the session, and drops a
     `getSession` answer that started before the latest bump. A new unit
     test in `SessionProvider.test.tsx` covers it.
