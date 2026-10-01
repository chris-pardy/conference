import { expect, test } from '@playwright/test'

// Review round 14: the default tokens load with the (lazy) blocks CSS, after
// a host's stylesheet, so they sit in a cascade layer to let the host win.

test('TC-33: a host theme loaded before the blocks still themes them', async ({ page }) => {
  await page.addInitScript(() => {
    document.addEventListener('DOMContentLoaded', () => {
      const style = document.createElement('style')
      style.id = 'org-theme'
      style.textContent = ':root { --g-color-primary: rgb(200, 30, 90); }'
      document.head.prepend(style)
    })
  })
  await page.goto('/dev/blocks')
  const primary = page.locator('[data-block="button"][data-variant="primary"]').first().getByRole('button')
  await primary.scrollIntoViewIfNeeded()
  await expect(primary).toHaveCSS('background-color', 'rgb(200, 30, 90)')
})
