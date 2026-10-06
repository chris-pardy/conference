import { existsSync, readFileSync } from 'node:fs'
import { expect, type Page, test } from '@playwright/test'
import {
  createAccountWithEmail,
  ownSpaceRecords,
  ownSpaceRev,
  parseAtUri,
  repoRev,
  writeSpaceRecord,
} from '../tests/support/atproto.ts'
import {
  ATMOSPHERECONF,
  Attendee,
  allowApp,
  type Conference,
  cliOkJson,
  type Deployment,
  e2eDeployment,
  importList,
  memberAction,
  NSID,
  seedConference,
  sharedCode,
  uniqueCode,
  WEDDING,
} from '../tests/support/conference.ts'
import { isCurrent, memberEntry, OtherApp } from '../tests/support/other-app.ts'
import { consent, createAttendee, header, signInAs, signInThroughUi, uniqueHandle, vivarium } from './support/auth.ts'

// Conferences in the PWA: the public page, joining, the inside, and leaving.
// Each test seeds its own organization through the admin CLI, against the
// run's server.

test.describe.configure({ timeout: 120_000 })

const newAccount = (name: string) => createAttendee(name)
const deployment = (baseURL: string | undefined): Deployment => e2eDeployment(baseURL as string)

/** A public conference's page: `/c/{super admin's DID or handle}/{event rkey}`. */
function publicPage(conference: Conference, actor?: string): string {
  const { repo, rkey } = parseAtUri(conference.event as string)
  return `/c/${actor ?? repo}/${rkey}`
}

const main = (page: Page) => page.getByRole('main')
const leaveControl = (page: Page) => main(page).getByRole('button', { name: /leave/i })
const joinControl = (page: Page) => main(page).getByRole('button', { name: /^join$/i })
const requestControl = (page: Page) => main(page).getByRole('button', { name: /request to join/i })
const codeEntry = (page: Page) =>
  main(page)
    .getByRole('textbox', { name: /code/i })
    .or(main(page).getByRole('button', { name: /enter a code/i }))

/** Enters an invite code on the public page and joins with it. */
async function joinWithCode(page: Page, code: string) {
  const field = main(page).getByRole('textbox', { name: /code/i })
  if (!(await field.isVisible()))
    await main(page)
      .getByRole('button', { name: /enter a code/i })
      .click()
  await field.fill(code)
  await joinControl(page).click()
}

/** The theme's primary color, as the page has it. */
const primaryColor = (page: Page) =>
  page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--g-color-primary').trim())

/** Bruiloft and its invite-only wedding, with a shared code. */
async function seedWedding(dep: Deployment) {
  const seeded = await seedConference(dep, newAccount, { superAdmin: 'sanne', spec: WEDDING, methods: ['code'] })
  const code = await sharedCode(dep, seeded.conference, uniqueCode('trouwen'))
  return { ...seeded, code }
}

test('TC-9: A non-member sees the public page and how to get in', async ({ page, baseURL }) => {
  const dep = deployment(baseURL)
  const { conference, superAdmin } = await seedConference(dep, newAccount, { methods: ['code', 'request'] })
  await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  // Something inside, which the public page must not show.
  await writeSpaceRecord(vivarium().url, superAdmin, conference.space, NSID.announcement, {
    text: 'Speaker dinner at De Kas',
    createdAt: new Date().toISOString(),
  })
  const mallory = await createAttendee('mallory')
  await signInAs(page, baseURL as string, mallory.handle)

  await page.goto(publicPage(conference))
  await expect(main(page).getByRole('heading', { name: 'AtmosphereConf' })).toBeVisible()
  await expect(
    main(page)
      .getByText(/amsterdam/i)
      .first(),
  ).toBeVisible()
  await expect(
    main(page)
      .getByText(/29\s*apr|apr\w*\s*29/i)
      .first(),
  ).toBeVisible()
  await expect(main(page).getByText(ATMOSPHERECONF.description as string)).toBeVisible()
  await expect(codeEntry(page).first()).toBeVisible()
  await expect(requestControl(page)).toBeVisible()

  await expect(page.getByText('Speaker dinner at De Kas')).toHaveCount(0)
  await expect(leaveControl(page)).toHaveCount(0)
})

test('TC-10: The public page works by DID or by handle', async ({ page, baseURL }) => {
  const dep = deployment(baseURL)
  const { conference, superAdmin } = await seedConference(dep, newAccount, { methods: ['code'] })

  await page.goto(publicPage(conference))
  await expect(main(page).getByRole('heading', { name: 'AtmosphereConf' })).toBeVisible()
  await page.goto(publicPage(conference, superAdmin.handle))
  await expect(main(page).getByRole('heading', { name: 'AtmosphereConf' })).toBeVisible()
  await expect(main(page).getByText(ATMOSPHERECONF.description as string)).toBeVisible()
})

