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
