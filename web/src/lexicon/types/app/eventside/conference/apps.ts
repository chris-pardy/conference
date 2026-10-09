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
const id = 'app.eventside.conference.apps'

export interface Main {
  $type: 'app.eventside.conference.apps'
  /** The conference space this record is about. Signed with it, so it counts only when read from that space. */
  space: string
  apps: App[]
  /** The version of the conference's allowed apps; only the latest counts. */
  seq: number
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

export interface App {
  $type?: 'app.eventside.conference.apps#app'
  /** An OAuth client ID. */
  client?: string
  service?: string
  uses: ('read' | 'cardProvider' | 'feedGenerator' | (string & {}))[]
}

const hashApp = 'app'

export function isApp<V>(v: V) {
  return is$typed(v, id, hashApp)
}

export function validateApp<V>(v: V) {
  return validate<App & V>(v, id, hashApp)
}
