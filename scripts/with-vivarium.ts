// Runs a command with one sealed vivarium box, exported as VIVARIUM_URL, so
// every test runner in the command shares the same box. Attaches to the box in
// VIVARIUM_URL when there already is one, and leaves it running.
//
// Usage: node scripts/with-vivarium.ts <command> [args...]
import { spawn } from 'node:child_process'
import { constants } from 'node:os'
import { startVivarium } from '@vivarium-dev/client'

const [command, ...args] = process.argv.slice(2)
if (!command) {
  console.error('usage: node scripts/with-vivarium.ts <command> [args...]')
  process.exit(2)
}

const box = await startVivarium({ upstream: false })

const child = spawn(command, args, {
  stdio: 'inherit',
  env: { ...process.env, VIVARIUM_URL: box.url },
})

const forward = (signal: NodeJS.Signals) => child.kill(signal)
process.on('SIGINT', forward)
process.on('SIGTERM', forward)

const code = await new Promise<number>((resolve) => {
  child.on('error', (err) => {
    console.error(`with-vivarium: could not run ${command}: ${err.message}`)
    resolve(127)
  })
  // A child killed by a signal reports the shell convention, 128 + the signal number.
  child.on('exit', (exitCode, signal) => resolve(exitCode ?? 128 + (signal ? constants.signals[signal] : 0)))
})

// Only stops a box this run started; an attached box keeps running.
try {
  await box.stop()
} catch (err) {
  console.error(`with-vivarium: failed to stop the box: ${(err as Error).message}`)
}
process.exit(code)
