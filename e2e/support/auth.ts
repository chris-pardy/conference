import { randomBytes } from 'node:crypto'
import { expect, type Locator, type Page } from '@playwright/test'
import { VivariumClient } from '@vivarium-dev/client'
import { giveProfile, signIn } from '../../tests/support/auth.ts'

export function vivarium(): VivariumClient {
  const url = process.env.VIVARIUM_URL
  if (!url) throw new Error('e2e needs the run vivarium in VIVARIUM_URL: use `pnpm test:e2e`')
  return new VivariumClient(url)
}

/** A unique handle on the run's vivarium. */
export const uniqueHandle = (name: string) => `${name}-${randomBytes(4).toString('hex')}.vivarium.test`

/** A new vivarium account, optionally with a profile (display name and avatar). */
export async function createAttendee(name: string, opts: { displayName?: string } = {}) {
  const viv = vivarium()
  const account = await viv.createAccount(uniqueHandle(name))
  if (opts.displayName) await giveProfile(viv.url, account, opts.displayName)
  return account
}

/** Signs a browser in without the UI: the real flow over HTTP, then its cookies. */
export async function signInAs(page: Page, baseURL: string, handle: string): Promise<void> {
  const { jar } = await signIn(baseURL, handle)
  const value = jar.get(baseURL, 'session')
  expect(value, 'signing in should set a session cookie').toBeTruthy()
  const { hostname } = new URL(baseURL)
  await page.context().addCookies([{ name: 'session', value: value as string, domain: hostname, path: '/' }])
}

/** The app's header. */
export const header = (page: Page): Locator => page.getByRole('banner')

/** The header's "Sign in" control, a link or a button. */
export const signInControl = (page: Page): Locator =>
  header(page)
    .getByRole('link', { name: /^sign in$/i })
    .or(header(page).getByRole('button', { name: /^sign in$/i }))

/** Finishes sign-in on vivarium's consent screens, as the person would. */
export async function consent(page: Page, account: string, decision: 'Allow' | 'Deny' = 'Allow') {
  const accountField = page.getByPlaceholder(/handle or DID/i)
  const decide = page.getByRole('button', { name: decision, exact: true })
  await expect(accountField.or(decide)).toBeVisible()
  if (await accountField.isVisible()) {
    await accountField.fill(account)
    await page.getByRole('button', { name: 'Continue' }).click()
  }
  await decide.click()
}

/** Signs in through the app's own screens. */
export async function signInThroughUi(page: Page, handle: string, account = handle) {
  await signInControl(page).click()
  await signInForm(page, handle)
  await consent(page, account)
}

/** Fills in and submits the app's sign-in page. */
export async function signInForm(page: Page, handle: string) {
  const main = page.getByRole('main')
  await main.getByLabel(/handle/i).fill(handle)
  await main.getByRole('button', { name: /^sign in$/i }).click()
}

/** The sign-in page's "Create account" control, a link or a button. */
export const createAccountControl = (page: Page): Locator =>
  page
    .getByRole('main')
    .getByRole('link', { name: /create account/i })
    .or(page.getByRole('main').getByRole('button', { name: /create account/i }))
