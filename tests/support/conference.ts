// Conferences for tests, set up the way an operator would
// (features/conference-space.md, design review rounds 1 and 2):
//
// - the organization is an ordinary account on the run's vivarium, connected
//   over OAuth with the server binary's `admin org connect`;
// - each conference's space lives on the organization's own PDS, with
//   eventside (the server's did:web) as its managing app;
// - admins connect with `admin connect` over OAuth, and every decision the
//   CLI makes runs `--as` a connected admin;
// - attendees use the app's conference XRPC with a browser's cookies.
//
// The CLI contract is the usage text in crates/server/src/conference/cli.rs.
// Later features' tests seed conferences with these too.
import { spawn } from 'node:child_process'
import { mkdtempSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { createInterface } from 'node:readline'
import type { VivariumScope } from '@vivarium-dev/client'
import { type Account, eventually, type Json, ownSpaceRecords, pdsOf, type XrpcAnswer, xrpcOk } from './atproto.ts'
import { type CookieJar, getSession, signIn } from './auth.ts'
import { type BrowsedTo, browse } from './browser.ts'
import { CONFERENCE_SPACE_TYPE } from './scopes.ts'
import { freePort, type RunningServer, SERVER_BIN, spawnServer, tempDatabase } from './server.ts'

/** A running server, and the environment its CLI runs with. */
export interface Deployment {
  /** The server's URL, which is also its PUBLIC_URL. */
  url: string
  /** The environment the server runs with, for its `admin` CLI. */
  env: Record<string, string>
  stop(): Promise<void>
  /**
   * Stops the server and starts it again on the same database, port and
   * client ID. `changes` sets environment variables, or with `undefined`
   * unsets them, for the server and its CLI from then on.
   */
  restart(changes?: Record<string, string | undefined>): Promise<void>
}

function vivariumUrl(): string {
  const url = process.env.VIVARIUM_URL
  if (!url) throw new Error('conference tests need the run vivarium in VIVARIUM_URL; run through with-vivarium')
  return url
}

/** Starts a server of its own, on a fresh database, whose PUBLIC_URL is its own URL. */
export async function deploy(extra: Record<string, string> = {}): Promise<Deployment> {
  const port = String(await freePort())
  const env: Record<string, string> = {
    PORT: port,
    PUBLIC_URL: `http://127.0.0.1:${port}`,
    DATABASE_URL: tempDatabase().databaseUrl,
    ATPROTO_URL: vivariumUrl(),
    SIGNUP_PDS_URL: vivariumUrl(),
    ALLOW_PRIVATE_NETWORK: 'true',
    ...extra,
  }
  let server: RunningServer | undefined = await spawnServer(env)
  return {
    url: server.url,
    env,
    async stop() {
      await server?.stop()
      server = undefined
    },
    async restart(changes = {}) {
      await server?.stop()
      for (const [key, value] of Object.entries(changes)) {
        if (value === undefined) delete env[key]
        else env[key] = value
      }
      server = await spawnServer(env)
    },
  }
}

/** The Playwright run's server, as its CLI sees it (see playwright.config.ts). */
export function e2eDeployment(baseURL: string): Deployment {
  const databasePath = process.env.E2E_DATABASE_PATH
  if (!databasePath) throw new Error('e2e needs E2E_DATABASE_PATH: run through `pnpm test:e2e`')
  return {
    url: baseURL,
    env: {
      PUBLIC_URL: baseURL,
      DATABASE_URL: `sqlite://${databasePath}?mode=rwc`,
      ATPROTO_URL: vivariumUrl(),
      SIGNUP_PDS_URL: vivariumUrl(),
      ALLOW_PRIVATE_NETWORK: 'true',
    },
    stop: async () => {},
    restart: async () => {
      throw new Error("the e2e server can't be restarted")
    },
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

/** Runs `conference-server admin <args>` against a deployment, never throwing on a failed command. */
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

/**
 * Runs a CLI command that prints a URL to open (`connect`, `org connect`),
 * opens it in a browser as `account`, gets through the PDS's consent screens,
 * and waits for the command to finish.
 */
export async function cliWithSignIn(
  dep: Deployment,
  args: string[],
  account: string,
  opts: { env?: Record<string, string>; jar?: CookieJar } = {},
): Promise<{ page: BrowsedTo & { jar: CookieJar }; cli: CliResult }> {
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
      reject(new Error(`admin ${args.join(' ')} printed no URL to open within 30s\n${stdout}\n${stderr}`))
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
      reject(
        new Error(`admin ${args.join(' ')} exited (${code}) before printing a URL: ${stderr.trim() || stdout.trim()}`),
      )
    })
  })
  const page = await browse(url, { account, jar: opts.jar })
  const code = await Promise.race([
    exited,
    new Promise<'timeout'>((resolve) => setTimeout(() => resolve('timeout'), 30_000)),
  ])
  if (code === 'timeout') {
    child.kill()
    throw new Error(`admin ${args.join(' ')} didn't finish within 30s of signing in\n${stdout}\n${stderr}`)
  }
  return { page, cli: { args, code, stdout, stderr } }
}

