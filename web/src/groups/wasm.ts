// Runs a card's wasm module. The ABI (see cards/abi/src/lib.rs):
// exports `memory`, `alloc(len) -> ptr` and `update(ptr, len) -> i64`,
// JSON in, JSON out, packed as (ptr << 32) | len.

import type { Card as CardUi } from '../blocks/BlockCard'

export type ModuleEvent =
  | { type: 'init' }
  | { type: 'data' }
  | { type: 'action'; blockId: string; actionId: string; value: unknown; item?: unknown }

export interface ModuleInput {
  event: ModuleEvent
  ui: CardUi
  base: CardUi
  data: unknown
  local: unknown
  viewer: { did: string; handle: string }
}

export interface ModuleOutput {
  ui?: CardUi
  local?: unknown
  /** Whether an action also goes to the server, or what to send instead. */
  forward?: boolean | { actionId?: string; value?: unknown }
}

interface Exports {
  memory: WebAssembly.Memory
  alloc(len: number): number
  update(ptr: number, len: number): bigint
}

export class CardModule {
  private constructor(private readonly exports: Exports) {}

  static async load(url: string): Promise<CardModule> {
    const res = await fetch(url, { credentials: 'same-origin' })
    if (!res.ok) throw new Error(`module: HTTP ${res.status}`)
    // No imports: a module can't reach the network, the DOM or a clock.
    const { instance } = await WebAssembly.instantiate(await res.arrayBuffer(), {})
    const exports = instance.exports as unknown as Exports
    if (!(exports.memory instanceof WebAssembly.Memory) || typeof exports.update !== 'function') {
      throw new Error('module: missing memory, alloc or update')
    }
    return new CardModule(exports)
  }

  update(input: ModuleInput): ModuleOutput {
    const seed = crypto.getRandomValues(new Uint32Array(1))[0]
    const bytes = new TextEncoder().encode(JSON.stringify({ ...input, seed }))
    const ptr = this.exports.alloc(bytes.length)
    new Uint8Array(this.exports.memory.buffer, ptr, bytes.length).set(bytes)
    const packed = this.exports.update(ptr, bytes.length)
    const outPtr = Number(packed >> 32n)
    const outLen = Number(packed & 0xffffffffn)
    const out = new TextDecoder().decode(new Uint8Array(this.exports.memory.buffer, outPtr, outLen))
    const parsed = JSON.parse(out)
    return parsed && typeof parsed === 'object' ? parsed : {}
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
