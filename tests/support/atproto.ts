// Plain atproto calls a test makes as a person, straight to their PDS on the
// run's vivarium: identity lookups, public records, records in their own repo
// in a permissioned space, and taking their account down and up again.

/** An account's own session at its PDS (vivarium's passwordless session). */
export interface Account {
  did: string
  handle: string
  accessJwt: string
}

// biome-ignore lint/suspicious/noExplicitAny: JSON from the network, under test
export type Json = Record<string, any>

export interface XrpcAnswer {
  status: number
  body: Json
}

/** Calls an XRPC method, never throwing on an error status. */
export async function xrpc(
  base: string,
  nsid: string,
  opts: {
    params?: Record<string, string | undefined>
    body?: unknown
    token?: string
    headers?: Record<string, string>
    method?: 'GET' | 'POST'
  } = {},
): Promise<XrpcAnswer> {
  const url = new URL(`/xrpc/${nsid}`, base)
  for (const [key, value] of Object.entries(opts.params ?? {})) {
    if (value !== undefined) url.searchParams.set(key, value)
  }
  const headers: Record<string, string> = { ...opts.headers }
  if (opts.token) headers.authorization = `Bearer ${opts.token}`
  if (opts.body !== undefined) headers['content-type'] = 'application/json'
  const res = await fetch(url, {
    method: opts.method ?? (opts.body === undefined ? 'GET' : 'POST'),
    headers,
    body: opts.body === undefined ? undefined : JSON.stringify(opts.body),
  })
  const text = await res.text()
  let body: Json
  try {
    body = text ? JSON.parse(text) : {}
  } catch {
    body = { raw: text }
  }
  return { status: res.status, body }
}

/** Like `xrpc`, but throws unless the answer is a success. */
export async function xrpcOk(base: string, nsid: string, opts: Parameters<typeof xrpc>[2] = {}): Promise<Json> {
  const answer = await xrpc(base, nsid, opts)
  if (answer.status < 200 || answer.status >= 300) {
    throw new Error(`${nsid} answered ${answer.status}: ${JSON.stringify(answer.body)}`)
  }
  return answer.body
}

/** A DID's document, from the run's PLC (vivarium serves it at `/{did}`). */
export async function didDocument(vivariumUrl: string, did: string): Promise<Json> {
  const res = await fetch(`${vivariumUrl}/${did}`)
  if (!res.ok) throw new Error(`no DID document for ${did}: ${res.status} ${await res.text()}`)
  return res.json()
}

/** A did:plc's current data (rotation keys, verification methods, services). */
export async function plcData(vivariumUrl: string, did: string): Promise<Json> {
  const res = await fetch(`${vivariumUrl}/${did}/data`)
  if (!res.ok) throw new Error(`no PLC data for ${did}: ${res.status} ${await res.text()}`)
  return res.json()
}

/** The endpoint of a service in a DID document, by its fragment. */
export function serviceEndpoint(doc: Json, fragment: string): string | undefined {
  const services: Json[] = doc.service ?? []
  return services.find((s) => s.id === `#${fragment}` || s.id === `${doc.id}#${fragment}`)?.serviceEndpoint
}

/** Where a DID's repo lives. */
export async function pdsOf(vivariumUrl: string, did: string): Promise<string> {
  const pds = serviceEndpoint(await didDocument(vivariumUrl, did), 'atproto_pds')
  if (!pds) throw new Error(`${did} names no PDS`)
  return pds
}

/** Where a space authority's spaces are hosted. */
export async function spaceHostOf(vivariumUrl: string, authority: string): Promise<string> {
  const doc = await didDocument(vivariumUrl, authority)
  const host = serviceEndpoint(doc, 'atproto_space_host') ?? serviceEndpoint(doc, 'atproto_pds')
  if (!host) throw new Error(`${authority} names no space host`)
  return host
}

/** The authority DID at the root of a space URI. */
export function authorityOf(space: string): string {
  const match = /^at:\/\/([^/]+)\/space\/[^/]+\/[^/]+$/.exec(space)
  if (!match) throw new Error(`not a space URI: ${space}`)
  return match[1]
}

/** Parses `at://{did}/{collection}/{rkey}`. */
export function parseAtUri(uri: string): { repo: string; collection: string; rkey: string } {
  const match = /^at:\/\/([^/]+)\/([^/]+)\/([^/]+)$/.exec(uri)
  if (!match) throw new Error(`not a record AT-URI: ${uri}`)
  return { repo: match[1], collection: match[2], rkey: match[3] }
}

/** Public records in a repo's collection. */
export async function publicRecords(vivariumUrl: string, did: string, collection: string): Promise<Json[]> {
  const body = await xrpcOk(await pdsOf(vivariumUrl, did), 'com.atproto.repo.listRecords', {
    params: { repo: did, collection, limit: '100' },
  })
  return body.records
}

/** Publishes a public record as the account, the way any other app would. */
export async function createPublicRecord(
  vivariumUrl: string,
  account: Account,
  collection: string,
  record: Json,
): Promise<{ uri: string; cid: string }> {
  const body = await xrpcOk(await pdsOf(vivariumUrl, account.did), 'com.atproto.repo.createRecord', {
    token: account.accessJwt,
    body: { repo: account.did, collection, record: { $type: collection, ...record } },
  })
  return { uri: body.uri, cid: body.cid }
}