/** Connects a person as an admin (`admin connect`), failing unless it worked. */
export async function connectAdmin(dep: Deployment, person: Pick<Account, 'handle'>): Promise<CliResult> {
  const { cli: result } = await cliWithSignIn(dep, ['connect', person.handle], person.handle)
  if (result.code !== 0) {
    throw new Error(`admin connect ${person.handle} failed (exit ${result.code}): ${result.stderr.trim()}`)
  }
  return result
}

/** The organization: its own account on the run's vivarium, connected to the server. */
export interface Org {
  /** The organization's account, with its own session (the test plays its owner). */
  account: Account
  did: string
  handle: string
}

/** Connects an organization's account with `admin org connect`, and returns what it printed. */
export async function connectOrgResult(dep: Deployment, account: Account) {
  return cliWithSignIn(dep, ['org', 'connect', account.handle, '--json'], account.handle)
}

/** Connects an organization's account, failing unless it worked. */
export async function connectOrg(dep: Deployment, account: Account): Promise<Org> {
  const { cli: result } = await connectOrgResult(dep, account)
  if (result.code !== 0) {
    throw new Error(`admin org connect ${account.handle} failed (exit ${result.code}): ${result.stderr.trim()}`)
  }
  return { account, did: account.did, handle: account.handle }
}

export type JoinMethod = 'code' | 'list' | 'open'

export interface ConferenceSpec {
  name: string
  startsAt: string
  endsAt: string
  city: string
  description?: string
  /** Theme tokens, by name without the `--g-` prefix: `{ 'color-primary': '#b0306a' }`. */
  theme?: Record<string, string>
}

export const ATMOSPHERECONF: ConferenceSpec = {
  name: 'AtmosphereConf 2027',
  startsAt: '2027-04-29T09:00:00+02:00',
  endsAt: '2027-05-02T18:00:00+02:00',
  city: 'Amsterdam',
  description: 'The atproto community conference, at the Openbare Bibliotheek Amsterdam.',
  theme: { 'color-primary': '#e4572e' },
}

export interface Conference {
  org: Org
  /** `at://{org did}/space/app.eventside.private/{rkey}`. */
  space: string
  /** The public `community.lexicon.calendar.event`'s AT-URI. */
  event: string
}

/** Creates a public conference as a connected admin, who becomes its first owner. */
export async function createConference(
  dep: Deployment,
  org: Org,
  as: Pick<Account, 'handle'>,
  spec: ConferenceSpec = ATMOSPHERECONF,
): Promise<Conference> {
  const args = ['conference', 'create', '--org', org.did, '--as', as.handle]
  args.push('--name', spec.name, '--starts', spec.startsAt, '--ends', spec.endsAt, '--city', spec.city)
  if (spec.description) args.push('--description', spec.description)
  if (spec.theme) args.push('--theme', JSON.stringify(spec.theme))
  const out = await cliOkJson(dep, args)
  return { org, space: out.space, event: out.event }
}

