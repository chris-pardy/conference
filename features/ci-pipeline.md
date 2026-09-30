---
status: complete
impact: cross-cutting
depends-on: []
branch: feature/ci-pipeline
tests-commit: 1e253c62660da8051a2e231e43cd1d4b41aa8372
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

None. The design review settled how vivarium is installed (the
`@vivarium-dev/cli` and `@vivarium-dev/client` npm packages, as of round 2) and how integration tests start the Rust server (a
prebuilt binary started by a global setup).

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

**Vivarium comes from npm, as a native binary, sealed, on loopback.** Two
devDependencies are pinned in `package.json` and locked in
`pnpm-lock.yaml`:

- **`@vivarium-dev/cli`** ships prebuilt binaries as platform packages:
  darwin-arm64/x64, linux-x64/arm64 (with musl variants) and win32-x64. No
  Bun or Docker is needed.
- **`@vivarium-dev/client`** is the test-suite client and vitest fixtures.
  It finds the CLI's binary for the current platform by itself, through
  `@vivarium-dev/cli/resolve`.

The same `pnpm install` works for a laptop and for CI, with no extra CI
step, no image and no `VIVARIUM_BIN`. `VIVARIUM_BIN` still overrides the
binary, e.g. to test against a local vivarium checkout. Upgrading vivarium
is a normal dependency bump.

`scripts/with-vivarium.ts` is a small node script that wraps `check:test`.
It calls `startVivarium({ upstream: false })` from `@vivarium-dev/client`,
which spawns the binary on a random port. It exports `VIVARIUM_URL`, runs the
wrapped command, and stops the box afterwards. So vitest, the Rust server
and Playwright all share one sealed box.

- If `VIVARIUM_URL` is already set, the script attaches to it instead.
- `vivFresh` keeps working, because it spawns a new box from the same
  binary.
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

**Prerequisite:** `@vivarium-dev/cli`, its platform packages,
`@vivarium-dev/client` and `@vivarium-dev/lexicons` are published to npm.
That's in progress in the vivarium repo. pnpm installs only the platform
package that matches the machine, and `--omit=optional` must never be
used, because it drops the binary.

### Components

```
Cargo.toml                 Cargo workspace: members = ["crates/*"]
rust-toolchain.toml        pinned stable version, with rustfmt and clippy
crates/server/             bin `conference-server`, axum + tokio
  src/main.rs              reads PORT and ATPROTO_URL, prints the listening line
  src/lib.rs               router: GET /health -> {status, atproto: ok|unreachable}
package.json               root: scripts, devDeps (biome, vitest, playwright,
                           @vivarium-dev/client, @vivarium-dev/cli),
                           packageManager pnpm@<pinned>
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
  `@vivarium-dev/client/vitest`, and `spawnServer(env)` from `tests/support`.
- **Required check names:** `lint`, `build`, `test`, `frozen-tests`.
  Renaming a job means updating the ruleset in the same PR.
- **The local prerequisites:** Node (the version in `.nvmrc`), pnpm via
  corepack, and rustup (which honours `rust-toolchain.toml`). Vivarium
  arrives with `pnpm install`.

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
| `test` | `pnpm check:test` | cargo test, vitest (both projects), Playwright (`playwright install --with-deps chromium`, browsers cached by version). Uploads `playwright-report/` + `test-results/` on failure, kept 14 days |
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
  - how vivarium is run: the `@vivarium-dev/cli` binary from npm, started
    by `@vivarium-dev/client`, never as a `services:` container or through
    Docker. This replaces the current "run the image as a service" line
  - the package scope rename, from `@vivarium/client/vitest` to
    `@vivarium-dev/client/vitest"
  - `pnpm check` as the "all green" command
- **`.claude/skills/ship-feature`** gets the two gate fixes above.

### Alternatives

- **Vivarium as a `services:` container:** can't be sealed, and can't use
  loopback.
- **`docker run --network host` in CI, with a Docker fallback locally:**
  `vivFresh` breaks without a binary, there are three code paths, and host
  networking is unreliable on macOS.
