# Agent Instructions

Every change to this app is a feature, and every feature goes through the
pipeline below. Each feature has a file in [`features/`](features/) that
records where it stands. Its `status` field says which step comes next.

## The pipeline

The pipeline has two phases, and a feature can wait between them as long as
it needs to. Many features can be specced before any of them are built.

**Spec** (steps 1–4): documents only, committed straight to `main`.

| # | Step | Skill | Who | Status when done |
|---|------|-------|-----|------------------|
| 1 | Feature brainstorming | [`brainstorm-feature`](.claude/skills/brainstorm-feature/SKILL.md) | agent + human Q&A | `analysis` |
| 2 | Architecture analysis | [`analyze-architecture`](.claude/skills/analyze-architecture/SKILL.md) | agent | `design-review` or `test-cases` |
| 3 | Design review (cross-cutting features only) | [`design-review`](.claude/skills/design-review/SKILL.md) | agent + human, one or more rounds | `test-cases` |
| 4 | Test cases | [`write-test-cases`](.claude/skills/write-test-cases/SKILL.md) | agent writes, human approves | `ready` |

**Build** (steps 5–12): on a `feature/<slug>` branch, landing through a PR.

| # | Step | Skill | Who | Status when done |
|---|------|-------|-----|------------------|
| 5–6 | Refresh the spec; write automated tests, shown red | [`write-tests`](.claude/skills/write-tests/SKILL.md) | agent writes, human approves | `implementing` |
| 7–9 | Implement → adversarial review → rework, repeated until clean | [`implement-feature`](.claude/skills/implement-feature/SKILL.md), [`adversarial-review`](.claude/skills/adversarial-review/SKILL.md) | agent, unsupervised | `implementing` |
| 10–12 | Mark complete, PR + demo video → review gate | [`ship-feature`](.claude/skills/ship-feature/SKILL.md) | agent; human only for architectural PRs | `complete` (in the PR) |

If you don't load skills automatically, read the linked `SKILL.md` for the
step before you start it.

Statuses, in order: `analysis`, `design-review`, `test-cases`, `ready`,
`implementing`, `complete`. `ready` means fully specced and waiting to be
built. A feature that can't go on without a human decision is `blocked`, and
its file explains why.

A PR that finishes a feature sets `status: complete` itself, so `main` shows
a feature as complete exactly when its PR merges. While the build is on a
branch, `main` still shows `ready`.

A spec session ends at `ready`. Don't start building unless the user asks.
When they ask to build "the next feature", pick a `ready` feature whose
`depends-on` features are all `complete`, and confirm the choice with them.

## Human gates

Stop and wait for the human only at these points:

1. The end of brainstorming, when they confirm the feature description.
2. Each design review round.
3. Approval of the test cases.
4. At the start of a build, any spec changes the refresh turned up.
5. Approval of the red automated tests.
6. Merging an architectural PR.

After the red tests are approved, work through steps 7–12 without asking
anything. If you're truly stuck (a frozen test looks wrong, or the
requirements contradict each other), set `status: blocked`, write the reason
in the feature file, push, and stop.

## Rules

- **Approved tests are frozen.** The commit recorded as `tests-commit` in the
  feature file holds the approved tests. From then on, nothing may change the
  files in that commit. `scripts/check-tests-unchanged.sh <tests-commit>`
  enforces this, and implementation, review and shipping all run it. If a
  frozen test is wrong, don't edit it. Set the feature to `blocked` and ask.
- **Specs on `main`, builds on branches.** Spec commits touch only
  `features/<slug>.md` and go directly to `main`. Run `git pull --rebase`
  before each push, since other spec sessions may be pushing too. A build
  starts a `feature/<slug>` branch at step 5, and the tests and code land
  through its PR, which also marks the feature complete. Nothing else is
  committed directly to `main`. Push after every commit.
- **Never rebase a feature branch once its tests are frozen.** Rebasing
  rewrites `tests-commit`, and the frozen-tests check then fails for good.
  Bring in `main` with `git merge origin/main` (the `git pull --rebase`
  habit is for `main` only).
- **Commit identity.** Every commit's author and committer is
  `Chris Pardy <chris.pardy@gmail.com>`. This repo's git config already sets
  it, so don't override it with `-c`, `--author` or environment variables.
- **All green before any commit of implementation code** (`pnpm check`). That means the
  whole suite, not only the feature's own tests.

## Testing

- **`pnpm check` is "all green".** It runs, in order, `check:lint`
  (rustfmt, clippy with `-D warnings`, Biome), `check:build` (tsc, the Vite
  PWA build, cargo build), `check:test` (every suite against one sealed
  vivarium) and `check:frozen` (the frozen-tests rule). CI runs the same four
  scripts as the required checks `lint`, `build`, `test` and `frozen-tests`.
  Use `pnpm test:rust`, `test:unit`, `test:integration`, `test:tooling` or
  `test:e2e` to run one suite.
- Use standard packages: **vitest** for unit and integration tests, and
  **Playwright** for UI flows and demo videos. The Rust backend's unit tests
  use `cargo test` and are named after their test case too
  (`fn tc_3_a_member_can_leave_a_session_chat()`). Anything that exercises
  the running backend is a vitest integration test in `tests/integration/`.
- Anything that talks to atproto (PLC, PDS, relay, Jetstream, Constellation,
  Slingshot, OAuth, spaces) runs against **vivarium**
  ([tangled.org/chris.pardy.family/vivarium](https://tangled.org/chris.pardy.family/vivarium)),
  never the live network.
  - It comes from npm: `@vivarium-dev/cli` (prebuilt binaries, no Bun or
    Docker) and `@vivarium-dev/client`, both pinned devDependencies. Never
    install with `--omit=optional`, which drops the binary.
  - `scripts/with-vivarium.ts` starts one sealed (`--no-upstream`) box for a
    whole run and exports `VIVARIUM_URL`. `check:test`, `test:integration`,
    `test:tooling` and `test:e2e` use it, and
    it attaches to an existing box when `VIVARIUM_URL` is already set.
    `VIVARIUM_BIN` overrides the binary, e.g. for a local vivarium checkout.
    CI never runs vivarium as a service container or through Docker.
  - In vitest, use `@vivarium-dev/client/vitest` and the `viv` fixture
    (scoped and safe in parallel). Use `vivFresh` only when a test needs a
    pristine world.
  - Integration tests get the backend through `inject('serverUrl')`, or
    start their own with `spawnServer(env)` from `tests/support/server.ts`.
- Name every automated test after the test case it covers, e.g.
  `test('TC-3: a member can leave a session chat', …)`.
- Format test files (`pnpm lint:fix`) before freezing them: frozen files
  can never be reformatted afterwards.
