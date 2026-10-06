import { expect, type Page, test } from '@playwright/test'
import { consent, createAttendee, header, signInControl, uniqueHandle } from '../e2e/support/auth.ts'

// attendee-sign-in on a phone, against the run's vivarium: signing in with a
// handle (TC-1, TC-2), the signed-in header, signing out (TC-15), creating an
// account (TC-8), and an idle session expiring, then signing in again from
// the banner (TC-16). The demo backend's idle timeout is ten seconds.

const pause = (page: Page, ms = 1200) => page.waitForTimeout(ms)
const GALLERY = '/dev/blocks'

async function signOut(page: Page, handle: string) {
  await header(page)
    .getByRole('button', { name: new RegExp(handle.replaceAll('.', '\\.')) })
    .click()
  await pause(page)
  await page
    .getByRole('menuitem', { name: /sign out/i })
    .or(page.getByRole('button', { name: /sign out/i }))
    .click()
  await expect(signInControl(page)).toBeVisible()
}

test('attendee-sign-in demo', async ({ page }) => {
  test.setTimeout(180_000)
  const ana = await createAttendee('ana', { displayName: 'Ana de Vries' })

  // Signed out: the block gallery works, with "Sign in" in the header.
  await page.goto(GALLERY)
  await expect(page.locator('[data-block]').first()).toBeVisible()
  await pause(page, 2000)

  // Sign in with a handle, then consent at the PDS.
  await signInControl(page).click()
  await pause(page)
  await page
    .getByRole('main')
    .getByLabel(/handle/i)
    .pressSequentially(ana.handle, { delay: 40 })
  await pause(page, 800)
  await page
    .getByRole('main')
    .getByRole('button', { name: /^sign in$/i })
    .click()
  await pause(page, 1500)
  await consent(page, ana.handle)

  // Back on the page she started from, with her avatar and handle.
  await expect(page).toHaveURL(new RegExp(`${GALLERY}$`))
  await expect(header(page).getByText(ana.handle)).toBeVisible()
  await pause(page, 2500)

  // Sign out from the account menu.
  await signOut(page, ana.handle)
  await pause(page, 2000)

  // A newcomer creates an account and comes back signed in.
  const newcomer = uniqueHandle('newcomer')
  await signInControl(page).click()
  await pause(page)
  await page
    .getByRole('main')
    .getByRole('link', { name: /create account/i })
    .or(page.getByRole('main').getByRole('button', { name: /create account/i }))
    .click()
  await pause(page, 1500)
  await consent(page, newcomer)
  await expect(header(page).getByText(newcomer)).toBeVisible()
  await pause(page, 2500)
  await signOut(page, newcomer)
  await pause(page, 1500)

  // Ana signs in again, then leaves the app idle until her session expires.
  await signInControl(page).click()
  await page
    .getByRole('main')
    .getByLabel(/handle/i)
    .fill(ana.handle)
  await page
    .getByRole('main')
    .getByRole('button', { name: /^sign in$/i })
    .click()
  await consent(page, ana.handle)
  await expect(header(page).getByText(ana.handle)).toBeVisible()
  await pause(page, 11_000)
  await page.goto(GALLERY)
  const banner = page.getByRole('alert').filter({ hasText: /session (has )?expired/i })
  await expect(banner).toBeVisible()
  await pause(page, 2500)

  // "Sign in again" goes straight to consent, then back to the gallery.
  await banner
    .getByRole('button', { name: /sign in again/i })
    .or(banner.getByRole('link', { name: /sign in again/i }))
    .click()
  await pause(page, 1500)
  await consent(page, ana.handle)
  await expect(page).toHaveURL(new RegExp(`${GALLERY}$`))
  await expect(header(page).getByText(ana.handle)).toBeVisible()
  await expect(page.getByText(/session (has )?expired/i)).toHaveCount(0)
  await pause(page, 2500)
})
