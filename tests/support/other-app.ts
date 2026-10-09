// "Another app": a second atproto OAuth client, written from the protocol and
// nothing of ours, that reads a permissioned space the way any app would.
//
// It serves its own client metadata and keys on loopback, signs people in
// through their PDS (PAR, PKCE, DPoP, private_key_jwt), asks their PDS for a
// delegation token for a space, trades it at the space's host (found in the
// authority's DID document: for a conference, the organization's own PDS)
// for a space credential, proving its P-256 key
// with an RFC 9421 signature and its client ID with a client attestation, and
// then uses the credential at the host (listRepos) and at each writer's PDS
// (their records).
import { createHash, randomBytes } from 'node:crypto'
import { createServer, type Server } from 'node:http'
import type { AddressInfo } from 'node:net'
import { type Account, authorityOf, type Json, pdsOf, spaceHostOf, type XrpcAnswer, xrpc } from './atproto.ts'
import { browse } from './browser.ts'
import { dpopProof, P256Key, spaceSignatureHeaders } from './crypto.ts'
import { CONFERENCE_SPACE_TYPE } from './scopes.ts'

/**
 * What another app asks for: reading conference spaces as delegated by the
 * person, and reading the person's own records in them.
 */
export const READ_SCOPES = ['atproto', `space:${CONFERENCE_SPACE_TYPE}?authority=*&action=read&action=read_self`]

const ASSERTION_TYPE = 'urn:ietf:params:oauth:client-assertion-type:jwt-bearer'

export class OtherApp {
  /** The key the app signs space requests with; credentials are bound to it. */
  readonly appKey = P256Key.generate()
  private readonly clientKey = P256Key.generate()

  private readonly vivariumUrl: string
  private readonly server: Server
  readonly origin: string

  private constructor(vivariumUrl: string, server: Server, origin: string) {
    this.vivariumUrl = vivariumUrl
    this.server = server
    this.origin = origin
  }

  /** Starts the app: its client metadata and keys, served on loopback. */
  static async start(vivariumUrl: string): Promise<OtherApp> {
    let app: OtherApp | undefined
    const server = createServer((req, res) => {
      const json = (body: unknown) => {
        res.writeHead(200, { 'content-type': 'application/json' })
        res.end(JSON.stringify(body))
      }
      if (!app) return res.writeHead(503).end()
      const path = new URL(req.url ?? '/', app.origin).pathname
      if (path === '/client-metadata.json') return json(app.metadata())
      if (path === '/jwks.json') return json({ keys: [app.clientKey.publicJwk()] })
      res.writeHead(404).end()
    })
    await new Promise<void>((ready) => server.listen(0, '127.0.0.1', ready))
    const { port } = server.address() as AddressInfo
    app = new OtherApp(vivariumUrl, server, `http://127.0.0.1:${port}`)
    return app
  }

  get clientId(): string {
    return `${this.origin}/client-metadata.json`
  }

  private get redirectUri(): string {
    return `${this.origin}/callback`
  }

  private metadata() {
    return {
      client_id: this.clientId,
      client_name: 'Another app',
      client_uri: this.origin,
      redirect_uris: [this.redirectUri],
      scope: READ_SCOPES.join(' '),
      grant_types: ['authorization_code', 'refresh_token'],
      response_types: ['code'],
      application_type: 'web',
      token_endpoint_auth_method: 'private_key_jwt',
      token_endpoint_auth_signing_alg: 'ES256',
      dpop_bound_access_tokens: true,
      jwks: { keys: [this.clientKey.publicJwk()] },
    }
  }

  async stop(): Promise<void> {
    await new Promise<void>((done) => this.server.close(() => done()))
  }

  private clientAssertion(audience: string): string {
    const now = Math.floor(Date.now() / 1000)
    return this.clientKey.jwt(
      { kid: this.clientKey.kid },
      {
        iss: this.clientId,
        sub: this.clientId,
        aud: audience,
        jti: randomBytes(16).toString('hex'),
        iat: now,
        exp: now + 60,
      },
    )
  }

  /** A client attestation for a space host, proving this app's client ID. */
  attestation(authority: string): string {
    const now = Math.floor(Date.now() / 1000)
    return this.clientKey.jwt(
      { typ: 'atproto-client-attestation+jwt', kid: this.clientKey.kid },
      {
        iss: this.clientId,
        sub: this.clientId,
        aud: `${authority}#atproto_space_host`,
        jti: randomBytes(16).toString('hex'),
        iat: now,
        exp: now + 60,
      },
    )
  }

