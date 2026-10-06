import { expect, test } from '@playwright/test'
import {
  consent,
  createAccountControl,
  createAttendee,
  header,
  signInAs,
  signInControl,
  signInForm,
  signInThroughUi,
  uniqueHandle,
} from './support/auth.ts'

const GALLERY = '/dev/blocks'

test('TC-1: Ana signs in with her handle', async ({ page }) => {
  const ana = await createAttendee('ana', { displayName: 'Ana de Vries' })
  await page.goto('/')
  await signInThroughUi(page, ana.handle)

  await expect(page).toHaveURL(/^http:\/\/127\.0\.0\.1:\d+\/$/)
  await expect(header(page).getByText(ana.handle)).toBeVisible()
  const avatar = header(page).locator('img')
  await expect(avatar).toBeVisible()
  expect(await avatar.evaluate((img: HTMLImageElement) => img.complete && img.naturalWidth > 0)).toBe(true)
  await expect(signInControl(page)).toHaveCount(0)
})

test('TC-2: signing in returns Ana to the page she started from', async ({ page }) => {
  const ana = await createAttendee('ana')
  await page.goto(GALLERY)
  await signInThroughUi(page, ana.handle)

  await expect(page).toHaveURL(new RegExp(`${GALLERY}$`))
  await expect(header(page).getByText(ana.handle)).toBeVisible()
  await expect(page.locator('[data-block]').first()).toBeVisible()
})

test('TC-3: an account without a profile still signs in', async ({ page }) => {
  const bram = await createAttendee('bram')
  await page.goto('/')
  await signInThroughUi(page, bram.handle)

  await expect(header(page).getByText(bram.handle)).toBeVisible()
  await expect(header(page).getByText('B', { exact: true })).toBeVisible()
  await expect(header(page).locator('img')).toHaveCount(0)
})

test('TC-4: an unknown handle is reported, not sent anywhere', async ({ page, baseURL }) => {
  await page.goto('/')
  await signInControl(page).click()
  await signInForm(page, uniqueHandle('nobody'))

  await expect(page.getByText(/couldn.t find that handle/i)).toBeVisible()
  expect(new URL(page.url()).origin).toBe(baseURL)
  await expect(signInControl(page)).toBeVisible()
})

test('TC-5: declining consent leaves Ana signed out', async ({ page, baseURL }) => {
  const ana = await createAttendee('ana')
  await page.goto('/')
  await signInControl(page).click()
  await signInForm(page, ana.handle)
  await consent(page, ana.handle, 'Deny')

  await expect(page.getByText(/sign-in was cancelled/i)).toBeVisible()
  expect(new URL(page.url()).origin).toBe(baseURL)
  await expect(signInControl(page)).toBeVisible()
})

test('TC-6: picking a different account at the PDS is refused', async ({ page }) => {
  const ana = await createAttendee('ana')
  const bram = await createAttendee('bram')
  await page.goto('/')
  await signInThroughUi(page, ana.handle, bram.handle)

  await expect(page.getByText(/account didn.t match/i)).toBeVisible()
  await expect(signInControl(page)).toBeVisible()
  await expect(header(page).getByText(bram.handle)).toHaveCount(0)
})

test('TC-8: a newcomer creates an account and comes back signed in', async ({ page }) => {
  const handle = uniqueHandle('newcomer')
  await page.goto('/')
  await signInControl(page).click()
  await createAccountControl(page).click()
  // Vivarium makes the account on the fly when a new handle is typed.
  await consent(page, handle)

  await expect(header(page).getByText(handle)).toBeVisible()
  await expect(signInControl(page)).toHaveCount(0)
})

test('TC-10: Ana stays signed in when she closes and reopens the app', async ({ page, browser, baseURL }) => {
  const ana = await createAttendee('ana')
  await page.goto('/')
  await signInThroughUi(page, ana.handle)
  await expect(header(page).getByText(ana.handle)).toBeVisible()

  // The cookie outlives the browser session.
  const cookie = (await page.context().cookies()).find((c) => c.name === 'session')
  expect(cookie, 'a session cookie should be set').toBeDefined()
  expect(cookie?.httpOnly).toBe(true)
  expect(cookie?.expires, 'the cookie should persist, not end with the browser').toBeGreaterThan(
    Date.now() / 1000 + 24 * 60 * 60,
  )

  // Close the app, then open it again from what the browser kept.
  const state = await page.context().storageState()
  await page.context().close()
  const reopened = await browser.newContext({ storageState: state, baseURL })
  const again = await reopened.newPage()
  await again.goto('/')
  await expect(header(again).getByText(ana.handle)).toBeVisible()
  await reopened.close()
})

test('TC-15: Ana signs out from the account menu', async ({ page, baseURL }) => {
  const ana = await createAttendee('ana')
  await signInAs(page, baseURL as string, ana.handle)
  await page.goto('/')
  await expect(header(page).getByText(ana.handle)).toBeVisible()

  await header(page)
    .getByRole('button', { name: new RegExp(ana.handle.replaceAll('.', '\\.')) })
    .click()
  await page
    .getByRole('menuitem', { name: /sign out/i })
    .or(page.getByRole('button', { name: /sign out/i }))
    .click()

  await expect(signInControl(page)).toBeVisible()
  await expect(page.getByText(/session (has )?expired/i)).toHaveCount(0)
  await page.reload()
  await expect(signInControl(page)).toBeVisible()
})

test('TC-16: an idle session expires, and Ana gets back to her page', async ({ page, baseURL }) => {
  test.setTimeout(60_000)
  const ana = await createAttendee('ana')
  await signInAs(page, baseURL as string, ana.handle)
  await page.goto('/')
  await expect(header(page).getByText(ana.handle)).toBeVisible()

  // The e2e backend's idle timeout is ten seconds.
  await page.waitForTimeout(11_000)
  await page.goto(GALLERY)
  const banner = page.getByRole('alert').filter({ hasText: /session (has )?expired/i })
  await expect(banner).toBeVisible()

  await banner
    .getByRole('button', { name: /sign in again/i })
    .or(banner.getByRole('link', { name: /sign in again/i }))
    .click()
  // No handle to retype: straight to the consent screens.
  await consent(page, ana.handle)

  await expect(page).toHaveURL(new RegExp(`${GALLERY}$`))
  await expect(header(page).getByText(ana.handle)).toBeVisible()
  await expect(page.getByText(/session (has )?expired/i)).toHaveCount(0)
})

test('TC-29: the home page and block gallery work signed out', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByText(/backend.*up/i)).toBeVisible()
  await expect(page.getByText(/atproto.*reachable/i)).toBeVisible()
  await expect(signInControl(page)).toBeVisible()

  await page.goto(GALLERY)
  await expect(page.getByRole('heading', { level: 1, name: /blocks/i })).toBeVisible()
  await expect(page.locator('[data-block]').first()).toBeVisible()
  await expect(signInControl(page)).toBeVisible()
})

test.describe('with the service worker active', () => {
  test.use({ serviceWorkers: 'allow' })

  test('TC-30: the installed app doesn’t swallow sign-in', async ({ page }) => {
    const ana = await createAttendee('ana')
    // Install the service worker, then open the app again so it's in control,
    // as it is whenever an installed app is launched.
    await page.goto('/')
    await page.evaluate(() => navigator.serviceWorker.ready)
    await page.reload()
    const controlled = await page.evaluate(() => navigator.serviceWorker.controller !== null)
    expect(controlled, 'the service worker should control the page').toBe(true)

    await signInThroughUi(page, ana.handle)
    await expect(header(page).getByText(ana.handle)).toBeVisible()
  })
})
