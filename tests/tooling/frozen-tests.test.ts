import { copyFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { afterEach, beforeEach, expect, test } from 'vitest'
import { run } from './run.ts'

const ROOT = resolve(import.meta.dirname, '../..')

let repo: string
let origin: string

const git = async (...args: string[]) => {
  const result = await run('git', args, { cwd: repo })
  if (result.code !== 0) throw new Error(`git ${args.join(' ')} failed:\n${result.output}`)
  return result.stdout.trim()
}

const write = (file: string, content: string) => {
  mkdirSync(join(repo, file, '..'), { recursive: true })
  writeFileSync(join(repo, file), content)
}

const featureFile = (testsCommit: string) =>
  `---\nstatus: implementing\nimpact: standalone\ndepends-on: []\nbranch: feature/demo\ntests-commit: ${testsCommit}\n---\n\n# Demo\n`

const commit = async (message: string) => {
  await git('add', '-A')
  await git('commit', '-q', '-m', message)
  return git('rev-parse', 'HEAD')
}

/** Runs the check the way CI and `pnpm check` do, with no PR context unless given. */
const check = (env: Record<string, string> = {}) => {
  const base = { ...process.env }
  delete base.GITHUB_HEAD_REF
  delete base.GITHUB_BASE_REF
  delete base.GITHUB_EVENT_NAME
  return run('bash', ['scripts/frozen-tests.sh'], { cwd: repo, env: { ...base, ...env } })
}

/** A feature branch whose frozen tests commit holds tests/demo.test.ts. */
const freezeTests = async () => {
  await git('switch', '-q', '-c', 'feature/demo')
  write('tests/demo.test.ts', "test('TC-1: demo', () => {})\n")
  const frozen = await commit('test(demo): red tests')
  write('features/demo.md', featureFile(frozen))
  await commit('docs(demo): record tests-commit')
  return frozen
}

beforeEach(async () => {
  const base = mkdtempSync(join(tmpdir(), 'frozen-tests-'))
  repo = join(base, 'repo')
  origin = join(base, 'origin.git')
  mkdirSync(repo)
  await run('git', ['init', '-q', '--bare', '-b', 'main', origin])
  await git('init', '-q', '-b', 'main')
  await git('config', 'user.name', 'Test')
  await git('config', 'user.email', 'test@example.com')
  await git('config', 'commit.gpgsign', 'false')
  mkdirSync(join(repo, 'scripts'))
  copyFileSync(join(ROOT, 'scripts/frozen-tests.sh'), join(repo, 'scripts/frozen-tests.sh'))
  copyFileSync(join(ROOT, 'scripts/check-tests-unchanged.sh'), join(repo, 'scripts/check-tests-unchanged.sh'))
  write('features/demo.md', featureFile(''))
  write('src/app.ts', 'export const app = 1\n')
  await commit('initial')
  await git('remote', 'add', 'origin', origin)
  await git('push', '-q', '-u', 'origin', 'main')
})

afterEach(() => {
  rmSync(join(repo, '..'), { recursive: true, force: true })
})

test('TC-18: branches that are not feature branches skip the check', async () => {
  await git('switch', '-q', '-c', 'chore/tidy')
  const result = await check()
  expect(result.code, result.output).toBe(0)
  expect(result.output).toMatch(/skipped/i)
  expect(result.output).toMatch(/not a feature branch/i)
})

test('TC-19: a feature branch with no frozen tests yet skips the check', async () => {
  await git('switch', '-q', '-c', 'feature/demo')
  write('src/app.ts', 'export const app = 2\n')
  await commit('work in progress')
  const result = await check()
  expect(result.code, result.output).toBe(0)
  expect(result.output).toMatch(/skipped/i)
  expect(result.output).toMatch(/no tests-commit/i)
})

test('TC-20: unchanged frozen tests pass', async () => {
  await freezeTests()
  write('src/app.ts', 'export const app = 3\n')
  await commit('feat(demo): implement')
  const result = await check()
  expect(result.code, result.output).toBe(0)
  expect(result.output).not.toMatch(/skipped/i)
})

test('TC-21: a changed frozen test fails the check', async () => {
  await freezeTests()

  // Changed but not committed.
  write('tests/demo.test.ts', "test('TC-1: demo', () => { expect(1).toBe(1) })\n")
  const uncommitted = await check()
  expect(uncommitted.code).not.toBe(0)
  expect(uncommitted.output).toContain('tests/demo.test.ts')

  // Changed and committed.
  await commit('loosen the test')
  const committed = await check()
  expect(committed.code).not.toBe(0)
  expect(committed.output).toContain('tests/demo.test.ts')
})

test('TC-22: erasing the freeze fails the check', async () => {
  await freezeTests()
  write('features/demo.md', featureFile(''))
  await commit('docs(demo): clear tests-commit')
  const result = await check()
  expect(result.code).not.toBe(0)
  expect(result.output).toMatch(/removed|blank|erased/i)
})

test('TC-23: re-pointing the freeze fails the check', async () => {
  const original = await freezeTests()
  write('tests/demo.test.ts', "test('TC-1: demo', () => { expect(true).toBe(true) })\n")
  const repointed = await commit('test(demo): rewrite tests')
  write('features/demo.md', featureFile(repointed))
  await commit('docs(demo): move tests-commit')
  const result = await check()
  expect(result.code).not.toBe(0)
  expect(result.output).toContain(original.slice(0, 7))
})

test("TC-24: a freeze outside the branch's history fails the check", async () => {
  await git('switch', '-q', '-c', 'elsewhere')
  write('tests/other.test.ts', "test('TC-9: other', () => {})\n")
  const outside = await commit('tests on another branch')
  await git('switch', '-q', 'main')
  await git('switch', '-q', '-c', 'feature/demo')
  write('features/demo.md', featureFile(outside))
  await commit('docs(demo): record tests-commit')
  const result = await check()
  expect(result.code).not.toBe(0)
  expect(result.output).toMatch(/ancestor|not in (the )?(branch'?s? )?history/i)
})

test('TC-25: in CI, the branch comes from the pull request', async () => {
  await freezeTests()
  write('tests/demo.test.ts', "test('TC-1: demo', () => { /* weakened */ })\n")
  await commit('loosen the test')
  // CI checks out the PR head commit, not a named branch.
  await git('checkout', '-q', '--detach', 'HEAD')

  const result = await check({ GITHUB_HEAD_REF: 'feature/demo', GITHUB_EVENT_NAME: 'pull_request' })
  expect(result.code).not.toBe(0)
  expect(result.output).toContain('tests/demo.test.ts')
})
