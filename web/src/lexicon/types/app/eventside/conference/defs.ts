/**
 * GENERATED CODE - DO NOT MODIFY
 */
import { type ValidationResult, BlobRef } from '@atproto/lexicon'
import { CID } from 'multiformats/cid'
import { validate as _validate } from '../../../../lexicons.js'
import {
  type $Typed,
  is$typed as _is$typed,
  type OmitKey,
} from '../../../../util.js'

const is$typed = _is$typed,
  validate = _validate
const id = 'app.eventside.conference.defs'

/** An inline attestation (badge.blue format) by an #eventside_attest key in the signer's DID document. It signs the CIDv1 (dag-cbor, sha2-256) of the record without `signatures`, plus `$sig`: this entry without `signature`, plus `repository`, the DID of the repo holding the record. */
export interface Signature {
  $type?: 'app.eventside.conference.defs#signature'
  /** The signing key, as `{signer did}#eventside_attest…`. The signer is the conference space's managing app. */
  key: string
  signedAt: string
  /** A low-S P-256 signature, 64 bytes r||s. */
  signature: Uint8Array
}

const hashSignature = 'signature'

export function isSignature<V>(v: V) {
  return is$typed(v, id, hashSignature)
}

export function validateSignature<V>(v: V) {
  return validate<Signature & V>(v, id, hashSignature)
}

export type Role = 'owner' | 'staff' | 'speaker' | 'attendee' | (string & {})
/** The weight a decision was made with, kept whatever happens to its maker's role later. */
export type Rank = 'owner' | 'staff' | 'self' | (string & {})

export interface Viewer {
  $type?: 'app.eventside.conference.defs#viewer'
  member: boolean
  role?: Role
}

const hashViewer = 'viewer'

export function isViewer<V>(v: V) {
  return is$typed(v, id, hashViewer)
}

export function validateViewer<V>(v: V) {
  return validate<Viewer & V>(v, id, hashViewer)
}
