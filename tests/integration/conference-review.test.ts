import { DatabaseSync } from 'node:sqlite'
import type { VivariumScope } from '@vivarium-dev/client'
import { afterAll, beforeAll, expect, test, vi } from '@vivarium-dev/client/vitest'
import {
  bringOnline,
  changeHandle,
  eventually,
  type Json,
  ownSpaceRecords,
  pdsOf,
  publicRecords,
  takeOffline,
  writeSpaceRecord,
  xrpcOk,
} from '../support/atproto.ts'
import {
  ATMOSPHERECONF,
  Attendee,
  accountsIn,
  addAdmin,
  allowApp,
  announcement,
  cli,
  cliOk,
  cliOkJson,
  createConference,
  createOrg,
  type Deployment,
  deploy,
  importList,
  NSID,
  plan,
  publicConference,
  seedConference,
  setRole,
  sharedCode,
  uniqueCode,
  WEDDING,
} from '../support/conference.ts'
import { OtherApp } from '../support/other-app.ts'

// Regressions from adversarial review round 1: each is named after the test
// case whose guarantee it covers by another path.

vi.setConfig({ testTimeout: 120_000, hookTimeout: 60_000 })

let dep: Deployment
let app: OtherApp
beforeAll(async () => {
  dep = await deploy()
  app = await OtherApp.start(process.env.VIVARIUM_URL as string)
})
afterAll(async () => {
  await app?.stop()
  await dep?.stop()
})

const sorted = (members: Json[]) => [...members].sort((a, b) => a.did.localeCompare(b.did))

test('TC-51: staff can’t give the owner role (review round 1)', async ({ viv }) => {
  const { conference, admins } = await seedConference(dep, accountsIn(viv), { staff: ['pim'], methods: ['code'] })
  const bram = await viv.createAccount(viv.handle('bram'))

  const refused = await setRole(dep, conference, bram, 'owner', { as: admins.pim })
  expect(refused.code).not.toBe(0)
  expect(refused.stderr).toMatch(/only owners/i)
  // Bram has no way in: the refused role didn't become one.
  const asBram = await Attendee.signIn(dep, bram)
  expect((await asBram.join({ conference: conference.space })).body.status).not.toBe('joined')
})

test('TC-53: a role a former admin gave stops admitting (review round 1)', async ({ viv }) => {
  const { org, conference, admins } = await seedConference(dep, accountsIn(viv), { staff: ['pim'], methods: ['code'] })
  const bram = await viv.createAccount(viv.handle('bram'))
  const given = await setRole(dep, conference, bram, 'speaker', { as: admins.pim })
  expect(given.code, given.stderr).toBe(0)
  const asBram = await Attendee.signIn(dep, bram)
  expect((await asBram.join({ conference: conference.space })).body.status, 'admitted by the role').toBe('joined')

  await cliOk(dep, ['org', 'admin', 'remove', admins.pim.handle, '--org', org.did])

  await eventually(
    () => asBram.isMember(conference.space),
    (member) => !member,
    'Bram stops being a member',
  )
})

test('TC-36: taking an app off a space’s list revokes its access (review round 1)', async ({ viv }) => {
  const { conference } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  await allowApp(dep, conference, app.clientId)
  const ana = await viv.createAccount(viv.handle('ana'))
  expect((await (await Attendee.signIn(dep, ana)).join({ conference: conference.space, code })).body.status).toBe(
    'joined',
  )
  await writeSpaceRecord(viv.url, ana, conference.space, NSID.event, plan('Borrel at Hannekes Boom'))
  const access = await (await app.signIn(ana)).open(conference.space)
  await eventually(
    () => access.listRecords(ana.did, NSID.event),
    (answer) => answer.status === 200,
    'the app reads Ana’s plan',
  )

  await cliOk(dep, ['apps', 'remove', app.clientId, '--conference', conference.space])

  // Our host refuses it at once, and Ana's PDS once the revocation lands.
  const host = await access.hostCall('app.eventside.space.listMembers')
  expect(host.status).toBe(401)
  expect(host.body.error).toBe('CredentialRevoked')
  const pds = await eventually(
    () => access.listRecords(ana.did, NSID.event),
    (answer) => answer.status !== 200,
    'the app’s credential is revoked at Ana’s PDS',
  )
  expect(pds.body.error).toBe('CredentialRevoked')
})

test('TC-50: reindex rebuilds a wiped index, conferences included (review round 1)', async ({ viv }) => {
  const { org, conference, superAdmin } = await seedConference(dep, accountsIn(viv), {
    staff: ['pim'],
    methods: ['code'],
  })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'), { maxUses: 5 })
  await allowApp(dep, conference, app.clientId)
  const ana = await Attendee.signIn(dep, await viv.createAccount(viv.handle('ana')))
  expect((await ana.join({ conference: conference.space, code })).body.status).toBe('joined')

  const snapshot = async () => {
    const access = await (await app.signIn(superAdmin)).open(conference.space)
    return {
      members: sorted(await access.members()),
      space: await access.getSpace(),
      org: await cliOkJson(dep, ['org', 'show', '--org', org.did]),
      page: (await publicConference(dep, conference.event as string)).body,
    }
  }
  const before = await snapshot()

  // What an operator restoring from a backup with only the kept state has.
  const db = new DatabaseSync(dep.databasePath as string)
  try {
    for (const table of ['space_records', 'space_repos', 'conferences']) db.exec(`DELETE FROM ${table}`)
  } finally {
    db.close()
  }
  expect((await publicConference(dep, conference.event as string)).status, 'the page is gone with the index').toBe(404)

  await cliOk(dep, ['reindex', '--org', org.did])

  expect(await snapshot()).toEqual(before)
  expect(await ana.isMember(conference.space)).toBe(true)
})