test('TC-11: Ana joins with a shared invite code', async ({ page, baseURL }) => {
  const dep = deployment(baseURL)
  const viv = vivarium()
  const { conference, superAdmin } = await seedConference(dep, newAccount, { methods: ['code'] })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const ana = await createAttendee('ana')
  const adminSpace = conference.org.adminSpace
  const olgaBefore = {
    public: await repoRev(viv.url, superAdmin.did),
    admin: await ownSpaceRev(viv.url, superAdmin, adminSpace),
    conference: await ownSpaceRev(viv.url, superAdmin, conference.space),
  }

  await signInAs(page, baseURL as string, ana.handle)
  await page.goto(publicPage(conference))
  await joinWithCode(page, code)
  await expect(leaveControl(page)).toBeVisible()
  const joinedAt = Date.now()

  // Her join is a record in her own repository…
  const joins = await ownSpaceRecords(viv.url, ana, conference.intake, NSID.join)
  expect(joins.map((r) => r.value.code)).toContain(code)
  // …and Olga's repository didn't change for it.
  expect({
    public: await repoRev(viv.url, superAdmin.did),
    admin: await ownSpaceRev(viv.url, superAdmin, adminSpace),
    conference: await ownSpaceRev(viv.url, superAdmin, conference.space),
  }).toEqual(olgaBefore)

  // Another app asking our server sees her, with read and write access, from now.
  const app = await OtherApp.start(viv.url)
  try {
    await allowApp(dep, conference, app.clientId)
    const access = await (await app.signIn(superAdmin)).open(conference.space)
    const entry = memberEntry(await access.members(), ana.did)
    expect(entry).toMatchObject({ read: true, write: true })
    expect(isCurrent(entry)).toBe(true)
    expect(Math.abs(Date.parse(entry?.periods.at(-1).since) - joinedAt)).toBeLessThan(60_000)
  } finally {
    await app.stop()
  }
})

test('TC-15: An invite link opens an invite-only conference', async ({ page, baseURL }) => {
  const dep = deployment(baseURL)
  const { code } = await seedWedding(dep)
  const ana = await createAttendee('ana')

  await page.goto(`/join/${code}`)
  await signInThroughUi(page, ana.handle)
  await expect(page).toHaveURL(new RegExp(`/join/${code}`))
  await expect(leaveControl(page).or(joinControl(page))).toBeVisible()
  if (await joinControl(page).isVisible()) await joinControl(page).click()

  await expect(leaveControl(page)).toBeVisible()
  await expect(main(page).getByRole('heading', { name: WEDDING.name })).toBeVisible()
  await expect.poll(() => primaryColor(page)).toBe(WEDDING.theme?.['color-primary'])
})

test('TC-18: Being on the list by email asks for a verified email, once', async ({ page, baseURL }) => {
  const dep = deployment(baseURL)
  const viv = vivarium()
  const { conference } = await seedConference(dep, newAccount, { methods: ['list', 'request'] })
  const handle = uniqueHandle('bram')
  const email = `${handle}@example.com`
  const bram = await createAccountWithEmail(viv.url, handle, email)
  await importList(dep, conference, [{ email }])

  await signInAs(page, baseURL as string, bram.handle)
  await page.goto(publicPage(conference))
  await joinControl(page).click()
  const verify = main(page).getByRole('button', { name: /verify my email/i })
  await expect(verify).toBeVisible()
  await expect(requestControl(page)).toBeVisible()

  await verify.click()
  await consent(page, bram.handle)
  await expect(leaveControl(page)).toBeVisible()

  // His email isn't kept, or shown, anywhere in the app.
  for (const file of [dep.databasePath as string, `${dep.databasePath}-wal`]) {
    if (existsSync(file)) expect(readFileSync(file).includes(email), file).toBe(false)
  }
  for (const path of [
    '/xrpc/app.eventside.auth.getSession',
    `/xrpc/app.eventside.conference.getConference?conference=${encodeURIComponent(conference.space)}`,
    '/xrpc/app.eventside.conference.listMyConferences',
  ]) {
    expect(await (await page.request.get(path)).text(), path).not.toContain(email)
  }
  await expect(page.getByText(email)).toHaveCount(0)
})

test('TC-20: Requesting to join, then being approved', async ({ page, baseURL }) => {
  const dep = deployment(baseURL)
  const { conference } = await seedConference(dep, newAccount, { methods: ['request'] })
  const bram = await createAttendee('bram')
  await signInAs(page, baseURL as string, bram.handle)

  await page.goto(publicPage(conference))
  await requestControl(page).click()
  await expect(main(page).getByText(/pending/i)).toBeVisible()
  await expect(leaveControl(page)).toHaveCount(0)
  const waiting = await cliOkJson(dep, ['requests', 'list', '--conference', conference.space])
  expect(waiting.requests.map((r: { did: string }) => r.did)).toContain(bram.did)

  expect((await memberAction(dep, conference, ['requests', 'approve'], bram)).code).toBe(0)
  await page.reload()
  await expect(leaveControl(page)).toBeVisible()
})

