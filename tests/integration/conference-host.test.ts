import { afterAll, beforeAll, expect, test, vi } from '@vivarium-dev/client/vitest'
import {
  authorityOf,
  eventually,
  publicRecords,
  serviceAuth,
  spaceHostOf,
  tidNow,
  writeSpaceRecord,
  xrpc,
} from '../support/atproto.ts'
import {
  Attendee,
  accountsIn,
  allowApp,
  allowAppForOrg,
  cli,
  type Deployment,
  deploy,
  NSID,
  plan,
  seedConference,
  sharedCode,
  uniqueCode,
} from '../support/conference.ts'
import { isCurrent, memberEntry, OtherApp } from '../support/other-app.ts'

// Seeding an organization takes several CLI runs and OAuth round trips.
vi.setConfig({ testTimeout: 120_000, hookTimeout: 60_000 })

// Our server as the space host: credentials for apps, write notices from
// writers' PDSes, and the records other apps read about roles and rules.

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

test('TC-30: Owners and staff are members with a role others can see', async ({ viv }) => {
  const { conference, superAdmin: olga, admins } = await seedConference(dep, accountsIn(viv), { staff: ['pim'] })
  await allowApp(dep, conference, app.clientId)

  const access = await (await app.signIn(admins.pim)).open(conference.space)
  const members = await access.members()
  expect(isCurrent(memberEntry(members, olga.did)), 'Olga is a member').toBe(true)
  expect(isCurrent(memberEntry(members, admins.pim.did)), 'Pim is a member').toBe(true)

  // Roles, from records Olga wrote into the space.
  const roles = (await access.records(olga.did, NSID.role)).map((r) => r.value)
  expect(roles).toContainEqual(expect.objectContaining({ subject: olga.did, role: 'owner' }))
  expect(roles).toContainEqual(expect.objectContaining({ subject: admins.pim.did, role: 'staff' }))
})

test('TC-33: The conference’s rules say who may post what', async ({ viv }) => {
  const { conference, superAdmin: olga } = await seedConference(dep, accountsIn(viv))
  await allowApp(dep, conference, app.clientId)

  // The rules come from the conference's super admin, as its settings name them.
  const settings = (await publicRecords(viv.url, olga.did, 'app.eventside.conference')).find(
    (r) => r.value.space === conference.space,
  )
  expect(settings?.value.superAdmin).toBe(olga.did)

  const access = await (await app.signIn(olga)).open(conference.space)
  const rules = await access.records(olga.did, NSID.rules)
  expect(rules, 'one rules record').toHaveLength(1)
  const writers = Object.fromEntries(
    (rules[0].value.rules as { collection: string; writers: string }[]).map((r) => [r.collection, r.writers]),
  )
  expect(writers).toMatchObject({
    [NSID.card]: 'admins',
    [NSID.announcement]: 'admins',
    [NSID.event]: 'members',
    [NSID.chat]: 'members',
  })
})

test('TC-36: By default, only eventside can read the space', async ({ viv }) => {
  const { conference } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const ana = await viv.createAccount(viv.handle('ana'))
  expect((await (await Attendee.signIn(dep, ana)).join({ conference: conference.space, code })).body.status).toBe(
    'joined',
  )
  const asAna = await app.signIn(ana)

  const refused = await asAna.requestCredential(conference.space)
  expect(refused.status).toBe(400)
  expect(refused.body.error).toBe('AppNotAuthorized')

  await allowApp(dep, conference, app.clientId)
  const access = await asAna.open(conference.space)
  expect(Array.isArray(await access.listRepos())).toBe(true)
})

test('TC-37: An open conference lets any app a member uses read', async ({ viv }) => {
  const { conference } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const ana = await viv.createAccount(viv.handle('ana'))
  const mallory = await viv.createAccount(viv.handle('mallory'))
  expect((await (await Attendee.signIn(dep, ana)).join({ conference: conference.space, code })).body.status).toBe(
    'joined',
  )

  // The CLI warns before it opens the space to any app.
  const warned = await cli(dep, ['apps', 'open', '--conference', conference.space])
  expect(warned.code, 'it waits for confirmation').not.toBe(0)
  expect(warned.stdout + warned.stderr).toMatch(/any app/i)
  const opened = await cli(dep, ['apps', 'open', '--conference', conference.space, '--yes'])
  expect(opened.code, opened.stderr).toBe(0)

  // This app is on no list.
  const access = await (await app.signIn(ana)).open(conference.space)
  expect(Array.isArray(await access.listRepos())).toBe(true)
  const asMallory = await (await app.signIn(mallory)).requestCredential(conference.space)
  expect(asMallory.status).toBe(400)
  expect(asMallory.body.error).toBe('UserNotAuthorized')
})

