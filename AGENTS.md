# Agent Instructions

Every change to this app is a feature, and every feature goes through the
pipeline below. Each feature has a file in [`features/`](features/) that
records where it stands. Its `status` field says which step comes next.

## The pipeline

| # | Step | Skill | Who | Status when done |
|---|------|-------|-----|------------------|
| 1 | Feature brainstorming | [`brainstorm-feature`](.claude/skills/brainstorm-feature/SKILL.md) | agent + human Q&A | `analysis` |
| 2 | Architecture analysis | [`analyze-architecture`](.claude/skills/analyze-architecture/SKILL.md) | agent | `design-review` or `test-cases` |
| 3 | Design review (cross-cutting features only) | [`design-review`](.claude/skills/design-review/SKILL.md) | agent + human, one or more rounds | `test-cases` |
| 4 | Test cases | [`write-test-cases`](.claude/skills/write-test-cases/SKILL.md) | agent writes, human approves | `tests` |
| 5–6 | Automated tests, shown red | [`write-tests`](.claude/skills/write-tests/SKILL.md) | agent writes, human approves | `implementing` |
| 7–9 | Implement → adversarial review → rework, repeated until clean | [`implement-feature`](.claude/skills/implement-feature/SKILL.md), [`adversarial-review`](.claude/skills/adversarial-review/SKILL.md) | agent, unsupervised | `pr` |
| 10–12 | PR + demo video → review gate → mark complete | [`ship-feature`](.claude/skills/ship-feature/SKILL.md) | agent; human only for architectural PRs | `complete` |

If you don't load skills automatically, read the linked `SKILL.md` for the
step before you start it.

Statuses, in order: `brainstorming`, `analysis`, `design-review`,
`test-cases`, `tests`, `implementing`, `pr`, `awaiting-human-review`,
`complete`. A feature that can't go on without a human decision is `blocked`,
and its file explains why.

## Human gates

Stop and wait for the human only at these points:

1. The end of brainstorming, when they confirm the feature description.
2. Each design review round.
3. Approval of the test cases.
4. Approval of the red automated tests.
5. Merging an architectural PR.

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
- **One branch per feature.** Name it `feature/<slug>` and create it at step 1.
  The feature file, tests and code all go on that branch and land together in
  the PR. Push after every commit. The only direct commit to `main` is
  marking a feature `complete` after its PR merges.
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

## Session start

Before starting new work, check for features in `awaiting-human-review`
whose PR has merged (`gh pr view <n> --json state`), and finish step 12 for
them (see `ship-feature`).
