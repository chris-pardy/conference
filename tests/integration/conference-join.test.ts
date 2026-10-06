import { afterAll, beforeAll, expect, test, vi } from '@vivarium-dev/client/vitest'
import { bringOnline, changeHandle, eventually, ownSpaceRecords, takeOffline } from '../support/atproto.ts'
import { sleep } from '../support/auth.ts'
import {
  Attendee,
  accountsIn,
  allowApp,
  cliOkJson,
  type Deployment,
  deploy,
  importList,
  memberAction,
  personalCode,
  seedConference,
  setRole,
  sharedCode,
  uniqueCode,
} from '../support/conference.ts'
import { isCurrent, memberEntry, OtherApp } from '../support/other-app.ts'

// Seeding an organization takes several CLI runs and OAuth round trips.
vi.setConfig({ testTimeout: 120_000, hookTimeout: 60_000 })

// Joining a conference, by each method, through the app's conference XRPC.

let dep: Deployment
beforeAll(async () => {
  dep = await deploy()
})
afterAll(async () => {
  await dep?.stop()
})

test('TC-12: A wrong code doesn’t admit anyone', async ({ viv }) => {
  const { conference } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const mallory = await Attendee.signIn(dep, await viv.createAccount(viv.handle('mallory')))

  const answer = await mallory.join({ conference: conference.space, code: uniqueCode('no-such-code') })
  expect(answer.status).toBe(400)
  expect(answer.body.error).toBe('InvalidCode')
  expect(await mallory.isMember(conference.space)).toBe(false)
})

test('TC-13: Expired and used-up shared codes stop working', async ({ viv }) => {
  const { conference } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  const expiring = await sharedCode(dep, conference, uniqueCode('early-bird'), {
    expires: new Date(Date.now() + 3_000).toISOString(),
  })
  const twoUses = await sharedCode(dep, conference, uniqueCode('crew'), { maxUses: 2 })
  for (const name of ['first', 'second']) {
    const someone = await Attendee.signIn(dep, await viv.createAccount(viv.handle(name)))
    expect((await someone.join({ conference: conference.space, code: twoUses })).body.status).toBe('joined')
  }
  await sleep(4_000) // past the first code's expiry

  const bram = await Attendee.signIn(dep, await viv.createAccount(viv.handle('bram')))
  for (const code of [expiring, twoUses]) {
    const answer = await bram.join({ conference: conference.space, code })
    expect(answer.status, code).toBe(400)
    expect(answer.body.error, code).toBe('InvalidCode')
  }
  expect(await bram.isMember(conference.space)).toBe(false)
})

test('TC-14: A personal code belongs to the first person who uses it', async ({ viv }) => {
  const { conference } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  const code = await personalCode(dep, conference)
  const ana = await Attendee.signIn(dep, await viv.createAccount(viv.handle('ana')))
  const bram = await Attendee.signIn(dep, await viv.createAccount(viv.handle('bram')))

  expect((await ana.join({ conference: conference.space, code })).body.status).toBe('joined')
  expect((await ana.leave(conference.space)).status).toBe(200)
  expect(await ana.isMember(conference.space)).toBe(false)
  expect((await ana.join({ conference: conference.space, code })).body.status).toBe('joined')
  expect(await ana.isMember(conference.space)).toBe(true)

  const answer = await bram.join({ conference: conference.space, code })
  expect(answer.status).toBe(400)
  expect(answer.body.error).toBe('InvalidCode')
  expect(await bram.isMember(conference.space)).toBe(false)
})

test('TC-16: Being on the attendee list by handle admits you on sign-in', async ({ viv }) => {
  const { conference } = await seedConference(dep, accountsIn(viv), { methods: ['list'] })
  const ana = await viv.createAccount(viv.handle('ana'))
  await importList(dep, conference, [{ handle: ana.handle }])

  const asAna = await Attendee.signIn(dep, ana)
  const answer = await asAna.join({ conference: conference.space })
  expect(answer.status).toBe(200)
  expect(answer.body.status).toBe('joined')
  expect(await asAna.isMember(conference.space)).toBe(true)
})

