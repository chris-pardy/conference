import { type ChildProcess, spawn } from 'node:child_process'
import { resolve } from 'node:path'
import { createInterface } from 'node:readline'

export const SERVER_BIN = resolve(import.meta.dirname, '../../target/debug/conference-server')

export interface RunningServer {
  url: string
  stop(): Promise<void>
}

/**
 * Spawns the backend binary on a free port, resolving once it announces its
 * URL. `ATPROTO_URL` defaults to the run's vivarium; pass env to override.
 */
export async function spawnServer(env: Record<string, string> = {}): Promise<RunningServer> {
  const atprotoUrl = env.ATPROTO_URL ?? process.env.VIVARIUM_URL
  if (!atprotoUrl) {
    throw new Error('spawnServer needs ATPROTO_URL or a run vivarium (VIVARIUM_URL); run through with-vivarium')
  }
  const child = spawn(SERVER_BIN, [], {
    env: { ...process.env, PORT: '0', ...env, ATPROTO_URL: atprotoUrl },
    stdio: ['ignore', 'pipe', 'pipe'],
  })
  let stderr = ''
  child.stderr?.on('data', (d) => {
    stderr += d
  })

  const url = await new Promise<string>((resolveUrl, reject) => {
    const timer = setTimeout(() => fail('did not announce its URL within 10s'), 10_000)
    const fail = (why: string) => {
      clearTimeout(timer)
      child.kill()
      reject(new Error(`conference-server ${why}\n${stderr}`))
    }
    createInterface({ input: child.stdout as NodeJS.ReadableStream }).on('line', (line) => {
      const match = /listening on (http:\/\/\S+)/.exec(line)
      if (match) {
        clearTimeout(timer)
        resolveUrl(match[1])
      }
    })
    child.on('error', (err) => fail(`failed to start: ${err.message}`))
    child.on('exit', (code) => fail(`exited early with code ${code}`))
  })

  return { url, stop: () => stop(child) }
}

function stop(child: ChildProcess): Promise<void> {
  if (child.exitCode !== null || child.signalCode !== null) return Promise.resolve()
  return new Promise((done) => {
    child.once('exit', () => done())
    child.kill()
  })
}
