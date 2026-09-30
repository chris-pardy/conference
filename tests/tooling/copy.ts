import { cpSync, mkdirSync, mkdtempSync, rmSync, symlinkSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { run } from './run.ts'

const ROOT = resolve(import.meta.dirname, '../..')

export interface ProjectCopy {
  dir: string
  env: NodeJS.ProcessEnv
  remove(): void
}

/**
 * Copies the project's tracked and unignored files to a throwaway directory,
 * sharing installed dependencies and the Rust build cache, so a check can be
 * run against deliberately broken code without touching the real repo.
 */
export async function copyProject(): Promise<ProjectCopy> {
  const dir = mkdtempSync(join(tmpdir(), 'conference-copy-'))
  const listed = await run('git', ['ls-files', '-co', '--exclude-standard', '-z'], { cwd: ROOT })
  for (const file of listed.stdout.split('\0').filter(Boolean)) {
    mkdirSync(dirname(join(dir, file)), { recursive: true })
    try {
      cpSync(join(ROOT, file), join(dir, file))
    } catch {
      // listed but deleted in the working tree
    }
  }
  symlinkSync(join(ROOT, 'node_modules'), join(dir, 'node_modules'))
  symlinkSync(join(ROOT, 'web/node_modules'), join(dir, 'web/node_modules'))
  const env = { ...process.env, CARGO_TARGET_DIR: join(ROOT, 'target/copies') }
  return { dir, env, remove: () => rmSync(dir, { recursive: true, force: true }) }
}