test('TC-7: an invite-only conference doesn’t exist for non-members anywhere (review round 1)', async ({ viv }) => {
  const sanne = await viv.createAccount(viv.handle('sanne'))
  const bruiloft = await createOrg(dep, sanne)
  const wedding = await createConference(dep, bruiloft, { ...WEDDING, methods: ['code', 'request'] })
  const mallory = await Attendee.signIn(dep, await viv.createAccount(viv.handle('mallory')))

  expect((await mallory.listRecords(wedding.space, NSID.event)).status).toBe(404)
  expect((await mallory.join({ conference: wedding.space })).status).toBe(404)
  expect((await mallory.join({ conference: wedding.space, code: 'guess' })).status).toBe(404)
  expect((await mallory.leave(wedding.space)).status).toBe(404)
})

test('TC-4: a conference’s super admin must be an admin (review round 1)', async ({ viv }) => {
  const olga = await viv.createAccount(viv.handle('olga'))
  const bram = await viv.createAccount(viv.handle('bram'))
  const org = await createOrg(dep, olga)
  const refused = await cli(dep, [
    'conference',
    'create',
    '--org',
    org.did,
    '--name',
    'AtmosphereConf',
    '--starts',
    '2027-04-29T09:00:00+02:00',
    '--ends',
    '2027-05-02T18:00:00+02:00',
    '--city',
    'Amsterdam',
    '--super-admin',
    bram.handle,
  ])
  expect(refused.code).not.toBe(0)
  expect(refused.stderr).toMatch(/isn.t an admin/)
})

test('TC-26: an admin can’t leave (review round 1)', async ({ viv }) => {
  const { conference, admins } = await seedConference(dep, accountsIn(viv), { staff: ['pim'], methods: ['code'] })
  const pim = await Attendee.signIn(dep, admins.pim)
  expect(await pim.isMember(conference.space)).toBe(true)
  const left = await pim.leave(conference.space)
  expect(left.status).toBe(400)
  expect(left.body.error).toBe('AdminCannotLeave')
  expect(await pim.isMember(conference.space)).toBe(true)
})

// Regressions from adversarial review round 2.

test('TC-29: an admin’s records outlive a change of role and their removal (review round 2)', async ({ viv }) => {
  const { org, conference, superAdmin, admins } = await seedConference(dep, accountsIn(viv), {
    staff: ['pim'],
    methods: ['code'],
  })
  await allowApp(dep, conference, app.clientId)
  await writeSpaceRecord(viv.url, admins.pim, conference.space, NSID.event, plan('Pim’s walking tour'))
  const asOlga = await Attendee.signIn(dep, superAdmin)
  const pimsPlans = async () => {
    const answer = await asOlga.listRecords(conference.space, NSID.event)
    expect(answer.status).toBe(200)
    return (answer.body.records as Json[]).filter((r) => r.author === admins.pim.did)
  }
  await eventually(pimsPlans, (records) => records.length === 1, 'Pim’s plan is listed')

  await cliOk(dep, ['org', 'admin', 'add', admins.pim.handle, '--org', org.did, '--role', 'owner'])
  expect(await pimsPlans(), 'still listed once Pim is an owner').toHaveLength(1)
  await cliOk(dep, ['org', 'admin', 'remove', admins.pim.handle, '--org', org.did])
  expect(await pimsPlans(), 'still listed once Pim isn’t an admin').toHaveLength(1)

  const access = await (await app.signIn(superAdmin)).open(conference.space)
  const pim = (await access.members()).find((m: Json) => m.did === admins.pim.did)
  expect(pim?.periods).toHaveLength(1)
  expect(pim?.periods[0].until, 'a past member, not forgotten').toBeDefined()
})

test('TC-51: admins can’t be removed or banned from a conference (review round 2)', async ({ viv }) => {
  const { conference, superAdmin, admins } = await seedConference(dep, accountsIn(viv), {
    owners: ['kees'],
    staff: ['pim'],
    methods: ['code'],
  })
  const ban = await cli(dep, [
    'member',
    'ban',
    superAdmin.handle,
    '--conference',
    conference.space,
    '--as',
    admins.kees.handle,
  ])
  expect(ban.code).not.toBe(0)
  expect(ban.stderr).toMatch(/is an admin/)
  const remove = await cli(dep, ['member', 'remove', admins.kees.handle, '--conference', conference.space])
  expect(remove.code).not.toBe(0)
  expect(await (await Attendee.signIn(dep, superAdmin)).isMember(conference.space)).toBe(true)
})

