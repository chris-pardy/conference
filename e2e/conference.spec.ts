import { expect, type Page, test } from '@playwright/test'
import { parseAtUri, writeSpaceRecord } from '../tests/support/atproto.ts'
import {
  ATMOSPHERECONF,
  type Conference,
  type Deployment,
  e2eDeployment,
  importList,
  joinWithCode,
  NSID,
  post,
  seedConference,
  sharedCode,
} from '../tests/support/conference.ts'
import { createAttendee, signInAs, vivarium } from './support/auth.ts'

// A conference's public page in the PWA, and joining from it. Each test seeds
// its own organization through the admin CLI, against the run's server.

test.describe.configure({ timeout: 180_000 })

const deployment = (baseURL: string | undefined): Deployment => e2eDeployment(baseURL as string)

/** A public conference's page: `/c/{organization's DID or handle}/{event rkey}`. */
function publicPage(conference: Conference, actor?: string): string {
  const { repo, rkey } = parseAtUri(conference.event)
  return `/c/${actor ?? repo}/${rkey}`
}

const main = (page: Page) => page.getByRole('main')
const leaveControl = (page: Page) => main(page).getByRole('button', { name: /leave/i })
const codeField = (page: Page) => main(page).getByRole('textbox', { name: /code/i })
const codeEntry = (page: Page) => codeField(page).or(main(page).getByRole('button', { name: /enter a code/i }))

/** The theme's primary color, as the page has it. */
const primaryColor = (page: Page) =>
  page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--g-color-primary').trim())

test('TC-6: A non-member sees the public page and how to get in', async ({ page, baseURL }) => {
  const dep = deployment(baseURL)
  const { conference, olga } = await seedConference(dep, createAttendee, { methods: ['code', 'list'] })
  await sharedCode(dep, conference, 'atmosphere27', olga)
  const listed = await createAttendee('listed')
  expect((await importList(dep, conference, [listed.handle], olga)).code).toBe(0)
  // Something inside, which the public page must not show.
  const ana = await createAttendee('ana')
  await joinWithCode(dep, ana, conference, 'atmosphere27')
  await writeSpaceRecord(vivarium().url, ana, conference.space, NSID.post, post('Speaker dinner at De Kas'))

  await page.goto(publicPage(conference))
  await expect(main(page).getByRole('heading', { name: ATMOSPHERECONF.name })).toBeVisible()
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
  expect((await primaryColor(page)).toLowerCase()).toBe(ATMOSPHERECONF.theme?.['color-primary'])

  // How to get in: a code, and "Sign in" for people on the list.
  await expect(codeEntry(page).first()).toBeVisible()
  await expect(
    main(page)
      .getByRole('link', { name: /^sign in$/i })
      .or(main(page).getByRole('button', { name: /^sign in$/i }))
      .first(),
  ).toBeVisible()

  // Nothing from inside.
  await expect(page.getByText('Speaker dinner at De Kas')).toHaveCount(0)
  await expect(page.getByText(ana.handle)).toHaveCount(0)
  await expect(leaveControl(page)).toHaveCount(0)
})

test('TC-7: The public page works by DID or by handle', async ({ page, baseURL }) => {
  const dep = deployment(baseURL)
  const { conference, org } = await seedConference(dep, createAttendee, { methods: ['code'] })

  await page.goto(publicPage(conference))
  await expect(main(page).getByRole('heading', { name: ATMOSPHERECONF.name })).toBeVisible()
  const byDid = await main(page).innerText()

  await page.goto(publicPage(conference, org.handle))
  await expect(main(page).getByRole('heading', { name: ATMOSPHERECONF.name })).toBeVisible()
  await expect(main(page).getByText(ATMOSPHERECONF.description as string)).toBeVisible()
  expect(await main(page).innerText()).toBe(byDid)
})

test('TC-8: Ana joins with a shared invite code', async ({ page, baseURL }) => {
  const dep = deployment(baseURL)
  const { conference, olga } = await seedConference(dep, createAttendee, { methods: ['code'] })
  await sharedCode(dep, conference, 'atmosphere27', olga)
  const ana = await createAttendee('ana')

  await signInAs(page, baseURL as string, ana.handle)
  await page.goto(publicPage(conference))
  if (!(await codeField(page).isVisible())) {
    await main(page)
      .getByRole('button', { name: /enter a code/i })
      .click()
  }
  await codeField(page).fill('atmosphere27')
  await main(page)
    .getByRole('button', { name: /^join$/i })
    .click()

  // She lands inside: the way in is gone, and she can leave.
  await expect(leaveControl(page)).toBeVisible()
  await expect(codeField(page)).toHaveCount(0)
})
