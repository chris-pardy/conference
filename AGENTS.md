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
- **Commit identity.** Every commit's author and committer is
  `Chris Pardy <chris.pardy@gmail.com>`. This repo's git config already sets
  it, so don't override it with `-c`, `--author` or environment variables.
- **All green before any commit of implementation code.** That means the
  whole suite, not only the feature's own tests.

## Testing

- Use standard packages: **vitest** for unit and integration tests, and
  **Playwright** for UI flows and demo videos.
- Anything that talks to atproto (PLC, PDS, relay, Jetstream, Constellation,
  Slingshot, OAuth, spaces) runs against **vivarium**
  (`~/Code/atproto/vivarium`, [tangled.org/chris.pardy.family/vivarium](https://tangled.org/chris.pardy.family/vivarium)),
  never the live network. Start it with `--no-upstream` so tests are sealed.
  - In vitest, use `@vivarium/client/vitest`: add its global setup and use
    the `viv` fixture (scoped and safe in parallel). Use `vivFresh` only when
    a test needs a pristine world.
  - `VIVARIUM_URL` attaches to a running box and `VIVARIUM_BIN` picks the
    binary. In CI, run the `atcr.io/chris.pardy.family/vivarium` image as a
    service and set `VIVARIUM_URL`.
- Name every automated test after the test case it covers, e.g.
  `test('TC-3: a member can leave a session chat', …)`.