test('TC-4: a conference’s super admin can’t stop being an admin (review round 2)', async ({ viv }) => {
  const olga = await viv.createAccount(viv.handle('olga'))
  const pim = await viv.createAccount(viv.handle('pim'))
  const org = await createOrg(dep, olga)
  await addAdmin(dep, org, pim, 'owner')
  await createConference(dep, org, { ...ATMOSPHERECONF, superAdmin: pim })
  const refused = await cli(dep, ['org', 'admin', 'remove', pim.handle, '--org', org.did])
  expect(refused.code).not.toBe(0)
  expect(refused.stderr).toMatch(/super admin of the conference/)
})

// Regressions from adversarial review round 3.

test('TC-51: staff can’t undo the super admin’s removal (review round 3)', async ({ viv }) => {
  const { conference, admins } = await seedConference(dep, accountsIn(viv), { staff: ['pim'], methods: ['code'] })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const bram = await Attendee.signIn(dep, await viv.createAccount(viv.handle('bram')))
  expect((await bram.join({ conference: conference.space, code })).body.status).toBe('joined')
  await cliOk(dep, ['member', 'remove', bram.person.handle, '--conference', conference.space])

  const undo = await cli(dep, [
    'member',
    'add',
    bram.person.handle,
    '--conference',
    conference.space,
    '--as',
    admins.pim.handle,
  ])
  expect(undo.code).not.toBe(0)
  expect(undo.stderr).toMatch(/super admin.s latest decision/)
  expect(await bram.isMember(conference.space)).toBe(false)
})

test('TC-51: the super admin stays an owner (review round 3)', async ({ viv }) => {
  const { org, superAdmin } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  const refused = await cli(dep, ['org', 'admin', 'add', superAdmin.handle, '--org', org.did, '--role', 'staff'])
  expect(refused.code).not.toBe(0)
  expect(refused.stderr).toMatch(/always an owner/)
  const shown = await cliOkJson(dep, ['org', 'show', '--org', org.did])
  expect(shown.admins).toContainEqual(expect.objectContaining({ did: superAdmin.did, role: 'owner' }))
})

test('TC-34: admin-only records are judged at the time they were written (review round 3)', async ({ viv }) => {
  const { org, conference, superAdmin, admins } = await seedConference(dep, accountsIn(viv), {
    staff: ['pim'],
    methods: ['code'],
  })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const anaAccount = await viv.createAccount(viv.handle('ana'))
  expect(
    (await (await Attendee.signIn(dep, anaAccount)).join({ conference: conference.space, code })).body.status,
  ).toBe('joined')
  await writeSpaceRecord(viv.url, admins.pim, conference.space, NSID.announcement, announcement('Pim: lunch at 12'))
  await writeSpaceRecord(viv.url, anaAccount, conference.space, NSID.announcement, announcement('Ana: before staff'))
  const asOlga = await Attendee.signIn(dep, superAdmin)
  const texts = async () =>
    ((await asOlga.listRecords(conference.space, NSID.announcement)).body.records as Json[]).map((r) => r.value.text)
  await eventually(texts, (listed) => listed.includes('Pim: lunch at 12'), 'Pim’s announcement is listed')

  await cliOk(dep, ['member', 'role', anaAccount.handle, '--role', 'staff', '--conference', conference.space])
  await cliOk(dep, ['org', 'admin', 'remove', admins.pim.handle, '--org', org.did])

  const listed = await texts()
  expect(listed, 'written while Pim was an admin').toContain('Pim: lunch at 12')
  expect(listed, 'written before Ana was staff').not.toContain('Ana: before staff')
})

test('TC-4: a conference can be handed to another super admin (review round 3)', async ({ viv }) => {
  const olga = await viv.createAccount(viv.handle('olga'))
  const pim = await viv.createAccount(viv.handle('pim'))
  const org = await createOrg(dep, olga)
  await addAdmin(dep, org, pim, 'owner')
  const conference = await createConference(dep, org, { ...ATMOSPHERECONF, superAdmin: pim, methods: ['code'] })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const ana = await viv.createAccount(viv.handle('ana'))
  const asAna = await Attendee.signIn(dep, ana)
  expect((await asAna.join({ conference: conference.space, code })).body.status).toBe('joined')
  await setRole(dep, conference, ana, 'speaker')

  const handed = await cliOkJson(dep, ['conference', 'super-admin', olga.handle, '--conference', conference.space])
  expect(handed.superAdmin).toBe(olga.did)
  await cliOk(dep, ['org', 'admin', 'remove', pim.handle, '--org', org.did])

  const page = await asAna.getConference(conference.space)
  expect(page.body.viewer, 'Ana keeps her role').toEqual(expect.objectContaining({ member: true, role: 'speaker' }))
  const event = await publicConference(dep, conference.event as string)
  expect(event.status).toBe(200)
})

