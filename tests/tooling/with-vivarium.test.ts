import { VivariumClient, startVivarium } from '@vivarium-dev/client'
import { expect, test } from 'vitest'
import { run } from './run.ts'

const WRAPPER = 'scripts/with-vivarium.ts'

/** The environment without any box a surrounding run may have provided. */
function envWithoutBox(): NodeJS.ProcessEnv {
  const env = { ...process.env }
  delete env.VIVARIUM_URL
  return env
}

// Prints the box URL the wrapped command sees, and whether it answers.
const REPORT = `node -e "const u=process.env.VIVARIUM_URL;fetch(u+'/xrpc/localhost.vivarium.getInfo').then(r=>console.log('BOX',u,r.ok),()=>console.log('BOX',u,false))"`

function boxesSeen(output: string): { url: string; up: boolean }[] {
  return [...output.matchAll(/^BOX (\S+) (true|false)$/gm)].map((m) => ({ url: m[1], up: m[2] === 'true' }))
}

test('TC-10: one box is shared across a test run', async () => {
  // Two separate processes inside one wrapped run, like vitest and Playwright.
  const result = await run('node', [WRAPPER, 'sh', '-c', `${REPORT} && ${REPORT}`], { env: envWithoutBox() })
  expect(result.code, result.output).toBe(0)

  const seen = boxesSeen(result.stdout)
  expect(seen).toHaveLength(2)
  expect(seen[0].url).toMatch(/^http:\/\/(localhost|127\.0\.0\.1):\d+$/)
  expect(seen[1].url).toBe(seen[0].url)
  expect(seen.every((b) => b.up)).toBe(true)

  // Stopped once the run ends.
  expect(await new VivariumClient(seen[0].url).isUp()).toBe(false)
})

test('TC-11: a run attaches to a box that is already running', async () => {
  const existing = await startVivarium({ attach: false, upstream: false })
  try {
    const result = await run('node', [WRAPPER, 'sh', '-c', REPORT], {
      env: { ...envWithoutBox(), VIVARIUM_URL: existing.url },
    })
    expect(result.code, result.output).toBe(0)
    expect(boxesSeen(result.stdout)).toEqual([{ url: existing.url, up: true }])

    // Left running afterwards.
    expect(await existing.isUp()).toBe(true)
  } finally {
    await existing.stop()
  }
})

test('TC-12: the test run reports the real result', async () => {
  const result = await run('node', [WRAPPER, 'sh', '-c', `${REPORT}; exit 3`], { env: envWithoutBox() })
  expect(result.code).toBe(3)

  const seen = boxesSeen(result.stdout)
  expect(seen, result.output).toHaveLength(1)
  expect(await new VivariumClient(seen[0].url).isUp()).toBe(false)
})
