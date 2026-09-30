---
status: test-cases
impact: cross-cutting
depends-on: []
branch:
tests-commit:
---

# CI pipeline

## Summary

A minimal toolchain skeleton plus GitHub Actions CI. Every later feature
builds on it: it defines what "green" means, and it gives `ship-feature`
checks to wait on before it auto-merges a PR. It's for the agents and humans
working on the app, not for conference attendees.

## Experience

**The stack.** The backend is Rust. The frontend is React with Vite, managed
with pnpm. It ships as a PWA now, and as a native app through Capacitor
later.

**The skeleton.** A fresh clone contains:

- A Rust backend crate that builds and runs a server.
- A React + Vite frontend, with a PWA manifest and service worker, using
  Biome for lint and format.
- vitest and Playwright configs wired to vivarium.
- One trivial passing test of each kind:
  - a `cargo test` unit test
  - a vitest unit test
  - a vitest integration test that starts the Rust server against vivarium
    (the `viv` fixture)
  - a Playwright smoke test that loads the app

It makes no framework or app decisions beyond that.

**Locally.** One command (e.g. `pnpm check`) runs exactly what CI runs. So
"all green before any commit" means the same thing on a laptop and in CI.

**In CI.** Runs are triggered by PRs to `main` and pushes to `main`. A new
push cancels the run still going for the same ref. The checks:

1. **Format and lint:** `cargo fmt --check`, `cargo clippy -D warnings`,
   Biome check.
2. **Typecheck and build:** `tsc`, the Vite PWA production build,
   `cargo build`.
3. **Tests:** `cargo test`, vitest (unit, and integration against a vivarium
   service container, `atcr.io/chris.pardy.family/vivarium`, reached through
   `VIVARIUM_URL`), and Playwright.
4. **Frozen tests:** on a PR from `feature/<slug>`, CI reads `tests-commit`
   from `features/<slug>.md` and runs `scripts/check-tests-unchanged.sh`
   with it. When there's no `tests-commit` yet, or the branch isn't a
   feature branch, the check passes and says it was skipped.

When a run fails, CI uploads the Playwright HTML report, traces, screenshots
and videos as an artifact kept for 14 days.

**Merging.** A GitHub ruleset on `main` requires the CI checks to pass
before a PR can merge. Repository admins can bypass it, so spec commits
(which touch only `features/<slug>.md`) can still be pushed straight to
`main`, as AGENTS.md requires.

**Docs.** AGENTS.md's Testing section gains the Rust convention: backend
unit tests are `cargo test` tests named after their test case (`tc_3_...`).
Integration tests stay in vitest, driving the Rust server against vivarium.

## Data

None. There are no atproto records or spaces. CI artifacts (Playwright
reports) live in GitHub Actions for 14 days.

## Options considered

### How much scaffolding

- **Minimal toolchain skeleton** (chosen): configs and one trivial test per
  kind. CI has real things to run, and app-level choices are left to the
  features that need them.
- **Skeleton plus app framework**: would decide app structure before any
  feature asks for it.
- **CI config only**: nothing real to run, so checks would be vacuous until
  later.

### Toolchain

- **Rust backend, React + Vite frontend, Capacitor mobile plus a PWA**
  (chosen, by the user).
- Bun + Biome (matching vivarium), Node + pnpm + ESLint/Prettier, and
  Node + npm + Biome were offered for an all-TS stack. They're superseded by
  the Rust backend decision.

### JS package manager

- **pnpm** (chosen): fast and strict, with good Vite/Capacitor compatibility
  and CI caching.
- **bun**: matches vivarium, but has occasional edge cases with Capacitor
  tooling.
- **npm**: zero setup, but slower.

### How the Rust backend is tested

- **cargo test for units, vitest for integration** (chosen): pure logic gets
  fast Rust tests. Anything touching atproto goes through vitest and the
  `viv` fixture, which is scoped and safe in parallel.
- **Everything through vitest**: slow feedback on pure logic.
- **cargo test for everything**: gives up the `viv` fixture's scoping and
  parallel safety.

### Mobile builds