test('TC-31: a role on the list comes with a handle that resolves only later (review round 3)', async ({ viv }) => {
  const { conference } = await seedConference(dep, accountsIn(viv), { methods: ['list'] })
  const zoeHandle = viv.handle('zoe')
  await importList(dep, conference, [{ handle: zoeHandle, role: 'speaker' }])
  const zoe = await viv.createAccount(zoeHandle)

  const answer = await (await Attendee.signIn(dep, zoe)).join({ conference: conference.space })
  expect(answer.body.status).toBe('joined')
  expect(answer.body.role).toBe('speaker')
})

// Regressions from adversarial review round 4.

test('TC-51: staff can’t strip a role by a removal the super admin’s decision overrides (review round 4)', async ({
  viv,
}) => {
  const { conference, admins } = await seedConference(dep, accountsIn(viv), { staff: ['pim'], methods: ['code'] })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const bram = await Attendee.signIn(dep, await viv.createAccount(viv.handle('bram')))
  expect((await bram.join({ conference: conference.space, code })).body.status).toBe('joined')
  await cliOk(dep, ['member', 'add', bram.person.handle, '--conference', conference.space])
  await setRole(dep, conference, bram.person, 'speaker')

  const refused = await cli(dep, [
    'member',
    'remove',
    bram.person.handle,
    '--conference',
    conference.space,
    '--as',
    admins.pim.handle,
  ])
  expect(refused.code).not.toBe(0)
  expect(refused.stderr).toMatch(/super admin.s latest decision/)
  const page = await bram.getConference(conference.space)
  expect(page.body.viewer, 'Bram keeps his role').toEqual(expect.objectContaining({ member: true, role: 'speaker' }))
})

test('TC-4: a public conference can be handed over twice (review round 4)', async ({ viv }) => {
  const olga = await viv.createAccount(viv.handle('olga'))
  const pim = await viv.createAccount(viv.handle('pim'))
  const kees = await viv.createAccount(viv.handle('kees'))
  const org = await createOrg(dep, olga)
  await addAdmin(dep, org, pim, 'owner')
  await addAdmin(dep, org, kees, 'owner')
  const conference = await createConference(dep, org, { ...ATMOSPHERECONF, superAdmin: pim, methods: ['code'] })

  await cliOk(dep, ['conference', 'super-admin', kees.handle, '--conference', conference.space])
  const handed = await cliOkJson(dep, ['conference', 'super-admin', olga.handle, '--conference', conference.space])
  expect(handed.superAdmin).toBe(olga.did)
  const again = await cliOkJson(dep, ['conference', 'super-admin', olga.handle, '--conference', conference.space])
  expect(again.superAdmin).toBe(olga.did)

  const sidecars = await publicRecords(viv.url, pim.did, 'app.eventside.conference')
  expect(sidecars.map((r) => r.value.superAdmin)).toEqual([olga.did])
  expect((await publicConference(dep, conference.event as string)).status).toBe(200)
})

test('TC-31: a role on the list comes with a handle bound after a code let them in (review round 4)', async ({
  viv,
}) => {
  const { conference } = await seedConference(dep, accountsIn(viv), { methods: ['list', 'code'] })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const zoeHandle = viv.handle('zoe')
  await importList(dep, conference, [{ handle: zoeHandle, role: 'speaker' }])
  const zoe = await viv.createAccount(zoeHandle)

  const answer = await (await Attendee.signIn(dep, zoe)).join({ conference: conference.space, code })
  expect(answer.body.status).toBe('joined')
  expect(answer.body.role).toBe('speaker')
})

test('TC-4: a conference’s super admin must be an owner (review round 4)', async ({ viv }) => {
  const olga = await viv.createAccount(viv.handle('olga'))
  const pim = await viv.createAccount(viv.handle('pim'))
  const kees = await viv.createAccount(viv.handle('kees'))
  const org = await createOrg(dep, olga)
  await addAdmin(dep, org, pim, 'staff')
  await addAdmin(dep, org, kees, 'owner')
  const conference = await createConference(dep, org, { ...ATMOSPHERECONF, superAdmin: kees })

  const handover = await cli(dep, ['conference', 'super-admin', pim.handle, '--conference', conference.space])
  expect(handover.code).not.toBe(0)
  expect(handover.stderr).toMatch(/must be an owner/)
  const demoted = await cli(dep, ['org', 'admin', 'add', kees.handle, '--org', org.did, '--role', 'staff'])
  expect(demoted.code).not.toBe(0)
  expect(demoted.stderr).toMatch(/super admin of the conference/)
})

test('TC-8: the super admin counts as an owner without an admin record (review round 4)', async ({ viv }) => {
  const { org, superAdmin, admins } = await seedConference(dep, accountsIn(viv), { owners: ['kees'] })
  await cliOk(dep, ['org', 'admin', 'remove', superAdmin.handle, '--org', org.did])
  await cliOk(dep, ['org', 'admin', 'remove', admins.kees.handle, '--org', org.did])
  const shown = await cliOkJson(dep, ['org', 'show', '--org', org.did])
  expect(shown.admins).not.toContainEqual(expect.objectContaining({ did: admins.kees.did }))
})

// Regressions from adversarial review round 5.