/** Makes a person an owner or staff member (`admin add`), as an admin. Never throws on refusal. */
export function addAdmin(
  dep: Deployment,
  conference: Conference,
  person: Pick<Account, 'handle'>,
  role: 'owner' | 'staff',
  as: Pick<Account, 'handle'>,
): Promise<CliResult> {
  return cli(dep, ['admin', 'add', person.handle, '--role', role, '--conference', conference.space, '--as', as.handle])
}

/** Removes a person as an admin (`admin remove`), as an admin. Never throws on refusal. */
export function removeAdmin(
  dep: Deployment,
  conference: Conference,
  person: Pick<Account, 'handle'>,
  as: Pick<Account, 'handle'>,
): Promise<CliResult> {
  return cli(dep, ['admin', 'remove', person.handle, '--conference', conference.space, '--as', as.handle])
}

/** Chooses a conference's join methods (all others off). */
export function setMethods(
  dep: Deployment,
  conference: Conference,
  methods: JoinMethod[],
  as: Pick<Account, 'handle'>,
): Promise<CliResult> {
  return cliOk(dep, [
    'join',
    'set',
    '--conference',
    conference.space,
    '--methods',
    methods.join(','),
    '--as',
    as.handle,
  ])
}

/** Creates a shared invite code. */
export function sharedCode(
  dep: Deployment,
  conference: Conference,
  code: string,
  as: Pick<Account, 'handle'>,
  opts: { expires?: string; maxUses?: number } = {},
): Promise<CliResult> {
  const args = ['code', 'create', code, '--conference', conference.space, '--as', as.handle]
  if (opts.expires) args.push('--expires', opts.expires)
  if (opts.maxUses !== undefined) args.push('--max-uses', String(opts.maxUses))
  return cliOk(dep, args)
}

/** Imports an attendee list, as a ticketing tool's CSV export with a handle column. Never throws on refusal. */
export function importList(
  dep: Deployment,
  conference: Conference,
  handles: string[],
  as: Pick<Account, 'handle'>,
): Promise<CliResult> {
  const file = join(mkdtempSync(join(tmpdir(), 'eventside-list-')), 'attendees.csv')
  writeFileSync(file, `${['name,handle', ...handles.map((h, i) => `Attendee ${i + 1},${h}`)].join('\n')}\n`)
  return cli(dep, ['list', 'import', file, '--conference', conference.space, '--as', as.handle])
}

export type MemberVerb = 'add' | 'remove' | 'ban' | 'unban'

/** `member add|remove|ban|unban <handle>`, as an admin. Never throws on refusal. */
export function member(
  dep: Deployment,
  conference: Conference,
  verb: MemberVerb,
  person: Pick<Account, 'handle'>,
  as: Pick<Account, 'handle'>,
  opts: { env?: Record<string, string> } = {},
): Promise<CliResult> {
  return cli(dep, ['member', verb, person.handle, '--conference', conference.space, '--as', as.handle], opts)
}

/** `member role <handle> --role …`, as an admin. Never throws on refusal. */
export function setRole(
  dep: Deployment,
  conference: Conference,
  person: Pick<Account, 'handle'>,
  role: 'owner' | 'staff' | 'speaker' | 'attendee',
  as: Pick<Account, 'handle'>,
): Promise<CliResult> {
  return cli(dep, [
    'member',
    'role',
    person.handle,
    '--role',
    role,
    '--conference',
    conference.space,
    '--as',
    as.handle,
  ])
}

/** Allows another app's client ID to read a conference's space. Never throws on refusal. */
export function allowApp(
  dep: Deployment,
  conference: Conference,
  clientId: string,
  as: Pick<Account, 'handle'>,
): Promise<CliResult> {
  return cli(dep, ['apps', 'allow', clientId, '--conference', conference.space, '--use', 'read', '--as', as.handle])
}

