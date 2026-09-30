import { expect, test } from 'vitest'
import { type Json, checkName, ruleset, workflow } from './project-files.ts'

const REQUIRED = ['build', 'frozen-tests', 'lint', 'test']
const ADMIN_ROLE_ID = 5

const requiredContexts = (rs: Json): string[] =>
  rs.rules
    .filter((r: Json) => r.type === 'required_status_checks')
    .flatMap((r: Json) => r.parameters.required_status_checks.map((c: Json) => c.context))

test('TC-32: the ruleset requires the four checks on main', () => {
  const rs = ruleset()
  expect(rs.target).toBe('branch')
  expect(rs.enforcement).toBe('active')
  const include: string[] = rs.conditions.ref_name.include
  expect(include.some((ref) => ref === 'refs/heads/main' || ref === '~DEFAULT_BRANCH')).toBe(true)

  expect(requiredContexts(rs).sort()).toEqual(REQUIRED)
  expect(rs.rules.some((r: Json) => r.type === 'pull_request'), 'pull requests should not be required').toBe(false)

  const admins = rs.bypass_actors.filter(
    (a: Json) => a.actor_type === 'RepositoryRole' && a.actor_id === ADMIN_ROLE_ID,
  )
  expect(admins).toHaveLength(1)
  expect(admins[0].bypass_mode).toBe('always')
})

test('TC-33: the ruleset and the workflow agree on check names', () => {
  const jobs = Object.entries(workflow().jobs).map(([key, def]) => checkName(key, def))
  for (const context of requiredContexts(ruleset())) {
    expect(jobs, `required check "${context}" has no matching CI job`).toContain(context)
  }
})
