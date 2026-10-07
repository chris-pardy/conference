import { DatabaseSync } from 'node:sqlite'
import { afterAll, beforeAll, expect, test, vi } from '@vivarium-dev/client/vitest'
import { eventually, type Json, writeSpaceRecord } from '../support/atproto.ts'
import {
  Attendee,
  accountsIn,
  allowApp,
  cli,
  cliOk,
  cliOkJson,
  createConference,
  createOrg,
  type Deployment,
  deploy,
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