test('TC-4: a handover moves the super admin’s records, not her plans (review round 5)', async ({ viv }) => {
  const olga = await viv.createAccount(viv.handle('olga'))
  const pim = await viv.createAccount(viv.handle('pim'))
  const org = await createOrg(dep, olga)
  await addAdmin(dep, org, pim, 'owner')
  const conference = await createConference(dep, org, { ...ATMOSPHERECONF, superAdmin: pim, methods: ['code'] })
  await writeSpaceRecord(viv.url, pim, conference.space, NSID.event, plan('Pim’s canal tour'))
  const asOlga = await Attendee.signIn(dep, olga)
  await eventually(
    () => asOlga.listRecords(conference.space, NSID.event),
    (answer) => (answer.body.records ?? []).some((r: Json) => r.author === pim.did),
    'Pim’s plan is listed',
  )

  await cliOk(dep, ['conference', 'super-admin', olga.handle, '--conference', conference.space])

  const pimsPlans = await ownSpaceRecords(viv.url, pim, conference.space, NSID.event)
  expect(
    pimsPlans.map((r) => r.value.name),
    'Pim keeps his plan',
  ).toEqual(['Pim’s canal tour'])
  expect(await ownSpaceRecords(viv.url, olga, conference.space, NSID.event), 'and Olga doesn’t get it').toEqual([])
  expect(await ownSpaceRecords(viv.url, pim, conference.space, NSID.role), 'Pim’s roles are moved').toEqual([])
  expect((await ownSpaceRecords(viv.url, olga, conference.space, NSID.role)).length).toBeGreaterThan(0)
})

test('TC-4: an invite-only handover moves the event, not the super admin’s plans (review round 5)', async ({ viv }) => {
  const sanne = await viv.createAccount(viv.handle('sanne'))
  const joost = await viv.createAccount(viv.handle('joost'))
  const bruiloft = await createOrg(dep, sanne)
  await addAdmin(dep, bruiloft, joost, 'owner')
  const wedding = await createConference(dep, bruiloft, { ...WEDDING, methods: ['code'] })
  await writeSpaceRecord(viv.url, sanne, wedding.space, NSID.event, plan('Sanne’s dress fitting'))
  const asSanne = await Attendee.signIn(dep, sanne)
  await eventually(
    () => asSanne.listRecords(wedding.space, NSID.event),
    (answer) => (answer.body.records ?? []).some((r: Json) => r.value?.name === 'Sanne’s dress fitting'),
    'Sanne’s plan is listed',
  )

  await cliOk(dep, ['conference', 'super-admin', joost.handle, '--conference', wedding.space])

  const names = async (who: typeof sanne) =>
    (await ownSpaceRecords(viv.url, who, wedding.space, NSID.event)).map((r) => r.value.name)
  expect(await names(sanne), 'Sanne keeps her plan, and only it').toEqual(['Sanne’s dress fitting'])
  expect(await names(joost), 'Joost has the wedding’s event').toEqual([WEDDING.name])
  const page = await (await Attendee.signIn(dep, joost)).getConference(wedding.space)
  expect(page.status).toBe(200)
  expect(JSON.stringify(page.body)).toContain(WEDDING.name)
})

test('TC-54: a late list handle doesn’t make a code join depend on the super admin’s PDS (review round 5)', async ({
  viv,
}) => {
  const { conference, superAdmin } = await seedConference(dep, accountsIn(viv), { methods: ['list', 'code'] })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const zoeHandle = viv.handle('zoe')
  await importList(dep, conference, [{ handle: zoeHandle, role: 'speaker' }])
  const zoe = await Attendee.signIn(dep, await viv.createAccount(zoeHandle))

  await takeOffline(viv.url, superAdmin)
  try {
    const answer = await zoe.join({ conference: conference.space, code })
    expect(answer.status).toBe(200)
    expect(answer.body.status, 'the code lets her in').toBe('joined')
  } finally {
    await bringOnline(viv.url, superAdmin)
  }
  // Once the super admin's PDS is back, a later join binds the handle.
  const again = await zoe.join({ conference: conference.space })
  expect(again.body.status).toBe('joined')
  expect(again.body.role).toBe('speaker')
})

test('TC-17: a list handle is bound to whoever a role let in (review round 5)', async ({ viv }) => {
  const { conference } = await seedConference(dep, accountsIn(viv), { methods: ['list'] })
  const zoeHandle = viv.handle('zoe')
  await importList(dep, conference, [{ handle: zoeHandle }])
  const zoe = await viv.createAccount(zoeHandle)
  await setRole(dep, conference, zoe, 'speaker')
  expect((await (await Attendee.signIn(dep, zoe)).join({ conference: conference.space })).body.status).toBe('joined')

  // Zoe moves on to another handle, and someone else takes hers.
  await changeHandle(viv.url, zoe, viv.handle('zoe-new'))
  const next = await viv.createAccount(zoeHandle)
  const answer = await (await Attendee.signIn(dep, next)).join({ conference: conference.space })
  expect(answer.body.status, 'the row stayed with Zoe').not.toBe('joined')
})

