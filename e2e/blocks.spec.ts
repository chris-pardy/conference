import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { expect, type Locator, type Page, test } from '@playwright/test'
import jsQR from 'jsqr'
import { PNG } from 'pngjs'

// The block gallery, on a phone. The gallery's cards are the reference for
// authors; these tests rely on it showing, among its examples:
// - a stack of at least three blocks, and columns of stats
// - a QR code for the venue check-in URL below
// - primary and danger buttons, and info, warning and success badges
// - a button that opens a sheet, and loading, unavailable and empty states

const ROOT = resolve(import.meta.dirname, '..')
const GALLERY = '/dev/blocks'
const CHECK_IN_URL = 'https://atmosphereconf.org/2027/check-in'

async function openGallery(page: Page) {
  await page.goto(GALLERY)
  await expect(page.getByRole('heading', { level: 1, name: /blocks/i })).toBeVisible()
  await expect(page.locator('[data-block]').first()).toBeVisible()
}

/** The vocabulary: every block type in the card lexicon's union, minus the two block-sandbox renders. */
function vocabulary(): string[] {
  const lexicon = JSON.parse(readFileSync(resolve(ROOT, 'lexicons/app/eventside/block/card.json'), 'utf8'))
  const refs: string[] = lexicon.defs.main.record.properties.blocks.items.refs
  return refs.map((ref) => ref.split('#')[1]).filter((type) => type !== 'custom' && type !== 'canvas')
}

/** The theme's semantic tokens, from the token contract. */
function themeTokens() {
  const file = resolve(ROOT, 'web/src/theme/tokens.css')
  expect(existsSync(file), 'the theme token contract should be in web/src/theme/tokens.css').toBe(true)
  const css = readFileSync(file, 'utf8')
  const unique = (re: RegExp) => [...new Set(css.match(re) ?? [])].sort()
  return { colors: unique(/--g-color-[a-z0-9-]+/g), fonts: unique(/--g-font-[a-z0-9-]+/g) }
}

/** A test theme giving every token a distinctive value. */
function testTheme(tokens: { colors: string[]; fonts: string[] }) {
  const colors = Object.fromEntries(
    tokens.colors.map((name, i) => [name, `rgb(${11 + i * 9}, ${201 - i * 7}, ${97 + i * 5})`]),
  )
  const fonts = Object.fromEntries(tokens.fonts.map((name, i) => [name, `"Eventside Test Font ${i}"`]))
  const css = `:root:root { ${Object.entries({ ...colors, ...fonts })
    .map(([name, value]) => `${name}: ${value} !important;`)
    .join(' ')} }`
  return { colors, fonts, css }
}

/** Every color and font used inside blocks, with where it was used. */
function blockStyles(page: Page) {
  return page.evaluate(() => {
    const used: { where: string; property: string; value: string }[] = []
    const elements = document.querySelectorAll('[data-block], [data-block] *')
    for (const el of elements) {
      if (el.closest('svg, canvas, img') || el instanceof HTMLImageElement) continue
      const style = getComputedStyle(el)
      if (style.display === 'none' || style.visibility === 'hidden') continue
      const where = `${el.closest('[data-block]')?.getAttribute('data-block')} ${el.tagName.toLowerCase()}`
      const hasText = [...el.childNodes].some((n) => n.nodeType === Node.TEXT_NODE && n.textContent?.trim())
      if (hasText) {
        used.push({ where, property: 'color', value: style.color })
        used.push({ where, property: 'font-family', value: style.fontFamily })
      }
      used.push({ where, property: 'background-color', value: style.backgroundColor })
      for (const side of ['Top', 'Right', 'Bottom', 'Left'] as const) {
        if (Number.parseFloat(style[`border${side}Width`]) > 0 && style[`border${side}Style`] !== 'none') {
          used.push({ where, property: `border-${side.toLowerCase()}-color`, value: style[`border${side}Color`] })
        }
      }
    }
    return used
  })
}

/** "rgb(1, 2, 3)" or "rgba(1, 2, 3, 0.5)" as its rgb part and alpha. */
function parseColor(value: string) {
  const m = /rgba?\((\d+),\s*(\d+),\s*(\d+)(?:,\s*([\d.]+))?\)/.exec(value)
  if (!m) return null
  return { rgb: `rgb(${m[1]}, ${m[2]}, ${m[3]})`, alpha: m[4] === undefined ? 1 : Number(m[4]) }
}

