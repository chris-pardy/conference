// Brings in vivarium's `vivariumUrl` provided-context type.
import type {} from '@vivarium-dev/client/vitest/global-setup'
import type { TestProject } from 'vitest/node'
import { spawnServer } from './server.ts'

declare module 'vitest' {
  export interface ProvidedContext {
    serverUrl: string
  }
}

/** Starts one backend for the run, pointed at the run's vivarium, and provides its URL. */
export default async function setup(project: TestProject): Promise<() => Promise<void>> {
  const vivariumUrl = process.env.VIVARIUM_URL ?? project.getProvidedContext().vivariumUrl
  const server = await spawnServer({ ATPROTO_URL: vivariumUrl })
  project.provide('serverUrl', server.url)
  return () => server.stop()
}
