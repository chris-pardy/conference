// The cryptography a test needs to act as another atproto app: P-256 keys as
// JWKs and did:keys, ES256 JWTs, DPoP proofs, and the RFC 9421 HTTP message
// signatures (label `atproto-space`) that permissioned spaces use in place of
// DPoP. Node's own crypto only, so the tests add no dependencies.
import { createHash, generateKeyPairSync, type KeyObject, randomBytes, sign } from 'node:crypto'

const b64url = (data: Buffer | string) => Buffer.from(data).toString('base64url')

/** A P-256 key pair, usable as a JWK, a did:key and an ES256 signer. */
export class P256Key {
  readonly kid: string
  private readonly privateKey: KeyObject
  private readonly publicKey: KeyObject

  private constructor(privateKey: KeyObject, publicKey: KeyObject) {
    this.privateKey = privateKey
    this.publicKey = publicKey
    this.kid = randomBytes(8).toString('hex')
  }

  static generate(): P256Key {
    const { privateKey, publicKey } = generateKeyPairSync('ec', { namedCurve: 'P-256' })
    return new P256Key(privateKey, publicKey)
  }

  /** The public JWK, with this key's `kid`, `alg` and `use`. */
  publicJwk(): Record<string, string> {
    const { kty, crv, x, y } = this.publicKey.export({ format: 'jwk' }) as Record<string, string>
    return { kty, crv, x, y, kid: this.kid, alg: 'ES256', use: 'sig' }
  }

  /** The bare public JWK (no `kid`), as a DPoP proof embeds it. */
  bareJwk(): Record<string, string> {
    const { kty, crv, x, y } = this.publicKey.export({ format: 'jwk' }) as Record<string, string>
    return { kty, crv, x, y }
  }

  /** `did:key:z…`: multicodec p256-pub (0x1200), compressed point, base58btc. */
  didKey(): string {
    const { x, y } = this.publicKey.export({ format: 'jwk' }) as { x: string; y: string }
    const xb = Buffer.from(x, 'base64url')
    const yb = Buffer.from(y, 'base64url')
    const compressed = Buffer.concat([Buffer.from([yb[yb.length - 1] % 2 === 0 ? 0x02 : 0x03]), xb])
    return `did:key:z${base58btc(Buffer.concat([Buffer.from([0x80, 0x24]), compressed]))}`
  }

  /** An ECDSA P-256 SHA-256 signature, as 64 bytes r||s. */
  sign(message: Buffer | string): Buffer {
    return sign('sha256', Buffer.from(message), { key: this.privateKey, dsaEncoding: 'ieee-p1363' })
  }

  /** A compact ES256 JWT. */
  jwt(header: Record<string, unknown>, payload: Record<string, unknown>): string {
    const input = `${b64url(JSON.stringify({ alg: 'ES256', ...header }))}.${b64url(JSON.stringify(payload))}`
    return `${input}.${b64url(this.sign(input))}`
  }

  /** The RFC 7638 thumbprint of the public key. */
  thumbprint(): string {
    const { crv, kty, x, y } = this.bareJwk()
    return b64url(createHash('sha256').update(JSON.stringify({ crv, kty, x, y })).digest())
  }
}

const ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'

export function base58btc(bytes: Buffer): string {
  let n = BigInt(`0x${bytes.toString('hex') || '0'}`)
  let out = ''
  while (n > 0n) {
    out = ALPHABET[Number(n % 58n)] + out
    n /= 58n
  }
  for (const byte of bytes) {
    if (byte !== 0) break
    out = `1${out}`
  }
  return out
}

/** A DPoP proof for a request, with the access token's hash when there is one. */
export function dpopProof(
  key: P256Key,
  method: string,
  url: string,
  opts: { nonce?: string; accessToken?: string } = {},
): string {
  const u = new URL(url)
  const payload: Record<string, unknown> = {
    jti: randomBytes(16).toString('hex'),
    htm: method,
    htu: `${u.origin}${u.pathname}`,
    iat: Math.floor(Date.now() / 1000),
  }
  if (opts.nonce) payload.nonce = opts.nonce
  if (opts.accessToken) payload.ath = b64url(createHash('sha256').update(opts.accessToken).digest())
  return key.jwt({ typ: 'dpop+jwt', jwk: key.bareJwk() }, payload)
}

/**
 * The `atproto-space` HTTP message signature headers. Without `audience`
 * (exchanging a delegation token) the signature covers `authorization` and
 * names the key; with it (using a credential) it covers `authorization` and
 * `atproto-space-audience`.
 */
export function spaceSignatureHeaders(key: P256Key, authorization: string, audience?: string): Record<string, string> {
  const input =
    audience === undefined ? `("authorization");keyid="${key.didKey()}"` : '("authorization" "atproto-space-audience")'
  const lines = [`"authorization": ${authorization}`]
  if (audience !== undefined) lines.push(`"atproto-space-audience": ${audience}`)
  lines.push(`"@signature-params": ${input}`)
  const signature = key.sign(lines.join('\n')).toString('base64')
  return {
    authorization,
    ...(audience === undefined ? {} : { 'atproto-space-audience': audience }),
    'signature-input': `atproto-space=${input}`,
    signature: `atproto-space=:${signature}:`,
  }
}

/** Decodes a JWT's payload without checking it. */
export function jwtPayload(jwt: string): Record<string, unknown> {
  return JSON.parse(Buffer.from(jwt.split('.')[1] ?? '', 'base64url').toString('utf8'))
}
