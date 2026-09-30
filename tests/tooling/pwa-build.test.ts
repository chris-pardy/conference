import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterAll, expect, test } from 'vitest'
import { ROOT } from './project-files.ts'
import { run } from './run.ts'

const outDir = mkdtempSync(join(tmpdir(), 'pwa-build-'))
afterAll(() => rmSync(outDir, { recursive: true, force: true }))

test('TC-8: the production build is an installable PWA', async () => {
  const build = await run('pnpm', ['--filter', '@conference/web', 'exec', 'vite', 'build', '--outDir', outDir, '--emptyOutDir'], {
    cwd: ROOT,
  })
  expect(build.code, build.output).toBe(0)

  const html = readFileSync(join(outDir, 'index.html'), 'utf8')
  const href = /<link[^>]*rel="manifest"[^>]*href="([^"]+)"/.exec(html)?.[1]
  expect(href, 'index.html should link a web app manifest').toBeDefined()

  const manifest = JSON.parse(readFileSync(join(outDir, (href as string).replace(/^\//, '')), 'utf8'))
  expect(manifest.name).toBeTypeOf('string')
  expect(manifest.name.length).toBeGreaterThan(0)
  expect(manifest.start_url).toBeTypeOf('string')
  expect(manifest.display).toBe('standalone')
  expect(manifest.icons.length).toBeGreaterThan(0)
  for (const icon of manifest.icons) {
    expect(existsSync(join(outDir, icon.src.replace(/^\//, ''))), `icon ${icon.src} should be in the build`).toBe(true)
  }

  expect(readdirSync(outDir)).toContain('sw.js')
})
