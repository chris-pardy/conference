import { expect, test } from 'vitest'
import { type Json, allSteps, packageJson, readProjectFile, workflow } from './project-files.ts'

const CHECKS = { lint: 'check:lint', build: 'check:build', test: 'check:test', 'frozen-tests': 'check:frozen' }

test('TC-1: a fresh clone passes the full check (the check runs every step in order)', () => {
  // The full fresh-clone run is proven by CI itself, which starts from a clean
  // checkout with only Node, pnpm and Rust (TC-38). Here: the one command runs
  // lint, build, tests and the frozen-tests check, in that order.
  const check: string = packageJson().scripts?.check ?? ''
  const order = ['check:lint', 'check:build', 'check:test', 'check:frozen'].map((s) => check.indexOf(s))
  expect(order.every((i) => i >= 0), `"check" should run all four steps, got: ${check}`).toBe(true)
  expect([...order].sort((a, b) => a - b)).toEqual(order)
})

test('TC-26: CI runs on pull requests and on main', () => {
  const on = workflow().on
  expect(Object.keys(on).sort()).toEqual(['pull_request', 'push'])
  expect(on.pull_request.branches).toEqual(['main'])
  expect(on.push.branches).toEqual(['main'])
})

test('TC-27: CI has four checks, each running the local script', () => {
  const wf = workflow()
  const scripts = packageJson().scripts ?? {}
  expect(Object.keys(wf.jobs).sort()).toEqual(Object.keys(CHECKS).sort())

  for (const [job, script] of Object.entries(CHECKS)) {
    expect(scripts[script], `package.json should define "${script}"`).toBeTypeOf('string')
    const runs = (wf.jobs[job].steps ?? []).map((s: Json) => s.run ?? '').join('\n')
    expect(runs, `job "${job}" should run "pnpm ${script}"`).toMatch(new RegExp(`pnpm (run )?${script}\\b`))
  }
})

test('TC-28: a newer push cancels an older PR run, but main runs finish', () => {
  const concurrency = workflow().concurrency
  expect(concurrency.group).toContain('github.ref')
  expect(String(concurrency['cancel-in-progress'])).toMatch(/github\.event_name\s*==\s*'pull_request'/)
})

test('TC-29: failed browser tests leave evidence', () => {
  const uploads = allSteps(workflow()).filter(({ step }) => String(step.uses ?? '').startsWith('actions/upload-artifact'))
  expect(uploads.length).toBeGreaterThan(0)
  for (const { job, step } of uploads) {
    expect(job).toBe('test')
    expect(String(step.if ?? '')).toMatch(/failure\(\)/)
    expect(String(step.with?.path ?? '')).toContain('playwright-report')
    expect(String(step.with?.path ?? '')).toContain('test-results')
    expect(Number(step.with?.['retention-days'])).toBe(14)
  }
})

test("TC-30: the frozen-tests check sees the PR's own commits", () => {
  const steps = workflow().jobs['frozen-tests']?.steps ?? []
  const checkout = steps.find((s: Json) => String(s.uses ?? '').startsWith('actions/checkout'))
  expect(checkout, 'frozen-tests should check out the code').toBeDefined()
  expect(String(checkout.with?.ref ?? '')).toContain('github.event.pull_request.head.sha')
  expect(Number(checkout.with?.['fetch-depth'])).toBe(0)
})

test("TC-31: CI's vivarium is pinned and sealed", () => {
  const wf = workflow()
  for (const [job, def] of Object.entries(wf.jobs) as [string, Json][]) {
    expect(def.services, `job "${job}" should have no service containers`).toBeUndefined()
    expect(def.container, `job "${job}" should not run in a container`).toBeUndefined()
  }
  for (const { job, step } of allSteps(wf)) {
    expect(`${step.run ?? ''} ${step.uses ?? ''}`, `a step in "${job}" uses Docker`).not.toMatch(/docker/i)
  }

  const dev = packageJson().devDependencies ?? {}
  for (const pkg of ['@vivarium-dev/cli', '@vivarium-dev/client']) {
    expect(dev[pkg], `${pkg} should be an exact, pinned devDependency`).toMatch(/^\d+\.\d+\.\d+$/)
  }
  expect(readProjectFile('pnpm-lock.yaml')).toContain('@vivarium-dev/cli')

  // Every box the check starts is sealed.
  expect(readProjectFile('scripts/with-vivarium.ts')).toMatch(/upstream:\s*false/)
})
