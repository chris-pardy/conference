---
status: design-review
impact: cross-cutting
depends-on: [ci-pipeline]
branch:
tests-commit:
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

- Session storage in the appview (SQLite, or something else) is the first
  database in the project. Architecture analysis should decide this for
  everyone.
- How the test helper signs in. Vivarium's consent page turned out to be a
  plain HTML form (pick an account, or type a handle and it's created on
  the fly), with no JavaScript, so the helper can run the real flow over
  HTTP.
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
- Per-conference onboarding (not specced yet): it runs after sign-in or
  sign-up and keys everything on the signed-in DID this feature provides.
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

### Approach

The appview is a **confidential atproto OAuth client**. Its client ID is
`<PUBLIC_URL>/oauth-client-metadata.json`, and it authenticates to
authorization servers with `private_key_jwt`, signed by an ES256 key whose
public half it serves at `<PUBLIC_URL>/oauth/jwks.json`. `PUBLIC_URL` is the
origin the browser uses: the PWA and the appview share it, so the session
cookie is first-party and the callback lands on the same origin. In dev and
tests the Vite server proxies the OAuth paths to the appview, as it already
does for `/xrpc/`. The metadata's `scope` is `atproto` plus
`OAUTH_EXTRA_SCOPES`, the scopes later features may step up to. It's empty
today, and tests set `transition:generic`.

**Signing in** is a top-level navigation, not a fetch:

1. The PWA sends the browser to `GET /oauth/login?handle=…&return_to=…`.
2. The appview resolves the handle to a DID, the DID to its PDS, and the PDS
   to its authorization server (protected-resource metadata, then
   authorization-server metadata). The spec's checks apply: the metadata's
   `issuer` is the URL it was fetched from, and the PDS lists that server.
3. It makes a PKCE verifier, a `state`, a fresh DPoP key and a random
   **pre-auth cookie** (`HttpOnly`, ten minutes, `SameSite=Lax`). It sends
   a pushed authorization request (PAR) with `scope=atproto` and the handle
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
     `atproto`
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
| The authorization server answers a refresh with `invalid_grant` | The session is marked ended | `401 SessionExpired` |

An ended session isn't deleted straight away. Its tokens and keys are
wiped, but the row keeps the DID and handle until the cookie's own
lifetime would have run out. That's what lets the "expired" banner sign
the same person back in (`login_hint`) and return them to the page they
were on. A sweeper deletes these rows and stale pending requests later.

Network errors and `5xx` from the authorization server never end a
session. They surface as `503` to the feature that asked.

**Tokens** are refreshed lazily, only when a feature asks for an
authenticated PDS client and the access token is within
`TOKEN_REFRESH_SKEW` (default 60 s) of expiring. `getSession` reads the row
and never calls the authorization server. That keeps the AS out of every
app open, so an AS-side revocation is only noticed at the next refresh.
Refresh tokens are single-use, so refreshes are serialized by a
compare-and-swap (`UPDATE … WHERE refresh_token = <old>`). That works the
same on SQLite and on several Postgres-backed instances: the loser re-reads
the row and uses the winner's tokens. DPoP nonces are cached per server in
memory.

**Step-up.** `GET /oauth/login?scope=<extra scopes>&return_to=…` while signed
in runs the same flow with `login_hint` set to the session's DID and
`scope=atproto <extra>`. The extra scopes must be in
`OAUTH_EXTRA_SCOPES`, so a cross-site link can't start consent for
anything else. The pending request records the session it belongs to. The
callback applies the new grant only if:

- that session still exists, isn't ended, and is the browser's current
  session
- the DID matches the session's

Otherwise it fails with an error, and the existing session is untouched.
On success the session keeps its ID and CSRF token, and takes the new
tokens, scopes and DPoP key. If two step-ups race, the last one to finish
wins.