test('TC-17: A handle on the list stays with the account it first named', async ({ viv }) => {
  const { conference } = await seedConference(dep, accountsIn(viv), { methods: ['list'] })
  const ana = await viv.createAccount(viv.handle('ana'))
  const mallory = await viv.createAccount(viv.handle('mallory'))
  const listed = ana.handle
  await importList(dep, conference, [{ handle: listed }])

  // Ana moves on to a new handle, and Mallory takes the old one.
  await changeHandle(viv.url, ana, viv.handle('ana-renamed'))
  await changeHandle(viv.url, mallory, listed)

  const asMallory = await Attendee.signIn(dep, { ...mallory, handle: listed })
  const answer = await asMallory.join({ conference: conference.space })
  expect(answer.body.status).not.toBe('joined')
  expect(await asMallory.isMember(conference.space)).toBe(false)

  // The place on the list is still Ana's.
  const asAna = await Attendee.signIn(dep, { ...ana, handle: viv.handle('ana-renamed') })
  expect((await asAna.join({ conference: conference.space })).body.status).toBe('joined')
})

test('TC-22: An open conference admits anyone signed in', async ({ viv }) => {
  const { conference } = await seedConference(dep, accountsIn(viv), { methods: ['open'] })
  const mallory = await Attendee.signIn(dep, await viv.createAccount(viv.handle('mallory')))

  const answer = await mallory.join({ conference: conference.space })
  expect(answer.status).toBe(200)
  expect(answer.body.status).toBe('joined')
  expect(await mallory.isMember(conference.space)).toBe(true)
})

test('TC-23: A conference with no way in for you refuses', async ({ viv }) => {
  const { conference } = await seedConference(dep, accountsIn(viv), { methods: ['list'] })
  await importList(dep, conference, [{ handle: viv.handle('someone-else') }])
  const mallory = await viv.createAccount(viv.handle('mallory'))
  const asMallory = await Attendee.signIn(dep, mallory)

  const answer = await asMallory.join({ conference: conference.space })
  expect(answer.status).toBe(200)
  expect(answer.body.status).toBe('refused')
  expect(await asMallory.isMember(conference.space)).toBe(false)

  // Nothing created for her: no join record in her repo, no request waiting.
  expect(await ownSpaceRecords(viv.url, mallory, conference.intake, 'app.eventside.intake.join')).toHaveLength(0)
  const requests = await cliOkJson(dep, ['requests', 'list', '--conference', conference.space])
  expect(requests.requests.map((r: { did: string }) => r.did)).not.toContain(mallory.did)
})

test('TC-24: Too many join attempts are slowed down', async ({ viv }) => {
  const { conference } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  const valid = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const mallory = await Attendee.signIn(dep, await viv.createAccount(viv.handle('mallory')))

  for (let attempt = 1; attempt <= 10; attempt++) {
    const answer = await mallory.join({ conference: conference.space, code: uniqueCode(`guess${attempt}`) })
    expect(answer.body.error, `attempt ${attempt}`).toBe('InvalidCode')
  }
  // The 11th within the minute isn't checked: even a valid code waits.
  const answer = await mallory.join({ conference: conference.space, code: valid })
  expect(answer.status).toBe(429)
  expect(answer.body.error).toBe('RateLimitExceeded')
  expect(await mallory.isMember(conference.space)).toBe(false)
})

test('TC-28: A banned person can’t get back in by any method', async ({ viv }) => {
  const { conference, superAdmin } = await seedConference(dep, accountsIn(viv), {
    methods: ['code', 'list', 'request'],
  })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const bram = await viv.createAccount(viv.handle('bram'))
  const asBram = await Attendee.signIn(dep, bram)
  expect((await asBram.join({ conference: conference.space, code })).body.status).toBe('joined')

  const banned = await memberAction(dep, conference, ['member', 'ban'], bram)
  expect(banned.code, banned.stderr).toBe(0)
  expect(await asBram.isMember(conference.space)).toBe(false)

  await importList(dep, conference, [{ handle: bram.handle }])
  for (const attempt of [
    { conference: conference.space, code },
    { conference: conference.space },
    { conference: conference.space, request: true },
  ]) {
    const answer = await asBram.join(attempt)
    expect(answer.body.status, JSON.stringify(attempt)).toBe('refused')
  }
  expect(await asBram.isMember(conference.space)).toBe(false)

  // The space's member list never has him back.
  const app = await OtherApp.start(viv.url)
  try {
    await allowApp(dep, conference, app.clientId)
    const access = await (await app.signIn(superAdmin)).open(conference.space)
    expect(isCurrent(memberEntry(await access.members(), bram.did))).toBe(false)
  } finally {
    await app.stop()
  }
})

