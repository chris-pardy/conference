// Checking an eventside signature the way any other app would, following the
// badge.blue attestation spec (Nick Gerakines' "ATProtocol Attestation
// Specification") as features/conference-space.md ("Signed decisions")
// describes it:
//
//   1. Copy the `signatures` entry and delete `signature`: that copy is `$sig`.
//   2. Add `repository`, the DID of the repo the record was read from.
//   3. Remove `signatures` from the record and add `$sig`.
//   4. Encode as canonical DAG-CBOR and build a CIDv1 (dag-cbor, sha2-256).
//   5. Check the low-S P-256 signature over the 36 CID bytes against the key
//      the entry names, as the authority's DID document lists it now.
//
// Node's own crypto only, with a small DAG-CBOR encoder, so the tests add no
// dependencies.
import { createHash, createPublicKey, verify } from 'node:crypto'
import type { Json } from './atproto.ts'

/** One `signatures` entry, and whether it verifies. */
export interface SignatureCheck {
  /** The key the entry names: `did:plc:…#eventside_attest…`. */
  key: string
  valid: boolean
  /** Why it doesn't verify. */
  reason?: string
}

/**
 * Checks every signature on a record, as read from `repository` in `space`,
 * against the space authority's DID document as it is now.
 */
export function checkSignatures(
  record: Json,
  where: { repository: string; space: string; authorityDoc: Json },
): SignatureCheck[] {
  const entries: Json[] = Array.isArray(record.signatures) ? record.signatures : []
  return entries.map((entry) => checkOne(record, entry, where))
}

/** The keys whose signatures on a record verify, sorted. */
export function validKeys(record: Json, where: { repository: string; space: string; authorityDoc: Json }): string[] {
  return checkSignatures(record, where)
    .filter((check) => check.valid)
    .map((check) => check.key)
    .sort()
}

function checkOne(
  record: Json,
  entry: Json,
  where: { repository: string; space: string; authorityDoc: Json },
): SignatureCheck {
  const key = String(entry.key ?? '')
  const fail = (reason: string): SignatureCheck => ({ key, valid: false, reason })
  const authority = String(where.authorityDoc.id)
  const [did, fragment] = key.split('#')
  if (did !== authority) return fail(`key ${key} isn't the authority's (${authority})`)
  if (!fragment?.startsWith('eventside_attest')) return fail(`key fragment #${fragment} isn't an eventside_attest key`)
  if (entry.space !== where.space) return fail(`signed for space ${entry.space}, read from ${where.space}`)
  const method = ((where.authorityDoc.verificationMethod ?? []) as Json[]).find(
    (m) => m.id === key || m.id === `#${fragment}`,
  )
  if (!method) return fail(`the DID document lists no #${fragment}`)
  const signature = entry.signature?.$bytes
  if (typeof signature !== 'string') return fail('no signature bytes')
  const sig = Buffer.from(signature, 'base64')
  if (sig.length !== 64) return fail(`signature is ${sig.length} bytes, not 64`)
  if (BigInt(`0x${sig.subarray(32).toString('hex')}`) > P256_HALF_N) return fail('signature is high-S')
  let publicKey: ReturnType<typeof createPublicKey>
  try {
    publicKey = p256PublicKey(String(method.publicKeyMultibase))
  } catch (err) {
    return fail(String(err))
  }
  const cid = attestationCid(record, entry, where.repository)
  const ok = verify('sha256', cid, { key: publicKey, dsaEncoding: 'ieee-p1363' }, sig)
  return ok ? { key, valid: true } : fail('signature doesn’t verify')
}

/** The 36-byte CIDv1 (dag-cbor, sha2-256) a `signatures` entry signs. */
export function attestationCid(record: Json, entry: Json, repository: string): Buffer {
  const { signature: _signature, ...rest } = entry
  const $sig = { ...rest, repository }
  const { signatures: _signatures, ...unsigned } = record
  const bytes = dagCbor({ ...unsigned, $sig })
  const digest = createHash('sha256').update(bytes).digest()
  return Buffer.concat([Buffer.from([0x01, 0x71, 0x12, 0x20]), digest])
}

// P-256's group order, halved: a low-S signature's s is at most this.
const P256_HALF_N = BigInt('0xFFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551') / 2n

/** A multikey (`z…`, p256-pub multicodec 0x1200, compressed point) as a Node public key. */
export function p256PublicKey(multibase: string): ReturnType<typeof createPublicKey> {
  if (!multibase.startsWith('z')) throw new Error(`not a base58btc multikey: ${multibase}`)
  const bytes = base58btcDecode(multibase.slice(1))
  if (bytes[0] !== 0x80 || bytes[1] !== 0x24) {
    throw new Error(`not a P-256 multikey (multicodec ${bytes.subarray(0, 2).toString('hex')})`)
  }
  const point = bytes.subarray(2)
  if (point.length !== 33) throw new Error(`a compressed P-256 point is 33 bytes, not ${point.length}`)
  // SubjectPublicKeyInfo for id-ecPublicKey on prime256v1, with the compressed point.
  const spki = Buffer.concat([Buffer.from('3039301306072a8648ce3d020106082a8648ce3d030107032200', 'hex'), point])
  return createPublicKey({ key: spki, format: 'der', type: 'spki' })
}

