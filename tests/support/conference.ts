// Conferences for tests, set up the way an operator would: the server binary's
// `admin` CLI against a running server, admins connected through the real
// OAuth flow, and attendees using the app's conference XRPC with a browser's
// cookies. Later features' tests seed conferences with these too.
//
// The CLI contract these helpers rely on is recorded in the feature file
// (features/conference-space.md, "Build notes").
import { spawn } from 'node:child_process'
import { mkdtempSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { createInterface } from 'node:readline'
import type { VivariumScope } from '@vivarium-dev/client'
import type { Account, Json, XrpcAnswer } from './atproto.ts'
import { type CookieJar, getSession, signIn } from './auth.ts'
import { type BrowsedTo, browse } from './browser.ts'
import { P256Key } from './crypto.ts'
import { freePort, SERVER_BIN, spawnServer, tempDatabase } from './server.ts'

/** A running server and the environment its CLI runs with. */
export interface Deployment {
  /** The server's URL, which is also its PUBLIC_URL. */
  url: string
  /** The environment the server runs with, for its `admin` CLI. */
  env: Record<string, string>
  /** The SQLite file the server keeps its state in. */
  databasePath?: string
  stop(): Promise<void>
}

function vivariumUrl(): string {
  const url = process.env.VIVARIUM_URL
  if (!url) throw new Error('conference tests need the run vivarium in VIVARIUM_URL; run through with-vivarium')
  return url
}

/** Starts a server of its own, on a fresh database, whose PUBLIC_URL is its own URL. */
export async function deploy(extra: Record<string, string> = {}): Promise<Deployment> {
  const port = String(await freePort())
  const env = {
    PORT: port,
    PUBLIC_URL: `http://127.0.0.1:${port}`,
    DATABASE_URL: tempDatabase().databaseUrl,
    ATPROTO_URL: vivariumUrl(),
    SIGNUP_PDS_URL: vivariumUrl(),
    ALLOW_PRIVATE_NETWORK: 'true',
    ...extra,
  }
  const server = await spawnServer(env)
  return { url: server.url, env, databasePath: server.databasePath, stop: () => server.stop() }
}

/** The Playwright run's server, as its CLI sees it (see playwright.config.ts). */
export function e2eDeployment(baseURL: string): Deployment {
  const databasePath = process.env.E2E_DATABASE_PATH
  if (!databasePath) throw new Error('e2e needs E2E_DATABASE_PATH: run through `pnpm test:e2e`')
  return {
    url: baseURL,
    databasePath,
    env: {
      PUBLIC_URL: baseURL,
      DATABASE_URL: `sqlite://${databasePath}?mode=rwc`,
      ATPROTO_URL: vivariumUrl(),
      SIGNUP_PDS_URL: vivariumUrl(),
      ALLOW_PRIVATE_NETWORK: 'true',
    },
    stop: async () => {},
  }
}

export interface CliResult {
  args: string[]
  code: number | null
  stdout: string
  stderr: string
}

/** The JSON a `--json` command printed: its last non-empty line of stdout. */
export function cliJson(result: CliResult): Json {
  const line = result.stdout.trim().split('\n').at(-1) ?? ''
  try {
    return JSON.parse(line)
  } catch {
    throw new Error(`admin ${result.args.join(' ')} printed no JSON:\n${result.stdout}\n${result.stderr}`)
  }
}

/** Runs `conference-server admin <args>` against a deployment. */
export function cli(dep: Deployment, args: string[], opts: { env?: Record<string, string> } = {}): Promise<CliResult> {
  return new Promise((resolve, reject) => {
    const child = spawn(SERVER_BIN, ['admin', ...args], {
      env: { ...process.env, ...dep.env, ...opts.env },
      stdio: ['ignore', 'pipe', 'pipe'],
    })
    let stdout = ''
    let stderr = ''
    child.stdout.on('data', (d) => {
      stdout += d
    })
    child.stderr.on('data', (d) => {
      stderr += d
    })
    const timer = setTimeout(() => {
      child.kill()
      reject(new Error(`admin ${args.join(' ')} didn't finish within 60s\n${stdout}\n${stderr}`))
    }, 60_000)
    child.on('error', (err) => {
      clearTimeout(timer)
      reject(err)
    })
    child.on('exit', (code) => {
      clearTimeout(timer)
      resolve({ args, code, stdout, stderr })
    })
  })
}

/** Runs a CLI command that must succeed, and returns what it printed. */
export async function cliOk(dep: Deployment, args: string[], opts: { env?: Record<string, string> } = {}) {
  const result = await cli(dep, args, opts)
  if (result.code !== 0) {
    throw new Error(
      `admin ${args.join(' ')} failed (exit ${result.code}): ${result.stderr.trim() || result.stdout.trim()}`,
    )
  }
  return result
}

/** Runs a `--json` CLI command that must succeed, and returns its JSON. */
export async function cliOkJson(dep: Deployment, args: string[], opts: { env?: Record<string, string> } = {}) {
  return cliJson(await cliOk(dep, [...args, '--json'], opts))
}

export interface Connected {
  /** What the browser was shown when sign-in finished. */
  page: BrowsedTo & { jar: CookieJar }
  cli: CliResult
}

/**
 * Connects a person as an admin: runs `admin connect <handle>`, opens the URL
 * it prints in a browser, completes their PDS's consent screens, and waits
 * for the CLI to finish.
 */
export async function connect(
  dep: Deployment,
  person: Pick<Account, 'handle'>,
  opts: { env?: Record<string, string>; jar?: CookieJar } = {},
): Promise<Connected> {
  const args = ['connect', person.handle]
  const child = spawn(SERVER_BIN, ['admin', ...args], {
    env: { ...process.env, ...dep.env, ...opts.env },
    stdio: ['ignore', 'pipe', 'pipe'],
  })
  let stdout = ''
  let stderr = ''
  child.stderr.on('data', (d) => {
    stderr += d
  })
  const exited = new Promise<number | null>((resolve) => child.on('exit', (code) => resolve(code)))
  const url = await new Promise<string>((resolve, reject) => {
    const timer = setTimeout(() => {
      child.kill()
      reject(new Error(`admin connect printed no URL to open within 30s\n${stdout}\n${stderr}`))
    }, 30_000)
    createInterface({ input: child.stdout }).on('line', (line) => {
      stdout += `${line}\n`
      const match = /https?:\/\/\S+/.exec(line)
      if (match) {
        clearTimeout(timer)
        resolve(match[0])
      }
    })
    void exited.then((code) => {
      clearTimeout(timer)
      reject(new Error(`admin connect exited (${code}) before printing a URL: ${stderr.trim() || stdout.trim()}`))
    })
  })
  const page = await browse(url, { account: person.handle, jar: opts.jar })
  const code = await Promise.race([
    exited,
    new Promise<'timeout'>((resolve) => setTimeout(() => resolve('timeout'), 30_000)),
  ])
  if (code === 'timeout') {
    child.kill()
    throw new Error(`admin connect didn't finish within 30s of signing in\n${stdout}\n${stderr}`)
  }
  return { page, cli: { args, code, stdout, stderr } }
}

/** Connects a person as an admin, failing unless it worked. */
export async function connectOk(
  dep: Deployment,
  person: Pick<Account, 'handle'>,
  opts: { env?: Record<string, string> } = {},
) {
  const connected = await connect(dep, person, opts)
  if (connected.cli.code !== 0) {
    throw new Error(
      `admin connect ${person.handle} failed (exit ${connected.cli.code}): ${connected.cli.stderr.trim()}`,
    )
  }
  return connected
}

/** An organization: a space authority our server minted, with its super admin. */
export interface Org {
  did: string
  superAdmin: Account
  /** `at://{did}/space/app.eventside.admin/self`. */
  adminSpace: string
}

/**
 * Creates an organization with a super admin, who then connects (which writes
 * their first admin records). The operator's recovery key is generated unless
 * given.
 */
export async function createOrg(
  dep: Deployment,
  superAdmin: Account,
  opts: { recoveryKey?: string; name?: string } = {},
): Promise<Org> {
  const recoveryKey = opts.recoveryKey ?? P256Key.generate().didKey()
  const args = ['org', 'create', '--super-admin', superAdmin.handle, '--recovery-key', recoveryKey]
  if (opts.name) args.push('--name', opts.name)
  const { did } = await cliOkJson(dep, args)
  await connectOk(dep, superAdmin)
  return { did, superAdmin, adminSpace: `at://${did}/space/app.eventside.admin/self` }
}

/** Adds an admin to an organization, as the super admin, and connects them. */
export async function addAdmin(dep: Deployment, org: Org, person: Account, role: 'owner' | 'staff'): Promise<void> {
  await cliOk(dep, ['org', 'admin', 'add', person.handle, '--org', org.did, '--role', role])
  await connectOk(dep, person)
}

export type JoinMethod = 'code' | 'request' | 'list' | 'open'

export interface ConferenceSpec {
  name: string
  startsAt: string
  endsAt: string
  city: string
  description?: string
  inviteOnly?: boolean
  /** Theme tokens, by name without the `--g-` prefix: `{ 'color-primary': '#b0306a' }`. */
  theme?: Record<string, string>
  /** An existing event to adopt instead of publishing a new one. */
  event?: string
  /** The conference's super admin, if not the organization's. */
  superAdmin?: Account
  /** Join methods to turn on after creating it. */
  methods?: JoinMethod[]
}

export const ATMOSPHERECONF: ConferenceSpec = {
  name: 'AtmosphereConf',
  startsAt: '2027-04-29T09:00:00+02:00',
  endsAt: '2027-05-02T18:00:00+02:00',
  city: 'Amsterdam',
  description: 'The atproto community conference, at the Openbare Bibliotheek Amsterdam.',
}

export const WEDDING: ConferenceSpec = {
  name: 'Sanne & Joost’s wedding',
  startsAt: '2027-06-12T14:00:00+02:00',
  endsAt: '2027-06-13T12:00:00+02:00',
  city: 'Haarlem',
  description: 'Two days of celebrating at the Teylers.',
  inviteOnly: true,
  theme: { 'color-primary': '#b0306a' },
}

export interface Conference {
  org: Org
  /** `at://{authority}/space/app.eventside.conference/{skey}`: the access boundary. */
  space: string
  /** `at://{authority}/space/app.eventside.intake/{skey}`. */
  intake: string
  /** The public event's AT-URI, for public conferences. */
  event?: string
  /** The conference's super admin. */
  superAdmin: Account
}

/** Creates a conference for an organization, as its super admin, then turns on its join methods. */
export async function createConference(dep: Deployment, org: Org, spec: ConferenceSpec): Promise<Conference> {
  const args = ['conference', 'create', '--org', org.did]
  // An adopted event already says what the conference is called, and when and where.
  if (spec.event) args.push('--event', spec.event)
  else {
    args.push('--name', spec.name, '--starts', spec.startsAt, '--ends', spec.endsAt, '--city', spec.city)
    if (spec.description) args.push('--description', spec.description)
  }
  if (spec.inviteOnly) args.push('--invite-only')
  if (spec.theme) args.push('--theme', JSON.stringify(spec.theme))
  if (spec.superAdmin) args.push('--super-admin', spec.superAdmin.handle)
  const out = await cliOkJson(dep, args)
  const conference: Conference = {
    org,
    space: out.space,
    intake: out.intake,
    event: out.event ?? undefined,
    superAdmin: spec.superAdmin ?? org.superAdmin,
  }
  if (spec.methods) await setMethods(dep, conference, spec.methods)
  return conference
}

/** Chooses a conference's join methods (all others off). */
export async function setMethods(
  dep: Deployment,
  conference: Conference,
  methods: JoinMethod[],
  opts: { as?: Account } = {},
): Promise<CliResult> {
  return cliOk(dep, [
    'join',
    'set',
    '--conference',
    conference.space,
    '--methods',
    methods.join(','),
    ...asArgs(opts.as),
  ])
}

/** `--as <handle>`, or nothing for the super admin. */
export const asArgs = (as?: Pick<Account, 'handle'>): string[] => (as ? ['--as', as.handle] : [])

/** Issues a shared code with the given text. */
export async function sharedCode(
  dep: Deployment,
  conference: Conference,
  code: string,
  opts: { expires?: string; maxUses?: number } = {},
): Promise<string> {
  const args = ['codes', 'issue', '--conference', conference.space, '--shared', code]
  if (opts.expires) args.push('--expires', opts.expires)
  if (opts.maxUses !== undefined) args.push('--max-uses', String(opts.maxUses))
  const out = await cliOkJson(dep, args)
  return out.codes[0]
}

/** Issues one personal code. */
export async function personalCode(dep: Deployment, conference: Conference): Promise<string> {
  const out = await cliOkJson(dep, ['codes', 'issue', '--conference', conference.space, '--personal'])
  return out.codes[0]
}

/** Imports an attendee list, as a ticketing tool's CSV export with handle, email and role columns. */
export async function importList(
  dep: Deployment,
  conference: Conference,
  rows: { handle?: string; email?: string; role?: string }[],
): Promise<CliResult> {
  const csv = ['handle,email,role', ...rows.map((r) => [r.handle ?? '', r.email ?? '', r.role ?? ''].join(','))].join(
    '\n',
  )
  const file = join(mkdtempSync(join(tmpdir(), 'eventside-list-')), 'attendees.csv')
  writeFileSync(file, `${csv}\n`)
  return cliOk(dep, ['list', 'import', file, '--conference', conference.space])
}

/** Allows another app's client ID into a conference's space. */
export async function allowApp(dep: Deployment, conference: Conference, clientId: string): Promise<CliResult> {
  return cliOk(dep, ['apps', 'add', clientId, '--conference', conference.space])
}

/** Allows another app's client ID into an organization's admin space (and so its intake spaces). */
export async function allowAppForOrg(dep: Deployment, org: Org, clientId: string): Promise<CliResult> {
  return cliOk(dep, ['apps', 'add', clientId, '--org', org.did])
}

/** A conference action on a person, as an admin (`member add|remove|ban`, `requests approve|deny`). */
export function memberAction(
  dep: Deployment,
  conference: Conference,
  action: ['member', 'add' | 'remove' | 'ban'] | ['requests', 'approve' | 'deny'],
  person: Pick<Account, 'handle'>,
  opts: { as?: Pick<Account, 'handle'>; env?: Record<string, string> } = {},
): Promise<CliResult> {
  return cli(dep, [...action, person.handle, '--conference', conference.space, ...asArgs(opts.as)], { env: opts.env })
}

/** Sets (or, with `none`, removes) a person's role in a conference. */
export function setRole(
  dep: Deployment,
  conference: Conference,
  person: Pick<Account, 'handle'>,
  role: 'owner' | 'staff' | 'speaker' | 'none',
  opts: { as?: Pick<Account, 'handle'> } = {},
): Promise<CliResult> {
  return cli(dep, [
    'member',
    'role',
    person.handle,
    '--role',
    role,
    '--conference',
    conference.space,
    ...asArgs(opts.as),
  ])
}

/** A signed-in attendee, using the app's conference XRPC with a browser's cookies. */
export class Attendee {
  readonly dep: Deployment
  readonly person: Account
  readonly jar: CookieJar
  private csrf: string

  private constructor(dep: Deployment, person: Account, jar: CookieJar, csrf: string) {
    this.dep = dep
    this.person = person
    this.jar = jar
    this.csrf = csrf
  }

  /** Signs a person in to the app through the real flow. */
  static async signIn(dep: Deployment, person: Account): Promise<Attendee> {
    const flow = await signIn(dep.url, person.handle)
    const session = await getSession(dep.url, flow.jar)
    if (session.status !== 200) throw new Error(`${person.handle} couldn't sign in: ${JSON.stringify(session.body)}`)
    return new Attendee(dep, person, flow.jar, session.body.csrfToken)
  }

  get did(): string {
    return this.person.did
  }

  /** Reads the session's CSRF token again, after a sign-in replaced the session. */
  async refresh(): Promise<void> {
    const session = await getSession(this.dep.url, this.jar)
    if (session.status !== 200) throw new Error(`${this.person.handle} is signed out: ${JSON.stringify(session.body)}`)
    this.csrf = session.body.csrfToken
  }

  /**
   * The email step: opens the `verifyUrl` a join answered `emailNeeded` with,
   * and approves sharing the email at the person's PDS.
   */
  async verifyEmail(verifyUrl: string): Promise<BrowsedTo> {
    const page = await browse(new URL(verifyUrl, this.dep.url).toString(), {
      jar: this.jar,
      account: this.person.handle,
      stopAt: `${this.dep.url}/`,
    })
    // Back at the app (anything but its own OAuth routes) is where the step ends.
    const end = page.location && !new URL(page.location).pathname.startsWith('/oauth/') ? page : await finish(page)
    await this.refresh()
    return end
  }

  async call(nsid: string, opts: { params?: Record<string, string>; body?: unknown } = {}): Promise<XrpcAnswer> {
    const url = new URL(`/xrpc/${nsid}`, this.dep.url)
    for (const [key, value] of Object.entries(opts.params ?? {})) url.searchParams.set(key, value)
    const post = opts.body !== undefined
    const headers: Record<string, string> = post
      ? { 'content-type': 'application/json', 'x-csrf-token': this.csrf }
      : {}
    const cookie = this.jar.header(url.toString())
    if (cookie) headers.cookie = cookie
    const res = await fetch(url, {
      method: post ? 'POST' : 'GET',
      headers,
      body: post ? JSON.stringify(opts.body) : undefined,
      redirect: 'manual',
    })
    this.jar.store(url.toString(), res)
    return parse(res)
  }

  getConference(conference: string): Promise<XrpcAnswer> {
    return this.call('app.eventside.conference.getConference', { params: { conference } })
  }

  join(input: { conference?: string; code?: string; request?: boolean }): Promise<XrpcAnswer> {
    return this.call('app.eventside.conference.join', { body: input })
  }

  leave(conference: string): Promise<XrpcAnswer> {
    return this.call('app.eventside.conference.leave', { body: { conference } })
  }

  listMyConferences(): Promise<XrpcAnswer> {
    return this.call('app.eventside.conference.listMyConferences')
  }

  /** The conference's records in a collection, as the app lists them for members. */
  listRecords(conference: string, collection: string): Promise<XrpcAnswer> {
    return this.call('app.eventside.conference.listRecords', { params: { conference, collection } })
  }

  /** Whether the app says this person is a member of the conference. */
  async isMember(conference: string): Promise<boolean> {
    const answer = await this.getConference(conference)
    if (answer.status !== 200) {
      throw new Error(`getConference answered ${answer.status}: ${JSON.stringify(answer.body)}`)
    }
    return answer.body.viewer?.member === true
  }
}

/** Follows redirects through the app's own OAuth routes until they lead elsewhere in the app. */
async function finish(page: BrowsedTo & { jar: CookieJar }): Promise<BrowsedTo> {
  let current: BrowsedTo & { jar: CookieJar } = page
  for (let hop = 0; hop < 10 && current.location; hop++) {
    const next = new URL(current.location)
    if (!next.pathname.startsWith('/oauth/')) return current
    current = await browse(next.toString(), { jar: current.jar, stopAt: `${next.origin}/` })
  }
  return current
}

/** The conference XRPC's `getConference`, signed out. */
export async function publicConference(dep: Deployment, conference: string): Promise<XrpcAnswer> {
  const url = new URL('/xrpc/app.eventside.conference.getConference', dep.url)
  url.searchParams.set('conference', conference)
  return parse(await fetch(url))
}

async function parse(res: Response): Promise<XrpcAnswer> {
  const text = await res.text()
  let body: Json
  try {
    body = text ? JSON.parse(text) : {}
  } catch {
    body = { raw: text }
  }
  return { status: res.status, body }
}

/** Unique, readable codes per test. */
export const uniqueCode = (word: string) => `${word}-${Math.random().toString(36).slice(2, 8)}`

/** Makes a new account on the run's vivarium, named for a role in a test. */
export type NewAccount = (name: string) => Promise<Account>

/** New accounts in a vitest `viv` scope, cleaned up after the test. */
export const accountsIn =
  (viv: VivariumScope): NewAccount =>
  (name) =>
    viv.createAccount(viv.handle(name))

export interface Seeded {
  org: Org
  conference: Conference
  superAdmin: Account
  /** Every admin by the name they were made with, the super admin included. */
  admins: Record<string, Account>
}

/**
 * Seeds an organization and one conference: the super admin creates and
 * connects, owners and staff are added and connect, then the conference is
 * created with its join methods.
 */
export async function seedConference(
  dep: Deployment,
  newAccount: NewAccount,
  opts: {
    superAdmin?: string
    owners?: string[]
    staff?: string[]
    spec?: ConferenceSpec
    methods?: JoinMethod[]
  } = {},
): Promise<Seeded> {
  const superAdminName = opts.superAdmin ?? 'olga'
  const superAdmin = await newAccount(superAdminName)
  const org = await createOrg(dep, superAdmin)
  const admins: Record<string, Account> = { [superAdminName]: superAdmin }
  for (const [names, role] of [
    [opts.owners ?? [], 'owner'],
    [opts.staff ?? [], 'staff'],
  ] as const) {
    for (const name of names) {
      admins[name] = await newAccount(name)
      await addAdmin(dep, org, admins[name], role)
    }
  }
  const spec = opts.spec ?? ATMOSPHERECONF
  const conference = await createConference(dep, org, { ...spec, methods: opts.methods ?? spec.methods })
  return { org, conference, superAdmin, admins }
}

/** The record types the conference tests write and read. */
export const NSID = {
  /** A plan (and a public conference's own event). */
  event: 'community.lexicon.calendar.event',
  announcement: 'app.eventside.conference.announcement',
  card: 'app.eventside.block.card',
  chat: 'app.eventside.chat.message',
  role: 'app.eventside.conference.role',
  rules: 'app.eventside.conference.rules',
  join: 'app.eventside.intake.join',
} as const

/** A plan, as a member writes it into the conference space. */
export const plan = (name: string) => ({
  name,
  startsAt: '2027-04-30T19:00:00+02:00',
  createdAt: new Date().toISOString(),
})

/** An announcement. */
export const announcement = (text: string) => ({ text, createdAt: new Date().toISOString() })
