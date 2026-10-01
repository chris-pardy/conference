import { readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { afterAll, expect, test } from 'vitest'
import { copyProject, type ProjectCopy } from './copy.ts'
import { run } from './run.ts'

let copy: ProjectCopy | undefined
afterAll(() => copy?.remove())

test('TC-3: generated types cannot drift from the lexicons', async () => {
  copy = await copyProject()
  const lexicon = join(copy.dir, 'lexicons/app/eventside/block/card.json')
  const doc = JSON.parse(readFileSync(lexicon, 'utf8'))
  doc.defs.main.record.properties.pinnedUntil = { type: 'string', format: 'datetime' }
  writeFileSync(lexicon, `${JSON.stringify(doc, null, 2)}\n`)

  const build = await run('pnpm', ['run', 'check:build'], { cwd: copy.dir, env: copy.env, timeoutMs: 280_000 })

  expect(build.code, `check:build should fail on stale generated types\n${build.output}`).not.toBe(0)
  expect(build.output).toMatch(/generated.*out of date|out of date.*generated/i)
})