/** The current revision of a public repo. */
export async function repoRev(vivariumUrl: string, did: string): Promise<string> {
  const body = await xrpcOk(await pdsOf(vivariumUrl, did), 'com.atproto.sync.getLatestCommit', {
    params: { did },
  })
  return body.rev
}

/** Writes a record into the account's own repo in a space, from its own PDS. */
export async function writeSpaceRecord(
  vivariumUrl: string,
  account: Account,
  space: string,
  collection: string,
  record: Json,
  rkey?: string,
): Promise<{ uri: string; cid: string }> {
  const body = await xrpcOk(await pdsOf(vivariumUrl, account.did), 'com.atproto.space.createRecord', {
    token: account.accessJwt,
    body: { space, repo: account.did, collection, rkey, record: { $type: collection, ...record } },
  })
  return { uri: body.uri, cid: body.cid }
}

/** Records in the account's own repo in a space (a user token reads its own repo). */
export async function ownSpaceRecords(
  vivariumUrl: string,
  account: Account,
  space: string,
  collection: string,
): Promise<Json[]> {
  const answer = await xrpc(await pdsOf(vivariumUrl, account.did), 'com.atproto.space.listRecords', {
    token: account.accessJwt,
    params: { space, repo: account.did, collection, limit: '100' },
  })
  // No repo in the space yet means no records in it.
  if (answer.body.error === 'RepoNotFound') return []
  if (answer.status !== 200) {
    throw new Error(`space listRecords answered ${answer.status}: ${JSON.stringify(answer.body)}`)
  }
  return answer.body.records
}

/** The revision of the account's own repo in a space, or undefined if it has none. */
export async function ownSpaceRev(vivariumUrl: string, account: Account, space: string): Promise<string | undefined> {
  const answer = await xrpc(await pdsOf(vivariumUrl, account.did), 'com.atproto.space.getLatestCommit', {
    token: account.accessJwt,
    params: { space, repo: account.did },
  })
  if (answer.body.error === 'RepoNotFound') return undefined
  if (answer.status !== 200) {
    throw new Error(`space getLatestCommit answered ${answer.status}: ${JSON.stringify(answer.body)}`)
  }
  return answer.body.commit.rev
}

/** A service-auth JWT from the account's PDS, as its PDS would mint for itself. */
export async function serviceAuth(vivariumUrl: string, account: Account, aud: string, lxm: string): Promise<string> {
  const body = await xrpcOk(await pdsOf(vivariumUrl, account.did), 'com.atproto.server.getServiceAuth', {
    token: account.accessJwt,
    params: { aud, lxm },
  })
  return body.token
}

/**
 * Takes the account's repo offline: its PDS refuses reads and writes to it,
 * which is how a test makes one person's PDS unreachable when every account
 * shares the run's vivarium.
 */
export async function takeOffline(vivariumUrl: string, account: Account): Promise<void> {
  await xrpcOk(await pdsOf(vivariumUrl, account.did), 'com.atproto.server.deactivateAccount', {
    token: account.accessJwt,
    body: {},
  })
}

/** Brings an account taken offline back. */
export async function bringOnline(vivariumUrl: string, account: Account): Promise<void> {
  await xrpcOk(await pdsOf(vivariumUrl, account.did), 'com.atproto.server.activateAccount', {
    token: account.accessJwt,
    body: {},
  })
}

/** Changes an account's handle. */
export async function changeHandle(vivariumUrl: string, account: Account, handle: string): Promise<void> {
  await xrpcOk(await pdsOf(vivariumUrl, account.did), 'com.atproto.identity.updateHandle', {
    token: account.accessJwt,
    body: { handle },
  })
}

/** Creates an account with an email address (vivarium treats it as confirmed). */
export async function createAccountWithEmail(vivariumUrl: string, handle: string, email: string): Promise<Account> {
  const body = await xrpcOk(vivariumUrl, 'com.atproto.server.createAccount', { body: { handle, email } })
  return { did: body.did, handle: body.handle, accessJwt: body.accessJwt }
}

const TID_CHARS = '234567abcdefghijklmnopqrstuvwxyz'

/** A TID for now: a repo revision as a PDS would assign it. */
export function tidNow(): string {
  let n = (BigInt(Date.now()) * 1000n) << 10n
  let out = ''
  for (let i = 0; i < 13; i++) {
    out = TID_CHARS[Number(n & 31n)] + out
    n >>= 5n
  }
  return out
}

/** Polls `check` until it holds, then returns its last value; fails after `ms`. */
export async function eventually<T>(
  check: () => Promise<T>,
  holds: (value: T) => boolean,
  what: string,
  ms = 10_000,
): Promise<T> {
  const until = Date.now() + ms
  let last: T = await check()
  while (!holds(last)) {
    if (Date.now() > until)
      throw new Error(`timed out after ${ms}ms waiting until ${what}; last saw ${JSON.stringify(last)}`)
    await new Promise((done) => setTimeout(done, 250))
    last = await check()
  }
  return last
}
