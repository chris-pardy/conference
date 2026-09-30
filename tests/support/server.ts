export interface RunningServer {
  url: string
  stop(): Promise<void>
}

/** Spawns the backend binary with extra env, resolving once it announces its URL. */
export async function spawnServer(_env: Record<string, string> = {}): Promise<RunningServer> {
  throw new Error('not implemented')
}
