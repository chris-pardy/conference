import { expect, test } from '@playwright/test'

test.use({ serviceWorkers: 'allow' })

test('TC-8: the page registers its service worker', async ({ page }) => {
  await page.goto('/')
  const scope = await page.evaluate(async () => {
    const ready = navigator.serviceWorker.ready.then((r) => r.scope)
    const timeout = new Promise<null>((resolve) => setTimeout(() => resolve(null), 10_000))
    return Promise.race([ready, timeout])
  })
  expect(scope, 'a service worker should be registered and active').not.toBeNull()
})
