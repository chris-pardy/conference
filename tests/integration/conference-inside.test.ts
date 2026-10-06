import { afterAll, beforeAll, expect, test, vi } from '@vivarium-dev/client/vitest'
import { type Account, eventually, type Json, writeSpaceRecord } from '../support/atproto.ts'
import {
  Attendee,
  accountsIn,
  allowApp,
  announcement,
  type Conference,
  type Deployment,
  deploy,
  memberAction,
  NSID,
  plan,
  seedConference,
  setRole,
  sharedCode,
  uniqueCode,
} from '../support/conference.ts'
import { isCurrent, memberEntry, OtherApp } from '../support/other-app.ts'

// Seeding an organization takes several CLI runs and OAuth round trips.
vi.setConfig({ testTimeout: 120_000, hookTimeout: 60_000 })

// Inside a conference: what members are served, leaving, removal, and the
// rules readers apply to the records in the space.

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

/** Signs a new person in and joins them with a code. */
async function joined(person: Account, conference: Conference, code: string) {
  const attendee = await Attendee.signIn(dep, person)
  const answer = await attendee.join({ conference: conference.space, code })
  expect(answer.body.status, `${person.handle} joins`).toBe('joined')
  return attendee
}

/** The conference's records in a collection, as the app lists them for a member. */
async function listed(member: Attendee, conference: Conference, collection: string): Promise<Json[]> {
  const answer = await member.listRecords(conference.space, collection)
  if (answer.status !== 200) throw new Error(`listRecords answered ${answer.status}: ${JSON.stringify(answer.body)}`)
  return answer.body.records
}

const texts = (records: Json[]) => records.map((r) => r.value.text ?? r.value.name)

test('TC-25: Non-members can’t see inside', async ({ viv }) => {
  const { conference, superAdmin } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  await allowApp(dep, conference, app.clientId)
  const ana = await joined(await viv.createAccount(viv.handle('ana')), conference, code)
  const mallory = await viv.createAccount(viv.handle('mallory'))
  await writeSpaceRecord(viv.url, superAdmin, conference.space, NSID.announcement, announcement('Doors open at 9'))

  // A member sees inside.
  await eventually(
    () => listed(ana, conference, NSID.announcement),
    (records) => texts(records).includes('Doors open at 9'),
    'Ana sees the announcement',
  )

  // Mallory gets nothing, from the app…
  const asMallory = await Attendee.signIn(dep, mallory)
  const inside = await asMallory.listRecords(conference.space, NSID.announcement)
  expect(inside.status).toBe(403)
  expect(JSON.stringify(inside.body)).not.toContain('Doors open at 9')
  // …or from the space itself, through an app it allows.
  const refused = await (await app.signIn(mallory)).requestCredential(conference.space)
  expect(refused.status).toBe(400)
  expect(refused.body.error).toBe('UserNotAuthorized')
})

test('TC-26: Ana leaves, and can come back', async ({ viv }) => {
  const { conference, superAdmin } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  await allowApp(dep, conference, app.clientId)
  const ana = await joined(await viv.createAccount(viv.handle('ana')), conference, code)
  const access = await (await app.signIn(superAdmin)).open(conference.space)
  expect(isCurrent(memberEntry(await access.members(), ana.did))).toBe(true)

  const left = await ana.leave(conference.space)
  const leftAt = Date.now()
  expect(left.status).toBe(200)
  expect((await ana.listRecords(conference.space, NSID.announcement)).status).toBe(403)
  expect(await ana.isMember(conference.space)).toBe(false)

  // Another app sees her membership ended now.
  const ended = memberEntry(await access.members(), ana.did)
  expect(isCurrent(ended)).toBe(false)
  const until = Date.parse(ended?.periods.at(-1).until)
  expect(Math.abs(until - leftAt)).toBeLessThan(60_000)

  // Rejoining starts a new period.
  expect((await ana.join({ conference: conference.space, code })).body.status).toBe('joined')
  const rejoined = memberEntry(await access.members(), ana.did)
  expect(isCurrent(rejoined)).toBe(true)
  expect(rejoined?.periods).toHaveLength(2)
  const [first, second] = rejoined?.periods ?? []
  expect(Date.parse(second.since)).toBeGreaterThanOrEqual(Date.parse(first.until))
})

test('TC-27: A removed member loses access at once', async ({ viv }) => {
  const { conference, superAdmin } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  await allowApp(dep, conference, app.clientId)
  const anaAccount = await viv.createAccount(viv.handle('ana'))
  const bramAccount = await viv.createAccount(viv.handle('bram'))
  await joined(anaAccount, conference, code)
  const bram = await joined(bramAccount, conference, code)
  await writeSpaceRecord(viv.url, anaAccount, conference.space, NSID.event, plan('Borrel at Hannekes Boom'))

  // An app already holding access through Bram reads Ana's records.
  const throughBram = await (await app.signIn(bramAccount)).open(conference.space)
  await eventually(
    () => throughBram.listRecords(anaAccount.did, NSID.event),
    (answer) => answer.status === 200 && answer.body.records.length === 1,
    'the app reads Ana’s plan through Bram',
  )

  const removed = await memberAction(dep, conference, ['member', 'remove'], bramAccount)
  expect(removed.code, removed.stderr).toBe(0)

  // His very next request gets nothing.
  expect((await bram.listRecords(conference.space, NSID.event)).status).toBe(403)
  // Another app sees his membership ended.
  const asOlga = await (await app.signIn(superAdmin)).open(conference.space)
  expect(isCurrent(memberEntry(await asOlga.members(), bramAccount.did))).toBe(false)
  // The access the app held through him no longer reads anyone's records.
  const revoked = await eventually(
    () => throughBram.listRecords(anaAccount.did, NSID.event),
    (answer) => answer.status !== 200,
    'the credential delegated by Bram is revoked',
  )
  expect(revoked.status).toBe(401)
  expect(revoked.body.error).toBe('CredentialRevoked')
  // And it can't get new access through him.
  const again = await (await app.signIn(bramAccount)).requestCredential(conference.space)
  expect(again.body.error).toBe('UserNotAuthorized')
})