- **Extract the binary from the Docker image in CI, with `VIVARIUM_BIN`
  locally** (round 1): superseded once the npm CLI existed. It needed an
  extra CI step, only ran on linux/amd64, depended on the image's internal
  layout, and needed a manual setup on laptops.
- **Vendor packed vivarium tarballs:** manual repacking, now unnecessary.
- **One `check` job:** fully serial, and one opaque check.
- **Fold `frozen-tests` into `lint`:** saves a runner, but hides which rule
  failed. Kept separate.
- **`cargo run` in the vitest global setup:** compile errors turn into
  timeouts.
- **ESLint + Prettier:** slower, with more config. Biome matches vivarium.

### Risks

- **The `@vivarium-dev/*` packages must be on npm** before the build.
  Publishing is in progress.
- **Optional platform packages:** if a machine's platform has no prebuilt
  binary, or optional dependencies are omitted, `startVivarium` falls back
  to `vivarium` on PATH and fails clearly.
- **The admin bypass** means an agent run could merge red if
  `ship-feature` is skipped. That's mitigated by the skill change, not
  enforced by GitHub.
- **Toolchain and vivarium drift** is avoided by pinning the Rust
  toolchain and the locked npm version, and bumping them deliberately.
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

### Round 2

**User feedback:** vivarium is being published as a standalone npm command
(`npx @vivarium-dev/cli`), with prebuilt binaries for each platform and no
Bun required. The npm scope was renamed to `@vivarium-dev`. Rework the
approach around that.

**What changed:**

- Vivarium comes from pinned `@vivarium-dev/cli` and `@vivarium-dev/client`
  devDependencies. `@vivarium-dev/client` finds the CLI's binary by itself.
- The Docker image extraction step, the pinned image tag and the local
  `VIVARIUM_BIN` requirement are gone.
- Everything else stands: one sealed box on loopback per run through
  `with-vivarium.ts`, `vivFresh`, and the jobs and ruleset.
- TC-1 and TC-31 are reworded, keeping their numbers. The user approved
  the reworked design and both test cases.

## Test cases

**Automation:** TC-1 to TC-33 are automated. The full fresh-clone run in
TC-1 would recurse inside the check, so its automated test checks that
`pnpm check` runs every step in order, and CI's clean checkout proves the
rest (TC-38). TC-34 to TC-38 are manual. They need the live GitHub repo or
the agent's own behavior, and are verified on the PR.

### The skeleton works end to end

#### TC-1: A fresh clone passes the full check

- **Given** a fresh clone with only Node, pnpm and Rust installed: no
  Docker, Bun or separately installed vivarium
- **When** the developer installs dependencies and runs the full check
- **Then** lint, build, tests and the frozen-tests check run in that order,
  and all pass
- **And** the command exits successfully

#### TC-2: The server reports that it can reach atproto

- **Given** the backend server is started against the test vivarium
- **When** a client asks for its health
- **Then** it answers that it's up and that atproto is reachable

#### TC-3: The server stays up when atproto is unreachable

- **Given** the backend server is started with an atproto address that
  nothing answers on
- **When** a client asks for its health
- **Then** it answers that it's up and that atproto is unreachable,
  rather than crashing or hanging

#### TC-4: The server picks a free port and announces it

- **Given** the backend server is told to use any free port
- **When** it starts
- **Then** it announces the address it's listening on
- **And** that address answers health requests

#### TC-5: The backend's health response has a fixed shape

- **Given** the backend's health logic, tested on its own with no network
- **When** atproto is reported reachable, and then unreachable
- **Then** the response carries the matching status each time
  (a backend unit test)

#### TC-6: The placeholder page shows backend health

- **Given** the frontend is rendered on its own with a faked backend
  answer
- **When** the health answer arrives
- **Then** the page shows the backend's status
- **And** before the answer arrives, the page shows a loading state
  (a frontend unit test)

#### TC-7: The app loads on a phone and shows the whole stack is connected

- **Given** the production build of the app, the backend server and the
  test vivarium are running
- **When** a phone-sized browser opens the app
- **Then** the placeholder page shows that the backend is up and that
  atproto is reachable

#### TC-8: The production build is an installable PWA