  /** Signs a person in to this app through their PDS's consent screens. */
  async signIn(person: Pick<Account, 'did' | 'handle'>): Promise<AppSession> {
    const pds = await pdsOf(this.vivariumUrl, person.did)
    const resource = await (await fetch(`${pds}/.well-known/oauth-protected-resource`)).json()
    const issuer: string = resource.authorization_servers[0]
    const server = await (await fetch(`${issuer}/.well-known/oauth-authorization-server`)).json()
    const dpopKey = P256Key.generate()
    const verifier = randomBytes(32).toString('base64url')
    const challenge = createHash('sha256').update(verifier).digest('base64url')

    const par = await fetch(server.pushed_authorization_request_endpoint, {
      method: 'POST',
      headers: { dpop: dpopProof(dpopKey, 'POST', server.pushed_authorization_request_endpoint) },
      body: new URLSearchParams({
        client_id: this.clientId,
        response_type: 'code',
        redirect_uri: this.redirectUri,
        scope: READ_SCOPES.join(' '),
        state: randomBytes(12).toString('hex'),
        code_challenge: challenge,
        code_challenge_method: 'S256',
        login_hint: person.handle,
        client_assertion_type: ASSERTION_TYPE,
        client_assertion: this.clientAssertion(issuer),
      }),
    })
    if (par.status !== 201 && par.status !== 200) {
      throw new Error(`another app's PAR answered ${par.status}: ${await par.text()}`)
    }
    const { request_uri } = await par.json()
    let nonce = par.headers.get('dpop-nonce') ?? undefined

    const authorize = new URL(server.authorization_endpoint)
    authorize.searchParams.set('client_id', this.clientId)
    authorize.searchParams.set('request_uri', request_uri)
    const consent = await browse(authorize.toString(), { account: person.handle, stopAt: this.redirectUri })
    if (!consent.location)
      throw new Error(`another app's sign-in didn't come back: ${consent.res.status} ${consent.text}`)
    const code = new URL(consent.location).searchParams.get('code')
    if (!code) throw new Error(`another app's sign-in came back without a code: ${consent.location}`)

    for (let attempt = 0; attempt < 2; attempt++) {
      const res = await fetch(server.token_endpoint, {
        method: 'POST',
        headers: { dpop: dpopProof(dpopKey, 'POST', server.token_endpoint, { nonce }) },
        body: new URLSearchParams({
          grant_type: 'authorization_code',
          code,
          redirect_uri: this.redirectUri,
          code_verifier: verifier,
          client_id: this.clientId,
          client_assertion_type: ASSERTION_TYPE,
          client_assertion: this.clientAssertion(issuer),
        }),
      })
      const body = await res.json()
      nonce = res.headers.get('dpop-nonce') ?? nonce
      if (res.status === 400 && body.error === 'use_dpop_nonce') continue
      if (!res.ok) throw new Error(`another app's token request answered ${res.status}: ${JSON.stringify(body)}`)
      if (body.sub !== person.did) throw new Error(`another app signed in ${body.sub}, not ${person.did}`)
      return new AppSession(this, this.vivariumUrl, person.did, pds, body.access_token, dpopKey, nonce)
    }
    throw new Error("another app's token request kept asking for a DPoP nonce")
  }
}

/** A person signed in to the other app. */
export class AppSession {
  private readonly app: OtherApp
  private readonly vivariumUrl: string
  readonly did: string
  private readonly pds: string
  private readonly accessToken: string
  private readonly dpopKey: P256Key
  private nonce: string | undefined

  constructor(
    app: OtherApp,
    vivariumUrl: string,
    did: string,
    pds: string,
    accessToken: string,
    dpopKey: P256Key,
    nonce: string | undefined,
  ) {
    this.app = app
    this.vivariumUrl = vivariumUrl
    this.did = did
    this.pds = pds
    this.accessToken = accessToken
    this.dpopKey = dpopKey
    this.nonce = nonce
  }

  /** An authenticated GET at the person's PDS, as this app. */
  private async atPds(nsid: string, params: Record<string, string>): Promise<XrpcAnswer> {
    const url = new URL(`/xrpc/${nsid}`, this.pds)
    for (const [key, value] of Object.entries(params)) url.searchParams.set(key, value)
    for (let attempt = 0; attempt < 2; attempt++) {
      const res = await fetch(url, {
        headers: {
          authorization: `DPoP ${this.accessToken}`,
          dpop: dpopProof(this.dpopKey, 'GET', url.toString(), { nonce: this.nonce, accessToken: this.accessToken }),
        },
      })
      this.nonce = res.headers.get('dpop-nonce') ?? this.nonce
      const body = await res.json().catch(() => ({}))
      if (res.status === 401 && body.error === 'use_dpop_nonce') continue
      return { status: res.status, body }
    }
    throw new Error(`${nsid} kept asking another app for a DPoP nonce`)
  }

