---
status: design-review
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

- Whether `@vivarium/client` is installable from a registry or has to come
  from the vivarium repo (git/workspace dependency). Architecture analysis
  should settle this.
- How the vitest integration fixture builds and starts the Rust server
  (prebuilt binary from the build job vs. `cargo run` in global setup).

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

## Test cases

## Review log