- **Given** the production build of the frontend
- **When** its web app manifest and service worker are inspected
- **Then** the manifest has a name, a start URL, a standalone display mode
  and icons
- **And** a service worker is generated and registered by the page

### Vivarium in tests

#### TC-9: Tests run against a sealed vivarium

- **Given** a test run started through the check
- **When** a test asks the box for an identity that exists only on the
  real network
- **Then** the box doesn't find it, and no request leaves the machine

#### TC-10: One box is shared across a test run

- **Given** a test run started through the check
- **When** integration tests and browser tests run
- **Then** they all talk to the same vivarium instance
- **And** it's stopped when the run ends, whether the run passed or failed

#### TC-11: A run attaches to a box that's already running

- **Given** a vivarium is already running and its address is provided
- **When** a test run starts
- **Then** it uses that box instead of starting a new one
- **And** it leaves the box running when it ends

#### TC-12: The test run reports the real result

- **Given** a test run started through the vivarium wrapper
- **When** the wrapped tests fail
- **Then** the wrapper exits with a failure, and the box is still stopped

#### TC-13: A test can ask for a pristine box

- **Given** a test that asks for a fresh vivarium
- **When** it runs inside the check, locally or in CI
- **Then** it gets its own new, sealed box, separate from the shared one

### Lint and build gates

#### TC-14: Badly formatted Rust fails the lint step

- **Given** a Rust file that isn't formatted to the project's style
- **When** the lint step runs
- **Then** it fails and names the file

#### TC-15: A Rust compiler lint warning fails the lint step

- **Given** Rust code that triggers a clippy warning
- **When** the lint step runs
- **Then** it fails

#### TC-16: A TypeScript lint or format problem fails the lint step

- **Given** a TypeScript file with a lint error or bad formatting
- **When** the lint step runs
- **Then** it fails and names the file

#### TC-17: A type error fails the build step

- **Given** a frontend file with a type error
- **When** the build step runs
- **Then** it fails

### Frozen tests

#### TC-18: Branches that aren't feature branches skip the check

- **Given** a branch not named `feature/<slug>`
- **When** the frozen-tests check runs
- **Then** it passes and says it was skipped because this isn't a feature
  branch

#### TC-19: A feature branch with no frozen tests yet skips the check

- **Given** a feature branch whose feature file has never had a
  `tests-commit`
- **When** the frozen-tests check runs
- **Then** it passes and says it was skipped because there's no
  tests-commit yet

#### TC-20: Unchanged frozen tests pass

- **Given** a feature branch with a recorded `tests-commit`, and later
  commits that only change other files
- **When** the frozen-tests check runs
- **Then** it passes

#### TC-21: A changed frozen test fails the check

- **Given** a feature branch with a recorded `tests-commit`
- **When** a file from that commit has been changed, either committed or
  only in the working tree
- **Then** the check fails and names the changed file

#### TC-22: Erasing the freeze fails the check

- **Given** a feature branch where `tests-commit` was set and later blanked
- **When** the frozen-tests check runs
- **Then** it fails, saying the freeze was removed

#### TC-23: Re-pointing the freeze fails the check

- **Given** a feature branch where `tests-commit` was changed to a
  different commit after it was first set
- **When** the frozen-tests check runs
- **Then** it fails, naming the original commit

#### TC-24: A freeze outside the branch's history fails the check

- **Given** a feature branch whose `tests-commit` isn't in the branch's
  history
- **When** the frozen-tests check runs
- **Then** it fails

#### TC-25: In CI, the branch comes from the pull request

- **Given** CI is checking a pull request from `feature/<slug>`, and the
  checkout isn't on a named branch
- **When** the frozen-tests check runs
- **Then** it checks the feature named by the pull request's branch

### The CI workflow

#### TC-26: CI runs on pull requests and on main

- **Given** the CI workflow
- **Then** it runs for pull requests targeting `main` and for pushes to
  `main`
- **And** it doesn't run for pushes to other branches

#### TC-27: CI has four checks, each running the local script

- **Given** the CI workflow
- **Then** it has exactly the checks `lint`, `build`, `test` and
  `frozen-tests`
- **And** each one runs the same package script a developer runs locally

#### TC-28: A newer push cancels an older PR run, but main runs finish

