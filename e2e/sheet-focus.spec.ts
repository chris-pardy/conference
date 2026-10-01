import { expect, test } from '@playwright/test'

// Review round 9: a real browser won't focus an inert element, which jsdom
// doesn't enforce, so focus return after closing a sheet is checked here.

test('TC-32: closing a sheet gives focus back to the button that opened it', async ({ page }) => {
  await page.goto('/dev/blocks')
  const opener = page.getByRole('button', { name: 'Wifi details' })
  await opener.scrollIntoViewIfNeeded()

  for (const close of ['Escape', 'Close button'] as const) {
    await opener.click()
    await expect(page.getByRole('dialog')).toBeVisible()
    if (close === 'Escape') await page.keyboard.press('Escape')
    else await page.getByRole('dialog').getByRole('button', { name: 'Close' }).click()
    await expect(page.getByRole('dialog')).toHaveCount(0)
    await expect(opener, `focus after closing with ${close}`).toBeFocused()
  }
})

test('TC-6: an open sheet with a long token does not scroll sideways', async ({ page }) => {
  await page.goto('/dev/blocks')
  const opener = page.getByRole('button', { name: 'Wifi details' })
  await opener.scrollIntoViewIfNeeded()
  await opener.click()
  const sheet = page.getByRole('dialog')
  await expect(sheet).toContainText('troubleshooting-atmosphere-gast')
  const { scrollWidth, clientWidth } = await sheet.evaluate((el) => ({
    scrollWidth: el.scrollWidth,
    clientWidth: el.clientWidth,
  }))
  expect(scrollWidth).toBeLessThanOrEqual(clientWidth)
  const close = await sheet.getByRole('button', { name: 'Close' }).boundingBox()
  expect((close?.x ?? 0) + (close?.width ?? 0)).toBeLessThanOrEqual(page.viewportSize()?.width ?? 0)
})

test('TC-32: Escape closes a sheet after a click on its text', async ({ page }) => {
  await page.goto('/dev/blocks')
  const opener = page.getByRole('button', { name: 'Wifi details' })
  await opener.scrollIntoViewIfNeeded()
  await opener.click()
  const sheet = page.getByRole('dialog')
  await sheet.getByRole('heading').click()
  await page.keyboard.press('Escape')
  await expect(page.getByRole('dialog')).toHaveCount(0)
  await expect(opener).toBeFocused()
})
