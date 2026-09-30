import { copyFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { afterEach, beforeEach, expect, test } from 'vitest'
import { run } from './run.ts'

// How scripts/frozen-tests.sh reads tests-commit from a feature file, beyond
// the frozen TC-18..25 cases: the forms features/README.md documents.

const ROOT = resolve(import.meta.dirname, '../..')
let repo: string

const git = async (...args: string[]) => {
  const result = await run('git', args, { cwd: repo })
  if (result.code !== 0) throw new Error(`git ${args.join(' ')} failed:\n${result.output}`)
  return result.stdout.trim()
}

const write = (file: string, content: string) => {
  mkdirSync(join(repo, file, '..'), { recursive: true })
  writeFileSync(join(repo, file), content)
}

const commit = async (message: string) => {
  await git('add', '-A')
  await git('commit', '-q', '-m', message)
  return git('rev-parse', 'HEAD')
}

const check = () => {
  const env = { ...process.env }
  delete env.GITHUB_HEAD_REF
  return run('bash', ['scripts/frozen-tests.sh'], { cwd: repo, env })
}

const feature = (frontmatterLine: string, body = '') =>
  `---\nstatus: implementing\n${frontmatterLine}\n---\n\n# Demo\n${body}`

beforeEach(async () => {
  repo = mkdtempSync(join(tmpdir(), 'frozen-format-'))
  await git('init', '-q', '-b', 'main')
  await git('config', 'user.name', 'Test')
  await git('config', 'user.email', 'test@example.com')
  await git('config', 'commit.gpgsign', 'false')
  mkdirSync(join(repo, 'scripts'))
  for (const script of ['frozen-tests.sh', 'check-tests-unchanged.sh']) {
    copyFileSync(join(ROOT, 'scripts', script), join(repo, 'scripts', script))
  }
  write('features/demo.md', feature('tests-commit:'))
  await commit('initial')
  await git('switch', '-q', '-c', 'feature/demo')
  write('tests/demo.test.ts', 'original\n')
})

afterEach(() => rmSync(repo, { recursive: true, force: true }))

test('a tests-commit with a trailing comment, as the README shows, is read as the SHA', async () => {
  const frozen = await commit('red tests')
  write('features/demo.md', feature(`tests-commit: ${frozen.slice(0, 7)}    # the approved red tests`))
  await commit('record')
  expect((await check()).code).toBe(0)

  write('tests/demo.test.ts', 'weakened\n')
  const changed = await check()
  expect(changed.code).not.toBe(0)
  expect(changed.output).toContain('tests/demo.test.ts')
})

test('a quoted tests-commit is read as the SHA', async () => {
  const frozen = await commit('red tests')
  write('features/demo.md', feature(`tests-commit: "${frozen}"`))
  await commit('record')
  const result = await check()
  expect(result.code, result.output).toBe(0)
  expect(result.output).not.toMatch(/skipped/i)
})

test('a tests-commit line outside the frontmatter is ignored', async () => {
  await commit('red tests')
  write('features/demo.md', feature('tests-commit:', '\n```yaml\ntests-commit: 3f0e1e9\n```\n'))
  await commit('docs with an example')
  const result = await check()
  expect(result.code, result.output).toBe(0)
  expect(result.output).toMatch(/no tests-commit/i)
})

test('a tests-commit that is not a SHA fails clearly', async () => {
  await commit('red tests')
  write('features/demo.md', feature('tests-commit: pending-approval'))
  await commit('record')
  const result = await check()
  expect(result.code).not.toBe(0)
  expect(result.output).toMatch(/isn't a commit SHA/)
})

test('a test file renamed in the frozen commit stays frozen', async () => {
  await commit('red tests')
  await git('mv', 'tests/demo.test.ts', 'tests/renamed.test.ts')
  const frozen = await commit('move the test into place')
  write('features/demo.md', feature(`tests-commit: ${frozen}`))
  await commit('record')
  write('tests/renamed.test.ts', 'weakened\n')
  const result = await check()
  expect(result.code).not.toBe(0)
  expect(result.output).toContain('tests/renamed.test.ts')
})