- **Given** the CI workflow
- **Then** a new push to a pull request cancels that PR's run still in
  progress
- **And** runs on `main` are never cancelled

#### TC-29: Failed browser tests leave evidence

- **Given** the CI workflow
- **Then** when the `test` check fails, the Playwright report, traces,
  screenshots and videos are uploaded and kept for 14 days
- **And** nothing is uploaded when it passes

#### TC-30: The frozen-tests check sees the PR's own commits

- **Given** the CI workflow
- **Then** the frozen-tests check examines the pull request's head commit,
  with full history, not GitHub's merge of it into `main`

#### TC-31: CI's vivarium is pinned and sealed

- **Given** the CI workflow and the project's dependencies
- **Then** vivarium comes from the locked npm package version, installed
  like any other dependency
- **And** the workflow has no Docker step or service container for it, and
  every box it starts is sealed

### Merge gate

#### TC-32: The ruleset requires the four checks on main

- **Given** the ruleset definition
- **Then** it targets `main` and requires `lint`, `build`, `test` and
  `frozen-tests` to pass
- **And** it doesn't require pull requests, and it lets repository admins
  bypass it

#### TC-33: The ruleset and the workflow agree on check names

- **Given** the ruleset definition and the CI workflow
- **Then** every required check in the ruleset is a job in the workflow

#### TC-34: Applying the ruleset twice changes nothing (manual)

- **Given** the ruleset has been applied to the GitHub repository
- **When** the apply script runs again
- **Then** it updates the existing ruleset instead of creating a second one

#### TC-35: A red pull request can't be merged by a non-admin (manual)

- **Given** a pull request with a failing required check
- **Then** GitHub shows it as blocked from merging for anyone without the
  bypass

#### TC-36: Spec commits can still go straight to main (manual)

- **Given** the ruleset is active
- **When** the admin pushes a commit that only touches a feature file to
  `main`
- **Then** the push is accepted

#### TC-37: The agent waits for every required check before merging (manual)

- **Given** a minor pull request whose checks are still pending, or have
  just been created and aren't listed yet
- **When** the agent reaches the review gate
- **Then** it waits for all four required checks to pass before merging,
  never merges with the admin override, and doesn't mistake "no checks yet"
  for "CI not configured"

#### TC-38: This feature's own pull request goes green in CI (manual)

- **Given** the pull request that builds this feature
- **When** CI runs on it
- **Then** all four checks pass on GitHub

## Review log

### Round 1

Reviewer: a fresh subagent following `adversarial-review`. Frozen tests
unchanged; `pnpm check` green. Verdict: not clean (2 major, 3 minor, 2 nit).

1. **[major] Pending runs on `main` could be cancelled.** GitHub cancels a
   *pending* run whenever a newer one joins its concurrency group, whatever
   `cancel-in-progress` says. So quick pushes to `main` could leave a commit
   unchecked.
   **Fixed:** PR runs share a group per ref. Every run on `main` gets its
   own group (`github.run_id`).
2. **[major] The PR under test ran its own enforcement scripts,** so it
   could disable its own frozen-tests check.
   **Fixed:** the `frozen-tests` job now takes `frozen-tests.sh` and
   `check-tests-unchanged.sh` from `origin/main` whenever `main` has them,
   and fails if `package.json`'s `check:frozen` no longer runs that script.
   **Residual:** a PR can still edit `.github/workflows/ci.yml` itself.
   `ship-feature` treats any CI change as architectural, so such a PR
   always waits for a human.
3. **[minor] The dev proxy defaulted to port 3100 but the server to 3000.**
   **Fixed:** the server now defaults to 3100, matching the proxy.
4. **[minor] `reqwest` had no TLS,** so every HTTPS atproto service looked
   unreachable and nothing reported why.
   **Fixed:** enabled `rustls-tls`. The health probe now logs why it
   failed to stderr.
5. **[minor] The e2e servers used fixed ports,** so concurrent checks
   collided.
   **Fixed:** the Playwright config picks free ports once in the main
   process and shares them with its workers through the environment.
6. **[nit] AGENTS.md overstated which scripts use `with-vivarium`.**
   **Fixed.**
