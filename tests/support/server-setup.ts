import type { TestProject } from 'vitest/node'

declare module 'vitest' {
  export interface ProvidedContext {
    serverUrl: string
  }
}

// not implemented: should start the backend against vivarium and provide `serverUrl`
export default async function setup(_project: TestProject): Promise<void> {}
