import { expect, test } from '@playwright/test'

test('TC-7: the app loads on a phone and shows the whole stack is connected', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByText(/backend.*up/i)).toBeVisible()
  await expect(page.getByText(/atproto.*reachable/i)).toBeVisible()
})
