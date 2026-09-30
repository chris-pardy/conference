import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { expect } from 'vitest'
import { parse } from 'yaml'

export const ROOT = resolve(import.meta.dirname, '../..')

export function readProjectFile(path: string): string {
  const full = resolve(ROOT, path)
  expect(existsSync(full), `${path} should exist`).toBe(true)
  return readFileSync(full, 'utf8')
}

// biome-ignore lint/suspicious/noExplicitAny: parsed YAML/JSON is untyped
export type Json = any

export const workflow = (): Json => parse(readProjectFile('.github/workflows/ci.yml'))
export const ruleset = (): Json => JSON.parse(readProjectFile('.github/rulesets/main.json'))
export const packageJson = (): Json => JSON.parse(readProjectFile('package.json'))

/** Every step of every job, with the job it belongs to. */
export function allSteps(wf: Json): { job: string; step: Json }[] {
  return Object.entries(wf.jobs ?? {}).flatMap(([job, def]: [string, Json]) =>
    (def.steps ?? []).map((step: Json) => ({ job, step })),
  )
}

/** The name GitHub reports a job's check under. */
export const checkName = (key: string, def: Json): string => def.name ?? key