test('TC-11: a code alone finds its conference among several organizations (review round 5)', async ({ viv }) => {
  const first = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  const second = await seedConference(dep, accountsIn(viv), { superAdmin: 'kees', methods: ['code'] })
  await sharedCode(dep, first.conference, uniqueCode('atmosphere27'))
  const code = await sharedCode(dep, second.conference, uniqueCode('atmosphere27'))
  const ana = await Attendee.signIn(dep, await viv.createAccount(viv.handle('ana')))

  const wrong = await ana.join({ code: uniqueCode('nope') })
  expect(wrong.status).toBe(400)
  expect(wrong.body.error).toBe('InvalidCode')

  // An index made before codes were looked up by HMAC: reindex fills it in.
  const db = new DatabaseSync(dep.databasePath as string)
  try {
    db.prepare('UPDATE space_records SET code_hmac = NULL WHERE space LIKE ?').run(`at://${second.org.did}/%`)
  } finally {
    db.close()
  }
  await cliOk(dep, ['reindex', '--org', second.org.did])

  const answer = await ana.join({ code })
  expect(answer.body.status).toBe('joined')
  expect(answer.body.conference).toBe(second.conference.space)
})

// Regressions from adversarial review round 6.

test('TC-31: a list role that couldn’t be given is given on a later join (review round 6)', async ({ viv }) => {
  const olga = await viv.createAccount(viv.handle('olga'))
  const pim = await viv.createAccount(viv.handle('pim'))
  const org = await createOrg(dep, olga)
  await addAdmin(dep, org, pim, 'owner')
  const conference = await createConference(dep, org, {
    ...ATMOSPHERECONF,
    superAdmin: pim,
    methods: ['list', 'code'],
  })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const zoeHandle = viv.handle('zoe')
  await importList(dep, conference, [{ handle: zoeHandle, role: 'speaker' }])
  const zoe = await Attendee.signIn(dep, await viv.createAccount(zoeHandle))

  // Pim, who writes the conference's roles, is down: the code lets Zoe in.
  await takeOffline(viv.url, pim)
  try {
    expect((await zoe.join({ conference: conference.space, code })).body.status).toBe('joined')
  } finally {
    await bringOnline(viv.url, pim)
  }
  const again = await zoe.join({ conference: conference.space })
  expect(again.body.status).toBe('joined')
  expect(again.body.role, 'the row’s role, once Pim is back').toBe('speaker')
})

// Regressions from adversarial review round 7.

test('TC-31: a list role given without its binding still lets the person in, and only them (review round 7)', async ({
  viv,
}) => {
  const olga = await viv.createAccount(viv.handle('olga'))
  const pim = await viv.createAccount(viv.handle('pim'))
  const org = await createOrg(dep, olga)
  await addAdmin(dep, org, pim, 'owner')
  const conference = await createConference(dep, org, { ...ATMOSPHERECONF, superAdmin: pim, methods: ['list'] })
  const zoeHandle = viv.handle('zoe')
  await importList(dep, conference, [{ handle: zoeHandle, role: 'speaker' }])
  const zoeAccount = await viv.createAccount(zoeHandle)
  const zoe = await Attendee.signIn(dep, zoeAccount)

  // Pim gives the row's role, but Olga, who writes the binding, is down.
  await takeOffline(viv.url, olga)
  try {
    const answer = await zoe.join({ conference: conference.space })
    expect(answer.status).toBe(200)
    expect(answer.body.status, 'the role lets her in').toBe('joined')
    expect(answer.body.role).toBe('speaker')
  } finally {
    await bringOnline(viv.url, olga)
  }

  // The row is Zoe's: the handle's next holder isn't let in by it.
  await changeHandle(viv.url, zoeAccount, viv.handle('zoe-new'))
  const next = await viv.createAccount(zoeHandle)
  const answer = await (await Attendee.signIn(dep, next)).join({ conference: conference.space })
  expect(answer.body.status, 'the row stayed with Zoe').not.toBe('joined')
})

test('TC-7: a bad cursor or limit doesn’t reveal an invite-only conference (review round 7)', async ({ viv }) => {
  const sanne = await viv.createAccount(viv.handle('sanne'))
  const bruiloft = await createOrg(dep, sanne)
  const wedding = await createConference(dep, bruiloft, { ...WEDDING, methods: ['code', 'request'] })
  const mallory = await Attendee.signIn(dep, await viv.createAccount(viv.handle('mallory')))
  const list = (who: Attendee, params: Record<string, string>) =>
    who.call('app.eventside.conference.listRecords', {
      params: { conference: wedding.space, collection: NSID.event, ...params },
    })

  expect((await list(mallory, { cursor: 'not a cursor' })).status).toBe(404)
  expect((await list(mallory, { limit: 'ten' })).status).toBe(404)

  // A member is told what's wrong, in the XRPC shape.
  const asSanne = await Attendee.signIn(dep, sanne)
  const badCursor = await list(asSanne, { cursor: 'not a cursor' })
  expect(badCursor.status).toBe(400)
  expect(badCursor.body.error).toBe('InvalidRequest')
  const badLimit = await list(asSanne, { limit: 'ten' })
  expect(badLimit.status).toBe(400)
  expect(badLimit.body.error).toBe('InvalidRequest')
})

