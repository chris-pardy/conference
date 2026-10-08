import { afterAll, beforeAll, expect, test, vi } from '@vivarium-dev/client/vitest'
import {
  Attendee,
  accountsIn,
  cli,
  cliOk,
  type Deployment,
  deploy,
  memberAction,
  seedConference,
  sharedCode,
  uniqueCode,
} from '../support/conference.ts'

// Seeding an organization takes several CLI runs and OAuth round trips.
vi.setConfig({ testTimeout: 120_000, hookTimeout: 60_000 })

let dep: Deployment
beforeAll(async () => {
  dep = await deploy()
})
afterAll(async () => {
  await dep?.stop()
})

test('TC-58: An owner can undo a former admin’s admissions', async ({ viv }) => {
  const { org, conference, admins } = await seedConference(dep, accountsIn(viv), { staff: ['pim'], methods: ['code'] })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const bram = await Attendee.signIn(dep, await viv.createAccount(viv.handle('bram')))
  const ana = await Attendee.signIn(dep, await viv.createAccount(viv.handle('ana')))
  // Ana joined with a code, and Pim admitted her too; Bram has only Pim's say-so.
  expect((await ana.join({ conference: conference.space, code })).body.status).toBe('joined')
  expect((await memberAction(dep, conference, ['member', 'add'], bram.person, { as: admins.pim })).code).toBe(0)
  expect((await memberAction(dep, conference, ['member', 'add'], ana.person, { as: admins.pim })).code).toBe(0)
  await cliOk(dep, ['org', 'admin', 'remove', admins.pim.handle, '--org', org.did])
  expect(await bram.isMember(conference.space), 'Pim’s admission still stands (TC-53)').toBe(true)

  // Olga undoes Pim's admissions.
  const undone = await cli(dep, ['org', 'admin', 'undo', admins.pim.handle, '--org', org.did, '--admissions'])
  expect(undone.code, undone.stderr).toBe(0)

  expect(await bram.isMember(conference.space), 'Bram is no longer a member').toBe(false)
  expect(await ana.isMember(conference.space), 'Ana still is, through her code').toBe(true)
})
