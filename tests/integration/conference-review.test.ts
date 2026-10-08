import { mkdtempSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { afterAll, beforeAll, expect, test, vi } from '@vivarium-dev/client/vitest'
import {
  bringOnline,
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
  expect(undo.stderr).toMatch(/stands, and .* can.t override it/)
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
  expect(refused.stderr).toMatch(/stands, and .* can.t override it/)
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

// Regressions from adversarial review round 7.

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

// Regressions from adversarial review round 9.

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

// Regressions from adversarial review round 10.

test('TC-32: importing a list leaves a role an admin gave alone (review round 10)', async ({ viv }) => {
  const { conference } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  const bram = await viv.createAccount(viv.handle('bram'))
  expect((await setRole(dep, conference, bram, 'speaker')).code).toBe(0)
  const asBram = await Attendee.signIn(dep, bram)
  expect((await asBram.join({ conference: conference.space })).body.status, 'admitted by the role').toBe('joined')

  // A list role admits only while the list does, which this conference doesn't use.
  const imported = await importList(dep, conference, [{ handle: bram.handle, role: 'speaker' }])
  expect(imported.stdout, 'the import says the role was kept').toContain(bram.did)
  const read = await asBram.call('app.eventside.conference.listRecords', {
    params: { conference: conference.space, collection: NSID.event },
  })
  expect(read.status, 'Bram is still a member').toBe(200)
})

test('TC-55: a removal whose role can’t be taken is undone (review round 10)', async ({ viv }) => {
  const olga = await viv.createAccount(viv.handle('olga'))
  const pim = await viv.createAccount(viv.handle('pim'))
  const org = await createOrg(dep, olga)
  await addAdmin(dep, org, pim, 'owner')
  const conference = await createConference(dep, org, { ...ATMOSPHERECONF, superAdmin: pim, methods: ['code'] })
  const bram = await viv.createAccount(viv.handle('bram'))
  expect((await setRole(dep, conference, bram, 'speaker')).code).toBe(0)
  const asBram = await Attendee.signIn(dep, bram)
  expect((await asBram.join({ conference: conference.space })).body.status).toBe('joined')

  // Pim, who writes the conference's roles, is down.
  await takeOffline(viv.url, pim)
  try {
    const removal = await cli(dep, ['member', 'remove', bram.handle, '--conference', conference.space])
    expect(removal.code, 'the removal fails').not.toBe(0)
    expect(removal.stderr).toMatch(/undone/)
  } finally {
    await bringOnline(viv.url, pim)
  }
  const read = await asBram.call('app.eventside.conference.listRecords', {
    params: { conference: conference.space, collection: NSID.event },
  })
  expect(read.status, 'nothing was half done: Bram is still a member').toBe(200)
})

// Regressions from the simplification (2026-10-07): list handles resolve at
// import, and admins' decisions rank by who made them.

test('TC-17: a list handle that doesn’t resolve at import is reported, and stores nothing (simplification)', async ({
  viv,
}) => {
  const { conference } = await seedConference(dep, accountsIn(viv), { methods: ['list'] })
  const zoeHandle = viv.handle('zoe')
  const ana = await viv.createAccount(viv.handle('ana'))
  const imported = await importList(dep, conference, [{ handle: zoeHandle, role: 'speaker' }, { handle: ana.handle }])
  expect(imported.stdout, 'the import names the handle it skipped').toContain(zoeHandle)
  expect(imported.stdout).not.toContain(ana.handle)

  // Whoever takes the handle later isn't on the list by it.
  const zoe = await Attendee.signIn(dep, await viv.createAccount(zoeHandle))
  const answer = await zoe.join({ conference: conference.space })
  expect(answer.body.status).toBe('refused')
  expect((await (await Attendee.signIn(dep, ana)).join({ conference: conference.space })).body.status).toBe('joined')
})

test('TC-52: staff can’t admit someone an owner denied (simplification)', async ({ viv }) => {
  const { org, conference, admins } = await seedConference(dep, accountsIn(viv), {
    staff: ['pim'],
    methods: ['request'],
  })
  const kees = await viv.createAccount(viv.handle('kees'))
  await addAdmin(dep, org, kees, 'owner')
  const bram = await Attendee.signIn(dep, await viv.createAccount(viv.handle('bram')))
  expect((await bram.join({ conference: conference.space, request: true })).body.status).toBe('pending')
  await cliOk(dep, ['requests', 'deny', bram.person.handle, '--conference', conference.space, '--as', kees.handle])

  const added = await cli(dep, [
    'member',
    'add',
    bram.person.handle,
    '--conference',
    conference.space,
    '--as',
    admins.pim.handle,
  ])
  expect(added.code).not.toBe(0)
  expect(added.stderr).toMatch(/by an owner or the super admin stands/)
  expect(await bram.isMember(conference.space)).toBe(false)
})

// Regressions from adversarial review round 11.

test('TC-52: a decision staff can’t make isn’t left on record to count later (review round 11)', async ({ viv }) => {
  const { org, conference, admins } = await seedConference(dep, accountsIn(viv), {
    staff: ['pim'],
    methods: ['code'],
  })
  const kees = await viv.createAccount(viv.handle('kees'))
  await addAdmin(dep, org, kees, 'owner')
  const ana = await Attendee.signIn(dep, await viv.createAccount(viv.handle('ana')))
  await cliOk(dep, ['member', 'add', ana.person.handle, '--conference', conference.space, '--as', kees.handle])

  const removal = await cli(dep, [
    'member',
    'remove',
    ana.person.handle,
    '--conference',
    conference.space,
    '--as',
    admins.pim.handle,
  ])
  expect(removal.code).not.toBe(0)
  expect(removal.stderr).toMatch(/nothing was changed/)
  // Promoting Pim doesn't bring his refused removal back to life.
  await addAdmin(dep, org, admins.pim, 'owner')
  expect(await ana.isMember(conference.space)).toBe(true)
})

test('TC-16: a bad role on an unresolved row refuses the import, and re-importing adds nothing twice (review round 11)', async ({
  viv,
}) => {
  const { conference, superAdmin, org } = await seedConference(dep, accountsIn(viv), { methods: ['list'] })
  const ana = await viv.createAccount(viv.handle('ana'))
  const bad = await cli(dep, [
    'list',
    'import',
    listFile([`${viv.handle('nobody-yet')},,organiser`, `${ana.handle},,speaker`]),
    '--conference',
    conference.space,
  ])
  expect(bad.code, 'a bad role refuses the import').not.toBe(0)

  const file = listFile([`${ana.handle},,speaker`])
  await cliOk(dep, ['list', 'import', file, '--conference', conference.space])
  await cliOk(dep, ['list', 'import', file, '--conference', conference.space])
  const entries = await ownSpaceRecords(viv.url, superAdmin, org.adminSpace, 'app.eventside.admin.listEntry')
  expect(
    entries.filter((r) => r.value.did === ana.did),
    'one row for Ana',
  ).toHaveLength(1)
  const answer = await (await Attendee.signIn(dep, ana)).join({ conference: conference.space })
  expect(answer.body.status).toBe('joined')
  expect(answer.body.role).toBe('speaker')
})

// Regressions from adversarial review round 12.

test('TC-51: the super admin can lift another owner’s ban (review round 12)', async ({ viv }) => {
  const { org, conference } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  const kees = await viv.createAccount(viv.handle('kees'))
  await addAdmin(dep, org, kees, 'owner')
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const bram = await Attendee.signIn(dep, await viv.createAccount(viv.handle('bram')))
  expect((await bram.join({ conference: conference.space, code })).body.status).toBe('joined')
  await cliOk(dep, ['member', 'ban', bram.person.handle, '--conference', conference.space, '--as', kees.handle])
  expect(await bram.isMember(conference.space)).toBe(false)

  await cliOk(dep, ['member', 'add', bram.person.handle, '--conference', conference.space])
  expect(await bram.isMember(conference.space), 'Olga’s admission lifts Kees’s ban').toBe(true)
})

test('TC-32: a list role another owner gave is taken over by a later import, and outlives them (review round 12)', async ({
  viv,
}) => {
  const { org, conference } = await seedConference(dep, accountsIn(viv), { methods: ['list'] })
  const sem = await viv.createAccount(viv.handle('sem'))
  const kees = await viv.createAccount(viv.handle('kees'))
  await addAdmin(dep, org, sem, 'owner')
  await addAdmin(dep, org, kees, 'owner')
  const zoe = await viv.createAccount(viv.handle('zoe'))
  const file = listFile([`${zoe.handle},,speaker`])
  await cliOk(dep, ['list', 'import', file, '--conference', conference.space, '--as', sem.handle])
  await cliOk(dep, ['list', 'import', file, '--conference', conference.space, '--as', kees.handle])
  await cliOk(dep, ['org', 'admin', 'remove', sem.handle, '--org', org.did])

  const answer = await (await Attendee.signIn(dep, zoe)).join({ conference: conference.space })
  expect(answer.body.status).toBe('joined')
  expect(answer.body.role, 'Kees’s list gives the role now').toBe('speaker')
})

// Regressions from adversarial review round 13.

test('TC-34: an owner’s list role taken over by another owner keeps its time; a different one isn’t replaced (review round 13)', async ({
  viv,
}) => {
  const { org, conference, superAdmin } = await seedConference(dep, accountsIn(viv), { methods: ['list'] })
  const kees = await viv.createAccount(viv.handle('kees'))
  const sem = await viv.createAccount(viv.handle('sem'))
  await addAdmin(dep, org, kees, 'owner')
  await addAdmin(dep, org, sem, 'owner')
  const zoe = await viv.createAccount(viv.handle('zoe'))
  const importAs = (as: string, role: string) =>
    cliOk(dep, ['list', 'import', listFile([`${zoe.handle},,${role}`]), '--conference', conference.space, '--as', as])
  await importAs(kees.handle, 'staff')
  expect((await (await Attendee.signIn(dep, zoe)).join({ conference: conference.space })).body.role).toBe('staff')
  await writeSpaceRecord(viv.url, zoe, conference.space, NSID.announcement, announcement('Zoe: doors at 9'))
  const asOlga = await Attendee.signIn(dep, superAdmin)
  const zoes = async () => {
    const answer = await asOlga.listRecords(conference.space, NSID.announcement)
    return (answer.body.records as Json[]).filter((r) => r.author === zoe.did)
  }
  await eventually(zoes, (records) => records.length === 1, 'Zoe’s announcement is shown')

  const other = await importAs(sem.handle, 'speaker')
  expect(other.stdout, 'Kees’s staff role is kept').toContain(zoe.did)
  await importAs(sem.handle, 'staff')
  await cliOk(dep, ['org', 'admin', 'remove', kees.handle, '--org', org.did])
  expect(await zoes(), 'her announcement from before the takeover').toHaveLength(1)
  const page = await (await Attendee.signIn(dep, zoe)).getConference(conference.space)
  expect(page.body.viewer?.role).toBe('staff')
})

// Regressions from adversarial review round 14.

test('TC-32: removing the owner whose list last gave a role hands it to another owner whose list gives it too (review round 14)', async ({
  viv,
}) => {
  const { org, conference } = await seedConference(dep, accountsIn(viv), { methods: ['list'] })
  const kees = await viv.createAccount(viv.handle('kees'))
  const sem = await viv.createAccount(viv.handle('sem'))
  await addAdmin(dep, org, kees, 'owner')
  await addAdmin(dep, org, sem, 'owner')
  const zoe = await viv.createAccount(viv.handle('zoe'))
  const importAs = (as: string) =>
    cliOk(dep, ['list', 'import', listFile([`${zoe.handle},,staff`]), '--conference', conference.space, '--as', as])
  await importAs(kees.handle)
  await importAs(sem.handle)
  await importAs(kees.handle)
  await cliOk(dep, ['org', 'admin', 'remove', kees.handle, '--org', org.did])

  const asZoe = await Attendee.signIn(dep, zoe)
  expect((await asZoe.join({ conference: conference.space })).body.status).toBe('joined')
  expect((await asZoe.getConference(conference.space)).body.viewer?.role, 'still staff, by Sem’s list').toBe('staff')
})

// Regressions from adversarial review round 15.

test('TC-32: demoting an owner hands their list roles on, or they stop counting (review round 15)', async ({ viv }) => {
  const { org, conference } = await seedConference(dep, accountsIn(viv), { methods: ['list'] })
  const kees = await viv.createAccount(viv.handle('kees'))
  const sem = await viv.createAccount(viv.handle('sem'))
  await addAdmin(dep, org, kees, 'owner')
  await addAdmin(dep, org, sem, 'owner')
  const zoe = await viv.createAccount(viv.handle('zoe'))
  const joost = await viv.createAccount(viv.handle('joost'))
  const importAs = (as: string, rows: string[]) =>
    cliOk(dep, ['list', 'import', listFile(rows), '--conference', conference.space, '--as', as])
  await importAs(sem.handle, [`${zoe.handle},,staff`])
  await importAs(kees.handle, [`${zoe.handle},,staff`, `${joost.handle},,speaker`])

  const demoted = await cliOk(dep, ['org', 'admin', 'add', kees.handle, '--org', org.did, '--role', 'staff'])
  const asZoe = await Attendee.signIn(dep, zoe)
  expect((await asZoe.join({ conference: conference.space })).body.status).toBe('joined')
  expect((await asZoe.getConference(conference.space)).body.viewer?.role, 'by Sem’s list').toBe('staff')
  // Joost was on Kees's list only: its speaker role doesn't let him in now.
  expect(demoted.stdout).not.toContain(zoe.handle)
  const answer = await (await Attendee.signIn(dep, joost)).join({ conference: conference.space })
  expect(answer.body.status).not.toBe('joined')
})

/** An attendee list file with the given `handle,email,role` rows. */
function listFile(rows: string[]): string {
  const file = join(mkdtempSync(join(tmpdir(), 'eventside-list-')), 'attendees.csv')
  writeFileSync(file, `${['handle,email,role', ...rows].join('\n')}\n`)
  return file
}