/** The block roots that are direct block children of `parent`. */
function childBlocks(parent: Locator) {
  return parent.evaluate((root) =>
    [...root.querySelectorAll('[data-block]')]
      .filter((el) => el.parentElement?.closest('[data-block]') === root)
      .map((el) => {
        const r = el.getBoundingClientRect()
        return { top: r.top, left: r.left, bottom: r.bottom, right: r.right }
      }),
  )
}

/** The block itself when it is a button, or the button inside it. */
function buttonOf(blockRoot: Locator) {
  return blockRoot.locator('xpath=self::button | .//button').first()
}

test('TC-6: layout blocks arrange their children on a phone, with no horizontal scrolling', async ({ page }) => {
  await openGallery(page)

  const stack = page.locator('[data-block="stack"]').first()
  await stack.scrollIntoViewIfNeeded()
  const stacked = await childBlocks(stack)
  expect(stacked.length, 'the gallery stack should hold at least three blocks').toBeGreaterThanOrEqual(3)
  for (let i = 1; i < stacked.length; i++) {
    expect(stacked[i].top, 'stacked blocks sit one below the other').toBeGreaterThanOrEqual(stacked[i - 1].bottom - 1)
  }

  const columns = page
    .locator('[data-block="columns"]')
    .filter({ has: page.locator('[data-block="stat"]') })
    .first()
  await columns.scrollIntoViewIfNeeded()
  const side = await childBlocks(columns)
  expect(side.length, 'the gallery columns should hold at least two stats').toBeGreaterThanOrEqual(2)
  for (let i = 1; i < side.length; i++) {
    expect(side[i].left, 'columns sit side by side').toBeGreaterThanOrEqual(side[i - 1].right - 1)
    expect(Math.abs(side[i].top - side[0].top), 'columns share a top edge').toBeLessThan(2)
  }

  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth)
  expect(overflow, 'the page should not scroll sideways').toBeLessThanOrEqual(0)
})

test('TC-30: a QR code encodes its value', async ({ page }) => {
  await openGallery(page)

  const codes = page.locator('[data-block="qr"]')
  expect(await codes.count()).toBeGreaterThan(0)
  const decoded: string[] = []
  for (const code of await codes.all()) {
    await code.scrollIntoViewIfNeeded()
    const shot = PNG.sync.read(await code.screenshot())
    // Put the code on a wide white margin, so decoding doesn't depend on the block's own padding.
    const margin = 48
    const padded = new PNG({ width: shot.width + margin * 2, height: shot.height + margin * 2 })
    padded.data.fill(255)
    PNG.bitblt(shot, padded, 0, 0, shot.width, shot.height, margin, margin)
    const result = jsQR(new Uint8ClampedArray(padded.data), padded.width, padded.height)
    if (result) decoded.push(result.data)
  }
  expect(decoded).toContain(CHECK_IN_URL)
})

test('TC-33: blocks use only the theme tokens', async ({ page }) => {
  const tokens = themeTokens()
  for (const name of ['primary', 'danger', 'info', 'warning', 'success']) {
    expect(tokens.colors, 'the token contract names each meaning').toContain(`--g-color-${name}`)
  }
  expect(tokens.fonts.length).toBeGreaterThan(0)

  await openGallery(page)
  const before = await blockStyles(page)

  const theme = testTheme(tokens)
  await page.addStyleTag({ content: theme.css })
  const after = await blockStyles(page)

  const allowedColors = new Set(Object.values(theme.colors))
  const allowedFonts = Object.values(theme.fonts)
  const offTheme = after.filter(({ property, value }) => {
    if (property === 'font-family') return !allowedFonts.some((font) => value.startsWith(font))
    const color = parseColor(value)
    if (!color) return true
    if (color.alpha === 0) return false
    return !allowedColors.has(color.rgb)
  })
  expect(offTheme, 'every color and font inside a block should come from a theme token').toEqual([])

  // And the test theme really changed what the default theme showed.
  const changed = after.filter((use, i) => before[i] && before[i].value !== use.value)
  expect(changed.length).toBeGreaterThan(0)
})