7. **[nit] `with-vivarium` exit codes:** a signal-killed child exited 128,
   and a failing `box.stop()` could hide the real exit code.
   **Fixed:** it now exits with 128 + the signal number, and a failure to
   stop the box is logged but no longer changes the exit code.

### Round 2

Reviewer: a fresh subagent following `adversarial-review`. Frozen tests
unchanged; `pnpm check` green. Verdict: not clean (3 major, 1 minor, 2 nit).

1. **[major] The PR's own pnpm configuration could still bypass
   frozen-tests.** An `.npmrc` with `script-shell=/usr/bin/true`, or a
   `precheck:frozen` script, got past round 1's fix.
   **Fixed:** the job now sets `npm_config_script_shell=/bin/bash` and
   `npm_config_enable_pre_post_scripts=false` in its environment, which
   overrides any project `.npmrc`. It restores `.npmrc` from `main` (or
   deletes it), and fails if `package.json` defines `precheck:frozen` or
   `postcheck:frozen`.
2. **[major] Frozen TC-7 matched "atproto: unreachable",** because
   `/atproto.*reachable/i` also matches "unreachable". So it passed without
   a connected stack.
   **Fixed in the implementation:** the page now shows an unreachable
   atproto as "atproto: offline". Verified: with atproto unreachable, TC-7
   now fails.
3. **[major] The service worker's navigation fallback served `index.html`
   for backend routes.**
   **Fixed:** added `navigateFallbackDenylist` for `/api/`, `/xrpc/` and
   `/health`. The built `sw.js` now carries the denylist.
4. **[minor] An empty `ATPROTO_URL` was probed as a relative URL.**
   **Fixed:** the server treats an empty value as unset. `spawnServer`
   and the Playwright config fail fast without a run vivarium.
5. **[nit] `apply-ruleset.sh` didn't paginate its lookup.**
   **Fixed:** it now uses `--paginate`.
6. **[nit] `pnpm check` builds the web app twice.**
   **Left as is:** each `test:*` script builds what it needs so it can run
   on its own, and the extra Vite build takes well under a second here.

### Round 3

Reviewer: a fresh subagent following `adversarial-review`. Frozen tests
unchanged; `pnpm check` green. Verdict: not clean (2 major, 2 minor, 1 nit).

1. **[major] A committed `node_modules/.bin` could replace `bash` or `git`
   in the `frozen-tests` job,** because pnpm puts it first on PATH.
   **Fixed:** the job fails if the branch tracks any `node_modules`, and
   deletes `node_modules` before running the check. It never installs
   dependencies.
2. **[major] The ruleset can't be applied:** the repository is private on a
   free plan, and GitHub returns 403 for rulesets and branch protection.
   **Was blocked**, and resolved by the user.
   - Moving to tangled with a spindle was researched first. Tangled has no
     merge gate at all: no branch protection or required checks. Its
     hosted spindle has a 5-minute default timeout that workflow YAML
     can't override. Its Nix images can't run the prebuilt vivarium
     binary. It has no artifact upload, and no `gh`-equivalent PR check
     watching or squash merge for `ship-feature`.
   - **The user chose to make the GitHub repository public.** After a scan
     of the history found no secrets, it was made public on 2026-09-30.
     `scripts/apply-ruleset.sh` created ruleset 24276263, which is active
     on the default branch, requires `lint`, `build`, `test` and
     `frozen-tests`, and lets admins bypass it.
   - **TC-34 verified:** running the script a second time updated the same
     ruleset, and the repository still has exactly one.
3. **[minor] No job timeouts.**
   **Fixed:** `timeout-minutes` is 15 for lint and build, 25 for test, and
   5 for frozen-tests.
4. **[minor] `VIVARIUM_UPSTREAM=1` in a developer's shell would unseal
   `vivFresh` boxes.**
   **Fixed:** `with-vivarium` removes it from the wrapped command's
   environment.
5. **[nit] The proxy prefix keys and the service-worker denylist
   disagreed.**
   **Fixed:** the proxy keys are now anchored patterns that match the
   denylist.

### Round 4

