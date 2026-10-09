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
const id = 'app.eventside.conference.getMembership'

export type QueryParams = {
  /** The conference's space URI. */
  conference: string
}
export type InputSchema = undefined
export type OutputSchema = AppEventsideConferenceDefs.Viewer
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
