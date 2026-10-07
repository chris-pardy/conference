import { afterAll, beforeAll, expect, test, vi } from '@vivarium-dev/client/vitest'
import { eventually, ownSpaceRecords } from '../support/atproto.ts'
import {
  Attendee,
  accountsIn,
  addAdmin,
  cli,
  type Deployment,
  deploy,
  memberAction,
  seedConference,
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

test('TC-58: An owner can keep the people a departing admin let in', async ({ viv }) => {
  const { org, conference, superAdmin, admins } = await seedConference(dep, accountsIn(viv), {
    staff: ['pim'],
    methods: ['code'],
  })
  const bram = await Attendee.signIn(dep, await viv.createAccount(viv.handle('bram')))
  expect((await memberAction(dep, conference, ['member', 'add'], bram.person, { as: admins.pim })).code).toBe(0)
  expect(await bram.isMember(conference.space)).toBe(true)

  // Olga removes Pim, and keeps Pim's admissions.
  const removed = await cli(dep, ['org', 'admin', 'remove', admins.pim.handle, '--org', org.did, '--keep-admissions'])
  expect(removed.code, removed.stderr).toBe(0)

  expect(await bram.isMember(conference.space), 'Bram is still a member').toBe(true)
  // Now on Olga's say-so: her own member record admits him.
  const hers = await ownSpaceRecords(viv.url, superAdmin, org.adminSpace, 'app.eventside.admin.member')
  expect(
    hers.some((r) => r.value.subject === bram.person.did && r.value.space === conference.space && !r.value.until),
    'Olga’s member record for Bram',
  ).toBe(true)

  // Without that choice, TC-53 applies.
  const lotte = await viv.createAccount(viv.handle('lotte'))
  await addAdmin(dep, org, lotte, 'staff')
  const cas = await Attendee.signIn(dep, await viv.createAccount(viv.handle('cas')))
  expect((await memberAction(dep, conference, ['member', 'add'], cas.person, { as: lotte })).code).toBe(0)
  expect(await cas.isMember(conference.space)).toBe(true)
  const plain = await cli(dep, ['org', 'admin', 'remove', lotte.handle, '--org', org.did])
  expect(plain.code, plain.stderr).toBe(0)
  await eventually(
    () => cas.isMember(conference.space),
    (member) => !member,
    'Cas stops being a member',
  )
  expect(await bram.isMember(conference.space), 'Bram is still in').toBe(true)
})