test('TC-48: junk written to an intake space doesn’t change the index (review round 7)', async ({ viv }) => {
  const { org, conference } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  const mallory = await viv.createAccount(viv.handle('mallory'))
  const db = new DatabaseSync(dep.databasePath as string)
  try {
    const generation = () =>
      (db.prepare('SELECT generation FROM index_generations WHERE org = ?').get(org.did) as { generation: number })
        .generation
    const read = () =>
      db.prepare('SELECT synced_rev FROM space_repos WHERE space = ? AND repo = ?').get(conference.intake, mallory.did)
    const before = generation()

    for (const n of [1, 2, 3]) {
      await writeSpaceRecord(viv.url, mallory, conference.intake, 'com.example.junk', { n, pad: 'x'.repeat(1000) })
    }
    await eventually(
      async () => read(),
      (row) => row !== undefined,
      'our host read Mallory’s repo in the intake space',
    )
    await new Promise((done) => setTimeout(done, 1_500))

    expect(generation(), 'nothing was stored, so nothing is re-derived').toBe(before)
    expect(
      db
        .prepare('SELECT COUNT(*) AS n FROM space_records WHERE space = ? AND repo = ?')
        .get(conference.intake, mallory.did),
    ).toEqual({ n: 0 })
  } finally {
    db.close()
  }
})

// Regressions from adversarial review round 8.

test('TC-32: a member’s plan leaves the index’s view alone; a role change re-derives it (review round 8)', async ({
  viv,
}) => {
  const { org, conference, superAdmin } = await seedConference(dep, accountsIn(viv))
  const ana = await viv.createAccount(viv.handle('ana'))
  const db = new DatabaseSync(dep.databasePath as string)
  try {
    const generation = () =>
      (db.prepare('SELECT generation FROM index_generations WHERE org = ?').get(org.did) as { generation: number })
        .generation
    const plans = () =>
      db
        .prepare('SELECT COUNT(*) AS n FROM space_records WHERE space = ? AND repo = ? AND collection = ?')
        .get(conference.space, superAdmin.did, NSID.event) as { n: number }
    // Whatever the seeding set off has settled.
    await new Promise((done) => setTimeout(done, 1_500))
    const before = generation()

    await writeSpaceRecord(viv.url, superAdmin, conference.space, NSID.event, plan('Olga’s canal tour'))
    await eventually(
      async () => plans(),
      (row) => row.n === 1,
      'our host stored the plan',
    )
    await new Promise((done) => setTimeout(done, 1_500))
    expect(generation(), 'no one’s access depends on a plan').toBe(before)

    expect((await setRole(dep, conference, ana, 'speaker')).code).toBe(0)
    expect(generation(), 'a role is re-derived').toBeGreaterThan(before)
  } finally {
    db.close()
  }
})

for (const role of ['staff', 'none'] as const) {
  test(`TC-17: a list row a role holds stays with its holder when the role becomes ${role} (review round 8)`, async ({
    viv,
  }) => {
    const olga = await viv.createAccount(viv.handle('olga'))
    const pim = await viv.createAccount(viv.handle('pim'))
    const org = await createOrg(dep, olga)
    await addAdmin(dep, org, pim, 'owner')
    const conference = await createConference(dep, org, { ...ATMOSPHERECONF, superAdmin: pim, methods: ['list'] })
    const zoeHandle = viv.handle('zoe')
    await importList(dep, conference, [{ handle: zoeHandle, role: 'speaker' }])
    const zoeAccount = await viv.createAccount(zoeHandle)
    const zoe = await Attendee.signIn(dep, zoeAccount)

    // The row's role is given, but not its binding: the role holds the row.
    await takeOffline(viv.url, olga)
    try {
      expect((await zoe.join({ conference: conference.space })).body.status).toBe('joined')
    } finally {
      await bringOnline(viv.url, olga)
    }

    expect((await setRole(dep, conference, zoeAccount, role)).code).toBe(0)

    // The row is still Zoe's: she stays in, and the handle's next holder
    // isn't let in by it.
    await changeHandle(viv.url, zoeAccount, viv.handle('zoe-new'))
    const next = await viv.createAccount(zoeHandle)
    const answer = await (await Attendee.signIn(dep, next)).join({ conference: conference.space })
    expect(answer.body.status, 'the row stayed with Zoe').not.toBe('joined')
    const read = await zoe.call('app.eventside.conference.listRecords', {
      params: { conference: conference.space, collection: NSID.event },
    })
    expect(read.status, 'Zoe is still a member').toBe(200)
  })
}

// Regressions from adversarial review round 9.