- **PWA build only, for now** (chosen): native Capacitor builds wait for a
  feature that actually needs native.
- **PWA + Android debug build**: slower CI, and needs a Capacitor project
  scaffold now.
- **PWA + Android + iOS**: macOS runners cost about 10x Linux minutes, plus
  Xcode setup.

### Triggers

- **PRs to main + pushes to main** (chosen): the ship-feature gate needs PR
  checks, and main is re-verified after each merge.
- **PRs only**: pushes to main go unchecked.
- **Every push on every branch**: runs twice for PR branches.

### Branch protection

- **Required checks, admins bypass** (chosen): red PRs can't merge, and spec
  commits can still go straight to main.
- **No protection**: relies on the agent waiting for checks.
- **Full protection, PRs only**: breaks the spec-on-main workflow.

### Frozen-tests enforcement

- **In CI, driven by the feature file** (chosen): a second line of defence
  behind the skills running the script locally.
- **Leave it to the agent**: nothing catches an agent that skips the step.

### Which checks gate a merge

- **Format + lint, typecheck + build, tests** (chosen).
- **Dependency audit** (rejected): can go red on upstream advisories that
  have nothing to do with the PR.

### Failure artifacts

- **Playwright traces/videos on failure, 14 days** (chosen).
- **Nothing**: flaky UI failures get hard to diagnose.
- **Always upload**: storage for reports nobody reads.

## Out of scope

- Android and iOS (Capacitor native) builds.
- Dependency or security audits.
- Deployment and continuous delivery.
- Caching beyond the standard pnpm, cargo and Playwright-browser caches.
- Demo videos. `ship-feature` records those outside CI.
- Any app functionality, routing or UI beyond a placeholder page.

## Open questions

- How `@vivarium/client` is installed, and how integration tests start the
  Rust server. Both are addressed by the design review below.

## Architecture analysis

**Existing code touched:** none: no code yet. The repo holds only the
pipeline docs (`AGENTS.md`, `.claude/skills/`), `features/` and
`scripts/check-tests-unchanged.sh`. CI calls that script and this feature
edits `AGENTS.md`'s Testing section.

**Features affected:** no other features are specced yet. Every future
feature is affected indirectly: it inherits this project layout, toolchain,
test harness and merge gate, and it can't merge without passing these
checks.

**New shared surfaces:**

- The repo layout: where the Rust crate, the frontend and the tests live.
- The toolchain: pnpm, Vite, Biome, Rust stable, clippy/rustfmt settings.
- The local `check` command, which becomes the meaning of "all green".
- The vitest setup: the vivarium global setup, the `viv` fixture, and how
  integration tests build and start the Rust server and point it at
  vivarium.
- The Playwright config: how it serves the app, the viewport, trace/video
  settings.
- The GitHub Actions workflow, its job and check names (which the ruleset
  pins as required), and the ruleset itself.
- The frozen-tests CI step, which reads the feature file frontmatter.
- The Rust test-naming convention in `AGENTS.md`.

**Findings that affect the design:**

- `@vivarium/client` is not on npm. In `~/Code/atproto/vivarium` it's a bun
  workspace package (`0.0.1`) whose dependencies use `workspace:*`
  (`@vivarium/lexicons`, and an optional peer `@vivarium/core`) and whose
  `dist/` is built with `bun build`. pnpm can't install it straight from the
  git repo, so the design must pick a way to get it: publish it, vendor a
  packed tarball, or use a git subdirectory with a prepare step.
- The global setup can spawn vivarium itself (`VIVARIUM_BIN`) or attach
  (`VIVARIUM_URL`). CI attaches to the service container. Locally, the
  design must say whether developers run the Docker image or a local
  binary.
- The integration tests' Rust server runs on the runner host, while
  vivarium runs in a service container mapped to `localhost:2580`. The box
  then sees the app's callbacks as loopback. The design should confirm that
  vivarium's `--app-host` and `--public-url` defaults work in that layout.

