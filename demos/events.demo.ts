import { devices, expect, type Page, test } from '@playwright/test'
import { consent, createAttendee, signInAs, signInControl } from '../e2e/support/auth.ts'

// Events, plans inside events, and redux cards, on two phones against the
// run's vivarium. Ana signs in through atproto; Bram is already signed in
// on his phone. Going to an event opens it; a plan made inside it shows up
// in its feed, live, and RSVPs from the card. Card modules run the same
// wasm on the phone and on the server.
//
// Run: DEMO_IDLE_TIMEOUT=1h node scripts/with-vivarium.ts npx playwright test -c demos/playwright.config.ts events

const pause = (page: Page, ms = 1200) => page.waitForTimeout(ms)
const card = (page: Page, title: string) =>
  page.locator('.live-card').filter({ has: page.getByRole('heading', { name: title }) })

test('events demo', async ({ page: ana, browser, baseURL }, testInfo) => {
  test.setTimeout(300_000)
  const anaAccount = await createAttendee('ana', { displayName: 'Ana de Vries' })
  const bramAccount = await createAttendee('bram', { displayName: 'Bram Mulder' })

  // Bram, on his own phone: signed in, going to AtmosphereConf, in the Hallway.
  const bramContext = await browser.newContext({
    ...devices['Pixel 7'],
    baseURL,
    serviceWorkers: 'block',
    recordVideo: { dir: testInfo.outputPath('bram'), size: { width: 412, height: 915 } },
  })
  const bram = await bramContext.newPage()
  await signInAs(bram, baseURL as string, bramAccount.handle)
  await bram.goto('/')
  await bram
    .getByRole('link', { name: /AtmosphereConf 2027/ })
    .first()
    .click()
  await bram.getByRole('button', { name: 'I’m going' }).first().click()
  await expect(bram.getByText('Live', { exact: true })).toBeVisible()

  // Ana signs in with her atmosphere handle and sees what's on.
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
  await expect(ana.getByRole('heading', { name: 'Events', exact: true })).toBeVisible()
  await pause(ana, 2000)

  // AtmosphereConf is locked until she says she's going.
  await ana
    .getByRole('link', { name: /AtmosphereConf 2027/ })
    .first()
    .click()
  await expect(ana.getByText(/Say you’re going to see/)).toBeVisible()
  await pause(ana, 2000)
  await ana.getByRole('region', { name: 'Locked' }).getByRole('button', { name: 'I’m going' }).click()
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

  // The Hallway: the quiz checks answers on her phone, and the server re-checks them.
  await ana.getByRole('link', { name: 'Hallway' }).click()
  const quiz = card(ana, 'Pub quiz: the atmosphere edition')
  await quiz.scrollIntoViewIfNeeded()
  await pause(ana)
  const answers = ['Authenticated Transfer', 'A decentralized identifier', 'In your PDS repo', 'Zaventem', 'Mustard']
  for (const answer of answers) {
    await quiz.getByRole('button', { name: answer }).click()
    await pause(ana, 800)
    await quiz.getByRole('button', { name: /next question|see my score/i }).click()
    await pause(ana, 300)
  }
  await expect(quiz.getByText('4 / 5')).toBeVisible()
  await quiz.getByRole('button', { name: 'Post my score' }).click()
  await expect(quiz.getByText(/Verified by the server: 4 of 5/)).toBeVisible()
  await ana.screenshot({ path: testInfo.outputPath('1-quiz.png') })
  await pause(ana, 1500)

  // Dice roll on the phone; posting one lets the server decide who's lowest.
  const dice = card(ana, 'Who buys the next round?')
  await dice.scrollIntoViewIfNeeded()
  await dice.getByRole('button', { name: 'Roll the dice' }).click()
  await pause(ana, 700)
  await dice.getByRole('button', { name: 'Roll again' }).click()
  await pause(ana)
  await dice.getByRole('button', { name: 'Post my roll' }).click()
  await expect(dice.getByText('Posted to the group.')).toBeVisible()
  await pause(ana, 1500)

  // Ana makes a plan inside the conference, announced in the Hallway.
  await ana.getByRole('link', { name: /^Plans/ }).click()
  await pause(ana)
  await ana.getByRole('region', { name: 'Plans' }).getByRole('button', { name: 'Make a plan' }).click()
  await ana.getByLabel('What').pressSequentially('Stroopwafel run', { delay: 30 })
  await ana.getByLabel('Where').pressSequentially('Albert Cuyp market', { delay: 20 })
  await ana.getByLabel('Details').pressSequentially('Warm ones, from the stall by the bike rack.', { delay: 10 })
  await ana.getByLabel('Announce it in').selectOption({ label: 'Hallway' })
  await ana.screenshot({ path: testInfo.outputPath('2-plan.png') })
  await ana.getByRole('button', { name: 'Make the plan' }).click()
  await expect(ana.getByRole('link', { name: /Stroopwafel run/ })).toBeVisible()
  await pause(ana, 1500)

  // It lands in Bram's Hallway; he says he's going from the card.
  await bram.getByRole('link', { name: 'Hallway' }).click()
  const plan = card(bram, 'Stroopwafel run')
  await expect(plan).toBeVisible()
  await plan.scrollIntoViewIfNeeded()
  await pause(bram)
  await plan.getByRole('button', { name: "I'm going" }).click()
  await expect(plan.getByText("You're going")).toBeVisible()
  await bram.screenshot({ path: testInfo.outputPath('3-bram-card.png') })

  // Ana's plan now has two people going.
  await expect(ana.getByRole('link', { name: /Stroopwafel run/ })).toContainText('2 going')
  await pause(ana, 1500)

  // Bram opens the plan: its own page and feed.
  await plan.getByRole('link', { name: 'Open plan →' }).click()
  await expect(bram).toHaveURL(/\/e\/[^/]+$/)
  await expect(bram.getByText(/A plan inside AtmosphereConf 2027/i)).toBeVisible()
  await expect(bram.getByRole('heading', { level: 1, name: 'Stroopwafel run' })).toBeVisible()
  await expect(bram.getByText('Live', { exact: true })).toBeVisible()
  await bram.screenshot({ path: testInfo.outputPath('4-plan-page.png') })
  await pause(bram, 1500)

  // What a card is made of: the actions that went up and came down.
  await ana.getByRole('link', { name: 'Hallway' }).click()
  const anaDice = card(ana, 'Who buys the next round?')
  await anaDice.scrollIntoViewIfNeeded()
  await anaDice.getByRole('button', { name: 'Source' }).click()
  await pause(ana)
  await ana.screenshot({ path: testInfo.outputPath('5-source.png'), fullPage: true })
  await pause(ana, 3000)

  await bramContext.close()
})