const B58 = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'

function base58btcDecode(text: string): Buffer {
  let n = 0n
  for (const ch of text) {
    const digit = B58.indexOf(ch)
    if (digit < 0) throw new Error(`not base58btc: ${text}`)
    n = n * 58n + BigInt(digit)
  }
  let hex = n === 0n ? '' : n.toString(16)
  if (hex.length % 2) hex = `0${hex}`
  const zeros = text.length - text.replace(/^1+/, '').length
  return Buffer.concat([Buffer.alloc(zeros), Buffer.from(hex, 'hex')])
}

const B32 = 'abcdefghijklmnopqrstuvwxyz234567'

/** A CID string (multibase base32, `b…`) to its bytes. */
function cidBytes(cid: string): Buffer {
  if (!cid.startsWith('b')) throw new Error(`not a base32 CID: ${cid}`)
  const out: number[] = []
  let bits = 0
  let value = 0
  for (const ch of cid.slice(1)) {
    const digit = B32.indexOf(ch)
    if (digit < 0) throw new Error(`not base32: ${cid}`)
    value = (value << 5) | digit
    bits += 5
    if (bits >= 8) {
      bits -= 8
      out.push((value >> bits) & 0xff)
    }
  }
  return Buffer.from(out)
}

/**
 * Canonical DAG-CBOR for a record in atproto's JSON form: `{$bytes}` is a
 * byte string and `{$link}` a CID (tag 42). Map keys sort by length, then
 * bytewise; integers take their shortest form; floats aren't allowed.
 */
export function dagCbor(value: unknown): Buffer {
  const parts: Buffer[] = []
  encode(value, parts)
  return Buffer.concat(parts)
}

function head(major: number, n: number | bigint, parts: Buffer[]) {
  const big = BigInt(n)
  const m = major << 5
  if (big < 24n) parts.push(Buffer.from([m | Number(big)]))
  else if (big < 0x100n) parts.push(Buffer.from([m | 24, Number(big)]))
  else if (big < 0x10000n) {
    const b = Buffer.alloc(3)
    b[0] = m | 25
    b.writeUInt16BE(Number(big), 1)
    parts.push(b)
  } else if (big < 0x100000000n) {
    const b = Buffer.alloc(5)
    b[0] = m | 26
    b.writeUInt32BE(Number(big), 1)
    parts.push(b)
  } else {
    const b = Buffer.alloc(9)
    b[0] = m | 27
    b.writeBigUInt64BE(big, 1)
    parts.push(b)
  }
}

function encode(value: unknown, parts: Buffer[]): void {
  if (value === null) {
    parts.push(Buffer.from([0xf6]))
  } else if (value === true || value === false) {
    parts.push(Buffer.from([value ? 0xf5 : 0xf4]))
  } else if (typeof value === 'number') {
    if (!Number.isSafeInteger(value)) throw new Error(`DAG-CBOR in atproto has no floats: ${value}`)
    if (value >= 0) head(0, value, parts)
    else head(1, -1 - value, parts)
  } else if (typeof value === 'string') {
    const bytes = Buffer.from(value, 'utf8')
    head(3, bytes.length, parts)
    parts.push(bytes)
  } else if (Buffer.isBuffer(value) || value instanceof Uint8Array) {
    head(2, value.length, parts)
    parts.push(Buffer.from(value))
  } else if (Array.isArray(value)) {
    head(4, value.length, parts)
    for (const item of value) encode(item, parts)
  } else if (typeof value === 'object') {
    const obj = value as Record<string, unknown>
    const keys = Object.keys(obj).filter((k) => obj[k] !== undefined)
    if (keys.length === 1 && keys[0] === '$bytes' && typeof obj.$bytes === 'string') {
      encode(Buffer.from(obj.$bytes, 'base64'), parts)
      return
    }
    if (keys.length === 1 && keys[0] === '$link' && typeof obj.$link === 'string') {
      const cid = Buffer.concat([Buffer.from([0x00]), cidBytes(obj.$link)])
      parts.push(Buffer.from([0xd8, 0x2a]))
      head(2, cid.length, parts)
      parts.push(cid)
      return
    }
    const sorted = keys
      .map((k) => ({ k, b: Buffer.from(k, 'utf8') }))
      .sort((a, b) => a.b.length - b.b.length || Buffer.compare(a.b, b.b))
    head(5, sorted.length, parts)
    for (const { k, b } of sorted) {
      head(3, b.length, parts)
      parts.push(b)
      encode(obj[k], parts)
    }
  } else {
    throw new Error(`can’t encode ${typeof value} as DAG-CBOR`)
  }
}