test('TC-29: Records written while not a member are never shown', async ({ viv }) => {
  const { conference } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const bramAccount = await viv.createAccount(viv.handle('bram'))
  const ana = await joined(await viv.createAccount(viv.handle('ana')), conference, code)
  await joined(bramAccount, conference, code)

  await writeSpaceRecord(viv.url, bramAccount, conference.space, NSID.event, plan('Coffee at the Hortus'))
  await eventually(
    () => listed(ana, conference, NSID.event),
    (records) => texts(records).includes('Coffee at the Hortus'),
    'Bram’s plan is listed',
  )

  expect((await memberAction(dep, conference, ['member', 'remove'], bramAccount)).code).toBe(0)
  // His own PDS still takes his writes while he's out.
  await writeSpaceRecord(viv.url, bramAccount, conference.space, NSID.event, plan('Written while removed'))
  expect((await memberAction(dep, conference, ['member', 'add'], bramAccount)).code).toBe(0)
  await writeSpaceRecord(viv.url, bramAccount, conference.space, NSID.event, plan('Back again'))

  const records = await eventually(
    () => listed(ana, conference, NSID.event),
    (records) => texts(records).includes('Back again'),
    'Bram’s plan from after he was let back in is listed',
  )
  expect(texts(records)).toContain('Coffee at the Hortus')
  expect(texts(records)).not.toContain('Written while removed')
})

test('TC-32: Changing and removing roles', async ({ viv }) => {
  const { conference, superAdmin } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  await allowApp(dep, conference, app.clientId)
  const ana = await viv.createAccount(viv.handle('ana'))
  expect((await setRole(dep, conference, ana, 'speaker')).code).toBe(0)
  const asAna = await Attendee.signIn(dep, ana)
  expect((await asAna.join({ conference: conference.space })).body.status).toBe('joined')

  const access = await (await app.signIn(superAdmin)).open(conference.space)
  const rolesOfAna = async () =>
    (await access.records(superAdmin.did, NSID.role))
      .filter((r) => r.value.subject === ana.did)
      .map((r) => r.value.role)
  await eventually(rolesOfAna, (roles) => roles.join() === 'speaker', 'Ana is a speaker')

  const staffed = await setRole(dep, conference, ana, 'staff')
  expect(staffed.code, staffed.stderr).toBe(0)
  expect(await eventually(rolesOfAna, (roles) => roles.includes('staff'), 'Ana is staff')).toEqual(['staff'])

  expect((await memberAction(dep, conference, ['member', 'remove'], ana)).code).toBe(0)
  expect(await eventually(rolesOfAna, (roles) => roles.length === 0, 'Ana has no role')).toEqual([])
})

test('TC-34: Records that break the rules aren’t shown', async ({ viv }) => {
  const { conference, superAdmin } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const anaAccount = await viv.createAccount(viv.handle('ana'))
  await joined(anaAccount, conference, code)
  const bram = await joined(await viv.createAccount(viv.handle('bram')), conference, code)

  await writeSpaceRecord(
    viv.url,
    anaAccount,
    conference.space,
    NSID.announcement,
    announcement('Free stroopwafels at my talk'),
  )
  await writeSpaceRecord(viv.url, superAdmin, conference.space, NSID.announcement, announcement('Doors open at 9'))

  const records = await eventually(
    () => listed(bram, conference, NSID.announcement),
    (records) => texts(records).includes('Doors open at 9'),
    'the organizer’s announcement is listed',
  )
  expect(texts(records)).not.toContain('Free stroopwafels at my talk')
})

test('TC-35: Roles and rules from anyone but the super admin are ignored', async ({ viv }) => {
  const { conference, superAdmin, admins } = await seedConference(dep, accountsIn(viv), {
    staff: ['pim'],
    methods: ['code'],
  })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const anaAccount = await viv.createAccount(viv.handle('ana'))
  const ana = await joined(anaAccount, conference, code)

  // Both claim Ana is an owner, and that any member may announce.
  const looserRules = {
    rules: [{ collection: NSID.announcement, writers: 'members' }],
    createdAt: new Date().toISOString(),
  }
  for (const author of [anaAccount, admins.pim]) {
    await writeSpaceRecord(viv.url, author, conference.space, NSID.role, {
      subject: anaAccount.did,
      role: 'owner',
      createdAt: new Date().toISOString(),
    })
    await writeSpaceRecord(viv.url, author, conference.space, NSID.rules, looserRules, 'self')
  }
  await writeSpaceRecord(viv.url, anaAccount, conference.space, NSID.announcement, announcement('Ana’s talk moved'))
  await writeSpaceRecord(viv.url, superAdmin, conference.space, NSID.announcement, announcement('Doors open at 9'))

  const records = await eventually(
    () => listed(ana, conference, NSID.announcement),
    (records) => texts(records).includes('Doors open at 9'),
    'the organizer’s announcement is listed',
  )
  expect(texts(records)).not.toContain('Ana’s talk moved')
  const viewer = (await ana.getConference(conference.space)).body.viewer
  expect(viewer?.member).toBe(true)
  expect(['owner', 'staff']).not.toContain(viewer?.role)
})
