import { type ChildProcess, spawn } from 'node:child_process'
import { mkdtempSync } from 'node:fs'
import { createServer } from 'node:net'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { createInterface } from 'node:readline'

export const SERVER_BIN = resolve(import.meta.dirname, '../../target/debug/conference-server')

export interface RunningServer {
  url: string
  /** The database the server was started on, so a test can restart on it. */
  databaseUrl: string
  /** The SQLite file behind `databaseUrl`, when it's a SQLite database. */
  databasePath?: string
  stop(): Promise<void>
}

/** A fresh SQLite database in its own temp directory. */
export function tempDatabase(): { databaseUrl: string; databasePath: string } {
  const databasePath = join(mkdtempSync(join(tmpdir(), 'eventside-db-')), 'eventside.db')
  return { databaseUrl: `sqlite://${databasePath}?mode=rwc`, databasePath }
}

/** A port nothing is listening on right now. */
export function freePort(): Promise<number> {
  return new Promise((resolvePort, reject) => {
    const server = createServer()
    server.once('error', reject)
    server.listen(0, '127.0.0.1', () => {
      const { port } = server.address() as { port: number }
      server.close(() => resolvePort(port))
    })
  })
}

/**
 * Spawns the backend binary, resolving once it announces its URL.
 *
 * Defaults, each overridable through env: a free port (`PORT=0`), the run's
 * vivarium as `ATPROTO_URL` (and so as the identity and sign-up services), a
 * fresh temp SQLite database, and `ALLOW_PRIVATE_NETWORK=true`, since vivarium
 * lives on localhost. To restart a server on the same database and client ID,
 * pass its `DATABASE_URL`, `PORT` and `PUBLIC_URL` again.
 */
export async function spawnServer(env: Record<string, string> = {}): Promise<RunningServer> {
  const atprotoUrl = env.ATPROTO_URL ?? process.env.VIVARIUM_URL
  if (!atprotoUrl) {
    throw new Error('spawnServer needs ATPROTO_URL or a run vivarium (VIVARIUM_URL); run through with-vivarium')
  }
  const db = env.DATABASE_URL
    ? { databaseUrl: env.DATABASE_URL, databasePath: sqlitePath(env.DATABASE_URL) }
    : tempDatabase()
  const child = spawn(SERVER_BIN, [], {
    env: {
      ...process.env,
      PORT: '0',
      SIGNUP_PDS_URL: atprotoUrl,
      ALLOW_PRIVATE_NETWORK: 'true',
      ...env,
      ATPROTO_URL: atprotoUrl,
      DATABASE_URL: db.databaseUrl,
    },
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

  return { url, ...db, stop: () => stop(child) }
}

function sqlitePath(databaseUrl: string): string | undefined {
  const match = /^sqlite:\/\/([^?]+)/.exec(databaseUrl)
  return match?.[1]
}

function stop(child: ChildProcess): Promise<void> {
  if (child.exitCode !== null || child.signalCode !== null) return Promise.resolve()
  return new Promise((done) => {
    child.once('exit', () => done())
    child.kill()
  })
}
