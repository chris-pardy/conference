import { appendFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { afterEach, beforeEach, describe, expect, test } from 'vitest'
import { type ProjectCopy, copyProject } from './copy.ts'
import { run } from './run.ts'

// These tests break a throwaway copy of the project, never the real one.
describe('lint and build gates', () => {
  let copy: ProjectCopy

  beforeEach(async () => {
    copy = await copyProject()
  })
  afterEach(() => copy.remove())

  const check = (script: string) => run('pnpm', ['run', script], { cwd: copy.dir, env: copy.env })

  test('TC-14: badly formatted Rust fails the lint step', async () => {
    const clean = await check('check:lint')
    expect(clean.code, `the unbroken project should pass lint:\n${clean.output}`).toBe(0)

    appendFileSync(join(copy.dir, 'crates/server/src/lib.rs'), '\npub fn   squashed( )->u8{1}\n')
    const broken = await check('check:lint')
    expect(broken.code).not.toBe(0)
    expect(broken.output).toContain('crates/server/src/lib.rs')
  })

  test('TC-15: a Rust compiler lint warning fails the lint step', async () => {
    const clean = await check('check:lint')
    expect(clean.code, `the unbroken project should pass lint:\n${clean.output}`).toBe(0)

    // Formatted correctly, so only clippy can object.
    appendFileSync(
      join(copy.dir, 'crates/server/src/lib.rs'),
      '\npub fn needless() -> u8 {\n    let x = 1;\n    return x;\n}\n',
    )
    const broken = await check('check:lint')
    expect(broken.code).not.toBe(0)
    expect(broken.output).toMatch(/needless_return|let_and_return/)
  })

  test('TC-16: a TypeScript lint or format problem fails the lint step', async () => {
    const clean = await check('check:lint')
    expect(clean.code, `the unbroken project should pass lint:\n${clean.output}`).toBe(0)

    writeFileSync(join(copy.dir, 'web/src/broken.ts'), 'export function stop() {\n  debugger\n}\n')
    const broken = await check('check:lint')
    expect(broken.code).not.toBe(0)
    expect(broken.output).toContain('web/src/broken.ts')
  })

  test('TC-17: a type error fails the build step', async () => {
    const clean = await check('check:build')
    expect(clean.code, `the unbroken project should build:\n${clean.output}`).toBe(0)

    writeFileSync(join(copy.dir, 'web/src/broken.ts'), "export const count: number = 'not a number'\n")
    const broken = await check('check:build')
    expect(broken.code).not.toBe(0)
    expect(broken.output).toMatch(/TS2322|not assignable to type 'number'/)
  })
})