/** Takes an app off a conference's allowed apps. Never throws on refusal. */
export function disallowApp(
  dep: Deployment,
  conference: Conference,
  clientId: string,
  as: Pick<Account, 'handle'>,
): Promise<CliResult> {
  return cli(dep, ['apps', 'disallow', clientId, '--conference', conference.space, '--as', as.handle])
}

/** The records eventside counts in a conference's space, in one collection (`records list`). */
export async function countedRecords(
  dep: Deployment,
  conference: Conference,
  collection: string,
  as: Pick<Account, 'handle'>,
): Promise<Json[]> {
  const out = await cliOkJson(dep, [
    'records',
    'list',
    '--conference',
    conference.space,
    '--collection',
    collection,
    '--as',
    as.handle,
  ])
  return out.records
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

  /** `app.eventside.conference.get`: by the event's AT-URI (DID or handle) or the space. */
  get(conference: string): Promise<XrpcAnswer> {
    return this.call('app.eventside.conference.get', { params: { conference } })
  }

  join(conference: Conference | string, code?: string): Promise<XrpcAnswer> {
    const space = typeof conference === 'string' ? conference : conference.space
    return this.call('app.eventside.conference.join', {
      body: code === undefined ? { conference: space } : { conference: space, code },
    })
  }

  leave(conference: Conference | string): Promise<XrpcAnswer> {
    const space = typeof conference === 'string' ? conference : conference.space
    return this.call('app.eventside.conference.leave', { body: { conference: space } })
  }

  getMembership(conference: Conference | string): Promise<XrpcAnswer> {
    const space = typeof conference === 'string' ? conference : conference.space
    return this.call('app.eventside.conference.getMembership', { params: { conference: space } })
  }

  /** The app's answer to "am I a member, and in which role", failing on anything but an answer. */
  async membership(conference: Conference | string): Promise<{ member: boolean; role?: string }> {
    const answer = await this.getMembership(conference)
    if (answer.status !== 200) {
      throw new Error(`getMembership answered ${answer.status}: ${JSON.stringify(answer.body)}`)
    }
    return { member: answer.body.member === true, role: answer.body.role }
  }

  async isMember(conference: Conference | string): Promise<boolean> {
    return (await this.membership(conference)).member
  }

  async role(conference: Conference | string): Promise<string | undefined> {
    const membership = await this.membership(conference)
    return membership.member ? membership.role : undefined
  }
}

/** `app.eventside.conference.get`, signed out. */
export async function publicConference(dep: Deployment, conference: string): Promise<XrpcAnswer> {
  const url = new URL('/xrpc/app.eventside.conference.get', dep.url)
  url.searchParams.set('conference', conference)
  return parse(await fetch(url))
}

