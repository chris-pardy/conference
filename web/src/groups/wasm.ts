// Runs a card's wasm module, ABI v2 (see DEMO.md and cards/abi): exports
// `memory`, `alloc(len) -> ptr`, `reduce(ptr, len) -> i64` and
// `view(ptr, len) -> i64`. JSON in, JSON out, packed as (ptr << 32) | len.

import type { Card as CardUi } from '../blocks/BlockCard'

export type Env = 'client' | 'server'

export interface ActionMeta {
  id?: string
  actor?: { did: string; handle: string; displayName?: string | null }
  at?: number
  origin?: 'ui' | 'client' | 'server' | 'platform'
  blockId?: string
  item?: unknown
  [key: string]: unknown
}

export interface Action {
  type: string
  payload?: unknown
  meta?: ActionMeta
  /** On an action the server emits: the one person it's for. */
  to?: string
}

export interface ReduceInput {
  env: Env
  state: unknown
  shared: unknown
  action: Action
  config: unknown
  base: CardUi
  context: unknown
  viewer?: { did: string; handle: string }
}

export interface ReduceOutput {
  /** Absent means unchanged. */
  state?: unknown
  actions?: Action[]
}

export interface ViewInput {
  shared: unknown
  local: unknown
  config: unknown
  base: CardUi
  context: unknown
  viewer: { did: string; handle: string }
}

export interface ViewOutput {
  ui?: CardUi
  data?: unknown
}

interface Exports {
  memory: WebAssembly.Memory
  alloc(len: number): number
  reduce(ptr: number, len: number): bigint
  view(ptr: number, len: number): bigint
}

export class CardModule {
  private constructor(private readonly exports: Exports) {}

  static async load(url: string): Promise<CardModule> {
    const res = await fetch(url, { credentials: 'same-origin' })
    if (!res.ok) throw new Error(`module: HTTP ${res.status}`)
    // No imports: a module can't reach the network, the DOM or a clock.
    const { instance } = await WebAssembly.instantiate(await res.arrayBuffer(), {})
    const exports = instance.exports as unknown as Exports
    if (
      !(exports.memory instanceof WebAssembly.Memory) ||
      typeof exports.alloc !== 'function' ||
      typeof exports.reduce !== 'function' ||
      typeof exports.view !== 'function'
    ) {
      throw new Error('module: missing memory, alloc, reduce or view')
    }
    return new CardModule(exports)
  }

  reduce(input: ReduceInput): ReduceOutput {
    return this.call(this.exports.reduce, input)
  }

  view(input: ViewInput): ViewOutput {
    return this.call(this.exports.view, input)
  }

  private call<T>(fn: (ptr: number, len: number) => bigint, input: object): T {
    // A module has no clock or RNG of its own: the host passes them in.
    const seed = crypto.getRandomValues(new Uint32Array(1))[0]
    const bytes = new TextEncoder().encode(JSON.stringify({ ...input, seed, now: Date.now() }))
    const ptr = this.exports.alloc(bytes.length)
    new Uint8Array(this.exports.memory.buffer, ptr, bytes.length).set(bytes)
    const packed = fn(ptr, bytes.length)
    const outPtr = Number(packed >> 32n)
    const outLen = Number(packed & 0xffffffffn)
    const out = new TextDecoder().decode(new Uint8Array(this.exports.memory.buffer, outPtr, outLen))
    const parsed = JSON.parse(out)
    return (parsed && typeof parsed === 'object' ? parsed : {}) as T
  }
}

const loaded = new Map<string, Promise<CardModule>>()

/** The module at a URL, loaded once. */
export function loadModule(url: string): Promise<CardModule> {
  let found = loaded.get(url)
  if (!found) {
    found = CardModule.load(url)
    found.catch(() => loaded.delete(url))
    loaded.set(url, found)
  }
  return found
}
