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
const id = 'app.eventside.conference.join'

export type QueryParams = {}

export interface InputSchema {
  /** The conference's space URI. */
  conference: string
  code?: string
  /** The app joining someone as they open the conference's page: admits only someone the conference has never decided anything about. */
  onOpen?: boolean
}

export interface OutputSchema {
  status: 'joined' | 'pending' | 'emailNeeded' | 'refused' | (string & {})
  conference?: string
}

export interface HandlerInput {
  encoding: 'application/json'
  body: InputSchema
}

export interface HandlerSuccess {
  encoding: 'application/json'
  body: OutputSchema
  headers?: { [key: string]: string }
}

export interface HandlerError {
  status: number
  message?: string
  error?: 'NotFound' | 'RateLimitExceeded'
}

export type HandlerOutput = HandlerError | HandlerSuccess
