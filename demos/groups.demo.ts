import { devices, expect, type Page, test } from '@playwright/test'
import { consent, createAttendee, signInAs, signInControl } from '../e2e/support/auth.ts'

// Groups, feeds and cards, on two phones against the run's vivarium. Ana
// signs in through atproto; Bram is already signed in on his phone. Each
// sees the other's actions arrive live. The Hallway cards carry wasm
// modules that answer on the device.
//
// Run: node scripts/with-vivarium.ts playwright test -c demos/playwright.config.ts groups

const pause = (page: Page, ms = 1200) => page.waitForTimeout(ms)
const card = (page: Page, title: string) =>
  page.locator('.live-card').filter({ has: page.getByRole('heading', { name: title }) })

test('groups demo', async ({ page: ana, browser, baseURL }, testInfo) => {
  test.setTimeout(240_000)
  const anaAccount = await createAttendee('ana', { displayName: 'Ana de Vries' })
  const bramAccount = await createAttendee('bram', { displayName: 'Bram Mulder' })

  // Bram, on his own phone, already signed in and on the main stage.
  const bramContext = await browser.newContext({
    ...devices['Pixel 7'],
    baseURL,
    serviceWorkers: 'block',
    recordVideo: { dir: testInfo.outputPath('bram'), size: { width: 412, height: 915 } },
  })
  const bram = await bramContext.newPage()
  await signInAs(bram, baseURL as string, bramAccount.handle)
  await bram.goto('/')
  await bram.getByRole('link', { name: /AtmosphereConf 2027/ }).click()
  await expect(bram.getByText('Live', { exact: true })).toBeVisible()

  // Ana signs in with her atmosphere handle.
  await ana.goto('/')
  await pause(ana, 1500)
  await signInControl(ana).click()
  await ana
    .getByRole('main')
    .getByLabel(/handle/i)
    .pressSequentially(anaAccount.handle, { delay: 30 })
  await ana
    .getByRole('main')
    .getByRole('button', { name: /^sign in$/i })
    .click()
  await consent(ana, anaAccount.handle)

  // Her groups: she's in AtmosphereConf already.
  await expect(ana.getByRole('heading', { name: 'Your groups' })).toBeVisible()
  await pause(ana, 1500)
  await ana.getByRole('link', { name: /AtmosphereConf 2027/ }).click()
  await expect(ana.getByText('Live', { exact: true })).toBeVisible()
  await pause(ana, 1500)

  // She votes in the keynote poll; Bram's bars move.
  const poll = card(ana, 'What should the keynote dig into?')
  await poll.scrollIntoViewIfNeeded()
  await pause(ana)
  await poll.getByRole('button', { name: 'Moderation at scale' }).click()
  await expect(poll.locator('[data-block="stat"]').getByText('6')).toBeVisible()
  const bramPoll = card(bram, 'What should the keynote dig into?')
  await bramPoll.scrollIntoViewIfNeeded()
  await expect(bramPoll.locator('[data-block="stat"]').getByText('6')).toBeVisible()
  await pause(ana, 1500)

  // Bram asks a question; it appears on Ana's phone, and she upvotes it.
  const bramQa = card(bram, 'Ask the keynote')
  await bramQa.scrollIntoViewIfNeeded()
  await bramQa.getByLabel(/your question/i).pressSequentially('Can a space span two PDS hosts?', { delay: 25 })
  await bramQa.getByRole('button', { name: 'Ask', exact: true }).click()
  const qa = card(ana, 'Ask the keynote')
  await qa.scrollIntoViewIfNeeded()
  const question = qa.locator('.g-section').filter({ hasText: 'Can a space span two PDS hosts?' })
  await expect(question).toBeVisible()
  await pause(ana)
  await question.getByRole('button', { name: 'Upvote' }).click()
  await expect(bramQa.locator('.g-section').filter({ hasText: 'two PDS hosts' }).getByText('1')).toBeVisible()
  await pause(ana, 1500)

  // The Hallway: cards with wasm modules, answering on the device.
  await ana.getByRole('link', { name: 'Hallway' }).click()
  const quiz = card(ana, 'Pub quiz: the atmosphere edition')
  await expect(quiz.getByText(/^wasm/)).toBeVisible()
  await pause(ana)
  const answers = ['Authenticated Transfer', 'A decentralized identifier', 'In your PDS repo', 'Zaventem', 'Mustard']
  for (const answer of answers) {
    await quiz.getByRole('button', { name: answer }).click()
    await pause(ana, 900)
    await quiz.getByRole('button', { name: /next question|see my score/i }).click()
    await pause(ana, 400)
  }
  await expect(quiz.getByText('4 / 5')).toBeVisible()
  await ana.screenshot({ path: testInfo.outputPath('1-quiz.png') })
  await pause(ana)
  await quiz.getByRole('button', { name: 'Post my score' }).click()
  await expect(quiz.getByText('Score posted')).toBeVisible()
  await pause(ana, 1500)

  const dice = card(ana, 'Who buys the next round?')
  await dice.scrollIntoViewIfNeeded()
  await dice.getByRole('button', { name: 'Roll the dice' }).click()
  for (let i = 0; i < 3; i++) {
    await pause(ana, 700)
    await dice.getByRole('button', { name: 'Roll again' }).click()
  }
  await pause(ana)
  await dice.getByRole('button', { name: 'Post my roll' }).click()
  await expect(dice.getByText('Posted to the group.')).toBeVisible()
  await dice.scrollIntoViewIfNeeded()
  await ana.screenshot({ path: testInfo.outputPath('2-dice.png') })
  await pause(ana, 1500)

  // Bram sees Ana's roll arrive in the Hallway.
  await bram.getByRole('link', { name: 'Hallway' }).click()
  const bramDice = card(bram, 'Who buys the next round?')
  await bramDice.scrollIntoViewIfNeeded()
  await expect(bramDice.getByText('Ana de Vries')).toBeVisible()
  await pause(bram, 1500)

  // Ana posts a new card from a template; it lands on Bram's phone, and his taps count.
  await ana.getByRole('button', { name: 'New card' }).click()
  await pause(ana)
  await ana.getByRole('radio', { name: 'Counter' }).click()
  await pause(ana, 1500)
  await ana.screenshot({ path: testInfo.outputPath('3-composer.png'), fullPage: true })
  await ana.getByRole('button', { name: 'Post to feed' }).click()
  const coffee = card(bram, 'Coffees drunk today')
  await expect(coffee).toBeVisible()
  await coffee.scrollIntoViewIfNeeded()
  for (let i = 0; i < 3; i++) {
    await coffee.getByRole('button', { name: '+1 coffee' }).click()
    await pause(bram, 500)
  }
  const anaCoffee = card(ana, 'Coffees drunk today')
  await anaCoffee.scrollIntoViewIfNeeded()
  await expect(anaCoffee.locator('[data-block="stat"]').first().getByText('3')).toBeVisible()
  await pause(ana, 1500)

  // What a card is made of: card kit UI and the server's data.
  await anaCoffee.getByRole('button', { name: 'Source' }).click()
  await ana.screenshot({ path: testInfo.outputPath('4-source.png') })
  await bram.screenshot({ path: testInfo.outputPath('5-bram.png') })
  await pause(ana, 3000)

  await bramContext.close()
})