**Verdict:** cross-cutting. This is the first feature in an empty repo. It
sets the stack, the project layout, the build, the test harness and CI, and
every later feature builds on all of those. The required-check names and
the `check` command become a contract that `ship-feature` and all future
PRs depend on.

## Design review

### Approach

The repo is one pnpm workspace and one Cargo workspace, side by side at the
root. Every check is a named package script:

- `pnpm check:lint`
- `pnpm check:build`
- `pnpm check:test`
- `pnpm check:frozen`

`pnpm check` runs them in order. Each CI job runs one of those scripts, so
local and CI parity comes from sharing scripts, not from copying commands
into YAML.

**Vivarium runs as a native binary, sealed, on loopback.** Everything
starts vivarium through `VIVARIUM_BIN`, and nothing uses a container at test
time:

- **In CI,** a step pulls the pinned image, runs `docker create`, and
  `docker cp`s out `/usr/local/bin/vivarium`. That path is documented in
  vivarium's Dockerfile. The step then exports `VIVARIUM_BIN`.
- **Locally,** `VIVARIUM_BIN` points to a compiled vivarium from the
  vivarium checkout.

`scripts/with-vivarium.ts` is a small node script that wraps `check:test`.
It calls `startVivarium({ upstream: false })` from `@vivarium/client`, which
spawns `$VIVARIUM_BIN` on a random port. It exports `VIVARIUM_URL`, runs the
wrapped command, and stops the box afterwards. So vitest, the Rust server
and Playwright all share one sealed box.

- If `VIVARIUM_URL` is already set, the script attaches to it instead.
- `vivFresh` keeps working, because it spawns a new box from `VIVARIUM_BIN`.
- Everything is on loopback, which is the layout vivarium treats as "your
  own service". Future OAuth, did:web and proxy features need that, with no
  `--app-host` or `--public-url`.

A GitHub `services:` container was rejected: it can't take `--no-upstream`,
and a bridge network breaks the loopback layout.

**The Rust server under test** is a prebuilt debug binary.
`check:test` runs `cargo build` first. The `integration` vitest project has
two global setups: vivarium's (which attaches through `VIVARIUM_URL`), then
`tests/support/server-setup.ts`. The server setup spawns
`target/debug/conference-server` with:

- `PORT=0`
- `ATPROTO_URL` set to `process.env.VIVARIUM_URL`

It waits for the `listening on http://127.0.0.1:<port>` line on stdout, and
`provide`s the URL as `serverUrl`. `spawnServer(env)` is exported for tests
that need their own instance. Playwright's `webServer` starts the same
binary, plus `vite preview`, and reads `VIVARIUM_URL` from the environment.

**Getting `@vivarium/client`:** `dist/*.js` already inlines lexicons, but
`dist/client.d.ts` imports types from `@vivarium/lexicons`. The fix is a
prerequisite in the vivarium repo: publish both `@vivarium/lexicons` and
`@vivarium/client` to npm with `bun publish`, which rewrites `workspace:*`
to real versions. This repo depends on pinned versions.

`@vivarium/lexicons` exports TypeScript source, which is fine for type-only
imports under `moduleResolution: bundler`.

### Components

```
Cargo.toml                 Cargo workspace: members = ["crates/*"]
rust-toolchain.toml        pinned stable version, with rustfmt and clippy
crates/server/             bin `conference-server`, axum + tokio
  src/main.rs              reads PORT and ATPROTO_URL, prints the listening line
  src/lib.rs               router: GET /health -> {status, atproto: ok|unreachable}
package.json               root: scripts, devDeps (biome, vitest, playwright,
                           @vivarium/client), packageManager pnpm@<pinned>
pnpm-workspace.yaml        packages: [web]
biome.json                 lint + format for web/, tests/, e2e/, scripts/
tsconfig.base.json
web/                       React + Vite + vite-plugin-pwa; placeholder page
  src/**/*.test.tsx        vitest unit tests (jsdom + Testing Library)
tests/integration/         vitest integration tests (node env, vivarium + server)
tests/support/             server global setup, spawnServer()
vitest.config.ts           projects: `unit` (web, jsdom, no global setup) and
                           `integration` (node, vivarium then server global setup)
e2e/                       Playwright specs
playwright.config.ts       chromium only, phone viewport, serviceWorkers: 'block',
                           trace/video/screenshot retain-on-failure;
                           webServer: server binary + `vite preview`
scripts/with-vivarium.ts   one sealed box for a command (above)
scripts/vivarium-version   the pinned image tag, read by CI
scripts/frozen-tests.sh    wraps check-tests-unchanged.sh with branch logic
.github/workflows/ci.yml   jobs: lint, build, test, frozen-tests
.github/rulesets/main.json the ruleset, as code
scripts/apply-ruleset.sh   gh api create/update of the ruleset
```

