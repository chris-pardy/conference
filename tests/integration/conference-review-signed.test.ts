import { DatabaseSync } from 'node:sqlite'
import { afterAll, beforeAll, expect, test, vi } from '@vivarium-dev/client/vitest'
import { eventually, ownSpaceRecords, pdsOf, xrpcOk } from '../support/atproto.ts'
import {
  Attendee,
  accountsIn,
  addAdmin,
  cli,
  cliJson,
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

/** Runs SQL against the deployment's own database, as the operator could. */
function sql(query: string, ...params: (string | number)[]) {
  const db = new DatabaseSync(dep.databasePath as string)
  try {
    db.prepare(query).run(...params)
  } finally {
    db.close()
  }
}

test('TC-53: a former admin’s deletion reaches the index without a reindex (review round 17)', async ({ viv }) => {
  const { org, conference } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  const kees = await viv.createAccount(viv.handle('kees'))
  await addAdmin(dep, org, kees, 'owner')
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const bram = await Attendee.signIn(dep, await viv.createAccount(viv.handle('bram')))
  expect((await memberAction(dep, conference, ['member', 'ban'], bram.person, { as: kees })).code).toBe(0)
  await cliOk(dep, ['org', 'admin', 'remove', kees.handle, '--org', org.did])

  // Kees, no longer an admin, withdraws his ban by deleting it from his own PDS.
  const ban = 'app.eventside.admin.ban'
  const bans = await ownSpaceRecords(viv.url, kees, org.adminSpace, ban)
  expect(bans).toHaveLength(1)
  await xrpcOk(await pdsOf(viv.url, kees.did), 'com.atproto.space.deleteRecord', {
    token: kees.accessJwt,
    body: { space: org.adminSpace, repo: kees.did, collection: ban, rkey: bans[0].rkey },
  })
  await eventually(
    async () => (await bram.getConference(conference.space)).body.viewer,
    (viewer) => viewer?.request === undefined,
    'the deletion reaches the index',
  )
  expect((await bram.join({ conference: conference.space, code })).body.status).toBe('joined')
})

test('TC-52: staff can’t let someone an owner removed back in with a code (review round 17)', async ({ viv }) => {
  const { org, conference, admins } = await seedConference(dep, accountsIn(viv), { staff: ['pim'], methods: ['code'] })
  const kees = await viv.createAccount(viv.handle('kees'))
  await addAdmin(dep, org, kees, 'owner')
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const issued = await cli(dep, [
    'codes',
    'issue',
    '--conference',
    conference.space,
    '--shared',
    uniqueCode('crew'),
    '--as',
    admins.pim.handle,
    '--json',
  ])
  expect(issued.code, issued.stderr).toBe(0)
  const staffCode = cliJson(issued).codes[0]
  const ana = await Attendee.signIn(dep, await viv.createAccount(viv.handle('ana')))
  expect((await ana.join({ conference: conference.space, code })).body.status).toBe('joined')
  expect((await memberAction(dep, conference, ['member', 'remove'], ana.person, { as: kees })).code).toBe(0)

  expect((await ana.join({ conference: conference.space, code: staffCode })).body.status, 'staff’s code').toBe(
    'refused',
  )
  expect(await ana.isMember(conference.space)).toBe(false)
  // The super admin's code is of a rank to stand over the owner's removal.
  expect((await ana.join({ conference: conference.space, code })).body.status).toBe('joined')
})

test('TC-55: a decision written but not read back doesn’t block the next one once indexed (review round 17)', async ({
  viv,
}) => {
  const { org, conference, admins } = await seedConference(dep, accountsIn(viv), { staff: ['pim'], methods: ['code'] })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const ana = await Attendee.signIn(dep, await viv.createAccount(viv.handle('ana')))
  expect((await ana.join({ conference: conference.space, code })).body.status).toBe('joined')
  // As if reading her join back had failed: its entry is still pending,
  // though the record is written and in the index.
  sql(
    "UPDATE signing_journal SET state = 'pending', created_at = ? WHERE authority = ? AND subject = ?",
    Date.now(),
    org.did,
    ana.did,
  )
  const removed = await memberAction(dep, conference, ['member', 'remove'], ana.person, { as: admins.pim })
  expect(removed.code, removed.stderr).toBe(0)
  expect(await ana.isMember(conference.space)).toBe(false)
})

test('TC-55: one signing with a fast clock doesn’t date later ones ahead (review round 17)', async ({ viv }) => {
  const { org, conference } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  const bram = await Attendee.signIn(dep, await viv.createAccount(viv.handle('bram')))
  // The last signing was dated an hour ahead, by a clock since put right.
  sql('UPDATE signing_counters SET last_signed_at = ? WHERE authority = ?', Date.now() + 3_600_000, org.did)
  const added = await memberAction(dep, conference, ['member', 'add'], bram.person)
  expect(added.code, added.stderr).toBe(0)
  expect(await bram.isMember(conference.space), 'his admission counts now').toBe(true)
})

test('TC-65: one code can be revoked, at its issuer’s rank (review round 17)', async ({ viv }) => {
  const { conference, admins } = await seedConference(dep, accountsIn(viv), { staff: ['pim'], methods: ['code'] })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const other = await sharedCode(dep, conference, uniqueCode('crew'))
  const byPim = await cli(dep, ['codes', 'revoke', code, '--conference', conference.space, '--as', admins.pim.handle])
  expect(byPim.code, 'staff can’t revoke the super admin’s code').not.toBe(0)
  await cliOk(dep, ['codes', 'revoke', code, '--conference', conference.space])

  const ana = await Attendee.signIn(dep, await viv.createAccount(viv.handle('ana')))
  expect((await ana.join({ conference: conference.space, code })).status).toBe(400)
  expect((await ana.join({ conference: conference.space, code: other })).body.status, 'the others still work').toBe(
    'joined',
  )
})