Reviewer: a fresh subagent following `adversarial-review`. Frozen tests
unchanged; `pnpm check` green; the live ruleset matches the file. Verdict:
not clean (2 major, 1 minor, 1 nit).

1. **[major] The `tests-commit` parser glued a trailing `# comment` onto the
   SHA.** That comment format is the one `features/README.md` documents. The
   parser also read quoted values and matched `tests-commit:` lines outside
   the frontmatter.
   **Fixed:** the parser now reads only the frontmatter, strips quotes and a
   trailing comment, and takes the first word. Anything that isn't a
   7–40 character hex SHA fails with "isn't a commit SHA". New unfrozen
   tests in `tests/tooling/frozen-tests-format.test.ts` cover the README
   format, quotes, a body example and a non-SHA.
2. **[major] A committed `.pnpmfile.cjs` could skip `pnpm check:frozen`.**
   **Fixed, and the whole class closed:** the job sets
   `npm_config_ignore_pnpmfile` and deletes `.pnpmfile.cjs`. It also now
   runs main's `frozen-tests.sh` directly: `/bin/bash` in a clean
   environment (`env -i`, `PATH=/usr/bin:/bin`), with pnpm and every other
   file the branch controls out of the loop. `pnpm check:frozen` stays for
   parity with local runs (TC-27).
3. **[minor] `check-tests-unchanged.sh` let a file renamed in the frozen
   commit escape the freeze,** because of git's rename detection.
   **Fixed:** it now uses `--no-renames`. A new test covers a renamed
   frozen file.
4. **[nit] The service-worker denylist didn't match `/health?query` like
   the proxy did.**
   **Fixed:** the denylist entry is now `/^\/health(\?|$)/`.

### Round 5

Reviewer: a fresh subagent following `adversarial-review`. Frozen tests
unchanged; `pnpm check` green; the first real CI run (run 36785876444 on
draft PR #1) passed all four checks, and both frozen-tests paths
enforced the freeze. Verdict: not clean, but only minor issues (0 blocking,
0 major, 3 minor, 1 nit). The reviewer said none of them touch the main
path or the stated requirements.

1. **[minor] Rebasing a frozen branch breaks the check for good,** and no
   doc said to merge instead.
   **Fixed:** added a rule to AGENTS.md: never rebase a feature branch once
   its tests are frozen; bring in main with `git merge origin/main`. The
   not-an-ancestor error now says the same. A new test covers it.
2. **[minor] A frozen path containing a space escaped the freeze,** because
   of an unquoted word-split.
   **Fixed:** `check-tests-unchanged.sh` now reads the file list
   NUL-separated and passes `:(literal)` pathspecs. A new test covers a
   frozen `tests/a b.test.ts`.
3. **[minor] The architectural checklist didn't name the configs that
   decide which tests run.**
   **Fixed:** `ship-feature` now lists `.github/`, `scripts/`, the vitest
   and Playwright configs, `biome.jsonc`, the Rust toolchain and format
   configs, `Cargo.toml` files, and `package.json` scripts.
4. **[nit] GitHub warned that the actions ran on the deprecated Node 20
   runtime.**
   **Fixed:** bumped to the current majors (`checkout@v7`,
   `setup-node@v7`, `cache@v6`, `upload-artifact@v7`,
   `pnpm/action-setup@v6`).

**Status after five rounds:** by the pipeline's rule, a fifth round that
isn't clean blocks the feature. The findings have gone down every round:

| Round | blocking | major | minor | nit |
|---|---|---|---|---|
| 1 | 0 | 2 | 3 | 2 |
| 2 | 0 | 3 | 1 | 2 |
| 3 | 0 | 2 | 2 | 1 |
| 4 | 0 | 2 | 1 | 1 |
| 5 | 0 | 0 | 3 | 1 |

What kept coming back was hardening the frozen-tests check against a
deliberately adversarial branch. The bypasses went: running the PR's own
scripts, then `.npmrc`, pre/post scripts, `node_modules/.bin`, and
`.pnpmfile.cjs`. Round 4 closed that class by also running main's script
directly in a clean environment, and round 5 found no bypass. Everything
round 5 found is fixed. **The user chose to ship** rather than run a
sixth round.
