import {expect, test} from 'bun:test'
import {readdirSync, readFileSync} from 'node:fs'
import {join} from 'node:path'

const SOURCE_DIR = join(import.meta.dir, '..')

const sourceFiles = (dir: string): string[] =>
  readdirSync(dir, {withFileTypes: true}).flatMap(entry => (entry.isDirectory() ? sourceFiles(join(dir, entry.name)) : entry.name.endsWith('.ts') ? [join(dir, entry.name)] : []))

test('the testing folder holds the shared fixtures', () => {
  expect(readdirSync(import.meta.dir)).toContain('fixtures.ts')
})

test('no production file imports from the testing folder', () => {
  const production = sourceFiles(SOURCE_DIR).filter(file => !file.endsWith('.test.ts') && !file.startsWith(import.meta.dir))
  const offenders = production.filter(file => /from '(\.\.?\/)+(testing\/|.*\/testing\/)/.test(readFileSync(file, 'utf8')))
  expect(offenders).toEqual([])
})