In Rust, a route asks `user.require_scopes(&[…])?`, which fails with
`403 ScopeRequired {scopes}`. Nothing calls it in this feature. Turning
`ScopeRequired` into a step-up navigation in the PWA is left to
[`block-actions`](block-actions.md), its first real user, so that
behaviour gets tested where it's used.

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
  | `OAUTH_EXTRA_SCOPES` | Scopes later features may step up to |
  | `SIGNUP_PDS_URL` | Where "Create account" sends people |
  | `PLC_URL`, `HANDLE_RESOLVER_URL` | Identity. Both default to `ATPROTO_URL`, so dev and tests point everything at vivarium. |
  | `ALLOW_PRIVATE_NETWORK` | On in dev and tests |
  | `SESSION_IDLE_TIMEOUT`, `TOKEN_REFRESH_SKEW` | Session timing |

- `db.rs` and `migrations/`: the database pool and embedded migrations,
  for either backend (see Data).
- `identity.rs`: handle to DID (via `com.atproto.identity.resolveHandle` on
  the resolver), DID to document (PLC or `did:web`), and the PDS and handle
  from the document, with the bidirectional handle check and the
  private-network guard.
- `oauth/`: client metadata and JWKS, discovery, PAR, the callback, refresh
  and revocation, DPoP (including the nonce retry), and client assertions.
  Built on `atproto-oauth` (see Alternatives).
- `auth/`: the session store, the cookies, the CSRF middleware, the
  `CurrentUser` extractor, `require_scopes`, and the login, signup,
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
  stays the same. It always sets `ALLOW_PRIVATE_NETWORK` and
  `OAUTH_EXTRA_SCOPES=transition:generic`.
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

- **Refresh refused:** delete the account in vivarium (`viv.deleteAccount`)
  with a short `TOKEN_REFRESH_SKEW`. The next refresh gets `invalid_grant`.
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
| `oauth_requests` | `state` (key), `kind` (login, signup, step-up), PKCE verifier, DPoP private key, issuer, expected DID, step-up session ID, scopes, `return_to`, `expires_at` | `/oauth/login` and `/oauth/signup` | `/oauth/callback`, which deletes the row |
| `sessions` | `id_hash` (key), DID, handle, display name, avatar URL, DPoP private key, issuer, access and refresh tokens, `token_expires_at`, scopes, CSRF token, `created_at`, `last_seen_at` | callback, refresh, step-up | every authenticated request |
| `client_keys` | `kid`, private JWK, `created_at` | startup, only when `OAUTH_SIGNING_KEY` is unset | client assertions, JWKS, and later `space-sync` |

Expired rows in `oauth_requests` and idle `sessions` are swept on a timer.
The schema is the first in the project, so the `migrations/` convention it
sets is the one later features follow.

### Interfaces

For later features:

- **Rust:**
  - `CurrentUser`: an axum extractor with `did`, `handle` and `scopes`.
    It rejects with `AuthRequired` or `SessionExpired`.
  - `user.require_scopes(&[...]) -> Result<(), ScopeRequired>`.
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
- [`space-sync`](space-sync.md): it asks for the organizer's space `read`
  grant with `require_scopes` and a step-up, not at sign-in. It reuses
  `ClientKeys` for attestation. Its spec needs one line updated when it's
  analyzed.
- [`block-actions`](block-actions.md): its `submitAction` uses `CurrentUser`
  and the CSRF middleware as planned. Before its first write it steps up to
  `space:` `create` on `app.eventside.*`, which it adds to
  `OAUTH_EXTRA_SCOPES`. It owns the PWA's `ScopeRequired` → step-up
  handling, and writes with `pds_client()`.
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

**The OAuth library.** We read the source of each candidate:

- **`atproto-oauth` 0.14, used as a toolkit** (chosen):
  - `oauth_init_with_prompt` takes authorization-server metadata directly,
    with `prompt` and `login_hint`. That covers sign-up and step-up.
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
  every later feature. A test suite that only runs on SQLite would let
  Postgres-breaking SQL through, so how Postgres gets tested matters (see
  the round 1 questions).
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

## Test cases

## Review log
