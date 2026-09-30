import { expect, test } from '@playwright/test'

// ci-pipeline's visible surface: the placeholder page proving the stack is
// wired end to end (TC-6, TC-7) on a phone, and the PWA install (TC-8).
test('ci-pipeline demo', async ({ page }) => {
  // Slow the health answer a little so the loading state is visible.
  await page.route('**/health', async (route) => {
    await new Promise((resolve) => setTimeout(resolve, 1200))
    await route.continue()
  })
  await page.goto('/')
  await expect(page.getByText(/checking/i)).toBeVisible()
  await expect(page.getByText(/backend.*up/i)).toBeVisible()
  await expect(page.getByText('atproto: reachable')).toBeVisible()
  await page.waitForTimeout(2000)

  const manifest = await page.evaluate(async () => {
    const link = document.querySelector('link[rel="manifest"]') as HTMLLinkElement
    return (await fetch(link.href)).json()
  })
  expect(manifest.display).toBe('standalone')
  await page.waitForTimeout(1000)
})
