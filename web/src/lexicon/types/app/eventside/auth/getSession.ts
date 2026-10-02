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
const id = 'app.eventside.auth.getSession'

export type QueryParams = {}
export type InputSchema = undefined

export interface OutputSchema {
  did: string
  handle: string
  displayName?: string
  avatar?: string
  /** The OAuth scopes the session was granted. */
  scopes: string[]
  /** Sent as X-CSRF-Token on every state-changing request. */
  csrfToken: string
}

export type HandlerInput = void

export interface HandlerSuccess {
  encoding: 'application/json'
  body: OutputSchema
  headers?: { [key: string]: string }
}

export interface HandlerError {
  status: number
  message?: string
  error?: 'AuthRequired' | 'SessionExpired'
}

export type HandlerOutput = HandlerError | HandlerSuccess