/** A conference whose list row for Zoe's handle is held only by her list role's claim. */
async function claimedListRow(viv: VivariumScope) {
  const olga = await viv.createAccount(viv.handle('olga'))
  const pim = await viv.createAccount(viv.handle('pim'))
  const org = await createOrg(dep, olga)
  await addAdmin(dep, org, pim, 'owner')
  const conference = await createConference(dep, org, { ...ATMOSPHERECONF, superAdmin: pim, methods: ['list'] })
  const zoeHandle = viv.handle('zoe')
  await importList(dep, conference, [{ handle: zoeHandle, role: 'speaker' }])
  const zoeAccount = await viv.createAccount(zoeHandle)
  const zoe = await Attendee.signIn(dep, zoeAccount)
  await takeOffline(viv.url, olga)
  try {
    expect((await zoe.join({ conference: conference.space })).body.status).toBe('joined')
  } finally {
    await bringOnline(viv.url, olga)
  }
  // Someone else taking Zoe's old handle isn't let in by her row.
  const rowStaysWithZoe = async () => {
    await changeHandle(viv.url, zoeAccount, viv.handle('zoe-new'))
    const next = await viv.createAccount(zoeHandle)
    const answer = await (await Attendee.signIn(dep, next)).join({ conference: conference.space })
    expect(answer.body.status, 'the row stayed with Zoe').not.toBe('joined')
  }
  return { olga, pim, org, conference, zoeAccount, zoe, rowStaysWithZoe }
}

test('TC-17: a list row a role holds stays with its holder when they’re made an admin (review round 9)', async ({
  viv,
}) => {
  const { org, zoeAccount, rowStaysWithZoe } = await claimedListRow(viv)
  await addAdmin(dep, org, zoeAccount, 'staff')
  await rowStaysWithZoe()
})

test('TC-17: a list row a role holds stays with its holder through a re-import under a new handle (review round 9)', async ({
  viv,
}) => {
  const { conference, zoeAccount, rowStaysWithZoe } = await claimedListRow(viv)
  // The organizers re-import Zoe under the handle she's moving to.
  const zoeNew = viv.handle('zoe-new')
  await changeHandle(viv.url, zoeAccount, zoeNew)
  await importList(dep, conference, [{ handle: zoeNew, role: 'speaker' }])
  await rowStaysWithZoe()
})

test('TC-17: a removal that can’t keep a list row with its holder changes nothing (review round 9)', async ({
  viv,
}) => {
  const { olga, pim, conference, zoeAccount, zoe } = await claimedListRow(viv)
  // Olga, who binds the row before the role goes, is down.
  await takeOffline(viv.url, olga)
  try {
    const removal = await cli(dep, [
      'member',
      'remove',
      zoeAccount.did,
      '--conference',
      conference.space,
      '--as',
      pim.handle,
    ])
    expect(removal.code, 'the removal fails').not.toBe(0)
  } finally {
    await bringOnline(viv.url, olga)
  }
  const read = await zoe.call('app.eventside.conference.listRecords', {
    params: { conference: conference.space, collection: NSID.event },
  })
  expect(read.status, 'nothing was half done: Zoe is still a member').toBe(200)
})

test('TC-48: reindex reads an intake repo only up to its cap, and keeps the rest for later syncs (review round 9)', async ({
  viv,
}) => {
  const { org, conference } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  const mallory = await viv.createAccount(viv.handle('mallory'))
  const pds = await pdsOf(viv.url, mallory.did)
  const db = new DatabaseSync(dep.databasePath as string)
  try {
    const synced = () =>
      (
        db
          .prepare('SELECT synced_rev FROM space_repos WHERE space = ? AND repo = ?')
          .get(conference.intake, mallory.did) as { synced_rev: string } | undefined
      )?.synced_rev
    const join = () =>
      db
        .prepare('SELECT rev FROM space_records WHERE space = ? AND repo = ? AND collection = ?')
        .get(conference.intake, mallory.did, 'app.eventside.intake.join') as { rev: string } | undefined

    // More junk than one read takes (1200 ops), each batch read before the next.
    for (let batch = 0; batch < 6; batch++) {
      const before = synced()
      await xrpcOk(pds, 'com.atproto.space.applyWrites', {
        token: mallory.accessJwt,
        body: {
          space: conference.intake,
          repo: mallory.did,
          writes: Array.from({ length: 200 }, (_, n) => ({
            $type: 'com.atproto.space.applyWrites#create',
            collection: 'com.example.junk',
            rkey: `j${batch}x${n}`,
            value: { $type: 'com.example.junk', n },
          })),
        },
      })
      await eventually(
        async () => synced(),
        (rev) => rev !== undefined && rev !== before,
        `our host read batch ${batch}`,
      )
    }
    // Then a join, past what one read takes.
    await writeSpaceRecord(viv.url, mallory, conference.intake, 'app.eventside.intake.join', {})
    const stored = await eventually(
      async () => join(),
      (row) => row !== undefined,
      'our host stored the join',
    )

    await cliOk(dep, ['reindex', '--org', org.did])

    expect(join(), 'the join past the cap is kept').toEqual(stored)
    expect((synced() as string) < (stored as { rev: string }).rev, 'reindex read only up to the cap').toBe(true)

    // The next write's sync goes on from where reindex stopped.
    await writeSpaceRecord(viv.url, mallory, conference.intake, 'com.example.junk', { n: 'last' })
    await eventually(
      async () => synced(),
      (rev) => rev !== undefined && rev > (stored as { rev: string }).rev,
      'a later sync read on past the join',
    )
    expect(join()).toEqual(stored)
  } finally {
    db.close()
  }
})