/** Any conference XRPC call, signed out. */
export async function signedOut(dep: Deployment, nsid: string, params: Record<string, string>): Promise<XrpcAnswer> {
  const url = new URL(`/xrpc/${nsid}`, dep.url)
  for (const [key, value] of Object.entries(params)) url.searchParams.set(key, value)
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

/** The cast of the test cases. */
export type Name = 'olga' | 'kees' | 'pim' | 'lotte' | 'ana' | 'bram' | 'joost' | 'ruud' | 'mallory'

export interface Seeded {
  org: Org
  conference: Conference
  /** The first owner, who created the conference. */
  olga: Account
  /** Every other admin, by name. */
  admins: Partial<Record<Name, Account>>
}

/**
 * Seeds AtmosphereConf: the organization "Atmosphere" connects, Olga connects
 * and creates the conference (so she's its first owner), then the other
 * owners and staff are added by Olga and connect, unless `unconnected` names
 * them. Join methods are turned on last.
 */
export async function seedConference(
  dep: Deployment,
  newAccount: NewAccount,
  opts: {
    owners?: Name[]
    staff?: Name[]
    unconnected?: Name[]
    methods?: JoinMethod[]
    spec?: ConferenceSpec
  } = {},
): Promise<Seeded> {
  const org = await connectOrg(dep, await newAccount('atmosphere'))
  const olga = await newAccount('olga')
  await connectAdmin(dep, olga)
  const conference = await createConference(dep, org, olga, opts.spec ?? ATMOSPHERECONF)
  const admins: Partial<Record<Name, Account>> = {}
  for (const [names, role] of [
    [opts.owners ?? [], 'owner'],
    [opts.staff ?? [], 'staff'],
  ] as const) {
    for (const name of names) {
      const person = await newAccount(name)
      admins[name] = person
      const added = await addAdmin(dep, conference, person, role, olga)
      if (added.code !== 0) throw new Error(`admin add ${name} failed: ${added.stderr.trim() || added.stdout.trim()}`)
      if (!opts.unconnected?.includes(name)) await connectAdmin(dep, person)
    }
  }
  if (opts.methods) await setMethods(dep, conference, opts.methods, olga)
  return { org, conference, olga, admins }
}

/** Signs a person in and joins them with a code, failing unless they're admitted. */
export async function joinWithCode(dep: Deployment, person: Account, conference: Conference, code: string) {
  const attendee = await Attendee.signIn(dep, person)
  const answer = await attendee.join(conference, code)
  if (answer.status !== 200 || answer.body.status !== 'joined') {
    throw new Error(`${person.handle} couldn't join with ${code}: ${answer.status} ${JSON.stringify(answer.body)}`)
  }
  return attendee
}

/** The record types the conference tests write and read. */
export const NSID = {
  event: 'community.lexicon.calendar.event',
  member: 'app.eventside.conference.member',
  ban: 'app.eventside.conference.ban',
  apps: 'app.eventside.conference.apps',
  sidecar: 'app.eventside.conference.sidecar',
  feed: 'app.eventside.feed.feed',
  /** Something an attendee writes into the space. */
  post: 'app.eventside.feed.post',
} as const

/** A post, as an attendee writes it into the space. */
export const post = (text: string) => ({ text, createdAt: new Date().toISOString() })

/** The organization's records in the space, read with its own session (as its owner could). */
export function orgSpaceRecords(conference: Conference, collection: string): Promise<Json[]> {
  return ownSpaceRecords(vivariumUrl(), conference.org.account, conference.space, collection)
}

/** The member record for a person in the organization's repo, once the outbox has written it. */
export async function memberRecord(
  conference: Conference,
  did: string,
  holds: (record: Json | undefined) => boolean = (record) => record !== undefined,
  what = `the member record for ${did}`,
): Promise<Json | undefined> {
  const records = await eventually(
    () => orgSpaceRecords(conference, NSID.member),
    (records) => holds(records.find((r) => r.rkey === did || r.value?.subject === did)),
    what,
  )
  return records.find((r) => r.rkey === did || r.value?.subject === did)
}

/** Whether a space URI is a conference space of this design. */
export const isConferenceSpace = (space: string) =>
  new RegExp(`^at://did:[a-z0-9]+:[A-Za-z0-9._:%-]+/space/${CONFERENCE_SPACE_TYPE.replaceAll('.', '\\.')}/[^/]+$`).test(
    space,
  )

/** Eventside's own DID document: the server's did:web, served at `/.well-known/did.json`. */
export async function eventsideDoc(dep: Deployment): Promise<Json> {
  const res = await fetch(`${dep.url}/.well-known/did.json`)
  if (!res.ok) throw new Error(`eventside's DID document answered ${res.status}: ${await res.text()}`)
  return res.json()
}

/**
 * A conference space's policies, from the organization's PDS, read with the
 * organization's own session (`simplespace.getSpace`).
 */
export async function spacePolicies(conference: Conference): Promise<Json> {
  const pds = await pdsOf(vivariumUrl(), conference.org.did)
  return xrpcOk(pds, 'com.atproto.simplespace.getSpace', {
    token: conference.org.account.accessJwt,
    params: { space: conference.space },
  })
}