test('TC-21: A denied request', async ({ page, baseURL }) => {
  const dep = deployment(baseURL)
  const { conference } = await seedConference(dep, newAccount, { methods: ['request'] })
  const bram = await createAttendee('bram')
  await signInAs(page, baseURL as string, bram.handle)
  await page.goto(publicPage(conference))
  await requestControl(page).click()
  await expect(main(page).getByText(/pending/i)).toBeVisible()

  expect((await memberAction(dep, conference, ['requests', 'deny'], bram)).code).toBe(0)
  await page.reload()
  await expect(main(page).getByText(/(weren.t|not|wasn.t) (been )?admitted/i)).toBeVisible()
  await expect(leaveControl(page)).toHaveCount(0)
  const viewer = (
    await (
      await page.request.get(
        `/xrpc/app.eventside.conference.getConference?conference=${encodeURIComponent(conference.space)}`,
      )
    ).json()
  ).viewer
  expect(viewer.member).toBe(false)
  expect(viewer.reason, 'no reason is given').toBeUndefined()
})

test('TC-26: Ana leaves, and can come back', async ({ page, baseURL }) => {
  const dep = deployment(baseURL)
  const { conference } = await seedConference(dep, newAccount, { methods: ['code'] })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const ana = await createAttendee('ana')
  await signInAs(page, baseURL as string, ana.handle)
  await page.goto(publicPage(conference))
  await joinWithCode(page, code)
  await expect(leaveControl(page)).toBeVisible()

  page.on('dialog', (dialog) => dialog.accept())
  await leaveControl(page).first().click()
  // A confirmation inside the page, if there is one.
  const confirm = page.getByRole('dialog').getByRole('button', { name: /leave/i })
  if (await confirm.isVisible()) await confirm.click()
  await expect(leaveControl(page)).toHaveCount(0)
  await expect(codeEntry(page).first()).toBeVisible()

  await joinWithCode(page, code)
  await expect(leaveControl(page)).toBeVisible()
})

test('TC-40: Ana is in two differently branded conferences with one account', async ({ page, baseURL }) => {
  const dep = deployment(baseURL)
  const atmosphereTheme = { 'color-primary': '#1d6b4f' }
  const { conference: atmosphere } = await seedConference(dep, newAccount, {
    spec: { ...ATMOSPHERECONF, theme: atmosphereTheme },
    methods: ['code'],
  })
  const atmosphereCode = await sharedCode(dep, atmosphere, uniqueCode('atmosphere27'))
  const { conference: wedding, code: weddingCode } = await seedWedding(dep)
  const ana = await createAttendee('ana')
  const asAna = await Attendee.signIn(dep, ana)
  expect((await asAna.join({ code: atmosphereCode })).body.status).toBe('joined')
  expect((await asAna.join({ code: weddingCode })).body.status).toBe('joined')
  expect(wedding.space).not.toBe(atmosphere.space)

  await signInAs(page, baseURL as string, ana.handle)
  await page.goto('/conferences')
  const atmosphereLink = main(page).getByRole('link', { name: /AtmosphereConf/ })
  const weddingLink = main(page).getByRole('link', { name: WEDDING.name })
  await expect(atmosphereLink).toBeVisible()
  await expect(weddingLink).toBeVisible()

  await atmosphereLink.click()
  await expect(main(page).getByRole('heading', { name: 'AtmosphereConf' })).toBeVisible()
  await expect(leaveControl(page)).toBeVisible()
  await expect.poll(() => primaryColor(page)).toBe(atmosphereTheme['color-primary'])
  await expect(page.getByText(WEDDING.name)).toHaveCount(0)

  await page.goto('/conferences')
  await weddingLink.click()
  await expect(main(page).getByRole('heading', { name: WEDDING.name })).toBeVisible()
  await expect(leaveControl(page)).toBeVisible()
  await expect.poll(() => primaryColor(page)).toBe(WEDDING.theme?.['color-primary'])
  await expect(page.getByText('AtmosphereConf')).toHaveCount(0)
})

test('TC-42: The block gallery still works signed out', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByText(/backend.*up/i)).toBeVisible()
  await expect(header(page).getByRole('link', { name: /^sign in$/i })).toBeVisible()
  await page.goto('/dev/blocks')
  await expect(page.getByRole('heading', { level: 1, name: /blocks/i })).toBeVisible()
  await expect(page.locator('[data-block]').first()).toBeVisible()
})