test('TC-47: Another app reads a member’s records with access from our server', async ({ viv }) => {
  const { conference } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  await allowApp(dep, conference, app.clientId)
  const ana = await viv.createAccount(viv.handle('ana'))
  const bram = await viv.createAccount(viv.handle('bram'))
  for (const person of [ana, bram]) {
    expect((await (await Attendee.signIn(dep, person)).join({ conference: conference.space, code })).body.status).toBe(
      'joined',
    )
  }
  await writeSpaceRecord(viv.url, ana, conference.space, NSID.event, plan('Dinner at Moeders'))

  const access = await (await app.signIn(bram)).open(conference.space)
  await eventually(
    () => access.writers(),
    (writers) => writers.includes(ana.did),
    'Ana is among the writers',
  )
  const records = await access.records(ana.did, NSID.event)
  expect(records.map((r) => r.value.name)).toContain('Dinner at Moeders')
})

test('TC-48: A non-member’s writes don’t enter the space', async ({ viv }) => {
  const { conference } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  await allowApp(dep, conference, app.clientId)
  const ana = await viv.createAccount(viv.handle('ana'))
  const mallory = await viv.createAccount(viv.handle('mallory'))
  expect((await (await Attendee.signIn(dep, ana)).join({ conference: conference.space, code })).body.status).toBe(
    'joined',
  )

  // Mallory's own PDS takes her write, and tells our server about it.
  await writeSpaceRecord(viv.url, mallory, conference.space, NSID.event, plan('Crashing the speaker dinner'))
  // The notice her PDS sends, made the same way: our server refuses it.
  const authority = authorityOf(conference.space)
  const token = await serviceAuth(viv.url, mallory, `${authority}#atproto_space_host`, 'com.atproto.space.notifyWrite')
  const notice = await xrpc(await spaceHostOf(viv.url, authority), 'com.atproto.space.notifyWrite', {
    token,
    body: {
      space: conference.space,
      repo: mallory.did,
      repoRev: tidNow(),
      hash: { $bytes: Buffer.alloc(32).toString('base64') },
    },
  })
  expect(notice.status).toBe(403)

  // A member's write enters; Mallory's never does.
  await writeSpaceRecord(viv.url, ana, conference.space, NSID.event, plan('Dinner at Moeders'))
  const access = await (await app.signIn(ana)).open(conference.space)
  const writers = await eventually(
    () => access.writers(),
    (writers) => writers.includes(ana.did),
    'Ana is a writer',
  )
  expect(writers).not.toContain(mallory.did)
})

test('TC-49: An app can’t get access to a space for someone who isn’t a member', async ({ viv }) => {
  const { conference } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  await allowApp(dep, conference, app.clientId)
  const mallory = await viv.createAccount(viv.handle('mallory'))

  const answer = await (await app.signIn(mallory)).requestCredential(conference.space)
  expect(answer.status).toBe(400)
  expect(answer.body.error).toBe('UserNotAuthorized')
  expect(answer.body.credential).toBeUndefined()
})

test('TC-56: Admins can read join records, attendees can’t', async ({ viv }) => {
  const { org, conference, admins } = await seedConference(dep, accountsIn(viv), { staff: ['pim'], methods: ['code'] })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const second = await sharedCode(dep, conference, uniqueCode('speakers27'))
  const ana = await viv.createAccount(viv.handle('ana'))
  const bram = await viv.createAccount(viv.handle('bram'))
  expect((await (await Attendee.signIn(dep, ana)).join({ conference: conference.space, code })).body.status).toBe(
    'joined',
  )
  expect(
    (await (await Attendee.signIn(dep, bram)).join({ conference: conference.space, code: second })).body.status,
  ).toBe('joined')
  // The intake has the admin space's app access.
  await allowAppForOrg(dep, org, app.clientId)

  const asPim = await (await app.signIn(admins.pim)).open(conference.intake)
  await eventually(
    () => asPim.writers(),
    (writers) => writers.includes(ana.did) && writers.includes(bram.did),
    'both joins are in the intake',
  )
  const codes = async (did: string) => (await asPim.records(did, NSID.join)).map((r) => r.value.code)
  expect(await codes(ana.did)).toContain(code)
  expect(await codes(bram.did)).toContain(second)

  const asAna = await (await app.signIn(ana)).requestCredential(conference.intake)
  expect(asAna.status).toBe(400)
  expect(asAna.body.error).toBe('UserNotAuthorized')
})