test('TC-31: A speaker assigned before joining gets the role on joining', async ({ viv }) => {
  const { conference, superAdmin } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const ana = await viv.createAccount(viv.handle('ana'))
  const assigned = await setRole(dep, conference, ana, 'speaker')
  expect(assigned.code, assigned.stderr).toBe(0)

  const asAna = await Attendee.signIn(dep, ana)
  const answer = await asAna.join({ conference: conference.space })
  expect(answer.body.status, 'admitted without a code').toBe('joined')
  expect(answer.body.role).toBe('speaker')

  const app = await OtherApp.start(viv.url)
  try {
    await allowApp(dep, conference, app.clientId)
    const access = await (await app.signIn(ana)).open(conference.space)
    const roles = await eventually(
      () => access.records(superAdmin.did, 'app.eventside.conference.role'),
      (records) => records.some((r) => r.value.subject === ana.did),
      'Ana’s role record is there',
    )
    expect(roles.map((r) => r.value)).toContainEqual(expect.objectContaining({ subject: ana.did, role: 'speaker' }))
  } finally {
    await app.stop()
  }
})

test('TC-54: Joining doesn’t depend on the super admin’s PDS', async ({ viv }) => {
  const { conference, superAdmin } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const ana = await Attendee.signIn(dep, await viv.createAccount(viv.handle('ana')))

  await takeOffline(viv.url, superAdmin)
  try {
    const answer = await ana.join({ conference: conference.space, code })
    expect(answer.status).toBe(200)
    expect(answer.body.status).toBe('joined')
    expect(await ana.isMember(conference.space)).toBe(true)
  } finally {
    await bringOnline(viv.url, superAdmin)
  }
})

test('TC-55: An admin action fails while that admin’s PDS is down', async ({ viv }) => {
  const { conference, admins } = await seedConference(dep, accountsIn(viv), { staff: ['pim'], methods: ['request'] })
  const pim = admins.pim
  const bram = await viv.createAccount(viv.handle('bram'))
  const asBram = await Attendee.signIn(dep, bram)
  expect((await asBram.join({ conference: conference.space })).body.status).toBe('pending')

  await takeOffline(viv.url, pim)
  const failed = await memberAction(dep, conference, ['requests', 'approve'], bram, { as: pim }).finally(() =>
    bringOnline(viv.url, pim),
  )
  expect(failed.code, 'the CLI reports the failure').not.toBe(0)
  expect(failed.stderr.trim()).not.toBe('')
  expect(await asBram.isMember(conference.space)).toBe(false)

  const retried = await memberAction(dep, conference, ['requests', 'approve'], bram, { as: pim })
  expect(retried.code, retried.stderr).toBe(0)
  expect(await asBram.isMember(conference.space)).toBe(true)
})

test('TC-57: A denied request can be made again; a ban can’t', async ({ viv }) => {
  const { conference } = await seedConference(dep, accountsIn(viv), { methods: ['request'] })
  const bram = await viv.createAccount(viv.handle('bram'))
  const asBram = await Attendee.signIn(dep, bram)
  expect((await asBram.join({ conference: conference.space })).body.status).toBe('pending')
  const denied = await memberAction(dep, conference, ['requests', 'deny'], bram)
  expect(denied.code, denied.stderr).toBe(0)
  expect(await asBram.isMember(conference.space)).toBe(false)

  const again = await asBram.join({ conference: conference.space })
  expect(again.body.status).toBe('pending')
  const requests = await cliOkJson(dep, ['requests', 'list', '--conference', conference.space])
  expect(requests.requests.map((r: { did: string }) => r.did)).toContain(bram.did)

  // Had he been banned instead, it would be refused.
  const kees = await viv.createAccount(viv.handle('kees'))
  const asKees = await Attendee.signIn(dep, kees)
  expect((await asKees.join({ conference: conference.space })).body.status).toBe('pending')
  const banned = await memberAction(dep, conference, ['member', 'ban'], kees)
  expect(banned.code, banned.stderr).toBe(0)
  expect((await asKees.join({ conference: conference.space })).body.status).toBe('refused')
})