The Vite dev server proxies `/api` and `/xrpc` to the Rust server, and
`vite preview` inherits that proxy. The placeholder page shows the result of
`/health`, so the Playwright smoke test proves the whole stack is wired
together.

### Data

None. There are no lexicons, records or spaces.

### Interfaces other features will use

- **The scripts:** `pnpm check`, plus `pnpm test:unit`,
  `test:integration`, `test:e2e`, `test:rust` for running a subset.
- **Test locations and naming:**
  - `web/**/*.test.tsx` for units
  - `tests/integration/**/*.test.ts` for integration
  - `e2e/**/*.spec.ts` for UI
  - `#[test] fn tc_N_...` in `crates/*`
- **The server contract for tests:** the binary reads `PORT` and
  `ATPROTO_URL`, and prints `listening on <url>`. Tests get `serverUrl` via
  `inject('serverUrl')`, `viv` and `vivFresh` from
  `@vivarium/client/vitest`, and `spawnServer(env)` from `tests/support`.
- **Required check names:** `lint`, `build`, `test`, `frozen-tests`.
  Renaming a job means updating the ruleset in the same PR.
- **The local prerequisites:** Node (the version in `.nvmrc`), pnpm via
  corepack, rustup (which honours `rust-toolchain.toml`), and `VIVARIUM_BIN`.

### CI workflow

The workflow runs on `pull_request` to `main` and `push` to `main`. Runs are
grouped by `ci-${{ github.ref }}`. A new push cancels the older run on PRs
only, so every run on `main` finishes.

Setup is `pnpm/action-setup`, then `actions/setup-node` with `cache: pnpm`,
then `Swatinem/rust-cache`.

| Job | Runs | Notes |
|-----|------|-------|
| `lint` | `pnpm check:lint` | cargo fmt --check, clippy `-D warnings`, biome ci |
| `build` | `pnpm check:build` | tsc -b, vite build (PWA), cargo build --locked |
| `test` | extract the vivarium binary, then `pnpm check:test` | cargo test, vitest (both projects), Playwright (`playwright install --with-deps chromium`, browsers cached by version). Uploads `playwright-report/` + `test-results/` on failure, kept 14 days |
| `frozen-tests` | `pnpm check:frozen` | checks out `github.event.pull_request.head.sha` with `fetch-depth: 0`, not the merge ref |

**`frozen-tests.sh`** works out the branch from `GITHUB_HEAD_REF` in CI, or
`git branch --show-current` locally. Anything other than `feature/<slug>`
passes with "skipped: not a feature branch".

On a feature branch, the script looks in the history of
`features/<slug>.md` on the branch (commits not on `origin/main`) for the
first non-empty `tests-commit` value. That value is the freeze.

- **No value ever set:** passes with "skipped: no tests-commit yet".
- **Current value differs from the first, or is blank:** fails. A PR can't
  re-point or erase the freeze.
- **The frozen commit isn't an ancestor of `HEAD`:** fails.
- **Otherwise:** runs `check-tests-unchanged.sh <sha>`.

A skipped check still reports success, so a required check never hangs.

### Ruleset

The ruleset targets `main` and requires the status checks `lint`, `build`,
`test` and `frozen-tests` (from GitHub Actions). "Require branches to be up
to date" is off. It doesn't require PRs, and the Repository admin role
bypasses it, so spec commits can still be pushed to `main`.

It's stored as JSON and applied by `scripts/apply-ruleset.sh` during the
build, using the user's admin `gh` auth.