test('TC-34: variants map to their meaning', async ({ page }) => {
  await openGallery(page)
  const theme = testTheme(themeTokens())
  await page.addStyleTag({ content: theme.css })

  /** The colors an element and its descendants use, by rgb. */
  const colorsOf = (el: Locator) =>
    el.evaluate((root) =>
      [root, ...root.querySelectorAll('*')].flatMap((node) => {
        const s = getComputedStyle(node)
        return [s.backgroundColor, s.color, s.borderTopColor]
      }),
    )
  const expectMeaning = async (el: Locator, token: string) => {
    await el.scrollIntoViewIfNeeded()
    const used = (await colorsOf(el)).map((v) => parseColor(v)?.rgb)
    expect(used, `${token} should color this block`).toContain(theme.colors[token])
  }

  await expectMeaning(
    buttonOf(page.locator('[data-block="button"][data-variant="primary"]').first()),
    '--g-color-primary',
  )
  await expectMeaning(
    buttonOf(page.locator('[data-block="button"][data-variant="danger"]').first()),
    '--g-color-danger',
  )
  for (const tone of ['info', 'warning', 'success']) {
    await expectMeaning(page.locator(`[data-block="badge"][data-tone="${tone}"]`).first(), `--g-color-${tone}`)
  }
})

test('TC-35: the gallery shows every block type and every surface', async ({ page }) => {
  await openGallery(page)

  const missing: string[] = []
  for (const type of vocabulary()) {
    if ((await page.locator(`[data-block="${type}"]`).count()) === 0) missing.push(type)
  }
  expect(missing, 'block types with no example in the gallery').toEqual([])

  for (const surface of ['feed', 'compact', 'ephemeral']) {
    await expect(page.locator(`[data-surface="${surface}"]`).first()).toBeAttached()
  }
  for (const state of ['loading', 'unavailable', 'empty']) {
    await expect(page.locator(`[data-block][data-state="${state}"]`).first()).toBeAttached()
  }

  const opener = page.locator('button[aria-haspopup="dialog"]').first()
  await opener.scrollIntoViewIfNeeded()
  await opener.click()
  const sheet = page.getByRole('dialog')
  await expect(sheet).toBeVisible()
  await expect(page.locator('[data-surface="sheet"]')).toBeVisible()
  await expect(sheet.locator('[data-block]').first()).toBeVisible()

  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth)
  expect(overflow).toBeLessThanOrEqual(0)
})

test('TC-36: the gallery shows the intents cards send', async ({ page }) => {
  await openGallery(page)
  const log = page.getByRole('log', { name: /intents/i })
  await expect(log).toBeAttached()

  // Tap a plain button on a feed card.
  const button = page
    .locator(
      '[data-surface="feed"] [data-block="button"][data-block-id]:not([aria-haspopup]):not(:has([aria-haspopup]))',
    )
    .first()
  const buttonCard = await button.evaluate((el) => el.closest('[data-card]')?.getAttribute('data-card'))
  const buttonId = await button.getAttribute('data-block-id')
  expect(buttonCard).toBeTruthy()
  await button.scrollIntoViewIfNeeded()
  await buttonOf(button).click()

  const first = log.getByRole('listitem').last()
  await expect(first).toContainText(buttonCard as string)
  await expect(first).toContainText(buttonId as string)
  for (const field of [/card/i, /block/i, /action/i, /value/i]) await expect(first).toContainText(field)

  // Fill in and submit a text input on a feed card.
  const form = page
    .locator('[data-card]:is([data-surface="feed"], [data-surface="feed"] *)')
    .filter({ has: page.locator('[data-block="textInput"]') })
    .filter({ has: page.locator('[data-block="submit"]') })
    .first()
  await form.scrollIntoViewIfNeeded()
  await form.getByRole('textbox').first().fill('Waar is de garderobe?')
  const submit = form.locator('[data-block="submit"]').first()
  const submitId = await submit.getAttribute('data-block-id')
  await buttonOf(submit).click()

  await expect(log.getByRole('listitem')).toHaveCount(2)
  const second = log.getByRole('listitem').last()
  await expect(second).toContainText(submitId as string)
  await expect(second).toContainText('Waar is de garderobe?')
  await expect(second).toContainText((await form.getAttribute('data-card')) as string)
})

test('TC-37: the home page still shows backend health, and the gallery has its own address', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByText(/backend.*up/i)).toBeVisible()
  await expect(page.locator('[data-block]')).toHaveCount(0)

  await openGallery(page)
  expect(new URL(page.url()).pathname).toBe(GALLERY)

  await page.goto('/')
  await expect(page.getByText(/backend.*up/i)).toBeVisible()
})
