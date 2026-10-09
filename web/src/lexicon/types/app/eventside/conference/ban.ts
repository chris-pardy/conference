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
import type * as AppEventsideConferenceDefs from './defs.js'

const is$typed = _is$typed,
  validate = _validate
const id = 'app.eventside.conference.ban'

export interface Main {
  $type: 'app.eventside.conference.ban'
  /** The conference space this record is about. Signed with it, so it counts only when read from that space. */
  space: string
  subject: string
  decidedBy: string
  decidedRank: AppEventsideConferenceDefs.Rank
  seq: number
  decidedAt: string
  createdAt: string
  signatures: AppEventsideConferenceDefs.Signature[]
  [k: string]: unknown
}

const hashMain = 'main'

export function isMain<V>(v: V) {
  return is$typed(v, id, hashMain)
}

export function validateMain<V>(v: V) {
  return validate<Main & V>(v, id, hashMain, true)
}

export {
  type Main as Record,
  isMain as isRecord,
  validateMain as validateRecord,
}
