import { afterAll, beforeAll, expect, test, vi } from '@vivarium-dev/client/vitest'
import {
  Attendee,
  accountsIn,
  addAdmin,
  cli,
  cliOk,
  cliOkJson,
  type Deployment,
  deploy,
  memberAction,
  seedConference,
  setRole,
  sharedCode,
  uniqueCode,
} from '../support/conference.ts'

// Regressions from adversarial reviews of the signed decisions (design
// review round 5), from round 16 on. Each is named after the test case
// whose guarantee it covers by another path.

vi.setConfig({ testTimeout: 120_000, hookTimeout: 60_000 })

let dep: Deployment
beforeAll(async () => {
  dep = await deploy()
})
afterAll(async () => {
  await dep?.stop()
})

test('TC-52: staff can’t let someone an owner removed back in with a role (review round 16)', async ({ viv }) => {
  const { org, conference, admins } = await seedConference(dep, accountsIn(viv), { staff: ['pim'], methods: ['code'] })
  const kees = await viv.createAccount(viv.handle('kees'))
  await addAdmin(dep, org, kees, 'owner')
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const ana = await Attendee.signIn(dep, await viv.createAccount(viv.handle('ana')))
  expect((await ana.join({ conference: conference.space, code })).body.status).toBe('joined')
  expect((await memberAction(dep, conference, ['member', 'remove'], ana.person, { as: kees })).code).toBe(0)

  const role = await setRole(dep, conference, ana.person, 'speaker', { as: admins.pim })
  expect(role.code, 'a role would let her back in').not.toBe(0)
  expect(role.stderr).toMatch(/stands, and .* can.t override it/)
  expect((await ana.join({ conference: conference.space })).body.status).not.toBe('joined')
  expect(await ana.isMember(conference.space)).toBe(false)
})

test('TC-63: a key that standing records depend on alone isn’t removed without --force (review round 16)', async ({
  viv,
}) => {
  const { org, conference } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  const bram = await Attendee.signIn(dep, await viv.createAccount(viv.handle('bram')))
  expect((await memberAction(dep, conference, ['member', 'add'], bram.person)).code).toBe(0)
  await cliOkJson(dep, ['org', 'keys', 'add', '--org', org.did])

  // Not re-signed: Bram's admission depends on the first key alone.
  const refused = await cli(dep, ['org', 'keys', 'remove', 'eventside_attest', '--org', org.did])
  expect(refused.code).not.toBe(0)
  expect(refused.stderr).toMatch(/signed only with #eventside_attest/)
  expect(await bram.isMember(conference.space)).toBe(true)

  await cliOk(dep, ['org', 'keys', 'resign', '--org', org.did])
  await cliOk(dep, ['org', 'keys', 'remove', 'eventside_attest', '--org', org.did])
  expect(await bram.isMember(conference.space)).toBe(true)
})