  /** A delegation token for a space, from the person's PDS. */
  async delegationToken(space: string): Promise<string> {
    const answer = await this.atPds('com.atproto.space.getDelegationToken', { space })
    if (answer.status !== 200) {
      throw new Error(`getDelegationToken answered ${answer.status}: ${JSON.stringify(answer.body)}`)
    }
    return answer.body.token
  }

  /** The person's own records in a space, read at their PDS with this app's OAuth token (read_self). */
  ownRecords(space: string, collection: string): Promise<XrpcAnswer> {
    return this.atPds('com.atproto.space.listRecords', { space, repo: this.did, collection, limit: '100' })
  }

  /** Asks the space's host for a credential, delegated by this person. */
  async requestCredential(space: string): Promise<XrpcAnswer> {
    const authority = authorityOf(space)
    const host = await spaceHostOf(this.vivariumUrl, authority)
    const delegation = await this.delegationToken(space)
    return xrpc(host, 'com.atproto.space.getSpaceCredential', {
      body: { space, clientAttestation: this.app.attestation(authority) },
      headers: spaceSignatureHeaders(this.app.appKey, `Bearer ${delegation}`),
    })
  }

  /** Access to a space as this person, or a thrown error if the host refuses. */
  async open(space: string): Promise<SpaceAccess> {
    const answer = await this.requestCredential(space)
    if (answer.status !== 200 || typeof answer.body.credential !== 'string') {
      throw new Error(
        `the space host refused a credential for ${space}: ${answer.status} ${JSON.stringify(answer.body)}`,
      )
    }
    const host = await spaceHostOf(this.vivariumUrl, authorityOf(space))
    return new SpaceAccess(this.app, this.vivariumUrl, space, host, answer.body.credential)
  }
}

/** What another app can do with a space credential. */
export class SpaceAccess {
  readonly authority: string
  private readonly app: OtherApp
  private readonly vivariumUrl: string
  readonly space: string
  readonly host: string
  readonly credential: string

  constructor(app: OtherApp, vivariumUrl: string, space: string, host: string, credential: string) {
    this.app = app
    this.vivariumUrl = vivariumUrl
    this.space = space
    this.host = host
    this.credential = credential
    this.authority = authorityOf(space)
  }

  private headers(audience: string) {
    return spaceSignatureHeaders(this.app.appKey, `Atproto-Space ${this.credential}`, audience)
  }

  /** A credential-authenticated call to the space host (audience: the authority). */
  hostCall(nsid: string, params: Record<string, string> = {}): Promise<XrpcAnswer> {
    return xrpc(this.host, nsid, { params: { space: this.space, ...params }, headers: this.headers(this.authority) })
  }

  /** Every writer in the space, from the host's `listRepos`. */
  async listRepos(): Promise<Json[]> {
    const repos: Json[] = []
    let cursor: string | undefined
    for (let page = 0; page < 50; page++) {
      const answer = await this.hostCall('com.atproto.space.listRepos', cursor ? { cursor } : {})
      if (answer.status !== 200) throw new Error(`listRepos answered ${answer.status}: ${JSON.stringify(answer.body)}`)
      repos.push(...answer.body.repos)
      if (!answer.body.cursor || answer.body.repos.length === 0) return repos
      cursor = String(answer.body.cursor)
    }
    return repos
  }

  /** The space's policies, from its host (`simplespace.getSpace`, with the credential). */
  async getSpace(): Promise<Json> {
    const answer = await this.hostCall('com.atproto.simplespace.getSpace')
    if (answer.status !== 200) throw new Error(`getSpace answered ${answer.status}: ${JSON.stringify(answer.body)}`)
    return answer.body
  }

  /** The DIDs of the space's writers. */
  async writers(): Promise<string[]> {
    return (await this.listRepos()).map((repo) => repo.did)
  }

  /** A writer's records in the space, read from the writer's own PDS. */
  async listRecords(repo: string, collection: string): Promise<XrpcAnswer> {
    return xrpc(await pdsOf(this.vivariumUrl, repo), 'com.atproto.space.listRecords', {
      params: { space: this.space, repo, collection, limit: '100' },
      headers: this.headers(repo),
    })
  }

  /** Like `listRecords`, but throws unless it worked; no repo in the space means no records. */
  async records(repo: string, collection: string): Promise<Json[]> {
    const answer = await this.listRecords(repo, collection)
    if (answer.body.error === 'RepoNotFound') return []
    if (answer.status !== 200) {
      throw new Error(`listRecords of ${repo} answered ${answer.status}: ${JSON.stringify(answer.body)}`)
    }
    return answer.body.records
  }
}
