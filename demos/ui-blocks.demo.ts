import { expect, type Locator, type Page, test } from '@playwright/test'

// ui-blocks' visible surface, through the block gallery on a phone: every
// block and surface rendered from sample cards (TC-35), interactions handing
// intents to the host (TC-19..24, TC-36), sheets (TC-32), conference-aware
// blocks (TC-26..30), placeholders (TC-16, TC-17) and the compact and
// ephemeral surfaces (TC-31).

const pause = (page: Page, ms = 1200) => page.waitForTimeout(ms)

async function show(page: Page, target: Locator, ms = 1500) {
  await target.scrollIntoViewIfNeeded()
  await target.evaluate((el) => el.scrollIntoView({ behavior: 'smooth', block: 'center' }))
  await pause(page, ms)
}

test('ui-blocks demo', async ({ page }) => {
  test.setTimeout(120_000)
  await page.goto('/dev/blocks')
  await expect(page.getByRole('heading', { level: 1, name: /blocks/i })).toBeVisible()
  await pause(page, 2000)

  // Header, image, rich text (mention, link, bold), divider, context.
  await show(page, page.getByRole('heading', { name: 'Welkom in Amsterdam' }), 2500)

  // Layout: a stack and columns of stats; data display in every tone.
  await show(page, page.getByRole('heading', { name: 'Dag 2 in cijfers' }), 2000)
  await show(page, page.getByRole('heading', { name: 'Snack poll results' }), 2500)

  // A text input and a submit: validation, then the intent.
  await show(page, page.getByRole('heading', { name: 'Ask the organizers' }))
  await page.getByRole('button', { name: 'Ask' }).click()
  await pause(page, 1500)
  await page.getByRole('textbox', { name: 'Your question' }).pressSequentially('Waar is de garderobe?', { delay: 60 })
  await page.getByRole('button', { name: 'Ask' }).click()
  await pause(page, 2000)

  // Buttons, a button group, a person and a countdown.
  await show(page, page.getByRole('heading', { name: 'Borrel op vrijdag' }), 2000)
  await page.getByRole('button', { name: 'Ik kom!' }).click()
  await pause(page)
  await page.getByRole('button', { name: 'Bitterballen' }).click()
  await pause(page, 1800)

  // Single and multiple select, with a limit.
  await show(page, page.getByRole('heading', { name: 'Plan your afternoon' }))
  await page.getByRole('radio', { name: 'Apps' }).click()
  await pause(page, 600)
  await page.getByRole('checkbox', { name: 'Lexicons in Practice' }).click()
  await pause(page, 600)
  await page.getByRole('checkbox', { name: 'Running a PDS' }).click()
  await pause(page, 1200)
  await show(page, page.getByRole('button', { name: 'Save my plan' }), 800)
  await page.getByRole('button', { name: 'Save my plan' }).click()
  await pause(page, 1800)

  // Lists over a collection (and empty), then loading and unavailable.
  await show(page, page.getByRole('heading', { name: 'Vragen voor de keynote' }), 2500)
  await show(page, page.getByRole('heading', { name: 'Wachtrij bij de garderobe' }), 2500)

  // Conference-aware: copyable, QR, session, room, and a sheet.
  await show(page, page.getByRole('heading', { name: 'Wifi & check-in' }), 2000)
  await show(page, page.locator('[data-block="qr"]'), 2000)
  const opener = page.getByRole('button', { name: 'Wifi details' })
  await show(page, opener, 800)
  await opener.click()
  await pause(page, 1500)
  const sheet = page.getByRole('dialog')
  await sheet
    .getByRole('textbox', { name: 'Report a problem' })
    .pressSequentially('Geen signaal in Zaal B', { delay: 50 })
  await sheet.getByRole('button', { name: 'Send' }).click()
  await pause(page, 1500)
  await sheet.getByRole('button', { name: 'Close' }).click()
  await pause(page, 1200)

  // Compact and ephemeral surfaces.
  await show(page, page.getByRole('heading', { name: 'Keukenhof trip' }), 2000)
  await show(page, page.getByRole('heading', { name: 'Je vraag is ontvangen' }), 1500)
  await page.getByRole('button', { name: 'Dismiss' }).click()
  await pause(page, 1500)

  // Every intent the cards handed their host.
  await show(page, page.getByRole('heading', { name: 'Action intents' }), 3000)
  await expect(page.getByRole('log', { name: /intents/i }).getByRole('listitem')).toHaveCount(5)
})
