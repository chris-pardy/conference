import { spawn } from 'node:child_process'
import { createInterface } from 'node:readline'
import { vivariumUrl } from '@vivarium-dev/client/vitest'
import { expect, inject, test } from 'vitest'
import { spawnServer } from '../support/server.ts'

const SERVER_BIN = 'target/debug/conference-server'

test('TC-2: the server reports that it can reach atproto', async () => {
  const serverUrl = inject('serverUrl')
  expect(serverUrl, 'the test run should provide a running server').toBeTypeOf('string')

  const res = await fetch(`${serverUrl}/health`)
  expect(res.status).toBe(200)
  expect(await res.json()).toEqual({ status: 'up', atproto: 'reachable' })
})

test('TC-3: the server stays up when atproto is unreachable', async () => {
  // Nothing listens on the discard port.
  const server = await spawnServer({ ATPROTO_URL: 'http://127.0.0.1:9' })
  try {
    const started = Date.now()
    const res = await fetch(`${server.url}/health`, { signal: AbortSignal.timeout(10_000) })
    expect(res.status).toBe(200)
    expect(await res.json()).toEqual({ status: 'up', atproto: 'unreachable' })
    expect(Date.now() - started).toBeLessThan(10_000)
  } finally {
    await server.stop()
  }
})

test('TC-4: the server picks a free port and announces it', async () => {
  const child = spawn(SERVER_BIN, [], {
    env: { ...process.env, PORT: '0', ATPROTO_URL: vivariumUrl() },
    stdio: ['ignore', 'pipe', 'pipe'],
  })
  try {
    const announced = await new Promise<string | undefined>((resolve) => {
      const timer = setTimeout(() => resolve(undefined), 10_000)
      createInterface({ input: child.stdout }).on('line', (line) => {
        const match = /listening on (http:\/\/\S+)/.exec(line)
        if (match) {
          clearTimeout(timer)
          resolve(match[1])
        }
      })
      child.on('exit', () => {
        clearTimeout(timer)
        resolve(undefined)
      })
    })
    expect(announced, 'the server should print "listening on <url>"').toBeTypeOf('string')
    expect(announced).toMatch(/^http:\/\/127\.0\.0\.1:\d+$/)
    expect(announced).not.toMatch(/:0$/)

    const res = await fetch(`${announced}/health`)
    expect(res.status).toBe(200)
    expect((await res.json()).status).toBe('up')
  } finally {
    child.kill()
  }
})