**Limitation:** the agent pushes and merges as an admin, and admins bypass
the ruleset. So the ruleset stops humans and non-admin tokens from merging
red. The agent's real gate is `ship-feature`. This feature tightens that
skill in two ways:

- **Before merging,** it confirms that every required check on the PR head
  is `pass`, and never uses `--admin`.
- **When `gh pr checks` reports "no checks",** it retries for up to 2
  minutes before concluding that CI isn't configured. Right after
  `gh pr create`, the workflow may simply not have registered yet.

### This feature's own tests

The frozen tests commit contains only test files: `crates/server` tests,
`web/src/**/*.test.tsx`, `tests/integration/**`, `e2e/**`, and a vitest
suite for `scripts/frozen-tests.sh` that runs it against throwaway git repos.

Configs, `tests/support/`, scripts and the workflow aren't frozen. Test
cases get defined in step 4.

A `tests-commit` recorded on a squash-merged feature's file is kept for
audit only. After the squash, the SHA is reachable through the PR's
`refs/pull/N/head`, not through `main`.

### Impact on existing features

None exist. The changes to shared docs are:

- **`AGENTS.md`** gets:
  - the Rust test convention
  - how vivarium is run (as a binary through `VIVARIUM_BIN`, never as a
    `services:` container), replacing the current "run the image as a
    service" line
  - `pnpm check` as the "all green" command
- **`.claude/skills/ship-feature`** gets the two gate fixes above.

### Alternatives

- **Vivarium as a `services:` container:** can't be sealed, and can't use
  loopback.
- **`docker run --network host` in CI, with a Docker fallback locally:**
  `vivFresh` breaks without a binary, there are three code paths, and host
  networking is unreliable on macOS.
- **Vendor a packed `@vivarium/client` tarball:** manual repacking. This is
  the fallback if publishing is blocked.
- **Move lexicons to devDependencies and publish only the client:** the
  client's `.d.ts` files import lexicons, so its types would break.
- **One `check` job:** fully serial, and one opaque check.
- **Fold `frozen-tests` into `lint`:** saves a runner, but hides which rule
  failed. Kept separate.
- **`cargo run` in the vitest global setup:** compile errors turn into
  timeouts.
- **ESLint + Prettier:** slower, with more config. Biome matches vivarium.

### Risks

- **Publishing the two vivarium packages** is work in another repo, and it
  blocks the build until it's done. The fallback is vendored tarballs.
- **The binary extracted from the image** is linux/amd64. It needs
  `ubuntu-latest` on x86 runners, and it relies on the Dockerfile's
  `/usr/local/bin/vivarium` path.
- **The admin bypass** means an agent run could merge red if
  `ship-feature` is skipped. That's mitigated by the skill change, not
  enforced by GitHub.
- **Toolchain and image drift** is avoided by pinning both, and bumping
  them deliberately.
### Round 1

The first draft ran vivarium with `docker run --network host` and fell back
to Docker locally, and planned to publish only `@vivarium/client`. A
subagent critique found several problems, and the draft was revised to fix
them:

- `vivFresh` and Playwright had no way to get a box. Fix: always use the
  binary via `VIVARIUM_BIN`, extracted from the image in CI, with one sealed
  box per run from `with-vivarium.ts`.
- The client's `.d.ts` files import `@vivarium/lexicons`. Fix: publish both
  packages with `bun publish`.
- The frozen-tests check could be bypassed by blanking `tests-commit`, and
  ran against the merge ref. Fix: use the first recorded value, and check
  out the PR head.
- Admin bypass means the ruleset doesn't gate the agent. Fix: tighten
  `ship-feature` to verify required checks and retry "no checks".
- Smaller fixes: `pnpm/action-setup` ordering,
  `playwright install --with-deps`, blocking service workers in tests, and
  the scope of this feature's own frozen tests.

The user approved the revised design as written, including publishing
`@vivarium/client` and `@vivarium/lexicons` to npm from the vivarium repo as
a prerequisite for the build.

## Test cases

## Review log
