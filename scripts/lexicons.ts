// Generates the TS types for our lexicons into web/src/lexicon, or checks that
// the committed ones are up to date. `check:build` runs the check, so the
// generated types never drift from lexicons/.
//
// Usage: node scripts/lexicons.ts gen | check
import { spawnSync } from 'node:child_process'
import { cpSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, relative, resolve } from 'node:path'

const ROOT = resolve(import.meta.dirname, '..')
const LEXICONS = join(ROOT, 'lexicons')
const OUT = join(ROOT, 'web/src/lexicon')

const mode = process.argv[2]
if (mode !== 'gen' && mode !== 'check') {
  console.error('usage: node scripts/lexicons.ts gen | check')
  process.exit(2)
}

function files(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name)
    return statSync(path).isDirectory() ? files(path) : [path]
  })
}

/** Runs lex-cli into a fresh directory and returns it. */
function generate(): string {
  const dir = mkdtempSync(join(tmpdir(), 'lexicon-gen-'))
  const lexicons = files(LEXICONS)
    .filter((f) => f.endsWith('.json'))
    .sort()
  const gen = spawnSync('pnpm', ['exec', 'lex', 'gen-server', '--yes', dir, ...lexicons], {
    cwd: ROOT,
    encoding: 'utf8',
  })
  if (gen.status !== 0) {
    console.error(gen.stdout, gen.stderr)
    console.error('lexicons: lex-cli failed')
    process.exit(1)
  }
  // Only the types and schemas: the XRPC server stub isn't ours to use.
  rmSync(join(dir, 'index.ts'))
  return dir
}

const fresh = generate()
try {
  if (mode === 'gen') {
    rmSync(OUT, { recursive: true, force: true })
    cpSync(fresh, OUT, { recursive: true })
    console.log(`lexicons: generated ${relative(ROOT, OUT)}`)
  } else {
    const rel = (base: string) => new Set(files(base).map((f) => relative(base, f)))
    let committed = new Set<string>()
    try {
      committed = rel(OUT)
    } catch {
      // nothing generated yet
    }
    const expected = rel(fresh)
    const stale = [...new Set([...committed, ...expected])]
      .sort()
      .filter(
        (f) =>
          !committed.has(f) ||
          !expected.has(f) ||
          readFileSync(join(OUT, f), 'utf8') !== readFileSync(join(fresh, f), 'utf8'),
      )
    if (stale.length > 0) {
      console.error(`lexicons: the generated types are out of date with lexicons/ (${stale.join(', ')}).`)
      console.error('lexicons: run `pnpm lex:gen` and commit the result.')
      process.exitCode = 1
    } else {
      console.log('lexicons: generated types are up to date')
    }
  }
} finally {
  rmSync(fresh, { recursive: true, force: true })
}
